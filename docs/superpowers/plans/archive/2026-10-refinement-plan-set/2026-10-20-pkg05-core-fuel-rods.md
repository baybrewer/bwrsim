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
| `docs/model/decay-heat-reference.csv` | Create (only if the reference is retrieved, Task 6) | Transcribed public decay-heat table (`t_s,p_over_p0,source_id,table,page`); immutable after transcription check |
| `docs/validation/findings.md` | Modify (P3 creates) | F-nnn entries: decay-heat reference retrieval outcome; fit-gate outcome; SDM pair criterion |
| `docs/validation/reports/<date>-pkg05.md` | Create | Complete compare-mode output and metric classification before baseline regeneration (master protocol) |
| `docs/model/parameters.csv` | Modify | Rows for every constant touched here, each in the task that touches the constant |
| `docs/model/assumptions.md` | Modify | Table rows A-050 .. A-053 (P1 schema); A-001, A-003, A-004, A-005 are reused, not duplicated |
| `docs/model/esbwr-applicability.csv` | Modify | E-01 closed from `pending`; new rows for DCD Ch. 4.6 and Ch. 15 borrowings |
| `tools/fit_decay_heat.py` | Rewrite | Fit `DECAY_GROUPS` to the reference CSV; report residuals; exit 1 above `--max-rel-err` |
| `crates/bwr-core/src/params.rs` | Modify | New constants; remove `FUEL_RESISTANCE` |
| `crates/bwr-core/src/reactor.rs` | Modify | `RodMotion`, `Rods` restructure, `ReactivityCoeffs`, `clad_t`, two-node update, `after_history`, SDM |
| `crates/bwr-core/src/sim.rs` | Modify | Call-site migration, stuck-fault mirroring, envelope detection, `full_power_with`, alarms |
| `crates/bwr-core/src/api.rs` | Modify | `clad_temp_c`, `fuel_envelope_exceeded`; remove the legacy damage-verdict field |
| `crates/bwr-core/tests/core_transients.rs` | Create | Package tests |
| `crates/bwr-core/examples/sensitivity.rs` | Create | Coefficient sensitivity study |
| `ui/src/sim/types.ts`, `ui/src/sim/mock.ts` | Modify | Contract mirror |
| `README.md` | Modify | Decay-heat and core-damage wording |

