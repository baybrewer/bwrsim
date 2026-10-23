// Browser mock parity (plan C5): runs the real MockSim and mock save store under Node.
import { test } from "node:test";
import assert from "node:assert/strict";
import { MockSim, MOCK_STATE_VERSION, parseMockState } from "../mock.ts";
import { RunStore } from "./runStore.ts";
import { createMockSaveStore, memoryStorage, MOCK_SAVES_KEY, STATE_CHANGED } from "../mockSaves.ts";

/** Engine state without the fields a restore is documented to change or that follow wall time. */
function comparable(sim) {
  const st = sim.exportState();
  delete st.snapshot.runId;
  delete st.snapshot.effectiveSpeed;
  return JSON.stringify(st);
}

function run(sim, ticks) {
  for (let i = 0; i < ticks; i++) sim.tick(0.1);
}

test("mock: JSON export/import continues identically, including acknowledged alarms", () => {
  const a = new MockSim();
  a.command({ type: "loadScenario", name: "fullPower" });
  a.command({ type: "setSpeed", speed: 10 });
  run(a, 30);
  a.command({ type: "tripTurbine" });
  run(a, 20);
  a.command({ type: "ackAlarms" });
  run(a, 20);
  assert.ok(a.snapshot().alarms.some((al) => al.acked), "has acknowledged alarms");
  a.command({ type: "setPaused", paused: true });

  const b = new MockSim();
  const restored = b.importState(JSON.parse(JSON.stringify(a.exportState())));
  assert.notEqual(restored.runId, a.runId, "restore is a new run");
  assert.equal(restored.paused, true);
  assert.equal(restored.rev, a.rev);
  assert.deepEqual(restored.alarms, a.snapshot().alarms);
  assert.equal(comparable(b), comparable(a));

  for (const sim of [a, b]) sim.command({ type: "setPaused", paused: false });
  for (let i = 0; i < 300; i++) {
    a.tick(0.1);
    b.tick(0.1);
    if (i === 100) for (const sim of [a, b]) sim.command({ type: "ackAlarms" });
  }
  assert.equal(comparable(b), comparable(a));
});

test("mock: run identity, init boundary and annunciator events follow engine semantics", () => {
  const sim = new MockSim();
  const s0 = sim.snapshot();
  assert.equal(s0.scenario, "hotStandby");
  assert.equal(s0.rev, 0);
  assert.equal(s0.annHead, s0.alarms.length);
  assert.ok(s0.annTail.every((e) => e.kind === "onset" && e.timeS === 0 && e.key.startsWith("cond:")));

  sim.command({ type: "manualScram" });
  const s1 = sim.snapshot();
  assert.equal(s1.rev, 1);
  assert.ok(s1.annTail.some((e) => e.kind === "onset" && e.key === "cond:scram"));
  assert.throws(() => sim.eventsSince(s1.runId + 1, 0), /^Error: RunChanged$/);
  const since = sim.eventsSince(s1.runId, 0);
  assert.equal(since.events.length, s1.annHead);
  assert.equal(since.truncated, false);

  // A paused tick changes nothing the engine would sequence.
  sim.command({ type: "setPaused", paused: true });
  const rev = sim.rev;
  sim.tick(0.1);
  assert.equal(sim.rev, rev);

  sim.command({ type: "loadScenario", name: "coldShutdown" });
  const s2 = sim.snapshot();
  assert.ok(s2.runId > s1.runId);
  assert.equal(s2.scenario, "coldShutdown");
  assert.equal(s2.rev, 1);
  assert.equal(s2.annTail[0]?.seq, 1);
});

function makeStore(sim, storage = memoryStorage()) {
  let t = 1_000;
  return { saves: createMockSaveStore(sim, storage, () => (t += 1000)), storage };
}

const record = { version: 1, runScenario: "hotStandby", history: { channels: [], levels: [] }, log: [], annConsumed: 0 };

async function save(saves, sim, id, name) {
  const p = await saves.prepare(sim.runId, 0);
  return saves.write(p.token, id, name, record);
}

