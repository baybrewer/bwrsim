# Package 10: Unify Runtime Behavior and Publish the Fidelity Record — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. The master plan `docs/superpowers/plans/2026-10-20-refinement-master.md` owns every shared name and signature; if this file and the master disagree, the master wins.

**Goal:** Make the supported application path (Tauri + Rust engine) show exactly the engine's behaviour with honest time accounting, step-level extrema, a compiler-checked UI contract, an unmistakable browser-mock label, and a published fidelity record that matches the executable and its tests.

**Architecture:** `Sim::advance` keeps a bounded backlog instead of zeroing it, reports effective speed and dropped time, and tracks per-step extrema that ride along with each snapshot. A Rust integration test generates `ui/src/sim/contract.generated.ts` so `tsc` fails when `api.rs` and `types.ts` diverge. The trend buffer keeps every snapshot and draws the extrema envelope. `docs/validation/results.md` is generated from the validation runner's `--report` output plus the model docs from Packages 4 through 9.

**Tech Stack:** Rust 2021, serde/serde_json (dev-dependency, available to tests and examples), Tauri 2.12, TypeScript 5.6 with `strict`, `noUnusedLocals`, `resolveJsonModule` (already on), React 18, Vite 5. No new dependencies.

**Spec package:** "Package 10: Unify runtime behavior and publish the fidelity record" in `docs/superpowers/plans/2026-10-20-existing-systems-refinement.md`. **Assumes Packages 1–9 are complete and committed**: `Sim` already has `ledger`, `log`, `faults`, `domain_errors`, `sensors`, `balance`; `Snapshot` already has every field listed in the master "API contract additions summary" for P3–P8; `examples/validation.rs`, `standard_cases()`, `Trace::metrics()`, `compare()`, `Tolerance`, `docs/validation/cases.csv`, `docs/validation/baseline/*.csv`, and `.github/workflows/validate.yml` exist.

## Global Constraints

Inherited verbatim from the master plan; repeated here because the task implementer sees only this file.

- "Keep Rust authoritative." The browser mock is a UI demonstration only.
- "No previous-test failure is concealed by updating expected output."
- "No claim of exact design reproduction, regulatory approval, or validation against unavailable plant data is made."
- No new Rust or npm dependencies without asking Jim first. Hand-roll CSV/JSON/TS emission with `std`, `serde_json`.
- Commands: `cargo test --release -p bwr-core`, `npm --prefix ui ci`, `npm --prefix ui run typecheck`, `npm --prefix ui run build`, `cargo run --release -p bwr-core --example validation`.
- Commit after every task. Commit messages end with `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.
- 2-space indentation in TypeScript; match surrounding Rust style by hand (do not run `cargo fmt` if the component is not installed).
- Jim does not test locally for deployed builds: the final task commits **and pushes**.
- After touching UI layout (Tasks 1, 4, 6), run the `frontend-design-audit` skill (`/fda:quick`) before calling the package done (CLAUDE.md).

**Acceptance gate (verbatim from the spec):** "Users see the same authoritative engine behavior in the supported application path. The fidelity record matches the executable and its tests."

**Baseline defects this package fixes (master list items 9 and 10):**

9. `sim.rs` `advance()`: when `n >= 4000` the accumulator is zeroed and simulation time is silently lost.
10. `history.ts`: 1 s sampling of 10 Hz snapshots, and 10 Hz snapshots of 20 Hz physics, can hide sub-second excursions.

## Names this package introduces beyond the master contract

| Name | Where | Meaning |
|---|---|---|
| `MAX_STEPS_PER_ADVANCE: usize = 4000` | `sim.rs` | hard cap on physics steps per `advance` call (was a literal) |
| `MAX_BACKLOG_S: f64 = 60.0` | `sim.rs` | sim-seconds of requested-but-not-yet-run time kept; excess is counted in `dropped_sim_time_s` |
| `Sim::backlog_s(&self) -> f64` | `sim.rs` | read access to the private accumulator |
| `Sim::advance_with(&mut self, real_dt: f64, cmds: Vec<Command>)` | `sim.rs` | applies commands, then `advance` |
| `Sim::snapshot_with_extrema(&mut self) -> Snapshot` | `sim.rs` | snapshot carrying `take_extrema()`; resets the window |
| `Extrema::seed(...)`, `Extrema::include(...)` | `api.rs` | window reset and per-step update |
| `TrendSample.powerPctMax/pressureMpaMax/levelNrCmMin/levelNrCmMax` | `history.ts` | folded extrema |
| `Series.envelope?: { lo; hi }` | `StripChart.tsx` | translucent band between two sample accessors |
| `examples/record_snapshots.rs` | engine | writes `ui/src/sim/recorded/fullPower.json` |
| `?replay=1` | `mock.ts` | browser mock replays recorded Rust snapshots |
| `--report` | `examples/validation.rs` | prints Markdown tables for `results.md` |
| `.demo-banner`, `.speed-eff`, `.speed-eff.slow`, `.dropped-chip` | `styles.css` | UI classes |

---

## File structure

```
crates/bwr-core/src/api.rs            Extrema (+ seed/include), Snapshot.effective_speed, .dropped_sim_time_s, .extrema
crates/bwr-core/src/sim.rs            MAX_STEPS_PER_ADVANCE, MAX_BACKLOG_S, advance, advance_with, backlog_s,
                                      effective_speed, dropped_sim_time_s, extrema, take_extrema, snapshot_with_extrema
