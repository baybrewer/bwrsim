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

/// Isothermal compressibility of liquid water, 1/Pa. ASSUMED: IF97 gives
/// ~4.6e-10 at 20 degC rising to ~1e-9 at 250 degC; a single mid-range value
/// is used for the water-solid branch. (Candidate for params.rs.)
pub const LIQUID_COMPRESSIBILITY: f64 = 7.0e-10;
/// Largest fractional over-fill (M*v_f - V)/V the linear compressibility model
/// accepts before the state is reported as outside the EOS domain (~28 MPa).
pub const MAX_LIQUID_COMPRESSION: f64 = 0.02;
/// Lowest temperature the EOS solves for, degC (first saturation-table row).
pub const EOS_T_MIN: f64 = 1.0;
/// Highest temperature the EOS solves for, degC (below the table's 350 degC end).
pub const EOS_T_MAX: f64 = 349.5;

/// Phase domain the vessel inventory occupies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EosRegime {
    /// Liquid fills the whole vessel; pressure comes from liquid compression.
    CompressedLiquid,
    /// Saturated liquid and vapour in equilibrium (vapour at its saturation
    /// partial pressure below 100 degC, where the head space is at atmospheric).
    TwoPhase,
}

/// Reasons the EOS cannot represent a (mass, energy) pair.
#[derive(Clone, Debug, PartialEq)]
pub enum EosError {
    /// Mass or energy is not a finite positive number.
    NonFinite,
    /// Energy is below the state at `EOS_T_MIN`.
    BelowDomain { energy: f64, min_energy: f64 },
    /// Energy exceeds the state at `EOS_T_MAX`.
    AboveDomain { energy: f64, max_energy: f64 },
    /// Vessel volume exceeds the saturated-vapour volume of the inventory.
    Superheated { t: f64, quality: f64 },
    /// Saturated-liquid volume exceeds the vessel by more than the
    /// compressibility model supports.
    OverCompressed { t: f64, over_fill: f64 },
    /// Bisection did not reproduce the requested energy.
    NoConvergence { residual: f64 },
}

impl std::fmt::Display for EosError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NonFinite => write!(f, "vessel mass/energy not finite"),
            Self::BelowDomain { energy, min_energy } => {
                write!(f, "vessel energy {energy:.3e} J below EOS minimum {min_energy:.3e} J ({EOS_T_MIN} degC)")
            }
            Self::AboveDomain { energy, max_energy } => {
                write!(f, "vessel energy {energy:.3e} J above EOS maximum {max_energy:.3e} J ({EOS_T_MAX} degC)")
            }
            Self::Superheated { t, quality } => {
                write!(f, "vessel inventory superheated at {t:.1} degC (quality {quality:.4} > 1)")
            }
            Self::OverCompressed { t, over_fill } => {
                write!(f, "vessel over-filled by {:.2} % at {t:.1} degC, beyond the compressibility model", over_fill * 100.0)
            }
            Self::NoConvergence { residual } => write!(f, "EOS solve residual {residual:.3e} J"),
        }
    }
}

impl std::error::Error for EosError {}

/// Equilibrium state of the vessel fluid.
#[derive(Clone, Copy, Debug)]
pub struct Eos {
    pub t: f64,
    pub p: f64,
    /// Static steam quality of the vessel inventory (steam mass / total).
    pub x: f64,
    /// Below the atmospheric boiling point: no net steam production, head
    /// space at atmospheric pressure.
    pub subcooled: bool,
    /// True if the vessel is water-solid (no steam space).
    pub solid: bool,
    pub regime: EosRegime,
    /// Fractional over-fill (M*v_f - V)/V in the compressed-liquid regime, else 0.
    pub over_fill: f64,
}

/// Specific volume of the inventory, m3/kg.
fn specific_volume(mass: f64) -> f64 {
    V_VESSEL_FLUID / mass
}

/// Lever-rule quality for the inventory at saturation temperature `t`
/// (negative when liquid alone would overfill the vessel, >1 when superheated).
fn quality_at(s: &Sat, mass: f64) -> f64 {
    (specific_volume(mass) - s.v_f()) / (s.v_g() - s.v_f())
}

