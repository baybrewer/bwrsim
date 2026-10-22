//! Point reactor kinetics with six delayed neutron groups.

use crate::params::{BETA, BETA_FRAC, GEN_TIME, LAMBDA_DN};
use std::fmt;

/// Largest kinetics substep, s. Resolves the fastest delayed group and keeps the
/// backward-Euler period error small for any sub-prompt-critical state.
pub const MAX_SUBSTEP: f64 = 1.0e-3;
/// Maximum prompt growth per substep, `(rho - beta) / Lambda * h`. The implicit
/// denominator is singular at a growth of 1.0, so this keeps it well positive
/// and the accumulated backward-Euler error at the ~1 % level per e-fold.
pub const PROMPT_GROWTH_PER_SUBSTEP: f64 = 0.02;
/// Populations at or below this cannot support a log-rate (period) estimate;
/// the meter holds its last value until both samples are above the floor.
pub const PERIOD_FLOOR: f64 = 1.0e-12;
/// Smoothing time constant of the period meter, s (log-rate amplifier).
const PERIOD_TAU: f64 = 1.0;

/// Why a kinetics advance could not be completed. The state is always left at
/// the last accepted substep, never at a clamped or extrapolated value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum KineticsError {
    /// Reactivity was NaN or infinite.
    NonFiniteReactivity,
    /// The implicit solve gave a non-positive denominator or a non-finite
    /// population for this substep (`rho`, `h`).
    StepRejected { rho: f64, h: f64 },
    /// The timestep was negative or non-finite, or the source was negative or
    /// non-finite (Sol 20): rejected before any state changes.
    InvalidInput { dt: f64, source: f64 },
}

impl fmt::Display for KineticsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFiniteReactivity => write!(f, "kinetics: non-finite reactivity"),
            Self::StepRejected { rho, h } => {
                write!(f, "kinetics: implicit step rejected (rho = {rho:.5}, h = {h:.2e} s)")
            }
            Self::InvalidInput { dt, source } => {
                write!(f, "kinetics: invalid input (dt = {dt:e} s, source = {source:e})")
            }
        }
    }
}

impl std::error::Error for KineticsError {}

#[derive(Clone, Debug)]
pub struct Kinetics {
    /// Neutron population relative to rated power.
    pub n: f64,
    /// Delayed neutron precursor concentrations, same units as n.
    pub c: [f64; 6],
    /// Inverse period estimate, 1/s. Always finite.
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

    /// Largest substep that keeps prompt growth per substep bounded for `rho`.
    pub fn max_substep(rho: f64) -> f64 {
        let growth_rate = (rho - BETA) / GEN_TIME;
        if growth_rate > 0.0 {
            MAX_SUBSTEP.min(PROMPT_GROWTH_PER_SUBSTEP / growth_rate)
        } else {
            MAX_SUBSTEP
        }
    }

    /// Domain of every public entry point: finite reactivity, a finite
    /// non-negative timestep and a finite non-negative source.
    fn check_inputs(rho: f64, s: f64, dt: f64) -> Result<(), KineticsError> {
        if !rho.is_finite() {
            return Err(KineticsError::NonFiniteReactivity);
        }
        if !(dt.is_finite() && dt >= 0.0 && s.is_finite() && s >= 0.0) {
            return Err(KineticsError::InvalidInput { dt, source: s });
        }
        Ok(())
    }

    /// One backward-Euler substep of `h` seconds with constant reactivity `rho`
    /// (dk/k) and source `s`. The solve is checked: a non-positive denominator
    /// or a non-finite result is rejected without touching the state.
    pub fn step(&mut self, rho: f64, s: f64, h: f64) -> Result<(), KineticsError> {
        Self::check_inputs(rho, s, h)?;
        let n0 = self.n;
        let mut lhs = 1.0 - h * (rho - BETA) / GEN_TIME;
        let mut rhs = n0 + h * s;
        for i in 0..6 {
            let d = 1.0 + h * LAMBDA_DN[i];
            lhs -= h * LAMBDA_DN[i] * h * BETA * BETA_FRAC[i] / GEN_TIME / d;
            rhs += h * LAMBDA_DN[i] * self.c[i] / d;
        }
        if lhs <= 0.0 {
            return Err(KineticsError::StepRejected { rho, h });
        }
        let n1 = rhs / lhs;
        if !n1.is_finite() || n1 < 0.0 {
            return Err(KineticsError::StepRejected { rho, h });
        }
        for i in 0..6 {
            self.c[i] = (self.c[i] + h * BETA * BETA_FRAC[i] / GEN_TIME * n1) / (1.0 + h * LAMBDA_DN[i]);
        }
        self.n = n1;
        // Period meter: only when both samples support a finite logarithm, and
        // smoothed like a real log-rate amplifier.
        if n0 > PERIOD_FLOOR && n1 > PERIOD_FLOOR {
            let ip = (n1 / n0).ln() / h;
            if ip.is_finite() {
                let a = (h / PERIOD_TAU).min(1.0);
                self.inv_period += (ip - self.inv_period) * a;
            }
        }
        Ok(())
    }

