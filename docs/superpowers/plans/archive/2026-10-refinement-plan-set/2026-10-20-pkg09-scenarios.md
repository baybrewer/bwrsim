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
    // Precondition: hot standby is deeply subcritical (k_eff < 0.95); 15 min of banked withdrawal only
    // raises the source-multiplied level, so assert a rise, not a decade.
    assert!(sim.core.kin.n > Sim::new(ScenarioName::HotStandby).core.kin.n, "power did not rise");
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
hs_startup_to_30pct,hotStandby,10800,Normal startup: critical; heatup; roll; sync; 30 %,none available,physical,plausibility,baseline
hs_power_change_30_to_60,hotStandby,14400,Power increase 30 to 60 % by rod withdrawal with period guard,none available,physical,plausibility,baseline
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
fp_controlled_shutdown,fullPower,10800,Rods in by notches; turbine off; mode switch to SHUTDOWN,none available,physical,plausibility,baseline
fp_mode_switch_shutdown,fullPower,600,Mode switch SHUTDOWN scram from full power,none available,physical,plausibility,baseline
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
- Consumes: `Fault` variants, `Rods::strongest_rod`, `N_FEED_PUMPS`, `N_COND_PUMPS`, `N_CW_PUMPS`, `N_IC_TRAINS`, `N_STEAM_LINES`, `RWCU_REJECT_MAX_KGS` (P6), P2 ids `fp_steady_600`, `fp_manual_scram`, `fp_turbine_trip`, `fp_msiv_closure`, `fp_pressure_step`, `fp_loss_fw_heating`, `fp_load_reject`, `fp_fw_pump_trip`, `hs_steady_600`, `cs_steady_600`.
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
        if row["role"] == "validation" {
            // Spec: only a transcribed public reference earns the validation label.
            let reference = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/validation/reference/").to_string() + c.id + ".csv";
            assert!(std::path::Path::new(&reference).exists(), "{}: role validation without {reference}", c.id);
            assert!(!row["reference"].starts_with("none"), "{}: role validation with reference '{}'", c.id, row["reference"]);
            assert_eq!(row["tolerance_class"], "source-comparison", "{}", c.id);
        }
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
faulted("fp_pressure_reg_fail_open", fp, 600.0, vec![at(10.0, TripTurbine)], vec![fault(5.0, Fault::PressureRegulatorFailed { demand_pct: 100 })]),
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
Case { id: "fp_feed_loss_no_ic", scenario: fp, duration_s: 6.0 * 3600.0, sample_dt_s: 10.0,
       commands: vec![at(5.0, SetRwcuReject { kgs: RWCU_REJECT_MAX_KGS })],
       faults: (0..N_FEED_PUMPS).map(|i| fault(5.0, Fault::FeedPumpTrip { index: i }))
           .chain((0..N_IC_TRAINS).map(|t| fault(0.0, Fault::IcTrainUnavailable { train: t })))
           .chain((0..N_STEAM_LINES).map(|l| fault(0.0, Fault::MsivStuck { line: l }))).collect(),
       dt_after_s: None, operator: Operator::None },
