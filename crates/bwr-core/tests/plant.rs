//! Whole-plant behaviour tests. These check the simulator against how a BWR
//! is expected to behave, not exact numbers.

use std::collections::{HashMap, HashSet};

use bwr_core::api::{AnnEventsSince, AnnKind, Command, ModeSwitch, ScenarioName, Severity, Snapshot};
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

/// Sol 15 / Astra 6: the coupled SRI response to a full-power turbine trip.
/// Vendor basis: GE 005N9751 Rev J section 3.2 quotes an approximate 0.8
/// percentage point/s power reduction. The band 0.8 +/- 0.3 point/s carries that
/// "approximate" plus the lumped-core model uncertainty; it is an acceptance
/// envelope, not a fit target.
const SRI_VENDOR_RATE_PCT_S: f64 = 0.8;
const SRI_RATE_BAND_PCT_S: f64 = 0.3;

#[test]
fn sri_power_reduction_rate_matches_vendor_basis() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    let start_pct = sim.core.kin.n * 100.0;
    sim.apply(Command::TripTurbine);
    run(&mut sim, 10.0);
    assert!(!sim.scrammed, "SRI must ride through without scram: {:?}", sim.snapshot().scram_reason);
    let rate = (start_pct - sim.core.kin.n * 100.0) / 10.0;
    assert!(
        (rate - SRI_VENDOR_RATE_PCT_S).abs() <= SRI_RATE_BAND_PCT_S,
        "first-10 s average power reduction {rate:.3} point/s, vendor basis {SRI_VENDOR_RATE_PCT_S} +/- {SRI_RATE_BAND_PCT_S}"
    );
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
//
// The reverse direction treats TypeScript as authoritative: at every field the
// value TypeScript permits (each enum member, `null` where `| null`, a sample
// number/string/boolean) is substituted and pushed through serde into the Rust
// type; it must be accepted and re-emitted unchanged, and `null` must be
// rejected where TypeScript forbids it. Every string-literal union is compared
// exhaustively with its Rust enum in both directions.

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

/// Members of an inline string-literal union type (`"a" | "b"`), if `ty` is one.
fn inline_union(ty: &str) -> Option<Vec<String>> {
    ty.starts_with('"').then(|| ty.split('|').map(|v| v.trim().trim_matches('"').to_string()).collect())
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
        union if inline_union(union).is_some() => {
            let s = v.as_str().ok_or_else(|| format!("{path}: expected {union}, got {v}"))?;
            if inline_union(union).unwrap().iter().any(|x| x == s) { Ok(()) } else { Err(format!("{path}: '{s}' is not in {union}")) }
        }
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

const UNIT_SUFFIXES: &[&str] = &["S", "Ms", "Mw", "Mwe", "Kpa", "Mpa", "C", "Cm", "M", "Kgs", "Kg", "Rpm", "Hz", "Deg", "Pct", "Cps", "KjKg", "Cph"];
/// Numeric fields that are genuinely dimensionless (fractions, counts, indices, pcm).
const DIMENSIONLESS: &[&str] = &[
    "speed", "effectiveSpeed", "total", "rods", "doppler", "void", "moderator", "xenon", "kEff", "irmRange", "gridSize", "selected",
    "positions", "targets", "layout", "powerFrac", "xenonRel", "iodineRel", "coreAvgVoid", "chimneyVoid", "exitQuality",
    "msivPos", "tcvPos", "bypassPos", "bypassManualPos", "fcvPos", "fcvManualPos", "valvePos",
    // Identities, counters and limits.
    "runId", "rev", "annHead", "seq", "max",
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
        let deser = |m: &Value| -> Result<Value, String> {
            let cmd: Command = serde_json::from_value(m.clone()).map_err(|e| e.to_string())?;
            serde_json::to_value(cmd).map_err(|e| e.to_string())
        };
        for (k, ty) in fields {
            probe_ts(c, &v, &format!("/{k}"), &format!("{tag}.{k}"), ty, &deser)?;
        }
        seen.insert(tag);
    }
    for tag in c.commands.keys() {
        if !seen.contains(tag) {
            return Err(format!("TypeScript command '{tag}' has no Rust variant"));
        }
    }
    Ok(())
}

type Deser<'a> = &'a dyn Fn(&Value) -> Result<Value, String>;

/// Substitute `probe` at JSON pointer `ptr`, push the document through the Rust
/// type and return what Rust re-emits at that location.
fn through_rust(root: &Value, ptr: &str, probe: Value, deser: Deser) -> Result<Value, String> {
    let mut m = root.clone();
    *m.pointer_mut(ptr).unwrap_or_else(|| panic!("{ptr}: walker left the fixture")) = probe;
    let back = deser(&m)?;
    back.pointer(ptr).cloned().ok_or_else(|| format!("{ptr}: Rust did not re-emit the field"))
}

