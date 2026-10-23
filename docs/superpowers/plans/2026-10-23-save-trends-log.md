# Save slots, full trend page, event log — plan (r4)

Date: 2026-10-23. Scope: three operator features for the BWRX-300 simulator.
Contract-level plan: interfaces, formats, behaviour, task order and tests. No
full code (lesson from the 2026-10-20 plan set).

Review log: r1 -> 15 comments (docs/reviews/2026-10-23-plan-r1-sol.md), all
accepted in r2. r2 -> 3 unresolved + 5 new (…-r2-sol.md), all accepted in r3:
the host publish cursor is replaced by a stateless event tail + UI pull.
r3 -> 1 unresolved + 1 new, both accepted in r4.

## Requirements (from Jim)

1. **Save state** any time, max **10 save files**. Clicking Save **pauses** the
   sim and lets the operator **pick an existing save** (overwrite) **or type a
   new name**.
2. **Big trend page**: the small charts at the bottom open a full page of
   plots with time-scale control: **last 1 min, 10 min, 1 h, All time** (All
   keeps expanding to include all history).
3. **Event log**: every operator **action** and every **warning/alarm** that
   appears goes in a log the operator can open.

## Assumptions (Jim to veto)

- A1. Load and delete are included, in the same dialog as Save.
- A2. Closing the Save dialog (saved, cancelled or failed) restores the run
  state the sim had before Save was clicked. A successful load leaves the sim
  **paused** (and suppresses that restoration).
- A3. History and log belong to the **current run**. A save stores them; a load
  restores them. `LoadScenario` starts a new run (history and log reset; first
  log entry records it).
- A4. No new dependencies. Charts extend the dependency-free canvas. Pure-logic
  TS is tested with Node 22's built-in runner (C7).
- A5. Saves from an incompatible engine or file version are listed but refuse
  to load, with the reason.

## Architecture

- **Engine** (Rust `Sim`, and `MockSim` for the browser) owns physics state,
  the run identity and the **annunciator transition events** (alarm onsets,
  clears, text changes) — because alarms are condition-based and can appear
  and clear between 10 Hz publishes, only the engine sees every transition.
- **UI run store** (TypeScript, one implementation for both backends) owns
  trend history and the event log. Actions are logged at the single command
  path; annunciator events arrive sequenced inside snapshots.
- **Host save store** (Tauri) owns files. A save file = engine state (opaque to
  the UI) + RunRecord (opaque to the host apart from validation of its
  envelope).
- Accepted cost (Q1): trend resolution is limited by the publish rate (10 Hz
  wall): 1 sample per sim-second at 10x, one per 10 sim-s at 100x. Alarm
  transitions are not affected (engine-sequenced). Revisit with an
  engine-side fine buffer only if needed.

## Contracts

### C1. Engine run identity, events and save/restore (crates/bwr-core)

- `Sim` gains `scenario: ScenarioName` (origin of the run; set in `Sim::new`
  and on successful `LoadScenario`), `run_id: u64` (new value on every
  `LoadScenario` and every restore; never reused within a process) and `rev:
  u64` (incremented on every `apply` and every `step` outcome; saved). All are
  published in `Snapshot` (`scenario`, `runId`, `rev`).
- **Annunciator events**: at the end of every `step` (accepted **or rejected**:
  after a rejection the diff runs on the rolled-back state plus the new
  `model_limit` event, so rolled-back intermediate alarms never appear) and
  after every `apply`, the engine diffs its visible alarm list against the
  previous one and appends
  `AnnEvent { seq: u64, time_s, kind: Onset | Clear | Changed, key, text,
  severity }` to a bounded ring (last 1024). `key` is a stable identity that
  separates condition alarms from operator events (`cond:<id>` vs
  `evt:<id>`), so they never collide. `Changed` = same key, different text or
  severity (e.g. scram-reset block reason). Alarms whose text carries a
  continuously varying number are listed explicitly and compared on their
  non-numeric template only.
- **Stateless delivery** (no publisher cursor): every `Snapshot` carries
  `annHead: u64` (latest seq) and `annTail: AnnEvent[]` (the last <= 64
  events). Consumers deduplicate by (`runId`, `seq`). A consumer whose last
  consumed seq is older than the tail's first seq calls
  `Sim::ann_events_since(run_id, seq) -> { run_id, events, truncated }`
  (Tauri command `sim_events_since`; mock method). If `run_id` is not the
  current run the call returns `RunChanged` with no events; the UI discards
  the pull and waits for the new run's snapshot (run transition rules, C4).
  `truncated` = the ring no longer holds them (UI logs a gap entry).
