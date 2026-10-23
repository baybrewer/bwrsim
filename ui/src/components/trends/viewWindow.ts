// Time-window state machine for the TRENDS page: presets, LIVE, zoom, pan. Pure.

export type Preset = "1M" | "10M" | "1H" | "ALL";
export const PRESETS: readonly Preset[] = ["1M", "10M", "1H", "ALL"];
export const PRESET_SPAN_S: Record<Exclude<Preset, "ALL">, number> = { "1M": 60, "10M": 600, "1H": 3600 };
export const PRESET_TITLES: Record<Preset, string> = { "1M": "Last minute", "10M": "Last 10 minutes", "1H": "Last hour", ALL: "Whole run" };

/** Narrowest window (s): raw samples are 1 s apart, so tighter shows nothing new. */
export const MIN_SPAN_S = 10;
/** ALL on a brand-new run still shows a minute so the axis is readable. */
export const ALL_MIN_SPAN_S = 60;
/** Zoom-out headroom beyond the whole run (or one hour, whichever is larger). */
const MAX_SPAN_FACTOR = 1.25;
/** How far a window may hang past run start / now, as a fraction of its span. */
const OVERHANG = 0.5;

/**
 * live: the window follows `now` using the preset (ALL keeps expanding from run start).
 * Not live: a fixed [t0, t1] chosen by zoom, pan, box or a focus time.
 */
export interface ViewState {
  preset: Preset;
  live: boolean;
  t0: number;
  t1: number;
}

/** Run start and latest sample time. */
export interface Bounds {
  start: number;
  now: number;
}

export interface Window {
  t0: number;
  t1: number;
}

export function initialView(preset: Preset = "10M"): ViewState {
  return { preset, live: true, t0: 0, t1: 0 };
}

export function resolveWindow(v: ViewState, b: Bounds): Window {
  if (!v.live) return { t0: v.t0, t1: v.t1 };
  if (v.preset === "ALL") return { t0: b.start, t1: Math.max(b.now, b.start + ALL_MIN_SPAN_S) };
  const span = PRESET_SPAN_S[v.preset];
  // Fixed span from the start of the run until it is filled, then it scrolls.
  const t1 = Math.max(b.now, b.start + span);
  return { t0: t1 - span, t1 };
}

export function selectPreset(_v: ViewState, preset: Preset): ViewState {
  return { preset, live: true, t0: 0, t1: 0 };
}

export function goLive(v: ViewState): ViewState {
  return { ...v, live: true };
}

function maxSpan(b: Bounds): number {
  return Math.max(b.now - b.start, 3600) * MAX_SPAN_FACTOR;
}

/** Clamps span and position so the window always overlaps the run. */
export function clampWindow(w: Window, b: Bounds): Window {
  let span = Math.min(Math.max(w.t1 - w.t0, MIN_SPAN_S), maxSpan(b));
  if (!Number.isFinite(span)) span = MIN_SPAN_S;
  const mid = (w.t0 + w.t1) / 2;
  let t0 = Number.isFinite(mid) ? mid - span / 2 : b.now - span;
  const lo = b.start - span * OVERHANG;
  const hi = b.now + span * OVERHANG - span;
  if (hi < lo) t0 = (lo + hi) / 2;
  else t0 = Math.min(Math.max(t0, lo), hi);
  return { t0, t1: t0 + span };
}

/** Fixed window (leaves LIVE). */
export function setWindow(v: ViewState, w: Window, b: Bounds): ViewState {
  const c = clampWindow(w, b);
  return { preset: v.preset, live: false, t0: c.t0, t1: c.t1 };
}

/** factor > 1 zooms out, < 1 zooms in; the anchor time stays under the cursor. */
export function zoomAbout(v: ViewState, b: Bounds, anchorT: number, factor: number): ViewState {
  const w = resolveWindow(v, b);
  const span = w.t1 - w.t0;
  const target = Math.min(Math.max(span * factor, MIN_SPAN_S), maxSpan(b));
  const frac = span > 0 ? (anchorT - w.t0) / span : 0.5;
  const t0 = anchorT - frac * target;
  return setWindow(v, { t0, t1: t0 + target }, b);
}

/** Shifts the window by dt seconds (positive = later). */
export function panBy(v: ViewState, b: Bounds, dt: number): ViewState {
  const w = resolveWindow(v, b);
  return setWindow(v, { t0: w.t0 + dt, t1: w.t1 + dt }, b);
}

/** Centres the current span on t (e.g. a log row click); leaves LIVE. */
export function centreOn(v: ViewState, b: Bounds, t: number): ViewState {
  const w = resolveWindow(v, b);
  const span = w.t1 - w.t0;
  return setWindow(v, { t0: t - span / 2, t1: t + span / 2 }, b);
}

/** Wheel delta (pixels) -> zoom factor; one notch (~100 px) is about 1.25x. */
export function wheelFactor(deltaY: number): number {
  return Math.exp(Math.max(-500, Math.min(500, deltaY)) * 0.0022);
}
