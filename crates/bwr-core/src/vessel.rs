//! Reactor vessel thermal-hydraulics.
//!
//! The vessel fluid is a single lumped mass/energy inventory in thermodynamic
//! equilibrium (saturated liquid + steam, or subcooled liquid below 100 degC
//! with the head vented). This captures the dominant pressure dynamics of a
//! BWR: stored energy in saturated water flashes when pressure drops and
//! steam condenses when it rises.
//!
//! On top of that a quasi-static axial core model computes subcooling, boiling
//! boundary, void fraction (drift flux) and mixture densities, and a momentum
//! equation for the natural circulation loop gives core flow from the density
//! difference between the downcomer and the core + chimney.

use crate::params::*;
use crate::steam::{self, sat_p, sat_t, Sat, P_ATM};

/// Head vent line flow coefficient, kg/s per Pa above atmospheric.
const C_HEAD_VENT: f64 = 5.0 / 7.0e6;
/// Minimum core flow (local recirculation inside the shroud), kg/s.
const W_MIN: f64 = 5.0;

#[derive(Clone, Debug, Default)]
pub struct CoreTh {
    pub alpha_avg: f64,
    pub rho_avg: f64,
    pub x_exit: f64,
    pub h_exit: f64,
    /// Steam leaving the top of the core, kg/s.
    pub steam_flow: f64,
    /// Height of the boiling boundary above the bottom of active fuel, m.
    pub boiling_boundary: f64,
}

/// Drift flux void fraction for steam/liquid superficial velocities.
fn drift_flux_void(j_g: f64, j_f: f64, s: &Sat) -> f64 {
    if j_g <= 0.0 {
        return 0.0;
    }
    let vgj = 1.41 * (s.sigma() * G * (s.rho_f - s.rho_g) / (s.rho_f * s.rho_f)).powf(0.25);
    (j_g / (C0 * (j_g + j_f) + vgj)).clamp(0.0, 0.999)
}

/// Axial core model: subcooled inlet, uniform-ish chopped cosine power shape.
pub fn core_th(s: &Sat, w: f64, q: f64, h_in: f64) -> CoreTh {
    let w = w.max(W_MIN);
    let n = N_AXIAL;
    let dz = CORE_HEIGHT / n as f64;
    // Chopped cosine shape, extrapolation length 0.3 m each end.
    let he = CORE_HEIGHT + 0.6;
    let shape: Vec<f64> = (0..n)
        .map(|k| {
            let z = (k as f64 + 0.5) * dz + 0.3;
            (std::f64::consts::PI * (z / he - 0.5)).cos()
        })
        .collect();
    let norm: f64 = shape.iter().sum();
    let mut h = h_in;
    let mut alpha_sum = 0.0;
    let mut rho_sum = 0.0;
    let mut bb = CORE_HEIGHT;
    let mut wg = 0.0;
    for k in 0..n {
        let dq = q * shape[k] / norm;
        let h_mid = h + 0.5 * dq / w;
        h += dq / w;
        let (alpha, rho) = if h_mid <= s.h_f {
            (0.0, steam::rho_liquid(steam::t_from_h_liquid(h_mid)))
        } else {
            if bb >= CORE_HEIGHT {
                bb = k as f64 * dz;
            }
            let wg_mid = ((h_mid - s.h_f) * w / s.h_fg()).min(w);
            let j_g = wg_mid / (s.rho_g * A_CORE);
            let j_f = (w - wg_mid) / (s.rho_f * A_CORE);
            let a = drift_flux_void(j_g, j_f, s);
            (a, a * s.rho_g + (1.0 - a) * s.rho_f)
        };
        alpha_sum += alpha;
        rho_sum += rho;
        if k == n - 1 {
            wg = ((h - s.h_f) * w / s.h_fg()).clamp(0.0, w);
        }
    }
    CoreTh {
        alpha_avg: alpha_sum / n as f64,
        rho_avg: rho_sum / n as f64,
        x_exit: wg / w,
        h_exit: h,
        steam_flow: wg,
        boiling_boundary: bb,
    }
}

