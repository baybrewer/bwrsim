//! Print a time trace of key plant variables.
//! Usage: cargo run --example trace -- <scenario> <seconds> [event@time ...]
//! scenario: cold | hot | full. Events: scram, turbinetrip, fwheaters, loadreject,
//! msiv, fwpump, rodsout, pset+ (raise pressure setpoint 0.1 MPa)

use bwr_core::api::{Command, ScenarioName};
use bwr_core::Sim;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let scen = match args.get(1).map(|s| s.as_str()) {
        Some("cold") => ScenarioName::ColdShutdown,
        Some("hot") => ScenarioName::HotStandby,
        _ => ScenarioName::FullPower,
    };
    let secs: f64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(120.0);
    let mut events: Vec<(f64, String)> = args[3.min(args.len())..]
        .iter()
        .filter_map(|e| {
            let (n, t) = e.split_once('@')?;
            Some((t.parse().ok()?, n.to_string()))
        })
        .collect();
    events.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let t0 = std::time::Instant::now();
    let mut sim = Sim::new(scen);
    eprintln!("init took {:?}", t0.elapsed());
    let every = (secs / 40.0).max(1.0);
    let mut next = 0.0;
    println!("{:>7} {:>7} {:>7} {:>6} {:>7} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6}",
        "t", "n%", "Qmw", "Pmpa", "lvl_cm", "Wcore", "void", "Wst", "Wfw", "Tfw", "MWe", "rho", "Tfuel", "ICmw");
    while sim.time < secs {
        while let Some((t, e)) = events.first().cloned() {
            if t > sim.time {
                break;
            }
            events.remove(0);
            let cmd = match e.as_str() {
                "scram" => Command::ManualScram,
                "turbinetrip" => Command::TripTurbine,
                "fwheaters" => Command::SetHeaters { in_service: false },
                "loadreject" => Command::OpenBreaker,
                "msiv" => Command::SetMsiv { line: 0, open: false },
                "fwpump" => Command::SetFeedPump { index: 0, on: false },
                "pset+" => Command::SetPressureSetpoint { mpa: sim.steam.p_setpoint / 1e6 + 0.1 },
                "rodsout" => {
                    let idx: Vec<usize> = (0..57).filter(|&i| sim.core.rods.pos[i] < 48.0).take(4).collect();
                    let tgt = sim.core.rods.pos[idx[0]] + 2.0;
                    Command::RodSetTarget { indices: idx, target: tgt }
                }
                _ => continue,
            };
            println!("--- {e} at {t}");
            sim.apply(cmd);
        }
        if sim.time >= next {
            let s = sim.snapshot();
            println!(
                "{:7.1} {:7.3} {:7.1} {:6.3} {:7.1} {:6.0} {:6.3} {:6.1} {:6.1} {:6.1} {:6.1} {:6.0} {:6.0} {:6.1}",
                sim.time,
                s.neutronics.power_frac * 100.0,
                s.neutronics.thermal_power_mw,
                s.vessel.pressure_mpa,
                s.vessel.level_nr_cm,
                s.vessel.core_flow_kgs,
                s.vessel.core_avg_void,
                s.vessel.steam_flow_kgs,
                s.vessel.feed_flow_kgs,
                s.feedwater.feed_temp_c,
                s.turbine.net_mwe,
                s.neutronics.reactivity.total,
                s.neutronics.fuel_temp_c,
                s.ic.iter().map(|t| t.heat_mw).sum::<f64>(),
            );
            next += every;
        }
        sim.step(bwr_core::sim::DT);
    }
    let s = sim.snapshot();
    for a in &s.alarms {
        println!("ALARM [{:?}] {}", a.severity, a.text);
    }
    println!("scram: {:?}  turbine tripped: {}  rpm {:.0} cond {:.1} kPa", s.scram_reason, s.turbine.tripped, s.turbine.speed_rpm, s.condenser.pressure_kpa);
}
