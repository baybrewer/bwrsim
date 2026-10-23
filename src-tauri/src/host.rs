//! Host-side save/load protocol (plan C2) and the worker clock (plan C3), kept
//! free of Tauri types so the command handlers stay thin and this is testable.

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

use bwr_core::api::{AnnEvent, AnnEventsSince, RunChanged};
use bwr_core::{Command, Sim, Snapshot};
use serde::Serialize;
use serde_json::Value;

use crate::saves::{random_u64, SaveContent, SaveList, SaveMeta, SaveStore};

/// Exact error string the UI matches on (plan C2a).
pub const RUN_CHANGED: &str = "RunChanged";
pub const STATE_CHANGED: &str = "state changed since Save was opened";

/// Most wall-clock time handed to `Sim::advance` per worker iteration, s. At the
/// maximum speed (100x) this is 50 simulated seconds, well below the step cap
/// inside `Sim::advance`; any excess waits in the backlog for the next iteration.
const MAX_ADVANCE_PER_ITER_S: f64 = 0.5;

/// Smoothing time constant of the achieved-speed readout, wall-clock s.
const EFFECTIVE_SPEED_TAU_S: f64 = 1.0;

/// Wall-clock interval between published snapshots, s.
const EMIT_INTERVAL_S: f64 = 0.1;

pub fn lock<T>(m: &Mutex<T>) -> Result<MutexGuard<'_, T>, String> {
    m.lock().map_err(|e| e.to_string())
}

