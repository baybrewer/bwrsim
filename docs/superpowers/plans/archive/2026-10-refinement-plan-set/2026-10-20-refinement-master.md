# BWRX-300 Existing Systems Refinement — Master Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. This master file is the single source of truth for file layout, shared type names, and function signatures. Each package has its own plan file (listed below); execute them in the order given here. If a package plan and this master disagree on a name or signature, this master wins and the package plan must be corrected.

**Goal:** Refine every subsystem already implemented in `crates/bwr-core` to the strongest fidelity supported by public BWRX-300 information, ESBWR design documentation, physical law, and reproducible validation, without adding new plant systems.

**Architecture:** Rust (`crates/bwr-core`) remains the single source of simulated behaviour; Tauri streams snapshots; React renders. Refinement adds (a) an evidence register under `docs/model/`, (b) a deterministic validation runner with machine-readable traces and residual accounting, (c) explicit domain-error reporting instead of silent clamps, (d) a transfer-allocation step so every component consumes the same applied mass and energy transfers, and (e) subsystem corrections driven by failing tests that demonstrate each defect before it is fixed.

**Tech Stack:** Rust 2021 (cargo 1.99 at planning time), serde/serde_json, TypeScript 5.6, React 18, Vite 5, Tauri 2.12, Python 3 with CoolProp/numpy/scipy for property and fit generation.

**Spec:** `docs/superpowers/plans/2026-10-20-existing-systems-refinement.md` (the codex plan). **Reviewed baseline:** commit `9cac46ac0e5b255f88c3d5a4deb064edac30a52c`. All 19 existing tests pass at this baseline (`cargo test --release -p bwr-core`: 11 unit + 8 integration).

**Revision:** r2, 2026-10-20, after Codex review cycle 1 (22 comments resolved; see "Review log" at the end).

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
- **Derives rule:** every new struct or enum stored inside `Sim` (directly or nested) derives at least `Clone, Debug`; anything placed in `api.rs` also derives `Serialize, Deserialize` with `#[serde(rename_all = "camelCase")]`. Replacing an existing declaration preserves its existing derives (`Eos`: `Clone, Copy, Debug`; `IcTrain`: `Clone, Debug` + `Default`; `SteamSystem`: `Clone, Debug`).
- **Non-finite inputs rule:** every `Command` carrying an `f64` is ignored (with event id `cmd`, text `Command ignored: non-finite value`) when the value is not finite. Every public step function treats a non-finite input as a `DomainError::PropertyOutOfRange`.
- **dt rule:** every public step function validates `dt.is_finite() && dt > 0.0` before touching state; on failure a `Result`-returning step returns `Err(DomainError::PropertyOutOfRange { what: "dt".into(), value: dt })` and a void-returning step sets its `last_error` field; state is unchanged either way. Every public numeric argument is validated the same way (`what` names the argument).
- **Register-row rule:** the `parameters.csv` row for a constant is added or updated in the SAME task that adds or changes the constant (never deferred to a later task), so `tests/register.rs` passes at every task boundary.
- **Baseline regeneration protocol** (every package that changes physics): (1) run `cargo run --release -p bwr-core --example validation` in compare mode and save its output to `docs/validation/reports/<date>-pkg<NN>.md`; (2) classify every changed metric as physical (with the reason), schema-population (a column newly populated), or numerical; (3) pass the package's independent physical gates (its own tests); (4) run `--write` exactly once per reviewed change set; (5) commit the report and the baselines together. Never expect byte-identical traces after a column is newly populated; never adjust a tolerance after a failure to make the failure pass.
- Commit after every task. Commit messages end with `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.
- 2-space indentation in TypeScript; rustfmt defaults in Rust (do not run `cargo fmt` if the component is not installed; match surrounding style by hand).

---

## Package plan files and execution order

| Order | Package | Plan file | Depends on |
|---|---|---|---|
| 1 | Evidence register | `2026-10-20-pkg01-evidence-register.md` | none |
| 1 | Reproducible baseline | `2026-10-20-pkg02-reproducible-baseline.md` | none (parallel with Pkg 1) |
| 2 | Numerics and conservation | `2026-10-20-pkg03-numerics-conservation.md` | Pkg 1 **and** Pkg 2 (both acceptance gates passed: no physics change before the register exists) |
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

## File structure (locked; all paths repository-relative)

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
docs/validation/cases.csv             (P2)  case register (id, scenario, purpose, reference, tolerance class, role)
docs/validation/baseline/<case>.csv   (P2)  regression traces (regression evidence, not truth)
docs/validation/reference/<case>.csv  (P9)  transcribed public traces, if any are found
docs/validation/paired-failures.md    (P9)
docs/validation/findings.md           (P3 creates, P9 extends)  F-nnn log of physical assertion failures that are recorded and #[ignore]d, never retuned
docs/validation/results/*.csv         (P7, P9) tables written by tests when BWR_WRITE_RESULTS=1
docs/validation/reports/<date>-pkg<NN>.md (every physics package) compare-mode output saved before regeneration
docs/validation/results.md            (P10) published results and limitations
```

### Engine (`crates/bwr-core`)

```
crates/bwr-core/src/lib.rs            add `pub mod domain; pub mod faults; pub mod ledger; pub mod validation; pub mod sensors;`
crates/bwr-core/src/domain.rs         (P3) DomainError enum
crates/bwr-core/src/faults.rs         (P2) Fault enum, honoured by P5-P9
crates/bwr-core/src/ledger.rs         (P2) Ledger + BoundaryFlows residual accounting
crates/bwr-core/src/validation.rs     (P2) Case, Trace, Metrics, Tolerance, run_case, standard_cases
crates/bwr-core/src/sensors.rs        (P8) LagSensor, Sensors
crates/bwr-core/src/kinetics.rs       (P3) adaptive substeps, domain rejection, finite period estimator
crates/bwr-core/src/reactor.rs        (P3 rod ordering; P5 clad node, rod motion modes, SDM, decay-heat history)
crates/bwr-core/src/vessel.rs         (P3 collapsed/mixture level, inventory bounds, error channel; P4 regime/gas/compressibility/moderator temp)
crates/bwr-core/src/steam.rs          (P4 kappa column, compressed-liquid density, reference-point tests)
crates/bwr-core/src/steam_table.rs    (P4 regenerated with 7 columns)
crates/bwr-core/src/ic.rs             (P3 pool energy; P7 full restructure with mass ports)
crates/bwr-core/src/bop.rs            (P6)
crates/bwr-core/src/sim.rs            (every package)
crates/bwr-core/src/api.rs            (P3, P4, P5, P6, P7, P8, P10 add fields listed in "API contract additions")
crates/bwr-core/tests/common/mod.rs   (P2) shared test helpers `run`, `assert_finite`
crates/bwr-core/tests/plant.rs        (existing; P2 adds trace-based checks, P9 extends)
crates/bwr-core/tests/register.rs     (P1) enforces parameters.csv completeness
crates/bwr-core/tests/numerics.rs     (P3)
crates/bwr-core/tests/conservation.rs (P3)
crates/bwr-core/tests/vessel_transients.rs   (P4)
crates/bwr-core/tests/core_transients.rs     (P5)
crates/bwr-core/tests/power_conversion.rs    (P6)
crates/bwr-core/tests/ic_transients.rs       (P7)
crates/bwr-core/tests/protection.rs          (P8)
crates/bwr-core/tests/scenarios.rs           (P9)
crates/bwr-core/tests/contract.rs            (P10) writes ui/src/sim/contract.generated.ts
crates/bwr-core/examples/validation.rs       (P2) runs standard_cases, writes/compares baselines; P9 adds --write-missing; P10 adds --report
crates/bwr-core/examples/sensitivity.rs      (P5)
crates/bwr-core/examples/paired_failures.rs  (P9)
crates/bwr-core/examples/record_snapshots.rs (P10)
```

### Tools (repository root)

```
tools/gen_steam_table.py     (P4) adds kappa_f column
tools/fit_decay_heat.py      (P5) fits to docs/model/decay-heat-reference.csv
```

### Shell and UI

