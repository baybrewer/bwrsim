# Package 10: Unify Runtime Behavior and Publish the Fidelity Record — Implementation Plan (r2)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. The master plan `docs/superpowers/plans/2026-10-20-refinement-master.md` (r2) owns every shared name and signature; if this file and the master disagree, the master wins.

**Goal:** Make the supported application path (Tauri + Rust engine) show exactly the engine's behaviour with honest time accounting in both the engine and the shell, step-level extrema on every emitted snapshot, a compiler-checked UI contract, an unmistakable browser-mock label, and a published fidelity record that matches the executable and its tests.

**Architecture:** `Sim::advance` keeps a bounded backlog instead of zeroing it and reports effective speed and dropped time; the Tauri shell stops clamping wall-clock time so the engine's backlog rule is the only time policy. `Sim` tracks per-step `Extrema` that are seeded from current values (never zeros) and consumed only by `snapshot_with_extrema`. A Rust integration test generates `ui/src/sim/contract.generated.ts` (scenario fixtures, synthesized enum/nullable fixtures, a reverse key check) and a Rust unit-suffix test enforces field naming, so `tsc` and `cargo test` fail when `api.rs` and `types.ts` diverge. The trend buffer keeps every snapshot and draws the extrema envelope. `docs/validation/results.md` is generated from the validation runner's `--report` output (validation and conservation tables) plus the model docs from Packages 4–9.

**Tech Stack:** Rust 2021, serde/serde_json (dev-dependency, available to tests and examples), Tauri 2.12, TypeScript 5.6 with `strict`, `noUnusedLocals`, `resolveJsonModule` (already on), React 18, Vite 5. No new dependencies.

**Spec package:** "Package 10: Unify runtime behavior and publish the fidelity record" in `docs/superpowers/plans/2026-10-20-existing-systems-refinement.md`. **Assumes Packages 1–9 are complete and committed**: `Sim` already has `ledger` (with `integrated`), `log`, `faults: Vec<Fault>`, `domain_errors`, `sensors`, `balance`, and the constructor epilogue `Sim::finalize_init(&mut self)`; `Snapshot` already has every field in the master "API contract additions summary" for P3–P8 (`unsupported`, `vessel.regime`, `neutronics.cladTempC`, `fuelEnvelopeExceeded`, `rods.motion`, sensor fields, P6/P7 fields); `examples/validation.rs`, `standard_cases()`, `run_case`, `Trace::from_csv(&str)`, `Trace::metrics()`, `compare()`, `Tolerance::{numerical, physical, source_comparison}`, trace schema v2 with `*_max_int` / `*_min_int` columns, `docs/validation/cases.csv` with the locked header `id,scenario,duration_s,purpose,reference,tolerance_class,role,status`, `docs/validation/baseline/*.csv`, and `.github/workflows/validate.yml` exist.

## Global Constraints

Inherited verbatim from the master plan; repeated here because the task implementer sees only this file.

- "Keep Rust authoritative." The browser mock is a UI demonstration only.
- "No previous-test failure is concealed by updating expected output."
- "No claim of exact design reproduction, regulatory approval, or validation against unavailable plant data is made."
- No new Rust or npm dependencies without asking Jim first. Hand-roll CSV/JSON/TS emission with `std`, `serde_json`.
- Commands: `cargo test --release -p bwr-core`, `npm --prefix ui ci`, `npm --prefix ui run typecheck`, `npm --prefix ui run build`, `cargo run --release -p bwr-core --example validation`.
- Every numeric constant added or changed gets a row in `docs/model/parameters.csv` (header `id,value,unit,definition,reference_state,source_design,status,source_ids,interval_lo,interval_hi,assumption_id,location,notes`) with a status of `confirmed`, `derived`, `calibrated`, `esbwr-estimate`, `assumed`, or `unavailable`.
- Derives rule: every new struct or enum stored inside `Sim` derives at least `Clone, Debug`; anything placed in `api.rs` also derives `Serialize, Deserialize` with `#[serde(rename_all = "camelCase")]`.
- Every consumer of a transfer uses the applied (post-allocation) value. (P10 adds no consumers; the conservation report reads the ledger, which records applied flows.)
- Commit after every task. Commit messages end with `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.
- 2-space indentation in TypeScript; match surrounding Rust style by hand (do not run `cargo fmt` if the component is not installed).
- Jim does not test locally for deployed builds: the final task commits **and pushes**.
- After touching UI layout (Tasks 1, 5, 7), run the `frontend-design-audit` skill (`/fda:quick`) before calling the package done (CLAUDE.md).

**Acceptance gate (verbatim from the spec):** "Users see the same authoritative engine behavior in the supported application path. The fidelity record matches the executable and its tests."

**Baseline defects this package fixes (master r2 items 9, 10, 11):**

9. `sim.rs` `advance()`: when `n >= 4000` the accumulator is zeroed and simulation time is silently lost.
10. `history.ts`: 1 s sampling of 10 Hz snapshots, and 10 Hz snapshots of 20 Hz physics, can hide sub-second excursions.
11. `src-tauri/src/lib.rs`: `let dt = (now - last).as_secs_f64().min(0.5)` discards wall-clock time whenever the thread stalls more than 0.5 s (sleep, lock contention); a second silent time-loss path independent of defect 9.

## Names this package introduces beyond the master contract

| Name | Where | Meaning |
|---|---|---|
| `Sim::backlog_s(&self) -> f64` | `sim.rs` | read access to the private accumulator |
| private `overload_logged: bool` | `sim.rs` | one log line per overload episode |
| `Extrema::update(&mut self, current: &Extrema)` | `api.rs` | per-step fold (step-order item 16 names it `extrema.update`) |
| `Sim::extrema_now(&self) -> Extrema` | `sim.rs` | current values packed as an `Extrema` with `steps = 0`; the argument to `Extrema::seed` |
| `SHELL_WALL_DT_CLAMP_S` | `parameters.csv` only | the removed `0.5` s shell clamp, recorded for traceability |
| `RodMotion::label(&self) -> &'static str` | `reactor.rs` | the five contract strings; P8's `snapshot()` should already use it, else add it here |
| `TrendSample.powerPctMax/pressureMpaMax/levelNrCmMin/levelNrCmMax` | `history.ts` | folded extrema |
| `Series.envelope?: { lo; hi }` | `StripChart.tsx` | translucent band between two sample accessors |
| `UNIT_SUFFIXES`, `DIMENSIONLESS` | `tests/contract.rs` | unit-suffix rule and its explicit allowlist |
| `export const _exhaustive` | `contract.generated.ts` | exported so `noUnusedLocals` accepts it |
| `examples/record_snapshots.rs` | engine | writes `ui/src/sim/recorded/fullPower.json` |
| `?replay=1` | `mock.ts` | browser mock replays recorded Rust snapshots |
| `--report` | `examples/validation.rs` | prints the validation and conservation Markdown tables for `results.md` |
| `.demo-banner`, `.speed-eff`, `.speed-eff.slow`, `.dropped-chip` | `styles.css` | UI classes |

---

## File structure

```
crates/bwr-core/src/api.rs            Extrema (+ seed/update), Snapshot.effective_speed, .dropped_sim_time_s, .extrema
crates/bwr-core/src/sim.rs            MAX_STEPS_PER_ADVANCE, MAX_BACKLOG_S, advance, advance_with, backlog_s,
                                      effective_speed, dropped_sim_time_s, extrema, extrema_now, snapshot_with_extrema,
                                      finalize_init extension
crates/bwr-core/src/reactor.rs        RodMotion::label (only if P8 did not add it)
crates/bwr-core/tests/runtime.rs      time accounting, shell clamp, boundaries, pause, reset, extrema tests
crates/bwr-core/tests/contract.rs     generates ui/src/sim/contract.generated.ts; unit-suffix test
crates/bwr-core/examples/record_snapshots.rs
crates/bwr-core/examples/validation.rs  --report
src-tauri/src/lib.rs                  clamp removed, snapshot_with_extrema, boundary comment
ui/src/sim/types.ts                   mirror of api.rs additions
ui/src/sim/mock.ts                    relabel header, mirror fields, replay mode
ui/src/sim/bridge.ts                  initial snapshot via sim_snapshot
ui/src/sim/history.ts                 no decimation, extrema folded in
ui/src/sim/contract.generated.ts      generated
ui/src/sim/recorded/fullPower.json    generated
ui/src/components/StripChart.tsx      envelope band
ui/src/components/TrendPanel.tsx      envelopes on thermal power, pressure, level
ui/src/components/TopBar.tsx          effective speed, dropped chip, demo banner
ui/src/styles.css                     new classes
.github/workflows/validate.yml        contract freshness check
docs/model/parameters.csv             three runtime rows
docs/validation/results.md            fidelity record
README.md                             Status / Data sources rewrite
```

---

### Task 1: Honest time accounting in `Sim::advance`

**Files:**
- Modify: `crates/bwr-core/src/sim.rs` (constants, struct fields, `base`, `finalize_init`, `advance`, `advance_with`, `backlog_s`, `apply` for `SetSpeed`, `snapshot`)
- Modify: `crates/bwr-core/src/api.rs` (`Snapshot` fields)
- Modify: `docs/model/parameters.csv`
- Modify: `ui/src/sim/types.ts`, `ui/src/sim/mock.ts` (`blankSnapshot`), `ui/src/components/TopBar.tsx`, `ui/src/styles.css`
- Test: `crates/bwr-core/tests/runtime.rs` (new)

**Interfaces:**
- Consumes: `Sim::step(dt)`, `DT`, `Sim.log: Vec<(f64, String)>` (P2), `Sim::event(id, text)`, `Sim::finalize_init(&mut self)` (P2).
- Produces: `pub const MAX_STEPS_PER_ADVANCE: usize = 4000`, `pub const MAX_BACKLOG_S: f64 = 60.0`, `Sim.effective_speed: f64`, `Sim.dropped_sim_time_s: f64`, `Sim::backlog_s(&self) -> f64`, `Sim::advance_with(&mut self, real_dt: f64, cmds: Vec<Command>)`, `Snapshot.effective_speed`, `Snapshot.dropped_sim_time_s`.

- [ ] **Step 1: Write the failing tests**

Create `crates/bwr-core/tests/runtime.rs`:

```rust
//! Runtime behaviour: time accounting, shell clamp, command boundaries, reset, extrema.

use bwr_core::api::{Command, ScenarioName};
use bwr_core::sim::{DT, MAX_BACKLOG_S, MAX_STEPS_PER_ADVANCE};
use bwr_core::Sim;

#[test]
fn advance_never_loses_time_silently() {
    let mut sim = Sim::new(ScenarioName::ColdShutdown);
    sim.apply(Command::SetSpeed { speed: 100.0 });
    // 1000 s of wall time at 100x asks for 100 000 s of simulation = 2 000 000 steps.
    sim.advance(1000.0);
    let ran = MAX_STEPS_PER_ADVANCE as f64 * DT;
    assert!((sim.time - ran).abs() < 1e-6, "ran {} s, expected {ran}", sim.time);
    assert!((sim.backlog_s() - MAX_BACKLOG_S).abs() < 1e-6, "backlog {}", sim.backlog_s());
    let accounted = sim.time + sim.backlog_s() + sim.dropped_sim_time_s;
    assert!((accounted - 100_000.0).abs() < 1e-3, "accounted {accounted}");
    assert!(sim.dropped_sim_time_s > 0.0);
    assert!(sim.log.iter().any(|(_, m)| m.starts_with("Simulation overloaded")), "log: {:?}", sim.log);
    // 200 s of sim in 1000 s of wall time is an effective speed of 0.2x, not 100x.
    assert!(sim.effective_speed < 1.0, "effective speed {}", sim.effective_speed);
    let snap = sim.snapshot();
    assert!((snap.dropped_sim_time_s - sim.dropped_sim_time_s).abs() < 1e-9);
    assert!((snap.effective_speed - sim.effective_speed).abs() < 1e-9);
}

#[test]
fn backlog_drains_without_dropping() {
    let mut sim = Sim::new(ScenarioName::ColdShutdown);
    sim.apply(Command::SetSpeed { speed: 100.0 });
    // 2.5 s of wall at 100x = 250 s requested -> 200 s run, 50 s backlog, nothing dropped.
    sim.advance(2.5);
    assert!((sim.backlog_s() - 50.0).abs() < 1e-6, "backlog {}", sim.backlog_s());
    assert_eq!(sim.dropped_sim_time_s, 0.0);
    // A quiet call drains the backlog.
    sim.advance(0.0);
    assert!(sim.backlog_s() < DT);
    assert!((sim.time - 250.0).abs() < 1e-6);
    assert_eq!(sim.dropped_sim_time_s, 0.0);
    assert!(!sim.log.iter().any(|(_, m)| m.starts_with("Simulation overloaded")));
}

#[test]
fn effective_speed_tracks_real_throughput() {
    let mut sim = Sim::new(ScenarioName::ColdShutdown);
    sim.apply(Command::SetSpeed { speed: 10.0 });
    for _ in 0..50 {
        sim.advance(0.1); // 1 s of sim per call, 20 steps, never overloaded
    }
    assert!((sim.effective_speed - 10.0).abs() < 0.05, "effective {}", sim.effective_speed);
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test --release -p bwr-core --test runtime`
Expected: compile error `cannot find value MAX_BACKLOG_S in module bwr_core::sim` / `no method named backlog_s` / `no field dropped_sim_time_s`.

- [ ] **Step 3: Implement the constants, fields and `advance`**

In `crates/bwr-core/src/sim.rs`, replace `pub const DT: f64 = 0.05;` with:

```rust
/// Fixed physics time step, s.
pub const DT: f64 = 0.05;
/// Hard cap on physics steps per `advance` call so one wall-clock tick never stalls the shell.
/// parameters.csv: MAX_STEPS_PER_ADVANCE (assumed, numerical).
pub const MAX_STEPS_PER_ADVANCE: usize = 4000;
/// Requested-but-not-yet-simulated time kept between calls, s. Beyond this the
/// excess is counted in `dropped_sim_time_s` and reported, never silently discarded.
/// parameters.csv: MAX_BACKLOG_S (assumed, numerical).
pub const MAX_BACKLOG_S: f64 = 60.0;
```