/// JSON equality with numbers compared by value (Rust re-emits 2 as 2.0 for f64).
fn same_json(a: &Value, b: &Value) -> bool {
    match (a.as_f64(), b.as_f64()) {
        (Some(x), Some(y)) => x == y,
        _ => a == b,
    }
}

fn expect_accepts(root: &Value, ptr: &str, label: &str, probe: Value, deser: Deser) -> Result<(), String> {
    let back = through_rust(root, ptr, probe.clone(), deser)
        .map_err(|e| format!("{label}: TypeScript allows {probe} but Rust rejects it ({e})"))?;
    if same_json(&back, &probe) {
        Ok(())
    } else {
        Err(format!("{label}: Rust re-emits {back} for TypeScript value {probe}"))
    }
}

fn expect_rejects(root: &Value, ptr: &str, label: &str, probe: Value, deser: Deser) -> Result<(), String> {
    match through_rust(root, ptr, probe.clone(), deser) {
        Ok(_) => Err(format!("{label}: TypeScript forbids {probe} but Rust accepts it")),
        Err(_) => Ok(()),
    }
}

/// Walk `root` (a Rust-emitted fixture) by TypeScript type `ty`, checking at every
/// location that Rust accepts exactly the values TypeScript allows there.
fn probe_ts(c: &TsContract, root: &Value, ptr: &str, label: &str, ty: &str, deser: Deser) -> Result<(), String> {
    let ty = ty.trim();
    if let Some(inner) = ty.strip_suffix(" | null") {
        expect_accepts(root, ptr, label, Value::Null, deser)?;
        return probe_non_null(c, root, ptr, label, inner, deser);
    }
    expect_rejects(root, ptr, label, Value::Null, deser)?;
    probe_non_null(c, root, ptr, label, ty, deser)
}

fn probe_non_null(c: &TsContract, root: &Value, ptr: &str, label: &str, ty: &str, deser: Deser) -> Result<(), String> {
    let here = root.pointer(ptr).expect("walker stays inside the fixture");
    if let Some(elem) = ty.strip_suffix("[]") {
        // Element types are probed on the first element; an empty array has nothing to substitute into.
        return match here.as_array() {
            Some(a) if !a.is_empty() => probe_ts(c, root, &format!("{ptr}/0"), &format!("{label}[0]"), elem, deser),
            _ => Ok(()),
        };
    }
    if ty.starts_with('[') && ty.ends_with(']') {
        for (i, p) in ty[1..ty.len() - 1].split(',').map(str::trim).enumerate() {
            probe_ts(c, root, &format!("{ptr}/{i}"), &format!("{label}[{i}]"), p, deser)?;
        }
        return Ok(());
    }
    match ty {
        "number" => expect_accepts(root, ptr, label, Value::from(2), deser),
        "string" => expect_accepts(root, ptr, label, Value::from("contract probe"), deser),
        "boolean" => {
            expect_accepts(root, ptr, label, Value::from(true), deser)?;
            expect_accepts(root, ptr, label, Value::from(false), deser)
        }
        name => {
            if let Some(vals) = c.enums.get(name).cloned().or_else(|| inline_union(name)) {
                for m in vals {
                    expect_accepts(root, ptr, label, Value::from(m.as_str()), deser)?;
                }
                return expect_rejects(root, ptr, label, Value::from("notATypeScriptMember"), deser);
            }
            let fields = c.interfaces.get(name).ok_or_else(|| format!("{label}: unknown TypeScript type {name}"))?;
            if !here.is_object() {
                // A nullable interface the fixture left null cannot be walked.
                return Err(format!("{label}: fixture has no {name} object to probe"));
            }
            for (k, fty) in fields {
                probe_ts(c, root, &format!("{ptr}/{k}"), &format!("{label}.{k}"), fty, deser)?;
            }
            Ok(())
        }
    }
}

fn check_snapshot_ts_authoritative(c: &TsContract, snap: &Value) -> Result<(), String> {
    let deser = |m: &Value| -> Result<Value, String> {
        let s: Snapshot = serde_json::from_value(m.clone()).map_err(|e| e.to_string())?;
        serde_json::to_value(s).map_err(|e| e.to_string())
    };
    probe_ts(c, snap, "", "snapshot", "Snapshot", &deser)
}

/// Every variant of each wire enum. The `match` has no wildcard, so a new Rust
/// variant fails to compile until it is listed here.
fn all_mode_switches() -> Vec<ModeSwitch> {
    use ModeSwitch::*;
    let v = vec![Shutdown, Refuel, Startup, Run];
    v.iter().for_each(|m| match m {
        Shutdown | Refuel | Startup | Run => {}
    });
    v
}
fn all_level_control_modes() -> Vec<LevelControlMode> {
    use LevelControlMode::*;
    let v = vec![Manual, SingleElement, ThreeElement];
    v.iter().for_each(|m| match m {
        Manual | SingleElement | ThreeElement => {}
    });
    v
}
fn all_severities() -> Vec<Severity> {
    use Severity::*;
    let v = vec![Info, Warning, Alarm, Trip];
    v.iter().for_each(|m| match m {
        Info | Warning | Alarm | Trip => {}
    });
    v
}
fn all_ann_kinds() -> Vec<AnnKind> {
    use AnnKind::*;
    let v = vec![Onset, Clear, Changed];
    v.iter().for_each(|m| match m {
        Onset | Clear | Changed => {}
    });
    v
}
fn all_scenarios() -> Vec<ScenarioName> {
    use ScenarioName::*;
    let v = vec![ColdShutdown, HotStandby, FullPower];
    v.iter().for_each(|m| match m {
        ColdShutdown | HotStandby | FullPower => {}
    });
    v
}