test("mock saves: slots, names, ids and tokens follow the host rules", async () => {
  const sim = new MockSim();
  const { saves } = makeStore(sim);
  const prep = await saves.prepare(sim.runId, 0);
  assert.equal(prep.wasPaused, false);
  assert.equal(prep.snapshot.paused, true);
  assert.equal(prep.events.length, prep.snapshot.annHead);
  await assert.rejects(saves.prepare(sim.runId + 99, 0), /^Error: RunChanged$/);

  const first = await saves.write(prep.token, null, "  Before trip ", record);
  assert.match(first.id, /^s_[0-9a-f]{16}$/);
  assert.equal(first.name, "Before trip");
  assert.equal(first.loadable, true);
  await assert.rejects(save(saves, sim, null, "BEFORE TRIP"), /a save named "BEFORE TRIP" already exists/);
  await assert.rejects(save(saves, sim, null, "   "), /name is required/);
  await assert.rejects(save(saves, sim, null, "x".repeat(41)), /at most 40/);
  await assert.rejects(save(saves, sim, "s_0000000000000000", "ghost"), /save not found/);
  await assert.rejects(save(saves, sim, "../x", "ghost"), /invalid save id/);
  // Overwrite may keep its own name.
  const over = await save(saves, sim, first.id, "before trip");
  assert.equal(over.id, first.id);

  // Any mutation between prepare and write is detected.
  const p2 = await saves.prepare(sim.runId, 0);
  sim.command({ type: "setSpeed", speed: 2 });
  await assert.rejects(saves.write(p2.token, null, "late", record), new RegExp(STATE_CHANGED));
  await assert.rejects(saves.write((await saves.prepare(sim.runId, 0)).token, null, "bad", [1]), /must be a JSON object/);

  for (let i = 2; i <= 10; i++) await save(saves, sim, null, `slot ${i}`);
  const list = await saves.list();
  assert.equal(list.max, 10);
  assert.equal(list.saves.length, 10);
  assert.ok(list.saves.every((m, i, all) => i === 0 || all[i - 1].savedAtMs >= m.savedAtMs), "newest first");
  await assert.rejects(save(saves, sim, null, "eleventh"), /all 10 save slots are in use/);
  await saves.remove(first.id);
  await assert.rejects(saves.remove(first.id), /save not found/);
  assert.equal((await saves.list()).saves.length, 9);
});

test("mock saves: load prepare/commit restores the run; version and damage checks; stale tokens", async () => {
  const sim = new MockSim();
  const { saves, storage } = makeStore(sim);
  sim.command({ type: "loadScenario", name: "fullPower" });
  run(sim, 10);
  const meta = await save(saves, sim, null, "full");
  const savedTime = sim.snapshot().timeS;
  run(sim, 5);
  sim.command({ type: "setPaused", paused: false });
  run(sim, 20);

  const p1 = await saves.loadPrepare(meta.id);
  const p2 = await saves.loadPrepare(meta.id);
  await assert.rejects(saves.loadCommit(p1.token), /no staged load for this token/);
  await saves.loadAbort(p1.token); // stale abort: no-op
  assert.deepEqual(p2.ui, record);
  const restored = await saves.loadCommit(p2.token);
  assert.equal(restored.runId, p2.snapshot.runId, "prepare announces the restored run id");
  assert.equal(sim.runId, restored.runId);
  assert.equal(restored.paused, true);
  assert.equal(restored.timeS, savedTime);
  await assert.rejects(saves.loadCommit(p2.token), /no staged load/);

  // Wrong engine version: real meta, not loadable. Damaged slot: placeholder meta, still counted.
  const raw = JSON.parse(storage.getItem(MOCK_SAVES_KEY));
  raw.slots[meta.id].engineVersion = MOCK_STATE_VERSION + 1;
  raw.slots.s_00000000000000ff = { junk: true };
  storage.setItem(MOCK_SAVES_KEY, JSON.stringify(raw));
  const list = await saves.list();
  const wrong = list.saves.find((m) => m.id === meta.id);
  assert.equal(wrong.name, "full");
  assert.equal(wrong.loadable, false);
  assert.match(wrong.error, /incompatible/);
  const broken = list.saves.find((m) => m.id === "s_00000000000000ff");
  assert.equal(broken.loadable, false);
  assert.equal(list.saves.length, 2);
  await assert.rejects(saves.loadPrepare(meta.id), /incompatible/);
  await assert.rejects(saves.loadPrepare("s_00000000000000ff"), /unreadable/);
  await saves.remove("s_00000000000000ff");
});

