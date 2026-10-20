//! Balance of plant: main steam, turbine control (EHC), turbine-generator,
//! condenser, condensate and feedwater.

use crate::params::*;
use crate::steam::{self, sat_p, P_ATM};
use serde::{Deserialize, Serialize};

// ------------------------------------------------------------------ main steam & EHC

#[derive(Clone, Debug)]
pub struct SteamSystem {
    pub msiv_pos: [f64; N_STEAM_LINES],
    pub msiv_cmd: [bool; N_STEAM_LINES],
    pub tcv_pos: f64,
    pub bypass_pos: f64,
    pub bypass_auto: bool,
    pub bypass_manual_pos: f64,
    pub p_setpoint: f64,
    pub w_tcv: f64,
    pub w_bypass: f64,
    pub p_chest: f64,
}

impl SteamSystem {
    pub fn new(msivs_open: bool) -> Self {
        let m = if msivs_open { 1.0 } else { 0.0 };
        Self {
            msiv_pos: [m; N_STEAM_LINES],
            msiv_cmd: [msivs_open; N_STEAM_LINES],
            tcv_pos: 0.0,
            bypass_pos: 0.0,
            bypass_auto: true,
            bypass_manual_pos: 0.0,
            p_setpoint: PRESSURE_SETPOINT_DEFAULT,
            w_tcv: 0.0,
            w_bypass: 0.0,
            p_chest: 0.0,
        }
    }

    fn msiv_open_fraction(&self) -> f64 {
        self.msiv_pos.iter().sum::<f64>() / N_STEAM_LINES as f64
    }

    /// Steam flows for the current valve positions at dome pressure `p`.
    pub fn compute_flows(&mut self, p: f64, tsv_open: bool) {
        let m = self.msiv_open_fraction();
        let a_tcv = if tsv_open { C_TCV * self.tcv_pos } else { 0.0 };
        let a_byp = C_BYPASS * self.bypass_pos;
        let a = a_tcv + a_byp;
        let p_eff = (p - P_ATM).max(0.0);
        if m < 1e-4 || a < 1e-12 || p_eff <= 0.0 {
            self.w_tcv = 0.0;
            self.w_bypass = 0.0;
            self.p_chest = if m < 1e-4 { 0.0 } else { p };
            return;
        }
        let k = STEAM_LINE_DP / (STEAM_FLOW_RATED * STEAM_FLOW_RATED) / (m * m);
        let w = (-1.0 + (1.0 + 4.0 * a * a * k * p_eff).sqrt()) / (2.0 * a * k);
        self.p_chest = p - k * w * w;
        self.w_tcv = a_tcv * (self.p_chest - P_ATM).max(0.0);
        self.w_bypass = a_byp * (self.p_chest - P_ATM).max(0.0);
    }

    pub fn total_flow(&self) -> f64 {
        self.w_tcv + self.w_bypass
    }

    /// Pressure regulator: total steam flow demand to hold the pressure setpoint, kg/s.
    pub fn pressure_demand(&self, p: f64) -> f64 {
        ((p - self.p_setpoint) / PRESSURE_REG_SPAN).max(0.0) * STEAM_FLOW_RATED
    }

    /// Move MSIVs, TCV and bypass valves toward their demands.
    pub fn step(&mut self, dt: f64, p: f64, tcv_flow_demand: f64, bypass_available: bool, tripped: bool) {
        for i in 0..N_STEAM_LINES {
            let target = if self.msiv_cmd[i] { 1.0 } else { 0.0 };
            let d = target - self.msiv_pos[i];
            self.msiv_pos[i] += d.clamp(-dt / MSIV_STROKE, dt / MSIV_STROKE);
        }
        let p_ref = if self.p_chest > 0.5e6 { self.p_chest } else { (p - 0.05e6).max(0.2e6) };

        // Turbine control valves.
        let tcv_target = if tripped { 0.0 } else { (tcv_flow_demand / (C_TCV * p_ref)).clamp(0.0, 1.0) };
        let d = tcv_target - self.tcv_pos;
        let (open_rate, close_rate) = if tripped { (0.0, 5.0) } else { (0.10, 0.5) };
        self.tcv_pos += d.clamp(-close_rate * dt, open_rate * dt);

        // Bypass valves.
        let target = if !bypass_available {
            0.0
        } else if self.bypass_auto {
            let demand = self.pressure_demand(p);
            let tcv_flow = if tripped { 0.0 } else { C_TCV * self.tcv_pos * p_ref };
            ((demand - tcv_flow).max(0.0) / (C_BYPASS * p_ref)).clamp(0.0, 1.0)
        } else {
            self.bypass_manual_pos.clamp(0.0, 1.0)
        };
        let d = target - self.bypass_pos;
        let rate = if self.bypass_auto { 2.0 } else { 0.2 };
        self.bypass_pos += d.clamp(-rate * dt, rate * dt);
    }
}

