# Package 3: Numerical and Conservation Defects — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Read `docs/superpowers/plans/2026-10-20-refinement-master.md` (revision r2) first; its interface contract wins over anything here. This plan is reconciled to master r2 (rejection semantics, transfer allocation, no kinetics work cap, `KIN_N_FLOOR`, trace schema v2).

**Goal:** Make every known numerical and conservation defect in `crates/bwr-core` visible through a failing test, then correct it so that mass and energy close to numerical tolerance and unsupported states are reported as explicit domain errors instead of silent clamps.

**Architecture:** Add `src/domain.rs` (`DomainError`) and plumb it through `Kinetics`, `Core`, `Vessel`, and `Sim` so each subsystem rejects states outside its envelope, with the rejection semantics fixed by master r2 ("what commits on failure"). Fix the five model defects (core coverage double-correction, kinetics floor collapse, zero-power period NaN, one-step rod reactivity lag, IC pool energy discard) and the two silent clamps (vessel 1 000 kg floor, EOS bisection range), and make every downstream consumer use the APPLIED transfers from the vessel's allocation step (master defect 13). Finish with edge-state and time-step convergence tests, measured numerical tolerances, and a reviewed baseline regeneration.

**Tech Stack:** Rust 2021, serde/serde_json (dev), TypeScript mirror in `ui/src/sim/types.ts`. No new dependencies.

**Depends on:** Packages 1 and 2 complete. From Package 2: `src/ledger.rs` (`Ledger` with `integrated`, `BoundaryFlows` with the IC ports `w_ic_steam`/`w_ic_return`/`h_ic_return`), `src/faults.rs` (`Sim.faults: Vec<Fault>`), `src/validation.rs` (`run_case`, `standard_cases`, `Trace::metrics`, trace schema v2 with 59 columns), `Sim.log`, `Sim.ledger`, `Sim::last_flows() -> &BoundaryFlows` (the applied flows of the most recent step, stored on `Sim`), `Sim::finalize_init`, `Vessel::step -> BoundaryFlows`, `tests/common/mod.rs` (owns `run()`), `examples/validation.rs` with compare and `--write` modes, and `docs/validation/baseline/*.csv`. From Package 1: `docs/model/parameters.csv` with its locked header.

## Global Constraints

All constraints in the master plan apply. Package-specific:

- "Tests demonstrate the original defect before correction." Every defect task has a Step that runs a demonstration test against the baseline and records its output before the fix is written.
- "Mass and energy residuals close to the stated numerical tolerance; domain errors are visible."
- "Replace silent collapse to the neutron floor with a solver strategy or explicit domain rejection appropriate to the supported transient."
- "Replace the vessel's 1,000 kg inventory floor and other silent physical clamps with bounded transfers, explicit residual accounting, or an unsupported-state outcome."
- "Compare results at 0.05, 0.025, and 0.0125 second plant steps, with suitable kinetics substeps. Require converged event timing and extrema for supported tests."
- "No previous-test failure is concealed by updating expected output." Baselines are regenerated once, in Task 9, with the before/after metric table in the commit body.
- Commands: `cargo test --release -p bwr-core`, `npm --prefix ui run typecheck`, `cargo run --release -p bwr-core --example validation`.

## Names introduced by this package (beyond the master contract)

| Name | Where | Purpose |
|---|---|---|
| `DomainError::reason(&self) -> String` | `src/domain.rs` | Human-readable text for alarms and logs |
| `Sim::domain_error(&mut self, e: DomainError)` (pub) | `src/sim.rs` | Dedup-by-variant push, log, alarm |
| `KIN_H_MAX`, `KIN_PROMPT_SAFETY`, `KIN_EXCESS_MAX`, `KIN_N_FLOOR` | `src/kinetics.rs` | Adaptive substep rule, supported reactivity ceiling (physical, no work cap), period-estimator floor |
| `Vessel.exhausted: Option<f64>` | `src/vessel.rs` | Deficit kg when outflows were bounded this step |
| `Vessel.last_error: Option<DomainError>` | `src/vessel.rs` | EOS rejection, drained by `Sim::step` |
| `T_EOS_MIN`, `T_EOS_MAX` | `src/vessel.rs` | EOS bisection envelope, 1.0 and 349.5 degC |
| `solve_eos(..) -> Result<Eos, DomainError>` | `src/vessel.rs` | Range-checked EOS |
| `IcStep.pool_cooling_w` | `src/ic.rs` | Fifth field on the master's `IcStep`: idle pool cooling reported as a boundary flow |
| `IcTrain::step(dt, t_sat, h_fg, steam_available)` | `src/ic.rs` | New `h_fg` parameter (J/kg at vessel pressure) |
| `CP_WATER`, `H_FG_ATM`, `POOL_BOIL_T`, `POOL_IDLE_COOLING_K_PER_S` | `src/ic.rs` | Named pool constants |
| `cmd_is_finite(&Command) -> bool` | `src/sim.rs` | Rejects non-finite numeric commands (master non-finite inputs rule) |
| `assert_finite(&Snapshot)` | `tests/common/mod.rs` | Appended to Package 2's file (`run()` already lives there; the file is NOT re-created) |

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
- Modify: `crates/bwr-core/tests/common/mod.rs` (owned by Package 2; this task only APPENDS `assert_finite`)
- Modify: `crates/bwr-core/src/validation.rs` (populate the `unsupported` trace column)
- Create: `crates/bwr-core/tests/numerics.rs`

