import type { TrendSample } from "../sim/history";
import { TREND_WINDOW_S } from "../sim/history";
import { Panel } from "./common/Panel";
import { StripChart } from "./StripChart";

const C = {
  green: "#3ee07a",
  amber: "#ffb43a",
  cyan: "#4cc9f0",
  red: "#ff5a4e",
  violet: "#b48cff",
  white: "#dfe6e2",
};

/** Re-renders with every snapshot; the canvases redraw from the shared sample buffer. */
export function TrendPanel(props: { samples: TrendSample[] }) {
  const common = { samples: props.samples, windowS: TREND_WINDOW_S };
  return (
    <Panel title="TRENDS · LAST 10 MIN" className="trend-panel">
      <div className="trend-grid">
        <StripChart {...common} title="THERMAL POWER" unit="%" series={[{ name: "P", color: C.green, get: (s) => s.powerPct }]} />
        <StripChart {...common} title="RX PRESSURE" unit="MPa" digits={2} series={[{ name: "P", color: C.amber, get: (s) => s.pressureMpa }]} />
        <StripChart {...common} title="LEVEL NR" unit="cm" digits={0} series={[{ name: "L", color: C.cyan, get: (s) => s.levelNrCm }]} />
        <StripChart {...common} title="CORE FLOW" unit="kg/s" digits={0} series={[{ name: "W", color: C.violet, get: (s) => s.coreFlowKgs }]} />
        <StripChart
          {...common}
          title="STEAM / FEED"
          unit="kg/s"
          digits={0}
          series={[
            { name: "STM", color: C.red, get: (s) => s.steamFlowKgs },
            { name: "FW", color: C.cyan, get: (s) => s.feedFlowKgs },
          ]}
        />
        <StripChart {...common} title="GENERATOR" unit="MWe" digits={0} series={[{ name: "G", color: C.white, get: (s) => s.grossMwe }]} />
      </div>
    </Panel>
  );
}
