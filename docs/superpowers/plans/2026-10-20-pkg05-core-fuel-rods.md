# Package 5: Core Physics, Fuel Response, and Rod Mechanisms — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Names and signatures come from `docs/superpowers/plans/2026-10-20-refinement-master.md` (the master); if this file and the master disagree, the master wins.

**Goal:** Make the core model traceable and testable: distinct rod insertion mechanisms, a two-node fuel/cladding thermal model with an explicit envelope instead of a "core damage" verdict, decay heat fitted to a named public reference with finite irradiation history, verified xenon behaviour, reproducible shutdown-margin evidence, and reactivity coefficients exposed for sensitivity studies.

**Architecture:** Point kinetics is retained (spec). `Rods` gains a per-rod `RodMotion` state machine and a `stuck` mask so hydraulic scram, motor run-in, Select Rod Insert, and normal motor motion are separate mechanisms with separate speeds. `Core` gains a cladding node and a `ReactivityCoeffs` value so feedback coefficients are data, not constants baked into `feedback()`. The 1200 degC average-fuel "core damage" rule is replaced by `DomainError::FuelModelEnvelope`, raised when the model leaves the regime it can defend. Decay heat is fitted by `tools/fit_decay_heat.py` to a transcribed reference table in `docs/model/decay-heat-reference.csv`.

**Tech Stack:** Rust 2021 (`crates/bwr-core`), serde, TypeScript/React UI contract mirror, Python 3 with numpy/scipy for the decay-heat fit.

**Prerequisites:** Packages 1–4 complete against master r2. In particular this plan relies on: `docs/model/parameters.csv` with the locked header `id,value,unit,definition,reference_state,source_design,status,source_ids,interval_lo,interval_hi,assumption_id,location,notes` (P1); `docs/model/assumptions.md` and `docs/model/esbwr-applicability.csv` (P1); `Sim::log`, `Fault`, `Sim::faults: Vec<Fault>`, `Sim::set_fault`, `Sim::has_fault`, `validation::run_case` with trace schema v2 (`clad_t_c`, `rods_min_pos`, `rods_max_pos` emitted as 0.0 until this package) (P2); `DomainError`, `Sim::domain_errors`, `Sim::domain_error(e)`, `Sim::finalize_init`, `Core::step_rods`, `Core::step(...) -> Result<f64, DomainError>`, `Kinetics::advance(rho, s, dt) -> Result<usize, DomainError>` (P3); `Vessel::moderator_t()` (P4). Baseline traces under `docs/validation/baseline/` exist (P2). `Sim::alternate_rod_insertion()` and `ARI_ENABLED` belong to P8; this package only supplies the two rod methods it will call.

## Global Constraints

Inherited verbatim from the master's Global Constraints section. Package-specific additions:

- "Preserve point kinetics initially. Add axial or channel detail only if supported benchmarks show a specific inadequacy that it can resolve." No axial or channel model is added in this package.
- "Do not call a fit to a simple power law an exact standards implementation."
- "Replace the average-fuel-temperature 1200 C damage rule with justified indications; do not present peak-cladding or CPR results without the required model and data."
- "Treat plant-specific margin targets as sourced requirements, not invented acceptance values."
- No `Command` variants are added. Faults stay engine-level.
- Every new constant gets a `parameters.csv` row with status `confirmed`, `derived`, `calibrated`, `esbwr-estimate`, `assumed`, or `unavailable`.

**Acceptance gate (spec, verbatim):** Supported reactivity and shutdown behavior has reproducible evidence. Uncertain rod worth, feedback, and fuel limits cannot silently appear as confirmed plant performance.

---

## File structure for this package

| File | Action | Responsibility |
|---|---|---|
| `docs/model/core.md` | Create | Core model specification: geometry, conventions, mechanisms, envelope, uncertainty |
| `docs/model/decay-heat-reference.csv` | Create | Transcribed public decay-heat table (`t_s,p_over_p0,source`) |
| `docs/model/parameters.csv` | Modify | Rows for every constant touched here |
| `docs/model/assumptions.md` | Modify | A-050 .. A-058 |
| `docs/model/esbwr-applicability.csv` | Modify | Rows for DCD Ch. 4.3, 4.6, 15 borrowings |
| `tools/fit_decay_heat.py` | Rewrite | Fit `DECAY_GROUPS` to the reference CSV; fail if error > 5 % |
| `crates/bwr-core/src/params.rs` | Modify | New constants; remove `FUEL_RESISTANCE` |
| `crates/bwr-core/src/reactor.rs` | Modify | `RodMotion`, `Rods` restructure, `ReactivityCoeffs`, `clad_t`, two-node update, `after_history`, SDM |
| `crates/bwr-core/src/sim.rs` | Modify | Call-site migration, stuck-fault mirroring, envelope detection, `full_power_with`, alarms |
| `crates/bwr-core/src/api.rs` | Modify | `clad_temp_c`, `fuel_envelope_exceeded`; remove `core_damage` |
| `crates/bwr-core/tests/core_transients.rs` | Create | Package tests |
| `crates/bwr-core/examples/sensitivity.rs` | Create | Coefficient sensitivity study |
| `ui/src/sim/types.ts`, `ui/src/sim/mock.ts` | Modify | Contract mirror |
| `README.md` | Modify | Decay-heat and core-damage wording |

Names from the master (r2) that this package owns: `RodMotion { Idle, Motor { target }, Scram, Sri, MotorRunIn }`, `Rods { motion, stuck }` and its method set, `ReactivityCoeffs { void, doppler, mod_t, xenon_worth }`, `Core { clad_t, coeffs }`, `DecayHeat::after_history`, `Sim::full_power_with`, `GAP_RESISTANCE_K_PER_W`, `CLAD_COOLANT_RESISTANCE_K_PER_W`, `CLAD_HEAT_CAP_J_PER_K`, `FUEL_MODEL_T_MAX_C`, `UNCOVERED_ENVELOPE_S`, `IRRADIATION_HISTORY_S`. Names introduced beyond the master: `Sim::uncovered_s`, `Sim::fuel_envelope_exceeded()`, `Sim::vessel_force_uncovered_for_test()` (test hook), `examples/sensitivity.rs`.

Derives (master Derives rule): `RodMotion` derives `Clone, Copy, Debug, PartialEq`; `ReactivityCoeffs` derives `Clone, Copy, Debug` (plus a manual `Default`); `Rods` and `Core` keep `Clone, Debug`.

---

### Task 1: Core model specification and parameter verification (documentation)

**Files:**
- Create: `docs/model/core.md`
- Modify: `docs/model/parameters.csv`, `docs/model/assumptions.md`, `docs/model/esbwr-applicability.csv`

**Interfaces:**
- Consumes: P1 register files and their header formats.
- Produces: the written envelope and conventions that every later task's tests cite; the `SDM_REQUIRED` row that Task 9's test reads.

- [ ] **Step 1: Write `docs/model/core.md`**

```markdown
# Core model

Status: Package 5 specification. Point kinetics with six delayed-neutron groups
is retained (spec: no axial or channel model until a supported benchmark shows
a specific inadequacy).

## 1. Geometry and counts

| Item | Value | Status | Source / note |
|---|---|---|---|
| Thermal power | 870 MWt | confirmed | GE 005N9751 (sources.csv) |
| Fuel bundles | 240 GNF2 | confirmed | GE 005N9751 |
| Control rods | 57, bottom entry, fine-motion drives (FMCRD) | confirmed | GE 005N9751 |
| Active fuel length | 3.81 m | assumed | typical GNF2; A-050 |
| Rod position convention | 0 = full in, 48 = full out, 2 positions per notch | assumed | legacy BWR convention; A-051 |
| Rod radial layout | 57 cells within radius sqrt(17) on a 9x9 grid | assumed | A-052; not the plant map |
| Rod relative worth | J0(2.405 r/R)^2 radial weighting, cosine-integral axial S-curve | assumed | A-052 |

## 2. Rod motion mechanisms

Rod motion is a per-rod `RodMotion` state (reactor.rs):

| Mechanism | State | Speed | Initiated by |
|---|---|---|---|
| Normal motor drive | `Motor { target }` | `ROD_SPEED` = 0.4 positions/s (assumed, A-053) | operator `RodSetTarget` |
| Hydraulic scram | `Scram` | `ROD_FULL_OUT / SCRAM_TIME` = 48 / 3 s (assumed, A-053) | RPS scram (`Sim::scram`) |
| Select Rod Insert | `Sri` | same hydraulic speed, SRI group only | turbine trip / load rejection at power |
| Motor run-in (FMCRD electric backup) | `MotorRunIn` | `ROD_SPEED` toward 0 | every scram, in parallel with hydraulic |
| Stuck rod | `stuck[i] = true` | none | `Fault::RodStuck { index }` only |

`Scram` and `Sri` move at the same speed but are distinct states: only `Scram`
counts as "scramming" (the SCRAM lamp), matching the previous behaviour where
an SRI did not light the scram indication. A scram converts `Sri` rods to
`Scram`; an SRI never downgrades a `Scram` rod. `Sri` rods return to `Idle` at
position 0. `Rods::target(i)` reports the commanded endpoint (motor target,
0 for any insertion mechanism, own position when idle), which is what the
rod display shows as TGT.

Initiation paths are layered: `Sim::scram()` (RPS latch `Sim.scrammed`) calls
`rods.scram_hydraulic()` then `rods.motor_run_in()`. An alternate rod
insertion path (`Sim::alternate_rod_insertion()`, separate latch, guarded by
`ARI_ENABLED = false` until its signals are sourced) is a Package 8 item that
reuses the same two rod methods.

FMCRD dual insertion (hydraulic plus electric run-in on the same drive) is a
shared concept with ESBWR (DCD Rev. 10 Tier 2 Ch. 4.6, see
esbwr-applicability.csv). BWRX-specific stroke times are not public; the
values above are assumptions with the stated sensitivity (A-053).

Scram reset requires every `Scram` and `MotorRunIn` rod to be at position 0.
A stuck rod is never commanded (it stays `Idle`), so it does not by itself
block reset; it remains out, and the operator sees TGT = POS = its stuck
position while the fault is active.

## 3. Reactivity

`ReactivityCoeffs { void, doppler, mod_t, xenon_worth }` holds the feedback
coefficients; defaults are the `params.rs` constants. All are **calibrated**
(not confirmed): the set is tuned so the hot-full-power, equilibrium-xenon core
is critical with a rod insertion fraction inside the 5-25 % plausibility band
tested by `core_transients::full_power_rod_insertion_fraction`.

| Constant | Value | Status |
|---|---|---|
| `RHO_EXCESS_COLD` | 0.130 dk/k | calibrated |
| `ROD_WORTH_TOTAL` | 0.200 dk/k | calibrated |
| `DOPPLER_A` | 0.0024 dk/k per sqrt(K) | assumed |
| `VOID_COEF` | -0.080 dk/k per unit void | assumed |
| `MOD_TEMP_COEF` | -6.0e-5 dk/k per K | assumed |
| `XENON_WORTH_EQ` | 0.028 dk/k | assumed |

Exposure dependence is not modelled. BOC/EOC reference cases are
`unavailable` in parameters.csv. Sensitivity of the turbine-trip transient to
±25 % on void and Doppler coefficients is produced by
`cargo run --release -p bwr-core --example sensitivity` and recorded in
docs/validation/results.md (P10).

## 4. Shutdown margin

`Rods::shutdown_margin(rho_other, withdrawn)` returns
`-(RHO_EXCESS_COLD + rho_other + rho_rods)` with all rods in except those
listed. Cold, xenon-free conditions mean fuel 20 degC, void 0, moderator
20 degC, xenon 0, for which `rho_other = 0`.

The plant SDM requirement is a sourced value from NEDO-33912-A Rev. 2
(reactivity control). It is recorded in parameters.csv as `SDM_REQUIRED`
with status `confirmed` only if the public document states the number;
otherwise the row has status `unavailable` and the tests assert SDM > 0 only.
Reported values (all-rods-in, strongest-rod-out, strongest-pair-out) are
evidence of the calibrated model, not plant performance.

## 5. Fuel thermal model and envelope

Two lumped nodes: average fuel (`fuel_t`) and average cladding (`clad_t`).

- Pellet-plus-gap resistance `GAP_RESISTANCE_K_PER_W` = 252 K / (870e6 W x 0.975), assumed (A-054): average fuel ~552 degC at rated.
- Clad-to-coolant resistance `CLAD_COOLANT_RESISTANCE_K_PER_W` = 12 K / (870e6 W x 0.975), assumed (A-054): clad ~12 K above coolant at rated. Scaled by 1/htc_factor when the core uncovers.
- `CLAD_HEAT_CAP_J_PER_K` = 3.1e6 J/K (about 9.5 t Zircaloy x 330 J/kg K), assumed (A-055).
- Sum of the two resistances equals the former single-node `FUEL_RESISTANCE` (264 K at rated), so rated fuel temperature is unchanged.

**Envelope.** The model is valid for covered, forced-free natural circulation
cooling with cladding below `FUEL_MODEL_T_MAX_C` = 1000 degC. When
`clad_t > FUEL_MODEL_T_MAX_C`, or the core is uncovered for more than
`UNCOVERED_ENVELOPE_S` = 60 s, `DomainError::FuelModelEnvelope` is raised,
`Snapshot.fuelEnvelopeExceeded` becomes true, and alarm `fuel_envelope`
("FUEL THERMAL MODEL ENVELOPE EXCEEDED") is shown. This is **not** a damage
limit. **Peak cladding temperature and critical power ratio are not
computed**; the model has no hot-channel, no CPR correlation, and no
post-dryout heat transfer. The former "CORE DAMAGE at 1200 degC" verdict is
removed because it presented an unsupported conclusion.

## 6. Decay heat

Nine exponential groups fitted by `tools/fit_decay_heat.py` to
`docs/model/decay-heat-reference.csv` (the `source` column names the public
reference; see sources.csv). The fit is to a transcribed table, not a
standards implementation. Maximum relative error of the fit is printed by the
tool and must be below 5 %.

Finite irradiation history: `DecayHeat::after_history(n, irr_s, cooled_s)`
uses `d_i = f_i * n * (1 - exp(-lambda_i * irr_s)) * exp(-lambda_i * cooled_s)`.
Scenarios use `IRRADIATION_HISTORY_S` = 6.31e7 s (2 years at rated, assumed,
A-056). Uncertainty: a +/-10-20 % band is typical for total decay heat without
actinide and exposure detail; this band is an assumption (A-057), not a
standards claim.

## 7. Xenon / iodine

Normalised two-nuclide model (reactor.rs `Xenon`). Verified behaviours
(`core_transients.rs`): stationary equilibrium at 30/60/100 % power, post-
shutdown peak between 6 h and 14 h, burnout on restart from the peak, and the
transient dip after a 50 % to 100 % power rise. Cross-section-times-flux
`SIGMA_PHI_XE` is assumed (A-058).
```

