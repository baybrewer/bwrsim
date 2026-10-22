//! Whole-plant behaviour tests. These check the simulator against how a BWR
//! is expected to behave, not exact numbers.

use std::collections::{HashMap, HashSet};

use bwr_core::api::{Command, ModeSwitch, ScenarioName};
use bwr_core::bop::LevelControlMode;
use bwr_core::params::*;
use bwr_core::sim::{irm_full_scale, DT};
use bwr_core::Sim;
use serde_json::Value;

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
            sim.apply(Command::SetSpeedSetpoint { rpm: RATED_RPM });
            stage = 1;
        }
        if stage == 1 && (sim.turbine.speed_rpm - RATED_RPM).abs() < 5.0 && sim.turbine.phase_deg.abs() < 5.0 {
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

// ------------------------------------------------------------------ contract
//
// The TypeScript contract (ui/src/sim/types.ts) is parsed here and every Rust
// snapshot / command is validated against it in both directions: no Rust key may
// be missing from TypeScript, no TypeScript field may be missing from Rust, and
// every leaf must have the declared type (a NaN serialises as null and fails a
// `number` field). Dimensional numeric fields must carry a unit suffix.

const TS_SOURCE: &str = include_str!("../../../ui/src/sim/types.ts");

#[derive(Debug, Default)]
struct TsContract {
    /// `export type X = "a" | "b"` string enums.
    enums: HashMap<String, Vec<String>>,
    /// `export interface X { field: type; }`.
    interfaces: HashMap<String, Vec<(String, String)>>,
    /// `Command` union: type tag -> (field, type) pairs (excluding the tag).
    commands: HashMap<String, Vec<(String, String)>>,
}

fn parse_ts(src: &str) -> TsContract {
    let mut c = TsContract::default();
    let mut lines = src.lines().peekable();
    while let Some(line) = lines.next() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("export type ") {
            let (name, body) = rest.split_once('=').expect("type alias");
            let name = name.trim().to_string();
            let body = body.trim();
            if body.starts_with('"') {
                let vals = body.trim_end_matches(';').split('|').map(|v| v.trim().trim_matches('"').to_string()).collect();
                c.enums.insert(name, vals);
            } else if name == "Command" {
                for l in lines.by_ref() {
                    let l = l.trim();
                    if let Some(v) = l.strip_prefix('|') {
                        let inner = v.trim().trim_end_matches(';').trim().trim_start_matches('{').trim_end_matches('}');
                        let mut tag = None;
                        let mut fields = Vec::new();
                        for part in inner.split(';').map(str::trim).filter(|p| !p.is_empty()) {
                            let (k, ty) = part.split_once(':').expect("field: type");
                            let (k, ty) = (k.trim(), ty.trim());
                            if k == "type" {
                                tag = Some(ty.trim_matches('"').to_string());
                            } else {
                                fields.push((k.to_string(), ty.to_string()));
                            }
                        }
                        c.commands.insert(tag.expect("command variant without type tag"), fields);
                        if l.ends_with(';') {
                            break;
                        }
                    } else if l.starts_with("export ") {
                        break;
                    }
                }
            } else {
                panic!("unhandled type alias {name}");
            }
        } else if let Some(rest) = t.strip_prefix("export interface ") {
            let name = rest.trim_end_matches('{').trim().to_string();
            let mut fields = Vec::new();
            for l in lines.by_ref() {
                let l = l.trim();
                if l == "}" {
                    break;
                }
                if l.is_empty() || l.starts_with("/*") || l.starts_with('*') || l.starts_with("//") {
                    continue;
                }
                let (k, ty) = l.split_once(':').unwrap_or_else(|| panic!("{name}: cannot parse '{l}'"));
                assert!(!k.ends_with('?'), "{name}.{k}: optional fields are not part of the serde contract");
                fields.push((k.trim().to_string(), ty.trim().trim_end_matches(';').trim().to_string()));
            }
            c.interfaces.insert(name, fields);
        }
    }
    c
}

