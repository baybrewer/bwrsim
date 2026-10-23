// Run event log (plan C4): operator actions, annunciator transitions and system notes. Pure.
import type { AnnEvent, Command, Severity, Snapshot } from "../types";
import { fx, rodName } from "../../util/format.ts";
import { LEVEL_MODE_LABELS, MODE_LABELS, SCENARIO_LABELS, trainLetter } from "./labels.ts";

export type LogKind = "action" | "alarm" | "clear" | "changed" | "system";

export interface LogEntry {
  seq: number;
  simTimeS: number;
  wallMs: number;
  kind: LogKind;
  severity: Severity;
  text: string;
  /** Shared by consecutive same-type setpoint actions (id = seq of the group's first entry). */
  group?: number;
}

export const LOG_CAP = 20_000;
/** Consecutive same-type setpoint actions closer than this (wall time) form one group. */
export const GROUP_WINDOW_MS = 2000;

const LOG_KINDS: readonly LogKind[] = ["action", "alarm", "clear", "changed", "system"];
const SEVERITIES: readonly Severity[] = ["info", "warning", "alarm", "trip"];

function onOff(on: boolean, onText: string, offText: string): string {
  return on ? onText : offText;
}

function rodLabel(index: number, s: Snapshot | null): string {
  const cell = s ? s.rods.layout[index] : undefined;
  return s && cell ? rodName(cell[0], cell[1], s.rods.gridSize) : `#${index + 1}`;
}

function rodsLabel(indices: readonly number[], s: Snapshot | null): string {
  if (indices.length === 1) return `Rod ${rodLabel(indices[0], s)}`;
  if (s && indices.length === s.rods.positions.length) return `All ${indices.length} rods`;
  const names = [...indices].sort((a, b) => a - b).map((i) => rodLabel(i, s));
  return names.length <= 3 ? `Rods ${names.join(", ")}` : `${names.length} rods (${names.slice(0, 3).join(", ")}, …)`;
}

/** Rod drives are fine-motion: show whole notches plainly, fractions to 0.1. */
function notches(v: number): string {
  return Number.isInteger(v) ? String(v) : v.toFixed(1);
}

function assertNever(x: never): never {
  throw new Error(`undescribed command: ${JSON.stringify(x)}`);
}

/**
 * Operator-readable text for a command, as the panels name things. `s` is the snapshot
 * the operator was looking at (rod names come from its core layout).
 */
export function describeCommand(cmd: Command, s: Snapshot | null): string {
  switch (cmd.type) {
    case "setPaused":
      return cmd.paused ? "Simulation paused" : "Simulation resumed";
    case "setSpeed":
      return `Sim speed ${cmd.speed}x`;
    case "loadScenario":
      return `Load scenario ${SCENARIO_LABELS[cmd.name]}`;
    case "manualScram":
      return "Manual scram";
    case "resetScram":
      return "Scram reset";
    case "setModeSwitch":
      return `Mode switch -> ${MODE_LABELS[cmd.mode]}`;
    case "selectRod":
      return cmd.index === null ? "Rod deselected" : `Select rod ${rodLabel(cmd.index, s)}`;
    case "rodSetTarget":
      return `${rodsLabel(cmd.indices, s)} -> ${notches(cmd.target)} notches`;
    case "rodStop":
      return "Rod motion stop";
    case "setIrmRange":
      return `IRM range ${cmd.range}`;
    case "setHeadVent":
      return `RPV head vent ${onOff(cmd.open, "open", "close")}`;
    case "setMsiv":
      return `MSIV ${trainLetter(cmd.line)} ${onOff(cmd.open, "open", "close")}`;
    case "setPressureSetpoint":
      return `Pressure setpoint ${fx(cmd.mpa, 2)} MPa`;
    case "setBypass":
      return cmd.auto ? "Turbine bypass AUTO" : `Turbine bypass manual ${fx(cmd.pos * 100, 0)} %`;
    case "setSpeedSetpoint":
      return `Turbine speed setpoint ${fx(cmd.rpm, 0)} rpm`;
    case "resetTurbine":
      return "Turbine reset";
    case "tripTurbine":
      return "Turbine manual trip";
    case "closeBreaker":
      return "Generator breaker close";
    case "openBreaker":
      return "Generator breaker open";
    case "setLoadLimit":
      return `Load limit ${fx(cmd.pct, 0)} %`;
    case "setCwPump":
      return `CW pump ${trainLetter(cmd.index)} ${onOff(cmd.on, "start", "stop")}`;
    case "setVacuumPump":
      return `Vacuum pump ${onOff(cmd.on, "start", "stop")}`;
    case "setFeedPump":
      return `Feed pump ${trainLetter(cmd.index)} ${onOff(cmd.on, "start", "stop")}`;
    case "setCondensatePump":
      return `Condensate pump ${trainLetter(cmd.index)} ${onOff(cmd.on, "start", "stop")}`;
    case "setLevelControl":
      return `Level control ${LEVEL_MODE_LABELS[cmd.mode]}`;
    case "setFcvManual":
      return `FCV manual ${fx(cmd.pos * 100, 0)} %`;
    case "setLevelSetpoint":
      return `Level setpoint ${fx(cmd.cm, 0)} cm`;
    case "setHeaters":
      return `Feedwater heaters ${onOff(cmd.inService, "in service", "out of service")}`;
    case "setRwcuReject":
      return `RWCU reject ${fx(cmd.kgs, 1)} kg/s`;
    case "setIcTrain":
      return `IC train ${trainLetter(cmd.index)} return valve ${onOff(cmd.open, "open", "close")}`;
    case "ackAlarms":
      return "Alarms acknowledged";
    default:
      return assertNever(cmd);
  }
}

