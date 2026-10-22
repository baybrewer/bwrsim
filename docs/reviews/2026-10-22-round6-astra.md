1. Reviewed `845af64..b3ffc49` and the requested core, desktop and UI scope; only the two authorized round5 reports were read under `docs/reviews/`.
2. Release core tests PASS: 85 unit + 9 limits + 36 plant = 130 tests, zero failures; zero doctests.
3. Release workspace build and UI production build PASS; interactive desktop operation was not tested.
4. Required validation and startup `0.4` PASS: sync 4716 s, startup ends at 39.3% without scram; validation ends at 47.2%.
5. Prior assessments: fixed=23, partial=10, notfixed=0, regressed=0 across 33 source-specific/carried rows; three new medium findings.
6. Verdict (a), works as intended: NO across the accepted gas-bearing domain; nominal cases and the corrected water-solid/vacuum/cold-core limits pass.
7. Verdict (b), physics appropriate for a BWRX-300 training simulator: NO for quantitative operator training; the earlier startup is physically plausible within this model, not independently qualified.
- (c) Finish mixture-aware flow measurement, valve capacity and turbine thermodynamics; define and enforce the supported thermal-hydraulic envelope.
- (c) Qualify startup/transients and IC mission duration, pool depletion and replenishment against independent references and explicit timestep tolerances.
- (c) Validate controls, protection, fuel/thermal-hydraulic limits, instruments and desktop/browser exercises against a defined training scope.

## Prior open items

The supplied reports call their new findings **R1-R4** and **R1-R5**, not N-numbers. The table includes all ten PARTIAL rows and all nine new R-findings, retaining source-specific duplicates, plus the fourteen carried N-identifiers in round5-astra to cover the requested N-number wording: 10 + 9 + 14 = 33 assessments. No other review file was consulted. FIXED applies to the identified failure and the explicitly accepted vapour-only main-line design, not unrestricted subsystem qualification. Gas transport is PARTIAL because species inventory now closes but the remaining mixture physics below does not. The acknowledged single-pressure core approximation remains PARTIAL without being raised again as a new defect.

