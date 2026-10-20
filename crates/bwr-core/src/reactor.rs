//! Reactor core: control rods, reactivity feedback, xenon, decay heat and fuel temperature.

use crate::kinetics::Kinetics;
use crate::params::*;

/// Grid size of the rod map.
pub const ROD_GRID: usize = 9;

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
        self.pos.iter().all(|&p| p <= 0.01)
    }

    pub fn moving(&self) -> bool {
        self.pos.iter().zip(&self.target).any(|(p, t)| (p - t).abs() > 1e-6)
    }

    pub fn step(&mut self, dt: f64) {
        if self.scramming {
            let v = ROD_FULL_OUT / SCRAM_TIME * dt;
            for (p, t) in self.pos.iter_mut().zip(self.target.iter_mut()) {
                *p = (*p - v).max(0.0);
                *t = 0.0;
            }
            if self.all_in() {
                self.scramming = false;
            }
            return;
        }
        let v = ROD_SPEED * dt;
        let vf = ROD_FULL_OUT / SCRAM_TIME * dt;
        for i in 0..self.pos.len() {
            if self.fast_insert[i] {
                self.target[i] = 0.0;
                self.pos[i] = (self.pos[i] - vf).max(0.0);
                if self.pos[i] <= 0.0 {
                    self.fast_insert[i] = false;
                }
                continue;
            }
            let d = self.target[i] - self.pos[i];
            self.pos[i] += d.clamp(-v, v);
        }
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

    pub fn stop(&mut self) {
        // Stop at the nearest notch in the direction of travel.
        for (p, t) in self.pos.iter().zip(self.target.iter_mut()) {
            if (*t - p).abs() > 1e-6 {
                *t = if *t > *p { (p / 2.0).ceil() * 2.0 } else { (p / 2.0).floor() * 2.0 };
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
    pub fn step(&mut self, dt: f64, coolant_t: f64, htc_factor: f64) -> f64 {
        self.rods.step(dt);
        let substeps = ((dt / 1.0e-3).ceil() as usize).max(1);
        self.kin.advance(self.rho_total, SOURCE, dt, substeps);
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
