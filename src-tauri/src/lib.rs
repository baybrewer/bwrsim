//! Desktop shell: runs the simulator on a background thread and streams
//! snapshots to the UI as `sim-state` events (10 per second).

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bwr_core::api::ScenarioName;
use bwr_core::{Command, Sim, Snapshot};
use tauri::{Emitter, State};

struct SimState(Arc<Mutex<Sim>>);

/// Most wall-clock time handed to `Sim::advance` per worker iteration, s. At the
/// maximum speed (100x) this is 50 simulated seconds, well below the step cap
/// inside `Sim::advance`; any excess waits in the backlog for the next iteration.
const MAX_ADVANCE_PER_ITER_S: f64 = 0.5;

/// Smoothing time constant of the achieved-speed readout, wall-clock s.
const EFFECTIVE_SPEED_TAU_S: f64 = 1.0;

#[tauri::command]
fn sim_command(state: State<'_, SimState>, cmd: Command) -> Result<(), String> {
    state.0.lock().map_err(|e| e.to_string())?.apply(cmd);
    Ok(())
}

#[tauri::command]
fn sim_snapshot(state: State<'_, SimState>) -> Result<Snapshot, String> {
    Ok(state.0.lock().map_err(|e| e.to_string())?.snapshot())
}

pub fn run() {
    let sim = Arc::new(Mutex::new(Sim::new(ScenarioName::ColdShutdown)));
    let worker = sim.clone();
    tauri::Builder::default()
        .manage(SimState(sim))
        .setup(move |app| {
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                let mut last = Instant::now();
                let mut last_emit = Instant::now();
                // Wall-clock time not yet handed to the simulator. Work per iteration is
                // bounded, but the remainder is carried instead of discarded so delayed
                // wake-ups do not change the simulated duration.
                let mut backlog = 0.0f64;
                // Achieved-speed readout: simulated time gained per wall second between emits.
                let mut effective_speed = 0.0f64;
                let mut last_emit_time_s: Option<f64> = None;
                loop {
                    std::thread::sleep(Duration::from_millis(20));
                    let now = Instant::now();
                    backlog += (now - last).as_secs_f64();
                    last = now;
                    let dt = backlog.min(MAX_ADVANCE_PER_ITER_S);
                    let snap = {
                        let mut s = match worker.lock() {
                            Ok(s) => s,
                            Err(p) => p.into_inner(),
                        };
                        if s.paused {
                            // Time spent paused is not owed to the simulation.
                            backlog = 0.0;
                        } else {
                            s.advance(dt);
                            backlog -= dt;
                        }
                        let since_emit = (now - last_emit).as_secs_f64();
                        if since_emit >= 0.1 {
                            let mut snap = s.snapshot();
                            // The first sample after start or resume seeds the filter.
                            let alpha = if effective_speed == 0.0 { 1.0 } else { (since_emit / EFFECTIVE_SPEED_TAU_S).min(1.0) };
                            match last_emit_time_s {
                                _ if s.paused => effective_speed = 0.0,
                                // A scenario load resets sim time; that window has no valid sample.
                                Some(t0) if snap.time_s >= t0 => {
                                    let rate = (snap.time_s - t0) / since_emit;
                                    effective_speed += (rate - effective_speed) * alpha;
                                }
                                _ => {}
                            }
                            last_emit_time_s = Some(snap.time_s);
                            snap.effective_speed = effective_speed;
                            snap.clock_backlog_s = s.backlog_s() + backlog * s.speed;
                            Some(snap)
                        } else {
                            None
                        }
                    };
                    if let Some(snap) = snap {
                        last_emit = now;
                        let _ = handle.emit("sim-state", snap);
                    }
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![sim_command, sim_snapshot])
        .run(tauri::generate_context!())
        .expect("error while running the BWRX-300 simulator");
}
