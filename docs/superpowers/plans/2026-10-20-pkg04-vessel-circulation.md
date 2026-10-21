# Package 4: Vessel Properties and Natural Circulation — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Names and signatures come from `docs/superpowers/plans/2026-10-20-refinement-master.md` (section "P4: vessel regimes and properties"); if this file and the master disagree, the master wins.

**Goal:** Make vessel pressure, level, and natural-circulation flow respond consistently to mass and energy changes across the whole operating envelope (cold vented, gas-blanketed, saturated, water-solid, low level), using IAPWS-IF97 properties including liquid compressibility, instead of the baseline rule "below 100 degC is atmospheric, above is saturated".

**Architecture:** `vessel.rs` keeps one lumped water mass/energy inventory plus a new noncondensable gas mass; `solve_eos` returns a `Regime` and total/partial pressures. The steam table gains an isothermal-compressibility column so water-solid pressure and compressed-liquid density are physical. The natural-circulation driving head becomes an explicit level-dependent hydrostatic balance instead of a 0.5 m smoothing switch. Moderator temperature feedback uses the core's actual liquid temperature profile. Every change lands behind a failing test.

**Tech Stack:** Rust 2021, serde, Python 3 + CoolProp (`tools/gen_steam_table.py`), existing `validation` module from Package 2.

**Prerequisites (assumed complete, master r2):** Package 1 (`docs/model/*.csv` with the locked headers, `assumptions.md`, `tests/register.rs` enforcing a `parameters.csv` row per constant), Package 2 (`ledger.rs` with `BoundaryFlows` incl. `w_ic_steam`/`w_ic_return`/`h_ic_return` and `Ledger.integrated`; `validation.rs` with trace schema v2 (`N_TRACE_COLS = 59`, columns `gas_mass_kg`, `t_coolant_c`, `moderator_t_c` emitted as 0.0 until this package); `standard_cases()` with ids `fp_steady_600`, `hs_steady_600`, `cs_steady_600`; `Sim::finalize_init()` as the constructor epilogue that re-bases the ledger), Package 3 (`domain.rs::DomainError`, `Vessel.collapsed_level_z`, `Vessel.exhausted: Option<f64>`, `Vessel.last_error: Option<DomainError>`, `pub const T_EOS_MIN/T_EOS_MAX` in `vessel.rs`, `solve_eos` returning `Result` with the master rejection semantics, outflow bounding in `Vessel::step` as the transfer-allocation point, `core_coverage` fix, `Sim::step` order steps 1–17).

**Derives rule (master r2):** `Regime` derives `Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize` with `#[serde(rename_all = "camelCase")]`; the replacement `Eos` keeps the baseline derives `Clone, Copy, Debug`. Both appear verbatim in Task 3 Step 3.

**Spec section:** Package 4 of `docs/superpowers/plans/2026-10-20-existing-systems-refinement.md`.

## Global Constraints

See master plan "Global Constraints". In addition for this package:

- "Do not calibrate all behavior to one assumed full-power flow." `k_fric` stays calibrated at one rated point, but flow must be tested at other power levels and levels.
- "Density-wave or regional stability claims remain limited to evidence actually demonstrated." This package makes no stability claim; `docs/model/vessel.md` says so.
- No coefficient retuning (`VOID_COEF`, `MOD_TEMP_COEF`, `DOPPLER_A`, `CORE_FLOW_RATED`, `LOOP_INERTIA`) in this package. If a test can only pass by retuning, stop and record the finding.
- Baseline traces in `docs/validation/baseline/` are regenerated only in Task 7, after the before/after metric differences are pasted into the commit body.

---

## File structure

| File | Action | Responsibility |
|---|---|---|
| `docs/model/vessel.md` | Create | Vessel envelope, regime definitions, equations, limitations, test map |
| `tools/gen_steam_table.py` | Modify | Emit 7-column table with `kappa_f` |
| `crates/bwr-core/src/steam_table.rs` | Regenerate | `SAT_TABLE: [[f64; 7]; 350]` |
| `crates/bwr-core/src/steam.rs` | Modify | `Sat.kappa_f`, `compressed_liquid_rho`, `T_TABLE_MIN/MAX`, reference tests |
| `crates/bwr-core/src/vessel.rs` | Modify | `Regime`, `Eos`, `solve_eos` with gas and water-solid, `gas_mass`, vent/gas dynamics, hydrostatic head, `moderator_t`, `steam_available`, `friction_dp` |
| `crates/bwr-core/src/sim.rs` | Modify | Replace `eos.subcooled`/`eos.solid` call sites, pass `moderator_t()`, snapshot `regime`, gas-blanket alarm |
| `crates/bwr-core/src/api.rs` | Modify | `Vessel.regime` |
| `crates/bwr-core/src/validation.rs` | Modify | Populate trace schema v2 columns `gas_mass_kg`, `t_coolant_c` (Task 3) and `moderator_t_c` (Task 6) in `run_case` sampling |
| `ui/src/sim/types.ts`, `ui/src/sim/mock.ts` | Modify | `Regime` type and `vessel.regime` field |
| `crates/bwr-core/tests/vessel_transients.rs` | Create | Shrink/swell, heatup, cooldown, pressure step, low level, steady consistency, water-solid, conservation |
| `docs/model/parameters.csv`, `assumptions.md`, `esbwr-applicability.csv`, `sources.csv`, `coverage.md` | Modify | Register rows for every constant and borrowed concept |
| `docs/validation/baseline/*.csv`, `docs/validation/method.md` | Modify | Regenerated baselines with change log |

Call sites of the removed `Eos.subcooled` / `Eos.solid` booleans (all must be updated in Task 3):

| File | Location | Replacement |
|---|---|---|
| `vessel.rs` | `Eos` struct, `solve_eos` | new struct / new function |
| `vessel.rs` | `Vessel::new` initial `eos: Eos { ..subcooled: true, solid: false }` | `Eos { t, p: P_ATM, p_steam: 0.0, p_gas: 0.0, x: 0.0, regime: Regime::Vented }` |
| `vessel.rs` | `update_level` `if self.eos.subcooled { rho_liquid } else { sat_t.rho_f }` | `compressed_liquid_rho(self.eos.t, self.eos.p)` |
| `vessel.rs` | `step` vent `!self.eos.subcooled`; `h_liq`; `w_steam = if subcooled {0}` | steam-space composition split and cap, no regime match (Task 4) |
| `vessel.rs` | `sat_for_core` | `sat_p(self.eos.p)` unconditionally |
| `sim.rs` | `step`: `compute_flows(if self.vessel.eos.subcooled { P_ATM } else { p }, ...)` | `compute_flows(p, ...)` |
| `sim.rs` | `step`: `let steam_avail = !self.vessel.eos.subcooled;` | `let steam_avail = self.vessel.steam_available();` |
| `sim.rs` | `protection`: `isolated_at_power = ... && !self.vessel.eos.subcooled && ...` | `&& self.vessel.steam_available() &&` |
| `sim.rs` | `active_alarms`: `if self.vessel.eos.solid` | `if self.vessel.eos.regime == Regime::WaterSolid` |
| `sim.rs` | `active_alarms`: `if !self.msivs_open() && !self.vessel.eos.subcooled` | `&& self.vessel.steam_available()` |
| `sim.rs` | `active_alarms`: `if !self.cond.available_for_bypass() && !self.vessel.eos.subcooled && p > 1.0e6` | `&& self.vessel.steam_available() && p > 1.0e6` |
| `bop.rs` | none | (takes pressures only) |
| Package 3 tests (`tests/numerics.rs`, `tests/conservation.rs`) | any `solve_eos(m, e)` | `solve_eos(m, e, 0.0, false)` |

---

### Task 1: Vessel envelope specification

**Files:**
- Create: `docs/model/vessel.md`
- Modify: `docs/model/coverage.md` (vessel row points to this file)

**Interfaces:**
- Consumes: Package 1 register files.
- Produces: the envelope that Tasks 2–7 implement and test. Numbers marked "(measured in Task 8)" are filled in Task 8.

- [ ] **Step 1: Write `docs/model/vessel.md`**

