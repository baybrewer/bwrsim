//! Whole-plant behaviour tests. These check the simulator against how a BWR
//! is expected to behave, not exact numbers.

use bwr_core::api::{Command, ModeSwitch, ScenarioName};
use bwr_core::bop::LevelControlMode;
use bwr_core::params::*;
use bwr_core::sim::{irm_full_scale, DT};
use bwr_core::Sim;

fn run(sim: &mut Sim, seconds: f64) {
    let steps = (seconds / DT).round() as usize;
    for _ in 0..steps {
        sim.step(DT);
    }
}

#[test]
fn full_power_is_steady() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 300.0);
    let s = sim.snapshot();
    assert!((s.neutronics.power_frac - 1.0).abs() < 0.03, "power {}", s.neutronics.power_frac);
    assert!((s.vessel.pressure_mpa - 7.2).abs() < 0.05, "pressure {}", s.vessel.pressure_mpa);
    assert!(s.vessel.level_nr_cm.abs() < 10.0, "level {}", s.vessel.level_nr_cm);
    assert!(s.turbine.net_mwe > 285.0 && s.turbine.net_mwe < 315.0, "net {}", s.turbine.net_mwe);
    assert!(s.vessel.core_flow_kgs > 1800.0 && s.vessel.core_flow_kgs < 2200.0);
    assert!(!s.scrammed);
}

#[test]
fn manual_scram_shuts_down_without_isolation() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    sim.apply(Command::ManualScram);
    sim.apply(Command::SetModeSwitch { mode: ModeSwitch::Shutdown });
    let mut min_level = f64::MAX;
    for _ in 0..(60.0 / DT) as usize {
        sim.step(DT);
        min_level = min_level.min(sim.vessel.level_nr_cm());
    }
    assert!(sim.core.kin.n < 0.01, "power after scram {}", sim.core.kin.n);
    assert!(sim.core.rods.all_in());
    assert!(min_level > L2_CM, "level fell to {min_level} cm");
    assert!(sim.ic.iter().all(|t| !t.cmd_open), "isolation condensers should not start");
}

#[test]
fn turbine_trip_rides_through_on_bypass() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    sim.apply(Command::TripTurbine);
    run(&mut sim, 120.0);
    let s = sim.snapshot();
    assert!(!s.scrammed, "scrammed: {:?}", s.scram_reason);
    assert!(s.neutronics.power_frac < 0.9, "select rod insert should cut power, got {}", s.neutronics.power_frac);
    assert!(s.steam.steam_to_bypass_kgs > 200.0);
    assert!(s.vessel.pressure_mpa < 7.4);
}

#[test]
fn msiv_closure_scrams_and_isolation_condensers_cool() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    sim.apply(Command::SetMsiv { line: 0, open: false });
    sim.apply(Command::SetMsiv { line: 1, open: false });
    run(&mut sim, 180.0);
    assert!(sim.scrammed);
    assert!(sim.ic.iter().any(|t| t.heat > 1.0e6), "IC should be removing heat");
    assert!(sim.vessel.eos.p < VESSEL_DESIGN_PRESSURE);
}

#[test]
fn raising_pressure_raises_power() {
    // Void collapse: positive pressure step adds reactivity in a BWR.
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    let n0 = sim.core.kin.n;
    sim.apply(Command::SetPressureSetpoint { mpa: sim.steam.p_setpoint / 1e6 + 0.1 });
    let mut peak: f64 = 0.0;
    for _ in 0..(20.0 / DT) as usize {
        sim.step(DT);
        peak = peak.max(sim.core.kin.n);
    }
    assert!(peak > n0 + 0.005, "peak {peak} vs {n0}");
}

