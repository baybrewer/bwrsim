import { test } from "node:test";
import assert from "node:assert/strict";
import { History, HISTORY_B, levelBucketS, validateHistoryData } from "./history.ts";
import { X_CHANNELS, fakeSnapshot } from "./testkit.mjs";

/** Ingests x = f(t) every `dt` seconds over (from, to]. */
function fill(h, from, to, f, dt = 1) {
  for (let t = from; t <= to; t += dt) h.ingestValues(t, [f(t)]);
}

function roundTrip(h) {
  const parsed = JSON.parse(JSON.stringify(h.toJSON()));
  const valid = validateHistoryData(parsed);
  assert.ok(!(valid instanceof Error), String(valid));
  return History.fromJSON(valid, X_CHANNELS);
}

test("ingest respects 1 s spacing; forced checkpoint at t=0.4 after t=0 is kept and upserts", () => {
  const h = new History(X_CHANNELS);
  assert.equal(h.ingestValues(0, [1]), true);
  assert.equal(h.ingestValues(0.4, [2]), false);
  assert.equal(h.ingestValues(0.4, [3], true), true);
  assert.deepEqual(h.level(0).t, [0, 0.4]);
  assert.equal(h.lastT, 0.4);
  // Same-time forced sample replaces, and the coarse bucket forgets the replaced value.
  assert.equal(h.ingestValues(0.4, [5], true), true);
  assert.deepEqual(h.level(0).t, [0, 0.4]);
  assert.deepEqual(h.level(0).series.x.last, [1, 5]);
  const l1 = h.level(1).series.x;
  assert.deepEqual([l1.min[0], l1.max[0], l1.last[0]], [1, 5, 5]);
  assert.equal(h.ingestValues(0.2, [9], true), false, "older than latest");
  assert.equal(h.ingestValues(1.4, [6]), true);
});

test("forced checkpoint upsert rebuilds open buckets at every level", () => {
  const h = new History(X_CHANNELS);
  fill(h, 0, 12_000, () => 1);
  assert.equal(h.levelCount, 3);
  h.ingestValues(12_000.5, [50], true);
  h.ingestValues(12_000.5, [2], true);
  for (let k = 1; k < h.levelCount; k++) {
    const lv = h.level(k);
    const j = lv.t.length - 1;
    assert.equal(lv.series.x.max[j], 2, `level ${k} max`);
    assert.equal(lv.series.x.last[j], 2, `level ${k} last`);
  }
});

test("snapshot ingest uses the registry; null period stays null", () => {
  const h = new History();
  h.ingest(fakeSnapshot({ timeS: 3 }));
  const lv = h.level(0);
  assert.equal(lv.series.periodS.last[0], null);
  assert.equal(lv.series.periodS.gap[0], true);
  assert.equal(lv.series.thermalPowerPct.last[0], 100);
  assert.equal(lv.series.icDutyMw.last[0], 3);
});

test("pyramid levels, bucket alignment and retention", () => {
  const h = new History(X_CHANNELS);
  fill(h, 0, 10_000, (t) => t);
  assert.equal(h.levelCount, 2, "no L2 before L1 exceeds B");
  fill(h, 10_001, 30 * 3600, (t) => t);
  const now = 30 * 3600;
  assert.equal(h.levelCount, 4, "L2 at ~10 240 s, L3 once L2 > B");
  for (let k = 1; k < h.levelCount; k++) {
    const lv = h.level(k);
    assert.ok(lv.t.every((t) => t % levelBucketS(k) === 0), `level ${k} aligned`);
  }
  const top = h.level(h.levelCount - 1);
  assert.ok(top.t.length <= HISTORY_B);
  // L0 keeps >= 1 h (trimmed in batches), L1 >= 24 h; L2+ keep the whole run.
  const l0 = h.level(0);
  assert.ok(l0.trimmed && l0.t[0] <= now - 3600 && l0.t[0] > now - 3600 - 600);
  const l1 = h.level(1);
  assert.ok(l1.trimmed && l1.t[0] <= now - 86_400 && l1.t[0] > now - 86_400 - 6000);
  assert.equal(h.level(2).trimmed, false);
  assert.equal(h.level(2).t[0], 0);
  assert.equal(h.level(3).t[0], 0);
  // Bucket min/max/last of a ramp.
  const l2 = h.level(2).series.x;
  assert.deepEqual([l2.min[1], l2.max[1], l2.last[1]], [100, 199, 199]);
});

test("a one-sample spike survives compaction to the coarsest level and ALL query", () => {
  const h = new History(X_CHANNELS);
  fill(h, 0, 40_000, (t) => (t === 5000 ? 100 : t === 5001 ? -50 : 1));
  for (let k = 1; k < h.levelCount; k++) {
    const lv = h.level(k);
    const j = lv.t.findIndex((t, i) => t <= 5000 && (i === lv.t.length - 1 || lv.t[i + 1] > 5001));
    assert.equal(lv.series.x.max[j], 100, `level ${k}`);
    assert.equal(lv.series.x.min[j], -50, `level ${k}`);
  }
  const q = h.query(0, 40_000, 50);
  assert.equal(Math.max(...q.series.x.max), 100);
  assert.equal(Math.min(...q.series.x.min), -50);
  assert.ok(q.t.length <= 50);
});

