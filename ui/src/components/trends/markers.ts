// Log entries -> event markers on the trend time axis. Pure.
import type { LogEntry } from "../../sim/logic/log.ts";

export type MarkerTone = "trip" | "alarm" | "action" | "info";

const TONE_RANK: Record<MarkerTone, number> = { trip: 3, alarm: 2, action: 1, info: 0 };

export function markerTone(e: LogEntry): MarkerTone {
  if (e.severity === "trip") return "trip";
  if (e.kind === "alarm" || (e.kind === "changed" && (e.severity === "alarm" || e.severity === "warning"))) return "alarm";
  if (e.kind === "action") return "action";
  return "info";
}

/** Markers closer than a few pixels merge; the most severe entry represents the cluster. */
export interface MarkerCluster {
  /** Pixel x within the plot (0 = left edge). */
  x: number;
  tone: MarkerTone;
  primary: LogEntry;
  entries: LogEntry[];
}

/** First index with simTimeS >= t (entries sorted by time, as EventLog.byTime). */
function lowerBoundTime(entries: readonly LogEntry[], t: number): number {
  let lo = 0;
  let hi = entries.length;
  while (lo < hi) {
    const mid = (lo + hi) >>> 1;
    if (entries[mid].simTimeS < t) lo = mid + 1;
    else hi = mid;
  }
  return lo;
}

/**
 * Clusters entries in [t0, t1] onto a plot `width` px wide. `entries` must be sorted by
 * (simTimeS, seq), i.e. EventLog.byTime, not the log's seq order: the range search and
 * the left-to-right merge both rely on it. Work is O(log n + k) for k
 * entries in range; the cluster count is bounded by width / minPx.
 */
export function clusterMarkers(entries: readonly LogEntry[], t0: number, t1: number, width: number, minPx = 7): MarkerCluster[] {
  const span = t1 - t0;
  if (!(span > 0) || width <= 0) return [];
  const out: MarkerCluster[] = [];
  for (let i = lowerBoundTime(entries, t0); i < entries.length; i++) {
    const e = entries[i];
    if (e.simTimeS > t1) break;
    const x = ((e.simTimeS - t0) / span) * width;
    const tone = markerTone(e);
    const last = out[out.length - 1];
    if (last && x - last.x < minPx) {
      last.entries.push(e);
      if (TONE_RANK[tone] > TONE_RANK[last.tone]) {
        last.tone = tone;
        last.primary = e;
      }
    } else {
      out.push({ x, tone, primary: e, entries: [e] });
    }
  }
  return out;
}