/// Validate `v` against TypeScript type `ty`, recursing through interfaces.
fn check_value(c: &TsContract, path: &str, ty: &str, v: &Value) -> Result<(), String> {
    let ty = ty.trim();
    if let Some(inner) = ty.strip_suffix(" | null") {
        return if v.is_null() { Ok(()) } else { check_value(c, path, inner, v) };
    }
    if let Some(elem) = ty.strip_suffix("[]") {
        let arr = v.as_array().ok_or_else(|| format!("{path}: expected {ty}, got {v}"))?;
        for (i, e) in arr.iter().enumerate() {
            check_value(c, &format!("{path}[{i}]"), elem, e)?;
        }
        return Ok(());
    }
    if ty.starts_with('[') && ty.ends_with(']') {
        let parts: Vec<&str> = ty[1..ty.len() - 1].split(',').map(str::trim).collect();
        let arr = v.as_array().ok_or_else(|| format!("{path}: expected tuple {ty}, got {v}"))?;
        if arr.len() != parts.len() {
            return Err(format!("{path}: tuple {ty} has {} elements, got {}", parts.len(), arr.len()));
        }
        for (i, (p, e)) in parts.iter().zip(arr).enumerate() {
            check_value(c, &format!("{path}[{i}]"), p, e)?;
        }
        return Ok(());
    }
    match ty {
        "number" => {
            let n = v.as_f64().ok_or_else(|| format!("{path}: expected number, got {v}"))?;
            if !n.is_finite() {
                return Err(format!("{path}: non-finite number"));
            }
            Ok(())
        }
        "string" => v.as_str().map(|_| ()).ok_or_else(|| format!("{path}: expected string, got {v}")),
        "boolean" => v.as_bool().map(|_| ()).ok_or_else(|| format!("{path}: expected boolean, got {v}")),
        name => {
            if let Some(vals) = c.enums.get(name) {
                let s = v.as_str().ok_or_else(|| format!("{path}: expected {name} string, got {v}"))?;
                return if vals.iter().any(|x| x == s) { Ok(()) } else { Err(format!("{path}: '{s}' is not a {name}")) };
            }
            let fields = c.interfaces.get(name).ok_or_else(|| format!("{path}: unknown TypeScript type {name}"))?;
            let obj = v.as_object().ok_or_else(|| format!("{path}: expected {name} object, got {v}"))?;
            for (k, _) in fields {
                if !obj.contains_key(k) {
                    return Err(format!("{path}: TypeScript field {name}.{k} missing from Rust"));
                }
            }
            for k in obj.keys() {
                if !fields.iter().any(|(f, _)| f == k) {
                    return Err(format!("{path}: Rust key {k} missing from TypeScript {name}"));
                }
            }
            for (k, fty) in fields {
                check_value(c, &format!("{path}.{k}"), fty, &obj[k])?;
            }
            Ok(())
        }
    }
}

const UNIT_SUFFIXES: &[&str] = &["S", "Mw", "Mwe", "Kpa", "Mpa", "C", "Cm", "M", "Kgs", "Kg", "Rpm", "Hz", "Deg", "Pct", "Cps", "KjKg", "Cph"];
/// Numeric fields that are genuinely dimensionless (fractions, counts, indices, pcm).
const DIMENSIONLESS: &[&str] = &[
    "speed", "total", "rods", "doppler", "void", "moderator", "xenon", "kEff", "irmRange", "gridSize", "selected",
    "positions", "targets", "layout", "powerFrac", "xenonRel", "iodineRel", "coreAvgVoid", "chimneyVoid", "exitQuality",
    "msivPos", "tcvPos", "bypassPos", "bypassManualPos", "fcvPos", "fcvManualPos", "valvePos",
];

fn has_unit_suffix(key: &str) -> bool {
    UNIT_SUFFIXES.iter().any(|s| {
        key.strip_suffix(s).map_or(false, |stem| stem.chars().last().map_or(false, |ch| ch.is_ascii_lowercase()))
    })
}

