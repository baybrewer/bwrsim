# Package 9: Verify Complete Existing-System Scenarios — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Names and signatures are governed by `docs/superpowers/plans/2026-10-20-refinement-master.md`; this file adds the names listed under "Contract additions".

**Goal:** Give every implemented subsystem and operator control an assigned, deterministic scenario with objective assertions, so that the engine's whole-plant behaviour is verified end to end and every unsupported condition is reported rather than silently extrapolated.

**Architecture:** Extend the Package 2 validation runner with closed-loop scripted operators (`Operator` enum + `run_sim_with`), define the full `standard_cases()` register and `docs/validation/cases.csv`, and add `tests/scenarios.rs`, a table-driven test file that asserts actuation order, extrema bounds, support envelope, and fault persistence for every case. Analytical component checks and a paired-failure exploration provide verification evidence where no public trace exists.

**Tech Stack:** Rust 2021 (`bwr-core`), existing `serde_json` dev-dependency, hand-rolled CSV (no new dependencies).

**Spec section:** Package 9 of `docs/superpowers/plans/2026-10-20-existing-systems-refinement.md`.

**Prerequisites:** Packages 1–8 complete: `validation.rs` (`Case`, `Trace`, `Metrics`, `Tolerance`, `run_case`, `standard_cases`, `compare`), `faults.rs` (`Fault`, `Sim::set_fault`, `Sim::has_fault`), `ledger.rs`, `domain.rs` (`DomainError`, `Sim::domain_errors`), `Sim.log`, `Regime`, `Rods::strongest_rod`, `Rods::shutdown_margin`, `RodMotion`, `Core::clad_t`, `DecayHeat::after_history`, `Sim.balance`, `IcTrain::in_service`, `IcTrain::available`, `IcTrain::ua_factor`, sensors.

## Global Constraints

Everything in the master plan's Global Constraints, plus:

- "Do not bulk-retune the plant between packages or overwrite validation references simply to restore a passing startup script."
- "No previous-test failure is concealed by updating expected output."
- "Compare against public transient traces where available. Calibrate on one set of cases and reserve independent cases for validation."
- "For missing public traces, use conservation, analytical component solutions, qualitative response ordering, parameter sensitivity, and convergence tests. Label this as verification or plausibility evidence, not quantitative BWRX transient validation."
- "Use deterministic test-level fault overrides for existing equipment so automatic logic cannot erase an injected stuck-valve or unavailable-train condition."
- **Finding policy (this package):** when a *physical* assertion in `tests/scenarios.rs` fails (ordering, pressure bound, envelope), do not change parameters or the assertion. Record the case id, the observed value and the suspected cause as a numbered finding `F-nnn` in `docs/validation/findings.md`, mark that one test `#[ignore = "F-nnn"]`, and move on. Numerical assertions (convergence, residuals, determinism) are never ignored; they are fixed.
- `Sim.log` event strings are read through the prefix constants in `mod ev` of `tests/scenarios.rs` (Task 6). Before writing any assertion, run `grep -n "log.push" crates/bwr-core/src/sim.rs` and set those constants to the literal prefixes Package 2 and Package 3 used. Only `mod ev` may contain event-string literals.

## Contract additions (append to the master plan in the same commit as Task 1)

```rust
// validation.rs
pub const OPERATOR_DT_S: f64 = 1.0;
pub type OperatorFn = Box<dyn FnMut(&Sim, f64) -> Vec<Command>>;

#[derive(Clone, Debug, PartialEq)]
pub enum Operator {
    None,
    /// Hot standby -> critical -> sync -> `first_frac`, hold `hold_s`, then `target_frac`.
    Startup { first_frac: f64, hold_s: f64, target_frac: f64 },
    ControlledShutdown,
    CooldownOnIc,
    RodWithdrawalError,
}
pub struct Case { /* P2 fields plus: */ pub operator: Operator }

/// Primitive: runs `sim` under `case` with an explicit operator; returns the trace and the final Sim.
pub fn run_sim_with(sim: Sim, case: &Case, dt: f64, operator: OperatorFn) -> (Trace, Sim);
/// `Sim::new(case.scenario)` + `run_sim_with`, trace only (ignores `case.operator`).
pub fn run_case_with(case: &Case, dt: f64, operator: OperatorFn) -> Trace;
/// = run_case_with(case, dt, operators::build(&case.operator))
pub fn run_case(case: &Case, dt: f64) -> Trace;

pub mod operators {
    pub fn build(op: &Operator) -> OperatorFn;
    pub fn startup(first_frac: f64, hold_s: f64, target_frac: f64) -> OperatorFn;
    pub fn controlled_shutdown() -> OperatorFn;
    pub fn cooldown_on_ic() -> OperatorFn;
    pub fn rod_withdrawal_error() -> OperatorFn;
}

/// Public reference points (t_s, column, value) compared to the trace with `tol`.
pub fn parse_reference_csv(s: &str) -> Result<Vec<(f64, String, f64)>, String>;
pub fn compare_reference(trace: &Trace, points: &[(f64, String, f64)], tol: &Tolerance) -> Vec<String>;

// sim.rs
impl Sim { pub fn settle_until_steady(&mut self, max_s: f64) -> f64; }
// params.rs
pub const IRRADIATION_HISTORY_S: f64 = 6.31e7;  // ASSUMED: two effective full-power years before each scenario
```

Files: `docs/validation/cases.csv` gains a trailing `role` column (`calibration | validation | plausibility`); `examples/validation.rs` gains `--write-missing`; new `examples/paired_failures.rs`, `docs/validation/paired-failures.md`, `docs/validation/findings.md`, `docs/validation/reference/<case>.csv` (only if a public trace is found).

**Cross-package names this plan relies on that are not in the master contract** (confirm in code before use; change only the call site): `Core::coeffs: ReactivityCoeffs { doppler_a, void_coef, mod_temp_coef, rho_excess_cold, .. }` (Package 5 sensitivity structure) and `params::CST_CAPACITY` (Package 6 condensate storage tank capacity, kg).

---

### Task 1: Closed-loop operators in the validation runner

**Files:**
- Modify: `crates/bwr-core/src/validation.rs` (add `Operator`, `OperatorFn`, `run_sim_with`, `run_case_with`, `operators` module; `run_case` becomes a one-liner)
- Modify: every `Case { .. }` literal in `validation.rs` (`standard_cases()`), adding `operator: Operator::None`
- Test: `crates/bwr-core/tests/scenarios.rs` (new)

**Interfaces:**
- Consumes: P2 `Case`, `Trace`, `run_case`'s stepping loop and row sampler (P2 named the sampler; call it `sample_row(&Sim) -> [f64; 24]` below and keep P2's name if it differs), `Sim::apply`, `Sim::set_fault`, `Sim.log`.
- Produces: the contract additions above. `run_sim_with` applies scripted faults, then scripted commands, then operator commands, at the start of each step whose time has reached their `at_s`; the operator runs every `OPERATOR_DT_S` seconds of simulation time.

- [ ] **Step 1: Write the failing test**

```rust
// crates/bwr-core/tests/scenarios.rs
use bwr_core::api::{Command, ScenarioName};
use bwr_core::sim::DT;
use bwr_core::validation::{run_case, run_case_with, run_sim_with, standard_cases, Case, Operator};
use bwr_core::Sim;

fn steady_case() -> Case {
    Case {
        id: "op_test",
        scenario: ScenarioName::FullPower,
        duration_s: 30.0,
        sample_dt_s: 1.0,
        commands: vec![],
        faults: vec![],
        operator: Operator::None,
    }
}

#[test]
fn operator_commands_are_applied_once_per_second() {
    let mut calls = 0u32;
    let trace = run_case_with(
        &steady_case(),
        DT,
        Box::new(move |sim: &Sim, t: f64| {
            calls += 1;
            assert!((t - (calls as f64 - 1.0)).abs() < 1e-6, "operator called at t={t}, call {calls}");
            if t >= 5.0 && !sim.scrammed { vec![Command::ManualScram] } else { vec![] }
        }),
    );
    let scram_t = trace.events.iter().find(|(_, s)| s.contains("Manual scram")).map(|(t, _)| *t);
    assert!(matches!(scram_t, Some(t) if (5.0..5.1).contains(&t)), "events {:?}", trace.events);
    assert_eq!(trace.column("scrammed").last().copied(), Some(1.0));
}

#[test]
fn run_case_equals_run_case_with_no_operator() {
    let c = steady_case();
    let a = run_case(&c, DT);
    let b = run_case_with(&c, DT, Box::new(|_: &Sim, _: f64| vec![]));
    assert_eq!(a, b);
}

#[test]
fn run_sim_with_returns_final_sim() {
    let c = steady_case();
    let (trace, sim) = run_sim_with(Sim::new(c.scenario), &c, DT, Box::new(|_: &Sim, _: f64| vec![]));
    assert!((sim.time - trace.rows.last().unwrap()[0]).abs() <= DT + 1e-9);
    assert!(standard_cases().iter().all(|c| c.operator == Operator::None || c.duration_s > 0.0));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --release -p bwr-core --test scenarios`
Expected: compile error `no field operator`, `cannot find function run_case_with`.

- [ ] **Step 3: Implement**

In `validation.rs`, add after the `Case` struct (and add `pub operator: Operator` as its last field):

```rust
pub const OPERATOR_DT_S: f64 = 1.0;
pub type OperatorFn = Box<dyn FnMut(&Sim, f64) -> Vec<Command>>;

#[derive(Clone, Debug, PartialEq)]
pub enum Operator {
    None,
    Startup { first_frac: f64, hold_s: f64, target_frac: f64 },
    ControlledShutdown,
    CooldownOnIc,
    RodWithdrawalError,
}

pub fn run_sim_with(mut sim: Sim, case: &Case, dt: f64, mut operator: OperatorFn) -> (Trace, Sim) {
    let mut cmds = case.commands.clone();
    cmds.sort_by(|a, b| a.at_s.partial_cmp(&b.at_s).unwrap());
    let mut faults = case.faults.clone();
    faults.sort_by(|a, b| a.at_s.partial_cmp(&b.at_s).unwrap());
    let (mut ci, mut fi) = (0usize, 0usize);
    let mut rows = Vec::new();
    let (mut next_sample, mut next_op) = (0.0f64, 0.0f64);
    let steps = (case.duration_s / dt).round() as usize;
    for _ in 0..=steps {
        let t = sim.time;
        while fi < faults.len() && faults[fi].at_s <= t + 1e-9 {
            sim.set_fault(faults[fi].fault.clone(), faults[fi].active);
            fi += 1;
        }
        while ci < cmds.len() && cmds[ci].at_s <= t + 1e-9 {
            sim.apply(cmds[ci].cmd.clone());
            ci += 1;
        }
        if t + 1e-9 >= next_op {
            for c in operator(&sim, t) {
                sim.apply(c);
            }
            next_op += OPERATOR_DT_S;
        }
        if t + 1e-9 >= next_sample {
            rows.push(sample_row(&sim));
            next_sample += case.sample_dt_s;
        }
        sim.step(dt);
    }
    let events = std::mem::take(&mut sim.log);
    (Trace { case_id: case.id.to_string(), dt_s: dt, rows, events }, sim)
}

pub fn run_case_with(case: &Case, dt: f64, operator: OperatorFn) -> Trace {
    run_sim_with(Sim::new(case.scenario), case, dt, operator).0
}

pub fn run_case(case: &Case, dt: f64) -> Trace {
    run_case_with(case, dt, operators::build(&case.operator))
}

pub mod operators {
    use super::{Operator, OperatorFn};
    use crate::api::Command;
    use crate::Sim;

    pub fn build(op: &Operator) -> OperatorFn {
        match *op {
            Operator::None => Box::new(|_: &Sim, _: f64| Vec::<Command>::new()),
            Operator::Startup { first_frac, hold_s, target_frac } => startup(first_frac, hold_s, target_frac),
            Operator::ControlledShutdown => controlled_shutdown(),
            Operator::CooldownOnIc => cooldown_on_ic(),
            Operator::RodWithdrawalError => rod_withdrawal_error(),
        }
    }
    // startup, controlled_shutdown, cooldown_on_ic, rod_withdrawal_error are added in Tasks 3 and 4.
    pub fn startup(_: f64, _: f64, _: f64) -> OperatorFn { Box::new(|_: &Sim, _: f64| vec![]) }
    pub fn controlled_shutdown() -> OperatorFn { Box::new(|_: &Sim, _: f64| vec![]) }
    pub fn cooldown_on_ic() -> OperatorFn { Box::new(|_: &Sim, _: f64| vec![]) }
    pub fn rod_withdrawal_error() -> OperatorFn { Box::new(|_: &Sim, _: f64| vec![]) }
}
```

