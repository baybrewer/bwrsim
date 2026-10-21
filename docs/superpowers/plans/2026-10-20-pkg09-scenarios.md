# Package 9: Verify Complete Existing-System Scenarios — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Names and signatures are governed by `docs/superpowers/plans/2026-10-20-refinement-master.md` (r2); where this file and the master disagree, the master wins.

**Goal:** Give every implemented subsystem and operator control an assigned, deterministic scenario with objective assertions, so whole-plant behaviour is verified end to end and every unsupported condition is reported rather than silently extrapolated.

**Architecture:** Fill in the closed-loop operator bodies that Package 2's runner dispatches (`Operator` → `validation::operators::*`), extend `standard_cases()` and `docs/validation/cases.csv` to the full register with roles, and add `tests/scenarios.rs`: table-driven tests asserting actuation order (through Package 2's event keys), extrema bounds, the support envelope, transfer allocation at exhaustion, and fault persistence. Analytical component checks and a paired-failure exploration give verification evidence where no public trace exists.

**Tech Stack:** Rust 2021 (`bwr-core`), existing `serde_json` dev-dependency, hand-rolled CSV (no new dependencies).

**Spec section:** Package 9 of `docs/superpowers/plans/2026-10-20-existing-systems-refinement.md`.

**Prerequisites (all from the master r2):** P2 `validation.rs` with `Case { .., dt_after_s, operator }`, `Operator { None, Startup { target_frac_pct }, PowerChange { from_pct, to_pct }, ControlledShutdown, CooldownOnIc, RodWithdrawalError }`, `OPERATOR_DT_S`, `run_case`, `run_case_with<F>`, `run_sim_with<F>`, `standard_cases`, `Trace` (schema 2, `N_TRACE_COLS`, `col_index`), `Metrics` (`initial_drift_per_s`, `first_event_time` keyed by event text before the first `:`), `Tolerance::{numerical, physical, source_comparison}`, `compare`; the event keys `scram`, `turbine trip`, `msiv isolation`, `ic auto-initiation`, `feed pump trip`, `domain error`; `Sim.log`; P2 `faults.rs` (`Fault`, `kind`, `equipment`, `is_valid`, `Sim.faults: Vec<Fault>`, `set_fault`, `has_fault`, `find_fault`); P3 `DomainError`, `Sim.domain_errors`, `Kinetics::advance -> Result`; P4 `Regime`; P5 `RodMotion`, `Rods::{moving, strongest_rod, shutdown_margin}`, `Core::{clad_t, coeffs}`, `ReactivityCoeffs { void, doppler, mod_t, xenon_worth }`, `Sim::full_power_with`, `DecayHeat::after_history`, `IRRADIATION_HISTORY_S`; P6 `Sim.balance`, `SteamSystem.line.mass`, `Condenser.{cst_mass, reject_flow}`, `CST_CAPACITY_KG`; P7 `IcTrain::{in_service, step(dt, p_vessel, w_steam_available)}`, `IcTrain.{available, ua_factor, return_valve_pos, steam_valve_pos, tube_mass}`, `IC_TRAIN_RATED_W`; P8 sensors.

## Global Constraints

Everything in the master plan's Global Constraints, plus:

- "Do not bulk-retune the plant between packages or overwrite validation references simply to restore a passing startup script."
- "No previous-test failure is concealed by updating expected output."
- "Compare against public transient traces where available. Calibrate on one set of cases and reserve independent cases for validation."
- "For missing public traces, use conservation, analytical component solutions, qualitative response ordering, parameter sensitivity, and convergence tests. Label this as verification or plausibility evidence, not quantitative BWRX transient validation."
- "Use deterministic test-level fault overrides for existing equipment so automatic logic cannot erase an injected stuck-valve or unavailable-train condition."
- **Finding policy:** when a *physical* assertion in `tests/scenarios.rs` fails (ordering, pressure bound, envelope), do not change parameters or the assertion. Record the case id, observed value and suspected cause as `F-nnn` in `docs/validation/findings.md`, set that case's `status` in `cases.csv` to `ignored:F-nnn`, move the table row to `IGNORED_EXPECTATIONS` (printed, not asserted) or mark the single test `#[ignore = "F-nnn"]`, and move on. Numerical assertions (convergence, residuals, determinism, allocation) are never ignored; they are fixed in the owning module.
- Event strings are matched only through Package 2's event keys (text before the first `:`), never by grepping `sim.rs`. The keys live in `mod ev` of `tests/scenarios.rs` and nowhere else in this package.
- `cases.csv` header is locked by the master: `id,scenario,duration_s,purpose,reference,tolerance_class,role,status`. Readers parse by header name.

## Contract additions owned by this package (already listed in the master r2)

```rust
// validation.rs: operator bodies (P2 ships the enum, Case.operator, run_case dispatch and the runner functions)
pub mod operators {
    pub const POWER_CHANGE_HOLD_S: f64 = 600.0;
    pub fn startup(target_frac_pct: u8) -> impl FnMut(&Sim, f64) -> Vec<Command>;
    pub fn power_change(from_pct: u8, to_pct: u8) -> impl FnMut(&Sim, f64) -> Vec<Command>;
    pub fn controlled_shutdown() -> impl FnMut(&Sim, f64) -> Vec<Command>;
    pub fn cooldown_on_ic() -> impl FnMut(&Sim, f64) -> Vec<Command>;
    pub fn rod_withdrawal_error() -> impl FnMut(&Sim, f64) -> Vec<Command>;
}
/// Public reference points (t_s, column, value) compared to a trace (Task 9).
pub fn parse_reference_csv(s: &str) -> Result<Vec<(f64, String, f64)>, String>;
pub fn compare_reference(trace: &Trace, points: &[(f64, String, f64)], tol: &Tolerance) -> Vec<String>;
// sim.rs
impl Sim { pub fn settle_until_steady(&mut self, max_s: f64) -> f64; }
```

