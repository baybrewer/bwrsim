// Rolling trend buffer sampled from snapshots (sim time based).
import { useRef } from "react";
import type { Snapshot } from "./types";

export interface TrendSample {
  t: number;
  powerPct: number;
  pressureMpa: number;
  levelNrCm: number;
  coreFlowKgs: number;
  steamFlowKgs: number;
  feedFlowKgs: number;
  grossMwe: number;
}

export const TREND_WINDOW_S = 600;
const MIN_SAMPLE_DT = 1;

export function sampleOf(s: Snapshot): TrendSample {
  return {
    t: s.timeS,
    powerPct: (s.neutronics.thermalPowerMw / 870) * 100,
    pressureMpa: s.vessel.pressureMpa,
    levelNrCm: s.vessel.levelNrCm,
    coreFlowKgs: s.vessel.coreFlowKgs,
    steamFlowKgs: s.vessel.steamFlowKgs,
    feedFlowKgs: s.vessel.feedFlowKgs,
    grossMwe: s.turbine.grossMwe,
  };
}

/** Returns the (mutable, append-only) sample array; resets when sim time jumps backwards. */
export function useTrendHistory(s: Snapshot | null): TrendSample[] {
  const buf = useRef<TrendSample[]>([]);
  if (s) {
    const b = buf.current;
    const last = b[b.length - 1];
    if (last && s.timeS < last.t) b.length = 0; // scenario load resets sim time
    if (!last || s.timeS - last.t >= MIN_SAMPLE_DT || b.length === 0) {
      b.push(sampleOf(s));
      const cutoff = s.timeS - TREND_WINDOW_S;
      let drop = 0;
      while (drop < b.length && b[drop].t < cutoff) drop++;
      if (drop > 0) b.splice(0, drop);
    }
  }
  return buf.current;
}
