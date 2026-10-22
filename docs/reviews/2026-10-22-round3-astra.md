1. Reviewed `c824f72..a64e870` and the requested current code; read only the two authorized existing files under `docs/reviews/`.
2. `cargo test --release -p bwr-core`: PASS, 73 unit tests plus 33 integration tests, zero failures and zero doctests.
3. `cargo build --release --workspace` and `npm --prefix ui run build`: PASS; no interactive desktop smoke test performed.
4. Both required examples PASS: validation runs eight scenarios plus refinements; startup synchronizes at 5311 s and reaches 39.2% at 5569 s without scram.
5. Prior open-item assessments: fixed=29, partial=2, notfixed=0, regressed=0 across 31 identified items; five findings below, including one confirmed round 3 regression.
6. Verdict (a), works as intended: NO across its accepted domain; nominal cases pass, but accepted liquid-through-turbine states produce impossible work and negative condenser inlet energy.
7. Verdict (b), physics appropriate for a BWRX-300 training simulator: NO for quantitative operator training; this remains a useful development model with unqualified physical and transient limits.

- (c) Close turbine/extraction energy and phase limits, reconcile delivered-flow pressure, and replace temperature-only steam/IC availability rules.
- (c) Add independent plant-response and mission-duration acceptance, reachable condenser-limit scenarios, and accepted-port conservation checks across depletion boundaries.
- (c) Correct browser boiling semantics and qualify controls, protection, fuel/thermal-hydraulic limits and instrument behavior against the intended training scope.

## Prior open items

Counts deduplicate the same carried identifier repeated in the two reports: six distinct original PARTIAL items, all eight carried Sol N-items, all six carried Astra N-items, and each report's own new R-findings (six Sol and five Astra). The supplied round 2 reports label their new sections R, not N, so both sets are explicitly covered. Every PARTIAL row in either supplied report maps to a row below. FIXED applies to the reported failure, not unrestricted qualification of its subsystem; new defects in related behavior are assessed separately.

