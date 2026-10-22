# Package 2: Reproducible and Measurable Baseline — Implementation Plan (r2)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Names and signatures come from `docs/superpowers/plans/2026-10-20-refinement-master.md` r2 ("Shared interface contract"); if anything here disagrees with the master, the master wins.

**Goal:** Make the baseline engine behaviour reproducible and measurable: deterministic scripted cases, machine-readable traces (schema v2) with per-step interval extrema and mass/energy residuals, an ordered fault-override hook, an append-only event log, a common end-of-construction bookkeeping step, saved regression baselines, and CI that runs all of it.

**Architecture:** Three new engine modules. `ledger.rs` accounts every mass and energy flow the vessel applies and reports residuals and integrated transfers. `faults.rs` is an ordered list of engine-level overrides that automatic logic may never clear (honoured by later packages). `validation.rs` runs a `Case` (scenario + scripted commands + scripted faults + optional closed-loop operator) to a `Trace`, reduces it to `Metrics`, and compares metrics under a `Tolerance`. `Sim::finalize_init` gives every scenario constructor one bookkeeping epilogue. An example binary writes or checks baseline CSVs under `docs/validation/baseline/`. Nothing in this package changes physics; it only measures it.

**Tech Stack:** Rust 2021 (cargo 1.99.1), `serde_json` (existing dev-dependency), hand-rolled CSV, GitHub Actions. No new dependencies.

**Spec:** Package 2 of `docs/superpowers/plans/2026-10-20-existing-systems-refinement.md`.

## Global Constraints

