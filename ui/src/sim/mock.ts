// In-browser stand-in for the Rust simulator, used when the UI runs outside Tauri.
// The physics here are deliberately crude: they only need to be plausible enough
// that every gauge moves and every button does something while developing the UI.

import type {
  Alarm,
  AnnEvent,
  AnnEventsSince,
  Command,
  ModeSwitch,
  ScenarioName,
  Severity,
  Snapshot,
} from "./types";
import { RUN_CHANGED } from "./logic/protocol.ts";
import { LEVEL_MODE_LABELS, MODE_LABELS, SCENARIO_LABELS } from "./logic/labels.ts";
import { SEVERITIES } from "./logic/log.ts";

const RATED_MWT = 870;
const BETA = 650; // pcm
const LAMBDA = 0.0767; // 1/s, one-group delayed neutron decay constant
const SOURCE_PCM = 1.2e-5; // neutron source term: subcritical P = SOURCE_PCM / -rho
const NWL_ABOVE_TAF_M = 9.5;
const ROD_SPEED = 2 / 3; // notch units per second (one notch every 3 s)
const SCRAM_SPEED = 16; // units per second
// Two 100 % adjustable-speed reactor feed pumps (GE 005N9751 Rev. J, section 5.1.6).
const N_FEED = 2;
const FEED_PUMP_CAP_KGS = 650; // per pump at zero vessel pressure (~rated flow at 7.2 MPa)
// 3,600 rpm two-pole 60 Hz machine (GE 005N9751 Rev. J, section 5.1.2): f = rpm / 60.
const RATED_RPM = 3600;
const OVERSPEED_RPM = 1.1 * RATED_RPM;
const STEAM_K_TCV = 72; // kg/s per MPa at full open
const STEAM_K_BYP = 36;
const IC_MW = 33;
const LAMBDA_I = 2.87e-5;
const LAMBDA_X = 2.09e-5;
const XE_BURN = 2;
const FUEL_ENVELOPE_C = 1200; // lumped fuel node model limit (same as the engine flag)
const COOLANT_MAX_C = 320; // upper edge of the mock's coolant-temperature model
const HOTWELL_KG_PER_PCT = 400; // hotwell inventory per % level (matches the level equation)
const VESSEL_KG_PER_M = 8900;
const PUMP_RAMP_PCT_S = 10; // ASD speed ramp
const H_FG_POOL = 2257; // kJ/kg, pool boiling at atmospheric pressure
const H_FG_IC = 1500; // kJ/kg, primary steam condensed in the IC tubes (~7 MPa)
const FEED_CP = 4.19; // kJ/kg/K
const COLD_FEED_C = 40;
const HEAD_SPACE_M3 = 100; // mock head-space volume for the trapped-gas readout
const R_AIR_KJ = 0.287; // kJ/kg/K (R_AIR in crates/bwr-core/src/vessel.rs)
const P_ATM_MPA = 0.101325;
const WATER_RHO = 750; // kg/m3, converts inventory level change into head-space volume
const HEAD_SPACE_MIN_M3 = 5; // floor so a high level cannot compress the gas to zero volume
// Feed heating: extraction enthalpy per kg of turbine throttle steam, sized so rated
// steam (475 kg/s) delivers the ~380 MW that lifts rated feed from 40 to ~230 C.
const EXTRACTION_KJ_PER_KG = 800;
const EXTRACTION_STEAM_CAP_KGS = 520; // bounds the extraction budget above rated flow
const FEED_TEMP_MAX_C = 230; // top heater outlet limit
const HEATER_TAU_S = 120; // heater shell/tube storage time constant
const EFFECTIVE_SPEED_TAU_S = 1; // smoothing of the achieved-speed readout
// Annunciator event ring and published tail (same sizes as the engine, plan C1).
const ANN_RING_LEN = 1024;
const ANN_TAIL_LEN = 64;
/** Version of the exported mock state; bump when Hidden or the state payload changes shape or meaning. */
export const MOCK_STATE_VERSION = 1;

// Run ids are never reused within a page (the run store orders runs by id).
let nextRunId = 1;
export function allocateRunId(): number {
  return nextRunId++;
}

/** 57 rods on a 9x9 grid: drop a 3+2+1 triangle from every corner. */
function buildLayout(): [number, number][] {
  const out: [number, number][] = [];
  for (let row = 0; row < 9; row++) {
    const edge = Math.min(row, 8 - row);
    const cut = Math.max(0, 3 - edge);
    for (let col = cut; col < 9 - cut; col++) out.push([col, row]);
  }
  return out;
}

const LAYOUT = buildLayout();
const WEIGHTS = (() => {
  const w = LAYOUT.map(([c, r]) => 1.6 - Math.hypot(c - 4, r - 4) / 5);
  const s = w.reduce((a, b) => a + b, 0);
  return w.map((x) => x / s);
})();

const tsat = (pMpa: number) => 179.9 * Math.pow(Math.max(pMpa, 0.01), 0.24);
const psat = (tC: number) => Math.pow(Math.max(tC, 1) / 179.9, 1 / 0.24);
const approach = (v: number, target: number, rate: number, dt: number) =>
  v < target ? Math.min(target, v + rate * dt) : Math.max(target, v - rate * dt);
const clamp = (v: number, lo: number, hi: number) => Math.max(lo, Math.min(hi, v));
/** Feed pump ASD speed demand: idle at 25 % when running, rising with valve demand. */
const pumpSpeedDemand = (running: boolean, fcvPos: number) => (running ? 25 + 75 * fcvPos : 0);
/** Ideal-gas mass of air at partial pressure pMpa (kPa * m3 / (kJ/kg/K * K) = kg). */
const gasKg = (pMpa: number, tC: number, volM3: number) => (pMpa * 1e3 * volM3) / (R_AIR_KJ * (tC + 273.15));
const gasPressureMpa = (kg: number, tC: number, volM3: number) => (kg * R_AIR_KJ * (tC + 273.15)) / (volM3 * 1e3);
/** Air partial pressure of a vented head space: tops vapour up to atmospheric. */
const ventedGasMpa = (tC: number) => Math.max(0, P_ATM_MPA - psat(tC));

interface Hidden {
  precursor: number; // normalized so equilibrium precursor == power
  decayFrac: number;
  rhoBias: number;
  pressInt: number;
  levelInt: number;
  massLevelM: number;
  alarmState: Map<string, Alarm>;
  prevPower: number;
  omega: number;
  /** Latest annunciator seq issued (0 = none yet). */
  annSeq: number;
  annRing: AnnEvent[];
  /** Visible alarms at the last diff (the comparison baseline), keyed `cond:<id>`. */
  annPrev: Map<string, { text: string; severity: Severity }>;
  /** Instrument-noise generator state, saved so a restored state continues identically. */
  rng: number;
}

type HiddenJson = Omit<Hidden, "alarmState" | "annPrev"> & {
  alarmState: [string, Alarm][];
  annPrev: [string, { text: string; severity: Severity }][];
};

/** Exported mock engine state (plan C5); listeners and timers are not part of it. */
export interface MockState {
  mockVersion: number;
  snapshot: Snapshot;
  hidden: HiddenJson;
  phase: number;
}

const HIDDEN_NUMBERS = ["precursor", "decayFrac", "rhoBias", "pressInt", "levelInt", "massLevelM", "prevPower", "omega", "annSeq", "rng"] as const;

