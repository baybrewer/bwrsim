//! Run identity, annunciator events and save/restore of the engine state (plan C1).

use bwr_core::api::{AnnEvent, AnnKind, Command, ModeSwitch, RunChanged, ScenarioName};
use bwr_core::params::*;
use bwr_core::sim::{ANN_RING_LEN, ANN_TAIL_LEN, DT, ENGINE_STATE_VERSION};
use bwr_core::Sim;
use serde_json::Value;

fn run(sim: &mut Sim, seconds: f64) {
    for _ in 0..(seconds / DT).round() as usize {
        sim.step(DT);
    }
}

/// Every event after `seq` in the current run.
fn events_after(sim: &Sim, seq: u64) -> Vec<AnnEvent> {
    sim.ann_events_since(sim.run_id, seq).unwrap().events
}

fn count(events: &[AnnEvent], kind: AnnKind, key: &str) -> usize {
    events.iter().filter(|e| e.kind == kind && e.key == key).count()
}

/// Structural JSON equality with every float compared by bit pattern
/// (`Value`'s own equality treats 0.0 and -0.0 as equal).
fn bit_equal(a: &Value, b: &Value, path: &str) -> Result<(), String> {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => match (x.is_f64(), y.is_f64()) {
            (true, true) if x.as_f64().unwrap().to_bits() == y.as_f64().unwrap().to_bits() => Ok(()),
            (false, false) if x == y => Ok(()),
            _ => Err(format!("{path}: {x} != {y}")),
        },
        (Value::Array(x), Value::Array(y)) => {
            if x.len() != y.len() {
                return Err(format!("{path}: length {} != {}", x.len(), y.len()));
            }
            x.iter().zip(y).enumerate().try_for_each(|(i, (p, q))| bit_equal(p, q, &format!("{path}[{i}]")))
        }
        (Value::Object(x), Value::Object(y)) => {
            if x.len() != y.len() || x.keys().any(|k| !y.contains_key(k)) {
                return Err(format!("{path}: key sets differ"));
            }
            x.iter().try_for_each(|(k, v)| bit_equal(v, &y[k], &format!("{path}.{k}")))
        }
        _ if a == b => Ok(()),
        _ => Err(format!("{path}: {a} != {b}")),
    }
}

/// Engine state with the documented restore overrides (pause, run identity) masked.
fn comparable(sim: &Sim) -> Value {
    let mut v = sim.to_state_json();
    v["sim"]["paused"] = Value::Null;
    v["sim"]["run_id"] = Value::Null;
    v
}

fn assert_same_state(a: &Sim, b: &Sim, when: &str) {
    bit_equal(&comparable(a), &comparable(b), "state").unwrap_or_else(|e| panic!("{when}: {e}"));
}

/// Real text round trip, as a save file takes.
fn restore_via_text(sim: &Sim) -> Sim {
    let text = serde_json::to_string(&sim.to_state_json()).unwrap();
    Sim::from_state_json(&serde_json::from_str(&text).unwrap()).unwrap()
}

/// At power after a turbine trip with rods still moving, alarms acknowledged,
/// operator events active and a fractional clock accumulator.
fn busy_plant() -> Sim {
    let mut sim = Sim::new(ScenarioName::FullPower);
    sim.apply(Command::SetSpeed { speed: 3.0 });
    sim.advance(2.0);
    let movable: Vec<usize> = (0..N_RODS).filter(|&i| sim.core.rods.pos[i] > 10.0).take(4).collect();
    sim.apply(Command::RodSetTarget { indices: movable, target: 2.0 });
    sim.apply(Command::TripTurbine);
    sim.advance(0.517);
    sim.apply(Command::AckAlarms);
    sim.apply(Command::SetSpeed { speed: f64::NAN }); // rejected: raises an operator event
    sim.advance(0.25);
    let snap = sim.snapshot();
    assert!(sim.backlog_s() > 0.0 && sim.backlog_s() < DT, "accumulator {}", sim.backlog_s());
    assert!(sim.core.rods.pos.iter().zip(&sim.core.rods.target).any(|(p, t)| (p - t).abs() > 1e-6), "no pending rod motion");
    assert!(sim.turbine.tripped && sim.turbine.gov_integral != 0.0, "turbine trip / governor integral");
    assert!(snap.alarms.iter().any(|a| a.acked) && snap.alarms.iter().any(|a| !a.acked), "acks");
    assert!(snap.alarms.iter().any(|a| a.id == "cmd_rejected"), "operator event");
    assert!(snap.ann_head > 0);
    sim
}

