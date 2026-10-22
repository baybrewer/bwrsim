1. Reviewed baseline `1a1dc4f` to HEAD `102423942e684c786a18d4e740667de5a47a8822` in the requested engine, bridge and UI contract/mock scope; read only the two named prior reports.
2. `cargo test --release -p bwr-core`: PASS, 46 unit tests, 19 integration tests, zero failures, zero doctests.
3. `cargo build --release --workspace`: PASS.
4. `npm --prefix ui run build`: PASS, including TypeScript checking and Vite production bundling.
5. `cargo run --release -p bwr-core --example validation`: PASS, all five cases; additional trace parser example test PASS and malformed-event execution exits 2 as intended.
6. Sol prior findings: FIXED=19, PARTIAL=9, NOT FIXED=0, REGRESSED=0.
7. Astra prior findings: FIXED=18, PARTIAL=11, NOT FIXED=0, REGRESSED=0; totals count both reviews separately.
8. Verdict (a), works as intended: NO across the exposed numerical and inventory limits; accepted-flow coupling and rejected-step rollback still fail despite green scripted cases.
9. Verdict (b), physics appropriate for a BWRX-300 training simulator: NO for quantitative transient training; sealed-gas/compressed-water behavior, whole-plant conservation and design-response acceptance remain incomplete.
10. Eight new findings below; isolated Rust/Node, independent IF97 and matrix-exponential checks supplement the suite; no graphical desktop smoke test was performed.

## Prior findings