    /// Advance by `dt` with constant reactivity, choosing the substep so that
    /// prompt growth per substep stays bounded (see [`Self::max_substep`]).
    /// On error the state is left at the last accepted substep.
    pub fn advance(&mut self, rho: f64, s: f64, dt: f64) -> Result<(), KineticsError> {
        Self::check_inputs(rho, s, dt)?;
        let steps = ((dt / Self::max_substep(rho)).ceil() as usize).max(1);
        let h = dt / steps as f64;
        for _ in 0..steps {
            self.step(rho, s, h)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::SOURCE;

    /// Independent reference: fixed tiny implicit steps, far below the adaptive bound.
    fn reference(rho: f64, dt: f64, h: f64) -> f64 {
        let mut k = Kinetics::equilibrium(1.0);
        let steps = (dt / h).round() as usize;
        for _ in 0..steps {
            k.step(rho, 0.0, dt / steps as f64).unwrap();
        }
        k.n
    }

    #[test]
    fn steady_state_stays_put() {
        let mut k = Kinetics::equilibrium(1.0);
        k.advance(0.0, 0.0, 100.0).unwrap();
        assert!((k.n - 1.0).abs() < 1e-6);
    }

    #[test]
    fn small_step_gives_textbook_period() {
        // +0.1 % dk/k (~17 cents): stable period should be ~57 s for U-235 data.
        let mut k = Kinetics::equilibrium(1.0);
        k.advance(0.001, 0.0, 120.0).unwrap();
        let n1 = k.n;
        k.advance(0.001, 0.0, 10.0).unwrap();
        let period = 10.0 / (k.n / n1).ln();
        assert!(period > 40.0 && period < 75.0, "period {period}");
    }

    #[test]
    fn prompt_drop_on_scram() {
        let mut k = Kinetics::equilibrium(1.0);
        k.advance(-0.05, 0.0, 0.2).unwrap();
        // Prompt drop to roughly beta/(beta - rho) ~ 0.11.
        assert!(k.n > 0.06 && k.n < 0.15, "n = {}", k.n);
    }

    /// Sol 1 / Astra 8: prompt-supercritical growth must be resolved, not
    /// clamped into shutdown. Old code returned n = 1e-14 for this case.
    #[test]
    fn prompt_supercritical_grows_and_matches_reference() {
        let mut k = Kinetics::equilibrium(1.0);
        k.advance(0.05, 0.0, 1.0e-3).unwrap();
        // Prompt-jump estimate: exp(1.1) + (beta/Lambda)/(1100/s) * (exp(1.1) - 1) ~ 3.28.
        let r = reference(0.05, 1.0e-3, 1.0e-8);
        assert!(k.n > 3.0 && k.n < 3.5, "n = {}", k.n);
        assert!((k.n - r).abs() / r < 0.02, "n = {} vs reference {r}", k.n);
        assert!(k.inv_period > 0.0 && k.inv_period.is_finite());
    }

    /// Astra 8: just below the 1 ms singularity the old code exaggerated growth
    /// (46.1 instead of 2.9). The adaptive substep must stay accurate there.
    #[test]
    fn near_singularity_growth_is_accurate() {
        let mut k = Kinetics::equilibrium(1.0);
        k.advance(0.045, 0.0, 1.0e-3).unwrap();
        let r = reference(0.045, 1.0e-3, 1.0e-8);
        assert!((k.n - r).abs() / r < 0.02, "n = {} vs reference {r}", k.n);
    }

    /// A raw substep across the denominator singularity is rejected and leaves
    /// the state untouched instead of clamping to the floor.
    #[test]
    fn implicit_step_rejects_nonpositive_denominator() {
        let mut k = Kinetics::equilibrium(1.0);
        let before = k.clone();
        let err = k.step(0.05, 0.0, 1.0e-3).unwrap_err();
        assert!(matches!(err, KineticsError::StepRejected { .. }));
        assert_eq!(k.n, before.n);
        assert_eq!(k.c, before.c);
        assert_eq!(k.inv_period, before.inv_period);
        // On the safe side of the singularity the same call is accepted.
        assert!(k.step(0.05, 0.0, Kinetics::max_substep(0.05)).is_ok());
    }

    #[test]
    fn non_finite_reactivity_is_rejected() {
        let mut k = Kinetics::equilibrium(1.0);
        assert_eq!(k.advance(f64::NAN, 0.0, 1.0e-3), Err(KineticsError::NonFiniteReactivity));
        assert_eq!(k.n, 1.0);
    }

    /// Sol 20: negative or non-finite time and source are rejected at the
    /// public entry points without touching the state (old code accepted
    /// advance(0, 0, -0.001) and returned n = 0.97).
    #[test]
    fn invalid_timestep_or_source_is_rejected() {
        for (s, dt) in [(0.0, -1.0e-3), (0.0, f64::NAN), (0.0, f64::INFINITY), (-1.0, 1.0e-3), (f64::NAN, 1.0e-3)] {
            let mut k = Kinetics::equilibrium(1.0);
            let before = k.clone();
            assert!(matches!(k.advance(1.0e-3, s, dt), Err(KineticsError::InvalidInput { .. })), "s={s} dt={dt}");
            assert!(matches!(k.step(1.0e-3, s, dt), Err(KineticsError::InvalidInput { .. })), "s={s} dt={dt}");
            assert_eq!(k.n, before.n);
            assert_eq!(k.c, before.c);
        }
    }

    /// Sol 19 / Astra 25: a source start from zero population keeps a finite
    /// period meter. Old code produced inv_period = NaN permanently.
    #[test]
    fn zero_population_source_start_keeps_period_finite() {
        let mut k = Kinetics::equilibrium(0.0);
        k.advance(-0.07, SOURCE, 0.05).unwrap();
        assert!(k.n > 0.0 && k.n.is_finite());
        assert!(k.inv_period.is_finite(), "inv_period = {}", k.inv_period);
        // Once above the floor the meter responds normally.
        k.advance(-0.07, SOURCE, 5.0).unwrap();
        assert!(k.inv_period.is_finite());
        // Zero population with zero source stays at zero without poisoning the meter.
        let mut z = Kinetics::equilibrium(0.0);
        z.advance(-0.07, 0.0, 1.0).unwrap();
        assert_eq!(z.n, 0.0);
        assert!(z.inv_period.is_finite());
    }
}