/// The Rust enum and the TypeScript union must have identical wire members.
fn check_enum<E: serde::Serialize + serde::de::DeserializeOwned>(c: &TsContract, ts_name: &str, all: &[E]) -> Result<(), String> {
    let ts = c.enums.get(ts_name).ok_or_else(|| format!("TypeScript enum {ts_name} missing"))?;
    check_enum_members(ts_name, ts, all)
}

fn check_enum_members<E: serde::Serialize + serde::de::DeserializeOwned>(ts_name: &str, ts: &[String], all: &[E]) -> Result<(), String> {
    let rust: Vec<String> = all
        .iter()
        .map(|e| serde_json::to_value(e).unwrap().as_str().expect("unit enum serialises as a string").to_string())
        .collect();
    if let Some(r) = rust.iter().find(|r| !ts.contains(r)) {
        return Err(format!("Rust {ts_name} variant '{r}' missing from TypeScript"));
    }
    for t in ts {
        let back: E = serde_json::from_value(Value::from(t.as_str()))
            .map_err(|_| format!("TypeScript {ts_name} member '{t}' has no Rust variant"))?;
        if serde_json::to_value(back).unwrap() != Value::from(t.as_str()) {
            return Err(format!("TypeScript {ts_name} member '{t}' does not round-trip"));
        }
    }
    Ok(())
}

fn check_enums(c: &TsContract) -> Result<(), String> {
    check_enum(c, "ModeSwitch", &all_mode_switches())?;
    check_enum(c, "LevelControlMode", &all_level_control_modes())?;
    check_enum(c, "Severity", &all_severities())?;
    check_enum(c, "ScenarioName", &all_scenarios())?;
    let kind_ty = c.interfaces.get("AnnEvent").and_then(|f| f.iter().find(|(k, _)| k == "kind")).map(|(_, t)| t.as_str());
    let kinds = kind_ty.and_then(inline_union).ok_or("TypeScript AnnEvent.kind is not a string union")?;
    check_enum_members("AnnEvent.kind", &kinds, &all_ann_kinds())?;
    let mapped: HashSet<&str> = ["ModeSwitch", "LevelControlMode", "Severity", "ScenarioName"].into();
    match c.enums.keys().find(|k| !mapped.contains(k.as_str())) {
        Some(k) => Err(format!("TypeScript enum {k} has no Rust enum mapped in check_enums")),
        None => Ok(()),
    }
}

/// Fixture snapshots: every scenario fresh and after a scram with a selected rod
/// (populates alarms, the period and `selected`).
fn contract_snapshots() -> Vec<(String, Value)> {
    let mut out = Vec::new();
    for name in all_scenarios() {
        let mut sim = Sim::new(name);
        out.push((format!("{name:?}"), serde_json::to_value(sim.snapshot()).unwrap()));
        sim.apply(Command::SelectRod { index: Some(1) });
        sim.apply(Command::ManualScram);
        run(&mut sim, 2.0);
        let v = serde_json::to_value(sim.snapshot()).unwrap();
        assert!(!v["alarms"].as_array().unwrap().is_empty());
        out.push((format!("{name:?} after scram"), v));
    }
    out
}

