// Read helpers over the run History for the TRENDS page. Pure (no React, no DOM).
import type { History } from "../../sim/logic/history.ts";

/** Earliest retained time: L0 is exact; coarser levels give their first bucket start. */
export function historyStart(h: History): number | null {
  for (let k = 0; k < h.levelCount; k++) {
    const lv = h.level(k);
    if (!lv.trimmed && lv.t.length > 0) return lv.t[0];
  }
  for (let k = h.levelCount - 1; k >= 0; k--) {
    const lv = h.level(k);
    if (lv.t.length > 0) return lv.t[0];
  }
  return null;
}

/**
 * As-of value of each channel at time t (History.asOf): the latest sample at or before t,
 * never one taken after it. Null when there is none, it is a gap, it lies wholly before
 * (t - tolS]. Inside a coarse bucket the reading comes from the previous complete
 * bucket, so it can lag t by up to two bucket widths but is never from after t. Cost O(log n).
 */
export function valuesAt(h: History, t: number, tolS: number, ids: readonly string[]): Record<string, number | null> {
  const r = h.asOf(t, ids);
  const recent = r !== null && r.to > t - tolS;
  const out: Record<string, number | null> = {};
  for (const id of ids) out[id] = recent ? (r.values[id] ?? null) : null;
  return out;
}
