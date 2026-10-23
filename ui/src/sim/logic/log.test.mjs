import { test } from "node:test";
import assert from "node:assert/strict";
import { EventLog, describeCommand, summarizeGroup, validateLogEntries } from "./log.ts";
import { fakeSnapshot } from "./testkit.mjs";

const snap = fakeSnapshot({ timeS: 12 });

test("describeCommand words commands like the panels, with units", () => {
  assert.equal(describeCommand({ type: "setPressureSetpoint", mpa: 7.1 }, snap), "Pressure setpoint 7.10 MPa");
  assert.equal(describeCommand({ type: "rodSetTarget", indices: [0], target: 48 }, snap), "Rod 02-35 -> 48 notches");
  assert.equal(describeCommand({ type: "rodSetTarget", indices: [2, 1], target: 12.5 }, snap), "Rods 06-35, 10-31 -> 12.5 notches");
  assert.equal(describeCommand({ type: "rodSetTarget", indices: [0, 1, 2], target: 0 }, snap), "All 3 rods -> 0 notches");
  assert.equal(describeCommand({ type: "setFeedPump", index: 1, on: false }, snap), "Feed pump B stop");
  assert.equal(describeCommand({ type: "setMsiv", line: 0, open: false }, snap), "MSIV A close");
  assert.equal(describeCommand({ type: "setFcvManual", pos: 0.45 }, snap), "FCV manual 45 %");
  assert.equal(describeCommand({ type: "loadScenario", name: "fullPower" }, snap), "Load scenario FULL POWER");
  assert.equal(describeCommand({ type: "setModeSwitch", mode: "startup" }, snap), "Mode switch -> STARTUP");
  assert.equal(describeCommand({ type: "selectRod", index: 1 }, null), "Select rod #2");
});

test("describeCommand covers every command variant", () => {
  const all = [
    { type: "setPaused", paused: true }, { type: "setSpeed", speed: 10 }, { type: "loadScenario", name: "coldShutdown" },
    { type: "manualScram" }, { type: "resetScram" }, { type: "setModeSwitch", mode: "run" }, { type: "selectRod", index: null },
    { type: "rodSetTarget", indices: [0], target: 4 }, { type: "rodStop" }, { type: "setIrmRange", range: 3 },
    { type: "setHeadVent", open: true }, { type: "setMsiv", line: 1, open: true }, { type: "setPressureSetpoint", mpa: 7 },
    { type: "setBypass", auto: false, pos: 0.3 }, { type: "setSpeedSetpoint", rpm: 1800 }, { type: "resetTurbine" },
    { type: "tripTurbine" }, { type: "closeBreaker" }, { type: "openBreaker" }, { type: "setLoadLimit", pct: 100 },
    { type: "setCwPump", index: 0, on: true }, { type: "setVacuumPump", on: false }, { type: "setFeedPump", index: 0, on: true },
    { type: "setCondensatePump", index: 1, on: true }, { type: "setLevelControl", mode: "singleElement" },
    { type: "setFcvManual", pos: 0.5 }, { type: "setLevelSetpoint", cm: -10 }, { type: "setHeaters", inService: false },
    { type: "setRwcuReject", kgs: 2.5 }, { type: "setIcTrain", index: 2, open: true }, { type: "ackAlarms" },
  ];
  const texts = all.map((c) => describeCommand(c, snap));
  assert.ok(texts.every((t) => typeof t === "string" && t.length > 0 && !t.includes("undefined")));
  assert.equal(new Set(texts).size, texts.length);
  assert.throws(() => describeCommand({ type: "noSuchCommand" }, snap), /undescribed command/);
});

test("consecutive same-type setpoint actions within 2 s share a group; every action is kept", () => {
  const log = new EventLog();
  log.logAction({ type: "setPressureSetpoint", mpa: 7.0 }, snap, 1000);
  log.logAction({ type: "setPressureSetpoint", mpa: 7.1 }, snap, 1700);
  log.logAction({ type: "setPressureSetpoint", mpa: 7.0 }, snap, 2400);
  log.logAction({ type: "setPressureSetpoint", mpa: 6.9 }, snap, 5000); // > 2 s later
  log.logAction({ type: "setLoadLimit", pct: 90 }, snap, 5100); // other knob
  log.logAction({ type: "setPressureSetpoint", mpa: 6.8 }, snap, 5200); // not consecutive with 6.9
  const e = log.entries;
  assert.equal(e.length, 6);
  assert.deepEqual(e.map((x) => x.group), [1, 1, 1, undefined, undefined, undefined]);
  assert.equal(summarizeGroup(e.slice(0, 3)), "Pressure setpoint 7.00 -> 7.10 -> 7.00 MPa, 3 actions, 1.4 s");
});

