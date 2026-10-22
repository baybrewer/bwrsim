1. Reviewed `1024239..c824f72` in the requested scope; `cargo test --release -p bwr-core` PASS: 65 unit tests, 28 integration tests, zero failures.
2. `cargo build --release --workspace` and `npm --prefix ui run build` both PASS; no interactive desktop smoke test was performed.
3. Required validation PASS: five cases plus dt refinement; required startup `-- 0.4` PASS: synchronized at 5311 s, n=0.392 at 5569 s, no scram.
4. Prior open-item assessments: FIXED=42, PARTIAL=12, NOT FIXED=0, REGRESSED=0; 54 source-specific rows, including overlapping checklist references; NEW FINDINGS=5.
5. Verdict (a), works as intended: NO across the supported state/time domain; isolated probes expose transfer-energy and condenser-state failures despite the passing nominal runs.
6. Verdict (b), physics appropriate for a BWRX-300 training simulator: NO for quantitative transient training; state/port consistency and independent response acceptance remain incomplete.
7. Verdict (c), work remaining before training-grade use:

- Close gas-dependent and compressed-liquid transfer properties, dry-condenser limits, and the known IC return mixing/quench omission.
- Make physical domain violations fail validation; add independent IC train, startup, depletion, transient and endurance acceptance with explicit tolerances.
- Validate protection/control response and instrument semantics against a documented plant basis; define and enforce the lumped fuel/thermal model's operating envelope.

## Prior open items

`source` identifies the supplied re-review, while `item` preserves its original Sol/Astra number or its own N-number. Every PARTIAL row from both supplied files and all fourteen N-findings appear below. Counts are assessments per source, not deduplicated defect counts. FIXED applies to the stated prior failure, not every behavior of the subsystem; newly introduced failures are separately numbered below.

