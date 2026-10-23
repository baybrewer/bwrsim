import { test } from "node:test";
import assert from "node:assert/strict";
import { RunStore, UNEXPECTED_RUN_ADOPT_COUNT } from "./runStore.ts";
import { History } from "./history.ts";
import { EventLog } from "./log.ts";
import { fakeSnapshot } from "./testkit.mjs";

const tick = () => new Promise((r) => setImmediate(r));

function ev(seq, timeS = 0, kind = "onset", key = `cond:a${seq}`) {
  return { seq, timeS, kind, key, text: `ALARM ${key} #${seq}`, severity: "warning" };
}
const range = (a, b) => Array.from({ length: Math.max(0, b - a + 1) }, (_, i) => a + i);
/** Snapshot of `runId` at `rev` whose engine has issued events 1..head (tail = last 64). */
function snap(runId, rev, timeS, head = 0, over = {}) {
  const tail = range(Math.max(1, head - 63), head).map((n) => ev(n, timeS));
  return fakeSnapshot({ runId, rev, timeS, annHead: head, annTail: tail, ...over });
}

function fakeBackend() {
  const listeners = new Set();
  const be = {
    kind: "mock",
    sent: [],
    pulls: [],
    head: Infinity,
    pull: (runId, seq) => ({ runId, events: range(seq + 1, 2000).map((n) => ev(n)), truncated: false }),
    publish(s) {
      for (const l of listeners) l(s);
    },
    subscribe(cb) {
      listeners.add(cb);
      return () => listeners.delete(cb);
    },
    async send(cmd) {
      be.sent.push(cmd);
      if (be.reject) throw new Error(be.reject);
    },
    async eventsSince(runId, seq) {
      be.pulls.push([runId, seq]);
      const r = be.pull(runId, seq);
      if (r instanceof Error) throw r;
      return { ...r, events: r.events.filter((e) => e.seq <= be.head) };
    },
    saves: {},
  };
  return be;
}

function setup() {
  const be = fakeBackend();
  let wall = 1000;
  const store = new RunStore(be, { now: () => (wall += 10) });
  store.connect();
  return { be, store };
}

const alarmTexts = (log) => log.entries.filter((e) => e.kind === "alarm").map((e) => e.text);

test("run store: several snapshots before a render are each ingested exactly once", () => {
  const { be, store } = setup();
  const views = [];
  store.subscribe(() => views.push(store.getSnapshot()));
  for (let r = 1; r <= 5; r++) be.publish(snap(1, r, r - 1, r));
  // Re-delivery of the latest snapshot changes nothing.
  be.publish(snap(1, 5, 4, 5));
  assert.equal(store.getSnapshot().snapshot.rev, 5);
  assert.deepEqual(store.history.level(0).t, [0, 1, 2, 3, 4]);
  assert.equal(alarmTexts(store.log).length, 5);
  assert.equal(store.annConsumed, 5);
  assert.ok(views.length >= 5);
  assert.equal(be.pulls.length, 0);
  assert.equal(store.log.entries[0].text, "Run start: HOT STANDBY");
});

test("run store: a delayed older-rev snapshot is dropped; equal rev only refreshes the display", () => {
  const { be, store } = setup();
  be.publish(snap(1, 5, 10));
  be.publish(snap(1, 3, 20));
  assert.equal(store.getSnapshot().snapshot.rev, 5);
  assert.deepEqual(store.history.level(0).t, [10]);
  be.publish(snap(1, 5, 10, 0, { effectiveSpeed: 0 }));
  assert.equal(store.getSnapshot().snapshot.effectiveSpeed, 0);
  assert.deepEqual(store.history.level(0).t, [10]);
});

test("run store: a tail that skips ahead is pulled first; overlapping tail and pull are logged once", async () => {
  const { be, store } = setup();
  be.publish(snap(1, 1, 0, 2));
  be.head = 100;
  be.publish(snap(1, 2, 1, 100)); // tail 37..100, consumed 2 -> pull 3..100
  be.publish(snap(1, 3, 2, 101)); // queued behind the pull
  be.head = 101;
  await tick();
  assert.deepEqual(be.pulls, [[1, 2]]);
  const texts = alarmTexts(store.log);
  assert.equal(texts.length, 101);
  assert.equal(new Set(texts).size, 101);
  assert.equal(store.annConsumed, 101);
  assert.deepEqual(store.history.level(0).t, [0, 1, 2]);
});

test("run store: a truncated pull logs a gap note", async () => {
  const { be, store } = setup();
  be.publish(snap(1, 1, 0, 0));
  be.pull = (runId) => ({ runId, events: range(1100, 1200).map((n) => ev(n)), truncated: true });
  be.publish(snap(1, 2, 1, 1200));
  await tick();
  assert.ok(store.log.entries.some((e) => e.kind === "system" && /gap: 1099 events lost/.test(e.text)));
});