#[test]
fn json_contract_matches_ui() {
    let c = parse_ts(TS_SOURCE);
    assert!(c.interfaces.contains_key("Snapshot") && !c.commands.is_empty(), "parser found nothing in types.ts");
    check_unit_naming(&c).unwrap();
    check_enums(&c).unwrap();
    check_commands(&c).unwrap();
    for (label, v) in contract_snapshots() {
        check_snapshot(&c, &v).unwrap_or_else(|e| panic!("{label}: {e}"));
        check_snapshot_ts_authoritative(&c, &v).unwrap_or_else(|e| panic!("{label}: {e}"));
    }
    // The event pull result (not part of a snapshot); cold shutdown starts with an alarm.
    let sim = Sim::new(ScenarioName::ColdShutdown);
    let pulled = serde_json::to_value(sim.ann_events_since(sim.run_id, 0).unwrap()).unwrap();
    assert!(!pulled["events"].as_array().unwrap().is_empty());
    check_value(&c, "pull", "AnnEventsSince", &pulled).unwrap();
    let deser = |m: &Value| -> Result<Value, String> {
        let s: AnnEventsSince = serde_json::from_value(m.clone()).map_err(|e| e.to_string())?;
        serde_json::to_value(s).map_err(|e| e.to_string())
    };
    probe_ts(&c, &pulled, "", "pull", "AnnEventsSince", &deser).unwrap();
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

/// Sol 24 / Astra 26 / Sol N8: the mutations the 2026-10-22 re-reviews showed
/// passing the self-roundtrip gate must each fail the TypeScript-authoritative one.
#[test]
fn contract_check_fails_on_payload_enum_and_nullability_drift() {
    let mutate = |from: &str, to: &str| {
        assert!(TS_SOURCE.contains(from), "mutation anchor '{from}' not in types.ts");
        parse_ts(&TS_SOURCE.replacen(from, to, 1))
    };
    let snaps = contract_snapshots();
    let snap_err = |c: &TsContract| {
        snaps.iter().find_map(|(_, v)| check_snapshot(c, v).and_then(|_| check_snapshot_ts_authoritative(c, v)).err())
    };
    let full_err = |c: &TsContract| check_enums(c).and_then(|_| check_commands(c)).err().or_else(|| snap_err(c));
    let cases: [(&str, &str, &str); 7] = [
        ("selectRod index loses nullability", "type: \"selectRod\"; index: number | null", "type: \"selectRod\"; index: number"),
        ("setSpeed gains nullability", "type: \"setSpeed\"; speed: number }", "type: \"setSpeed\"; speed: number | null }"),
        ("setSpeed payload renamed", "type: \"setSpeed\"; speed: number }", "type: \"setSpeed\"; rate: number }"),
        ("ModeSwitch member renamed", "\"shutdown\" | \"refuel\" |", "\"shutdown\" | \"refuellingTypo\" |"),
        ("ModeSwitch member removed", "\"shutdown\" | \"refuel\" |", "\"shutdown\" |"),
        ("ModeSwitch member added", "| \"run\";", "| \"run\" | \"bogus\";"),
        ("kineticsFault string -> number", "kineticsFault: string | null;", "kineticsFault: number | null;"),
    ];
    for (what, from, to) in cases {
        let err = full_err(&mutate(from, to)).unwrap_or_else(|| panic!("contract gate passed a mutation: {what}"));
        eprintln!("{what}: {err}");
    }
    // Snapshot-side enum drift is caught on the snapshot walk alone.
    let err = snap_err(&mutate("\"manual\" | \"singleElement\"", "\"manualTypo\" | \"singleElement\""));
    eprintln!("LevelControlMode member renamed (snapshot walk): {}", err.expect("snapshot walk passed an enum rename"));
    assert_eq!(full_err(&parse_ts(TS_SOURCE)), None);
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
        // The rejection events themselves (and the revision count) are expected changes.
        for k in ["annHead", "annTail", "rev"] {
            v[k] = Value::Null;
        }
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
    // IC steam leaves at the start-of-step port enthalpy the trains requested with.
    let h_ic_steam = sim.vessel.steam_port_enthalpy();
    let heat_loss = HEAT_LOSS_HOT * ((t0 - 20.0) / 268.0).max(0.0);
    sim.step(DT);
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

/// Sol R4 / Astra R5: near-minimum liquid inventory limits the accepted RWCU
/// flow below the 15 kg/s request. The balance must close with the vessel's
/// accepted `flows.w_liquid` (a request-based helper invented ~0.74 kg here).
#[test]
fn vessel_balance_closes_with_phase_limited_rwcu() {
    let mut sim = Sim::new(ScenarioName::ColdShutdown);
    let t = 150.0;
    let m = 1.00101 * V_VESSEL_FLUID * bwr_core::steam::sat_t(t).rho_g;
    sim.vessel.head_vent_open = false;
    sim.vessel.gas_mass = 0.0;
    sim.vessel.mass = m;
    sim.vessel.energy = bwr_core::vessel::energy_for(m, t).unwrap();
    sim.vessel.eos = bwr_core::vessel::solve_eos(m, sim.vessel.energy).expect("eos");
    sim.apply(Command::SetRwcuReject { kgs: 15.0 });
    let (m0, e0) = (sim.vessel.mass, sim.vessel.energy);
    let (dm, de) = port_balance_step(&mut sim);
    assert!(sim.model_error.is_none(), "{:?}", sim.model_error);
    let accepted = sim.vessel.flows.w_liquid;
    assert!(accepted > 0.0 && accepted < 1.0, "accepted RWCU {accepted} kg/s should be phase-limited");
    let m_res = (sim.vessel.mass - m0) - dm;
    let e_res = (sim.vessel.energy - e0) - de;
    assert!(m_res.abs() <= 1e-6, "mass residual {m_res:.3e} kg");
    assert!(e_res.abs() <= 1.0, "energy residual {e_res:.3e} J");
}

// ------------------------------------------------------------------ round 2 integration

/// Sol 14 / Astra 14: generator load rejection (breaker opens, no turbine trip)
/// must fast-open the bypass within the published 300 ms (GE 005N9751 Rev J 5.1.4).
#[test]
fn load_rejection_fast_opens_bypass_within_300_ms() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    sim.apply(Command::OpenBreaker);
    run(&mut sim, 0.3);
    assert!(!sim.turbine.tripped, "load rejection alone, no turbine trip");
    assert!(sim.steam.bypass_pos >= 0.9, "bypass {} at 0.3 s after load rejection", sim.steam.bypass_pos);
}

/// Sol N4 / Astra N2: a large public step that exhausts the hotwell must not
/// create water: vessel + hotwell/shell + IC inventory changes only by the
/// explicit CST makeup/reject/overflow, relief and RWCU/vent boundary flows.
#[test]
fn hotwell_exhaustion_does_not_create_vessel_water() {
    let mut sim = Sim::new(ScenarioName::HotStandby);
    sim.steam.msiv_cmd = [false; N_STEAM_LINES];
    sim.steam.msiv_pos = [0.0; N_STEAM_LINES];
    sim.fw = bwr_core::bop::Feedwater::new(2, 150.0, LevelControlMode::Manual, 1.0);
    sim.cond.hotwell_mass = 6000.1;
    sim.cond.set_temperature(35.0);
    let inv = |s: &Sim| s.vessel.mass + s.cond.inventory_kg();
    let (m0, mk0, rj0, ov0, rl0) = (inv(&sim), sim.cond.makeup_kg, sim.cond.reject_kg, sim.cond.overflow_kg, sim.cond.relief_kg);
    sim.step(20.0);
    assert!(sim.model_error.is_none(), "{:?}", sim.model_error);
    assert_eq!(sim.cond.w_out_shortfall, 0.0, "feed is bounded before the vessel, so nothing is short");
    let f = sim.vessel.flows;
    let explicit = (sim.cond.makeup_kg - mk0) - (sim.cond.reject_kg - rj0) - (sim.cond.overflow_kg - ov0)
        - (sim.cond.relief_kg - rl0)
        - 20.0 * (f.w_liquid + f.w_vent);
    let ic_in: f64 = sim.ic.iter().map(|t| t.last.w_steam_in - t.last.w_cond_return).sum::<f64>() * 20.0;
    let created = inv(&sim) - m0 - explicit + ic_in;
    assert!(created.abs() < 1.0, "created {created} kg");
}

/// Sol N2: when the vessel cannot deliver the requested turbine steam, the
/// heaters may only take extraction heat from what was delivered.
#[test]
fn extraction_is_funded_by_delivered_turbine_steam() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 1.0);
    let t = sim.vessel.eos.t;
    let m = V_VESSEL_FLUID * bwr_core::steam::sat_t(t).rho_f * (1.0 - 1e-8);
    sim.vessel.mass = m;
    sim.vessel.energy = bwr_core::vessel::energy_for(m, t).unwrap();
    sim.vessel.eos = bwr_core::vessel::solve_eos(m, sim.vessel.energy).expect("eos");
    sim.step(DT);
    let delivered = sim.vessel.flows.w_steam;
    assert!(
        sim.fw.q_extraction_w <= delivered * FW_EXTRACTION_HEAT_PER_KG + 1e-6,
        "extraction {} W from {} kg/s delivered",
        sim.fw.q_extraction_w,
        delivered
    );
    assert!(sim.cond.q_in_w > -1.0e3, "condenser inlet energy {}", sim.cond.q_in_w);
}

