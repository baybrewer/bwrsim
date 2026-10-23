// Operator-facing names shared by the log text and (later) the panels, so wording has one source.
import type { LevelControlMode, ModeSwitch, ScenarioName } from "../types";

export const SCENARIO_LABELS: Record<ScenarioName, string> = {
  coldShutdown: "COLD SHUTDOWN",
  hotStandby: "HOT STANDBY",
  fullPower: "FULL POWER",
};

export const MODE_LABELS: Record<ModeSwitch, string> = {
  shutdown: "SHUTDOWN",
  refuel: "REFUEL",
  startup: "STARTUP",
  run: "RUN",
};

export const LEVEL_MODE_LABELS: Record<LevelControlMode, string> = {
  manual: "MANUAL",
  singleElement: "1-ELEM",
  threeElement: "3-ELEM",
};

/** Equipment trains are lettered from A on the panels (FWP A, CW PUMP B, MSIV A, IC train C). */
export function trainLetter(index: number): string {
  return String.fromCharCode(65 + index);
}