```

`PressureRegulatorFailed { demand_pct: 100 }` is the fail-open bound because `Fault::is_valid` rejects `demand_pct > 100` (master). At rated power the regulator already demands 100 % (`pressure_demand` = (7.2 - 6.95)/0.25 × rated), so a frozen-high output is only consequential once steam generation falls: the turbine trip at 10 s (SRI cuts power) leaves the bypass held wide open by the frozen demand, pressure falls, and the RUN-mode low steam-line-pressure isolation follows. Demand above rated is covered by `fp_excess_steam_demand`.

**Preconditions (checked against `sim.rs` scenario constructors and protection logic).** Every `fp_*` case starts in RUN with the breaker closed (`Turbine::new(true)`), condenser vacuum and both CW pumps on, feed pumps 0 and 1 and both condensate pumps running, three-element level control at 0 cm, head vent closed, ICs closed, rods banked near position 34 (about 28 % of rod worth inserted by the critical search, highest-worth rods first), RWCU reject 0. Every `hs_*` case starts in STARTUP, all rods in, k_eff < 0.95, 6.9 MPa, condenser vacuum on, one condensate and one feed pump, single-element control, turbine tripped. `cs_*` starts vented at 40 degC with no pumps and no vacuum.

| Case | Precondition / why the purpose is reachable |
|---|---|
| hs_startup_to_30pct, hs_power_change_30_to_60 | Condenser vacuum and bypass regulator available from hot standby; the script is the baseline `tests/plant.rs` script that already reaches 30 % in under 3 h. |
| fp_controlled_shutdown | Insertion is never rod-blocked; RUN is left at n < 0.12, below the 0.15 APRM setdown scram; IRM range 10 keeps the IRM reading at 30 % there; turbine load is below 10 MW near n = 0.05 so the breaker opens before reverse power. |
| fp_cooldown_on_ic | After the scram the vessel is still saturated, so closing both MSIVs at 60 s raises `isolated_at_power` and all three trains auto-initiate; a train the operator closes stays closed because the auto-initiation edge is one-shot and neither 7.6 MPa nor L2 recurs. |
| fp_turbine_trip_no_bypass, fp_load_reject_no_bypass | `BypassUnavailable` at 0 s precedes the trip; with no steam path pressure must reach 7.5 MPa (scram) and 7.6 MPa (IC). |
| fp_msiv_single | Average MSIV position 0.5 < 0.85 raises the RUN MSIV-closure scram during the 4 s stroke. This row encodes `protection.csv` row `msiv_closure_scram`; if Package 8 sources a both-lines logic, change the expectation to `order: &[]` and cite that row. |
| fp_pressure_reg_fail_closed | Demand 0 closes TCV and bypass; no steam path, as above. |
| fp_pressure_reg_fail_open | See the paragraph above this table. |
| fp_excess_steam_demand | Setpoint 6.5 MPa gives demand 2.8 × rated; TCV at the 1.05 load limit plus bypass exceeds 1.4 × rated only if the Package 6 bypass capacity is above 35 % of rated; below that the case reaches the low steam-line-pressure isolation instead. The expected order (isolation, then scram) holds either way; the purpose names both. |
| fp_feed_pump_trip_all, fp_condensate_pump_loss | Pumps 0 and 1 are running, so the trips are consequential; with no feed, L3 (-100 cm) is reached in about 15 s at rated steam flow, scram shrink then reaches L2. Condensate loss trips the feed pumps after the 3 s low-suction timer. |
| fp_fcv_stuck | FCV frozen near 0.7 before the trip; SRI cuts steam generation while feed stays near rated, so level reaches L8 in about a minute; the Package 8 independent L8 feed-pump trip then empties the vessel to L3. |
| fp_high_level | Manual mode is bumpless, so the 1.0 command ramps the valve at 0.1/s; RWCU reject is 0 and nothing else preempts L8. |
| fp_low_level | In manual the controller cannot refill; the valve closes in 7 s. |
| fp_cw_loss | With no CW flow the condenser relaxes toward 1.2 atm with a 120 s constant: 25 kPa (turbine trip) at about 22 s, 30 kPa (bypass block) at about 27 s, then no steam path. |
| fp_vacuum_pump_fail | Vacuum pump is on at full power; the 0.004 kg/s in-leakage raises air partial pressure by under 1 kPa in 2 h, so the purpose is the slow rise only. |
| fp_rod_withdrawal_error | The strongest rod is at about position 34, not full out; n starts at 1.0, below the 1.08 RUN withdraw block, so withdrawal proceeds until the block or the 1.18 scram. |
| fp_scram_stuck_rod | Stuck fault precedes the scram; the rod is partially inserted, so "unchanged" is a real check. |
| fp_msiv_closure_ic1/2/3 | Unavailable trains are set before the isolation; one train (about 34 MW) is below the initial decay heat (about 52 MW), hence record-only for ic1. |
| fp_inadvertent_ic | Trains are closed at full power; the operator command is not interlocked. |
| fp_feed_loss_no_ic | See the case definition: MSIVs are held open by `MsivStuck` so the L2 isolation cannot seal the vessel, and RWCU reject at its cap accelerates the drain; without those the sealed vessel heats at under 50 K/h, leaves the envelope only after about 2 h if at all, and never exhausts. |

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
fp_manual_scram,fullPower,300,Manual scram with mode switch to SHUTDOWN: power decay and level shrink without isolation,none (regression),numerical,plausibility,baseline
fp_turbine_trip,fullPower,300,Turbine trip at rated power: bypass ride-through and Select Rod Insert,none available (ESBWR DCD R10 T2 15.2 turbine trip is a candidate; nothing transcribed yet; Task 9),numerical,plausibility,baseline
fp_msiv_closure,fullPower,600,Closure of both MSIVs: scram and isolation condenser initiation,none available (ESBWR DCD R10 T2 15.2 MSIV closure is a candidate; nothing transcribed yet; Task 9),numerical,plausibility,baseline
fp_pressure_step,fullPower,60,+0.1 MPa pressure setpoint step: void collapse reactivity response,none (regression),numerical,plausibility,baseline
fp_loss_fw_heating,fullPower,300,Feedwater heaters removed from service: cold feed reactivity response,none available (ESBWR DCD R10 T2 15.2 LOFWH is a candidate; nothing transcribed yet; Task 9),numerical,plausibility,baseline
fp_load_reject,fullPower,300,Generator breaker opened at rated load: speed excursion and bypass,none available (ESBWR DCD R10 T2 15.2 load rejection is a candidate; nothing transcribed yet; Task 9),numerical,plausibility,baseline
fp_fw_pump_trip,fullPower,300,One of two running feed pumps stopped: level control recovery,none (regression),numerical,plausibility,baseline
hs_steady_600,hotStandby,600,Hot standby steady state: decay heat balance and level drift,none (regression),numerical,calibration,baseline
cs_steady_600,coldShutdown,600,Cold shutdown steady state: vented vessel and 30-day decay heat,none (regression),numerical,calibration,baseline
hs_startup_to_30pct,hotStandby,10800,Normal startup: critical; heatup; roll; sync; 30 %,none available,physical,plausibility,baseline
hs_power_change_30_to_60,hotStandby,14400,Power increase 30 to 60 % by rod withdrawal with period guard,none available,physical,plausibility,baseline
fp_controlled_shutdown,fullPower,10800,Rods in by notches; turbine off; mode switch to SHUTDOWN,none available,physical,plausibility,baseline
fp_mode_switch_shutdown,fullPower,600,Mode switch SHUTDOWN scram from full power,none available,physical,plausibility,baseline
fp_cooldown_on_ic,fullPower,28800,Isolated cooldown on ICs throttled to 30-50 degC/h until the IC pool-temperature limit,none available,plausibility,plausibility,baseline
fp_turbine_trip_no_bypass,fullPower,600,Turbine trip with bypass unavailable: high pressure scram and IC initiation,none available,physical,plausibility,baseline
fp_load_reject_no_bypass,fullPower,600,Load rejection with bypass unavailable,none available,physical,plausibility,baseline
fp_msiv_single,fullPower,600,Closure of one of two main steam lines,none available,physical,plausibility,baseline
fp_pressure_reg_fail_closed,fullPower,600,Pressure regulator output fails to zero demand,none available,physical,plausibility,baseline
fp_pressure_reg_fail_open,fullPower,600,Pressure regulator output frozen at 100 % demand then turbine trip: bypass held open; low steam-line-pressure isolation,none available,physical,plausibility,baseline
fp_excess_steam_demand,fullPower,600,Setpoint step to 6.5 MPa: high steam flow or low steam-line-pressure isolation then scram,none available,physical,plausibility,baseline
fp_feed_pump_trip_all,fullPower,1800,Loss of all feedwater: L3 scram; L2 isolation; ICs,none available,physical,plausibility,baseline
fp_condensate_pump_loss,fullPower,1800,Loss of both condensate pumps: feed pump low-suction trip,none available,physical,plausibility,baseline
fp_fcv_stuck,fullPower,900,Feed control valve stuck then turbine trip: high level response,none available,physical,plausibility,baseline
fp_high_level,fullPower,900,Manual overfeed to L8: turbine and feed pump trips,none available,physical,plausibility,baseline
fp_low_level,fullPower,1800,Manual underfeed to L3/L2: scram; isolation; ICs,none available,physical,plausibility,baseline
fp_cw_loss,fullPower,900,Loss of circulating water: vacuum loss; turbine trip; bypass block,none available,physical,plausibility,baseline
fp_vacuum_pump_fail,fullPower,7200,Vacuum pump failure: slow condenser pressure rise,none available,plausibility,plausibility,baseline
fp_rod_withdrawal_error,fullPower,300,Continuous withdrawal of the strongest rod at power,none available,physical,plausibility,baseline
fp_scram_stuck_rod,fullPower,120,Manual scram with the strongest rod stuck,none available,physical,plausibility,baseline
fp_msiv_closure_ic1,fullPower,3600,Isolation with one IC train available (record only),none available,plausibility,plausibility,baseline
fp_msiv_closure_ic2,fullPower,3600,Isolation with two IC trains available,none available,physical,plausibility,baseline
fp_msiv_closure_ic3,fullPower,3600,Isolation with three IC trains available,none available,physical,plausibility,baseline
fp_inadvertent_ic,fullPower,600,Inadvertent initiation of one IC train at power,none available,plausibility,plausibility,baseline
fp_feed_loss_no_ic,fullPower,21600,Loss of feedwater with no IC credited and MSIVs held open: deliberately leaves the model envelope through core uncovery to inventory exhaustion,none available,plausibility,plausibility,baseline
```

Append to `docs/validation/method.md`:

```
## Case roles and status

- role `calibration`: used to fix `k_fric`, `TURBINE_DH_RATED` and the critical rod configuration at initialisation; never cited as evidence of anything.
- role `validation`: only a case with a transcribed public reference under `docs/validation/reference/<id>.csv` (Task 9). When that reference is ESBWR, every report of the case carries the label "ESBWR reference configuration mechanism check, not BWRX acceptance". No case has this role until such a file exists.
- role `plausibility`: everything else. Ordering, bounds, conservation, analytical solutions, sensitivity and convergence are verification or plausibility evidence under the spec, never quantitative BWRX transient validation, even when a test asserts them.
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

### Task 6: Expected-behaviour table: actuation order, extrema, support envelope, transfer allocation

Evidence class: qualitative response ordering, bounds and conservation. This is verification and plausibility evidence under the spec, not BWRX transient validation; every case in this table has role `plausibility` unless Task 9 later attaches a public reference.

**Files:**
- Test: `crates/bwr-core/tests/scenarios.rs`
- Create (only on a failure): `docs/validation/findings.md`

**Interfaces:**
- Consumes: `Metrics::{first_event_time, peak, min}`, `Sim.domain_errors`, `DomainError`, `VESSEL_DESIGN_PRESSURE`, `L2_CM`, `Sim.ledger.integrated`, `SteamSystem.line.mass`, `Condenser::{hotwell_mass, cst_mass}`, `IcTrain.tube_mass`, `Vessel::{mass, head_vent_open}`.
- Produces: `mod ev`, `event_order`.

- [ ] **Step 1: Event keys and helpers**

```rust
use bwr_core::validation::{Metrics, Trace};

