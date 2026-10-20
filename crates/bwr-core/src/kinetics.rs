//! Point reactor kinetics with six delayed neutron groups.

use crate::params::{BETA, BETA_FRAC, GEN_TIME, LAMBDA_DN};

#[derive(Clone, Debug)]
pub struct Kinetics {
    /// Neutron population relative to rated power.
    pub n: f64,
    /// Delayed neutron precursor concentrations, same units as n.
    pub c: [f64; 6],
    /// Inverse period estimate, 1/s.
    pub inv_period: f64,
}

impl Kinetics {
    pub fn equilibrium(n: f64) -> Self {
        let mut c = [0.0; 6];
        for i in 0..6 {
            c[i] = BETA * BETA_FRAC[i] * n / (GEN_TIME * LAMBDA_DN[i]);
        }
        Self { n, c, inv_period: 0.0 }
    }

    /// Advance by `dt` with constant reactivity `rho` (dk/k) and source `s`
    /// using backward Euler (stable for any sub-prompt-critical step).
    pub fn step(&mut self, rho: f64, s: f64, dt: f64) {
        let n0 = self.n;
        let mut lhs = 1.0 - dt * (rho - BETA) / GEN_TIME;
        let mut rhs = n0 + dt * s;
        for i in 0..6 {
            let d = 1.0 + dt * LAMBDA_DN[i];
            lhs -= dt * LAMBDA_DN[i] * dt * BETA * BETA_FRAC[i] / GEN_TIME / d;
            rhs += dt * LAMBDA_DN[i] * self.c[i] / d;
        }
        let n1 = (rhs / lhs).max(1e-14);
        for i in 0..6 {
            self.c[i] = (self.c[i] + dt * BETA * BETA_FRAC[i] / GEN_TIME * n1) / (1.0 + dt * LAMBDA_DN[i]);
        }
        self.n = n1;
        let ip = (n1 / n0).ln() / dt;
        // Smooth the period meter like a real log-rate amplifier.
        let a = (dt / 1.0).min(1.0);
        self.inv_period += (ip - self.inv_period) * a;
    }

    /// Advance with `steps` equal substeps.
    pub fn advance(&mut self, rho: f64, s: f64, dt: f64, steps: usize) {
        let h = dt / steps as f64;
        for _ in 0..steps {
            self.step(rho, s, h);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steady_state_stays_put() {
        let mut k = Kinetics::equilibrium(1.0);
        k.advance(0.0, 0.0, 100.0, 100_000);
        assert!((k.n - 1.0).abs() < 1e-6);
    }

    #[test]
    fn small_step_gives_textbook_period() {
        // +0.1 % dk/k (~17 cents): stable period should be ~57 s for U-235 data.
        let mut k = Kinetics::equilibrium(1.0);
        k.advance(0.001, 0.0, 120.0, 120_000);
        let n1 = k.n;
        k.advance(0.001, 0.0, 10.0, 10_000);
        let period = 10.0 / (k.n / n1).ln();
        assert!(period > 40.0 && period < 75.0, "period {period}");
    }

    #[test]
    fn prompt_drop_on_scram() {
        let mut k = Kinetics::equilibrium(1.0);
        k.advance(-0.05, 0.0, 0.2, 200);
        // Prompt drop to roughly beta/(beta - rho) ~ 0.11.
        assert!(k.n > 0.06 && k.n < 0.15, "n = {}", k.n);
    }
}
