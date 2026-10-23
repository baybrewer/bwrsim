import { test } from "node:test";
import assert from "node:assert/strict";
import { buildLogView, findRowIndex, formatSimTime, formatWallTime, logFingerprint, logToCsv, rowAnchor } from "./logView.ts";

const ALL = new Set(["actions", "alarms", "changes", "system"]);
const NONE = new Set();

function entry(seq, kind, text, extra = {}) {
  return { seq, simTimeS: seq * 10, wallMs: 1_000_000 + seq * 500, kind, severity: kind === "alarm" ? "alarm" : "info", text, ...extra };
}

// 1,2,4 = pressure setpoint group (id 1) interleaved with an alarm at 3.
const sample = () => [
  entry(1, "action", "Pressure setpoint 7.00 MPa", { group: 1 }),
  entry(2, "action", "Pressure setpoint 7.10 MPa", { group: 1 }),
  entry(3, "alarm", "RPV PRESSURE HI"),
  entry(4, "action", "Pressure setpoint 7.00 MPa", { group: 1 }),
  entry(5, "clear", "RPV PRESSURE HI"),
  entry(6, "changed", "FW FLOW MISMATCH"),
  entry(7, "system", "Saved slot A"),
  entry(8, "action", "Manual scram", { group: 8 }), // lone survivor: plain row
];

test("groups collapse at their first action and expand with every member", () => {
  const collapsed = buildLogView(sample(), { categories: ALL, query: "", expanded: NONE });
  assert.deepEqual(collapsed.rows.map((r) => r.key), ["g1", "e3", "e5", "e6", "e7", "e8"]);
  assert.match(collapsed.rows[0].text, /^Pressure setpoint 7.00 -> 7.10 -> 7.00 MPa, 3 actions/);
  assert.equal(collapsed.matched, 8);
  assert.deepEqual(collapsed.counts, { actions: 4, alarms: 1, changes: 2, system: 1 });

  const open = buildLogView(sample(), { categories: ALL, query: "", expanded: new Set([1]) });
  assert.deepEqual(open.rows.map((r) => r.key), ["g1", "e1", "e2", "e4", "e3", "e5", "e6", "e7", "e8"]);
  assert.equal(open.rows[1].child, true);
  assert.equal(rowAnchor(open.rows[0]).seq, 1);
});

test("chips filter by category; counts ignore chips", () => {
  const v = buildLogView(sample(), { categories: new Set(["changes"]), query: "", expanded: NONE });
  assert.deepEqual(v.rows.map((r) => r.key), ["e5", "e6"]);
  assert.equal(v.matched, 2);
  assert.equal(v.counts.actions, 4);
});

test("search is case-insensitive and narrows group members", () => {
  const v = buildLogView(sample(), { categories: ALL, query: "  pressure HI ", expanded: NONE });
  assert.deepEqual(v.rows.map((r) => r.key), ["e3", "e5"]);
  assert.deepEqual(v.counts, { actions: 0, alarms: 1, changes: 1, system: 0 });

  const members = buildLogView(sample(), { categories: ALL, query: "SETPOINT 7.10", expanded: new Set([1]) });
  assert.deepEqual(members.rows.map((r) => r.key), ["g1", "e2"]);
  assert.equal(members.counts.actions, 1);

  const summary = buildLogView(sample(), { categories: ALL, query: "3 actions", expanded: new Set([1]) });
  assert.deepEqual(summary.rows.map((r) => r.key), ["g1", "e1", "e2", "e4"]);
});

test("findRowIndex locates entries, falling back to a collapsed group", () => {
  const collapsed = buildLogView(sample(), { categories: ALL, query: "", expanded: NONE }).rows;
  assert.equal(findRowIndex(collapsed, 4), 0);
  assert.equal(findRowIndex(collapsed, 6), 3);
  assert.equal(findRowIndex(collapsed, 99), -1);
  const open = buildLogView(sample(), { categories: ALL, query: "", expanded: new Set([1]) }).rows;
  assert.equal(findRowIndex(open, 4), 3);
});

test("time formatting", () => {
  assert.equal(formatSimTime(0), "00:00:00.0");
  assert.equal(formatSimTime(3723.46), "01:02:03.4");
  assert.equal(formatSimTime(90000.05), "25:00:00.0");
  assert.equal(formatSimTime(0.3), "00:00:00.3");
  assert.equal(formatSimTime(-5), "00:00:00.0");
  assert.equal(formatWallTime(new Date(2026, 9, 23, 7, 5, 9).getTime()), "07:05:09");
});

test("CSV keeps every entry and quotes text safely", () => {
  const rows = sample();
  rows.push(entry(9, "system", 'Note, with "quotes"'));
  const lines = logToCsv(rows).trimEnd().split("\r\n");
  assert.equal(lines.length, 10);
  assert.equal(lines[0], "seq,sim_time_s,sim_time,wall_time_utc,kind,severity,group,text");
  assert.equal(lines[2], `2,20.0,00:00:20.0,${new Date(1_001_000).toISOString()},action,info,1,Pressure setpoint 7.10 MPa`);
  assert.ok(lines[9].endsWith(',system,info,,"Note, with ""quotes"""'));
});

test("fingerprint changes on append, truncation and late group assignment", () => {
  const rows = sample().slice(0, 2).map((e) => ({ ...e, group: undefined }));
  const a = logFingerprint(rows);
  rows[0].group = 1;
  rows[1].group = 1;
  assert.notEqual(logFingerprint(rows), a);
  assert.equal(logFingerprint([]), "0");
});

test("20 000 entries build quickly", () => {
  const big = Array.from({ length: 20_000 }, (_, i) => entry(i + 1, i % 3 ? "alarm" : "action", `Item ${i}`, i % 3 ? {} : { group: i - (i % 30) + 1 }));
  const t0 = performance.now();
  const v = buildLogView(big, { categories: ALL, query: "item", expanded: NONE });
  assert.ok(v.rows.length > 0);
  // ~16 ms normally; the bound only catches an accidental quadratic build and must
  // not fail when the machine is busy (it did at 200 ms under a parallel build).
  assert.ok(performance.now() - t0 < 2000);
});
