//! Limiting-state fixtures for the 2026-10-22 round-3 and round-5 findings: a
//! water-solid vessel and the main steam line, supply-limited steam delivery,
//! isolation condenser duty across 100 C, discharge into condenser vacuum and
//! noncondensable transport. These are valid thermodynamic boundary states,
//! not claimed reachable operator sequences.

use bwr_core::api::ScenarioName;
use bwr_core::bop::{valve_flow, Condenser, Feedwater, LevelControlMode, LineGas};
use bwr_core::params::*;
use bwr_core::sim::DT;
use bwr_core::steam::{isentropic_drop, sat_p, sat_t};
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
    set_sealed_vessel_with_gas(sim, m, t, 0.0);
}

/// As `set_sealed_vessel`, with `gas` kg of trapped air in the head space.
fn set_sealed_vessel_with_gas(sim: &mut Sim, m: f64, t: f64, gas: f64) {
    sim.vessel.head_vent_open = false;
    sim.vessel.gas_mass = gas;
    sim.vessel.mass = m;
    sim.vessel.energy = energy_for(m, t).unwrap();
    sim.vessel.eos = solve_eos_with(m, sim.vessel.energy, HeadSpace::Sealed { gas_mass: gas }).expect("eos");
}

/// Accepted vessel water transfers of the last step, kg/s (in minus out).
fn vessel_net_water(sim: &Sim) -> f64 {
    let f = sim.vessel.flows;
    f.w_feed + f.w_ic_return - f.w_steam - f.w_liquid - f.w_vent - f.w_ic_steam
}

/// A cold-shutdown plant at `t` with the given sealed inventory, MSIVs open,
/// the bypass manually full open, feed off and an evacuated 35 C condenser.
fn open_line_cold_fixture(t: f64, m: f64, gas: f64) -> Sim {
    let mut sim = Sim::new(ScenarioName::ColdShutdown);
    set_sealed_vessel_with_gas(&mut sim, m, t, gas);
    sim.steam.msiv_cmd = [true; N_STEAM_LINES];
    sim.steam.msiv_pos = [1.0; N_STEAM_LINES];
    sim.steam.bypass_auto = false;
    sim.steam.bypass_manual_pos = 1.0;
    sim.steam.bypass_pos = 1.0;
    sim.cond = Condenser::new(true);
    sim.cond.set_temperature(35.0);
    sim.fw = Feedwater::new(0, t, LevelControlMode::Manual, 0.0);
    sim.core.fuel_t = t;
    sim
}

/// Round-5 decision (Sol R1, Astra R1/R2): the main steam line carries dome
/// gas only. A water-solid vessel at power conditions with MSIVs and TCVs open
/// delivers nothing to it: the turbine gets no inlet and does no work, the
/// condenser receives no line flow, and vessel mass and energy close on the
/// remaining ports.
fn water_solid_main_line_fixture(t: f64) {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 1.0);
    let m = 1.002 * V_VESSEL_FLUID * sat_t(t).rho_f;
    set_sealed_vessel(&mut sim, m, t);
    assert!(sim.vessel.eos.solid, "fixture must be water-solid: {:?}", sim.vessel.eos);
    assert!(sim.steam.tcv_pos > 0.1 && !sim.turbine.tripped, "line must be open to the turbine");
    let (m0, e0) = (sim.vessel.mass, sim.vessel.energy);
    let heat_loss = HEAT_LOSS_HOT * ((sim.vessel.eos.t - 20.0) / 268.0).max(0.0);
    sim.step(DT);
    assert!(sim.model_error.is_none(), "{:?}", sim.model_error);

    let f = sim.vessel.flows;
    assert_eq!((f.w_steam, f.w_steam_gas), (0.0, 0.0), "{t} C: water-solid main-line delivery");
    assert_eq!(sim.steam.total_flow(), 0.0);
    assert_eq!(sim.turbine.p_mech, 0.0);
    let ic_in: f64 = sim.ic.iter().map(|tr| tr.last.w_steam_in).sum();
    assert_eq!(ic_in, 0.0, "a water-solid vessel supplies no IC steam");
    // The condenser receives only the RWCU reject, net of extraction.
    let q_in = f.w_liquid * f.h_liquid - sim.fw.q_extraction_w;
    assert!((sim.cond.q_in_w - q_in).abs() <= 1e-9 * q_in.abs().max(1.0), "{} vs {q_in}", sim.cond.q_in_w);

    let res_m = sim.vessel.mass - m0 - vessel_net_water(&sim) * DT;
    assert!(res_m.abs() <= 1e-9 * m0, "{t} C: vessel mass residual {res_m} kg");
    let de = sim.q_core - heat_loss + f.w_feed * sim.fw.h_feed() - f.w_vent * f.h_vent - f.w_liquid * f.h_liquid;
    let res = sim.vessel.energy - e0 - de * DT;
    assert!(res.abs() <= 1e-9 * e0.abs(), "{t} C: vessel energy residual {res} J");
}