| source | item | status | evidence |
| --- | --- | --- | --- |
| round5-sol, carried round4-sol | Sol 22 | PARTIAL | `crates/bwr-core/examples/validation.rs:205`: startup gates pass at 0.472 end power; no independent transient reference was added. |
| round5-sol, carried round4-sol | Astra 28 | PARTIAL | `crates/bwr-core/examples/validation.rs:94`: 1800 s IC cases pass at 2.700/0.920/0.377 MPa for 1/2/3 trains; mission-duration and replenishment qualification remain absent. |
| round5-sol, carried round4-sol | R2 | FIXED | `crates/bwr-core/src/vessel.rs:721`: water-solid main-line delivery is zero; isolated 40 C open-bypass runs preserve water inventory at all three timesteps, consistent with the accepted design. |
| round5-sol, carried round4-astra | Sol 22 | PARTIAL | `crates/bwr-core/examples/validation.rs:205`: internal startup acceptance passes, but independent plant response qualification remains absent. |
| round5-sol, carried round4-astra | Astra 28 | PARTIAL | `crates/bwr-core/examples/validation.rs:94`: train-count gates cover only 1800 s, not mission-duration pool depletion/replenishment. |
| round5-sol, carried round4-astra | R3 | FIXED | `crates/bwr-core/tests/limits.rs:89`: 110/200 C water-solid coupled tests pass with zero main-line delivery and zero turbine work; unsupported liquid delivery has been removed by design. |
| round5-sol | R1 | FIXED | `crates/bwr-core/src/vessel.rs:721`: the reversed liquid-valve response is no longer an accepted plant path; water-solid sources supply zero main-line flow. |
| round5-sol | R2 | PARTIAL | Isolated 90 C/180 t/100 kg air run exports 11.289418 kg air in 1 s, exactly booked by the condenser, but valve composition and turbine partial-pressure handling remain wrong (new R2/R3). |
| round5-sol | R3 | PARTIAL | `crates/bwr-core/src/vessel.rs:855`: isolated 90 C/500 kg/s/8.7 MW fixture now has zero void/quality at all three timesteps; head is included at one mid-plane pressure, without axial pressure/flow-loss resolution. |
| round5-sol | R4 | FIXED | Isolated 70.182 kPa dome into 5.929 kPa condenser supplies 4.597951 kg/s; equal/reversed receiver pressure gives zero and subcritical delivery reconciliation closes to 1.48e-14 kg/s. |
| round5-astra, carried both round4 tables | Sol 22 | PARTIAL | `crates/bwr-core/examples/validation.rs:205`: required startup and validation pass; independent references remain unavailable. |
| round5-astra, carried both round4 tables | Astra 28 | PARTIAL | `crates/bwr-core/examples/validation.rs:94`: 1/2/3-train pressure and decay-heat-removal gates pass at 1800 s; longer mission/replenishment qualification remains outstanding. |
| round5-astra, carried round4-sol | R2 | FIXED | `crates/bwr-core/src/vessel.rs:721`: the accepted vapour-only boundary removes liquid-valve operation; cold water-solid refinement passes without draining or flashing. |
| round5-astra, carried round4-astra | R3 | FIXED | `crates/bwr-core/tests/limits.rs:89`: hot water-solid coupled tests deliver neither liquid nor vapour to the main line, as required by the design decision. |
| round5-astra | R1 | FIXED | `crates/bwr-core/src/vessel.rs:721`: the offending liquid conductance is no longer used for accepted vessel discharge. |
| round5-astra | R2 | FIXED | Isolated 40 C/249966.847 kg/open-bypass runs at dt=0.05/0.025/0.0125 s lose exactly zero water and remain water-solid at 2.279846/2.279842/2.279840 MPa. |
| round5-astra | R3 | FIXED | `crates/bwr-core/src/sim.rs:445`: actual condenser pressure reaches the hydraulic solve; independent positive-vacuum, zero/reverse-difference and subcritical supply-limit checks pass. |
| round5-astra | R4 | PARTIAL | `crates/bwr-core/src/vessel.rs:754`: air leaves at the dome mass ratio 3.615915 kg/kg water and reaches the condenser; species debit is fixed, but new R2/R3 prevent physically consistent mixture treatment. |
| round5-astra | R5 | PARTIAL | Isolated cold-core pressure is 0.198046 MPa with zero void/quality; IF97 gives 119.901442 C versus table 119.901700 C, but the acknowledged single mid-plane saturation approximation remains. |
| round5-astra, carried Sol | N1 | FIXED | `crates/bwr-core/tests/plant.rs:1064` and `:1093`: water-solid and vapour-limited IC conservation regressions pass. |
| round5-astra, carried Sol | N2 | FIXED | `crates/bwr-core/tests/limits.rs:210`: vapour-limited hydraulic reconciliation passes; independent subcritical accepted flow satisfies the valve relation to 1.48e-14 kg/s. |
| round5-astra, carried Sol | N3 | FIXED | `crates/bwr-core/tests/plant.rs:1132` and `:1147`: complete-state/governor rollback and retained advance debt regressions pass. |
| round5-astra, carried Sol | N4 | FIXED | `crates/bwr-core/tests/plant.rs:1017`: hotwell exhaustion test passes with accepted feed prelimited before vessel integration. |
| round5-astra, carried Sol | N5 | FIXED | `crates/bwr-core/src/bop.rs:1601`: standalone relief-depletion/dryout rejection test passes with a closed accepted state; plant rollback remains fault-injection coverage. |
| round5-astra, carried Sol | N6 | FIXED | Required validation's maximum reported relative whole-plant energy residual is 2.10e-13; signed heater/storage tests pass. |
| round5-astra, carried Sol | N7 | FIXED | Freshly bundled browser mock, FullPower -> turbine trip -> 100 x 0.05 s: turbine flow=0, extraction=0 MW, stored feed temperature=225.142195 C. |
| round5-astra, carried Sol | N8 | FIXED | `crates/bwr-core/tests/plant.rs:613` and `:652`: actual Rust/TypeScript contract and payload/enum/nullability mutation tests pass; UI build passes. |
| round5-astra, carried Astra | N1 | FIXED | `crates/bwr-core/tests/plant.rs:1093`: accepted IC scaling/conservation regression passes; water-solid IC refusal also passes. |
| round5-astra, carried Astra | N2 | FIXED | `crates/bwr-core/tests/plant.rs:1017`: accepted feed/hotwell inventory regression passes. |
| round5-astra, carried Astra | N3 | FIXED | `crates/bwr-core/tests/plant.rs:1132`, `:1147`, `:1195`: governor, backlog and injected condenser-fault rollback tests pass; reachable CW-loss protection test at `:1225` passes. |
| round5-astra, carried Astra | N4 | FIXED | Required startup exits 0: sync at 4716 s, end power 0.393 at 4970 s, no scram. |
| round5-astra, carried Astra | N5 | FIXED | `crates/bwr-core/src/bop.rs:1723`: coasting-pump delivery, rundown and eventual check-valve closure regression passes. |
| round5-astra, carried Astra | N6 | FIXED | `crates/bwr-core/src/sim.rs:481`: vessel feed receipt uses the heater's delivered enthalpy; signed heater and whole-plant energy checks pass. |

