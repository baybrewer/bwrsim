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

/// Static pressure at the core mid-plane, Pa: dome total pressure plus the
/// in-shroud column above it up to the mixture level `level_z` (upper half of
/// the core at the core-average void, the chimney at its void, liquid above
/// the chimney top, where the separators stand in the downcomer pool).
/// Liquid and vapour densities are those of the bulk inventory. Flow losses
/// are left out (a few kPa at rated flow, zero in the stagnant cold states
/// where the head matters most). Boiling is evaluated at this one elevation
/// for the whole axial core, chimney and loop: the core-average void and the
/// chimney exit share one saturation state, so their energy balances agree.
pub fn core_midplane_pressure(p_dome: f64, level_z: f64, alpha_core: f64, alpha_ch: f64, rho_l: f64, rho_g: f64) -> f64 {
    let span = |lo: f64, hi: f64| (level_z.min(hi) - lo).max(0.0);
    let mix = |a: f64| a * rho_g + (1.0 - a) * rho_l;
    let z_mid = Z_CORE_BOTTOM + 0.5 * CORE_HEIGHT;
    let column = mix(alpha_core) * span(z_mid, Z_TAF)
        + mix(alpha_ch) * span(Z_TAF, Z_CHIMNEY_TOP)
        + rho_l * span(Z_CHIMNEY_TOP, f64::INFINITY);
    p_dome + G * column
}

/// Highest pressure the compressed-liquid closure accepts, Pa (upper bound
/// of IAPWS-IF97 Region 1).
pub const EOS_P_MAX: f64 = 100.0e6;
/// Lowest temperature the EOS solves for, degC (first saturation-table row).
pub const EOS_T_MIN: f64 = 1.0;
/// Highest temperature the EOS solves for, degC (below the table's 350 degC end).
pub const EOS_T_MAX: f64 = 349.5;
/// Specific gas constant of the noncondensable head-space gas (air), J/kgK.
pub const R_AIR: f64 = 287.05;

/// Phase domain the vessel inventory occupies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EosRegime {
    /// Liquid fills the whole vessel; pressure comes from liquid compression.
    CompressedLiquid,
    /// Saturated liquid and vapour in equilibrium (vapour at its saturation
    /// partial pressure; noncondensable gas adds its own partial pressure).
    TwoPhase,
}