**Interfaces:**
- Consumes: `Sim.log: Vec<(f64, String)>`, `Sim::finalize_init`, `tests/common/mod.rs::run`, trace schema v2 from Package 2 (the `unsupported` column exists and is emitted as 0.0).
- Produces: `DomainError` (master contract, six variants), `DomainError::reason()`, `Sim.domain_errors: Vec<(f64, DomainError)>`, `pub fn Sim::domain_error(&mut self, e: DomainError)`, `api::Unsupported { since_s, reason }`, `Snapshot.unsupported: Option<Unsupported>`, alarm id `unsupported` severity `Trip`.

- [ ] **Step 1: Write the failing test**

Append to Package 2's `crates/bwr-core/tests/common/mod.rs` (do not re-create the file; `run()` already exists there):

```rust
/// Fields that are legitimately `null` in the JSON contract (see ui/src/sim/types.ts).
const NULLABLE: [&str; 4] = [
    "snapshot.scramReason",
    "snapshot.neutronics.periodS",
    "snapshot.rods.selected",
    "snapshot.unsupported",
];

/// Panics if any number in the snapshot is NaN/inf. serde_json turns a non-finite
/// f64 into `null`, so an unexpected `null` is treated as non-finite too.
pub fn assert_finite(s: &Snapshot) {
    fn walk(v: &serde_json::Value, path: &str) {
        match v {
            serde_json::Value::Number(n) => {
                assert!(n.as_f64().map_or(true, f64::is_finite), "non-finite value at {path}");
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
    assert!(sim.log.iter().any(|(_, t)| t.starts_with("DOMAIN:")));

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
        self.log.push((self.time, format!("DOMAIN: {}", e.reason())));
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
- Produces: `Kinetics::advance(&mut self, rho: f64, s: f64, dt: f64) -> Result<usize, DomainError>` (returns substeps used; state untouched on Err); `Core::step(..) -> Result<f64, DomainError>` (on Err: rods already moved by `step_rods`, xenon/decay/fuel NOT advanced); constants `KIN_H_MAX = 1.0e-3`, `KIN_PROMPT_SAFETY = 0.25`, `KIN_EXCESS_MAX = 0.2`, `KIN_N_FLOOR = 1e-12`. There is NO work cap.

**Baseline defects (verified in code; master defects 2 and 12):**
- Defect 2: `step()` computes `lhs = 1 - dt*(rho - BETA)/GEN_TIME - ...`. With the fixed 1 ms substep used by `Core::step`, `lhs <= 0` once `rho` exceeds about 0.046 dk/k, and `n1 = (rhs/lhs).max(1e-14)` then silently drives power to the floor instead of growing.
- Defect 12: `Kinetics::equilibrium(0.0)` followed by `step()` computes `let ip = (n1 / n0).ln() / dt;` with `n0 = 0`, giving an infinite `inv_period` (ln of inf), then NaN once a later ratio is 0/0 or inf is mixed into the smoothing lag.

**Envelope decision (master r2):** the supported ceiling is the physical constant `KIN_EXCESS_MAX = 0.2` dk/k above `BETA`, recorded as an assumption in `parameters.csv`. There is NO substep work cap: a cap would reject the legitimate long calls in the existing unit tests (100 s and 120 s advances) and any threshold expressed in substeps would change meaning with `dt`. The substep count at fixed `h` is linear in `dt` and bounded in practice by `Sim`'s 0.05 s plant step (50 substeps at `rho <= BETA`, up to 5 000 at the `KIN_EXCESS_MAX` ceiling where `h = 0.25*GEN_TIME/0.2 = 5e-5 s`).

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
#[test]
fn kinetics_rejects_prompt_supercritical_instead_of_collapsing() {
    // Above prompt critical but inside the solver envelope: power grows, no clamp.
    let mut k = Kinetics::equilibrium(1.0);
    let substeps = k.advance(0.05, 0.0, 0.05).expect("rho = 0.05 is inside the envelope");
    assert!(substeps > 50, "prompt-limited substeps expected, got {substeps}");
    assert!(k.n > 1.0, "power must grow above prompt critical, got {}", k.n);
    assert!(k.inv_period > 0.0);

    // Beyond the physical envelope: explicit rejection, state untouched
    // (rejection semantics: Kinetics unchanged on Err).
    let mut k = Kinetics::equilibrium(1.0);
    let before = k.clone();
    let err = k.advance(0.3, 0.0, 0.05).unwrap_err();
    assert!(matches!(err, DomainError::PromptSupercritical { rho } if (rho - 0.3).abs() < 1e-12));
    assert_eq!(k.n, before.n);
    assert_eq!(k.c, before.c);
    assert_eq!(k.inv_period, before.inv_period);

    // Ordinary operation uses the 1 ms substep.
    let mut k = Kinetics::equilibrium(1.0);
    assert_eq!(k.advance(0.0, 0.0, 0.05).unwrap(), 50);

    // No work cap: a 100 s call at rho = 0 is 100_000 substeps, as at baseline.
    let mut k = Kinetics::equilibrium(1.0);
    assert_eq!(k.advance(0.0, 0.0, 100.0).unwrap(), 100_000);
    assert!((k.n - 1.0).abs() < 1e-6);
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

Run: `cargo test --release -p bwr-core --test numerics kinetics_rejects_prompt_supercritical`
Expected: compile error `this method takes 4 arguments but 3 arguments were supplied`.

- [ ] **Step 4: Implement the adaptive solver**

Replace the whole of `crates/bwr-core/src/kinetics.rs` above `#[cfg(test)]`:

