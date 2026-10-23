//! Isolation condenser system: passive decay heat removal and overpressure protection.
//!
//! Each train condenses reactor steam in tubes submerged in a water pool above
//! containment; condensate drains back to the vessel by gravity once the
//! condensate return valve opens. No pumps or AC power are needed.

use crate::params::*;
use serde::{Deserialize, Serialize};
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
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
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

/// Transfers one train asks of the vessel for one step (see `IcTrain::request`).
/// Rates in W and kg/s.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct IcRequest {
    /// Valve position at the end of the step (the actuator moves regardless
    /// of steam supply).
    pub valve_pos: f64,
    /// Duty delivered to the pool, split into sensible and latent parts.
    pub q_pool: f64,
    pub q_sensible: f64,
    pub q_latent: f64,
    /// Sensible heating rate that brings the pool to boiling this step.
    pub sensible_cap_w: f64,
    /// Lagged duty the tubes could not deliver to the pool.
    pub q_rejected: f64,
    pub w_steam_in: f64,
    pub h_steam_in: f64,
    /// Enthalpy the condensed steam is stored at.
    pub h_condensate: f64,
    pub w_cond_return: f64,
    pub h_return: f64,
}

impl IcRequest {
    /// The request with steam, condensate return and pool duty scaled by
    /// `ratio` (clamped to 0..=1); the pool budget is re-split so sensible
    /// heating still precedes boil-off.
    pub fn scaled(&self, ratio: f64) -> Self {
        let r = if ratio.is_finite() { ratio.clamp(0.0, 1.0) } else { 0.0 };
        let q_pool = self.q_pool * r;
        let q_sensible = q_pool.min(self.sensible_cap_w);
        Self {
            q_pool,
            q_sensible,
            q_latent: q_pool - q_sensible,
            q_rejected: self.q_rejected * r,
            w_steam_in: self.w_steam_in * r,
            w_cond_return: self.w_cond_return * r,
            ..*self
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
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
    /// energy sink; use `request`/`commit` to wire the mass ports instead).
    pub fn step(&mut self, dt: f64, t_sat: f64, steam_available: bool) -> f64 {
        self.step_full(dt, t_sat, steam_available).q_pool
    }

    /// Request then commit with the whole request accepted. Only valid when
    /// the vessel is known to supply the full steam request.
    pub fn step_full(&mut self, dt: f64, t_sat: f64, steam_available: bool) -> IcStep {
        let req = self.request(dt, t_sat, steam_available);
        self.commit(dt, req)
    }

    /// Transfers this train would make over `dt` if the vessel supplied all
    /// the steam it asks for. Pure: nothing changes until `commit`.
    /// `t_sat` is the condensing (vapour saturation) temperature; it is not
    /// derived from the total vessel pressure, which may include
    /// noncondensable gas.
    pub fn request(&self, dt: f64, t_sat: f64, steam_available: bool) -> IcRequest {
        let target = if self.cmd_open { 1.0 } else { 0.0 };
        let d = target - self.valve_pos;
        let valve_pos = self.valve_pos + d.clamp(-dt / IC_VALVE_STROKE, dt / IC_VALVE_STROKE);

        let s = sat_t(t_sat);
        let coverage = (self.pool_level_pct() / 40.0).clamp(0.0, 1.0);
        let dt_drive = if steam_available { (t_sat - self.pool_t).max(0.0) } else { 0.0 };
        let target_q = ua() * dt_drive * valve_pos.sqrt() * coverage;
        // Tubes drain and fill with steam over ~20 s; no steam means no duty at all.
        // The exchanger is algebraic (no tube-metal energy store), so the lagged
        // duty is condensation of current primary steam and may never exceed what
        // the instantaneous primary-to-pool drive can pass through the full tube
        // area: it falls continuously to zero as the primary cools to the pool
        // temperature and never moves heat uphill into a hotter pool (R1).
        let q_drive_max = ua() * dt_drive;
        let q_lag = if steam_available { (self.heat + (target_q - self.heat) * (dt / 20.0).min(1.0)).min(q_drive_max) } else { 0.0 };
        // Stored condensate drains through the return valve (standby inventory
        // first); condensation stops once the primary side is flooded.
        let w_return = (valve_pos * self.tube_mass / IC_DRAIN_TAU).min(self.tube_mass / dt);
        let rho_stored = steam::rho_liquid(steam::t_from_h_liquid(self.tube_h));
        let room_m3 = (IC_PRIMARY_VOLUME - (self.tube_mass - w_return * dt) / rho_stored).max(0.0);
        let q_max_primary = room_m3 * s.rho_f / dt * s.h_fg();
        let q_req = q_lag.min(q_max_primary).max(0.0) * dt;

        // Pool energy budget: sensible heating to boiling, then boil-off, with
        // the remainder refused (and reported) rather than lost.
        let sensible_cap = self.pool_mass * POOL_CP * (POOL_T_BOIL - self.pool_t).max(0.0);
        let sensible = q_req.min(sensible_cap);
        let dm = ((q_req - sensible) / POOL_H_FG).min(self.pool_mass);
        let latent = dm * POOL_H_FG;
        let accepted = sensible + latent;
        let q_pool = accepted / dt;

        IcRequest {
            valve_pos,
            q_pool,
            q_sensible: sensible / dt,
            q_latent: latent / dt,
            sensible_cap_w: sensible_cap / dt,
            q_rejected: (q_req - accepted) / dt,
            w_steam_in: if steam_available { q_pool / s.h_fg() } else { 0.0 },
            h_steam_in: s.h_g,
            h_condensate: s.h_f,
            w_cond_return: w_return,
            h_return: self.tube_h,
        }
    }

    /// Apply the transfers the vessel actually accepted (a request scaled by
    /// the vessel's IC acceptance ratio). The only state change of a step.
    pub fn commit(&mut self, dt: f64, applied: IcRequest) -> IcStep {
        self.valve_pos = applied.valve_pos;
        let sensible = applied.q_sensible * dt;
        let dm = applied.q_latent * dt / POOL_H_FG;
        if self.pool_mass > 0.0 {
            self.pool_t += sensible / (self.pool_mass * POOL_CP);
        }
        self.pool_mass = (self.pool_mass - dm).max(0.0);
        self.boiloff_kg += dm;
        self.heat = applied.q_pool;
        // Slow cooling of a hot idle pool by its cooling system.
        if self.heat < 1.0e5 && self.pool_t > IC_STANDBY_T {
            self.pool_t -= 0.0005 * dt;
        }

        // Primary side: condensed steam collects in the tubes and drains back.
        let (w_cond, w_return) = (applied.w_steam_in, applied.w_cond_return);
        let m_new = self.tube_mass + (w_cond - w_return) * dt;
        if m_new > 0.0 {
            self.tube_h =
                (self.tube_mass * self.tube_h + (w_cond * applied.h_condensate - w_return * applied.h_return) * dt) / m_new;
        }
        self.tube_mass = m_new.max(0.0);

        self.last = IcStep {
            q_pool: applied.q_pool,
            q_sensible: applied.q_sensible,
            q_latent: applied.q_latent,
            boiloff: dm / dt,
            q_rejected: applied.q_rejected,
            w_steam_in: w_cond,
            h_steam_in: applied.h_steam_in,
            w_cond_return: w_return,
            h_return: applied.h_return,
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

    fn snapshot(t: &IcTrain) -> [f64; 5] {
        [t.tube_mass, t.tube_h, t.pool_mass, t.pool_t, t.boiloff_kg]
    }

    /// Sol N1 / Astra N1: a request is pure and a commit at ratio 0 moves no
    /// steam, condensate or heat.
    #[test]
    fn commit_at_zero_ratio_changes_no_tube_or_pool_state() {
        let mut t = open_train();
        t.heat = IC_TRAIN_RATED_W;
        let before = snapshot(&t);
        let req = t.request(0.05, T_SAT, true);
        assert_eq!(req, t.request(0.05, T_SAT, true));
        assert!(req.w_steam_in > 1.0 && req.w_cond_return > 1.0 && req.q_pool > 1.0e6, "{req:?}");
        let r = t.commit(0.05, req.scaled(0.0));
        assert_eq!(snapshot(&t), before);
        assert_eq!((r.w_steam_in, r.w_cond_return, r.q_pool), (0.0, 0.0, 0.0));
        assert_eq!(t.heat, 0.0);
    }

    #[test]
    fn commit_at_half_ratio_moves_exactly_half() {
        for pool_t in [IC_STANDBY_T, 100.0] {
            let mut full = open_train();
            full.heat = IC_TRAIN_RATED_W;
            full.pool_t = pool_t;
            let mut half = full.clone();
            let dt = 0.05;
            let req = full.request(dt, T_SAT, true);
            let pool_energy = |t: &IcTrain| t.pool_mass * POOL_CP * t.pool_t + t.boiloff_kg * POOL_H_FG;
            let (e0, m0) = (pool_energy(&full), full.tube_mass);
            let rf = full.commit(dt, req);
            let rh = half.commit(dt, req.scaled(0.5));
            let (dq_full, dq_half) = (pool_energy(&full) - e0, pool_energy(&half) - e0);
            assert!((dq_half - 0.5 * dq_full).abs() < 1e-9 * dq_full.abs() + 1e-3, "{dq_half} vs {dq_full}");
            assert!((rh.q_pool - 0.5 * rf.q_pool).abs() < 1e-9 * rf.q_pool);
            assert!((rh.w_steam_in - 0.5 * rf.w_steam_in).abs() < 1e-12);
            assert!((rh.w_cond_return - 0.5 * rf.w_cond_return).abs() < 1e-12);
            let (dm_full, dm_half) = (full.tube_mass - m0, half.tube_mass - m0);
            assert!((dm_half - 0.5 * dm_full).abs() < 1e-9, "{dm_half} vs {dm_full}");
            // Steam in less condensate stored equals the pool duty, at any ratio.
            assert!((rh.w_steam_in * (rh.h_steam_in - req.h_condensate) - rh.q_pool).abs() < 1e-6 * rh.q_pool);
        }
    }

    /// R1 (both round-4 reviews): with no tube-metal energy store, primary
    /// steam no hotter than the pool condenses nothing, whatever the preceding
    /// duty, under timestep refinement.
    #[test]
    fn no_duty_into_an_equal_or_hotter_pool() {
        for dt in [0.05, 0.01] {
            for (t_primary, pool_t) in [(100.0, 100.0), (90.0, 100.0), (99.99, 100.0), (60.0, 80.0)] {
                let mut t = open_train();
                t.heat = 5.0e6;
                t.pool_t = pool_t;
                let req = t.request(dt, t_primary, true);
                assert_eq!((req.q_pool, req.w_steam_in, req.q_rejected), (0.0, 0.0, 0.0), "dt {dt} {t_primary}/{pool_t}: {req:?}");
                let r = t.step_full(dt, t_primary, true);
                assert_eq!(r.q_pool, 0.0);
                assert!(t.pool_t <= pool_t && t.boiloff_kg == 0.0, "pool heated by colder primary");
            }
        }
    }

    /// R1: a cooldown through a 100 degC boiling pool. The duty never exceeds
    /// the instantaneous drive, falls continuously to zero as the primary
    /// reaches the pool temperature and stays zero below it.
    #[test]
    fn cooldown_crossing_pool_temperature_takes_duty_continuously_to_zero() {
        for dt in [0.05, 0.01] {
            let mut t = open_train();
            t.pool_t = 100.0;
            // Lagged duty well above what the shrinking drive can pass.
            t.heat = 5.0e6;
            let (t_start, rate) = (101.0, -0.01);
            let mut prev_q: Option<f64> = None;
            let mut max_jump: f64 = 0.0;
            for i in 0..(200.0 / dt) as usize {
                let t_primary = t_start + rate * i as f64 * dt;
                let drive_max = ua() * (t_primary - t.pool_t).max(0.0);
                let q = t.step_full(dt, t_primary, true).q_pool;
                assert!(q <= drive_max * (1.0 + 1e-12), "dt {dt} T {t_primary}: q {q} > {drive_max}");
                if t_primary <= t.pool_t {
                    assert_eq!(q, 0.0, "dt {dt}: duty {q} W into a pool at {} from {t_primary}", t.pool_t);
                }
                if let Some(p) = prev_q {
                    // After the first step clips the lag to the drive, the duty
                    // follows the drive: no step jumps by more than the drive's own change.
                    if i > 1 {
                        max_jump = max_jump.max((q - p).abs());
                    }
                }
                prev_q = Some(q);
            }
            let drive_step = ua() * rate.abs() * dt;
            assert!(max_jump <= drive_step * (1.0 + 1e-9), "dt {dt}: jump {max_jump} W > drive change {drive_step} W");
            assert_eq!(t.heat, 0.0);
        }
    }
}