Add to `pub struct Sim` (after `accumulator: f64,`):

```rust
    /// Sim-seconds advanced per wall-second, exponentially averaged over ~1 s of wall time.
    pub effective_speed: f64,
    /// Simulation time requested by the shell but discarded because the backlog cap was hit.
    pub dropped_sim_time_s: f64,
    /// True while an overload episode is being reported, so the log gets one line per episode.
    overload_logged: bool,
```

In `Sim::base`, after `accumulator: 0.0,` add `effective_speed: 1.0, dropped_sim_time_s: 0.0, overload_logged: false,`.

In `Sim::finalize_init` (the P2 constructor epilogue), add after the line that resets `accumulator`:

```rust
        self.dropped_sim_time_s = 0.0;
        self.overload_logged = false;
        self.effective_speed = self.speed;
```

Replace the whole `pub fn advance(&mut self, real_dt: f64)` with:

```rust
    /// Read access to the requested-but-not-yet-run simulation time, s.
    pub fn backlog_s(&self) -> f64 {
        self.accumulator
    }

    /// Apply `cmds` at a step boundary, then advance. Commands never land mid-step.
    pub fn advance_with(&mut self, real_dt: f64, cmds: Vec<Command>) {
        for c in cmds {
            self.apply(c);
        }
        self.advance(real_dt);
    }

    /// Advance by wall-clock `real_dt` seconds scaled by the sim speed.
    ///
    /// Runs at most `MAX_STEPS_PER_ADVANCE` steps. Whatever is left stays in the
    /// backlog up to `MAX_BACKLOG_S`; anything beyond that is added to
    /// `dropped_sim_time_s` and logged once per overload episode. The shell passes
    /// the true elapsed wall time (no clamp): this is the only time policy.
    pub fn advance(&mut self, real_dt: f64) {
        if self.paused {
            return;
        }
        self.accumulator += real_dt * self.speed;
        let mut n = 0usize;
        while self.accumulator >= DT && n < MAX_STEPS_PER_ADVANCE {
            self.step(DT);
            self.accumulator -= DT;
            n += 1;
        }
        if self.accumulator > MAX_BACKLOG_S {
            let dropped = self.accumulator - MAX_BACKLOG_S;
            self.accumulator = MAX_BACKLOG_S;
            self.dropped_sim_time_s += dropped;
            if !self.overload_logged {
                self.overload_logged = true;
                let text = format!("Simulation overloaded: dropped {dropped:.1} s of requested simulation time");
                self.log.push((self.time, text.clone()));
                self.event("overload", text);
            }
        } else if self.accumulator < DT {
            self.overload_logged = false;
        }
        if real_dt > 0.0 {
            let instantaneous = n as f64 * DT / real_dt;
            let a = (real_dt / 1.0).min(1.0);
            self.effective_speed += (instantaneous - self.effective_speed) * a;
        }
        // Forget acknowledgements of alarms that have cleared, so they flash again if they return.
        let active: HashSet<String> = self.active_alarms().into_iter().map(|a| a.0).collect();
        self.acked.retain(|id| active.contains(id));
    }
```

In `apply`, change the `SetSpeed` arm (leave `LoadScenario` as it is: it goes through the constructor and `finalize_init`, which now sets `effective_speed = speed`; but the constructor runs before `speed` is copied back, so set it after):

```rust
            Command::SetSpeed { speed } => {
                self.speed = speed.clamp(0.1, 100.0);
                self.effective_speed = self.speed;
            }
            Command::LoadScenario { name } => {
                let (speed, paused) = (self.speed, self.paused);
                *self = Sim::new(name);
                self.speed = speed;
                self.paused = paused;
                self.effective_speed = speed;
            }
```

In `api.rs` `Snapshot`, after `pub speed: f64,` add:

```rust
    /// Sim-seconds per wall-second actually achieved (compare with `speed`).
    pub effective_speed: f64,
    /// Requested simulation time the shell could not run and discarded, s. Zero when healthy.
    pub dropped_sim_time_s: f64,
```

In `Sim::snapshot`, after `speed: self.speed,` add `effective_speed: self.effective_speed, dropped_sim_time_s: self.dropped_sim_time_s,`.

- [ ] **Step 4: Run the tests**

Run: `cargo test --release -p bwr-core --test runtime`
Expected: `test result: ok. 3 passed`.

Run: `cargo test --release -p bwr-core`
Expected: all tests pass.

- [ ] **Step 5: Register the constants**

Append to `docs/model/parameters.csv` (no commas inside fields):

```
MAX_STEPS_PER_ADVANCE,4000,steps,Maximum physics steps run by one Sim::advance call,n/a,n/a,assumed,,1000,20000,,crates/bwr-core/src/sim.rs,Runtime bound on shell latency; numerical not physical
MAX_BACKLOG_S,60,s,Requested simulation time retained between advance calls before the excess is counted as dropped,n/a,n/a,assumed,,10,600,,crates/bwr-core/src/sim.rs,Excess is reported in dropped_sim_time_s and logged; never silent
```

- [ ] **Step 6: Mirror in the UI**

`ui/src/sim/types.ts`, in `interface Snapshot` after `speed: number;`:

```ts
  /** Sim-seconds per wall-second actually achieved; compare with `speed`. */
  effectiveSpeed: number;
  /** Requested simulation time the engine could not run and discarded, s. Zero when healthy. */
  droppedSimTimeS: number;
```

`ui/src/sim/mock.ts`, in `blankSnapshot()` next to `speed: 1,` add `effectiveSpeed: 1,` and `droppedSimTimeS: 0,`. In the `setSpeed` command arm add `s.effectiveSpeed = s.speed;` after the clamp.

`ui/src/components/TopBar.tsx`: replace the TIME CONTROL block with

```tsx
      <div className="tb-block">
        <div className="tb-label">TIME CONTROL</div>
        <div className="row gap4">
          <Btn tone="amber" active={s.paused} onClick={() => send({ type: "setPaused", paused: !s.paused })}>
            {s.paused ? "▶ RUN" : "❚❚ PAUSE"}
          </Btn>
          {SPEEDS.map((x) => (
            <Btn key={x} tone="plain" active={s.speed === x} onClick={() => send({ type: "setSpeed", speed: x })}>
              {x}×
            </Btn>
          ))}
          <span
            className={`speed-eff${!s.paused && s.effectiveSpeed < 0.9 * s.speed ? " slow" : ""}`}
            title="Effective simulation speed actually achieved"
          >
            EFF {s.effectiveSpeed.toFixed(1)}×
          </span>
          {s.droppedSimTimeS > 0 && (
            <span className="dropped-chip" title="Simulation time requested but not run (engine overloaded)">
              ⚠ DROPPED {s.droppedSimTimeS.toFixed(0)} s
            </span>
          )}
        </div>
      </div>
```

`ui/src/styles.css`, after the `.cmd-error { ... }` rule:

```css
.speed-eff {
  font-family: var(--mono);
  font-size: 11px;
  color: #9fb0a8;
  padding: 0 6px;
  align-self: center;
}
.speed-eff.slow {
  color: var(--amber);
}
.dropped-chip {
  font-family: var(--mono);
  font-size: 11px;
  color: #ffd2cc;
  background: #3a1513;
  border: 1px solid var(--red);
  border-radius: 3px;
  padding: 2px 6px;
  align-self: center;
}
```

- [ ] **Step 7: Typecheck and build the UI**

Run: `npm --prefix ui run typecheck && npm --prefix ui run build`
Expected: no errors; `vite build` reports `✓ built`.

- [ ] **Step 8: Commit**

```bash
git add crates/bwr-core/src/sim.rs crates/bwr-core/src/api.rs crates/bwr-core/tests/runtime.rs docs/model/parameters.csv ui/src/sim/types.ts ui/src/sim/mock.ts ui/src/components/TopBar.tsx ui/src/styles.css
git commit -m "fix(runtime): keep a bounded backlog in Sim::advance and report effective speed and dropped time

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 2: Remove the shell's 0.5 s wall-clock clamp (defect 11)

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `docs/model/parameters.csv`
- Test: `crates/bwr-core/tests/runtime.rs`

**Interfaces:**
- Consumes: `Sim::advance` backlog rule (Task 1).
- Produces: the shell passes true elapsed wall time; `parameters.csv` row `SHELL_WALL_DT_CLAMP_S` recording the removed constant.

- [ ] **Step 1: Write the demonstration test**

The shell loop cannot be unit-tested without a Tauri runtime, so the test reproduces the shell's old policy against the engine and shows the loss. Append to `crates/bwr-core/tests/runtime.rs`:

```rust
#[test]
fn shell_stall_is_accounted_not_clamped() {
    // Old src-tauri/src/lib.rs policy: dt = elapsed.min(0.5). A 2 s stall (sleep,
    // lock contention) handed the engine 0.5 s and nobody ever heard of the other 1.5 s.
    let stall_s = 2.0;
    let mut clamped = Sim::new(ScenarioName::ColdShutdown);
    clamped.advance(stall_s.min(0.5));
    let lost = stall_s - (clamped.time + clamped.backlog_s() + clamped.dropped_sim_time_s);
    assert!((lost - 1.5).abs() < 1e-9, "the clamp loses {lost} s with no record");

    // New policy: the shell passes the true elapsed time; the engine's backlog rule accounts for all of it.
    let mut honest = Sim::new(ScenarioName::ColdShutdown);
    honest.advance(stall_s);
    let accounted = honest.time + honest.backlog_s() + honest.dropped_sim_time_s;
    assert!((accounted - stall_s).abs() < 1e-9, "accounted {accounted}");
    assert!((honest.time - stall_s).abs() < 1e-9, "2 s at 1x is 40 steps, well under the cap");

    // A pathological stall (laptop asleep for an hour at 30x) is bounded and reported, not lost.
    let mut asleep = Sim::new(ScenarioName::ColdShutdown);
    asleep.apply(Command::SetSpeed { speed: 30.0 });
    asleep.advance(3600.0);
    let requested = 3600.0 * 30.0;
    let accounted = asleep.time + asleep.backlog_s() + asleep.dropped_sim_time_s;
    assert!((accounted - requested).abs() < 1e-3);
    assert!(asleep.dropped_sim_time_s > 0.0);
    assert!(asleep.log.iter().any(|(_, m)| m.starts_with("Simulation overloaded")));
}
```

- [ ] **Step 2: Run it**

Run: `cargo test --release -p bwr-core --test runtime shell_stall_is_accounted_not_clamped`
Expected: PASS (the engine side is already honest after Task 1; this test pins the demonstration). The defect lives in the shell, fixed next.

- [ ] **Step 3: Remove the clamp from the shell**

In `src-tauri/src/lib.rs`, in the worker loop replace

```rust
                    let dt = (now - last).as_secs_f64().min(0.5);
```

with

```rust
                    // True elapsed wall time, unclamped (defect 11). The engine's backlog rule in
                    // Sim::advance (MAX_BACKLOG_S, dropped_sim_time_s) is the only time policy; a
                    // long stall shows up as a DROPPED chip in the UI instead of vanishing here.
                    let dt = (now - last).as_secs_f64();
```

Run: `grep -n "min(0.5)" src-tauri/src/lib.rs`
Expected: no output.

Run: `cargo check -p bwrx300-sim`
Expected: no errors.

- [ ] **Step 4: Register the removed constant**

Append to `docs/model/parameters.csv`:

```
SHELL_WALL_DT_CLAMP_S,0.5,s,Former cap on wall-clock elapsed time passed from the Tauri shell to Sim::advance,n/a,n/a,assumed,,,,,src-tauri/src/lib.rs (removed in Package 10),Defect 11: discarded elapsed time silently; removed; MAX_BACKLOG_S applies instead
```

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/lib.rs docs/model/parameters.csv crates/bwr-core/tests/runtime.rs
git commit -m "fix(shell): pass true elapsed wall time to Sim::advance instead of clamping to 0.5 s (defect 11)

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 3: Command boundaries, pause, scenario reset, and initial snapshot delivery

**Files:**
- Modify: `crates/bwr-core/src/sim.rs` (doc comment on `apply`)
- Modify: `src-tauri/src/lib.rs` (boundary comment)
- Modify: `ui/src/sim/bridge.ts`
- Test: `crates/bwr-core/tests/runtime.rs`

**Interfaces:**
- Consumes: `Sim::advance_with` (Task 1), `Sim.faults: Vec<Fault>` + `Sim::set_fault` (P2), `Sim.ledger` (P2), `Sim.log` (P2), `Sim.domain_errors` (P3), `Fault::BypassUnavailable` (P2/P6), `ROD_FULL_OUT`, `SCRAM_TIME`, `Sim::finalize_init` (P2).
- Produces: guarantee that `apply` only runs between steps; `LoadScenario` resets every runtime accumulator and clears `faults`; UI receives a snapshot immediately on subscribe.

- [ ] **Step 1: Write the tests**

Append to `crates/bwr-core/tests/runtime.rs`:

```rust
use bwr_core::faults::Fault;
use bwr_core::params::{ROD_FULL_OUT, SCRAM_TIME};

#[test]
fn commands_apply_at_step_boundaries() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    let before = sim.core.rods.pos.clone();
    // Exactly one step with exactly one command: the scram is applied before the
    // step, so every rod has moved exactly one step of hydraulic travel.
    sim.advance_with(DT, vec![Command::ManualScram]);
    assert!(sim.scrammed);
    assert!((sim.time - DT).abs() < 1e-9);
    let v = ROD_FULL_OUT / SCRAM_TIME * DT;
    for (i, (p0, p1)) in before.iter().zip(&sim.core.rods.pos).enumerate() {
        let want = (p0 - v).max(0.0);
        assert!((want - p1).abs() < 1e-9, "rod {i}: {p0} -> {p1}, want {want}");
    }
}