- [ ] **Step 2: Append parameter rows**

Append to `docs/model/parameters.csv`, in that file's header column order (the P1 header contains at least `name,value,unit,status`; fill the remaining P1 columns with the definition and source text shown here):

```
GAP_RESISTANCE_K_PER_W,2.9712e-7,K/W,assumed,Pellet plus gap thermal resistance: 252 K at 848.25 MW,A-054
CLAD_COOLANT_RESISTANCE_K_PER_W,1.4148e-8,K/W,assumed,Clad to coolant resistance: 12 K at 848.25 MW,A-054
CLAD_HEAT_CAP_J_PER_K,3.1e6,J/K,assumed,Cladding lumped heat capacity (9.5 t Zircaloy x 330 J/kgK),A-055
FUEL_MODEL_T_MAX_C,1000,degC,assumed,Cladding temperature above which the thermal model is outside its envelope (not a damage limit),A-054
UNCOVERED_ENVELOPE_S,60,s,assumed,Continuous core-uncovered duration beyond which the thermal model is outside its envelope,A-054
IRRADIATION_HISTORY_S,6.31e7,s,assumed,Irradiation time at rated power assumed for scenario decay-heat initialisation (2 years),A-056
SDM_REQUIRED,,dk/k,unavailable,Plant shutdown margin requirement; set from NEDO-33912-A Rev. 2 only if the public text states a number,core.md section 4
ROD_SPEED,0.4,positions/s,assumed,FMCRD normal drive speed (~30 mm/s),A-053
SCRAM_TIME,3.0,s,assumed,Hydraulic scram full-stroke time,A-053
ROD_WORTH_TOTAL,0.200,dk/k,calibrated,Total rod worth; tuned with RHO_EXCESS_COLD for critical HFP with 5-25 % insertion,core.md section 3
RHO_EXCESS_COLD,0.130,dk/k,calibrated,Cold clean all-rods-out excess reactivity,core.md section 3
FUEL_RESISTANCE,,K/W,removed,Replaced by GAP_RESISTANCE_K_PER_W + CLAD_COOLANT_RESISTANCE_K_PER_W in Package 5,core.md section 5
```

If a row for `ROD_SPEED`, `SCRAM_TIME`, `ROD_WORTH_TOTAL`, or `RHO_EXCESS_COLD` already exists from P1, update its status/note instead of adding a duplicate.

- [ ] **Step 3: Append assumptions**

Append to `docs/model/assumptions.md`:

```markdown
## A-050 Active fuel length
3.81 m, typical GNF2. Sensitivity: +/-5 % changes void-weighted elevations and core volume proportionally; no BWRX public value.

## A-051 Rod position convention
0 = full in, 48 = full out, 2 positions per notch (legacy BWR convention). BWRX FMCRD position encoding is not public.

## A-052 Rod layout and relative worth
57 rods on a 9x9 grid inside radius sqrt(17); relative worth from J0^2 radial weighting and a cosine-integral axial S-curve. Not the plant control-rod map.

## A-053 Rod speeds
Normal drive 0.4 positions/s (~30 mm/s); hydraulic scram 3.0 s full stroke; motor run-in at normal drive speed. Sensitivity: scram time 2-4 s changes the first-second power drop only.

## A-054 Fuel thermal resistances and envelope
Pellet+gap 252 K and clad-coolant 12 K at rated (sum equals former single-node 264 K). Model envelope: clad < 1000 degC and core covered (or uncovered < 60 s). Outside: DomainError::FuelModelEnvelope. PCT and CPR are not computed.

## A-055 Cladding heat capacity
3.1e6 J/K (9.5 t Zircaloy x 330 J/kg K). Affects only sub-second clad response.

## A-056 Irradiation history for scenarios
2 years at rated power (6.31e7 s) before the stated cooling time. Affects long-lived decay-heat groups by < 10 % at 30 days.

## A-057 Decay-heat uncertainty band
+/-10-20 % on total decay heat (no actinide, exposure, or bundle-history detail). Not an ANS-5.1 uncertainty statement.

## A-058 Xenon cross-section times flux
SIGMA_PHI_XE = 2.65e-18 cm2 x 3.0e13 n/cm2/s. Affects equilibrium xenon fraction and peak time by the stated model only.
```

- [ ] **Step 4: Append applicability rows**

Append to `docs/model/esbwr-applicability.csv` (P1 header: ESBWR source/section, target subsystem, supporting BWRX evidence, relevant differences, required adaptation, applicability limits, associated test, classification):

```
DCD Rev.10 Tier 2 Ch. 4.3 (nuclear design: feedback mechanisms),core reactivity,GE 005N9751 (natural circulation BWR; GNF2),smaller core and different exposure history,coefficients kept as calibrated ReactivityCoeffs with sensitivity study,qualitative sign and order only,core_transients::full_power_rod_insertion_fraction; examples/sensitivity.rs,shared concept
DCD Rev.10 Tier 2 Ch. 4.6 (FMCRD: hydraulic scram plus electric run-in),rod mechanisms,GE 005N9751 (57 FMCRDs; NEDO-33912-A reactivity control),57 drives vs 269; stroke times not public,RodMotion::Scram / RodMotion::Sri (hydraulic) and RodMotion::MotorRunIn (electric) as distinct states with assumed speeds,timing values assumed (A-053),core_transients::scram_hydraulic_three_seconds; core_transients::motor_run_in_120_seconds,adapted model
DCD Rev.10 Tier 2 Ch. 15 (transient mechanisms: turbine trip reactivity response),core transient verification,none numerical,bypass capacity and SRI group differ,used only to order the expected response (void collapse then Doppler); no numerical trace imported,qualitative,examples/sensitivity.rs,shared concept
```

- [ ] **Step 5: Commit**

```bash
git add docs/model/core.md docs/model/parameters.csv docs/model/assumptions.md docs/model/esbwr-applicability.csv
git commit -m "docs(core): Package 5 core model specification, parameters, assumptions

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 2: Rod motion state machine

**Files:**
- Modify: `crates/bwr-core/src/reactor.rs` (replace `Rods`)
- Modify: `crates/bwr-core/src/sim.rs` (every `rods.target`, `rods.scramming`, `rods.fast_insert` use)
- Create: `crates/bwr-core/tests/core_transients.rs`

**Interfaces:**
- Consumes: `Core::step_rods(dt)` (P3) which calls `self.rods.step(dt)`.
- Produces (master contract): `RodMotion`, `Rods { layout, weights, pos, motion, stuck, selected }`, `Rods::{target, targets, set_motor_target, scram_hydraulic, motor_run_in, select_rod_insert, scramming, moving, stop, step, all_in, reactivity, sri_group}`.

- [ ] **Step 1: Write the failing tests**

Create `crates/bwr-core/tests/core_transients.rs`:

```rust
//! Package 5: core physics, fuel response and rod mechanism tests.

use bwr_core::api::{Command, ScenarioName};
use bwr_core::params::*;
use bwr_core::reactor::{RodMotion, Rods};
use bwr_core::sim::DT;
use bwr_core::Sim;

fn run(sim: &mut Sim, seconds: f64) {
    for _ in 0..(seconds / DT).round() as usize {
        sim.step(DT);
    }
}

fn step_rods(r: &mut Rods, seconds: f64) {
    for _ in 0..(seconds / DT).round() as usize {
        r.step(DT);
    }
}

#[test]
fn motor_motion_reaches_target_and_idles() {
    let mut r = Rods::new(0.0);
    r.set_motor_target(3, 4.0);
    assert_eq!(r.motion[3], RodMotion::Motor { target: 4.0 });
    assert_eq!(r.target(3), 4.0);
    assert!(r.moving());
    step_rods(&mut r, 4.0 / ROD_SPEED + DT);
    assert!((r.pos[3] - 4.0).abs() < 1e-9, "pos {}", r.pos[3]);
    assert_eq!(r.motion[3], RodMotion::Idle);
    assert!(!r.moving());
    assert_eq!(r.targets()[3], 4.0);
}