function isObj(x: unknown): x is Record<string, unknown> {
  return typeof x === "object" && x !== null && !Array.isArray(x);
}

const ANN_KINDS: readonly string[] = ["onset", "clear", "changed"];
/** Snapshot fields that are null in the blank template, with the type they carry when set. */
const NULLABLE: Record<string, "number" | "string"> = {
  scramReason: "string",
  modelError: "string",
  "neutronics.periodS": "number",
  "neutronics.kineticsFault": "string",
  "rods.selected": "number",
};
const ENUMS: Record<string, readonly string[]> = {
  scenario: Object.keys(SCENARIO_LABELS),
  modeSwitch: Object.keys(MODE_LABELS),
  "feedwater.levelControlMode": Object.keys(LEVEL_MODE_LABELS),
};

function isAlarm(x: unknown): x is Alarm {
  return isObj(x) && typeof x.id === "string" && typeof x.text === "string" && SEVERITIES.includes(x.severity as Severity) && typeof x.acked === "boolean";
}

function isAnnEvent(x: unknown): x is AnnEvent {
  return (
    isObj(x) &&
    Number.isInteger(x.seq) &&
    (x.seq as number) > 0 &&
    Number.isFinite(x.timeS) &&
    ANN_KINDS.includes(x.kind as string) &&
    typeof x.key === "string" &&
    typeof x.text === "string" &&
    SEVERITIES.includes(x.severity as Severity)
  );
}

/** Snapshot lists whose length varies; every other array has the template's fixed length. */
const VARIABLE_LISTS: Record<string, (x: unknown) => boolean> = { alarms: isAlarm, annTail: isAnnEvent };

/**
 * First path where `x` departs from the blank-snapshot template (types, nested objects,
 * fixed array lengths, enums), or null. The template is the SSOT for the Snapshot shape
 * the mock publishes, so a restored state can be stepped and rendered like a fresh one.
 */
function shapeError(tpl: unknown, x: unknown, path: string): string | null {
  const bad = path || "snapshot";
  if (path in ENUMS) return typeof x === "string" && ENUMS[path].includes(x) ? null : bad;
  if (path in VARIABLE_LISTS) return Array.isArray(x) && x.every(VARIABLE_LISTS[path]) ? null : bad;
  if (tpl === null) {
    const kind = NULLABLE[path];
    return x === null || (kind === "number" ? Number.isFinite(x) : typeof x === kind) ? null : bad;
  }
  if (typeof tpl === "number") return Number.isFinite(x) ? null : bad;
  if (typeof tpl !== "object") return typeof x === typeof tpl ? null : bad;
  if (Array.isArray(tpl)) {
    if (!Array.isArray(x) || x.length !== tpl.length) return bad;
    for (let i = 0; i < tpl.length; i++) {
      const err = shapeError(tpl[i], x[i], path);
      if (err) return `${err}[${i}]`;
    }
    return null;
  }
  if (!isObj(x)) return bad;
  for (const [k, v] of Object.entries(tpl)) {
    const err = shapeError(v, x[k], path ? `${path}.${k}` : k);
    if (err) return err;
  }
  return null;
}

/**
 * Full check of an untrusted exported state (save files): the snapshot against the
 * template, every Hidden field, and the annunciator ring. Run before staging a load so a
 * damaged state is rejected while the active sim is untouched (C2/C5).
 */
export function parseMockState(x: unknown): MockState | Error {
  if (!isObj(x)) return new Error("mock state: not an object");
  if (x.mockVersion !== MOCK_STATE_VERSION) return new Error(`mock state: unsupported version ${String(x.mockVersion)}`);
  const { snapshot: s, hidden: h } = x;
  if (!Number.isFinite(x.phase)) return new Error("mock state: phase not finite");
  const bad = shapeError(blankSnapshot(), s, "");
  if (bad) return new Error(`mock state: bad snapshot field ${bad}`);
  if (!isObj(h)) return new Error("mock state: hidden state missing");
  for (const k of HIDDEN_NUMBERS) if (!Number.isFinite(h[k])) return new Error(`mock state: hidden.${k} not finite`);
  const pairs = (v: unknown, ok: (key: string, val: unknown) => boolean) =>
    Array.isArray(v) && v.every((e) => Array.isArray(e) && e.length === 2 && typeof e[0] === "string" && ok(e[0], e[1]));
  if (!pairs(h.alarmState, (id, a) => isAlarm(a) && a.id === id)) return new Error("mock state: alarm state malformed");
  const visible = (_: string, v: unknown) => isObj(v) && typeof v.text === "string" && SEVERITIES.includes(v.severity as Severity);
  if (!pairs(h.annPrev, visible)) return new Error("mock state: annunciator baseline malformed");
  // The ring is the engine's: ascending seqs, at most ANN_RING_LEN, none beyond the issued head.
  const ring = h.annRing;
  const annSeq = h.annSeq as number;
  if (!Number.isInteger(annSeq) || annSeq < 0) return new Error("mock state: hidden.annSeq not a sequence number");
  if (
    !Array.isArray(ring) ||
    ring.length > ANN_RING_LEN ||
    !ring.every((e, i) => isAnnEvent(e) && (i === 0 || e.seq > (ring[i - 1] as AnnEvent).seq) && e.seq <= annSeq)
  )
    return new Error("mock state: annunciator ring malformed");
  return x as unknown as MockState;
}

/** The snapshot a restored state publishes: paused, a new run id, no wall-clock readouts. */
export function restoredSnapshot(state: MockState, runId: number): Snapshot {
  const s = structuredClone(state.snapshot);
  s.runId = runId;
  s.paused = true;
  s.effectiveSpeed = 0;
  s.clockBacklogS = 0;
  s.annHead = state.hidden.annSeq;
  s.annTail = state.hidden.annRing.slice(-ANN_TAIL_LEN);
  return s;
}

