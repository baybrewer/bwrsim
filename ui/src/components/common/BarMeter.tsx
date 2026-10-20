import { clamp } from "../../util/format";

interface Mark {
  at: number;
  label: string;
}

/** Horizontal bar meter. `log` maps value on log10 between min and max. */
export function BarMeter(props: {
  label: string;
  value: number;
  min: number;
  max: number;
  log?: boolean;
  display: string;
  unit?: string;
  marks?: Mark[];
  warnAbove?: number;
  alarmAbove?: number;
}) {
  const { min, max, log } = props;
  const frac = (v: number) =>
    log
      ? clamp((Math.log10(Math.max(v, min)) - Math.log10(min)) / (Math.log10(max) - Math.log10(min)), 0, 1)
      : clamp((v - min) / (max - min), 0, 1);
  const f = frac(props.value);
  const tone =
    props.alarmAbove !== undefined && props.value >= props.alarmAbove
      ? "alarm"
      : props.warnAbove !== undefined && props.value >= props.warnAbove
        ? "warn"
        : "normal";
  return (
    <div className="bar-meter">
      <div className="bar-head">
        <span className="bar-label">{props.label}</span>
        <span className={`bar-value tone-${tone}`}>
          {props.display}
          {props.unit && <span className="readout-unit">{props.unit}</span>}
        </span>
      </div>
      <div className="bar-track">
        <div className={`bar-fill fill-${tone}`} style={{ width: `${f * 100}%` }} />
        {props.marks?.map((m) => (
          <div key={m.label} className="bar-tick" style={{ left: `${frac(m.at) * 100}%` }}>
            <span>{m.label}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
