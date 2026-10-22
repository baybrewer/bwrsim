//! Reactor core: control rods, reactivity feedback, xenon, decay heat and fuel temperature.

use crate::kinetics::{Kinetics, KineticsError, MAX_SUBSTEP};
use crate::params::*;

/// Grid size of the rod map.
pub const ROD_GRID: usize = 9;

/// Full-stroke insertion time of a Select Rod Insert run-in, s. ASSUMED: an
/// electric motor run-in at FMCRD drive speed rather than the hydraulic scram
/// (SCRAM_TIME). With the four-rod SRI group this gives a coupled power
/// reduction near the ~0.8 percentage point/s quoted for SCRRI in GE 005N9751
/// Rev J section 3.2. TODO(params): migrate to params.rs next to SCRAM_TIME.
pub const SRI_INSERT_TIME: f64 = 120.0;

/// Rods closer than this to a target or to full-in count as there.
const ROD_POS_EPS: f64 = 1e-6;
/// A rod whose position or target is above this counts as withdrawn.
const ROD_OUT_EPS: f64 = 0.01;

/// Rod coordinates on a 9x9 grid: every cell within radius sqrt(17) of the centre (57 cells).
pub fn rod_layout() -> Vec<(usize, usize)> {
    let mut v = Vec::with_capacity(N_RODS);
    for row in 0..ROD_GRID {
        for col in 0..ROD_GRID {
            let dx = col as i32 - 4;
            let dy = row as i32 - 4;
            if dx * dx + dy * dy <= 17 {
                v.push((col, row));
            }
        }
    }
    debug_assert_eq!(v.len(), N_RODS);
    v
}

/// Relative worth of each rod (sums to 1), weighted by flux^2 ~ J0(2.405 r/R)^2.
fn rod_weights(layout: &[(usize, usize)]) -> Vec<f64> {
    let r_ext = 5.3;
    let w: Vec<f64> = layout
        .iter()
        .map(|&(c, r)| {
            let dx = c as f64 - 4.0;
            let dy = r as f64 - 4.0;
            let x = 2.405 * (dx * dx + dy * dy).sqrt() / r_ext;
            let j0 = bessel_j0(x);
            j0 * j0
        })
        .collect();
    let s: f64 = w.iter().sum();
    w.into_iter().map(|x| x / s).collect()
}

fn bessel_j0(x: f64) -> f64 {
    // Series is fine for 0 <= x <= 2.405.
    let mut sum = 0.0;
    let mut term = 1.0;
    for k in 0..20 {
        if k > 0 {
            term *= -(x * x / 4.0) / (k as f64 * k as f64);
        }
        sum += term;
    }
    sum
}

/// Integral rod worth for inserted fraction f (0..1), cosine flux shape.
fn s_curve(f: f64) -> f64 {
    let f = f.clamp(0.0, 1.0);
    f - (2.0 * std::f64::consts::PI * f).sin() / (2.0 * std::f64::consts::PI)
}

#[derive(Clone, Debug)]
pub struct Rods {
    pub layout: Vec<(usize, usize)>,
    pub weights: Vec<f64>,
    pub pos: Vec<f64>,
    pub target: Vec<f64>,
    pub selected: Option<usize>,
    /// Hydraulic scram in progress (rods driving in fast).
    pub scramming: bool,
    /// Rods being hydraulically inserted by Select Rod Insert.
    pub fast_insert: Vec<bool>,
}

impl Rods {
    pub fn new(pos: f64) -> Self {
        let layout = rod_layout();
        let weights = rod_weights(&layout);
        Self {
            layout,
            weights,
            pos: vec![pos; N_RODS],
            target: vec![pos; N_RODS],
            selected: None,
            scramming: false,
            fast_insert: vec![false; N_RODS],
        }
    }

    pub fn reactivity(&self) -> f64 {
        let inserted: f64 = self
            .pos
            .iter()
            .zip(&self.weights)
            .map(|(p, w)| w * s_curve((ROD_FULL_OUT - p) / ROD_FULL_OUT))
            .sum();
        -ROD_WORTH_TOTAL * inserted
    }