function blankSnapshot(): Snapshot {
  return {
    // Run identity and annunciator events are set by loadScenario / importState.
    scenario: "hotStandby",
    runId: 0,
    rev: 0,
    annHead: 0,
    annTail: [],
    timeS: 0,
    speed: 1,
    effectiveSpeed: 0,
    clockBacklogS: 0,
    paused: false,
    modeSwitch: "shutdown",
    scrammed: false,
    scramReason: null,
    modelError: null,
    neutronics: {
      powerFrac: 0,
      thermalPowerMw: 0,
      decayHeatMw: 0,
      periodS: null,
      reactivity: { total: 0, rods: 0, doppler: 0, void: 0, moderator: 0, xenon: 0 },
      kEff: 1,
      srmCps: 0,
      irmRange: 1,
      irmReadingPct: 0,
      aprmPct: 0,
      xenonRel: 0,
      iodineRel: 0,
      fuelTempC: 20,
      fuelEnvelopeExceeded: false,
      kineticsFault: null,
    },
    rods: {
      positions: LAYOUT.map(() => 0),
      targets: LAYOUT.map(() => 0),
      layout: LAYOUT.map(([c, r]) => [c, r] as [number, number]),
      gridSize: 9,
      selected: null,
      scramInProgress: false,
    },
    vessel: {
      pressureMpa: 0.1,
      coolantTempC: 40,
      levelAboveTafM: NWL_ABOVE_TAF_M,
      levelNrCm: 0,
      coreFlowKgs: 0,
      coreInletSubcoolingKjKg: 0,
      coreAvgVoid: 0,
      chimneyVoid: 0,
      exitQuality: 0,
      steamFlowKgs: 0,
      feedFlowKgs: 0,
      headVentOpen: true,
      heatupRateCph: 0,
      inventoryShortfallKg: 0,
      headGasKg: 0,
      headGasPressureMpa: 0,
    },
    steam: {
      msivPos: [0, 0],
      msivCommandOpen: [false, false],
      tcvPos: 0,
      bypassPos: 0,
      bypassAuto: true,
      bypassManualPos: 0,
      steamToTurbineKgs: 0,
      steamToBypassKgs: 0,
      pressureSetpointMpa: 7.2,
    },
    turbine: {
      tripped: true,
      speedRpm: 0,
      speedSetpointRpm: 0,
      breakerClosed: false,
      grossMwe: 0,
      netMwe: 0,
      syncPhaseDeg: 0,
      slipHz: -60, // electrical frequency at standstill minus the 60 Hz grid
      loadLimitPct: 100,
    },
    condenser: {
      pressureKpa: 101,
      hotwellLevelPct: 60,
      cwPumps: [true, false],
      vacuumPumpOn: false,
      heatLoadMw: 0,
      cwHeatRejectionMw: 0,
      hotwellTempC: 35,
      overflowKg: 0,
      withdrawalDeficitKg: 0,
    },
    feedwater: {
      pumps: Array.from({ length: N_FEED }, () => ({ running: false, flowKgs: 0, speedPct: 0 })),
      condensatePumps: [true, false],
      fcvPos: 0,
      fcvManualPos: 0,
      levelControlMode: "manual",
      levelSetpointCm: 0,
      feedTempC: 40,
      heatersInService: false,
      rwcuRejectKgs: 0,
      extractionHeatMw: 0,
    },
    ic: Array.from({ length: 3 }, () => ({
      valvePos: 0,
      commandOpen: false,
      heatMw: 0,
      poolTempC: 30,
      poolLevelPct: 100,
      boiloffKgs: 0,
      condensateReturnKgs: 0,
      poolEmpty: false,
    })),
    alarms: [],
  };
}

export class MockSim {
  private s: Snapshot = blankSnapshot();
  private h: Hidden = {
    precursor: 0,
    decayFrac: 0,
    rhoBias: 0,
    pressInt: 0,
    levelInt: 0,
    massLevelM: NWL_ABOVE_TAF_M,
    alarmState: new Map(),
    prevPower: 0,
    omega: 0,
    annSeq: 0,
    annRing: [],
    annPrev: new Map(),
    rng: (Math.random() * 2 ** 32) | 0,
  };
  // Wall-clock time of the previous timer tick, for the achieved-speed readout.
  private lastWallMs: number | null = null;
  private listeners = new Set<(s: Snapshot) => void>();
  private timer: ReturnType<typeof setInterval> | null = null;
  private phase = 0;

  constructor() {
    this.loadScenario("hotStandby");
  }

  start() {
    if (this.timer) return;
    this.timer = setInterval(() => this.tick(0.1), 100);
  }

  stop() {
    if (this.timer) clearInterval(this.timer);
    this.timer = null;
    this.lastWallMs = null;
  }

  subscribe(cb: (s: Snapshot) => void): () => void {
    this.listeners.add(cb);
    cb(this.publish());
    this.start();
    return () => {
      this.listeners.delete(cb);
      if (this.listeners.size === 0) this.stop();
    };
  }

  private publish(): Snapshot {
    // Hand out a deep copy so React sees a new object each tick.
    return structuredClone(this.s);
  }

  /** The current published state (a copy). */
  snapshot(): Snapshot {
    return this.publish();
  }

  get runId(): number {
    return this.s.runId;
  }

  get rev(): number {
    return this.s.rev;
  }

  /** Annunciator events of `runId` after `seq` (engine `ann_events_since`). */
  eventsSince(runId: number, seq: number): AnnEventsSince {
    if (runId !== this.s.runId) throw new Error(RUN_CHANGED);
    const ring = this.h.annRing;
    const truncated = seq < this.h.annSeq && (ring.length === 0 || ring[0].seq > seq + 1);
    return { runId, events: ring.filter((e) => e.seq > seq).map((e) => ({ ...e })), truncated };
  }

  exportState(): MockState {
    const { alarmState, annPrev, ...rest } = this.h;
    return structuredClone({
      mockVersion: MOCK_STATE_VERSION,
      snapshot: this.s,
      hidden: { ...rest, alarmState: [...alarmState], annPrev: [...annPrev] },
      phase: this.phase,
    });
  }

  /**
   * Restores an exported state into this instance (listeners and timer stay), as a new
   * run that is paused; the wall-clock anchor restarts so no elapsed time is charged.
   */
  importState(state: MockState, runId = allocateRunId()): Snapshot {
    const st = structuredClone(state);
    this.s = restoredSnapshot(st, runId);
    this.h = { ...st.hidden, alarmState: new Map(st.hidden.alarmState), annPrev: new Map(st.hidden.annPrev) };
    this.phase = st.phase;
    this.lastWallMs = null;
    this.emit();
    return this.publish();
  }

  /** Seeded instrument noise in [0, 1) (mulberry32), so restored runs replay identically. */
  private random(): number {
    let t = (this.h.rng = (this.h.rng + 0x6d2b79f5) | 0);
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  }

  /** Engine semantics: diff the visible alarm list against the baseline and sequence the transitions. */
  private recordAnnunciator() {
    const s = this.s;
    const h = this.h;
    const cur: Hidden["annPrev"] = new Map(s.alarms.map((a) => [`cond:${a.id}`, { text: a.text, severity: a.severity }]));
    const push = (kind: AnnEvent["kind"], key: string, a: { text: string; severity: Severity }) => {
      h.annRing.push({ seq: ++h.annSeq, timeS: s.timeS, kind, key, text: a.text, severity: a.severity });
      if (h.annRing.length > ANN_RING_LEN) h.annRing.shift();
    };
    for (const [key, p] of h.annPrev) if (!cur.has(key)) push("clear", key, p);
    for (const [key, c] of cur) {
      const p = h.annPrev.get(key);
      if (!p) push("onset", key, c);
      else if (p.text !== c.text || p.severity !== c.severity) push("changed", key, c);
    }
    h.annPrev = cur;
    s.annHead = h.annSeq;
    s.annTail = h.annRing.slice(-ANN_TAIL_LEN);
  }