test("mock saves: a storage quota failure fails the write with a clear message", async () => {
  const sim = new MockSim();
  const full = memoryStorage();
  const storage = {
    getItem: (k) => full.getItem(k),
    setItem: () => {
      const e = new Error("exceeded the quota");
      e.name = "QuotaExceededError";
      throw e;
    },
  };
  const { saves } = makeStore(sim, storage);
  await assert.rejects(save(saves, sim, null, "big"), /browser storage is full .*delete a save/);
  assert.equal((await saves.list()).saves.length, 0);
});

const annOf = (sim, key) => sim.eventsSince(sim.runId, 0).events.filter((e) => e.key === key);

test("mock: every substep is an engine step outcome; a transient between publishes is sequenced at its own times", () => {
  const sim = new MockSim();
  sim.command({ type: "loadScenario", name: "fullPower" });
  const published = [];
  const off = sim.subscribe((s) => published.push(s));
  sim.stop(); // drive ticks by hand
  const t0 = sim.snapshot().timeS;
  const rev0 = sim.rev;
  // Pressure above 7.5 MPa in the first of two 0.05 s substeps only.
  let n = 0;
  sim.step = function (dt) {
    this.s.timeS += dt;
    this.s.vessel.pressureMpa = n++ === 0 ? 7.6 : 7.2;
  };
  published.length = 0;
  sim.tick(0.1);
  assert.equal(published.length, 1, "one publish per timer tick");
  assert.equal(sim.rev, rev0 + 2, "one revision per substep");
  const ev = annOf(sim, "cond:pressHi");
  assert.deepEqual(ev.map((e) => e.kind), ["onset", "clear"]);
  assert.ok(Math.abs(ev[0].timeS - (t0 + 0.05)) < 1e-9 && Math.abs(ev[1].timeS - (t0 + 0.1)) < 1e-9, "substep times");

  // A model halt mid-tick: its outcome is recorded, later substeps do not run.
  sim.command({ type: "setSpeed", speed: 10 });
  n = 0;
  sim.step = function (dt) {
    this.s.timeS += dt;
    if (++n === 3) {
      this.s.modelError = "test halt";
      this.s.paused = true;
    }
  };
  const before = sim.rev;
  sim.tick(0.1);
  assert.equal(n, 3);
  assert.equal(sim.rev, before + 3);
  const halt = annOf(sim, "cond:model");
  assert.equal(halt.length, 1);
  assert.equal(halt[0].kind, "onset");
  assert.equal(sim.snapshot().annHead, halt[0].seq, "published head includes the halting substep");
  off();
});

test("mock: alarms follow active conditions regardless of ACK; onset/clear/onset without ACK; ACK resets on recurrence", () => {
  const sim = new MockSim(); // hot standby: pressurized, head vent closed
  const vent = (open) => sim.command({ type: "setHeadVent", open });
  const listed = () => sim.snapshot().alarms.find((a) => a.id === "vent");
  vent(true);
  vent(false);
  assert.equal(listed(), undefined, "a cleared, unacknowledged alarm drops out");
  vent(true);
  assert.deepEqual(annOf(sim, "cond:vent").map((e) => e.kind), ["onset", "clear", "onset"]);
  sim.command({ type: "ackAlarms" });
  assert.equal(listed().acked, true);
  vent(false);
  vent(true);
  assert.equal(listed().acked, false, "a recurrence flashes again");
  assert.deepEqual(annOf(sim, "cond:vent").map((e) => e.kind), ["onset", "clear", "onset", "clear", "onset"]);
  // Acknowledging never stamps a clear: only a condition change does.
  const head = sim.snapshot().annHead;
  sim.command({ type: "ackAlarms" });
  assert.equal(sim.snapshot().annHead, head);
});

