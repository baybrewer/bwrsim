1. Reviewed working-tree engine, tests, examples, Tauri bridge, and UI contract/mock at HEAD `1a1dc4fdd8455d84e45ab3e8a4a6de89c389d6f9`; pre-existing changes were preserved.
2. `cargo build --release --workspace`: PASS, including the Tauri Rust targets; a graphical desktop smoke test was not performed.
3. `cargo test --release -p bwr-core`: PASS, 11 unit tests, 8 integration tests, 0 doctests; these results do not establish physical validation.
4. `cargo run --release -p bwr-core --example validation`: FAIL, no such example; only `trace`, `startup`, and the pre-existing untracked `review_probe` exist.
5. `npm --prefix ui run typecheck` and `npm --prefix ui run build`: PASS; Rust and TypeScript field names currently agree on inspection, but the contract test does not check the mirror.
6. Ran the existing review probe, a 180-second trace, and additional isolated Rust/Node probes; reproduced numerical, conservation, protection, and mock command defects below.
7. Verdict (a): normal scripted cases run, but the engine does not work correctly across its exposed controls and limiting states; passing tests miss consequential defects.
8. Verdict (b): suitable only for qualitative gameplay at present; closed-system energy, phase-domain behavior, protection, and several design assumptions prevent reliance on training transient results.
9. Independently compared all 350 steam-table rows with fresh CoolProp IF97 calculations: maximum relative errors in pressure/density/enthalpy were below 4.9e-7; the main thermodynamic defects are in state handling and coupling.
10. A 10-second full-power scram at dt=0.05/0.025/0.0125 s gave pressures 6.994288/6.994282/6.994293 MPa and fuel temperatures 331.862/331.604/331.482 C; this limited convergence check does not validate other events or long-term cooling.

## 1 — Prompt-supercritical steps silently become reactor shutdown

- **severity:** high
- **area:** correctness
- **file:line:** crates/bwr-core/src/kinetics.rs:39
- **wrong:** The backward-Euler Schur denominator can become nonpositive; its negative solution is then replaced by `1e-14`. Reproduction: `Kinetics::equilibrium(1.0).step(0.05, 0.0, 0.001)` returns n=1e-14 and a negative inverse period. The physical prompt growth rate is approximately `(0.05-0.006)/4e-5 = 1100 /s`, so this is rapid growth, not extinction. The coupled core always uses substeps near 1 ms and never checks this condition.
- **consequence:** Large positive reactivity, including a sufficiently withdrawn cold clean core, can appear to extinguish neutron power and bypass the expected high-flux response.
- **fix:** Reject unsupported reactivity explicitly before mutating state, or use adaptive/error-controlled kinetics integration with positive solutions, such as a matrix exponential or sufficiently small implicit steps with checked denominators. Do not use a neutron floor to conceal an invalid solve. Add tests on both sides of the denominator singularity and against an independent constant-reactivity solution.

## 2 — REFUEL permits simultaneous withdrawal of multiple rods

- **severity:** high
- **area:** correctness
- **file:line:** crates/bwr-core/src/sim.rs:293
- **wrong:** The interlock counts actual positions only, blocks only when the count exceeds one, and is evaluated once before processing the command. Starting cold, switch to REFUEL and command `[0,1]` to 48; after 10 s both rods are at position 2. Two separate commands issued before stepping produce the same result. Their pending targets are invisible to the block.
- **consequence:** The advertised one-rod REFUEL restriction fails both within one command and across queued operator actions.
- **fix:** Validate the complete proposed target vector atomically. The union of rods actually out and rods commanded out must contain at most one rod, while allowing continued movement of that same rod. Include multi-index, consecutive-command, and already-one-out tests.

## 3 — L8 feed-pump trip depends on the turbine being untripped

- **severity:** high
- **area:** correctness
- **file:line:** crates/bwr-core/src/sim.rs:411
- **wrong:** Both the high-level turbine trip and the feed-pump trip are inside `if !self.turbine.tripped`. In hot standby, construct a consistent vessel at NWL+2 m and step 0.05 s: level remains +199.78 cm, the turbine is tripped, and feed pumps remain `[true,false,false]`. After an ordinary L8 trip, restarting a pump also bypasses the trip because the turbine is now tripped.
- **consequence:** High water level can continue receiving feedwater precisely during shutdown or turbine-trip conditions, defeating the implemented overflow protection.
- **fix:** Evaluate L8 feed-pump protection independently of turbine state on every protection scan. Reject pump starts while the active high-level signal persists, or immediately enforce the trip independently. Test initial standby, turbine already tripped, and attempted restart at L8.