crates/bwr-core/tests/runtime.rs      time accounting, boundaries, pause, reset, extrema tests
crates/bwr-core/tests/contract.rs     generates ui/src/sim/contract.generated.ts
crates/bwr-core/examples/record_snapshots.rs
crates/bwr-core/examples/validation.rs  --report
src-tauri/src/lib.rs                  snapshot_with_extrema, boundary comment
ui/src/sim/types.ts                   mirror of api.rs additions
ui/src/sim/mock.ts                    relabel header, mirror fields, replay mode
ui/src/sim/bridge.ts                  initial snapshot via sim_snapshot
ui/src/sim/history.ts                 no decimation, extrema folded in
ui/src/sim/contract.generated.ts      generated
ui/src/sim/recorded/fullPower.json    generated
ui/src/components/StripChart.tsx      envelope band
ui/src/components/TrendPanel.tsx      envelopes on pressure and level
ui/src/components/TopBar.tsx          effective speed, dropped chip, demo banner
ui/src/styles.css                     new classes
.github/workflows/validate.yml        contract freshness check
docs/validation/results.md            fidelity record
README.md                             Status / Data sources rewrite
```

---

### Task 1: Honest time accounting in `Sim::advance`

**Files:**
- Modify: `crates/bwr-core/src/sim.rs` (struct fields, `base`, `advance`, `apply` for `SetSpeed`/`LoadScenario`, `snapshot`)
- Modify: `crates/bwr-core/src/api.rs` (`Snapshot` fields)
- Modify: `ui/src/sim/types.ts`, `ui/src/sim/mock.ts` (`blankSnapshot`), `ui/src/components/TopBar.tsx`, `ui/src/styles.css`
- Test: `crates/bwr-core/tests/runtime.rs` (new)

**Interfaces:**
- Consumes: `Sim::step(dt)`, `DT`, `Sim.log: Vec<(f64, String)>` (P2), `Sim::event(id, text)`.
- Produces: `pub const MAX_STEPS_PER_ADVANCE: usize`, `pub const MAX_BACKLOG_S: f64`, `Sim.effective_speed: f64`, `Sim.dropped_sim_time_s: f64`, `Sim::backlog_s(&self) -> f64`, `Snapshot.effective_speed`, `Snapshot.dropped_sim_time_s`.

- [ ] **Step 1: Write the failing tests**

Create `crates/bwr-core/tests/runtime.rs`:

```rust
//! Runtime behaviour: time accounting, command boundaries, reset, extrema.

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
    // 3 s of wall at 100x = 300 s requested; one call runs 200 s, keeps 100 s? No: backlog cap is 60 s.
    // Use 2.5 s: 250 s requested -> 200 s run, 50 s backlog, nothing dropped.
    sim.advance(2.5);
    assert!((sim.backlog_s() - 50.0).abs() < 1e-6);
    assert_eq!(sim.dropped_sim_time_s, 0.0);
    // A quiet call drains the backlog.
    sim.advance(0.0);
    assert!(sim.backlog_s() < DT);
    assert!((sim.time - 250.0).abs() < 1e-6);
    assert_eq!(sim.dropped_sim_time_s, 0.0);
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

- [ ] **Step 3: Implement the fields and `advance`**

In `crates/bwr-core/src/sim.rs`, replace `pub const DT: f64 = 0.05;` with:

```rust
/// Fixed physics time step, s.
pub const DT: f64 = 0.05;
/// Hard cap on physics steps per `advance` call so one wall-clock tick never stalls the shell.
pub const MAX_STEPS_PER_ADVANCE: usize = 4000;
/// Requested-but-not-yet-simulated time kept between calls, s. Beyond this the
/// excess is counted in `dropped_sim_time_s` and reported, never silently discarded.
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
    /// `dropped_sim_time_s` and logged once per overload episode.
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

In `apply`, change the two speed-related arms:

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
Expected: all tests pass (the `json_contract_matches_ui` test in `tests/plant.rs` still passes; it checks presence of keys only).

- [ ] **Step 5: Mirror in the UI**

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

- [ ] **Step 6: Typecheck and build the UI**

Run: `npm --prefix ui run typecheck && npm --prefix ui run build`
Expected: no errors; `vite build` reports `✓ built`.

- [ ] **Step 7: Commit**

```bash
git add crates/bwr-core/src/sim.rs crates/bwr-core/src/api.rs crates/bwr-core/tests/runtime.rs ui/src/sim/types.ts ui/src/sim/mock.ts ui/src/components/TopBar.tsx ui/src/styles.css
git commit -m "fix(runtime): keep a bounded backlog in Sim::advance and report effective speed and dropped time

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 2: Command boundaries, pause, scenario reset, and initial snapshot delivery

**Files:**
- Modify: `crates/bwr-core/src/sim.rs` (`full_power` clears runtime state; doc comment on `apply`)
- Modify: `src-tauri/src/lib.rs` (boundary comment)
- Modify: `ui/src/sim/bridge.ts`
- Test: `crates/bwr-core/tests/runtime.rs`

**Interfaces:**
- Consumes: `Sim::advance_with` (Task 1), `Sim.faults` + `Sim::set_fault` (P2), `Sim.ledger` (P2), `Sim.log` (P2), `Sim.domain_errors` (P3), `Fault::BypassUnavailable` (P2/P6), `ROD_FULL_OUT`, `SCRAM_TIME`.
- Produces: guarantee that `apply` only runs between steps; `LoadScenario` resets every runtime accumulator; UI receives a snapshot immediately on subscribe.

- [ ] **Step 1: Write the failing tests**

Append to `crates/bwr-core/tests/runtime.rs`:

