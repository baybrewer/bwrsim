export type LampColor = "green" | "red" | "amber" | "white" | "blue" | "off";

export function Lamp(props: { color: LampColor; on: boolean; label?: string; blink?: boolean; title?: string }) {
  const cls = `lamp lamp-${props.on ? props.color : "off"}${props.blink && props.on ? " blink" : ""}`;
  return (
    <span className="lamp-wrap" title={props.title}>
      <span className={cls} />
      {props.label && <span className="lamp-label">{props.label}</span>}
    </span>
  );
}