```
package.json                         (P2) `validate` script
src-tauri/src/lib.rs                 (P10) effective speed, extrema, command boundaries, wall-clock clamp accounting
ui/src/sim/bridge.ts                 (P10) initial snapshot on subscribe, reconnect
ui/src/sim/types.ts                  (P3, P4, P5, P6, P7, P8, P10) mirror of api.rs
ui/src/sim/contract.generated.ts     (P10) generated by tests/contract.rs
ui/src/sim/mock.ts                   (each package that adds a Snapshot field; P10 relabel)
ui/src/sim/history.ts                (P10) no decimation loss
ui/src/components/common/ValueTag.tsx (P8)
ui/src/components/*                  (P5 fuel envelope text; P8 actual/commanded/measured labelling; P10 demo banner, effective speed)
.github/workflows/validate.yml       (P2)
README.md                            (P1, P2, P5, P10)
```

---

## Shared interface contract

Names below are the only names later packages may use for these concepts. Signatures are exact. A package that needs something not listed here adds it to its own plan and, if another package will consume it, appends it to this section in the same commit.

### Shared `Sim::step` order (final form after P8; earlier packages implement the subset that exists)

```
 1. core.step_rods(dt)                                  (P3)   rods move first
 2. rods.stuck <- faults                                (P5)
 3. core.update_reactivity(alpha_core, vessel.moderator_t())   (P4 for moderator_t; eos.t before P4)
 4. q_core <- core.step(dt, t_cool, coverage)           on Err: keep previous q_core, record error, kinetics not advanced
 5. steam valves step, requested steam flows            (bop: SteamSystem::step, compute_flows)
 6. IC trains `request` -> requested IC steam withdrawal and condensate return   (P7; before P7 the IC `step` returns q_removed)
 7. feedwater step -> requested feed flow, bounded by hotwell inventory (P6: HotwellExhausted on shortfall)
 8. vessel.step(dt, &VesselInputs) -> BoundaryFlows     bounds ALL outflows to available inventory; this is the transfer-allocation point
 9. apply the allocation ratio to consumers: the APPLIED flow values (steam.w_tcv, steam.w_bypass, the RWCU flow passed to the
    condenser as flows.w_liquid_out, ic[i] applied IcRequest) are scaled by flows.w_steam / requested_w_steam (and the IC/RWCU
    equivalents) so every downstream component sees APPLIED transfers; operator setpoints (fw.rwcu_reject, p_setpoint, ...) are
    never mutated by allocation;
    IC trains `commit` with the ratio-scaled IcRequest (P7)
10. steam line storage step with applied inflow; the node bounds its own outflow (P6)
11. turbine.step with the node's applied w_tcv (P6) / vessel-scaled w_tcv (before P6)
12. condenser.step with applied w_tcv, w_bypass, rwcu, feed
13. sensors.step(dt, truth)                              (P8)
14. time += dt
15. protection()                                        (reads sensors only, from P8)
16. ledger.record(dt, &flows); extrema.update(...)       (P2 ledger; P10 extrema)
17. drain vessel.last_error / core error (held locally since step 4) / ic errors into domain_errors, all stamped with the same post-step time
```

The allocation ratio is 1.0 whenever inventory is adequate; it differs from 1.0 only in the step in which `InventoryExhausted` is raised. P3 adds a coupled low-inventory regression (`tests/conservation.rs`) asserting that the condenser hotwell gains exactly what the vessel released.

### Locked CSV headers (owned by the package in parentheses; every other package reads these columns by name)

```
docs/model/parameters.csv          (P1) id,value,unit,definition,reference_state,source_design,status,source_ids,interval_lo,interval_hi,assumption_id,location,notes
docs/model/sources.csv             (P1) see P1 plan (12 columns, first column `id`)
docs/model/esbwr-applicability.csv (P1) id,esbwr_source,esbwr_section,target_subsystem,bwrx_evidence,differences,adaptation,applicability_limits,test,classification
docs/validation/cases.csv          (P2) id,scenario,duration_s,purpose,reference,tolerance_class,role,status   (role: calibration | validation | plausibility; P9 fills it)
docs/model/protection.csv          (P8) id,function,signal,measurement,mode_applicability,threshold,threshold_unit,threshold_source,delay_s,latch,reset_condition,override,consequence,test
```

Tests that read a register (P1 `register.rs`, P5 SDM test, P9) parse by header name, never by column position, and refer to the parameter row by its `id` (e.g. `SDM_REQUIRED`).

### Scenario construction and bookkeeping

Every scenario constructor (`cold_shutdown`, `hot_standby`, `full_power`) ends by calling `Sim::finalize_init(&mut self)` which: sets `time = 0.0`, re-bases `ledger = Ledger::new(vessel.mass, vessel.energy)`, clears `log`, `events`, `acked`, `extrema` (seeded from current values), `accumulator`, `dropped_sim_time_s`, initialises `sensors` from truth (P8), and **retains** any `domain_errors` raised during initialisation, re-stamped at `t = 0.0` (an invalid initial condition must stay visible). `LoadScenario` clears `faults` (documented) and goes through the constructor, so it inherits this. Tests in P2 (`ledger` zero after `full_power()`), P3 (`domain_errors` retained), P10 (`extrema` seeded).

### P2: `src/ledger.rs`

```rust
/// What the vessel actually applied in one step, after bounding.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BoundaryFlows {
    pub w_feed: f64,         // kg/s into vessel
    pub h_feed: f64,         // J/kg
    pub w_steam: f64,        // kg/s out through main steam (applied)
    pub h_steam: f64,        // J/kg
    pub w_vent: f64,         // kg/s out through head vent
    pub w_liquid_out: f64,   // kg/s RWCU reject (applied)
    pub h_liquid_out: f64,   // J/kg
    pub w_ic_steam: f64,     // kg/s out to isolation condensers (applied; 0 before P7)
    pub w_ic_return: f64,    // kg/s condensate returned from ICs (0 before P7)
    pub h_ic_return: f64,    // J/kg
    pub q_core: f64,         // W into coolant
    pub q_ic: f64,           // W: before P7 the IC heat removal; from P7 DERIVED as w_ic_steam*h_steam - w_ic_return*h_ic_return (never added separately)
    pub q_loss: f64,         // W insulation loss
    pub mass_clipped: f64,   // kg created or destroyed by a clamp this step (0 after P3)
    pub energy_clipped: f64, // J created or destroyed by a clamp this step (0 after P3)
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
    /// Integrated transfers, for Metrics.integrated: keys "feed_kg", "steam_kg", "ic_steam_kg", "ic_return_kg", "rwcu_kg", "vent_kg", "core_j", "ic_j", "loss_j".
    pub integrated: std::collections::BTreeMap<&'static str, f64>,
}

impl Ledger {
    pub fn new(mass0_kg: f64, energy0_j: f64) -> Self;
    pub fn record(&mut self, dt: f64, f: &BoundaryFlows);
    /// mass_now - (mass0 + in - out). Zero means closed.
    pub fn mass_residual(&self, mass_now: f64) -> f64;
    pub fn energy_residual(&self, energy_now: f64) -> f64;
}
```

`Vessel::step(&mut self, dt: f64, inp: &VesselInputs) -> BoundaryFlows` (changed return type in P2). `Sim` gains `pub ledger: Ledger` and `pub fn last_flows(&self) -> &BoundaryFlows` (the flows applied in the most recent step; P2 stores them on `Sim`, P7 and tests read them).

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

