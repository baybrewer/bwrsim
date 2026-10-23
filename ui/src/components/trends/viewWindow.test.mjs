import { test } from "node:test";
import assert from "node:assert/strict";
import { MIN_SPAN_S, centreOn, clampWindow, goLive, initialView, panBy, resolveWindow, selectPreset, setWindow, zoomAbout } from "./viewWindow.ts";

const B = (now, start = 0) => ({ start, now });

test("live presets follow now with a fixed span, filling from run start", () => {
  const v = initialView("10M");
  assert.deepEqual(resolveWindow(v, B(100)), { t0: 0, t1: 600 });
  assert.deepEqual(resolveWindow(v, B(5000)), { t0: 4400, t1: 5000 });
  assert.deepEqual(resolveWindow(selectPreset(v, "1M"), B(5000)), { t0: 4940, t1: 5000 });
});

test("ALL spans run start to now and keeps expanding", () => {
  const v = selectPreset(initialView(), "ALL");
  assert.deepEqual(resolveWindow(v, B(10, 0)), { t0: 0, t1: 60 });
  assert.deepEqual(resolveWindow(v, B(7200, 0)), { t0: 0, t1: 7200 });
  assert.deepEqual(resolveWindow(v, B(9000, 0)), { t0: 0, t1: 9000 });
});

test("zoom keeps the anchor under the cursor and leaves LIVE", () => {
  const v = initialView("10M");
  const b = B(5000);
  const z = zoomAbout(v, b, 4700, 0.5);
  assert.equal(z.live, false);
  assert.equal(z.t1 - z.t0, 300);
  assert.equal((4700 - z.t0) / (z.t1 - z.t0), 0.5);
  const tiny = zoomAbout(z, b, 4700, 1e-6);
  assert.equal(tiny.t1 - tiny.t0, MIN_SPAN_S);
});

test("pan shifts but cannot leave the run behind", () => {
  const b = B(5000);
  const p = panBy(initialView("10M"), b, -1000);
  assert.deepEqual([p.t0, p.t1, p.live], [3400, 4000, false]);
  const far = panBy(p, b, 1e6);
  assert.equal(far.t1, 5000 + 300);
  const back = panBy(p, b, -1e6);
  assert.equal(back.t0, -300);
});

test("LIVE resumes the preset; double-click reset is goLive", () => {
  const b = B(5000);
  const p = panBy(initialView("1H"), b, -100);
  const l = goLive(p);
  assert.equal(l.live, true);
  assert.deepEqual(resolveWindow(l, b), { t0: 1400, t1: 5000 });
});

test("centreOn keeps the span and leaves LIVE; box window is clamped", () => {
  const b = B(5000);
  const c = centreOn(initialView("1M"), b, 2000);
  assert.deepEqual([c.t0, c.t1, c.live], [1970, 2030, false]);
  const box = setWindow(c, { t0: 2010, t1: 2012 }, b);
  assert.equal(box.t1 - box.t0, MIN_SPAN_S);
  assert.equal(box.t0 + MIN_SPAN_S / 2, 2011);
});

test("clamp centres a window wider than the allowed range", () => {
  const w = clampWindow({ t0: -1e9, t1: 1e9 }, B(100));
  assert.equal(w.t1 - w.t0, 3600 * 1.25);
  assert.ok(Number.isFinite(w.t0));
});
