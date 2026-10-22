//! Scripted validation runner: fixed scenarios, fixed durations, dt = 0.05 s.
//! Usage: cargo run --release -p bwr-core --example validation
//!
//! Each case writes a CSV trace to docs/validation/traces/<case>.csv and prints
//! its extrema. The run exits non-zero if any case produces a non-finite value,
//! a physics model error, a kinetics fault, or a residual above the tolerances
//! below. Three gates:
//!
//! 1. Vessel ports: the vessel's integrated state against the port quantities
//!    the connected systems report (BOP steam and feed, RWCU, head vent, IC
//!    trains), so a wiring inconsistency in sim.rs shows up here.
//! 2. Whole plant: every water inventory (vessel, IC tubes and pools, condenser
//!    hotwell and shell vapour; the steam lines and feed train hold no mass
//!    state) and the fluid-side stored energy (vessel, IC tubes, condenser node,
//!    heater node) against what crossed the plant boundary: core heat, vessel
//!    ambient loss, IC pool duty and boil-off, CW rejection, turbine shaft
//!    power, feed pump work, heater ambient loss, CST makeup/reject/overflow,
//!    shell relief, head vent and the condenser's domain residual. RWCU reject
//!    returns to the condenser, so it is internal. Shaft power is the boundary
//!    with the turbine-generator; the electrical output is reported beside it.
//! 3. Timestep refinement: one case is rerun at dt = 0.025 s and its peak
//!    pressure and minimum level must agree with the dt = 0.05 s run.
//!
//! These are conservation, convergence and robustness checks of the implemented
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
/// Whole-plant bounds, the same as the vessel gate: every term is a number the
/// subsystems integrate themselves, so only roundoff is legitimate.
/// Mass, kg (about 3e5 kg of water inside the boundary).
const PLANT_MASS_TOL_KG: f64 = 1.0e-3;
/// Energy, as a fraction of the integrated core heat.
const PLANT_ENERGY_TOL_REL: f64 = 1.0e-6;
/// Timestep refinement: case, refined dt and agreement bounds with the dt = DT run.
const REFINE_CASE: &str = "turbine_trip";
const REFINE_DT: f64 = 0.025;
const REFINE_PEAK_P_TOL_MPA: f64 = 0.02;
const REFINE_MIN_LEVEL_TOL_CM: f64 = 3.0;
const SAMPLE_EVERY_S: f64 = 1.0;
const CSV_HEADER: &str =
    "t_s,power_frac,q_core_mw,pressure_mpa,level_nr_cm,coolant_t_c,fuel_t_c,steam_kgs,feed_kgs,ic_mw,net_mwe,cond_kpa,mass_res_kg,energy_res_j,plant_mass_res_kg,plant_energy_res_j";

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
fn port_balance_step(sim: &mut Sim, dt: f64) -> (f64, f64) {
    let t0 = sim.vessel.eos.t;
    let sat0 = sim.vessel.sat();
    let heat_loss = vessel_heat_loss(t0);
    sim.step(dt);
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
    (dm * dt, de * dt)
}

/// Ambient loss law of `Vessel::step_with_ic` at the start-of-step temperature, W.
fn vessel_heat_loss(t: f64) -> f64 {
    HEAT_LOSS_HOT * ((t - 20.0) / 268.0).max(0.0)
}

/// Water inside the plant boundary (kg) and fluid-side stored energy (J). The
/// IC pools hold boundary mass but their energy is outside: pool duty is the
/// energy boundary there.
fn plant_inventory(sim: &Sim) -> (f64, f64) {
    let ic_mass: f64 = sim.ic.iter().map(|t| t.tube_mass + t.pool_mass).sum();
    let ic_energy: f64 = sim.ic.iter().map(|t| t.tube_mass * t.tube_h).sum();
    let mass = sim.vessel.mass + sim.cond.inventory_kg() + ic_mass;
    let energy = sim.vessel.energy + sim.cond.energy_j + FW_HEATER_HEAT_CAP * sim.fw.t_feed + ic_energy;
    (mass, energy)
}

/// Cumulative boundary exchanges the subsystems count themselves (kg, J):
/// CST makeup/reject/overflow, shell relief, condenser domain residual, IC boil-off.
fn plant_counters(sim: &Sim) -> (f64, f64) {
    let c = &sim.cond;
    let boiloff: f64 = sim.ic.iter().map(|t| t.boiloff_kg).sum();
    let mass = c.makeup_kg - c.reject_kg - c.overflow_kg - c.relief_kg - boiloff;
    let energy = c.cst_energy_j - c.relief_energy_j - c.energy_residual_j;
    (mass, energy)
}

