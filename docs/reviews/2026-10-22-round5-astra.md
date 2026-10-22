1. Reviewed `95bb74f..845af64` and the requested core, tests/examples, desktop and UI scope; read only the two authorized round4 files under `docs/reviews/`.
2. `cargo test --release -p bwr-core` PASS: 124 tests, zero failures.
3. `cargo build --release --workspace` and `npm --prefix ui run build` PASS; interactive desktop operation was not tested.
4. Both required examples PASS: validation completes all gates; startup `0.4` synchronizes at 5311 s and ends at 39.2% power at 5569 s without scram.
5. Prior-item assessments: fixed=19, partial=4, notfixed=0, regressed=0 across 23 rows; five findings below, all medium.
6. Verdict (a), works as intended: NO across the accepted domain; nominal trajectories pass, but cold discharge is not numerically converged and several hydraulic/boiling limits are physically wrong.
7. Verdict (b), physics appropriate for a BWRX-300 training simulator: NO for quantitative operator training; internal conservation and selected trajectories do not establish plant fidelity.

- (c) Resolve liquid/flashing hydraulics, pressure-dependent discharge integration, actual downstream pressure, gas transport and local core boiling pressure.
- (c) Add independent plant transient acceptance, mission-duration IC/pool depletion and replenishment cases, and limiting-state/timestep gates beyond the nominal turbine trip.
- (c) Qualify controls, protection, fuel/thermal-hydraulic limits, instruments and desktop/browser behavior against a defined training scope.

## Prior open items

The round4 reports introduce **R1-R3**, not N-numbered new sections. This table assesses their six R-findings separately, the three distinct identifiers left PARTIAL in their tables, and all fourteen carried N-identifiers to cover the requested N-number wording: 6 + 3 + 14 = 23 rows. Repeated carried identifiers are counted once. FIXED means the previously demonstrated failure is corrected, not that every operating limit of that subsystem is qualified. In particular, removal of the temperature-only pressure gates is fixed; distinct downstream-pressure, gas-transport and hydrostatic omissions are documented below. The new failures do not silently redefine the older findings.

