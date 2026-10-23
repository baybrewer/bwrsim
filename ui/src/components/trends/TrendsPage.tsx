import { useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import { CHANNELS, CHANNEL_GROUPS } from "../../sim/logic/channels.ts";
import type { RunStore } from "../../sim/logic/runStore.ts";
import { fmtClock } from "../../util/format.ts";
import { spanLabel } from "./axis.ts";
import { ChartCard, cardId } from "./ChartCard";
import type { Scales } from "./drawChart.ts";
import { csvFileName, queryCsv } from "./csv.ts";
import { historyStart, valuesAt } from "./historyView.ts";
import { PRESETS, PRESET_TITLES, centreOn, goLive, initialView, resolveWindow, selectPreset } from "./viewWindow.ts";
import type { Bounds, ViewState } from "./viewWindow.ts";
import "./trends.css";

/** Export rows: one per second at most, capped so a whole-run export stays a sane size. */
const CSV_MAX_ROWS = 5000;
/** Cursor readout looks back this many pixel-widths (about) for the last sample. */
const READOUT_TOL_FRACTION = 1 / 400;
const FOCUS_HIGHLIGHT_MS = 2400;

type Columns = 1 | 2;

function downloadText(text: string, fileName: string): void {
  const url = URL.createObjectURL(new Blob([text], { type: "text/csv;charset=utf-8" }));
  const a = document.createElement("a");
  a.href = url;
  a.download = fileName;
  document.body.appendChild(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

export function TrendsPage(props: { store: RunStore; focusGroup?: string; focusTimeS?: number; onClose: () => void; onOpenLog: (seq: number) => void }) {
  const { store } = props;
  const run = useSyncExternalStore(store.subscribe, store.getSnapshot);
  const history = store.history;
  const lastT = history.lastT;
  const now = Math.max(lastT ?? 0, run.snapshot?.timeS ?? 0);
  const bounds: Bounds = { start: historyStart(history) ?? now, now };
  const boundsRef = useRef(bounds);
  boundsRef.current = bounds;

  const [view, setView] = useState<ViewState>(() =>
    props.focusTimeS !== undefined ? centreOn(initialView(), bounds, props.focusTimeS) : initialView(),
  );
  const [columns, setColumns] = useState<Columns>(2);
  const [hidden, setHidden] = useState<ReadonlySet<string>>(() => new Set());
  const [locks, setLocks] = useState<Record<string, Scales>>({});
  const [hoverT, setHoverT] = useState<number | null>(null);
  const [highlight, setHighlight] = useState<string | null>(null);
  const titleRef = useRef<HTMLHeadingElement>(null);

  const win = resolveWindow(view, bounds);
  const span = win.t1 - win.t0;

  // A new run (scenario or load) starts from LIVE on the same preset.
  const runIdRef = useRef(run.runId);
  useEffect(() => {
    if (runIdRef.current === run.runId) return;
    runIdRef.current = run.runId;
    setView((v) => selectPreset(v, v.preset));
    setLocks({});
  }, [run.runId]);

  const firstFocusTime = useRef(true);
  useEffect(() => {
    // The initial focus time is applied in the state initializer.
    if (firstFocusTime.current) {
      firstFocusTime.current = false;
      return;
    }
    if (props.focusTimeS !== undefined) setView((v) => centreOn(v, boundsRef.current, props.focusTimeS as number));
  }, [props.focusTimeS]);

  useEffect(() => {
    const group = props.focusGroup;
    if (!group) {
      titleRef.current?.focus();
      return;
    }
    const el = document.getElementById(cardId(group));
    const reduceMotion = window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;
    el?.scrollIntoView({ block: "start", behavior: reduceMotion ? "auto" : "smooth" });
    el?.querySelector<HTMLCanvasElement>("canvas")?.focus({ preventScroll: true });
    setHighlight(group);
    const timer = setTimeout(() => setHighlight(null), FOCUS_HIGHLIGHT_MS);
    return () => clearTimeout(timer);
  }, [props.focusGroup]);

  // Readout time: the cursor, else the right edge of the window (latest when LIVE).
  const readoutT = hoverT ?? Math.min(win.t1, now);
  const tol = Math.max(1.5, span * READOUT_TOL_FRACTION);
  const cursorValues = useMemo(
    () => valuesAt(history, readoutT, tol, CHANNELS.map((c) => c.id)),
    // run.version: new samples can change the value at the same time.
    [history, readoutT, tol, run.version],
  );

  const toggleSeries = (id: string) =>
    setHidden((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });

  const exportCsv = () => {
    const rows = Math.max(1, Math.min(CSV_MAX_ROWS, Math.ceil(span)));
    const text = queryCsv(history.query(win.t0, win.t1, rows, CHANNELS.map((c) => c.id)), CHANNELS);
    downloadText(text, csvFileName(Math.max(0, win.t0), Math.max(0, win.t1)));
  };

  const empty = lastT === null;
  return (
    <div className="trends">
      <header className="tr-head">
        <div className="tr-head-id">
          <h1 className="tr-title" ref={titleRef} tabIndex={-1}>
            TRENDS
          </h1>
          <div className="tr-clock" aria-label="Sim time">
            <span className="tr-k">SIM</span> {fmtClock(now)}
          </div>
        </div>

        <div className="tr-range" role="group" aria-label="Time range">
          {PRESETS.map((p) => (
            <button
              type="button"
              key={p}
              className={`btn btn-green tr-btn tr-preset${view.preset === p ? (view.live ? " on" : " held") : ""}`}
              aria-pressed={view.live && view.preset === p}
              title={`${PRESET_TITLES[p]}${p === "ALL" ? " (keeps expanding)" : ""}`}
              onClick={() => setView((v) => selectPreset(v, p))}
            >
              {p}
            </button>
          ))}
          <button
            type="button"
            className={`btn btn-green tr-btn tr-live${view.live ? " on" : ""}`}
            aria-pressed={view.live}
            title={view.live ? "Following the newest data" : "Resume following the newest data"}
            onClick={() => setView(goLive)}
          >
            <span className="tr-live-dot" aria-hidden="true" />
            {view.live ? "LIVE" : "GO LIVE"}
          </button>
        </div>

        <div className="tr-window" aria-live="off">
          <div>
            <span className="tr-k">WINDOW</span> {fmtClock(Math.max(0, win.t0))} → {fmtClock(Math.max(0, win.t1))}
          </div>
          <div>
            <span className="tr-k">SPAN</span> {spanLabel(span)}
            <span className="tr-k tr-cursor-k">CURSOR</span> {hoverT === null ? "—" : fmtClock(hoverT)}
          </div>
        </div>

        <div className="tr-tools">
          <div className="tr-seg" role="group" aria-label="Layout">
            {([1, 2] as const).map((n) => (
              <button
                type="button"
                key={n}
                className={`btn btn-green tr-btn${columns === n ? " on" : ""}`}
                aria-pressed={columns === n}
                aria-label={`${n} column${n > 1 ? "s" : ""}`}
                onClick={() => setColumns(n)}
              >
                {n} COL
              </button>
            ))}
          </div>
          <button type="button" className="btn tr-btn" onClick={exportCsv} disabled={empty} title="Download the visible time range as CSV">
            EXPORT CSV
          </button>
          <button type="button" className="btn tr-btn tr-close" onClick={props.onClose} aria-label="Close trends">
            CLOSE ✕
          </button>
        </div>
        <p className="tr-hint">
          Wheel zoom · drag pan · shift-drag box zoom · double-click back to {view.preset} live · focused chart: ←/→ pan, +/− zoom, Enter live ·
          markers open the LOG
        </p>
      </header>

      {empty && <div className="tr-empty">No trend data yet: history starts with the first sample.</div>}

      <div className={`tr-grid cols-${columns}`}>
        {CHANNEL_GROUPS.map((group) => (
          <ChartCard
            key={group}
            group={group}
            channels={CHANNELS.filter((c) => c.group === group)}
            history={history}
            log={store.log.byTime}
            version={run.version}
            win={win}
            bounds={bounds}
            live={view.live}
            hidden={hidden}
            onToggleSeries={toggleSeries}
            locked={locks[group] ?? null}
            onLock={(scales) =>
              setLocks((prev) => {
                const next = { ...prev };
                if (scales) next[group] = scales;
                else delete next[group];
                return next;
              })
            }
            hoverT={hoverT}
            onHover={setHoverT}
            cursorValues={cursorValues}
            onView={(fn) => setView((v) => fn(v))}
            onOpenLog={props.onOpenLog}
            highlight={highlight === group}
          />
        ))}
      </div>
    </div>
  );
}