/// Integrated whole-plant boundary terms that are rates, plus totals for the report.
#[derive(Default)]
struct PlantFlows {
    mass_kg: f64,
    energy_j: f64,
    core_j: f64,
    cw_j: f64,
    shaft_j: f64,
    elec_j: f64,
}

impl PlantFlows {
    /// Accumulate the rate terms of the step just taken (`t0`: start-of-step vessel temperature).
    fn add_step(&mut self, sim: &Sim, t0: f64, dt: f64) {
        let f = sim.vessel.flows;
        let ic_duty: f64 = sim.ic.iter().map(|t| t.last.q_pool).sum();
        let net = sim.q_core - vessel_heat_loss(t0) - ic_duty - sim.cond.q_cw_w - sim.turbine.p_mech + sim.fw.q_pump_w
            - sim.fw.q_loss_w
            - f.w_vent * f.h_vent;
        self.mass_kg -= f.w_vent * dt;
        self.energy_j += net * dt;
        self.core_j += sim.q_core * dt;
        self.cw_j += sim.cond.q_cw_w * dt;
        self.shaft_j += sim.turbine.p_mech * dt;
        self.elec_j += sim.turbine.p_elec * dt;
    }
}

struct Outcome {
    peak_p_mpa: f64,
    min_level_cm: f64,
    peak_fuel_c: f64,
    scram_s: Option<f64>,
    mass_res_kg: f64,
    energy_res_rel: f64,
    plant_mass_res_kg: f64,
    plant_energy_res_rel: f64,
    plant: PlantFlows,
    failures: Vec<String>,
}

