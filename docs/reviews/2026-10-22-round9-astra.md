1. Reviewed `7b9c0ae..fdf5cbc` and the requested core, examples, tests, desktop and UI scope; read only the two specified round8 checklists under `docs/reviews/`.
2. `cargo test --release -p bwr-core` PASS: 89 unit + 15 limits + 36 plant = 140 tests; zero failures and zero doctests.
3. Workspace release build, UI production build, validation and startup `0.4` PASS; startup synchronizes at 4716 s and ends at 39.3% power without scram.
4. Deduplicated prior open items: fixed=1, partial=3, notfixed=0, regressed=0; one new medium numerical-physics finding in the round9 integration path.
5. Release timing on this Apple M1 Pro supports 1x-10x: the slowest measured 10x workload consumes 0.923% of its nominal wall-time budget; desktop rendering was not exercised.
6. Verdict (a), works as intended: NO across the full accepted core-transient domain; the endpoint heat defect is fixed, but post-excursion prompt-drop energy exceeds the independent solution by 6.1%-6.7% (R1).
7. Verdict (b), physics appropriate for a BWRX-300 training simulator: NO for qualified quantitative operator training; the successful nominal trajectories establish internal acceptance, not plant-response fidelity.
- (c) Bound transient source-energy error for nonequilibrium precursor states and qualify startup/transient responses against independent references and explicit accuracy tolerances.
- (c) Qualify IC mission duration, pool depletion and replenishment, together with the acknowledged core-pressure and other lumped-model approximations.
- (c) Validate controls, protection, instruments, thermal limits and desktop/browser operator exercises within a defined training scope.

## Prior open items

The supplied round8 reports use R-numbers for new findings, not N-numbers. Sol raises none; Astra raises R1. Their three PARTIAL rows describe the same three underlying items and are counted once each. Previously FIXED rows are outside this table. The original endpoint-rectangle defect is assessed separately from numerical accuracy of the neutron trajectory supplied to the new integral; round8-astra itself distinguished the smaller kinetics discretization error from its dominant heat-deposition defect.

| source | item | status | evidence |
| --- | --- | --- | --- |
| round8-sol and round8-astra, carried Sol 22 | Independent startup/transient qualification | PARTIAL | `crates/bwr-core/examples/validation.rs:205`: internal acceptance passes with criticality at 3625 s, synchronization at 4716 s and final power 0.472; independent plant transient references remain unavailable. |
| round8-sol and round8-astra, carried Astra 28 | IC mission-duration, depletion and replenishment qualification | PARTIAL | `crates/bwr-core/examples/validation.rs:93`: endurance still stops at 1800 s; fresh 1/2/3-train endpoints are 2.701/0.920/0.377 MPa, without a coupled mission-duration depletion/replenishment qualification case. |
| round8-sol and round8-astra, carried round5-sol R3 / round5-astra R5 | Core saturation pressure and hydrostatic approximation | PARTIAL | `crates/bwr-core/src/vessel.rs:850`: saturation still uses a single mid-plane pressure; the cold-core hydrostatic/subcooling regression at `:1402` passes, while axial pressure nodes and head flow losses remain explicitly omitted. |
| round8-astra new R1 | Final neutron power applied over the entire plant step | FIXED | `crates/bwr-core/src/reactor.rs:411`: fission, decay and fuel/coolant energy now use the resolved population integral; the prior 0.010-reactivity fixture generates 297,527.429/297,527.429/297,407.492 J at dt=0.05/0.025/0.0125 s against an independent 294,435.836 J, replacing the former 1,593,820 J coarse-step result. |

## New findings

One new actionable finding concerns the round9 population-integral path and its new decay-step accuracy bound. No additional defect was established in the unchanged desktop/UI integration or the sampled water/steam limiting cases. The accepted design decisions and known omissions are retained below without counting them as new findings.

## R1 — Prompt-decay substep bound overstates post-excursion energy

