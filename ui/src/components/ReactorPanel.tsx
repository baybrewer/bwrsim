import { useState } from "react";
import type { Command, Snapshot } from "../sim/types";
import { Panel } from "./common/Panel";
import { CoreMap } from "./CoreMap";
import { NuclearInstruments } from "./NuclearInstruments";
import { RodControls } from "./RodControls";

export function ReactorPanel(props: { s: Snapshot; send: (c: Command) => void }) {
  const { s, send } = props;
  const [multi, setMulti] = useState<Set<number>>(new Set());
  const onRod = (i: number, additive: boolean) => {
    if (additive) {
      const next = new Set(multi);
      if (next.has(i)) next.delete(i);
      else next.add(i);
      if (s.rods.selected !== null && !multi.size && s.rods.selected !== i) next.add(s.rods.selected);
      setMulti(next);
    } else {
      setMulti(new Set());
    }
    send({ type: "selectRod", index: i });
  };
  return (
    <Panel
      title="REACTOR · ROD CONTROL & NUCLEAR INSTRUMENTATION"
      className="reactor-panel"
      right={
        <span className="legend">
          <i className="lg in" />
          IN <i className="lg mid" />
          MID <i className="lg out" />
          OUT
        </span>
      }
    >
      <CoreMap rods={s.rods} multi={multi} onClick={onRod} />
      <RodControls rods={s.rods} multi={multi} setMulti={setMulti} send={send} scrammed={s.scrammed} />
      <NuclearInstruments n={s.neutronics} mode={s.modeSwitch} send={send} />
    </Panel>
  );
}