| source | item | status | evidence |
| --- | --- | --- | --- |
| Both round 2 reports | Sol 9 | FIXED | `crates/bwr-core/src/bop.rs:669`: relief dryout rejects without mutation at 170.40/170.42 s for dt=0.05/0.01; maximum accepted-state reconstruction error is 1.53e-5 J. |
| Both round 2 reports | Sol 22 | PARTIAL | `crates/bwr-core/examples/validation.rs:202`: startup and three 1800 s IC cases now pass, but independent transient/mission acceptance is absent and the startup gate accepts 20% for its 40% target. |
| Both round 2 reports | Astra 19 | FIXED | `crates/bwr-core/src/vessel.rs:766`: compensated 100 kg/s returns at 40/150 C leave identical bulk T/E but change core inlet enthalpy by 1142.598 J/kg; accepted IC return now affects local mixing. |
| Both round 2 reports | Astra 28 | PARTIAL | `crates/bwr-core/examples/validation.rs:229`: train-count outcomes and cooling refinement run, but 30-minute model-internal inequalities do not establish independent IC endurance or plant transient accuracy; RWCU port accounting still fails R5. |
| Both round 2 reports | Sol 21 | FIXED | `src-tauri/src/lib.rs:75`: event snapshots publish measured simulated-time/wall-time rate and engine plus host backlog; `plant.rs:1152` tests engine fields, and TopBar renders both. Desktop runtime was not smoke-tested. |
| Both round 2 reports | Astra 21 | FIXED | `crates/bwr-core/tests/plant.rs:1119`: rejected advance retains 0.1 s; `api.rs:199` and `src-tauri/src/lib.rs:83` expose the retained engine debt plus host backlog. |
| Both, carried Sol findings | N1 | FIXED | `crates/bwr-core/tests/plant.rs:1036`: water-solid and vapor-limited request/accept/commit mass regressions pass; the gas-bearing primary-energy probe also now closes within 1.83e-5 J. |
| Both, carried Sol findings | N2 | FIXED | `crates/bwr-core/src/sim.rs:499`: vapor-limited delivery, Steam fields and snapshot all equal 0.0020007075 kg/s, extraction is zero and condenser inlet is +4135.462 W; chest-pressure and liquid-path defects are separate R2/R1. |
| Both, carried Sol findings | N3 | FIXED | `crates/bwr-core/tests/plant.rs:1104`: repeated failed accelerating-turbine steps and failed advance pass full compared-state and time-debt checks; `sim.rs:380` restores the expanded checkpoint. |
| Both, carried Sol findings | N4 | FIXED | `crates/bwr-core/tests/plant.rs:989`: the 20 s coupled hotwell-exhaustion regression passes; the vessel receives only the prelimited withdrawal committed by the condenser. |
| Both, carried Sol findings | N5 | FIXED | `crates/bwr-core/src/bop.rs:1268`: signed relief mass/enthalpy test passes; the original dry-shell limitation now rejects transactionally with closed accepted states at three timesteps. |
| Both, carried Sol findings | N6 | FIXED | `crates/bwr-core/src/bop.rs:1386`: heater signed-energy identity passes through extraction loss and delivery limiting; whole-plant scram/isolation residuals remain below 1.2e-13 relative to core heat. |
| Both, carried Sol findings | N7 | FIXED | `ui/src/sim/mock.ts:710`: executed full-power extraction is 384.4224 MW; after turbine trip and 5 s it is exactly zero with zero turbine steam, while stored heat keeps feed at 225.142 C. |
| Both, carried Sol findings | N8 | FIXED | `crates/bwr-core/tests/plant.rs:652`: payload, enum and nullability mutation rejection tests pass, as does the real Rust/TypeScript snapshot contract including the new clock fields. |
| Both, carried Astra findings | N1 | FIXED | `crates/bwr-core/src/sim.rs:484`: accepted IC transfers commit one ratio; both phase-limited integration tests pass and gas-bearing whole-loop energy closes under timestep refinement. |
| Both, carried Astra findings | N2 | FIXED | `crates/bwr-core/tests/plant.rs:989`: the original large-step hotwell shortfall reproduction passes combined inventory closure and accepted withdrawal equality. |
| Both, carried Astra findings | N3 | FIXED | `crates/bwr-core/tests/plant.rs:1104`: governor rollback, repeated rejection and preserved 0.1 s backlog tests pass; condenser-rejection rollback is additionally tested with an injected fault. |
| Both, carried Astra findings | N4 | FIXED | `crates/bwr-core/examples/startup.rs:85`: required run exits zero after sync at 5311 s and n=0.392 at 5569 s without scram; incomplete startup retains its nonzero exit gate. |
| Both, carried Astra findings | N5 | FIXED | `crates/bwr-core/src/bop.rs:1436`: coasting-pump regression passes first-scan delivery, monotone rundown and check-valve closure; hydraulics includes turning shafts. |
| Both, carried Astra findings | N6 | FIXED | `crates/bwr-core/src/sim.rs:475`: the vessel receives h_feed_delivered, the same value debited by the heater at `bop.rs:1029`; signed component and whole-plant tests pass. |
| round2-sol.md | R1 | FIXED | `crates/bwr-core/src/vessel.rs:608`: both IC and vessel use h_g(T)=2745919 J/kg at 150 C; one-second gas-case residuals are -1.38e-5, -1.83e-5 and +1.31e-6 J at dt=0.05/0.025/0.0125. |
| round2-sol.md | R2 | FIXED | `crates/bwr-core/src/bop.rs:673`: the 1 GW relief/depletion reproduction rejects the dry step instead of forcing an inconsistent temperature; accepted energy, vapor volume and pressure remain closed. |
| round2-sol.md | R3 | FIXED | `crates/bwr-core/src/vessel.rs:619`: compressed liquid h=u+pv is used by RWCU and solid main/vent ports; the 40 C withdrawal gives 171573.371 J/kg versus independent IF97 171573.418. |
| round2-sol.md | R4 | FIXED | `ui/src/sim/mock.ts:710`: executed trip transition gives extraction=0 with turbine flow=0, while feed temperature decays separately rather than being relabeled extraction. |
| round2-sol.md | R5 | FIXED | `ui/src/sim/mock.ts:857`: seal/heat/reopen probe retains 103.535627 kg while sealed, computes 0.115297288 MPa gas pressure at 90 C and total 0.171107656 MPa; reopening restores 0.101325 MPa. |
| round2-sol.md | R6 | FIXED | `crates/bwr-core/src/sim.rs:499`: published turbine plus bypass flow equals the accepted 0.0020007075 kg/s; feedforward reads the preceding committed flow at `:439`. Requested-flow chest pressure remains R2 below. |
| round2-astra.md | R1 | FIXED | `crates/bwr-core/tests/plant.rs:1205`: gas-bearing plant balance test passes; independent one-second whole-loop residual stays below 1.83e-5 J over three timestep sizes. |
| round2-astra.md | R2 | FIXED | `crates/bwr-core/src/bop.rs:693`: dry and out-of-domain condenser states reject without mutation; reproduced depletion converges to about 170.42 s and accepted EOS residual stays below 1.53e-5 J. |
| round2-astra.md | R3 | FIXED | `crates/bwr-core/src/steam.rs:204`: pressure-dependent outlet anchoring matches the storage EOS; independently checked 40 C/4.562267 MPa and 200 C/50.000049 MPa reference enthalpies pass the new regression. |
| round2-astra.md | R4 | FIXED | `crates/bwr-core/examples/validation.rs:293`: physical counters exclude numerical corrections; public 60 s cooling ends at 20.1630 C versus 20.1604 C refined, with no artificial energy correction. |
| round2-astra.md | R5 | FIXED | `ui/src/sim/mock.ts:846`: initial gas inventory, sealed pressure evolution and vent reopening all pass the executed browser probe; gas pressure agrees with its current head-space ideal-gas relation. |