- **severity:** medium
- **area:** physics
- **file:line:** crates/bwr-core/src/kinetics.rs:18; crates/bwr-core/src/kinetics.rs:162
- **wrong:** The new decay bound allows `(beta-rho)*h/Lambda` up to 0.15, and `advance_integrating` integrates backward-Euler populations with trapezoids. For the scalar prompt-decay limit `dn/dt=-a*n`, the resulting integral to completion is `n0/a * (1+a*h/2)`: the allowed excess approaches 7.5%, despite the comment promising approximately 1%. Equilibrium precursor fixtures mask much of this error with sustained delayed-neutron power. Isolated production check: start `Core::new(1e-4, 48, 40)`, hold total reactivity at 0.010 for 0.05 s, then clone that resulting neutron/precursor/decay/fuel state and hold reactivity at -0.020 for another 0.05 s, with coolant at 40 C and no feedback refresh. Initial neutron fraction for the second interval is 0.038132944019. A separate augmented matrix exponential, initialized from exactly that same state, gives 50,089.407121 J generated during the second interval. Production gives 53,150.110179 J at both dt=0.05 and 0.025 s, 53,032.392345 J at 0.0125 s and 53,150.110179 J at 0.001 s: excesses of 6.110%, 6.110%, 5.875% and 6.110%. Refining to 0.0001/0.00001 s gives 51,619.778146/50,242.444327 J, reducing the excess to 3.055%/0.306%. Final neutron fractions already agree closely: 0.000059684561 versus the reference 0.000059684005. Repeating the second interval at reactivity -0.070 gives 18,648.922426 J versus 17,470.407877 J, a 6.746% excess. All fixture states are finite, temperatures remain around 40 C, and production reports no kinetics error.
- **consequence:** After a rapid rise leaves precursors below equilibrium with the neutron population, the following prompt decrease overproduces integrated fission heat and its decay-heat production. For the -0.020 fixture, delivered coolant energy is 5,009.713504 J versus 4,898.129254 J, a 2.278% excess; the remaining excess is stored in fuel. This is a component transient accuracy failure, not a demonstrated stock operator sequence or a 6% error in the entire two-stage event. Outer-step refinement at the standard plant timesteps gives false reassurance because it retains essentially the same internal kinetics steps. Whole-plant energy conservation also remains good because both storage and boundary accounting use the same biased `q_gen`. The new regression at `crates/bwr-core/src/reactor.rs:736` starts with equilibrium precursors and does not cover this condition.
- **fix:** Reduce the prompt-decay bound to at most 0.02 for an approximately 1% scalar-mode area bound, or use an independently verified integration method that controls the population integral's error. Add a rise-then-drop regression initialized with the actual nonequilibrium precursor/decay state, comparing generated energy, coolant energy and fuel storage with an augmented matrix exponential or independently converged ODE reference. Exercise -0.020 and -0.070 drops and refine the internal kinetics timestep as well as the outer plant timestep. Recheck 1x-10x throughput after changing the bound; the current benchmark shows substantial headroom.

## Verification evidence and limits

All five required commands exited 0 at `fdf5cbc597220d92a2018402b903971986059cf0`:

| command | evidence |
| --- | --- |
| `cargo test --release -p bwr-core` | 140 tests passed; `/tmp/bwrsim-round9-astra/test.log`. |
| `cargo build --release --workspace` | Core and Tauri release build completed; `/tmp/bwrsim-round9-astra/build.log`. |
| `npm --prefix ui run build` | TypeScript and Vite production build completed; `/tmp/bwrsim-round9-ui.log`. |
| `cargo run --release -p bwr-core --example validation` | Eight cases plus refinement, IC train-count and cold-condenser checks passed; `/tmp/bwrsim-round9-astra/validation.log`. |
| `cargo run --release -p bwr-core --example startup -- 0.4` | Synchronization at 4716 s; end at 4970 s, power 0.393, no scram; `/tmp/bwrsim-round9-astra/startup.log`. |

Validation now includes fuel energy in inventory and uses `core.q_gen` as the boundary source. Its maximum reported absolute whole-plant water residual is 8.38e-9 kg and maximum relative energy residual is 5.63e-12. Turbine-trip refinement from dt=0.05 to 0.025 s changes peak pressure by +0.0012 MPa and minimum level by -0.01 cm. These checks establish conservation and convergence of the sampled trajectories; source accuracy still requires an independent reference.

The original equilibrium-precursor prompt-drop fixture at reactivity -0.020 generates 1,354.416972 J versus the independent 1,348.234605 J, a 0.459% excess. The original rapid-rise fixture's remaining source error is 1.050%. The coupled cold-start boundary fixture from round8 also accepts its first scan with 297,527.429 J, fuel temperature 40.019692 C, no kinetics/model error, and short-period scram at the end of the scan. These results support closure of the original endpoint-rectangle error while exposing the additional precursor-state coverage required by R1.