#[test]
fn scram_hydraulic_three_seconds() {
    let mut r = Rods::new(ROD_FULL_OUT);
    r.scram_hydraulic();
    assert!(r.scramming());
    step_rods(&mut r, SCRAM_TIME - DT);
    assert!(!r.all_in());
    step_rods(&mut r, 2.0 * DT);
    assert!(r.all_in());
    assert!(!r.scramming());
    assert!(r.motion.iter().all(|m| *m == RodMotion::Idle));
}

#[test]
fn motor_run_in_120_seconds() {
    let mut r = Rods::new(ROD_FULL_OUT);
    r.motor_run_in();
    assert!(r.motion.iter().all(|m| *m == RodMotion::MotorRunIn));
    assert!(!r.scramming(), "run-in is not a hydraulic scram");
    step_rods(&mut r, ROD_FULL_OUT / ROD_SPEED - 1.0);
    assert!(!r.all_in());
    step_rods(&mut r, 2.0);
    assert!(r.all_in());
}

#[test]
fn stuck_rod_ignores_every_mechanism() {
    let mut r = Rods::new(ROD_FULL_OUT);
    r.stuck[7] = true;
    r.set_motor_target(7, 0.0);
    step_rods(&mut r, 10.0);
    assert_eq!(r.pos[7], ROD_FULL_OUT);
    r.scram_hydraulic();
    r.motor_run_in();
    step_rods(&mut r, 5.0);
    assert_eq!(r.pos[7], ROD_FULL_OUT, "stuck rod moved under scram");
    assert!(r.pos.iter().enumerate().all(|(i, &p)| i == 7 || p == 0.0));
    assert!(!r.moving(), "a stuck rod must not count as moving");
}

#[test]
fn stop_parks_at_nearest_notch_in_direction_of_travel() {
    let mut r = Rods::new(0.0);
    r.set_motor_target(0, 10.0);
    step_rods(&mut r, 2.0 / ROD_SPEED + 0.5); // ~2.2 positions
    r.stop();
    assert_eq!(r.motion[0], RodMotion::Motor { target: 4.0 });
    step_rods(&mut r, 10.0);
    assert!((r.pos[0] - 4.0).abs() < 1e-9);
    // Hydraulic motion is not stoppable.
    r.scram_hydraulic();
    r.stop();
    assert!(r.scramming());
}

#[test]
fn select_rod_insert_is_hydraulic_on_the_sri_group_only() {
    let mut r = Rods::new(ROD_FULL_OUT);
    r.select_rod_insert();
    let g = r.sri_group();
    assert_eq!(g.len(), 4);
    for i in 0..N_RODS {
        let expect = if g.contains(&i) { RodMotion::Hydraulic } else { RodMotion::Idle };
        assert_eq!(r.motion[i], expect, "rod {i}");
    }
    step_rods(&mut r, SCRAM_TIME + DT);
    for &i in &g {
        assert_eq!(r.pos[i], 0.0);
    }
}

#[test]
fn full_power_snapshot_targets_follow_motion() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 1.0);
    let before = sim.snapshot();
    assert_eq!(before.rods.targets, before.rods.positions, "idle rods target their own position");
    sim.apply(Command::ManualScram);
    let s = sim.snapshot();
    assert!(s.rods.scram_in_progress);
    assert!(s.rods.targets.iter().all(|&t| t == 0.0));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --release -p bwr-core --test core_transients 2>&1 | head -20`
Expected: compile error `no variant or associated item named ... RodMotion` / `cannot find type RodMotion in module bwr_core::reactor`.

- [ ] **Step 3: Replace `Rods` in `reactor.rs`**

Replace the whole `pub struct Rods { ... }` and `impl Rods { ... }` block (from `#[derive(Clone, Debug)] pub struct Rods` through the end of `pub fn stop`) with:

```rust
/// How a rod is being driven right now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RodMotion {
    Idle,
    /// Normal fine-motion drive toward `target` at `ROD_SPEED`.
    Motor { target: f64 },
    /// Hydraulic scram insertion at `ROD_FULL_OUT / SCRAM_TIME`.
    Hydraulic,
    /// Electric motor run-in toward 0 at `ROD_SPEED` (FMCRD backup / ARI path).
    MotorRunIn,
}

#[derive(Clone, Debug)]
pub struct Rods {
    pub layout: Vec<(usize, usize)>,
    pub weights: Vec<f64>,
    pub pos: Vec<f64>,
    pub motion: Vec<RodMotion>,
    /// Mirrored from `Fault::RodStuck` by `Sim` every step. A stuck rod never moves.
    pub stuck: Vec<bool>,
    pub selected: Option<usize>,
}

impl Rods {
    pub fn new(pos: f64) -> Self {
        let layout = rod_layout();
        let weights = rod_weights(&layout);
        Self {
            layout,
            weights,
            pos: vec![pos; N_RODS],
            motion: vec![RodMotion::Idle; N_RODS],
            stuck: vec![false; N_RODS],
            selected: None,
        }
    }

    pub fn reactivity(&self) -> f64 {
        let inserted: f64 = self
            .pos
            .iter()
            .zip(&self.weights)
            .map(|(p, w)| w * s_curve((ROD_FULL_OUT - p) / ROD_FULL_OUT))
            .sum();
        -ROD_WORTH_TOTAL * inserted
    }

    pub fn all_in(&self) -> bool {
        self.pos.iter().all(|&p| p <= 0.01)
    }

    /// Where rod `i` is heading: motor target, 0 for insertion mechanisms, own position when idle.
    pub fn target(&self, i: usize) -> f64 {
        match self.motion[i] {
            RodMotion::Motor { target } => target,
            RodMotion::Hydraulic | RodMotion::MotorRunIn => 0.0,
            RodMotion::Idle => self.pos[i],
        }
    }

    pub fn targets(&self) -> Vec<f64> {
        (0..self.pos.len()).map(|i| self.target(i)).collect()
    }

    /// Command normal motor motion. Ignored while an insertion mechanism owns the rod.
    pub fn set_motor_target(&mut self, i: usize, target: f64) {
        if matches!(self.motion[i], RodMotion::Hydraulic | RodMotion::MotorRunIn) {
            return;
        }
        self.motion[i] = if (target - self.pos[i]).abs() < 1e-6 {
            RodMotion::Idle
        } else {
            RodMotion::Motor { target }
        };
    }

    /// Hydraulic scram on every rod that is not stuck.
    pub fn scram_hydraulic(&mut self) {
        for i in 0..self.pos.len() {
            if !self.stuck[i] {
                self.motion[i] = RodMotion::Hydraulic;
            }
        }
    }

    /// Electric run-in on every rod that is not stuck, not already hydraulic, and not at 0.
    pub fn motor_run_in(&mut self) {
        for i in 0..self.pos.len() {
            if !self.stuck[i] && self.motion[i] != RodMotion::Hydraulic && self.pos[i] > 0.0 {
                self.motion[i] = RodMotion::MotorRunIn;
            }
        }
    }

    /// Hydraulically insert the Select Rod Insert group.
    pub fn select_rod_insert(&mut self) {
        for i in self.sri_group() {
            if !self.stuck[i] {
                self.motion[i] = RodMotion::Hydraulic;
            }
        }
    }

    /// True while any rod is under hydraulic insertion.
    pub fn scramming(&self) -> bool {
        self.motion.iter().any(|m| *m == RodMotion::Hydraulic)
    }

    /// True while any non-stuck rod is being driven.
    pub fn moving(&self) -> bool {
        (0..self.pos.len()).any(|i| self.motion[i] != RodMotion::Idle && !self.stuck[i])
    }

    /// Stop motor motion at the nearest notch in the direction of travel.
    /// Hydraulic and run-in insertions cannot be stopped.
    pub fn stop(&mut self) {
        for i in 0..self.pos.len() {
            if let RodMotion::Motor { target } = self.motion[i] {
                let p = self.pos[i];
                let notch = if target > p { (p / 2.0).ceil() * 2.0 } else { (p / 2.0).floor() * 2.0 };
                self.motion[i] = if (notch - p).abs() < 1e-6 { RodMotion::Idle } else { RodMotion::Motor { target: notch } };
            }
        }
    }

    pub fn step(&mut self, dt: f64) {
        let v_motor = ROD_SPEED * dt;
        let v_hyd = ROD_FULL_OUT / SCRAM_TIME * dt;
        for i in 0..self.pos.len() {
            if self.stuck[i] {
                continue;
            }
            match self.motion[i] {
                RodMotion::Idle => {}
                RodMotion::Motor { target } => {
                    let d = target - self.pos[i];
                    self.pos[i] += d.clamp(-v_motor, v_motor);
                    if (self.pos[i] - target).abs() < 1e-9 {
                        self.pos[i] = target;
                        self.motion[i] = RodMotion::Idle;
                    }
                }
                RodMotion::MotorRunIn => {
                    self.pos[i] = (self.pos[i] - v_motor).max(0.0);
                    if self.pos[i] <= 0.0 {
                        self.motion[i] = RodMotion::Idle;
                    }
                }
                RodMotion::Hydraulic => {
                    self.pos[i] = (self.pos[i] - v_hyd).max(0.0);
                    if self.pos[i] <= 0.0 {
                        self.motion[i] = RodMotion::Idle;
                    }
                }
            }
        }
    }

    /// Rods in the Select Rod Insert group: four symmetric rods worth ~1.4 % dk/k.
    pub fn sri_group(&self) -> Vec<usize> {
        (0..self.layout.len())
            .filter(|&i| {
                let (c, r) = self.layout[i];
                (c as i32 - 4).abs() == 2 && (r as i32 - 4).abs() == 2
            })
            .collect()
    }
}
```

- [ ] **Step 4: Migrate call sites in `sim.rs`**

Add `RodMotion` to the reactor import:

```rust
use crate::reactor::{Core, DecayHeat, RodMotion, Xenon, ROD_GRID};
```

In `full_power()`, the rod-insertion loop currently sets both `pos` and `target`; replace

```rust
                let p = (s.core.rods.pos[i] - 2.0).max(0.0);
                s.core.rods.pos[i] = p;
                s.core.rods.target[i] = p;
```
with
```rust
                let p = (s.core.rods.pos[i] - 2.0).max(0.0);
                s.core.rods.pos[i] = p;
```

In `scram()`, replace `self.core.rods.scramming = true;` with

```rust
        self.core.rods.scram_hydraulic();
        self.core.rods.motor_run_in();
```

In `apply`, `Command::ResetScram` arm, replace the final `else { ... }` with

```rust
                } else if !self.core.rods.all_in() {
                    self.event("scram_reset", "Scram reset blocked: rods not fully inserted");
                } else {
                    self.scrammed = false;
                    self.scram_reason = None;
                    for m in self.core.rods.motion.iter_mut() {
                        *m = RodMotion::Idle;
                    }
                }
```

In `Command::RodSetTarget`, replace the body of the per-index loop after the block check:

```rust
                    let rods = &mut self.core.rods;
                    if !rods.scramming() {
                        rods.set_motor_target(i, target);
                    }
```

(The earlier `if target > rods.pos[i] + 1e-6 { if let Some(b) = block { ... continue; } }` stays as is.)

In `protection()`, replace the "Stop withdrawals if a rod block appears" block with

```rust
        // Stop withdrawals if a rod block appears: finish the current notch, no further.
        if self.withdraw_block().is_some() {
            let rods = &mut self.core.rods;
            for i in 0..rods.pos.len() {
                if let RodMotion::Motor { target } = rods.motion[i] {
                    let p = rods.pos[i];
                    if target > p + 1e-6 {
                        let notch = ((p / 2.0).ceil() * 2.0).min(p + 2.0).max(p);
                        rods.motion[i] = if notch - p < 1e-6 { RodMotion::Idle } else { RodMotion::Motor { target: notch } };
                    }
                }
            }
        }
```