```rust
//! Point reactor kinetics with six delayed neutron groups.

use crate::domain::DomainError;
use crate::params::{BETA, BETA_FRAC, GEN_TIME, LAMBDA_DN};

/// Largest substep, s. Resolves the fastest delayed group (lambda = 3.01 1/s) well.
pub const KIN_H_MAX: f64 = 1.0e-3;
/// Substep is limited so that h * (rho - BETA) / GEN_TIME <= this factor.
pub const KIN_PROMPT_SAFETY: f64 = 0.25;
/// Supported reactivity ceiling above BETA, dk/k (a physical envelope, not a work
/// cap). Beyond this the lumped point model is not a credible description and the
/// step is rejected. There is deliberately no substep work cap: long advances
/// (e.g. 100 s at rho = 0) are legitimate and their cost is linear in dt.
pub const KIN_EXCESS_MAX: f64 = 0.2;
/// Floor for the period estimator only: inv_period contribution is 0 when the
/// starting population is below this, and max(n, KIN_N_FLOOR) is used inside the
/// log so the meter stays finite from zero power (baseline defect 12).
pub const KIN_N_FLOOR: f64 = 1e-12;

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
    /// Returns Err, without mutating, if the implicit denominator is nonpositive.
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
        for i in 0..6 {
            self.c[i] = (self.c[i] + dt * BETA * BETA_FRAC[i] / GEN_TIME * n1) / (1.0 + dt * LAMBDA_DN[i]);
        }
        self.n = n1;
        // Period meter: finite by construction (defect 12). No contribution while
        // the population is below the floor; the log uses floored values.
        let ip = if n0 < KIN_N_FLOOR {
            0.0
        } else {
            (n1.max(KIN_N_FLOOR) / n0.max(KIN_N_FLOOR)).ln() / dt
        };
        // Smooth the period meter like a real log-rate amplifier.
        let a = (dt / 1.0).min(1.0);
        self.inv_period += (ip - self.inv_period) * a;
        Ok(())
    }

    /// Advance by `dt` with adaptive substeps:
    /// h = min(KIN_H_MAX, KIN_PROMPT_SAFETY * GEN_TIME / max(rho - BETA, 0));
    /// substeps = ceil(dt / h), with no work cap. Returns the number of substeps
    /// used, or Err (state untouched) when `rho - BETA > KIN_EXCESS_MAX`, an input
    /// is not finite, or the implicit denominator is still nonpositive.
    pub fn advance(&mut self, rho: f64, s: f64, dt: f64) -> Result<usize, DomainError> {
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

Update the unit tests in the same file (long advances must now be looped because the work guard is per call):

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
        self.rods.step(dt);
        self.kin.advance(self.rho_total, SOURCE, dt)?;
```

and change the last line of that function from `q_conv + p * DIRECT_COOLANT_HEAT` to `Ok(q_conv + p * DIRECT_COOLANT_HEAT)`. (Task 4 removes the `self.rods.step(dt)` line.)

The `?` on `kin.advance` sits BEFORE the xenon, decay-heat and fuel-temperature updates, which is exactly the master's rejection semantics: on Err nothing in `Core` beyond the rods commits. The test `core_step_err_leaves_xenon_decay_and_fuel_unadvanced` (Step 2) pins this; do not reorder those updates above the kinetics call. `Sim`-level forcing of the same Err is impractical (`update_reactivity` recomputes `rho_total` from real state each step), so the `Sim` side of the rule, keeping the previous `q_core`, is covered by the match arm below rather than a dedicated test.

In `crates/bwr-core/src/sim.rs`, `Sim::step`, replace

```rust
        self.q_core = self.core.step(dt, t_cool, cov);
```

with

```rust
        // On a kinetics domain error the core state is left untouched for this
        // step and the previous heat input is reused; the plant is flagged.
        self.q_core = match self.core.step(dt, t_cool, cov) {
            Ok(q) => q,
            Err(e) => {
                self.domain_error(e);
                self.q_core
            }
        };
```

- [ ] **Step 6: Run the tests**

Run: `cargo test --release -p bwr-core`
Expected: all pass, including the three kinetics unit tests and `kinetics_rejects_prompt_supercritical_instead_of_collapsing ... ok`.

- [ ] **Step 7: Record the constants**

Append to `docs/model/parameters.csv`, using its header's column order (Package 1), one row each:

| id | value | unit | definition | status | notes |
|---|---|---|---|---|---|
| `KIN_H_MAX` | 1.0e-3 | s | largest kinetics substep | assumed | numerical; resolves lambda_6 = 3.01 1/s with h*lambda = 0.003 |
| `KIN_PROMPT_SAFETY` | 0.25 | - | bound on h*(rho-beta)/Lambda | assumed | numerical; keeps the backward-Euler denominator > 0.75 |
| `KIN_EXCESS_MAX` | 0.2 | dk/k | supported reactivity above beta | assumed | model envelope; lumped point kinetics is not credible beyond this; rejected as DomainError (no work cap exists) |
| `KIN_N_FLOOR` | 1e-12 | - | period-estimator population floor | assumed | numerical; affects the period meter only, never `n` itself |

- [ ] **Step 8: Commit**

