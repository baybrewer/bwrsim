# BWRX-300 Existing Systems Refinement — Master Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. This master file is the single source of truth for file layout, shared type names, and function signatures. Each package has its own plan file (listed below); execute them in the order given here. If a package plan and this master disagree on a name or signature, this master wins and the package plan must be corrected.

**Goal:** Refine every subsystem already implemented in `crates/bwr-core` to the strongest fidelity supported by public BWRX-300 information, ESBWR design documentation, physical law, and reproducible validation, without adding new plant systems.

**Architecture:** Rust (`crates/bwr-core`) remains the single source of simulated behaviour; Tauri streams snapshots; React renders. Refinement adds (a) an evidence register under `docs/model/`, (b) a deterministic validation runner with machine-readable traces and residual accounting, (c) explicit domain-error reporting instead of silent clamps, and (d) subsystem corrections driven by failing tests that demonstrate each defect before it is fixed.

**Tech Stack:** Rust 2021 (cargo 1.99 at planning time), serde/serde_json, TypeScript 5.6, React 18, Vite 5, Tauri 2.12, Python 3 with CoolProp/numpy/scipy for property and fit generation.

**Spec:** `docs/superpowers/plans/2026-10-20-existing-systems-refinement.md` (the codex plan). **Reviewed baseline:** commit `9cac46ac0e5b255f88c3d5a4deb064edac30a52c`. All 19 existing tests pass at this baseline (`cargo test --release -p bwr-core`: 11 unit + 8 integration).

## Global Constraints

Copied from the spec and from Jim's CLAUDE.md. Every task in every package implicitly includes these.