  // ---------------------------------------------------------------- commands
  command(cmd: Command): void {
    const s = this.s;
    switch (cmd.type) {
      case "setPaused":
        s.paused = cmd.paused;
        break;
      case "setSpeed":
        s.speed = clamp(cmd.speed, 0.1, 100);
        break;
      case "loadScenario":
        this.loadScenario(cmd.name);
        break;
      case "manualScram":
        this.scram("MANUAL SCRAM");
        break;
      case "resetScram":
        // The mode switch in SHUTDOWN is itself a scram signal: the reset is
        // blocked until another mode is selected (same as the Rust engine).
        if (s.scrammed && s.rods.positions.every((p) => p < 0.5) && s.modeSwitch !== "shutdown") {
          s.scrammed = false;
          s.scramReason = null;
          s.rods.scramInProgress = false;
        }
        break;
      case "setModeSwitch":
        this.setMode(cmd.mode);
        break;
      case "selectRod":
        s.rods.selected = cmd.index;
        break;
      case "rodSetTarget": {
        // Validate the whole batch against the current state before touching any
        // target, so a rejected command has no partial effect.
        if (s.scrammed) throw new Error("ROD BLOCK: scram not reset");
        // Fine-motion drives: any position in [0, 48] is valid (same as the engine).
        if (!Number.isFinite(cmd.target)) throw new Error("Command rejected: non-finite rod target");
        const t = clamp(cmd.target, 0, 48);
        const accepted = cmd.indices.filter((i) => Number.isInteger(i) && i >= 0 && i < s.rods.targets.length);
        const withdrawBlocked = s.modeSwitch === "shutdown" || s.modeSwitch === "refuel";
        if (withdrawBlocked && accepted.some((i) => t > s.rods.positions[i] + 1e-6))
          throw new Error(`ROD WITHDRAW BLOCK: mode switch in ${s.modeSwitch.toUpperCase()}`);
        for (const i of accepted) s.rods.targets[i] = t;
        break;
      }
      case "rodStop":
        // Fine-motion drives stop where they are; no snap to a notch.
        s.rods.targets = [...s.rods.positions];
        break;
      case "setIrmRange":
        s.neutronics.irmRange = clamp(Math.round(cmd.range), 1, 10);
        break;
      case "setHeadVent":
        s.vessel.headVentOpen = cmd.open;
        break;
      case "setMsiv":
        if (cmd.line >= 0 && cmd.line < s.steam.msivCommandOpen.length)
          s.steam.msivCommandOpen[cmd.line] = cmd.open;
        break;
      case "setPressureSetpoint":
        s.steam.pressureSetpointMpa = clamp(cmd.mpa, 0.2, 7.8);
        break;
      case "setBypass":
        s.steam.bypassAuto = cmd.auto;
        s.steam.bypassManualPos = clamp(cmd.pos, 0, 1);
        break;
      case "setSpeedSetpoint":
        s.turbine.speedSetpointRpm = clamp(cmd.rpm, 0, RATED_RPM * 1.02);
        break;
      case "resetTurbine":
        if (s.condenser.pressureKpa > 30) throw new Error("TURBINE RESET BLOCKED: low condenser vacuum");
        s.turbine.tripped = false;
        break;
      case "tripTurbine":
        s.turbine.tripped = true;
        s.turbine.breakerClosed = false;
        break;
      case "closeBreaker": {
        const t = s.turbine;
        if (t.tripped) throw new Error("BREAKER CLOSE BLOCKED: turbine tripped");
        if (Math.abs(t.slipHz) > 0.2 || Math.abs(t.syncPhaseDeg) > 20)
          throw new Error("SYNC CHECK: out of phase / slip too high");
        t.breakerClosed = true;
        break;
      }
      case "openBreaker":
        s.turbine.breakerClosed = false;
        break;
      case "setLoadLimit":
        s.turbine.loadLimitPct = clamp(cmd.pct, 0, 110);
        break;
      case "setCwPump":
        s.condenser.cwPumps[cmd.index] = cmd.on;
        break;
      case "setVacuumPump":
        s.condenser.vacuumPumpOn = cmd.on;
        break;
      case "setFeedPump":
        if (s.feedwater.pumps[cmd.index]) s.feedwater.pumps[cmd.index].running = cmd.on;
        break;
      case "setCondensatePump":
        s.feedwater.condensatePumps[cmd.index] = cmd.on;
        break;
      case "setLevelControl":
        s.feedwater.levelControlMode = cmd.mode;
        this.h.levelInt = cmd.mode === "threeElement" ? 0 : s.feedwater.fcvPos;
        break;
      case "setFcvManual":
        s.feedwater.fcvManualPos = clamp(cmd.pos, 0, 1);
        break;
      case "setLevelSetpoint":
        s.feedwater.levelSetpointCm = clamp(cmd.cm, -100, 100);
        break;
      case "setHeaters":
        s.feedwater.heatersInService = cmd.inService;
        break;
      case "setRwcuReject":
        s.feedwater.rwcuRejectKgs = clamp(cmd.kgs, 0, 10);
        break;
      case "setIcTrain":
        if (s.ic[cmd.index]) s.ic[cmd.index].commandOpen = cmd.open;
        break;
      case "ackAlarms":
        // Acknowledges the active alarms (the only ones listed); updateAlarms republishes them.
        for (const a of this.h.alarmState.values()) a.acked = true;
        break;
    }
    // Engine apply: one revision per command, then the annunciator diff (this.s may be a new run).
    this.stepOutcome();
    this.emit();
  }

  private emit() {
    const snap = this.publish();
    for (const l of this.listeners) l(snap);
  }

  private setMode(mode: ModeSwitch) {
    const prev = this.s.modeSwitch;
    this.s.modeSwitch = mode;
    if (mode === "shutdown" && prev !== "shutdown") this.scram("MODE SWITCH IN SHUTDOWN");
  }

  private scram(reason: string) {
    const s = this.s;
    if (!s.scrammed) s.scramReason = reason;
    s.scrammed = true;
    s.rods.scramInProgress = true;
    s.rods.targets = s.rods.targets.map(() => 0);
  }