```bash
git add crates/bwr-core/src/kinetics.rs crates/bwr-core/src/reactor.rs crates/bwr-core/src/sim.rs crates/bwr-core/tests/numerics.rs docs/model/parameters.csv
git commit -m "fix(kinetics): adaptive substeps, finite period meter, prompt-supercritical rejection

Baseline clamped n to 1e-14 once rho exceeded ~0.046 dk/k at the fixed 1 ms
substep, and the period meter went non-finite from zero power (defect 12):
<paste both printed lines from Step 1>. The solver now shortens the substep
so h*(rho-beta)/Lambda <= 0.25 (no work cap), rejects rho - beta > 0.2 dk/k
as DomainError::PromptSupercritical without mutating state, and floors the
period estimator at KIN_N_FLOOR.

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
- Produces: `pub fn Core::step_rods(&mut self, dt: f64)`. `Core::step` no longer moves rods. `Sim::step` implements the P3 subset of the master's shared step order: `core.step_rods` → `core.update_reactivity` → `core.step` → steam valves and REQUESTED flows → IC → feedwater → `vessel.step` (the transfer-allocation point) → scale consumers to APPLIED flows → `turbine.step` → `cond.step` → `time += dt` → `protection()` → `ledger.record` → drain vessel errors (Task 6).

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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --release -p bwr-core --test numerics rod_reactivity_has_no_one_step_lag -- --nocapture`
Expected: FAIL with `rho_rods used by kinetics <x> but rods are at <y>` where `|x - y|` is on the order of 1e-3 dk/k (the worth of 0.8 positions on every rod). Paste the message into the commit body.

- [ ] **Step 3: Implement the reorder**

In `crates/bwr-core/src/reactor.rs`, add before `pub fn step(`:

```rust
    /// Move the rods by `dt`. Called by `Sim::step` before `update_reactivity` so the
    /// kinetics see the positions of this step, not the previous one.
    pub fn step_rods(&mut self, dt: f64) {
        self.rods.step(dt);
    }
```

and delete the line `self.rods.step(dt);` from `Core::step`.

In `crates/bwr-core/src/sim.rs`, `Sim::step`, replace

```rust
        // --- reactor core
        self.core.update_reactivity(self.vessel.alpha_core, t_cool);
        let cov = self.vessel.core_coverage();
```

with

```rust
        // --- reactor core: move rods first so reactivity matches this step's positions.
        self.core.step_rods(dt);
        self.core.update_reactivity(self.vessel.alpha_core, t_cool);
        let cov = self.vessel.core_coverage();
```

- [ ] **Step 4: Run the tests**

Run: `cargo test --release -p bwr-core`
Expected: all pass. `turbine_trip_rides_through_on_bypass` and `manual_scram_shuts_down_without_isolation` still pass (the fix makes insertion act 0.05 s earlier, inside their tolerances).

- [ ] **Step 5: Commit**

