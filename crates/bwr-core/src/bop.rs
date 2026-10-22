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
    /// Steam-line inlet pressure of the committed hydraulic state, Pa: dome
    /// pressure while the vessel supplies the hydraulic request, lower when the
    /// vessel's phase inventory limits delivery (see `limit_to_delivered`).
    pub p_line_inlet: f64,
    /// Line resistance of the last flow solution, Pa/(kg/s)^2.
    pub k_line: f64,
    /// Specific enthalpy of the steam delivered to the turbine and bypass, J/kg.
    /// The steam line and valve throttling are isenthalpic: this is the vessel
    /// outlet enthalpy, not the saturated-vapour enthalpy at chest pressure.
    pub h_steam: f64,
    /// Remaining time of a latched bypass fast-open stroke, s. A turbine-trip or
    /// load-rejection signal arms it for one full fast stroke even when the
    /// signal itself lasts only one scan.
    pub fast_open_latch: f64,
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
            p_line_inlet: 0.0,
            k_line: 0.0,
            h_steam: sat_p(P_ATM).h_g,
            fast_open_latch: 0.0,
        }
    }

    fn msiv_open_fraction(&self) -> f64 {
        self.msiv_pos.iter().sum::<f64>() / N_STEAM_LINES as f64
    }

    /// Steam flows for the current valve positions at dome pressure `p`, taking the
    /// vessel outlet as saturated vapour at `p`.
    pub fn compute_flows(&mut self, p: f64, tsv_open: bool) {
        let s = sat_p(p.max(P_ATM));
        self.compute_flows_from(p, s.h_g, s.rho_g, tsv_open);
    }

    /// Steam flows for the current valve positions at dome pressure `p` with vessel
    /// outlet enthalpy `h_in` and inlet density `rho_in` (homogeneous density for
    /// a mixture). No heat or work is exchanged in the line or the throttles, so
    /// the delivered enthalpy equals `h_in`.
    ///
    /// Both resistances are steam calibrations, corrected for the inlet density
    /// at equal mass flow (R3): the line is Darcy friction, dp = K w^2 / rho, with
    /// K fixed by the rated-steam calibration, so its coefficient scales by
    /// rho_rated / rho_in. The valves' pressure-linear law is the choked-vapour
    /// law, whose density (proportional to inlet pressure) is already built in;
    /// its resistance therefore scales by rho_vapour(p) / rho_in, which is 1 for
    /// saturated vapour at every pressure. At rated steam both factors are 1.
    pub fn compute_flows_from(&mut self, p: f64, h_in: f64, rho_in: f64, tsv_open: bool) {
        self.h_steam = h_in;
        let m = self.msiv_open_fraction();
        let (line_factor, valve_factor) = Self::density_factors(p, rho_in);
        let a_tcv = if tsv_open { C_TCV * self.tcv_pos * valve_factor } else { 0.0 };
        let a_byp = C_BYPASS * self.bypass_pos * valve_factor;
        let a = a_tcv + a_byp;
        let p_eff = (p - P_ATM).max(0.0);
        self.p_line_inlet = p;
        self.k_line = 0.0;
        if m < 1e-4 || a < 1e-12 || p_eff <= 0.0 {
            self.w_tcv = 0.0;
            self.w_bypass = 0.0;
            self.p_chest = if m < 1e-4 { 0.0 } else { p };
            return;
        }
        let k = STEAM_LINE_DP / (STEAM_FLOW_RATED * STEAM_FLOW_RATED) / (m * m) * line_factor;
        let w = (-1.0 + (1.0 + 4.0 * a * a * k * p_eff).sqrt()) / (2.0 * a * k);
        self.k_line = k;
        self.p_chest = p - k * w * w;
        self.w_tcv = a_tcv * (self.p_chest - P_ATM).max(0.0);
        self.w_bypass = a_byp * (self.p_chest - P_ATM).max(0.0);
    }

    /// (line resistance multiplier, valve conductance multiplier) for an inlet
    /// of density `rho_in` at pressure `p`; see `compute_flows_from`.
    fn density_factors(p: f64, rho_in: f64) -> (f64, f64) {
        if !(rho_in.is_finite() && rho_in > 0.0) {
            return (1.0, 1.0);
        }
        let rho_rated = sat_p(P_RATED).rho_g;
        let rho_vapour = sat_p(p.max(P_ATM)).rho_g;
        (rho_rated / rho_in, rho_in / rho_vapour)
    }

    pub fn total_flow(&self) -> f64 {
        self.w_tcv + self.w_bypass
    }

    /// Commit the flow the vessel actually delivered (`w_delivered`, at most the
    /// hydraulic request). Valve areas are fixed for the step, so a supply
    /// shortfall cannot coexist with dome pressure at the line inlet: the
    /// starved inlet is the explicit supply-limited boundary. The valves pass the
    /// delivered flow at the chest pressure their own relation gives,
    /// w = A (p_chest - P_ATM), and the line inlet sits at p_chest + k w^2, below
    /// dome pressure. Both relations hold at the committed flow, so turbine work,
    /// the next EHC scan and MSIV protection see one hydraulic state (R2).
    pub fn limit_to_delivered(&mut self, w_delivered: f64) {
        let w_req = self.total_flow();
        if !(w_req > 0.0 && w_delivered < w_req) {
            return;
        }
        let ratio = (w_delivered / w_req).max(0.0);
        self.w_tcv *= ratio;
        self.w_bypass *= ratio;
        // Flow is linear in (p_chest - P_ATM) at fixed area.
        self.p_chest = P_ATM + ratio * (self.p_chest - P_ATM);
        let w = self.total_flow();
        self.p_line_inlet = self.p_chest + self.k_line * w * w;
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
    /// is the valid turbine-trip / generator load-rejection signal. The signal
    /// latches for one full fast stroke (`BYPASS_FAST_OPEN_TIME`). While latched,
    /// the TCVs fast-close to the governor demand (power/load unbalance action)
    /// and the bypass takes the pressure regulator demand net of that governor
    /// demand at once, instead of waiting for the TCVs to stroke shut.
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
        if fast_open {
            self.fast_open_latch = BYPASS_FAST_OPEN_TIME;
        }
        let latched = self.fast_open_latch > 0.0;

        // Turbine control valves.
        let tcv_target = if tripped { 0.0 } else { (tcv_flow_demand / (C_TCV * p_ref)).clamp(0.0, 1.0) };
        let d = tcv_target - self.tcv_pos;
        let (open_rate, close_rate) = if tripped {
            (0.0, 5.0)
        } else if latched {
            (0.10, 5.0)
        } else {
            (0.10, 0.5)
        };
        self.tcv_pos += d.clamp(-close_rate * dt, open_rate * dt);

        // Bypass valves.
        let target = if !bypass_available {
            0.0
        } else if self.bypass_auto {
            let demand = self.pressure_demand(p);
            // During a fast-open stroke the TCVs are heading for their target, so
            // the bypass is sized against that target, not the valve's lagging position.
            let tcv_open = if latched { tcv_target } else { self.tcv_pos };
            let tcv_flow = if tripped { 0.0 } else { C_TCV * tcv_open * p_ref };
            ((demand - tcv_flow).max(0.0) / (C_BYPASS * p_ref)).clamp(0.0, 1.0)
        } else {
            self.bypass_manual_pos.clamp(0.0, 1.0)
        };
        let d = target - self.bypass_pos;
        let (open_rate, close_rate) = if !self.bypass_auto {
            (BYPASS_MANUAL_RATE, BYPASS_MANUAL_RATE)
        } else if latched {
            (BYPASS_FAST_OPEN_RATE, BYPASS_REG_RATE)
        } else {
            (BYPASS_REG_RATE, BYPASS_REG_RATE)
        };
        self.bypass_pos += d.clamp(-close_rate * dt, open_rate * dt);
        // Drop a rounding remainder so a stroke of n steps latches exactly n steps.
        self.fast_open_latch = self.fast_open_latch - dt;
        if self.fast_open_latch < 1e-9 {
            self.fast_open_latch = 0.0;
        }
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

    /// Advance shaft dynamics given the accepted turbine inlet. Only its vapour
    /// does work, never more per kg than the isentropic bound.
    pub fn step(&mut self, dt: f64, inlet: &TurbineInlet) {
        let (p_chest, p_cond) = (inlet.p_chest, inlet.p_cond);
        let pf = if p_chest > p_cond && p_cond > 0.0 {
            ((p_chest / p_cond).ln() / (7.0e6f64 / 5.6e3).ln()).clamp(0.0, 1.2)
        } else {
            0.0
        };
        self.p_mech = inlet.w_vapour * (TURBINE_DH_RATED * pf).min(inlet.work_limit);
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

// ------------------------------------------------------------------ turbine inlet

/// Phase split and energy budget of the flow the turbine accepted this step.
/// Throttling to chest pressure is isenthalpic, so the inlet quality there
/// follows from the delivered enthalpy. Only the vapour expands through the
/// blading and feeds the extraction heaters; the liquid fraction (carryover or
/// a water-solid discharge) drains to the condenser at its own enthalpy with
/// no work (R1).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TurbineInlet {
    /// Total accepted flow, kg/s, and its enthalpy, J/kg.
    pub w: f64,
    pub h: f64,
    /// Vapour fraction, kg/s, and its enthalpy at chest pressure, J/kg.
    pub w_vapour: f64,
    pub h_vapour: f64,
    /// Liquid fraction, kg/s, and its enthalpy, J/kg.
    pub w_liquid: f64,
    pub h_liquid: f64,
    pub p_chest: f64,
    pub p_cond: f64,
    /// Shaft work bound per kg of vapour, J/kg: TURBINE_ETA_IS times the
    /// isentropic drop from the vapour state at chest pressure to the condenser.
    pub work_limit: f64,
    /// Saturated-liquid enthalpy at condenser pressure, J/kg: the lowest state
    /// the vapour's exhaust and extraction drains can reach in the condenser.
    pub h_floor: f64,
}

impl TurbineInlet {
    pub fn new(w: f64, h: f64, p_chest: f64, p_cond: f64) -> Self {
        let w = w.max(0.0);
        let h_floor = sat_p(p_cond.max(1.0)).h_f;
        if !(w > 0.0 && p_chest > 0.0) {
            return Self { w, h, h_vapour: h, h_liquid: h, w_liquid: w, p_chest, p_cond, h_floor, ..Self::default() };
        }
        let s = sat_p(p_chest);
        let x = ((h - s.h_f) / s.h_fg()).clamp(0.0, 1.0);
        let h_vapour = if x >= 1.0 { h } else { s.h_g };
        let h_liquid = if x <= 0.0 { h } else { s.h_f };
        let w_vapour = w * x;
        let work_limit = if w_vapour > 0.0 { TURBINE_ETA_IS * steam::isentropic_drop(h_vapour, p_chest, p_cond) } else { 0.0 };
        Self { w, h, w_vapour, h_vapour, w_liquid: w - w_vapour, h_liquid, p_chest, p_cond, work_limit, h_floor }
    }

    /// Extraction heat the vapour can still give up after `p_mech` of shaft
    /// work, W: extraction and exhaust share one available-energy budget, so
    /// neither can draw the condenser inlet below saturated liquid.
    pub fn extraction_budget(&self, p_mech: f64) -> f64 {
        (self.w_vapour * (self.h_vapour - self.h_floor) - p_mech).max(0.0)
    }
}

// ------------------------------------------------------------------ condenser

/// Shell vapour mass in saturated equilibrium at `t` with total node mass `m`, kg:
/// the shell steam space is filled with saturated vapour, unless there is too
/// little water to fill it.
fn condenser_vapour_mass(t: f64, m: f64) -> f64 {
    (sat_t(t).rho_g * CONDENSER_VOLUME).min(m.max(0.0))
}

/// Equilibrium energy of the condenser node, J, at temperature `t` with total
/// water mass `m` (hotwell liquid plus shell vapour): liquid on a saturated-liquid
/// enthalpy basis (zero at 0 degC), vapour on its internal energy, plus the
/// condenser metal at the same temperature.
fn condenser_energy(t: f64, m: f64) -> f64 {
    let sat = sat_t(t);
    let m_v = condenser_vapour_mass(t, m);
    (m - m_v) * sat.h_f + m_v * sat.u_g() + CONDENSER_METAL_HEAT_CAP * t
}

const CONDENSER_T_MIN: f64 = 1.0;
const CONDENSER_T_MAX: f64 = 200.0;

/// Inverse of `condenser_energy` for the temperature, degC (bisection; the
/// energy is strictly increasing in temperature because u_g > h_f).
fn condenser_temperature(energy: f64, m: f64) -> f64 {
    let (mut lo, mut hi) = (CONDENSER_T_MIN, CONDENSER_T_MAX);
    if condenser_energy(lo, m) >= energy {
        return lo;
    }
    if condenser_energy(hi, m) <= energy {
        return hi;
    }
    for _ in 0..60 {
        let mid = 0.5 * (lo + hi);
        if condenser_energy(mid, m) < energy {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// Node temperature path over one step: a constant heating rate `rate` (K/s,
/// every source but cooling water), CW exchange `a (T - t_cw)` (1/s) that only
/// removes heat, and the node held at `t_cap` (the relief temperature) while
/// the heating exceeds the CW removal there. Exact piecewise solution (linear
/// below the CW inlet, exponential approach above it, constant at the cap), so
/// no step length overshoots the CW inlet temperature. Returns the end
/// temperature (before relief) and the integral of max(T - t_cw, 0), K s.
fn cw_trajectory(t0: f64, rate: f64, a: f64, dt: f64, t_cw: f64, t_cap: f64) -> (f64, f64) {
    let (mut t, mut left, mut integral) = (t0, dt, 0.0);
    for _ in 0..4 {
        if left <= 0.0 {
            break;
        }
        if t < t_cw || (t <= t_cw && rate <= 0.0) {
            // No CW exchange at or below its inlet temperature.
            if rate <= 0.0 || t + rate * left <= t_cw {
                return (t + rate * left, integral);
            }
            left -= (t_cw - t) / rate;
            t = t_cw;
            continue;
        }
        let t_inf = t_cw + rate / a;
        if t >= t_cap && t_inf >= t_cap {
            return (t, integral + (t - t_cw) * left);
        }
        // Exponential approach to t_inf, interrupted on reaching the cap
        // (rising) or the CW inlet (falling past it).
        let stop = if t_inf > t && t < t_cap {
            Some(t_cap)
        } else if t_inf < t_cw {
            Some(t_cw)
        } else {
            None
        };
        let hit = stop.and_then(|b| {
            let ratio = (b - t_inf) / (t - t_inf);
            let seg = -ratio.ln() / a;
            (ratio > 0.0 && ratio < 1.0 && seg < left).then_some((b, seg))
        });
        let seg = hit.map_or(left, |(_, seg)| seg);
        let decay = (-a * seg).exp();
        integral += (t_inf - t_cw) * seg + (t - t_inf) * (1.0 - decay) / a;
        left -= seg;
        t = hit.map_or(t_inf + (t - t_inf) * decay, |(b, _)| b);
    }
    (t, integral)
}

/// Condenser shell and hotwell as one equilibrium two-phase node: shell vapour
/// (`vapour_mass`) and hotwell liquid (`hotwell_mass`) share one saturation
/// temperature, so condensation and evaporation are the phase split of the
/// conserved total mass and energy. Cooling water removes heat from the node,
/// the shell relief vents saturated steam (mass and enthalpy) at its set
/// pressure, and every mass transfer across the boundary is reported explicitly.
#[derive(Clone, Debug)]
pub struct Condenser {
    pub m_air: f64,
    /// Steam partial pressure in the shell, Pa (saturation at the node temperature).
    pub p_steam: f64,
    pub cw_pumps: [bool; N_CW_PUMPS],
    pub vacuum_pump: bool,
    /// Hotwell liquid, kg.
    pub hotwell_mass: f64,
    /// Saturated vapour in the shell steam space, kg.
    pub vapour_mass: f64,
    /// Thermal load delivered to the node this step, W (same as `q_in_w`).
    pub heat_load: f64,
    /// Node (condensing / hotwell) temperature, degC.
    pub t_sat: f64,
    /// Node energy, J: the conserved state. See `condenser_energy`.
    pub energy_j: f64,
    /// Enthalpy flow entering the node this step, W (steam, drains, returns).
    pub q_in_w: f64,
    /// Heat rejected to the circulating water this step, W.
    pub q_cw_w: f64,
    /// Steam vented through the shell relief this step, kg/s.
    pub w_relief: f64,
    /// Enthalpy vented with that steam this step, W (`w_relief * h_g`).
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
    /// Net enthalpy flow exchanged with the condensate storage tank this step
    /// (makeup in minus reject and overflow out), W.
    pub q_cst_w: f64,
    /// Cumulative boundary exchanges with the condensate storage tank, kg.
    pub makeup_kg: f64,
    pub reject_kg: f64,
    pub overflow_kg: f64,
    /// Cumulative net enthalpy received from the condensate storage tank, J.
    pub cst_energy_j: f64,
    /// Cumulative withdrawal the hotwell could not supply, kg.
    pub withdrawal_deficit_kg: f64,
    /// Cumulative steam vented through the shell relief, kg.
    pub relief_kg: f64,
    /// Cumulative enthalpy vented through the shell relief, J.
    pub relief_energy_j: f64,
}

impl Condenser {
    pub fn new(vacuum: bool) -> Self {
        let p_air = if vacuum { 0.3e3 } else { P_ATM - 2.3e3 };
        let mut c = Self {
            m_air: p_air * CONDENSER_VOLUME / (287.0 * 303.0),
            p_steam: 0.0,
            cw_pumps: [vacuum; N_CW_PUMPS],
            vacuum_pump: vacuum,
            hotwell_mass: 0.6 * HOTWELL_CAPACITY,
            vapour_mass: 0.0,
            heat_load: 0.0,
            t_sat: CW_INLET_C,
            energy_j: 0.0,
            q_in_w: 0.0,
            q_cw_w: 0.0,
            w_relief: 0.0,
            q_relief_w: 0.0,
            w_makeup: 0.0,
            w_reject: 0.0,
            w_overflow: 0.0,
            w_out_accepted: 0.0,
            w_out_shortfall: 0.0,
            q_cst_w: 0.0,
            makeup_kg: 0.0,
            reject_kg: 0.0,
            overflow_kg: 0.0,
            cst_energy_j: 0.0,
            withdrawal_deficit_kg: 0.0,
            relief_kg: 0.0,
            relief_energy_j: 0.0,
        };
        c.set_temperature(CW_INLET_C);
        c
    }

    /// Set the node temperature directly (scenario initialisation), keeping
    /// `hotwell_mass` as the liquid inventory and filling the shell with
    /// saturated vapour at that temperature.
    pub fn set_temperature(&mut self, t: f64) {
        self.t_sat = t;
        self.p_steam = sat_t(t).p;
        self.vapour_mass = sat_t(t).rho_g * CONDENSER_VOLUME;
        self.energy_j = condenser_energy(t, self.inventory_kg());
    }

    /// Water held by the node (hotwell liquid plus shell vapour), kg.
    pub fn inventory_kg(&self) -> f64 {
        self.hotwell_mass + self.vapour_mass
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
        (self.cw_conductance() * (t - CW_INLET_C)).max(0.0)
    }

    /// `m_cw cp eff`, W/K.
    fn cw_conductance(&self) -> f64 {
        let m_cw = self.cw_flow();
        if m_cw <= 0.0 {
            return 0.0;
        }
        let mcp = m_cw * CP_WATER;
        mcp * (1.0 - (-CONDENSER_UA / mcp).exp())
    }

    /// Stored energy minus the equilibrium energy of the reported temperature
    /// and inventory, J: zero up to the temperature solve's roundoff for every
    /// accepted state.
    pub fn equilibrium_residual_j(&self) -> f64 {
        self.energy_j - condenser_energy(self.t_sat, self.inventory_kg())
    }

    /// Heat the circulating water removes over a step, J, from start
    /// temperature `t0` to the node holding `mass` and, before CW, `energy`.
    /// The non-CW heating is spread uniformly over the step and the path is
    /// integrated exactly (`cw_trajectory`) with the node heat capacity taken as
    /// the secant over the temperature change actually made, so the CW duty and
    /// end temperature agree for any step length.
    fn cw_heat_over_step(&self, dt: f64, t0: f64, energy: f64, mass: f64, t_cap: f64) -> f64 {
        let k = self.cw_conductance();
        if k <= 0.0 {
            return 0.0;
        }
        let e0 = condenser_energy(t0, mass);
        let mut cap = condenser_energy(t0 + 1.0, mass) - e0;
        let mut q = 0.0;
        for _ in 0..30 {
            let (t_end, integral) = cw_trajectory(t0, (energy - e0) / (cap * dt), k / cap, dt, CW_INLET_C, t_cap);
            let q_new = k * integral;
            let converged = (q_new - q).abs() <= 1e-13 * q_new.abs() + 1e-6;
            q = q_new;
            let t_end = t_end.clamp(CONDENSER_T_MIN, CONDENSER_T_MAX);
            if converged || (t_end - t0).abs() < 1e-9 {
                break;
            }
            cap = (condenser_energy(t_end, mass) - e0) / (t_end - t0);
        }
        q
    }

    /// Makeup rate the level controller runs this step, kg/s.
    fn makeup_rate(&self) -> f64 {
        if self.hotwell_mass / HOTWELL_CAPACITY < HOTWELL_MAKEUP_FRAC {
            HOTWELL_TRANSFER_RATE
        } else {
            0.0
        }
    }

    /// Condensate withdrawal the hotwell can supply over the next step of
    /// length `dt` from its current liquid plus makeup, kg/s, before counting
    /// any of this step's returns. A caller that bounds its feed demand by this
    /// before integrating the receiving vessel gets `w_out_accepted` equal to
    /// that bounded demand from `step_energy`, so both ends see the same flow.
    pub fn withdrawal_limit(&self, dt: f64) -> f64 {
        if dt <= 0.0 {
            return 0.0;
        }
        (self.hotwell_mass / dt + self.makeup_rate()).max(0.0)
    }

    /// Advance with `q` the heat to reject (W) net of the hotwell enthalpy of the
    /// returned mass, `w_in` water returned to the hotwell and `w_out` condensate
    /// pumped out, kg/s. Kept for callers that already subtract `w_in * h(hotwell)`;
    /// `step_energy` takes the gross enthalpy flow instead.
    pub fn step(&mut self, dt: f64, q: f64, w_in: f64, w_out: f64) -> Result<(), String> {
        let q_in = q + w_in * steam::h_liquid(self.t_sat);
        self.step_energy(dt, q_in, w_in, w_out)
    }

    /// Advance the node: `q_in_w` is the enthalpy flow carried into the shell by
    /// the returned mass `w_in` (kg/s), net of any turbine work and extraction
    /// already taken from that steam; `w_out` is the condensate withdrawal demand,
    /// taken at the start-of-step hotwell enthalpy (the enthalpy the feedwater
    /// train is given for the same step).
    ///
    /// Order: returns, makeup, CW and the withdrawal change the conserved mass and
    /// energy; the node then re-equilibrates (condensing or evaporating); the
    /// relief vents saturated steam if the shell is above its set pressure; level
    /// reject and overflow finally remove liquid at the end temperature, which
    /// leaves that temperature unchanged.
    ///
    /// The step is transactional. It returns an error and leaves the node
    /// unchanged when the end state is one the wet equilibrium node cannot
    /// represent: a shell with too little water to fill it with saturated vapour
    /// (including relief that would vent more than the liquid can evaporate) or
    /// a temperature outside 1..200 degC. The caller must roll its step back.
    pub fn step_energy(&mut self, dt: f64, q_in_w: f64, w_in: f64, w_out: f64) -> Result<(), String> {
        let mut next = self.clone();
        next.advance(dt, q_in_w, w_in, w_out)?;
        *self = next;
        Ok(())
    }

    fn advance(&mut self, dt: f64, q_in_w: f64, w_in: f64, w_out: f64) -> Result<(), String> {
        let t0 = self.t_sat;
        let h_hw0 = steam::h_liquid(t0);
        let h_cst = steam::h_liquid(CST_TEMP_C);
        let frac0 = self.hotwell_mass / HOTWELL_CAPACITY;

        // Returns and makeup, then the withdrawal bounded by the liquid that is there.
        self.w_makeup = self.makeup_rate();
        let mut mass = self.inventory_kg() + (w_in.max(0.0) + self.w_makeup) * dt;
        let liquid = (mass - self.vapour_mass).max(0.0);
        self.w_out_accepted = w_out.max(0.0).min(liquid / dt);
        self.w_out_shortfall = w_out.max(0.0) - self.w_out_accepted;
        mass -= self.w_out_accepted * dt;

        self.q_in_w = q_in_w;
        self.heat_load = q_in_w;
        let mut energy = self.energy_j + (q_in_w + self.w_makeup * h_cst - self.w_out_accepted * h_hw0) * dt;
        let t_relief = sat_p(CONDENSER_RELIEF_P).t;
        let q_cw_j = self.cw_heat_over_step(dt, t0, energy, mass, t_relief);
        self.q_cw_w = q_cw_j / dt;
        energy -= q_cw_j;

        // Shell relief: vent saturated steam until the node is back at the relief
        // temperature. While liquid remains at T_r, removing m kg changes the
        // equilibrium energy by m h_f, so E - m h_g = E_eq(M, T_r) - m h_f gives
        // the vented mass. That identity needs liquid left after the vent: relief
        // that would evaporate more than the liquid present boils the shell dry.
        self.w_relief = 0.0;
        self.q_relief_w = 0.0;
        let e_relief = condenser_energy(t_relief, mass);
        if energy > e_relief {
            let sat_r = sat_t(t_relief);
            let m_relief = (energy - e_relief) / sat_r.h_fg();
            let liquid_r = mass - sat_r.rho_g * CONDENSER_VOLUME;
            if m_relief > liquid_r {
                return Err(format!(
                    "condenser shell boils dry under relief: {m_relief:.1} kg to vent, {:.1} kg of liquid",
                    liquid_r.max(0.0)
                ));
            }
            self.w_relief = m_relief / dt;
            self.q_relief_w = self.w_relief * sat_r.h_g;
            mass -= m_relief;
            energy -= m_relief * sat_r.h_g;
        }
        self.relief_kg += self.w_relief * dt;
        self.relief_energy_j += self.q_relief_w * dt;

        // Re-solve the equilibrium of what is left and reject unrepresentable states.
        let t = condenser_temperature(energy, mass);
        if t <= CONDENSER_T_MIN || t >= CONDENSER_T_MAX {
            return Err(format!("condenser temperature outside its {CONDENSER_T_MIN}..{CONDENSER_T_MAX} degC domain"));
        }
        let m_vsat = sat_t(t).rho_g * CONDENSER_VOLUME;
        if mass < m_vsat * (1.0 - 1e-9) {
            return Err(format!("condenser shell dry: {mass:.1} kg of water, {m_vsat:.1} kg needed to fill it at {t:.2} degC"));
        }
        self.vapour_mass = m_vsat.min(mass);
        let mut liquid = mass - self.vapour_mass;

        // Level control and overflow to the CST: liquid leaves at h_f(t), which
        // keeps the equilibrium temperature unchanged.
        let h_hw = steam::h_liquid(t);
        self.w_reject = if frac0 > HOTWELL_REJECT_FRAC { HOTWELL_TRANSFER_RATE.min(liquid / dt) } else { 0.0 };
        liquid -= self.w_reject * dt;
        self.w_overflow = ((liquid - HOTWELL_CAPACITY) / dt).max(0.0);
        liquid -= self.w_overflow * dt;
        energy -= (self.w_reject + self.w_overflow) * h_hw * dt;
        self.q_cst_w = self.w_makeup * h_cst - (self.w_reject + self.w_overflow) * h_hw;

        self.hotwell_mass = liquid;
        self.energy_j = energy;
        self.t_sat = t;
        self.p_steam = sat_t(t).p;
        self.makeup_kg += self.w_makeup * dt;
        self.reject_kg += self.w_reject * dt;
        self.overflow_kg += self.w_overflow * dt;
        self.cst_energy_j += self.q_cst_w * dt;
        self.withdrawal_deficit_kg += self.w_out_shortfall * dt;

        let p = self.pressure();
        let leak = AIR_INLEAKAGE * ((P_ATM - p) / P_ATM).clamp(0.0, 1.0);
        let removal = if self.vacuum_pump { self.m_air / VACUUM_PUMP_TAU } else { 0.0 };
        self.m_air = (self.m_air + (leak - removal) * dt).max(0.0);
        // Air cannot exceed atmospheric partial pressure (relief to atmosphere).
        let max_air = (P_ATM - self.p_steam).max(0.0) * CONDENSER_VOLUME / (287.0 * 303.0);
        self.m_air = self.m_air.min(max_air.max(0.0));
        Ok(())
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
    /// Feedwater flow, kg/s: the pump hydraulic flow after `step_hydraulics`, and
    /// the flow actually delivered (after any hotwell limit) after `step_heater`.
    pub flow: f64,
    /// Specific enthalpy of the feedwater delivered this step, J/kg: the heater
    /// outlet at the start of the step. The heater energy balance debits exactly
    /// `flow * h_feed_delivered`, so the vessel feed port must use this value.
    pub h_feed_delivered: f64,
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
            h_feed_delivered: steam::h_liquid(t_feed),
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
        // Every turning shaft develops head by the affinity law, energised or
        // coasting; a pump's check valve closes once its shutoff head falls below
        // the header differential (`pump_curve_flow` returns zero there).
        let speeds: Vec<f64> = self.pump_speed.iter().copied().filter(|&s| s > 0.0).collect();

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
            .pump_speed
            .iter()
            .map(|&s| if s > 0.0 { Self::pump_curve_flow(s, dp) } else { 0.0 })
            .collect();
        let sum: f64 = raw.iter().sum();
        if sum > 0.0 {
            for (pf, r) in self.pump_flow.iter_mut().zip(raw.iter()) {
                *pf = r / sum * q;
            }
        }
        q
    }

    /// Level controller, pump drives, hydraulics and heater energy balance in one
    /// call, with the extraction bounded by `turbine_flow` and the whole pump flow
    /// delivered. A coupled plant step should call `step_hydraulics` before the
    /// vessel and `step_heater` after it, with the delivered flows.
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
        let msg = self.step_hydraulics(dt, p_reactor, level_cm, steam_flow, hotwell_ok);
        self.step_heater(dt, self.flow, turbine_flow, hotwell_t);
        msg
    }

    /// Level controller, pump drives and hydraulics: sets `flow` to the pump flow
    /// demand on the hotwell and fixes `h_feed_delivered` for this step.
    /// Returns any pump trip message.
    pub fn step_hydraulics(
        &mut self,
        dt: f64,
        p_reactor: f64,
        level_cm: f64,
        steam_flow: f64,
        hotwell_ok: bool,
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
        self.h_feed_delivered = steam::h_liquid(self.t_feed);
        msg
    }

    /// Feedwater heater node over the step, on the flows that were actually
    /// delivered: `w_feed` (kg/s, at most the hydraulic flow) leaves at
    /// `h_feed_delivered`, and extraction heat is bounded by `turbine_flow`, the
    /// steam the vessel actually delivered to the turbine (kg/s). Extraction steam
    /// is the only heat source; the stored energy drains into the feedwater once
    /// that steam is gone.
    pub fn step_heater(&mut self, dt: f64, w_feed: f64, turbine_flow: f64, hotwell_t: f64) {
        self.step_heater_capped(dt, w_feed, turbine_flow, f64::INFINITY, hotwell_t);
    }

    /// As `step_heater`, with `turbine_flow` the vapour the turbine expanded and
    /// extraction also bounded by `q_budget` (W), the energy that vapour has left
    /// after shaft work (`TurbineInlet::extraction_budget`).
    pub fn step_heater_capped(&mut self, dt: f64, w_feed: f64, turbine_flow: f64, q_budget: f64, hotwell_t: f64) {
        let w_feed = w_feed.clamp(0.0, self.flow);
        if self.flow > 0.0 {
            let scale = w_feed / self.flow;
            for pf in self.pump_flow.iter_mut() {
                *pf *= scale;
            }
        }
        self.flow = w_feed;
        let h_in = steam::h_liquid(hotwell_t);
        let h_out = self.h_feed_delivered;
        let service = if self.heaters_in_service { 1.0 } else { 0.65 };
        let load = (turbine_flow / STEAM_FLOW_RATED).clamp(0.0, 1.2);
        let t_des = hotwell_t + (T_FW_RATED - 35.0) * load.powf(0.75) * service + FW_PUMP_TEMP_RISE;
        let pumped: f64 = self.pump_flow.iter().sum();
        self.q_pump_w = pumped * CP_WATER * FW_PUMP_TEMP_RISE;
        self.q_loss_w = FW_HEATER_LOSS_UA * (self.t_feed - CST_TEMP_C).max(0.0);
        self.q_extraction_avail_w = (turbine_flow.max(0.0) * FW_EXTRACTION_HEAT_PER_KG * service).min(q_budget.max(0.0));
        let q_needed = self.flow * (steam::h_liquid(t_des) - h_in) + FW_HEATER_HEAT_CAP * (t_des - self.t_feed) / 60.0
            - self.q_pump_w
            + self.q_loss_w;
        self.q_extraction_w = q_needed.clamp(0.0, self.q_extraction_avail_w);
        let q_net = self.q_extraction_w + self.q_pump_w - self.q_loss_w - self.flow * (h_out - h_in);
        self.t_feed += q_net * dt / FW_HEATER_HEAT_CAP;
    }

    /// Feedwater enthalpy delivered to the vessel for the current step, J/kg.
    /// This is `h_feed_delivered`, the value the heater balance debits.
    pub fn h_feed(&self) -> f64 {
        self.h_feed_delivered
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

    /// R3: rated saturated steam sees exactly the old steam calibration.
    #[test]
    fn rated_steam_hydraulics_are_unchanged_by_density_scaling() {
        let mut s = SteamSystem::new(true);
        s.tcv_pos = 0.85;
        s.bypass_pos = 0.1;
        s.compute_flows(P_RATED, true);
        // The pre-R3 solve, written out independently.
        let a = C_TCV * 0.85 + C_BYPASS * 0.1;
        let k = STEAM_LINE_DP / (STEAM_FLOW_RATED * STEAM_FLOW_RATED);
        let w = (-1.0 + (1.0 + 4.0 * a * a * k * (P_RATED - P_ATM)).sqrt()) / (2.0 * a * k);
        let p_chest = P_RATED - k * w * w;
        assert!((s.total_flow() - w).abs() <= 1e-9 * w, "{} vs {w}", s.total_flow());
        assert!((s.p_chest - p_chest).abs() <= 1e-9 * p_chest, "{} vs {p_chest}", s.p_chest);
    }

    /// R3 (both reviews' 200 degC water-solid fixture): liquid at 3.834878 MPa,
    /// 866.4 kg/m3, solved against an independent density-scaled calculation.
    /// At the reviewers' 242.48 kg/s the line loss falls from the steam-
    /// calibrated 53.23 kPa to ~2.316 kPa (factor rho_rated/rho_liquid ~ 22.98).
    #[test]
    fn liquid_line_and_valve_resistance_scale_with_inlet_density() {
        let (p, rho) = (3.834878e6, 866.396863);
        let (tcv, byp) = (0.3, 0.2);
        let mut s = SteamSystem::new(true);
        s.tcv_pos = tcv;
        s.bypass_pos = byp;
        s.compute_flows_from(p, steam::h_liquid(200.0), rho, true);
        let rho_rated = sat_p(P_RATED).rho_g;
        assert!((rho_rated - 37.696617).abs() < 0.02 * 37.696617, "table rho_g(rated) {rho_rated}");
        let k = STEAM_LINE_DP / (STEAM_FLOW_RATED * STEAM_FLOW_RATED) * rho_rated / rho;
        assert!((k * 242.481409f64.powi(2) - 2316.2).abs() < 0.02 * 2316.2, "line loss {}", k * 242.481409f64.powi(2));
        let a = (C_TCV * tcv + C_BYPASS * byp) * rho / sat_p(p).rho_g;
        // Independent bisection of k w^2 + w / a = p - P_ATM.
        let (mut lo, mut hi) = (0.0, 1.0e5);
        for _ in 0..200 {
            let mid = 0.5 * (lo + hi);
            if k * mid * mid + mid / a < p - P_ATM { lo = mid } else { hi = mid }
        }
        let w = lo;
        assert!((s.total_flow() - w).abs() <= 1e-9 * w, "{} vs {w}", s.total_flow());
        assert!((s.p_chest - (p - k * w * w)).abs() <= 1e-6 * p, "chest {}", s.p_chest);
        assert!((s.k_line - k).abs() <= 1e-12 * k);
        // A supply-limited commit stays on the same density-scaled relations.
        s.limit_to_delivered(0.5 * w);
        let wd = s.total_flow();
        assert!((wd - 0.5 * w).abs() < 1e-9 * w);
        assert!((wd - a * (s.p_chest - P_ATM)).abs() < 1e-9 * w, "valve law off at delivered flow");
        assert!((s.p_line_inlet - (s.p_chest + k * wd * wd)).abs() < 1e-6, "line law off at delivered flow");
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
            c.step_energy(DT, q, w_in, 0.0).unwrap();
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
            c.step_energy(DT, q_in, w, w).unwrap();
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
            c.step_energy(DT, q_in, STEAM_FLOW_RATED, STEAM_FLOW_RATED).unwrap();
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
        let m0 = c.inventory_kg();
        c.step(1.0, 1.0e6, 100.0, 0.0).unwrap();
        assert_eq!(c.hotwell_mass, HOTWELL_CAPACITY);
        // CW cools the node, so some shell vapour condenses: count the whole inventory.
        let accounted = (c.inventory_kg() - m0) + c.reject_kg + c.overflow_kg;
        assert!((accounted - 100.0).abs() < 1e-9, "accounted {accounted}");
        assert!(c.overflow_kg > 0.0 && c.reject_kg > 0.0);
    }

    #[test]
    fn hotwell_withdrawal_is_bounded_by_inventory() {
        let mut c = Condenser::new(true);
        c.hotwell_mass = 10.0;
        c.set_temperature(35.0);
        c.step(1.0, 0.0, 0.0, 100.0).unwrap();
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

    /// One condenser step's mass and energy residuals against its reported
    /// boundary flows, (kg, J).
    fn condenser_step_residuals(c: &mut Condenser, dt: f64, q_in: f64, w_in: f64, w_out: f64) -> (f64, f64) {
        let (m0, e0, h_hw0) = (c.inventory_kg(), c.energy_j, steam::h_liquid(c.t_sat));
        c.step_energy(dt, q_in, w_in, w_out).unwrap();
        let dm = (w_in + c.w_makeup - c.w_out_accepted - c.w_reject - c.w_overflow - c.w_relief) * dt;
        let de = (q_in - c.q_cw_w - c.w_out_accepted * h_hw0 + c.q_cst_w - c.q_relief_w) * dt;
        (c.inventory_kg() - m0 - dm, c.energy_j - e0 - de)
    }

    #[test]
    fn condenser_without_cw_conserves_mass_and_energy_and_accumulates_vapour() {
        let mut c = Condenser::new(false);
        c.set_temperature(35.0);
        let (w, dt) = (STEAM_FLOW_RATED, 1.0);
        let (v0, p0) = (c.vapour_mass, c.p_steam);
        let (mut max_dm, mut max_de) = (0.0f64, 0.0f64);
        let mut saw_vapour_rise = false;
        // 300 s: relief from about 70 s, ending before the boil-off (~215 kg/s net
        // of makeup) empties the hotwell, which the node rejects as dryout.
        for _ in 0..300 {
            let q_in = 548.0e6 + w * steam::h_liquid(c.t_sat);
            let (dm, de) = condenser_step_residuals(&mut c, dt, q_in, w, w);
            max_dm = max_dm.max(dm.abs());
            max_de = max_de.max(de.abs());
            saw_vapour_rise |= c.w_relief == 0.0 && c.vapour_mass > 5.0 * v0;
        }
        eprintln!("no-CW condenser: max |residual| {max_dm:.3e} kg, {max_de:.3e} J per step");
        // Residuals are floating-point rounding on a ~1e10 J, ~1e5 kg node.
        assert!(max_dm < 1e-6, "mass residual {max_dm} kg per step");
        assert!(max_de < 1e-2, "energy residual {max_de} J per step");
        assert!(saw_vapour_rise, "shell vapour must accumulate before relief");
        assert!(c.p_steam > 10.0 * p0, "pressure {} from {}", c.p_steam, p0);
        assert!(c.relief_kg > 0.0, "relief must export steam mass");
    }

    #[test]
    fn condenser_relief_exports_steam_mass_with_its_enthalpy() {
        let mut c = Condenser::new(false);
        c.set_temperature(sat_p(CONDENSER_RELIEF_P).t);
        let m0 = c.inventory_kg();
        let (dm, de) = condenser_step_residuals(&mut c, 1.0, 100.0e6, 0.0, 0.0);
        assert!(dm.abs() < 1e-6 && de.abs() < 1e-2, "residuals {dm} kg {de} J");
        let h_g = sat_p(CONDENSER_RELIEF_P).h_g;
        let h_fg = sat_p(CONDENSER_RELIEF_P).h_fg();
        // 100 MJ boils off about 100 MJ / h_fg of water, which leaves as steam at h_g.
        assert!((c.relief_kg - 100.0e6 / h_fg).abs() < 0.02 * c.relief_kg, "relief {} kg", c.relief_kg);
        assert!((m0 - c.inventory_kg() - c.relief_kg).abs() < 1e-6);
        assert!((c.relief_energy_j - c.relief_kg * h_g).abs() < 1e-3);
        assert!(c.pressure() <= CONDENSER_RELIEF_P);
    }

    /// Isolated relief depletion (no CW, 1 GW into a node at the relief
    /// temperature, no returns or withdrawal, automatic makeup) at timestep `dt`.
    /// Every accepted state must reconstruct from its own T and M (energy,
    /// saturated vapour filling the shell, saturation pressure) and close its
    /// port balance; the step that would boil the shell dry must be rejected
    /// without changing the node. Returns the time of that rejection, s.
    fn relief_depletion_time(dt: f64) -> f64 {
        let mut c = Condenser::new(false);
        c.set_temperature(sat_p(CONDENSER_RELIEF_P).t);
        let q_in = 1.0e9;
        let mut time = 0.0;
        loop {
            let before = c.clone();
            if let Err(e) = c.step_energy(dt, q_in, 0.0, 0.0) {
                assert!(e.contains("dry"), "{e}");
                assert_eq!((c.energy_j, c.inventory_kg(), c.relief_kg), (before.energy_j, before.inventory_kg(), before.relief_kg));
                return time;
            }
            time += dt;
            let dm = c.inventory_kg() - before.inventory_kg() - (c.w_makeup - c.w_relief) * dt;
            let de = c.energy_j - before.energy_j - (q_in + c.q_cst_w - c.q_relief_w) * dt;
            assert!(dm.abs() < 1e-6 && de.abs() < 1e-2, "port residuals {dm} kg {de} J at {time} s");
            assert!(c.equilibrium_residual_j().abs() < 1e-3, "E - E(T,M) = {} J at {time} s", c.equilibrium_residual_j());
            let sat = sat_t(c.t_sat);
            assert!((c.vapour_mass - sat.rho_g * CONDENSER_VOLUME).abs() < 1e-9 * c.vapour_mass);
            assert!(c.hotwell_mass >= 0.0 && c.p_steam == sat.p && c.pressure() <= CONDENSER_RELIEF_P);
            assert!(time < 1000.0, "the shell must boil dry");
        }
    }

    #[test]
    fn relief_depletion_is_rejected_at_dryout_with_a_closed_state() {
        let (coarse, production, fine) = (relief_depletion_time(1.0), relief_depletion_time(DT), relief_depletion_time(0.01));
        eprintln!("relief dryout at {coarse} s (dt 1), {production} s (dt {DT}), {fine} s (dt 0.01)");
        // About 70 t of liquid boiled off by 1 GW at ~446 kg/s, less 30 kg/s makeup once low.
        assert!(production > 150.0 && production < 200.0, "dryout at {production} s");
        assert!((production - fine).abs() <= 2.0 * DT && (coarse - fine).abs() <= 2.0, "{coarse} {production} {fine}");
    }

    #[test]
    fn relief_with_no_liquid_is_rejected() {
        let mut c = Condenser::new(false);
        c.hotwell_mass = 0.0;
        c.set_temperature(sat_p(CONDENSER_RELIEF_P).t);
        let before = c.clone();
        let e = c.step_energy(DT, 100.0e6, 0.0, 0.0).unwrap_err();
        assert!(e.contains("dry"), "{e}");
        assert_eq!((c.energy_j, c.inventory_kg()), (before.energy_j, before.inventory_kg()));
    }

    #[test]
    fn cw_cooling_is_stable_and_converged_for_large_steps() {
        // Both CW pumps, no load, from 35 degC: one 60 s step must not cool past
        // the CW inlet and must agree with the refined response.
        let end_t = |dt: f64| {
            let mut c = Condenser::new(true);
            c.set_temperature(35.0);
            for _ in 0..(60.0 / dt).round() as usize {
                let e0 = c.energy_j;
                c.step_energy(dt, 0.0, 0.0, 0.0).unwrap();
                assert!(c.t_sat > CW_INLET_C, "cooled to {} below the {CW_INLET_C} degC inlet", c.t_sat);
                assert!((c.energy_j - e0 + c.q_cw_w * dt).abs() < 1e-3, "CW duty must be the whole energy change");
                assert!(c.equilibrium_residual_j().abs() < 1e-3);
            }
            c.t_sat
        };
        let (coarse, mid, fine) = (end_t(60.0), end_t(1.0), end_t(DT));
        eprintln!("CW cooling 35 degC + 60 s: {coarse:.6} (dt 60), {mid:.6} (dt 1), {fine:.6} (dt {DT})");
        assert!(fine < 21.0, "cooling time constant ~13 s, end {fine}");
        assert!((coarse - fine).abs() < 0.01 && (mid - fine).abs() < 0.01, "{coarse} {mid} {fine}");
    }

    #[test]
    fn cw_trajectory_matches_closed_forms() {
        let a = 0.1;
        // Pure decay from 30 to the 20 degC inlet.
        let (t, i) = cw_trajectory(30.0, 0.0, a, 5.0, 20.0, 100.0);
        assert!((t - (20.0 + 10.0 * (-0.5f64).exp())).abs() < 1e-12);
        assert!((i - 10.0 * (1.0 - (-0.5f64).exp()) / a).abs() < 1e-12);
        // Heating below the inlet is linear, then exponential above it.
        let (t, _) = cw_trajectory(10.0, 2.0, a, 10.0, 20.0, 100.0);
        assert!((t - (40.0 - 20.0 * (-0.5f64).exp())).abs() < 1e-12, "t {t}");
        // Strong heating reaches the cap and holds there.
        let (t, i) = cw_trajectory(90.0, 100.0, a, 10.0, 20.0, 100.0);
        assert_eq!(t, 100.0);
        assert!(i > 70.0 * 9.0 && i < 80.0 * 10.0, "integral {i}");
    }

    #[test]
    fn bounded_feed_demand_is_fully_accepted_by_the_hotwell() {
        let mut c = Condenser::new(true);
        c.hotwell_mass = 6000.1;
        c.set_temperature(35.0);
        let dt = 20.0;
        let w_feed = 989.0f64.min(c.withdrawal_limit(dt));
        assert!(w_feed < 989.0, "the limit must bind in this case");
        c.step_energy(dt, 50.0 * steam::h_liquid(35.0), 50.0, w_feed).unwrap();
        assert_eq!(c.w_out_accepted, w_feed);
        assert_eq!(c.w_out_shortfall, 0.0);
        assert!(c.hotwell_mass >= 0.0);
    }

    #[test]
    fn heater_energy_identity_closes_each_step() {
        let mut fw = Feedwater::new(2, T_FW_RATED, LevelControlMode::Manual, 0.7);
        fw.fcv_manual = 0.7;
        let hw = 35.0;
        let h_hw = steam::h_liquid(hw);
        let mut max_res = 0.0f64;
        for k in 0..2400 {
            // Full extraction, then none, then a delivered-feed limit.
            let w_t = if k < 800 { STEAM_FLOW_RATED } else { 0.0 };
            let t0 = fw.t_feed;
            fw.step_hydraulics(DT, P_RATED, 0.0, 0.0, true);
            let w_feed = if k >= 1600 { 0.5 * fw.flow } else { fw.flow };
            fw.step_heater(DT, w_feed, w_t, hw);
            assert_eq!(fw.h_feed(), fw.h_feed_delivered, "vessel and heater must see one enthalpy");
            let stored = FW_HEATER_HEAT_CAP * (fw.t_feed - t0);
            let ports = DT * (fw.q_extraction_w + fw.q_pump_w - fw.q_loss_w - fw.flow * (fw.h_feed_delivered - h_hw));
            max_res = max_res.max((stored - ports).abs());
        }
        eprintln!("heater identity: max |residual| {max_res:.3e} J per step");
        assert!(max_res < 1e-3, "heater residual {max_res} J");
    }

    #[test]
    fn extraction_is_bounded_by_delivered_turbine_flow() {
        let mut fw = Feedwater::new(2, T_FW_RATED, LevelControlMode::Manual, 0.7);
        fw.fcv_manual = 0.7;
        // The governor requests rated steam but the vessel delivers almost none.
        fw.step_hydraulics(DT, P_RATED, 0.0, STEAM_FLOW_RATED, true);
        let delivered = 0.002;
        fw.step_heater(DT, fw.flow, delivered, 35.0);
        assert!(fw.q_extraction_w <= delivered * FW_EXTRACTION_HEAT_PER_KG + 1e-9, "q {}", fw.q_extraction_w);
    }

    #[test]
    fn load_rejection_latches_bypass_fast_open() {
        let mut s = SteamSystem::new(true);
        s.p_chest = 7.0e6;
        s.tcv_pos = 0.87;
        // Full power: regulator demand is rated flow, all of it through the TCVs.
        assert!((s.pressure_demand(P_RATED) - STEAM_FLOW_RATED).abs() < 1e-6);
        // Breaker opens: one scan of fast-open, governor demand collapses, no trip.
        for k in 0..6 {
            s.step_ext(DT, P_RATED, 0.0, true, false, k == 0);
        }
        assert!(s.bypass_pos >= 0.9, "bypass {} at 0.3 s", s.bypass_pos);
        assert!(s.tcv_pos < 0.1, "TCVs fast-close on load rejection, at {}", s.tcv_pos);
        assert_eq!(s.fast_open_latch, 0.0, "the latch spans one stroke only");
    }

    #[test]
    fn coasting_feed_pump_delivers_flow_until_its_check_valve_closes() {
        let mut fw = Feedwater::new(2, T_FW_RATED, LevelControlMode::Manual, 0.7);
        fw.fcv_manual = 0.7;
        for _ in 0..20 {
            fw.step(DT, P_RATED, 0.0, 0.0, 0.0, true, 35.0);
        }
        let total0 = fw.flow;
        fw.pumps[1] = false;
        fw.step(DT, P_RATED, 0.0, 0.0, 0.0, true, 35.0);
        assert!(fw.pump_speed[1] > 0.9, "shaft still near speed: {}", fw.pump_speed[1]);
        assert!(fw.pump_flow[1] > 0.5 * fw.pump_flow[0], "coasting pump flow {}", fw.pump_flow[1]);
        assert!(fw.flow > 0.9 * total0, "no flow step on trip: {} -> {}", total0, fw.flow);
        let mut prev = fw.pump_flow[1];
        let mut closed_at_speed = None;
        for _ in 0..200 {
            fw.step(DT, P_RATED, 0.0, 0.0, 0.0, true, 35.0);
            assert!(fw.pump_flow[1] <= prev + 1e-9, "flow decays with speed");
            prev = fw.pump_flow[1];
            if prev == 0.0 && closed_at_speed.is_none() {
                closed_at_speed = Some(fw.pump_speed[1]);
            }
        }
        // Check valve closes at the shutoff-head crossover, while the shaft still turns.
        let s_close = closed_at_speed.expect("check valve must close");
        assert!(s_close > 0.5, "closed at speed {s_close}");
    }
}