    pub fn all_in(&self) -> bool {
        self.pos.iter().all(|&p| p <= ROD_OUT_EPS)
    }

    pub fn moving(&self) -> bool {
        self.pos.iter().zip(&self.target).any(|(p, t)| (p - t).abs() > ROD_POS_EPS)
    }

    /// Rods commanded outward (target above position).
    pub fn n_withdrawing(&self) -> usize {
        self.pos.iter().zip(&self.target).filter(|(p, t)| **t > **p + ROD_POS_EPS).count()
    }

    /// Rods withdrawn or commanded to withdraw (position or pending target out).
    pub fn n_out(&self) -> usize {
        self.pos.iter().zip(&self.target).filter(|(p, t)| **p > ROD_OUT_EPS || **t > ROD_OUT_EPS).count()
    }

    /// Rods that would be out if `indices` were commanded to `target`: the union
    /// of rods already out, rods with a pending outward target, and the proposed
    /// batch. Lets REFUEL's one-rod interlock validate a whole command atomically.
    pub fn n_out_after(&self, indices: &[usize], target: f64) -> usize {
        (0..self.pos.len())
            .filter(|&i| {
                if indices.contains(&i) {
                    target > ROD_OUT_EPS || self.pos[i] > ROD_OUT_EPS
                } else {
                    self.pos[i] > ROD_OUT_EPS || self.target[i] > ROD_OUT_EPS
                }
            })
            .count()
    }

    /// Latch a hydraulic scram demand.
    pub fn scram(&mut self) {
        self.scramming = true;
    }

    /// Rods are actually driving in under a scram demand. Stays false once all
    /// rods are in, however often the demand is re-asserted.
    pub fn scram_in_progress(&self) -> bool {
        self.scramming && !self.all_in()
    }

    /// Move rods for `dt`. Returns true if any position changed.
    pub fn step(&mut self, dt: f64) -> bool {
        let mut moved = false;
        if self.scramming {
            let v = ROD_FULL_OUT / SCRAM_TIME * dt;
            for (p, t) in self.pos.iter_mut().zip(self.target.iter_mut()) {
                let next = (*p - v).max(0.0);
                moved |= next != *p;
                *p = next;
                *t = 0.0;
            }
            if self.all_in() {
                self.scramming = false;
            }
            return moved;
        }
        let v = ROD_SPEED * dt;
        let vf = ROD_FULL_OUT / SRI_INSERT_TIME * dt;
        for i in 0..self.pos.len() {
            if self.fast_insert[i] {
                self.target[i] = 0.0;
                let next = (self.pos[i] - vf).max(0.0);
                moved |= next != self.pos[i];
                self.pos[i] = next;
                if self.pos[i] <= 0.0 {
                    self.fast_insert[i] = false;
                }
                continue;
            }
            let d = self.target[i] - self.pos[i];
            if d != 0.0 {
                self.pos[i] += d.clamp(-v, v);
                moved = true;
            }
        }
        moved
    }

    /// Rods in the Select Rod Insert group: four symmetric rods worth ~1.4 % dk/k.
    pub fn sri_group(&self) -> Vec<usize> {
        (0..self.layout.len())
            .filter(|&i| {
                let (c, r) = self.layout[i];
                (c as i32 - 4).abs() == 2 && (r as i32 - 4).abs() == 2
            })
            .collect()
    }

    /// Hydraulically insert the Select Rod Insert group.
    pub fn select_rod_insert(&mut self) {
        for i in self.sri_group() {
            self.fast_insert[i] = true;
        }
    }

    /// Halt normal drive motion at the current position (FMCRDs are fine-motion
    /// drives; no notch quantisation). Hydraulic scram and SRI are unaffected.
    pub fn stop(&mut self) {
        if self.scramming {
            return;
        }
        for i in 0..self.pos.len() {
            if !self.fast_insert[i] {
                self.target[i] = self.pos[i];
            }
        }
    }
}

