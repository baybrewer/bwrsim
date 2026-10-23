// Run store (plan C4): the single owner of the current run's trend history and event log.
// Pure TS (no React/DOM): React reads it through useSyncExternalStore (subscribe/getSnapshot).
//
// Every backend snapshot is ingested once, in the subscription callback. Within a run,
// snapshots are ordered by engine `rev`; across runs, by `runId` (monotonic per process),
// and a newer run is only adopted when the store expects it (scenario load, committed load).
import type { AnnEvent, Command, SaveList, SaveMeta, ScenarioName, Snapshot } from "../types";
import type { Channel } from "./channels.ts";
import { CHANNELS } from "./channels.ts";
import { History } from "./history.ts";
import { EventLog, LOG_CAP } from "./log.ts";
import { SCENARIO_LABELS } from "./labels.ts";
import type { RunRecord } from "./runRecord.ts";
import { RUN_RECORD_VERSION, validateRunRecord } from "./runRecord.ts";
import type { LoadPrepared, SimBackend } from "./protocol.ts";
import { RUN_CHANGED, isRunChanged, toError } from "./protocol.ts";

/** Immutable view handed to React; replaced on every change. */
export interface RunView {
  snapshot: Snapshot | null;
  runId: number | null;
  runScenario: ScenarioName | null;
  /** Increments on every change of snapshot, history or log. */
  version: number;
}

/** An open Save dialog: the sim is paused at this checkpoint until `finishSave`. */
export interface SaveSession {
  token: string;
  wasPaused: boolean;
  runId: number;
  snapshot: Snapshot;
}

export interface RunStoreOptions {
  channels?: readonly Channel[];
  logCap?: number;
  now?: () => number;
}

/**
 * Consecutive snapshots of the same unexpected newer run after which the store adopts it
 * anyway (2 s at the 10 Hz publish rate), so a run change from an unforeseen path cannot
 * freeze the display forever.
 */
export const UNEXPECTED_RUN_ADOPT_COUNT = 20;

type Item =
  | { kind: "snapshot"; s: Snapshot }
  | { kind: "checkpoint"; s: Snapshot; events: AnnEvent[]; done: (err: Error | null) => void };

interface PendingLoad {
  runId: number;
  record: RunRecord;
  name: string;
  /** Built from the record before commit, so installing the restored run cannot fail on it. */
  hist: History;
  log: EventLog;
}

/** An action sent while a scenario load is in flight: it executes in the new run, so it is logged there. */
interface BufferedAction {
  cmd: Command;
  wallMs: number;
  rejected: string | null;
}

/** A sent loadScenario not yet seen as a new run, with the actions queued behind it. */
interface ScenarioTicket {
  actions: BufferedAction[];
  /** The backend has applied this load (its send resolved). */
  done: boolean;
}

/** True when `events` (plus nothing older) continue the log right after `consumed`. */
function continues(events: readonly AnnEvent[], consumed: number, head: number): boolean {
  return head <= consumed || events.some((e) => e.seq === consumed + 1);
}

export class RunStore {
  private readonly backend: SimBackend;
  private readonly channels: readonly Channel[];
  private readonly logCap: number;
  private readonly now: () => number;
  private readonly listeners = new Set<() => void>();

  private hist: History;
  private eventLog: EventLog;
  private view: RunView = { snapshot: null, runId: null, runScenario: null, version: 0 };
  private lastRev = -Infinity;

  private queue: Item[] = [];
  private blocked = false;

  /** Scenario loads in flight, oldest first (the backend applies commands in send order). */
  private scenarioTickets: ScenarioTicket[] = [];
  private pendingLoad: PendingLoad | null = null;
  /** A run the engine reported as replaced (RunChanged): its late snapshots are dropped. */
  private goneRunId: number | null = null;
  private unexpected: { runId: number; count: number } | null = null;

  constructor(backend: SimBackend, opts: RunStoreOptions = {}) {
    this.backend = backend;
    this.channels = opts.channels ?? CHANNELS;
    this.logCap = opts.logCap ?? LOG_CAP;
    this.now = opts.now ?? Date.now;
    this.hist = new History(this.channels);
    this.eventLog = new EventLog(this.logCap);
  }

  /** Starts ingesting backend snapshots; returns the unsubscribe function. */
  connect(): () => void {
    return this.backend.subscribe((s) => this.ingest(s));
  }

