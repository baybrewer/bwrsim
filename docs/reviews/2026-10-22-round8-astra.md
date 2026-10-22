1. Reviewed `d95428d..7b9c0ae` and the requested core, examples, tests, desktop and UI scope; read only the two specified round7 checklists.
2. `cargo test --release -p bwr-core` PASS: 86 unit + 15 limits + 36 plant = 137 tests, zero failures; zero doctests.
3. `cargo build --release --workspace` and `npm --prefix ui run build` PASS; interactive desktop/browser operation was not exercised.
4. Validation and startup `0.4` PASS: synchronization at 4716 s, startup ends at 39.3% without scram; validation startup ends at 47.2%.
5. Deduplicated prior items: fixed=4, partial=3, notfixed=0, regressed=0; one new medium finding in unchanged core code, no demonstrated round8-introduced defect.
6. Verdict (a), works as intended: NO across the full accepted transient domain; round8 admission exclusion works, but rapid neutron-power changes produce incorrect integrated heat (R1).
7. Verdict (b), physics appropriate for a BWRX-300 training simulator: NO for quantitative operator training; nominal internal acceptance is not transient-fidelity qualification.
- (c) Integrate fission and decay energy consistently with the kinetics timeline, and qualify rapid-power thermal responses with independent numerical references.
- (c) Establish independent startup/transient acceptance, timestep tolerances and IC mission-duration, pool-depletion and replenishment qualification.
- (c) Qualify controls, protection, thermal limits, instruments and operator exercises within an explicit envelope that documents the accepted lumped-model approximations.

## Prior open items

The supplied round7 reports label their new findings **R1/R2**, not N-numbers. This table covers every PARTIAL item and both reports' new findings, with repeated historical rows and cross-review aliases consolidated. Previously FIXED rows are not reopened. Gas transport, inlet phase treatment, wet-mixture expansion and property-domain handling are separately identified defects; historical repetition of each is counted only once. FIXED means the identified defect is resolved within the explicitly chosen admission envelope, not that the entire subsystem is qualified.

| source | item | status | evidence |
| --- | --- | --- | --- |
| Both round7 tables, all carried Sol 22 rows | Independent startup/transient validation | PARTIAL | `crates/bwr-core/examples/validation.rs:205`: internal startup gates pass, including 4716 s synchronization, but no independent plant transient reference exists. |
| Both round7 tables, all carried Astra 28 rows | IC mission-duration, depletion and replenishment qualification | PARTIAL | `crates/bwr-core/examples/validation.rs:93`: endurance remains 1800 s; 1/2/3-train endpoints pass at 2.700/0.920/0.377 MPa, without mission-duration or coupled replenishment qualification. |
| Both round7 tables, carried round5-sol R3 and round5-astra R5 | Core boiling pressure and hydrostatic treatment | PARTIAL | `crates/bwr-core/src/vessel.rs:850`: cold-core hydrostatic/subcooling regressions pass, but saturation still uses one mid-plane pressure without axial pressure nodes or head flow losses, as acknowledged. |
| Both round7 tables, carried round5-sol R2 and round5-astra R4 | Dome-gas coflow and downstream treatment | FIXED | Isolated bypass purge exports/books 13.389462/13.367877/13.357120 kg air at dt=0.05/0.025/0.0125 s; maximum vessel water+air residual is 3.49e-11 kg, and `src/sim.rs:439` excludes the previously unresolved gas-bearing turbine path. |
| Both round7 tables, carried round6-sol R1 and round6-astra R2 | Gas-bearing turbine inlet phase treatment | FIXED | `crates/bwr-core/tests/limits.rs:401` passes: the 90 C/180 t/100 kg air fixture now admits zero turbine flow/work/extraction; trace gas below the stated threshold remains admitted under the accepted pure-steam approximation. |
| round7-sol new R1; round7-astra new R2 | Frozen vapour composition during wet-mixture expansion | FIXED | `crates/bwr-core/src/sim.rs:439`, `:452`: actual hydraulic admission is zero above the gas threshold, including already-open TCVs; isolated threshold and purge-clearance probes confirm exclusion and recovery without a latched trip. |
| round7-sol new R2; round7-astra new R1 | Silent partial-pressure property clamping | FIXED | `crates/bwr-core/src/bop.rs:571`: unsupported nonzero-flow endpoints are explicitly flagged with zero work/extraction; the former 40 C air-rich turbine fixture is blocked upstream and the release property-domain regression at `tests/limits.rs:456` passes. |

## New findings

No new defect introduced by the six-file round8 change was demonstrated. R1 below is a remaining physics/integration defect in unchanged code. It concerns the time integral of the model's own neutron-power history, independently of plant calibration, mixture expansion or the acknowledged core pressure approximation.

## R1 — Fuel heat uses final neutron power for the entire plant step