| source | item | status | evidence |
| --- | --- | --- | --- |
| Sol re-review | Sol 5 | FIXED | `crates/bwr-core/src/vessel.rs:354`: independent IF97 joint M/V/U solves agree within 0.00013 C and 0.00047 MPa; executed cold sealed/vented tests now distinguish pressure. New transfer-property defect is R1, not the former EOS-pressure defect. |
| Sol re-review | Sol 6 | FIXED | `crates/bwr-core/tests/plant.rs:1103`: executed three rejected accelerating-turbine retries preserve physical/controller state; governor is restored at `sim.rs:436`. |
| Sol re-review | Sol 8 | FIXED | `crates/bwr-core/src/sim.rs:429`: vessel and heater use the same delivered enthalpy; executed heater identity and five whole-plant cases close, including scram/isolation; extraction uses accepted steam at `:459`. |
| Sol re-review | Sol 9 | PARTIAL | `crates/bwr-core/src/bop.rs:572`: relief now debits steam mass and enthalpy, but dry-shell limiting checks violate phase-volume/energy closure (R2); reject or model the all-vapour regime. |
| Sol re-review | Sol 14 | FIXED | `crates/bwr-core/tests/plant.rs:975`: executed breaker-only load rejection reaches at least 90% bypass opening in 0.30 s without turbine trip; latch and TCV fast closure are exercised. |
| Sol re-review | Sol 15 | FIXED | `crates/bwr-core/tests/plant.rs:72`: coupled first-10-s SCCRI acceptance passes; isolated dt=0.05/0.025/0.0125 s gives 0.80121/0.80445/0.80615 percentage point/s against the documented 0.8 +/- 0.3 band. |
| Sol re-review | Sol 20 | FIXED | `crates/bwr-core/src/kinetics.rs:80`: both public kinetics entry points validate time/source/reactivity before mutation; executed invalid-input regression rejects the prior negative-dt case. |
| Sol re-review | Sol 22 | PARTIAL | `crates/bwr-core/examples/validation.rs:108`: whole-plant inventories and dt refinement now run, but the new gate credits numerical correction as boundary energy (R4); startup case still has no criticality/synchronization assertion or independent IC-train endpoint acceptance. |
| Sol re-review | Sol 24 | FIXED | `crates/bwr-core/tests/plant.rs:652`: executed payload rename, both nullability directions, enum add/remove/rename and nullable fault-type mutations all fail; full enum sets and TypeScript-directed Rust probes pass on production types. |
| Sol re-review | Astra 4 | FIXED | `crates/bwr-core/src/steam.rs:172`: pressure-dependent Region 1 volume/internal-energy closure gives 201.858650 C/16.274242 MPa versus independent 201.858653 C/16.273779 MPa; separate outlet-enthalpy omission is R3. |
| Sol re-review | Astra 5 | FIXED | `crates/bwr-core/src/bop.rs:920`: heater storage debits the same h_feed_delivered the vessel receives; executed signed heater identity and coupled scram/isolation plant balances pass. |
| Sol re-review | Astra 6 | FIXED | `crates/bwr-core/src/params.rs:36`: calibrated 82 s run-in has an explicit coupled acceptance band; isolated first-10-s reduction converges to about 0.806 percentage point/s under dt refinement. |
| Sol re-review | Astra 9 | FIXED | `crates/bwr-core/src/vessel.rs:903`: executed fixed-mass cold heating distinguishes sealed and vented pressure, retains trapped gas, and accounts for vented gas mass; new steam-property coupling failure is R1. |
| Sol re-review | Astra 10 | FIXED | `crates/bwr-core/tests/plant.rs:1103`: disconnected turbine reference/governor and other pre-vessel states remain identical through repeated EOS rejection; failed time debit is separately covered at `:1118`. |
| Sol re-review | Astra 18 | FIXED | `ui/src/sim/mock.ts:277` and `ui/src/components/RodControls.tsx:78`: MOVE TO preserves the entered fraction; executed browser commands retain target=12.25 and STOP at exactly 12.6. |
| Sol re-review | Astra 19 | PARTIAL | `crates/bwr-core/src/sim.rs:447`: accepted IC transfers now commit consistently in the no-gas depletion tests; `vessel.rs:739` still mixes only feedwater, omitting accepted IC return from the local chimney/downcomer closure. Add that mixing/quench contribution and coupled acceptance tests. |
| Sol re-review | Astra 20 | FIXED | `crates/bwr-core/tests/plant.rs:988`: executed 20 s hotwell-exhaustion check closes combined inventory; `sim.rs:422` limits feed before vessel integration and condenser accepts the same withdrawal. |
| Sol re-review | Astra 22 | FIXED | `crates/bwr-core/src/kinetics.rs:80`: negative/nonfinite dt and source reject before both step and advance mutate state; executed release invalid-input tests pass. |
| Sol re-review | Astra 26 | FIXED | `crates/bwr-core/tests/plant.rs:468`: TypeScript types now drive nullable/scalar probes through Rust deserialization; exhaustive enum comparison and all seven formerly escaping mutation cases pass their rejection tests. |
| Sol re-review | Astra 28 | PARTIAL | `crates/bwr-core/examples/validation.rs:307`: turbine-trip dt refinement and whole-plant bookkeeping pass; independent transient/IC train acceptance and actual startup outcomes remain absent from this runner, and R4 masks domain energy errors. |
| Sol re-review | N1 | FIXED | `crates/bwr-core/tests/plant.rs:1035` and `:1064`: executed water-solid and vapour-limited vessel-plus-IC tests close mass and commit only accepted steam; the new trapped-gas enthalpy mismatch is isolated separately in R1. |
| Sol re-review | N2 | PARTIAL | `crates/bwr-core/src/sim.rs:459`: extraction is now funded by accepted steam, but isolated flooded-vessel step publishes 467.478611 kg/s in Steam versus 0.002000707 kg/s actually delivered. Commit accepted TCV/bypass values for snapshots and the feed controller (`:418`, `:913`). |
| Sol re-review | N3 | FIXED | `crates/bwr-core/tests/plant.rs:1103`: executed repeated rejected hot-standby turbine-roll steps leave governor/speed reference and physical state unchanged. |
| Sol re-review | N4 | FIXED | `crates/bwr-core/tests/plant.rs:988`: the original 20 s hotwell-limited reproduction passes combined inventory closure with zero withdrawal shortfall after shared-port limiting. |
| Sol re-review | N5 | PARTIAL | `crates/bwr-core/src/bop.rs:572`: the wet-node relief mass/enthalpy regression passes, but the required inventory-exhaustion limit is still invalid: at dt=0.05 s the dry-node EOS residual reaches 10.865 MJ by 0.65 s (R2). |
| Sol re-review | N6 | FIXED | `crates/bwr-core/src/bop.rs:1188`: executed heater identity closes below 0.001 J/step through extraction loss and delivery limiting; whole-plant manual-scram/isolation cases close at roundoff. |
| Sol re-review | N7 | FIXED | `ui/src/sim/mock.ts:559`, `:577`, `:675`: executed browser probes latch fuel-envelope crossing, report empty pools and nonzero operating heater heat; boiloff and condensate return now derive from duty. New gas telemetry omission is R5. |
| Sol re-review | N8 | FIXED | `crates/bwr-core/tests/plant.rs:583`: all four wire-enum sets are compared in both directions; executed refuel rename/removal and bogus-member mutations are rejected. |
| Astra re-review | Sol 5 | FIXED | `crates/bwr-core/src/vessel.rs:354`: independent IF97 joint M/V/U solves agree within 0.00013 C and 0.00047 MPa; executed cold sealed/vented tests now distinguish pressure. New transfer-property defect is R1, not the former EOS-pressure defect. |
| Astra re-review | Sol 8 | FIXED | `crates/bwr-core/src/sim.rs:429`: vessel and heater use the same delivered enthalpy; executed heater identity and five whole-plant cases close, including scram/isolation; extraction uses accepted steam at `:459`. |
| Astra re-review | Sol 9 | PARTIAL | `crates/bwr-core/src/bop.rs:572`: relief now debits steam mass and enthalpy, but dry-shell limiting checks violate phase-volume/energy closure (R2); reject or model the all-vapour regime. |
| Astra re-review | Sol 14 | FIXED | `crates/bwr-core/tests/plant.rs:975`: executed breaker-only load rejection reaches at least 90% bypass opening in 0.30 s without turbine trip; latch and TCV fast closure are exercised. |
| Astra re-review | Sol 15 | FIXED | `crates/bwr-core/tests/plant.rs:72`: coupled first-10-s SCCRI acceptance passes; isolated dt=0.05/0.025/0.0125 s gives 0.80121/0.80445/0.80615 percentage point/s against the documented 0.8 +/- 0.3 band. |
| Astra re-review | Sol 17 | FIXED | `crates/bwr-core/examples/startup.rs:49`: executed startup commands rated rpm, synchronizes at 5311 s, reaches n=0.392 at 5569 s without scram, and now fails its process if the required outcome is missed. |
| Astra re-review | Sol 21 | PARTIAL | `crates/bwr-core/tests/plant.rs:1118`: failed advance retains the complete 0.1 s backlog; `src-tauri/src/lib.rs:42` preserves worker backlog, but Snapshot still omits backlog/effective speed. Expose those readouts to finish this checklist item. |
| Astra re-review | Sol 22 | PARTIAL | `crates/bwr-core/examples/validation.rs:108`: whole-plant inventories and dt refinement now run, but the new gate credits numerical correction as boundary energy (R4); startup case still has no criticality/synchronization assertion or independent IC-train endpoint acceptance. |
| Astra re-review | Sol 23 | FIXED | `crates/bwr-core/src/vessel.rs:863`: fixed external M/V/U fixtures replace the compressed-state self-roundtrip; independent Python IF97 reconstruction reproduced both reference solutions within the declared tolerances. |
| Astra re-review | Sol 24 | FIXED | `crates/bwr-core/tests/plant.rs:652`: executed payload rename, both nullability directions, enum add/remove/rename and nullable fault-type mutations all fail; full enum sets and TypeScript-directed Rust probes pass on production types. |
| Astra re-review | Astra 4 | FIXED | `crates/bwr-core/src/steam.rs:172`: pressure-dependent Region 1 volume/internal-energy closure gives 201.858650 C/16.274242 MPa versus independent 201.858653 C/16.273779 MPa; separate outlet-enthalpy omission is R3. |
| Astra re-review | Astra 5 | FIXED | `crates/bwr-core/src/bop.rs:920`: heater storage debits the same h_feed_delivered the vessel receives; executed signed heater identity and coupled scram/isolation plant balances pass. |
| Astra re-review | Astra 6 | FIXED | `crates/bwr-core/src/params.rs:36`: calibrated 82 s run-in has an explicit coupled acceptance band; isolated first-10-s reduction converges to about 0.806 percentage point/s under dt refinement. |
| Astra re-review | Astra 9 | FIXED | `crates/bwr-core/src/vessel.rs:903`: executed fixed-mass cold heating distinguishes sealed and vented pressure, retains trapped gas, and accounts for vented gas mass; new steam-property coupling failure is R1. |
| Astra re-review | Astra 17 | FIXED | `crates/bwr-core/examples/startup.rs:52`: rated-rpm synchronization now succeeds in the required 0.4 startup run; final power n=0.392, stage 2, no scram, process exit 0. |
| Astra re-review | Astra 18 | FIXED | `ui/src/sim/mock.ts:277` and `ui/src/components/RodControls.tsx:78`: MOVE TO preserves the entered fraction; executed browser commands retain target=12.25 and STOP at exactly 12.6. |
| Astra re-review | Astra 19 | PARTIAL | `crates/bwr-core/src/sim.rs:447`: accepted IC transfers now commit consistently in the no-gas depletion tests; `vessel.rs:739` still mixes only feedwater, omitting accepted IC return from the local chimney/downcomer closure. Add that mixing/quench contribution and coupled acceptance tests. |
| Astra re-review | Astra 21 | PARTIAL | `crates/bwr-core/src/sim.rs:214`: failed steps no longer consume DT; executed backlog regression retains 0.1 s. Snapshot still provides neither backlog nor effective speed, the remaining readout requirement in this source. |
| Astra re-review | Astra 26 | FIXED | `crates/bwr-core/tests/plant.rs:468`: TypeScript types now drive nullable/scalar probes through Rust deserialization; exhaustive enum comparison and all seven formerly escaping mutation cases pass their rejection tests. |
| Astra re-review | Astra 28 | PARTIAL | `crates/bwr-core/examples/validation.rs:307`: turbine-trip dt refinement and whole-plant bookkeeping pass; independent transient/IC train acceptance and actual startup outcomes remain absent from this runner, and R4 masks domain energy errors. |
| Astra re-review | N1 | FIXED | `crates/bwr-core/src/sim.rs:403` and `:447`: request/accept/commit replaces premature train mutation; both original no-gas phase-limited integrated regressions pass. R1 covers the newly introduced gas-dependent energy error. |
| Astra re-review | N2 | FIXED | `crates/bwr-core/src/sim.rs:422`: vessel feed is bounded by hotwell supply before integration; executed depletion test verifies combined inventory and matching accepted withdrawal. |
| Astra re-review | N3 | FIXED | `crates/bwr-core/tests/plant.rs:1103` and `:1118`: executed governor transaction and failed-advance tests preserve physical/controller state and all owed clock time. |
| Astra re-review | N4 | FIXED | `crates/bwr-core/examples/startup.rs:49`: required startup run synchronizes at 5311 s and reaches 39.2% power at 5569 s; final outcome gate exits nonzero on failure. |
| Astra re-review | N5 | FIXED | `crates/bwr-core/src/bop.rs:772`: hydraulics now includes coasting shaft speeds; executed pump-trip test retains more than 90% of preceding total flow on the first scan and checks eventual check-valve closure. |
| Astra re-review | N6 | FIXED | `crates/bwr-core/src/sim.rs:429`: vessel receives the exact pre-step enthalpy debited by heater storage; signed component and whole-plant isolation balances pass. |

