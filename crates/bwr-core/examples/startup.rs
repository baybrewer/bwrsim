//! Scripted operator: hot standby -> critical -> heat -> turbine roll -> sync -> 40 % power.
use bwr_core::api::{Command, ModeSwitch, ScenarioName};
use bwr_core::bop::LevelControlMode;
use bwr_core::sim::{irm_full_scale, DT};
use bwr_core::Sim;

fn main() {
    let target: f64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(0.4);
    let mut sim = Sim::new(ScenarioName::HotStandby);
    let mut order: Vec<usize> = (0..57).collect();
    order.sort_by(|&a, &b| sim.core.rods.weights[b].partial_cmp(&sim.core.rods.weights[a]).unwrap());
    let mut ptr = 0usize;
    let mut next_report = 0.0;
    let mut stage = 0;
    sim.apply(Command::SetLevelSetpoint { cm: -40.0 });
    sim.apply(Command::SetSpeed { speed: 1.0 });
    while sim.time < 4.0 * 3600.0 {
        for _ in 0..20 {
            sim.step(DT);
        }
        let n = sim.core.kin.n;
        let period = 1.0 / sim.core.kin.inv_period;
        // Level: reject water through RWCU when high.
        let lvl = sim.vessel.level_nr_cm();
        sim.apply(Command::SetRwcuReject { kgs: if lvl > 20.0 { 15.0 } else { 0.0 } });
        // IRM ranging.
        let r = sim.irm_range;
        if n / irm_full_scale(r) > 0.8 && r < 10 {
            sim.apply(Command::SetIrmRange { range: r + 1 });
        }
        let goal = match stage { 0 | 1 => 0.06, _ => target };
        let slow = period > 0.0 && period < 150.0;
        if n < goal && !slow && !sim.core.rods.moving() && !sim.scrammed {
            // Withdraw the next rod one notch (banked: all rods to 12, then 24, ...).
            for _ in 0..57 {
                let i = order[ptr % 57];
                let bank = ((ptr / 57) as f64 + 1.0) * 8.0;
                ptr += 1;
                let p = sim.core.rods.pos[i];
                if p < bank.min(48.0) {
                    sim.apply(Command::RodSetTarget { indices: vec![i], target: p + 2.0 });
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
                println!("*** synchronised at t={:.0}s", sim.time);
            }
        }
        if stage == 2 && n > 0.2 && sim.fw.running_pumps() < 2 {
            sim.apply(Command::SetCondensatePump { index: 1, on: true });
            sim.apply(Command::SetFeedPump { index: 1, on: true });
            sim.apply(Command::SetLevelControl { mode: LevelControlMode::ThreeElement });
        }
        if sim.time >= next_report {
            let s = sim.snapshot();
            println!(
                "t={:6.0} st{} n={:8.4}% per={:7.0} P={:.3} lvl={:6.1} T={:5.1} rpm={:5.0} ph={:5.0} MWe={:6.1} byp={:.2} tcv={:.2} fw={:5.1} rods_out={:.1} irm={} xe={:.2} scram={:?}",
                sim.time, stage, n * 100.0, period, s.vessel.pressure_mpa, s.vessel.level_nr_cm, s.vessel.coolant_temp_c,
                s.turbine.speed_rpm, s.turbine.sync_phase_deg, s.turbine.net_mwe, s.steam.bypass_pos, s.steam.tcv_pos,
                s.vessel.feed_flow_kgs, sim.core.rods.pos.iter().sum::<f64>() / 57.0, sim.irm_range, s.neutronics.xenon_rel, s.scram_reason
            );
            next_report += 120.0;
        }
        if stage == 2 && n >= target * 0.98 && sim.time > 600.0 {
            break;
        }
    }
    let s = sim.snapshot();
    println!("END t={:.0} n={:.3} MWe={:.1} scram={:?}", sim.time, s.neutronics.power_frac, s.turbine.net_mwe, s.scram_reason);
    for a in s.alarms {
        println!("  {:?} {}", a.severity, a.text);
    }
}