The carried N2 flow-shortfall fix still reconciles hydraulic request and delivered mass. New R1 below is a separate composition/consumer error introduced when that delivered mass began including air. The table does not redefine the earlier pure-steam shortfall fixture.

## New findings

R1 is introduced by round 6's change in the meaning of the main-line flow fields. R2 and R3 are remaining mixture-model defects exposed by tracing the newly transported gas through downstream consumers. Numerical fixtures establish accepted component/plant-state failures; they are not claims that each state is reachable from a stock scenario using operator commands alone. None requires liquid main-line hydraulics, axial core pressure nodes, IC gas transport or including shell air in water balances.

## R1 — Main-line air is counted as replacement water by feed control

- **severity:** medium
- **area:** correctness
- **file:line:** crates/bwr-core/src/sim.rs:439
- **wrong:** Round 6 changes `SteamSystem::total_flow()` to water plus air (`sim.rs:504`), but passes that quantity unchanged to the three-element feed controller at `:470`; `bop.rs:1181` treats it as water to replace. The consistent sealed 90 C/180000 kg water/100 kg air fixture initially delivers 3.286540 kg/s water and 11.883851 kg/s air. The controller receives 15.170391 kg/s. An isolated controller/pump run at the same vessel pressure and zero level error settles at 15.170391 kg/s water, versus 3.286540 kg/s when fed the actual water-loss signal. The snapshot simultaneously reports vessel steam=3.286540 and bypass steam=15.170391 kg/s (`sim.rs:968`, `:984`), with no gas-flow field to explain the difference. The first-step discrepancy is identical at dt=0.05/0.025/0.0125 s.
- **consequence:** Gas purging produces a false water-replacement demand and inconsistent steam-flow indications. In the isolated steady controller check, replacement is 4.616 times the actual water loss. Plant level feedback will react to the resulting inventory error, so that factor is not asserted as a sustained coupled-plant overfeed prediction.
- **fix:** Keep mixture hydraulic flow separate from accepted water-vapour flow. Feed the latter, with the existing one-scan timing, to the water level controller. Preserve water-based steam snapshot semantics or explicitly expose mixture and gas flows with clear units/labels. Add a gas-bearing controller/telemetry regression checking both species and controller demand; do not alter pure-steam behavior.

## R2 — Turbine phase properties use total mixture pressure as pure-water pressure