/// Gas boundary of the head space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HeadSpace {
    /// Head vent open: below the atmospheric boiling point air fills the
    /// head space to atmospheric total pressure.
    Vented,
    /// Head vent shut: a fixed mass (kg) of noncondensable gas shares the
    /// head space with the vapour (Dalton). Its heat capacity (~1e-4 of the
    /// inventory's) is neglected in the energy balance.
    Sealed { gas_mass: f64 },
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
    /// Liquid compression needs more than `EOS_P_MAX`.
    OverCompressed { t: f64, over_fill: f64 },
    /// Sealed noncondensable gas has no room left (water-solid, or its partial
    /// pressure would exceed `EOS_P_MAX`).
    GasSpaceCollapsed { t: f64, gas_mass: f64 },
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
            Self::OverCompressed { t, over_fill } => write!(
                f,
                "vessel over-filled by {:.2} % at {t:.1} degC, compression beyond {} MPa",
                over_fill * 100.0,
                EOS_P_MAX / 1e6
            ),
            Self::GasSpaceCollapsed { t, gas_mass } => {
                write!(f, "sealed head-space gas ({gas_mass:.1} kg) compressed to no volume at {t:.1} degC")
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
    /// Total head-space (or liquid, when water-solid) pressure, Pa.
    pub p: f64,
    /// Static steam quality of the vessel inventory (steam mass / total).
    pub x: f64,
    /// Below the atmospheric boiling point (T < 100 degC). A plant-mode
    /// (cold) indicator for annunciators and mode-dependent logic only: it is
    /// never a physics gate. Core boiling follows total pressure (`core_sat`),
    /// port selection follows phase inventory (`vapour_available`).
    pub subcooled: bool,
    /// True if the vessel is water-solid (no steam space).
    pub solid: bool,
    pub regime: EosRegime,
    /// Fractional over-fill (M*v_f - V)/V in the compressed-liquid regime, else 0.
    pub over_fill: f64,
    /// Noncondensable partial pressure in the head space, Pa.
    pub p_gas: f64,
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

/// Fluid state of the inventory at one temperature, before the gas boundary.
struct Phase {
    /// Total energy (fluid + metal), J.
    energy: f64,
    x: f64,
    /// Vapour (two-phase) or compressed-liquid pressure, Pa.
    p_fluid: f64,
    regime: EosRegime,
    over_fill: f64,
}

/// Fluid state at temperature `t`. Continuous through 100 degC (the vapour
/// is at its saturation partial pressure below it) and through the
/// two-phase / water-solid boundary (compressed liquid is IF97 Region 1
/// anchored on the saturated state). States outside the domain are errors,
/// never clamped.
fn phase_at(t: f64, mass: f64) -> Result<Phase, EosError> {
    let s = sat_t(t);
    let x = quality_at(&s, mass);
    if x > 1.0 + 1e-9 {
        return Err(EosError::Superheated { t, quality: x });
    }
    if x >= 0.0 {
        let x = x.min(1.0);
        let energy = mass * (s.u_f() + x * (s.u_g() - s.u_f())) + METAL_HEAT_CAP * t;
        return Ok(Phase { energy, x, p_fluid: s.p, regime: EosRegime::TwoPhase, over_fill: 0.0 });
    }
    let over_fill = (mass * s.v_f() - V_VESSEL_FLUID) / V_VESSEL_FLUID;
    let (p, u) = steam::compressed_liquid(t, specific_volume(mass), EOS_P_MAX)
        .map_err(|_| EosError::OverCompressed { t, over_fill })?;
    let energy = mass * u + METAL_HEAT_CAP * t;
    Ok(Phase { energy, x: 0.0, p_fluid: p, regime: EosRegime::CompressedLiquid, over_fill })
}

/// Total pressure and noncondensable partial pressure for a fluid state.
fn head_pressure(t: f64, mass: f64, ph: &Phase, head: HeadSpace) -> Result<(f64, f64), EosError> {
    match (ph.regime, head) {
        // A vented water-solid vessel cannot sit below atmospheric.
        (EosRegime::CompressedLiquid, HeadSpace::Vented) => Ok((ph.p_fluid.max(P_ATM), 0.0)),
        (EosRegime::CompressedLiquid, HeadSpace::Sealed { gas_mass }) => {
            if gas_mass > 0.0 {
                Err(EosError::GasSpaceCollapsed { t, gas_mass })
            } else {
                Ok((ph.p_fluid, 0.0))
            }
        }
        (EosRegime::TwoPhase, HeadSpace::Vented) => {
            let p_gas = (P_ATM - ph.p_fluid).max(0.0);
            Ok((ph.p_fluid + p_gas, p_gas))
        }
        (EosRegime::TwoPhase, HeadSpace::Sealed { gas_mass }) => {
            if gas_mass <= 0.0 {
                return Ok((ph.p_fluid, 0.0));
            }
            let v_space = head_space_volume(t, mass, ph.x);
            let p_gas = gas_mass * R_AIR * (t + 273.15) / v_space;
            if !(v_space > 0.0) || !(ph.p_fluid + p_gas <= EOS_P_MAX) {
                return Err(EosError::GasSpaceCollapsed { t, gas_mass });
            }
            Ok((ph.p_fluid + p_gas, p_gas))
        }
    }
}

/// Volume not occupied by liquid (shared by vapour and noncondensables), m3.
fn head_space_volume(t: f64, mass: f64, x: f64) -> f64 {
    V_VESSEL_FLUID - mass * (1.0 - x) * sat_t(t).v_f()
}

/// Noncondensable mass that fills the head space to atmospheric total
/// pressure when vented (zero at or above the atmospheric boiling point,
/// where vapour purges the air).
pub fn vented_gas_mass(eos: &Eos, mass: f64) -> f64 {
    if eos.solid {
        return 0.0;
    }
    let p_gas = (P_ATM - sat_t(eos.t).p).max(0.0);
    p_gas * head_space_volume(eos.t, mass, eos.x).max(0.0) / (R_AIR * (eos.t + 273.15))
}

/// Solve for temperature/pressure from total fluid mass and energy (incl.
/// metal) with the head space vented. See `solve_eos_with`.
pub fn solve_eos(mass: f64, energy: f64) -> Result<Eos, EosError> {
    solve_eos_with(mass, energy, HeadSpace::Vented)
}

/// Solve for temperature/pressure from total fluid mass and energy (incl.
/// metal) and the head-space gas boundary. Errors when the pair lies outside
/// the saturated/compressed-liquid domain; callers must keep their previous
/// state in that case.
pub fn solve_eos_with(mass: f64, energy: f64, head: HeadSpace) -> Result<Eos, EosError> {
    if !(mass > 0.0) || !mass.is_finite() || !energy.is_finite() {
        return Err(EosError::NonFinite);
    }
    // Inadmissible states (superheat, over-compression) lie at the hot end
    // for a fixed mass, so the lower bracket end must be admissible.
    let min_energy = phase_at(EOS_T_MIN, mass)?.energy;
    if energy < min_energy {
        return Err(EosError::BelowDomain { energy, min_energy });
    }
    let top = phase_at(EOS_T_MAX, mass);
    if let Ok(ph) = &top {
        if energy > ph.energy {
            return Err(EosError::AboveDomain { energy, max_energy: ph.energy });
        }
    }
    // Bisect on temperature; an inadmissible midpoint counts as too hot and
    // its error is reported if the energy lies beyond the admissible states.
    let mut beyond = top.err();
    let (mut lo, mut hi) = (EOS_T_MIN, EOS_T_MAX);
    for _ in 0..80 {
        let mid = 0.5 * (lo + hi);
        match phase_at(mid, mass) {
            Ok(ph) if ph.energy < energy => lo = mid,
            Ok(_) => hi = mid,
            Err(e) => {
                hi = mid;
                beyond = Some(e);
            }
        }
    }
    let ph = phase_at(lo, mass)?;
    let residual = ph.energy - energy;
    if residual.abs() > 1e-6 * energy.abs() + 1.0 {
        return Err(beyond.unwrap_or(EosError::NoConvergence { residual }));
    }
    let (p, p_gas) = head_pressure(lo, mass, &ph, head)?;
    let regime = ph.regime;
    Ok(Eos {
        t: lo,
        p,
        x: ph.x,
        subcooled: lo < 100.0,
        solid: regime == EosRegime::CompressedLiquid,
        regime,
        over_fill: ph.over_fill,
        p_gas,
    })
}

/// Total energy (fluid + metal) for a fluid mass at temperature t.
pub fn energy_for(mass: f64, t: f64) -> Result<f64, EosError> {
    phase_at(t, mass).map(|ph| ph.energy)
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
    /// Main steam line water-vapour flow, kg/s, and its enthalpy (dome
    /// vapour; zero flow when the vessel is water-solid).
    pub w_steam: f64,
    pub h_steam: f64,
    /// Noncondensable gas carried with it at the dome composition, kg/s.
    pub w_steam_gas: f64,
    /// RWCU reject flow and enthalpy.
    pub w_liquid: f64,
    pub h_liquid: f64,
    /// Head vent flow and enthalpy.
    pub w_vent: f64,
    pub h_vent: f64,
    /// Accepted isolation-condenser steam draw and condensate return, kg/s,
    /// and the return enthalpy, J/kg.
    pub w_ic_steam: f64,
    pub w_ic_return: f64,
    pub h_ic_return: f64,
    /// Accepted / requested IC transfer (1.0 when nothing was requested).
    /// Both IC ports are scaled by it; the trains must commit with it.
    pub ic_ratio: f64,
    /// Noncondensable gas expelled through the open head vent, kg/s
    /// (negative when air is drawn in).
    pub w_vent_gas: f64,
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
    /// Noncondensable gas in the head space, kg. Tracks the atmosphere while
    /// the head vent is open and is trapped when it shuts.
    pub gas_mass: f64,
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
            eos: Eos {
                t,
                p: P_ATM,
                x: 0.0,
                subcooled: true,
                solid: false,
                regime: EosRegime::TwoPhase,
                over_fill: 0.0,
                p_gas: 0.0,
            },
            core_flow: CORE_FLOW_RATED,
            h_in: steam::h_liquid(t),
            alpha_core: 0.0,
            alpha_ch: 0.0,
            core: CoreTh::default(),
            level_z,
            head_vent_open: vent_open,
            gas_mass: 0.0,
            vent_flow: 0.0,
            heatup_rate: 0.0,
            k_fric: Self::calibrate_friction(),
            drive_dp: 0.0,
            last_error: None,
            flows: VesselFlows::default(),
            exhausted: false,
            shortfall_kg: 0.0,
        };
        // Iterate flow, voids and inventory to a consistent state. Core boiling
        // uses the initial total pressure plus the column above the core
        // (`core_sat`): 1 atm in the head space below 100 degC, where air fills
        // it, the vapour pressure above it.
        let s_bulk = sat_t(t);
        let p_dome = if t < 100.0 { P_ATM } else { s_bulk.p };
        let rho_bulk = if t < 100.0 { steam::rho_liquid(t) } else { s_bulk.rho_f };
        let h_fw = steam::h_liquid(t_feed);
        for _ in 0..200 {
            let s = sat_p(core_midplane_pressure(p_dome, level_z, v.alpha_core, v.alpha_ch, rho_bulk, s_bulk.rho_g));
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
        let rho_l = rho_bulk;
        let v_g = V_VESSEL_FLUID - v_l;
        // Head space holds equilibrium vapour at every temperature (partial
        // pressure below 100 degC), matching the EOS closure.
        let m_g = v_g * sat_t(t).rho_g;
        v.mass = v_l * rho_l + m_g;
        v.energy = energy_for(v.mass, t).expect("initial vessel state must be inside the EOS domain");
        // The head space starts at atmospheric total pressure below 100 degC,
        // vented or sealed; above it the vapour has purged the air.
        v.eos = solve_eos(v.mass, v.energy).expect("initial vessel state must be inside the EOS domain");
        v.gas_mass = vented_gas_mass(&v.eos, v.mass);
        v.update_level();
        v
    }

    fn calibrate_friction() -> f64 {
        // Rated state: 7.2 MPa dome, 870 MW, rated feed at 215 degC, level at
        // NWL. Boiling at the core mid-plane pressure, as in `Vessel::step`;
        // recirculated liquid returns at the dome saturation enthalpy.
        let dome = sat_p(P_RATED);
        let w = CORE_FLOW_RATED;
        let h_fw = steam::h_liquid(T_FW_RATED);
        let h_in = (dome.h_f * (w - STEAM_FLOW_RATED) + h_fw * STEAM_FLOW_RATED) / w;
        let mut s = dome;
        for _ in 0..50 {
            let core = core_th(&s, w, RATED_POWER_W, h_in);
            let (a_ch, _) = chimney_th(&s, w, &core);
            s = sat_p(core_midplane_pressure(P_RATED, Z_NWL, core.alpha_avg, a_ch, dome.rho_f, dome.rho_g));
        }
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
        // Same liquid density the EOS lever rule uses, at every temperature.
        let rho_l = sat_t(self.eos.t).rho_f;
        let v_l = self.mass * (1.0 - self.eos.x) / rho_l;
        self.level_z = level_from_volume(v_l, self.alpha_core, self.alpha_ch);
    }

    /// Water-vapour saturation properties of the equilibrium inventory: at the
    /// vessel temperature, where vapour sits at its own saturation (partial)
    /// pressure. Total pressure (`eos.p`, including trapped noncondensables)
    /// is the mechanical quantity for relief setpoints, line flow and display;
    /// it never sets a port enthalpy. Core boiling uses `core_sat`.
    pub fn sat(&self) -> Sat {
        sat_t(self.eos.t)
    }

    /// Enthalpy (J/kg) of vapour leaving through any steam port (main steam,
    /// head vent, IC). IC requests at `eos.t` receive exactly this value.
    pub fn steam_port_enthalpy(&self) -> f64 {
        self.sat().h_g
    }

    /// Enthalpy (J/kg) of liquid leaving the vessel: the compressed state the
    /// EOS stores when water-solid, saturated liquid at `eos.t` otherwise.
    pub fn liquid_port_enthalpy(&self) -> f64 {
        if self.eos.solid {
            steam::compressed_liquid_h(self.eos.t, self.eos.p)
        } else {
            self.sat().h_f
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
        let t_before = self.eos.t;
        let (solid, x) = (self.eos.solid, self.eos.x);

        let h_steam = self.steam_port_enthalpy();
        let h_liq = self.liquid_port_enthalpy();
        let heat_loss = HEAT_LOSS_HOT * ((self.eos.t - 20.0) / 268.0).max(0.0);

        // Head vent: steam escapes while above atmospheric; a water-solid
        // vessel relieves liquid through it instead. Both terms follow the
        // mechanical state (zero at or below 1 atm / 100 degC), not a mode flag.
        let vent_req = if self.head_vent_open {
            (C_HEAD_VENT * (self.eos.p - P_ATM)).max(0.0) + 0.02 * (self.eos.t - 100.0).max(0.0)
        } else {
            0.0
        };
        // The main steam lines draw dome gas only. A water-solid vessel has no
        // steam space and delivers nothing to them: the plant trips turbine and
        // feed at L8 long before carryover, so no liquid line path is modelled.
        // The request (`inp.w_steam`) is the line's total mass flow of dome gas,
        // which leaves at the dome composition: noncondensables go with the
        // vapour in the dome's mass ratio (no selective separator), so a sealed
        // vessel purges its air through an open line. A head vent only flows
        // with no air in the dome (at or above 100 degC, or above 1 atm when
        // vented), so its stream is vapour alone.
        let m_vap = self.mass * x;
        let m_liq = self.mass - m_vap;
        let line_req = if solid { 0.0 } else { inp.w_steam };
        let gas_frac = if line_req > 0.0 { self.dome_gas_mass_fraction() } else { 0.0 };
        let steam_req = line_req * (1.0 - gas_frac);
        let line_gas_req = line_req * gas_frac;
        let h_vent_port = if solid { h_liq } else { h_steam };
        let ic_steam_req = if solid { 0.0 } else { ic.w_steam };

        // Bound each phase port by the phase inventory, then the total by the
        // smallest inventory the EOS can represent.
        let (vap_req, liq_req) = if solid {
            (0.0, inp.w_liquid_out + vent_req)
        } else {
            (steam_req + vent_req + ic_steam_req, inp.w_liquid_out)
        };
        let scale = |req: f64, avail: f64| if req * dt > avail { (avail / (req * dt)).max(0.0) } else { 1.0 };
        let f_vap = scale(vap_req, m_vap);
        let f_liq = scale(liq_req, m_liq);
        let out_phase = vap_req * f_vap + liq_req * f_liq;
        // The IC return is left out of the available total: it is itself
        // scaled by the IC acceptance below (no steam drawn, no drain).
        let avail_total = (self.mass + inp.w_feed * dt - 1.001 * self.min_mass()).max(0.0);
        let f_tot = scale(out_phase, avail_total);
        let (f_v, f_l) = (f_vap * f_tot, f_liq * f_tot);
        let f_vent = if solid { f_l } else { f_v };
        // IC acceptance: the condenser loop runs only as fast as the vessel
        // supplies its steam (none when water-solid), and the condensate
        // return is scaled with it so the trains can commit one ratio.
        let ic_ratio = if ic.w_steam > 0.0 { ic_steam_req * f_v / ic.w_steam } else { 1.0 };
        let w_ic_return = ic.w_return * ic_ratio;
        let flows = VesselFlows {
            w_feed: inp.w_feed,
            w_steam: steam_req * f_v,
            // The line keeps the dome composition at the accepted rate.
            w_steam_gas: (line_gas_req * f_v).min(self.gas_mass / dt),
            h_steam,
            w_liquid: inp.w_liquid_out * f_l,
            h_liquid: h_liq,
            w_vent: vent_req * f_vent,
            h_vent: h_vent_port,
            w_ic_steam: ic.w_steam * ic_ratio,
            w_ic_return,
            h_ic_return: ic.h_return,
            ic_ratio,
            w_vent_gas: 0.0,
            shortfall: (steam_req + vent_req + ic_steam_req + inp.w_liquid_out) - out_phase * f_tot,
        };

        let dm = inp.w_feed + flows.w_ic_return - flows.w_steam - flows.w_liquid - flows.w_vent - flows.w_ic_steam;
        let de = inp.q_core - inp.q_ic - heat_loss
            + inp.w_feed * inp.h_feed
            + flows.w_ic_return * flows.h_ic_return
            - flows.w_steam * flows.h_steam
            - flows.w_vent * flows.h_vent
            - flows.w_ic_steam * h_steam
            - flows.w_liquid * flows.h_liquid;
        let mass = self.mass + dm * dt;
        let energy = self.energy + de * dt;
        // Noncondensables carry no energy here (heat capacity neglected, see
        // `HeadSpace::Sealed`); only their inventory moves.
        let gas_after_line = (self.gas_mass - flows.w_steam_gas * dt).max(0.0);
        let head = if self.head_vent_open { HeadSpace::Vented } else { HeadSpace::Sealed { gas_mass: gas_after_line } };
        let mut flows = flows;
        match solve_eos_with(mass, energy, head) {
            Ok(eos) => {
                self.mass = mass;
                self.energy = energy;
                self.eos = eos;
                self.last_error = None;
                if self.head_vent_open {
                    let gas = vented_gas_mass(&eos, mass);
                    flows.w_vent_gas = (gas_after_line - gas) / dt;
                    self.gas_mass = gas;
                } else {
                    self.gas_mass = gas_after_line;
                }
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
        let s = self.core_sat();

        // Core inlet enthalpy: separated liquid mixed in the downcomer with the
        // accepted cold inflows (feedwater and IC condensate return).
        let w = self.core_flow.max(W_MIN);
        let w_cold = flows.w_feed + flows.w_ic_return;
        let h_cold = if w_cold > 0.0 {
            (flows.w_feed * inp.h_feed + flows.w_ic_return * flows.h_ic_return) / w_cold
        } else {
            0.0
        };
        let w_cold = w_cold.min(w);
        // Separated / recirculating liquid is at the bulk vessel temperature.
        let h_ret = steam::h_liquid(self.eos.t).min(s.h_f);
        let h_in_target = ((w - w_cold) * h_ret + w_cold * h_cold) / w;
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

    /// Saturation state for core boiling: bubbles nucleate against the local
    /// mechanical pressure, dome total pressure (including trapped gas) plus
    /// the column above the core mid-plane (`core_midplane_pressure`). In a cold
    /// stagnant vessel that head (~1.1 bar) dominates; at rated pressure it is
    /// under 1 % (round-5 R3/R5). Uses the lagged voids and the current level.
    pub fn core_sat(&self) -> Sat {
        sat_p(self.core_pressure())
    }

    /// Core mid-plane pressure, Pa; see `core_midplane_pressure`.
    pub fn core_pressure(&self) -> f64 {
        let s = self.sat();
        core_midplane_pressure(self.eos.p, self.level_z, self.alpha_core, self.alpha_ch, s.rho_f, s.rho_g)
    }

    /// Vapour the steam ports (main steam, IC) can draw: present at any
    /// temperature in the equilibrium EOS, absent when water-solid.
    pub fn vapour_available(&self) -> bool {
        !self.eos.solid && self.eos.x > 0.0
    }

    /// Noncondensable mass fraction of the dome gas: the composition every main
    /// steam line withdrawal carries (no selective separator).
    pub fn dome_gas_mass_fraction(&self) -> f64 {
        if self.gas_mass > 0.0 { self.gas_mass / (self.gas_mass + self.mass * self.eos.x) } else { 0.0 }
    }

    /// Density (kg/m3) of the dome gas entering the main steam lines: the
    /// vapour plus the noncondensables sharing the head space. Unused when
    /// water-solid (the vessel delivers nothing to the lines then).
    pub fn steam_line_density(&self) -> f64 {
        let v_space = head_space_volume(self.eos.t, self.mass, self.eos.x);
        let rho_gas = if v_space > 0.0 { self.gas_mass / v_space } else { 0.0 };
        self.sat().rho_g + rho_gas
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vessel_at(t: f64, mass: f64) -> Vessel {
        let mut v = Vessel::new(t.min(99.0), Z_NWL, 0.0, 0.0, 40.0, false);
        v.mass = mass;
        v.energy = energy_for(mass, t).unwrap();
        v.eos = solve_eos(mass, v.energy).unwrap();
        // Head-space gas as if vented at the new state (none above 100 degC).
        v.gas_mass = vented_gas_mass(&v.eos, mass);
        v.update_level();
        v
    }

    #[test]
    fn eos_roundtrip() {
        // Physically admissible two-phase fixtures: v_f < V/M < v_g at each temperature.
        for &(t, m) in &[(40.0, 200_000.0), (150.0, 200_000.0), (287.0, 150_000.0)] {
            let e = energy_for(m, t).unwrap();
            let eos = solve_eos(m, e).unwrap();
            assert!((eos.t - t).abs() < 1e-3, "t {t} -> {}", eos.t);
            assert_eq!(eos.regime, EosRegime::TwoPhase);
            let s = sat_t(eos.t);
            let occupied = m * ((1.0 - eos.x) * s.v_f() + eos.x * s.v_g());
            assert!((occupied - V_VESSEL_FLUID).abs() < 1e-6 * V_VESSEL_FLUID, "volume {occupied}");
            assert!((energy_for(m, eos.t).unwrap() - e).abs() < 1e-6 * e.abs());
        }
    }

    #[test]
    fn eos_rejects_compression_beyond_region1() {
        // 200 t at 287 degC overfills the vessel by ~6 %: a legitimate
        // compressed-liquid state (tens of MPa), not a two-phase one.
        let m = 200_000.0;
        let eos = solve_eos(m, energy_for(m, 287.0).unwrap()).unwrap();
        assert_eq!(eos.regime, EosRegime::CompressedLiquid);
        assert!(eos.p > 20.0e6 && eos.p < EOS_P_MAX, "p {}", eos.p);
        // 240 t at 287 degC needs ~22 % compression, beyond Region 1's 100 MPa.
        let m = 240_000.0;
        assert!(matches!(energy_for(m, 287.0), Err(EosError::OverCompressed { .. })));
        let e = m * sat_t(287.0).u_f() + METAL_HEAT_CAP * 287.0;
        match solve_eos(m, e) {
            // Reported at the hottest admissible temperature (100 MPa there).
            Err(EosError::OverCompressed { over_fill, .. }) => assert!(over_fill > 0.05, "over {over_fill}"),
            other => panic!("expected OverCompressed, got {other:?}"),
        }
    }

    #[test]
    fn eos_rejects_energy_outside_bracket() {
        let m = 100_000.0;
        let e_max = energy_for(m, EOS_T_MAX).unwrap();
        let e_min = energy_for(m, EOS_T_MIN).unwrap();
        assert!(matches!(solve_eos(m, e_max + 1e12), Err(EosError::AboveDomain { .. })));
        assert!(matches!(solve_eos(m, e_min - 1e12), Err(EosError::BelowDomain { .. })));
        assert!(matches!(solve_eos(0.0, 1.0), Err(EosError::NonFinite)));
        // Just inside both ends still solves (for a mass that is two-phase at both).
        assert!(solve_eos(m, energy_for(m, EOS_T_MAX - 0.01).unwrap()).is_ok());
        assert!(solve_eos(m, energy_for(m, EOS_T_MIN + 0.01).unwrap()).is_ok());
    }

    /// Sol rereview / Astra 10: the hot bracket end is no longer evaluated
    /// with a clamped quality. A light inventory is superheated there and a
    /// heavy one over-compressed; energies past the admissible states report
    /// that reason instead of a fictitious AboveDomain maximum.
    #[test]
    fn hot_bracket_end_reports_the_inadmissible_phase() {
        let m = 1_000.0;
        assert!(matches!(energy_for(m, EOS_T_MAX), Err(EosError::Superheated { .. })));
        let e = m * sat_t(EOS_T_MAX).u_g() + METAL_HEAT_CAP * EOS_T_MAX;
        assert!(matches!(solve_eos(m, e), Err(EosError::Superheated { .. })), "{:?}", solve_eos(m, e));
        // Inside the two-phase range the same light inventory still solves.
        let ok = solve_eos(m, m * sat_t(150.0).u_g() + METAL_HEAT_CAP * 150.0).unwrap();
        assert!(ok.x > 0.5 && ok.x <= 1.0, "{ok:?}");
        let m = 230_000.0;
        assert!(matches!(energy_for(m, EOS_T_MAX), Err(EosError::OverCompressed { .. })));
        let e = m * sat_t(300.0).u_f() + METAL_HEAT_CAP * 300.0;
        assert!(matches!(solve_eos(m, e), Err(EosError::OverCompressed { .. })), "{:?}", solve_eos(m, e));
    }

    /// Sol 5 / Astra 4 / Sol 23: water-solid state against independent
    /// IAPWS-IF97 Region 1 solves of fixed mass, volume and energy (both
    /// re-reviews). Tolerances (5 mK, 10 kPa) cover the 1 degC table
    /// interpolation the model anchors on; it lands within 0.5 kPa.
    #[test]
    fn compressed_liquid_matches_independent_if97_solve() {
        // Astra re-review: M, E given; IF97 gives 201.858653 degC / 16.273779 MPa.
        let (m, e) = (219_799.661608, 241_960_515_187.185);
        let eos = solve_eos(m, e).unwrap();
        assert_eq!(eos.regime, EosRegime::CompressedLiquid);
        assert!(eos.solid && eos.x == 0.0);
        assert!((eos.t - 201.858653).abs() < 0.005, "t {}", eos.t);
        assert!((eos.p - 16.273779e6).abs() < 0.01e6, "p {}", eos.p);
        // The solved state occupies the vessel at the solved (T, p).
        let v = m * steam::region1(eos.t, eos.p).v;
        assert!((v - V_VESSEL_FLUID).abs() < 1e-3 * V_VESSEL_FLUID, "volume {v}");
        assert!(eos.p > VESSEL_DESIGN_PRESSURE);
        // Sol re-review: 1 % over-fill at 287 degC with saturated-liquid energy;
        // IF97 gives 288.847 degC / 13.408 MPa.
        let s = sat_t(287.0);
        let m = 1.01 * V_VESSEL_FLUID * s.rho_f;
        let e = m * s.u_f() + METAL_HEAT_CAP * 287.0;
        let eos = solve_eos(m, e).unwrap();
        assert!((eos.t - 288.847).abs() < 0.005, "t {}", eos.t);
        assert!((eos.p - 13.408e6).abs() < 0.01e6, "p {}", eos.p);
    }

    #[test]
    fn water_solid_state_is_continuous_with_two_phase() {
        let t = 200.0;
        let m_full = V_VESSEL_FLUID * sat_t(t).rho_f;
        let below = solve_eos(m_full * (1.0 - 1e-9), energy_for(m_full * (1.0 - 1e-9), t).unwrap()).unwrap();
        let above = solve_eos(m_full * (1.0 + 1e-9), energy_for(m_full * (1.0 + 1e-9), t).unwrap()).unwrap();
        assert_eq!(below.regime, EosRegime::TwoPhase);
        assert_eq!(above.regime, EosRegime::CompressedLiquid);
        assert!((above.p - below.p).abs() < 5.0e3, "{} -> {}", below.p, above.p);
        // Cold overfill is pressurised, not pinned to atmospheric.
        let m_cold = V_VESSEL_FLUID * steam::rho_liquid(40.0) * 1.005;
        let cold = solve_eos(m_cold, energy_for(m_cold, 40.0).unwrap()).unwrap();
        assert!(cold.solid && cold.subcooled && cold.p > 5.0e6, "p {}", cold.p);
    }

    /// Astra 9: below 100 degC a vented head space sits at atmospheric while a
    /// sealed one carries vapour plus trapped-gas partial pressure.
    #[test]
    fn cold_gas_boundary_depends_on_vent_state() {
        let m = 200_000.0;
        let e40 = energy_for(m, 40.0).unwrap();
        let at40 = solve_eos(m, e40).unwrap();
        assert_eq!(at40.p, P_ATM);
        let gas = vented_gas_mass(&at40, m);
        assert!(gas > 10.0, "air {gas} kg");
        // Sealing at 40 degC keeps atmospheric total pressure at that state.
        let sealed40 = solve_eos_with(m, e40, HeadSpace::Sealed { gas_mass: gas }).unwrap();
        assert!((sealed40.p - P_ATM).abs() < 1e-6 * P_ATM, "p {}", sealed40.p);
        // Heat to 90 degC at constant mass.
        let e90 = energy_for(m, 90.0).unwrap();
        let vented = solve_eos_with(m, e90, HeadSpace::Vented).unwrap();
        let sealed = solve_eos_with(m, e90, HeadSpace::Sealed { gas_mass: gas }).unwrap();
        assert_eq!(vented.p, P_ATM);
        // Independent ideal-gas + Dalton check at the solved state.
        let s = sat_t(sealed.t);
        let v_space = V_VESSEL_FLUID - m * (1.0 - sealed.x) / s.rho_f;
        let p_expect = s.p + gas * R_AIR * (sealed.t + 273.15) / v_space;
        assert!((sealed.p - p_expect).abs() < 1e-9 * p_expect, "{} vs {p_expect}", sealed.p);
        assert!(sealed.p > 1.2 * P_ATM, "p {}", sealed.p);
        // Sealed without gas (closed hot, then cooled) is at vapour pressure.
        let no_gas = solve_eos_with(m, e90, HeadSpace::Sealed { gas_mass: 0.0 }).unwrap();
        assert!((no_gas.p - sat_t(no_gas.t).p).abs() < 1.0 && no_gas.p < P_ATM);
        // Trapped gas cannot be compressed into a water-solid vessel.
        let m_solid = V_VESSEL_FLUID * steam::rho_liquid(60.0) * 1.002;
        let e = energy_for(m_solid, 60.0).unwrap();
        assert!(matches!(
            solve_eos_with(m_solid, e, HeadSpace::Sealed { gas_mass: gas }),
            Err(EosError::GasSpaceCollapsed { .. })
        ));
    }

    #[test]
    fn vessel_traps_gas_when_sealed_and_vents_it_when_open() {
        let heat = VesselInputs { q_core: 20.0e6, q_ic: 0.0, w_feed: 0.0, h_feed: 0.0, w_steam: 0.0, w_liquid_out: 0.0 };
        let mut vented = Vessel::new(40.0, Z_NWL, 0.0, 0.0, 40.0, true);
        let mut sealed = Vessel::new(40.0, Z_NWL, 0.0, 0.0, 40.0, false);
        let gas0 = sealed.gas_mass;
        assert!(gas0 > 10.0 && (vented.gas_mass - gas0).abs() < 1e-9);
        let mut expelled = 0.0;
        while vented.eos.t < 90.0 {
            vented.step(1.0, &heat);
            sealed.step(1.0, &heat);
            assert!(vented.last_error.is_none() && sealed.last_error.is_none());
            expelled += vented.flows.w_vent_gas;
        }
        assert_eq!(vented.eos.p, P_ATM);
        assert!(sealed.eos.p > 1.2 * P_ATM, "sealed p {}", sealed.eos.p);
        assert_eq!(sealed.gas_mass, gas0);
        assert!(vented.gas_mass < gas0 && (gas0 - vented.gas_mass - expelled).abs() < 1e-6, "expelled {expelled}");
    }

    #[test]
    fn energy_is_continuous_through_boiling_onset() {
        let m = 100_000.0;
        let gap = energy_for(m, 100.0).unwrap() - energy_for(m, 99.999).unwrap();
        assert!(gap.abs() < 1.0e6, "gap {gap}");
        for &t in &[99.9995, 100.0005] {
            let e = energy_for(m, t).unwrap();
            let eos = solve_eos(m, e).unwrap();
            assert!((eos.t - t).abs() < 1e-6, "t {t} -> {}", eos.t);
            let residual = energy_for(m, eos.t).unwrap() - e;
            assert!(residual.abs() < 1.0, "residual {residual}");
        }
        let below = solve_eos(m, energy_for(m, 99.9995).unwrap()).unwrap();
        let above = solve_eos(m, energy_for(m, 100.0005).unwrap()).unwrap();
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

    /// Astra 19 / Sol N1: IC transfers the vessel cannot supply are refused,
    /// reported with the ratio the trains commit with, and the combined
    /// vessel + IC inventory is conserved.
    fn ic_round_trip(v: &mut Vessel, trains: &mut [crate::ic::IcTrain], dt: f64) -> (f64, f64) {
        let t_sat = v.eos.t;
        // Ask as if vapour were available: these tests check that the vessel's
        // phase-inventory acceptance, not the caller's gate, refuses the draw.
        let reqs: Vec<_> = trains.iter().map(|t| t.request(dt, t_sat, true)).collect();
        let mut ports = IcPorts::default();
        let mut h_sum = 0.0;
        for r in &reqs {
            ports.w_steam += r.w_steam_in;
            ports.w_return += r.w_cond_return;
            h_sum += r.w_cond_return * r.h_return;
        }
        ports.h_return = if ports.w_return > 0.0 { h_sum / ports.w_return } else { 0.0 };
        let inp = VesselInputs { q_core: 0.0, q_ic: 0.0, w_feed: 0.0, h_feed: 0.0, w_steam: 0.0, w_liquid_out: 0.0 };
        v.step_with_ic(dt, &inp, &ports);
        assert!(v.last_error.is_none(), "{:?}", v.last_error);
        let ratio = v.flows.ic_ratio;
        let mut q_pool = 0.0;
        for (t, r) in trains.iter_mut().zip(reqs) {
            q_pool += t.commit(dt, r.scaled(ratio)).q_pool;
        }
        (ports.w_steam, q_pool)
    }

    fn open_trains() -> Vec<crate::ic::IcTrain> {
        (0..3)
            .map(|_| {
                let mut t = crate::ic::IcTrain::new();
                t.cmd_open = true;
                t.valve_pos = 1.0;
                t.tube_mass = 500.0;
                t.heat = IC_TRAIN_RATED_W;
                t
            })
            .collect()
    }

    fn primary_inventory(v: &Vessel, trains: &[crate::ic::IcTrain]) -> (f64, f64) {
        let m = v.mass + trains.iter().map(|t| t.tube_mass).sum::<f64>();
        let e = v.energy + trains.iter().map(|t| t.tube_mass * t.tube_h).sum::<f64>();
        (m, e)
    }

    #[test]
    fn water_solid_vessel_refuses_ic_transfers_and_trains_commit_nothing() {
        let t = 200.0;
        let m = V_VESSEL_FLUID * sat_t(t).rho_f * 1.005;
        let mut v = vessel_at(t, m);
        assert!(v.eos.solid);
        let mut trains = open_trains();
        let (m0, e0) = primary_inventory(&v, &trains);
        let (requested, q_pool) = ic_round_trip(&mut v, &mut trains, 0.05);
        assert!(requested > 10.0);
        assert_eq!(v.flows.ic_ratio, 0.0);
        assert_eq!((v.flows.w_ic_steam, v.flows.w_ic_return, q_pool), (0.0, 0.0, 0.0));
        let (m1, e1) = primary_inventory(&v, &trains);
        let loss = HEAT_LOSS_HOT * ((v.eos.t - 20.0) / 268.0).max(0.0) * 0.05;
        assert!((m1 - m0).abs() < 1e-6, "mass created {}", m1 - m0);
        assert!((e1 - e0 + loss).abs() < 1.0e3, "energy residual {}", e1 - e0 + loss);
    }

    #[test]
    fn vapour_limited_ic_draw_is_scaled_and_conserved() {
        // Nearly full vessel: a few kg of vapour against ~60 kg/s requested.
        let t = 200.0;
        let m = V_VESSEL_FLUID * sat_t(t).rho_f * (1.0 - 1e-6);
        let mut v = vessel_at(t, m);
        assert!(!v.eos.solid && v.mass * v.eos.x < 1.0, "vapour {}", v.mass * v.eos.x);
        let mut trains = open_trains();
        let (m0, e0) = primary_inventory(&v, &trains);
        let ret_req: f64 = trains.iter().map(|tr| tr.request(0.05, v.eos.t, true).w_cond_return).sum();
        let (requested, q_pool) = ic_round_trip(&mut v, &mut trains, 0.05);
        let f = v.flows;
        assert!(f.ic_ratio > 0.0 && f.ic_ratio < 0.1, "ratio {}", f.ic_ratio);
        assert!((f.w_ic_steam - f.ic_ratio * requested).abs() < 1e-9);
        assert!((f.w_ic_return - f.ic_ratio * ret_req).abs() < 1e-9);
        assert!(f.h_ic_return > 0.0);
        let committed: f64 = trains.iter().map(|t| t.last.w_steam_in).sum();
        assert!((committed - f.w_ic_steam).abs() < 1e-9, "trains {committed} vs vessel {}", f.w_ic_steam);
        let (m1, e1) = primary_inventory(&v, &trains);
        let loss = HEAT_LOSS_HOT * ((v.eos.t - 20.0) / 268.0).max(0.0) * 0.05;
        assert!((m1 - m0).abs() < 1e-6, "mass created {}", m1 - m0);
        // Combined primary energy falls by exactly the pool duty and ambient loss.
        assert!((e1 - e0 + q_pool * 0.05 + loss).abs() < 1.0e3, "energy residual {}", e1 - e0 + q_pool * 0.05 + loss);
    }

    /// R1: with trapped head gas the vessel and the IC trains exchange one
    /// steam enthalpy. Sealed 150 degC inventory, 50 kg air, core heat on, all
    /// ICs open: vessel + IC primary energy changes by exactly core heat minus
    /// pool duty and ambient loss, summed independently each step.
    #[test]
    fn gas_bearing_heatup_with_ic_closes_primary_energy() {
        let mut v = vessel_at(150.0, 180_000.0);
        v.head_vent_open = false;
        v.gas_mass = 50.0;
        v.eos = solve_eos_with(v.mass, v.energy, HeadSpace::Sealed { gas_mass: 50.0 }).unwrap();
        assert!(v.eos.p_gas > 5.0e4 && sat_p(v.eos.p).t > v.eos.t + 5.0, "{:?}", v.eos);
        let mut trains = open_trains();
        let (dt, q_core) = (0.05, 20.0e6);
        let (m0, e0) = primary_inventory(&v, &trains);
        let mut external = 0.0;
        for _ in 0..20 {
            let t0 = v.eos.t;
            let h_port = v.steam_port_enthalpy();
            let reqs: Vec<_> = trains.iter().map(|t| t.request(dt, t0, v.vapour_available())).collect();
            assert!(reqs.iter().all(|r| r.w_steam_in == 0.0 || r.h_steam_in == h_port));
            let mut ports = IcPorts::default();
            let mut h_sum = 0.0;
            for r in &reqs {
                ports.w_steam += r.w_steam_in;
                ports.w_return += r.w_cond_return;
                h_sum += r.w_cond_return * r.h_return;
            }
            ports.h_return = if ports.w_return > 0.0 { h_sum / ports.w_return } else { 0.0 };
            let inp = VesselInputs { q_core, q_ic: 0.0, w_feed: 0.0, h_feed: 0.0, w_steam: 0.0, w_liquid_out: 0.0 };
            v.step_with_ic(dt, &inp, &ports);
            assert!(v.last_error.is_none(), "{:?}", v.last_error);
            let ratio = v.flows.ic_ratio;
            let q_pool: f64 = trains.iter_mut().zip(reqs).map(|(t, r)| t.commit(dt, r.scaled(ratio)).q_pool).sum();
            let loss = HEAT_LOSS_HOT * ((t0 - 20.0) / 268.0).max(0.0);
            external += (q_core - q_pool - loss) * dt;
        }
        assert!(v.flows.w_ic_steam > 1.0, "IC idle: {:?}", v.flows);
        let (m1, e1) = primary_inventory(&v, &trains);
        assert!((m1 - m0).abs() < 1e-6, "mass created {}", m1 - m0);
        assert!((e1 - e0 - external).abs() < 10.0, "energy residual {} J", e1 - e0 - external);
    }

    /// R3: liquid withdrawn from a water-solid vessel carries the compressed
    /// state's enthalpy. Independent IF97 Region 1 at the reviewers' fixture
    /// (M = 1.002 V rho_f(40 C): 40 C, 4.562267 MPa) is 171573.418 J/kg.
    #[test]
    fn compressed_liquid_withdrawal_uses_pressure_dependent_enthalpy() {
        let mut v = vessel_at(40.0, 1.002 * V_VESSEL_FLUID * sat_t(40.0).rho_f);
        assert!(v.eos.solid && (v.eos.p - 4.562267e6).abs() < 2.0e4, "{:?}", v.eos);
        let (m0, e0, dt) = (v.mass, v.energy, 0.05);
        let inp = VesselInputs { q_core: 0.0, q_ic: 0.0, w_feed: 0.0, h_feed: 0.0, w_steam: 0.0, w_liquid_out: 10.0 };
        v.step(dt, &inp);
        assert!(v.last_error.is_none(), "{:?}", v.last_error);
        assert!((v.flows.h_liquid - 171_573.418).abs() < 50.0, "h_liquid {}", v.flows.h_liquid);
        let loss = HEAT_LOSS_HOT * ((40.0 - 20.0) / 268.0) * dt;
        assert!((v.mass - m0 + 10.0 * dt).abs() < 1e-9);
        assert!((v.energy - e0 + 10.0 * dt * v.flows.h_liquid + loss).abs() < 1e-3);

        // Hot water-solid vessel: RWCU carries the compressed enthalpy, above
        // saturated h_f(T); the main steam lines receive nothing (no steam space).
        let mut v = vessel_at(200.0, 1.002 * V_VESSEL_FLUID * sat_t(200.0).rho_f);
        assert!(v.eos.solid);
        let m0 = v.mass;
        let h_c = steam::compressed_liquid_h(v.eos.t, v.eos.p);
        assert!(h_c - sat_t(v.eos.t).h_f > 500.0, "dh {}", h_c - sat_t(v.eos.t).h_f);
        let inp = VesselInputs { w_steam: 5.0, ..inp };
        v.step(dt, &inp);
        assert!(v.last_error.is_none(), "{:?}", v.last_error);
        assert_eq!(v.flows.h_liquid, h_c);
        assert!(v.flows.w_steam == 0.0 && v.flows.w_steam_gas == 0.0 && v.flows.w_liquid == 10.0);
        assert!((v.mass - m0 + 10.0 * dt).abs() < 1e-9);
    }

    /// Astra 19: accepted IC condensate return mixes into the downcomer with
    /// feedwater, so its temperature reaches the core inlet.
    #[test]
    fn ic_return_mixes_into_core_inlet() {
        let inlet = |t_ret: f64| {
            let mut v = Vessel::new(sat_p(P_RATED).t, Z_NWL, RATED_POWER_W, STEAM_FLOW_RATED, T_FW_RATED, false);
            let inp = VesselInputs { q_core: 0.0, q_ic: 0.0, w_feed: 0.0, h_feed: 0.0, w_steam: 0.0, w_liquid_out: 0.0 };
            v.step_with_ic(1.0, &inp, &IcPorts { w_steam: 0.0, w_return: 100.0, h_return: steam::h_liquid(t_ret) });
            assert!(v.last_error.is_none() && v.flows.w_ic_return == 100.0);
            v.h_in
        };
        let (cold, warm) = (inlet(40.0), inlet(150.0));
        assert!(warm - cold > 1.0e3, "inlet {cold} vs {warm}");
    }

    #[test]
    fn ic_ratio_is_one_without_a_steam_request() {
        let mut v = Vessel::new(sat_p(P_RATED).t, Z_NWL, RATED_POWER_W, STEAM_FLOW_RATED, T_FW_RATED, false);
        let inp = VesselInputs { q_core: 0.0, q_ic: 0.0, w_feed: 0.0, h_feed: 0.0, w_steam: 0.0, w_liquid_out: 0.0 };
        v.step_with_ic(0.05, &inp, &IcPorts { w_steam: 0.0, w_return: 50.0, h_return: 1.0e5 });
        assert_eq!(v.flows.ic_ratio, 1.0);
        assert_eq!((v.flows.w_ic_return, v.flows.h_ic_return), (50.0, 1.0e5));
    }

    /// Sealed vessel at `t` holding `mass` of water and `gas` kg of trapped air.
    fn sealed_at(t: f64, mass: f64, gas: f64) -> Vessel {
        let mut v = vessel_at(t, mass);
        v.head_vent_open = false;
        v.gas_mass = gas;
        v.eos = solve_eos_with(mass, v.energy, HeadSpace::Sealed { gas_mass: gas }).unwrap();
        v.update_level();
        v
    }

    /// R2 (Sol R3 fixture): sealed 90 degC, 180 t, 100 kg air. Total pressure
    /// 229956 Pa boils at ~124.68 degC (IF97), so 30 MW into 500 kg/s of 90 degC
    /// inlet water makes no void; the old atmospheric shortcut made ~22.7 %.
    #[test]
    fn sealed_cold_gas_bearing_core_boils_at_total_pressure() {
        let mut v = sealed_at(90.0, 180_000.0, 100.0);
        assert!(v.eos.subcooled && !v.eos.solid);
        assert!((v.eos.p - 229_956.46).abs() < 1.0e-3 * 229_956.46, "p {}", v.eos.p);
        assert!((sat_p(v.eos.p).t - 124.681).abs() < 0.3, "dome T_sat {}", sat_p(v.eos.p).t);
        // The core sits below ~12 m of water on top of that (R5).
        assert!(v.core_sat().t > sat_p(v.eos.p).t + 10.0, "core T_sat {}", v.core_sat().t);
        let h_in = steam::h_liquid(90.0);
        assert!(core_th(&sat_p(P_ATM), 500.0, 30.0e6, h_in).alpha_avg > 0.2, "fixture must discriminate");
        v.core_flow = 500.0;
        v.h_in = h_in;
        let inp = VesselInputs { q_core: 30.0e6, q_ic: 0.0, w_feed: 0.0, h_feed: 0.0, w_steam: 0.0, w_liquid_out: 0.0 };
        v.step(0.05, &inp);
        assert!(v.last_error.is_none(), "{:?}", v.last_error);
        assert_eq!((v.core.alpha_avg, v.core.x_exit), (0.0, 0.0), "{:?}", v.core);
    }

    /// Round-5 R3/R5: core boiling uses dome pressure plus the static column
    /// above the core mid-plane. Void-free vessels at 1 atm (vented, 99.5 degC)
    /// and at their own vapour pressure (100.5 degC) are checked against an
    /// independent liquid-column sum; both boil near 120 degC, not 100 degC.
    #[test]
    fn core_boils_at_dome_pressure_plus_liquid_head() {
        let mut vented = vessel_at(99.5, 180_000.0);
        vented.head_vent_open = true;
        vented.eos = solve_eos(vented.mass, vented.energy).unwrap();
        assert_eq!(vented.eos.p, P_ATM);
        let above = vessel_at(100.5, 180_000.0);
        for v in [&vented, &above] {
            assert_eq!((v.alpha_core, v.alpha_ch), (0.0, 0.0));
            let z_mid = Z_CORE_BOTTOM + 0.5 * CORE_HEIGHT;
            let p_mid = v.eos.p + sat_t(v.eos.t).rho_f * G * (v.level_z - z_mid);
            assert!((v.core_pressure() - p_mid).abs() < 1e-6 * p_mid, "{} vs {p_mid}", v.core_pressure());
            assert!(v.core_sat().t > 115.0, "core T_sat {}", v.core_sat().t);
        }
    }

    /// Astra R5 / Sol R3: sealed, gas-free 90 degC, 180 t (dome 70.18 kPa). The
    /// core mid-plane sees ~1.9 bar, so 8.7 MW into 500 kg/s of 90 degC water
    /// (exit ~394 kJ/kg) stays subcooled: no void, no exit quality, in the core
    /// model and in accepted vessel steps.
    #[test]
    fn gas_free_cold_core_is_subcooled_under_its_water_head() {
        let mut v = sealed_at(90.0, 180_000.0, 0.0);
        assert!((v.eos.p - 70_182.36).abs() < 1.0e-3 * 70_182.36, "p {}", v.eos.p);
        let h_in = steam::h_liquid(90.0);
        assert!(core_th(&sat_p(v.eos.p), 500.0, 8.7e6, h_in).alpha_avg > 0.3, "fixture must discriminate");
        assert!(v.core_sat().t > 116.0, "core T_sat {}", v.core_sat().t);
        assert_eq!(core_th(&v.core_sat(), 500.0, 8.7e6, h_in).alpha_avg, 0.0);
        v.core_flow = 500.0;
        v.h_in = h_in;
        let inp = VesselInputs { q_core: 8.7e6, q_ic: 0.0, w_feed: 0.0, h_feed: 0.0, w_steam: 0.0, w_liquid_out: 0.0 };
        for _ in 0..20 {
            v.step(0.05, &inp);
            assert!(v.last_error.is_none(), "{:?}", v.last_error);
            assert_eq!((v.core.alpha_avg, v.core.x_exit), (0.0, 0.0), "{:?}", v.core);
        }
        assert!(v.alpha_core.abs() < 1e-12 && v.alpha_ch.abs() < 1e-12);
    }

    /// Cold compressed liquid (water-solid 40 degC, several MPa) is far from
    /// boiling, and an open main line takes nothing from it: the lines carry
    /// dome gas only, so mass and energy change by the heat loss alone.
    #[test]
    fn cold_compressed_liquid_does_not_boil_and_supplies_no_main_line_flow() {
        let m = V_VESSEL_FLUID * sat_t(40.0).rho_f * 1.001;
        let mut v = sealed_at(40.0, m, 0.0);
        assert!(v.eos.solid && v.eos.subcooled && v.eos.p > 1.0e6, "{:?}", v.eos);
        assert!(v.core_sat().t > 150.0, "core T_sat {}", v.core_sat().t);
        assert_eq!(core_th(&v.core_sat(), 500.0, 30.0e6, steam::h_liquid(40.0)).alpha_avg, 0.0);
        let (m0, e0, dt) = (v.mass, v.energy, 0.05);
        let inp = VesselInputs { q_core: 0.0, q_ic: 0.0, w_feed: 0.0, h_feed: 0.0, w_steam: 500.0, w_liquid_out: 0.0 };
        v.step(dt, &inp);
        assert!(v.last_error.is_none(), "{:?}", v.last_error);
        assert_eq!((v.flows.w_steam, v.flows.w_steam_gas), (0.0, 0.0));
        assert!(v.eos.solid);
        let loss = HEAT_LOSS_HOT * ((40.0 - 20.0) / 268.0) * dt;
        assert_eq!(v.mass, m0);
        assert!((v.energy - e0 + loss).abs() < 1e-12 * e0, "energy residual {}", v.energy - e0 + loss);
    }

    /// R2: sealed, gas-bearing pressure/temperature sweep across 100 degC.
    /// Total pressure, core boiling temperature, core void and the main-line
    /// request all move continuously (no 100 degC switch).
    #[test]
    fn sealed_sweep_across_100c_is_continuous() {
        let (mass, gas, q, w_core) = (180_000.0, 100.0, 8.7e6, 500.0);
        let mut prev: Option<[f64; 4]> = None;
        let mut t = 98.0;
        while t <= 102.0 + 1e-9 {
            let v = sealed_at(t, mass, gas);
            let s = v.core_sat();
            let void = core_th(&s, w_core, q, steam::h_liquid(t).min(s.h_f)).alpha_avg;
            let mut line = crate::bop::SteamSystem::new(true);
            line.tcv_pos = 0.1;
            line.compute_flows_from(v.eos.p, v.steam_port_enthalpy(), v.steam_line_density(), crate::bop::LineGas::STEAM, 5.6e3, true);
            let now = [v.eos.p, s.t, void, line.total_flow()];
            assert!(now[3] > 0.0, "pressurized line must flow at {t} degC");
            if let Some(p) = prev {
                assert!((now[0] - p[0]).abs() < 2.0e-3 * p[0], "{t}: p {} -> {}", p[0], now[0]);
                assert!((now[1] - p[1]).abs() < 0.1, "{t}: T_sat {} -> {}", p[1], now[1]);
                assert!((now[2] - p[2]).abs() < 1e-3, "{t}: void {} -> {}", p[2], now[2]);
                assert!((now[3] - p[3]).abs() < 0.02 * p[3], "{t}: line {} -> {}", p[3], now[3]);
            }
            prev = Some(now);
            t += 0.02;
        }
    }
}
