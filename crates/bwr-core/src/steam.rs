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
}