  // ------------------------------------------------------------ external store
  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };

  getSnapshot = (): RunView => this.view;

  get history(): History {
    return this.hist;
  }

  /** The current run's log (read it; mutate only through the store). */
  get log(): EventLog {
    return this.eventLog;
  }

  /** Last annunciator seq consumed into the log. */
  get annConsumed(): number {
    return this.eventLog.annConsumed;
  }

  /** The UI half of a save file for the current run. */
  buildRecord(): RunRecord {
    const scenario = this.view.runScenario;
    if (scenario === null) throw new Error("no run to record");
    return {
      version: RUN_RECORD_VERSION,
      runScenario: scenario,
      history: this.hist.toJSON(),
      log: this.eventLog.toJSON(),
      annConsumed: this.eventLog.annConsumed,
    };
  }

  // ---------------------------------------------------------------- ingest
  /** Feeds one backend snapshot. Synchronous unless an event pull is needed. */
  ingest(s: Snapshot): void {
    this.queue.push({ kind: "snapshot", s });
    this.drain();
  }

  private drain(): void {
    while (!this.blocked && this.queue.length > 0) {
      const item = this.queue.shift()!;
      const pending = item.kind === "snapshot" ? this.processSnapshot(item.s) : this.processCheckpoint(item);
      if (pending) {
        // Later snapshots wait behind the pull so the log and history stay in engine order.
        this.blocked = true;
        void pending.finally(() => {
          this.blocked = false;
          this.drain();
        });
        return;
      }
    }
  }

  private processSnapshot(s: Snapshot): Promise<void> | null {
    const cur = this.view.runId;
    if (cur === null || s.runId !== cur) {
      // Checked before anything is replaced: a snapshot the store cannot ingest must not
      // leave a half-installed run (or consume the pending load's record).
      if (cur !== null && s.runId < cur) return null;
      this.probe(s);
      if (!this.acceptRun(s)) return null;
    } else if (s.runId === this.goneRunId || s.rev < this.lastRev) {
      return null;
    } else if (s.rev === this.lastRev) {
      // Same engine state (e.g. paused publishes); refresh host-side readouts only.
      this.setView({ snapshot: s });
      return null;
    }
    this.unexpected = null;
    const consumed = this.eventLog.annConsumed;
    if (!continues(s.annTail, consumed, s.annHead)) return this.pullThenApply(s, consumed);
    this.apply(s, s.annTail, false, false);
    return null;
  }

  /** Decides whether a snapshot of another run starts a new run here; resets/installs if so. */
  private acceptRun(s: Snapshot): boolean {
    const cur = this.view.runId;
    if (cur !== null && s.runId < cur) return false;
    const load = this.pendingLoad;
    if (load && s.runId === load.runId) {
      this.pendingLoad = null;
      this.installLoaded(load, s);
      return true;
    }
    if (cur === null) {
      this.startRun(s, `Run start: ${SCENARIO_LABELS[s.scenario]}`);
      return true;
    }
    if (this.scenarioTickets.length > 0) {
      // Several loads can complete between two desktop publishes (10 Hz): the run on
      // screen is the latest *applied* one; earlier applied loads were superseded before
      // they ever published. If none has resolved yet, the snapshot raced its invoke
      // reply and belongs to the oldest load.
      const lastDone = this.scenarioTickets.map((t) => t.done).lastIndexOf(true);
      const idx = Math.max(0, lastDone);
      const superseded = this.scenarioTickets.splice(0, idx);
      const ticket = this.scenarioTickets.shift()!;
      this.startRun(s, `Scenario ${SCENARIO_LABELS[s.scenario]} loaded`);
      const lost = superseded.reduce((n, t) => n + t.actions.length, 0);
      if (superseded.length > 0) {
        // Those actions ran in a run that never published; there is no log to put them in.
        this.eventLog.logSystem(
          `${superseded.length} earlier scenario load(s) replaced before display` + (lost ? `; ${lost} action(s) taken in them are not logged` : ""),
          s.timeS,
          this.now(),
          "warning",
        );
      }
      for (const a of ticket.actions) this.logBuffered(a, s);
      return true;
    }
    if (this.goneRunId === cur) {
      this.startRun(s, `Run replaced by the simulator: ${SCENARIO_LABELS[s.scenario]}`, "warning");
      return true;
    }
    const u = this.unexpected;
    this.unexpected = u && u.runId === s.runId ? { runId: s.runId, count: u.count + 1 } : { runId: s.runId, count: 1 };
    if (this.unexpected.count < UNEXPECTED_RUN_ADOPT_COUNT) return false;
    this.startRun(s, `Run replaced by the simulator: ${SCENARIO_LABELS[s.scenario]}`, "warning");
    return true;
  }

  private startRun(s: Snapshot, note: string, severity: "info" | "warning" = "info"): void {
    this.hist = new History(this.channels);
    this.eventLog = new EventLog(this.logCap);
    // A new engine run numbers its annunciator events from 1.
    this.eventLog.bindRun(s.runId, 0);
    this.eventLog.logSystem(note, s.timeS, this.now(), severity);
    this.beginRunState(s.runId, s.scenario);
  }

  private installLoaded(load: PendingLoad, s: Snapshot): void {
    const rec = load.record;
    this.hist = load.hist;
    // The restored engine keeps its event sequence under the new runId.
    this.eventLog = load.log;
    this.eventLog.bindRun(s.runId, rec.annConsumed);
    this.eventLog.seedBaseline(s);
    this.eventLog.logSystem(`Loaded save "${load.name}"`, s.timeS, this.now());
    this.beginRunState(s.runId, rec.runScenario);
  }

  /**
   * Feeds `s` through the store's consumers on throwaway copies; throws where a real
   * ingest would. Only run changes pay for it (they replace the run, which cannot be undone).
   */
  private probe(s: Snapshot): void {
    new History(this.channels).ingest(s, true);
    const log = new EventLog(this.logCap);
    log.logAnnEvents(s.runId, s.annTail, 0);
    log.observeSnapshot(s, 0);
    continues(s.annTail, 0, s.annHead);
  }

  private beginRunState(runId: number, scenario: ScenarioName): void {
    this.lastRev = -Infinity;
    this.goneRunId = null;
    this.unexpected = null;
    this.view = { ...this.view, runId, runScenario: scenario };
  }

  private pullThenApply(s: Snapshot, consumed: number): Promise<void> {
    return this.backend.eventsSince(s.runId, consumed).then(
      (res) => {
        if (res.runId !== s.runId || this.view.runId !== s.runId) return;
        this.eventLog.logAnnEvents(s.runId, res.events, this.now(), res.truncated);
        this.apply(s, s.annTail, false, false);
      },
      (err: unknown) => {
        if (isRunChanged(err)) {
          // A load or scenario switch replaced this run: log nothing; the new run's
          // snapshots follow the run transition rules.
          this.goneRunId = s.runId;
          return;
        }
        console.warn("annunciator pull failed", err);
        this.apply(s, s.annTail, true, false);
      },
    );
  }

  private processCheckpoint(item: Extract<Item, { kind: "checkpoint" }>): null {
    const s = item.s;
    if (s.runId !== this.view.runId || s.runId === this.goneRunId) {
      item.done(new Error(RUN_CHANGED));
      return null;
    }
    const consumed = this.eventLog.annConsumed;
    const events = [...item.events, ...s.annTail];
    this.apply(s, events, !continues(events, consumed, s.annHead), true);
    item.done(null);
    return null;
  }

  private apply(s: Snapshot, events: readonly AnnEvent[], truncated: boolean, force: boolean): void {
    const now = this.now();
    this.eventLog.logAnnEvents(s.runId, events, now, truncated);
    this.eventLog.observeSnapshot(s, now);
    this.hist.ingest(s, force);
    this.lastRev = Math.max(this.lastRev, s.rev);
    this.setView({ snapshot: s });
  }

  private setView(patch: Partial<RunView>): void {
    this.view = { ...this.view, ...patch, version: this.view.version + 1 };
    for (const l of this.listeners) l();
  }

  private touch(): void {
    this.setView({});
  }

  // --------------------------------------------------------------- actions
  /** Logs the action and sends it through the ordered queue. Rejections are logged and rethrown. */
  send(cmd: Command): Promise<void> {
    // A scenario load starts a new run whose first entry records it; nothing goes into the old log.
    // Commands sent behind it execute in that new run, so they wait for it to be installed.
    const ticket: ScenarioTicket | null = cmd.type === "loadScenario" ? { actions: [], done: false } : null;
    const pendingTicket = this.scenarioTickets.at(-1);
    let buffered: BufferedAction | null = null;
    if (ticket) this.scenarioTickets.push(ticket);
    else if (pendingTicket) {
      buffered = { cmd, wallMs: this.now(), rejected: null };
      pendingTicket.actions.push(buffered);
    } else if (this.view.runId !== null) {
      this.eventLog.logAction(cmd, this.view.snapshot, this.now());
      this.touch();
    }
    return this.backend.send(cmd).then(() => {
      if (ticket) ticket.done = true;
    }, (e: unknown) => {
      const err = toError(e);
      if (ticket) this.dropTicket(ticket);
      if (buffered && this.isBuffered(buffered)) {
        // Still waiting for its run: the note follows the action when the action is logged.
        buffered.rejected = err.message;
      } else if (this.view.runId !== null) {
        this.eventLog.logSystem(`Command rejected: ${err.message}`, this.view.snapshot?.timeS ?? 0, this.now(), "warning");
        this.touch();
      }
      throw err;
    });
  }

  /**
   * A scenario load failed: the commands queued behind it ran in the run before it, which
   * is the previous in-flight load's new run, or else the current run.
   */
  private dropTicket(ticket: ScenarioTicket): void {
    const i = this.scenarioTickets.indexOf(ticket);
    if (i < 0) return;
    this.scenarioTickets.splice(i, 1);
    const previous = i > 0 ? this.scenarioTickets[i - 1] : null;
    if (previous) previous.actions.push(...ticket.actions);
    else if (this.view.runId !== null) {
      for (const a of ticket.actions) this.logBuffered(a, this.view.snapshot);
      this.touch();
    }
  }

  private isBuffered(a: BufferedAction): boolean {
    return this.scenarioTickets.some((t) => t.actions.includes(a));
  }

  private logBuffered(a: BufferedAction, s: Snapshot | null): void {
    this.eventLog.logAction(a.cmd, s, a.wallMs);
    if (a.rejected !== null) this.eventLog.logSystem(`Command rejected: ${a.rejected}`, s?.timeS ?? 0, a.wallMs, "warning");
  }

  listSaves(): Promise<SaveList> {
    return this.backend.saves.list();
  }

  deleteSave(id: string): Promise<void> {
    return this.backend.saves.remove(id);
  }

  /**
   * Opens a save: the host pauses the sim and returns the authoritative checkpoint,
   * which is ingested as a forced history sample with every event through its head.
   */
  async beginSave(): Promise<SaveSession> {
    const runId = this.view.runId;
    if (runId === null) throw new Error("no run to save");
    const prep = await this.backend.saves.prepare(runId, this.eventLog.annConsumed);
    await new Promise<void>((resolve, reject) => {
      this.queue.push({ kind: "checkpoint", s: prep.snapshot, events: prep.events, done: (err) => (err ? reject(err) : resolve()) });
      this.drain();
    });
    return { token: prep.token, wasPaused: prep.wasPaused, runId, snapshot: prep.snapshot };
  }

  /** Writes the checkpoint to a slot (`id` null = new slot). */
  async commitSave(session: SaveSession, id: string | null, name: string): Promise<SaveMeta> {
    if (session.runId !== this.view.runId) throw new Error("the run changed since Save was opened");
    const meta = await this.backend.saves.write(session.token, id, name, this.buildRecord());
    this.eventLog.logSystem(`Saved "${meta.name}"`, session.snapshot.timeS, this.now());
    this.touch();
    return meta;
  }

  /** Closes a save session: resumes the sim if it was running, unless a load replaced the run (A2). */
  async finishSave(session: SaveSession): Promise<void> {
    if (session.wasPaused || session.runId !== this.view.runId) return;
    await this.backend.send({ type: "setPaused", paused: false });
  }

  /** prepare -> validate the run record -> commit; the restored run is installed with its record. */
  async load(id: string, name: string): Promise<Snapshot> {
    const saves = this.backend.saves;
    const prep = await saves.loadPrepare(id);
    // Everything that can reject the restored run is checked before commit (C2/C5): after
    // it the engine has dropped the current run, so a later failure could only lose it.
    let pending: PendingLoad;
    try {
      pending = this.prepareLoad(prep, name);
    } catch (e) {
      await saves.loadAbort(prep.token).catch(() => undefined);
      throw toError(e);
    }
    this.pendingLoad = pending;
    // A failed commit keeps pendingLoad: if the engine did switch, its run still installs
    // with the record instead of being adopted later as an unexpected run without it.
    const snap = await saves.loadCommit(prep.token).catch((e: unknown) => {
      throw toError(e);
    });
    // The subscription may not have delivered the restored run yet; install from the commit result.
    if (this.view.runId !== snap.runId) {
      if (this.pendingLoad) this.pendingLoad.runId = snap.runId;
      this.ingest(snap);
    }
    return snap;
  }

  private prepareLoad(prep: LoadPrepared, name: string): PendingLoad {
    const record = validateRunRecord(prep.ui);
    if (record instanceof Error) throw record;
    const s = prep.snapshot;
    this.probe(s);
    const hist = History.fromJSON(record.history, this.channels);
    const log = EventLog.fromEntries(record.log, s.runId, record.annConsumed, this.logCap);
    return { runId: s.runId, record, name, hist, log };
  }
}