impl Fault {
    /// Variant name without payload, e.g. "PressureRegulatorFailed".
    pub fn kind(&self) -> &'static str;
    /// Equipment index the fault applies to (line, index, train; for IcReturnValveStuck: train*N_IC_RETURN_VALVES + valve). None for plant-wide faults.
    pub fn equipment(&self) -> Option<usize>;
    /// Payload validation: demand_pct <= 100, pct <= 100, indices within N_* bounds.
    pub fn is_valid(&self) -> bool;
}
```

`Sim` gains `pub faults: Vec<Fault>` (ordered, deterministic; **not** a HashSet).
`pub fn set_fault(&mut self, f: Fault, active: bool)`: if `!f.is_valid()` it is ignored with a log entry; when `active`, any existing entry with the same `(kind, equipment)` is replaced (so two `PressureRegulatorFailed` payloads can never coexist); when inactive, the entry with the same `(kind, equipment)` is removed.
`pub fn has_fault(&self, f: &Fault) -> bool` (exact equality) and `pub fn find_fault(&self, kind: &str, equipment: Option<usize>) -> Option<&Fault>` (for payload lookup). Faults are not `Command`s and never reach the UI contract. Automatic logic must never clear a fault; a package that honours a fault must test that.

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
    /// (from_t_s, dt_s): after from_t_s the runner switches to dt_s (long-duration cases, P7). None = single dt.
    pub dt_after_s: Option<(f64, f64)>,
    /// Closed-loop operator (P9). P2 case literals use `operator: Operator::None`.
    pub operator: Operator,
}

/// Closed-loop operators (P9). Named variants so `Case` stays `Clone + Debug` without boxing closures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operator { None, Startup { target_frac_pct: u8 }, PowerChange { from_pct: u8, to_pct: u8 }, ControlledShutdown, CooldownOnIc, RodWithdrawalError }
pub const OPERATOR_DT_S: f64 = 1.0;   // operators are consulted once per OPERATOR_DT_S of sim time

pub const TRACE_SCHEMA_VERSION: u32 = 2;
pub const N_TRACE_COLS: usize = 58;
/// Column order is fixed. Columns whose source does not exist yet are emitted as 0.0 and
/// documented in method.md as "populated from Package N".
pub const TRACE_COLUMNS: [&str; N_TRACE_COLS] = [
    "t",
    "power_frac", "power_frac_max_int", "thermal_mw", "decay_mw",
    "pressure_pa", "pressure_pa_max_int", "pressure_sensed_pa",            // sensed: P8
    "level_z_m", "collapsed_level_m",                                      // collapsed: P3
    "level_nr_cm", "level_nr_cm_min_int", "level_nr_cm_max_int", "level_model_cm", // sensed/model split: P8
    "mass_kg", "energy_j", "gas_mass_kg",                                  // gas: P4
    "core_flow_kgs", "steam_flow_kgs", "feed_flow_kgs", "t_feed_c",
    "t_coolant_c", "fuel_t_c", "clad_t_c", "moderator_t_c",                // clad: P5, moderator: P4
    "rods_avg_pos", "rods_min_pos", "rods_max_pos",
    "msiv_pos_0", "msiv_pos_1", "tsv_pos", "tcv_pos", "bypass_pos", "header_pressure_pa", // tsv/header: P6
    "turbine_rpm", "gross_we", "scrammed", "turbine_tripped", "breaker_closed", "mode",   // mode: 0 shutdown,1 refuel,2 startup,3 run
    "ic_heat_w_0", "ic_heat_w_1", "ic_heat_w_2",
    "ic_pool_kg_0", "ic_pool_kg_1", "ic_pool_kg_2",
    "ic_return_pos_0", "ic_return_pos_1", "ic_return_pos_2",               // max of the valve pair from P7; valve_pos before
    "cond_pressure_pa", "hotwell_kg", "cst_kg",                            // cst: P6
    "fw_pump_0", "fw_pump_1", "fw_pump_2",
    "mass_residual_kg", "energy_residual_j", "unsupported",                // unsupported: P3
];
```

