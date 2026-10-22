//! Scripted validation runner: fixed scenarios, fixed durations, dt = 0.05 s.
//! Usage: cargo run --release -p bwr-core --example validation
//!
//! Each case writes a CSV trace to docs/validation/traces/<case>.csv and prints
//! its extrema. The run exits non-zero if any case produces a non-finite value,
//! a physics model error, a kinetics fault, or a residual above the tolerances
//! below. Five gates:
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
//!    shell relief and head vent. RWCU reject returns to the condenser, so it is
//!    internal. Shaft power is the boundary with the turbine-generator; the
//!    electrical output is reported beside it. Numerical corrections are not
//!    boundary terms: the condenser's stored energy must equal the equilibrium
//!    energy of its reported temperature and inventory after every step, to
//!    roundoff (a state it cannot represent is a model error instead).
//! 3. Timestep refinement: one case is rerun at dt = 0.025 s and its peak
//!    pressure and minimum level must agree with the dt = 0.05 s run.
//! 4. Outcomes: the startup case must reach criticality and synchronise the
//!    generator without a scram and end within 0.98-1.2 x its 40% power
//!    target; the isolation-condenser endurance cases (1, 2 and 3 trains in
//!    service after MSIV closure) must hold pressure below the
//!    vessel design pressure and remove decay heat (see `ic_endurance_accept`).
//! 5. Condenser limiting step: cold shutdown, both CW pumps, condenser at 35 degC,
//!    one public 60 s step against 1 s and DT steps of the same minute.
//!
//! These are conservation, convergence and robustness checks of the implemented
//! model, not an independent physical validation against plant data; the
//! reported "peak fuel" is the lumped average fuel node (no cladding model).

use std::path::PathBuf;

use bwr_core::api::{Command, ModeSwitch, ScenarioName};
use bwr_core::bop::LevelControlMode;
use bwr_core::params::*;
use bwr_core::sim::{irm_full_scale, DT};
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
/// Per-step bound on the condenser's |E - E_eq(T, M)|, J: the temperature
/// solve's roundoff on a ~2e10 J node, far below any physical exchange.
const COND_EQ_TOL_J: f64 = 1.0e-2;
const SAMPLE_EVERY_S: f64 = 1.0;
/// Sample columns used by the outcome checks.
const COL_Q_CORE_MW: usize = 2;
const COL_P_MPA: usize = 3;
const COL_IC_MW: usize = 9;
const CSV_HEADER: &str =
    "t_s,power_frac,q_core_mw,pressure_mpa,level_nr_cm,coolant_t_c,fuel_t_c,steam_kgs,feed_kgs,ic_mw,net_mwe,cond_kpa,mass_res_kg,energy_res_j,plant_mass_res_kg,plant_energy_res_j";

struct Case {
    name: &'static str,
    scenario: ScenarioName,
    duration_s: f64,
    events: Vec<(f64, Command)>,
    /// Closed-loop operator actions, called before every step.
    driver: Option<fn(&mut Sim, &mut DriveState)>,
    /// IC trains in service: the others are held closed (out of service)
    /// against auto-initiation before every step.
    ic_in_service: usize,
    /// Case-specific outcome acceptance; returns failures.
    accept: Option<fn(&Case, &Outcome) -> Vec<String>>,
}

impl Case {
    fn scripted(name: &'static str, scenario: ScenarioName, duration_s: f64, events: Vec<(f64, Command)>) -> Self {
        Self { name, scenario, duration_s, events, driver: None, ic_in_service: N_IC_TRAINS, accept: None }
    }
}

/// IC endurance: MSIV closure from full power, run for this long.
const IC_ENDURANCE_S: f64 = 1800.0;
/// Decay heat removal is judged over the last this many seconds.
const IC_REMOVAL_WINDOW_S: f64 = 600.0;