```markdown
# Reactor vessel thermal-hydraulic model

Status: Package 4 of the refinement program. Model owner file: `crates/bwr-core/src/vessel.rs`.

## 1. Supported envelope

| Quantity | Supported range | Outside the range |
|---|---|---|
| Water temperature `t` | 1 .. 349.5 degC (saturation table 1..350 degC, `steam::T_TABLE_MIN/MAX`) | `DomainError::EosOutOfRange` |
| Total pressure `p` | 0.1 MPa (vented) .. 8.62 MPa design (`VESSEL_DESIGN_PRESSURE`); alarmed above design; water-solid pressure up to 2 x design | `DomainError::PropertyOutOfRange { what: "water-solid pressure" }` above 2 x design |
| Noncondensable partial pressure | 0 .. 2 x design | `DomainError::PropertyOutOfRange { what: "gas partial pressure" }` |
| Water inventory | from exhaustion (`DomainError::InventoryExhausted`, Package 3) to water-solid | — |
| Static quality `x` | 0 .. 1 | clamped by volume (cannot leave) |
| Water level | 0 .. `VESSEL_HEIGHT` (26.7 m); narrow-range display is relative to `Z_NWL` | — |

The simulation keeps running after a `DomainError`; for the offending step the vessel
rolls `mass`, `energy` and `gas_mass` back to their entry values and reports
`BoundaryFlows::default()` (nothing applied), `Vessel.last_error` is set, and the Sim
raises `MODEL ENVELOPE EXCEEDED`. When outflows must be bounded to the available
inventory, `Vessel.exhausted` carries the deficit and the bounded flows are applied.

## 2. State variables

- `mass` kg of water (liquid + steam), `energy` J of water internal energy plus vessel
  metal sensible heat (`METAL_HEAT_CAP * t`), `gas_mass` kg of noncondensable gas (air,
  `R_AIR = 287 J/kg/K`), `head_vent_open`.
- Derived each step by `solve_eos(mass, energy, gas_mass, head_vent_open)`: temperature `t`,
  steam partial pressure `p_steam = p_sat(t)`, gas partial pressure `p_gas`, total `p`,
  quality `x`, and `regime`.

## 3. Regimes

| Regime | Condition | Pressure |
|---|---|---|
| `Vented` | head vent open | `p = p_sat(t) + p_gas`; vent flow drives `p -> P_ATM` (air ingress below, steam+gas egress above) |
| `Saturated` | vent closed, steam space present, `p_gas <= p_sat(t)` | `p = p_sat(t) + p_gas` (p_gas is 0 after purging) |
| `GasBlanketed` | vent closed, steam space present, `p_gas > p_sat(t)` | `p = p_sat(t) + p_gas`, with `p_gas = gas_mass * R_AIR * T_K / V_gas` |
| `WaterSolid` | `mass * v_f(t) >= V_VESSEL_FLUID` | `p = p_sat(t) + (mass * v_f(t) - V) / (V * kappa_f(t))` (liquid compressibility) |

Regime is a label for display, alarms and tests only; no physical quantity or outflow
depends on it (outflows are split by the steam-space composition, section 4, and
`steam_available` depends on `p_steam` and the water-solid condition alone). `solve_eos`
is a pure function; `Vessel::step` applies a hysteresis band of `REGIME_HYSTERESIS`
(1.05) to the Saturated/GasBlanketed label so it cannot chatter when `p_gas ~ p_sat`.
Pressure is continuous across every boundary: Vented/Saturated differ only by the vent
flag; Saturated/GasBlanketed share `p = p_sat + p_gas`; at the water-solid boundary the
compressibility term is zero. Only the pressure derivative is discontinuous at the
water-solid boundary (tested by `regime_boundaries_are_continuous_and_cross_once`).

A gas-blanketed vessel cannot be driven water-solid in the model: as the gas space
vanishes `p_gas` rises without bound and `PropertyOutOfRange { what: "gas space volume"
| "gas partial pressure" }` is raised before the water-solid branch is reached. Gas
dissolution is not modelled (A-P4-1); only gas below `WATER_SOLID_GAS_TOLERANCE_KG`
(1 kg) is treated as dissolved. A vessel below 100 degC with the vent closed and no gas
sits at `p_sat(t)` below atmospheric (a sealed vessel draws vacuum); air in-leakage is
not modelled (A-P4-8).

## 4. Equations

- Energy: `E = mass * (u_f + x (u_g - u_f)) + METAL_HEAT_CAP * t`; the noncondensable gas
  internal energy is neglected (A-P4-3).
- Quality from volume: `x = (V/mass - v_f) / (v_g - v_f)` clamped to 0..1.
- Liquid density: `rho = rho_f(t) * (1 + kappa_f(t) * (p - p_sat(t)))`
  (`steam::compressed_liquid_rho`), used for level, downcomer density and the
  downcomer transport lag.
- Boiling in the core is evaluated at the total vessel pressure `p` (`sat_for_core`),
  so a cold, air-filled vessel at 0.1 MPa does not boil until 100 degC. The downcomer
  hydrostatic head on the core inlet pressure is neglected (A-P4-5).
- Outflows from the steam space (steam lines and head vent) leave as steam and gas in
  proportion to their masses in the steam space, `y_steam = m_steam / (m_steam + m_gas)`
  with `m_steam = mass * x`; steam leaves at `h_g(p_steam)` and gas carries no energy
  (A-P4-3). No more steam can leave in a step than the steam space holds
  (`mass * x / dt`), so steam outflow goes to zero continuously as the vessel approaches
  water-solid; the shortfall is an unmet request that the Sim allocation step scales
  downstream (master step 9). Gas swept out through the steam lines is removed from
  `gas_mass` (this is how trapped air is purged during heatup, A-P4-4). No liquid ever
  leaves through the steam lines or the vent.
- Natural circulation head (section 6).

## 5. Head vent

`w_vent = C_HEAD_VENT * (p - P_ATM) + VENT_BOILOFF_KGS_PER_K * max(t - 100, 0)` kg/s
of steam-space fluid (steam + gas by composition) out when `p > P_ATM`; air ingress
`C_HEAD_VENT * (P_ATM - p)` kg/s of gas (no energy) when `p < P_ATM`. Both coefficients
are ASSUMED (parameters.csv). A water-solid vessel has no steam space and the vent
passes nothing: water-solid pressure is bounded only by the envelope check (2 x design
pressure, `PropertyOutOfRange`). This is a declared limitation (A-P4-2), not a plant
claim. Vented gas is outside the water ledger; vented steam is recorded in
`BoundaryFlows.w_vent` at `h_steam`.

## 6. Natural circulation

Momentum: `LOOP_INERTIA * dW/dt = drive - k_fric * phi2 * W^2`, with

```
h_dc   = max(min(level_z, Z_CHIMNEY_TOP) - Z_CORE_BOTTOM, 0)
h_core = clamp(level_z - Z_CORE_BOTTOM, 0, CORE_HEIGHT)
h_ch   = clamp(min(level_z, Z_CHIMNEY_TOP) - Z_TAF, 0, CHIMNEY_HEIGHT)
drive  = g * (rho_dc * h_dc - rho_core * h_core - rho_ch * h_ch) * loop_closed
loop_closed = clamp((level_z - Z_TAF) / LOOP_FADE_M, 0, 1)
```

At or above the chimney top this is identical to the baseline formula. Below the chimney
top the head falls with the submerged column heights. Below `Z_TAF + LOOP_FADE_M` the
separated-liquid return path is lost and flow decays to `W_MIN` (A-P4-6).
`k_fric` is calibrated once at rated conditions; flows at 30 %, 60 % and decay-heat
conditions are reported, not calibrated (table filled in Task 8):

| Condition | Core flow kg/s | Core void | Chimney void |
|---|---|---|---|
| 100 % / rated feed | (measured in Task 8) | | |
| 60 % | | | |
| 30 % | | | |
| Hot standby decay heat | | | |

No density-wave or regional stability behaviour is modelled or claimed.

## 7. Moderator temperature

`moderator_t = (bb * (t_in + t_top)/2 + (CORE_HEIGHT - bb) * t_sat) / CORE_HEIGHT`
where `bb` is the boiling boundary height, `t_in` the core inlet liquid temperature, and
`t_top = t_sat` if boiling occurs, else the exit liquid temperature. This replaces the
baseline use of the bulk saturation temperature for `MOD_TEMP_COEF`.

## 8. Tests

| Behaviour | Test |
|---|---|
| Property reference points | `steam::tests::reference_points_iapws`, `compressibility_in_published_range` |
| EOS round-trip in every regime | `vessel::tests::eos_roundtrip_all_regimes` |
| EOS bisection monotonicity across the water-solid branch | `vessel::tests::state_energy_is_monotonic_in_temperature` |
| Regime boundaries continuous, label changes once | `vessel_transients::regime_boundaries_are_continuous_and_cross_once` |
| Vented heatup purges air, ledger closes | `vessel_transients::vented_heatup_purges_air_and_closes_ledger` |
| Cold vented vessel at atmospheric pressure | `vessel_transients::cold_vented_vessel_is_atmospheric` |
| Heatup with trapped air | `vessel_transients::closed_vessel_heatup_transitions_gas_to_saturated` |
| Cooldown | `vessel_transients::sealed_cooldown_draws_vacuum` |
| Pressure step | `vessel_transients::adding_energy_raises_pressure` |
| Water-solid | `vessel_transients::water_solid_pressure_rises_steeply_and_leaves_envelope` |
| Shrink/swell | `vessel_transients::pressure_step_shrinks_then_recovers`, `..._swells` |
| Low level | `vessel_transients::draining_breaks_natural_circulation_in_order` |
| Steady consistency | `vessel_transients::steady_points_balance_momentum`, `flow_is_positive_and_rises_with_power` |
| Baseline equivalence | `vessel_transients::head_formula_matches_baseline_above_chimney_top` |
| Moderator temperature | `vessel_transients::moderator_temperature_between_inlet_and_saturation` |
| Conservation | `vessel_transients::steady_cases_conserve_mass_and_energy` |
```

- [ ] **Step 2: Point `docs/model/coverage.md` vessel row at this file**

Edit the "Vessel and circulation" row of `docs/model/coverage.md` to read `docs/model/vessel.md` under the model-description column and `tests/vessel_transients.rs` under the test column.

- [ ] **Step 3: Commit**

```bash
git add docs/model/vessel.md docs/model/coverage.md
git commit -m "docs(model): vessel envelope, regimes and natural-circulation specification

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 2: Steam table with liquid compressibility and reference-value tests

**Files:**
- Modify: `tools/gen_steam_table.py`
- Regenerate: `crates/bwr-core/src/steam_table.rs`
- Modify: `crates/bwr-core/src/steam.rs`
- Modify: `docs/model/sources.csv`, `docs/model/parameters.csv`

**Interfaces:**
- Produces: `pub const T_TABLE_MIN: f64 = 1.0`, `pub const T_TABLE_MAX: f64 = 350.0`, `Sat.kappa_f: f64` (1/Pa), `pub fn compressed_liquid_rho(t: f64, p: f64) -> f64`.

- [ ] **Step 1: Write the failing tests in `crates/bwr-core/src/steam.rs`**

Append inside the existing `mod tests`:

```rust
    #[test]
    fn reference_points_iapws() {
        // (p Pa, Tsat degC, v_f m3/kg, v_g m3/kg, h_f J/kg, h_g J/kg) from standard
        // IAPWS-IF97 saturation tables (sources.csv: SRC-IAPWS-TABLES).
        let pts = [
            (100.0e3, 99.61, 0.001043, 1.6941, 417.51e3, 2675.0e3),
            (1.0e6, 179.88, 0.001127, 0.19436, 762.51e3, 2777.1e3),
            (7.0e6, 285.83, 0.001352, 0.027378, 1267.5e3, 2772.6e3),
        ];
        let rel = |a: f64, b: f64| ((a - b) / b).abs();
        for (p, t, vf, vg, hf, hg) in pts {
            let s = sat_p(p);
            assert!((s.t - t).abs() < 0.3, "Tsat({p}) = {}", s.t);
            assert!(rel(s.v_f(), vf) < 3e-3, "v_f({p}) = {}", s.v_f());
            assert!(rel(s.v_g(), vg) < 3e-3, "v_g({p}) = {}", s.v_g());
            assert!(rel(s.h_f, hf) < 3e-3, "h_f({p}) = {}", s.h_f);
            assert!(rel(s.h_g, hg) < 3e-3, "h_g({p}) = {}", s.h_g);
        }
    }

    #[test]
    fn compressibility_in_published_range() {
        // Isothermal compressibility of liquid water: ~4.59e-10 1/Pa at 20 degC;
        // rises to the 1e-9 range near 300 degC. Executor: after regenerating the
        // table, print sat_t(300.0).kappa_f and tighten the 300 degC bounds to +/-10 %
        // of the CoolProp value in this test and in parameters.csv.
        let k20 = sat_t(20.0).kappa_f;
        assert!(k20 > 4.4e-10 && k20 < 4.7e-10, "kappa_f(20) = {k20}");
        let k300 = sat_t(300.0).kappa_f;
        assert!(k300 > 1.5e-9 && k300 < 3.5e-9, "kappa_f(300) = {k300}");
        let ratio = compressed_liquid_rho(20.0, 7.0e6) / rho_liquid(20.0);
        assert!(ratio > 1.0030 && ratio < 1.0034, "7 MPa compression at 20 degC = {ratio}");
        // At or below saturation pressure the correction is zero.
        assert_eq!(compressed_liquid_rho(150.0, 0.1e6), rho_liquid(150.0));
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --release -p bwr-core steam::tests -- --nocapture`
Expected: compile error `no field kappa_f on type Sat` and `cannot find function compressed_liquid_rho`.

- [ ] **Step 3: Update the generator script**

Replace the whole of `tools/gen_steam_table.py` with:

```python
"""Generate the saturated-water table embedded in bwr-core (IAPWS-IF97 via CoolProp).

Run: python3 tools/gen_steam_table.py > crates/bwr-core/src/steam_table.rs

Columns: T [degC], p_sat [Pa], rho_f [kg/m3], rho_g [kg/m3], h_f [J/kg], h_g [J/kg],
kappa_f [1/Pa]. kappa_f is the isothermal compressibility of the saturated liquid,
(1/rho) d(rho)/dp, from a finite difference of IF97 liquid density evaluated 0.1 MPa
and 0.3 MPa above saturation (both points lie in IF97 region 1 up to 350 degC).
"""
import CoolProp.CoolProp as CP

F = "IF97::Water"
rows = []
T = 1.0
while T <= 350.0 + 1e-9:
    Tk = T + 273.15
    p = CP.PropsSI("P", "T", Tk, "Q", 0, F)
    rf = CP.PropsSI("D", "T", Tk, "Q", 0, F)
    rg = CP.PropsSI("D", "T", Tk, "Q", 1, F)
    hf = CP.PropsSI("H", "T", Tk, "Q", 0, F)
    hg = CP.PropsSI("H", "T", Tk, "Q", 1, F)
    p1, p2 = p + 1.0e5, p + 3.0e5
    r1 = CP.PropsSI("D", "T", Tk, "P", p1, F)
    r2 = CP.PropsSI("D", "T", Tk, "P", p2, F)
    kf = (r2 - r1) / (r1 * (p2 - p1))
    rows.append((T, p, rf, rg, hf, hg, kf))
    T += 1.0

print("// Generated by tools/gen_steam_table.py from IAPWS-IF97 (CoolProp). Do not edit by hand.")
print("// Columns: T [degC], p_sat [Pa], rho_f [kg/m3], rho_g [kg/m3], h_f [J/kg], h_g [J/kg], kappa_f [1/Pa]")
print(f"pub const SAT_TABLE: [[f64; 7]; {len(rows)}] = [")
for r in rows:
    print("    [" + ", ".join(f"{v:.6e}" for v in r) + "],")
print("];")
```

- [ ] **Step 4: Regenerate the table**

Run: `python3 -c "import CoolProp; print(CoolProp.__version__)"` (if this fails: `pip3 install CoolProp`, which is a tool-side Python package already required by the baseline script, not a new project dependency).
Run: `python3 tools/gen_steam_table.py > crates/bwr-core/src/steam_table.rs`
Run: `head -4 crates/bwr-core/src/steam_table.rs && grep -c '^\s*\[' crates/bwr-core/src/steam_table.rs`
Expected: header mentions `kappa_f`, type `[[f64; 7]; 350]`, 350 rows. Record the CoolProp version in `docs/model/sources.csv` (Step 7).

- [ ] **Step 5: Update `steam.rs`**

Replace the constants and `Sat`, `row`, `lerp` and add `compressed_liquid_rho`:

```rust
/// Standard atmospheric pressure, Pa.
pub const P_ATM: f64 = 101_325.0;
/// First and last table temperatures, degC. The EOS envelope is this range.
pub const T_TABLE_MIN: f64 = 1.0;
pub const T_TABLE_MAX: f64 = 350.0;

const T0: f64 = T_TABLE_MIN;
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
    /// Isothermal compressibility of the saturated liquid, 1/Pa.
    pub kappa_f: f64,
}
```

```rust
fn row(i: usize) -> Sat {
    let r = SAT_TABLE[i];
    Sat { t: r[0], p: r[1], rho_f: r[2], rho_g: r[3], h_f: r[4], h_g: r[5], kappa_f: r[6] }
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
        kappa_f: l(a.kappa_f, b.kappa_f),
    }
}
```

Append after `rho_liquid`:

```rust
/// Density of compressed liquid water at `t` (degC) and total pressure `p` (Pa):
/// rho_f(t) * (1 + kappa_f(t) * (p - p_sat(t))). Linear in pressure, which is
/// accurate to better than 0.1 % up to 10 MPa above saturation. At or below the
/// saturation pressure the saturated-liquid density is returned.
pub fn compressed_liquid_rho(t: f64, p: f64) -> f64 {
    let s = sat_t(t);
    s.rho_f * (1.0 + s.kappa_f * (p - s.p).max(0.0))
}
```

- [ ] **Step 6: Run the tests**

Run: `cargo test --release -p bwr-core steam::tests -- --nocapture`
Expected: `known_points`, `reference_points_iapws`, `compressibility_in_published_range` all `ok`. Then run `cargo test --release -p bwr-core` to confirm nothing else broke (the extra column is only read through `row()`).

- [ ] **Step 7: Register rows**

Append to `docs/model/sources.csv` (use the column order of the Package 1 header) a row with id `SRC-IAPWS-TABLES`, title "Saturated water and steam tables (IAPWS-IF97 values as tabulated in standard thermodynamics texts)", revision "IF97 (2007)", jurisdiction "n/a", document kind "accepted methodology", retrieval date today, and in notes the three reference points used in `reference_points_iapws`. Update the existing CoolProp/IAPWS row (or add `SRC-COOLPROP`) with the CoolProp version printed in Step 4.

Append to `docs/model/parameters.csv` (locked header `id,value,unit,definition,reference_state,source_design,status,source_ids,interval_lo,interval_hi,assumption_id,location,notes`), filling `<k20>` with the value printed by `compressibility_in_published_range`:

```csv
KAPPA_F_TABLE,table,1/Pa,isothermal compressibility of saturated liquid (SAT_TABLE column 7),saturation line 1..350 degC,water property,confirmed,SRC-IAPWS-TABLES;SRC-COOLPROP,,,,crates/bwr-core/src/steam_table.rs,finite difference of IF97 region-1 density 0.1-0.3 MPa above saturation; 20 degC value <k20>
T_TABLE_MIN,1.0,degC,first saturation-table temperature,,water property,confirmed,SRC-IAPWS-TABLES,,,,crates/bwr-core/src/steam.rs,
T_TABLE_MAX,350.0,degC,last saturation-table temperature,,water property,confirmed,SRC-IAPWS-TABLES,,,,crates/bwr-core/src/steam.rs,
```

Run: `cargo test --release -p bwr-core --test register`
Expected: pass (every constant has a row).

- [ ] **Step 8: Commit**

```bash
git add tools/gen_steam_table.py crates/bwr-core/src/steam_table.rs crates/bwr-core/src/steam.rs docs/model/sources.csv docs/model/parameters.csv
git commit -m "feat(steam): liquid compressibility column and IAPWS reference-point tests

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 3: Vessel regimes, noncondensable gas and water-solid EOS