  // --------------------------------------------------------------- scenarios
  private loadScenario(name: ScenarioName) {
    const { speed: keepSpeed, paused: keepPaused, effectiveSpeed: keepEffective } = this.s;
    const s = blankSnapshot();
    s.scenario = name;
    s.speed = keepSpeed;
    s.paused = keepPaused;
    s.effectiveSpeed = keepEffective;
    this.s = s;
    this.h.alarmState.clear();
    this.h.pressInt = 0;
    this.h.levelInt = 0;
    this.h.rhoBias = 0;

    if (name === "coldShutdown") {
      s.modeSwitch = "shutdown";
      s.neutronics.powerFrac = 1e-9;
      s.neutronics.irmRange = 1;
      this.h.decayFrac = 0.002;
      s.neutronics.fuelTempC = 40;
      s.vessel.levelAboveTafM = NWL_ABOVE_TAF_M;
      this.h.massLevelM = NWL_ABOVE_TAF_M + 0.45 * 1.2;
    } else {
      const full = name === "fullPower";
      const pattern = full ? 44 : 30;
      LAYOUT.forEach(([c, r], i) => {
        const ring = Math.max(Math.abs(c - 4), Math.abs(r - 4));
        const pos = full ? (ring >= 3 ? 48 : pattern - ((c + r) % 2) * 8) : (c + r) % 2 === 0 ? pattern : 12;
        s.rods.positions[i] = s.rods.targets[i] = pos;
      });
      s.modeSwitch = full ? "run" : "startup";
      s.neutronics.powerFrac = full ? 1 : 0.02;
      s.neutronics.irmRange = 10;
      s.neutronics.fuelTempC = full ? 900 : 292;
      s.neutronics.xenonRel = full ? 1 : 0;
      s.neutronics.iodineRel = full ? 1 : 0;
      this.h.decayFrac = full ? 0.065 : 0.004;
      s.neutronics.thermalPowerMw = s.neutronics.powerFrac * RATED_MWT * (1 - 0.065) + this.h.decayFrac * RATED_MWT;
      s.neutronics.decayHeatMw = this.h.decayFrac * RATED_MWT;
      s.vessel.pressureMpa = full ? 7.2 : 6.9;
      s.vessel.coolantTempC = tsat(s.vessel.pressureMpa);
      s.vessel.headVentOpen = false;
      s.steam.msivPos = [1, 1];
      s.steam.msivCommandOpen = [true, true];
      s.steam.pressureSetpointMpa = full ? 7.2 : 6.9;
      s.condenser.cwPumps = [true, full];
      s.condenser.vacuumPumpOn = true;
      s.condenser.pressureKpa = full ? 6 : 9;
      s.feedwater.condensatePumps = [true, full];
      s.feedwater.pumps[0].running = true;
      s.feedwater.pumps[1].running = full;
      s.feedwater.levelControlMode = full ? "threeElement" : "singleElement";
      s.feedwater.heatersInService = full;
      s.feedwater.fcvPos = full ? 0.79 : 0.03;
      s.feedwater.feedTempC = full ? 230 : 40;
      this.h.levelInt = full ? 0 : s.feedwater.fcvPos;
      s.feedwater.rwcuRejectKgs = 0;
      if (full) {
        s.turbine.tripped = false;
        s.turbine.speedRpm = RATED_RPM;
        s.turbine.speedSetpointRpm = RATED_RPM;
        s.turbine.breakerClosed = true;
        s.turbine.slipHz = 0;
        s.vessel.steamFlowKgs = s.vessel.feedFlowKgs = 475;
        this.h.pressInt = 0.94;
        s.steam.tcvPos = 0.94;
        s.steam.steamToTurbineKgs = 475;
        s.turbine.grossMwe = 312;
        s.turbine.netMwe = 290;
      } else {
        this.h.pressInt = 0.016;
        s.steam.bypassPos = 0.032;
      }
      this.updateVoids();
      this.h.massLevelM = NWL_ABOVE_TAF_M - (s.vessel.chimneyVoid - 0.45) * 1.2;
      s.vessel.levelAboveTafM = NWL_ABOVE_TAF_M;
      // Choose a bias so the chosen rod pattern is just critical.
      this.computeReactivity();
      this.h.rhoBias = -s.neutronics.reactivity.total;
      this.computeReactivity();
    }
    // Loaded plant starts at its steady pump speed rather than ramping from zero.
    for (const p of s.feedwater.pumps) p.speedPct = pumpSpeedDemand(p.running, s.feedwater.fcvPos);
    // Head-space gas starts at the vented equilibrium (none above the atmospheric boiling point).
    s.vessel.headGasKg = gasKg(ventedGasMpa(s.vessel.coolantTempC), s.vessel.coolantTempC, this.headSpaceM3());
    this.updateHeadSpace();
    this.h.precursor = s.neutronics.powerFrac;
    this.h.prevPower = s.neutronics.powerFrac;
    this.h.omega = 0;
    this.derive();
    // Initialization boundary (C1): a fresh run whose visible alarms are onsets at time 0.
    s.runId = allocateRunId();
    s.rev = 0;
    this.h.annSeq = 0;
    this.h.annRing = [];
    this.h.annPrev = new Map();
    this.recordAnnunciator();
  }

  // ----------------------------------------------------------------- physics
  /** Timer callback (public so tests can drive the mock without timers). */
  tick(realDt: number) {
    const s = this.s;
    const t0 = s.timeS;
    // A paused tick changes no engine state: no revision and no annunciator diff.
    const stepped = !s.paused;
    if (stepped) {
      let remaining = realDt * s.speed;
      while (remaining > 1e-9) {
        const dt = Math.min(0.05, remaining);
        this.step(dt);
        remaining -= dt;
        // One engine step outcome per substep (C1/C5): its own revision and annunciator diff,
        // so an alarm that comes and goes between publishes is still sequenced, at its own time.
        this.stepOutcome();
        if (s.paused) break; // a model-domain error pauses mid-tick, after its outcome is recorded
      }
    }
    // The mock steps each tick's time synchronously and drops late timer time rather
    // than carrying it, so it has no backlog; a throttled timer shows as low achieved speed.
    s.clockBacklogS = 0;
    const wallNow = performance.now();
    const wallDt = this.lastWallMs === null ? realDt : Math.max((wallNow - this.lastWallMs) / 1000, 1e-3);
    this.lastWallMs = wallNow;
    const rate = (s.timeS - t0) / wallDt;
    // The first sample after start or resume seeds the filter instead of ramping from zero.
    const alpha = s.effectiveSpeed === 0 ? 1 : Math.min(1, wallDt / EFFECTIVE_SPEED_TAU_S);
    s.effectiveSpeed = s.paused ? 0 : s.effectiveSpeed + (rate - s.effectiveSpeed) * alpha;
    // Published once per timer tick, like the host's fixed publish rate.
    this.emit();
  }

  /** Engine apply bookkeeping after a command or a step: one revision, then the annunciator diff. */
  private stepOutcome() {
    this.s.rev++;
    this.updateAlarms();
    this.recordAnnunciator();
  }

  private computeReactivity() {
    const s = this.s;
    const n = s.neutronics;
    const f = s.rods.positions.reduce((acc, p, i) => acc + WEIGHTS[i] * (p / 48), 0);
    const r = n.reactivity;
    r.rods = -12000 + 20000 * f + this.h.rhoBias;
    r.doppler = -2.4 * (n.fuelTempC - s.vessel.coolantTempC);
    r.moderator = -8 * (s.vessel.coolantTempC - 20);
    r.void = -4000 * s.vessel.coreAvgVoid;
    r.xenon = -2800 * n.xenonRel;
    r.total = r.rods + r.doppler + r.moderator + r.void + r.xenon;
    n.kEff = 1 / (1 - r.total * 1e-5);
  }

  /** Void fraction lags power (tau ~1.5 s); instant when dt is undefined (scenario load). */
  private updateVoids(dt?: number) {
    const v = this.s.vessel;
    const q = this.s.neutronics.thermalPowerMw / RATED_MWT;
    // Bubbles inside the core liquid oppose total mechanical pressure (gas included), as in the engine's core_sat().
    const sat = v.coolantTempC >= tsat(v.pressureMpa) - 0.5 && v.pressureMpa > 0.15;
    const pScale = clamp(7.2 / Math.max(v.pressureMpa, 0.5), 1, 4);
    const target = sat ? clamp(0.38 * Math.sqrt(Math.max(q, 0)) * Math.sqrt(pScale), 0, 0.75) : 0;
    v.coreAvgVoid = dt === undefined ? target : v.coreAvgVoid + (target - v.coreAvgVoid) * Math.min(1, dt / 1.5);
    v.chimneyVoid = clamp(v.coreAvgVoid * 1.5, 0, 0.85);
    v.exitQuality = sat ? clamp(0.16 * q, 0, 0.3) : 0;
    v.coreFlowKgs = 250 + 600 * Math.sqrt(Math.max(q, 0)) * (sat ? 0.4 : 1) + 2200 * Math.sqrt(v.chimneyVoid);
  }

