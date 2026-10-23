import { useEffect, useRef } from "react";
import type { HistoryQueryResult } from "../sim/logic/history.ts";
import { noNegZero } from "../util/format";

export interface Series {
  /** History channel id. */
  id: string;
  name: string;
  color: string;
}

/**
 * Dependency-free canvas strip chart over the run history. Auto-scales Y unless fixed
 * bounds are given. The history is queried per draw at the canvas width, so cost is
 * bounded by pixels, not by run length.
 */
export function StripChart(props: {
  title: string;
  unit: string;
  series: Series[];
  /** Current readout per series (null = no value). */
  values: (number | null)[];
  query: (t0: number, t1: number, pixels: number) => HistoryQueryResult;
  /** Sim time at the right edge of the window. */
  tNow: number;
  windowS: number;
  yMin?: number;
  yMax?: number;
  digits?: number;
}) {
  const ref = useRef<HTMLCanvasElement>(null);
  const { series, windowS } = props;

  useEffect(() => {
    const cv = ref.current;
    if (!cv) return;
    const dpr = window.devicePixelRatio || 1;
    const w = cv.clientWidth;
    const h = cv.clientHeight;
    if (cv.width !== Math.round(w * dpr) || cv.height !== Math.round(h * dpr)) {
      cv.width = Math.round(w * dpr);
      cv.height = Math.round(h * dpr);
    }
    const ctx = cv.getContext("2d");
    if (!ctx) return;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, w, h);

    const padL = 40;
    const padR = 6;
    const padT = 4;
    const padB = 14;
    const pw = w - padL - padR;
    const ph = h - padT - padB;

    const tEnd = Math.max(props.tNow, windowS);
    const tStart = tEnd - windowS;
    const data = props.query(tStart, tEnd, Math.max(1, Math.round(pw)));

    // y range
    let lo = props.yMin ?? Infinity;
    let hi = props.yMax ?? -Infinity;
    if (props.yMin === undefined || props.yMax === undefined) {
      for (const se of series) {
        const cols = data.series[se.id];
        if (!cols) continue;
        for (let i = 0; i < data.t.length; i++) {
          const mn = cols.min[i];
          const mx = cols.max[i];
          if (props.yMin === undefined && mn !== null) lo = Math.min(lo, mn);
          if (props.yMax === undefined && mx !== null) hi = Math.max(hi, mx);
        }
      }
      if (!Number.isFinite(lo) || !Number.isFinite(hi)) {
        lo = 0;
        hi = 1;
      }
      const span = hi - lo;
      const minSpan = Math.max(Math.abs(hi) * 0.02, 1e-3);
      if (span < minSpan) {
        const mid = (hi + lo) / 2;
        lo = props.yMin ?? mid - minSpan / 2;
        hi = props.yMax ?? mid + minSpan / 2;
      } else {
        if (props.yMin === undefined) lo -= span * 0.08;
        if (props.yMax === undefined) hi += span * 0.08;
      }
    }
    const X = (t: number) => padL + ((t - tStart) / windowS) * pw;
    const Y = (v: number) => padT + (1 - (v - lo) / (hi - lo)) * ph;

    // grid
    ctx.strokeStyle = "#26302b";
    ctx.lineWidth = 1;
    ctx.fillStyle = "#7f8a85";
    ctx.font = "10px ui-monospace, Menlo, Consolas, monospace";
    ctx.textAlign = "right";
    ctx.textBaseline = "middle";
    const step = (hi - lo) / 4;
    const digits = Math.max(props.digits ?? 1, step > 0 && step < 1 ? Math.min(3, Math.ceil(-Math.log10(step))) : 0);
    for (let k = 0; k <= 4; k++) {
      const v = lo + ((hi - lo) * k) / 4;
      const y = Math.round(Y(v)) + 0.5;
      ctx.beginPath();
      ctx.moveTo(padL, y);
      ctx.lineTo(w - padR, y);
      ctx.stroke();
      ctx.fillText(noNegZero(v.toFixed(digits)), padL - 4, y);
    }
    ctx.textAlign = "center";
    ctx.textBaseline = "alphabetic";
    for (let m = 0; m <= windowS / 60; m += 2) {
      const x = Math.round(padL + (m * 60 * pw) / windowS) + 0.5;
      ctx.beginPath();
      ctx.moveTo(x, padT);
      ctx.lineTo(x, padT + ph);
      ctx.stroke();
      ctx.fillText(`-${windowS / 60 - m}m`, x, h - 3);
    }

    // traces
    ctx.save();
    ctx.beginPath();
    ctx.rect(padL, padT, pw, ph);
    ctx.clip();
    for (const se of series) {
      const cols = data.series[se.id];
      if (!cols) continue;
      ctx.strokeStyle = se.color;
      ctx.lineWidth = 1.6;
      ctx.beginPath();
      let started = false;
      for (let i = 0; i < data.t.length; i++) {
        const v = cols.last[i];
        // A missing value breaks the line rather than bridging the gap.
        if (v === null || cols.gap[i]) {
          started = false;
          if (v === null) continue;
        }
        const x = X(data.t[i]);
        const y = Y(v);
        if (!started) {
          ctx.moveTo(x, y);
          started = true;
        } else ctx.lineTo(x, y);
      }
      ctx.stroke();
    }
    ctx.restore();
  });

  return (
    <div className="strip">
      <div className="strip-head">
        <span className="strip-title">{props.title}</span>
        <span className="strip-vals">
          {series.map((se, i) => {
            const v = props.values[i];
            return (
              <span key={se.id} style={{ color: se.color }}>
                {series.length > 1 ? `${se.name} ` : ""}
                {v === null || v === undefined ? "---" : noNegZero(v.toFixed(props.digits ?? 1))}
              </span>
            );
          })}
          <span className="strip-unit">{props.unit}</span>
        </span>
      </div>
      <canvas ref={ref} className="strip-canvas" />
    </div>
  );
}
