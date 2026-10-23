import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import type { KeyboardEvent, WheelEvent } from "react";
import type { LogKind } from "../../sim/logic/log.ts";
import type { LogRow } from "./logView.ts";
import { KIND_LABEL, formatSimTime, formatWallTime, rowAnchor } from "./logView.ts";

/** Fixed row height is what makes windowing a pure index calculation. Keep in sync with log.css. */
export const ROW_H = 26;
const OVERSCAN = 8;

const KIND_ICON: Record<LogKind, string> = {
  action: "▸",
  alarm: "▲",
  clear: "✓",
  changed: "◆",
  system: "●",
};

export interface ScrollRequest {
  index: number;
  /** Bumped per request so repeating the same index still scrolls. */
  nonce: number;
}

export interface LogTableProps {
  rows: LogRow[];
  emptyText: string;
  activeKey: string | null;
  highlightKey: string | null;
  follow: boolean;
  scrollRequest: ScrollRequest | null;
  onActiveChange: (key: string) => void;
  onFollowChange: (follow: boolean) => void;
  onOpen: (row: LogRow) => void;
  onToggle: (groupId: number, expand: boolean) => void;
}

/** Virtualized event table (treegrid: a setpoint group is a level-1 row with its actions at level 2). */
export function LogTable(props: LogTableProps) {
  const { rows, follow, onFollowChange } = props;
  const gridRef = useRef<HTMLDivElement>(null);
  const viewportRef = useRef<HTMLDivElement>(null);
  const [scrollTop, setScrollTop] = useState(0);
  const [viewportH, setViewportH] = useState(600);

  useLayoutEffect(() => {
    const el = viewportRef.current;
    if (!el) return;
    setViewportH(el.clientHeight);
    const ro = new ResizeObserver(() => setViewportH(el.clientHeight));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  // Auto-follow: pin to the newest row whenever rows grow (or follow resumes).
  useLayoutEffect(() => {
    const el = viewportRef.current;
    if (follow && el) {
      el.scrollTop = el.scrollHeight;
      setScrollTop(el.scrollTop);
    }
  }, [follow, rows.length]);

  useLayoutEffect(() => {
    const el = viewportRef.current;
    const req = props.scrollRequest;
    if (!el || !req) return;
    el.scrollTop = Math.max(0, req.index * ROW_H - el.clientHeight / 2 + ROW_H / 2);
    setScrollTop(el.scrollTop);
  }, [props.scrollRequest]);

  const onScroll = () => {
    const el = viewportRef.current;
    if (!el) return;
    setScrollTop(el.scrollTop);
    const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight <= ROW_H / 2;
    if (atBottom !== follow) onFollowChange(atBottom);
  };

  const activeIndex = useMemo(
    () => (props.activeKey === null ? -1 : rows.findIndex((r) => r.key === props.activeKey)),
    [rows, props.activeKey],
  );

  const ensureVisible = useCallback((index: number) => {
    const el = viewportRef.current;
    if (!el) return;
    const top = index * ROW_H;
    if (top < el.scrollTop) el.scrollTop = top;
    else if (top + ROW_H > el.scrollTop + el.clientHeight) el.scrollTop = top + ROW_H - el.clientHeight;
  }, []);

  // The page opens with the keyboard ready on the table.
  useEffect(() => {
    gridRef.current?.focus({ preventScroll: true });
  }, []);

  const pageRows = Math.max(1, Math.floor(viewportH / ROW_H) - 1);

  const moveTo = (index: number) => {
    if (rows.length === 0) return;
    const clamped = Math.min(rows.length - 1, Math.max(0, index));
    props.onActiveChange(rows[clamped].key);
    ensureVisible(clamped);
    if (clamped === rows.length - 1) onFollowChange(true);
  };

  const onKeyDown = (ev: KeyboardEvent<HTMLDivElement>) => {
    const current = activeIndex >= 0 ? activeIndex : rows.length - 1;
    const row = rows[current] as LogRow | undefined;
    switch (ev.key) {
      case "ArrowDown":
        moveTo(activeIndex < 0 ? rows.length - 1 : current + 1);
        break;
      case "ArrowUp":
        moveTo(current - 1);
        break;
      case "PageDown":
        moveTo(current + pageRows);
        break;
      case "PageUp":
        moveTo(current - pageRows);
        break;
      case "Home":
        moveTo(0);
        break;
      case "End":
        moveTo(rows.length - 1);
        break;
      case "Enter":
        if (row) props.onOpen(row);
        break;
      case "ArrowRight":
        if (row?.type === "group" && !row.expanded) props.onToggle(row.groupId, true);
        break;
      case "ArrowLeft":
        if (row?.type === "group" && row.expanded) props.onToggle(row.groupId, false);
        else if (row?.type === "entry" && row.child && row.entry.group !== undefined) {
          const parent = rows.findIndex((r) => r.type === "group" && r.groupId === row.entry.group);
          if (parent >= 0) moveTo(parent);
        }
        break;
      case " ":
        if (row?.type === "group") props.onToggle(row.groupId, !row.expanded);
        break;
      default:
        return;
    }
    ev.preventDefault();
  };

  // Programmatic scrolls set scrollTop; a wheel-up is the operator's intent to leave follow
  // immediately, before the next append can snap the view back down.
  const onWheel = (ev: WheelEvent) => {
    if (ev.deltaY < 0 && follow) onFollowChange(false);
  };

  const first = Math.max(0, Math.floor(scrollTop / ROW_H) - OVERSCAN);
  const last = Math.min(rows.length, Math.ceil((scrollTop + viewportH) / ROW_H) + OVERSCAN);
  const visible = rows.slice(first, last);
  // Only point at a row that is actually in the DOM (the window may have scrolled away from it).
  const activeId = activeIndex >= first && activeIndex < last ? rowDomId(rows[activeIndex].key) : undefined;

  return (
    <div
      ref={gridRef}
      className="log-grid"
      role="treegrid"
      aria-label="Event log"
      aria-rowcount={rows.length + 1}
      aria-activedescendant={activeId}
      tabIndex={0}
      onKeyDown={onKeyDown}
    >
      <div className="log-row log-head" role="row" aria-rowindex={1}>
        <span role="columnheader">SIM TIME</span>
        <span role="columnheader" className="log-col-wall">
          WALL
        </span>
        <span role="columnheader">TYPE</span>
        <span role="columnheader" className="log-col-sev">
          SEVERITY
        </span>
        <span role="columnheader">EVENT</span>
      </div>
      <div ref={viewportRef} className="log-viewport" onScroll={onScroll} onWheel={onWheel}>
        {rows.length === 0 ? (
          <div className="log-empty">{props.emptyText}</div>
        ) : (
          <div className="log-spacer" role="rowgroup" style={{ height: rows.length * ROW_H }}>
            {visible.map((row, i) => (
              <LogRowView
                key={row.key}
                row={row}
                index={first + i}
                active={first + i === activeIndex}
                highlight={row.key === props.highlightKey}
                onClick={() => {
                  props.onActiveChange(row.key);
                  gridRef.current?.focus({ preventScroll: true });
                  props.onOpen(row);
                }}
                onToggle={props.onToggle}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

function rowDomId(key: string): string {
  return `log-row-${key}`;
}

function LogRowView(props: {
  row: LogRow;
  index: number;
  active: boolean;
  highlight: boolean;
  onClick: () => void;
  onToggle: (groupId: number, expand: boolean) => void;
}) {
  const { row } = props;
  const anchor = rowAnchor(row);
  const isGroup = row.type === "group";
  const level = row.type === "entry" && row.child ? 2 : 1;
  const classes = [
    "log-row",
    `sev-${anchor.severity}`,
    `kind-${anchor.kind}`,
    level === 2 ? "is-child" : "",
    props.active ? "is-active" : "",
    props.highlight ? "is-focus" : "",
  ].join(" ");
  return (
    <div
      id={rowDomId(row.key)}
      className={classes}
      role="row"
      aria-rowindex={props.index + 2}
      aria-level={level}
      aria-selected={props.active}
      aria-expanded={isGroup ? row.expanded : undefined}
      style={{ transform: `translateY(${props.index * ROW_H}px)` }}
      onClick={props.onClick}
      title="Open trends at this time"
    >
      <span role="gridcell" className="log-time">
        {formatSimTime(anchor.simTimeS)}
      </span>
      <span role="gridcell" className="log-time log-col-wall" title={new Date(anchor.wallMs).toLocaleString()}>
        {formatWallTime(anchor.wallMs)}
      </span>
      <span role="gridcell" className="log-kind">
        <span className="log-kind-icon" aria-hidden="true">
          {KIND_ICON[anchor.kind]}
        </span>
        {KIND_LABEL[anchor.kind]}
      </span>
      <span role="gridcell" className="log-sev log-col-sev">
        {anchor.severity.toUpperCase()}
      </span>
      <span role="gridcell" className="log-text">
        {isGroup ? (
          <>
            <button
              type="button"
              className="log-caret"
              tabIndex={-1}
              aria-label={row.expanded ? "Collapse group" : "Expand group"}
              aria-expanded={row.expanded}
              onClick={(ev) => {
                ev.stopPropagation();
                props.onToggle(row.groupId, !row.expanded);
              }}
            >
              {row.expanded ? "▾" : "▸"}
            </button>
            <span className="log-text-body">{row.text}</span>
            <span className="log-badge">×{row.entries.length}</span>
          </>
        ) : (
          <span className="log-text-body">{row.entry.text}</span>
        )}
      </span>
    </div>
  );
}
