//! Limiting-state fixtures for the 2026-10-22 round-3 findings: liquid in the
//! main steam line (R1), supply-limited steam delivery (R2) and isolation
//! condenser duty across 100 C (Astra R4). These are valid thermodynamic
//! boundary states, not claimed reachable operator sequences.

use bwr_core::api::ScenarioName;
use bwr_core::bop::{Feedwater, LevelControlMode};
use bwr_core::params::*;
use bwr_core::sim::DT;
use bwr_core::steam::{isentropic_drop, sat_p, sat_t, P_ATM};
use bwr_core::vessel::{energy_for, solve_eos_with, HeadSpace};
use bwr_core::Sim;

fn run(sim: &mut Sim, seconds: f64) {
    for _ in 0..(seconds / DT).round() as usize {
        sim.step(DT);
    }
}

/// Replace only the vessel inventory with a consistent sealed, gas-free state
/// of mass `m` at temperature `t` (M, U and EOS agree).
fn set_sealed_vessel(sim: &mut Sim, m: f64, t: f64) {
    sim.vessel.head_vent_open = false;
    sim.vessel.gas_mass = 0.0;
    sim.vessel.mass = m;
    sim.vessel.energy = energy_for(m, t).unwrap();
    sim.vessel.eos = solve_eos_with(m, sim.vessel.energy, HeadSpace::Sealed { gas_mass: 0.0 }).expect("eos");
}

/// R1: a water-solid vessel discharging compressed liquid into the main steam
/// line. The turbine may extract no more than the reversible (isentropic)
/// work of that liquid, the exhaust stays between saturated liquid at the
/// condenser and the inlet enthalpy, and the condenser inlet energy is >= 0.
fn liquid_main_line_fixture(t: f64) {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 1.0);
    let m = 1.002 * V_VESSEL_FLUID * sat_t(t).rho_f;
    set_sealed_vessel(&mut sim, m, t);
    assert!(sim.vessel.eos.solid, "fixture must be water-solid: {:?}", sim.vessel.eos);
    let (p_dome, p_cond, e0) = (sim.vessel.eos.p, sim.cond.pressure(), sim.vessel.energy);
    let heat_loss = HEAT_LOSS_HOT * ((sim.vessel.eos.t - 20.0) / 268.0).max(0.0);
    sim.step(DT);
    assert!(sim.model_error.is_none(), "{:?}", sim.model_error);

    let f = sim.vessel.flows;
    let (w_tcv, w_byp) = (sim.steam.w_tcv, sim.steam.w_bypass);
    assert!(w_tcv > 10.0, "fixture must pass liquid to the turbine, w_tcv {w_tcv}");
    assert!((f.w_steam - (w_tcv + w_byp)).abs() <= 1e-9 * f.w_steam);
    let h = f.h_steam;
    let (p_mech, q_ext) = (sim.turbine.p_mech, sim.fw.q_extraction_w);

    // Independent reversible limit from the dome state (the chest is lower,
    // so the model's own bound is tighter still).
    let reversible = w_tcv * isentropic_drop(h, p_dome, p_cond);
    assert!(p_mech <= reversible + 1.0, "{t} C: work {p_mech} W > isentropic {reversible} W");
    assert!(p_mech <= w_tcv * TURBINE_ETA_IS * isentropic_drop(h, sim.steam.p_chest, p_cond) + 1.0);

    // Turbine-side exhaust (after work and extraction) is an admissible state.
    let h_exhaust = h - (p_mech + q_ext) / w_tcv;
    let h_floor = sat_p(p_cond).h_f;
    assert!(h_exhaust >= h_floor - 1e-6 && h_exhaust <= h + 1e-6, "{t} C: exhaust {h_exhaust} J/kg (floor {h_floor})");

    // Condenser inlet: non-negative and exactly the delivered energy net of work and extraction.
    let q_in = (w_tcv + w_byp) * h + f.w_liquid * f.h_liquid - p_mech - q_ext;
    assert!(sim.cond.q_in_w >= 0.0, "{t} C: condenser inlet energy {}", sim.cond.q_in_w);
    assert!((sim.cond.q_in_w - q_in).abs() <= 1e-9 * q_in.abs().max(1.0), "{} vs {q_in}", sim.cond.q_in_w);

    // Vessel energy closes on the same port enthalpy the turbine received.
    let ic_in: f64 = sim.ic.iter().map(|tr| tr.last.w_steam_in).sum();
    assert_eq!(ic_in, 0.0, "a water-solid vessel supplies no IC steam");
    let de = sim.q_core - heat_loss + sim.fw.flow * sim.fw.h_feed() - f.w_steam * h - f.w_vent * f.h_vent - f.w_liquid * f.h_liquid;
    let res = sim.vessel.energy - e0 - de * DT;
    assert!(res.abs() <= 1e-9 * e0.abs(), "{t} C: vessel energy residual {res} J");
}

