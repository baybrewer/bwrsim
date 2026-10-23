import { useCallback, useEffect, useId, useRef, useState } from "react";
import type { FormEvent, KeyboardEvent } from "react";
import type { RunStore, SaveSession } from "../../sim/logic/runStore.ts";
import { toError } from "../../sim/logic/protocol.ts";
import { MODE_LABELS, SCENARIO_LABELS } from "../../sim/logic/labels.ts";
import type { SaveList, SaveMeta } from "../../sim/types";
import { fmtClock, fx } from "../../util/format";
import { Btn } from "../common/Button";
import { useFocusTrap } from "../common/useFocusTrap";
import { isStaleSave, slotStatus } from "./slotStatus";

export type SaveTab = "save" | "load";

// Mirrors the host's name rule (trimmed, 1-40 chars); the host remains the authority.
const NAME_MAX = 40;

type Pending = { kind: "overwrite" | "load" | "delete"; id: string } | null;
type SessionState = { status: "none" | "opening" } | { status: "ready"; session: SaveSession } | { status: "failed"; error: string };
type DialogError = { message: string; reopen: boolean } | null;

const slotDomId = (id: string) => `save-slot-${id}`;
const tabDomId = (base: string, tab: SaveTab) => `${base}-tab-${tab}`;
const fmtSavedAt = (ms: number) => (ms > 0 ? new Date(ms).toLocaleString() : "—");

/**
 * Save / Load dialog (plan C6, A1-A2). The Save tab holds a save session: the sim is paused at
 * a checkpoint from the moment the tab opens until the dialog closes, when the prior run state
 * is restored (unless a load replaced the run).
 */