fn cases() -> Vec<Case> {
    let msiv = |line| Command::SetMsiv { line, open: false };
    let isolate = || vec![(5.0, msiv(0)), (5.0, msiv(1))];
    let ic_case = |name, n| Case {
        ic_in_service: n,
        accept: Some(ic_endurance_accept as fn(&Case, &Outcome) -> Vec<String>),
        ..Case::scripted(name, ScenarioName::FullPower, IC_ENDURANCE_S, isolate())
    };
    vec![
        Case::scripted("full_power_steady", ScenarioName::FullPower, 300.0, vec![]),
        Case::scripted("turbine_trip", ScenarioName::FullPower, 180.0, vec![(5.0, Command::TripTurbine)]),
        Case::scripted(
            "manual_scram",
            ScenarioName::FullPower,
            180.0,
            vec![(5.0, Command::ManualScram), (5.0, Command::SetModeSwitch { mode: ModeSwitch::Shutdown })],
        ),
        Case::scripted("msiv_closure_ic_cooling", ScenarioName::FullPower, 600.0, isolate()),
        ic_case("ic_endurance_1_train", 1),
        ic_case("ic_endurance_2_trains", 2),
        ic_case("ic_endurance_3_trains", 3),
        Case {
            driver: Some(startup_driver),
            accept: Some(startup_accept),
            ..Case::scripted("startup_to_sync", ScenarioName::HotStandby, STARTUP_S, vec![])
        },
    ]
}

/// Startup run length, s: examples/startup.rs synchronises at about 5300 s.
const STARTUP_S: f64 = 6600.0;
/// Power the startup driver raises to after synchronisation, fraction of rated.
const STARTUP_TARGET: f64 = 0.4;
/// Accepted end power, multiples of `STARTUP_TARGET` (see `startup_accept`).
const STARTUP_POWER_TOL: (f64, f64) = (0.98, 1.2);

/// State of a closed-loop driver and the milestones it observed.
#[derive(Default)]
struct DriveState {
    next_s: f64,
    order: Vec<usize>,
    ptr: usize,
    stage: u8,
    critical_s: Option<f64>,
    sync_s: Option<f64>,
}

/// The examples/startup.rs operator, once per simulated second: hot standby
/// -> banked rod withdrawal on a period limit -> heat-up -> turbine roll ->
/// synchronisation -> power ascension to `STARTUP_TARGET`. Kept in step with
/// that example (examples cannot share a module without a new file).
fn startup_driver(sim: &mut Sim, d: &mut DriveState) {
    if sim.time > 0.0 && d.critical_s.is_none() && sim.core.rho_total >= 0.0 {
        d.critical_s = Some(sim.time);
    }
    if sim.time + 1e-9 < d.next_s {
        return;
    }
    d.next_s += 1.0;
    if d.order.is_empty() {
        d.order = (0..N_RODS).collect();
        let w = &sim.core.rods.weights;
        d.order.sort_by(|&a, &b| w[b].partial_cmp(&w[a]).unwrap());
        sim.apply(Command::SetLevelSetpoint { cm: -40.0 });
    }
    let n = sim.core.kin.n;
    let period = 1.0 / sim.core.kin.inv_period;
    let lvl = sim.vessel.level_nr_cm();
    sim.apply(Command::SetRwcuReject { kgs: if lvl > 20.0 { 15.0 } else { 0.0 } });
    let r = sim.irm_range;
    if n / irm_full_scale(r) > 0.8 && r < 10 {
        sim.apply(Command::SetIrmRange { range: r + 1 });
    }
    let goal = if d.stage < 2 { 0.06 } else { STARTUP_TARGET };
    let slow = period > 0.0 && period < 150.0;
    if n < goal && !slow && !sim.core.rods.moving() && !sim.scrammed {
        for _ in 0..N_RODS {
            let i = d.order[d.ptr % N_RODS];
            let bank = ((d.ptr / N_RODS) as f64 + 1.0) * 8.0;
            d.ptr += 1;
            let p = sim.core.rods.pos[i];
            if p < bank.min(ROD_FULL_OUT) {
                sim.apply(Command::RodSetTarget { indices: vec![i], target: p + 2.0 });
                break;
            }
        }
    }
    if d.stage == 0 && n > 0.035 && sim.vessel.eos.p > 6.9e6 {
        sim.apply(Command::ResetTurbine);
        sim.apply(Command::SetSpeedSetpoint { rpm: RATED_RPM });
        d.stage = 1;
    }
    if d.stage == 1 && (sim.turbine.speed_rpm - RATED_RPM).abs() < 5.0 && sim.turbine.phase_deg.abs() < 5.0 {
        sim.apply(Command::CloseBreaker);
        if sim.turbine.breaker_closed {
            sim.apply(Command::SetModeSwitch { mode: ModeSwitch::Run });
            d.stage = 2;
            d.sync_s = Some(sim.time);
        }
    }
    if d.stage == 2 && n > 0.2 && sim.fw.running_pumps() < 2 {
        sim.apply(Command::SetCondensatePump { index: 1, on: true });
        sim.apply(Command::SetFeedPump { index: 1, on: true });
        sim.apply(Command::SetLevelControl { mode: LevelControlMode::ThreeElement });
    }
}

