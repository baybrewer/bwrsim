/** Digital readout: small label above a 7-seg-ish monospace value. */
export function Readout(props: {
  label: string;
  value: string;
  unit?: string;
  tone?: "normal" | "warn" | "alarm" | "dim";
  wide?: boolean;
}) {
  return (
    <div className={`readout ${props.wide ? "wide" : ""}`}>
      <div className="readout-label">{props.label}</div>
      <div className={`readout-value tone-${props.tone ?? "normal"}`}>
        {props.value}
        {props.unit && <span className="readout-unit">{props.unit}</span>}
      </div>
    </div>
  );
}
