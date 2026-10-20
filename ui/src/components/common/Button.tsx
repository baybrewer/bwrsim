import type { ReactNode } from "react";

/** Square panel push-button; `active` lights it (green/red/amber). */
export function Btn(props: {
  children: ReactNode;
  onClick?: () => void;
  active?: boolean;
  tone?: "green" | "red" | "amber" | "plain";
  disabled?: boolean;
  title?: string;
  small?: boolean;
  className?: string;
}) {
  const tone = props.tone ?? "green";
  return (
    <button
      type="button"
      className={`btn btn-${tone}${props.active ? " on" : ""}${props.small ? " small" : ""} ${props.className ?? ""}`}
      onClick={props.onClick}
      disabled={props.disabled}
      title={props.title}
    >
      {props.children}
    </button>
  );
}

/** Two-button ON/OFF (or OPEN/CLOSE) control with state lamps. */
export function OnOff(props: {
  label: string;
  on: boolean;
  onSet: (on: boolean) => void;
  onText?: string;
  offText?: string;
}) {
  return (
    <div className="onoff">
      <div className="onoff-label">{props.label}</div>
      <div className="onoff-btns">
        <Btn small tone="red" active={props.on} onClick={() => props.onSet(true)}>
          {props.onText ?? "ON"}
        </Btn>
        <Btn small tone="green" active={!props.on} onClick={() => props.onSet(false)}>
          {props.offText ?? "OFF"}
        </Btn>
      </div>
    </div>
  );
}