/// Sol N1 / Astra N1: a water-solid vessel cannot supply IC steam, so the
/// trains commit nothing: vessel + IC tube inventory is unchanged and no heat
/// reaches the pools (old code committed the train before the vessel refused).
#[test]
fn water_solid_vessel_ic_transfers_conserve_mass_at_plant_level() {
    let mut sim = Sim::new(ScenarioName::HotStandby);
    let m = V_VESSEL_FLUID * bwr_core::steam::sat_t(200.0).rho_f * 1.005;
    sim.vessel.mass = m;
    sim.vessel.energy = bwr_core::vessel::energy_for(m, 200.0).unwrap();
    sim.vessel.eos = bwr_core::vessel::solve_eos(m, sim.vessel.energy).unwrap();
    sim.vessel.gas_mass = 0.0;
    sim.steam.msiv_cmd = [false; N_STEAM_LINES];
    sim.steam.msiv_pos = [0.0; N_STEAM_LINES];
    sim.fw = bwr_core::bop::Feedwater::new(0, 150.0, LevelControlMode::Manual, 0.0);
    for t in sim.ic.iter_mut() {
        t.cmd_open = true;
        t.valve_pos = 1.0;
        t.heat = IC_TRAIN_RATED_W;
        t.tube_mass = 500.0;
    }
    let inv = |s: &Sim| s.vessel.mass + s.ic.iter().map(|t| t.tube_mass).sum::<f64>();
    let m0 = inv(&sim);
    sim.step(0.05);
    assert!(sim.model_error.is_none(), "{:?}", sim.model_error);
    assert!((inv(&sim) - m0).abs() < 1e-6, "vessel + IC tubes changed by {} kg", inv(&sim) - m0);
    assert!(sim.ic.iter().all(|t| t.last.q_pool == 0.0), "pool heat with no steam supplied");
}

