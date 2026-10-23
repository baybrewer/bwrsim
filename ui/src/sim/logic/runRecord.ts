// Run record (plan C4): the UI-owned half of a save file — trend history and event log.
import type { ScenarioName } from "../types";
import type { HistoryData } from "./history.ts";
import { validateHistoryData } from "./history.ts";
import type { LogEntry } from "./log.ts";
import { validateLogEntries } from "./log.ts";
import { SCENARIO_LABELS } from "./labels.ts";

export const RUN_RECORD_VERSION = 1;

export interface RunRecord {
  version: 1;
  runScenario: ScenarioName;
  history: HistoryData;
  log: LogEntry[];
  /** Last annunciator event seq already in `log`; the restored engine continues the sequence. */
  annConsumed: number;
}

/** Validates an untrusted record (from a save file). Returns the record or the first problem found. */
export function validateRunRecord(x: unknown): RunRecord | Error {
  if (typeof x !== "object" || x === null) return new Error("run record: not an object");
  const r = x as Partial<RunRecord>;
  if (r.version !== RUN_RECORD_VERSION) return new Error(`run record: unsupported version ${String(r.version)}`);
  if (typeof r.runScenario !== "string" || !Object.prototype.hasOwnProperty.call(SCENARIO_LABELS, r.runScenario)) {
    return new Error(`run record: unknown scenario ${String(r.runScenario)}`);
  }
  if (typeof r.annConsumed !== "number" || !Number.isInteger(r.annConsumed) || r.annConsumed < 0) {
    return new Error("run record: annConsumed must be a non-negative integer");
  }
  const history = validateHistoryData(r.history);
  if (history instanceof Error) return history;
  const log = validateLogEntries(r.log);
  if (log instanceof Error) return log;
  return r as RunRecord;
}