```rust
use bwr_core::faults::Fault;
use bwr_core::params::{ROD_FULL_OUT, SCRAM_TIME};

#[test]
fn commands_apply_at_step_boundaries() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    let before = sim.core.rods.pos.clone();
    // One wall-second at 1x with exactly one command: the scram is applied before
    // the single step, so every rod has moved exactly one step of hydraulic travel.
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
    sim.apply(Command::SetSpeed { speed: 100.0 });
    sim.advance(1000.0);
    assert!(sim.dropped_sim_time_s > 0.0);
    assert!(!sim.log.is_empty());
    assert!(sim.ledger.mass_in_kg > 0.0 || sim.ledger.mass_out_kg > 0.0);

    sim.apply(Command::LoadScenario { name: ScenarioName::HotStandby });
    assert_eq!(sim.time, 0.0);
    assert!(sim.faults.is_empty(), "faults are test/instructor state and clear on load");
    assert_eq!(sim.dropped_sim_time_s, 0.0);
    assert_eq!(sim.backlog_s(), 0.0);
    assert!(sim.log.is_empty(), "log: {:?}", sim.log);
    assert!(sim.domain_errors.is_empty());
    assert_eq!(sim.ledger.mass_in_kg, 0.0);
    assert_eq!(sim.ledger.mass_out_kg, 0.0);
    assert_eq!(sim.speed, 100.0);
    assert_eq!(sim.effective_speed, 100.0);
}

#[test]
fn full_power_initialisation_leaves_no_runtime_residue() {
    // full_power() settles for 12 minutes internally; that settling must not leak
    // into the log, ledger or extrema that the user sees at t = 0.
    let sim = Sim::new(ScenarioName::FullPower);
    assert_eq!(sim.time, 0.0);
    assert!(sim.log.is_empty(), "log: {:?}", sim.log);
    assert_eq!(sim.ledger.mass_in_kg, 0.0);
    assert_eq!(sim.backlog_s(), 0.0);
}
```

- [ ] **Step 2: Run to verify which fail**

Run: `cargo test --release -p bwr-core --test runtime`
Expected: `commands_apply_at_step_boundaries` and `paused_advance_accumulates_nothing` pass (behaviour already holds); `load_scenario_resets_runtime_state_but_keeps_speed` and/or `full_power_initialisation_leaves_no_runtime_residue` FAIL if `full_power()` or `Sim::new` leaves `log`/`ledger` populated from the internal settle. If both pass, P2/P3 already clear them: go to Step 4.

- [ ] **Step 3: Clear runtime residue at the end of `full_power()`**

In `crates/bwr-core/src/sim.rs` `full_power()`, replace

```rust
        s.time = 0.0;
        s.acked.clear();
        s.events.clear();
        s
```

with

```rust
        s.time = 0.0;
        s.acked.clear();
        s.events.clear();
        s.log.clear();
        s.ledger = crate::ledger::Ledger::new(s.vessel.mass, s.vessel.energy);
        s.domain_errors.clear();
        s.accumulator = 0.0;
        s
```

Add this doc comment above `pub fn apply(&mut self, cmd: Command)`:

```rust
    /// Apply one operator command. Commands are only ever applied between physics
    /// steps: the desktop shell holds the simulator mutex for the whole `advance`
    /// call, and `advance_with` applies its batch before stepping. A command can
    /// therefore never observe or alter a half-stepped plant.
```

- [ ] **Step 4: Run the tests**

Run: `cargo test --release -p bwr-core --test runtime`
Expected: `test result: ok. 7 passed`.

- [ ] **Step 5: Deliver the initial snapshot immediately in the bridge**

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

In `src-tauri/src/lib.rs`, above `fn sim_command`, add:

```rust
/// Commands lock the same mutex the physics thread holds for a whole `advance`
/// call, so they are applied strictly between physics steps.
```

- [ ] **Step 6: Typecheck**

Run: `npm --prefix ui run typecheck`
Expected: no errors.

- [ ] **Step 7: Commit**

```bash
git add crates/bwr-core/src/sim.rs crates/bwr-core/tests/runtime.rs src-tauri/src/lib.rs ui/src/sim/bridge.ts
git commit -m "fix(runtime): reset runtime state on scenario load and deliver the initial snapshot on subscribe

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 3: Per-step extrema ride along with every snapshot

**Files:**
- Modify: `crates/bwr-core/src/api.rs` (`Extrema`, `Snapshot.extrema`)
- Modify: `crates/bwr-core/src/sim.rs` (`extrema` field, `step`, `take_extrema`, `snapshot_with_extrema`, `snapshot`, `full_power`)
- Modify: `src-tauri/src/lib.rs` (emit `snapshot_with_extrema`)
- Modify: `ui/src/sim/types.ts`, `ui/src/sim/mock.ts`
- Test: `crates/bwr-core/tests/runtime.rs`

**Interfaces:**
- Consumes: `Sim::level_cm()` (sensed narrow-range level after P8), `self.vessel.eos.p`, `self.core.kin.n`.
- Produces: `api::Extrema { power_frac_max, pressure_mpa_max, level_nr_cm_min, level_nr_cm_max, steps }`, `Extrema::seed(power_frac, pressure_mpa, level_nr_cm) -> Extrema`, `Extrema::include(&mut self, power_frac, pressure_mpa, level_nr_cm)`, `Sim.extrema: Extrema`, `Sim::take_extrema(&mut self) -> Extrema`, `Sim::snapshot_with_extrema(&mut self) -> Snapshot`, `Snapshot.extrema`.

- [ ] **Step 1: Write the failing tests**

Append to `crates/bwr-core/tests/runtime.rs`:

```rust
#[test]
fn extrema_track_every_physics_step() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    let _ = sim.take_extrema();
    let mut p_max = f64::MIN;
    let mut l_min = f64::MAX;
    let mut l_max = f64::MIN;
    for _ in 0..20 {
        sim.step(DT);
        p_max = p_max.max(sim.vessel.eos.p / 1e6);
        l_min = l_min.min(sim.level_cm_sensed());
        l_max = l_max.max(sim.level_cm_sensed());
    }
    let ex = sim.take_extrema();
    assert_eq!(ex.steps, 20);
    assert!((ex.pressure_mpa_max - p_max).abs() < 1e-12);
    assert!((ex.level_nr_cm_min - l_min).abs() < 1e-12);
    assert!((ex.level_nr_cm_max - l_max).abs() < 1e-12);
    // The window resets: a second take has no steps and is seeded with the current state.
    let ex2 = sim.take_extrema();
    assert_eq!(ex2.steps, 0);
    assert!((ex2.pressure_mpa_max - sim.vessel.eos.p / 1e6).abs() < 1e-12);
}