/// Every numeric field in the snapshot interfaces must carry a unit suffix or be
/// declared dimensionless.
fn check_unit_naming(c: &TsContract) -> Result<(), String> {
    for (name, fields) in &c.interfaces {
        for (k, ty) in fields {
            let numeric = ty.starts_with("number") || ty.starts_with("[number");
            if numeric && !has_unit_suffix(k) && !DIMENSIONLESS.contains(&k.as_str()) {
                return Err(format!("{name}.{k}: numeric field without a unit suffix (or add it to DIMENSIONLESS)"));
            }
        }
    }
    Ok(())
}

fn check_snapshot(c: &TsContract, snap: &Value) -> Result<(), String> {
    check_value(c, "snapshot", "Snapshot", snap)
}

/// One sample of every Rust command variant. The `match` below has no wildcard,
/// so adding a variant without a sample here fails to compile.
fn all_commands() -> Vec<Command> {
    use Command::*;
    let v = vec![
        SetPaused { paused: true },
        SetSpeed { speed: 2.0 },
        LoadScenario { name: ScenarioName::FullPower },
        ManualScram,
        ResetScram,
        SetModeSwitch { mode: ModeSwitch::Run },
        SelectRod { index: Some(3) },
        RodSetTarget { indices: vec![1, 2], target: 12.0 },
        RodStop,
        SetIrmRange { range: 5 },
        SetHeadVent { open: true },
        SetMsiv { line: 0, open: false },
        SetPressureSetpoint { mpa: 7.0 },
        SetBypass { auto: false, pos: 0.5 },
        SetSpeedSetpoint { rpm: RATED_RPM },
        ResetTurbine,
        TripTurbine,
        CloseBreaker,
        OpenBreaker,
        SetLoadLimit { pct: 90.0 },
        SetCwPump { index: 1, on: true },
        SetVacuumPump { on: true },
        SetFeedPump { index: 1, on: true },
        SetCondensatePump { index: 1, on: true },
        SetLevelControl { mode: LevelControlMode::ThreeElement },
        SetFcvManual { pos: 0.3 },
        SetLevelSetpoint { cm: -20.0 },
        SetHeaters { in_service: false },
        SetRwcuReject { kgs: 5.0 },
        SetIcTrain { index: 0, open: true },
        AckAlarms,
    ];
    for c in &v {
        match c {
            SetPaused { .. } | SetSpeed { .. } | LoadScenario { .. } | ManualScram | ResetScram | SetModeSwitch { .. }
            | SelectRod { .. } | RodSetTarget { .. } | RodStop | SetIrmRange { .. } | SetHeadVent { .. }
            | SetMsiv { .. } | SetPressureSetpoint { .. } | SetBypass { .. } | SetSpeedSetpoint { .. } | ResetTurbine
            | TripTurbine | CloseBreaker | OpenBreaker | SetLoadLimit { .. } | SetCwPump { .. } | SetVacuumPump { .. }
            | SetFeedPump { .. } | SetCondensatePump { .. } | SetLevelControl { .. } | SetFcvManual { .. }
            | SetLevelSetpoint { .. } | SetHeaters { .. } | SetRwcuReject { .. } | SetIcTrain { .. } | AckAlarms => {}
        }
    }
    v
}

fn check_commands(c: &TsContract) -> Result<(), String> {
    let mut seen = HashSet::new();
    for cmd in all_commands() {
        let v = serde_json::to_value(&cmd).map_err(|e| e.to_string())?;
        let obj = v.as_object().ok_or("command is not an object")?;
        let tag = obj["type"].as_str().ok_or("command without type tag")?.to_string();
        let fields = c.commands.get(&tag).ok_or_else(|| format!("Rust command '{tag}' missing from TypeScript"))?;
        for (k, ty) in fields {
            let val = obj.get(k).ok_or_else(|| format!("{tag}: TypeScript field {k} missing from Rust"))?;
            check_value(c, &format!("{tag}.{k}"), ty, val)?;
        }
        for k in obj.keys().filter(|k| *k != "type") {
            if !fields.iter().any(|(f, _)| f == k) {
                return Err(format!("{tag}: Rust field {k} missing from TypeScript"));
            }
        }
        // The TypeScript shape must also deserialise back into Rust.
        serde_json::from_value::<Command>(v.clone()).map_err(|e| format!("{tag}: {e}"))?;
        seen.insert(tag);
    }
    for tag in c.commands.keys() {
        if !seen.contains(tag) {
            return Err(format!("TypeScript command '{tag}' has no Rust variant"));
        }
    }
    Ok(())
}

