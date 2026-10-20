import type { ModeSwitch as Mode } from "../sim/types";

const MODES: { mode: Mode; label: string; angle: number }[] = [
  { mode: "shutdown", label: "SHUTDOWN", angle: -78 },
  { mode: "refuel", label: "REFUEL", angle: -30 },
  { mode: "startup", label: "STARTUP", angle: 30 },
  { mode: "run", label: "RUN", angle: 78 },
];

/** Rotary-style reactor mode switch. Click a legend (or the knob to step right). */
export function ModeSwitch(props: { mode: Mode; onSet: (m: Mode) => void }) {
  const idx = MODES.findIndex((m) => m.mode === props.mode);
  const cur = MODES[Math.max(0, idx)];
  const step = (d: number) => {
    const n = MODES[(idx + d + MODES.length) % MODES.length];
    props.onSet(n.mode);
  };
  return (
    <div className="mode-switch" title="Reactor mode switch">
      <svg viewBox="-75 -44 150 64" width={160} height={68}>
        {MODES.map((m) => {
          const a = ((m.angle - 90) * Math.PI) / 180;
          const x = Math.cos(a) * 52;
          const y = Math.sin(a) * 34 + 8;
          const active = m.mode === props.mode;
          return (
            <text
              key={m.mode}
              x={x}
              y={y}
              className={`ms-legend${active ? " active" : ""}`}
              textAnchor="middle"
              onClick={() => props.onSet(m.mode)}
            >
              {m.label}
            </text>
          );
        })}
        <g
          transform={`translate(0 10) rotate(${cur.angle})`}
          className="ms-knob"
          onClick={() => step(1)}
          onContextMenu={(e) => {
            e.preventDefault();
            step(-1);
          }}
        >
          <circle r={17} className="ms-body" />
          <rect x={-4} y={-19} width={8} height={30} rx={2} className="ms-handle" />
          <circle cy={-14} r={2} className="ms-dot" />
        </g>
      </svg>
      <div className="ms-caption">MODE SWITCH</div>
    </div>
  );
}