#[test]
fn restore_is_exact_and_continues_bit_identically() {
    let mut a = busy_plant();
    let mut b = restore_via_text(&a);
    assert!(b.paused && b.run_id != a.run_id && b.scenario == a.scenario);
    assert_same_state(&a, &b, "after restore");
    assert_eq!(b.backlog_s().to_bits(), a.backlog_s().to_bits());
    // The restored run continues the same annunciator sequence.
    assert_eq!(b.snapshot().ann_tail, a.snapshot().ann_tail);

    a.apply(Command::SetPaused { paused: false });
    b.apply(Command::SetPaused { paused: false });
    for (i, dt) in [0.1, 0.033, 1.7, 0.02, 5.0, 0.0071, 12.0].into_iter().enumerate() {
        a.advance(dt);
        b.advance(dt);
        assert_same_state(&a, &b, &format!("continuation {i}"));
    }
    assert_eq!(a.time.to_bits(), b.time.to_bits());
}

#[test]
fn restore_of_a_halted_sim_keeps_the_fault_and_retries_identically() {
    let mut a = Sim::new(ScenarioName::FullPower);
    run(&mut a, 1.0);
    a.vessel.energy *= 3.0;
    assert!(!a.step(DT) && a.model_error.is_some());
    let mut b = restore_via_text(&a);
    assert_same_state(&a, &b, "halted restore");
    assert!(b.model_error.is_some());
    assert_eq!(a.step(DT), b.step(DT));
    assert_same_state(&a, &b, "after retry");
}

#[test]
fn saved_acknowledgements_are_sorted() {
    let sim = busy_plant();
    let acked: Vec<String> = serde_json::from_value(sim.to_state_json()["sim"]["acked"].clone()).unwrap();
    assert!(acked.len() > 1, "{acked:?}");
    assert!(acked.windows(2).all(|w| w[0] < w[1]), "{acked:?}");
}

#[test]
fn restore_rejects_invalid_state() {
    let good = Sim::new(ScenarioName::HotStandby).to_state_json();
    let reject = |what: &str, edit: &dyn Fn(&mut Value)| {
        let mut v = good.clone();
        edit(&mut v);
        let err = Sim::from_state_json(&v).err().unwrap_or_else(|| panic!("{what} accepted"));
        eprintln!("{what}: {err}");
    };
    reject("version mismatch", &|v| v["version"] = Value::from(ENGINE_STATE_VERSION + 1));
    reject("missing version", &|v| v["version"] = Value::Null);
    for field in ["pos", "target", "weights", "fast_insert", "layout"] {
        reject(field, &|v| {
            v["sim"]["core"]["rods"][field].as_array_mut().unwrap().pop();
        });
    }
    reject("IC count", &|v| {
        v["sim"]["ic"].as_array_mut().unwrap().pop();
    });
    reject("null float", &|v| v["sim"]["time"] = Value::Null);
    reject("irm range", &|v| v["sim"]["irm_range"] = Value::from(11));
    reject("rod beyond travel", &|v| v["sim"]["core"]["rods"]["pos"][0] = Value::from(ROD_FULL_OUT + 1.0));
    reject("unknown enum", &|v| v["sim"]["mode"] = Value::from("bogus"));
    assert!(Sim::from_state_json(&good).is_ok());

    // An in-memory non-finite value is caught too.
    let mut sim = Sim::new(ScenarioName::HotStandby);
    sim.vessel.mass = f64::NAN;
    assert!(sim.validate().is_err());
    let mut sim = Sim::new(ScenarioName::HotStandby);
    sim.core.kin.c[3] = f64::INFINITY;
    assert!(sim.validate().is_err());
}

// ------------------------------------------------------------------ run identity

#[test]
fn run_id_changes_on_new_run_and_restore_only() {
    let mut sim = Sim::new(ScenarioName::ColdShutdown);
    let first = sim.run_id;
    assert_ne!(Sim::new(ScenarioName::ColdShutdown).run_id, first);
    sim.apply(Command::SetSpeed { speed: 2.0 });
    run(&mut sim, 1.0);
    assert_eq!(sim.run_id, first);
    let restored = restore_via_text(&sim);
    assert_ne!(restored.run_id, first);
    sim.apply(Command::LoadScenario { name: ScenarioName::ColdShutdown });
    assert!(sim.run_id != first && sim.run_id != restored.run_id);
    assert_eq!(sim.scenario, ScenarioName::ColdShutdown);
    sim.apply(Command::LoadScenario { name: ScenarioName::HotStandby });
    assert_eq!(sim.snapshot().scenario, ScenarioName::HotStandby);
    assert_eq!(sim.snapshot().run_id, sim.run_id);
}