// ------------------------------------------------------------------ turbine-generator

#[derive(Clone, Debug)]
pub struct Turbine {
    pub speed_rpm: f64,
    /// Ramped speed reference.
    pub speed_ref: f64,
    /// Operator speed target.
    pub speed_target: f64,
    pub tripped: bool,
    pub breaker_closed: bool,
    pub phase_deg: f64,
    pub load_limit: f64,
    pub gov_integral: f64,
    pub p_mech: f64,
    pub p_elec: f64,
    pub trip_reason: Option<String>,
    reverse_power_timer: f64,
}

impl Turbine {
    pub fn new(online: bool) -> Self {
        Self {
            speed_rpm: if online { RATED_RPM } else { 0.0 },
            speed_ref: if online { RATED_RPM } else { 0.0 },
            speed_target: if online { RATED_RPM } else { 0.0 },
            tripped: !online,
            breaker_closed: online,
            phase_deg: 0.0,
            load_limit: 1.05,
            gov_integral: 0.0,
            p_mech: 0.0,
            p_elec: 0.0,
            trip_reason: if online { None } else { Some("Turbine not latched".into()) },
            reverse_power_timer: 0.0,
        }
    }

    pub fn trip(&mut self, reason: &str) {
        if !self.tripped {
            self.trip_reason = Some(reason.to_string());
        }
        self.tripped = true;
        self.speed_target = 0.0;
    }

    pub fn reset(&mut self) {
        self.tripped = false;
        self.trip_reason = None;
        self.speed_ref = self.speed_rpm;
        self.speed_target = self.speed_rpm.min(self.speed_target);
        self.gov_integral = 0.0;
    }

    /// Turbine control valve flow demand, kg/s, from the speed/load governor.
    pub fn tcv_demand(&mut self, dt: f64, pressure_demand: f64) -> f64 {
        if self.tripped {
            return 0.0;
        }
        let limit = self.load_limit * STEAM_FLOW_RATED;
        if self.breaker_closed {
            // Reactor leads: the pressure regulator positions the valves.
            return pressure_demand.min(limit);
        }
        let d = self.speed_target - self.speed_ref;
        self.speed_ref += d.clamp(-SPEED_RAMP * dt, SPEED_RAMP * dt);
        let err = (self.speed_ref - self.speed_rpm) / RATED_RPM;
        self.gov_integral = (self.gov_integral + 0.5 * err * dt).clamp(-0.05, 0.10);
        let ff = TG_LOSSES * (self.speed_ref / RATED_RPM).powi(2) / TURBINE_DH_RATED;
        let demand = ff + (10.0 * err + self.gov_integral) * STEAM_FLOW_RATED;
        // The turbine can only take steam the pressure regulator is already passing
        // (through the bypass), so rolling it never drags reactor pressure down.
        demand.clamp(0.0, limit.min(pressure_demand.max(0.0) + 0.01 * STEAM_FLOW_RATED))
    }

    pub fn grid_rpm() -> f64 {
        RATED_RPM
    }

    pub fn slip_hz(&self) -> f64 {
        self.speed_rpm / 30.0 - GRID_HZ
    }