```bash
git add crates/bwr-core/src/reactor.rs crates/bwr-core/src/sim.rs crates/bwr-core/tests/numerics.rs
git commit -m "fix(core): rod motion precedes reactivity evaluation within a step

update_reactivity ran before rods moved, so kinetics used positions from
the previous step (0.8 positions per step during scram). Baseline:
<paste failure message from Step 2>

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 5: Isolation condenser pool energy accounting

**Files:**
- Modify: `crates/bwr-core/src/ic.rs`
- Modify: `crates/bwr-core/src/sim.rs` (`IcTrain::step` call)
- Modify: `docs/model/parameters.csv`

**Interfaces:**
- Consumes: `Vessel::sat().h_fg()`.
- Produces: `pub struct IcStep { q_removed: f64, w_condensate_return: f64, energy_to_pool: f64, pool_boiloff_kg: f64, pool_cooling_w: f64 }` (master's four fields plus `pool_cooling_w`); `IcTrain::step(&mut self, dt, t_sat, h_fg, steam_available) -> IcStep`; constants `CP_WATER = 4186.0`, `H_FG_ATM = 2.257e6`, `POOL_BOIL_T = 100.0`, `POOL_IDLE_COOLING_K_PER_S = 0.0005`.

**Baseline defect (verified in code):** `pool_t += q/(m cp)` can overshoot 100 degC and the excess sensible energy vanishes; `pool_mass = (pool_mass - q/h_fg).max(0.0)` discards energy at depletion; the lagged `heat` keeps reporting removal for ~20 s after the pool is gone; idle cooling `pool_t -= 0.0005*dt` removes energy with no boundary record.

- [ ] **Step 1: Write the failing unit tests**

Append to the `#[cfg(test)] mod tests` in `crates/bwr-core/src/ic.rs` (create the module if absent):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pool_never_overshoots_boiling_and_energy_closes() {
        let mut t = IcTrain::new();
        t.cmd_open = true;
        t.valve_pos = 1.0;
        t.pool_t = 99.99;
        t.pool_mass = 1000.0;
        t.heat = 30.0e6;
        let (m0, t0) = (t.pool_mass, t.pool_t);
        let out = t.step(1.0, 287.0, 1.5e6, true);
        assert!(t.pool_t <= POOL_BOIL_T + 1e-9, "pool overshot to {}", t.pool_t);
        let sensible = m0 * CP_WATER * (t.pool_t - t0);
        let latent = out.pool_boiloff_kg * H_FG_ATM;
        assert!((out.energy_to_pool - (sensible + latent)).abs() < 1.0, "energy_to_pool {} vs {}", out.energy_to_pool, sensible + latent);
        assert!((out.q_removed * 1.0 - out.energy_to_pool).abs() < 1.0);
        assert!(out.pool_boiloff_kg > 0.0);
        assert!((t.pool_mass - (m0 - out.pool_boiloff_kg)).abs() < 1e-9);
        assert!((out.w_condensate_return - out.q_removed / 1.5e6).abs() < 1e-9);
        assert_eq!(out.pool_cooling_w, 0.0, "no idle cooling while loaded");
    }

    #[test]
    fn depleted_pool_removes_no_heat() {
        let mut t = IcTrain::new();
        t.cmd_open = true;
        t.valve_pos = 1.0;
        t.pool_mass = 0.0;
        t.pool_t = POOL_BOIL_T;
        t.heat = 10.0e6; // lagged value left over from before depletion
        let out = t.step(1.0, 287.0, 1.5e6, true);
        assert_eq!(out.q_removed, 0.0);
        assert_eq!(out.pool_boiloff_kg, 0.0);
        assert_eq!(out.energy_to_pool, 0.0);
        assert_eq!(t.heat, 0.0);
        assert!(t.pool_mass >= 0.0);
    }

    #[test]
    fn boiloff_is_bounded_by_remaining_pool() {
        let mut t = IcTrain::new();
        t.cmd_open = true;
        t.valve_pos = 1.0;
        t.pool_mass = 1.0; // 1 kg left: at most 2.257 MJ can be absorbed
        t.pool_t = POOL_BOIL_T;
        t.heat = 30.0e6;
        let out = t.step(1.0, 287.0, 1.5e6, true);
        assert!(out.pool_boiloff_kg <= 1.0 + 1e-12);
        assert!(out.energy_to_pool <= H_FG_ATM + 1.0);
        assert!(t.pool_mass >= 0.0);
    }

    #[test]
    fn idle_pool_cooling_is_reported() {
        let mut t = IcTrain::new();
        t.pool_t = 80.0;
        t.heat = 0.0;
        let out = t.step(1.0, 287.0, 1.5e6, true);
        let expected_w = POOL_IDLE_COOLING_K_PER_S * IC_POOL_MASS * CP_WATER;
        assert!((out.pool_cooling_w - expected_w).abs() < 1.0);
        assert!((t.pool_t - (80.0 - POOL_IDLE_COOLING_K_PER_S)).abs() < 1e-9);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --release -p bwr-core ic::tests`
Expected: compile errors (`POOL_BOIL_T` not found, `step` takes 3 arguments, no field `pool_boiloff_kg`). To demonstrate the overshoot on the baseline, temporarily run only this snippet in a scratch test with the old signature: `t.step(1.0, 287.0, true); println!("{}", t.pool_t)` with the same initial state. Expected printed value above 100 (about 106.8). Paste it into the commit body, then remove the scratch test.

- [ ] **Step 3: Implement**

Replace everything in `crates/bwr-core/src/ic.rs` above `impl Default for IcTrain`:

```rust
//! Isolation condenser system: passive decay heat removal and overpressure protection.
//!
//! Each train condenses reactor steam in tubes submerged in a water pool above
//! containment; condensate drains back to the vessel by gravity once the
//! condensate return valve opens. No pumps or AC power are needed.
//!
//! Pool energy is conserved: heat first raises the pool to boiling, the remainder
//! boils water off, and removal is limited to what the remaining pool can absorb.
//! Package 7 restructures the train (steam and parallel return valves, tube
//! inventory); this file only fixes the energy accounting.

use crate::params::*;
use crate::steam::sat_p;

pub const CP_WATER: f64 = 4186.0;
/// Latent heat at atmospheric pressure (pool boils at 100 degC), J/kg.
pub const H_FG_ATM: f64 = 2.257e6;
pub const POOL_BOIL_T: f64 = 100.0;
/// Idle cooling of a hot pool by its (unmodelled) cooling system, K/s. ASSUMED;
/// reported as `IcStep::pool_cooling_w` so it is a declared boundary flow.
pub const POOL_IDLE_COOLING_K_PER_S: f64 = 0.0005;

/// Heat transfer UA per train, W/K (rated duty at 7.2 MPa with a boiling pool).
fn ua() -> f64 {
    IC_TRAIN_RATED_W / (sat_p(P_RATED).t - 100.0)
}

/// What one train did in one step.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct IcStep {
    /// Heat removed from the vessel, W.
    pub q_removed: f64,
    /// Condensate returned to the vessel, kg/s (= q_removed / h_fg at vessel pressure).
    pub w_condensate_return: f64,
    /// Energy absorbed by the pool this step, J (sensible + latent). Equals q_removed * dt.
    pub energy_to_pool: f64,
    pub pool_boiloff_kg: f64,
    /// Energy removed from the pool by its idle cooling system this step, W.
    pub pool_cooling_w: f64,
}

#[derive(Clone, Debug)]
pub struct IcTrain {
    pub valve_pos: f64,
    pub cmd_open: bool,
    pub heat: f64,
    pub pool_t: f64,
    pub pool_mass: f64,
}

impl IcTrain {
    pub fn new() -> Self {
        Self { valve_pos: 0.0, cmd_open: false, heat: 0.0, pool_t: 40.0, pool_mass: IC_POOL_MASS }
    }

    pub fn pool_level_pct(&self) -> f64 {
        self.pool_mass / IC_POOL_MASS * 100.0
    }