#[test]
fn json_contract_matches_ui() {
    let c = parse_ts(TS_SOURCE);
    assert!(c.interfaces.contains_key("Snapshot") && !c.commands.is_empty(), "parser found nothing in types.ts");
    check_unit_naming(&c).unwrap();
    check_commands(&c).unwrap();

    // Snapshots from every scenario, after enough operation to populate alarms,
    // events, a selected rod and a finite period.
    for name in [ScenarioName::ColdShutdown, ScenarioName::HotStandby, ScenarioName::FullPower] {
        let mut sim = Sim::new(name);
        check_snapshot(&c, &serde_json::to_value(sim.snapshot()).unwrap()).unwrap_or_else(|e| panic!("{name:?}: {e}"));
        sim.apply(Command::SelectRod { index: Some(1) });
        sim.apply(Command::ManualScram);
        run(&mut sim, 2.0);
        let v = serde_json::to_value(sim.snapshot()).unwrap();
        assert!(!v["alarms"].as_array().unwrap().is_empty());
        check_snapshot(&c, &v).unwrap_or_else(|e| panic!("{name:?} after scram: {e}"));
    }
}

#[test]
fn contract_check_fails_on_a_mirror_mutation() {
    let v = serde_json::to_value(Sim::new(ScenarioName::ColdShutdown).snapshot()).unwrap();
    // Renamed TypeScript field.
    let mutated = TS_SOURCE.replace("coolantTempC: number;", "coolantTemperatureC: number;");
    assert!(check_snapshot(&parse_ts(&mutated), &v).is_err());
    // Removed TypeScript field (the Rust key then has no home).
    let mutated = TS_SOURCE.replace("  heatLoadMw: number;\n", "");
    assert!(check_snapshot(&parse_ts(&mutated), &v).is_err());
    // Nullability changed in TypeScript while Rust emits null.
    let mutated = TS_SOURCE.replace("periodS: number | null;", "periodS: number;");
    assert!(check_snapshot(&parse_ts(&mutated), &v).is_err());
    // Rust emitting a NaN (serialised as null) into a declared number.
    let mut nan = v.clone();
    nan["vessel"]["pressureMpa"] = Value::Null;
    assert!(check_snapshot(&parse_ts(TS_SOURCE), &nan).is_err());
    // A dropped TypeScript command variant.
    let mutated = TS_SOURCE.replace("  | { type: \"rodStop\" }\n", "");
    assert!(check_commands(&parse_ts(&mutated)).is_err());
    // A numeric field without a unit suffix.
    let mutated = TS_SOURCE.replace("coolantTempC: number;", "coolantTemp: number;");
    assert!(check_unit_naming(&parse_ts(&mutated)).is_err());
}

// ------------------------------------------------------------------ energy / BOP regressions

#[test]
fn isolation_stops_extraction_heating() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    assert!(sim.fw.q_extraction_w > 100.0e6, "extraction heating at power {}", sim.fw.q_extraction_w);
    sim.apply(Command::SetMsiv { line: 0, open: false });
    sim.apply(Command::SetMsiv { line: 1, open: false });
    run(&mut sim, 30.0);
    assert!(sim.steam.w_tcv < 1.0, "turbine steam {}", sim.steam.w_tcv);
    let t_iso = sim.fw.t_feed;
    for _ in 0..(90.0 / DT) as usize {
        sim.step(DT);
        assert_eq!(sim.fw.q_extraction_w, 0.0, "extraction heat with no turbine steam");
        assert!(sim.fw.q_extraction_avail_w == 0.0);
    }
    assert!(sim.fw.t_feed < t_iso, "heater store must drain: {} -> {}", t_iso, sim.fw.t_feed);
}

#[test]
fn turbine_trip_fast_opens_bypass_within_300_ms() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    sim.apply(Command::TripTurbine);
    run(&mut sim, 0.3);
    assert!(sim.steam.bypass_pos > 0.95, "bypass {} after 0.3 s", sim.steam.bypass_pos);
    run(&mut sim, 0.7);
    assert!(sim.steam.w_bypass > 0.5 * STEAM_FLOW_RATED, "bypass flow {}", sim.steam.w_bypass);
}