## 4 — Core coverage applies void swell twice

- **severity:** high
- **area:** physics
- **file:line:** crates/bwr-core/src/vessel.rs:325
- **wrong:** `update_level()` already inverts `liquid_volume_below()` using the core/chimney voids, so `level_z` is a mixture-surface elevation. `core_coverage()` treats it as a collapsed height and divides by `(1-alpha)` again. At `level_z=Z_CORE_BOTTOM+CORE_HEIGHT/2` and alpha=0.5, it returns 1.0 although half the active height is above the mixture surface.
- **consequence:** Uncovery alarms and fuel heat-transfer degradation are delayed or suppressed, making loss-of-inventory behavior falsely benign.
- **fix:** For this definition of `level_z`, use `clamp((level_z-Z_CORE_BOTTOM)/CORE_HEIGHT,0,1)` for mixture coverage. If a collapsed level is needed for instrumentation, compute it as a separate quantity from liquid inventory and do not reinterpret the mixture elevation. Test partial-height coverage at multiple void fractions.

## 5 — Vessel pressure is wrong outside saturated two-phase conditions

- **severity:** high
- **area:** physics
- **file:line:** crates/bwr-core/src/vessel.rs:163
- **wrong:** Below 100 C the EOS always returns atmospheric pressure regardless of vent status, gas inventory, or filling. Above 100 C it always returns saturation pressure, even when the required liquid volume exceeds the vessel volume; `solid=true` merely labels that state. For example, mass=400,000 kg and an energy generated at 150 C returns 0.4761 MPa with zero quality in a 254.795 m3 vessel, despite incompatible liquid specific volume. Closing the cold head vent has no thermodynamic effect until 100 C.
- **consequence:** Sealed heatup, vent transitions, and water-solid overfill have incorrect pressure response. High-pressure protection can be absent during conditions where liquid compression should dominate.
- **fix:** Solve mass, volume, and internal energy in the applicable phase domain. Use pressure-dependent compressed-liquid properties for liquid states, saturated properties only when the quality and volume equations admit a two-phase state, and a tracked noncondensable gas mass/vent boundary for a cold gas space. If these states are outside scope, stop with an explicit unsupported-state result instead of continuing at an invented pressure. Basis: [IAPWS IF97 release, Regions 1 and 4](https://iapws.org/technical-guidance/release/IF97-Rev).

## 6 — EOS silently hides energy outside its temperature bracket

- **severity:** high
- **area:** correctness
- **file:line:** crates/bwr-core/src/vessel.rs:175
- **wrong:** The fixed 1..349.5 C bisection never verifies that the supplied energy is bracketed or that the returned state reproduces it. For mass=100,000 kg and `energy_for(mass,349.5)+1e12`, the returned temperature is still 349.5 C with an unreported 1e12 J reconstruction residual. The lower boundary similarly hides excessive energy removal. Quality clipping also lets incompatible states masquerade as valid solutions.
- **consequence:** A no-sink heatup or excessive cooldown keeps producing bounded-looking pressure and temperature while the stored energy ceases to correspond to the reported state.
- **fix:** Check the EOS domain and reconstruction residual before accepting a result. Return a typed domain failure for energy, mass, or volume outside the supported envelope, or extend the property model to the required liquid/superheated regions. Propagate the failure to the simulator and UI; do not silently keep stepping.

## 7 — Vessel inventory floor manufactures water

- **severity:** high
- **area:** correctness
- **file:line:** crates/bwr-core/src/vessel.rs:357
- **wrong:** Mass is floored at 1,000 kg after the requested withdrawals, while energy is debited for the full withdrawals. An isolated check with 1,000 kg, a 15 kg/s liquid outlet, and dt=1 s leaves 1,000 kg: the mass residual is +15 kg. Nothing records or reports this correction.
- **consequence:** Depletion can never empty the vessel, outlet mass sent to the condenser has no corresponding vessel loss, and the remaining specific energy becomes inconsistent.
- **fix:** Bound each actual port transfer by available inventory and phase, and use the same accepted mass and enthalpy transfer at both ends. If the model cannot handle the resulting state, report its domain boundary. Remove unaccounted inventory floors and assert integrated mass closure during depletion.

## 8 — Feedwater heating draws energy from an unavailable source

- **severity:** high
- **area:** physics
- **file:line:** crates/bwr-core/src/sim.rs:369
- **wrong:** Feedwater temperature has a 180 s lag, but there is no stored heater energy. Its full enthalpy rise is subtracted from condenser duty regardless of available extraction steam. When both MSIVs are shut immediately from full power, the next step still feeds about 466.62 kg/s at 215.56 C, adding approximately 363.46 MW above hotwell enthalpy with zero main-steam flow. Raw condenser duty becomes negative and `Condenser::step` clips it to zero.
- **consequence:** During isolation or turbine trip, hundreds of megawatts can be added to the reactor without any debit to a physical source. Pressure, cooldown, and inventory transients are materially distorted.
- **fix:** Introduce a finite heater thermal store and extraction-energy balance, or bound instantaneous feedwater heating by available extraction duty plus a debited store. Use `dU_heater/dt=Q_extraction+Q_pump-W_fw*(h_fw-h_in)-Q_loss`; debit extraction from turbine/exhaust consistently. Do not use a negative condenser-duty clamp to balance missing energy. Add a zero-steam, initially hot-heater conservation test.

## 9 — Condenser condenses steam without a heat sink or energy store

- **severity:** high
- **area:** physics
- **file:line:** crates/bwr-core/src/bop.rs:290
- **wrong:** `q` drives a prescribed pressure target, rather than an energy balance. Every kilogram in `w_in` immediately enters the liquid hotwell even with both CW pumps off; no vapor inventory or condenser energy is tracked. An isolated 1 s test with no CW, q=100 MW, and w_in=40 kg/s adds all 40 kg to the hotwell and reports 25.63 C, but has no term storing or exporting the supplied 100 MJ. The 120 s pressure lag and pressure cap do not supply that missing balance.
- **consequence:** Loss of cooling and overload still receive fictitious condensation and condensate return; vacuum-loss timing is imposed by tuning rather than the available heat capacity and phase inventories.
- **fix:** Track condenser fluid/metal energy and vapor/liquid inventory. Compute CW removal from `Q_cw=m_cw*cp*epsilon*(T_sat-T_cw,in)` and evolve `dU/dt=sum(W_in*h_in)-W_out*h_out-Q_cw` with consistent phase equilibrium. Bound condensation by that balance. Any vent/relief mass and energy must be explicit boundary flows.

## 10 — Rod motion and kinetics use different rod positions

- **severity:** high
- **area:** correctness
- **file:line:** crates/bwr-core/src/reactor.rs:302
- **wrong:** `Sim::step` calculates reactivity, then `Core::step` moves rods for the entire plant step and integrates kinetics using the old reactivity for all microsteps. With rods initially at position 24 and scram active, a 0.05 s step changes rod worth by about -666.06 pcm, while kinetics uses the pre-motion worth. Snapshot reactivity is also stale relative to the displayed positions.
- **consequence:** Scram and SRI power changes are delayed by a plant step, and the error depends strongly on timestep and differential rod worth. The 1 ms kinetics substeps do not correct this coupling error.
- **fix:** Move rods and update their worth on the same integration timeline as kinetics, using substep or midpoint positions rather than an entire step of stale worth. Refresh reported reactivity for the accepted state. Test a prescribed rod ramp against a refined reference, including motion through maximum differential worth.

## 11 — IC pool boiling and depletion discard energy

- **severity:** medium
- **area:** physics
- **file:line:** crates/bwr-core/src/ic.rs:47
- **wrong:** A step crossing 100 C applies all heat as sensible heating; the next step resets temperature to 100 without converting the overshoot to vaporization. At pool_t=99.9999 C, full mass, full valve, and rated train heat, two 0.05 s steps lose 1,059,600 J. At depletion, mass is clamped to zero while the vessel is still charged the whole `heat*dt`; a 0.001 kg pool with residual heat=1 MW loses another 47,618 J in one 0.05 s check.
- **consequence:** Pool energy and boiloff do not close at phase boundaries; an exhausted pool can temporarily remove heat it cannot absorb.
- **fix:** Spend each step's energy budget first on `m*cp*(100-T)`, then on `h_fg*dm`, carrying the unused fraction across the boundary within that same step. Limit accepted duty by remaining capacity, and account explicitly for any tube thermal store supporting lagged duty. Test boiling onset and final depletion with energy residual assertions.

## 12 — Scram can be reset while SHUTDOWN remains selected

- **severity:** medium
- **area:** correctness
- **file:line:** crates/bwr-core/src/sim.rs:476
- **wrong:** Entering SHUTDOWN generates a one-time scram, but SHUTDOWN is deliberately absent from `scram_signals()`. Reproduction: cold scenario, select STARTUP, select SHUTDOWN, settle 10.05 s, reset scram. The result is `scrammed=false, mode=Shutdown`. The browser mock instead treats SHUTDOWN as a persistent scram signal.
- **consequence:** Reset permissives and annunciation disagree with the mode that generated shutdown and differ between backends; selecting STARTUP afterward no longer requires clearing the shutdown scram latch.
- **fix:** Represent the SHUTDOWN switch as a persistent applicable scram/reset-inhibit input. Preserve any intentionally unlatched initial cold scenario explicitly, but block clearing an established mode-switch scram while its cause remains selected. Test reset before and after changing modes.

## 13 — Main-steam pressure loss changes enthalpy without heat or work

- **severity:** medium
- **area:** physics
- **file:line:** crates/bwr-core/src/sim.rs:366
- **wrong:** The vessel exports saturated-vapor enthalpy at dome pressure, but the condenser receives newly evaluated saturated-vapor enthalpy at chest pressure. A throttled line is approximately isenthalpic; the outlet need not remain saturated. At settled full power, the difference is +2,618.43 J/kg, creating about 1.224 MW in downstream accounting. At 2 MPa with full bypass, the same calculation instead loses about 19.17 kW.
- **consequence:** Whole-plant energy closure depends on pressure drop and load, adding an artificial heat source or sink to the condenser.
- **fix:** Carry the actual inlet enthalpy through steam lines and valve throttles, subtracting only explicitly modeled heat loss or work. If chest storage is modeled, evolve its mass and energy. Pass the resulting state/enthalpy to turbine and condenser rather than resaturating it at the downstream pressure.

## 14 — Bypass fast opening is slower than the published response

- **severity:** medium
- **area:** physics
- **file:line:** crates/bwr-core/src/bop.rs:100
- **wrong:** Automatic bypass motion is limited to 2 full strokes/s, requiring 0.5 s for a closed-to-open stroke. With chest pressure=7 MPa, dome pressure=P_RATED, full pressure demand, bypass available, and turbine tripped, six 0.05 s steps leave bypass position at 0.6. The vendor description specifies completion of bypass fast opening within 300 ms of the valid trip/load-rejection signal. Basis: [GE 005N9751 Rev. J, section 5.1.4, printed p.56](https://www.gevernova.com/content/dam/gevernova-nuclear/global/en_us/documents/carbon-free-power/005N9751_Rev_BWRX-300_General_Description.pdf).
- **consequence:** Load rejection and turbine-trip steam diversion are delayed, affecting pressure peaks and protection timing independently of the assumed bypass capacity.
- **fix:** Model the trip/load-rejection fast-open demand separately from ordinary pressure-regulating motion. Use a sourced stroke profile completing within 0.3 s; for a linear full stroke this requires at least 1/0.3 full strokes/s, adjusted for any modeled signal delay. Test valve position and steam diversion throughout the first second.

## 15 — Selected rod insertion uses the full hydraulic-scram timescale

- **severity:** medium
- **area:** physics
- **file:line:** crates/bwr-core/src/reactor.rs:120
- **wrong:** SRI uses `ROD_FULL_OUT/SCRAM_TIME` and drives four arbitrarily selected rods fully in. From the full-power scenario, a turbine trip reduces neutron power from 0.99346 to 0.47766 within 3 s. The current public design description gives SCCRI power reduction around 0.8 percentage point/s, rather than this 51.58-point drop. Basis: [GE 005N9751 Rev. J, June 2026, section 3.2, printed p.39](https://www.gevernova.com/content/dam/gevernova-nuclear/global/en_us/documents/carbon-free-power/005N9751_Rev_BWRX-300_General_Description.pdf).
- **consequence:** Turbine-trip ride-through is made much easier by an excessively fast and strong modeled power reduction. Its passing test cannot establish the documented plant response.
- **fix:** Separate SCCRI/SRI command, bank selection, and insertion rate from full hydraulic scram. Establish the applicable design configuration and calibrate the prescribed insertion trajectory to its documented response; keep unspecified bank worth as an assumption with sensitivity bounds. Add a power-reduction rate/time-history test rather than only checking power after 120 s.

## 16 — Vessel geometry does not match the public reference configuration

- **severity:** medium
- **area:** physics
- **file:line:** crates/bwr-core/src/params.rs:85
- **wrong:** The engine uses ID=4.0 m, height=26.7 m, and active-fuel bottom=2.5 m. The current vendor description gives approximately 4.2 m, 27.36 m, and 5.12 m respectively. These are consequential inputs, not just labels: they derive downcomer area, inventories, normal level, and head. Basis: [GE 005N9751 Rev. J, section 3.1.1, printed p.31](https://www.gevernova.com/content/dam/gevernova-nuclear/global/en_us/documents/carbon-free-power/005N9751_Rev_BWRX-300_General_Description.pdf).
- **consequence:** Stored fluid/metal response, elevation margins, level-to-inventory conversion, and circulation calibration correspond to a different geometry while some inputs are described as confirmed BWRX values.
- **fix:** Choose and record the applicable design revision. For Rev. J use the published dimensions above, then reconstruct a consistent free-volume/elevation model including internals and rederive level references and circulation losses. Do not simply change one elevation or borrow the appendix's aggregate inventory without reconciling its definition and physical volume.

## 17 — Turbine speed and electrical pole count use a different machine

- **severity:** medium
- **area:** physics
- **file:line:** crates/bwr-core/src/params.rs:156
- **wrong:** Rated speed is 1,800 rpm at 60 Hz, and `slip_hz()` divides rpm by 30, implementing a four-pole generator. The current vendor description specifies a 3,000/3,600 rpm turbine. Basis: [GE 005N9751 Rev. J, section 5.1.2, printed p.55](https://www.gevernova.com/content/dam/gevernova-nuclear/global/en_us/documents/carbon-free-power/005N9751_Rev_BWRX-300_General_Description.pdf).
- **consequence:** Turbine rolling, synchronization, overspeed setpoints, and shaft energy train against a different rotating machine.
- **fix:** For the described 60 Hz configuration use 3,600 rpm and two poles: `f=rpm/60`, with phase derived from generator frequency minus grid frequency. Update synchronization controls, examples, mock, and tests consistently. If retaining H=4 s on 330 MVA, derive inertia using `J=2*H*S/omega_rated^2`, approximately 18,576 kg m2, rather than retaining the 1,800 rpm inertia; derive the assumed 110% overspeed threshold as 3,960 rpm.

## 18 — Average fuel temperature is presented as a core-damage criterion

- **severity:** medium
- **area:** physics
- **file:line:** crates/bwr-core/src/sim.rs:437
- **wrong:** `fuel_t` is one lumped average fuel state. There is no separate cladding node, hot-channel/peak correction, or damage model, yet crossing 1,200 C permanently declares CORE DAMAGE. This state is neither peak cladding temperature nor fuel melting temperature. The requested fuel/cladding two-node model is not present; the vessel node does not substitute for cladding.
- **consequence:** The simulator can declare damage without a defensible material criterion, or miss local cladding damage while average fuel temperature remains lower. Uncovery outcomes look more physically established than they are.
- **fix:** Until a justified criterion exists, expose an explicitly modeled fuel-temperature limit/exceeded-envelope indication instead of physical core damage. If damage-related training is required, introduce at least `C_f*dT_f/dt=Q_f-(T_f-T_c)/R_fc` and `C_c*dT_c/dt=(T_f-T_c)/R_fc-Q_coolant`, plus justified peak factors and material/heat-transfer limits. Validate these before assigning damage outcomes.

## 19 — A valid zero-power kinetics state permanently corrupts the period meter

- **severity:** medium
- **area:** correctness
- **file:line:** crates/bwr-core/src/kinetics.rs:45
- **wrong:** `equilibrium(0.0)` is accepted, but the first source-driven step computes `ln(n1/0)`, giving infinity. The next smoothing update produces `inf-inf`, yielding NaN permanently. With rho=-0.07, source=SOURCE, dt=0.001, two steps give n=3.7100e-9 and `inv_period=NaN`.
- **consequence:** A legitimate zero-power/source startup loses its period signal. A snapshot can serialize a non-finite period as null, hiding the failure behind the same representation used for an effectively infinite period.
- **fix:** Define startup period behavior explicitly when the previous population is zero or below the source/noise floor. Initialize or temporarily mark the log-rate estimate unavailable until both samples support a finite logarithm; never inject infinity into the smoothing state. Assert finite period-estimator state through zero-power and source-only tests.

## 20 — Public numerical entry points accept values that poison state

- **severity:** medium
- **area:** correctness
- **file:line:** crates/bwr-core/src/sim.rs:462
- **wrong:** Floating-point clamps do not reject NaN. `Sim::cold_shutdown(); apply(SetSpeed{speed:NaN}); advance(1.0)` leaves speed=NaN and time=0; the poisoned accumulator prevents subsequent progress. `advance(NaN)` similarly poisons time accounting, and `step(0)` computes undefined period/heatup rates. This is a Rust library API defect; ordinary JSON IPC does not carry NaN literals.
- **consequence:** Embedders and numerical probes can silently freeze or corrupt the simulator with no rejection, and correcting the visible speed afterward does not repair the accumulator.
- **fix:** Validate finite command scalars before any mutation and validate timestep domains at numerical entry points. Define zero elapsed time as a no-op where appropriate; reject negative/non-finite physics steps. Return a structured error without changing state, and test that the full state remains unchanged after rejection.

## 21 — Runtime silently drops elapsed simulation time

- **severity:** medium
- **area:** correctness
- **file:line:** src-tauri/src/lib.rs:37
- **wrong:** Tauri caps each wall-clock interval at 0.5 s and discards the excess. Separately, `Sim::advance` zeroes its accumulator after 4,000 steps. At speed=100, `advance(3.0)` advances only 200 simulation seconds; another `advance(0.0)` does not recover the missing 100 s. The displayed speed still says 100.
- **consequence:** Delayed workers, lock contention, or an overloaded host change event timing relative to real time without reporting reduced effective speed. Accelerated runs cannot be compared reliably by wall time.
- **fix:** Preserve unprocessed elapsed time in a backlog while bounding work per iteration. If deliberate throttling is required, track and expose effective speed and backlog/dropped time explicitly. Do not silently overwrite the accumulator. Test delayed-worker and accelerated advance accounting.

## 22 — The requested validation runner and physical baselines do not exist

- **severity:** medium
- **area:** tests
- **file:line:** crates/bwr-core/examples/trace.rs:30
- **wrong:** Running the requested validation command fails because there is no `validation` target. `trace` prints selected values but does not assert case acceptance, inventory/energy residuals, or event timing. There are no executable validation baselines under `docs/validation/`; the refinement plans are not results. In particular, the 100% bypass assumption and calibrated natural-circulation/feedback values are not independently validated by the passing gameplay scenarios.
- **consequence:** There is no reproducible validation result for conservation, operating-envelope boundaries, IC endurance, or comparison with a selected BWRX reference. Script completion and calibrated steady-state tests can be mistaken for physical validation.
- **fix:** Add the deterministic `validation` example with named cases, machine-readable traces, nonzero failure exit status, complete mass/energy port accounting, event extrema/times, and documented independent references/tolerances. Keep baseline regression comparisons separate from physical acceptance. Include timestep convergence and declared one-/two-/three-train cooling endpoints within the supported envelope.

## 23 — EOS round-trip test passes a physically impossible vessel state

- **severity:** medium
- **area:** tests
- **file:line:** crates/bwr-core/src/vessel.rs:405
- **wrong:** The test generates energy with `energy_for` and inverts it with the same EOS, checking temperature only. At its existing mass=200,000 kg, T=287 C case, saturated liquid alone occupies approximately 271.166 m3, exceeding the model's 254.795 m3 volume. The EOS clips quality to zero and the test still passes. Thus it checks shared implementation consistency while failing to enforce the volume constraint it is supposed to support.
- **consequence:** A passing EOS test conceals incorrect compressed-liquid/water-solid handling and provides no independent evidence of thermodynamic correctness.
- **fix:** Retain the round trip as an algebra check, but assert reconstructed mass, energy, and occupied volume for physically admissible two-phase cases. Add independent IF97 compressed-liquid and two-phase fixtures, vented/sealed cases, and explicit rejection of unsupported states. Do not generate all expected values through the production EOS.

## 24 — JSON contract test does not inspect the TypeScript contract

- **severity:** medium
- **area:** tests
- **file:line:** crates/bwr-core/tests/plant.rs:144
- **wrong:** `json_contract_matches_ui` checks a subset of Rust snapshot keys and deserializes seven hand-written commands. It never reads, generates, compiles against, or otherwise checks `types.ts`; removing a TypeScript field or changing its nullability leaves this Rust test unchanged. Numerical validity, most nested fields, vector cardinalities, and most command variants are also unchecked. Current field-name agreement was established by manual inspection, not by this test.
- **consequence:** The named contract gate remains green through UI/engine schema drift, so it cannot protect the manually maintained mirror.
- **fix:** Generate TypeScript types from the Rust schema, or export exhaustive Rust command/snapshot fixtures and validate them against an independently executable UI schema. Cover every variant and nested field, enum values, nullability, finite numbers, and equipment cardinalities. Prove the check fails on a deliberate mirror mutation.

## 25 — Browser protection stops running after a reactor scram

- **severity:** medium
- **area:** correctness
- **file:line:** ui/src/sim/mock.ts:670
- **wrong:** `autoTrips()` returns immediately whenever `scrammed` is true, skipping independent turbine/vacuum protection as well as reactor scram decisions. In a Node probe with reactor scrammed, turbine reset, and condenser pressure=100 kPa, `autoTrips()` leaves `turbine.tripped=false`. The Rust engine continues scanning turbine protection after scram.
- **consequence:** A reactor scram prevents later equipment protection in the browser preview. This is a control-logic error even though the browser physics is deliberately approximate and labeled MOCK SIM.
- **fix:** Gate only the act of latching a new reactor scram. Continue evaluating independent equipment trips on every tick. Test a vacuum loss both before and after reactor scram and check the intended command/protection behavior against the Rust backend.

## 26 — Rejected mock rod commands can partially mutate targets

- **severity:** medium
- **area:** correctness
- **file:line:** ui/src/sim/mock.ts:243
- **wrong:** The command mutates targets while iterating, then throws if a later rod withdrawal is blocked. Reproduction: cold SHUTDOWN, rod 0 actual/target=48, rod 1 actual/target=0; command indices `[0,1]`, target=24. It throws a withdrawal-block error for rod 1 but leaves rod 0 target changed from 48 to 24. No successful command emission occurs for that partial change.
- **consequence:** An action surfaced to the user as rejected can still move an earlier rod on the next tick, leaving the displayed/rejected command semantics unreliable.
- **fix:** Validate indices, interlocks, and every proposed movement first, then commit all accepted targets together. If partial success is intentional, return explicit per-rod results and emit the accepted state rather than throwing an undifferentiated rejection. Test unchanged state after a mixed insertion/blocked-withdrawal rejection.

## 27 — Loading a browser scenario unexpectedly resumes a paused simulation

- **severity:** low
- **area:** contract
- **file:line:** ui/src/sim/mock.ts:355
- **wrong:** Mock scenario loading preserves speed but replaces `paused` with the blank snapshot's false value. A Node probe issuing pause followed by loadScenario produces `paused=false`. Rust's `LoadScenario` explicitly preserves both speed and pause state.
- **consequence:** Scenario reset behaves differently in browser and desktop mode, and a user can lose the paused inspection point during a scenario change.
- **fix:** Preserve both speed and paused state in `MockSim.loadScenario`, matching Rust. Add a shared command-sequence fixture asserting pause and speed across every scenario reset.

## 28 — Persistent scram inputs report rod motion after all rods are in

- **severity:** low
- **area:** contract
- **file:line:** crates/bwr-core/src/sim.rs:230
- **wrong:** Every active protection scan calls `scram()` and unconditionally sets `rods.scramming=true`. Although `Rods::step` clears it when all rods are in, protection sets it again before the snapshot. With an all-rods-in hot-standby vessel at NWL-2 m, one 0.05 s step reports `all_in=true`, `scrammed=true`, and `scram_in_progress=true` under the persistent L3 signal.
- **consequence:** The UI can show hydraulic rod insertion in progress indefinitely when no rod is moving, confusing latch status with actual mechanism activity.
- **fix:** Separate the scram demand/latch from actual insertion progress. Derive progress from a demand plus rods not yet fully inserted, or avoid setting the motion flag for an already-completed insertion. Test persistent low-level and high-pressure signals with all rods already in.

FINDINGS: 28