#[test]
fn extrema_expose_excursions_between_snapshots() {
    // Snapshots are emitted every 1 s (20 steps). A scram at the start of the
    // window drops power within the window; the snapshot at the end shows low
    // power, but the extrema must still show the pre-scram peak.
    let mut sim = Sim::new(ScenarioName::FullPower);
    let _ = sim.take_extrema();
    sim.apply(Command::ManualScram);
    for _ in 0..20 {
        sim.step(DT);
    }
    let snap = sim.snapshot_with_extrema();
    assert!(snap.neutronics.power_frac < 0.9, "power after 1 s of scram {}", snap.neutronics.power_frac);
    assert!(snap.extrema.power_frac_max > 0.95, "peak {}", snap.extrema.power_frac_max);
    assert!(snap.extrema.power_frac_max >= snap.neutronics.power_frac);
    assert_eq!(snap.extrema.steps, 20);
    // snapshot() alone never resets the window.
    let plain = sim.snapshot();
    assert_eq!(plain.extrema.steps, 0);
}
```

`Sim::level_cm_sensed()` is the public name for the narrow-range sensor reading that P8 made `level_cm()` return. If P8 left `level_cm` private, add in `sim.rs`:

```rust
    /// Narrow-range level as the protection system and displays see it, cm from NWL.
    pub fn level_cm_sensed(&self) -> f64 {
        self.level_cm()
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test --release -p bwr-core --test runtime`
Expected: compile error `no method named take_extrema found for struct Sim`.

- [ ] **Step 3: Implement `Extrema` and the window**

In `crates/bwr-core/src/api.rs`, before `pub struct Snapshot`:

```rust
/// Per-step extrema since the last snapshot was taken, so a 10 Hz display cannot
/// hide a 20 Hz excursion. `steps` is how many physics steps the window covers.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Extrema {
    pub power_frac_max: f64,
    pub pressure_mpa_max: f64,
    pub level_nr_cm_min: f64,
    pub level_nr_cm_max: f64,
    pub steps: u32,
}

impl Extrema {
    /// Start a window at the current state.
    pub fn seed(power_frac: f64, pressure_mpa: f64, level_nr_cm: f64) -> Self {
        Self {
            power_frac_max: power_frac,
            pressure_mpa_max: pressure_mpa,
            level_nr_cm_min: level_nr_cm,
            level_nr_cm_max: level_nr_cm,
            steps: 0,
        }
    }

    /// Fold one physics step into the window.
    pub fn include(&mut self, power_frac: f64, pressure_mpa: f64, level_nr_cm: f64) {
        self.power_frac_max = self.power_frac_max.max(power_frac);
        self.pressure_mpa_max = self.pressure_mpa_max.max(pressure_mpa);
        self.level_nr_cm_min = self.level_nr_cm_min.min(level_nr_cm);
        self.level_nr_cm_max = self.level_nr_cm_max.max(level_nr_cm);
        self.steps += 1;
    }
}
```

In `Snapshot`, after `pub dropped_sim_time_s: f64,` add:

```rust
    /// Extrema over the physics steps since the previous emitted snapshot.
    pub extrema: Extrema,
```

In `sim.rs`: import `Extrema` (`use crate::api::{self, Command, Extrema, ModeSwitch, ScenarioName, Severity, Snapshot};`), add `pub extrema: Extrema,` to `Sim` after `dropped_sim_time_s`, and in `Sim::base` add `extrema: Extrema::default(),` then, at the end of `base` just before `s`:

```rust
        s.extrema = Extrema::seed(s.core.kin.n, s.vessel.eos.p / 1e6, s.level_cm());
```

At the end of `Sim::step`, after `self.protection();`, add:

```rust
        self.extrema.include(self.core.kin.n, self.vessel.eos.p / 1e6, self.level_cm());
```

In `full_power()`, in the block from Task 2 Step 3, add `s.extrema = Extrema::seed(s.core.kin.n, s.vessel.eos.p / 1e6, s.level_cm());` after `s.accumulator = 0.0;`.

Add these methods to `impl Sim` next to `snapshot`:

```rust
    /// Return the extrema window and start a new one at the current state.
    pub fn take_extrema(&mut self) -> Extrema {
        let out = self.extrema;
        self.extrema = Extrema::seed(self.core.kin.n, self.vessel.eos.p / 1e6, self.level_cm());
        out
    }

    /// Snapshot for emission: carries the extrema since the last emitted snapshot
    /// and resets the window. `snapshot()` alone never resets it.
    pub fn snapshot_with_extrema(&mut self) -> Snapshot {
        let extrema = self.take_extrema();
        let mut snap = self.snapshot();
        snap.extrema = extrema;
        snap
    }
```

In `snapshot()`, after `dropped_sim_time_s: ...,` add `extrema: self.extrema,`.

- [ ] **Step 4: Run the tests**

Run: `cargo test --release -p bwr-core --test runtime`
Expected: `test result: ok. 9 passed`.

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
                            // Carries per-step extrema since the last emit and resets that window.
                            Some(s.snapshot_with_extrema())
                        } else {
                            None
                        }
                    };
```

`sim_snapshot` keeps using `snapshot()` so a reconnect never resets the window.

- [ ] **Step 6: Mirror in the UI**

`ui/src/sim/types.ts`, before `export interface Snapshot`:

```ts
/** Per-step extrema since the previous emitted snapshot (`steps` physics steps). */
export interface Extrema {
  powerFracMax: number;
  pressureMpaMax: number;
  levelNrCmMin: number;
  levelNrCmMax: number;
  steps: number;
}
```

and in `Snapshot` after `droppedSimTimeS: number;` add `extrema: Extrema;`.

`ui/src/sim/mock.ts`: import `Extrema` in the type import list; in `blankSnapshot()` add
`extrema: { powerFracMax: 0, pressureMpaMax: 0, levelNrCmMin: 0, levelNrCmMax: 0, steps: 0 },`.
In `private tick(realDt)`, replace the body with one that folds the mock's own sub-steps:

