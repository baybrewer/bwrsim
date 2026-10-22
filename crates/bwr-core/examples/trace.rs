//! Print a time trace of key plant variables.
//! Usage: cargo run --example trace -- <scenario> <seconds> [event@time ...]
//! scenario: cold | hot | full. Events: scram, turbinetrip, fwheaters, loadreject,
//! msiv, fwpump, rodsout, pset+ (raise pressure setpoint 0.1 MPa)
//!
//! Every argument is validated before the simulation is built; a malformed
//! scenario, duration or event exits with status 2 and a diagnostic naming it.

use bwr_core::api::{Command, ScenarioName};
use bwr_core::Sim;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event {
    Scram,
    TurbineTrip,
    FwHeaters,
    LoadReject,
    Msiv,
    FwPump,
    RodsOut,
    PsetUp,
}

const EVENT_NAMES: &str = "scram, turbinetrip, fwheaters, loadreject, msiv, fwpump, rodsout, pset+";

fn parse_scenario(arg: &str) -> Result<ScenarioName, String> {
    match arg {
        "cold" => Ok(ScenarioName::ColdShutdown),
        "hot" => Ok(ScenarioName::HotStandby),
        "full" => Ok(ScenarioName::FullPower),
        other => Err(format!("unknown scenario '{other}' (expected cold | hot | full)")),
    }
}

fn parse_seconds(arg: &str) -> Result<f64, String> {
    let secs: f64 = arg.parse().map_err(|_| format!("duration '{arg}' is not a number"))?;
    if !secs.is_finite() || secs <= 0.0 {
        return Err(format!("duration '{arg}' must be a finite, positive number of seconds"));
    }
    Ok(secs)
}

/// Parse `name@time` into a validated event.
fn parse_event(arg: &str) -> Result<(f64, Event), String> {
    let (name, time) = arg
        .split_once('@')
        .ok_or_else(|| format!("event '{arg}' is not of the form name@time"))?;
    let kind = match name {
        "scram" => Event::Scram,
        "turbinetrip" => Event::TurbineTrip,
        "fwheaters" => Event::FwHeaters,
        "loadreject" => Event::LoadReject,
        "msiv" => Event::Msiv,
        "fwpump" => Event::FwPump,
        "rodsout" => Event::RodsOut,
        "pset+" => Event::PsetUp,
        other => return Err(format!("event '{arg}': unknown event name '{other}' (expected one of {EVENT_NAMES})")),
    };
    let t: f64 = time.parse().map_err(|_| format!("event '{arg}': time '{time}' is not a number"))?;
    if !t.is_finite() || t < 0.0 {
        return Err(format!("event '{arg}': time must be finite and non-negative"));
    }
    Ok((t, kind))
}

fn command_for(sim: &Sim, e: Event) -> Command {
    match e {
        Event::Scram => Command::ManualScram,
        Event::TurbineTrip => Command::TripTurbine,
        Event::FwHeaters => Command::SetHeaters { in_service: false },
        Event::LoadReject => Command::OpenBreaker,
        Event::Msiv => Command::SetMsiv { line: 0, open: false },
        Event::FwPump => Command::SetFeedPump { index: 0, on: false },
        Event::PsetUp => Command::SetPressureSetpoint { mpa: sim.steam.p_setpoint / 1e6 + 0.1 },
        Event::RodsOut => {
            let idx: Vec<usize> = (0..57).filter(|&i| sim.core.rods.pos[i] < 48.0).take(4).collect();
            let tgt = idx.first().map(|&i| sim.core.rods.pos[i] + 2.0).unwrap_or(48.0);
            Command::RodSetTarget { indices: idx, target: tgt }
        }
    }
}

fn fail(msg: String) -> ! {
    eprintln!("trace: {msg}");
    eprintln!("usage: trace <cold|hot|full> <seconds> [event@time ...]");
    std::process::exit(2);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let scen = args.get(1).map(|s| parse_scenario(s)).unwrap_or(Ok(ScenarioName::FullPower)).unwrap_or_else(|e| fail(e));
    let secs = args.get(2).map(|s| parse_seconds(s)).unwrap_or(Ok(120.0)).unwrap_or_else(|e| fail(e));
    let mut events: Vec<(f64, Event)> = args[3.min(args.len())..]
        .iter()
        .map(|e| parse_event(e))
        .collect::<Result<_, _>>()
        .unwrap_or_else(|e| fail(e));
    events.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    let t0 = std::time::Instant::now();
    let mut sim = Sim::new(scen);
    eprintln!("init took {:?}", t0.elapsed());
    let every = (secs / 40.0).max(1.0);
    let mut next = 0.0;
    println!("{:>7} {:>7} {:>7} {:>6} {:>7} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6}",
        "t", "n%", "Qmw", "Pmpa", "lvl_cm", "Wcore", "void", "Wst", "Wfw", "Tfw", "MWe", "rho", "Tfuel", "ICmw");
    while sim.time < secs {
        while let Some(&(t, e)) = events.first() {
            if t > sim.time {
                break;
            }
            events.remove(0);
            let cmd = command_for(&sim, e);
            println!("--- {e:?} at {t}");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_events_are_rejected() {
        assert!(parse_event("scram@typo").is_err());
        assert!(parse_event("scrm@10").is_err());
        assert!(parse_event("scram").is_err());
        assert!(parse_event("scram@-1").is_err());
        assert!(parse_event("scram@NaN").is_err());
        assert_eq!(parse_event("pset+@12.5").unwrap(), (12.5, Event::PsetUp));
        assert!(parse_scenario("warm").is_err());
        assert!(parse_seconds("0").is_err());
    }
}