  private step(dt: number) {
    const s = this.s;
    const n = s.neutronics;
    const v = s.vessel;
    const st = s.steam;
    const tb = s.turbine;
    const cd = s.condenser;
    const fw = s.feedwater;
    s.timeS += dt;

    // Rods
    let anyMoving = false;
    for (let i = 0; i < s.rods.positions.length; i++) {
      const rate = s.rods.scramInProgress ? SCRAM_SPEED : ROD_SPEED;
      const p = s.rods.positions[i];
      const np = approach(p, s.rods.targets[i], rate, dt);
      if (np !== p) anyMoving = true;
      s.rods.positions[i] = np;
    }
    if (s.rods.scramInProgress && !anyMoving) s.rods.scramInProgress = false;

    // Reactivity & kinetics (prompt-jump, one delayed group)
    this.computeReactivity();
    // The prompt-jump form is only valid below prompt critical; the clamp is the mock's kinetics fault.
    const rho = Math.min(n.reactivity.total, BETA - 30);
    n.kineticsFault = n.reactivity.total > BETA - 30 ? "reactivity at prompt critical: prompt-jump model clamped" : null;
    const P = (BETA * this.h.precursor + SOURCE_PCM) / (BETA - rho);
    const pNew = Math.max(P, 1e-12);
    this.h.precursor += LAMBDA * (pNew - this.h.precursor) * dt;
    // Smoothed inverse period (d ln P / dt).
    const omega = Math.log(pNew / Math.max(this.h.prevPower, 1e-12)) / dt;
    this.h.omega += (omega - this.h.omega) * Math.min(1, dt / 1.5);
    n.periodS = Math.abs(this.h.omega) < 1e-4 ? null : 1 / this.h.omega;
    this.h.prevPower = pNew;
    n.powerFrac = pNew;
    this.h.decayFrac += ((0.065 * Math.min(pNew, 1.2) - this.h.decayFrac) / 4000) * dt;
    n.decayHeatMw = this.h.decayFrac * RATED_MWT;
    n.thermalPowerMw = pNew * RATED_MWT * (1 - 0.065) + n.decayHeatMw;
    n.fuelTempC += ((v.coolantTempC + 620 * pNew - n.fuelTempC) / 5) * dt;
    if (n.fuelTempC > FUEL_ENVELOPE_C) n.fuelEnvelopeExceeded = true; // latched until scenario load

    // Xenon / iodine (normalised to 100% equilibrium)
    n.iodineRel += LAMBDA_I * (Math.min(pNew, 1.2) - n.iodineRel) * dt;
    n.xenonRel +=
      LAMBDA_X *
      ((0.95 * n.iodineRel + 0.05 * pNew) * (1 + XE_BURN) - n.xenonRel * (1 + XE_BURN * pNew)) *
      dt;

    // MSIVs, IC valves
    st.msivPos = st.msivPos.map((p, i) => approach(p, st.msivCommandOpen[i] ? 1 : 0, 1 / 5, dt));
    for (const ic of s.ic) {
      ic.valvePos = approach(ic.valvePos, ic.commandOpen ? 1 : 0, 1 / 20, dt);
      // A dry pool has no heat sink.
      const sink = ic.poolEmpty ? 0 : 1;
      ic.heatMw = sink * ic.valvePos * IC_MW * clamp(v.pressureMpa / 7.2, 0, 1) * (v.coolantTempC > 110 ? 1 : 0);
      ic.poolTempC = Math.min(100, ic.poolTempC + (ic.heatMw / 2000) * dt - ((ic.poolTempC - 30) / 1e5) * dt);
      const pooling = ic.poolTempC >= 100;
      if (pooling) ic.poolLevelPct = Math.max(0, ic.poolLevelPct - (ic.heatMw / 5000) * dt);
      ic.poolEmpty = ic.poolLevelPct <= 0;
      ic.boiloffKgs = pooling ? (ic.heatMw * 1000) / H_FG_POOL : 0;
      ic.condensateReturnKgs = (ic.heatMw * 1000) / H_FG_IC;
    }

    // Pressure control: integrator on (p - setpoint) drives total valve demand.
    const msivFrac = Math.min(...st.msivPos);
    const err = v.pressureMpa - st.pressureSetpointMpa;
    this.h.pressInt = clamp(this.h.pressInt + 0.05 * err * dt, 0, 1.5);
    const demand = clamp(this.h.pressInt + 0.6 * err, 0, 1.5);
    // Turbine control valve: speed control off-line, load limit on-line.
    let tcvTarget = 0;
    if (!tb.tripped) {
      if (tb.breakerClosed) tcvTarget = Math.min(demand, tb.loadLimitPct / 100);
      else tcvTarget = clamp(0.01 + (tb.speedSetpointRpm - tb.speedRpm) * 0.00025 + (tb.speedRpm / RATED_RPM) * 0.04, 0, 0.1);
    }
    st.tcvPos = approach(st.tcvPos, tcvTarget, 0.2, dt);
    const vacuumOk = cd.pressureKpa < 30;
    const bypTarget = !vacuumOk
      ? 0
      : st.bypassAuto
        ? clamp(((demand - st.tcvPos) * STEAM_K_TCV) / STEAM_K_BYP, 0, 1)
        : st.bypassManualPos;
    st.bypassPos = approach(st.bypassPos, bypTarget, 0.25, dt);
    const pDrive = Math.max(v.pressureMpa - 0.1, 0);
    st.steamToTurbineKgs = st.tcvPos * STEAM_K_TCV * pDrive * msivFrac;
    st.steamToBypassKgs = st.bypassPos * STEAM_K_BYP * pDrive * msivFrac;
    const ventFlow = v.headVentOpen ? 0.5 * pDrive : 0;
    v.steamFlowKgs = st.steamToTurbineKgs + st.steamToBypassKgs + ventFlow;

    // Turbine-generator
    if (tb.breakerClosed) {
      tb.speedRpm = RATED_RPM;
      tb.slipHz = 0;
      tb.syncPhaseDeg = approach(tb.syncPhaseDeg, 0, 5, dt);
    } else {
      const steamOk = st.steamToTurbineKgs > 2;
      const target = tb.tripped || !steamOk ? 0 : tb.speedSetpointRpm;
      const rate = target > tb.speedRpm ? 16 : tb.tripped ? 12 : 16;
      tb.speedRpm = approach(tb.speedRpm, target, rate, dt);
      tb.slipHz = tb.speedRpm / 60 - 60; // two-pole: f = rpm / 60
      if (tb.speedRpm > OVERSPEED_RPM) tb.tripped = true;
      this.phase = (this.phase + 360 * tb.slipHz * dt) % 360;
      tb.syncPhaseDeg = ((this.phase + 540) % 360) - 180;
    }
    const vacEff = clamp(1 - (cd.pressureKpa - 5) / 60, 0, 1);
    tb.grossMwe = tb.breakerClosed ? st.steamToTurbineKgs * 0.66 * vacEff : 0;
    const houseLoad = 8 + fw.pumps.filter((p) => p.running).length * 3 + cd.cwPumps.filter(Boolean).length * 2;
    tb.netMwe = tb.grossMwe - houseLoad;
    if (tb.breakerClosed && st.steamToTurbineKgs < 5 && !tb.tripped) {
      // reverse power
      tb.breakerClosed = false;
    }

    // Condenser
    const nCw = cd.cwPumps.filter(Boolean).length;
    cd.heatLoadMw = (st.steamToTurbineKgs + st.steamToBypassKgs) * 2.0 - tb.grossMwe;
    cd.cwHeatRejectionMw = nCw > 0 ? Math.max(0, cd.heatLoadMw) : 0;
    const pTarget = cd.vacuumPumpOn
      ? nCw > 0
        ? 4 + cd.heatLoadMw / (nCw * 90)
        : 15 + cd.heatLoadMw / 5
      : nCw > 0 && cd.heatLoadMw > 1
        ? cd.pressureKpa
        : 101;
    cd.pressureKpa = clamp(cd.pressureKpa + ((pTarget - cd.pressureKpa) / (cd.vacuumPumpOn ? 120 : 400)) * dt, 2, 110);
    if (cd.heatLoadMw > 5 && nCw === 0) cd.pressureKpa = Math.min(110, cd.pressureKpa + 0.05 * cd.heatLoadMw * dt);

    // Feedwater & level control
    const nCond = fw.condensatePumps.filter(Boolean).length;
    const nFeed = fw.pumps.filter((p) => p.running).length;
    const levelErr = (fw.levelSetpointCm - v.levelNrCm) / 100; // m
    const headOk = nCond > 0 && cd.hotwellLevelPct > 5;
    const dpFactor = clamp(1 - 0.0375 * v.pressureMpa, 0, 1);
    const cap = nFeed * FEED_PUMP_CAP_KGS * dpFactor;
    let fcvTarget = fw.fcvManualPos;
    if (fw.levelControlMode === "singleElement") {
      this.h.levelInt = clamp(this.h.levelInt + 0.004 * levelErr * dt, 0, 1);
      fcvTarget = clamp(this.h.levelInt + 0.25 * levelErr, 0, 1);
    } else if (fw.levelControlMode === "threeElement") {
      this.h.levelInt = clamp(this.h.levelInt + 0.004 * levelErr * dt, -0.3, 0.3);
      const ff = cap > 0 ? v.steamFlowKgs / cap : 0;
      fcvTarget = clamp(ff + this.h.levelInt + 0.3 * levelErr, 0, 1);
    }
    fw.fcvPos = approach(fw.fcvPos, fcvTarget, 0.1, dt);
    const totalFeed = headOk ? fw.fcvPos * cap : 0;
    // Feed demanded of a hotwell too low to supply it (pump suction cut off).
    if (nCond > 0 && !headOk) cd.withdrawalDeficitKg += fw.fcvPos * cap * dt;
    const condOnly = headOk && nFeed === 0 && v.pressureMpa < 1.5 ? fw.fcvPos * nCond * 60 * (1 - v.pressureMpa / 1.5) : 0;
    v.feedFlowKgs = totalFeed + condOnly;
    for (const p of fw.pumps) {
      p.flowKgs = p.running && nFeed > 0 ? totalFeed / nFeed : 0;
      p.speedPct = approach(p.speedPct, pumpSpeedDemand(p.running, fw.fcvPos), PUMP_RAMP_PCT_S, dt);
    }
    // Extraction heat comes only from current turbine steam (zero after a trip); heater
    // storage then lags the feed outlet temperature toward what that heat can support.
    const extractionSteamKgs = fw.heatersInService ? Math.min(st.steamToTurbineKgs, EXTRACTION_STEAM_CAP_KGS) : 0;
    fw.extractionHeatMw = (extractionSteamKgs * EXTRACTION_KJ_PER_KG) / 1000;
    const feedRiseC = fw.extractionHeatMw > 0 ? (fw.extractionHeatMw * 1000) / (Math.max(v.feedFlowKgs, 1) * FEED_CP) : 0;
    const feedTempTarget = Math.min(COLD_FEED_C + feedRiseC, FEED_TEMP_MAX_C);
    fw.feedTempC += (feedTempTarget - fw.feedTempC) * Math.min(1, dt / HEATER_TAU_S);

    // Hotwell
    const hotwellRaw =
      cd.hotwellLevelPct +
      ((st.steamToTurbineKgs + st.steamToBypassKgs + fw.rwcuRejectKgs - v.feedFlowKgs) / HOTWELL_KG_PER_PCT) * dt +
      (cd.hotwellLevelPct < 50 ? 0.05 * dt : 0);
    cd.overflowKg += Math.max(0, hotwellRaw - 100) * HOTWELL_KG_PER_PCT;
    cd.hotwellLevelPct = clamp(hotwellRaw, 0, 100);
    // Condensate settles toward saturation at condenser pressure.
    cd.hotwellTempC = approach(cd.hotwellTempC, tsat(cd.pressureKpa / 1000), 0.05, dt);

    // Vessel energy balance
    const qIc = s.ic.reduce((a, ic) => a + ic.heatMw, 0);
    const hSteam = 2770;
    const hFeed = 4.19 * fw.feedTempC;
    const hLiquid = 4.19 * v.coolantTempC * 1.1;
    // Free-surface boil-off is set by the vapour partial pressure; trapped gas raises only the total.
    const tSat = tsat(v.pressureMpa - v.headGasPressureMpa);
    const boiling = v.coolantTempC >= tSat - 0.5 && v.coolantTempC > 100;
    const qSteam = boiling ? (v.steamFlowKgs * (hSteam - hLiquid)) / 1000 : 0;
    const qFeed = (v.feedFlowKgs * (hLiquid - hFeed)) / 1000;
    const qLoss = 0.5 + (v.coolantTempC - 20) * 0.003;
    const capMJperK = 1100;
    const dT = ((n.thermalPowerMw - qSteam - qFeed - qIc - qLoss) / capMJperK) * dt;
    const prevT = v.coolantTempC;
    const coolantRaw = v.coolantTempC + dT;
    v.coolantTempC = clamp(coolantRaw, 15, COOLANT_MAX_C);
    if (v.headVentOpen && v.coolantTempC > 100 && v.pressureMpa < 0.2) v.coolantTempC = 100;
    v.heatupRateCph = v.heatupRateCph + (((v.coolantTempC - prevT) / dt) * 3600 - v.heatupRateCph) * Math.min(1, dt / 30);
    this.updateHeadSpace();
    // Inlet subcooling is measured against core boiling, i.e. total pressure after the head-space update.
    v.coreInletSubcoolingKjKg = Math.max(0, 4.19 * (tsat(v.pressureMpa) - (v.coolantTempC * 0.9 + fw.feedTempC * 0.1)));

    // Inventory: 8900 kg per metre of level, plus void swell
    const boilOff = boiling ? v.steamFlowKgs : 0;
    this.h.massLevelM += ((v.feedFlowKgs - boilOff - fw.rwcuRejectKgs) / VESSEL_KG_PER_M) * dt;
    v.inventoryShortfallKg += Math.max(0, -6 - this.h.massLevelM) * VESSEL_KG_PER_M;
    this.h.massLevelM = clamp(this.h.massLevelM, -6, 16);
    this.updateVoids(dt);
    v.levelAboveTafM = this.h.massLevelM + (v.chimneyVoid - 0.45) * 1.2 ;
    v.levelNrCm = (v.levelAboveTafM - NWL_ABOVE_TAF_M) * 100;

    // Instruments
    const noise = () => 1 + (this.random() - 0.5) * 0.04;
    n.srmCps = Math.min(1e6, 5e10 * n.powerFrac * noise() + 0.5);
    const fs = Math.pow(10, -6 + 0.5 * (n.irmRange - 1));
    n.irmReadingPct = clamp((n.powerFrac / fs) * 100 * noise(), 0, 125);
    n.aprmPct = (n.thermalPowerMw / RATED_MWT) * 100 * (1 + (this.random() - 0.5) * 0.004);

    // Leaving the mock's modeled range pauses the sim, as the engine does on a model-domain error.
    const finite = [v.pressureMpa, v.coolantTempC, n.powerFrac, n.fuelTempC, this.h.massLevelM].every(Number.isFinite);
    s.modelError = !finite
      ? "non-finite state in mock step"
      : coolantRaw > COOLANT_MAX_C
        ? `coolant temperature above mock model range (${COOLANT_MAX_C} C)`
        : null;
    if (s.modelError) s.paused = true;

    this.autoTrips();
  }

