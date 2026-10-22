# Package 3: Numerical and Conservation Defects — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Read `docs/superpowers/plans/2026-10-20-refinement-master.md` (revision r2) first; its interface contract wins over anything here. This plan is reconciled to master r2 as amended on 2026-10-21 for review cycle 2 (rejection semantics, transfer allocation, no kinetics work cap, `KIN_N_FLOOR`, `KIN_PROMPT_SAFETY`/`KIN_EXCESS_MAX`/`KIN_N_MAX` with transactional rollback and accuracy evidence, the dt rule, the register-row rule, the baseline regeneration protocol, the `IcTrain::request`/`commit` interim form, the shared step order with the core error drained at step 17, and `docs/validation/findings.md`).

**Goal:** Make every known numerical and conservation defect in `crates/bwr-core` visible through a failing test, then correct it so that mass and energy close to numerical tolerance and unsupported states are reported as explicit domain errors instead of silent clamps.

**Architecture:** Add `src/domain.rs` (`DomainError`) and plumb it through `Kinetics`, `Core`, `Vessel`, and `Sim` so each subsystem rejects states outside its envelope, with the rejection semantics fixed by master r2 ("what commits on failure"). Fix the five model defects (core coverage double-correction, kinetics floor collapse, zero-power period NaN, one-step rod reactivity lag, IC pool energy discard) and the two silent clamps (vessel 1 000 kg floor, EOS bisection range), and make every downstream consumer use the APPLIED transfers from the vessel's allocation step (master defect 13). Finish with edge-state and time-step convergence tests, measured numerical tolerances, and a reviewed baseline regeneration.

**Tech Stack:** Rust 2021, serde/serde_json (dev), TypeScript mirror in `ui/src/sim/types.ts`. No new dependencies.