/// Package 2 event keys: the text before the first ':' of every `Sim.log` line.
mod ev {
    pub const SCRAM: &str = "scram";
    pub const TURBINE_TRIP: &str = "turbine trip";
    pub const ISOLATION: &str = "msiv isolation";
    pub const IC_INIT: &str = "ic auto-initiation";
    pub const FW_TRIP: &str = "feed pump trip";
    pub const DOMAIN: &str = "domain error";
}

/// Each key must have occurred, and first occurrences must be in nondecreasing time order.
fn event_order(m: &Metrics, keys: &[&str]) -> Result<(), String> {
    let mut last = f64::NEG_INFINITY;
    for k in keys {
        let t = *m.first_event_time.get(*k).ok_or_else(|| format!("event '{k}' never occurred; events {:?}", m.first_event_time))?;
        if t < last {
            return Err(format!("event '{k}' at {t:.2} s precedes the previous key at {last:.2} s"));
        }
        last = t;
    }
    Ok(())
}
```

- [ ] **Step 2: Expectation table**

```rust
struct Expect {
    id: &'static str,
    order: &'static [&'static str],
    no_scram: bool,
    pressure_below_design: bool,
    supported: bool,
    /// `DomainError::kind`-style prefixes accepted when !supported (checked with `format!("{d:?}")`).
    expect_domain: &'static [&'static str],
}

