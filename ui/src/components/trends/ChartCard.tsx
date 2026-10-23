import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { Channel } from "../../sim/logic/channels.ts";
import type { History } from "../../sim/logic/history.ts";
import type { LogEntry } from "../../sim/logic/log.ts";
import { fmtClock, noNegZero } from "../../util/format.ts";
import { LANE, PAD, TONE_COLOR, computeScales, drawChart, plotRect } from "./drawChart.ts";
import type { AxisSpec, DrawSeries, Scales } from "./drawChart.ts";
import { clusterMarkers } from "./markers.ts";
import type { MarkerCluster } from "./markers.ts";
import { goLive, panBy, setWindow, wheelFactor, zoomAbout } from "./viewWindow.ts";
import type { Bounds, ViewState, Window } from "./viewWindow.ts";

export type ViewUpdate = (fn: (v: ViewState) => ViewState) => void;

export interface ChartCardProps {
  group: string;
  channels: readonly Channel[];
  history: History;
  /** Log entries sorted by (simTimeS, seq): EventLog.byTime. */
  log: readonly LogEntry[];
  /** Store version: a change means new data to draw. */
  version: number;
  win: Window;
  bounds: Bounds;
  live: boolean;
  hidden: ReadonlySet<string>;
  onToggleSeries: (id: string) => void;
  locked: Scales | null;
  onLock: (scales: Scales | null) => void;
  hoverT: number | null;
  onHover: (t: number | null) => void;
  cursorValues: Record<string, number | null>;
  onView: ViewUpdate;
  onOpenLog: (seq: number) => void;
  highlight: boolean;
}

const NEUTRAL_AXIS = "#a9b3ad";
const KEY_PAN_FRACTION = 0.1;
const KEY_ZOOM_IN = 0.8;
const MAX_TIP_ROWS = 6;

type Drag = { mode: "pan"; x0: number; win: Window } | { mode: "box"; x0: number; x1: number };

/** DOM id of a group's card (TrendsPage scrolls to it). */
export function cardId(group: string): string {
  return `trends-card-${group.replace(/[^a-z0-9]+/gi, "-").toLowerCase()}`;
}

function fmtValue(v: number | null | undefined, digits: number): string {
  return v === null || v === undefined || !Number.isFinite(v) ? "—" : noNegZero(v.toFixed(digits));
}

/** Left axis = first visible unit, right axis = second; other units read from the legend. */
function axisSpecs(series: readonly (DrawSeries & { color: string })[]): [AxisSpec | null, AxisSpec | null] {
  const units: string[] = [];
  for (const se of series) if (!units.includes(se.unit)) units.push(se.unit);
  const spec = (unit: string | undefined): AxisSpec | null => {
    if (unit === undefined) return null;
    const of = series.filter((se) => se.unit === unit);
    return { unit, color: of.length === 1 ? of[0].color : NEUTRAL_AXIS };
  };
  return [spec(units[0]), spec(units[1])];
}