Delete P2's stepping loop from the old `run_case` (it is now `run_sim_with`). The empty operator bodies above exist only so this task compiles; Tasks 3 and 4 replace them with the real closures and must not leave any of them empty.

- [ ] **Step 4: Run tests**

Run: `cargo test --release -p bwr-core`
Expected: all P2–P8 tests still pass; the three new tests pass. `cargo run --release -p bwr-core --example validation` reports every baseline unchanged (the loop is identical).

- [ ] **Step 5: Commit**

```bash
git add crates/bwr-core/src/validation.rs crates/bwr-core/tests/scenarios.rs docs/superpowers/plans/2026-10-20-refinement-master.md
git commit -m "feat(validation): closed-loop operators and run_sim_with primitive

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 2: Initial conditions match their claimed histories

**Files:**
- Modify: `crates/bwr-core/src/sim.rs` (`cold_shutdown`, `hot_standby`, `full_power`, new `settle_until_steady`)
- Modify: `crates/bwr-core/src/params.rs` (`IRRADIATION_HISTORY_S`)
- Modify: `docs/model/parameters.csv`, `docs/model/assumptions.md`
- Test: `crates/bwr-core/tests/scenarios.rs`

**Interfaces:**
- Consumes: `DecayHeat::after_history(n, irr_s, cooled_s)`, `Xenon::equilibrium`, `Xenon::step`, `Regime`, `Sim.balance.residual_w`, `Trace::metrics().drift_per_s`, `Tolerance::numerical().abs`.
- Produces: `Sim::settle_until_steady(max_s) -> f64`, `params::IRRADIATION_HISTORY_S`.

- [ ] **Step 1: Write the failing tests**

```rust
use bwr_core::params::*;
use bwr_core::reactor::{DecayHeat, Xenon};
use bwr_core::steam::P_ATM;
use bwr_core::validation::Tolerance;
use bwr_core::vessel::Regime;

#[test]
fn cold_shutdown_matches_claimed_history() {
    let sim = Sim::new(ScenarioName::ColdShutdown);
    let expect = DecayHeat::after_history(1.0, IRRADIATION_HISTORY_S, 30.0 * 86400.0).power();
    assert!((sim.core.decay.power() - expect).abs() < 1e-9, "decay {} vs {expect}", sim.core.decay.power());
    assert_eq!((sim.core.xenon.xenon, sim.core.xenon.iodine), (0.0, 0.0));
    assert!((sim.vessel.eos.t - 40.0).abs() < 0.5, "t {}", sim.vessel.eos.t);
    assert_eq!(sim.vessel.eos.regime, Regime::Vented);
    assert!((sim.vessel.eos.p - P_ATM).abs() < 1.0);
    assert!(sim.vessel.head_vent_open);
    assert!(sim.core.rods.pos.iter().all(|&p| p <= 0.01));
    assert!(sim.core.k_eff() < 0.95);
    assert!(sim.ic.iter().all(|t| !t.cmd_open && !t.in_service()));
    assert_eq!(sim.fw.flow, 0.0);
    assert_eq!(sim.steam.total_flow(), 0.0);
}

#[test]
fn hot_standby_matches_claimed_history() {
    let sim = Sim::new(ScenarioName::HotStandby);
    let expect = DecayHeat::after_history(1.0, IRRADIATION_HISTORY_S, 2.0 * 86400.0).power();
    assert!((sim.core.decay.power() - expect).abs() < 1e-9);
    let mut xe = Xenon::equilibrium(1.0);
    for _ in 0..(48 * 60) {
        xe.step(0.0, 60.0);
    }
    assert!((sim.core.xenon.xenon - xe.xenon).abs() < 1e-9 && (sim.core.xenon.iodine - xe.iodine).abs() < 1e-9);
    assert!((sim.vessel.eos.p - 6.9e6).abs() < 0.05e6, "p {}", sim.vessel.eos.p);
    assert_eq!(sim.vessel.eos.regime, Regime::Saturated);
    assert!(sim.cond.cw_pumps.iter().any(|&b| b) && sim.cond.pressure() < 10.0e3);
    assert!(sim.ic.iter().all(|t| !t.cmd_open));
    assert!(sim.core.rods.pos.iter().all(|&p| p <= 0.01));
}

