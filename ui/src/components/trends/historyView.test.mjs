import { test } from "node:test";
import assert from "node:assert/strict";
import { History } from "../../sim/logic/history.ts";
import { X_CHANNELS } from "../../sim/logic/testkit.mjs";
import { historyStart, valuesAt } from "./historyView.ts";
import { queryCsv } from "./csv.ts";

test("historyStart is null when empty, else the first sample", () => {
  const h = new History(X_CHANNELS);
  assert.equal(historyStart(h), null);
  h.ingestValues(12, [1]);
  h.ingestValues(13, [2]);
  assert.equal(historyStart(h), 12);
});

test("valuesAt returns the last value within tolerance, null for gaps or none", () => {
  const h = new History(X_CHANNELS);
  for (let t = 0; t <= 20; t++) h.ingestValues(t, [t === 10 ? null : t * 2]);
  assert.deepEqual(valuesAt(h, 5.5, 1.5, ["x"]), { x: 10 });
  assert.deepEqual(valuesAt(h, 10, 0.5, ["x"]), { x: null });
  assert.deepEqual(valuesAt(h, 50, 1.5, ["x"]), { x: null });
});

test("after raw and L1 retention expire: readouts are as-of (never future), CSV rows carry true sample bounds", () => {
  const h = new History(X_CHANNELS);
  // x = sample time; the engine was not sampled in [300, 400) (e.g. a host stall).
  for (let t = 0; t <= 100_000; t++) if (t < 300 || t >= 400) h.ingestValues(t, [t]);
  assert.ok(h.level(0).trimmed && h.level(0).t[0] > 20 && h.level(1).trimmed && h.level(1).t[0] > 20, "t=20 only in L2+");

  // Crosshair: t=20 lies inside the L2 bucket [0, 100) whose last sample is 99 (the future).
  assert.deepEqual(valuesAt(h, 20, 1.5, ["x"]), { x: null });
  // Inside a bucket: the previous complete bucket's last sample (199 <= 200), never later.
  assert.deepEqual(valuesAt(h, 200, 150, ["x"]), { x: 199 });
  // No bucket holds t: the previous one lies wholly before t, so its last sample is as-of t.
  assert.deepEqual(valuesAt(h, 350, 150, ["x"]), { x: 299 });
  assert.deepEqual(valuesAt(h, 350, 1.5, ["x"]), { x: null }, "older than the tolerance");
  // L1 (raw expired): same rule with 10 s buckets; raw: exact within tolerance.
  assert.deepEqual(valuesAt(h, 50_005, 15, ["x"]), { x: 49_999 });
  assert.deepEqual(valuesAt(h, 99_990.5, 1.5, ["x"]), { x: 99_990 });
  for (const t of [20, 150, 5_000, 20_000.5, 50_003, 97_000, 99_999.5, 100_000]) {
    const { x } = valuesAt(h, t, 1e9, ["x"]);
    assert.ok(x === null || x <= t, `no future value at ${t} (got ${x})`);
  }

  // Export of [20, 30]: one interval row [0, 100) holding its last sample 99, not "20 -> 99".
  const [head, row, ...rest] = queryCsv(h.query(20, 30, 10, ["x"]), X_CHANNELS).trim().split("\r\n");
  assert.equal(head, "sim_time_s,sim_time,interval_end_s,X []");
  assert.equal(row, "0,00:00:00,100,99.00");
  assert.deepEqual(rest, []);
  // Every exported value was sampled inside its row's bounds, at L1 and L2 alike.
  for (const [t0, t1, px] of [[20, 5_000, 40], [40_000, 41_000, 50]]) {
    const lines = queryCsv(h.query(t0, t1, px, ["x"]), X_CHANNELS).trim().split("\r\n").slice(1);
    assert.ok(lines.length > 0);
    for (const line of lines) {
      const [from, , end, v] = line.split(",").map(Number);
      assert.ok(v >= from && v < end, `${line}: value within [from, end)`);
    }
  }
  // Raw history: exact sample times, no interval.
  const raw = queryCsv(h.query(99_990, 99_993, 4, ["x"]), X_CHANNELS).trim().split("\r\n").slice(1);
  assert.deepEqual(raw, ["99990,27:46:30,,99990.00", "99991,27:46:31,,99991.00", "99992,27:46:32,,99992.00", "99993,27:46:33,,99993.00"]);
});