    /// Advance shaft dynamics given turbine steam flow and pressures.
    pub fn step(&mut self, dt: f64, w_turbine: f64, p_chest: f64, p_cond: f64) {
        let pf = if p_chest > p_cond && p_cond > 0.0 {
            ((p_chest / p_cond).ln() / (7.0e6f64 / 5.6e3).ln()).clamp(0.0, 1.2)
        } else {
            0.0
        };
        self.p_mech = w_turbine * TURBINE_DH_RATED * pf;
        let w_frac = self.speed_rpm / RATED_RPM;
        let losses = TG_LOSSES * w_frac * w_frac + 0.5e6 * w_frac.min(1.0) * (self.speed_rpm > 1.0) as i32 as f64;

        if self.breaker_closed {
            self.speed_rpm = Self::grid_rpm();
            self.phase_deg = 0.0;
            self.p_elec = (self.p_mech - losses) * GENERATOR_EFF;
            if self.p_elec < 0.0 {
                self.reverse_power_timer += dt;
                if self.reverse_power_timer > 3.0 {
                    self.breaker_closed = false;
                }
            } else {
                self.reverse_power_timer = 0.0;
            }
        } else {
            self.p_elec = 0.0;
            let omega = self.speed_rpm * std::f64::consts::PI / 30.0;
            let e = 0.5 * TG_INERTIA * omega * omega + (self.p_mech - losses) * dt;
            let omega = (2.0 * e.max(0.0) / TG_INERTIA).sqrt();
            self.speed_rpm = omega * 30.0 / std::f64::consts::PI;
            if self.speed_rpm < 0.5 && self.p_mech <= 0.0 {
                self.speed_rpm = 0.0;
            }
            self.phase_deg += 360.0 * self.slip_hz() * dt;
            self.phase_deg = (self.phase_deg + 180.0).rem_euclid(360.0) - 180.0;
        }
        if self.speed_rpm > OVERSPEED_TRIP_RPM {
            self.trip("Turbine overspeed");
        }
    }

    /// Try to close the generator breaker. Returns an error message if the
    /// synchronising conditions are not met (which trips the generator).
    pub fn close_breaker(&mut self) -> Result<(), String> {
        if self.breaker_closed {
            return Ok(());
        }
        if self.tripped {
            return Err("Breaker close blocked: turbine tripped".into());
        }
        if (self.speed_rpm - RATED_RPM).abs() > 20.0 {
            return Err("Breaker close blocked: speed not near 1800 rpm".into());
        }
        if self.phase_deg.abs() > 15.0 || self.slip_hz().abs() > 0.25 {
            self.trip("Out-of-phase synchronisation");
            return Err("Out-of-phase synchronisation: generator lockout".into());
        }
        self.breaker_closed = true;
        Ok(())
    }
}

// ------------------------------------------------------------------ condenser

#[derive(Clone, Debug)]
pub struct Condenser {
    pub m_air: f64,
    pub p_steam: f64,
    pub cw_pumps: [bool; N_CW_PUMPS],
    pub vacuum_pump: bool,
    pub hotwell_mass: f64,
    pub heat_load: f64,
    pub t_sat: f64,
}

impl Condenser {
    pub fn new(vacuum: bool) -> Self {
        let p_air = if vacuum { 0.3e3 } else { P_ATM - 2.3e3 };
        Self {
            m_air: p_air * CONDENSER_VOLUME / (287.0 * 303.0),
            p_steam: 2.3e3,
            cw_pumps: [vacuum; N_CW_PUMPS],
            vacuum_pump: vacuum,
            hotwell_mass: 0.6 * HOTWELL_CAPACITY,
            heat_load: 0.0,
            t_sat: CW_INLET_C,
        }
    }

    pub fn p_air(&self) -> f64 {
        self.m_air * 287.0 * 303.0 / CONDENSER_VOLUME
    }

    pub fn pressure(&self) -> f64 {
        (self.p_air() + self.p_steam).min(1.2 * P_ATM)
    }

    pub fn hotwell_t(&self) -> f64 {
        self.t_sat.max(CW_INLET_C)
    }

