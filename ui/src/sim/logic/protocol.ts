// Backend contract shared by the bridge, the run store and the mock host (plan C2/C2a). Pure.
import type { AnnEvent, AnnEventsSince, Command, SaveList, SaveMeta, Snapshot } from "../types";
import type { RunRecord } from "./runRecord.ts";

/** Exact error string for a request about a run that is no longer the engine's current run. */
export const RUN_CHANGED = "RunChanged";

export interface SavePrepared {
  token: string;
  snapshot: Snapshot;
  /** Annunciator events after the requested seq through `snapshot.annHead`. */
  events: AnnEvent[];
  wasPaused: boolean;
}

export interface LoadPrepared {
  token: string;
  /** Unvalidated run record from the file; the UI validates it before committing. */
  ui: unknown;
  /** The snapshot the staged sim will publish (its runId is the restored run's). */
  snapshot: Snapshot;
}

/** Host save store (C2a: save_list, save_prepare, save_write, load_prepare, load_commit, load_abort, save_delete). */
export interface SaveStore {
  list(): Promise<SaveList>;
  prepare(runId: number, sinceSeq: number): Promise<SavePrepared>;
  write(token: string, id: string | null, name: string, ui: RunRecord): Promise<SaveMeta>;
  loadPrepare(id: string): Promise<LoadPrepared>;
  loadCommit(token: string): Promise<Snapshot>;
  loadAbort(token: string): Promise<void>;
  remove(id: string): Promise<void>;
}

export interface SimBackend {
  readonly kind: "tauri" | "mock";
  subscribe(cb: (s: Snapshot) => void): () => void;
  /** Ordered: resolves after this command and every earlier one has been applied. */
  send(cmd: Command): Promise<void>;
  eventsSince(runId: number, seq: number): Promise<AnnEventsSince>;
  saves: SaveStore;
}

/** Tauri rejects invokes with bare strings; normalize so callers can read `.message`. */
export function toError(e: unknown): Error {
  if (e instanceof Error) return e;
  return new Error(typeof e === "string" ? e : JSON.stringify(e));
}

export function isRunChanged(e: unknown): boolean {
  return toError(e).message === RUN_CHANGED;
}