- **severity:** medium
- **area:** physics
- **file:line:** crates/bwr-core/src/reactor.rs:387
- **wrong:** `Core::step` resolves the neutron history in substeps, then advances decay heat using only the final neutron population and applies the final `thermal_power()` over the entire outer `dt` at lines 388-397. The deposited energy is therefore an end-point rectangle, not the integral of the resolved power history. Isolated check: initial neutron fraction 1e-4, equilibrium precursors and decay groups at that fraction, stationary rods, fixed reactivity 0.010, fuel/coolant initially 40 C, elapsed time 0.05 s. One 0.05 s call deposits 1,593,820.063 J; two 0.025 s calls deposit 856,912.122 J; fifty 0.001 s calls deposit 323,289.413 J. All three finish at the same neutron fraction, 0.039638966429, so the nearly fivefold difference between the first and third cannot be explained by different neutron endpoints. The energy measured is coolant heat integrated over the interval plus `FUEL_HEAT_CAP * delta(fuel_t)`. An independent augmented matrix exponential for the same six delayed-neutron groups, source, nine decay groups and energy integral gives 294,435.836 J and neutron fraction 0.037650249560; the 0.0001 s production refinement gives 302,222.970 J. The independent endpoint also exposes a much smaller kinetics discretization difference, distinct from the dominant thermal integration error.
- **consequence:** Rapid power increases over-deposit fuel/direct-coolant heat and distort subsequent Doppler feedback and pressure response; prompt decreases under-deposit it. The latter is also verified: at reactivity -0.020, the coarse step deposits 1,245.025 J versus the independent 1,348.235 J. This is not confined to an invalid component input: a consistent cold-shutdown plant switched to STARTUP, IRM range 10, core initialized at 1e-4 and stationary rod positions chosen for reactivity 0.010 accepts the first 0.05 s scan, deposits the same 1.594 MJ, and reports no kinetics/model error; short-period protection scrams at the end of that scan. This is an accepted boundary-state demonstration, not a claimed stock operator sequence. Fuel temperature remains only 40.105 C. Existing whole-plant conservation gates cannot expose this error because `examples/validation.rs:323` treats `sim.q_core` as an imposed boundary source and its inventory excludes fuel energy.
- **fix:** Accumulate fission and decay energy on the accepted kinetics substep timeline. Advance the fuel node on that timeline, or drive its outer-step update with an accurately integrated source; return the step-average heat actually transferred to coolant. Evolve decay production from that same history rather than the final neutron value. Add independent constant-reactivity growth and prompt-drop energy-integral checks, plus outer-step refinement at 0.05/0.025/0.0125 s; assert integrated source energy and fuel-plus-coolant energy, not only final neutron power or conservation against `q_core`.

## Verification evidence and limits

All five required commands completed with exit 0 at `7b9c0ae6b3ff10f49864e0e89f1f31b9345dca6d`. Exact logs are `/tmp/bwrsim-round8-astra/{test,build,ui,validation,startup}.log`. The workspace build briefly waited for a Cargo lock. The first UI launch encountered a temporary-directory creation race before npm started; its immediate rerun completed successfully. The required examples regenerated ignored validation traces.

Validation reports maximum absolute whole-plant mass residual 2.79e-9 kg and maximum relative whole-plant energy residual 2.10e-13. Turbine-trip refinement from dt=0.05 to 0.025 s changes peak pressure by +0.0012 MPa and minimum level by -0.02 cm. Cold-condenser 60 s endpoints are 20.1630/20.1604/20.1604 C for dt=60/1/0.05 s. These establish accounting and the tested convergence behavior, not the accuracy of an externally supplied core-heat history.

Fresh admission probes at 90 C admit gas mass fractions 0, 0.000999 and 0.001; fractions 0.001001, 0.1 and 0.78335823 admit exactly zero water, gas and shaft power through the turbine. At the threshold, admitted water is 0.388627161 kg/s and shaft power is 94,699.384 W. A bypass purge starting at gas fraction 0.001444277 restores positive turbine flow at 2.30 s, without a latched turbine trip. The snapshot can reflect the post-step gas fraction, whereas admission uses the start-of-step fraction; the probes account for that distinction.

One-second blocked-admission/bypass runs at dt=0.05/0.025/0.0125 s discharge 3.960757986/3.960997295/3.961116672 kg water and 13.389462251/13.367877416/13.357120168 kg air. Condenser received-air counters match vessel export. Maximum per-step vessel water+air residual is 3.49e-11 kg and water/metal energy residual is below 7.57e-6 J. The separate 40 C admission and explicit out-of-domain inlet cases pass in the release limits suite.

A pure-steam pressure/enthalpy sweep was independently checked with [IAPWS-IF97](https://iapws.org/technical-guidance/release/IF97-Rev). It did not show an excessive reversible-work estimate in the sampled states. The existing linear superheat entropy approximation remains approximate and is not promoted here to an independently accurate turbine performance model. Gas-bearing turbine expansion was deliberately excluded from this review's required domain.

The desktop/UI files have no round8 diff. Contract and mutation tests pass, and both production builds succeed. A freshly bundled browser mock preserves a 0.125 fractional target, stops at the actual rod position, and after a turbine trip plus 100 x 0.05 s reports zero turbine flow and zero extraction heat while stored feed temperature remains 225.148832 C. This is a UI-stand-in check; desktop rendering, event-loop responsiveness and operator exercises were not run.

Review-only probes and output are `/tmp/bwrsim-round8-astra/src/main.rs` and `probes.log`; reproduce with `cargo run --release --offline --manifest-path /tmp/bwrsim-round8-astra/Cargo.toml`. Independent calculations are `reference.py` and `reference.log`, run with `/tmp/iapwsenv/bin/python`. The kinetics reference uses an augmented linear system for neutron population, six precursors, nine decay groups, a constant source and integrated thermal energy, evaluated with `scipy.linalg.expm`; it does not call production kinetics or fuel functions. Browser checks are `mock-check.cjs`, `mock-build.log` and `mock.log` in that directory.

The requested design decisions and known omissions remain accepted: vapour-only main line; exclusion of turbine admission above 1e-3 dome gas mass fraction; single mid-plane core saturation/no head flow losses; no choked-flow temperature correction; pure-vapour IC draw; condenser shell air outside water balances; fault-injected unreachable condenser dryout; and absent independent plant transient references. None is counted as a new finding. The carried IC mission/depletion/replenishment qualification gap remains PARTIAL.

Only this review report was written in the repository by this review; production source and tests were unchanged. Pre-existing and concurrently appearing untracked files were neither read nor modified, including the other round8 report.

OPEN: fixed=4 partial=3 notfixed=0 regressed=0
NEW FINDINGS: 1