## New findings

R3 is a confirmed round 3 behavioral regression. R1 and R4 are remaining modeling defects, exposed here with additional limiting checks. R2 and R5 are the two known incomplete details named in the request, quantified rather than presented as newly introduced defects. The known injected condenser-fault test limitation is discussed under verification and is not counted as another code defect.

## R1 — Liquid entering the turbine receives a steam-only work and extraction budget

- **severity:** high
- **area:** physics
- **file:line:** crates/bwr-core/src/sim.rs:503
- **wrong:** The round 3 outlet fix correctly carries compressed-liquid enthalpy, but `Turbine::step` at `bop.rs:263` still assigns work from flow and pressure alone, and `step_heater` at `bop.rs:1036` similarly assigns a fixed steam extraction budget. Start from FullPower, replace only vessel inventory/EOS with a consistent sealed, gas-free 110 C state of mass `1.002 * V * rho_f(110 C)`, and step 0.05 s. The accepted liquid flow is 259.512613 kg/s at 464246.666 J/kg and 4.101863 MPa: 120.478 MW enters, 168.336 MW becomes shaft work, and condenser inlet energy is -47.858 MW, with no model error. At dt=0.05/0.025/0.0125, assigned work remains 648.663/648.622/648.602 kJ/kg. Independent IF97 gives only about 37.2 kJ/kg ideal isentropic expansion work for that inlet down to 5.6 kPa. A 287 C compressed fixture also accepts 365.194 MW shaft plus 394.497 MW extraction and a negative condenser inlet. These are valid thermodynamic boundary fixtures, not claimed reachable operator sequences.
- **consequence:** A flooded main-steam path can produce impossible electric power while cooling the condenser with an incoming positive liquid mass flow. Port bookkeeping still balances because the condenser supplies the artificial deficit; a conservation PASS cannot validate this cycle.
- **fix:** Pass accepted inlet phase and thermodynamic state to turbine/heater calculations. If liquid carryover is outside the model, reject or isolate that path explicitly before crediting work. Otherwise calculate expansion from inlet h/s and backpressure, bound work by the isentropic limit, and allocate extraction and exhaust from one available-energy budget. Add coupled compressed-liquid fixtures that assert physically admissible exhaust enthalpy as well as conservation.

## R2 — Delivered steam flow and chest pressure belong to different hydraulic solutions