- **Initialization boundary**: scenario construction (incl. `full_power`'s
  `settle` warm-up steps) records no events; at the end of `Sim::new` the ring
  is cleared, `seq` restarts at 0 and the visible alarms are recorded as
  `Onset` at time 0. The ring and its comparison baseline are part of the
  saved state, so a restore continues the same sequence.
- Serde: derive `Serialize, Deserialize` on `Sim` and every contained type
  (all are `Clone`), private fields included. serde_json feature
  `float_roundtrip` enabled (existing dependency). `acked: HashSet` is
  serialized sorted (deterministic files).
- `pub const ENGINE_STATE_VERSION: u32` in `sim.rs`; bump when any saved struct
  changes shape or meaning.
- `Sim::to_state_json(&self) -> Value`; `Sim::from_state_json(&Value) ->
  Result<Sim, String>` = deserialize + `Sim::validate()` (vector lengths: rod
  `pos/target/weights/fast_insert` = `N_RODS`, IC trains = 3, all f64 finite,
  ranges like `irm_range`, enum states consistent). Restored sim is paused,
  gets a fresh `run_id`, keeps file `speed` and `accumulator`.

### C2. Host save store (src-tauri)

Store logic in a plain module `saves.rs` (unit-testable on a temp dir); the
command handlers are thin.

- Directory `app_data_dir()/saves/`. IDs are generated: `s_` + 16 lowercase hex
  chars. Every command validates the ID against `^s_[0-9a-f]{16}$`, resolves
  `saves/<id>.json` only, and rejects symlinks and non-regular files.
- File (JSON): `{ format: 1, engineVersion, id, name, savedAtMs, simTimeS,
  scenario, runSummary: { mode, powerPct, pressureMpa, scrammed }, engine,
  ui }`. Temporary files are `saves/.tmp-<random>` and are never enumerated.
- A store-wide `Mutex` serializes every list-check-mutate operation.
- `save_list() -> { max: 10, saves: SaveMeta[] }`, newest first. `SaveMeta` =
  file fields minus `engine`/`ui`, plus `loadable` and `error?`. Unreadable or
  invalid files are listed as not loadable (with filename), never fatal.
  `MAX_SAVES = 10` is defined once, in Rust; the UI reads `max`.
