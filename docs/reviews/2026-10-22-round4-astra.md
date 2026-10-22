1. Reviewed `a64e870..95bb74f` and the requested current core, tests/examples, desktop and UI scope; read only the two authorized prior review files.
2. Release core tests PASS: 74 unit + 6 limits + 35 plant tests = 115 tests, zero failures; zero doctests.
3. Release workspace build and UI production build PASS; no interactive Tauri smoke test was run.
4. Validation and startup examples PASS; startup synchronizes at 5311 s and reaches 39.2% power at 5569 s without scram.
5. Prior assessments: fixed=23, partial=3, notfixed=0, regressed=0 across 26 rows; three findings below: one new regression and two acknowledged remaining physics defects.
6. Verdict (a), works as intended: NO across the accepted model domain; nominal cases pass, but cold IC heat transfer and low-temperature/phase-dependent hydraulics remain wrong.
7. Verdict (b), physics appropriate for a BWRX-300 training simulator: NO for quantitative operator training; passing internal conservation and selected trajectories do not establish plant fidelity.

- (c) Enforce passive heat-flow direction and consistent pressure/phase-dependent boiling and main-line hydraulics throughout the accepted domain.
- (c) Add independent BWRX-300 transient references, mission-duration IC/pool endpoints and second-law/limiting-state acceptance alongside conservation gates.
- (c) Qualify protection, controls, fuel and thermal-hydraulic envelopes, instrumentation and desktop/browser behavior against an explicit training scope.

## Prior open items

The two round3 reports each leave Sol 22 and Astra 28 PARTIAL. Those repeated identifiers are counted once. Each report raises five new findings labeled **R1-R5**, not N; all ten are assessed separately. To cover the request's N-number wording, the eight carried Sol N-items and six carried Astra N-items reproduced in those reports are also assessed once each. Total: 2 + 10 + 14 = 26 rows. Older rows already classified FIXED, other than the explicitly requested N-items, are not reopened. FIXED refers to the reported symptom, not unrestricted physical qualification of that subsystem.

