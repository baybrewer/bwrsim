// SVG primitives for the plant mimic.

/** Gate-valve bow-tie. Filled proportionally to position (0 closed/hollow .. 1 open/filled). */
export function Valve(props: {
  x: number;
  y: number;
  pos: number;
  vertical?: boolean;
  size?: number;
  label?: string;
  labelPos?: "above" | "below" | "left" | "right";
  onClick?: () => void;
  title?: string;
}) {
  const s = props.size ?? 9;
  const d = `M ${-s} ${-s * 0.75} L ${s} ${s * 0.75} L ${s} ${-s * 0.75} L ${-s} ${s * 0.75} Z`;
  const p = Math.max(0, Math.min(1, props.pos));
  const lp = props.labelPos ?? "above";
  const lx = lp === "left" ? -s - 4 : lp === "right" ? s + 4 : 0;
  const ly = lp === "above" ? -s - 4 : lp === "below" ? s + 11 : 4;
  const anchor = lp === "left" ? "end" : lp === "right" ? "start" : "middle";
  return (
    <g
      transform={`translate(${props.x} ${props.y})`}
      className={`valve${props.onClick ? " clickable" : ""}`}
      onClick={props.onClick}
    >
      <title>{props.title ?? props.label}</title>
      <g transform={props.vertical ? "rotate(90)" : undefined}>
        <rect x={-s - 3} y={-s - 3} width={2 * s + 6} height={2 * s + 6} className="hit" />
        <path d={d} className="valve-body" style={{ fillOpacity: p }} />
      </g>
      {props.label && (
        <text x={lx} y={ly} textAnchor={anchor} className="m-label">
          {props.label}
        </text>
      )}
    </g>
  );
}

/** Centrifugal pump: circle + discharge triangle. Green running, grey stopped. */
export function Pump(props: {
  x: number;
  y: number;
  running: boolean;
  label?: string;
  r?: number;
  onClick?: () => void;
  dir?: "left" | "right" | "up";
}) {
  const r = props.r ?? 11;
  const rot = props.dir === "right" ? 0 : props.dir === "up" ? -90 : 180;
  return (
    <g
      transform={`translate(${props.x} ${props.y})`}
      className={`pump${props.running ? " running" : ""}${props.onClick ? " clickable" : ""}`}
      onClick={props.onClick}
    >
      <title>{props.label}</title>
      <circle r={r} className="pump-body" />
      <path d={`M ${-r * 0.5} ${-r * 0.55} L ${r * 0.6} 0 L ${-r * 0.5} ${r * 0.55} Z`} transform={`rotate(${rot})`} className="pump-tri" />
      {props.label && (
        <text y={r + 11} textAnchor="middle" className="m-label">
          {props.label}
        </text>
      )}
    </g>
  );
}

/** Small digital value tag. */
export function Tag(props: { x: number; y: number; text: string; anchor?: "start" | "middle" | "end"; tone?: "warn" | "alarm" | "dim" }) {
  const w = props.text.length * 6.1 + 6;
  const a = props.anchor ?? "middle";
  const x0 = a === "middle" ? props.x - w / 2 : a === "end" ? props.x - w : props.x;
  return (
    <g className={`tag${props.tone ? ` tag-${props.tone}` : ""}`}>
      <rect x={x0} y={props.y - 9} width={w} height={13} rx={1.5} />
      <text x={x0 + w / 2} y={props.y + 1} textAnchor="middle">
        {props.text}
      </text>
    </g>
  );
}

export function Pipe(props: { d: string; kind?: "steam" | "water" | "cw"; flowing?: boolean }) {
  return <path d={props.d} className={`pipe pipe-${props.kind ?? "water"}${props.flowing ? " flowing" : ""}`} />;
}