Files: `cases.csv` `role` column filled (`calibration | validation | plausibility`); `examples/validation.rs --write-missing`; `examples/paired_failures.rs`; `docs/validation/paired-failures.md`; `docs/validation/results/paired_failures.csv`; `docs/validation/findings.md`; `docs/validation/reference/<case>.csv` (only when a public trace is found). Test-local helpers in `tests/scenarios.rs`: `operator_of`, `event_order`, `mod ev`.

**Known cross-package inconsistency to resolve before Task 6:** the Package 8 plan writes log lines such as `L8 high level: feed pumps tripped`, `Turbine trip: low condenser vacuum`, `Isolation condensers auto-initiated`, which do not start with Package 2's event keys. Package 8 must use the keys (`feed pump trip: L8 high level`, `turbine trip: low condenser vacuum`, `ic auto-initiation: all trains`); if that has not happened when Package 9 starts, fix it in `sim.rs` under the Package 8 plan first (one-line commit), not here.

---

### Task 1: Operator bodies behind Package 2's dispatch

**Files:**
- Modify: `crates/bwr-core/src/validation.rs` (`mod operators`: replace the P2 stub bodies)
- Test: `crates/bwr-core/tests/scenarios.rs` (new)

**Interfaces:**
- Consumes: `Operator`, `OPERATOR_DT_S`, `run_case`, `run_case_with`, `run_sim_with`, `Sim::{scrammed, mode, irm_range}`, `irm_full_scale`, `Rods::{pos, weights, moving}`, `Kinetics.inv_period`, `Turbine::{breaker_closed, speed_rpm, phase_deg, p_elec}`, `Feedwater::running_pumps`, `Vessel::{level_nr_cm, eos, heatup_rate}`, `IcTrain::{cmd_open, available}`, `SteamSystem.msiv_cmd`, `Rods::strongest_rod`.
- Produces: the five operator functions above and `POWER_CHANGE_HOLD_S`.

- [ ] **Step 1: Confirm the P2 skeleton exists**

Run:

```bash
grep -n "pub enum Operator\|pub fn run_sim_with\|pub fn run_case_with\|pub mod operators\|pub fn startup\|pub fn power_change\|pub fn controlled_shutdown\|pub fn cooldown_on_ic\|pub fn rod_withdrawal_error" crates/bwr-core/src/validation.rs
```

Expected: nine matches. If any is missing, Package 2's acceptance gate was not met; stop and complete Package 2 first (do not add the runner here).

- [ ] **Step 2: Write the failing tests**

```rust
// crates/bwr-core/tests/scenarios.rs
use bwr_core::api::{Command, ModeSwitch, ScenarioName};
use bwr_core::sim::DT;
use bwr_core::validation::{operators, run_case, run_case_with, run_sim_with, standard_cases, Case, Operator};
use bwr_core::Sim;

/// Boxed dispatcher matching `run_case`'s, for tests that need the final `Sim`.
fn operator_of(op: Operator) -> Box<dyn FnMut(&Sim, f64) -> Vec<Command>> {
    match op {
        Operator::None => Box::new(|_: &Sim, _: f64| Vec::new()),
        Operator::Startup { target_frac_pct } => Box::new(operators::startup(target_frac_pct)),
        Operator::PowerChange { from_pct, to_pct } => Box::new(operators::power_change(from_pct, to_pct)),
        Operator::ControlledShutdown => Box::new(operators::controlled_shutdown()),
        Operator::CooldownOnIc => Box::new(operators::cooldown_on_ic()),
        Operator::RodWithdrawalError => Box::new(operators::rod_withdrawal_error()),
    }
}

fn case(id: &str) -> Case {
    standard_cases().into_iter().find(|c| c.id == id).unwrap_or_else(|| panic!("no case {id}"))
}

/// Runs a registered case with its own operator and returns the trace and final Sim.
fn run_full(id: &str) -> (bwr_core::validation::Trace, Sim) {
    let c = case(id);
    run_sim_with(Sim::new(c.scenario), &c, DT, operator_of(c.operator))
}

fn probe(id: &'static str, scenario: ScenarioName, duration_s: f64) -> Case {
    Case { id, scenario, duration_s, sample_dt_s: 1.0, commands: vec![], faults: vec![], dt_after_s: None, operator: Operator::None }
}

#[test]
fn operator_is_consulted_once_per_second_and_its_commands_apply() {
    let mut calls = 0u32;
    let trace = run_case_with(&probe("op_probe", ScenarioName::FullPower, 30.0), DT, move |sim: &Sim, t: f64| {
        calls += 1;
        assert!((t - (calls as f64 - 1.0)).abs() < 1e-6, "operator called at t={t}, call {calls}");
        if t >= 5.0 && !sim.scrammed { vec![Command::ManualScram] } else { vec![] }
    });
    let t_scram = trace.metrics().first_event_time.get("scram").copied();
    assert!(matches!(t_scram, Some(t) if (5.0..5.1).contains(&t)), "events {:?}", trace.events);
    assert_eq!(trace.column("scrammed").last().copied(), Some(1.0));
}

#[test]
fn startup_operator_pulls_rods_and_ranges_irms_from_hot_standby() {
    let c = probe("startup_probe", ScenarioName::HotStandby, 900.0);
    let (trace, sim) = run_sim_with(Sim::new(c.scenario), &c, DT, operators::startup(30));
    assert!(trace.column("rods_avg_pos").last().unwrap() > &0.5, "no rods withdrawn in 15 min");
    assert!(sim.irm_range >= 1 && !sim.scrammed, "scram {:?}", sim.scram_reason);
    assert!(sim.core.kin.n > Sim::new(ScenarioName::HotStandby).core.kin.n * 10.0, "power did not rise");
}

#[test]
fn rod_withdrawal_error_operator_targets_the_strongest_rod() {
    let mut op = operators::rod_withdrawal_error();
    let sim = Sim::new(ScenarioName::FullPower);
    assert!(op(&sim, 0.0).is_empty(), "no action before 5 s");
    let cmds = op(&sim, 5.0);
    let i = sim.core.rods.strongest_rod();
    assert!(matches!(cmds.as_slice(), [Command::RodSetTarget { indices, target }] if indices == &vec![i] && *target == bwr_core::params::ROD_FULL_OUT));
}

#[test]
fn run_case_dispatches_the_case_operator() {
    let mut c = probe("dispatch_probe", ScenarioName::FullPower, 30.0);
    c.operator = Operator::RodWithdrawalError;
    let a = run_case(&c, DT);
    let b = run_case_with(&c, DT, operators::rod_withdrawal_error());
    assert_eq!(a, b);
    assert!(a.column("rods_max_pos").last().unwrap() > a.column("rods_max_pos").first().unwrap());
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test --release -p bwr-core --test scenarios`
Expected: the P2 stub operators return no commands, so `startup_operator_pulls_rods...`, `rod_withdrawal_error_operator_targets...` and `run_case_dispatches...` FAIL on their assertions; the first test passes (runner only).