| source | item | status | evidence |
| --- | --- | --- | --- |
| Both round3 tables | Sol 22 | PARTIAL | `crates/bwr-core/examples/validation.rs:218`: startup now requires 0.392-0.480 final power and passes at 0.461, but the transient acceptance still has no independent plant reference. |
| Both round3 tables | Astra 28 | PARTIAL | `crates/bwr-core/examples/validation.rs:94`: all three 1800 s IC runs pass, but mission-duration/pool-depletion acceptance and independent BWRX-300 response benchmarks remain absent. |
| Both round3 tables, carried Sol | N1 | FIXED | `crates/bwr-core/tests/plant.rs:1062` and `:1091`: both phase-limited IC mass regressions pass; independent gas-bearing loop energy residual stays below 1.83e-5 J. |
| Both round3 tables, carried Sol | N2 | FIXED | `crates/bwr-core/src/sim.rs:501`: isolated vapor-limited delivery equals published turbine flow, 0.002000722657 kg/s, with zero extraction and positive condenser inlet energy. |
| Both round3 tables, carried Sol | N3 | FIXED | `crates/bwr-core/tests/plant.rs:1130` and `:1145`: repeated rejected steps restore governor/plant state, and rejected advance retains its 0.1 s debt; both pass. |
| Both round3 tables, carried Sol | N4 | FIXED | `crates/bwr-core/tests/plant.rs:1015`: coupled hotwell-exhaustion regression passes with prelimited feed and matching condenser withdrawal. |
| Both round3 tables, carried Sol | N5 | FIXED | `crates/bwr-core/src/bop.rs:761`: independent relief dryout rejects unchanged at 170.400/170.420 s for dt=0.05/0.01 s; accepted equilibrium residual <=1.53e-5 J. |
| Both round3 tables, carried Sol | N6 | FIXED | `crates/bwr-core/src/bop.rs:1481`: signed heater-energy test passes; validation plant balances use physical boundary heat and reach <=3.79e-13 relative energy residual. |
| Both round3 tables, carried Sol | N7 | FIXED | `ui/src/sim/mock.ts:710`: executed FullPower -> turbine trip -> 5 s yields turbine flow=0, extraction=0 MW and stored feed temperature=225.142 C. |
| Both round3 tables, carried Sol | N8 | FIXED | `crates/bwr-core/tests/plant.rs:652`: all payload, enum and nullability mutation gates pass, together with the actual Rust/TypeScript contract. |
| Both round3 tables, carried Astra | N1 | FIXED | `crates/bwr-core/tests/plant.rs:1091`: requested/accepted IC scaling closes mass; independent gas-loop residuals at dt=0.05/0.025/0.0125 s are -1.38e-5/-1.83e-5/+1.31e-6 J. |
| Both round3 tables, carried Astra | N2 | FIXED | `crates/bwr-core/tests/plant.rs:1015`: the 20 s hotwell-limited plant step passes combined water-inventory and accepted-withdrawal checks. |
| Both round3 tables, carried Astra | N3 | FIXED | `crates/bwr-core/tests/plant.rs:1130`, `:1145` and `:1193`: governor, rejected clock debt and injected condenser-fault rollback regressions all pass. |
| Both round3 tables, carried Astra | N4 | FIXED | `crates/bwr-core/examples/startup.rs:85`: required 0.4 run exits 0, synchronizes at 5311 s and reaches 0.392 power at 5569 s without scram. |
| Both round3 tables, carried Astra | N5 | FIXED | `crates/bwr-core/src/bop.rs:1531`: pump coastdown regression passes first-scan delivery, monotone decay and eventual check-valve closure. |
| Both round3 tables, carried Astra | N6 | FIXED | `crates/bwr-core/src/sim.rs:478` and `crates/bwr-core/src/bop.rs:1124`: vessel receipt and heater debit use the same delivered enthalpy; signed heater and plant-energy checks pass. |
| round3-sol.md | R1 (new finding) | FIXED | `crates/bwr-core/tests/limits.rs:82`: 200 C liquid fixture passes; independent plant probe now gives zero shaft/extraction work and +206.914 MW condenser input instead of excessive steam work. |
| round3-sol.md | R2 (new finding) | FIXED | `crates/bwr-core/src/bop.rs:108`: committed starved-source/chest pressures are 101354.538636/101354.538632 Pa; line/TCV residuals are 3.22e-12 Pa/4.52e-16 kg/s, and RUN isolates. |
| round3-sol.md | R3 (new finding) | FIXED | `ui/src/sim/mock.ts:547`: executed sealed gas-bearing 90/110/150 C probes now give zero core void and exit quality; inlet subcooling also uses total pressure at `:746`. |
| round3-sol.md | R4 (new finding) | FIXED | `crates/bwr-core/examples/validation.rs:269` and `tests/plant.rs:976`: 150 C phase-limited RWCU accepts 0.1282461004 kg/s; accepted-port residuals are <1e-18 kg and -2.89e-6 J. |
| round3-sol.md | R5 (new finding) | FIXED | `crates/bwr-core/tests/plant.rs:1223`: consistent low-hotwell/CW-loss protection regression passes; independent run trips at 25.071 kPa, blocks bypass and never reaches relief. |
| round3-astra.md | R1 (new finding) | FIXED | `crates/bwr-core/tests/limits.rs:77` and `:90`: liquid/wet inlet work-budget checks pass; 110 C plant probe gives zero shaft/extraction and +120.500 MW condenser input at dt=0.05 s; work stays zero under refinement. |
| round3-astra.md | R2 (new finding) | FIXED | `crates/bwr-core/tests/limits.rs:112`: delivered flow, both hydraulic relations, work bound and low-chest-pressure isolation pass using the explicitly modeled starved inlet boundary. |
| round3-astra.md | R3 (new finding) | FIXED | `ui/src/sim/mock.ts:547`: at 90 C and total/gas pressures 0.171108/0.115297 MPa, executed browser core void and exit quality are both zero. |
| round3-astra.md | R4 (new finding) | PARTIAL | `crates/bwr-core/tests/limits.rs:176`: 99.99/100.01 C IC duties are continuous at 15.043382/15.043409 MW, but the main/core shortcuts remain and newly enabled cold IC duty can reverse heat flow (R1/R2 below). |
| round3-astra.md | R5 (new finding) | FIXED | `crates/bwr-core/tests/plant.rs:976`: accepted RWCU balances pass; independent 200 C probe accepts 0.3956609565 kg/s with zero mass residual and +2.34e-6 J energy residual. |

The chest fix uses the prior review's allowed explicit supply-limit boundary: the line inlet may fall below dome pressure while throttling preserves inlet enthalpy. It now reconciles committed pressure and flow and drives protection consistently. This does not validate a physical chest volume, transient flashing capacity or the liquid hydraulic law. The latter remains R3 below. The condenser test correctly distinguishes a physically consistent protection trajectory from the injected fault used to exercise unreachable dryout rollback; a reachable plant dryout is not required or claimed.