Fresh admission and conservation checks retain the intended turbine envelope. At 90 C, gas fractions 0, 0.000999 and 0.001 admit steam; 0.001001, 0.1 and 0.78335823 admit zero turbine water, gas and shaft power. A bypass purge from gas fraction 0.001444277 restores admission at 2.30 s without a latched trip. One-second blocked-admission bypass runs export 13.389461793/13.367877186/13.357120056 kg air at dt=0.05/0.025/0.0125 s, each equal to condenser receipt. Maximum per-step vessel water-plus-air residual is 3.35e-11 kg and water/metal energy residual is below 7.49e-6 J. The release limits suite also passes the water-solid, unsupported turbine-property and cold-core cases.

A fresh pure-water expansion sweep used the installed `iapws` 1.5.5 implementation of [IAPWS-IF97](https://iapws.org/technical-guidance/release/IF97-Rev), independently of production steam functions. For dome enthalpies at 0.2/1/7.2/15 MPa, inlet pressures from 6 to 200 kPa and exhaust at 5.6 kPa, the model's isentropic drop remained below the reference. The largest sampled underestimate was 19.7% in a strongly throttled, superheated state. This is consistent with the documented conservative superheat approximation; it does not establish quantitative turbine performance accuracy or constitute a new thermodynamic-bound violation. Gas-bearing turbine expansion was excluded as instructed.

Release performance was measured without competing build jobs on this arm64 Apple M1 Pro. Each workload ran 3,000 calls to `Sim::advance(0.02)`, with one snapshot every five calls, representing 60 s of nominal desktop time. Requested speeds were 1x, 2x, 5x and 10x. Cold shutdown, hot standby, full power, manual scram and an outward rod-target fixture all completed without model errors. Across the 20 runs, compute throughput was 982-2,239 simulated seconds per measured execution second. At 10x:

| workload | execution time for 600 simulated s | fraction of 60 s budget | p99 advance/snapshot time |
| --- | --- | --- | --- |
| Cold shutdown | 0.421 s | 0.702% | 0.187 ms |
| Hot standby | 0.469 s | 0.782% | 0.197 ms |
| Full power | 0.271 s | 0.451% | 0.146 ms |
| Manual scram | 0.554 s | 0.923% | 0.299 ms |
| Outward rod target | 0.274 s | 0.457% | 0.158 ms |

The largest individual 10x call took 1.106 ms, below the host's nominal 20 ms wake interval. At 10x the accumulator ended at zero; the 1x fixture retained approximately one 0.05 s integration quantum because of floating-point clock accumulation, not throughput exhaustion. These are computation measurements, excluding actual sleep, Tauri event serialization/IPC, rendering and OS scheduling. They support adequate engine capacity at 1x-10x on this machine, not a measured GUI responsiveness guarantee or performance qualification on other hardware.

The changed files are limited to `kinetics.rs`, `reactor.rs` and `examples/validation.rs`; desktop/UI source and serialization contracts are unchanged. Existing Rust contract tests and both builds pass. A freshly bundled mock preserves a fractional rod target of 0.125, stops at the actual position, and reports zero turbine flow and extraction heat five seconds after a turbine trip, with feedwater temperature 225.148832 C retained by heater storage. Interactive desktop/browser operation was not exercised.

Review-only probes and logs are under `/tmp/bwrsim-round9-astra/`. Reproduce the general limiting-state checks with `cargo run --release --offline --manifest-path /tmp/bwrsim-round9-astra/Cargo.toml --bin round9-review-probe`; append `-- bench` for timing. Reproduce R1 with `cargo run --release --offline --manifest-path /tmp/bwrsim-round9-astra/Cargo.toml --bin drop`, whose captured output is `drop.log`, then run `/tmp/iapwsenv/bin/python /tmp/bwrsim-round9-astra/drop-reference.py`. The reference uses SciPy 1.18.1 `expm` on a 20-state linear system containing neutrons, six precursors, nine decay groups, fuel temperature, two energy integrals and a constant source; it calls no production kinetics, decay or fuel update. `reference.py`/`reference.log` contain the original-fixture and IF97 cross-checks; `mock-check.cjs`/`mock.log` contain the mock check.

Accepted limits remain unchanged: vapour-only main steam and no water-solid delivery; no turbine admission above 1e-3 dome gas mass fraction; single mid-plane core saturation without axial pressure nodes/head losses; no choked-flow temperature correction; pure-vapour IC draw; condenser shell air outside water balances; fault-injected unreachable condenser dryout; unavailable independent plant transient references; and intentionally one-step-lagged reactivity feedback, including Doppler. None is re-raised as a new finding.

Only this report was written in the repository by this review. Required examples regenerated ignored validation traces. Production files and tests were unchanged; pre-existing and concurrently appearing untracked files, including the other round9 report, were not read or modified.

OPEN: fixed=1 partial=3 notfixed=0 regressed=0
NEW FINDINGS: 1