/** Setpoint-style commands that are swept in steps; same key = same knob. null = never grouped. */
export function actionGroupKey(cmd: Command): string | null {
  switch (cmd.type) {
    case "setSpeed":
    case "setIrmRange":
    case "setPressureSetpoint":
    case "setSpeedSetpoint":
    case "setLoadLimit":
    case "setFcvManual":
    case "setLevelSetpoint":
    case "setRwcuReject":
      return cmd.type;
    case "setBypass":
      return cmd.auto ? null : cmd.type;
    case "rodSetTarget":
      return `${cmd.type}:${[...cmd.indices].sort((a, b) => a - b).join(",")}`;
    default:
      return null;
  }
}

/**
 * One-line summary of a group, e.g. "Pressure setpoint 7.00 -> 7.10 -> 7.00 MPa, 3 actions, 1.4 s".
 * Texts of one group share a label prefix and unit suffix; only the value between them varies.
 */
export function summarizeGroup(entries: readonly LogEntry[]): string {
  if (entries.length === 0) return "";
  if (entries.length === 1) return entries[0].text;
  const texts = entries.map((e) => e.text);
  const shortest = Math.min(...texts.map((t) => t.length));
  let pre = 0;
  while (pre < shortest && texts.every((t) => t[pre] === texts[0][pre])) pre++;
  pre = texts[0].lastIndexOf(" ", pre - 1) + 1;
  let suf = 0;
  while (suf < shortest - pre && texts.every((t) => t[t.length - 1 - suf] === texts[0][texts[0].length - 1 - suf])) suf++;
  const tail = texts[0].slice(texts[0].length - suf);
  const unit = tail.includes(" ") ? tail.slice(tail.indexOf(" ")) : "";
  const values = texts.map((t) => t.slice(pre, t.length - unit.length));
  const spanS = (entries[entries.length - 1].wallMs - entries[0].wallMs) / 1000;
  return `${texts[0].slice(0, pre)}${values.join(" -> ")}${unit}, ${entries.length} actions, ${spanS.toFixed(1)} s`;
}

/**
 * The log of one run. Owns entry numbering, action grouping, annunciator dedup
 * (by run id + event seq) and scram/model-error edge detection.
 */
export class EventLog {
  private items: LogEntry[] = [];
  private nextSeq = 1;
  private droppedCount = 0;
  private lastAction: { key: string | null; entry: LogEntry } | null = null;
  private annRun: number | null = null;
  private consumed = 0;
  private baseline: { scrammed: boolean; modelError: string | null } | null = null;
  private readonly cap: number;

  constructor(cap = LOG_CAP) {
    this.cap = cap;
  }

  /** Restores a validated record's entries and annunciator cursor. */
  static fromEntries(entries: readonly LogEntry[], annRunId: number | null, annConsumed: number, cap = LOG_CAP): EventLog {
    const log = new EventLog(cap);
    log.items = entries.map((e) => ({ ...e }));
    log.nextSeq = entries.reduce((m, e) => Math.max(m, e.seq), 0) + 1;
    log.bindRun(annRunId, annConsumed);
    return log;
  }

  get entries(): readonly LogEntry[] {
    return this.items;
  }

  /** Last annunciator seq logged for the bound run (saved in the RunRecord). */
  get annConsumed(): number {
    return this.consumed;
  }

  get annRunId(): number | null {
    return this.annRun;
  }

  /** Points annunciator dedup at a run. A restored run keeps its sequence, so pass its saved cursor. */
  bindRun(runId: number | null, annConsumed = 0): void {
    this.annRun = runId;
    this.consumed = annConsumed;
  }

  /** Sets the scram/model-error baseline without logging (run start, load). */
  seedBaseline(s: Snapshot): void {
    this.baseline = { scrammed: s.scrammed, modelError: s.modelError };
  }

  logAction(cmd: Command, s: Snapshot | null, wallMs: number): LogEntry {
    const key = actionGroupKey(cmd);
    const entry = this.push({ simTimeS: s?.timeS ?? 0, wallMs, kind: "action", severity: "info", text: describeCommand(cmd, s) });
    const prev = this.lastAction;
    if (key !== null && prev && prev.key === key && wallMs - prev.entry.wallMs <= GROUP_WINDOW_MS) {
      prev.entry.group ??= prev.entry.seq;
      entry.group = prev.entry.group;
    }
    this.lastAction = { key, entry };
    return entry;
  }

