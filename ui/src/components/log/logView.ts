// LOG page view model (plan C6): filtering, grouping, formatting and CSV. Pure (no React, no DOM).
import type { LogEntry, LogKind } from "../../sim/logic/log.ts";
import { summarizeGroup } from "../../sim/logic/log.ts";

/** Filter chips; one chip can cover several entry kinds. */
export type LogCategory = "actions" | "alarms" | "changes" | "system";

export const LOG_CATEGORIES: readonly { id: LogCategory; label: string }[] = [
  { id: "actions", label: "ACTIONS" },
  { id: "alarms", label: "ALARMS" },
  { id: "changes", label: "CLEARS + CHANGES" },
  { id: "system", label: "SYSTEM" },
];

const CATEGORY_OF: Record<LogKind, LogCategory> = {
  action: "actions",
  alarm: "alarms",
  clear: "changes",
  changed: "changes",
  system: "system",
};

export const KIND_LABEL: Record<LogKind, string> = {
  action: "ACTION",
  alarm: "ALARM",
  clear: "CLEAR",
  changed: "CHANGED",
  system: "SYSTEM",
};

export function categoryOf(kind: LogKind): LogCategory {
  return CATEGORY_OF[kind];
}

export type LogRow =
  | { type: "entry"; key: string; entry: LogEntry; child: boolean }
  | { type: "group"; key: string; groupId: number; entries: LogEntry[]; text: string; expanded: boolean };

export interface LogViewOptions {
  categories: ReadonlySet<LogCategory>;
  /** Case-insensitive substring; blank = no search. */
  query: string;
  /** Group ids shown expanded. */
  expanded: ReadonlySet<number>;
}

export interface LogView {
  rows: LogRow[];
  /** Entries per category that match the search (independent of the chips), groups counted per action. */
  counts: Record<LogCategory, number>;
  /** Entries (groups expanded) that pass chips and search. */
  matched: number;
}

/** The entry a row stands for in time: a group is placed at its first action. */
export function rowAnchor(row: LogRow): LogEntry {
  return row.type === "group" ? row.entries[0] : row.entry;
}

function textMatches(text: string, needle: string): boolean {
  return needle === "" || text.toLowerCase().includes(needle);
}

/** Members of every group with two or more entries (a lone survivor of truncation is shown as a plain entry). */
function collectGroups(entries: readonly LogEntry[]): Map<number, LogEntry[]> {
  const groups = new Map<number, LogEntry[]>();
  for (const e of entries) {
    if (e.group === undefined) continue;
    const members = groups.get(e.group);
    if (members) members.push(e);
    else groups.set(e.group, [e]);
  }
  for (const [id, members] of groups) if (members.length < 2) groups.delete(id);
  return groups;
}

/**
 * Rows in log order (newest last). A group sits where its first action is; when the
 * group summary matches the search every member is shown on expand, otherwise only
 * the matching members are.
 */
export function buildLogView(entries: readonly LogEntry[], opts: LogViewOptions): LogView {
  const needle = opts.query.trim().toLowerCase();
  const groups = collectGroups(entries);
  const counts: Record<LogCategory, number> = { actions: 0, alarms: 0, changes: 0, system: 0 };
  const rows: LogRow[] = [];
  let matched = 0;

  for (const e of entries) {
    const members = e.group === undefined ? undefined : groups.get(e.group);
    if (!members) {
      if (!textMatches(e.text, needle)) continue;
      const cat = categoryOf(e.kind);
      counts[cat]++;
      if (!opts.categories.has(cat)) continue;
      matched++;
      rows.push({ type: "entry", key: `e${e.seq}`, entry: e, child: false });
      continue;
    }
    if (members[0] !== e) continue; // later members are emitted with the group row
    const groupId = e.group as number;
    const text = summarizeGroup(members);
    const shown = textMatches(text, needle) ? members : members.filter((m) => textMatches(m.text, needle));
    if (shown.length === 0) continue;
    const cat = categoryOf(e.kind);
    counts[cat] += shown.length;
    if (!opts.categories.has(cat)) continue;
    matched += shown.length;
    const expanded = opts.expanded.has(groupId);
    rows.push({ type: "group", key: `g${groupId}`, groupId, entries: members, text, expanded });
    if (expanded) for (const m of shown) rows.push({ type: "entry", key: `e${m.seq}`, entry: m, child: true });
  }
  return { rows, counts, matched };
}

/** Index of the row showing `seq` (the entry itself, or its collapsed group); -1 if not shown. */
export function findRowIndex(rows: readonly LogRow[], seq: number): number {
  for (let i = 0; i < rows.length; i++) {
    const row = rows[i];
    if (row.type === "entry" && row.entry.seq === seq) return i;
    if (row.type === "group" && !row.expanded && row.entries.some((m) => m.seq === seq)) return i;
  }
  return -1;
}

function pad(n: number, width = 2): string {
  return String(n).padStart(width, "0");
}

/** Sim seconds -> "hh:mm:ss.s" (tenths truncated; hours keep counting past 24). */
export function formatSimTime(totalS: number): string {
  const tenths = Math.max(0, Math.floor(totalS * 10 + 1e-6));
  const h = Math.floor(tenths / 36000);
  const m = Math.floor((tenths % 36000) / 600);
  const s = Math.floor((tenths % 600) / 10);
  return `${pad(h)}:${pad(m)}:${pad(s)}.${tenths % 10}`;
}

/** Wall clock ms -> local "HH:MM:SS". */
export function formatWallTime(wallMs: number): string {
  const d = new Date(wallMs);
  return `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}

function csvField(value: string): string {
  return /[",\r\n]/.test(value) ? `"${value.replace(/"/g, '""')}"` : value;
}

/** Every entry (groups expanded), oldest first. Wall time is ISO 8601 UTC so the file is unambiguous. */
export function logToCsv(entries: readonly LogEntry[]): string {
  const lines = ["seq,sim_time_s,sim_time,wall_time_utc,kind,severity,group,text"];
  for (const e of entries) {
    lines.push(
      [
        String(e.seq),
        e.simTimeS.toFixed(1),
        formatSimTime(e.simTimeS),
        new Date(e.wallMs).toISOString(),
        e.kind,
        e.severity,
        e.group === undefined ? "" : String(e.group),
        csvField(e.text),
      ].join(","),
    );
  }
  return `${lines.join("\r\n")}\r\n`;
}

/**
 * A cheap fingerprint that changes whenever the visible log content can change
 * (append, truncation, or a group id assigned to the previous action).
 */
export function logFingerprint(entries: readonly LogEntry[]): string {
  const n = entries.length;
  if (n === 0) return "0";
  const prev = n > 1 ? entries[n - 2].group ?? "" : "";
  return `${n}:${entries[0].seq}:${entries[n - 1].seq}:${prev}:${entries[n - 1].group ?? ""}`;
}