## New findings

## R1 — Trapped gas makes the vessel and IC disagree on steam enthalpy

- **severity:** high
- **area:** physics
- **file:line:** crates/bwr-core/src/sim.rs:403
- **wrong:** Round 2 correctly requests IC duty at the vessel temperature, but `Vessel::sat()` at `vessel.rs:602` still evaluates saturation at total pressure, including air. The vessel debits that enthalpy at `:708`, while IC storage/pool heat receive `sat_t(T).h_g`. Isolated HotStandby setup: M=180000 kg, T=150 C, trapped air=50 kg, sealed head, closed MSIVs, no feed, all IC valves open with 500 kg tube inventories and preceding rated duty. Gas partial pressure is 109333.537 Pa; the two steam enthalpies are 2755067.857 and 2745919 J/kg. A one-second vessel+heater+condenser+IC-primary balance loses 432487.081 J at dt=0.05 s, 432569.007 J at 0.025 s and 432609.862 J at 0.0125 s. The sum of accepted IC flow times the enthalpy disagreement predicts each residual to better than 0.001 J.
- **consequence:** The newly supported trapped-gas state destroys energy in a connected cooling loop; timestep refinement does not remove the defect. Current validation scenarios start with no trapped gas and miss it.
- **fix:** Define the steam transfer properties from the water-vapour equilibrium state separately from total mechanical pressure. Pass one identical accepted steam enthalpy to vessel withdrawal, every IC train and the BOP. Add the sealed-gas whole-plant residual case at multiple timesteps.

