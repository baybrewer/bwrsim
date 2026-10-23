// Canvas renderer for one TRENDS chart card. No React; cost is one history query per draw.
import type { HistoryQueryResult } from "../../sim/logic/history.ts";
import { fmtClock, noNegZero } from "../../util/format.ts";
import { fitRange, niceTimeTicks, niceValueTicks, timeLabel } from "./axis.ts";
import type { MarkerCluster, MarkerTone } from "./markers.ts";

export const PAD = { l: 58, r: 58, t: 20, b: 34 } as const;
/** Marker lane just under the plot, above the time labels. */
export const LANE = { top: 2, height: 12 } as const;

const C = {
  bg: "#0a0d0c",
  plot: "#070a09",
  grid: "#1e2723",
  gridMajor: "#2a3530",
  text: "#8b958f",
  label: "#a9b3ad",
  outside: "#ffffff06",
  now: "#57f08a",
  cursor: "#e8f0ec",
  box: "#4cc9f0",
};
export const TONE_COLOR: Record<MarkerTone, string> = { trip: "#ff4b3e", alarm: "#ffb43a", action: "#8b958f", info: "#5d6a64" };
const TONE_ALPHA: Record<MarkerTone, number> = { trip: 0.55, alarm: 0.35, action: 0.18, info: 0.12 };

export interface PlotRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export function plotRect(w: number, h: number): PlotRect {
  return { x: PAD.l, y: PAD.t, w: Math.max(1, w - PAD.l - PAD.r), h: Math.max(1, h - PAD.t - PAD.b) };
}

export interface DrawSeries {
  id: string;
  color: string;
  unit: string;
}

/** Per-unit Y range; series sharing a unit share a scale. */
export type Scales = Record<string, [number, number]>;

/** Fits each unit's visible min/max envelope (5% pad) unless locked. */
export function computeScales(data: HistoryQueryResult, series: readonly DrawSeries[], locked: Scales | null): Scales {
  const out: Scales = {};
  const ext: Record<string, [number, number]> = {};
  for (const se of series) {
    const cols = data.series[se.id];
    const e = (ext[se.unit] ??= [Infinity, -Infinity]);
    if (!cols) continue;
    for (let i = 0; i < cols.min.length; i++) {
      const mn = cols.min[i];
      const mx = cols.max[i];
      if (mn !== null && mn < e[0]) e[0] = mn;
      if (mx !== null && mx > e[1]) e[1] = mx;
    }
  }
  for (const unit of Object.keys(ext)) out[unit] = locked?.[unit] ?? fitRange(ext[unit][0], ext[unit][1]);
  return out;
}

export interface AxisSpec {
  unit: string;
  color: string;
}

export interface DrawInput {
  w: number;
  h: number;
  t0: number;
  t1: number;
  start: number;
  now: number;
  live: boolean;
  data: HistoryQueryResult;
  series: readonly DrawSeries[];
  scales: Scales;
  left: AxisSpec | null;
  right: AxisSpec | null;
  hoverT: number | null;
  cursorValues: Record<string, number | null>;
  markers: readonly MarkerCluster[];
  box: { x0: number; x1: number } | null;
}

function fmtTick(v: number, digits: number): string {
  const a = Math.abs(v);
  if (a >= 1e5 || (a > 0 && a < 1e-3)) return v.toExponential(1).toUpperCase();
  return noNegZero(v.toFixed(digits));
}