- Reference configuration: "a coherent North American 60 Hz reference configuration, matching the existing application. Identify differences before using UK or Canadian details."
- "Do not classify an ESBWR value or procedure as confirmed for BWRX solely because of design ancestry."
- "Do not replace simplified equations with elaborate unvalidated equations merely to increase model complexity. A validated reduced model is preferable to unsupported detail."
- "Do not scale dimensions, inventory, flow, decay-heat removal capacity, or timing by electrical output alone."
- "Do not bulk-retune the plant between packages or overwrite validation references simply to restore a passing startup script. Controller tuning follows corrections to the underlying physical model."
- "No previous-test failure is concealed by updating expected output."
- "No claim of exact design reproduction, regulatory approval, or validation against unavailable plant data is made."
- "Keep Rust authoritative." The browser mock is a UI demonstration only.
- A full containment model, LOCA package, electrical distribution model, chemical shutdown system, spent fuel system, or severe accident progression package is out of scope.
- Commands: `cargo test --release -p bwr-core`, `npm --prefix ui ci`, `npm --prefix ui run typecheck`, `npm --prefix ui run build`. After Package 2: `cargo run --release -p bwr-core --example validation`.
- No new Rust or npm dependencies without asking Jim first (CLAUDE.md). Hand-roll CSV and JSON fixture writing with `std` and the existing `serde_json` dev-dependency.
- Every numeric constant added or changed gets a row in `docs/model/parameters.csv` with a status of `confirmed`, `derived`, `calibrated`, `esbwr-estimate`, `assumed`, or `unavailable`.
- Commit after every task. Commit messages end with `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.
- 2-space indentation in TypeScript; rustfmt defaults in Rust (do not run `cargo fmt` if the component is not installed; match surrounding style by hand).

---

## Package plan files and execution order

| Order | Package | Plan file | Depends on |
|---|---|---|---|
| 1 | Evidence register | `2026-10-20-pkg01-evidence-register.md` | none |
| 1 | Reproducible baseline | `2026-10-20-pkg02-reproducible-baseline.md` | none (parallel with Pkg 1) |
| 2 | Numerics and conservation | `2026-10-20-pkg03-numerics-conservation.md` | Pkg 2 |
| 3 | Vessel and circulation | `2026-10-20-pkg04-vessel-circulation.md` | Pkg 3 |
| 3 | Core, fuel, rods | `2026-10-20-pkg05-core-fuel-rods.md` | Pkg 3 (run after Pkg 4; both edit `params.rs` and `sim.rs`) |
| 4 | Power conversion | `2026-10-20-pkg06-power-conversion.md` | Pkg 4, 5 |
| 4 | Isolation condensers | `2026-10-20-pkg07-isolation-condensers.md` | Pkg 4 (run after Pkg 6; both edit `sim.rs`) |
| 5 | Protection and controls | `2026-10-20-pkg08-protection-controls.md` | Pkg 6, 7 |
| 6 | Scenarios | `2026-10-20-pkg09-scenarios.md` | Pkg 8 |
| 7 | Runtime and fidelity record | `2026-10-20-pkg10-runtime-fidelity-record.md` | Pkg 9 |

Deliveries (from the spec): first = Pkg 1 + 2 + 3; second = Pkg 4 + 5; third = Pkg 6 + 7 + 8; fourth = Pkg 9 + 10.

Each package follows the spec's five-step loop: resolve requirements and envelope; write the subsystem spec and failing tests; implement the smallest justified change; run targeted tests and dependent scenarios; record results and sources in a reviewable commit.

---

## File structure (locked)

### Documentation (created by the package in parentheses)

```
docs/model/design-basis.md            (P1)  reference configuration, jurisdiction, revision policy
docs/model/sources.csv                (P1)  one row per public document
docs/model/parameters.csv             (P1)  one row per constant in the engine and UI
docs/model/assumptions.md             (P1)  numbered assumptions A-001.. referenced by code comments
docs/model/coverage.md                (P1)  subsystem -> evidence/assumption/test coverage table
docs/model/esbwr-applicability.csv    (P1)  one row per borrowed ESBWR concept
docs/model/decay-heat-reference.csv   (P5)  transcribed public decay-heat table used by the fit tool
docs/model/vessel.md                  (P4)  vessel envelope and model description
docs/model/core.md                    (P5)
docs/model/power-conversion.md        (P6)
docs/model/isolation-condensers.md    (P7)
docs/model/protection.csv             (P8)  cause-and-effect matrix
docs/validation/method.md             (P2)  metrics, tolerances, what a comparison means
docs/validation/cases.csv             (P2)  case register (id, scenario, purpose, reference, tolerance class)
docs/validation/baseline/<case>.csv   (P2)  regression traces (regression evidence, not truth)
docs/validation/results.md            (P10) published results and limitations
```

### Engine (`crates/bwr-core`)

```
src/lib.rs            add `pub mod domain; pub mod faults; pub mod ledger; pub mod validation; pub mod sensors;`
src/domain.rs         (P3) DomainError enum
src/faults.rs         (P2) Fault enum, honoured by P5-P9
src/ledger.rs         (P2) Ledger + BoundaryFlows residual accounting
src/validation.rs     (P2) Case, Trace, Metrics, Tolerance, run_case, standard_cases
src/sensors.rs        (P8) LagSensor
src/kinetics.rs       (P3) adaptive substeps, domain rejection
src/reactor.rs        (P3 rod ordering; P5 clad node, rod motion modes, SDM, decay-heat history)
src/vessel.rs         (P3 collapsed/mixture level, inventory bounds; P4 regime/gas/compressibility/moderator temp)
src/steam.rs          (P4 kappa column, compressed-liquid density, reference-point tests)
src/steam_table.rs    (P4 regenerated with 7 columns)
src/ic.rs             (P3 pool energy; P7 full restructure)
src/bop.rs            (P6)
src/sim.rs            (every package)
src/api.rs            (P3, P5, P6, P7, P8, P10 add fields listed in "API contract additions")
tests/plant.rs        (existing; P2 adds trace-based checks, P9 extends)
tests/numerics.rs     (P3)
tests/conservation.rs (P3)
tests/vessel_transients.rs   (P4)
tests/core_transients.rs     (P5)
tests/power_conversion.rs    (P6)
tests/ic_transients.rs       (P7)
tests/protection.rs          (P8)
tests/scenarios.rs           (P9)
tests/contract.rs            (P10) writes ui/src/sim/contract.generated.ts
examples/validation.rs       (P2) runs standard_cases, writes/compares baselines
tools/gen_steam_table.py     (P4) adds kappa_f column
tools/fit_decay_heat.py      (P5) fits to docs/model/decay-heat-reference.csv
```

### Shell and UI

```
src-tauri/src/lib.rs                 (P10) effective speed, extrema, command boundaries
ui/src/sim/types.ts                  (P3, P5, P6, P7, P8, P10) mirror of api.rs
ui/src/sim/contract.generated.ts     (P10) generated by tests/contract.rs; `satisfies Snapshot`/`Command`
ui/src/sim/mock.ts                   (each package that adds a Snapshot field; P10 relabel)
ui/src/sim/history.ts                (P10) no decimation loss
ui/src/components/*                  (P8 actual/commanded/measured labelling; P10 demo banner)
.github/workflows/validate.yml       (P2)
README.md                            (P2, P10)
```

---

## Shared interface contract

Names below are the only names later packages may use for these concepts. Signatures are exact. A package that needs something not listed here adds it to its own plan and, if another package will consume it, appends it to this section in the same commit.

### P2: `src/ledger.rs`

```rust
/// What the vessel actually applied in one step, after any bounding.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BoundaryFlows {
    pub w_feed: f64,        // kg/s into vessel
    pub h_feed: f64,        // J/kg
    pub w_steam: f64,       // kg/s out through main steam (after bounding)
    pub h_steam: f64,       // J/kg
    pub w_vent: f64,        // kg/s out through head vent
    pub w_liquid_out: f64,  // kg/s RWCU reject (after bounding)
    pub h_liquid_out: f64,  // J/kg
    pub q_core: f64,        // W into coolant
    pub q_ic: f64,          // W removed by ICs
    pub q_loss: f64,        // W insulation loss
    pub mass_clipped: f64,  // kg created or destroyed by a clamp this step (0 after P3)
    pub energy_clipped: f64,// J created or destroyed by a clamp this step (0 after P3)
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ledger {
    pub mass0_kg: f64,
    pub energy0_j: f64,
    pub mass_in_kg: f64,
    pub mass_out_kg: f64,
    pub energy_in_j: f64,
    pub energy_out_j: f64,
    pub mass_clipped_kg: f64,
    pub energy_clipped_j: f64,
}

impl Ledger {
    pub fn new(mass0_kg: f64, energy0_j: f64) -> Self;
    pub fn record(&mut self, dt: f64, f: &BoundaryFlows);
    /// mass_now - (mass0 + in - out). Zero means closed.
    pub fn mass_residual(&self, mass_now: f64) -> f64;
    pub fn energy_residual(&self, energy_now: f64) -> f64;
}
```

`Vessel::step(&mut self, dt: f64, inp: &VesselInputs) -> BoundaryFlows` (changed return type in P2). `Sim` gains `pub ledger: Ledger`, reset in `Sim::base` after construction and in `LoadScenario`.

### P2: `src/faults.rs`

```rust
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Fault {
    MsivStuck { line: usize },                 // P6: valve holds its current position
    TsvStuck,                                  // P6
    TcvStuck,                                  // P6
    BypassUnavailable,                         // P6
    PressureRegulatorFailed { demand_pct: u8 },// P6: regulator output frozen at demand_pct % of rated steam flow
    FeedPumpTrip { index: usize },             // P6: pump cannot run
    CondensatePumpTrip { index: usize },       // P6
    FcvStuck,                                  // P6
    HeatersLost,                               // P6
    CwPumpTrip { index: usize },               // P6
    VacuumPumpFailed,                          // P6
    IcTrainUnavailable { train: usize },       // P7: no heat removal, valves do not move
    IcReturnValveStuck { train: usize, valve: usize }, // P7
    IcSteamValveClosed { train: usize },       // P7
    RodStuck { index: usize },                 // P5: rod does not move under any mechanism
    IcNoncondensableDegradation { train: usize, pct: u8 }, // P7: UA reduced by pct %
}
```

`Sim` gains `pub faults: std::collections::HashSet<Fault>`, `pub fn set_fault(&mut self, f: Fault, active: bool)`, `pub fn has_fault(&self, f: &Fault) -> bool`. Faults are not `Command`s and never reach the UI contract. Automatic logic must never clear a fault; a package that honours a fault must test that.

### P2: `src/validation.rs`

```rust
use crate::api::{Command, ScenarioName};
use crate::faults::Fault;

#[derive(Clone, Debug)]
pub struct ScriptedCommand { pub at_s: f64, pub cmd: Command }

#[derive(Clone, Debug)]
pub struct ScriptedFault { pub at_s: f64, pub fault: Fault, pub active: bool }

#[derive(Clone, Debug)]
pub struct Case {
    pub id: &'static str,
    pub scenario: ScenarioName,
    pub duration_s: f64,
    pub sample_dt_s: f64,
    pub commands: Vec<ScriptedCommand>,
    pub faults: Vec<ScriptedFault>,
}

pub const TRACE_COLUMNS: [&str; 24] = [
    "t", "power_frac", "thermal_mw", "pressure_pa", "level_z_m", "level_nr_cm",
    "mass_kg", "energy_j", "core_flow_kgs", "steam_flow_kgs", "feed_flow_kgs",
    "t_feed_c", "fuel_t_c", "ic_heat_w", "rods_avg_pos", "msiv_avg", "tcv_pos",
    "bypass_pos", "scrammed", "mass_residual_kg", "energy_residual_j",
    "cond_pressure_pa", "hotwell_kg", "ic_pool_kg",
];

#[derive(Clone, Debug, PartialEq)]
pub struct Trace {
    pub case_id: String,
    pub dt_s: f64,
    pub rows: Vec<[f64; 24]>,
    /// (sim time, text) for scram, trips, isolations, IC initiation, domain errors.
    pub events: Vec<(f64, String)>,
}

impl Trace {
    pub fn to_csv(&self) -> String;
    pub fn from_csv(case_id: &str, s: &str) -> Result<Trace, String>;
    pub fn column(&self, name: &str) -> Vec<f64>;
    pub fn metrics(&self) -> Metrics;
}

#[derive(Clone, Debug, PartialEq)]
pub struct Metrics {
    pub final_values: std::collections::BTreeMap<String, f64>,
    pub peak: std::collections::BTreeMap<String, (f64, f64)>,  // name -> (value, time)
    pub min: std::collections::BTreeMap<String, (f64, f64)>,
    pub first_event_time: std::collections::BTreeMap<String, f64>, // event text prefix -> time
    pub max_abs_mass_residual_kg: f64,
    pub max_abs_energy_residual_j: f64,
    /// Max |x(t_end) - x(t_end - 60 s)| / 60 s over the last minute, per column.
    pub drift_per_s: std::collections::BTreeMap<String, f64>,
}

#[derive(Clone, Debug)]
pub struct Tolerance {
    pub rel: f64,                                   // relative on peaks/finals
    pub abs: std::collections::BTreeMap<String, f64>, // per-column absolute floor
    pub event_time_s: f64,
}

impl Tolerance {
    /// Numerical class: rel 1e-3, event 0.1 s, abs floors from method.md.
    pub fn numerical() -> Self;
    /// Physical class: rel 5e-2, event 2 s.
    pub fn physical() -> Self;
}

/// Returns one human-readable violation per metric outside tolerance; empty = pass.
pub fn compare(reference: &Metrics, candidate: &Metrics, tol: &Tolerance) -> Vec<String>;

/// Deterministic: same case + dt -> identical Trace.
pub fn run_case(case: &Case, dt: f64) -> Trace;

pub fn standard_cases() -> Vec<Case>;
```

`Sim` gains an append-only `pub log: Vec<(f64, String)>` written by `scram()`, turbine trips, MSIV isolation, IC auto-initiation, feed pump trips, and (from P3) domain errors. `run_case` drains it into `Trace.events`.

### P3: `src/domain.rs`

```rust
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum DomainError {
    PromptSupercritical { rho: f64 },
    EosOutOfRange { mass_kg: f64, energy_j: f64 },
    InventoryExhausted { deficit_kg: f64 },
    PropertyOutOfRange { what: String, value: f64 },
    FuelModelEnvelope { fuel_t_c: f64, clad_t_c: f64 },   // raised by P5
    IcModelEnvelope { train: usize, reason: String },     // raised by P7
}
```

`Sim` gains `pub domain_errors: Vec<(f64, DomainError)>` (first occurrence of each variant kept, in order). `Snapshot` gains `unsupported: Option<Unsupported>` where

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Unsupported { pub since_s: f64, pub reason: String }
```

The simulation keeps running after a domain error; the flag persists until `LoadScenario`. An alarm with id `unsupported`, severity `Trip`, text `MODEL ENVELOPE EXCEEDED: <reason>` is raised while the flag is set.

### P3: kinetics, core, vessel

```rust
// kinetics.rs
impl Kinetics {
    /// Adaptive backward Euler. Substep h = min(1e-3, 0.25*GEN_TIME/max(rho-BETA, 0)).
    /// Err if more than 20_000 substeps would be needed (rho - BETA > ~0.2) or the
    /// implicit denominator is still nonpositive.
    pub fn advance(&mut self, rho: f64, s: f64, dt: f64) -> Result<usize, DomainError>;
}
// reactor.rs
impl Core {
    pub fn step_rods(&mut self, dt: f64);          // moved out of step(); Sim calls it first
    pub fn step(&mut self, dt: f64, coolant_t: f64, htc_factor: f64) -> Result<f64, DomainError>;
}
// vessel.rs
pub struct Vessel { /* existing fields plus: */
    /// Level if all vapour were removed from the mixture, m above vessel bottom.
    pub collapsed_level_z: f64,
    /// Existing `level_z` is the two-phase mixture level in the downcomer/separator region.
}
impl Vessel {
    /// Fraction of active fuel covered by two-phase mixture: uses `level_z` only.
    pub fn core_coverage(&self) -> f64;
    pub fn step(&mut self, dt: f64, inp: &VesselInputs) -> BoundaryFlows; // outflows bounded by inventory
}
pub const M_VESSEL_MIN: f64 = 0.0; // the 1000 kg floor is removed; exhaustion is a DomainError
```

`Sim::step` order after P3: `core.step_rods(dt)` → `core.update_reactivity(...)` → `core.step(...)` → steam/turbine → IC → feedwater → vessel → condenser → `time += dt` → `protection()` → ledger record.

### P4: vessel regimes and properties

```rust
// steam.rs
pub struct Sat { /* existing plus */ pub kappa_f: f64 /* isothermal compressibility of sat. liquid, 1/Pa */ }
/// rho_f(t) * (1 + kappa_f(t) * (p - p_sat(t))), valid for p >= p_sat(t).
pub fn compressed_liquid_rho(t: f64, p: f64) -> f64;
pub const T_TABLE_MIN: f64 = 1.0;   // degC
pub const T_TABLE_MAX: f64 = 350.0;
// steam_table.rs: SAT_TABLE: [[f64; 7]; 350], column 6 = kappa_f [1/Pa]