In `snapshot()`, replace

```rust
                targets: c.rods.target.clone(),
                ...
                scram_in_progress: c.rods.scramming,
```
with
```rust
                targets: c.rods.targets(),
                ...
                scram_in_progress: c.rods.scramming(),
```

Search for any remaining uses: `grep -n "rods.target\b\|rods.scramming\b\|fast_insert\|\.target\[" crates/bwr-core/src crates/bwr-core/tests crates/bwr-core/examples`. Expected: no matches except `rods.target(` / `rods.targets(` calls.

- [ ] **Step 5: Run the whole suite**

Run: `cargo test --release -p bwr-core 2>&1 | grep -E "^test |test result"`
Expected: every test `ok`, including the 7 new `core_transients` tests and all pre-existing `plant`, `numerics`, `conservation`, `vessel_transients` tests. `stuck_rod_ignores_every_mechanism` passes without `Sim` involvement because it sets `stuck` directly.

- [ ] **Step 6: Commit**

```bash
git add crates/bwr-core/src/reactor.rs crates/bwr-core/src/sim.rs crates/bwr-core/tests/core_transients.rs
git commit -m "feat(core): per-rod motion state machine with hydraulic, motor and run-in mechanisms

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 3: Stuck-rod fault, dual-mechanism scram, and reset gating at plant level

**Files:**
- Modify: `crates/bwr-core/src/sim.rs` (`step`, scram path)
- Modify: `crates/bwr-core/tests/core_transients.rs`

**Interfaces:**
- Consumes: `Fault::RodStuck { index }`, `Sim::set_fault`, `Sim::has_fault` (P2).
- Produces: `rods.stuck` mirrored from faults each step; scram reset blocked while any rod is out.

- [ ] **Step 1: Write the failing tests**

Append to `crates/bwr-core/tests/core_transients.rs`:

```rust
use bwr_core::faults::Fault;

fn first_rod_fully_out(sim: &Sim) -> usize {
    (0..N_RODS).find(|&i| sim.core.rods.pos[i] >= ROD_FULL_OUT - 1e-9).expect("a fully withdrawn rod at full power")
}

#[test]
fn stuck_rod_fault_survives_scram_and_blocks_reset() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 1.0);
    let idx = first_rod_fully_out(&sim);
    sim.set_fault(Fault::RodStuck { index: idx }, true);
    run(&mut sim, DT);
    assert!(sim.core.rods.stuck[idx], "fault must be mirrored into rods.stuck");
    sim.apply(Command::ManualScram);
    run(&mut sim, 20.0);
    assert_eq!(sim.core.rods.pos[idx], ROD_FULL_OUT, "stuck rod moved");
    assert!(sim.core.rods.pos.iter().enumerate().all(|(i, &p)| i == idx || p == 0.0));
    assert!(sim.has_fault(&Fault::RodStuck { index: idx }), "automatic logic cleared the fault");
    sim.apply(Command::ResetScram);
    assert!(sim.scrammed, "reset must be blocked while a rod is out");
    let s = sim.snapshot();
    assert!(s.alarms.iter().any(|a| a.text.contains("rods not fully inserted")), "{:?}", s.alarms);
    // Clearing the fault lets the motor run-in finish and the reset succeed.
    sim.set_fault(Fault::RodStuck { index: idx }, false);
    sim.apply(Command::ManualScram); // re-issue so the freed rod receives a mechanism
    run(&mut sim, 5.0);
    assert!(sim.core.rods.all_in());
    sim.apply(Command::ResetScram);
    assert!(!sim.scrammed);
}

#[test]
fn scram_reset_clears_motion_when_all_in() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 1.0);
    sim.apply(Command::ManualScram);
    run(&mut sim, 20.0);
    assert!(sim.core.rods.all_in());
    sim.apply(Command::ResetScram);
    assert!(!sim.scrammed);
    assert!(sim.core.rods.motion.iter().all(|m| *m == RodMotion::Idle));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --release -p bwr-core --test core_transients stuck_rod_fault_survives_scram_and_blocks_reset 2>&1 | tail -5`
Expected: FAIL at `assert!(sim.core.rods.stuck[idx])` ("fault must be mirrored into rods.stuck").

- [ ] **Step 3: Mirror faults into the rods every step**

In `Sim::step`, as the very first statements (before `self.core.step_rods(dt)` from P3):

```rust
        // Faults are the only source of stuck rods; copy them so the mechanism
        // code never needs to know about the fault set.
        for i in 0..N_RODS {
            self.core.rods.stuck[i] = self.faults.contains(&Fault::RodStuck { index: i });
        }
```

Add `use crate::faults::Fault;` to the `sim.rs` imports if P2 did not already.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --release -p bwr-core --test core_transients 2>&1 | grep -E "^test |test result"`
Expected: all 9 tests `ok`.

- [ ] **Step 5: Commit**

```bash
git add crates/bwr-core/src/sim.rs crates/bwr-core/tests/core_transients.rs
git commit -m "feat(core): honour Fault::RodStuck and gate scram reset on all-rods-in

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 4: Two-node fuel and cladding thermal model

**Files:**
- Modify: `crates/bwr-core/src/params.rs` (remove `FUEL_RESISTANCE`; add three constants)
- Modify: `crates/bwr-core/src/reactor.rs` (`Core.clad_t`, `Core::step`)
- Modify: `crates/bwr-core/src/sim.rs` (`full_power` initial temperatures)
- Modify: `crates/bwr-core/src/api.rs`, `ui/src/sim/types.ts`, `ui/src/sim/mock.ts` (`cladTempC`)
- Modify: `crates/bwr-core/tests/core_transients.rs`

**Interfaces:**
- Consumes: `Core::step(dt, coolant_t, htc_factor) -> Result<f64, DomainError>` as left by P3 (rods are no longer stepped inside).
- Produces: `Core.clad_t: f64`, `Snapshot.neutronics.clad_temp_c`, constants `GAP_RESISTANCE_K_PER_W`, `CLAD_COOLANT_RESISTANCE_K_PER_W`, `CLAD_HEAT_CAP_J_PER_K`.

Derivation (write this into the `params.rs` comments): at rated, 870e6 W x (1 - 0.025) = 848.25e6 W reaches the coolant through the fuel. Choosing average fuel ~552 degC and clad 12 K above the 287.7 degC coolant gives pellet+gap delta 252 K and clad-coolant delta 12 K, so `R_gap = 252 / 848.25e6 = 2.9712e-7 K/W` and `R_clad = 12 / 848.25e6 = 1.4148e-8 K/W`. Their sum, 264 K / 848.25e6 W, equals the former single-node `FUEL_RESISTANCE`, so rated fuel temperature is unchanged.

Implicit two-node update (backward Euler, unconditionally stable). With `a = dt/(C_f R_g)`, `b = dt/(C_c R_g)`, `c = dt/(C_c R_c)`, `r_f = T_f + dt Q/C_f`, `r_c = T_c + c T_cool`:

```
(1+a) T_f' - a T_c'       = r_f
-b T_f'   + (1+b+c) T_c'  = r_c
det = (1+a)(1+b+c) - a b
T_f' = (r_f (1+b+c) + a r_c) / det
T_c' = ((1+a) r_c + b r_f) / det
q_conv = (T_c' - T_cool) / R_c
```

- [ ] **Step 1: Write the failing tests**

Append to `crates/bwr-core/tests/core_transients.rs`:

```rust
#[test]
fn rated_fuel_and_clad_temperatures_match_resistance_derivation() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 60.0);
    let t_cool = sim.vessel.eos.t;
    let q = sim.core.thermal_power() * (1.0 - DIRECT_COOLANT_HEAT);
    let clad_expected = t_cool + CLAD_COOLANT_RESISTANCE_K_PER_W * q;
    let fuel_expected = clad_expected + GAP_RESISTANCE_K_PER_W * q;
    assert!((sim.core.clad_t - clad_expected).abs() < 1.0, "clad {} vs {}", sim.core.clad_t, clad_expected);
    assert!((sim.core.fuel_t - fuel_expected).abs() < 2.0, "fuel {} vs {}", sim.core.fuel_t, fuel_expected);
    // Regression against the single-node baseline: 264 K above coolant at rated.
    let baseline = t_cool + 264.0 * sim.core.kin.n;
    assert!((sim.core.fuel_t - baseline).abs() < 5.0, "fuel {} vs baseline {}", sim.core.fuel_t, baseline);
    let s = sim.snapshot();
    assert!((s.neutronics.clad_temp_c - sim.core.clad_t).abs() < 1e-9);
}

#[test]
fn clad_responds_faster_than_fuel_on_scram() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    let (f0, c0) = (sim.core.fuel_t, sim.core.clad_t);
    sim.apply(Command::ManualScram);
    run(&mut sim, 2.0);
    let clad_drop = (c0 - sim.core.clad_t) / (c0 - sim.vessel.eos.t).max(1.0);
    let fuel_drop = (f0 - sim.core.fuel_t) / (f0 - sim.vessel.eos.t);
    assert!(clad_drop > fuel_drop, "clad {clad_drop} fuel {fuel_drop}");
    assert!(sim.core.clad_t >= sim.vessel.eos.t - 1e-6);
}