  /**
   * Logs annunciator events of `runId` not yet consumed, each exactly once, so
   * overlapping snapshot tails and pulls can be fed in freely. Events of another run
   * are ignored. `truncated` (the engine ring lost events) adds a gap note first.
   * Returns the number of entries added.
   */
  logAnnEvents(runId: number, events: readonly AnnEvent[], wallMs: number, truncated = false): number {
    if (this.annRun === null) this.bindRun(runId, 0);
    if (runId !== this.annRun) return 0;
    const fresh = events.filter((e) => e.seq > this.consumed).sort((a, b) => a.seq - b.seq);
    let added = 0;
    if (truncated) {
      const first = fresh[0];
      const missing = first ? first.seq - this.consumed - 1 : 0;
      const text = missing > 0 ? `Annunciator history gap: ${missing} events lost` : "Annunciator history gap: events lost";
      this.push({ simTimeS: first?.timeS ?? this.lastSimTime(), wallMs, kind: "system", severity: "warning", text });
      added++;
    }
    for (const e of fresh) {
      if (e.seq <= this.consumed) continue; // duplicate seq inside one batch
      const kind: LogKind = e.kind === "onset" ? "alarm" : e.kind === "clear" ? "clear" : "changed";
      this.push({ simTimeS: e.timeS, wallMs, kind, severity: e.severity, text: e.text });
      this.consumed = e.seq;
      added++;
    }
    return added;
  }

  /** Logs scram onset and new model errors (edge-triggered against the baseline). */
  observeSnapshot(s: Snapshot, wallMs: number): void {
    const b = this.baseline;
    if (b && s.scrammed && !b.scrammed) {
      this.logSystem(`Reactor scram: ${s.scramReason ?? "reason not reported"}`, s.timeS, wallMs, "trip");
    }
    if (b && s.modelError !== null && s.modelError !== b.modelError) {
      this.logSystem(`Model error: ${s.modelError}`, s.timeS, wallMs, "trip");
    }
    this.seedBaseline(s);
  }

  /** Save, load, scenario load and other notes. */
  logSystem(text: string, simTimeS: number, wallMs: number, severity: Severity = "info"): LogEntry {
    return this.push({ simTimeS, wallMs, kind: "system", severity, text });
  }

  toJSON(): LogEntry[] {
    return this.items.map((e) => ({ ...e }));
  }

  private lastSimTime(): number {
    return this.items.length ? this.items[this.items.length - 1].simTimeS : 0;
  }

  private push(fields: Omit<LogEntry, "seq">): LogEntry {
    const entry: LogEntry = { seq: this.nextSeq++, ...fields };
    this.items.push(entry);
    if (this.items.length > this.cap) this.truncate();
    return entry;
  }

  // Drops a batch (5 % of the cap) at once so a long run does not shift 20 000 entries per event;
  // a single note at the head records how many are gone.
  private truncate(): void {
    const hasNote = this.items[0]?.kind === "system" && this.items[0].text.startsWith("Log truncated:");
    const keep = this.cap - Math.max(1, Math.floor(this.cap / 20));
    const start = hasNote ? 1 : 0;
    const drop = this.items.length - start - keep;
    const removed = this.items.splice(start, drop);
    if (hasNote) this.items.shift();
    this.droppedCount += removed.length;
    const head = this.items[0];
    this.items.unshift({
      // Reuse the newest dropped seq: unique, and ordered before every kept entry.
      seq: removed[removed.length - 1].seq,
      simTimeS: head.simTimeS,
      wallMs: head.wallMs,
      kind: "system",
      severity: "info",
      text: `Log truncated: ${this.droppedCount} oldest entries dropped (limit ${this.cap})`,
    });
  }
}

/** Shape check for stored log entries. */
export function validateLogEntries(x: unknown): LogEntry[] | Error {
  if (!Array.isArray(x)) return new Error("log: not an array");
  let prevSeq = -Infinity;
  for (let i = 0; i < x.length; i++) {
    const e = x[i] as Partial<LogEntry> | null;
    const where = `log entry ${i}`;
    if (typeof e !== "object" || e === null) return new Error(`${where}: not an object`);
    if (typeof e.seq !== "number" || !Number.isInteger(e.seq) || e.seq <= prevSeq) return new Error(`${where}: seq must be increasing integers`);
    prevSeq = e.seq;
    if (typeof e.simTimeS !== "number" || !Number.isFinite(e.simTimeS)) return new Error(`${where}: simTimeS not finite`);
    if (typeof e.wallMs !== "number" || !Number.isFinite(e.wallMs)) return new Error(`${where}: wallMs not finite`);
    if (!LOG_KINDS.includes(e.kind as LogKind)) return new Error(`${where}: unknown kind`);
    if (!SEVERITIES.includes(e.severity as Severity)) return new Error(`${where}: unknown severity`);
    if (typeof e.text !== "string") return new Error(`${where}: text must be a string`);
    if (e.group !== undefined && (typeof e.group !== "number" || !Number.isInteger(e.group))) return new Error(`${where}: group must be an integer`);
  }
  return x as LogEntry[];
}
