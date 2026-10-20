# Package 2: Reproducible and Measurable Baseline — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Names and signatures come from `docs/superpowers/plans/2026-10-20-refinement-master.md` ("Shared interface contract"); if anything here disagrees with the master, the master wins.

**Goal:** Make the baseline engine behaviour reproducible and measurable: deterministic scripted cases, machine-readable traces with mass/energy residuals, a fault-override hook, an append-only event log, saved regression baselines, and CI that runs all of it.

**Architecture:** Three new engine modules. `ledger.rs` accounts every mass and energy flow the vessel applies and reports residuals. `faults.rs` is a set of engine-level overrides that automatic logic may never clear (honoured by later packages). `validation.rs` runs a `Case` (scenario + scripted commands + scripted faults) to a `Trace`, reduces it to `Metrics`, and compares metrics under a `Tolerance`. An example binary writes or checks baseline CSVs under `docs/validation/baseline/`. Nothing in this package changes physics; it only measures it.

**Tech Stack:** Rust 2021 (cargo 1.99.1), `serde_json` (existing dev-dependency), hand-rolled CSV, GitHub Actions. No new dependencies.

**Spec:** Package 2 of `docs/superpowers/plans/2026-10-20-existing-systems-refinement.md`.

## Global Constraints

- "No previous-test failure is concealed by updating expected output."
- "Save baseline results as regression evidence, explicitly not as physical truth."
- "Separate numerical, physical, and source-comparison tolerances. Set tolerances from uncertainty or convergence studies before tuning."
- Commands: `cargo test --release -p bwr-core`, `npm --prefix ui ci`, `npm --prefix ui run typecheck`, `npm --prefix ui run build`, and after Task 9 `cargo run --release -p bwr-core --example validation`.
- No new Rust or npm dependencies.
- No physics changes in this package. The vessel's `.max(1000.0)` clamp stays; it is measured here and removed in Package 3.
- Commit after every task; commit messages end with `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.
- Match the surrounding Rust style by hand (4-space indent, `rustfmt` defaults); 2-space indent in YAML/JSON/Markdown.

---

## File structure

| Path | Action | Responsibility |
|---|---|---|
| `crates/bwr-core/src/ledger.rs` | create | `BoundaryFlows`, `Ledger`: per-step boundary accounting and residuals |
| `crates/bwr-core/src/faults.rs` | create | `Fault` enum and its unit test |
| `crates/bwr-core/src/validation.rs` | create | `Case`, `Trace`, `Metrics`, `Tolerance`, `compare`, `run_case`, `standard_cases` |
| `crates/bwr-core/src/lib.rs` | modify | register the three modules |
| `crates/bwr-core/src/vessel.rs` | modify | `Vessel::step` returns `BoundaryFlows`, measures the mass clamp |
| `crates/bwr-core/src/sim.rs` | modify | `ledger`, `faults`, `log`, `prev_turbine_tripped` fields; `set_fault`, `has_fault`; log pushes |
| `crates/bwr-core/tests/plant.rs` | modify | ledger closure, log timing, determinism, convergence report tests |
| `crates/bwr-core/examples/validation.rs` | create | baseline writer/checker with convergence report |
| `docs/validation/method.md` | create | metric and tolerance definitions |
| `docs/validation/cases.csv` | create | case register |
| `docs/validation/baseline/*.csv` | create (generated) | regression traces |
| `.github/workflows/validate.yml` | create | validation CI job |
| `package.json` (root) | modify | `validate` script |
| `README.md` | modify | "Validation" section |

---

### Task 1: Toolchain check and baseline run

**Files:**
- None modified. This task records the measured starting point.

**Interfaces:**
- Consumes: nothing.
- Produces: confirmation that the 19 baseline tests and UI checks pass before any change.

- [ ] **Step 1: Confirm the Rust toolchain**

Run: `cargo --version && rustc --version`
Expected: `cargo 1.99.1 (...)` and a matching `rustc 1.99.x` line. If `cargo` is missing, stop and install via `https://rustup.rs` before continuing (the previous review could not run the tests for this reason).

- [ ] **Step 2: Run the engine tests at the baseline**

Run: `cargo test --release -p bwr-core`
Expected (counts must match exactly; this is the baseline evidence):

```
running 11 tests
...
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
...
running 8 tests
...
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

If any test fails at this step, do not modify expected values. Record the failure text in the commit message of Task 10 and continue; Package 3 owns physics fixes.

- [ ] **Step 3: Run the UI checks**

Run: `npm --prefix ui ci && npm --prefix ui run typecheck && npm --prefix ui run build`
Expected: `tsc --noEmit` prints nothing; `vite build` ends with `✓ built in ...`.

- [ ] **Step 4: Confirm the working tree is clean apart from plans**

Run: `git status --short`
Expected: only `?? docs/superpowers/` (or nothing). No commit for this task.

---

### Task 2: Ledger module

**Files:**
- Create: `crates/bwr-core/src/ledger.rs`
- Modify: `crates/bwr-core/src/lib.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `BoundaryFlows` (Copy struct, 12 fields) and `Ledger` with `new(mass0_kg, energy0_j)`, `record(dt, &BoundaryFlows)`, `mass_residual(mass_now) -> f64`, `energy_residual(energy_now) -> f64`. Consumed by Task 3 (`Vessel::step`, `Sim`), Task 6 (`run_case`), and Package 3 (residual tests).

- [ ] **Step 1: Write the failing unit tests inside the new module file**

Create `crates/bwr-core/src/ledger.rs` with the tests only:

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
            w_vent: 0.0,
            w_liquid_out: 0.0,
            h_liquid_out: 0.0,
            q_core: 10.0 * (2.0e6 - 1.0e5),
            q_ic: 0.0,
            q_loss: 0.0,
            mass_clipped: 0.0,
            energy_clipped: 0.0,
        };
        for _ in 0..100 {
            l.record(0.05, &f);
        }
        assert!((l.mass_residual(1000.0)).abs() < 1e-9, "mass residual {}", l.mass_residual(1000.0));
        assert!((l.energy_residual(5.0e6)).abs() < 1e-3, "energy residual {}", l.energy_residual(5.0e6));
        assert!((l.mass_in_kg - 50.0).abs() < 1e-9);
        assert!((l.mass_out_kg - 50.0).abs() < 1e-9);
    }

    #[test]
    fn residual_tracks_inventory_change() {
        let mut l = Ledger::new(1000.0, 0.0);
        let f = BoundaryFlows { w_feed: 2.0, ..Default::default() };
        l.record(1.0, &f);
        // Inventory should now be 1002 kg; if the model holds 1000 the residual is -2.
        assert!((l.mass_residual(1002.0)).abs() < 1e-12);
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
    fn all_outflows_and_sinks_are_counted() {
        let mut l = Ledger::new(0.0, 0.0);
        let f = BoundaryFlows {
            w_steam: 1.0,
            h_steam: 10.0,
            w_vent: 2.0,
            w_liquid_out: 3.0,
            h_liquid_out: 5.0,
            q_ic: 100.0,
            q_loss: 50.0,
            ..Default::default()
        };
        l.record(1.0, &f);
        assert!((l.mass_out_kg - 6.0).abs() < 1e-12);
        // steam+vent at h_steam, liquid at h_liquid_out, plus heat sinks
        assert!((l.energy_out_j - (3.0 * 10.0 + 3.0 * 5.0 + 150.0)).abs() < 1e-9);
    }
}
```

- [ ] **Step 2: Register the module and run the tests to verify they fail**

Edit `crates/bwr-core/src/lib.rs`: after `pub mod bop;` insert `pub mod faults;` is NOT yet needed; for this task insert only:

```rust
pub mod ledger;
```

so the module list reads `api, bop, ic, kinetics, ledger, params, reactor, sim, steam, steam_table, vessel`.

Run: `cargo test --release -p bwr-core ledger`
Expected: compile error `cannot find type `Ledger` in this scope` (and `BoundaryFlows`).

- [ ] **Step 3: Implement the module**

Insert above the `#[cfg(test)]` block in `crates/bwr-core/src/ledger.rs`:

```rust
/// What the vessel actually applied in one step, after any bounding.
/// Rates are per second; `mass_clipped`/`energy_clipped` are amounts for the step.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BoundaryFlows {
    /// kg/s into the vessel.
    pub w_feed: f64,
    /// J/kg.
    pub h_feed: f64,
    /// kg/s out through the main steam lines (after bounding).
    pub w_steam: f64,
    /// J/kg.
    pub h_steam: f64,
    /// kg/s out through the head vent.
    pub w_vent: f64,
    /// kg/s RWCU reject (after bounding).
    pub w_liquid_out: f64,
    /// J/kg.
    pub h_liquid_out: f64,
    /// W into the coolant from the core.
    pub q_core: f64,
    /// W removed by the isolation condensers.
    pub q_ic: f64,
    /// W insulation loss.
    pub q_loss: f64,
    /// kg created (positive) or destroyed (negative) by a clamp this step.
    pub mass_clipped: f64,
    /// J created (positive) or destroyed (negative) by a clamp this step.
    pub energy_clipped: f64,
}

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
}

impl Ledger {
    pub fn new(mass0_kg: f64, energy0_j: f64) -> Self {
        Self { mass0_kg, energy0_j, ..Default::default() }
    }

    pub fn record(&mut self, dt: f64, f: &BoundaryFlows) {
        self.mass_in_kg += f.w_feed * dt;
        self.mass_out_kg += (f.w_steam + f.w_vent + f.w_liquid_out) * dt;
        self.energy_in_j += (f.q_core + f.w_feed * f.h_feed) * dt;
        self.energy_out_j +=
            (f.q_ic + f.q_loss + (f.w_steam + f.w_vent) * f.h_steam + f.w_liquid_out * f.h_liquid_out) * dt;
        self.mass_clipped_kg += f.mass_clipped;
        self.energy_clipped_j += f.energy_clipped;
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

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --release -p bwr-core ledger`
Expected: `test result: ok. 4 passed`.

- [ ] **Step 5: Commit**

```bash
git add crates/bwr-core/src/ledger.rs crates/bwr-core/src/lib.rs
git commit -m "feat(core): add boundary-flow ledger for mass/energy residuals

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 3: Vessel reports boundary flows; Sim records the ledger

**Files:**
- Modify: `crates/bwr-core/src/vessel.rs` (`Vessel::step`, lines around `let w_steam = if self.eos.subcooled ...`)
- Modify: `crates/bwr-core/src/sim.rs` (`Sim` struct, `base`, `full_power`, `step`)
- Modify: `crates/bwr-core/tests/plant.rs` (append one test)

**Interfaces:**
- Consumes: `Ledger`, `BoundaryFlows` from Task 2.
- Produces: `Vessel::step(&mut self, dt: f64, inp: &VesselInputs) -> BoundaryFlows`; `Sim::ledger: Ledger` (public), reset to the post-initialisation state for every scenario. Package 3 relies on `mass_clipped` being exactly the amount the clamp added.

- [ ] **Step 1: Write the failing integration test**

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
    // The ledger must have been reset after scenario initialisation.
    assert!((sim.ledger.mass0_kg - (sim.vessel.mass - sim.ledger.mass_in_kg + sim.ledger.mass_out_kg + rm)).abs() < 1e-6);
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test --release -p bwr-core --test plant ledger_closes`
Expected: compile error `no field `ledger` on type `Sim``.

- [ ] **Step 3: Make `Vessel::step` return `BoundaryFlows`**

In `crates/bwr-core/src/vessel.rs`, change the import line

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

Change the signature `pub fn step(&mut self, dt: f64, inp: &VesselInputs) {` to `pub fn step(&mut self, dt: f64, inp: &VesselInputs) -> BoundaryFlows {`.

Replace this block inside `step`:

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
            q_core: inp.q_core,
            q_ic: inp.q_ic,
            q_loss: heat_loss,
            mass_clipped,
            energy_clipped: 0.0,
        };
```

and change the last two lines of `step`

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

- [ ] **Step 4: Add the ledger to `Sim`**

In `crates/bwr-core/src/sim.rs`, add the import after `use crate::ic::IcTrain;`:

```rust
use crate::ledger::Ledger;
```

In the `Sim` struct, after `accumulator: f64,` add:

```rust
    /// Mass/energy boundary accounting for the vessel inventory (see ledger.rs).
    pub ledger: Ledger,
```

In `fn base(...)`, change the opening

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

In `fn full_power()`, change

```rust
        s.settle(120.0);
        s.time = 0.0;
```

to

```rust
        s.settle(120.0);
        s.ledger = Ledger::new(s.vessel.mass, s.vessel.energy);
        s.time = 0.0;
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

- [ ] **Step 5: Run the full test suite**

Run: `cargo test --release -p bwr-core`
Expected: unit tests `15 passed` (11 + 4 ledger), integration `9 passed` (8 + `ledger_closes_mass_and_energy_at_full_power`). All previous tests unchanged and green.

- [ ] **Step 6: Commit**

```bash
git add crates/bwr-core/src/vessel.rs crates/bwr-core/src/sim.rs crates/bwr-core/tests/plant.rs
git commit -m "feat(core): vessel reports boundary flows; Sim keeps a residual ledger

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 4: Fault overrides

**Files:**
- Create: `crates/bwr-core/src/faults.rs`
- Modify: `crates/bwr-core/src/lib.rs`, `crates/bwr-core/src/sim.rs`

**Interfaces:**
- Consumes: `Sim`.
- Produces: `Fault` enum (exactly the master's 16 variants), `Sim::faults: HashSet<Fault>`, `Sim::set_fault(&mut self, f: Fault, active: bool)`, `Sim::has_fault(&self, f: &Fault) -> bool`. Honouring is done in Packages 5, 6, 7, 9. Faults are not `Command`s and never appear in `Snapshot`. `LoadScenario` builds a fresh `Sim`, so faults are cleared by scenario reload only.

- [ ] **Step 1: Write the failing unit test in the new module**

Create `crates/bwr-core/src/faults.rs`:

```rust
//! Test-level fault overrides for existing equipment.
//!
//! A fault is an engine-level flag that forces a component into a failed
//! state so scenario tests can inject stuck valves or unavailable trains
//! deterministically. Automatic plant logic must never clear a fault; only
//! `Sim::set_fault(.., false)` or a scenario reload does. Faults are not
//! operator commands and are not part of the UI contract.

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
        assert_eq!(sim.faults.len(), 1);
    }

    #[test]
    fn setting_the_same_fault_twice_is_idempotent() {
        let mut sim = Sim::new(ScenarioName::ColdShutdown);
        sim.set_fault(Fault::BypassUnavailable, true);
        sim.set_fault(Fault::BypassUnavailable, true);
        assert_eq!(sim.faults.len(), 1);
    }
}
```

- [ ] **Step 2: Register and run to verify failure**

Edit `crates/bwr-core/src/lib.rs`: insert `pub mod faults;` between `pub mod bop;` and `pub mod ic;`.

Run: `cargo test --release -p bwr-core faults`
Expected: compile error `cannot find type `Fault``.

- [ ] **Step 3: Implement the enum and the `Sim` hooks**

Insert above the `#[cfg(test)]` block in `faults.rs`:

```rust
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
```

In `crates/bwr-core/src/sim.rs` add the import after `use crate::bop::...;`:

```rust
use crate::faults::Fault;
```

In the `Sim` struct after `pub ledger: Ledger,` add:

```rust
    /// Active test-level fault overrides (see faults.rs). Never cleared by automatic logic.
    pub faults: HashSet<Fault>,
```

In `base`, in the struct literal after `ledger,` add `faults: HashSet::new(),`.

Add these methods to `impl Sim`, directly after `pub fn msivs_open(&self) -> bool { ... }`:

```rust
    /// Activate or clear a test-level fault override.
    pub fn set_fault(&mut self, f: Fault, active: bool) {
        if active {
            self.faults.insert(f);
        } else {
            self.faults.remove(&f);
        }
    }

    pub fn has_fault(&self, f: &Fault) -> bool {
        self.faults.contains(f)
    }
```

- [ ] **Step 4: Run tests**

Run: `cargo test --release -p bwr-core`
Expected: unit `17 passed`, integration `9 passed`.

- [ ] **Step 5: Commit**

```bash
git add crates/bwr-core/src/faults.rs crates/bwr-core/src/lib.rs crates/bwr-core/src/sim.rs
git commit -m "feat(core): add engine-level fault overrides (not yet honoured)

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 5: Append-only event log on `Sim`

**Files:**
- Modify: `crates/bwr-core/src/sim.rs` (struct, `base`, `full_power`, `scram`, `step`, `protection`)
- Modify: `crates/bwr-core/tests/plant.rs` (append one test)

**Interfaces:**
- Consumes: nothing new.
- Produces: `Sim::log: Vec<(f64, String)>` (public). Entry text format is `"<key>: <detail>"`; `Metrics::first_event_time` keys on the text before the first `:`. Keys written here: `scram`, `turbine trip`, `msiv isolation`, `ic auto-initiation`, `feed pump trip`. Package 3 adds `domain error`. `run_case` drains the log each step.

- [ ] **Step 1: Write the failing test**

Append to `crates/bwr-core/tests/plant.rs`:

```rust
#[test]
fn log_records_scram_and_turbine_trip_with_times() {
    let mut sim = Sim::new(ScenarioName::FullPower);
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
    // The post-initialisation settle must not leave entries behind.
    let fresh = Sim::new(ScenarioName::FullPower);
    assert!(fresh.log.is_empty());
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --release -p bwr-core --test plant log_records`
Expected: compile error `no field `log` on type `Sim``.

- [ ] **Step 3: Add the field and the pushes**

In the `Sim` struct after `pub faults: HashSet<Fault>,` add:

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

and in the struct literal after `faults: HashSet::new(),` add:

```rust
            log: Vec::new(),
            prev_turbine_tripped,
```

In `full_power()`, change

```rust
        s.ledger = Ledger::new(s.vessel.mass, s.vessel.energy);
        s.time = 0.0;
```

to

```rust
        s.ledger = Ledger::new(s.vessel.mass, s.vessel.energy);
        s.log.clear();
        s.time = 0.0;
```

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
Expected: unit `17 passed`, integration `10 passed`.

- [ ] **Step 5: Commit**

```bash
git add crates/bwr-core/src/sim.rs crates/bwr-core/tests/plant.rs
git commit -m "feat(core): append-only event log with sim times for protective actions

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 6: Validation module — cases, traces, CSV, runner

**Files:**
- Create: `crates/bwr-core/src/validation.rs`
- Modify: `crates/bwr-core/src/lib.rs`

**Interfaces:**
- Consumes: `Sim` (public fields `time, core, vessel, steam, fw, ic, cond, scrammed, ledger, log`), `Command`, `Fault`, `DT`.
- Produces: `ScriptedCommand`, `ScriptedFault`, `Case`, `N_TRACE_COLS = 24`, `TRACE_COLUMNS`, `Trace { case_id, dt_s, rows: Vec<[f64; 24]>, events }`, `Trace::to_csv`, `Trace::from_csv(case_id, s)`, `Trace::column(name)`, `run_case(&Case, dt) -> Trace`. Consumed by Task 7 (metrics), Task 8 (cases), Task 9 (example), Package 3 (convergence), Package 9 (scenarios).

- [ ] **Step 1: Write the failing unit tests**

Create `crates/bwr-core/src/validation.rs`:

```rust
//! Deterministic validation runner: scripted cases, machine-readable traces,
//! metrics and tolerance comparison. See docs/validation/method.md.

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
            r[col_index("power_frac")] = 1.0;
            rows.push(r);
        }
        Trace { case_id: "tiny".into(), dt_s: 0.05, rows, events: vec![(2.0, "scram: test".into())] }
    }

    #[test]
    fn csv_roundtrip_is_exact() {
        let t = tiny_trace();
        let s = t.to_csv();
        assert!(s.starts_with("# case,tiny,dt,"), "{s}");
        assert!(s.contains("\nt,power_frac,thermal_mw,"), "{s}");
        assert!(s.contains("# event,2.0000000000000000e0,scram: test"), "{s}");
        let back = Trace::from_csv("tiny", &s).unwrap();
        assert_eq!(back, t);
    }

    #[test]
    fn csv_rejects_wrong_header() {
        let err = Trace::from_csv("x", "a,b,c\n1,2,3\n").unwrap_err();
        assert!(err.contains("column header"), "{err}");
    }

    #[test]
    fn column_extracts_by_name() {
        let t = tiny_trace();
        let p = t.column("pressure_pa");
        assert_eq!(p.len(), 5);
        assert!((p[4] - 7.04e6).abs() < 1e-6);
    }

    #[test]
    fn run_case_samples_at_requested_interval_and_applies_commands() {
        let case = Case {
            id: "unit_scram",
            scenario: ScenarioName::FullPower,
            duration_s: 10.0,
            sample_dt_s: 1.0,
            commands: vec![ScriptedCommand { at_s: 3.0, cmd: Command::ManualScram }],
            faults: vec![],
        };
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
    }

    #[test]
    fn run_case_applies_faults() {
        let case = Case {
            id: "unit_fault",
            scenario: ScenarioName::HotStandby,
            duration_s: 1.0,
            sample_dt_s: 1.0,
            commands: vec![],
            faults: vec![ScriptedFault { at_s: 0.0, fault: Fault::BypassUnavailable, active: true }],
        };
        // Faults are not visible in the trace yet (nothing honours them); the runner must not panic.
        let tr = run_case(&case, DT);
        assert_eq!(tr.rows.len(), 2);
    }
}
```

- [ ] **Step 2: Register the module and verify failure**

Edit `crates/bwr-core/src/lib.rs`: insert `pub mod validation;` between `pub mod steam_table;` and `pub mod vessel;`.

Run: `cargo test --release -p bwr-core validation`
Expected: compile errors (`cannot find type `Trace``, `cannot find function `run_case``).

- [ ] **Step 3: Implement cases, traces, CSV and the runner**

Insert above the `#[cfg(test)]` block in `validation.rs`:

```rust
use std::collections::BTreeMap;

use crate::api::{Command, ScenarioName};
use crate::faults::Fault;
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
}

pub const N_TRACE_COLS: usize = 24;

/// Trace columns, in row order. Units in docs/validation/method.md.
pub const TRACE_COLUMNS: [&str; N_TRACE_COLS] = [
    "t", "power_frac", "thermal_mw", "pressure_pa", "level_z_m", "level_nr_cm",
    "mass_kg", "energy_j", "core_flow_kgs", "steam_flow_kgs", "feed_flow_kgs",
    "t_feed_c", "fuel_t_c", "ic_heat_w", "rods_avg_pos", "msiv_avg", "tcv_pos",
    "bypass_pos", "scrammed", "mass_residual_kg", "energy_residual_j",
    "cond_pressure_pa", "hotwell_kg", "ic_pool_kg",
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
    pub dt_s: f64,
    pub rows: Vec<[f64; N_TRACE_COLS]>,
    /// (sim time, "<key>: <detail>") for scram, trips, isolations, IC initiation, domain errors.
    pub events: Vec<(f64, String)>,
}

fn sample(sim: &Sim) -> [f64; N_TRACE_COLS] {
    let c = &sim.core;
    let v = &sim.vessel;
    let n_rods = c.rods.pos.len() as f64;
    let n_lines = sim.steam.msiv_pos.len() as f64;
    [
        sim.time,
        c.kin.n,
        c.thermal_power() / 1e6,
        v.eos.p,
        v.level_z,
        v.level_nr_cm(),
        v.mass,
        v.energy,
        v.core_flow,
        sim.steam.total_flow() + v.vent_flow,
        sim.fw.flow,
        sim.fw.t_feed,
        c.fuel_t,
        sim.ic.iter().map(|t| t.heat).sum(),
        c.rods.pos.iter().sum::<f64>() / n_rods,
        sim.steam.msiv_pos.iter().sum::<f64>() / n_lines,
        sim.steam.tcv_pos,
        sim.steam.bypass_pos,
        if sim.scrammed { 1.0 } else { 0.0 },
        sim.ledger.mass_residual(v.mass),
        sim.ledger.energy_residual(v.energy),
        sim.cond.pressure(),
        sim.cond.hotwell_mass,
        sim.ic.iter().map(|t| t.pool_mass).sum(),
    ]
}

/// Run a case with a fixed physics step. Deterministic: same case and dt give an
/// identical `Trace`.
pub fn run_case(case: &Case, dt: f64) -> Trace {
    let mut sim = Sim::new(case.scenario);
    let mut cmds = case.commands.clone();
    cmds.sort_by(|a, b| a.at_s.partial_cmp(&b.at_s).expect("command time is NaN"));
    let mut faults = case.faults.clone();
    faults.sort_by(|a, b| a.at_s.partial_cmp(&b.at_s).expect("fault time is NaN"));
    let (mut ci, mut fi) = (0usize, 0usize);
    let sample_every = (case.sample_dt_s / dt).round().max(1.0) as usize;
    let n_steps = (case.duration_s / dt).round() as usize;
    let mut trace = Trace { case_id: case.id.to_string(), dt_s: dt, rows: Vec::new(), events: Vec::new() };
    for step in 0..=n_steps {
        let due = sim.time + 0.5 * dt;
        while ci < cmds.len() && cmds[ci].at_s <= due {
            sim.apply(cmds[ci].cmd.clone());
            ci += 1;
        }
        while fi < faults.len() && faults[fi].at_s <= due {
            sim.set_fault(faults[fi].fault.clone(), faults[fi].active);
            fi += 1;
        }
        if step % sample_every == 0 {
            trace.rows.push(sample(&sim));
        }
        if step == n_steps {
            break;
        }
        sim.step(dt);
        trace.events.extend(sim.log.drain(..));
    }
    trace
}

impl Trace {
    /// CSV with a `# case` line, a column header, rows in `{:.16e}` (exact f64
    /// round trip), and trailing `# event,<t>,<text>` lines.
    pub fn to_csv(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!("# case,{},dt,{:.16e}\n", self.case_id, self.dt_s));
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

    pub fn from_csv(case_id: &str, s: &str) -> Result<Trace, String> {
        let mut rows = Vec::new();
        let mut events = Vec::new();
        let mut dt_s = crate::sim::DT;
        let mut header_seen = false;
        let expected_header = TRACE_COLUMNS.join(",");
        for (i, raw) in s.lines().enumerate() {
            let ln = i + 1;
            let line = raw.trim();
            if line.is_empty() {
                continue;
            }
            if let Some(rest) = line.strip_prefix("# event,") {
                let (t, text) = rest.split_once(',').ok_or(format!("line {ln}: malformed event"))?;
                let t: f64 = t.parse().map_err(|e| format!("line {ln}: event time: {e}"))?;
                events.push((t, text.to_string()));
            } else if let Some(rest) = line.strip_prefix("# case,") {
                let parts: Vec<&str> = rest.split(',').collect();
                if parts.len() >= 3 && parts[1] == "dt" {
                    dt_s = parts[2].parse().map_err(|e| format!("line {ln}: dt: {e}"))?;
                }
            } else if line.starts_with('#') {
                continue;
            } else if !header_seen {
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
        if !header_seen {
            return Err("no column header found".to_string());
        }
        Ok(Trace { case_id: case_id.to_string(), dt_s, rows, events })
    }

    pub fn column(&self, name: &str) -> Vec<f64> {
        let i = col_index(name);
        self.rows.iter().map(|r| r[i]).collect()
    }
}
```

(`BTreeMap` is imported now because Task 7 uses it in the same file; leave the unused import warning until Task 7 or add `#[allow(unused_imports)]` temporarily — prefer finishing Task 7 before pushing.)

- [ ] **Step 4: Run the tests**

Run: `cargo test --release -p bwr-core validation`
Expected: `test result: ok. 5 passed` (one warning about unused `BTreeMap` is acceptable until Task 7).

- [ ] **Step 5: Commit**

```bash
git add crates/bwr-core/src/validation.rs crates/bwr-core/src/lib.rs
git commit -m "feat(core): deterministic validation runner with CSV traces

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 7: Metrics, tolerances and comparison

**Files:**
- Modify: `crates/bwr-core/src/validation.rs`

**Interfaces:**
- Consumes: `Trace`.
- Produces: `Metrics` (`final_values`, `peak`, `min`, `first_event_time`, `max_abs_mass_residual_kg`, `max_abs_energy_residual_j`, `drift_per_s`), `Tolerance { rel, abs, event_time_s }` with `Tolerance::numerical()` and `Tolerance::physical()`, `compare(&Metrics, &Metrics, &Tolerance) -> Vec<String>`, `Trace::metrics()`. Used by Task 8, Task 9, Package 3 (convergence assertions) and Package 9.

- [ ] **Step 1: Add failing tests to the `tests` module in `validation.rs`**

```rust
    #[test]
    fn metrics_find_final_peak_min_and_drift() {
        // Pressure rises 1e4 Pa per second over 5 samples: final 7.04e6, peak at t=4, min at t=0.
        let m = tiny_trace().metrics();
        assert!((m.final_values["pressure_pa"] - 7.04e6).abs() < 1e-6);
        assert_eq!(m.peak["pressure_pa"], (7.04e6, 4.0));
        assert_eq!(m.min["pressure_pa"], (7.0e6, 0.0));
        assert!((m.drift_per_s["pressure_pa"] - 1.0e4).abs() < 1e-6, "{}", m.drift_per_s["pressure_pa"]);
        assert_eq!(m.first_event_time["scram"], 2.0);
        assert_eq!(m.max_abs_mass_residual_kg, 0.0);
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
        assert!(v.iter().any(|s| s.starts_with("pressure_pa: peak value")), "{v:?}");
        assert!(v.iter().any(|s| s.starts_with("event scram: missing")), "{v:?}");
        // Same change passes the looser physical class on value, still fails on the missing event.
        let vp = compare(&a, &b, &Tolerance::physical());
        assert!(!vp.iter().any(|s| s.starts_with("pressure_pa")), "{vp:?}");
        assert!(vp.iter().any(|s| s.starts_with("event scram")), "{vp:?}");
    }

    #[test]
    fn compare_flags_residuals_by_absolute_value_only() {
        let a = tiny_trace().metrics();
        let mut t = tiny_trace();
        t.rows[1][col_index("mass_residual_kg")] = 5.0; // above the 1 kg floor
        let v = compare(&a, &t.metrics(), &Tolerance::numerical());
        assert!(v.iter().any(|s| s.contains("mass residual")), "{v:?}");
        let mut t2 = tiny_trace();
        t2.rows[1][col_index("mass_residual_kg")] = 0.5; // under the floor: ok even though reference is 0
        assert!(!compare(&a, &t2.metrics(), &Tolerance::numerical()).iter().any(|s| s.contains("mass residual")));
    }

    #[test]
    fn compare_flags_event_timing() {
        let a = tiny_trace().metrics();
        let mut t = tiny_trace();
        t.events = vec![(2.5, "scram: test".into())];
        let v = compare(&a, &t.metrics(), &Tolerance::numerical());
        assert!(v.iter().any(|s| s.starts_with("event scram: at 2.5")), "{v:?}");
        assert!(compare(&a, &t.metrics(), &Tolerance::physical()).is_empty());
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --release -p bwr-core validation`
Expected: compile errors (`no method named `metrics``, `cannot find type `Tolerance``).

- [ ] **Step 3: Implement**

Insert into `validation.rs`, after the `impl Trace { ... }` block and before `#[cfg(test)]`:

```rust
/// Scalar reductions of a trace. Keys are trace column names (except `t`) or event keys.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Metrics {
    pub final_values: BTreeMap<String, f64>,
    /// name -> (value, time of maximum)
    pub peak: BTreeMap<String, (f64, f64)>,
    /// name -> (value, time of minimum)
    pub min: BTreeMap<String, (f64, f64)>,
    /// event key (text before the first ':') -> first time it occurred
    pub first_event_time: BTreeMap<String, f64>,
    pub max_abs_mass_residual_kg: f64,
    pub max_abs_energy_residual_j: f64,
    /// |x(t_end) - x(t_end - 60 s)| / 60 s (or over the whole trace if shorter), per column.
    pub drift_per_s: BTreeMap<String, f64>,
}

impl Trace {
    pub fn metrics(&self) -> Metrics {
        let mut m = Metrics::default();
        if self.rows.is_empty() {
            return m;
        }
        let t: Vec<f64> = self.rows.iter().map(|r| r[0]).collect();
        let t_end = *t.last().unwrap();
        let j0 = t.iter().position(|&tt| tt >= t_end - 60.0).unwrap_or(0);
        for (i, name) in TRACE_COLUMNS.iter().enumerate().skip(1) {
            let col: Vec<f64> = self.rows.iter().map(|r| r[i]).collect();
            let (mut imax, mut imin) = (0usize, 0usize);
            for (k, &v) in col.iter().enumerate() {
                if v > col[imax] {
                    imax = k;
                }
                if v < col[imin] {
                    imin = k;
                }
            }
            m.final_values.insert(name.to_string(), *col.last().unwrap());
            m.peak.insert(name.to_string(), (col[imax], t[imax]));
            m.min.insert(name.to_string(), (col[imin], t[imin]));
            let span = t_end - t[j0];
            let drift = if span > 0.0 { (col.last().unwrap() - col[j0]).abs() / span } else { 0.0 };
            m.drift_per_s.insert(name.to_string(), drift);
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
}

fn abs_floors(scale: f64) -> BTreeMap<String, f64> {
    let base: [(&str, f64); 23] = [
        ("power_frac", 1e-4),
        ("thermal_mw", 0.1),
        ("pressure_pa", 100.0),
        ("level_z_m", 1e-3),
        ("level_nr_cm", 0.1),
        ("mass_kg", 1.0),
        ("energy_j", 1e6),
        ("core_flow_kgs", 0.1),
        ("steam_flow_kgs", 0.1),
        ("feed_flow_kgs", 0.1),
        ("t_feed_c", 0.01),
        ("fuel_t_c", 0.01),
        ("ic_heat_w", 1e3),
        ("rods_avg_pos", 0.01),
        ("msiv_avg", 1e-3),
        ("tcv_pos", 1e-3),
        ("bypass_pos", 1e-3),
        ("scrammed", 0.0),
        ("mass_residual_kg", 1.0),
        ("energy_residual_j", 1e6),
        ("cond_pressure_pa", 10.0),
        ("hotwell_kg", 1.0),
        ("ic_pool_kg", 1.0),
    ];
    base.iter().map(|(k, v)| (k.to_string(), v * scale)).collect()
}

impl Tolerance {
    /// Numerical class (same physics, different step or platform): rel 1e-3, events within 0.1 s.
    /// Floors are initial values; Package 3's convergence study revises them in method.md.
    pub fn numerical() -> Self {
        Self { rel: 1e-3, abs: abs_floors(1.0), event_time_s: 0.1 }
    }

    /// Physical class (model change expected to preserve behaviour): rel 5e-2, events within 2 s.
    pub fn physical() -> Self {
        Self { rel: 5e-2, abs: abs_floors(10.0), event_time_s: 2.0 }
    }
}

fn within(a: f64, b: f64, rel: f64, abs: f64) -> bool {
    (a - b).abs() <= abs.max(rel * a.abs().max(b.abs()))
}

fn is_residual(name: &str) -> bool {
    name.ends_with("_residual_kg") || name.ends_with("_residual_j")
}

/// One human-readable violation per metric outside tolerance; empty means pass.
///
/// Rules: finals, peaks and minimums compare value-wise with `rel`/`abs`. Extremum
/// *times* are compared only when the extremum is distinct from the final value
/// (a flat signal has an arbitrary argmax), with a tolerance of 10x `event_time_s`
/// because broad extrema have poorly conditioned timing. Residual columns are
/// judged by absolute value against their floor, never against the reference.
/// Every reference event must occur in the candidate within `event_time_s`;
/// candidate events absent from the reference are violations.
pub fn compare(reference: &Metrics, candidate: &Metrics, tol: &Tolerance) -> Vec<String> {
    let mut v = Vec::new();
    let abs_of = |name: &str| tol.abs.get(name).copied().unwrap_or(0.0);

    for (name, r) in &reference.final_values {
        let Some(c) = candidate.final_values.get(name) else {
            v.push(format!("{name}: missing in candidate"));
            continue;
        };
        if is_residual(name) {
            continue;
        }
        if !within(*r, *c, tol.rel, abs_of(name)) {
            v.push(format!("{name}: final {c:.6e} vs reference {r:.6e}"));
        }
    }
    for (label, rmap, cmap) in [("peak", &reference.peak, &candidate.peak), ("min", &reference.min, &candidate.min)] {
        for (name, (rv, rt)) in rmap {
            if is_residual(name) {
                continue;
            }
            let Some((cv, ct)) = cmap.get(name) else { continue };
            if !within(*rv, *cv, tol.rel, abs_of(name)) {
                v.push(format!("{name}: {label} value {cv:.6e} vs reference {rv:.6e}"));
                continue;
            }
            let distinct = reference
                .final_values
                .get(name)
                .map(|f| !within(*rv, *f, tol.rel, abs_of(name)))
                .unwrap_or(false);
            if distinct && (rt - ct).abs() > 10.0 * tol.event_time_s {
                v.push(format!("{name}: {label} time {ct:.2} s vs reference {rt:.2} s"));
            }
        }
    }
    if candidate.max_abs_mass_residual_kg > abs_of("mass_residual_kg") {
        v.push(format!(
            "max |mass residual| {:.4e} kg exceeds {:.1e} kg",
            candidate.max_abs_mass_residual_kg,
            abs_of("mass_residual_kg")
        ));
    }
    if candidate.max_abs_energy_residual_j > abs_of("energy_residual_j") {
        v.push(format!(
            "max |energy residual| {:.4e} J exceeds {:.1e} J",
            candidate.max_abs_energy_residual_j,
            abs_of("energy_residual_j")
        ));
    }
    for (key, rt) in &reference.first_event_time {
        match candidate.first_event_time.get(key) {
            None => v.push(format!("event {key}: missing (reference at {rt:.2} s)")),
            Some(ct) if (rt - ct).abs() > tol.event_time_s => {
                v.push(format!("event {key}: at {ct:.2} s vs reference {rt:.2} s"))
            }
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

- [ ] **Step 4: Run the tests**

Run: `cargo test --release -p bwr-core validation`
Expected: `test result: ok. 9 passed`, no warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/bwr-core/src/validation.rs
git commit -m "feat(core): trace metrics, tolerance classes and comparison

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 8: Standard cases, determinism and convergence tests

**Files:**
- Modify: `crates/bwr-core/src/validation.rs` (add `standard_cases`)
- Modify: `crates/bwr-core/tests/plant.rs` (append two tests)

**Interfaces:**
- Consumes: `Case`, `run_case`, `compare`, `Tolerance`, `Command`, `PRESSURE_SETPOINT_DEFAULT`.
- Produces: `standard_cases() -> Vec<Case>` with ids `fp_steady_600`, `fp_manual_scram`, `fp_turbine_trip`, `fp_msiv_closure`, `fp_pressure_step`, `fp_loss_fw_heating`, `fp_load_reject`, `fp_fw_pump_trip`, `hs_steady_600`, `cs_steady_600`. Package 9 appends cases to this function and to `docs/validation/cases.csv`.

- [ ] **Step 1: Write the failing tests**

Append to `crates/bwr-core/tests/plant.rs` (also add `use bwr_core::validation::{compare, run_case, standard_cases, Tolerance};` to the imports at the top of the file):

```rust
fn case(id: &str) -> bwr_core::validation::Case {
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
        assert!(tr.rows.iter().all(|r| r.iter().all(|v| v.is_finite())), "{}: non-finite value", c.id);
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

Add to the `use` lines at the top of `validation.rs`:

```rust
use crate::api::ModeSwitch;
use crate::params::PRESSORE_SETPOINT_DEFAULT_PLACEHOLDER;
```

Correction: the constant is `PRESSURE_SETPOINT_DEFAULT`; the import line is exactly:

```rust
use crate::api::{Command, ModeSwitch, ScenarioName};
use crate::params::PRESSURE_SETPOINT_DEFAULT;
```

(replace the existing `use crate::api::{Command, ScenarioName};` line). Then insert before `#[cfg(test)]`:

```rust
fn at(at_s: f64, cmd: Command) -> ScriptedCommand {
    ScriptedCommand { at_s, cmd }
}

fn case(id: &'static str, scenario: ScenarioName, duration_s: f64, commands: Vec<ScriptedCommand>) -> Case {
    Case { id, scenario, duration_s, sample_dt_s: 1.0, commands, faults: vec![] }
}

/// The regression case set. Ids are stable; adding a case means adding a row to
/// docs/validation/cases.csv and a baseline file.
pub fn standard_cases() -> Vec<Case> {
    use Command::*;
    let fp = ScenarioName::FullPower;
    vec![
        case("fp_steady_600", fp, 600.0, vec![]),
        case(
            "fp_manual_scram",
            fp,
            300.0,
            vec![at(5.0, ManualScram), at(5.0, SetModeSwitch { mode: ModeSwitch::Shutdown })],
        ),
        case("fp_turbine_trip", fp, 300.0, vec![at(5.0, TripTurbine)]),
        case(
            "fp_msiv_closure",
            fp,
            600.0,
            vec![at(5.0, SetMsiv { line: 0, open: false }), at(5.0, SetMsiv { line: 1, open: false })],
        ),
        case(
            "fp_pressure_step",
            fp,
            60.0,
            vec![at(5.0, SetPressureSetpoint { mpa: PRESSURE_SETPOINT_DEFAULT / 1e6 + 0.1 })],
        ),
        case("fp_loss_fw_heating", fp, 300.0, vec![at(5.0, SetHeaters { in_service: false })]),
        case("fp_load_reject", fp, 300.0, vec![at(5.0, OpenBreaker)]),
        case("fp_fw_pump_trip", fp, 300.0, vec![at(5.0, SetFeedPump { index: 0, on: false })]),
        case("hs_steady_600", ScenarioName::HotStandby, 600.0, vec![]),
        case("cs_steady_600", ScenarioName::ColdShutdown, 600.0, vec![]),
    ]
}
```

- [ ] **Step 4: Run the whole suite and read the convergence report**

Run: `cargo test --release -p bwr-core -- --nocapture convergence_report_half_step`
Expected: the printed report (any number of violations is acceptable in this package; copy the list into the Task 10 commit message body as the measured starting point).

Run: `cargo test --release -p bwr-core`
Expected: unit `21 passed`, integration `13 passed`.

- [ ] **Step 5: Commit**

```bash
git add crates/bwr-core/src/validation.rs crates/bwr-core/tests/plant.rs
git commit -m "feat(core): standard validation cases; determinism and convergence tests

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 9: Validation example, baselines, case register and method

**Files:**
- Create: `crates/bwr-core/examples/validation.rs`
- Create: `docs/validation/method.md`, `docs/validation/cases.csv`
- Create (generated): `docs/validation/baseline/<id>.csv` for the 10 ids

**Interfaces:**
- Consumes: `standard_cases`, `run_case`, `compare`, `Tolerance::numerical`, `Trace`, `Metrics`, `DT`.
- Produces: the `validation` example (`--write`, `--case <id>`), exit code 1 on any FAIL; baseline files; `docs/validation/cases.csv` with header `id,scenario,duration_s,purpose,reference,tolerance_class,status`. Package 9 appends rows and baselines; Package 10 cites `method.md` in `results.md`.

- [ ] **Step 1: Write the example**

Create `crates/bwr-core/examples/validation.rs`:

```rust
//! Validation runner.
//!
//! ```sh
//! cargo run --release -p bwr-core --example validation              # compare against baselines
//! cargo run --release -p bwr-core --example validation -- --write   # regenerate baselines
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
        "  {:<20} {:>14} {:>14} {:>9} {:>14} {:>9} {:>12}",
        "column", "final", "peak", "t_peak", "min", "t_min", "drift/s"
    );
    for (name, f) in &m.final_values {
        let (pv, pt) = m.peak[name];
        let (mv, mt) = m.min[name];
        let d = m.drift_per_s[name];
        println!("  {name:<20} {f:>14.6e} {pv:>14.6e} {pt:>9.1} {mv:>14.6e} {mt:>9.1} {d:>12.3e}");
    }
    println!(
        "  max |mass residual| {:.3e} kg, max |energy residual| {:.3e} J",
        m.max_abs_mass_residual_kg, m.max_abs_energy_residual_j
    );
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
                Ok(s) => match Trace::from_csv(case.id, &s) {
                    Ok(reference) => compare(&reference.metrics(), &m, &tol),
                    Err(e) => vec![format!("baseline unreadable: {e}")],
                },
                Err(_) => vec![format!("no baseline at {}; run with --write", path.display())],
            };
            if violations.is_empty() {
                println!("PASS {}", case.id);
            } else {
                failed += 1;
                println!("FAIL {}", case.id);
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
Expected: ten `PASS <id>` lines and `exit=0`. Each case also prints its `convergence dt/2` report; any violations there are recorded, not fixed, in this package.

Run: `ls docs/validation/baseline | wc -l`
Expected: `10`.

- [ ] **Step 4: Write the case register**

Create `docs/validation/cases.csv`:

```csv
id,scenario,duration_s,purpose,reference,tolerance_class,status
fp_steady_600,fullPower,600,Rated-power steady state: initial equilibrium residual and drift,none (regression),numerical,baseline
fp_manual_scram,fullPower,300,Manual scram with mode switch to SHUTDOWN: power decay and level shrink without isolation,none (regression),numerical,baseline
fp_turbine_trip,fullPower,300,Turbine trip at rated power: bypass ride-through and Select Rod Insert,none (regression),numerical,baseline
fp_msiv_closure,fullPower,600,Closure of both MSIVs: scram and isolation condenser initiation,none (regression),numerical,baseline
fp_pressure_step,fullPower,60,+0.1 MPa pressure setpoint step: void collapse reactivity response,none (regression),numerical,baseline
fp_loss_fw_heating,fullPower,300,Feedwater heaters removed from service: cold feed reactivity response,none (regression),numerical,baseline
fp_load_reject,fullPower,300,Generator breaker opened at rated load: speed excursion and bypass,none (regression),numerical,baseline
fp_fw_pump_trip,fullPower,300,One of two running feed pumps stopped: level control recovery,none (regression),numerical,baseline
hs_steady_600,hotStandby,600,Hot standby steady state: decay heat balance and level drift,none (regression),numerical,baseline
cs_steady_600,coldShutdown,600,Cold shutdown steady state: vented vessel and 30-day decay heat,none (regression),numerical,baseline
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
plus a time-stamped script of operator commands and fault injections. Running a
case produces a trace that any reviewer can regenerate bit-for-bit. Metrics
reduce the trace to a small set of numbers that can be compared under an
explicit tolerance. Baselines saved from the current engine are regression
evidence: they show that behaviour did not change unintentionally. They are
not physical truth and must never be regenerated merely to make a failing
comparison pass.

## Determinism

`run_case(case, dt)` builds the scenario, then for each physics step applies
every scripted command and fault whose time is at or before the step start,
samples the state every `sample_dt_s`, advances one fixed step `dt`, and drains
the engine event log. Commands therefore take effect at a physics boundary.
The same case and `dt` give an identical `Trace` on any platform; this is
asserted by `validation_runner_is_deterministic` in `tests/plant.rs`.

## Trace columns

| Column | Unit | Source |
|---|---|---|
| `t` | s | simulation time |
| `power_frac` | - | neutron power relative to rated (`core.kin.n`) |
| `thermal_mw` | MW | total core thermal power incl. decay heat |
| `pressure_pa` | Pa | vessel dome pressure |
| `level_z_m` | m | mixture level above vessel bottom |
| `level_nr_cm` | cm | narrow-range level relative to NWL |
| `mass_kg` | kg | vessel fluid inventory |
| `energy_j` | J | vessel fluid + metal internal energy |
| `core_flow_kgs` | kg/s | natural circulation core flow |
| `steam_flow_kgs` | kg/s | main steam plus head vent flow |
| `feed_flow_kgs` | kg/s | feedwater flow |
| `t_feed_c` | degC | feedwater temperature |
| `fuel_t_c` | degC | lumped average fuel temperature |
| `ic_heat_w` | W | total isolation condenser heat removal |
| `rods_avg_pos` | notch units (0 in .. 48 out) | mean rod position |
| `msiv_avg` | - | mean MSIV position |
| `tcv_pos` | - | turbine control valve position |
| `bypass_pos` | - | bypass valve position |
| `scrammed` | 0/1 | scram latched |
| `mass_residual_kg` | kg | `mass - (mass0 + in - out)` from the ledger |
| `energy_residual_j` | J | `energy - (energy0 + in - out)` from the ledger |
| `cond_pressure_pa` | Pa | condenser pressure |
| `hotwell_kg` | kg | hotwell inventory |
| `ic_pool_kg` | kg | total IC pool inventory |

Events are `(t, "<key>: <detail>")` lines. Keys at this package: `scram`,
`turbine trip`, `msiv isolation`, `ic auto-initiation`, `feed pump trip`.
Package 3 adds `domain error`.

## Residuals

The ledger integrates the flows the vessel actually applied. A closed model has
a residual that is zero to floating-point error. A non-physical clamp shows up
as `mass_clipped`/`energy_clipped` in the ledger and as a nonzero residual. At
the Package 2 baseline the only such term is the 1000 kg inventory floor, which
no standard case reaches; Package 3 removes it.

## Metrics

For every column except `t`:

- **final value** at the last sample;
- **peak** (maximum) and **minimum** with the time at which each first occurs;
- **drift**: |x(t_end) - x(t_end - 60 s)| / 60 s, or over the whole trace if
  shorter than 60 s. The **initial equilibrium residual** of a steady case is
  the drift computed over its first 60 s; report it with the case.

For the whole trace:

- **max |mass residual|** and **max |energy residual|**;
- **first event time** per event key, which gives actuation order and delay;
- **integrated transfers**: feed mass in, steam/vent/liquid mass out, energy in
  and out, read directly from `Sim::ledger` at the end of the run.

## Tolerance classes

| Class | Use | `rel` | event time | abs floors |
|---|---|---|---|---|
| numerical | same physics, different step, platform, or refactor | 1e-3 | 0.1 s | table below x1 |
| physical | an intended model change that should preserve behaviour | 5e-2 | 2 s | table below x10 |
| source-comparison | comparison with a public reference trace | per case, stated in `cases.csv` | per case | per case |

Absolute floors (numerical class): pressure 100 Pa; mixture level 1e-3 m;
narrow-range level 0.1 cm; mass 1 kg; energy 1e6 J; flows 0.1 kg/s;
temperatures 0.01 degC; power fraction 1e-4; thermal power 0.1 MW; IC heat
1 kW; rod position 0.01; valve positions 1e-3; condenser pressure 10 Pa;
hotwell and IC pool inventory 1 kg; residuals 1 kg and 1e6 J.

These floors are starting values. Package 3's convergence study (0.05, 0.025
and 0.0125 s steps) sets them from measured discretisation error, and the
revised values are recorded here with the study results. Tolerances are set
before any tuning, never after.

## Comparison rules

`compare(reference, candidate, tol)` reports one violation per:

- final, peak or minimum value outside `max(abs, rel * max(|a|, |b|))`;
- extremum time differing by more than 10 x the event-time tolerance, checked
  only when the extremum is distinct from the final value (a flat signal has an
  arbitrary argmax, and broad extrema have poorly conditioned timing);
- max |residual| above its floor (residuals are judged absolutely, never
  against the reference);
- a reference event missing from the candidate, or occurring more than the
  event-time tolerance away; an event in the candidate that is not in the
  reference.

A comparison is reported as PASS only with its tolerance class and reference
named. "PASS against regression baseline" is not a statement about the plant.

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

### Task 10: CI, package script and README

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
`docs/validation/method.md` defines the trace columns, metrics and tolerance classes. The runner also reports how each
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
Expected: unit `21 passed`, integration `13 passed`.

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

---

## Acceptance gate (verbatim from the spec)

"Repeat runs give reproducible traces. Each claimed comparison identifies its reference and tolerance. No previous-test failure is concealed by updating expected output."

Checked by: `validation_runner_is_deterministic` (reproducible traces); `docs/validation/cases.csv` `reference` and `tolerance_class` columns plus the PASS/FAIL output naming the tolerance class (each comparison identifies its reference and tolerance); Task 1 baseline run recorded before any change and the rule in `method.md` and README that baselines are never regenerated to hide a failure.

## Self-review against the spec's Package 2 bullets

| Spec bullet | Task |
|---|---|
| Establish Rust toolchain access and run the existing tests before changing physics | Task 1 |
| Deterministic validation runner with simulation time, scripted commands, machine-readable traces incl. pressure, inventory, energy, power, level, flow, temperatures, valve states, rod positions, protection actions, numerical residuals | Tasks 2, 3, 5, 6 (24 columns + events) |
| Save baseline results as regression evidence, not physical truth | Task 9 (files, `method.md`, README) |
| Comparison metrics: initial equilibrium residuals, peaks and times, minimum level, integrated transfers, actuation order and delay, final state, drift | Task 7 (`Metrics`), `method.md` |
| Separate numerical, physical, source-comparison tolerances; set from convergence before tuning | Task 7 (`Tolerance::numerical/physical`), Task 8 convergence report, `method.md` |
| Automate engine tests, UI checks, platform-appropriate desktop verification | Task 10 (`validate.yml`; `ci.yml` desktop matrix unchanged) |
| Deterministic test-level fault overrides (needed by later packages) | Task 4 |