#[test]
fn paused_advance_accumulates_nothing() {
    let mut sim = Sim::new(ScenarioName::HotStandby);
    sim.apply(Command::SetPaused { paused: true });
    sim.advance(10.0);
    assert_eq!(sim.time, 0.0);
    assert_eq!(sim.backlog_s(), 0.0);
    sim.apply(Command::SetPaused { paused: false });
    sim.advance(0.1);
    assert!((sim.time - 0.1).abs() < 1e-9, "time {}", sim.time);
}

#[test]
fn load_scenario_resets_runtime_state_but_keeps_speed() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    sim.set_fault(Fault::BypassUnavailable, true);
    assert_eq!(sim.faults.len(), 1);
    sim.apply(Command::SetSpeed { speed: 100.0 });
    sim.advance(1000.0);
    assert!(sim.dropped_sim_time_s > 0.0);
    assert!(!sim.log.is_empty());
    assert!(sim.ledger.mass_in_kg > 0.0 || sim.ledger.mass_out_kg > 0.0);

    sim.apply(Command::LoadScenario { name: ScenarioName::HotStandby });
    assert_eq!(sim.time, 0.0);
    assert!(sim.faults.is_empty(), "faults are test/instructor state and clear on load (master: LoadScenario clears faults)");
    assert_eq!(sim.dropped_sim_time_s, 0.0);
    assert_eq!(sim.backlog_s(), 0.0);
    assert!(sim.log.is_empty(), "log: {:?}", sim.log);
    assert!(sim.domain_errors.is_empty(), "hot standby is a valid initial condition");
    assert_eq!(sim.ledger.mass_in_kg, 0.0);
    assert_eq!(sim.ledger.mass_out_kg, 0.0);
    assert_eq!(sim.speed, 100.0);
    assert_eq!(sim.effective_speed, 100.0);
    assert_eq!(sim.extrema.steps, 0);
}

#[test]
fn constructors_leave_no_runtime_residue() {
    // full_power() settles for 12 minutes internally; finalize_init must hide that
    // from the log, ledger, backlog, dropped time and extrema the user sees at t = 0.
    for name in [ScenarioName::ColdShutdown, ScenarioName::HotStandby, ScenarioName::FullPower] {
        let sim = Sim::new(name);
        assert_eq!(sim.time, 0.0);
        assert!(sim.log.is_empty(), "{name:?} log: {:?}", sim.log);
        assert_eq!(sim.ledger.mass_in_kg, 0.0, "{name:?}");
        assert_eq!(sim.backlog_s(), 0.0, "{name:?}");
        assert_eq!(sim.dropped_sim_time_s, 0.0, "{name:?}");
        assert_eq!(sim.extrema.steps, 0, "{name:?}");
    }
}
```

- [ ] **Step 2: Run them**

Run: `cargo test --release -p bwr-core --test runtime`
Expected: `commands_apply_at_step_boundaries`, `paused_advance_accumulates_nothing` pass. `load_scenario_resets_runtime_state_but_keeps_speed` and `constructors_leave_no_runtime_residue` fail only on the `extrema.steps` assertion (compile error `no field extrema` until Task 4). If they fail on `log`, `ledger`, `backlog_s` or `dropped_sim_time_s`, `finalize_init` from P2 is incomplete: fix it there (it must set `time = 0.0`, re-base `ledger`, clear `log`, `events`, `acked`, `accumulator`, `dropped_sim_time_s`) rather than patching constructors. Leave the two `extrema` assertions in place; Task 4 makes them compile and pass.

- [ ] **Step 3: Document the boundary guarantee**

Add this doc comment above `pub fn apply(&mut self, cmd: Command)` in `sim.rs`:

```rust
    /// Apply one operator command. Commands are only ever applied between physics
    /// steps: the desktop shell holds the simulator mutex for the whole `advance`
    /// call, and `advance_with` applies its batch before stepping. A command can
    /// therefore never observe or alter a half-stepped plant. `LoadScenario`
    /// rebuilds the plant through the scenario constructor and `finalize_init`,
    /// which clears faults, log, ledger, domain errors raised after construction,
    /// backlog, dropped time and the extrema window; `speed` and `paused` survive.
```

In `src-tauri/src/lib.rs`, above `fn sim_command`, add:

```rust
/// Commands lock the same mutex the physics thread holds for a whole `advance`
/// call, so they are applied strictly between physics steps.
```

- [ ] **Step 4: Deliver the initial snapshot immediately in the bridge**

Replace `createTauriBackend` in `ui/src/sim/bridge.ts` with:

```ts
function createTauriBackend(): SimBackend {
  return {
    kind: "tauri",
    subscribe(cb) {
      let unlisten: (() => void) | null = null;
      let cancelled = false;
      void (async () => {
        const [{ listen }, { invoke }] = await Promise.all([
          import("@tauri-apps/api/event"),
          import("@tauri-apps/api/core"),
        ]);
        // Pull the current state first so the panels render at once instead of
        // waiting for the next 100 ms broadcast; this also covers reconnects after
        // a Vite hot reload. Fetching before listening keeps sim time monotonic
        // for the trend buffer (a broadcast can never be older than this fetch).
        try {
          const first = await invoke<Snapshot>("sim_snapshot");
          if (!cancelled) cb(first);
        } catch (e) {
          console.warn("sim_snapshot failed", e);
        }
        if (cancelled) return;
        const un = await listen<Snapshot>("sim-state", (e) => cb(e.payload));
        if (cancelled) un();
        else unlisten = un;
      })();
      return () => {
        cancelled = true;
        unlisten?.();
      };
    },
    async send(cmd) {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("sim_command", { cmd });
    },
  };
}
```

- [ ] **Step 5: Typecheck**

Run: `npm --prefix ui run typecheck`
Expected: no errors.

- [ ] **Step 6: Commit**

```bash
git add crates/bwr-core/src/sim.rs crates/bwr-core/tests/runtime.rs src-tauri/src/lib.rs ui/src/sim/bridge.ts
git commit -m "fix(runtime): document step-boundary command application and deliver the initial snapshot on subscribe

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