/// Void fraction and mixture density in the chimney for a given core exit.
pub fn chimney_th(s: &Sat, w: f64, core: &CoreTh) -> (f64, f64) {
    let w = w.max(W_MIN);
    if core.steam_flow <= 0.0 {
        let rho = steam::rho_liquid(steam::t_from_h_liquid(core.h_exit.min(s.h_f)));
        return (0.0, rho);
    }
    let j_g = core.steam_flow / (s.rho_g * A_CHIMNEY);
    let j_f = (w - core.steam_flow) / (s.rho_f * A_CHIMNEY);
    let a = drift_flux_void(j_g, j_f, s);
    (a, a * s.rho_g + (1.0 - a) * s.rho_f)
}

/// Two-phase friction multiplier (homogeneous-like, damped).
fn phi2(s: &Sat, x: f64) -> f64 {
    1.0 + 0.5 * x * (s.rho_f / s.rho_g - 1.0)
}

/// Liquid volume below elevation z, given core and chimney voids.
fn liquid_volume_below(z: f64, a_core: f64, a_ch: f64) -> f64 {
    if z <= Z_CORE_BOTTOM {
        return V_LOWER_PLENUM * (z / Z_CORE_BOTTOM).max(0.0);
    }
    let mut v = V_LOWER_PLENUM;
    let zc = z.min(Z_TAF);
    v += A_CORE * (zc - Z_CORE_BOTTOM) * (1.0 - a_core);
    if z > Z_TAF {
        let zc = z.min(Z_CHIMNEY_TOP);
        v += A_CHIMNEY * (zc - Z_TAF) * (1.0 - a_ch);
    }
    v += A_DOWNCOMER * (z.min(Z_CHIMNEY_TOP) - Z_CORE_BOTTOM);
    if z > Z_CHIMNEY_TOP {
        v += A_SEPARATOR_REGION * (z.min(Z_DRYER) - Z_CHIMNEY_TOP);
    }
    if z > Z_DRYER {
        v += A_DOME * (z - Z_DRYER);
    }
    v
}