const EXPECTATIONS: &[Expect] = &[
    Expect { id: "fp_turbine_trip", order: &[ev::TURBINE_TRIP], no_scram: true, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_turbine_trip_no_bypass", order: &[ev::TURBINE_TRIP, ev::SCRAM, ev::IC_INIT], no_scram: false, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_load_reject", order: &[], no_scram: true, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_load_reject_no_bypass", order: &[ev::SCRAM], no_scram: false, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_msiv_single", order: &[ev::SCRAM], no_scram: false, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_msiv_closure", order: &[ev::SCRAM, ev::IC_INIT], no_scram: false, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_pressure_step", order: &[], no_scram: true, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_pressure_reg_fail_closed", order: &[ev::SCRAM, ev::IC_INIT], no_scram: false, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_pressure_reg_fail_open", order: &[ev::TURBINE_TRIP, ev::ISOLATION, ev::SCRAM], no_scram: false, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_excess_steam_demand", order: &[ev::ISOLATION, ev::SCRAM], no_scram: false, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_fw_pump_trip", order: &[], no_scram: false, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_feed_pump_trip_all", order: &[ev::SCRAM, ev::ISOLATION, ev::IC_INIT], no_scram: false, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_condensate_pump_loss", order: &[ev::FW_TRIP, ev::SCRAM, ev::ISOLATION, ev::IC_INIT], no_scram: false, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_fcv_stuck", order: &[ev::TURBINE_TRIP, ev::FW_TRIP], no_scram: false, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_loss_fw_heating", order: &[], no_scram: true, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_high_level", order: &[ev::TURBINE_TRIP, ev::FW_TRIP], no_scram: false, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_low_level", order: &[ev::SCRAM, ev::ISOLATION, ev::IC_INIT], no_scram: false, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_cw_loss", order: &[ev::TURBINE_TRIP, ev::SCRAM, ev::IC_INIT], no_scram: false, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_vacuum_pump_fail", order: &[], no_scram: true, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_rod_withdrawal_error", order: &[], no_scram: false, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_scram_stuck_rod", order: &[ev::SCRAM], no_scram: false, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_msiv_closure_ic1", order: &[ev::SCRAM, ev::IC_INIT], no_scram: false, pressure_below_design: false, supported: true, expect_domain: &[] },
    Expect { id: "fp_msiv_closure_ic2", order: &[ev::SCRAM, ev::IC_INIT], no_scram: false, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_msiv_closure_ic3", order: &[ev::SCRAM, ev::IC_INIT], no_scram: false, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_inadvertent_ic", order: &[], no_scram: true, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_manual_scram", order: &[ev::SCRAM], no_scram: false, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_mode_switch_shutdown", order: &[ev::SCRAM], no_scram: false, pressure_below_design: true, supported: true, expect_domain: &[] },
    Expect { id: "fp_feed_loss_no_ic", order: &[ev::SCRAM, ev::ISOLATION, ev::DOMAIN], no_scram: false, pressure_below_design: false, supported: false,
             expect_domain: &["FuelModelEnvelope", "InventoryExhausted", "EosOutOfRange"] },  // core uncovers before the vessel empties
];

/// Rows moved here under the finding policy are printed but not asserted. Keep the `// F-nnn` comment on each.
const IGNORED_EXPECTATIONS: &[Expect] = &[];

fn check_expectation(e: &Expect) -> Vec<String> {
    let (trace, sim) = run_full(e.id);
    let m = trace.metrics();
    let mut errs = Vec::new();
    if let Err(s) = event_order(&m, e.order) { errs.push(s); }
    if e.no_scram && m.first_event_time.contains_key(ev::SCRAM) { errs.push(format!("unexpected scram: {:?}", sim.scram_reason)); }
    let p_max = m.peak["pressure_pa"].0;
    if e.pressure_below_design && p_max >= VESSEL_DESIGN_PRESSURE { errs.push(format!("peak pressure {:.3} MPa >= design", p_max / 1e6)); }
    if e.supported && !sim.domain_errors.is_empty() { errs.push(format!("unsupported state: {:?}", sim.domain_errors)); }
    if !e.supported {
        match sim.domain_errors.first() {
            None => errs.push("expected a domain error, none raised".into()),
            Some((t, d)) => {
                let name = format!("{d:?}");
                if !e.expect_domain.iter().any(|v| name.starts_with(v)) { errs.push(format!("domain error {name} at {t:.1} s not in {:?}", e.expect_domain)); }
            }
        }
    }
    if m.nonfinite { errs.push("nonfinite value in trace".into()); }
    println!("{:28} peak p {:.3} MPa  min level {:.0} cm  events {:?}", e.id, p_max / 1e6, m.min["level_nr_cm"].0, m.first_event_time);
    errs
}

#[test]
fn transient_expectations() {
    let failures: Vec<String> = EXPECTATIONS.iter().map(|e| (e.id, check_expectation(e))).filter(|(_, v)| !v.is_empty()).map(|(id, v)| format!("{id}: {}", v.join(" | "))).collect();
    for e in IGNORED_EXPECTATIONS { println!("IGNORED {}: {:?}", e.id, check_expectation(e)); }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

#[test]
fn manual_scram_keeps_level_above_l2_and_ics_closed() {
    let (trace, sim) = run_full("fp_manual_scram");
    let m = trace.metrics();
    assert!(m.min["level_nr_cm"].0 > L2_CM);
    assert!(!m.first_event_time.contains_key(ev::ISOLATION) && !m.first_event_time.contains_key(ev::IC_INIT));
    assert!(sim.core.rods.pos.iter().all(|&p| p <= 0.01));
}

#[test]
fn stuck_rod_scram_reports_shutdown_margin() {
    let (_, sim) = run_full("fp_scram_stuck_rod");
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

- [ ] **Step 3: Transfer allocation through exhaustion (generalises the P3 coupled regression)**

```rust
use bwr_core::domain::DomainError;

/// Mass held by every component that exchanges mass with the vessel.
fn plant_mass(sim: &Sim) -> f64 {
    sim.vessel.mass + sim.steam.line.mass + sim.cond.hotwell_mass + sim.cond.cst_mass + sim.ic.iter().map(|t| t.tube_mass).sum::<f64>()
}

#[test]
fn hotwell_and_cst_gain_exactly_what_the_vessel_loses_through_exhaustion() {
    let c = case("fp_feed_loss_no_ic");
    let sim0 = Sim::new(c.scenario);
    assert!(!sim0.vessel.head_vent_open, "closed head vent so the only external mass path is zero");
    let m0 = plant_mass(&sim0);
    let mut worst = (0.0f64, 0.0f64);
    let (_, sim) = run_sim_with(sim0, &c, DT, |s: &Sim, t: f64| {
        let vent = s.ledger.integrated.get("vent_kg").copied().unwrap_or(0.0);
        let d = plant_mass(s) + vent - m0;
        if d.abs() > worst.0.abs() { worst = (d, t); }
        vec![]
    });
    assert!(sim.domain_errors.iter().any(|(_, d)| matches!(d, DomainError::InventoryExhausted { .. })), "case must reach exhaustion: {:?}", sim.domain_errors);
    assert!(worst.0.abs() < 1.0, "plant mass changed by {:.3} kg at t = {:.0} s: a consumer used a requested, not applied, transfer", worst.0, worst.1);
    assert!(sim.ledger.mass_residual(sim.vessel.mass).abs() < 1e-6 * m0);
    assert_eq!(sim.ledger.mass_clipped_kg, 0.0);
}
```

- [ ] **Step 4: Run**

Run: `cargo test --release -p bwr-core --test scenarios -- transient_expectations manual_scram stuck_rod exhaustion`
Expected: PASS. For a failing physical assertion apply the finding policy: create `docs/validation/findings.md` with the header `# Validation findings` and one entry:

```
## F-001 fp_turbine_trip_no_bypass: peak pressure 8.71 MPa >= design
Observed: ... Suspected cause: ... Package to revisit: 6/7. Enforcement: row moved to IGNORED_EXPECTATIONS; cases.csv status ignored:F-001.
```

`hotwell_and_cst_gain_exactly...` is a numerical assertion: a failure is a defect in the transfer-allocation step (master step 9) and is fixed in `sim.rs`, never ignored.

- [ ] **Step 5: Commit**

```bash
git add crates/bwr-core/tests/scenarios.rs docs/validation
git commit -m "test(scenarios): actuation order, bounds, envelope and transfer allocation per case

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 7: Fault overrides persist, are honoured, and replace by (kind, equipment)

**Files:**
- Test: `crates/bwr-core/tests/scenarios.rs`

**Interfaces:**
- Consumes: all 16 `Fault` variants, `Fault::{kind, equipment, is_valid}`, `Sim::{set_fault, has_fault, find_fault, faults, bop_faults}`, `SteamSystem::{msiv_pos, tsv_pos, tcv_pos, bypass_pos, pressure_demand(p, &SteamFaults)}`, `Feedwater::{pumps, cond_pumps, fcv_pos, t_feed}`, `Condenser::{cw_pumps, m_air}`, `IcTrain::{heat, in_service, return_valve_pos, steam_valve_pos, ua_factor}`, `Rods.pos`.

- [ ] **Step 1: Table-driven persistence test**

```rust
use bwr_core::bop::LevelControlMode;
use bwr_core::faults::Fault;
use bwr_core::validation::{ScriptedCommand, ScriptedFault};

type Check = Box<dyn Fn(&Sim, &Sim) -> Result<(), String>>;
fn ok_if(cond: bool, msg: String) -> Result<(), String> { if cond { Ok(()) } else { Err(msg) } }

/// (fault, commands automatic logic or an operator would use to undo it, check(initial, final))
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
        // Demand 50 % at rated power raises pressure to the 7.5 MPa scram, after which steam generation collapses;
        // the invariant is the frozen regulator OUTPUT, read through the Package 6 fault view, not the achieved flow.
        (Fault::PressureRegulatorFailed { demand_pct: 50 }, vec![],
         Box::new(|_, s| { let (sf, _, _) = s.bop_faults(); let d = s.steam.pressure_demand(s.vessel.eos.p, &sf);
                           ok_if((d - 0.5 * STEAM_FLOW_RATED).abs() < 1e-6, format!("regulator demand {d} not frozen at 50 %")) })),
        (Fault::FeedPumpTrip { index: 0 }, vec![at(5.0, Command::SetFeedPump { index: 0, on: true })],
         Box::new(|_, s| ok_if(!s.fw.pumps[0], "feed pump 0 restarted".into()))),
        (Fault::CondensatePumpTrip { index: 0 }, vec![at(5.0, Command::SetCondensatePump { index: 0, on: true })],
         Box::new(|_, s| ok_if(!s.fw.cond_pumps[0], "condensate pump 0 restarted".into()))),
        (Fault::FcvStuck, vec![at(5.0, Command::SetLevelControl { mode: LevelControlMode::Manual }), at(6.0, Command::SetFcvManual { pos: 0.0 })],
         Box::new(|s0, s| ok_if((s.fw.fcv_pos - s0.fw.fcv_pos).abs() < 1e-6, format!("FCV moved {} -> {}", s0.fw.fcv_pos, s.fw.fcv_pos)))),
        // Package 6 may keep `heaters_in_service` true and override the heating in the fault view, so check the effect.
        (Fault::HeatersLost, vec![at(5.0, Command::SetHeaters { in_service: true })],
         Box::new(|s0, s| ok_if(s.fw.t_feed < s0.fw.t_feed - 20.0, format!("feed temperature {} did not fall from {}", s.fw.t_feed, s0.fw.t_feed)))),
        (Fault::CwPumpTrip { index: 0 }, vec![at(5.0, Command::SetCwPump { index: 0, on: true })],
         Box::new(|_, s| ok_if(!s.cond.cw_pumps[0], "CW pump 0 restarted".into()))),
        // At rated conditions in-leakage balances removal; with the pump failed the air inventory must grow.
        (Fault::VacuumPumpFailed, vec![at(5.0, Command::SetVacuumPump { on: true })],
         Box::new(|s0, s| ok_if(s.cond.m_air > s0.cond.m_air * 1.05, format!("condenser air {} did not grow from {}", s.cond.m_air, s0.cond.m_air)))),
        (Fault::IcTrainUnavailable { train: 0 }, vec![at(5.0, Command::SetIcTrain { index: 0, open: true })],
         Box::new(|_, s| ok_if(!s.ic[0].in_service() && s.ic[0].heat < 1.0, format!("IC 0 in service, heat {}", s.ic[0].heat)))),
        (Fault::IcReturnValveStuck { train: 0, valve: 0 }, vec![at(5.0, Command::SetIcTrain { index: 0, open: true })],
         Box::new(|_, s| ok_if(s.ic[0].return_valve_pos[0] < 1e-6 && s.ic[0].return_valve_pos[1] > 0.99, format!("return valves {:?}", s.ic[0].return_valve_pos)))),
        (Fault::IcSteamValveClosed { train: 0 }, vec![at(5.0, Command::SetIcTrain { index: 0, open: true })],
         Box::new(|_, s| ok_if(s.ic[0].steam_valve_pos < 1e-6 && s.ic[0].heat < 1.0e5, format!("steam valve {} heat {}", s.ic[0].steam_valve_pos, s.ic[0].heat)))),
        (Fault::RodStuck { index: strongest }, vec![at(5.0, Command::ManualScram)],
         Box::new(move |s0, s| ok_if((s.core.rods.pos[strongest] - s0.core.rods.pos[strongest]).abs() < 1e-9, "stuck rod moved".into()))),
        (Fault::IcNoncondensableDegradation { train: 0, pct: 50 }, vec![at(5.0, Command::SetIcTrain { index: 0, open: true }), at(5.0, Command::SetIcTrain { index: 1, open: true })],
         Box::new(|_, s| ok_if((s.ic[0].ua_factor - 0.5).abs() < 1e-9 && s.ic[0].heat < 0.6 * s.ic[1].heat, format!("ua_factor {} heat0 {} heat1 {}", s.ic[0].ua_factor, s.ic[0].heat, s.ic[1].heat)))),
    ]
}

#[test]
fn every_fault_persists_and_is_honoured() {
    let table = fault_table();
    assert_eq!(table.len(), 16, "one row per Fault variant");
    let mut failures = Vec::new();
    for (f, cmds, check) in table {
        assert!(f.is_valid(), "{f:?} must be a valid payload");
        let c = Case { id: "fault_probe", scenario: ScenarioName::FullPower, duration_s: 300.0, sample_dt_s: 1.0, commands: cmds,
                       faults: vec![ScriptedFault { at_s: 0.0, fault: f.clone(), active: true }], dt_after_s: None, operator: Operator::None };
        let s0 = Sim::new(ScenarioName::FullPower);
        let (_, s) = run_sim_with(Sim::new(c.scenario), &c, DT, |_: &Sim, _: f64| vec![]);
        if !s.has_fault(&f) { failures.push(format!("{f:?}: fault cleared by automatic logic")); }
        if let Err(m) = check(&s0, &s) { failures.push(format!("{f:?}: {m}")); }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

#[test]
fn set_fault_replaces_same_kind_and_equipment_and_rejects_invalid_payloads() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    sim.set_fault(Fault::PressureRegulatorFailed { demand_pct: 50 }, true);
    sim.set_fault(Fault::PressureRegulatorFailed { demand_pct: 70 }, true);
    assert_eq!(sim.faults.iter().filter(|f| f.kind() == "PressureRegulatorFailed").count(), 1);
    assert_eq!(sim.find_fault("PressureRegulatorFailed", None), Some(&Fault::PressureRegulatorFailed { demand_pct: 70 }));
    assert!(!sim.has_fault(&Fault::PressureRegulatorFailed { demand_pct: 50 }));

    sim.set_fault(Fault::IcNoncondensableDegradation { train: 0, pct: 50 }, true);
    sim.set_fault(Fault::IcNoncondensableDegradation { train: 0, pct: 80 }, true);
    sim.set_fault(Fault::IcNoncondensableDegradation { train: 1, pct: 50 }, true);
    assert_eq!(sim.faults.iter().filter(|f| f.kind() == "IcNoncondensableDegradation").count(), 2, "different equipment coexists; same equipment replaces");
    assert_eq!(sim.find_fault("IcNoncondensableDegradation", Some(0)), Some(&Fault::IcNoncondensableDegradation { train: 0, pct: 80 }));

    let n = sim.faults.len();
    sim.set_fault(Fault::PressureRegulatorFailed { demand_pct: 150 }, true);
    assert_eq!(sim.faults.len(), n, "invalid payload must be ignored");
    assert_eq!(sim.find_fault("PressureRegulatorFailed", None), Some(&Fault::PressureRegulatorFailed { demand_pct: 70 }), "invalid payload must not replace a valid one");

    sim.set_fault(Fault::PressureRegulatorFailed { demand_pct: 0 }, false);
    assert!(sim.find_fault("PressureRegulatorFailed", None).is_none(), "deactivation removes by (kind, equipment) regardless of payload");
    let before = sim.faults.clone();
    sim.settle(60.0);
    assert_eq!(sim.faults, before, "stepping never changes the fault list");
}
```

- [ ] **Step 2: Run**

Run: `cargo test --release -p bwr-core --test scenarios -- every_fault set_fault`
Expected: PASS. A failure is a defect in the owning package (P2 for `set_fault`, P5–P7 for honouring): fix it there with its own failing unit test, then re-run. These are logical assertions and are never ignored.

- [ ] **Step 3: Commit**

```bash
git add crates/bwr-core/tests/scenarios.rs
git commit -m "test(scenarios): fault overrides persist, are honoured, and replace by kind and equipment

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 8: Analytical, sensitivity and convergence evidence

Evidence class: analytical component solutions, parameter sensitivity and convergence. Verification evidence, not validation.

**Files:**
- Test: `crates/bwr-core/tests/scenarios.rs`
- Modify: `docs/validation/method.md`

**Interfaces:**
- Consumes: `Kinetics::{equilibrium, advance -> Result}`, `Vessel::new`, `Vessel::step -> BoundaryFlows`, `VesselInputs` (P7 form), `Ledger`, `IcTrain::{new, step(dt, p_vessel, w_steam_available)}`, `IC_TRAIN_RATED_W`, `ReactivityCoeffs`, `Sim::full_power_with`, `Tolerance::numerical`, `compare`.

- [ ] **Step 1: Write the tests**

```rust
use bwr_core::ic::IcTrain;
use bwr_core::kinetics::Kinetics;
use bwr_core::ledger::Ledger;
use bwr_core::reactor::ReactivityCoeffs;
use bwr_core::steam::sat_p;
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
    let inp = VesselInputs { q_core: 0.0, w_feed: 0.0, h_feed: 0.0, w_steam: 100.0, w_liquid_out: 0.0, w_ic_steam: 0.0, w_ic_return: 0.0, h_ic_return: 0.0 };
    let mut removed = 0.0;
    for _ in 0..200 {
        let f = v.step(DT, &inp);
        ledger.record(DT, &f);
        removed += (f.w_steam * f.h_steam + f.q_loss) * DT;
    }
    assert!(v.last_error.is_none());
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
    for _ in 0..(600.0 / DT) as usize {
        ic.step(DT, P_RATED, 1000.0);
    }
    let err = (ic.heat - IC_TRAIN_RATED_W).abs() / IC_TRAIN_RATED_W;
    assert!(err < 0.03, "IC duty {:.2} MW vs registered rated duty {:.2} MW", ic.heat / 1e6, IC_TRAIN_RATED_W / 1e6);
}

#[test]
fn turbine_trip_peak_power_sensitivity_to_void_coefficient() {
    let c = case("fp_turbine_trip");
    let peak = |sim: Sim| run_sim_with(sim, &c, DT, |_: &Sim, _: f64| vec![]).0.metrics().peak["power_frac"].0;
    let scaled = |f: f64| { let mut k = ReactivityCoeffs::default(); k.void *= f; Sim::full_power_with(k) };
    let (lo, base, hi) = (peak(scaled(0.75)), peak(Sim::new(c.scenario)), peak(scaled(1.25)));
    println!("fp_turbine_trip peak power: void coef x0.75 -> {lo:.4}, x1 -> {base:.4}, x1.25 -> {hi:.4}");
    // A less negative void coefficient inserts less reactivity on void collapse: a lower peak.
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

- [ ] **Step 3: Document and commit**

Append to `docs/validation/method.md`:

```
## Verification evidence (not BWRX transient validation)

- Inhour: `kinetics_matches_the_inhour_equation` (analytical, 2 % on asymptotic period).
- Closed-vessel blowdown: `closed_vessel_blowdown_energy_is_conserved` (ledger residual < 1e-9 relative).
- IC duty: `single_ic_train_duty_at_rated_pressure_with_boiling_pool` (3 % of the registered duty IC_TRAIN_RATED_W, status esbwr-estimate).
- Qualitative ordering: `transient_expectations` (actuation order per case through event keys).
- Transfer allocation: `hotwell_and_cst_gain_exactly_what_the_vessel_loses_through_exhaustion`.
- Sensitivity: `turbine_trip_peak_power_sensitivity_to_void_coefficient` (peak band printed, monotonicity asserted).
- Convergence: `all_short_cases_are_converged_at_half_step` (numerical tolerance class).

None of the above is quantitative BWRX transient validation. A case is called validated only against a transcribed public reference (role `validation`), and an ESBWR reference is reported as "ESBWR reference configuration mechanism check, not BWRX acceptance".
```

```bash
git add crates/bwr-core/tests/scenarios.rs docs/validation/method.md
git commit -m "test(scenarios): analytical, sensitivity and convergence verification evidence

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 9: Public reference traces (search, transcribe, compare)

**Files:**
- Modify: `crates/bwr-core/src/validation.rs` (`parse_reference_csv`, `compare_reference`, unit test)
- Modify: `crates/bwr-core/examples/validation.rs`
- Create: `docs/validation/reference/<case>.csv` (only for traces actually found)
- Modify: `docs/model/sources.csv`, `docs/validation/cases.csv` (`reference` cells)

**Interfaces:**
- Consumes: `Tolerance::source_comparison(cases_csv_row: &str)`, `Tolerance::{rel, abs}`, `Trace`, `col_index`, `TRACE_COLUMNS`, `N_TRACE_COLS`.
- Produces: `parse_reference_csv`, `compare_reference`.

- [ ] **Step 1: Search**

Open, in this order, looking for time-history figures of reactor power, dome pressure, steam flow, and water level:
1. ESBWR DCD Rev. 10 Tier 2 Chapter 15, sections 15.2.1 (generator load rejection with/without bypass), 15.2.2 (turbine trip with/without bypass), 15.2.3 (closure of all MSIVs), 15.2.4 (loss of feedwater heating): link in the spec's "Public source starting points".
2. NRC stability safety evaluation (ML26078A299) and NEDO-34270-A for any decay-ratio or power-oscillation trace.
3. NEDO-33910-A Rev. 2 for any pressure-time curve.

For each readable figure: add it to `docs/model/sources.csv` (document, section, figure number, page, retrieval date, PDF checksum), read at least 8 points per curve by eye, and write `docs/validation/reference/<case>.csv`:

```
t_s,column,value,source
0,power_frac,1.0,ESBWR DCD R10 T2 Fig 15.2-7 reactor power; read by eye +/-2 % of full scale
```

`column` must be a `TRACE_COLUMNS` name; pressure in Pa, power as a fraction of rated. For each case with a transcribed file set, in `cases.csv`, `tolerance_class` to `source-comparison`, `role` to `validation`, and `reference` to the figure id followed by ` (ESBWR reference configuration mechanism check, not BWRX acceptance)` whenever the source is ESBWR; the registration test (Task 5) enforces that `validation` appears only with such a file. If no figure can be read to ±5 % of full scale, write nothing and leave `role` at `plausibility` and `reference` at `none available (...)`.

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
            let mut row = [0.0; N_TRACE_COLS];
            row[0] = k as f64;
            row[col_index("power_frac")] = 1.0 - 0.05 * k as f64;
            trace.rows.push(row);
        }
        assert!(compare_reference(&trace, &pts, &Tolerance::physical()).is_empty());
        let bad = vec![(5.0, "power_frac".to_string(), 0.5)];
        assert_eq!(compare_reference(&trace, &bad, &Tolerance::physical()).len(), 1);
        let outside = vec![(99.0, "power_frac".to_string(), 0.5)];
        assert_eq!(compare_reference(&trace, &outside, &Tolerance::physical()).len(), 1, "point outside the trace is a violation");
        trace.rows[10][col_index("power_frac")] = f64::NAN;
        assert!(!compare_reference(&trace, &pts, &Tolerance::physical()).is_empty(), "nonfinite candidate is a violation");
        assert!(parse_reference_csv("t_s,column,value,source\n0,no_such_column,1,x\n").is_err());
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
        if !t.is_finite() || !v.is_finite() { return Err(format!("line {k}: non-finite value")); }
        out.push((t, col, v));
    }
    Ok(out)
}