| source | item | status | evidence |
| --- | --- | --- | --- |
| Both round4 tables, carried Sol | 22 | PARTIAL | `crates/bwr-core/examples/validation.rs:205`: startup acceptance passes at 0.461 within its 0.392-0.480 band, but there is still no independent plant transient reference. |
| Both round4 tables, carried Astra | 28 | PARTIAL | `crates/bwr-core/examples/validation.rs:94`: 1/2/3-train endpoints pass at 2.758/0.941/0.363 MPa after 1800 s; this does not test mission-duration pool depletion/replenishment or independent plant response. |
| Both round4 tables, carried round3-astra | R4 | FIXED | `crates/bwr-core/src/vessel.rs:690`, `:811` and `src/ic.rs:164`: the temperature-only main-flow/core gates are removed; isolated 99.99/100.01 C gas-bearing cases both deliver 1 kg/s with zero core void, and the prior uphill IC fixture now transfers zero heat. |
| round4-sol | R1 | FIXED | `crates/bwr-core/src/ic.rs:164`: the reproduced sealed 99.99 C primary/100 C pool fixture exports exactly 0 J at dt=0.05/0.025/0.0125 s; equal/hotter-pool and prescribed cooldown-crossing unit tests pass. |
| round4-sol | R2 | PARTIAL | `crates/bwr-core/src/bop.rs:98`, `:114`: line friction now has the correct inverse-density factor, but the valve retains the density-scaled steam law and no main-line flashing solve; R1 below demonstrates its wrong pressure dependence. |
| round4-sol | R3 | FIXED | `crates/bwr-core/src/vessel.rs:1312`, `:1353`: cold gas and compressed-liquid regressions pass; independent 90 C/0.255243 MPa check gives zero core void and IF97 boiling temperature 128.098866 C, while cold pressurized liquid now discharges. |
| round4-astra | R1 | FIXED | `crates/bwr-core/tests/plant.rs:1308`: coupled 90 C primary/100 C pools export zero heat; independent 99.99 C refinement also gives zero uphill transfer, with primary energy residual at most 7.89e-5 J. |
| round4-astra | R2 | FIXED | `crates/bwr-core/src/sim.rs:444`, `src/vessel.rs:811`: mechanical pressure replaces the cold override; 99.99/100.01 C gas-bearing boiling temperatures are continuous at 133.114512/133.125236 C and accepted main flow is 1 kg/s on both sides. |
| round4-astra | R3 | PARTIAL | `crates/bwr-core/src/bop.rs:1209`: same-flow liquid line loss is corrected to approximately 2.316 kPa, but the valve test bisects the same assumed steam equation; neither a liquid valve benchmark nor flashing hydraulics is implemented. |
| Both round4 tables, carried Sol | N1 | FIXED | `crates/bwr-core/tests/plant.rs:1062`, `:1091`: water-solid and vapor-limited IC conservation tests pass; independent gas-bearing IC loop residual stays within 4.95e-5 J under three timestep refinements. |
| Both round4 tables, carried Sol | N2 | FIXED | `crates/bwr-core/tests/plant.rs:1159`: delivered-flow telemetry test passes; independent starved-source probe commits 0.002000722641 kg/s with line/valve residuals below 3.3e-12 Pa / 3.1e-16 kg/s. |
| Both round4 tables, carried Sol | N3 | FIXED | `crates/bwr-core/tests/plant.rs:1130`, `:1145`: full-state/governor rollback and retained advance-debt regressions pass. |
| Both round4 tables, carried Sol | N4 | FIXED | `crates/bwr-core/tests/plant.rs:1015`: coupled hotwell-exhaustion test passes with prelimited vessel feed and matching committed condenser withdrawal. |
| Both round4 tables, carried Sol | N5 | FIXED | Isolated consistent condenser depletion rejects unchanged at 170.0/170.400/170.420 s for dt=1/0.05/0.01 s; maximum accepted equilibrium residual is 1.53e-5 J (`src/bop.rs:1482`). |
| Both round4 tables, carried Sol | N6 | FIXED | `crates/bwr-core/src/bop.rs:1554`: signed heater-energy regression passes; required validation's maximum relative whole-plant energy residual is 3.98e-13. |
| Both round4 tables, carried Sol | N7 | FIXED | Executed current browser mock: FullPower -> turbine trip -> 100 x 0.05 s gives turbine flow=0, extraction=0 MW and stored feed temperature=225.142195 C (`ui/src/sim/mock.ts:711`). |
| Both round4 tables, carried Sol | N8 | FIXED | `crates/bwr-core/tests/plant.rs:613`, `:652`: actual Rust/TypeScript contract and payload/enum/nullability mutation-rejection tests pass; UI production build also passes. |
| Both round4 tables, carried Astra | N1 | FIXED | `crates/bwr-core/tests/plant.rs:1091`: accepted IC scaling regression passes; independent gas-bearing loop energy residuals are -1.34e-5/-9.30e-6/-4.95e-5 J at dt=0.05/0.025/0.0125 s. |
| Both round4 tables, carried Astra | N2 | FIXED | `crates/bwr-core/tests/plant.rs:1015`: accepted feed/hotwell inventory test passes; the feed prelimit still precedes vessel integration (`src/sim.rs:473`). |
| Both round4 tables, carried Astra | N3 | FIXED | `crates/bwr-core/tests/plant.rs:1130`, `:1145`, `:1193`: governor, clock-debt and injected condenser-fault rollback tests all pass; reachable CW-loss protection test at `:1223` also passes. |
| Both round4 tables, carried Astra | N4 | FIXED | Required `cargo run --release -p bwr-core --example startup -- 0.4` exits 0: synchronizes at 5311 s, ends at 0.392 power without scram. |
| Both round4 tables, carried Astra | N5 | FIXED | `crates/bwr-core/src/bop.rs:1604`: coastdown regression passes initial delivery, monotone rundown and eventual check-valve closure from shaft speed. |
| Both round4 tables, carried Astra | N6 | FIXED | `crates/bwr-core/src/sim.rs:480`, `src/bop.rs:1164`: vessel receipt and heater debit still use the same delivered enthalpy; signed heater and whole-plant conservation gates pass. |