/// Startup outcome: critical, synchronised and still online, no scram, power ascended.
fn startup_accept(_: &Case, o: &Outcome) -> Vec<String> {
    let mut f = Vec::new();
    let (Some(crit), Some(sync)) = (o.drive.critical_s, o.drive.sync_s) else {
        f.push(format!("startup milestones missing: critical {:?}, synchronised {:?}", o.drive.critical_s, o.drive.sync_s));
        return f;
    };
    if crit >= sync {
        f.push(format!("synchronised at {sync:.0} s before criticality at {crit:.0} s"));
    }
    if let Some(t) = o.scram_s {
        f.push(format!("scram at {t:.1} s"));
    }
    if !o.gen_online_at_end {
        f.push("generator not online at the end of the run".into());
    }
    let p_end = o.samples.last().map_or(0.0, |r| r[1]);
    // Lower bound is examples/startup.rs's own success test (98% of target). The
    // driver only withdraws rods, so it settles ~15% above target (0.46 at
    // 6600 s); the upper bound catches a runaway ascension without inserting rods.
    let (lo, hi) = (STARTUP_POWER_TOL.0 * STARTUP_TARGET, STARTUP_POWER_TOL.1 * STARTUP_TARGET);
    if !(lo..=hi).contains(&p_end) {
        f.push(format!("power {p_end:.3} at the end, outside [{lo:.3}, {hi:.3}] for target {STARTUP_TARGET}"));
    }
    f
}

/// IC endurance acceptance, for `case.ic_in_service` trains after MSIV closure.
/// No safety/relief valves are modelled, so the ICs are the only pressure
/// relief: peak pressure must stay below the vessel design pressure. Over the
/// last `IC_REMOVAL_WINDOW_S` the trains must remove the decay heat (mean IC
/// duty at least the mean core heat) and the vessel must not be pressurising.
fn ic_endurance_accept(_: &Case, o: &Outcome) -> Vec<String> {
    let mut f = Vec::new();
    if o.peak_p_mpa * 1e6 >= VESSEL_DESIGN_PRESSURE {
        f.push(format!("peak pressure {:.3} MPa reaches design {:.2} MPa", o.peak_p_mpa, VESSEL_DESIGN_PRESSURE / 1e6));
    }
    let t_end = o.samples.last().map_or(0.0, |r| r[0]);
    let window: Vec<_> = o.samples.iter().filter(|r| r[0] >= t_end - IC_REMOVAL_WINDOW_S).collect();
    let mean = |c: usize| window.iter().map(|r| r[c]).sum::<f64>() / window.len().max(1) as f64;
    let (ic_mw, core_mw) = (mean(COL_IC_MW), mean(COL_Q_CORE_MW));
    if ic_mw < core_mw {
        f.push(format!("last {IC_REMOVAL_WINDOW_S} s: IC duty {ic_mw:.2} MW below core heat {core_mw:.2} MW"));
    }
    let (p_start, p_end) = (window.first().map_or(0.0, |r| r[COL_P_MPA]), window.last().map_or(0.0, |r| r[COL_P_MPA]));
    if p_end > p_start + 1e-4 {
        f.push(format!("pressurising over the last {IC_REMOVAL_WINDOW_S} s: {p_start:.4} -> {p_end:.4} MPa"));
    }
    f
}