`*_max_int` / `*_min_int` columns are extrema accumulated at **every physics step** since the previous sample row, independent of `sample_dt_s`, so sampled peaks cannot miss sub-sample excursions.

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct Trace {
    pub case_id: String,
    pub dt_s: f64,
    pub rows: Vec<[f64; N_TRACE_COLS]>,
    /// (sim time, text) for scram, trips, isolations, IC initiation, domain errors.
    pub events: Vec<(f64, String)>,
    /// Ledger.integrated at the end of the run (keys as in Ledger); serialised as `# integrated,<key>,<value>` lines.
    pub integrated: std::collections::BTreeMap<String, f64>,
    /// Per-column extrema with their times of FIRST occurrence, accumulated at every physics step (not at sample rows);
    /// serialised as `# extremum,<col>,<max>,<t_max>,<min>,<t_min>` lines. Metrics.peak/min read these, never the sampled rows.
    pub extrema: std::collections::BTreeMap<String, Extremum>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Extremum { pub max: f64, pub t_max: f64, pub min: f64, pub t_min: f64 }

impl Trace {
    /// CSV: line 1 `# schema,2`; line 2 `# case,<id>`; line 3 `# dt_s,<v>`; line 4 header; rows with `{:.16e}`;
    /// then one `# event,<t>,<text>` line per event. from_csv(to_csv(t)) == t exactly.
    pub fn to_csv(&self) -> String;
    pub fn from_csv(s: &str) -> Result<Trace, String>;
    pub fn column(&self, name: &str) -> Vec<f64>;
    pub fn metrics(&self) -> Metrics;
}
pub fn col_index(name: &str) -> usize; // panics on unknown column

#[derive(Clone, Debug, PartialEq)]
pub struct Metrics {  // peak/min come from Trace.extrema (physics-step resolution), not from sampled rows
    pub final_values: std::collections::BTreeMap<String, f64>,
    pub peak: std::collections::BTreeMap<String, (f64, f64)>,  // name -> (value, time); uses *_max_int where available
    pub min: std::collections::BTreeMap<String, (f64, f64)>,
    /// Keyed by event text before the first ':' (e.g. "scram", "turbine trip", "msiv isolation", "ic auto-initiation", "feed pump trip", "domain error").
    pub first_event_time: std::collections::BTreeMap<String, f64>,
    pub max_abs_mass_residual_kg: f64,
    pub max_abs_energy_residual_j: f64,
    /// Max |dx/dt| over the FIRST 60 s, per column: the initial-equilibrium residual.
    pub initial_drift_per_s: std::collections::BTreeMap<String, f64>,
    /// |x(t_end) - x(t_end - 60 s)| / 60 s, per column.
    pub drift_per_s: std::collections::BTreeMap<String, f64>,
    /// From Ledger.integrated at the end of the run.
    pub integrated: std::collections::BTreeMap<String, f64>,
    pub nonfinite: bool,
}

#[derive(Clone, Debug)]
pub struct Tolerance {
    pub rel: f64,
    pub abs: std::collections::BTreeMap<String, f64>,
    pub event_time_s: f64,
    pub extremum_time_s: f64,
    pub residual_mass_kg: f64,
    pub residual_energy_j: f64,
}

impl Tolerance {
    /// Numerical class. Initial values rel 1e-3, event 0.1 s, extremum time 1 s, residuals 1e-3 kg / 1e3 J,
    /// abs floors from method.md; P3 REPLACES these with 2x the measured dt-halving differences from the convergence study.
    pub fn numerical() -> Self;
    /// Physical class: rel 5e-2, event 2 s, extremum 10 s, abs floors 10x numerical.
    pub fn physical() -> Self;
    /// Source-comparison class: per-case values recorded in cases.csv (tolerance_class column); this constructor
    /// reads them from a `&str` of the cases.csv row. P9 uses it.
    pub fn source_comparison(cases_csv_row: &str) -> Self;
}

/// One violation per metric outside tolerance. Rules: a reference event missing in the candidate is a violation;
/// a nonfinite candidate value is a violation; extremum time is compared only when the extremum differs from the
/// final value; drift is reported, not compared; residuals are compared against the absolute residual tolerance.
pub fn compare(reference: &Metrics, candidate: &Metrics, tol: &Tolerance) -> Vec<String>;

/// Deterministic: same case + dt -> identical Trace. Runner clock is monotonic; commands and faults whose
/// at_s <= time + dt/2 are applied before the step, commands before faults at equal times, each in list order.
/// LoadScenario, SetPaused and SetSpeed are rejected (panic with the case id): scripted cases run at fixed dt.
/// Timing validation (panic naming the case id on failure): dt, sample_dt_s, duration_s and any dt_after_s.1 are finite and > 0;
/// sample_dt_s and duration_s are integer multiples of the dt in force (within 1e-9 relative); dt_after_s.0 lies on a sample boundary.
/// Terminal handling: the log is drained after every boundary action AND after the last step, so terminal-boundary commands and
/// operator actions appear in Trace.events exactly once; one final row is always emitted at time == duration_s.
/// A Trace with no rows, or missing the schema/case/dt metadata, or missing integrated/extremum lines, is rejected by from_csv
/// and by compare (a header-only baseline can never PASS). Nonfinite event times, metadata or tolerance inputs are violations.
pub fn run_case(case: &Case, dt: f64) -> Trace;
/// Closed-loop variant (P9): `operator` is called every OPERATOR_DT_S of sim time and returns commands to apply.
pub fn run_case_with<F: FnMut(&crate::Sim, f64) -> Vec<Command>>(case: &Case, dt: f64, operator: F) -> Trace;
/// Same, but on a caller-supplied Sim and returning it (P9 initial-condition tests).
pub fn run_sim_with<F: FnMut(&crate::Sim, f64) -> Vec<Command>>(sim: crate::Sim, case: &Case, dt: f64, operator: F) -> (Trace, crate::Sim);
/// `Case.operator` is dispatched by `run_case` to `validation::operators::{startup, power_change, controlled_shutdown, cooldown_on_ic, rod_withdrawal_error}`.

pub fn standard_cases() -> Vec<Case>;
```

`Sim` gains an append-only `pub log: Vec<(f64, String)>` written by `scram()`, turbine trips (detected via a private `prev_turbine_tripped` flag), MSIV isolation, IC auto-initiation, feed pump trips, ignored commands, and (from P3) domain errors. `run_case` drains it into `Trace.events`.

**Log-line convention (locked; every `self.log.push` in every package):** the text is `"<key>: <detail>"` where `<key>` is one of
`scram`, `turbine trip`, `msiv isolation`, `ic auto-initiation`, `feed pump trip`, `domain error`, `rod block`, `sri`, `ari`, `breaker open`, `command ignored`, `overload`, `ic model`, `ic manual`, `fault`.
`Metrics.first_event_time` is keyed on the text before the first `:`. A package that needs a new key appends it to this list in the same commit. Examples: `feed pump trip: L8 high level`, `turbine trip: low condenser vacuum`, `ic auto-initiation: high pressure`. `tests/plant.rs` (P2) carries a test that every log line produced by the standard cases starts with a listed key.

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
    HotwellExhausted { deficit_kg: f64 },                 // raised by P6 (feed request exceeds hotwell inventory)
}
impl DomainError { pub fn reason(&self) -> String; }
```

`Sim` gains `pub domain_errors: Vec<(f64, DomainError)>` (first occurrence of each variant kept, in order) and `pub fn domain_error(&mut self, e: DomainError)` (public so tests can inject). `Snapshot` gains `unsupported: Option<Unsupported>` where

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Unsupported { pub since_s: f64, pub reason: String }
```

The simulation keeps running after a domain error; the flag persists until `LoadScenario`. An alarm with id `unsupported`, severity `Trip`, text `MODEL ENVELOPE EXCEEDED: <reason>` is raised while the flag is set.

**Rejection semantics (what commits on failure):**
- `Kinetics::advance` Err: `n`, `c`, `inv_period` unchanged.
- `Core::step` Err: rods already moved (step 1), xenon/decay/fuel **not** advanced, returns Err; `Sim` keeps the previous `q_core`.
- `Vessel::step` with `solve_eos` Err: `mass` and `energy` rolled back to their values at entry, flows returned as `BoundaryFlows::default()` (nothing applied), `last_error = Some(EosOutOfRange)`; downstream consumers therefore receive zero applied transfers for that step.
- `Vessel::step` inventory exhaustion: outflows scaled so `mass + dm*dt >= 0`, deficit recorded in `exhausted`, state advanced with the bounded flows.

### P3: kinetics, core, vessel

```rust
// kinetics.rs
pub const KIN_H_MAX: f64 = 1e-3;          // s, largest substep
pub const KIN_PROMPT_SAFETY: f64 = 0.05;  // h <= KIN_PROMPT_SAFETY * GEN_TIME / (rho - BETA): an ACCURACY bound (<= 5 % prompt growth per substep), not just positivity
pub const KIN_EXCESS_MAX: f64 = 0.02;     // dk/k above BETA: supported envelope. Growth rate 500/s; beyond this the fuel model is meaningless and f64 overflows within a plant step
pub const KIN_N_FLOOR: f64 = 1e-12;       // period estimator uses max(n, KIN_N_FLOOR); inv_period = 0 when n0 < floor
pub const KIN_N_MAX: f64 = 1e6;           // relative power ceiling: a candidate n1 > KIN_N_MAX or non-finite -> Err(PromptSupercritical), full rollback of n, c, inv_period
impl Kinetics {
    /// Adaptive backward Euler. h = min(KIN_H_MAX, KIN_PROMPT_SAFETY*GEN_TIME/max(rho-BETA, 0)); substeps = ceil(dt/h)
    /// with NO work cap (a 100 s call at rho=0 is 100_000 substeps, as today). Err(PromptSupercritical) when
    /// rho - BETA > KIN_EXCESS_MAX, the implicit denominator is nonpositive, or any substep candidate is non-finite or > KIN_N_MAX;
    /// on Err the ENTIRE entry state is restored (transactional across substeps). Period = (ln n1 - ln n0)/dt with the floor; always finite.
    /// Accuracy evidence (P3): compared against an independent reference (inhour asymptotic period for rho in {-0.05, -0.01, 0.001, 0.003};
    /// prompt-jump + h/10 refinement for rho - BETA in {0.005, 0.01, 0.02}) within 2 % on n after 0.05 s; the test is part of the envelope claim.
    pub fn advance(&mut self, rho: f64, s: f64, dt: f64) -> Result<usize, DomainError>;
}
// reactor.rs
impl Core {
    pub fn step_rods(&mut self, dt: f64);          // moved out of step(); Sim calls it first
    pub fn step(&mut self, dt: f64, coolant_t: f64, htc_factor: f64) -> Result<f64, DomainError>;
}
// vessel.rs
pub const T_EOS_MIN: f64 = 1.0;    // degC
pub const T_EOS_MAX: f64 = 349.5;
pub struct Vessel { /* existing fields plus: */
    /// Level if all vapour were removed from the mixture, m above vessel bottom.
    pub collapsed_level_z: f64,
    /// Deficit (kg) when outflows had to be bounded this step; None otherwise.
    pub exhausted: Option<f64>,
    /// Error raised in the last step, drained by Sim each step.
    pub last_error: Option<DomainError>,
    // Existing `level_z` is the two-phase mixture level in the downcomer/separator region.
}
impl Vessel {
    /// Fraction of active fuel covered by two-phase mixture: uses `level_z` only.
    pub fn core_coverage(&self) -> f64;
    pub fn step(&mut self, dt: f64, inp: &VesselInputs) -> BoundaryFlows; // outflows bounded by inventory
}
pub fn solve_eos(mass: f64, energy: f64) -> Result<Eos, DomainError>;  // P4 adds gas_mass and vent_open params
// The 1000 kg floor is removed; exhaustion is a DomainError.
```

`VesselInputs` from P3: `{ q_core, q_ic, w_feed, h_feed, w_steam, w_liquid_out }` (unchanged) and from P7: `q_ic` is replaced by `w_ic_steam, w_ic_return, h_ic_return`.

```rust
// ic.rs (P3 interim form; P7 extends the same request/commit shape with valves, tubes and mass ports)
pub struct IcRequest { pub w_steam_in: f64, pub w_condensate_return: f64, pub h_condensate_return: f64 }   // P3: q/h_fg, same value, h_f
pub struct IcStep { pub q_removed: f64, pub w_steam_in: f64, pub w_condensate_return: f64, pub h_condensate_return: f64, pub energy_to_pool: f64, pub pool_boiloff_kg: f64, pub pool_cooling_w: f64 }
impl IcTrain {
    pub fn request(&self, dt: f64, p_vessel: f64, w_steam_available: f64) -> IcRequest;   // pure
    pub fn commit(&mut self, dt: f64, p_vessel: f64, applied: IcRequest) -> IcStep;      // the only state change; applied = request x vessel ratio (0 on EOS rejection)
}
// Sim integrates every IcStep field: energy_to_pool and pool_cooling_w go to the ledger/energy balance as boundary terms from P3 on.
pub const CP_WATER: f64 = 4186.0; pub const H_FG_ATM: f64 = 2.257e6; pub const POOL_BOIL_T: f64 = 100.0; pub const POOL_IDLE_COOLING_K_PER_S: f64 = 0.0005;
```

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
#[derive(Clone, Copy, Debug)]
pub struct Eos { pub t: f64, pub p: f64, pub x: f64, pub regime: Regime }
// `subcooled` and `solid` booleans are removed; callers use `regime`.
pub const R_AIR: f64 = 287.0;
pub struct Vessel { /* plus */ pub gas_mass: f64 /* kg noncondensable in the head */ }
pub fn solve_eos(mass: f64, energy: f64, gas_mass: f64, vent_open: bool) -> Result<Eos, DomainError>;
impl Vessel {
    /// Average moderator temperature seen by the core: inlet temperature over the
    /// non-boiling length, saturation temperature over the boiling length.
    pub fn moderator_t(&self) -> f64;
}
```

`Snapshot.vessel` gains `regime: Regime` (TS: `"vented" | "saturated" | "waterSolid" | "gasBlanketed"`). P4 owns the `api.rs`/`types.ts`/`mock.ts` edits for this field.

P4 extensions (amended 2026-10-21 from the cycle-2 batch-3 review; consumed by P7's IC request hint and by tests):

```rust
// vessel.rs: Eos carries the partial pressures; `p == p_steam + p_gas` in every regime.
pub struct Eos { pub t: f64, pub p: f64, pub p_steam: f64, pub p_gas: f64, pub x: f64, pub regime: Regime }
pub const DISSOLVED_GAS_CAPACITY_KG: f64 = 1.0;  // gas up to this mass exerts no partial pressure
pub const P_EOS_MAX: f64 = 16.0e6;               // numerical total-pressure limit, every regime; sat_p never clamps below it
pub const GAS_BLANKET_ALARM_T_C: f64 = 90.0;
// steam.rs
pub const T_COMPRESSED_MAX: f64 = 300.0;         // linear-compressibility domain; water-solid states above it are rejected
impl Vessel {
    /// Dome holds steam above atmospheric pressure (label-independent). The IC supply hint at
    /// step 6 is `if vessel.steam_available() { mass * x / dt } else { 0.0 }`.
    pub fn steam_available(&self) -> bool;
    pub fn friction_dp(&self) -> f64;   // k_fric * phi2 * W^2, for momentum-balance checks
}
```

Transfer contract at step 8: `VesselInputs.w_steam` is a request for steam (water) mass. The vessel bounds it by the steam-space inventory and `w_liquid_out` by the liquid inventory; any binding cap is reported as `InventoryExhausted` with the deficit, and the ratio `flows.w_steam / requested` is the allocation ratio at step 9. Free noncondensable gas is swept out with the steam in the dome mass ratio, outside the water ledger and never counted against the request. `mass == 0` is a valid (zero-water) vessel state; a state with volume-derived quality above 1 is `EosOutOfRange`.

P3 tests migrated by P4 (not regressions): `solve_eos` non-finite arguments raise `PropertyOutOfRange` (non-finite rule) instead of `EosOutOfRange`; the P3 "outflows scaled so `mass + dm*dt >= 0`" rule becomes phase-aware (steam-space and liquid caps), so P3's one-step total-drain and Sim-level drain tests are migrated in P4 Task 4 Step 4, and a drained vessel under net heating exits via `EosOutOfRange` (superheated vapour) rather than `InventoryExhausted`.

### P5: core, fuel, rods

```rust
// reactor.rs
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RodMotion {
    Idle,
    Motor { target: f64 },  // operator motion at ROD_SPEED
    Scram,                  // hydraulic insertion at ROD_FULL_OUT/SCRAM_TIME, initiated by the RPS scram
    Sri,                    // hydraulic insertion of the SRI group, initiated by turbine trip / load rejection
    MotorRunIn,             // electric run-in at ROD_SPEED toward 0, initiated with every scram (FMCRD backup path)
}
pub struct Rods { /* layout, weights, pos, selected as today; plus */
    pub motion: Vec<RodMotion>,
    pub stuck: Vec<bool>,     // set from Fault::RodStuck by Sim each step
    pub last_error: Option<DomainError>, // dt rule: error channel of the void-returning `step`/`set_motor_target`; Core::step_rods forwards it into Core.last_error (drained at step 17)
}
impl Rods {
    /// Commanded endpoint: Motor target, 0.0 for Scram/Sri/MotorRunIn, pos for Idle.
    pub fn target(&self, i: usize) -> f64;
    pub fn targets(&self) -> Vec<f64>;                // for Snapshot.rods.targets
    pub fn set_motor_target(&mut self, i: usize, target: f64);   // rejects non-finite or out-of-range targets via last_error
    pub fn refresh_stuck(&mut self, mask: &[bool]);                // Sim each step: sets `stuck`; a newly stuck rod is parked Idle
    pub fn scram_hydraulic(&mut self);                // every non-stuck rod with pos > 0 -> Scram (Sri rods become Scram; rods already in stay Idle)
    pub fn motor_run_in(&mut self);                   // every non-stuck rod with pos > 0 not already Scram -> MotorRunIn
    pub fn select_rod_insert(&mut self);              // SRI group -> Sri (unless already Scram)
    pub fn scramming(&self) -> bool;                  // any rod in Scram (Sri does NOT count)
    pub fn moving(&self) -> bool;
    pub fn stop(&mut self);                           // Motor -> Idle at nearest notch; Scram/Sri/MotorRunIn are not stoppable
    pub fn step(&mut self, dt: f64);
    /// Shutdown margin, dk/k, for the given feedback state with all rods in except `withdrawn`.
    pub fn shutdown_margin(&self, rho_other: f64, withdrawn: &[usize]) -> f64;
    pub fn strongest_rod(&self) -> usize;
}
#[derive(Clone, Copy, Debug)]
pub struct ReactivityCoeffs { pub void: f64, pub doppler: f64, pub mod_t: f64, pub xenon_worth: f64 }  // Default = params; replaces Core::feedback and Xenon::reactivity
pub struct Core { /* plus */ pub clad_t: f64, pub coeffs: ReactivityCoeffs }
impl DecayHeat {
    /// Finite irradiation `irr_s` at power `n`, then cooled for `cooled_s`.
    pub fn after_history(n: f64, irr_s: f64, cooled_s: f64) -> Self;
}
// params.rs (P5): IRRADIATION_HISTORY_S = 6.31e7 (2 years, assumed), GAP_RESISTANCE_K_PER_W, CLAD_COOLANT_RESISTANCE_K_PER_W
// (sum equals the removed FUEL_RESISTANCE so rated fuel temperature is unchanged), CLAD_HEAT_CAP_J_PER_K,
// FUEL_MODEL_T_MAX_C = 1000.0 (clad, envelope not damage), UNCOVERED_ENVELOPE_S = 60.0, CORE_COVERED_FRAC = 0.999,
// Q_FUEL_RATED (made pub and registered), DECAY_FIT_MAX_REL_ERR (only when the decay-heat reference is retrieved; P5 Task 6).
// Sim (P5): `pub clad_t_step_max: f64` = max(entry, exit clad_t) of the step; the FuelModelEnvelope check reads it, not `core.clad_t`.
// Sim (P5): while `scrammed` is latched, `Sim::step` re-asserts `scram_hydraulic()` + `motor_run_in()` after the stuck-mask refresh so a freed rod resumes; nothing is replayed after `ResetScram`.
// Sim (P5): `pub fn full_power_with(coeffs: ReactivityCoeffs) -> Sim` (sensitivity).
// Sim (P9, not P5): `pub fn settle_until_steady(&mut self, max_s: f64) -> f64` (returns seconds settled).
```

Scram initiation paths are distinct and layered: `Sim::scram()` (RPS) calls `rods.scram_hydraulic()` then `rods.motor_run_in()`; `Sim::alternate_rod_insertion()` (P8; signals from the register with status `assumed`, guarded by `ARI_ENABLED: bool = false` until sourced) calls the same two rod methods through a separate latch `Sim.ari_latched`. `Sim.scrammed` remains the RPS latch; `ResetScram` sets Scram/MotorRunIn rods to Idle only when every such rod is at 0.0. SRI rods return to Idle on reaching 0.0.

`Snapshot.neutronics` gains `clad_temp_c: f64`. `Snapshot` gains `fuel_envelope_exceeded: bool`. `Sim.core_damage` (a `Sim` field, not a `Snapshot` field) and the `core_damage` alarm are deleted; the replacement alarm id is `fuel_envelope`, text `FUEL THERMAL MODEL ENVELOPE EXCEEDED`.

### P6: power conversion

```rust
// bop.rs
#[derive(Clone, Copy, Debug)]
pub struct SteamLine { pub mass: f64, pub energy: f64, pub p: f64, pub q_wall_w: f64 }
// Lumped main steam line + header storage. mass and energy are INTEGRATED from applied enthalpy flows
// (dm = w_in - w_out; dE = w_in*h_in - w_out*h_out). p is solved from mass via rho_g(p) = mass / V_STEAM_LINE_M3 on the
// saturation line within the registered property bounds (bracket validated; out of bounds -> DomainError::PropertyOutOfRange,
// transactional). The saturated-state constraint cannot hold energy exactly, so the mismatch is an EXPLICIT boundary term
// q_wall_w = (E_integrated - mass*u_g(p)) / dt (ASSUMED: exchange with line metal), reported in EnergyBalance.q_line_wall_w.
// Allocation with the node (from P6): the vessel bounds vessel->line inflow (w_line); the node then solves and bounds
// line->consumer outflow (w_tcv, w_bypass) against its own inventory plus applied inflow. Consumers see the NODE's applied outflow.
// P3's assertion `w_tcv + w_bypass == vessel steam outflow` is migrated by P6 to the storage-aware identity
// `w_tcv + w_bypass == flows.w_steam - dm_line/dt`, keeping a separate assertion `sum(w_line) == flows.w_steam`.
pub struct SteamSystem { /* plus */
    pub tsv_pos: f64,                 // turbine stop valve, 0..1, closes in TSV_STROKE s on trip
    pub w_line: [f64; N_STEAM_LINES], // per-line flow through each MSIV (vessel -> line node)
    pub line: SteamLine,
}
pub struct Turbine { /* plus */
    pub p_extraction: f64,  // W of shaft work forgone to feedwater heating (applied, bounded by steam availability)
    pub p_losses: f64,      // W dissipated this step, computed from entry energy + applied steam work - exit energy (never clipped)
    // p_elec is SIGNED: negative = grid import while motoring; Snapshot.gross_mwe reports the signed value
}
pub struct Condenser { /* plus */
    pub energy: f64,        // J, condenser + hotwell water energy, evolved by the first law each step
    pub q_in_w: f64,        // W thermal load arriving (steam + drains); NOT the rejection
    pub q_cw_w: f64,        // W actually rejected to cooling water = m_cw*cp*(T_out - T_in); zero with no CW pumps
    pub cst_mass: f64, pub makeup_flow: f64, pub reject_flow: f64,
    pub makeup_h: f64, pub reject_h: f64,  // J/kg recorded at transfer time
    pub overflow_kg: f64, pub deficit_kg: f64,
}
// Condenser temperature and pressure are DERIVED from `energy` and inventory (sat state of the hotwell water + air partial pressure),
// not lagged toward a target; with CW lost, q_cw_w = 0 and energy accumulates (pressure rises) until the trip.
pub struct Feedwater { /* plus */
    pub pump_speed: [f64; N_FEED_PUMPS],  // 0..1, coastdown; hydraulics use speed^2 affinity per pump
    pub heater_energy: f64,               // J stored in heater metal/shells: the 180 s lag is this storage, so heat after steam loss is accounted
    pub q_extraction_applied_w: f64,      // the ONE extraction value used by turbine, feedwater and condenser equations; bounded by steam-side availability
    pub pump_work_w: f64,                 // the 2 K pump term, counted once (fluid work), excluded from extraction heat
}
// Hotwell bound (step 7): requested feed is bounded by hotwell inventory, w_feed = min(request, hotwell_mass/dt), BEFORE vessel allocation;
// a shortfall raises DomainError::HotwellExhausted { deficit_kg } (new variant, added to domain.rs by P6) and the applied w_feed is used everywhere
// (vessel, condenser withdrawal, heater energy). Hotwell mass is never set to zero from a negative value.

/// Whole-plant energy balance, W, evaluated every step. Control volume: vessel + steam lines + turbine + condenser/hotwell + feed train + IC tubes.
/// Sign convention: positive = into the control volume. residual = in - out - storage_rate.
///   in      = q_core + makeup_flow*makeup_h + house_pump_work (fluid pump work, counted once)
///   out     = q_condenser (= q_cw_w, to cooling water) + q_ic (pool-side absorption) + q_loss + p_elec_gross (signed) + p_losses
///             + reject_flow*reject_h (incl. overflow to a full CST) + w_vent*h_g + q_line_wall_w
///   storage = d/dt(vessel.energy) + d/dt(line.energy) + d/dt(condenser.energy) + d/dt(fw.heater_energy) + d/dt(shaft kinetic energy)
///             + d/dt(ic tube energy, P7)   (finite differences over the step)
/// p_extraction is NOT a boundary term (it is internal: steam -> feedwater); it is reported for reconciliation only.
#[derive(Clone, Copy, Debug, Default)]
pub struct EnergyBalance {
    pub q_core_w: f64, pub q_condenser_w: f64, pub q_ic_w: f64, pub q_loss_w: f64,
    pub p_elec_gross_w: f64, pub p_losses_w: f64, pub p_extraction_w: f64, pub house_pump_work_w: f64,
    pub makeup_w: f64, pub reject_w: f64, pub vent_w: f64, pub q_line_wall_w: f64,
    pub storage_w: f64, pub residual_w: f64,
}
// Closure gate: |residual_w| < 0.01 * RATED_POWER_W at steady state AND through the first 60 s of fp_turbine_trip, CW loss and vacuum
// degradation. A failure is fixed by finding the missing transfer or storage term; tolerances are never loosened after a failure.
```

Signature changes owned by P6: `SteamSystem::new(msivs_open: bool, tsv_open: bool)`; `SteamSystem::compute_flows(&mut self, dt: f64, p_vessel: f64)` (the `tsv_open: bool` parameter is removed; TSV is state); `SteamSystem::vessel_steam_flow(&self) -> f64` (sum of `w_line`, the vessel's requested outflow); `SteamSystem::init_line_steady(&mut self, p_vessel: f64)` (scenario build); `Turbine::step(&mut self, dt, w_turbine, p_chest, p_cond, extraction_heat_w: f64)`; `Condenser::step(&mut self, dt, q, w_in, w_out, faults: &CondenserFaults)`; `Feedwater::step(.., faults: &FeedFaults)`; `Feedwater::set_mode(&mut self, mode)` (bumpless); fault views `SteamFaults`, `CondenserFaults`, `FeedFaults` are plain `Copy` structs of bools/options built by `Sim::bop_faults()` from `Sim.faults` each step. Constants: `MSIV_DP_FRACTION`, `LINE_FLOW_RATED`, `K_LINE`, `K_MSIV_OPEN` (MSIV as series resistance `K_LINE + K_MSIV_OPEN/pos^2`), `V_STEAM_LINE_M3`, `TSV_STROKE`, `TSV_OPEN_RATE`, `EXTRACTION_WORK_FRACTION = 0.25` (interval 0.15..0.35; 0.35 would exceed the isentropic drop), `AIR_UA_DEGRADATION`, `CST_CAPACITY_KG`, `CST_MASS_INITIAL`, `HOTWELL_MAKEUP_KGS`, `FW_STATIC_DP`, `FW_PUMP_COASTDOWN_S`, `SRI_TRIGGER_POWER_FRAC`, `RWCU_REJECT_MAX_KGS`. RWCU reject is pumped and feasible at any vessel pressure (assumption row); only the flow cap applies.

`Sim` gains `pub balance: EnergyBalance`. `Snapshot.steam` gains `tsv_pos`, `line_flows_kgs: Vec<f64>`, `header_pressure_mpa`; `Snapshot.condenser` gains `cst_level_pct`, `makeup_kgs`, `reject_kgs`; `Snapshot.turbine` gains `extraction_mw`.

### P7: isolation condensers

```rust
// ic.rs
pub const N_IC_RETURN_VALVES: usize = 2;
#[derive(Clone, Debug)]
pub struct IcTrain {
    pub steam_valve_pos: f64,                    // normally open
    pub return_valve_pos: [f64; N_IC_RETURN_VALVES], // parallel, normally closed
    pub cmd_open: bool,
    pub available: bool,
    pub ua_factor: f64,                           // 1.0 nominal; reduced by noncondensable fault
    pub tube_mass: f64,                           // condensate held in tubes/drain line, kg
    pub tube_energy: f64,                         // J, for the energy balance storage term
    pub heat: f64,
    pub w_steam_in: f64,                          // requested; Sim scales to applied after vessel allocation
    pub last_error: Option<DomainError>,          // dt rule (P3 interim form, kept by P7): commit's error channel, drained at step 17
    pub w_condensate_return: f64,
    pub pool_t: f64,
    pub pool_mass: f64,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct IcRequest { pub w_steam_in: f64, pub w_condensate_return: f64, pub h_condensate_return: f64 }
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct IcStep { pub q_removed: f64, pub w_steam_in: f64, pub w_condensate_return: f64, pub h_condensate_return: f64, pub energy_to_pool: f64, pub pool_boiloff_kg: f64, pub pool_cooling_w: f64 }
impl IcTrain {
    pub fn in_service(&self) -> bool;  // steam valve open and any return valve open
    /// Pure (no state change). `w_steam_available` is the vessel's allocation hint (kg/s; INFINITY when adequate, 0.0 with no steam space); the train never requests more.
    pub fn request(&self, dt: f64, p_vessel: f64, w_steam_available: f64) -> IcRequest;
    /// Advances valves, tubes and pool with the APPLIED flows: the request scaled by the vessel's steam and return ratios
    /// (1.0 normally, the vessel's bounding scale when exhausted, 0.0 when the vessel applied nothing).
    pub fn commit(&mut self, dt: f64, p_vessel: f64, applied: IcRequest) -> IcStep;
}
```

`Sim::step` calls `request` at step 6, builds `VesselInputs` from the summed requests, calls `commit` at step 9 with the ratio-scaled requests. No pool ever absorbs energy the vessel did not give up. IC energy accounting: `BoundaryFlows.q_ic` is vessel-side removal (ledger); `EnergyBalance.q_ic_w = sum(IcStep.q_removed)` is pool-side absorption (boundary term); `Sim.ic_tube_storage_w = sum(d tube_energy / dt)` is storage; identity `flows.q_ic == balance.q_ic_w + ic_tube_storage_w` holds every step and is tested. When steam supply disappears (`w_steam_available == 0`), `heat` is limited to the energy that can be released from `tube_mass` cooling; it does not continue through a lag. `Snapshot.ic[i]` gains `steam_valve_pos`, `return_valve_pos: Vec<f64>`, `condensate_return_kgs`, `available`. The existing `valve_pos` field is kept as `max(return_valve_pos)` for backward compatibility with the mimic.

### P8: protection, sensors, display

```rust
// sensors.rs
#[derive(Clone, Copy, Debug)]
pub struct LagSensor {
    pub tau_s: f64, pub lo: f64, pub hi: f64,
    pub state: f64,     // lagged, UNCLAMPED signal
    pub reading: f64,   // state clamped to [lo, hi]; frozen when failed
    pub failed: bool,
}
impl LagSensor {
    pub fn new(tau_s: f64, lo: f64, hi: f64, initial: f64) -> Self;
    pub fn step(&mut self, dt: f64, truth: f64) -> f64;   // first-order lag on state; reading = clamp; no update when failed
    pub fn out_of_range(&self) -> bool;                    // state < lo || state > hi (so an exact rail value is in range)
    pub fn fail(&mut self);                                // freezes reading at its current value, sets failed
}
#[derive(Clone, Debug)]
pub struct Sensors { pub level_nr: LagSensor, pub level_wr: LagSensor, pub pressure: LagSensor, pub header_pressure: LagSensor, pub condenser_pressure: LagSensor, pub last_error: Option<DomainError> }  // dt rule: error channel of the void-returning Sensors::step, drained at step 17 (so Sensors derives Clone, Debug, not Copy; LagSensor stays Clone, Copy, Debug)
impl Sensors { pub fn from_truth(sim: &Sim) -> Self; pub fn step(&mut self, dt: f64, sim_truth: &SensorTruth); }
pub struct SensorTruth { pub level_nr_cm: f64, pub level_wr_cm: f64, pub pressure_pa: f64, pub header_pressure_pa: f64, pub condenser_pressure_pa: f64 }
```

Measurement sources for every protection row are fixed in `docs/model/protection.csv` column `measurement`: level (NR for L8/L7/L4/L3, WR for L2/L1), vessel pressure, header pressure (low steam pressure isolation), condenser pressure (turbine trip, bypass block) come from `Sensors`; neutron power (APRM/IRM/SRM) and steam flow are **direct model inputs**, justified in `protection.csv` as fast instruments (lag below the physics step) with status `assumed`. The feedwater level controller reads `sensors.level_nr.reading`. `Snapshot.vessel.level_nr_cm` becomes the narrow-range sensor reading; new fields `level_wr_cm`, `level_model_cm`, `level_nr_out_of_range: bool`, `pressure_sensed_mpa`. `Snapshot.rods` gains `motion: Vec<String>` (`"idle" | "motor" | "scram" | "sri" | "motorRunIn"`). The REFUEL interlock (including the `out > 1` off-by-one), the L8 feed-pump trip, and reset behaviour are corrected per `protection.csv`.

### P10: runtime

`Sim::advance(real_dt)` no longer discards accumulated time. Constants `MAX_STEPS_PER_ADVANCE: usize = 4000`, `MAX_BACKLOG_S: f64 = 60.0`. Backlog beyond `MAX_BACKLOG_S` sim-seconds is added to `dropped_sim_time_s` and logged once per episode. `Sim` gains `pub effective_speed: f64` (exponential average over about 1 s of wall time of sim-seconds advanced per real second), `pub dropped_sim_time_s: f64`, `pub fn advance_with(&mut self, real_dt: f64, cmds: Vec<Command>)` (applies commands then advances), and `pub fn snapshot_with_extrema(&mut self) -> Snapshot` (the only consumer of the extrema window: fills `extrema`, then re-seeds). The Tauri shell's `dt.min(0.5)` wall-clock clamp (defect 11) is removed; the shell passes the true elapsed time and `advance` applies the backlog rule.

```rust
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Extrema {
    pub power_frac_max: f64, pub thermal_mw_max: f64, pub pressure_mpa_max: f64, pub level_nr_cm_min: f64, pub level_nr_cm_max: f64,
    pub fuel_t_c_max: f64, pub clad_t_c_max: f64, pub cond_pressure_kpa_max: f64, pub steps: u32,
}
impl Extrema { pub fn seed(snapshot_values: &Extrema) -> Self; /* copies the current values as both min and max, steps = 0 */ }
```

`Extrema` is seeded in `finalize_init` and after every `take`, never from zeros. `Snapshot` gains `effective_speed`, `dropped_sim_time_s`, `extrema: Extrema`. `history.ts` keeps every snapshot and folds `extrema` into each sample so charts draw an envelope band.

`tests/contract.rs` writes `ui/src/sim/contract.generated.ts` with: one `Snapshot` fixture per scenario **plus** synthesized fixtures that cover every enum value (`ModeSwitch`, `Regime`, `Severity`, `LevelControlMode`, `ScenarioName`, rod motion strings) and both states of every nullable field; a fixture for every `Command` variant (enumerated through an exhaustive `match`); `const _exhaustive: Record<keyof Snapshot, true>` built from the Rust field names (reverse direction); and a Rust-side unit-suffix test (every leaf field name ends with one of `_s, _mw, _mwe, _we, _mpa, _kpa, _pa, _c, _k, _m, _cm, _kgs, _kg, _pct, _rpm, _hz, _deg, _frac, _rel, _cps, _j_kg, _kj_kg, _cph, _pcm` or is in an explicit dimensionless allowlist). CI runs `git diff --exit-code ui/src/sim/contract.generated.ts` after `cargo test`.

---

## API contract additions summary (for `ui/src/sim/types.ts` and `mock.ts`)

| Package | Snapshot / Command change |
|---|---|
| P3 | `unsupported: { sinceS: number; reason: string } \| null` |
| P4 | `vessel.regime: "vented" \| "saturated" \| "waterSolid" \| "gasBlanketed"` |
| P5 | `neutronics.cladTempC: number`; `fuelEnvelopeExceeded: boolean` |
| P6 | `steam.tsvPos`, `steam.lineFlowsKgs: number[]`, `steam.headerPressureMpa`; `condenser.cstLevelPct`, `condenser.makeupKgs`, `condenser.rejectKgs`; `turbine.extractionMw` |
| P7 | `ic[].steamValvePos`, `ic[].returnValvePos: number[]`, `ic[].condensateReturnKgs`, `ic[].available` |
| P8 | `vessel.levelWrCm`, `vessel.levelModelCm`, `vessel.levelNrOutOfRange`, `vessel.pressureSensedMpa`; `rods.motion: string[]` |
| P10 | `effectiveSpeed`, `droppedSimTimeS`, `extrema` |

No `Command` variants are added by this program. Faults are engine-level only.

---

## Verified defects at baseline (drive Package 3, 8 and 10 tests)

Confirmed by reading the baseline code and, for 7 and 12, by independent reproduction during the Codex review. Each package plan contains a test that demonstrates the defect before fixing it.

1. `vessel.rs` `core_coverage()`: `level_z` comes from `level_from_volume(v_l, alpha_core, alpha_ch)`, which already distributes liquid with core voids; dividing again by `(1 - alpha_core)` double-corrects and overstates coverage. (P3)
2. `kinetics.rs` `step()`: `lhs` can be nonpositive when `dt*(rho - BETA)/GEN_TIME` exceeds about 1 (with the fixed 1 ms substep this is `rho > ~0.046`); the result is clamped to `1e-14` silently. (P3)
3. `sim.rs` `step()`: `core.update_reactivity` is evaluated with rod positions from the previous step, then `core.step` moves rods and advances kinetics with the stale reactivity: a one-step (0.05 s) lag during scram and rod motion. (P3)
4. `ic.rs` `step()`: pool sensible heating can overshoot 100 degC and the excess is discarded; `pool_mass.max(0.0)` discards energy at depletion; idle pool cooling removes energy with no boundary accounting; the 20 s heat lag keeps removing heat after steam supply disappears. (P3, P7)
5. `vessel.rs` `step()`: `self.mass = (...).max(1000.0)` creates mass; `solve_eos` bisection clamps silently outside 1..349.5 degC. (P3)
6. `bop.rs` `Condenser::step()`: hotwell makeup/reject at ±30 kg/s and `clamp(0, HOTWELL_CAPACITY)` are unaccounted boundary flows. (P6)
7. `sim.rs` `withdraw_block()` REFUEL branch counts actual positions only and blocks only when `out > 1`: a single `RodSetTarget` with several indices, a second command before motion starts, **and a second rod commanded after the first has fully withdrawn** all bypass the one-rod interlock. (P8)
8. `sim.rs` `protection()`: the L8 feed-pump trip sits inside `if !self.turbine.tripped`, so with the turbine already tripped, high level never trips the feed pumps. (P8)
9. `sim.rs` `advance()`: when `n >= 4000` the accumulator is zeroed and simulation time is silently lost. (P10)
10. `history.ts`: 1 s sampling of 10 Hz snapshots, and 10 Hz snapshots of 20 Hz physics, can hide sub-second excursions. (P10)
11. `src-tauri/src/lib.rs`: `let dt = (now - last).as_secs_f64().min(0.5)` discards wall-clock time whenever the thread stalls more than 0.5 s (sleep, lock contention); a second silent time-loss path independent of defect 9. (P10)
12. `kinetics.rs` `step()`: `Kinetics::equilibrium(0.0)` followed by `step()` computes `ln(n1/n0)` with `n0 = 0`, giving an infinite `inv_period`, then NaN on the next step. (P3)
13. `sim.rs` `step()` and `protection()`: downstream components (turbine, condenser, feedwater) consume **requested** steam and reject flows; nothing bounds them when the vessel cannot supply the request, so a vessel-only inventory bound would create mass downstream. (P3, via the transfer-allocation step)

---

## Self-review checklist for every package plan

Before a package plan is accepted for execution:

- [ ] Every spec bullet for that package maps to a named task.
- [ ] No placeholder text ("TBD", "add validation", "similar to Task N", code-free code steps).
- [ ] Every type and function name matches this master contract (r2).
- [ ] Every new struct/enum carries the derives required by the Derives rule.
- [ ] Every new constant has a `parameters.csv` row step and a status.
- [ ] Every Snapshot change has a `types.ts` and `mock.ts` step and keeps `npm --prefix ui run typecheck` green.
- [ ] Every defect fix has a failing test first.
- [ ] Every consumer of a transfer uses the applied (post-allocation) value.

---

## Review log

**Cycle 1 (Codex, planner role, 2026-10-20):** 22 comments. Resolved in r2 as follows. 1: rejection semantics and `Vessel.last_error`/`exhausted` channel added. 2: transfer-allocation step 8-9 added; defect 13 added; coupled low-inventory regression assigned to P3. 3: IC mass ports (`w_ic_steam`, `w_ic_return`, `h_ic_return`) added to `VesselInputs`/`BoundaryFlows`; `q_ic` declared derived; `IcStep` gains `w_steam_in`/`h_condensate_return`; tube energy storage and no-steam behaviour specified. 4: Derives rule added; `Sensors`, `SteamLine`, `Eos`, `IcTrain` derives stated. 5: work cap removed, physical envelope `KIN_EXCESS_MAX` kept, long-duration unit tests preserved. 6: `faults: Vec<Fault>`, `(kind, equipment)` replacement rule, `is_valid`, `find_fault`. 7: `finalize_init` defined. 8: trace schema v2 with per-equipment columns and per-step interval extrema. 9: `initial_drift_per_s`, `integrated`, `Tolerance::source_comparison`, compare rules, tolerances set from the P3 convergence study. 10: CSV metadata lines, runner clock/ordering/rejection rules. 11: `EnergyBalance` control volume and equation written. 12: `RodMotion::{Scram, Sri}` split, `target()` returns the commanded endpoint, initiation paths layered, `ARI_ENABLED` guard. 13: `Sensors` with five instruments, step-order position 13, direct-input justification rule. 14: `LagSensor.state` unclamped, `fail()`. 15: `Extrema::seed`, `snapshot_with_extrema` sole consumer, extra fields. 16: exhaustive enum/nullable fixtures, reverse `Record<keyof Snapshot, true>`, unit-suffix test, CI freshness. 17: defect 11 and P10 scope. 18: defect 7 extended. 19: defect 12 and `KIN_N_FLOOR`. 20: tools path, `package.json`, `bridge.ts` added. 21: P4 owns its API edits; `core_damage` wording fixed. 22: P3 depends on P1 and P2.