(The runtime test binary does not compile until Task 4 adds `extrema`; commit anyway, Task 4 follows immediately. If the executor's process requires green at every commit, squash Tasks 3 and 4 into one commit.)

---

### Task 4: Per-step extrema, seeded from current values, consumed only by `snapshot_with_extrema`

**Files:**
- Modify: `crates/bwr-core/src/api.rs` (`Extrema`, `Snapshot.extrema`)
- Modify: `crates/bwr-core/src/sim.rs` (`extrema` field, `extrema_now`, `step`, `finalize_init`, `snapshot_with_extrema`, `snapshot`)
- Modify: `src-tauri/src/lib.rs` (emit `snapshot_with_extrema`)
- Modify: `ui/src/sim/types.ts`, `ui/src/sim/mock.ts`
- Test: `crates/bwr-core/tests/runtime.rs`

**Interfaces:**
- Consumes: `self.core.kin.n`, `self.core.thermal_power()`, `self.vessel.eos.p`, `self.level_cm()` (sensed NR level, P8), `self.core.fuel_t`, `self.core.clad_t` (P5), `self.cond.pressure()`.
- Produces (master r2): `api::Extrema { power_frac_max, thermal_mw_max, pressure_mpa_max, level_nr_cm_min, level_nr_cm_max, fuel_t_c_max, clad_t_c_max, cond_pressure_kpa_max, steps: u32 }`, `Extrema::seed(snapshot_values: &Extrema) -> Extrema`, `Extrema::update(&mut self, current: &Extrema)`, `Sim.extrema: Extrema`, `Sim::extrema_now(&self) -> Extrema`, `Sim::snapshot_with_extrema(&mut self) -> Snapshot` (the only consumer of the window), `Snapshot.extrema`.

- [ ] **Step 1: Write the failing tests**

Append to `crates/bwr-core/tests/runtime.rs`:

```rust
#[test]
fn extrema_are_seeded_from_current_values_never_zeros() {
    let sim = Sim::new(ScenarioName::FullPower);
    let ex = sim.extrema;
    let snap = sim.snapshot();
    assert_eq!(ex.steps, 0);
    assert!((ex.pressure_mpa_max - snap.vessel.pressure_mpa).abs() < 1e-12);
    assert!((ex.level_nr_cm_min - snap.vessel.level_nr_cm).abs() < 1e-12);
    assert!((ex.level_nr_cm_max - snap.vessel.level_nr_cm).abs() < 1e-12);
    assert!((ex.thermal_mw_max - snap.neutronics.thermal_power_mw).abs() < 1e-9);
    assert!((ex.fuel_t_c_max - snap.neutronics.fuel_temp_c).abs() < 1e-9);
    assert!((ex.clad_t_c_max - snap.neutronics.clad_temp_c).abs() < 1e-9);
    assert!((ex.cond_pressure_kpa_max - snap.condenser.pressure_kpa).abs() < 1e-9);
    assert!(ex.power_frac_max > 0.9, "seeded from the live value, not zero");
    // snapshot() alone reports a zero-step window of current values and never consumes the real window.
    assert_eq!(snap.extrema.steps, 0);
}

#[test]
fn extrema_track_every_physics_step() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    let _ = sim.snapshot_with_extrema(); // start a fresh window
    let mut p_max = f64::MIN;
    let mut l_min = f64::MAX;
    let mut l_max = f64::MIN;
    let mut q_max = f64::MIN;
    let mut c_max = f64::MIN;
    for _ in 0..20 {
        sim.step(DT);
        let s = sim.snapshot();
        p_max = p_max.max(s.vessel.pressure_mpa);
        l_min = l_min.min(s.vessel.level_nr_cm);
        l_max = l_max.max(s.vessel.level_nr_cm);
        q_max = q_max.max(s.neutronics.thermal_power_mw);
        c_max = c_max.max(s.condenser.pressure_kpa);
    }
    let ex = sim.snapshot_with_extrema().extrema;
    assert_eq!(ex.steps, 20);
    assert!((ex.pressure_mpa_max - p_max).abs() < 1e-12);
    assert!((ex.level_nr_cm_min - l_min).abs() < 1e-12);
    assert!((ex.level_nr_cm_max - l_max).abs() < 1e-12);
    assert!((ex.thermal_mw_max - q_max).abs() < 1e-9);
    assert!((ex.cond_pressure_kpa_max - c_max).abs() < 1e-9);
    // The window was re-seeded: a second take has no steps and current values.
    let ex2 = sim.snapshot_with_extrema().extrema;
    assert_eq!(ex2.steps, 0);
    assert!((ex2.pressure_mpa_max - sim.snapshot().vessel.pressure_mpa).abs() < 1e-12);
}

#[test]
fn extrema_expose_excursions_between_snapshots() {
    // Snapshots are emitted every 1 s (20 steps). A scram at the start of the
    // window drops power within the window; the snapshot at the end shows low
    // power, but the extrema must still show the pre-scram peak.
    let mut sim = Sim::new(ScenarioName::FullPower);
    let _ = sim.snapshot_with_extrema();
    sim.apply(Command::ManualScram);
    for _ in 0..20 {
        sim.step(DT);
    }
    let snap = sim.snapshot_with_extrema();
    assert!(snap.neutronics.power_frac < 0.9, "power after 1 s of scram {}", snap.neutronics.power_frac);
    assert!(snap.extrema.power_frac_max > 0.95, "peak {}", snap.extrema.power_frac_max);
    assert!(snap.extrema.power_frac_max >= snap.neutronics.power_frac);
    assert!(snap.extrema.thermal_mw_max >= snap.neutronics.thermal_power_mw);
    assert_eq!(snap.extrema.steps, 20);
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test --release -p bwr-core --test runtime`
Expected: compile error `no field extrema on type Sim` / `no method named snapshot_with_extrema`.

- [ ] **Step 3: Implement `Extrema` and the window**

In `crates/bwr-core/src/api.rs`, before `pub struct Snapshot`:

```rust
/// Per-step extrema since the previous emitted snapshot, so a 10 Hz display cannot
/// hide a 20 Hz excursion. `steps` is how many physics steps the window covers.
/// Seeded from the current values (never zeros) and consumed only by
/// `Sim::snapshot_with_extrema`.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Extrema {
    pub power_frac_max: f64,
    pub thermal_mw_max: f64,
    pub pressure_mpa_max: f64,
    pub level_nr_cm_min: f64,
    pub level_nr_cm_max: f64,
    pub fuel_t_c_max: f64,
    pub clad_t_c_max: f64,
    pub cond_pressure_kpa_max: f64,
    pub steps: u32,
}

impl Extrema {
    /// Start a window: copies the current values as both min and max, `steps = 0`.
    pub fn seed(snapshot_values: &Extrema) -> Self {
        Self { steps: 0, ..*snapshot_values }
    }

    /// Fold one physics step (`current` carries the step's values in the max/min fields).
    pub fn update(&mut self, current: &Extrema) {
        self.power_frac_max = self.power_frac_max.max(current.power_frac_max);
        self.thermal_mw_max = self.thermal_mw_max.max(current.thermal_mw_max);
        self.pressure_mpa_max = self.pressure_mpa_max.max(current.pressure_mpa_max);
        self.level_nr_cm_min = self.level_nr_cm_min.min(current.level_nr_cm_min);
        self.level_nr_cm_max = self.level_nr_cm_max.max(current.level_nr_cm_max);
        self.fuel_t_c_max = self.fuel_t_c_max.max(current.fuel_t_c_max);
        self.clad_t_c_max = self.clad_t_c_max.max(current.clad_t_c_max);
        self.cond_pressure_kpa_max = self.cond_pressure_kpa_max.max(current.cond_pressure_kpa_max);
        self.steps += 1;
    }
}
```

In `Snapshot`, after `pub dropped_sim_time_s: f64,` add:

```rust
    /// Extrema over the physics steps since the previous emitted snapshot.
    pub extrema: Extrema,
```

In `sim.rs`: import `Extrema` (`use crate::api::{self, Command, Extrema, ModeSwitch, ScenarioName, Severity, Snapshot};`), add `pub extrema: Extrema,` to `Sim` after `dropped_sim_time_s`, and in `Sim::base` add `extrema: Extrema::default(),`. Add to `impl Sim`:

```rust
    /// Current values packed as an `Extrema` (each min and max field holds the live
    /// value, `steps = 0`): the argument to `Extrema::seed` and `Extrema::update`.
    pub fn extrema_now(&self) -> Extrema {
        let level = self.level_cm();
        Extrema {
            power_frac_max: self.core.kin.n,
            thermal_mw_max: self.core.thermal_power() / 1e6,
            pressure_mpa_max: self.vessel.eos.p / 1e6,
            level_nr_cm_min: level,
            level_nr_cm_max: level,
            fuel_t_c_max: self.core.fuel_t,
            clad_t_c_max: self.core.clad_t,
            cond_pressure_kpa_max: self.cond.pressure() / 1e3,
            steps: 0,
        }
    }

    /// Snapshot for emission: carries the extrema since the last emitted snapshot
    /// and re-seeds the window. This is the ONLY consumer of the window;
    /// `snapshot()` reports a zero-step window of current values.
    pub fn snapshot_with_extrema(&mut self) -> Snapshot {
        let window = std::mem::replace(&mut self.extrema, Extrema::seed(&self.extrema_now()));
        let mut snap = self.snapshot();
        snap.extrema = window;
        snap
    }
```

In `Sim::finalize_init`, add (after the sensors initialisation so `level_cm()` reads a sensed value):

```rust
        self.extrema = Extrema::seed(&self.extrema_now());
```

In `Sim::step`, at step-order position 16 (right after `self.ledger.record(dt, &flows);` and before the domain-error drain):

```rust
        let now = self.extrema_now();
        self.extrema.update(&now);
```

In `snapshot()`, after `dropped_sim_time_s: ...,` add `extrema: Extrema::seed(&self.extrema_now()),`.

- [ ] **Step 4: Run the tests**

Run: `cargo test --release -p bwr-core --test runtime`
Expected: `test result: ok. 11 passed` (3 from Task 1, 1 from Task 2, 4 from Task 3, 3 from this task).

- [ ] **Step 5: Emit extrema from the desktop shell**

In `src-tauri/src/lib.rs`, change the emission inside the worker loop:

```rust
                    let snap = {
                        let mut s = match worker.lock() {
                            Ok(s) => s,
                            Err(p) => p.into_inner(),
                        };
                        s.advance(dt);
                        if now - last_emit >= Duration::from_millis(100) {
                            // Carries per-step extrema since the last emit and re-seeds that window.
                            Some(s.snapshot_with_extrema())
                        } else {
                            None
                        }
                    };
```

`sim_snapshot` keeps using `snapshot()` so a reconnect never consumes the window.

- [ ] **Step 6: Mirror in the UI**

`ui/src/sim/types.ts`, before `export interface Snapshot`:

```ts
/** Per-step extrema since the previous emitted snapshot (`steps` physics steps). */
export interface Extrema {
  powerFracMax: number;
  thermalMwMax: number;
  pressureMpaMax: number;
  levelNrCmMin: number;
  levelNrCmMax: number;
  fuelTCMax: number;
  cladTCMax: number;
  condPressureKpaMax: number;
  steps: number;
}
```

(serde camelCase of `fuel_t_c_max` is `fuelTCMax` and of `clad_t_c_max` is `cladTCMax`; Task 6's generated contract will catch any mismatch.)

In `Snapshot` after `droppedSimTimeS: number;` add `extrema: Extrema;`.

`ui/src/sim/mock.ts`: import `Extrema` in the type import list; add a helper above `class MockSim`:

```ts
/** Current values as a zero-step extrema window (mirrors Sim::extrema_now). */
function extremaNow(s: Snapshot): Extrema {
  return {
    powerFracMax: s.neutronics.powerFrac,
    thermalMwMax: s.neutronics.thermalPowerMw,
    pressureMpaMax: s.vessel.pressureMpa,
    levelNrCmMin: s.vessel.levelNrCm,
    levelNrCmMax: s.vessel.levelNrCm,
    fuelTCMax: s.neutronics.fuelTempC,
    cladTCMax: s.neutronics.cladTempC,
    condPressureKpaMax: s.condenser.pressureKpa,
    steps: 0,
  };
}
```

In `blankSnapshot()` add `extrema: { powerFracMax: 0, thermalMwMax: 0, pressureMpaMax: 0, levelNrCmMin: 0, levelNrCmMax: 0, fuelTCMax: 0, cladTCMax: 0, condPressureKpaMax: 0, steps: 0 },` and at the end of `loadScenario(...)` (after the scenario values are set) add `this.s.extrema = extremaNow(this.s);`. Replace `private tick(realDt)` with:

```ts
  private tick(realDt: number) {
    const s = this.s;
    const ex = extremaNow(s);
    if (!s.paused) {
      let remaining = realDt * s.speed;
      while (remaining > 1e-9) {
        const dt = Math.min(0.05, remaining);
        this.step(dt);
        remaining -= dt;
        const now = extremaNow(s);
        ex.powerFracMax = Math.max(ex.powerFracMax, now.powerFracMax);
        ex.thermalMwMax = Math.max(ex.thermalMwMax, now.thermalMwMax);
        ex.pressureMpaMax = Math.max(ex.pressureMpaMax, now.pressureMpaMax);
        ex.levelNrCmMin = Math.min(ex.levelNrCmMin, now.levelNrCmMin);
        ex.levelNrCmMax = Math.max(ex.levelNrCmMax, now.levelNrCmMax);
        ex.fuelTCMax = Math.max(ex.fuelTCMax, now.fuelTCMax);
        ex.cladTCMax = Math.max(ex.cladTCMax, now.cladTCMax);
        ex.condPressureKpaMax = Math.max(ex.condPressureKpaMax, now.condPressureKpaMax);
        ex.steps += 1;
      }
    }
    s.extrema = ex;
    this.updateAlarms();
    this.emit();
  }
```

- [ ] **Step 7: Typecheck, build, and the Tauri crate compiles**

Run: `npm --prefix ui run typecheck && npm --prefix ui run build && cargo check -p bwrx300-sim`
Expected: no errors.

- [ ] **Step 8: Commit**

```bash
git add crates/bwr-core/src/api.rs crates/bwr-core/src/sim.rs crates/bwr-core/tests/runtime.rs src-tauri/src/lib.rs ui/src/sim/types.ts ui/src/sim/mock.ts
git commit -m "feat(runtime): per-step extrema seeded from live values travel with every emitted snapshot

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 5: Trend buffer keeps every snapshot and draws the extrema envelope

**Files:**
- Modify: `ui/src/sim/history.ts` (full rewrite below)
- Modify: `ui/src/components/StripChart.tsx` (`Series.envelope`, auto-scale, band)
- Modify: `ui/src/components/TrendPanel.tsx`

**Interfaces:**
- Consumes: `Snapshot.extrema` (Task 4).
- Produces: `TrendSample.powerPctMax` (thermal, from `thermalMwMax`), `.pressureMpaMax`, `.levelNrCmMin`, `.levelNrCmMax`; `Series.envelope?: { lo: (s: TrendSample) => number; hi: (s: TrendSample) => number }`.

- [ ] **Step 1: Rewrite `ui/src/sim/history.ts`**

```ts
// Rolling trend buffer sampled from snapshots (sim time based).
// Every emitted snapshot is kept (10 Hz x 600 s = 6000 samples) and each sample
// carries the engine's per-step extrema, so no excursion between snapshots is
// lost on the charts.
import { useRef } from "react";
import type { Snapshot } from "./types";

const RATED_MWT = 870;

export interface TrendSample {
  t: number;
  /** Thermal power, % of rated. */
  powerPct: number;
  /** Thermal power peak over the physics steps behind this sample, % of rated. */
  powerPctMax: number;
  pressureMpa: number;
  pressureMpaMax: number;
  levelNrCm: number;
  levelNrCmMin: number;
  levelNrCmMax: number;
  coreFlowKgs: number;
  steamFlowKgs: number;
  feedFlowKgs: number;
  grossMwe: number;
}

export const TREND_WINDOW_S = 600;

export function sampleOf(s: Snapshot): TrendSample {
  const ex = s.extrema;
  const powerPct = (s.neutronics.thermalPowerMw / RATED_MWT) * 100;
  return {
    t: s.timeS,
    powerPct,
    powerPctMax: Math.max((ex.thermalMwMax / RATED_MWT) * 100, powerPct),
    pressureMpa: s.vessel.pressureMpa,
    pressureMpaMax: Math.max(ex.pressureMpaMax, s.vessel.pressureMpa),
    levelNrCm: s.vessel.levelNrCm,
    levelNrCmMin: Math.min(ex.levelNrCmMin, s.vessel.levelNrCm),
    levelNrCmMax: Math.max(ex.levelNrCmMax, s.vessel.levelNrCm),
    coreFlowKgs: s.vessel.coreFlowKgs,
    steamFlowKgs: s.vessel.steamFlowKgs,
    feedFlowKgs: s.vessel.feedFlowKgs,
    grossMwe: s.turbine.grossMwe,
  };
}

/**
 * Returns the (mutable, append-only) sample array. Every snapshot with a new sim
 * time is appended; a re-render with the same snapshot adds nothing; a backwards
 * jump in sim time (scenario load) clears the buffer.
 */
export function useTrendHistory(s: Snapshot | null): TrendSample[] {
  const buf = useRef<TrendSample[]>([]);
  if (s) {
    const b = buf.current;
    const last = b[b.length - 1];
    if (last && s.timeS < last.t) b.length = 0;
    const tail = b[b.length - 1];
    if (!tail || s.timeS > tail.t) {
      b.push(sampleOf(s));
      const cutoff = s.timeS - TREND_WINDOW_S;
      let drop = 0;
      while (drop < b.length && b[drop].t < cutoff) drop++;
      if (drop > 0) b.splice(0, drop);
    }
  }
  return buf.current;
}
```

- [ ] **Step 2: Add the envelope to `StripChart.tsx`**

Change the `Series` interface:

```ts
export interface Series {
  name: string;
  color: string;
  get: (s: TrendSample) => number;
  /** Optional min/max accessors drawn as a translucent band behind the line. */
  envelope?: { lo: (s: TrendSample) => number; hi: (s: TrendSample) => number };
}
```

In the auto-scale loop, replace

```ts
      for (const smp of samples)
        for (const se of series) {
          const v = se.get(smp);
          if (!Number.isFinite(v)) continue;
          if (props.yMin === undefined) lo = Math.min(lo, v);
          if (props.yMax === undefined) hi = Math.max(hi, v);
        }
```

with

```ts
      for (const smp of samples)
        for (const se of series) {
          const vals = se.envelope ? [se.get(smp), se.envelope.lo(smp), se.envelope.hi(smp)] : [se.get(smp)];
          for (const v of vals) {
            if (!Number.isFinite(v)) continue;
            if (props.yMin === undefined) lo = Math.min(lo, v);
            if (props.yMax === undefined) hi = Math.max(hi, v);
          }
        }
```

In the `// traces` section, insert the band drawing immediately after `ctx.clip();` and before `for (const se of series) {`:

```ts
    // Extrema envelope: everything the physics did between two snapshots.
    for (const se of series) {
      if (!se.envelope) continue;
      ctx.beginPath();
      let started = false;
      for (const smp of samples) {
        const v = se.envelope.hi(smp);
        if (!Number.isFinite(v)) continue;
        const x = X(smp.t);
        const y = Y(v);
        if (!started) {
          ctx.moveTo(x, y);
          started = true;
        } else ctx.lineTo(x, y);
      }
      for (let i = samples.length - 1; i >= 0; i--) {
        const v = se.envelope.lo(samples[i]);
        if (!Number.isFinite(v)) continue;
        ctx.lineTo(X(samples[i].t), Y(v));
      }
      if (started) {
        ctx.closePath();
        ctx.globalAlpha = 0.22;
        ctx.fillStyle = se.color;
        ctx.fill();
        ctx.globalAlpha = 1;
      }
    }
```

- [ ] **Step 3: Use the envelope on thermal power, pressure and level in `TrendPanel.tsx`**

Replace the three charts:

```tsx
        <StripChart
          {...common}
          title="THERMAL POWER"
          unit="%"
          series={[
            {
              name: "P",
              color: C.green,
              get: (s) => s.powerPct,
              envelope: { lo: (s) => s.powerPct, hi: (s) => s.powerPctMax },
            },
          ]}
        />
        <StripChart
          {...common}
          title="RX PRESSURE"
          unit="MPa"
          digits={2}
          series={[
            {
              name: "P",
              color: C.amber,
              get: (s) => s.pressureMpa,
              envelope: { lo: (s) => s.pressureMpa, hi: (s) => s.pressureMpaMax },
            },
          ]}
        />
        <StripChart
          {...common}
          title="LEVEL NR"
          unit="cm"
          digits={0}
          series={[
            {
              name: "L",
              color: C.cyan,
              get: (s) => s.levelNrCm,
              envelope: { lo: (s) => s.levelNrCmMin, hi: (s) => s.levelNrCmMax },
            },
          ]}
        />
```

- [ ] **Step 4: Typecheck and build**

Run: `npm --prefix ui run typecheck && npm --prefix ui run build`
Expected: no errors.

- [ ] **Step 5: Manual verification (no TypeScript test runner, no new deps)**

Run: `npm run dev`
1. Load FULL POWER, set speed 30×, wait 20 s, trip the turbine from the turbine panel.
2. Expected: THERMAL POWER shows a green band above the line through the SRI power cut, RX PRESSURE shows an amber band above the line during the rise after the trip, LEVEL NR shows a cyan band around the shrink/swell. At 1× the bands are thin (10 Hz snapshot ≈ 2 steps); at 30× they are clearly visible (≈60 steps per snapshot).
3. Cross-check the peak against the engine: `cargo run --release -p bwr-core --example trace -- full 120 turbinetrip@5` and confirm the printed `Pmpa` column's maximum is not above the top of the drawn band.

- [ ] **Step 6: Commit**

```bash
git add ui/src/sim/history.ts ui/src/components/StripChart.tsx ui/src/components/TrendPanel.tsx
git commit -m "feat(ui): keep every snapshot in the trend buffer and draw the per-step extrema envelope

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 6: Compiler-checked UI contract generated from Rust, with enum/nullable coverage and a unit-suffix rule

**Files:**
- Create: `crates/bwr-core/tests/contract.rs`
- Create (generated): `ui/src/sim/contract.generated.ts`
- Modify: `crates/bwr-core/src/reactor.rs` (only if `RodMotion::label` does not exist)
- Modify: `.github/workflows/validate.yml`
- Modify: `crates/bwr-core/tests/plant.rs` (replace `json_contract_matches_ui` with `command_json_parses`)

**Interfaces:**
- Consumes: `Sim::new`, `Sim::snapshot`, every `Command` variant, `api::{Alarm, ModeSwitch, ScenarioName, Severity, Snapshot, Unsupported}`, `bop::LevelControlMode`, `vessel::Regime` (P4), `reactor::RodMotion` (P5), `Snapshot.rods.motion: Vec<String>` (P8).
- Produces: `ui/src/sim/contract.generated.ts` exporting `SNAPSHOT_FIXTURES: Snapshot[]`, `COMMAND_FIXTURES: Command[]`, `_exhaustive: Record<keyof Snapshot, true>`; Rust tests `ui_contract_is_generated_and_current`, `generation_is_deterministic`, `snapshot_leaf_fields_carry_unit_suffixes`.

How it works: a fresh object literal assigned to a typed `const` is checked for excess properties recursively, and string-literal unions are checked contextually, so a field added in Rust but missing in `types.ts` fails `tsc`, and a wrong enum spelling fails `tsc`. Synthesized fixtures put every enum value and both states of every nullable field into literals so every union member and every `| null` is exercised. `_exhaustive` closes the other direction at the top level: a key present in `types.ts` but absent from the Rust output is a missing-property error; nested keys present in `types.ts` but absent from Rust are missing-property errors in the fixture literals themselves.

- [ ] **Step 1: Make sure `RodMotion::label` exists**

Run: `grep -n "fn label" crates/bwr-core/src/reactor.rs`
If empty, add to `impl RodMotion` in `reactor.rs` (and make `snapshot()` in `sim.rs` use it for `rods.motion` instead of any inline match):

```rust
    /// Contract string for `Snapshot.rods.motion` (mirrored in ui/src/sim/types.ts).
    pub fn label(&self) -> &'static str {
        match self {
            RodMotion::Idle => "idle",
            RodMotion::Motor { .. } => "motor",
            RodMotion::Scram => "scram",
            RodMotion::Sri => "sri",
            RodMotion::MotorRunIn => "motorRunIn",
        }
    }
```

- [ ] **Step 2: Write the generator and the unit-suffix test**

Create `crates/bwr-core/tests/contract.rs`:

```rust
//! Generates ui/src/sim/contract.generated.ts from the Rust API so that
//! `npm --prefix ui run typecheck` fails when types.ts and api.rs diverge, and
//! enforces the unit-suffix naming rule on every Snapshot leaf field.
//! The TS file is rewritten only when its content changes; CI checks it is committed.

use std::fs;
use std::path::PathBuf;

use bwr_core::api::{Alarm, Command, ModeSwitch, ScenarioName, Severity, Snapshot, Unsupported};
use bwr_core::bop::LevelControlMode;
use bwr_core::reactor::RodMotion;
use bwr_core::vessel::Regime;
use bwr_core::Sim;
use serde_json::Value;

// ------------------------------------------------------------------ commands

/// One example per `Command` variant. The `match` below has no wildcard, so adding a
/// variant to `Command` fails to compile here until an example is added.
fn all_commands() -> Vec<Command> {
    let mut v = vec![
        Command::SetPaused { paused: true },
        Command::SetSpeed { speed: 5.0 },
        Command::ManualScram,
        Command::ResetScram,
        Command::SetModeSwitch { mode: ModeSwitch::Run },
        Command::SelectRod { index: Some(3) },
        Command::SelectRod { index: None },
        Command::RodSetTarget { indices: vec![1, 2], target: 12.0 },
        Command::RodStop,
        Command::SetIrmRange { range: 4 },
        Command::SetHeadVent { open: true },
        Command::SetMsiv { line: 0, open: false },
        Command::SetPressureSetpoint { mpa: 6.95 },
        Command::SetBypass { auto: false, pos: 0.3 },
        Command::SetSpeedSetpoint { rpm: 1800.0 },
        Command::ResetTurbine,
        Command::TripTurbine,
        Command::CloseBreaker,
        Command::OpenBreaker,
        Command::SetLoadLimit { pct: 105.0 },
        Command::SetCwPump { index: 1, on: true },
        Command::SetVacuumPump { on: true },
        Command::SetFeedPump { index: 2, on: false },
        Command::SetCondensatePump { index: 0, on: true },
        Command::SetFcvManual { pos: 0.5 },
        Command::SetLevelSetpoint { cm: -40.0 },
        Command::SetHeaters { in_service: false },
        Command::SetRwcuReject { kgs: 15.0 },
        Command::SetIcTrain { index: 0, open: true },
        Command::AckAlarms,
    ];
    // Every enum value that rides inside a Command: ScenarioName and LevelControlMode.
    for name in all_scenarios() {
        v.push(Command::LoadScenario { name });
    }
    for mode in all_level_modes() {
        v.push(Command::SetLevelControl { mode });
    }
    for c in &v {
        match c {
            Command::SetPaused { .. }
            | Command::SetSpeed { .. }
            | Command::LoadScenario { .. }
            | Command::ManualScram
            | Command::ResetScram
            | Command::SetModeSwitch { .. }
            | Command::SelectRod { .. }
            | Command::RodSetTarget { .. }
            | Command::RodStop
            | Command::SetIrmRange { .. }
            | Command::SetHeadVent { .. }
            | Command::SetMsiv { .. }
            | Command::SetPressureSetpoint { .. }
            | Command::SetBypass { .. }
            | Command::SetSpeedSetpoint { .. }
            | Command::ResetTurbine
            | Command::TripTurbine
            | Command::CloseBreaker
            | Command::OpenBreaker
            | Command::SetLoadLimit { .. }
            | Command::SetCwPump { .. }
            | Command::SetVacuumPump { .. }
            | Command::SetFeedPump { .. }
            | Command::SetCondensatePump { .. }
            | Command::SetLevelControl { .. }
            | Command::SetFcvManual { .. }
            | Command::SetLevelSetpoint { .. }
            | Command::SetHeaters { .. }
            | Command::SetRwcuReject { .. }
            | Command::SetIcTrain { .. }
            | Command::AckAlarms => {}
        }
    }
    v
}

// ------------------------------------------------------------------ enum enumerations
// Each list is guarded by an exhaustive match so a new variant breaks the build here.

fn all_scenarios() -> Vec<ScenarioName> {
    let v = vec![ScenarioName::ColdShutdown, ScenarioName::HotStandby, ScenarioName::FullPower];
    for s in &v {
        match s {
            ScenarioName::ColdShutdown | ScenarioName::HotStandby | ScenarioName::FullPower => {}
        }
    }
    v
}

fn all_modes() -> Vec<ModeSwitch> {
    let v = vec![ModeSwitch::Shutdown, ModeSwitch::Refuel, ModeSwitch::Startup, ModeSwitch::Run];
    for m in &v {
        match m {
            ModeSwitch::Shutdown | ModeSwitch::Refuel | ModeSwitch::Startup | ModeSwitch::Run => {}
        }
    }
    v
}

fn all_regimes() -> Vec<Regime> {
    let v = vec![Regime::Vented, Regime::Saturated, Regime::WaterSolid, Regime::GasBlanketed];
    for r in &v {
        match r {
            Regime::Vented | Regime::Saturated | Regime::WaterSolid | Regime::GasBlanketed => {}
        }
    }
    v
}

fn all_severities() -> Vec<Severity> {
    let v = vec![Severity::Info, Severity::Warning, Severity::Alarm, Severity::Trip];
    for s in &v {
        match s {
            Severity::Info | Severity::Warning | Severity::Alarm | Severity::Trip => {}
        }
    }
    v
}

fn all_level_modes() -> Vec<LevelControlMode> {
    let v = vec![LevelControlMode::Manual, LevelControlMode::SingleElement, LevelControlMode::ThreeElement];
    for m in &v {
        match m {
            LevelControlMode::Manual | LevelControlMode::SingleElement | LevelControlMode::ThreeElement => {}
        }
    }
    v
}

fn all_motion_labels() -> Vec<&'static str> {
    let v = vec![RodMotion::Idle, RodMotion::Motor { target: 24.0 }, RodMotion::Scram, RodMotion::Sri, RodMotion::MotorRunIn];
    for m in &v {
        match m {
            RodMotion::Idle | RodMotion::Motor { .. } | RodMotion::Scram | RodMotion::Sri | RodMotion::MotorRunIn => {}
        }
    }
    v.iter().map(|m| m.label()).collect()
}

