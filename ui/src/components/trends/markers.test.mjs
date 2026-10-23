import { test } from "node:test";
import assert from "node:assert/strict";
import { clusterMarkers, markerTone } from "./markers.ts";
import { RunStore } from "../../sim/logic/runStore.ts";
import { fakeSnapshot } from "../../sim/logic/testkit.mjs";

const e = (seq, simTimeS, kind, severity, text = `e${seq}`) => ({ seq, simTimeS, wallMs: 0, kind, severity, text });

test("tones: trip red, alarm amber, action dim, other info", () => {
  assert.equal(markerTone(e(1, 0, "system", "trip")), "trip");
  assert.equal(markerTone(e(1, 0, "alarm", "warning")), "alarm");
  assert.equal(markerTone(e(1, 0, "changed", "alarm")), "alarm");
  assert.equal(markerTone(e(1, 0, "action", "info")), "action");
  assert.equal(markerTone(e(1, 0, "clear", "info")), "info");
});

test("clusters entries in range, most severe becomes primary", () => {
  const log = [e(1, 5, "action", "info"), e(2, 50, "action", "info"), e(3, 50.3, "system", "trip"), e(4, 50.6, "alarm", "alarm"), e(5, 90, "clear", "info"), e(6, 200, "action", "info")];
  const c = clusterMarkers(log, 10, 110, 1000);
  assert.equal(c.length, 2);
  assert.equal(c[0].tone, "trip");
  assert.equal(c[0].primary.seq, 3);
  assert.deepEqual(c[0].entries.map((x) => x.seq), [2, 3, 4]);
  assert.equal(c[0].x, 400);
  assert.equal(c[1].primary.seq, 5);
});

test("empty window or width yields no markers", () => {
  assert.deepEqual(clusterMarkers([e(1, 5, "action", "info")], 10, 10, 100), []);
  assert.deepEqual(clusterMarkers([e(1, 5, "action", "info")], 0, 10, 0), []);
});

test("markers use the time index: a real RunStore trace whose log times are not monotonic", () => {
  const listeners = new Set();
  const backend = {
    kind: "mock",
    subscribe: (cb) => (listeners.add(cb), () => listeners.delete(cb)),
    send: async () => {},
    eventsSince: async (runId) => ({ runId, events: [], truncated: false }),
    saves: {},
  };
  const store = new RunStore(backend, { now: () => 1 });
  store.connect();
  const onset = (seq) => ({ seq, timeS: 0, kind: "onset", key: `cond:a${seq}`, text: `A${seq}`, severity: "alarm" });
  // First snapshot at t=10 carrying the run's t=0 initial alarms.
  for (const l of listeners) l(fakeSnapshot({ runId: 1, rev: 1, timeS: 10, annHead: 2, annTail: [onset(1), onset(2)] }));
  assert.deepEqual(store.log.entries.map((x) => x.simTimeS), [10, 0, 0], "log keeps seq order");
  assert.deepEqual(store.log.byTime.map((x) => x.seq), [2, 3, 1]);

  const start = clusterMarkers(store.log.byTime, 9, 11, 1000);
  assert.equal(start.length, 1);
  assert.match(start[0].primary.text, /^Run start/);
  // Distant markers stay apart (unsorted input merged them through a negative x delta).
  const all = clusterMarkers(store.log.byTime, 0, 20, 100);
  assert.deepEqual(all.map((c) => [c.x, c.entries.map((x) => x.seq)]), [[0, [2, 3]], [50, [1]]]);
});
