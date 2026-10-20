import type { Command, LevelControlMode, Snapshot } from "../sim/types";
import { clamp, fx } from "../util/format";
import { Btn, OnOff } from "./common/Button";
import { CommitSlider } from "./common/CommitSlider";
import { Group, Panel } from "./common/Panel";
import { Readout } from "./common/Readout";

const MODES: { m: LevelControlMode; label: string }[] = [
  { m: "manual", label: "MANUAL" },
  { m: "singleElement", label: "1-ELEM" },
  { m: "threeElement", label: "3-ELEM" },
];



export function FeedwaterPanel(props: { s: Snapshot; send: (c: Command) => void }) {
  const { s, send } = props;
  const fw = s.feedwater;
  const v = s.vessel;
  const lvlTone = Math.abs(v.levelNrCm) > 150 ? "alarm" : Math.abs(v.levelNrCm) > 60 ? "warn" : "normal";
  return (
    <Panel title="FEEDWATER · REACTOR LEVEL CONTROL" className="fw-panel">
      <div className="readout-grid">
        <Readout label="LEVEL NR" value={fx(v.levelNrCm, 0)} unit="cm" tone={lvlTone} />
        <Readout label="ABOVE TAF" value={fx(v.levelAboveTafM, 2)} unit="m" tone={v.levelAboveTafM < 2 ? "alarm" : "normal"} />
        <Readout label="FEED FLOW" value={fx(v.feedFlowKgs, 0)} unit="kg/s" />
        <Readout label="STEAM FLOW" value={fx(v.steamFlowKgs, 0)} unit="kg/s" />
        <Readout label="FEED TEMP" value={fx(fw.feedTempC, 0)} unit="°C" />
        <Readout label="FCV POS" value={fx(fw.fcvPos * 100, 0)} unit="%" />
        <Readout label="HOTWELL" value={fx(s.condenser.hotwellLevelPct, 0)} unit="%" tone={s.condenser.hotwellLevelPct < 20 ? "warn" : "normal"} />
        <Readout label="RWCU REJ" value={fx(fw.rwcuRejectKgs, 1)} unit="kg/s" />
      </div>

      <Group label="LEVEL CONTROLLER">
        <div className="row gap4 wrap">
          {MODES.map(({ m, label }) => (
            <Btn key={m} tone="plain" active={fw.levelControlMode === m} onClick={() => send({ type: "setLevelControl", mode: m })}>
              {label}
            </Btn>
          ))}
          <span className="sep" />
          <span className="tb-label">SETPT</span>
          <Btn small tone="plain" onClick={() => send({ type: "setLevelSetpoint", cm: fw.levelSetpointCm - 5 })}>
            −5
          </Btn>
          <span className="digit">{fx(fw.levelSetpointCm, 0)} cm</span>
          <Btn small tone="plain" onClick={() => send({ type: "setLevelSetpoint", cm: fw.levelSetpointCm + 5 })}>
            +5
          </Btn>
        </div>
        <div className="row gap8 slider-row">
          <span className="tb-label">FCV MAN</span>
          <CommitSlider
            value={fw.fcvManualPos * 100}
            min={0}
            max={100}
            step={1}
            onCommit={(p) => send({ type: "setFcvManual", pos: clamp(p / 100, 0, 1) })}
          />
          <span className="digit">{fx(fw.fcvManualPos * 100, 0)}%</span>
        </div>
      </Group>

      <Group label="PUMPS">
        <div className="pump-row">
          {fw.pumps.map((p, i) => (
            <OnOff
              key={i}
              label={`FWP ${String.fromCharCode(65 + i)} · ${fx(p.flowKgs, 0)}`}
              on={p.running}
              onSet={(on) => send({ type: "setFeedPump", index: i, on })}
              onText="RUN"
              offText="STOP"
            />
          ))}
        </div>
        <div className="pump-row">
          {fw.condensatePumps.map((on, i) => (
            <OnOff
              key={i}
              label={`COND P ${String.fromCharCode(65 + i)}`}
              on={on}
              onSet={(o) => send({ type: "setCondensatePump", index: i, on: o })}
              onText="RUN"
              offText="STOP"
            />
          ))}
          <OnOff
            label="FW HEATERS"
            on={fw.heatersInService}
            onSet={(o) => send({ type: "setHeaters", inService: o })}
            onText="IN"
            offText="OUT"
          />
        </div>
        <div className="row gap8 slider-row">
          <span className="tb-label">RWCU REJECT</span>
          <CommitSlider value={fw.rwcuRejectKgs} min={0} max={10} step={0.5} onCommit={(k) => send({ type: "setRwcuReject", kgs: k })} />
          <span className="digit">{fx(fw.rwcuRejectKgs, 1)} kg/s</span>
        </div>
      </Group>
    </Panel>
  );
}