#[test]
fn liquid_main_line_at_110c_gets_no_steam_work() {
    liquid_main_line_fixture(110.0);
}

#[test]
fn liquid_main_line_at_200c_gets_no_steam_work() {
    liquid_main_line_fixture(200.0);
}

/// R1, two-phase inlet: wet steam at the chest expands only its vapour. Work
/// stays below eta * isentropic drop of the whole inlet and the extraction
/// budget never pushes the exhaust below saturated liquid at the condenser.
#[test]
fn wet_turbine_inlet_work_and_extraction_share_one_budget() {
    use bwr_core::bop::TurbineInlet;
    let (p_chest, p_cond) = (6.9e6, 5.6e3);
    let s = sat_p(p_chest);
    for x in [0.0, 0.3, 0.9, 1.0] {
        let h = s.h_f + x * s.h_fg();
        let inlet = TurbineInlet::new(400.0, h, p_chest, p_cond);
        assert!((inlet.w_vapour * inlet.h_vapour + inlet.w_liquid * inlet.h_liquid - 400.0 * h).abs() < 1e-3);
        assert!((inlet.w_vapour - 400.0 * x).abs() < 1e-9);
        let work = inlet.w_vapour * inlet.work_limit;
        assert!(work <= 400.0 * TURBINE_ETA_IS * isentropic_drop(h, p_chest, p_cond) + 1.0, "x {x}");
        let exhaust = 400.0 * h - work - inlet.extraction_budget(work);
        assert!(exhaust >= 400.0 * sat_p(p_cond).h_f - 1e-3, "x {x}: exhaust {exhaust} W");
    }
}

/// R2: the vessel holds almost no vapour, so it delivers far less than the
/// valves request. The committed chest pressure must satisfy the valve
/// relation and the line relation at the delivered flow, with the line inlet
/// (the supply-limited boundary) at or below dome pressure; the turbine and
/// protection then act on that committed state.
#[test]
fn vapour_limited_chest_pressure_matches_delivered_flow() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 1.0);
    let t = sim.vessel.eos.t;
    let m = V_VESSEL_FLUID * sat_t(t).rho_f * (1.0 - 1e-8);
    set_sealed_vessel(&mut sim, m, t);
    let (p_dome, p_cond) = (sim.vessel.eos.p, sim.cond.pressure());
    assert!(!sim.turbine.tripped);
    sim.step(DT);
    assert!(sim.model_error.is_none(), "{:?}", sim.model_error);
    let f = sim.vessel.flows;
    assert!(f.shortfall > 1.0, "case must be supply-limited, shortfall {}", f.shortfall);

    let st = &sim.steam;
    let w = st.total_flow();
    assert!((w - f.w_steam).abs() <= 1e-12 * w.max(1.0), "committed {w} vs delivered {}", f.w_steam);
    // Valve relation at the delivered flow (areas fixed for the step).
    let dp_valve = st.p_chest - P_ATM;
    assert!((st.w_tcv - C_TCV * st.tcv_pos * dp_valve).abs() <= 1e-9 * st.w_tcv.max(1e-6), "TCV {} vs {}", st.w_tcv, C_TCV * st.tcv_pos * dp_valve);
    assert!((st.w_bypass - C_BYPASS * st.bypass_pos * dp_valve).abs() <= 1e-9 * st.w_bypass.max(1e-6));
    // Line relation at the delivered flow, from the inlet boundary.
    let m_open = st.msiv_pos.iter().sum::<f64>() / N_STEAM_LINES as f64;
    let k = STEAM_LINE_DP / (STEAM_FLOW_RATED * STEAM_FLOW_RATED) / (m_open * m_open);
    assert!((st.p_line_inlet - st.p_chest - k * w * w).abs() <= 1e-6, "line dp {} vs {}", st.p_line_inlet - st.p_chest, k * w * w);
    assert!(st.p_line_inlet < p_dome, "starved inlet {} must sit below dome {p_dome}", st.p_line_inlet);

    // Turbine work is bounded at the committed chest pressure.
    assert!(sim.turbine.p_mech <= st.w_tcv * TURBINE_ETA_IS * isentropic_drop(f.h_steam, st.p_chest, p_cond) + 1e-6);
    // Protection sees the same pressure: a starved line is a low-pressure line.
    if st.p_chest > 0.0 && st.p_chest < MSIV_LOW_PRESSURE_RUN {
        assert!(st.msiv_cmd.iter().all(|&c| !c), "low chest pressure {} must isolate in RUN", st.p_chest);
    }
}