// ------------------------------------------------------------------ snapshot fixtures

/// Snapshots that put every enum value and both states of every nullable field into
/// a literal, so the TS unions and `| null` members are all exercised.
fn synthesized(base: &Snapshot) -> Vec<Snapshot> {
    let mut out = Vec::new();
    for mode in all_modes() {
        let mut s = base.clone();
        s.mode_switch = mode;
        out.push(s);
    }
    for regime in all_regimes() {
        let mut s = base.clone();
        s.vessel.regime = regime;
        out.push(s);
    }
    for severity in all_severities() {
        let mut s = base.clone();
        s.alarms = vec![Alarm { id: "synth".into(), text: format!("synthesized {severity:?}"), severity, acked: true }];
        out.push(s);
    }
    for mode in all_level_modes() {
        let mut s = base.clone();
        s.feedwater.level_control_mode = mode;
        out.push(s);
    }
    {
        let mut s = base.clone();
        let labels = all_motion_labels();
        s.rods.motion = (0..s.rods.positions.len()).map(|i| labels[i % labels.len()].to_string()).collect();
        out.push(s);
    }
    // Nullable fields, both states each: scram_reason, neutronics.period_s, rods.selected, unsupported.
    {
        let mut some = base.clone();
        some.scram_reason = Some("synthesized scram".into());
        some.neutronics.period_s = Some(60.0);
        some.rods.selected = Some(0);
        some.unsupported = Some(Unsupported { since_s: 1.0, reason: "synthesized".into() });
        out.push(some);
        let mut none = base.clone();
        none.scram_reason = None;
        none.neutronics.period_s = None;
        none.rods.selected = None;
        none.unsupported = None;
        out.push(none);
    }
    out
}

fn scenario_snapshots() -> Vec<Snapshot> {
    all_scenarios().into_iter().map(|n| Sim::new(n).snapshot()).collect()
}

fn generated_ts() -> String {
    let scen = scenario_snapshots();
    let mut fixtures: Vec<Value> = Vec::new();
    for s in &scen {
        fixtures.push(serde_json::to_value(s).expect("snapshot json"));
    }
    for s in synthesized(&scen[2]) {
        fixtures.push(serde_json::to_value(&s).expect("snapshot json"));
    }
    let keys: Vec<String> = fixtures[0].as_object().expect("snapshot is an object").keys().cloned().collect();
    let cmds: Vec<Value> = all_commands().iter().map(|c| serde_json::to_value(c).expect("command json")).collect();

    let mut out = String::new();
    out.push_str("// Generated by crates/bwr-core/tests/contract.rs; do not edit.\n");
    out.push_str("// One snapshot per scenario, synthesized snapshots covering every enum value and both\n");
    out.push_str("// states of every nullable field, and one example per Command variant, all typed against\n");
    out.push_str("// types.ts so `npm --prefix ui run typecheck` fails when api.rs and types.ts diverge.\n");
    out.push_str("import type { Command, Snapshot } from \"./types\";\n\n");
    out.push_str("export const SNAPSHOT_FIXTURES: Snapshot[] = [\n");
    for f in &fixtures {
        out.push_str("  ");
        out.push_str(&serde_json::to_string(f).expect("compact"));
        out.push_str(",\n");
    }
    out.push_str("];\n\nexport const COMMAND_FIXTURES: Command[] = [\n");
    for c in &cmds {
        out.push_str("  ");
        out.push_str(&serde_json::to_string(c).expect("compact"));
        out.push_str(",\n");
    }
    out.push_str("];\n\n// Every top-level key the engine emits (reverse direction): a key declared in types.ts\n");
    out.push_str("// but missing here is a missing-property error. Exported so noUnusedLocals accepts it.\n");
    out.push_str("export const _exhaustive: Record<keyof Snapshot, true> = {\n");
    for k in &keys {
        out.push_str(&format!("  {k}: true,\n"));
    }
    out.push_str("};\n");
    out
}

#[test]
fn ui_contract_is_generated_and_current() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../ui/src/sim/contract.generated.ts");
    let want = generated_ts();
    // serde_json writes NaN/inf as null; a null in a numeric field will fail tsc, which is the point.
    let have = fs::read_to_string(&path).unwrap_or_default();
    if have != want {
        fs::write(&path, &want).expect("write contract.generated.ts");
        eprintln!("updated {} - commit it", path.display());
    }
    assert!(want.contains("export const SNAPSHOT_FIXTURES: Snapshot[]"));
    assert!(want.contains("export const _exhaustive: Record<keyof Snapshot, true>"));
    // 30 fixed examples + 3 LoadScenario + 3 SetLevelControl.
    assert_eq!(all_commands().len(), 36);
    // 3 scenarios + 4 modes + 4 regimes + 4 severities + 3 level modes + 1 motion + 2 nullable.
    assert_eq!(scenario_snapshots().len() + synthesized(&scenario_snapshots()[2]).len(), 21);
}

