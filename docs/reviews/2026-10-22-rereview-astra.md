1. Reviewed `1a1dc4f..102423942e684c786a18d4e740667de5a47a8822` in the requested engine, examples/tests, desktop bridge, and UI contract/mock scope against all 57 prior findings.
2. `cargo test --release -p bwr-core`: PASS, 46 unit tests, 19 integration tests, zero failures, zero doctests.
3. `cargo build --release --workspace`: PASS; no interactive desktop smoke test was performed.
4. `npm --prefix ui run build`: PASS, including TypeScript checking and Vite production output.
5. `cargo run --release -p bwr-core --example validation`: PASS, all five cases; generated CSV traces under `docs/validation/traces`; its conservation acceptance covers the vessel, not the whole plant.
6. Sol checklist: FIXED=18, PARTIAL=10, NOT FIXED=0, REGRESSED=0, total=28.
7. Astra checklist: FIXED=19, PARTIAL=10, NOT FIXED=0, REGRESSED=0, total=29.
8. Independent checks included IF97 compressed-liquid reconstruction, combined inventory/energy balances, timestep refinement, executed browser commands, schema mutations, and startup execution; six new actionable defects are recorded below.
9. Verdict (a), works-as-intended: NO; startup no longer synchronizes, rejected steps mutate state, and coupled transfers can violate inventory and energy conservation despite passing tests.
10. Verdict (b), physics appropriate for a BWRX-300 training simulator: NO; cold sealed/compressed-liquid closure, condenser phase/relief behavior, transient control fidelity, and independent physical acceptance remain incomplete.

## Prior findings

FIXED means the original defect is corrected and was checked by an executed test or isolated reproduction. PARTIAL identifies the remaining unmet part, not merely an absent comment. A distinct regression introduced by a fix is listed under New findings rather than automatically changing the original defect's classification to REGRESSED. Numbers refer to the two supplied review files. Temporary probes exercised the committed implementation without production edits.

