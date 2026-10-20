//! The plant simulator: ties all systems together, runs protection logic,
//! raises alarms and applies operator commands.

use std::collections::{BTreeMap, HashSet};

use crate::api::{self, Command, ModeSwitch, ScenarioName, Severity, Snapshot};
use crate::bop::{Condenser, Feedwater, LevelControlMode, SteamSystem, Turbine};
use crate::ic::IcTrain;
use crate::params::*;
use crate::reactor::{Core, DecayHeat, Xenon, ROD_GRID};
use crate::steam::{self, sat_p, P_ATM};
use crate::vessel::{Vessel, VesselInputs};

/// Fixed physics time step, s.
pub const DT: f64 = 0.05;

#[derive(Clone, Debug)]
pub struct Sim {
    pub time: f64,
    pub paused: bool,
    pub speed: f64,
    pub mode: ModeSwitch,
    pub core: Core,
    pub vessel: Vessel,
    pub steam: SteamSystem,
    pub turbine: Turbine,
    pub cond: Condenser,
    pub fw: Feedwater,
    pub ic: Vec<IcTrain>,
    pub scrammed: bool,
    pub scram_reason: Option<String>,
    pub scram_time: f64,
    pub irm_range: u8,
    pub core_damage: bool,
    pub q_core: f64,
    acked: HashSet<String>,
    /// Transient operator messages: id -> (text, expiry time).
    events: BTreeMap<String, (String, f64)>,
    prev_msiv_open: bool,
    prev_gen_online: bool,
    accumulator: f64,
}

/// IRM full scale for a range (relative power).
pub fn irm_full_scale(range: u8) -> f64 {
    0.4 * 10f64.powf((range as f64 - 10.0) / 2.0)
}

impl Sim {
    pub fn new(scenario: ScenarioName) -> Self {
        match scenario {
            ScenarioName::ColdShutdown => Self::cold_shutdown(),
            ScenarioName::HotStandby => Self::hot_standby(),
            ScenarioName::FullPower => Self::full_power(),
        }
    }

    fn base(core: Core, vessel: Vessel, steam: SteamSystem, turbine: Turbine, cond: Condenser, fw: Feedwater, mode: ModeSwitch) -> Self {
        let mut s = Self {
            time: 0.0,
            paused: false,
            speed: 1.0,
            mode,
            core,
            vessel,
            steam,
            turbine,
            cond,
            fw,
            ic: (0..N_IC_TRAINS).map(|_| IcTrain::new()).collect(),
            scrammed: false,
            scram_reason: None,
            scram_time: -100.0,
            irm_range: 1,
            core_damage: false,
            q_core: 0.0,
            acked: HashSet::new(),
            events: BTreeMap::new(),
            prev_msiv_open: true,
            prev_gen_online: false,
            accumulator: 0.0,
        };
        s.prev_msiv_open = s.msivs_open();
        s.prev_gen_online = s.turbine.breaker_closed && !s.turbine.tripped;
        s.core.update_reactivity(s.vessel.alpha_core, s.vessel.eos.t);
        s
    }

    /// Shut down for 30 days, 40 degC, vessel vented, all rods in.
    pub fn cold_shutdown() -> Self {
        let t = 40.0;
        let mut core = Core::new(0.0, 0.0, t);
        core.decay = DecayHeat::after_shutdown(30.0 * 86400.0);
        core.xenon = Xenon { iodine: 0.0, xenon: 0.0 };
        let q = core.decay.power() * RATED_POWER_W;
        let vessel = Vessel::new(t, Z_NWL, q, 0.0, t, true);
        let mut s = Self::base(
            core,
            vessel,
            SteamSystem::new(true),
            Turbine::new(false),
            Condenser::new(false),
            Feedwater::new(0, t, LevelControlMode::Manual, 0.0),
            ModeSwitch::Shutdown,
        );
        s.set_source_equilibrium();
        s.irm_range = 1;
        s
    }

