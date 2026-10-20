import { fx } from "../util/format";

/** Synchroscope: needle at syncPhaseDeg (0 = 12 o'clock = in phase). */
export function Synchroscope(props: { phaseDeg: number; slipHz: number; active: boolean }) {
  const inSync = props.active && Math.abs(props.slipHz) < 0.1 && Math.abs(props.phaseDeg) < 15;
  const ticks = Array.from({ length: 12 }, (_, i) => i * 30);
  return (
    <div className="synchro">
      <svg viewBox="-50 -50 100 100" width={104} height={104}>
        <circle r={46} className="sync-face" />
        <path d="M -12 -44 A 46 46 0 0 1 12 -44" className="sync-zone" />
        {ticks.map((a) => (
          <line key={a} x1={0} y1={-46} x2={0} y2={a === 0 ? -36 : -40} transform={`rotate(${a})`} className="sync-tick" />
        ))}
        <text y={-24} textAnchor="middle" className="sync-txt">
          FAST ↻
        </text>
        <text y={30} textAnchor="middle" className="sync-txt">
          SLOW ↺
        </text>
        {props.active && (
          <line x1={0} y1={8} x2={0} y2={-40} transform={`rotate(${props.phaseDeg})`} className="sync-needle" />
        )}
        <circle r={4} className="sync-hub" />
      </svg>
      <div className="sync-info">
        <div className="tb-label">SLIP</div>
        <div className={`digit ${inSync ? "tone-ok" : ""}`}>{props.active ? `${fx(props.slipHz, 2)} Hz` : "---"}</div>
        <div className="tb-label">PHASE</div>
        <div className="digit">{props.active ? `${fx(props.phaseDeg, 0)}°` : "---"}</div>
        <span className={`lamp lamp-${inSync ? "green" : "off"}`} /> <span className="lamp-label">SYNC</span>
      </div>
    </div>
  );
}