- **severity:** medium
- **area:** physics
- **file:line:** crates/bwr-core/src/sim.rs:513
- **wrong:** The new water fraction corrects turbine mass, but `TurbineInlet::new` still receives mixture total chest pressure and evaluates `sat_p(p_chest)` at `bop.rs:476`. With the sealed 90 C/180 t/100 kg air fixture, TCV=0.01 and condenser=5928.62 Pa, chest total pressure is 229955.957 Pa while water partial pressure is about 70182.206 Pa. The isenthalpic stream contains water vapour at approximately 2659.529 kJ/kg; its slightly reduced partial pressure makes it vapour, yet the code assigns water quality 0.9757305 and creates 0.00089298 kg/s liquid from 0.0367942 kg/s water. Independent IF97 gives the same erroneous 0.9757303 only when deliberately supplied the wrong total pressure. It gives vapour at the water partial pressure. The code also changes the vapour work bound from about 303966 to 453639 J/kg by using that false state. The relevant mixture principle is species partial pressure, consistent with the vessel's own Dalton closure; see [NASA's mixture formulation](https://www.grc.nasa.gov/WWW/wind/TFAWS2007/Formulation.pdf) and the [IAPWS-IF97 formulation](https://www.iapws.org/relguide/IF97-Rev.html).
- **consequence:** Gas-bearing turbine admission invents condensate and misstates vapour enthalpy, extraction availability and shaft response. Water-energy bookkeeping can still close while the state and available-work calculation are physically wrong. The numerical work comparison isolates the steam-state error; it is not an independent reversible-work bound for the complete air/steam mixture.
- **fix:** Carry composition and temperature through the throttle/turbine boundary, use water partial pressure for water phase equilibrium, and define a consistent mixture expansion model. If that turbine operating domain is unsupported, explicitly isolate/reject gas-bearing turbine admission while allowing supported bypass purging. Add an all-vapour air/steam throttle limit and a pure-steam recovery check.

## R3 — Valve mass capacity is independent of gas composition

- **severity:** medium
- **area:** physics
- **file:line:** crates/bwr-core/src/bop.rs:136
- **wrong:** Mixture density affects only line friction. Both valve conductances and the fixed critical ratio remain pure-steam constants, so choked mass flow is essentially unchanged as the gas tends toward air. An isolated fixed-temperature comparison at 110 C, 100000 Pa upstream, 5000 Pa downstream and TCV=0.001 gives 0.007385714046 kg/s for steam density 0.573125 kg/m3 and 0.007385714134 kg/s for air density 0.909230 kg/m3. Line drops are only 0.00325/0.00205 Pa. At fixed temperature and area, the ideal-gas choked factor is `sqrt(gamma/R) * (2/(gamma+1))^((gamma+1)/(2*(gamma-1)))`; steam/air approximations gamma=1.3/1.4 and R=461.526/287.05 give an air/steam mass-flow ratio of 1.3012, not 1.000000012. A 78.34% air mass fraction, as in the prior 100 kg-air fixture, gives an illustrative ideal-mixture factor of about 1.21 relative to the same steam reference. This is a limiting-law comparison, not calibrated BWRX-300 valve capacity. The composition dependence follows [NASA's choked mass-flow relation](https://www.grc.nasa.gov/www/k-12/BGP/mflchk.html).
- **consequence:** Air-rich startup/purge capacity and condenser air loading remain systematically misrepresented despite exact species bookkeeping. Correcting line density cannot fix a valve-dominated limit.
- **fix:** Normalize valve capacity and choking ratio for supported mixture gas properties while retaining the rated steam calibration, and use the same relation in delivered-flow reconciliation. Add fixed-temperature composition sweeps, including the air-dominated limit. This finding is distinct from the acknowledged missing temperature correction: temperature is held constant in the discriminating check.

## Startup timing and physical plausibility

The exact required round 5 startup reproduces at **5311 s**, rather than the prompt's approximately 5308 s. Round 6 reproduces at **4716 s**, a 595 s (11.2%) advance. To separate the causes, a temporary copy of round 6 set only the added hydrostatic column contribution to zero, including its uses in initialization and friction calibration. It synchronizes at **5330 s**. Thus the head treatment accounts for the dominant advance; the changed line law and the scripted synchronizer affect the final seconds.

| startup variant, dt=0.05 s | initial core flow kg/s | initial average void | first critical s | turbine roll s | sync s |
| --- | ---: | ---: | ---: | ---: | ---: |
| round 5 `845af64` | 778.733 | 0.027635 | 3651.40 | 4942 | 5311 |
| round 6 with head contribution disabled | 778.733 | 0.027635 | 3651.45 | 4942 | 5330 |
| round 6 `b3ffc49` | 562.187 | 0.008046 | 3624.85 | 4348 | 4716 |

The direction is physically credible: local head increases the boiling enthalpy threshold, reduces low-power void, and therefore reduces negative void feedback. The initial void change corresponds to about **+156.7 pcm** with this model's `VOID_COEF=-0.080`. Low decay-heat power is especially sensitive to the added subcooling despite the head being a small fraction of 6.9 MPa. The fixed-level initializer also adds about 1066 kg of water because there is less void displacement; the ablation includes that consistent inventory change. Natural-circulation flow falls as the void-driven density difference falls. The script's period-limited, discrete rod withdrawals amplify the change in the time to reach 3.5% power and begin turbine roll.

Refining round 6 to dt=0.025/0.0125 s gives first criticality at 3624.85/3624.85 s, turbine roll at 4347/4347 s, and synchronization at 4745/4754 s. The example checks phase and speed once per second, so synchronization is an event-sensitive endpoint; these results do not establish an exactly converged 4716 s prediction. All runs reach the target without scram. The roughly ten-minute head-induced advance remains much larger than this 38 s timing spread. This is a plausible response of the implemented feedback/control model, not evidence that a real plant's startup should advance by that amount.

## Verification evidence and limits

All five required commands exited 0 on `b3ffc495586784e5b696ae2c79bcdd6220b53477`. Logs are `/tmp/bwrsim-round6-astra/{test,build,ui,validation,startup}.log`. The initial parallel UI log redirection ran before its temporary directory existed; rerunning the actual UI build completed successfully. No build or test failure is hidden by that shell setup issue.

Required validation's maximum reported absolute whole-plant mass residual is **2.79e-9 kg**, and maximum relative energy residual is **2.10e-13**. Turbine-trip refinement changes peak pressure by +0.0012 MPa and minimum level by -0.02 cm. The condenser's independent cold-step endpoints remain 20.1630/20.1604/20.1604 C for dt=60/1/0.05 s. These are conservation/convergence checks of implemented trajectories, not independent transient validation.

Independent gas-bearing one-second runs at dt=0.05/0.025/0.0125 s export respectively **3.302679/3.303085/3.303287 kg water** and **11.289418/11.274837/11.267565 kg air**. Each condenser books exactly the exported air. Maximum per-step combined vessel water+air residual is 3.35e-11 kg; maximum water/metal energy residual is 7.57e-6 J. This verifies the inventory fix while demonstrating why conservation alone does not reject new R1-R3. The cold 90 C core check remains deeply subcooled, with approximately 503.367 kJ/kg local saturated-liquid enthalpy versus less than 398 kJ/kg calculated core-exit enthalpy after one second.

Probe source/results are `/tmp/bwrsim-round6-astra/src/main.rs` and `probes.log`; reproduce with `cargo run --release --offline --manifest-path /tmp/bwrsim-round6-astra/Cargo.toml`. Independent IF97 source/results are `if97.py` and `if97.log`, using the pre-existing `/tmp/iapwsenv/bin/python`. The `baseline/`, `headless/` and `current/` temporary projects contain the startup ablation/instrumentation, with corresponding logs and `startup-dt025.log`/`startup-dt0125.log`. Only `crates/bwr-core` was extracted from the comparison commits. The headless change exists solely in the temporary review copy.

The scoped desktop/UI files have no round 6 diff. Their build and actual contract tests pass. The freshly bundled mock preserves fractional rod targets and stops, retains zero extraction after a turbine trip, and passes the previously selected gas-bearing void checks. Its deliberately crude physics is not equivalent to the Rust backend; neither interactive desktop emission timing nor operator exercises were tested.

The accepted vapour-only main-line design is respected: no liquid main-line law is requested. The single mid-plane core pressure, omitted head flow losses, absent choked temperature correction, IC pure-vapour draw, shell air outside water balances, injected plant condenser-dryout rollback and absent independent plant transient references remain explicitly acknowledged limitations. None is presented as a new finding here. Mission-duration IC validation remains open even though the 1800 s gates and component pool tests pass.

Only this review file was written in the repository. Production source/tests were unchanged, generated validation traces are ignored, and pre-existing untracked files were preserved.

OPEN: fixed=23 partial=10 notfixed=0 regressed=0
NEW FINDINGS: 3