    /// Advance; `t_sat` is the reactor steam temperature (degC) and `h_fg` the
    /// latent heat at vessel pressure (J/kg).
    pub fn step(&mut self, dt: f64, t_sat: f64, h_fg: f64, steam_available: bool) -> IcStep {
        let target = if self.cmd_open { 1.0 } else { 0.0 };
        let d = target - self.valve_pos;
        self.valve_pos += d.clamp(-dt / IC_VALVE_STROKE, dt / IC_VALVE_STROKE);

        let coverage = (self.pool_level_pct() / 40.0).clamp(0.0, 1.0);
        let dt_drive = if steam_available { (t_sat - self.pool_t).max(0.0) } else { 0.0 };
        let target_q = ua() * dt_drive * self.valve_pos.sqrt() * coverage;
        // Tubes drain and fill with steam over ~20 s.
        self.heat += (target_q - self.heat) * (dt / 20.0).min(1.0);

        // The pool can absorb at most: sensible heat to boiling + latent heat of all of it.
        let sensible_cap = (self.pool_mass * CP_WATER * (POOL_BOIL_T - self.pool_t)).max(0.0);
        let latent_cap = self.pool_mass * H_FG_ATM;
        let demand = self.heat * dt;
        let absorbed = demand.min(sensible_cap + latent_cap);
        if absorbed < demand {
            // Sink-limited: the tubes cannot condense more than the pool can take.
            self.heat = absorbed / dt;
        }
        let sensible = absorbed.min(sensible_cap);
        let latent = absorbed - sensible;
        if sensible > 0.0 {
            self.pool_t += sensible / (self.pool_mass * CP_WATER);
        }
        if latent > 0.0 {
            self.pool_t = POOL_BOIL_T;
        }
        let boiloff = latent / H_FG_ATM;
        self.pool_mass -= boiloff;

        // Slow cooling of a hot idle pool by its cooling system (declared boundary flow).
        let pool_cooling_w = if self.heat < 1.0e5 && self.pool_t > 40.0 && self.pool_mass > 0.0 {
            self.pool_t -= POOL_IDLE_COOLING_K_PER_S * dt;
            POOL_IDLE_COOLING_K_PER_S * self.pool_mass * CP_WATER
        } else {
            0.0
        };

        IcStep {
            q_removed: self.heat,
            w_condensate_return: if h_fg > 0.0 { self.heat / h_fg } else { 0.0 },
            energy_to_pool: absorbed,
            pool_boiloff_kg: boiloff,
            pool_cooling_w,
        }
    }
}
```

In `crates/bwr-core/src/sim.rs`, `Sim::step`, replace

```rust
        let steam_avail = !self.vessel.eos.subcooled;
        let q_ic: f64 = self.ic.iter_mut().map(|t| t.step(dt, t_cool, steam_avail)).sum();
```

with

```rust
        let steam_avail = !self.vessel.eos.subcooled;
        let h_fg = self.vessel.sat().h_fg();
        let q_ic: f64 = self.ic.iter_mut().map(|t| t.step(dt, t_cool, h_fg, steam_avail).q_removed).sum();
```

- [ ] **Step 4: Run the tests**

Run: `cargo test --release -p bwr-core`
Expected: all pass, including the four new `ic::tests`. `msiv_closure_scrams_and_isolation_condensers_cool` still passes (pools start at 40 degC with 1.5e6 kg, far from any limit in 180 s).

- [ ] **Step 5: Record the constants**

Append to `docs/model/parameters.csv`:

| name | value | unit | definition | status | notes |
|---|---|---|---|---|---|
| `CP_WATER` | 4186 | J/(kg K) | pool water specific heat | confirmed (physical) | |
| `H_FG_ATM` | 2.257e6 | J/kg | latent heat at 101.325 kPa | confirmed (physical) | |
| `POOL_BOIL_T` | 100 | degC | pool boiling point at atmospheric pressure | confirmed (physical) | pool assumed vented to atmosphere |
| `POOL_IDLE_COOLING_K_PER_S` | 0.0005 | K/s | idle pool cooling rate | assumed | boundary flow; cooling system not modelled (Package 7 revisits) |

- [ ] **Step 6: Commit**

```bash
git add crates/bwr-core/src/ic.rs crates/bwr-core/src/sim.rs docs/model/parameters.csv
git commit -m "fix(ic): conserve pool energy across boiling and depletion

Sensible heating no longer overshoots 100 degC (baseline reached <paste
value from Step 2>), boil-off is bounded by the remaining pool, a depleted
pool removes no heat, and idle cooling is reported as a boundary flow.
IcTrain::step returns IcStep and takes h_fg for the condensate return rate.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 6: Vessel inventory floor and EOS range rejection

**Files:**
- Modify: `crates/bwr-core/src/vessel.rs` (`solve_eos`, `Vessel` fields, `new`, `step`)
- Modify: `crates/bwr-core/src/sim.rs` (`Sim::step` drains vessel errors)
- Create: `crates/bwr-core/tests/conservation.rs`

**Interfaces:**
- Consumes: `BoundaryFlows`, `Ledger` (Package 2), `DomainError`.
- Produces: `pub fn solve_eos(mass: f64, energy: f64) -> Result<Eos, DomainError>`; `T_EOS_MIN = 1.0`, `T_EOS_MAX = 349.5`; `Vessel.exhausted: Option<f64>`; `Vessel.last_error: Option<DomainError>`; `Vessel::step` keeps the master signature `-> BoundaryFlows` and reports what it actually applied; `BoundaryFlows.mass_clipped` and `energy_clipped` are always `0.0` from now on (kept for the Package 2 ledger).

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
    let inp = VesselInputs { q_core: 0.0, q_ic: 0.0, w_feed: 0.0, h_feed: 0.0, w_steam: 0.0, w_liquid_out: 1.0e7, h_feed: 0.0 };
    let flows = v.step(DT, &inp);
    println!("baseline mass after draining step: {} kg, mass_clipped {} kg", v.mass, flows.mass_clipped);
    assert_eq!(v.mass, 1000.0, "baseline floors the inventory");
    assert!(flows.mass_clipped > 0.0, "Package 2 ledger must see the created mass");
}
```

(Remove the duplicated `h_feed` field; `VesselInputs` is `{ q_core, q_ic, w_feed, h_feed, w_steam, w_liquid_out }`.)

Run: `cargo test --release -p bwr-core --test conservation baseline_vessel_floor_creates_mass -- --nocapture`
Expected: PASS with the printed line showing `1000 kg` and a positive `mass_clipped`. Paste it into the commit body, then delete this test.

- [ ] **Step 2: Write the failing tests for the corrected behaviour**

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
    assert_eq!(sim.ledger.mass_clipped_kg, 0.0);
    let res = sim.ledger.mass_residual(sim.vessel.mass);
    assert!(res.abs() < 1e-6 * m0, "mass residual {res} kg");
    assert!(sim.snapshot().unsupported.is_some());
    common::assert_finite(&sim.snapshot());
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test --release -p bwr-core --test conservation`
Expected: compile errors (`no field exhausted`, `solve_eos` returns `Eos` not `Result`).

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