fn interpolate(trace: &Trace, col: &str, t: f64) -> Option<f64> {
    let ci = col_index(col);
    let rows = &trace.rows;
    let k = rows.iter().position(|r| r[0] >= t)?;
    if k == 0 { return Some(rows[0][ci]); }
    let (a, b) = (rows[k - 1], rows[k]);
    let f = (t - a[0]) / (b[0] - a[0]).max(1e-12);
    Some(a[ci] + f * (b[ci] - a[ci]))
}

/// Same rules as `compare`: a point outside the trace or a nonfinite candidate value is a violation;
/// the allowed difference is max(rel * |reference|, abs floor for the column).
pub fn compare_reference(trace: &Trace, points: &[(f64, String, f64)], tol: &Tolerance) -> Vec<String> {
    let mut v = Vec::new();
    for (t, col, reference) in points {
        match interpolate(trace, col, *t) {
            None => v.push(format!("{col} at {t} s: outside trace")),
            Some(sim) if !sim.is_finite() => v.push(format!("{col} at {t} s: nonfinite candidate")),
            Some(sim) => {
                let floor = tol.abs.get(col).copied().unwrap_or(0.0);
                let allowed = (tol.rel * reference.abs()).max(floor);
                if (sim - reference).abs() > allowed {
                    v.push(format!("{col} at {t} s: sim {sim:.4} vs reference {reference:.4} (allowed +/-{allowed:.4})"));
                }
            }
        }
    }
    v
}
```

In `examples/validation.rs`, after the baseline comparison inside the per-case loop:

```rust
let ref_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../../docs/validation/reference/{}.csv", case.id));
if let Ok(s) = std::fs::read_to_string(&ref_path) {
    let cases_csv = std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/validation/cases.csv")).expect("cases.csv");
    let row = cases_csv.lines().find(|l| l.starts_with(&format!("{},", case.id))).expect("case row in cases.csv");
    let pts = parse_reference_csv(&s).expect("reference csv");
    let v = compare_reference(&trace, &pts, &Tolerance::source_comparison(row));
    println!("  reference: ESBWR reference configuration mechanism check, not BWRX acceptance: {} points, {} outside tolerance", pts.len(), v.len());
    for line in &v { println!("     {line}"); }
}
```

Reference mismatches are printed as findings; they never fail the run (spec: "its numerical trace is not automatically a BWRX acceptance curve").

- [ ] **Step 4: Run and commit**

Run: `cargo test --release -p bwr-core` → PASS; `cargo run --release -p bwr-core --example validation` → prints a `reference:` line for every case that has a file under `docs/validation/reference/`.

```bash
git add crates/bwr-core/src/validation.rs crates/bwr-core/examples/validation.rs docs/validation docs/model/sources.csv
git commit -m "feat(validation): public reference point comparison (ESBWR mechanism check)

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 10: Paired-failure exploration