## New findings

R1 is introduced by round4's expansion of the IC operating domain. R2 and R3 are the acknowledged remaining physics defects, quantified here rather than presented as newly introduced changes. The new IC regression is distinct from the original below-100 C cutoff, so no prior row is labeled REGRESSED solely because R1 exists.

## R1 — Newly enabled cold IC duty can move heat from colder primary water to a hotter pool

- **severity:** medium
- **area:** physics
- **file:line:** crates/bwr-core/src/sim.rs:450; crates/bwr-core/src/ic.rs:155
- **wrong:** Round4 permits IC steam draw below 100 C, but `IcTrain::request` only clamps the target driving temperature; `q_lag` at `ic.rs:158` remains positive after the primary becomes colder than the pool. It funds that duty by condensing current primary vapor at its current temperature (`:183-185`), without a hotter tube-metal energy store. In a sealed, gas-free consistent vessel at 99.99 C, mass `0.6*V*rho_f`, three open trains, 100 C pools, 500 kg of tube water per train at 99.99 C and preceding duty 5 MW/train, the next second sends 14.6124/14.6218/14.6265 MJ from the colder primary to the hotter pools at dt=0.05/0.025/0.0125 s. The corresponding heat-transfer entropy changes are -1.865/-1.889/-1.900 J/K. Primary energy still closes within 8.64e-5 J, so conservation alone misses the violation. Rebuilding `a64e870` and executing the identical fixture gives zero IC duty. A second consistent fixture starts above the pool at 100.01 C: accepted cooldown crosses below 100 C and still exports 56.686/57.089/57.291 MJ to the hotter pools over the remainder of a five-second run, ending at 99.94157/99.94151/99.94148 C.
- **consequence:** Passive cooldown can undershoot its heat sink while reporting continued condensation and pool boil-off. The finite error persists under timestep refinement and can contaminate low-pressure cooldown/endurance results.
- **fix:** Bound actual condensation by the instantaneous primary-to-pool driving condition, with zero primary-funded pool duty when that condition reverses. If lag represents stored hot metal, add an explicit metal temperature/energy node and debit it separately. Add equal-temperature, reversed-temperature and cooldown-crossing regressions with hot pools; the new tests currently use 40 C pools and therefore miss this boundary. These are controlled valid state fixtures, not a claim of reachability from a stock operator scenario.

## R2 — Atmospheric shortcuts still produce wrong core boiling and suppress pressurized main-line delivery

- **severity:** medium
- **area:** physics
- **file:line:** crates/bwr-core/src/vessel.rs:800; crates/bwr-core/src/sim.rs:440; crates/bwr-core/src/vessel.rs:682
- **wrong:** `eos.subcooled` means only `T<100 C` (`vessel.rs:394`). `core_sat` then substitutes atmospheric pressure even for a sealed pressurized vessel, and both the main-line pressure and vessel delivery gate suppress flow below that temperature. In an equilibrium-consistent 90 C vessel with 100 kg trapped air, total pressure is 0.255243 MPa: independent IF97 gives a 128.098866 C boiling point and 161.162 kJ/kg bulk subcooling, whereas the engine uses 99.974371 C. With `h_in=h_liquid(90 C)` and 8.7 MW core heat, an accepted 0.05 s vessel step reports 3.918% equilibrium core void and exit quality 0.001762 despite insufficient heat to reach the mechanical-pressure boiling state. At 99.99/100.01 C with the same gas inventory, the chosen saturation temperature jumps from 99.974 to 133.125 C, calculated equilibrium core void jumps from 63.21% to zero, and a 1 kg/s main-line request jumps from zero to full delivery. No phase disappearance or pressure discontinuity justifies those jumps.
- **consequence:** Gas-bearing heatup can generate premature void feedback and level swell in the engine after the browser regression has been fixed. Sub-100 C pressurized or water-solid discharge is disabled irrespective of its actual mechanical driving pressure. This is the known unfinished shortcut, not a round4 regression.
- **fix:** Derive core boiling from actual total mechanical pressure throughout the supported domain, including initialization. Derive main-line availability and hydraulics from actual phase inventory, upstream/downstream pressures and inlet state rather than a temperature flag. Add sealed gas-bearing, gas-free vacuum and compressed-liquid sweeps on both sides of 100 C; distinguish equilibrium head-space vapor from local core boiling.