test("mock state validation rejects damaged engines: snapshot fields, nested enums, hidden state, annunciator ring", () => {
  const sim = new MockSim();
  sim.command({ type: "manualScram" });
  const good = () => JSON.parse(JSON.stringify(sim.exportState()));
  assert.ok(!(parseMockState(good()) instanceof Error));
  const damage = [
    [(st) => delete st.snapshot.neutronics, /neutronics/],
    [(st) => (st.snapshot.neutronics.reactivity.xenon = "x"), /neutronics\.reactivity\.xenon/],
    [(st) => (st.snapshot.modeSwitch = "bogus"), /modeSwitch/],
    [(st) => (st.snapshot.feedwater.levelControlMode = "auto"), /levelControlMode/],
    [(st) => (st.snapshot.neutronics.periodS = "fast"), /periodS/],
    [(st) => (st.snapshot.steam.msivPos = [1]), /msivPos/],
    [(st) => delete st.snapshot.ic[1].poolEmpty, /ic\.poolEmpty/],
    [(st) => (st.snapshot.alarms[0].severity = "loud"), /alarms/],
    [(st) => (st.snapshot.annTail[0].kind = "boom"), /annTail/],
    [(st) => delete st.hidden.omega, /hidden\.omega/],
    [(st) => (st.hidden.alarmState[0][1].acked = "yes"), /alarm state/],
    [(st) => (st.hidden.annPrev[0][1] = null), /baseline/],
    [(st) => st.hidden.annRing.reverse(), /ring/],
    [(st) => (st.hidden.annSeq = 1), /ring/],
  ];
  for (const [hurt, msg] of damage) {
    const st = good();
    hurt(st);
    const r = parseMockState(st);
    assert.ok(r instanceof Error, `rejected: ${msg}`);
    assert.match(r.message, msg);
  }
});

test("mock saves: a damaged engine save is rejected before commit; the run store keeps the original run", async () => {
  const sim = new MockSim();
  const { saves, storage } = makeStore(sim);
  const backend = {
    kind: "mock",
    subscribe: (cb) => sim.subscribe(cb),
    send: async (cmd) => sim.command(cmd),
    eventsSince: async (runId, seq) => sim.eventsSince(runId, seq),
    saves,
  };
  const store = new RunStore(backend);
  const off = store.connect();
  sim.stop(); // drive ticks by hand
  try {
    run(sim, 30);
    const session = await store.beginSave();
    const meta = await store.commitSave(session, null, "good");
    await store.finishSave(session);
    run(sim, 30);
    const raw = JSON.parse(storage.getItem(MOCK_SAVES_KEY));
    delete raw.slots[meta.id].engine.snapshot.neutronics;
    storage.setItem(MOCK_SAVES_KEY, JSON.stringify(raw));

    const view = store.getSnapshot();
    const before = { runId: view.runId, simRun: sim.runId, timeS: view.snapshot.timeS, record: JSON.stringify(store.buildRecord()) };
    await assert.rejects(store.load(meta.id, "good"), /damaged save file: .*neutronics/);
    assert.equal(sim.runId, before.simRun, "active sim untouched");
    assert.equal(store.getSnapshot().runId, before.runId);
    assert.equal(store.getSnapshot().snapshot.timeS, before.timeS);
    assert.equal(JSON.stringify(store.buildRecord()), before.record, "history and log intact");
    run(sim, 10);
    assert.ok(store.getSnapshot().snapshot.timeS > before.timeS, "the original run keeps running");
  } finally {
    off();
  }
});