```ts
  private tick(realDt: number) {
    const s = this.s;
    const ex: Extrema = {
      powerFracMax: s.neutronics.powerFrac,
      pressureMpaMax: s.vessel.pressureMpa,
      levelNrCmMin: s.vessel.levelNrCm,
      levelNrCmMax: s.vessel.levelNrCm,
      steps: 0,
    };
    if (!s.paused) {
      let remaining = realDt * s.speed;
      while (remaining > 1e-9) {
        const dt = Math.min(0.05, remaining);
        this.step(dt);
        remaining -= dt;
        ex.powerFracMax = Math.max(ex.powerFracMax, s.neutronics.powerFrac);
        ex.pressureMpaMax = Math.max(ex.pressureMpaMax, s.vessel.pressureMpa);
        ex.levelNrCmMin = Math.min(ex.levelNrCmMin, s.vessel.levelNrCm);
        ex.levelNrCmMax = Math.max(ex.levelNrCmMax, s.vessel.levelNrCm);
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
git commit -m "feat(runtime): per-step extrema travel with every emitted snapshot

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 4: Trend buffer keeps every snapshot and draws the extrema envelope

**Files:**
- Modify: `ui/src/sim/history.ts` (full rewrite below)
- Modify: `ui/src/components/StripChart.tsx` (`Series.envelope`, auto-scale, band)
- Modify: `ui/src/components/TrendPanel.tsx`

**Interfaces:**
- Consumes: `Snapshot.extrema` (Task 3).
- Produces: `TrendSample.powerPctMax`, `.pressureMpaMax`, `.levelNrCmMin`, `.levelNrCmMax`; `Series.envelope?: { lo: (s: TrendSample) => number; hi: (s: TrendSample) => number }`.

- [ ] **Step 1: Rewrite `ui/src/sim/history.ts`**

```ts
// Rolling trend buffer sampled from snapshots (sim time based).
// Every emitted snapshot is kept (10 Hz x 600 s = 6000 samples) and each sample
// carries the engine's per-step extrema, so no excursion between snapshots is
// lost on the charts.
import { useRef } from "react";
import type { Snapshot } from "./types";