/// Xenon-135 / iodine-135, normalised to 100 % power equilibrium.
#[derive(Clone, Debug)]
pub struct Xenon {
    pub iodine: f64,
    pub xenon: f64,
}

impl Xenon {
    pub fn equilibrium(n: f64) -> Self {
        let x = n * (LAMBDA_XE + SIGMA_PHI_XE) / (LAMBDA_XE + SIGMA_PHI_XE * n);
        Self { iodine: n, xenon: x }
    }

    pub fn step(&mut self, n: f64, dt: f64) {
        let n = n.max(0.0);
        let g = GAMMA_I + GAMMA_XE;
        let di = LAMBDA_I * (n - self.iodine);
        let prod = (LAMBDA_XE + SIGMA_PHI_XE) / g * (GAMMA_XE * n + GAMMA_I * self.iodine);
        let loss = (LAMBDA_XE + SIGMA_PHI_XE * n) * self.xenon;
        self.iodine += di * dt;
        self.xenon += (prod - loss) * dt;
    }

    pub fn reactivity(&self) -> f64 {
        -XENON_WORTH_EQ * self.xenon
    }
}

/// Fission product decay heat groups, relative to rated power.
#[derive(Clone, Debug)]
pub struct DecayHeat {
    pub d: [f64; 9],
}

impl DecayHeat {
    pub fn equilibrium(n: f64) -> Self {
        let mut d = [0.0; 9];
        for (i, (f, _)) in DECAY_GROUPS.iter().enumerate() {
            d[i] = f * n;
        }
        Self { d }
    }

    /// Decay heat `t` seconds after shutdown from long operation at rated power.
    pub fn after_shutdown(t: f64) -> Self {
        let mut d = [0.0; 9];
        for (i, (f, l)) in DECAY_GROUPS.iter().enumerate() {
            d[i] = f * (-l * t).exp();
        }
        Self { d }
    }

    pub fn total_fraction() -> f64 {
        DECAY_GROUPS.iter().map(|g| g.0).sum()
    }

    pub fn power(&self) -> f64 {
        self.d.iter().sum()
    }

    pub fn step(&mut self, n: f64, dt: f64) {
        for (i, (f, l)) in DECAY_GROUPS.iter().enumerate() {
            // Exact update for a first-order lag.
            let a = 1.0 - (-l * dt).exp();
            self.d[i] += (f * n - self.d[i]) * a;
        }
    }
}

/// Everything in the reactor core except thermal-hydraulics.
#[derive(Clone, Debug)]
pub struct Core {
    pub kin: Kinetics,
    pub rods: Rods,
    pub xenon: Xenon,
    pub decay: DecayHeat,
    /// Lumped average fuel temperature, degC.
    pub fuel_t: f64,
    pub rho_rods: f64,
    pub rho_doppler: f64,
    pub rho_void: f64,
    pub rho_mod: f64,
    pub rho_xenon: f64,
    pub rho_total: f64,
    /// First kinetics fault of the last `step`, if any. The neutron population
    /// is frozen at the last accepted substep while this is set; callers should
    /// surface it rather than trust `kin.n`.
    pub kin_error: Option<KineticsError>,
}

impl Core {
    pub fn new(n: f64, rod_pos: f64, fuel_t: f64) -> Self {
        Self {
            kin: Kinetics::equilibrium(n),
            rods: Rods::new(rod_pos),
            xenon: Xenon::equilibrium(n),
            decay: DecayHeat::equilibrium(n),
            fuel_t,
            rho_rods: 0.0,
            rho_doppler: 0.0,
            rho_void: 0.0,
            rho_mod: 0.0,
            rho_xenon: 0.0,
            rho_total: 0.0,
            kin_error: None,
        }
    }

    /// Reactivity from everything except rods, for given conditions.
    pub fn feedback(fuel_t: f64, void: f64, mod_t: f64) -> (f64, f64, f64) {
        let doppler = -DOPPLER_A * ((fuel_t + 273.15).sqrt() - 293.15f64.sqrt());
        let v = VOID_COEF * void;
        let m = MOD_TEMP_COEF * (mod_t - 20.0);
        (doppler, v, m)
    }