- **severity:** medium
- **area:** physics
- **file:line:** crates/bwr-core/src/sim.rs:504
- **wrong:** Accepted-flow scaling at lines 499-500 leaves `p_chest` from the unrestricted solve at `bop.rs:79`. In the existing vapor-limited fixture, start pressure is 7.202308719 MPa and delivered flow is 0.0020007075 kg/s, but chest pressure remains 7.004448771 MPa. The code's own line-resistance law gives 0.000003624 Pa drop at delivered flow, versus the retained 197859.948 Pa. Turbine work and the following valve controller consume that incompatible pressure.
- **consequence:** Limiting the vessel's supply fixes the flow indicators but leaves the expansion ratio and valve feedback based on an unachieved flow. This is a known remaining issue, not a regression in the newly committed flow telemetry.
- **fix:** Reconcile supply limitation, line loss and chest state in one supported model. Either model chest storage/source pressure during starvation or explicitly reject that unsupported state; do not combine a rescaled flow with the old quasi-steady pressure. Test the accepted state against the chosen pressure/flow relation and turbine energy balance.

## R3 — Browser gas correction treats subcooled core liquid as boiling

- **severity:** medium
- **area:** contract
- **file:line:** ui/src/sim/mock.ts:546
- **wrong:** Round 3 subtracts head-space gas pressure before deciding core void and exit quality, and uses the same partial-pressure temperature for inlet subcooling at line 744. Head-space vapor equilibrium and bulk core boiling are different conditions: the engine explicitly uses total mechanical pressure for core bubbles at `vessel.rs:798`. Executed cold load, seal, set coolant to 90 C, and evaluate the core at 8.7 MW: total pressure is 0.171107520 MPa, gas partial pressure 0.115297273 MPa, and even the mock's own saturation relation gives a 117.765 C boiling point at total pressure. Nevertheless the new code reports 7.6% core void and exit quality 0.0016. Bundling `c824f72` and HEAD and evaluating that identical state gives zero void/quality before round 3 and those nonzero values afterward.
- **consequence:** Browser heatup creates vapor feedback, level swell and circulation changes while the modeled bulk liquid remains subcooled, contradicting the desktop engine's gas/core separation. The new gas-pressure fields themselves are fixed; this is a distinct regression in their consumers.
- **fix:** Retain partial pressure for head-space vapor properties, but derive core boiling/subcooling from total pressure and the local liquid heating model. At minimum restore the total-pressure criterion in `updateVoids` and the subcooling readout. Add a sealed cold-heatup transition regression that checks voids as well as gas mass and pressure.

## R4 — The engine abruptly disables IC condensation below 100 C

- **severity:** medium
- **area:** physics
- **file:line:** crates/bwr-core/src/sim.rs:447
- **wrong:** `steam_avail = !eos.subcooled` inherits `subcooled: T < 100` from `vessel.rs:394`, although the revised EOS represents equilibrium vapor at all temperatures. Two otherwise equivalent gas-free, sealed vessel fixtures at 99.99 and 100.01 C contain 32.840 and 32.860 kg vapor at 101381.7 and 101454.1 Pa. With all IC valves fully open, 40 C pools, 500 kg tube inventories and 5 MW preceding duty per train, one accepted 0.05 s step gives total IC duty 0 versus 15.0434 MW and steam draw 0 versus 6.66686 kg/s. No phase exhaustion or reversal of the 60 K condensing temperature difference occurs across that 0.02 C interval. Main-line availability and the below-100 C core saturation rule share the same atmospheric-temperature shortcut.
- **consequence:** Cooldown/low-pressure heat-removal behavior has an artificial discontinuity unrelated to available vapor or pressure. Results near that boundary cannot establish passive-cooling endurance, even though the new 1800 s IC cases pass above it.
- **fix:** Determine condensation from actual vapor availability, phase state and primary-to-pool temperature/pressure driving conditions, using the existing accepted-mass limit. Use mechanical pressure for core saturation. If operation below a defined condition is intentionally unsupported, declare and enforce that model boundary instead of silently setting heat transfer to zero. Add continuous temperature/pressure sweeps across 100 C and a cooldown case that crosses it.

## R5 — Vessel balance checks still count requested RWCU flow