  private autoTrips() {
    const s = this.s;
    const n = s.neutronics;
    const v = s.vessel;
    // Equipment protection is independent of the reactor scram state.
    if (s.condenser.pressureKpa > 30 && !s.turbine.tripped) {
      s.turbine.tripped = true;
      s.turbine.breakerClosed = false;
    }
    // Reactor scram signals: only the act of latching a new scram is gated.
    if (s.scrammed) return;
    // The mode switch in SHUTDOWN is a persistent scram signal while selected,
    // except for the deliberately unlatched cold-shutdown start (rods all in).
    if (s.modeSwitch === "shutdown" && s.rods.positions.some((p) => p > 0.5)) this.scram("MODE SWITCH IN SHUTDOWN");
    if (s.modeSwitch === "run" && n.aprmPct > 118) this.scram("APRM HIGH FLUX");
    if (s.modeSwitch !== "run" && n.aprmPct > 15) this.scram("APRM HIGH FLUX (SETDOWN)");
    if (s.modeSwitch !== "run" && n.irmReadingPct > 120) this.scram("IRM HIGH-HIGH");
    if (n.periodS !== null && n.periodS > 0 && n.periodS < 10 && n.powerFrac > 1e-5) this.scram("SHORT PERIOD");
    if (v.pressureMpa > 7.8) this.scram("REACTOR HIGH PRESSURE");
    if (v.levelNrCm < -150) this.scram("REACTOR LOW LEVEL (L3)");
    if (s.modeSwitch === "run" && Math.min(...s.steam.msivPos) < 0.9) this.scram("MSIV CLOSURE");
    if (s.modeSwitch === "run" && s.turbine.tripped && n.aprmPct > 40) this.scram("TURBINE TRIP");
  }

