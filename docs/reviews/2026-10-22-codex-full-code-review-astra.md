Scope: reviewed bwr-core src/tests/examples, src-tauri, and the UI contract/mock at HEAD 1a1dc4fdd8455d84e45ab3e8a4a6de89c389d6f9; prior docs/reviews reports were not read.  
Engine build: PASS - cargo build --release -p bwr-core --all-targets.  
Engine tests: PASS - cargo test --release -p bwr-core; 11 unit tests, 8 integration tests, 0 failed, 0 doc tests.  
Desktop build: PASS - cargo build -p bwrx300-sim; interactive desktop behavior was not smoke-tested.  
UI verification: PASS - npm --prefix ui run build, including tsc --noEmit and the Vite production build.  
Validation example: FAIL - cargo run --release -p bwr-core --example validation exits 101 because the target is absent.  
Existing examples: trace full 120 turbinetrip@5 and startup 0.4 completed; startup reached n=0.393 at 4494 s without a scram, which is execution evidence rather than physical validation.  
Independent checks: temporary Rust/TypeScript probes reproduced the findings; all 350 saturation-table rows matched independent IF97 calculations within 4.83e-7 relative; coupled trip steps were compared at 0.05/0.025/0.0125 s.  
Verdict (a): does not fully work as intended - passing tests miss interlock bypasses, lost elapsed time, nonfinite state, and mass/energy defects.  
Verdict (b): unsuitable as validated BWRX-300 training physics in its present form - normal-point tuning does not establish transient fidelity; fuel is one lumped node, and compression, IC activation, and equipment/protection behavior need correction.  

## 1 — REFUEL accepts multiple pending rod withdrawals

- **severity:** high
- **area:** correctness
- **file:line:** crates/bwr-core/src/sim.rs:293
- **wrong:** `withdraw_block()` counts actual positions, ignores targets, and waits until more than one rod is out. The command handler at lines 495-506 computes that result once for the whole batch. Reproduction: ColdShutdown -> Refuel -> RodSetTarget([0,1,2],48) accepts all three targets. After 4 s, all three positions are 1.6 and all three targets are 2. Two separate commands issued before movement also succeed.
- **consequence:** The one-rod refueling interlock is bypassed through normal commands; the later block only limits further movement after multiple rods have already withdrawn.
- **fix:** Before changing any target, validate the union of rods with nonzero actual positions and proposed nonzero targets. Permit withdrawal only when that resulting set contains at most one rod, including previously queued targets. Validate the entire batch atomically. Test batched, successive, already-moving, and insertion-only commands; changing >1 to >=1 alone is insufficient.

## 2 — L8 feed-pump trip depends on whether the turbine has already tripped

- **severity:** high
- **area:** correctness
- **file:line:** crates/bwr-core/src/sim.rs:411
- **wrong:** The high-water feed-pump trip is inside `if !self.turbine.tripped`. An isolated probe with a physically constructed vessel at +200 cm and all three feed pumps running stops all pumps when the turbine starts untripped. The identical case with the turbine already tripped leaves all three pumps running. HotStandby already starts with the turbine tripped.
- **consequence:** High-water protection fails during startup and after a turbine trip, allowing continued vessel filling precisely when steam admission is unavailable.
- **fix:** Evaluate the L8 feed-pump trip independently of turbine state. Retain the turbine-trip latch separately, and block pump restart while the applicable high-water signal remains active. Add tests for both initial turbine states and restart attempts under a persistent signal.

## 3 — Core coverage applies void expansion twice