/// Sol N1 / Astra N1, vapour-limited case: a two-phase vessel holding almost
/// no vapour can supply only part of the IC steam the trains request. The
/// trains commit exactly the accepted fraction, so vessel + tube inventory is
/// conserved and pool duty never exceeds what the accepted steam can carry.
#[test]
fn vapour_limited_ic_draw_is_conserved_at_plant_level() {
    let mut sim = Sim::new(ScenarioName::HotStandby);
    let t = sim.vessel.eos.t;
    let m = V_VESSEL_FLUID * bwr_core::steam::sat_t(t).rho_f * (1.0 - 1e-6);
    sim.vessel.mass = m;
    sim.vessel.energy = bwr_core::vessel::energy_for(m, t).unwrap();
    sim.vessel.eos = bwr_core::vessel::solve_eos(m, sim.vessel.energy).unwrap();
    sim.steam.msiv_cmd = [false; N_STEAM_LINES];
    sim.steam.msiv_pos = [0.0; N_STEAM_LINES];
    sim.fw = bwr_core::bop::Feedwater::new(0, 150.0, LevelControlMode::Manual, 0.0);
    for tr in sim.ic.iter_mut() {
        tr.cmd_open = true;
        tr.valve_pos = 1.0;
        tr.heat = IC_TRAIN_RATED_W;
    }
    let inv = |s: &Sim| s.vessel.mass + s.ic.iter().map(|t| t.tube_mass).sum::<f64>();
    let m0 = inv(&sim);
    sim.step(DT);
    assert!(sim.model_error.is_none(), "{:?}", sim.model_error);
    let ratio = sim.vessel.flows.ic_ratio;
    assert!(ratio < 1.0, "case must be vapour-limited, ic_ratio {ratio}");
    let drawn: f64 = sim.ic.iter().map(|t| t.last.w_steam_in).sum();
    assert!((drawn - sim.vessel.flows.w_ic_steam).abs() <= 1e-9 * drawn.max(1.0), "trains {drawn} vs vessel {}", sim.vessel.flows.w_ic_steam);
    assert!((inv(&sim) - m0).abs() < 1e-6, "vessel + IC tubes changed by {} kg", inv(&sim) - m0);
}

/// The physical and controller state a rejected step must leave untouched.
fn transactional_state(sim: &Sim) -> String {
    let v = &sim.vessel;
    format!(
        "{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{}|{}|{}|{}|{}|{}",
        sim.core, sim.steam, sim.turbine, sim.cond, sim.fw, sim.ic, v.eos, v.mass, v.energy, v.gas_mass, sim.time, sim.q_core, sim.scrammed
    )
}

/// Sol N3 / Astra N3: a rejected step restores every subsystem and controller
/// (here an accelerating, disconnected turbine whose governor would ramp) and
/// repeated retries change nothing.
#[test]
fn rejected_step_restores_turbine_governor_and_all_state() {
    let mut sim = Sim::new(ScenarioName::HotStandby);
    sim.apply(Command::ResetTurbine);
    sim.apply(Command::SetSpeedSetpoint { rpm: 3600.0 });
    sim.vessel.energy *= 3.0;
    let before = transactional_state(&sim);
    for _ in 0..3 {
        assert!(!sim.step(DT), "step must report rejection");
        assert!(sim.model_error.is_some());
        assert_eq!(transactional_state(&sim), before, "rejected step mutated plant/controller state");
    }
}

/// Astra N3: a rejected step inside `advance` keeps its DT in the backlog.
#[test]
fn rejected_step_does_not_consume_clock_backlog() {
    let mut sim = Sim::new(ScenarioName::HotStandby);
    sim.vessel.energy *= 3.0;
    sim.advance(0.1);
    assert_eq!(sim.time, 0.0);
    assert!(sim.paused && sim.model_error.is_some());
    assert!((sim.backlog_s() - 0.1).abs() < 1e-12, "backlog {} s, expected 0.1 s retained", sim.backlog_s());
}

// ------------------------------------------------------------------ round 3 integration

/// Sol R6: in a vapour-limited step the published turbine and bypass flows are
/// the steam the vessel delivered, not the hydraulic request.
#[test]
fn published_steam_flow_is_the_accepted_vessel_delivery() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 1.0);
    let t = sim.vessel.eos.t;
    let m = V_VESSEL_FLUID * bwr_core::steam::sat_t(t).rho_f * (1.0 - 1e-8);
    sim.vessel.mass = m;
    sim.vessel.energy = bwr_core::vessel::energy_for(m, t).unwrap();
    sim.vessel.eos = bwr_core::vessel::solve_eos(m, sim.vessel.energy).expect("eos");
    assert!(sim.step(DT), "{:?}", sim.model_error);
    let delivered = sim.vessel.flows.w_steam;
    assert!(delivered < 1.0, "case must be vapour-limited, delivered {delivered} kg/s");
    let s = sim.snapshot().steam;
    let published = s.steam_to_turbine_kgs + s.steam_to_bypass_kgs;
    assert!((published - delivered).abs() <= 1e-9 * delivered.max(1.0), "published {published} vs delivered {delivered} kg/s");
}

