// Multiresolution trend history (plan C4). Pure: no React, no DOM.
//
// Level 0 holds raw samples (>= 1 s apart, except forced checkpoints), kept for 1 h.
// Level k >= 1 holds 10^k-second buckets: L1 kept 24 h, L2+ kept for the whole run.
// Every bucket stores, per channel, min/max over non-null values, the last value and
// a gap flag (the bucket contains a null sample). When the coarsest level exceeds
// HISTORY_B buckets a 10x coarser level is built from it, so the coarsest level always
// holds <= HISTORY_B buckets and any window can be drawn in O(pixels + log n).
import type { Snapshot } from "../types";
import type { Channel } from "./channels.ts";
import { CHANNELS, readChannel } from "./channels.ts";

export const HISTORY_B = 1024;
export const MIN_SAMPLE_SPACING_S = 1;
/** Retention per level index; levels beyond this list keep the whole run. */
export const LEVEL_RETAIN_S: readonly number[] = [3600, 86_400];
// Expired entries are dropped in batches so retention is amortized O(1) per ingest.
const TRIM_BATCH = 512;

/** One channel's columns at one level, parallel to the level's `t`. */
export interface SeriesColumns {
  min: (number | null)[];
  max: (number | null)[];
  last: (number | null)[];
  gap: boolean[];
}

/** One pyramid level: bucket start times (L0: sample times), ascending. */
export interface LevelData {
  /** Older entries were dropped by retention, so the level no longer reaches back to run start. */
  trimmed: boolean;
  t: number[];
  series: Record<string, SeriesColumns>;
}

/** JSON form (columnar, null for missing). Level k bucket width is 10^k s (L0 = raw samples). */
export interface HistoryData {
  channels: string[];
  levels: LevelData[];
}

/** Per-pixel columns; only columns that received at least one bucket are present. */
export interface HistoryQueryResult {
  level: number;
  bucketS: number;
  /** Buckets aggregated (the cost of the query). */
  buckets: number;
  /** Start time of the latest bucket in each column. */
  t: number[];
  series: Record<string, SeriesColumns>;
}

export function levelBucketS(level: number): number {
  return 10 ** level;
}

function emptyColumns(): SeriesColumns {
  return { min: [], max: [], last: [], gap: [] };
}

function emptyLevel(channelIds: readonly string[]): LevelData {
  const series: Record<string, SeriesColumns> = {};
  for (const id of channelIds) series[id] = emptyColumns();
  return { trimmed: false, t: [], series };
}

/** First index in [from, arr.length) with arr[i] >= x. */
function lowerBound(arr: readonly number[], x: number, from = 0): number {
  let lo = from;
  let hi = arr.length;
  while (lo < hi) {
    const mid = (lo + hi) >>> 1;
    if (arr[mid] < x) lo = mid + 1;
    else hi = mid;
  }
  return lo;
}

/** First index with arr[i] > x. */
function upperBound(arr: readonly number[], x: number): number {
  let lo = 0;
  let hi = arr.length;
  while (lo < hi) {
    const mid = (lo + hi) >>> 1;
    if (arr[mid] <= x) lo = mid + 1;
    else hi = mid;
  }
  return lo;
}

function minOf(a: number | null, b: number | null): number | null {
  return a === null ? b : b === null ? a : Math.min(a, b);
}

function maxOf(a: number | null, b: number | null): number | null {
  return a === null ? b : b === null ? a : Math.max(a, b);
}

/** Folds entry i of `src` into the open aggregate at index j of `dst`. */
function foldInto(dst: SeriesColumns, j: number, src: SeriesColumns, i: number): void {
  dst.min[j] = minOf(dst.min[j], src.min[i]);
  dst.max[j] = maxOf(dst.max[j], src.max[i]);
  dst.last[j] = src.last[i];
  dst.gap[j] = dst.gap[j] || src.gap[i];
}

function pushFrom(dst: SeriesColumns, src: SeriesColumns, i: number): void {
  dst.min.push(src.min[i]);
  dst.max.push(src.max[i]);
  dst.last.push(src.last[i]);
  dst.gap.push(src.gap[i]);
}

