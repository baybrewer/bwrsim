// Transport between the UI and the simulator.
// Inside Tauri: the Rust backend emits "sim-state" events and accepts the invokes of
// plan C2a. In a plain browser (vite dev): an in-process mock with the same contract.

import { useCallback, useEffect, useState, useSyncExternalStore } from "react";
import type { Command, Snapshot } from "./types";
import type { SaveStore, SimBackend } from "./logic/protocol.ts";
import { toError } from "./logic/protocol.ts";
import { RunStore } from "./logic/runStore.ts";
import type { RunView } from "./logic/runStore.ts";
import type { SerialQueue } from "./logic/serialQueue.ts";
import { createSerialQueue } from "./logic/serialQueue.ts";

export type { SaveStore, SimBackend } from "./logic/protocol.ts";

export const isTauri = (): boolean =>
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/**
 * Orders every state-changing call (commands, save/load steps) through one queue, so a
 * save or load issued after a click runs after every earlier command. Reads are direct.
 */
function withQueue(raw: SimBackend): SimBackend {
  const q: SerialQueue = createSerialQueue();
  const s = raw.saves;
  return {
    kind: raw.kind,
    subscribe: (cb) => raw.subscribe(cb),
    send: (cmd) => q(() => raw.send(cmd)),
    eventsSince: (runId, seq) => raw.eventsSince(runId, seq),
    saves: {
      list: () => s.list(),
      prepare: (runId, sinceSeq) => q(() => s.prepare(runId, sinceSeq)),
      write: (token, id, name, ui) => q(() => s.write(token, id, name, ui)),
      loadPrepare: (id) => q(() => s.loadPrepare(id)),
      loadCommit: (token) => q(() => s.loadCommit(token)),
      loadAbort: (token) => q(() => s.loadAbort(token)),
      remove: (id) => q(() => s.remove(id)),
    },
  };
}

function createTauriBackend(): SimBackend {
  const call = async <T>(command: string, args?: Record<string, unknown>): Promise<T> => {
    const { invoke } = await import("@tauri-apps/api/core");
    try {
      return await invoke<T>(command, args);
    } catch (e) {
      throw toError(e);
    }
  };
  const saves: SaveStore = {
    list: () => call("save_list"),
    prepare: (runId, sinceSeq) => call("save_prepare", { runId, sinceSeq }),
    write: (token, id, name, ui) => call("save_write", { token, id, name, ui }),
    loadPrepare: (id) => call("load_prepare", { id }),
    loadCommit: (token) => call("load_commit", { token }),
    loadAbort: async (token) => {
      await call<null>("load_abort", { token });
    },
    remove: async (id) => {
      await call<null>("save_delete", { id });
    },
  };
  return {
    kind: "tauri",
    subscribe(cb) {
      let unlisten: (() => void) | null = null;
      let cancelled = false;
      void import("@tauri-apps/api/event").then(({ listen }) =>
        listen<Snapshot>("sim-state", (e) => cb(e.payload)).then((un) => {
          if (cancelled) un();
          else unlisten = un;
        }),
      );
      return () => {
        cancelled = true;
        unlisten?.();
      };
    },
    async send(cmd) {
      await call<void>("sim_command", { cmd });
    },
    eventsSince: (runId, seq) => call("sim_events_since", { runId, seq }),
    saves,
  };
}

function browserStorage(memory: () => Pick<Storage, "getItem" | "setItem">): Pick<Storage, "getItem" | "setItem"> {
  try {
    if (window.localStorage) return window.localStorage;
  } catch {
    // Blocked site data: fall through to memory.
  }
  return memory();
}

function createMockBackend(): SimBackend {
  // Loaded lazily so the mock is not part of the Tauri code path.
  type Host = { sim: import("./mock.ts").MockSim; saves: SaveStore };
  let hostP: Promise<Host> | null = null;
  const getHost = () =>
    (hostP ??= Promise.all([import("./mock.ts"), import("./mockSaves.ts")]).then(([m, ms]) => {
      const sim = new m.MockSim();
      return { sim, saves: ms.createMockSaveStore(sim, browserStorage(ms.memoryStorage)) };
    }));
  const saves = async () => (await getHost()).saves;
  return {
    kind: "mock",
    subscribe(cb) {
      let un: (() => void) | null = null;
      let cancelled = false;
      void getHost().then(({ sim }) => {
        if (!cancelled) un = sim.subscribe(cb);
      });
      return () => {
        cancelled = true;
        un?.();
      };
    },
    async send(cmd) {
      (await getHost()).sim.command(cmd);
    },
    eventsSince: async (runId, seq) => (await getHost()).sim.eventsSince(runId, seq),
    saves: {
      list: async () => (await saves()).list(),
      prepare: async (runId, sinceSeq) => (await saves()).prepare(runId, sinceSeq),
      write: async (token, id, name, ui) => (await saves()).write(token, id, name, ui),
      loadPrepare: async (id) => (await saves()).loadPrepare(id),
      loadCommit: async (token) => (await saves()).loadCommit(token),
      loadAbort: async (token) => (await saves()).loadAbort(token),
      remove: async (id) => (await saves()).remove(id),
    },
  };
}

let backend: SimBackend | null = null;
export function getBackend(): SimBackend {
  return (backend ??= withQueue(isTauri() ? createTauriBackend() : createMockBackend()));
}

let runStore: RunStore | null = null;
/** The app-lifetime run store, connected to the backend on first use. */
export function getRunStore(): RunStore {
  if (!runStore) {
    runStore = new RunStore(getBackend());
    runStore.connect();
  }
  return runStore;
}

export interface SimHandle {
  snapshot: Snapshot | null;
  /** Fire-and-forget; logged as an operator action; rejections are surfaced through `lastError`. */
  send: (cmd: Command) => void;
  lastError: string | null;
  backendKind: SimBackend["kind"];
  /** History, log and save/load actions of the current run. */
  store: RunStore;
  /** Changes whenever snapshot, history or log change. */
  version: RunView["version"];
}

export function useSim(): SimHandle {
  const store = getRunStore();
  const view = useSyncExternalStore(store.subscribe, store.getSnapshot);
  const [lastError, setLastError] = useState<string | null>(null);

  useEffect(() => {
    if (!lastError) return;
    const t = setTimeout(() => setLastError(null), 6000);
    return () => clearTimeout(t);
  }, [lastError]);

  const send = useCallback(
    (cmd: Command) => {
      store.send(cmd).catch((e: unknown) => {
        const msg = toError(e).message;
        console.warn("sim_command failed", cmd, e);
        setLastError(msg);
      });
    },
    [store],
  );

  return { snapshot: view.snapshot, send, lastError, backendKind: getBackend().kind, store, version: view.version };
}
