//! Saturated water/steam properties from an embedded IAPWS-IF97 table.

use crate::steam_table::SAT_TABLE;

/// Standard atmospheric pressure, Pa.
pub const P_ATM: f64 = 101_325.0;

const T0: f64 = 1.0; // first table temperature, degC
const N: usize = SAT_TABLE.len();

/// Saturation state at one temperature/pressure.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sat {
    /// Saturation temperature, degC.
    pub t: f64,
    /// Saturation pressure, Pa.
    pub p: f64,
    pub rho_f: f64,
    pub rho_g: f64,
    /// Saturated liquid / vapour specific enthalpy, J/kg.
    pub h_f: f64,
    pub h_g: f64,
}

impl Sat {
    pub fn h_fg(&self) -> f64 {
        self.h_g - self.h_f
    }
    pub fn u_f(&self) -> f64 {
        self.h_f - self.p / self.rho_f
    }
    pub fn u_g(&self) -> f64 {
        self.h_g - self.p / self.rho_g
    }
    pub fn v_f(&self) -> f64 {
        1.0 / self.rho_f
    }
    pub fn v_g(&self) -> f64 {
        1.0 / self.rho_g
    }
    /// Saturated-liquid specific entropy, J/kgK, from IF97 Region 1 at saturation.
    pub fn s_f(&self) -> f64 {
        region1(self.t, self.p).s
    }
    /// Saturated-vapour specific entropy, J/kgK: s_f + h_fg/T, so the latent
    /// entropy is exactly consistent with the table's latent heat.
    pub fn s_g(&self) -> f64 {
        self.s_f() + self.h_fg() / (self.t + 273.15)
    }
    /// Surface tension, N/m (IAPWS 2014 correlation).
    pub fn sigma(&self) -> f64 {
        let tau = 1.0 - (self.t + 273.15) / 647.096;
        if tau <= 0.0 {
            return 0.0;
        }
        0.2358 * tau.powf(1.256) * (1.0 - 0.625 * tau)
    }
}

fn row(i: usize) -> Sat {
    let r = SAT_TABLE[i];
    Sat { t: r[0], p: r[1], rho_f: r[2], rho_g: r[3], h_f: r[4], h_g: r[5] }
}

fn lerp(a: Sat, b: Sat, f: f64) -> Sat {
    let l = |x: f64, y: f64| x + (y - x) * f;
    Sat {
        t: l(a.t, b.t),
        p: (a.p.ln() + (b.p.ln() - a.p.ln()) * f).exp(),
        rho_f: l(a.rho_f, b.rho_f),
        rho_g: (a.rho_g.ln() + (b.rho_g.ln() - a.rho_g.ln()) * f).exp(),
        h_f: l(a.h_f, b.h_f),
        h_g: l(a.h_g, b.h_g),
    }
}

/// Saturation properties at temperature `t` (degC), clamped to 1..350 degC.
pub fn sat_t(t: f64) -> Sat {
    let x = (t - T0).clamp(0.0, (N - 1) as f64);
    let i = (x.floor() as usize).min(N - 2);
    lerp(row(i), row(i + 1), x - i as f64)
}

