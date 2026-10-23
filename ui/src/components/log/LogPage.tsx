import { useCallback, useDeferredValue, useEffect, useMemo, useState, useSyncExternalStore } from "react";
import type { RunStore } from "../../sim/logic/runStore.ts";
import type { LogEntry } from "../../sim/logic/log.ts";
import { Btn } from "../common/Button";
import { LogTable } from "./LogTable";
import type { ScrollRequest } from "./LogTable";
import type { LogCategory, LogRow } from "./logView.ts";
import { LOG_CATEGORIES, buildLogView, categoryOf, findRowIndex, logFingerprint, logToCsv, rowAnchor } from "./logView.ts";
import "./log.css";

const ALL_CATEGORIES: ReadonlySet<LogCategory> = new Set(LOG_CATEGORIES.map((c) => c.id));

/**
 * Full-screen event log: every operator action and annunciator transition of the run.
 * The host overlay owns dialog semantics, the focus trap and Esc-to-close.
 */
export function LogPage(props: { store: RunStore; focusSeq?: number; onClose: () => void; onOpenTrends: (timeS: number) => void }) {
  const { store, focusSeq, onClose, onOpenTrends } = props;

  // Re-render only when the log content changes, not on every snapshot tick.
  const fingerprint = useSyncExternalStore(store.subscribe, () => `${store.getSnapshot().runId}|${logFingerprint(store.log.entries)}`);
  const log = store.log;

  const [categories, setCategories] = useState<ReadonlySet<LogCategory>>(ALL_CATEGORIES);
  const [query, setQuery] = useState("");
  const deferredQuery = useDeferredValue(query);
  const [expanded, setExpanded] = useState<ReadonlySet<number>>(() => new Set());
  const [follow, setFollow] = useState(focusSeq === undefined);
  const [activeKey, setActiveKey] = useState<string | null>(null);
  const [highlightKey, setHighlightKey] = useState<string | null>(null);
  const [scrollRequest, setScrollRequest] = useState<ScrollRequest | null>(null);
  const [pendingFocus, setPendingFocus] = useState<number | null>(null);

  const view = useMemo(
    () => buildLogView(log.entries, { categories, query: deferredQuery, expanded }),
    // fingerprint stands in for the in-place-mutated entries array
    [log, fingerprint, categories, deferredQuery, expanded],
  );

  // Focus request: make the entry visible (chip on, search cleared, group open), then scroll once rows exist.
  useEffect(() => {
    if (focusSeq === undefined) return;
    const entry = log.entries.find((e) => e.seq === focusSeq);
    if (!entry) return;
    setCategories((prev) => (prev.has(categoryOf(entry.kind)) ? prev : new Set([...prev, categoryOf(entry.kind)])));
    setQuery("");
    if (entry.group !== undefined) {
      const groupId = entry.group;
      setExpanded((prev) => (prev.has(groupId) ? prev : new Set([...prev, groupId])));
    }
    setFollow(false);
    setPendingFocus(focusSeq);
  }, [focusSeq, log]);

  useEffect(() => {
    if (pendingFocus === null || deferredQuery !== query) return;
    const index = findRowIndex(view.rows, pendingFocus);
    if (index < 0) return;
    const key = view.rows[index].key;
    setActiveKey(key);
    setHighlightKey(key);
    setScrollRequest((prev) => ({ index, nonce: (prev?.nonce ?? 0) + 1 }));
    setPendingFocus(null);
  }, [pendingFocus, view, deferredQuery, query]);

  const toggleCategory = (id: LogCategory) =>
    setCategories((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });

  const toggleGroup = useCallback((groupId: number, expand: boolean) => {
    setExpanded((prev) => {
      if (prev.has(groupId) === expand) return prev;
      const next = new Set(prev);
      if (expand) next.add(groupId);
      else next.delete(groupId);
      return next;
    });
  }, []);

  const openRow = useCallback((row: LogRow) => onOpenTrends(rowAnchor(row).simTimeS), [onOpenTrends]);

  const exportCsv = () => downloadCsv(log.entries, store.getSnapshot().runScenario);

  const total = log.entries.length;
  const emptyText =
    total === 0
      ? "No events yet. Operator actions and annunciator alarms appear here as they happen."
      : categories.size === 0
        ? "All filters are off. Turn on a filter above to see entries."
        : "No entries match the current filters and search.";

  return (
    <section className="log-page" aria-labelledby="log-title">
      <header className="log-bar">
        <div className="log-title-block">
          <h1 id="log-title" className="log-title">
            EVENT LOG
          </h1>
          <span className="log-stat">
            <b>{view.matched.toLocaleString()}</b> shown · {total.toLocaleString()} total
          </span>
        </div>
        <div className="log-bar-actions">
          <Btn tone="plain" onClick={exportCsv} disabled={total === 0} title="Download every entry as CSV (groups expanded)">
            EXPORT CSV
          </Btn>
          <Btn tone="plain" onClick={onClose} title="Close (Esc)">
            CLOSE
          </Btn>
        </div>
      </header>

      <div className="log-tools">
        <div className="log-chips" role="group" aria-label="Filter by event type">
          {LOG_CATEGORIES.map((c) => {
            const on = categories.has(c.id);
            return (
              <button
                key={c.id}
                type="button"
                className={`log-chip chip-${c.id}${on ? " on" : ""}`}
                aria-pressed={on}
                onClick={() => toggleCategory(c.id)}
              >
                <span className="log-chip-lamp" aria-hidden="true" />
                {c.label}
                <span className="log-chip-count">{view.counts[c.id].toLocaleString()}</span>
              </button>
            );
          })}
        </div>
        <label className="log-search">
          <span className="log-search-label">SEARCH</span>
          <input
            type="search"
            value={query}
            placeholder="text in event…"
            spellCheck={false}
            onChange={(ev) => setQuery(ev.target.value)}
            onKeyDown={(ev) => {
              // First Esc clears the search; the next one closes the page (the overlay's
              // focus trap skips an Esc whose default was prevented).
              if (ev.key === "Escape" && query !== "") {
                ev.preventDefault();
                setQuery("");
              }
            }}
          />
        </label>
      </div>

      <div className="log-body">
        <LogTable
          rows={view.rows}
          emptyText={emptyText}
          activeKey={activeKey}
          highlightKey={highlightKey}
          follow={follow}
          scrollRequest={scrollRequest}
          onActiveChange={setActiveKey}
          onFollowChange={setFollow}
          onOpen={openRow}
          onToggle={toggleGroup}
        />
        {!follow && view.rows.length > 0 && (
          <button type="button" className="log-jump" onClick={() => setFollow(true)}>
            ▼ JUMP TO LATEST
          </button>
        )}
      </div>

      <footer className="log-foot">
        <span className={`log-follow${follow ? " on" : ""}`}>
          <span className="log-follow-lamp" aria-hidden="true" />
          {follow ? "FOLLOWING NEWEST" : "PAUSED ON HISTORY"}
        </span>
        <span className="log-hint">↑↓ select · Enter or click: trends at that time · ←→ / Space: open or close group · Esc: close</span>
      </footer>
    </section>
  );
}

function downloadCsv(entries: readonly LogEntry[], scenario: string | null): void {
  const blob = new Blob([logToCsv(entries)], { type: "text/csv;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const stamp = new Date().toISOString().slice(0, 16).replace(/[-:T]/g, "");
  const a = document.createElement("a");
  a.href = url;
  a.download = `bwrsim-log-${scenario ?? "run"}-${stamp}.csv`;
  document.body.appendChild(a);
  a.click();
  a.remove();
  // Revoke after the click has been dispatched; some browsers read the URL asynchronously.
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}
