//! Data exchanged with the UI. Mirrors ui/src/sim/types.ts; keep both in sync.

use crate::bop::LevelControlMode;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModeSwitch {
    Shutdown,
    Refuel,
    Startup,
    Run,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warning,
    Alarm,
    Trip,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ScenarioName {
    ColdShutdown,
    HotStandby,
    FullPower,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reactivity {
    pub total: f64,
    pub rods: f64,
    pub doppler: f64,
    pub void: f64,
    pub moderator: f64,
    pub xenon: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Neutronics {
    pub power_frac: f64,
    pub thermal_power_mw: f64,
    pub decay_heat_mw: f64,
    pub period_s: Option<f64>,
    pub reactivity: Reactivity,
    pub k_eff: f64,
    pub srm_cps: f64,
    pub irm_range: u8,
    pub irm_reading_pct: f64,
    pub aprm_pct: f64,
    pub xenon_rel: f64,
    pub iodine_rel: f64,
    pub fuel_temp_c: f64,
    /// Lumped average fuel temperature has left the single-node model envelope
    /// (`FUEL_MODEL_ENVELOPE_C`). A model-validity flag, not a damage criterion.
    pub fuel_envelope_exceeded: bool,
    /// Point-kinetics fault of the last step; neutron power is frozen while set.
    pub kinetics_fault: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rods {
    pub positions: Vec<f64>,
    pub targets: Vec<f64>,
    pub layout: Vec<(usize, usize)>,
    pub grid_size: usize,
    pub selected: Option<usize>,
    pub scram_in_progress: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Vessel {
    pub pressure_mpa: f64,
    pub coolant_temp_c: f64,
    pub level_above_taf_m: f64,
    pub level_nr_cm: f64,
    pub core_flow_kgs: f64,
    pub core_inlet_subcooling_kj_kg: f64,
    pub core_avg_void: f64,
    pub chimney_void: f64,
    pub exit_quality: f64,
    pub steam_flow_kgs: f64,
    pub feed_flow_kgs: f64,
    pub head_vent_open: bool,
    pub heatup_rate_cph: f64,
    /// Cumulative outflow requested of the vessel that its inventory could not supply.
    pub inventory_shortfall_kg: f64,
    /// Noncondensable gas in the head space (follows the atmosphere while the
    /// head vent is open, trapped once it shuts).
    pub head_gas_kg: f64,
    /// Partial pressure of that gas; total pressure = vapour + gas.
    pub head_gas_pressure_mpa: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Steam {
    pub msiv_pos: Vec<f64>,
    pub msiv_command_open: Vec<bool>,
    pub tcv_pos: f64,
    pub bypass_pos: f64,
    pub bypass_auto: bool,
    pub bypass_manual_pos: f64,
    pub steam_to_turbine_kgs: f64,
    pub steam_to_bypass_kgs: f64,
    pub pressure_setpoint_mpa: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Turbine {
    pub tripped: bool,
    pub speed_rpm: f64,
    pub speed_setpoint_rpm: f64,
    pub breaker_closed: bool,
    pub gross_mwe: f64,
    pub net_mwe: f64,
    pub sync_phase_deg: f64,
    pub slip_hz: f64,
    pub load_limit_pct: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Condenser {
    pub pressure_kpa: f64,
    pub hotwell_level_pct: f64,
    pub cw_pumps: Vec<bool>,
    pub vacuum_pump_on: bool,
    pub heat_load_mw: f64,
    pub cw_heat_rejection_mw: f64,
    pub hotwell_temp_c: f64,
    pub overflow_kg: f64,
    pub withdrawal_deficit_kg: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedPump {
    pub running: bool,
    pub flow_kgs: f64,
    pub speed_pct: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Feedwater {
    pub pumps: Vec<FeedPump>,
    pub condensate_pumps: Vec<bool>,
    pub fcv_pos: f64,
    pub fcv_manual_pos: f64,
    pub level_control_mode: LevelControlMode,
    pub level_setpoint_cm: f64,
    pub feed_temp_c: f64,
    pub heaters_in_service: bool,
    pub rwcu_reject_kgs: f64,
    pub extraction_heat_mw: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IcTrain {
    pub valve_pos: f64,
    pub command_open: bool,
    pub heat_mw: f64,
    pub pool_temp_c: f64,
    pub pool_level_pct: f64,
    pub boiloff_kgs: f64,
    pub condensate_return_kgs: f64,
    pub pool_empty: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Alarm {
    pub id: String,
    pub text: String,
    pub severity: Severity,
    pub acked: bool,
}

/// Annunciator transition kind (plan C1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AnnKind {
    Onset,
    Clear,
    /// Same key, different text or severity.
    Changed,
}

/// Annunciator transition recorded by the engine. `key` is `cond:<id>` for
/// condition alarms and `evt:<id>` for transient operator events, so the two
/// never collide even where they share a legacy alarm id.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnnEvent {
    pub seq: u64,
    pub time_s: f64,
    pub kind: AnnKind,
    pub key: String,
    pub text: String,
    pub severity: Severity,
}

/// Annunciator events after a given sequence number of one run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnnEventsSince {
    pub run_id: u64,
    pub events: Vec<AnnEvent>,
    /// The engine ring no longer holds every requested event.
    pub truncated: bool,
}

/// The requested run is no longer the current one (scenario load or restore
/// happened); the caller discards its pull and waits for the new run's snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunChanged {
    pub current_run_id: u64,
}

impl std::fmt::Display for RunChanged {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "run changed (current run {})", self.current_run_id)
    }
}

impl std::error::Error for RunChanged {}

/// Headline plant state for a save slot listing (plan C2 `runSummary`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunSummary {
    pub mode: ModeSwitch,
    pub power_pct: f64,
    pub pressure_mpa: f64,
    pub scrammed: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    /// Origin scenario of the current run.
    pub scenario: ScenarioName,
    /// Identity of the current run; changes on scenario load and state restore.
    pub run_id: u64,
    /// Engine revision; increments on every command and step outcome.
    pub rev: u64,
    /// Latest annunciator event sequence number (0 = none yet).
    pub ann_head: u64,
    /// The most recent annunciator events (at most `ANN_TAIL_LEN`), oldest first.
    pub ann_tail: Vec<AnnEvent>,
    pub time_s: f64,
    /// Requested sim-speed multiplier (simulated s per wall-clock s).
    pub speed: f64,
    /// Achieved simulated s per wall-clock s, smoothed; 0 while paused. The
    /// engine reports the nominal value; the host overwrites it with the
    /// measured rate.
    pub effective_speed: f64,
    /// Simulated seconds owed to the clock but not yet stepped: the engine
    /// accumulator plus (set by the host) its unadvanced wall time x speed.
    pub clock_backlog_s: f64,
    pub paused: bool,
    pub mode_switch: ModeSwitch,
    pub scrammed: bool,
    pub scram_reason: Option<String>,
    /// Set when the last step left the physics model's supported domain; the
    /// simulator is paused on it and the state is the last accepted one.
    pub model_error: Option<String>,
    pub neutronics: Neutronics,
    pub rods: Rods,
    pub vessel: Vessel,
    pub steam: Steam,
    pub turbine: Turbine,
    pub condenser: Condenser,
    pub feedwater: Feedwater,
    pub ic: Vec<IcTrain>,
    pub alarms: Vec<Alarm>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Command {
    SetPaused { paused: bool },
    SetSpeed { speed: f64 },
    LoadScenario { name: ScenarioName },
    ManualScram,
    ResetScram,
    SetModeSwitch { mode: ModeSwitch },
    SelectRod { index: Option<usize> },
    RodSetTarget { indices: Vec<usize>, target: f64 },
    RodStop,
    SetIrmRange { range: u8 },
    SetHeadVent { open: bool },
    SetMsiv { line: usize, open: bool },
    SetPressureSetpoint { mpa: f64 },
    SetBypass { auto: bool, pos: f64 },
    SetSpeedSetpoint { rpm: f64 },
    ResetTurbine,
    TripTurbine,
    CloseBreaker,
    OpenBreaker,
    SetLoadLimit { pct: f64 },
    SetCwPump { index: usize, on: bool },
    SetVacuumPump { on: bool },
    SetFeedPump { index: usize, on: bool },
    SetCondensatePump { index: usize, on: bool },
    SetLevelControl { mode: LevelControlMode },
    SetFcvManual { pos: f64 },
    SetLevelSetpoint { cm: f64 },
    SetHeaters { in_service: bool },
    SetRwcuReject { kgs: f64 },
    SetIcTrain { index: usize, open: bool },
    AckAlarms,
}
