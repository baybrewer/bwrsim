// Transport between the UI and the simulator.
// Inside Tauri: the Rust backend emits "sim-state" events and accepts the
// "sim_command" invoke. In a plain browser (vite dev): an in-process mock.

import { useCallback, useEffect, useState } from "react";
import type { Command, Snapshot } from "./types";

export interface SimBackend {
  readonly kind: "tauri" | "mock";
  subscribe(cb: (s: Snapshot) => void): () => void;
  send(cmd: Command): Promise<void>;
}

export const isTauri = (): boolean =>
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

function createTauriBackend(): SimBackend {
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
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("sim_command", { cmd });
    },
  };
}

function createMockBackend(): SimBackend {
  // Loaded lazily so the mock is not part of the Tauri code path.
  let simP: Promise<import("./mock").MockSim> | null = null;
  const getSim = () => (simP ??= import("./mock").then((m) => new m.MockSim()));
  return {
    kind: "mock",
    subscribe(cb) {
      let un: (() => void) | null = null;
      let cancelled = false;
      void getSim().then((sim) => {
        if (!cancelled) un = sim.subscribe(cb);
      });
      return () => {
        cancelled = true;
        un?.();
      };
    },
    async send(cmd) {
      const sim = await getSim();
      sim.command(cmd);
    },
  };
}

let backend: SimBackend | null = null;
export function getBackend(): SimBackend {
  return (backend ??= isTauri() ? createTauriBackend() : createMockBackend());
}

export interface SimHandle {
  snapshot: Snapshot | null;
  /** Fire-and-forget; rejections are surfaced through `lastError`. */
  send: (cmd: Command) => void;
  lastError: string | null;
  backendKind: SimBackend["kind"];
}

export function useSim(): SimHandle {
  const be = getBackend();
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [lastError, setLastError] = useState<string | null>(null);

  useEffect(() => be.subscribe(setSnapshot), [be]);

  useEffect(() => {
    if (!lastError) return;
    const t = setTimeout(() => setLastError(null), 6000);
    return () => clearTimeout(t);
  }, [lastError]);

  const send = useCallback(
    (cmd: Command) => {
      be.send(cmd).catch((e: unknown) => {
        const msg = e instanceof Error ? e.message : String(e);
        console.warn("sim_command failed", cmd, e);
        setLastError(msg);
      });
    },
    [be],
  );

  return { snapshot, send, lastError, backendKind: be.kind };
}
