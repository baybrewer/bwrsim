import { test } from "node:test";
import assert from "node:assert/strict";
import { fitRange, niceTimeStep, niceTimeTicks, niceValueTicks, spanLabel, timeLabel } from "./axis.ts";

test("time steps snap to s/min/h candidates", () => {
  assert.equal(niceTimeStep(60, 8), 10);
  assert.equal(niceTimeStep(600, 8), 120);
  assert.equal(niceTimeStep(3600, 8), 600);
  assert.equal(niceTimeStep(86_400, 8), 10_800);
  assert.equal(niceTimeStep(30 * 86_400, 6), 5 * 86_400);
});

test("time ticks are step multiples inside the window", () => {
  const { stepS, ticks } = niceTimeTicks(95, 215, 6);
  assert.equal(stepS, 30);
  assert.deepEqual(ticks, [120, 150, 180, 210]);
});

test("labels: sim clock, nothing before run start; span words", () => {
  assert.equal(timeLabel(3725), "01:02:05");
  assert.equal(timeLabel(-10), "");
  assert.equal(spanLabel(60), "60 s");
  assert.equal(spanLabel(600), "10 min");
  assert.equal(spanLabel(5400), "1 h 30 min");
  assert.equal(spanLabel(3 * 86_400), "3 d");
});

test("value ticks use 1/2/5 steps with matching digits", () => {
  const a = niceValueTicks(0, 100, 5);
  assert.equal(a.step, 20);
  assert.deepEqual(a.ticks, [0, 20, 40, 60, 80, 100]);
  assert.equal(a.digits, 0);
  const b = niceValueTicks(7.01, 7.13, 4);
  assert.equal(b.step, 0.05);
  assert.equal(b.digits, 2);
  assert.equal(b.ticks.length, 2);
});

test("fitRange pads 5% and widens flat or empty data", () => {
  assert.deepEqual(fitRange(0, 100), [-5, 105]);
  const [lo, hi] = fitRange(50, 50);
  assert.ok(lo < 50 && hi > 50 && Math.abs(hi - lo - 1) < 1e-9);
  assert.deepEqual(fitRange(Infinity, -Infinity), [0, 1]);
});