#[test]
fn rev_counts_commands_and_step_outcomes() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    assert_eq!(sim.rev, 0);
    sim.apply(Command::RodStop);
    sim.apply(Command::SetSpeed { speed: f64::NAN }); // rejected commands count too
    assert!(sim.step(DT));
    sim.step(0.0); // not a step
    assert_eq!(sim.rev, 3);
    sim.vessel.energy *= 3.0;
    assert!(!sim.step(DT));
    assert_eq!(sim.snapshot().rev, 4);
}

#[test]
fn events_since_a_replaced_run_report_run_changed() {
    let mut sim = Sim::new(ScenarioName::ColdShutdown);
    let old = sim.run_id;
    assert!(sim.ann_events_since(old, 0).is_ok());
    sim.apply(Command::LoadScenario { name: ScenarioName::HotStandby });
    assert_eq!(sim.ann_events_since(old, 0), Err(RunChanged { current_run_id: sim.run_id }));
}

// ------------------------------------------------------------------ annunciator events

#[test]
fn construction_and_scenario_load_publish_only_time_zero_onsets() {
    for sim in [Sim::new(ScenarioName::FullPower), Sim::new(ScenarioName::ColdShutdown)] {
        let s = sim.snapshot();
        assert_eq!(s.time_s, 0.0);
        assert_eq!(s.ann_head as usize, s.ann_tail.len());
        assert!(s.ann_tail.iter().enumerate().all(|(i, e)| e.seq == i as u64 + 1 && e.time_s == 0.0 && e.kind == AnnKind::Onset));
    }
    // A run that has logged events, then a scenario load: the sequence restarts.
    let mut sim = Sim::new(ScenarioName::ColdShutdown);
    sim.apply(Command::SetSpeed { speed: f64::NAN });
    run(&mut sim, 2.0);
    assert!(sim.snapshot().ann_head >= 2);
    sim.apply(Command::LoadScenario { name: ScenarioName::FullPower });
    let s = sim.snapshot();
    assert_eq!((s.time_s, s.rev), (0.0, 1));
    assert_eq!(s.ann_head as usize, s.ann_tail.len());
    assert!(s.ann_tail.iter().all(|e| e.time_s == 0.0 && e.kind == AnnKind::Onset), "{:?}", s.ann_tail);
    let since = sim.ann_events_since(sim.run_id, 0).unwrap();
    assert!(!since.truncated && since.events == s.ann_tail);
}

#[test]
fn alarm_that_appears_and_clears_between_publishes_is_logged() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    let head = sim.snapshot().ann_head;
    sim.apply(Command::SetHeadVent { open: true });
    sim.apply(Command::SetHeadVent { open: false });
    let s = sim.snapshot();
    assert!(!s.alarms.iter().any(|a| a.id == "vent"));
    let ev = events_after(&sim, head);
    assert_eq!(ev.iter().map(|e| (e.kind, e.key.as_str())).collect::<Vec<_>>(), [(AnnKind::Onset, "cond:vent"), (AnnKind::Clear, "cond:vent")]);
    assert_eq!(ev[1].text, ev[0].text);
}

#[test]
fn changed_scram_reset_block_reason_is_a_changed_event() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    sim.apply(Command::SetModeSwitch { mode: ModeSwitch::Shutdown });
    let head = sim.snapshot().ann_head;
    sim.apply(Command::ResetScram);
    run(&mut sim, 11.0);
    sim.apply(Command::ResetScram);
    let ev: Vec<AnnEvent> = events_after(&sim, head).into_iter().filter(|e| e.key == "evt:scram_reset").collect();
    assert_eq!(ev.len(), 2, "{ev:?}");
    assert_eq!((ev[0].kind, ev[1].kind), (AnnKind::Onset, AnnKind::Changed));
    assert!(ev[0].text.contains("wait 10 s") && ev[1].text.contains("SHUTDOWN"), "{ev:?}");
}

#[test]
fn condition_and_event_with_the_same_legacy_id_stay_distinct() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    let head = sim.snapshot().ann_head;
    // APRM upscale raises the rod-block condition; a withdrawal then raises the rod-block event.
    sim.core.kin.n = 1.1;
    let rod = (0..N_RODS).find(|&i| sim.core.rods.pos[i] < ROD_FULL_OUT - 1.0).expect("an inserted rod");
    sim.apply(Command::RodSetTarget { indices: vec![rod], target: ROD_FULL_OUT });
    let ev = events_after(&sim, head);
    assert_eq!(count(&ev, AnnKind::Onset, "cond:rod_block"), 1, "{ev:?}");
    assert_eq!(count(&ev, AnnKind::Onset, "evt:rod_block"), 1, "{ev:?}");
    let ids: Vec<&str> = sim.snapshot().alarms.iter().map(|a| a.id.as_str()).filter(|&id| id == "rod_block").collect();
    assert_eq!(ids.len(), 2);
}

