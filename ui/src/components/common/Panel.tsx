import type { ReactNode } from "react";

export function Panel(props: { title: string; className?: string; children: ReactNode; right?: ReactNode }) {
  return (
    <section className={`panel ${props.className ?? ""}`}>
      <header className="panel-title">
        <span>{props.title}</span>
        {props.right}
      </header>
      <div className="panel-body">{props.children}</div>
    </section>
  );
}

export function Group(props: { label: string; children: ReactNode; className?: string }) {
  return (
    <div className={`group ${props.className ?? ""}`}>
      <div className="group-label">{props.label}</div>
      {props.children}
    </div>
  );
}