**Files:**
- Modify: `crates/bwr-core/src/vessel.rs` (`Eos`, `Regime`, `solve_eos`, `energy_for`, `Vessel` fields, `Vessel::new`, `update_level`, `sat_for_core`, `steam_available`)
- Modify: `crates/bwr-core/src/sim.rs` (call sites in the table above; snapshot)
- Modify: `crates/bwr-core/src/api.rs`, `ui/src/sim/types.ts`, `ui/src/sim/mock.ts`
- Modify: `crates/bwr-core/src/validation.rs` (`run_case` sampling of `gas_mass_kg`, `t_coolant_c`)
- Modify: any Package 3 test calling `solve_eos`

**Interfaces:**
- Consumes: `steam::compressed_liquid_rho`, `Sat.kappa_f`, `domain::DomainError`, Package 3's `pub const T_EOS_MIN: f64 = 1.0` and `pub const T_EOS_MAX: f64 = 349.5` in `vessel.rs` (not redeclared here), `Vessel.last_error`, `Vessel.exhausted`.
- Produces (master contract): `pub enum Regime { Vented, Saturated, WaterSolid, GasBlanketed }`, `pub struct Eos { t, p, p_steam, p_gas, x, regime }`, `pub const R_AIR: f64 = 287.0`, `pub fn solve_eos(mass: f64, energy: f64, gas_mass: f64, vent_open: bool) -> Result<Eos, DomainError>`, `Vessel.gas_mass: f64`, `Vessel::steam_available(&self) -> bool`, `Snapshot.vessel.regime`.

- [ ] **Step 1: Write the failing unit tests in `vessel.rs`**

Replace the existing `eos_roundtrip` test with:

```rust
    #[test]
    fn eos_roundtrip_all_regimes() {
        // Saturated / vacuum (no gas) and vented at three temperatures.
        for &t in &[40.0, 150.0, 287.0] {
            let m = 200_000.0;
            let e = energy_for(m, t);
            let eos = solve_eos(m, e, 0.0, false).unwrap();
            assert!((eos.t - t).abs() < 1e-3, "t {t} -> {}", eos.t);
            assert_eq!(eos.regime, Regime::Saturated);
            assert!((eos.p - eos.p_steam).abs() < 1.0, "no gas: p == p_steam");
            let eos = solve_eos(m, e, 0.0, true).unwrap();
            assert_eq!(eos.regime, Regime::Vented);
        }
        // Gas-blanketed: cold vessel with enough air for atmospheric pressure.
        let m = 200_000.0;
        let (_, _, v_liq) = state_at(40.0, m);
        let v_gas = V_VESSEL_FLUID - v_liq;
        let gas = (P_ATM - sat_t(40.0).p) * v_gas / (R_AIR * 313.15);
        let eos = solve_eos(m, energy_for(m, 40.0), gas, false).unwrap();
        assert_eq!(eos.regime, Regime::GasBlanketed);
        assert!((eos.p - P_ATM).abs() < 500.0, "p = {}", eos.p);
        assert!(eos.p_gas > eos.p_steam);
        // Water-solid: more water than the vessel holds at saturation density.
        let m_solid = V_VESSEL_FLUID * sat_t(40.0).rho_f * 1.0001;
        let eos = solve_eos(m_solid, energy_for(m_solid, 40.0), 0.0, false).unwrap();
        assert_eq!(eos.regime, Regime::WaterSolid);
        assert_eq!(eos.x, 0.0);
        let expect = sat_t(40.0).p + 1.0e-4 / sat_t(40.0).kappa_f;
        assert!((eos.p - expect).abs() / expect < 0.02, "p = {} vs {expect}", eos.p);
        // Water-solid beyond 2 x design pressure is a domain error.
        let m_over = V_VESSEL_FLUID * sat_t(40.0).rho_f * 1.02;
        assert!(matches!(
            solve_eos(m_over, energy_for(m_over, 40.0), 0.0, false),
            Err(DomainError::PropertyOutOfRange { .. })
        ));
        // Energy outside the table is a domain error, not a clamp.
        assert!(matches!(solve_eos(m, energy_for(m, 1.0) - 1.0e9, 0.0, false), Err(DomainError::EosOutOfRange { .. })));
    }

    #[test]
    fn state_energy_is_monotonic_in_temperature() {
        // The bisection in solve_eos assumes total energy increases with temperature at
        // fixed mass, across the two-phase / water-solid branch of state_at. Masses:
        // two-phase throughout; water-solid below ~45 degC then two-phase; always solid.
        for &m in &[150_000.0, 200_000.0, V_VESSEL_FLUID * 990.0, V_VESSEL_FLUID * 1000.5] {
            let mut prev = f64::MIN;
            let mut t = T_EOS_MIN;
            while t <= T_EOS_MAX {
                let e = state_at(t, m).0;
                assert!(e > prev, "energy not increasing at t={t} mass={m}: {e} <= {prev}");
                prev = e;
                t += 0.25;
            }
        }
    }

    #[test]
    fn new_cold_vessel_is_vented_at_atmospheric() {
        let v = Vessel::new(40.0, Z_NWL, 0.3e6, 0.0, 40.0, true);
        assert_eq!(v.eos.regime, Regime::Vented);
        assert!((v.eos.p - P_ATM).abs() < 500.0, "p = {}", v.eos.p);
        assert!(v.gas_mass > 50.0 && v.gas_mass < 500.0, "gas {}", v.gas_mass);
        assert!(!v.steam_available());
        assert!(v.last_error.is_none() && v.exhausted.is_none());
        let h = Vessel::new(sat_p(P_RATED).t, Z_NWL, RATED_POWER_W, STEAM_FLOW_RATED, T_FW_RATED, false);
        assert_eq!(h.eos.regime, Regime::Saturated);
        assert_eq!(h.gas_mass, 0.0);
        assert!(h.steam_available());
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --release -p bwr-core vessel::tests`
Expected: compile errors (`Regime` not found, `solve_eos` takes 2 arguments, `state_at` not found).

- [ ] **Step 3: Implement `Regime`, `Eos`, `state_at`, `solve_eos`**

In `vessel.rs`, replace the `Eos` struct, `energy_at`, `solve_eos`, and `energy_for` with:

```rust
use crate::domain::DomainError;
use crate::steam::{self, compressed_liquid_rho, sat_p, sat_t, Sat, P_ATM};
use serde::{Deserialize, Serialize};

// `pub const T_EOS_MIN: f64 = 1.0;` and `pub const T_EOS_MAX: f64 = 349.5;` already
// exist from Package 3 and are used below; do not redeclare them.

/// Gas constant of air (noncondensable in the head), J/kg/K.
pub const R_AIR: f64 = 287.0;
/// Gas below this mass is treated as dissolved when the vessel goes water-solid, kg.
pub const WATER_SOLID_GAS_TOLERANCE_KG: f64 = 1.0;
/// Hysteresis ratio on p_gas / p_steam for the Saturated <-> GasBlanketed label
/// (applied in Vessel::step; solve_eos itself is pure). ASSUMED, display only.
pub const REGIME_HYSTERESIS: f64 = 1.05;

/// Derives per master r2 Derives rule (nested in Sim via Vessel.eos and placed in api.rs).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Regime {
    Vented,
    Saturated,
    WaterSolid,
    GasBlanketed,
}

/// Equilibrium state of the vessel fluid. Keeps the baseline derives (Clone, Copy, Debug).
#[derive(Clone, Copy, Debug)]
pub struct Eos {
    pub t: f64,
    /// Total pressure in the steam space (liquid pressure when water-solid), Pa.
    pub p: f64,
    /// Steam partial pressure = p_sat(t), Pa.
    pub p_steam: f64,
    /// Noncondensable partial pressure, Pa.
    pub p_gas: f64,
    /// Static steam quality of the water inventory (steam mass / water mass).
    pub x: f64,
    pub regime: Regime,
}

/// (total energy incl. metal, static quality, liquid volume) for water `mass` at `t`.
fn state_at(t: f64, mass: f64) -> (f64, f64, f64) {
    let s = sat_t(t);
    let v = V_VESSEL_FLUID / mass;
    if v <= s.v_f() {
        // Water-solid: all liquid, slightly compressed. Pressure work on the liquid
        // energy is neglected (A-P4-3).
        return (mass * s.u_f() + METAL_HEAT_CAP * t, 0.0, V_VESSEL_FLUID);
    }
    let x = ((v - s.v_f()) / (s.v_g() - s.v_f())).clamp(0.0, 1.0);
    let v_liq = mass * (1.0 - x) * s.v_f();
    (mass * (s.u_f() + x * (s.u_g() - s.u_f())) + METAL_HEAT_CAP * t, x, v_liq)
}

/// Solve temperature, pressures, quality and regime from water mass, total energy
/// (water + metal), noncondensable gas mass and vent status.
pub fn solve_eos(mass: f64, energy: f64, gas_mass: f64, vent_open: bool) -> Result<Eos, DomainError> {
    if !(mass > 0.0) || !energy.is_finite() {
        return Err(DomainError::EosOutOfRange { mass_kg: mass, energy_j: energy });
    }
    if energy < state_at(T_EOS_MIN, mass).0 || energy > state_at(T_EOS_MAX, mass).0 {
        return Err(DomainError::EosOutOfRange { mass_kg: mass, energy_j: energy });
    }
    let (mut lo, mut hi) = (T_EOS_MIN, T_EOS_MAX);
    for _ in 0..60 {
        let mid = 0.5 * (lo + hi);
        if state_at(mid, mass).0 < energy {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let t = 0.5 * (lo + hi);
    let (_, x, v_liq) = state_at(t, mass);
    let s = sat_t(t);
    let t_k = t + 273.15;

    if mass * s.v_f() >= V_VESSEL_FLUID {
        if gas_mass > WATER_SOLID_GAS_TOLERANCE_KG {
            return Err(DomainError::PropertyOutOfRange { what: "gas space volume".into(), value: 0.0 });
        }
        let p = s.p + (mass * s.v_f() - V_VESSEL_FLUID) / (V_VESSEL_FLUID * s.kappa_f);
        if p > 2.0 * VESSEL_DESIGN_PRESSURE {
            return Err(DomainError::PropertyOutOfRange { what: "water-solid pressure".into(), value: p });
        }
        return Ok(Eos { t, p, p_steam: s.p, p_gas: 0.0, x: 0.0, regime: Regime::WaterSolid });
    }

    let v_gas = V_VESSEL_FLUID - v_liq;
    let p_gas = gas_mass * R_AIR * t_k / v_gas;
    if p_gas > 2.0 * VESSEL_DESIGN_PRESSURE {
        return Err(DomainError::PropertyOutOfRange { what: "gas partial pressure".into(), value: p_gas });
    }
    let p = s.p + p_gas;
    let regime = if vent_open {
        Regime::Vented
    } else if p_gas > s.p {
        Regime::GasBlanketed
    } else {
        Regime::Saturated
    };
    Ok(Eos { t, p, p_steam: s.p, p_gas, x, regime })
}

/// Total energy (water + metal) for a water mass at temperature t.
pub fn energy_for(mass: f64, t: f64) -> f64 {
    state_at(t, mass).0
}
```