| review | # | status | evidence |
| --- | --- | --- | --- |
| Sol | 1 | FIXED | `crates/bwr-core/src/kinetics.rs:69`: adaptive `advance` resolves positive prompt growth and raw invalid solves return errors; independent 100,000-step checks at rho=0.045/0.05 gave n=2.935435/3.315760 versus 2.905226/3.277505, errors 1.04%/1.17%, rather than extinction; singularity and core tests pass. |
| Sol | 2 | FIXED | `crates/bwr-core/src/sim.rs:607`: isolated REFUEL checks rejected a two-rod batch and a second queued rod, allowed continued motion of the same rod and insertion-only batches, and preserved fractional targets. |
| Sol | 3 | FIXED | `crates/bwr-core/src/sim.rs:483`: L8 pump protection is independent of turbine state and restart is blocked; `l8_feed_pump_trip_is_independent_of_turbine_state` passes for hot standby and full power. |
| Sol | 4 | FIXED | `crates/bwr-core/src/vessel.rs:499`: coverage is geometric mixture height; executed coverage test returns 0.6 at 60% active height for void fractions 0, 0.4, and 0.7. |
| Sol | 5 | PARTIAL | `crates/bwr-core/src/vessel.rs:246` and `:253`: compression pressure is added while energy still uses saturated internal energy; at M=219799.661608 kg and E=241960515187.185 J, model gives 200 C/15.840386 MPa versus independent IF97 201.858653 C/16.273779 MPa. Identical vented/sealed 40-to-90 C heatups both end at 101325 Pa with zero vent flow. Solve pressure-dependent liquid energy/volume and the gas boundary, or reject those unsupported states. |
| Sol | 6 | FIXED | `crates/bwr-core/src/vessel.rs:274`: EOS now rejects both energy-bracket violations, excess compression, and superheat; endpoint and vessel rollback tests pass. The distinct simulator rollback defect is N3. |
| Sol | 7 | FIXED | `crates/bwr-core/src/vessel.rs:548`: isolated 1000 kg/40 C vessel with 2000 kg/s liquid demand for 1 s ends at 12.908523 kg, accepts 987.091477 kg, and has zero mass residual; no 1000 kg floor. Newly introduced IC acceptance mismatch is N1. |
| Sol | 8 | PARTIAL | `crates/bwr-core/src/bop.rs:747`: finite heater storage and extraction limits eliminate the old unbounded source; isolation test passes, but pre/post-step feed enthalpies still disagree, giving -1.611203 MJ whole-loop residual over 120 s at dt=0.05; N6. |
| Sol | 9 | PARTIAL | `crates/bwr-core/src/bop.rs:465`: water/metal storage and CW heat removal now exist, but no vapor inventory or relief mass does. At 105.164407 C with no CW and 470 kg/s steam in/liquid out, one 0.05 s step exports 1.054087 GW through `q_relief_w` while preserving hotwell mass. Model relief mass/enthalpy and phase inventory, or halt at this boundary. |
| Sol | 10 | FIXED | `crates/bwr-core/src/reactor.rs:376`: rods and kinetics now advance together; prescribed scram over 0.05 s gives n=0.066944260 with plant steps 0.05 and 0.001, versus 0.066943556 at 0.0001; committed rod worth agrees with positions. |
| Sol | 11 | FIXED | `crates/bwr-core/src/ic.rs:114`: sensible heating and latent boiloff share one accepted budget; boiling/depletion/empty-pool tests pass; independent original two-step boiling fixture has only 0.000100 J residual rather than approximately 1.06 MJ. |
| Sol | 12 | FIXED | `crates/bwr-core/src/sim.rs:586`: reset explicitly refuses SHUTDOWN; executed mode-switch test verifies reset remains blocked after insertion completes and succeeds after selecting another mode. |
| Sol | 13 | FIXED | `crates/bwr-core/src/sim.rs:443`: condenser receives accepted vessel steam enthalpy; `steam_line_is_isenthalpic` passes and rejects the old chest-resaturation difference greater than 1 kJ/kg. |
| Sol | 14 | PARTIAL | `crates/bwr-core/src/sim.rs:379` and `crates/bwr-core/src/bop.rs:118`: turbine-trip opening test passes, but OpenBreaker asserts fast-open for only one step and demand follows slow TCV closure; full-power rejection reaches bypass=0.164976 at 0.30 s and 0.549240 at 1 s. Latch the rejection stroke/demand until completion and test rejection separately against the published 300 ms response. |
| Sol | 15 | PARTIAL | `crates/bwr-core/src/params.rs:32`: SRI now has a separate 60 s actuator, but the only dedicated test asserts slower-than-scram travel. Coupled trip power falls from 99.2820% to 97.3551% at 1 s and 87.2263% at 10 s, averaging 1.927 and 1.206 percentage points/s; refined steps reproduce this. Establish an accepted SCCRI time-history/rate envelope around the published approximate 0.8 percentage point/s response rather than accepting actuator slowness alone. |
| Sol | 16 | FIXED | `crates/bwr-core/src/params.rs:94`: Rev. J dimensions are 4.2/27.36/5.12 m, with derived relative elevations and annulus/free volume; volume=251.684523 m3, NWL=20.93 m; level-volume and rated circulation tests pass. Assumed internals remain explicitly identified. |
| Sol | 17 | PARTIAL | `crates/bwr-core/src/params.rs:193`: 3600 rpm, two poles, and J=18575.55 kg m2 pass core tests, but `crates/bwr-core/examples/startup.rs:48` still requests 1800 rpm and cannot synchronize; executed regression N4. |
| Sol | 18 | FIXED | `crates/bwr-core/src/sim.rs:515`: physical damage claim is replaced with a fuel-model-envelope flag/alarm and API field; an isolated 1201 C fuel-node step latches that flag and emits the unsupported-envelope indication. |
| Sol | 19 | FIXED | `crates/bwr-core/src/kinetics.rs:100`: logarithmic period estimation requires positive samples above a floor; executed source-start and zero-source tests retain finite meter state. |
| Sol | 20 | FIXED | `crates/bwr-core/src/sim.rs:565`, `:207`, and `:352`: all command floating scalars and nonfinite/negative time inputs are checked; original NaN/Infinity/zero-step regression passes with unchanged physical snapshot and subsequent normal clock progress. |
| Sol | 21 | PARTIAL | `src-tauri/src/lib.rs:42` and `crates/bwr-core/src/sim.rs:210`: ordinary backlog is preserved; speed=100, advance(3) gives time=200 plus backlog=100, and advance(0) recovers the remainder within DT. Failed steps still debit DT despite rollback, as reproduced in N3; backlog/effective speed is not in the snapshot. |
| Sol | 22 | PARTIAL | `crates/bwr-core/examples/validation.rs:36` and `:60`: runner exists, produces CSV, exits on failures, and all five cases pass. It checks only vessel-port closure and finiteness, with no timestep gate, complete plant balance, independent physical reference acceptance, or one-/two-/three-train cooling endpoints; N1/N2/N6 escape its cases. Add those acceptance cases and separate them from regression traces. |
| Sol | 23 | PARTIAL | `crates/bwr-core/src/vessel.rs:661`: two-phase fixtures now enforce volume and reject the old impossible 200000 kg/287 C state. Compressed-liquid test at `:700` still generates its expected energy internally and accepts a broad pressure interval; independent IF97 at its reported T/P reconstructs energy 2.242412 GJ low and volume 0.513720 m3 low. Add independent joint M/V/U fixtures. |
| Sol | 24 | PARTIAL | `crates/bwr-core/tests/plant.rs:367`: mirror parsing and nested checks are real, but reverse checking deserializes the Rust sample again. Every current fixture still passes after each mutation: selectRod index loses nullability; setSpeed gains unsupported nullability; ModeSwitch loses refuel/adds bogus; kineticsFault changes string to number. Compare complete schemas or exhaustively cover enums and nullable branches. |
| Sol | 25 | FIXED | `ui/src/sim/mock.ts:694`: independent turbine protection precedes the reactor-scram return; executed FullPower -> manualScram -> condenser=40 kPa -> tick(0.1) trips turbine and opens breaker. |
| Sol | 26 | FIXED | `ui/src/sim/mock.ts:260`: complete batch validation precedes mutation; executed HotStandby -> Refuel -> indices=[1,0], target=24 throws with byte-equivalent targets before/after. |
| Sol | 27 | FIXED | `ui/src/sim/mock.ts:373`: paused state is retained; compiled browser probes preserve pause across all three scenario loads. |
| Sol | 28 | FIXED | `crates/bwr-core/src/reactor.rs:144`: progress derives from scram demand and rods not all in; persistent-demand unit test and shutdown integration test report no insertion in progress after completion. |
| Astra | 1 | FIXED | `crates/bwr-core/src/sim.rs:607`: isolated batch, consecutive-command, already-out, and insertion-only REFUEL checks pass; pending targets participate in the atomic proposed-state check. |
| Astra | 2 | FIXED | `crates/bwr-core/src/sim.rs:483`: independent L8 feed trip and start inhibit pass the two-scenario integration test, including initially tripped turbine. |
| Astra | 3 | FIXED | `crates/bwr-core/src/vessel.rs:499`: geometric coverage test returns 0.6 for 60% height at several void fractions; no second swell factor. |
| Astra | 4 | PARTIAL | `crates/bwr-core/src/vessel.rs:263`: compression pressure is present, but saturated energy remains at `:249`; same M/V/E gives model 200 C/15.840386 MPa versus independent IF97 201.858653 C/16.273779 MPa. Solve compressed-liquid volume and energy consistently or reject that region. |
| Astra | 5 | PARTIAL | `crates/bwr-core/src/bop.rs:747`: finite heater store and zero-steam extraction bound pass; the remaining integrated heater/vessel enthalpy mismatch is quantified in N6. |
| Astra | 6 | PARTIAL | `crates/bwr-core/src/params.rs:32`: hydraulic scram rate is no longer used for SRI, but a travel-speed test substitutes for the requested coupled SCCRI rate acceptance; measured first-1/10-s reductions average 1.927/1.206 percentage points/s, versus the approximate published 0.8; see Sol 15. |
| Astra | 7 | FIXED | `crates/bwr-core/src/sim.rs:245`, `:282`, and `:323`: distinct period block/trip and low-flux validity are implemented outside RUN with provisional setpoints; executed test verifies invalid source-range signal, rejected withdrawal, stopping existing motion, and startup scram. |
| Astra | 8 | FIXED | `crates/bwr-core/src/kinetics.rs:69`: independent refined positive-reactivity checks cross both sides of the old singularity with approximately 1% error, while invalid raw steps return errors and strongly supercritical core advance grows or flags a fault. |
| Astra | 9 | PARTIAL | `crates/bwr-core/src/vessel.rs:246`: original energy gap is fixed; at M=100000 kg the 99.999999-to-100 C increment is 702.284485 J and midpoint residual is zero. The same finding's cold gas-boundary requirement remains absent: sealed and vented 90 C probes both report atmospheric pressure and zero vent exchange. |
| Astra | 10 | FIXED | `crates/bwr-core/src/vessel.rs:274`: explicit bracket/domain/residual errors and local rollback pass boundary tests; distinct plant-level transaction regression is N3. |
| Astra | 11 | FIXED | `crates/bwr-core/src/ic.rs:114`: boiling-boundary budget and finite depletion capacity pass three tests plus the independent approximately 0.000100 J boundary residual check. |
| Astra | 12 | FIXED | `crates/bwr-core/src/vessel.rs:548`: original 1000 kg floor is removed; 1 s depletion probe accepts 987.091477 kg from 1000 kg with zero mass residual and explicit shortfall. New connected IC failure is N1. |
| Astra | 13 | FIXED | `crates/bwr-core/src/sim.rs:382` and `:443`: source enthalpy is carried through the line and into condenser accounting; executed isenthalpic unit test passes. |
| Astra | 14 | FIXED | `crates/bwr-core/src/reactor.rs:376`: moving-rod benchmark at dt=0.05/0.001/0.0001 gives n=0.066944260/0.066944260/0.066943556, with committed rod worth matching positions. |
| Astra | 15 | FIXED | `crates/bwr-core/src/sim.rs:515`: lumped temperature now marks the model envelope rather than physical core damage; isolated 1201 C probe verifies the renamed API flag and alarm. |
| Astra | 16 | FIXED | `crates/bwr-core/src/params.rs:244` and `crates/bwr-core/src/bop.rs:738`: two variable-speed 100% pumps, matching arrays and individual flows are implemented; single-pump rated-flow test passes. Newly introduced coastdown omission is N5. |
| Astra | 17 | PARTIAL | `crates/bwr-core/src/params.rs:193`: core speed, pole relation, and inertia pass tests, but shipped startup retains 1800 rpm and never synchronizes; N4. |
| Astra | 18 | PARTIAL | `crates/bwr-core/src/sim.rs:607` and `crates/bwr-core/src/reactor.rs:205` accept native fine targets/instant STOP, but `ui/src/sim/mock.ts:260` rounds target=12.25 to 12 and `:269` STOP at 12.6 to 13 in executed probes; supporting `ui/src/components/RodControls.tsx:76` also rounds MOVE TO to two-position notches. Remove frontend/mock rounding and share a defined fine-jog increment. |
| Astra | 19 | PARTIAL | `crates/bwr-core/src/ic.rs:131`: finite primary storage and standby return exist; isolated 120 s activation closes modeled mass to 2.55e-11 kg and energy to 2.03e-6 J. Coupled accepted transfers fail in N1, and `crates/bwr-core/src/vessel.rs:609` still has no chimney-local return/quench treatment. |
| Astra | 20 | FIXED | `crates/bwr-core/src/bop.rs:450`: makeup, reject, overflow and accepted withdrawal are explicit, and overflow mass-closure test passes. Separate failure to share the accepted withdrawal with vessel integration is N2. |
| Astra | 21 | PARTIAL | `src-tauri/src/lib.rs:42` preserves ordinary delayed ticks, and the 4000-step core probe preserves 300 s as time plus backlog. `crates/bwr-core/src/sim.rs:214` still discards DT after a rejected step; N3. Backlog/effective speed remains absent from the UI snapshot. |
| Astra | 22 | FIXED | `crates/bwr-core/src/sim.rs:565`: original nonfinite commands cannot poison state; executed numeric-input regression also checks time inputs and subsequent finite/nullability-correct snapshots. |
| Astra | 23 | FIXED | `ui/src/sim/mock.ts:260`: mixed blocked/insertion batch rejection leaves all targets unchanged in the executed browser probe. |
| Astra | 24 | FIXED | `ui/src/sim/mock.ts:694`: executed vacuum-loss-after-manual-scram probe still trips turbine and opens generator breaker. |
| Astra | 25 | FIXED | `crates/bwr-core/src/kinetics.rs:100`: source startup from zero and zero population with zero source keep finite inverse period; regression passes. |
| Astra | 26 | PARTIAL | `crates/bwr-core/tests/plant.rs:383`: the claimed reverse check is a Rust self-roundtrip; four independent payload/enum/nullability mutations listed under Sol 24 still pass the complete current fixture set. |
| Astra | 27 | FIXED | `ui/src/sim/mock.ts:373`: executed pause/load sequences retain pause for cold shutdown, hot standby, and full power. |
| Astra | 28 | PARTIAL | `crates/bwr-core/examples/validation.rs:36`: deterministic runner/CSV/nonzero-failure behavior now exists and five cases pass, but no full-plant conservation, timestep-refinement acceptance, or independent physical reference checks; `hot_standby_startup` never asserts attaining criticality or synchronizing. Add explicit scenario outcomes and the missing independent acceptance gates. |
| Astra | 29 | FIXED | `crates/bwr-core/examples/trace.rs`: executed `cargo test --release -p bwr-core --example trace` passes its parser test; `trace full 1 scram@typo` exits 2 with a diagnostic rather than dropping the event. |