test("rod sweep up and back keeps every action in one group; other rods break it", () => {
  const log = new EventLog();
  const targets = [24, 26, 28, 26, 24];
  targets.forEach((target, i) => log.logAction({ type: "rodSetTarget", indices: [1], target }, snap, i * 300));
  log.logAction({ type: "rodSetTarget", indices: [2], target: 24 }, snap, 1600);
  const e = log.entries;
  assert.equal(e.length, 6);
  assert.ok(e.slice(0, 5).every((x) => x.group === e[0].seq));
  assert.equal(e[5].group, undefined);
  assert.equal(summarizeGroup(e.slice(0, 5)), "Rod 06-35 -> 24 -> 26 -> 28 -> 26 -> 24 notches, 5 actions, 1.2 s");
  // Non-setpoint commands never group.
  log.logAction({ type: "ackAlarms" }, snap, 1700);
  log.logAction({ type: "ackAlarms" }, snap, 1800);
  assert.equal(log.entries.at(-1).group, undefined);
});

const ev = (seq, kind, text = `alarm ${seq}`, severity = "alarm") => ({ seq, timeS: seq * 0.5, kind, key: `cond:a${seq}`, text, severity });

test("annunciator events map to alarm/clear/changed with engine severity, deduplicated across overlapping batches", () => {
  const log = new EventLog();
  log.bindRun(7, 0);
  assert.equal(log.logAnnEvents(7, [ev(1, "onset"), ev(2, "onset", "Low level", "trip"), ev(3, "clear")], 100), 3);
  assert.equal(log.logAnnEvents(7, [ev(2, "onset", "Low level", "trip"), ev(3, "clear"), ev(4, "changed", "Reset blocked: rods out", "warning")], 200), 1);
  assert.equal(log.logAnnEvents(8, [ev(5, "onset")], 300), 0, "other run ignored");
  assert.equal(log.logAnnEvents(7, [ev(4, "changed"), ev(4, "changed")], 300), 0);
  assert.deepEqual(
    log.entries.map((e) => [e.kind, e.severity, e.text, e.simTimeS]),
    [
      ["alarm", "alarm", "alarm 1", 0.5],
      ["alarm", "trip", "Low level", 1],
      ["clear", "alarm", "alarm 3", 1.5],
      ["changed", "warning", "Reset blocked: rods out", 2],
    ],
  );
  assert.equal(log.annConsumed, 4);
});

test("a restored run keeps its annunciator cursor (sequence continues under a new run id)", () => {
  const saved = new EventLog();
  saved.bindRun(3, 0);
  saved.logAnnEvents(3, [ev(1, "onset"), ev(2, "onset")], 0);
  const restored = EventLog.fromEntries(saved.toJSON(), 9, saved.annConsumed);
  assert.equal(restored.logAnnEvents(9, [ev(1, "onset"), ev(2, "onset"), ev(3, "clear")], 10), 1);
  assert.deepEqual(restored.entries.map((e) => e.seq), [1, 2, 3]);
});

test("truncated pull logs one gap entry with the number of lost events", () => {
  const log = new EventLog();
  log.bindRun(1, 10);
  assert.equal(log.logAnnEvents(1, [ev(40, "onset"), ev(41, "clear")], 0, true), 3);
  assert.deepEqual(log.entries.map((e) => [e.kind, e.severity]), [["system", "warning"], ["alarm", "alarm"], ["clear", "alarm"]]);
  assert.match(log.entries[0].text, /29 events lost/);
});

test("scram onset and model errors are logged once as trips; baseline seeding logs nothing", () => {
  const log = new EventLog();
  log.seedBaseline(fakeSnapshot({ scrammed: true, scramReason: "Old" }));
  log.observeSnapshot(fakeSnapshot({ scrammed: true }), 0);
  log.observeSnapshot(fakeSnapshot({ scrammed: false }), 0);
  log.observeSnapshot(fakeSnapshot({ timeS: 5, scrammed: true, scramReason: "High flux" }), 10);
  log.observeSnapshot(fakeSnapshot({ timeS: 6, scrammed: true, scramReason: "High flux" }), 20);
  log.observeSnapshot(fakeSnapshot({ timeS: 7, scrammed: true, modelError: "Pressure out of range" }), 30);
  log.observeSnapshot(fakeSnapshot({ timeS: 8, scrammed: true, modelError: "Pressure out of range" }), 40);
  assert.deepEqual(
    log.entries.map((e) => [e.kind, e.severity, e.text, e.simTimeS]),
    [
      ["system", "trip", "Reactor scram: High flux", 5],
      ["system", "trip", "Model error: Pressure out of range", 7],
    ],
  );
  log.logSystem("Saved as \"before trip\"", 8, 50);
  assert.deepEqual([log.entries.at(-1).kind, log.entries.at(-1).severity], ["system", "info"]);
});

test("cap drops the oldest entries behind a single truncation note", () => {
  const log = new EventLog(100);
  for (let i = 0; i < 250; i++) log.logSystem(`note ${i}`, i, i);
  const e = log.entries;
  assert.ok(e.length <= 100);
  assert.match(e[0].text, /^Log truncated: \d+ oldest entries dropped \(limit 100\)$/);
  assert.equal(e.filter((x) => x.text.startsWith("Log truncated")).length, 1);
  assert.equal(e.at(-1).text, "note 249");
  const dropped = Number(/(\d+) oldest/.exec(e[0].text)[1]);
  assert.equal(dropped + e.length - 1, 250);
  assert.ok(!(validateLogEntries(log.toJSON()) instanceof Error), "seq stays increasing");
});
