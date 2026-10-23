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
  /** Lumped fuel node above its model envelope (1200 degC). Model validity, not core damage. */
  fuelEnvelopeExceeded: boolean;
  /** Point-kinetics fault of the last step; neutron power is frozen while set. */
  kineticsFault: string | null;
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
  /** Cumulative outflow requested of the vessel that its inventory could not supply. */
  inventoryShortfallKg: number;
  /** Noncondensable gas in the head space (atmospheric while vented, trapped once sealed). */
  headGasKg: number;
  /** Partial pressure of the head-space gas; total pressure = vapour + gas. */
  headGasPressureMpa: number;
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
  cwHeatRejectionMw: number;
  hotwellTempC: number;
  overflowKg: number;
  withdrawalDeficitKg: number;
}

export interface FeedPump {
  running: boolean;
  flowKgs: number;
  speedPct: number;
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
  extractionHeatMw: number;
}

export interface IcTrain {
  /** Condensate return valve position 0..1 (opening it puts the train in service). */
  valvePos: number;
  commandOpen: boolean;
  heatMw: number;
  poolTempC: number;
  poolLevelPct: number;
  boiloffKgs: number;
  condensateReturnKgs: number;
  poolEmpty: boolean;
}

export interface Alarm {
  id: string;
  text: string;
  severity: Severity;
  acked: boolean;
}

/** Annunciator transition recorded by the engine (plan C1). `key` is `cond:<id>` for condition alarms, `evt:<id>` for operator events. */
export interface AnnEvent {
  seq: number;
  timeS: number;
  kind: "onset" | "clear" | "changed";
  key: string;
  text: string;
  severity: Severity;
}

/** Result of pulling annunciator events older than a snapshot's tail (plan C1). */
export interface AnnEventsSince {
  runId: number;
  events: AnnEvent[];
  /** The engine ring no longer holds every requested event. */
  truncated: boolean;
}

export interface Snapshot {
  /** Origin scenario of the current run. */
  scenario: ScenarioName;
  /** Identity of the current run; changes on scenario load and save restore. */
  runId: number;
  /** Engine revision; increments on every command and step outcome. */
  rev: number;
  /** Latest annunciator event sequence number (0 = none yet). */
  annHead: number;
  /** The most recent annunciator events (at most 64), oldest first. */
  annTail: AnnEvent[];
  timeS: number;
  /** Requested sim-speed multiplier (simulated seconds per wall-clock second). */
  speed: number;
  /** Achieved simulated seconds per wall-clock second, smoothed; 0 while paused. Below `speed` when the host cannot keep up. */
  effectiveSpeed: number;
  /** Simulated seconds owed to the clock but not yet stepped (engine accumulator plus host wall-clock backlog x speed). */
  clockBacklogS: number;
  paused: boolean;
  modeSwitch: ModeSwitch;
  scrammed: boolean;
  scramReason: string | null;
  /** Set when the last step left the physics model's domain; the sim is paused on it. */
  modelError: string | null;
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
  /** Move rods to a target position (0..48, any fine-motion position). */
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

/** One save file's metadata (plan C2); everything except the engine state and run record. */
export interface SaveMeta {
  id: string;
  name: string;
  savedAtMs: number;
  simTimeS: number;
  scenario: ScenarioName;
  runSummary: { mode: ModeSwitch; powerPct: number; pressureMpa: number; scrammed: boolean };
  /** Engine and file versions match this build. */
  loadable: boolean;
  /** Why the save cannot be read or loaded. */
  error: string | null;
}

export interface SaveList {
  /** Maximum number of saves (defined once, by the host). */
  max: number;
  saves: SaveMeta[];
}