function savedRecord() {
  const h = new History();
  h.ingest(fakeSnapshot({ timeS: 0 }));
  h.ingest(fakeSnapshot({ timeS: 50 }));
  const log = EventLog.fromEntries([], 7, 0);
  log.logSystem("Run start: HOT STANDBY", 0, 1);
  log.logAnnEvents(7, [ev(1), ev(2), ev(3)], 2);
  return JSON.parse(JSON.stringify({ version: 1, runScenario: "hotStandby", history: h.toJSON(), log: log.toJSON(), annConsumed: 3 }));
}

test("run store: a pull answered RunChanged (load committed meanwhile) logs nothing into either run", async () => {
  const { be, store } = setup();
  be.publish(snap(1, 1, 0, 2));
  const oldLog = store.log;
  const oldCount = oldLog.entries.length;
  let commit;
  be.saves = {
    loadPrepare: async () => ({ token: "t", ui: savedRecord(), snapshot: snap(2, 40, 50, 3, { paused: true }) }),
    loadCommit: () => new Promise((resolve) => (commit = resolve)),
    loadAbort: async () => {},
  };
  const loading = store.load("s_0123456789abcdef", "before trip");
  await tick();
  // A run-1 snapshot whose tail skips ahead; by the time the pull runs, run 1 is gone.
  be.pull = () => new Error("RunChanged");
  be.publish(fakeSnapshot({ runId: 1, rev: 9, timeS: 3, annHead: 140, annTail: range(77, 140).map((n) => ev(n, 3, "onset", `cond:old${n}`)) }));
  // Restored run: continues the saved sequence (seq 4 is new).
  const restored = fakeSnapshot({ runId: 2, rev: 40, timeS: 50, paused: true, annHead: 4, annTail: [ev(1), ev(2), ev(3), ev(4, 50)] });
  be.publish(restored);
  await tick();
  commit(restored);
  await loading;
  assert.equal(oldLog.entries.length, oldCount, "old run log untouched");
  assert.equal(store.getSnapshot().runId, 2);
  assert.ok(!store.log.entries.some((e) => e.text.includes("old")), "no old-run events in the restored log");
  assert.equal(alarmTexts(store.log).length, 4, "3 saved + 1 new");
  assert.ok(store.log.entries.some((e) => e.text === 'Loaded save "before trip"'));
  assert.deepEqual(store.history.level(0).t, [0, 50]);
  // Stale snapshots of the replaced run are dropped after the load.
  be.publish(snap(1, 999, 99, 0));
  assert.equal(store.getSnapshot().snapshot.runId, 2);
  assert.equal(store.getSnapshot().snapshot.timeS, 50);
});

test("run store: load installs from the commit result when the subscription has not published yet", async () => {
  const { be, store } = setup();
  be.publish(snap(1, 1, 0, 0));
  const restored = snap(5, 40, 50, 3, { paused: true });
  be.saves = {
    loadPrepare: async () => ({ token: "t", ui: savedRecord(), snapshot: restored }),
    loadCommit: async () => restored,
  };
  await store.load("s_0123456789abcdef", "x");
  assert.equal(store.getSnapshot().runId, 5);
  assert.equal(store.annConsumed, 3);
});

test("run store: an invalid run record aborts the load and keeps the current run", async () => {
  const { be, store } = setup();
  be.publish(snap(1, 1, 0, 0));
  const aborted = [];
  be.saves = {
    loadPrepare: async () => ({ token: "t", ui: { version: 9 }, snapshot: snap(2, 1, 0) }),
    loadCommit: async () => assert.fail("must not commit"),
    loadAbort: async (t) => void aborted.push(t),
  };
  await assert.rejects(store.load("s_0123456789abcdef", "x"), /unsupported version/);
  assert.deepEqual(aborted, ["t"]);
  be.publish(snap(2, 1, 0));
  assert.equal(store.getSnapshot().runId, 1, "unexpected run dropped");
});

test("run store: repeated scenario load at t=0 resets each time; unexpected runs are dropped", async () => {
  const { be, store } = setup();
  for (let t = 0; t <= 10; t++) be.publish(snap(1, t + 1, t, 0));
  await store.send({ type: "loadScenario", name: "fullPower" });
  be.publish(snap(2, 1, 0, 2, { scenario: "fullPower" }));
  assert.deepEqual(store.history.level(0).t, [0]);
  assert.equal(store.log.entries[0].text, "Scenario FULL POWER loaded");
  assert.equal(store.getSnapshot().runScenario, "fullPower");
  await store.send({ type: "loadScenario", name: "fullPower" });
  be.publish(snap(3, 1, 0, 2, { scenario: "fullPower" }));
  assert.equal(store.getSnapshot().runId, 3);
  assert.deepEqual(store.history.level(0).t, [0]);
  assert.equal(alarmTexts(store.log).length, 2);
  assert.ok(!store.log.entries.some((e) => e.kind === "action"), "the load itself is not an action in either log");
  be.publish(snap(4, 1, 0));
  assert.equal(store.getSnapshot().runId, 3);
});