**Depends on:** Packages 1 and 2 complete. From Package 2: `src/ledger.rs` (`Ledger` with `integrated`, `BoundaryFlows` with the IC ports `w_ic_steam`/`w_ic_return`/`h_ic_return`), `src/faults.rs` (`Sim.faults: Vec<Fault>`), `src/validation.rs` (`run_case`, `standard_cases`, `Trace::metrics`, `compare`, `Tolerance`, trace schema v2 with 58 columns: the master's `TRACE_COLUMNS` array is reused as-is and this package adds NO column, it only populates `collapsed_level_m` and `unsupported`), `Sim.log`, `Sim.ledger`, `Sim::last_flows() -> &BoundaryFlows` (the applied flows of the most recent step, stored on `Sim`), `Sim::finalize_init`, `Vessel::step -> BoundaryFlows`, `tests/common/mod.rs` (owns `run()` and `assert_finite(what, values)`), `examples/validation.rs` with compare and `--write` modes, and `docs/validation/baseline/*.csv`. From Package 1: `docs/model/parameters.csv` with its locked header.

## Global Constraints

All constraints in the master plan apply. Package-specific:

- "Tests demonstrate the original defect before correction." Every defect task has a Step that runs a demonstration test against the baseline and records its output before the fix is written.
- "Mass and energy residuals close to the stated numerical tolerance; domain errors are visible."
- "Replace silent collapse to the neutron floor with a solver strategy or explicit domain rejection appropriate to the supported transient."
- "Replace the vessel's 1,000 kg inventory floor and other silent physical clamps with bounded transfers, explicit residual accounting, or an unsupported-state outcome."
- "Compare results at 0.05, 0.025, and 0.0125 second plant steps, with suitable kinetics substeps. Require converged event timing and extrema for supported tests."
- "No previous-test failure is concealed by updating expected output." Baselines are regenerated once, in Task 9, following the master's baseline regeneration protocol: compare first and save the compare-mode output to `docs/validation/reports/2026-10-21-pkg03.md`, classify every changed metric as physical / schema-population / numerical, pass this package's own physical gates, run `--write` exactly once per reviewed change set, and commit the report with the baselines. Byte-identical traces are never expected once a column is newly populated (`collapsed_level_m`, `unsupported`).
- **Register-row rule (master):** every constant this package adds or changes (`KIN_*`, `T_EOS_*`, `CP_WATER`, `H_FG_ATM`, `POOL_BOIL_T`, `POOL_IDLE_COOLING_K_PER_S`, `validation.TOL_NUMERICAL_*`) gets its `docs/model/parameters.csv` row added or updated in the SAME task, with a status from the locked vocabulary `confirmed | derived | calibrated | esbwr-estimate | assumed | unavailable` (qualifications go in `notes`, never in `status`), so `tests/register.rs` passes at every task boundary.
- **dt rule and non-finite inputs rule (master):** every public step function this package touches validates `dt.is_finite() && dt > 0.0` and every other numeric argument before touching state (`Kinetics::advance`, `Core::step`, `Core::step_rods`, `IcTrain::request`/`commit`, `Vessel::step`, `Sim::step`). `Result`-returning steps return `Err(DomainError::PropertyOutOfRange { what, value })`; void-returning steps set their `last_error` field; `Sim::step` records the error itself. Zero or negative `dt` is rejected the same way as a non-finite one. The BOP step functions are Package 6's half of this rule.
- **Finding policy (master `docs/validation/findings.md`, created by this package):** when a *physical* expectation fails during this package (an edge-state test in Task 7, a steady case violating comparison in Task 9, a level reaching TAF in Task 2), the assertion and the parameters are NOT changed. The failure is recorded in `docs/validation/findings.md` as an entry `## F-nnn <case>: <assertion>` with the fields `id`, `date`, `case`, `assertion`, `observed`, `disposition` (the disposition names the owning package and whether the test is `#[ignore = "F-nnn"]`), and the file is created by the first task that records a finding, or by Task 9 Step 2 at the latest (with the header and format and the line `No open findings.` if none arose). Numerical assertions (convergence, residuals, determinism, allocation, rejection) are never ignored; they are fixed in the owning module.
- Commands: `cargo test --release -p bwr-core`, `npm --prefix ui run typecheck`, `cargo run --release -p bwr-core --example validation`.

## Names introduced by this package (beyond the master contract)

| Name | Where | Purpose |
|---|---|---|
| `DomainError::reason(&self) -> String` | `src/domain.rs` | Human-readable text for alarms and logs |
| `Sim::domain_error(&mut self, e: DomainError)` (pub) | `src/sim.rs` | Dedup-by-variant push, log, alarm |
| `KIN_H_MAX`, `KIN_PROMPT_SAFETY`, `KIN_EXCESS_MAX`, `KIN_N_FLOOR`, `KIN_N_MAX` | `src/kinetics.rs` | Adaptive substep rule (accuracy bound), supported reactivity ceiling (physical, no work cap), period-estimator floor, relative-power ceiling for candidate rejection (master values 1e-3, 0.05, 0.02, 1e-12, 1e6) |
| `Core.last_error: Option<DomainError>` | `src/reactor.rs` | Error channel of the void-returning `Core::step_rods` (dt rule), drained by `Sim::step` at step 17 |
| `Vessel.exhausted: Option<f64>` | `src/vessel.rs` | Deficit kg when outflows were bounded this step |
| `Vessel.last_error: Option<DomainError>` | `src/vessel.rs` | EOS rejection, drained by `Sim::step` |
| `T_EOS_MIN`, `T_EOS_MAX` | `src/vessel.rs` | EOS bisection envelope, 1.0 and 349.5 degC |
| `solve_eos(..) -> Result<Eos, DomainError>` | `src/vessel.rs` | Range-checked EOS |
| `IcRequest`, `IcStep` (seven fields), `IcTrain::request(dt, p_vessel, w_steam_available)`, `IcTrain::commit(dt, p_vessel, applied)` | `src/ic.rs` | Master interim form: a pure request before the vessel step, the only state change in `commit` with the APPLIED request (zero on EOS rejection) |
| `IcTrain.last_error: Option<DomainError>` | `src/ic.rs` | Error channel of `commit` (dt rule), drained by `Sim::step` at step 17 |
| `IcBoundary { q_removed_w, energy_to_pool_j, pool_cooling_j, pool_boiloff_kg, condensate_return_kg }` and `Sim.ic_boundary` | `src/ic.rs`, `src/sim.rs` | Integration of every `IcStep` field as boundary terms (master: "Sim integrates every IcStep field"); Package 6's `EnergyBalance.q_ic_w` reads `q_removed_w` |
| `CP_WATER`, `H_FG_ATM`, `POOL_BOIL_T`, `POOL_IDLE_COOLING_K_PER_S` | `src/ic.rs` | Named pool constants |
| `cmd_is_finite(&Command) -> bool` | `src/sim.rs` | Rejects non-finite numeric commands (master non-finite inputs rule) |
| `TOL_NUMERICAL_MASS_KG`, `TOL_NUMERICAL_ENERGY_J` | `src/validation.rs` (Task 6) | Conservation floors of the numerical class (1e-3 kg, 1e3 J): roundoff bounds, asserted by every conservation test and used as the `residual_*` fields of `Tolerance::numerical()` |
| `TOL_NUMERICAL_REL`, `TOL_NUMERICAL_EVENT_TIME_S`, `TOL_NUMERICAL_EXTREMUM_TIME_S`, `TOL_NUMERICAL_ABS: &[(&str, f64)]` | `src/validation.rs` (Task 8) | Every other field of `Tolerance::numerical()`, each 2x the measured dt-halving difference from the convergence table in `method.md`; one register row per constant and per `TOL_NUMERICAL_ABS` entry |
| `assert_snapshot_finite(&Snapshot)` | `tests/common/mod.rs` | Extends Package 2's file: walks the serialised snapshot and calls Package 2's existing `assert_finite(what, values)` per number. P2's `assert_finite` is kept unchanged with its callers; no second `assert_finite` is defined |

All kinetics constants above are in the master r2 contract; they are listed here because this package introduces them into the code. `KIN_MAX_SUBSTEPS` from the r1 draft is **removed**: a work cap would reject the existing 100 s / 120 s kinetics unit tests and its threshold depended on `dt`. `KIN_EXCESS_MAX` is the sole, physical, envelope.

---

### Task 1: `DomainError`, `Sim::domain_errors`, `Snapshot.unsupported`

**Files:**
- Create: `crates/bwr-core/src/domain.rs`
- Modify: `crates/bwr-core/src/lib.rs`
- Modify: `crates/bwr-core/src/api.rs` (add `Unsupported`, `Snapshot.unsupported`)
- Modify: `crates/bwr-core/src/sim.rs` (field, helper, alarm, snapshot)
- Modify: `crates/bwr-core/tests/plant.rs` (`json_contract_matches_ui` key list)
- Modify: `ui/src/sim/types.ts`, `ui/src/sim/mock.ts`
- Modify: `crates/bwr-core/tests/common/mod.rs` (owned by Package 2; this task only APPENDS `assert_snapshot_finite`, which calls P2's existing `assert_finite`; that function, its imports and its callers in `plant.rs`/`validation.rs` tests are untouched)
- Modify: `crates/bwr-core/src/validation.rs` (populate the `unsupported` trace column)
- Create: `crates/bwr-core/tests/numerics.rs`

**Interfaces:**
- Consumes: `Sim.log: Vec<(f64, String)>`, `Sim::finalize_init`, `tests/common/mod.rs::run`, trace schema v2 from Package 2 (the `unsupported` column exists and is emitted as 0.0).
- Produces: `DomainError` (master contract, six variants), `DomainError::reason()`, `Sim.domain_errors: Vec<(f64, DomainError)>`, `pub fn Sim::domain_error(&mut self, e: DomainError)`, `api::Unsupported { since_s, reason }`, `Snapshot.unsupported: Option<Unsupported>`, alarm id `unsupported` severity `Trip`.

- [ ] **Step 1: Write the failing test**

Append to Package 2's `crates/bwr-core/tests/common/mod.rs` (do not re-create the file; `run()` and `assert_finite(what: &str, values: impl IntoIterator<Item = f64>)` already exist there and are NOT redefined; add `use bwr_core::api::Snapshot;` to its `use` block):

```rust
/// Fields that are legitimately `null` in the JSON contract (see ui/src/sim/types.ts).
const NULLABLE: [&str; 4] = [
    "snapshot.scramReason",
    "snapshot.neutronics.periodS",
    "snapshot.rods.selected",
    "snapshot.unsupported",
];

/// Panics if any number in the snapshot is NaN/inf, naming the JSON path. Every
/// number goes through Package 2's `assert_finite`; serde_json turns a non-finite
/// f64 into `null`, so an unexpected `null` is treated as non-finite too.
pub fn assert_snapshot_finite(s: &Snapshot) {
    fn walk(v: &serde_json::Value, path: &str) {
        match v {
            serde_json::Value::Number(n) => {
                if let Some(x) = n.as_f64() {
                    assert_finite(path, std::iter::once(x));
                }
            }
            serde_json::Value::Null => {
                assert!(NULLABLE.contains(&path), "unexpected null (non-finite f64?) at {path}");
            }
            serde_json::Value::Array(a) => {
                for (i, x) in a.iter().enumerate() {
                    walk(x, &format!("{path}[{i}]"));
                }
            }
            serde_json::Value::Object(o) => {
                for (k, x) in o {
                    walk(x, &format!("{path}.{k}"));
                }
            }
            _ => {}
        }
    }
    let v = serde_json::to_value(s).expect("snapshot serialises");
    walk(&v, "snapshot");
}
```

Create `crates/bwr-core/tests/numerics.rs`:

```rust
//! Numerical defects: each test first demonstrated the baseline defect (see the
//! Package 3 plan), then was rewritten to assert the corrected behaviour.

mod common;

use bwr_core::api::{Command, ScenarioName, Severity};
use bwr_core::domain::DomainError;
use bwr_core::Sim;
use common::run;

#[test]
fn domain_errors_are_deduplicated_logged_and_alarmed() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 1.0);
    sim.domain_error(DomainError::PromptSupercritical { rho: 0.3 });
    sim.domain_error(DomainError::PromptSupercritical { rho: 0.4 });
    sim.domain_error(DomainError::InventoryExhausted { deficit_kg: 5.0 });
    assert_eq!(sim.domain_errors.len(), 2, "one entry per variant");
    assert_eq!(sim.domain_errors[0].1, DomainError::PromptSupercritical { rho: 0.3 });

    let s = sim.snapshot();
    let u = s.unsupported.expect("unsupported flag set");
    assert!((u.since_s - 1.0).abs() < 1e-9, "since {}", u.since_s);
    assert!(u.reason.contains("prompt supercritical"));
    assert!(s.alarms.iter().any(|a| a.id == "unsupported" && a.severity == Severity::Trip));
    // Locked log key (master log-line convention): Metrics.first_event_time keys on it.
    assert!(sim.log.iter().any(|(_, t)| t.starts_with("domain error: ")), "{:?}", sim.log);

    // The engine keeps running while flagged.
    run(&mut sim, 1.0);
    assert!(sim.time > 1.9);
    assert!(sim.snapshot().unsupported.is_some());

    // Only a scenario reload clears it.
    sim.apply(Command::ResetScram);
    assert!(sim.snapshot().unsupported.is_some());
    sim.apply(Command::LoadScenario { name: ScenarioName::FullPower });
    assert!(sim.domain_errors.is_empty());
    assert!(sim.snapshot().unsupported.is_none());
}

#[test]
fn domain_errors_raised_during_initialisation_survive_finalize_init() {
    // finalize_init (Package 2) clears log/events/ledger but must RETAIN domain
    // errors, re-stamped at t = 0: an invalid initial condition must stay visible.
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    sim.domain_error(DomainError::PropertyOutOfRange { what: "init".into(), value: 1.0 });
    sim.finalize_init();
    assert_eq!(sim.time, 0.0);
    assert_eq!(sim.domain_errors.len(), 1);
    assert_eq!(sim.domain_errors[0].0, 0.0, "error must be re-stamped at t = 0");
    assert!(sim.snapshot().unsupported.is_some());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --release -p bwr-core --test numerics domain_errors_are_deduplicated_logged_and_alarmed`
Expected: compile error `unresolved import bwr_core::domain` (and `no method named domain_error`).

- [ ] **Step 3: Create `src/domain.rs`**

```rust
//! Explicit model-envelope violations.
//!
//! The engine keeps running after a domain error is raised, but the state is
//! flagged as unsupported (alarm `unsupported`, `Snapshot.unsupported`) until the
//! scenario is reloaded. Nothing downstream of a flagged state may be presented
//! as a plant prediction.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum DomainError {
    /// Reactivity beyond the point-kinetics solver envelope (see kinetics.rs).
    PromptSupercritical { rho: f64 },
    /// Vessel mass/energy outside the 1..349.5 degC saturation-table bisection range.
    EosOutOfRange { mass_kg: f64, energy_j: f64 },
    /// Outflows demanded more inventory than the vessel held this step.
    InventoryExhausted { deficit_kg: f64 },
    PropertyOutOfRange { what: String, value: f64 },
    /// Raised by Package 5.
    FuelModelEnvelope { fuel_t_c: f64, clad_t_c: f64 },
    /// Raised by Package 7.
    IcModelEnvelope { train: usize, reason: String },
}

impl DomainError {
    pub fn reason(&self) -> String {
        match self {
            Self::PromptSupercritical { rho } => {
                format!("prompt supercritical (rho = {rho:.4} dk/k) beyond the point-kinetics envelope")
            }
            Self::EosOutOfRange { mass_kg, energy_j } => {
                format!("vessel state outside the property range (mass {mass_kg:.0} kg, energy {energy_j:.3e} J)")
            }
            Self::InventoryExhausted { deficit_kg } => {
                format!("vessel inventory exhausted (deficit {deficit_kg:.1} kg)")
            }
            Self::PropertyOutOfRange { what, value } => format!("{what} = {value:.4e} outside the property table"),
            Self::FuelModelEnvelope { fuel_t_c, clad_t_c } => format!(
                "fuel thermal model envelope exceeded (fuel {fuel_t_c:.0} degC, clad {clad_t_c:.0} degC)"
            ),
            Self::IcModelEnvelope { train, reason } => {
                format!("isolation condenser train {} outside the model envelope: {reason}", train + 1)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialises_with_camel_case_tag() {
        let e = DomainError::InventoryExhausted { deficit_kg: 2.5 };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["kind"], "inventoryExhausted");
        assert_eq!(v["deficitKg"], 2.5);
        let back: DomainError = serde_json::from_value(v).unwrap();
        assert_eq!(back, e);
    }
}
```

In `crates/bwr-core/src/lib.rs` add `pub mod domain;` in alphabetical order after `pub mod bop;`.

- [ ] **Step 4: Add `Unsupported` to `api.rs`**

After the `Alarm` struct in `crates/bwr-core/src/api.rs`:

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Unsupported {
    pub since_s: f64,
    pub reason: String,
}
```

In `Snapshot`, after `pub scram_reason: Option<String>,` add:

```rust
    /// Set after the first model-envelope violation; cleared only by LoadScenario.
    pub unsupported: Option<Unsupported>,
```

- [ ] **Step 5: Plumb through `sim.rs`**

Add to the `use` block: `use crate::domain::DomainError;`.

Add the field to `Sim` after `pub q_core: f64,`:

```rust
    /// First occurrence of each domain-error variant, with the sim time it was raised.
    pub domain_errors: Vec<(f64, DomainError)>,
```

Initialise in `base()` after `q_core: 0.0,`: `domain_errors: Vec::new(),`.

Add the helper after `pub fn scram(&mut self, reason: &str)`:

```rust
    /// Record a model-envelope violation. Only the first occurrence of each
    /// variant is kept; the log gets every first occurrence.
    pub fn domain_error(&mut self, e: DomainError) {
        let d = std::mem::discriminant(&e);
        if self.domain_errors.iter().any(|(_, x)| std::mem::discriminant(x) == d) {
            return;
        }
        // Locked log key `domain error` (master log-line convention).
        self.log.push((self.time, format!("domain error: {}", e.reason())));
        self.domain_errors.push((self.time, e));
    }
```

In `active_alarms()`, directly after the `if self.scrammed { ... }` block:

```rust
        if let Some((_, e)) = self.domain_errors.first() {
            add("unsupported", format!("MODEL ENVELOPE EXCEEDED: {}", e.reason()), Severity::Trip);
        }
```

In `snapshot()`, after `scram_reason: self.scram_reason.clone(),`:

```rust
            unsupported: self
                .domain_errors
                .first()
                .map(|(t, e)| api::Unsupported { since_s: *t, reason: e.reason() }),
```

`LoadScenario` goes through the scenario constructors and therefore through `Sim::finalize_init` (Package 2), which starts a fresh `domain_errors`. In `finalize_init`, add the retention rule (master "Scenario construction and bookkeeping"): before the clears, take the current `domain_errors` and re-insert them re-stamped at `t = 0.0`:

```rust
        // An invalid initial condition must stay visible after the bookkeeping reset.
        let kept: Vec<(f64, DomainError)> = self.domain_errors.drain(..).map(|(_, e)| (0.0, e)).collect();
        // ... existing finalize_init clears ...
        self.domain_errors = kept;
```

Do not touch `ResetScram`.

In `src/validation.rs`, the trace row builder (Package 2) emits the `unsupported` column as `0.0`. Change that expression to `if sim.domain_errors.is_empty() { 0.0 } else { 1.0 }` and update the "populated from Package N" note for `unsupported` in `docs/validation/method.md` from "Package 3 (pending)" to implemented.

- [ ] **Step 6: Mirror in the UI and the contract test**

`ui/src/sim/types.ts`: after `export interface Alarm { ... }` add

```ts
export interface Unsupported {
  /** Sim time at which the model first left its supported envelope. */
  sinceS: number;
  reason: string;
}
```

and in `Snapshot`, after `scramReason: string | null;` add `unsupported: Unsupported | null;`.

`ui/src/sim/mock.ts`: in the object literal that builds the `Snapshot` (the one containing `alarms:`), add `unsupported: null,` directly after `scramReason`.

`crates/bwr-core/tests/plant.rs`, in `json_contract_matches_ui`, add `"unsupported"` to the key array and after the `modeSwitch` assertion add `assert!(v["unsupported"].is_null());`.

- [ ] **Step 7: Run tests and typecheck**

Run: `cargo test --release -p bwr-core`
Expected: all previous tests pass plus `domain_errors_are_deduplicated_logged_and_alarmed ... ok` and `domain::tests::serialises_with_camel_case_tag ... ok`.

Run: `npm --prefix ui run typecheck`
Expected: exit 0.

- [ ] **Step 8: Commit**

```bash
git add crates/bwr-core/src/domain.rs crates/bwr-core/src/lib.rs crates/bwr-core/src/api.rs crates/bwr-core/src/sim.rs crates/bwr-core/src/validation.rs crates/bwr-core/tests/common/mod.rs crates/bwr-core/tests/numerics.rs crates/bwr-core/tests/plant.rs ui/src/sim/types.ts ui/src/sim/mock.ts docs/validation/method.md
git commit -m "feat(core): explicit DomainError and unsupported-state reporting

Adds domain.rs, Sim::domain_errors (first occurrence per variant), the
'unsupported' trip alarm and Snapshot.unsupported. No physics change.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 2: Core coverage double void correction

**Files:**
- Modify: `crates/bwr-core/src/vessel.rs` (`Vessel` struct, `new`, `update_level`, `core_coverage`)
- Modify: `crates/bwr-core/src/validation.rs` (populate the `collapsed_level_m` trace column)
- Modify: `crates/bwr-core/tests/numerics.rs`

**Interfaces:**
- Consumes: `Vessel::new(t, level_z, q, w_feed, t_feed, vent_open)`, `level_from_volume`.
- Produces: `Vessel.collapsed_level_z: f64`; `Vessel::core_coverage()` now uses the mixture level `level_z` only.

**Baseline defect (verified in code):** `level_z` is the solution of `liquid_volume_below(z, alpha_core, alpha_ch) == v_liquid`, which already distributes the liquid through voided core and chimney volumes, so `level_z` is a two-phase mixture level. `core_coverage()` then divides `(level_z - Z_CORE_BOTTOM)/CORE_HEIGHT` by `(1 - alpha_core).max(0.2)` again, overstating coverage by up to a factor 5.

- [ ] **Step 1: Write the demonstration test and record the defect**

Append to `tests/numerics.rs`:

```rust
use bwr_core::params::*;
use bwr_core::steam::sat_p;
use bwr_core::vessel::Vessel;

#[test]
fn core_coverage_uses_mixture_level_without_second_void_correction() {
    // Hot vessel with the mixture level one metre below the top of active fuel.
    let v = Vessel::new(sat_p(P_RATED).t, Z_TAF - 1.0, 0.05 * RATED_POWER_W, 0.0, T_FW_RATED, false);
    assert!(v.alpha_core > 0.05, "test needs a voided core, got {}", v.alpha_core);
    assert!(v.level_z < Z_TAF, "mixture level {} should be below TAF {}", v.level_z, Z_TAF);
    let expected = ((v.level_z - Z_CORE_BOTTOM) / CORE_HEIGHT).clamp(0.0, 1.0);
    println!("level_z {:.3} alpha_core {:.3} coverage {:.3} expected {:.3}", v.level_z, v.alpha_core, v.core_coverage(), expected);
    assert!((v.core_coverage() - expected).abs() < 1e-9, "coverage {} expected {}", v.core_coverage(), expected);
    assert!(v.core_coverage() < 1.0, "a level below TAF cannot mean a fully covered core");
    // Collapsed level is always at or below the mixture level.
    assert!(v.collapsed_level_z <= v.level_z + 1e-9);

    // At full power and NWL the core is covered and the voids swell the level.
    let v = Vessel::new(sat_p(P_RATED).t, Z_NWL, RATED_POWER_W, STEAM_FLOW_RATED, T_FW_RATED, false);
    assert_eq!(v.core_coverage(), 1.0);
    assert!(v.collapsed_level_z < v.level_z - 0.5, "collapsed {} mixture {}", v.collapsed_level_z, v.level_z);
}
```

- [ ] **Step 2: Run test to verify it fails on the baseline**

Run: `cargo test --release -p bwr-core --test numerics core_coverage_uses_mixture_level -- --nocapture`
Expected: compile error `no field collapsed_level_z`. To record the defect itself, temporarily comment out the two `collapsed_level_z` assertions and run again. Expected: FAIL with `coverage 1 expected 0.73...` (the printed line shows `coverage 1.000` while `level_z` is below `Z_TAF`). Paste that printed line into the Task 2 commit body. Restore the two assertions.

- [ ] **Step 3: Implement the fix**

In `crates/bwr-core/src/vessel.rs`, in `pub struct Vessel`, replace

```rust
    /// Level elevation above vessel bottom, m.
    pub level_z: f64,
```

with

```rust
    /// Two-phase mixture level, m above vessel bottom: the elevation at which the
    /// liquid inventory, distributed through the voided core and chimney, ends.
    /// This is what the core sees and what the downcomer level instruments measure.
    pub level_z: f64,
    /// Collapsed level, m above vessel bottom: where the same liquid would stand
    /// with all vapour removed. Always <= `level_z`. Used for inventory reporting only.
    pub collapsed_level_z: f64,
```

In `Vessel::new`, after `level_z,` in the struct literal add `collapsed_level_z: level_z,`.

Replace `update_level`:

```rust
    fn update_level(&mut self) {
        let rho_l = if self.eos.subcooled { steam::rho_liquid(self.eos.t) } else { sat_t(self.eos.t).rho_f };
        let v_l = self.mass * (1.0 - self.eos.x) / rho_l;
        self.level_z = level_from_volume(v_l, self.alpha_core, self.alpha_ch);
        self.collapsed_level_z = level_from_volume(v_l, 0.0, 0.0);
    }
```

Replace `core_coverage`:

```rust
    /// Fraction of the active fuel covered by two-phase mixture.
    /// `level_z` is already the mixture level (voids swell it), so no further void
    /// correction applies; dividing by (1 - alpha) again was the baseline defect.
    pub fn core_coverage(&self) -> f64 {
        ((self.level_z - Z_CORE_BOTTOM) / CORE_HEIGHT).clamp(0.0, 1.0)
    }
```

The `CORE UNCOVERED` alarm in `sim.rs` (`if self.vessel.core_coverage() < 0.999`) and the `htc_factor` passed to `Core::step` both call this function and need no edit.

In `src/validation.rs`, the trace row builder emits `collapsed_level_m` as `0.0` (schema v2, "populated from Package 3"). Change that expression to `sim.vessel.collapsed_level_z` and update the pending note in `docs/validation/method.md`.

- [ ] **Step 4: Run the tests**

Run: `cargo test --release -p bwr-core`
Expected: all pass, including `core_coverage_uses_mixture_level_without_second_void_correction ... ok`. If `full_power_is_steady` or `startup_from_hot_standby_to_30_percent` fail, stop: that would mean the level reaches TAF in those cases, which is a separate defect to report, not a tolerance to loosen.

- [ ] **Step 5: Commit**

```bash
git add crates/bwr-core/src/vessel.rs crates/bwr-core/src/validation.rs crates/bwr-core/tests/numerics.rs docs/validation/method.md
git commit -m "fix(vessel): core coverage no longer applies a second void correction

level_z is the mixture level (level_from_volume already distributes liquid
through the voided core), so core_coverage divided by (1 - alpha) twice.
Adds collapsed_level_z for inventory reporting.

Baseline demonstration: <paste printed line from Step 2>

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 3: Kinetics adaptive substeps and prompt-supercritical rejection

**Files:**
- Modify: `crates/bwr-core/src/kinetics.rs`
- Modify: `crates/bwr-core/src/reactor.rs` (`Core::step` caller; full rewrite of the kinetics call happens in Task 4)
- Modify: `crates/bwr-core/src/sim.rs` (`Sim::step` handles `Result`)
- Modify: `crates/bwr-core/tests/numerics.rs`
- Modify: `docs/model/parameters.csv`

**Interfaces:**
- Consumes: `DomainError::PromptSupercritical`.
- Produces: `Kinetics::advance(&mut self, rho: f64, s: f64, dt: f64) -> Result<usize, DomainError>` (returns substeps used; the ENTIRE entry state `n`, `c`, `inv_period` is restored on any Err, transactionally across substeps); `Core::step(dt, coolant_t, htc_factor) -> Result<f64, DomainError>` (validates all three arguments; on Err: rods already moved by `step_rods`, xenon/decay/fuel NOT advanced); constants with the master's values `KIN_H_MAX = 1.0e-3`, `KIN_PROMPT_SAFETY = 0.05`, `KIN_EXCESS_MAX = 0.02`, `KIN_N_FLOOR = 1e-12`, `KIN_N_MAX = 1e6`. There is NO work cap.
- Err conditions of `advance` (master): `dt` not finite or `<= 0`, `rho` or `s` not finite (`PropertyOutOfRange`, `what` = `"dt"`, `"kinetics rho"`, `"kinetics source"`); `rho - BETA > KIN_EXCESS_MAX`; an implicit denominator `<= 0`; any substep candidate `n1` or `c1[i]` non-finite or `n1 > KIN_N_MAX` (all `PromptSupercritical { rho }`). The period is `(ln n1 - ln n0) / h` on floored values, never a ratio that can overflow.
- Accuracy evidence (master): `kinetics_matches_inhour_and_refined_reference` compares the solver with an independent inhour solution (asymptotic inverse period for `rho` in {-0.05, -0.01, 0.001, 0.003}) and, above prompt critical (`rho - BETA` in {0.005, 0.01, 0.02}), with the inhour prompt root and an h/10 refinement of the same equations, within 2 %. The test is part of the envelope claim: `KIN_EXCESS_MAX` is only supported where this test passes.

**Baseline defects (verified in code; master defects 2 and 12):**
- Defect 2: `step()` computes `lhs = 1 - dt*(rho - BETA)/GEN_TIME - ...`. With the fixed 1 ms substep used by `Core::step`, `lhs <= 0` once `rho` exceeds about 0.046 dk/k, and `n1 = (rhs/lhs).max(1e-14)` then silently drives power to the floor instead of growing.
- Defect 12: `Kinetics::equilibrium(0.0)` followed by `step()` computes `let ip = (n1 / n0).ln() / dt;` with `n0 = 0`, giving an infinite `inv_period` (ln of inf), then NaN once a later ratio is 0/0 or inf is mixed into the smoothing lag.

**Envelope decision (master r2, amended 2026-10-21):** the supported ceiling is the physical constant `KIN_EXCESS_MAX = 0.02` dk/k above `BETA` (growth rate 500/s; beyond it the fuel model is meaningless and f64 overflows within one plant step), recorded as an assumption in `parameters.csv`. `KIN_PROMPT_SAFETY = 0.05` is an ACCURACY bound (at most 5 % prompt growth per substep), not merely a positivity bound; the solver is also protected by `KIN_N_MAX = 1e6` (a candidate above it, or non-finite, is rejected with full rollback). There is NO substep work cap: a cap would reject the legitimate long calls in the existing unit tests (100 s and 120 s advances) and any threshold expressed in substeps would change meaning with `dt`. The substep count at fixed `h` is linear in `dt` and bounded in practice by `Sim`'s 0.05 s plant step (50 substeps at `rho <= BETA`, 500 at the `KIN_EXCESS_MAX` ceiling where `h = 0.05*GEN_TIME/0.02 = 1e-4 s`).

**Accuracy analysis (done while planning; the test in Step 2 is the evidence, this paragraph only predicts it):** backward Euler amplifies by `1/(1 - wh)` per substep against the exact `exp(wh)`, so its growth-rate error is about `wh/2` and its error on `n` after `wT` e-foldings is about `wT * wh/2`. For the inhour cases `wh <= 0.2e-3`, far inside 2 %. For the prompt cases at `KIN_PROMPT_SAFETY = 0.05` the prediction is a growth-rate error near 2.6 % and an `n` error near 14 % at `rho - BETA = 0.005` over 0.05 s. If Step 6 confirms that prediction, the tolerance is NOT loosened: the executor lowers `KIN_PROMPT_SAFETY` along 0.05 -> 0.02 -> 0.01 -> 0.005 -> 0.002 until the test passes, records the measured error at every value in the commit body, registers the final value, and appends a one-line amendment (`KIN_PROMPT_SAFETY` value and the measured table) to the master's P3 kinetics section in the same commit. The substep cost at 0.002 is 12 500 substeps per 0.05 s plant step at the envelope edge and unchanged (50) at `rho <= BETA`.

- [ ] **Step 1: Demonstrate the two baseline defects**

Append to `tests/numerics.rs` (temporary, old signature):

```rust
use bwr_core::kinetics::Kinetics;

#[test]
fn baseline_kinetics_collapses_to_floor() {
    let mut k = Kinetics::equilibrium(1.0);
    k.advance(0.05, 0.0, 0.05, 50); // rho = 0.05 dk/k, 1 ms substeps as Core::step uses
    println!("baseline n after 50 ms at rho=0.05: {}", k.n);
    assert_eq!(k.n, 1e-14, "baseline clamps to the neutron floor instead of growing");
}

#[test]
fn baseline_zero_power_period_is_not_finite() {
    // Defect 12: equilibrium(0.0) then step() divides by n0 = 0 in the log-rate.
    let mut k = Kinetics::equilibrium(0.0);
    k.advance(0.0, 8.0e-6, 0.002, 2); // source startup from true zero power
    println!("baseline inv_period from zero power: {}", k.inv_period);
    assert!(!k.inv_period.is_finite(), "baseline period meter is inf/NaN from zero power");
}
```

Run: `cargo test --release -p bwr-core --test numerics baseline_ -- --nocapture`
Expected: both PASS, printing `baseline n after 50 ms at rho=0.05: 0.00000000000001` and a non-finite `inv_period` (inf or NaN). Paste both printed lines into the Task 3 commit body, then delete these two tests.

- [ ] **Step 2: Write the failing test for the corrected behaviour**

Append to `tests/numerics.rs`:

```rust
use bwr_core::kinetics::{KIN_EXCESS_MAX, KIN_H_MAX, KIN_N_MAX, KIN_PROMPT_SAFETY};

#[test]
fn kinetics_rejects_prompt_supercritical_instead_of_collapsing() {
    // Above prompt critical but inside the solver envelope: power grows, no clamp,
    // and the substep is the accuracy-bounded h = KIN_PROMPT_SAFETY*GEN_TIME/(rho-BETA).
    let mut k = Kinetics::equilibrium(1.0);
    let excess = 0.01;
    let substeps = k.advance(BETA + excess, 0.0, 0.05).expect("rho - BETA = 0.01 is inside the envelope");
    let h = KIN_H_MAX.min(KIN_PROMPT_SAFETY * GEN_TIME / excess);
    assert_eq!(substeps, (0.05 / h).ceil() as usize, "substeps at rho - BETA = {excess}");
    assert!(k.n > 1.0, "power must grow above prompt critical, got {}", k.n);
    assert!(k.inv_period > 0.0);

    // Beyond the physical envelope: explicit rejection, state untouched
    // (rejection semantics: Kinetics unchanged on Err). The baseline's 0.05 dk/k
    // demonstration value is now outside the envelope (0.044 > KIN_EXCESS_MAX).
    for rho in [BETA + KIN_EXCESS_MAX + 1e-6, 0.05, 0.3] {
        let mut k = Kinetics::equilibrium(1.0);
        let before = k.clone();
        let err = k.advance(rho, 0.0, 0.05).unwrap_err();
        assert!(matches!(err, DomainError::PromptSupercritical { rho: r } if (r - rho).abs() < 1e-12), "{err:?}");
        assert_eq!(k.n, before.n);
        assert_eq!(k.c, before.c);
        assert_eq!(k.inv_period, before.inv_period);
    }

    // Ordinary operation uses the 1 ms substep.
    let mut k = Kinetics::equilibrium(1.0);
    assert_eq!(k.advance(0.0, 0.0, 0.05).unwrap(), 50);

    // No work cap: a 100 s call at rho = 0 is 100_000 substeps, as at baseline.
    let mut k = Kinetics::equilibrium(1.0);
    assert_eq!(k.advance(0.0, 0.0, 100.0).unwrap(), 100_000);
    assert!((k.n - 1.0).abs() < 1e-6);
}

#[test]
fn kinetics_rejects_overflow_candidates_transactionally() {
    // Inside the reactivity envelope but growing past the power ceiling: at
    // rho - BETA = 0.02 (500/s) n crosses KIN_N_MAX within 0.03 s, so a 0.05 s call
    // must reject (never Ok with n = inf or inv_period = NaN) and restore n, c and
    // inv_period from the state at ENTRY, not from the last good substep.
    let mut k = Kinetics::equilibrium(1.0);
    let before = k.clone();
    let err = k.advance(BETA + KIN_EXCESS_MAX, 0.0, 0.05).unwrap_err();
    assert!(matches!(err, DomainError::PromptSupercritical { .. }), "{err:?}");
    assert_eq!(k.n, before.n);
    assert_eq!(k.c, before.c);
    assert_eq!(k.inv_period, before.inv_period);

    // Repeated plant steps at rho - BETA = 0.008 (n ~ e^10 per 0.05 s): the first
    // call commits, the call that would cross the ceiling is rejected, and the
    // state stays at the last committed value, finite and below the ceiling.
    let mut k = Kinetics::equilibrium(1.0);
    let mut committed = 0;
    for _ in 0..10 {
        match k.advance(BETA + 0.008, 0.0, 0.05) {
            Ok(_) => committed += 1,
            Err(DomainError::PromptSupercritical { .. }) => break,
            Err(e) => panic!("unexpected {e:?}"),
        }
    }
    assert!(committed >= 1, "the first 0.05 s at rho - BETA = 0.008 must commit");
    assert!(k.n.is_finite() && k.n <= KIN_N_MAX, "n = {}", k.n);
    assert!(k.c.iter().all(|c| c.is_finite()));
    assert!(k.inv_period.is_finite());
    let frozen = k.clone();
    assert!(k.advance(BETA + 0.008, 0.0, 0.05).is_err());
    assert_eq!(k.n, frozen.n, "a rejected call never moves the state");
    assert_eq!(k.inv_period, frozen.inv_period);

    // Starting just under the ceiling, even one substep of growth is rejected.
    let mut k = Kinetics::equilibrium(KIN_N_MAX * 0.999);
    assert!(k.advance(BETA + 0.001, 0.0, 0.05).is_err());
    assert_eq!(k.n, KIN_N_MAX * 0.999);
}

#[test]
fn kinetics_and_core_reject_invalid_step_arguments_unchanged() {
    use bwr_core::reactor::Core;
    // Master dt rule and non-finite inputs rule, table-driven: every numeric
    // argument of the two Result-returning public step functions, state unchanged.
    let bad_dt = [f64::NAN, f64::INFINITY, 0.0, -0.05];
    let bad_val = [f64::NAN, f64::INFINITY, f64::NEG_INFINITY];
    let k0 = Kinetics::equilibrium(1.0);
    let mut cases: Vec<(&str, f64, f64, f64)> = bad_dt.iter().map(|&dt| ("dt", 0.0, 0.0, dt)).collect();
    for &v in &bad_val {
        cases.push(("kinetics rho", v, 0.0, 0.05));
        cases.push(("kinetics source", 0.0, v, 0.05));
    }
    for (what, rho, src, dt) in cases {
        let mut k = k0.clone();
        let err = k.advance(rho, src, dt).unwrap_err();
        assert!(matches!(&err, DomainError::PropertyOutOfRange { what: w, .. } if w == what), "{what}: {err:?}");
        assert_eq!((k.n, k.c, k.inv_period), (k0.n, k0.c, k0.inv_period), "{what}: kinetics state changed");
    }

    let core0 = Core::new(1.0, ROD_FULL_OUT, 550.0);
    let core_cases: Vec<(&str, f64, f64, f64)> = bad_dt
        .iter()
        .map(|&dt| ("dt", dt, 287.0, 1.0))
        .chain(bad_val.iter().map(|&v| ("coolant_t", 0.05, v, 1.0)))
        .chain(bad_val.iter().map(|&v| ("htc_factor", 0.05, 287.0, v)))
        .collect();
    for (what, dt, t, htc) in core_cases {
        let mut core = core0.clone();
        let err = core.step(dt, t, htc).unwrap_err();
        assert!(matches!(&err, DomainError::PropertyOutOfRange { what: w, .. } if w == what), "{what}: {err:?}");
        assert_eq!(core.kin.n, core0.kin.n, "{what}");
        assert_eq!(core.xenon.xenon, core0.xenon.xenon, "{what}");
        assert_eq!(core.decay.d, core0.decay.d, "{what}");
        assert_eq!(core.fuel_t, core0.fuel_t, "{what}");
    }
}

/// Inhour relation of the point model solved in kinetics.rs,
/// rho = GEN_TIME*w + sum_i BETA*BETA_FRAC[i]*w/(w + LAMBDA_DN[i]),
/// which is monotonic on (-LAMBDA_DN[0], inf). Bisected for the asymptotic (largest)
/// root with the Keepin data in params.rs. Independent of the solver under test.
fn inhour_omega(rho: f64) -> f64 {
    let f = |w: f64| GEN_TIME * w + (0..6).map(|i| BETA * BETA_FRAC[i] * w / (w + LAMBDA_DN[i])).sum::<f64>();
    let (mut lo, mut hi) = (-LAMBDA_DN[0] * (1.0 - 1e-9), 1.0e4);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if f(mid) < rho {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

#[test]
fn kinetics_matches_inhour_and_refined_reference() {
    // Master accuracy evidence for the envelope claim: 2 % against independent references.
    // (a) Sub-prompt and negative reactivity: the asymptotic inverse period of the
    //     inhour equation. The run settles past the transient modes (the slowest lies
    //     between -LAMBDA_DN[1] and -LAMBDA_DN[0]), then the growth rate is measured
    //     over a window; rho = 0.003 is kept short so n stays below KIN_N_MAX.
    for (rho, settle_s, window_s) in [(-0.05, 600.0, 20.0), (-0.01, 600.0, 20.0), (0.001, 300.0, 20.0), (0.003, 40.0, 10.0)] {
        let w_ref = inhour_omega(rho);
        let mut k = Kinetics::equilibrium(1.0);
        for _ in 0..settle_s as usize {
            k.advance(rho, 0.0, 1.0).unwrap();
        }
        let n_a = k.n;
        for _ in 0..window_s as usize {
            k.advance(rho, 0.0, 1.0).unwrap();
        }
        let w_num = (k.n / n_a).ln() / window_s;
        println!("INHOUR rho {rho:+.4} omega_ref {w_ref:+.6e} omega_num {w_num:+.6e} err {:+.3} %", (w_num / w_ref - 1.0) * 100.0);
        assert!((w_num - w_ref).abs() <= 0.02 * w_ref.abs(), "rho {rho}: omega {w_num} vs inhour {w_ref}");
        assert!((k.inv_period - w_ref).abs() <= 0.02 * w_ref.abs() + 1e-6, "rho {rho}: period meter {} vs {w_ref}", k.inv_period);
    }

    // (b) Above prompt critical: the production call (one plant step, substep h chosen
    //     by the solver) against (1) the same equations advanced in h/10 calls through
    //     the public API and (2) the inhour prompt root for the late-window growth rate.
    //     The window at the envelope edge is 0.02 s so n stays below KIN_N_MAX.
    for (excess, window) in [(0.005, 0.05), (0.01, 0.05), (0.02, 0.02)] {
        let rho = BETA + excess;
        let h = KIN_H_MAX.min(KIN_PROMPT_SAFETY * GEN_TIME / excess);
        let w_ref = inhour_omega(rho);

        let mut prod = Kinetics::equilibrium(1.0);
        let steps = prod.advance(rho, 0.0, 0.9 * window).unwrap();
        assert_eq!(steps, (0.9 * window / h).round() as usize, "production substeps");
        let n_late = prod.n;
        prod.advance(rho, 0.0, 0.1 * window).unwrap();
        let w_late = (prod.n / n_late).ln() / (0.1 * window);

        let mut fine = Kinetics::equilibrium(1.0);
        let h_fine = h / 10.0;
        for _ in 0..(window / h_fine).round() as usize {
            fine.advance(rho, 0.0, h_fine).unwrap();
        }
        println!(
            "PROMPT excess {excess:.3} h {h:.2e} n_prod {:.6e} n_fine {:.6e} dn {:+.3} % omega_late {w_late:.4e} omega_ref {w_ref:.4e} dw {:+.3} %",
            prod.n, fine.n, (prod.n / fine.n - 1.0) * 100.0, (w_late / w_ref - 1.0) * 100.0
        );
        assert!(prod.n < KIN_N_MAX && fine.n < KIN_N_MAX, "window too long for the ceiling");
        assert!((prod.n / fine.n - 1.0).abs() <= 0.02, "rho - BETA = {excess}: n {} vs h/10 reference {}", prod.n, fine.n);
        assert!((w_late / w_ref - 1.0).abs() <= 0.02, "rho - BETA = {excess}: growth rate {w_late} vs inhour {w_ref}");
    }
}

#[test]
fn period_meter_is_finite_from_zero_power() {
    // Defect 12. Source startup from true zero power: n grows, period stays finite.
    let mut k = Kinetics::equilibrium(0.0);
    for _ in 0..1000 {
        k.advance(0.0, 8.0e-6, 0.05).unwrap();
        assert!(k.inv_period.is_finite(), "inv_period went non-finite at n = {}", k.n);
    }
    assert!(k.n > 0.0);

    // Zero source at zero power: nothing moves, period reads zero.
    let mut k = Kinetics::equilibrium(0.0);
    k.advance(0.0, 0.0, 1.0).unwrap();
    assert_eq!(k.n, 0.0);
    assert_eq!(k.inv_period, 0.0);

    // Deep shutdown decay toward the floor never produces a non-finite period.
    let mut k = Kinetics::equilibrium(1e-9);
    for _ in 0..200 {
        k.advance(-0.1, 0.0, 0.05).unwrap();
        assert!(k.inv_period.is_finite());
    }
}

#[test]
fn core_step_err_leaves_xenon_decay_and_fuel_unadvanced() {
    // Rejection semantics for Core::step (master r2): on Err the rods have already
    // moved (step_rods runs first in Sim::step) but xenon, decay heat and fuel
    // temperature commit nothing, and Sim keeps the previous q_core.
    use bwr_core::reactor::Core;
    let mut core = Core::new(1.0, ROD_FULL_OUT, 550.0);
    core.rho_total = 0.3; // force a PromptSupercritical rejection
    let xe = core.xenon.clone();
    let decay = core.decay.clone();
    let fuel_t = core.fuel_t;
    let n = core.kin.n;
    let err = core.step(0.05, 287.0, 1.0).unwrap_err();
    assert!(matches!(err, DomainError::PromptSupercritical { .. }));
    assert_eq!(core.kin.n, n);
    assert_eq!(core.xenon.iodine, xe.iodine);
    assert_eq!(core.xenon.xenon, xe.xenon);
    assert_eq!(core.decay.d, decay.d);
    assert_eq!(core.fuel_t, fuel_t);
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test --release -p bwr-core --test numerics kinetics_`
Expected: compile error `this method takes 4 arguments but 3 arguments were supplied` (and `unresolved import bwr_core::kinetics::KIN_EXCESS_MAX`).

- [ ] **Step 4: Implement the adaptive solver**

Replace the whole of `crates/bwr-core/src/kinetics.rs` above `#[cfg(test)]`:

```rust
//! Point reactor kinetics with six delayed neutron groups.

use crate::domain::DomainError;
use crate::params::{BETA, BETA_FRAC, GEN_TIME, LAMBDA_DN};

/// Largest substep, s. Resolves the fastest delayed group (lambda = 3.01 1/s) well.
pub const KIN_H_MAX: f64 = 1.0e-3;
/// Substep is limited so that h * (rho - BETA) / GEN_TIME <= this factor: an
/// ACCURACY bound (<= 5 % prompt growth per substep), not just denominator positivity.
/// The evidence is tests/numerics.rs::kinetics_matches_inhour_and_refined_reference.
pub const KIN_PROMPT_SAFETY: f64 = 0.05;
/// Supported reactivity ceiling above BETA, dk/k (a physical envelope, not a work
/// cap): growth rate 500/s; beyond this the fuel model is meaningless and f64
/// overflows within a plant step. There is deliberately no substep work cap: long
/// advances (e.g. 100 s at rho = 0) are legitimate and their cost is linear in dt.
pub const KIN_EXCESS_MAX: f64 = 0.02;
/// Floor for the period estimator only: inv_period contribution is 0 when the
/// starting population is below this, and max(n, KIN_N_FLOOR) is used inside the
/// log so the meter stays finite from zero power (baseline defect 12).
pub const KIN_N_FLOOR: f64 = 1e-12;
/// Relative-power ceiling. A substep candidate above it, or non-finite, is a
/// PromptSupercritical rejection with full rollback of n, c and inv_period.
pub const KIN_N_MAX: f64 = 1e6;

#[derive(Clone, Debug)]
pub struct Kinetics {
    /// Neutron population relative to rated power.
    pub n: f64,
    /// Delayed neutron precursor concentrations, same units as n.
    pub c: [f64; 6],
    /// Inverse period estimate, 1/s.
    pub inv_period: f64,
}

impl Kinetics {
    pub fn equilibrium(n: f64) -> Self {
        let mut c = [0.0; 6];
        for i in 0..6 {
            c[i] = BETA * BETA_FRAC[i] * n / (GEN_TIME * LAMBDA_DN[i]);
        }
        Self { n, c, inv_period: 0.0 }
    }

    /// One backward-Euler substep with constant reactivity `rho` (dk/k) and source `s`.
    /// Every candidate is checked before anything commits: Err leaves `self` untouched.
    fn step(&mut self, rho: f64, s: f64, dt: f64) -> Result<(), DomainError> {
        let n0 = self.n;
        let mut lhs = 1.0 - dt * (rho - BETA) / GEN_TIME;
        let mut rhs = n0 + dt * s;
        for i in 0..6 {
            let d = 1.0 + dt * LAMBDA_DN[i];
            lhs -= dt * LAMBDA_DN[i] * dt * BETA * BETA_FRAC[i] / GEN_TIME / d;
            rhs += dt * LAMBDA_DN[i] * self.c[i] / d;
        }
        if lhs <= 0.0 {
            return Err(DomainError::PromptSupercritical { rho });
        }
        let n1 = rhs / lhs;
        // Candidate admissibility: finite, non-negative, below the power ceiling.
        if !n1.is_finite() || n1 < 0.0 || n1 > KIN_N_MAX {
            return Err(DomainError::PromptSupercritical { rho });
        }
        let mut c1 = [0.0; 6];
        for i in 0..6 {
            c1[i] = (self.c[i] + dt * BETA * BETA_FRAC[i] / GEN_TIME * n1) / (1.0 + dt * LAMBDA_DN[i]);
            if !c1[i].is_finite() {
                return Err(DomainError::PromptSupercritical { rho });
            }
        }
        // Period meter: finite by construction (defect 12). No contribution while
        // the population is below the floor; log DIFFERENCES on floored values, never
        // a ratio that could overflow.
        let ip = if n0 < KIN_N_FLOOR {
            0.0
        } else {
            (n1.max(KIN_N_FLOOR).ln() - n0.max(KIN_N_FLOOR).ln()) / dt
        };
        // Smooth the period meter like a real log-rate amplifier.
        let a = (dt / 1.0).min(1.0);
        let inv_period = self.inv_period + (ip - self.inv_period) * a;
        if !inv_period.is_finite() {
            return Err(DomainError::PromptSupercritical { rho });
        }
        // Commit only after every candidate passed.
        self.n = n1;
        self.c = c1;
        self.inv_period = inv_period;
        Ok(())
    }

    /// Advance by `dt` with adaptive substeps:
    /// h = min(KIN_H_MAX, KIN_PROMPT_SAFETY * GEN_TIME / max(rho - BETA, 0));
    /// substeps = ceil(dt / h), with no work cap. Returns the number of substeps
    /// used, or Err with the ENTIRE entry state restored (transactional across
    /// substeps) when `dt` is not finite or not positive, `rho` or `s` is not finite,
    /// `rho - BETA > KIN_EXCESS_MAX`, an implicit denominator is nonpositive, or any
    /// substep candidate is non-finite or above KIN_N_MAX.
    pub fn advance(&mut self, rho: f64, s: f64, dt: f64) -> Result<usize, DomainError> {
        // Master dt rule and non-finite inputs rule, before anything is touched.
        if !(dt.is_finite() && dt > 0.0) {
            return Err(DomainError::PropertyOutOfRange { what: "dt".into(), value: dt });
        }
        if !rho.is_finite() {
            return Err(DomainError::PropertyOutOfRange { what: "kinetics rho".into(), value: rho });
        }
        if !s.is_finite() {
            return Err(DomainError::PropertyOutOfRange { what: "kinetics source".into(), value: s });
        }
        let excess = rho - BETA;
        if excess > KIN_EXCESS_MAX {
            return Err(DomainError::PromptSupercritical { rho });
        }
        let h_prompt = if excess > 0.0 { KIN_PROMPT_SAFETY * GEN_TIME / excess } else { f64::INFINITY };
        let h = KIN_H_MAX.min(h_prompt);
        let steps = (dt / h).ceil().max(1.0) as usize;
        let h = dt / steps as f64;
        // Transactional across substeps: a failure in substep k restores the state
        // at ENTRY, not the state after substep k-1.
        let saved = self.clone();
        for _ in 0..steps {
            if let Err(e) = self.step(rho, s, h) {
                *self = saved;
                return Err(e);
            }
        }
        Ok(steps)
    }
}
```

Update the unit tests in the same file (long advances are looped in 0.1 s calls to mirror plant use; there is no work cap, so the single 100 s call is also exercised by the integration test):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steady_state_stays_put() {
        let mut k = Kinetics::equilibrium(1.0);
        for _ in 0..1000 {
            k.advance(0.0, 0.0, 0.1).unwrap();
        }
        assert!((k.n - 1.0).abs() < 1e-6);
    }

    #[test]
    fn small_step_gives_textbook_period() {
        // +0.1 % dk/k (~17 cents): stable period should be ~57 s for U-235 data.
        let mut k = Kinetics::equilibrium(1.0);
        for _ in 0..1200 {
            k.advance(0.001, 0.0, 0.1).unwrap();
        }
        let n1 = k.n;
        for _ in 0..100 {
            k.advance(0.001, 0.0, 0.1).unwrap();
        }
        let period = 10.0 / (k.n / n1).ln();
        assert!(period > 40.0 && period < 75.0, "period {period}");
    }

    #[test]
    fn prompt_drop_on_scram() {
        let mut k = Kinetics::equilibrium(1.0);
        k.advance(-0.05, 0.0, 0.2).unwrap();
        // Prompt drop to roughly beta/(beta - rho) ~ 0.11.
        assert!(k.n > 0.06 && k.n < 0.15, "n = {}", k.n);
    }
}
```

- [ ] **Step 5: Update the caller in `reactor.rs` and `sim.rs`**

In `crates/bwr-core/src/reactor.rs` add `use crate::domain::DomainError;` and change `Core::step`:

Replace

```rust
    pub fn step(&mut self, dt: f64, coolant_t: f64, htc_factor: f64) -> f64 {
        self.rods.step(dt);
        let substeps = ((dt / 1.0e-3).ceil() as usize).max(1);
        self.kin.advance(self.rho_total, SOURCE, dt, substeps);
```

with

```rust
    pub fn step(&mut self, dt: f64, coolant_t: f64, htc_factor: f64) -> Result<f64, DomainError> {
        // Master dt rule and non-finite inputs rule: every argument, before any mutation.
        if !(dt.is_finite() && dt > 0.0) {
            return Err(DomainError::PropertyOutOfRange { what: "dt".into(), value: dt });
        }
        if !coolant_t.is_finite() {
            return Err(DomainError::PropertyOutOfRange { what: "coolant_t".into(), value: coolant_t });
        }
        if !htc_factor.is_finite() {
            return Err(DomainError::PropertyOutOfRange { what: "htc_factor".into(), value: htc_factor });
        }
        self.rods.step(dt);
        self.kin.advance(self.rho_total, SOURCE, dt)?;
```

and change the last line of that function from `q_conv + p * DIRECT_COOLANT_HEAT` to `Ok(q_conv + p * DIRECT_COOLANT_HEAT)`. (Task 4 removes the `self.rods.step(dt)` line; until then an invalid `dt` is rejected before the rods move, which is stricter than the final rule and harmless.)

The `?` on `kin.advance` sits BEFORE the xenon, decay-heat and fuel-temperature updates, which is exactly the master's rejection semantics: on Err nothing in `Core` beyond the rods commits. The test `core_step_err_leaves_xenon_decay_and_fuel_unadvanced` (Step 2) pins this; do not reorder those updates above the kinetics call. The `Sim` side of the rule (previous `q_core` kept, error drained at the end of the step with the post-step time stamp) gets its own test in Task 4, which can force the Err through `KIN_N_MAX`.

In `crates/bwr-core/src/sim.rs`, `Sim::step`, replace

```rust
        self.q_core = self.core.step(dt, t_cool, cov);
```

with

```rust
        // On a kinetics domain error the core state is left untouched for this
        // step and the previous heat input is reused. The error is HELD LOCALLY and
        // drained at the end of the step (master shared step order, item 17) so it
        // carries the same post-step time stamp as vessel and IC errors.
        let core_err = match self.core.step(dt, t_cool, cov) {
            Ok(q) => {
                self.q_core = q;
                None
            }
            Err(e) => Some(e),
        };
```

and at the END of `Sim::step`, after `self.protection();` (Task 4 moves this to its final position after `ledger.record`):

```rust
        // 17: drain the core error with the post-step time stamp.
        if let Some(e) = core_err {
            self.domain_error(e);
        }
```

- [ ] **Step 6: Run the tests**

Run: `cargo test --release -p bwr-core --test numerics kinetics_ -- --nocapture`
Expected: `kinetics_rejects_prompt_supercritical_instead_of_collapsing`, `kinetics_rejects_overflow_candidates_transactionally`, `kinetics_and_core_reject_invalid_step_arguments_unchanged` and `period_meter_is_finite_from_zero_power` pass. `kinetics_matches_inhour_and_refined_reference` prints one `INHOUR` line per sub-prompt case (expected inside 2 %) and one `PROMPT` line per prompt case. If a `PROMPT` line exceeds 2 % at `KIN_PROMPT_SAFETY = 0.05` (the accuracy analysis above predicts it will), apply the envelope-decision rule: lower `KIN_PROMPT_SAFETY` along 0.05 -> 0.02 -> 0.01 -> 0.005 -> 0.002, rerunning each time, until every `PROMPT` line is inside 2 %; never change the tolerance, the window, or the reference. Paste every `INHOUR`/`PROMPT` line (for every value tried) into the commit body; the final value is what Step 7 registers and what the master amendment states.

Run: `cargo test --release -p bwr-core`
Expected: all pass, including the three kinetics unit tests.

- [ ] **Step 7: Record the constants (register-row rule: same task as the constants)**

Append to `docs/model/parameters.csv`, using its header's column order (Package 1), one row each:

| id | value | unit | definition | status | notes |
|---|---|---|---|---|---|
| `KIN_H_MAX` | 1.0e-3 | s | largest kinetics substep | assumed | numerical; resolves lambda_6 = 3.01 1/s with h*lambda = 0.003 |
| `KIN_PROMPT_SAFETY` | 0.05 (or the lower value Step 6 settled on) | - | accuracy bound on h*(rho-beta)/Lambda, <= 5 % prompt growth per substep | assumed | numerical; evidence tests/numerics.rs kinetics_matches_inhour_and_refined_reference (2 % vs inhour and h/10 refinement); measured errors in the Task 3 commit body |
| `KIN_EXCESS_MAX` | 0.02 | dk/k | supported reactivity above beta | assumed | model envelope: growth rate 500/s; fuel model meaningless and f64 overflows within a plant step beyond it; rejected as DomainError (no work cap exists) |
| `KIN_N_FLOOR` | 1e-12 | - | period-estimator population floor | assumed | numerical; affects the period meter only, never `n` itself |
| `KIN_N_MAX` | 1e6 | - | relative-power ceiling for kinetics candidates | assumed | numerical; a candidate above it or non-finite is PromptSupercritical with full rollback |

- [ ] **Step 8: Commit**

```bash
git add crates/bwr-core/src/kinetics.rs crates/bwr-core/src/reactor.rs crates/bwr-core/src/sim.rs crates/bwr-core/tests/numerics.rs docs/model/parameters.csv
git commit -m "fix(kinetics): adaptive substeps, finite period meter, prompt-supercritical rejection

Baseline clamped n to 1e-14 once rho exceeded ~0.046 dk/k at the fixed 1 ms
substep, and the period meter went non-finite from zero power (defect 12):
<paste both printed lines from Step 1>. The solver now shortens the substep
so h*(rho-beta)/Lambda <= KIN_PROMPT_SAFETY (an accuracy bound, no work cap),
rejects rho - beta > 0.02 dk/k and any candidate above KIN_N_MAX = 1e6 or
non-finite as DomainError::PromptSupercritical with the entire entry state
restored, validates dt/rho/source, and floors the period estimator at
KIN_N_FLOOR using log differences.

Accuracy evidence (2 % vs inhour and h/10 refinement):
<paste every INHOUR/PROMPT line from Step 6, for every KIN_PROMPT_SAFETY tried>
<if KIN_PROMPT_SAFETY differs from 0.05: "master amendment: KIN_PROMPT_SAFETY = <value>">

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 4: One-step rod reactivity lag and the transfer-allocation step order

**Files:**
- Modify: `crates/bwr-core/src/reactor.rs` (`Core::step_rods`, `Core::step`)
- Modify: `crates/bwr-core/src/sim.rs` (`Sim::step` restructured to the master order)
- Modify: `crates/bwr-core/tests/numerics.rs`

**Interfaces:**
- Consumes: `Sim::last_flows()` storage and `ledger.record` placement from Package 2.
- Produces: `pub fn Core::step_rods(&mut self, dt: f64)` (void; an invalid `dt` sets `Core.last_error` and moves nothing) and `Core.last_error: Option<DomainError>`. `Core::step` no longer moves rods. `Sim::step` validates `dt` first (void; records `PropertyOutOfRange { what: "dt" }` itself and returns without advancing) and implements the P3 subset of the master's shared step order: `core.step_rods` → `core.update_reactivity` → `core.step` (Err held locally, previous `q_core` kept) → steam valves and REQUESTED flows → IC (request from Task 5) → feedwater → `vessel.step` (the transfer-allocation point) → scale consumers to APPLIED flows (IC commit from Task 5) → `turbine.step` → `cond.step` with applied steam, RWCU and FEED (`flows.w_feed` for withdrawal and heating) → `time += dt` → `protection()` → `ledger.record` → 17: drain core / vessel (Task 6) / IC (Task 5) errors with the same post-step time stamp.

**Baseline defects (verified in code; master defects 3 and 13):**
- Defect 3: `Sim::step` calls `self.core.update_reactivity(...)` first, then `self.core.step(...)`, which moves the rods and then advances kinetics with the `rho_total` computed from the *previous* positions. Every rod motion, including scram, acts on power one step (0.05 s) late.
- Defect 13: the turbine and condenser consume the REQUESTED steam and reject flows (`self.steam.w_tcv`, `self.fw.rwcu_reject`) computed before the vessel step; when the vessel cannot supply the request (subcooled gating today, inventory bounding after Task 6), mass appears downstream from nowhere.

- [ ] **Step 1: Write the failing test and the lag measurement**

Append to `tests/numerics.rs`:

```rust
use bwr_core::sim::DT;

#[test]
fn rod_reactivity_has_no_one_step_lag() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    sim.apply(Command::ManualScram);
    sim.step(DT);
    let fresh = sim.core.rods.reactivity();
    assert!(
        (sim.core.rho_rods - fresh).abs() < 1e-12,
        "rho_rods used by kinetics {} but rods are at {}",
        sim.core.rho_rods,
        fresh
    );
}

#[test]
fn scram_motion_per_step_is_quantified() {
    // Documents the size of the baseline lag: one step of hydraulic insertion.
    let mut sim = Sim::new(ScenarioName::FullPower);
    sim.apply(Command::ManualScram);
    let pos0: Vec<f64> = sim.core.rods.pos.clone();
    let rho0 = sim.core.rods.reactivity();
    sim.step(DT);
    let moved = pos0[0] - sim.core.rods.pos[0];
    let drho = sim.core.rods.reactivity() - rho0;
    println!("one scram step moves {moved:.3} positions, worth {:.1} pcm", drho * 1e5);
    assert!((moved - ROD_FULL_OUT / SCRAM_TIME * DT).abs() < 1e-9, "moved {moved}");
    assert!(drho < 0.0);
}

#[test]
fn core_error_keeps_previous_q_core_and_is_drained_post_step() {
    // Master step order item 4 and 17: on Core::step Err the previous q_core is kept
    // and the error is drained at the END of the step, stamped with the post-step time.
    use bwr_core::kinetics::KIN_N_MAX;
    let mut sim = Sim::new(ScenarioName::FullPower);
    sim.step(DT);
    assert!(sim.domain_errors.is_empty());
    let q_before = sim.q_core;
    let n_forced = 2.0 * KIN_N_MAX; // every kinetics candidate is above the ceiling: Err
    sim.core.kin.n = n_forced;
    sim.step(DT);
    assert_eq!(sim.q_core, q_before, "previous q_core kept on a core error");
    assert_eq!(sim.core.kin.n, n_forced, "kinetics not advanced on Err");
    let (t, e) = &sim.domain_errors[0];
    assert!(matches!(e, DomainError::PromptSupercritical { .. }), "{e:?}");
    assert!((t - sim.time).abs() < 1e-12, "core error stamped {t} but the step ended at {}", sim.time);
    assert!((sim.time - 2.0 * DT).abs() < 1e-9, "time advanced despite the error");
}

#[test]
fn step_rods_and_sim_step_reject_invalid_dt_without_moving() {
    // Void-returning steps under the master dt rule: Core::step_rods sets last_error,
    // Sim::step records the error itself; neither advances anything.
    use bwr_core::reactor::Core;
    for dt in [f64::NAN, f64::INFINITY, 0.0, -0.05] {
        let mut core = Core::new(1.0, ROD_FULL_OUT, 550.0);
        core.rods.scramming = true;
        let pos = core.rods.pos.clone();
        core.step_rods(dt);
        assert_eq!(core.rods.pos, pos, "dt {dt}: rods moved");
        assert!(matches!(core.last_error.take(), Some(DomainError::PropertyOutOfRange { what, .. }) if what == "dt"), "dt {dt}");

        let mut sim = Sim::new(ScenarioName::FullPower);
        let (m0, e0, n0) = (sim.vessel.mass, sim.vessel.energy, sim.core.kin.n);
        sim.step(dt);
        assert_eq!(sim.time, 0.0, "dt {dt}: time advanced");
        assert_eq!((sim.vessel.mass, sim.vessel.energy, sim.core.kin.n), (m0, e0, n0), "dt {dt}: state advanced");
        assert!(matches!(&sim.domain_errors[..], [(_, DomainError::PropertyOutOfRange { what, .. })] if what == "dt"), "dt {dt}: {:?}", sim.domain_errors);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --release -p bwr-core --test numerics rod_reactivity_has_no_one_step_lag -- --nocapture`
Expected: FAIL with `rho_rods used by kinetics <x> but rods are at <y>` where `|x - y|` is on the order of 1e-3 dk/k (the worth of 0.8 positions on every rod). Paste the message into the commit body.

- [ ] **Step 3: Implement the reorder and the allocation step**

In `crates/bwr-core/src/reactor.rs`, add to `pub struct Core` after `pub fuel_t: f64,` (or the last existing field):

```rust
    /// Error raised by the void-returning `step_rods` (master dt rule); drained by `Sim::step` at step 17.
    pub last_error: Option<DomainError>,
```

initialise it with `last_error: None,` in `Core::new`, and add before `pub fn step(`:

```rust
    /// Move the rods by `dt`. Called by `Sim::step` before `update_reactivity` so the
    /// kinetics see the positions of this step, not the previous one. Void-returning:
    /// an invalid `dt` sets `last_error` (master dt rule) and moves nothing.
    pub fn step_rods(&mut self, dt: f64) {
        if !(dt.is_finite() && dt > 0.0) {
            self.last_error = Some(DomainError::PropertyOutOfRange { what: "dt".into(), value: dt });
            return;
        }
        self.rods.step(dt);
    }
```

and delete the line `self.rods.step(dt);` from `Core::step`.

In `crates/bwr-core/src/sim.rs`, replace the body of `Sim::step` with the master's P3-subset order. The turbine and condenser move BELOW the vessel step and consume applied flows:

```rust
    /// One physics step (master shared step order, P3 subset).
    pub fn step(&mut self, dt: f64) {
        // Master dt rule. Sim::step is void and Sim owns the error channel, so the
        // error is recorded here and nothing advances.
        if !(dt.is_finite() && dt > 0.0) {
            self.domain_error(DomainError::PropertyOutOfRange { what: "dt".into(), value: dt });
            return;
        }
        let p = self.vessel.eos.p;
        let t_cool = self.vessel.eos.t;

        // 1,3,4: rods move first so reactivity matches this step's positions.
        self.core.step_rods(dt);
        self.core.update_reactivity(self.vessel.alpha_core, t_cool);
        let cov = self.vessel.core_coverage();
        // On Err the previous q_core is kept and the error is HELD until step 17.
        let core_err = match self.core.step(dt, t_cool, cov) {
            Ok(q) => {
                self.q_core = q;
                None
            }
            Err(e) => Some(e),
        };

        // 5: steam valve motion and REQUESTED steam flows.
        let demand = self.steam.pressure_demand(p);
        let tcv_demand = self.turbine.tcv_demand(dt, demand);
        let byp_ok = self.cond.available_for_bypass();
        self.steam.step(dt, p, tcv_demand, byp_ok, self.turbine.tripped);
        self.steam.compute_flows(if self.vessel.eos.subcooled { P_ATM } else { p }, !self.turbine.tripped);

        // 6: isolation condensers. Until Task 5 this is the baseline single-call form
        // (the pool commits before the vessel step); Task 5 splits it into a pure
        // `request` here and a `commit` at step 9 with the APPLIED flows.
        let steam_avail = !self.vessel.eos.subcooled;
        let q_ic: f64 = self.ic.iter_mut().map(|t| t.step(dt, t_cool, steam_avail)).sum();

        // 7: feedwater (level setpoint setdown after a scram).
        self.fw.setdown_cm = if self.scrammed { -100.0 } else { 0.0 };
        let hotwell_ok = self.cond.hotwell_mass > 0.05 * HOTWELL_CAPACITY;
        let msg = self.fw.step(
            dt,
            p,
            self.level_cm(),
            self.steam.total_flow(),
            self.steam.w_tcv,
            hotwell_ok,
            self.cond.hotwell_t(),
        );
        if let Some(m) = msg {
            self.event("fw_trip", m);
        }

        // 8: vessel — the transfer-allocation point; all outflows are bounded here.
        let requested_steam = self.steam.total_flow();
        let inputs = VesselInputs {
            q_core: self.q_core,
            q_ic,
            w_feed: self.fw.flow,
            h_feed: self.fw.h_feed(),
            w_steam: requested_steam,
            w_liquid_out: self.fw.rwcu_reject,
        };
        let flows = self.vessel.step(dt, &inputs);

        // 9: allocation — every downstream consumer sees APPLIED transfers.
        // The ratio is 1.0 whenever inventory is adequate (it differs only in a
        // step that raises InventoryExhausted, or under subcooled steam gating) and
        // 0.0 when the vessel rejected the step (flows == BoundaryFlows::default()).
        let steam_ratio = if requested_steam > 1e-12 { flows.w_steam / requested_steam } else { 0.0 };
        self.steam.w_tcv *= steam_ratio;
        self.steam.w_bypass *= steam_ratio;
        // fw.rwcu_reject and fw.flow are operator setpoint / controller state and are
        // never mutated here; the applied reject and feed flows are flows.w_liquid_out
        // and flows.w_feed, used below and by the ledger. (Task 5 adds the IC commit here.)

        // 11: turbine shaft dynamics with the applied TCV flow.
        let p_cond = self.cond.pressure();
        self.turbine.step(dt, self.steam.w_tcv, self.steam.p_chest, p_cond);

        // 12: condenser with applied flows only: steam, RWCU reject AND feed. A
        // rejected vessel step (zero applied feed) withdraws nothing from the hotwell
        // and heats nothing, even though fw.flow still holds the controller's request.
        let h_steam = sat_p(self.steam.p_chest.max(P_ATM)).h_g;
        let h_hw = steam::h_liquid(self.cond.hotwell_t());
        let w_in = self.steam.w_tcv + self.steam.w_bypass + flows.w_liquid_out;
        let fw_heating = flows.w_feed * (flows.h_feed - h_hw).max(0.0);
        let q_cond = self.steam.w_tcv * h_steam - self.turbine.p_mech + self.steam.w_bypass * h_steam
            + flows.w_liquid_out * flows.h_liquid_out
            - w_in * h_hw
            - fw_heating;
        self.cond.step(dt, q_cond, w_in, flows.w_feed);

        // 14-16: clock, protection, bookkeeping.
        self.time += dt;
        self.protection();
        self.ledger.record(dt, &flows);
        self.last_flows = flows; // Package 2 field behind Sim::last_flows()

        // 17: drain every error raised this step, all stamped with the same post-step
        // time. Task 5 adds the IC channels and Task 6 the vessel channels here.
        if let Some(e) = self.core.last_error.take() {
            self.domain_error(e);
        }
        if let Some(e) = core_err {
            self.domain_error(e);
        }
    }
```

This is the Package 2 body with five changes: the `dt` guard, the core block at the top (rods first, `Result` held locally), `turbine.step`/`cond.step` moved below `vessel.step`, the allocation of applied flows between them (including `flows.w_feed` for the condenser withdrawal and feed heating), and the step-17 drain. If Package 2 named its `last_flows` storage differently, keep its name; `Sim::last_flows()` is the accessor.

`flows.h_liquid_out` replaces the baseline's `self.fw.rwcu_reject * self.vessel.sat().h_f` term so the condenser receives exactly the enthalpy the vessel released; `flows.w_feed`/`flows.h_feed` replace `self.fw.flow`/`self.fw.h_feed()` so the hotwell gives up exactly the feed the vessel accepted.

- [ ] **Step 4: Add the applied-flows invariant test**

Append to `tests/numerics.rs`. This invariant holds trivially at steady full power (ratio 1.0); it bites, and its failing-first demonstration lives, in Task 6's coupled regressions (`coupled_run_hotwell_gain_equals_vessel_release`, `eos_rejection_leaves_hotwell_and_pool_unchanged`). It is still asserted here so any later reorder breaks loudly:

```rust
#[test]
fn downstream_consumers_see_applied_flows() {
    for scenario in [ScenarioName::FullPower, ScenarioName::HotStandby] {
        let mut sim = Sim::new(scenario);
        for _ in 0..(10.0 / DT) as usize {
            sim.step(DT);
            let f = sim.last_flows();
            assert!(
                (sim.steam.w_tcv + sim.steam.w_bypass - f.w_steam).abs() < 1e-9,
                "{scenario:?}: turbine+bypass {} but vessel applied {}",
                sim.steam.w_tcv + sim.steam.w_bypass,
                f.w_steam
            );
        }
    }
}
```

- [ ] **Step 5: Run the tests**

Run: `cargo test --release -p bwr-core`
Expected: all pass, including `core_error_keeps_previous_q_core_and_is_drained_post_step` and `step_rods_and_sim_step_reject_invalid_dt_without_moving`. `turbine_trip_rides_through_on_bypass` and `manual_scram_shuts_down_without_isolation` still pass (insertion acts 0.05 s earlier, inside their tolerances). `full_power_is_steady` still passes: at steady state the allocation ratio is 1.0 and the turbine sees the same flow one phase later in the step, which only re-times `p_mech` by one step. If net MWe drifts outside the existing 285-315 band, that is a wiring mistake in the reorder, not a tolerance problem.

- [ ] **Step 6: Commit**

```bash
git add crates/bwr-core/src/reactor.rs crates/bwr-core/src/sim.rs crates/bwr-core/tests/numerics.rs
git commit -m "fix(core): rods move before reactivity; consumers get applied transfers

update_reactivity ran before rods moved, so kinetics used positions from
the previous step (0.8 positions per step during scram). Baseline:
<paste failure message from Step 2>

Sim::step now follows the master shared order: vessel.step is the
transfer-allocation point and turbine/condenser consume the applied
steam, RWCU and feed flows (defect 13), so a vessel-side bound can never
create mass downstream. Core errors are held and drained at step 17 with
the post-step time; Sim::step and Core::step_rods reject an invalid dt.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 5: Isolation condenser pool energy accounting

**Files:**
- Modify: `crates/bwr-core/src/ic.rs`
- Modify: `crates/bwr-core/src/sim.rs` (`IcTrain::request` at step 6, `commit` at step 9, `Sim.ic_boundary`, step-17 drain, `finalize_init`)
- Modify: `crates/bwr-core/tests/numerics.rs` (coupled vessel-side / pool-side identity)
- Modify: `docs/model/parameters.csv`

**Interfaces:**
- Consumes: `sat_p(p).h_fg()`, `sat_p(p).h_f`, `Vessel.eos.x`, `BoundaryFlows.q_ic`, `Ledger.integrated["ic_j"]`, `DomainError::PropertyOutOfRange`.
- Produces (master r2 interim form, verbatim names): `pub struct IcRequest { w_steam_in, w_condensate_return, h_condensate_return }` (P3: `q/h_fg`, the same value, `h_f`) with `IcRequest::scaled(ratio)`; `pub struct IcStep { q_removed, w_steam_in, w_condensate_return, h_condensate_return, energy_to_pool, pool_boiloff_kg, pool_cooling_w }` (seven fields); `IcTrain::request(&self, dt, p_vessel, w_steam_available) -> IcRequest` (pure); `IcTrain::commit(&mut self, dt, p_vessel, applied: IcRequest) -> IcStep` (the ONLY state change; `applied` = request x vessel ratio, zero on EOS rejection); `IcTrain.last_error: Option<DomainError>`; `IcBoundary { q_removed_w, energy_to_pool_j, pool_cooling_j, pool_boiloff_kg, condensate_return_kg }` and `Sim.ic_boundary` (every `IcStep` field integrated as a boundary term; Package 6 reads `q_removed_w` into `EnergyBalance.q_ic_w`); constants `CP_WATER = 4186.0`, `H_FG_ATM = 2.257e6`, `POOL_BOIL_T = 100.0`, `POOL_IDLE_COOLING_K_PER_S = 0.0005`. The single-call `IcTrain::step` is REMOVED (master: `request`/`commit` replace it).
- Rejection (master dt rule): `commit` validates `dt`, `p_vessel` and every `applied` field before any mutation; on failure it sets `last_error = Some(PropertyOutOfRange { what, value })`, returns `IcStep::default()` and leaves the train unchanged. `request` is pure and cannot report, so an invalid argument yields `IcRequest::default()`; `Sim` passes the same `dt`/`p_vessel` to `commit`, which reports it.
- Sim wiring: step 6 `requests[i] = ic[i].request(dt, p, w_steam_available)` with `w_steam_available = 0.0` when the vessel is subcooled, else the vessel's vapour inventory rate `(vessel.mass * vessel.eos.x / dt).max(0.0)` (Package 7 replaces this hint with its allocation); `VesselInputs.q_ic = sum(requests[i].w_steam_in * h_fg)`; step 9 `ratio = flows.q_ic / q_ic_requested` (0.0 when nothing was requested or the vessel rejected the step) and `ic[i].commit(dt, p, requests[i].scaled(ratio))`, accumulated into `Sim.ic_boundary`; step 17 drains every `ic[i].last_error`. Identity (no tube storage before Package 7): `flows.q_ic * dt == sum(IcStep.energy_to_pool)` every step, so `Ledger.integrated["ic_j"] == ic_boundary.energy_to_pool_j`.

**Baseline defect (verified in code):** `pool_t += q/(m cp)` can overshoot 100 degC and the excess sensible energy vanishes; `pool_mass = (pool_mass - q/h_fg).max(0.0)` discards energy at depletion; the lagged `heat` keeps reporting removal for ~20 s after the pool is gone; idle cooling `pool_t -= 0.0005*dt` removes energy with no boundary record; and the pool commits `q_ic` BEFORE the vessel step, so a rejected vessel step still adds energy to the pool (review cycle 2, comment 9).

- [ ] **Step 1: Write the failing unit tests**

Append to the `#[cfg(test)] mod tests` in `crates/bwr-core/src/ic.rs` (create the module if absent):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::DomainError;

    const P: f64 = P_RATED;

    /// request + commit at ratio 1.0: what Sim does when inventory is adequate.
    fn step(t: &mut IcTrain, dt: f64, p: f64) -> IcStep {
        let req = t.request(dt, p, 1.0e9);
        t.commit(dt, p, req)
    }

    fn loaded(pool_t: f64, pool_mass: f64, heat: f64) -> IcTrain {
        let mut t = IcTrain::new();
        t.cmd_open = true;
        t.valve_pos = 1.0;
        t.pool_t = pool_t;
        t.pool_mass = pool_mass;
        t.heat = heat;
        t
    }

    fn stored_j(t: &IcTrain) -> f64 {
        t.pool_mass * CP_WATER * t.pool_t
    }

    #[test]
    fn request_is_pure_and_commit_is_the_only_state_change() {
        let t = loaded(60.0, 1000.0, 30.0e6);
        let before = format!("{t:?}");
        let req = t.request(1.0, P, 1.0e9);
        assert_eq!(format!("{t:?}"), before, "request mutated the train");
        assert!(req.w_steam_in > 0.0);
        assert_eq!(req.w_condensate_return, req.w_steam_in, "P3 interim: no tube storage");
        assert!((req.h_condensate_return - sat_p(P).h_f).abs() < 1e-6);
        assert_eq!(t.request(1.0, P, 1.0e9), req, "request is deterministic");
    }

    #[test]
    fn commit_uses_the_applied_request_not_its_own() {
        // Zero applied (the vessel rejected the step): nothing reaches the pool and
        // q_removed is zero, even though the train's own request was 30 MW.
        let mut t = loaded(60.0, 1000.0, 30.0e6);
        let (m0, t0) = (t.pool_mass, t.pool_t);
        let out = t.commit(1.0, P, IcRequest::default());
        assert_eq!(out.q_removed, 0.0);
        assert_eq!(out.energy_to_pool, 0.0);
        assert_eq!(out.pool_boiloff_kg, 0.0);
        assert_eq!(out.w_condensate_return, 0.0);
        assert_eq!(t.pool_mass, m0);
        // With no duty the pool is idle, so only its own cooling system acts (a boundary term).
        assert!((t.pool_t - (t0 - POOL_IDLE_COOLING_K_PER_S)).abs() < 1e-9);
        assert!((out.pool_cooling_w - POOL_IDLE_COOLING_K_PER_S * m0 * CP_WATER).abs() < 1.0);

        // Half applied: the pool sees half, q_removed is the applied value.
        let mut t = loaded(60.0, 1000.0, 30.0e6);
        let req = t.request(1.0, P, 1.0e9);
        let out = t.commit(1.0, P, req.scaled(0.5));
        let h_fg = sat_p(P).h_fg();
        assert!((out.q_removed - 0.5 * req.w_steam_in * h_fg).abs() < 1.0, "{out:?}");
        assert!((out.energy_to_pool - out.q_removed * 1.0).abs() < 1.0);
        assert!((out.w_condensate_return - 0.5 * req.w_condensate_return).abs() < 1e-9);
        assert!((t.heat - out.q_removed).abs() < 1e-6, "the tube lag state follows the applied duty");
    }

    #[test]
    fn pool_never_overshoots_boiling_and_energy_closes() {
        let mut t = loaded(99.99, 1000.0, 30.0e6);
        let (m0, t0) = (t.pool_mass, t.pool_t);
        let out = step(&mut t, 1.0, P);
        assert!(t.pool_t <= POOL_BOIL_T + 1e-9, "pool overshot to {}", t.pool_t);
        let sensible = m0 * CP_WATER * (t.pool_t - t0);
        let latent = out.pool_boiloff_kg * H_FG_ATM;
        assert!((out.energy_to_pool - (sensible + latent)).abs() < 1.0, "energy_to_pool {} vs {}", out.energy_to_pool, sensible + latent);
        assert!((out.q_removed * 1.0 - out.energy_to_pool).abs() < 1.0);
        assert!(out.pool_boiloff_kg > 0.0);
        assert!((t.pool_mass - (m0 - out.pool_boiloff_kg)).abs() < 1e-9);
        assert!((out.w_condensate_return - out.q_removed / sat_p(P).h_fg()).abs() < 1e-9);
        assert_eq!(out.pool_cooling_w, 0.0, "no idle cooling while loaded");
    }

    #[test]
    fn depleted_pool_removes_no_heat() {
        let mut t = loaded(POOL_BOIL_T, 0.0, 10.0e6); // lagged duty left over from before depletion
        let req = t.request(1.0, P, 1.0e9);
        assert_eq!(req, IcRequest { w_steam_in: 0.0, w_condensate_return: 0.0, h_condensate_return: sat_p(P).h_f });
        let out = t.commit(1.0, P, req);
        assert_eq!(out.q_removed, 0.0);
        assert_eq!(out.pool_boiloff_kg, 0.0);
        assert_eq!(out.energy_to_pool, 0.0);
        assert_eq!(t.heat, 0.0);
        assert!(t.pool_mass >= 0.0);
    }

    #[test]
    fn boiloff_is_bounded_by_remaining_pool() {
        let mut t = loaded(POOL_BOIL_T, 1.0, 30.0e6); // 1 kg left: at most 2.257 MJ can be absorbed
        let out = step(&mut t, 1.0, P);
        assert!(out.pool_boiloff_kg <= 1.0 + 1e-12);
        assert!(out.energy_to_pool <= H_FG_ATM + 1.0);
        assert!(out.q_removed * 1.0 <= H_FG_ATM + 1.0, "the request itself is sink-limited");
        assert!(t.pool_mass >= 0.0);
    }

    #[test]
    fn idle_pool_cooling_is_reported() {
        let mut t = IcTrain::new();
        t.pool_t = 80.0;
        t.heat = 0.0;
        let out = step(&mut t, 1.0, P);
        let expected_w = POOL_IDLE_COOLING_K_PER_S * IC_POOL_MASS * CP_WATER;
        assert!((out.pool_cooling_w - expected_w).abs() < 1.0);
        assert!((t.pool_t - (80.0 - POOL_IDLE_COOLING_K_PER_S)).abs() < 1e-9);
    }

    #[test]
    fn pool_stored_energy_closes_against_applied_heat_cooling_and_discharge() {
        // Pool stored energy (reference 0 degC) changes by exactly energy_to_pool minus
        // idle cooling minus the enthalpy carried away by boiled-off steam
        // (cp*100 + h_fg; the latent part is inside energy_to_pool). States: heated
        // and idle-cooled at once (mixed), boiling, idle only, and depleted this step.
        for (pool_t, pool_mass, heat) in [(80.0, 1000.0, 5.0e4), (99.99, 1000.0, 30.0e6), (80.0, 1000.0, 0.0), (60.0, 1.0, 30.0e6)] {
            let mut t = loaded(pool_t, pool_mass, heat);
            let e0 = stored_j(&t);
            let out = step(&mut t, 1.0, P);
            let discharge = out.pool_boiloff_kg * (CP_WATER * POOL_BOIL_T + H_FG_ATM);
            let expected = e0 + out.energy_to_pool - out.pool_cooling_w * 1.0 - discharge;
            assert!(
                (stored_j(&t) - expected).abs() < 1.0,
                "state ({pool_t}, {pool_mass}, {heat}): stored {} expected {expected}; {out:?}",
                stored_j(&t)
            );
            assert!((out.energy_to_pool - out.q_removed * 1.0).abs() <= 1e-6 * out.energy_to_pool.max(1.0), "{out:?}");
        }
        // The mixed case really is mixed: heat absorbed AND idle cooling in the same step.
        let mut t = loaded(80.0, 1000.0, 5.0e4);
        let out = step(&mut t, 1.0, P);
        assert!(out.energy_to_pool > 0.0 && out.pool_cooling_w > 0.0, "{out:?}");
    }

    #[test]
    fn commit_rejects_invalid_arguments_unchanged() {
        // Master dt rule and non-finite inputs rule, table-driven, for the void-style
        // (last_error) reporting path of commit; request gives a zero request for the
        // same invalid dt / p_vessel because it is pure.
        let base = loaded(60.0, 1000.0, 30.0e6);
        let good = base.request(1.0, P, 1.0e9);
        let bad = [f64::NAN, f64::INFINITY, f64::NEG_INFINITY];
        let mut cases: Vec<(&str, f64, f64, IcRequest)> = vec![("dt", 0.0, P, good), ("dt", -1.0, P, good)];
        for &v in &bad {
            cases.push(("dt", v, P, good));
            cases.push(("p_vessel", 1.0, v, good));
            cases.push(("applied w_steam_in", 1.0, P, IcRequest { w_steam_in: v, ..good }));
            cases.push(("applied w_condensate_return", 1.0, P, IcRequest { w_condensate_return: v, ..good }));
            cases.push(("applied h_condensate_return", 1.0, P, IcRequest { h_condensate_return: v, ..good }));
        }
        for (what, dt, p, applied) in cases {
            let mut t = base.clone();
            let out = t.commit(dt, p, applied);
            assert_eq!(out, IcStep::default(), "{what}");
            assert!(
                matches!(t.last_error.take(), Some(DomainError::PropertyOutOfRange { what: w, .. }) if w == what),
                "{what}: wrong or missing last_error"
            );
            assert_eq!(format!("{t:?}"), format!("{base:?}"), "{what}: state changed");
            if what == "dt" || what == "p_vessel" {
                assert_eq!(base.request(dt, p, 1.0e9), IcRequest::default(), "{what}: request must be zero");
            }
        }
        assert_eq!(base.request(1.0, P, f64::NAN), IcRequest::default());
    }
}
```

Append to `tests/numerics.rs` the coupled identity (vessel side equals pool side, every step, and `Sim` integrates every `IcStep` field):

```rust
#[test]
fn ic_pool_energy_equals_vessel_side_removal_every_step() {
    // Master: flows.q_ic (vessel side, ledger) == sum(IcStep.q_removed) (pool side);
    // with no tube storage before Package 7 the identity is exact every step.
    let mut sim = Sim::new(ScenarioName::HotStandby);
    sim.apply(Command::SetIcTrain { index: 0, open: true });
    for _ in 0..(120.0 / DT) as usize {
        let e_before = sim.ic_boundary.energy_to_pool_j;
        sim.step(DT);
        let f = sim.last_flows();
        let pool_side = sim.ic_boundary.energy_to_pool_j - e_before;
        assert!((f.q_ic * DT - pool_side).abs() <= 1e-6 * (f.q_ic * DT).abs() + 1e-3, "vessel {} J vs pool {pool_side} J", f.q_ic * DT);
        assert!((f.q_ic - sim.ic_boundary.q_removed_w).abs() <= 1e-6 * f.q_ic.abs() + 1e-6);
    }
    assert!(sim.ic_boundary.energy_to_pool_j > 0.0, "the IC must have removed heat");
    assert!(sim.ic_boundary.condensate_return_kg > 0.0);
    let ledger_ic = sim.ledger.integrated["ic_j"];
    assert!((ledger_ic - sim.ic_boundary.energy_to_pool_j).abs() <= 1e-6 * ledger_ic, "ledger {ledger_ic} vs pools {}", sim.ic_boundary.energy_to_pool_j);
    assert!(sim.domain_errors.is_empty(), "{:?}", sim.domain_errors);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --release -p bwr-core ic::tests`
Expected: compile errors (`POOL_BOIL_T` not found, no method `request`/`commit`, no field `pool_boiloff_kg`, no field `last_error`). To demonstrate the overshoot on the baseline, temporarily run only this snippet in a scratch test with the old signature: `t.step(1.0, 287.0, true); println!("{}", t.pool_t)` with the `loaded(99.99, 1000.0, 30.0e6)` state. Expected printed value above 100 (about 106.8). Paste it into the commit body, then remove the scratch test.

- [ ] **Step 3: Implement**

Replace everything in `crates/bwr-core/src/ic.rs` above `impl Default for IcTrain`:

```rust
//! Isolation condenser system: passive decay heat removal and overpressure protection.
//!
//! Each train condenses reactor steam in tubes submerged in a water pool above
//! containment; condensate drains back to the vessel by gravity once the
//! condensate return valve opens. No pumps or AC power are needed.
//!
//! Request/commit (master interim form): `request` is pure and states what the
//! train would take from the vessel this step; the vessel allocates; `commit`
//! applies the ratio-scaled request and is the only state change, so a rejected
//! vessel step never adds energy to a pool. Pool energy is conserved: heat first
//! raises the pool to boiling, the remainder boils water off, and the request is
//! limited to what the remaining pool can absorb. Package 7 extends the same shape
//! with steam and return valves, tube inventory and the vessel mass ports.

use crate::domain::DomainError;
use crate::params::*;
use crate::steam::sat_p;

pub const CP_WATER: f64 = 4186.0;
/// Latent heat at atmospheric pressure (pool boils at 100 degC), J/kg.
pub const H_FG_ATM: f64 = 2.257e6;
pub const POOL_BOIL_T: f64 = 100.0;
/// Idle cooling of a hot pool by its (unmodelled) cooling system, K/s. ASSUMED;
/// reported as `IcStep::pool_cooling_w` so it is a declared boundary flow.
pub const POOL_IDLE_COOLING_K_PER_S: f64 = 0.0005;
/// Tube duty below which the pool counts as idle (baseline value, unchanged).
const IDLE_DUTY_W: f64 = 1.0e5;

/// Heat transfer UA per train, W/K (rated duty at 7.2 MPa with a boiling pool).
fn ua() -> f64 {
    IC_TRAIN_RATED_W / (sat_p(P_RATED).t - 100.0)
}

/// What one train asks the vessel for this step. Pure output of `request`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct IcRequest {
    /// Steam withdrawn from the vessel, kg/s (= q / h_fg at vessel pressure).
    pub w_steam_in: f64,
    /// Condensate returned, kg/s (equal to `w_steam_in` before Package 7: no tube storage).
    pub w_condensate_return: f64,
    /// Return enthalpy, J/kg (saturated liquid at vessel pressure before Package 7).
    pub h_condensate_return: f64,
}

impl IcRequest {
    /// The request after the vessel's allocation: ratio 1.0 when inventory was
    /// adequate, 0.0 when the vessel applied nothing (EOS rejection).
    pub fn scaled(self, ratio: f64) -> Self {
        Self {
            w_steam_in: self.w_steam_in * ratio,
            w_condensate_return: self.w_condensate_return * ratio,
            h_condensate_return: self.h_condensate_return,
        }
    }
}

/// What one train did in one step (master interim form).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct IcStep {
    /// Heat removed from the vessel, W (the APPLIED duty).
    pub q_removed: f64,
    pub w_steam_in: f64,
    pub w_condensate_return: f64,
    pub h_condensate_return: f64,
    /// Energy absorbed by the pool this step, J (sensible + latent). Equals q_removed * dt.
    pub energy_to_pool: f64,
    pub pool_boiloff_kg: f64,
    /// Energy removed from the pool by its idle cooling system, W.
    pub pool_cooling_w: f64,
}

/// Every `IcStep` field integrated over the run, kept on `Sim` as boundary terms
/// (master: "Sim integrates every IcStep field"). `q_removed_w` is the last step's
/// total (Package 6 reads it into `EnergyBalance.q_ic_w`); the rest accumulate.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct IcBoundary {
    pub q_removed_w: f64,
    pub energy_to_pool_j: f64,
    pub pool_cooling_j: f64,
    pub pool_boiloff_kg: f64,
    pub condensate_return_kg: f64,
}

impl IcBoundary {
    pub fn add(&mut self, s: &IcStep, dt: f64) {
        self.q_removed_w += s.q_removed;
        self.energy_to_pool_j += s.energy_to_pool;
        self.pool_cooling_j += s.pool_cooling_w * dt;
        self.pool_boiloff_kg += s.pool_boiloff_kg;
        self.condensate_return_kg += s.w_condensate_return * dt;
    }
}

#[derive(Clone, Debug)]
pub struct IcTrain {
    pub valve_pos: f64,
    pub cmd_open: bool,
    /// Applied tube duty of the last step, W (also the lag state for the next request).
    pub heat: f64,
    pub pool_t: f64,
    pub pool_mass: f64,
    /// Error raised by `commit` (master dt rule); drained by `Sim::step` at step 17.
    pub last_error: Option<DomainError>,
}

impl IcTrain {
    pub fn new() -> Self {
        Self { valve_pos: 0.0, cmd_open: false, heat: 0.0, pool_t: 40.0, pool_mass: IC_POOL_MASS, last_error: None }
    }

    pub fn pool_level_pct(&self) -> f64 {
        self.pool_mass / IC_POOL_MASS * 100.0
    }

    /// Energy the pool can still absorb, J: sensible heat to boiling plus the latent
    /// heat of all of it.
    fn pool_capacity_j(&self) -> f64 {
        (self.pool_mass * CP_WATER * (POOL_BOIL_T - self.pool_t)).max(0.0) + self.pool_mass * H_FG_ATM
    }

    fn valve_after(&self, dt: f64) -> f64 {
        let target = if self.cmd_open { 1.0 } else { 0.0 };
        let d = target - self.valve_pos;
        self.valve_pos + d.clamp(-dt / IC_VALVE_STROKE, dt / IC_VALVE_STROKE)
    }

    /// What this train would take from the vessel this step. Pure. `p_vessel` is the
    /// reactor pressure (Pa); `w_steam_available` is the vessel's steam supply hint
    /// (kg/s; 0.0 when there is no steam space). An invalid argument yields a zero
    /// request; `commit` receives the same `dt`/`p_vessel` and reports it.
    pub fn request(&self, dt: f64, p_vessel: f64, w_steam_available: f64) -> IcRequest {
        if !(dt.is_finite() && dt > 0.0) || !p_vessel.is_finite() || !w_steam_available.is_finite() {
            return IcRequest::default();
        }
        let s = sat_p(p_vessel.max(P_ATM));
        let h_fg = s.h_fg();
        let coverage = (self.pool_level_pct() / 40.0).clamp(0.0, 1.0);
        let dt_drive = if w_steam_available > 0.0 { (s.t - self.pool_t).max(0.0) } else { 0.0 };
        let target_q = ua() * dt_drive * self.valve_after(dt).sqrt() * coverage;
        // Tubes drain and fill with steam over ~20 s (candidate only; `commit` stores the applied value).
        let q_lag = self.heat + (target_q - self.heat) * (dt / 20.0).min(1.0);
        // Sink-limited by what the pool can still absorb this step, and by the steam supply.
        let q = q_lag.max(0.0).min(self.pool_capacity_j() / dt).min(w_steam_available * h_fg);
        let w = if h_fg > 0.0 { q / h_fg } else { 0.0 };
        IcRequest { w_steam_in: w, w_condensate_return: w, h_condensate_return: s.h_f }
    }

    /// Advance the valve and the pool with the flows the vessel APPLIED. The only
    /// state change. `applied` is the request scaled by the vessel's allocation
    /// ratio: identical to the request when inventory was adequate, all zero when
    /// the vessel applied nothing.
    pub fn commit(&mut self, dt: f64, p_vessel: f64, applied: IcRequest) -> IcStep {
        // Master dt rule and non-finite inputs rule: validate everything before any mutation.
        let checks = [
            ("dt", dt, dt.is_finite() && dt > 0.0),
            ("p_vessel", p_vessel, p_vessel.is_finite()),
            ("applied w_steam_in", applied.w_steam_in, applied.w_steam_in.is_finite()),
            ("applied w_condensate_return", applied.w_condensate_return, applied.w_condensate_return.is_finite()),
            ("applied h_condensate_return", applied.h_condensate_return, applied.h_condensate_return.is_finite()),
        ];
        if let Some((what, value, _)) = checks.iter().find(|(_, _, ok)| !ok) {
            self.last_error = Some(DomainError::PropertyOutOfRange { what: (*what).into(), value: *value });
            return IcStep::default();
        }
        let s = sat_p(p_vessel.max(P_ATM));
        let h_fg = s.h_fg();
        self.valve_pos = self.valve_after(dt);
        let w_in = applied.w_steam_in.max(0.0);
        let q = w_in * h_fg;
        self.heat = q;

        // Pool: sensible heating to boiling, then boil-off, bounded by the remaining pool.
        let sensible_cap = (self.pool_mass * CP_WATER * (POOL_BOIL_T - self.pool_t)).max(0.0);
        let absorbed = (q * dt).min(self.pool_capacity_j());
        let sensible = absorbed.min(sensible_cap);
        let latent = absorbed - sensible;
        if sensible > 0.0 {
            self.pool_t += sensible / (self.pool_mass * CP_WATER);
        }
        if latent > 0.0 {
            self.pool_t = POOL_BOIL_T;
        }
        let boiloff = latent / H_FG_ATM;
        self.pool_mass = (self.pool_mass - boiloff).max(0.0);

        // Slow cooling of a hot idle pool by its cooling system: a declared boundary flow.
        let pool_cooling_w = if self.heat < IDLE_DUTY_W && self.pool_t > 40.0 && self.pool_mass > 0.0 {
            self.pool_t -= POOL_IDLE_COOLING_K_PER_S * dt;
            POOL_IDLE_COOLING_K_PER_S * self.pool_mass * CP_WATER
        } else {
            0.0
        };

        IcStep {
            q_removed: q,
            w_steam_in: w_in,
            w_condensate_return: applied.w_condensate_return.max(0.0),
            h_condensate_return: applied.h_condensate_return,
            energy_to_pool: absorbed,
            pool_boiloff_kg: boiloff,
            pool_cooling_w,
        }
    }
}
```

In `crates/bwr-core/src/sim.rs`:

Add `use crate::ic::{IcBoundary, IcRequest};` to the `use` block, the field `pub ic_boundary: IcBoundary,` to `Sim` after `pub ic: Vec<IcTrain>,`, initialise it with `ic_boundary: IcBoundary::default(),` in `base()`, and reset it in `finalize_init` next to the ledger re-base (`self.ic_boundary = IcBoundary::default();`).

In `Sim::step` (the body written in Task 4), replace the step-6 block

```rust
        let steam_avail = !self.vessel.eos.subcooled;
        let q_ic: f64 = self.ic.iter_mut().map(|t| t.step(dt, t_cool, steam_avail)).sum();
```

with

```rust
        // 6: isolation condensers: pure requests from the start-of-step state. The
        // supply hint is the vessel's vapour inventory rate (0 when subcooled);
        // Package 7 replaces it with its allocation.
        let h_fg = sat_p(p.max(P_ATM)).h_fg();
        let w_steam_available =
            if self.vessel.eos.subcooled { 0.0 } else { (self.vessel.mass * self.vessel.eos.x / dt).max(0.0) };
        let ic_requests: Vec<IcRequest> = self.ic.iter().map(|t| t.request(dt, p, w_steam_available)).collect();
        let q_ic: f64 = ic_requests.iter().map(|r| r.w_steam_in * h_fg).sum();
```

(`q_ic` is now the REQUESTED removal that goes into `VesselInputs`; the applied value is `flows.q_ic`.) Then, in the step-9 allocation block, after the `self.steam.w_bypass *= steam_ratio;` line, add:

```rust
        // 9 (IC): commit with the ratio-scaled request. The ratio is 1.0 when the
        // vessel applied the request and 0.0 when it rejected the step, so a pool
        // can never absorb energy the vessel did not give up.
        let ic_ratio = if q_ic > 1e-9 { flows.q_ic / q_ic } else { 0.0 };
        self.ic_boundary.q_removed_w = 0.0;
        for (train, req) in self.ic.iter_mut().zip(&ic_requests) {
            let out = train.commit(dt, p, req.scaled(ic_ratio));
            self.ic_boundary.add(&out, dt);
        }
```

and in the step-17 drain block add, before the core-error drains:

```rust
        let ic_errs: Vec<DomainError> = self.ic.iter_mut().filter_map(|t| t.last_error.take()).collect();
        for e in ic_errs {
            self.domain_error(e);
        }
```

The snapshot's IC fields (`heat`, `pool_t`, `pool_level_pct`, `valve_pos`) read the same struct fields as before; no API change.

- [ ] **Step 4: Run the tests**

Run: `cargo test --release -p bwr-core`
Expected: all pass, including the eight `ic::tests` and `ic_pool_energy_equals_vessel_side_removal_every_step`. `msiv_closure_scrams_and_isolation_condensers_cool` still passes (pools start at 40 degC with 1.5e6 kg, far from any limit in 180 s; the IC duty is now applied one half-step later within the step, which is a re-timing inside its tolerance).

- [ ] **Step 5: Record the constants (register-row rule: same task as the constants; status from the locked vocabulary, qualifications in `notes`)**

Append to `docs/model/parameters.csv`, using its header's column order (Package 1), one row each:

| id | value | unit | definition | status | notes |
|---|---|---|---|---|---|
| `CP_WATER` | 4186 | J/(kg K) | pool water specific heat | confirmed | physical property (liquid water at 1 atm, 15-100 degC); source id of the steam-table reference in sources.csv |
| `H_FG_ATM` | 2.257e6 | J/kg | latent heat of vaporisation at 101.325 kPa | confirmed | physical property; same source |
| `POOL_BOIL_T` | 100 | degC | pool boiling point at atmospheric pressure | confirmed | physical property; the pool is ASSUMED vented to atmosphere (assumption id in assumptions.md) |
| `POOL_IDLE_COOLING_K_PER_S` | 0.0005 | K/s | idle pool cooling rate | assumed | boundary flow reported as IcStep.pool_cooling_w; cooling system not modelled (Package 7 revisits) |

- [ ] **Step 6: Commit**

```bash
git add crates/bwr-core/src/ic.rs crates/bwr-core/src/sim.rs crates/bwr-core/tests/numerics.rs docs/model/parameters.csv
git commit -m "fix(ic): request/commit with applied flows; conserve pool energy

IcTrain::step is replaced by the master's pure request / commit pair: the
pool only absorbs what the vessel applied (zero on EOS rejection). Sensible
heating no longer overshoots 100 degC (baseline reached <paste value from
Step 2>), boil-off is bounded by the remaining pool, a depleted pool removes
no heat, idle cooling is a reported boundary flow, and Sim integrates every
IcStep field (Sim.ic_boundary). commit validates dt and its inputs.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 6: Vessel inventory floor and EOS range rejection

**Files:**
- Modify: `crates/bwr-core/src/vessel.rs` (`solve_eos`, `Vessel` fields, `new`, `step`)
- Modify: `crates/bwr-core/src/sim.rs` (`Sim::step` drains vessel errors; `finalize_init` drains the constructor channel)
- Modify: `crates/bwr-core/src/validation.rs` (conservation floors `TOL_NUMERICAL_MASS_KG`, `TOL_NUMERICAL_ENERGY_J`)
- Modify: `docs/model/parameters.csv` (register rows for the two floors, same task)
- Create: `crates/bwr-core/tests/conservation.rs`

**Interfaces:**
- Consumes: `BoundaryFlows` (r2 shape, incl. `w_ic_steam`/`w_ic_return`/`h_ic_return`, zero until Package 7), `Ledger.integrated`, `Ledger::{mass_residual, energy_residual}`, `Sim::last_flows()`, `Sim::finalize_init` (Package 2), `DomainError`, the Task 4 allocation step, `Sim.ic_boundary` and `POOL_IDLE_COOLING_K_PER_S` (Task 5), `HOTWELL_CAPACITY` (params), `Command::{SetIcTrain, SetHeadVent, SetRwcuReject, SetFeedPump}`.
- Produces: `pub fn solve_eos(mass: f64, energy: f64) -> Result<Eos, DomainError>`; `T_EOS_MIN = 1.0`, `T_EOS_MAX = 349.5`; `Vessel.exhausted: Option<f64>`; `Vessel.last_error: Option<DomainError>` (also set by a non-panicking `Vessel::new` when the initial state is outside the envelope); `Vessel::step` keeps the master signature `-> BoundaryFlows` and reports what it actually applied; `BoundaryFlows.mass_clipped` and `energy_clipped` are always `0.0` from now on (kept for the Package 2 ledger); `validation::TOL_NUMERICAL_MASS_KG = 1e-3` and `validation::TOL_NUMERICAL_ENERGY_J = 1e3`, the conservation floors of the numerical class (master initial values; Task 8 keeps them as the `residual_*` fields of `Tolerance::numerical()` and justifies them as roundoff bounds).
- Rejection semantics (master r2): on `solve_eos` Err inside `step`, `mass` and `energy` roll back to their values at entry, `last_error = Some(..)`, and the returned flows are `BoundaryFlows::default()`, so downstream consumers and the ledger see zero applied transfers for that step. On exhaustion, outflows are scaled so `mass + dm*dt >= 0`, the deficit is recorded in `exhausted`, and the state advances with the bounded flows.

**Baseline defects (verified in code):** `self.mass = (self.mass + dm * dt).max(1000.0)` creates mass; `solve_eos` bisects over 1..349.5 degC and silently returns the nearest bound outside that range.

- [ ] **Step 1: Demonstrate the floor on the baseline**

Create `crates/bwr-core/tests/conservation.rs`:

```rust
//! Mass and energy conservation at the vessel boundary.

mod common;

use bwr_core::api::{Command, ScenarioName};
use bwr_core::domain::DomainError;
use bwr_core::params::*;
use bwr_core::sim::DT;
use bwr_core::steam::sat_p;
use bwr_core::vessel::{solve_eos, Vessel, VesselInputs};
use bwr_core::Sim;

#[test]
fn baseline_vessel_floor_creates_mass() {
    let mut v = Vessel::new(sat_p(P_RATED).t, Z_NWL, RATED_POWER_W, STEAM_FLOW_RATED, T_FW_RATED, false);
    let inp = VesselInputs { q_core: 0.0, q_ic: 0.0, w_feed: 0.0, h_feed: 0.0, w_steam: 0.0, w_liquid_out: 1.0e7 };
    let flows = v.step(DT, &inp);
    println!("baseline mass after draining step: {} kg, mass_clipped {} kg", v.mass, flows.mass_clipped);
    assert_eq!(v.mass, 1000.0, "baseline floors the inventory");
    assert!(flows.mass_clipped > 0.0, "Package 2 ledger must see the created mass");
}
```

Run: `cargo test --release -p bwr-core --test conservation baseline_vessel_floor_creates_mass -- --nocapture`
Expected: PASS with the printed line showing `1000 kg` and a positive `mass_clipped`. Paste it into the commit body, then delete this test.

- [ ] **Step 2: Write the failing tests for the corrected behaviour**

Add to the `use` block of `tests/conservation.rs`:

```rust
use bwr_core::ic::POOL_IDLE_COOLING_K_PER_S;
use bwr_core::ledger::BoundaryFlows;
use bwr_core::validation::{TOL_NUMERICAL_ENERGY_J, TOL_NUMERICAL_MASS_KG};
```

Replace the deleted test in `tests/conservation.rs` with:

```rust
#[test]
fn outflows_are_bounded_by_inventory_and_reported() {
    let mut v = Vessel::new(sat_p(P_RATED).t, Z_NWL, RATED_POWER_W, STEAM_FLOW_RATED, T_FW_RATED, false);
    let m0 = v.mass;
    let inp = VesselInputs { q_core: 0.0, q_ic: 0.0, w_feed: 0.0, h_feed: 0.0, w_steam: 0.0, w_liquid_out: 1.0e7 };
    let flows = v.step(DT, &inp);
    assert!(v.mass >= 0.0 && v.mass < 1.0, "mass {}", v.mass);
    assert_eq!(flows.mass_clipped, 0.0);
    assert!((flows.w_liquid_out * DT - m0).abs() < 1e-6, "applied outflow {} kg vs inventory {} kg", flows.w_liquid_out * DT, m0);
    let deficit = v.exhausted.expect("exhaustion reported");
    assert!((deficit - (1.0e7 * DT - m0)).abs() < 1e-6, "deficit {deficit}");
}

#[test]
fn eos_rejects_states_outside_the_table() {
    let m = 200_000.0;
    assert!(matches!(solve_eos(m, -1.0), Err(DomainError::EosOutOfRange { .. })));
    assert!(matches!(solve_eos(m, 1.0e18), Err(DomainError::EosOutOfRange { .. })));
    assert!(matches!(solve_eos(m, f64::NAN), Err(DomainError::EosOutOfRange { .. })));
    let e = bwr_core::vessel::energy_for(m, 150.0);
    let eos = solve_eos(m, e).unwrap();
    assert!((eos.t - 150.0).abs() < 1e-3);
}

#[test]
fn drained_vessel_is_flagged_not_refilled() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    for i in 0..N_FEED_PUMPS {
        sim.apply(Command::SetFeedPump { index: i, on: false });
    }
    sim.apply(Command::SetRwcuReject { kgs: 15.0 });
    let m0 = sim.vessel.mass;
    let mut t = 0.0;
    while t < 6.0 * 3600.0 && sim.domain_errors.is_empty() {
        sim.step(DT);
        t += DT;
    }
    assert!(
        sim.domain_errors.iter().any(|(_, e)| matches!(e, DomainError::InventoryExhausted { .. })),
        "vessel never ran dry in 6 h (mass {} kg); errors {:?}",
        sim.vessel.mass,
        sim.domain_errors
    );
    println!("inventory exhausted at t = {:.0} s", sim.time);
    // The bounded step scaled the reject flow and every consumer saw the scaled value.
    let f = sim.last_flows();
    let deficit = sim.vessel.exhausted.expect("the last step is the bounded one");
    assert!(deficit > 0.0 && f.w_liquid_out < 15.0 - 1e-9, "bounded step must scale the reject flow: {f:?}");
    assert!((sim.steam.w_tcv + sim.steam.w_bypass - f.w_steam).abs() < 1e-9);
    assert_eq!(sim.ledger.mass_clipped_kg, 0.0);
    assert_eq!(sim.ledger.energy_clipped_j, 0.0);
    // Both residuals against the documented numerical tolerance, through the
    // depletion and the inventory-bounded step (1e-3 kg is far tighter than
    // 1e-6 x mass0, which would allow 0.2 kg).
    let res_m = sim.ledger.mass_residual(sim.vessel.mass);
    let res_e = sim.ledger.energy_residual(sim.vessel.energy);
    assert!(res_m.abs() <= TOL_NUMERICAL_MASS_KG, "mass residual {res_m} kg (mass0 {m0} kg)");
    assert!(res_e.abs() <= TOL_NUMERICAL_ENERGY_J, "energy residual {res_e} J");
    assert!(sim.snapshot().unsupported.is_some());
    common::assert_snapshot_finite(&sim.snapshot());
}

#[test]
fn eos_error_rolls_back_vessel_state() {
    // Rejection semantics (master r2): on an EOS error nothing commits and the
    // returned flows are zero, so the ledger and consumers see no transfer.
    let mut v = Vessel::new(sat_p(P_RATED).t, Z_NWL, RATED_POWER_W, STEAM_FLOW_RATED, T_FW_RATED, false);
    let (m0, e0, t0) = (v.mass, v.energy, v.eos.t);
    // 1e15 W of removal for one step drives the energy far below the table range.
    let inp = VesselInputs { q_core: 0.0, q_ic: 1.0e15, w_feed: 0.0, h_feed: 0.0, w_steam: 0.0, w_liquid_out: 0.0 };
    let flows = v.step(DT, &inp);
    assert!(matches!(v.last_error, Some(DomainError::EosOutOfRange { .. })));
    assert_eq!(v.mass, m0, "mass must roll back on EOS rejection");
    assert_eq!(v.energy, e0, "energy must roll back on EOS rejection");
    assert_eq!(v.eos.t, t0);
    assert_eq!(flows, bwr_core::ledger::BoundaryFlows::default(), "no transfers applied");
}

#[test]
fn invalid_initial_vessel_is_a_retained_domain_error_not_a_panic() {
    // Master "Scenario construction and bookkeeping": an initial state outside
    // the EOS envelope is a domain error that survives finalize_init re-stamped
    // at t = 0 and flags the plant unsupported; Vessel::new must not panic.
    // 0.5 degC lies below T_EOS_MIN, so energy_for(mass, 0.5) is outside the table.
    let bad = Vessel::new(0.5, Z_NWL, 0.0, 0.0, 20.0, false);
    assert!(matches!(bad.last_error, Some(DomainError::EosOutOfRange { .. })), "{:?}", bad.last_error);
    assert!(bad.eos.t.is_finite() && bad.mass > 0.0, "fallback Eos must be usable");
    let mut sim = Sim::new(ScenarioName::HotStandby);
    sim.vessel = bad; // test-only stand-in for a scenario constructor handed an invalid state
    sim.finalize_init();
    assert_eq!(sim.time, 0.0);
    assert_eq!(sim.domain_errors.len(), 1, "{:?}", sim.domain_errors);
    assert_eq!(sim.domain_errors[0].0, 0.0, "initialisation error must be stamped at t = 0");
    assert!(matches!(sim.domain_errors[0].1, DomainError::EosOutOfRange { .. }));
    assert!(sim.snapshot().unsupported.is_some());
    sim.step(DT); // keeps rejecting, deduplicated by variant, no panic
    assert_eq!(sim.domain_errors.len(), 1);
    assert_eq!(*sim.last_flows(), BoundaryFlows::default());
    common::assert_snapshot_finite(&sim.snapshot());
}

/// Sim-level EOS forcing (the vessel-level form is eos_error_rolls_back_vessel_state):
/// an energy no EOS state can have, with `eos` left as it was so every request in
/// the next step is still positive. The caller restores `energy` afterwards; the
/// rollback leaves `mass` and `eos` untouched, so the restored Sim is exactly the
/// pre-step Sim plus one zero-flow ledger record.
fn force_eos_rejection_for_one_step(sim: &mut Sim) {
    let e_saved = sim.vessel.energy;
    sim.vessel.energy = -1.0;
    sim.step(DT);
    assert_eq!(*sim.last_flows(), BoundaryFlows::default(), "the forced step must apply nothing");
    assert!(matches!(sim.vessel.last_error, None), "drained by Sim::step");
    assert!(sim.domain_errors.iter().any(|(_, e)| matches!(e, DomainError::EosOutOfRange { .. })));
    sim.vessel.energy = e_saved;
}

#[test]
fn eos_rejection_leaves_hotwell_and_pool_unchanged() {
    // Review cycle 2, comment 9: a rejected vessel step applies NOTHING, so the
    // hotwell must not give up the requested feed, must receive no steam or RWCU
    // reject, and no IC pool may absorb the requested heat. Full power with IC 0
    // in service and RWCU on, so feed, steam, RWCU and IC transfers are all
    // requested (positive) in the rejected step.
    let mut sim = Sim::new(ScenarioName::FullPower);
    sim.apply(Command::SetIcTrain { index: 0, open: true });
    sim.apply(Command::SetRwcuReject { kgs: 15.0 });
    for _ in 0..(60.0 / DT) as usize {
        sim.step(DT);
    }
    let f = *sim.last_flows();
    assert!(f.w_feed > 0.0 && f.w_steam > 0.0 && f.w_liquid_out > 0.0 && f.q_ic > 0.0, "fixture must have every transfer active: {f:?}");
    let frac = sim.cond.hotwell_mass / HOTWELL_CAPACITY;
    assert!(frac > 0.4 && frac < 0.85, "hotwell must sit inside the makeup/reject dead band so its mass is a pure receipt: {frac}");
    assert!(sim.domain_errors.is_empty(), "{:?}", sim.domain_errors);

    let (m_v, t_v, p_v, e_v) = (sim.vessel.mass, sim.vessel.eos.t, sim.vessel.eos.p, sim.vessel.energy);
    let hotwell_before = sim.cond.hotwell_mass;
    let (pool_m, pool_t) = (sim.ic[0].pool_mass, sim.ic[0].pool_t);
    let (e_pool, boiloff, ret) =
        (sim.ic_boundary.energy_to_pool_j, sim.ic_boundary.pool_boiloff_kg, sim.ic_boundary.condensate_return_kg);

    force_eos_rejection_for_one_step(&mut sim);

    assert_eq!(sim.domain_errors.len(), 1, "{:?}", sim.domain_errors);
    assert_eq!((sim.vessel.mass, sim.vessel.energy, sim.vessel.eos.t, sim.vessel.eos.p), (m_v, e_v, t_v, p_v), "vessel must roll back");
    assert_eq!(sim.cond.hotwell_mass, hotwell_before, "hotwell gave up feed the vessel never accepted, or received steam/RWCU it never released");
    assert_eq!(sim.ic[0].pool_mass, pool_m, "pool boiled off mass for heat the vessel did not give up");
    assert_eq!(sim.ic_boundary.energy_to_pool_j, e_pool, "pool absorbed heat the vessel did not give up");
    assert_eq!(sim.ic_boundary.pool_boiloff_kg, boiloff);
    assert_eq!(sim.ic_boundary.condensate_return_kg, ret);
    // The only permitted pool change is the recorded idle loss (a boundary term).
    assert!(
        sim.ic[0].pool_t <= pool_t && pool_t - sim.ic[0].pool_t <= POOL_IDLE_COOLING_K_PER_S * DT + 1e-12,
        "pool_t {pool_t} -> {}",
        sim.ic[0].pool_t
    );
    assert!(sim.ledger.mass_residual(sim.vessel.mass).abs() <= TOL_NUMERICAL_MASS_KG);
    assert!(sim.ledger.energy_residual(sim.vessel.energy).abs() <= TOL_NUMERICAL_ENERGY_J);
}

#[test]
fn coupled_run_hotwell_gain_equals_vessel_release() {
    // Defect 13 regression, measured on STATE at both ends, not on the ledger:
    // the hotwell's actual mass change must equal the vessel's actual mass
    // change (sign reversed) once the transfers the vessel applied have been
    // handed to the condenser. Full power, feed and steam at rated, RWCU reject
    // on, 400 steps, with one forced EOS rejection in the middle so applied !=
    // requested for every consumer at least once (ratio 0). Line and IC
    // inventories are not modelled before Packages 6/7, the head vent is shut
    // and the IC trains are isolated, so the hotwell is the only receiver; its
    // automatic +/-30 kg/s makeup/reject (an unaccounted boundary flow until
    // Package 6, master defect 6) is kept out of the measurement by asserting
    // the dead band every step.
    let mut sim = Sim::new(ScenarioName::FullPower);
    sim.apply(Command::SetRwcuReject { kgs: 15.0 });
    let m_vessel0 = sim.vessel.mass;
    let m_hotwell0 = sim.cond.hotwell_mass;
    for i in 0..400 {
        if i == 200 {
            force_eos_rejection_for_one_step(&mut sim);
        } else {
            sim.step(DT);
            let f = sim.last_flows();
            assert!(f.w_feed > 0.0 && f.w_steam > 0.0 && f.w_liquid_out > 0.0, "every normal step must move feed, steam and RWCU: {f:?}");
            assert_eq!(f.w_vent, 0.0);
            assert_eq!(f.q_ic, 0.0);
            assert!((sim.steam.w_tcv + sim.steam.w_bypass - f.w_steam).abs() < 1e-9);
        }
        let frac = sim.cond.hotwell_mass / HOTWELL_CAPACITY;
        assert!(frac > 0.4 && frac < 0.85, "hotwell left the makeup/reject dead band at step {i}: {frac}");
    }
    assert_eq!(sim.domain_errors.len(), 1, "{:?}", sim.domain_errors);
    let hotwell_gain = sim.cond.hotwell_mass - m_hotwell0;
    let vessel_release = m_vessel0 - sim.vessel.mass;
    assert!(
        (hotwell_gain - vessel_release).abs() <= TOL_NUMERICAL_MASS_KG,
        "hotwell gained {hotwell_gain} kg but the vessel released {vessel_release} kg"
    );
    // The ledger is the third, independent observer and must agree with both states.
    let ledger_release =
        sim.ledger.integrated["steam_kg"] + sim.ledger.integrated["rwcu_kg"] - sim.ledger.integrated["feed_kg"];
    assert!((ledger_release - vessel_release).abs() <= TOL_NUMERICAL_MASS_KG, "ledger {ledger_release} kg vs vessel {vessel_release} kg");
    assert!(sim.ledger.integrated["rwcu_kg"] > 0.0 && sim.ledger.integrated["feed_kg"] > 0.0 && sim.ledger.integrated["steam_kg"] > 0.0);
    assert!(sim.ledger.energy_residual(sim.vessel.energy).abs() <= TOL_NUMERICAL_ENERGY_J);
}

#[test]
fn residuals_stay_within_numerical_tolerance_with_every_transfer_active() {
    // Acceptance gate: "mass and energy residuals close to the stated numerical
    // tolerance" with feed, steam, head vent, RWCU and IC transfers all active
    // at once, through normal steps, a rejected step and the steps after it.
    // (Inventory-bounded steps: drained_vessel_is_flagged_not_refilled.)
    let mut sim = Sim::new(ScenarioName::FullPower);
    sim.apply(Command::SetIcTrain { index: 0, open: true });
    sim.apply(Command::SetRwcuReject { kgs: 15.0 });
    sim.apply(Command::SetHeadVent { open: true });
    let check = |sim: &Sim, what: &str| {
        let rm = sim.ledger.mass_residual(sim.vessel.mass);
        let re = sim.ledger.energy_residual(sim.vessel.energy);
        assert!(rm.abs() <= TOL_NUMERICAL_MASS_KG, "{what}: mass residual {rm} kg at t = {}", sim.time);
        assert!(re.abs() <= TOL_NUMERICAL_ENERGY_J, "{what}: energy residual {re} J at t = {}", sim.time);
    };
    for _ in 0..(60.0 / DT) as usize {
        sim.step(DT);
        check(&sim, "normal step");
    }
    let f = *sim.last_flows();
    assert!(
        f.w_feed > 0.0 && f.w_steam > 0.0 && f.w_vent > 0.0 && f.w_liquid_out > 0.0 && f.q_ic > 0.0,
        "every transfer must be active at once: {f:?}"
    );
    force_eos_rejection_for_one_step(&mut sim);
    assert_eq!(sim.domain_errors.len(), 1, "{:?}", sim.domain_errors);
    check(&sim, "rejected step");
    for _ in 0..(60.0 / DT) as usize {
        sim.step(DT);
        check(&sim, "step after rejection");
    }
    for key in ["feed_kg", "steam_kg", "vent_kg", "rwcu_kg", "ic_j"] {
        assert!(sim.ledger.integrated[key] > 0.0, "{key} never moved");
    }
    assert_eq!(sim.ledger.mass_clipped_kg, 0.0);
    assert_eq!(sim.ledger.energy_clipped_j, 0.0);
}
```

`coupled_run_hotwell_gain_equals_vessel_release` is the master's "coupled regression asserting that the condenser hotwell gains exactly what the vessel released"; it compares two STATES (hotwell mass, vessel mass) and uses the ledger only as a third observer, so it is not two integrations of the same `BoundaryFlows`. Its defect-13 demonstration is a one-off manual check, recorded in the commit body: it must FAIL at step 200 with each consumer miswired in turn, (a) `cond.step(.., flows.w_feed)` fed `self.fw.flow` (the hotwell loses about 50 kg of feed the vessel never accepted), (b) the condenser `w_in` fed `self.fw.rwcu_reject` instead of `flows.w_liquid_out` (0.75 kg received that was never released), (c) the two `steam_ratio` lines removed (about 50 kg of steam received that was never released). Each miswire is restored before the next; the failure messages go in the commit body.

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test --release -p bwr-core --test conservation`
Expected: compile errors (`no field exhausted`, `solve_eos` returns `Eos` not `Result`, unresolved imports `TOL_NUMERICAL_MASS_KG`/`TOL_NUMERICAL_ENERGY_J`).

- [ ] **Step 4: Implement**

In `crates/bwr-core/src/vessel.rs` add `use crate::domain::DomainError;` and `use crate::ledger::BoundaryFlows;` (the latter already exists from Package 2).

Add after the `W_MIN` constant:

```rust
/// EOS bisection envelope, degC. Outside it the state is a DomainError, not a clamp.
pub const T_EOS_MIN: f64 = 1.0;
pub const T_EOS_MAX: f64 = 349.5;
```

Replace `solve_eos`:

```rust
/// Solve for temperature/pressure from total fluid mass and energy (incl. metal).
/// Err when the energy lies outside [energy_at(T_EOS_MIN), energy_at(T_EOS_MAX)]
/// or the inputs are not finite.
pub fn solve_eos(mass: f64, energy: f64) -> Result<Eos, DomainError> {
    let out_of_range = !(mass >= 0.0)
        || !energy.is_finite()
        || energy < energy_at(T_EOS_MIN, mass).0
        || energy > energy_at(T_EOS_MAX, mass).0;
    if out_of_range {
        return Err(DomainError::EosOutOfRange { mass_kg: mass, energy_j: energy });
    }
    let (mut lo, mut hi) = (T_EOS_MIN, T_EOS_MAX);
    for _ in 0..60 {
        let mid = 0.5 * (lo + hi);
        if energy_at(mid, mass).0 < energy {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let t = 0.5 * (lo + hi);
    let (_, x) = energy_at(t, mass);
    let s = sat_t(t);
    let subcooled = t < 100.0;
    let solid = !subcooled && x <= 0.0;
    Ok(Eos { t, p: if subcooled { P_ATM } else { s.p }, x, subcooled, solid })
}
```

In `pub struct Vessel` add after `pub drive_dp: f64,`:

```rust
    /// Deficit, kg, when this step's outflows exceeded the inventory and were scaled down.
    pub exhausted: Option<f64>,
    /// EOS rejection from this step; drained by `Sim::step` into a DomainError.
    pub last_error: Option<DomainError>,
```

In `Vessel::new`, add `exhausted: None, last_error: None,` to the struct literal, and change

```rust
        v.eos = solve_eos(v.mass, v.energy);
```

to the non-panicking form

```rust
        // An initial state outside the EOS envelope is a retained domain error, never a
        // panic: keep a provisional saturated-liquid Eos at the nearest envelope bound
        // (what the old clamped bisection returned) and leave the error in `last_error`
        // for the normal Sim drain (Sim::step step 17, or finalize_init for a scenario
        // that never stepped). Every later `step` rejects until LoadScenario, so the
        // plant stays flagged unsupported from t = 0.
        match solve_eos(v.mass, v.energy) {
            Ok(eos) => v.eos = eos,
            Err(e) => {
                v.last_error = Some(e);
                let t_c = t.clamp(T_EOS_MIN, T_EOS_MAX);
                let subcooled = t_c < 100.0;
                v.eos = Eos { t: t_c, p: if subcooled { P_ATM } else { sat_t(t_c).p }, x: 0.0, subcooled, solid: false };
            }
        }
```

(`update_level()` on the next line runs on the fallback Eos, which is a valid liquid state, so the constructor completes.) `Vessel::step` must not clear `last_error` at entry (it only sets it on Err; `Sim::step` takes it), so a constructor error that is not drained before the first step is drained by that step.

Replace the first half of `Vessel::step`, from `pub fn step(` down to and including the line `let s = self.sat_for_core();`, with:

```rust
    pub fn step(&mut self, dt: f64, inp: &VesselInputs) -> BoundaryFlows {
        let s0 = self.sat();
        let t_before = self.eos.t;
        self.exhausted = None;

        // Head vent: steam escapes while the vessel is above atmospheric pressure.
        let vent_demand = if self.head_vent_open && !self.eos.subcooled {
            (C_HEAD_VENT * (self.eos.p - P_ATM)).max(0.0) + 0.02 * (self.eos.t - 100.0).max(0.0)
        } else {
            0.0
        };

        let h_steam = s0.h_g;
        let h_liq = if self.eos.subcooled { steam::h_liquid(self.eos.t) } else { s0.h_f };
        let heat_loss = HEAT_LOSS_HOT * ((self.eos.t - 20.0) / 268.0).max(0.0);

        // Bound every outflow by the inventory that exists this step. Outflows are
        // scaled proportionally and the shortfall is reported; mass is never created.
        let steam_demand = if self.eos.subcooled { 0.0 } else { inp.w_steam };
        let out_demand = steam_demand + inp.w_liquid_out + vent_demand;
        let available = self.mass + inp.w_feed * dt;
        let scale = if out_demand * dt > available {
            (available / (out_demand * dt)).clamp(0.0, 1.0)
        } else {
            1.0
        };
        if scale < 1.0 {
            self.exhausted = Some(out_demand * dt - available);
        }
        let w_steam = steam_demand * scale;
        let w_liquid_out = inp.w_liquid_out * scale;
        self.vent_flow = vent_demand * scale;

        let dm = inp.w_feed - w_steam - w_liquid_out - self.vent_flow;
        let de = inp.q_core - inp.q_ic - heat_loss + inp.w_feed * inp.h_feed
            - (w_steam + self.vent_flow) * h_steam
            - w_liquid_out * h_liq;
        let flows = BoundaryFlows {
            w_feed: inp.w_feed,
            h_feed: inp.h_feed,
            w_steam,
            h_steam,
            w_vent: self.vent_flow,
            w_liquid_out,
            h_liquid_out: h_liq,
            w_ic_steam: 0.0,   // IC mass ports open in Package 7
            w_ic_return: 0.0,
            h_ic_return: 0.0,
            q_core: inp.q_core,
            q_ic: inp.q_ic,
            q_loss: heat_loss,
            mass_clipped: 0.0,
            energy_clipped: 0.0,
        };

        let mass1 = self.mass + dm * dt;
        let energy1 = self.energy + de * dt;
        match solve_eos(mass1, energy1) {
            Ok(eos) => {
                self.mass = mass1;
                self.energy = energy1;
                self.eos = eos;
            }
            Err(err) => {
                // Nothing is applied: the vessel freezes this step and the plant is flagged.
                self.last_error = Some(err);
                self.vent_flow = 0.0;
                return BoundaryFlows::default();
            }
        }
        let s = self.sat_for_core();
```

The remainder of `step` (core inlet enthalpy, voids, momentum, `update_level`, heat-up rate) is unchanged except that its final expression must be `flows` (Package 2 already returns the flows struct; replace its name if different).

In `crates/bwr-core/src/sim.rs`, `Sim::step` (the Task 4 body), at the END of the function, after `self.last_flows = flows;` (master shared step order, item 17: "drain vessel.last_error / core error / ic errors into domain_errors"):

```rust
        // 17: drain vessel errors raised this step.
        if let Some(deficit) = self.vessel.exhausted {
            self.domain_error(DomainError::InventoryExhausted { deficit_kg: deficit });
        }
        if let Some(e) = self.vessel.last_error.take() {
            self.domain_error(e);
        }
```

In `Sim::finalize_init` (Package 2; Task 1 added the retention rule), drain the constructor error channels BEFORE the `kept` line, so an invalid initial vessel reaches `domain_errors` even when the scenario constructor never stepped:

```rust
        // Constructor-raised errors (Vessel::new outside the EOS envelope) take the
        // normal drain path; the retention rule below then re-stamps them at t = 0.
        if let Some(e) = self.vessel.last_error.take() {
            self.domain_error(e);
        }
```

(`full_power` steps during `settle`, so there the step-17 drain fires first and the retention rule re-stamps; `hot_standby` does not step, so this drain is the only path. Both end as `(0.0, EosOutOfRange)` in `domain_errors`, which `invalid_initial_vessel_is_a_retained_domain_error_not_a_panic` pins.)

Rejection is transactional across components because of the step order, not because of anything added here: the IC trains only `request` before the vessel step and `commit` after it with the ratio-scaled request (Task 5), and the condenser is handed `flows.w_feed`, `flows.w_liquid_out` and the ratio-scaled `steam.w_tcv`/`w_bypass` (Task 4). On an EOS rejection `flows == BoundaryFlows::default()`, so `steam_ratio` and `ic_ratio` are both 0.0: the pools commit a zero request (idle loss only, recorded), the hotwell withdraws zero feed and heats nothing, and the turbine sees zero flow, while `fw.flow`, `fw.rwcu_reject` and the IC `cmd_open` state keep their requested values for the next step. `eos_rejection_leaves_hotwell_and_pool_unchanged` pins every one of those receivers; `coupled_run_hotwell_gain_equals_vessel_release` pins the mass closure end to end. Do not reintroduce `self.fw.flow`, `self.fw.rwcu_reject` or a pre-vessel IC commit anywhere below step 8.

In `crates/bwr-core/src/validation.rs`, add next to `Tolerance` (these are the `residual_mass_kg` / `residual_energy_j` values of the numerical class; Task 8 wires them into `Tolerance::numerical()` and justifies them in `method.md`):

```rust
/// Conservation floors of the numerical tolerance class (master initial values).
/// They bound f64 roundoff on the ledger sums, not discretisation error, so they
/// are not derived from the dt-halving study: eps (2.2e-16) x the largest
/// standard-case throughput (about 1e6 kg / 1e14 J) random-walked over 1e6 steps
/// is about 2e-7 kg / 2e1 J; these leave two decades of margin and are still
/// below 1e-8 of the stored inventory. A residual above them is a bookkeeping
/// defect in the owning module, never a reason to raise the floor.
pub const TOL_NUMERICAL_MASS_KG: f64 = 1e-3;
pub const TOL_NUMERICAL_ENERGY_J: f64 = 1e3;
```

and add the two register rows to `docs/model/parameters.csv` in this task (register-row rule): `validation.TOL_NUMERICAL_MASS_KG`, `1e-3`, `kg`, status `derived`, notes `roundoff floor of the ledger mass residual; see method.md convergence section`; `validation.TOL_NUMERICAL_ENERGY_J`, `1e3`, `J`, status `derived`, notes likewise. Run `cargo test --release -p bwr-core --test register` before moving on.

Update the existing unit test in `vessel.rs`:

```rust
    #[test]
    fn eos_roundtrip() {
        for &t in &[40.0, 150.0, 287.0] {
            let m = 200_000.0;
            let e = energy_for(m, t);
            let eos = solve_eos(m, e).unwrap();
            assert!((eos.t - t).abs() < 1e-3, "t {t} -> {}", eos.t);
        }
    }
```

- [ ] **Step 5: Run the tests**

Run: `cargo test --release -p bwr-core --test conservation -- --nocapture`
Expected: all eight tests pass; `drained_vessel_is_flagged_not_refilled` prints `inventory exhausted at t = <about 9000-12000> s` and runs in well under a minute in release. Then perform the defect-13 demonstration described after the Step 2 listing: miswire each consumer in turn ((a) condenser `w_out` fed `self.fw.flow`, (b) condenser `w_in` fed `self.fw.rwcu_reject`, (c) the two `steam_ratio` lines removed), rerun `coupled_run_hotwell_gain_equals_vessel_release` and `eos_rejection_leaves_hotwell_and_pool_unchanged`, confirm both FAIL at the forced step, record the three failure messages for the commit body, and restore the wiring.

Run: `cargo test --release -p bwr-core`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add crates/bwr-core/src/vessel.rs crates/bwr-core/src/sim.rs crates/bwr-core/src/validation.rs crates/bwr-core/tests/conservation.rs docs/model/parameters.csv
git commit -m "fix(vessel): bounded outflows and EOS range rejection replace silent clamps

Baseline: <paste printed line from Step 1>. Outflows are now scaled to the
available inventory and the deficit raised as DomainError::InventoryExhausted;
solve_eos returns EosOutOfRange instead of the nearest table bound; on EOS
rejection nothing commits and zero flows are returned; an invalid initial
vessel is a retained t = 0 domain error, not a panic. Conservation floors
TOL_NUMERICAL_MASS_KG / TOL_NUMERICAL_ENERGY_J registered. Coupled regression
with each consumer miswired (feed / RWCU / steam scaling):
<paste the three defect-13 failure messages from Step 5>.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 7: Edge states and non-finite command inputs

**Files:**
- Modify: `crates/bwr-core/src/sim.rs` (`cmd_is_finite`, `apply`)
- Modify: `crates/bwr-core/src/vessel.rs` (non-finite input guard at the top of `step`)
- Modify: `crates/bwr-core/tests/numerics.rs`

**Interfaces:**
- Consumes: `assert_finite` (Task 1), `IcTrain.pool_mass` (Task 5 makes an empty pool a true no-sink), `Vessel.last_error` (Task 6).
- Produces: `fn cmd_is_finite(cmd: &Command) -> bool` (private to `sim.rs`); non-finite numeric commands are ignored with event id `cmd` and text `Command ignored: non-finite value` (master non-finite inputs rule). `Vessel::step` treats a non-finite `VesselInputs` field as `DomainError::PropertyOutOfRange` (same rule for public step functions; `Kinetics::advance` already does this from Task 3).

**Baseline defect (verified in code):** `f64::clamp` propagates NaN, so `SetSpeed { speed: NAN }` freezes `advance()` forever, and a NaN rod target or pressure setpoint poisons rod positions or the pressure regulator. (JSON cannot carry NaN, so this only reaches the engine from in-process callers such as tests and scripted cases, which is exactly where the validation runner lives.)

- [ ] **Step 1: Write the failing tests**

Append to `tests/numerics.rs`:

```rust
use common::assert_finite;

#[test]
fn cold_vented_vessel_with_vent_closed_and_no_pumps_stays_finite() {
    // Zero power, no steam, closed valves, no pumps.
    let mut sim = Sim::new(ScenarioName::ColdShutdown);
    sim.apply(Command::SetHeadVent { open: false });
    for _ in 0..60 {
        run(&mut sim, 60.0);
        assert_finite(&sim.snapshot());
    }
    assert!(sim.domain_errors.is_empty(), "{:?}", sim.domain_errors);
    assert!(sim.core.kin.n < 1e-6);
}

#[test]
fn isolated_hot_vessel_with_no_heat_sink_pressurises_finitely() {
    let mut sim = Sim::new(ScenarioName::HotStandby);
    sim.apply(Command::SetMsiv { line: 0, open: false });
    sim.apply(Command::SetMsiv { line: 1, open: false });
    for t in sim.ic.iter_mut() {
        t.pool_mass = 0.0; // no sink: a depleted pool removes nothing (Task 5)
    }
    let p0 = sim.vessel.eos.p;
    for _ in 0..60 {
        run(&mut sim, 60.0);
        let s = sim.snapshot();
        assert_finite(&s);
        if s.vessel.pressure_mpa * 1e6 > VESSEL_DESIGN_PRESSURE {
            assert!(s.alarms.iter().any(|a| a.id == "overpressure"), "overpressure alarm missing");
        }
    }
    assert!(sim.vessel.eos.p > p0, "pressure should rise with decay heat and no sink");
    assert!(sim.ic.iter().all(|t| t.heat == 0.0));
}

#[test]
fn all_steam_paths_closed_at_power_stays_finite() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    sim.apply(Command::SetBypass { auto: false, pos: 0.0 });
    sim.apply(Command::TripTurbine);
    for _ in 0..18 {
        run(&mut sim, 10.0);
        assert_finite(&sim.snapshot());
    }
    assert!(sim.scrammed, "closing every steam path must scram on pressure");
    assert!(sim.ic.iter().any(|t| t.cmd_open), "IC should auto-initiate on high pressure");
}

#[test]
fn non_finite_commands_are_ignored() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    let rods_before = sim.core.rods.target.clone();
    let sp_before = sim.steam.p_setpoint;
    sim.apply(Command::SetSpeed { speed: f64::NAN });
    sim.apply(Command::RodSetTarget { indices: vec![0, 1], target: f64::NAN });
    sim.apply(Command::RodSetTarget { indices: vec![2], target: f64::INFINITY });
    sim.apply(Command::SetPressureSetpoint { mpa: f64::NAN });
    sim.apply(Command::SetLevelSetpoint { cm: f64::NEG_INFINITY });
    sim.apply(Command::SetRwcuReject { kgs: f64::NAN });
    assert_eq!(sim.speed, 1.0);
    assert_eq!(sim.core.rods.target, rods_before);
    assert_eq!(sim.steam.p_setpoint, sp_before);
    assert_eq!(sim.fw.level_sp_cm, 0.0);
    assert_eq!(sim.fw.rwcu_reject, 0.0);
    assert!(sim.snapshot().alarms.iter().any(|a| a.id == "cmd"));
    sim.advance(0.1);
    assert!(sim.time > 0.0, "advance must not be frozen by a NaN speed");
    assert_finite(&sim.snapshot());
}

#[test]
fn vessel_step_rejects_non_finite_inputs() {
    use bwr_core::vessel::{Vessel, VesselInputs};
    let mut v = Vessel::new(sat_p(P_RATED).t, Z_NWL, RATED_POWER_W, STEAM_FLOW_RATED, T_FW_RATED, false);
    let (m0, e0) = (v.mass, v.energy);
    let inp = VesselInputs { q_core: f64::NAN, q_ic: 0.0, w_feed: 0.0, h_feed: 0.0, w_steam: 0.0, w_liquid_out: 0.0 };
    let flows = v.step(DT, &inp);
    assert!(matches!(v.last_error, Some(DomainError::PropertyOutOfRange { .. })));
    assert_eq!((v.mass, v.energy), (m0, e0), "nothing commits on a non-finite input");
    assert_eq!(flows, bwr_core::ledger::BoundaryFlows::default());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --release -p bwr-core --test numerics non_finite_commands_are_ignored`
Expected: FAIL at `assert_eq!(sim.speed, 1.0)` (left: NaN). The three edge-state tests are expected to pass already; if any fails, that failure is a finding: record it in the commit body and fix the cause in this task rather than weakening the test.

- [ ] **Step 3: Implement**

In `crates/bwr-core/src/sim.rs`, above `impl Sim` add:

```rust
/// Numeric command payloads must be finite; NaN/inf would poison clamps and lags.
fn cmd_is_finite(cmd: &Command) -> bool {
    match cmd {
        Command::SetSpeed { speed } => speed.is_finite(),
        Command::RodSetTarget { target, .. } => target.is_finite(),
        Command::SetPressureSetpoint { mpa } => mpa.is_finite(),
        Command::SetBypass { pos, .. } => pos.is_finite(),
        Command::SetSpeedSetpoint { rpm } => rpm.is_finite(),
        Command::SetLoadLimit { pct } => pct.is_finite(),
        Command::SetFcvManual { pos } => pos.is_finite(),
        Command::SetLevelSetpoint { cm } => cm.is_finite(),
        Command::SetRwcuReject { kgs } => kgs.is_finite(),
        _ => true,
    }
}
```

In `crates/bwr-core/src/vessel.rs`, at the top of `Vessel::step` (before `let s0 = self.sat();`), add the public-step-function guard:

```rust
        // Master non-finite inputs rule: reject before anything commits.
        let fields = [
            ("q_core", inp.q_core), ("q_ic", inp.q_ic), ("w_feed", inp.w_feed),
            ("h_feed", inp.h_feed), ("w_steam", inp.w_steam), ("w_liquid_out", inp.w_liquid_out),
        ];
        if let Some((what, value)) = fields.iter().find(|(_, v)| !v.is_finite()) {
            self.last_error = Some(DomainError::PropertyOutOfRange { what: format!("vessel input {what}"), value: *value });
            self.vent_flow = 0.0;
            return BoundaryFlows::default();
        }
```

At the top of `pub fn apply(&mut self, cmd: Command)`, before `match cmd {`:

```rust
        if !cmd_is_finite(&cmd) {
            // Master non-finite inputs rule: exact event text; detail goes to the log.
            self.log.push((self.time, format!("command ignored: non-finite value in {cmd:?}")));
            self.event("cmd", "Command ignored: non-finite value");
            return;
        }
```

- [ ] **Step 4: Run the tests**

Run: `cargo test --release -p bwr-core --test numerics`
Expected: all pass. Note the runtime of the two 1 h edge cases in the commit body.

- [ ] **Step 5: Commit**

```bash
git add crates/bwr-core/src/sim.rs crates/bwr-core/src/vessel.rs crates/bwr-core/tests/numerics.rs
git commit -m "test(core): edge-state finiteness and non-finite input rejection

Covers zero power/no steam/closed valves, isolated vessel with no heat
sink, all steam paths closed at power, and NaN/inf command payloads
(previously NaN speed froze advance()).

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 8: Time-step convergence and measured numerical tolerances

**Files:**
- Modify: `crates/bwr-core/tests/numerics.rs`
- Modify: `crates/bwr-core/src/validation.rs` (`Tolerance::numerical()` values)
- Modify: `docs/validation/method.md` (convergence subsection and tolerance table)

**Interfaces:**
- Consumes: `validation::{run_case, standard_cases, compare, Tolerance}`, `Trace::metrics()`, `Metrics.{first_event_time, peak, min, final_values, integrated, max_abs_mass_residual_kg, max_abs_energy_residual_j, nonfinite}` (Package 2), `TOL_NUMERICAL_MASS_KG` / `TOL_NUMERICAL_ENERGY_J` (Task 6), case ids `fp_turbine_trip` and `fp_manual_scram` from `standard_cases()`.
- Produces: every field of `Tolerance::numerical()` as a named `pub const TOL_NUMERICAL_*` derived from the measurement table (master r2: "P3 REPLACES these with 2x the measured dt-halving differences from the convergence study"); `Tolerance::physical()` keeps the master shape (abs floors 10x numerical) and therefore follows.
- Also: `docs/model/parameters.csv` rows for every `TOL_NUMERICAL_*` constant and every `TOL_NUMERICAL_ABS` entry (register-row rule, this task).

**dt audit (done while planning; no hidden `DT` assumptions found):** every time-dependent update in `sim.rs`, `bop.rs`, `vessel.rs`, `ic.rs`, `reactor.rs` is either exact (`DecayHeat::step`, implicit fuel node), rate-limited by `dt` (valve strokes, rod speed, `SPEED_RAMP`), a first-order explicit lag of the form `x += (target - x) * (dt / tau).min(1.0)` (`h_in`, `alpha_core`, `alpha_ch`, `heatup_rate`, IC `heat`, condenser `p_steam`, `t_feed`, `inv_period`), an explicit Euler integral (`Xenon::step`, governor and level-controller integrals, air mass), or an absolute-time comparison (`scram_time`, event expiry, `reverse_power_timer`, `low_suction_timer`). All converge at first order in `dt`; the semi-implicit momentum step and `solve_eos` are stable for any `dt`. The only `DT`-specific code is `Sim::advance` and `Sim::settle`, which the runner does not use. Protection is evaluated once per step, so trip times are quantised to `dt`; the 0.1 s event tolerance below covers that.

- [ ] **Step 1: Write the convergence test**

Append to `tests/numerics.rs`. The test both ENFORCES convergence across the three steps and PRINTS the measurement table that Step 3 turns into `Tolerance::numerical()`. The measurements are printed before any assertion so a failure still leaves the table in the output.

```rust
use bwr_core::validation::{compare, run_case, standard_cases, Metrics, Tolerance, TOL_NUMERICAL_ENERGY_J, TOL_NUMERICAL_MASS_KG};

const CONVERGENCE_CASES: [&str; 2] = ["fp_turbine_trip", "fp_manual_scram"];
/// Production step first, then the two refinements.
const CONVERGENCE_DTS: [f64; 3] = [0.05, 0.025, 0.0125];

/// Event SETS must match between two runs: an event present at one dt and
/// missing at the other is a failure. A time is compared with the coarser run's
/// quantisation, max(dt_coarse, tol): protection is evaluated once per step, so
/// no event can agree better than the step it was detected in.
fn assert_event_sets_match(id: &str, coarse: &Metrics, dt_coarse: f64, fine: &Metrics, dt_fine: f64, tol_s: f64) {
    let quant = dt_coarse.max(tol_s);
    for (ev, t_c) in &coarse.first_event_time {
        let t_f = fine
            .first_event_time
            .get(ev)
            .unwrap_or_else(|| panic!("{id}: event {ev:?} at dt={dt_coarse} is missing at dt={dt_fine}"));
        assert!((t_c - t_f).abs() <= quant, "{id}: {ev:?} at {t_c} s (dt {dt_coarse}) vs {t_f} s (dt {dt_fine}); allowed {quant} s");
    }
    for ev in fine.first_event_time.keys() {
        assert!(coarse.first_event_time.contains_key(ev), "{id}: event {ev:?} at dt={dt_fine} is missing at dt={dt_coarse}");
    }
}

/// Integrated transfers (ledger totals): relative to the finer run, with the
/// conservation floor as the absolute floor for totals near zero.
fn assert_integrated_match(id: &str, coarse: &Metrics, dt_coarse: f64, fine: &Metrics, rel: f64) {
    assert_eq!(
        coarse.integrated.keys().collect::<Vec<_>>(),
        fine.integrated.keys().collect::<Vec<_>>(),
        "{id}: integrated key sets differ at dt={dt_coarse}"
    );
    for (k, v_f) in &fine.integrated {
        let v_c = coarse.integrated[k];
        let floor = if k.ends_with("_kg") { TOL_NUMERICAL_MASS_KG } else { TOL_NUMERICAL_ENERGY_J };
        assert!((v_c - v_f).abs() <= rel * v_f.abs() + floor, "{id}: integrated {k} {v_c} (dt {dt_coarse}) vs {v_f} (finer)");
    }
}

/// One `TOL` line per row of the method.md measurement table, measured on the
/// fine pair (0.025 s vs 0.0125 s). Step 3 derives Tolerance::numerical() from
/// these lines and nothing else.
fn print_measurements(id: &str, mid: &Metrics, fine: &Metrics) {
    let cols: std::collections::BTreeSet<&String> =
        fine.peak.keys().chain(fine.min.keys()).chain(fine.final_values.keys()).collect();
    let mut rel_max: f64 = 0.0;
    let mut t_ext_max: f64 = 0.0;
    for col in cols {
        let d_max = (mid.peak[col].0 - fine.peak[col].0).abs();
        let d_min = (mid.min[col].0 - fine.min[col].0).abs();
        let d_fin = (mid.final_values[col] - fine.final_values[col]).abs();
        let abs = d_max.max(d_min).max(d_fin);
        println!("TOL {id} abs {col} {abs:.6e}   # max {d_max:.3e} min {d_min:.3e} final {d_fin:.3e}");
        for (v_f, d) in [(fine.peak[col].0, d_max), (fine.min[col].0, d_min), (fine.final_values[col], d_fin)] {
            if v_f.abs() > abs {
                rel_max = rel_max.max(d / v_f.abs());
            }
        }
        // Extremum times count only where the extremum differs from the final value
        // (the compare rule): a flat trace has no meaningful time of extremum.
        for (ext_m, ext_f) in [(mid.peak[col], fine.peak[col]), (mid.min[col], fine.min[col])] {
            if (ext_f.0 - fine.final_values[col]).abs() > abs {
                let dt_ext = (ext_m.1 - ext_f.1).abs();
                t_ext_max = t_ext_max.max(dt_ext);
                println!("TOL {id} t_ext {col} {dt_ext:.6e}");
            }
        }
    }
    println!("TOL {id} rel {rel_max:.6e}");
    println!("TOL {id} extremum_time_s {t_ext_max:.6e}");
    for (ev, t_f) in &fine.first_event_time {
        println!("TOL {id} event {ev} {:.6e}", (mid.first_event_time[ev] - t_f).abs());
    }
    for (k, v_f) in &fine.integrated {
        let d = (mid.integrated[k] - v_f).abs();
        println!("TOL {id} integrated {k} {d:.6e}   # rel {:.3e}", d / v_f.abs().max(f64::MIN_POSITIVE));
    }
    println!("TOL {id} residual_mass_kg {:.6e}", mid.max_abs_mass_residual_kg.max(fine.max_abs_mass_residual_kg));
    println!("TOL {id} residual_energy_j {:.6e}", mid.max_abs_energy_residual_j.max(fine.max_abs_energy_residual_j));
}

#[test]
fn convergence_across_three_time_steps() {
    let num = Tolerance::numerical();
    let phys = Tolerance::physical();
    for id in CONVERGENCE_CASES {
        let case = standard_cases().into_iter().find(|c| c.id == id).unwrap_or_else(|| panic!("case {id} missing"));
        let [m50, m25, m125] = CONVERGENCE_DTS.map(|dt| run_case(&case, dt).metrics());
        for (m, dt) in [(&m50, 0.05), (&m25, 0.025), (&m125, 0.0125)] {
            assert!(!m.nonfinite, "{id}: non-finite metrics at dt={dt}");
            assert!(!m.first_event_time.is_empty(), "{id}: no events at dt={dt}");
            // Conservation is a roundoff property, not a discretisation one: every run
            // sits under the Task 6 floors, or the ledger has a bookkeeping defect.
            assert!(m.max_abs_mass_residual_kg <= TOL_NUMERICAL_MASS_KG, "{id} dt={dt}: mass residual {}", m.max_abs_mass_residual_kg);
            assert!(m.max_abs_energy_residual_j <= TOL_NUMERICAL_ENERGY_J, "{id} dt={dt}: energy residual {}", m.max_abs_energy_residual_j);
        }
        print_measurements(id, &m25, &m125);

        // (a) The same supported event set at all three steps; times within the quantisation.
        assert_event_sets_match(id, &m50, 0.05, &m25, 0.025, num.event_time_s);
        assert_event_sets_match(id, &m25, 0.025, &m125, 0.0125, num.event_time_s);
        assert_event_sets_match(id, &m50, 0.05, &m125, 0.0125, num.event_time_s);
        // (b) Fine pair inside the numerical class: every final value, every extremum
        //     value AND time (compare's rule: the time only where the extremum differs
        //     from the final value), both residuals; plus every integrated transfer.
        //     This is the self-consistency check of Step 3's derivation.
        let v = compare(&m125, &m25, &num);
        assert!(v.is_empty(), "{id}: 0.025 s vs 0.0125 s outside Tolerance::numerical():\n{}", v.join("\n"));
        assert_integrated_match(id, &m25, 0.025, &m125, num.rel);
        // (c) Production-step acceptance: the 0.05 s run against the 0.025 s run inside
        //     the physical class (abs floors 10x numerical, rel 5e-2). First-order
        //     convergence makes the coarse-pair difference about equal to the numerical
        //     tolerance itself, so the numerical class is not the right gate here; the
        //     event quantisation in (a) and the integrated transfers are still checked.
        let v = compare(&m25, &m50, &phys);
        assert!(v.is_empty(), "{id}: production step 0.05 s not accepted against 0.025 s:\n{}", v.join("\n"));
        assert_integrated_match(id, &m50, 0.05, &m25, phys.rel);
        // (d) Nothing compared at one dt may be absent at another.
        for (name, c, f) in [("peak", &m50.peak, &m125.peak), ("min", &m50.min, &m125.min)] {
            assert_eq!(c.keys().collect::<Vec<_>>(), f.keys().collect::<Vec<_>>(), "{id}: {name} key sets differ between 0.05 and 0.0125");
        }
    }
}
```

- [ ] **Step 2: Run the test and capture the measurement table**

Run: `cargo test --release -p bwr-core --test numerics convergence_across_three_time_steps -- --nocapture 2>&1 | tee /tmp/pkg03-convergence.txt`
Expected: the `TOL` lines are printed for both cases (one `abs` line per compared column covering max, min and final; one `t_ext` line per extremum that differs from the final value; `rel`; `extremum_time_s`; one `event` line per event key; one `integrated` line per ledger key; `residual_mass_kg`; `residual_energy_j`). Checks (a), (c), (d) and the residual floors must PASS: they do not depend on Step 3. Check (b) may fail under the provisional Package 2 values of `Tolerance::numerical()`; that is what Step 3 replaces. Any other failure is a non-converged model element (an event present at one dt and not another, a production-step metric outside the physical class, a residual above the floor): find the explicit update responsible (use the audit list above), make it semi-implicit or shorten its internal substep, and rerun. Do not widen a tolerance and do not touch the floors. The table goes in the commit body.

- [ ] **Step 3: Derive `Tolerance::numerical()` entirely from the table**

Master r2 rule: the Package 2 values of `Tolerance::numerical()` are provisional; this step REPLACES every field with a value derived from the `TOL` lines, 2x the measured 0.025 s vs 0.0125 s difference, maximum over the two cases. Two floors and the conservation tolerances are the only values not taken from the table, each justified in one sentence below. Rules, one per table row:

- `abs[col]` = 2 x max over cases of `TOL <case> abs <col>` (this one measurement already covers that column's max, min and final differences, so a minimum can never receive an unrelated tolerance). Measured 0 -> `1e-9`: the roundoff floor, so the comparison never demands bit equality of physics.
- `rel` = 2 x max over cases of `TOL <case> rel`; if any `TOL <case> integrated <key>` relative difference (the `# rel` annotation) is larger than that, `rel` = 2 x the largest such value instead (integrated transfers are compared with `rel` in the test).
- `event_time_s` = max(2 x max over cases and events of `TOL <case> event <key>`, 0.05): the floor is one production step, because protection is evaluated once per step and no event time can be resolved finer than the step it is detected in.
- `extremum_time_s` = max(2 x max over cases of `TOL <case> extremum_time_s`, 0.05): the same quantisation floor.
- `residual_mass_kg` = `TOL_NUMERICAL_MASS_KG`, `residual_energy_j` = `TOL_NUMERICAL_ENERGY_J` (Task 6): conservation is f64 roundoff, not discretisation, so these are not scaled from the study; the test asserts every measured `residual_*` line sits under them.

Write the derived values as named constants in `crates/bwr-core/src/validation.rs` (the register keys) and build the constructor from them. `<2 x M>` is not code: the executor writes one plain `f64` literal with the measured value in the trailing comment.

```rust
/// Everything below is 2x the measured 0.025 s vs 0.0125 s difference from the
/// Package 3 convergence study (tests/numerics.rs::convergence_across_three_time_steps),
/// maximum over fp_turbine_trip and fp_manual_scram; measured values in the
/// trailing comments, the full table in docs/validation/method.md.
pub const TOL_NUMERICAL_REL: f64 = <2 x M_rel>;                                   // measured <M_rel>
pub const TOL_NUMERICAL_EVENT_TIME_S: f64 = <max(2 x M_event, 0.05)>;             // measured <M_event>; floor one production step
pub const TOL_NUMERICAL_EXTREMUM_TIME_S: f64 = <max(2 x M_extremum, 0.05)>;       // measured <M_extremum>; floor one production step
/// One entry per column that Metrics compares (every `TOL <case> abs <col>` line);
/// a measured 0 gets the 1e-9 roundoff floor.
pub const TOL_NUMERICAL_ABS: &[(&str, f64)] = &[
    ("pressure_pa", <2 x M>), // measured <M> Pa
    // ... one line per column, in TRACE_COLUMNS order
];

impl Tolerance {
    /// Numerical class: see the constants above and method.md.
    pub fn numerical() -> Self {
        Self {
            rel: TOL_NUMERICAL_REL,
            abs: TOL_NUMERICAL_ABS.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
            event_time_s: TOL_NUMERICAL_EVENT_TIME_S,
            extremum_time_s: TOL_NUMERICAL_EXTREMUM_TIME_S,
            residual_mass_kg: TOL_NUMERICAL_MASS_KG,
            residual_energy_j: TOL_NUMERICAL_ENERGY_J,
        }
    }
}
```

If Package 2 wrote `Tolerance::physical()` with literal abs floors, rebuild it from `numerical()` (`abs` x 10, `rel` 5e-2, event 2 s, extremum 10 s, residuals unchanged) so it follows the measurement. If any 2x value is LOOSER than the Package 2 provisional value, the measured value wins and the change is called out in method.md; that is measurement, not tolerance-widening.

Register rows (same task): one `docs/model/parameters.csv` row per constant (`validation.TOL_NUMERICAL_REL`, `validation.TOL_NUMERICAL_EVENT_TIME_S`, `validation.TOL_NUMERICAL_EXTREMUM_TIME_S`) and one per `TOL_NUMERICAL_ABS` entry (`validation.TOL_NUMERICAL_ABS[<col>]`), status `derived`, notes `2x measured dt-halving difference <M>, Package 3 convergence study, commit <hash>` (or `roundoff floor` / `quantisation floor` where a floor applied). Run `cargo test --release -p bwr-core --test register`.

Rerun: `cargo test --release -p bwr-core --test numerics convergence_across_three_time_steps -- --nocapture`
Expected: PASS on all four checks; the derivation guarantees (b) by construction, so a failure here means the table was transcribed wrongly. Then run `cargo run --release -p bwr-core --example validation` in compare mode and KEEP its output: it is expected to report violations against the Package 2 baselines (this package changed physics and newly populates two columns). Do not write baselines here; Task 9 saves the report, classifies every change and regenerates once.

- [ ] **Step 4: Document**

Append to `docs/validation/method.md`:

```markdown
## Time-step convergence and the numerical tolerance class

`fp_turbine_trip` and `fp_manual_scram` are run at plant steps of 0.05
(production), 0.025 and 0.0125 s with the kinetics substep chosen adaptively
(`kinetics.rs`). `tests/numerics.rs::convergence_across_three_time_steps`
enforces, per case: (a) the same set of supported events at all three steps,
each time agreeing within max(dt_coarse, event tolerance) because protection
is evaluated once per step; (b) the 0.025 s run against the 0.0125 s run inside
`Tolerance::numerical()` (final values, extremum values and times, residuals)
plus every integrated transfer within the relative tolerance; (c) the 0.05 s
production step accepted against the 0.025 s run inside `Tolerance::physical()`
(abs floors 10x numerical: first-order convergence makes the coarse-pair
difference about equal to the numerical tolerance itself); (d) mass and energy
residuals under the conservation floors at every step size.

`Tolerance::numerical()` is derived entirely from the table below: every field
is 2x the measured 0.025 s vs 0.0125 s difference, maximum over the two cases
(measured <date>, commit <hash>). Two floors are justified separately: a
measured difference of 0 gets 1e-9 (f64 roundoff; the comparison never demands
bit equality), and event and extremum times get at least one production step,
0.05 s, because an event cannot be resolved finer than the step it is detected
in. The conservation tolerances are not scaled from this table: they bound f64
roundoff on the ledger sums (eps 2.2e-16 x the largest standard-case throughput
of about 1e6 kg / 1e14 J, random-walked over 1e6 steps, is about 2e-7 kg /
2e1 J), so 1e-3 kg / 1e3 J leave two decades of margin while staying below
1e-8 of the stored inventory; the test asserts every measured residual lies
under them.

| metric | column / key | measured (0.025 vs 0.0125, max over cases) | tolerance | derivation |
|---|---|---|---|---|
| abs floor (max, min and final) | <one row per compared column> | | | 2x; 0 -> 1e-9 |
| relative | all columns and integrated transfers | | `TOL_NUMERICAL_REL` | 2x |
| extremum time | max over columns with a distinct extremum | | `TOL_NUMERICAL_EXTREMUM_TIME_S` | 2x; floor 0.05 s |
| event time | <one row per event key> | | `TOL_NUMERICAL_EVENT_TIME_S` | 2x; floor 0.05 s |
| integrated transfer | <one row per ledger key> | | within `rel` | must sit under `rel`, else `rel` raised to 2x |
| mass residual | all runs | | `TOL_NUMERICAL_MASS_KG` = 1e-3 kg | roundoff floor (Task 6) |
| energy residual | all runs | | `TOL_NUMERICAL_ENERGY_J` = 1e3 J | roundoff floor (Task 6) |

`collapsed_level_m` and `unsupported` trace columns are populated from
Package 3 (Tasks 2 and 1).
```

- [ ] **Step 5: Commit**

```bash
git add crates/bwr-core/tests/numerics.rs crates/bwr-core/src/validation.rs docs/validation/method.md docs/model/parameters.csv
git commit -m "test(core): time-step convergence; numerical tolerances set from measurement

<paste the TOL table from Step 2>

Convergence enforced across 0.05 / 0.025 / 0.0125 s: matching event sets with
step quantisation, fine pair inside Tolerance::numerical(), production step
inside Tolerance::physical(), integrated transfers and residual floors.
Every field of Tolerance::numerical() is 2x the measured dt-halving
difference (floors: 1e-9 roundoff, 0.05 s quantisation; conservation floors
from Task 6); provisional Package 2 values replaced.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 9: Reviewed baseline regeneration

**Files:**
- Create: `docs/validation/reports/<date>-pkg03.md` (compare-mode output plus the classification table; master baseline regeneration protocol step 1)
- Modify: `docs/validation/baseline/*.csv` (EVERY case: `collapsed_level_m` and `unsupported` are newly populated, so every trace differs from Package 2's)
- Modify: `docs/validation/cases.csv` notes column
- `docs/validation/results.md` is not created here (Package 10); `docs/validation/findings.md` only if the Finding policy fires.

**Interfaces:**
- Consumes: `examples/validation.rs` compare mode (default) and write mode (`--write`, as defined by Package 2), `Tolerance::numerical()` from Task 8.

This task follows the master's baseline regeneration protocol in order: compare first and save the report, classify every change, pass the package's own physical gates, write exactly once, commit report and baselines together. Package 2's baselines stay in git history as the "before"; nothing here overwrites them until Step 4.

- [ ] **Step 1: Compare and save the report**

Run: `mkdir -p docs/validation/reports && cargo run --release -p bwr-core --example validation > docs/validation/reports/<date>-pkg03.md`
Then prepend to the file a header: `# Package 3 baseline comparison, <date>, commit <hash>: Package 2 baselines vs the Package 3 engine, Tolerance::numerical() from Task 8`.
Expected: a per-case list of metric violations. There is no clean-comparison expectation: this package changed physics and populates two columns. Changes expected, with the physical reason:
- cases that scram (`fp_manual_scram`, `fp_msiv_closure`, any scram case): power falls one step (0.05 s) earlier (Task 4).
- every case: turbine `p_mech` and condenser load are evaluated one phase later within the step (Task 4 reorder); at steady state this is a sub-tolerance re-timing, not a level change.
- any case in which the level approaches the top of active fuel: fuel temperature higher and `CORE UNCOVERED` alarm earlier (Task 2).
- long isolated cases with IC in service for hours: pool temperature capped at 100 degC (Task 5).
- cases that previously touched the 1 000 kg floor: now end in an `unsupported` flag (Task 6).
- every case: the `unsupported` and `collapsed_level_m` columns are non-zero for the first time (Tasks 1 and 2): a schema-population change, never a physical one.
- source startups from zero power: `period_s` finite throughout (Task 3, defect 12).
Cases that only exercise steady operation (`fp_steady`, startup) must show no physical violation; a violation there is a defect to investigate before anything is written.

- [ ] **Step 2: Classify every change**

Append to the report a table `case, metric, before, after, class, reason (task)` with one row per violated metric and one row per newly populated column, where `class` is one of `physical` (reason from the Step 1 list, mandatory), `schema-population` (`collapsed_level_m`, `unsupported`) or `numerical` (a sub-tolerance re-timing such as the Task 4 phase change; it must be below `Tolerance::numerical()` for that metric, or it is not numerical). A violation with no reason in the Step 1 list is a new finding: stop, add a test in `tests/numerics.rs` that captures it, and fix it, or record it under the Finding policy, before regenerating. The same table is the commit body.

- [ ] **Step 3: Pass the package's physical gates**

Run: `cargo test --release -p bwr-core`
Expected: all green BEFORE any baseline is written (the conservation, rejection, edge-state and convergence tests of Tasks 1 to 8 are the independent gates). If anything fails, the regeneration does not happen.

- [ ] **Step 4: Regenerate once**

Run: `cargo run --release -p bwr-core --example validation -- --write`
Expected: every baseline CSV rewritten; `git status` shows every case modified. No trace is expected to come back unchanged, because two columns are newly populated; determinism is Package 2's run-twice property, not something this diff shows. `--write` runs exactly once for this reviewed change set: if review of the report asks for a further model change, go back to Step 1 against the committed baselines and produce a new report before writing again.

Run: `cargo run --release -p bwr-core --example validation`
Expected: no violations.

Run: `cargo test --release -p bwr-core && npm --prefix ui run typecheck && npm --prefix ui run build`
Expected: all green.

- [ ] **Step 5: Commit the report and the baselines together**

```bash
git add docs/validation/reports/<date>-pkg03.md docs/validation/baseline docs/validation/cases.csv
git commit -m "chore(validation): regenerate baselines after Package 3 corrections

Regression evidence only, not physical truth. Report with the compare-mode
output and the classification of every change (physical / schema-population /
numerical) in docs/validation/reports/<date>-pkg03.md. Gates: cargo test green
before --write; --write run once.

| case | metric | before | after | class | reason |
|---|---|---|---|---|---|
<paste table from Step 2>

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

## Acceptance gate (verbatim from the spec)

"Tests demonstrate the original defect before correction. Mass and energy residuals close to the stated numerical tolerance; domain errors are visible."

Checklist against the gate:

- [ ] Each of Tasks 2 to 6 recorded a baseline demonstration (printed line or failure message) in its commit body before the fix, covering master defects 1, 2, 3, 4 (P3 part), 5, 12 and 13.
- [ ] `tests/conservation.rs` shows `mass_clipped_kg == 0`, `energy_clipped_j == 0`, `|mass_residual| <= TOL_NUMERICAL_MASS_KG` and `|energy_residual| <= TOL_NUMERICAL_ENERGY_J` through a drained-vessel transient (normal and inventory-bounded steps), through a run with feed, steam, vent, RWCU and IC transfers simultaneously active, and across a rejected step; `coupled_run_hotwell_gain_equals_vessel_release` shows the hotwell's measured gain equal to the vessel's measured release (state to state, the ledger as third observer) and was shown to fail with each consumer miswired; `eos_rejection_leaves_hotwell_and_pool_unchanged` shows a rejected step moves nothing into the hotwell or any IC pool.
- [ ] Every rejection rule in master r2 ("what commits on failure") has its own test: `Kinetics` unchanged on Err (Task 3), `Core::step` leaves xenon/decay/fuel un-advanced (Task 3), `Vessel::step` rolls back on EOS Err and returns zero flows (Task 6), exhaustion scales outflows and records the deficit (Task 6).
- [ ] `Snapshot.unsupported`, the `unsupported` alarm, and `Sim.log` `domain error:` entries make every domain error visible; `LoadScenario` is the only reset; errors raised during initialisation survive `finalize_init` re-stamped at t = 0, and an initial vessel state outside the EOS envelope is such an error, not a panic (`invalid_initial_vessel_is_a_retained_domain_error_not_a_panic`).
- [ ] Every consumer of a transfer uses the applied (post-allocation) value: `steam.w_tcv`, `steam.w_bypass` scaled; condenser fed `flows.w_liquid_out`; the RWCU setpoint is never mutated.
- [ ] Every new struct/enum carries the master Derives rule (`DomainError`: Clone, Debug, PartialEq, Serialize, Deserialize; `Unsupported`: Clone, Debug, Serialize, Deserialize, camelCase; `IcStep`: Clone, Copy, Debug, Default, PartialEq).
- [ ] The `collapsed_level_m` and `unsupported` trace columns (schema v2) are populated, and `method.md` no longer lists them as pending.
- [ ] Convergence at 0.05 / 0.025 / 0.0125 s is enforced for `fp_turbine_trip` and `fp_manual_scram`: matching event sets with step quantisation, the fine pair inside `Tolerance::numerical()` (every extremum value and time, finals, residuals, integrated transfers), the production step accepted inside `Tolerance::physical()`, residuals under the conservation floors at every dt; every field of `Tolerance::numerical()` is derived from the measurement table in `method.md` (2x measured; roundoff and quantisation floors and the conservation floors justified there).
- [ ] Baselines were regenerated once per the master protocol: compare-mode report saved under `docs/validation/reports/`, every change classified physical / schema-population / numerical, package gates green before `--write`, report and baselines committed together; no unchanged trace was expected and no tolerance was widened beyond measurement.