/// Runs one case at timestep `dt`; writes its trace when `out_dir` is given.
fn run_case(case: &Case, dt: f64, out_dir: Option<&std::path::Path>) -> Outcome {
    let mut sim = Sim::new(case.scenario);
    let (m0, e0) = (sim.vessel.mass, sim.vessel.energy);
    let (pm0, pe0) = plant_inventory(&sim);
    let (cm0, ce0) = plant_counters(&sim);
    let plant_residual = |sim: &Sim, flows: &PlantFlows| {
        let ((pm, pe), (cm, ce)) = (plant_inventory(sim), plant_counters(sim));
        ((pm - pm0) - (cm - cm0) - flows.mass_kg, (pe - pe0) - (ce - ce0) - flows.energy_j)
    };
    let (mut m_ports, mut e_ports, mut q_core_j) = (0.0, 0.0, 0.0);
    let mut o = Outcome {
        peak_p_mpa: 0.0,
        min_level_cm: f64::MAX,
        peak_fuel_c: 0.0,
        scram_s: None,
        mass_res_kg: 0.0,
        energy_res_rel: 0.0,
        plant_mass_res_kg: 0.0,
        plant_energy_res_rel: 0.0,
        plant: PlantFlows::default(),
        failures: Vec::new(),
    };
    let mut events = case.events.clone();
    let mut csv = String::from(CSV_HEADER);
    let mut next_sample = 0.0;
    let steps = (case.duration_s / dt).round() as usize;
    for i in 0..=steps {
        while events.first().is_some_and(|(t, _)| *t <= sim.time + 1e-9) {
            sim.apply(events.remove(0).1);
        }
        let s = sim.snapshot();
        let m_res = (sim.vessel.mass - m0) - m_ports;
        let e_res = (sim.vessel.energy - e0) - e_ports;
        let (pm_res, pe_res) = plant_residual(&sim, &o.plant);
        let ic_mw: f64 = s.ic.iter().map(|t| t.heat_mw).sum();
        let row = [
            sim.time, s.neutronics.power_frac, sim.q_core / 1e6, s.vessel.pressure_mpa, s.vessel.level_nr_cm,
            s.vessel.coolant_temp_c, s.neutronics.fuel_temp_c, s.vessel.steam_flow_kgs, s.vessel.feed_flow_kgs,
            ic_mw, s.turbine.net_mwe, s.condenser.pressure_kpa, m_res, e_res, pm_res, pe_res,
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
        let t0 = sim.vessel.eos.t;
        let (dm, de) = port_balance_step(&mut sim, dt);
        m_ports += dm;
        e_ports += de;
        q_core_j += sim.q_core * dt;
        o.plant.add_step(&sim, t0, dt);
        if sim.cond.w_out_shortfall > 0.0 {
            o.failures.push(format!("hotwell withdrawal shortfall {:.3e} kg/s at t={:.2} s", sim.cond.w_out_shortfall, sim.time));
            break;
        }
    }
    o.mass_res_kg = (sim.vessel.mass - m0) - m_ports;
    o.energy_res_rel = ((sim.vessel.energy - e0) - e_ports).abs() / q_core_j.max(1.0);
    if o.mass_res_kg.abs() > MASS_TOL_KG {
        o.failures.push(format!("mass residual {:.3e} kg exceeds {MASS_TOL_KG:.0e} kg", o.mass_res_kg));
    }
    if o.energy_res_rel > ENERGY_TOL_REL {
        o.failures.push(format!("energy residual {:.3e} of core heat exceeds {ENERGY_TOL_REL:.0e}", o.energy_res_rel));
    }
    let (pm_res, pe_res) = plant_residual(&sim, &o.plant);
    o.plant_mass_res_kg = pm_res;
    o.plant_energy_res_rel = pe_res.abs() / q_core_j.max(1.0);
    if pm_res.abs() > PLANT_MASS_TOL_KG {
        o.failures.push(format!("plant mass residual {pm_res:.3e} kg exceeds {PLANT_MASS_TOL_KG:.0e} kg"));
    }
    if o.plant_energy_res_rel > PLANT_ENERGY_TOL_REL {
        o.failures.push(format!("plant energy residual {:.3e} of core heat exceeds {PLANT_ENERGY_TOL_REL:.0e}", o.plant_energy_res_rel));
    }
    if let Some(dir) = out_dir {
        let path = dir.join(format!("{}.csv", case.name));
        if let Err(e) = std::fs::write(&path, csv) {
            o.failures.push(format!("cannot write {}: {e}", path.display()));
        }
    }
    o
}

fn main() {
    let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/validation/traces");
    std::fs::create_dir_all(&out_dir).expect("create trace directory");
    println!("dt = {DT} s; vessel ports: energy {ENERGY_TOL_REL:.0e} of core heat, mass {MASS_TOL_KG:.0e} kg;");
    println!("whole plant: energy {PLANT_ENERGY_TOL_REL:.0e} of core heat, mass {PLANT_MASS_TOL_KG:.0e} kg");
    println!(
        "{:<24} {:>8} {:>10} {:>10} {:>9} {:>11} {:>11} {:>11} {:>11}  result",
        "case", "peakPmpa", "minLvl_cm", "peakFuel_C", "scram_s", "vesMass_kg", "vesEnergy", "plantMs_kg", "plantEnergy"
    );
    let mut failed = false;
    let mut reference = None;
    for case in cases() {
        let o = run_case(&case, DT, Some(&out_dir));
        let scram = o.scram_s.map_or("-".to_string(), |t| format!("{t:.2}"));
        let result = if o.failures.is_empty() { "PASS".to_string() } else { format!("FAIL: {}", o.failures.join("; ")) };
        println!(
            "{:<24} {:>8.3} {:>10.1} {:>10.0} {:>9} {:>11.2e} {:>11.2e} {:>11.2e} {:>11.2e}  {result}",
            case.name, o.peak_p_mpa, o.min_level_cm, o.peak_fuel_c, scram, o.mass_res_kg, o.energy_res_rel,
            o.plant_mass_res_kg, o.plant_energy_res_rel
        );
        let gj = |j: f64| j / 1e9;
        println!(
            "{:<24} core {:.1} GJ, CW {:.1} GJ, shaft {:.1} GJ, electrical {:.1} GJ",
            "", gj(o.plant.core_j), gj(o.plant.cw_j), gj(o.plant.shaft_j), gj(o.plant.elec_j)
        );
        failed |= !o.failures.is_empty();
        if case.name == REFINE_CASE {
            reference = Some((o.peak_p_mpa, o.min_level_cm));
        }
    }

    // Timestep refinement: halving dt must not move the limiting values.
    let case = cases().into_iter().find(|c| c.name == REFINE_CASE).expect("refinement case exists");
    let (p_ref, lvl_ref) = reference.expect("refinement case ran");
    let fine = run_case(&case, REFINE_DT, None);
    let (dp, dl) = (fine.peak_p_mpa - p_ref, fine.min_level_cm - lvl_ref);
    let ok = fine.failures.is_empty() && dp.abs() <= REFINE_PEAK_P_TOL_MPA && dl.abs() <= REFINE_MIN_LEVEL_TOL_CM;
    println!(
        "refinement {REFINE_CASE} dt {DT} -> {REFINE_DT} s: peak P {p_ref:.4} -> {:.4} MPa (d {dp:+.4}, bound {REFINE_PEAK_P_TOL_MPA}), \
         min level {lvl_ref:.2} -> {:.2} cm (d {dl:+.2}, bound {REFINE_MIN_LEVEL_TOL_CM})  {}",
        fine.peak_p_mpa,
        fine.min_level_cm,
        if ok { "PASS".to_string() } else { format!("FAIL {}", fine.failures.join("; ")) }
    );
    failed |= !ok;
    println!("traces: {}", out_dir.canonicalize().unwrap_or(out_dir).display());
    if failed {
        std::process::exit(1);
    }
}
