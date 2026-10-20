import { Annunciator } from "./components/Annunciator";
import { FeedwaterPanel } from "./components/FeedwaterPanel";
import { PlantMimic } from "./components/mimic/PlantMimic";
import { Panel } from "./components/common/Panel";
import { ReactorPanel } from "./components/ReactorPanel";
import { TopBar } from "./components/TopBar";
import { TrendPanel } from "./components/TrendPanel";
import { TurbinePanel } from "./components/TurbinePanel";
import { useTrendHistory } from "./sim/history";
import { useSim } from "./sim/bridge";

export default function App() {
  const { snapshot: s, send, lastError, backendKind } = useSim();
  const samples = useTrendHistory(s);

  if (!s) {
    return (
      <div className="waiting">
        <div className="lamp lamp-amber blink" />
        WAITING FOR SIMULATOR ({backendKind})…
      </div>
    );
  }

  return (
    <div className="app">
      <TopBar s={s} send={send} backendKind={backendKind} lastError={lastError} />
      <Annunciator alarms={s.alarms} send={send} />
      <main className="main">
        <div className="col col-left">
          <ReactorPanel s={s} send={send} />
        </div>
        <div className="col col-center">
          <Panel title="PLANT MIMIC · NUCLEAR STEAM SUPPLY & POWER CONVERSION" className="mimic-panel">
            <PlantMimic s={s} send={send} />
          </Panel>
          <TrendPanel samples={samples} />
        </div>
        <div className="col col-right">
          <FeedwaterPanel s={s} send={send} />
          <TurbinePanel s={s} send={send} />
        </div>
      </main>
    </div>
  );
}