- **Save (checkpoint protocol)**:
  1. `save_prepare(runId, sinceSeq) -> { token, snapshot, events, wasPaused }`
     (`RunChanged` error if `runId` is not current):
     under the sim lock, pause, and return the authoritative snapshot, every
     annunciator event after the UI's last consumed seq (`sinceSeq`) through
     the snapshot's `annHead`, and token = (`runId`, `rev`). `wasPaused` lets
     the UI restore the prior state (A2).
  2. UI ingests that snapshot and events as a **forced checkpoint** (C4), then
     calls
     `save_write(token, id | null, name, ui) -> SaveMeta`. Under both locks the
     host checks the sim still matches the token (else "state changed since
     Save was opened" and nothing is written), then captures engine state.
     `id: null` creates (rejected at 10 saves); an `id` must exist (unknown ->
     error, never a create). Name: trimmed, 1-40 chars, unique
     case-insensitively among other saves. Atomic write: temp + fsync + rename.
- **Load (prepare / commit)**:
  1. `load_prepare(id) -> { token, ui, snapshot }`: read, check format and
     engine versions, deserialize + validate engine (C1), stage the `Sim` in
     host memory under `token`; return the RunRecord and the snapshot the staged
     sim will publish. The active sim is untouched.
  2. UI validates the RunRecord (C4 `validateRunRecord`). On success
     `load_commit(token)`: under the sim lock install the staged sim and raise
     the worker reset flag (C3); on failure `load_abort(token)`. One staged load
     at a time; a new prepare replaces an older staged one.
- `save_delete(id)`.
- The token uses the engine's `rev` (C1), so any mutation between prepare and
  write is detected.

### C2a. Exact invoke signatures (Tauri; the mock implements the same in-process)

Argument keys are camelCase (Tauri converts to snake_case Rust params). Errors
are strings; `RunChanged` is the exact error string `"RunChanged"`.

| command | args | returns |
|---|---|---|
| `sim_events_since` | `{ runId, seq }` | `AnnEventsSince` |
| `save_list` | `{}` | `SaveList` |
| `save_prepare` | `{ runId, sinceSeq }` | `{ token: string, snapshot: Snapshot, events: AnnEvent[], wasPaused: boolean }` |
| `save_write` | `{ token, id: string \| null, name, ui: RunRecord }` | `SaveMeta` |
| `load_prepare` | `{ id }` | `{ token: string, ui: unknown, snapshot: Snapshot }` |
| `load_commit` | `{ token }` | `Snapshot` (first snapshot of the restored run) |
| `load_abort` | `{ token }` | `null` |
| `save_delete` | `{ id }` | `null` |

`ui` from `load_prepare` is unvalidated JSON; the UI runs `validateRunRecord`
before `load_commit`. Tokens are opaque strings.

### C3. Tauri worker clock reset

- The worker remembers the `run_id` it last advanced. Under the sim lock, before
  any advance, if `sim.run_id` differs (restore **or** `LoadScenario`, from any
  path) it resets its wall-clock state: `last = now`, `backlog = 0`,
  `effective_speed = 0`, `last_emit_time_s = None`. The engine's own
  `accumulator` is preserved (exact restore). No event cursor exists (C1 is
  stateless), so no commands or events between commit and the reset can be
  skipped; at most one worker iteration (20 ms) of wall time is discarded.

### C4. UI run store (ui/src/sim/logic/, pure TS)

```
interface RunRecord { version: 1; runScenario: ScenarioName;
  history: HistoryData; log: LogEntry[] }
```
- `validateRunRecord(x: unknown): RunRecord | Error` checks version, column
  lengths, monotonic time, value types, log entry shapes.
- **Run store** (`runStore.ts`): a plain object with subscribe/getSnapshot for
  `useSyncExternalStore`. It ingests every snapshot **in the backend
  subscription callback** (not during render), exactly once. Within a run it
  drops any snapshot whose `rev` is not newer than the last ingested (delayed
  pre-checkpoint snapshots cannot replace newer state). It keeps
  `annConsumed` (last consumed seq; saved in the RunRecord) and pulls missing
  events (C1) before ingesting a snapshot whose tail skips ahead. It tracks
  `runId`; a snapshot with a different `runId` than the current run is
  dropped unless the store is expecting it (scenario load or committed load),
  in which case the store resets (scenario) or installs the loaded record
  (load) and seeds its baselines from that snapshot. Snapshots whose `runId`
  is older than the current run are always dropped.
- **Channel registry** (`channels.ts`, SSOT): `{ id, label, unit, digits,
  color, group, get(s): number | null }`. `null` = no value (e.g. `periodS`
  null when the period is effectively infinite). Groups: Reactor (thermal
  power %, APRM %, period s, total reactivity pcm, fuel temp C), Vessel
  (pressure MPa, level NR cm, core flow kg/s, dome temp C), Steam/Feed (steam
  flow, feed flow kg/s, feed temp C), Turbine (speed rpm, gross MWe),
  Condenser/IC (vacuum kPa, IC duty MW). The six mini charts are registry ids.
- **History** (`history.ts`): samples by sim time, minimum spacing 1 s, except
  a **forced checkpoint** (save) which always upserts a sample at the
  checkpoint time (replacing a sample at the same time) so the saved history
  ends exactly at the saved engine state.
  Multiresolution pyramid, each level a columnar time-sorted array with per
  channel `min/max/last` (nulls ignored; a bucket with no values is null):
  - L0 raw 1 s, retained 1 h; L1 10 s buckets, retained 24 h; L2 = 100 s
    buckets and every further level k = 10^k s buckets, retained for the whole
    run. A new coarser level is created whenever the current coarsest exceeds
    `B = 1024` buckets, so the coarsest level always holds <= B buckets for
    the whole run.
  - All levels are appended incrementally on ingest (O(1) amortized per
    level; levels are O(log duration)).
  - Query(t0, t1, pixels): choose the finest level that retains t0 and whose
    bucket count over [t0, t1] is <= max(4 x pixels, B); the coarsest level
    always qualifies, so a level exists for every span and width. Binary-search
    the start;
    aggregate into per-pixel min/max/last. Work is O(pixels + log n) for any
    window including ALL, at any run length.
  - Gaps: each bucket also stores `gap: boolean` = it contained a null sample or
    began after a null; a pixel column aggregating any gap bucket breaks the
    line on both sides while still drawing the valid min/max envelope. A
    bucket whose `last` is null is a gap.
  - JSON: columnar arrays, `null` for missing.
- **Log** (`log.ts`):
  ```
  interface LogEntry { seq: number; simTimeS: number; wallMs: number;
    kind: "action" | "alarm" | "clear" | "changed" | "system";
    severity: "info" | "warning" | "alarm" | "trip"; text: string;
    group?: number }
  ```
  - Actions: recorded when a command is sent, text from
    `describeCommand(cmd, snapshot)` — exhaustive `switch` over the `Command`
    union (new commands fail typecheck until described). Human-readable with
    units ("Rod 14 -> 48 notches", "Pressure setpoint 7.10 MPa").
  - Every action is stored. Consecutive same-type setpoint actions within 2 s
    share a `group` id; the LOG page shows a group collapsed ("Pressure
    setpoint 7.00 -> 7.10 -> 7.00 MPa, 3 actions, 1.4 s") and expandable;
    export keeps every action.
  - Annunciator events (C1) -> `alarm` / `clear` / `changed` entries with
    engine severity, each exactly once (dedup by runId+seq); `truncated` -> a
    `system` gap entry.
  - Scram onset (with reason), model error, save, load, scenario load ->
    `system` entries (scram/model error severity `trip`).
  - Cap 20 000 entries; oldest dropped, with a `system` truncation note.
- Command path: `send()` becomes an ordered queue (each invoke awaits the
  previous), so `save_prepare` issued after a click runs after every earlier
  command.

### C5. Browser mock parity (ui/src/sim/mock.ts)

- `MockSim` publishes `runId`, `scenario`, and annunciator events with the same
  semantics (it already computes alarms and an `alarmState` Map).
- Versioned mock state payload: `{ mockVersion, snapshot, hidden (all
  `Hidden` fields; Maps encoded as entry arrays), phase }`. Import restores
  into the existing instance (no listeners/timers serialized), resets
  wall-clock anchors, leaves paused, emits the restored snapshot.
- Mock `SaveStore` implements C2 semantics over `localStorage`
  (`bwrsim.saves.v1`): same ID grammar, 10 max, name rules, prepare/commit,
  version checks. Quota errors fail the write with a clear message.

### C6. UI pages and dialogs

Navigation: TopBar buttons SAVE, LOAD, TRENDS, LOG. Clicking a mini chart opens
TRENDS scrolled to that chart. `Esc` closes overlays; no other hotkeys.
Overlays do not stop the sim (except Save, A2).

**Save/Load dialog**: up to 10 slots (name, sim time hh:mm:ss, scenario, mode,
power %, pressure, scrammed badge, saved-at, "incompatible"/"unreadable"
badge with reason). Save tab: click a slot -> overwrite confirm; or type a
name -> save new (disabled with the reason at 10). Load tab: click -> confirm
("current run will be replaced") -> prepare/validate/commit. Delete per slot
(confirm). Inline errors. Focus trap, Esc cancels, Enter submits.

**TRENDS page**:
- Range bar `1M 10M 1H ALL` + `LIVE`. ALL = run start to now, expanding.
- Wheel zooms time about the cursor; drag pans; shift-drag box zooms;
  double-click returns to the preset. Zoom/pan leaves LIVE; `LIVE` resumes.
- Synchronized crosshair across charts with sim-time and per-series values
  (missing shown as "—").
- Chart cards per registry group; legend chips toggle series; per-card Y auto
  (fit visible min/max, 5% pad) or lock. Null gaps break the line.
- Event markers from the log on the time axis (trip red, alarm amber, action
  dim); hover shows text; click opens LOG at that entry.
- 1 / 2 column layout toggle. Export visible range as CSV (missing = empty).
- Redraw on requestAnimationFrame at most once per store change; cost per C4
  query bound.

**LOG page**: table (sim time, wall time, kind, severity colour, text; groups
collapsed/expandable), auto-follow newest (off when scrolled up; resume
button), filter chips (Actions / Alarms / Clears+Changes / System), search,
counts. Row click -> TRENDS centred on that time (not LIVE). Export CSV (all
entries, groups expanded).

All UI work goes through the mandatory `frontend-design-audit` before done.

### C7. TS logic tests

- Logic modules under `ui/src/sim/logic/` are pure (no React, no DOM) and use
  explicit `.ts` runtime import specifiers within their graph.
- Tests are `ui/src/sim/logic/*.test.mjs` importing the `.ts` modules.
- Script: `"test": "node --experimental-strip-types --test \"src/sim/logic/*.test.mjs\""`
  (Node 22 treats a bare directory argument as one file; the glob is required)
  in `ui/package.json`. Every task's completion check = `cargo test
  --workspace`, `npm --prefix ui test`, `npm --prefix ui run build`.

## Tasks (in order; each ends green per C7)

1. **Engine** (C1): `scenario`, `run_id`, `rev`, annunciator ring + tail + `ann_events_since`,
   serde + version + `validate`, contract types in `api.rs`/`types.ts`.
2. **Host** (C2, C3): `saves.rs` store, commands, `sim_events_since`, run_id-triggered worker clock reset.
3. **UI command queue + run store skeleton** (C4 run identity, ingest at the
   subscription boundary), mock parity for runId/scenario/events (C5).
4. **Channel registry + pyramid history** (C4); mini trend panel on registry.
5. **Event log** (C4): `describeCommand`, groups, annunciator/system entries.
6. **Save stores** for both backends (C2/C5) + Save/Load dialog (C6).
7. **TRENDS page** (C6).
8. **LOG page** (C6) + cross-links.
9. **Design audit**, full checks, manual Tauri verification, commit, push.

## Test plan

Rust (bwr-core):
- Exact restore: real text round trip of full state; compare every f64 by bit
  pattern and collections semantically (HashSet as set); only the documented
  pause/run_id override differs. Then resume and feed identical `advance()`
  durations with a nonzero fractional `accumulator`, covering pending rod
  motion, controller integrals, acknowledgements, active events and a fault
  state; continuations must stay bit-identical.
- `validate` rejects mismatched rod vectors, wrong IC count, non-finite
  values; version mismatch rejected.
- Annunciator events: a condition that appears and clears within one publish
  interval yields Onset + Clear; changed reset-block reason yields Changed;
  condition and event with the same legacy id stay distinct; ring overflow
  sets truncated; a vessel and a condenser rejection with no further command
  produce their condition and `model_limit` onsets exactly once; FullPower
  construction and LoadScenario(FullPower) publish no warm-up events and no
  entry later than the run's time.
Rust (src-tauri `saves.rs`, temp dir):
- 10-slot limit; concurrent creates at 9 slots -> exactly one succeeds;
  simultaneous same-name writes -> one succeeds; overwrite of unknown id
  rejected; hostile ids (`../x`, absolute, symlink) rejected; temp files not
  listed; unreadable file listed not fatal; token mismatch writes nothing;
  load_prepare of invalid engine leaves active sim untouched.
- Worker reset: resume immediately after load, a load delayed by lock
  contention, and a scenario switch while paused then immediately resumed do
  not charge elapsed wall time to the new run; load_commit -> ManualScram
  before the first publish -> the scram onset reaches the UI.
TS (Node runner):
- History: pyramid levels and retention, envelope preservation of a spike
  through compaction, null handling (all-null and mixed buckets) through JSON
  round trip, query work bounded (count aggregated buckets) for growing
  histories incl. ALL.
- Run store: a pull answered `RunChanged` (load committed between snapshot and
  pull) logs nothing into either run; stale-runId snapshots dropped after load; repeated scenario load
  at t=0 resets; several snapshots before a render all ingested once; a
  delayed pre-checkpoint snapshot (older rev) is dropped; overlapping tail and
  pulled events are logged once; a save at t=0.4 after a sample at t=0
  includes the checkpoint sample; a scenario switch after seq 500 logs the
  new run's first events.
- History gaps: value/null/value inside one bucket and a trailing null both
  render as breaks after compaction and JSON round trip; level creation keeps
  every query <= max(4 x pixels, 1024) buckets for synthetic 1000 h
  histories at widths 50, 600 and 2000 px.
- Log: groups (sweep up and back within 2 s keeps every action), annunciator
  mapping, truncation, `validateRunRecord` failures.
- Mock: state export/import JSON round trip continues identically for N ticks,
  including acknowledged alarms.
Manual (Tauri): save right after a command at 100x -> engine, history and log
agree at the checkpoint; full power -> save -> trip turbine -> load ->
identical; 10-slot limit; TRENDS 1M/10M/1H/ALL with 2 h+ history at 10x.

## Resolved review questions

- Q1 (r1 #7): keep UI-owned trend storage (sampling loss accepted and
  documented), but alarm transitions are engine-sequenced so the log captures
  every engine onset.
- Q2 (r1 #9, r2): 1 h raw / 24 h at 10 s retention kept; dynamically added
  decade levels with indexed queries bound cost for any range and run length.
- Q3 (r1 #14): Node 22 stripping, `.ts` specifiers in logic modules, `.mjs`
  tests, `npm test` in every completion check.