| review | # | status | evidence |
| --- | --- | --- | --- |
| Sol | 1 | FIXED | `crates/bwr-core/src/kinetics.rs:63`: adaptive substeps and checked solves; release singularity tests pass; independent matrix exponential gives n=3.277484 at rho=.05/1 ms versus model 3.315760 (1.17% error, within the declared 2% test bound). |
| Sol | 2 | FIXED | `crates/bwr-core/src/sim.rs:622`: validates proposed positions plus pending targets atomically; isolated ColdShutdown/Refuel batch [0,1] leaves both targets zero; consecutive requests leave targets [24,0]. |
| Sol | 3 | FIXED | `crates/bwr-core/src/sim.rs:490`: L8 trips feed pumps independently; release `l8_feed_pump_trip_is_independent_of_turbine_state` covers standby, power and restart rejection. |
| Sol | 4 | FIXED | `crates/bwr-core/src/vessel.rs:499`: coverage is geometric; release test checks 60% coverage at alpha=0/.4/.7 and both height limits. |
| Sol | 5 | PARTIAL | `crates/bwr-core/src/vessel.rs:255`: compression is added, but sealed cold headspace still has no gas state or vent-dependent EOS; isolated constant-mass 40/80/99 C states all return 101325 Pa. At 287 C/1% compression the model gives 21.408 MPa versus an independent fixed-mass/volume/energy IF97 solve of 13.408 MPa; use Region 1 and a gas inventory or explicitly reject these regimes. |
| Sol | 6 | PARTIAL | `crates/bwr-core/src/vessel.rs:279`: endpoint/domain rejection tests pass and Sim pauses, but rollback omits turbine governor state (`sim.rs:357`); the rejected-step probe changes speed_ref 0 to .5 rpm at unchanged time. See N3. |
| Sol | 7 | FIXED | `crates/bwr-core/src/vessel.rs:559`: bounds accepted withdrawals instead of flooring mass; release depletion test crosses 1000 kg, reaches below 20 kg and verifies reported shortfall on successive steps. |
| Sol | 8 | PARTIAL | `crates/bwr-core/src/bop.rs:750`: heater store now supplies isolation cooling and zero-steam extraction test passes, but outgoing enthalpy uses a different time level from the store debit; 60 s coupled scram loses 4.024 MJ. Extraction also uses unaccepted steam. See N2/N6. |
| Sol | 9 | PARTIAL | `crates/bwr-core/src/bop.rs:465`: sensible water/metal energy and CW removal now balance before relief (release tests pass), but there is still no vapor mass state and relief removes heat without water. See N5. |
| Sol | 10 | FIXED | `crates/bwr-core/src/reactor.rs:374`: rods and worth advance on the 1 ms kinetics timeline; release coarse/fine moving-rod test passes and checks published rod worth against committed positions. |
| Sol | 11 | FIXED | `crates/bwr-core/src/ic.rs:116`: heat budget partitions sensible heat and latent boiloff, bounds depletion; release onset, empty-pool and final-depletion tests pass. |
| Sol | 12 | FIXED | `crates/bwr-core/src/sim.rs:587`: reset explicitly refuses SHUTDOWN; release test verifies refusal after 10.05 s, then successful reset after selecting STARTUP. |
| Sol | 13 | FIXED | `crates/bwr-core/src/sim.rs:443`: condenser receives vessel accepted-flow enthalpy; `bop.rs:59` carries inlet enthalpy unchanged; release isenthalpic-line and turbine-trip vessel-balance tests pass. |
| Sol | 14 | PARTIAL | `crates/bwr-core/src/sim.rs:379`: trip stroke passes 300 ms tests, but OpenBreaker load rejection only selects fast_open for one scan and retains the pressure target based on slowly closing TCVs; isolated FullPower/OpenBreaker gives bypass=.164976 at .300 s. Latch fast-open through its stroke and supply the load-rejection target; add a breaker-only timing test. |
| Sol | 15 | PARTIAL | `crates/bwr-core/src/params.rs:32`: separate 60 s actuator and slower-than-scram test exist, but no coupled rate acceptance test; FullPower/TripTurbine loses 3.921 percentage points by 2 s and 12.056 by 10 s versus the cited approximately .8 point/s. Calibrate bank/trajectory with explicit uncertainty and verify the time history. |
| Sol | 16 | FIXED | `crates/bwr-core/src/params.rs:105`: Rev J ID=4.2 m, height=27.36 m, fuel-bottom=5.12 m; derived free volume is 251.684523 m3, elevations remain ordered, and release volume/level and rated-flow tests pass. |
| Sol | 17 | FIXED | `crates/bwr-core/src/params.rs:194`: 3600 rpm, two poles, 3960 rpm overspeed and inertia derived from H/S; release frequency/inertia tests and startup synchronization pass; Node mock FullPower starts at 3600 rpm. |
| Sol | 18 | FIXED | `crates/bwr-core/src/sim.rs:517`: damage declaration replaced by explicit unsupported fuel-model-envelope flag; isolated fuel_t=1201 C with dt=1e-5 sets the snapshot flag; no cladding-damage claim remains in the engine. |
| Sol | 19 | FIXED | `crates/bwr-core/src/kinetics.rs:100`: log-rate requires both populations above a positive floor; release zero-population/source and zero-source tests keep the meter finite. |
| Sol | 20 | PARTIAL | `crates/bwr-core/src/sim.rs:563`: Sim command/step/advance rejection tests pass, but public `Kinetics::advance` at `kinetics.rs:113` still accepts negative dt; isolated equilibrium(1).advance(.001,0,-.001) returns Ok and n=.9714266. Validate timestep/source domains at every public numerical entry point. |
| Sol | 21 | FIXED | `src-tauri/src/lib.rs:46`: worker preserves wall-time backlog; `sim.rs:210` preserves bounded-work remainder. Isolated speed=100/advance(3) gives 200 s plus 100 s backlog; advance(0) yields 299.95 s plus .05 s remainder, conserving 300 s. |
| Sol | 22 | PARTIAL | `crates/bwr-core/examples/validation.rs:60`: executable runner and all five cases pass, but balances only vessel state against its own input ports; no heater/condenser/whole-plant residual, independent physical acceptance or timestep/endurance suite. N1/N2/N4/N5/N6 survive this gate; add those independent inventories and limiting cases. |
| Sol | 23 | FIXED | `crates/bwr-core/src/vessel.rs:662`: two-phase roundtrip fixtures now fit volume, assert reconstructed occupied volume and regime; former overfilled 200 t/287 C fixture is separately rejected. Both release tests pass. |
| Sol | 24 | PARTIAL | `crates/bwr-core/tests/plant.rs:391`: actual TypeScript mirror is parsed and several mutation tests pass, but enum members are sampled rather than exhaustively compared; renaming refuel passes every production contract fixture. See N8. |
| Sol | 25 | FIXED | `ui/src/sim/mock.ts:694`: vacuum trip runs before the scrammed guard; isolated Node FullPower/manualScram/40 kPa/autoTrips sets turbine.tripped=true. |
| Sol | 26 | FIXED | `ui/src/sim/mock.ts:262`: prevalidates all accepted indices; isolated mixed Refuel batch [1,0], target=24 throws with byte-identical targets. |
| Sol | 27 | FIXED | `ui/src/sim/mock.ts:373`: preserves paused and speed when loading; Node pause/load checks remain paused for all three scenarios. |
| Sol | 28 | FIXED | `crates/bwr-core/src/reactor.rs:143`: progress is scram demand AND rods not all in; release persistent-demand rod test and shutdown-mode integration test show false after completed insertion. |
| Astra | 1 | FIXED | `crates/bwr-core/src/sim.rs:622`: validates proposed positions plus pending targets atomically; isolated ColdShutdown/Refuel batch [0,1] leaves both targets zero; consecutive requests leave targets [24,0]. |
| Astra | 2 | FIXED | `crates/bwr-core/src/sim.rs:490`: L8 trips feed pumps independently; release `l8_feed_pump_trip_is_independent_of_turbine_state` covers standby, power and restart rejection. |
| Astra | 3 | FIXED | `crates/bwr-core/src/vessel.rs:499`: coverage is geometric; release test checks 60% coverage at alpha=0/.4/.7 and both height limits. |
| Astra | 4 | PARTIAL | `crates/bwr-core/src/vessel.rs:263`: saturation-only pressure replaced by constant compressibility, but energy still uses saturated u_f and no P dependence. Independent IF97 for M=1.01*V*rho_f(287 C), E=energy_for(M,287 C) gives 288.847 C/13.408 MPa versus 287 C/21.408 MPa; replace with coupled Region 1 mass/volume/energy solve or reject compression. |
| Astra | 5 | PARTIAL | `crates/bwr-core/src/bop.rs:750`: heater store now supplies isolation cooling and zero-steam extraction test passes, but outgoing enthalpy uses a different time level from the store debit; 60 s coupled scram loses 4.024 MJ. Extraction also uses unaccepted steam. See N2/N6. |
| Astra | 6 | PARTIAL | `crates/bwr-core/src/params.rs:32`: separate 60 s actuator and slower-than-scram test exist, but no coupled rate acceptance test; FullPower/TripTurbine loses 3.921 percentage points by 2 s and 12.056 by 10 s versus the cited approximately .8 point/s. Calibrate bank/trajectory with explicit uncertainty and verify the time history. |
| Astra | 7 | FIXED | `crates/bwr-core/src/sim.rs:282`: distinct non-RUN period scram and withdrawal block added with explicit provisional thresholds; release test verifies invalid low-flux signal, blocked new/ongoing withdrawal, and short-period scram. |
| Astra | 8 | FIXED | `crates/bwr-core/src/kinetics.rs:63`: adaptive substeps and checked solves; release singularity tests pass; independent matrix exponential gives n=3.277484 at rho=.05/1 ms versus model 3.315760 (1.17% error, within the declared 2% test bound). |
| Astra | 9 | PARTIAL | `crates/bwr-core/src/vessel.rs:246`: onset energy is continuous and release cross-100 C reconstruction test passes; gas-space boundary remains wrong because pressure_at never sees vent state and assumes atmospheric cold pressure. Track sealed gas and vent mass/energy, or reject sealed-cold operation. |
| Astra | 10 | PARTIAL | `crates/bwr-core/src/vessel.rs:279`: endpoint/domain rejection tests pass and Sim pauses, but rollback omits turbine governor state (`sim.rs:357`); the rejected-step probe changes speed_ref 0 to .5 rpm at unchanged time. See N3. |
| Astra | 11 | FIXED | `crates/bwr-core/src/ic.rs:116`: heat budget partitions sensible heat and latent boiloff, bounds depletion; release onset, empty-pool and final-depletion tests pass. |
| Astra | 12 | FIXED | `crates/bwr-core/src/vessel.rs:559`: bounds accepted withdrawals instead of flooring mass; release depletion test crosses 1000 kg, reaches below 20 kg and verifies reported shortfall on successive steps. |
| Astra | 13 | FIXED | `crates/bwr-core/src/sim.rs:443`: condenser receives vessel accepted-flow enthalpy; `bop.rs:59` carries inlet enthalpy unchanged; release isenthalpic-line and turbine-trip vessel-balance tests pass. |
| Astra | 14 | FIXED | `crates/bwr-core/src/reactor.rs:374`: rods and worth advance on the 1 ms kinetics timeline; release coarse/fine moving-rod test passes and checks published rod worth against committed positions. |
| Astra | 15 | FIXED | `crates/bwr-core/src/sim.rs:517`: damage declaration replaced by explicit unsupported fuel-model-envelope flag; isolated fuel_t=1201 C with dt=1e-5 sets the snapshot flag; no cladding-damage claim remains in the engine. |
| Astra | 16 | FIXED | `crates/bwr-core/src/bop.rs:625`: two 100% speed-dependent pump curves and low-flow FCV replace three fixed pumps; release one-pump rated-capacity and integrated single-pump-operation tests pass; mock array contains two pumps. |
| Astra | 17 | FIXED | `crates/bwr-core/src/params.rs:194`: 3600 rpm, two poles, 3960 rpm overspeed and inertia derived from H/S; release frequency/inertia tests and startup synchronization pass; Node mock FullPower starts at 3600 rpm. |
| Astra | 18 | PARTIAL | `crates/bwr-core/src/sim.rs:613`: Rust accepts fine targets and STOP holds exact position (release tests pass), but `ui/src/sim/mock.ts:261` still rounds 12.1 to 12 and `:269` rounds STOP position. Remove browser quantization and test shared fine-target/STOP sequences. |
| Astra | 19 | PARTIAL | `crates/bwr-core/src/ic.rs:107`: finite standby inventory and primary-port tests exist, but train state is committed before vessel acceptance and never reconciled; isolated phase-limit case creates 2.980384 kg in .05 s (N1). Return enthalpy also never enters the chimney inlet/quench closure at `vessel.rs:610`; wire accepted return into that local mixing state. |
| Astra | 20 | PARTIAL | `crates/bwr-core/src/bop.rs:450`: explicit makeup/reject/overflow and bounded local withdrawal tests pass, but Sim sends requested feed to vessel before reading accepted hotwell withdrawal; the dt=20 s limiting check creates 13182.073 kg beyond explicit makeup (N4). |
| Astra | 21 | FIXED | `src-tauri/src/lib.rs:46`: worker preserves wall-time backlog; `sim.rs:210` preserves bounded-work remainder. Isolated speed=100/advance(3) gives 200 s plus 100 s backlog; advance(0) yields 299.95 s plus .05 s remainder, conserving 300 s. |
| Astra | 22 | PARTIAL | `crates/bwr-core/src/sim.rs:563`: Sim command/step/advance rejection tests pass, but public `Kinetics::advance` at `kinetics.rs:113` still accepts negative dt; isolated equilibrium(1).advance(.001,0,-.001) returns Ok and n=.9714266. Validate timestep/source domains at every public numerical entry point. |
| Astra | 23 | FIXED | `ui/src/sim/mock.ts:262`: prevalidates all accepted indices; isolated mixed Refuel batch [1,0], target=24 throws with byte-identical targets. |
| Astra | 24 | FIXED | `ui/src/sim/mock.ts:694`: vacuum trip runs before the scrammed guard; isolated Node FullPower/manualScram/40 kPa/autoTrips sets turbine.tripped=true. |
| Astra | 25 | FIXED | `crates/bwr-core/src/kinetics.rs:100`: log-rate requires both populations above a positive floor; release zero-population/source and zero-source tests keep the meter finite. |
| Astra | 26 | PARTIAL | `crates/bwr-core/tests/plant.rs:391`: actual TypeScript mirror is parsed and several mutation tests pass, but enum members are sampled rather than exhaustively compared; renaming refuel passes every production contract fixture. See N8. |
| Astra | 27 | FIXED | `ui/src/sim/mock.ts:373`: preserves paused and speed when loading; Node pause/load checks remain paused for all three scenarios. |
| Astra | 28 | PARTIAL | `crates/bwr-core/examples/validation.rs:60`: executable runner and all five cases pass, but balances only vessel state against its own input ports; no heater/condenser/whole-plant residual, independent physical acceptance or timestep/endurance suite. N1/N2/N4/N5/N6 survive this gate; add those independent inventories and limiting cases. |
| Astra | 29 | FIXED | `crates/bwr-core/examples/trace.rs:44`: typed argument parsing rejects malformed schedules before constructing Sim; release example parser test passes and trace full 1 scram@typo exits 2 with diagnostic. |