export function drawChart(ctx: CanvasRenderingContext2D, dpr: number, d: DrawInput): void {
  const { w, h, t0, t1 } = d;
  const p = plotRect(w, h);
  const span = t1 - t0 || 1;
  const X = (t: number) => p.x + ((t - t0) / span) * p.w;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.fillStyle = C.bg;
  ctx.fillRect(0, 0, w, h);
  ctx.fillStyle = C.plot;
  ctx.fillRect(p.x, p.y, p.w, p.h);
  ctx.font = "10px ui-monospace, Menlo, Consolas, monospace";

  // Shade time outside the run (before start, after now) so an empty edge reads as "no data yet".
  ctx.fillStyle = C.outside;
  const xs = X(d.start);
  if (xs > p.x) ctx.fillRect(p.x, p.y, Math.min(xs, p.x + p.w) - p.x, p.h);
  const xn = X(d.now);
  if (xn < p.x + p.w) ctx.fillRect(Math.max(xn, p.x), p.y, p.x + p.w - Math.max(xn, p.x), p.h);

  // Time grid and labels.
  const { ticks } = niceTimeTicks(t0, t1, Math.max(2, Math.floor(p.w / 96)));
  ctx.lineWidth = 1;
  ctx.textAlign = "center";
  ctx.textBaseline = "alphabetic";
  for (const t of ticks) {
    const x = Math.round(X(t)) + 0.5;
    ctx.strokeStyle = C.grid;
    ctx.beginPath();
    ctx.moveTo(x, p.y);
    ctx.lineTo(x, p.y + p.h + 3);
    ctx.stroke();
    ctx.fillStyle = C.text;
    ctx.fillText(timeLabel(t), x, h - 5);
  }

  // Y grid from the left scale; labels for left and right scales in their own colours.
  const yTicks = Math.max(2, Math.floor(p.h / 44));
  const drawAxis = (a: AxisSpec | null, side: "l" | "r") => {
    const sc = a && d.scales[a.unit];
    if (!a || !sc) return;
    const [lo, hi] = sc;
    const Y = (v: number) => p.y + (1 - (v - lo) / (hi - lo)) * p.h;
    const { ticks: vt, digits } = niceValueTicks(lo, hi, yTicks);
    ctx.textBaseline = "middle";
    ctx.textAlign = side === "l" ? "right" : "left";
    for (const v of vt) {
      const y = Math.round(Y(v)) + 0.5;
      if (side === "l") {
        ctx.strokeStyle = C.grid;
        ctx.beginPath();
        ctx.moveTo(p.x, y);
        ctx.lineTo(p.x + p.w, y);
        ctx.stroke();
      } else {
        ctx.strokeStyle = C.gridMajor;
        ctx.beginPath();
        ctx.moveTo(p.x + p.w, y);
        ctx.lineTo(p.x + p.w + 4, y);
        ctx.stroke();
      }
      ctx.fillStyle = a.color;
      ctx.fillText(fmtTick(v, digits), side === "l" ? p.x - 6 : p.x + p.w + 7, y);
    }
    ctx.textBaseline = "alphabetic";
    ctx.fillStyle = a.color;
    // Above the axis, not beside the bottom tick, so it never collides with the time labels.
    ctx.fillText(a.unit, side === "l" ? p.x - 6 : p.x + p.w + 7, p.y - 7);
  };
  drawAxis(d.left, "l");
  drawAxis(d.right, "r");

  ctx.save();
  ctx.beginPath();
  ctx.rect(p.x, p.y, p.w, p.h);
  ctx.clip();

  // Event marker rules through the plot (the clickable chips are DOM buttons in the lane).
  for (const m of d.markers) {
    const x = Math.round(p.x + m.x) + 0.5;
    ctx.globalAlpha = TONE_ALPHA[m.tone];
    ctx.strokeStyle = TONE_COLOR[m.tone];
    ctx.setLineDash(m.tone === "trip" ? [] : [3, 3]);
    ctx.beginPath();
    ctx.moveTo(x, p.y);
    ctx.lineTo(x, p.y + p.h);
    ctx.stroke();
  }
  ctx.setLineDash([]);
  ctx.globalAlpha = 1;

  const n = d.data.t.length;
  const colW = Math.max(1, p.w / Math.max(1, n));
  for (const se of d.series) {
    const cols = d.data.series[se.id];
    const sc = d.scales[se.unit];
    if (!cols || !sc) continue;
    const [lo, hi] = sc;
    const Y = (v: number) => p.y + (1 - (v - lo) / (hi - lo)) * p.h;
    // Min/max envelope: shows the spread a pixel column hides when several samples share it.
    ctx.fillStyle = se.color;
    ctx.globalAlpha = 0.22;
    for (let i = 0; i < n; i++) {
      const mn = cols.min[i];
      const mx = cols.max[i];
      if (mn === null || mx === null) continue;
      const yTop = Y(mx);
      const hgt = Y(mn) - yTop;
      if (hgt >= 1) ctx.fillRect(X(d.data.t[i]) - colW / 2, yTop, Math.max(1, colW), hgt);
    }
    ctx.globalAlpha = 1;
    // Last-value line; a gap column breaks it on both sides.
    ctx.strokeStyle = se.color;
    ctx.lineWidth = 1.6;
    ctx.lineJoin = "round";
    ctx.beginPath();
    let segLen = 0;
    let lx = 0;
    let ly = 0;
    const endSeg = () => {
      if (segLen === 1) ctx.rect(lx - 1, ly - 1, 2, 2);
      segLen = 0;
    };
    for (let i = 0; i < n; i++) {
      const v = cols.last[i];
      if (v === null || cols.gap[i]) {
        endSeg();
        continue;
      }
      lx = X(d.data.t[i]);
      ly = Y(v);
      if (segLen === 0) ctx.moveTo(lx, ly);
      else ctx.lineTo(lx, ly);
      segLen++;
    }
    endSeg();
    ctx.stroke();
  }

  // LIVE edge.
  if (d.live && xn >= p.x && xn <= p.x + p.w) {
    ctx.strokeStyle = C.now;
    ctx.globalAlpha = 0.6;
    ctx.beginPath();
    ctx.moveTo(Math.round(xn) - 0.5, p.y);
    ctx.lineTo(Math.round(xn) - 0.5, p.y + p.h);
    ctx.stroke();
    ctx.globalAlpha = 1;
  }

  // Box-zoom selection.
  if (d.box) {
    const bx = Math.min(d.box.x0, d.box.x1);
    const bw = Math.abs(d.box.x1 - d.box.x0);
    ctx.fillStyle = C.box;
    ctx.globalAlpha = 0.12;
    ctx.fillRect(p.x + bx, p.y, bw, p.h);
    ctx.globalAlpha = 0.8;
    ctx.strokeStyle = C.box;
    ctx.strokeRect(p.x + bx + 0.5, p.y + 0.5, bw, p.h - 1);
    ctx.globalAlpha = 1;
  }

  // Crosshair with a dot on each series at the cursor value.
  if (d.hoverT !== null && d.hoverT >= t0 && d.hoverT <= t1) {
    const x = Math.round(X(d.hoverT)) + 0.5;
    ctx.strokeStyle = C.cursor;
    ctx.globalAlpha = 0.55;
    ctx.beginPath();
    ctx.moveTo(x, p.y);
    ctx.lineTo(x, p.y + p.h);
    ctx.stroke();
    ctx.globalAlpha = 1;
    for (const se of d.series) {
      const v = d.cursorValues[se.id];
      const sc = d.scales[se.unit];
      if (v === null || v === undefined || !sc) continue;
      const y = p.y + (1 - (v - sc[0]) / (sc[1] - sc[0])) * p.h;
      ctx.fillStyle = C.plot;
      ctx.strokeStyle = se.color;
      ctx.lineWidth = 1.5;
      ctx.beginPath();
      ctx.arc(x, y, 3.2, 0, Math.PI * 2);
      ctx.fill();
      ctx.stroke();
    }
  }
  ctx.restore();

  // Cursor time flag on the time axis.
  if (d.hoverT !== null && d.hoverT >= t0 && d.hoverT <= t1) {
    const x = X(d.hoverT);
    const label = fmtClock(d.hoverT);
    const tw = ctx.measureText(label).width + 10;
    const bx = Math.min(Math.max(x - tw / 2, p.x), p.x + p.w - tw);
    ctx.fillStyle = "#dfe6e2";
    ctx.fillRect(bx, h - 16, tw, 14);
    ctx.fillStyle = "#070a09";
    ctx.textAlign = "center";
    ctx.fillText(label, bx + tw / 2, h - 5);
  }

  // Plot frame.
  ctx.strokeStyle = C.gridMajor;
  ctx.lineWidth = 1;
  ctx.strokeRect(p.x - 0.5, p.y - 0.5, p.w + 1, p.h + 1);
}