#[test]
fn water_solid_vessel_at_110c_delivers_no_main_line_flow() {
    water_solid_main_line_fixture(110.0);
}

#[test]
fn water_solid_vessel_at_200c_delivers_no_main_line_flow() {
    water_solid_main_line_fixture(200.0);
}

/// Astra round-5 R2: sealed, gas-free, water-solid 40 C vessel (~250 t,
/// ~2.28 MPa) with the bypass open into a working condenser. It used to drain
/// 292.5 kg in 1 s at dt = 0.05 s against 234.6 kg at 0.001 s and flash to a
/// steam space; now nothing leaves through the line at any step size, the
/// vessel stays water-solid and the end state does not depend on dt.
#[test]
fn cold_water_solid_vessel_does_not_drain_through_open_bypass() {
    let t = 40.0;
    let m = 1.001 * V_VESSEL_FLUID * sat_t(t).rho_f;
    let mut ends = Vec::new();
    for dt in [0.05_f64, 0.025, 0.0125] {
        let mut sim = open_line_cold_fixture(t, m, 0.0);
        assert!(sim.vessel.eos.solid && sim.vessel.eos.p > 2.0e6, "{:?}", sim.vessel.eos);
        for _ in 0..(1.0 / dt).round() as usize {
            let m0 = sim.vessel.mass;
            assert!(sim.step(dt), "{:?}", sim.model_error);
            assert_eq!(sim.steam.total_flow(), 0.0);
            assert_eq!(sim.vessel.flows.w_steam, 0.0);
            let res = sim.vessel.mass - m0 - vessel_net_water(&sim) * dt;
            assert!(res.abs() <= 1e-9 * m0, "dt {dt}: mass residual {res}");
        }
        assert!(sim.vessel.eos.solid, "dt {dt}: vessel flashed {:?}", sim.vessel.eos);
        ends.push((sim.vessel.mass, sim.vessel.eos.p));
    }
    for &(m_end, p_end) in &ends[1..] {
        assert!((m_end - ends[0].0).abs() <= 1e-6 * m_end, "mass {m_end} vs {}", ends[0].0);
        assert!((p_end - ends[0].1).abs() <= 1e-3 * p_end, "pressure {p_end} vs {}", ends[0].1);
    }
}

/// Sol R4 / Astra R3: a sealed, gas-free 90 C vessel (dome ~70.18 kPa) with
/// the bypass open discharges into the ~5.9 kPa condenser instead of stopping
/// at atmospheric pressure; the line solve uses the condenser as its receiver.
#[test]
fn sub_atmospheric_vessel_discharges_into_condenser_vacuum() {
    let probe = Sim::new(ScenarioName::ColdShutdown);
    for dt in [0.05_f64, 0.025, 0.0125] {
        let mut sim = open_line_cold_fixture(90.0, probe.vessel.mass, 0.0);
        let (p0, p_cond) = (sim.vessel.eos.p, sim.cond.pressure());
        assert!((p0 - 70_182.36).abs() < 1e-3 * p0 && p_cond < 7.0e3, "dome {p0}, condenser {p_cond}");
        assert!(sim.step(dt), "{:?}", sim.model_error);
        let st = &sim.steam;
        assert!(st.w_bypass > 1.0, "dt {dt}: bypass flow {}", st.w_bypass);
        assert_eq!(st.p_down, p_cond);
        assert!(st.p_chest > p_cond && st.p_chest < p0, "chest {}", st.p_chest);
        assert!((st.total_flow() - sim.vessel.flows.w_steam).abs() <= 1e-9 * st.total_flow());
    }
}