#[test]
fn condenser_load_is_rejected_to_cooling_water_at_power() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 60.0);
    let c = &sim.cond;
    assert!(c.q_in_w > 400.0e6 && c.q_in_w < 700.0e6, "load {}", c.q_in_w);
    // Steady state: what comes in leaves either with the condensate or to the circulating water.
    let net = c.q_in_w - c.w_out_accepted * bwr_core::steam::h_liquid(c.hotwell_t());
    assert!((c.q_cw_w - net).abs() < 0.05 * net, "cw {} vs net load {}", c.q_cw_w, net);
    assert!(c.pressure() < 8.0e3, "condenser {}", c.pressure());
    assert_eq!(c.overflow_kg, 0.0);
    assert_eq!(c.withdrawal_deficit_kg, 0.0);
}

#[test]
fn single_feed_pump_carries_rated_flow() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    assert_eq!(sim.fw.pumps.len(), 2);
    sim.apply(Command::SetFeedPump { index: 1, on: false });
    run(&mut sim, 120.0);
    assert!(!sim.scrammed, "scrammed: {:?}", sim.scram_reason);
    assert!(sim.fw.flow > 0.9 * STEAM_FLOW_RATED, "one pump flow {}", sim.fw.flow);
    assert!(sim.fw.pump_flow[1] == 0.0 && sim.fw.pump_speed[1] == 0.0);
    assert!(sim.vessel.level_nr_cm().abs() < 30.0, "level {}", sim.vessel.level_nr_cm());
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

// ------------------------------------------------------------------ protection and robustness (2026-10-22 review fixes)

fn has_alarm(sim: &Sim, id: &str) -> bool {
    sim.snapshot().alarms.iter().any(|a| a.id == id)
}

/// Sol 3 / Astra 2: the L8 feed-pump trip acts whether or not the turbine is
/// already tripped (hot standby starts tripped), and a restart is refused while
/// L8 persists.
#[test]
fn l8_feed_pump_trip_is_independent_of_turbine_state() {
    for name in [ScenarioName::HotStandby, ScenarioName::FullPower] {
        let mut sim = Sim::new(name);
        assert_eq!(sim.turbine.tripped, name == ScenarioName::HotStandby);
        let t = sim.vessel.eos.t;
        sim.vessel = bwr_core::vessel::Vessel::new(t, Z_NWL + 1.5, sim.core.thermal_power(), sim.fw.flow, sim.fw.t_feed, false);
        assert!(sim.vessel.level_nr_cm() > L8_CM);
        assert!(sim.fw.pumps.iter().any(|&on| on));
        sim.step(DT);
        assert!(sim.fw.pumps.iter().all(|&on| !on), "{name:?}: feed pumps still running at L8");
        assert!(sim.turbine.tripped);
        assert!(has_alarm(&sim, "l8"));
        sim.apply(Command::SetFeedPump { index: 0, on: true });
        assert!(!sim.fw.pumps[0], "{name:?}: pump restart must be refused while L8 persists");
    }
}

