// Browser stand-in for the host save store (plan C2/C5) over localStorage. Same id
// grammar, slot limit, name rules, prepare/commit tokens, version checks and error
// strings as src-tauri, so the Save/Load dialog behaves identically in both backends.
import type { SaveList, SaveMeta, ScenarioName, Snapshot } from "./types";
import type { LoadPrepared, SavePrepared, SaveStore } from "./logic/protocol.ts";
import { RUN_CHANGED } from "./logic/protocol.ts";
import type { MockSim, MockState } from "./mock.ts";
import { MOCK_STATE_VERSION, allocateRunId, parseMockState, restoredSnapshot } from "./mock.ts";

export const MOCK_SAVES_KEY = "bwrsim.saves.v1";
/** Defined here only because this module stands in for the host (MAX_SAVES in src-tauri). */
export const MOCK_MAX_SAVES = 10;
export const SAVE_FILE_FORMAT = 1;
export const SAVE_NAME_MAX = 40;
export const SAVE_ID_RE = /^s_[0-9a-f]{16}$/;
export const STATE_CHANGED = "state changed since Save was opened";

export type KeyValueStorage = Pick<Storage, "getItem" | "setItem">;

interface SaveFile {
  format: number;
  engineVersion: number;
  id: string;
  name: string;
  savedAtMs: number;
  simTimeS: number;
  scenario: ScenarioName;
  runSummary: SaveMeta["runSummary"];
  engine: unknown;
  ui: unknown;
}

/** Every slot keyed by id; a slot's value is untrusted until checked. */
type Slots = Record<string, unknown>;

function isObj(x: unknown): x is Record<string, unknown> {
  return typeof x === "object" && x !== null && !Array.isArray(x);
}

function isSaveFile(x: unknown, id: string): x is SaveFile {
  if (!isObj(x) || x.id !== id || typeof x.name !== "string") return false;
  if (!Number.isFinite(x.format) || !Number.isFinite(x.engineVersion)) return false;
  if (!Number.isFinite(x.savedAtMs) || !Number.isFinite(x.simTimeS) || typeof x.scenario !== "string") return false;
  const r = x.runSummary;
  return isObj(r) && typeof r.mode === "string" && Number.isFinite(r.powerPct) && Number.isFinite(r.pressureMpa) && typeof r.scrammed === "boolean";
}

/** Why a well-formed file cannot be loaded by this build, or null. */
function versionError(f: SaveFile): string | null {
  if (f.format !== SAVE_FILE_FORMAT) return `unsupported save file format ${f.format}`;
  if (f.engineVersion !== MOCK_STATE_VERSION) return `saved by an incompatible simulator (engine version ${f.engineVersion}, this build ${MOCK_STATE_VERSION})`;
  return null;
}

function metaOf(id: string, x: unknown): SaveMeta {
  if (!isSaveFile(x, id)) {
    // Placeholder fields, as the host lists an unreadable file: only id and error are meaningful.
    return {
      id,
      name: id,
      savedAtMs: 0,
      simTimeS: 0,
      scenario: "hotStandby",
      runSummary: { mode: "shutdown", powerPct: 0, pressureMpa: 0, scrammed: false },
      loadable: false,
      error: "unreadable save file",
    };
  }
  const error = versionError(x);
  return {
    id,
    name: x.name,
    savedAtMs: x.savedAtMs,
    simTimeS: x.simTimeS,
    scenario: x.scenario,
    runSummary: { ...x.runSummary },
    loadable: error === null,
    error,
  };
}

function newId(taken: Slots): string {
  for (;;) {
    const bytes = crypto.getRandomValues(new Uint8Array(8));
    const id = `s_${[...bytes].map((b) => b.toString(16).padStart(2, "0")).join("")}`;
    if (!(id in taken)) return id;
  }
}

function randomToken(): string {
  return [...crypto.getRandomValues(new Uint8Array(12))].map((b) => b.toString(16).padStart(2, "0")).join("");
}

function checkId(id: string): void {
  if (!SAVE_ID_RE.test(id)) throw new Error(`invalid save id "${id}"`);
}

/** Trimmed, 1-40 characters, unique case-insensitively among the other saves. */
function checkName(name: string, slots: Slots, selfId: string | null): string {
  const trimmed = name.trim();
  if (trimmed.length === 0) throw new Error("a save name is required");
  if (trimmed.length > SAVE_NAME_MAX) throw new Error(`a save name is at most ${SAVE_NAME_MAX} characters`);
  const lower = trimmed.toLowerCase();
  for (const [id, x] of Object.entries(slots)) {
    if (id !== selfId && isSaveFile(x, id) && x.name.trim().toLowerCase() === lower) throw new Error(`a save named "${trimmed}" already exists`);
  }
  return trimmed;
}