#[test]
fn two_node_update_is_stable_for_large_steps() {
    use bwr_core::reactor::Core;
    let mut c = Core::new(1.0, ROD_FULL_OUT, 800.0);
    c.update_reactivity(0.0, 287.0);
    for _ in 0..10 {
        c.step(5.0, 287.0, 1.0).unwrap();
        assert!(c.fuel_t.is_finite() && c.clad_t.is_finite());
        assert!(c.fuel_t >= c.clad_t - 1e-6 && c.clad_t >= 287.0 - 1e-6, "fuel {} clad {}", c.fuel_t, c.clad_t);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --release -p bwr-core --test core_transients rated_fuel 2>&1 | head -10`
Expected: compile error `no field clad_t on type Core` / `cannot find value CLAD_COOLANT_RESISTANCE_K_PER_W`.

- [ ] **Step 3: Parameters**

In `params.rs`, delete the two lines

```rust
/// Fuel-to-coolant thermal resistance, K/W (avg fuel ~550 degC at rated). ASSUMED.
pub const FUEL_RESISTANCE: f64 = 264.0 / (RATED_POWER_W * (1.0 - DIRECT_COOLANT_HEAT));
```

and insert in their place:

```rust
/// Heat reaching the coolant through the fuel at rated power, W.
const Q_FUEL_RATED: f64 = RATED_POWER_W * (1.0 - DIRECT_COOLANT_HEAT);
/// Pellet + gap thermal resistance, K/W: 252 K at rated (avg fuel ~552 degC). ASSUMED (A-054).
pub const GAP_RESISTANCE_K_PER_W: f64 = 252.0 / Q_FUEL_RATED;
/// Cladding-to-coolant resistance, K/W: 12 K at rated. ASSUMED (A-054).
/// GAP + CLAD = 264 K at rated, equal to the former single-node FUEL_RESISTANCE.
pub const CLAD_COOLANT_RESISTANCE_K_PER_W: f64 = 12.0 / Q_FUEL_RATED;
/// Cladding lumped heat capacity (~9.5 t Zircaloy x 330 J/kgK), J/K. ASSUMED (A-055).
pub const CLAD_HEAT_CAP_J_PER_K: f64 = 3.1e6;
```

- [ ] **Step 4: Cladding node in `Core`**

In `reactor.rs`, add the field after `fuel_t`:

```rust
    /// Lumped average fuel temperature, degC.
    pub fuel_t: f64,
    /// Lumped average cladding temperature, degC.
    pub clad_t: f64,
```

In `Core::new`, add `clad_t: fuel_t,` next to `fuel_t,`.

Replace the thermal part of `Core::step` (everything after `self.decay.step(self.kin.n, dt);`) so the whole function reads:

```rust
    /// Advance neutronics, xenon, decay heat and the fuel/cladding nodes.
    /// Rods are stepped separately by `step_rods` so reactivity is current.
    /// `coolant_t` is the coolant temperature, `htc_factor` scales clad-to-coolant
    /// heat transfer (1 = normal, lower when the core is uncovered).
    /// Returns heat delivered to the coolant, W.
    pub fn step(&mut self, dt: f64, coolant_t: f64, htc_factor: f64) -> Result<f64, DomainError> {
        self.kin.advance(self.rho_total, SOURCE, dt)?;
        self.xenon.step(self.kin.n, dt);
        self.decay.step(self.kin.n, dt);

        let p = self.thermal_power();
        let q = p * (1.0 - DIRECT_COOLANT_HEAT);
        let r_g = GAP_RESISTANCE_K_PER_W;
        let r_c = CLAD_COOLANT_RESISTANCE_K_PER_W / htc_factor.max(0.02);
        // Backward Euler on the two-node system (see docs/model/core.md section 5).
        let a = dt / (FUEL_HEAT_CAP * r_g);
        let b = dt / (CLAD_HEAT_CAP_J_PER_K * r_g);
        let c = dt / (CLAD_HEAT_CAP_J_PER_K * r_c);
        let r_f = self.fuel_t + dt * q / FUEL_HEAT_CAP;
        let r_cl = self.clad_t + c * coolant_t;
        let det = (1.0 + a) * (1.0 + b + c) - a * b;
        self.fuel_t = (r_f * (1.0 + b + c) + a * r_cl) / det;
        self.clad_t = ((1.0 + a) * r_cl + b * r_f) / det;
        let q_conv = (self.clad_t - coolant_t) / r_c;
        Ok(q_conv + p * DIRECT_COOLANT_HEAT)
    }
```

(If P3 left `use crate::domain::DomainError;` out of `reactor.rs`, add it.)

- [ ] **Step 5: Initial conditions in `sim.rs`**

In `full_power()`, replace

```rust
        // Fuel temperature at rated power.
        s.core.fuel_t = t + FUEL_RESISTANCE * RATED_POWER_W * (1.0 - DIRECT_COOLANT_HEAT);
```
with
```rust
        // Fuel and cladding temperatures at rated power.
        let q_fuel = RATED_POWER_W * (1.0 - DIRECT_COOLANT_HEAT);
        s.core.clad_t = t + CLAD_COOLANT_RESISTANCE_K_PER_W * q_fuel;
        s.core.fuel_t = s.core.clad_t + GAP_RESISTANCE_K_PER_W * q_fuel;
```

`cold_shutdown()` and `hot_standby()` call `Core::new(0.0, 0.0, t)` which now sets `clad_t = t`; no change needed.

- [ ] **Step 6: API and UI mirror**

`api.rs`, `Neutronics`: add `pub clad_temp_c: f64,` after `fuel_temp_c`. `sim.rs` `snapshot()`: add `clad_temp_c: c.clad_t,` after `fuel_temp_c: c.fuel_t,`.

`ui/src/sim/types.ts`, interface `Neutronics`: add after `fuelTempC: number;`:

```ts
  /** Lumped average cladding temperature, degC (two-node fuel model). */
  cladTempC: number;
```

`ui/src/sim/mock.ts`: find the object literal that builds `neutronics:` (grep `fuelTempC:`), and add immediately after it:

```ts
      cladTempC: Math.min(fuelTempC, coolantTempC + 12),
```

where `fuelTempC` and `coolantTempC` are the expressions already used for `fuelTempC:` and `vessel.coolantTempC:` in that literal (bind them to local `const`s above the literal if they are inline expressions).

- [ ] **Step 7: Run tests and UI typecheck**

Run: `cargo test --release -p bwr-core 2>&1 | grep -E "^test |test result"`
Expected: all `ok`, including the three new tests. `plant::full_power_is_steady` must still pass without retuning.

Run: `npm --prefix ui run typecheck`
Expected: exit 0, no output.

- [ ] **Step 8: Commit**

```bash
git add crates/bwr-core/src/params.rs crates/bwr-core/src/reactor.rs crates/bwr-core/src/sim.rs crates/bwr-core/src/api.rs crates/bwr-core/tests/core_transients.rs ui/src/sim/types.ts ui/src/sim/mock.ts
git commit -m "feat(core): two-node fuel/cladding thermal model replacing the single fuel node

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 5: Fuel model envelope replaces the "core damage" verdict

**Files:**
- Modify: `crates/bwr-core/src/params.rs` (`FUEL_MODEL_T_MAX_C`, `UNCOVERED_ENVELOPE_S`)
- Modify: `crates/bwr-core/src/sim.rs` (remove `core_damage`; add `uncovered_s`, envelope detection, alarm)
- Modify: `crates/bwr-core/src/api.rs`, `ui/src/sim/types.ts`, `ui/src/sim/mock.ts`, UI components that show "CORE DAMAGE"
- Modify: `crates/bwr-core/tests/core_transients.rs`

**Interfaces:**
- Consumes: `DomainError::FuelModelEnvelope { fuel_t_c, clad_t_c }`, `Sim::domain_errors: Vec<(f64, DomainError)>` (P3).
- Produces: `Sim::uncovered_s`, `Sim::fuel_envelope_exceeded()`, `Snapshot.fuel_envelope_exceeded`, alarm id `fuel_envelope`.

- [ ] **Step 1: Write the failing tests**

Append to `crates/bwr-core/tests/core_transients.rs`:

```rust
use bwr_core::domain::DomainError;

#[test]
fn hot_cladding_raises_fuel_model_envelope_once() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 1.0);
    assert!(!sim.fuel_envelope_exceeded());
    sim.core.clad_t = FUEL_MODEL_T_MAX_C + 1.0;
    sim.core.fuel_t = FUEL_MODEL_T_MAX_C + 300.0;
    run(&mut sim, DT);
    assert!(sim.fuel_envelope_exceeded());
    let n = sim.domain_errors.iter().filter(|(_, e)| matches!(e, DomainError::FuelModelEnvelope { .. })).count();
    run(&mut sim, 1.0);
    let n2 = sim.domain_errors.iter().filter(|(_, e)| matches!(e, DomainError::FuelModelEnvelope { .. })).count();
    assert_eq!((n, n2), (1, 1), "envelope error recorded once");
    let s = sim.snapshot();
    assert!(s.fuel_envelope_exceeded);
    assert!(s.alarms.iter().any(|a| a.id == "fuel_envelope" && a.text == "FUEL THERMAL MODEL ENVELOPE EXCEEDED"));
    assert!(s.alarms.iter().all(|a| !a.text.contains("CORE DAMAGE")), "core damage verdict must be gone");
    assert!(s.unsupported.is_some(), "P3 unsupported flag must also be set");
}

#[test]
fn prolonged_uncovery_raises_fuel_model_envelope() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 1.0);
    sim.uncovered_s = UNCOVERED_ENVELOPE_S - 2.0 * DT;
    // A covered core resets the timer; an uncovered one accumulates it.
    run(&mut sim, DT);
    assert_eq!(sim.uncovered_s, 0.0, "covered core must reset the timer");
    sim.uncovered_s = UNCOVERED_ENVELOPE_S + DT;
    sim.vessel_force_uncovered_for_test();
    run(&mut sim, DT);
    assert!(sim.fuel_envelope_exceeded());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --release -p bwr-core --test core_transients envelope 2>&1 | head -10`
Expected: compile error `no method named fuel_envelope_exceeded` / `cannot find value FUEL_MODEL_T_MAX_C`.

- [ ] **Step 3: Parameters**

Append to `params.rs` core section:

```rust
/// Cladding temperature above which the fuel thermal model is outside its
/// envelope. An indication, not a damage limit (A-054). ASSUMED.
pub const FUEL_MODEL_T_MAX_C: f64 = 1000.0;
/// Continuous core-uncovered time after which the thermal model is outside
/// its envelope (no post-dryout heat transfer modelled). ASSUMED (A-054).
pub const UNCOVERED_ENVELOPE_S: f64 = 60.0;
```

- [ ] **Step 4: Engine changes in `sim.rs`**

Remove the field `pub core_damage: bool,` from `Sim` and its initialiser `core_damage: false,` in `base()`. Add the field

```rust
    /// Continuous time the core has been uncovered, s (envelope timer).
    pub uncovered_s: f64,
```

initialised to `0.0` in `base()`.

If P3 did not already provide a recording helper, add this method to `impl Sim` (if P3 provided one with a different name, use P3's and skip this):

```rust
    /// Record the first occurrence of each domain-error variant and log it.
    pub fn record_domain_error(&mut self, e: DomainError) {
        let dup = self.domain_errors.iter().any(|(_, x)| std::mem::discriminant(x) == std::mem::discriminant(&e));
        if !dup {
            self.log.push((self.time, format!("domain error: {e:?}")));
            self.domain_errors.push((self.time, e));
        }
    }

    pub fn fuel_envelope_exceeded(&self) -> bool {
        self.domain_errors.iter().any(|(_, e)| matches!(e, DomainError::FuelModelEnvelope { .. }))
    }

    /// Test hook: drive the vessel level below the top of active fuel.
    #[doc(hidden)]
    pub fn vessel_force_uncovered_for_test(&mut self) {
        self.vessel.level_z = Z_CORE_BOTTOM + 0.5 * CORE_HEIGHT;
    }
```

In `protection()`, replace the "Core damage" block

```rust
        // Core damage.
        if self.core.fuel_t > 1200.0 && !self.core_damage {
            self.core_damage = true;
            self.event("core_damage", "CORE DAMAGE: fuel temperature exceeded 1200 degC");
        }
```
with
```rust
        // Fuel thermal model envelope (not a damage verdict; see docs/model/core.md section 5).
        if self.core.clad_t > FUEL_MODEL_T_MAX_C || self.uncovered_s > UNCOVERED_ENVELOPE_S {
            self.record_domain_error(DomainError::FuelModelEnvelope { fuel_t_c: self.core.fuel_t, clad_t_c: self.core.clad_t });
        }
```

In `step()`, immediately after `self.vessel.step(dt, &inputs)` (and the ledger record P2/P3 placed there), add:

```rust
        let covered = self.vessel.core_coverage() >= 0.999;
        self.uncovered_s = if covered { 0.0 } else { self.uncovered_s + dt };
```

In `active_alarms()`, replace

```rust
        if self.core_damage {
            add("core_damage", "CORE DAMAGE".into(), Severity::Trip);
        }
```
with
```rust
        if self.fuel_envelope_exceeded() {
            add("fuel_envelope", "FUEL THERMAL MODEL ENVELOPE EXCEEDED".into(), Severity::Trip);
        }
```

Add `use crate::domain::DomainError;` to the imports if absent.

- [ ] **Step 5: API and UI**

`api.rs` `Snapshot`: add `pub fuel_envelope_exceeded: bool,` after `scram_reason`. `sim.rs` `snapshot()`: add `fuel_envelope_exceeded: self.fuel_envelope_exceeded(),` after `scram_reason: ...`.

`ui/src/sim/types.ts` `Snapshot`: add after `scramReason: string | null;`:

```ts
  /** True once the fuel thermal model has left its declared envelope (not a damage verdict). */
  fuelEnvelopeExceeded: boolean;
```

Run: `grep -rn "CORE DAMAGE\|coreDamage\|core_damage" ui/src`
For each hit in `ui/src/sim/mock.ts`: replace the alarm id `core_damage` with `fuel_envelope`, the text `CORE DAMAGE...` with `FUEL THERMAL MODEL ENVELOPE EXCEEDED`, rename any `coreDamage` hidden-state field to `fuelEnvelopeExceeded`, and emit `fuelEnvelopeExceeded: <that field>` at the top level of the mock snapshot next to `scramReason`. For hits in `ui/src/components/*`: replace the displayed string with `FUEL MODEL ENVELOPE` and read `s.fuelEnvelopeExceeded` instead of matching alarm text. If `grep` returns no component hits, only `mock.ts` changes.

- [ ] **Step 6: Run tests, typecheck, build**

Run: `cargo test --release -p bwr-core 2>&1 | grep -E "^test |test result"`
Expected: all `ok`.

Run: `npm --prefix ui run typecheck && npm --prefix ui run build 2>&1 | tail -3`
Expected: typecheck silent; build ends with `✓ built in ...`.

- [ ] **Step 7: Commit**

```bash
git add crates/bwr-core/src/params.rs crates/bwr-core/src/sim.rs crates/bwr-core/src/api.rs crates/bwr-core/tests/core_transients.rs ui/src
git commit -m "feat(core): fuel thermal model envelope replaces the 1200 degC core damage verdict

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 6: Decay heat reference, fit tool, and finite irradiation history

**Files:**
- Create: `docs/model/decay-heat-reference.csv`
- Rewrite: `tools/fit_decay_heat.py`
- Modify: `crates/bwr-core/src/params.rs` (`DECAY_GROUPS` regenerated, `IRRADIATION_HISTORY_S`)
- Modify: `crates/bwr-core/src/reactor.rs` (`DecayHeat::after_history`, tests)
- Modify: `crates/bwr-core/src/sim.rs` (scenario initialisation)
- Modify: `docs/model/sources.csv`, `README.md`

**Interfaces:**
- Produces: `DecayHeat::after_history(n: f64, irr_s: f64, cooled_s: f64) -> DecayHeat`, `IRRADIATION_HISTORY_S`.

- [ ] **Step 1: Transcribe the reference table**

Create `docs/model/decay-heat-reference.csv` with the header

```
t_s,p_over_p0,source
```

and at least 12 rows transcribed from ONE named public reference for total decay heat after infinite irradiation at constant power. Acceptable choices: the ANSI/ANS-5.1 (any public edition) total-decay-heat table, or the Todreas & Kazimi *Nuclear Systems I* decay-heat table that the previous `fit_decay_heat.py` docstring cited. The `source` column holds the sources.csv document ID plus table/figure number, for example `TK-NS1,Table 3-?` (replace `?` with the actual table number from the book in hand). Cover at least 1 s, 10 s, 100 s, 1000 s, 1 h, 1 day, 1 week, 30 days. Add the document to `docs/model/sources.csv` with the fields P1 requires (ID, title, revision/edition, date, URL or ISBN, pages, retrieval date, checksum if a file). Do not type values from memory: transcribe from the open document and record the page.

- [ ] **Step 2: Rewrite `tools/fit_decay_heat.py`**

```python
"""Fit exponential decay-heat groups to a transcribed public reference curve.

Usage:
    python3 tools/fit_decay_heat.py docs/model/decay-heat-reference.csv [--groups 9] [--max-rel-err 0.05]

Prints the DECAY_GROUPS block for crates/bwr-core/src/params.rs on stdout and a
point-by-point comparison on stderr. Exits 1 if the maximum relative error of
the fit against the reference points exceeds --max-rel-err.
"""
import argparse
import csv
import sys

import numpy as np
from scipy.optimize import nnls


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("csv", help="reference table with columns t_s,p_over_p0,source")
    ap.add_argument("--groups", type=int, default=9)
    ap.add_argument("--max-rel-err", type=float, default=0.05)
    args = ap.parse_args()

    t, p, sources = [], [], set()
    with open(args.csv, newline="") as f:
        for row in csv.DictReader(f):
            t.append(float(row["t_s"]))
            p.append(float(row["p_over_p0"]))
            sources.add(row["source"])
    if len(t) < 12:
        print(f"need at least 12 reference points, got {len(t)}", file=sys.stderr)
        return 1
    order = np.argsort(t)
    t = np.array(t)[order]
    p = np.array(p)[order]

    # Group decay constants spread over the reference time span (log-spaced).
    lams = np.logspace(-0.3, -7.5, args.groups)
    # Dense log-log interpolation so sparse table points do not dominate the fit.
    tg = np.logspace(np.log10(t.min()), np.log10(t.max()), 200)
    pg = np.exp(np.interp(np.log(tg), np.log(t), np.log(p)))
    A = np.exp(-np.outer(tg, lams))
    w = 1.0 / pg  # relative-error weighting
    f, _ = nnls(A * w[:, None], pg * w)

    ref_fit = (f[None, :] * np.exp(-np.outer(t, lams))).sum(axis=1)
    err = float(np.max(np.abs(ref_fit / p - 1.0)))

    print(f"/// Decay heat groups (fraction of rated power at equilibrium, decay constant 1/s).")
    print(f"/// Fitted by tools/fit_decay_heat.py to docs/model/decay-heat-reference.csv")
    print(f"/// (source: {', '.join(sorted(sources))}); max relative error {err:.2%} at the reference points.")
    print(f"pub const DECAY_GROUPS: [(f64, f64); {args.groups}] = [")
    for lam, ff in zip(lams, f):
        print(f"    ({ff:.6e}, {lam:.6e}),")
    print("];")

    print(f"sum f = {f.sum():.5f}", file=sys.stderr)
    for tt, pp, ff in zip(t, p, ref_fit):
        print(f"t={tt:10.0f}  ref={pp:.5f}  fit={ff:.5f}  rel={ff / pp - 1:+.3%}", file=sys.stderr)
    if err > args.max_rel_err:
        print(f"fit error {err:.2%} exceeds {args.max_rel_err:.0%}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
```

Run: `python3 tools/fit_decay_heat.py docs/model/decay-heat-reference.csv > /tmp/decay_groups.rs; echo exit=$?`
Expected: `exit=0`, and `/tmp/decay_groups.rs` holds a 9-row `DECAY_GROUPS` block. If the exit code is 1, the transcription has an error or the reference needs more points; fix the CSV, do not loosen `--max-rel-err`.

- [ ] **Step 3: Write the failing Rust tests**

In `reactor.rs` `mod tests`, replace `decay_heat_curve` with:

```rust
    fn decay_reference() -> Vec<(f64, f64)> {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/model/decay-heat-reference.csv");
        let text = std::fs::read_to_string(path).expect("docs/model/decay-heat-reference.csv");
        text.lines()
            .skip(1)
            .filter(|l| !l.trim().is_empty())
            .map(|l| {
                let c: Vec<&str> = l.split(',').collect();
                (c[0].trim().parse().unwrap(), c[1].trim().parse().unwrap())
            })
            .collect()
    }

    #[test]
    fn decay_heat_matches_transcribed_reference_within_5_percent() {
        let r = decay_reference();
        assert!(r.len() >= 12, "reference needs >= 12 points");
        for (t, p) in r {
            let got = DecayHeat::after_shutdown(t).power();
            assert!((got / p - 1.0).abs() < 0.05, "t={t}: fit {got} vs reference {p}");
        }
    }

    #[test]
    fn stepped_decay_heat_matches_closed_form() {
        let mut d = DecayHeat::equilibrium(1.0);
        for _ in 0..3600 {
            d.step(0.0, 1.0);
        }
        let closed = DecayHeat::after_shutdown(3600.0).power();
        assert!((d.power() - closed).abs() < 1e-6, "{} vs {}", d.power(), closed);
    }

    #[test]
    fn infinite_history_equals_after_shutdown() {
        for &t in &[0.0, 10.0, 3600.0, 86400.0, 30.0 * 86400.0] {
            let a = DecayHeat::after_history(1.0, f64::INFINITY, t).power();
            let b = DecayHeat::after_shutdown(t).power();
            assert!((a - b).abs() < 1e-12, "t={t}: {a} vs {b}");
        }
    }

    #[test]
    fn finite_history_gives_less_decay_heat() {
        let short = DecayHeat::after_history(1.0, 30.0 * 86400.0, 86400.0).power();
        let long = DecayHeat::after_history(1.0, IRRADIATION_HISTORY_S, 86400.0).power();
        let inf = DecayHeat::after_shutdown(86400.0).power();
        assert!(short < long && long <= inf, "{short} {long} {inf}");
        assert!(DecayHeat::after_history(0.5, 1e6, 100.0).power() < DecayHeat::after_history(1.0, 1e6, 100.0).power());
    }
```

Run: `cargo test --release -p bwr-core --lib decay 2>&1 | head -10`
Expected: compile error `no function or associated item named after_history`.

- [ ] **Step 4: Implement**

`params.rs`: replace the existing `DECAY_GROUPS` doc comment and array with the contents of `/tmp/decay_groups.rs`. Append after it:

```rust
/// Irradiation time at rated power assumed before each scenario's cooling period, s (2 years). ASSUMED (A-056).
pub const IRRADIATION_HISTORY_S: f64 = 6.31e7;
```

`reactor.rs`, in `impl DecayHeat`, add after `after_shutdown`:

```rust
    /// Decay heat after operating `irr_s` seconds at relative power `n`, then
    /// cooling for `cooled_s` seconds. `irr_s = f64::INFINITY` reproduces `after_shutdown`.
    pub fn after_history(n: f64, irr_s: f64, cooled_s: f64) -> Self {
        let mut d = [0.0; 9];
        for (i, (f, l)) in DECAY_GROUPS.iter().enumerate() {
            d[i] = f * n * (1.0 - (-l * irr_s).exp()) * (-l * cooled_s).exp();
        }
        Self { d }
    }
```

`sim.rs`: in `cold_shutdown()` replace `core.decay = DecayHeat::after_shutdown(30.0 * 86400.0);` with `core.decay = DecayHeat::after_history(1.0, IRRADIATION_HISTORY_S, 30.0 * 86400.0);` and in `hot_standby()` replace `DecayHeat::after_shutdown(2.0 * 86400.0)` with `DecayHeat::after_history(1.0, IRRADIATION_HISTORY_S, 2.0 * 86400.0)`.

- [ ] **Step 5: Run tests**

Run: `cargo test --release -p bwr-core 2>&1 | grep -E "^test |test result"`
Expected: all `ok`. If `plant::cold_shutdown_is_subcritical_and_rods_blocked` or `plant::startup_from_hot_standby_to_30_percent` changes outcome, the cause is the refit, not the history term (the history term lowers decay heat by < 10 %); investigate the refit before touching assertions.

- [ ] **Step 6: README wording**

In `README.md` table row "Decay heat", replace `9-group fit to the standard decay heat curve (7.7 % at shutdown, 1.3 % after 1 h)` with `9-group fit to a transcribed public decay-heat table (docs/model/decay-heat-reference.csv), with finite irradiation history`. In the "Fuel" row replace `core damage above 1200 °C` with `an explicit model envelope flag instead of a damage verdict`.

- [ ] **Step 7: Commit**

```bash
git add docs/model/decay-heat-reference.csv docs/model/sources.csv tools/fit_decay_heat.py crates/bwr-core/src/params.rs crates/bwr-core/src/reactor.rs crates/bwr-core/src/sim.rs README.md
git commit -m "feat(core): decay heat fitted to a transcribed public reference with finite irradiation history

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 7: Xenon verification tests

**Files:**
- Modify: `crates/bwr-core/src/reactor.rs` (tests only)

**Interfaces:**
- Consumes: `Xenon::{equilibrium, step}`.

- [ ] **Step 1: Write the tests**

In `reactor.rs` `mod tests`, add (keep `xenon_peaks_after_shutdown`):

```rust
    #[test]
    fn xenon_equilibrium_is_stationary() {
        for &n in &[0.3, 0.6, 1.0] {
            let mut x = Xenon::equilibrium(n);
            let x0 = x.clone();
            for _ in 0..3600 {
                x.step(n, 1.0);
            }
            assert!((x.xenon - x0.xenon).abs() < 1e-6 && (x.iodine - x0.iodine).abs() < 1e-6, "n={n}: {x:?}");
        }
    }

    #[test]
    fn restart_from_xenon_peak_burns_xenon_out() {
        let mut x = Xenon::equilibrium(1.0);
        let mut peak = x.clone();
        for _ in 0..(14 * 3600) {
            x.step(0.0, 1.0);
            if x.xenon > peak.xenon {
                peak = x.clone();
            }
        }
        let mut r = peak.clone();
        for _ in 0..3600 {
            r.step(1.0, 1.0);
        }
        assert!(r.xenon < peak.xenon, "restart at 100 % must burn xenon: {} vs peak {}", r.xenon, peak.xenon);
    }

    #[test]
    fn power_rise_from_50_to_100_percent_dips_xenon_first() {
        let mut x = Xenon::equilibrium(0.5);
        let start = x.xenon;
        let mut min = start;
        for _ in 0..(12 * 3600) {
            x.step(1.0, 1.0);
            min = min.min(x.xenon);
        }
        assert!(min < 0.98 * start, "expected a transient dip, min {min} start {start}");
        for _ in 0..(72 * 3600) {
            x.step(1.0, 1.0);
        }
        let eq = Xenon::equilibrium(1.0).xenon;
        assert!((x.xenon - eq).abs() < 0.01, "should approach new equilibrium {eq}, got {}", x.xenon);
    }
```

- [ ] **Step 2: Run**

Run: `cargo test --release -p bwr-core --lib xenon 2>&1 | grep -E "^test |test result"`
Expected: 4 xenon tests `ok`. If `power_rise_from_50_to_100_percent_dips_xenon_first` fails, the model's burnout term is wrong; do not change the test threshold, investigate `Xenon::step`.

- [ ] **Step 3: Commit**

```bash
git add crates/bwr-core/src/reactor.rs
git commit -m "test(core): xenon stationarity, restart burnout and power-rise dip

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 8: Reactivity coefficients as data and `full_power_with`

**Files:**
- Modify: `crates/bwr-core/src/reactor.rs` (`ReactivityCoeffs`, `Core.coeffs`, `update_reactivity`)
- Modify: `crates/bwr-core/src/sim.rs` (`full_power_with`)
- Modify: `crates/bwr-core/tests/core_transients.rs`

**Interfaces:**
- Produces: `ReactivityCoeffs { void, doppler, mod_t, xenon_worth }` (Copy, Default), `ReactivityCoeffs::feedback(&self, fuel_t, void, mod_t) -> (f64, f64, f64)`, `Core.coeffs`, `Sim::full_power_with(coeffs: ReactivityCoeffs) -> Sim`. `Core::feedback` (associated fn) and `Xenon::reactivity` are removed.

- [ ] **Step 1: Write the failing tests**

Append to `crates/bwr-core/tests/core_transients.rs`:

```rust
use bwr_core::reactor::ReactivityCoeffs;

#[test]
fn full_power_rod_insertion_fraction() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 60.0);
    let inserted = -sim.core.rho_rods / ROD_WORTH_TOTAL;
    println!("critical rod insertion fraction at HFP, equilibrium xenon: {:.3}", inserted);
    assert!(inserted > 0.05 && inserted < 0.25, "insertion fraction {inserted} outside the 5-25 % plausibility band (core.md section 3)");
}

#[test]
fn default_coeffs_equal_params_and_feedback_matches() {
    let c = ReactivityCoeffs::default();
    assert_eq!((c.void, c.doppler, c.mod_t, c.xenon_worth), (VOID_COEF, DOPPLER_A, MOD_TEMP_COEF, XENON_WORTH_EQ));
    let (d, v, m) = c.feedback(550.0, 0.4, 280.0);
    assert!((d - (-DOPPLER_A * ((550.0f64 + 273.15).sqrt() - 293.15f64.sqrt()))).abs() < 1e-12);
    assert!((v - VOID_COEF * 0.4).abs() < 1e-12);
    assert!((m - MOD_TEMP_COEF * 260.0).abs() < 1e-12);
}

#[test]
fn full_power_with_stronger_void_coefficient_is_critical_with_fewer_rods_in() {
    let base = Sim::full_power_with(ReactivityCoeffs::default());
    let strong = Sim::full_power_with(ReactivityCoeffs { void: VOID_COEF * 1.25, ..ReactivityCoeffs::default() });
    assert!((strong.core.kin.n - 1.0).abs() < 0.05, "power {}", strong.core.kin.n);
    assert!(-strong.core.rho_rods < -base.core.rho_rods, "more negative void feedback needs less rod worth: {} vs {}", strong.core.rho_rods, base.core.rho_rods);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --release -p bwr-core --test core_transients coeffs 2>&1 | head -10`
Expected: compile error `cannot find type ReactivityCoeffs`.

- [ ] **Step 3: Implement `ReactivityCoeffs`**

In `reactor.rs`, before `pub struct Core`, add:

```rust
/// Feedback coefficients. Defaults are the calibrated `params.rs` values; a
/// sensitivity study builds a plant with modified values via `Sim::full_power_with`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReactivityCoeffs {
    /// dk/k per unit core-average void fraction.
    pub void: f64,
    /// Doppler: rho = -doppler * (sqrt(Tf[K]) - sqrt(293.15)).
    pub doppler: f64,
    /// dk/k per K of moderator temperature above 20 degC.
    pub mod_t: f64,
    /// dk/k of equilibrium xenon at 100 % power.
    pub xenon_worth: f64,
}

impl Default for ReactivityCoeffs {
    fn default() -> Self {
        Self { void: VOID_COEF, doppler: DOPPLER_A, mod_t: MOD_TEMP_COEF, xenon_worth: XENON_WORTH_EQ }
    }
}

impl ReactivityCoeffs {
    /// (doppler, void, moderator) reactivity for the given state.
    pub fn feedback(&self, fuel_t: f64, void: f64, mod_t: f64) -> (f64, f64, f64) {
        let doppler = -self.doppler * ((fuel_t + 273.15).sqrt() - 293.15f64.sqrt());
        (doppler, self.void * void, self.mod_t * (mod_t - 20.0))
    }
}
```

Add `pub coeffs: ReactivityCoeffs,` to `Core` (after `clad_t`) and `coeffs: ReactivityCoeffs::default(),` in `Core::new`. Delete the associated function `Core::feedback`. Replace `update_reactivity`:

```rust
    pub fn update_reactivity(&mut self, void: f64, mod_t: f64) {
        let (d, v, m) = self.coeffs.feedback(self.fuel_t, void, mod_t);
        self.rho_rods = self.rods.reactivity();
        self.rho_doppler = d;
        self.rho_void = v;
        self.rho_mod = m;
        self.rho_xenon = -self.coeffs.xenon_worth * self.xenon.xenon;
        self.rho_total = RHO_EXCESS_COLD + self.rho_rods + d + v + m + self.rho_xenon;
    }
```

Delete `Xenon::reactivity` (its only caller was `update_reactivity`). Run `grep -rn "Core::feedback\|\.reactivity()" crates/bwr-core/src` and fix any other caller to use `coeffs.feedback` (expected: only `rods.reactivity()` remains).

- [ ] **Step 4: `full_power_with` in `sim.rs`**

Rename the existing `pub fn full_power() -> Self` to `pub fn full_power_with(coeffs: ReactivityCoeffs) -> Self`, and right after `let core = Core::new(1.0, ROD_FULL_OUT, t + 264.0);` change it to

```rust
        let mut core = Core::new(1.0, ROD_FULL_OUT, t + 264.0);
        core.coeffs = coeffs;
```

Then add:

```rust
    /// Rated power with equilibrium xenon, generator on line, default coefficients.
    pub fn full_power() -> Self {
        Self::full_power_with(ReactivityCoeffs::default())
    }
```

Add `ReactivityCoeffs` to the `crate::reactor` import in `sim.rs`.

- [ ] **Step 5: Run tests**

Run: `cargo test --release -p bwr-core 2>&1 | grep -E "^test |test result"`
Expected: all `ok`; the `full_power_rod_insertion_fraction` test prints the fraction (run with `-- --nocapture` to see it) and it sits inside 5-25 %.

- [ ] **Step 6: Commit**

```bash
git add crates/bwr-core/src/reactor.rs crates/bwr-core/src/sim.rs crates/bwr-core/tests/core_transients.rs
git commit -m "refactor(core): reactivity coefficients as ReactivityCoeffs data with full_power_with

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 9: Shutdown margin evidence

**Files:**
- Modify: `crates/bwr-core/src/reactor.rs` (`Rods::shutdown_margin`, `Rods::strongest_rod`)
- Modify: `crates/bwr-core/tests/core_transients.rs`

**Interfaces:**
- Produces: `Rods::shutdown_margin(&self, rho_other: f64, withdrawn: &[usize]) -> f64`, `Rods::strongest_rod(&self) -> usize`.
- Consumes: `docs/model/parameters.csv` row `SDM_REQUIRED` (Task 1).

- [ ] **Step 1: Write the failing tests**

Append to `crates/bwr-core/tests/core_transients.rs`:

```rust
/// `SDM_REQUIRED` from parameters.csv if its status is `confirmed`; None otherwise.
fn sdm_required() -> Option<f64> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/model/parameters.csv");
    let text = std::fs::read_to_string(path).ok()?;
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next()?.split(',').map(|s| s.trim()).collect();
    let name_i = header.iter().position(|h| *h == "name")?;
    let value_i = header.iter().position(|h| *h == "value")?;
    let status_i = header.iter().position(|h| *h == "status")?;
    for line in lines {
        let cols: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
        if cols.get(name_i) == Some(&"SDM_REQUIRED") && cols.get(status_i) == Some(&"confirmed") {
            return cols.get(value_i)?.parse().ok();
        }
    }
    None
}

fn assert_sdm(label: &str, sdm: f64) {
    println!("SDM {label}: {:.4} dk/k ({:.0} pcm)", sdm, sdm * 1e5);
    match sdm_required() {
        Some(req) => assert!(sdm >= req, "{label}: SDM {sdm} below sourced requirement {req}"),
        None => assert!(sdm > 0.0, "{label}: SDM {sdm} not positive (SDM_REQUIRED unavailable)"),
    }
}

#[test]
fn shutdown_margin_cold_xenon_free() {
    let rods = Rods::new(0.0);
    let (d, v, m) = ReactivityCoeffs::default().feedback(20.0, 0.0, 20.0);
    let rho_other = d + v + m; // xenon-free: no xenon term
    assert!(rho_other.abs() < 1e-12, "cold clean feedback should be zero, got {rho_other}");
    assert_sdm("all rods in", rods.shutdown_margin(rho_other, &[]));
    let strongest = rods.strongest_rod();
    assert_sdm("strongest rod out", rods.shutdown_margin(rho_other, &[strongest]));
    let mut order: Vec<usize> = (0..N_RODS).collect();
    order.sort_by(|&a, &b| rods.weights[b].partial_cmp(&rods.weights[a]).unwrap());
    assert_eq!(order[0], strongest);
    assert_sdm("strongest pair out", rods.shutdown_margin(rho_other, &[order[0], order[1]]));
    // Consistency with the reactivity model: all-in SDM equals rod worth minus cold excess.
    assert!((rods.shutdown_margin(0.0, &[]) - (ROD_WORTH_TOTAL - RHO_EXCESS_COLD)).abs() < 1e-12);
}

#[test]
fn shutdown_margin_hot_with_equilibrium_xenon_is_larger_than_cold() {
    use bwr_core::reactor::Xenon;
    let rods = Rods::new(0.0);
    let c = ReactivityCoeffs::default();
    let (d, v, m) = c.feedback(287.7, 0.0, 287.7);
    let rho_hot = d + v + m - c.xenon_worth * Xenon::equilibrium(1.0).xenon;
    let cold = rods.shutdown_margin(0.0, &[]);
    let hot = rods.shutdown_margin(rho_hot, &[]);
    assert!(hot > cold, "hot {hot} should exceed cold {cold}: cold xenon-free is limiting");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --release -p bwr-core --test core_transients shutdown_margin 2>&1 | head -10`
Expected: compile error `no method named shutdown_margin`.

- [ ] **Step 3: Implement**

In `impl Rods` (reactor.rs), add:

```rust
    /// Index of the rod with the largest relative worth.
    pub fn strongest_rod(&self) -> usize {
        (0..self.weights.len())
            .max_by(|&a, &b| self.weights[a].partial_cmp(&self.weights[b]).unwrap())
            .expect("rods")
    }

    /// Shutdown margin, dk/k, with every rod fully inserted except those in
    /// `withdrawn` (fully out), for feedback reactivity `rho_other`
    /// (Doppler + void + moderator + xenon at the condition of interest).
    /// Positive means subcritical by that amount.
    pub fn shutdown_margin(&self, rho_other: f64, withdrawn: &[usize]) -> f64 {
        let inserted: f64 = (0..self.pos.len())
            .filter(|i| !withdrawn.contains(i))
            .map(|i| self.weights[i] * s_curve(1.0))
            .sum();
        let rho_rods = -ROD_WORTH_TOTAL * inserted;
        -(RHO_EXCESS_COLD + rho_other + rho_rods)
    }
```

- [ ] **Step 4: Run tests**

Run: `cargo test --release -p bwr-core --test core_transients shutdown_margin -- --nocapture 2>&1 | grep -E "SDM|^test |test result"`
Expected: three `SDM ...` lines printed (all-in about 0.07 dk/k, strongest-rod-out and pair-out smaller but positive), both tests `ok`.

- [ ] **Step 5: Commit**

```bash
git add crates/bwr-core/src/reactor.rs crates/bwr-core/tests/core_transients.rs
git commit -m "feat(core): shutdown margin evidence for all-in, strongest-rod-out and pair-out

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 10: Coefficient sensitivity study example

**Files:**
- Create: `crates/bwr-core/examples/sensitivity.rs`

**Interfaces:**
- Consumes: `Sim::full_power_with`, `ReactivityCoeffs`, `Command::TripTurbine`.

- [ ] **Step 1: Write the example**

```rust
//! Sensitivity of the full-power turbine-trip transient to reactivity coefficients.
//! Usage: cargo run --release -p bwr-core --example sensitivity
//! Mirrors the Package 2 turbine-trip validation case: trip at t = 5 s, observe 120 s.

use bwr_core::api::Command;
use bwr_core::reactor::ReactivityCoeffs;
use bwr_core::sim::DT;
use bwr_core::Sim;

struct Outcome {
    peak_power: f64,
    peak_power_t: f64,
    peak_pressure_mpa: f64,
    min_level_cm: f64,
    scrammed: bool,
    final_power: f64,
}

fn turbine_trip(coeffs: ReactivityCoeffs) -> Outcome {
    let mut sim = Sim::full_power_with(coeffs);
    for _ in 0..(5.0 / DT) as usize {
        sim.step(DT);
    }
    sim.apply(Command::TripTurbine);
    let mut o = Outcome { peak_power: 0.0, peak_power_t: 0.0, peak_pressure_mpa: 0.0, min_level_cm: f64::MAX, scrammed: false, final_power: 0.0 };
    for _ in 0..(120.0 / DT) as usize {
        sim.step(DT);
        if sim.core.kin.n > o.peak_power {
            o.peak_power = sim.core.kin.n;
            o.peak_power_t = sim.time;
        }
        o.peak_pressure_mpa = o.peak_pressure_mpa.max(sim.vessel.eos.p / 1e6);
        o.min_level_cm = o.min_level_cm.min(sim.vessel.level_nr_cm());
    }
    o.scrammed = sim.scrammed;
    o.final_power = sim.core.kin.n;
    o
}

fn main() {
    let b = ReactivityCoeffs::default();
    let variants: Vec<(&str, ReactivityCoeffs)> = vec![
        ("nominal", b),
        ("void -25 %", ReactivityCoeffs { void: b.void * 0.75, ..b }),
        ("void +25 %", ReactivityCoeffs { void: b.void * 1.25, ..b }),
        ("doppler -25 %", ReactivityCoeffs { doppler: b.doppler * 0.75, ..b }),
        ("doppler +25 %", ReactivityCoeffs { doppler: b.doppler * 1.25, ..b }),
    ];
    println!("{:<14} {:>8} {:>8} {:>9} {:>9} {:>6} {:>8}", "variant", "peak n", "t_peak", "peak MPa", "min lvl", "scram", "final n");
    for (name, c) in variants {
        let o = turbine_trip(c);
        println!(
            "{:<14} {:8.3} {:8.2} {:9.3} {:9.1} {:>6} {:8.3}",
            name, o.peak_power, o.peak_power_t, o.peak_pressure_mpa, o.min_level_cm, o.scrammed, o.final_power
        );
    }
    println!("Coefficients are calibrated/assumed (docs/model/core.md section 3); this table is a sensitivity, not plant data.");
}
```

- [ ] **Step 2: Run**

Run: `cargo run --release -p bwr-core --example sensitivity`
Expected: a 5-row table; `nominal` row shows `scram false` and peak pressure below 7.4 MPa (consistent with `plant::turbine_trip_rides_through_on_bypass`). Copy the table into the commit body.

- [ ] **Step 3: Commit**

```bash
git add crates/bwr-core/examples/sensitivity.rs
git commit -m "feat(core): reactivity coefficient sensitivity example for the turbine-trip transient

<paste the table here>

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 11: Baseline regeneration, register closure, and final verification

**Files:**
- Modify: `docs/validation/baseline/*.csv`, `docs/model/parameters.csv`, `docs/model/coverage.md`, `docs/model/core.md`

**Interfaces:**
- Consumes: P2's `examples/validation.rs` and its documented update mode (see `docs/validation/method.md`).

- [ ] **Step 1: Compare before and after**

Run: `cargo run --release -p bwr-core --example validation 2>&1 | tail -40`
Expected: comparison violations only in metrics explained by this package (fuel temperature columns, early-second power after scram due to the run-in and rod-order change, decay-heat levels after the refit). Record each violated metric, its baseline value, and its new value in a table.

- [ ] **Step 2: Regenerate baselines**

Run the update mode documented in `docs/validation/method.md` (P2 defines it as `cargo run --release -p bwr-core --example validation -- --update`; use method.md's flag if it differs).
Expected: baselines rewritten; a second plain run reports zero violations.

- [ ] **Step 3: Close the register**

Verify every constant touched in this package has a `parameters.csv` row: run

```bash
for n in GAP_RESISTANCE_K_PER_W CLAD_COOLANT_RESISTANCE_K_PER_W CLAD_HEAT_CAP_J_PER_K FUEL_MODEL_T_MAX_C UNCOVERED_ENVELOPE_S IRRADIATION_HISTORY_S SDM_REQUIRED DECAY_GROUPS; do grep -q "^$n," docs/model/parameters.csv && echo "ok $n" || echo "MISSING $n"; done
```

Expected: eight `ok` lines. Add a `DECAY_GROUPS` row (status `calibrated`, note "fit to decay-heat-reference.csv, max rel err <value from the tool>") if missing.

In `docs/model/coverage.md`, update the core/fuel/rods row: evidence = core.md; tests = `core_transients.rs` (list the test names) and the reactor unit tests; limitations = "no axial/channel model, no exposure dependence, no PCT/CPR, rod speeds assumed, SDM requirement unavailable unless sourced".

Append to `docs/model/core.md` a "Measured results (Package 5)" section with: the rod insertion fraction printed by `full_power_rod_insertion_fraction`, the three SDM values, the sensitivity table, and the decay-fit maximum relative error.

- [ ] **Step 4: Full verification**

Run:

```bash
cargo test --release -p bwr-core 2>&1 | grep -E "test result"
npm --prefix ui run typecheck
npm --prefix ui run build 2>&1 | tail -2
grep -rn "core_damage\|CORE DAMAGE\|FUEL_RESISTANCE\|fast_insert\|rods\.target\b" crates ui/src README.md; echo "grep exit=$?"
```

Expected: every `test result:` line reads `ok`; typecheck silent; build `✓ built`; the grep prints nothing and `grep exit=1`.

- [ ] **Step 5: Commit**

```bash
git add docs/validation/baseline docs/model/parameters.csv docs/model/coverage.md docs/model/core.md
git commit -m "chore(validation): regenerate baselines after Package 5 core refinements

Before/after metric table:
<paste the table from Step 1>

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

## Spec coverage check (Package 5 bullets → tasks)

| Spec bullet | Task |
|---|---|
| Verify fuel and rod counts, active length, arrangement, travel convention, speeds, worth, SDM criteria | 1 |
| Preserve point kinetics initially | 1 (statement), no code change |
| Replace undocumented reactivity tuning with traceable relationships and sensitivity studies; exposure via explicit reference cases if supported | 1, 8, 10 |
| Verify iodine/xenon equilibrium, shutdown peak, restart, power-history dependence | 7 |
| Reassess decay heat against an identified public reference, finite irradiation history, uncertainty; no "exact standards" claim | 6 |
| Separate fuel and cladding; replace the 1200 C rule; no PCT/CPR claims | 4, 5 |
| Hydraulic scram, alternate initiation, motor run-in as distinct mechanisms | 2, 3 |
| All-rods-in, strongest-rod, paired-rod, cold xenon-free SDM; sourced margin targets | 9 |
| Stuck-rod fault honoured (master `Fault::RodStuck`) | 3 |
| Register, assumptions, applicability, baselines | 1, 11 |

**Acceptance gate (verbatim):** Supported reactivity and shutdown behavior has reproducible evidence. Uncertain rod worth, feedback, and fuel limits cannot silently appear as confirmed plant performance.