/// Steps once and returns the (mass, energy) the connected systems report having
/// exchanged with the vessel, using the same ambient loss law as `Vessel::step_with_ic`.
/// Mirrors `port_balance_step` in tests/plant.rs.
fn port_balance_step(sim: &mut Sim, dt: f64) -> (f64, f64) {
    let t0 = sim.vessel.eos.t;
    // IC steam leaves at the start-of-step port enthalpy the trains requested with.
    let h_ic_steam = sim.vessel.steam_port_enthalpy();
    let heat_loss = vessel_heat_loss(t0);
    sim.step(dt);
    let ic_in: f64 = sim.ic.iter().map(|t| t.last.w_steam_in).sum();
    let ic_ret_m: f64 = sim.ic.iter().map(|t| t.last.w_cond_return).sum();
    let ic_ret_e: f64 = sim.ic.iter().map(|t| t.last.w_cond_return * t.last.h_return).sum();
    let f = sim.vessel.flows;
    // RWCU is the flow the vessel accepted (phase-limited), not the fw.rwcu_reject request.
    // The committed line flow carries any dome noncondensables with the vapour;
    // only the water is vessel inventory.
    let (w_steam, w_feed, w_rwcu) = (sim.steam.water_flow(), sim.fw.flow, f.w_liquid);
    let dm = w_feed + ic_ret_m - w_steam - w_rwcu - f.w_vent - ic_in;
    let de = sim.q_core - heat_loss + w_feed * sim.fw.h_feed() + ic_ret_e
        - w_steam * sim.steam.h_steam
        - f.w_vent * f.h_vent
        - ic_in * h_ic_steam
        - w_rwcu * f.h_liquid;
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
/// CST makeup/reject/overflow, shell relief, IC boil-off. Physical exchanges only.
fn plant_counters(sim: &Sim) -> (f64, f64) {
    let c = &sim.cond;
    let boiloff: f64 = sim.ic.iter().map(|t| t.boiloff_kg).sum();
    let mass = c.makeup_kg - c.reject_kg - c.overflow_kg - c.relief_kg - boiloff;
    let energy = c.cst_energy_j - c.relief_energy_j;
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
    /// Largest per-step condenser |E - E_eq(T, M)|, J.
    cond_eq_res_j: f64,
    /// The CSV rows, one per sample.
    samples: Vec<[f64; 16]>,
    drive: DriveState,
    gen_online_at_end: bool,
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
        cond_eq_res_j: 0.0,
        samples: Vec::new(),
        drive: DriveState::default(),
        gen_online_at_end: false,
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
        if let Some(drive) = case.driver {
            drive(&mut sim, &mut o.drive);
        }
        for index in case.ic_in_service..N_IC_TRAINS {
            if sim.ic[index].cmd_open {
                sim.apply(Command::SetIcTrain { index, open: false });
            }
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
            o.samples.push(row);
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
        let eq_res = sim.cond.equilibrium_residual_j().abs();
        o.cond_eq_res_j = o.cond_eq_res_j.max(eq_res);
        if eq_res > COND_EQ_TOL_J {
            o.failures.push(format!("condenser |E - E_eq(T,M)| {eq_res:.3e} J at t={:.2} s", sim.time));
            break;
        }
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
    o.gen_online_at_end = sim.turbine.breaker_closed && !sim.turbine.tripped;
    if let Some(accept) = case.accept {
        let extra = accept(case, &o);
        o.failures.extend(extra);
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
    let mut ic_end_p = Vec::new();
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
            "{:<24} core {:.1} GJ, CW {:.1} GJ, shaft {:.1} GJ, electrical {:.1} GJ, max condenser |E-E_eq| {:.1e} J",
            "", gj(o.plant.core_j), gj(o.plant.cw_j), gj(o.plant.shaft_j), gj(o.plant.elec_j), o.cond_eq_res_j
        );
        let last = o.samples.last().copied().unwrap_or([0.0; 16]);
        if case.driver.is_some() {
            let at = |t: Option<f64>| t.map_or("never".to_string(), |t| format!("{t:.0} s"));
            println!(
                "{:<24} critical at {}, synchronised at {}, end power {:.3}, generator online {}",
                "", at(o.drive.critical_s), at(o.drive.sync_s), last[1], o.gen_online_at_end
            );
        }
        if case.accept.is_some() && case.driver.is_none() {
            println!(
                "{:<24} {} IC train(s): end pressure {:.3} MPa, IC {:.2} MW vs core {:.2} MW at {:.0} s",
                "", case.ic_in_service, last[COL_P_MPA], last[COL_IC_MW], last[COL_Q_CORE_MW], last[0]
            );
            ic_end_p.push((case.ic_in_service, last[COL_P_MPA], o.peak_p_mpa));
        }
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

    // More IC trains in service must not leave the vessel at a higher pressure.
    ic_end_p.sort_by_key(|e| e.0);
    let monotonic = ic_end_p.windows(2).all(|w| w[1].1 <= w[0].1 + 1e-4 && w[1].2 <= w[0].2 + 1e-4);
    println!(
        "IC train count (trains, end MPa, peak MPa): {:?}  {}",
        ic_end_p.iter().map(|e| (e.0, (e.1 * 1e3).round() / 1e3, (e.2 * 1e3).round() / 1e3)).collect::<Vec<_>>(),
        if monotonic { "PASS" } else { "FAIL: more trains gave a higher pressure" }
    );
    failed |= !monotonic;

    let (line, ok) = cold_condenser_check();
    println!("{line}");
    failed |= !ok;
    println!("traces: {}", out_dir.canonicalize().unwrap_or(out_dir).display());
    if failed {
        std::process::exit(1);
    }
}

/// Condenser end temperature (degC), plant energy residual (J) and largest
/// condenser |E - E_eq| (J) over one minute of cold shutdown with both CW
/// pumps and the condenser started at 35 degC, stepped at `dt` through the
/// public API; Err on a rejected step or a model error.
fn cold_condenser_minute(dt: f64) -> Result<(f64, f64, f64), String> {
    let mut sim = Sim::new(ScenarioName::ColdShutdown);
    for index in 0..N_CW_PUMPS {
        sim.apply(Command::SetCwPump { index, on: true });
    }
    sim.cond.set_temperature(35.0);
    let ((pm0, pe0), (cm0, ce0)) = (plant_inventory(&sim), plant_counters(&sim));
    let mut flows = PlantFlows::default();
    let mut max_eq = 0.0f64;
    for _ in 0..(60.0 / dt).round() as usize {
        let t0 = sim.vessel.eos.t;
        if !sim.step(dt) {
            return Err(format!("dt {dt} s step rejected: {:?}", sim.model_error));
        }
        flows.add_step(&sim, t0, dt);
        max_eq = max_eq.max(sim.cond.equilibrium_residual_j().abs());
    }
    let ((pm, pe), (cm, ce)) = (plant_inventory(&sim), plant_counters(&sim));
    let pm_res = (pm - pm0) - (cm - cm0) - flows.mass_kg;
    if pm_res.abs() > PLANT_MASS_TOL_KG {
        return Err(format!("dt {dt} s plant mass residual {pm_res:.3e} kg"));
    }
    Ok((sim.cond.t_sat, (pe - pe0) - (ce - ce0) - flows.energy_j, max_eq))
}

/// Gate 5. The one-step response must stay above the CW inlet, conserve plant
/// energy to the gate-2 tolerance (relative to the heat the CW removed) and
/// agree with the refined steps; the exact solution of the lumped node is
/// 20 + 15 exp(-60 / ~13 s) degC.
fn cold_condenser_check() -> (String, bool) {
    const T_TOL_C: f64 = 0.05;
    let runs: Vec<_> = [60.0, 1.0, DT].iter().map(|&dt| (dt, cold_condenser_minute(dt))).collect();
    let mut fails = Vec::new();
    let mut ends = Vec::new();
    for (dt, r) in &runs {
        match r {
            Err(e) => fails.push(e.clone()),
            Ok((t, pe_res, eq)) => {
                // CW removes about 15 K x 4.8e8 J/K from the node.
                let scale = 7.0e9;
                if *t <= CW_INLET_C {
                    fails.push(format!("dt {dt} s cooled to {t:.3} degC, below the {CW_INLET_C} degC CW inlet"));
                }
                if pe_res.abs() > PLANT_ENERGY_TOL_REL * scale {
                    fails.push(format!("dt {dt} s plant energy residual {pe_res:.3e} J"));
                }
                if *eq > COND_EQ_TOL_J {
                    fails.push(format!("dt {dt} s condenser |E - E_eq| {eq:.3e} J"));
                }
                ends.push((*dt, *t));
            }
        }
    }
    if let Some(&(_, t_ref)) = ends.last() {
        for (dt, t) in &ends {
            if (t - t_ref).abs() > T_TOL_C {
                fails.push(format!("dt {dt} s ends at {t:.4} degC vs {t_ref:.4} degC at dt {DT} s"));
            }
        }
    }
    let ends_txt: Vec<String> = ends.iter().map(|(dt, t)| format!("dt {dt} s -> {t:.4} degC")).collect();
    let ok = fails.is_empty();
    let verdict = if ok { "PASS".to_string() } else { format!("FAIL: {}", fails.join("; ")) };
    (format!("cold condenser 60 s, 35 degC, 2 CW pumps: {}  {verdict}", ends_txt.join(", ")), ok)
}