test("run store: an unexpected newer run is adopted after a sustained stream", () => {
  const { be, store } = setup();
  be.publish(snap(1, 1, 0));
  for (let i = 1; i < UNEXPECTED_RUN_ADOPT_COUNT; i++) be.publish(snap(9, i, i));
  assert.equal(store.getSnapshot().runId, 1);
  be.publish(snap(9, 99, 99));
  assert.equal(store.getSnapshot().runId, 9);
  assert.equal(store.log.entries[0].severity, "warning");
});

test("run store: a scenario switch after seq 500 logs the new run's first events", async () => {
  const { be, store } = setup();
  be.publish(snap(1, 1, 0, 0));
  be.head = 500;
  be.publish(snap(1, 2, 5, 500));
  await tick();
  assert.equal(store.annConsumed, 500);
  await store.send({ type: "loadScenario", name: "coldShutdown" });
  be.publish(snap(2, 1, 0, 3, { scenario: "coldShutdown" }));
  assert.equal(alarmTexts(store.log).length, 3);
  assert.equal(store.annConsumed, 3);
});

test("run store: a save at t=0.4 after a sample at t=0 includes the checkpoint; older snapshots are dropped after it", async () => {
  const { be, store } = setup();
  be.publish(snap(1, 1, 0, 1));
  be.publish(snap(1, 2, 0.4, 1));
  assert.deepEqual(store.history.level(0).t, [0]);
  let written;
  be.saves = {
    prepare: async (runId, since) => {
      assert.deepEqual([runId, since], [1, 1]);
      return { token: "1:3", snapshot: snap(1, 3, 0.4, 2, { paused: true }), events: [ev(2, 0.4)], wasPaused: false };
    },
    write: async (token, id, name, ui) => {
      written = { token, id, name, ui: JSON.parse(JSON.stringify(ui)) };
      return { id: "s_0123456789abcdef", name: name.trim(), savedAtMs: 1, simTimeS: 0.4, scenario: "hotStandby", runSummary: {}, loadable: true, error: null };
    },
  };
  const session = await store.beginSave();
  be.publish(snap(1, 2, 0.4, 1)); // delayed pre-checkpoint publish
  assert.equal(store.getSnapshot().snapshot.rev, 3);
  await store.commitSave(session, null, "  test  ");
  assert.equal(written.token, "1:3");
  assert.deepEqual(written.ui.history.levels[0].t, [0, 0.4]);
  assert.equal(written.ui.annConsumed, 2);
  assert.ok(store.log.entries.some((e) => e.text === 'Saved "test"'));
  await store.finishSave(session);
  assert.deepEqual(be.sent.at(-1), { type: "setPaused", paused: false });
});

test("run store: a checkpoint whose events skip ahead logs a gap; a paused sim is not resumed", async () => {
  const { be, store } = setup();
  be.publish(snap(1, 1, 0, 1));
  be.saves = { prepare: async () => ({ token: "x", snapshot: snap(1, 3, 1, 90, { paused: true, annTail: [ev(90)] }), events: [ev(80), ev(90)], wasPaused: true }) };
  const session = await store.beginSave();
  assert.ok(store.log.entries.some((e) => e.kind === "system" && /gap/.test(e.text)));
  await store.finishSave(session);
  assert.equal(be.sent.length, 0);
});

test("run store: actions are logged when sent; rejections add a note and rethrow", async () => {
  const { be, store } = setup();
  be.publish(snap(1, 1, 0));
  await store.send({ type: "setPressureSetpoint", mpa: 7.1 });
  be.reject = "ROD BLOCK: scram not reset";
  await assert.rejects(store.send({ type: "rodStop" }), /ROD BLOCK/);
  const tail = store.log.entries.slice(-3).map((e) => [e.kind, e.text]);
  assert.deepEqual(tail, [
    ["action", "Pressure setpoint 7.10 MPa"],
    ["action", "Rod motion stop"],
    ["system", "Command rejected: ROD BLOCK: scram not reset"],
  ]);
});