/// Total energy (fluid + metal) at temperature `t`, and the clamped quality.
/// Continuous through 100 degC: below it the head space holds equilibrium
/// vapour at its saturation partial pressure, above it the same lever rule
/// applies at full saturation pressure.
fn energy_at(t: f64, mass: f64) -> (f64, f64) {
    let s = sat_t(t);
    let x = quality_at(&s, mass).clamp(0.0, 1.0);
    (mass * (s.u_f() + x * (s.u_g() - s.u_f())) + METAL_HEAT_CAP * t, x)
}

/// Pressure and regime for the inventory at temperature `t`.
fn pressure_at(t: f64, mass: f64) -> Result<(f64, EosRegime, f64), EosError> {
    let s = sat_t(t);
    // The head space never falls below atmospheric (vented, or air-filled when sealed cold).
    let p_base = s.p.max(P_ATM);
    let x = quality_at(&s, mass);
    if x < 0.0 {
        let over_fill = (mass * s.v_f() - V_VESSEL_FLUID) / V_VESSEL_FLUID;
        if over_fill > MAX_LIQUID_COMPRESSION {
            return Err(EosError::OverCompressed { t, over_fill });
        }
        return Ok((p_base + over_fill / LIQUID_COMPRESSIBILITY, EosRegime::CompressedLiquid, over_fill));
    }
    if x > 1.0 + 1e-9 {
        return Err(EosError::Superheated { t, quality: x });
    }
    Ok((p_base, EosRegime::TwoPhase, 0.0))
}