export interface TrendSample {
  t: number;
  /** Thermal power, % of rated. */
  powerPct: number;
  /** Neutron power peak over the physics steps behind this sample, % of rated. */
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
  return {
    t: s.timeS,
    powerPct: (s.neutronics.thermalPowerMw / 870) * 100,
    powerPctMax: Math.max(ex.powerFracMax, s.neutronics.powerFrac) * 100,
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

- [ ] **Step 3: Use the envelope on the safety-relevant charts in `TrendPanel.tsx`**

Replace the pressure and level charts:

```tsx
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

Leave the THERMAL POWER chart without an envelope: the drawn line is thermal power while the extremum is neutron power, and mixing them on one axis would mislead.

- [ ] **Step 4: Typecheck and build**

Run: `npm --prefix ui run typecheck && npm --prefix ui run build`
Expected: no errors.

- [ ] **Step 5: Manual verification (no TypeScript test runner, no new deps)**

Run: `npm run dev`
1. Load FULL POWER, set speed 30×, wait 20 s, trip the turbine from the turbine panel.
2. Expected: the RX PRESSURE chart shows an amber band above the line during the pressure rise after the trip, and the LEVEL NR chart shows a cyan band around the shrink/swell. With 1× speed the band is thin (10 Hz snapshot ≈ 2 steps); at 30× it is clearly visible (≈60 steps per snapshot).
3. Cross-check the peak against the engine: `cargo run --release -p bwr-core --example trace -- full 120 turbinetrip@5` and confirm the printed `Pmpa` column's maximum is not above the top of the drawn band.

- [ ] **Step 6: Commit**

```bash
git add ui/src/sim/history.ts ui/src/components/StripChart.tsx ui/src/components/TrendPanel.tsx
git commit -m "feat(ui): keep every snapshot in the trend buffer and draw the per-step extrema envelope

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 5: Compiler-checked UI contract generated from Rust

**Files:**
- Create: `crates/bwr-core/tests/contract.rs`
- Create (generated): `ui/src/sim/contract.generated.ts`
- Modify: `.github/workflows/validate.yml`
- Modify: `crates/bwr-core/tests/plant.rs` (delete the now-redundant `json_contract_matches_ui`, keeping the command-parse half)

**Interfaces:**
- Consumes: `Sim::new`, `Sim::snapshot`, every `Command` variant from `api.rs`, `ScenarioName`, `ModeSwitch`, `LevelControlMode`.
- Produces: `ui/src/sim/contract.generated.ts` exporting `SNAPSHOT_FIXTURES: Snapshot[]`, `COMMAND_FIXTURES: Command[]`, `SNAPSHOT_KEY_CHECK: Record<keyof Snapshot, true>`.

How it works: a fresh object literal assigned to a typed `const` is checked for excess properties recursively, and string-literal unions are checked contextually, so a field added in Rust but missing in `types.ts` fails `tsc`, and a wrong enum spelling fails `tsc`. `SNAPSHOT_KEY_CHECK` closes the other direction at the top level: a key present in `types.ts` but absent from the Rust output is a missing-property error. Nested fields present in `types.ts` but absent from Rust would be reported as missing properties in the fixture literal itself.

- [ ] **Step 1: Write the generator test**

Create `crates/bwr-core/tests/contract.rs`:

```rust
//! Generates ui/src/sim/contract.generated.ts from the Rust API so that
//! `npm --prefix ui run typecheck` fails when types.ts and api.rs diverge.
//! The file is rewritten only when its content changes; CI checks it is committed.

use std::fs;
use std::path::PathBuf;

use bwr_core::api::{Command, ModeSwitch, ScenarioName};
use bwr_core::bop::LevelControlMode;
use bwr_core::Sim;

/// One example per `Command` variant. The `match` below has no wildcard, so adding a
/// variant to `Command` fails to compile here until an example is added.
fn all_commands() -> Vec<Command> {
    let v = vec![
        Command::SetPaused { paused: true },
        Command::SetSpeed { speed: 5.0 },
        Command::LoadScenario { name: ScenarioName::FullPower },
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
        Command::SetLevelControl { mode: LevelControlMode::ThreeElement },
        Command::SetFcvManual { pos: 0.5 },
        Command::SetLevelSetpoint { cm: -40.0 },
        Command::SetHeaters { in_service: false },
        Command::SetRwcuReject { kgs: 15.0 },
        Command::SetIcTrain { index: 0, open: true },
        Command::AckAlarms,
    ];
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

fn generated_ts() -> String {
    let scenarios = [ScenarioName::ColdShutdown, ScenarioName::HotStandby, ScenarioName::FullPower];
    let snapshots: Vec<serde_json::Value> =
        scenarios.iter().map(|&n| serde_json::to_value(Sim::new(n).snapshot()).expect("snapshot json")).collect();
    let keys: Vec<String> = snapshots[0].as_object().expect("snapshot is an object").keys().cloned().collect();
    let cmds: Vec<serde_json::Value> = all_commands().iter().map(|c| serde_json::to_value(c).expect("command json")).collect();

    let mut out = String::new();
    out.push_str("// Generated by crates/bwr-core/tests/contract.rs; do not edit.\n");
    out.push_str("// One snapshot per scenario and one example per Command variant, typed against\n");
    out.push_str("// types.ts so `npm --prefix ui run typecheck` fails when api.rs and types.ts diverge.\n");
    out.push_str("import type { Command, Snapshot } from \"./types\";\n\n");
    out.push_str("export const SNAPSHOT_FIXTURES: Snapshot[] = ");
    out.push_str(&serde_json::to_string_pretty(&snapshots).expect("pretty"));
    out.push_str(";\n\nexport const COMMAND_FIXTURES: Command[] = ");
    out.push_str(&serde_json::to_string_pretty(&cmds).expect("pretty"));
    out.push_str(";\n\n// Every top-level key the engine emits. A key declared in types.ts but missing here\n");
    out.push_str("// is a missing-property error, closing the Rust -> TS direction at the top level.\n");
    out.push_str("export const SNAPSHOT_KEY_CHECK: Record<keyof Snapshot, true> = {\n");
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
    assert_eq!(all_commands().len(), 32, "one example per Command variant plus the two SelectRod forms");
}

#[test]
fn generation_is_deterministic() {
    assert_eq!(generated_ts(), generated_ts());
}
```

- [ ] **Step 2: Run it and inspect the generated file**

Run: `cargo test --release -p bwr-core --test contract`
Expected: `test result: ok. 2 passed`, stderr line `updated .../ui/src/sim/contract.generated.ts - commit it`.

Run: `head -20 ui/src/sim/contract.generated.ts && grep -c '"timeS"' ui/src/sim/contract.generated.ts`
Expected: the header comment, the import line, and `3` (one snapshot per scenario).

- [ ] **Step 3: Prove the check bites**

Temporarily add `bogus: number;` to `interface Snapshot` in `ui/src/sim/types.ts`, run `npm --prefix ui run typecheck`.
Expected: errors in `contract.generated.ts`: `Property 'bogus' is missing in type ...` (fixtures) and `Property 'bogus' is missing in type` for `SNAPSHOT_KEY_CHECK`.
Revert the edit. Then temporarily rename `effectiveSpeed` to `effectiveSpeedX` in `types.ts`, typecheck.
Expected: `Object literal may only specify known properties, and 'effectiveSpeed' does not exist in type 'Snapshot'`. Revert.

Run: `npm --prefix ui run typecheck`
Expected: clean.

- [ ] **Step 4: Replace the hand-written key check in `tests/plant.rs`**

In `crates/bwr-core/tests/plant.rs`, replace the body of `json_contract_matches_ui` so it keeps only the command-parse half (the key presence half is now covered by `tests/contract.rs`):

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

(Delete the old `json_contract_matches_ui` function entirely.)

- [ ] **Step 5: CI freshness check**

In `.github/workflows/validate.yml`, in the job that runs `cargo test --release -p bwr-core`, add immediately after that step:

```yaml
      - name: UI contract generated file is committed and current
        run: git diff --exit-code -- ui/src/sim/contract.generated.ts
```

- [ ] **Step 6: Run everything**

Run: `cargo test --release -p bwr-core && npm --prefix ui run typecheck && git diff --exit-code -- ui/src/sim/contract.generated.ts`
Expected: all tests pass; typecheck clean; `git diff` exits 0 after the file is staged (it is new, so stage it first in Step 7 and re-run the diff check as `git diff --cached`... simpler: commit, then `git diff --exit-code` is trivially clean).

- [ ] **Step 7: Commit**

```bash
git add crates/bwr-core/tests/contract.rs crates/bwr-core/tests/plant.rs ui/src/sim/contract.generated.ts .github/workflows/validate.yml
git commit -m "test(contract): generate ui/src/sim/contract.generated.ts from the Rust API and check it in CI

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
cargo test --release -p bwr-core --test contract && git diff --exit-code -- ui/src/sim/contract.generated.ts
```

Expected: the final diff check exits 0 (regeneration is idempotent).

---

### Task 6: Browser mock is unmistakably a demo; optional recorded-snapshot replay

**Files:**
- Modify: `ui/src/sim/mock.ts` (header, replay mode)
- Modify: `ui/src/components/TopBar.tsx` (demo banner), `ui/src/styles.css`
- Create: `crates/bwr-core/examples/record_snapshots.rs`
- Create (generated): `ui/src/sim/recorded/fullPower.json`

**Interfaces:**
- Consumes: `Sim::snapshot_with_extrema` (Task 3), `Command::TripTurbine`.
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
Expected: clean. `vite build` output includes the JSON as part of the mock chunk only (the Tauri path lazy-imports `./mock`, so the recording is not loaded in the desktop app).

Run: `npm run ui:dev`, open the printed URL with `?replay=1`.
Expected: amber "UI DEMO" banner; sim clock advances one second per second from the recording; a turbine trip alarm appears at t ≈ 5 s prefixed `[RECORDED]`.

- [ ] **Step 6: Commit**

```bash
git add ui/src/sim/mock.ts ui/src/components/TopBar.tsx ui/src/styles.css crates/bwr-core/examples/record_snapshots.rs ui/src/sim/recorded/fullPower.json
git commit -m "feat(ui): label the browser mock as a demo and add recorded Rust snapshot replay

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 7: Validation report generator (`--report`) and the fidelity record

**Files:**
- Modify: `crates/bwr-core/examples/validation.rs` (add `--report`)
- Create: `docs/validation/results.md`

**Interfaces:**
- Consumes (P2): `bwr_core::validation::{standard_cases, run_case, compare, Tolerance, Trace, Metrics}`, `bwr_core::sim::DT`, `docs/validation/cases.csv` with header `id,scenario,purpose,reference,tolerance_class` (tolerance_class is `numerical` or `physical`), `docs/validation/baseline/<id>.csv`.
- Produces: `--report` prints Markdown tables; `docs/validation/results.md`.

- [ ] **Step 1: Add the report function**

Add to `crates/bwr-core/examples/validation.rs` (keep P2's existing `main` behaviour for the no-flag and `--write-baseline` paths; add a `--report` branch that calls `report()`):

```rust
use std::collections::BTreeMap;

/// Rows of docs/validation/cases.csv keyed by case id: (purpose, reference, tolerance_class).
fn case_register() -> BTreeMap<String, (String, String, String)> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/validation/cases.csv");
    let text = std::fs::read_to_string(path).expect("docs/validation/cases.csv");
    let mut out = BTreeMap::new();
    for line in text.lines().skip(1) {
        let cols: Vec<&str> = line.split(',').map(|c| c.trim()).collect();
        if cols.len() < 5 {
            continue;
        }
        out.insert(cols[0].to_string(), (cols[2].to_string(), cols[3].to_string(), cols[4].to_string()));
    }
    out
}

/// Print Markdown tables for docs/validation/results.md.
fn report() {
    let register = case_register();
    println!("| Case | Purpose | Tolerance class | Reference | Max |mass residual| kg | Max |energy residual| J | First event | Result |");
    println!("|---|---|---|---|---|---|---|---|");
    let mut failures = 0usize;
    for case in bwr_core::validation::standard_cases() {
        let (purpose, reference, class) = register
            .get(case.id)
            .cloned()
            .unwrap_or_else(|| ("(not in cases.csv)".into(), "none".into(), "numerical".into()));
        let tol = if class == "physical" { bwr_core::validation::Tolerance::physical() } else { bwr_core::validation::Tolerance::numerical() };
        let trace = bwr_core::validation::run_case(&case, bwr_core::sim::DT);
        let m = trace.metrics();
        let baseline_path = format!("{}/../../docs/validation/baseline/{}.csv", env!("CARGO_MANIFEST_DIR"), case.id);
        let result = match std::fs::read_to_string(&baseline_path) {
            Ok(text) => {
                let reference_trace = bwr_core::validation::Trace::from_csv(case.id, &text).expect("baseline csv");
                let violations = bwr_core::validation::compare(&reference_trace.metrics(), &m, &tol);
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
        let first_event = m
            .first_event_time
            .iter()
            .min_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(k, t)| format!("{k} @ {t:.1} s"))
            .unwrap_or_else(|| "none".into());
        println!(
            "| `{}` | {} | {} | {} | {:.3e} | {:.3e} | {} | {} |",
            case.id, purpose, class, reference, m.max_abs_mass_residual_kg, m.max_abs_energy_residual_j, first_event, result
        );
    }
    println!();
    println!("{} case(s) outside tolerance or without baseline.", failures);
    if failures > 0 {
        std::process::exit(1);
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
Expected: one Markdown row per standard case, every row `PASS`, final line `0 case(s) outside tolerance or without baseline.` If any row is `FAIL` or `NO BASELINE`, that is a real finding from Packages 2–9: do not edit baselines to make it pass; record it in `results.md` "Open findings" and stop to report it.

- [ ] **Step 3: Write `docs/validation/results.md`**

Create the file with this structure, filling every table from the commands shown (paste the `--report` output under "Validation results"; copy supported-range rows from the four model docs; count assumption statuses with the one-liner):

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
`docs/validation/baseline/` are regression evidence, not physical truth.

(paste the report tables here)

### Calibration vs. validation split

List, from `docs/validation/cases.csv`, which cases were used to calibrate parameters
(column `purpose` = calibration) and which are reserved validation cases.

## 5. Paired-failure exploration (Package 9)

Copy the table from `crates/bwr-core/tests/scenarios.rs` doc comments / `docs/validation/cases.csv`
rows with purpose `paired-failure`: case id, failures injected, time at which the model
left its envelope (or "stayed inside"), and the last supported observation.

## 6. Runtime guarantees (Package 10)

- Commands apply only between physics steps (`tests/runtime.rs::commands_apply_at_step_boundaries`).
- No simulation time is discarded silently: backlog up to 60 s, beyond that counted
  in `droppedSimTimeS` and shown in the top bar (`advance_never_loses_time_silently`).
- Every emitted snapshot carries per-step extrema (`extrema_expose_excursions_between_snapshots`);
  the pressure and level trends draw them as an envelope.
- `ui/src/sim/contract.generated.ts` is regenerated by `cargo test` and type-checked
  against `types.ts`; CI fails if it is stale.
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
4. Trip the turbine by hand. Confirm: SRI event, bypass opens, RX PRESSURE chart
   shows the amber envelope band above the line during the rise, LEVEL NR shows
   the cyan band through the shrink.
5. Compare the chart's pressure peak with
   `cargo run --release -p bwr-core --example trace -- full 120 turbinetrip@5`.
6. Reload the UI with the dev server running (Vite HMR): panels repopulate at once
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

(Any `FAIL` / `NO BASELINE` rows from the report, with the owning package.)
```

- [ ] **Step 4: Fill every table**

Run the three commands named in the file and paste their output. For Section 1 copy the envelope rows verbatim from the four model documents. For Section 5 copy the Package 9 table. Replace `<git rev-parse --short HEAD>` and the date. The file must contain no `...` cells when committed.

Run: `grep -n '\.\.\.' docs/validation/results.md`
Expected: no output.

- [ ] **Step 5: Commit**

```bash
git add crates/bwr-core/examples/validation.rs docs/validation/results.md
git commit -m "docs(validation): publish the fidelity record and add --report to the validation runner

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 8: README matches the executable

**Files:**
- Modify: `README.md` (Status table, Playing hints that reference removed behaviour, Project layout, Data sources)

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
| Vessel | Mass and energy balance with IAPWS-IF97 saturation tables plus liquid compressibility; vented, saturated, water-solid and gas-blanketed regimes; collapsed and mixture levels kept separate |
| Natural circulation | Core flow from the downcomer / core + chimney density difference with drift-flux voids and hydrostatic head that follows the actual downcomer level |
| Steam | Two main steam lines with individual MSIV flows, a lumped line/header volume, turbine stop valve, turbine control valves, bypass to the condenser, head vent |
| Turbine-generator | EHC pressure regulator, speed governor, shaft dynamics, synchroscope, generator breaker, overspeed, extraction steam accounted in the shaft balance |
| Condenser | Circulating water pumps, vacuum pump, air in-leakage, hotwell with explicit makeup / reject to a declared condensate storage inventory |
| Feedwater | 2 condensate + 3 feed pumps with pump curves and coastdown, equal-percentage control valve, manual / 1-element / 3-element level control with bumpless transfer, extraction heating, RWCU reject |
| Isolation condensers | 3 trains with steam supply and parallel condensate return valves, tube inventory, pool heat-up and boil-off, unavailability and noncondensable-gas sensitivity |
| Protection | Mode switch, scram, rod blocks, MSIV isolation, IC auto-start, Select Rod Insert, L8 trips, all driven by lagged, range-limited sensor readings; the full cause-and-effect matrix is `docs/model/protection.csv` |
| Runtime | Fixed 50 ms physics step, honest time accounting with effective-speed display, per-step extrema on the trends, generated UI contract |
```

- [ ] **Step 2: Fix the prose that no longer matches**

- In the paragraph under Status, keep the scenarios sentence.
- In "Playing", keep the hints; they still hold. Remove nothing unless a model doc says the behaviour changed (check `docs/model/power-conversion.md` for the feedwater-heating hint).
- In "Project layout", change the `tests/plant.rs` sentence to: "`tests/` holds per-subsystem transient tests, `tests/runtime.rs` for time accounting and command boundaries, and `tests/contract.rs`, which generates `ui/src/sim/contract.generated.ts` so the UI types cannot drift from the engine."
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

### Task 9: Final verification, design audit, release push

**Files:**
- No new files. Possibly `ui/src/**` edits from the design audit.

- [ ] **Step 1: Engine**

Run: `cargo test --release -p bwr-core`
Expected: all tests pass, including `tests/runtime.rs` (9 tests) and `tests/contract.rs` (2 tests).

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

Invoke the `fda:quick` skill on `ui/src` (TopBar effective-speed chip, dropped chip, demo banner, trend envelope). Apply its severity 3–4 fixes and practical severity 2 fixes; re-run `npm --prefix ui run typecheck && npm --prefix ui run build`.

- [ ] **Step 6: Commit and push**

```bash
git add -A
git commit -m "chore(release): Package 10 verification, design audit fixes, fidelity record finalised

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
git push origin main
```

Expected: CI `physics`, `ui`, `validate`, and `desktop` jobs green on GitHub; the desktop artifacts are what Jim tests.

---

## Self-review against the spec (Package 10 bullets)

| Spec bullet | Task |
|---|---|
| Keep Rust authoritative; make browser mock unmistakably a UI demonstration, using recorded Rust snapshots when useful. A browser physics port is a separate decision. | Task 6 (banner, header, `?replay=1`; no port) |
| Generate API types from Rust or add a complete contract check covering commands, field names, units, enums, and nullability. | Task 5 (fixtures typed against `Snapshot`/`Command`, key check; units live in the `types.ts` doc comments and `api.rs` field names, both checked by name) |
| Verify command application at deterministic physics boundaries, scenario reset, pause, reconnect, and initial snapshot delivery. | Task 2 |
| Preserve simulation-time accounting under acceleration and overload. Report effective speed rather than silently discarding accumulated time. | Task 1 |
| Verify that display sampling and trend decimation do not hide excursions used to assess safety behavior. | Tasks 3, 4 |
| Run full engine validation, UI typecheck/build, and desktop smoke tests using the Rust backend. | Task 9 (+ checklist in Task 7) |
| Publish supported ranges, source revisions, outstanding assumptions, validation results, and limitations for every subsystem. | Tasks 7, 8 |

Type-consistency check: `Extrema` field names (`power_frac_max`, `pressure_mpa_max`, `level_nr_cm_min`, `level_nr_cm_max`, `steps`) are identical in `api.rs`, the master contract, `types.ts` (camelCase), `mock.ts`, `history.ts`, and `tests/runtime.rs`. `effective_speed` / `dropped_sim_time_s` ↔ `effectiveSpeed` / `droppedSimTimeS` in all of `sim.rs`, `api.rs`, `types.ts`, `mock.ts`, `TopBar.tsx`. `snapshot_with_extrema` is used by `lib.rs`, `record_snapshots.rs`, and the runtime tests; `snapshot()` never resets the window.