/** An in-memory stand-in when the browser refuses localStorage (private mode, blocked site data). */
export function memoryStorage(): KeyValueStorage {
  const m = new Map<string, string>();
  return { getItem: (k) => m.get(k) ?? null, setItem: (k, v) => void m.set(k, v) };
}

export function createMockSaveStore(sim: MockSim, storage: KeyValueStorage, now: () => number = Date.now): SaveStore {
  let staged: { token: string; state: MockState; runId: number } | null = null;

  const readSlots = (): Slots => {
    const raw = storage.getItem(MOCK_SAVES_KEY);
    if (raw === null) return {};
    let parsed: unknown;
    try {
      parsed = JSON.parse(raw);
    } catch {
      parsed = null;
    }
    // Refuse to overwrite unreadable data rather than silently discarding every save.
    if (!isObj(parsed) || !isObj(parsed.slots)) throw new Error(`saved games in browser storage are unreadable (key ${MOCK_SAVES_KEY})`);
    return parsed.slots;
  };

  const writeSlots = (slots: Slots): void => {
    const json = JSON.stringify({ slots });
    try {
      storage.setItem(MOCK_SAVES_KEY, json);
    } catch (e) {
      const quota = e instanceof Error && /quota/i.test(`${e.name} ${e.message}`);
      throw new Error(
        quota
          ? `browser storage is full (${Math.ceil(json.length / 1024)} KB needed): delete a save and try again`
          : `could not write browser storage: ${e instanceof Error ? e.message : String(e)}`,
      );
    }
  };

  const tokenOf = () => `${sim.runId}:${sim.rev}`;

  return {
    async list(): Promise<SaveList> {
      const slots = readSlots();
      const saves = Object.entries(slots).map(([id, x]) => metaOf(id, x));
      saves.sort((a, b) => b.savedAtMs - a.savedAtMs);
      return { max: MOCK_MAX_SAVES, saves };
    },

    async prepare(runId: number, sinceSeq: number): Promise<SavePrepared> {
      if (runId !== sim.runId) throw new Error(RUN_CHANGED);
      const wasPaused = sim.snapshot().paused;
      if (!wasPaused) sim.command({ type: "setPaused", paused: true });
      const snapshot = sim.snapshot();
      const events = sim.eventsSince(runId, sinceSeq).events.filter((e) => e.seq <= snapshot.annHead);
      return { token: tokenOf(), snapshot, events, wasPaused };
    },

    async write(token, id, name, ui): Promise<SaveMeta> {
      if (!isObj(ui)) throw new Error("run record must be a JSON object");
      if (token !== tokenOf()) throw new Error(STATE_CHANGED);
      const slots = readSlots();
      if (id === null) {
        if (Object.keys(slots).length >= MOCK_MAX_SAVES) throw new Error(`all ${MOCK_MAX_SAVES} save slots are in use`);
      } else {
        checkId(id);
        if (!(id in slots)) throw new Error("save not found");
      }
      const trimmed = checkName(name, slots, id);
      const s: Snapshot = sim.snapshot();
      const fileId = id ?? newId(slots);
      const file: SaveFile = {
        format: SAVE_FILE_FORMAT,
        engineVersion: MOCK_STATE_VERSION,
        id: fileId,
        name: trimmed,
        savedAtMs: now(),
        simTimeS: s.timeS,
        scenario: s.scenario,
        runSummary: { mode: s.modeSwitch, powerPct: s.neutronics.aprmPct, pressureMpa: s.vessel.pressureMpa, scrammed: s.scrammed },
        engine: sim.exportState(),
        ui,
      };
      writeSlots({ ...slots, [fileId]: file });
      return metaOf(fileId, file);
    },

    async loadPrepare(id: string): Promise<LoadPrepared> {
      checkId(id);
      const x = readSlots()[id];
      if (x === undefined) throw new Error("save not found");
      if (!isSaveFile(x, id)) throw new Error("unreadable save file");
      const verr = versionError(x);
      if (verr) throw new Error(verr);
      const state = parseMockState(x.engine);
      if (state instanceof Error) throw new Error(`damaged save file: ${state.message}`);
      // A newer prepare replaces (and so invalidates) an older staged load.
      const runId = allocateRunId();
      staged = { token: randomToken(), state, runId };
      return { token: staged.token, ui: x.ui, snapshot: restoredSnapshot(state, runId) };
    },

    async loadCommit(token: string): Promise<Snapshot> {
      if (!staged || staged.token !== token) throw new Error("no staged load for this token");
      const { state, runId } = staged;
      staged = null;
      return sim.importState(state, runId);
    },

    async loadAbort(token: string): Promise<void> {
      // A stale token is a no-op.
      if (staged && staged.token === token) staged = null;
    },

    async remove(id: string): Promise<void> {
      checkId(id);
      const slots = readSlots();
      if (!(id in slots)) throw new Error("save not found");
      const rest = { ...slots };
      delete rest[id];
      writeSlots(rest);
    },
  };
}