    /// Advance: `q` heat to reject (W), `w_in` water returned to the hotwell,
    /// `w_out` condensate pumped out, kg/s.
    pub fn step(&mut self, dt: f64, q: f64, w_in: f64, w_out: f64) {
        self.heat_load = q.max(0.0);
        let n_cw = self.cw_pumps.iter().filter(|&&b| b).count() as f64;
        let m_cw = n_cw * CW_FLOW_PER_PUMP;
        let target = if m_cw > 0.0 {
            let mcp = m_cw * 4186.0;
            let eff = 1.0 - (-CONDENSER_UA / mcp).exp();
            let ts = CW_INLET_C + self.heat_load / (mcp * eff);
            steam::sat_t(ts).p
        } else if self.heat_load > 1.0e5 {
            1.2 * P_ATM
        } else {
            steam::sat_t(30.0).p
        };
        let tau = if m_cw > 0.0 { 15.0 } else { 120.0 };
        self.p_steam += (target - self.p_steam) * (dt / tau).min(1.0);
        self.t_sat = sat_p(self.p_steam.max(700.0)).t;

        let p = self.pressure();
        let leak = AIR_INLEAKAGE * ((P_ATM - p) / P_ATM).clamp(0.0, 1.0);
        let removal = if self.vacuum_pump { self.m_air / VACUUM_PUMP_TAU } else { 0.0 };
        self.m_air = (self.m_air + (leak - removal) * dt).max(0.0);
        // Air cannot exceed atmospheric partial pressure (relief to atmosphere).
        let max_air = (P_ATM - self.p_steam).max(0.0) * CONDENSER_VOLUME / (287.0 * 303.0);
        self.m_air = self.m_air.min(max_air.max(0.0));

        // Hotwell inventory with automatic makeup/reject to the condensate storage tank.
        self.hotwell_mass += (w_in - w_out) * dt;
        let frac = self.hotwell_mass / HOTWELL_CAPACITY;
        if frac < 0.4 {
            self.hotwell_mass += 30.0 * dt;
        } else if frac > 0.85 {
            self.hotwell_mass -= 30.0 * dt;
        }
        self.hotwell_mass = self.hotwell_mass.clamp(0.0, HOTWELL_CAPACITY);
    }

    pub fn available_for_bypass(&self) -> bool {
        self.pressure() < CONDENSER_BYPASS_BLOCK_P && self.cw_pumps.iter().any(|&b| b)
    }
}

// ------------------------------------------------------------------ feedwater