- [ ] **Step 4: Implement the operator bodies**

Replace the body of `pub mod operators` in `validation.rs` with:

```rust
pub mod operators {
    use crate::api::{Command, ModeSwitch};
    use crate::bop::LevelControlMode;
    use crate::params::{N_RODS, ROD_FULL_OUT};
    use crate::sim::irm_full_scale;
    use crate::Sim;

    /// Hold time at the intermediate power of a `PowerChange` before continuing.
    pub const POWER_CHANGE_HOLD_S: f64 = 600.0;

    /// Hot standby -> critical -> heatup -> roll -> sync -> `target_frac_pct` % power.
    pub fn startup(target_frac_pct: u8) -> impl FnMut(&Sim, f64) -> Vec<Command> {
        let t = target_frac_pct as f64 / 100.0;
        startup_schedule(t, 0.0, t)
    }

    /// Startup to `from_pct`, hold POWER_CHANGE_HOLD_S, then withdraw to `to_pct`.
    pub fn power_change(from_pct: u8, to_pct: u8) -> impl FnMut(&Sim, f64) -> Vec<Command> {
        startup_schedule(from_pct as f64 / 100.0, POWER_CHANGE_HOLD_S, to_pct as f64 / 100.0)
    }

    /// Scripted operator ported from the original tests/plant.rs startup script: banked
    /// withdrawal highest-worth first, period guard, IRM ranging, RWCU level trim, turbine
    /// roll at 3.5 % and 6.9 MPa, sync, RUN, second pump set and three-element at 20 %.
    fn startup_schedule(first_frac: f64, hold_s: f64, target_frac: f64) -> impl FnMut(&Sim, f64) -> Vec<Command> {
        let mut order: Vec<usize> = Vec::new();
        let mut ptr = 0usize;
        let mut stage = 0u8;
        let mut started = false;
        let mut hold_reached_at: Option<f64> = None;
        move |sim: &Sim, t: f64| {
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
            if n < goal && !slow && !sim.core.rods.moving() {
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
        }
    }

    /// Rods in by notches (lowest-worth first), leave RUN below the APRM setdown, trip the
    /// turbine at low load, then mode switch to SHUTDOWN once power is on decay heat.
    pub fn controlled_shutdown() -> impl FnMut(&Sim, f64) -> Vec<Command> {
        let mut order: Vec<usize> = Vec::new();
        let mut started = false;
        let mut ptr = 0usize;
        let mut turbine_off = false;
        move |sim: &Sim, _t: f64| {
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
            if !sim.core.rods.moving() {
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
        }
    }

    /// After scram, isolate at 60 s and throttle IC trains to hold the cooldown between
    /// 30 and 50 degC/h until the vessel approaches the IC pool temperature.
    pub fn cooldown_on_ic() -> impl FnMut(&Sim, f64) -> Vec<Command> {
        move |sim: &Sim, t: f64| {
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
        }
    }

    /// Continuous withdrawal of the strongest rod from 5 s until it is out, blocked, or scrammed.
    pub fn rod_withdrawal_error() -> impl FnMut(&Sim, f64) -> Vec<Command> {
        move |sim: &Sim, t: f64| {
            if t < 5.0 || sim.scrammed {
                return vec![];
            }
            let i = sim.core.rods.strongest_rod();
            if sim.core.rods.pos[i] >= ROD_FULL_OUT {
                return vec![];
            }
            vec![Command::RodSetTarget { indices: vec![i], target: ROD_FULL_OUT }]
        }
    }
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test --release -p bwr-core`
Expected: all pass. `cargo run --release -p bwr-core --example validation` still passes (no registered case has an operator yet).

- [ ] **Step 6: Commit**