/// Binary search on a monotonically increasing table column; returns (index, fraction).
fn locate(col: usize, v: f64, log: bool) -> (usize, f64) {
    let (mut lo, mut hi) = (0usize, N - 1);
    let v = v.clamp(SAT_TABLE[0][col], SAT_TABLE[N - 1][col]);
    while hi - lo > 1 {
        let mid = (lo + hi) / 2;
        if SAT_TABLE[mid][col] <= v {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let (a, b) = (SAT_TABLE[lo][col], SAT_TABLE[hi][col]);
    let f = if log { (v.ln() - a.ln()) / (b.ln() - a.ln()) } else { (v - a) / (b - a) };
    (lo, f.clamp(0.0, 1.0))
}

/// Saturation properties at pressure `p` (Pa).
pub fn sat_p(p: f64) -> Sat {
    let (i, f) = locate(1, p, true);
    sat_t(T0 + i as f64 + f)
}

/// Temperature (degC) of liquid water with specific enthalpy `h` (J/kg),
/// using the saturated-liquid curve (good for compressed liquid at BWR pressures).
pub fn t_from_h_liquid(h: f64) -> f64 {
    let (i, f) = locate(4, h, false);
    T0 + i as f64 + f
}

/// Saturated-liquid enthalpy at temperature `t` (degC).
pub fn h_liquid(t: f64) -> f64 {
    sat_t(t).h_f
}

/// Liquid density at temperature `t` (degC).
pub fn rho_liquid(t: f64) -> f64 {
    sat_t(t).rho_f
}

/// IAPWS-IF97 Region 1 coefficients (I, J, n), Table 2 of IF97-Rev.
const R1_IJN: [(i32, i32, f64); 34] = [
    (0, -2, 0.14632971213167), (0, -1, -0.84548187169114), (0, 0, -0.37563603672040e1),
    (0, 1, 0.33855169168385e1), (0, 2, -0.95791963387872), (0, 3, 0.15772038513228),
    (0, 4, -0.16616417199501e-1), (0, 5, 0.81214629983568e-3), (1, -9, 0.28319080123804e-3),
    (1, -7, -0.60706301565874e-3), (1, -1, -0.18990068218419e-1), (1, 0, -0.32529748770505e-1),
    (1, 1, -0.21841717175414e-1), (1, 3, -0.52838357969930e-4), (2, -3, -0.47184321073267e-3),
    (2, 0, -0.30001780793026e-3), (2, 1, 0.47661393906987e-4), (2, 3, -0.44141845330846e-5),
    (2, 17, -0.72694996297594e-15), (3, -4, -0.31679644845054e-4), (3, 0, -0.28270797985312e-5),
    (3, 6, -0.85205128120103e-9), (4, -5, -0.22425281908000e-5), (4, -2, -0.65171222895601e-6),
    (4, 10, -0.14341729937924e-12), (5, -8, -0.40516996860117e-6), (8, -11, -0.12734301741641e-8),
    (8, -6, -0.17424871230634e-9), (21, -29, -0.68762131295531e-18), (23, -31, 0.14478307828521e-19),
    (29, -38, 0.26335781662795e-22), (30, -39, -0.11947622640071e-22), (31, -40, 0.18228094581404e-23),
    (32, -41, -0.93537087292458e-25),
];
/// Specific gas constant of water in IF97, J/kgK.
pub const R_WATER: f64 = 461.526;

/// Compressed-liquid state from IF97 Region 1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Liquid {
    /// Specific volume, m3/kg.
    pub v: f64,
    /// Specific enthalpy and internal energy, J/kg.
    pub h: f64,
    pub u: f64,
    /// Isothermal compressibility, 1/Pa.
    pub kappa: f64,
    /// Cubic expansion coefficient, 1/K.
    pub beta: f64,
    /// Specific entropy, J/kgK.
    pub s: f64,
}

/// IAPWS-IF97 Region 1 at temperature `t` (degC) and pressure `p` (Pa).
/// Valid for 0..350 degC and p_sat(t)..100 MPa.
pub fn region1(t: f64, p: f64) -> Liquid {
    let tk = t + 273.15;
    let pi = p / 16.53e6;
    let tau = 1386.0 / tk;
    let (a, b) = (7.1 - pi, tau - 1.222);
    let (mut g, mut g_p, mut g_pp, mut g_t, mut g_pt) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for &(i, j, n) in R1_IJN.iter() {
        let (fi, fj) = (i as f64, j as f64);
        g += n * a.powi(i) * b.powi(j);
        g_p -= n * fi * a.powi(i - 1) * b.powi(j);
        g_pp += n * fi * (fi - 1.0) * a.powi(i - 2) * b.powi(j);
        g_t += n * a.powi(i) * fj * b.powi(j - 1);
        g_pt -= n * fi * a.powi(i - 1) * fj * b.powi(j - 1);
    }
    let v = pi * g_p * R_WATER * tk / p;
    let h = tau * g_t * R_WATER * tk;
    let s = (tau * g_t - g) * R_WATER;
    Liquid { v, h, u: h - p * v, kappa: -pi * g_pp / (g_p * p), beta: (1.0 - tau * g_pt / g_p) / tk, s }
}

/// Compressed liquid at temperature `t` squeezed to specific volume `v`
/// (below the saturated-liquid volume). The Region 1 increments from the
/// saturation state are added to the table's saturated liquid, so the state
/// is continuous with `sat_t` at the phase boundary. Returns (p, u) or, when
/// the required pressure exceeds `p_max`, `Err(p)` with the pressure reached.
pub fn compressed_liquid(t: f64, v: f64, p_max: f64) -> Result<(f64, f64), f64> {
    let s = sat_t(t);
    let at_sat = region1(t, s.p);
    // Volume ratio to the saturated state; v(p)/v_sat is convex and decreasing
    // in p, so Newton from p_sat approaches the root monotonically from below.
    let target = v / s.v_f();
    let mut p = s.p;
    let mut liq = at_sat;
    for _ in 0..50 {
        let f = liq.v / at_sat.v - target;
        if f.abs() < 1e-14 {
            break;
        }
        p += f / (liq.kappa * liq.v / at_sat.v);
        if !(p <= p_max) {
            return Err(p);
        }
        liq = region1(t, p);
    }
    Ok((p, anchored(&s, &at_sat, &liq).1))
}

/// (v, u) of compressed liquid: Region 1 increments from the saturated state
/// added to the table's saturated liquid (the anchoring `compressed_liquid` uses).
fn anchored(s: &Sat, at_sat: &Liquid, liq: &Liquid) -> (f64, f64) {
    (s.v_f() * liq.v / at_sat.v, s.u_f() + (liq.u - at_sat.u))
}

/// Specific enthalpy (J/kg) of liquid at temperature `t` (degC) and pressure
/// `p` (Pa), with the same saturation anchoring as `compressed_liquid`, so a
/// port drawing liquid from a compressed vessel carries u + p*v of the very
/// state the EOS stores. Saturated liquid at or below p_sat(t).
pub fn compressed_liquid_h(t: f64, p: f64) -> f64 {
    let s = sat_t(t);
    if !(p > s.p) {
        return s.h_f;
    }
    let (v, u) = anchored(&s, &region1(t, s.p), &region1(t, p));
    u + p * v
}

/// Temperature (degC) of liquid at pressure `p` (Pa) with specific enthalpy
/// `h` (J/kg), inverting `compressed_liquid_h` by bisection (monotonic in t).
fn liquid_t_from_hp(h: f64, p: f64) -> f64 {
    let (mut lo, mut hi) = (T0, sat_p(p).t);
    for _ in 0..60 {
        let mid = 0.5 * (lo + hi);
        if compressed_liquid_h(mid, p) < h {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// Specific entropy (J/kgK) of water at enthalpy `h` (J/kg) and pressure `p`
/// (Pa): Region 1 for liquid at or below saturated-liquid enthalpy, the lever
/// rule (s_f + (h - h_f)/T_sat) inside the dome. Above h_g the state is taken
/// as slightly superheated with ds = dh/T_sat, which overstates s (T > T_sat
/// there) and so understates any isentropic drop computed from it.
pub fn entropy_hp(h: f64, p: f64) -> f64 {
    let s = sat_p(p);
    let tk = s.t + 273.15;
    if h <= s.h_f {
        let t = liquid_t_from_hp(h, p);
        region1(t, p.max(sat_t(t).p)).s
    } else if h <= s.h_g {
        s.s_f() + (h - s.h_f) / tk
    } else {
        s.s_g() + (h - s.h_g) / tk
    }
}

/// Isentropic enthalpy drop (J/kg, >= 0) expanding water at (`h_in`, `p_in`)
/// to `p_out`: the reversible work limit of any expansion between those states.
/// The exit state at `p_out` is wet (lever rule on entropy), compressed liquid
/// (drop = v_f dp, liquid is nearly incompressible) or slightly superheated.
pub fn isentropic_drop(h_in: f64, p_in: f64, p_out: f64) -> f64 {
    if !(p_in > p_out && p_out > 0.0) {
        return 0.0;
    }
    let s_in = entropy_hp(h_in, p_in);
    let o = sat_p(p_out);
    let (s_f, s_g) = (o.s_f(), o.s_g());
    let h_out = if s_in <= s_f {
        return o.v_f() * (p_in - p_out);
    } else if s_in <= s_g {
        o.h_f + (s_in - s_f) / (s_g - s_f) * o.h_fg()
    } else {
        o.h_g + (s_in - s_g) * (o.t + 273.15)
    };
    (h_in - h_out).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_points() {
        let s = sat_p(7.2e6);
        assert!((s.t - 287.7).abs() < 0.3, "t_sat(7.2 MPa) = {}", s.t);
        assert!((s.h_fg() - 1.497e6).abs() < 0.01e6);
        let s = sat_t(100.0);
        assert!((s.p - 101_418.0).abs() < 200.0);
        assert!((t_from_h_liquid(h_liquid(215.0)) - 215.0).abs() < 1e-6);
    }

    /// IF97-Rev Table 5 verification values for Region 1.
    #[test]
    fn region1_matches_if97_verification_table() {
        let close = |a: f64, b: f64| ((a - b) / b).abs() < 1e-8;
        let r = region1(300.0 - 273.15, 3.0e6);
        assert!(close(r.v, 0.100215168e-2) && close(r.h, 0.115331273e6) && close(r.u, 0.112324818e6), "{r:?}");
        let r = region1(300.0 - 273.15, 80.0e6);
        assert!(close(r.v, 0.971180894e-3) && close(r.h, 0.184142828e6) && close(r.u, 0.106448356e6), "{r:?}");
        let r = region1(500.0 - 273.15, 3.0e6);
        assert!(close(r.v, 0.120241800e-2) && close(r.h, 0.975542239e6) && close(r.u, 0.971934985e6), "{r:?}");
        assert!(close(r.beta, 0.164118128e-2) && close(r.kappa, 0.112892188e-8), "{r:?}");
    }

    #[test]
    fn compressed_liquid_is_continuous_with_saturation_and_bounded() {
        let s = sat_t(200.0);
        let (p, u) = compressed_liquid(200.0, s.v_f(), 100.0e6).unwrap();
        assert!((p - s.p).abs() < 1.0 && (u - s.u_f()).abs() < 1e-6);
        let (p, _) = compressed_liquid(200.0, s.v_f() * 0.99, 100.0e6).unwrap();
        assert!(p > 12.0e6 && p < 14.0e6, "p {p}");
        assert!(compressed_liquid(200.0, s.v_f() * 0.9, 100.0e6).is_err());
    }

    /// R3: compressed-liquid port enthalpy against independent IAPWS-IF97
    /// Region 1 h(T, P) (Python iapws 1.5.5, quoted in the round-2 reviews).
    #[test]
    fn compressed_liquid_enthalpy_matches_if97_at_pressure() {
        for &(t, p, h_ref) in &[(40.0, 4.562267e6, 171_573.418), (200.0, 50.000049e6, 875_311.210)] {
            let h = compressed_liquid_h(t, p);
            assert!((h - h_ref).abs() < 50.0, "h({t} C, {p} Pa) = {h} vs IF97 {h_ref}");
            assert!(h - h_liquid(t) > 900.0, "pressure term missing at {t} C");
        }
        // Continuous with saturated liquid at the phase boundary.
        let s = sat_t(200.0);
        assert_eq!(compressed_liquid_h(200.0, s.p), s.h_f);
        assert!((compressed_liquid_h(200.0, s.p * (1.0 + 1e-9)) - s.h_f).abs() < 1e-3);
    }

    /// Entropy and isentropic drops against IAPWS-IF97 (Python iapws 1.5,
    /// `IAPWS97(P=.., s=..)`): saturation entropies, a textbook Rankine
    /// expansion and the two compressed-liquid main-line fixtures of the
    /// 2026-10-22 reviews.
    #[test]
    fn entropy_and_isentropic_drop_match_iapws() {
        let s = sat_t(100.0);
        assert!((s.s_f() - 1307.014).abs() < 2.0, "s_f {}", s.s_f());
        assert!((s.s_g() - 7354.077).abs() < 5.0, "s_g {}", s.s_g());
        let g = sat_p(7.0e6);
        assert!((entropy_hp(g.h_g, 7.0e6) - 5814.633).abs() < 5.0);
        let cases = [
            (g.h_g, 7.0e6, 10.0e3, 933_211.0),
            (sat_p(1.0e6).h_g, 1.0e6, 5.6e3, 757_546.0),
            (853_319.599, 3.834878e6, 5.609667e3, 145_330.790),
            (464_246.619, 4.101863e6, 5.6e3, 37_216.458),
        ];
        for (h, p_in, p_out, ref_dh) in cases {
            let dh = isentropic_drop(h, p_in, p_out);
            assert!((dh - ref_dh).abs() < 1e-4 * ref_dh + 20.0, "({h}, {p_in}->{p_out}) {dh} vs {ref_dh}");
        }
        assert_eq!(isentropic_drop(g.h_g, 5.0e3, 7.0e6), 0.0);
    }
}