**Files:**
- Create: `crates/bwr-core/examples/paired_failures.rs`
- Create (generated): `docs/validation/paired-failures.md`, `docs/validation/results/paired_failures.csv`

**Interfaces:**
- Consumes: `run_sim_with`, `operators`, `Fault`, `Core.clad_t`, `Sim.domain_errors`, `DomainError::reason`, `Metrics`.

- [ ] **Step 1: Write the example**

```rust
//! Explore the most consequential supported paired failures and report where the model
//! leaves its envelope. Writes docs/validation/paired-failures.md and
//! docs/validation/results/paired_failures.csv. Nothing here is asserted.
use std::path::PathBuf;

use bwr_core::api::{Command, ScenarioName};
use bwr_core::bop::LevelControlMode;
use bwr_core::faults::Fault;
use bwr_core::params::N_IC_TRAINS;
use bwr_core::sim::DT;
use bwr_core::validation::{operators, run_sim_with, Case, Operator, ScriptedCommand, ScriptedFault};
use bwr_core::Sim;

fn at(at_s: f64, cmd: Command) -> ScriptedCommand { ScriptedCommand { at_s, cmd } }
fn f(at_s: f64, fault: Fault) -> ScriptedFault { ScriptedFault { at_s, fault, active: true } }
fn close_both() -> Vec<ScriptedCommand> { vec![at(5.0, Command::SetMsiv { line: 0, open: false }), at(5.0, Command::SetMsiv { line: 1, open: false })] }

fn main() {
    let strongest = Sim::new(ScenarioName::FullPower).core.rods.strongest_rod();
    let pairs: Vec<(&str, Vec<ScriptedCommand>, Vec<ScriptedFault>, Operator)> = vec![
        ("turbine trip + bypass unavailable", vec![at(5.0, Command::TripTurbine)], vec![f(0.0, Fault::BypassUnavailable)], Operator::None),
        ("MSIV closure + one IC train", close_both(), vec![f(0.0, Fault::IcTrainUnavailable { train: 1 }), f(0.0, Fault::IcTrainUnavailable { train: 2 })], Operator::None),
        ("loss of feedwater + stuck strongest rod", vec![], (0..3).map(|i| f(5.0, Fault::FeedPumpTrip { index: i })).chain([f(0.0, Fault::RodStuck { index: strongest })]).collect(), Operator::None),
        ("CW loss + vacuum pump failure", vec![], vec![f(5.0, Fault::CwPumpTrip { index: 0 }), f(5.0, Fault::CwPumpTrip { index: 1 }), f(5.0, Fault::VacuumPumpFailed)], Operator::None),
        ("load rejection + regulator fail open", vec![at(5.0, Command::OpenBreaker)], vec![f(5.0, Fault::PressureRegulatorFailed { demand_pct: 100 })], Operator::None),
        ("heater loss + rod withdrawal error", vec![], vec![f(5.0, Fault::HeatersLost)], Operator::RodWithdrawalError),
        ("low level + IC return valves stuck", vec![at(5.0, Command::SetLevelControl { mode: LevelControlMode::Manual }), at(6.0, Command::SetFcvManual { pos: 0.0 })],
         vec![f(0.0, Fault::IcReturnValveStuck { train: 0, valve: 0 }), f(0.0, Fault::IcReturnValveStuck { train: 0, valve: 1 })], Operator::None),
        ("MSIV closure + 50 % IC noncondensable degradation", close_both(), (0..N_IC_TRAINS).map(|t| f(0.0, Fault::IcNoncondensableDegradation { train: t, pct: 50 })).collect(), Operator::None),
    ];

    let mut md = String::from("# Paired-failure exploration\n\nGenerated by `cargo run --release -p bwr-core --example paired_failures`. Each pair runs 1800 s from full power. Rows report where the model leaves its declared envelope; nothing here is an acceptance claim.\n\n| Pair | First domain error | Peak pressure (MPa) | Min NR level (cm) | Peak clad (degC) |\n|---|---|---|---|---|\n");
    let mut csv = String::from("pair,first_domain_error,first_domain_error_t_s,peak_pressure_mpa,min_level_nr_cm,peak_clad_c\n");
    for (name, commands, faults, operator) in pairs {
        let case = Case { id: "pair", scenario: ScenarioName::FullPower, duration_s: 1800.0, sample_dt_s: 1.0, commands, faults, dt_after_s: None, operator };
        let mut clad_peak = 0.0f64;
        let mut inner: Box<dyn FnMut(&Sim, f64) -> Vec<Command>> = match operator {
            Operator::RodWithdrawalError => Box::new(operators::rod_withdrawal_error()),
            _ => Box::new(|_: &Sim, _: f64| Vec::new()),
        };
        let (trace, sim) = run_sim_with(Sim::new(case.scenario), &case, DT, |s: &Sim, t: f64| { clad_peak = clad_peak.max(s.core.clad_t); inner(s, t) });
        let m = trace.metrics();
        let (first, t_first) = sim.domain_errors.first().map(|(t, d)| (d.reason(), *t)).unwrap_or(("none".into(), f64::NAN));
        md.push_str(&format!("| {name} | {first}{} | {:.3} | {:.0} | {:.0} |\n", if t_first.is_finite() { format!(" at {t_first:.0} s") } else { String::new() }, m.peak["pressure_pa"].0 / 1e6, m.min["level_nr_cm"].0, clad_peak));
        csv.push_str(&format!("\"{name}\",\"{first}\",{t_first},{:.6},{:.3},{:.1}\n", m.peak["pressure_pa"].0 / 1e6, m.min["level_nr_cm"].0, clad_peak));
        println!("{name}: {first}");
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/validation");
    std::fs::create_dir_all(root.join("results")).expect("results dir");
    std::fs::write(root.join("paired-failures.md"), md).expect("write paired-failures.md");
    std::fs::write(root.join("results/paired_failures.csv"), csv).expect("write paired_failures.csv");
    println!("wrote {}", root.display());
}
```