The injected condenser fault is correctly treated as rollback coverage, not a physically reachable plant dryout. No independent plant transient references were found or assumed. The acknowledged lack of a main-line two-phase/flashing model and dedicated liquid valve law remains a substantive limitation, not a completed fix.

## New findings

All fixtures below lie inside the model's accepted state domain. They are controlled limiting checks, not claims that each state can be reached from a stock scenario through operator commands alone. R1's reversed pressure response and R2's cold discharge overshoot arise with round5's hydraulic changes. R4 and R5 expose existing omissions in newly enabled cold operating paths, confirmed against `95bb74f`. R3 is a pre-existing remaining defect.

## R1 — Liquid valve scaling makes flow decrease as upstream pressure increases

- **severity:** medium
- **area:** physics
- **file:line:** crates/bwr-core/src/bop.rs:114
- **wrong:** The new multiplier makes valve mass flow proportional to `rho_liquid / rho_saturated_vapour(p)` times pressure drop. At fixed 40 C, open MSIVs, bypass shut and TCV=0.001, the isolated solver gives 14.245757 kg/s at 4 MPa, 13.665665 kg/s at 8 MPa and 10.919365 kg/s at 16 MPa. Line losses are only 6.97/6.40/4.07 Pa, so line friction cannot explain the decrease. Independent IF97 liquid densities are used in this fixture. The fixed-area nonflashing liquid limit instead scales as `sqrt(rho * delta_p)`: the 4 -> 8 MPa flow ratio is 1.424611, compared with the model's 0.959280. This is a limiting-law comparison, not a calibrated prediction of this plant's valve capacity. Liquid choking also requires a liquid vapor-pressure/recovery-factor model, not the density of hypothetical saturated steam at the upstream pressure. See [Emerson's liquid valve sizing discussion](https://www.emersonautomationexperts.com/2025/valves-actuators-regulators/calculations-for-sizing-control-valves/).
- **consequence:** Flooded discharge can respond in the wrong direction to rising pressure. The new density scaling fixes the leading line-friction dependence but does not make the valve relation physically appropriate; the added test verifies the chosen equation against itself.
- **fix:** Retain density-corrected single-phase line friction, but implement a separately calibrated liquid valve relation with an explicit cavitation/flashing limit and actual downstream pressure. Add independent fixed-opening pressure/density sweeps. Until supported, isolate or reject the liquid path transactionally.

## R2 — Cold liquid discharge overshoots its pressure boundary at the production timestep

- **severity:** medium
- **area:** correctness
- **file:line:** crates/bwr-core/src/vessel.rs:703
- **wrong:** The liquid withdrawal limit checks the entire liquid inventory, not the much smaller amount that can leave before pressure collapses or the phase changes. Round5's newly enabled cold flow and larger liquid conductance are then integrated explicitly for the whole step at `:742`. A consistent sealed, gas-free 40 C vessel with 249966.847432 kg, initial pressure 2.279631 MPa, both MSIVs open, manual bypass=1, feed off and a working 35 C condenser accepts a one-second coupled run. At dt=0.05/0.025/0.0125/0.00625/0.001 s it drains 292.548385/244.582063/242.166179/235.241615/234.628772 kg and ends at 7.363810/9.044382/31.471235/95.632643/101.325459 kPa. The production step drains 24.69% more than the fine result and creates a vapor space; the fine result remains water-solid. Every run reports no model error and per-step mass residual stays below 1.46e-11 kg. The standalone vessel/line check reproduces the overshoot, isolating it from protection/controller timing. These equations stop flow at 101.325 kPa and contain no line momentum store that could fund continued discharge below that boundary.
- **consequence:** The default timestep creates false depressurization, phase change and level recovery even while conservation tests pass. Smaller steps produce materially different states and subsequent protection/control inputs.
- **fix:** Couple outflow to the pressure/EOS update with an implicit solve or adaptive substeps that locate loss of driving pressure and phase-boundary events. Recompute flow and port phase at those events, and commit the integrated accepted transfers. Add this coupled open-bypass case to timestep acceptance; the new single 0.01 s cold-liquid test does not cover it.

## R3 — Main steam hydraulics still use atmosphere instead of condenser backpressure

- **severity:** medium
- **area:** physics
- **file:line:** crates/bwr-core/src/bop.rs:89
- **wrong:** Both valve paths use `p - P_ATM`, and `compute_flows_from` receives no downstream pressure. A consistent sealed, gas-free 90 C vessel has 70.182360 kPa dome pressure. With open MSIVs, manual bypass=1 and an available condenser at 5.928620 kPa, accepted main flow is exactly zero at dt=0.05/0.025/0.0125 s despite a 64.253740 kPa forward pressure difference and available vapor. The bypass remains fully open and no model limit is raised. Removing the temperature gate does not remove this separate atmospheric-pressure cutoff.
- **consequence:** Vacuum cooldown/discharge stops at an arbitrary atmospheric boundary. Near atmospheric conditions, bypass capacity and reactor/condenser pressure coupling are wrong even for a pure-vapor inlet.
- **fix:** Pass the condenser pressure into the bypass solve, and use a compressible valve relation that transitions between subcritical and choked flow from actual upstream/downstream pressures. Give the turbine path its appropriate downstream/stage-pressure boundary. Update delivered-flow reconciliation consistently and test zero differential, forward vacuum flow and reverse-flow prevention.

## R4 — Newly enabled cold main-line flow leaves all noncondensable gas trapped

- **severity:** medium
- **area:** physics
- **file:line:** crates/bwr-core/src/vessel.rs:744
- **wrong:** Closing the head vent freezes `gas_mass` even when the main steam line is open; all main-line mass and enthalpy are treated as pure water vapor. In a sealed 90 C, 180000 kg water/100 kg air vessel with open MSIVs and bypass, the initial dome air-to-water-vapor mass ratio is 3.615915. One second exports 2.830646/2.830634/2.830628 kg of water at dt=0.05/0.025/0.0125 s while retaining exactly 100 kg of air. The condenser receives no corresponding vessel-air flow. An open bulk-flow pipe does not selectively retain the noncondensable component of this gas mixture. The isolated water-energy balance nevertheless closes within 2.51e-5 J. Rebuilding `95bb74f` and executing the same fixture yields zero main-line discharge; round5 enables the unsupported cold mixed-gas path.
- **consequence:** Gas-bearing startup/blowdown cannot purge air through the main line, vessel pressure retains an artificial gas contribution, and condenser air loading/vacuum response is understated. Water-only conservation does not detect the missing species transport.
- **fix:** Model main-line gas mixture density, composition and enthalpy; debit air and water separately and deliver air to the condenser noncondensable balance. Treat a closed head vent independently from other open gas exits. If mixed-gas discharge is outside scope, reject/isolate it explicitly and test that boundary.

## R5 — Cold core boiling uses dome pressure without the hydrostatic head above the fuel

- **severity:** medium
- **area:** physics
- **file:line:** crates/bwr-core/src/vessel.rs:811
- **wrong:** `core_sat()` now correctly uses total dome pressure but applies it at every core elevation. In the consistent gas-free 90 C, 180000 kg fixture, liquid level is 20.525167 m and top of active fuel is 8.93 m. Dome pressure is 70.182360 kPa; the initially liquid column gives approximately 179.984375 kPa at the top of the fuel, with still higher pressure lower down. At 500 kg/s and 8.7 MW, the current axial model predicts average void 0.625810 and exit quality 0.007623. Even at the lowest core hydrostatic pressure, independent IF97 gives saturation temperature 116.909716 C and saturated-liquid enthalpy 490.656971 kJ/kg, whereas inlet enthalpy plus the entire core heat input is only 394.368444 kJ/kg. The corresponding liquid-column boiling check produces zero void and quality. At these low velocities, the omitted roughly 110 kPa static head cannot be treated as a small rated-pressure correction. The same probe on `95bb74f` produces zero void; its incorrect atmospheric substitute happened to suppress this particular cold failure.
- **consequence:** Expanding the core model into gas-free sub-100 C states introduces premature boiling, large void feedback and level swell. The gas-bearing regression tests pass because trapped-air pressure suppresses boiling, so they do not exercise this limit.
- **fix:** Use local axial mechanical pressure, including the hydrostatic column and supported flow-loss terms, for boiling and phase properties; keep dome saturation separate from core saturation. Add low-pressure gas-free tests where inlet enthalpy plus axial heat remains below the local boiling threshold. If a lumped dome-pressure approximation is retained, constrain its supported operating envelope explicitly.

## Verification evidence and limits

All five required commands exited 0 on `845af64`. Logs are `/tmp/bwrsim-round5-astra/{test,build,ui,validation,startup}.log`. The release test/build output uses the requested RTK compression; individual physical findings above also have separately executed numerical evidence. The required validation reports maximum absolute whole-plant mass residual 1.86e-9 kg and maximum relative energy residual 3.98e-13. Turbine-trip refinement changes peak pressure by +0.0012 MPa and minimum level by -0.02 cm. Cold-condenser final temperatures are 20.163016/20.160441/20.160440 C at dt=60/1/0.05 s. These demonstrate conservation and selected numerical convergence, not general physical qualification; R2 is a direct counterexample outside the existing refinement case.

Current probes and results are `/tmp/bwrsim-round5-astra/src/main.rs` and `probes.log`; run them with `cargo run --release --offline --manifest-path /tmp/bwrsim-round5-astra/Cargo.toml`. The prior revision was extracted using `git archive 95bb74f crates/bwr-core` into that directory's `baseline/`; its separate probe and output are `baseline-probe/` and `baseline.log`. No other review documents were extracted or read. The reported hydrostatic and liquid-valve comparisons are independent limiting calculations, not independent plant transient references.

Independent property checks use Python `iapws` in `/tmp/iapwsenv`, with source and results in `if97.py` and `if97.log`. They implement the separately published [IAPWS-IF97 formulation](https://iapws.org/technical-guidance/release/IF97-Rev). Besides the boiling references above, the liquid-inlet reversible drops remain 145330.790 J/kg for 200 C/3.834878 MPa -> 5.609667 kPa and 37216.458 J/kg for the 110 C fixture; the current liquid work tests pass. The standalone condenser dryout remains a component check, and injected plant dryout remains fault-injection coverage, as requested.

The current mock was bundled and executed without its timer; source/output are `mock-probes.cjs` and `mock-probes.log`. Gas-bearing core checks at 90/110/150 C retain zero void/exit quality, turbine-trip extraction falls to zero, and fractional rod targets/stop positions are preserved. No new round5 defect was found in the scoped desktop bridge, TypeScript contract, RodControls or TopBar. Interactive emission timing and operator exercises were not tested. The mock is a UI development approximation, not an independently qualified physics backend.

Only this requested review file was written in the repository. Production source and tests were not changed; validation traces are ignored and pre-existing untracked files were left untouched.

OPEN: fixed=19 partial=4 notfixed=0 regressed=0
NEW FINDINGS: 5