to

```rust
        v.eos = solve_eos(v.mass, v.energy).expect("initial vessel state must be inside the EOS envelope");
```

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

In `crates/bwr-core/src/sim.rs`, `Sim::step`, directly after the `self.vessel.step(dt, &inputs)` call (keep Package 2's binding of its return value for the ledger):

```rust
        if let Some(deficit) = self.vessel.exhausted {
            self.domain_error(DomainError::InventoryExhausted { deficit_kg: deficit });
        }
        if let Some(e) = self.vessel.last_error.take() {
            self.domain_error(e);
        }
```

Note for the executor: when the vessel scales steam outflow down, `self.steam.total_flow()` (used by the condenser) still carries the unscaled demand. That mismatch exists only in an already-flagged exhausted state and is accepted here; Package 6 reconciles the steam path.

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
Expected: three tests pass; `drained_vessel_is_flagged_not_refilled` prints `inventory exhausted at t = <about 9000-12000> s` and runs in well under a minute in release.

Run: `cargo test --release -p bwr-core`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add crates/bwr-core/src/vessel.rs crates/bwr-core/src/sim.rs crates/bwr-core/tests/conservation.rs
git commit -m "fix(vessel): bounded outflows and EOS range rejection replace silent clamps

Baseline: <paste printed line from Step 1>. Outflows are now scaled to the
available inventory and the deficit raised as DomainError::InventoryExhausted;
solve_eos returns EosOutOfRange instead of the nearest table bound.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 7: Edge states and non-finite command inputs

**Files:**
- Modify: `crates/bwr-core/src/sim.rs` (`cmd_is_finite`, `apply`)
- Modify: `crates/bwr-core/tests/numerics.rs`

**Interfaces:**
- Consumes: `assert_finite` (Task 1), `IcTrain.pool_mass` (Task 5 makes an empty pool a true no-sink).
- Produces: `fn cmd_is_finite(cmd: &Command) -> bool` (private to `sim.rs`); non-finite numeric commands are ignored with event id `cmd`.

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

At the top of `pub fn apply(&mut self, cmd: Command)`, before `match cmd {`:

```rust
        if !cmd_is_finite(&cmd) {
            self.event("cmd", format!("Command ignored: non-finite value in {cmd:?}"));
            return;
        }
```

- [ ] **Step 4: Run the tests**

Run: `cargo test --release -p bwr-core --test numerics`
Expected: all pass. Note the runtime of the two 1 h edge cases in the commit body.

- [ ] **Step 5: Commit**

```bash
git add crates/bwr-core/src/sim.rs crates/bwr-core/tests/numerics.rs
git commit -m "test(core): edge-state finiteness and non-finite command rejection

Covers zero power/no steam/closed valves, isolated vessel with no heat
sink, all steam paths closed at power, and NaN/inf command payloads
(previously NaN speed froze advance()).

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 8: Time-step convergence

**Files:**
- Modify: `crates/bwr-core/tests/numerics.rs`
- Modify: `docs/validation/method.md` (add the convergence subsection)

**Interfaces:**
- Consumes: `validation::{run_case, standard_cases}`, `Trace::metrics()`, `Metrics.first_event_time`, `Metrics.peak` (Package 2), case ids `fp_turbine_trip` and `fp_manual_scram` from `standard_cases()`.

**dt audit (done while planning; no hidden `DT` assumptions found):** every time-dependent update in `sim.rs`, `bop.rs`, `vessel.rs`, `ic.rs`, `reactor.rs` is either exact (`DecayHeat::step`, implicit fuel node), rate-limited by `dt` (valve strokes, rod speed, `SPEED_RAMP`), a first-order explicit lag of the form `x += (target - x) * (dt / tau).min(1.0)` (`h_in`, `alpha_core`, `alpha_ch`, `heatup_rate`, IC `heat`, condenser `p_steam`, `t_feed`, `inv_period`), an explicit Euler integral (`Xenon::step`, governor and level-controller integrals, air mass), or an absolute-time comparison (`scram_time`, event expiry, `reverse_power_timer`, `low_suction_timer`). All converge at first order in `dt`; the semi-implicit momentum step and `solve_eos` are stable for any `dt`. The only `DT`-specific code is `Sim::advance` and `Sim::settle`, which the runner does not use. Protection is evaluated once per step, so trip times are quantised to `dt`; the 0.1 s event tolerance below covers that.

- [ ] **Step 1: Write the failing test**

Append to `tests/numerics.rs`:

```rust
use bwr_core::validation::{run_case, standard_cases};

#[test]
fn event_timing_and_peaks_converge_with_time_step() {
    for id in ["fp_turbine_trip", "fp_manual_scram"] {
        let case = standard_cases().into_iter().find(|c| c.id == id).unwrap_or_else(|| panic!("case {id} missing"));
        let m50 = run_case(&case, 0.05).metrics();
        let m25 = run_case(&case, 0.025).metrics();
        let m125 = run_case(&case, 0.0125).metrics();

        assert!(!m125.first_event_time.is_empty(), "{id}: no events recorded");
        for (ev, t_fine) in &m125.first_event_time {
            let t_mid = m25.first_event_time.get(ev).unwrap_or_else(|| panic!("{id}: event {ev:?} missing at dt=0.025"));
            assert!((t_mid - t_fine).abs() <= 0.1, "{id}: {ev:?} at {t_mid} s (dt 0.025) vs {t_fine} s (dt 0.0125)");
            if let Some(t_c) = m50.first_event_time.get(ev) {
                println!("{id} {ev:?}: dt0.05 {t_c:.3} s, dt0.0125 {t_fine:.3} s, diff {:+.3} s", t_c - t_fine);
            }
        }
        let p_mid = m25.peak["pressure_pa"].0;
        let p_fine = m125.peak["pressure_pa"].0;
        assert!((p_mid - p_fine).abs() / p_fine < 0.005, "{id}: peak pressure {p_mid} (0.025) vs {p_fine} (0.0125)");
        println!("{id} peak pressure: dt0.05 {:.0} Pa, dt0.0125 {:.0} Pa, diff {:+.2} %", m50.peak["pressure_pa"].0, p_fine, (m50.peak["pressure_pa"].0 / p_fine - 1.0) * 100.0);
        println!("{id} min level: dt0.05 {:.2} cm, dt0.0125 {:.2} cm", m50.min["level_nr_cm"].0, m125.min["level_nr_cm"].0);
    }
}
```

- [ ] **Step 2: Run the test**

Run: `cargo test --release -p bwr-core --test numerics event_timing_and_peaks_converge -- --nocapture`
Expected: PASS and printed differences. If an event time differs by more than 0.1 s between 0.025 and 0.0125, or peak pressure by more than 0.5 %, that is a non-converged model element: find the explicit update responsible (use the audit list above), make it semi-implicit or shorten its internal substep, and rerun. Do not widen the tolerance. Record the printed table in the commit body.

- [ ] **Step 3: Document**

Append to `docs/validation/method.md`:

```markdown
## Time-step convergence

Supported cases are run at plant steps of 0.05, 0.025 and 0.0125 s with the
kinetics substep chosen adaptively (`kinetics.rs`). The numerical tolerance
class requires first-event times to agree within 0.1 s and pressure peaks
within 0.5 % between the two finer steps. The 0.05 s production step is
accepted when its difference from 0.0125 s is reported and inside the
physical tolerance class. Enforced by `tests/numerics.rs::event_timing_and_peaks_converge_with_time_step`.
```

- [ ] **Step 4: Commit**

```bash
git add crates/bwr-core/tests/numerics.rs docs/validation/method.md
git commit -m "test(core): time-step convergence of event timing and extrema

<paste printed table from Step 2>

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 9: Reviewed baseline regeneration

**Files:**
- Modify: `docs/validation/baseline/*.csv` (only the cases that changed)
- Modify: `docs/validation/results.md` is not created here (Package 10); record findings in the commit body and in `docs/validation/cases.csv` notes column.

**Interfaces:**
- Consumes: `examples/validation.rs` compare mode (default) and write mode (`--write`, as defined by Package 2).

- [ ] **Step 1: Compare against the Package 2 baselines**

Run: `cargo run --release -p bwr-core --example validation`
Expected: a per-case list of metric violations against the numerical tolerance class. Expected to change, with the physical reason:
- cases that scram (`fp_manual_scram`, `fp_msiv_closure`, any scram case): power falls one step (0.05 s) earlier (Task 4).
- any case in which the level approaches the top of active fuel: fuel temperature higher and `CORE UNCOVERED` alarm earlier (Task 2).
- long isolated cases with IC in service for hours: pool temperature capped at 100 degC (Task 5).
- cases that previously touched the 1 000 kg floor: now end in an `unsupported` flag (Task 6).
Cases that only exercise steady operation (`fp_steady`, startup) must show no violation; if they do, investigate before writing.

- [ ] **Step 2: Build the before/after table**

For every violated metric, record `case, metric, before, after, reason (task number)` in a table. Save it as the commit body. A violation with no reason in the list above is a new finding: stop, add a test in `tests/numerics.rs` that captures it, and fix or document it before regenerating.

- [ ] **Step 3: Regenerate**

Run: `cargo run --release -p bwr-core --example validation -- --write`
Expected: baseline CSVs rewritten for all cases; `git status` shows only the cases listed in the table as modified (byte-identical traces for unchanged cases confirm determinism).

Run: `cargo run --release -p bwr-core --example validation`
Expected: no violations.

Run: `cargo test --release -p bwr-core && npm --prefix ui run typecheck && npm --prefix ui run build`
Expected: all green.

- [ ] **Step 4: Commit**

```bash
git add docs/validation/baseline docs/validation/cases.csv
git commit -m "chore(validation): regenerate baselines after Package 3 corrections

Regression evidence only, not physical truth. Changed cases and reasons:

| case | metric | before | after | reason |
|---|---|---|---|---|
<paste table from Step 2>

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

## Acceptance gate (verbatim from the spec)

"Tests demonstrate the original defect before correction. Mass and energy residuals close to the stated numerical tolerance; domain errors are visible."

Checklist against the gate:

- [ ] Each of Tasks 2 to 6 recorded a baseline demonstration (printed line or failure message) in its commit body before the fix.
- [ ] `tests/conservation.rs` shows `mass_clipped_kg == 0` and `|mass_residual| < 1e-6 * mass0` through a drained-vessel transient.
- [ ] `Snapshot.unsupported`, the `unsupported` alarm, and `Sim.log` `DOMAIN:` entries make every domain error visible; `LoadScenario` is the only reset.
- [ ] Convergence at 0.05 / 0.025 / 0.0125 s is enforced for `fp_turbine_trip` and `fp_manual_scram`.
- [ ] Baselines were regenerated once with a reasoned before/after table; no tolerance was widened.