- [ ] **Step 4: Add `gas_mass` and update `Vessel::new`, `update_level`, `sat_for_core`, add `steam_available`**

Add to the `Vessel` struct (after `energy`):

```rust
    /// Noncondensable gas (air) in the head, kg.
    pub gas_mass: f64,
```

In `Vessel::new`, change the struct literal and the initialisation tail:

```rust
        let mut v = Self {
            mass: 0.0,
            energy: 0.0,
            gas_mass: 0.0,
            eos: Eos { t, p: P_ATM, p_steam: 0.0, p_gas: 0.0, x: 0.0, regime: Regime::Vented },
            // ... remaining fields unchanged (core_flow, h_in, alpha_core, alpha_ch, core,
            // level_z, collapsed_level_z, head_vent_open, vent_flow, heatup_rate, k_fric,
            // drive_dp, exhausted: None, last_error: None)
        };
        // A vessel colder than 100 degC sits at atmospheric pressure under air;
        // a hotter one is at saturation pressure with no gas.
        let p0 = sat_t(t).p.max(P_ATM);
        let s = sat_p(p0);
        let h_fw = steam::h_liquid(t_feed);
        for _ in 0..200 {
            // ... unchanged relaxation loop ...
        }
        v.drive_dp = v.drive(&s).0;
        // Inventory: liquid volume to reach the requested level, plus dome steam.
        let v_l = liquid_volume_below(level_z, v.alpha_core, v.alpha_ch);
        let rho_l = compressed_liquid_rho(t, p0);
        let v_g = (V_VESSEL_FLUID - v_l).max(0.0);
        let sg = sat_t(t);
        v.mass = v_l * rho_l + v_g * sg.rho_g;
        v.energy = energy_for(v.mass, t);
        v.gas_mass = if v_g > 0.0 && sg.p < P_ATM { (P_ATM - sg.p) * v_g / (R_AIR * (t + 273.15)) } else { 0.0 };
        // An invalid initial condition must stay visible (master: finalize_init retains
        // domain errors), so it is recorded, not panicked on; the fallback eos is the
        // atmospheric placeholder set above.
        match solve_eos(v.mass, v.energy, v.gas_mass, vent_open) {
            Ok(e) => v.eos = e,
            Err(e) => v.last_error = Some(e),
        }
        v.update_level();
        v
```

(Delete the old `let s = if t < 100.0 { sat_p(P_ATM) } else { sat_t(t) };` and the old `rho_l`/`m_g` lines.) `Vessel::new` adds no settling; the scenario constructors in `sim.rs` are untouched by this task and still end with `Sim::finalize_init()`, which re-bases the ledger from the vessel built here and drains `vessel.last_error` into `domain_errors` at `t = 0.0`.

Replace `update_level`:

```rust
    fn update_level(&mut self) {
        let rho_l = compressed_liquid_rho(self.eos.t, self.eos.p);
        let v_l = self.mass * (1.0 - self.eos.x) / rho_l;
        self.level_z = level_from_volume(v_l, self.alpha_core, self.alpha_ch);
        self.collapsed_level_z = level_from_volume(v_l, 0.0, 0.0);
    }
```

Replace `sat_for_core` and add `steam_available`:

```rust
    /// Saturation state the core boils against: the total vessel pressure. A cold
    /// air-filled vessel at 0.1 MPa therefore does not boil below 100 degC. The
    /// downcomer hydrostatic head is neglected (A-P4-5).
    fn sat_for_core(&self) -> Sat {
        sat_p(self.eos.p)
    }

    /// True when the dome holds steam above atmospheric pressure that the isolation
    /// condensers can draw. Deliberately independent of the Saturated/GasBlanketed
    /// label (which may flip near p_gas ~ p_sat): it depends on the steam partial
    /// pressure and on there being a steam space at all. Gas-rich steam degrades IC
    /// condensation; that is Package 7's noncondensable sensitivity, not a cutoff here.
    pub fn steam_available(&self) -> bool {
        self.eos.regime != Regime::WaterSolid && self.eos.p_steam > P_ATM
    }
```

In `Vessel::step` (full replacement comes in Task 4) make the minimum edits so it compiles now: `let h_liq = steam::h_liquid(self.eos.t);`, `let w_steam = if self.steam_available() { inp.w_steam } else { 0.0 };`, vent condition `self.head_vent_open && self.eos.p > P_ATM`, and the `solve_eos` call becomes `solve_eos(self.mass, self.energy, self.gas_mass, self.head_vent_open)` with the Package 3 error handling unchanged.

- [ ] **Step 5: Update `sim.rs` call sites**

Apply every row of the call-site table at the top of this plan. Add `use crate::vessel::{Regime, Vessel, VesselInputs};`. In `snapshot()` add `regime: v.eos.regime,` to the `api::Vessel` literal. In `active_alarms` add after the `solid` alarm:

```rust
        if self.vessel.eos.regime == Regime::GasBlanketed && self.vessel.eos.t > 90.0 {
            add("gas_blanket", "NONCONDENSABLE GAS IN VESSEL HEAD".into(), Severity::Warning);
        }
```

- [ ] **Step 6: Update the API contract**

`crates/bwr-core/src/api.rs`, in `pub struct Vessel`, add after `heatup_rate_cph`:

```rust
    pub regime: crate::vessel::Regime,
```

`ui/src/sim/types.ts`, after `ScenarioName`:

```ts
export type Regime = "vented" | "saturated" | "waterSolid" | "gasBlanketed";
```

and in `interface Vessel` after `heatupRateCph: number;`:

```ts
  /** Thermodynamic regime of the vessel inventory (see docs/model/vessel.md). */
  regime: Regime;
```

`ui/src/sim/mock.ts`: run `grep -n "headVentOpen:" ui/src/sim/mock.ts` to find the vessel literal in the snapshot builder and add on the next line `regime: <headVentOpen expression> ? "vented" : "saturated",` using the same expression the mock uses for `headVentOpen`. Add `Regime` to the type import list at the top of `mock.ts` only if the literal needs a cast (it should not).

Run: `grep -n "subcooled\|\.solid" ui/src/components -r` — expected: no matches (the mimic reads only Snapshot fields).

Trace schema v2 (Package 2 emits these as 0.0 "populated from Package 4"): in `crates/bwr-core/src/validation.rs` `run_case` sampling, set column `gas_mass_kg` from `sim.vessel.gas_mass` and `t_coolant_c` from `sim.vessel.eos.t` (use `col_index("gas_mass_kg")` / `col_index("t_coolant_c")`, never a literal index). `moderator_t_c` is populated in Task 6 once `moderator_t()` exists. Update the "populated from Package N" note for these two columns in `docs/validation/method.md`.

- [ ] **Step 7: Fix Package 3 call sites and run everything**

Run: `grep -rn "solve_eos(" crates/bwr-core/tests crates/bwr-core/src` and change any two-argument call to `solve_eos(m, e, 0.0, false)`.
Run: `cargo test --release -p bwr-core`
Expected: all tests pass, including `eos_roundtrip_all_regimes`, `new_cold_vessel_is_vented_at_atmospheric`, and the unchanged `plant.rs` tests (cold shutdown still vented at `P_ATM`; hot standby and full power have `gas_mass = 0` and identical pressure).
Run: `npm --prefix ui run typecheck`
Expected: exit 0.

- [ ] **Step 8: Commit**

```bash
git add crates/bwr-core/src/vessel.rs crates/bwr-core/src/sim.rs crates/bwr-core/src/api.rs crates/bwr-core/src/validation.rs crates/bwr-core/tests ui/src/sim/types.ts ui/src/sim/mock.ts docs/validation/method.md
git commit -m "feat(vessel): regimes, noncondensable gas and water-solid equation of state

Replaces the 'below 100 degC is atmospheric' rule with Vented / Saturated /
GasBlanketed / WaterSolid regimes; water-solid pressure uses liquid
compressibility; gas partial pressure uses the ideal-gas law.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 4: Head-vent gas dynamics and regime-dependent outflows

**Files:**
- Modify: `crates/bwr-core/src/vessel.rs` (`step`, constants)
- Create: `crates/bwr-core/tests/vessel_transients.rs`

**Interfaces:**
- Consumes: `ledger::BoundaryFlows`, `Vessel.last_error`.
- Produces: `pub const VENT_BOILOFF_KGS_PER_K: f64 = 0.02` (moved from the inline `0.02`), `Vessel::step` returning `BoundaryFlows` with vented gas handled outside the water ledger.

- [ ] **Step 1: Write the failing tests**

Create `crates/bwr-core/tests/vessel_transients.rs`:

```rust
//! Vessel thermal-hydraulic transients across the operating envelope (Package 4).

use bwr_core::api::{Command, ScenarioName};
use bwr_core::domain::DomainError;
use bwr_core::params::*;
use bwr_core::sim::DT;
use bwr_core::steam::{self, sat_p, sat_t, P_ATM};
use bwr_core::vessel::{Regime, Vessel, VesselInputs};
use bwr_core::Sim;

fn run(sim: &mut Sim, seconds: f64) {
    let steps = (seconds / DT).round() as usize;
    for _ in 0..steps {
        sim.step(DT);
    }
}

fn inputs(q_core: f64, q_ic: f64, w_liquid_out: f64) -> VesselInputs {
    VesselInputs { q_core, q_ic, w_feed: 0.0, h_feed: 0.0, w_steam: 0.0, w_liquid_out }
}

#[test]
fn cold_vented_vessel_is_atmospheric() {
    let mut v = Vessel::new(40.0, Z_NWL, 0.3e6, 0.0, 40.0, true);
    for _ in 0..(600.0 / DT) as usize {
        v.step(DT, &inputs(0.3e6, 0.0, 0.0));
    }
    assert_eq!(v.eos.regime, Regime::Vented);
    assert!((v.eos.p - P_ATM).abs() < 1.0e3, "p = {}", v.eos.p);
    assert!(v.last_error.is_none());
}

#[test]
fn closed_vessel_heatup_transitions_gas_to_saturated() {
    let mut v = Vessel::new(40.0, Z_NWL, 0.0, 0.0, 40.0, false);
    assert_eq!(v.eos.regime, Regime::GasBlanketed);
    let (mut t_prev, mut p_prev) = (v.eos.t, v.eos.p);
    let mut t_switch = None;
    let mut hist: Vec<(f64, f64)> = Vec::new();
    let mut time = 0.0;
    while v.eos.t < 140.0 && time < 4.0 * 3600.0 {
        v.step(DT, &inputs(50.0e6, 0.0, 0.0));
        time += DT;
        assert!(v.eos.t >= t_prev - 1e-9, "temperature fell at t={time}");
        assert!(v.eos.p >= p_prev - 1.0, "pressure fell at t={time}: {} -> {}", p_prev, v.eos.p);
        if t_switch.is_none() && v.eos.regime == Regime::Saturated {
            t_switch = Some(v.eos.t);
        }
        hist.push((time, v.eos.t));
        t_prev = v.eos.t;
        p_prev = v.eos.p;
    }
    let ts = t_switch.expect("never became saturated");
    assert!(ts > 95.0 && ts < 115.0, "gas -> saturated at {ts} degC");
    assert!(v.eos.p > sat_t(v.eos.t).p, "trapped air adds to pressure");
    // Heatup-rate meter agrees with the finite difference over the last 10 minutes.
    let n = hist.len();
    let (t0, tt0) = hist[n - (600.0 / DT) as usize];
    let (t1, tt1) = hist[n - 1];
    let fd = (tt1 - tt0) / (t1 - t0) * 3600.0;
    assert!((v.heatup_rate - fd).abs() < 0.05 * fd, "meter {} vs {fd} degC/h", v.heatup_rate);
    assert!(v.last_error.is_none());
}

