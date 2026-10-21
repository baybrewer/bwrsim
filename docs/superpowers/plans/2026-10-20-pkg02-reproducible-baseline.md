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
- Known master discrepancy, flagged to the coordinator: the master's `TRACE_COLUMNS` literal lists 58 names but declares `N_TRACE_COLS = 59`. This plan uses the master's exact 58-name list and `N_TRACE_COLS = 58`; the Rust compiler rejects any other pairing. If the master adds a 59th column, append it to the end of the list here and bump the constant.

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
- Produces: `Vessel::step(&mut self, dt: f64, inp: &VesselInputs) -> BoundaryFlows`; `Sim::ledger: Ledger`; `Sim::finalize_init(&mut self)` called at the end of `cold_shutdown`, `hot_standby`, `full_power`. At this package it sets `time = 0`, re-bases the ledger, clears `events`, `acked`, `accumulator` (Task 5 adds `log.clear()`; P3 adds `domain_errors` re-stamping; P8 sensors; P10 extrema). Package 3 relies on `mass_clipped` being exactly what the clamp added.

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
use crate::ledger::Ledger;
```

In the `Sim` struct, after `accumulator: f64,` add:

```rust
    /// Mass/energy boundary accounting for the vessel inventory (see ledger.rs).
    pub ledger: Ledger,
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

and in the struct literal after `accumulator: 0.0,` add `ledger,`.

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
        self.ledger.record(dt, &flows);
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
- Produces: `Sim::log: Vec<(f64, String)>` (public). Entry text format is `"<key>: <detail>"`; `Metrics::first_event_time` keys on the text before the first `:`. Keys written here: `scram`, `turbine trip`, `msiv isolation`, `ic auto-initiation`, `feed pump trip`, `fault ignored` (Task 6). Package 3 adds `domain error`; the master's non-finite command rule (`Command ignored: non-finite value`) is implemented by Package 3 alongside `DomainError`. `run_sim_with` drains the log each step. `finalize_init` clears it.

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
        let ignored = sim.log.iter().filter(|(_, e)| e.starts_with("fault ignored")).count();
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
            self.log.push((self.time, format!("fault ignored: invalid payload {f:?}")));
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
git add crates/bwr-core/src/faults.rs crates/bwr-core/src/ic.rs crates/bwr-core/src/lib.rs crates/bwr-core/src/sim.rs
git commit -m "feat(core): ordered engine-level fault overrides with payload validation

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---