The PARTIAL entries identify the remaining work on prior findings; those original defects are not separately counted again below. FIXED means the original failure was corrected and exercised, rather than certifying the entire containing subsystem.

Independent thermodynamic comparisons use the mass, geometric volume and total energy together, including the 275 MJ/K vessel metal node, against [IAPWS-IF97 Regions 1 and 4](https://iapws.org/public/documents/UWTF-/IF97-Rev.pdf). The design-response checks use the approximately 0.8% power/s SCCRI response and 300 ms bypass response in [GE 005N9751 Rev J, sections 3.2 and 5.1.4](https://www.gevernova.com/content/dam/gevernova-nuclear/global/en_us/documents/carbon-free-power/005N9751-BWRX-300-General-Description.pdf). Geometry and rotating-machine selections were checked against sections 3.1.1 and 5.1.2 of the same document. These checks establish specific discrepancies, not a plant-validation dataset.

## New findings

## N1 - IC trains commit steam the vessel did not supply

- **severity:** high
- **area:** physics
- **file:line:** crates/bwr-core/src/sim.rs:389
- **wrong:** `step_full` immediately credits condensed steam to each primary inventory and heats the pool. The vessel later scales `w_ic_steam` to its available vapor (`vessel.rs:576`), but Sim never returns that accepted quantity to the trains. Reproduction: HotStandby; set vessel mass to `V_VESSEL_FLUID*sat_t(T).rho_f*(1-1e-8)`, energy to `energy_for(M,T)`, and recompute EOS; close MSIV positions/commands, disable feed, set each IC valve/cmd open, tube_mass=500 kg and heat=30 MW; step .05 s. No model error occurs: trains credit 59.609584 kg/s, vessel supplies .001901 kg/s, and combined vessel-plus-IC mass increases 2.980384 kg without any external mass input.
- **consequence:** The new IC inventory model manufactures primary water and credits cooling from nonexistent steam near flooding/depletion; component-only IC conservation tests do not detect the broken connection.
- **fix:** Separate IC transfer requests from commitment. Allocate vessel-accepted steam among trains, then recompute/commit train condensation, pool heat and primary energy using exactly those accepted flows and shared enthalpies. Test integrated vessel-plus-all-trains mass and energy under vapor exhaustion and water-solid conditions.

## N2 - Extraction heating is funded by requested steam after the vessel limits delivery

- **severity:** high
- **area:** physics
- **file:line:** crates/bwr-core/src/sim.rs:404
- **wrong:** Feedwater heaters receive unbounded `self.steam.w_tcv` before vessel acceptance. Later only turbine work and condenser inlet steam are scaled by acceptance, while the full extraction debit remains. Reproduction: initialize FullPower, replace vessel mass with `V_VESSEL_FLUID*sat_t(T).rho_f*(1-1e-8)`, regenerate its energy/EOS, and step .05 s. Requested turbine flow is 467.478511 kg/s; accepted main steam is .002001 kg/s, yet extraction is 361.548428 MW and condenser gross inlet energy becomes -361.544292 MW, with no model error.
- **consequence:** The condenser becomes an unmodeled reverse heat source for a hot feedwater heater, and published main-steam readings continue describing requests rather than delivered flow. The standalone extraction-bound test passes because its supplied turbine-flow argument is never constrained by a vessel.
- **fix:** Determine shared accepted steam transfers before heater energy integration; bound extraction against the accepted turbine mass/enthalpy budget after turbine work. Commit accepted TCV/bypass flows for controls and snapshots. Add a coupled vapor-limited extraction test that closes the steam, heater and condenser budgets.

## N3 - Rejected EOS steps still advance the turbine governor

- **severity:** medium
- **area:** correctness
- **file:line:** crates/bwr-core/src/sim.rs:357
- **wrong:** The rollback tuple excludes `self.turbine`, although `tcv_demand` mutates speed_ref and gov_integral before EOS acceptance. Reproduction: HotStandby; ResetTurbine; SetSpeedSetpoint(3600); multiply vessel.energy by 3; step .05 s. The simulator reports a model error at time=0, but speed_ref changes from 0 to .5 rpm and governor integral from 0 to .000003472222. The new rollback regression only compares selected other subsystems.
- **consequence:** Failed steps have persistent physical/controller side effects. Retrying a rejected step can ramp the governor without advancing simulation time, contradicting the new transactional-step contract.
- **fix:** Save/restore turbine state and every controller mutated before commit, or compute a complete candidate physical state and atomically commit it after EOS success. Extend the rejection test to compare all physical/controller fields, permitting only the documented halt/error diagnostics to differ.

## N4 - Accepted hotwell withdrawal does not bound vessel feed

- **severity:** medium
- **area:** physics
- **file:line:** crates/bwr-core/src/sim.rs:444
- **wrong:** Condenser now bounds `w_out_accepted`, but Sim has already credited the full `fw.flow` to the vessel and heater. Reproduction through the unrestricted public step API: HotStandby with both MSIV positions/commands closed; Feedwater::new(2,150,Manual,1); hotwell_mass=6000.1 kg with set_temperature(35 C); step(20). This finite positive timestep is accepted with no model error. Vessel feed is 989.108639 kg/s, accepted hotwell withdrawal is 330.005 kg/s and shortfall is 659.103639 kg/s. Combined vessel/hotwell/IC mass rises 13782.072784 kg, of which only 600 kg is explicit makeup: 13182.072784 kg is created.
- **consequence:** The local depletion fix hides downstream water creation when a supported API step exhausts the hotwell. The 6000 kg suction reserve avoids this particular failure during ordinary .05 s steps, but does not make the public arbitrary-positive-dt contract safe.
- **fix:** Allocate condensate withdrawal before integrating vessel/feedwater states, and use the same accepted flow and enthalpy in all three balances. Alternatively substep public large timesteps with a justified inventory guarantee. Add a coupled depletion test that checks total mass after crossing available hotwell inventory.

## N5 - Condenser relief exports energy without exporting steam mass

- **severity:** medium
- **area:** physics
- **file:line:** crates/bwr-core/src/bop.rs:473
- **wrong:** The new relief path subtracts any energy above a temperature cap and records it as vented, but neither hotwell nor vapor inventory loses water. Isolated check: Condenser::new(false), CW pumps off, node temperature set to sat_p(CONDENSER_RELIEF_P).t, then step_energy(1,100e6,0,0). Result: q_relief=100 MW for one second, mass change exactly 0 kg, pressure pinned to 121590 Pa. A steam rupture disc cannot supply an energy-only sink; the passing relief test checks only the energy counter.
- **consequence:** After loss of CW, relief can dispose of unlimited energy while retaining all condensate inventory and return capability. Recording an energy counter does not close the physical mass/phase balance.
- **fix:** Represent shell vapor inventory/energy and an actual pressure-driven relief steam flow. Debit its mass and matching enthalpy, including evaporation from hotwell liquid when applicable, and expose cumulative relief mass. Test mass, energy, pressure response and inventory exhaustion together; explicitly stop at an unsupported phase boundary if vapor storage remains outside scope.

## N6 - Heater storage and delivered feedwater use different enthalpies for the same step

- **severity:** medium
- **area:** physics
- **file:line:** crates/bwr-core/src/bop.rs:763
- **wrong:** Heater energy is debited with h(t_feed_before), then `t_feed` is advanced; vessel input and the test's delivered-energy calculation use h(t_feed_after). Isolated Feedwater::new(2,215,Manual,.7), step(.05,P_RATED,0,470,0,true,35) gives flow=622.661804 kg/s, T_after=214.931767 C and residual `C*dT-dt*(Q_extract+Q_pump-Q_loss-W*(h_feed_after-h_hw))=-9746.069908 J`. Over ten seconds residuals are -1.858092/-0.928956/-0.464456 MJ at dt=.05/.025/.0125 s. A complete vessel+heater+condenser+IC-primary energy check after a 60 s FullPower manual scram gives -4.024117 MJ after accounting for q_core, turbine work, pump work, CW, ambient losses and explicit boundary flows. Existing heater tests allow unaccounted losses through inequalities, and validation checks the vessel alone.
- **consequence:** The new heater store still creates or destroys net plant energy during transients. Refinement reduces the coupling error, but the claimed conservation tolerance does not hold at the production timestep.
- **fix:** Return a step-integrated feed enthalpy transfer and use it unchanged on both sides, or solve the heater temperature implicitly and debit storage with the same end-step enthalpy used by the vessel. Assert a signed full heater balance including losses, then a whole-plant residual through isolation/scram at multiple timesteps.

## N7 - Newly added mock status fields never follow the modeled state

- **severity:** medium
- **area:** contract
- **file:line:** ui/src/sim/mock.ts:171
- **wrong:** New non-null telemetry is initialized in blankSnapshot but never updated: poolEmpty stays false, fuelEnvelopeExceeded stays false, extractionHeatMw stays zero, and boiloffKgs/condensateReturnKgs remain zero even as existing mock state evolves. Node checks show poolLevelPct=0 with poolEmpty=false, fuelTempC=1201 with fuelEnvelopeExceeded=false, and a normal FullPower tick with heaters enabled still reporting extractionHeatMw=0. The actual IC and fuel update sites (`:546` and `:535`) have no corresponding assignments; the structural contract test accepts the stale constants.
- **consequence:** Clients see validly typed but contradictory availability/model-validity telemetry in browser mode. This concerns the meaning of the newly required fields, independently of approximate mock thermodynamics.
- **fix:** Derive each supported status from the same updated mock state, latch the fuel envelope flag consistently, and compute the new flow/heat telemetry from the approximate balances already used. Represent genuinely unsupported quantities explicitly in the contract instead of asserting false/zero measurements. Add browser state-transition checks for empty pools, fuel-envelope crossing and operating heaters.

## N8 - The replacement contract gate still misses enum drift

- **severity:** low
- **area:** tests
- **file:line:** crates/bwr-core/tests/plant.rs:391
- **wrong:** The new parser validates enum values only when a scenario or one sampled command emits them. SetModeSwitch is sampled only as Run; Refuel is never emitted by the contract fixtures. In an isolated copy of the test, replacing the TypeScript enum literal "refuel" with "refuellingTypo" still passes check_commands and every pre/post-scram scenario snapshot used by json_contract_matches_ui. Thus a currently meaningful backend enum value can disappear from the mirror without failing the replacement gate.
- **consequence:** The new contract test provides incomplete protection for command/snapshot enum compatibility despite its exhaustive-shape claim.
- **fix:** Enumerate and roundtrip every value of ModeSwitch, ScenarioName, LevelControlMode and Severity against the TypeScript sets in both directions, including rejecting extra TypeScript values. Add deliberate enum-add/remove/rename mutations alongside the existing field/command mutations.

## Verification evidence and limits

All four requested commands exited zero. The validation cases were full_power_steady, turbine_trip, manual_scram, msiv_closure_ic_cooling and hot_standby_startup. Their maximum reported vessel mass residual was 5.675e-10 kg and maximum relative vessel energy residual was 9.255e-13; neither statistic includes heater, condenser or IC primary energy storage. Their green status therefore coexists with N1/N2/N4/N5/N6.

Review-only probes and their output are retained outside the repository at `/tmp/bwr-rereview-sol/`: `src/main.rs`, `probe-output.txt`, `tests/contract.rs`, `mock.cjs`, and an isolated Python environment with iapws/scipy. Rust reproduction command: `cargo run --release --offline --manifest-path /tmp/bwr-rereview-sol/Cargo.toml`; enum-mutation reproduction: `cargo test --release --offline --manifest-path /tmp/bwr-rereview-sol/Cargo.toml rereview_omitted_enum_values_pass -- --nocapture`. Node checks used an esbuild bundle of the current mock without starting its timer. These are diagnostic fixtures, not changes to production code or the committed test suite.

The whole-plant check uses the vessel's delivered core heat as an external input and sums vessel energy, heater C*T, condenser energy and each IC primary M*h. It independently subtracts mechanical turbine work, CW rejection, IC pool duty, heater/vessel ambient losses, relief and outgoing boundary enthalpies, and adds explicit pump work/makeup. The independent constant-reactivity kinetics oracle used a seven-state matrix exponential, not the production backward-Euler solver. Compressed-liquid comparisons solved IF97 density and internal energy simultaneously with the metal energy term. No other file in docs/reviews was read, and unrelated untracked files were left untouched.

PRIOR: fixed=37 partial=20 notfixed=0 regressed=0
NEW FINDINGS: 8
