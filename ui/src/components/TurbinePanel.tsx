import type { Command, Snapshot } from "../sim/types";
import { clamp, fx } from "../util/format";
import { Btn, OnOff } from "./common/Button";
import { Lamp } from "./common/Lamp";
import { Group, Panel } from "./common/Panel";
import { Readout } from "./common/Readout";
import { CommitSlider } from "./common/CommitSlider";
import { Synchroscope } from "./Synchroscope";

// Rated is 3600 rpm (two-pole, 60 Hz); 1800 is the half-speed hold point.
const SPEED_SETPOINTS = [0, 100, 500, 1800, 3600];

export function TurbinePanel(props: { s: Snapshot; send: (c: Command) => void }) {
  const { s, send } = props;
  const st = s.steam;
  const tb = s.turbine;
  const cd = s.condenser;
  const psp = (mpa: number) => send({ type: "setPressureSetpoint", mpa: Math.round(clamp(mpa, 0, 8) * 100) / 100 });
  return (
    <Panel title="TURBINE · EHC · GENERATOR · CONDENSER" className="tg-panel">
      <div className="readout-grid">
        <Readout label="RX PRESS" value={fx(s.vessel.pressureMpa, 3)} unit="MPa" tone={s.vessel.pressureMpa > 7.5 ? "alarm" : "normal"} />
        <Readout label="GROSS" value={fx(tb.grossMwe, 1)} unit="MWe" />
        <Readout label="NET" value={fx(tb.netMwe, 1)} unit="MWe" />
        <Readout label="SPEED" value={fx(tb.speedRpm, 0)} unit="rpm" />
        <Readout label="TCV" value={fx(st.tcvPos * 100, 1)} unit="%" />
        <Readout label="BYPASS" value={fx(st.bypassPos * 100, 1)} unit="%" />
        <Readout label="CONDENSER" value={fx(cd.pressureKpa, 1)} unit="kPa" tone={cd.pressureKpa > 25 ? "alarm" : cd.pressureKpa > 15 ? "warn" : "normal"} />
        <Readout label="STM→TURB" value={fx(st.steamToTurbineKgs, 0)} unit="kg/s" />
      </div>

      <Group label="EHC PRESSURE CONTROL">
        <div className="row gap4 wrap">
          <span className="tb-label">P SETPT</span>
          <Btn small tone="plain" onClick={() => psp(st.pressureSetpointMpa - 0.1)}>
            −.1
          </Btn>
          <Btn small tone="plain" onClick={() => psp(st.pressureSetpointMpa - 0.01)}>
            −.01
          </Btn>
          <span className="digit">{fx(st.pressureSetpointMpa, 2)} MPa</span>
          <Btn small tone="plain" onClick={() => psp(st.pressureSetpointMpa + 0.01)}>
            +.01
          </Btn>
          <Btn small tone="plain" onClick={() => psp(st.pressureSetpointMpa + 0.1)}>
            +.1
          </Btn>
        </div>
        <div className="row gap8 slider-row">
          <Btn small tone="plain" active={st.bypassAuto} onClick={() => send({ type: "setBypass", auto: !st.bypassAuto, pos: st.bypassManualPos })}>
            BYP {st.bypassAuto ? "AUTO" : "MAN"}
          </Btn>
          <CommitSlider
            value={st.bypassManualPos * 100}
            min={0}
            max={100}
            step={1}
            onCommit={(p) => send({ type: "setBypass", auto: st.bypassAuto, pos: p / 100 })}
          />
          <span className="digit">{fx(st.bypassManualPos * 100, 0)}%</span>
        </div>
      </Group>

      <Group label="TURBINE">
        <div className="row gap4 wrap">
          <Lamp color="red" on={tb.tripped} label="TRIPPED" />
          <Lamp color="green" on={!tb.tripped} label="LATCHED" />
          <span className="sep" />
          <Btn tone="green" onClick={() => send({ type: "resetTurbine" })} disabled={!tb.tripped}>
            RESET
          </Btn>
          <Btn tone="red" onClick={() => send({ type: "tripTurbine" })} disabled={tb.tripped}>
            TRIP
          </Btn>
        </div>
        <div className="row gap4 wrap">
          <span className="tb-label">SPEED SP</span>
          {SPEED_SETPOINTS.map((r) => (
            <Btn key={r} small tone="plain" active={tb.speedSetpointRpm === r} onClick={() => send({ type: "setSpeedSetpoint", rpm: r })}>
              {r}
            </Btn>
          ))}
        </div>
      </Group>

      <Group label="GENERATOR">
        <div className="gen-row">
          <Synchroscope phaseDeg={tb.syncPhaseDeg} slipHz={tb.slipHz} active={!tb.breakerClosed && tb.speedRpm > 1500} />
          <div className="gen-ctrl">
            <div className="row gap4">
              <Lamp color="red" on={tb.breakerClosed} label="BKR CLOSED" />
              <Lamp color="green" on={!tb.breakerClosed} label="OPEN" />
            </div>
            <div className="row gap4">
              <Btn tone="red" onClick={() => send({ type: "closeBreaker" })} disabled={tb.breakerClosed}>
                CLOSE
              </Btn>
              <Btn tone="green" onClick={() => send({ type: "openBreaker" })} disabled={!tb.breakerClosed}>
                OPEN
              </Btn>
            </div>
            <div className="row gap4">
              <span className="tb-label">LOAD LIM</span>
              <Btn small tone="plain" onClick={() => send({ type: "setLoadLimit", pct: clamp(tb.loadLimitPct - 5, 0, 110) })}>
                −5
              </Btn>
              <span className="digit">{fx(tb.loadLimitPct, 0)}%</span>
              <Btn small tone="plain" onClick={() => send({ type: "setLoadLimit", pct: clamp(tb.loadLimitPct + 5, 0, 110) })}>
                +5
              </Btn>
            </div>
          </div>
        </div>
      </Group>

      <Group label="CONDENSER · AUX">
        <div className="pump-row">
          {cd.cwPumps.map((on, i) => (
            <OnOff key={i} label={`CW PUMP ${String.fromCharCode(65 + i)}`} on={on} onSet={(o) => send({ type: "setCwPump", index: i, on: o })} onText="RUN" offText="STOP" />
          ))}
          <OnOff label="VACUUM PUMP" on={cd.vacuumPumpOn} onSet={(o) => send({ type: "setVacuumPump", on: o })} onText="RUN" offText="STOP" />
          <OnOff label="RPV HEAD VENT" on={s.vessel.headVentOpen} onSet={(o) => send({ type: "setHeadVent", open: o })} onText="OPEN" offText="CLOSE" />
        </div>
      </Group>
    </Panel>
  );
}