function pushValue(dst: SeriesColumns, v: number | null): void {
  dst.min.push(v);
  dst.max.push(v);
  dst.last.push(v);
  dst.gap.push(v === null);
}

function setValue(dst: SeriesColumns, j: number, v: number | null): void {
  dst.min[j] = v;
  dst.max[j] = v;
  dst.last[j] = v;
  dst.gap[j] = v === null;
}

function foldValue(dst: SeriesColumns, j: number, v: number | null): void {
  dst.min[j] = minOf(dst.min[j], v);
  dst.max[j] = maxOf(dst.max[j], v);
  dst.last[j] = v;
  if (v === null) dst.gap[j] = true;
}

function copyColumns(c: SeriesColumns): SeriesColumns {
  return { min: c.min.slice(), max: c.max.slice(), last: c.last.slice(), gap: c.gap.slice() };
}

/** Aggregates `src` entries [from, to) into `dst` buckets of width bucketS (appending). */
function aggregateRange(src: LevelData, from: number, to: number, dst: LevelData, bucketS: number, ids: readonly string[]): void {
  for (let i = from; i < to; i++) {
    const start = Math.floor(src.t[i] / bucketS) * bucketS;
    const n = dst.t.length;
    if (n > 0 && dst.t[n - 1] === start) {
      for (const id of ids) foldInto(dst.series[id], n - 1, src.series[id], i);
    } else {
      dst.t.push(start);
      for (const id of ids) pushFrom(dst.series[id], src.series[id], i);
    }
  }
}

/**
 * Trend history for one run. Feed it with `ingest` (snapshots) or `ingestValues`
 * (values in `channelIds` order); read it with `query`; persist with `toJSON` /
 * `History.fromJSON`.
 */
export class History {
  private ids: readonly string[];
  private channels: readonly (Channel | undefined)[];
  private levels: LevelData[];

  constructor(channels: readonly Channel[] = CHANNELS) {
    this.channels = channels;
    this.ids = channels.map((c) => c.id);
    this.levels = [emptyLevel(this.ids), emptyLevel(this.ids)];
  }

  /** Restores a validated record. Registry channels absent from the record are added as all-null. */
  static fromJSON(data: HistoryData, channels: readonly Channel[] = CHANNELS): History {
    const h = new History(channels);
    const byId = new Map(channels.map((c) => [c.id, c]));
    h.ids = [...data.channels, ...channels.map((c) => c.id).filter((id) => !data.channels.includes(id))];
    h.channels = h.ids.map((id) => byId.get(id));
    h.levels = data.levels.map((lv) => {
      const series: Record<string, SeriesColumns> = {};
      for (const id of h.ids) {
        const stored = lv.series[id];
        series[id] = stored
          ? copyColumns(stored)
          : { min: lv.t.map(() => null), max: lv.t.map(() => null), last: lv.t.map(() => null), gap: lv.t.map(() => true) };
      }
      return { trimmed: lv.trimmed, t: lv.t.slice(), series };
    });
    return h;
  }

  /** Stored channel ids; `ingestValues` takes values in this order. */
  get channelIds(): readonly string[] {
    return this.ids;
  }

  toJSON(): HistoryData {
    return {
      channels: [...this.channelIds],
      levels: this.levels.map((lv) => {
        const series: Record<string, SeriesColumns> = {};
        for (const id of this.channelIds) series[id] = copyColumns(lv.series[id]);
        return { trimmed: lv.trimmed, t: lv.t.slice(), series };
      }),
    };
  }

  get levelCount(): number {
    return this.levels.length;
  }

  /** Read-only view of one level (tests, diagnostics). */
  level(k: number): Readonly<LevelData> {
    return this.levels[k];
  }

  /** Time of the latest sample, or null when empty. */
  get lastT(): number | null {
    const t = this.levels[0].t;
    return t.length ? t[t.length - 1] : null;
  }

  ingest(s: Snapshot, force = false): boolean {
    return this.ingestValues(
      s.timeS,
      this.channels.map((c) => (c ? readChannel(c, s) : null)),
      force,
    );
  }

