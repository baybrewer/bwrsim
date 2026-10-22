//! Isolation condenser system: passive decay heat removal and overpressure protection.
//!
//! Each train condenses reactor steam in tubes submerged in a water pool above
//! containment; condensate drains back to the vessel by gravity once the
//! condensate return valve opens. No pumps or AC power are needed.

use crate::params::*;
use crate::steam::{self, sat_t};

/// Pool water specific heat, J/kgK.
const POOL_CP: f64 = 4186.0;
/// Pool boiling temperature (open to atmosphere), degC.
const POOL_T_BOIL: f64 = 100.0;
/// Latent heat of the pool boil-off at atmospheric pressure, J/kg.
const POOL_H_FG: f64 = 2.257e6;
/// Primary-side (tube + return line) water volume per train, m3. ASSUMED:
/// bounded estimate of the stored standby condensate (NEDO-34168 credits the
/// return-pipe inventory on initiation). (Candidate for params.rs.)
pub const IC_PRIMARY_VOLUME: f64 = 10.0;
/// Standby temperature of the stored condensate, degC (pool temperature).
pub const IC_STANDBY_T: f64 = 40.0;
/// Drain time constant of the stored condensate through a fully open return valve, s. ASSUMED.
pub const IC_DRAIN_TAU: f64 = 30.0;

/// Heat transfer UA per train, W/K (rated duty at 7.2 MPa with a boiling pool).
fn ua() -> f64 {
    IC_TRAIN_RATED_W / (steam::sat_p(P_RATED).t - 100.0)
}

/// Boundary quantities of one IC step. The vessel loses `w_steam_in` at
/// `h_steam_in` and gains `w_cond_return` at `h_return`; their difference
/// equals `q_pool` plus the change in stored primary-side energy.
#[derive(Clone, Copy, Debug, Default)]
pub struct IcStep {
    /// Heat accepted by the pool (sensible + latent), W.
    pub q_pool: f64,
    pub q_sensible: f64,
    pub q_latent: f64,
    /// Pool boil-off to atmosphere, kg/s.
    pub boiloff: f64,
    /// Lagged duty the tubes could not deliver to the pool, W.
    pub q_rejected: f64,
    pub w_steam_in: f64,
    pub h_steam_in: f64,
    pub w_cond_return: f64,
    pub h_return: f64,
    pub pool_empty: bool,
}

#[derive(Clone, Debug)]
pub struct IcTrain {
    pub valve_pos: f64,
    pub cmd_open: bool,
    /// Duty delivered to the pool in the last step, W.
    pub heat: f64,
    pub pool_t: f64,
    pub pool_mass: f64,
    /// Liquid stored on the primary side (tubes and return line), kg.
    pub tube_mass: f64,
    /// Specific enthalpy of the stored liquid, J/kg.
    pub tube_h: f64,
    /// Integrated pool boil-off, kg.
    pub boiloff_kg: f64,
    pub last: IcStep,
}

impl IcTrain {
    pub fn new() -> Self {
        Self {
            valve_pos: 0.0,
            cmd_open: false,
            heat: 0.0,
            pool_t: IC_STANDBY_T,
            pool_mass: IC_POOL_MASS,
            tube_mass: IC_PRIMARY_VOLUME * steam::rho_liquid(IC_STANDBY_T),
            tube_h: steam::h_liquid(IC_STANDBY_T),
            boiloff_kg: 0.0,
            last: IcStep::default(),
        }
    }

    pub fn pool_level_pct(&self) -> f64 {
        self.pool_mass / IC_POOL_MASS * 100.0
    }

    /// Advance; `t_sat` is the reactor steam temperature. Returns the heat
    /// accepted by the pool, W (the closed-loop approximation of the vessel
    /// energy sink; use `step_full` to wire the mass ports instead).
    pub fn step(&mut self, dt: f64, t_sat: f64, steam_available: bool) -> f64 {
        self.step_full(dt, t_sat, steam_available).q_pool
    }

    /// Advance and report the boundary quantities.
    pub fn step_full(&mut self, dt: f64, t_sat: f64, steam_available: bool) -> IcStep {
        let target = if self.cmd_open { 1.0 } else { 0.0 };
        let d = target - self.valve_pos;
        self.valve_pos += d.clamp(-dt / IC_VALVE_STROKE, dt / IC_VALVE_STROKE);

        let s = sat_t(t_sat);
        let coverage = (self.pool_level_pct() / 40.0).clamp(0.0, 1.0);
        let dt_drive = if steam_available { (t_sat - self.pool_t).max(0.0) } else { 0.0 };
        let target_q = ua() * dt_drive * self.valve_pos.sqrt() * coverage;
        // Tubes drain and fill with steam over ~20 s; no steam means no duty at all.
        let q_lag = if steam_available { self.heat + (target_q - self.heat) * (dt / 20.0).min(1.0) } else { 0.0 };
        // Stored condensate drains through the return valve (standby inventory
        // first); condensation stops once the primary side is flooded.
        let w_return = (self.valve_pos * self.tube_mass / IC_DRAIN_TAU).min(self.tube_mass / dt);
        let h_return = self.tube_h;
        let rho_stored = steam::rho_liquid(steam::t_from_h_liquid(self.tube_h));
        let room_m3 = (IC_PRIMARY_VOLUME - (self.tube_mass - w_return * dt) / rho_stored).max(0.0);
        let q_max_primary = room_m3 * s.rho_f / dt * s.h_fg();
        let q_req = q_lag.min(q_max_primary).max(0.0) * dt;

        // Pool energy budget: sensible heating to boiling, then boil-off, with
        // the remainder refused (and reported) rather than lost.
        let sensible = q_req.min(self.pool_mass * POOL_CP * (POOL_T_BOIL - self.pool_t).max(0.0));
        let dm = ((q_req - sensible) / POOL_H_FG).min(self.pool_mass);
        let latent = dm * POOL_H_FG;
        let accepted = sensible + latent;
        if self.pool_mass > 0.0 {
            self.pool_t += sensible / (self.pool_mass * POOL_CP);
        }
        self.pool_mass -= dm;
        self.boiloff_kg += dm;
        self.heat = accepted / dt;
        // Slow cooling of a hot idle pool by its cooling system.
        if self.heat < 1.0e5 && self.pool_t > IC_STANDBY_T {
            self.pool_t -= 0.0005 * dt;
        }

        // Primary side: condensed steam collects in the tubes and drains back.
        let w_cond = if steam_available { self.heat / s.h_fg() } else { 0.0 };
        let m_new = self.tube_mass + (w_cond - w_return) * dt;
        if m_new > 0.0 {
            self.tube_h = (self.tube_mass * self.tube_h + (w_cond * s.h_f - w_return * h_return) * dt) / m_new;
        }
        self.tube_mass = m_new.max(0.0);

        self.last = IcStep {
            q_pool: self.heat,
            q_sensible: sensible / dt,
            q_latent: latent / dt,
            boiloff: dm / dt,
            q_rejected: (q_req - accepted) / dt,
            w_steam_in: w_cond,
            h_steam_in: s.h_g,
            w_cond_return: w_return,
            h_return,
            pool_empty: self.pool_mass <= 0.0,
        };
        self.last
    }
}

