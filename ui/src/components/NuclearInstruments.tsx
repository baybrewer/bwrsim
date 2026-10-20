import type { Command, ModeSwitch, Neutronics } from "../sim/types";
import { fmtExp, fmtPeriod, fx, fxSigned } from "../util/format";
import { BarMeter } from "./common/BarMeter";
import { Btn } from "./common/Button";
import { Readout } from "./common/Readout";

export function NuclearInstruments(props: { n: Neutronics; mode: ModeSwitch; send: (c: Command) => void }) {
  const { n, send } = props;
  // IRM trips are bypassed with the mode switch in RUN, so do not flash it there.
  const irmBypassed = props.mode === "run";
  const period = n.periodS;
  const periodTone = period !== null && period > 0 && period < 30 ? "alarm" : period !== null && period > 0 && period < 60 ? "warn" : "normal";
  return (
    <div className="nis">
      <BarMeter
        label="SRM"
        value={n.srmCps}
        min={1}
        max={1e6}
        log
        display={fmtExp(n.srmCps)}
        unit="cps"
        warnAbove={1e5}
        marks={[1, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6].map((v) => ({ at: v, label: `${Math.log10(v)}` }))}
      />
      <div className="irm">
        <BarMeter
          label={`IRM  RNG ${n.irmRange}${irmBypassed ? "  (BYPASSED IN RUN)" : ""}`}
          value={n.irmReadingPct}
          min={0}
          max={125}
          display={fx(n.irmReadingPct, 1)}
          unit="%"
          warnAbove={irmBypassed ? undefined : 100}
          alarmAbove={irmBypassed ? undefined : 120}
          marks={[0, 25, 50, 75, 100, 125].map((v) => ({ at: v, label: `${v}` }))}
        />
        <div className="irm-range">
          <Btn small tone="plain" onClick={() => send({ type: "setIrmRange", range: Math.max(1, n.irmRange - 1) })}>
            ◀
          </Btn>
          {Array.from({ length: 10 }, (_, i) => i + 1).map((r) => (
            <button
              key={r}
              type="button"
              className={`irm-r${r === n.irmRange ? " on" : ""}`}
              onClick={() => send({ type: "setIrmRange", range: r })}
            >
              {r}
            </button>
          ))}
          <Btn small tone="plain" onClick={() => send({ type: "setIrmRange", range: Math.min(10, n.irmRange + 1) })}>
            ▶
          </Btn>
        </div>
      </div>
      <BarMeter
        label="APRM"
        value={n.aprmPct}
        min={0}
        max={125}
        display={fx(n.aprmPct, 1)}
        unit="%"
        warnAbove={105}
        alarmAbove={118}
        marks={[0, 25, 50, 75, 100, 125].map((v) => ({ at: v, label: `${v}` }))}
      />
      <div className="readout-grid">
        <Readout label="PERIOD" value={fmtPeriod(period)} unit="s" tone={periodTone} />
        <Readout label="K-EFF" value={fx(n.kEff, 5)} />
        <Readout label="THERMAL" value={fx(n.thermalPowerMw, 1)} unit="MWt" />
        <Readout label="DECAY HEAT" value={fx(n.decayHeatMw, 1)} unit="MW" />
        <Readout label="FUEL TEMP" value={fx(n.fuelTempC, 0)} unit="°C" tone={n.fuelTempC > 1200 ? "alarm" : "normal"} />
        <Readout label="XENON" value={fx(n.xenonRel * 100, 1)} unit="%" />
        <Readout label="IODINE" value={fx(n.iodineRel * 100, 1)} unit="%" />
        <Readout label="NEUTRON PWR" value={fmtExp(n.powerFrac * 100, 2)} unit="%" />
      </div>
      <table className="rho-table">
        <thead>
          <tr>
            <th colSpan={6}>REACTIVITY (pcm)</th>
          </tr>
        </thead>
        <tbody>
          <tr>
            {(["rods", "doppler", "void", "moderator", "xenon", "total"] as const).map((k) => (
              <td key={k} className="rho-name">
                {k === "moderator" ? "MOD" : k.toUpperCase()}
              </td>
            ))}
          </tr>
          <tr>
            {(["rods", "doppler", "void", "moderator", "xenon", "total"] as const).map((k) => {
              const v = n.reactivity[k];
              return (
                <td key={k} className={`rho-val${k === "total" ? " total" : ""} ${v > 0 ? "pos" : v < 0 ? "neg" : ""}`}>
                  {fxSigned(v, 0)}
                </td>
              );
            })}
          </tr>
        </tbody>
      </table>
    </div>
  );
}
