//! Isolation condenser system: passive decay heat removal and overpressure protection.
//!
//! Each train condenses reactor steam in tubes submerged in a water pool above
//! containment; condensate drains back to the vessel by gravity once the
//! condensate return valve opens. No pumps or AC power are needed.

use crate::params::*;
use crate::steam::sat_p;

/// Heat transfer UA per train, W/K (rated duty at 7.2 MPa with a boiling pool).
fn ua() -> f64 {
    IC_TRAIN_RATED_W / (sat_p(P_RATED).t - 100.0)
}

#[derive(Clone, Debug)]
pub struct IcTrain {
    pub valve_pos: f64,
    pub cmd_open: bool,
    pub heat: f64,
    pub pool_t: f64,
    pub pool_mass: f64,
}

impl IcTrain {
    pub fn new() -> Self {
        Self { valve_pos: 0.0, cmd_open: false, heat: 0.0, pool_t: 40.0, pool_mass: IC_POOL_MASS }
    }

    pub fn pool_level_pct(&self) -> f64 {
        self.pool_mass / IC_POOL_MASS * 100.0
    }

    /// Advance; `t_sat` is the reactor steam temperature. Returns heat removed, W.
    pub fn step(&mut self, dt: f64, t_sat: f64, steam_available: bool) -> f64 {
        let target = if self.cmd_open { 1.0 } else { 0.0 };
        let d = target - self.valve_pos;
        self.valve_pos += d.clamp(-dt / IC_VALVE_STROKE, dt / IC_VALVE_STROKE);

        let coverage = (self.pool_level_pct() / 40.0).clamp(0.0, 1.0);
        let dt_drive = if steam_available { (t_sat - self.pool_t).max(0.0) } else { 0.0 };
        let target_q = ua() * dt_drive * self.valve_pos.sqrt() * coverage;
        // Tubes drain and fill with steam over ~20 s.
        self.heat += (target_q - self.heat) * (dt / 20.0).min(1.0);

        // Pool heats up to boiling, then boils off.
        let q = self.heat * dt;
        if self.pool_t < 100.0 {
            self.pool_t += q / (self.pool_mass.max(1.0) * 4186.0);
        } else {
            self.pool_t = 100.0;
            self.pool_mass = (self.pool_mass - q / 2.257e6).max(0.0);
        }
        // Slow cooling of a hot idle pool by its cooling system.
        if self.heat < 1.0e5 && self.pool_t > 40.0 {
            self.pool_t -= 0.0005 * dt;
        }
        self.heat
    }
}

impl Default for IcTrain {
    fn default() -> Self {
        Self::new()
    }
}