    /// Hot and pressurised two days after shutdown, condenser on line.
    pub fn hot_standby() -> Self {
        let p = 6.9e6;
        let t = sat_p(p).t;
        let mut core = Core::new(0.0, 0.0, t);
        core.decay = DecayHeat::after_shutdown(2.0 * 86400.0);
        let mut xe = Xenon::equilibrium(1.0);
        for _ in 0..(48 * 60) {
            xe.step(0.0, 60.0);
        }
        core.xenon = xe;
        let q = core.decay.power() * RATED_POWER_W;
        let vessel = Vessel::new(t, Z_NWL, q, 0.0, 180.0, false);
        let mut fw = Feedwater::new(1, 150.0, LevelControlMode::SingleElement, 0.05);
        fw.cond_pumps = [true, false];
        let mut s = Self::base(
            core,
            vessel,
            SteamSystem::new(true),
            Turbine::new(false),
            Condenser::new(true),
            fw,
            ModeSwitch::Startup,
        );
        s.set_source_equilibrium();
        s
    }

    /// Rated power with equilibrium xenon, generator on line.
    pub fn full_power() -> Self {
        let t = sat_p(P_RATED).t;
        let core = Core::new(1.0, ROD_FULL_OUT, t + 264.0);
        let vessel = Vessel::new(t, Z_NWL, RATED_POWER_W, STEAM_FLOW_RATED, T_FW_RATED, false);
        let mut steam = SteamSystem::new(true);
        steam.tcv_pos = 0.85;
        let fw = Feedwater::new(2, T_FW_RATED, LevelControlMode::ThreeElement, 0.7);
        let mut s = Self::base(core, vessel, steam, Turbine::new(true), Condenser::new(true), fw, ModeSwitch::Run);
        s.irm_range = 10;
        s.cond.p_steam = 5.6e3;
        s.cond.t_sat = 35.0;
        s.fw.flow = STEAM_FLOW_RATED;

        // Fuel temperature at rated power.
        s.core.fuel_t = t + FUEL_RESISTANCE * RATED_POWER_W * (1.0 - DIRECT_COOLANT_HEAT);
        // Insert rods one notch at a time (highest worth first) until critical.
        let mut order: Vec<usize> = (0..N_RODS).collect();
        order.sort_by(|&a, &b| s.core.rods.weights[b].partial_cmp(&s.core.rods.weights[a]).unwrap());
        'outer: for _ in 0..24 {
            for &i in &order {
                s.core.update_reactivity(s.vessel.alpha_core, t);
                if s.core.rho_total <= 0.0 {
                    break 'outer;
                }
                let p = (s.core.rods.pos[i] - 2.0).max(0.0);
                s.core.rods.pos[i] = p;
                s.core.rods.target[i] = p;
            }
        }
        // Let everything settle with the rods fixed.
        s.settle(600.0);
        s.core.xenon = Xenon::equilibrium(s.core.kin.n);
        s.settle(120.0);
        s.time = 0.0;
        s.acked.clear();
        s.events.clear();
        s
    }

    fn set_source_equilibrium(&mut self) {
        self.core.update_reactivity(self.vessel.alpha_core, self.vessel.eos.t);
        let rho = self.core.rho_total.min(-1e-3);
        let n = SOURCE * crate::params::GEN_TIME / -rho;
        self.core.kin = crate::kinetics::Kinetics::equilibrium(n);
        // Precursors in equilibrium with the source-driven population.
    }

    /// Run the physics for `seconds` without operator input.
    pub fn settle(&mut self, seconds: f64) {
        let steps = (seconds / DT).round() as usize;
        for _ in 0..steps {
            self.step(DT);
        }
    }

    /// Advance by wall-clock `real_dt` seconds scaled by the sim speed.
    pub fn advance(&mut self, real_dt: f64) {
        if self.paused {
            return;
        }
        self.accumulator += real_dt * self.speed;
        let mut n = 0;
        while self.accumulator >= DT && n < 4000 {
            self.step(DT);
            self.accumulator -= DT;
            n += 1;
        }
        if n >= 4000 {
            self.accumulator = 0.0;
        }
        // Forget acknowledgements of alarms that have cleared, so they flash again if they return.
        let active: HashSet<String> = self.active_alarms().into_iter().map(|a| a.0).collect();
        self.acked.retain(|id| active.contains(id));
    }

    pub fn msivs_open(&self) -> bool {
        self.steam.msiv_pos.iter().sum::<f64>() / N_STEAM_LINES as f64 > 0.9
    }

    fn event(&mut self, id: &str, text: impl Into<String>) {
        self.events.insert(id.to_string(), (text.into(), self.time + 30.0));
        self.acked.remove(id);
    }

    pub fn scram(&mut self, reason: &str) {
        if !self.scrammed {
            self.scram_reason = Some(reason.to_string());
            self.scram_time = self.time;
        }
        self.scrammed = true;
        self.core.rods.scramming = true;
    }

    fn irm_reading(&self) -> f64 {
        self.core.kin.n / irm_full_scale(self.irm_range) * 100.0
    }

    fn level_cm(&self) -> f64 {
        self.vessel.level_nr_cm()
    }

    /// Active scram signals (excluding manual and mode switch).
    fn scram_signals(&self) -> Vec<&'static str> {
        let mut v = Vec::new();
        let n = self.core.kin.n;
        match self.mode {
            ModeSwitch::Run => {
                if n > APRM_HIGH_RUN {
                    v.push("APRM high flux");
                }
                if self.steam.msiv_pos.iter().sum::<f64>() / (N_STEAM_LINES as f64) < 0.85 {
                    v.push("MSIV closure");
                }
            }
            _ => {
                if n > APRM_HIGH_SETDOWN {
                    v.push("APRM high flux (setdown)");
                }
                if self.irm_reading() > IRM_HIGH_HIGH {
                    v.push("IRM high-high");
                }
            }
        }
        if self.vessel.eos.p > SCRAM_HIGH_PRESSURE {
            v.push("Reactor high pressure");
        }
        if self.level_cm() < L3_CM {
            v.push("Reactor low water level (L3)");
        }
        v
    }

    fn isolation_signals(&self) -> Vec<&'static str> {
        let mut v = Vec::new();
        if self.level_cm() < L2_CM {
            v.push("Low-low water level (L2)");
        }
        if self.mode == ModeSwitch::Run && self.steam.p_chest > 0.0 && self.steam.p_chest < MSIV_LOW_PRESSURE_RUN {
            v.push("Low steam line pressure in RUN");
        }
        if self.steam.total_flow() > 1.4 * STEAM_FLOW_RATED {
            v.push("High steam flow");
        }
        v
    }

    fn withdraw_block(&self) -> Option<&'static str> {
        if self.scrammed {
            return Some("scram not reset");
        }
        match self.mode {
            ModeSwitch::Shutdown => return Some("mode switch in SHUTDOWN"),
            ModeSwitch::Refuel => {
                let out = self.core.rods.pos.iter().filter(|&&p| p > 0.01).count();
                if out > 1 {
                    return Some("REFUEL mode: one rod withdrawn");
                }
            }
            ModeSwitch::Startup => {
                if self.irm_reading() > 108.0 {
                    return Some("IRM upscale");
                }
                if self.core.kin.n > 0.12 {
                    return Some("APRM setdown upscale");
                }
            }
            ModeSwitch::Run => {
                if self.core.kin.n > 1.08 {
                    return Some("APRM upscale");
                }
            }
        }
        None
    }

    /// One physics step.
    pub fn step(&mut self, dt: f64) {
        let p = self.vessel.eos.p;
        let t_cool = self.vessel.eos.t;

        // --- reactor core
        self.core.update_reactivity(self.vessel.alpha_core, t_cool);
        let cov = self.vessel.core_coverage();
        self.q_core = self.core.step(dt, t_cool, cov);

        // --- main steam, EHC, turbine
        let demand = self.steam.pressure_demand(p);
        let tcv_demand = self.turbine.tcv_demand(dt, demand);
        let byp_ok = self.cond.available_for_bypass();
        self.steam.step(dt, p, tcv_demand, byp_ok, self.turbine.tripped);
        self.steam.compute_flows(if self.vessel.eos.subcooled { P_ATM } else { p }, !self.turbine.tripped);
        let p_cond = self.cond.pressure();
        self.turbine.step(dt, self.steam.w_tcv, self.steam.p_chest, p_cond);

        // --- isolation condensers
        let steam_avail = !self.vessel.eos.subcooled;
        let q_ic: f64 = self.ic.iter_mut().map(|t| t.step(dt, t_cool, steam_avail)).sum();

        // --- feedwater (level setpoint setdown after a scram)
        self.fw.setdown_cm = if self.scrammed { -100.0 } else { 0.0 };
        let hotwell_ok = self.cond.hotwell_mass > 0.05 * HOTWELL_CAPACITY;
        let msg = self.fw.step(
            dt,
            p,
            self.level_cm(),
            self.steam.total_flow(),
            self.steam.w_tcv,
            hotwell_ok,
            self.cond.hotwell_t(),
        );
        if let Some(m) = msg {
            self.event("fw_trip", m);
        }

        // --- vessel
        let inputs = VesselInputs {
            q_core: self.q_core,
            q_ic,
            w_feed: self.fw.flow,
            h_feed: self.fw.h_feed(),
            w_steam: self.steam.total_flow(),
            w_liquid_out: self.fw.rwcu_reject,
        };
        self.vessel.step(dt, &inputs);

        // --- condenser
        let h_steam = sat_p(self.steam.p_chest.max(P_ATM)).h_g;
        let h_hw = steam::h_liquid(self.cond.hotwell_t());
        let w_in = self.steam.w_tcv + self.steam.w_bypass + self.fw.rwcu_reject;
        let fw_heating = self.fw.flow * (self.fw.h_feed() - h_hw).max(0.0);
        let q_cond = self.steam.w_tcv * h_steam - self.turbine.p_mech + self.steam.w_bypass * h_steam
            + self.fw.rwcu_reject * self.vessel.sat().h_f
            - w_in * h_hw
            - fw_heating;
        self.cond.step(dt, q_cond, w_in, self.fw.flow);

        self.time += dt;
        self.protection();
    }

    fn protection(&mut self) {
        // Scram signals.
        if let Some(sig) = self.scram_signals().first() {
            let sig = sig.to_string();
            self.scram(&sig);
        }

        // Isolation: close MSIVs (latched until the operator reopens them).
        let iso = self.isolation_signals();
        if !iso.is_empty() && self.steam.msiv_cmd.iter().any(|&c| c) {
            self.steam.msiv_cmd = [false; N_STEAM_LINES];
            self.event("isolation", format!("MSIV isolation: {}", iso[0]));
        }
        let msiv_open = self.msivs_open();
        let isolated_at_power = self.prev_msiv_open && !msiv_open && !self.vessel.eos.subcooled
            && self.steam.msiv_pos.iter().all(|&p| p < 0.1);
        if self.steam.msiv_pos.iter().all(|&p| p < 0.1) || msiv_open {
            self.prev_msiv_open = msiv_open;
        }

        // Isolation condenser auto-initiation.
        if self.vessel.eos.p > IC_HIGH_PRESSURE || self.level_cm() < L2_CM || isolated_at_power {
            if self.ic.iter().any(|t| !t.cmd_open) {
                for t in self.ic.iter_mut() {
                    t.cmd_open = true;
                }
                self.event("ic_init", "Isolation condensers auto-initiated");
            }
        }

        // Turbine trips.
        if !self.turbine.tripped {
            if self.level_cm() > L8_CM {
                self.turbine.trip("Reactor high water level (L8)");
                self.fw.pumps = [false; N_FEED_PUMPS];
                self.event("l8", "L8 high level: turbine and feed pumps tripped");
            } else if self.cond.pressure() > CONDENSER_TRIP_P {
                self.turbine.trip("Low condenser vacuum");
            }
        }
        // Select Rod Insert: on turbine trip or load rejection at high power, a
        // preselected group of rods is inserted hydraulically so the reactor can
        // ride through on the turbine bypass.
        let gen_online = self.turbine.breaker_closed && !self.turbine.tripped;
        if self.prev_gen_online && !gen_online && self.core.kin.n > 0.4 && !self.scrammed {
            self.core.rods.select_rod_insert();
            self.event("sri", "Select Rod Insert: turbine trip / load rejection");
        }
        self.prev_gen_online = gen_online;
        if self.turbine.tripped && self.turbine.breaker_closed {
            // Breaker opens on reverse power once the valves are shut.
            if self.turbine.p_mech < 1.0e6 {
                self.turbine.breaker_closed = false;
            }
        }

        // Core damage.
        if self.core.fuel_t > 1200.0 && !self.core_damage {
            self.core_damage = true;
            self.event("core_damage", "CORE DAMAGE: fuel temperature exceeded 1200 degC");
        }

        // Stop withdrawals if a rod block appears.
        if self.withdraw_block().is_some() {
            let rods = &mut self.core.rods;
            for (p, t) in rods.pos.iter().zip(rods.target.iter_mut()) {
                if *t > *p + 1e-6 {
                    *t = (p / 2.0).ceil() * 2.0;
                    *t = t.min(*p + 2.0).max(*p);
                }
            }
        }

        let now = self.time;
        self.events.retain(|_, (_, until)| *until > now);
    }

    // ------------------------------------------------------------------ commands

    pub fn apply(&mut self, cmd: Command) {
        match cmd {
            Command::SetPaused { paused } => self.paused = paused,
            Command::SetSpeed { speed } => self.speed = speed.clamp(0.1, 100.0),
            Command::LoadScenario { name } => {
                let (speed, paused) = (self.speed, self.paused);
                *self = Sim::new(name);
                self.speed = speed;
                self.paused = paused;
            }
            Command::ManualScram => self.scram("Manual scram"),
            Command::ResetScram => {
                if !self.scrammed {
                    return;
                }
                if self.time - self.scram_time < 10.0 {
                    self.event("scram_reset", "Scram reset blocked: wait 10 s after scram");
                } else if let Some(sig) = self.scram_signals().first() {
                    self.event("scram_reset", format!("Scram reset blocked: {sig}"));
                } else {
                    self.scrammed = false;
                    self.scram_reason = None;
                    self.core.rods.scramming = false;
                }
            }
            Command::SetModeSwitch { mode } => {
                if mode == ModeSwitch::Shutdown && self.mode != ModeSwitch::Shutdown {
                    self.mode = mode;
                    self.scram("Mode switch in SHUTDOWN");
                } else {
                    self.mode = mode;
                }
            }
            Command::SelectRod { index } => self.core.rods.selected = index.filter(|&i| i < N_RODS),
            Command::RodSetTarget { indices, target } => {
                let target = ((target / 2.0).round() * 2.0).clamp(0.0, ROD_FULL_OUT);
                let block = self.withdraw_block();
                for i in indices.into_iter().filter(|&i| i < N_RODS) {
                    let rods = &mut self.core.rods;
                    if target > rods.pos[i] + 1e-6 {
                        if let Some(b) = block {
                            self.event("rod_block", format!("Rod withdrawal blocked: {b}"));
                            continue;
                        }
                    }
                    let rods = &mut self.core.rods;
                    if !rods.scramming {
                        rods.target[i] = target;
                    }
                }
            }
            Command::RodStop => self.core.rods.stop(),
            Command::SetIrmRange { range } => self.irm_range = range.clamp(1, 10),
            Command::SetHeadVent { open } => self.vessel.head_vent_open = open,
            Command::SetMsiv { line, open } => {
                if line < N_STEAM_LINES {
                    if open {
                        if let Some(sig) = self.isolation_signals().first() {
                            self.event("msiv", format!("MSIV open blocked: {sig}"));
                            return;
                        }
                    }
                    self.steam.msiv_cmd[line] = open;
                }
            }
            Command::SetPressureSetpoint { mpa } => self.steam.p_setpoint = (mpa * 1e6).clamp(0.2e6, 7.4e6),
            Command::SetBypass { auto, pos } => {
                self.steam.bypass_auto = auto;
                self.steam.bypass_manual_pos = pos.clamp(0.0, 1.0);
            }
            Command::SetSpeedSetpoint { rpm } => self.turbine.speed_target = rpm.clamp(0.0, RATED_RPM * 1.02),
            Command::ResetTurbine => {
                if self.level_cm() > L8_CM {
                    self.event("turbine", "Turbine reset blocked: high water level");
                } else if self.cond.pressure() > CONDENSER_TRIP_P {
                    self.event("turbine", "Turbine reset blocked: low condenser vacuum");
                } else {
                    self.turbine.reset();
                }
            }
            Command::TripTurbine => self.turbine.trip("Manual turbine trip"),
            Command::CloseBreaker => {
                if let Err(e) = self.turbine.close_breaker() {
                    self.event("breaker", e);
                }
            }
            Command::OpenBreaker => self.turbine.breaker_closed = false,
            Command::SetLoadLimit { pct } => self.turbine.load_limit = (pct / 100.0).clamp(0.0, 1.1),
            Command::SetCwPump { index, on } => {
                if index < N_CW_PUMPS {
                    self.cond.cw_pumps[index] = on;
                }
            }
            Command::SetVacuumPump { on } => self.cond.vacuum_pump = on,
            Command::SetFeedPump { index, on } => {
                if index < N_FEED_PUMPS {
                    if on && !self.fw.cond_pumps.iter().any(|&b| b) {
                        self.event("fw", "Feed pump start blocked: no condensate pump running");
                        return;
                    }
                    self.fw.pumps[index] = on;
                }
            }
            Command::SetCondensatePump { index, on } => {
                if index < N_COND_PUMPS {
                    self.fw.cond_pumps[index] = on;
                }
            }
            Command::SetLevelControl { mode } => {
                if mode == LevelControlMode::Manual {
                    self.fw.fcv_manual = self.fw.fcv_pos;
                }
                self.fw.mode = mode;
            }
            Command::SetFcvManual { pos } => self.fw.fcv_manual = pos.clamp(0.0, 1.0),
            Command::SetLevelSetpoint { cm } => self.fw.level_sp_cm = cm.clamp(-100.0, 50.0),
            Command::SetHeaters { in_service } => self.fw.heaters_in_service = in_service,
            Command::SetRwcuReject { kgs } => self.fw.rwcu_reject = kgs.clamp(0.0, 15.0),
            Command::SetIcTrain { index, open } => {
                if index < self.ic.len() {
                    self.ic[index].cmd_open = open;
                }
            }
            Command::AckAlarms => {
                let ids: Vec<String> = self.active_alarms().into_iter().map(|a| a.0).collect();
                self.acked.extend(ids);
            }
        }
    }

    // ------------------------------------------------------------------ alarms & snapshot

    fn active_alarms(&self) -> Vec<(String, String, Severity)> {
        let mut v: Vec<(String, String, Severity)> = Vec::new();
        let mut add = |id: &str, text: String, sev: Severity| v.push((id.to_string(), text, sev));
        let n = self.core.kin.n;
        let lvl = self.level_cm();
        let p = self.vessel.eos.p;
        if self.scrammed {
            add("scram", format!("REACTOR SCRAM: {}", self.scram_reason.clone().unwrap_or_default()), Severity::Trip);
        }
        if self.core_damage {
            add("core_damage", "CORE DAMAGE".into(), Severity::Trip);
        }
        if p > VESSEL_DESIGN_PRESSURE {
            add("overpressure", "VESSEL ABOVE DESIGN PRESSURE".into(), Severity::Trip);
        }
        if n > 1.05 {
            add("aprm_hi", "APRM HIGH".into(), Severity::Alarm);
        }
        if let Some(b) = self.withdraw_block() {
            if self.mode != ModeSwitch::Shutdown && !self.scrammed {
                add("rod_block", format!("ROD WITHDRAWAL BLOCK: {b}"), Severity::Warning);
            }
        }
        let ip = self.core.kin.inv_period;
        if ip > 1.0 / 20.0 && n > 1e-7 {
            add("period", format!("SHORT PERIOD ({:.0} s)", 1.0 / ip), Severity::Alarm);
        }
        if self.mode != ModeSwitch::Run && self.irm_reading() > 108.0 {
            add("irm_up", "IRM UPSCALE".into(), Severity::Warning);
        }
        if lvl > L7_CM {
            add("lvl_hi", "REACTOR WATER LEVEL HIGH".into(), Severity::Alarm);
        }
        if lvl < L4_CM {
            add("lvl_lo", "REACTOR WATER LEVEL LOW".into(), Severity::Alarm);
        }
        if lvl < L2_CM {
            add("lvl_lolo", "REACTOR WATER LEVEL LOW-LOW (L2)".into(), Severity::Trip);
        }
        if self.vessel.core_coverage() < 0.999 {
            add("core_uncovered", "CORE UNCOVERED".into(), Severity::Trip);
        }
        if p > 7.35e6 {
            add("p_hi", "REACTOR PRESSURE HIGH".into(), Severity::Alarm);
        }
        if self.vessel.heatup_rate.abs() > 55.0 {
            let w = if self.vessel.heatup_rate > 0.0 { "HEATUP" } else { "COOLDOWN" };
            add("heatup", format!("{w} RATE > 55 degC/h"), Severity::Warning);
        }
        if self.vessel.head_vent_open && p > 0.2e6 {
            add("vent", "HEAD VENT OPEN ABOVE 0.2 MPa".into(), Severity::Warning);
        }
        if self.vessel.eos.solid {
            add("solid", "REACTOR VESSEL WATER SOLID".into(), Severity::Alarm);
        }
        if !self.msivs_open() && !self.vessel.eos.subcooled {
            add("msiv", "MAIN STEAM ISOLATED".into(), Severity::Alarm);
        }
        if self.ic.iter().any(|t| t.valve_pos > 0.05) {
            add("ic", "ISOLATION CONDENSER IN SERVICE".into(), Severity::Warning);
        }
        if self.ic.iter().any(|t| t.pool_level_pct() < 50.0) {
            add("ic_pool", "IC POOL LEVEL LOW".into(), Severity::Alarm);
        }
        if self.turbine.tripped && self.turbine.speed_rpm > 100.0 {
            add("turb_trip", format!("TURBINE TRIP: {}", self.turbine.trip_reason.clone().unwrap_or_default()), Severity::Alarm);
        }
        if self.cond.pressure() > 15.0e3 && !self.turbine.tripped {
            add("vac", "CONDENSER VACUUM LOW".into(), Severity::Alarm);
        }
        if !self.cond.available_for_bypass() && !self.vessel.eos.subcooled && p > 1.0e6 {
            add("byp_unavail", "TURBINE BYPASS UNAVAILABLE".into(), Severity::Warning);
        }
        if self.fw.t_feed < 150.0 && n > 0.3 {
            add("fw_temp", "FEEDWATER TEMPERATURE LOW".into(), Severity::Warning);
        }
        if self.cond.hotwell_mass < 0.3 * HOTWELL_CAPACITY {
            add("hotwell", "HOTWELL LEVEL LOW".into(), Severity::Warning);
        }
        if self.core.fuel_t > 1000.0 {
            add("fuel_hot", "FUEL TEMPERATURE HIGH".into(), Severity::Alarm);
        }
        for (id, (text, _)) in &self.events {
            add(id, text.clone(), Severity::Info);
        }
        v
    }

    pub fn snapshot(&self) -> Snapshot {
        let c = &self.core;
        let v = &self.vessel;
        let s = v.sat();
        let pcm = |x: f64| x * 1e5;
        let ip = c.kin.inv_period;
        let period = if ip.abs() < 1e-4 { None } else { Some(1.0 / ip) };
        let house = HOUSE_BASE
            + self.fw.running_pumps() as f64 * FW_PUMP_POWER
            + self.fw.cond_pumps.iter().filter(|&&b| b).count() as f64 * COND_PUMP_POWER
            + self.cond.cw_pumps.iter().filter(|&&b| b).count() as f64 * CW_PUMP_POWER;
        let gross = self.turbine.p_elec.max(0.0);
        Snapshot {
            time_s: self.time,
            speed: self.speed,
            paused: self.paused,
            mode_switch: self.mode,
            scrammed: self.scrammed,
            scram_reason: self.scram_reason.clone(),
            neutronics: api::Neutronics {
                power_frac: c.kin.n,
                thermal_power_mw: c.thermal_power() / 1e6,
                decay_heat_mw: c.decay.power() * RATED_POWER_W / 1e6,
                period_s: period,
                reactivity: api::Reactivity {
                    total: pcm(c.rho_total),
                    rods: pcm(c.rho_rods),
                    doppler: pcm(c.rho_doppler),
                    void: pcm(c.rho_void),
                    moderator: pcm(c.rho_mod),
                    xenon: pcm(c.rho_xenon),
                },
                k_eff: c.k_eff(),
                srm_cps: (c.kin.n * 2.0e10 + 3.0).min(1.0e6),
                irm_range: self.irm_range,
                irm_reading_pct: self.irm_reading().clamp(0.0, 125.0),
                aprm_pct: (c.kin.n * 100.0).clamp(0.0, 125.0),
                xenon_rel: c.xenon.xenon,
                iodine_rel: c.xenon.iodine,
                fuel_temp_c: c.fuel_t,
            },
            rods: api::Rods {
                positions: c.rods.pos.clone(),
                targets: c.rods.target.clone(),
                layout: c.rods.layout.clone(),
                grid_size: ROD_GRID,
                selected: c.rods.selected,
                scram_in_progress: c.rods.scramming,
            },
            vessel: api::Vessel {
                pressure_mpa: v.eos.p / 1e6,
                coolant_temp_c: v.eos.t,
                level_above_taf_m: v.level_above_taf(),
                level_nr_cm: v.level_nr_cm(),
                core_flow_kgs: v.core_flow,
                core_inlet_subcooling_kj_kg: ((s.h_f - v.h_in) / 1e3).max(0.0),
                core_avg_void: v.alpha_core,
                chimney_void: v.alpha_ch,
                exit_quality: v.core.x_exit,
                steam_flow_kgs: self.steam.total_flow() + v.vent_flow,
                feed_flow_kgs: self.fw.flow,
                head_vent_open: v.head_vent_open,
                heatup_rate_cph: v.heatup_rate,
            },
            steam: api::Steam {
                msiv_pos: self.steam.msiv_pos.to_vec(),
                msiv_command_open: self.steam.msiv_cmd.to_vec(),
                tcv_pos: self.steam.tcv_pos,
                bypass_pos: self.steam.bypass_pos,
                bypass_auto: self.steam.bypass_auto,
                bypass_manual_pos: self.steam.bypass_manual_pos,
                steam_to_turbine_kgs: self.steam.w_tcv,
                steam_to_bypass_kgs: self.steam.w_bypass,
                pressure_setpoint_mpa: self.steam.p_setpoint / 1e6,
            },
            turbine: api::Turbine {
                tripped: self.turbine.tripped,
                speed_rpm: self.turbine.speed_rpm,
                speed_setpoint_rpm: self.turbine.speed_target,
                breaker_closed: self.turbine.breaker_closed,
                gross_mwe: gross / 1e6,
                net_mwe: (gross - house) / 1e6,
                sync_phase_deg: self.turbine.phase_deg,
                slip_hz: self.turbine.slip_hz(),
                load_limit_pct: self.turbine.load_limit * 100.0,
            },
            condenser: api::Condenser {
                pressure_kpa: self.cond.pressure() / 1e3,
                hotwell_level_pct: self.cond.hotwell_mass / HOTWELL_CAPACITY * 100.0,
                cw_pumps: self.cond.cw_pumps.to_vec(),
                vacuum_pump_on: self.cond.vacuum_pump,
                heat_load_mw: self.cond.heat_load / 1e6,
            },
            feedwater: api::Feedwater {
                pumps: self
                    .fw
                    .pumps
                    .iter()
                    .map(|&r| api::FeedPump { running: r, flow_kgs: if r { self.fw.flow_per_pump() } else { 0.0 } })
                    .collect(),
                condensate_pumps: self.fw.cond_pumps.to_vec(),
                fcv_pos: self.fw.fcv_pos,
                fcv_manual_pos: self.fw.fcv_manual,
                level_control_mode: self.fw.mode,
                level_setpoint_cm: self.fw.level_sp_cm,
                feed_temp_c: self.fw.t_feed,
                heaters_in_service: self.fw.heaters_in_service,
                rwcu_reject_kgs: self.fw.rwcu_reject,
            },
            ic: self
                .ic
                .iter()
                .map(|t| api::IcTrain {
                    valve_pos: t.valve_pos,
                    command_open: t.cmd_open,
                    heat_mw: t.heat / 1e6,
                    pool_temp_c: t.pool_t,
                    pool_level_pct: t.pool_level_pct(),
                })
                .collect(),
            alarms: self
                .active_alarms()
                .into_iter()
                .map(|(id, text, severity)| {
                    let acked = self.acked.contains(&id);
                    api::Alarm { id, text, severity, acked }
                })
                .collect(),
        }
    }
}
