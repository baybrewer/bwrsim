import { useState } from "react";
import type { Command, ScenarioName, Snapshot } from "../sim/types";
import { fmtClock } from "../util/format";
import { Btn } from "./common/Button";
import { Lamp } from "./common/Lamp";
import { ModeSwitch } from "./ModeSwitch";
import { ScramButton } from "./ScramButton";

const SPEEDS = [1, 2, 5, 10, 30];
const SCENARIOS: { name: ScenarioName; label: string }[] = [
  { name: "coldShutdown", label: "COLD SHUTDOWN" },
  { name: "hotStandby", label: "HOT STANDBY" },
  { name: "fullPower", label: "FULL POWER" },
];

export function TopBar(props: {
  s: Snapshot;
  send: (c: Command) => void;
  backendKind: string;
  lastError: string | null;
}) {
  const { s, send } = props;
  const [scenario, setScenario] = useState<ScenarioName>("hotStandby");
  const [confirmLoad, setConfirmLoad] = useState(false);
  const allIn = s.rods.positions.every((p) => p < 0.5);
  // The host is falling behind the requested speed (smoothed rate below 90 %, or time piling up).
  const lagging = !s.paused && (s.effectiveSpeed < 0.9 * s.speed || s.clockBacklogS > 1);

  return (
    <header className="topbar">
      <div className="brand">
        <div className="brand-name">BWRX-300</div>
        <div className="brand-sub">UNIT 1 MAIN CONTROL ROOM · {props.backendKind === "mock" ? "MOCK SIM" : "SIM LINK"}</div>
      </div>

      <div className="tb-block">
        <div className="tb-label">SIM TIME</div>
        <div className="clock">{fmtClock(s.timeS)}</div>
      </div>

      <div className="tb-block">
        <div className="tb-label">TIME CONTROL</div>
        <div className="row gap4">
          <Btn tone="amber" active={s.paused} onClick={() => send({ type: "setPaused", paused: !s.paused })}>
            {s.paused ? "▶ RUN" : "❚❚ PAUSE"}
          </Btn>
          {SPEEDS.map((x) => (
            <Btn key={x} tone="plain" active={s.speed === x} onClick={() => send({ type: "setSpeed", speed: x })}>
              {x}×
            </Btn>
          ))}
        </div>
        <div className={`sim-rate${lagging ? " tone-warn" : ""}`} title="Achieved simulated seconds per real second, and simulated time owed but not yet computed">
          ACHIEVED {s.paused ? "—" : `${s.effectiveSpeed.toFixed(1)}×`} · BACKLOG {s.clockBacklogS.toFixed(1)} s
          {lagging && " · LAGGING"}
        </div>
      </div>

      <div className="tb-block">
        <div className="tb-label">SCENARIO</div>
        <div className="row gap4">
          <select
            className="select"
            value={scenario}
            onChange={(e) => {
              setScenario(e.target.value as ScenarioName);
              setConfirmLoad(false);
            }}
          >
            {SCENARIOS.map((sc) => (
              <option key={sc.name} value={sc.name}>
                {sc.label}
              </option>
            ))}
          </select>
          <Btn
            tone="amber"
            active={confirmLoad}
            onClick={() => {
              if (!confirmLoad) {
                setConfirmLoad(true);
                setTimeout(() => setConfirmLoad(false), 4000);
              } else {
                setConfirmLoad(false);
                send({ type: "loadScenario", name: scenario });
              }
            }}
          >
            {confirmLoad ? "CONFIRM" : "LOAD"}
          </Btn>
        </div>
      </div>

      <div className="tb-spacer" />

      {props.lastError && <div className="cmd-error">⚠ {props.lastError}</div>}

      <ModeSwitch mode={s.modeSwitch} onSet={(mode) => send({ type: "setModeSwitch", mode })} />

      <div className="tb-block scram-status">
        <div className="tb-label">RPS</div>
        <div className="row gap8">
          <Lamp color="red" on={s.scrammed} blink={s.rods.scramInProgress} label="SCRAM" />
          <Lamp color="green" on={allIn} label="ALL RODS IN" />
          <Lamp color="white" on={!s.scrammed} label="RPS NORMAL" />
        </div>
        <div className={`scram-reason${s.scrammed ? " active" : ""}`}>{s.scrammed ? s.scramReason ?? "SCRAM" : "—"}</div>
      </div>

      <Btn tone="amber" onClick={() => send({ type: "resetScram" })} disabled={!s.scrammed} title="Reset scram (rods must be fully inserted)">
        SCRAM
        <br />
        RESET
      </Btn>

      <ScramButton onScram={() => send({ type: "manualScram" })} />
    </header>
  );
}