- **severity:** high
- **area:** physics
- **file:line:** crates/bwr-core/src/vessel.rs:325
- **wrong:** `update_level()` already inverts a liquid-volume function containing core and chimney voids. `core_coverage()` then divides geometric coverage by 1-alpha again. At level_z=4.786 m, alpha_core=0.4, and TAF=6.31 m, 1.524 m of active fuel is above the modeled mixture level, yet coverage is 1.0 instead of 0.6. A normally constructed low-level Vessel at the same elevation gives 0.772126 instead of 0.6 without manually setting alpha.
- **consequence:** The core-uncovered alarm is suppressed and the fuel heat-transfer coefficient is overstated during inventory loss.
- **fix:** For the current single mixture-level geometry use coverage=clamp((level_z-Z_CORE_BOTTOM)/CORE_HEIGHT,0,1). Keep any future collapsed level distinct from mixture elevation. Test partial coverage at several void fractions with a geometric oracle, not the inverse of the same implementation. The distinction is described in [NRC Issue 101](https://www.nrc.gov/sr0933/section-3-new-generic-issues/issue-101-bwr-water-level-redundancy).

## 4 — Water-solid states return saturation pressure instead of compression pressure

- **severity:** high
- **area:** physics
- **file:line:** crates/bwr-core/src/vessel.rs:169
- **wrong:** Quality is clamped to zero when V/M is below saturated-liquid specific volume, and `solve_eos()` still returns saturation pressure. For M=222515.704694 kg, V=254.794564 m3, and E=energy_for(M,200 C), the engine reports 200 C and 1.554672 MPa. Its reported liquid occupies 257.342510 m3, exceeding vessel volume by 1%. An independent IF97 solve of the same mass, volume, and energy gives approximately 201.864911 C and 16.283787 MPa.
- **consequence:** Overfilling can hide a pressure excursion exceeding the modeled vessel design pressure; flow and protection calculations use the false low pressure even though the water-solid alarm appears.
- **fix:** Use the saturation lever rule only when v_f<=V/M<=v_g. For compressed liquid solve M*v(T,P)=V and M*u(T,P)+C_m*T=E simultaneously; solve the appropriate vapor region or explicitly reject it when quality would exceed one. Propagate an unsupported-state outcome if these regions are not implemented. Basis: [IAPWS-IF97 regions 1, 2, and 4](https://www.iapws.org/relguide/IF97-Rev.pdf).

## 5 — Feedwater heating creates energy after main-steam isolation

- **severity:** high
- **area:** physics
- **file:line:** crates/bwr-core/src/sim.rs:369
- **wrong:** Feed temperature remains elevated through the 180 s lag in bop.rs:505 even after extraction steam disappears. Its enthalpy gain is subtracted from condenser duty, but bop.rs:291 clips negative duty to zero and no heater/hotwell energy state supplies the deficit. Starting at FullPower, closing both MSIVs, and integrating 120 s at DT=0.05 produces 21.474057 GJ of clipped negative condenser duty, peaking at 277.963363 MW.
- **consequence:** Hot feedwater continues adding energy to the vessel without a corresponding depletion of steam or stored energy. Isolation pressure, cooldown, level, and reactivity transients therefore fail the first-law balance.
- **fix:** Represent heater/hotwell stored energy and extraction heat explicitly. Enforce dE_heater/dt=Q_extraction+Q_pump-W_fw*(h_out-h_in)-Q_loss and use the identical accepted transfers in the vessel and condenser balances. Bound heating by available extraction plus stored energy; do not repair the residual with max(0). Add an integrated isolation energy-balance regression.

## 6 — Selected rod insertion uses the hydraulic scram rate

- **severity:** high
- **area:** physics
- **file:line:** crates/bwr-core/src/reactor.rs:120
- **wrong:** The fast-insert branch uses ROD_FULL_OUT/SCRAM_TIME, identical to hydraulic scram. From FullPower, TripTurbine reduces neutron power from 99.35% to 70.78% at 0.5 s, 47.99% at 1 s, and 37.81% at 2 s; the four selected rods finish in about 2.3 s. The public BWRX description gives approximately 0.8 percentage point of power reduction per second for SCCRI. [GE 005N9751 Rev J, section 3.2, p39](https://www.gevernova.com/content/dam/gevernova-nuclear/global/en_us/documents/carbon-free-power/005N9751-BWRX-300-General-Description.pdf).
- **consequence:** The modeled reduction is tens of times faster than the stated response, making the passing turbine-trip bypass test substantially easier and distorting pressure and heat-removal demands.
- **fix:** Give selected rapid insertion its own actuator law, selected-bank targets, and termination conditions, separate from SCRAM_TIME. Validate the coupled power reduction against the published approximate rate with declared uncertainty; do not force neutron power directly. Re-evaluate tests/plant.rs:48 after correcting the insertion mechanism.

## 7 — Startup short-period protection is only an alarm

- **severity:** high
- **area:** physics
- **file:line:** crates/bwr-core/src/sim.rs:241
- **wrong:** Neither scram_signals() nor withdraw_block() examines reactor period; only active_alarms() does. A seeded HotStandby/Startup probe at n=1e-4, rho=0.001, IRM range 10, and inverse-period indication 1/s accepts a rod withdrawal. After a normal step its indicated period is 0.885856 s, the rod is moving, and scrammed=false. BWRX WRNM short-period withdrawal blocking and shorter-period startup scram are explicitly described in [NEDO-34169 Rev B, pp41,44 and Table 7-5 p72](https://www.gevernova.com/content/dam/gevernova-nuclear/global/en_us/documents/uk-smr/en/17.pdf).
- **consequence:** Operators can continue withdrawing rods during a fast startup excursion until a separate flux threshold is crossed; the simulator omits the documented preventive action.
- **fix:** Add distinct positive-period block and trip signals with the documented mode applicability, using a validated log-rate indication and defined low-flux validity. Put numerical setpoints in params.rs with source or explicit provisional-assumption status; the cited public material establishes the functions, not exact thresholds. Test each threshold, mode, invalid signal, and already-moving withdrawal.

## 8 — Positive kinetics growth is silently clamped into a shutdown

- **severity:** medium
- **area:** correctness
- **file:line:** crates/bwr-core/src/kinetics.rs:35
- **wrong:** At the production 1 ms substep the backward-Euler denominator becomes nonpositive near rho=0.046. The negative solution is replaced by 1e-14. From n=1 over 1 ms, rho=0.05 yields 1e-14, whereas 10000 equal substeps yield 3.27769. Even rho=0.045 gives 46.1095 instead of 2.90536. Core::new(1e-8,48,40) followed by update_reactivity(0,40) and step(0.05,40,1) has rho=0.127421 but falls to the floor. This is a public-engine/extreme-state reproduction, not a demonstrated ordinary operator sequence.
- **consequence:** A strongly supercritical state can appear safely shut down, and near the singularity the solver instead greatly exaggerates growth.
- **fix:** Use a matrix exponential or adaptive integration that resolves the positive kinetics eigenvalue and checks accuracy as well as positivity. Reject unsupported reactivity explicitly before mutation; never treat an invalid solve as a neutron floor. Add positive-reactivity convergence tests across the denominator boundary. The expected direction follows the [DOE point-kinetics equations](https://www.energy.gov/documents/doe-hdbk-1019-93vol2).

## 9 — The EOS has an unsolvable energy gap at 100 C

- **severity:** medium
- **area:** physics
- **file:line:** crates/bwr-core/src/vessel.rs:165
- **wrong:** The below-100 C branch assumes no vapor, while the next branch introduces equilibrium vapor throughout the headspace. For M=100000 kg, energy_for(M,100)-energy_for(M,99.999999) is 187.926947 MJ. Solving for the midpoint returns 100 C with a reconstructed energy residual of +93.963473 MJ. Bisection still reports a normal Eos state. Vent status is also absent from this closure, so sealed and vented cold headspaces are treated identically.
- **consequence:** Heatup through boiling onset contains an artificial energy plateau and wrong vapor inventory; cold closed-vessel pressure cannot respond to trapped gas.
- **fix:** Use a continuous mass/energy closure through boiling onset and define the gas-space boundary: prescribed atmospheric pressure with accounted vent flows, or a trapped-gas inventory for a closed vent. Check reconstructed energy and volume residuals before accepting the solve. Saturation is a phase condition, not a fixed 100 C switch; see [IAPWS-IF97](https://www.iapws.org/relguide/IF97-Rev.pdf).

## 10 — EOS endpoints silently conceal out-of-domain energy

- **severity:** medium
- **area:** correctness
- **file:line:** crates/bwr-core/src/vessel.rs:175
- **wrong:** The fixed 1..349.5 C bisection interval has no bracketing or residual check. With M=1000 kg and E=energy_for(M,349.5)+1 GJ, solve_eos returns 349.5 C, 16.427920 MPa, x=1, and no error, despite a -1 GJ reconstructed-energy residual. Arbitrary additional heating cannot raise its reported temperature or pressure. Excessive cooling is likewise pinned to the lower endpoint.
- **consequence:** The simulator continues presenting plausible-looking instruments and protection decisions after the thermodynamic model has left its property domain.
- **fix:** Check the admissible energy interval and solver residual before returning Eos. Return and propagate an explicit domain-exit error, preserving the last valid state or stopping the step transactionally; alternatively extend the property model. Add boundary tests just inside and outside both endpoints. Do not equate saturation-table clamping with a valid EOS solution.

## 11 — IC pool boundary handling loses heat and permits cooling with an empty pool

- **severity:** medium
- **area:** physics
- **file:line:** crates/bwr-core/src/ic.rs:43
- **wrong:** For an open train at rated steam temperature, pool mass 1.5e6 kg, pool temperature 99.9999 C, and initial heat 33.75 MW, two DT=0.05 steps remove 3.375000 MJ from the vessel. Sensible heating plus boiloff accounts for only 2.315400 MJ; 1.059600 MJ disappears when the overshot temperature is reset. An empty pool with a preceding 1 MW heat state still removes 997.5 kW next step and 19.95 MJ over its lag decay.
- **consequence:** Pool endurance and vessel cooldown do not conserve energy at boiling or depletion, and cooling remains available without a represented sink.
- **fix:** Partition Q into Q_sensible=min(Q,M*cp*max(T_b-T,0)) and boiloff dm=min(M,(Q-Q_sensible)/h_fg). Return only the accepted sensible-plus-latent heat to the vessel. Any dry tube/metal heat sink needs an explicit finite energy state. Test boiling crossings, depletion, and step-size convergence. Pool heating and boiloff are the physical sink in [NEDO-34168 Rev B, section 6.2.1.1](https://www.gevernova.com/content/dam/gevernova-nuclear/global/en_us/documents/uk-smr/en/16.pdf).

## 12 — The vessel inventory floor manufactures water

- **severity:** medium
- **area:** physics
- **file:line:** crates/bwr-core/src/vessel.rs:357
- **wrong:** Mass is clamped to at least 1000 kg after computing unrestricted outgoing flow, while energy loses the full outgoing enthalpy. At M=1000 kg and 40 C, a 10 kg/s liquid withdrawal for 1 s leaves M=1000 kg while energy falls 1.750037 MJ including heat loss. The mass residual is +10 kg; repeated withdrawals can continue indefinitely.
- **consequence:** Depletion is hidden, downstream components receive water not removed from the vessel, and the mass and energy states cease representing the same inventory.
- **fix:** Bound accepted outgoing transfers using available inventory before updating either balance, share those accepted flows with connected systems, and signal an unsupported depletion condition. Apply enthalpy removal to exactly the accepted mass. Add cumulative conservation tests that cross the minimum supported inventory rather than asserting only finite results.

## 13 — Steam enthalpy changes across an adiabatic pressure drop

- **severity:** medium
- **area:** physics
- **file:line:** crates/bwr-core/src/sim.rs:366
- **wrong:** The vessel exports h_g at dome pressure, but the condenser receives newly evaluated saturated-vapor enthalpy at chest pressure. At initialized FullPower, dome/chest pressures are 7.202375/7.004411 MPa, flow is 467.600769 kg/s, and the two enthalpies are 2769891.313/2772509.741 J/kg. This creates approximately 1.224379 MW between the vessel and balance of plant before considering other residuals.
- **consequence:** The cycle energy balance is biased even at steady power; larger throttling changes further corrupt condenser duty and efficiency.
- **fix:** Carry upstream specific enthalpy with each steam mass port. For an adiabatic line/valve with negligible kinetic-energy change and no shaft work, h_out=h_in. If line storage or heat loss is added, account for it explicitly. Determine downstream phase from (P,h), rather than assuming saturated vapor again. Test the shared mass-flow times enthalpy at both sides of every port.

## 14 — Rod motion and neutron integration use different positions within a step

- **severity:** medium
- **area:** correctness
- **file:line:** crates/bwr-core/src/reactor.rs:302
- **wrong:** Sim::step computes reactivity before Core::step moves the rods. Kinetics then integrates the entire plant step using the old rho_total, and the published reactivity still describes old positions. With all rods at mid-travel and scramming, one 0.05 s step changes actual rod reactivity from -0.100000 to -0.10666058, a 666.06 pcm change omitted from that step's kinetics.
- **consequence:** Scram and rapid insertion effects are delayed by the plant step, and snapshots can show rod positions inconsistent with reported reactivity. A 1 ms kinetics substep does not resolve this 50 ms coupling lag.
- **fix:** Integrate actuator position and rod worth on consistent substeps, or evaluate a documented midpoint/predictor-corrector reactivity for each plant step. Recompute published reactivity from the committed state. Add a moving-rod benchmark against a refined coupled solution. The existing turbine-trip check at dt=0.05/0.025/0.0125 gives peak pressure 7.223398/7.224474/7.225007 MPa, illustrating why event/extrema convergence should be measured separately from final-state agreement.

## 15 — Average fuel temperature is incorrectly reported as a core-damage criterion

- **severity:** medium
- **area:** physics
- **file:line:** crates/bwr-core/src/sim.rs:437
- **wrong:** The irreversible CORE DAMAGE latch compares the single lumped average fuel temperature with 1200 C. Core has no cladding-temperature node, axial peak, oxidation, or damage state. A temperature of the fuel material cannot be substituted for peak cladding temperature, and crossing an analysis limit is not itself a universal damage model. The familiar approximately 1204 C criterion concerns peak cladding temperature, as explained in [NRC Issue 170](https://www.nrc.gov/sr0933/section-3-new-generic-issues/issue-170-fuel-damage-criteria-high-burnup-fuel).
- **consequence:** The simulator can announce damage on the wrong physical quantity and imply accident-prediction capability that its thermal model does not provide.
- **fix:** Replace this latch with an explicitly named model-limit/unsupported-condition indication unless a damage model is implemented. For fuel/cladding response introduce separate energy states, e.g. C_f*dT_f/dt=Q_f-(T_f-T_c)/R_fc and C_c*dT_c/dt=(T_f-T_c)/R_fc-Q_cool. A two-node average model still needs justified peak reconstruction before applying a peak-cladding criterion; retain that limitation in the API/display.

## 16 — The feed-pump configuration disagrees with the published BWRX arrangement

- **severity:** medium
- **area:** physics
- **file:line:** crates/bwr-core/src/params.rs:182
- **wrong:** The simulator has three nominal 50% feed pumps with 400 kg/s runout per pump, starts two at full power, and uses fixed-speed curves plus an FCV. GE specifies two 100% reactor feed pumps with adjustable-speed drives. [GE 005N9751 Rev J, section 5.1.6, p57](https://www.gevernova.com/content/dam/gevernova-nuclear/global/en_us/documents/carbon-free-power/005N9751-BWRX-300-General-Description.pdf). The same count/capacity appears in [NEDO-34173 Rev B, section 10.2.2](https://www.gevernova.com/content/dam/gevernova-nuclear/global/en_us/documents/uk-smr/en/21.pdf).
- **consequence:** Pump availability, single-pump operation, failure response, level-control behavior, and house loads teach a different plant configuration.
- **fix:** Set N_FEED_PUMPS=2, model each pump for 100% required flow at operating head, and use speed-dependent pump curves and the documented low-flow control path. Reconcile initialization, UI/mock arrays, pump power, and single-pump tests together. Preserve explicit assumptions for unpublished curve coefficients rather than scaling the current curve by pump count alone.

## 17 — The 60 Hz turbine uses the wrong shaft speed and pole relationship

- **severity:** medium
- **area:** physics
- **file:line:** crates/bwr-core/src/params.rs:156
- **wrong:** RATED_RPM=1800, OVERSPEED_TRIP_RPM=1980, and bop.rs:185 uses rpm/30 for electrical frequency, representing a four-pole machine. GE describes the BWRX turbine as 3000/3600 rpm with a direct-driven 50/60 Hz generator. For the requested North American 60 Hz reference this is 3600 rpm. [GE 005N9751 Rev J, section 5.1.2, pp55-56](https://www.gevernova.com/content/dam/gevernova-nuclear/global/en_us/documents/carbon-free-power/005N9751-BWRX-300-General-Description.pdf).
- **consequence:** Turbine rolling, synchronizing, overspeed indications, and rotational energy use the wrong machine characteristics.
- **fix:** Use 3600 rpm and the two-pole relationship f=rpm/60 for the selected 60 Hz configuration. Update speed limits, synchronization text, mock, and startup tests. Recalculate inertia from I=2*H*S/(2*pi*N/60)^2; retaining H=4 s and S=330 MVA gives approximately 18576 kg m2. Do not retain the old inertia after doubling speed. Keep the assumed overspeed ratio explicitly documented.

## 18 — FMCRD commands discard fine positioning and continuous stopping

- **severity:** medium
- **area:** physics
- **file:line:** crates/bwr-core/src/sim.rs:494
- **wrong:** Every command is rounded to a two-position notch, and reactor.rs:156 continues motion to the next such notch when STOP is requested. With 3.81 m travel and 48 position units, this is 158.75 mm per notch. A strongest-rod middle-travel notch changes reactivity by about 71.85 pcm. The described BWRX FMCRDs support fine increments and continuous motion. [GE 005N9751 Rev J, section 3.2, p39](https://www.gevernova.com/content/dam/gevernova-nuclear/global/en_us/documents/carbon-free-power/005N9751-BWRX-300-General-Description.pdf).
- **consequence:** Near-critical approach and small power adjustments are forced into coarse reactivity changes, and STOP continues withdrawal after the requested stop.
- **fix:** Represent physical or normalized continuous travel and stop at the actual position, subject to an explicit actuator stopping model. If discrete jogs are retained, define a separately sourced or clearly assumed fine increment in params.rs rather than hard-coding 2/48. Update commands, UI/mock controls, and startup tests to use that shared travel convention.

## 19 — IC activation omits the standby condensate return transient

- **severity:** medium
- **area:** physics
- **file:line:** crates/bwr-core/src/sim.rs:334
- **wrong:** Each train returns only a heat rate; VesselInputs has no IC steam or condensate mass/enthalpy ports, and IcTrain has no primary-side inventory. Opening an IC therefore cannot inject its stored standby condensate or temporarily redistribute primary inventory. Public BWRX documentation explicitly describes stored subcooled condensate entering the chimney on initiation and credits return-pipe stored water in analysis. [NEDO-34168 Rev B, section 6.2.1.1, pp6-7](https://www.gevernova.com/content/dam/gevernova-nuclear/global/en_us/documents/uk-smr/en/16.pdf).
- **consequence:** IC initiation misses the inventory addition and local steam-quench response. A heat-only approximation can represent a steady closed loop, but it cannot reproduce this documented activation transient.
- **fix:** Add finite primary-side IC mass/energy storage and separate accepted steam-out/condensate-return ports. Enforce dM_IC/dt=W_steam-W_return and dE_IC/dt=W_steam*h_steam-W_return*h_return-Q_pool, with opposite vessel terms and a return location consistent with the chimney. Source the stored volume or declare a bounded estimate; test total vessel-plus-IC conservation and initiation inventory transfer.

## 20 — The hotwell clamp silently discards incoming water

- **severity:** medium
- **area:** physics
- **file:line:** crates/bwr-core/src/bop.rs:324
- **wrong:** After adding inflow and subtracting the modeled 30 kg/s reject, hotwell_mass is clipped to capacity without recording an overflow. Starting at 119999 kg, step(1,1e6,100,0) ends at 120000 kg, although the stated flows imply 120069 kg. The missing 69 kg has no destination. The lower clamp has the corresponding unsupported-outflow problem.
- **consequence:** Long-duration water inventory does not close, so apparent level stabilization can be caused by an unreported sink rather than the simulated controls.
- **fix:** Represent makeup, reject, and overflow as explicit boundary flows with associated enthalpies, and bound outgoing transfers by available water. Return the accepted condensate/feed transfer to both connected components. If an unlimited storage tank is intentionally external, report its cumulative mass/energy exchange instead of concealing it in a clamp.

## 21 — Delayed updates permanently discard elapsed simulation time

- **severity:** medium
- **area:** correctness
- **file:line:** src-tauri/src/lib.rs:37
- **wrong:** The worker clips elapsed wall time to 0.5 s and then advances last to now, discarding any excess. Independently, Sim::advance erases its accumulator after 4000 steps at sim.rs:207. At speed=100, advance(3.0) advances only 200 s; advance(0.0) cannot recover the missing 100 s. The current desktop cap prevents this particular core limit from being reached on its usual path, but the desktop cap itself loses time.
- **consequence:** Requested acceleration and simulated duration diverge silently under scheduling delays or larger public-API updates, compromising timed procedures and reproducible scenario execution.
- **fix:** Accumulate all elapsed time and preserve the remainder across bounded work batches. Expose effective speed/backlog when the worker cannot keep up, with an explicit pause/suspend policy if long system sleeps should be excluded. Test cumulative wall-time accounting across delayed ticks and the 4000-step boundary.

## 22 — Nonfinite native commands poison persistent state

- **severity:** medium
- **area:** correctness
- **file:line:** crates/bwr-core/src/sim.rs:462
- **wrong:** Floating-point clamp does not reject NaN. Through the public Rust API, SetSpeed(NaN), advance(1), SetSpeed(1), advance(1) leaves simulation time permanently at zero because the accumulator remains NaN. RodSetTarget(NaN) installs a NaN target that serde_json serializes as null, contrary to the TypeScript number[] contract. This is a native API defect; ordinary JSON input cannot encode NaN, so normal Tauri UI reachability is not established.
- **consequence:** Invalid input can permanently freeze a simulation or produce snapshots that violate their schema, with no rejection or recovery.
- **fix:** Validate finiteness and ranges before mutating any command-controlled state. Validate real_dt and physics dt, rejecting negative/nonfinite values and treating zero consistently. Return a typed error from apply/advance as appropriate and propagate it through the bridge. Assert byte-equivalent physical state after rejected inputs and finite/nullability-correct snapshots afterward.

## 23 — A rejected browser rod batch still changes earlier targets

- **severity:** medium
- **area:** correctness
- **file:line:** ui/src/sim/mock.ts:243
- **wrong:** The mock validates and mutates each index in one loop. In initial HotStandby, rod 1 target is 30 and rod 0 target is 12. Select Refuel and send indices=[1,0], target=24: rod 1 changes to 24 before rod 0 triggers ROD WITHDRAW BLOCK and the command throws. The rejected request therefore has a physical side effect.
- **consequence:** The UI reports failure while part of the requested rod motion has been accepted, making retries and operator recovery misleading even in demonstration mode.
- **fix:** Prevalidate the complete batch against one proposed resulting state before committing any targets. Alternatively define explicit per-index results, but do not throw a whole-command rejection after partial mutation. Add a mixed insertion/blocked-withdrawal test asserting unchanged targets after rejection.

## 24 — Browser turbine protection stops after a reactor scram

- **severity:** medium
- **area:** correctness
- **file:line:** ui/src/sim/mock.ts:670
- **wrong:** autoTrips() returns immediately when s.scrammed, before the independent condenser-vacuum turbine trip at lines 679-681. FullPower -> manualScram, followed by a condenser-pressure fault override of 40 kPa and tick(0.1), leaves the turbine untripped at 39.974 kPa. This reproduces a missing protection action, not merely a difference in approximate mock physics.
- **consequence:** The browser demonstration allows the turbine to remain running above its own vacuum trip threshold once the reactor has scrammed.
- **fix:** Evaluate independent turbine protections regardless of reactor scram state. Restrict the existing guard to repeated reactor-scram initiation. Test condenser-vacuum loss before and after manual/automatic reactor scrams.

## 25 — Zero initial neutron population permanently invalidates the period meter

- **severity:** low
- **area:** correctness
- **file:line:** crates/bwr-core/src/kinetics.rs:40
- **wrong:** Kinetics::equilibrium(0).advance(-0.07,SOURCE,0.05,50) yields finite n=4.2169e-9 but inv_period=NaN. The first logarithm divides by n0=0, and the filter subsequently evaluates infinity minus infinity. Built-in Sim scenarios avoid this by resetting source equilibrium, but zero is accepted by the public constructor and represents a meaningful source-start limiting case.
- **consequence:** A valid source-driven population can retain an unusable period indication indefinitely, undermining downstream period alarms/protection in custom scenarios.
- **fix:** Define the meter behavior below a positive detection/logarithm floor, initialize its state when entering the valid range, and avoid evaluating log(n1/n0) with nonpositive operands. Recover or explicitly invalidate a nonfinite filter state. Add zero-source and zero-population-with-source limiting tests.

## 26 — The contract test does not compare the Rust and TypeScript contracts

- **severity:** low
- **area:** tests
- **file:line:** crates/bwr-core/tests/plant.rs:144
- **wrong:** json_contract_matches_ui only serializes a Rust snapshot, checks selected keys, and deserializes seven hand-written Rust command examples. It never imports, generates, parses, or validates ui/src/sim/types.ts. Renaming an unchecked TypeScript field or altering its nullability cannot affect this test. The test passes while the native NaN-target reproduction emits null into a declared number[] field.
- **consequence:** A green test named as cross-language compatibility evidence provides no such guarantee and leaves most command variants and nested snapshot fields unchecked.
- **fix:** Generate TypeScript types from the Rust schema or add an exhaustive cross-language fixture/schema check for every command variant and all snapshot fields, enum values, units, and nullability. Include all scenarios and invalid-number rejection. Keep behavioral mock parity tests separate from structural schema checks.

## 27 — Loading a browser scenario unexpectedly resumes a paused simulation

- **severity:** low
- **area:** contract
- **file:line:** ui/src/sim/mock.ts:355
- **wrong:** loadScenario saves speed but replaces the snapshot with blankSnapshot without retaining paused. setPaused(true) followed by loadScenario(coldShutdown) results in paused=false. The Rust implementation explicitly retains both speed and paused at sim.rs:464-467.
- **consequence:** The same command sequence has different pause semantics in browser and desktop, and state begins evolving while the operator expects to inspect a paused reset.
- **fix:** Preserve both speed and paused when rebuilding the mock scenario, and verify pause preservation for all scenarios in a shared command-sequence test.

## 28 — The required validation entry point and acceptance runner are absent

- **severity:** low
- **area:** tests
- **file:line:** crates/bwr-core/examples/trace.rs:25
- **wrong:** `cargo run --release -p bwr-core --example validation` exits 101: no example target named validation. The tracked trace/startup examples print results but do not implement the requirements plan's machine-readable validation cases, conservation residuals, or acceptance failures. All 19 release tests pass despite the reproduced mass/energy and protection failures above.
- **consequence:** The requested validation command cannot be run, and trace execution can be mistaken for successful validation without checking any independent physical acceptance criterion.
- **fix:** Add examples/validation.rs with deterministic simulation-time scripts, explicit case results, machine-readable state/transfer/residual traces, and a nonzero exit on failed acceptance criteria. Register independent conservation, limiting-case, and step-refinement checks with documented tolerances. Keep regression baselines labeled as previous model behavior, not physical truth.

## 29 — Malformed trace events are silently omitted

- **severity:** low
- **area:** correctness
- **file:line:** crates/bwr-core/examples/trace.rs:19
- **wrong:** filter_map discards event strings whose time cannot be parsed; the later command match also silently skips unknown event names. `cargo run --release -p bwr-core --example trace -- full 1 scram@typo` exits successfully and ends with scram: None, with no rejected-event diagnostic.
- **consequence:** A mistyped fault schedule produces an apparently successful trace of a different scenario, undermining the example's use as transient review evidence.
- **fix:** Parse all scenario arguments and events into validated typed values before constructing the simulation. Reject unknown names, malformed syntax, and nonfinite/negative times with a diagnostic and nonzero exit status. Add parser tests ensuring an invalid requested event cannot be silently removed.

FINDINGS: 29
