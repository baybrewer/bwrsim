//! Balance of plant: main steam, turbine control (EHC), turbine-generator,
//! condenser, condensate and feedwater.

use crate::params::*;
use crate::steam::{self, sat_p, sat_t, P_ATM};
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
    /// Specific enthalpy of the steam delivered to the turbine and bypass, J/kg.
    /// The steam line and valve throttling are isenthalpic: this is the vessel
    /// outlet enthalpy, not the saturated-vapour enthalpy at chest pressure.
    pub h_steam: f64,
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
            h_steam: sat_p(P_ATM).h_g,
        }
    }

    fn msiv_open_fraction(&self) -> f64 {
        self.msiv_pos.iter().sum::<f64>() / N_STEAM_LINES as f64
    }

    /// Steam flows for the current valve positions at dome pressure `p`, taking the
    /// vessel outlet as saturated vapour at `p`.
    pub fn compute_flows(&mut self, p: f64, tsv_open: bool) {
        self.compute_flows_from(p, sat_p(p.max(P_ATM)).h_g, tsv_open);
    }

    /// Steam flows for the current valve positions at dome pressure `p` with vessel
    /// outlet enthalpy `h_in`. No heat or work is exchanged in the line or the
    /// throttles, so the delivered enthalpy equals `h_in`.
    pub fn compute_flows_from(&mut self, p: f64, h_in: f64, tsv_open: bool) {
        self.h_steam = h_in;
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

    /// Move MSIVs, TCV and bypass valves toward their demands. A turbine trip
    /// raises the bypass fast-open demand; see `step_ext` for load rejection.
    pub fn step(&mut self, dt: f64, p: f64, tcv_flow_demand: f64, bypass_available: bool, tripped: bool) {
        self.step_ext(dt, p, tcv_flow_demand, bypass_available, tripped, tripped);
    }

    /// As `step`, with the bypass fast-open demand given explicitly: `fast_open`
    /// is the valid turbine-trip / generator load-rejection signal.
    pub fn step_ext(
        &mut self,
        dt: f64,
        p: f64,
        tcv_flow_demand: f64,
        bypass_available: bool,
        tripped: bool,
        fast_open: bool,
    ) {
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
        let (open_rate, close_rate) = if !self.bypass_auto {
            (BYPASS_MANUAL_RATE, BYPASS_MANUAL_RATE)
        } else if fast_open {
            (BYPASS_FAST_OPEN_RATE, BYPASS_REG_RATE)
        } else {
            (BYPASS_REG_RATE, BYPASS_REG_RATE)
        };
        self.bypass_pos += d.clamp(-close_rate * dt, open_rate * dt);
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

    /// Generator electrical frequency, Hz: f = rpm * poles / 120.
    pub fn electrical_hz(&self) -> f64 {
        self.speed_rpm * GENERATOR_POLES / 120.0
    }

    /// Slip frequency, Hz (generator minus grid).
    pub fn slip_hz(&self) -> f64 {
        self.electrical_hz() - GRID_HZ
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
            return Err(format!("Breaker close blocked: speed not near {RATED_RPM:.0} rpm"));
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

/// Lumped condenser / hotwell energy, J, on a liquid-enthalpy basis (zero at 0 degC):
/// hotwell water plus the condenser metal at the same temperature.
fn condenser_energy(t: f64, hotwell_mass: f64) -> f64 {
    hotwell_mass * steam::h_liquid(t) + CONDENSER_METAL_HEAT_CAP * t
}

/// Inverse of `condenser_energy` for the temperature, degC (bisection; the
/// energy is strictly increasing in temperature).
fn condenser_temperature(energy: f64, hotwell_mass: f64) -> f64 {
    let (mut lo, mut hi) = (1.0f64, 200.0f64);
    if condenser_energy(lo, hotwell_mass) >= energy {
        return lo;
    }
    if condenser_energy(hi, hotwell_mass) <= energy {
        return hi;
    }
    for _ in 0..50 {
        let mid = 0.5 * (lo + hi);
        if condenser_energy(mid, hotwell_mass) < energy {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// Condenser and hotwell as one first-law node: the shell vapour is in
/// equilibrium with the hotwell liquid, cooling water removes heat from that
/// node, and every mass transfer across the boundary is reported explicitly.
#[derive(Clone, Debug)]
pub struct Condenser {
    pub m_air: f64,
    /// Steam partial pressure in the shell, Pa (saturation at the node temperature).
    pub p_steam: f64,
    pub cw_pumps: [bool; N_CW_PUMPS],
    pub vacuum_pump: bool,
    pub hotwell_mass: f64,
    /// Thermal load delivered to the node this step, W (same as `q_in_w`).
    pub heat_load: f64,
    /// Node (condensing / hotwell) temperature, degC.
    pub t_sat: f64,
    /// Lumped water + metal energy, J. See `condenser_energy`.
    pub energy_j: f64,
    /// Enthalpy flow entering the node this step, W (steam, drains, returns).
    pub q_in_w: f64,
    /// Heat rejected to the circulating water this step, W.
    pub q_cw_w: f64,
    /// Energy vented through the shell relief this step, W.
    pub q_relief_w: f64,
    /// Makeup from the condensate storage tank this step, kg/s.
    pub w_makeup: f64,
    /// Reject to the condensate storage tank on high hotwell level, kg/s.
    pub w_reject: f64,
    /// Overflow to the condensate storage tank when the hotwell is full, kg/s.
    pub w_overflow: f64,
    /// Condensate withdrawal actually supplied this step, kg/s.
    pub w_out_accepted: f64,
    /// Requested withdrawal that the hotwell could not supply, kg/s.
    pub w_out_shortfall: f64,
    /// Cumulative boundary exchanges with the condensate storage tank, kg.
    pub makeup_kg: f64,
    pub reject_kg: f64,
    pub overflow_kg: f64,
    /// Cumulative withdrawal the hotwell could not supply, kg.
    pub withdrawal_deficit_kg: f64,
    /// Cumulative energy vented through the shell relief, J.
    pub relief_energy_j: f64,
    /// Cumulative energy the node could not absorb within its temperature
    /// domain (only non-zero if the caller withdraws more heat than stored), J.
    pub energy_residual_j: f64,
}

impl Condenser {
    pub fn new(vacuum: bool) -> Self {
        let p_air = if vacuum { 0.3e3 } else { P_ATM - 2.3e3 };
        let t = CW_INLET_C;
        let hotwell_mass = 0.6 * HOTWELL_CAPACITY;
        Self {
            m_air: p_air * CONDENSER_VOLUME / (287.0 * 303.0),
            p_steam: sat_t(t).p,
            cw_pumps: [vacuum; N_CW_PUMPS],
            vacuum_pump: vacuum,
            hotwell_mass,
            heat_load: 0.0,
            t_sat: t,
            energy_j: condenser_energy(t, hotwell_mass),
            q_in_w: 0.0,
            q_cw_w: 0.0,
            q_relief_w: 0.0,
            w_makeup: 0.0,
            w_reject: 0.0,
            w_overflow: 0.0,
            w_out_accepted: 0.0,
            w_out_shortfall: 0.0,
            makeup_kg: 0.0,
            reject_kg: 0.0,
            overflow_kg: 0.0,
            withdrawal_deficit_kg: 0.0,
            relief_energy_j: 0.0,
            energy_residual_j: 0.0,
        }
    }

    /// Set the node temperature directly (scenario initialisation).
    pub fn set_temperature(&mut self, t: f64) {
        self.t_sat = t;
        self.p_steam = sat_t(t).p;
        self.energy_j = condenser_energy(t, self.hotwell_mass);
    }

    pub fn p_air(&self) -> f64 {
        self.m_air * 287.0 * 303.0 / CONDENSER_VOLUME
    }

    pub fn pressure(&self) -> f64 {
        (self.p_air() + self.p_steam).min(CONDENSER_RELIEF_P)
    }

    pub fn hotwell_t(&self) -> f64 {
        self.t_sat
    }

    /// Circulating water flow, kg/s.
    pub fn cw_flow(&self) -> f64 {
        self.cw_pumps.iter().filter(|&&b| b).count() as f64 * CW_FLOW_PER_PUMP
    }

    /// Heat the circulating water would remove from the node at temperature `t`, W:
    /// `m_cw cp eff (t - t_cw_in)` with the NTU effectiveness of the tube bundle.
    pub fn cw_rejection(&self, t: f64) -> f64 {
        let m_cw = self.cw_flow();
        if m_cw <= 0.0 {
            return 0.0;
        }
        let mcp = m_cw * CP_WATER;
        let eff = 1.0 - (-CONDENSER_UA / mcp).exp();
        (mcp * eff * (t - CW_INLET_C)).max(0.0)
    }

    /// Advance with `q` the heat to reject (W) net of the hotwell enthalpy of the
    /// returned mass, `w_in` water returned to the hotwell and `w_out` condensate
    /// pumped out, kg/s. Kept for callers that already subtract `w_in * h(hotwell)`;
    /// `step_energy` takes the gross enthalpy flow instead.
    pub fn step(&mut self, dt: f64, q: f64, w_in: f64, w_out: f64) {
        let q_in = q + w_in * steam::h_liquid(self.t_sat);
        self.step_energy(dt, q_in, w_in, w_out);
    }

    /// Advance the node: `q_in_w` is the enthalpy flow carried into the shell by
    /// the returned mass `w_in` (kg/s), net of any turbine work and extraction
    /// already taken from that steam; `w_out` is the condensate withdrawal demand.
    pub fn step_energy(&mut self, dt: f64, q_in_w: f64, w_in: f64, w_out: f64) {
        let h_hw = steam::h_liquid(self.t_sat);
        let h_cst = steam::h_liquid(CST_TEMP_C);
        let frac0 = self.hotwell_mass / HOTWELL_CAPACITY;

        // Mass: returns and makeup first, then withdrawals bounded by what is there.
        self.w_makeup = if frac0 < HOTWELL_MAKEUP_FRAC { HOTWELL_TRANSFER_RATE } else { 0.0 };
        let mut mass = self.hotwell_mass + (w_in.max(0.0) + self.w_makeup) * dt;
        self.w_out_accepted = w_out.max(0.0).min(mass / dt);
        self.w_out_shortfall = w_out.max(0.0) - self.w_out_accepted;
        mass -= self.w_out_accepted * dt;
        self.w_reject = if frac0 > HOTWELL_REJECT_FRAC { HOTWELL_TRANSFER_RATE.min(mass / dt) } else { 0.0 };
        mass -= self.w_reject * dt;
        self.w_overflow = ((mass - HOTWELL_CAPACITY) / dt).max(0.0);
        mass -= self.w_overflow * dt;
        self.hotwell_mass = mass;
        self.makeup_kg += self.w_makeup * dt;
        self.reject_kg += self.w_reject * dt;
        self.overflow_kg += self.w_overflow * dt;
        self.withdrawal_deficit_kg += self.w_out_shortfall * dt;

        // Energy: first law on the lumped node.
        self.q_in_w = q_in_w;
        self.heat_load = q_in_w;
        self.q_cw_w = self.cw_rejection(self.t_sat);
        let q_boundary = self.w_makeup * h_cst - (self.w_out_accepted + self.w_reject + self.w_overflow) * h_hw;
        self.energy_j += (q_in_w - self.q_cw_w + q_boundary) * dt;
        let t_relief = sat_p(CONDENSER_RELIEF_P).t;
        let e_relief = condenser_energy(t_relief, self.hotwell_mass);
        self.q_relief_w = if self.energy_j > e_relief { (self.energy_j - e_relief) / dt } else { 0.0 };
        self.energy_j -= self.q_relief_w * dt;
        self.relief_energy_j += self.q_relief_w * dt;
        let t = condenser_temperature(self.energy_j, self.hotwell_mass);
        let e_t = condenser_energy(t, self.hotwell_mass);
        // Only the lower domain bound can leave a residual (heat withdrawn below 1 degC).
        self.energy_residual_j += self.energy_j - e_t;
        self.energy_j = e_t;
        self.t_sat = t;
        self.p_steam = sat_t(t).p;

        let p = self.pressure();
        let leak = AIR_INLEAKAGE * ((P_ATM - p) / P_ATM).clamp(0.0, 1.0);
        let removal = if self.vacuum_pump { self.m_air / VACUUM_PUMP_TAU } else { 0.0 };
        self.m_air = (self.m_air + (leak - removal) * dt).max(0.0);
        // Air cannot exceed atmospheric partial pressure (relief to atmosphere).
        let max_air = (P_ATM - self.p_steam).max(0.0) * CONDENSER_VOLUME / (287.0 * 303.0);
        self.m_air = self.m_air.min(max_air.max(0.0));
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

/// Condensate / feedwater train: two 100 % adjustable-speed reactor feed pumps,
/// a low-flow control valve, and a lumped feedwater heater energy store fed by
/// turbine extraction steam.
///
/// The level controller output `fcv_pos` (0..1) is the feed demand: it strokes
/// the low-flow control valve directly and sets the pump speed demand from
/// `FW_PUMP_MIN_SPEED` to full speed, so the valve dominates at low flow and the
/// drives at power.
#[derive(Clone, Debug)]
pub struct Feedwater {
    pub pumps: [bool; N_FEED_PUMPS],
    /// Actual pump speed, per unit of rated.
    pub pump_speed: [f64; N_FEED_PUMPS],
    /// Flow delivered by each pump, kg/s.
    pub pump_flow: [f64; N_FEED_PUMPS],
    pub cond_pumps: [bool; N_COND_PUMPS],
    /// Feed demand / low-flow control valve position, 0..1.
    pub fcv_pos: f64,
    pub fcv_manual: f64,
    pub mode: LevelControlMode,
    pub level_sp_cm: f64,
    /// Automatic level setpoint setdown after a scram, cm.
    pub setdown_cm: f64,
    pub heaters_in_service: bool,
    /// Heater train outlet (feedwater) temperature, degC: the lumped heater node.
    pub t_feed: f64,
    pub rwcu_reject: f64,
    pub flow: f64,
    /// Extraction heat actually transferred to the heater node this step, W.
    /// Debit this from the turbine / condenser side.
    pub q_extraction_w: f64,
    /// Extraction heat the turbine steam could have supplied this step, W.
    pub q_extraction_avail_w: f64,
    /// Pumping work deposited in the feedwater this step, W.
    pub q_pump_w: f64,
    /// Heat lost from the heater train to ambient this step, W.
    pub q_loss_w: f64,
    integral: f64,
    low_suction_timer: f64,
}

impl Feedwater {
    pub fn new(pumps_running: usize, t_feed: f64, mode: LevelControlMode, fcv: f64) -> Self {
        let mut pumps = [false; N_FEED_PUMPS];
        let mut pump_speed = [0.0; N_FEED_PUMPS];
        for (p, s) in pumps.iter_mut().zip(pump_speed.iter_mut()).take(pumps_running) {
            *p = true;
            *s = Self::speed_demand(fcv);
        }
        let cond = pumps_running > 0;
        Self {
            pumps,
            pump_speed,
            pump_flow: [0.0; N_FEED_PUMPS],
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
            q_extraction_w: 0.0,
            q_extraction_avail_w: 0.0,
            q_pump_w: 0.0,
            q_loss_w: 0.0,
            integral: fcv,
            low_suction_timer: 0.0,
        }
    }

    pub fn running_pumps(&self) -> usize {
        self.pumps.iter().filter(|&&b| b).count()
    }

    /// Pump speed demand (per unit) for a feed demand `u` in 0..1.
    pub fn speed_demand(u: f64) -> f64 {
        FW_PUMP_MIN_SPEED + (1.0 - FW_PUMP_MIN_SPEED) * u.clamp(0.0, 1.0)
    }

    /// Stored energy of the heater node relative to the hotwell, J.
    pub fn heater_energy_j(&self, hotwell_t: f64) -> f64 {
        FW_HEATER_HEAT_CAP * (self.t_feed - hotwell_t)
    }

    fn suction_pressure(&self, hotwell_ok: bool) -> f64 {
        if hotwell_ok && self.cond_pumps.iter().any(|&b| b) {
            CONDENSATE_PRESSURE
        } else {
            0.0
        }
    }

    /// Flow from one pump at speed `s` against a differential `dp`, kg/s
    /// (affinity-law parabola: head scales with s^2, flow with s).
    fn pump_curve_flow(s: f64, dp: f64) -> f64 {
        let x = s * s - dp / FW_PUMP_SHUTOFF_DP;
        if x <= 0.0 {
            0.0
        } else {
            FW_PUMP_RUNOUT * x.sqrt()
        }
    }

    /// Solve the feed system hydraulics for flow into a vessel at `p_reactor`.
    /// Returns total flow and sets `pump_flow`.
    fn hydraulics(&mut self, p_reactor: f64, p_suction: f64) -> f64 {
        let pos = self.fcv_pos;
        self.pump_flow = [0.0; N_FEED_PUMPS];
        if pos < 1e-4 || p_suction <= 0.0 {
            return 0.0;
        }
        let static_dp = 0.15e6;
        let valve = |q: f64| (q / (C_FCV * fcv_characteristic(pos))).powi(2);
        // Differential the pumps must supply at total flow q.
        let header_dp = |q: f64| p_reactor + static_dp + valve(q) - p_suction;
        let speeds: Vec<f64> =
            self.pumps.iter().zip(self.pump_speed.iter()).filter(|(&on, _)| on).map(|(_, &s)| s).collect();

        if speeds.is_empty() {
            // Condensate pressure alone, through the idle feed pumps.
            let f = |q: f64| -header_dp(q) - (q / 150.0).powi(2) * 1.0e5;
            if f(0.0) <= 0.0 {
                return 0.0;
            }
            let (mut lo, mut hi) = (0.0, FW_PUMP_RUNOUT);
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
            return 0.5 * (lo + hi);
        }

        // Parallel pumps: supplied flow at the header differential minus demanded flow.
        let supply = |q: f64| speeds.iter().map(|&s| Self::pump_curve_flow(s, header_dp(q))).sum::<f64>();
        let f = |q: f64| supply(q) - q;
        if f(0.0) <= 0.0 {
            return 0.0;
        }
        let (mut lo, mut hi) = (0.0, speeds.len() as f64 * FW_PUMP_RUNOUT);
        for _ in 0..60 {
            let mid = 0.5 * (lo + hi);
            if f(mid) > 0.0 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let q = 0.5 * (lo + hi);
        let dp = header_dp(q);
        let raw: Vec<f64> = self
            .pumps
            .iter()
            .zip(self.pump_speed.iter())
            .map(|(&on, &s)| if on { Self::pump_curve_flow(s, dp) } else { 0.0 })
            .collect();
        let sum: f64 = raw.iter().sum();
        if sum > 0.0 {
            for (pf, r) in self.pump_flow.iter_mut().zip(raw.iter()) {
                *pf = r / sum * q;
            }
        }
        q
    }

    /// Level controller, pump drives, hydraulics and heater energy balance.
    /// Returns any pump trip message.
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

        // Adjustable-speed drives follow the feed demand; stopped pumps coast down.
        let s_demand = Self::speed_demand(self.fcv_pos);
        for (on, s) in self.pumps.iter().zip(self.pump_speed.iter_mut()) {
            let (target, rate) = if *on { (s_demand, FW_PUMP_SPEED_RATE) } else { (0.0, FW_PUMP_COASTDOWN_RATE) };
            *s += (target - *s).clamp(-rate * dt, rate * dt);
        }

        self.flow = self.hydraulics(p_reactor, p_s);

        // Feedwater heater node: extraction steam from the turbine is the only heat
        // source, bounded by the steam actually flowing to the turbine, and the
        // node's stored energy drains into the feedwater once that steam is gone.
        let h_in = steam::h_liquid(hotwell_t);
        let h_out = steam::h_liquid(self.t_feed);
        let service = if self.heaters_in_service { 1.0 } else { 0.65 };
        let load = (turbine_flow / STEAM_FLOW_RATED).clamp(0.0, 1.2);
        let t_des = hotwell_t + (T_FW_RATED - 35.0) * load.powf(0.75) * service + FW_PUMP_TEMP_RISE;
        self.q_pump_w = if self.running_pumps() > 0 { self.flow * CP_WATER * FW_PUMP_TEMP_RISE } else { 0.0 };
        self.q_loss_w = FW_HEATER_LOSS_UA * (self.t_feed - CST_TEMP_C).max(0.0);
        self.q_extraction_avail_w = turbine_flow.max(0.0) * FW_EXTRACTION_HEAT_PER_KG * service;
        let q_needed = self.flow * (steam::h_liquid(t_des) - h_in) + FW_HEATER_HEAT_CAP * (t_des - self.t_feed) / 60.0
            - self.q_pump_w
            + self.q_loss_w;
        self.q_extraction_w = q_needed.clamp(0.0, self.q_extraction_avail_w);
        let q_net = self.q_extraction_w + self.q_pump_w - self.q_loss_w - self.flow * (h_out - h_in);
        self.t_feed += q_net * dt / FW_HEATER_HEAT_CAP;
        msg
    }

    pub fn h_feed(&self) -> f64 {
        steam::h_liquid(self.t_feed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f64 = 0.05;

    #[test]
    fn steam_line_is_isenthalpic() {
        let mut s = SteamSystem::new(true);
        s.tcv_pos = 0.85;
        s.compute_flows(P_RATED, true);
        assert!(s.p_chest < P_RATED - 0.1e6, "chest {}", s.p_chest);
        assert!(s.w_tcv > 300.0);
        let h_in = sat_p(P_RATED).h_g;
        assert!((s.h_steam - h_in).abs() < 1.0, "delivered {} vs inlet {}", s.h_steam, h_in);
        // The old model resaturated at chest pressure, which differs by > 1 kJ/kg.
        assert!((sat_p(s.p_chest).h_g - h_in).abs() > 1.0e3);
    }

    #[test]
    fn bypass_fast_opens_within_300_ms_on_turbine_trip() {
        let mut s = SteamSystem::new(true);
        s.p_chest = 7.0e6;
        s.p_setpoint = P_RATED - 2.0 * PRESSURE_REG_SPAN; // full pressure demand
        for _ in 0..6 {
            s.step(DT, P_RATED, 0.0, true, true);
        }
        assert!(s.bypass_pos > 0.99, "bypass at {} after 0.3 s", s.bypass_pos);
    }

    #[test]
    fn turbine_is_a_two_pole_3600_rpm_machine() {
        assert_eq!(RATED_RPM, 3600.0);
        assert!((OVERSPEED_TRIP_RPM - 3960.0).abs() < 1e-6);
        assert!((TG_INERTIA - 18_576.0).abs() / 18_576.0 < 0.01, "inertia {TG_INERTIA}");
        let mut t = Turbine::new(true);
        assert_eq!(t.slip_hz(), 0.0);
        t.speed_rpm = 3570.0;
        assert!((t.slip_hz() + 0.5).abs() < 1e-9, "slip {}", t.slip_hz());
        assert_eq!(t.electrical_hz(), 59.5);
    }

    #[test]
    fn turbine_shaft_energy_matches_inertia_constant() {
        // 0.5 J w^2 at rated speed == H * S by construction.
        let e = 0.5 * TG_INERTIA * OMEGA_RATED * OMEGA_RATED;
        assert!((e - TG_H_S * TG_MVA).abs() / (TG_H_S * TG_MVA) < 1e-9);
    }

    #[test]
    fn condenser_stores_energy_without_cooling_water() {
        let mut c = Condenser::new(false);
        c.hotwell_mass = 0.6 * HOTWELL_CAPACITY;
        c.set_temperature(25.0);
        let e0 = c.energy_j;
        let (q, w_in) = (100.0e6, 40.0);
        let mut supplied = 0.0;
        for _ in 0..200 {
            c.step_energy(DT, q, w_in, 0.0);
            supplied += q * DT;
            assert_eq!(c.q_cw_w, 0.0);
            assert_eq!(c.q_relief_w, 0.0);
        }
        // 1 GJ in, nothing out: it is all in the node, and the node got hotter.
        assert!((c.energy_j - e0 - supplied).abs() < 1.0e-6 * supplied, "dE {} vs {}", c.energy_j - e0, supplied);
        // 1 GJ over ~4.8e8 J/K of water + metal is about 2 K.
        assert!(c.t_sat > 26.5, "temperature {}", c.t_sat);
        assert!(c.p_steam > sat_t(26.5).p);
    }

    #[test]
    fn condenser_rejects_rated_load_at_design_vacuum() {
        let mut c = Condenser::new(true);
        let (q, w) = (548.0e6, STEAM_FLOW_RATED);
        for _ in 0..(600.0 / DT) as usize {
            let q_in = q + w * steam::h_liquid(c.t_sat);
            c.step_energy(DT, q_in, w, w);
        }
        assert!((c.q_cw_w - q).abs() < 0.01 * q, "cw {} vs {}", c.q_cw_w, q);
        assert!((c.t_sat - 34.8).abs() < 1.0, "t {}", c.t_sat);
        assert!((c.pressure() - 5.6e3).abs() < 0.6e3, "p {}", c.pressure());
    }

    #[test]
    fn condenser_losing_cooling_water_heats_up_to_relief() {
        let mut c = Condenser::new(true);
        c.set_temperature(35.0);
        c.cw_pumps = [false; N_CW_PUMPS];
        let mut vented = 0.0;
        for _ in 0..(300.0 / DT) as usize {
            let q_in = 548.0e6 + STEAM_FLOW_RATED * steam::h_liquid(c.t_sat);
            c.step_energy(DT, q_in, STEAM_FLOW_RATED, STEAM_FLOW_RATED);
            vented += c.q_relief_w * DT;
        }
        assert!(c.pressure() >= P_ATM, "pressure {}", c.pressure());
        assert!(vented > 0.0 && (c.relief_energy_j - vented).abs() < 1.0);
    }

    #[test]
    fn hotwell_overflow_and_reject_are_explicit() {
        let mut c = Condenser::new(true);
        c.hotwell_mass = HOTWELL_CAPACITY - 1.0;
        c.set_temperature(35.0);
        let m0 = c.hotwell_mass;
        c.step(1.0, 1.0e6, 100.0, 0.0);
        assert_eq!(c.hotwell_mass, HOTWELL_CAPACITY);
        let accounted = (c.hotwell_mass - m0) + c.reject_kg + c.overflow_kg;
        assert!((accounted - 100.0).abs() < 1e-9, "accounted {accounted}");
        assert!(c.overflow_kg > 0.0 && c.reject_kg > 0.0);
    }

    #[test]
    fn hotwell_withdrawal_is_bounded_by_inventory() {
        let mut c = Condenser::new(true);
        c.hotwell_mass = 10.0;
        c.set_temperature(35.0);
        c.step(1.0, 0.0, 0.0, 100.0);
        assert!(c.hotwell_mass >= 0.0);
        assert!((c.w_out_accepted + c.w_out_shortfall - 100.0).abs() < 1e-9);
        assert!(c.withdrawal_deficit_kg > 0.0, "deficit must be recorded");
        assert!((c.w_out_accepted - (10.0 + HOTWELL_TRANSFER_RATE)).abs() < 1e-9);
    }

    #[test]
    fn feedwater_heating_is_bounded_by_turbine_steam() {
        let mut fw = Feedwater::new(2, T_FW_RATED, LevelControlMode::Manual, 0.7);
        fw.fcv_manual = 0.7;
        let hw = 35.0;
        let e0 = fw.heater_energy_j(hw);
        let mut delivered = 0.0;
        let mut pump_work = 0.0;
        let t0 = fw.t_feed;
        for _ in 0..(120.0 / DT) as usize {
            fw.step(DT, P_RATED, 0.0, 0.0, 0.0, true, hw);
            assert_eq!(fw.q_extraction_w, 0.0, "no turbine steam, no extraction heat");
            delivered += fw.flow * (fw.h_feed() - steam::h_liquid(hw)) * DT;
            pump_work += fw.q_pump_w * DT;
        }
        assert!(fw.flow > 100.0, "flow {}", fw.flow);
        assert!(fw.t_feed < t0 - 20.0, "heater must cool once steam is gone, t {}", fw.t_feed);
        assert!(delivered <= e0 + pump_work, "delivered {delivered} J from a store of {e0} J");
        // Store depletion equals what was delivered, within the ambient loss.
        let e1 = fw.heater_energy_j(hw);
        assert!(e0 - e1 >= delivered - pump_work - 1.0, "store {} -> {}, delivered {}", e0, e1, delivered);
    }

    #[test]
    fn feedwater_extraction_never_exceeds_available_steam() {
        let mut fw = Feedwater::new(2, 40.0, LevelControlMode::Manual, 0.7);
        fw.fcv_manual = 0.7;
        let w_t = 0.5 * STEAM_FLOW_RATED;
        for _ in 0..(60.0 / DT) as usize {
            fw.step(DT, P_RATED, 0.0, w_t, w_t, true, 35.0);
            assert!(fw.q_extraction_w <= w_t * FW_EXTRACTION_HEAT_PER_KG + 1e-6);
            assert!(fw.q_extraction_w >= 0.0);
        }
        assert!(fw.t_feed > 60.0, "a cold heater warms up on extraction steam, t {}", fw.t_feed);
    }

    #[test]
    fn two_full_capacity_variable_speed_feed_pumps() {
        assert_eq!(N_FEED_PUMPS, 2);
        let mut fw = Feedwater::new(1, T_FW_RATED, LevelControlMode::Manual, 1.0);
        fw.fcv_manual = 1.0;
        fw.pump_speed[0] = 1.0;
        fw.step(DT, P_RATED, 0.0, 0.0, 0.0, true, 35.0);
        assert!(fw.flow >= STEAM_FLOW_RATED, "one pump at full speed gives {} kg/s", fw.flow);
        assert!((fw.pump_flow[0] - fw.flow).abs() < 1e-6 && fw.pump_flow[1] == 0.0);
        // A stopped pump does not pass flow backwards; speed ramps toward demand.
        assert!(fw.pump_speed[1] == 0.0);
        let mut both = Feedwater::new(2, T_FW_RATED, LevelControlMode::Manual, 0.5);
        both.fcv_manual = 0.5;
        both.pump_speed = [1.0, 1.0];
        both.step(DT, P_RATED, 0.0, 0.0, 0.0, true, 35.0);
        assert!(both.pump_speed[0] < 1.0, "drives follow the feed demand");
        assert!((both.pump_flow[0] - both.pump_flow[1]).abs() < 1e-6);
    }
}
