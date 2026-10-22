//! Scripted validation runner: fixed scenarios, fixed durations, dt = 0.05 s.
//! Usage: cargo run --release -p bwr-core --example validation
//!
//! Each case writes a CSV trace to docs/validation/traces/<case>.csv and prints
//! its extrema. The run exits non-zero if any case produces a non-finite value,
//! a physics model error, a kinetics fault, or a vessel mass/energy residual
//! above the tolerances below. The residual compares the vessel's integrated
//! state with the port quantities the connected systems report (BOP steam and
//! feed, RWCU, head vent, IC trains), so a wiring inconsistency in sim.rs shows
//! up here. These are conservation and robustness checks of the implemented
//! model, not an independent physical validation against plant data; the
//! reported "peak fuel" is the lumped average fuel node (no cladding model).

use std::path::PathBuf;

use bwr_core::api::{Command, ModeSwitch, ScenarioName};
use bwr_core::params::*;
use bwr_core::sim::DT;
use bwr_core::Sim;

/// Energy residual bound as a fraction of the integrated core heat.
const ENERGY_TOL_REL: f64 = 1.0e-6;
/// Mass residual bound, kg (vessel inventory is of order 2e5 kg).
const MASS_TOL_KG: f64 = 1.0e-3;
const SAMPLE_EVERY_S: f64 = 1.0;
const CSV_HEADER: &str =
    "t_s,power_frac,q_core_mw,pressure_mpa,level_nr_cm,coolant_t_c,fuel_t_c,steam_kgs,feed_kgs,ic_mw,net_mwe,cond_kpa,mass_res_kg,energy_res_j";

struct Case {
    name: &'static str,
    scenario: ScenarioName,
    duration_s: f64,
    events: Vec<(f64, Command)>,
}

fn cases() -> Vec<Case> {
    let msiv = |line| Command::SetMsiv { line, open: false };
    vec![
        Case { name: "full_power_steady", scenario: ScenarioName::FullPower, duration_s: 300.0, events: vec![] },
        Case { name: "turbine_trip", scenario: ScenarioName::FullPower, duration_s: 180.0, events: vec![(5.0, Command::TripTurbine)] },
        Case {
            name: "manual_scram",
            scenario: ScenarioName::FullPower,
            duration_s: 180.0,
            events: vec![(5.0, Command::ManualScram), (5.0, Command::SetModeSwitch { mode: ModeSwitch::Shutdown })],
        },
        Case { name: "msiv_closure_ic_cooling", scenario: ScenarioName::FullPower, duration_s: 600.0, events: vec![(5.0, msiv(0)), (5.0, msiv(1))] },
        Case {
            name: "hot_standby_startup",
            scenario: ScenarioName::HotStandby,
            duration_s: 300.0,
            events: vec![(5.0, Command::RodSetTarget { indices: (0..N_RODS).collect(), target: 6.0 })],
        },
    ]
}

/// Steps once and returns the (mass, energy) the connected systems report having
/// exchanged with the vessel, using the same ambient loss law as `Vessel::step_with_ic`.
/// Mirrors `port_balance_step` in tests/plant.rs.
fn port_balance_step(sim: &mut Sim) -> (f64, f64) {
    let t0 = sim.vessel.eos.t;
    let sat0 = sim.vessel.sat();
    let heat_loss = HEAT_LOSS_HOT * ((t0 - 20.0) / 268.0).max(0.0);
    sim.step(DT);
    let ic_in: f64 = sim.ic.iter().map(|t| t.last.w_steam_in).sum();
    let ic_ret_m: f64 = sim.ic.iter().map(|t| t.last.w_cond_return).sum();
    let ic_ret_e: f64 = sim.ic.iter().map(|t| t.last.w_cond_return * t.last.h_return).sum();
    let f = sim.vessel.flows;
    let (w_steam, w_feed, w_rwcu) = (sim.steam.total_flow(), sim.fw.flow, sim.fw.rwcu_reject);
    let dm = w_feed + ic_ret_m - w_steam - w_rwcu - f.w_vent - ic_in;
    let de = sim.q_core - heat_loss + w_feed * sim.fw.h_feed() + ic_ret_e
        - w_steam * sim.steam.h_steam
        - f.w_vent * f.h_vent
        - ic_in * sat0.h_g
        - w_rwcu * sat0.h_f;
    (dm * DT, de * DT)
}

struct Outcome {
    peak_p_mpa: f64,
    min_level_cm: f64,
    peak_fuel_c: f64,
    scram_s: Option<f64>,
    mass_res_kg: f64,
    energy_res_rel: f64,
    failures: Vec<String>,
}

