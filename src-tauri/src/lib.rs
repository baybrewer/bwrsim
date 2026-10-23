//! Desktop shell: runs the simulator on a background thread and streams
//! snapshots to the UI as `sim-state` events (10 per second). Save slots and
//! the worker clock live in `saves`/`host`; these handlers only adapt them.

mod host;
mod saves;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bwr_core::api::{AnnEventsSince, ScenarioName};
use bwr_core::{Command, Sim, Snapshot};
use host::{lock, LoadPrepared, SavePrepared, SaveService, WorkerClock};
use saves::{SaveList, SaveMeta, SaveStore};
use serde_json::Value;
use tauri::{Emitter, Manager, State};

struct SimState(Arc<Mutex<Sim>>);

#[tauri::command]
fn sim_command(state: State<'_, SimState>, cmd: Command) -> Result<(), String> {
    lock(&state.0)?.apply(cmd);
    Ok(())
}

#[tauri::command]
fn sim_snapshot(state: State<'_, SimState>) -> Result<Snapshot, String> {
    Ok(lock(&state.0)?.snapshot())
}

#[tauri::command]
fn sim_events_since(svc: State<'_, SaveService>, run_id: u64, seq: u64) -> Result<AnnEventsSince, String> {
    svc.events_since(run_id, seq)
}

#[tauri::command]
fn save_list(svc: State<'_, SaveService>) -> SaveList {
    svc.list()
}

#[tauri::command]
fn save_prepare(svc: State<'_, SaveService>, run_id: u64, since_seq: u64) -> Result<SavePrepared, String> {
    svc.save_prepare(run_id, since_seq)
}

#[tauri::command]
fn save_write(svc: State<'_, SaveService>, token: String, id: Option<String>, name: String, ui: Value) -> Result<SaveMeta, String> {
    svc.save_write(&token, id.as_deref(), &name, ui)
}

#[tauri::command]
fn load_prepare(svc: State<'_, SaveService>, id: String) -> Result<LoadPrepared, String> {
    svc.load_prepare(&id)
}

#[tauri::command]
fn load_commit(svc: State<'_, SaveService>, token: String) -> Result<Snapshot, String> {
    svc.load_commit(&token)
}

#[tauri::command]
fn load_abort(svc: State<'_, SaveService>, token: String) -> Result<(), String> {
    svc.load_abort(&token)
}

#[tauri::command]
fn save_delete(svc: State<'_, SaveService>, id: String) -> Result<(), String> {
    svc.delete(&id)
}

pub fn run() {
    let sim = Arc::new(Mutex::new(Sim::new(ScenarioName::ColdShutdown)));
    let worker = sim.clone();
    let saves_sim = sim.clone();
    tauri::Builder::default()
        .manage(SimState(sim))
        .setup(move |app| {
            let dir = app.path().app_data_dir()?.join("saves");
            app.manage(SaveService::new(saves_sim, SaveStore::new(dir)));
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                let mut clock = WorkerClock::new(Instant::now());
                loop {
                    std::thread::sleep(Duration::from_millis(20));
                    let snap = {
                        let mut s = match worker.lock() {
                            Ok(s) => s,
                            Err(p) => p.into_inner(),
                        };
                        clock.tick(&mut s, Instant::now())
                    };
                    if let Some(snap) = snap {
                        let _ = handle.emit("sim-state", snap);
                    }
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            sim_command,
            sim_snapshot,
            sim_events_since,
            save_list,
            save_prepare,
            save_write,
            load_prepare,
            load_commit,
            load_abort,
            save_delete
        ])
        .run(tauri::generate_context!())
        .expect("error while running the BWRX-300 simulator");
}