/// Solve for temperature/pressure from total fluid mass and energy (incl. metal).
/// Errors when the pair lies outside the saturated/compressed-liquid domain;
/// callers must keep their previous state in that case.
pub fn solve_eos(mass: f64, energy: f64) -> Result<Eos, EosError> {
    if !(mass > 0.0) || !mass.is_finite() || !energy.is_finite() {
        return Err(EosError::NonFinite);
    }
    let (mut lo, mut hi) = (EOS_T_MIN, EOS_T_MAX);
    let min_energy = energy_at(lo, mass).0;
    let max_energy = energy_at(hi, mass).0;
    if energy < min_energy {
        return Err(EosError::BelowDomain { energy, min_energy });
    }
    if energy > max_energy {
        return Err(EosError::AboveDomain { energy, max_energy });
    }
    for _ in 0..60 {
        let mid = 0.5 * (lo + hi);
        if energy_at(mid, mass).0 < energy {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let t = 0.5 * (lo + hi);
    let (e_t, x) = energy_at(t, mass);
    let residual = e_t - energy;
    if residual.abs() > 1e-6 * energy.abs() + 1.0 {
        return Err(EosError::NoConvergence { residual });
    }
    let (p, regime, over_fill) = pressure_at(t, mass)?;
    Ok(Eos {
        t,
        p,
        x,
        subcooled: t < 100.0,
        solid: regime == EosRegime::CompressedLiquid,
        regime,
        over_fill,
    })
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

/// Isolation condenser primary-side mass ports (see `ic::IcStep`).
#[derive(Clone, Copy, Debug, Default)]
pub struct IcPorts {
    /// Steam drawn into the IC tubes, kg/s (leaves at the dome enthalpy).
    pub w_steam: f64,
    /// Condensate returned to the vessel, kg/s.
    pub w_return: f64,
    /// Specific enthalpy of the returned condensate, J/kg.
    pub h_return: f64,
}

/// Port transfers actually applied in the last `Vessel::step`, after bounding
/// by phase inventory. Connected systems must use these, not their requests.
#[derive(Clone, Copy, Debug, Default)]
pub struct VesselFlows {
    pub w_feed: f64,
    /// Main steam line flow, kg/s, and the enthalpy it carries (saturated
    /// liquid when the vessel is water-solid and liquid is displaced).
    pub w_steam: f64,
    pub h_steam: f64,
    /// RWCU reject flow and enthalpy.
    pub w_liquid: f64,
    pub h_liquid: f64,
    /// Head vent flow and enthalpy.
    pub w_vent: f64,
    pub h_vent: f64,
    pub w_ic_steam: f64,
    pub w_ic_return: f64,
    /// Requested outflow the inventory could not supply, kg/s.
    pub shortfall: f64,
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
    /// Set when the last step could not be represented by the EOS; the state
    /// is left unchanged and the simulator should stop advancing.
    pub last_error: Option<EosError>,
    /// Accepted port transfers of the last step.
    pub flows: VesselFlows,
    /// True when the last step had to cut requested outflow for lack of inventory.
    pub exhausted: bool,
    /// Integrated requested-but-unavailable outflow, kg.
    pub shortfall_kg: f64,
}

impl Vessel {
    /// Build a vessel at temperature `t` with level at `level_z`, flow and voids
    /// consistent with heat input `q` and feed conditions.
    pub fn new(t: f64, level_z: f64, q: f64, w_feed: f64, t_feed: f64, vent_open: bool) -> Self {
        let mut v = Self {
            mass: 0.0,
            energy: 0.0,
            eos: Eos { t, p: P_ATM, x: 0.0, subcooled: true, solid: false, regime: EosRegime::TwoPhase, over_fill: 0.0 },
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
            last_error: None,
            flows: VesselFlows::default(),
            exhausted: false,
            shortfall_kg: 0.0,
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
        // Head space holds equilibrium vapour at every temperature (partial
        // pressure below 100 degC), matching the EOS closure.
        let m_g = v_g * sat_t(t).rho_g;
        v.mass = v_l * rho_l + m_g;
        v.energy = energy_for(v.mass, t);
        v.eos = solve_eos(v.mass, v.energy).expect("initial vessel state must be inside the EOS domain");
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

    /// Saturation properties at the current vessel pressure (at the vessel
    /// temperature when water-solid, where pressure is set by compression).
    pub fn sat(&self) -> Sat {
        if self.eos.solid {
            sat_t(self.eos.t)
        } else {
            sat_p(self.eos.p)
        }
    }

    /// Fraction of the active fuel covered by two-phase mixture. `level_z` is
    /// already the mixture-surface elevation (void swell is in
    /// `liquid_volume_below`), so coverage is purely geometric.
    pub fn core_coverage(&self) -> f64 {
        ((self.level_z - Z_CORE_BOTTOM) / CORE_HEIGHT).clamp(0.0, 1.0)
    }

    /// Smallest inventory the EOS can represent at the current temperature:
    /// the vessel filled with saturated vapour.
    fn min_mass(&self) -> f64 {
        V_VESSEL_FLUID / sat_t(self.eos.t).v_g()
    }

    pub fn level_nr_cm(&self) -> f64 {
        (self.level_z - Z_NWL) * 100.0
    }

    pub fn level_above_taf(&self) -> f64 {
        self.level_z - Z_TAF
    }

    /// Advance one step without isolation-condenser mass ports.
    pub fn step(&mut self, dt: f64, inp: &VesselInputs) {
        self.step_with_ic(dt, inp, &IcPorts::default());
    }

    /// Advance one step with explicit isolation-condenser mass ports. Outflows
    /// are bounded by the phase inventory that can supply them; the shortfall
    /// is reported in `flows`. If the resulting state leaves the EOS domain the
    /// step is rejected: `last_error` is set and nothing changes.
    pub fn step_with_ic(&mut self, dt: f64, inp: &VesselInputs, ic: &IcPorts) {
        let s0 = self.sat();
        let t_before = self.eos.t;
        let (subcooled, solid, x) = (self.eos.subcooled, self.eos.solid, self.eos.x);

        let h_steam = s0.h_g;
        let h_liq = if subcooled { steam::h_liquid(self.eos.t) } else { s0.h_f };
        let heat_loss = HEAT_LOSS_HOT * ((self.eos.t - 20.0) / 268.0).max(0.0);

        // Head vent: steam escapes while above atmospheric; a water-solid
        // vessel relieves liquid through it instead.
        let vent_req = if self.head_vent_open && (!subcooled || solid) {
            (C_HEAD_VENT * (self.eos.p - P_ATM)).max(0.0)
                + if subcooled { 0.0 } else { 0.02 * (self.eos.t - 100.0).max(0.0) }
        } else {
            0.0
        };
        // Steam lines carry steam when a steam space exists, displaced liquid when solid.
        let steam_req = if subcooled { 0.0 } else { inp.w_steam };
        let (h_steam_port, h_vent_port) = if solid { (h_liq, h_liq) } else { (h_steam, h_steam) };
        let ic_steam_req = if solid { 0.0 } else { ic.w_steam };

        // Bound each phase port by the phase inventory, then the total by the
        // smallest inventory the EOS can represent.
        let m_vap = self.mass * x;
        let m_liq = self.mass - m_vap;
        let (vap_req, liq_req) = if solid {
            (0.0, inp.w_liquid_out + steam_req + vent_req)
        } else {
            (steam_req + vent_req + ic_steam_req, inp.w_liquid_out)
        };
        let scale = |req: f64, avail: f64| if req * dt > avail { (avail / (req * dt)).max(0.0) } else { 1.0 };
        let f_vap = scale(vap_req, m_vap);
        let f_liq = scale(liq_req, m_liq);
        let inflow = inp.w_feed + ic.w_return;
        let out_phase = vap_req * f_vap + liq_req * f_liq;
        let avail_total = (self.mass + inflow * dt - 1.001 * self.min_mass()).max(0.0);
        let f_tot = scale(out_phase, avail_total);
        let (f_v, f_l) = (f_vap * f_tot, f_liq * f_tot);
        let (f_steam, f_vent) = if solid { (f_l, f_l) } else { (f_v, f_v) };
        let flows = VesselFlows {
            w_feed: inp.w_feed,
            w_steam: steam_req * f_steam,
            h_steam: h_steam_port,
            w_liquid: inp.w_liquid_out * f_l,
            h_liquid: h_liq,
            w_vent: vent_req * f_vent,
            h_vent: h_vent_port,
            w_ic_steam: ic_steam_req * f_v,
            w_ic_return: ic.w_return,
            shortfall: (steam_req + vent_req + ic_steam_req + inp.w_liquid_out) - out_phase * f_tot,
        };

        let dm = inflow - flows.w_steam - flows.w_liquid - flows.w_vent - flows.w_ic_steam;
        let de = inp.q_core - inp.q_ic - heat_loss
            + inp.w_feed * inp.h_feed
            + ic.w_return * ic.h_return
            - flows.w_steam * flows.h_steam
            - flows.w_vent * flows.h_vent
            - flows.w_ic_steam * h_steam
            - flows.w_liquid * flows.h_liquid;
        let mass = self.mass + dm * dt;
        let energy = self.energy + de * dt;
        match solve_eos(mass, energy) {
            Ok(eos) => {
                self.mass = mass;
                self.energy = energy;
                self.eos = eos;
                self.last_error = None;
            }
            Err(e) => {
                self.last_error = Some(e);
                self.flows = VesselFlows::default();
                self.vent_flow = 0.0;
                return;
            }
        }
        self.flows = flows;
        self.vent_flow = flows.w_vent;
        self.exhausted = flows.shortfall > 0.0;
        self.shortfall_kg += flows.shortfall * dt;
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

    fn vessel_at(t: f64, mass: f64) -> Vessel {
        let mut v = Vessel::new(t.min(99.0), Z_NWL, 0.0, 0.0, 40.0, false);
        v.mass = mass;
        v.energy = energy_for(mass, t);
        v.eos = solve_eos(mass, v.energy).unwrap();
        v.update_level();
        v
    }

    #[test]
    fn eos_roundtrip() {
        // Physically admissible two-phase fixtures: v_f < V/M < v_g at each temperature.
        for &(t, m) in &[(40.0, 200_000.0), (150.0, 200_000.0), (287.0, 150_000.0)] {
            let e = energy_for(m, t);
            let eos = solve_eos(m, e).unwrap();
            assert!((eos.t - t).abs() < 1e-3, "t {t} -> {}", eos.t);
            assert_eq!(eos.regime, EosRegime::TwoPhase);
            let s = sat_t(eos.t);
            let occupied = m * ((1.0 - eos.x) * s.v_f() + eos.x * s.v_g());
            assert!((occupied - V_VESSEL_FLUID).abs() < 1e-6 * V_VESSEL_FLUID, "volume {occupied}");
            assert!((energy_for(m, eos.t) - e).abs() < 1e-6 * e.abs());
        }
    }

    #[test]
    fn eos_rejects_liquid_volume_beyond_the_vessel() {
        // 200 t at 287 degC needs ~271 m3 of saturated liquid in a 254.8 m3 vessel.
        let m = 200_000.0;
        match solve_eos(m, energy_for(m, 287.0)) {
            Err(EosError::OverCompressed { over_fill, .. }) => assert!(over_fill > 0.05, "over {over_fill}"),
            other => panic!("expected OverCompressed, got {other:?}"),
        }
    }

    #[test]
    fn eos_rejects_energy_outside_bracket() {
        let m = 100_000.0;
        assert!(matches!(solve_eos(m, energy_for(m, EOS_T_MAX) + 1e12), Err(EosError::AboveDomain { .. })));
        assert!(matches!(solve_eos(m, energy_for(m, EOS_T_MIN) - 1e12), Err(EosError::BelowDomain { .. })));
        let m = 1_000.0;
        assert!(matches!(solve_eos(m, energy_for(m, EOS_T_MAX) + 1e9), Err(EosError::AboveDomain { .. })));
        assert!(matches!(solve_eos(0.0, 1.0), Err(EosError::NonFinite)));
        // Just inside both ends still solves (for a mass that is two-phase at both).
        let m = 100_000.0;
        assert!(solve_eos(m, energy_for(m, EOS_T_MAX - 0.01)).is_ok());
        assert!(solve_eos(m, energy_for(m, EOS_T_MIN + 0.01)).is_ok());
    }

    #[test]
    fn water_solid_state_reports_compression_pressure() {
        // Astra #4 fixture: 1 % more saturated liquid than the vessel holds at
        // 200 degC; IF97 gives ~16 MPa, the old code reported 1.55 MPa.
        let m = V_VESSEL_FLUID * sat_t(200.0).rho_f * 1.01;
        let eos = solve_eos(m, energy_for(m, 200.0)).unwrap();
        assert_eq!(eos.regime, EosRegime::CompressedLiquid);
        assert!(eos.solid && eos.x == 0.0);
        assert!((eos.t - 200.0).abs() < 1e-3);
        assert!(eos.p > 12.0e6 && eos.p < 20.0e6, "p {}", eos.p);
        assert!((eos.over_fill - 0.01).abs() < 1e-6);
        assert!(eos.p > VESSEL_DESIGN_PRESSURE);
        // Cold overfill is also pressurised, not pinned to atmospheric.
        let m_cold = V_VESSEL_FLUID * steam::rho_liquid(40.0) * 1.005;
        let cold = solve_eos(m_cold, energy_for(m_cold, 40.0)).unwrap();
        assert!(cold.solid && cold.subcooled && cold.p > 5.0e6, "p {}", cold.p);
    }

    #[test]
    fn energy_is_continuous_through_boiling_onset() {
        let m = 100_000.0;
        let gap = energy_for(m, 100.0) - energy_for(m, 99.999);
        assert!(gap.abs() < 1.0e6, "gap {gap}");
        for &t in &[99.9995, 100.0005] {
            let e = energy_for(m, t);
            let eos = solve_eos(m, e).unwrap();
            assert!((eos.t - t).abs() < 1e-6, "t {t} -> {}", eos.t);
            assert!((energy_for(m, eos.t) - e).abs() < 1.0, "residual {}", energy_for(m, eos.t) - e);
        }
        let below = solve_eos(m, energy_for(m, 99.9995)).unwrap();
        let above = solve_eos(m, energy_for(m, 100.0005)).unwrap();
        assert!(below.subcooled && !above.subcooled);
        assert!((above.p - below.p).abs() < 200.0, "pressure jump {} -> {}", below.p, above.p);
    }

    #[test]
    fn core_coverage_is_geometric_fraction_of_mixture_level() {
        let mut v = Vessel::new(sat_p(P_RATED).t, Z_NWL, RATED_POWER_W, STEAM_FLOW_RATED, T_FW_RATED, false);
        for &alpha in &[0.0, 0.4, 0.7] {
            v.alpha_core = alpha;
            v.level_z = Z_CORE_BOTTOM + 0.6 * CORE_HEIGHT;
            assert!((v.core_coverage() - 0.6).abs() < 1e-9, "alpha {alpha}: {}", v.core_coverage());
        }
        v.level_z = Z_TAF + 1.0;
        assert_eq!(v.core_coverage(), 1.0);
        v.level_z = Z_CORE_BOTTOM - 0.1;
        assert_eq!(v.core_coverage(), 0.0);
    }

    #[test]
    fn outflow_is_bounded_by_inventory_and_shortfall_reported() {
        let mut v = vessel_at(40.0, 1000.0);
        let inp = VesselInputs { q_core: 0.0, q_ic: 0.0, w_feed: 0.0, h_feed: 0.0, w_steam: 0.0, w_liquid_out: 2000.0 };
        v.step(1.0, &inp);
        assert!(v.last_error.is_none(), "{:?}", v.last_error);
        assert!(v.mass >= 0.0 && v.mass < 20.0, "mass {}", v.mass);
        assert!(v.flows.w_liquid > 980.0 && v.flows.w_liquid < 1000.0, "accepted {}", v.flows.w_liquid);
        assert!((v.flows.w_liquid + v.flows.shortfall - 2000.0).abs() < 1e-9);
        assert!(v.exhausted && v.shortfall_kg > 1000.0);
        let m1 = v.mass;
        v.step(1.0, &inp);
        assert!(v.last_error.is_none(), "{:?}", v.last_error);
        assert!((v.mass - m1).abs() < 1e-3, "mass manufactured: {m1} -> {}", v.mass);
        assert!(v.flows.shortfall > 1999.0);
    }

    #[test]
    fn step_rejects_out_of_domain_state_and_keeps_previous() {
        let mut v = Vessel::new(sat_p(P_RATED).t, Z_NWL, RATED_POWER_W, STEAM_FLOW_RATED, T_FW_RATED, false);
        let (m0, e0, p0) = (v.mass, v.energy, v.eos.p);
        let inp = VesselInputs { q_core: 1.0e15, q_ic: 0.0, w_feed: 0.0, h_feed: 0.0, w_steam: 0.0, w_liquid_out: 0.0 };
        v.step(1.0, &inp);
        assert!(matches!(v.last_error, Some(EosError::AboveDomain { .. })), "{:?}", v.last_error);
        assert_eq!((v.mass, v.energy, v.eos.p), (m0, e0, p0));
        assert_eq!(v.flows.w_steam, 0.0);
        let msg = v.last_error.as_ref().unwrap().to_string();
        assert!(msg.contains("above EOS maximum"), "{msg}");
    }

    #[test]
    fn ic_ports_move_mass_and_energy_consistently() {
        let mut v = Vessel::new(sat_p(P_RATED).t, Z_NWL, RATED_POWER_W, STEAM_FLOW_RATED, T_FW_RATED, false);
        let (m0, e0) = (v.mass, v.energy);
        let h_g = v.sat().h_g;
        let dt = 0.05;
        let inp = VesselInputs { q_core: 0.0, q_ic: 0.0, w_feed: 0.0, h_feed: 0.0, w_steam: 0.0, w_liquid_out: 0.0 };
        let ports = IcPorts { w_steam: 20.0, w_return: 300.0, h_return: steam::h_liquid(40.0) };
        v.step_with_ic(dt, &inp, &ports);
        assert!(v.last_error.is_none());
        let heat_loss = HEAT_LOSS_HOT * ((v.eos.t - 20.0) / 268.0).max(0.0);
        let dm = (300.0 - 20.0) * dt;
        let de = (300.0 * ports.h_return - 20.0 * h_g) * dt;
        assert!((v.mass - m0 - dm).abs() < 1e-6, "dm {}", v.mass - m0);
        assert!((v.energy - e0 - de).abs() < heat_loss * dt * 1.01 + 1.0, "de {}", v.energy - e0);
        assert_eq!(v.flows.w_ic_steam, 20.0);
        assert_eq!(v.flows.w_ic_return, 300.0);
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