## R3 — Liquid main-line flow still uses the rated-steam hydraulic resistance

- **severity:** medium
- **area:** physics
- **file:line:** crates/bwr-core/src/bop.rs:87
- **wrong:** `compute_flows_from` accepts liquid enthalpy, but the line resistance remains `STEAM_LINE_DP / STEAM_FLOW_RATED^2 / opening^2` without a density or phase correction. The consistent 200 C water-solid fixture delivers 242.481409 kg/s and incurs 53.234254 kPa modeled line loss. Its compressed-liquid density is 866.396863 kg/m3 versus rated steam's 37.696617 kg/m3. At the same mass flow and geometry, the density factor in a single-phase friction law would reduce that loss to approximately 2.316203 kPa, a factor of 22.98; this comparison isolates density dependence and is not a claimed calibrated liquid-flow solution. Valve flow also remains a steam-calibrated pressure-linear law. The new turbine work/extraction protection correctly prevents impossible liquid shaft work, but does not fix this hydraulic state.
- **consequence:** Flooded discharge, chest pressure and subsequent isolation/control responses remain quantitatively wrong even though mass and energy balance and turbine work limits pass. This is the known unfinished phase-dependent resistance, not a round4 regression.
- **fix:** Use phase-appropriate density and valve/line flow relations for compressed liquid and flashing discharge, or reject/isolate the unsupported liquid main-line path transactionally. Keep an independent same-flow density-scaling check and a supported liquid-flow benchmark; retuning a single steam coefficient will not cover both phases.

## Verification evidence and limits

All five required commands exited 0 on `95bb74f`. Logs, standalone probes and the baseline comparison are under `/tmp/bwrsim-round4-review/`: `test.log`, `build.log`, `ui.log`, `validation.log`, `startup.log`, `probes.log`, `src/main.rs`, `mock-probes.cjs`, `mock-probes.log`, `if97.log` and `baseline.log`. Reproduce current Rust probes with `cargo run --release --offline --manifest-path /tmp/bwrsim-round4-review/Cargo.toml`; the baseline package is `baseline-probe/Cargo.toml`. Browser probes execute the bundled current mock without starting its timer. Only this requested review file was written in the repository; source and tests were not edited, generated traces are ignored, and unrelated files were left untouched.

Validation runs eight trajectories plus timestep/train-count/cold-condenser checks. The maximum absolute whole-plant mass residual is 5.59e-9 kg; the maximum reported relative plant-energy residual is 3.79e-13. Startup validation ends at 0.461 power, within the newly tightened 0.392-0.480 range. The separate required startup run ends at 0.392. IC one/two/three-train endpoints are 2.750/0.930/0.355 MPa at 1800 s. Turbine-trip refinement changes peak pressure by +0.0012 MPa and minimum level by -0.02 cm. These verify implemented trajectories, not independent plant response or long-duration IC qualification.

Independent limiting checks reproduce accepted-port RWCU closure, gas-bearing IC primary energy closure, compressed-liquid outlet enthalpy, dry-condenser rejection and cold-condenser timestep convergence. The reachable CW-loss check peaks at 25.071336 kPa and trips on low vacuum after 8.35 s of stepping; bypass remains blocked, the hotwell refills to 15172.080 kg by 300 s and no model limit occurs. The injected condenser-fault test remains useful rollback coverage and is accurately labeled.

The new liquid-inlet work limit is checked against the separate Python `iapws` implementation of [IAPWS-IF97](https://iapws.org/technical-guidance/release/IF97-Rev). At the actual probe backpressure, independent reversible drops are 37.1606 and 145.2758 kJ/kg for the 110 and 200 C compressed-liquid fixtures; modeled shaft work is zero in both. At the nominal probe inlet the independent ideal drop is 988.845 kJ/kg, above modeled work of about 701.5 kJ/kg. These are thermodynamic inferences from independently computed states, not vendor turbine-performance validation. The new superheat entropy approximation is conservative in the additional tested states but is not a general Region 2 replacement.

No new round4 defect was found in the scoped desktop bridge, TypeScript contract, RodControls or TopBar. Builds and contract tests pass; interactive desktop emission timing and human training exercises were not tested. The mock remains explicitly a UI development approximation, so its corrected gas/core pressure semantics do not establish quantitative browser physics fidelity.

OPEN: fixed=23 partial=3 notfixed=0 regressed=0
NEW FINDINGS: 3