Physical reference checks: the compressed-state comparison independently solves mass/volume and internal-energy equations using CoolProp 8.0 IF97, following the [IAPWS IF97 formulation](https://iapws.org/technical-guidance/release/IF97-Rev). Geometry, speed, bypass timing and approximate SCCRI response were checked against [GE 005N9751 Rev. J, June 2026, sections 3.1.1, 3.2, 5.1.2 and 5.1.4](https://www.gevernova.com/content/dam/gevernova-nuclear/global/en_us/documents/carbon-free-power/005N9751-BWRX-300-General-Description.pdf). These checks do not validate the simulator's assumed coefficients or establish a plant transient benchmark.

## New findings

## N1 - ICs consume steam that the vessel refuses to supply

- **severity:** high
- **area:** physics
- **file:line:** crates/bwr-core/src/sim.rs:385
- **wrong:** `steam_avail` excludes only subcooled states. ICs commit primary inventory and pool heat before vessel acceptance; `vessel.rs:546` subsequently refuses IC steam in a water-solid state, and phase-inventory scaling can reduce it in other states. The simulator reconciles main-steam acceptance but never reconciles `flows.w_ic_steam`. Reproduction: start hot standby, use M=`V_VESSEL_FLUID * sat_t(200).rho_f * 1.005`, E=`energy_for(M,200)`, and its successful EOS solve; close MSIV commands/positions, stop feed pumps, and set all three ICs fully open with preceding heat `IC_TRAIN_RATED_W`. A 0.05 s step succeeds with no model error. ICs record 52.180346591 kg/s input while vessel acceptance is zero. Combined vessel-plus-IC primary inventory has a +2.609017330 kg residual and modeled stored energy has a +7.284538143 MJ residual after actual external ports, core heat, ambient loss and pool duty are included.
- **consequence:** Newly added primary ports manufacture water and energy during the very phase-limited states that the vessel safeguards attempt to handle; the pools also receive 101.212595 MW without the corresponding vessel steam withdrawal.
- **fix:** Compute requests, negotiate accepted steam/return transfers, then commit identical transfers to vessel, IC primary storage and pool energy. Exclude solid states from steam availability, and handle ordinary limited-steam acceptance as well. Add combined vessel/IC mass and energy assertions for both limiting cases.

## N2 - Condenser withdrawal limits are ignored by the receiving vessel

- **severity:** medium
- **area:** correctness
- **file:line:** crates/bwr-core/src/sim.rs:416
- **wrong:** Vessel feed is integrated using the requested `fw.flow` before `cond.step_energy` limits the actual withdrawal; `cond.w_out_accepted` is never used to correct the receiving transfer. Isolated public-API case: cold shutdown, hotwell=6001 kg at 35 C, both condensate/feed pumps on, feed-pump speeds=1, manual FCV=1, then `Sim::step(10.0)`. The step succeeds with no model error. Vessel feed is 1600 kg/s, accepted hotwell withdrawal is 630.1 kg/s including 30 kg/s makeup, and the combined inventory acquires an unexplained 9699 kg. The ordinary 0.05 s `advance` path's suction cutoff avoids this particular large-step reproduction; the public `step` API accepts it without subdivision or rejection.
- **consequence:** Recording a withdrawal deficit does not prevent manufactured reactor inventory, and the accepted public timestep changes whether connected inventories conserve mass.
- **fix:** Negotiate available feed before integrating heater/vessel balances and use the identical accepted mass and enthalpy transfer at both ends. Internally subdivide supported large steps or explicitly reject them, while retaining shared-port limits. Test combined inventory through hotwell exhaustion.

## N3 - Rejected plant steps mutate turbine controls and consume clock backlog

- **severity:** medium
- **area:** correctness
- **file:line:** crates/bwr-core/src/sim.rs:357
- **wrong:** The rollback tuple omits the turbine, although `tcv_demand` mutates its speed reference/governor before the vessel can reject the step. `advance` also unconditionally subtracts DT at `sim.rs:214` after a failed step. Reproduction matching the existing domain-error test: hot standby, ResetTurbine, speed target=3600, multiply vessel energy by three, then call `step(0.05)` three times. Time remains zero and every step reports a model error, but turbine speed reference advances 0 -> 0.5 -> 1.0 -> 1.5 rpm. A separate failed `advance(0.1)` leaves time=0 and backlog=0.05 instead of retaining 0.1 s.
- **consequence:** The promised transactional rejection is false; retries evolve controls without elapsed physics time and lose scheduled duration. The current rollback test checks only a few fields in the grid-connected state, where this governor mutation is inactive.
- **fix:** Make the plant step report acceptance, restore every mutated subsystem/control state on failure, and debit the accumulator only after acceptance. Preserve the diagnostic/pause as the explicit failure result. Test rejected steps with an accelerating disconnected turbine and compare complete physical state plus clock accounting before/after.

## N4 - The turbine speed migration breaks the shipped startup procedure

- **severity:** medium
- **area:** correctness
- **file:line:** crates/bwr-core/src/params.rs:193
- **wrong:** The machine now synchronizes at 3600 rpm, but `crates/bwr-core/examples/startup.rs:48` commands 1800 rpm and `:51` attempts synchronization near that speed. Executed `cargo run --release -p bwr-core --example startup -- 0.4` exits zero after `END t=14401 n=0.000 MWe=-12.5 scram=Some("APRM high flux (setdown)")`; it remains in stage 1 at 1800 rpm and never closes the breaker. The pre-fix review recorded this example attaining n=0.393 at 4494 s.
- **consequence:** The shipped startup demonstration can no longer execute its advertised procedure, and its successful exit masks the regression. The required validation runner's nominal startup case never asserts synchronization and does not catch it.
- **fix:** Use `params::RATED_RPM` for both rolling demand and synchronization comparison; make the example exit nonzero unless synchronization and the requested power target are reached. Exercise the complete scripted startup in an acceptance check and adjust the operator sequence if further failures emerge.

## N5 - Stopped feed pumps lose flow immediately despite near-rated shaft speed

- **severity:** medium
- **area:** physics
- **file:line:** crates/bwr-core/src/bop.rs:635
- **wrong:** Hydraulics filters pumps by the running boolean and sets each stopped pump's flow to zero at `:680`, while `:738` models a finite shaft coastdown. In an isolated normal-pressure case, two pumps at speed 0.94 deliver 311.330902 kg/s each. Stop pump 2 and step 0.05 s: its speed is still 0.93, but its flow becomes zero and total flow jumps from 622.661804 to 379.184592 kg/s. No modeled discharge isolation justifies discarding its rotating head.
- **consequence:** Feed-pump-trip and vessel-level transients omit coastdown delivery even though the API reports the shaft still rotating. Tests verify only eventual stopped speed/flow and miss the initial discontinuity.
- **fix:** Evaluate hydraulic head/flow from actual shaft speed for both energized and coasting pumps, with explicit check-valve/no-reverse-flow behavior. Use the running flag for motor acceleration rather than immediate hydraulic disconnection. Test the first seconds after a pump trip.

## N6 - Heater storage and vessel feed use different enthalpies for the same transfer

- **severity:** medium
- **area:** physics
- **file:line:** crates/bwr-core/src/bop.rs:751
- **wrong:** The heater energy balance uses pre-step `h_out`, then updates `t_feed`; `sim.rs:417` subsequently supplies post-step `h_feed()` to the vessel. An independent balance of vessel energy, condenser stored energy and `FW_HEATER_HEAT_CAP * t_feed`, accounting for core heat, vessel ambient/vent/IC ports, shaft work, CW/relief, heater pump work/loss and CST transfers, gives -1,611,203.374 J over 120 s after both MSIVs close from full power at dt=0.05. At dt=0.025 the residual is -807,759.839 J. The accumulated `fw.flow * (h_feed_after - h_feed_before) * dt` predicts the residual within 0.01 J. This is substantially smaller than the old fictitious heating source, but remains an actual port-conservation error.
- **consequence:** Cooling loses energy and heating creates it in a timestep-dependent way. Vessel-only balance tests pass because they never include the heater's opposite-side debit.
- **fix:** Return a step transfer enthalpy from the heater calculation and use that identical value in both its storage balance and the vessel feed port, or solve the heater implicitly with a common end-state transfer enthalpy. Add a whole-loop isolation balance that includes heater storage and all boundary terms, with timestep refinement.

PRIOR: fixed=37 partial=20 notfixed=0 regressed=0
NEW FINDINGS: 6