/// Water level elevation (m above vessel bottom) for a liquid volume.
fn level_from_volume(v_l: f64, a_core: f64, a_ch: f64) -> f64 {
    let (mut lo, mut hi) = (0.0, VESSEL_HEIGHT);
    if liquid_volume_below(hi, a_core, a_ch) <= v_l {
        return VESSEL_HEIGHT;
    }
    for _ in 0..50 {
        let mid = 0.5 * (lo + hi);
        if liquid_volume_below(mid, a_core, a_ch) < v_l {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// Equilibrium state of the vessel fluid.
#[derive(Clone, Copy, Debug)]
pub struct Eos {
    pub t: f64,
    pub p: f64,
    /// Static steam quality of the vessel inventory (steam mass / total).
    pub x: f64,
    pub subcooled: bool,
    /// True if the vessel is water-solid (no steam space).
    pub solid: bool,
}

fn energy_at(t: f64, mass: f64) -> (f64, f64) {
    let s = sat_t(t);
    if t < 100.0 {
        return (mass * s.u_f() + METAL_HEAT_CAP * t, 0.0);
    }
    let v = V_VESSEL_FLUID / mass;
    let x = ((v - s.v_f()) / (s.v_g() - s.v_f())).clamp(0.0, 1.0);
    (mass * (s.u_f() + x * (s.u_g() - s.u_f())) + METAL_HEAT_CAP * t, x)
}

/// Solve for temperature/pressure from total fluid mass and energy (incl. metal).
pub fn solve_eos(mass: f64, energy: f64) -> Eos {
    let (mut lo, mut hi) = (1.0, 349.5);
    for _ in 0..60 {
        let mid = 0.5 * (lo + hi);
        if energy_at(mid, mass).0 < energy {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let t = 0.5 * (lo + hi);
    let (_, x) = energy_at(t, mass);
    let s = sat_t(t);
    let subcooled = t < 100.0;
    let solid = !subcooled && x <= 0.0;
    Eos { t, p: if subcooled { P_ATM } else { s.p }, x, subcooled, solid }
}

/// Total energy for a fluid mass at temperature t.
pub fn energy_for(mass: f64, t: f64) -> f64 {
    energy_at(t, mass).0
}

#[derive(Clone, Debug)]
pub struct VesselInputs {
    /// Heat from the core into the coolant, W.
    pub q_core: f64,
    /// Heat removed by isolation condensers, W.
    pub q_ic: f64,
    pub w_feed: f64,
    pub h_feed: f64,
    /// Steam leaving through the main steam lines, kg/s.
    pub w_steam: f64,
    /// Liquid removed (RWCU reject), kg/s.
    pub w_liquid_out: f64,
}

#[derive(Clone, Debug)]
pub struct Vessel {
    pub mass: f64,
    pub energy: f64,
    pub eos: Eos,
    pub core_flow: f64,
    pub h_in: f64,
    pub alpha_core: f64,
    pub alpha_ch: f64,
    pub core: CoreTh,
    /// Level elevation above vessel bottom, m.
    pub level_z: f64,
    pub head_vent_open: bool,
    pub vent_flow: f64,
    pub heatup_rate: f64,
    /// Friction coefficient, calibrated so rated flow is CORE_FLOW_RATED.
    pub k_fric: f64,
    pub drive_dp: f64,
}

impl Vessel {
    /// Build a vessel at temperature `t` with level at `level_z`, flow and voids
    /// consistent with heat input `q` and feed conditions.
    pub fn new(t: f64, level_z: f64, q: f64, w_feed: f64, t_feed: f64, vent_open: bool) -> Self {
        let mut v = Self {
            mass: 0.0,
            energy: 0.0,
            eos: Eos { t, p: P_ATM, x: 0.0, subcooled: true, solid: false },
            core_flow: CORE_FLOW_RATED,
            h_in: steam::h_liquid(t),
            alpha_core: 0.0,
            alpha_ch: 0.0,
            core: CoreTh::default(),
            level_z,
            head_vent_open: vent_open,
            vent_flow: 0.0,
            heatup_rate: 0.0,
            k_fric: Self::calibrate_friction(),
            drive_dp: 0.0,
        };
        // Iterate flow, voids and inventory to a consistent state.
        let s = if t < 100.0 { sat_p(P_ATM) } else { sat_t(t) };
        let h_fw = steam::h_liquid(t_feed);
        for _ in 0..200 {
            let w_rec = (v.core_flow - w_feed).max(0.0);
            // Liquid returning to the downcomer is at the bulk vessel temperature.
            let h_ret = steam::h_liquid(t.min(s.t));
            v.h_in = if v.core_flow > 0.0 {
                (w_rec * h_ret + w_feed.min(v.core_flow) * h_fw) / v.core_flow
            } else {
                h_ret
            };
            v.core = core_th(&s, v.core_flow, q, v.h_in);
            let (a_ch, _) = chimney_th(&s, v.core_flow, &v.core);
            v.alpha_core = v.core.alpha_avg;
            v.alpha_ch = a_ch;
            let (drive, phi) = v.drive(&s);
            let w_new = if drive > 0.0 { (drive / (v.k_fric * phi)).sqrt() } else { W_MIN };
            v.core_flow = 0.7 * v.core_flow + 0.3 * w_new.max(W_MIN);
        }
        // Inventory: liquid volume to reach the requested level, plus dome steam.
        let v_l = liquid_volume_below(level_z, v.alpha_core, v.alpha_ch);
        let rho_l = if t < 100.0 { steam::rho_liquid(t) } else { s.rho_f };
        let v_g = V_VESSEL_FLUID - v_l;
        let m_g = if t < 100.0 { 0.0 } else { v_g * s.rho_g };
        v.mass = v_l * rho_l + m_g;
        v.energy = energy_for(v.mass, t);
        v.eos = solve_eos(v.mass, v.energy);
        v.update_level();
        v
    }

    fn calibrate_friction() -> f64 {
        // Rated state: 7.2 MPa, 870 MW, rated feed at 215 degC, level at NWL.
        let s = sat_p(P_RATED);
        let w = CORE_FLOW_RATED;
        let h_fw = steam::h_liquid(T_FW_RATED);
        let h_in = (s.h_f * (w - STEAM_FLOW_RATED) + h_fw * STEAM_FLOW_RATED) / w;
        let core = core_th(&s, w, RATED_POWER_W, h_in);
        let (_, rho_ch) = chimney_th(&s, w, &core);
        let rho_dc = steam::rho_liquid(steam::t_from_h_liquid(h_in));
        let drive = G
            * (rho_dc * (Z_CHIMNEY_TOP - Z_CORE_BOTTOM) - core.rho_avg * CORE_HEIGHT - rho_ch * CHIMNEY_HEIGHT);
        drive / (w * w * phi2(&s, core.x_exit))
    }

    /// Natural circulation driving head (Pa) and two-phase friction multiplier.
    fn drive(&self, s: &Sat) -> (f64, f64) {
        let (a_ch_qs, rho_ch_qs) = chimney_th(s, self.core_flow, &self.core);
        let dr = s.rho_g - s.rho_f;
        // Quasi-static densities corrected for the lagged void fractions.
        let rho_core = self.core.rho_avg + (self.alpha_core - self.core.alpha_avg) * dr;
        let rho_ch = rho_ch_qs + (self.alpha_ch - a_ch_qs) * dr;
        let rho_dc = steam::rho_liquid(steam::t_from_h_liquid(self.h_in.min(s.h_f)));
        let dp = G * (rho_dc * (Z_CHIMNEY_TOP - Z_CORE_BOTTOM) - rho_core * CORE_HEIGHT - rho_ch * CHIMNEY_HEIGHT);
        // When the downcomer level falls below the chimney top, spillover from
        // the separators stops and the loop is broken.
        let spill = ((self.level_z - (Z_CHIMNEY_TOP - 0.5)) / 0.5).clamp(0.0, 1.0);
        (dp * spill, phi2(s, self.core.x_exit))
    }

    fn update_level(&mut self) {
        let rho_l = if self.eos.subcooled { steam::rho_liquid(self.eos.t) } else { sat_t(self.eos.t).rho_f };
        let v_l = self.mass * (1.0 - self.eos.x) / rho_l;
        self.level_z = level_from_volume(v_l, self.alpha_core, self.alpha_ch);
    }

    /// Saturation properties at the current vessel pressure.
    pub fn sat(&self) -> Sat {
        sat_p(self.eos.p)
    }

    /// Fraction of the active fuel covered by two-phase mixture.
    pub fn core_coverage(&self) -> f64 {
        let collapsed = (self.level_z - Z_CORE_BOTTOM) / CORE_HEIGHT;
        (collapsed / (1.0 - self.alpha_core).max(0.2)).clamp(0.0, 1.0)
    }

    pub fn level_nr_cm(&self) -> f64 {
        (self.level_z - Z_NWL) * 100.0
    }

    pub fn level_above_taf(&self) -> f64 {
        self.level_z - Z_TAF
    }

    pub fn step(&mut self, dt: f64, inp: &VesselInputs) {
        let s0 = self.sat();
        let t_before = self.eos.t;

        // Head vent: steam escapes while the vessel is above atmospheric pressure.
        self.vent_flow = if self.head_vent_open && !self.eos.subcooled {
            (C_HEAD_VENT * (self.eos.p - P_ATM)).max(0.0) + 0.02 * (self.eos.t - 100.0).max(0.0)
        } else {
            0.0
        };

        let h_steam = s0.h_g;
        let h_liq = if self.eos.subcooled { steam::h_liquid(self.eos.t) } else { s0.h_f };
        let heat_loss = HEAT_LOSS_HOT * ((self.eos.t - 20.0) / 268.0).max(0.0);

        let w_steam = if self.eos.subcooled { 0.0 } else { inp.w_steam };
        let dm = inp.w_feed - w_steam - inp.w_liquid_out - self.vent_flow;
        let de = inp.q_core - inp.q_ic - heat_loss + inp.w_feed * inp.h_feed
            - (w_steam + self.vent_flow) * h_steam
            - inp.w_liquid_out * h_liq;
        self.mass = (self.mass + dm * dt).max(1000.0);
        self.energy += de * dt;
        self.eos = solve_eos(self.mass, self.energy);
        let s = self.sat_for_core();

        // Core inlet enthalpy: separated liquid mixed with feedwater in the downcomer.
        let w = self.core_flow.max(W_MIN);
        let w_fw = inp.w_feed.min(w);
        // Separated / recirculating liquid is at the bulk vessel temperature.
        let h_ret = steam::h_liquid(self.eos.t).min(s.h_f);
        let h_in_target = ((w - w_fw) * h_ret + w_fw * inp.h_feed) / w;
        let tau_dc = (V_DOWNCOMER * 750.0 / w).clamp(2.0, 600.0);
        self.h_in += (h_in_target - self.h_in) * (dt / tau_dc).min(1.0);
        self.h_in = self.h_in.min(s.h_f);

        // Core and chimney voids with transport lags.
        self.core = core_th(&s, w, inp.q_core, self.h_in);
        let (a_ch, _) = chimney_th(&s, w, &self.core);
        self.alpha_core += (self.core.alpha_avg - self.alpha_core) * (dt / 0.5).min(1.0);
        self.alpha_ch += (a_ch - self.alpha_ch) * (dt / 2.0).min(1.0);

        // Natural circulation momentum (semi-implicit friction).
        let (drive, phi) = self.drive(&s);
        self.drive_dp = drive;
        let k = self.k_fric * phi;
        self.core_flow = (self.core_flow + dt * drive / LOOP_INERTIA) / (1.0 + dt * k * self.core_flow.abs() / LOOP_INERTIA);
        self.core_flow = self.core_flow.max(W_MIN);

        self.update_level();
        let rate = (self.eos.t - t_before) / dt * 3600.0;
        self.heatup_rate += (rate - self.heatup_rate) * (dt / 30.0).min(1.0);
    }

    /// Saturation state used by the core: actual pressure, or atmospheric when subcooled.
    fn sat_for_core(&self) -> Sat {
        if self.eos.subcooled {
            sat_p(P_ATM)
        } else {
            sat_p(self.eos.p)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eos_roundtrip() {
        for &t in &[40.0, 150.0, 287.0] {
            let m = 200_000.0;
            let e = energy_for(m, t);
            let eos = solve_eos(m, e);
            assert!((eos.t - t).abs() < 1e-3, "t {t} -> {}", eos.t);
        }
    }

    #[test]
    fn rated_flow_and_voids_are_plausible() {
        let v = Vessel::new(sat_p(P_RATED).t, Z_NWL, RATED_POWER_W, STEAM_FLOW_RATED, T_FW_RATED, false);
        assert!((v.core_flow - CORE_FLOW_RATED).abs() < 50.0, "flow {}", v.core_flow);
        assert!(v.alpha_core > 0.25 && v.alpha_core < 0.55, "core void {}", v.alpha_core);
        assert!(v.alpha_ch > 0.5 && v.alpha_ch < 0.85, "chimney void {}", v.alpha_ch);
        assert!((v.level_z - Z_NWL).abs() < 0.05, "level {}", v.level_z);
        assert!((v.eos.p - P_RATED).abs() < 0.05e6, "p {}", v.eos.p);
    }

    #[test]
    fn level_volume_inverse() {
        for &z in &[1.0, 4.0, 10.0, 16.0, 19.0, 22.0] {
            let v = liquid_volume_below(z, 0.4, 0.7);
            assert!((level_from_volume(v, 0.4, 0.7) - z).abs() < 1e-6);
        }
    }
}