/// A sealed, gas-free two-phase vessel at `t` with MSIVs shut, feed off and
/// all IC trains open on 40 C pools after 5 MW of preceding duty.
fn ic_fixture(t: f64) -> Sim {
    let mut sim = Sim::new(ScenarioName::HotStandby);
    let m = 0.6 * V_VESSEL_FLUID * sat_t(t).rho_f;
    set_sealed_vessel(&mut sim, m, t);
    assert!(!sim.vessel.eos.solid && sim.vessel.eos.x > 0.0);
    sim.steam.msiv_cmd = [false; N_STEAM_LINES];
    sim.steam.msiv_pos = [0.0; N_STEAM_LINES];
    sim.fw = Feedwater::new(0, 150.0, LevelControlMode::Manual, 0.0);
    for tr in sim.ic.iter_mut() {
        tr.cmd_open = true;
        tr.valve_pos = 1.0;
        tr.pool_t = 40.0;
        tr.heat = 5.0e6;
        tr.tube_mass = 500.0;
    }
    sim
}

fn ic_duty(sim: &Sim) -> (f64, f64) {
    let q = sim.ic.iter().map(|tr| tr.last.q_pool).sum();
    let w = sim.ic.iter().map(|tr| tr.last.w_steam_in).sum();
    (q, w)
}

/// Astra R4: IC condensation depends on the vapour present, not on a 100 C
/// shortcut. One step across a 0.02 C interval changes duty by ~0, and a
/// sweep across 100 C is smooth.
#[test]
fn ic_duty_is_continuous_across_100c() {
    let temps = [99.0, 99.99, 100.01, 101.0];
    let duties: Vec<(f64, f64)> = temps
        .iter()
        .map(|&t| {
            let mut sim = ic_fixture(t);
            sim.step(DT);
            assert!(sim.model_error.is_none(), "{t} C: {:?}", sim.model_error);
            ic_duty(&sim)
        })
        .collect();
    for (&t, &(q, w)) in temps.iter().zip(&duties) {
        assert!(q > 1.0e6 && w > 0.0, "{t} C: duty {q} W, steam {w} kg/s");
    }
    let (q_lo, q_hi) = (duties[1].0, duties[2].0);
    assert!((q_hi - q_lo).abs() <= 1e-3 * q_hi, "99.99 C {q_lo} W vs 100.01 C {q_hi} W");
    let (w_lo, w_hi) = (duties[1].1, duties[2].1);
    assert!((w_hi - w_lo).abs() <= 1e-3 * w_hi, "steam {w_lo} vs {w_hi} kg/s");
}

/// Astra R4: a passive IC cooldown keeps condensing straight through 100 C.
#[test]
fn ic_cooldown_crosses_100c_without_losing_duty() {
    let mut sim = ic_fixture(101.0);
    let (mut above, mut below) = (None, None);
    let mut prev_q: Option<f64> = None;
    for _ in 0..(3600.0 / DT) as usize {
        sim.step(DT);
        assert!(sim.model_error.is_none(), "{:?}", sim.model_error);
        let (q, _) = ic_duty(&sim);
        let t = sim.vessel.eos.t;
        if let Some(p) = prev_q {
            // The trains' 20 s lag allows ~0.25 % per step; a gate would drop to zero.
            assert!(q >= 0.9 * p, "duty fell {p} -> {q} W at {t} C");
        }
        prev_q = Some(q);
        if t > 100.0 {
            above = Some(q);
        } else {
            below.get_or_insert(q);
        }
        if t < 99.0 {
            break;
        }
    }
    let (above, below) = (above.expect("started above 100 C"), below.expect("cooldown must cross 100 C"));
    assert!(sim.vessel.eos.t < 99.0, "cooldown stalled at {} C", sim.vessel.eos.t);
    assert!(below > 0.95 * above, "duty {above} W above vs {below} W below 100 C");
}
