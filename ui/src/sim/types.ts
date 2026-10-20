// Contract between the Rust simulator (crates/bwr-core) and the UI.
// Mirrors `Snapshot` and `Command` in crates/bwr-core/src/api.rs (serde camelCase).
// Keep both files in sync.

export type ModeSwitch = "shutdown" | "refuel" | "startup" | "run";
export type LevelControlMode = "manual" | "singleElement" | "threeElement";
export type Severity = "info" | "warning" | "alarm" | "trip";
export type ScenarioName = "coldShutdown" | "hotStandby" | "fullPower";

export interface Reactivity {
  /** All values in pcm (1 pcm = 1e-5 dk/k). */
  total: number;
  rods: number;
  doppler: number;
  void: number;
  moderator: number;
  xenon: number;
}

export interface Neutronics {
  /** Neutron power relative to rated (1.0 = 870 MWt fission power). */
  powerFrac: number;
  thermalPowerMw: number;
  decayHeatMw: number;
  /** Reactor period in seconds; null when effectively infinite (|period| > 1e4 s). */
  periodS: number | null;
  reactivity: Reactivity;
  kEff: number;
  /** Source range monitor, counts per second (saturates around 1e6). */
  srmCps: number;
  /** Intermediate range monitor: selected range 1..10 and reading 0..125 % of that range. */
  irmRange: number;
  irmReadingPct: number;
  /** Average power range monitor, % of rated thermal power. */
  aprmPct: number;
  /** Xenon-135 concentration relative to 100 % power equilibrium. */
  xenonRel: number;
  iodineRel: number;
  fuelTempC: number;
}

export interface Rods {
  /** Position of each of the 57 rods, 0 = fully inserted, 48 = fully withdrawn. */
  positions: number[];
  /** Target position of each rod. */
  targets: number[];
  /** Grid coordinates of each rod on the core map (column, row), 0-based. */
  layout: [number, number][];
  gridSize: number;
  selected: number | null;
  scramInProgress: boolean;
}

export interface Vessel {
  pressureMpa: number;
  /** Saturation temperature at vessel pressure (or bulk liquid temp when subcooled). */
  coolantTempC: number;
  /** Water level in metres above top of active fuel (TAF). */
  levelAboveTafM: number;
  /** Narrow-range level, cm relative to normal water level (NWL). */
  levelNrCm: number;
  coreFlowKgs: number;
  coreInletSubcoolingKjKg: number;
  coreAvgVoid: number;
  chimneyVoid: number;
  exitQuality: number;
  steamFlowKgs: number;
  feedFlowKgs: number;
  headVentOpen: boolean;
  /** Coolant heat-up rate, degC per hour (negative = cooldown). */
  heatupRateCph: number;
}

export interface Steam {
  /** MSIV positions per steam line, 0 closed .. 1 open. */
  msivPos: number[];
  msivCommandOpen: boolean[];
  tcvPos: number;
  bypassPos: number;
  bypassAuto: boolean;
  bypassManualPos: number;
  steamToTurbineKgs: number;
  steamToBypassKgs: number;
  pressureSetpointMpa: number;
}

export interface Turbine {
  tripped: boolean;
  speedRpm: number;
  speedSetpointRpm: number;
  breakerClosed: boolean;
  grossMwe: number;
  netMwe: number;
  /** Synchroscope angle, degrees -180..180 (0 = in phase). */
  syncPhaseDeg: number;
  /** Slip frequency, Hz (generator minus grid). */
  slipHz: number;
  loadLimitPct: number;
}

export interface Condenser {
  pressureKpa: number;
  hotwellLevelPct: number;
  cwPumps: boolean[];
  vacuumPumpOn: boolean;
  heatLoadMw: number;
}

export interface FeedPump {
  running: boolean;
  flowKgs: number;
}

export interface Feedwater {
  pumps: FeedPump[];
  condensatePumps: boolean[];
  fcvPos: number;
  fcvManualPos: number;
  levelControlMode: LevelControlMode;
  levelSetpointCm: number;
  feedTempC: number;
  heatersInService: boolean;
  rwcuRejectKgs: number;
}

export interface IcTrain {
  /** Condensate return valve position 0..1 (opening it puts the train in service). */
  valvePos: number;
  commandOpen: boolean;
  heatMw: number;
  poolTempC: number;
  poolLevelPct: number;
}

export interface Alarm {
  id: string;
  text: string;
  severity: Severity;
  acked: boolean;
}

export interface Snapshot {
  timeS: number;
  speed: number;
  paused: boolean;
  modeSwitch: ModeSwitch;
  scrammed: boolean;
  scramReason: string | null;
  neutronics: Neutronics;
  rods: Rods;
  vessel: Vessel;
  steam: Steam;
  turbine: Turbine;
  condenser: Condenser;
  feedwater: Feedwater;
  ic: IcTrain[];
  alarms: Alarm[];
}

export type Command =
  | { type: "setPaused"; paused: boolean }
  | { type: "setSpeed"; speed: number }
  | { type: "loadScenario"; name: ScenarioName }
  | { type: "manualScram" }
  | { type: "resetScram" }
  | { type: "setModeSwitch"; mode: ModeSwitch }
  | { type: "selectRod"; index: number | null }
  /** Move rods to a target position (0..48, even notches). */
  | { type: "rodSetTarget"; indices: number[]; target: number }
  | { type: "rodStop" }
  | { type: "setIrmRange"; range: number }
  | { type: "setHeadVent"; open: boolean }
  | { type: "setMsiv"; line: number; open: boolean }
  | { type: "setPressureSetpoint"; mpa: number }
  | { type: "setBypass"; auto: boolean; pos: number }
  | { type: "setSpeedSetpoint"; rpm: number }
  | { type: "resetTurbine" }
  | { type: "tripTurbine" }
  | { type: "closeBreaker" }
  | { type: "openBreaker" }
  | { type: "setLoadLimit"; pct: number }
  | { type: "setCwPump"; index: number; on: boolean }
  | { type: "setVacuumPump"; on: boolean }
  | { type: "setFeedPump"; index: number; on: boolean }
  | { type: "setCondensatePump"; index: number; on: boolean }
  | { type: "setLevelControl"; mode: LevelControlMode }
  | { type: "setFcvManual"; pos: number }
  | { type: "setLevelSetpoint"; cm: number }
  | { type: "setHeaters"; inService: boolean }
  | { type: "setRwcuReject"; kgs: number }
  | { type: "setIcTrain"; index: number; open: boolean }
  | { type: "ackAlarms" };