impl Default for IcTrain {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T_SAT: f64 = 287.7;

    fn open_train() -> IcTrain {
        let mut t = IcTrain::new();
        t.cmd_open = true;
        t.valve_pos = 1.0;
        // Standby condensate already drained so the pool tests see full duty.
        t.tube_mass = 500.0;
        t
    }

    #[test]
    fn pool_boiling_onset_conserves_energy() {
        let mut t = open_train();
        t.heat = IC_TRAIN_RATED_W;
        t.pool_t = 99.9999;
        let (m0, t0) = (t.pool_mass, t.pool_t);
        let dt = 0.05;
        let removed: f64 = (0..2).map(|_| t.step(dt, T_SAT, true) * dt).sum();
        let absorbed = m0 * POOL_CP * (t.pool_t - t0) + (m0 - t.pool_mass) * POOL_H_FG;
        assert!((removed - absorbed).abs() < 1.0, "removed {removed} J, pool absorbed {absorbed} J");
        assert!(t.pool_mass < m0 && (t.pool_t - POOL_T_BOIL).abs() < 1e-9);
        assert!(t.last.q_latent > 0.0 && t.last.boiloff > 0.0);
    }

    #[test]
    fn empty_pool_removes_no_heat() {
        let mut t = open_train();
        t.pool_mass = 0.0;
        t.pool_t = 100.0;
        t.heat = 1.0e6;
        let q = t.step(0.05, T_SAT, true);
        assert_eq!(q, 0.0);
        assert!(t.last.pool_empty && t.last.w_steam_in == 0.0);
        assert_eq!(t.heat, 0.0);
    }

    #[test]
    fn depletion_limits_accepted_heat_to_remaining_pool() {
        let mut t = open_train();
        t.pool_mass = 0.001;
        t.pool_t = 100.0;
        t.heat = 1.0e6;
        let dt = 0.05;
        let q = t.step(dt, T_SAT, true);
        assert!(q * dt <= 0.001 * POOL_H_FG + 1e-6, "accepted {} J", q * dt);
        assert!(t.last.q_rejected > 0.0 && t.pool_mass == 0.0);
    }

    #[test]
    fn activation_returns_stored_condensate_and_conserves_primary_mass() {
        let mut t = IcTrain::new();
        let m0 = t.tube_mass;
        assert!(m0 > 9.0e3 && m0 < 11.0e3, "stored condensate {m0} kg");
        t.cmd_open = true;
        let dt = 0.05;
        let mut returned = 0.0;
        let mut net = 0.0;
        let mut h_first = None;
        for _ in 0..(300.0 / dt) as usize {
            let r = t.step_full(dt, T_SAT, true);
            if r.w_cond_return > 0.0 && h_first.is_none() {
                h_first = Some(r.h_return);
            }
            returned += r.w_cond_return * dt;
            net += (r.w_steam_in - r.w_cond_return) * dt;
        }
        assert!(returned > 0.5 * m0, "returned {returned} of {m0} kg");
        assert!((m0 + net - t.tube_mass).abs() < 1e-6 * m0, "primary mass drift {}", m0 + net - t.tube_mass);
        // The first condensate back is the cold standby inventory.
        assert!((h_first.unwrap() - steam::h_liquid(IC_STANDBY_T)).abs() < 1.0e3);
        // Once drained, the return runs at saturated-liquid enthalpy and matches condensation.
        assert!((t.last.h_return - sat_t(T_SAT).h_f).abs() < 0.05 * sat_t(T_SAT).h_f, "h_return {}", t.last.h_return);
        assert!(t.last.w_steam_in > 10.0 && (t.last.w_cond_return - t.last.w_steam_in).abs() < 0.1 * t.last.w_steam_in);
        // Net vessel energy loss through the ports equals the pool duty at steady state.
        let q_ports = t.last.w_steam_in * t.last.h_steam_in - t.last.w_cond_return * t.last.h_return;
        assert!((q_ports - t.last.q_pool).abs() < 0.1 * t.last.q_pool, "ports {q_ports} vs pool {}", t.last.q_pool);
    }
}