#[test]
fn generation_is_deterministic() {
    assert_eq!(generated_ts(), generated_ts());
}

// ------------------------------------------------------------------ unit-suffix rule

/// Allowed unit suffixes on Rust (snake_case) leaf field names (master plan r2, P10).
const UNIT_SUFFIXES: &[&str] = &[
    "_s", "_mw", "_mwe", "_we", "_mpa", "_kpa", "_pa", "_c", "_k", "_m", "_cm", "_kgs", "_kg", "_pct", "_rpm", "_hz",
    "_deg", "_frac", "_rel", "_cps", "_j_kg", "_kj_kg",
];

/// Leaf fields that legitimately carry no unit suffix. Every entry says why. Adding an
/// entry requires a comment naming the unit or the reason it is dimensionless.
const DIMENSIONLESS: &[&str] = &[
    "speed",                 // multiplier of real time
    "effective_speed",       // multiplier of real time
    "paused",                // bool
    "mode_switch",           // enum
    "scrammed",              // bool
    "scram_reason",          // text
    "k_eff",                 // ratio
    "irm_range",             // range index 1..10
    "positions",             // rod notch positions 0..48 (array)
    "targets",               // rod notch positions 0..48 (array)
    "layout",                // grid coordinates (array)
    "grid_size",             // count
    "selected",              // index
    "scram_in_progress",     // bool
    "motion",                // enum strings (array)
    "total",                 // reactivity, pcm (documented in types.ts Reactivity)
    "rods",                  // reactivity, pcm
    "doppler",               // reactivity, pcm
    "void",                  // reactivity, pcm
    "moderator",             // reactivity, pcm
    "xenon",                 // reactivity, pcm
    "core_avg_void",         // fraction 0..1
    "chimney_void",          // fraction 0..1
    "exit_quality",          // fraction 0..1
    "head_vent_open",        // bool
    "heatup_rate_cph",       // degC per hour; candidate suffix `_cph` for master r3
    "regime",                // enum
    "level_nr_out_of_range", // bool
    "msiv_pos",              // valve fraction 0..1 (array)
    "msiv_command_open",     // bool (array)
    "tcv_pos",               // valve fraction 0..1
    "bypass_pos",            // valve fraction 0..1
    "bypass_auto",           // bool
    "bypass_manual_pos",     // valve fraction 0..1
    "tsv_pos",               // valve fraction 0..1
    "tripped",               // bool
    "breaker_closed",        // bool
    "cw_pumps",              // bool (array)
    "vacuum_pump_on",        // bool
    "running",               // bool
    "condensate_pumps",      // bool (array)
    "fcv_pos",               // valve fraction 0..1
    "fcv_manual_pos",        // valve fraction 0..1
    "level_control_mode",    // enum
    "heaters_in_service",    // bool
    "valve_pos",             // valve fraction 0..1
    "command_open",          // bool
    "steam_valve_pos",       // valve fraction 0..1
    "return_valve_pos",      // valve fraction 0..1 (array)
    "available",             // bool
    "id",                    // text
    "text",                  // text
    "severity",              // enum
    "acked",                 // bool
    "reason",                // text (Unsupported.reason)
    "fuel_envelope_exceeded",// bool
    "steps",                 // count (Extrema.steps)
];

fn to_snake(camel: &str) -> String {
    let mut s = String::with_capacity(camel.len() + 4);
    for ch in camel.chars() {
        if ch.is_ascii_uppercase() {
            s.push('_');
            s.push(ch.to_ascii_lowercase());
        } else {
            s.push(ch);
        }
    }
    s
}

/// Collect leaf field names (snake_case). Objects recurse; arrays of objects recurse into
/// the first element; arrays of scalars and scalars are leaves; empty arrays are skipped
/// (another fixture covers them).
fn leaves(v: &Value, out: &mut Vec<String>) {
    if let Value::Object(map) = v {
        for (k, val) in map {
            match val {
                Value::Object(_) => leaves(val, out),
                Value::Array(items) => match items.first() {
                    Some(Value::Object(_)) => leaves(&items[0], out),
                    Some(_) => out.push(to_snake(k)),
                    None => {}
                },
                _ => out.push(to_snake(k)),
            }
        }
    }
}

#[test]
fn snapshot_leaf_fields_carry_unit_suffixes() {
    let scen = scenario_snapshots();
    let mut all: Vec<String> = Vec::new();
    for s in scen.iter().chain(synthesized(&scen[2]).iter()) {
        leaves(&serde_json::to_value(s).unwrap(), &mut all);
    }
    all.sort();
    all.dedup();
    let mut bad = Vec::new();
    for leaf in &all {
        // `_max` / `_min` are modifiers (Extrema fields); the unit precedes them.
        let base = leaf.strip_suffix("_max").or_else(|| leaf.strip_suffix("_min")).unwrap_or(leaf);
        let suffixed = UNIT_SUFFIXES.iter().any(|suf| base.ends_with(suf));
        if !suffixed && !DIMENSIONLESS.contains(&base) {
            bad.push(leaf.clone());
        }
    }
    assert!(bad.is_empty(), "leaf fields without a unit suffix and not in DIMENSIONLESS: {bad:?}");
    // The allowlist must not rot: every entry still names a real field.
    let stale: Vec<&&str> = DIMENSIONLESS.iter().filter(|d| !all.iter().any(|l| l == **d || l.trim_end_matches("_max").trim_end_matches("_min") == **d)).collect();
    assert!(stale.is_empty(), "DIMENSIONLESS entries that no longer exist: {stale:?}");
}
```

- [ ] **Step 3: Run it and inspect the generated file**

Run: `cargo test --release -p bwr-core --test contract`
Expected: `test result: ok. 3 passed`, stderr line `updated .../ui/src/sim/contract.generated.ts - commit it`. If `snapshot_leaf_fields_carry_unit_suffixes` fails, the message lists the offending fields: either a P3–P8 field is misnamed (fix it in its owning package's file and `types.ts`) or it is a genuine dimensionless field (add it to `DIMENSIONLESS` with a reason comment). Do not weaken `UNIT_SUFFIXES`.

Run: `head -8 ui/src/sim/contract.generated.ts && grep -c '"timeS"' ui/src/sim/contract.generated.ts`
Expected: the header comment, the import line, and `21`.

- [ ] **Step 4: Prove the check bites in both directions and on enums**

1. Add `bogus: number;` to `interface Snapshot` in `ui/src/sim/types.ts`; run `npm --prefix ui run typecheck`. Expected: `Property 'bogus' is missing` errors on the fixtures and on `_exhaustive`. Revert.
2. Rename `effectiveSpeed` to `effectiveSpeedX` in `types.ts`; typecheck. Expected: `Object literal may only specify known properties, and 'effectiveSpeed' does not exist in type 'Snapshot'`. Revert.
3. Change `export type Regime = ...` in `types.ts` to drop `"gasBlanketed"`; typecheck. Expected: `Type '"gasBlanketed"' is not assignable to type 'Regime'` on the synthesized fixture. Revert.
4. Change `unsupported: ... | null` to non-nullable; typecheck. Expected: `Type 'null' is not assignable`. Revert.

Run: `npm --prefix ui run typecheck`
Expected: clean.

- [ ] **Step 5: Replace the hand-written key check in `tests/plant.rs`**

In `crates/bwr-core/tests/plant.rs`, delete `json_contract_matches_ui` and add:

```rust
#[test]
fn command_json_parses() {
    let cmds = [
        r#"{"type":"rodSetTarget","indices":[1,2],"target":12}"#,
        r#"{"type":"setHeaters","inService":false}"#,
        r#"{"type":"loadScenario","name":"fullPower"}"#,
        r#"{"type":"setLevelControl","mode":"threeElement"}"#,
        r#"{"type":"selectRod","index":null}"#,
        r#"{"type":"manualScram"}"#,
        r#"{"type":"setModeSwitch","mode":"run"}"#,
    ];
    for c in cmds {
        serde_json::from_str::<Command>(c).unwrap_or_else(|e| panic!("{c}: {e}"));
    }
}
```

- [ ] **Step 6: CI freshness check**

In `.github/workflows/validate.yml`, in the job that runs `cargo test --release -p bwr-core`, add immediately after that step:

```yaml
      - name: UI contract generated file is committed and current
        run: git diff --exit-code -- ui/src/sim/contract.generated.ts
```

- [ ] **Step 7: Run everything and commit**

Run: `cargo test --release -p bwr-core && npm --prefix ui run typecheck`
Expected: all tests pass; typecheck clean.

```bash
git add crates/bwr-core/tests/contract.rs crates/bwr-core/tests/plant.rs crates/bwr-core/src/reactor.rs crates/bwr-core/src/sim.rs ui/src/sim/contract.generated.ts .github/workflows/validate.yml
git commit -m "test(contract): generate ui/src/sim/contract.generated.ts with enum and nullable coverage, enforce unit suffixes, check freshness in CI

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
cargo test --release -p bwr-core --test contract && git diff --exit-code -- ui/src/sim/contract.generated.ts
```

Expected: the final diff check exits 0 (regeneration is idempotent).

---

### Task 7: Browser mock is unmistakably a demo; recorded-snapshot replay

**Files:**
- Modify: `ui/src/sim/mock.ts` (header, replay mode)
- Modify: `ui/src/components/TopBar.tsx` (demo banner), `ui/src/styles.css`
- Create: `crates/bwr-core/examples/record_snapshots.rs`
- Create (generated): `ui/src/sim/recorded/fullPower.json`

**Interfaces:**
- Consumes: `Sim::snapshot_with_extrema` (Task 4), `Command::TripTurbine`.
- Produces: `ui/src/sim/recorded/fullPower.json` (array of `Snapshot`, one per sim-second, 120 entries); `?replay=1` URL switch.

- [ ] **Step 1: Relabel the mock**

Replace the header comment of `ui/src/sim/mock.ts` with:

```ts
// UI DEMO ONLY. In-browser stand-in used when the panels run outside Tauri
// (`npm run ui:dev`). It is NOT the physics engine: the Rust crate `bwr-core` is
// the single authoritative model, and nothing here is validated. The numbers only
// need to be plausible enough that every gauge moves and every button does
// something while developing the UI. Add `?replay=1` to the dev URL to replay
// recorded Rust snapshots (ui/src/sim/recorded/fullPower.json) instead.
```

- [ ] **Step 2: Demo banner in the TopBar**

In `ui/src/components/TopBar.tsx`, inside `<header className="topbar">` as the first child (before `<div className="brand">`):

```tsx
      {props.backendKind === "mock" && (
        <div className="demo-banner" role="status">
          UI DEMO: browser mock, not the physics engine
        </div>
      )}
```

In `ui/src/styles.css`, after `.dropped-chip { ... }`:

```css
.demo-banner {
  position: absolute;
  left: 50%;
  top: 0;
  transform: translateX(-50%);
  background: var(--amber);
  color: #1b1500;
  font-family: var(--mono);
  font-size: 11px;
  letter-spacing: 0.08em;
  padding: 2px 10px;
  border-radius: 0 0 4px 4px;
  z-index: 5;
}
```

If `.topbar` is not already `position: relative`, add `position: relative;` to the `.topbar` rule (around line 190).

- [ ] **Step 3: Record Rust snapshots**

Create `crates/bwr-core/examples/record_snapshots.rs`:

```rust
//! Record one Rust snapshot per simulated second of a full-power turbine trip for
//! the browser UI demo (`npm run ui:dev` with `?replay=1`).
//! Output: ui/src/sim/recorded/fullPower.json
use bwr_core::api::{Command, ScenarioName};
use bwr_core::sim::DT;
use bwr_core::Sim;

fn main() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    let steps_per_s = (1.0 / DT).round() as usize;
    let mut frames = Vec::with_capacity(120);
    for sec in 0..120 {
        if sec == 5 {
            sim.apply(Command::TripTurbine);
        }
        for _ in 0..steps_per_s {
            sim.step(DT);
        }
        frames.push(sim.snapshot_with_extrema());
    }
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../ui/src/sim/recorded/fullPower.json");
    std::fs::create_dir_all(std::path::Path::new(path).parent().unwrap()).unwrap();
    std::fs::write(path, serde_json::to_string(&frames).unwrap()).unwrap();
    println!("wrote {} frames to {path}", frames.len());
}
```

Run: `cargo run --release -p bwr-core --example record_snapshots`
Expected: `wrote 120 frames to .../ui/src/sim/recorded/fullPower.json`.

- [ ] **Step 4: Replay mode in `MockSim`**

In `ui/src/sim/mock.ts` add after the existing imports:

```ts
import recorded from "./recorded/fullPower.json";
```

Add these fields to `class MockSim` after `private phase = 0;`:

```ts
  /** Recorded Rust snapshots, played back one per second when the URL has ?replay=1. */
  private readonly replay: Snapshot[] | null =
    typeof location !== "undefined" && new URLSearchParams(location.search).get("replay") === "1"
      ? (recorded as Snapshot[])
      : null;
  private replayIdx = 0;
  private replayAcc = 0;
```

At the top of `private tick(realDt: number)`, before `const s = this.s;`:

```ts
    if (this.replay) {
      this.replayAcc += realDt;
      while (this.replayAcc >= 1) {
        this.replayAcc -= 1;
        this.replayIdx = (this.replayIdx + 1) % this.replay.length;
      }
      const frame = structuredClone(this.replay[this.replayIdx]);
      frame.alarms = frame.alarms.map((a) => ({ ...a, text: `[RECORDED] ${a.text}` }));
      for (const l of this.listeners) l(frame);
      return;
    }