/// Sol R2 / Astra R4: sealed 90 C, 180 t water with 100 kg air, line open into
/// the condenser. The line takes air with the vapour at the dome mass ratio,
/// the vessel gas inventory falls, the condenser books exactly that gas, and
/// the vessel water + gas balance closes.
#[test]
fn gas_bearing_main_line_flow_purges_air_to_the_condenser() {
    for dt in [0.05_f64, 0.025, 0.0125] {
        let mut sim = open_line_cold_fixture(90.0, 180_000.0, 100.0);
        let v = &sim.vessel;
        let ratio0 = v.gas_mass / (v.mass * v.eos.x);
        assert!((ratio0 - 3.615915).abs() < 1e-3 * ratio0, "initial air/vapour {ratio0}");
        let (mut first, mut water_out) = (None, 0.0);
        for _ in 0..(1.0 / dt).round() as usize {
            let (m0, g0) = (sim.vessel.mass, sim.vessel.gas_mass);
            assert!(sim.step(dt), "{:?}", sim.model_error);
            let f = sim.vessel.flows;
            first.get_or_insert(f.w_steam_gas / f.w_steam);
            water_out += f.w_steam * dt;
            assert!((sim.steam.total_flow() - (f.w_steam + f.w_steam_gas)).abs() <= 1e-9 * sim.steam.total_flow());
            let res = (sim.vessel.mass + sim.vessel.gas_mass) - (m0 + g0) - (vessel_net_water(&sim) - f.w_steam_gas) * dt;
            assert!(res.abs() <= 1e-9 * m0, "dt {dt}: water + gas residual {res} kg");
        }
        let first = first.unwrap();
        assert!((first - ratio0).abs() <= 1e-9 * ratio0, "line air/vapour {first} vs dome {ratio0}");
        let gas_out = 100.0 - sim.vessel.gas_mass;
        assert!(gas_out > 0.1 && water_out > 0.0, "dt {dt}: gas out {gas_out}, water out {water_out}");
        assert!((sim.cond.noncondensable_in_kg - gas_out).abs() <= 1e-9 * 100.0, "booked {} vs {gas_out}", sim.cond.noncondensable_in_kg);
    }
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
    // Valve relation into the condenser at the delivered flow (areas fixed for the step).
    assert_eq!(st.p_down, p_cond);
    let w_tcv = valve_flow(C_TCV * st.tcv_pos, st.p_chest, p_cond);
    assert!((st.w_tcv - w_tcv).abs() <= 1e-9 * st.w_tcv.max(1e-6), "TCV {} vs {w_tcv}", st.w_tcv);
    assert!((st.w_bypass - valve_flow(C_BYPASS * st.bypass_pos, st.p_chest, p_cond)).abs() <= 1e-9 * st.w_bypass.max(1e-6));
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

/// Round-6 fixture: sealed 90 C, 180 t water, 100 kg air; MSIVs open into the
/// evacuated condenser (~5928.62 Pa) through the TCVs at 0.01 (bypass shut,
/// turbine stop valves open).
fn gas_turbine_fixture() -> Sim {
    let mut sim = open_line_cold_fixture(90.0, 180_000.0, 100.0);
    sim.steam.bypass_manual_pos = 0.0;
    sim.steam.bypass_pos = 0.0;
    sim.steam.tcv_pos = 0.01;
    sim
}

fn dome_gas(sim: &Sim) -> LineGas {
    LineGas::new(sim.vessel.dome_gas_mass_fraction(), sim.vessel.dome_water_mole_fraction())
}

/// Round-6 Sol R2 / Astra R1: three-element feed replaces the water that left,
/// not the air purged with it, and the published steam flows are water only.
#[test]
fn gas_bearing_line_feeds_forward_water_only() {
    let mut sim = open_line_cold_fixture(90.0, 180_000.0, 100.0);
    let cap = dome_gas(&sim).capacity;
    assert!(sim.step(0.05), "{:?}", sim.model_error);
    let f = sim.vessel.flows;
    // The reviewers' 3.286540 water + 11.883851 air kg/s used the pure-steam
    // valve capacity. The mixture capacity raises both alike, by slightly less
    // than the valve factor because the larger flow also drops more in the line.
    let rise = (f.w_steam / 3.286540, f.w_steam_gas / 11.883851);
    assert!((rise.0 - rise.1).abs() < 1e-5 && rise.0 > 1.15 && rise.0 < cap, "rise {rise:?}, valve factor {cap}");
    let st = &sim.steam;
    assert!((st.water_flow() - f.w_steam).abs() <= 1e-12 * st.total_flow(), "water {} vs {}", st.water_flow(), f.w_steam);
    assert!((st.total_flow() - st.water_flow() - f.w_steam_gas).abs() <= 1e-9 * st.total_flow());
    let snap = sim.snapshot();
    let published = snap.steam.steam_to_turbine_kgs + snap.steam.steam_to_bypass_kgs;
    assert!((published - snap.vessel.steam_flow_kgs).abs() <= 1e-9 * published, "published {published} vs vessel {}", snap.vessel.steam_flow_kgs);

    // Coupled: the next step's controller scan receives the water signal. A
    // clone fed the water loss directly reproduces it; one fed the mixture does not.
    let (p, level) = (sim.vessel.eos.p, sim.vessel.level_nr_cm());
    // One manual scan at FCV 0.5 seeds the integral there (bumpless transfer).
    sim.fw.fcv_pos = 0.5;
    sim.fw.fcv_manual = 0.5;
    sim.fw.step_hydraulics(0.05, p, level, 0.0, true);
    sim.fw.mode = LevelControlMode::ThreeElement;
    sim.fw.level_sp_cm = level;
    let (mut water_ref, mut mixture_ref) = (sim.fw.clone(), sim.fw.clone());
    for fw in [&mut water_ref, &mut mixture_ref] {
        fw.setdown_cm = 0.0;
    }
    water_ref.step_hydraulics(0.05, p, level, f.w_steam, true);
    mixture_ref.step_hydraulics(0.05, p, level, f.w_steam + f.w_steam_gas, true);
    assert!(sim.step(0.05), "{:?}", sim.model_error);
    assert_eq!(sim.fw.fcv_pos, water_ref.fcv_pos);
    assert!(mixture_ref.fcv_pos > water_ref.fcv_pos + 1e-4, "mixture {} vs water {}", mixture_ref.fcv_pos, water_ref.fcv_pos);
}

/// Round-6 Sol R1 / Astra R2: the air-bearing chest stream is all vapour at
/// the water partial pressure (~70.18 kPa, not the ~230 kPa total); the work
/// bound is the water's own isentropic drop between partial pressures, and a
/// gas-free inlet is exactly the pure-steam one.
#[test]
fn gas_bearing_turbine_inlet_uses_water_partial_pressure() {
    use bwr_core::bop::TurbineInlet;
    let sim = gas_turbine_fixture();
    let (v, p_cond) = (&sim.vessel, sim.cond.pressure());
    assert!((p_cond - 5928.62).abs() < 0.1, "condenser {p_cond}");
    let gas = dome_gas(&sim);
    let h = v.steam_port_enthalpy();
    let mut st = sim.steam.clone();
    st.compute_flows_from(v.eos.p, h, v.steam_line_density(), gas, p_cond, true);
    assert!(st.p_chest > 2.29e5 && v.eos.p - st.p_chest < 1.0, "chest {} vs dome {}", st.p_chest, v.eos.p);
    let p_w = st.p_chest * gas.y_water;
    assert!((p_w - sat_t(90.0).p).abs() < 1e-4 * p_w, "water partial {p_w}");
    let w = st.w_tcv * st.water_frac;
    let inlet = TurbineInlet::mixture(w, h, st.p_chest, p_cond, gas.y_water);
    assert_eq!(inlet.w_liquid, 0.0, "no condensate invented at the water partial pressure");
    assert_eq!((inlet.w_vapour, inlet.h_vapour), (w, h));
    let bound = TURBINE_ETA_IS * isentropic_drop(h, p_w, p_cond * gas.y_water);
    assert!((inlet.work_limit - bound).abs() <= 1e-9 * bound, "work bound {} vs {bound}", inlet.work_limit);
    // Frozen-composition expansion to the water's exhaust partial pressure,
    // below the ~453.6 kJ/kg the total-pressure state claimed.
    assert!(inlet.work_limit < TurbineInlet::new(w, h, st.p_chest, p_cond).work_limit);
    assert_eq!(inlet.h_floor, sat_p(p_cond).h_f);
    // The total-pressure state wrongly made ~2.43% liquid.
    assert!(TurbineInlet::new(w, h, st.p_chest, p_cond).w_liquid > 0.02 * w);
    // Pure steam: identical to the pure-water inlet.
    for (p_chest, h) in [(6.9e6, sat_p(6.9e6).h_g), (6.9e6, sat_p(7.1e6).h_g), (0.5e6, 2.9e6)] {
        assert_eq!(TurbineInlet::mixture(400.0, h, p_chest, 5.6e3, 1.0), TurbineInlet::new(400.0, h, p_chest, 5.6e3));
    }
}

/// Independent ideal-gas choked law at fixed T: w / (A p) ~ sqrt(g / R) psi(g).
fn ideal_choked(gamma: f64, r: f64) -> f64 {
    (gamma / r).sqrt() * (2.0 / (gamma + 1.0)).powf((gamma + 1.0) / (2.0 * (gamma - 1.0)))
}

/// Round-6 Sol R3 / Astra R3: at fixed temperature and pressures, choked valve
/// capacity follows the mixture gas constant and gamma, rising from the steam
/// calibration toward the air value (about 1.30) as the gas fraction -> 1.
#[test]
fn valve_capacity_follows_composition_at_fixed_temperature() {
    let (p, p_down) = (100_000.0, 5_000.0);
    let rho_steam = sat_t(110.0).rho_g;
    let w_per_p = |gas: LineGas, rho: f64| {
        let mut s = bwr_core::bop::SteamSystem::new(true);
        s.tcv_pos = 0.001;
        s.compute_flows_from(p, 2.69e6, rho, gas, p_down, true);
        assert!(p - s.p_chest < 0.01, "line drop {}", p - s.p_chest);
        s.total_flow() / s.p_chest
    };
    let base = w_per_p(LineGas::STEAM, rho_steam);
    assert_eq!(base, C_TCV * 0.001, "pure steam is the calibration");
    assert_eq!(w_per_p(LineGas::new(0.0, 1.0), rho_steam), base);
    let mut prev = 1.0;
    for x in [1e-9, 0.1, 0.3, 0.5, 0.7834, 0.9, 1.0] {
        let gas = LineGas::new(x, 0.5);
        let rho = rho_steam / (1.0 - x * (1.0 - 461.526 / 287.05)).max(1e-9);
        let ratio = w_per_p(gas, rho) / base;
        // Independent ideal mixture: R and cp mix by mass, gamma = cp / cv.
        let (r_m, cp_m) = ((1.0 - x) * 461.526 + x * 287.05, (1.0 - x) * 2000.038 + x * 1004.675);
        let expect = ideal_choked(cp_m / (cp_m - r_m), r_m) / ideal_choked(1.3, 461.526);
        assert!((ratio - expect).abs() < 1e-4 * expect, "x {x}: {ratio} vs {expect}");
        assert!(ratio > prev, "x {x}: capacity must rise toward air");
        prev = ratio;
    }
    assert!((prev - 1.3012).abs() < 2e-3, "air limit {prev}");
    let mid = LineGas::new(0.78335823, 0.3);
    assert!((mid.capacity - 1.21).abs() < 5e-3, "78.3% air {}", mid.capacity);
    // The air critical ratio follows the ideal law, scaled to the 0.55 calibration.
    let air = LineGas::new(1.0, 0.0);
    assert!((air.critical_ratio - 0.55 * 0.528282 / 0.545727).abs() < 1e-4, "air critical {}", air.critical_ratio);
}

/// Round-6 R3: the sealed 90 C 0 / 100 kg air fixtures no longer share one
/// normalized choked flow, and the supply-limited commit inverts the same
/// mixture law (valve relation holds at the delivered flow, subcritical too).
#[test]
fn mixture_valve_law_is_consistent_in_delivery() {
    let (dry, wet) = (open_line_cold_fixture(90.0, 180_000.0, 0.0), gas_turbine_fixture());
    let per_p = |sim: &Sim| {
        let v = &sim.vessel;
        let mut s = sim.steam.clone();
        s.tcv_pos = 0.001;
        s.bypass_pos = 0.0;
        s.compute_flows_from(v.eos.p, v.steam_port_enthalpy(), v.steam_line_density(), dome_gas(sim), sim.cond.pressure(), true);
        s.total_flow() / s.p_chest
    };
    let ratio = per_p(&wet) / per_p(&dry);
    assert!((ratio - dome_gas(&wet).capacity).abs() < 1e-9 && ratio > 1.19, "wet/dry {ratio}");

    for p_down in [5.0e3, 90.0e3] {
        let gas = LineGas::new(0.6, 0.4);
        let mut s = bwr_core::bop::SteamSystem::new(true);
        s.tcv_pos = 0.4;
        s.bypass_pos = 0.3;
        s.compute_flows_from(100.0e3, 2.69e6, 0.8, gas, p_down, true);
        let w = s.total_flow();
        let a = C_TCV * 0.4 + C_BYPASS * 0.3;
        assert!((gas.valve_flow(a, s.p_chest, p_down) - w).abs() <= 1e-9 * w);
        s.limit_to_delivered(0.2 * w, 0.3 * w);
        let wd = s.total_flow();
        assert!((wd - 0.5 * w).abs() < 1e-9 * w && (s.water_frac - 0.4).abs() < 1e-12);
        assert!((gas.valve_flow(a, s.p_chest, p_down) - wd).abs() < 1e-9 * w, "p_down {p_down}: valve law off at delivered flow");
        assert!((s.water_flow() - 0.2 * w).abs() < 1e-9 * w);
    }
}
