import { test } from "node:test";
import assert from "node:assert/strict";
import { CHANNELS, CHANNEL_GROUPS, MINI_CHARTS, channelById, readChannel } from "./channels.ts";
import { fakeSnapshot } from "./testkit.mjs";

test("registry ids are unique and every group is used", () => {
  assert.equal(new Set(CHANNELS.map((c) => c.id)).size, CHANNELS.length);
  assert.deepEqual([...new Set(CHANNELS.map((c) => c.group))].sort(), [...CHANNEL_GROUPS].sort());
});

test("mini charts are registry ids with the panel's values and colours", () => {
  const s = fakeSnapshot();
  const expected = {
    thermalPowerPct: [100, "#3ee07a"],
    pressureMpa: [7.07, "#ffb43a"],
    levelNrCm: [0, "#4cc9f0"],
    coreFlowKgs: [4000, "#b48cff"],
    steamFlowKgs: [470, "#ff5a4e"],
    feedFlowKgs: [468, "#4cc9f0"],
    grossMwe: [300, "#dfe6e2"],
  };
  const ids = MINI_CHARTS.flatMap((c) => c.channelIds);
  assert.equal(MINI_CHARTS.length, 6);
  assert.deepEqual(ids.sort(), Object.keys(expected).sort());
  for (const [id, [value, color]] of Object.entries(expected)) {
    const c = channelById(id);
    assert.equal(readChannel(c, s), value, id);
    assert.equal(c.color, color, id);
  }
});

test("null and non-finite readings are missing", () => {
  const period = channelById("periodS");
  assert.equal(readChannel(period, fakeSnapshot()), null);
  const n = fakeSnapshot().neutronics;
  assert.equal(readChannel(period, fakeSnapshot({ neutronics: { ...n, periodS: 42 } })), 42);
  assert.equal(readChannel(channelById("fuelTempC"), fakeSnapshot({ neutronics: { ...n, fuelTempC: NaN } })), null);
});
