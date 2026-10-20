import type { Rods } from "../sim/types";
import { fmtRodPos, rodName } from "../util/format";

/** 9x9 rod position display. Click selects, shift/ctrl-click toggles multi-select. */
export function CoreMap(props: {
  rods: Rods;
  multi: Set<number>;
  onClick: (index: number, additive: boolean) => void;
}) {
  const { rods } = props;
  const n = rods.gridSize;
  const byCell = new Map<string, number>();
  rods.layout.forEach(([c, r], i) => byCell.set(`${c},${r}`, i));

  const cells = [];
  for (let r = 0; r < n; r++) {
    for (let c = 0; c < n; c++) {
      const i = byCell.get(`${c},${r}`);
      if (i === undefined) {
        cells.push(<div key={`${c},${r}`} className="rod-cell empty" />);
        continue;
      }
      const pos = rods.positions[i];
      const tgt = rods.targets[i];
      const state = pos < 0.5 ? "in" : pos > 47.5 ? "out" : "mid";
      const moving = Math.abs(tgt - pos) > 0.05;
      const sel = rods.selected === i;
      const inMulti = props.multi.has(i);
      cells.push(
        <button
          type="button"
          key={`${c},${r}`}
          className={`rod-cell rod-${state}${sel ? " sel" : ""}${inMulti ? " multi" : ""}${moving ? " moving" : ""}`}
          onClick={(e) => props.onClick(i, e.shiftKey || e.ctrlKey || e.metaKey)}
          title={`Rod ${rodName(c, r, n)}  pos ${fmtRodPos(pos)}  tgt ${fmtRodPos(tgt)}`}
        >
          <span className="rod-pos">{fmtRodPos(pos)}</span>
          {moving && <span className="rod-arrow">{tgt > pos ? "▲" : "▼"}</span>}
        </button>,
      );
    }
  }
  return (
    <div className="core-map" style={{ gridTemplateColumns: `repeat(${n}, 1fr)` }}>
      {cells}
    </div>
  );
}