/// Astra 7: outside RUN a short positive period blocks withdrawal at
/// PERIOD_ROD_BLOCK_S, halts a withdrawal already under way, and scrams at
/// PERIOD_SCRAM_S; the indication is ignored below PERIOD_SIGNAL_MIN_N.
#[test]
fn short_period_blocks_withdrawal_and_scrams_outside_run() {
    // Source-range flux is below the validity floor: the signal must be ignored.
    let mut sim = Sim::new(ScenarioName::HotStandby);
    assert_eq!(sim.mode, ModeSwitch::Startup);
    assert!(sim.core.kin.n < PERIOD_SIGNAL_MIN_N);
    sim.core.kin.inv_period = 1.0 / (0.5 * PERIOD_SCRAM_S);
    sim.step(DT);
    assert!(!sim.scrammed, "period signal must be invalid at source range: {:?}", sim.scram_reason);

    // Critical core (uniform rod position found by bisection) so the period meter
    // decays only through its own filter while we step.
    let mut sim = Sim::new(ScenarioName::HotStandby);
    sim.irm_range = 10;
    let (mut lo, mut hi) = (0.0, ROD_FULL_OUT);
    for _ in 0..60 {
        let mid = 0.5 * (lo + hi);
        for i in 0..N_RODS {
            sim.core.rods.pos[i] = mid;
            sim.core.rods.target[i] = mid;
        }
        sim.core.update_reactivity(sim.vessel.alpha_core, sim.vessel.eos.t);
        if sim.core.rho_total < 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    assert!(sim.core.rho_total.abs() < 1e-4, "no critical position: rho {}", sim.core.rho_total);
    sim.core.kin = bwr_core::kinetics::Kinetics::equilibrium(1.0e-3);
    let pos0 = sim.core.rods.pos[0];

    // Block band: shorter than the block setpoint, longer than the scram setpoint.
    sim.core.kin.inv_period = 1.0 / (0.5 * (PERIOD_ROD_BLOCK_S + PERIOD_SCRAM_S));
    sim.apply(Command::RodSetTarget { indices: vec![0], target: pos0 + 4.0 });
    assert_eq!(sim.core.rods.target[0], pos0, "withdrawal must be refused on a short period");
    assert!(has_alarm(&sim, "rod_block"));
    // A withdrawal already in progress is halted where it is.
    sim.core.rods.target[0] = pos0 + 4.0;
    sim.step(DT);
    assert!(!sim.scrammed, "{:?}", sim.scram_reason);
    assert!((sim.core.rods.target[0] - sim.core.rods.pos[0]).abs() < 1e-9, "moving withdrawal not halted");

    // Scram band.
    sim.core.kin.inv_period = 1.0 / (0.5 * PERIOD_SCRAM_S);
    sim.step(DT);
    assert!(sim.scrammed);
    assert_eq!(sim.scram_reason.as_deref(), Some("Short reactor period"));
}

/// Sol 12 / Sol 28: SHUTDOWN is a persistent scram input while selected, so the
/// scram it caused cannot be reset until another mode is chosen; the unlatched
/// cold-shutdown start is preserved; rod motion is not reported once all rods are in.
#[test]
fn shutdown_mode_scram_persists_until_another_mode_is_selected() {
    let mut sim = Sim::new(ScenarioName::ColdShutdown);
    assert!(!sim.scrammed, "cold shutdown starts unlatched");
    sim.apply(Command::SetModeSwitch { mode: ModeSwitch::Startup });
    sim.apply(Command::SetModeSwitch { mode: ModeSwitch::Shutdown });
    assert!(sim.scrammed);
    run(&mut sim, 10.05);
    sim.apply(Command::ResetScram);
    assert!(sim.scrammed, "reset must be refused while SHUTDOWN is selected");
    assert!(has_alarm(&sim, "scram_reset"));
    sim.apply(Command::SetModeSwitch { mode: ModeSwitch::Startup });
    sim.apply(Command::ResetScram);
    assert!(!sim.scrammed);

    let mut sim = Sim::new(ScenarioName::FullPower);
    sim.apply(Command::SetModeSwitch { mode: ModeSwitch::Shutdown });
    sim.step(DT);
    assert!(sim.scrammed && sim.snapshot().rods.scram_in_progress);
    run(&mut sim, 12.0);
    assert!(sim.core.rods.all_in());
    assert!(!sim.snapshot().rods.scram_in_progress, "no rod motion once every rod is in");
    sim.apply(Command::ResetScram);
    assert!(sim.scrammed);
}

/// Sol 20 / Astra 22: a command carrying a non-finite number changes nothing but
/// raises an event; non-finite or non-positive time steps are no-ops and never
/// poison the clock.
#[test]
fn non_finite_commands_and_steps_are_rejected_without_side_effects() {
    use Command::*;
    let mut sim = Sim::new(ScenarioName::ColdShutdown);
    let strip_alarms = |mut v: Value| {
        v["alarms"] = Value::Null;
        v
    };
    let before = strip_alarms(serde_json::to_value(sim.snapshot()).unwrap());
    let (nan, inf) = (f64::NAN, f64::INFINITY);
    for cmd in [
        SetSpeed { speed: nan },
        RodSetTarget { indices: vec![0], target: nan },
        SetPressureSetpoint { mpa: inf },
        SetBypass { auto: false, pos: nan },
        SetSpeedSetpoint { rpm: -inf },
        SetLoadLimit { pct: nan },
        SetFcvManual { pos: nan },
        SetLevelSetpoint { cm: nan },
        SetRwcuReject { kgs: nan },
    ] {
        sim.apply(cmd);
    }
    assert!(has_alarm(&sim, "cmd_rejected"));
    assert_eq!(before, strip_alarms(serde_json::to_value(sim.snapshot()).unwrap()));
    sim.advance(nan);
    sim.advance(-1.0);
    sim.step(0.0);
    sim.step(nan);
    assert_eq!(sim.time, 0.0);
    sim.advance(1.0);
    assert!((sim.time + sim.backlog_s() - 1.0).abs() < 1e-9, "time accounting poisoned: {} s + backlog {} s", sim.time, sim.backlog_s());
    assert!(sim.backlog_s() < DT);
    check_snapshot(&parse_ts(TS_SOURCE), &serde_json::to_value(sim.snapshot()).unwrap()).unwrap();
}

/// Sol 6 / Astra 10: a state the EOS cannot represent halts the simulator with
/// the model error surfaced; the step is rolled back and time does not advance.
#[test]
fn eos_error_halts_the_step_without_advancing_time() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 1.0);
    sim.vessel.energy *= 3.0; // far above the 349.5 degC EOS ceiling
    let before = sim.clone();
    sim.step(DT);
    assert!(sim.model_error.is_some() && sim.paused, "model error must halt the sim");
    assert_eq!(sim.time, before.time);
    assert_eq!(sim.core.kin.n, before.core.kin.n);
    assert_eq!(sim.vessel.mass, before.vessel.mass);
    assert_eq!(sim.fw.flow, before.fw.flow);
    let snap = sim.snapshot();
    assert!(snap.model_error.is_some() && has_alarm(&sim, "model") && has_alarm(&sim, "model_limit"));
    check_snapshot(&parse_ts(TS_SOURCE), &serde_json::to_value(&snap).unwrap()).unwrap();
    sim.advance(1.0);
    assert_eq!(sim.time, before.time, "advance must not run while halted");
}