  /**
   * Adds a sample. Ordinary samples closer than MIN_SAMPLE_SPACING_S to the previous
   * one are skipped. `force` (save checkpoint) always records the sample, replacing
   * one at the same time, so a saved history ends exactly at the saved state.
   * Samples older than the latest are ignored. Returns whether the sample was stored.
   */
  ingestValues(t: number, values: readonly (number | null)[], force = false): boolean {
    if (!Number.isFinite(t)) return false;
    const vals = values.map((v) => (v === null || !Number.isFinite(v) ? null : v));
    const last = this.lastT;
    if (last !== null) {
      if (t < last) return false;
      if (t === last) {
        if (!force) return false;
        this.replaceLatest(vals);
        return true;
      }
      // Tolerance: sim time accumulates float steps (0.4 + 1.0 lands a hair under 1.4).
      if (t - last < MIN_SAMPLE_SPACING_S - 1e-6 && !force) return false;
    }
    this.append(t, vals);
    return true;
  }

  private append(t: number, vals: readonly (number | null)[]): void {
    const ids = this.channelIds;
    const raw = this.levels[0];
    raw.t.push(t);
    ids.forEach((id, c) => pushValue(raw.series[id], vals[c]));
    for (let k = 1; k < this.levels.length; k++) {
      const lv = this.levels[k];
      const bucketS = levelBucketS(k);
      const start = Math.floor(t / bucketS) * bucketS;
      const n = lv.t.length;
      if (n > 0 && lv.t[n - 1] === start) {
        ids.forEach((id, c) => foldValue(lv.series[id], n - 1, vals[c]));
      } else {
        lv.t.push(start);
        ids.forEach((id, c) => pushValue(lv.series[id], vals[c]));
      }
    }
    this.trim(t);
    this.growPyramid();
  }

  /** Upsert at the latest time: rewrite the raw sample, then rebuild each level's open bucket from the level below. */
  private replaceLatest(vals: readonly (number | null)[]): void {
    const ids = this.channelIds;
    const raw = this.levels[0];
    const j = raw.t.length - 1;
    ids.forEach((id, c) => setValue(raw.series[id], j, vals[c]));
    for (let k = 1; k < this.levels.length; k++) {
      const lv = this.levels[k];
      const below = this.levels[k - 1];
      const open = lv.t.length - 1;
      const start = lv.t[open];
      const from = lowerBound(below.t, start);
      lv.t.pop();
      for (const id of ids) {
        const col = lv.series[id];
        col.min.pop();
        col.max.pop();
        col.last.pop();
        col.gap.pop();
      }
      aggregateRange(below, from, below.t.length, lv, levelBucketS(k), ids);
    }
  }

  private trim(now: number): void {
    LEVEL_RETAIN_S.forEach((retainS, k) => {
      const lv = this.levels[k];
      if (!lv) return;
      // Keep the bucket that contains the cutoff so the level still covers it.
      const drop = lowerBound(lv.t, now - retainS) - (k === 0 ? 0 : 1);
      if (drop < TRIM_BATCH) return;
      lv.t.splice(0, drop);
      for (const id of this.channelIds) {
        const col = lv.series[id];
        col.min.splice(0, drop);
        col.max.splice(0, drop);
        col.last.splice(0, drop);
        col.gap.splice(0, drop);
      }
      lv.trimmed = true;
    });
  }

  private growPyramid(): void {
    for (;;) {
      const top = this.levels[this.levels.length - 1];
      if (top.t.length <= HISTORY_B) return;
      const next = emptyLevel(this.channelIds);
      aggregateRange(top, 0, top.t.length, next, levelBucketS(this.levels.length), this.channelIds);
      // A level built from a trimmed one is missing the same early span.
      next.trimmed = top.trimmed;
      this.levels.push(next);
    }
  }

  private retains(k: number, t0: number): boolean {
    const lv = this.levels[k];
    return !lv.trimmed || (lv.t.length > 0 && lv.t[0] <= t0);
  }

  private range(k: number, t0: number, t1: number): [number, number] {
    const lv = this.levels[k];
    // Coarse buckets are keyed by start time, so begin at the bucket containing t0.
    const startKey = k === 0 ? t0 : Math.floor(t0 / levelBucketS(k)) * levelBucketS(k);
    return [lowerBound(lv.t, startKey), upperBound(lv.t, t1)];
  }