export function ChartCard(props: ChartCardProps) {
  const wrapRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [size, setSize] = useState({ w: 0, h: 0 });
  const [drag, setDrag] = useState<Drag | null>(null);
  const [tip, setTip] = useState<MarkerCluster | null>(null);
  const scalesRef = useRef<Scales>({});
  const propsRef = useRef(props);
  propsRef.current = props;
  const rafRef = useRef(0);

  const { win, bounds } = props;
  const plot = plotRect(size.w, size.h);
  const span = win.t1 - win.t0;
  const visible = props.channels.filter((c) => !props.hidden.has(c.id));
  const markers = size.w > 0 ? clusterMarkers(props.log, win.t0, win.t1, plot.w) : [];
  const box = drag?.mode === "box" ? { x0: drag.x0, x1: drag.x1 } : null;
  // The frame callback may run after later renders; it reads the newest render's values.
  const frameRef = useRef({ size, box });
  frameRef.current = { size, box };

  useLayoutEffect(() => {
    const el = wrapRef.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setSize({ w: el.clientWidth, h: el.clientHeight }));
    ro.observe(el);
    setSize({ w: el.clientWidth, h: el.clientHeight });
    return () => ro.disconnect();
  }, []);

  // Coalesce every render (store change, hover, view change) into at most one draw per frame.
  useEffect(() => {
    if (rafRef.current) return;
    rafRef.current = requestAnimationFrame(() => {
      rafRef.current = 0;
      const cv = canvasRef.current;
      const p = propsRef.current;
      const { size, box } = frameRef.current;
      if (!cv || size.w === 0 || size.h === 0) return;
      const dpr = window.devicePixelRatio || 1;
      const bw = Math.round(size.w * dpr);
      const bh = Math.round(size.h * dpr);
      if (cv.width !== bw || cv.height !== bh) {
        cv.width = bw;
        cv.height = bh;
      }
      const ctx = cv.getContext("2d");
      if (!ctx) return;
      const pr = plotRect(size.w, size.h);
      const series = p.channels.filter((c) => !p.hidden.has(c.id)).map((c) => ({ id: c.id, color: c.color, unit: c.unit }));
      const data = p.history.query(p.win.t0, p.win.t1, Math.max(1, Math.round(pr.w)), series.map((s) => s.id));
      const scales = computeScales(data, series, p.locked);
      scalesRef.current = scales;
      const [left, right] = axisSpecs(series);
      drawChart(ctx, dpr, {
        w: size.w,
        h: size.h,
        t0: p.win.t0,
        t1: p.win.t1,
        start: p.bounds.start,
        now: p.bounds.now,
        live: p.live,
        data,
        series,
        scales,
        left,
        right,
        hoverT: p.hoverT,
        cursorValues: p.cursorValues,
        markers: clusterMarkers(p.log, p.win.t0, p.win.t1, pr.w),
        box,
      });
    });
  });

  // Clearing the id matters: StrictMode remounts after this cleanup, and a stale id
  // would make every later draw think a frame is already pending.
  useEffect(() => () => {
    cancelAnimationFrame(rafRef.current);
    rafRef.current = 0;
  }, []);

  /** Pointer x relative to the plot's left edge. */
  const localX = (clientX: number) => {
    const r = canvasRef.current?.getBoundingClientRect();
    return r ? clientX - r.left - PAD.l : 0;
  };
  const tAt = (px: number) => win.t0 + (px / plot.w) * span;
  const inPlotX = (px: number) => px >= 0 && px <= plot.w;

  // Wheel needs a non-passive listener to stop the page scrolling while zooming.
  useEffect(() => {
    const cv = canvasRef.current;
    if (!cv) return;
    const onWheel = (ev: WheelEvent) => {
      const p = propsRef.current;
      const r = cv.getBoundingClientRect();
      const pw = plotRect(r.width, r.height).w;
      const px = ev.clientX - r.left - PAD.l;
      if (px < 0 || px > pw) return;
      ev.preventDefault();
      const dy = ev.deltaMode === 1 ? ev.deltaY * 16 : ev.deltaMode === 2 ? ev.deltaY * 400 : ev.deltaY;
      const anchor = p.win.t0 + (px / pw) * (p.win.t1 - p.win.t0);
      p.onView((v) => zoomAbout(v, p.bounds, anchor, wheelFactor(dy)));
    };
    cv.addEventListener("wheel", onWheel, { passive: false });
    return () => cv.removeEventListener("wheel", onWheel);
  }, []);

  const onPointerDown = (ev: React.PointerEvent<HTMLCanvasElement>) => {
    if (ev.button !== 0) return;
    const px = localX(ev.clientX);
    if (!inPlotX(px)) return;
    ev.currentTarget.setPointerCapture(ev.pointerId);
    setDrag(ev.shiftKey ? { mode: "box", x0: px, x1: px } : { mode: "pan", x0: px, win });
  };

  const onPointerMove = (ev: React.PointerEvent<HTMLCanvasElement>) => {
    const px = localX(ev.clientX);
    const clamped = Math.min(Math.max(px, 0), plot.w);
    if (drag?.mode === "pan") {
      const dt = -((px - drag.x0) / plot.w) * (drag.win.t1 - drag.win.t0);
      const start = drag.win;
      props.onView((v) => setWindow(v, { t0: start.t0 + dt, t1: start.t1 + dt }, bounds));
    } else if (drag?.mode === "box") {
      setDrag({ ...drag, x1: clamped });
    }
    props.onHover(inPlotX(px) || drag ? tAt(clamped) : null);
  };

  const onPointerUp = () => {
    if (drag?.mode === "box" && Math.abs(drag.x1 - drag.x0) > 4) {
      const a = tAt(Math.min(drag.x0, drag.x1));
      const b = tAt(Math.max(drag.x0, drag.x1));
      props.onView((v) => setWindow(v, { t0: a, t1: b }, bounds));
    }
    setDrag(null);
  };

  const onKeyDown = (ev: React.KeyboardEvent<HTMLCanvasElement>) => {
    const centre = props.hoverT !== null && props.hoverT >= win.t0 && props.hoverT <= win.t1 ? props.hoverT : (win.t0 + win.t1) / 2;
    const step = span * (ev.shiftKey ? 0.5 : KEY_PAN_FRACTION);
    let update: ((v: ViewState) => ViewState) | null = null;
    // Esc cancels a box zoom in progress; stopping it here keeps the App from closing the page.
    if (ev.key === "Escape" && drag) {
      ev.preventDefault();
      ev.stopPropagation();
      setDrag(null);
      return;
    }
    if (ev.key === "ArrowLeft") update = (v) => panBy(v, bounds, -step);
    else if (ev.key === "ArrowRight") update = (v) => panBy(v, bounds, step);
    else if (ev.key === "+" || ev.key === "=") update = (v) => zoomAbout(v, bounds, centre, KEY_ZOOM_IN);
    else if (ev.key === "-" || ev.key === "_") update = (v) => zoomAbout(v, bounds, centre, 1 / KEY_ZOOM_IN);
    else if (ev.key === "Enter") update = goLive;
    if (update) {
      ev.preventDefault();
      props.onView(update);
    }
  };

  const groupId = cardId(props.group);
  return (
    <section className={`tr-card${props.highlight ? " tr-card-focus" : ""}`} id={groupId} aria-labelledby={`${groupId}-title`}>
      <header className="tr-card-head">
        <h2 className="tr-card-title" id={`${groupId}-title`}>
          {props.group.toUpperCase()}
        </h2>
        <div className="tr-legend" role="group" aria-label={`${props.group} series`}>
          {props.channels.map((c) => {
            const on = !props.hidden.has(c.id);
            const value = fmtValue(props.cursorValues[c.id], c.digits);
            return (
              <button
                type="button"
                key={c.id}
                className="tr-chip"
                aria-pressed={on}
                title={on ? `Hide ${c.label}` : `Show ${c.label}`}
                style={{ "--chip": c.color } as React.CSSProperties}
                onClick={() => props.onToggleSeries(c.id)}
              >
                <span className="tr-chip-swatch" aria-hidden="true" />
                <span className="tr-chip-label">{c.label}</span>
                <span className="tr-chip-value">{value}</span>
                <span className="tr-chip-unit">{c.unit}</span>
              </button>
            );
          })}
        </div>
        <button
          type="button"
          className={`btn btn-amber tr-btn tr-ylock${props.locked ? " on" : ""}`}
          aria-pressed={props.locked !== null}
          title={props.locked ? "Y scale locked; click to auto-fit" : "Y scale auto-fits the window; click to lock"}
          onClick={() => props.onLock(props.locked ? null : { ...scalesRef.current })}
        >
          Y {props.locked ? "LOCK" : "AUTO"}
        </button>
      </header>
      <div className="tr-canvas-wrap" ref={wrapRef}>
        <canvas
          ref={canvasRef}
          className={`tr-canvas${drag?.mode === "pan" ? " panning" : ""}`}
          tabIndex={0}
          role="img"
          aria-label={`${props.group} trend, ${fmtClock(Math.max(0, win.t0))} to ${fmtClock(Math.max(0, win.t1))}. ${visible
            .map((c) => `${c.label} ${fmtValue(props.cursorValues[c.id], c.digits)} ${c.unit}`)
            .join(", ")}. Arrow keys pan, plus and minus zoom, Enter returns to live.`}
          onPointerDown={onPointerDown}
          onPointerMove={onPointerMove}
          onPointerUp={onPointerUp}
          onPointerCancel={() => setDrag(null)}
          onPointerLeave={() => !drag && props.onHover(null)}
          onDoubleClick={() => props.onView(goLive)}
          onKeyDown={onKeyDown}
        />
        <div className="tr-lane" style={{ left: PAD.l, width: plot.w, top: PAD.t + plot.h + LANE.top, height: LANE.height }}>
          {markers.map((m) => (
            <button
              type="button"
              key={m.primary.seq}
              className={`tr-marker tone-${m.tone}`}
              style={{ left: m.x, "--tone": TONE_COLOR[m.tone] } as React.CSSProperties}
              aria-label={`${fmtClock(m.primary.simTimeS)} ${m.primary.text}${m.entries.length > 1 ? ` and ${m.entries.length - 1} more` : ""}. Open in log`}
              onMouseEnter={() => setTip(m)}
              onMouseLeave={() => setTip(null)}
              onFocus={() => setTip(m)}
              onBlur={() => setTip(null)}
              onClick={() => props.onOpenLog(m.primary.seq)}
            />
          ))}
        </div>
        {tip && (
          <div
            className="tr-tip"
            role="tooltip"
            style={{ left: Math.min(Math.max(PAD.l + tip.x - 140, 4), Math.max(4, size.w - 284)), bottom: size.h - (PAD.t + plot.h) + 2 }}
          >
            {tip.entries.slice(0, MAX_TIP_ROWS).map((e) => (
              <div key={e.seq} className={`tr-tip-row tone-${e === tip.primary ? tip.tone : ""}`}>
                <span className="tr-tip-time">{fmtClock(e.simTimeS)}</span>
                <span>{e.text}</span>
              </div>
            ))}
            {tip.entries.length > MAX_TIP_ROWS && <div className="tr-tip-more">+{tip.entries.length - MAX_TIP_ROWS} more</div>}
            <div className="tr-tip-hint">Click to open in LOG</div>
          </div>
        )}
      </div>
    </section>
  );
}