/// Steps once and returns the (mass, energy) the connected systems report having
/// exchanged with the vessel over the step: BOP steam and feed ports, RWCU
/// reject, head vent and the IC trains' own port quantities, plus the ambient
/// loss law of `Vessel::step_with_ic`. The vessel's integrated state must agree
/// (the same balance drives examples/validation.rs).
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

/// Energy and mass closure of the vessel control volume through a turbine trip
/// (bypass, SRI, extraction loss). Bound: |energy residual| <= 1e-6 of the
/// integrated core heat and |mass residual| <= 1e-3 kg; the ports are the same
/// numbers the vessel integrates, so only roundoff is legitimate.
#[test]
fn energy_and_mass_close_across_a_turbine_trip() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 2.0);
    let (m0, e0) = (sim.vessel.mass, sim.vessel.energy);
    let (mut m_ports, mut e_ports, mut q_core_j) = (0.0, 0.0, 0.0);
    for i in 0..(60.0 / DT) as usize {
        if i == (5.0 / DT) as usize {
            sim.apply(Command::TripTurbine);
        }
        let (dm, de) = port_balance_step(&mut sim);
        m_ports += dm;
        e_ports += de;
        q_core_j += sim.q_core * DT;
    }
    assert!(sim.turbine.tripped && !sim.scrammed && sim.model_error.is_none());
    let e_res = (sim.vessel.energy - e0) - e_ports;
    let m_res = (sim.vessel.mass - m0) - m_ports;
    assert!(e_res.abs() <= 1e-6 * q_core_j, "energy residual {e_res:.3e} J over {q_core_j:.3e} J of core heat");
    assert!(m_res.abs() <= 1e-3, "mass residual {m_res:.3e} kg");
}