  /**
   * Per-pixel min/max/last/gap over [t0, t1] at `pixels` columns. Uses the finest level
   * that still holds t0 and needs <= max(4 * pixels, HISTORY_B) buckets; the coarsest
   * level always qualifies.
   */
  query(t0: number, t1: number, pixels: number, channelIds: readonly string[] = this.channelIds): HistoryQueryResult {
    const width = Math.max(1, Math.floor(pixels));
    const limit = Math.max(4 * width, HISTORY_B);
    let k = this.levels.length - 1;
    for (let cand = 0; cand < this.levels.length; cand++) {
      if (!this.retains(cand, t0)) continue;
      const [a, b] = this.range(cand, t0, t1);
      if (b - a <= limit) {
        k = cand;
        break;
      }
    }
    const lv = this.levels[k];
    const [from, to] = this.range(k, t0, t1);
    const ids = channelIds.filter((id) => lv.series[id]);
    const out: HistoryQueryResult = { level: k, bucketS: levelBucketS(k), buckets: to - from, t: [], series: {} };
    for (const id of ids) out.series[id] = emptyColumns();
    const span = t1 - t0;
    let col = -1;
    for (let i = from; i < to; i++) {
      const bt = Math.max(lv.t[i], t0);
      const px = span > 0 ? Math.min(width - 1, Math.floor(((bt - t0) / span) * width)) : 0;
      if (px !== col) {
        col = px;
        out.t.push(bt);
        for (const id of ids) pushFrom(out.series[id], lv.series[id], i);
      } else {
        const j = out.t.length - 1;
        out.t[j] = bt;
        for (const id of ids) foldInto(out.series[id], j, lv.series[id], i);
      }
    }
    return out;
  }
}

function isNumOrNull(v: unknown): boolean {
  return v === null || (typeof v === "number" && Number.isFinite(v));
}

/** Checks the JSON shape of a stored history: column lengths, ascending time, value types. */
export function validateHistoryData(x: unknown): HistoryData | Error {
  if (typeof x !== "object" || x === null) return new Error("history: not an object");
  const d = x as Partial<HistoryData>;
  if (!Array.isArray(d.channels) || !d.channels.every((c) => typeof c === "string")) return new Error("history: channels must be strings");
  if (new Set(d.channels).size !== d.channels.length) return new Error("history: duplicate channel ids");
  if (!Array.isArray(d.levels) || d.levels.length < 2) return new Error("history: needs at least levels 0 and 1");
  for (let k = 0; k < d.levels.length; k++) {
    const lv = d.levels[k] as Partial<LevelData> | null;
    const where = `history level ${k}`;
    if (typeof lv !== "object" || lv === null) return new Error(`${where}: not an object`);
    if (typeof lv.trimmed !== "boolean") return new Error(`${where}: trimmed must be boolean`);
    if (!Array.isArray(lv.t)) return new Error(`${where}: t must be an array`);
    for (let i = 0; i < lv.t.length; i++) {
      const t = lv.t[i];
      if (typeof t !== "number" || !Number.isFinite(t)) return new Error(`${where}: t[${i}] not a finite number`);
      if (i > 0 && t <= lv.t[i - 1]) return new Error(`${where}: time not increasing at ${i}`);
    }
    if (typeof lv.series !== "object" || lv.series === null) return new Error(`${where}: series missing`);
    for (const id of d.channels) {
      const col = lv.series[id] as Partial<SeriesColumns> | undefined;
      if (!col) return new Error(`${where}: channel ${id} missing`);
      for (const key of ["min", "max", "last", "gap"] as const) {
        const arr = col[key];
        if (!Array.isArray(arr) || arr.length !== lv.t.length) return new Error(`${where}: ${id}.${key} length mismatch`);
        const ok = key === "gap" ? arr.every((v) => typeof v === "boolean") : arr.every(isNumOrNull);
        if (!ok) return new Error(`${where}: ${id}.${key} has invalid values`);
      }
    }
  }
  return d as HistoryData;
}