// vessel.rs
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Regime { Vented, Saturated, WaterSolid, GasBlanketed }
pub struct Eos { pub t: f64, pub p: f64, pub x: f64, pub regime: Regime }
// `subcooled` and `solid` booleans are removed; callers use `regime`.
pub const R_AIR: f64 = 287.0;
pub struct Vessel { /* plus */ pub gas_mass: f64 /* kg noncondensable in the head */ }
impl Vessel {
    /// Average moderator temperature seen by the core: inlet temperature over the
    /// non-boiling length, saturation temperature over the boiling length.
    pub fn moderator_t(&self) -> f64;
}
```

`Snapshot.vessel` gains `regime: Regime` (TS: `"vented" | "saturated" | "waterSolid" | "gasBlanketed"`).

### P5: core, fuel, rods

```rust
// reactor.rs
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RodMotion { Idle, Motor { target: f64 }, Hydraulic, MotorRunIn }
pub struct Rods { /* layout, weights, pos, selected as today; plus */
    pub motion: Vec<RodMotion>,
    pub stuck: Vec<bool>,     // set from Fault::RodStuck by Sim each step
}
impl Rods {
    pub fn target(&self, i: usize) -> f64;            // Motor target or pos
    pub fn targets(&self) -> Vec<f64>;                // for Snapshot.rods.targets
    pub fn set_motor_target(&mut self, i: usize, target: f64);
    pub fn scram_hydraulic(&mut self);                // all rods -> Hydraulic
    pub fn motor_run_in(&mut self);                   // all rods -> MotorRunIn (ARI/FMCRD backup)
    pub fn select_rod_insert(&mut self);              // SRI group -> Hydraulic
    pub fn scramming(&self) -> bool;                  // any Hydraulic
    pub fn stop(&mut self);
    pub fn step(&mut self, dt: f64);
    /// Shutdown margin, dk/k, for the given feedback state with all rods in except `withdrawn`.
    pub fn shutdown_margin(&self, rho_other: f64, withdrawn: &[usize]) -> f64;
    pub fn strongest_rod(&self) -> usize;
}
pub struct Core { /* plus */ pub clad_t: f64 }
impl DecayHeat {
    /// Finite irradiation `irr_s` at power `n`, then cooled for `cooled_s`.
    pub fn after_history(n: f64, irr_s: f64, cooled_s: f64) -> Self;
}
```

`Snapshot.neutronics` gains `clad_temp_c: f64`. `Snapshot` gains `fuel_envelope_exceeded: bool` (replaces the `core_damage` concept; alarm id `fuel_envelope`, text `FUEL THERMAL MODEL ENVELOPE EXCEEDED`). The `core_damage` field and alarm are deleted.

### P6: power conversion

```rust
// bop.rs
pub struct SteamLine { pub mass: f64, pub p: f64 }   // lumped main steam line + header storage
pub struct SteamSystem { /* plus */
    pub tsv_pos: f64,                 // turbine stop valve, 0..1, closes in TSV_STROKE s on trip
    pub w_line: [f64; N_STEAM_LINES], // per-line flow through each MSIV
    pub line: SteamLine,
}
pub struct Turbine { /* plus */ pub p_extraction: f64 /* W of shaft work forgone to feedwater heating */ }
pub struct Condenser { /* plus */
    pub cst_mass: f64, pub makeup_flow: f64, pub reject_flow: f64,
}
pub struct Feedwater { /* plus */ pub pump_speed: [f64; N_FEED_PUMPS] /* 0..1, coastdown */ }
/// One-line plant energy balance computed every step.
#[derive(Clone, Copy, Debug, Default)]
pub struct EnergyBalance {
    pub q_core_w: f64, pub q_steam_out_w: f64, pub q_feed_in_w: f64, pub p_shaft_w: f64,
    pub p_extraction_w: f64, pub q_condenser_w: f64, pub q_ic_w: f64, pub q_loss_w: f64,
    pub house_w: f64, pub residual_w: f64,
}
```

`Sim` gains `pub balance: EnergyBalance`. `Snapshot.steam` gains `tsv_pos`, `line_flows_kgs: Vec<f64>`, `header_pressure_mpa`; `Snapshot.condenser` gains `cst_level_pct`, `makeup_kgs`, `reject_kgs`; `Snapshot.turbine` gains `extraction_mw`.

### P7: isolation condensers

```rust
// ic.rs
pub const N_IC_RETURN_VALVES: usize = 2;
pub struct IcTrain {
    pub steam_valve_pos: f64,                    // normally open
    pub return_valve_pos: [f64; N_IC_RETURN_VALVES], // parallel, normally closed
    pub cmd_open: bool,
    pub available: bool,
    pub ua_factor: f64,                           // 1.0 nominal; reduced by noncondensable fault
    pub tube_mass: f64,                           // condensate held in tubes/drain line, kg
    pub heat: f64,
    pub w_steam_in: f64,
    pub w_condensate_return: f64,
    pub pool_t: f64,
    pub pool_mass: f64,
}
pub struct IcStep { pub q_removed: f64, pub w_condensate_return: f64, pub energy_to_pool: f64, pub pool_boiloff_kg: f64 }
impl IcTrain {
    pub fn in_service(&self) -> bool;  // steam valve open and any return valve open
    pub fn step(&mut self, dt: f64, p_vessel: f64, t_sat: f64, steam_available: bool) -> IcStep;
}
```

`Snapshot.ic[i]` gains `steam_valve_pos`, `return_valve_pos: Vec<f64>`, `condensate_return_kgs`, `available`. The existing `valve_pos` field is kept as `max(return_valve_pos)` for backward compatibility with the mimic.

### P8: protection, sensors, display

```rust
// sensors.rs
#[derive(Clone, Copy, Debug)]
pub struct LagSensor { pub tau_s: f64, pub lo: f64, pub hi: f64, pub reading: f64, pub failed: bool }
impl LagSensor {
    pub fn new(tau_s: f64, lo: f64, hi: f64, initial: f64) -> Self;
    pub fn step(&mut self, dt: f64, truth: f64) -> f64;   // first-order lag then clamp
    pub fn out_of_range(&self) -> bool;
}
```

`Sim` gains `pub sensors: Sensors { pub level_nr: LagSensor, pub level_wr: LagSensor, pub pressure: LagSensor }`. All protection logic reads sensors, never `vessel.eos` directly. `Snapshot.vessel.level_nr_cm` becomes the narrow-range sensor reading; new fields `level_wr_cm`, `level_model_cm`, `level_nr_out_of_range: bool`, `pressure_sensed_mpa`. `Snapshot.rods` gains `motion: Vec<string>` (`"idle" | "motor" | "hydraulic" | "motorRunIn"`). The REFUEL interlock, the L8 feed-pump trip, and reset behaviour are corrected per `docs/model/protection.csv`.

### P10: runtime

`Sim::advance(real_dt)` no longer discards accumulated time. `Sim` gains `pub effective_speed: f64` and `pub dropped_sim_time_s: f64` (incremented only when the hard cap of `MAX_STEPS_PER_ADVANCE = 4000` is hit and the shell chose to drop; reported, never silent). `Sim` gains `pub fn take_extrema(&mut self) -> Extrema` with

```rust
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Extrema { pub power_frac_max: f64, pub pressure_mpa_max: f64, pub level_nr_cm_min: f64, pub level_nr_cm_max: f64, pub steps: u32 }
```

`Snapshot` gains `effective_speed`, `dropped_sim_time_s`, `extrema: Extrema`. `tests/contract.rs` writes `ui/src/sim/contract.generated.ts`; `npm --prefix ui run typecheck` fails when `types.ts` and `api.rs` diverge.

---

## API contract additions summary (for `ui/src/sim/types.ts` and `mock.ts`)

| Package | Snapshot / Command change |
|---|---|
| P3 | `unsupported: { sinceS: number; reason: string } \| null` |
| P4 | `vessel.regime: "vented" \| "saturated" \| "waterSolid" \| "gasBlanketed"` |
| P5 | `neutronics.cladTempC: number`; `fuelEnvelopeExceeded: boolean`; `rods.motion: RodMotion[]` added in P8 |
| P6 | `steam.tsvPos`, `steam.lineFlowsKgs: number[]`, `steam.headerPressureMpa`; `condenser.cstLevelPct`, `condenser.makeupKgs`, `condenser.rejectKgs`; `turbine.extractionMw` |
| P7 | `ic[].steamValvePos`, `ic[].returnValvePos: number[]`, `ic[].condensateReturnKgs`, `ic[].available` |
| P8 | `vessel.levelWrCm`, `vessel.levelModelCm`, `vessel.levelNrOutOfRange`, `vessel.pressureSensedMpa`; `rods.motion` |
| P10 | `effectiveSpeed`, `droppedSimTimeS`, `extrema` |

No `Command` variants are added by this program. Faults are engine-level only.

---

## Verified defects at baseline (drive Package 3 and 8 tests)

These were confirmed by reading the baseline code; each package plan contains a test that demonstrates the defect before fixing it.

1. `vessel.rs` `core_coverage()`: `level_z` comes from `level_from_volume(v_l, alpha_core, alpha_ch)`, which already distributes liquid with core voids; dividing again by `(1 - alpha_core)` double-corrects and overstates coverage. (P3)
2. `kinetics.rs` `step()`: `lhs` can be nonpositive when `dt*(rho - BETA)/GEN_TIME` exceeds about 1 (with the fixed 1 ms substep this is `rho > ~0.046`); the result is clamped to `1e-14` silently. (P3)
3. `sim.rs` `step()`: `core.update_reactivity` is evaluated with rod positions from the previous step, then `core.step` moves rods and advances kinetics with the stale reactivity: a one-step (0.05 s) lag during scram and rod motion. (P3)
4. `ic.rs` `step()`: pool sensible heating can overshoot 100 degC and the excess is discarded; `pool_mass.max(0.0)` discards energy at depletion; idle pool cooling removes energy with no boundary accounting. (P3, P7)
5. `vessel.rs` `step()`: `self.mass = (...).max(1000.0)` creates mass; `solve_eos` bisection clamps silently outside 1..349.5 degC. (P3)
6. `bop.rs` `Condenser::step()`: hotwell makeup/reject at ±30 kg/s and `clamp(0, HOTWELL_CAPACITY)` are unaccounted boundary flows. (P6)
7. `sim.rs` `withdraw_block()` REFUEL branch counts actual positions only; a single `RodSetTarget` with several indices, or a second command before motion starts, bypasses the one-rod interlock. (P8)
8. `sim.rs` `protection()`: the L8 feed-pump trip sits inside `if !self.turbine.tripped`, so with the turbine already tripped, high level never trips the feed pumps. (P8)
9. `sim.rs` `advance()`: when `n >= 4000` the accumulator is zeroed and simulation time is silently lost. (P10)
10. `history.ts`: 1 s sampling of 10 Hz snapshots, and 10 Hz snapshots of 20 Hz physics, can hide sub-second excursions. (P10)

---

## Self-review checklist for every package plan

Before a package plan is accepted for execution:

- [ ] Every spec bullet for that package maps to a named task.
- [ ] No placeholder text ("TBD", "add validation", "similar to Task N", code-free code steps).
- [ ] Every type and function name matches this master contract.
- [ ] Every new constant has a `parameters.csv` row step and a status.
- [ ] Every Snapshot change has a `types.ts` and `mock.ts` step and keeps `npm --prefix ui run typecheck` green.
- [ ] Every defect fix has a failing test first.