- "No previous-test failure is concealed by updating expected output."
- "Save baseline results as regression evidence, explicitly not as physical truth."
- "Separate numerical, physical, and source-comparison tolerances. Set tolerances from uncertainty or convergence studies before tuning."
- Commands: `cargo test --release -p bwr-core`, `npm --prefix ui ci`, `npm --prefix ui run typecheck`, `npm --prefix ui run build`, and after Task 10 `cargo run --release -p bwr-core --example validation`.
- No new Rust or npm dependencies.
- No physics changes in this package. The vessel's `.max(1000.0)` clamp stays; it is measured here and removed in Package 3.
- Derives rule (master): every new struct or enum stored inside `Sim` derives at least `Clone, Debug`.
- Commit after every task; commit messages end with `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.
- Match the surrounding Rust style by hand (4-space indent, `rustfmt` defaults); 2-space indent in YAML/JSON/Markdown.
- `N_TRACE_COLS = 58` with the master's exact 58-name `TRACE_COLUMNS` list; the Rust compiler rejects any other pairing, so a new column means appending the name and bumping the constant in the master first.
- Every numerical constant this package introduces (`N_IC_RETURN_VALVES`, `OPERATOR_DT_S`, every `Tolerance::numerical()`/`physical()` number) gets a `docs/model/parameters.csv` row (locked header `id,value,unit,definition,reference_state,source_design,status,source_ids,interval_lo,interval_hi,assumption_id,location,notes`; fields never contain commas) in the task that introduces it; Task 11 Step 5a runs Package 1's `tests/register.rs` as the P1/P2 reconciliation gate.

---

## File structure

| Path | Action | Responsibility |
|---|---|---|
| `crates/bwr-core/tests/common/mod.rs` | create | shared integration-test helpers `run`, `assert_finite` |
| `crates/bwr-core/src/ledger.rs` | create | `BoundaryFlows`, `Ledger`: per-step boundary accounting, residuals, integrated transfers |
| `crates/bwr-core/src/faults.rs` | create | `Fault` enum with `kind`/`equipment`/`is_valid`, unit tests |
| `crates/bwr-core/src/validation.rs` | create | `Case`, `Operator`, `Trace`, `Metrics`, `Tolerance`, `compare`, `run_case`, `run_case_with`, `run_sim_with`, `standard_cases` |
| `crates/bwr-core/src/lib.rs` | modify | register the three modules |
| `crates/bwr-core/src/ic.rs` | modify | add `N_IC_RETURN_VALVES` (needed by `Fault::equipment`; P7 keeps it) |
| `crates/bwr-core/src/vessel.rs` | modify | `Vessel::step` returns `BoundaryFlows`, measures the mass clamp |
| `crates/bwr-core/src/sim.rs` | modify | `ledger`, `faults`, `log`, `prev_turbine_tripped`; `finalize_init`, `set_fault`, `has_fault`, `find_fault`; log pushes |
| `crates/bwr-core/tests/plant.rs` | modify | use `common`, add ledger/finalize/log/determinism/convergence tests |
| `crates/bwr-core/examples/validation.rs` | create | baseline writer/checker with convergence report |
| `docs/validation/method.md` | create | schema, metric and tolerance definitions |
| `docs/validation/cases.csv` | create | case register (locked header with `role`) |
| `docs/model/parameters.csv` | modify (append rows; create with the locked header if Package 1 has not landed yet) | register rows for `N_IC_RETURN_VALVES`, `OPERATOR_DT_S` and every tolerance number |
| `docs/validation/baseline/*.csv` | create (generated) | regression traces |
| `.github/workflows/validate.yml` | create | validation CI job |
| `package.json` (root) | modify | `validate` script |
| `README.md` | modify | "Validation" section |

---

### Task 1: Toolchain check and baseline run

**Files:** none modified; this task records the measured starting point.

**Interfaces:** produces confirmation that the 19 baseline tests and UI checks pass before any change.

- [ ] **Step 1: Confirm the Rust toolchain**

Run: `cargo --version && rustc --version`
Expected: `cargo 1.99.1 (...)` and a matching `rustc 1.99.x`. If `cargo` is missing, install via `https://rustup.rs` before continuing (the previous review could not run the tests for this reason).

- [ ] **Step 2: Run the engine tests at the baseline**

Run: `cargo test --release -p bwr-core`
Expected (counts must match exactly):

```
running 11 tests
...
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
...
running 8 tests
...
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

If any test fails here, do not modify expected values; record the failure text in the Task 11 commit message and continue. Package 3 owns physics fixes.

- [ ] **Step 3: Run the UI checks**

Run: `npm --prefix ui ci && npm --prefix ui run typecheck && npm --prefix ui run build`
Expected: `tsc --noEmit` prints nothing; `vite build` ends with `✓ built in ...`.

- [ ] **Step 4: Confirm the working tree is clean apart from plans**

Run: `git status --short`
Expected: only `?? docs/superpowers/` (or nothing). No commit for this task.

---

### Task 2: Shared integration-test helpers

**Files:**
- Create: `crates/bwr-core/tests/common/mod.rs`
- Modify: `crates/bwr-core/tests/plant.rs` (replace its private `run`)

**Interfaces:**
- Produces: `common::run(sim: &mut Sim, seconds: f64)` and `common::assert_finite(what: &str, values: impl IntoIterator<Item = f64>)`. Every later integration test file (P3 `numerics.rs`, `conservation.rs`, P4–P9) starts with `mod common;`.

- [ ] **Step 1: Create the helper module**

Create `crates/bwr-core/tests/common/mod.rs`:

```rust
//! Shared helpers for integration tests. Each test file includes it with `mod common;`.
//! Cargo treats `tests/common/mod.rs` as a module, not as a test crate.
#![allow(dead_code)]

use bwr_core::sim::DT;
use bwr_core::Sim;

/// Advance the plant by `seconds` of simulation time at the fixed physics step.
pub fn run(sim: &mut Sim, seconds: f64) {
    let steps = (seconds / DT).round() as usize;
    for _ in 0..steps {
        sim.step(DT);
    }
}

/// Panic naming `what` if any value is NaN or infinite.
pub fn assert_finite(what: &str, values: impl IntoIterator<Item = f64>) {
    for (i, v) in values.into_iter().enumerate() {
        assert!(v.is_finite(), "{what}: value {i} is {v}");
    }
}
```

- [ ] **Step 2: Use it from `plant.rs`**

In `crates/bwr-core/tests/plant.rs`, delete the local helper

```rust
fn run(sim: &mut Sim, seconds: f64) {
    let steps = (seconds / DT).round() as usize;
    for _ in 0..steps {
        sim.step(DT);
    }
}
```

and add directly under the existing `use` lines:

```rust
mod common;
use common::run;
```

- [ ] **Step 3: Run the tests**

Run: `cargo test --release -p bwr-core`
Expected: unit `11 passed`, integration `8 passed` (unchanged behaviour).

- [ ] **Step 4: Commit**

```bash
git add crates/bwr-core/tests/common/mod.rs crates/bwr-core/tests/plant.rs
git commit -m "test(core): shared integration-test helpers

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 3: Ledger module

**Files:**
- Create: `crates/bwr-core/src/ledger.rs`
- Modify: `crates/bwr-core/src/lib.rs`

**Interfaces:**
- Produces: `BoundaryFlows` (15 fields, `Clone, Copy, Debug, Default, PartialEq`), `INTEGRATED_KEYS`, `Ledger` with `new`, `record`, `mass_residual`, `energy_residual`, `integrated: BTreeMap<&'static str, f64>`. Consumed by Task 4 (`Vessel::step`, `Sim`), Task 7/8 (`run_sim_with`, `Metrics.integrated`), Package 3 (residual tests), Package 7 (IC mass ports).

- [ ] **Step 1: Write the failing unit tests in the new module file**

Create `crates/bwr-core/src/ledger.rs` containing only:

```rust
//! Boundary-flow accounting for the vessel inventory.
//!
//! Every step the vessel reports what it actually applied as a [`BoundaryFlows`].
//! The [`Ledger`] integrates those flows so that the difference between the
//! stored inventory and (initial + in - out) is a measurable residual. A closed
//! model has a residual of zero up to floating-point error; a clamp shows up as
//! `mass_clipped`/`energy_clipped`.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_flow_has_zero_residual() {
        let mut l = Ledger::new(1000.0, 5.0e6);
        let f = BoundaryFlows {
            w_feed: 10.0,
            h_feed: 1.0e5,
            w_steam: 10.0,
            h_steam: 2.0e6,
            q_core: 10.0 * (2.0e6 - 1.0e5),
            ..Default::default()
        };
        for _ in 0..100 {
            l.record(0.05, &f);
        }
        assert!(l.mass_residual(1000.0).abs() < 1e-9, "mass residual {}", l.mass_residual(1000.0));
        assert!(l.energy_residual(5.0e6).abs() < 1e-3, "energy residual {}", l.energy_residual(5.0e6));
        assert!((l.mass_in_kg - 50.0).abs() < 1e-9);
        assert!((l.mass_out_kg - 50.0).abs() < 1e-9);
    }

    #[test]
    fn residual_tracks_inventory_change() {
        let mut l = Ledger::new(1000.0, 0.0);
        let f = BoundaryFlows { w_feed: 2.0, ..Default::default() };
        l.record(1.0, &f);
        assert!(l.mass_residual(1002.0).abs() < 1e-12);
        assert!((l.mass_residual(1000.0) + 2.0).abs() < 1e-12);
    }

    #[test]
    fn clipped_amounts_accumulate_per_step_not_per_second() {
        let mut l = Ledger::new(0.0, 0.0);
        let f = BoundaryFlows { mass_clipped: 3.0, energy_clipped: 7.0, ..Default::default() };
        l.record(0.05, &f);
        l.record(0.05, &f);
        assert!((l.mass_clipped_kg - 6.0).abs() < 1e-12);
        assert!((l.energy_clipped_j - 14.0).abs() < 1e-12);
    }

    #[test]
    fn all_ports_and_sinks_are_counted() {
        let mut l = Ledger::new(0.0, 0.0);
        let f = BoundaryFlows {
            w_steam: 1.0,
            h_steam: 10.0,
            w_vent: 2.0,
            w_liquid_out: 3.0,
            h_liquid_out: 5.0,
            w_ic_steam: 4.0,
            w_ic_return: 1.5,
            h_ic_return: 8.0,
            q_ic: 100.0,
            q_loss: 50.0,
            ..Default::default()
        };
        l.record(1.0, &f);
        assert!((l.mass_out_kg - (1.0 + 2.0 + 3.0 + 4.0)).abs() < 1e-12);
        assert!((l.mass_in_kg - 1.5).abs() < 1e-12);
        // Energy: steam+vent at h_steam, liquid at h_liquid_out, q_ic and q_loss. The IC mass
        // ports carry NO separate enthalpy terms because q_ic is defined as their net (master P2).
        assert!((l.energy_out_j - (3.0 * 10.0 + 3.0 * 5.0 + 150.0)).abs() < 1e-9);
        assert!(l.energy_in_j.abs() < 1e-12);
    }

    #[test]
    fn integrated_keys_are_present_and_filled() {
        let mut l = Ledger::new(0.0, 0.0);
        for k in INTEGRATED_KEYS {
            assert_eq!(l.integrated[k], 0.0, "{k} must exist at construction");
        }
        let f = BoundaryFlows { w_feed: 2.0, q_core: 5.0, w_vent: 1.0, ..Default::default() };
        l.record(2.0, &f);
        assert!((l.integrated["feed_kg"] - 4.0).abs() < 1e-12);
        assert!((l.integrated["core_j"] - 10.0).abs() < 1e-12);
        assert!((l.integrated["vent_kg"] - 2.0).abs() < 1e-12);
        assert_eq!(l.integrated["steam_kg"], 0.0);
    }
}
```

- [ ] **Step 2: Register the module and verify failure**

Edit `crates/bwr-core/src/lib.rs`: insert `pub mod ledger;` between `pub mod kinetics;` and `pub mod params;`.

Run: `cargo test --release -p bwr-core ledger`
Expected: compile error `cannot find type `Ledger` in this scope`.

- [ ] **Step 3: Implement the module**

Insert above `#[cfg(test)]` in `ledger.rs`:

```rust
use std::collections::BTreeMap;

/// What the vessel actually applied in one step, after bounding.
/// Rates are per second; `mass_clipped`/`energy_clipped` are amounts for the step.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BoundaryFlows {
    /// kg/s into the vessel.
    pub w_feed: f64,
    /// J/kg.
    pub h_feed: f64,
    /// kg/s out through the main steam lines (applied).
    pub w_steam: f64,
    /// J/kg.
    pub h_steam: f64,
    /// kg/s out through the head vent.
    pub w_vent: f64,
    /// kg/s RWCU reject (applied).
    pub w_liquid_out: f64,
    /// J/kg.
    pub h_liquid_out: f64,
    /// kg/s out to the isolation condensers (applied; 0 before Package 7).
    pub w_ic_steam: f64,
    /// kg/s condensate returned from the isolation condensers (0 before Package 7).
    pub w_ic_return: f64,
    /// J/kg.
    pub h_ic_return: f64,
    /// W into the coolant from the core.
    pub q_core: f64,
    /// W removed by the isolation condensers. Before Package 7 this is the IC model's
    /// heat removal; from Package 7 it is DERIVED as
    /// `w_ic_steam * h_steam - w_ic_return * h_ic_return` and never added separately.
    /// The ledger therefore counts `q_ic` and does not count IC port enthalpies.
    pub q_ic: f64,
    /// W insulation loss.
    pub q_loss: f64,
    /// kg created (positive) or destroyed (negative) by a clamp this step.
    pub mass_clipped: f64,
    /// J created (positive) or destroyed (negative) by a clamp this step.
    pub energy_clipped: f64,
}

/// Keys of `Ledger::integrated`, in report order.
pub const INTEGRATED_KEYS: [&str; 9] =
    ["feed_kg", "steam_kg", "ic_steam_kg", "ic_return_kg", "rwcu_kg", "vent_kg", "core_j", "ic_j", "loss_j"];

/// Running integrals of boundary flows since a reference state.
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
    /// Integrated transfers keyed by `INTEGRATED_KEYS`, for `Metrics.integrated`.
    pub integrated: BTreeMap<&'static str, f64>,
}

impl Ledger {
    pub fn new(mass0_kg: f64, energy0_j: f64) -> Self {
        let integrated = INTEGRATED_KEYS.iter().map(|k| (*k, 0.0)).collect();
        Self { mass0_kg, energy0_j, integrated, ..Default::default() }
    }

    pub fn record(&mut self, dt: f64, f: &BoundaryFlows) {
        self.mass_in_kg += (f.w_feed + f.w_ic_return) * dt;
        self.mass_out_kg += (f.w_steam + f.w_vent + f.w_liquid_out + f.w_ic_steam) * dt;
        self.energy_in_j += (f.q_core + f.w_feed * f.h_feed) * dt;
        self.energy_out_j +=
            (f.q_ic + f.q_loss + (f.w_steam + f.w_vent) * f.h_steam + f.w_liquid_out * f.h_liquid_out) * dt;
        self.mass_clipped_kg += f.mass_clipped;
        self.energy_clipped_j += f.energy_clipped;
        let add = |m: &mut BTreeMap<&'static str, f64>, k: &'static str, v: f64| *m.entry(k).or_insert(0.0) += v;
        add(&mut self.integrated, "feed_kg", f.w_feed * dt);
        add(&mut self.integrated, "steam_kg", f.w_steam * dt);
        add(&mut self.integrated, "ic_steam_kg", f.w_ic_steam * dt);
        add(&mut self.integrated, "ic_return_kg", f.w_ic_return * dt);
        add(&mut self.integrated, "rwcu_kg", f.w_liquid_out * dt);
        add(&mut self.integrated, "vent_kg", f.w_vent * dt);
        add(&mut self.integrated, "core_j", f.q_core * dt);
        add(&mut self.integrated, "ic_j", f.q_ic * dt);
        add(&mut self.integrated, "loss_j", f.q_loss * dt);
    }

    /// `mass_now - (mass0 + in - out)`. Zero means the model is closed.
    pub fn mass_residual(&self, mass_now: f64) -> f64 {
        mass_now - (self.mass0_kg + self.mass_in_kg - self.mass_out_kg)
    }

    /// `energy_now - (energy0 + in - out)`. Zero means the model is closed.
    pub fn energy_residual(&self, energy_now: f64) -> f64 {
        energy_now - (self.energy0_j + self.energy_in_j - self.energy_out_j)
    }
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test --release -p bwr-core ledger`
Expected: `test result: ok. 5 passed`.

- [ ] **Step 5: Commit**

```bash
git add crates/bwr-core/src/ledger.rs crates/bwr-core/src/lib.rs
git commit -m "feat(core): boundary-flow ledger with residuals and integrated transfers

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 4: Vessel reports boundary flows; `Sim::finalize_init` and the ledger

**Files:**
- Modify: `crates/bwr-core/src/vessel.rs` (`Vessel::step`)
- Modify: `crates/bwr-core/src/sim.rs` (struct, `base`, three constructors, `step`, new `finalize_init`)
- Modify: `crates/bwr-core/tests/plant.rs` (append two tests)

**Interfaces:**
- Consumes: `Ledger`, `BoundaryFlows`.
- Produces: `Vessel::step(&mut self, dt: f64, inp: &VesselInputs) -> BoundaryFlows`; `Sim::ledger: Ledger`; `Sim::last_flows(&self) -> &BoundaryFlows` (the flows applied in the most recent step; Package 7 and tests read them); `Sim::finalize_init(&mut self)` called at the end of `cold_shutdown`, `hot_standby`, `full_power`. At this package it sets `time = 0`, re-bases the ledger, clears `events`, `acked`, `accumulator` (Task 5 adds `log.clear()`; P3 adds `domain_errors` re-stamping; P8 sensors; P10 extrema). Package 3 relies on `mass_clipped` being exactly what the clamp added. `ledger.record` runs at position 16 of the master's `Sim::step` order, after `protection()`.

- [ ] **Step 1: Write the failing tests**

Append to `crates/bwr-core/tests/plant.rs`:

```rust
#[test]
fn ledger_closes_mass_and_energy_at_full_power() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 120.0);
    let rm = sim.ledger.mass_residual(sim.vessel.mass);
    let re = sim.ledger.energy_residual(sim.vessel.energy);
    // At the baseline the only non-physical term is the inventory clamp, which the
    // ledger reports separately; residual and clipped mass must agree exactly.
    assert!((rm - sim.ledger.mass_clipped_kg).abs() < 1e-3, "mass residual {rm} vs clipped {}", sim.ledger.mass_clipped_kg);
    assert!(re.abs() < 1e3, "energy residual {re} J");
    // Feed flow at rated power is ~470 kg/s, so 120 s must record well over 40 t.
    assert!(sim.ledger.mass_in_kg > 40_000.0, "feed mass recorded {}", sim.ledger.mass_in_kg);
    assert!((sim.ledger.integrated["feed_kg"] - sim.ledger.mass_in_kg).abs() < 1e-6, "no IC return yet");
    assert!(sim.last_flows().w_feed > 100.0, "last_flows holds the applied step: {:?}", sim.last_flows());
}

#[test]
fn finalize_init_zeroes_bookkeeping_for_every_constructor_and_reload() {
    for name in [ScenarioName::ColdShutdown, ScenarioName::HotStandby, ScenarioName::FullPower] {
        let sim = Sim::new(name);
        assert_eq!(sim.time, 0.0, "{name:?}");
        assert_eq!(sim.ledger.mass_in_kg, 0.0, "{name:?}: ledger not re-based after settle");
        assert_eq!(sim.ledger.mass_out_kg, 0.0, "{name:?}");
        assert_eq!(sim.ledger.mass0_kg, sim.vessel.mass, "{name:?}");
        assert_eq!(sim.ledger.energy0_j, sim.vessel.energy, "{name:?}");
        assert!(sim.snapshot().alarms.iter().all(|a| !a.acked), "{name:?}: acked set not cleared");
    }
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 60.0);
    assert!(sim.ledger.mass_in_kg > 0.0);
    sim.apply(Command::LoadScenario { name: ScenarioName::HotStandby });
    assert_eq!(sim.time, 0.0);
    assert_eq!(sim.ledger.mass_in_kg, 0.0, "LoadScenario must go through finalize_init");
    assert_eq!(sim.ledger.mass0_kg, sim.vessel.mass);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --release -p bwr-core --test plant ledger_closes`
Expected: compile error `no field `ledger` on type `Sim``.

- [ ] **Step 3: Make `Vessel::step` return `BoundaryFlows`**

In `crates/bwr-core/src/vessel.rs`, change

```rust
use crate::params::*;
use crate::steam::{self, sat_p, sat_t, Sat, P_ATM};
```

to

```rust
use crate::ledger::BoundaryFlows;
use crate::params::*;
use crate::steam::{self, sat_p, sat_t, Sat, P_ATM};
```

Change `pub fn step(&mut self, dt: f64, inp: &VesselInputs) {` to `pub fn step(&mut self, dt: f64, inp: &VesselInputs) -> BoundaryFlows {`.

Replace, inside `step`,

```rust
        let w_steam = if self.eos.subcooled { 0.0 } else { inp.w_steam };
        let dm = inp.w_feed - w_steam - inp.w_liquid_out - self.vent_flow;
        let de = inp.q_core - inp.q_ic - heat_loss + inp.w_feed * inp.h_feed
            - (w_steam + self.vent_flow) * h_steam
            - inp.w_liquid_out * h_liq;
        self.mass = (self.mass + dm * dt).max(1000.0);
        self.energy += de * dt;
```

with

```rust
        let w_steam = if self.eos.subcooled { 0.0 } else { inp.w_steam };
        let dm = inp.w_feed - w_steam - inp.w_liquid_out - self.vent_flow;
        let de = inp.q_core - inp.q_ic - heat_loss + inp.w_feed * inp.h_feed
            - (w_steam + self.vent_flow) * h_steam
            - inp.w_liquid_out * h_liq;
        // Baseline inventory floor. Package 3 removes it; here we only measure it.
        let mass_unclamped = self.mass + dm * dt;
        self.mass = mass_unclamped.max(1000.0);
        let mass_clipped = self.mass - mass_unclamped;
        self.energy += de * dt;
        let flows = BoundaryFlows {
            w_feed: inp.w_feed,
            h_feed: inp.h_feed,
            w_steam,
            h_steam,
            w_vent: self.vent_flow,
            w_liquid_out: inp.w_liquid_out,
            h_liquid_out: h_liq,
            w_ic_steam: 0.0,
            w_ic_return: 0.0,
            h_ic_return: 0.0,
            q_core: inp.q_core,
            q_ic: inp.q_ic,
            q_loss: heat_loss,
            mass_clipped,
            energy_clipped: 0.0,
        };
```

and change the end of `step`

```rust
        let rate = (self.eos.t - t_before) / dt * 3600.0;
        self.heatup_rate += (rate - self.heatup_rate) * (dt / 30.0).min(1.0);
    }
```

to

```rust
        let rate = (self.eos.t - t_before) / dt * 3600.0;
        self.heatup_rate += (rate - self.heatup_rate) * (dt / 30.0).min(1.0);
        flows
    }
```

- [ ] **Step 4: Add the ledger and `finalize_init` to `Sim`**

In `crates/bwr-core/src/sim.rs`, add after `use crate::ic::IcTrain;`:

```rust
use crate::ledger::{BoundaryFlows, Ledger};
```

In the `Sim` struct, after `accumulator: f64,` add:

```rust
    /// Mass/energy boundary accounting for the vessel inventory (see ledger.rs).
    pub ledger: Ledger,
    /// Boundary flows applied in the most recent physics step.
    last_flows: BoundaryFlows,
```

In `fn base(...)`, change

```rust
    fn base(core: Core, vessel: Vessel, steam: SteamSystem, turbine: Turbine, cond: Condenser, fw: Feedwater, mode: ModeSwitch) -> Self {
        let mut s = Self {
```

to

```rust
    fn base(core: Core, vessel: Vessel, steam: SteamSystem, turbine: Turbine, cond: Condenser, fw: Feedwater, mode: ModeSwitch) -> Self {
        let ledger = Ledger::new(vessel.mass, vessel.energy);
        let mut s = Self {
```

and in the struct literal after `accumulator: 0.0,` add `ledger,` and `last_flows: BoundaryFlows::default(),`.

Add the epilogue method to `impl Sim`, directly after `fn base` ends:

```rust
    /// Common end of every scenario constructor: zero the clock and bookkeeping
    /// without touching the physical state. Later packages extend it (P3:
    /// re-stamp retained domain errors at t = 0; P8: seed sensors; P10: seed extrema).
    pub fn finalize_init(&mut self) {
        self.time = 0.0;
        self.ledger = Ledger::new(self.vessel.mass, self.vessel.energy);
        self.events.clear();
        self.acked.clear();
        self.accumulator = 0.0;
    }

    /// Boundary flows applied in the most recent physics step (master P2 contract).
    pub fn last_flows(&self) -> &BoundaryFlows {
        &self.last_flows
    }
```

In `pub fn cold_shutdown()`, change

```rust
        s.set_source_equilibrium();
        s.irm_range = 1;
        s
    }
```

to

```rust
        s.set_source_equilibrium();
        s.irm_range = 1;
        s.finalize_init();
        s
    }
```

In `pub fn hot_standby()`, change

```rust
        s.set_source_equilibrium();
        s
    }
```

to

```rust
        s.set_source_equilibrium();
        s.finalize_init();
        s
    }
```

In `pub fn full_power()`, change

```rust
        s.settle(120.0);
        s.time = 0.0;
        s.acked.clear();
        s.events.clear();
        s
    }
```

to

```rust
        s.settle(120.0);
        s.finalize_init();
        s
    }
```

In `fn step(&mut self, dt: f64)`, change

```rust
        self.vessel.step(dt, &inputs);
```

to

```rust
        let flows = self.vessel.step(dt, &inputs);
```

(`flows` stays a local; `BoundaryFlows` is `Copy`.) Then change the end of `step`

```rust
        self.time += dt;
        self.protection();
    }
```

to

```rust
        self.time += dt;
        self.protection();
        // Master step order: 14 time, 15 protection, 16 ledger record. The ledger is
        // written last so later packages' extrema/domain-error bookkeeping (16, 17)
        // and P8 protection see the same ordering the master locks.
        self.ledger.record(dt, &flows);
        self.last_flows = flows;
    }
```

(`LoadScenario` already does `*self = Sim::new(name)`, so it inherits `finalize_init`.)

- [ ] **Step 5: Run the full suite**

Run: `cargo test --release -p bwr-core`
Expected: unit `16 passed`, integration `10 passed`.

- [ ] **Step 6: Commit**

```bash
git add crates/bwr-core/src/vessel.rs crates/bwr-core/src/sim.rs crates/bwr-core/tests/plant.rs
git commit -m "feat(core): vessel boundary flows, Sim ledger and finalize_init epilogue

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 5: Append-only event log on `Sim`

**Files:**
- Modify: `crates/bwr-core/src/sim.rs`
- Modify: `crates/bwr-core/tests/plant.rs` (append one test)

**Interfaces:**
- Produces: `Sim::log: Vec<(f64, String)>` (public). Entry text format follows the master's locked log-line convention `"<key>: <detail>"` with `<key>` from `validation::LOG_KEYS` (Task 7); `Metrics::first_event_time` keys on the text before the first `:`. Keys written in this package: `scram`, `turbine trip`, `msiv isolation`, `ic auto-initiation`, `feed pump trip`, and `fault` (Task 6). Package 3 adds `domain error` and `command ignored` (the master's non-finite command rule). Task 9 adds the conformance test that every log line from the standard cases starts with a listed key. `run_sim_with` drains the log each step. `finalize_init` clears it.

- [ ] **Step 1: Write the failing test**

Append to `crates/bwr-core/tests/plant.rs`:

```rust
#[test]
fn log_records_scram_and_turbine_trip_with_times() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    assert!(sim.log.is_empty(), "finalize_init must clear the log");
    run(&mut sim, 5.0);
    sim.apply(Command::TripTurbine);
    run(&mut sim, 1.0);
    sim.apply(Command::ManualScram);
    run(&mut sim, 1.0);
    let trip = sim.log.iter().find(|(_, e)| e.starts_with("turbine trip")).expect("turbine trip logged");
    // Command trips are detected at the end of the next protection pass, so within one step.
    assert!((trip.0 - 5.0).abs() <= DT + 1e-9, "trip time {}", trip.0);
    assert!(trip.1.contains("Manual turbine trip"), "{}", trip.1);
    let scram = sim.log.iter().find(|(_, e)| e.starts_with("scram")).expect("scram logged");
    assert!((scram.0 - 6.0).abs() <= DT + 1e-9, "scram time {}", scram.0);
    assert!(scram.1.contains("Manual scram"), "{}", scram.1);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --release -p bwr-core --test plant log_records`
Expected: compile error `no field `log` on type `Sim``.

- [ ] **Step 3: Add the field and the pushes**

In the `Sim` struct after `pub ledger: Ledger,` add:

```rust
    /// Append-only event log: (sim time, "<key>: <detail>"). Drained by the validation runner.
    pub log: Vec<(f64, String)>,
    prev_turbine_tripped: bool,
```

In `base`, change `let ledger = Ledger::new(vessel.mass, vessel.energy);` to

```rust
        let ledger = Ledger::new(vessel.mass, vessel.energy);
        let prev_turbine_tripped = turbine.tripped;
```

and in the struct literal after `ledger,` add:

```rust
            log: Vec::new(),
            prev_turbine_tripped,
```

In `finalize_init`, after `self.ledger = Ledger::new(...);` add `self.log.clear();`.

In `pub fn scram(&mut self, reason: &str)`, change

```rust
        if !self.scrammed {
            self.scram_reason = Some(reason.to_string());
            self.scram_time = self.time;
        }
```

to

```rust
        if !self.scrammed {
            self.scram_reason = Some(reason.to_string());
            self.scram_time = self.time;
            self.log.push((self.time, format!("scram: {reason}")));
        }
```

In `step`, change

```rust
        if let Some(m) = msg {
            self.event("fw_trip", m);
        }
```

to

```rust
        if let Some(m) = msg {
            self.log.push((self.time, format!("feed pump trip: {m}")));
            self.event("fw_trip", m);
        }
```

In `protection`, change

```rust
            self.steam.msiv_cmd = [false; N_STEAM_LINES];
            self.event("isolation", format!("MSIV isolation: {}", iso[0]));
```

to

```rust
            self.steam.msiv_cmd = [false; N_STEAM_LINES];
            self.log.push((self.time, format!("msiv isolation: {}", iso[0])));
            self.event("isolation", format!("MSIV isolation: {}", iso[0]));
```

change

```rust
                self.event("ic_init", "Isolation condensers auto-initiated");
```

to

```rust
                self.log.push((self.time, "ic auto-initiation: all trains".to_string()));
                self.event("ic_init", "Isolation condensers auto-initiated");
```

change

```rust
                self.turbine.trip("Reactor high water level (L8)");
                self.fw.pumps = [false; N_FEED_PUMPS];
                self.event("l8", "L8 high level: turbine and feed pumps tripped");
```

to

```rust
                self.turbine.trip("Reactor high water level (L8)");
                self.fw.pumps = [false; N_FEED_PUMPS];
                self.log.push((self.time, "feed pump trip: L8 high level".to_string()));
                self.event("l8", "L8 high level: turbine and feed pumps tripped");
```

and at the end of `protection`, change

```rust
        let now = self.time;
        self.events.retain(|_, (_, until)| *until > now);
    }
```

to

```rust
        // Turbine trips originate in several places (overspeed in Turbine::step,
        // L8/vacuum above, operator commands); log the transition in one place.
        if self.turbine.tripped && !self.prev_turbine_tripped {
            let reason = self.turbine.trip_reason.clone().unwrap_or_default();
            self.log.push((self.time, format!("turbine trip: {reason}")));
        }
        self.prev_turbine_tripped = self.turbine.tripped;

        let now = self.time;
        self.events.retain(|_, (_, until)| *until > now);
    }
```

- [ ] **Step 4: Run tests**

Run: `cargo test --release -p bwr-core`
Expected: unit `16 passed`, integration `11 passed`.

- [ ] **Step 5: Commit**

```bash
git add crates/bwr-core/src/sim.rs crates/bwr-core/tests/plant.rs
git commit -m "feat(core): append-only event log with sim times for protective actions

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 6: Ordered fault overrides

**Files:**
- Create: `crates/bwr-core/src/faults.rs`
- Modify: `crates/bwr-core/src/lib.rs`, `crates/bwr-core/src/ic.rs`, `crates/bwr-core/src/sim.rs`
- Modify: `docs/model/parameters.csv` (append the `N_IC_RETURN_VALVES` row)

**Interfaces:**
- Consumes: `Sim::log`, `params::{N_STEAM_LINES, N_FEED_PUMPS, N_COND_PUMPS, N_CW_PUMPS, N_IC_TRAINS, N_RODS}`, `ic::N_IC_RETURN_VALVES` (added here; Package 7 keeps it).
- Produces: `Fault` (master's 16 variants) with `kind() -> &'static str`, `equipment() -> Option<usize>`, `is_valid() -> bool`; `Sim::faults: Vec<Fault>`; `Sim::set_fault(&mut self, f: Fault, active: bool)`; `Sim::has_fault(&self, f: &Fault) -> bool`; `Sim::find_fault(&self, kind: &str, equipment: Option<usize>) -> Option<&Fault>`. Honouring is done in Packages 5, 6, 7, 9. Faults are not `Command`s and never appear in `Snapshot`. `LoadScenario` builds a fresh `Sim`, so faults are cleared by scenario reload only (documented behaviour).

- [ ] **Step 1: Add the IC valve-count constant**

In `crates/bwr-core/src/ic.rs`, after `use crate::steam::sat_p;` add:

```rust
/// Parallel condensate return valves per train (modelled from Package 7; the
/// count is needed now so `Fault::equipment` can address a valve).
pub const N_IC_RETURN_VALVES: usize = 2;
```

- [ ] **Step 1a: Register `N_IC_RETURN_VALVES` in the parameter register**

Append to `docs/model/parameters.csv` (if Package 1 has not landed yet the file does not exist: create it with the locked header line `id,value,unit,definition,reference_state,source_design,status,source_ids,interval_lo,interval_hi,assumption_id,location,notes` first; Package 1's rows are concatenated on merge and Task 11 Step 5a catches any clash). The id is the constant name because Package 7 moves it to `params.rs` and **updates** this row in place (status `esbwr-estimate`, A-021, S-ESBWR-05) rather than appending a second one:

```csv
N_IC_RETURN_VALVES,2,-,Parallel condensate return valves per isolation condenser train (addressed by Fault::IcReturnValveStuck),-,ESBWR,assumed,,1,2,A-013,ic.rs,ESBWR-derived parallel return arrangement; modelled and verified in Package 7
```

Run (when `crates/bwr-core/tests/register.rs` exists): `cargo test --release -p bwr-core --test register`
Expected: `4 passed`.

- [ ] **Step 2: Write the failing unit tests in the new module**

Create `crates/bwr-core/src/faults.rs`:

```rust
//! Test-level fault overrides for existing equipment.
//!
//! A fault is an engine-level flag that forces a component into a failed
//! state so scenario tests can inject stuck valves or unavailable trains
//! deterministically. Automatic plant logic must never clear a fault; only
//! `Sim::set_fault(.., false)` or a scenario reload does. Faults are not
//! operator commands and are not part of the UI contract. The active set is an
//! ordered `Vec` so iteration order (and therefore behaviour) is deterministic.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::ScenarioName;
    use crate::sim::DT;
    use crate::Sim;

    #[test]
    fn faults_persist_across_steps_and_are_distinct() {
        let mut sim = Sim::new(ScenarioName::FullPower);
        sim.set_fault(Fault::MsivStuck { line: 0 }, true);
        sim.set_fault(Fault::IcTrainUnavailable { train: 2 }, true);
        for _ in 0..100 {
            sim.step(DT);
        }
        assert!(sim.has_fault(&Fault::MsivStuck { line: 0 }));
        assert!(!sim.has_fault(&Fault::MsivStuck { line: 1 }));
        assert!(sim.has_fault(&Fault::IcTrainUnavailable { train: 2 }));
        sim.set_fault(Fault::MsivStuck { line: 0 }, false);
        assert!(!sim.has_fault(&Fault::MsivStuck { line: 0 }));
        assert_eq!(sim.faults, vec![Fault::IcTrainUnavailable { train: 2 }]);
    }

    #[test]
    fn same_kind_and_equipment_replaces_instead_of_coexisting() {
        let mut sim = Sim::new(ScenarioName::HotStandby);
        sim.set_fault(Fault::PressureRegulatorFailed { demand_pct: 20 }, true);
        sim.set_fault(Fault::PressureRegulatorFailed { demand_pct: 80 }, true);
        assert_eq!(sim.faults.len(), 1, "two regulator payloads must not coexist");
        assert_eq!(sim.find_fault("PressureRegulatorFailed", None), Some(&Fault::PressureRegulatorFailed { demand_pct: 80 }));
        assert!(!sim.has_fault(&Fault::PressureRegulatorFailed { demand_pct: 20 }));
        // Deactivating by (kind, equipment) works whatever payload is passed.
        sim.set_fault(Fault::PressureRegulatorFailed { demand_pct: 0 }, false);
        assert!(sim.faults.is_empty());
        // Different equipment indices are distinct entries.
        sim.set_fault(Fault::FeedPumpTrip { index: 0 }, true);
        sim.set_fault(Fault::FeedPumpTrip { index: 2 }, true);
        assert_eq!(sim.faults.len(), 2);
        assert_eq!(sim.find_fault("FeedPumpTrip", Some(2)), Some(&Fault::FeedPumpTrip { index: 2 }));
        assert_eq!(sim.find_fault("FeedPumpTrip", Some(1)), None);
    }

    #[test]
    fn invalid_payloads_are_ignored_with_a_log_entry() {
        let mut sim = Sim::new(ScenarioName::ColdShutdown);
        sim.set_fault(Fault::MsivStuck { line: 7 }, true);
        sim.set_fault(Fault::PressureRegulatorFailed { demand_pct: 101 }, true);
        sim.set_fault(Fault::IcReturnValveStuck { train: 0, valve: 2 }, true);
        sim.set_fault(Fault::RodStuck { index: 57 }, true);
        assert!(sim.faults.is_empty());
        let ignored = sim.log.iter().filter(|(_, e)| e.starts_with("fault:")).count();
        assert_eq!(ignored, 4, "{:?}", sim.log);
        assert!(Fault::IcNoncondensableDegradation { train: 1, pct: 100 }.is_valid(), "100 % is the inclusive limit");
    }

    #[test]
    fn kind_and_equipment_mapping() {
        assert_eq!(Fault::TsvStuck.kind(), "TsvStuck");
        assert_eq!(Fault::TsvStuck.equipment(), None);
        assert_eq!(Fault::MsivStuck { line: 1 }.equipment(), Some(1));
        assert_eq!(Fault::IcReturnValveStuck { train: 2, valve: 1 }.equipment(), Some(2 * N_IC_RETURN_VALVES + 1));
        assert_eq!(Fault::IcNoncondensableDegradation { train: 1, pct: 50 }.equipment(), Some(1));
        assert!(Fault::CwPumpTrip { index: N_CW_PUMPS - 1 }.is_valid());
        assert!(!Fault::CwPumpTrip { index: N_CW_PUMPS }.is_valid());
    }
}
```

- [ ] **Step 3: Register and run to verify failure**

Edit `crates/bwr-core/src/lib.rs`: insert `pub mod faults;` between `pub mod bop;` and `pub mod ic;`.

Run: `cargo test --release -p bwr-core faults`
Expected: compile error `cannot find type `Fault``.

- [ ] **Step 4: Implement the enum and the `Sim` hooks**

Insert above `#[cfg(test)]` in `faults.rs`:

```rust
use crate::ic::N_IC_RETURN_VALVES;
use crate::params::{N_COND_PUMPS, N_CW_PUMPS, N_FEED_PUMPS, N_IC_TRAINS, N_RODS, N_STEAM_LINES};

/// Deterministic equipment faults. The package that honours each variant is noted.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Fault {
    /// Valve holds its current position. (P6)
    MsivStuck { line: usize },
    /// Turbine stop valve holds its current position. (P6)
    TsvStuck,
    /// Turbine control valve holds its current position. (P6)
    TcvStuck,
    /// Bypass valves cannot open. (P6)
    BypassUnavailable,
    /// Regulator output frozen at `demand_pct` % of rated steam flow. (P6)
    PressureRegulatorFailed { demand_pct: u8 },
    /// Pump cannot run. (P6)
    FeedPumpTrip { index: usize },
    /// Pump cannot run. (P6)
    CondensatePumpTrip { index: usize },
    /// Feedwater control valve holds its current position. (P6)
    FcvStuck,
    /// Feedwater heating lost regardless of heater selection. (P6)
    HeatersLost,
    /// Pump cannot run. (P6)
    CwPumpTrip { index: usize },
    /// Vacuum pump removes no air. (P6)
    VacuumPumpFailed,
    /// No heat removal; valves do not move. (P7)
    IcTrainUnavailable { train: usize },
    /// One of the parallel condensate return valves holds its position. (P7)
    IcReturnValveStuck { train: usize, valve: usize },
    /// Steam supply valve closed. (P7)
    IcSteamValveClosed { train: usize },
    /// Rod does not move under any mechanism. (P5)
    RodStuck { index: usize },
    /// Train UA reduced by `pct` %. (P7)
    IcNoncondensableDegradation { train: usize, pct: u8 },
}

impl Fault {
    /// Variant name without payload, e.g. "PressureRegulatorFailed".
    pub fn kind(&self) -> &'static str {
        match self {
            Fault::MsivStuck { .. } => "MsivStuck",
            Fault::TsvStuck => "TsvStuck",
            Fault::TcvStuck => "TcvStuck",
            Fault::BypassUnavailable => "BypassUnavailable",
            Fault::PressureRegulatorFailed { .. } => "PressureRegulatorFailed",
            Fault::FeedPumpTrip { .. } => "FeedPumpTrip",
            Fault::CondensatePumpTrip { .. } => "CondensatePumpTrip",
            Fault::FcvStuck => "FcvStuck",
            Fault::HeatersLost => "HeatersLost",
            Fault::CwPumpTrip { .. } => "CwPumpTrip",
            Fault::VacuumPumpFailed => "VacuumPumpFailed",
            Fault::IcTrainUnavailable { .. } => "IcTrainUnavailable",
            Fault::IcReturnValveStuck { .. } => "IcReturnValveStuck",
            Fault::IcSteamValveClosed { .. } => "IcSteamValveClosed",
            Fault::RodStuck { .. } => "RodStuck",
            Fault::IcNoncondensableDegradation { .. } => "IcNoncondensableDegradation",
        }
    }

    /// Equipment index the fault applies to (line, index, train; for
    /// `IcReturnValveStuck`: `train * N_IC_RETURN_VALVES + valve`). None for plant-wide faults.
    pub fn equipment(&self) -> Option<usize> {
        match *self {
            Fault::MsivStuck { line } => Some(line),
            Fault::FeedPumpTrip { index } | Fault::CondensatePumpTrip { index } | Fault::CwPumpTrip { index } => Some(index),
            Fault::RodStuck { index } => Some(index),
            Fault::IcTrainUnavailable { train }
            | Fault::IcSteamValveClosed { train }
            | Fault::IcNoncondensableDegradation { train, .. } => Some(train),
            Fault::IcReturnValveStuck { train, valve } => Some(train * N_IC_RETURN_VALVES + valve),
            Fault::TsvStuck
            | Fault::TcvStuck
            | Fault::BypassUnavailable
            | Fault::PressureRegulatorFailed { .. }
            | Fault::FcvStuck
            | Fault::HeatersLost
            | Fault::VacuumPumpFailed => None,
        }
    }

    /// Payload validation: percentages <= 100, indices within the plant's equipment counts.
    pub fn is_valid(&self) -> bool {
        match *self {
            Fault::MsivStuck { line } => line < N_STEAM_LINES,
            Fault::PressureRegulatorFailed { demand_pct } => demand_pct <= 100,
            Fault::FeedPumpTrip { index } => index < N_FEED_PUMPS,
            Fault::CondensatePumpTrip { index } => index < N_COND_PUMPS,
            Fault::CwPumpTrip { index } => index < N_CW_PUMPS,
            Fault::IcTrainUnavailable { train } | Fault::IcSteamValveClosed { train } => train < N_IC_TRAINS,
            Fault::IcReturnValveStuck { train, valve } => train < N_IC_TRAINS && valve < N_IC_RETURN_VALVES,
            Fault::IcNoncondensableDegradation { train, pct } => train < N_IC_TRAINS && pct <= 100,
            Fault::RodStuck { index } => index < N_RODS,
            Fault::TsvStuck | Fault::TcvStuck | Fault::BypassUnavailable | Fault::FcvStuck | Fault::HeatersLost | Fault::VacuumPumpFailed => true,
        }
    }
}
```

In `crates/bwr-core/src/sim.rs` add after `use crate::bop::{...};`:

```rust
use crate::faults::Fault;
```

In the `Sim` struct after `pub ledger: Ledger,` add:

```rust
    /// Active test-level fault overrides, in activation order (see faults.rs).
    /// Never cleared by automatic logic; cleared by scenario reload.
    pub faults: Vec<Fault>,
```

In `base`, in the struct literal after `ledger,` add `faults: Vec::new(),`.

Add to `impl Sim`, directly after `pub fn msivs_open(&self) -> bool { ... }`:

```rust
    /// Activate or clear a test-level fault override. Activating replaces any
    /// existing entry with the same `(kind, equipment)`; clearing removes it.
    /// Invalid payloads are ignored and logged.
    pub fn set_fault(&mut self, f: Fault, active: bool) {
        if !f.is_valid() {
            self.log.push((self.time, format!("fault: ignored invalid payload {f:?}")));
            return;
        }
        let (kind, eq) = (f.kind(), f.equipment());
        self.faults.retain(|g| !(g.kind() == kind && g.equipment() == eq));
        if active {
            self.faults.push(f);
        }
    }

    /// Exact-equality membership test.
    pub fn has_fault(&self, f: &Fault) -> bool {
        self.faults.contains(f)
    }

    /// Payload lookup by `(kind, equipment)`.
    pub fn find_fault(&self, kind: &str, equipment: Option<usize>) -> Option<&Fault> {
        self.faults.iter().find(|g| g.kind() == kind && g.equipment() == equipment)
    }
```

- [ ] **Step 5: Run tests**

Run: `cargo test --release -p bwr-core`
Expected: unit `20 passed`, integration `11 passed`.

- [ ] **Step 6: Commit**

```bash
git add crates/bwr-core/src/faults.rs crates/bwr-core/src/ic.rs crates/bwr-core/src/lib.rs crates/bwr-core/src/sim.rs docs/model/parameters.csv
git commit -m "feat(core): ordered engine-level fault overrides with payload validation

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 7: Validation module — cases, operators, traces (schema v2), CSV, runner

**Files:**
- Create: `crates/bwr-core/src/validation.rs`
- Modify: `crates/bwr-core/src/lib.rs`
- Modify: `docs/model/parameters.csv` (append the `OPERATOR_DT_S` row)

**Interfaces:**
- Consumes: `Sim` (public fields `time, core, vessel, steam, turbine, fw, ic, cond, scrammed, mode, ledger, log`), `Command`, `Fault`, `ModeSwitch`, `RATED_POWER_W`, `DT`.
- Produces: `ScriptedCommand`, `ScriptedFault`, `Case { id, scenario, duration_s, sample_dt_s, commands, faults, dt_after_s, operator }`, `Operator` enum, `OPERATOR_DT_S`, `TRACE_SCHEMA_VERSION = 2`, `N_TRACE_COLS = 58`, `TRACE_COLUMNS`, `col_index`, `LOG_KEYS` (the master's locked log-line keys), `Trace { case_id, dt_s, rows, events, integrated, extrema }`, `Extremum { max, t_max, min, t_min }`, `Trace::to_csv`, `Trace::from_csv(s)`, `Trace::column`, `run_case`, `run_case_with`, `run_sim_with`. Consumed by Task 8 (metrics), Task 9 (cases), Task 10 (example), P3 (convergence), P7 (`dt_after_s`), P9 (operators, `run_sim_with`).
- Master runner rules implemented here: timing validation (dt, `sample_dt_s`, `duration_s`, `dt_after_s.1` finite and > 0; `sample_dt_s` and `duration_s` integer multiples of every dt in force within 1e-9 relative; `dt_after_s.0` on a sample boundary; every failure panics with a message starting `case <id>:`); the log is drained after every boundary action and after every step, so terminal-boundary commands and operator actions reach `Trace.events` exactly once; one final row is always emitted at `t == duration_s`; `Trace.extrema` is accumulated at every physics step for every column (time of first occurrence) and serialised as `# extremum,<col>,<max>,<t_max>,<min>,<t_min>` lines; `from_csv` rejects a trace with no rows, missing `# case`/`# dt_s`/`# schema` metadata, a non-finite or non-positive `dt_s`, or missing `# integrated`/`# extremum` lines.
- Contract addition (reported to the coordinator): `Trace` carries `integrated: BTreeMap<String, f64>` (the ledger's integrated transfers at the end of the run) so `Metrics.integrated` can be "from `Ledger.integrated`" as the master requires; the CSV stores them as `# integrated,<key>,<value>` metadata lines.

- [ ] **Step 1: Write the failing unit tests**

Create `crates/bwr-core/src/validation.rs`:

```rust
//! Deterministic validation runner: scripted cases, machine-readable traces
//! (schema v2), metrics and tolerance comparison. See docs/validation/method.md.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::Command;
    use crate::sim::DT;

    fn tiny_trace() -> Trace {
        let mut rows = Vec::new();
        for i in 0..5 {
            let mut r = [0.0; N_TRACE_COLS];
            r[0] = i as f64;
            r[col_index("pressure_pa")] = 7.0e6 + 1.0e4 * (i as f64);
            r[col_index("pressure_pa_max_int")] = 7.0e6 + 1.0e4 * (i as f64) + 500.0;
            r[col_index("power_frac")] = 1.0;
            r[col_index("power_frac_max_int")] = 1.0;
            r[col_index("level_nr_cm")] = 0.0;
            rows.push(r);
        }
        let mut integrated = BTreeMap::new();
        integrated.insert("feed_kg".to_string(), 1234.5);
        // Extrema folded from the rows, plus a sub-sample pressure excursion (+500 Pa at
        // 3.5 s) that the sampled rows never show.
        let mut extrema = BTreeMap::new();
        for r in &rows {
            update_extrema(&mut extrema, r);
        }
        extrema.insert("pressure_pa".to_string(), Extremum { max: 7.04e6 + 500.0, t_max: 3.5, min: 7.0e6, t_min: 0.0 });
        Trace { case_id: "tiny".into(), dt_s: 0.05, rows, events: vec![(2.0, "scram: test".into())], integrated, extrema }
    }

    fn unit_case(id: &'static str, scenario: ScenarioName, duration_s: f64, commands: Vec<ScriptedCommand>) -> Case {
        Case { id, scenario, duration_s, sample_dt_s: 1.0, commands, faults: vec![], dt_after_s: None, operator: Operator::None }
    }

    /// +0.1 MPa pressure-setpoint pulse (raised at 2 s, restored at 4 s) at full power:
    /// dome pressure rises and falls back, so its maximum lies strictly inside the
    /// first 10 s and is not on any 5 s or 10 s sample row.
    fn pulse_case(id: &'static str, sample_dt_s: f64) -> Case {
        let mut c = unit_case(
            id,
            ScenarioName::FullPower,
            20.0,
            vec![
                ScriptedCommand { at_s: 2.0, cmd: Command::SetPressureSetpoint { mpa: 7.05 } },
                ScriptedCommand { at_s: 4.0, cmd: Command::SetPressureSetpoint { mpa: 6.95 } },
            ],
        );
        c.sample_dt_s = sample_dt_s;
        c
    }

    /// Message of the panic `f` raises (empty if it did not panic with a string payload).
    fn panic_message(f: impl FnOnce()) -> String {
        let payload = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).expect_err("expected a panic");
        payload
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| payload.downcast_ref::<&str>().map(|m| m.to_string()))
            .unwrap_or_default()
    }

    #[test]
    fn column_count_matches_schema() {
        assert_eq!(TRACE_COLUMNS.len(), N_TRACE_COLS);
        assert_eq!(TRACE_COLUMNS[0], "t");
        assert_eq!(TRACE_COLUMNS[N_TRACE_COLS - 1], "unsupported");
        assert_eq!(col_index("level_nr_cm_min_int"), col_index("level_nr_cm") + 1);
    }

    #[test]
    fn csv_roundtrip_is_exact() {
        let t = tiny_trace();
        let s = t.to_csv();
        let mut lines = s.lines();
        assert_eq!(lines.next(), Some("# schema,2"));
        assert_eq!(lines.next(), Some("# case,tiny"));
        assert!(lines.next().unwrap().starts_with("# dt_s,5.0000000000000003e-2"));
        assert!(s.contains("# integrated,feed_kg,"), "{s}");
        assert!(
            s.contains("# extremum,pressure_pa,7.0405000000000000e6,3.5000000000000000e0,7.0000000000000000e6,0.0000000000000000e0"),
            "{s}"
        );
        assert!(s.contains("\nt,power_frac,power_frac_max_int,thermal_mw,"), "{s}");
        assert!(s.contains("# event,2.0000000000000000e0,scram: test"), "{s}");
        let back = Trace::from_csv(&s).unwrap();
        assert_eq!(back, t);
    }

    #[test]
    fn csv_rejects_wrong_header_and_schema() {
        let err = Trace::from_csv("# schema,2\n# case,x\n# dt_s,0.05\na,b,c\n1,2,3\n").unwrap_err();
        assert!(err.contains("column header"), "{err}");
        let err = Trace::from_csv("# schema,1\n# case,x\n").unwrap_err();
        assert!(err.contains("schema"), "{err}");
    }

    #[test]
    fn csv_rejects_header_only_and_missing_metadata() {
        // A header-only file (schema, case, dt, column header, no rows) is what a
        // truncated or never-written baseline looks like; it must never load.
        let header = TRACE_COLUMNS.join(",");
        let err = Trace::from_csv(&format!("# schema,2\n# case,x\n# dt_s,0.05\n{header}\n")).unwrap_err();
        assert!(err.contains("no rows"), "{err}");
        // Dropping one metadata kind at a time from a complete trace is rejected each time.
        let full = tiny_trace().to_csv();
        for (prefix, expect) in [("# case,", "case"), ("# dt_s,", "dt_s"), ("# integrated,", "integrated"), ("# extremum,", "extremum")] {
            let without: String = full.lines().filter(|l| !l.starts_with(prefix)).map(|l| format!("{l}\n")).collect();
            let err = Trace::from_csv(&without).unwrap_err();
            assert!(err.contains(expect), "dropping {prefix}: {err}");
        }
        let err = Trace::from_csv(&full.replace("# dt_s,5.0000000000000003e-2", "# dt_s,NaN")).unwrap_err();
        assert!(err.contains("dt_s"), "{err}");
        // A row cut mid-line (partially written file) is rejected on its cell count.
        let rows_start = full.find(&header).unwrap() + header.len() + 1;
        let first_row_end = rows_start + full[rows_start..].find('\n').unwrap();
        let cut = format!("{}\n", full[..first_row_end].rsplit_once(',').unwrap().0);
        let err = Trace::from_csv(&cut).unwrap_err();
        assert!(err.contains("cells"), "{err}");
    }

    #[test]
    fn column_extracts_by_name() {
        let p = tiny_trace().column("pressure_pa");
        assert_eq!(p.len(), 5);
        assert!((p[4] - 7.04e6).abs() < 1e-6);
    }

    #[test]
    fn terminal_boundary_command_is_logged_and_applied() {
        // Regression: a command scheduled at t == duration_s used to change the final
        // state without ever reaching Trace.events (the log was drained only after a step).
        let case = unit_case(
            "unit_terminal_cmd",
            ScenarioName::FullPower,
            1.0,
            vec![ScriptedCommand { at_s: 1.0, cmd: Command::ManualScram }],
        );
        let tr = run_case(&case, DT);
        assert_eq!(tr.rows.len(), 2);
        assert_eq!(*tr.column("scrammed").last().unwrap(), 1.0, "final row shows the scram");
        let scrams: Vec<&(f64, String)> = tr.events.iter().filter(|(_, e)| e.starts_with("scram")).collect();
        assert_eq!(scrams.len(), 1, "exactly one scram event: {:?}", tr.events);
        assert!((scrams[0].0 - 1.0).abs() < 1e-9, "scram logged at the terminal boundary, got {}", scrams[0].0);
    }

    #[test]
    fn terminal_operator_action_is_logged_once() {
        let case = unit_case("unit_terminal_op", ScenarioName::FullPower, 2.0, vec![]);
        let tr = run_case_with(&case, DT, |_, t| if (t - 2.0).abs() < 1e-9 { vec![Command::ManualScram] } else { Vec::new() });
        let scrams: Vec<&(f64, String)> = tr.events.iter().filter(|(_, e)| e.starts_with("scram")).collect();
        assert_eq!(scrams.len(), 1, "operator action at the terminal boundary logged exactly once: {:?}", tr.events);
        assert!((scrams[0].0 - 2.0).abs() < 1e-9, "{}", scrams[0].0);
        assert_eq!(*tr.column("scrammed").last().unwrap(), 1.0);
    }

    #[test]
    fn final_row_is_always_emitted_at_duration() {
        // Regression: duration 1.5 s with 1 s sampling used to end with a final row
        // describing t = 1 s. The run must always end with a row at t == duration_s.
        let mut case = unit_case("unit_final_row", ScenarioName::ColdShutdown, 1.5, vec![]);
        case.sample_dt_s = 1.0;
        let (tr, sim) = run_sim_with(crate::Sim::new(case.scenario), &case, DT, |_, _| Vec::new());
        let t = tr.column("t");
        assert_eq!(t.len(), 3, "rows at 0, 1 and the terminal 1.5 s: {t:?}");
        assert!((t[1] - 1.0).abs() < 1e-9, "{t:?}");
        assert!((t[2] - 1.5).abs() < 1e-9, "final row describes t = duration_s, got {}", t[2]);
        assert!((sim.time - 1.5).abs() < 1e-9, "{}", sim.time);
        // When duration is itself a sample boundary there is still exactly one final row.
        let aligned = unit_case("unit_final_row_aligned", ScenarioName::ColdShutdown, 2.0, vec![]);
        let tr2 = run_case(&aligned, DT);
        assert_eq!(tr2.rows.len(), 3, "{:?}", tr2.column("t"));
    }

    #[test]
    fn timing_validation_rejects_malformed_cases_naming_the_case() {
        let base = |id: &'static str| unit_case(id, ScenarioName::ColdShutdown, 2.0, vec![]);
        let mut sample_off = base("unit_sample_off");
        sample_off.sample_dt_s = 0.07; // not an integer multiple of the 0.05 s step
        let mut duration_off = base("unit_duration_off");
        duration_off.duration_s = 1.02;
        let mut frozen = base("unit_frozen_clock");
        frozen.dt_after_s = Some((1.0, 0.0)); // would stop the clock forever
        let mut negative = base("unit_negative_switch");
        negative.dt_after_s = Some((1.0, -0.1));
        let mut nan_sample = base("unit_nan_sample");
        nan_sample.sample_dt_s = f64::NAN;
        let mut inf_duration = base("unit_inf_duration");
        inf_duration.duration_s = f64::INFINITY;
        let mut off_boundary = base("unit_switch_off_boundary");
        off_boundary.dt_after_s = Some((1.5, 0.1)); // 1.5 s is not on the 1 s sample grid
        let mut switch_misaligned = base("unit_switch_misaligned");
        switch_misaligned.dt_after_s = Some((1.0, 0.3)); // 2.0 s duration is not a multiple of 0.3 s
        for (case, expect) in [
            (sample_off, "sample_dt_s"),
            (duration_off, "duration_s"),
            (frozen, "dt_after_s.1"),
            (negative, "dt_after_s.1"),
            (nan_sample, "sample_dt_s"),
            (inf_duration, "duration_s"),
            (off_boundary, "sample boundary"),
            (switch_misaligned, "dt_after_s.1"),
        ] {
            let msg = panic_message(|| {
                let _ = run_case(&case, DT);
            });
            assert!(msg.starts_with(&format!("case {}:", case.id)), "{}: diagnostic must name the case: {msg}", case.id);
            assert!(msg.contains(expect), "{}: {msg}", case.id);
        }
        let msg = panic_message(|| {
            let _ = run_case(&base("unit_bad_dt"), f64::NAN);
        });
        assert!(msg.starts_with("case unit_bad_dt:") && msg.contains("dt must be"), "{msg}");
    }

    #[test]
    fn extrema_are_accumulated_per_physics_step_not_per_sample() {
        let dense = run_case(&pulse_case("unit_extrema_dense", DT), DT);
        let sparse = run_case(&pulse_case("unit_extrema_sparse", 5.0), DT);
        assert_eq!(dense.extrema, sparse.extrema, "same physics must give identical extrema at 0.05 s and 5 s sampling");
        let e = sparse.extrema["pressure_pa"];
        assert!(e.max > e.min && e.t_max > 2.0 && e.t_max < 20.0, "peak lies inside the run: {e:?}");
        // The dense rows hold every physics state, so their sampled maximum is the extremum.
        let (dp, dt_col) = (dense.column("pressure_pa"), dense.column("t"));
        let k = dp.iter().enumerate().fold(0, |k, (i, v)| if *v > dp[k] { i } else { k });
        assert_eq!((dp[k], dt_col[k]), (e.max, e.t_max));
        assert_eq!(sparse.extrema.len(), N_TRACE_COLS - 1, "one extremum per column except t");
    }

    #[test]
    fn run_case_samples_at_requested_interval_and_applies_commands() {
        let case = unit_case(
            "unit_scram",
            ScenarioName::FullPower,
            10.0,
            vec![ScriptedCommand { at_s: 3.0, cmd: Command::ManualScram }],
        );
        let tr = run_case(&case, DT);
        assert_eq!(tr.rows.len(), 11, "t = 0..=10 inclusive");
        assert!((tr.rows[0][0] - 0.0).abs() < 1e-12);
        assert!((tr.rows[10][0] - 10.0).abs() < 1e-6);
        let scrammed = tr.column("scrammed");
        assert_eq!(scrammed[2], 0.0);
        assert_eq!(scrammed[3], 1.0, "command at 3 s applies before the step that starts at 3 s");
        let (t, text) = &tr.events[0];
        assert!(text.starts_with("scram: Manual scram"), "{text}");
        assert!((t - 3.0).abs() < 1e-6, "scram at {t}");
        let feed_max = tr.column("feed_flow_kgs").iter().cloned().fold(0.0, f64::max);
        assert!(tr.integrated["feed_kg"] > 0.0 && tr.integrated["feed_kg"] <= 10.0 * feed_max + 1e-6, "{:?}", tr.integrated);
        assert!(tr.integrated.contains_key("loss_j"), "all ledger keys are carried: {:?}", tr.integrated);
        assert_eq!(tr.column("mode")[0], 3.0, "full power runs in RUN");
    }

    #[test]
    fn interval_extrema_catch_sub_sample_excursions() {
        // Fixture with a known interior excursion: the setpoint pulse (2 s to 4 s) makes
        // dome pressure rise and fall back inside the first 10 s sample interval. The
        // sparse (10 s) trace must report an interval maximum that (a) strictly exceeds
        // both interval endpoints and (b) equals the maximum of the dense (0.05 s) trace,
        // whose rows hold every physics state. An endpoint-only mutation of the
        // accumulation (updating `Interval` only at sample rows) fails (a); updating at
        // some but not all steps fails (b).
        let sparse = run_case(&pulse_case("unit_interval_sparse", 10.0), DT);
        let dense = run_case(&pulse_case("unit_interval_dense", DT), DT);
        let p = sparse.column("pressure_pa");
        let pm = sparse.column("pressure_pa_max_int");
        assert_eq!(p.len(), 3, "rows at 0, 10 and 20 s");
        assert_eq!(pm[0], p[0], "first row is seeded from the initial state");
        assert!(pm[1] > p[0] && pm[1] > p[1], "interval max {} must exceed both endpoints {} and {}", pm[1], p[0], p[1]);
        let dense_max = dense
            .column("t")
            .iter()
            .zip(dense.column("pressure_pa"))
            .filter(|(t, _)| **t > 0.0 && **t <= 10.0 + 1e-9)
            .map(|(_, v)| v)
            .fold(f64::NEG_INFINITY, f64::max);
        assert!((pm[1] - dense_max).abs() <= 1e-9, "sparse interval max {} vs dense max {}", pm[1], dense_max);
        for k in 1..p.len() {
            assert!(pm[k] >= p[k] - 1e-9 && pm[k] >= p[k - 1] - 1e-9, "row {k}: max_int {} < samples {} {}", pm[k], p[k - 1], p[k]);
        }
        let lo = sparse.column("level_nr_cm_min_int");
        let hi = sparse.column("level_nr_cm_max_int");
        assert!(lo.iter().zip(&hi).all(|(a, b)| a <= b));
    }

    #[test]
    fn run_case_applies_faults_and_dt_switch() {
        let mut case = unit_case("unit_fault", ScenarioName::HotStandby, 4.0, vec![]);
        case.faults = vec![ScriptedFault { at_s: 1.0, fault: Fault::BypassUnavailable, active: true }];
        case.dt_after_s = Some((2.0, 0.1));
        let (tr, sim) = run_sim_with(crate::Sim::new(case.scenario), &case, DT, |_, _| Vec::new());
        assert_eq!(tr.rows.len(), 5);
        assert!(sim.has_fault(&Fault::BypassUnavailable));
        assert!((sim.time - 4.0).abs() < 1e-6, "clock ends at duration across the dt switch: {}", sim.time);
        assert_eq!(tr.dt_s, DT, "trace records the initial dt");
    }

    #[test]
    fn operator_is_consulted_once_per_operator_dt() {
        let case = unit_case("unit_operator", ScenarioName::ColdShutdown, 5.0, vec![]);
        let mut calls = Vec::new();
        let _ = run_case_with(&case, DT, |_, t| {
            calls.push(t);
            Vec::new()
        });
        assert_eq!(calls.len(), 6, "t = 0,1,2,3,4,5: {calls:?}");
        assert!((calls[1] - OPERATOR_DT_S).abs() < 1e-9);
    }

    #[test]
    #[should_panic(expected = "case unit_reject")]
    fn runtime_control_commands_are_rejected() {
        let case = unit_case(
            "unit_reject",
            ScenarioName::ColdShutdown,
            1.0,
            vec![ScriptedCommand { at_s: 0.0, cmd: Command::SetSpeed { speed: 10.0 } }],
        );
        let _ = run_case(&case, DT);
    }
}
```

- [ ] **Step 2: Register the module and verify failure**

Edit `crates/bwr-core/src/lib.rs`: insert `pub mod validation;` between `pub mod steam_table;` and `pub mod vessel;`.

Run: `cargo test --release -p bwr-core validation`
Expected: compile errors (`cannot find type `Trace``, `cannot find function `run_case``).

- [ ] **Step 3: Implement cases, operators, traces, CSV and the runner**

Insert above `#[cfg(test)]` in `validation.rs`:

```rust
use std::collections::BTreeMap;

use crate::api::{Command, ModeSwitch, ScenarioName};
use crate::faults::Fault;
use crate::params::RATED_POWER_W;
use crate::Sim;

#[derive(Clone, Debug)]
pub struct ScriptedCommand {
    pub at_s: f64,
    pub cmd: Command,
}

#[derive(Clone, Debug)]
pub struct ScriptedFault {
    pub at_s: f64,
    pub fault: Fault,
    pub active: bool,
}

/// Closed-loop operators (Package 9). Named variants so `Case` stays `Clone + Debug`
/// without boxing closures. Package 2 implements only `None`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operator {
    None,
    Startup { target_frac_pct: u8 },
    PowerChange { from_pct: u8, to_pct: u8 },
    ControlledShutdown,
    CooldownOnIc,
    RodWithdrawalError,
}

/// Operators are consulted once per this much simulation time.
pub const OPERATOR_DT_S: f64 = 1.0;

/// A scripted run. Commands and faults are applied at the physics boundary that
/// starts at or after `at_s`, before that step is taken.
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
    /// Closed-loop operator (P9). Package 2 case literals use `Operator::None`.
    pub operator: Operator,
}

pub const TRACE_SCHEMA_VERSION: u32 = 2;
pub const N_TRACE_COLS: usize = 58;

/// Column order is fixed. Columns whose source does not exist yet are emitted as 0.0
/// and documented in method.md as "populated from Package N".
pub const TRACE_COLUMNS: [&str; N_TRACE_COLS] = [
    "t",
    "power_frac", "power_frac_max_int", "thermal_mw", "decay_mw",
    "pressure_pa", "pressure_pa_max_int", "pressure_sensed_pa",
    "level_z_m", "collapsed_level_m",
    "level_nr_cm", "level_nr_cm_min_int", "level_nr_cm_max_int", "level_model_cm",
    "mass_kg", "energy_j", "gas_mass_kg",
    "core_flow_kgs", "steam_flow_kgs", "feed_flow_kgs", "t_feed_c",
    "t_coolant_c", "fuel_t_c", "clad_t_c", "moderator_t_c",
    "rods_avg_pos", "rods_min_pos", "rods_max_pos",
    "msiv_pos_0", "msiv_pos_1", "tsv_pos", "tcv_pos", "bypass_pos", "header_pressure_pa",
    "turbine_rpm", "gross_we", "scrammed", "turbine_tripped", "breaker_closed", "mode",
    "ic_heat_w_0", "ic_heat_w_1", "ic_heat_w_2",
    "ic_pool_kg_0", "ic_pool_kg_1", "ic_pool_kg_2",
    "ic_return_pos_0", "ic_return_pos_1", "ic_return_pos_2",
    "cond_pressure_pa", "hotwell_kg", "cst_kg",
    "fw_pump_0", "fw_pump_1", "fw_pump_2",
    "mass_residual_kg", "energy_residual_j", "unsupported",
];

/// Locked log-line keys (master): every `Sim::log` entry is "<key>: <detail>" with one
/// of these keys. A package that needs a new key appends it here in the same commit.
pub const LOG_KEYS: [&str; 14] = [
    "scram", "turbine trip", "msiv isolation", "ic auto-initiation", "feed pump trip",
    "domain error", "rod block", "sri", "ari", "breaker open", "command ignored",
    "overload", "ic model", "fault",
];

/// Index of a trace column. Panics on an unknown name (a programming error).
pub fn col_index(name: &str) -> usize {
    TRACE_COLUMNS
        .iter()
        .position(|c| *c == name)
        .unwrap_or_else(|| panic!("unknown trace column {name}"))
}

#[derive(Clone, Debug, PartialEq)]
pub struct Trace {
    pub case_id: String,
    /// Initial physics step (before any `dt_after_s` switch).
    pub dt_s: f64,
    pub rows: Vec<[f64; N_TRACE_COLS]>,
    /// (sim time, "<key>: <detail>") for scram, trips, isolations, IC initiation, domain errors.
    pub events: Vec<(f64, String)>,
    /// `Ledger::integrated` at the end of the run.
    pub integrated: BTreeMap<String, f64>,
    /// Per-column extrema with their times of FIRST occurrence, accumulated at every
    /// physics step (not at sample rows); serialised as `# extremum,<col>,<max>,<t_max>,<min>,<t_min>`
    /// lines. `Metrics.peak`/`min` read these, never the sampled rows.
    pub extrema: BTreeMap<String, Extremum>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Extremum {
    pub max: f64,
    pub t_max: f64,
    pub min: f64,
    pub t_min: f64,
}

/// Fold one physics-step state (a full sample row, `row[0]` = time) into the per-column
/// extrema. Strict comparisons keep the time of first occurrence. Every column except `t`.
fn update_extrema(extrema: &mut BTreeMap<String, Extremum>, row: &[f64; N_TRACE_COLS]) {
    let t = row[0];
    for (i, name) in TRACE_COLUMNS.iter().enumerate().skip(1) {
        let v = row[i];
        match extrema.get_mut(*name) {
            Some(e) => {
                if v > e.max {
                    e.max = v;
                    e.t_max = t;
                }
                if v < e.min {
                    e.min = v;
                    e.t_min = t;
                }
            }
            None => {
                extrema.insert(name.to_string(), Extremum { max: v, t_max: t, min: v, t_min: t });
            }
        }
    }
}

/// Extrema accumulated at every physics step since the previous sample row.
#[derive(Clone, Copy, Debug)]
struct Interval {
    power_frac_max: f64,
    pressure_max: f64,
    level_nr_min: f64,
    level_nr_max: f64,
}

impl Interval {
    fn seed(sim: &Sim) -> Self {
        let lvl = sim.vessel.level_nr_cm();
        Self { power_frac_max: sim.core.kin.n, pressure_max: sim.vessel.eos.p, level_nr_min: lvl, level_nr_max: lvl }
    }

    fn update(&mut self, sim: &Sim) {
        let lvl = sim.vessel.level_nr_cm();
        self.power_frac_max = self.power_frac_max.max(sim.core.kin.n);
        self.pressure_max = self.pressure_max.max(sim.vessel.eos.p);
        self.level_nr_min = self.level_nr_min.min(lvl);
        self.level_nr_max = self.level_nr_max.max(lvl);
    }
}

fn b(x: bool) -> f64 {
    if x { 1.0 } else { 0.0 }
}

fn mode_code(m: ModeSwitch) -> f64 {
    match m {
        ModeSwitch::Shutdown => 0.0,
        ModeSwitch::Refuel => 1.0,
        ModeSwitch::Startup => 2.0,
        ModeSwitch::Run => 3.0,
    }
}

fn sample(sim: &Sim, iv: &Interval) -> [f64; N_TRACE_COLS] {
    let c = &sim.core;
    let v = &sim.vessel;
    let ic = |i: usize, f: fn(&crate::ic::IcTrain) -> f64| sim.ic.get(i).map(f).unwrap_or(0.0);
    let fwp = |i: usize| sim.fw.pumps.get(i).map(|&r| b(r)).unwrap_or(0.0);
    let pos = &c.rods.pos;
    let n_rods = pos.len() as f64;
    [
        sim.time,
        c.kin.n,
        iv.power_frac_max,
        c.thermal_power() / 1e6,
        c.decay.power() * RATED_POWER_W / 1e6,
        v.eos.p,
        iv.pressure_max,
        0.0, // pressure_sensed_pa: populated from Package 8
        v.level_z,
        0.0, // collapsed_level_m: populated from Package 3
        v.level_nr_cm(),
        iv.level_nr_min,
        iv.level_nr_max,
        0.0, // level_model_cm: populated from Package 8
        v.mass,
        v.energy,
        0.0, // gas_mass_kg: populated from Package 4
        v.core_flow,
        sim.steam.total_flow() + v.vent_flow,
        sim.fw.flow,
        sim.fw.t_feed,
        v.eos.t,
        c.fuel_t,
        0.0, // clad_t_c: populated from Package 5
        0.0, // moderator_t_c: populated from Package 4
        pos.iter().sum::<f64>() / n_rods,
        pos.iter().cloned().fold(f64::INFINITY, f64::min),
        pos.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        sim.steam.msiv_pos[0],
        sim.steam.msiv_pos[1],
        0.0, // tsv_pos: populated from Package 6
        sim.steam.tcv_pos,
        sim.steam.bypass_pos,
        0.0, // header_pressure_pa: populated from Package 6
        sim.turbine.speed_rpm,
        sim.turbine.p_elec.max(0.0),
        b(sim.scrammed),
        b(sim.turbine.tripped),
        b(sim.turbine.breaker_closed),
        mode_code(sim.mode),
        ic(0, |t| t.heat),
        ic(1, |t| t.heat),
        ic(2, |t| t.heat),
        ic(0, |t| t.pool_mass),
        ic(1, |t| t.pool_mass),
        ic(2, |t| t.pool_mass),
        ic(0, |t| t.valve_pos), // max of the valve pair from Package 7
        ic(1, |t| t.valve_pos),
        ic(2, |t| t.valve_pos),
        sim.cond.pressure(),
        sim.cond.hotwell_mass,
        0.0, // cst_kg: populated from Package 6
        fwp(0),
        fwp(1),
        fwp(2),
        sim.ledger.mass_residual(v.mass),
        sim.ledger.energy_residual(v.energy),
        0.0, // unsupported: populated from Package 3
    ]
}

/// Scripted cases run at a fixed step with no scenario reload; these commands would
/// break reproducibility and are rejected loudly.
fn reject_runtime_command(case_id: &str, cmd: &Command) {
    if matches!(cmd, Command::LoadScenario { .. } | Command::SetPaused { .. } | Command::SetSpeed { .. }) {
        panic!("case {case_id}: {cmd:?} is not allowed in a scripted case");
    }
}

/// Run a case with a fixed physics step. Deterministic: same case and dt give an
/// identical `Trace`. `Case.operator` other than `None` is dispatched by Package 9.
pub fn run_case(case: &Case, dt: f64) -> Trace {
    match case.operator {
        Operator::None => run_case_with(case, dt, |_, _| Vec::new()),
        other => panic!("case {}: operator {other:?} is implemented in Package 9", case.id),
    }
}

/// Closed-loop variant: `operator` is called every `OPERATOR_DT_S` of sim time
/// (first at t = 0) and returns commands to apply at that boundary.
pub fn run_case_with<F: FnMut(&Sim, f64) -> Vec<Command>>(case: &Case, dt: f64, operator: F) -> Trace {
    run_sim_with(Sim::new(case.scenario), case, dt, operator).0
}

/// `x` is an integer multiple of `h` within 1e-9 relative (0 counts as a multiple).
fn is_multiple(x: f64, h: f64) -> bool {
    let n = (x / h).round();
    (x - n * h).abs() <= 1e-9 * x.abs().max(h)
}

/// Timing rules (master): `dt`, `sample_dt_s`, `duration_s` and any `dt_after_s.1` are
/// finite and > 0; `sample_dt_s` and `duration_s` are integer multiples of every dt in
/// force; `dt_after_s.0` is finite, >= 0 and on a sample boundary. Every failure panics
/// with a message that starts with `case <id>:` so a scripted case is diagnosed by name.
fn validate_timing(case: &Case, dt: f64) {
    let id = case.id;
    let positive = |name: &str, v: f64| {
        assert!(v.is_finite() && v > 0.0, "case {id}: {name} must be finite and > 0, got {v}");
    };
    positive("dt", dt);
    positive("sample_dt_s", case.sample_dt_s);
    positive("duration_s", case.duration_s);
    let mut steps = vec![("dt", dt)];
    if let Some((from, dt2)) = case.dt_after_s {
        positive("dt_after_s.1", dt2);
        assert!(from.is_finite() && from >= 0.0, "case {id}: dt_after_s.0 must be finite and >= 0, got {from}");
        assert!(
            is_multiple(from, case.sample_dt_s),
            "case {id}: dt_after_s.0 = {from} is not on a sample boundary (sample_dt_s = {})",
            case.sample_dt_s
        );
        steps.push(("dt_after_s.1", dt2));
    }
    for (name, h) in steps {
        assert!(
            is_multiple(case.sample_dt_s, h),
            "case {id}: sample_dt_s = {} is not an integer multiple of {name} = {h}",
            case.sample_dt_s
        );
        assert!(
            is_multiple(case.duration_s, h),
            "case {id}: duration_s = {} is not an integer multiple of {name} = {h}",
            case.duration_s
        );
    }
}

/// Same, on a caller-supplied `Sim`, returning it (initial-condition tests).
///
/// Runner clock `t` is monotonic and advances by the current step. At each
/// boundary, in this order: scripted commands with `at_s <= t + dt/2` (list order,
/// stable by time), then scripted faults likewise, then the operator if due, then
/// the log is drained into `events`, then a sample row if due. The run ends when `t`
/// reaches `duration_s`; a final row is always emitted there, and the log is drained
/// after every step as well, so terminal-boundary actions are recorded exactly once.
/// `extrema` is folded from the state after every physics step (seeded at t = 0).
pub fn run_sim_with<F: FnMut(&Sim, f64) -> Vec<Command>>(
    mut sim: Sim,
    case: &Case,
    dt: f64,
    mut operator: F,
) -> (Trace, Sim) {
    validate_timing(case, dt);
    let mut cmds = case.commands.clone();
    cmds.sort_by(|a, b| a.at_s.partial_cmp(&b.at_s).unwrap_or_else(|| panic!("case {}: command time is NaN", case.id)));
    for c in &cmds {
        reject_runtime_command(case.id, &c.cmd);
    }
    let mut faults = case.faults.clone();
    faults.sort_by(|a, b| a.at_s.partial_cmp(&b.at_s).unwrap_or_else(|| panic!("case {}: fault time is NaN", case.id)));
    let (mut ci, mut fi) = (0usize, 0usize);
    let mut trace = Trace {
        case_id: case.id.to_string(),
        dt_s: dt,
        rows: Vec::new(),
        events: Vec::new(),
        integrated: BTreeMap::new(),
        extrema: BTreeMap::new(),
    };
    let mut interval = Interval::seed(&sim);
    update_extrema(&mut trace.extrema, &sample(&sim, &interval));
    let mut cur_dt = dt;
    let mut t = 0.0_f64;
    let mut next_sample = 0.0_f64;
    let mut next_operator = 0.0_f64;
    loop {
        if let Some((from, dt2)) = case.dt_after_s {
            if t >= from - 0.5 * cur_dt {
                cur_dt = dt2;
            }
        }
        let due = t + 0.5 * cur_dt;
        while ci < cmds.len() && cmds[ci].at_s <= due {
            sim.apply(cmds[ci].cmd.clone());
            ci += 1;
        }
        while fi < faults.len() && faults[fi].at_s <= due {
            sim.set_fault(faults[fi].fault.clone(), faults[fi].active);
            fi += 1;
        }
        if t >= next_operator - 0.5 * cur_dt {
            for cmd in operator(&sim, t) {
                reject_runtime_command(case.id, &cmd);
                sim.apply(cmd);
            }
            next_operator += OPERATOR_DT_S;
        }
        // Boundary actions (including those at the terminal boundary) are recorded
        // now; `drain` empties the log, so no line can be recorded twice.
        trace.events.extend(sim.log.drain(..));
        let sample_due = t >= next_sample - 0.5 * cur_dt;
        let terminal = t >= case.duration_s - 0.5 * cur_dt;
        if sample_due || terminal {
            // One row at every sample boundary and always one at t == duration_s.
            trace.rows.push(sample(&sim, &interval));
            interval = Interval::seed(&sim);
            if sample_due {
                next_sample += case.sample_dt_s;
            }
        }
        if terminal {
            break;
        }
        sim.step(cur_dt);
        t += cur_dt;
        interval.update(&sim);
        update_extrema(&mut trace.extrema, &sample(&sim, &interval));
        trace.events.extend(sim.log.drain(..));
    }
    trace.integrated = sim.ledger.integrated.iter().map(|(k, v)| (k.to_string(), *v)).collect();
    (trace, sim)
}

impl Trace {
    /// CSV: `# schema,2`, `# case,<id>`, `# dt_s,<v>`, `# integrated,<key>,<v>` lines,
    /// `# extremum,<col>,<max>,<t_max>,<min>,<t_min>` lines, the column header, rows in
    /// `{:.16e}` (exact f64 round trip), then one `# event,<t>,<text>` line per event.
    /// `from_csv(to_csv(t)) == t`.
    pub fn to_csv(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!("# schema,{TRACE_SCHEMA_VERSION}\n"));
        s.push_str(&format!("# case,{}\n", self.case_id));
        s.push_str(&format!("# dt_s,{:.16e}\n", self.dt_s));
        for (k, v) in &self.integrated {
            s.push_str(&format!("# integrated,{k},{v:.16e}\n"));
        }
        for (k, e) in &self.extrema {
            s.push_str(&format!("# extremum,{k},{:.16e},{:.16e},{:.16e},{:.16e}\n", e.max, e.t_max, e.min, e.t_min));
        }
        s.push_str(&TRACE_COLUMNS.join(","));
        s.push('\n');
        for r in &self.rows {
            let cells: Vec<String> = r.iter().map(|v| format!("{v:.16e}")).collect();
            s.push_str(&cells.join(","));
            s.push('\n');
        }
        for (t, text) in &self.events {
            s.push_str(&format!("# event,{t:.16e},{text}\n"));
        }
        s
    }

    /// Rejects (master): a trace with no rows, a missing `# schema`, `# case` or `# dt_s`
    /// line, a non-finite or non-positive `dt_s`, or no `# integrated` / `# extremum`
    /// lines, so a header-only or truncated baseline can never be compared.
    pub fn from_csv(s: &str) -> Result<Trace, String> {
        let mut rows = Vec::new();
        let mut events = Vec::new();
        let mut integrated = BTreeMap::new();
        let mut extrema = BTreeMap::new();
        let mut case_id: Option<String> = None;
        let mut dt_s: Option<f64> = None;
        let mut schema_seen = false;
        let mut header_seen = false;
        let expected_header = TRACE_COLUMNS.join(",");
        for (i, raw) in s.lines().enumerate() {
            let ln = i + 1;
            let line = raw.trim();
            if line.is_empty() {
                continue;
            }
            if let Some(rest) = line.strip_prefix("# schema,") {
                let v: u32 = rest.trim().parse().map_err(|e| format!("line {ln}: schema: {e}"))?;
                if v != TRACE_SCHEMA_VERSION {
                    return Err(format!("line {ln}: trace schema {v} is not the supported schema {TRACE_SCHEMA_VERSION}"));
                }
                schema_seen = true;
            } else if let Some(rest) = line.strip_prefix("# case,") {
                case_id = Some(rest.trim().to_string());
            } else if let Some(rest) = line.strip_prefix("# dt_s,") {
                dt_s = Some(rest.trim().parse().map_err(|e| format!("line {ln}: dt_s: {e}"))?);
            } else if let Some(rest) = line.strip_prefix("# integrated,") {
                let (k, v) = rest.split_once(',').ok_or(format!("line {ln}: malformed integrated line"))?;
                let v: f64 = v.trim().parse().map_err(|e| format!("line {ln}: integrated {k}: {e}"))?;
                integrated.insert(k.trim().to_string(), v);
            } else if let Some(rest) = line.strip_prefix("# extremum,") {
                let f: Vec<&str> = rest.split(',').collect();
                if f.len() != 5 {
                    return Err(format!("line {ln}: extremum line needs col,max,t_max,min,t_min"));
                }
                let num = |k: usize| -> Result<f64, String> {
                    f[k].trim().parse().map_err(|e| format!("line {ln}: extremum {}: {e}", f[0]))
                };
                extrema.insert(f[0].trim().to_string(), Extremum { max: num(1)?, t_max: num(2)?, min: num(3)?, t_min: num(4)? });
            } else if let Some(rest) = line.strip_prefix("# event,") {
                let (t, text) = rest.split_once(',').ok_or(format!("line {ln}: malformed event"))?;
                let t: f64 = t.parse().map_err(|e| format!("line {ln}: event time: {e}"))?;
                events.push((t, text.to_string()));
            } else if line.starts_with('#') {
                continue;
            } else if !header_seen {
                if !schema_seen {
                    return Err(format!("line {ln}: missing '# schema,{TRACE_SCHEMA_VERSION}' line before the header"));
                }
                if line != expected_header {
                    return Err(format!("line {ln}: column header does not match TRACE_COLUMNS"));
                }
                header_seen = true;
            } else {
                let cells: Vec<&str> = line.split(',').collect();
                if cells.len() != N_TRACE_COLS {
                    return Err(format!("line {ln}: expected {N_TRACE_COLS} cells, got {}", cells.len()));
                }
                let mut row = [0.0; N_TRACE_COLS];
                for (k, c) in cells.iter().enumerate() {
                    row[k] = c.parse().map_err(|e| format!("line {ln}: cell {k}: {e}"))?;
                }
                rows.push(row);
            }
        }
        if !schema_seen {
            return Err("missing '# schema' line".to_string());
        }
        if !header_seen {
            return Err("no column header found".to_string());
        }
        let case_id = case_id.filter(|c| !c.is_empty()).ok_or("missing '# case' line")?;
        let dt_s = dt_s.ok_or("missing '# dt_s' line")?;
        if !(dt_s.is_finite() && dt_s > 0.0) {
            return Err(format!("dt_s {dt_s} is not finite and positive"));
        }
        if rows.is_empty() {
            return Err("trace has no rows (header-only or truncated file)".to_string());
        }
        if integrated.is_empty() {
            return Err("missing '# integrated' lines".to_string());
        }
        if extrema.is_empty() {
            return Err("missing '# extremum' lines".to_string());
        }
        Ok(Trace { case_id, dt_s, rows, events, integrated, extrema })
    }

    pub fn column(&self, name: &str) -> Vec<f64> {
        let i = col_index(name);
        self.rows.iter().map(|r| r[i]).collect()
    }
}
```

- [ ] **Step 3a: Register `OPERATOR_DT_S` in the parameter register**

Append to `docs/model/parameters.csv` (if Package 1 has not landed yet the file does not exist: create it with the locked header line `id,value,unit,definition,reference_state,source_design,status,source_ids,interval_lo,interval_hi,assumption_id,location,notes` first; Package 1's rows are concatenated on merge and Task 11 Step 5a catches any clash):

```csv
validation.OPERATOR_DT_S,1.0,s,Interval at which a closed-loop case operator is consulted by the validation runner,-,numerical,assumed,,0.5,5,A-016,validation.rs,runner setting not a plant parameter; Package 9 operators act on this cadence
```

Run (when `crates/bwr-core/tests/register.rs` exists): `cargo test --release -p bwr-core --test register`
Expected: `4 passed` (row shape, status and unique id accepted).

- [ ] **Step 4: Run the tests**

Run: `cargo test --release -p bwr-core validation`
Expected: `test result: ok. 15 passed` (including the `should_panic` test).

- [ ] **Step 5: Commit**

```bash
git add crates/bwr-core/src/validation.rs crates/bwr-core/src/lib.rs docs/model/parameters.csv
git commit -m "feat(core): deterministic validation runner, schema v2 traces, per-step extrema

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 8: Metrics, tolerance classes and comparison

**Files:**
- Modify: `crates/bwr-core/src/validation.rs`
- Modify: `docs/model/parameters.csv` (append one row per tolerance number)

**Interfaces:**
- Consumes: `Trace`.
- Produces: `Metrics` (`final_values`, `peak`, `min`, `first_event_time`, `max_abs_mass_residual_kg`, `max_abs_energy_residual_j`, `initial_drift_per_s`, `drift_per_s`, `integrated`, `nonfinite`), `Tolerance { rel, abs, event_time_s, extremum_time_s, residual_mass_kg, residual_energy_j }` with `numerical()`, `physical()`, `source_comparison(cases_csv_row)`, `compare(&Metrics, &Metrics, &Tolerance) -> Vec<String>`, `Trace::metrics()`. Used by Tasks 9–10, Package 3 (convergence assertions replace the numerical floors with 2x the measured dt-halving differences) and Package 9 (source comparison).
- Master rules implemented here: `Metrics.peak`/`min` come from `Trace.extrema` (physics-step resolution), never from sampled rows; `Metrics.nonfinite` covers rows, integrated values, extrema, event times and `dt_s`; `compare` reports a violation when either side has no rows, no integrated transfers or no extrema (a header-only baseline can never PASS), when either side is non-finite, or when any tolerance input is non-finite; `Tolerance::source_comparison` resolves `tolerance_class` by header name and requires exactly 8 fields.

- [ ] **Step 1: Add failing tests to the `tests` module in `validation.rs`**

```rust
    #[test]
    fn metrics_find_final_peak_min_drift_and_use_trace_extrema() {
        let m = tiny_trace().metrics();
        assert!((m.final_values["pressure_pa"] - 7.04e6).abs() < 1e-6);
        // Peak comes from Trace.extrema (the +500 Pa excursion at 3.5 s that no row shows).
        assert_eq!(m.peak["pressure_pa"], (7.04e6 + 500.0, 3.5));
        assert_eq!(m.min["pressure_pa"], (7.0e6, 0.0));
        assert_eq!(m.peak["power_frac"], (1.0, 0.0), "flat signal: first occurrence");
        assert!(!m.final_values.contains_key("pressure_pa_max_int"), "interval columns fold into their parent");
        assert!(!m.peak.contains_key("pressure_pa_max_int"));
        assert!((m.drift_per_s["pressure_pa"] - 1.0e4).abs() < 1e-6, "{}", m.drift_per_s["pressure_pa"]);
        assert!((m.initial_drift_per_s["pressure_pa"] - 1.0e4).abs() < 1e-6);
        assert_eq!(m.first_event_time["scram"], 2.0);
        assert_eq!(m.max_abs_mass_residual_kg, 0.0);
        assert_eq!(m.integrated["feed_kg"], 1234.5);
        assert!(!m.nonfinite);
        let mut t = tiny_trace();
        t.rows[2][col_index("fuel_t_c")] = f64::NAN;
        assert!(t.metrics().nonfinite);
    }

    #[test]
    fn compare_passes_identical_and_flags_changes() {
        let a = tiny_trace().metrics();
        assert!(compare(&a, &a, &Tolerance::numerical()).is_empty());
        let mut t = tiny_trace();
        let i = col_index("pressure_pa");
        t.rows[4][i] += 50_000.0; // 0.7 % change, above rel 1e-3 and abs 100 Pa
        t.events.clear();
        let b = t.metrics();
        let v = compare(&a, &b, &Tolerance::numerical());
        assert!(v.iter().any(|s| s.starts_with("pressure_pa: final")), "{v:?}");
        assert!(v.iter().any(|s| s.starts_with("event scram: missing")), "{v:?}");
        // The same change passes the looser physical class on value, still fails on the missing event.
        let vp = compare(&a, &b, &Tolerance::physical());
        assert!(!vp.iter().any(|s| s.starts_with("pressure_pa")), "{vp:?}");
        assert!(vp.iter().any(|s| s.starts_with("event scram")), "{vp:?}");
        // Non-finite candidate is always a violation.
        let mut t2 = tiny_trace();
        t2.rows[1][col_index("mass_kg")] = f64::INFINITY;
        assert!(compare(&a, &t2.metrics(), &Tolerance::physical()).iter().any(|s| s.contains("non-finite")));
    }

    #[test]
    fn compare_judges_residuals_absolutely() {
        let a = tiny_trace().metrics();
        let mut t = tiny_trace();
        t.rows[1][col_index("mass_residual_kg")] = 5.0e-3; // above the 1e-3 kg numerical tolerance
        let v = compare(&a, &t.metrics(), &Tolerance::numerical());
        assert!(v.iter().any(|s| s.contains("mass residual")), "{v:?}");
        let mut t2 = tiny_trace();
        t2.rows[1][col_index("mass_residual_kg")] = 5.0e-4; // under it: ok even though reference is 0
        assert!(!compare(&a, &t2.metrics(), &Tolerance::numerical()).iter().any(|s| s.contains("mass residual")));
    }

    #[test]
    fn compare_flags_event_and_extremum_timing() {
        let a = tiny_trace().metrics();
        let mut t = tiny_trace();
        t.events = vec![(2.5, "scram: test".into())];
        let v = compare(&a, &t.metrics(), &Tolerance::numerical());
        assert!(v.iter().any(|s| s.starts_with("event scram: at 2.5")), "{v:?}");
        assert!(compare(&a, &t.metrics(), &Tolerance::physical()).is_empty());
        // A distinct minimum that moves in time is flagged; a flat signal's argmax is not.
        let mut t3 = tiny_trace();
        // Same minimum value (7.0e6) but now first reached at t=2 instead of t=0: 2 s > 1 s extremum tolerance.
        t3.extrema.get_mut("pressure_pa").unwrap().t_min = 2.0;
        // power_frac is flat (max == final), so a moved argmax must not be reported.
        t3.extrema.get_mut("power_frac").unwrap().t_max = 3.0;
        let v3 = compare(&a, &t3.metrics(), &Tolerance::numerical());
        assert!(v3.iter().any(|s| s.starts_with("pressure_pa: min time")), "{v3:?}");
        assert!(!v3.iter().any(|s| s.starts_with("power_frac")), "flat power must not report a time violation: {v3:?}");
    }

    #[test]
    fn source_comparison_tolerance_parses_a_cases_csv_row() {
        let row = "fp_turbine_trip,fullPower,300,Turbine trip at rated power,NEDO-X Fig. 3,source(rel=0.1;event=5;extremum=20),validation,baseline";
        let t = Tolerance::source_comparison(row);
        assert_eq!(t.rel, 0.1);
        assert_eq!(t.event_time_s, 5.0);
        assert_eq!(t.extremum_time_s, 20.0);
        assert_eq!(t.abs["pressure_pa"], Tolerance::physical().abs["pressure_pa"]);
        let n = Tolerance::source_comparison("x,fullPower,1,p,none (regression),numerical,calibration,baseline");
        assert_eq!(n.rel, Tolerance::numerical().rel);
        assert_eq!(CASES_COLUMNS.iter().position(|c| *c == "tolerance_class"), Some(5), "resolved by header name");
    }

    #[test]
    #[should_panic(expected = "case unit_commas_row: cases.csv row has 9 fields")]
    fn source_comparison_rejects_unescaped_commas() {
        // A comma inside a free-text field shifts every later column; the row is rejected
        // rather than read positionally from either end.
        let row = "unit_commas_row,fullPower,300,Turbine trip, with commas in purpose,NEDO-X Fig. 3,source(rel=0.1;event=5;extremum=20),validation,baseline";
        let _ = Tolerance::source_comparison(row);
    }

    #[test]
    fn compare_rejects_empty_or_truncated_metrics() {
        // A schema/case/dt/header-only baseline yields empty metrics; every comparison
        // used to be skipped, giving a false PASS. Both sides must carry rows, integrals
        // and extrema.
        let good = tiny_trace().metrics();
        let empty = Metrics::default();
        let v = compare(&empty, &good, &Tolerance::numerical());
        assert!(v.iter().any(|s| s.starts_with("reference has no rows")), "{v:?}");
        let v = compare(&good, &empty, &Tolerance::numerical());
        assert!(v.iter().any(|s| s.starts_with("candidate has no rows")), "{v:?}");
        let mut no_int = tiny_trace();
        no_int.integrated.clear();
        let v = compare(&no_int.metrics(), &good, &Tolerance::numerical());
        assert!(v.iter().any(|s| s.starts_with("reference has no integrated")), "{v:?}");
        let mut no_ex = tiny_trace();
        no_ex.extrema.clear();
        let v = compare(&good, &no_ex.metrics(), &Tolerance::numerical());
        assert!(v.iter().any(|s| s.starts_with("candidate has no extrema")), "{v:?}");
        assert!(v.iter().any(|s| s == "pressure_pa: peak missing in candidate"), "{v:?}");
    }

    #[test]
    fn nonfinite_event_times_and_tolerances_are_violations() {
        let good = tiny_trace().metrics();
        // Regression: a NaN scram time used to compare as a pass.
        let mut t = tiny_trace();
        t.events.push((f64::NAN, "turbine trip: test".into()));
        let m = t.metrics();
        assert!(m.nonfinite, "a NaN event time must set nonfinite");
        assert!(compare(&good, &m, &Tolerance::numerical()).iter().any(|s| s == "candidate contains non-finite values"));
        assert!(compare(&m, &good, &Tolerance::numerical()).iter().any(|s| s == "reference contains non-finite values"));
        let mut t2 = tiny_trace();
        t2.dt_s = f64::INFINITY;
        assert!(t2.metrics().nonfinite, "non-finite metadata is non-finite");
        let mut t3 = tiny_trace();
        t3.extrema.get_mut("mass_kg").unwrap().t_max = f64::NAN;
        assert!(t3.metrics().nonfinite, "non-finite extremum time is non-finite");
        let mut tol = Tolerance::numerical();
        tol.rel = f64::NAN;
        assert!(compare(&good, &good, &tol).iter().any(|s| s.starts_with("tolerance contains non-finite")), "NaN tolerance must not pass");
        let mut tol2 = Tolerance::physical();
        tol2.abs.insert("pressure_pa".into(), f64::INFINITY);
        assert!(!compare(&good, &good, &tol2).is_empty());
    }

    #[test]
    fn peak_time_is_identical_at_dense_and_sparse_sampling() {
        // Peak times come from Trace.extrema, so they cannot depend on sample_dt_s.
        let dense = run_case(&pulse_case("unit_peak_dense", DT), DT).metrics();
        let sparse = run_case(&pulse_case("unit_peak_sparse", 5.0), DT).metrics();
        assert_eq!(dense.peak["pressure_pa"], sparse.peak["pressure_pa"]);
        assert_eq!(dense.min["level_nr_cm"], sparse.min["level_nr_cm"]);
        assert_eq!(dense.peak, sparse.peak);
        assert_eq!(dense.min, sparse.min);
        assert!(dense.peak["pressure_pa"].1 > 2.0, "peak after the setpoint step at 2 s: {:?}", dense.peak["pressure_pa"]);
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --release -p bwr-core validation`
Expected: compile errors (`no method named `metrics``, `cannot find type `Tolerance``).

- [ ] **Step 3: Implement**

Insert into `validation.rs` after the `impl Trace { ... }` block and before `#[cfg(test)]`:

```rust
/// Locked header of docs/validation/cases.csv (master); readers resolve columns by name.
pub const CASES_COLUMNS: [&str; 8] = ["id", "scenario", "duration_s", "purpose", "reference", "tolerance_class", "role", "status"];

fn is_interval_column(name: &str) -> bool {
    name.ends_with("_max_int") || name.ends_with("_min_int")
}

fn is_residual(name: &str) -> bool {
    name.ends_with("_residual_kg") || name.ends_with("_residual_j")
}

/// Scalar reductions of a trace. Keys are trace column names (except `t` and the
/// `*_int` columns, which fold into their parent's extremum) or event keys.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Metrics {
    pub final_values: BTreeMap<String, f64>,
    /// name -> (value, time of first maximum), from `Trace.extrema` (physics-step resolution).
    pub peak: BTreeMap<String, (f64, f64)>,
    /// name -> (value, time of first minimum), from `Trace.extrema`.
    pub min: BTreeMap<String, (f64, f64)>,
    /// Keyed by event text before the first ':'.
    pub first_event_time: BTreeMap<String, f64>,
    pub max_abs_mass_residual_kg: f64,
    pub max_abs_energy_residual_j: f64,
    /// Max |dx/dt| between consecutive samples over the FIRST 60 s: the initial-equilibrium residual.
    pub initial_drift_per_s: BTreeMap<String, f64>,
    /// |x(t_end) - x(t_end - 60 s)| / 60 s (whole trace if shorter), per column.
    pub drift_per_s: BTreeMap<String, f64>,
    /// `Ledger::integrated` at the end of the run.
    pub integrated: BTreeMap<String, f64>,
    /// True if any row value, integrated value, extremum value or time, event time or
    /// `dt_s` is NaN or infinite.
    pub nonfinite: bool,
}

impl Trace {
    pub fn metrics(&self) -> Metrics {
        let mut m = Metrics::default();
        m.integrated = self.integrated.clone();
        m.nonfinite = !self.dt_s.is_finite()
            || self.rows.iter().any(|r| r.iter().any(|v| !v.is_finite()))
            || self.integrated.values().any(|v| !v.is_finite())
            || self.extrema.values().any(|e| ![e.max, e.t_max, e.min, e.t_min].iter().all(|v| v.is_finite()))
            || self.events.iter().any(|(t, _)| !t.is_finite());
        if self.rows.is_empty() {
            return m;
        }
        let t: Vec<f64> = self.rows.iter().map(|r| r[0]).collect();
        let t_end = *t.last().unwrap();
        let j0 = t.iter().position(|&tt| tt >= t_end - 60.0).unwrap_or(0);
        for (i, name) in TRACE_COLUMNS.iter().enumerate().skip(1) {
            if is_interval_column(name) {
                continue;
            }
            let col: Vec<f64> = self.rows.iter().map(|r| r[i]).collect();
            m.final_values.insert(name.to_string(), *col.last().unwrap());
            // Extrema come from the per-step accumulation, never from the sampled rows.
            // A column absent from `extrema` (malformed trace) gets no peak/min; compare
            // reports it as missing.
            if let Some(e) = self.extrema.get(*name) {
                m.peak.insert(name.to_string(), (e.max, e.t_max));
                m.min.insert(name.to_string(), (e.min, e.t_min));
            }
            let span = t_end - t[j0];
            let drift = if span > 0.0 { (col.last().unwrap() - col[j0]).abs() / span } else { 0.0 };
            m.drift_per_s.insert(name.to_string(), drift);
            let mut init = 0.0_f64;
            for k in 1..col.len() {
                if t[k] > 60.0 + 1e-9 {
                    break;
                }
                let h = t[k] - t[k - 1];
                if h > 0.0 {
                    init = init.max((col[k] - col[k - 1]).abs() / h);
                }
            }
            m.initial_drift_per_s.insert(name.to_string(), init);
        }
        m.max_abs_mass_residual_kg = self.column("mass_residual_kg").iter().fold(0.0, |a, v| a.max(v.abs()));
        m.max_abs_energy_residual_j = self.column("energy_residual_j").iter().fold(0.0, |a, v| a.max(v.abs()));
        for (time, text) in &self.events {
            let key = text.split(':').next().unwrap_or(text).trim().to_string();
            m.first_event_time.entry(key).or_insert(*time);
        }
        m
    }
}

/// Comparison tolerance. `abs` is a per-column floor below which differences are ignored.
#[derive(Clone, Debug)]
pub struct Tolerance {
    pub rel: f64,
    pub abs: BTreeMap<String, f64>,
    pub event_time_s: f64,
    pub extremum_time_s: f64,
    pub residual_mass_kg: f64,
    pub residual_energy_j: f64,
}

/// Numerical-class absolute floor for a column (initial values; see method.md).
/// Every number here is registered in docs/model/parameters.csv as `validation.TOL_ABS_*`.
fn abs_floor(name: &str) -> f64 {
    match name {
        "power_frac" => 1e-4,
        "thermal_mw" => 0.1,
        "decay_mw" => 0.01,
        "pressure_pa" | "pressure_sensed_pa" | "header_pressure_pa" => 100.0,
        "level_z_m" | "collapsed_level_m" => 1e-3,
        "level_nr_cm" | "level_model_cm" => 0.1,
        "mass_kg" | "hotwell_kg" | "cst_kg" => 1.0,
        "energy_j" => 1e6,
        "gas_mass_kg" => 0.01,
        "core_flow_kgs" | "steam_flow_kgs" | "feed_flow_kgs" => 0.1,
        "t_feed_c" | "t_coolant_c" | "fuel_t_c" | "clad_t_c" | "moderator_t_c" => 0.01,
        "rods_avg_pos" | "rods_min_pos" | "rods_max_pos" => 0.01,
        "msiv_pos_0" | "msiv_pos_1" | "tsv_pos" | "tcv_pos" | "bypass_pos" => 1e-3,
        "turbine_rpm" => 0.1,
        "gross_we" => 1e4,
        "cond_pressure_pa" => 10.0,
        n if n.starts_with("ic_heat_w_") => 1e3,
        n if n.starts_with("ic_pool_kg_") => 1.0,
        n if n.starts_with("ic_return_pos_") => 1e-3,
        // Booleans, mode, pump states, unsupported, residuals (judged separately): exact.
        _ => 0.0,
    }
}

fn abs_floors(scale: f64) -> BTreeMap<String, f64> {
    TRACE_COLUMNS.iter().skip(1).map(|n| (n.to_string(), abs_floor(n) * scale)).collect()
}

impl Tolerance {
    /// Numerical class (same physics, different step/platform/refactor). Initial values:
    /// rel 1e-3, event 0.1 s, extremum time 1 s, residuals 1e-3 kg / 1e3 J, floors x1
    /// (registered as `validation.TOL_NUMERICAL_*` in docs/model/parameters.csv).
    /// Package 3 REPLACES these with 2x the measured dt-halving differences.
    pub fn numerical() -> Self {
        Self { rel: 1e-3, abs: abs_floors(1.0), event_time_s: 0.1, extremum_time_s: 1.0, residual_mass_kg: 1e-3, residual_energy_j: 1e3 }
    }

    /// Physical class (an intended model change that should preserve behaviour):
    /// rel 5e-2, event 2 s, extremum 10 s, floors x10, residuals 1e-2 kg / 1e4 J
    /// (registered as `validation.TOL_PHYSICAL_*`).
    pub fn physical() -> Self {
        Self { rel: 5e-2, abs: abs_floors(10.0), event_time_s: 2.0, extremum_time_s: 10.0, residual_mass_kg: 1e-2, residual_energy_j: 1e4 }
    }

    /// Source-comparison class from a `docs/validation/cases.csv` row (locked header
    /// `CASES_COLUMNS`). The `tolerance_class` column is resolved by header name and the
    /// row must have exactly `CASES_COLUMNS.len()` fields: fields never contain commas,
    /// and a row that does is rejected rather than read positionally. Accepted values:
    /// `numerical`, `physical`, or `source(rel=<r>;event=<s>;extremum=<s>)` which uses
    /// the physical floors and residual tolerances. Panics on a malformed row
    /// (test-support code); the message starts with the row's first field (the case id).
    pub fn source_comparison(cases_csv_row: &str) -> Self {
        let fields: Vec<&str> = cases_csv_row.split(',').map(str::trim).collect();
        let id = fields.first().copied().unwrap_or("");
        assert!(
            fields.len() == CASES_COLUMNS.len(),
            "case {id}: cases.csv row has {} fields, expected exactly {} ({}): {cases_csv_row}",
            fields.len(),
            CASES_COLUMNS.len(),
            CASES_COLUMNS.join(",")
        );
        let col = |name: &str| CASES_COLUMNS.iter().position(|c| *c == name).expect("locked cases.csv column");
        let class = fields[col("tolerance_class")];
        match class {
            "numerical" => Self::numerical(),
            "physical" => Self::physical(),
            c if c.starts_with("source(") && c.ends_with(')') => {
                let mut t = Self::physical();
                for kv in c["source(".len()..c.len() - 1].split(';') {
                    let (k, v) = kv.split_once('=').unwrap_or_else(|| panic!("bad tolerance term '{kv}' in {cases_csv_row}"));
                    let v: f64 = v.trim().parse().unwrap_or_else(|_| panic!("bad tolerance value '{kv}' in {cases_csv_row}"));
                    match k.trim() {
                        "rel" => t.rel = v,
                        "event" => t.event_time_s = v,
                        "extremum" => t.extremum_time_s = v,
                        other => panic!("unknown tolerance key '{other}' in {cases_csv_row}"),
                    }
                }
                t
            }
            other => panic!("unknown tolerance_class '{other}' in {cases_csv_row}"),
        }
    }
}

fn within(a: f64, b: f64, rel: f64, abs: f64) -> bool {
    (a - b).abs() <= abs.max(rel * a.abs().max(b.abs()))
}

/// One human-readable violation per metric outside tolerance; empty means pass.
///
/// Rules (master r2): a nonfinite candidate value is a violation; finals, peaks and
/// minimums compare value-wise with `rel`/`abs`; an extremum *time* is compared
/// (against `extremum_time_s`) only when the extremum differs from the final value,
/// because a flat signal has an arbitrary argmax; residuals are compared against the
/// absolute residual tolerances, never against the reference; integrated transfers
/// compare with `rel` and the mass/energy floors; a reference event missing from the
/// candidate, or more than `event_time_s` away, is a violation, and so is a candidate
/// event absent from the reference; drift is reported, not compared.
pub fn compare(reference: &Metrics, candidate: &Metrics, tol: &Tolerance) -> Vec<String> {
    let mut v = Vec::new();
    let abs_of = |name: &str| tol.abs.get(name).copied().unwrap_or(0.0);

    // Completeness and finiteness gates (master): empty metrics come from a header-only
    // or truncated trace and must never PASS by having nothing to compare.
    for (label, m) in [("reference", reference), ("candidate", candidate)] {
        if m.final_values.is_empty() {
            v.push(format!("{label} has no rows (no final values)"));
        }
        if m.integrated.is_empty() {
            v.push(format!("{label} has no integrated transfers"));
        }
        if m.peak.is_empty() || m.min.is_empty() {
            v.push(format!("{label} has no extrema"));
        }
        if m.nonfinite {
            v.push(format!("{label} contains non-finite values"));
        }
    }
    let scalars = [tol.rel, tol.event_time_s, tol.extremum_time_s, tol.residual_mass_kg, tol.residual_energy_j];
    if scalars.iter().any(|x| !x.is_finite()) || tol.abs.values().any(|x| !x.is_finite()) {
        v.push("tolerance contains non-finite values".to_string());
    }
    for (name, r) in &reference.final_values {
        if is_residual(name) {
            continue;
        }
        let Some(c) = candidate.final_values.get(name) else {
            v.push(format!("{name}: missing in candidate"));
            continue;
        };
        if !within(*r, *c, tol.rel, abs_of(name)) {
            v.push(format!("{name}: final {c:.6e} vs reference {r:.6e}"));
        }
    }
    for (label, rmap, cmap) in [("peak", &reference.peak, &candidate.peak), ("min", &reference.min, &candidate.min)] {
        for (name, (rv, rt)) in rmap {
            if is_residual(name) {
                continue;
            }
            let Some((cv, ct)) = cmap.get(name) else {
                v.push(format!("{name}: {label} missing in candidate"));
                continue;
            };
            if !within(*rv, *cv, tol.rel, abs_of(name)) {
                v.push(format!("{name}: {label} value {cv:.6e} vs reference {rv:.6e}"));
                continue;
            }
            let distinct =
                reference.final_values.get(name).map(|f| !within(*rv, *f, tol.rel, abs_of(name))).unwrap_or(false);
            if distinct && (rt - ct).abs() > tol.extremum_time_s {
                v.push(format!("{name}: {label} time {ct:.2} s vs reference {rt:.2} s"));
            }
        }
    }
    if candidate.max_abs_mass_residual_kg > tol.residual_mass_kg {
        v.push(format!("max |mass residual| {:.4e} kg exceeds {:.1e} kg", candidate.max_abs_mass_residual_kg, tol.residual_mass_kg));
    }
    if candidate.max_abs_energy_residual_j > tol.residual_energy_j {
        v.push(format!("max |energy residual| {:.4e} J exceeds {:.1e} J", candidate.max_abs_energy_residual_j, tol.residual_energy_j));
    }
    for (key, r) in &reference.integrated {
        let floor = if key.ends_with("_kg") { abs_of("mass_kg") } else { abs_of("energy_j") };
        match candidate.integrated.get(key) {
            None => v.push(format!("integrated {key}: missing in candidate")),
            Some(c) if !within(*r, *c, tol.rel, floor) => v.push(format!("integrated {key}: {c:.6e} vs reference {r:.6e}")),
            _ => {}
        }
    }
    for (key, rt) in &reference.first_event_time {
        match candidate.first_event_time.get(key) {
            None => v.push(format!("event {key}: missing (reference at {rt:.2} s)")),
            Some(ct) if (rt - ct).abs() > tol.event_time_s => v.push(format!("event {key}: at {ct:.2} s vs reference {rt:.2} s")),
            _ => {}
        }
    }
    for (key, ct) in &candidate.first_event_time {
        if !reference.first_event_time.contains_key(key) {
            v.push(format!("event {key}: unexpected at {ct:.2} s"));
        }
    }
    v
}
```

- [ ] **Step 3a: Register every tolerance number in the parameter register**

Append to `docs/model/parameters.csv` (create it with the locked header first if Package 1 has not landed; see Task 7 Step 3a). One row per number in `Tolerance::numerical()`, `Tolerance::physical()` and `abs_floor`; the `notes` column names the trace columns each floor applies to:

```csv
validation.TOL_NUMERICAL_REL,1e-3,-,Numerical-class relative tolerance on final and extremum values and integrated transfers,-,numerical,assumed,,none,none,A-016,validation.rs Tolerance::numerical(),Package 3 replaces with 2x the measured dt-halving difference
validation.TOL_NUMERICAL_EVENT_S,0.1,s,Numerical-class tolerance on first-event times,-,numerical,assumed,,none,none,A-016,validation.rs Tolerance::numerical(),Package 3 replaces from the convergence study
validation.TOL_NUMERICAL_EXTREMUM_S,1.0,s,Numerical-class tolerance on extremum times (checked only when the extremum differs from the final value),-,numerical,assumed,,none,none,A-016,validation.rs Tolerance::numerical(),Package 3 replaces from the convergence study
validation.TOL_NUMERICAL_RESIDUAL_MASS_KG,1e-3,kg,Numerical-class absolute limit on max |mass residual|,-,numerical,assumed,,none,none,A-016,validation.rs Tolerance::numerical(),judged absolutely never against the reference
validation.TOL_NUMERICAL_RESIDUAL_ENERGY_J,1e3,J,Numerical-class absolute limit on max |energy residual|,-,numerical,assumed,,none,none,A-016,validation.rs Tolerance::numerical(),judged absolutely never against the reference
validation.TOL_PHYSICAL_REL,5e-2,-,Physical-class relative tolerance on final and extremum values and integrated transfers,-,numerical,assumed,,none,none,A-016,validation.rs Tolerance::physical(),intended model change that should preserve behaviour
validation.TOL_PHYSICAL_EVENT_S,2.0,s,Physical-class tolerance on first-event times,-,numerical,assumed,,none,none,A-016,validation.rs Tolerance::physical(),
validation.TOL_PHYSICAL_EXTREMUM_S,10.0,s,Physical-class tolerance on extremum times,-,numerical,assumed,,none,none,A-016,validation.rs Tolerance::physical(),
validation.TOL_PHYSICAL_RESIDUAL_MASS_KG,1e-2,kg,Physical-class absolute limit on max |mass residual|,-,numerical,assumed,,none,none,A-016,validation.rs Tolerance::physical(),
validation.TOL_PHYSICAL_RESIDUAL_ENERGY_J,1e4,J,Physical-class absolute limit on max |energy residual|,-,numerical,assumed,,none,none,A-016,validation.rs Tolerance::physical(),
validation.TOL_PHYSICAL_FLOOR_SCALE,10,-,Physical-class absolute floors are the numerical floors times this factor,-,numerical,assumed,,none,none,A-016,validation.rs Tolerance::physical(),
validation.TOL_ABS_POWER_FRAC,1e-4,-,Numerical-class absolute floor on neutron power fraction,-,numerical,assumed,,none,none,A-016,validation.rs abs_floor(),power_frac
validation.TOL_ABS_THERMAL_MW,0.1,MW,Numerical-class absolute floor on thermal power,-,numerical,assumed,,none,none,A-016,validation.rs abs_floor(),thermal_mw
validation.TOL_ABS_DECAY_MW,0.01,MW,Numerical-class absolute floor on decay heat,-,numerical,assumed,,none,none,A-016,validation.rs abs_floor(),decay_mw
validation.TOL_ABS_PRESSURE_PA,100,Pa,Numerical-class absolute floor on vessel and header pressures,-,numerical,assumed,,none,none,A-016,validation.rs abs_floor(),pressure_pa; pressure_sensed_pa; header_pressure_pa
validation.TOL_ABS_LEVEL_M,1e-3,m,Numerical-class absolute floor on levels in metres,-,numerical,assumed,,none,none,A-016,validation.rs abs_floor(),level_z_m; collapsed_level_m
validation.TOL_ABS_LEVEL_CM,0.1,cm,Numerical-class absolute floor on narrow-range levels,-,numerical,assumed,,none,none,A-016,validation.rs abs_floor(),level_nr_cm; level_model_cm
validation.TOL_ABS_INVENTORY_KG,1.0,kg,Numerical-class absolute floor on inventories,-,numerical,assumed,,none,none,A-016,validation.rs abs_floor(),mass_kg; hotwell_kg; cst_kg; ic_pool_kg_i
validation.TOL_ABS_ENERGY_J,1e6,J,Numerical-class absolute floor on vessel energy,-,numerical,assumed,,none,none,A-016,validation.rs abs_floor(),energy_j
validation.TOL_ABS_GAS_MASS_KG,0.01,kg,Numerical-class absolute floor on noncondensable gas mass,-,numerical,assumed,,none,none,A-016,validation.rs abs_floor(),gas_mass_kg
validation.TOL_ABS_FLOW_KGS,0.1,kg/s,Numerical-class absolute floor on mass flows,-,numerical,assumed,,none,none,A-016,validation.rs abs_floor(),core_flow_kgs; steam_flow_kgs; feed_flow_kgs
validation.TOL_ABS_TEMPERATURE_C,0.01,degC,Numerical-class absolute floor on temperatures,-,numerical,assumed,,none,none,A-016,validation.rs abs_floor(),t_feed_c; t_coolant_c; fuel_t_c; clad_t_c; moderator_t_c
validation.TOL_ABS_ROD_POS,0.01,position,Numerical-class absolute floor on rod positions,-,numerical,assumed,,none,none,A-016,validation.rs abs_floor(),rods_avg_pos; rods_min_pos; rods_max_pos
validation.TOL_ABS_VALVE_POS,1e-3,-,Numerical-class absolute floor on valve positions,-,numerical,assumed,,none,none,A-016,validation.rs abs_floor(),msiv_pos_i; tsv_pos; tcv_pos; bypass_pos; ic_return_pos_i
validation.TOL_ABS_TURBINE_RPM,0.1,rpm,Numerical-class absolute floor on turbine speed,-,numerical,assumed,,none,none,A-016,validation.rs abs_floor(),turbine_rpm
validation.TOL_ABS_GROSS_WE,1e4,W,Numerical-class absolute floor on gross electrical output,-,numerical,assumed,,none,none,A-016,validation.rs abs_floor(),gross_we
validation.TOL_ABS_COND_PRESSURE_PA,10,Pa,Numerical-class absolute floor on condenser pressure,-,numerical,assumed,,none,none,A-016,validation.rs abs_floor(),cond_pressure_pa
validation.TOL_ABS_IC_HEAT_W,1e3,W,Numerical-class absolute floor on IC heat removal,-,numerical,assumed,,none,none,A-016,validation.rs abs_floor(),ic_heat_w_i
```

Booleans, mode, pump states, `unsupported` and the residual columns have an exact (0) floor and no row: 0 is not a tuned number.

Run (when `crates/bwr-core/tests/register.rs` exists): `cargo test --release -p bwr-core --test register`
Expected: `4 passed`.

- [ ] **Step 4: Run the tests**

Run: `cargo test --release -p bwr-core validation`
Expected: `test result: ok. 24 passed`, no warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/bwr-core/src/validation.rs docs/model/parameters.csv
git commit -m "feat(core): trace metrics, three tolerance classes and comparison rules

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 9: Standard cases, determinism and convergence tests

**Files:**
- Modify: `crates/bwr-core/src/validation.rs` (add `standard_cases`)
- Modify: `crates/bwr-core/tests/plant.rs` (append four tests)

**Interfaces:**
- Consumes: `Case`, `Operator`, `run_case`, `compare`, `Tolerance`, `Command`, `PRESSURE_SETPOINT_DEFAULT`, `common::assert_finite`.
- Produces: `standard_cases() -> Vec<Case>` with ids `fp_steady_600`, `fp_manual_scram`, `fp_turbine_trip`, `fp_msiv_closure`, `fp_pressure_step`, `fp_loss_fw_heating`, `fp_load_reject`, `fp_fw_pump_trip`, `hs_steady_600`, `cs_steady_600`. Package 9 appends cases here and rows to `docs/validation/cases.csv`.

- [ ] **Step 1: Write the failing tests**

In `crates/bwr-core/tests/plant.rs`, add to the imports:

```rust
use bwr_core::validation::{compare, run_case, standard_cases, Case, Tolerance, LOG_KEYS};
use common::assert_finite;
```

and append:

```rust
fn case(id: &str) -> Case {
    standard_cases().into_iter().find(|c| c.id == id).unwrap_or_else(|| panic!("no case {id}"))
}

#[test]
fn validation_runner_is_deterministic() {
    let c = case("fp_turbine_trip");
    let a = run_case(&c, DT);
    let b = run_case(&c, DT);
    assert_eq!(a, b);
    assert_eq!(a.rows.len(), 301);
    assert!(a.events.iter().any(|(_, e)| e.starts_with("turbine trip")), "events: {:?}", a.events);
}

#[test]
fn standard_cases_are_complete_and_run() {
    let ids: Vec<&str> = standard_cases().iter().map(|c| c.id).collect();
    assert_eq!(
        ids,
        [
            "fp_steady_600", "fp_manual_scram", "fp_turbine_trip", "fp_msiv_closure", "fp_pressure_step",
            "fp_loss_fw_heating", "fp_load_reject", "fp_fw_pump_trip", "hs_steady_600", "cs_steady_600",
        ]
    );
    for c in standard_cases() {
        let tr = run_case(&c, DT);
        assert_eq!(tr.rows.len(), (c.duration_s / c.sample_dt_s).round() as usize + 1, "{}", c.id);
        assert_finite(c.id, tr.rows.iter().flat_map(|r| r.iter().copied()));
        assert!(!tr.metrics().nonfinite, "{}", c.id);
    }
}

#[test]
fn standard_case_log_lines_use_locked_keys() {
    for c in standard_cases() {
        let tr = run_case(&c, DT);
        for (t, text) in &tr.events {
            let key = text.split(':').next().unwrap_or(text).trim();
            assert!(LOG_KEYS.contains(&key), "{}: log line at {t} s has unlisted key '{key}': {text}", c.id);
        }
    }
}

/// Convergence report. Package 2 only reports; Package 3 turns this into an assertion
/// after the convergence study sets the numerical tolerance floors (spec: "set
/// tolerances from uncertainty or convergence studies before tuning").
#[test]
fn convergence_report_half_step() {
    let c = case("fp_steady_600");
    let a = run_case(&c, DT).metrics();
    let b = run_case(&c, DT / 2.0).metrics();
    let v = compare(&a, &b, &Tolerance::numerical());
    println!("fp_steady_600 dt/2 convergence: {} violation(s)", v.len());
    for s in &v {
        println!("  {s}");
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --release -p bwr-core --test plant standard_cases`
Expected: compile error `cannot find function `standard_cases``.

- [ ] **Step 3: Implement `standard_cases`**

At the top of `validation.rs`, after `use crate::params::RATED_POWER_W;`, add:

```rust
use crate::params::PRESSURE_SETPOINT_DEFAULT;
```

Insert before `#[cfg(test)]`:

```rust
fn at(at_s: f64, cmd: Command) -> ScriptedCommand {
    ScriptedCommand { at_s, cmd }
}

fn case(id: &'static str, scenario: ScenarioName, duration_s: f64, commands: Vec<ScriptedCommand>) -> Case {
    Case { id, scenario, duration_s, sample_dt_s: 1.0, commands, faults: vec![], dt_after_s: None, operator: Operator::None }
}

/// The regression case set. Ids are stable; adding a case means adding a row to
/// docs/validation/cases.csv and a baseline file.
pub fn standard_cases() -> Vec<Case> {
    use Command::*;
    let fp = ScenarioName::FullPower;
    vec![
        case("fp_steady_600", fp, 600.0, vec![]),
        case("fp_manual_scram", fp, 300.0, vec![at(5.0, ManualScram), at(5.0, SetModeSwitch { mode: ModeSwitch::Shutdown })]),
        case("fp_turbine_trip", fp, 300.0, vec![at(5.0, TripTurbine)]),
        case("fp_msiv_closure", fp, 600.0, vec![at(5.0, SetMsiv { line: 0, open: false }), at(5.0, SetMsiv { line: 1, open: false })]),
        case("fp_pressure_step", fp, 60.0, vec![at(5.0, SetPressureSetpoint { mpa: PRESSURE_SETPOINT_DEFAULT / 1e6 + 0.1 })]),
        case("fp_loss_fw_heating", fp, 300.0, vec![at(5.0, SetHeaters { in_service: false })]),
        case("fp_load_reject", fp, 300.0, vec![at(5.0, OpenBreaker)]),
        case("fp_fw_pump_trip", fp, 300.0, vec![at(5.0, SetFeedPump { index: 0, on: false })]),
        case("hs_steady_600", ScenarioName::HotStandby, 600.0, vec![]),
        case("cs_steady_600", ScenarioName::ColdShutdown, 600.0, vec![]),
    ]
}
```

- [ ] **Step 4: Run the suite and read the convergence report**

Run: `cargo test --release -p bwr-core -- --nocapture convergence_report_half_step`
Expected: the printed report (any number of violations is acceptable in this package; copy the list into the Task 11 commit message as the measured starting point).

Run: `cargo test --release -p bwr-core`
Expected: unit `44 passed`, integration `15 passed`.

- [ ] **Step 5: Commit**

```bash
git add crates/bwr-core/src/validation.rs crates/bwr-core/tests/plant.rs
git commit -m "feat(core): standard validation cases; determinism and convergence tests

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 10: Validation example, baselines, case register and method

**Files:**
- Create: `crates/bwr-core/examples/validation.rs`
- Create: `docs/validation/method.md`, `docs/validation/cases.csv`
- Create (generated): `docs/validation/baseline/<id>.csv` for the 10 ids

**Interfaces:**
- Consumes: `standard_cases`, `run_case`, `compare`, `Tolerance::numerical`, `Trace`, `Metrics`, `DT`.
- Produces: the `validation` example (`--write`, `--case <id>`; Package 9 adds `--write-missing`, Package 10 `--report`), exit code 1 on any FAIL; baseline files; `docs/validation/cases.csv` with the locked header `id,scenario,duration_s,purpose,reference,tolerance_class,role,status`. Package 10 cites `method.md` in `results.md`.

- [ ] **Step 1: Write the example**

Create `crates/bwr-core/examples/validation.rs`:

```rust
//! Validation runner.
//!
//! ```sh
//! cargo run --release -p bwr-core --example validation              # compare against baselines
//! cargo run --release -p bwr-core --example validation -- --write   # regenerate ALL baselines
//! cargo run --release -p bwr-core --example validation -- --case fp_turbine_trip
//! ```
//!
//! Baseline CSVs under docs/validation/baseline are regression evidence, not
//! physical truth. Never regenerate them to make a failing case pass; record the
//! physical justification in the commit that changes them.

use std::path::PathBuf;

use bwr_core::sim::DT;
use bwr_core::validation::{compare, run_case, standard_cases, Metrics, Tolerance, Trace};

fn baseline_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/validation/baseline")
}

fn print_metrics(m: &Metrics) {
    println!(
        "  {:<20} {:>14} {:>14} {:>9} {:>14} {:>9} {:>12} {:>12}",
        "column", "final", "peak", "t_peak", "min", "t_min", "init_drift/s", "drift/s"
    );
    for (name, f) in &m.final_values {
        let (pv, pt) = m.peak[name];
        let (mv, mt) = m.min[name];
        println!(
            "  {name:<20} {f:>14.6e} {pv:>14.6e} {pt:>9.1} {mv:>14.6e} {mt:>9.1} {:>12.3e} {:>12.3e}",
            m.initial_drift_per_s[name], m.drift_per_s[name]
        );
    }
    println!(
        "  max |mass residual| {:.3e} kg, max |energy residual| {:.3e} J{}",
        m.max_abs_mass_residual_kg,
        m.max_abs_energy_residual_j,
        if m.nonfinite { "  NON-FINITE VALUES PRESENT" } else { "" }
    );
    for (k, v) in &m.integrated {
        println!("  integrated {k:<14} {v:.6e}");
    }
    for (k, t) in &m.first_event_time {
        println!("  event {k} at {t:.2} s");
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let write = args.iter().any(|a| a == "--write");
    let only: Option<String> = args.iter().position(|a| a == "--case").and_then(|i| args.get(i + 1).cloned());
    let dir = baseline_dir();
    std::fs::create_dir_all(&dir).expect("create baseline dir");
    let tol = Tolerance::numerical();
    let mut failed = 0usize;
    let mut ran = 0usize;

    for case in standard_cases() {
        if let Some(o) = &only {
            if o != case.id {
                continue;
            }
        }
        ran += 1;
        let trace = run_case(&case, DT);
        let m = trace.metrics();
        let path = dir.join(format!("{}.csv", case.id));
        if write {
            std::fs::write(&path, trace.to_csv()).expect("write baseline");
            println!("WROTE {} ({} rows, {} events)", path.display(), trace.rows.len(), trace.events.len());
        } else {
            let violations = match std::fs::read_to_string(&path) {
                Ok(s) => match Trace::from_csv(&s) {
                    Ok(reference) if reference.case_id == case.id => compare(&reference.metrics(), &m, &tol),
                    Ok(reference) => vec![format!("baseline case id '{}' does not match '{}'", reference.case_id, case.id)],
                    Err(e) => vec![format!("baseline unreadable: {e}")],
                },
                Err(_) => vec![format!("no baseline at {}; run with --write", path.display())],
            };
            if violations.is_empty() {
                println!("PASS {} (numerical class vs regression baseline)", case.id);
            } else {
                failed += 1;
                println!("FAIL {} (numerical class vs regression baseline)", case.id);
                for s in &violations {
                    println!("     {s}");
                }
            }
        }
        print_metrics(&m);
        let half = run_case(&case, DT / 2.0).metrics();
        let conv = compare(&m, &half, &tol);
        println!("  convergence dt/2: {} violation(s)", conv.len());
        for s in &conv {
            println!("     {s}");
        }
        println!();
    }

    if ran == 0 {
        eprintln!("no case matched {only:?}");
        std::process::exit(2);
    }
    if failed > 0 {
        eprintln!("{failed} case(s) FAILED");
        std::process::exit(1);
    }
}
```

- [ ] **Step 2: Run in compare mode before baselines exist; verify it fails loudly**

Run: `cargo run --release -p bwr-core --example validation; echo "exit=$?"`
Expected: ten `FAIL <id>` lines each followed by `no baseline at ...; run with --write`, then `10 case(s) FAILED` and `exit=1`.

- [ ] **Step 3: Generate the baselines and verify compare passes**

Run: `cargo run --release -p bwr-core --example validation -- --write`
Expected: ten `WROTE .../docs/validation/baseline/<id>.csv (N rows, M events)` lines; `fp_steady_600` has 601 rows, `fp_pressure_step` 61 rows.

Run: `cargo run --release -p bwr-core --example validation; echo "exit=$?"`
Expected: ten `PASS <id>` lines and `exit=0`. Each case also prints its `convergence dt/2` report; violations there are recorded, not fixed, in this package.

Run: `ls docs/validation/baseline | wc -l && head -3 docs/validation/baseline/fp_steady_600.csv`
Expected: `10`, then `# schema,2`, `# case,fp_steady_600`, `# dt_s,5.0000000000000003e-2`.

- [ ] **Step 4: Write the case register**

Create `docs/validation/cases.csv` (fields never contain commas; `role` is `calibration` for the two steady cases used to set tolerances and `validation` otherwise, per the master):

```csv
id,scenario,duration_s,purpose,reference,tolerance_class,role,status
fp_steady_600,fullPower,600,Rated-power steady state: initial equilibrium residual and drift,none (regression),numerical,calibration,baseline
fp_manual_scram,fullPower,300,Manual scram with mode switch to SHUTDOWN: power decay and level shrink without isolation,none (regression),numerical,validation,baseline
fp_turbine_trip,fullPower,300,Turbine trip at rated power: bypass ride-through and Select Rod Insert,none (regression),numerical,validation,baseline
fp_msiv_closure,fullPower,600,Closure of both MSIVs: scram and isolation condenser initiation,none (regression),numerical,validation,baseline
fp_pressure_step,fullPower,60,+0.1 MPa pressure setpoint step: void collapse reactivity response,none (regression),numerical,validation,baseline
fp_loss_fw_heating,fullPower,300,Feedwater heaters removed from service: cold feed reactivity response,none (regression),numerical,validation,baseline
fp_load_reject,fullPower,300,Generator breaker opened at rated load: speed excursion and bypass,none (regression),numerical,validation,baseline
fp_fw_pump_trip,fullPower,300,One of two running feed pumps stopped: level control recovery,none (regression),numerical,validation,baseline
hs_steady_600,hotStandby,600,Hot standby steady state: decay heat balance and level drift,none (regression),numerical,calibration,baseline
cs_steady_600,coldShutdown,600,Cold shutdown steady state: vented vessel and 30-day decay heat,none (regression),numerical,validation,baseline
```

- [ ] **Step 5: Write the method document**

Create `docs/validation/method.md`:

```markdown
# Validation method

This document defines what a validation case measures, how a comparison is
judged, and what a passing comparison does and does not mean. It is the
reference for `crates/bwr-core/src/validation.rs`, the `validation` example,
`docs/validation/cases.csv`, and the baseline traces under
`docs/validation/baseline/`.

## Purpose

The simulator must be reproducible before it is refined. A case is a scenario
plus a time-stamped script of operator commands and fault injections, and
optionally a closed-loop operator (Package 9). Running a case produces a trace
that any reviewer can regenerate bit-for-bit. Metrics reduce the trace to a
small set of numbers that can be compared under an explicit tolerance.
Baselines saved from the current engine are regression evidence: they show that
behaviour did not change unintentionally. They are not physical truth and must
never be regenerated merely to make a failing comparison pass.

## Determinism and runner rules

`run_case(case, dt)` first validates the timing: `dt`, `sample_dt_s`,
`duration_s` and any `dt_after_s.1` must be finite and positive, `sample_dt_s`
and `duration_s` must be integer multiples (within 1e-9 relative) of every step
in force, and `dt_after_s.0` must lie on a sample boundary; a violation panics
with a message naming the case. It then builds the scenario with its
constructor (which ends in `Sim::finalize_init`) and loops over physics
boundaries with a monotonic runner clock `t`. At each boundary, in order:
scripted commands whose `at_s <= t + dt/2` (stable by time, list order for
ties), then scripted faults likewise, then the operator if `OPERATOR_DT_S` has
elapsed (first at `t = 0`), then the engine log is drained into the trace's
events, then a sample row if `sample_dt_s` has elapsed. The log is drained
again after every physics step, so an action at the terminal boundary is
recorded exactly once. The run ends when `t` reaches `duration_s`, and one
final row is always emitted at `t == duration_s` whether or not it is a sample
boundary. A case may switch to a second step size at
`dt_after_s = (from_t_s, dt_s)` for long-duration runs; the trace records the
initial `dt_s`. `LoadScenario`, `SetPaused` and `SetSpeed` are rejected with a
panic naming the case: scripted cases run at a fixed step with no reload.

The same case and `dt` give an identical `Trace` when run twice in the same
build and environment; asserted by `validation_runner_is_deterministic` in
`tests/plant.rs` (a single-process test, so it says nothing about other
platforms). Traces produced on a different platform, toolchain or CPU are
compared under `Tolerance::numerical()`, never for bit equality.

## Trace schema v2

The CSV starts with `# schema,2`, `# case,<id>`, `# dt_s,<v>`, one
`# integrated,<key>,<value>` line per ledger integral, one
`# extremum,<col>,<max>,<t_max>,<min>,<t_min>` line per column (every column
except `t`), then the header, rows in `{:.16e}` (exact f64 round trip), then
`# event,<t>,<text>` lines. A file with no rows, without the schema, case or
dt lines, with a non-finite `dt_s`, or without integrated or extremum lines is
rejected by `from_csv`, and `compare` reports a violation for metrics missing
any of these, so a header-only or truncated baseline can never PASS.

Columns marked "from P*n*" are emitted as 0.0 until that package populates them.

| Column | Unit | Source |
|---|---|---|
| `t` | s | simulation time |
| `power_frac` / `power_frac_max_int` | - | neutron power relative to rated; interval maximum |
| `thermal_mw` | MW | total core thermal power incl. decay heat |
| `decay_mw` | MW | decay heat |
| `pressure_pa` / `pressure_pa_max_int` | Pa | dome pressure; interval maximum |
| `pressure_sensed_pa` | Pa | pressure sensor reading (from P8) |
| `level_z_m` | m | mixture level above vessel bottom |
| `collapsed_level_m` | m | collapsed level (from P3) |
| `level_nr_cm` / `_min_int` / `_max_int` | cm | narrow-range level vs NWL; interval min and max |
| `level_model_cm` | cm | model level when `level_nr_cm` becomes the sensed value (from P8) |
| `mass_kg`, `energy_j` | kg, J | vessel inventory and fluid+metal energy |
| `gas_mass_kg` | kg | noncondensable gas in the head (from P4) |
| `core_flow_kgs`, `steam_flow_kgs`, `feed_flow_kgs` | kg/s | core flow; main steam plus head vent; feedwater |
| `t_feed_c`, `t_coolant_c`, `fuel_t_c` | degC | feed, bulk coolant, lumped fuel temperatures |
| `clad_t_c` | degC | cladding (from P5) |
| `moderator_t_c` | degC | moderator temperature (from P4) |
| `rods_avg_pos`, `rods_min_pos`, `rods_max_pos` | notch units 0..48 | rod positions |
| `msiv_pos_0`, `msiv_pos_1` | - | per-line MSIV position |
| `tsv_pos` | - | turbine stop valve (from P6) |
| `tcv_pos`, `bypass_pos` | - | control and bypass valve positions |
| `header_pressure_pa` | Pa | steam header pressure (from P6) |
| `turbine_rpm`, `gross_we` | rpm, W | shaft speed, gross electrical output |
| `scrammed`, `turbine_tripped`, `breaker_closed` | 0/1 | latched states |
| `mode` | 0..3 | mode switch: 0 shutdown, 1 refuel, 2 startup, 3 run |
| `ic_heat_w_i` | W | heat removal per IC train |
| `ic_pool_kg_i` | kg | pool inventory per train |
| `ic_return_pos_i` | - | condensate return valve position (max of the pair from P7) |
| `cond_pressure_pa`, `hotwell_kg` | Pa, kg | condenser |
| `cst_kg` | kg | condensate storage inventory (from P6) |
| `fw_pump_i` | 0/1 | feed pump running |
| `mass_residual_kg`, `energy_residual_j` | kg, J | ledger residuals |
| `unsupported` | 0/1 | model-envelope flag (from P3) |

Interval extrema (`*_max_int`, `*_min_int`) are accumulated at every physics
step since the previous sample row, so sampled peaks cannot miss sub-sample
excursions. The `# extremum` lines hold, for every column, the maximum and
minimum over the whole run with the time of first occurrence, accumulated at
every physics step (seeded from the initial state); they are independent of
`sample_dt_s`, so a peak time is the same at 0.05 s and 5 s sampling.

Events are `(t, "<key>: <detail>")` where `<key>` is one of the locked keys in
`validation::LOG_KEYS`: `scram`, `turbine trip`, `msiv isolation`,
`ic auto-initiation`, `feed pump trip`, `domain error`, `rod block`, `sri`,
`ari`, `breaker open`, `command ignored`, `overload`, `ic model`, `fault`.
This package produces the first five and `fault`; the rest are reserved for
later packages. A conformance test in `tests/plant.rs` asserts every log line
from the standard cases starts with a listed key.

Integrated transfers (`# integrated` lines, keys from `ledger::INTEGRATED_KEYS`):
`feed_kg`, `steam_kg`, `ic_steam_kg`, `ic_return_kg`, `rwcu_kg`, `vent_kg`,
`core_j`, `ic_j`, `loss_j`.

## Residuals

The ledger integrates the flows the vessel actually applied. A closed model has
a residual that is zero to floating-point error. A non-physical clamp shows up
as `mass_clipped`/`energy_clipped` in the ledger and as a nonzero residual. At
the Package 2 baseline the only such term is the 1000 kg inventory floor, which
no standard case reaches; Package 3 removes it. From Package 7 the IC heat term
`q_ic` is derived from the IC mass ports and is never counted twice.

## Metrics

For every column except `t` and the interval columns (which fold into their
parent's extremum):

- **final value** at the last sample;
- **peak** and **minimum** with the time of first occurrence, taken from the
  trace's `# extremum` lines (physics-step resolution), never from the sampled
  rows;
- **initial drift**: max |dx/dt| between consecutive samples over the first
  60 s (the initial-equilibrium residual of a steady case);
- **drift**: |x(t_end) - x(t_end - 60 s)| / 60 s, or over the whole trace if
  shorter.

For the whole trace: **max |mass residual|** and **max |energy residual|**;
**first event time** per key (actuation order and delay); **integrated
transfers** from the ledger; **nonfinite** flag, set if any row value,
integrated value, extremum value or time, event time or the `dt_s` metadata is
NaN or infinite.

## Tolerance classes

| Class | Use | `rel` | event | extremum time | residuals | abs floors |
|---|---|---|---|---|---|---|
| numerical | same physics, different step/platform/refactor | 1e-3 | 0.1 s | 1 s | 1e-3 kg, 1e3 J | table x1 |
| physical | an intended model change that should preserve behaviour | 5e-2 | 2 s | 10 s | 1e-2 kg, 1e4 J | table x10 |
| source-comparison | comparison with a public reference trace | per case | per case | per case | physical | physical |

Absolute floors (numerical class): power fraction 1e-4; thermal power 0.1 MW;
decay heat 0.01 MW; pressures 100 Pa; levels 1e-3 m or 0.1 cm; inventories
1 kg; energy 1e6 J; gas mass 0.01 kg; flows 0.1 kg/s; temperatures 0.01 degC;
rod positions 0.01; valve positions 1e-3; turbine speed 0.1 rpm; gross output
10 kW; IC heat 1 kW; condenser pressure 10 Pa; booleans, mode, pump states and
the `unsupported` flag exact.

The source-comparison class is written in `cases.csv` as
`source(rel=<r>;event=<s>;extremum=<s>)`; `Tolerance::source_comparison(row)`
resolves the `tolerance_class` column by its name in the locked header and
rejects (panics on) any row that does not have exactly 8 fields, so a stray
comma in a free-text field is an error, never a silent column shift.

These floors are starting values. Package 3's convergence study (0.05, 0.025
and 0.0125 s steps) replaces the numerical-class values with 2x the measured
dt-halving differences, recorded here with the study results. Tolerances are
set before any tuning, never after.

## Comparison rules

`compare(reference, candidate, tol)` reports one violation per:

- a reference or candidate with no rows, no integrated transfers or no extrema
  (an empty or truncated trace never passes by having nothing to compare);
- any non-finite value in the reference or the candidate (including event
  times and metadata), or any non-finite tolerance input;
- final, peak or minimum value outside `max(abs, rel * max(|a|, |b|))`;
- extremum time differing by more than `extremum_time_s`, checked only when the
  extremum is distinct from the final value (a flat signal has an arbitrary
  argmax);
- max |residual| above the class's absolute residual tolerance (residuals are
  never judged against the reference);
- integrated transfer outside `rel` with the mass or energy floor;
- a reference event missing from the candidate, or more than `event_time_s`
  away; a candidate event absent from the reference.

Drift and initial drift are reported, not compared. A comparison is reported
as PASS only with its tolerance class and reference named. "PASS against
regression baseline" is not a statement about the plant.

## Case register

`docs/validation/cases.csv` has the locked header
`id,scenario,duration_s,purpose,reference,tolerance_class,role,status`. `role`
is `calibration` for cases used to set tolerances or tune, `validation` for
cases reserved to check, and `plausibility` (Package 9) for cases without a
quantitative reference. Fields never contain commas; every row has exactly 8
fields and readers resolve columns by header name.

## Convergence

The `validation` example also runs every case at `dt/2` and reports the
numerical-class violations. Package 2 only reports these; Package 3 requires
converged event timing and extrema for all supported cases.
```

- [ ] **Step 6: Commit**

```bash
git add crates/bwr-core/examples/validation.rs docs/validation/method.md docs/validation/cases.csv docs/validation/baseline
git commit -m "feat(validation): baseline runner, case register, method and regression traces

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 11: CI, package script and README

**Files:**
- Create: `.github/workflows/validate.yml`
- Modify: `package.json` (root), `README.md`

**Interfaces:**
- Consumes: the `validation` example.
- Produces: `npm run validate`; a CI job that fails on any baseline or test regression. `.github/workflows/ci.yml` is unchanged; it already builds the desktop bundles on Windows, macOS and Linux, which is the platform-appropriate desktop verification for this package.

- [ ] **Step 1: Add the workflow**

Create `.github/workflows/validate.yml`:

```yaml
name: Validate

on:
  push:
    branches: [main]
  pull_request:

jobs:
  engine:
    name: Engine tests and validation cases
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo test --release -p bwr-core
      - name: Validation cases against baselines
        run: cargo run --release -p bwr-core --example validation | tee validation-report.txt
      - uses: actions/upload-artifact@v4
        if: always()
        with:
          name: validation-report
          path: validation-report.txt

  ui:
    name: UI typecheck and build
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 22
      - run: npm ci --prefix ui
      - run: npm --prefix ui run typecheck
      - run: npm --prefix ui run build
```

(GitHub's default `bash` shell runs with `-eo pipefail`, so a non-zero exit from the example fails the step even through `tee`.)

- [ ] **Step 2: Add the npm script**

In the root `package.json`, change

```json
    "test": "cargo test --release -p bwr-core"
```

to

```json
    "test": "cargo test --release -p bwr-core",
    "validate": "cargo run --release -p bwr-core --example validation"
```

- [ ] **Step 3: Add the README section**

In `README.md`, after the "## Building" section's closing paragraph (the line starting with `` `npm run ui:dev` runs just the panels``), insert:

```markdown
## Validation

`npm run validate` runs the scripted cases in `crates/bwr-core/src/validation.rs` and compares their metrics with the
regression baselines in `docs/validation/baseline/`. Each case is listed in `docs/validation/cases.csv`;
`docs/validation/method.md` defines the trace schema, metrics and tolerance classes. The runner also reports how each
case changes when the physics step is halved.

```sh
npm run validate                                                       # compare against baselines (exit 1 on any FAIL)
cargo run --release -p bwr-core --example validation -- --case fp_turbine_trip
cargo run --release -p bwr-core --example validation -- --write        # regenerate baselines
```

Baseline traces are regression evidence, not physical truth. Regenerate them only with a physical justification in the
commit message, never to make a failing comparison pass. CI runs the engine tests and the validation cases on every push
and pull request (`.github/workflows/validate.yml`).
```

- [ ] **Step 4: Verify everything end to end**

Run: `npm run validate; echo "exit=$?"`
Expected: ten `PASS` lines, `exit=0`.

Run: `cargo test --release -p bwr-core`
Expected: unit `44 passed`, integration `15 passed` (plus Package 1's `register` target when it has landed).

Run: `npm --prefix ui run typecheck && npm --prefix ui run build`
Expected: both succeed (no UI contract changed in this package).

- [ ] **Step 5: Commit, including the measured convergence report**

```bash
git add .github/workflows/validate.yml package.json README.md
git commit -m "ci: validation workflow, npm validate script, README validation section

Measured dt/2 convergence at baseline (fp_steady_600):
<paste the violation list printed by convergence_report_half_step here, or 'none'>

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

- [ ] **Step 5a: P1/P2 reconciliation gate**

Precondition: Package 1 has landed on this branch (`docs/model/parameters.csv` carries Package 1's rows and `crates/bwr-core/tests/register.rs` exists). If it has not, stop here, rebase this branch on Package 1 (keeping the single locked header line and all rows from both packages), and return to this step; this package is not complete until the gate passes.

Run: `cargo test --release -p bwr-core --test register`
Expected: `test result: ok. 4 passed` (`params_rs_scan_finds_the_constants`, `header_contains_every_locked_column`, `every_params_const_is_registered`, `every_row_has_valid_shape_and_status`). The rows this package added (`N_IC_RETURN_VALVES`, `validation.OPERATOR_DT_S`, `validation.TOL_*`) have 13 comma-free fields, an allowed status, an `A-` assumption id and unique ids.

Run: `grep -c "^validation\.TOL_\|^validation\.OPERATOR_DT_S\|^N_IC_RETURN_VALVES," docs/model/parameters.csv`
Expected: `30` (28 tolerance rows, `OPERATOR_DT_S`, `N_IC_RETURN_VALVES`).

If any register test fails, fix the row (never the test) and amend the commit of the task that added it.

---

## Acceptance gate (verbatim from the spec)

"Repeat runs give reproducible traces. Each claimed comparison identifies its reference and tolerance. No previous-test failure is concealed by updating expected output."

Checked by: `validation_runner_is_deterministic` (reproducible traces); the `reference` and `tolerance_class` columns of `docs/validation/cases.csv` and the PASS/FAIL lines naming the class and reference (each comparison identifies its reference and tolerance); the Task 1 baseline run recorded before any change, plus the rule in `method.md` and the README that baselines are never regenerated to hide a failure.

## Self-review against the spec's Package 2 bullets

| Spec bullet | Task |
|---|---|
| Establish Rust toolchain access and run the existing tests before changing physics | Task 1 |
| Deterministic validation runner with simulation time, scripted commands, machine-readable traces incl. pressure, inventory, energy, power, level, flow, temperatures, valve states, rod positions, protection actions, numerical residuals | Tasks 3, 4, 5, 7 (58 columns + events + integrated) |
| Save baseline results as regression evidence, not physical truth | Task 10 (files, `method.md`, README) |
| Comparison metrics: initial equilibrium residuals, peaks and times, minimum level, integrated transfers, actuation order and delay, final state, drift | Task 8 (`Metrics`), `method.md` |
| Separate numerical, physical, source-comparison tolerances; set from convergence before tuning | Task 8 (`Tolerance::numerical/physical/source_comparison`), Task 9 convergence report, `method.md` |
| Automate engine tests, UI checks, platform-appropriate desktop verification | Task 11 (`validate.yml`; `ci.yml` desktop matrix unchanged) |
| Deterministic test-level fault overrides (needed by later packages) | Task 6 |
| Scenario bookkeeping epilogue `finalize_init` (master) | Task 4 |
| Shared test helpers `tests/common/mod.rs` (master) | Task 2 |

## Master self-review checklist (run on this plan)

- [x] Every spec bullet maps to a named task (table above).
- [x] No placeholder text: grep for `TBD`, `TODO`, `similar to Task`, `add validation` returns nothing; every code step shows the code.
- [x] Names match the master r2 contract: `BoundaryFlows` (15 fields), `Ledger.integrated`, `Fault::{kind,equipment,is_valid}`, `Sim::{faults: Vec<Fault>, set_fault, has_fault, find_fault, log, ledger, finalize_init}`, `Case::{dt_after_s, operator}`, `Operator`, `OPERATOR_DT_S`, `TRACE_SCHEMA_VERSION`, `col_index`, `Trace::from_csv(s)`, `run_case`, `run_case_with`, `run_sim_with`, `Metrics::{initial_drift_per_s, integrated, nonfinite}`, `Tolerance::{extremum_time_s, residual_mass_kg, residual_energy_j, source_comparison}`, `Sim::last_flows`, `LOG_KEYS` (locked log-line keys; all push sites use them and Task 9 enforces conformance). Additions beyond the master, reported to the coordinator: `Trace.integrated`, `ledger::INTEGRATED_KEYS`, `ic::N_IC_RETURN_VALVES` defined here rather than in P7, private `Sim::prev_turbine_tripped`. `N_TRACE_COLS = 58`, `Trace.extrema` and `Extremum` are the master's own declarations.
- [x] Derives rule: `BoundaryFlows` (Clone, Copy, Debug, Default, PartialEq), `Ledger` (Clone, Debug, Default, PartialEq), `Fault` (Clone, Debug, PartialEq, Eq, Hash), `Operator` (Clone, Copy, Debug, PartialEq, Eq), `Case`/`ScriptedCommand`/`ScriptedFault` (Clone, Debug), `Trace`/`Metrics` (Clone, Debug, PartialEq), `Extremum` (Clone, Copy, Debug, PartialEq), `Tolerance` (Clone, Debug).
- [x] New constants registered in `docs/model/parameters.csv` in the task that introduces them: `N_IC_RETURN_VALVES = 2` (Task 6 Step 1a; status `assumed`, A-013; Package 7 updates the row), `OPERATOR_DT_S = 1.0` (Task 7 Step 3a; runner setting, A-016) and every `Tolerance::numerical()`/`physical()`/`abs_floor` number (Task 8 Step 3a, 28 rows). Package 1 owns the register and this plan runs in parallel with it, so each step creates the file with the locked header if needed and Task 11 Step 5a (P1/P2 reconciliation gate) runs `tests/register.rs` and must pass before the package is complete.
- [x] No `Snapshot` change in this package; `types.ts`/`mock.ts` untouched; UI typecheck stays green.
- [x] No defect fix in this package (measurement only); every behaviour change has a failing test first.
- [x] Transfer-allocation rule not yet applicable (no consumer changes); `BoundaryFlows` carries the applied values so Package 3 can enforce it.
