import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import { Annunciator } from "./components/Annunciator";
import { FeedwaterPanel } from "./components/FeedwaterPanel";
import { PlantMimic } from "./components/mimic/PlantMimic";
import { Panel } from "./components/common/Panel";
import { useFocusTrap } from "./components/common/useFocusTrap";
import { LogPage } from "./components/log/LogPage";
import { ReactorPanel } from "./components/ReactorPanel";
import { SaveDialog } from "./components/save/SaveDialog";
import type { SaveTab } from "./components/save/SaveDialog";
import { TopBar } from "./components/TopBar";
import { TrendPanel } from "./components/TrendPanel";
import { TrendsPage } from "./components/trends/TrendsPage";
import { TurbinePanel } from "./components/TurbinePanel";
import { useSim } from "./sim/bridge";

/** At most one overlay covers the control room; only Save pauses the sim (plan C6, A2). */
type Overlay =
  | { kind: "none" }
  | { kind: "trends"; focusGroup?: string; focusTimeS?: number }
  | { kind: "log"; focusSeq?: number }
  | { kind: "save"; tab: SaveTab };

const NONE: Overlay = { kind: "none" };

export default function App() {
  const { snapshot: s, send, lastError, backendKind, store } = useSim();
  const [overlay, setOverlay] = useState<Overlay>(NONE);
  const appRef = useRef<HTMLDivElement>(null);
  const covered = overlay.kind !== "none";
  // One-line result of the last save/load, shown after the dialog closes.
  const [notice, setNotice] = useState<string | null>(null);

  useEffect(() => {
    if (!notice) return;
    const timer = setTimeout(() => setNotice(null), 6000);
    return () => clearTimeout(timer);
  }, [notice]);

  // The control room stays mounted (and live) under an overlay, but out of reach of focus and AT.
  // Layout effect: lifted before the overlay's focus trap restores focus into the control room.
  useLayoutEffect(() => {
    appRef.current?.toggleAttribute("inert", covered);
  }, [covered, s === null]);

  if (!s) {
    return (
      <div className="waiting">
        <div className="lamp lamp-amber blink" />
        WAITING FOR SIMULATOR ({backendKind})…
      </div>
    );
  }

  const close = () => setOverlay(NONE);
  const openTrends = (focus: { focusGroup?: string; focusTimeS?: number } = {}) => setOverlay({ kind: "trends", ...focus });
  const openLog = (focusSeq?: number) => setOverlay({ kind: "log", focusSeq });
  const closeSave = (result?: string) => {
    close();
    if (result) setNotice(result);
  };

  return (
    <>
      <div className="app" ref={appRef} aria-hidden={covered || undefined}>
        <TopBar
          s={s}
          send={send}
          backendKind={backendKind}
          lastError={lastError}
          onSave={() => setOverlay({ kind: "save", tab: "save" })}
          onLoad={() => setOverlay({ kind: "save", tab: "load" })}
          onTrends={() => openTrends()}
          onLog={() => openLog()}
        />
        <Annunciator alarms={s.alarms} send={send} />
        <main className="main">
          <div className="col col-left">
            <ReactorPanel s={s} send={send} />
          </div>
          <div className="col col-center">
            <Panel title="PLANT MIMIC · NUCLEAR STEAM SUPPLY & POWER CONVERSION" className="mimic-panel">
              <PlantMimic s={s} send={send} />
            </Panel>
            <TrendPanel s={s} history={store.history} onOpenTrends={(focusGroup) => openTrends({ focusGroup })} />
          </div>
          <div className="col col-right">
            <FeedwaterPanel s={s} send={send} />
            <TurbinePanel s={s} send={send} />
          </div>
        </main>
      </div>

      {overlay.kind === "trends" && (
        <OverlayPage label="Trends" onClose={close}>
          <TrendsPage
            store={store}
            focusGroup={overlay.focusGroup}
            focusTimeS={overlay.focusTimeS}
            onClose={close}
            onOpenLog={(seq) => openLog(seq)}
          />
        </OverlayPage>
      )}
      {overlay.kind === "log" && (
        <OverlayPage label="Event log" onClose={close}>
          <LogPage store={store} focusSeq={overlay.focusSeq} onClose={close} onOpenTrends={(focusTimeS) => openTrends({ focusTimeS })} />
        </OverlayPage>
      )}
      {overlay.kind === "save" && <SaveDialog store={store} initialTab={overlay.tab} onClose={closeSave} />}
      {/* Always mounted so screen readers announce the text when it appears. */}
      <div className="app-notice-region" role="status" aria-live="polite">
        {notice && (
          <div className="app-notice">
            <span>✓ {notice}</span>
            <button type="button" className="app-notice-x" aria-label="Dismiss" onClick={() => setNotice(null)}>
              ✕
            </button>
          </div>
        )}
      </div>
    </>
  );
}

/** Full-screen page over the control room; Esc closes unless the page claimed the key. */
function OverlayPage(props: { label: string; onClose: () => void; children: ReactNode }) {
  const ref = useRef<HTMLDivElement>(null);
  useFocusTrap(ref, props.onClose);
  return (
    <div ref={ref} className="overlay-page" role="dialog" aria-modal="true" aria-label={props.label} tabIndex={-1}>
      {props.children}
    </div>
  );
}