#[test]
fn startup_from_hot_standby_to_30_percent() {
    let mut sim = Sim::new(ScenarioName::HotStandby);
    sim.apply(Command::SetLevelSetpoint { cm: -40.0 });
    let mut order: Vec<usize> = (0..N_RODS).collect();
    order.sort_by(|&a, &b| sim.core.rods.weights[b].partial_cmp(&sim.core.rods.weights[a]).unwrap());
    let mut ptr = 0usize;
    let mut stage = 0;
    while sim.time < 3.0 * 3600.0 {
        run(&mut sim, 1.0);
        assert!(!sim.scrammed, "scram during startup at t={}: {:?}", sim.time, sim.scram_reason);
        let n = sim.core.kin.n;
        let lvl = sim.vessel.level_nr_cm();
        sim.apply(Command::SetRwcuReject { kgs: if lvl > 20.0 { 15.0 } else { 0.0 } });
        if n / irm_full_scale(sim.irm_range) > 0.8 && sim.irm_range < 10 {
            sim.apply(Command::SetIrmRange { range: sim.irm_range + 1 });
        }
        let goal = if stage < 2 { 0.06 } else { 0.30 };
        let period = 1.0 / sim.core.kin.inv_period;
        if n < goal && !(period > 0.0 && period < 150.0) && !sim.core.rods.moving() {
            for _ in 0..N_RODS {
                let i = order[ptr % N_RODS];
                let bank = ((ptr / N_RODS) as f64 + 1.0) * 8.0;
                ptr += 1;
                if sim.core.rods.pos[i] < bank.min(ROD_FULL_OUT) {
                    sim.apply(Command::RodSetTarget { indices: vec![i], target: sim.core.rods.pos[i] + 2.0 });
                    break;
                }
            }
        }
        if stage == 0 && n > 0.035 && sim.vessel.eos.p > 6.9e6 {
            sim.apply(Command::ResetTurbine);
            sim.apply(Command::SetSpeedSetpoint { rpm: 1800.0 });
            stage = 1;
        }
        if stage == 1 && (sim.turbine.speed_rpm - 1800.0).abs() < 5.0 && sim.turbine.phase_deg.abs() < 5.0 {
            sim.apply(Command::CloseBreaker);
            if sim.turbine.breaker_closed {
                sim.apply(Command::SetModeSwitch { mode: ModeSwitch::Run });
                stage = 2;
            }
        }
        if stage == 2 && n > 0.2 && sim.fw.running_pumps() < 2 {
            sim.apply(Command::SetCondensatePump { index: 1, on: true });
            sim.apply(Command::SetFeedPump { index: 1, on: true });
            sim.apply(Command::SetLevelControl { mode: LevelControlMode::ThreeElement });
        }
        if stage == 2 && n > 0.29 {
            break;
        }
    }
    assert_eq!(stage, 2, "generator never synchronised");
    assert!(sim.core.kin.n > 0.29);
    assert!(sim.snapshot().turbine.net_mwe > 40.0);
}

#[test]
fn json_contract_matches_ui() {
    let sim = Sim::new(ScenarioName::ColdShutdown);
    let v = serde_json::to_value(sim.snapshot()).unwrap();
    for key in ["timeS", "modeSwitch", "neutronics", "rods", "vessel", "steam", "turbine", "condenser", "feedwater", "ic", "alarms"] {
        assert!(v.get(key).is_some(), "missing {key}");
    }
    assert_eq!(v["modeSwitch"], "shutdown");
    assert!(v["neutronics"].get("irmReadingPct").is_some());
    assert!(v["vessel"].get("coreInletSubcoolingKjKg").is_some());
    assert_eq!(v["feedwater"]["levelControlMode"], "manual");

    let cmds = [
        r#"{"type":"rodSetTarget","indices":[1,2],"target":12}"#,
        r#"{"type":"setHeaters","inService":false}"#,
        r#"{"type":"loadScenario","name":"fullPower"}"#,
        r#"{"type":"setLevelControl","mode":"threeElement"}"#,
        r#"{"type":"selectRod","index":null}"#,
        r#"{"type":"manualScram"}"#,
        r#"{"type":"setModeSwitch","mode":"run"}"#,
    ];
    for c in cmds {
        serde_json::from_str::<Command>(c).unwrap_or_else(|e| panic!("{c}: {e}"));
    }
}

#[test]
fn cold_shutdown_is_subcritical_and_rods_blocked() {
    let mut sim = Sim::new(ScenarioName::ColdShutdown);
    run(&mut sim, 60.0);
    let s = sim.snapshot();
    assert!(s.neutronics.k_eff < 0.95);
    assert!(s.neutronics.srm_cps > 10.0 && s.neutronics.srm_cps < 1000.0, "srm {}", s.neutronics.srm_cps);
    sim.apply(Command::RodSetTarget { indices: vec![0], target: 10.0 });
    run(&mut sim, 10.0);
    assert!(sim.core.rods.pos[0] < 0.01, "withdrawal must be blocked in SHUTDOWN");
}