fn run_case(case: &Case, out_dir: &std::path::Path) -> Outcome {
    let mut sim = Sim::new(case.scenario);
    let (m0, e0) = (sim.vessel.mass, sim.vessel.energy);
    let (mut m_ports, mut e_ports, mut q_core_j) = (0.0, 0.0, 0.0);
    let mut o = Outcome {
        peak_p_mpa: 0.0,
        min_level_cm: f64::MAX,
        peak_fuel_c: 0.0,
        scram_s: None,
        mass_res_kg: 0.0,
        energy_res_rel: 0.0,
        failures: Vec::new(),
    };
    let mut events = case.events.clone();
    let mut csv = String::from(CSV_HEADER);
    let mut next_sample = 0.0;
    let steps = (case.duration_s / DT).round() as usize;
    for i in 0..=steps {
        while events.first().is_some_and(|(t, _)| *t <= sim.time + 1e-9) {
            sim.apply(events.remove(0).1);
        }
        let s = sim.snapshot();
        let m_res = (sim.vessel.mass - m0) - m_ports;
        let e_res = (sim.vessel.energy - e0) - e_ports;
        let ic_mw: f64 = s.ic.iter().map(|t| t.heat_mw).sum();
        let row = [
            sim.time, s.neutronics.power_frac, sim.q_core / 1e6, s.vessel.pressure_mpa, s.vessel.level_nr_cm,
            s.vessel.coolant_temp_c, s.neutronics.fuel_temp_c, s.vessel.steam_flow_kgs, s.vessel.feed_flow_kgs,
            ic_mw, s.turbine.net_mwe, s.condenser.pressure_kpa, m_res, e_res,
        ];
        if let Some(bad) = row.iter().position(|x| !x.is_finite()) {
            o.failures.push(format!("non-finite value in column {} at t={:.2} s", CSV_HEADER.split(',').nth(bad).unwrap(), sim.time));
            break;
        }
        o.peak_p_mpa = o.peak_p_mpa.max(s.vessel.pressure_mpa);
        o.min_level_cm = o.min_level_cm.min(s.vessel.level_nr_cm);
        o.peak_fuel_c = o.peak_fuel_c.max(s.neutronics.fuel_temp_c);
        if s.scrammed && o.scram_s.is_none() {
            o.scram_s = Some(sim.time);
        }
        if let Some(e) = &s.model_error {
            o.failures.push(format!("model error at t={:.2} s: {e}", sim.time));
            break;
        }
        if let Some(e) = &s.neutronics.kinetics_fault {
            o.failures.push(format!("kinetics fault at t={:.2} s: {e}", sim.time));
            break;
        }
        if sim.time + 1e-9 >= next_sample || i == steps {
            let cells: Vec<String> = row.iter().map(|x| format!("{x:.6e}")).collect();
            csv.push('\n');
            csv.push_str(&cells.join(","));
            next_sample += SAMPLE_EVERY_S;
        }
        if i == steps {
            break;
        }
        let (dm, de) = port_balance_step(&mut sim);
        m_ports += dm;
        e_ports += de;
        q_core_j += sim.q_core * DT;
    }
    o.mass_res_kg = (sim.vessel.mass - m0) - m_ports;
    o.energy_res_rel = ((sim.vessel.energy - e0) - e_ports).abs() / q_core_j.max(1.0);
    if o.mass_res_kg.abs() > MASS_TOL_KG {
        o.failures.push(format!("mass residual {:.3e} kg exceeds {MASS_TOL_KG:.0e} kg", o.mass_res_kg));
    }
    if o.energy_res_rel > ENERGY_TOL_REL {
        o.failures.push(format!("energy residual {:.3e} of core heat exceeds {ENERGY_TOL_REL:.0e}", o.energy_res_rel));
    }
    let path = out_dir.join(format!("{}.csv", case.name));
    if let Err(e) = std::fs::write(&path, csv) {
        o.failures.push(format!("cannot write {}: {e}", path.display()));
    }
    o
}

fn main() {
    let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/validation/traces");
    std::fs::create_dir_all(&out_dir).expect("create trace directory");
    println!("dt = {DT} s; energy tolerance {ENERGY_TOL_REL:.0e} of core heat, mass tolerance {MASS_TOL_KG:.0e} kg");
    println!(
        "{:<24} {:>8} {:>10} {:>10} {:>9} {:>12} {:>12}  result",
        "case", "peakPmpa", "minLvl_cm", "peakFuel_C", "scram_s", "massRes_kg", "energyRes"
    );
    let mut failed = false;
    for case in cases() {
        let o = run_case(&case, &out_dir);
        let scram = o.scram_s.map_or("-".to_string(), |t| format!("{t:.2}"));
        let result = if o.failures.is_empty() { "PASS".to_string() } else { format!("FAIL: {}", o.failures.join("; ")) };
        println!(
            "{:<24} {:>8.3} {:>10.1} {:>10.0} {:>9} {:>12.3e} {:>12.3e}  {result}",
            case.name, o.peak_p_mpa, o.min_level_cm, o.peak_fuel_c, scram, o.mass_res_kg, o.energy_res_rel
        );
        failed |= !o.failures.is_empty();
    }
    println!("traces: {}", out_dir.canonicalize().unwrap_or(out_dir).display());
    if failed {
        std::process::exit(1);
    }
}
