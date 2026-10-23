// CSV text for the TRENDS export. Pure.
import type { Channel } from "../../sim/logic/channels.ts";
import type { HistoryQueryResult } from "../../sim/logic/history.ts";
import { fmtClock } from "../../util/format.ts";

export interface CsvColumn {
  header: string;
  digits: number;
  values: readonly (number | null)[];
}

/** RFC 4180 quoting for fields containing separators, quotes or line breaks. */
export function csvField(s: string): string {
  return /[",\r\n]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s;
}

const secs = (t: number) => String(Math.round(t * 1000) / 1000);

/**
 * One row per reading; missing values are empty fields. CRLF line ends.
 *
 * A row is never a coarse aggregate dressed as a point sample: `sim_time_s` is when its
 * values were sampled, exactly when `interval_end_s` is empty (raw history). Once raw
 * history has expired, rows are intervals: each value is the last sample taken in
 * [sim_time_s, interval_end_s), and that interval may extend past the exported range.
 */
export function buildCsv(t: readonly number[], intervalEnd: readonly (number | null)[], columns: readonly CsvColumn[]): string {
  const lines = [["sim_time_s", "sim_time", "interval_end_s", ...columns.map((c) => c.header)].map(csvField).join(",")];
  for (let i = 0; i < t.length; i++) {
    const end = intervalEnd[i];
    const row = [secs(t[i]), fmtClock(t[i]), end === null || end === undefined ? "" : secs(end)];
    for (const c of columns) {
      const v = c.values[i];
      row.push(v === null || v === undefined || !Number.isFinite(v) ? "" : v.toFixed(c.digits));
    }
    lines.push(row.map(csvField).join(","));
  }
  return lines.join("\r\n") + "\r\n";
}

/**
 * The TRENDS export of a history query: one row per column, timed by when its values
 * were sampled (exact on raw history, else the coarse bucket's bounds), never by the
 * bucket's clamped drawing position.
 */
export function queryCsv(r: HistoryQueryResult, channels: readonly Channel[]): string {
  return buildCsv(
    r.lastBucketT,
    r.lastBucketT.map((t) => (r.level === 0 ? null : t + r.bucketS)),
    channels.map((c) => ({ header: `${c.label} [${c.unit}]`, digits: c.digits + 2, values: r.series[c.id]?.last ?? [] })),
  );
}

/** "bwrsim-trends_001500-002500.csv" for a [t0, t1] window. */
export function csvFileName(t0: number, t1: number): string {
  const tag = (t: number) => fmtClock(t).replace(/:/g, "");
  return `bwrsim-trends_${tag(t0)}-${tag(t1)}.csv`;
}
