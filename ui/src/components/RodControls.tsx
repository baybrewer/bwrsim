import { useState } from "react";
import type { Command, Rods } from "../sim/types";
import { clamp, fmtRodPos, rodName } from "../util/format";
import { Btn } from "./common/Button";

/** Rod motion controls. Acts on the multi-select set if non-empty, else the selected rod. */
export function RodControls(props: {
  rods: Rods;
  multi: Set<number>;
  setMulti: (s: Set<number>) => void;
  send: (c: Command) => void;
  scrammed: boolean;
}) {
  const { rods, multi, send } = props;
  const [to, setTo] = useState(48);
  const targets = multi.size > 0 ? [...multi] : rods.selected !== null ? [rods.selected] : [];

  /** Move each rod by delta notch units; groups rods sharing a destination into one command. */
  const step = (delta: number) => {
    const groups = new Map<number, number[]>();
    for (const i of targets) {
      const base = Math.round(rods.targets[i] / 2) * 2;
      const t = clamp(base + delta, 0, 48);
      groups.set(t, [...(groups.get(t) ?? []), i]);
    }
    for (const [target, indices] of groups) send({ type: "rodSetTarget", indices, target });
  };

  const sel = rods.selected;
  const selName = sel !== null ? rodName(rods.layout[sel][0], rods.layout[sel][1], rods.gridSize) : "--";
  const all = () => props.setMulti(new Set(rods.positions.map((_, i) => i)));
  const parity = (p: number) =>
    props.setMulti(new Set(rods.layout.map(([c, r], i) => ((c + r) % 2 === p ? i : -1)).filter((i) => i >= 0)));

  return (
    <div className="rod-ctrl">
      <div className="rod-sel-info">
        <div>
          <div className="tb-label">SELECTED ROD</div>
          <div className="digit big">{selName}</div>
        </div>
        <div>
          <div className="tb-label">POS / TGT</div>
          <div className="digit big">
            {sel !== null ? `${fmtRodPos(rods.positions[sel])}/${fmtRodPos(rods.targets[sel])}` : "--/--"}
          </div>
        </div>
        <div>
          <div className="tb-label">GROUP</div>
          <div className="digit big">{multi.size || (sel !== null ? 1 : 0)}</div>
        </div>
      </div>
      <div className="row gap4 wrap">
        <Btn tone="green" onClick={() => step(-2)} disabled={!targets.length}>
          ▼ INSERT
        </Btn>
        <Btn tone="red" onClick={() => step(+2)} disabled={!targets.length || props.scrammed}>
          ▲ WITHDRAW
        </Btn>
        <Btn tone="amber" onClick={() => send({ type: "rodStop" })}>
          STOP
        </Btn>
        <span className="sep" />
        <input
          className="num-input"
          type="number"
          min={0}
          max={48}
          step={2}
          value={to}
          onChange={(e) => setTo(clamp(Number(e.target.value) || 0, 0, 48))}
        />
        <Btn
          tone="plain"
          disabled={!targets.length}
          onClick={() => send({ type: "rodSetTarget", indices: targets, target: Math.round(to / 2) * 2 })}
        >
          MOVE TO
        </Btn>
      </div>
      <div className="row gap4 wrap">
        <span className="tb-label">GROUP SELECT</span>
        <Btn small tone="plain" onClick={all}>
          ALL
        </Btn>
        <Btn small tone="plain" onClick={() => parity(0)}>
          A (EVEN)
        </Btn>
        <Btn small tone="plain" onClick={() => parity(1)}>
          B (ODD)
        </Btn>
        <Btn small tone="plain" onClick={() => props.setMulti(new Set())} disabled={!multi.size}>
          CLEAR
        </Btn>
        <span className="hint">shift-click = add/remove</span>
      </div>
    </div>
  );
}