```bash
git add crates/bwr-core/src/validation.rs crates/bwr-core/tests/scenarios.rs
git commit -m "feat(validation): scripted operator bodies for startup, shutdown, cooldown, RWE

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 2: Initial conditions match their claimed histories

**Files:**
- Modify: `crates/bwr-core/src/sim.rs` (`settle_until_steady`, `full_power`)
- Test: `crates/bwr-core/tests/scenarios.rs`

**Interfaces:**
- Consumes: `DecayHeat::after_history`, `IRRADIATION_HISTORY_S` (both P5), `Xenon::{equilibrium, step}`, `Regime`, `Sim.balance.residual_w` (P6), `Metrics.initial_drift_per_s`, `Tolerance::numerical().abs`, `IcTrain::in_service`.
- Produces: `Sim::settle_until_steady(max_s: f64) -> f64`.

- [ ] **Step 1: Write the tests**

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
    assert_eq!(sim.steam.vessel_steam_flow(), 0.0);
    assert!(sim.domain_errors.is_empty(), "{:?}", sim.domain_errors);
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
    assert!(sim.domain_errors.is_empty(), "{:?}", sim.domain_errors);
}

#[test]
fn full_power_is_an_initial_equilibrium() {
    let sim = Sim::new(ScenarioName::FullPower);
    let xe_eq = Xenon::equilibrium(sim.core.kin.n);
    assert!((sim.core.xenon.xenon - xe_eq.xenon).abs() < 0.01, "xenon {} vs {}", sim.core.xenon.xenon, xe_eq.xenon);
    assert!((sim.vessel.eos.p - P_RATED).abs() < 0.05e6);
    assert!(sim.vessel.level_nr_cm().abs() < 5.0);
    assert!(sim.balance.residual_w.abs() < 0.01 * RATED_POWER_W, "balance residual {} W", sim.balance.residual_w);
    assert!(sim.domain_errors.is_empty(), "{:?}", sim.domain_errors);
    let m = run_case(&case("fp_steady_600"), DT).metrics();
    let tol = Tolerance::numerical();
    for col in ["power_frac", "pressure_pa", "level_nr_cm"] {
        // The initial-equilibrium residual (method.md) may not consume the numerical floor within a minute.
        let limit = tol.abs[col] / 60.0;
        assert!(m.initial_drift_per_s[col] < limit, "{col}: initial drift {} /s >= {limit}", m.initial_drift_per_s[col]);
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --release -p bwr-core --test scenarios -- history equilibrium`
Expected: the cold and hot tests PASS (Package 5 switched the constructors to `after_history`); `full_power_is_an_initial_equilibrium` is expected to FAIL on initial drift because `full_power()` settles for a fixed 600 s + 120 s. Record the printed drift values in the commit message. If it already passes, still implement Step 3 (the method is required by the master) and record "no baseline change".

- [ ] **Step 3: Implement**

Add to `impl Sim` in `sim.rs`:

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

In `full_power()` replace the final `s.settle(120.0);` (before `finalize_init`) with `s.settle_until_steady(3600.0);`. Do not change any parameter. If `fp_steady_600`'s baseline then differs, regenerate only that baseline with `--case fp_steady_600 --write` and say so in the commit message (initialisation fix, not retuning).

- [ ] **Step 4: Run tests**

Run: `cargo test --release -p bwr-core` → all pass; `cargo run --release -p bwr-core --example validation` → PASS for every case (or only `fp_steady_600` regenerated, as above).

- [ ] **Step 5: Commit**

```bash
git add crates/bwr-core/src/sim.rs crates/bwr-core/tests/scenarios.rs docs/validation/baseline
git commit -m "fix(sim): full-power initialisation settles to the initial-equilibrium limit

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 3: Startup and power-change cases

**Files:**
- Modify: `crates/bwr-core/src/validation.rs` (`standard_cases()`)
- Modify: `crates/bwr-core/tests/plant.rs` (`startup_from_hot_standby_to_30_percent` reuses the case)
- Modify: `docs/validation/cases.csv`
- Test: `crates/bwr-core/tests/scenarios.rs`

**Interfaces:**
- Consumes: Task 1 operators.
- Produces: cases `hs_startup_to_30pct`, `hs_power_change_30_to_60`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn startup_reaches_30_percent_without_scram() {
    let (trace, sim) = run_full("hs_startup_to_30pct");
    assert!(!sim.scrammed, "scram: {:?}", sim.scram_reason);
    assert!(sim.turbine.breaker_closed, "generator never synchronised");
    assert!(sim.core.kin.n > 0.29, "n {}", sim.core.kin.n);
    assert!(sim.snapshot().turbine.net_mwe > 40.0);
    assert!(trace.column("thermal_mw").last().unwrap() > &(0.29 * 870.0));
    assert!(sim.domain_errors.is_empty(), "{:?}", sim.domain_errors);
}

#[test]
fn power_change_30_to_60_percent() {
    let (trace, sim) = run_full("hs_power_change_30_to_60");
    assert!(!sim.scrammed, "scram: {:?}", sim.scram_reason);
    assert!(sim.core.kin.n > 0.58, "n {}", sim.core.kin.n);
    assert!(trace.metrics().peak["power_frac"].0 < 0.65, "overshoot above 65 %");
    assert!(sim.domain_errors.is_empty());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --release -p bwr-core --test scenarios -- startup_reaches power_change`
Expected: panic `no case hs_startup_to_30pct`.

- [ ] **Step 3: Register the cases**