- **severity:** low
- **area:** tests
- **file:line:** crates/bwr-core/examples/validation.rs:261
- **wrong:** Both this helper and `crates/bwr-core/tests/plant.rs:935` use `fw.rwcu_reject` in their mass and energy sums, despite vessel phase limiting and downstream use of `flows.w_liquid`. Executed gas-free 200 C low-inventory fixture with `M=1.00101 * V * rho_g(200 C)`, no feed/main steam and a valid 15 kg/s RWCU setting: the 0.05 s step succeeds and actually delivers 0.395660957 kg/s, changing vessel mass by -0.019783048 kg. The checker assumes -0.75 kg and invents a +0.730216952 kg residual. Its energy sum is wrong by the same flow difference times the liquid-port enthalpy.
- **consequence:** Depletion acceptance can reject a conserving step and cannot distinguish a real port mismatch from an unfulfilled request. Existing selected trajectories pass because they do not activate this limitation; this is the known uncompleted RWCU accounting change.
- **fix:** Use `sim.vessel.flows.w_liquid` for both mass and enthalpy sums, keep requested/shortfall telemetry separate, and require an accepted step before accumulating ports. Add this limited-liquid boundary case to the balance gate.

## Verification evidence and limits

All five required commands exited 0 on `a64e870adbcb0f452bf629e82611f166f32e3798`. Logs and review-only probes are in `/tmp/bwrsim-round3-astra/`: `test.log`, `test-list.log`, `build.log`, `ui.log`, `validation.log`, `startup.log`, `src/main.rs`, `probes.log`, `mock-probes.cjs`, `mock-probes.log`, `mock-regression.log`, and `if97.log`. Reproduce the Rust boundary checks with `cargo run --release --offline --manifest-path /tmp/bwrsim-round3-astra/Cargo.toml`; browser probes run the bundled current mock without starting its timer. No production source or test was edited. Generated validation traces are ignored; unrelated untracked files were left untouched.

The validation runner's largest absolute whole-plant mass residual is 3.73e-9 kg and largest relative energy residual is 3.60e-13. Startup validation observes criticality at 3649 s, synchronization at 5308 s, and 46.1% final power online without scram. One/two/three IC train cases end at 2.751/0.930/0.355 MPa after 1800 s; peak pressures are 7.275/7.249/7.233 MPa. Turbine-trip refinement changes peak pressure by +0.0012 MPa and minimum level by about -0.02 cm. Independent cold-condenser checks end at 20.163016/20.160441/20.160440 C for dt=60/1/0.05 s, with physical energy residual below 9e-6 J.

The condenser fix satisfies the prior requested option of rejecting unsupported dry states; it does not model dry-shell thermodynamics. The integrated rollback test at `crates/bwr-core/tests/plant.rs:1180` deliberately adds 1 GJ to an otherwise initialized condenser. It proves rollback of an injected rejection, not that a reachable command sequence reaches and handles the boundary. That known coverage gap remains. The standalone dryout sequence is physically closed through every accepted step and was reproduced at dt=1/0.05/0.01 s.

Independent property checks used the local Python `iapws` implementation of [IAPWS-IF97](https://iapws.org/technical-guidance/release/IF97-Rev). Compressed outlet checks now agree with independently calculated reference enthalpies; R1's turbine work limit uses a separate isentropic expansion calculation, not the simulator's pressure-factor formula. R1, R2 and R4 deliberately exercise consistent boundary fixtures; their reachability from ordinary operator actions was not established.

The new startup and IC gates are useful regression tests, but their thresholds remain model acceptance assumptions. `startup_accept` accepts final power of 0.20 for the stated 0.40 target and imposes no upper tracking bound. The IC cases cover 30 minutes with forced train availability, not depletion, replenishment, low-pressure crossover or a mission-length reference trajectory. The published BWRX-300 description specifies one IC train for AOOs and seven-day decay-heat removal with two trains for the described LOCA basis; this is a target for separately scoped qualification, not a claim that this simulator models LOCA. See [GE 005N9751 Rev J, section 3.3](https://www.gevernova.com/content/dam/gevernova-nuclear/global/en_us/documents/carbon-free-power/005N9751-BWRX-300-General-Description.pdf).

Clock fields were checked through source, schema gates, engine tests and successful desktop/UI compilation. The desktop event stream computes measured rate, while the direct `sim_snapshot` command still returns the documented engine nominal rate. No live desktop timing or UI interaction acceptance was performed. Browser rod target 12.25 and STOP at 12.6 remain exact in executed checks.

OPEN: fixed=29 partial=2 notfixed=0 regressed=0
NEW FINDINGS: 5
