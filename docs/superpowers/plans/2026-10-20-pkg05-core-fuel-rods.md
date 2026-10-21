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
| `crates/bwr-core/src/api.rs` | Modify | `clad_temp_c`, `fuel_envelope_exceeded`; remove `core_damage` |
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
shared concept with ESBWR (DCD Rev. 10 Tier 2 Ch. 4.6, see
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
/// envelope. An indication, not a damage limit (A-050). ASSUMED.
pub const FUEL_MODEL_T_MAX_C: f64 = 1000.0;
/// Continuous core-uncovered time after which the thermal model is outside
/// its envelope (no post-dryout heat transfer modelled). ASSUMED (A-050).
pub const UNCOVERED_ENVELOPE_S: f64 = 60.0;
```

- [ ] **Step 4: Engine changes in `sim.rs`**

Remove the field `pub core_damage: bool,` from `Sim` and its initialiser `core_damage: false,` in `base()`. Add the field

```rust
    /// Continuous time the core has been uncovered, s (envelope timer).
    pub uncovered_s: f64,
```

initialised to `0.0` in `base()`.

Add to `impl Sim` (P3's public `Sim::domain_error(e)` records the first occurrence of each variant and logs it; this package only adds the query and the test hook):

```rust
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
            self.domain_error(DomainError::FuelModelEnvelope { fuel_t_c: self.core.fuel_t, clad_t_c: self.core.clad_t });
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
/// Irradiation time at rated power assumed before each scenario's cooling period, s (2 years). ASSUMED (A-052).
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
/// `SDM_REQUIRED` from parameters.csv (locked header; row found by `id`, columns by name) if its
/// status is `confirmed`; None otherwise. Rows read here must not contain quoted commas.
fn sdm_required() -> Option<f64> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/model/parameters.csv");
    let text = std::fs::read_to_string(path).ok()?;
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next()?.split(',').map(|s| s.trim()).collect();
    let id_i = header.iter().position(|h| *h == "id")?;
    let value_i = header.iter().position(|h| *h == "value")?;
    let status_i = header.iter().position(|h| *h == "status")?;
    for line in lines {
        let cols: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
        if cols.get(id_i) == Some(&"SDM_REQUIRED") && cols.get(status_i) == Some(&"confirmed") {
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
- Consumes: P2's `examples/validation.rs` with its `--write` flag (see `docs/validation/method.md`).

- [ ] **Step 1: Compare before and after**

Run: `cargo run --release -p bwr-core --example validation 2>&1 | tail -40`
Expected: comparison violations only in metrics explained by this package (fuel temperature columns, early-second power after scram due to the run-in and rod-order change, decay-heat levels after the refit). Record each violated metric, its baseline value, and its new value in a table.

- [ ] **Step 2: Regenerate baselines**

Run: `cargo run --release -p bwr-core --example validation -- --write` (P2's baseline-writing flag, documented in `docs/validation/method.md`).
Expected: baselines rewritten; a second plain run reports zero violations.

- [ ] **Step 3: Close the register**

Verify every constant touched in this package has a `parameters.csv` row: run

```bash
for n in GAP_RESISTANCE_K_PER_W CLAD_COOLANT_RESISTANCE_K_PER_W CLAD_HEAT_CAP_J_PER_K FUEL_MODEL_T_MAX_C UNCOVERED_ENVELOPE_S IRRADIATION_HISTORY_S SDM_REQUIRED DECAY_GROUPS; do grep -q "^$n," docs/model/parameters.csv && echo "ok $n" || echo "MISSING $n"; done
cut -d, -f1 docs/model/parameters.csv | sort | uniq -d
```

Expected: eight `ok` lines and no duplicate ids. Update the `DECAY_GROUPS` row's `notes` with the maximum relative error printed by the fit tool.

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
| Hydraulic scram, SRI, motor run-in as distinct mechanisms; ARI reuses them (P8, `ARI_ENABLED = false`) | 2, 3 |
| All-rods-in, strongest-rod, paired-rod, cold xenon-free SDM; sourced margin targets | 9 |
| Stuck-rod fault honoured (master `Fault::RodStuck`) | 3 |
| Trace schema v2 columns `clad_t_c`, `rods_min_pos`, `rods_max_pos` populated | 2, 4 |
| Register, assumptions, applicability, baselines | 1, 11 |

**Acceptance gate (verbatim):** Supported reactivity and shutdown behavior has reproducible evidence. Uncertain rod worth, feedback, and fuel limits cannot silently appear as confirmed plant performance.