    pub fn update_reactivity(&mut self, void: f64, mod_t: f64) {
        let (d, v, m) = Self::feedback(self.fuel_t, void, mod_t);
        self.rho_rods = self.rods.reactivity();
        self.rho_doppler = d;
        self.rho_void = v;
        self.rho_mod = m;
        self.rho_xenon = self.xenon.reactivity();
        self.rho_total = RHO_EXCESS_COLD + self.rho_rods + d + v + m + self.rho_xenon;
    }

    /// Prompt fission power fraction (excluding decay heat), relative to rated.
    pub fn prompt_power(&self) -> f64 {
        self.kin.n * (1.0 - DecayHeat::total_fraction())
    }

    /// Total thermal power generated in the core, W.
    pub fn thermal_power(&self) -> f64 {
        (self.prompt_power() + self.decay.power()) * RATED_POWER_W
    }

    pub fn k_eff(&self) -> f64 {
        1.0 / (1.0 - self.rho_total)
    }

    /// Advance neutronics, xenon, decay heat and fuel temperature.
    /// `coolant_t` is the coolant temperature, `htc_factor` scales fuel-to-coolant
    /// heat transfer (1 = normal, lower when the core is uncovered).
    /// Returns heat delivered to the coolant, W.
    ///
    /// Rod motion and rod worth are advanced on the kinetics substep timeline,
    /// so the neutron integration always sees the rod positions of the same
    /// substep. Thermal feedback (Doppler, void, moderator, xenon) is held at
    /// the values set by the last `update_reactivity`, i.e. one plant step of
    /// lag. `rho_rods`/`rho_total` are refreshed to the committed end-of-step
    /// positions. A kinetics fault is recorded in `kin_error`; rods keep moving
    /// but the population stays at the last accepted substep.
    pub fn step(&mut self, dt: f64, coolant_t: f64, htc_factor: f64) -> f64 {
        let rho_feedback = RHO_EXCESS_COLD + self.rho_doppler + self.rho_void + self.rho_mod + self.rho_xenon;
        self.kin_error = None;
        self.rho_rods = self.rods.reactivity();
        self.rho_total = rho_feedback + self.rho_rods;
        let substeps = ((dt / MAX_SUBSTEP).ceil() as usize).max(1);
        let h = dt / substeps as f64;
        for _ in 0..substeps {
            if self.rods.step(h) {
                self.rho_rods = self.rods.reactivity();
                self.rho_total = rho_feedback + self.rho_rods;
            }
            if self.kin_error.is_none() {
                if let Err(e) = self.kin.advance(self.rho_total, SOURCE, h) {
                    self.kin_error = Some(e);
                }
            }
        }
        self.xenon.step(self.kin.n, dt);
        self.decay.step(self.kin.n, dt);

        let p = self.thermal_power();
        let to_fuel = p * (1.0 - DIRECT_COOLANT_HEAT);
        let r = FUEL_RESISTANCE / htc_factor.max(0.02);
        // Implicit update of the fuel node (stable for any dt).
        let a = dt / (FUEL_HEAT_CAP * r);
        self.fuel_t = (self.fuel_t + dt * to_fuel / FUEL_HEAT_CAP + a * coolant_t) / (1.0 + a);
        let q_conv = (self.fuel_t - coolant_t) / r;
        q_conv + p * DIRECT_COOLANT_HEAT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_has_57_rods() {
        assert_eq!(rod_layout().len(), 57);
    }

    #[test]
    fn rod_worths() {
        let r = Rods::new(0.0);
        assert!((r.reactivity() + ROD_WORTH_TOTAL).abs() < 1e-9);
        let r = Rods::new(ROD_FULL_OUT);
        assert!(r.reactivity().abs() < 1e-12);
        let max_w = r.weights.iter().cloned().fold(0.0, f64::max);
        // Strongest single rod should be worth roughly 0.5-1 % dk/k.
        assert!(max_w * ROD_WORTH_TOTAL > 0.004 && max_w * ROD_WORTH_TOTAL < 0.012);
    }

    #[test]
    fn xenon_peaks_after_shutdown() {
        let mut x = Xenon::equilibrium(1.0);
        let mut peak = (0.0, 0.0);
        for i in 0..(72 * 3600) {
            x.step(0.0, 1.0);
            if x.xenon > peak.1 {
                peak = (i as f64 / 3600.0, x.xenon);
            }
        }
        assert!(peak.0 > 6.0 && peak.0 < 14.0, "peak at {} h", peak.0);
        assert!(peak.1 > 1.5 && peak.1 < 4.0, "peak value {}", peak.1);
    }

    /// Sol 15 / Astra 6: SRI must not run at the hydraulic scram rate. Old code
    /// had the SRI group fully in within SCRAM_TIME.
    #[test]
    fn sri_is_slower_than_scram() {
        let mut r = Rods::new(ROD_FULL_OUT);
        r.select_rod_insert();
        let group = r.sri_group();
        assert_eq!(group.len(), 4);
        for _ in 0..((SCRAM_TIME / 0.05) as usize) {
            r.step(0.05);
        }
        for &i in &group {
            assert!(r.pos[i] > ROD_FULL_OUT * 0.9, "SRI rod {i} at {} after {SCRAM_TIME} s", r.pos[i]);
        }
        // But the run-in does complete in its own time, and only for the group.
        for _ in 0..((SRI_INSERT_TIME / 0.05) as usize + 1) {
            r.step(0.05);
        }
        for i in 0..N_RODS {
            if group.contains(&i) {
                assert!(r.pos[i] <= 0.0 && !r.fast_insert[i]);
            } else {
                assert_eq!(r.pos[i], ROD_FULL_OUT);
            }
        }
        // A scram from full out finishes within SCRAM_TIME.
        let mut s = Rods::new(ROD_FULL_OUT);
        s.scram();
        for _ in 0..((SCRAM_TIME / 0.05) as usize + 1) {
            s.step(0.05);
        }
        assert!(s.all_in());
    }

    /// Astra 18: STOP halts at the actual position, not the next notch. Old
    /// code moved the target to 2.0 here.
    #[test]
    fn stop_halts_at_current_position() {
        let mut r = Rods::new(0.0);
        r.target[0] = 20.0;
        r.step(1.0);
        let p = r.pos[0];
        assert!((p - ROD_SPEED).abs() < 1e-9);
        r.stop();
        assert_eq!(r.target[0], p);
        assert!(!r.moving());
        // Fine (sub-notch) targets are honoured as commanded.
        r.target[0] = p + 0.1;
        for _ in 0..10 {
            r.step(0.1);
        }
        assert!((r.pos[0] - (p + 0.1)).abs() < 1e-9);
        // STOP leaves an SRI run-in alone.
        let mut s = Rods::new(ROD_FULL_OUT);
        s.select_rod_insert();
        s.step(1.0);
        s.stop();
        let i = s.sri_group()[0];
        assert_eq!(s.target[i], 0.0);
        assert!(s.fast_insert[i]);
    }

    /// Sol 28: a scram demand re-asserted every step must not report rod
    /// motion once every rod is in.
    #[test]
    fn scram_in_progress_false_when_all_in() {
        let mut r = Rods::new(0.0);
        r.scram();
        assert!(r.scramming);
        assert!(!r.scram_in_progress());

        let mut r = Rods::new(24.0);
        r.scram();
        assert!(r.scram_in_progress());
        let mut steps_in_progress = 0;
        for _ in 0..200 {
            r.step(0.05);
            r.scram(); // persistent protection signal
            if r.scram_in_progress() {
                steps_in_progress += 1;
            }
        }
        assert!(r.all_in());
        assert!(r.scramming, "demand latch stays set");
        assert!(!r.scram_in_progress());
        // Half travel at the scram rate takes ~1.5 s = 30 steps.
        assert!(steps_in_progress >= 28 && steps_in_progress <= 31, "{steps_in_progress}");
    }

    /// Helpers for the REFUEL one-rod interlock (Sol 2 / Astra 1, fix in sim.rs).
    #[test]
    fn withdrawal_counters_include_pending_targets() {
        let mut r = Rods::new(0.0);
        assert_eq!(r.n_withdrawing(), 0);
        assert_eq!(r.n_out(), 0);
        // Pending targets count even before any motion.
        r.target[3] = 4.0;
        assert_eq!(r.n_withdrawing(), 1);
        assert_eq!(r.n_out(), 1);
        assert_eq!(r.n_out_after(&[3], 6.0), 1, "continuing the same rod");
        assert_eq!(r.n_out_after(&[5], 2.0), 2, "a second rod");
        assert_eq!(r.n_out_after(&[5, 7], 2.0), 3, "a batch of two more");
        assert_eq!(r.n_out_after(&[3], 0.0), 0, "inserting the pending rod");
        // Actual position counts once the target is reached.
        r.step(10.0);
        r.target[3] = 4.0;
        assert_eq!(r.n_withdrawing(), 0);
        assert_eq!(r.n_out(), 1);
        assert_eq!(r.n_out_after(&[3], 0.0), 1, "still out until it actually moves in");
    }

    /// Sol 10 / Astra 14: the neutron integration must see the rod positions of
    /// the same step. One 50 ms step with scramming rods must agree with fifty
    /// 1 ms steps, and the published reactivity must match the committed
    /// positions. Old code used a full step of stale worth (~666 pcm here).
    #[test]
    fn kinetics_uses_rod_positions_of_the_same_step() {
        let make = || {
            let mut c = Core::new(1.0, 24.0, 600.0);
            c.update_reactivity(0.4, 280.0);
            c.rods.scram();
            c
        };
        let mut coarse = make();
        coarse.step(0.05, 280.0, 1.0);
        let mut fine = make();
        for _ in 0..50 {
            fine.step(0.001, 280.0, 1.0);
        }
        assert!(coarse.kin_error.is_none() && fine.kin_error.is_none());
        let rel = (coarse.kin.n - fine.kin.n).abs() / fine.kin.n;
        assert!(rel < 0.01, "coarse {} vs fine {} ({rel:.3} rel)", coarse.kin.n, fine.kin.n);
        assert!((coarse.rho_rods - coarse.rods.reactivity()).abs() < 1e-12, "published rod worth is stale");
        assert!(coarse.rods.pos[0] < 24.0 - 0.5, "rods moved during the step");
    }

    /// Sol 1 / Astra 8 at the core level: a strongly supercritical core grows
    /// instead of collapsing to the floor, and an unresolvable excursion is
    /// flagged rather than clamped.
    #[test]
    fn core_prompt_supercritical_is_resolved_or_flagged() {
        let mut c = Core::new(1e-8, ROD_FULL_OUT, 40.0);
        c.update_reactivity(0.0, 40.0);
        assert!(c.rho_total > 0.1, "rho = {}", c.rho_total);
        c.step(0.01, 40.0, 1.0);
        assert!(c.kin_error.is_none(), "{:?}", c.kin_error);
        assert!(c.kin.n > 1e-8 * 10.0, "n = {}", c.kin.n);
        // Drive it until f64 overflows: the fault is reported, n stays finite.
        let mut faulted = false;
        for _ in 0..200 {
            c.step(0.05, 40.0, 1.0);
            if c.kin_error.is_some() {
                faulted = true;
                break;
            }
        }
        assert!(faulted, "expected a kinetics fault, n = {}", c.kin.n);
        assert!(c.kin.n.is_finite() && c.kin.n > 0.0);
    }

    #[test]
    fn decay_heat_curve() {
        let mut d = DecayHeat::equilibrium(1.0);
        assert!((d.power() - 0.0766).abs() < 0.002);
        for _ in 0..3600 {
            d.step(0.0, 1.0);
        }
        assert!((d.power() - 0.0128).abs() < 0.002, "1 h: {}", d.power());
    }
}