fn run_changed(_: RunChanged) -> String {
    RUN_CHANGED.to_string()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavePrepared {
    pub token: String,
    pub snapshot: Snapshot,
    pub events: Vec<AnnEvent>,
    pub was_paused: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadPrepared {
    pub token: String,
    /// The save's RunRecord, unvalidated; the UI validates it before committing.
    pub ui: Value,
    pub snapshot: Snapshot,
}

/// A restored sim waiting for the UI to accept its RunRecord.
struct Staged {
    token: String,
    sim: Sim,
}

/// Save/load over the shared sim. Lock order is always sim, then store.
pub struct SaveService {
    sim: Arc<Mutex<Sim>>,
    store: SaveStore,
    staged: Mutex<Option<Staged>>,
}

/// Save token: the (run, revision) the UI checkpointed, so any engine change
/// between prepare and write is detected.
fn save_token(sim: &Sim) -> String {
    format!("{}:{}", sim.run_id, sim.rev)
}

impl SaveService {
    pub fn new(sim: Arc<Mutex<Sim>>, store: SaveStore) -> Self {
        SaveService { sim, store, staged: Mutex::new(None) }
    }

    pub fn events_since(&self, run_id: u64, seq: u64) -> Result<AnnEventsSince, String> {
        lock(&self.sim)?.ann_events_since(run_id, seq).map_err(run_changed)
    }

    pub fn list(&self) -> SaveList {
        self.store.list()
    }

    pub fn save_prepare(&self, run_id: u64, since_seq: u64) -> Result<SavePrepared, String> {
        let mut sim = lock(&self.sim)?;
        if sim.run_id != run_id {
            return Err(RUN_CHANGED.into());
        }
        let was_paused = sim.paused;
        if !was_paused {
            sim.apply(Command::SetPaused { paused: true });
        }
        let snapshot = sim.snapshot();
        // Same lock as the snapshot, so these end exactly at its `annHead`.
        let events = sim.ann_events_since(run_id, since_seq).map_err(run_changed)?.events;
        Ok(SavePrepared { token: save_token(&sim), snapshot, events, was_paused })
    }

    pub fn save_write(&self, token: &str, id: Option<&str>, name: &str, ui: Value) -> Result<SaveMeta, String> {
        // Held across the write so the captured state is the one the token names.
        let sim = lock(&self.sim)?;
        if token != save_token(&sim) {
            return Err(STATE_CHANGED.into());
        }
        let content = SaveContent {
            sim_time_s: sim.snapshot().time_s,
            scenario: sim.scenario,
            run_summary: sim.summary(),
            engine: sim.to_state_json(),
            ui,
        };
        self.store.write(id, name, content)
    }

    /// Stage the save's engine state; the active sim is untouched. Replaces any
    /// older staged load (one at a time).
    pub fn load_prepare(&self, id: &str) -> Result<LoadPrepared, String> {
        let file = self.store.read(id)?;
        let sim = Sim::from_state_json(&file.engine)?;
        let token = format!("l_{:016x}", random_u64());
        let snapshot = sim.snapshot();
        *lock(&self.staged)? = Some(Staged { token: token.clone(), sim });
        Ok(LoadPrepared { token, ui: file.ui, snapshot })
    }

    /// Install the staged sim. The worker sees the new `run_id` and resets its clock (C3).
    pub fn load_commit(&self, token: &str) -> Result<Snapshot, String> {
        let staged = {
            let mut slot = lock(&self.staged)?;
            match slot.take() {
                Some(s) if s.token == token => s,
                other => {
                    *slot = other;
                    return Err("no staged load for this token".into());
                }
            }
        };
        let mut sim = lock(&self.sim)?;
        *sim = staged.sim;
        Ok(sim.snapshot())
    }

    /// Drop the staged load if it is this token's; a stale abort is harmless.
    pub fn load_abort(&self, token: &str) -> Result<(), String> {
        let mut slot = lock(&self.staged)?;
        if slot.as_ref().is_some_and(|s| s.token == token) {
            *slot = None;
        }
        Ok(())
    }

    pub fn delete(&self, id: &str) -> Result<(), String> {
        self.store.delete(id)
    }
}

/// Worker wall-clock state between iterations.
pub struct WorkerClock {
    last: Instant,
    last_emit: Instant,
    /// Wall-clock time not yet handed to the simulator. Work per iteration is
    /// bounded, but the remainder is carried instead of discarded so delayed
    /// wake-ups do not change the simulated duration.
    backlog: f64,
    /// Achieved-speed readout: simulated time gained per wall second between emits.
    effective_speed: f64,
    last_emit_time_s: Option<f64>,
    /// Run last advanced; a different one means a restore or scenario load.
    run_id: Option<u64>,
}

impl WorkerClock {
    pub fn new(now: Instant) -> Self {
        WorkerClock { last: now, last_emit: now, backlog: 0.0, effective_speed: 0.0, last_emit_time_s: None, run_id: None }
    }

    /// One worker iteration under the sim lock: advance, and return a snapshot
    /// when one is due for publishing.
    pub fn tick(&mut self, s: &mut Sim, now: Instant) -> Option<Snapshot> {
        if self.run_id != Some(s.run_id) {
            // Wall time that passed before this run existed (or while a load
            // waited for the lock) is not owed to it. The engine accumulator is
            // part of the restored state and is left alone.
            self.run_id = Some(s.run_id);
            self.last = now;
            self.backlog = 0.0;
            self.effective_speed = 0.0;
            self.last_emit_time_s = None;
        }
        self.backlog += now.saturating_duration_since(self.last).as_secs_f64();
        self.last = now;
        let dt = self.backlog.min(MAX_ADVANCE_PER_ITER_S);
        if s.paused {
            // Time spent paused is not owed to the simulation.
            self.backlog = 0.0;
        } else {
            s.advance(dt);
            self.backlog -= dt;
        }
        let since_emit = now.saturating_duration_since(self.last_emit).as_secs_f64();
        if since_emit < EMIT_INTERVAL_S {
            return None;
        }
        let mut snap = s.snapshot();
        // The first sample after start or resume seeds the filter.
        let alpha = if self.effective_speed == 0.0 { 1.0 } else { (since_emit / EFFECTIVE_SPEED_TAU_S).min(1.0) };
        match self.last_emit_time_s {
            _ if s.paused => self.effective_speed = 0.0,
            // A scenario load resets sim time; that window has no valid sample.
            Some(t0) if snap.time_s >= t0 => {
                let rate = (snap.time_s - t0) / since_emit;
                self.effective_speed += (rate - self.effective_speed) * alpha;
            }
            _ => {}
        }
        self.last_emit_time_s = Some(snap.time_s);
        self.last_emit = now;
        snap.effective_speed = self.effective_speed;
        snap.clock_backlog_s = s.backlog_s() + self.backlog * s.speed;
        Some(snap)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::saves::tests::TempDir;
    use bwr_core::api::{AnnKind, ScenarioName};
    use std::time::Duration;

    fn service(dir: &TempDir) -> SaveService {
        SaveService::new(Arc::new(Mutex::new(Sim::new(ScenarioName::HotStandby))), SaveStore::new(dir.0.clone()))
    }

    fn ui() -> Value {
        serde_json::json!({ "record": 1 })
    }

    fn saved(svc: &SaveService) -> String {
        let run_id = lock(&svc.sim).unwrap().run_id;
        let p = svc.save_prepare(run_id, 0).unwrap();
        svc.save_write(&p.token, None, "base", ui()).unwrap().id
    }

    #[test]
    fn save_prepare_pauses_and_returns_events_through_head() {
        let dir = TempDir::new();
        let svc = service(&dir);
        let run_id = lock(&svc.sim).unwrap().run_id;
        assert!(!lock(&svc.sim).unwrap().paused);
        assert_eq!(svc.save_prepare(run_id + 1000, 0).err().unwrap(), RUN_CHANGED);
        let p = svc.save_prepare(run_id, 0).unwrap();
        assert!(!p.was_paused && p.snapshot.paused && lock(&svc.sim).unwrap().paused);
        assert_eq!(p.events.last().map_or(0, |e| e.seq), p.snapshot.ann_head);
        assert!(svc.save_prepare(run_id, 0).unwrap().was_paused);
        assert_eq!(svc.events_since(run_id + 1000, 0).unwrap_err(), RUN_CHANGED);
    }

    #[test]
    fn token_mismatch_writes_nothing() {
        let dir = TempDir::new();
        let svc = service(&dir);
        let run_id = lock(&svc.sim).unwrap().run_id;
        let p = svc.save_prepare(run_id, 0).unwrap();
        lock(&svc.sim).unwrap().apply(Command::ManualScram);
        assert_eq!(svc.save_write(&p.token, None, "x", ui()).unwrap_err(), STATE_CHANGED);
        assert_eq!(svc.save_write("garbage", None, "x", ui()).unwrap_err(), STATE_CHANGED);
        assert!(svc.list().saves.is_empty());
        let p = svc.save_prepare(run_id, 0).unwrap();
        let meta = svc.save_write(&p.token, None, "x", ui()).unwrap();
        assert!(meta.loadable && meta.run_summary.scrammed);
    }

    #[test]
    fn invalid_staged_load_leaves_active_sim_untouched() {
        let dir = TempDir::new();
        let svc = service(&dir);
        let id = saved(&svc);
        let path = dir.0.join(format!("{id}.json"));
        let mut v: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        v["engine"]["sim"]["irm_range"] = 99.into();
        std::fs::write(&path, serde_json::to_vec(&v).unwrap()).unwrap();
        let before = serde_json::to_string(&lock(&svc.sim).unwrap().to_state_json()).unwrap();
        assert!(svc.load_prepare(&id).is_err());
        assert!(svc.load_prepare("../x").is_err());
        assert_eq!(serde_json::to_string(&lock(&svc.sim).unwrap().to_state_json()).unwrap(), before);
        assert!(lock(&svc.staged).unwrap().is_none());
    }

    #[test]
    fn staged_load_commit_abort_and_replacement() {
        let dir = TempDir::new();
        let svc = service(&dir);
        let id = saved(&svc);
        let active_run = lock(&svc.sim).unwrap().run_id;
        let first = svc.load_prepare(&id).unwrap();
        assert_eq!(first.ui, ui());
        assert_ne!(first.snapshot.run_id, active_run);
        assert_eq!(lock(&svc.sim).unwrap().run_id, active_run);
        // A newer prepare replaces the older one; its token no longer commits.
        let second = svc.load_prepare(&id).unwrap();
        assert!(svc.load_commit(&first.token).is_err());
        svc.load_abort(&first.token).unwrap();
        let snap = svc.load_commit(&second.token).unwrap();
        assert_eq!(snap.run_id, second.snapshot.run_id);
        assert_eq!(lock(&svc.sim).unwrap().run_id, second.snapshot.run_id);
        assert!(svc.load_commit(&second.token).is_err());
        let third = svc.load_prepare(&id).unwrap();
        svc.load_abort(&third.token).unwrap();
        assert!(svc.load_commit(&third.token).is_err());
    }

    #[test]
    fn scram_right_after_commit_reaches_events_since() {
        let dir = TempDir::new();
        let svc = service(&dir);
        let id = saved(&svc);
        let staged = svc.load_prepare(&id).unwrap();
        let snap = svc.load_commit(&staged.token).unwrap();
        lock(&svc.sim).unwrap().apply(Command::ManualScram);
        let pulled = svc.events_since(snap.run_id, snap.ann_head).unwrap();
        assert!(pulled.events.iter().any(|e| e.kind == AnnKind::Onset && e.seq > snap.ann_head), "{:?}", pulled.events);
        assert!(svc.events_since(snap.run_id - 1, 0).is_err());
    }

    fn running(scenario: ScenarioName) -> Sim {
        let mut s = Sim::new(scenario);
        s.apply(Command::SetPaused { paused: false });
        s
    }

    #[test]
    fn clock_does_not_charge_wall_time_to_a_restored_run() {
        let t0 = Instant::now();
        let mut clock = WorkerClock::new(t0);
        let mut old = running(ScenarioName::HotStandby);
        clock.tick(&mut old, t0 + Duration::from_millis(20));
        // A load delayed by lock contention: the next tick comes 3 s later.
        let mut restored = Sim::from_state_json(&old.to_state_json()).unwrap();
        restored.apply(Command::SetPaused { paused: false });
        let t_before = restored.snapshot().time_s;
        let acc_before = restored.backlog_s();
        clock.tick(&mut restored, t0 + Duration::from_secs(3));
        assert_eq!(restored.snapshot().time_s, t_before);
        assert_eq!(restored.backlog_s(), acc_before);
        // Normal advance resumes on the next iteration.
        clock.tick(&mut restored, t0 + Duration::from_millis(3200));
        assert!(restored.snapshot().time_s > t_before);
    }

    #[test]
    fn clock_resets_on_scenario_switch_while_paused_then_resumed() {
        let t0 = Instant::now();
        let mut clock = WorkerClock::new(t0);
        let mut s = Sim::new(ScenarioName::ColdShutdown);
        s.apply(Command::SetPaused { paused: true });
        clock.tick(&mut s, t0 + Duration::from_millis(20));
        s.apply(Command::LoadScenario { name: ScenarioName::HotStandby });
        s.apply(Command::SetPaused { paused: false });
        let snap = clock.tick(&mut s, t0 + Duration::from_secs(5)).unwrap();
        assert_eq!(snap.time_s, 0.0);
        assert_eq!(snap.effective_speed, 0.0);
    }

    #[test]
    fn clock_still_advances_and_carries_backlog_within_a_run() {
        let t0 = Instant::now();
        let mut clock = WorkerClock::new(t0);
        let mut s = running(ScenarioName::HotStandby);
        clock.tick(&mut s, t0);
        // 2 s late wake-up: at most 0.5 s per iteration, the rest carried.
        let snap = clock.tick(&mut s, t0 + Duration::from_secs(2)).unwrap();
        assert!(snap.clock_backlog_s > 1.0);
        for i in 1..=4 {
            clock.tick(&mut s, t0 + Duration::from_secs(2) + Duration::from_millis(i));
        }
        assert!((s.snapshot().time_s - 2.0 * s.speed).abs() < 0.1, "{}", s.snapshot().time_s);
    }
}