test("serial queue: jobs run in order and a failure does not block the next", async () => {
  const { createSerialQueue } = await import("./serialQueue.ts");
  const q = createSerialQueue();
  const order = [];
  const slow = q(async () => {
    await tick();
    order.push(1);
  });
  const bad = q(() => {
    order.push(2);
    throw new Error("x");
  });
  const fast = q(() => order.push(3));
  await slow;
  await assert.rejects(bad);
  await fast;
  assert.deepEqual(order, [1, 2, 3]);
});

test("run store: actions sent behind a scenario load are logged in the new run; a failed load keeps them in the current one", async () => {
  const { be, store } = setup();
  be.publish(snap(1, 1, 5));
  const plain = be.send;
  be.send = async (cmd) => {
    await plain(cmd);
    if (cmd.type === "rodStop") throw new Error("ROD BLOCK");
  };
  // START, then commands before the new run's first (10 Hz) publish: they execute in the new run.
  await store.send({ type: "loadScenario", name: "fullPower" });
  await store.send({ type: "setSpeed", speed: 10 });
  await assert.rejects(store.send({ type: "rodStop" }), /ROD BLOCK/);
  assert.ok(!store.log.entries.some((e) => e.kind === "action" || /rejected/.test(e.text)), "nothing in the old run");
  be.publish(snap(2, 1, 0, 0, { scenario: "fullPower" }));
  assert.deepEqual(
    store.log.entries.map((e) => [e.kind, e.text, e.simTimeS]),
    [
      ["system", "Scenario FULL POWER loaded", 0],
      ["action", "Sim speed 10x", 0],
      ["action", "Rod motion stop", 0],
      ["system", "Command rejected: ROD BLOCK", 0],
    ],
  );

  // A rejected scenario load: what was queued behind it ran in the current run.
  let fail;
  be.send = (cmd) => (cmd.type === "loadScenario" ? new Promise((_, reject) => (fail = reject)) : plain(cmd));
  const failing = store.send({ type: "loadScenario", name: "coldShutdown" });
  await store.send({ type: "manualScram" });
  assert.ok(!store.log.entries.some((e) => e.text === "Manual scram"));
  fail(new Error("load refused"));
  await assert.rejects(failing, /load refused/);
  assert.deepEqual(store.log.entries.slice(-2).map((e) => [e.kind, e.text]), [
    ["action", "Manual scram"],
    ["system", "Command rejected: load refused"],
  ]);
  be.publish(snap(3, 1, 0));
  assert.equal(store.getSnapshot().runId, 2, "no scenario load is expected any more");
});

test("run store: two scenario loads applied before one publish install the latest; no ticket is left buffering", async () => {
  const { be, store } = setup();
  be.publish(snap(1, 1, 5));
  // Both loads complete between two desktop publishes; only the second run ever publishes.
  await store.send({ type: "loadScenario", name: "fullPower" });
  await store.send({ type: "setSpeed", speed: 10 });
  await store.send({ type: "loadScenario", name: "coldShutdown" });
  await store.send({ type: "manualScram" });
  be.publish(snap(3, 1, 0, 0, { scenario: "coldShutdown" }));
  const texts = store.log.entries.map((e) => [e.kind, e.text]);
  assert.deepEqual(texts[0], ["system", "Scenario COLD SHUTDOWN loaded"]);
  assert.ok(texts.some(([k, x]) => k === "system" && /1 earlier scenario load\(s\) replaced before display; 1 action/.test(x)));
  assert.ok(texts.some(([k, x]) => k === "action" && x === "Manual scram"), "the final run's action is logged");
  assert.ok(!texts.some(([, x]) => x === "Sim speed 10x"), "the superseded run's action is not put in this run");
  // Nothing is left buffering: a later action is logged straight away.
  await store.send({ type: "rodStop" });
  assert.equal(store.log.entries.at(-1).text, "Rod motion stop");
});

test("run store: a staged snapshot the store cannot ingest aborts the load before commit; the run is intact", async () => {
  const { be, store } = setup();
  for (let t = 0; t <= 3; t++) be.publish(snap(1, t + 1, t, 1));
  const before = { runId: store.getSnapshot().runId, record: JSON.stringify(store.buildRecord()) };
  const damaged = snap(2, 40, 50, 3, { paused: true });
  delete damaged.neutronics;
  const aborted = [];
  be.saves = {
    loadPrepare: async () => ({ token: "t", ui: savedRecord(), snapshot: damaged }),
    loadCommit: async () => assert.fail("must not commit"),
    loadAbort: async (t) => void aborted.push(t),
  };
  await assert.rejects(store.load("s_0123456789abcdef", "x"));
  assert.deepEqual(aborted, ["t"]);
  // Even if such a snapshot is published, it neither installs nor consumes anything.
  assert.throws(() => be.publish(damaged));
  assert.equal(store.getSnapshot().runId, before.runId);
  assert.equal(JSON.stringify(store.buildRecord()), before.record);
});