/// Sol 21: the engine reports the nominal rate (0 while paused) and its own clock backlog.
#[test]
fn snapshot_reports_nominal_speed_and_engine_backlog() {
    let mut sim = Sim::new(ScenarioName::HotStandby);
    sim.apply(Command::SetSpeed { speed: 2.0 });
    sim.advance(0.06);
    let s = sim.snapshot();
    assert!(sim.backlog_s() > 0.0);
    assert_eq!(s.clock_backlog_s, sim.backlog_s());
    assert_eq!(s.effective_speed, s.speed);
    sim.apply(Command::SetPaused { paused: true });
    assert_eq!(sim.snapshot().effective_speed, 0.0);
}

/// R3 condenser: a shell that cannot represent the step's end state rejects
/// it, and the whole plant step rolls back (vessel and IC trains included,
/// since they commit before the condenser runs).
#[test]
fn condenser_rejection_rolls_back_the_plant_step() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 1.0);
    sim.cond.cw_pumps = [false; N_CW_PUMPS];
    sim.cond.hotwell_mass = 0.0;
    sim.cond.set_temperature(bwr_core::steam::sat_p(CONDENSER_RELIEF_P).t);
    // Plant steam always brings its own liquid, so (like the vessel EOS fixtures)
    // the unrepresentable state is injected: relief must vent ~440 kg from a
    // shell holding only this step's returns. No supported sequence reaches it:
    // loss of CW blocks the bypass and trips the turbine on low vacuum well
    // below relief pressure, and makeup refills the hotwell (asserted by
    // `loss_of_cw_with_low_hotwell_is_stopped_by_bypass_block_and_vacuum_trip`).
    sim.cond.energy_j += 1.0e9;
    for t in sim.ic.iter_mut() {
        t.cmd_open = true;
    }
    let before = transactional_state(&sim);
    assert!(!sim.step(DT), "condenser dryout must reject the step");
    assert!(sim.model_error.as_deref().is_some_and(|e| e.contains("condenser")) && sim.paused, "{:?}", sim.model_error);
    assert!(has_alarm(&sim, "model_limit"));
    assert_eq!(transactional_state(&sim), before, "rejected step mutated plant state");
}

/// Sol R5: the reachable counterpart of the injected-fault test above. From an
/// equilibrium-consistent full-power state with a nearly empty hotwell, loss
/// of all CW (public commands) cannot drive the shell to relief dryout:
/// no CW blocks the bypass, low vacuum trips the turbine, and makeup refills
/// the hotwell. Every step is accepted, the vessel port balance closes on each,
/// and the shell stays on its equilibrium.
#[test]
fn loss_of_cw_with_low_hotwell_is_stopped_by_bypass_block_and_vacuum_trip() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 1.0);
    sim.cond.hotwell_mass = 0.02 * HOTWELL_CAPACITY;
    let t_shell = sim.cond.t_sat;
    sim.cond.set_temperature(t_shell);
    for index in 0..N_CW_PUMPS {
        sim.apply(Command::SetCwPump { index, on: false });
    }
    let (mut peak_p, mut q_core_j) = (0.0f64, 0.0);
    for i in 0..(300.0 / DT) as usize {
        let (m0, e0) = (sim.vessel.mass, sim.vessel.energy);
        let (dm, de) = port_balance_step(&mut sim);
        assert!(sim.model_error.is_none(), "step {i} rejected: {:?}", sim.model_error);
        q_core_j += sim.q_core * DT;
        assert!(((sim.vessel.mass - m0) - dm).abs() <= 1e-6, "step {i}: vessel mass residual");
        assert!(((sim.vessel.energy - e0) - de).abs() <= 1e-6 * q_core_j.max(1.0), "step {i}: vessel energy residual");
        assert!(sim.cond.equilibrium_residual_j().abs() <= 1.0, "step {i}: shell off equilibrium");
        peak_p = peak_p.max(sim.cond.pressure());
    }
    assert!(sim.turbine.tripped, "low vacuum must trip the turbine");
    assert!(!sim.cond.available_for_bypass() && sim.steam.bypass_pos == 0.0, "bypass must be blocked");
    assert!(peak_p < CONDENSER_RELIEF_P, "shell reached relief: peak {peak_p:.0} Pa");
}