/// Equal-percentage valve characteristic (rangeability 30), 0 at closed, 1 at full open.
pub fn fcv_characteristic(pos: f64) -> f64 {
    const R: f64 = 30.0;
    if pos <= 0.0 {
        return 0.0;
    }
    ((R.powf(pos.min(1.0) - 1.0) - 1.0 / R) / (1.0 - 1.0 / R)).max(1e-6)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LevelControlMode {
    Manual,
    SingleElement,
    ThreeElement,
}

#[derive(Clone, Debug)]
pub struct Feedwater {
    pub pumps: [bool; N_FEED_PUMPS],
    pub cond_pumps: [bool; N_COND_PUMPS],
    pub fcv_pos: f64,
    pub fcv_manual: f64,
    pub mode: LevelControlMode,
    pub level_sp_cm: f64,
    /// Automatic level setpoint setdown after a scram, cm.
    pub setdown_cm: f64,
    pub heaters_in_service: bool,
    pub t_feed: f64,
    pub rwcu_reject: f64,
    pub flow: f64,
    integral: f64,
    low_suction_timer: f64,
}

impl Feedwater {
    pub fn new(pumps_running: usize, t_feed: f64, mode: LevelControlMode, fcv: f64) -> Self {
        let mut pumps = [false; N_FEED_PUMPS];
        for p in pumps.iter_mut().take(pumps_running) {
            *p = true;
        }
        let cond = pumps_running > 0;
        Self {
            pumps,
            cond_pumps: [cond, cond && pumps_running > 1],
            fcv_pos: fcv,
            fcv_manual: fcv,
            mode,
            level_sp_cm: 0.0,
            setdown_cm: 0.0,
            heaters_in_service: true,
            t_feed,
            rwcu_reject: 0.0,
            flow: 0.0,
            integral: fcv,
            low_suction_timer: 0.0,
        }
    }

    pub fn running_pumps(&self) -> usize {
        self.pumps.iter().filter(|&&b| b).count()
    }

    pub fn flow_per_pump(&self) -> f64 {
        let n = self.running_pumps();
        if n == 0 {
            0.0
        } else {
            self.flow / n as f64
        }
    }

    fn suction_pressure(&self, hotwell_ok: bool) -> f64 {
        if hotwell_ok && self.cond_pumps.iter().any(|&b| b) {
            CONDENSATE_PRESSURE
        } else {
            0.0
        }
    }

    /// Solve the feed system hydraulics for flow into a vessel at `p_reactor`.
    fn hydraulics(&self, p_reactor: f64, p_suction: f64) -> f64 {
        let n = self.running_pumps() as f64;
        let pos = self.fcv_pos;
        if pos < 1e-4 || p_suction <= 0.0 {
            return 0.0;
        }
        let static_dp = 0.15e6;
        let f = |q: f64| -> f64 {
            let pump = if n > 0.0 {
                let qp = q / n;
                FW_PUMP_SHUTOFF_DP * (1.0 - (qp / FW_PUMP_RUNOUT).powi(2))
            } else {
                -(q / 150.0).powi(2) * 1.0e5 // condensate only, through idle pumps
            };
            let valve = (q / (C_FCV * fcv_characteristic(pos))).powi(2);
            p_suction + pump - p_reactor - static_dp - valve
        };
        if f(0.0) <= 0.0 {
            return 0.0;
        }
        let (mut lo, mut hi) = (0.0, (n.max(1.0)) * FW_PUMP_RUNOUT);
        if f(hi) > 0.0 {
            return hi;
        }
        for _ in 0..60 {
            let mid = 0.5 * (lo + hi);
            if f(mid) > 0.0 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        0.5 * (lo + hi)
    }

    /// Level controller and hydraulics. Returns any pump trip message.
    pub fn step(
        &mut self,
        dt: f64,
        p_reactor: f64,
        level_cm: f64,
        steam_flow: f64,
        turbine_flow: f64,
        hotwell_ok: bool,
        hotwell_t: f64,
    ) -> Option<String> {
        let mut msg = None;
        let p_s = self.suction_pressure(hotwell_ok);
        if self.running_pumps() > 0 && p_s <= 0.0 {
            self.low_suction_timer += dt;
            if self.low_suction_timer > 3.0 {
                self.pumps = [false; N_FEED_PUMPS];
                msg = Some("Feed pumps tripped on low suction pressure".to_string());
            }
        } else {
            self.low_suction_timer = 0.0;
        }

        let err = level_cm - (self.level_sp_cm + self.setdown_cm);
        let target = match self.mode {
            LevelControlMode::Manual => {
                self.integral = self.fcv_pos;
                self.fcv_manual
            }
            LevelControlMode::SingleElement => {
                self.integral = (self.integral - 0.0002 * err * dt).clamp(0.0, 1.0);
                (self.integral - 0.004 * err).clamp(0.0, 1.0)
            }
            LevelControlMode::ThreeElement => {
                let demand = steam_flow - 1.2 * err;
                let flow_err = (demand - self.flow) / STEAM_FLOW_RATED;
                self.integral = (self.integral + 0.3 * flow_err * dt - 0.0002 * err * dt).clamp(0.0, 1.0);
                (self.integral + 0.2 * flow_err).clamp(0.0, 1.0)
            }
        };
        let d = target - self.fcv_pos;
        self.fcv_pos += d.clamp(-0.1 * dt, 0.1 * dt);
        if self.mode == LevelControlMode::Manual {
            self.fcv_manual = self.fcv_manual.clamp(0.0, 1.0);
        }

        self.flow = self.hydraulics(p_reactor, p_s);

        // Feedwater heating from turbine extraction steam.
        let load = (turbine_flow / STEAM_FLOW_RATED).clamp(0.0, 1.2);
        // Taking the high-pressure heaters out of service loses about a third of the heating.
        let full = (T_FW_RATED - 35.0) * load.powf(0.75);
        let heating = if self.heaters_in_service { full } else { 0.65 * full };
        let pump_heat = if self.running_pumps() > 0 { 2.0 } else { 0.0 };
        let target_t = hotwell_t + heating + pump_heat;
        self.t_feed += (target_t - self.t_feed) * (dt / 180.0).min(1.0);
        msg
    }

    pub fn h_feed(&self) -> f64 {
        steam::h_liquid(self.t_feed)
    }
}