Names from the master (r2) that this package owns: `RodMotion { Idle, Motor { target }, Scram, Sri, MotorRunIn }`, `Rods { motion, stuck }` and its method set, `ReactivityCoeffs { void, doppler, mod_t, xenon_worth }`, `Core { clad_t, coeffs }`, `DecayHeat::after_history`, `Sim::full_power_with`, `GAP_RESISTANCE_K_PER_W`, `CLAD_COOLANT_RESISTANCE_K_PER_W`, `CLAD_HEAT_CAP_J_PER_K`, `FUEL_MODEL_T_MAX_C`, `UNCOVERED_ENVELOPE_S`, `IRRADIATION_HISTORY_S`. Names introduced beyond the master: `Rods::last_error` (the master dt rule's `last_error` field for the void-returning `Rods::step`; forwarded into `Core.last_error` by `Core::step_rods` and drained by `Sim::step` at step 17), `Rods::refresh_stuck(&[bool])`, `Sim::uncovered_s`, `Sim::clad_t_step_max`, `Sim::fuel_envelope_exceeded()`, `CORE_COVERED_FRAC`, `Q_FUEL_RATED`, `DECAY_FIT_MAX_REL_ERR` (Task 6, retrieved branch only), `examples/sensitivity.rs`. No production-only test hooks are added; tests build low-inventory states through `Vessel::new`.

Two semantics this plan fixes that the master leaves open (listed again under "Master amendments" at the end): `Rods::scram_hydraulic` and `Rods::motor_run_in` command only rods with `pos > 0.0` (a rod already at 0 stays `Idle`, so a latched scram can be re-asserted every step without `scramming()` sticking true), and a rod that becomes stuck while any mechanism owns it is parked `Idle` by `refresh_stuck` (its outstanding command is dropped; TGT = POS).

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
| Active fuel length | 3.81 m | assumed | typical GNF2; A-001 |
| Rod position convention | 0 = full in, 48 = full out, 2 positions per notch | assumed | legacy BWR convention; A-001 |
| Rod radial layout | 57 cells within radius sqrt(17) on a 9x9 grid | assumed | A-001; not the plant map |
| Rod relative worth | J0(2.405 r/R)^2 radial weighting, cosine-integral axial S-curve | assumed | A-003 |

## 2. Rod motion mechanisms

Rod motion is a per-rod `RodMotion` state (reactor.rs):

| Mechanism | State | Speed | Initiated by |
|---|---|---|---|
| Normal motor drive | `Motor { target }` | `ROD_SPEED` = 0.4 positions/s (assumed, A-001) | operator `RodSetTarget` |
| Hydraulic scram | `Scram` | `ROD_FULL_OUT / SCRAM_TIME` = 48 / 3 s (assumed, A-001) | RPS scram (`Sim::scram`) |
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
concept adapted from ESBWR (E-10, classification `adapted-model`; DCD Rev. 10 Tier 2 Ch. 4.6, see
esbwr-applicability.csv). BWRX-specific stroke times are not public; the
values above are assumptions with the stated sensitivity (A-001).

Scram reset requires every `Scram` and `MotorRunIn` rod to be at position 0.

**Stuck-rod policy** (`Fault::RodStuck`, mirrored into `rods.stuck` at step 2 of
the plant step, after the rods have moved at step 1):

| Fault arrives while | Result at the refresh | TGT shown |
|---|---|---|
| `Idle` | stays `Idle` | POS |
| `Motor { target }` | parked `Idle`; the motor command is dropped | POS |
| `Scram`, `Sri` or `MotorRunIn` | parked `Idle`; the insertion command is dropped for that rod, so `scramming()` and the reset rule ignore it | POS |

A stuck rod is never commanded: `set_motor_target`, `scram_hydraulic`,
`select_rod_insert` and `motor_run_in` skip rods whose `stuck` flag is set.
A command issued at the same boundary as the fault (before the next step's
refresh) is accepted and moves the rod once; the refresh then parks it.

When the fault clears, the rod stays `Idle` at its position; nothing is
replayed automatically. The one exception is the latched RPS demand: while
`Sim.scrammed` is true, the plant step re-asserts `scram_hydraulic()` and
`motor_run_in()` after the refresh, so a freed rod resumes hydraulic insertion
from the next step. (Both methods command only rods with `pos > 0`, so rods
already in stay `Idle` and `scramming()` clears when insertion completes.)
An SRI demand is not latched and is not replayed. Scram reset is therefore
never blocked by a stuck rod: it remains out, and the operator sees
TGT = POS = its stuck position while the fault is active.

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
(reactivity control, S-08). It is recorded in parameters.csv as `SDM_REQUIRED`
with status `confirmed` only if the public document states the number;
otherwise the row has status `unavailable` (interval none/none: a margin
requirement has no defensible interval) and the tests assert SDM > 0 only.

**Applicability of the threshold.** A BWR shutdown-margin requirement governs
the cold, xenon-free core with the highest-worth control rod fully withdrawn
(the standard technical-specification definition). `SDM_REQUIRED`, when
confirmed, is therefore applied by the tests to the strongest-rod-out case
only. The all-rods-in value and any two-rod value are reported with a
positivity check, not against the threshold.

**Paired-rod case.** Which rods form a drive pair (shared hydraulic control
unit or shared scram path) in the BWRX-300 FMCRD arrangement is not public, so
the "applicable paired-rod unavailable" plant case is recorded as
`unavailable` (findings.md, Task 9) and is **not** computed. What the tests do
report is the generic worst-two-worth sensitivity: the two rods with the
largest relative weights on the assumed grid, labelled "worst two rods
(model sensitivity)". That number is evidence about the calibrated model's
worth distribution, not the plant paired-drive case, and no threshold is
applied to it.

Reported values are evidence of the calibrated model, not plant performance.
Every reported configuration is also rebuilt independently as explicit rod
positions and its reactivity compared with `shutdown_margin` (Task 9).

## 5. Fuel thermal model and envelope

Two lumped nodes: average fuel (`fuel_t`) and average cladding (`clad_t`).

- Pellet-plus-gap resistance `GAP_RESISTANCE_K_PER_W` = 252 K / (870e6 W x 0.975), assumed (A-050): average fuel ~552 degC at rated.
- Clad-to-coolant resistance `CLAD_COOLANT_RESISTANCE_K_PER_W` = 12 K / (870e6 W x 0.975), assumed (A-050): clad ~12 K above coolant at rated. Scaled by 1/htc_factor when the core uncovers.
- `CLAD_HEAT_CAP_J_PER_K` = 3.1e6 J/K (about 9.5 t Zircaloy x 330 J/kg K), assumed (A-051).
- Sum of the two resistances equals the former single-node `FUEL_RESISTANCE` (264 K at rated), so rated fuel temperature is unchanged.

**Envelope.** The model is valid for covered, forced-free natural circulation
cooling with cladding below `FUEL_MODEL_T_MAX_C` = 1000 degC. When the
cladding temperature exceeds `FUEL_MODEL_T_MAX_C`, or the core is uncovered
for more than `UNCOVERED_ENVELOPE_S` = 60 s, `DomainError::FuelModelEnvelope`
is raised once (first occurrence, P3 semantics), `Snapshot.fuelEnvelopeExceeded`
becomes true, and alarm `fuel_envelope` ("FUEL THERMAL MODEL ENVELOPE
EXCEEDED") is shown. This is **not** a damage limit. **Peak cladding
temperature and critical power ratio are not computed**; the model has no
hot-channel, no CPR correlation, and no post-dryout heat transfer. The former
1200 degC average-fuel damage verdict is removed because it presented an
unsupported conclusion.

**Detection semantics.** The cladding check is evaluated on the plant-step
endpoints: `Sim.clad_t_step_max = max(clad_t at step entry, clad_t at step
exit)` is compared with the limit in `protection()`. An entry-state excursion
therefore latches even if the fast cladding node cools below the limit within
the same step; an excursion that begins and ends strictly inside one 0.05 s
step is below the model's own resolution and is not claimed. The uncovered
timer counts continuous plant time with `core_coverage() < CORE_COVERED_FRAC`
(0.999) and resets to zero on any covered step; the limit is strict (`>`).

## 6. Decay heat

Exponential groups fitted by `tools/fit_decay_heat.py`. Task 6 is a bounded
evidence task first: it records in docs/validation/findings.md which public
reference table (if any) could be retrieved and read, with the table's
isotope/fission mix, irradiation history, normalisation, included
contributions and stated uncertainty. Outcome `retrieved`: the groups are
fitted to the transcribed `docs/model/decay-heat-reference.csv` (immutable
after its transcription check; the `source_id`, `table` and `page` columns
point into sources.csv). Outcome `unavailable`: the existing nine-group fit
to the 0.066 t^-0.2 approximation (S-STD-04) is kept and the `DECAY_GROUPS`
row says so. In neither case is the fit a standards implementation. The fit
gate is 5 % maximum relative error at the reference points; the predeclared
failure path (more groups, then `calibrated` status with the measured error
and a findings entry) is in Task 6, and the source values are never edited to
pass. This paragraph is completed by Task 6 with the outcome, the fitted
error and the group count.

Finite irradiation history: `DecayHeat::after_history(n, irr_s, cooled_s)`
uses `d_i = f_i * n * (1 - exp(-lambda_i * irr_s)) * exp(-lambda_i * cooled_s)`.
Scenarios use `IRRADIATION_HISTORY_S` = 6.31e7 s (2 years at rated, assumed,
A-052). Uncertainty: a +/-10-20 % band is typical for total decay heat without
actinide and exposure detail; this band is an assumption (A-053), not a
standards claim.

## 7. Xenon / iodine

Normalised two-nuclide model (reactor.rs `Xenon`). Verified behaviours
(`core_transients.rs`): stationary equilibrium at 30/60/100 % power, post-
shutdown peak between 6 h and 14 h, burnout on restart from the peak, and the
transient dip after a 50 % to 100 % power rise. Cross-section-times-flux
`SIGMA_PHI_XE` is assumed (A-004).
```

- [ ] **Step 2: Append the documentation-owned parameter row**

Master register-row rule: a constant's row is added or updated in the task that adds or changes the constant. Task 1 therefore adds only the row that no code task owns, `SDM_REQUIRED` (a sourced requirement, not a `params.rs` constant). The code-owned rows are added in Task 4 (`Q_FUEL_RATED`, `GAP_RESISTANCE_K_PER_W`, `CLAD_COOLANT_RESISTANCE_K_PER_W`, `CLAD_HEAT_CAP_J_PER_K`; `FUEL_RESISTANCE` retired there), Task 5 (`FUEL_MODEL_T_MAX_C`, `UNCOVERED_ENVELOPE_S`, `CORE_COVERED_FRAC`) and Task 6 (`IRRADIATION_HISTORY_S`, `DECAY_GROUPS` update, `DECAY_FIT_MAX_REL_ERR`). P1's existing rows for `ROD_SPEED`, `SCRAM_TIME`, `ROD_WORTH_TOTAL`, `RHO_EXCESS_COLD`, `FUEL_RESISTANCE` and `DECAY_GROUPS` are left exactly as they are in this task (their constants do not change here; `FUEL_RESISTANCE` stays live and `derived` until Task 4 removes it).

Append to `docs/model/parameters.csv` (locked header `id,value,unit,definition,reference_state,source_design,status,source_ids,interval_lo,interval_hi,assumption_id,location,notes`; 13 fields; no field contains a comma). `every_row_has_valid_shape_and_status` requires `unavailable` rows to carry either two numeric bounds or `none`/`none`; a margin requirement has no defensible interval, so:

```
SDM_REQUIRED,,dk/k,Plant shutdown margin requirement (cold xenon-free; highest-worth rod fully withdrawn),cold xenon-free strongest rod out,BWRX-300,unavailable,,none,none,,docs/model/core.md section 4,no defensible interval for a requirement; set value and status confirmed with source_ids S-08 only if NEDO-33912-A Rev. 2 states the number; applies to the strongest-rod-out case only (core.md section 4)
```

If NEDO-33912-A Rev. 2 (S-08) is retrieved during this task and states the number, write the row with the value, status `confirmed`, `source_ids` `S-08`, and the P1 section rule satisfied (S-08's `section` field must not be `-`); record the page in `notes`. Never infer the value from another plant.

Verify shape and uniqueness, then run the P1 register gate:

```bash
cut -d, -f1 docs/model/parameters.csv | sort | uniq -d
cargo test --release -p bwr-core --test register
```
Expected: no duplicate ids; every `register` test `ok` (in particular `every_row_has_valid_shape_and_status` and `every_assumption_id_exists_in_assumptions_md`).

- [ ] **Step 3: Append assumption table rows**

P1's `assumption_ids()` recognises only table rows of the form `| A-nnn | ... |` (six columns: `Id | Title | Statement | Sensitivity treatment | Interval basis | Resolved by`); headings are invisible to it. P1 already holds the rod geometry and kinematics (A-001), reactivity coefficients and rod worth (A-003), xenon data (A-004) and the fuel/decay lumped data (A-005), so this package does not restate them; `core.md` cites those ids. Append the following rows to the table in `docs/model/assumptions.md` (ids A-050..A-053 are unused by P1-P8; if a lower package has taken one by the time this runs, take the next free id and update every reference in this plan consistently). The P1 interval rule: a numeric `interval_lo`/`interval_hi` on a row is allowed only when the `Interval basis` cell names the parameter and its basis.

```markdown
| A-050 | Fuel thermal resistances and envelope | Two lumped nodes (average fuel, average cladding). Pellet+gap rise 252 K and clad-coolant rise 12 K at rated (sum equals the former single-node 264 K, A-005). Model envelope: cladding below 1000 degC and core covered (or uncovered for at most 60 s); outside it DomainError::FuelModelEnvelope is raised and no further claim is made. PCT and CPR are not computed. | Envelope flag and alarm instead of a damage verdict; envelope limits are indications only. | GAP_RESISTANCE_K_PER_W 2.4e-7..3.6e-7 K/W and CLAD_COOLANT_RESISTANCE_K_PER_W 0.7e-8..2.8e-8 K/W: the 252 K and 12 K rises scaled by the 0.8..1.2 and 0.6..2.0 ranges typical of gap conductance and single-phase versus boiling clad-coolant coefficients; FUEL_MODEL_T_MAX_C, UNCOVERED_ENVELOPE_S and CORE_COVERED_FRAC none (model-envelope choices) | Package 5 |
| A-051 | Cladding heat capacity | 3.1e6 J/K (about 9.5 t Zircaloy x 330 J/kg K). Affects the sub-second cladding response only. | None; the energy identity and transient reference tests bound the numerical effect. | CLAD_HEAT_CAP_J_PER_K 2.5e6..4.0e6 J/K: Zircaloy mass 8..12 t at 300..340 J/kg K | Package 5 |
| A-052 | Irradiation history for scenarios | Two years at rated power (6.31e7 s) before each scenario's stated cooling time; no exposure or bundle-history detail. | Long-lived decay-heat groups change by under 10 % at 30 days across the interval. | IRRADIATION_HISTORY_S 3.0e7..1.3e8 s: one to four years at power | Package 5 |
| A-053 | Decay-heat uncertainty band | Total decay heat carries a +/-10..20 % band without actinide, exposure or bundle-history detail. Not an ANS-5.1 uncertainty statement. | Reported with every decay-heat figure; the fit error is reported separately (Task 6). | none (band, not a parameter) | Package 5 |
```

- [ ] **Step 4: Append applicability rows**

P1's `register.rs` accepts only the classification vocabulary `shared-concept`, `adapted-model`, `provisional-estimate`, `inapplicable`, `pending`, and P1 already carries `E-01` (Ch 4 core fuel and rods, classification `pending`, test `core_transients.rs (Package 5)`) whose closure is this package's job. Ids `E-07`..`E-09` are taken by other packages; use `E-10` and `E-11` (if a lower package has consumed them by the time this runs, take the next free ids and update the references in this plan). Cite retrieved source ids: `S-06` (005N9751), `S-08` (NEDO-33912-A), and the sources.csv id whose `section` covers DCD Tier 2 Ch 4 (`S-ESBWR-04` at the P1 baseline; confirm with `grep -n "^S-ESBWR" docs/model/sources.csv`). No ESBWR numeric value is borrowed by this package, so the `BORROWED` table in `tests/register.rs` is unchanged.

In `docs/model/esbwr-applicability.csv`, edit the existing `E-01` row in place: keep every field, set `test` to `core_transients.rs (Package 5): full_power_rod_insertion_fraction; shutdown_margin_cold_xenon_free; scram_hydraulic_three_seconds` and `classification` to `adapted-model` (mechanisms and methodology borrowed; every number re-derived or calibrated for the 240-bundle core). Then append (locked header `id,esbwr_source,esbwr_section,target_subsystem,bwrx_evidence,differences,adaptation,applicability_limits,test,classification`; no field contains a comma):

```
E-10,DCD Rev 10 Tier 2;NUREG-1966 V2,S-ESBWR-04 Section 4.6 (FMCRD: hydraulic scram plus electric motor run-in on the same drive),Rod mechanisms,S-06 (57 FMCRDs);S-08 (FMCRD arrangement and shutdown-margin methodology),57 drives vs 269; BWRX stroke times and pair topology not public,RodMotion::Scram and RodMotion::Sri (hydraulic) and RodMotion::MotorRunIn (electric) as distinct states with the assumed speeds of A-001,timing values assumed (A-001); no ESBWR stroke time is a BWRX value,core_transients.rs (Package 5): scram_hydraulic_three_seconds; motor_run_in_120_seconds; motor_run_in_never_replaces_a_hydraulic_scram,adapted-model
E-11,DCD Rev 10 Tier 2;NUREG-1966 V2,Ch 15 (turbine trip transient mechanism: void collapse then Doppler),Core transient verification,none numerical,bypass capacity and SRI group differ,used only to order the expected qualitative response; no numerical trace imported,qualitative only,examples/sensitivity.rs (Package 5),shared-concept
```

Run the register gate again before committing:

```bash
cargo test --release -p bwr-core --test register
```
Expected: all `ok` (no unknown classification, no unknown assumption id, no duplicate id).

- [ ] **Step 5: Commit (register gate must be green)**

```bash
cargo test --release -p bwr-core --test register
git add docs/model/core.md docs/model/parameters.csv docs/model/assumptions.md docs/model/esbwr-applicability.csv
git commit -m "docs(core): Package 5 core model specification, SDM requirement row, assumptions

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 2: Rod motion state machine

**Files:**
- Modify: `crates/bwr-core/src/reactor.rs` (replace `Rods`)
- Modify: `crates/bwr-core/src/sim.rs` (every `rods.target`, `rods.scramming`, `rods.fast_insert` use)
- Modify: `crates/bwr-core/src/validation.rs` (populate `rods_min_pos`, `rods_max_pos`)
- Modify: `docs/validation/method.md` (column provenance)
- Create: `crates/bwr-core/tests/core_transients.rs`

**Interfaces:**
- Consumes: `Core::step_rods(dt)` (P3) which calls `self.rods.step(dt)`; `validation::{Case, ScriptedCommand, Operator, run_case}` and `Trace::column` (P2).
- Produces (master r2): `RodMotion { Idle, Motor { target }, Scram, Sri, MotorRunIn }` deriving `Clone, Copy, Debug, PartialEq`; `Rods { layout, weights, pos, motion, stuck, selected, last_error }`; `Rods::{target, targets, set_motor_target, scram_hydraulic, motor_run_in, select_rod_insert, scramming, moving, stop, step, all_in, reactivity, sri_group, refresh_stuck}`.
- Error channel (master dt rule): `Rods::step` is a public void-returning step, so it validates `dt` before touching state and sets `Rods.last_error` on failure. `Core::step_rods` (P3) validates `dt` first itself, then calls `rods.step(dt)` and forwards `rods.last_error.take()` into `Core.last_error`, which `Sim::step` already drains at step 17 (P3). `set_motor_target` rejects a non-finite or out-of-range target the same way (`what: "target"`).

Behaviour locked by the master: `Scram` and `Sri` move at the hydraulic speed but are distinct states; `scramming()` is true only for `Scram`; `scram_hydraulic` converts `Sri` rods to `Scram`; `select_rod_insert` leaves `Scram` rods alone; `motor_run_in` sets every non-stuck rod not already `Scram` to `MotorRunIn` (callers always invoke `scram_hydraulic` first, so `Sri` rods are already `Scram` by then); `Sri`, `Scram` and `MotorRunIn` rods return to `Idle` at 0.0; `stop()` affects `Motor` only; `target(i)` is the commanded endpoint. Fixed by this plan (core.md section 2): `scram_hydraulic` and `motor_run_in` command only rods with `pos > 0.0`; `refresh_stuck(mask)` parks every stuck rod `Idle` whatever mechanism owned it; `set_motor_target` ignores stuck rods.

- [ ] **Step 1: Write the failing tests**

Create `crates/bwr-core/tests/core_transients.rs`:

```rust
//! Package 5: core physics, fuel response and rod mechanism tests.

use bwr_core::api::{Command, ScenarioName};
use bwr_core::domain::DomainError;
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
    assert_eq!(r.targets()[3], 4.0, "idle rod targets its own position");
}

#[test]
fn scram_hydraulic_three_seconds() {
    let mut r = Rods::new(ROD_FULL_OUT);
    r.scram_hydraulic();
    assert!(r.scramming());
    assert!(r.motion.iter().all(|m| *m == RodMotion::Scram));
    assert!(r.targets().iter().all(|&t| t == 0.0), "targets show the commanded endpoint");
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
    assert!(r.targets().iter().all(|&t| t == 0.0));
    step_rods(&mut r, ROD_FULL_OUT / ROD_SPEED - 1.0);
    assert!(!r.all_in());
    step_rods(&mut r, 2.0);
    assert!(r.all_in());
    assert!(r.motion.iter().all(|m| *m == RodMotion::Idle));
}

#[test]
fn motor_run_in_never_replaces_a_hydraulic_scram() {
    let mut r = Rods::new(ROD_FULL_OUT);
    r.scram_hydraulic();
    r.motor_run_in();
    assert!(r.motion.iter().all(|m| *m == RodMotion::Scram));
    let mut at_zero = Rods::new(0.0);
    at_zero.motor_run_in();
    assert!(at_zero.motion.iter().all(|m| *m == RodMotion::Idle), "rods already in are not commanded");
}

#[test]
fn select_rod_insert_is_sri_on_the_group_only_and_is_not_a_scram() {
    let mut r = Rods::new(ROD_FULL_OUT);
    r.select_rod_insert();
    let g = r.sri_group();
    assert_eq!(g.len(), 4);
    for i in 0..N_RODS {
        let expect = if g.contains(&i) { RodMotion::Sri } else { RodMotion::Idle };
        assert_eq!(r.motion[i], expect, "rod {i}");
    }
    assert!(!r.scramming(), "SRI must not count as a scram");
    assert!(r.moving());
    for &i in &g {
        assert_eq!(r.target(i), 0.0);
    }
    step_rods(&mut r, SCRAM_TIME + DT);
    for &i in &g {
        assert_eq!(r.pos[i], 0.0);
        assert_eq!(r.motion[i], RodMotion::Idle, "SRI rods return to Idle at 0");
    }
}

#[test]
fn scram_upgrades_sri_rods_and_sri_never_downgrades_scram() {
    let mut r = Rods::new(ROD_FULL_OUT);
    r.select_rod_insert();
    r.scram_hydraulic();
    assert!(r.motion.iter().all(|m| *m == RodMotion::Scram), "Sri rods become Scram");
    let mut r2 = Rods::new(ROD_FULL_OUT);
    r2.scram_hydraulic();
    r2.select_rod_insert();
    assert!(r2.motion.iter().all(|m| *m == RodMotion::Scram), "SRI leaves Scram rods alone");
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
    r.select_rod_insert();
    assert_ne!(r.motion[7], RodMotion::Scram, "a stuck rod is never commanded by a scram");
    step_rods(&mut r, 5.0);
    assert_eq!(r.pos[7], ROD_FULL_OUT, "stuck rod moved under scram");
    assert!(r.pos.iter().enumerate().all(|(i, &p)| i == 7 || p == 0.0));
    assert!(!r.moving(), "a stuck rod must not count as moving");
}

#[test]
fn rods_step_rejects_invalid_dt_without_moving() {
    // Master dt rule for the void-returning Rods::step: last_error set, nothing moves.
    for dt in [f64::NAN, f64::INFINITY, 0.0, -0.05] {
        let mut r = Rods::new(ROD_FULL_OUT);
        r.scram_hydraulic();
        let (pos, motion) = (r.pos.clone(), r.motion.clone());
        r.step(dt);
        assert_eq!(r.pos, pos, "dt {dt}: rods moved");
        assert_eq!(r.motion, motion, "dt {dt}: motion changed");
        assert!(matches!(r.last_error.take(), Some(DomainError::PropertyOutOfRange { what, .. }) if what == "dt"), "dt {dt}");
    }
}

#[test]
fn motor_target_rejects_non_finite_and_out_of_range_targets() {
    let mut r = Rods::new(10.0);
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0, ROD_FULL_OUT + 1.0] {
        r.set_motor_target(2, bad);
        assert_eq!(r.motion[2], RodMotion::Idle, "target {bad} was accepted");
        assert!(matches!(r.last_error.take(), Some(DomainError::PropertyOutOfRange { what, .. }) if what == "target"), "target {bad}");
        step_rods(&mut r, 1.0);
        assert_eq!(r.pos[2], 10.0, "target {bad} moved the rod");
    }
    r.set_motor_target(2, ROD_FULL_OUT);
    assert_eq!(r.motion[2], RodMotion::Motor { target: ROD_FULL_OUT });
    assert!(r.last_error.is_none());
}

#[test]
fn motor_command_to_a_stuck_rod_is_ignored() {
    let mut r = Rods::new(10.0);
    r.stuck[5] = true;
    r.set_motor_target(5, 20.0);
    assert_eq!(r.motion[5], RodMotion::Idle);
    assert_eq!(r.target(5), 10.0, "TGT = POS for a stuck rod");
    assert!(r.last_error.is_none(), "a stuck rod is a mechanism state, not an argument error");
}

#[test]
fn refresh_stuck_parks_a_rod_in_every_motion_state_and_never_replays() {
    // core.md section 2 policy table: whatever owned the rod, a fault parks it Idle
    // with TGT = POS, and clearing the fault replays nothing at rod level.
    let mechanisms: [(&str, fn(&mut Rods, usize)); 4] = [
        ("motor", |r, i| r.set_motor_target(i, 0.0)),
        ("scram", |r, _| r.scram_hydraulic()),
        ("sri", |r, _| r.select_rod_insert()),
        ("run-in", |r, _| r.motor_run_in()),
    ];
    for (name, start) in mechanisms {
        let mut r = Rods::new(ROD_FULL_OUT);
        let i = r.sri_group()[0]; // in the SRI group, so every mechanism commands it
        start(&mut r, i);
        assert_ne!(r.motion[i], RodMotion::Idle, "{name}: rod not commanded");
        step_rods(&mut r, 1.0);
        let moved_to = r.pos[i];
        assert!(moved_to < ROD_FULL_OUT, "{name}: rod did not move before the fault");
        let mut mask = vec![false; N_RODS];
        mask[i] = true;
        r.refresh_stuck(&mask);
        assert!(r.stuck[i]);
        assert_eq!(r.motion[i], RodMotion::Idle, "{name}: a stuck rod must be parked");
        assert_eq!(r.target(i), moved_to, "{name}: TGT = POS for a stuck rod");
        step_rods(&mut r, 10.0);
        assert_eq!(r.pos[i], moved_to, "{name}: stuck rod moved");
        assert!(!r.scramming(), "{name}: a parked rod must not hold the scram indication once the others are in");
        r.refresh_stuck(&vec![false; N_RODS]);
        assert!(!r.stuck[i]);
        step_rods(&mut r, 1.0);
        assert_eq!((r.motion[i], r.pos[i]), (RodMotion::Idle, moved_to), "{name}: Rods never replays a dropped command (Sim re-asserts a latched scram, Task 3)");
    }
}

#[test]
fn scram_hydraulic_and_run_in_leave_rods_already_in_idle() {
    // Needed so Sim can re-assert a latched scram every step without scramming() sticking true.
    let mut r = Rods::new(0.0);
    r.scram_hydraulic();
    r.motor_run_in();
    assert!(r.motion.iter().all(|m| *m == RodMotion::Idle));
    assert!(!r.scramming());
}

#[test]
fn stop_parks_motor_motion_only() {
    let mut r = Rods::new(0.0);
    r.set_motor_target(0, 10.0);
    step_rods(&mut r, 2.0 / ROD_SPEED + 0.5); // about 2.2 positions
    r.stop();
    assert_eq!(r.motion[0], RodMotion::Motor { target: 4.0 });
    step_rods(&mut r, 10.0);
    assert!((r.pos[0] - 4.0).abs() < 1e-9);
    // Insertion mechanisms are not stoppable.
    let mut s = Rods::new(ROD_FULL_OUT);
    s.select_rod_insert();
    s.stop();
    assert!(s.motion.iter().any(|m| *m == RodMotion::Sri));
    s.scram_hydraulic();
    s.stop();
    assert!(s.scramming());
    let mut m = Rods::new(ROD_FULL_OUT);
    m.motor_run_in();
    m.stop();
    assert!(m.motion.iter().all(|x| *x == RodMotion::MotorRunIn));
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

#[test]
fn sri_at_plant_level_does_not_light_scram_and_a_scram_takes_over() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    sim.apply(Command::TripTurbine);
    run(&mut sim, 1.0);
    let g = sim.core.rods.sri_group();
    assert!(g.iter().all(|&i| sim.core.rods.motion[i] == RodMotion::Sri), "SRI should fire on turbine trip at power");
    assert!(!sim.scrammed && !sim.core.rods.scramming());
    assert!(!sim.snapshot().rods.scram_in_progress);
    sim.apply(Command::ManualScram);
    assert!(sim.core.rods.motion.iter().zip(&sim.core.rods.pos).all(|(m, &p)| *m == RodMotion::Scram || p == 0.0));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --release -p bwr-core --test core_transients > /tmp/p5-t2-red.log 2>&1; echo exit=$?; grep -n "cannot find" /tmp/p5-t2-red.log`
Expected: `exit=101`; the grep shows `cannot find type RodMotion in module bwr_core::reactor`.

- [ ] **Step 3: Replace `Rods` in `reactor.rs`**

Replace the whole `pub struct Rods { ... }` and `impl Rods { ... }` block (from `#[derive(Clone, Debug)] pub struct Rods` through the end of `pub fn stop`) with:

```rust
/// How a rod is being driven right now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RodMotion {
    Idle,
    /// Normal fine-motion drive toward `target` at `ROD_SPEED`.
    Motor { target: f64 },
    /// Hydraulic scram insertion at `ROD_FULL_OUT / SCRAM_TIME`, initiated by the RPS scram.
    Scram,
    /// Hydraulic insertion of the Select Rod Insert group (same speed as Scram,
    /// but not a scram: it does not light the scram indication).
    Sri,
    /// Electric motor run-in toward 0 at `ROD_SPEED` (FMCRD backup path, every scram).
    MotorRunIn,
}

impl RodMotion {
    fn is_hydraulic(self) -> bool {
        matches!(self, RodMotion::Scram | RodMotion::Sri)
    }
    fn is_insertion(self) -> bool {
        matches!(self, RodMotion::Scram | RodMotion::Sri | RodMotion::MotorRunIn)
    }
}

#[derive(Clone, Debug)]
pub struct Rods {
    pub layout: Vec<(usize, usize)>,
    pub weights: Vec<f64>,
    pub pos: Vec<f64>,
    pub motion: Vec<RodMotion>,
    /// Mirrored from `Fault::RodStuck` by `Sim` every step through `refresh_stuck`.
    /// A stuck rod is never commanded by any mechanism and never moves.
    pub stuck: Vec<bool>,
    pub selected: Option<usize>,
    /// Error raised by the void-returning `step` or by `set_motor_target` (master dt
    /// and non-finite rules); forwarded into `Core.last_error` by `Core::step_rods`.
    pub last_error: Option<DomainError>,
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
            last_error: None,
        }
    }

    /// Mirror the fault mask (`Sim`, step 2 of the plant step). A stuck rod is parked
    /// `Idle` whatever mechanism owned it, so its outstanding command is dropped and
    /// TGT = POS (core.md section 2). Clearing a fault changes nothing else here;
    /// `Sim` re-asserts a latched scram itself.
    pub fn refresh_stuck(&mut self, mask: &[bool]) {
        assert_eq!(mask.len(), self.pos.len(), "stuck mask length");
        for i in 0..self.pos.len() {
            if mask[i] {
                self.motion[i] = RodMotion::Idle;
            }
            self.stuck[i] = mask[i];
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

    /// Commanded endpoint: motor target, 0 for any insertion mechanism, own position when idle.
    pub fn target(&self, i: usize) -> f64 {
        match self.motion[i] {
            RodMotion::Motor { target } => target,
            RodMotion::Scram | RodMotion::Sri | RodMotion::MotorRunIn => 0.0,
            RodMotion::Idle => self.pos[i],
        }
    }

    pub fn targets(&self) -> Vec<f64> {
        (0..self.pos.len()).map(|i| self.target(i)).collect()
    }

    /// Command normal motor motion. A non-finite or out-of-range target is rejected
    /// and sets `last_error` (`Sim` has already dropped non-finite command values,
    /// so this is defence in depth). Ignored while an insertion mechanism owns the
    /// rod or the rod is stuck.
    pub fn set_motor_target(&mut self, i: usize, target: f64) {
        if !(target.is_finite() && (0.0..=ROD_FULL_OUT).contains(&target)) {
            self.last_error = Some(DomainError::PropertyOutOfRange { what: "target".into(), value: target });
            return;
        }
        if self.stuck[i] || self.motion[i].is_insertion() {
            return;
        }
        self.motion[i] = if (target - self.pos[i]).abs() < 1e-6 {
            RodMotion::Idle
        } else {
            RodMotion::Motor { target }
        };
    }

    /// RPS scram: every non-stuck rod with `pos > 0` -> Scram (Sri rods become Scram).
    /// Rods already at 0 stay Idle, so `Sim` can re-assert a latched scram each step.
    pub fn scram_hydraulic(&mut self) {
        for i in 0..self.pos.len() {
            if !self.stuck[i] && self.pos[i] > 0.0 {
                self.motion[i] = RodMotion::Scram;
            }
        }
    }

    /// Electric run-in: every non-stuck rod not already in Scram and not at 0 -> MotorRunIn.
    /// Callers invoke `scram_hydraulic` first, so this only matters for rods the
    /// hydraulic path did not take (none today; the ARI path in Package 8 reuses it).
    pub fn motor_run_in(&mut self) {
        for i in 0..self.pos.len() {
            if !self.stuck[i] && self.motion[i] != RodMotion::Scram && self.pos[i] > 0.0 {
                self.motion[i] = RodMotion::MotorRunIn;
            }
        }
    }

    /// Select Rod Insert: SRI group -> Sri, unless a rod is already in Scram or stuck.
    pub fn select_rod_insert(&mut self) {
        for i in self.sri_group() {
            if !self.stuck[i] && self.motion[i] != RodMotion::Scram {
                self.motion[i] = RodMotion::Sri;
            }
        }
    }

    /// True while any rod is in Scram. Sri does not count.
    pub fn scramming(&self) -> bool {
        self.motion.iter().any(|m| *m == RodMotion::Scram)
    }

    /// True while any non-stuck rod is being driven by any mechanism.
    pub fn moving(&self) -> bool {
        (0..self.pos.len()).any(|i| self.motion[i] != RodMotion::Idle && !self.stuck[i])
    }

    /// Stop motor motion at the nearest notch in the direction of travel.
    /// Scram, Sri and MotorRunIn cannot be stopped.
    pub fn stop(&mut self) {
        for i in 0..self.pos.len() {
            if let RodMotion::Motor { target } = self.motion[i] {
                let p = self.pos[i];
                let notch = if target > p { (p / 2.0).ceil() * 2.0 } else { (p / 2.0).floor() * 2.0 };
                self.motion[i] = if (notch - p).abs() < 1e-6 { RodMotion::Idle } else { RodMotion::Motor { target: notch } };
            }
        }
    }

    /// Void-returning public step (master dt rule): an invalid `dt` sets `last_error`
    /// and moves nothing.
    pub fn step(&mut self, dt: f64) {
        if !(dt.is_finite() && dt > 0.0) {
            self.last_error = Some(DomainError::PropertyOutOfRange { what: "dt".into(), value: dt });
            return;
        }
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
                m if m.is_hydraulic() => {
                    self.pos[i] = (self.pos[i] - v_hyd).max(0.0);
                    if self.pos[i] <= 0.0 {
                        self.motion[i] = RodMotion::Idle;
                    }
                }
                RodMotion::MotorRunIn => {
                    self.pos[i] = (self.pos[i] - v_motor).max(0.0);
                    if self.pos[i] <= 0.0 {
                        self.motion[i] = RodMotion::Idle;
                    }
                }
                _ => unreachable!("every RodMotion variant is handled above"),
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

`reactor.rs` needs `use crate::domain::DomainError;` (P3 added it for `Core::step`; keep it). In P3's `Core::step_rods`, after the line `self.rods.step(dt);` add the forwarding so the rod-level channel reaches `Sim::step`'s step-17 drain:

```rust
        if let Some(e) = self.rods.last_error.take() {
            self.last_error = Some(e);
        }
```

(`Core::step_rods` keeps its own `dt` guard before calling `rods.step`, so P3's `step_rods_and_sim_step_reject_invalid_dt_without_moving` still sees `Core.last_error` set and no motion.)

- [ ] **Step 4: Migrate call sites in `sim.rs` and the P3 tests**

Add `RodMotion` to the reactor import:

```rust
use crate::reactor::{Core, DecayHeat, RodMotion, Xenon, ROD_GRID};
```

In `full_power()` (renamed `full_power_with` in Task 8), the rod-insertion loop currently sets both `pos` and `target`; replace

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

In `scram()`, replace `self.core.rods.scramming = true;` with the layered RPS initiation:

```rust
        self.core.rods.scram_hydraulic();
        self.core.rods.motor_run_in();
```

In `apply`, `Command::ResetScram` arm, replace the final `else { ... }` with

```rust
                } else if self
                    .core
                    .rods
                    .motion
                    .iter()
                    .zip(&self.core.rods.pos)
                    .any(|(m, &p)| matches!(m, RodMotion::Scram | RodMotion::MotorRunIn) && p > 0.0)
                {
                    self.event("scram_reset", "Scram reset blocked: scram insertion not complete");
                } else {
                    self.scrammed = false;
                    self.scram_reason = None;
                    for m in self.core.rods.motion.iter_mut() {
                        if matches!(m, RodMotion::Scram | RodMotion::MotorRunIn) {
                            *m = RodMotion::Idle;
                        }
                    }
                }
```

(`Sri` rods are not touched by a scram reset; they finish on their own.)

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

The SRI call in `protection()` (`self.core.rods.select_rod_insert();`) is unchanged.

**P3 tests that touch the removed fields** (`crates/bwr-core/tests/numerics.rs`; exact edits, preserving every original invalid-input assertion):

- In `step_rods_and_sim_step_reject_invalid_dt_without_moving`, replace the line `core.rods.scramming = true;` with `core.rods.scram_hydraulic();` (the core is built at `ROD_FULL_OUT`, so every rod is `Scram` and would move on a valid `dt`). The assertions on `core.rods.pos`, `core.last_error` and the `Sim` state are unchanged.
- In `non_finite_commands_are_ignored`, replace `let rods_before = sim.core.rods.target.clone();` with `let rods_before = sim.core.rods.targets();` and `assert_eq!(sim.core.rods.target, rods_before);` with `assert_eq!(sim.core.rods.targets(), rods_before);`. Add immediately after it `assert!(sim.core.rods.motion.iter().all(|m| *m == RodMotion::Idle), "a non-finite target must not start motor motion");` with `use bwr_core::reactor::RodMotion;` at the top of that file.
- `kinetics_and_core_reject_invalid_step_arguments_unchanged` needs no edit in this task (Task 4 extends it with `clad_t`).

Search recursively for any remaining use of the removed fields (allowed: the `rods.target(i)`, `rods.targets()` and `rods.scramming()` method calls):

```bash
rg -n "rods\.target\[|rods\.target\.|rods\.target,|rods\.target\)|rods\.target;|rods\.scramming[^(]|fast_insert|\.target\[" crates/bwr-core/src crates/bwr-core/tests crates/bwr-core/examples; echo "rg exit=$?"
```
Expected: no lines and `rg exit=1`. Fix every hit before continuing.

- [ ] **Step 5: Populate the rod extrema trace columns**

In `crates/bwr-core/src/validation.rs`, in the row-filling code of `run_case` (locate it with `grep -n '"rods_avg_pos"\|rods_avg_pos' crates/bwr-core/src/validation.rs`; P2 fills columns by name or by index into `TRACE_COLUMNS`), replace the `0.0` placeholders for `rods_min_pos` and `rods_max_pos` with

```rust
sim.core.rods.pos.iter().cloned().fold(f64::INFINITY, f64::min),
sim.core.rods.pos.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
```

In `docs/validation/method.md`, in the column provenance table, change `rods_min_pos` and `rods_max_pos` from "0.0 until Package 5" to "populated from Package 5 (per-rod positions)".

Append to `crates/bwr-core/tests/core_transients.rs`:

```rust
use bwr_core::validation::{run_case, Case, Operator, ScriptedCommand};

#[test]
fn trace_rod_extrema_columns_are_populated() {
    let case = Case {
        id: "p5_rod_columns",
        scenario: ScenarioName::FullPower,
        duration_s: 10.0,
        sample_dt_s: 1.0,
        commands: vec![ScriptedCommand { at_s: 2.0, cmd: Command::ManualScram }],
        faults: vec![],
        dt_after_s: None,
        operator: Operator::None,
    };
    let trace = run_case(&case, DT);
    let min = trace.column("rods_min_pos");
    let max = trace.column("rods_max_pos");
    let avg = trace.column("rods_avg_pos");
    assert!((max[0] - ROD_FULL_OUT).abs() < 1e-9, "some rods are fully out at full power: {}", max[0]);
    assert!(min[0] <= avg[0] && avg[0] <= max[0]);
    let last = trace.rows.len() - 1;
    assert_eq!((min[last], max[last]), (0.0, 0.0), "all rods in after the scram");
}
```

- [ ] **Step 6: Run the whole suite**

Run: `set -o pipefail; cargo test --release -p bwr-core 2>&1 | tee /tmp/p5-t2.log | grep -E "^test |test result"`
Expected: exit 0 and every test `ok`, including the 16 new `core_transients` tests and all pre-existing `plant`, `numerics` (with the two migrated tests), `conservation`, `vessel_transients`, `register` tests. `stuck_rod_ignores_every_mechanism` passes without `Sim` involvement because it sets `stuck` directly. On any failure stop here; the full log is in `/tmp/p5-t2.log`.

- [ ] **Step 7: Commit (includes the migrated P3 test file)**

```bash
git add crates/bwr-core/src/reactor.rs crates/bwr-core/src/sim.rs crates/bwr-core/src/validation.rs docs/validation/method.md crates/bwr-core/tests/core_transients.rs crates/bwr-core/tests/numerics.rs
git commit -m "feat(core): per-rod motion state machine with scram, SRI, motor and run-in mechanisms

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 3: Stuck-rod fault honoured at plant level

**Files:**
- Modify: `crates/bwr-core/src/sim.rs` (`step`)
- Modify: `crates/bwr-core/tests/core_transients.rs`

**Interfaces:**
- Consumes: `Fault::RodStuck { index }`, `Sim::faults: Vec<Fault>`, `Sim::set_fault`, `Sim::has_fault` (P2); master step order position 2 (`rods.stuck <- faults`, right after `core.step_rods(dt)`).
- Produces: `rods.stuck` mirrored from faults each step; a stuck rod survives scram and is never cleared by automatic logic.

- [ ] **Step 1: Write the failing tests**

Append to `crates/bwr-core/tests/core_transients.rs`:

```rust
use bwr_core::faults::Fault;

fn first_rod_fully_out(sim: &Sim) -> usize {
    (0..N_RODS).find(|&i| sim.core.rods.pos[i] >= ROD_FULL_OUT - 1e-9).expect("a fully withdrawn rod at full power")
}

#[test]
fn stuck_rod_fault_survives_scram_and_is_visible_in_targets() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 1.0);
    let idx = first_rod_fully_out(&sim);
    sim.set_fault(Fault::RodStuck { index: idx }, true);
    run(&mut sim, DT);
    assert!(sim.core.rods.stuck[idx], "fault must be mirrored into rods.stuck");
    sim.apply(Command::ManualScram);
    run(&mut sim, 20.0);
    assert_eq!(sim.core.rods.pos[idx], ROD_FULL_OUT, "stuck rod moved");
    assert_eq!(sim.core.rods.motion[idx], RodMotion::Idle, "a stuck rod is never commanded");
    assert!(sim.core.rods.pos.iter().enumerate().all(|(i, &p)| i == idx || p == 0.0));
    assert!(sim.has_fault(&Fault::RodStuck { index: idx }), "automatic logic cleared the fault");
    let s = sim.snapshot();
    assert_eq!(s.rods.targets[idx], ROD_FULL_OUT, "TGT shows the stuck position, not 0");
    assert!(!s.rods.scram_in_progress, "all commanded rods are in");
    // Reset is permitted: every Scram/MotorRunIn rod is at 0 (the stuck rod was never commanded).
    sim.apply(Command::ResetScram);
    assert!(!sim.scrammed);
    assert_eq!(sim.core.rods.pos[idx], ROD_FULL_OUT);
    // After reset nothing is latched: clearing the fault leaves the rod Idle where it is,
    // and the operator inserts it with a new scram.
    sim.set_fault(Fault::RodStuck { index: idx }, false);
    run(&mut sim, 1.0);
    assert!(!sim.core.rods.stuck[idx]);
    assert_eq!((sim.core.rods.motion[idx], sim.core.rods.pos[idx]), (RodMotion::Idle, ROD_FULL_OUT), "nothing is replayed after reset");
    sim.apply(Command::ManualScram);
    run(&mut sim, SCRAM_TIME + 1.0);
    assert!(sim.core.rods.all_in());
}

#[test]
fn freed_rod_resumes_insertion_while_the_scram_is_latched() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 1.0);
    let idx = first_rod_fully_out(&sim);
    sim.set_fault(Fault::RodStuck { index: idx }, true);
    run(&mut sim, DT);
    sim.apply(Command::ManualScram);
    run(&mut sim, 20.0);
    assert!(sim.scrammed && !sim.core.rods.scramming(), "latched but hydraulic insertion complete");
    assert_eq!(sim.core.rods.pos[idx], ROD_FULL_OUT);
    sim.set_fault(Fault::RodStuck { index: idx }, false);
    run(&mut sim, 2.0 * DT);
    assert_eq!(sim.core.rods.motion[idx], RodMotion::Scram, "the latched RPS demand is re-asserted on the freed rod");
    assert!(sim.core.rods.scramming());
    run(&mut sim, SCRAM_TIME + 1.0);
    assert!(sim.core.rods.all_in());
    assert!(!sim.core.rods.scramming(), "scramming() clears once the freed rod is in");
}

#[test]
fn fault_during_scram_parks_the_rod_and_does_not_block_reset() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 1.0);
    let idx = first_rod_fully_out(&sim);
    sim.apply(Command::ManualScram);
    run(&mut sim, 1.0); // mid-stroke: about 16 positions travelled
    assert_eq!(sim.core.rods.motion[idx], RodMotion::Scram);
    sim.set_fault(Fault::RodStuck { index: idx }, true);
    sim.step(DT); // moves once more (step 1), then the refresh parks it (step 2)
    let parked = sim.core.rods.pos[idx];
    assert!(parked > 0.0 && parked < ROD_FULL_OUT - 10.0, "parked mid-stroke at {parked}");
    assert_eq!(sim.core.rods.motion[idx], RodMotion::Idle, "a rod stuck mid-scram is parked, not left in Scram");
    assert_eq!(sim.core.rods.target(idx), parked, "TGT = POS");
    run(&mut sim, SCRAM_TIME + 1.0);
    assert_eq!(sim.core.rods.pos[idx], parked, "parked rod moved");
    assert!(!sim.core.rods.scramming(), "scramming() must not stick true because of the stuck rod");
    assert!(sim.core.rods.pos.iter().enumerate().all(|(i, &p)| i == idx || p == 0.0));
    sim.apply(Command::ResetScram);
    assert!(!sim.scrammed, "reset must not be blocked by a parked stuck rod");
    assert!(sim.core.rods.motion.iter().all(|m| *m == RodMotion::Idle));
}

#[test]
fn stuck_rod_fault_is_applied_after_rods_step() {
    // Master step order: rods move (step 1) before the fault mask is refreshed (step 2),
    // so a fault and a command issued at the same boundary still move the rod once;
    // the refresh then parks it with the motor command dropped.
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 1.0);
    let idx = first_rod_fully_out(&sim);
    sim.apply(Command::RodSetTarget { indices: vec![idx], target: ROD_FULL_OUT - 2.0 });
    sim.set_fault(Fault::RodStuck { index: idx }, true);
    sim.step(DT);
    let after_one = sim.core.rods.pos[idx];
    assert!(after_one < ROD_FULL_OUT, "the step in which the fault arrives still moves the rod once");
    assert_eq!(sim.core.rods.motion[idx], RodMotion::Idle, "the motor command is dropped by the refresh");
    run(&mut sim, 5.0);
    assert_eq!(sim.core.rods.pos[idx], after_one, "stuck from the next step on");
    // A motor command issued while the mask is current is ignored outright.
    sim.apply(Command::RodSetTarget { indices: vec![idx], target: ROD_FULL_OUT - 4.0 });
    run(&mut sim, 1.0);
    assert_eq!(sim.core.rods.pos[idx], after_one);
    assert_eq!(sim.core.rods.motion[idx], RodMotion::Idle);
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

Run: `cargo test --release -p bwr-core --test core_transients stuck_rod_fault > /tmp/p5-t3-red.log 2>&1; echo exit=$?; grep -n "panicked\|FAILED" /tmp/p5-t3-red.log`
Expected: `exit=101`; FAIL at `assert!(sim.core.rods.stuck[idx])` ("fault must be mirrored into rods.stuck").

- [ ] **Step 3: Mirror faults into the rods every step and re-assert a latched scram**

In `Sim::step`, immediately after `self.core.step_rods(dt);` (master step order position 2, after the rods have moved):

```rust
        // Faults are the only source of stuck rods; the mask is mirrored here so the
        // mechanism code never needs to know about the fault list. A rod that becomes
        // stuck is parked Idle (its command dropped). While the RPS demand is latched
        // a freed rod must resume insertion, so the scram is re-asserted every step;
        // scram_hydraulic/motor_run_in skip rods at 0, so scramming() clears normally.
        let stuck_mask: Vec<bool> = (0..N_RODS).map(|i| self.has_fault(&Fault::RodStuck { index: i })).collect();
        self.core.rods.refresh_stuck(&stuck_mask);
        if self.scrammed {
            self.core.rods.scram_hydraulic();
            self.core.rods.motor_run_in();
        }
```

Add `use crate::faults::Fault;` to the `sim.rs` imports if P2 did not already.

- [ ] **Step 4: Run tests to verify they pass**

Run: `set -o pipefail; cargo test --release -p bwr-core --test core_transients 2>&1 | tee /tmp/p5-t3.log | grep -E "^test |test result"`
Expected: exit 0; all 21 tests `ok`. Then the whole suite: `set -o pipefail; cargo test --release -p bwr-core 2>&1 | tee /tmp/p5-t3-all.log | grep -E "test result"` with exit 0 (the re-assert must not disturb `plant::manual_scram_shuts_down_without_isolation` or the P3 `numerics` tests).

- [ ] **Step 5: Commit**

```bash
git add crates/bwr-core/src/sim.rs crates/bwr-core/tests/core_transients.rs
git commit -m "feat(core): honour Fault::RodStuck in the rod mechanisms

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 4: Two-node fuel and cladding thermal model

**Files:**
- Modify: `crates/bwr-core/src/params.rs` (remove `FUEL_RESISTANCE`; add three constants)
- Modify: `crates/bwr-core/src/reactor.rs` (`Core.clad_t`, `Core::step`)
- Modify: `crates/bwr-core/src/sim.rs` (`full_power` initial temperatures)
- Modify: `crates/bwr-core/src/api.rs`, `ui/src/sim/types.ts`, `ui/src/sim/mock.ts` (`cladTempC`)
- Modify: `crates/bwr-core/src/validation.rs`, `docs/validation/method.md` (`clad_t_c` column)
- Modify: `crates/bwr-core/tests/core_transients.rs`

**Interfaces:**
- Consumes: `Core::step(dt, coolant_t, htc_factor) -> Result<f64, DomainError>` as left by P3 (rods are no longer stepped inside).
- Produces: `Core.clad_t: f64` (public; P10's `Extrema.clad_t_c_max` reads it), `Snapshot.neutronics.clad_temp_c`, the `clad_t_c` trace column, constants `GAP_RESISTANCE_K_PER_W`, `CLAD_COOLANT_RESISTANCE_K_PER_W`, `CLAD_HEAT_CAP_J_PER_K`.

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

/// Component fixture for the thermal nodes: rated power held constant by setting the
/// kinetics exactly critical (`rho_total = 0`; `update_reactivity` is NOT called, so the
/// calibrated coefficients cannot push the fixture outside KIN_EXCESS_MAX), equilibrium
/// precursors and decay heat, and the fixed source. Power drifts by under SOURCE * t
/// (2e-4 over 20 s), which is below every tolerance used here.
fn critical_core_at(fuel_t: f64, clad_t: f64) -> bwr_core::reactor::Core {
    let mut c = bwr_core::reactor::Core::new(1.0, ROD_FULL_OUT, fuel_t);
    c.clad_t = clad_t;
    c.rho_total = 0.0;
    c
}

/// Steady two-node temperatures for heat `q_fuel` through the fuel at coolant `t_cool`.
fn steady_nodes(q_fuel: f64, t_cool: f64, htc: f64) -> (f64, f64) {
    let clad = t_cool + CLAD_COOLANT_RESISTANCE_K_PER_W / htc * q_fuel;
    (clad + GAP_RESISTANCE_K_PER_W * q_fuel, clad)
}

#[test]
fn two_node_update_is_stable_and_converges_for_every_step_size() {
    for &dt in &[DT, 0.5, 5.0, 50.0] {
        let mut c = critical_core_at(800.0, 800.0);
        let q_fuel = c.thermal_power() * (1.0 - DIRECT_COOLANT_HEAT);
        let (f_inf, c_inf) = steady_nodes(q_fuel, 287.0, 1.0);
        let mut t = 0.0;
        while t < 600.0 {
            let q = c.step(dt, 287.0, 1.0).unwrap_or_else(|e| panic!("dt {dt}: thermal step rejected: {e:?}"));
            t += dt;
            assert!(q.is_finite() && c.fuel_t.is_finite() && c.clad_t.is_finite(), "dt {dt}");
            assert!(c.fuel_t >= c.clad_t - 1e-6 && c.clad_t >= 287.0 - 1e-6, "dt {dt}: fuel {} clad {}", c.fuel_t, c.clad_t);
            assert!(c.fuel_t <= 800.0 + 1e-6, "dt {dt}: no overshoot above the initial state");
        }
        assert!((c.kin.n - 1.0).abs() < 1e-3, "dt {dt}: fixture power drifted to {}", c.kin.n);
        assert!((c.fuel_t - f_inf).abs() < 0.5 && (c.clad_t - c_inf).abs() < 0.5, "dt {dt}: ({}, {}) vs steady ({f_inf}, {c_inf})", c.fuel_t, c.clad_t);
    }
}

#[test]
fn two_node_energy_identity_holds_each_step() {
    // Generated thermal energy = fuel storage + clad storage + energy returned to the
    // coolant, exactly, for backward Euler; includes DIRECT_COOLANT_HEAT and reduced coverage.
    for &(htc, dt, fuel0, clad0) in &[(1.0, DT, 800.0, 600.0), (0.3, DT, 800.0, 600.0), (0.02, 0.5, 1400.0, 1400.0), (1.0, 5.0, 300.0, 290.0)] {
        let mut c = critical_core_at(fuel0, clad0);
        let q_out = c.step(dt, 287.0, htc).unwrap();
        let p = c.thermal_power(); // the value used inside step: nothing advances after the thermal update
        let stored = FUEL_HEAT_CAP * (c.fuel_t - fuel0) + CLAD_HEAT_CAP_J_PER_K * (c.clad_t - clad0);
        let generated = p * dt;
        let removed = q_out * dt;
        assert!(
            (generated - stored - removed).abs() < 1e-9 * generated.abs(),
            "htc {htc} dt {dt}: generated {generated:.3e} = stored {stored:.3e} + removed {removed:.3e}"
        );
        assert!(q_out.is_finite());
    }
}

#[test]
fn two_node_transient_matches_an_independent_rk4_reference() {
    // Constant-input relaxation from a hot fuel node (cladding started near its
    // quasi-steady value so the stiff clad mode is small) compared with a classical RK4
    // integration of the same two ODEs at h = 1e-4 s. Predeclared bound: 2 K on both
    // nodes at every checkpoint; the final state must also sit within 1 K of the
    // analytic steady state.
    let (t_cool, htc) = (287.0, 1.0);
    let mut c = critical_core_at(800.0, 320.0);
    let q_fuel = c.thermal_power() * (1.0 - DIRECT_COOLANT_HEAT);
    let r_g = GAP_RESISTANCE_K_PER_W;
    let r_c = CLAD_COOLANT_RESISTANCE_K_PER_W / htc;
    let deriv = |tf: f64, tc: f64| {
        let gap = (tf - tc) / r_g;
        ((q_fuel - gap) / FUEL_HEAT_CAP, (gap - (tc - t_cool) / r_c) / CLAD_HEAT_CAP_J_PER_K)
    };
    let (mut rf, mut rc) = (c.fuel_t, c.clad_t);
    let h = 1e-4;
    let mut t_ref = 0.0;
    let mut t_core = 0.0;
    for &tk in &[1.0, 5.0, 20.0] {
        while t_ref < tk - 1e-9 {
            let (k1f, k1c) = deriv(rf, rc);
            let (k2f, k2c) = deriv(rf + 0.5 * h * k1f, rc + 0.5 * h * k1c);
            let (k3f, k3c) = deriv(rf + 0.5 * h * k2f, rc + 0.5 * h * k2c);
            let (k4f, k4c) = deriv(rf + h * k3f, rc + h * k3c);
            rf += h / 6.0 * (k1f + 2.0 * k2f + 2.0 * k3f + k4f);
            rc += h / 6.0 * (k1c + 2.0 * k2c + 2.0 * k3c + k4c);
            t_ref += h;
        }
        while t_core < tk - 1e-9 {
            c.step(DT, t_cool, htc).unwrap();
            t_core += DT;
        }
        assert!((c.fuel_t - rf).abs() < 2.0, "t={tk}: fuel {} vs RK4 {rf}", c.fuel_t);
        assert!((c.clad_t - rc).abs() < 2.0, "t={tk}: clad {} vs RK4 {rc}", c.clad_t);
    }
    let (f_inf, c_inf) = steady_nodes(q_fuel, t_cool, htc);
    assert!((c.fuel_t - f_inf).abs() < 1.0 && (c.clad_t - c_inf).abs() < 1.0, "steady ({}, {}) vs ({f_inf}, {c_inf})", c.fuel_t, c.clad_t);
}
```

Also extend P3's argument-guard test so the new node is covered (`crates/bwr-core/tests/numerics.rs`, `kinetics_and_core_reject_invalid_step_arguments_unchanged`): after the line `assert_eq!(core.fuel_t, core0.fuel_t, "{what}");` add `assert_eq!(core.clad_t, core0.clad_t, "{what}");`. Every original assertion stays.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --release -p bwr-core --test core_transients rated_fuel > /tmp/p5-t4-red.log 2>&1; echo exit=$?; grep -n "no field\|cannot find" /tmp/p5-t4-red.log`
Expected: `exit=101`; compile error `no field clad_t on type Core` / `cannot find value CLAD_COOLANT_RESISTANCE_K_PER_W`.

- [ ] **Step 3: Parameters and their register rows (same task, master register-row rule)**

In `params.rs`, delete the two lines

```rust
/// Fuel-to-coolant thermal resistance, K/W (avg fuel ~550 degC at rated). ASSUMED.
pub const FUEL_RESISTANCE: f64 = 264.0 / (RATED_POWER_W * (1.0 - DIRECT_COOLANT_HEAT));
```

and insert in their place:

```rust
/// Heat reaching the coolant through the fuel at rated power, W. DERIVED.
pub const Q_FUEL_RATED: f64 = RATED_POWER_W * (1.0 - DIRECT_COOLANT_HEAT);
/// Pellet + gap thermal resistance, K/W: 252 K at rated (avg fuel ~552 degC). ASSUMED (A-050).
pub const GAP_RESISTANCE_K_PER_W: f64 = 252.0 / Q_FUEL_RATED;
/// Cladding-to-coolant resistance, K/W: 12 K at rated. ASSUMED (A-050).
/// GAP + CLAD = 264 K at rated, equal to the former single-node FUEL_RESISTANCE.
pub const CLAD_COOLANT_RESISTANCE_K_PER_W: f64 = 12.0 / Q_FUEL_RATED;
/// Cladding lumped heat capacity (~9.5 t Zircaloy x 330 J/kgK), J/K. ASSUMED (A-051).
pub const CLAD_HEAT_CAP_J_PER_K: f64 = 3.1e6;
```

In `docs/model/parameters.csv`: delete P1's `FUEL_RESISTANCE` row (the constant no longer exists; `every_params_const_is_registered` checks params.rs -> csv, and the history lives in the `GAP_RESISTANCE_K_PER_W` notes), and append (13 fields each, no commas inside fields):

```
Q_FUEL_RATED,8.4825e8,W,Heat reaching the coolant through the fuel at rated power: RATED_POWER_W x (1 - DIRECT_COOLANT_HEAT),hot full power,derived,derived,,,,A-005,crates/bwr-core/src/params.rs,denominator of the two thermal resistances
GAP_RESISTANCE_K_PER_W,2.9712e-7,K/W,Pellet plus gap thermal resistance (252 K at Q_FUEL_RATED),hot full power,BWRX-300,assumed,,2.4e-7,3.6e-7,A-050,crates/bwr-core/src/params.rs,sum with CLAD_COOLANT_RESISTANCE_K_PER_W equals the former single-node FUEL_RESISTANCE (264 K at rated) removed in Package 5 so rated fuel temperature is unchanged
CLAD_COOLANT_RESISTANCE_K_PER_W,1.4148e-8,K/W,Cladding to coolant thermal resistance (12 K at Q_FUEL_RATED),hot full power,BWRX-300,assumed,,0.7e-8,2.8e-8,A-050,crates/bwr-core/src/params.rs,scaled by 1/htc_factor (floor 0.02) when the core is uncovered
CLAD_HEAT_CAP_J_PER_K,3.1e6,J/K,Cladding lumped heat capacity (9.5 t Zircaloy x 330 J/kgK),any,BWRX-300,assumed,,2.5e6,4.0e6,A-051,crates/bwr-core/src/params.rs,affects sub-second clad response only
```

Run the register gate now: `cargo test --release -p bwr-core --test register` (expected: all `ok`; the A-050/A-051 rows exist from Task 1).

- [ ] **Step 4: Cladding node in `Core` (P3 argument guards preserved)**

In `reactor.rs`, add the field after `fuel_t`:

```rust
    /// Lumped average fuel temperature, degC.
    pub fuel_t: f64,
    /// Lumped average cladding temperature, degC.
    pub clad_t: f64,
```

In `Core::new`, add `clad_t: fuel_t,` next to `fuel_t,`.

Replace the thermal part of `Core::step` (everything after `self.decay.step(self.kin.n, dt);`). P3's three argument guards stay exactly where they are, BEFORE `kin.advance`, so `kinetics_and_core_reject_invalid_step_arguments_unchanged` keeps passing (the kinetics validate `dt` but cannot validate `coolant_t` or `htc_factor`; without the guards a NaN coolant temperature would advance kinetics, xenon and decay heat before corrupting both nodes). The whole function reads:

```rust
    /// Advance neutronics, xenon, decay heat and the fuel/cladding nodes.
    /// Rods are stepped separately by `step_rods` so reactivity is current.
    /// `coolant_t` is the coolant temperature, `htc_factor` scales clad-to-coolant
    /// heat transfer (1 = normal, lower when the core is uncovered).
    /// Returns heat delivered to the coolant, W.
    pub fn step(&mut self, dt: f64, coolant_t: f64, htc_factor: f64) -> Result<f64, DomainError> {
        // Master dt rule and non-finite inputs rule (P3): every argument, before any mutation.
        if !(dt.is_finite() && dt > 0.0) {
            return Err(DomainError::PropertyOutOfRange { what: "dt".into(), value: dt });
        }
        if !coolant_t.is_finite() {
            return Err(DomainError::PropertyOutOfRange { what: "coolant_t".into(), value: coolant_t });
        }
        if !htc_factor.is_finite() {
            return Err(DomainError::PropertyOutOfRange { what: "htc_factor".into(), value: htc_factor });
        }
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

- [ ] **Step 7: Populate the `clad_t_c` trace column**

In `crates/bwr-core/src/validation.rs`, in the row-filling code of `run_case` (locate it with `grep -n 'clad_t_c\|"fuel_t_c"' crates/bwr-core/src/validation.rs`), replace the `0.0` placeholder for `clad_t_c` with `sim.core.clad_t`. In `docs/validation/method.md`, change the `clad_t_c` provenance from "0.0 until Package 5" to "populated from Package 5 (`Core.clad_t`)".

Append to `crates/bwr-core/tests/core_transients.rs`:

```rust
#[test]
fn trace_clad_column_is_populated() {
    let case = Case {
        id: "p5_clad_column",
        scenario: ScenarioName::FullPower,
        duration_s: 5.0,
        sample_dt_s: 1.0,
        commands: vec![],
        faults: vec![],
        dt_after_s: None,
        operator: Operator::None,
    };
    let trace = run_case(&case, DT);
    let clad = trace.column("clad_t_c");
    let fuel = trace.column("fuel_t_c");
    let cool = trace.column("t_coolant_c");
    for k in 0..trace.rows.len() {
        assert!(cool[k] < clad[k] && clad[k] < fuel[k], "row {k}: coolant {} clad {} fuel {}", cool[k], clad[k], fuel[k]);
    }
}
```

- [ ] **Step 8: Run tests and UI typecheck**

Run: `set -o pipefail; cargo test --release -p bwr-core 2>&1 | tee /tmp/p5-t4.log | grep -E "^test |test result"`
Expected: exit 0; all `ok`, including the six new tests (`rated_fuel_and_clad_temperatures_match_resistance_derivation`, `clad_responds_faster_than_fuel_on_scram`, `two_node_update_is_stable_and_converges_for_every_step_size`, `two_node_energy_identity_holds_each_step`, `two_node_transient_matches_an_independent_rk4_reference`, `trace_clad_column_is_populated`), the extended `numerics::kinetics_and_core_reject_invalid_step_arguments_unchanged`, and `register`. `plant::full_power_is_steady` must still pass without retuning. The three component tests are this package's independent physical gates for the thermal model and are re-run before baseline regeneration (Task 11).

Run: `npm --prefix ui run typecheck`
Expected: exit 0, no output.

- [ ] **Step 9: Commit**

```bash
git add crates/bwr-core/src/params.rs crates/bwr-core/src/reactor.rs crates/bwr-core/src/sim.rs crates/bwr-core/src/api.rs crates/bwr-core/src/validation.rs docs/validation/method.md docs/model/parameters.csv crates/bwr-core/tests/core_transients.rs crates/bwr-core/tests/numerics.rs ui/src/sim/types.ts ui/src/sim/mock.ts
git commit -m "feat(core): two-node fuel/cladding thermal model replacing the single fuel node

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 5: Fuel model envelope replaces the "core damage" verdict

**Files:**
- Modify: `crates/bwr-core/src/params.rs` (`FUEL_MODEL_T_MAX_C`, `UNCOVERED_ENVELOPE_S`, `CORE_COVERED_FRAC`) and `docs/model/parameters.csv` (their rows)
- Modify: `crates/bwr-core/src/sim.rs` (remove the legacy damage-verdict field; add `uncovered_s`, `clad_t_step_max`, envelope detection, alarm)
- Modify: `crates/bwr-core/src/api.rs`, `ui/src/sim/types.ts`, `ui/src/sim/mock.ts`
- Modify: `crates/bwr-core/tests/core_transients.rs`

**Interfaces:**
- Consumes: `DomainError::FuelModelEnvelope { fuel_t_c, clad_t_c }`, `Sim::domain_errors: Vec<(f64, DomainError)>` (P3); `Vessel::new(t, level_z, q, w_feed, t_feed, vent_open)` and `Vessel::core_coverage()` (P3/P4) for the low-inventory fixture.
- Produces: `Sim::uncovered_s`, `Sim::clad_t_step_max`, `Sim::fuel_envelope_exceeded()`, `Snapshot.fuel_envelope_exceeded`, alarm id `fuel_envelope`, `CORE_COVERED_FRAC`.

Detection semantics (core.md section 5, written in Task 1): the cladding check uses the plant-step endpoints, `clad_t_step_max = max(entry, exit)`, so an entry-state excursion latches even though the fast cladding node cools within the step; the uncovered timer accumulates continuous time with `core_coverage() < CORE_COVERED_FRAC`, resets on any covered step, and the limit is strict.

- [ ] **Step 1: Write the failing tests**

Append to `crates/bwr-core/tests/core_transients.rs` (`DomainError` is already imported at the top of the file from Task 2):

```rust
use bwr_core::bop::LevelControlMode;
use bwr_core::vessel::Vessel;

fn envelope_errors(sim: &Sim) -> usize {
    sim.domain_errors.iter().filter(|(_, e)| matches!(e, DomainError::FuelModelEnvelope { .. })).count()
}

/// Replace the vessel with a thermodynamically consistent low-inventory state built
/// through the vessel constructor (mixture level at mid-core, same coolant temperature),
/// and hold feed at zero so the level cannot recover on its own. No production hook.
fn uncover_core(sim: &mut Sim) {
    let t = sim.vessel.eos.t;
    sim.vessel = Vessel::new(t, Z_CORE_BOTTOM + 0.5 * CORE_HEIGHT, RATED_POWER_W, 0.0, T_FW_RATED, false);
    sim.apply(Command::SetLevelControl { mode: LevelControlMode::Manual });
    sim.apply(Command::SetFcvManual { pos: 0.0 });
    assert!(sim.vessel.core_coverage() < CORE_COVERED_FRAC, "fixture is not uncovered: {}", sim.vessel.core_coverage());
}

/// Put back a consistent full-inventory vessel at normal water level.
fn recover_core(sim: &mut Sim) {
    let t = sim.vessel.eos.t;
    sim.vessel = Vessel::new(t, Z_NWL, RATED_POWER_W, STEAM_FLOW_RATED, T_FW_RATED, false);
    assert!(sim.vessel.core_coverage() >= CORE_COVERED_FRAC);
}

#[test]
fn entry_state_cladding_excursion_latches_the_envelope_once() {
    // Entry-state detection: the seeded cladding node cools well below the limit within
    // one step (fast clad mode), so only the step-endpoint maximum can catch it.
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 1.0);
    assert!(!sim.fuel_envelope_exceeded());
    sim.core.clad_t = FUEL_MODEL_T_MAX_C + 1.0;
    run(&mut sim, DT);
    assert!(sim.core.clad_t < FUEL_MODEL_T_MAX_C, "fixture check: the clad node cooled within the step to {}", sim.core.clad_t);
    assert!(sim.fuel_envelope_exceeded(), "entry-state excursion must latch (core.md section 5)");
    assert!((sim.clad_t_step_max - (FUEL_MODEL_T_MAX_C + 1.0)).abs() < 1e-9);
    let n = envelope_errors(&sim);
    run(&mut sim, 1.0);
    assert_eq!((n, envelope_errors(&sim)), (1, 1), "envelope error recorded once");
    let s = sim.snapshot();
    assert!(s.fuel_envelope_exceeded);
    assert!(s.alarms.iter().any(|a| a.id == "fuel_envelope" && a.text == "FUEL THERMAL MODEL ENVELOPE EXCEEDED"));
    assert!(s.alarms.iter().all(|a| !a.text.contains("DAMAGE")), "the damage verdict must be gone");
    assert!(s.unsupported.is_some(), "P3 unsupported flag must also be set");
}

#[test]
fn sustained_cladding_exceedance_in_an_uncovered_hot_core_latches() {
    // Post-step detection with a physically consistent fixture: an uncovered core
    // (clad-coolant resistance at its 1/0.02 floor) with a hot fuel node keeps the
    // cladding above the limit AFTER the thermal update, not only at entry.
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 1.0);
    uncover_core(&mut sim);
    sim.core.fuel_t = 1400.0;
    sim.core.clad_t = 1400.0;
    run(&mut sim, DT);
    assert!(sim.core.clad_t > FUEL_MODEL_T_MAX_C, "fixture check: clad after the step is {}", sim.core.clad_t);
    assert!(sim.fuel_envelope_exceeded());
    assert_eq!(envelope_errors(&sim), 1);
}

#[test]
fn prolonged_uncovery_accumulates_resets_and_latches_once() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 1.0);
    assert_eq!(sim.uncovered_s, 0.0, "covered at full power");
    uncover_core(&mut sim);
    let steps_to_limit = (UNCOVERED_ENVELOPE_S / DT).round() as usize;
    // Just below the limit: accumulated, nothing raised.
    for _ in 0..steps_to_limit - 1 {
        sim.step(DT);
        assert!(sim.vessel.core_coverage() < CORE_COVERED_FRAC, "fixture must stay uncovered at t={}", sim.time);
    }
    assert!((sim.uncovered_s - (UNCOVERED_ENVELOPE_S - DT)).abs() < 1e-6, "timer {}", sim.uncovered_s);
    assert!(!sim.fuel_envelope_exceeded(), "below the limit");
    // At the limit: the timer is a floating sum, so only its value is asserted here.
    sim.step(DT);
    assert!((sim.uncovered_s - UNCOVERED_ENVELOPE_S).abs() < 1e-6, "timer {}", sim.uncovered_s);
    // Above the limit: raised once.
    sim.step(DT);
    assert!(sim.fuel_envelope_exceeded(), "above the limit");
    run(&mut sim, 1.0);
    assert_eq!(envelope_errors(&sim), 1, "latched once");
    // Recovery with a consistent covered vessel resets the timer; the latch persists (P3).
    recover_core(&mut sim);
    sim.step(DT);
    assert_eq!(sim.uncovered_s, 0.0, "covered core must reset the timer");
    assert!(sim.fuel_envelope_exceeded());
}

#[test]
fn brief_uncovery_resets_without_raising() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 1.0);
    uncover_core(&mut sim);
    run(&mut sim, 5.0);
    assert!(sim.uncovered_s > 4.9 && sim.uncovered_s < 5.1, "timer {}", sim.uncovered_s);
    recover_core(&mut sim);
    sim.step(DT);
    assert_eq!(sim.uncovered_s, 0.0);
    assert!(!sim.fuel_envelope_exceeded());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --release -p bwr-core --test core_transients envelope > /tmp/p5-t5-red.log 2>&1; echo exit=$?; grep -n "no method\|cannot find" /tmp/p5-t5-red.log`
Expected: `exit=101`; compile error `no method named fuel_envelope_exceeded` / `cannot find value FUEL_MODEL_T_MAX_C`.

- [ ] **Step 3: Parameters and their register rows**

Append to `params.rs` core section:

```rust
/// Cladding temperature above which the fuel thermal model is outside its
/// envelope. An indication, not a damage limit (A-050). ASSUMED.
pub const FUEL_MODEL_T_MAX_C: f64 = 1000.0;
/// Continuous core-uncovered time after which the thermal model is outside
/// its envelope (no post-dryout heat transfer modelled). ASSUMED (A-050).
pub const UNCOVERED_ENVELOPE_S: f64 = 60.0;
/// Core coverage fraction at or above which the core counts as covered (the
/// uncovered timer and the low-coverage alarm share it). ASSUMED (A-050).
pub const CORE_COVERED_FRAC: f64 = 0.999;
```

`sim.rs` already compares `core_coverage()` with the literal `0.999` for the coverage alarm in `active_alarms()` (`grep -n "0.999" crates/bwr-core/src/sim.rs`); replace that literal with `CORE_COVERED_FRAC` so there is one definition.

Append to `docs/model/parameters.csv`:

```
FUEL_MODEL_T_MAX_C,1000,degC,Cladding temperature above which the fuel thermal model is outside its envelope (indication not a damage limit),any,model,assumed,,none,none,A-050,crates/bwr-core/src/params.rs,raises DomainError::FuelModelEnvelope; checked on plant-step endpoints (core.md section 5)
UNCOVERED_ENVELOPE_S,60,s,Continuous core-uncovered duration beyond which the thermal model is outside its envelope,any,model,assumed,,none,none,A-050,crates/bwr-core/src/params.rs,no post-dryout heat transfer modelled; strict limit
CORE_COVERED_FRAC,0.999,-,Core coverage fraction at or above which the core counts as covered,any,model,assumed,,none,none,A-050,crates/bwr-core/src/params.rs,shared by the uncovered timer and the low-coverage alarm
```

(`none`/`none` intervals on `assumed` rows are allowed because A-050's Interval basis names these three as model-envelope choices without an interval.) Run `cargo test --release -p bwr-core --test register` (expected: all `ok`).

- [ ] **Step 4: Engine changes in `sim.rs`**

Locate the legacy damage verdict: `grep -n "damage" crates/bwr-core/src/sim.rs` (at the baseline: the `bool` field on `Sim`, its `false` initialiser in `base()`, the 1200 degC block in `protection()` that sets it and emits the event, and the alarm `add(...)` in `active_alarms()`). Remove the field and its initialiser. Add the fields

```rust
    /// Continuous time the core has been uncovered, s (envelope timer).
    pub uncovered_s: f64,
    /// max(clad_t at step entry, clad_t at step exit) of the last plant step (envelope check).
    pub clad_t_step_max: f64,
```

initialised to `0.0` and `0.0` in `base()`; `finalize_init` (P3) sets `clad_t_step_max = core.clad_t`.

Add to `impl Sim` (P3's public `Sim::domain_error(e)` records the first occurrence of each variant and logs it; this package only adds the query):

```rust
    pub fn fuel_envelope_exceeded(&self) -> bool {
        self.domain_errors.iter().any(|(_, e)| matches!(e, DomainError::FuelModelEnvelope { .. }))
    }
```

In `step()`, record the entry cladding temperature before the core step and the endpoint maximum after it:

```rust
        let clad_entry = self.core.clad_t;
        let core_err = match self.core.step(dt, t_cool, cov) { /* unchanged P3 arms */ };
        self.clad_t_step_max = clad_entry.max(self.core.clad_t);
```

In `protection()`, replace the 1200 degC block (the `if self.core.fuel_t > 1200.0 && ...` statement and its event) with

```rust
        // Fuel thermal model envelope (not a damage verdict; see docs/model/core.md section 5).
        if self.clad_t_step_max > FUEL_MODEL_T_MAX_C || self.uncovered_s > UNCOVERED_ENVELOPE_S {
            self.domain_error(DomainError::FuelModelEnvelope { fuel_t_c: self.core.fuel_t, clad_t_c: self.core.clad_t });
        }
```

In `step()`, immediately after `self.vessel.step(dt, &inputs)` (and the ledger record P2/P3 placed there), add:

```rust
        let covered = self.vessel.core_coverage() >= CORE_COVERED_FRAC;
        self.uncovered_s = if covered { 0.0 } else { self.uncovered_s + dt };
```

In `active_alarms()`, replace the legacy `add(...)` for the damage verdict with

```rust
        if self.fuel_envelope_exceeded() {
            add("fuel_envelope", "FUEL THERMAL MODEL ENVELOPE EXCEEDED".into(), Severity::Trip);
        }
```

Add `use crate::domain::DomainError;` to the imports if absent. Afterwards `grep -n "damage" crates/bwr-core/src/sim.rs` must show only the comment "not a damage verdict".

- [ ] **Step 5: API and UI**

`api.rs` `Snapshot`: add `pub fuel_envelope_exceeded: bool,` after `scram_reason`. `sim.rs` `snapshot()`: add `fuel_envelope_exceeded: self.fuel_envelope_exceeded(),` after `scram_reason: ...`.

`ui/src/sim/types.ts` `Snapshot`: add after `scramReason: string | null;`:

```ts
  /** True once the fuel thermal model has left its declared envelope (not a damage verdict). */
  fuelEnvelopeExceeded: boolean;
```

`ui/src/sim/mock.ts` holds no legacy damage string (verified at the baseline), so the edits are unconditional:

1. In `blankSnapshot()`, directly after the line `scramReason: null,` add `fuelEnvelopeExceeded: false,` (top level of the returned object, matching the Rust field position).
2. Reset/update policy for the demo: `loadScenario` rebuilds the snapshot from `blankSnapshot()` (the `const s = blankSnapshot();` in the scenario loader), which resets the flag to `false` on every scenario load, mirroring the engine's `LoadScenario` behaviour. In the per-tick alarm table (the array of `[id, text, severity, condition]` rows that contains `["scram", ...]`), add the row `["fuel_envelope", "FUEL THERMAL MODEL ENVELOPE EXCEEDED", "trip", s.fuelEnvelopeExceeded]`, and immediately before that table is evaluated set `s.fuelEnvelopeExceeded = s.fuelEnvelopeExceeded || s.neutronics.cladTempC > 1000;` (latching, demo-only; the mock is a UI demonstration, master "Keep Rust authoritative"). Leave the existing `["fuel", "FUEL TEMP HIGH", ...]` row as it is (it is an indication, not a verdict).

No UI component renders a damage verdict at the baseline (`rg -n "DAMAGE" ui/src` is empty); if one appears from a later package, read `s.fuelEnvelopeExceeded` and show `FUEL MODEL ENVELOPE`.

- [ ] **Step 6: Run tests, typecheck, build**

Run: `set -o pipefail; cargo test --release -p bwr-core 2>&1 | tee /tmp/p5-t5.log | grep -E "^test |test result"`
Expected: exit 0; all `ok`, including the four envelope tests and `register`.

Run: `npm --prefix ui run typecheck && npm --prefix ui run build > /tmp/p5-t5-ui.log 2>&1; echo exit=$?; tail -3 /tmp/p5-t5-ui.log`
Expected: `exit=0`; typecheck silent; build ends with `✓ built in ...`.

- [ ] **Step 7: Commit**

```bash
git add crates/bwr-core/src/params.rs crates/bwr-core/src/sim.rs crates/bwr-core/src/api.rs docs/model/parameters.csv crates/bwr-core/tests/core_transients.rs ui/src
git commit -m "feat(core): fuel thermal model envelope replaces the 1200 degC core damage verdict

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 6: Decay heat reference, fit tool, and finite irradiation history

**Files:**
- Modify: `docs/validation/findings.md` (F-nnn: reference retrieval outcome; fit-gate outcome if taken)
- Create (retrieved branch only): `docs/model/decay-heat-reference.csv`
- Rewrite: `tools/fit_decay_heat.py`
- Modify: `crates/bwr-core/src/params.rs` (`DECAY_GROUPS` regenerated in the retrieved branch; `IRRADIATION_HISTORY_S`; `DECAY_FIT_MAX_REL_ERR` in the retrieved branch), `docs/model/parameters.csv` (their rows)
- Modify: `crates/bwr-core/src/reactor.rs` (`DecayHeat::after_history`, tests)
- Modify: `crates/bwr-core/src/sim.rs` (scenario initialisation)
- Modify: `docs/model/sources.csv` (retrieved branch), `docs/model/core.md` section 6, `README.md`

**Interfaces:**
- Produces: `DecayHeat::after_history(n: f64, irr_s: f64, cooled_s: f64) -> DecayHeat`, `IRRADIATION_HISTORY_S`; retrieved branch also `DECAY_FIT_MAX_REL_ERR`.

The central numerical input of this task is a public reference table that this plan cannot transcribe (no edition of ANSI/ANS-5.1 or the textbook table was readable when the plan was written, and values are never typed from memory). Step 1 is therefore a bounded evidence task whose recorded outcome selects one of two explicit branches; neither branch is silent.

- [ ] **Step 1: Bounded evidence task: identify and record the decay-heat reference**

Time box: one working session, at most two hours. Try the candidates in order and stop at the first that can actually be opened and read page by page; never type values from memory or from a secondary quotation.

1. ANSI/ANS-5.1 (2014, 2005 or 1979 edition): the table of total decay-heat fraction P/P0 for U-235 thermal fission after infinite irradiation (the "infinite operation" / 1e13 s column). A purchased or library copy only.
2. A public NRC document that reproduces the ANS-5.1-1979 total-decay-heat table (search NRC ADAMS for "ANS-5.1" "decay heat" "fraction of operating power"). Use it only if the PDF opens and the table is legible and attributed.
3. Todreas and Kazimi, Nuclear Systems Volume 1, 2nd ed. (S-STD-04, Chapter 3): the tabulated decay-heat values, if the chapter in hand contains a table and not only the 0.066 t^-0.2 approximation.

Record the outcome in `docs/validation/findings.md` as the next free `F-nnn` entry: `F-nnn decay-heat reference: retrieved | unavailable`, with document id, edition/revision, table number, page, retrieval date, sha256 (if a file), and the reference-state definition exactly as the table states it: fissioning nuclide mix (U-235 thermal only; with or without the U-238 capture contribution), irradiation history (infinite or which finite value), normalisation (fraction of operating power), included contributions (fission products only; actinides; neutron capture in fission products), and the stated uncertainty. If a candidate was found but any of those five items is not stated, record `unavailable` with that reason: a table whose reference state is unknown is not a validation reference.

Branch selection for every following step:

- **retrieved**: add the document to `docs/model/sources.csv` (12 columns `id,document_id,title,revision,date,jurisdiction,url,section,retrieval_date,sha256,kind,notes`; `kind` `standard` or `textbook`; `section` = the table identifier; no `UNRETRIEVED` field, P1 rule), then Steps 1b-7 as written.
- **unavailable**: no CSV and no fit. `DECAY_GROUPS` keeps its current values; its P1 row's `notes` becomes `refit deferred: no readable public reference table (F-nnn); fit target remains the 0.066 t^-0.2 approximation (S-STD-04)` (status stays `calibrated`). Steps 3-5 implement only `after_history` and `IRRADIATION_HISTORY_S`; the reference test is written in its `#[ignore]` form (Step 3) so the gap is recorded, never skipped silently; Step 6 uses the unavailable wording.

- [ ] **Step 1b (retrieved branch): Transcribe the table and freeze it**

Create `docs/model/decay-heat-reference.csv` with the header

```
t_s,p_over_p0,source_id,table,page
```

and one row per table point (at least 12; include 1 s, 10 s, 100 s, 1000 s, 1 h, 1 day, 1 week and 30 days when the table has them). `source_id` is the sources.csv id; `table` and `page` identify the table and page. Transcription check: a second pass reads the CSV back against the open document row by row; record `transcription checked <date>, sha256 <value>` (`shasum -a 256 docs/model/decay-heat-reference.csv`) in the F-nnn entry. From this point the CSV is immutable: the fit tool only reads it, and no later step edits a value to improve a fit. A transcription error found later is a new findings entry with the corrected sha256, never a silent edit.

- [ ] **Step 2 (retrieved branch): Rewrite `tools/fit_decay_heat.py`**

```python
"""Fit exponential decay-heat groups to a transcribed public reference curve.

Usage:
    python3 tools/fit_decay_heat.py docs/model/decay-heat-reference.csv [--groups 9] [--max-rel-err 0.05] [--report PATH]

Reads the reference CSV (never writes it). Prints the DECAY_GROUPS block for
crates/bwr-core/src/params.rs on stdout and a point-by-point residual table on
stderr (and to --report when given). Exits 1 if the maximum relative error of
the fit against the reference points exceeds --max-rel-err. An exit of 1 means
the chosen basis cannot represent the reference to the gate; it never means the
reference should be changed.
"""
import argparse
import csv
import sys

import numpy as np
from scipy.optimize import nnls


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("csv", help="reference table with columns t_s,p_over_p0,source_id,table,page")
    ap.add_argument("--groups", type=int, default=9)
    ap.add_argument("--max-rel-err", type=float, default=0.05)
    ap.add_argument("--report", help="also write the residual table to this path")
    args = ap.parse_args()

    t, p, sources = [], [], set()
    with open(args.csv, newline="") as f:
        for row in csv.DictReader(f):
            t.append(float(row["t_s"]))
            p.append(float(row["p_over_p0"]))
            sources.add(f'{row["source_id"]} {row["table"]}')
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

    lines = [f"groups={args.groups}  sum f = {f.sum():.5f}  max rel err = {err:.3%}"]
    for tt, pp, ff in zip(t, p, ref_fit):
        lines.append(f"t={tt:10.0f}  ref={pp:.5f}  fit={ff:.5f}  rel={ff / pp - 1:+.3%}")
    print("\n".join(lines), file=sys.stderr)
    if args.report:
        with open(args.report, "w") as rep:
            rep.write("\n".join(lines) + "\n")
    if err > args.max_rel_err:
        print(f"fit error {err:.2%} exceeds {args.max_rel_err:.0%} with {args.groups} groups (basis limitation; do not edit the reference)", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
```

Run: `python3 tools/fit_decay_heat.py docs/model/decay-heat-reference.csv --groups 9 --report /tmp/decay_fit_9.txt > /tmp/decay_groups.rs; echo exit=$?`
Expected: `exit=0` and `/tmp/decay_groups.rs` holds a 9-row `DECAY_GROUPS` block; `DECAY_FIT_MAX_REL_ERR` is then `0.05` and the `DECAY_GROUPS` row status `derived`.

**Predeclared failure procedure** (`exit=1`): nine fixed log-spaced decay constants are not guaranteed to represent an arbitrary table to 5 %, and an error above 5 % is not evidence of a transcription error. The CSV is not touched. (1) Keep `/tmp/decay_fit_9.txt` (per-point residuals). (2) Rerun with `--groups 11`, then `--groups 13` (the basis changes, not the source; `DecayHeat::d` is `[f64; 9]` today, so a larger count changes that array length and the declared length of `DECAY_GROUPS` together, and `after_history` below uses `DECAY_GROUPS.len()`). (3) If 13 groups still exceed 5 %, take the group count with the smallest maximum error, set `DECAY_FIT_MAX_REL_ERR` to that error rounded up to the next 0.005, set the `DECAY_GROUPS` row status to `calibrated` with the measured error and group count in `notes`, and add a findings entry `F-nnn decay-heat fit: <err> % with <k> groups exceeds the 5 % gate; representation limited` so the limitation is visible wherever decay heat is cited. The Rust test (Step 3) asserts against `DECAY_FIT_MAX_REL_ERR`, so the gate is never loosened silently: the registered number plus the finding is the record. Never edit a reference value, remove a reference point, or adjust the weighting to pass.

- [ ] **Step 3: Write the failing Rust tests**

In `reactor.rs` `mod tests`, replace `decay_heat_curve` with the tests below. In the **unavailable** branch, `decay_reference` is omitted and the first test is written instead as

```rust
    #[test]
    #[ignore = "F-nnn: decay-heat reference unavailable; DECAY_GROUPS remain the 0.066 t^-0.2 fit (S-STD-04)"]
    fn decay_heat_matches_transcribed_reference_within_registered_error() {
        panic!("F-nnn: no readable public decay-heat reference table; see docs/validation/findings.md");
    }
```

(the findings.md convention: recorded and `#[ignore]`d with its id, never deleted). Retrieved branch:

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
    fn decay_heat_matches_transcribed_reference_within_registered_error() {
        // The gate is the registered DECAY_FIT_MAX_REL_ERR: 0.05 when the fit met the
        // 5 % target, or the recorded larger value (status calibrated + findings entry)
        // when the predeclared failure path was taken. Either way the number is on record.
        let r = decay_reference();
        assert!(r.len() >= 12, "reference needs >= 12 points");
        for (t, p) in r {
            let got = DecayHeat::after_shutdown(t).power();
            assert!((got / p - 1.0).abs() < DECAY_FIT_MAX_REL_ERR, "t={t}: fit {got} vs reference {p} (gate {DECAY_FIT_MAX_REL_ERR})");
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

Run: `cargo test --release -p bwr-core --lib decay > /tmp/p5-t6-red.log 2>&1; echo exit=$?; grep -n "no function\|cannot find" /tmp/p5-t6-red.log`
Expected: `exit=101`; compile error `no function or associated item named after_history`.

- [ ] **Step 4: Implement, and register every constant touched (same task)**

`params.rs`, retrieved branch: replace the existing `DECAY_GROUPS` doc comment and array with the contents of `/tmp/decay_groups.rs` and append

```rust
/// Maximum relative error of the DECAY_GROUPS fit at the reference points, as measured
/// by tools/fit_decay_heat.py (0.05 when the 5 % gate was met; otherwise the recorded
/// value with status calibrated and a findings entry). DERIVED.
pub const DECAY_FIT_MAX_REL_ERR: f64 = 0.05;
```

Unavailable branch: `DECAY_GROUPS` is untouched. Both branches append:

```rust
/// Irradiation time at rated power assumed before each scenario's cooling period, s (2 years). ASSUMED (A-052).
pub const IRRADIATION_HISTORY_S: f64 = 6.31e7;
```

`docs/model/parameters.csv`, both branches, append:

```
IRRADIATION_HISTORY_S,6.31e7,s,Irradiation time at rated power assumed before each scenario cooling period (2 years),scenario init,BWRX-300,assumed,,3.0e7,1.3e8,A-052,crates/bwr-core/src/params.rs,used by cold_shutdown and hot_standby via DecayHeat::after_history
```

Retrieved branch, also: update P1's `DECAY_GROUPS` row in place to `DECAY_GROUPS,<k> pairs (see params.rs),fraction;1/s,Decay-heat group amplitudes and decay constants,<reference state from the F-nnn entry>,reference table,<derived | calibrated>,<new source id>,,,A-053,params.rs,fit by tools/fit_decay_heat.py to decay-heat-reference.csv (sha256 <value>); max rel err <value> with <k> groups; F-nnn` and append

```
DECAY_FIT_MAX_REL_ERR,<0.05 or the recorded value>,-,Maximum relative error of the DECAY_GROUPS fit at the reference points,reference table points,derived,derived,,,,A-053,crates/bwr-core/src/params.rs,measured by tools/fit_decay_heat.py on <date>; gate of the decay-heat reference test
```

Unavailable branch, also: update the `DECAY_GROUPS` row `notes` as Step 1 states (F-nnn, refit deferred). Then `cargo test --release -p bwr-core --test register` (expected: all `ok`).

`reactor.rs`, in `impl DecayHeat`, add after `after_shutdown`:

```rust
    /// Decay heat after operating `irr_s` seconds at relative power `n`, then
    /// cooling for `cooled_s` seconds. `irr_s = f64::INFINITY` reproduces `after_shutdown`.
    pub fn after_history(n: f64, irr_s: f64, cooled_s: f64) -> Self {
        let mut d = [0.0; DECAY_GROUPS.len()];
        for (i, (f, l)) in DECAY_GROUPS.iter().enumerate() {
            d[i] = f * n * (1.0 - (-l * irr_s).exp()) * (-l * cooled_s).exp();
        }
        Self { d }
    }
```

(`DecayHeat.d` is declared `[f64; 9]` at the baseline; if the predeclared failure path changed the group count, change that declaration to `[f64; DECAY_GROUPS.len()]` in the same commit.)

`sim.rs`: in `cold_shutdown()` replace `core.decay = DecayHeat::after_shutdown(30.0 * 86400.0);` with `core.decay = DecayHeat::after_history(1.0, IRRADIATION_HISTORY_S, 30.0 * 86400.0);` and in `hot_standby()` replace `DecayHeat::after_shutdown(2.0 * 86400.0)` with `DecayHeat::after_history(1.0, IRRADIATION_HISTORY_S, 2.0 * 86400.0)`.

- [ ] **Step 5: Run tests**

Run: `set -o pipefail; cargo test --release -p bwr-core 2>&1 | tee /tmp/p5-t6.log | grep -E "^test |test result"`
Expected: exit 0; all `ok` (the unavailable branch shows the reference test as `ignored`). If `plant::cold_shutdown_is_subcritical_and_rods_blocked` or `plant::startup_from_hot_standby_to_30_percent` changes outcome, the cause is the refit, not the history term (the history term lowers decay heat by < 10 %); investigate the refit before touching assertions.

- [ ] **Step 6: Wording in core.md and README**

Complete `docs/model/core.md` section 6 with the branch taken: the F-nnn id, the source id and table, the reference state, the group count and the measured maximum relative error with its gate (retrieved), or the sentence "Reference unavailable (F-nnn): the groups remain the fit to the 0.066 t^-0.2 approximation (S-STD-04); the 5 % gate is not claimed" (unavailable).

In `README.md` table row "Decay heat", replace `9-group fit to the standard decay heat curve (7.7 % at shutdown, 1.3 % after 1 h)` with, retrieved: `<k>-group fit to a transcribed public decay-heat table (docs/model/decay-heat-reference.csv; max error <value>), with finite irradiation history`; unavailable: `9-group fit to the 0.066 t^-0.2 approximation (no public reference table retrieved, see docs/validation/findings.md), with finite irradiation history`. In the "Fuel" row replace `core damage above 1200 °C` with `an explicit model envelope flag instead of a damage verdict`.

- [ ] **Step 7: Commit**

```bash
git add docs/validation/findings.md docs/model/sources.csv docs/model/parameters.csv docs/model/core.md tools/fit_decay_heat.py crates/bwr-core/src/params.rs crates/bwr-core/src/reactor.rs crates/bwr-core/src/sim.rs README.md
git add docs/model/decay-heat-reference.csv 2>/dev/null || true   # retrieved branch only
git commit -m "feat(core): decay heat reference evidence task, fit gate, finite irradiation history

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

Run: `set -o pipefail; cargo test --release -p bwr-core --lib xenon 2>&1 | tee /tmp/p5-t7.log | grep -E "^test |test result"`
Expected: exit 0; 4 xenon tests `ok`. If `power_rise_from_50_to_100_percent_dips_xenon_first` fails, the model's burnout term is wrong; do not change the test threshold, investigate `Xenon::step`.

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

Run: `cargo test --release -p bwr-core --test core_transients coeffs > /tmp/p5-t8-red.log 2>&1; echo exit=$?; grep -n "cannot find" /tmp/p5-t8-red.log`
Expected: `exit=101`; compile error `cannot find type ReactivityCoeffs`.

- [ ] **Step 3: Implement `ReactivityCoeffs`**

In `reactor.rs`, before `pub struct Core`, add:

```rust
/// Feedback coefficients. Defaults are the calibrated `params.rs` values; a
/// sensitivity study builds a plant with modified values via `Sim::full_power_with`.
/// Derives per the master Derives rule: Clone, Copy, Debug (Default is manual).
#[derive(Clone, Copy, Debug)]
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

Run: `set -o pipefail; cargo test --release -p bwr-core 2>&1 | tee /tmp/p5-t8.log | grep -E "^test |test result"`
Expected: exit 0; all `ok`; the `full_power_rod_insertion_fraction` test prints the fraction (run with `-- --nocapture` to see it) and it sits inside 5-25 %.

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
- Consumes: `docs/model/parameters.csv` row `SDM_REQUIRED` (Task 1); core.md section 4 for the threshold's applicability (strongest-single-rod-out, cold xenon-free only) and the paired-rod case (`unavailable`).

- [ ] **Step 1: Write the failing tests**

Append to `crates/bwr-core/tests/core_transients.rs`:

```rust
/// Outcome of reading the SDM_REQUIRED row (locked header; row by `id`, columns by name).
#[derive(Debug, PartialEq)]
enum SdmRequirement {
    Confirmed(f64),
    Unavailable,
}

/// Strict reader: any I/O, header, row or value problem is an Err (and a test failure),
/// never a silent downgrade. Only a well-formed `unavailable` row yields `Unavailable`.
/// Rows read here must not contain quoted commas (P1 shape rule).
fn parse_sdm_required(text: &str) -> Result<SdmRequirement, String> {
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().ok_or("empty parameters.csv")?.split(',').map(str::trim).collect();
    let col = |name: &str| header.iter().position(|h| *h == name).ok_or_else(|| format!("missing column {name}"));
    let (id_i, value_i, status_i) = (col("id")?, col("value")?, col("status")?);
    let rows: Vec<Vec<&str>> = lines
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.split(',').map(str::trim).collect::<Vec<&str>>())
        .filter(|c| c.get(id_i) == Some(&"SDM_REQUIRED"))
        .collect();
    let row = match rows.as_slice() {
        [row] => row,
        [] => return Err("no SDM_REQUIRED row".into()),
        _ => return Err(format!("{} SDM_REQUIRED rows", rows.len())),
    };
    if row.len() != header.len() {
        return Err(format!("SDM_REQUIRED row has {} fields, header has {}", row.len(), header.len()));
    }
    let (value, status) = (row[value_i], row[status_i]);
    match status {
        "confirmed" => {
            let v: f64 = value.parse().map_err(|e| format!("confirmed SDM_REQUIRED value {value:?} unparseable: {e}"))?;
            if !(v.is_finite() && v > 0.0) {
                return Err(format!("confirmed SDM_REQUIRED value {v} is not a positive finite margin"));
            }
            Ok(SdmRequirement::Confirmed(v))
        }
        "unavailable" => {
            if !value.is_empty() {
                return Err(format!("unavailable SDM_REQUIRED row carries a value {value:?}"));
            }
            Ok(SdmRequirement::Unavailable)
        }
        other => Err(format!("SDM_REQUIRED status {other:?} is neither confirmed nor unavailable")),
    }
}

fn sdm_required() -> SdmRequirement {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/model/parameters.csv");
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
    parse_sdm_required(&text).unwrap_or_else(|e| panic!("parameters.csv SDM_REQUIRED: {e}"))
}

/// Gate logic, separated so it can be tested without the real register. `governed` says
/// whether SDM_REQUIRED applies to the configuration (core.md section 4: the cold
/// xenon-free strongest-single-rod-out case only). A confirmed requirement is applied
/// to every governed case and never falls back to positivity; ungoverned cases and the
/// unavailable requirement check positivity only.
fn sdm_check(req: &SdmRequirement, governed: bool, sdm: f64) -> Result<&'static str, String> {
    if !(sdm.is_finite() && sdm > 0.0) {
        return Err(format!("SDM {sdm} not a positive finite margin"));
    }
    match (req, governed) {
        (SdmRequirement::Confirmed(r), true) if sdm < *r => Err(format!("SDM {sdm} below sourced requirement {r}")),
        (SdmRequirement::Confirmed(_), true) => Ok("meets sourced requirement"),
        (SdmRequirement::Confirmed(_), false) => Ok("positive; sourced requirement not applicable to this configuration"),
        (SdmRequirement::Unavailable, _) => Ok("positive; SDM_REQUIRED unavailable"),
    }
}

fn assert_sdm(label: &str, sdm: f64, governed: bool) {
    match sdm_check(&sdm_required(), governed, sdm) {
        Ok(note) => println!("SDM {label}: {:.4} dk/k ({:.0} pcm): {note}", sdm, sdm * 1e5),
        Err(e) => panic!("{label}: {e}"),
    }
}

#[test]
fn sdm_reader_fails_loudly_on_missing_or_malformed_rows() {
    let hdr = "id,value,unit,definition,reference_state,source_design,status,source_ids,interval_lo,interval_hi,assumption_id,location,notes";
    assert_eq!(parse_sdm_required(&format!("{hdr}\nSDM_REQUIRED,0.0038,dk/k,d,r,s,confirmed,S-08,,,,loc,n")), Ok(SdmRequirement::Confirmed(0.0038)));
    assert_eq!(parse_sdm_required(&format!("{hdr}\nSDM_REQUIRED,,dk/k,d,r,s,unavailable,,none,none,,loc,n")), Ok(SdmRequirement::Unavailable));
    let bad: Vec<(String, &str)> = vec![
        (String::new(), "empty file"),
        (hdr.to_string(), "header only"),
        (format!("{hdr}\nOTHER,1,u,d,r,s,confirmed,,,,,l,n"), "missing row"),
        (format!("{hdr}\nSDM_REQUIRED,0.004,u,d,r,s,confirmed,,,,,l,n\nSDM_REQUIRED,0.004,u,d,r,s,confirmed,,,,,l,n"), "duplicate row"),
        (format!("{hdr}\nSDM_REQUIRED,abc,u,d,r,s,confirmed,,,,,l,n"), "unparseable confirmed value"),
        (format!("{hdr}\nSDM_REQUIRED,NaN,u,d,r,s,confirmed,,,,,l,n"), "non-finite confirmed value"),
        (format!("{hdr}\nSDM_REQUIRED,-0.01,u,d,r,s,confirmed,,,,,l,n"), "negative confirmed value"),
        (format!("{hdr}\nSDM_REQUIRED,,u,d,r,s,confirmed,,,,,l,n"), "empty confirmed value"),
        (format!("{hdr}\nSDM_REQUIRED,0.004,u,d,r,s,assumed,,,,,l,n"), "unexpected status"),
        (format!("{hdr}\nSDM_REQUIRED,0.004,u,d,r,s,unavailable,,none,none,,l,n"), "unavailable row with a value"),
        (format!("{hdr}\nSDM_REQUIRED,0.004,u,d,r,s,confirmed,,,,l"), "short row"),
        ("id,unit,status\nSDM_REQUIRED,u,confirmed".to_string(), "missing value column"),
    ];
    for (text, why) in &bad {
        let got = parse_sdm_required(text);
        assert!(got.is_err(), "{why}: must be an error, got {got:?}");
    }
}

#[test]
fn confirmed_requirement_never_falls_back_to_positivity() {
    let req = SdmRequirement::Confirmed(0.0038);
    assert!(sdm_check(&req, true, 0.0030).is_err(), "below a confirmed requirement must fail");
    assert!(sdm_check(&req, true, 0.0040).is_ok());
    assert!(sdm_check(&req, false, 0.0030).is_ok(), "ungoverned configurations are positivity-only");
    assert!(sdm_check(&SdmRequirement::Unavailable, true, 0.0001).is_ok());
    for bad in [0.0, -0.01, f64::NAN, f64::INFINITY] {
        assert!(sdm_check(&SdmRequirement::Unavailable, false, bad).is_err(), "{bad}");
    }
    // The real register must parse: a corrupted or missing row is a failure, not None.
    let _ = sdm_required();
}

/// Independent construction of a configuration: explicit rod positions evaluated through
/// the production reactivity path, compared with `shutdown_margin`.
fn sdm_from_positions(withdrawn: &[usize], rho_other: f64) -> f64 {
    let mut r = Rods::new(0.0);
    for &i in withdrawn {
        r.pos[i] = ROD_FULL_OUT;
    }
    -(RHO_EXCESS_COLD + rho_other + r.reactivity())
}

fn rods_by_descending_weight(rods: &Rods) -> Vec<usize> {
    let mut order: Vec<usize> = (0..N_RODS).collect();
    order.sort_by(|&a, &b| rods.weights[b].partial_cmp(&rods.weights[a]).unwrap());
    order
}

#[test]
fn shutdown_margin_agrees_with_explicit_rod_configurations() {
    let rods = Rods::new(0.0);
    let strongest = rods.strongest_rod();
    let order = rods_by_descending_weight(&rods);
    assert_eq!(order[0], strongest);
    let configs: Vec<(&str, Vec<usize>)> = vec![
        ("all in", vec![]),
        ("strongest out", vec![strongest]),
        ("worst two out", vec![order[0], order[1]]),
        ("weakest two out", vec![order[N_RODS - 1], order[N_RODS - 2]]),
    ];
    for (label, w) in &configs {
        for rho_other in [0.0, -0.02, 0.01] {
            let a = rods.shutdown_margin(rho_other, w);
            let b = sdm_from_positions(w, rho_other);
            assert!((a - b).abs() < 1e-12, "{label} rho_other {rho_other}: shutdown_margin {a} vs explicit positions {b}");
        }
    }
    assert!(rods.shutdown_margin(0.0, &[strongest]) < rods.shutdown_margin(0.0, &[]), "withdrawing the strongest rod must reduce the margin");
    assert!(rods.shutdown_margin(0.0, &[order[0], order[1]]) < rods.shutdown_margin(0.0, &[strongest]));
    assert!(rods.shutdown_margin(0.0, &[order[N_RODS - 1], order[N_RODS - 2]]) > rods.shutdown_margin(0.0, &[order[0], order[1]]));
}

#[test]
fn shutdown_margin_cold_xenon_free() {
    let rods = Rods::new(0.0);
    let (d, v, m) = ReactivityCoeffs::default().feedback(20.0, 0.0, 20.0);
    let rho_other = d + v + m; // xenon-free: no xenon term
    assert!(rho_other.abs() < 1e-12, "cold clean feedback should be zero, got {rho_other}");
    assert_sdm("all rods in (not the governed case)", rods.shutdown_margin(rho_other, &[]), false);
    let strongest = rods.strongest_rod();
    assert_sdm("strongest rod out (governed by SDM_REQUIRED when confirmed)", rods.shutdown_margin(rho_other, &[strongest]), true);
    let order = rods_by_descending_weight(&rods);
    // Generic worst-two-worth sensitivity on the assumed grid. NOT the plant paired-drive
    // case: the BWRX-300 pair topology is not public (core.md section 4, findings F-nnn).
    assert_sdm("worst two rods out (model sensitivity; paired-drive case unavailable)", rods.shutdown_margin(rho_other, &[order[0], order[1]]), false);
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

Run: `cargo test --release -p bwr-core --test core_transients shutdown_margin > /tmp/p5-t9-red.log 2>&1; echo exit=$?; grep -n "no method" /tmp/p5-t9-red.log`
Expected: `exit=101`; compile error `no method named shutdown_margin`.

- [ ] **Step 3: Implement, and record the paired-rod criterion**

Add to `docs/validation/findings.md` the next free entry: `F-nnn SDM paired-rod case: unavailable. The pair-selection criterion (which FMCRDs share a hydraulic control unit or scram path, and therefore which pair the "applicable paired-rod unavailable" case means) is not stated in the public BWRX-300 documents reviewed (S-06; S-08 <sections checked>). The tests report a generic worst-two-worth sensitivity on the assumed grid, labelled as model evidence; no threshold is applied to it.` Replace `F-nnn` in the test comment above with the allocated id.

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

Run: `set -o pipefail; cargo test --release -p bwr-core --test core_transients sdm shutdown_margin confirmed_requirement -- --nocapture 2>&1 | tee /tmp/p5-t9.log | grep -E "SDM|^test |test result"`
Expected: exit 0; three `SDM ...` lines printed with their applicability notes (all-in about 0.07 dk/k; strongest-rod-out and worst-two-out smaller but positive), and all five tests `ok` (`sdm_reader_fails_loudly_on_missing_or_malformed_rows`, `confirmed_requirement_never_falls_back_to_positivity`, `shutdown_margin_agrees_with_explicit_rod_configurations`, `shutdown_margin_cold_xenon_free`, `shutdown_margin_hot_with_equilibrium_xenon_is_larger_than_cold`).

- [ ] **Step 5: Commit**

```bash
git add crates/bwr-core/src/reactor.rs crates/bwr-core/tests/core_transients.rs docs/validation/findings.md
git commit -m "feat(core): shutdown margin evidence (all-in, strongest-rod-out, worst-two sensitivity) with strict register gate

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
//! Every variant carries a support status; only `supported` rows are comparable, and
//! unsupported rows print their reason instead of numbers.

use bwr_core::api::Command;
use bwr_core::reactor::ReactivityCoeffs;
use bwr_core::sim::DT;
use bwr_core::Sim;

const SETTLE_S: f64 = 5.0;
const OBSERVE_S: f64 = 120.0;
/// Boundary-state steadiness limits: |n - 1| and |rho_total| after settling.
const N_RESIDUAL_MAX: f64 = 0.01;
const RHO_RESIDUAL_MAX: f64 = 5e-5;

#[derive(Debug)]
enum Status {
    /// Steady boundary state, no domain error retained from initialisation or raised
    /// during the run, every sampled metric finite at every step.
    Supported,
    /// Domain error retained from initialisation or raised during the run.
    Unsupported { at_s: f64, reason: String },
    /// A sampled metric became non-finite.
    NonFinite { at_s: f64 },
    /// Boundary state not steady.
    NotSteady { n_residual: f64, rho_residual: f64 },
}

struct Outcome {
    status: Status,
    n_residual: f64,
    rho_residual: f64,
    peak_power: f64,
    peak_power_t: f64,
    peak_pressure_mpa: f64,
    min_level_cm: f64,
    scrammed: bool,
    final_power: f64,
}

fn turbine_trip(coeffs: ReactivityCoeffs) -> Outcome {
    let mut sim = Sim::full_power_with(coeffs);
    let retained_init_errors = sim.domain_errors.clone();
    for _ in 0..(SETTLE_S / DT).round() as usize {
        sim.step(DT);
    }
    // Boundary state: residuals recorded, extrema seeded from the actual values (never zeros).
    let n_residual = (sim.core.kin.n - 1.0).abs();
    let rho_residual = sim.core.rho_total.abs();
    let mut o = Outcome {
        status: Status::Supported,
        n_residual,
        rho_residual,
        peak_power: sim.core.kin.n,
        peak_power_t: sim.time,
        peak_pressure_mpa: sim.vessel.eos.p / 1e6,
        min_level_cm: sim.vessel.level_nr_cm(),
        scrammed: sim.scrammed,
        final_power: f64::NAN,
    };
    if let Some((t, e)) = retained_init_errors.first().or(sim.domain_errors.first()) {
        o.status = Status::Unsupported { at_s: *t, reason: e.reason() };
        return o;
    }
    if n_residual > N_RESIDUAL_MAX || rho_residual > RHO_RESIDUAL_MAX {
        o.status = Status::NotSteady { n_residual, rho_residual };
        return o;
    }
    sim.apply(Command::TripTurbine);
    for _ in 0..(OBSERVE_S / DT).round() as usize {
        sim.step(DT);
        if let Some((t, e)) = sim.domain_errors.first() {
            o.status = Status::Unsupported { at_s: *t, reason: e.reason() };
            return o;
        }
        let (n, p, l) = (sim.core.kin.n, sim.vessel.eos.p / 1e6, sim.vessel.level_nr_cm());
        if !(n.is_finite() && p.is_finite() && l.is_finite()) {
            o.status = Status::NonFinite { at_s: sim.time };
            return o;
        }
        if n > o.peak_power {
            o.peak_power = n;
            o.peak_power_t = sim.time;
        }
        o.peak_pressure_mpa = o.peak_pressure_mpa.max(p);
        o.min_level_cm = o.min_level_cm.min(l);
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
    println!(
        "{:<14} {:>11} {:>8} {:>8} {:>9} {:>9} {:>6} {:>8}  {}",
        "variant", "status", "peak n", "t_peak", "peak MPa", "min lvl", "scram", "final n", "residuals / reason"
    );
    let mut excluded = 0;
    for (name, c) in variants {
        let o = turbine_trip(c);
        match &o.status {
            Status::Supported => println!(
                "{:<14} {:>11} {:8.3} {:8.2} {:9.3} {:9.1} {:>6} {:8.3}  n_res {:.1e} rho_res {:.1e}",
                name, "supported", o.peak_power, o.peak_power_t, o.peak_pressure_mpa, o.min_level_cm, o.scrammed, o.final_power, o.n_residual, o.rho_residual
            ),
            other => {
                excluded += 1;
                println!("{:<14} {:>11} {:>8} {:>8} {:>9} {:>9} {:>6} {:>8}  {:?}", name, "UNSUPPORTED", "-", "-", "-", "-", "-", "-", other);
            }
        }
    }
    println!("Coefficients are calibrated/assumed (docs/model/core.md section 3); this table is a sensitivity, not plant data.");
    println!("Only `supported` rows are comparable; {excluded} variant(s) excluded from comparison.");
}
```

- [ ] **Step 2: Run**

Run: `cargo run --release -p bwr-core --example sensitivity > /tmp/p5-sensitivity.txt 2>&1; echo exit=$?; cat /tmp/p5-sensitivity.txt`
Expected: `exit=0`; a 5-row table with a `status` column; the `nominal` row is `supported`, shows `scram false` and peak pressure below 7.4 MPa (consistent with `plant::turbine_trip_rides_through_on_bypass`), and prints its boundary residuals. A `nominal` row that is not `supported` is a defect in `full_power_with`: fix it, do not proceed. A +/-25 % variant that prints `UNSUPPORTED` is a legitimate finding: keep the row exactly as printed (its reason and time), record it in findings.md, and do not alter the variant or the limits to make it supported. The whole table, status column included, is what Task 11 copies into core.md.

- [ ] **Step 3: Commit**

```bash
git add crates/bwr-core/examples/sensitivity.rs
git commit -m "feat(core): reactivity coefficient sensitivity example for the turbine-trip transient

<paste /tmp/p5-sensitivity.txt here, status column included>

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 11: Baseline regeneration (master protocol), register closure, and final verification

**Files:**
- Create: `docs/validation/reports/<date>-pkg05.md`
- Modify: `docs/validation/baseline/*.csv`, `docs/model/coverage.md`, `docs/model/core.md`

**Interfaces:**
- Consumes: P2's `examples/validation.rs` (compare mode prints `PASS <case>` / `FAIL <case>` with one indented violation line per metric, the metrics, and a `convergence dt/2: N violation(s)` line per case; `--write` rewrites the baselines; see `docs/validation/method.md`).

Master baseline regeneration protocol, in this order and nothing skipped: (1) compare-mode output saved complete to the report; (2) every changed metric classified; (3) the package's independent physical gates and the contract gates green BEFORE regeneration; (4) `--write` exactly once for the reviewed change set; (5) report and baselines committed together; then compare mode verified without any tolerance change. Each step below runs cargo unpiped or with `pipefail` so no failure is masked.

- [ ] **Step 1: Independent gates before any regeneration**

```bash
set -o pipefail; cargo test --release -p bwr-core 2>&1 | tee /tmp/p5-t11-tests.log | grep -E "test result"
npm --prefix ui run typecheck
npm --prefix ui run build > /tmp/p5-t11-build.log 2>&1; echo build_exit=$?
```
Expected: every `test result:` line `ok` with exit 0 (this is where the package's physical gates run: `two_node_energy_identity_holds_each_step`, `two_node_transient_matches_an_independent_rk4_reference`, `two_node_update_is_stable_and_converges_for_every_step_size`, `rated_fuel_and_clad_temperatures_match_resistance_derivation`, the decay-heat reference test in the retrieved branch, the xenon tests, `shutdown_margin_agrees_with_explicit_rod_configurations`, the envelope tests, and P3's `conservation` tests); typecheck silent; `build_exit=0`. Stop on any failure: nothing is regenerated while a gate is red.

- [ ] **Step 2: Compare mode, complete output saved to the report**

```bash
mkdir -p docs/validation/reports
D=$(date +%Y-%m-%d)
cargo run --release -p bwr-core --example validation > /tmp/p5-compare.log 2>&1; echo compare_exit=$?
{ echo "# Package 5 baseline regeneration report ($D)"; echo; echo "## Compare mode before regeneration (complete output)"; echo; echo '```'; cat /tmp/p5-compare.log; echo '```'; } > "docs/validation/reports/$D-pkg05.md"
```
Expected: a non-zero `compare_exit` is normal at this point (cases that moved fail compare); the report holds every `PASS`/`FAIL` line and every violation line untruncated.

- [ ] **Step 3: Classify every changed metric**

Append to the report a section `## Classification` with one table row per violation line in the compare output (`case | metric | baseline | new | class | reason`). `class` is exactly one of:

- `physical`: the reason names the mechanism: two-node thermal response (`fuel_t_c` and the first seconds of any scram transient); electric run-in plus the rods-before-kinetics order in the first seconds after a scram; decay-heat level in `cold_shutdown`/`hot_standby` from the finite irradiation history (both branches) and from the refit (retrieved branch only). The stuck-rod and scram re-assert changes have no effect without a fault and must not appear here.
- `schema-population`: `clad_t_c`, `rods_min_pos`, `rods_max_pos` newly populated (byte-identical traces are not expected).
- `numerical`: within the `convergence dt/2` band printed for that case in the same output.

A violation that fits none of these is a defect: stop, fix it in a new commit, and restart this task from Step 1. No tolerance is adjusted at any point.

- [ ] **Step 4: Regenerate exactly once, then verify compare mode**

```bash
cargo run --release -p bwr-core --example validation -- --write > /tmp/p5-write.log 2>&1; echo write_exit=$?
cargo run --release -p bwr-core --example validation > /tmp/p5-compare-after.log 2>&1; echo compare_after_exit=$?
grep -c "^PASS" /tmp/p5-compare-after.log; grep -c "^FAIL" /tmp/p5-compare-after.log
```
Expected: `write_exit=0`; `compare_after_exit=0`; the PASS count equals the number of standard cases and the FAIL count is 0 with no tolerance changed. Append `## After regeneration` with both counts and the `convergence dt/2` lines to the report. `--write` runs exactly once for this reviewed change set; if any later commit in this package changes physics, the whole task restarts from Step 1.

- [ ] **Step 5: Close the register and the model record**

```bash
for n in Q_FUEL_RATED GAP_RESISTANCE_K_PER_W CLAD_COOLANT_RESISTANCE_K_PER_W CLAD_HEAT_CAP_J_PER_K FUEL_MODEL_T_MAX_C UNCOVERED_ENVELOPE_S CORE_COVERED_FRAC IRRADIATION_HISTORY_S SDM_REQUIRED DECAY_GROUPS; do grep -q "^$n," docs/model/parameters.csv && echo "ok $n" || echo "MISSING $n"; done
cut -d, -f1 docs/model/parameters.csv | sort | uniq -d
cargo test --release -p bwr-core --test register
```
Expected: ten `ok` lines (add `DECAY_FIT_MAX_REL_ERR` to the loop in the retrieved branch: eleven), no duplicate ids, every `register` test `ok`. Every row was written in its owning task (1, 4, 5, 6); nothing is added here.

In `docs/model/coverage.md`, update the core/fuel/rods row: evidence = core.md; tests = `core_transients.rs` (list the test names) and the reactor unit tests; limitations = "no axial/channel model, no exposure dependence, no PCT/CPR, rod speeds assumed, SDM requirement unavailable unless sourced, paired-rod case unavailable, decay-heat reference <retrieved with max error ... | unavailable (F-nnn)>".

Append to `docs/model/core.md` a "Measured results (Package 5)" section with: the rod insertion fraction printed by `full_power_rod_insertion_fraction`; the three SDM lines exactly as printed (each with its applicability note); the sensitivity table from `/tmp/p5-sensitivity.txt` including the status column; and the decay-heat outcome (branch, group count, measured maximum relative error and gate, or the unavailable finding).

- [ ] **Step 6: Obsolete-symbol and wording checks (separate checks, source paths only)**

```bash
rg -n "pub fast_insert|pub scramming: bool|pub target: Vec<f64>|pub const FUEL_RESISTANCE" crates/bwr-core/src; echo "decl_exit=$?"
rg -n "rods\.target\[|rods\.target\.|rods\.target,|rods\.target\)|rods\.target;|rods\.scramming[^(]|fast_insert" crates/bwr-core/src crates/bwr-core/tests crates/bwr-core/examples ui/src; echo "access_exit=$?"
rg -n "FUEL_RESISTANCE" crates/bwr-core/src crates/bwr-core/tests crates/bwr-core/examples --glob '!params.rs'; echo "resistance_exit=$?"
rg -n "DAMAGE" crates/bwr-core/src ui/src README.md; echo "visible_exit=$?"
```
Expected: every `*_exit=1` (no hits). Declarations and field accesses are checked separately from the method calls `rods.target(i)`, `rods.targets()` and `rods.scramming()`, which are valid. `FUEL_RESISTANCE` is allowed only inside `params.rs` doc comments (the historical note on `CLAD_COOLANT_RESISTANCE_K_PER_W`), hence the glob. The upper-case `DAMAGE` check is the user-visible wording gate; lower-case "not a damage verdict" comments are allowed.

- [ ] **Step 7: Commit the report and the baselines together**

```bash
git add "docs/validation/reports/$D-pkg05.md" docs/validation/baseline docs/model/coverage.md docs/model/core.md
git commit -m "chore(validation): regenerate baselines after Package 5 core refinements

Report: docs/validation/reports/$D-pkg05.md (compare output, classification, after-regeneration counts)

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
| Hydraulic scram, SRI, motor run-in as distinct mechanisms; ARI reuses them (P8, `ARI_ENABLED = false`) | 2, 3 |
| All-rods-in, strongest-rod, paired-rod, cold xenon-free SDM; sourced margin targets | 9 |
| Stuck-rod fault honoured (master `Fault::RodStuck`) | 3 |
| Trace schema v2 columns `clad_t_c`, `rods_min_pos`, `rods_max_pos` populated | 2, 4 |
| Register, assumptions, applicability, baselines | 1, 4, 5, 6 (rows in owning tasks), 11 (report + baselines) |

**Acceptance gate (verbatim):** Supported reactivity and shutdown behavior has reproducible evidence. Uncertain rod worth, feedback, and fuel limits cannot silently appear as confirmed plant performance.

---

## Master amendments

Items this plan fixes beyond the master's text. The master dt rule already permits a package to add `last_error` to its own void-returning step owner, so `Rods.last_error` needs no amendment. The following should be appended to the master's `### P5: core, fuel, rods` section when the master is next revised (this plan does not edit the master):

1. In the `Rods` struct comment, add `pub last_error: Option<DomainError>, // set by the void-returning step()/set_motor_target(); forwarded into Core.last_error by Core::step_rods`.
2. Add the method `pub fn refresh_stuck(&mut self, mask: &[bool]); // Sim step 2: stuck rods are parked Idle (outstanding command dropped, TGT = POS)`.
3. Change the `scram_hydraulic` comment to `every non-stuck rod with pos > 0 -> Scram (Sri rods become Scram; rods already in stay Idle)` and the `motor_run_in` comment to `every non-stuck rod with pos > 0 not already Scram -> MotorRunIn`.
4. After the sentence about `Sim::scram()`, add: `While Sim.scrammed is latched, Sim::step re-asserts rods.scram_hydraulic() and rods.motor_run_in() after the stuck-mask refresh (step 2), so a rod freed from Fault::RodStuck resumes insertion; nothing is replayed after ResetScram.`
5. Add to the params.rs line: `CORE_COVERED_FRAC = 0.999 (shared by the uncovered-envelope timer and the low-coverage alarm)`, `Q_FUEL_RATED = RATED_POWER_W * (1 - DIRECT_COOLANT_HEAT)` (pub, registered), and, retrieved branch only, `DECAY_FIT_MAX_REL_ERR`.
6. Add `Sim.clad_t_step_max: f64` (max of entry and exit cladding temperature of the last plant step; the envelope check reads it, not `core.clad_t`).
