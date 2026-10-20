import type { Alarm, Command } from "../sim/types";
import { Btn } from "./common/Button";

const MIN_TILES = 24;

/** Annunciator window grid. Unacked alarms flash; colour by severity. */
export function Annunciator(props: { alarms: Alarm[]; send: (c: Command) => void }) {
  const order = { trip: 0, alarm: 1, warning: 2, info: 3 } as const;
  const sorted = [...props.alarms].sort((a, b) => order[a.severity] - order[b.severity]);
  const blanks = Math.max(0, MIN_TILES - sorted.length);
  const unacked = sorted.some((a) => !a.acked);
  return (
    <div className="annunciator">
      <div className="ann-grid">
        {sorted.map((a) => (
          <div
            key={a.id}
            className={`ann-tile sev-${a.severity}${a.acked ? " acked" : " flash"}`}
            title={`${a.severity.toUpperCase()}: ${a.text}`}
          >
            {a.text}
          </div>
        ))}
        {Array.from({ length: blanks }, (_, i) => (
          <div key={`b${i}`} className="ann-tile blank" />
        ))}
      </div>
      <div className="ann-ctrl">
        <Btn tone="amber" active={unacked} onClick={() => props.send({ type: "ackAlarms" })}>
          ACK
        </Btn>
        <div className="ann-count">
          {props.alarms.length} ACTIVE
          <br />
          {props.alarms.filter((a) => !a.acked).length} NEW
        </div>
      </div>
    </div>
  );
}
