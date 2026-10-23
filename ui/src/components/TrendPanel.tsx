import type { Snapshot } from "../sim/types";
import type { History } from "../sim/logic/history.ts";
import { MINI_CHARTS, channelById, readChannel } from "../sim/logic/channels.ts";
import type { Channel } from "../sim/logic/channels.ts";
import { Panel } from "./common/Panel";
import { StripChart } from "./StripChart";

const WINDOW_S = 600;

/** Mini charts only have room for a word: the first word of the registry label. */
const shortName = (c: Channel) => c.label.split(" ")[0].toUpperCase();

/** Re-renders with every store change; the canvases redraw from the run history. */
export function TrendPanel(props: { s: Snapshot; history: History }) {
  const { s, history } = props;
  const query = (t0: number, t1: number, px: number) => history.query(t0, t1, px);
  return (
    <Panel title="TRENDS · LAST 10 MIN" className="trend-panel">
      <div className="trend-grid">
        {MINI_CHARTS.map((chart) => {
          const channels = chart.channelIds.map((id) => channelById(id)).filter((c): c is Channel => c !== undefined);
          return (
            <StripChart
              key={chart.title}
              title={chart.title}
              unit={chart.unit}
              digits={chart.digits}
              series={channels.map((c) => ({ id: c.id, name: shortName(c), color: c.color }))}
              values={channels.map((c) => readChannel(c, s))}
              query={query}
              tNow={s.timeS}
              windowS={WINDOW_S}
            />
          );
        })}
      </div>
    </Panel>
  );
}