## R2 — Condenser relief applies a wet-node formula after the liquid is exhausted

- **severity:** medium
- **area:** physics
- **file:line:** crates/bwr-core/src/bop.rs:572
- **wrong:** The relief derivation assumes remaining saturated liquid, so removing water changes equilibrium energy by `m*h_f`. Once the node is all vapour, `condenser_vapour_mass` clamps vapour to total mass, but the relief code still uses h_fg and forcibly sets the relief temperature; `p_steam` remains saturation pressure even when the water cannot fill the 3000 m3 shell at saturation density. Isolated no-CW/no-air condenser at 105.164407 C, zero hotwell liquid, q_in=100 MW, w_in=w_out=0 (automatic makeup retained), dt=0.05 s: by 0.65 s it holds 2101.393348 kg against a required saturated vapour mass of 2126.200066 kg, yet reports 121590 Pa; stored E differs from the model's own E(T,M) by +10.865098 MJ. A one-second 5 GW limiting pulse exhausts all water but still reports 121590 Pa and leaves +4.558493 GJ outside E(T,M), with energy_residual_j=0.
- **consequence:** Loss-of-inventory cooling/relief can continue from an impossible pressure/temperature/energy state. Mass and port-energy bookkeeping alone passes because it does not reconstruct the thermodynamic state.
- **fix:** Detect the dry boundary and either reject the step transactionally or solve a supported single-phase vapour EOS. Solve relief against the actual remaining phase and energy, without imposing the wet-node temperature after depletion. Test E(T,M), occupied volume, pressure and mass/energy together through dryout and complete exhaustion.

