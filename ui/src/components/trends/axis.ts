// Axis math for the TRENDS page: nice time/value ticks and Y auto-fit. Pure.
import { fmtClock } from "../../util/format.ts";

/** Candidate time-tick steps (s): seconds, minutes, hours, then whole days. */
const TIME_STEPS_S = [1, 2, 5, 10, 15, 30, 60, 120, 300, 600, 900, 1800, 3600, 7200, 10_800, 21_600, 43_200, 86_400];

/** Smallest candidate step giving at most `maxTicks` ticks over the span. */
export function niceTimeStep(spanS: number, maxTicks: number): number {
  const raw = spanS / Math.max(1, maxTicks);
  for (const step of TIME_STEPS_S) if (step >= raw) return step;
  return Math.ceil(raw / 86_400) * 86_400;
}

/** Tick times (multiples of the step) inside [t0, t1]. */
export function niceTimeTicks(t0: number, t1: number, maxTicks: number): { stepS: number; ticks: number[] } {
  const stepS = niceTimeStep(t1 - t0, maxTicks);
  const ticks: number[] = [];
  for (let t = Math.ceil(t0 / stepS) * stepS; t <= t1 + 1e-9; t += stepS) ticks.push(t);
  return { stepS, ticks };
}

/** Sim clock label; negative times (before run start) are not labelled. */
export function timeLabel(t: number): string {
  return t < 0 ? "" : fmtClock(t);
}

/** Human span: "45 s", "10 min", "1 h 30 min", "2 d 3 h". */
export function spanLabel(spanS: number): string {
  const s = Math.round(spanS);
  if (s < 120) return `${s} s`;
  if (s < 3600) return `${Math.round(s / 60)} min`;
  if (s < 172_800) {
    const h = Math.floor(s / 3600);
    const m = Math.round((s % 3600) / 60);
    return m ? `${h} h ${m} min` : `${h} h`;
  }
  const d = Math.floor(s / 86_400);
  const h = Math.round((s % 86_400) / 3600);
  return h ? `${d} d ${h} h` : `${d} d`;
}

/** 1/2/5 x 10^k step giving at most `maxTicks` intervals over [lo, hi]. */
export function niceValueStep(lo: number, hi: number, maxTicks: number): number {
  const raw = (hi - lo) / Math.max(1, maxTicks);
  if (!(raw > 0) || !Number.isFinite(raw)) return 1;
  const mag = 10 ** Math.floor(Math.log10(raw));
  for (const m of [1, 2, 5, 10]) if (m * mag >= raw - 1e-12 * mag) return m * mag;
  return 10 * mag;
}

export function niceValueTicks(lo: number, hi: number, maxTicks: number): { step: number; ticks: number[]; digits: number } {
  const step = niceValueStep(lo, hi, maxTicks);
  const ticks: number[] = [];
  // Index-based so float accumulation cannot drift past the range end.
  const first = Math.ceil(lo / step - 1e-9);
  const last = Math.floor(hi / step + 1e-9);
  for (let i = first; i <= last; i++) ticks.push(i * step || 0);
  return { step, ticks, digits: Math.max(0, Math.min(6, -Math.floor(Math.log10(step) + 1e-9))) };
}

export const Y_PAD_FRACTION = 0.05;

/**
 * Auto-fit range over observed [lo, hi] with 5% padding each side. A flat or empty
 * series gets a small band around its value so it still draws mid-height.
 */
export function fitRange(lo: number, hi: number): [number, number] {
  if (!Number.isFinite(lo) || !Number.isFinite(hi)) return [0, 1];
  const span = hi - lo;
  // Absolute floor of 1 unit: a signal sitting at zero (turbine speed at rest)
  // otherwise auto-fits to a +/-5e-4 band with scientific-notation ticks.
  const minSpan = Math.max(Math.abs(hi) * 0.02, Math.abs(lo) * 0.02, 1);
  if (span < minSpan) {
    const mid = (hi + lo) / 2;
    return [mid - minSpan / 2, mid + minSpan / 2];
  }
  return [lo - span * Y_PAD_FRACTION, hi + span * Y_PAD_FRACTION];
}