/// Steps `v` with heat `q` until `done`, returning every regime change (from, to).
fn regime_changes(v: &mut Vessel, q: f64, done: impl Fn(&Vessel) -> bool) -> Vec<(Regime, Regime, f64, f64)> {
    let mut out = Vec::new();
    let mut time = 0.0;
    while !done(v) && time < 4.0 * 3600.0 {
        let before = v.eos;
        v.step(DT, &inputs(q, 0.0, 0.0));
        time += DT;
        if v.eos.regime != before.regime {
            out.push((before.regime, v.eos.regime, before.p, v.eos.p));
        }
    }
    out
}

#[test]
fn regime_boundaries_are_continuous_and_cross_once() {
    // (a) Closed vessel with trapped air: GasBlanketed -> Saturated exactly once, with
    //     no pressure jump at the relabel (same formula on both sides).
    let mut v = Vessel::new(40.0, Z_NWL, 0.0, 0.0, 40.0, false);
    let ch = regime_changes(&mut v, 50.0e6, |v| v.eos.t >= 140.0);
    assert_eq!(ch.len(), 1, "regime changes: {ch:?}");
    assert_eq!((ch[0].0, ch[0].1), (Regime::GasBlanketed, Regime::Saturated));
    assert!((ch[0].3 - ch[0].2).abs() < 500.0, "pressure jump at relabel: {ch:?}");
    assert!(v.last_error.is_none());

    // (b) Vented cold vessel: closing the vent relabels once, pressure unchanged.
    let mut v = Vessel::new(40.0, Z_NWL, 0.0, 0.0, 40.0, true);
    for _ in 0..100 {
        v.step(DT, &inputs(0.0, 0.0, 0.0));
    }
    let p_before = v.eos.p;
    v.head_vent_open = false;
    v.step(DT, &inputs(0.0, 0.0, 0.0));
    assert_eq!(v.eos.regime, Regime::GasBlanketed);
    assert!((v.eos.p - p_before).abs() < 500.0, "closing the vent moved pressure by {}", v.eos.p - p_before);

    // (c) Nearly full saturated vessel (no gas): Saturated -> WaterSolid exactly once;
    //     pressure continuous (the jump at the crossing step is no larger than the
    //     steps that follow it, where compressibility sets the slope).
    let mut v = Vessel::new(150.0, VESSEL_HEIGHT - 0.02, 0.0, 0.0, 150.0, false);
    assert_eq!(v.gas_mass, 0.0);
    assert_eq!(v.eos.regime, Regime::Saturated);
    let mut p_hist = vec![v.eos.p];
    let mut cross_idx = None;
    let mut time = 0.0;
    while v.last_error.is_none() && v.eos.t < 160.0 && time < 3600.0 {
        let before = v.eos.regime;
        v.step(DT, &inputs(50.0e6, 0.0, 0.0));
        time += DT;
        p_hist.push(v.eos.p);
        if before != v.eos.regime {
            assert!(cross_idx.is_none(), "regime changed twice");
            assert_eq!((before, v.eos.regime), (Regime::Saturated, Regime::WaterSolid));
            cross_idx = Some(p_hist.len() - 1);
        }
    }
    let i = cross_idx.expect("never went water-solid");
    assert!(i + 21 < p_hist.len(), "too few steps after crossing");
    let jump = (p_hist[i] - p_hist[i - 1]).abs();
    let after = (i + 1..=i + 20).map(|k| (p_hist[k] - p_hist[k - 1]).abs()).fold(0.0, f64::max);
    assert!(jump <= 2.0 * after + 100.0, "pressure jump {jump} Pa at the water-solid crossing vs {after} Pa/step after");
    assert!(v.last_error.is_none(), "{:?}", v.last_error);

    // (d) Gas-blanketed vessel driven toward water-solid leaves the envelope through
    //     the gas-space check, never by a silent branch switch.
    let mut v = Vessel::new(40.0, VESSEL_HEIGHT - 0.5, 0.0, 0.0, 40.0, false);
    assert!(v.gas_mass > WATER_SOLID_GAS_TOLERANCE_KG);
    let ch = regime_changes(&mut v, 50.0e6, |v| v.last_error.is_some() || v.eos.t > 120.0);
    assert!(ch.is_empty(), "unexpected relabel {ch:?}");
    assert!(matches!(v.last_error, Some(DomainError::PropertyOutOfRange { .. })), "{:?}", v.last_error);
}

#[test]
fn vented_heatup_purges_air_and_closes_ledger() {
    use bwr_core::ledger::Ledger;
    let mut v = Vessel::new(40.0, Z_NWL, 0.0, 0.0, 40.0, true);
    let gas0 = v.gas_mass;
    let mut ledger = Ledger::new(v.mass, v.energy);
    let mut gas_prev = gas0;
    let mut time = 0.0;
    while v.eos.t < 120.0 && time < 4.0 * 3600.0 {
        let flows = v.step(DT, &inputs(20.0e6, 0.0, 0.0));
        ledger.record(DT, &flows);
        time += DT;
        assert_eq!(v.eos.regime, Regime::Vented);
        assert!(v.last_error.is_none(), "{:?}", v.last_error);
        assert!(v.gas_mass >= 0.0, "gas mass negative at t={time}");
        assert!(flows.w_vent >= 0.0 && flows.w_steam == 0.0 && flows.w_liquid_out == 0.0);
        if v.eos.p > P_ATM {
            assert!(v.gas_mass <= gas_prev + 1e-9, "air entered a vessel above atmospheric at t={time}");
        }
        gas_prev = v.gas_mass;
    }
    assert!(v.eos.t >= 120.0, "did not reach 120 degC: {}", v.eos.t);
    assert!(v.gas_mass < gas0, "no air purged: {} of {gas0} kg", v.gas_mass);
    let rm = ledger.mass_residual(v.mass);
    let re = ledger.energy_residual(v.energy);
    assert!(rm.abs() < 1e-6 * v.mass, "mass residual {rm} kg");
    assert!(re.abs() < 1e-6 * v.energy.abs(), "energy residual {re} J");
}

#[test]
fn sealed_cooldown_draws_vacuum() {
    let t0 = sat_p(P_RATED).t;
    let mut v = Vessel::new(t0, Z_NWL, 20.0e6, 0.0, 215.0, false);
    let (mut t_prev, mut p_prev) = (v.eos.t, v.eos.p);
    let mut time = 0.0;
    while v.eos.t > 90.0 && time < 4.0 * 3600.0 {
        v.step(DT, &inputs(20.0e6, 60.0e6, 0.0));
        time += DT;
        assert!(v.eos.t <= t_prev + 1e-9, "temperature rose at t={time}");
        assert!(v.eos.p <= p_prev + 1.0, "pressure rose at t={time}");
        assert_eq!(v.eos.regime, Regime::Saturated);
        assert_eq!(v.gas_mass, 0.0);
        t_prev = v.eos.t;
        p_prev = v.eos.p;
    }
    assert!(v.eos.t <= 90.0, "did not cool: {}", v.eos.t);
    assert!(v.eos.p < P_ATM, "a sealed vessel draws vacuum below 100 degC: p = {}", v.eos.p);
    assert!(v.last_error.is_none());
}

#[test]
fn adding_energy_raises_pressure() {
    let mut v = Vessel::new(sat_p(P_RATED).t, Z_NWL, 0.0, 0.0, 215.0, false);
    let p0 = v.eos.p;
    let e0 = v.energy;
    for _ in 0..(10.0 / DT) as usize {
        v.step(DT, &inputs(1.0e8, 0.0, 0.0));
    }
    assert!(v.energy > e0);
    assert!(v.eos.p > p0 + 1.0e4, "dp = {}", v.eos.p - p0);
}

