// Test helpers shared by the logic tests (not a test file itself).

/** Minimal Snapshot-shaped object; only fields the logic reads are meaningful. */
export function fakeSnapshot(over = {}) {
  const base = {
    scenario: "hotStandby",
    runId: 1,
    rev: 1,
    annHead: 0,
    annTail: [],
    timeS: 0,
    speed: 1,
    effectiveSpeed: 1,
    clockBacklogS: 0,
    paused: false,
    modeSwitch: "run",
    scrammed: false,
    scramReason: null,
    modelError: null,
    neutronics: {
      powerFrac: 1, thermalPowerMw: 870, decayHeatMw: 50, periodS: null,
      reactivity: { total: 0, rods: 0, doppler: 0, void: 0, moderator: 0, xenon: 0 },
      kEff: 1, srmCps: 1e6, irmRange: 10, irmReadingPct: 100, aprmPct: 100,
      xenonRel: 1, iodineRel: 1, fuelTempC: 900, fuelEnvelopeExceeded: false, kineticsFault: null,
    },
    rods: {
      positions: [48, 48, 48], targets: [48, 48, 48],
      layout: [[0, 0], [1, 0], [2, 1]], gridSize: 9, selected: null, scramInProgress: false,
    },
    vessel: {
      pressureMpa: 7.07, coolantTempC: 287, levelAboveTafM: 8, levelNrCm: 0, coreFlowKgs: 4000,
      coreInletSubcoolingKjKg: 0, coreAvgVoid: 0.4, chimneyVoid: 0.6, exitQuality: 0.1,
      steamFlowKgs: 470, feedFlowKgs: 468, headVentOpen: false, heatupRateCph: 0,
      inventoryShortfallKg: 0, headGasKg: 0, headGasPressureMpa: 0,
    },
    steam: {
      msivPos: [1, 1], msivCommandOpen: [true, true], tcvPos: 0.8, bypassPos: 0, bypassAuto: true,
      bypassManualPos: 0, steamToTurbineKgs: 470, steamToBypassKgs: 0, pressureSetpointMpa: 7.07,
    },
    turbine: {
      tripped: false, speedRpm: 1800, speedSetpointRpm: 1800, breakerClosed: true, grossMwe: 300,
      netMwe: 285, syncPhaseDeg: 0, slipHz: 0, loadLimitPct: 100,
    },
    condenser: {
      pressureKpa: 5, hotwellLevelPct: 50, cwPumps: [true, true], vacuumPumpOn: true, heatLoadMw: 560,
      cwHeatRejectionMw: 560, hotwellTempC: 33, overflowKg: 0, withdrawalDeficitKg: 0,
    },
    feedwater: {
      pumps: [{ running: true, flowKgs: 234, speedPct: 80 }, { running: true, flowKgs: 234, speedPct: 80 }],
      condensatePumps: [true, true], fcvPos: 0.5, fcvManualPos: 0.5, levelControlMode: "threeElement",
      levelSetpointCm: 0, feedTempC: 215, heatersInService: true, rwcuRejectKgs: 0, extractionHeatMw: 0,
    },
    ic: [
      { valvePos: 0, commandOpen: false, heatMw: 1, poolTempC: 30, poolLevelPct: 100, boiloffKgs: 0, condensateReturnKgs: 0, poolEmpty: false },
      { valvePos: 0, commandOpen: false, heatMw: 2, poolTempC: 30, poolLevelPct: 100, boiloffKgs: 0, condensateReturnKgs: 0, poolEmpty: false },
    ],
    alarms: [],
  };
  return { ...base, ...over };
}

/** A one-channel registry over plain `{ timeS, x }` objects, for synthetic histories. */
export const X_CHANNELS = [{ id: "x", label: "X", unit: "", digits: 0, color: "#fff", group: "Reactor", get: (s) => s.x }];