#[test]
fn numeric_alarm_text_changes_are_not_logged() {
    let mut sim = Sim::new(ScenarioName::HotStandby);
    sim.core.kin.n = 1e-3;
    sim.core.kin.inv_period = 1.0 / 15.0;
    sim.apply(Command::RodStop);
    let head = sim.snapshot().ann_head;
    assert!(sim.snapshot().alarms.iter().any(|a| a.id == "period" && a.text.contains("15")));
    sim.core.kin.inv_period = 1.0 / 12.0;
    sim.apply(Command::RodStop);
    assert!(sim.snapshot().alarms.iter().any(|a| a.id == "period" && a.text.contains("12")));
    assert!(events_after(&sim, head).iter().all(|e| e.key != "cond:period"));
}

#[test]
fn ring_overflow_reports_truncation() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    for _ in 0..ANN_RING_LEN {
        sim.apply(Command::SetHeadVent { open: true });
        sim.apply(Command::SetHeadVent { open: false });
    }
    let head = sim.snapshot().ann_head;
    assert!(head as usize >= 2 * ANN_RING_LEN);
    let all = sim.ann_events_since(sim.run_id, 0).unwrap();
    assert!(all.truncated);
    assert_eq!(all.events.len(), ANN_RING_LEN);
    assert_eq!(all.events.last().unwrap().seq, head);
    let recent = sim.ann_events_since(sim.run_id, head - 10).unwrap();
    assert!(!recent.truncated && recent.events.len() == 10);
    let oldest_held = head - ANN_RING_LEN as u64 + 1;
    assert!(!sim.ann_events_since(sim.run_id, oldest_held - 1).unwrap().truncated);
    assert!(sim.ann_events_since(sim.run_id, oldest_held - 2).unwrap().truncated);
    let tail = sim.snapshot().ann_tail;
    assert_eq!(tail.len(), ANN_TAIL_LEN);
    assert_eq!(tail.last().unwrap().seq, head);
    assert!(sim.ann_events_since(sim.run_id, head).unwrap().events.is_empty());
}

/// The model-limit condition and event appear exactly once however often the
/// halted step is retried (rolled-back intermediate states never surface).
fn assert_rejection_onsets_once(mut sim: Sim, label: &str) {
    let head = sim.snapshot().ann_head;
    for _ in 0..3 {
        assert!(!sim.step(DT), "{label}: step must be rejected");
    }
    sim.advance(1.0); // halted: does nothing
    let ev = events_after(&sim, head);
    for key in ["cond:model", "evt:model_limit"] {
        assert_eq!(count(&ev, AnnKind::Onset, key), 1, "{label}: {key} in {ev:?}");
        assert_eq!(ev.iter().filter(|e| e.key == key).count(), 1, "{label}: {key} in {ev:?}");
    }
}

#[test]
fn vessel_and_condenser_rejections_raise_their_onsets_once() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 1.0);
    sim.vessel.energy *= 3.0;
    assert_rejection_onsets_once(sim, "vessel");

    // Same injected condenser dryout as `condenser_rejection_rolls_back_the_plant_step`.
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 1.0);
    sim.cond.cw_pumps = [false; N_CW_PUMPS];
    sim.cond.hotwell_mass = 0.0;
    sim.cond.set_temperature(bwr_core::steam::sat_p(CONDENSER_RELIEF_P).t);
    sim.cond.energy_j += 1.0e9;
    for t in sim.ic.iter_mut() {
        t.cmd_open = true;
    }
    assert!(!sim.clone().step(DT));
    assert_rejection_onsets_once(sim, "condenser");
}

#[test]
fn summary_matches_the_snapshot() {
    let sim = Sim::new(ScenarioName::FullPower);
    let (sum, snap) = (sim.summary(), sim.snapshot());
    assert_eq!(sum.mode, snap.mode_switch);
    assert_eq!(sum.power_pct, snap.neutronics.aprm_pct);
    assert_eq!(sum.pressure_mpa, snap.vessel.pressure_mpa);
    assert_eq!(sum.scrammed, snap.scrammed);
}
