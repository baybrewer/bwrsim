import { test } from "node:test";
import assert from "node:assert/strict";
import { History } from "./history.ts";
import { EventLog } from "./log.ts";
import { validateRunRecord } from "./runRecord.ts";
import { X_CHANNELS } from "./testkit.mjs";

function goodRecord() {
  const h = new History(X_CHANNELS);
  for (let t = 0; t < 30; t++) h.ingestValues(t, [t === 5 ? null : t]);
  const log = new EventLog();
  log.logSystem("Scenario HOT STANDBY loaded", 0, 0);
  log.logAction({ type: "setLoadLimit", pct: 90 }, null, 10);
  return JSON.parse(JSON.stringify({ version: 1, runScenario: "hotStandby", history: h.toJSON(), log: log.toJSON(), annConsumed: 0 }));
}

test("a well-formed record validates", () => {
  const r = validateRunRecord(goodRecord());
  assert.ok(!(r instanceof Error), String(r));
});

const failures = {
  "not an object": () => null,
  "wrong version": (r) => ({ ...r, version: 2 }),
  "unknown scenario": (r) => ({ ...r, runScenario: "toString" }),
  "missing annConsumed": (r) => ({ ...r, annConsumed: undefined }),
  "negative annConsumed": (r) => ({ ...r, annConsumed: -1 }),
  "column length mismatch": (r) => (r.history.levels[0].series.x.max.pop(), r),
  "non-monotonic time": (r) => ((r.history.levels[0].t[3] = 1), r),
  "string value": (r) => ((r.history.levels[1].series.x.last[0] = "7"), r),
  "non-boolean gap": (r) => ((r.history.levels[1].series.x.gap[0] = 0), r),
  "missing channel column": (r) => ((r.history.channels = ["x", "y"]), r),
  "single level": (r) => ((r.history.levels = r.history.levels.slice(0, 1)), r),
  "bad log kind": (r) => ((r.log[0].kind = "note"), r),
  "bad log severity": (r) => ((r.log[0].severity = "critical"), r),
  "log text not a string": (r) => ((r.log[1].text = 5), r),
  "log seq not increasing": (r) => ((r.log[1].seq = r.log[0].seq), r),
  "log not an array": (r) => ({ ...r, log: {} }),
};

for (const [name, mutate] of Object.entries(failures)) {
  test(`validateRunRecord rejects: ${name}`, () => {
    const result = validateRunRecord(mutate(goodRecord()));
    assert.ok(result instanceof Error, `${name} accepted`);
  });
}