## R3 — Compressed-liquid outlets still use saturated-liquid enthalpy

- **severity:** medium
- **area:** physics
- **file:line:** crates/bwr-core/src/vessel.rs:646
- **wrong:** Round 2 stores compressed-liquid energy using Region 1, but RWCU and water-solid head-vent/main-line withdrawals still use h_f(T), ignoring the solved pressure. Isolated water-solid inventory M=1.002*V*rho_f(40 C) gives 4.562267 MPa; a 10 kg/s liquid withdrawal exports 167541 J/kg, while independent IF97 at that same T/P gives 171573.418 J/kg, a 40.324 kW missing energy export. The step succeeds. At 200 C the analogous 3.834878 MPa state differs by 926.499 J/kg. These errors are much larger than the saturation-table interpolation error.
- **consequence:** Compressed-vessel draining/depressurization now evolves a pressure-dependent stored-energy state using incompatible boundary energy, biasing temperature and pressure response even below vessel design pressure in the cold example.
- **fix:** Supply compressed-liquid h=u+p*v from the same Region 1 state/anchoring used by the EOS to all liquid ports and to BOP delivery when the main line carries liquid. Add independent T/P enthalpy and finite withdrawal checks rather than balancing against the same incorrect h_f port.

## R4 — The new conservation gate cancels numerical energy corrections

