// Trend channel registry (plan C4): the single definition of every trended quantity.
import type { Snapshot } from "../types";

/** Rated thermal power used to express thermal power in % (870 MWt). */
export const RATED_THERMAL_MW = 870;

export type ChannelGroup = "Reactor" | "Vessel" | "Steam/Feed" | "Turbine" | "Condenser/IC";

export interface Channel {
  id: string;
  label: string;
  unit: string;
  digits: number;
  color: string;
  group: ChannelGroup;
  /** null = no value at this instant (drawn as a gap). */
  get: (s: Snapshot) => number | null;
}

export const CHANNEL_GROUPS: readonly ChannelGroup[] = ["Reactor", "Vessel", "Steam/Feed", "Turbine", "Condenser/IC"];

// The first seven colours are the mini trend panel's palette; reused so the full page matches it.
const COLOR = {
  green: "#3ee07a",
  amber: "#ffb43a",
  cyan: "#4cc9f0",
  red: "#ff5a4e",
  violet: "#b48cff",
  white: "#dfe6e2",
  pink: "#ff7ac6",
  lime: "#c8f060",
};

export const CHANNELS: readonly Channel[] = [
  { id: "thermalPowerPct", label: "Thermal power", unit: "%", digits: 1, color: COLOR.green, group: "Reactor", get: (s) => (s.neutronics.thermalPowerMw / RATED_THERMAL_MW) * 100 },
  { id: "aprmPct", label: "APRM", unit: "%", digits: 1, color: COLOR.amber, group: "Reactor", get: (s) => s.neutronics.aprmPct },
  { id: "periodS", label: "Period", unit: "s", digits: 1, color: COLOR.violet, group: "Reactor", get: (s) => s.neutronics.periodS },
  { id: "reactivityPcm", label: "Total reactivity", unit: "pcm", digits: 0, color: COLOR.cyan, group: "Reactor", get: (s) => s.neutronics.reactivity.total },
  { id: "fuelTempC", label: "Fuel temp", unit: "°C", digits: 0, color: COLOR.red, group: "Reactor", get: (s) => s.neutronics.fuelTempC },

  { id: "pressureMpa", label: "Rx pressure", unit: "MPa", digits: 2, color: COLOR.amber, group: "Vessel", get: (s) => s.vessel.pressureMpa },
  { id: "levelNrCm", label: "Level NR", unit: "cm", digits: 0, color: COLOR.cyan, group: "Vessel", get: (s) => s.vessel.levelNrCm },
  { id: "coreFlowKgs", label: "Core flow", unit: "kg/s", digits: 0, color: COLOR.violet, group: "Vessel", get: (s) => s.vessel.coreFlowKgs },
  // Saturation temperature at vessel pressure is the steam dome temperature.
  { id: "domeTempC", label: "Dome temp", unit: "°C", digits: 1, color: COLOR.pink, group: "Vessel", get: (s) => s.vessel.coolantTempC },

  { id: "steamFlowKgs", label: "Steam flow", unit: "kg/s", digits: 0, color: COLOR.red, group: "Steam/Feed", get: (s) => s.vessel.steamFlowKgs },
  { id: "feedFlowKgs", label: "Feed flow", unit: "kg/s", digits: 0, color: COLOR.cyan, group: "Steam/Feed", get: (s) => s.vessel.feedFlowKgs },
  { id: "feedTempC", label: "Feed temp", unit: "°C", digits: 0, color: COLOR.amber, group: "Steam/Feed", get: (s) => s.feedwater.feedTempC },

  { id: "turbineSpeedRpm", label: "Turbine speed", unit: "rpm", digits: 0, color: COLOR.lime, group: "Turbine", get: (s) => s.turbine.speedRpm },
  { id: "grossMwe", label: "Generator gross", unit: "MWe", digits: 0, color: COLOR.white, group: "Turbine", get: (s) => s.turbine.grossMwe },

  { id: "condenserKpa", label: "Condenser pressure", unit: "kPa", digits: 1, color: COLOR.amber, group: "Condenser/IC", get: (s) => s.condenser.pressureKpa },
  { id: "icDutyMw", label: "IC duty", unit: "MW", digits: 1, color: COLOR.cyan, group: "Condenser/IC", get: (s) => s.ic.reduce((sum, train) => sum + train.heatMw, 0) },
];

const BY_ID = new Map(CHANNELS.map((c) => [c.id, c]));

export function channelById(id: string): Channel | undefined {
  return BY_ID.get(id);
}

/** Non-finite readings are treated as missing so they never poison min/max envelopes. */
export function readChannel(c: Channel, s: Snapshot): number | null {
  const v = c.get(s);
  return v === null || !Number.isFinite(v) ? null : v;
}

/** The mini trend panel: six charts, each a list of registry ids (titles as on the panel). */
export const MINI_CHARTS: readonly { title: string; unit: string; digits: number; channelIds: readonly string[] }[] = [
  { title: "THERMAL POWER", unit: "%", digits: 1, channelIds: ["thermalPowerPct"] },
  { title: "RX PRESSURE", unit: "MPa", digits: 2, channelIds: ["pressureMpa"] },
  { title: "LEVEL NR", unit: "cm", digits: 0, channelIds: ["levelNrCm"] },
  { title: "CORE FLOW", unit: "kg/s", digits: 0, channelIds: ["coreFlowKgs"] },
  { title: "STEAM / FEED", unit: "kg/s", digits: 0, channelIds: ["steamFlowKgs", "feedFlowKgs"] },
  { title: "GENERATOR", unit: "MWe", digits: 0, channelIds: ["grossMwe"] },
];