In `standard_cases()` (P2's `case(...)` helper builds `Case` with `dt_after_s: None, operator: Operator::None`), append:

```rust
Case { id: "hs_startup_to_30pct", scenario: ScenarioName::HotStandby, duration_s: 3.0 * 3600.0, sample_dt_s: 10.0,
       commands: vec![], faults: vec![], dt_after_s: None, operator: Operator::Startup { target_frac_pct: 30 } },
Case { id: "hs_power_change_30_to_60", scenario: ScenarioName::HotStandby, duration_s: 4.0 * 3600.0, sample_dt_s: 10.0,
       commands: vec![], faults: vec![], dt_after_s: None, operator: Operator::PowerChange { from_pct: 30, to_pct: 60 } },
```

In `tests/plant.rs`, replace the body of `startup_from_hot_standby_to_30_percent` with:

```rust
use bwr_core::validation::{operators, run_sim_with, standard_cases};
let c = standard_cases().into_iter().find(|c| c.id == "hs_startup_to_30pct").unwrap();
let (_, sim) = run_sim_with(Sim::new(c.scenario), &c, DT, operators::startup(30));
assert!(!sim.scrammed, "scram during startup: {:?}", sim.scram_reason);
assert!(sim.turbine.breaker_closed, "generator never synchronised");
assert!(sim.core.kin.n > 0.29);
assert!(sim.snapshot().turbine.net_mwe > 40.0);
```

Append to `docs/validation/cases.csv` (Task 5 rewrites the file with the locked header; until then append with the P2 header plus `role` placed before `status` as the master requires):

```
hs_startup_to_30pct,hotStandby,10800,Normal startup: critical; heatup; roll; sync; 30 %,none available,physical,validation,baseline
hs_power_change_30_to_60,hotStandby,14400,Power increase 30 to 60 % by rod withdrawal with period guard,none available,physical,validation,baseline
```

- [ ] **Step 4: Run tests and write the baselines**

Run: `cargo test --release -p bwr-core` → all pass.
Run: `cargo run --release -p bwr-core --example validation -- --case hs_startup_to_30pct --write` and the same for `hs_power_change_30_to_60`; then `cargo run --release -p bwr-core --example validation` → all PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/bwr-core/src/validation.rs crates/bwr-core/tests/plant.rs crates/bwr-core/tests/scenarios.rs docs/validation
git commit -m "feat(validation): startup and power-change operator cases

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 4: Controlled shutdown, mode-switch shutdown, cooldown on ICs

**Files:**
- Modify: `crates/bwr-core/src/validation.rs` (three cases)
- Modify: `docs/validation/cases.csv`
- Test: `crates/bwr-core/tests/scenarios.rs`

**Interfaces:**
- Consumes: Task 1 operators; P2's existing `fp_manual_scram` case (manual scram at 5 s with mode switch to SHUTDOWN); trace column `t_coolant_c`.
- Produces: cases `fp_controlled_shutdown`, `fp_mode_switch_shutdown`, `fp_cooldown_on_ic`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn controlled_shutdown_ends_with_mode_switch_scram_and_all_rods_in() {
    let (_, sim) = run_full("fp_controlled_shutdown");
    assert!(sim.scrammed);
    assert_eq!(sim.scram_reason.as_deref(), Some("Mode switch in SHUTDOWN"));
    assert!(sim.core.rods.pos.iter().all(|&p| p <= 0.01));
    assert!(!sim.turbine.breaker_closed);
    assert!(sim.domain_errors.is_empty(), "{:?}", sim.domain_errors);
}

#[test]
fn cooldown_on_isolation_condensers_reaches_the_ics_lower_limit() {
    let (trace, sim) = run_full("fp_cooldown_on_ic");
    let t_final = sim.vessel.eos.t;
    assert!(t_final > 100.0 && t_final < 140.0, "final coolant temperature {t_final}");
    let tc = trace.column("t_coolant_c");
    let t = trace.column("t");
    let mut worst = 0.0f64;
    for k in 1..tc.len() {
        worst = worst.min((tc[k] - tc[k - 1]) / ((t[k] - t[k - 1]) / 3600.0));
    }
    // 55 degC/h administrative limit plus the throttling dead band of a once-per-second operator on a 30 s filtered rate.
    assert!(worst > -70.0, "peak cooldown rate {worst} degC/h");
    assert!(sim.domain_errors.is_empty(), "{:?}", sim.domain_errors);
    assert!(sim.ic.iter().map(|t| t.heat).sum::<f64>() < 3.0e6, "ICs still removing heat near the pool temperature");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --release -p bwr-core --test scenarios -- controlled cooldown`
Expected: panic `no case fp_controlled_shutdown`.

- [ ] **Step 3: Register the cases**

```rust
Case { id: "fp_controlled_shutdown", scenario: ScenarioName::FullPower, duration_s: 3.0 * 3600.0, sample_dt_s: 10.0,
       commands: vec![], faults: vec![], dt_after_s: None, operator: Operator::ControlledShutdown },
case("fp_mode_switch_shutdown", fp, 600.0, vec![at(5.0, SetModeSwitch { mode: ModeSwitch::Shutdown })]),
Case { id: "fp_cooldown_on_ic", scenario: ScenarioName::FullPower, duration_s: 8.0 * 3600.0, sample_dt_s: 10.0,
       commands: vec![at(5.0, Command::ManualScram), at(6.0, Command::SetModeSwitch { mode: ModeSwitch::Shutdown })],
       faults: vec![], dt_after_s: None, operator: Operator::CooldownOnIc },
```

`cases.csv` rows:

```
fp_controlled_shutdown,fullPower,10800,Rods in by notches; turbine off; mode switch to SHUTDOWN,none available,physical,validation,baseline
fp_mode_switch_shutdown,fullPower,600,Mode switch SHUTDOWN scram from full power,none available,physical,validation,baseline
fp_cooldown_on_ic,fullPower,28800,Isolated cooldown on ICs throttled to 30-50 degC/h until the IC pool-temperature limit,none available,plausibility,plausibility,baseline
```

- [ ] **Step 4: Run tests, write baselines, commit**

Run: `cargo test --release -p bwr-core --test scenarios` → PASS (finding policy applies to the final-temperature assertion). Write the three baselines with `--case <id> --write`.

```bash
git add crates/bwr-core/src/validation.rs crates/bwr-core/tests/scenarios.rs docs/validation
git commit -m "feat(validation): shutdown and IC cooldown operator cases

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 5: Full transient case register, `role` column, `--write-missing`

**Files:**
- Modify: `crates/bwr-core/src/validation.rs` (`standard_cases()`)
- Modify: `crates/bwr-core/examples/validation.rs`
- Modify: `docs/validation/cases.csv` (rewrite with the locked header), `docs/validation/method.md`
- Create: `docs/validation/baseline/<id>.csv` for every new id

**Interfaces:**
- Consumes: `Fault` variants, `Rods::strongest_rod`, P2 ids `fp_steady_600`, `fp_manual_scram`, `fp_turbine_trip`, `fp_msiv_closure`, `fp_pressure_step`, `fp_loss_fw_heating`, `fp_load_reject`, `fp_fw_pump_trip`, `hs_steady_600`, `cs_steady_600`.
- Produces: every id in the table below; `--write-missing`.

- [ ] **Step 1: Write the failing test**

```rust
const ALL_CASE_IDS: &[&str] = &[
    "fp_steady_600", "hs_steady_600", "cs_steady_600",
    "fp_manual_scram", "fp_turbine_trip", "fp_msiv_closure", "fp_pressure_step", "fp_loss_fw_heating", "fp_load_reject", "fp_fw_pump_trip",
    "hs_startup_to_30pct", "hs_power_change_30_to_60", "fp_controlled_shutdown", "fp_mode_switch_shutdown", "fp_cooldown_on_ic",
    "fp_turbine_trip_no_bypass", "fp_load_reject_no_bypass", "fp_msiv_single", "fp_pressure_reg_fail_closed", "fp_pressure_reg_fail_open",
    "fp_excess_steam_demand", "fp_feed_pump_trip_all", "fp_condensate_pump_loss", "fp_fcv_stuck", "fp_high_level", "fp_low_level",
    "fp_cw_loss", "fp_vacuum_pump_fail", "fp_rod_withdrawal_error", "fp_scram_stuck_rod",
    "fp_msiv_closure_ic1", "fp_msiv_closure_ic2", "fp_msiv_closure_ic3", "fp_inadvertent_ic", "fp_feed_loss_no_ic",
];

fn cases_csv() -> String {
    std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/validation/cases.csv")).unwrap()
}

/// Parse cases.csv by header name: id -> (column -> value).
fn cases_table() -> std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>> {
    let csv = cases_csv();
    let mut lines = csv.lines().filter(|l| !l.trim().is_empty());
    let header: Vec<&str> = lines.next().unwrap().split(',').collect();
    assert_eq!(header, ["id", "scenario", "duration_s", "purpose", "reference", "tolerance_class", "role", "status"], "cases.csv header is locked by the master");
    lines.map(|l| {
        let cells: Vec<&str> = l.splitn(header.len(), ',').collect();
        assert_eq!(cells.len(), header.len(), "bad row: {l}");
        (cells[0].to_string(), header.iter().zip(&cells).map(|(h, c)| (h.to_string(), c.to_string())).collect())
    }).collect()
}

#[test]
fn every_planned_case_is_registered_with_a_role() {
    let ids: std::collections::BTreeSet<&str> = standard_cases().iter().map(|c| c.id).collect();
    for id in ALL_CASE_IDS {
        assert!(ids.contains(id), "case {id} missing from standard_cases()");
    }
    let table = cases_table();
    for c in standard_cases() {
        let row = table.get(c.id).unwrap_or_else(|| panic!("{} missing from cases.csv", c.id));
        assert!(["calibration", "validation", "plausibility"].contains(&row["role"].as_str()), "{}: bad role {}", c.id, row["role"]);
        assert_eq!(row["duration_s"].parse::<f64>().unwrap(), c.duration_s, "{}: duration_s", c.id);
        assert!(row["status"] == "baseline" || row["status"].starts_with("ignored:F-"), "{}: bad status {}", c.id, row["status"]);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --release -p bwr-core --test scenarios -- registered`
Expected: FAIL `case fp_turbine_trip_no_bypass missing`.

- [ ] **Step 3: Register the cases**

Add two helpers next to P2's `case(...)` and the cases below. The strongest rod is deterministic (fixed layout), so compute it once:

```rust
use crate::faults::Fault;

fn faulted(id: &'static str, scenario: ScenarioName, duration_s: f64, commands: Vec<ScriptedCommand>, faults: Vec<ScriptedFault>) -> Case {
    Case { id, scenario, duration_s, sample_dt_s: 1.0, commands, faults, dt_after_s: None, operator: Operator::None }
}
fn fault(at_s: f64, fault: Fault) -> ScriptedFault {
    ScriptedFault { at_s, fault, active: true }
}

// inside standard_cases(), after `let fp = ScenarioName::FullPower;`
let strongest = crate::reactor::Rods::new(0.0).strongest_rod();
let close_both = || vec![at(5.0, SetMsiv { line: 0, open: false }), at(5.0, SetMsiv { line: 1, open: false })];
// appended to the returned Vec:
faulted("fp_turbine_trip_no_bypass", fp, 600.0, vec![at(5.0, TripTurbine)], vec![fault(0.0, Fault::BypassUnavailable)]),
faulted("fp_load_reject_no_bypass", fp, 600.0, vec![at(5.0, OpenBreaker)], vec![fault(0.0, Fault::BypassUnavailable)]),
case("fp_msiv_single", fp, 600.0, vec![at(5.0, SetMsiv { line: 0, open: false })]),
faulted("fp_pressure_reg_fail_closed", fp, 600.0, vec![], vec![fault(5.0, Fault::PressureRegulatorFailed { demand_pct: 0 })]),
faulted("fp_pressure_reg_fail_open", fp, 600.0, vec![], vec![fault(5.0, Fault::PressureRegulatorFailed { demand_pct: 100 })]),
case("fp_excess_steam_demand", fp, 600.0, vec![at(5.0, SetPressureSetpoint { mpa: 6.5 })]),
faulted("fp_feed_pump_trip_all", fp, 1800.0, vec![], (0..N_FEED_PUMPS).map(|i| fault(5.0, Fault::FeedPumpTrip { index: i })).collect()),
faulted("fp_condensate_pump_loss", fp, 1800.0, vec![], (0..N_COND_PUMPS).map(|i| fault(5.0, Fault::CondensatePumpTrip { index: i })).collect()),
faulted("fp_fcv_stuck", fp, 900.0, vec![at(60.0, TripTurbine)], vec![fault(5.0, Fault::FcvStuck)]),
case("fp_high_level", fp, 900.0, vec![at(5.0, SetLevelControl { mode: LevelControlMode::Manual }), at(6.0, SetFcvManual { pos: 1.0 })]),
case("fp_low_level", fp, 1800.0, vec![at(5.0, SetLevelControl { mode: LevelControlMode::Manual }), at(6.0, SetFcvManual { pos: 0.0 })]),
faulted("fp_cw_loss", fp, 900.0, vec![], (0..N_CW_PUMPS).map(|i| fault(5.0, Fault::CwPumpTrip { index: i })).collect()),
faulted("fp_vacuum_pump_fail", fp, 2.0 * 3600.0, vec![], vec![fault(5.0, Fault::VacuumPumpFailed)]),
Case { id: "fp_rod_withdrawal_error", scenario: fp, duration_s: 300.0, sample_dt_s: 1.0, commands: vec![], faults: vec![],
       dt_after_s: None, operator: Operator::RodWithdrawalError },
faulted("fp_scram_stuck_rod", fp, 120.0, vec![at(5.0, ManualScram)], vec![fault(0.0, Fault::RodStuck { index: strongest })]),
faulted("fp_msiv_closure_ic1", fp, 3600.0, close_both(), vec![fault(0.0, Fault::IcTrainUnavailable { train: 1 }), fault(0.0, Fault::IcTrainUnavailable { train: 2 })]),
faulted("fp_msiv_closure_ic2", fp, 3600.0, close_both(), vec![fault(0.0, Fault::IcTrainUnavailable { train: 2 })]),
case("fp_msiv_closure_ic3", fp, 3600.0, close_both()),
case("fp_inadvertent_ic", fp, 600.0, vec![at(5.0, SetIcTrain { index: 0, open: true })]),
faulted("fp_feed_loss_no_ic", fp, 3600.0, vec![],
    (0..N_FEED_PUMPS).map(|i| fault(5.0, Fault::FeedPumpTrip { index: i }))
        .chain((0..N_IC_TRAINS).map(|t| fault(0.0, Fault::IcTrainUnavailable { train: t }))).collect()),
```

`PressureRegulatorFailed { demand_pct: 100 }` is the fail-open bound because `Fault::is_valid` rejects `demand_pct > 100` (master); the regulator at 100 % with the bypass still in automatic drives steam flow above the rated turbine admission, which is the fail-open consequence under test.

- [ ] **Step 4: `--write-missing` in `examples/validation.rs`**

In P2's `main`, after `let write = ...;` add `let write_missing = args.iter().any(|a| a == "--write-missing");` and change the branch `if write {` to:

```rust
let missing = !path.exists();
if write || (write_missing && missing) {
```

Update the doc comment at the top of the file:

```rust
//! cargo run --release -p bwr-core --example validation -- --write-missing   # write baselines only for cases that have none
```

- [ ] **Step 5: Rewrite `docs/validation/cases.csv` with the locked header**

Final content. Purposes of P2 rows are kept verbatim; `role` is inserted before `status`.

```csv
id,scenario,duration_s,purpose,reference,tolerance_class,role,status
fp_steady_600,fullPower,600,Rated-power steady state: initial equilibrium residual and drift,none (regression),numerical,calibration,baseline
fp_manual_scram,fullPower,300,Manual scram with mode switch to SHUTDOWN: power decay and level shrink without isolation,none (regression),numerical,validation,baseline
fp_turbine_trip,fullPower,300,Turbine trip at rated power: bypass ride-through and Select Rod Insert,ESBWR DCD R10 T2 15.2 turbine trip (mechanism only; Task 9),numerical,validation,baseline
fp_msiv_closure,fullPower,600,Closure of both MSIVs: scram and isolation condenser initiation,ESBWR DCD R10 T2 15.2 MSIV closure (mechanism only; Task 9),numerical,validation,baseline
fp_pressure_step,fullPower,60,+0.1 MPa pressure setpoint step: void collapse reactivity response,none (regression),numerical,validation,baseline
fp_loss_fw_heating,fullPower,300,Feedwater heaters removed from service: cold feed reactivity response,ESBWR DCD R10 T2 15.2 LOFWH (mechanism only; Task 9),numerical,validation,baseline
fp_load_reject,fullPower,300,Generator breaker opened at rated load: speed excursion and bypass,ESBWR DCD R10 T2 15.2 load rejection (mechanism only; Task 9),numerical,validation,baseline
fp_fw_pump_trip,fullPower,300,One of two running feed pumps stopped: level control recovery,none (regression),numerical,validation,baseline
hs_steady_600,hotStandby,600,Hot standby steady state: decay heat balance and level drift,none (regression),numerical,calibration,baseline
cs_steady_600,coldShutdown,600,Cold shutdown steady state: vented vessel and 30-day decay heat,none (regression),numerical,calibration,baseline
hs_startup_to_30pct,hotStandby,10800,Normal startup: critical; heatup; roll; sync; 30 %,none available,physical,validation,baseline
hs_power_change_30_to_60,hotStandby,14400,Power increase 30 to 60 % by rod withdrawal with period guard,none available,physical,validation,baseline
fp_controlled_shutdown,fullPower,10800,Rods in by notches; turbine off; mode switch to SHUTDOWN,none available,physical,validation,baseline
fp_mode_switch_shutdown,fullPower,600,Mode switch SHUTDOWN scram from full power,none available,physical,validation,baseline
fp_cooldown_on_ic,fullPower,28800,Isolated cooldown on ICs throttled to 30-50 degC/h until the IC pool-temperature limit,none available,plausibility,plausibility,baseline
fp_turbine_trip_no_bypass,fullPower,600,Turbine trip with bypass unavailable: high pressure scram and IC initiation,none available,physical,validation,baseline
fp_load_reject_no_bypass,fullPower,600,Load rejection with bypass unavailable,none available,physical,validation,baseline
fp_msiv_single,fullPower,600,Closure of one of two main steam lines,none available,physical,validation,baseline
fp_pressure_reg_fail_closed,fullPower,600,Pressure regulator output fails to zero demand,none available,physical,validation,baseline
fp_pressure_reg_fail_open,fullPower,600,Pressure regulator output fails to 100 % demand with bypass in automatic,none available,physical,validation,baseline
fp_excess_steam_demand,fullPower,600,Setpoint step to 6.5 MPa: high steam flow isolation,none available,physical,validation,baseline
fp_feed_pump_trip_all,fullPower,1800,Loss of all feedwater: L3 scram; L2 isolation; ICs,none available,physical,validation,baseline
fp_condensate_pump_loss,fullPower,1800,Loss of both condensate pumps: feed pump low-suction trip,none available,physical,validation,baseline
fp_fcv_stuck,fullPower,900,Feed control valve stuck then turbine trip: high level response,none available,physical,validation,baseline
fp_high_level,fullPower,900,Manual overfeed to L8: turbine and feed pump trips,none available,physical,validation,baseline
fp_low_level,fullPower,1800,Manual underfeed to L3/L2: scram; isolation; ICs,none available,physical,validation,baseline
fp_cw_loss,fullPower,900,Loss of circulating water: vacuum loss; turbine trip; bypass block,none available,physical,validation,baseline
fp_vacuum_pump_fail,fullPower,7200,Vacuum pump failure: slow condenser pressure rise,none available,plausibility,plausibility,baseline
fp_rod_withdrawal_error,fullPower,300,Continuous withdrawal of the strongest rod at power,none available,physical,validation,baseline
fp_scram_stuck_rod,fullPower,120,Manual scram with the strongest rod stuck,none available,physical,validation,baseline
fp_msiv_closure_ic1,fullPower,3600,Isolation with one IC train available (record only),none available,plausibility,plausibility,baseline
fp_msiv_closure_ic2,fullPower,3600,Isolation with two IC trains available,none available,physical,validation,baseline
fp_msiv_closure_ic3,fullPower,3600,Isolation with three IC trains available,none available,physical,validation,baseline
fp_inadvertent_ic,fullPower,600,Inadvertent initiation of one IC train at power,none available,plausibility,plausibility,baseline
fp_feed_loss_no_ic,fullPower,3600,Loss of feedwater with no IC credited: deliberately leaves the model envelope,none available,plausibility,plausibility,baseline
```

Append to `docs/validation/method.md`:

```
## Case roles and status

- role `calibration`: used to fix `k_fric`, `TURBINE_DH_RATED` and the critical rod configuration at initialisation; never cited as validation evidence.
- role `validation`: independent of calibration; compared to a public reference where one exists, otherwise to ordering, bounds and conservation.
- role `plausibility`: no public reference and no bound asserted; the trace is recorded and reported.
- status `baseline`: a regression baseline exists and the case's scenario assertions are active.
- status `ignored:F-nnn`: a physical assertion failed; the finding is open in findings.md and the assertion is not enforced until the owning package resolves it.
```

- [ ] **Step 6: Run and write baselines**

Run: `cargo test --release -p bwr-core --test scenarios -- registered` → PASS.
Run: `cargo run --release -p bwr-core --example validation -- --write-missing`
Expected: `WROTE` for each new id, `PASS` for every pre-existing id. Then `cargo run --release -p bwr-core --example validation` → all PASS, exit 0.

- [ ] **Step 7: Commit**

```bash
git add crates/bwr-core/src/validation.rs crates/bwr-core/examples/validation.rs crates/bwr-core/tests/scenarios.rs docs/validation
git commit -m "feat(validation): full transient case register with roles and --write-missing

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---
