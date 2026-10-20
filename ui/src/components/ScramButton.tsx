import { useEffect, useState } from "react";

/** Guarded manual scram: first click lifts the cover (5 s), second click scrams. */
export function ScramButton(props: { onScram: () => void }) {
  const [armed, setArmed] = useState(false);
  useEffect(() => {
    if (!armed) return;
    const t = setTimeout(() => setArmed(false), 5000);
    return () => clearTimeout(t);
  }, [armed]);
  return (
    <div className={`scram-guard${armed ? " armed" : ""}`}>
      <button
        type="button"
        className="scram-btn"
        onClick={() => {
          if (!armed) setArmed(true);
          else {
            setArmed(false);
            props.onScram();
          }
        }}
        title={armed ? "Click again to SCRAM" : "Lift guard cover"}
      >
        <span className="scram-text">{armed ? "SCRAM" : "MANUAL SCRAM"}</span>
        <span className="scram-sub">{armed ? "PRESS TO TRIP" : "LIFT COVER"}</span>
      </button>
    </div>
  );
}
