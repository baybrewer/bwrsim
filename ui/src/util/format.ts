/** Shared number/time formatting helpers for panel readouts. */

export function pad2(n: number): string {
  return String(Math.max(0, Math.floor(n))).padStart(2, "0");
}

/** Seconds -> "hh:mm:ss" (hours keep counting past 24). */
export function fmtClock(totalS: number): string {
  const s = Math.max(0, Math.floor(totalS));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  return `${pad2(h)}:${pad2(m)}:${pad2(s % 60)}`;
}

/** Fixed decimals, with a dash for non-finite values. */
export function fx(v: number | null | undefined, digits = 1): string {
  if (v === null || v === undefined || !Number.isFinite(v)) return "---";
  return noNegZero(v.toFixed(digits));
}

/** "-0.0" -> "0.0" */
export function noNegZero(s: string): string {
  return /^-0(\.0+)?$/.test(s) ? s.slice(1) : s;
}

/** Fixed decimals with explicit sign. */
export function fxSigned(v: number, digits = 0): string {
  if (!Number.isFinite(v)) return "---";
  const s = noNegZero(v.toFixed(digits));
  return v > 0 && Number(s) !== 0 ? `+${s}` : s;
}

/** Engineering-ish exponent format: 3.2E+04 */
export function fmtExp(v: number, digits = 1): string {
  if (!Number.isFinite(v)) return "---";
  if (v <= 0) return "0.0E+00";
  const e = v.toExponential(digits).toUpperCase();
  return e.replace(/E([+-])(\d)$/, "E$10$2");
}

/** Reactor period: "∞" when null/huge. */
export function fmtPeriod(p: number | null): string {
  if (p === null || !Number.isFinite(p) || Math.abs(p) > 9999) return "∞";
  return `${p > 0 ? "+" : ""}${p.toFixed(Math.abs(p) < 100 ? 1 : 0)}`;
}

/** Rod position as BWR 2-digit notch "00".."48". */
export function fmtRodPos(p: number): string {
  const n = Math.round(Math.max(0, Math.min(48, p)));
  return String(n).padStart(2, "0");
}

/** Conventional BWR rod coordinate name "XX-YY" from grid col/row (row 0 at top). */
export function rodName(col: number, row: number, gridSize = 9): string {
  const x = 2 + 4 * col;
  const y = 3 + 4 * (gridSize - 1 - row);
  return `${pad2(x)}-${pad2(y)}`;
}

export function clamp(v: number, lo: number, hi: number): number {
  return Math.max(lo, Math.min(hi, v));
}

/** 0..1 fraction -> percent string. */
export function pct(frac: number, digits = 0): string {
  return fx(frac * 100, digits);
}