- **severity:** medium
- **area:** tests
- **file:line:** crates/bwr-core/examples/validation.rs:126
- **wrong:** `plant_counters` subtracts condenser `energy_residual_j` as though it were a physical external energy transfer. It is actually the energy created/discarded by the temperature clamp at `bop.rs:583`, and no acceptance assertion requires it to remain zero. Reproduction through the public plant API: ColdShutdown, both CW pumps on, condenser initialized at 35 C, `step(60.0)`. It returns true, advances to 60 s with no model error, clamps the condenser to 1 C and creates 16.719039 GJ relative to actual ports; energy_residual_j=-16.719039 GJ cancels the discrepancy exactly under the new gate's arithmetic. At dt=20 s the explicit CW step already cools to 12.329 C, below its warmer inlet boundary, without a domain error.
- **consequence:** A reported whole-plant conservation PASS can certify a numerical heat source/sink. Supported large timesteps can silently produce unphysical cooling, while the new acceptance calculation hides rather than detects it.
- **fix:** Exclude numerical correction counters from physical boundary exchanges and fail on nonzero per-step correction beyond roundoff. Integrate condenser CW exchange with a stable method or subdivide supported large steps; propagate an unrepresentable condenser state to plant rollback/model_error. Add the public 60 s limiting check and dt refinement of the cooling response.

## R5 — Browser trapped-gas telemetry freezes after the vent closes

- **severity:** medium
- **area:** contract
- **file:line:** ui/src/sim/mock.ts:706
- **wrong:** The new headGasPressureMpa is only assigned while vented; once sealed it is never recomputed from retained mass and temperature, and pressureMpa ignores it. Executed browser sequence: load coldShutdown, step once at 40 C, close the head vent, set coolant to 90 C and step again. Trapped mass remains 110.624411 kg; the reported gas pressure remains 0.099422655 MPa although the mock's own constant-volume ideal-gas relation gives 0.115297256 MPa. Total pressure remains 0.101 MPa, less than gas partial pressure alone. The added TypeScript contract explicitly says total pressure equals vapour plus gas.
- **consequence:** Browser clients receive internally contradictory required telemetry; closing the vent suppresses the intended sealed-gas pressure response regardless of later heating.
- **fix:** Initialize gas state on scenario load, preserve its mass while sealed, recompute its partial pressure every step from current temperature/volume, and include it in total pressure. Add a vent-close/heat/reopen browser state-transition test checking both the ideal-gas relation and total-pressure composition.

## Verification evidence

All five required commands exited 0. Logs and review-only executable probes are under `/tmp/bwrsim-round2-review/`: `test.log`, `build.log`, `ui.log`, `validation.log`, `startup.log`, `src/main.rs`, `probes.log`, `mock-probes.cjs`, `mock-probes.log`, and `if97.log`. Reproduce the Rust checks with `cargo run --release --offline --manifest-path /tmp/bwrsim-round2-review/Cargo.toml`; browser probes use the bundled current mock without starting its timer. No production source or tests were edited.

The required validation runner's largest whole-plant mass residual was 9.31e-10 kg and largest relative energy residual was 5.58e-13. Turbine-trip dt=0.05 to 0.025 s changed peak pressure by +0.0012 MPa and minimum level by +0.02 cm. Those results apply to its five selected trajectories; they do not exercise R1/R2/R4.

Independent thermodynamic checks used Python iapws 1.5.5 and scipy root solves for fixed mass, geometric volume and total fluid-plus-metal energy under [IAPWS-IF97](https://iapws.org/technical-guidance/release/IF97-Rev). The two compressed-state reference solutions are 201.858652634 C/16.273779245 MPa and 288.846655310 C/13.408130159 MPa; production gives 201.858650376 C/16.274241548 MPa and 288.846533741 C/13.408483742 MPa. These verify the revised storage EOS; R3 concerns the separate outgoing enthalpy.

The approximately 0.8 percentage point/s SCCRI basis and 300 ms bypass response were checked against sections 3.2 and 5.1.4 of [GE 005N9751 Rev J, June 2026](https://www.gevernova.com/content/dam/gevernova-nuclear/global/en_us/documents/carbon-free-power/005N9751-BWRX-300-General-Description.pdf). Matching those limited public design figures does not provide independent plant transient validation. The known missing IC return mixing remains Astra 19 in the table and is not counted again as a new finding. Likewise, uncommitted accepted steam readouts remain Sol re-review N2.

OPEN: fixed=42 partial=12 notfixed=0 regressed=0
NEW FINDINGS: 5