/** `onClose` gets a one-line result notice after a save or load, so the outcome stays visible. */
export function SaveDialog(props: { store: RunStore; initialTab: SaveTab; onClose: (notice?: string) => void }) {
  const { store } = props;
  const titleId = useId();
  const rootRef = useRef<HTMLDivElement>(null);
  const [tab, setTab] = useState<SaveTab>(props.initialTab);
  const [list, setList] = useState<SaveList | null>(null);
  const [listError, setListError] = useState<string | null>(null);
  const [session, setSession] = useState<SessionState>({ status: "none" });
  const [pending, setPending] = useState<Pending>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<DialogError>(null);
  const [newName, setNewName] = useState("");

  // The live session promise; a ref so StrictMode's double effect and late replies see one session.
  const sessionP = useRef<Promise<SaveSession> | null>(null);
  const loadedRef = useRef(false);

  const refreshList = useCallback(async () => {
    try {
      setList(await store.listSaves());
      setListError(null);
    } catch (e) {
      setListError(toError(e).message);
    }
  }, [store]);

  /** Opens (or re-opens) the checkpoint; `keepWasPaused` carries the original pre-Save state. */
  const openSession = useCallback(
    (keepWasPaused?: boolean) => {
      const p = store.beginSave().then((s) => (keepWasPaused === undefined ? s : { ...s, wasPaused: keepWasPaused }));
      sessionP.current = p;
      setSession({ status: "opening" });
      p.then(
        (s) => {
          if (sessionP.current === p) setSession({ status: "ready", session: s });
        },
        (e: unknown) => {
          if (sessionP.current !== p) return;
          sessionP.current = null;
          setSession({ status: "failed", error: toError(e).message });
        },
      );
    },
    [store],
  );

  useEffect(() => {
    void refreshList();
    if (props.initialTab === "save" && !sessionP.current) openSession();
    // Mount-only: tab switches open the session from the click handler.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const close = useCallback(
    (notice?: string) => {
      const p = sessionP.current;
      sessionP.current = null;
      // A2: restore the run state from before Save, unless a load replaced the run.
      if (p && !loadedRef.current) void p.then((s) => store.finishSave(s)).catch(() => undefined);
      props.onClose(notice);
    },
    [store, props],
  );

  const cancelPending = useCallback(() => {
    if (!pending) return;
    const id = pending.id;
    setPending(null);
    // Return focus to the slot whose confirm was dismissed.
    requestAnimationFrame(() => document.getElementById(slotDomId(id))?.focus());
  }, [pending]);

  useFocusTrap(rootRef, () => {
    if (busy) return;
    if (pending) cancelPending();
    else close();
  });

  // Move focus to the confirm action as soon as a slot is armed.
  useEffect(() => {
    if (pending) rootRef.current?.querySelector<HTMLElement>(".confirm-go")?.focus();
  }, [pending]);

  const switchTab = (next: SaveTab) => {
    if (next === tab) return;
    setTab(next);
    setPending(null);
    setError(null);
    if (next === "save" && !sessionP.current) openSession();
  };

  // WAI-ARIA tabs: arrow keys move between SAVE and LOAD (roving tabindex).
  const onTabKey = (e: KeyboardEvent) => {
    if (e.key !== "ArrowLeft" && e.key !== "ArrowRight" && e.key !== "Home" && e.key !== "End") return;
    e.preventDefault();
    // Home/End select the first/last tab; only the arrows toggle. A stray End on LOAD must
    // not open SAVE, which would pause the sim for a checkpoint.
    const next: SaveTab = e.key === "Home" ? "save" : e.key === "End" ? "load" : tab === "save" ? "load" : "save";
    if (next === tab) return;
    switchTab(next);
    requestAnimationFrame(() => document.getElementById(tabDomId(titleId, next))?.focus());
  };

  const run = async (op: () => Promise<void>) => {
    setBusy(true);
    setError(null);
    try {
      await op();
    } catch (e) {
      const message = toError(e).message;
      setError({ message, reopen: tab === "save" && isStaleSave(message) });
      void refreshList();
    } finally {
      setBusy(false);
    }
  };

  const saveTo = (id: string | null, name: string) =>
    run(async () => {
      if (session.status !== "ready") throw new Error("the save checkpoint is not open");
      await store.commitSave(session.session, id, name);
      close(`Saved "${name}" at sim time ${fmtClock(session.session.snapshot.timeS)}.`);
    });

  const loadSlot = (meta: SaveMeta) =>
    run(async () => {
      await store.load(meta.id, meta.name);
      loadedRef.current = true;
      close(`Loaded "${meta.name}". The run is paused; press RUN to continue.`);
    });

  const deleteSlot = (meta: SaveMeta) =>
    run(async () => {
      await store.deleteSave(meta.id);
      setPending(null);
      await refreshList();
    });

  const reopen = () => {
    const prior = session.status === "ready" ? session.session.wasPaused : undefined;
    setError(null);
    setPending(null);
    openSession(prior);
  };

  const confirm = () => {
    const meta = list?.saves.find((m) => m.id === pending?.id);
    if (!pending || !meta) return;
    if (pending.kind === "overwrite") void saveTo(meta.id, meta.name);
    else if (pending.kind === "load") void loadSlot(meta);
    else void deleteSlot(meta);
  };

  const sessionReady = session.status === "ready";
  const saves = list?.saves ?? [];
  const full = list !== null && saves.length >= list.max;
  const trimmed = newName.trim();
  const duplicate = saves.find((m) => slotStatus(m) !== "unreadable" && m.name.trim().toLowerCase() === trimmed.toLowerCase());
  const newReason = !list
    ? "Reading saved runs…"
    : full
      ? `All ${list.max} slots are used. Overwrite a slot or delete one.`
      : duplicate
        ? `A save named "${duplicate.name}" exists. Click its slot to overwrite it.`
        : null;
  const canSaveNew = sessionReady && !busy && !newReason && trimmed.length > 0;

  const onSubmitNew = (e: FormEvent) => {
    e.preventDefault();
    if (canSaveNew) void saveTo(null, trimmed);
  };

  /** Why a slot cannot be picked on the current tab, or null. */
  const slotBlock = (meta: SaveMeta): string | null => {
    const status = slotStatus(meta);
    if (tab === "load") return status === "ok" ? null : `Cannot load: ${meta.error ?? status}`;
    if (status === "unreadable") return "Unreadable: delete it to free the slot";
    return sessionReady ? null : "Waiting for the save checkpoint";
  };

  return (
    <div className="modal-backdrop">
      <div ref={rootRef} className="panel save-dialog" role="dialog" aria-modal="true" aria-labelledby={titleId} tabIndex={-1}>
        <header className="panel-title">
          <span id={titleId}>{tab === "save" ? "SAVE RUN" : "LOAD RUN"}</span>
          <div className="row gap4">
            <div className="row gap4" role="tablist" aria-label="Save or load" onKeyDown={onTabKey}>
              {(["save", "load"] as const).map((t) => (
                <button
                  key={t}
                  id={tabDomId(titleId, t)}
                  type="button"
                  role="tab"
                  aria-selected={tab === t}
                  aria-controls={`${titleId}-panel`}
                  tabIndex={tab === t ? 0 : -1}
                  className={`btn btn-plain small${tab === t ? " on" : ""}`}
                  disabled={busy}
                  onClick={() => switchTab(t)}
                >
                  {t.toUpperCase()}
                </button>
              ))}
            </div>
            <Btn small tone="plain" className="dlg-close" onClick={() => close()} disabled={busy} title="Close (Esc)">
              ✕ CLOSE
            </Btn>
          </div>
        </header>

        <div id={`${titleId}-panel`} className="panel-body save-body" role="tabpanel" aria-labelledby={tabDomId(titleId, tab)}>
          <SessionLine tab={tab} session={session} onRetry={() => openSession()} />

          {error && (
            <div className="dlg-error" role="alert">
              ⚠ {error.message}
              {error.reopen && (
                <Btn small tone="amber" onClick={reopen}>
                  RE-OPEN SAVE
                </Btn>
              )}
            </div>
          )}

          {listError && (
            <div className="dlg-error" role="alert">
              ⚠ Could not list saved runs: {listError}
              <Btn small tone="plain" onClick={() => void refreshList()}>
                RETRY
              </Btn>
            </div>
          )}

          <div className="slot-count tb-label">
            SLOTS {list ? `${saves.length} / ${list.max}` : "…"}
          </div>

          {list && saves.length === 0 && (
            <div className="slot-empty">{tab === "save" ? "No saved runs yet. Name this run below to create the first one." : "No saved runs yet. Use SAVE to store the current run."}</div>
          )}

          <ul className="slot-list">
            {saves.map((meta) => {
              const block = slotBlock(meta);
              const armed = pending?.id === meta.id ? pending : null;
              return (
                <li key={meta.id} className={`slot${armed ? " armed" : ""}`}>
                  <div className="slot-row">
                    <button
                      id={slotDomId(meta.id)}
                      type="button"
                      className="slot-main"
                      disabled={busy || block !== null}
                      title={block ?? (tab === "save" ? "Overwrite this save" : "Load this save")}
                      onClick={() => setPending({ kind: tab === "save" ? "overwrite" : "load", id: meta.id })}
                    >
                      <SlotSummary meta={meta} />
                      {block === null && (
                        <span className="slot-action" aria-hidden="true">
                          {tab === "save" ? "OVERWRITE ▸" : "LOAD ▸"}
                        </span>
                      )}
                    </button>
                    <Btn small tone="plain" disabled={busy} title={`Delete "${meta.name}"`} onClick={() => setPending({ kind: "delete", id: meta.id })}>
                      DELETE
                    </Btn>
                  </div>
                  {armed && (
                    <div className="slot-confirm" role="group" aria-label="Confirm">
                      <span>{confirmText(armed.kind, meta, session)}</span>
                      <Btn small tone={armed.kind === "delete" ? "red" : "amber"} active disabled={busy} onClick={confirm} className="confirm-go">
                        {busy ? "WORKING…" : armed.kind.toUpperCase()}
                      </Btn>
                      <Btn small tone="plain" disabled={busy} onClick={cancelPending}>
                        CANCEL
                      </Btn>
                    </div>
                  )}
                </li>
              );
            })}
          </ul>

          {tab === "save" && (
            <form className="new-save" onSubmit={onSubmitNew}>
              <label className="tb-label" htmlFor={`${titleId}-name`}>
                NEW SAVE NAME
              </label>
              <div className="row gap4">
                <input
                  id={`${titleId}-name`}
                  className="select new-save-input"
                  value={newName}
                  maxLength={NAME_MAX}
                  disabled={busy || full}
                  // Naming the save is the Save tab's main task; start the cursor there.
                  autoFocus={props.initialTab === "save"}
                  placeholder="e.g. 50% power, before turbine roll"
                  aria-describedby={newReason ? `${titleId}-reason` : undefined}
                  onChange={(e) => setNewName(e.target.value)}
                />
                <button type="submit" className="btn btn-green" disabled={!canSaveNew}>
                  {busy && !pending ? "SAVING…" : "SAVE NEW"}
                </button>
              </div>
              {newReason && (
                <div id={`${titleId}-reason`} className="new-save-reason">
                  {newReason}
                </div>
              )}
            </form>
          )}
        </div>
      </div>
    </div>
  );
}

function confirmText(kind: NonNullable<Pending>["kind"], meta: SaveMeta, session: SessionState): string {
  if (kind === "delete") return `Delete "${meta.name}" permanently?`;
  if (kind === "load") return `Load "${meta.name}"? The current run will be replaced; its unsaved history and log are lost.`;
  const at = session.status === "ready" ? ` at ${fmtClock(session.session.snapshot.timeS)}` : "";
  return `Overwrite "${meta.name}" with the current run${at}?`;
}

function SessionLine(props: { tab: SaveTab; session: SessionState; onRetry: () => void }) {
  const { tab, session } = props;
  if (tab === "load") {
    return <div className="session-line">Loading replaces the current run. The loaded run starts paused.</div>;
  }
  if (session.status === "ready") {
    const s = session.session;
    return (
      <div className="session-line">
        Sim paused at <span className="mono">{fmtClock(s.snapshot.timeS)}</span>
        {s.wasPaused ? " (it was already paused)." : ". It resumes when this dialog closes."}
      </div>
    );
  }
  if (session.status === "failed") {
    return (
      <div className="dlg-error" role="alert">
        ⚠ Could not open the save checkpoint: {session.error}
        <Btn small tone="amber" onClick={props.onRetry}>
          RETRY
        </Btn>
      </div>
    );
  }
  return <div className="session-line">Pausing the sim for the save checkpoint…</div>;
}

function SlotSummary(props: { meta: SaveMeta }) {
  const { meta } = props;
  const status = slotStatus(meta);
  // An unreadable file's listed fields are placeholders: show only its name, reason and file time.
  if (status === "unreadable") {
    return (
      <span className="slot-grid">
        <span className="slot-name mono">{meta.name}</span>
        <span className="slot-badges">
          <span className="badge badge-red">UNREADABLE</span>
        </span>
        <span className="slot-saved">{fmtSavedAt(meta.savedAtMs)}</span>
        <span className="slot-reason">{meta.error ?? "unreadable save file"}</span>
      </span>
    );
  }
  const r = meta.runSummary;
  return (
    <span className="slot-grid">
      <span className="slot-name">{meta.name}</span>
      <span className="slot-facts mono">
        <span title="Sim time">{fmtClock(meta.simTimeS)}</span>
        <span>{SCENARIO_LABELS[meta.scenario] ?? meta.scenario}</span>
        <span title="Mode switch">{MODE_LABELS[r.mode] ?? r.mode}</span>
        <span title="Thermal power">{fx(r.powerPct, 1)} %</span>
        <span title="Reactor pressure">{fx(r.pressureMpa, 2)} MPa</span>
      </span>
      <span className="slot-badges">
        {r.scrammed && <span className="badge badge-red">SCRAMMED</span>}
        {status === "incompatible" && <span className="badge badge-amber">INCOMPATIBLE</span>}
      </span>
      <span className="slot-saved">{fmtSavedAt(meta.savedAtMs)}</span>
      {status === "incompatible" && <span className="slot-reason">{meta.error}</span>}
    </span>
  );
}