#[test]
fn water_solid_pressure_rises_steeply_and_leaves_envelope() {
    let mut v = Vessel::new(40.0, VESSEL_HEIGHT, 0.0, 0.0, 40.0, true);
    v.head_vent_open = false;
    let mut p_at = std::collections::BTreeMap::<i64, f64>::new();
    let mut time = 0.0;
    while v.last_error.is_none() && time < 3600.0 {
        v.step(DT, &inputs(50.0e6, 0.0, 0.0));
        time += DT;
        p_at.entry(v.eos.t.floor() as i64).or_insert(v.eos.p);
        if v.last_error.is_none() {
            assert_eq!(v.eos.regime, Regime::WaterSolid, "at t={time} T={}", v.eos.t);
        }
    }
    let slope = (p_at[&43] - p_at[&41]) / 2.0;
    assert!(slope > 0.5e6, "water-solid dp/dT = {slope} Pa/K");
    assert!(
        matches!(v.last_error, Some(DomainError::PropertyOutOfRange { .. })),
        "expected envelope exit, got {:?}",
        v.last_error
    );
    assert!(v.eos.t < 80.0, "left envelope only at {} degC", v.eos.t);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --release -p bwr-core --test vessel_transients`
Expected: `closed_vessel_heatup_transitions_gas_to_saturated` fails (pressure or regime assertions, because vent/gas dynamics are not yet in `step`), `vented_heatup_purges_air_and_closes_ledger` fails (gas never purged; ledger energy residual from the interim `step`), `regime_boundaries_are_continuous_and_cross_once` fails on (a) or (b), and/or `water_solid_pressure_rises_steeply_and_leaves_envelope` fails on the regime assertion. `cold_vented_vessel_is_atmospheric` may pass already. Record which fail.

- [ ] **Step 3: Replace `Vessel::step`**

Add near the top of `vessel.rs`:

```rust
/// Extra vent flow of a hot vented vessel per degC above 100, kg/s/K. ASSUMED
/// (parameters.csv: VENT_BOILOFF_KGS_PER_K); existed inline in the baseline.
pub const VENT_BOILOFF_KGS_PER_K: f64 = 0.02;
```

Replace the body of `pub fn step(&mut self, dt: f64, inp: &VesselInputs) -> BoundaryFlows` up to the core-inlet enthalpy block with:

```rust
        let t_before = self.eos.t;
        let eos = self.eos;
        let s_steam = sat_t(eos.t);
        let h_liq = steam::h_liquid(eos.t);
        let heat_loss = HEAT_LOSS_HOT * ((eos.t - 20.0) / 268.0).max(0.0);

        // Steam-space composition. Steam and noncondensable gas leave through the
        // steam lines and the head vent in proportion to their masses, and no more
        // steam can leave in a step than the steam space holds, so every outflow is
        // continuous across regime boundaries (no label-dependent switch; see
        // docs/model/vessel.md section 4).
        let m_steam_space = (self.mass * eos.x).max(0.0);
        let m_gas = self.gas_mass.max(0.0);
        let y_steam = if m_steam_space + m_gas > 0.0 { m_steam_space / (m_steam_space + m_gas) } else { 0.0 };

        // Main steam lines: the requested draw, split by composition.
        let w_line_steam_raw = inp.w_steam.max(0.0) * y_steam;
        let w_line_gas_raw = inp.w_steam.max(0.0) * (1.0 - y_steam);

        // Head vent: relief above atmospheric, air ingress below (gas only, no energy).
        // A water-solid vessel has no steam space and the vent passes nothing (A-P4-2).
        let mut w_vent_steam_raw = 0.0;
        let mut w_vent_gas_raw = 0.0;
        let mut w_air_in = 0.0;
        if self.head_vent_open && eos.regime != Regime::WaterSolid {
            let dp = eos.p - P_ATM;
            if dp > 0.0 {
                let w = C_HEAD_VENT * dp + VENT_BOILOFF_KGS_PER_K * (eos.t - 100.0).max(0.0);
                w_vent_steam_raw = w * y_steam;
                w_vent_gas_raw = w * (1.0 - y_steam);
            } else {
                w_air_in = C_HEAD_VENT * (-dp);
            }
        }

        // Steam-space and gas caps: proportional scaling keeps the line/vent split.
        let steam_cap = m_steam_space / dt;
        let steam_raw = w_line_steam_raw + w_vent_steam_raw;
        let f_steam = if steam_raw > steam_cap && steam_raw > 0.0 { steam_cap / steam_raw } else { 1.0 };
        let gas_cap = m_gas / dt;
        let gas_raw = w_line_gas_raw + w_vent_gas_raw;
        let f_gas = if gas_raw > gas_cap && gas_raw > 0.0 { gas_cap / gas_raw } else { 1.0 };
        let w_steam_req = w_line_steam_raw * f_steam;
        let w_vent_steam_raw = w_vent_steam_raw * f_steam;
        let w_gas_out = gas_raw * f_gas;
        let h_steam_out = s_steam.h_g;
        let w_liquid_req = inp.w_liquid_out.max(0.0);

        // Transfer-allocation point (master step 8): bound ALL water outflows to the
        // inventory available this step, BEFORE solving the state. Before Package 7 the
        // IC removes energy only (inp.q_ic); Package 7 adds inp.w_ic_steam to `out_req`
        // and reports w_ic_steam / w_ic_return.
        self.exhausted = None;
        let out_req = (w_steam_req + w_vent_steam_raw + w_liquid_req) * dt;
        let avail = self.mass + inp.w_feed * dt;
        let scale = if out_req > avail && out_req > 0.0 { avail / out_req } else { 1.0 };
        if scale < 1.0 {
            let deficit = out_req - avail;
            self.exhausted = Some(deficit);
            self.last_error = Some(DomainError::InventoryExhausted { deficit_kg: deficit });
        }
        let w_steam = w_steam_req * scale;
        let w_vent_steam = w_vent_steam_raw * scale;
        let w_liquid_out = w_liquid_req * scale;
        self.vent_flow = w_vent_steam;

        // Energy uses exactly the products Ledger::record (P2) integrates from `flows`:
        //   out = (w_steam + w_vent) * h_steam + w_liquid_out * h_liquid_out + q_ic + q_loss
        //   in  = w_feed * h_feed + q_core
        // Gas carries no energy (A-P4-3), so air ingress and gas egress do not appear.
        // If Package 2's record() turns out to integrate different products, make this
        // expression term-identical to record(), never the reverse.
        let dm = inp.w_feed - w_steam - w_vent_steam - w_liquid_out;
        let de = inp.q_core - inp.q_ic - heat_loss + inp.w_feed * inp.h_feed
            - (w_steam + w_vent_steam) * h_steam_out
            - w_liquid_out * h_liq;
        let flows = BoundaryFlows {
            w_feed: inp.w_feed,
            h_feed: inp.h_feed,
            w_steam,
            h_steam: h_steam_out,
            w_vent: w_vent_steam,
            w_liquid_out,
            h_liquid_out: h_liq,
            w_ic_steam: 0.0,
            w_ic_return: 0.0,
            h_ic_return: 0.0,
            q_core: inp.q_core,
            q_ic: inp.q_ic,
            q_loss: heat_loss,
            mass_clipped: 0.0,
            energy_clipped: 0.0,
        };

        // Commit only if the new state is inside the envelope (master rejection
        // semantics): on Err roll mass, energy and gas_mass back to their entry values,
        // report nothing applied, and record the error.
        let (mass0, energy0, gas0) = (self.mass, self.energy, self.gas_mass);
        let gas_new = (gas0 + (w_air_in - w_gas_out) * dt).max(0.0);
        match solve_eos(mass0 + dm * dt, energy0 + de * dt, gas_new, self.head_vent_open) {
            Ok(mut e) => {
                // Label hysteresis (display only; nothing physical reads the label):
                // Saturated <-> GasBlanketed flips only once p_gas crosses p_steam by
                // REGIME_HYSTERESIS. Vented and WaterSolid are unaffected.
                if !self.head_vent_open {
                    e.regime = match (self.eos.regime, e.regime) {
                        (Regime::Saturated, Regime::GasBlanketed) if e.p_gas < REGIME_HYSTERESIS * e.p_steam => Regime::Saturated,
                        (Regime::GasBlanketed, Regime::Saturated) if e.p_gas * REGIME_HYSTERESIS > e.p_steam => Regime::GasBlanketed,
                        (_, r) => r,
                    };
                }
                self.mass = mass0 + dm * dt;
                self.energy = energy0 + de * dt;
                self.gas_mass = gas_new;
                self.eos = e;
            }
            Err(e) => {
                self.mass = mass0;
                self.energy = energy0;
                self.gas_mass = gas0;
                self.vent_flow = 0.0;
                self.exhausted = None;
                self.last_error = Some(e);
                return BoundaryFlows::default();
            }
        }
        let s = self.sat_for_core();
```

Keep the rest of `step` (core inlet enthalpy, voids, momentum, `update_level`, heatup rate) unchanged except that the function ends with `flows` as its return value. `Sim::step` step 9 (Package 3) continues to scale `steam.w_tcv`, `steam.w_bypass` and `fw.rwcu_reject` by `flows.w_steam / requested` so downstream consumers see applied transfers; nothing in this task changes that code. The vented gas is outside the water `Ledger` by design (gas is not part of `mass`); `docs/model/vessel.md` section 2 states this.

- [ ] **Step 4: Run the tests**

Run: `cargo test --release -p bwr-core --test vessel_transients`
Expected: all seven tests pass (`cold_vented_vessel_is_atmospheric`, `closed_vessel_heatup_transitions_gas_to_saturated`, `regime_boundaries_are_continuous_and_cross_once`, `vented_heatup_purges_air_and_closes_ledger`, `sealed_cooldown_draws_vacuum`, `adding_energy_raises_pressure`, `water_solid_pressure_rises_steeply_and_leaves_envelope`). Then `cargo test --release -p bwr-core` — all pass, including Package 3 `conservation.rs` (residual columns remain ~0 because `flows` reports exactly what was applied).

Package 3 rejection semantics are preserved by the body above: (a) `exhausted` is reset at the allocation point every step; (b) all water outflows (steam-line steam, vent steam, RWCU; IC steam from P7) are bounded to `mass + w_feed*dt` before `solve_eos` is called; (c) on `Err` mass, energy and gas_mass are restored to entry values, `vent_flow` and `exhausted` are cleared, `last_error` is set, and `BoundaryFlows::default()` is returned; (d) no field is written between the allocation and the `match`, so no partial update can escape. One documented consequence: if the same step both exhausts inventory and fails `solve_eos`, `last_error` holds the EOS error (the exhaustion is moot because nothing was applied).

Package 3 tests that can be affected, and the adjustment to make in the same commit:
- `tests/numerics.rs` / `tests/conservation.rs` calls of `solve_eos(m, e)` → `solve_eos(m, e, 0.0, false)` (Task 3 Step 7).
- Any construction of an `Eos { .. }` literal or read of `eos.subcooled` / `eos.solid` → use `p_steam: sat_t(t).p, p_gas: 0.0, regime: Regime::Saturated` and `eos.regime` (run `grep -n "subcooled\|\.solid\|Eos {" crates/bwr-core/tests`).
- A P3 assertion that `flows.h_steam == sat_p(vessel.eos.p).h_g` still holds whenever `gas_mass == 0` (then `p == p_steam`); with gas present `h_steam` is `sat_t(eos.t).h_g`, so such an assertion must read `sat_t(sim.vessel.eos.t).h_g`.
- The P3 coupled low-inventory regression (hotwell gains what the vessel released) is unaffected: `flows.w_steam` is the applied value and `Sim::step` step 9 scales consumers by `flows.w_steam / requested` as before; the new steam-space cap only lowers the applied value further when the steam space is nearly empty, which is a legitimate shortfall handled by the same ratio.

If `closed_vessel_heatup_transitions_gas_to_saturated` reports the switch outside 95..115 degC, print `v.gas_mass` and the gas-space volume at 40 degC and check the `gas_mass` initialisation in `Vessel::new` before touching the test bounds.

- [ ] **Step 5: Commit**

```bash
git add crates/bwr-core/src/vessel.rs crates/bwr-core/tests/vessel_transients.rs
git commit -m "feat(vessel): head-vent gas dynamics, regime-dependent outflows, envelope tests

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 5: Level-dependent natural-circulation head

**Files:**
- Modify: `crates/bwr-core/src/vessel.rs` (`drive`, `calibrate_friction`, `tau_dc`, `friction_dp`)
- Modify: `crates/bwr-core/tests/vessel_transients.rs`
- Modify: `docs/model/parameters.csv`

**Interfaces:**
- Produces: `pub const LOOP_FADE_M: f64 = 0.5`, `Vessel::friction_dp(&self) -> f64`, `Vessel.drive_dp` populated by `Vessel::new`.

- [ ] **Step 1: Write the failing tests**

Append to `tests/vessel_transients.rs`:

```rust
#[test]
fn head_formula_matches_baseline_above_chimney_top() {
    // At rated conditions with level above the chimney top the new hydrostatic
    // formula must equal the baseline expression exactly.
    let v = Vessel::new(sat_p(P_RATED).t, Z_NWL, RATED_POWER_W, STEAM_FLOW_RATED, T_FW_RATED, false);
    let s = sat_p(v.eos.p);
    let (_, rho_ch) = bwr_core::vessel::chimney_th(&s, v.core_flow, &v.core);
    let rho_dc = steam::compressed_liquid_rho(steam::t_from_h_liquid(v.h_in.min(s.h_f)), v.eos.p);
    let baseline = G * (rho_dc * (Z_CHIMNEY_TOP - Z_CORE_BOTTOM) - v.core.rho_avg * CORE_HEIGHT - rho_ch * CHIMNEY_HEIGHT);
    assert!(v.level_z >= Z_CHIMNEY_TOP);
    assert!((v.drive_dp - baseline).abs() < 1e-6 * baseline.abs(), "{} vs {baseline}", v.drive_dp);
}

#[test]
fn steady_points_balance_momentum() {
    for frac in [0.3, 0.6, 1.0] {
        let v = Vessel::new(sat_p(P_RATED).t, Z_NWL, frac * RATED_POWER_W, frac * STEAM_FLOW_RATED, T_FW_RATED, false);
        let res = (v.drive_dp - v.friction_dp()).abs();
        assert!(res < 1e-3 * v.drive_dp, "{frac}: drive {} friction {}", v.drive_dp, v.friction_dp());
    }
}

#[test]
fn flow_is_positive_and_rises_with_power() {
    let flow = |q: f64, w: f64| Vessel::new(sat_p(P_RATED).t, Z_NWL, q, w, T_FW_RATED, false).core_flow;
    let decay = flow(0.005 * RATED_POWER_W, 0.0);
    let w30 = flow(0.3 * RATED_POWER_W, 0.3 * STEAM_FLOW_RATED);
    let w60 = flow(0.6 * RATED_POWER_W, 0.6 * STEAM_FLOW_RATED);
    let w100 = flow(RATED_POWER_W, STEAM_FLOW_RATED);
    println!("core flow kg/s: decay {decay:.0} 30% {w30:.0} 60% {w60:.0} 100% {w100:.0}");
    assert!(decay > 5.0 && decay < w30, "decay-heat flow {decay}");
    assert!(w30 > 0.0 && w60 >= 0.95 * w30 && w100 >= 0.95 * w60, "{w30} {w60} {w100}");
    assert!((w100 - CORE_FLOW_RATED).abs() < 50.0);
}

#[test]
fn draining_breaks_natural_circulation_in_order() {
    let t0 = sat_p(P_RATED).t;
    let mut v = Vessel::new(t0, Z_NWL, 20.0e6, 0.0, 215.0, false);
    let w0 = v.core_flow;
    let mut time = 0.0;
    let mut last_sample = (0.0, v.core_flow);
    let mut below_chimney_top_flow = None;
    while v.last_error.is_none() && time < 3.0 * 3600.0 {
        v.step(DT, &inputs(20.0e6, 20.0e6, 50.0));
        time += DT;
        if v.level_z >= Z_CHIMNEY_TOP {
            assert!((v.core_flow - w0).abs() < 0.05 * w0, "flow changed above chimney top: {}", v.core_flow);
        } else {
            if below_chimney_top_flow.is_none() {
                below_chimney_top_flow = Some(v.core_flow);
                last_sample = (time, v.core_flow);
            }
            if time - last_sample.0 >= 10.0 {
                assert!(v.core_flow <= last_sample.1 * 1.01, "flow rose while draining at t={time}");
                last_sample = (time, v.core_flow);
            }
        }
        if v.level_z >= Z_TAF {
            assert_eq!(v.core_coverage(), 1.0, "coverage dropped with level above TAF");
        } else {
            assert!(v.core_coverage() < 1.0);
        }
        if v.level_z < Z_CORE_BOTTOM + 0.5 {
            assert!(v.core_flow < 20.0, "loop still flowing with level in the lower plenum: {}", v.core_flow);
        }
    }
    assert!(matches!(v.last_error, Some(DomainError::InventoryExhausted { .. })), "{:?}", v.last_error);
    assert!(v.collapsed_level_z < Z_CORE_BOTTOM, "exhausted at collapsed level {}", v.collapsed_level_z);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --release -p bwr-core --test vessel_transients`
Expected: compile error `no method named friction_dp`; after stubbing, `draining_breaks_natural_circulation_in_order` fails at the chimney-top spill switch (flow collapses within 0.5 m of the chimney top instead of fading toward TAF).

- [ ] **Step 3: Implement**

In `vessel.rs` add:

```rust
/// Height above the top of active fuel over which the separated-liquid return path,
/// and with it the loop head, fades out, m. ASSUMED (A-P4-6).
pub const LOOP_FADE_M: f64 = 0.5;
```

Replace `drive`:

```rust
    /// Natural circulation driving head (Pa) and two-phase friction multiplier,
    /// from the submerged column heights (docs/model/vessel.md section 6).
    fn drive(&self, s: &Sat) -> (f64, f64) {
        let (a_ch_qs, rho_ch_qs) = chimney_th(s, self.core_flow, &self.core);
        let dr = s.rho_g - s.rho_f;
        // Quasi-static densities corrected for the lagged void fractions.
        let rho_core = self.core.rho_avg + (self.alpha_core - self.core.alpha_avg) * dr;
        let rho_ch = rho_ch_qs + (self.alpha_ch - a_ch_qs) * dr;
        let t_dc = steam::t_from_h_liquid(self.h_in.min(s.h_f));
        let rho_dc = compressed_liquid_rho(t_dc, self.eos.p);
        let z = self.level_z;
        let h_dc = (z.min(Z_CHIMNEY_TOP) - Z_CORE_BOTTOM).max(0.0);
        let h_core = (z - Z_CORE_BOTTOM).clamp(0.0, CORE_HEIGHT);
        let h_ch = (z.min(Z_CHIMNEY_TOP) - Z_TAF).clamp(0.0, CHIMNEY_HEIGHT);
        let dp = G * (rho_dc * h_dc - rho_core * h_core - rho_ch * h_ch);
        let loop_closed = ((z - Z_TAF) / LOOP_FADE_M).clamp(0.0, 1.0);
        (dp * loop_closed, phi2(s, self.core.x_exit))
    }

    /// Friction pressure drop at the current flow, Pa (for momentum-balance checks).
    pub fn friction_dp(&self) -> f64 {
        let s = self.sat_for_core();
        self.k_fric * phi2(&s, self.core.x_exit) * self.core_flow * self.core_flow
    }
```

In `calibrate_friction` change `let rho_dc = steam::rho_liquid(steam::t_from_h_liquid(h_in));` to `let rho_dc = compressed_liquid_rho(steam::t_from_h_liquid(h_in), P_RATED);` so the calibration uses the same density model as `drive`.

In `step`, replace `let tau_dc = (V_DOWNCOMER * 750.0 / w).clamp(2.0, 600.0);` with:

```rust
        let rho_dc = compressed_liquid_rho(steam::t_from_h_liquid(self.h_in.min(s.h_f)), self.eos.p);
        let tau_dc = (V_DOWNCOMER * rho_dc / w).clamp(2.0, 600.0);
```

(`Vessel::new` already stores `v.drive_dp = v.drive(&s).0;` from Task 3.)

- [ ] **Step 4: Run the tests**

Run: `cargo test --release -p bwr-core --test vessel_transients -- --nocapture`
Expected: all pass; the printed flow line is copied into `docs/model/vessel.md` section 6 in Task 8.
Run: `cargo test --release -p bwr-core`
Expected: all pass. `rated_flow_and_voids_are_plausible` (flow within 50 kg/s of rated) must still pass because calibration and drive use the same density.

- [ ] **Step 5: Parameter rows**

Append to `docs/model/parameters.csv` (locked header; `assumption_id` values are the ids assigned in Task 8 Step 2, written as `A-P4-n` until then):

```csv
LOOP_FADE_M,0.5,m,height above TAF over which the natural-circulation loop head fades to zero,level in the core region,none,assumed,,0.2,1.0,A-P4-6,crates/bwr-core/src/vessel.rs,no public BWRX value; sensitivity interval covers the chimney inlet geometry uncertainty
VENT_BOILOFF_KGS_PER_K,0.02,kg/s/K,extra head-vent steam flow per degC above 100 with the vent open,vented hot vessel,none,assumed,,0.0,0.05,A-P4-2,crates/bwr-core/src/vessel.rs,was an inline literal in the baseline step(); if Package 1 registered it under another id rename that row
R_AIR,287.0,J/kg/K,specific gas constant of air (head noncondensable),,physical constant,confirmed,,,,A-P4-1,crates/bwr-core/src/vessel.rs,
WATER_SOLID_GAS_TOLERANCE_KG,1.0,kg,gas mass treated as dissolved when the vessel goes water-solid,water-solid,none,assumed,,0.1,10.0,A-P4-1,crates/bwr-core/src/vessel.rs,
REGIME_HYSTERESIS,1.05,-,p_gas/p_steam ratio band for the Saturated/GasBlanketed display label,vent closed,none,assumed,,1.01,1.2,A-P4-1,crates/bwr-core/src/vessel.rs,display and alarm only; no physics reads the label
```

Update the existing `C_HEAD_VENT` row (Package 1 inventoried the private constant): set `notes` to "also the air-ingress coefficient below atmospheric and the liquid-relief coefficient when water-solid (A-P4-2)" and `interval_lo,interval_hi` to `1e-7,1e-5`. Delete the row Package 1 created for the `tau_dc` literal `750.0` (downcomer density): the literal is replaced by `compressed_liquid_rho`, a computed property, not a constant. Note the deletion in the commit body.

Run: `cargo test --release -p bwr-core --test register`
Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add crates/bwr-core/src/vessel.rs crates/bwr-core/tests/vessel_transients.rs docs/model/parameters.csv
git commit -m "feat(vessel): level-dependent natural-circulation head with loop-loss fade

Replaces the 0.5 m spill switch below the chimney top with submerged column
heights; flow at 30/60/100 % and decay heat is tested, not calibrated.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 6: Moderator temperature from the core liquid profile

**Files:**
- Modify: `crates/bwr-core/src/vessel.rs` (`moderator_t`)
- Modify: `crates/bwr-core/src/sim.rs` (`base`, `set_source_equilibrium`, `full_power`, `step`)
- Modify: `crates/bwr-core/tests/vessel_transients.rs`

**Interfaces:**
- Produces: `Vessel::moderator_t(&self) -> f64` (degC).

- [ ] **Step 1: Write the failing test**

Append to `tests/vessel_transients.rs`:

```rust
#[test]
fn moderator_temperature_between_inlet_and_saturation() {
    let sim = Sim::new(ScenarioName::FullPower);
    let v = &sim.vessel;
    let s = v.sat();
    let t_in = steam::t_from_h_liquid(v.h_in);
    let m = v.moderator_t();
    assert!(m > t_in && m < s.t, "inlet {t_in} < moderator {m} < sat {}", s.t);
    assert!(s.t - m < 5.0, "moderator should be within a few degC of saturation at full power: {m}");
    // Cold, non-boiling core: the moderator is at the liquid temperature, not 100 degC.
    let cold = Vessel::new(40.0, Z_NWL, 0.3e6, 0.0, 40.0, true);
    let mc = cold.moderator_t();
    assert!(mc >= 40.0 && mc < 45.0, "cold moderator {mc}");
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --release -p bwr-core --test vessel_transients moderator`
Expected: compile error `no method named moderator_t`.

- [ ] **Step 3: Implement**

Add to `impl Vessel`:

```rust
    /// Average moderator temperature seen by the core, degC: a linear rise from the
    /// inlet liquid temperature over the non-boiling length, then saturation over the
    /// boiling length (docs/model/vessel.md section 7).
    pub fn moderator_t(&self) -> f64 {
        let s = self.sat_for_core();
        let t_in = steam::t_from_h_liquid(self.h_in.min(s.h_f));
        let bb = self.core.boiling_boundary.clamp(0.0, CORE_HEIGHT);
        let t_top = if bb < CORE_HEIGHT { s.t } else { steam::t_from_h_liquid(self.core.h_exit.min(s.h_f)) };
        (bb * 0.5 * (t_in + t_top) + (CORE_HEIGHT - bb) * s.t) / CORE_HEIGHT
    }
```

In `sim.rs`, wherever the moderator-temperature argument handed to the core reactivity update is currently the bulk vessel temperature (`self.vessel.eos.t`, the local `t`, or `t_cool`), pass `self.vessel.moderator_t()` (or `s.vessel.moderator_t()` inside constructors) instead. Four sites: `base`, `set_source_equilibrium`, the `full_power` critical-rod search loop, and `step` (master step order item 3). Change nothing else about the call or about `Core` internals: Package 5 replaces `Core::feedback` with `ReactivityCoeffs` and must find only the argument changed here.

Also in `crates/bwr-core/src/validation.rs` `run_case` sampling, set the trace column `moderator_t_c` from `sim.vessel.moderator_t()` (it was emitted as 0.0 by Package 2) and update the "populated from Package 4" note in `docs/validation/method.md`.

- [ ] **Step 4: Run the tests and record the shift**

Run: `cargo test --release -p bwr-core`
Expected: all pass. Then run:

```bash
cargo run --release -p bwr-core --example trace -- full 60 | head -5
```

and compare the `rho` and rod-average columns with the same command on the previous commit (`git stash` is not needed: run `git show HEAD~1:crates/bwr-core/src/sim.rs > /dev/null` only to confirm the prior commit exists, then `git checkout HEAD~1 -- . && cargo run ... && git checkout HEAD -- .`). Paste both first lines into the commit body. No coefficient is changed; the full-power critical rod search absorbs the ~1 degC moderator shift.

- [ ] **Step 5: Commit**

```bash
git add crates/bwr-core/src/vessel.rs crates/bwr-core/src/sim.rs crates/bwr-core/src/validation.rs crates/bwr-core/tests/vessel_transients.rs docs/validation/method.md
git commit -m "feat(vessel): moderator temperature from the core liquid profile

Before/after full-power rod position and rho (trace example, t=0):
<paste>

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 7: Whole-plant shrink/swell, conservation, and baseline regeneration

**Files:**
- Modify: `crates/bwr-core/tests/vessel_transients.rs`
- Modify: `docs/validation/baseline/*.csv`, `docs/validation/method.md`

**Interfaces:**
- Consumes: `validation::{run_case, standard_cases, Tolerance, compare}`, case ids `fp_steady_600`, `hs_steady_600`, `cs_steady_600`.

- [ ] **Step 1: Write the tests**

Append to `tests/vessel_transients.rs`:

```rust
use bwr_core::validation::{run_case, standard_cases};

#[test]
fn pressure_step_shrinks_then_recovers() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 30.0);
    let l0 = sim.vessel.level_nr_cm();
    let sp = sim.steam.p_setpoint / 1e6 + 0.2;
    sim.apply(Command::SetPressureSetpoint { mpa: sp });
    let mut min_early = f64::MAX;
    for _ in 0..(10.0 / DT) as usize {
        sim.step(DT);
        min_early = min_early.min(sim.vessel.level_nr_cm());
    }
    assert!(min_early < l0 - 2.0, "no shrink: min {min_early} vs {l0}");
    run(&mut sim, 110.0);
    let sp_cm = sim.fw.level_sp_cm;
    assert!((sim.vessel.level_nr_cm() - sp_cm).abs() < 5.0, "level did not recover: {}", sim.vessel.level_nr_cm());
    assert!(!sim.scrammed, "{:?}", sim.scram_reason);
}

#[test]
fn pressure_step_down_swells() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 30.0);
    let l0 = sim.vessel.level_nr_cm();
    let sp = sim.steam.p_setpoint / 1e6 - 0.2;
    sim.apply(Command::SetPressureSetpoint { mpa: sp });
    let mut max_early = f64::MIN;
    for _ in 0..(10.0 / DT) as usize {
        sim.step(DT);
        max_early = max_early.max(sim.vessel.level_nr_cm());
    }
    assert!(max_early > l0 + 2.0, "no swell: max {max_early} vs {l0}");
}

#[test]
fn steady_cases_conserve_mass_and_energy() {
    // Residuals come from the Sim ledger (Ledger::mass_residual / energy_residual),
    // re-based by finalize_init at t = 0; no hand-rolled sums.
    for scenario in [ScenarioName::FullPower, ScenarioName::HotStandby, ScenarioName::ColdShutdown] {
        let mut sim = Sim::new(scenario);
        assert!(sim.domain_errors.is_empty(), "{scenario:?}: invalid initial condition {:?}", sim.domain_errors);
        let mass0 = sim.vessel.mass;
        let energy0 = sim.vessel.energy.abs();
        run(&mut sim, 600.0);
        let rm = sim.ledger.mass_residual(sim.vessel.mass);
        let re = sim.ledger.energy_residual(sim.vessel.energy);
        assert!(rm.abs() < 1e-6 * mass0, "{scenario:?}: mass residual {rm} kg");
        assert!(re.abs() < 1e-6 * energy0, "{scenario:?}: energy residual {re} J");
        assert!(sim.domain_errors.is_empty(), "{scenario:?}: {:?}", sim.domain_errors);
    }
    // The trace runner reports the same ledger residuals and the Package 4 columns.
    for id in ["fp_steady_600", "hs_steady_600", "cs_steady_600"] {
        let case = standard_cases().into_iter().find(|c| c.id == id).unwrap_or_else(|| panic!("missing case {id}"));
        let tr = run_case(&case, DT);
        let m = tr.metrics();
        let mass = tr.column("mass_kg")[0];
        assert!(m.max_abs_mass_residual_kg < 1e-6 * mass, "{id}: trace mass residual {}", m.max_abs_mass_residual_kg);
        assert!(!m.nonfinite, "{id}: nonfinite trace");
        let gas = tr.column("gas_mass_kg");
        let tc = tr.column("t_coolant_c");
        let tm = tr.column("moderator_t_c");
        assert!(tc.iter().all(|&v| v > 0.0), "{id}: t_coolant_c not populated");
        assert!(tm.iter().all(|&v| v > 0.0), "{id}: moderator_t_c not populated");
        if id == "cs_steady_600" {
            assert!(gas[0] > 50.0, "{id}: cold vented vessel should carry air, gas_mass_kg = {}", gas[0]);
        } else {
            assert_eq!(gas[0], 0.0, "{id}: hot vessel should carry no gas");
        }
    }
}
```

- [ ] **Step 2: Run**

Run: `cargo test --release -p bwr-core --test vessel_transients`
Expected: all pass. If `pressure_step_shrinks_then_recovers` fails only on the magnitude thresholds (2 cm / 5 cm), print `l0`, `min_early`, and the 120 s level, record the measured values in `docs/model/vessel.md` section 8, and adjust the threshold to half the measured excursion with that justification in the commit body. A failure where level moves the wrong way is a defect: stop and investigate the void/level coupling, do not loosen the test.

- [ ] **Step 3: Compare against the Package 2 baselines, then regenerate**

Run: `cargo run --release -p bwr-core --example validation` (compare mode as defined in the Package 2 plan).
Expected: violations listed for cases that drain level below the chimney top or involve vented/cold conditions; steady full-power cases within the numerical tolerance class. Copy the full violation list into a scratch file.
Run: the Package 2 write/update mode of the same example (e.g. `cargo run --release -p bwr-core --example validation -- --write`; use the flag the Package 2 plan defined).
Append to `docs/validation/method.md` a "Baseline change log" entry: date, commit subject, the pasted violation list, and the one-line reason per case ("level-dependent loop head", "gas-blanketed pressure", "compressed-liquid level"). Baselines are regression evidence, not physical truth.

- [ ] **Step 4: Commit**

```bash
git add crates/bwr-core/tests/vessel_transients.rs docs/validation/baseline docs/validation/method.md
git commit -m "test(vessel): shrink/swell, conservation; regenerate baselines after Package 4 physics

<paste violation list>

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 8: Evidence register, applicability, and measured values

**Files:**
- Modify: `docs/model/vessel.md`, `docs/model/assumptions.md`, `docs/model/esbwr-applicability.csv`, `docs/model/parameters.csv`, `docs/model/coverage.md`, `README.md`

- [ ] **Step 1: Fill the measured table in `docs/model/vessel.md`**

Run: `cargo test --release -p bwr-core --test vessel_transients flow_is_positive -- --nocapture` and copy the printed `core flow kg/s` line into section 6's table (core and chimney voids from `Vessel::new(...).alpha_core/alpha_ch` at the same points; add two `println!` lines to the test temporarily or compute in a one-off `examples/trace` run, then remove the prints). Replace every "(measured in Task 8)".

- [ ] **Step 2: Assumptions**

Append to `docs/model/assumptions.md`, assigning the next sequential ids and replacing `A-P4-n` in `vessel.rs` comments and `vessel.md` with the assigned ids:

- A-P4-1: Noncondensable gas in the head is air with `R_AIR = 287 J/kg/K`, ideal gas, at the water temperature.
- A-P4-2: Head-vent flow coefficient `C_HEAD_VENT` applies to steam, gas, air ingress and liquid relief alike; `VENT_BOILOFF_KGS_PER_K` is retained from the baseline. Sensitivity: cold-shutdown pressure is pinned to `P_ATM` for any coefficient above 1e-7 kg/s/Pa; water-solid heating pressure is not controllable by the vent at any plausible coefficient.
- A-P4-3: Gas internal energy and pressure work on compressed liquid are neglected in the energy balance (below 0.1 % of vessel energy).
- A-P4-4: Steam-line and IC steam supply are not credited while `GasBlanketed`; gas transport through steam lines and IC tube gas accumulation are not modelled here (Package 7 treats IC degradation as a sensitivity).
- A-P4-5: Core boiling is evaluated at dome total pressure; the downcomer hydrostatic head (about 0.15 MPa at rated level) is neglected, under-predicting core inlet saturation temperature by about 1 degC at 7.2 MPa.
- A-P4-6: Below `Z_TAF + LOOP_FADE_M` the separated-liquid return path is lost and the loop head fades linearly to zero; residual core flow is `W_MIN = 5 kg/s`. No public BWRX value supports either number.
- A-P4-7: Compressed-liquid enthalpy is approximated by the saturated-liquid enthalpy at the same temperature (error below 0.5 % at 7 MPa).
- A-P4-8: Air in-leakage into a sealed sub-atmospheric vessel is not modelled.

- [ ] **Step 3: ESBWR applicability rows**

Append to `docs/model/esbwr-applicability.csv` (locked header `id,esbwr_source,esbwr_section,target_subsystem,bwrx_evidence,differences,adaptation,applicability_limits,test,classification`; `esbwr_source` values are `sources.csv` ids, e.g. `SRC-ESBWR-DCD-R10` as named by Package 1):

```csv
ESBWR-P4-1,SRC-ESBWR-DCD-R10,Tier 2 Sec. 4.4 thermal-hydraulic design: natural-circulation head/loss balance and chimney,vessel/circulation,BWRX-300 General Description 005N9751: natural circulation with tall chimney; RPV ~4 m x 26-27 m,BWRX chimney height; areas and rated flow are ASSUMED; no ESBWR numerical value transferred,hydrostatic head formula only; geometry from parameters.csv,single level for shroud and downcomer; no regional or density-wave stability,head_formula_matches_baseline_above_chimney_top; flow_is_positive_and_rises_with_power,shared concept
ESBWR-P4-2,SRC-ESBWR-DCD-R10,Tier 2 Sec. 5.3/5.4 reactor vessel: head vent and water-solid operation principles,vessel,005N9751 RPV description,BWRX integral RPV isolation valves and no SRVs; vent capacity unknown,vent as a mass boundary with an ASSUMED coefficient,water-solid pressure cannot be held by the vent at any plausible coefficient,cold_vented_vessel_is_atmospheric; water_solid_pressure_rises_steeply_and_leaves_envelope,adapted model
ESBWR-P4-3,SRC-ESBWR-DCD-R10,Tier 2 Sec. 4.4 drift-flux void correlation,core and chimney voids,none BWRX-specific,GNF2 bundle geometry assumed; C0 = 1.13 retained from the baseline,none,validity demonstrated only by plausibility above 1 MPa,rated_flow_and_voids_are_plausible,provisional numerical estimate
```

- [ ] **Step 4: Coverage and README**

In `docs/model/coverage.md` vessel row, list the sixteen tests from `vessel.md` section 8 and mark "natural circulation below chimney top" and "gas-blanketed heatup" as plausibility-verified, not validated. In `README.md` "Vessel" row replace "Mass and energy balance with real IAPWS-IF97 steam tables, so pressure, flashing, swell and shrink are physical" with "Mass, energy and noncondensable-gas balance with IAPWS-IF97 properties including liquid compressibility; vented, gas-blanketed, saturated and water-solid regimes" and add "flow stops if level falls below the chimney top" → "loop head falls with level and is lost near the top of the fuel".

- [ ] **Step 5: Final verification and commit**

Run: `cargo test --release -p bwr-core && npm --prefix ui run typecheck && npm --prefix ui run build`
Expected: all green.

```bash
git add docs/model README.md
git commit -m "docs(model): vessel assumptions, ESBWR applicability and measured circulation points

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

## Acceptance gate (verbatim from the spec)

"Pressure and level respond consistently to mass/energy changes and boundary conditions. Natural circulation works across tested conditions, not solely the calibration point. Density-wave or regional stability claims remain limited to evidence actually demonstrated."

Evidence: `tests/vessel_transients.rs` (15 tests), `steam::tests::reference_points_iapws`, `vessel::tests::eos_roundtrip_all_regimes`, `vessel::tests::state_energy_is_monotonic_in_temperature`, `docs/model/vessel.md` sections 1–8, regenerated baselines with change log.

## Self-review (master r2 checklist)

- [x] **Spec coverage:** envelope (Task 1, 3); "below 100 C is atmospheric" replaced with vent/gas/compressibility (Task 3, 4); compressed-water properties with independent reference tests and out-of-domain behaviour (Task 2, 3); one geometry/inventory model (unchanged `liquid_volume_below`, now with compressed density, Task 3); chimney spillover cutoff replaced by hydrostatic head across level regimes, not calibrated to one point (Task 5); core inlet mixing/transport-delay density and moderator temperature with compatible states (Task 5, 6); shrink/swell, heatup, cooldown, pressure step, low level, steady points (Task 4, 5, 7).
- [x] **No placeholders:** every code step shows code; the only fill-ins are measured values (`<k20>`, `<paste>`, "(measured in Task 8)") that the executor produces by running the named command.
- [x] **Master r2 names:** `Regime`, `Eos { t, p, x, regime }` plus the additional `p_steam`/`p_gas` fields, `R_AIR`, `solve_eos(mass, energy, gas_mass, vent_open) -> Result<Eos, DomainError>`, `Vessel.gas_mass`, `Vessel::moderator_t`, `Sat.kappa_f`, `compressed_liquid_rho`, `T_TABLE_MIN/MAX`, `T_EOS_MIN/MAX` (reused from P3, not redeclared), `Vessel.exhausted`, `Vessel.last_error`, `BoundaryFlows` with `w_ic_steam`/`w_ic_return`/`h_ic_return`, `Ledger::mass_residual`/`energy_residual`, trace columns `gas_mass_kg`/`t_coolant_c`/`moderator_t_c`, `Sim::finalize_init`.
- [x] **Derives rule:** `Regime` derives `Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize` + `camelCase`; `Eos` keeps `Clone, Copy, Debug` (Task 3 Step 3 shows both).
- [x] **parameters.csv rows with status:** `KAPPA_F_TABLE`, `T_TABLE_MIN`, `T_TABLE_MAX` (Task 2 Step 7); `LOOP_FADE_M`, `VENT_BOILOFF_KGS_PER_K`, `R_AIR`, `WATER_SOLID_GAS_TOLERANCE_KG`, `C_HEAD_VENT` update, `tau_dc` literal row deleted (Task 5 Step 5); `tests/register.rs` run after each.
- [x] **Snapshot change:** `vessel.regime` has `api.rs`, `types.ts`, `mock.ts` steps and a typecheck run (Task 3 Step 6–7).
- [x] **Failing test first:** Tasks 2–6 each start with a failing test; Task 7 adds whole-plant tests on top of the implemented behaviour and compares baselines before regenerating.
- [x] **Applied transfers:** `Vessel::step` returns applied flows after bounding all outflows (steam, vent, RWCU; IC steam added by P7 at the marked line) and rolls back on `solve_eos` Err; `Sim::step` step 9 scaling is untouched.
- [x] **Codex cycle-2 audit (regime transitions, vent accounting, P3 semantics):** no physics reads the regime label (outflows split by steam-space composition with a steam-space cap; `steam_available` reads `p_steam` and the water-solid condition only); label hysteresis `REGIME_HYSTERESIS` applied in `Vessel::step`, `solve_eos` stays pure; `state_at` monotonicity tested across the water-solid branch; gas-blanketed vessels cannot reach water-solid and leave the envelope explicitly (documented, tested); no liquid ever leaves through vent or steam lines (so `BoundaryFlows.w_liquid_out` remains RWCU-only and the Sim RWCU allocation ratio stays valid); `de` is term-identical to the ledger products and vent steam is in `w_vent` at `h_steam`; gas egress through lines and vent is capped at `gas_mass/dt`, air ingress carries no energy; ledger closure asserted in `vented_heatup_purges_air_and_closes_ledger`.
- Names introduced beyond the master contract: `Eos.p_steam`, `Eos.p_gas`, `Vessel::friction_dp`, `Vessel::steam_available`, `VENT_BOILOFF_KGS_PER_K`, `LOOP_FADE_M`, `WATER_SOLID_GAS_TOLERANCE_KG`, `REGIME_HYSTERESIS`, `vessel::state_at` (private), alarm id `gas_blanket`, register ids `KAPPA_F_TABLE`, `ESBWR-P4-1..3`.
- Accepted deviations (coordinator, r2): `sat_for_core` uses total pressure (the core boils against total local pressure; using steam partial pressure would make a cold air-filled vessel boil at 40 degC); `moderator_t` uses the exit liquid temperature, not saturation, as the top of the rise when the core is non-boiling (otherwise a cold core would report ~70 degC).