- [ ] **Step 2: Run and commit**

Run: `cargo run --release -p bwr-core --example paired_failures`
Expected: eight lines and `wrote .../docs/validation`; the markdown table has eight rows. Any domain error row is expected output, not a failure.

```bash
git add crates/bwr-core/examples/paired_failures.rs docs/validation/paired-failures.md docs/validation/results/paired_failures.csv
git commit -m "feat(validation): paired-failure exploration report

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 11: Coverage: every command has a scenario or an explicit limitation

**Files:**
- Modify: `crates/bwr-core/src/validation.rs` (`standard_cases()`)
- Modify: `docs/validation/cases.csv`, `docs/model/coverage.md`
- Test: `crates/bwr-core/tests/scenarios.rs`

**Interfaces:**
- Consumes: `Command` (serde tag `type`), `standard_cases()`, `operator_of`.

- [ ] **Step 1: Write the failing test**

```rust
/// Commands with no plant effect or rejected by the scripted runner (master: LoadScenario, SetPaused, SetSpeed panic in run_case).
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
        if c.operator != Operator::None {
            // Collect what the operator emits by running it in the loop (cheap: 1 s operator period).
            let mut op = operator_of(c.operator);
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

- [ ] **Step 2: Run**

Run: `cargo test --release -p bwr-core --test scenarios -- every_command`
Expected: FAIL listing `resetScram`, `rodStop`, `setHeadVent`, `setBypass`, `setLoadLimit`.

- [ ] **Step 3: Add the missing cases**

In `standard_cases()`:

```rust
// Precondition: the post-scram setdown (-100 cm) equals L3, so the L3 scram signal would block a reset;
// raising the level setpoint to its +50 cm cap parks level at -50 cm before the reset is attempted.
case("fp_scram_reset_after_manual", fp, 300.0, vec![at(5.0, ManualScram), at(10.0, SetLevelSetpoint { cm: 50.0 }), at(120.0, ResetScram)]),
// Precondition: the strongest rod sits near position 34 at full power, so 3 s of motor withdrawal (1.2 positions)
// then STOP lands on an even notch well short of full out; n cannot reach the 1.08 block in 3 s.
case("fp_rod_stop_mid_motion", fp, 120.0, vec![at(5.0, RodSetTarget { indices: vec![strongest], target: ROD_FULL_OUT }), at(8.0, RodStop)]),
// Precondition: at 40 degC the head holds about 1 atm of air (Package 4 gas model); closing the vent moves the regime
// to GasBlanketed and the 30-day decay heat (about 1 MW net of insulation loss) raises temperature by a few K in 1 h.
case("cs_head_vent_close_and_heatup", ScenarioName::ColdShutdown, 3600.0, vec![at(5.0, SetHeadVent { open: false })]),
case("fp_bypass_manual_10pct", fp, 300.0, vec![at(5.0, SetBypass { auto: false, pos: 0.10 })]),
case("fp_load_limit_80pct", fp, 600.0, vec![at(5.0, SetLoadLimit { pct: 80.0 })]),
```

`cases.csv` rows (and add the five ids to `ALL_CASE_IDS`):

```
fp_scram_reset_after_manual,fullPower,300,Manual scram; level setpoint raised above L3; reset at 120 s accepted; rods stay in,none available,physical,plausibility,baseline
fp_rod_stop_mid_motion,fullPower,120,Rod stop halts a withdrawal at the nearest notch,none available,physical,plausibility,baseline
cs_head_vent_close_and_heatup,coldShutdown,3600,Head vent closed at cold shutdown: regime leaves Vented; pressure follows gas and decay heat,none available,plausibility,plausibility,baseline
fp_bypass_manual_10pct,fullPower,300,Bypass to manual 10 %: pressure regulator compensates on the TCV,none available,physical,plausibility,baseline
fp_load_limit_80pct,fullPower,600,Load limit 80 %: TCV limited; bypass carries the remainder,none available,physical,plausibility,baseline
```

Add to `EXPECTATIONS`:

```rust
Expect { id: "fp_scram_reset_after_manual", order: &[ev::SCRAM], no_scram: false, pressure_below_design: true, supported: true, expect_domain: &[] },
Expect { id: "fp_rod_stop_mid_motion", order: &[], no_scram: true, pressure_below_design: true, supported: true, expect_domain: &[] },
Expect { id: "fp_bypass_manual_10pct", order: &[], no_scram: true, pressure_below_design: true, supported: true, expect_domain: &[] },
Expect { id: "fp_load_limit_80pct", order: &[], no_scram: true, pressure_below_design: true, supported: true, expect_domain: &[] },
```

and three direct tests:

```rust
#[test]
fn scram_reset_is_accepted_once_no_signal_persists() {
    let (trace, sim) = run_full("fp_scram_reset_after_manual");
    assert!(trace.metrics().first_event_time.contains_key(ev::SCRAM));
    assert!(!sim.scrammed, "reset refused: {:?}", sim.scram_reason);
    assert!(sim.core.rods.pos.iter().all(|&p| p <= 0.01), "rods moved after reset");
}

#[test]
fn rod_stop_leaves_the_rod_on_an_even_notch_short_of_full_out() {
    let (_, sim) = run_full("fp_rod_stop_mid_motion");
    let i = sim.core.rods.strongest_rod();
    let p = sim.core.rods.pos[i];
    assert!((p / 2.0 - (p / 2.0).round()).abs() < 1e-6, "not on a notch: {p}");
    assert!(p < ROD_FULL_OUT, "rod reached full out despite stop");
}

#[test]
fn closing_the_head_vent_at_cold_shutdown_changes_regime_without_a_domain_error() {
    let (_, sim) = run_full("cs_head_vent_close_and_heatup");
    assert_ne!(sim.vessel.eos.regime, Regime::Vented);
    assert!(sim.vessel.eos.p > P_ATM);
    assert!(sim.domain_errors.is_empty(), "{:?}", sim.domain_errors);
}
```

Run `cargo run --release -p bwr-core --example validation -- --write-missing`.

- [ ] **Step 4: Update `docs/model/coverage.md`**

Add a `Scenario(s)` column to the subsystem table, filling each subsystem row with the case ids above, and these limitation rows:

```
| Command/area | Scenario(s) or limitation |
| setPaused, setSpeed, loadScenario, selectRod, ackAlarms | UI only; no plant effect; rejected by the scripted runner; exercised by UI typecheck and the P10 contract test |
| Cold shutdown below the IC pool temperature | Limitation: requires an unimplemented shutdown cooling system (docs/model/isolation-condensers.md); fp_cooldown_on_ic stops at that boundary |
| ATWS, LOCA, severe accident | Out of scope; any run reaching a DomainError is reported as unsupported (fp_feed_loss_no_ic, paired-failures.md) |
```

- [ ] **Step 5: Run everything and commit**

Run: `cargo test --release -p bwr-core` → PASS; `cargo run --release -p bwr-core --example validation` → all PASS; `npm --prefix ui run typecheck` → unchanged (this package adds no API field).

```bash
git add crates/bwr-core/src/validation.rs crates/bwr-core/tests/scenarios.rs docs/validation docs/model/coverage.md
git commit -m "test(scenarios): every command has a scenario or an explicit limitation

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

## Acceptance gate (verbatim from the spec)

**Acceptance gate:** Every current subsystem and control has an assigned scenario or explicit limitation. No unsupported ATWS, LOCA, or severe-accident conclusion is inferred from partial models.

Evidence: `every_command_is_exercised_by_a_case_or_listed_as_ui_only` and `docs/model/coverage.md` (first sentence); `transient_expectations` with `supported`/`expect_domain`, `fp_feed_loss_no_ic`, `hotwell_and_cst_gain_exactly_what_the_vessel_loses_through_exhaustion`, and `docs/validation/paired-failures.md` (second sentence).

## Self-review against the spec's Package 9 bullets and the master checklist

| Spec bullet | Task |
|---|---|
| Correct initialization of cold shutdown, hot standby, full power | Task 2 |
| Normal startup, power changes, controlled shutdown, manual scram, mode-switch shutdown, supported cooldown endpoints | Tasks 3, 4 (manual scram is P2's `fp_manual_scram`, asserted in Task 6) |
| Turbine trip and load rejection with/without bypass; single and multiple steam isolation; pressure-controller faults; excessive steam demand | Tasks 5, 6 |
| Feed-pump/condensate-pump loss, feed-valve faults, loss of feedwater heating, high/low level, cooling-water loss, vacuum degradation, hotwell limits | Tasks 5, 6; hotwell limits: `fp_feed_loss_no_ic` exercises hotwell/CST accounting through exhaustion (Task 6 Step 3); a CST-full case is not registered because no `Fault` variant can preset `cst_mass` and the runner takes no setup hook, so the CST-full boundary is covered by Package 6's unit tests and recorded as a limitation in `coverage.md` |
| Rod withdrawal errors, rod-insertion failures, IC availability combinations | Tasks 5, 6 |
| Deterministic test-level fault overrides | Task 7 |
| Public transient traces; calibrate/validate split | Tasks 5 (roles: `calibration` for the three steady cases, `plausibility` for everything else at planning time), 9 (a case becomes `validation` only when a public trace is transcribed; ESBWR references carry the mechanism-check label) |
| Conservation, analytical, ordering, sensitivity, convergence evidence, labelled | Tasks 6 and 8 (each headed with its evidence class; `method.md` repeats the label) |
| Paired failures; report envelope exits | Task 10 |

Master checklist: no placeholders; every name matches r2 (`Operator` variants, generic `run_sim_with`/`run_case_with`, `Case.dt_after_s`, `N_TRACE_COLS`/`col_index`, `initial_drift_per_s`, `first_event_time` keys, `Tolerance::source_comparison`, `faults: Vec<Fault>` with `kind/equipment/is_valid/find_fault`, `ReactivityCoeffs { void, .. }`, `Sim::full_power_with`, `IcTrain::request(dt, p_vessel, w_steam_available)` / `IcTrain::commit(dt, p_vessel, applied)`, `VesselInputs` P7 form, `CST_CAPACITY_KG`, locked `cases.csv` header); this package adds no struct or enum to `Sim` and no `Snapshot` field (Derives and typecheck rules trivially hold); every new constant (`POWER_CHANGE_HOLD_S`, a test-script tuning value with no plant meaning) is documented in the operator's doc comment and needs no `parameters.csv` row; every consumer of a transfer is checked through the allocation test in Task 6.