/// R3 condenser: one public 60 s step from a 35 degC shell with both CW pumps
/// is accepted and cannot cool the shell below the CW inlet.
#[test]
fn long_cold_condenser_step_is_accepted_above_cw_inlet() {
    let mut sim = Sim::new(ScenarioName::ColdShutdown);
    for index in 0..N_CW_PUMPS {
        sim.apply(Command::SetCwPump { index, on: true });
    }
    sim.cond.set_temperature(35.0);
    assert!(sim.step(60.0), "{:?}", sim.model_error);
    assert!(sim.cond.t_sat > CW_INLET_C, "shell {} degC", sim.cond.t_sat);
}

/// Sol R1: sealed, gas-bearing inventory heated to 150 degC, MSIVs shut, no
/// feed, all ICs open. Vessel + IC primary energy changes by exactly core heat
/// minus pool duty and vessel ambient loss, summed independently each step
/// (the vessel debit and IC receipt use one steam enthalpy).
#[test]
fn gas_bearing_heatup_with_ic_closes_primary_energy_at_plant_level() {
    let mut sim = Sim::new(ScenarioName::ColdShutdown);
    sim.vessel.head_vent_open = false;
    let gas = sim.vessel.gas_mass;
    assert!(gas > 1.0, "cold vessel must hold head-space air, {gas} kg");
    let m = sim.vessel.mass;
    sim.vessel.energy = bwr_core::vessel::energy_for(m, 150.0).unwrap();
    sim.vessel.eos =
        bwr_core::vessel::solve_eos_with(m, sim.vessel.energy, bwr_core::vessel::HeadSpace::Sealed { gas_mass: gas }).unwrap();
    assert!(sim.vessel.eos.p_gas > 0.0 && !sim.vessel.eos.subcooled, "{:?}", sim.vessel.eos);
    sim.steam.msiv_cmd = [false; N_STEAM_LINES];
    sim.steam.msiv_pos = [0.0; N_STEAM_LINES];
    sim.fw = bwr_core::bop::Feedwater::new(0, 40.0, LevelControlMode::Manual, 0.0);
    for t in sim.ic.iter_mut() {
        t.cmd_open = true;
        t.valve_pos = 1.0;
        t.tube_mass = 500.0;
        t.heat = IC_TRAIN_RATED_W;
    }
    let primary = |s: &Sim| {
        let ic_m: f64 = s.ic.iter().map(|t| t.tube_mass).sum();
        let ic_e: f64 = s.ic.iter().map(|t| t.tube_mass * t.tube_h).sum();
        (s.vessel.mass + ic_m, s.vessel.energy + ic_e)
    };
    let (m0, e0) = primary(&sim);
    let mut external = 0.0;
    for _ in 0..20 {
        let loss = HEAT_LOSS_HOT * ((sim.vessel.eos.t - 20.0) / 268.0).max(0.0);
        assert!(sim.step(DT), "{:?}", sim.model_error);
        let q_pool: f64 = sim.ic.iter().map(|t| t.last.q_pool).sum();
        external += (sim.q_core - q_pool - loss) * DT;
    }
    assert!(sim.vessel.flows.w_ic_steam > 1.0, "IC idle: {:?}", sim.vessel.flows);
    let (m1, e1) = primary(&sim);
    assert!((m1 - m0).abs() < 1e-6, "mass created {} kg", m1 - m0);
    assert!((e1 - e0 - external).abs() < 10.0, "energy residual {} J", e1 - e0 - external);
}

/// R1 (both round-4 reviews): sealed, gas-free 90 degC vessel with all ICs
/// open into 100 degC pools after 5 MW/train of preceding duty. No primary
/// steam condenses and no pool heat is accepted, at either timestep.
#[test]
fn cold_primary_moves_no_heat_into_hotter_ic_pools() {
    for dt in [0.05, 0.01] {
        let mut sim = Sim::new(ScenarioName::ColdShutdown);
        sim.vessel.head_vent_open = false;
        sim.vessel.gas_mass = 0.0;
        let m = sim.vessel.mass;
        sim.vessel.energy = bwr_core::vessel::energy_for(m, 90.0).unwrap();
        sim.vessel.eos =
            bwr_core::vessel::solve_eos_with(m, sim.vessel.energy, bwr_core::vessel::HeadSpace::Sealed { gas_mass: 0.0 }).unwrap();
        assert!(sim.vessel.vapour_available(), "{:?}", sim.vessel.eos);
        sim.steam.msiv_cmd = [false; N_STEAM_LINES];
        sim.steam.msiv_pos = [0.0; N_STEAM_LINES];
        for t in sim.ic.iter_mut() {
            t.cmd_open = true;
            t.valve_pos = 1.0;
            t.tube_mass = 500.0;
            t.pool_t = 100.0;
            t.heat = 5.0e6;
        }
        let mut accepted = 0.0;
        for _ in 0..(1.0 / dt) as usize {
            assert!(sim.step(dt), "{:?}", sim.model_error);
            accepted += sim.ic.iter().map(|t| t.last.q_pool * dt).sum::<f64>();
        }
        assert_eq!(accepted, 0.0, "dt {dt}: {accepted} J into hotter pools");
        assert_eq!(sim.vessel.flows.w_ic_steam, 0.0);
    }
}