/** Gap scenario: in bucket [20,30) value/null/value; bucket [40,50) all null; bucket [60,70) ends in null. */
function gapValue(t) {
  if (t === 21) return null;
  if (t >= 40 && t < 50) return null;
  if (t === 69) return null;
  return 7;
}

test("null handling: mixed, all-null and trailing-null buckets are gaps at every level and after JSON", () => {
  const h = new History(X_CHANNELS);
  fill(h, 0, 12_000, gapValue);
  const l1 = h.level(1);
  const at = (start) => l1.t.indexOf(start);
  const s = l1.series.x;
  assert.deepEqual([s.min[at(20)], s.max[at(20)], s.last[at(20)], s.gap[at(20)]], [7, 7, 7, true]);
  assert.deepEqual([s.min[at(40)], s.max[at(40)], s.last[at(40)], s.gap[at(40)]], [null, null, null, true]);
  assert.deepEqual([s.last[at(60)], s.gap[at(60)]], [null, true]);
  assert.equal(s.gap[at(30)], false);
  // Compacted: L2 bucket [0,100) holds all three.
  const l2 = h.level(2).series.x;
  assert.deepEqual([l2.min[0], l2.max[0], l2.gap[0]], [7, 7, true]);
  assert.equal(l2.gap[1], false);

  const back = roundTrip(h);
  assert.deepEqual(back.toJSON(), h.toJSON());
  for (const hist of [h, back]) {
    // L0 no longer reaches t=0, so [0,99] at 10 px is drawn from L1: one bucket per pixel.
    const q = hist.query(0, 99, 10);
    assert.equal(q.level, 1);
    assert.deepEqual(q.series.x.gap, [false, false, true, false, true, false, true, false, false, false]);
    assert.deepEqual(q.series.x.last.slice(2, 7), [7, 7, null, 7, null]);
    const all = hist.query(0, 12_000, 1); // L0 12 001 and L1 1 201 buckets exceed B -> L2
    assert.equal(all.level, 2);
    assert.equal(all.series.x.gap[0], true);
  }
});

test("pixel columns aggregate gaps and envelopes from L1", () => {
  const h = new History(X_CHANNELS);
  fill(h, 0, 4000, gapValue);
  // 2000 s at 50 px: L0 has 2001 > 1024 buckets, L1 has 200 -> L1, 40 s per pixel.
  const q = h.query(0, 2000, 50);
  assert.equal(q.level, 1);
  assert.deepEqual(q.series.x.gap.slice(0, 3), [true, true, false]);
  // Column [40,80) holds the all-null and trailing-null buckets yet keeps the valid envelope.
  assert.deepEqual([q.series.x.min[1], q.series.x.max[1], q.series.x.last[1]], [7, 7, 7]);
});

test("JSON round trip continues identically", () => {
  const h = new History(X_CHANNELS);
  const f = (t) => Math.sin(t / 50) * 10;
  fill(h, 0, 15_000, f);
  const back = roundTrip(h);
  fill(h, 15_001, 20_000, f);
  fill(back, 15_001, 20_000, f);
  assert.deepEqual(back.toJSON(), h.toJSON());
});

test("restoring an older record adds missing registry channels as all-null gaps", () => {
  const h = new History(X_CHANNELS);
  fill(h, 0, 30, () => 1);
  const extra = [...X_CHANNELS, { ...X_CHANNELS[0], id: "y", get: (s) => s.y }];
  const back = History.fromJSON(h.toJSON(), extra);
  assert.deepEqual(back.channelIds, ["x", "y"]);
  assert.ok(back.level(1).series.y.gap.every(Boolean));
  back.ingest({ timeS: 31, x: 2, y: 3 });
  assert.equal(back.level(0).series.y.last.at(-1), 3);
});

test("query bucket count <= max(4 x pixels, B) for growing 1000 h histories, incl. ALL", () => {
  const h = new History(X_CHANNELS);
  const checkpoints = [600, 3600, 4000, 36_000, 90_000, 360_000, 1_000 * 3600];
  let t = 0;
  for (const end of checkpoints) {
    for (t += 1; t <= end; t++) h.ingestValues(t, [t % 977]);
    t = end;
    const windows = [
      [end - 60, end],
      [end - 600, end],
      [end - 3600, end],
      [end - 86_400, end],
      [0, end],
      [end / 3, end / 2],
    ].map(([a, b]) => [Math.max(0, a), b]);
    for (const px of [50, 600, 2000]) {
      for (const [t0, t1] of windows) {
        const q = h.query(t0, t1, px);
        const label = `end=${end} px=${px} [${t0},${t1}] level=${q.level}`;
        assert.ok(q.buckets <= Math.max(4 * px, HISTORY_B), `${label} buckets=${q.buckets}`);
        assert.ok(q.t.length <= px, label);
        assert.ok(q.t.length > 0, label);
        // The chosen level reaches back to t0 (no silently clipped window start).
        assert.ok(q.t[0] - t0 <= q.bucketS * (1 + (t1 - t0) / px / q.bucketS), `${label} starts at ${q.t[0]}`);
      }
    }
  }
  assert.ok(h.level(h.levelCount - 1).t.length <= HISTORY_B);
  assert.equal(h.levelCount, 5);
});