```

Commands in replay mode are ignored by design (the recording cannot respond); the banner from Step 2 still shows.

- [ ] **Step 5: Typecheck, build, verify**

Run: `npm --prefix ui run typecheck && npm --prefix ui run build`
Expected: clean. The recording is only in the lazily imported mock chunk, never in the desktop path.

Run: `npm run ui:dev`, open the printed URL with `?replay=1`.
Expected: amber "UI DEMO" banner; sim clock advances one second per second from the recording; a turbine trip alarm appears at t ≈ 5 s prefixed `[RECORDED]`; the RX PRESSURE envelope band is visible because the recorded frames carry 20-step extrema.

- [ ] **Step 6: Commit**

```bash
git add ui/src/sim/mock.ts ui/src/components/TopBar.tsx ui/src/styles.css crates/bwr-core/examples/record_snapshots.rs ui/src/sim/recorded/fullPower.json
git commit -m "feat(ui): label the browser mock as a demo and add recorded Rust snapshot replay

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 8: Validation report generator (`--report`) and the fidelity record

**Files:**
- Modify: `crates/bwr-core/examples/validation.rs` (add `--report`)
- Create: `docs/validation/results.md`

**Interfaces:**
- Consumes (P2/P9): `bwr_core::validation::{standard_cases, run_case, compare, Tolerance, Trace, Metrics}`, `Metrics.{peak, min, first_event_time, max_abs_mass_residual_kg, max_abs_energy_residual_j, integrated}`, `Tolerance::{numerical, physical, source_comparison(&str)}`, `Trace::from_csv(&str)`, `bwr_core::sim::DT`, `docs/validation/cases.csv` with header `id,scenario,duration_s,purpose,reference,tolerance_class,role,status` (parsed by header name), `docs/validation/baseline/<id>.csv`, trace schema v2 columns `power_frac_max_int`, `pressure_pa_max_int`, `level_nr_cm_min_int`.
- Produces: `--report` prints two Markdown tables (validation, conservation); `docs/validation/results.md`.

- [ ] **Step 1: Add the report functions**

