import { useEffect, useState } from "react";

/** Slider that only sends on release, but shows the live drag value. */
export function CommitSlider(props: { value: number; min: number; max: number; step: number; onCommit: (v: number) => void }) {
  const [v, setV] = useState(props.value);
  const [drag, setDrag] = useState(false);
  useEffect(() => {
    if (!drag) setV(props.value);
  }, [props.value, drag]);
  const commit = () => {
    setDrag(false);
    props.onCommit(v);
  };
  return (
    <input
      type="range"
      className="slider"
      min={props.min}
      max={props.max}
      step={props.step}
      value={v}
      onPointerDown={() => setDrag(true)}
      onChange={(e) => setV(Number(e.target.value))}
      onPointerUp={commit}
      onKeyUp={commit}
    />
  );
}