#[test]
fn full_power_is_an_initial_equilibrium() {
    let sim = Sim::new(ScenarioName::FullPower);
    let xe_eq = Xenon::equilibrium(sim.core.kin.n);
    assert!((sim.core.xenon.xenon - xe_eq.xenon).abs() < 0.01, "xenon {} vs {}", sim.core.xenon.xenon, xe_eq.xenon);
    assert!((sim.vessel.eos.p - P_RATED).abs() < 0.05e6);
    assert!(sim.vessel.level_nr_cm().abs() < 5.0);
    assert!(sim.balance.residual_w.abs() < 0.01 * RATED_POWER_W, "balance residual {} W", sim.balance.residual_w);
    let case = standard_cases().into_iter().find(|c| c.id == "fp_steady_600").unwrap();
    let m = run_case(&case, DT).metrics();
    let tol = Tolerance::numerical();
    for col in ["power_frac", "pressure_pa", "level_nr_cm"] {
        let limit = tol.abs[col] / 60.0;
        assert!(m.drift_per_s[col].abs() < limit, "{col} drifts {} /s > {limit}", m.drift_per_s[col]);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --release -p bwr-core --test scenarios -- initial history equilibrium`
Expected: `cold_shutdown_matches_claimed_history` and `hot_standby_matches_claimed_history` fail on the decay-heat comparison (the constructors still use infinite irradiation, `DecayHeat::after_shutdown`). `full_power_is_an_initial_equilibrium` may fail on drift (600 s + 120 s settle is not converged); record the printed drift values in the commit message.

- [ ] **Step 3: Implement**

`params.rs`:

```rust
/// Effective full-power irradiation before every scenario's shutdown, s. ASSUMED
/// (two years at rated power; bounds decay heat below the infinite-operation value). A-0xx.
pub const IRRADIATION_HISTORY_S: f64 = 6.31e7;
```

`sim.rs`, in `cold_shutdown()` replace `core.decay = DecayHeat::after_shutdown(30.0 * 86400.0);` with
`core.decay = DecayHeat::after_history(1.0, IRRADIATION_HISTORY_S, 30.0 * 86400.0);` and in `hot_standby()` the same with `2.0 * 86400.0`.

Add to `impl Sim`:

```rust
/// Settle in 60 s blocks until power and pressure drift are below the initial-equilibrium
/// limits in docs/validation/method.md, or `max_s` elapses. Returns the seconds settled.
pub fn settle_until_steady(&mut self, max_s: f64) -> f64 {
    let t0 = self.time;
    loop {
        let (n0, p0) = (self.core.kin.n, self.vessel.eos.p);
        self.settle(60.0);
        let dn = (self.core.kin.n - n0).abs() / 60.0;
        let dp = (self.vessel.eos.p - p0).abs() / 60.0;
        if (dn < 1.0e-6 && dp < 20.0) || self.time - t0 >= max_s {
            return self.time - t0;
        }
    }
}
```

In `full_power()`, replace the final `s.settle(120.0);` with `s.settle_until_steady(3600.0);`. Do not change any parameter. If `fp_steady_600`'s baseline changes because of the longer settle, regenerate only that baseline with `--write` and say so in the commit message (this is an initialization fix, not retuning).

Add rows to `docs/model/parameters.csv`: `IRRADIATION_HISTORY_S,6.31e7,s,"irradiation before scenario shutdown",assumed,A-0xx,sim.rs` and the assumption text to `assumptions.md`: "A-0xx: Every scenario's fuel has operated two effective full-power years before the stated shutdown. Used only for decay heat. Sensitivity: 1 to 4 years changes 30-day decay heat by less than 10 %."

- [ ] **Step 4: Run tests**

Run: `cargo test --release -p bwr-core`
Expected: all pass. `cargo run --release -p bwr-core --example validation` passes (or only `fp_steady_600`, `cs_steady_600`, `hs_steady_600` differ, in which case regenerate those three with `--write` and state the reason in the commit).

- [ ] **Step 5: Commit**

```bash
git add crates/bwr-core/src/sim.rs crates/bwr-core/src/params.rs crates/bwr-core/tests/scenarios.rs docs/model/parameters.csv docs/model/assumptions.md docs/validation/baseline
git commit -m "fix(sim): finite irradiation history and converged full-power initialisation

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 3: Startup and power-change operator

**Files:**
- Modify: `crates/bwr-core/src/validation.rs` (`operators::startup`, two cases in `standard_cases()`)
- Modify: `crates/bwr-core/tests/plant.rs` (`startup_from_hot_standby_to_30_percent` reuses the operator)
- Modify: `docs/validation/cases.csv`
- Test: `crates/bwr-core/tests/scenarios.rs`

**Interfaces:**
- Consumes: `irm_full_scale`, `Sim.irm_range`, `Rods.pos`, `Rods.weights`, `Rods.motion: Vec<RodMotion>`, `Kinetics.inv_period`, `Turbine.breaker_closed/speed_rpm/phase_deg`, `Feedwater::running_pumps`.
- Produces: `operators::startup(first_frac, hold_s, target_frac)`; cases `hs_startup_to_30pct`, `hs_power_change_30_to_60`.

- [ ] **Step 1: Write the failing tests**

```rust
fn case(id: &str) -> Case {
    standard_cases().into_iter().find(|c| c.id == id).unwrap_or_else(|| panic!("no case {id}"))
}

#[test]
fn startup_reaches_30_percent_without_scram() {
    let c = case("hs_startup_to_30pct");
    let (trace, sim) = run_sim_with(Sim::new(c.scenario), &c, DT, operators::build(&c.operator));
    assert!(!sim.scrammed, "scram: {:?}", sim.scram_reason);
    assert!(sim.turbine.breaker_closed, "generator never synchronised");
    assert!(sim.core.kin.n > 0.29, "n {}", sim.core.kin.n);
    assert!(sim.snapshot().turbine.net_mwe > 40.0);
    assert!(trace.column("thermal_mw").last().unwrap() > &(0.29 * 870.0));
    assert!(sim.domain_errors.is_empty(), "{:?}", sim.domain_errors);
}

#[test]
fn power_change_30_to_60_percent() {
    let c = case("hs_power_change_30_to_60");
    let (trace, sim) = run_sim_with(Sim::new(c.scenario), &c, DT, operators::build(&c.operator));
    assert!(!sim.scrammed, "scram: {:?}", sim.scram_reason);
    assert!(sim.core.kin.n > 0.58, "n {}", sim.core.kin.n);
    let p = trace.column("power_frac");
    // Power never exceeds the 60 % target by more than 5 % of rated during the manoeuvre.
    assert!(p.iter().cloned().fold(0.0, f64::max) < 0.65);
    assert!(sim.domain_errors.is_empty());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --release -p bwr-core --test scenarios -- startup power_change`
Expected: panic `no case hs_startup_to_30pct`.

- [ ] **Step 3: Implement the operator**

Replace the stub `startup` in `validation::operators` with:

```rust
use crate::api::ModeSwitch;
use crate::bop::LevelControlMode;
use crate::params::{N_RODS, ROD_FULL_OUT};
use crate::reactor::RodMotion;
use crate::sim::irm_full_scale;

fn rods_moving(sim: &Sim) -> bool {
    sim.core.rods.motion.iter().any(|m| *m != RodMotion::Idle)
}

/// Scripted operator ported from tests/plant.rs::startup_from_hot_standby_to_30_percent.
pub fn startup(first_frac: f64, hold_s: f64, target_frac: f64) -> OperatorFn {
    let mut order: Vec<usize> = Vec::new();
    let mut ptr = 0usize;
    let mut stage = 0u8;
    let mut started = false;
    let mut hold_reached_at: Option<f64> = None;
    Box::new(move |sim: &Sim, t: f64| {
        let mut cmds = Vec::new();
        if !started {
            started = true;
            order = (0..N_RODS).collect();
            order.sort_by(|&a, &b| sim.core.rods.weights[b].partial_cmp(&sim.core.rods.weights[a]).unwrap());
            cmds.push(Command::SetLevelSetpoint { cm: -40.0 });
        }
        if sim.scrammed {
            return cmds;
        }
        let n = sim.core.kin.n;
        let lvl = sim.vessel.level_nr_cm();
        cmds.push(Command::SetRwcuReject { kgs: if lvl > 20.0 { 15.0 } else { 0.0 } });
        if n / irm_full_scale(sim.irm_range) > 0.8 && sim.irm_range < 10 {
            cmds.push(Command::SetIrmRange { range: sim.irm_range + 1 });
        }
        let goal = if stage < 2 {
            0.06
        } else {
            match hold_reached_at {
                None => {
                    if n >= first_frac {
                        hold_reached_at = Some(t);
                    }
                    first_frac
                }
                Some(t0) if t - t0 >= hold_s => target_frac,
                Some(_) => first_frac,
            }
        };
        let period = 1.0 / sim.core.kin.inv_period;
        let slow = period > 0.0 && period < 150.0;
        if n < goal && !slow && !rods_moving(sim) {
            for _ in 0..N_RODS {
                let i = order[ptr % N_RODS];
                let bank = ((ptr / N_RODS) as f64 + 1.0) * 8.0;
                ptr += 1;
                let p = sim.core.rods.pos[i];
                if p < bank.min(ROD_FULL_OUT) {
                    cmds.push(Command::RodSetTarget { indices: vec![i], target: p + 2.0 });
                    break;
                }
            }
        }
        match stage {
            0 if n > 0.035 && sim.vessel.eos.p > 6.9e6 => {
                cmds.push(Command::ResetTurbine);
                cmds.push(Command::SetSpeedSetpoint { rpm: 1800.0 });
                stage = 1;
            }
            1 if sim.turbine.breaker_closed => {
                cmds.push(Command::SetModeSwitch { mode: ModeSwitch::Run });
                stage = 2;
            }
            1 if (sim.turbine.speed_rpm - 1800.0).abs() < 5.0 && sim.turbine.phase_deg.abs() < 5.0 => {
                cmds.push(Command::CloseBreaker);
            }
            2 if n > 0.2 && sim.fw.running_pumps() < 2 => {
                cmds.push(Command::SetCondensatePump { index: 1, on: true });
                cmds.push(Command::SetFeedPump { index: 1, on: true });
                cmds.push(Command::SetLevelControl { mode: LevelControlMode::ThreeElement });
            }
            _ => {}
        }
        cmds
    })
}
```

Add to `standard_cases()`:

```rust
Case {
    id: "hs_startup_to_30pct",
    scenario: ScenarioName::HotStandby,
    duration_s: 3.0 * 3600.0,
    sample_dt_s: 10.0,
    commands: vec![],
    faults: vec![],
    operator: Operator::Startup { first_frac: 0.30, hold_s: 0.0, target_frac: 0.30 },
},
Case {
    id: "hs_power_change_30_to_60",
    scenario: ScenarioName::HotStandby,
    duration_s: 4.0 * 3600.0,
    sample_dt_s: 10.0,
    commands: vec![],
    faults: vec![],
    operator: Operator::Startup { first_frac: 0.30, hold_s: 600.0, target_frac: 0.60 },
},
```

In `tests/plant.rs`, replace the body of `startup_from_hot_standby_to_30_percent` with:

```rust
let c = bwr_core::validation::standard_cases().into_iter().find(|c| c.id == "hs_startup_to_30pct").unwrap();
let (_, sim) = bwr_core::validation::run_sim_with(Sim::new(c.scenario), &c, DT, bwr_core::validation::operators::build(&c.operator));
assert!(!sim.scrammed, "scram during startup: {:?}", sim.scram_reason);
assert!(sim.turbine.breaker_closed, "generator never synchronised");
assert!(sim.core.kin.n > 0.29);
assert!(sim.snapshot().turbine.net_mwe > 40.0);
```

Append to `docs/validation/cases.csv` (header is `id,scenario,purpose,reference,tolerance_class,role`; Task 5 adds the `role` column to existing rows):

```
hs_startup_to_30pct,hotStandby,"Normal startup: critical, heatup, roll, sync, 30 %",none available,physical,validation
hs_power_change_30_to_60,hotStandby,"Power increase 30 to 60 % by rod withdrawal with period guard",none available,physical,validation
```

- [ ] **Step 4: Run tests and write the two baselines**

Run: `cargo test --release -p bwr-core` → all pass.
Run: `cargo run --release -p bwr-core --example validation -- --write-missing` (flag added in Task 5; until then use `--write` only for the two new ids by temporarily filtering, or defer this step to Task 5 — note which in the commit).

- [ ] **Step 5: Commit**

```bash
git add crates/bwr-core/src/validation.rs crates/bwr-core/tests/plant.rs crates/bwr-core/tests/scenarios.rs docs/validation/cases.csv
git commit -m "feat(validation): startup and power-change operator cases

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 4: Controlled shutdown, mode-switch shutdown, manual scram, cooldown on ICs

**Files:**
- Modify: `crates/bwr-core/src/validation.rs` (`operators::controlled_shutdown`, `operators::cooldown_on_ic`, four cases)
- Modify: `docs/validation/cases.csv`
- Test: `crates/bwr-core/tests/scenarios.rs`

**Interfaces:**
- Consumes: `Turbine.p_elec`, `Vessel.heatup_rate` (degC/h, negative = cooldown), `IcTrain.cmd_open/available`, `SteamSystem.msiv_cmd`, `Sim.mode`.
- Produces: cases `fp_controlled_shutdown`, `fp_mode_switch_shutdown`, `fp_manual_scram`, `fp_cooldown_on_ic`.

- [ ] **Step 1: Write the failing tests**

```rust
use bwr_core::steam::sat_p;

#[test]
fn controlled_shutdown_ends_with_mode_switch_scram_and_all_rods_in() {
    let c = case("fp_controlled_shutdown");
    let (_, sim) = run_sim_with(Sim::new(c.scenario), &c, DT, operators::build(&c.operator));
    assert!(sim.scrammed);
    assert_eq!(sim.scram_reason.as_deref(), Some("Mode switch in SHUTDOWN"));
    assert!(sim.core.rods.pos.iter().all(|&p| p <= 0.01));
    assert!(!sim.turbine.breaker_closed);
    assert!(sim.domain_errors.is_empty(), "{:?}", sim.domain_errors);
}

#[test]
fn cooldown_on_isolation_condensers_reaches_the_ics_lower_limit() {
    let c = case("fp_cooldown_on_ic");
    let (trace, sim) = run_sim_with(Sim::new(c.scenario), &c, DT, operators::build(&c.operator));
    let t_final = sim.vessel.eos.t;
    assert!(t_final > 100.0 && t_final < 140.0, "final coolant temperature {t_final}");
    // Cooldown rate never exceeded the 55 degC/h administrative limit by more than the
    // throttling dead band (operator acts once per second on a 30 s filtered rate).
    let p = trace.column("pressure_pa");
    let t = trace.column("t");
    let mut worst = 0.0f64;
    for k in 1..p.len() {
        let dt_h = (t[k] - t[k - 1]) / 3600.0;
        let rate = (sat_p(p[k]).t - sat_p(p[k - 1]).t) / dt_h;
        worst = worst.min(rate);
    }
    assert!(worst > -70.0, "peak cooldown rate {worst} degC/h");
    assert!(sim.domain_errors.is_empty(), "{:?}", sim.domain_errors);
    // ICs cannot cool below their pool temperature: heat removal has collapsed at the end.
    assert!(sim.ic.iter().map(|t| t.heat).sum::<f64>() < 3.0e6);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --release -p bwr-core --test scenarios -- controlled cooldown`
Expected: panic `no case fp_controlled_shutdown`.

- [ ] **Step 3: Implement**

Replace the two stubs in `validation::operators`:

```rust
/// Rods in by notches (lowest-worth first), leave RUN below the APRM setdown, trip the
/// turbine at low load, then mode switch to SHUTDOWN once power is on decay heat.
pub fn controlled_shutdown() -> OperatorFn {
    let mut order: Vec<usize> = Vec::new();
    let mut started = false;
    let mut ptr = 0usize;
    let mut turbine_off = false;
    Box::new(move |sim: &Sim, _t: f64| {
        let mut cmds = Vec::new();
        if !started {
            started = true;
            order = (0..N_RODS).collect();
            order.sort_by(|&a, &b| sim.core.rods.weights[a].partial_cmp(&sim.core.rods.weights[b]).unwrap());
        }
        if sim.scrammed {
            return cmds;
        }
        let n = sim.core.kin.n;
        if sim.mode == ModeSwitch::Run && n < 0.12 {
            cmds.push(Command::SetModeSwitch { mode: ModeSwitch::Startup });
        }
        if !turbine_off && sim.turbine.breaker_closed && sim.turbine.p_elec < 10.0e6 {
            cmds.push(Command::OpenBreaker);
            cmds.push(Command::TripTurbine);
            cmds.push(Command::SetLevelControl { mode: LevelControlMode::SingleElement });
            turbine_off = true;
        }
        if n < 0.01 && sim.mode == ModeSwitch::Startup && sim.core.rods.pos.iter().all(|&p| p <= 8.0) {
            cmds.push(Command::SetModeSwitch { mode: ModeSwitch::Shutdown });
            return cmds;
        }
        if !rods_moving(sim) {
            for _ in 0..N_RODS {
                let i = order[ptr % N_RODS];
                let bank = ROD_FULL_OUT - ((ptr / N_RODS) as f64 + 1.0) * 8.0;
                ptr += 1;
                let p = sim.core.rods.pos[i];
                if p > bank.max(0.0) {
                    cmds.push(Command::RodSetTarget { indices: vec![i], target: p - 2.0 });
                    break;
                }
            }
        }
        cmds
    })
}

/// After scram and isolation, throttle IC trains to hold the cooldown between 30 and 50 degC/h.
pub fn cooldown_on_ic() -> OperatorFn {
    Box::new(move |sim: &Sim, t: f64| {
        let mut cmds = Vec::new();
        if t < 60.0 {
            return cmds;
        }
        if sim.steam.msiv_cmd.iter().any(|&c| c) {
            cmds.push(Command::SetMsiv { line: 0, open: false });
            cmds.push(Command::SetMsiv { line: 1, open: false });
            return cmds;
        }
        let open: Vec<usize> = (0..sim.ic.len()).filter(|&i| sim.ic[i].cmd_open).collect();
        let closed: Vec<usize> = (0..sim.ic.len()).filter(|&i| !sim.ic[i].cmd_open && sim.ic[i].available).collect();
        let rate = sim.vessel.heatup_rate;
        if rate < -50.0 && open.len() > 1 {
            cmds.push(Command::SetIcTrain { index: open[open.len() - 1], open: false });
        } else if rate > -30.0 && !closed.is_empty() && sim.vessel.eos.t > 105.0 {
            cmds.push(Command::SetIcTrain { index: closed[0], open: true });
        }
        cmds
    })
}
```

Add to `standard_cases()`:

```rust
Case { id: "fp_controlled_shutdown", scenario: ScenarioName::FullPower, duration_s: 3.0 * 3600.0, sample_dt_s: 10.0,
       commands: vec![], faults: vec![], operator: Operator::ControlledShutdown },
Case { id: "fp_mode_switch_shutdown", scenario: ScenarioName::FullPower, duration_s: 600.0, sample_dt_s: 1.0,
       commands: vec![ScriptedCommand { at_s: 5.0, cmd: Command::SetModeSwitch { mode: ModeSwitch::Shutdown } }],
       faults: vec![], operator: Operator::None },
Case { id: "fp_manual_scram", scenario: ScenarioName::FullPower, duration_s: 600.0, sample_dt_s: 1.0,
       commands: vec![
           ScriptedCommand { at_s: 5.0, cmd: Command::ManualScram },
           ScriptedCommand { at_s: 6.0, cmd: Command::SetModeSwitch { mode: ModeSwitch::Shutdown } },
       ],
       faults: vec![], operator: Operator::None },
Case { id: "fp_cooldown_on_ic", scenario: ScenarioName::FullPower, duration_s: 8.0 * 3600.0, sample_dt_s: 10.0,
       commands: vec![
           ScriptedCommand { at_s: 5.0, cmd: Command::ManualScram },
           ScriptedCommand { at_s: 6.0, cmd: Command::SetModeSwitch { mode: ModeSwitch::Shutdown } },
       ],
       faults: vec![], operator: Operator::CooldownOnIc },
```

`cases.csv` rows:

```
fp_controlled_shutdown,fullPower,"Rods in by notches, turbine off, mode switch to SHUTDOWN",none available,physical,validation
fp_mode_switch_shutdown,fullPower,"Mode switch SHUTDOWN scram from full power",none available,physical,validation
fp_manual_scram,fullPower,"Manual scram, no isolation, level stays above L2",none available,physical,validation
fp_cooldown_on_ic,fullPower,"Isolated cooldown on ICs throttled to 30-50 degC/h until the IC pool-temperature limit",none available,plausibility,plausibility
```

- [ ] **Step 4: Run tests**

Run: `cargo test --release -p bwr-core --test scenarios`
Expected: pass. If `cooldown_on_isolation_condensers_reaches_the_ics_lower_limit` fails on the final temperature, apply the finding policy (do not retune).

- [ ] **Step 5: Commit**

```bash
git add crates/bwr-core/src/validation.rs crates/bwr-core/tests/scenarios.rs docs/validation/cases.csv
git commit -m "feat(validation): shutdown, scram and IC cooldown operator cases

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 5: Full transient case register, `role` column, `--write-missing`

**Files:**
- Modify: `crates/bwr-core/src/validation.rs` (`standard_cases()`, `operators::rod_withdrawal_error`)
- Modify: `crates/bwr-core/examples/validation.rs` (`--write-missing`)
- Modify: `docs/validation/cases.csv`, `docs/validation/method.md` (role definitions)
- Create: `docs/validation/baseline/<id>.csv` for every new id

**Interfaces:**
- Consumes: `Fault` variants, `Rods::strongest_rod`.
- Produces: every case id listed in the table below; `--write-missing`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn every_planned_case_is_registered() {
    let ids: std::collections::BTreeSet<&str> = standard_cases().iter().map(|c| c.id).collect();
    for id in [
        "fp_steady_600", "hs_steady_600", "cs_steady_600",
        "hs_startup_to_30pct", "hs_power_change_30_to_60", "fp_controlled_shutdown", "fp_mode_switch_shutdown",
        "fp_manual_scram", "fp_cooldown_on_ic",
        "fp_turbine_trip", "fp_turbine_trip_no_bypass", "fp_load_reject", "fp_load_reject_no_bypass",
        "fp_msiv_single", "fp_msiv_both", "fp_pressure_reg_fail_closed", "fp_pressure_reg_fail_open",
        "fp_excess_steam_demand", "fp_feed_pump_trip_one", "fp_feed_pump_trip_all", "fp_condensate_pump_loss",
        "fp_fcv_stuck", "fp_heater_loss", "fp_high_level", "fp_low_level", "fp_cw_loss", "fp_vacuum_pump_fail",
        "fp_rod_withdrawal_error", "fp_scram_stuck_rod", "fp_msiv_both_ic1", "fp_msiv_both_ic2", "fp_msiv_both_ic3",
        "fp_inadvertent_ic", "fp_feed_loss_no_ic",
    ] {
        assert!(ids.contains(id), "case {id} missing from standard_cases()");
    }
    let csv = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/validation/cases.csv")).unwrap();
    for id in &ids {
        assert!(csv.lines().any(|l| l.starts_with(&format!("{id},"))), "{id} missing from cases.csv");
    }
    let header = csv.lines().next().unwrap();
    assert!(header.ends_with(",role"), "cases.csv header must end with role: {header}");
    for line in csv.lines().skip(1).filter(|l| !l.is_empty()) {
        let role = line.rsplit(',').next().unwrap();
        assert!(["calibration", "validation", "plausibility"].contains(&role), "bad role in: {line}");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --release -p bwr-core --test scenarios -- registered`
Expected: FAIL `case fp_turbine_trip_no_bypass missing`.

- [ ] **Step 3: Implement the operator and the cases**

Replace the `rod_withdrawal_error` stub:

```rust
/// Continuous withdrawal of the strongest rod from full power until blocked or scrammed.
pub fn rod_withdrawal_error() -> OperatorFn {
    Box::new(move |sim: &Sim, t: f64| {
        if t < 5.0 || sim.scrammed {
            return vec![];
        }
        let i = sim.core.rods.strongest_rod();
        if sim.core.rods.pos[i] >= ROD_FULL_OUT {
            return vec![];
        }
        vec![Command::RodSetTarget { indices: vec![i], target: ROD_FULL_OUT }]
    })
}
```

Add a local helper at the top of `standard_cases()` and the cases below (keep P2's existing `fp_steady_600`, `hs_steady_600`, `cs_steady_600`, `fp_turbine_trip` as they are, adding `operator: Operator::None`):

```rust
fn fp(id: &'static str, duration_s: f64, commands: Vec<ScriptedCommand>, faults: Vec<ScriptedFault>) -> Case {
    Case { id, scenario: ScenarioName::FullPower, duration_s, sample_dt_s: 1.0, commands, faults, operator: Operator::None }
}
fn at(at_s: f64, cmd: Command) -> ScriptedCommand { ScriptedCommand { at_s, cmd } }
fn fault(at_s: f64, fault: Fault) -> ScriptedFault { ScriptedFault { at_s, fault, active: true } }
use crate::faults::Fault;

// ... inside the Vec returned by standard_cases():
fp("fp_turbine_trip_no_bypass", 600.0, vec![at(5.0, Command::TripTurbine)], vec![fault(0.0, Fault::BypassUnavailable)]),
fp("fp_load_reject", 300.0, vec![at(5.0, Command::OpenBreaker)], vec![]),
fp("fp_load_reject_no_bypass", 600.0, vec![at(5.0, Command::OpenBreaker)], vec![fault(0.0, Fault::BypassUnavailable)]),
fp("fp_msiv_single", 600.0, vec![at(5.0, Command::SetMsiv { line: 0, open: false })], vec![]),
fp("fp_msiv_both", 1800.0, vec![at(5.0, Command::SetMsiv { line: 0, open: false }), at(5.0, Command::SetMsiv { line: 1, open: false })], vec![]),
fp("fp_pressure_reg_fail_closed", 600.0, vec![], vec![fault(5.0, Fault::PressureRegulatorFailed { demand_pct: 0 })]),
fp("fp_pressure_reg_fail_open", 600.0, vec![], vec![fault(5.0, Fault::PressureRegulatorFailed { demand_pct: 130 })]),
fp("fp_excess_steam_demand", 600.0, vec![at(5.0, Command::SetPressureSetpoint { mpa: 6.5 })], vec![]),
fp("fp_feed_pump_trip_one", 600.0, vec![], vec![fault(5.0, Fault::FeedPumpTrip { index: 0 })]),
fp("fp_feed_pump_trip_all", 1800.0, vec![], vec![fault(5.0, Fault::FeedPumpTrip { index: 0 }), fault(5.0, Fault::FeedPumpTrip { index: 1 }), fault(5.0, Fault::FeedPumpTrip { index: 2 })]),
fp("fp_condensate_pump_loss", 1800.0, vec![], vec![fault(5.0, Fault::CondensatePumpTrip { index: 0 }), fault(5.0, Fault::CondensatePumpTrip { index: 1 })]),
fp("fp_fcv_stuck", 900.0, vec![at(60.0, Command::TripTurbine)], vec![fault(5.0, Fault::FcvStuck)]),
fp("fp_heater_loss", 900.0, vec![], vec![fault(5.0, Fault::HeatersLost)]),
fp("fp_high_level", 900.0, vec![at(5.0, Command::SetLevelControl { mode: LevelControlMode::Manual }), at(6.0, Command::SetFcvManual { pos: 1.0 })], vec![]),
fp("fp_low_level", 1800.0, vec![at(5.0, Command::SetLevelControl { mode: LevelControlMode::Manual }), at(6.0, Command::SetFcvManual { pos: 0.0 })], vec![]),
fp("fp_cw_loss", 900.0, vec![], vec![fault(5.0, Fault::CwPumpTrip { index: 0 }), fault(5.0, Fault::CwPumpTrip { index: 1 })]),
fp("fp_vacuum_pump_fail", 2.0 * 3600.0, vec![], vec![fault(5.0, Fault::VacuumPumpFailed)]),
Case { id: "fp_rod_withdrawal_error", scenario: ScenarioName::FullPower, duration_s: 300.0, sample_dt_s: 1.0,
       commands: vec![], faults: vec![], operator: Operator::RodWithdrawalError },
fp("fp_scram_stuck_rod", 120.0, vec![at(5.0, Command::ManualScram)], vec![]),  // RodStuck index set in Step 4
fp("fp_msiv_both_ic1", 3600.0, vec![at(5.0, Command::SetMsiv { line: 0, open: false }), at(5.0, Command::SetMsiv { line: 1, open: false })],
   vec![fault(0.0, Fault::IcTrainUnavailable { train: 1 }), fault(0.0, Fault::IcTrainUnavailable { train: 2 })]),
fp("fp_msiv_both_ic2", 3600.0, vec![at(5.0, Command::SetMsiv { line: 0, open: false }), at(5.0, Command::SetMsiv { line: 1, open: false })],
   vec![fault(0.0, Fault::IcTrainUnavailable { train: 2 })]),
fp("fp_msiv_both_ic3", 3600.0, vec![at(5.0, Command::SetMsiv { line: 0, open: false }), at(5.0, Command::SetMsiv { line: 1, open: false })], vec![]),
fp("fp_inadvertent_ic", 600.0, vec![at(5.0, Command::SetIcTrain { index: 0, open: true })], vec![]),
fp("fp_feed_loss_no_ic", 3600.0, vec![],
   vec![fault(5.0, Fault::FeedPumpTrip { index: 0 }), fault(5.0, Fault::FeedPumpTrip { index: 1 }), fault(5.0, Fault::FeedPumpTrip { index: 2 }),
        fault(0.0, Fault::IcTrainUnavailable { train: 0 }), fault(0.0, Fault::IcTrainUnavailable { train: 1 }), fault(0.0, Fault::IcTrainUnavailable { train: 2 })]),
```

- [ ] **Step 4: Stuck-rod index**

`Fault::RodStuck { index }` needs the strongest rod, which depends on the layout. The layout is deterministic, so compute it once inside `standard_cases()`:

```rust
let strongest = crate::reactor::Rods::new(0.0).strongest_rod();
// then in the fp_scram_stuck_rod literal:
fp("fp_scram_stuck_rod", 120.0, vec![at(5.0, Command::ManualScram)], vec![fault(0.0, Fault::RodStuck { index: strongest })]),
```

- [ ] **Step 5: `--write-missing` in `examples/validation.rs`**

Replace P2's `--write` boolean with:

```rust
#[derive(Clone, Copy, PartialEq)]
enum Mode { Check, WriteAll, WriteMissing }

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let mode = if args.iter().any(|a| a == "--write") { Mode::WriteAll }
        else if args.iter().any(|a| a == "--write-missing") { Mode::WriteMissing }
        else { Mode::Check };
    let mut failures = Vec::new();
    for case in standard_cases() {
        let path = format!("{}/../../docs/validation/baseline/{}.csv", env!("CARGO_MANIFEST_DIR"), case.id);
        let trace = run_case(&case, DT);
        let exists = std::path::Path::new(&path).exists();
        match (mode, exists) {
            (Mode::WriteAll, _) | (Mode::WriteMissing, false) => {
                std::fs::write(&path, trace.to_csv()).map_err(|e| e.to_string())?;
                println!("wrote {path}");
            }
            (_, false) => failures.push(format!("{}: no baseline (run with --write-missing)", case.id)),
            (_, true) => {
                let reference = Trace::from_csv(case.id, &std::fs::read_to_string(&path).map_err(|e| e.to_string())?)?;
                let v = compare(&reference.metrics(), &trace.metrics(), &Tolerance::numerical());
                if v.is_empty() { println!("ok    {}", case.id); } else { failures.push(format!("{}: {}", case.id, v.join("; "))); }
            }
        }
    }
    // Task 9 inserts the reference comparison here.
    if failures.is_empty() { Ok(()) } else { Err(failures.join("\n")) }
}
```

Keep whatever P2 printed per case beyond this (metrics summary) inside the `(_, true)` arm.

- [ ] **Step 6: `cases.csv` and `method.md`**

Rewrite `docs/validation/cases.csv` so every row ends with a role. Final content (P2 rows keep their original purpose text; only the role is appended):

```
id,scenario,purpose,reference,tolerance_class,role
fp_steady_600,fullPower,"Rated steady state: initial equilibrium residuals and drift",none available,numerical,calibration
hs_steady_600,hotStandby,"Hot standby steady state",none available,numerical,calibration
cs_steady_600,coldShutdown,"Cold shutdown steady state, vented",none available,numerical,calibration
hs_startup_to_30pct,hotStandby,"Normal startup: critical, heatup, roll, sync, 30 %",none available,physical,validation
hs_power_change_30_to_60,hotStandby,"Power increase 30 to 60 % by rod withdrawal with period guard",none available,physical,validation
fp_controlled_shutdown,fullPower,"Rods in by notches, turbine off, mode switch to SHUTDOWN",none available,physical,validation
fp_mode_switch_shutdown,fullPower,"Mode switch SHUTDOWN scram from full power",none available,physical,validation
fp_manual_scram,fullPower,"Manual scram, no isolation, level stays above L2",none available,physical,validation
fp_cooldown_on_ic,fullPower,"Isolated cooldown on ICs throttled to 30-50 degC/h until the IC pool-temperature limit",none available,plausibility,plausibility
fp_turbine_trip,fullPower,"Turbine trip with bypass: SRI ride-through",ESBWR DCD Tier 2 15.2 turbine trip (mechanism only; see Task 9),physical,validation
fp_turbine_trip_no_bypass,fullPower,"Turbine trip, bypass unavailable: high pressure scram and IC initiation",none available,physical,validation
fp_load_reject,fullPower,"Generator load rejection with bypass",ESBWR DCD Tier 2 15.2 load rejection (mechanism only; see Task 9),physical,validation
fp_load_reject_no_bypass,fullPower,"Load rejection, bypass unavailable",none available,physical,validation
fp_msiv_single,fullPower,"Closure of one of two main steam lines",none available,physical,validation
fp_msiv_both,fullPower,"Closure of both main steam lines: scram, IC auto-initiation, pressure below design",ESBWR DCD Tier 2 15.2 MSIV closure (mechanism only; see Task 9),physical,validation
fp_pressure_reg_fail_closed,fullPower,"Pressure regulator output fails to zero demand",none available,physical,validation
fp_pressure_reg_fail_open,fullPower,"Pressure regulator output fails to 130 % demand",none available,physical,validation
fp_excess_steam_demand,fullPower,"Setpoint step to 6.5 MPa: high steam flow isolation",none available,physical,validation
fp_feed_pump_trip_one,fullPower,"Trip of one of two running feed pumps",none available,physical,validation
fp_feed_pump_trip_all,fullPower,"Loss of all feedwater: L3 scram, L2 isolation, ICs",none available,physical,validation
fp_condensate_pump_loss,fullPower,"Loss of both condensate pumps: feed pump low-suction trip",none available,physical,validation
fp_fcv_stuck,fullPower,"Feed control valve stuck then turbine trip: high level response",none available,physical,validation
fp_heater_loss,fullPower,"Loss of feedwater heating: power rise without scram",ESBWR DCD Tier 2 15.2 LOFWH (mechanism only; see Task 9),physical,validation
fp_high_level,fullPower,"Manual overfeed to L8: turbine and feed pump trips",none available,physical,validation
fp_low_level,fullPower,"Manual underfeed to L3/L2: scram, isolation, ICs",none available,physical,validation
fp_cw_loss,fullPower,"Loss of circulating water: vacuum loss, turbine trip, bypass block",none available,physical,validation
fp_vacuum_pump_fail,fullPower,"Vacuum pump failure: slow condenser pressure rise",none available,plausibility,plausibility
fp_rod_withdrawal_error,fullPower,"Continuous withdrawal of the strongest rod at power",none available,physical,validation
fp_scram_stuck_rod,fullPower,"Manual scram with the strongest rod stuck",none available,physical,validation
fp_msiv_both_ic1,fullPower,"Isolation with one IC train available (record only)",none available,plausibility,plausibility
fp_msiv_both_ic2,fullPower,"Isolation with two IC trains available",none available,physical,validation
fp_msiv_both_ic3,fullPower,"Isolation with three IC trains available",none available,physical,validation
fp_inadvertent_ic,fullPower,"Inadvertent initiation of one IC train at power",none available,plausibility,plausibility
fp_feed_loss_no_ic,fullPower,"Loss of feedwater with no IC credited: deliberately leaves the model envelope",none available,plausibility,plausibility
```

Append to `docs/validation/method.md`:

```
## Case roles

- calibration: used to fix `k_fric`, `TURBINE_DH_RATED`, and the critical rod configuration at initialisation. Never cited as validation evidence.
- validation: independent of calibration; compared to a public reference where one exists, otherwise to ordering, bounds and conservation.
- plausibility: no public reference and no bound asserted; the trace is recorded and reported.
```

- [ ] **Step 7: Run and write baselines**

Run: `cargo test --release -p bwr-core --test scenarios -- registered` → PASS.
Run: `cargo run --release -p bwr-core --example validation -- --write-missing`
Expected: `wrote .../<id>.csv` for each new id, `ok` for every pre-existing id. Then `cargo run --release -p bwr-core --example validation` → all `ok`, exit 0.

- [ ] **Step 8: Commit**

```bash
git add crates/bwr-core/src/validation.rs crates/bwr-core/examples/validation.rs crates/bwr-core/tests/scenarios.rs docs/validation
git commit -m "feat(validation): full transient case register with roles and --write-missing

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 6: Expected-behaviour table: actuation order, extrema, support envelope

**Files:**
- Test: `crates/bwr-core/tests/scenarios.rs`
- Modify: `docs/validation/findings.md` (create; only if a physical assertion fails)

**Interfaces:**
- Consumes: `Trace.events`, `Metrics.peak/min`, `Sim.domain_errors`, `VESSEL_DESIGN_PRESSURE`, `L1_CM`, `L2_CM`.
- Produces: `event_order`, `mod ev`.

- [ ] **Step 1: Set the event prefixes**

Run: `grep -n "log.push" crates/bwr-core/src/sim.rs` and copy the literal prefixes into:

```rust
/// Event-string prefixes as pushed to `Sim.log`. The only place event literals may appear.
mod ev {
    pub const SCRAM: &str = "SCRAM: ";
    pub const ISOLATION: &str = "MSIV isolation: ";
    pub const IC_INIT: &str = "Isolation condensers auto-initiated";
    pub const TURBINE_TRIP: &str = "Turbine trip: ";
    pub const FW_TRIP: &str = "Feed pump";
    pub const DOMAIN: &str = "Domain error: ";
}
```

- [ ] **Step 2: Write the helper and the table test**

```rust
use bwr_core::validation::Trace;

/// Each prefix must appear in `trace.events` no earlier than the previous one.
fn event_order(trace: &Trace, prefixes: &[&str]) -> Result<(), String> {
    let mut last_t = f64::NEG_INFINITY;
    for p in prefixes {
        let t = trace
            .events
            .iter()
            .find(|(t, s)| *t >= last_t && s.starts_with(p))
            .map(|(t, _)| *t)
            .ok_or_else(|| format!("event '{p}' not found after t={last_t:.2}; events: {:?}", trace.events))?;
        last_t = t;
    }
    Ok(())
}

fn has_event(trace: &Trace, prefix: &str) -> bool {
    trace.events.iter().any(|(_, s)| s.starts_with(prefix))
}

struct Expect {
    id: &'static str,
    order: &'static [&'static str],
    no_scram: bool,
    pressure_below_design: bool,
    supported: bool,
    /// Allowed DomainError variant names when !supported (checked with `format!("{:?}")`).
    expect_domain: &'static [&'static str],
}

const EXPECTATIONS: &[Expect] = &[
    Expect { id: "fp_turbine_trip", order: &[ev::TURBINE_TRIP], no_scram: true, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_turbine_trip_no_bypass", order: &[ev::TURBINE_TRIP, ev::SCRAM, ev::IC_INIT], no_scram: false, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_load_reject", order: &[], no_scram: true, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_load_reject_no_bypass", order: &[ev::SCRAM], no_scram: false, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_msiv_single", order: &[ev::SCRAM], no_scram: false, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_msiv_both", order: &[ev::SCRAM, ev::IC_INIT], no_scram: false, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_pressure_reg_fail_closed", order: &[ev::SCRAM, ev::IC_INIT], no_scram: false, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_pressure_reg_fail_open", order: &[ev::ISOLATION, ev::SCRAM], no_scram: false, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_excess_steam_demand", order: &[ev::ISOLATION, ev::SCRAM], no_scram: false, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_feed_pump_trip_one", order: &[], no_scram: false, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_feed_pump_trip_all", order: &[ev::SCRAM, ev::ISOLATION, ev::IC_INIT], no_scram: false, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_condensate_pump_loss", order: &[ev::FW_TRIP, ev::SCRAM, ev::ISOLATION, ev::IC_INIT], no_scram: false, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_fcv_stuck", order: &[ev::TURBINE_TRIP, ev::FW_TRIP], no_scram: false, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_heater_loss", order: &[], no_scram: true, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_high_level", order: &[ev::TURBINE_TRIP, ev::FW_TRIP], no_scram: false, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_low_level", order: &[ev::SCRAM, ev::ISOLATION, ev::IC_INIT], no_scram: false, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_cw_loss", order: &[ev::TURBINE_TRIP, ev::SCRAM, ev::IC_INIT], no_scram: false, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_vacuum_pump_fail", order: &[], no_scram: true, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_rod_withdrawal_error", order: &[], no_scram: false, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_scram_stuck_rod", order: &[ev::SCRAM], no_scram: false, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_msiv_both_ic1", order: &[ev::SCRAM, ev::IC_INIT], no_scram: false, pressure_below_design: false, supported: true, expect_domain: &[] },
    Expect { id: "fp_msiv_both_ic2", order: &[ev::SCRAM, ev::IC_INIT], no_scram: false, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_msiv_both_ic3", order: &[ev::SCRAM, ev::IC_INIT], no_scram: false, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_inadvertent_ic", order: &[], no_scram: true, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_manual_scram", order: &[ev::SCRAM], no_scram: false, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_mode_switch_shutdown", order: &[ev::SCRAM], no_scram: false, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_feed_loss_no_ic", order: &[ev::SCRAM, ev::ISOLATION], no_scram: false, pressure_below_design: false, supported: false,
             expect_domain: &["EosOutOfRange", "InventoryExhausted", "FuelModelEnvelope"] },
];

#[test]
fn transient_expectations() {
    let mut failures = Vec::new();
    for e in EXPECTATIONS {
        let c = case(e.id);
        let (trace, sim) = run_sim_with(Sim::new(c.scenario), &c, DT, operators::build(&c.operator));
        let m = trace.metrics();
        let mut errs = Vec::new();
        if let Err(s) = event_order(&trace, e.order) { errs.push(s); }
        if e.no_scram && has_event(&trace, ev::SCRAM) { errs.push(format!("unexpected scram: {:?}", sim.scram_reason)); }
        let p_max = m.peak["pressure_pa"].0;
        if e.pressure_below_design && p_max >= VESSEL_DESIGN_PRESSURE {
            errs.push(format!("peak pressure {:.3} MPa >= design", p_max / 1e6));
        }
        if e.supported && !sim.domain_errors.is_empty() {
            errs.push(format!("unsupported state: {:?}", sim.domain_errors));
        }
        if !e.supported {
            match sim.domain_errors.first() {
                None => errs.push("expected a domain error, none raised".into()),
                Some((t, d)) => {
                    let name = format!("{d:?}");
                    if !e.expect_domain.iter().any(|v| name.starts_with(v)) {
                        errs.push(format!("domain error {name} at {t:.1} s not in {:?}", e.expect_domain));
                    }
                }
            }
        }
        println!("{:28} peak p {:.3} MPa  min level {:.0} cm  events {}", e.id, p_max / 1e6, m.min["level_nr_cm"].0, trace.events.len());
        if !errs.is_empty() { failures.push(format!("{}: {}", e.id, errs.join(" | "))); }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

#[test]
fn manual_scram_keeps_level_above_l2_and_ics_closed() {
    let c = case("fp_manual_scram");
    let (trace, sim) = run_sim_with(Sim::new(c.scenario), &c, DT, operators::build(&c.operator));
    assert!(trace.metrics().min["level_nr_cm"].0 > L2_CM);
    assert!(!has_event(&trace, ev::ISOLATION) && !has_event(&trace, ev::IC_INIT));
    assert!(sim.core.rods.pos.iter().all(|&p| p <= 0.01));
}

#[test]
fn stuck_rod_scram_reports_shutdown_margin() {
    let c = case("fp_scram_stuck_rod");
    let (_, sim) = run_sim_with(Sim::new(c.scenario), &c, DT, operators::build(&c.operator));
    let i = sim.core.rods.strongest_rod();
    let p0 = Sim::new(ScenarioName::FullPower).core.rods.pos[i];
    assert!((sim.core.rods.pos[i] - p0).abs() < 1e-9, "stuck rod moved from {p0} to {}", sim.core.rods.pos[i]);
    assert!(sim.core.rods.pos.iter().enumerate().all(|(k, &p)| k == i || p <= 0.01));
    assert!(sim.core.kin.n < 0.02);
    let rho_other = sim.core.rho_doppler + sim.core.rho_void + sim.core.rho_mod + sim.core.rho_xenon + RHO_EXCESS_COLD;
    let sdm = sim.core.rods.shutdown_margin(rho_other, &[i]);
    println!("shutdown margin with strongest rod out: {:.4} dk/k", sdm);
    assert!(sdm > 0.0, "core not subcritical with strongest rod withdrawn: {sdm}");
}
```

- [ ] **Step 3: Run**

Run: `cargo test --release -p bwr-core --test scenarios -- transient_expectations manual_scram stuck_rod`
Expected: PASS. For every failing physical assertion apply the finding policy: create `docs/validation/findings.md` with the header `# Validation findings` and one entry per finding:

```
## F-001 fp_turbine_trip_no_bypass: peak pressure 8.71 MPa >= design
Observed: ... Suspected cause: ... Package to revisit: 6/7. Test: transient_expectations (row ignored by moving it to `IGNORED_EXPECTATIONS` with a comment `// F-001`).
```

For table rows, "ignoring" means moving the row to a second `const IGNORED_EXPECTATIONS: &[Expect]` that the test prints but does not assert; never delete rows.

- [ ] **Step 4: Commit**

```bash
git add crates/bwr-core/tests/scenarios.rs docs/validation/findings.md
git commit -m "test(scenarios): actuation order, pressure bounds and support envelope per case

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 7: Fault overrides persist and are honoured for every variant

**Files:**
- Test: `crates/bwr-core/tests/scenarios.rs`

**Interfaces:**
- Consumes: all 16 `Fault` variants; `SteamSystem.{msiv_pos,tsv_pos,tcv_pos,bypass_pos,w_tcv,w_bypass}`, `Feedwater.{pumps,cond_pumps,fcv_pos,heaters_in_service}`, `Condenser.{cw_pumps,vacuum_pump}`, `IcTrain.{heat,in_service,return_valve_pos,steam_valve_pos,ua_factor}`, `Rods.pos`.

- [ ] **Step 1: Write the table-driven test**

```rust
use bwr_core::faults::Fault;

type Check = Box<dyn Fn(&Sim, &Sim) -> Result<(), String>>;

fn ok_if(cond: bool, msg: String) -> Result<(), String> { if cond { Ok(()) } else { Err(msg) } }

/// (fault, commands that automatic logic or the operator would use to undo it, check(initial, final))
fn fault_table() -> Vec<(Fault, Vec<ScriptedCommand>, Check)> {
    let strongest = Sim::new(ScenarioName::FullPower).core.rods.strongest_rod();
    let at = |at_s: f64, cmd: Command| ScriptedCommand { at_s, cmd };
    vec![
        (Fault::MsivStuck { line: 0 }, vec![at(5.0, Command::SetMsiv { line: 0, open: false })],
         Box::new(|_, s| ok_if(s.steam.msiv_pos[0] > 0.99, format!("MSIV 0 moved to {}", s.steam.msiv_pos[0])))),
        (Fault::TsvStuck, vec![at(5.0, Command::TripTurbine)],
         Box::new(|_, s| ok_if(s.steam.tsv_pos > 0.99, format!("TSV moved to {}", s.steam.tsv_pos)))),
        (Fault::TcvStuck, vec![at(5.0, Command::TripTurbine)],
         Box::new(|s0, s| ok_if((s.steam.tcv_pos - s0.steam.tcv_pos).abs() < 1e-6, format!("TCV moved {} -> {}", s0.steam.tcv_pos, s.steam.tcv_pos)))),
        (Fault::BypassUnavailable, vec![at(5.0, Command::TripTurbine)],
         Box::new(|_, s| ok_if(s.steam.bypass_pos < 1e-6, format!("bypass opened to {}", s.steam.bypass_pos)))),
        (Fault::PressureRegulatorFailed { demand_pct: 50 }, vec![],
         Box::new(|_, s| ok_if((s.steam.total_flow() - 0.5 * STEAM_FLOW_RATED).abs() < 0.15 * STEAM_FLOW_RATED,
                               format!("total steam flow {} not held near 50 %", s.steam.total_flow())))),
        (Fault::FeedPumpTrip { index: 0 }, vec![at(5.0, Command::SetFeedPump { index: 0, on: true })],
         Box::new(|_, s| ok_if(!s.fw.pumps[0], "feed pump 0 restarted".into()))),
        (Fault::CondensatePumpTrip { index: 0 }, vec![at(5.0, Command::SetCondensatePump { index: 0, on: true })],
         Box::new(|_, s| ok_if(!s.fw.cond_pumps[0], "condensate pump 0 restarted".into()))),
        (Fault::FcvStuck, vec![at(5.0, Command::SetLevelControl { mode: LevelControlMode::Manual }), at(6.0, Command::SetFcvManual { pos: 0.0 })],
         Box::new(|s0, s| ok_if((s.fw.fcv_pos - s0.fw.fcv_pos).abs() < 1e-6, format!("FCV moved {} -> {}", s0.fw.fcv_pos, s.fw.fcv_pos)))),
        (Fault::HeatersLost, vec![at(5.0, Command::SetHeaters { in_service: true })],
         Box::new(|_, s| ok_if(!s.fw.heaters_in_service, "heaters back in service".into()))),
        (Fault::CwPumpTrip { index: 0 }, vec![at(5.0, Command::SetCwPump { index: 0, on: true })],
         Box::new(|_, s| ok_if(!s.cond.cw_pumps[0], "CW pump 0 restarted".into()))),
        (Fault::VacuumPumpFailed, vec![at(5.0, Command::SetVacuumPump { on: true })],
         Box::new(|_, s| ok_if(!s.cond.vacuum_pump, "vacuum pump restarted".into()))),
        (Fault::IcTrainUnavailable { train: 0 }, vec![at(5.0, Command::SetIcTrain { index: 0, open: true })],
         Box::new(|_, s| ok_if(!s.ic[0].in_service() && s.ic[0].heat < 1.0, format!("IC 0 in service, heat {}", s.ic[0].heat)))),
        (Fault::IcReturnValveStuck { train: 0, valve: 0 }, vec![at(5.0, Command::SetIcTrain { index: 0, open: true })],
         Box::new(|_, s| ok_if(s.ic[0].return_valve_pos[0] < 1e-6 && s.ic[0].return_valve_pos[1] > 0.99,
                               format!("return valves {:?}", s.ic[0].return_valve_pos)))),
        (Fault::IcSteamValveClosed { train: 0 }, vec![at(5.0, Command::SetIcTrain { index: 0, open: true })],
         Box::new(|_, s| ok_if(s.ic[0].steam_valve_pos < 1e-6 && s.ic[0].heat < 1.0e5, format!("steam valve {} heat {}", s.ic[0].steam_valve_pos, s.ic[0].heat)))),
        (Fault::RodStuck { index: strongest }, vec![at(5.0, Command::ManualScram)],
         Box::new(move |s0, s| ok_if((s.core.rods.pos[strongest] - s0.core.rods.pos[strongest]).abs() < 1e-9, "stuck rod moved".into()))),
        (Fault::IcNoncondensableDegradation { train: 0, pct: 50 },
         vec![at(5.0, Command::SetIcTrain { index: 0, open: true }), at(5.0, Command::SetIcTrain { index: 1, open: true })],
         Box::new(|_, s| ok_if((s.ic[0].ua_factor - 0.5).abs() < 1e-9 && s.ic[0].heat < 0.6 * s.ic[1].heat,
                               format!("ua_factor {} heat0 {} heat1 {}", s.ic[0].ua_factor, s.ic[0].heat, s.ic[1].heat)))),
    ]
}

#[test]
fn every_fault_persists_and_is_honoured() {
    let table = fault_table();
    assert_eq!(table.len(), 16, "one row per Fault variant");
    let mut failures = Vec::new();
    for (f, cmds, check) in table {
        let c = Case { id: "fault_probe", scenario: ScenarioName::FullPower, duration_s: 300.0, sample_dt_s: 1.0,
                       commands: cmds, faults: vec![ScriptedFault { at_s: 0.0, fault: f.clone(), active: true }], operator: Operator::None };
        let s0 = Sim::new(ScenarioName::FullPower);
        let (_, s) = run_sim_with(Sim::new(c.scenario), &c, DT, Box::new(|_: &Sim, _: f64| vec![]));
        if !s.has_fault(&f) { failures.push(format!("{f:?}: fault cleared by automatic logic")); }
        if let Err(m) = check(&s0, &s) { failures.push(format!("{f:?}: {m}")); }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}
```

- [ ] **Step 2: Run**

Run: `cargo test --release -p bwr-core --test scenarios -- every_fault`
Expected: PASS. A failure here is a defect in the package that owns the fault (P5–P7): fix it there, in the owning module, with its own failing unit test, then re-run. This test is numerical/logical, so it is never ignored.

- [ ] **Step 3: Commit**

```bash
git add crates/bwr-core/tests/scenarios.rs
git commit -m "test(scenarios): fault overrides persist and are honoured for all variants

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 8: Analytical, sensitivity and convergence evidence

**Files:**
- Test: `crates/bwr-core/tests/scenarios.rs`
- Modify: `docs/validation/method.md` (label these as verification evidence)

**Interfaces:**
- Consumes: `Kinetics::equilibrium/advance` (P3 signature, `Result`), `Vessel::new/step` returning `BoundaryFlows`, `Ledger`, `IcTrain::new/step` (P7 signature), `Core::coeffs` (P5), `Tolerance::numerical`, `compare`.

- [ ] **Step 1: Write the tests**

```rust
use bwr_core::ic::IcTrain;
use bwr_core::kinetics::Kinetics;
use bwr_core::ledger::Ledger;
use bwr_core::validation::compare;
use bwr_core::vessel::{Vessel, VesselInputs};

/// Inhour relation for the Λ-form point kinetics used by `Kinetics`.
fn inhour_rho(omega: f64) -> f64 {
    omega * GEN_TIME + (0..6).map(|i| omega * BETA * BETA_FRAC[i] / (omega + LAMBDA_DN[i])).sum::<f64>()
}

fn inhour_omega(rho: f64) -> f64 {
    let (mut lo, mut hi) = (0.0, 1.0 / GEN_TIME);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if inhour_rho(mid) < rho { lo = mid; } else { hi = mid; }
    }
    0.5 * (lo + hi)
}

#[test]
fn kinetics_matches_the_inhour_equation() {
    for rho in [0.0005, 0.001, 0.002] {
        let expected_period = 1.0 / inhour_omega(rho);
        let mut k = Kinetics::equilibrium(1.0);
        k.advance(rho, 0.0, 200.0).unwrap();
        let n1 = k.n;
        k.advance(rho, 0.0, 20.0).unwrap();
        let period = 20.0 / (k.n / n1).ln();
        let err = (period - expected_period).abs() / expected_period;
        assert!(err < 0.02, "rho {rho}: period {period:.2} s vs inhour {expected_period:.2} s ({:.1} %)", err * 100.0);
    }
}

#[test]
fn closed_vessel_blowdown_energy_is_conserved() {
    let mut v = Vessel::new(sat_p(P_RATED).t, Z_NWL, RATED_POWER_W, STEAM_FLOW_RATED, T_FW_RATED, false);
    let (m0, e0) = (v.mass, v.energy);
    let mut ledger = Ledger::new(m0, e0);
    let inp = VesselInputs { q_core: 0.0, q_ic: 0.0, w_feed: 0.0, h_feed: 0.0, w_steam: 100.0, w_liquid_out: 0.0 };
    let mut removed = 0.0;
    for _ in 0..200 {
        let f = v.step(DT, &inp);
        ledger.record(DT, &f);
        removed += (f.w_steam * f.h_steam + f.q_loss) * DT;
    }
    assert!(ledger.mass_residual(v.mass).abs() < 1e-6, "mass residual {}", ledger.mass_residual(v.mass));
    assert!(ledger.energy_residual(v.energy).abs() < 1e-9 * e0, "energy residual {}", ledger.energy_residual(v.energy));
    assert!(((e0 - v.energy) - removed).abs() < 1e-9 * e0);
    assert!(((m0 - v.mass) - 100.0 * 200.0 * DT).abs() < 1e-6);
}

#[test]
fn single_ic_train_duty_at_rated_pressure_with_boiling_pool() {
    let mut ic = IcTrain::new();
    ic.pool_t = 100.0;
    ic.cmd_open = true;
    let t_sat = sat_p(P_RATED).t;
    for _ in 0..(600.0 / DT) as usize {
        ic.step(DT, P_RATED, t_sat, true);
    }
    let err = (ic.heat - IC_TRAIN_RATED_W).abs() / IC_TRAIN_RATED_W;
    assert!(err < 0.03, "IC duty {:.2} MW vs rated {:.2} MW", ic.heat / 1e6, IC_TRAIN_RATED_W / 1e6);
}

/// Scale the void coefficient while keeping the initial state critical.
fn with_void_coef_scaled(factor: f64) -> Sim {
    let mut sim = Sim::new(ScenarioName::FullPower);
    let before = sim.core.rho_void;
    sim.core.coeffs.void_coef *= factor;
    sim.core.update_reactivity(sim.vessel.alpha_core, sim.vessel.moderator_t());
    let shift = sim.core.rho_void - before;
    sim.core.coeffs.rho_excess_cold -= shift;
    sim.core.update_reactivity(sim.vessel.alpha_core, sim.vessel.moderator_t());
    assert!(sim.core.rho_total.abs() < 1e-9);
    sim
}

#[test]
fn turbine_trip_peak_power_sensitivity_to_void_coefficient() {
    let c = case("fp_turbine_trip");
    let peak = |sim: Sim| run_sim_with(sim, &c, DT, Box::new(|_: &Sim, _: f64| vec![])).0.metrics().peak["power_frac"].0;
    let (lo, base, hi) = (peak(with_void_coef_scaled(0.75)), peak(Sim::new(c.scenario)), peak(with_void_coef_scaled(1.25)));
    println!("fp_turbine_trip peak power: void coef x0.75 -> {lo:.4}, x1 -> {base:.4}, x1.25 -> {hi:.4}");
    // A less negative void coefficient gives less reactivity insertion on void collapse: a lower peak.
    assert!(lo <= base + 1e-6 && base <= hi + 1e-6, "peak power not monotone in void coefficient: {lo} {base} {hi}");
}

#[test]
fn all_short_cases_are_converged_at_half_step() {
    let mut failures = Vec::new();
    for c in standard_cases().into_iter().filter(|c| c.duration_s <= 1800.0) {
        let a = run_case(&c, DT).metrics();
        let b = run_case(&c, DT / 2.0).metrics();
        let v = compare(&a, &b, &Tolerance::numerical());
        if !v.is_empty() { failures.push(format!("{}: {}", c.id, v.join("; "))); }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}
```

- [ ] **Step 2: Run**

Run: `cargo test --release -p bwr-core --test scenarios -- inhour blowdown single_ic sensitivity converged`
Expected: PASS. A convergence failure is a numerical defect: fix it in the owning module (P3 adaptive stepping, P6 valve rate limits, P7 tube lag) with a unit test there; never widen `Tolerance::numerical()`.

- [ ] **Step 3: Document**

Append to `docs/validation/method.md`:

```
## Verification evidence (not BWRX transient validation)

- Inhour: `kinetics_matches_the_inhour_equation` (analytical, 2 % on asymptotic period).
- Closed-vessel blowdown: `closed_vessel_blowdown_energy_is_conserved` (ledger residual < 1e-9 relative).
- IC duty: `single_ic_train_duty_at_rated_pressure_with_boiling_pool` (3 % of the registered rated duty).
- Qualitative ordering: `transient_expectations` (actuation order per case).
- Sensitivity: `turbine_trip_peak_power_sensitivity_to_void_coefficient` (peak band printed, monotonicity asserted).
- Convergence: `all_short_cases_are_converged_at_half_step` (numerical tolerance class).
```

- [ ] **Step 4: Commit**

```bash
git add crates/bwr-core/tests/scenarios.rs docs/validation/method.md
git commit -m "test(scenarios): analytical, sensitivity and convergence verification evidence

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 9: Public reference traces (search, transcribe, compare)

**Files:**
- Modify: `crates/bwr-core/src/validation.rs` (`parse_reference_csv`, `compare_reference`)
- Modify: `crates/bwr-core/examples/validation.rs`
- Create: `docs/validation/reference/<case>.csv` (only for traces actually found)
- Modify: `docs/model/sources.csv`, `docs/validation/cases.csv` (reference column)
- Test: `crates/bwr-core/src/validation.rs` unit tests

**Interfaces:**
- Produces: `parse_reference_csv(&str) -> Result<Vec<(f64, String, f64)>, String>`, `compare_reference(&Trace, &[(f64, String, f64)], &Tolerance) -> Vec<String>`.

- [ ] **Step 1: Search**

Open, in this order, and look for time-history figures of reactor power, dome pressure, steam flow, and water level:
1. ESBWR DCD Rev. 10 Tier 2 Chapter 15, sections 15.2.1 (generator load rejection with/without bypass), 15.2.2 (turbine trip with/without bypass), 15.2.3 (closure of all MSIVs), 15.2.4 (loss of feedwater heating) — link in the spec's "Public source starting points".
2. NRC stability safety evaluation (ML26078A299) and NEDO-34270-A for any decay-ratio or power-oscillation trace.
3. NEDO-33910-A Rev. 2 (RPV isolation and overpressure) for any pressure-time curve.

For each figure found: record it in `docs/model/sources.csv` (document, section, figure number, page, retrieval date, checksum of the PDF), then read at least 8 points per curve by eye from the figure axes and write `docs/validation/reference/<case>.csv`:

```
t_s,column,value,source
0,power_frac,1.0,"ESBWR DCD R10 T2 Fig 15.2-7 (reactor power), digitised by eye ±2 % of full scale"
```

Columns must be names from `TRACE_COLUMNS`. Pressure in Pa, power as a fraction of rated. If no figure can be read to ±5 % of full scale, write nothing and set the `reference` cell in `cases.csv` to `none available` (it already is for every row except the four marked "mechanism only").

- [ ] **Step 2: Write the failing unit test**

```rust
#[cfg(test)]
mod reference_tests {
    use super::*;
    #[test]
    fn reference_points_compare_with_interpolation() {
        let csv = "t_s,column,value,source\n0,power_frac,1.0,x\n10,power_frac,0.5,x\n";
        let pts = parse_reference_csv(csv).unwrap();
        assert_eq!(pts.len(), 2);
        let mut trace = Trace { case_id: "t".into(), dt_s: 1.0, rows: vec![], events: vec![] };
        for k in 0..=20 {
            let mut row = [0.0; 24];
            row[0] = k as f64;
            row[1] = 1.0 - 0.05 * k as f64;
            trace.rows.push(row);
        }
        assert!(compare_reference(&trace, &pts, &Tolerance::physical()).is_empty());
        let bad = vec![(5.0, "power_frac".to_string(), 0.5)];
        assert_eq!(compare_reference(&trace, &bad, &Tolerance::physical()).len(), 1);
    }
}
```

- [ ] **Step 3: Implement**

```rust
pub fn parse_reference_csv(s: &str) -> Result<Vec<(f64, String, f64)>, String> {
    let mut out = Vec::new();
    for (k, line) in s.lines().enumerate().skip(1) {
        if line.trim().is_empty() { continue; }
        let mut it = line.splitn(4, ',');
        let t: f64 = it.next().ok_or(format!("line {k}: missing t"))?.trim().parse().map_err(|e| format!("line {k}: {e}"))?;
        let col = it.next().ok_or(format!("line {k}: missing column"))?.trim().to_string();
        let v: f64 = it.next().ok_or(format!("line {k}: missing value"))?.trim().parse().map_err(|e| format!("line {k}: {e}"))?;
        if !TRACE_COLUMNS.contains(&col.as_str()) { return Err(format!("line {k}: unknown column {col}")); }
        out.push((t, col, v));
    }
    Ok(out)
}

fn interpolate(trace: &Trace, col: &str, t: f64) -> Option<f64> {
    let ci = TRACE_COLUMNS.iter().position(|c| *c == col)?;
    let rows = &trace.rows;
    let k = rows.iter().position(|r| r[0] >= t)?;
    if k == 0 { return Some(rows[0][ci]); }
    let (a, b) = (rows[k - 1], rows[k]);
    let f = (t - a[0]) / (b[0] - a[0]).max(1e-12);
    Some(a[ci] + f * (b[ci] - a[ci]))
}

pub fn compare_reference(trace: &Trace, points: &[(f64, String, f64)], tol: &Tolerance) -> Vec<String> {
    let mut v = Vec::new();
    for (t, col, reference) in points {
        match interpolate(trace, col, *t) {
            None => v.push(format!("{col} at {t} s: outside trace")),
            Some(sim) => {
                let floor = tol.abs.get(col).copied().unwrap_or(0.0);
                let allowed = (tol.rel * reference.abs()).max(floor);
                if (sim - reference).abs() > allowed {
                    v.push(format!("{col} at {t} s: sim {sim:.4} vs reference {reference:.4} (allowed ±{allowed:.4})"));
                }
            }
        }
    }
    v
}
```

In `examples/validation.rs`, inside the `(_, true)` arm after the baseline comparison:

```rust
let ref_path = format!("{}/../../docs/validation/reference/{}.csv", env!("CARGO_MANIFEST_DIR"), case.id);
if let Ok(s) = std::fs::read_to_string(&ref_path) {
    let pts = parse_reference_csv(&s)?;
    let v = compare_reference(&trace, &pts, &Tolerance::physical());
    println!("      reference: ESBWR reference configuration mechanism check, not BWRX acceptance: {} points, {} outside tolerance", pts.len(), v.len());
    for line in &v { println!("        {line}"); }
}
```

Reference mismatches are printed as findings; they do not fail the run (the spec: "its numerical trace is not automatically a BWRX acceptance curve").

- [ ] **Step 4: Run and commit**

Run: `cargo test --release -p bwr-core` → PASS; `cargo run --release -p bwr-core --example validation` → prints reference lines for every case with a file in `docs/validation/reference/`.

```bash
git add crates/bwr-core/src/validation.rs crates/bwr-core/examples/validation.rs docs/validation docs/model/sources.csv
git commit -m "feat(validation): public reference point comparison (ESBWR mechanism check)

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 10: Paired-failure exploration

**Files:**
- Create: `crates/bwr-core/examples/paired_failures.rs`
- Create: `docs/validation/paired-failures.md` (generated)

**Interfaces:**
- Consumes: `run_sim_with`, `Fault`, `Core.clad_t`, `Sim.domain_errors`, `Metrics`.

- [ ] **Step 1: Write the example**

```rust
//! Explore the most consequential supported paired failures and report where the
//! model leaves its envelope. Output: docs/validation/paired-failures.md (report, never asserted).
use std::cell::Cell;
use std::rc::Rc;

use bwr_core::api::{Command, ScenarioName};
use bwr_core::bop::LevelControlMode;
use bwr_core::faults::Fault;
use bwr_core::sim::DT;
use bwr_core::validation::{run_sim_with, Case, Operator, ScriptedCommand, ScriptedFault};
use bwr_core::Sim;

fn at(at_s: f64, cmd: Command) -> ScriptedCommand { ScriptedCommand { at_s, cmd } }
fn f(at_s: f64, fault: Fault) -> ScriptedFault { ScriptedFault { at_s, fault, active: true } }

fn main() {
    let strongest = Sim::new(ScenarioName::FullPower).core.rods.strongest_rod();
    let pairs: Vec<(&str, Vec<ScriptedCommand>, Vec<ScriptedFault>, Operator)> = vec![
        ("turbine trip + bypass unavailable", vec![at(5.0, Command::TripTurbine)], vec![f(0.0, Fault::BypassUnavailable)], Operator::None),
        ("MSIV closure + one IC train", vec![at(5.0, Command::SetMsiv { line: 0, open: false }), at(5.0, Command::SetMsiv { line: 1, open: false })],
         vec![f(0.0, Fault::IcTrainUnavailable { train: 1 }), f(0.0, Fault::IcTrainUnavailable { train: 2 })], Operator::None),
        ("loss of feedwater + stuck strongest rod", vec![],
         vec![f(5.0, Fault::FeedPumpTrip { index: 0 }), f(5.0, Fault::FeedPumpTrip { index: 1 }), f(5.0, Fault::FeedPumpTrip { index: 2 }), f(0.0, Fault::RodStuck { index: strongest })], Operator::None),
        ("CW loss + vacuum pump failure", vec![], vec![f(5.0, Fault::CwPumpTrip { index: 0 }), f(5.0, Fault::CwPumpTrip { index: 1 }), f(5.0, Fault::VacuumPumpFailed)], Operator::None),
        ("load rejection + regulator fail open", vec![at(5.0, Command::OpenBreaker)], vec![f(5.0, Fault::PressureRegulatorFailed { demand_pct: 100 })], Operator::None),
        ("heater loss + rod withdrawal error", vec![], vec![f(5.0, Fault::HeatersLost)], Operator::RodWithdrawalError),
        ("low level + IC return valve stuck", vec![at(5.0, Command::SetLevelControl { mode: LevelControlMode::Manual }), at(6.0, Command::SetFcvManual { pos: 0.0 })],
         vec![f(0.0, Fault::IcReturnValveStuck { train: 0, valve: 0 }), f(0.0, Fault::IcReturnValveStuck { train: 0, valve: 1 })], Operator::None),
        ("MSIV closure + 50 % IC noncondensable degradation", vec![at(5.0, Command::SetMsiv { line: 0, open: false }), at(5.0, Command::SetMsiv { line: 1, open: false })],
         (0..3).map(|t| f(0.0, Fault::IcNoncondensableDegradation { train: t, pct: 50 })).collect(), Operator::None),
    ];

    let mut md = String::from("# Paired-failure exploration\n\nGenerated by `cargo run --release -p bwr-core --example paired_failures`. Each pair runs 1800 s from full power. Rows report where the model leaves its declared envelope; nothing here is an acceptance claim.\n\n| Pair | First domain error | Peak pressure (MPa) | Min NR level (cm) | Peak clad (degC) |\n|---|---|---|---|---|\n");
    for (name, commands, faults, operator) in pairs {
        let case = Case { id: "pair", scenario: ScenarioName::FullPower, duration_s: 1800.0, sample_dt_s: 1.0, commands, faults, operator: operator.clone() };
        let clad_peak = Rc::new(Cell::new(0.0f64));
        let cp = clad_peak.clone();
        let mut inner = bwr_core::validation::operators::build(&operator);
        let op = Box::new(move |sim: &Sim, t: f64| {
            cp.set(cp.get().max(sim.core.clad_t));
            inner(sim, t)
        });
        let (trace, sim) = run_sim_with(Sim::new(case.scenario), &case, DT, op);
        let m = trace.metrics();
        let first = sim.domain_errors.first().map(|(t, d)| format!("{d:?} at {t:.0} s")).unwrap_or_else(|| "none".into());
        md.push_str(&format!("| {name} | {first} | {:.3} | {:.0} | {:.0} |\n", m.peak["pressure_pa"].0 / 1e6, m.min["level_nr_cm"].0, clad_peak.get()));
        println!("{name}: {first}");
    }
    let path = format!("{}/../../docs/validation/paired-failures.md", env!("CARGO_MANIFEST_DIR"));
    std::fs::write(&path, md).expect("write paired-failures.md");
    println!("wrote {path}");
}
```

- [ ] **Step 2: Run and commit**

Run: `cargo run --release -p bwr-core --example paired_failures`
Expected: eight lines and `wrote .../paired-failures.md`; the table has eight rows. Any `DomainError` row is expected output, not a failure.

```bash
git add crates/bwr-core/examples/paired_failures.rs docs/validation/paired-failures.md
git commit -m "feat(validation): paired-failure exploration report

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 11: Coverage: every command and subsystem has a scenario or an explicit limitation

**Files:**
- Test: `crates/bwr-core/tests/scenarios.rs`
- Modify: `docs/model/coverage.md`

**Interfaces:**
- Consumes: `Command` (serde tag `type`), `standard_cases()`, `operators::build`.

- [ ] **Step 1: Write the failing test**

```rust
/// Commands that are UI conveniences with no plant effect; listed here as the explicit limitation.
const UI_ONLY_COMMANDS: &[&str] = &["setPaused", "setSpeed", "loadScenario", "selectRod", "ackAlarms"];

fn command_tag(c: &Command) -> String {
    serde_json::to_value(c).unwrap()["type"].as_str().unwrap().to_string()
}

/// Exhaustive match: adding a Command variant without updating this list is a compile error.
fn all_command_tags() -> Vec<String> {
    use Command::*;
    let examples = vec![
        SetPaused { paused: true }, SetSpeed { speed: 1.0 }, LoadScenario { name: ScenarioName::FullPower }, ManualScram, ResetScram,
        SetModeSwitch { mode: ModeSwitch::Run }, SelectRod { index: None }, RodSetTarget { indices: vec![], target: 0.0 }, RodStop,
        SetIrmRange { range: 1 }, SetHeadVent { open: true }, SetMsiv { line: 0, open: true }, SetPressureSetpoint { mpa: 7.0 },
        SetBypass { auto: true, pos: 0.0 }, SetSpeedSetpoint { rpm: 0.0 }, ResetTurbine, TripTurbine, CloseBreaker, OpenBreaker,
        SetLoadLimit { pct: 100.0 }, SetCwPump { index: 0, on: true }, SetVacuumPump { on: true }, SetFeedPump { index: 0, on: true },
        SetCondensatePump { index: 0, on: true }, SetLevelControl { mode: LevelControlMode::Manual }, SetFcvManual { pos: 0.0 },
        SetLevelSetpoint { cm: 0.0 }, SetHeaters { in_service: true }, SetRwcuReject { kgs: 0.0 }, SetIcTrain { index: 0, open: true }, AckAlarms,
    ];
    for c in &examples {
        match c {
            SetPaused { .. } | SetSpeed { .. } | LoadScenario { .. } | ManualScram | ResetScram | SetModeSwitch { .. } | SelectRod { .. }
            | RodSetTarget { .. } | RodStop | SetIrmRange { .. } | SetHeadVent { .. } | SetMsiv { .. } | SetPressureSetpoint { .. }
            | SetBypass { .. } | SetSpeedSetpoint { .. } | ResetTurbine | TripTurbine | CloseBreaker | OpenBreaker | SetLoadLimit { .. }
            | SetCwPump { .. } | SetVacuumPump { .. } | SetFeedPump { .. } | SetCondensatePump { .. } | SetLevelControl { .. }
            | SetFcvManual { .. } | SetLevelSetpoint { .. } | SetHeaters { .. } | SetRwcuReject { .. } | SetIcTrain { .. } | AckAlarms => {}
        }
    }
    examples.iter().map(command_tag).collect()
}

#[test]
fn every_command_is_exercised_by_a_case_or_listed_as_ui_only() {
    let mut used = std::collections::BTreeSet::new();
    for c in standard_cases() {
        for sc in &c.commands { used.insert(command_tag(&sc.cmd)); }
        // Operator cases: run them and collect the commands the operator emits.
        if c.operator != Operator::None {
            let mut op = operators::build(&c.operator);
            let mut sim = Sim::new(c.scenario);
            let mut t = 0.0;
            while t < c.duration_s {
                for cmd in op(&sim, t) { used.insert(command_tag(&cmd)); sim.apply(cmd); }
                for _ in 0..20 { sim.step(DT); }
                t += 1.0;
            }
        }
    }
    let missing: Vec<String> = all_command_tags().into_iter().filter(|t| !used.contains(t) && !UI_ONLY_COMMANDS.contains(&t.as_str())).collect();
    assert!(missing.is_empty(), "commands with no scenario and no limitation: {missing:?}");
}
```

Add `serde_json` is already a dev-dependency (used by `tests/plant.rs`).

- [ ] **Step 2: Run**

Run: `cargo test --release -p bwr-core --test scenarios -- every_command`
Expected: FAIL listing commands such as `resetScram`, `rodStop`, `setHeadVent`, `setBypass`, `setLoadLimit`.

- [ ] **Step 3: Add the missing cases**

Add to `standard_cases()` (and `cases.csv`, role `validation`):

```rust
fp("fp_scram_reset_after_manual", 300.0, vec![at(5.0, Command::ManualScram), at(30.0, Command::ResetScram)], vec![]),
fp("fp_rod_stop_mid_motion", 120.0, vec![at(5.0, Command::RodSetTarget { indices: vec![strongest], target: ROD_FULL_OUT }), at(8.0, Command::RodStop)], vec![]),
Case { id: "cs_head_vent_close_and_heatup", scenario: ScenarioName::ColdShutdown, duration_s: 3600.0, sample_dt_s: 10.0,
       commands: vec![at(5.0, Command::SetHeadVent { open: false })], faults: vec![], operator: Operator::None },
fp("fp_bypass_manual_10pct", 300.0, vec![at(5.0, Command::SetBypass { auto: false, pos: 0.10 })], vec![]),
fp("fp_load_limit_80pct", 600.0, vec![at(5.0, Command::SetLoadLimit { pct: 80.0 })], vec![]),
```

```
fp_scram_reset_after_manual,fullPower,"Manual scram then reset after 30 s: reset accepted, rods stay in, no withdrawal",none available,physical,validation
fp_rod_stop_mid_motion,fullPower,"Rod stop command halts a withdrawal at the nearest notch",none available,physical,validation
cs_head_vent_close_and_heatup,coldShutdown,"Head vent closed at cold shutdown: regime leaves Vented, pressure follows gas and decay heat",none available,plausibility,plausibility
fp_bypass_manual_10pct,fullPower,"Bypass to manual 10 %: pressure regulator compensates on the TCV",none available,physical,validation
fp_load_limit_80pct,fullPower,"Load limit 80 %: TCV limited, bypass carries the remainder",none available,physical,validation
```

Add expectations rows to `EXPECTATIONS` for the four full-power cases (`order: &[]`, `pressure_below_design: true`, `supported: true`; `no_scram: true` for the bypass and load-limit cases, `false` for the two scram-related ones). Run `--write-missing`.

- [ ] **Step 4: Update `docs/model/coverage.md`**

Add a column `Scenario(s)` to the subsystem table, filling each subsystem row with the case ids above, and a final row per explicit limitation:

```
| Command/area | Scenario(s) or limitation |
| setPaused, setSpeed, loadScenario, selectRod, ackAlarms | UI only; no plant effect; exercised by UI typecheck and P10 contract test |
| Cold shutdown below the IC pool temperature | Limitation: requires an unimplemented shutdown cooling system (see docs/model/isolation-condensers.md); fp_cooldown_on_ic stops at that boundary |
| ATWS, LOCA, severe accident | Out of scope; any run reaching a DomainError is reported as unsupported |
```

- [ ] **Step 5: Run everything and commit**

Run: `cargo test --release -p bwr-core` → PASS; `cargo run --release -p bwr-core --example validation` → all ok; `npm --prefix ui run typecheck` → unchanged (no API change in this package).

```bash
git add crates/bwr-core/src/validation.rs crates/bwr-core/tests/scenarios.rs docs/validation docs/model/coverage.md
git commit -m "test(scenarios): every command has a scenario or an explicit limitation

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

## Acceptance gate (verbatim from the spec)

**Acceptance gate:** Every current subsystem and control has an assigned scenario or explicit limitation. No unsupported ATWS, LOCA, or severe-accident conclusion is inferred from partial models.

Evidence: `every_command_is_exercised_by_a_case_or_listed_as_ui_only` and `docs/model/coverage.md` (first sentence); `transient_expectations` with `supported`/`expect_domain`, `fp_feed_loss_no_ic`, and `docs/validation/paired-failures.md` (second sentence).

## Self-review against the spec's Package 9 bullets

| Spec bullet | Task |
|---|---|
| Correct initialization of cold shutdown, hot standby, full power | Task 2 |
| Normal startup, power changes, controlled shutdown, manual scram, mode-switch shutdown, supported cooldown endpoints | Tasks 3, 4 |
| Turbine trip and load rejection with/without bypass; single and multiple steam isolation; pressure-controller faults; excessive steam demand | Tasks 5, 6 |
| Feed-pump/condensate-pump loss, feed-valve faults, loss of feedwater heating, high/low level, cooling-water loss, vacuum degradation, hotwell limits | Tasks 5, 6 (hotwell limit: `fp_feed_loss_no_ic` and P6's CST accounting; if P6 provides `CST_CAPACITY`, add `fp_hotwell_cst_full` built with `run_sim_with` after setting `sim.cond.cst_mass = CST_CAPACITY`, asserting `sim.cond.reject_flow == 0.0` at the end and no domain error) |
| Rod withdrawal errors, rod-insertion failures, IC availability combinations | Tasks 5, 6 |
| Deterministic test-level fault overrides | Task 7 |
| Public transient traces; calibrate/validate split | Tasks 5 (roles), 9 |
| Conservation, analytical, ordering, sensitivity, convergence evidence, labelled | Task 8 |
| Paired failures; report envelope exits | Task 10 |