  private updateAlarms() {
    const s = this.s;
    const n = s.neutronics;
    const v = s.vessel;
    const cond: [string, string, Severity, boolean][] = [
      ["scram", `REACTOR SCRAM${s.scramReason ? ` — ${s.scramReason}` : ""}`, "trip", s.scrammed],
      ["rodsMoving", "SCRAM IN PROGRESS", "alarm", s.rods.scramInProgress],
      ["aprmHi", "APRM UPSCALE", "alarm", n.aprmPct > 105],
      ["period", "SHORT PERIOD", "warning", n.periodS !== null && n.periodS > 0 && n.periodS < 30],
      ["irmHi", "IRM UPSCALE", "warning", s.modeSwitch !== "run" && n.irmReadingPct > 100],
      ["srmHi", "SRM UPSCALE", "info", n.srmCps > 1e5],
      ["pressHi", "RX PRESSURE HIGH", "alarm", v.pressureMpa > 7.5],
      ["levelHi", "RX LEVEL HIGH (L7)", "warning", v.levelNrCm > 60],
      ["levelLo", "RX LEVEL LOW (L4)", "warning", v.levelNrCm < -60],
      ["levelLoLo", "RX LEVEL LOW (L3)", "alarm", v.levelNrCm < -150],
      ["heatup", "HEATUP RATE > 55 C/H", "warning", Math.abs(v.heatupRateCph) > 55],
      ["msiv", "MSIV NOT FULL OPEN", "info", s.steam.msivPos.some((p) => p < 0.99) && v.pressureMpa > 1],
      ["turbTrip", "TURBINE TRIPPED", "info", s.turbine.tripped],
      ["vacuum", "CONDENSER LOW VACUUM", "warning", s.condenser.pressureKpa > 25],
      ["hotwell", "HOTWELL LEVEL LOW", "warning", s.condenser.hotwellLevelPct < 20],
      ["feedLow", "NO FEED PUMP RUNNING", "info", s.feedwater.pumps.every((p) => !p.running)],
      ["xenon", "XENON TRANSIENT", "info", Math.abs(n.xenonRel - n.iodineRel) > 0.3],
      ["fuel", "FUEL TEMP HIGH", "alarm", n.fuelTempC > 1200],
      ["ic", "ISOLATION CONDENSER IN SERVICE", "info", s.ic.some((i) => i.valvePos > 0.05)],
      ["genOff", "GENERATOR BREAKER OPEN", "info", !s.turbine.breakerClosed],
      ["vent", "RPV HEAD VENT OPEN", "info", v.headVentOpen && v.pressureMpa > 0.11],
      ["modeShut", "MODE SWITCH IN SHUTDOWN", "info", s.modeSwitch === "shutdown"],
      ["model", "PHYSICS MODEL LIMIT - SIM HALTED", "alarm", s.modelError !== null],
    ];
    // Engine semantics: the visible list is the active conditions only. A cleared alarm drops
    // out whether or not it was acknowledged, so its clear is an event and a recurrence is a
    // fresh, unacknowledged onset; an acknowledgement lasts while the condition stays active.
    const prev = this.h.alarmState;
    const next: Hidden["alarmState"] = new Map();
    for (const [id, text, severity, active] of cond) {
      if (active) next.set(id, { id, text, severity, acked: prev.get(id)?.acked ?? false });
    }
    this.h.alarmState = next;
    s.alarms = [...next.values()].map((a) => ({ ...a }));
  }

  /** Head-space volume above the inventory, shrinking as the level rises. */
  private headSpaceM3() {
    const rise = ((this.h.massLevelM - NWL_ABOVE_TAF_M) * VESSEL_KG_PER_M) / WATER_RHO;
    return Math.max(HEAD_SPACE_MIN_M3, HEAD_SPACE_M3 - rise);
  }

  /** Total pressure = vapour (saturation at coolant temperature) + head-space gas (Dalton). */
  private updateHeadSpace() {
    const v = this.s.vessel;
    const pVap = psat(v.coolantTempC);
    const vol = this.headSpaceM3();
    if (v.headVentOpen) {
      // Vented: air exchanges with the atmosphere, so mass and pressure re-equilibrate.
      v.headGasPressureMpa = ventedGasMpa(v.coolantTempC);
      v.headGasKg = gasKg(v.headGasPressureMpa, v.coolantTempC, vol);
      v.pressureMpa = Math.min(Math.max(pVap, P_ATM_MPA), 0.12);
    } else {
      // Sealed: trapped mass is fixed; its partial pressure follows temperature and volume.
      v.headGasPressureMpa = gasPressureMpa(v.headGasKg, v.coolantTempC, vol);
      v.pressureMpa = pVap + v.headGasPressureMpa;
    }
  }

  private derive() {
    this.updateAlarms();
  }
}