Add to `crates/bwr-core/examples/validation.rs` (keep P2's existing `main` behaviour for the no-flag and baseline-writing paths; add a `--report` branch that calls `report()`):

```rust
use std::collections::BTreeMap;

use bwr_core::validation::{compare, run_case, standard_cases, Metrics, Tolerance, Trace};

struct CaseRow {
    purpose: String,
    reference: String,
    tolerance_class: String,
    role: String,
    status: String,
    /// The raw CSV line, handed to Tolerance::source_comparison for source-comparison cases.
    raw: String,
}

/// docs/validation/cases.csv parsed by header name (locked header:
/// id,scenario,duration_s,purpose,reference,tolerance_class,role,status).
fn case_register() -> BTreeMap<String, CaseRow> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/validation/cases.csv");
    let text = std::fs::read_to_string(path).expect("docs/validation/cases.csv");
    let mut lines = text.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#'));
    let header: Vec<String> = lines.next().expect("cases.csv header").split(',').map(|c| c.trim().to_string()).collect();
    let idx = |name: &str| header.iter().position(|h| h == name).unwrap_or_else(|| panic!("cases.csv lacks column `{name}`"));
    let (i_id, i_purpose, i_ref, i_tol, i_role, i_status) =
        (idx("id"), idx("purpose"), idx("reference"), idx("tolerance_class"), idx("role"), idx("status"));
    let mut out = BTreeMap::new();
    for line in lines {
        let cols: Vec<&str> = line.split(',').map(str::trim).collect();
        if cols.len() < header.len() {
            continue;
        }
        out.insert(
            cols[i_id].to_string(),
            CaseRow {
                purpose: cols[i_purpose].to_string(),
                reference: cols[i_ref].to_string(),
                tolerance_class: cols[i_tol].to_string(),
                role: cols[i_role].to_string(),
                status: cols[i_status].to_string(),
                raw: line.to_string(),
            },
        );
    }
    out
}

fn tolerance_for(row: &CaseRow) -> Tolerance {
    match row.tolerance_class.as_str() {
        "numerical" => Tolerance::numerical(),
        "physical" => Tolerance::physical(),
        _ => Tolerance::source_comparison(&row.raw),
    }
}

/// Peak from the per-step interval column when present, else the sampled column.
fn peak_of(m: &Metrics, base: &str) -> Option<(f64, f64)> {
    m.peak.get(&format!("{base}_max_int")).or_else(|| m.peak.get(base)).copied()
}

fn min_of(m: &Metrics, base: &str) -> Option<(f64, f64)> {
    m.min.get(&format!("{base}_min_int")).or_else(|| m.min.get(base)).copied()
}

fn fmt_at(v: Option<(f64, f64)>, scale: f64, digits: usize) -> String {
    match v {
        Some((x, t)) => format!("{:.*} @ {:.1} s", digits, x * scale, t),
        None => "n/a".into(),
    }
}

fn integrated(m: &Metrics, key: &str) -> String {
    m.integrated.get(key).map(|v| format!("{v:.3e}")).unwrap_or_else(|| "n/a".into())
}

/// Print the Markdown tables for docs/validation/results.md. Exit 1 if any case fails.
fn report() {
    let register = case_register();
    let mut rows: Vec<(String, CaseRow, Metrics, Trace, String)> = Vec::new();
    let mut failures = 0usize;
    for case in standard_cases() {
        let row = register.remove_or_default(case.id);
        let tol = tolerance_for(&row);
        let trace = run_case(&case, bwr_core::sim::DT);
        let m = trace.metrics();
        let baseline_path = format!("{}/../../docs/validation/baseline/{}.csv", env!("CARGO_MANIFEST_DIR"), case.id);
        let result = match std::fs::read_to_string(&baseline_path) {
            Ok(text) => {
                let reference = Trace::from_csv(&text).expect("baseline csv");
                let violations = compare(&reference.metrics(), &m, &tol);
                if violations.is_empty() {
                    "PASS".to_string()
                } else {
                    failures += 1;
                    format!("FAIL: {}", violations.join("; "))
                }
            }
            Err(_) => {
                failures += 1;
                "NO BASELINE".to_string()
            }
        };
        rows.push((case.id.to_string(), row, m, trace, result));
    }

    println!("### Validation results");
    println!();
    println!("| Case | Role | Status | Purpose | Tolerance class | Reference | Peak pressure MPa | Peak power % | Min level cm | First event | Result |");
    println!("|---|---|---|---|---|---|---|---|---|---|---|");
    for (id, row, m, _, result) in &rows {
        let first_event = m
            .first_event_time
            .iter()
            .min_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(k, t)| format!("{k} @ {t:.1} s"))
            .unwrap_or_else(|| "none".into());
        println!(
            "| `{}` | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            id,
            row.role,
            row.status,
            row.purpose,
            row.tolerance_class,
            row.reference,
            fmt_at(peak_of(m, "pressure_pa"), 1e-6, 3),
            fmt_at(peak_of(m, "power_frac"), 100.0, 1),
            fmt_at(min_of(m, "level_nr_cm"), 1.0, 0),
            first_event,
            result
        );
    }

    println!();
    println!("### Transfer allocation and conservation");
    println!();
    println!("| Case | Max abs mass residual kg | Max abs energy residual J | Feed kg | Steam kg | IC steam kg | IC return kg | RWCU kg | Vent kg | Core J | IC J | Loss J | Inventory-exhausted events |");
    println!("|---|---|---|---|---|---|---|---|---|---|---|---|---|");
    for (id, _, m, trace, _) in &rows {
        let exhausted = trace.events.iter().filter(|(_, t)| t.contains("InventoryExhausted")).count();
        println!(
            "| `{}` | {:.3e} | {:.3e} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            id,
            m.max_abs_mass_residual_kg,
            m.max_abs_energy_residual_j,
            integrated(m, "feed_kg"),
            integrated(m, "steam_kg"),
            integrated(m, "ic_steam_kg"),
            integrated(m, "ic_return_kg"),
            integrated(m, "rwcu_kg"),
            integrated(m, "vent_kg"),
            integrated(m, "core_j"),
            integrated(m, "ic_j"),
            integrated(m, "loss_j"),
            exhausted
        );
    }
    println!();
    println!("{} case(s) outside tolerance or without baseline.", failures);
    if failures > 0 {
        std::process::exit(1);
    }
}

trait RemoveOrDefault {
    fn remove_or_default(&mut self, id: &str) -> CaseRow;
}
impl RemoveOrDefault for BTreeMap<String, CaseRow> {
    fn remove_or_default(&mut self, id: &str) -> CaseRow {
        self.remove(id).unwrap_or_else(|| CaseRow {
            purpose: "(not in cases.csv)".into(),
            reference: "none".into(),
            tolerance_class: "numerical".into(),
            role: "unregistered".into(),
            status: "unregistered".into(),
            raw: String::new(),
        })
    }
}
```

In `main`, add before the existing argument handling:

```rust
    if std::env::args().any(|a| a == "--report") {
        report();
        return;
    }
```

- [ ] **Step 2: Run it**

Run: `cargo run --release -p bwr-core --example validation -- --report`
Expected: both tables, one row per standard case, every validation row `PASS`, no row with role `unregistered`, final line `0 case(s) outside tolerance or without baseline.` If any row is `FAIL`, `NO BASELINE` or `unregistered`, that is a real finding from Packages 2–9: do not edit baselines or `cases.csv` to make it pass; record it under "Open findings" in `results.md` and stop to report it.

- [ ] **Step 3: Write `docs/validation/results.md`**

Create the file with this structure, filling every table from the commands shown (paste the `--report` output under sections 4 and 4.1; copy supported-range rows from the four model docs; count assumption statuses with the one-liner):

```markdown
# BWRX-300 Simulator Fidelity Record

Generated for commit `<git rev-parse --short HEAD>` on 2026-MM-DD. Regenerate the
tables with the commands shown under each heading whenever the engine changes.

This is a game-grade engineering model built from public information. It makes no
claim of exact design reproduction, regulatory approval, or validation against
unavailable plant data. Every number below is either traced to a public source,
derived from physical law under stated assumptions, calibrated to a declared
reference point, or marked as assumed.

## 1. Supported operating envelope per subsystem

Copied from the "Supported envelope" section of each model document; those documents are authoritative.

| Subsystem | Document | Pressure | Temperature / power | Inventory / level | Known limits |
|---|---|---|---|---|---|
| Vessel and natural circulation | `docs/model/vessel.md` | ... | ... | ... | ... |
| Core, fuel, rods | `docs/model/core.md` | ... | ... | ... | ... |
| Steam, turbine, condenser, feedwater | `docs/model/power-conversion.md` | ... | ... | ... | ... |
| Isolation condensers | `docs/model/isolation-condensers.md` | ... | ... | ... | ... |
| Protection and instruments | `docs/model/protection.csv` | (cause-and-effect matrix) | | | |

Outside these envelopes the engine raises a `MODEL ENVELOPE EXCEEDED` alarm and sets
`Snapshot.unsupported`; the run continues but its results are not supported.

## 2. Source revisions

Command: `cut -d, -f1-5 docs/model/sources.csv` (document id, title, revision, date, jurisdiction).

| Document ID | Title | Revision | Date | Jurisdiction |
|---|---|---|---|---|
| ... | ... | ... | ... | ... |

## 3. Outstanding assumptions

Command:
`python3 -c "import csv,collections;print(collections.Counter(r['status'] for r in csv.DictReader(open('docs/model/parameters.csv'))))"`

| Status | Count |
|---|---|
| confirmed | N |
| derived | N |
| calibrated | N |
| esbwr-estimate | N |
| assumed | N |
| unavailable | N |

The full list with sensitivity intervals is `docs/model/parameters.csv`; the numbered
assumptions are `docs/model/assumptions.md`.

## 4. Validation results

Command: `cargo run --release -p bwr-core --example validation -- --report`
Method and tolerance classes: `docs/validation/method.md`. Baselines under
`docs/validation/baseline/` are regression evidence, not physical truth. Peak and
minimum columns come from the per-physics-step interval columns of the trace
(`*_max_int`, `*_min_int`), so sampled rows cannot hide an excursion.

(paste the "Validation results" table here)

### 4.1 Transfer allocation and conservation

Every outflow the vessel cannot supply is bounded at the transfer-allocation point
(`Vessel::step`, master step order items 8–9) and every downstream component consumes
the applied flow, so the ledger residuals below close to the numerical tolerance in
`docs/validation/method.md`. The "Inventory-exhausted events" column counts the steps
in which the allocation ratio was below 1; it is zero for every case inside the
envelope. Package 3's coupled low-inventory regression (`tests/conservation.rs`) shows
the condenser hotwell gaining exactly what the vessel released, and Package 9's
scenarios keep the same closure through faults and paired failures.

(paste the "Transfer allocation and conservation" table here)

Numerical residual tolerance (from `method.md`): mass `<tol> kg`, energy `<tol> J`.
Largest observed: mass `<value> kg` in `<case>`, energy `<value> J` in `<case>`.

### 4.2 Calibration vs. validation split

From `docs/validation/cases.csv` column `role`: list the calibration cases (parameters
tuned on them), the validation cases (held out), and the plausibility cases (no public
trace; conservation, analytical and ordering evidence only).

## 5. Paired-failure exploration (Package 9)

Copy the table from `crates/bwr-core/tests/scenarios.rs` doc comments / `docs/validation/cases.csv`
rows with purpose `paired-failure`: case id, failures injected, time at which the model
left its envelope (or "stayed inside"), and the last supported observation.

## 6. Runtime guarantees (Package 10)

- Commands apply only between physics steps (`tests/runtime.rs::commands_apply_at_step_boundaries`).
- No simulation time is discarded silently, in the engine or the shell: backlog up to
  60 s, beyond that counted in `droppedSimTimeS` and shown in the top bar
  (`advance_never_loses_time_silently`, `shell_stall_is_accounted_not_clamped`).
- Every emitted snapshot carries per-step extrema seeded from live values
  (`extrema_are_seeded_from_current_values_never_zeros`, `extrema_expose_excursions_between_snapshots`);
  the thermal power, pressure and level trends draw them as an envelope.
- `ui/src/sim/contract.generated.ts` is regenerated by `cargo test`, covers every enum
  value and both states of every nullable field, and is type-checked against `types.ts`;
  every Snapshot leaf field carries a unit suffix or is on the explicit dimensionless
  allowlist (`snapshot_leaf_fields_carry_unit_suffixes`); CI fails if the file is stale.
- The browser mock is labelled "UI DEMO" and is never used inside the desktop app.

## 7. Desktop smoke checklist

Run before each release candidate on the platform being released. Jim verifies
from the pushed build (CI artifacts), so record the commit hash tested.

1. `npm run dev` (or install the CI bundle). The window shows panels within 200 ms
   of launch, not "WAITING FOR SIMULATOR".
2. Load COLD SHUTDOWN, HOT STANDBY, FULL POWER in turn. Each load resets SIM TIME
   to 0, clears alarms, and shows no DROPPED chip.
3. At FULL POWER set 30×: `EFF` reads within 10 % of 30 and is not amber on a
   laptop-class machine. Note the value.
4. Put the laptop to sleep for 2 minutes at 30× and wake it: a DROPPED chip appears
   with about 3400 s (2 min × 30 − 200 s run − 60 s backlog) and the log shows one
   "Simulation overloaded" line.
5. Trip the turbine by hand. Confirm: SRI event, bypass opens, THERMAL POWER shows the
   green band through the power cut, RX PRESSURE shows the amber envelope band above
   the line during the rise, LEVEL NR shows the cyan band through the shrink.
6. Compare the chart's pressure peak with
   `cargo run --release -p bwr-core --example trace -- full 120 turbinetrip@5`.
7. Reload the UI with the dev server running (Vite HMR): panels repopulate at once
   and the trend buffer restarts without a stale sample.

## 8. Limitations

- Not a containment, LOCA, electrical distribution, chemical shutdown, spent fuel,
  or severe accident model. Interfaces to such systems are explicit assumptions
  (`docs/model/assumptions.md`).
- ESBWR material is used for shared concepts and adapted models only, as recorded
  in `docs/model/esbwr-applicability.csv`; no ESBWR value is treated as confirmed
  for BWRX-300 by ancestry alone.
- Where no public transient trace exists, evidence is conservation, analytical
  component solutions, response ordering, sensitivity and convergence: plausibility
  evidence, not quantitative BWRX-300 transient validation.
- No claim of exact design reproduction, regulatory approval, or validation against
  unavailable plant data is made.

## Open findings

(Any `FAIL` / `NO BASELINE` / `unregistered` rows from the report, with the owning package.)
```

- [ ] **Step 4: Fill every table**

Run the three commands named in the file and paste their output. For Section 1 copy the envelope rows verbatim from the four model documents. For Section 4.1 fill the tolerance and largest-observed line from `method.md` and the conservation table. For Section 5 copy the Package 9 table. Replace `<git rev-parse --short HEAD>` and the date. The file must contain no `...`, `<tol>`, `<value>` or `<case>` placeholders when committed.

Run: `grep -nE '\.\.\.|<tol>|<value>|<case>|2026-MM-DD' docs/validation/results.md`
Expected: no output.

- [ ] **Step 5: Commit**

```bash
git add crates/bwr-core/examples/validation.rs docs/validation/results.md
git commit -m "docs(validation): publish the fidelity record with validation and conservation tables from --report

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 9: README matches the executable

**Files:**
- Modify: `README.md` (Status table, Project layout, Data sources)

**Interfaces:**
- Consumes: the final state of Packages 3–9 as reflected in `docs/validation/results.md`.

- [ ] **Step 1: Rewrite the Status table**

Replace the `## Status` section's table with rows that describe what the engine now does. Use these rows, editing only where a package changed the specifics (check the model docs):

```markdown
| Area | What is simulated |
|---|---|
| Neutronics | Point kinetics with 6 delayed neutron groups and adaptive implicit substeps, neutron source, period meter, SRM / IRM (10 ranges) / APRM. Prompt-supercritical states are reported as outside the model envelope, not simulated. |
| Reactivity | 57 bottom-entry rods with radial and axial worth, Doppler on fuel temperature, void, moderator temperature from the core inlet and boiling lengths, xenon-135 / iodine-135; shutdown-margin check with the strongest rod out |
| Decay heat | Exponential-group fit to the public reference table in `docs/model/decay-heat-reference.csv`, with finite irradiation history |
| Fuel | Two-node fuel and cladding temperatures; degraded cooling when the core uncovers; a FUEL THERMAL MODEL ENVELOPE EXCEEDED flag instead of a damage prediction (no peak-cladding or CPR model) |
| Vessel | Mass and energy balance with IAPWS-IF97 saturation tables plus liquid compressibility; vented, saturated, water-solid and gas-blanketed regimes; collapsed and mixture levels kept separate; every outflow bounded by inventory and the bound propagated to every consumer |
| Natural circulation | Core flow from the downcomer / core + chimney density difference with drift-flux voids and hydrostatic head that follows the actual downcomer level |
| Steam | Two main steam lines with individual MSIV flows, a lumped line/header volume, turbine stop valve, turbine control valves, bypass to the condenser, head vent |
| Turbine-generator | EHC pressure regulator, speed governor, shaft dynamics, synchroscope, generator breaker, overspeed, extraction steam accounted in the shaft balance |
| Condenser | Circulating water pumps, vacuum pump, air in-leakage, hotwell with explicit makeup / reject to a declared condensate storage inventory |
| Feedwater | 2 condensate + 3 feed pumps with pump curves and coastdown, equal-percentage control valve, manual / 1-element / 3-element level control with bumpless transfer, extraction heating, RWCU reject |
| Isolation condensers | 3 trains with steam supply and parallel condensate return valves, tube inventory, pool heat-up and boil-off, unavailability and noncondensable-gas sensitivity |
| Protection | Mode switch, scram, rod blocks, MSIV isolation, IC auto-start, Select Rod Insert, L8 trips, driven by lagged, range-limited sensor readings; the full cause-and-effect matrix is `docs/model/protection.csv` |
| Runtime | Fixed 50 ms physics step, honest time accounting in engine and shell with an effective-speed display, per-step extrema on the trends, generated and type-checked UI contract |
```

- [ ] **Step 2: Fix the prose that no longer matches**

- Keep the scenarios sentence under Status.
- In "Playing", keep the hints; remove nothing unless a model doc says the behaviour changed (check `docs/model/power-conversion.md` for the feedwater-heating hint).
- In "Project layout", change the `tests/plant.rs` sentence to: "`tests/` holds per-subsystem transient tests, `tests/runtime.rs` for time accounting and command boundaries, and `tests/contract.rs`, which generates `ui/src/sim/contract.generated.ts` so the UI types cannot drift from the engine and enforces unit suffixes on every field name."
- Change "`ui/src/sim/types.ts` is the contract with the engine" to "`ui/src/sim/types.ts` mirrors the engine's `api.rs` and is checked by the generated `contract.generated.ts`".
- Add after the `tools` bullet: "`docs/model` is the evidence register (sources, parameters with status, assumptions, ESBWR applicability) and one document per subsystem; `docs/validation` is the validation method, case register, regression baselines and the published fidelity record `results.md`."

- [ ] **Step 3: Replace "Data sources"**

Replace the whole `## Data sources` section with:

```markdown
## Model evidence and validation

Every parameter in the engine has a row in [`docs/model/parameters.csv`](docs/model/parameters.csv) with a status of
confirmed, derived, calibrated, esbwr-estimate, assumed or unavailable, and every public document used is listed with its
revision in [`docs/model/sources.csv`](docs/model/sources.csv). Borrowed ESBWR concepts and their adaptation limits are in
[`docs/model/esbwr-applicability.csv`](docs/model/esbwr-applicability.csv).

What was validated, against what, and to what tolerance is published in
[`docs/validation/results.md`](docs/validation/results.md); the method is [`docs/validation/method.md`](docs/validation/method.md).
Regenerate the results tables with `cargo run --release -p bwr-core --example validation -- --report`.

This is a game, not an engineering tool. It makes no claim of exact design reproduction, regulatory approval, or validation
against unavailable plant data. It is not affiliated with GE Vernova, Hitachi or the makers of Nucleares.
```

(The old "Confirmed figures ... Not yet confirmed ..." paragraph is deleted; `parameters.csv` is now the single source of that information.)

- [ ] **Step 4: Check and commit**

Run: `grep -n "1200\|9-group\|CONFIRMED\|ASSUMED" README.md`
Expected: no output (if `params.rs` still uses CONFIRMED/ASSUMED markers after Package 1, keep that mention in "Project layout" only).

```bash
git add README.md
git commit -m "docs(readme): describe the refined engine and point to the evidence register and fidelity record

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 10: Final verification, design audit, release push

**Files:**
- No new files. Possibly `ui/src/**` edits from the design audit.

- [ ] **Step 1: Engine**

Run: `cargo test --release -p bwr-core`
Expected: all tests pass, including `tests/runtime.rs` (11 tests) and `tests/contract.rs` (3 tests).

Run: `cargo test --release -p bwr-core -- --ignored`
Expected: the long-duration ICS endurance tests from Package 7 pass (72 h and 7 day cases).

Run: `cargo run --release -p bwr-core --example validation -- --report`
Expected: every row `PASS`, `0 case(s) outside tolerance or without baseline.`, identical to the tables pasted in `docs/validation/results.md` (re-paste if the hash/date line differs).

- [ ] **Step 2: Contract freshness**

Run: `git diff --exit-code -- ui/src/sim/contract.generated.ts`
Expected: exit 0.

- [ ] **Step 3: UI**

Run: `npm --prefix ui ci && npm --prefix ui run typecheck && npm --prefix ui run build`
Expected: clean install, no type errors, `✓ built`.

- [ ] **Step 4: Desktop**

Run: `npm ci && npx tauri build`
Expected: a bundle under `target/release/bundle/` for this platform. Then walk the Section 7 desktop smoke checklist in `docs/validation/results.md` and record the commit hash and `EFF` reading in that section.

- [ ] **Step 5: Front-end design audit (CLAUDE.md mandatory)**

Invoke the `fda:quick` skill on `ui/src` (TopBar effective-speed chip, dropped chip, demo banner, trend envelopes). Apply its severity 3–4 fixes and practical severity 2 fixes; re-run `npm --prefix ui run typecheck && npm --prefix ui run build`.

- [ ] **Step 6: Commit and push**

```bash
git add -A
git commit -m "chore(release): Package 10 verification, design audit fixes, fidelity record finalised

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
git push origin main
```

Expected: CI `physics`, `ui`, `validate`, and `desktop` jobs green on GitHub; the desktop artifacts are what Jim tests.

---

## Self-review against the spec (Package 10 bullets) and the master checklist

| Spec bullet | Task |
|---|---|
| Keep Rust authoritative; make browser mock unmistakably a UI demonstration, using recorded Rust snapshots when useful. A browser physics port is a separate decision. | Task 7 (banner, header, `?replay=1`; no port) |
| Generate API types from Rust or add a complete contract check covering commands, field names, units, enums, and nullability. | Task 6 (fixtures typed against `Snapshot`/`Command`; synthesized enum and nullable fixtures; `_exhaustive` reverse key check; unit-suffix test with explicit dimensionless allowlist; CI freshness) |
| Verify command application at deterministic physics boundaries, scenario reset, pause, reconnect, and initial snapshot delivery. | Task 3 |
| Preserve simulation-time accounting under acceleration and overload. Report effective speed rather than silently discarding accumulated time. | Tasks 1 (engine, defect 9) and 2 (shell, defect 11) |
| Verify that display sampling and trend decimation do not hide excursions used to assess safety behavior. | Tasks 4, 5 (defect 10) |
| Run full engine validation, UI typecheck/build, and desktop smoke tests using the Rust backend. | Task 10 (+ checklist in Task 8) |
| Publish supported ranges, source revisions, outstanding assumptions, validation results, and limitations for every subsystem. | Tasks 8, 9 (results.md includes the transfer-allocation and conservation subsection) |

Master checklist:

- Every spec bullet maps to a task: see table.
- No placeholders: the only `...`/`<...>` tokens are inside the `results.md` template and Task 8 Step 4 greps them out before commit.
- Names match master r2: `MAX_STEPS_PER_ADVANCE`, `MAX_BACKLOG_S`, `effective_speed`, `dropped_sim_time_s`, `advance_with`, `snapshot_with_extrema` (sole consumer), `Extrema` with all nine fields and `seed(&Extrema)`, `finalize_init`, `faults: Vec<Fault>`, `Trace::from_csv(&str)`, `Tolerance::source_comparison(&str)`, `Metrics.integrated`, cases.csv header `id,scenario,duration_s,purpose,reference,tolerance_class,role,status`, `RodMotion::{Idle, Motor, Scram, Sri, MotorRunIn}` labels `"idle" | "motor" | "scram" | "sri" | "motorRunIn"`, `Regime::{Vented, Saturated, WaterSolid, GasBlanketed}`. The earlier `take_extrema` and `level_cm_sensed` names are gone.
- Derives rule: `Extrema` derives `Clone, Copy, Debug, Default, Serialize, Deserialize` with camelCase.
- Every new constant has a `parameters.csv` row: `MAX_STEPS_PER_ADVANCE`, `MAX_BACKLOG_S` (Task 1), `SHELL_WALL_DT_CLAMP_S` (Task 2).
- Every Snapshot change has `types.ts` and `mock.ts` steps (Tasks 1, 4) and the contract test guards them (Task 6).
- Every defect fix has a failing test or demonstration first (Task 1 Step 1, Task 2 Step 1, Task 4 Step 1).
- Applied transfers: P10 adds no transfer consumers; the conservation table reads `Ledger.integrated`, which records applied flows.
- Type consistency: `Extrema` field names are identical in `api.rs`, master, `types.ts` (camelCase: `fuelTCMax`, `cladTCMax`), `mock.ts`, `history.ts`, `tests/runtime.rs`; `effective_speed`/`dropped_sim_time_s` ↔ `effectiveSpeed`/`droppedSimTimeS` in `sim.rs`, `api.rs`, `types.ts`, `mock.ts`, `TopBar.tsx`; `snapshot_with_extrema` is used by `lib.rs`, `record_snapshots.rs`, and the runtime tests; `snapshot()` never consumes the window.
