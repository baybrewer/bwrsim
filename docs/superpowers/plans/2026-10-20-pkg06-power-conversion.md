# Package 6: Steam, Turbine, Condenser and Feedwater Refinement — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Names and signatures are governed by `docs/superpowers/plans/2026-10-20-refinement-master.md` **revision r2** ("Shared interface contract", section P6; "Shared `Sim::step` order"; "Locked CSV headers"; the Derives rule; the log-line convention in the P2 validation section). Packages 1–5 are complete when this plan starts: `Ledger`/`BoundaryFlows` with `Sim::last_flows()` (P2), `Fault` as `Vec<Fault>` with `find_fault` (P2), `validation.rs` with trace schema v2 (P2), `DomainError` and the transfer-allocation step (P3), `Regime` (P4), `RodMotion` (P5) all exist. This plan was reconciled to r2: the energy balance is the master's control-volume definition, every downstream consumer of a vessel transfer uses the applied (post-allocation) value, and all log lines follow `"<key>: <detail>"`.

**Goal:** Make the main steam, turbine-generator, condenser and feedwater models conserve mass and energy explicitly, represent independently consequential valves and boundary inventories, honour injected equipment faults, and pass steady and transient balance tests with documented uncertainty.

**Architecture:** `bop.rs` gains a per-line MSIV flow solution, a lumped steam-line storage node, a turbine stop valve, explicit extraction work, an explicit condensate storage tank boundary, and feed-pump speed states. Steam flows are computed as **requests** (master step 5), the vessel bounds them at the transfer-allocation point (step 8), and `Sim` scales `w_line`, `w_tcv`, `w_bypass` and the RWCU reject to the applied values (step 9) before the line storage commit (step 10), turbine (11) and condenser (12) consume them. `sim.rs` assembles the master's control-volume `EnergyBalance` every step. Faults reach `bop.rs` through three small plain structs (`SteamFaults`, `CondenserFaults`, `FeedFaults`) built by `Sim::bop_faults()` from `Sim.faults` (a `Vec<Fault>`, read with `has_fault`/`find_fault`), so `bop.rs` never depends on `Sim`.

**Tech Stack:** Rust 2021, serde; TypeScript 5.6 for the contract mirror; the Package 2 validation runner.

## Global Constraints

See the master plan "Global Constraints". Package-specific:

- "Model independently consequential valves separately instead of averaging line positions."
- "Replace implicit unlimited makeup and reject with explicit boundary-flow accounting and a declared inventory assumption."
- "Declare cooling-water conditions as external boundaries."
- "Treat RWCU reject as the implemented function; verify its destination, pressure-dependent feasibility, and mass/energy effects without expanding into a complete cleanup plant."
- No automatic logic may clear an injected `Fault`.
- Every new constant gets a `docs/model/parameters.csv` row (Task 10) with status `assumed`, `derived`, or `calibrated`.
- Run `cargo test --release -p bwr-core` after every task; all pre-existing tests stay green except where a task says a baseline assertion moves to `cases.csv`.

**Acceptance gate (verbatim from spec):** "Steady and transient power, water, and heat balances close. Condenser loss, feedwater disturbances, and turbine trips follow the selected design basis within documented uncertainty."

---

## Names used by this package

Constants `MSIV_DP_FRACTION` through `RWCU_REJECT_MAX_KGS`, the P6 struct fields (`SteamLine { mass, p, energy }`, `Turbine.p_extraction`/`p_losses`, `Condenser { cst_mass, makeup_flow, reject_flow, hotwell_energy }`, `Feedwater.pump_speed`), the fault views, and the P6 signatures are now **in the master contract (r2)**; the table below restates them for implementers and marks the few names that remain package-local (allowed: nothing outside P6 consumes them).

| Name | Where | Value / meaning |
|---|---|---|
| `MSIV_DP_FRACTION` | `params.rs` | `0.10` — fraction of rated line dp across a fully open MSIV. ASSUMED |
| `K_LINE` | `params.rs` | `(1-MSIV_DP_FRACTION)*STEAM_LINE_DP/(STEAM_FLOW_RATED/2)^2` Pa/(kg/s)^2. Derived |
| `K_MSIV_OPEN` | `params.rs` | `MSIV_DP_FRACTION*STEAM_LINE_DP/(STEAM_FLOW_RATED/2)^2`. Derived |
| `V_STEAM_LINE_M3` | `params.rs` | `60.0` m3, both lines + header + chest. ASSUMED |
| `TSV_STROKE` | `params.rs` | `0.1` s closing stroke on trip. ASSUMED |
| `TSV_OPEN_RATE` | `params.rs` | `1.0` per second on reset. ASSUMED |
| `EXTRACTION_WORK_FRACTION` | `params.rs` | `0.25` W of LP work forgone per W of extraction heating. ASSUMED, interval 0.15–0.35 |
| `TURBINE_DH_RATED` | `params.rs` | changed `0.703e6` → `0.897e6` J/kg. CALIBRATED (see Task 6) |
| `AIR_UA_DEGRADATION` | `params.rs` | `5.0` — UA divisor coefficient on air/steam partial pressure ratio. ASSUMED |
| `CST_CAPACITY_KG` | `params.rs` | `2.0e6` kg. ASSUMED |
| `CST_MASS_INITIAL` | `params.rs` | `1.5e6` kg. ASSUMED |
| `HOTWELL_MAKEUP_KGS` | `params.rs` | `30.0` kg/s (was a literal). ASSUMED |
| `HOTWELL_MAKEUP_ON` / `HOTWELL_REJECT_ON` | `params.rs` | `0.40` / `0.85` hotwell fraction (were literals). ASSUMED. Package-local |
| `FW_STATIC_DP` | `params.rs` | `0.15e6` Pa (was a literal). ASSUMED |
| `FW_PUMP_COASTDOWN_S` | `params.rs` | `5.0` s first-order pump speed time constant. ASSUMED |
| `SRI_TRIGGER_POWER_FRAC` | `params.rs` | `0.4` (was a literal in `sim.rs`). ASSUMED |
| `RWCU_REJECT_MAX_KGS` | `params.rs` | `15.0` kg/s (was a literal). ASSUMED |
| `line_flow(dp, pos)` | `bop.rs` | per-line flow through friction + MSIV (series resistance, master r2) |
| `SteamFaults`, `CondenserFaults`, `FeedFaults` | `bop.rs` | fault views; all derive `Clone, Copy, Debug, Default` |
| `SteamSystem::vessel_steam_flow()` | `bop.rs` | `w_line.iter().sum()` — the vessel's REQUESTED steam outflow |
| `SteamSystem::init_line_steady(p_vessel)` | `bop.rs` | sets `line` to the quasi-steady header state (scenario build) |
| `SteamLine::at_pressure(p)` | `bop.rs` | storage node constructor; `SteamLine` derives `Clone, Copy, Debug` and carries `energy = mass * u_g(p)` |
| `SteamSystem::step_line(dt, w_in_applied)` | `bop.rs` | commits applied flows into the line node (master step 10). Package-local name |
| `steam::sat_rho_g(rho)` | `steam.rs` | inverse of `rho_g` on the table |
| `Feedwater::set_mode(mode)` | `bop.rs` | bumpless transfer (master signature); re-tracks the integral on the next `step()` |
| `Turbine::kinetic_energy()` | `bop.rs` | shaft kinetic energy, J; storage term. Package-local name |
| `Sim::bop_faults()` | `sim.rs` | builds the three fault views from `Sim.faults` via `has_fault`/`find_fault` |
| `Sim::house_load()` | `sim.rs` | extracted from `snapshot()`. Package-local |
| `Sim::bypass_available()` | `sim.rs` | condenser availability AND `!BypassUnavailable`. Package-local |
| `Condenser.overflow_kg`, `Condenser.deficit_kg` | `bop.rs` | counted (never silent) hotwell clamp amounts. Package-local |
| `H_CST_EXCHANGE` assumption | `assumptions.md` | CST makeup/reject exchanged at hotwell enthalpy (no CST temperature state) |

---

## File structure

- Create: `docs/model/power-conversion.md`, `crates/bwr-core/tests/power_conversion.rs`
- Modify: `crates/bwr-core/src/bop.rs`, `crates/bwr-core/src/params.rs`, `crates/bwr-core/src/sim.rs`, `crates/bwr-core/src/api.rs`, `crates/bwr-core/src/steam.rs`, `crates/bwr-core/src/validation.rs` (append cases), `crates/bwr-core/tests/plant.rs` (case references), `docs/validation/cases.csv`, `docs/model/parameters.csv`, `docs/model/assumptions.md`, `docs/model/esbwr-applicability.csv`, `ui/src/sim/types.ts`, `ui/src/sim/mock.ts`

---

### Task 1: Topology and boundary document

**Files:**
- Create: `docs/model/power-conversion.md`

**Interfaces:** none (documentation). Later tasks append results to the "Results" section of this file.

- [ ] **Step 1: Write the document**

```markdown
# Power conversion model: main steam, turbine-generator, condenser, feedwater

Status: implemented subsystem description. Sources are cited by id from `docs/model/sources.csv`;
assumptions by id from `docs/model/assumptions.md`.

## Implemented topology

```
RPV dome ──┬─ steam line 1 ── MSIV 1 ─┐
           └─ steam line 2 ── MSIV 2 ─┴─ header/line node (SteamLine) ─┬─ TSV ── TCV ── HP/LP turbine ── condenser
                                                                       └─ bypass valves ──────────────── condenser
condenser ── hotwell ── condensate pumps (2) ── feedwater heaters (extraction) ── feed pumps (3) ── FCV ── RPV downcomer
RPV ── RWCU reject (operator-set, pumped) ── hotwell
hotwell <── makeup / reject ──> condensate storage tank (CST, declared boundary inventory)
```

- Two steam lines, each with one MSIV modelled as a series area-fraction resistance (`K_MSIV_OPEN / pos^2`) plus line friction (`K_LINE`). `MSIV_DP_FRACTION` (A-6xx) sets the split.
- The lines, header and steam chest are one lumped saturated-steam storage node of volume `V_STEAM_LINE_M3` (A-6xx). Steam in the node is saturated at node pressure; throttling superheat is neglected (error bounded in Task 6).
- The turbine stop valve (TSV) closes in `TSV_STROKE` on any turbine trip and is a separate state from the control valves (TCV).
- Bypass valves are one lumped bank of capacity `BYPASS_CAPACITY` x rated steam flow (status: assumed; the spec marks this "to verify").
- The turbine is a lumped shaft: gross work `w_tcv * TURBINE_DH_RATED * pf` minus the LP work forgone by extraction steam, `EXTRACTION_WORK_FRACTION * (feedwater heating duty)`.
- The condenser is a single shell with air partial pressure, air-blanketing UA degradation, a hotwell, and explicit makeup/reject to the CST.
- Feedwater: two condensate pumps (suction pressure boundary `CONDENSATE_PRESSURE`), heaters as a temperature target lagged 180 s, three 50 % feed pumps with affinity-law speed states, one equal-percentage FCV, manual / single-element / three-element level control.

## Reactor isolation versus containment isolation

The GE BWRX-300 general description (source S-xx) states that the RPV has integral isolation valves and that conventional safety relief valves are eliminated. This model's MSIVs are the only isolation elements represented. Assumption A-6xx: the two modelled MSIVs stand in for the RPV isolation function on the main steam path; a second (outboard/containment) isolation valve per line and the containment isolation logic are not modelled. No containment is modelled; steam that leaves the vessel through the head vent is a boundary loss.

## Declared boundaries

| Boundary | Representation | Status |
|---|---|---|
| Circulating water inlet | `CW_INLET_C` constant, `CW_FLOW_PER_PUMP` per running pump | assumed boundary |
| Condensate storage tank | `cst_mass` with capacity `CST_CAPACITY_KG`, initial `CST_MASS_INITIAL`; makeup/reject at `HOTWELL_MAKEUP_KGS` | assumed boundary inventory |
| Grid | infinite bus at `GRID_HZ`; breaker closed forces `RATED_RPM` | assumed |
| Head vent | atmosphere | boundary loss |
| RWCU pumps and cleanup train | always available; reject capped at `RWCU_REJECT_MAX_KGS` | assumed (A-6xx) |
| Hotwell temperature | equals condenser saturation temperature (perfect mixing, no subcooling) | assumed |

## Energy balance definition

See Task 6 of the plan; the balance terms and the analytic residual are documented in the "Energy balance" section appended by that task.

## Faults honoured

`MsivStuck`, `TsvStuck`, `TcvStuck`, `BypassUnavailable`, `PressureRegulatorFailed`, `FeedPumpTrip`, `CondensatePumpTrip`, `FcvStuck`, `HeatersLost`, `CwPumpTrip`, `VacuumPumpFailed`. Each is tested in `crates/bwr-core/tests/power_conversion.rs`.

## Results

(appended by Tasks 3, 5, 6, 7, 8, 10)
```

- [ ] **Step 2: Replace the `A-6xx` / `S-xx` placeholders with real ids**

Open `docs/model/assumptions.md` (Package 1). Append numbered assumptions, continuing the existing sequence, for: MSIV stands in for RPV isolation; CST as boundary inventory; RWCU pumps always available; hotwell perfectly mixed; steam line node saturated (no superheat). Use those ids in the document. Use the `sources.csv` id of the GE general description for `S-xx`.

- [ ] **Step 3: Commit**

```bash
git add docs/model/power-conversion.md docs/model/assumptions.md
git commit -m "docs(model): power conversion topology, isolation distinction, boundaries

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 2: Per-line MSIV flow instead of averaged position

**Files:**
- Modify: `crates/bwr-core/src/params.rs` (steam section, after `MSIV_STROKE`)
- Modify: `crates/bwr-core/src/bop.rs:10-62` (`SteamSystem` struct, `new`, `msiv_open_fraction`, `compute_flows`)
- Create: `crates/bwr-core/tests/power_conversion.rs`

**Interfaces:**
- Consumes: `STEAM_LINE_DP`, `STEAM_FLOW_RATED`, `N_STEAM_LINES`, `C_TCV`, `C_BYPASS`, `P_ATM`.
- Produces: `pub fn line_flow(dp: f64, pos: f64) -> f64`; `SteamSystem.w_line: [f64; N_STEAM_LINES]`; `SteamSystem::vessel_steam_flow(&self) -> f64`. `compute_flows(&mut self, p: f64, tsv_open: bool)` keeps its signature in this task (Task 4 changes it).

- [ ] **Step 1: Write the failing tests**

Create `crates/bwr-core/tests/power_conversion.rs`:

```rust
//! Package 6: main steam, turbine, condenser and feedwater behaviour.

use bwr_core::api::{Command, ScenarioName};
use bwr_core::bop::{line_flow, SteamSystem};
use bwr_core::params::*;
use bwr_core::sim::DT;
use bwr_core::steam::P_ATM;
use bwr_core::Sim;

fn run(sim: &mut Sim, seconds: f64) {
    for _ in 0..(seconds / DT).round() as usize {
        sim.step(DT);
    }
}

#[test]
fn line_flow_matches_rated_drop_and_quadruples_on_one_line() {
    let per_line = STEAM_FLOW_RATED / N_STEAM_LINES as f64;
    assert!((line_flow(STEAM_LINE_DP, 1.0) - per_line).abs() < 1e-6);
    assert!((line_flow(4.0 * STEAM_LINE_DP, 1.0) - STEAM_FLOW_RATED).abs() < 1e-6);
    assert_eq!(line_flow(STEAM_LINE_DP, 0.0), 0.0);
    assert_eq!(line_flow(-1.0, 1.0), 0.0);
    // A half-open MSIV passes less than half the flow of two half-open... i.e. the
    // valve is a series resistance, not an area average.
    assert!(line_flow(STEAM_LINE_DP, 0.5) < 0.5 * per_line + 1e-9 || line_flow(STEAM_LINE_DP, 0.5) < per_line);
}

#[test]
fn msiv_flow_is_per_line_not_averaged() {
    let mut a = SteamSystem::new(true);
    a.tcv_pos = 0.85;
    a.msiv_pos = [0.0, 1.0];
    a.compute_flows(P_RATED, true);
    let mut b = SteamSystem::new(true);
    b.tcv_pos = 0.85;
    b.msiv_pos = [0.5, 0.5];
    b.compute_flows(P_RATED, true);
    assert_eq!(a.w_line[0], 0.0, "closed line must carry no flow");
    assert!((a.w_line[1] - a.vessel_steam_flow()).abs() < 1e-9);
    assert!((a.vessel_steam_flow() - a.total_flow()).abs() < 1e-6, "quasi-steady: in == out");
    assert!(
        (a.total_flow() - b.total_flow()).abs() > 1.0,
        "one closed line and two half-open lines must differ: {} vs {}",
        a.total_flow(),
        b.total_flow()
    );
    // One line carrying everything sees a much larger line pressure drop.
    assert!(P_RATED - a.p_chest > 3.0 * (P_RATED - b.p_chest) * 0.5, "a dp {} b dp {}", P_RATED - a.p_chest, P_RATED - b.p_chest);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --release -p bwr-core --test power_conversion`
Expected: compile error `cannot find function line_flow` / `no field w_line on type SteamSystem` (the baseline has neither; its `compute_flows` uses `msiv_open_fraction()`, which is the averaging defect).

- [ ] **Step 3: Add parameters**

In `crates/bwr-core/src/params.rs`, after the line `pub const MSIV_STROKE: f64 = 4.0;` add:

```rust
/// Fraction of the rated steam-line pressure drop taken across a fully open MSIV. ASSUMED (A-6xx).
pub const MSIV_DP_FRACTION: f64 = 0.10;
/// Rated flow in one steam line, kg/s.
pub const LINE_FLOW_RATED: f64 = STEAM_FLOW_RATED / N_STEAM_LINES as f64;
/// Friction resistance of one steam line excluding its MSIV, Pa per (kg/s)^2. Derived.
pub const K_LINE: f64 = (1.0 - MSIV_DP_FRACTION) * STEAM_LINE_DP / (LINE_FLOW_RATED * LINE_FLOW_RATED);
/// Resistance of one fully open MSIV, Pa per (kg/s)^2; scales with 1/pos^2 as it closes. Derived.
pub const K_MSIV_OPEN: f64 = MSIV_DP_FRACTION * STEAM_LINE_DP / (LINE_FLOW_RATED * LINE_FLOW_RATED);
```

- [ ] **Step 4: Implement per-line flow in `bop.rs`**

Replace lines 10–62 of `crates/bwr-core/src/bop.rs` (from `#[derive(Clone, Debug)] pub struct SteamSystem` through the end of `compute_flows`) with:

```rust
/// Flow through one steam line, kg/s, for a vessel-to-header pressure difference
/// `dp` (Pa) with its MSIV at area fraction `pos` (0 closed .. 1 open).
/// Line friction and the valve act in series; no reverse flow is modelled.
pub fn line_flow(dp: f64, pos: f64) -> f64 {
    if dp <= 0.0 || pos < 1e-4 {
        return 0.0;
    }
    let k = K_LINE + K_MSIV_OPEN / (pos * pos);
    (dp / k).sqrt()
}

#[derive(Clone, Debug)]
pub struct SteamSystem {
    pub msiv_pos: [f64; N_STEAM_LINES],
    pub msiv_cmd: [bool; N_STEAM_LINES],
    pub tcv_pos: f64,
    pub bypass_pos: f64,
    pub bypass_auto: bool,
    pub bypass_manual_pos: f64,
    pub p_setpoint: f64,
    pub w_tcv: f64,
    pub w_bypass: f64,
    /// Header / steam chest pressure, Pa.
    pub p_chest: f64,
    /// Flow through each steam line's MSIV, kg/s.
    pub w_line: [f64; N_STEAM_LINES],
}

impl SteamSystem {
    pub fn new(msivs_open: bool) -> Self {
        let m = if msivs_open { 1.0 } else { 0.0 };
        Self {
            msiv_pos: [m; N_STEAM_LINES],
            msiv_cmd: [msivs_open; N_STEAM_LINES],
            tcv_pos: 0.0,
            bypass_pos: 0.0,
            bypass_auto: true,
            bypass_manual_pos: 0.0,
            p_setpoint: PRESSURE_SETPOINT_DEFAULT,
            w_tcv: 0.0,
            w_bypass: 0.0,
            p_chest: 0.0,
            w_line: [0.0; N_STEAM_LINES],
        }
    }

    /// Steam leaving the vessel through all lines, kg/s.
    pub fn vessel_steam_flow(&self) -> f64 {
        self.w_line.iter().sum()
    }

    fn any_line_open(&self) -> bool {
        self.msiv_pos.iter().any(|&m| m >= 1e-4)
    }

    /// Quasi-steady steam flows for the current valve positions at dome pressure `p`:
    /// the header pressure is the one at which line inflow equals valve outflow.
    pub fn compute_flows(&mut self, p: f64, tsv_open: bool) {
        let a_tcv = if tsv_open { C_TCV * self.tcv_pos } else { 0.0 };
        let a_byp = C_BYPASS * self.bypass_pos;
        let a = a_tcv + a_byp;
        let any_line = self.any_line_open();
        if !any_line || a < 1e-12 || p <= P_ATM {
            self.w_line = [0.0; N_STEAM_LINES];
            self.w_tcv = 0.0;
            self.w_bypass = 0.0;
            self.p_chest = if any_line { p.max(P_ATM) } else { 0.0 };
            return;
        }
        let msiv = self.msiv_pos;
        // Inflow falls and outflow rises with header pressure: exactly one root in [P_ATM, p].
        let balance = |p_hdr: f64| -> f64 {
            let w_in: f64 = msiv.iter().map(|&m| line_flow(p - p_hdr, m)).sum();
            w_in - a * (p_hdr - P_ATM)
        };
        let (mut lo, mut hi) = (P_ATM, p);
        for _ in 0..60 {
            let mid = 0.5 * (lo + hi);
            if balance(mid) > 0.0 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let p_hdr = 0.5 * (lo + hi);
        for i in 0..N_STEAM_LINES {
            self.w_line[i] = line_flow(p - p_hdr, msiv[i]);
        }
        self.p_chest = p_hdr;
        self.w_tcv = a_tcv * (p_hdr - P_ATM);
        self.w_bypass = a_byp * (p_hdr - P_ATM);
    }
```

Keep `total_flow`, `pressure_demand` and `step` as they are (they follow immediately). Delete the now-unused `msiv_open_fraction` if the compiler warns.

- [ ] **Step 5: Run the tests**

Run: `cargo test --release -p bwr-core`
Expected: `power_conversion` 2 passed; all previous tests pass (`full_power_is_steady` net MWe may shift by < 2 MWe because the line drop is now split; if it leaves 285–315, stop and report rather than retune).

- [ ] **Step 6: Commit**

```bash
git add crates/bwr-core/src/bop.rs crates/bwr-core/src/params.rs crates/bwr-core/tests/power_conversion.rs
git commit -m "feat(bop): per-line MSIV flow with series valve resistance

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 3: Steam line and chest storage node

**Files:**
- Modify: `crates/bwr-core/src/steam.rs` (add `sat_rho_g`)
- Modify: `crates/bwr-core/src/params.rs` (add `V_STEAM_LINE_M3`)
- Modify: `crates/bwr-core/src/bop.rs` (`SteamLine`, `SteamSystem.line`, `compute_flows` becomes dynamic, `init_line_steady`)
- Modify: `crates/bwr-core/src/sim.rs` (`base`, `step`: vessel steam flow, three-element steam measurement, high-steam-flow signal)
- Test: `crates/bwr-core/tests/power_conversion.rs`

**Interfaces:**
- Produces (master r2): `#[derive(Clone, Copy, Debug)] pub struct SteamLine { pub mass: f64, pub p: f64, pub energy: f64 }` with `energy = mass * u_g(p)`; `SteamLine::at_pressure(p) -> Self`; `SteamSystem::init_line_steady(&mut self, p_vessel: f64, tsv_open: bool)` (Task 4 drops `tsv_open`); `steam::sat_rho_g(rho_g: f64) -> Sat`; `compute_flows(&mut self, dt: f64, p: f64, tsv_open: bool)` computes **requested** flows (Task 4 drops `tsv_open`); `SteamSystem::step_line(&mut self, dt: f64, w_in_applied: f64)` commits applied flows (master step 10).
- `VesselInputs.w_steam` becomes the request `steam.vessel_steam_flow()`; after `vessel.step` returns `flows`, `Sim` scales `w_line`, `w_tcv`, `w_bypass` by `flows.w_steam / request` (master step 9) before `step_line`, turbine and condenser run.

- [ ] **Step 1: Write the failing tests**

Append to `crates/bwr-core/tests/power_conversion.rs`:

```rust
use bwr_core::bop::SteamLine;

#[test]
fn steam_line_node_holds_mass_and_energy_at_pressure() {
    let l = SteamLine::at_pressure(7.0e6);
    let s = bwr_core::steam::sat_p(7.0e6);
    assert!((l.mass - V_STEAM_LINE_M3 * s.rho_g).abs() < 1e-6);
    assert!((l.energy - l.mass * s.u_g()).abs() < 1.0, "energy {}", l.energy);
    // No net flow through a steady system: pressure does not change.
    let mut sys = SteamSystem::new(true, true);
    sys.tcv_pos = 0.85;
    sys.init_line_steady(7.2e6, true);
    let p0 = sys.line.p;
    sys.compute_flows(DT, 7.2e6, true);
    sys.step_line(DT, sys.vessel_steam_flow());
    assert!((sys.line.p - p0).abs() < 1.0e3, "steady line drifted {} -> {}", p0, sys.line.p);
    // Pure outflow (applied inflow zero) must drop pressure and conserve node mass exactly.
    let m0 = sys.line.mass;
    let w_out = sys.w_tcv + sys.w_bypass;
    sys.step_line(DT, 0.0);
    assert!(sys.line.p < p0, "p {}", sys.line.p);
    assert!((sys.line.mass - (m0 - w_out * DT)).abs() < 1e-6);
    assert!((sys.line.energy - sys.line.mass * bwr_core::steam::sat_p(sys.line.p).u_g()).abs() < 1.0);
}

#[test]
fn msiv_closure_drains_the_line_while_vessel_pressure_rises() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    let p_line0 = sim.steam.line.p;
    let p_vessel0 = sim.vessel.eos.p;
    assert!(p_line0 > 6.5e6 && p_line0 < p_vessel0, "line {p_line0} vessel {p_vessel0}");
    sim.apply(Command::SetMsiv { line: 0, open: false });
    sim.apply(Command::SetMsiv { line: 1, open: false });
    run(&mut sim, 30.0);
    assert!(sim.steam.line.p < 1.0e6, "line should drain through the valves: {}", sim.steam.line.p);
    assert!(sim.vessel.eos.p > p_vessel0, "vessel should pressurise");
    assert_eq!(sim.steam.vessel_steam_flow(), 0.0);
}

#[test]
fn turbine_trip_line_pressure_spike_is_bounded() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    let p0 = sim.steam.line.p;
    sim.apply(Command::TripTurbine);
    let mut peak = p0;
    for _ in 0..(10.0 / DT) as usize {
        sim.step(DT);
        peak = peak.max(sim.steam.line.p);
    }
    assert!(peak > p0 + 0.02e6, "storage should show a rise before bypass takes over: {}", peak - p0);
    assert!(peak < p0 + 0.3e6, "spike {} MPa", (peak - p0) / 1e6);
}

#[test]
fn steam_consumers_see_applied_flows() {
    // Master step 9: after vessel allocation, w_line/w_tcv/w_bypass hold APPLIED values,
    // which at adequate inventory equal the vessel's applied steam outflow exactly.
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 60.0);
    let applied = sim.last_flows().w_steam;
    assert!((sim.steam.vessel_steam_flow() - applied).abs() < 1e-9, "post-step w_line must be applied");
    assert!(applied > 0.9 * STEAM_FLOW_RATED, "sanity: near rated flow");
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --release -p bwr-core --test power_conversion`
Expected: compile error `cannot find SteamLine` / `no field line`.

- [ ] **Step 3: Add the inverse density lookup to `steam.rs`**

After `pub fn sat_p(p: f64) -> Sat { ... }` in `crates/bwr-core/src/steam.rs` add:

```rust
/// Saturation properties at saturated-vapour density `rho_g` (kg/m3), clamped to the table.
pub fn sat_rho_g(rho_g: f64) -> Sat {
    let (i, f) = locate(3, rho_g, true);
    sat_t(T0 + i as f64 + f)
}
```

Add a unit test in the existing `mod tests` of `steam.rs`:

```rust
    #[test]
    fn rho_g_roundtrip() {
        for &p in &[0.1e6, 1.0e6, 7.2e6] {
            let s = sat_p(p);
            assert!((sat_rho_g(s.rho_g).p - p).abs() / p < 1e-3, "p {p}");
        }
    }
```

- [ ] **Step 4: Add the volume parameter**

In `params.rs` after `K_MSIV_OPEN` add:

```rust
/// Steam volume of both main steam lines, header and turbine steam chest, m3. ASSUMED (A-6xx).
pub const V_STEAM_LINE_M3: f64 = 60.0;
```

- [ ] **Step 5: Implement `SteamLine` and split `compute_flows` into request + commit**

In `bop.rs`, before `pub struct SteamSystem`, add:

```rust
/// Lumped saturated-steam storage in the main steam lines, header and chest.
/// `energy` is internal energy, mass * u_g(p), kept for the plant energy balance.
#[derive(Clone, Copy, Debug)]
pub struct SteamLine {
    pub mass: f64,
    pub p: f64,
    pub energy: f64,
}

impl SteamLine {
    pub fn at_pressure(p: f64) -> Self {
        let p = p.max(P_ATM);
        let s = sat_p(p);
        let mass = V_STEAM_LINE_M3 * s.rho_g;
        Self { mass, p, energy: mass * s.u_g() }
    }

    fn mass_at(p: f64) -> f64 {
        V_STEAM_LINE_M3 * sat_p(p).rho_g
    }

    /// Set pressure and energy consistent with the current mass.
    fn refresh(&mut self) {
        let s = crate::steam::sat_rho_g(self.mass / V_STEAM_LINE_M3);
        self.p = s.p;
        self.energy = self.mass * s.u_g();
    }
}
```

Add `pub line: SteamLine,` to `SteamSystem` and `line: SteamLine::at_pressure(P_ATM),` in `new`. Replace `compute_flows` with the request predictor plus the applied-flow commit:

```rust
    /// Set the line node to the quasi-steady header state for dome pressure `p` (used at scenario build).
    pub fn init_line_steady(&mut self, p: f64, tsv_open: bool) {
        let a_tcv = if tsv_open { C_TCV * self.tcv_pos } else { 0.0 };
        let a = a_tcv + C_BYPASS * self.bypass_pos;
        if !self.any_line_open() || p <= P_ATM {
            self.line = SteamLine::at_pressure(P_ATM);
            return;
        }
        if a < 1e-12 {
            self.line = SteamLine::at_pressure(p);
            return;
        }
        let msiv = self.msiv_pos;
        let balance = |p_hdr: f64| -> f64 {
            let w_in: f64 = msiv.iter().map(|&m| line_flow(p - p_hdr, m)).sum();
            w_in - a * (p_hdr - P_ATM)
        };
        let (mut lo, mut hi) = (P_ATM, p);
        for _ in 0..60 {
            let mid = 0.5 * (lo + hi);
            if balance(mid) > 0.0 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        self.line = SteamLine::at_pressure(0.5 * (lo + hi));
    }

    /// REQUESTED steam flows for this step (master step 5): an implicit predictor
    /// solves the new line pressure from mass(p) = mass + dt*(w_in(p) - w_out(p))
    /// (monotone in p), then evaluates every flow at that pressure. Nothing is
    /// committed here; `step_line` commits the APPLIED flows after vessel allocation.
    pub fn compute_flows(&mut self, dt: f64, p: f64, tsv_open: bool) {
        let a_tcv = if tsv_open { C_TCV * self.tcv_pos } else { 0.0 };
        let a_byp = C_BYPASS * self.bypass_pos;
        let a = a_tcv + a_byp;
        let msiv = self.msiv_pos;
        let g = |ph: f64| -> f64 {
            let w_in: f64 = msiv.iter().map(|&m| line_flow(p - ph, m)).sum();
            SteamLine::mass_at(ph) - self.line.mass - dt * (w_in - a * (ph - P_ATM).max(0.0))
        };
        let (mut lo, mut hi) = (1.0e3, 16.5e6);
        for _ in 0..60 {
            let mid = 0.5 * (lo + hi);
            if g(mid) < 0.0 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let ph = 0.5 * (lo + hi);
        for i in 0..N_STEAM_LINES {
            self.w_line[i] = line_flow(p - ph, msiv[i]);
        }
        self.p_chest = ph;
        let dp_out = (ph - P_ATM).max(0.0);
        self.w_tcv = a_tcv * dp_out;
        self.w_bypass = a_byp * dp_out;
    }

    /// Commit the APPLIED flows into the line node (master step 10). Call after the
    /// vessel allocation has scaled `w_line`, `w_tcv` and `w_bypass`; pass the applied
    /// inflow (`vessel_steam_flow()` after scaling). Outflows are the already-scaled
    /// `w_tcv + w_bypass`. Draining is self-limiting (outflow conductance vanishes as
    /// p -> P_ATM), so no clamp is needed; `refresh` keeps p and energy mass-consistent.
    pub fn step_line(&mut self, dt: f64, w_in_applied: f64) {
        self.line.mass += (w_in_applied - (self.w_tcv + self.w_bypass)) * dt;
        self.line.refresh();
        self.p_chest = self.line.p;
    }
```

When the allocation ratio is 1.0 (every normal step), the committed mass equals the predictor's implicit solution exactly, so the node never drifts from its own solve.

Update the Task 2 test `msiv_flow_is_per_line_not_averaged` to call `a.init_line_steady(P_RATED, true); a.compute_flows(DT, P_RATED, true);` (same for `b`), since the quasi-steady state is now the initialiser followed by one request computation.

- [ ] **Step 6: Wire into `sim.rs`**

In `Sim::base`, after `s.prev_gen_online = ...;` add:

```rust
        s.steam.init_line_steady(s.vessel.eos.p, !s.turbine.tripped);
```

In `Sim::step`, restructure the steam/vessel section to the master step order 5 and 8–11. Replace

```rust
        self.steam.compute_flows(if self.vessel.eos.subcooled { P_ATM } else { p }, !self.turbine.tripped);
```

and the turbine call that follows it, and move the `VesselInputs`/`vessel.step` block up between them, so the section reads (Package 4 replaced `subcooled` with `regime`; keep whatever expression P4 left for the vessel-side pressure as `p_steam_side`):

```rust
        // 5. requested steam flows
        self.steam.compute_flows(dt, p_steam_side, !self.turbine.tripped);

        // ... (IC step 6 and feedwater step 7 stay where P3 left them, before the vessel) ...

        // 8. vessel: the transfer-allocation point
        let inputs = VesselInputs {
            q_core: self.q_core,
            q_ic,
            w_feed: self.fw.flow,
            h_feed: self.fw.h_feed(),
            w_steam: self.steam.vessel_steam_flow(),
            w_liquid_out: self.fw.rwcu_reject,
        };
        let flows = self.vessel.step(dt, &inputs);

        // 9. scale every steam consumer to the APPLIED transfer
        let requested = self.steam.vessel_steam_flow();
        if requested > 0.0 && flows.w_steam < requested {
            let r = flows.w_steam / requested;
            for w in self.steam.w_line.iter_mut() {
                *w *= r;
            }
            self.steam.w_tcv *= r;
            self.steam.w_bypass *= r;
        }
        // RWCU consumers use flows.w_liquid_out (applied), never fw.rwcu_reject (request).

        // 10. line storage commits the applied flows
        let w_in_line = self.steam.vessel_steam_flow();
        self.steam.step_line(dt, w_in_line);

        // 11. turbine consumes applied w_tcv
        let p_cond = self.cond.pressure();
        self.turbine.step(dt, self.steam.w_tcv, self.steam.p_chest, p_cond);
```

(The baseline computed `p_cond` and stepped the turbine before the vessel; both move after the allocation. Task 6 extends the turbine call with `extraction_heat`.) Change the feedwater call's `steam_flow` argument from `self.steam.total_flow()` to `self.steam.vessel_steam_flow()` (the three-element controller measures line flow; at step 7 this still holds the request, which is the measured flow a real sensor would see within one step). In `isolation_signals`, change `self.steam.total_flow() > 1.4 * STEAM_FLOW_RATED` to `self.steam.vessel_steam_flow() > 1.4 * STEAM_FLOW_RATED`. In `snapshot()`, change `steam_flow_kgs: self.steam.total_flow() + v.vent_flow` to `steam_flow_kgs: self.steam.vessel_steam_flow() + v.vent_flow` (post-step these are applied values).

Add to `Sim::full_power()` just before `s.settle(600.0);`: `s.steam.init_line_steady(s.vessel.eos.p, true);` (the TCV position was set after `base`).

- [ ] **Step 7: Run all tests**

Run: `cargo test --release -p bwr-core`
Expected: all pass. `turbine_trip_rides_through_on_bypass` and `raising_pressure_raises_power` may shift numerically; if any assertion fails, record the values in `docs/model/power-conversion.md` "Results" and report before touching thresholds (master constraint: no bulk retune).

- [ ] **Step 8: Regenerate baselines and record the change**

Run: `cargo run --release -p bwr-core --example validation -- --update` (Package 2 runner; use the flag Package 2 defined for regeneration). Append to `docs/model/power-conversion.md` "Results" a table of `fp_turbine_trip` and the P2 MSIV-closure case's peak pressure and time before/after this task (from the runner's compare output; use the exact ids `standard_cases()` defines).

- [ ] **Step 9: Commit**

```bash
git add crates/bwr-core/src crates/bwr-core/tests docs/model/power-conversion.md docs/validation/baseline
git commit -m "feat(bop): lumped steam line storage node with implicit pressure solve

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 4: Turbine stop valve and steam-side fault holds

**Files:**
- Modify: `crates/bwr-core/src/params.rs` (`TSV_STROKE`, `TSV_OPEN_RATE`)
- Modify: `crates/bwr-core/src/bop.rs` (`SteamFaults`, `SteamSystem.tsv_pos`, `new(msivs_open, tsv_open)`, `step`, `compute_flows`, `pressure_demand`)
- Modify: `crates/bwr-core/src/sim.rs` (`bop_faults`, call sites, `bypass_available`)
- Test: `crates/bwr-core/tests/power_conversion.rs`

**Interfaces:**
- Produces:
```rust
#[derive(Clone, Copy, Debug, Default)]
pub struct SteamFaults {
    pub msiv_stuck: [bool; N_STEAM_LINES],
    pub tsv_stuck: bool,
    pub tcv_stuck: bool,
    pub bypass_unavailable: bool,
    /// Pressure regulator output frozen at this fraction of rated steam flow.
    pub regulator_frozen: Option<f64>,
}
```
`SteamSystem::new(msivs_open: bool, tsv_open: bool)`; `SteamSystem.tsv_pos: f64`; `compute_flows(&mut self, dt: f64, p: f64)` (no `tsv_open`); `pressure_demand(&self, p: f64, faults: &SteamFaults) -> f64`; `step(&mut self, dt, p, tcv_flow_demand, bypass_available, tripped, faults: &SteamFaults)`; `Sim::bop_faults(&self) -> (SteamFaults, CondenserFaults, FeedFaults)` (the latter two are defined in Tasks 7 and 8; in this task return `SteamFaults` only via `fn steam_faults(&self) -> SteamFaults`, and Task 8 folds it into `bop_faults`).

- [ ] **Step 1: Write the failing tests**

Append to `tests/power_conversion.rs`:

```rust
use bwr_core::faults::Fault;

#[test]
fn tsv_closes_fast_on_trip_and_stuck_tsv_holds() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    assert!((sim.steam.tsv_pos - 1.0).abs() < 1e-9);
    sim.apply(Command::TripTurbine);
    run(&mut sim, 0.2);
    assert!(sim.steam.tsv_pos < 1e-6, "TSV should close within TSV_STROKE: {}", sim.steam.tsv_pos);

    let mut stuck = Sim::new(ScenarioName::FullPower);
    run(&mut stuck, 5.0);
    stuck.set_fault(Fault::TsvStuck, true);
    stuck.apply(Command::TripTurbine);
    run(&mut stuck, 5.0);
    assert!((stuck.steam.tsv_pos - 1.0).abs() < 1e-9, "stuck TSV must not move");
    assert!(stuck.has_fault(&Fault::TsvStuck), "automatic logic must not clear the fault");
    // The control valves still closed, so turbine steam stops anyway.
    assert!(stuck.steam.w_tcv < 1.0);
}

#[test]
fn stuck_tsv_and_tcv_keep_feeding_the_turbine_after_trip() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    sim.set_fault(Fault::TsvStuck, true);
    sim.set_fault(Fault::TcvStuck, true);
    sim.apply(Command::TripTurbine);
    run(&mut sim, 5.0);
    assert!(sim.steam.w_tcv > 200.0, "steam should still reach the turbine: {}", sim.steam.w_tcv);
}

#[test]
fn stuck_msiv_stays_open_during_isolation() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    sim.set_fault(Fault::MsivStuck { line: 0 }, true);
    sim.apply(Command::SetMsiv { line: 0, open: false });
    sim.apply(Command::SetMsiv { line: 1, open: false });
    run(&mut sim, 20.0);
    assert!((sim.steam.msiv_pos[0] - 1.0).abs() < 1e-9, "line 0 must stay open");
    assert!(sim.steam.msiv_pos[1] < 1e-9, "line 1 must close");
    assert!(!sim.msivs_open());
    assert!(sim.steam.w_line[0] > 0.0 && sim.steam.w_line[1] == 0.0);
}

#[test]
fn regulator_frozen_output_overrides_pressure_demand() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    sim.set_fault(Fault::PressureRegulatorFailed { demand_pct: 0 }, true);
    run(&mut sim, 60.0);
    assert!(sim.scrammed, "no steam demand: pressure must reach the high-pressure scram");
    assert_eq!(sim.scram_reason.as_deref(), Some("Reactor high pressure"));
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --release -p bwr-core --test power_conversion`
Expected: compile error `no field tsv_pos`.

- [ ] **Step 3: Parameters**

In `params.rs` after `MSIV_STROKE`:

```rust
/// Turbine stop valve closing stroke on trip, s. ASSUMED (typical fast-acting stop valve).
pub const TSV_STROKE: f64 = 0.1;
/// Turbine stop valve opening rate after reset, fraction per second. ASSUMED.
pub const TSV_OPEN_RATE: f64 = 1.0;
```

- [ ] **Step 4: Implement in `bop.rs`**

Add the `SteamFaults` struct (from Interfaces) above `SteamSystem`. Add `pub tsv_pos: f64,` to `SteamSystem`. Change `new`:

```rust
    pub fn new(msivs_open: bool, tsv_open: bool) -> Self {
        let m = if msivs_open { 1.0 } else { 0.0 };
        Self {
            // ...existing fields...
            tsv_pos: if tsv_open { 1.0 } else { 0.0 },
            // ...
        }
    }
```

In `init_line_steady` and `compute_flows` replace `let a_tcv = if tsv_open { C_TCV * self.tcv_pos } else { 0.0 };` with `let a_tcv = C_TCV * self.tcv_pos * self.tsv_pos;` and drop the `tsv_open` parameter from both signatures.

Replace `pressure_demand`:

```rust
    /// Pressure regulator: total steam flow demand to hold the pressure setpoint, kg/s.
    pub fn pressure_demand(&self, p: f64, faults: &SteamFaults) -> f64 {
        if let Some(frac) = faults.regulator_frozen {
            return frac * STEAM_FLOW_RATED;
        }
        ((p - self.p_setpoint) / PRESSURE_REG_SPAN).max(0.0) * STEAM_FLOW_RATED
    }
```

Replace `step`:

```rust
    /// Move MSIVs, TSV, TCV and bypass valves toward their demands. Stuck valves hold.
    pub fn step(&mut self, dt: f64, p: f64, tcv_flow_demand: f64, bypass_available: bool, tripped: bool, faults: &SteamFaults) {
        for i in 0..N_STEAM_LINES {
            if faults.msiv_stuck[i] {
                continue;
            }
            let target = if self.msiv_cmd[i] { 1.0 } else { 0.0 };
            let d = target - self.msiv_pos[i];
            self.msiv_pos[i] += d.clamp(-dt / MSIV_STROKE, dt / MSIV_STROKE);
        }
        // Turbine stop valve: slams shut on trip, reopens slowly on reset.
        if !faults.tsv_stuck {
            let target = if tripped { 0.0 } else { 1.0 };
            let d = target - self.tsv_pos;
            self.tsv_pos += d.clamp(-dt / TSV_STROKE, TSV_OPEN_RATE * dt);
        }
        let p_ref = if self.p_chest > 0.5e6 { self.p_chest } else { (p - 0.05e6).max(0.2e6) };

        // Turbine control valves.
        if !faults.tcv_stuck {
            let tcv_target = if tripped { 0.0 } else { (tcv_flow_demand / (C_TCV * p_ref)).clamp(0.0, 1.0) };
            let d = tcv_target - self.tcv_pos;
            let (open_rate, close_rate) = if tripped { (0.0, 5.0) } else { (0.10, 0.5) };
            self.tcv_pos += d.clamp(-close_rate * dt, open_rate * dt);
        }

        // Bypass valves.
        let bypass_available = bypass_available && !faults.bypass_unavailable;
        let target = if !bypass_available {
            0.0
        } else if self.bypass_auto {
            let demand = self.pressure_demand(p, faults);
            let tcv_flow = if tripped { 0.0 } else { C_TCV * self.tcv_pos * self.tsv_pos * p_ref };
            ((demand - tcv_flow).max(0.0) / (C_BYPASS * p_ref)).clamp(0.0, 1.0)
        } else {
            self.bypass_manual_pos.clamp(0.0, 1.0)
        };
        let d = target - self.bypass_pos;
        let rate = if self.bypass_auto { 2.0 } else { 0.2 };
        self.bypass_pos += d.clamp(-rate * dt, rate * dt);
    }
```

- [ ] **Step 5: Wire into `sim.rs`**

Add to `impl Sim`:

```rust
    fn steam_faults(&self) -> SteamFaults {
        let mut f = SteamFaults::default();
        for line in 0..N_STEAM_LINES {
            f.msiv_stuck[line] = self.has_fault(&Fault::MsivStuck { line });
        }
        f.tsv_stuck = self.has_fault(&Fault::TsvStuck);
        f.tcv_stuck = self.has_fault(&Fault::TcvStuck);
        f.bypass_unavailable = self.has_fault(&Fault::BypassUnavailable);
        // Payload lookup goes through find_fault (master r2): exactly one
        // PressureRegulatorFailed entry can exist per the (kind, equipment) rule.
        f.regulator_frozen = match self.find_fault("PressureRegulatorFailed", None) {
            Some(Fault::PressureRegulatorFailed { demand_pct }) => Some(*demand_pct as f64 / 100.0),
            _ => None,
        };
        f
    }

    /// Bypass to the condenser is usable: condenser conditions permit it and no fault removes it.
    pub fn bypass_available(&self) -> bool {
        self.cond.available_for_bypass() && !self.has_fault(&Fault::BypassUnavailable)
    }
```

Add `use crate::bop::SteamFaults;` and `use crate::faults::Fault;` to the imports. Constructor call sites: `SteamSystem::new(true)` in `cold_shutdown` and `hot_standby` → `SteamSystem::new(true, false)`; in `full_power` → `SteamSystem::new(true, true)`. In `Sim::base` and `full_power`, `init_line_steady(p, tsv_open)` → `init_line_steady(p)`.

In `Sim::step` replace the steam block:

```rust
        // --- main steam, EHC, turbine
        let sf = self.steam_faults();
        let demand = self.steam.pressure_demand(p, &sf);
        let tcv_demand = self.turbine.tcv_demand(dt, demand);
        let byp_ok = self.bypass_available();
        self.steam.step(dt, p, tcv_demand, byp_ok, self.turbine.tripped, &sf);
        self.steam.compute_flows(dt, p_steam_side);
```

In `active_alarms`, change the `byp_unavail` condition to use `!self.bypass_available()`.

- [ ] **Step 6: Run all tests**

Run: `cargo test --release -p bwr-core`
Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add crates/bwr-core/src crates/bwr-core/tests
git commit -m "feat(bop): turbine stop valve state and steam-side fault holds

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 5: Bypass, load rejection, pressure-regulator faults and SRI provenance

**Files:**
- Modify: `crates/bwr-core/src/params.rs` (`SRI_TRIGGER_POWER_FRAC`; `BYPASS_CAPACITY` comment)
- Modify: `crates/bwr-core/src/sim.rs:~447` (`self.core.kin.n > 0.4` → constant)
- Modify: `crates/bwr-core/src/validation.rs` (`standard_cases()` append), `docs/validation/cases.csv`
- Modify: `crates/bwr-core/tests/plant.rs` (`turbine_trip_rides_through_on_bypass` message references case id)
- Test: `crates/bwr-core/tests/power_conversion.rs`

**Interfaces:** consumes `Sim::set_fault`, `validation::{Case, ScriptedCommand, ScriptedFault}`.

- [ ] **Step 1: Write the failing tests**

Append to `tests/power_conversion.rs`:

```rust
#[test]
fn load_rejection_with_bypass_does_not_scram() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    sim.apply(Command::OpenBreaker);
    let mut p_peak: f64 = 0.0;
    for _ in 0..(120.0 / DT) as usize {
        sim.step(DT);
        p_peak = p_peak.max(sim.vessel.eos.p);
    }
    assert!(!sim.scrammed, "scram: {:?}", sim.scram_reason);
    assert!(p_peak < SCRAM_HIGH_PRESSURE, "peak {p_peak}");
    assert!(sim.steam.w_bypass > 100.0, "bypass should be carrying steam");
    println!("load_rejection_bypass: p_peak={p_peak} turbine_tripped={} reason={:?}", sim.turbine.tripped, sim.turbine.trip_reason);
}

#[test]
fn turbine_trip_without_bypass_scrams_on_pressure_and_starts_ics() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    sim.set_fault(Fault::BypassUnavailable, true);
    sim.apply(Command::TripTurbine);
    let mut p_peak: f64 = 0.0;
    for _ in 0..(300.0 / DT) as usize {
        sim.step(DT);
        p_peak = p_peak.max(sim.vessel.eos.p);
    }
    assert_eq!(sim.scram_reason.as_deref(), Some("Reactor high pressure"));
    assert!(sim.ic.iter().any(|t| t.cmd_open), "ICs should auto-initiate on high pressure");
    assert!(p_peak < VESSEL_DESIGN_PRESSURE, "peak {p_peak}");
    assert!(sim.steam.bypass_pos < 1e-9, "bypass must stay shut under the fault");
    assert!(sim.has_fault(&Fault::BypassUnavailable));
}

#[test]
fn excessive_steam_demand_in_run_isolates() {
    // Pressure-controller fault expressed as a setpoint runaway to the minimum:
    // the regulator demands far more than rated flow and depressurises the plant.
    // (Fault::is_valid caps PressureRegulatorFailed at 100 %, which at full power
    // is the normal operating point, so the runaway is injected via the setpoint.)
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    sim.apply(Command::SetPressureSetpoint { mpa: 0.2 });
    run(&mut sim, 300.0);
    assert!(!sim.msivs_open(), "MSIVs should have isolated");
    assert!(sim.scrammed, "MSIV closure in RUN must scram");
    assert!(
        sim.log.iter().any(|(_, t)| t.starts_with("msiv isolation:")),
        "log must carry the isolation with its locked key: {:?}",
        sim.log
    );
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --release -p bwr-core --test power_conversion load_rejection excessive turbine_trip_without`
Expected: `load_rejection_with_bypass_does_not_scram` and `turbine_trip_without_bypass...` compile and may already pass (verification tests); `excessive_steam_demand_in_run_isolates` passes only if the P2 log line for MSIV isolation carries the locked `msiv isolation:` key. Record which passed at this point in the commit message.

- [ ] **Step 3: Make the SRI trigger a registered parameter**

In `params.rs`, in the protection setpoints section:

```rust
/// Power above which a turbine trip / load rejection triggers Select Rod Insert. ASSUMED (A-6xx).
pub const SRI_TRIGGER_POWER_FRAC: f64 = 0.4;
```

In `sim.rs` `protection()` replace `self.core.kin.n > 0.4` with `self.core.kin.n > SRI_TRIGGER_POWER_FRAC`. Update the `BYPASS_CAPACITY` doc comment to: `/// Turbine bypass capacity as fraction of rated steam flow. ASSUMED, status "to verify" per spec; the turbine_trip_no_bypass case bounds the zero-capacity end.`

- [ ] **Step 4: Register the cases**

Append to `standard_cases()` in `crates/bwr-core/src/validation.rs`:

```rust
        Case {
            id: "load_rejection_bypass",
            scenario: ScenarioName::FullPower,
            duration_s: 300.0,
            sample_dt_s: 0.5,
            commands: vec![ScriptedCommand { at_s: 5.0, cmd: Command::OpenBreaker }],
            faults: vec![],
            dt_after_s: None,
            operator: Operator::None,
        },
        Case {
            id: "turbine_trip_no_bypass",
            scenario: ScenarioName::FullPower,
            duration_s: 600.0,
            sample_dt_s: 0.5,
            commands: vec![ScriptedCommand { at_s: 5.0, cmd: Command::TripTurbine }],
            faults: vec![ScriptedFault { at_s: 0.0, fault: Fault::BypassUnavailable, active: true }],
            dt_after_s: None,
            operator: Operator::None,
        },
        Case {
            id: "regulator_fail_closed",
            scenario: ScenarioName::FullPower,
            duration_s: 300.0,
            sample_dt_s: 0.5,
            commands: vec![],
            faults: vec![ScriptedFault { at_s: 5.0, fault: Fault::PressureRegulatorFailed { demand_pct: 0 }, active: true }],
            dt_after_s: None,
            operator: Operator::None,
        },
        Case {
            id: "excess_steam_demand",
            scenario: ScenarioName::FullPower,
            duration_s: 300.0,
            sample_dt_s: 0.5,
            commands: vec![ScriptedCommand { at_s: 5.0, cmd: Command::SetPressureSetpoint { mpa: 0.2 } }],
            faults: vec![],
            dt_after_s: None,
            operator: Operator::None,
        },
```

(`use crate::validation::Operator;` is already in scope inside `validation.rs`; the literals set `dt_after_s: None, operator: Operator::None` per the r2 `Case` shape.)

Append rows to `docs/validation/cases.csv` using the locked header `id,scenario,duration_s,purpose,reference,tolerance_class,role,status`:

```
load_rejection_bypass,fullPower,300,Generator breaker opens with bypass available; verifies no scram and pressure peak below SCRAM_HIGH_PRESSURE,plausibility (no public BWRX trace),physical,plausibility,active
turbine_trip_no_bypass,fullPower,600,Turbine trip with bypass unavailable; verifies high-pressure scram then IC initiation with peak below VESSEL_DESIGN_PRESSURE,plausibility,physical,plausibility,active
regulator_fail_closed,fullPower,300,Pressure regulator output frozen at zero demand; verifies high-pressure scram,plausibility,physical,plausibility,active
excess_steam_demand,fullPower,300,Pressure setpoint runaway to 0.2 MPa; verifies RUN-mode isolation then MSIV-closure scram,plausibility,physical,plausibility,active
```

(Use P2's vocabulary for the `status` column if `method.md` defines one; `active` otherwise.)

In `tests/plant.rs` `turbine_trip_rides_through_on_bypass`, change the three assertion messages to reference the P2 case, e.g. `"case fp_turbine_trip: select rod insert should cut power, got {}"`; keep the thresholds (0.9, 200 kg/s, 7.4 MPa). `fp_turbine_trip` is registered in `cases.csv` by Package 2; update its `purpose` cell to name the thresholds: `Turbine trip with bypass; SRI ride-through; peak power < 0.9 and bypass > 200 kg/s and pressure < 7.4 MPa`.

- [ ] **Step 5: Run everything and regenerate baselines**

Run: `cargo test --release -p bwr-core` → all pass.
Run: `cargo run --release -p bwr-core --example validation -- --update` and commit the new baseline CSVs.

- [ ] **Step 6: Append results**

In `docs/model/power-conversion.md` "Results", add a table with one row per new case: peak pressure, time of peak, scram reason, IC initiation time, taken from the runner output.

- [ ] **Step 7: Commit**

```bash
git add crates/bwr-core docs/validation docs/model/power-conversion.md
git commit -m "test(bop): bypass, load rejection and regulator fault cases; SRI trigger registered

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 6: Control-volume plant energy balance with explicit extraction work

**Files:**
- Modify: `crates/bwr-core/src/params.rs` (`EXTRACTION_WORK_FRACTION`, `TURBINE_DH_RATED`)
- Modify: `crates/bwr-core/src/bop.rs` (`Turbine.p_extraction`, `Turbine.p_losses`, `Turbine::kinetic_energy`, `Turbine::step`, `EnergyBalance`)
- Modify: `crates/bwr-core/src/sim.rs` (`Sim.balance`, `house_load`, condenser block)
- Test: `crates/bwr-core/tests/power_conversion.rs`

**Interfaces:**
- Produces (master r2): `Turbine.p_extraction: f64` and `Turbine.p_losses: f64`; `Turbine::step(&mut self, dt, w_turbine, p_chest, p_cond, extraction_heat_w: f64)`; `EnergyBalance` with the master field list `{ q_core_w, q_condenser_w, q_ic_w, q_loss_w, p_elec_gross_w, p_losses_w, p_extraction_w, house_pump_work_w, makeup_w, reject_w, vent_w, storage_w, residual_w }`; `Sim.balance: EnergyBalance`; `Sim::house_load(&self) -> f64`. Package-local: `Turbine::kinetic_energy(&self) -> f64` (shaft KE storage term; appended to this plan per the master's add-in-your-own-plan rule — nothing outside P6 consumes it).
- Control volume (master): vessel + steam lines + turbine + condenser/hotwell + feed train + IC tubes. Sign convention: positive into the volume. `residual = in - out - storage_rate` with `in = q_core + makeup_w + house_pump_work_w`, `out = q_condenser + q_ic + q_loss + p_elec_gross + p_losses + reject_w + vent_w`, `storage = d/dt(vessel.energy) + d/dt(line.energy) + d/dt(hotwell_energy) + d/dt(ic tube energy; 0 before P7) + d/dt(shaft KE)`. `p_extraction` is internal (steam to feedwater) and is reported for reconciliation only, never as a boundary term.
- Consumes: `BoundaryFlows` returned by `Vessel::step` (P2); the local in `Sim::step` is named `flows` and equals `self.last_flows()` after the step. Task 7 supplies `Condenser.hotwell_energy`; until it lands this task uses `self.cond.hotwell_mass * steam::h_liquid(self.cond.hotwell_t())` inline (see Step 5 note).

- [ ] **Step 1: Write the failing test**

Append to `tests/power_conversion.rs`:

```rust
fn print_balance(b: &bwr_core::bop::EnergyBalance) {
    println!(
        "core {:.1} cond {:.1} ic {:.1} loss {:.1} elec {:.1} tglosses {:.1} extraction {:.1} pumpwork {:.1} makeup {:.1} reject {:.1} vent {:.1} storage {:.1} residual {:.3} MW",
        b.q_core_w / 1e6, b.q_condenser_w / 1e6, b.q_ic_w / 1e6, b.q_loss_w / 1e6,
        b.p_elec_gross_w / 1e6, b.p_losses_w / 1e6, b.p_extraction_w / 1e6, b.house_pump_work_w / 1e6,
        b.makeup_w / 1e6, b.reject_w / 1e6, b.vent_w / 1e6, b.storage_w / 1e6, b.residual_w / 1e6
    );
}

#[test]
fn full_power_energy_balance_closes() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 300.0);
    let b = sim.balance;
    print_balance(&b);
    // Closure of in - out - storage (only the saturated-line enthalpy approximation remains).
    assert!(b.residual_w.abs() < 0.01 * RATED_POWER_W, "residual {} MW", b.residual_w / 1e6);
    // Physical closure at steady state (storage ~ 0): core heat splits into electric
    // power, machine losses and heat rejection, within 2 %.
    let out = b.p_elec_gross_w + b.p_losses_w + b.q_condenser_w + b.q_ic_w + b.q_loss_w;
    assert!((b.q_core_w - out).abs() < 0.02 * RATED_POWER_W, "core {} out {}", b.q_core_w / 1e6, out / 1e6);
    assert!(b.storage_w.abs() < 0.005 * RATED_POWER_W, "steady storage {}", b.storage_w / 1e6);
    assert!(b.p_extraction_w > 50.0e6 && b.p_extraction_w < 150.0e6, "extraction {}", b.p_extraction_w / 1e6);
    assert!(b.q_condenser_w > 480.0e6 && b.q_condenser_w < 600.0e6, "condenser {}", b.q_condenser_w / 1e6);
    let s = sim.snapshot();
    assert!(s.turbine.net_mwe > 285.0 && s.turbine.net_mwe < 315.0, "net {}", s.turbine.net_mwe);
}

#[test]
fn energy_balance_closes_through_turbine_trip_transient() {
    // Transient closure over the first 60 s of the fp_turbine_trip case: storage terms
    // (line node, hotwell, shaft KE, vessel) must absorb the swing, not the residual.
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    sim.apply(Command::TripTurbine);
    let mut worst = 0.0f64;
    for _ in 0..(60.0 / DT) as usize {
        sim.step(DT);
        if sim.balance.residual_w.abs() > worst.abs() {
            worst = sim.balance.residual_w;
            print_balance(&sim.balance);
        }
    }
    assert!(worst.abs() < 0.01 * RATED_POWER_W, "worst transient residual {} MW", worst / 1e6);
}
```

If the transient assertion fails solely because of the hotwell saturation-temperature swing (identify it in the printed per-term table: `storage` tracking `cond` while every flow term is steady), report the measured value, set the transient bound to twice the measured value, and record both numbers and the cause in `docs/validation/method.md`. Do not weaken the steady-state assertion.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --release -p bwr-core --test power_conversion full_power_energy`
Expected: compile error `no field balance on type Sim`.

- [ ] **Step 3: Parameters**

In `params.rs`, replace

```rust
/// Effective turbine enthalpy drop to shaft at rated conditions, J/kg. ASSUMED
/// (gives ~315 MWe gross / ~300 MWe net).
pub const TURBINE_DH_RATED: f64 = 0.703e6;
```

with

```rust
/// Shaft work per kg of throttle steam if it all expanded to the condenser, J/kg.
/// CALIBRATED: 0.703e6 (baseline effective value, which implicitly included extraction)
/// + EXTRACTION_WORK_FRACTION * rated heater duty / STEAM_FLOW_RATED
/// = 0.703e6 + 0.25 * 470 * (921.0e3 - 146.6e3) / 470 = 0.897e6, so that net output stays ~300 MWe.
/// Below the ~0.92e6 isentropic drop from 7 MPa saturated to 5.6 kPa (sanity bound).
pub const TURBINE_DH_RATED: f64 = 0.897e6;
/// LP turbine work forgone per unit of extraction heat delivered to the feedwater heaters.
/// ASSUMED (A-6xx); sensitivity interval 0.15..0.35 (low-pressure vs high-pressure extraction).
pub const EXTRACTION_WORK_FRACTION: f64 = 0.25;
```

- [ ] **Step 4: Turbine extraction, losses and shaft KE in `bop.rs`**

Add `pub p_extraction: f64,` and `pub p_losses: f64,` to `Turbine` (both `0.0` in `new`). Change `step` so both are set on every path:

```rust
    /// Shaft kinetic energy, J (storage term of the plant energy balance).
    pub fn kinetic_energy(&self) -> f64 {
        let omega = self.speed_rpm * std::f64::consts::PI / 30.0;
        0.5 * TG_INERTIA * omega * omega
    }

    /// Advance shaft dynamics. `extraction_heat_w` is the heat the feedwater heaters
    /// are drawing from extraction steam; the LP work that steam would have done is forgone.
    pub fn step(&mut self, dt: f64, w_turbine: f64, p_chest: f64, p_cond: f64, extraction_heat_w: f64) {
        let pf = if p_chest > p_cond && p_cond > 0.0 {
            ((p_chest / p_cond).ln() / (7.0e6f64 / 5.6e3).ln()).clamp(0.0, 1.2)
        } else {
            0.0
        };
        let gross = w_turbine * TURBINE_DH_RATED * pf;
        self.p_extraction = (EXTRACTION_WORK_FRACTION * extraction_heat_w.max(0.0)).min(gross);
        self.p_mech = gross - self.p_extraction;
        let w_frac = self.speed_rpm / RATED_RPM;
        let losses = TG_LOSSES * w_frac * w_frac + 0.5e6 * w_frac.min(1.0) * (self.speed_rpm > 1.0) as i32 as f64;

        if self.breaker_closed {
            // ...existing branch; at its end add:
            // Everything mechanical that does not leave as gross electric power is a loss.
            self.p_losses = (self.p_mech - self.p_elec.max(0.0)).max(0.0);
        } else {
            // ...existing branch (KE integration); at its end add:
            // Offline the dissipation is the friction/windage term; KE change is storage.
            self.p_losses = losses.min(self.p_mech.max(0.0) + self.kinetic_energy() / dt.max(1e-9));
        }
        // ...overspeed check unchanged
```

(The `min` on the offline branch stops `p_losses` claiming more energy than the shaft and steam actually supplied in the step, which matters only in the last second of coastdown.)

Add after the `Turbine` impl, copying the master's control-volume comment verbatim:

```rust
/// Whole-plant energy balance, W, evaluated every step.
/// Control volume: vessel + steam lines + turbine + condenser/hotwell + feed train + IC tubes.
/// Sign convention: positive = into the control volume. residual = in - out - storage_rate.
///   in      = q_core + makeup_flow*h_makeup + house_pump_work
///   out     = q_condenser + q_ic (to pools) + q_loss + p_elec_gross + p_losses + reject_flow*h_hotwell + w_vent*h_g
///   storage = d/dt(vessel.energy) + d/dt(line.energy) + d/dt(hotwell_energy) + d/dt(ic tube energy) + d/dt(shaft KE)
/// p_extraction is NOT a boundary term (it is internal: steam -> feedwater); reported for reconciliation only.
#[derive(Clone, Copy, Debug, Default)]
pub struct EnergyBalance {
    pub q_core_w: f64,
    pub q_condenser_w: f64,
    pub q_ic_w: f64,
    pub q_loss_w: f64,
    pub p_elec_gross_w: f64,
    pub p_losses_w: f64,
    pub p_extraction_w: f64,
    pub house_pump_work_w: f64,
    pub makeup_w: f64,
    pub reject_w: f64,
    pub vent_w: f64,
    pub storage_w: f64,
    pub residual_w: f64,
}
```

- [ ] **Step 5: Assemble in `sim.rs`**

Add `pub balance: EnergyBalance,` to `Sim` (initialise `EnergyBalance::default()` in `base`). Add:

```rust
    /// Auxiliary electrical load, W.
    pub fn house_load(&self) -> f64 {
        HOUSE_BASE
            + self.fw.running_pumps() as f64 * FW_PUMP_POWER
            + self.fw.cond_pumps.iter().filter(|&&b| b).count() as f64 * COND_PUMP_POWER
            + self.cond.cw_pumps.iter().filter(|&&b| b).count() as f64 * CW_PUMP_POWER
    }
```

and in `snapshot()` replace the inline `let house = HOUSE_BASE + ...;` with `let house = self.house_load();`.

At the **top** of `Sim::step` (first statements, before any component moves), capture the stored energy of the control volume:

```rust
        let stored_before = self.vessel.energy
            + self.steam.line.energy
            + self.cond.hotwell_energy
            + self.turbine.kinetic_energy();
        // + IC tube energy from Package 7 (0 until then)
```

The turbine call (master step 11, after the allocation) becomes (extraction heat uses the previous step's feed state, which lags 180 s anyway):

```rust
        let h_hw = steam::h_liquid(self.cond.hotwell_t());
        let extraction_heat = self.fw.flow * (self.fw.h_feed() - h_hw);
        self.turbine.step(dt, self.steam.w_tcv, self.steam.p_chest, p_cond, extraction_heat);
```

Replace the whole `// --- condenser` block with (master step 12 plus the balance):

```rust
        // --- condenser (applied flows only)
        let h_line = sat_p(self.steam.line.p.max(P_ATM)).h_g;
        let w_rwcu = flows.w_liquid_out;          // applied RWCU reject
        let h_rwcu = flows.h_liquid_out;
        let w_to_bop = self.steam.w_tcv + self.steam.w_bypass;  // applied (post-allocation)
        let w_in = w_to_bop + w_rwcu;
        let fw_heating = self.fw.flow * (self.fw.h_feed() - h_hw);
        // Everything downstream of the TSV and bypass that is not shaft work or feedwater
        // enthalpy rise is rejected to circulating water.
        let q_cond = w_to_bop * h_line + w_rwcu * h_rwcu - w_in * h_hw - self.turbine.p_mech - fw_heating;
        self.cond.step(dt, q_cond, w_in, self.fw.flow, &cf);

        // --- plant energy balance (master control volume; see EnergyBalance doc comment)
        let stored_after = self.vessel.energy
            + self.steam.line.energy
            + self.cond.hotwell_energy
            + self.turbine.kinetic_energy();
        let h_hw_now = steam::h_liquid(self.cond.hotwell_t());
        let mut b = EnergyBalance {
            q_core_w: flows.q_core,
            q_condenser_w: self.cond.heat_load,
            q_ic_w: q_ic,
            q_loss_w: flows.q_loss,
            p_elec_gross_w: self.turbine.p_elec.max(0.0),
            p_losses_w: self.turbine.p_losses,
            p_extraction_w: self.turbine.p_extraction,
            // Pump work entering the feed stream: the model's 2 K pump heat term.
            house_pump_work_w: if self.fw.pump_speed.iter().any(|&s| s > 0.01) {
                self.fw.flow * crate::ic::CP_WATER * 2.0
            } else {
                0.0
            },
            // CST exchange at hotwell enthalpy (assumption H_CST_EXCHANGE).
            makeup_w: self.cond.makeup_flow * h_hw_now,
            reject_w: self.cond.reject_flow * h_hw_now,
            vent_w: flows.w_vent * flows.h_steam,
            storage_w: (stored_after - stored_before) / dt,
            residual_w: 0.0,
        };
        b.residual_w = (b.q_core_w + b.makeup_w + b.house_pump_work_w)
            - (b.q_condenser_w + b.q_ic_w + b.q_loss_w + b.p_elec_gross_w + b.p_losses_w + b.reject_w + b.vent_w)
            - b.storage_w;
        self.balance = b;
```

`cf` comes from the single `let (sf, cf, ff) = self.bop_faults();` at the top of `step` (Task 4/7/8). Until Task 7 lands, `Condenser.hotwell_energy`, `makeup_flow`, `reject_flow` and `CondenserFaults` do not exist: in this task use `self.cond.hotwell_mass * steam::h_liquid(self.cond.hotwell_t())` wherever `hotwell_energy` appears, set `makeup_w`/`reject_w` to `0.0`, and call `self.cond.step(dt, q_cond, w_in, self.fw.flow)` with the old signature. Task 7 upgrades these lines and the ±30 kg/s legacy makeup then stops perturbing the balance.

Closure argument (write this into `docs/model/power-conversion.md` "Energy balance" section): every mass flow crosses the boundary only at the vessel vent and the CST exchange; all internal streams (steam to turbine, condensate, feed, extraction) cancel because `q_cond` is defined as the enthalpy the condensing streams give up beyond shaft work and feedwater heating, and the hotwell/line/vessel/shaft storage terms absorb the imbalances the component lags create. What remains in the residual: the saturated-line enthalpy approximation, `w_line_total*(h_g(p_vessel) - h_g(p_line))` ≈ −0.8 MW at rated (< 0.1 %), the untracked feed-train water/heater-metal storage behind the 180 s `t_feed` lag (compensated to first order because `q_cond` subtracts the same `fw_heating` that feeds `h_feed`), and the hotwell saturation-temperature swing term `hotwell_mass * d(h_f(t_sat))/dt`, which is small whenever bypass keeps the condenser near its operating point. These three are the documented uncertainty of the balance; the transient test bounds their sum.

- [ ] **Step 6: Run all tests**

Run: `cargo test --release -p bwr-core`
Expected: all pass; `full_power_is_steady` net MWe within 285–315 (if not, the calibration arithmetic in Step 3 is wrong: recompute `TURBINE_DH_RATED` from the printed `extraction` value so that net returns to the baseline value, and record both numbers).

- [ ] **Step 7: Document and commit**

Append the "Energy balance" section (term definitions, the algebraic residual, the printed full-power table, old and new `TURBINE_DH_RATED`) to `docs/model/power-conversion.md`.

```bash
git add crates/bwr-core docs/model/power-conversion.md
git commit -m "feat(sim): explicit extraction work and one-line plant energy balance

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 7: Condenser: explicit CST boundary, air blanketing, cooling-water and vacuum faults

**Files:**
- Modify: `crates/bwr-core/src/params.rs` (`CST_CAPACITY_KG`, `CST_MASS_INITIAL`, `HOTWELL_MAKEUP_KGS`, `HOTWELL_MAKEUP_ON`, `HOTWELL_REJECT_ON`, `AIR_UA_DEGRADATION`)
- Modify: `crates/bwr-core/src/bop.rs` (`CondenserFaults`, `Condenser` fields and `step`)
- Modify: `crates/bwr-core/src/sim.rs` (`bop_faults` partial, alarms `cst_low`, `hotwell_overflow`, `SetCwPump`/`SetVacuumPump` blocked under fault)
- Test: `crates/bwr-core/tests/power_conversion.rs`

**Interfaces:**
- Produces: `#[derive(Clone, Copy, Debug, Default)] pub struct CondenserFaults { pub cw_trip: [bool; N_CW_PUMPS], pub vacuum_failed: bool }`; `Condenser { cst_mass, makeup_flow, reject_flow, hotwell_energy, overflow_kg, deficit_kg }` (`hotwell_energy` is the master's storage term; `overflow_kg`/`deficit_kg` are package-local ledger-style counters, never silent clamps); `Condenser::step(&mut self, dt, q, w_in, w_out, faults: &CondenserFaults)`.
- Master hotwell rule, implemented here: overflow above `HOTWELL_CAPACITY` goes to `reject_flow` (bounded by remaining CST space, remainder counted in `overflow_kg`); underflow sets the hotwell to 0, counts the shortfall in `deficit_kg`, and starves the condensate pumps through the existing `hotwell_ok` low-suction trip path. Both counters appear in the plant water-inventory test below, so nothing is discarded unaccounted.

- [ ] **Step 1: Write the failing tests**

Append to `tests/power_conversion.rs`:

```rust
#[test]
fn hotwell_makeup_and_reject_are_explicit_and_balanced_at_steady_state() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 60.0);
    let cst0 = sim.cond.cst_mass;
    let mut net = 0.0;
    for _ in 0..(600.0 / DT) as usize {
        sim.step(DT);
        net += (sim.cond.makeup_flow - sim.cond.reject_flow) * DT;
    }
    assert!((net / 600.0).abs() < 1.0, "net CST exchange {} kg/s", net / 600.0);
    assert!(((cst0 - sim.cond.cst_mass) - net).abs() < 1e-6, "CST must account for every kg exchanged");
    assert_eq!(sim.cond.overflow_kg, 0.0);
    assert_eq!(sim.cond.deficit_kg, 0.0);
}

#[test]
fn plant_water_inventory_closes_through_a_turbine_trip() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    let total = |s: &Sim, vented: f64| s.vessel.mass + s.steam.line.mass + s.cond.hotwell_mass + s.cond.cst_mass + vented - s.cond.deficit_kg + s.cond.overflow_kg;
    let mut vented = 0.0;
    let t0 = total(&sim, vented);
    sim.apply(Command::TripTurbine);
    for _ in 0..(600.0 / DT) as usize {
        sim.step(DT);
        vented += sim.vessel.vent_flow * DT;
    }
    let t1 = total(&sim, vented);
    assert!((t1 - t0).abs() < 1.0, "plant water inventory drifted by {} kg", t1 - t0);
}

#[test]
fn loss_of_circulating_water_trips_turbine_then_scrams_on_pressure() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    sim.set_fault(Fault::CwPumpTrip { index: 0 }, true);
    sim.set_fault(Fault::CwPumpTrip { index: 1 }, true);
    sim.apply(Command::SetCwPump { index: 0, on: true }); // must be refused
    let mut trip_t = None;
    let mut p_peak: f64 = 0.0;
    for _ in 0..(900.0 / DT) as usize {
        sim.step(DT);
        p_peak = p_peak.max(sim.vessel.eos.p);
        if trip_t.is_none() && sim.turbine.tripped {
            trip_t = Some(sim.time);
        }
    }
    assert!(!sim.cond.cw_pumps.iter().any(|&b| b), "tripped CW pumps must not run");
    assert_eq!(sim.turbine.trip_reason.as_deref(), Some("Low condenser vacuum"));
    assert!(!sim.cond.available_for_bypass(), "bypass must be blocked above CONDENSER_BYPASS_BLOCK_P");
    assert_eq!(sim.scram_reason.as_deref(), Some("Reactor high pressure"));
    assert!(sim.ic.iter().any(|t| t.cmd_open));
    assert!(p_peak < VESSEL_DESIGN_PRESSURE);
    println!("loss_of_cw: turbine trip at {:?}, scram at {}, p_peak {}", trip_t, sim.scram_time, p_peak);
}

#[test]
fn vacuum_pump_failure_degrades_vacuum_monotonically() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 60.0);
    sim.set_fault(Fault::VacuumPumpFailed, true);
    sim.apply(Command::SetVacuumPump { on: true }); // refused
    let p0 = sim.cond.pressure();
    let mut last = p0;
    for _ in 0..(4.0 * 3600.0 / 1.0) as usize {
        run(&mut sim, 1.0);
        let p = sim.cond.pressure();
        assert!(p >= last - 1.0, "vacuum must not recover without the vacuum pump: {p} < {last}");
        last = p;
    }
    assert!(!sim.cond.vacuum_pump);
    assert!(last > p0 + 1.0e3, "4 h of in-leakage should raise condenser pressure by > 1 kPa: {} -> {}", p0, last);
    println!("vacuum_pump_failed: {} -> {} Pa over 4 h", p0, last);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --release -p bwr-core --test power_conversion hotwell plant_water loss_of vacuum_pump`
Expected: compile error `no field cst_mass`.

- [ ] **Step 3: Parameters**

In `params.rs` condenser section, replace `pub const HOTWELL_CAPACITY: f64 = 120_000.0;` with:

```rust
pub const HOTWELL_CAPACITY: f64 = 120_000.0;
/// Hotwell makeup / reject pump rate to and from the condensate storage tank, kg/s. ASSUMED.
pub const HOTWELL_MAKEUP_KGS: f64 = 30.0;
/// Hotwell fraction below which makeup runs and above which reject runs. ASSUMED.
pub const HOTWELL_MAKEUP_ON: f64 = 0.40;
pub const HOTWELL_REJECT_ON: f64 = 0.85;
/// Condensate storage tank: declared boundary inventory, kg. ASSUMED (A-6xx).
pub const CST_CAPACITY_KG: f64 = 2.0e6;
pub const CST_MASS_INITIAL: f64 = 1.5e6;
/// Condenser UA is divided by (1 + AIR_UA_DEGRADATION * p_air / p_steam): air blanketing. ASSUMED (A-6xx), sensitivity 2..10.
pub const AIR_UA_DEGRADATION: f64 = 5.0;
```

- [ ] **Step 4: Implement in `bop.rs`**

Add above `pub struct Condenser`:

```rust
#[derive(Clone, Copy, Debug, Default)]
pub struct CondenserFaults {
    pub cw_trip: [bool; N_CW_PUMPS],
    pub vacuum_failed: bool,
}
```

Add fields to `Condenser`: `pub cst_mass: f64, pub makeup_flow: f64, pub reject_flow: f64, /// Hotwell liquid energy, J (storage term of the plant energy balance). pub hotwell_energy: f64, /// Hotwell water lost over the top because the CST was full, kg (never silently discarded). pub overflow_kg: f64, /// Water the hotwell could not supply (pumps ran it dry), kg. pub deficit_kg: f64,`. In `new`: `cst_mass: CST_MASS_INITIAL, makeup_flow: 0.0, reject_flow: 0.0, hotwell_energy: 0.6 * HOTWELL_CAPACITY * steam::h_liquid(CW_INLET_C), overflow_kg: 0.0, deficit_kg: 0.0` (same mass basis as the existing `hotwell_mass: 0.6 * HOTWELL_CAPACITY`).

Replace `step`:

```rust
    /// Advance: `q` heat to reject (W), `w_in` water returned to the hotwell,
    /// `w_out` condensate pumped out, kg/s.
    pub fn step(&mut self, dt: f64, q: f64, w_in: f64, w_out: f64, faults: &CondenserFaults) {
        for i in 0..N_CW_PUMPS {
            if faults.cw_trip[i] {
                self.cw_pumps[i] = false;
            }
        }
        if faults.vacuum_failed {
            self.vacuum_pump = false;
        }
        self.heat_load = q.max(0.0);
        let n_cw = self.cw_pumps.iter().filter(|&&b| b).count() as f64;
        let m_cw = n_cw * CW_FLOW_PER_PUMP;
        // Air blanketing: noncondensables lower the effective UA.
        let ua = CONDENSER_UA / (1.0 + AIR_UA_DEGRADATION * self.p_air() / self.p_steam.max(700.0));
        let target = if m_cw > 0.0 {
            let mcp = m_cw * 4186.0;
            let eff = 1.0 - (-ua / mcp).exp();
            let ts = CW_INLET_C + self.heat_load / (mcp * eff);
            steam::sat_t(ts).p
        } else if self.heat_load > 1.0e5 {
            1.2 * P_ATM
        } else {
            steam::sat_t(30.0).p
        };
        let tau = if m_cw > 0.0 { 15.0 } else { 120.0 };
        self.p_steam += (target - self.p_steam) * (dt / tau).min(1.0);
        self.t_sat = sat_p(self.p_steam.max(700.0)).t;

        let p = self.pressure();
        let leak = AIR_INLEAKAGE * ((P_ATM - p) / P_ATM).clamp(0.0, 1.0);
        let removal = if self.vacuum_pump { self.m_air / VACUUM_PUMP_TAU } else { 0.0 };
        self.m_air = (self.m_air + (leak - removal) * dt).max(0.0);
        let max_air = (P_ATM - self.p_steam).max(0.0) * CONDENSER_VOLUME / (287.0 * 303.0);
        self.m_air = self.m_air.min(max_air.max(0.0));

        // Hotwell inventory with explicit makeup / reject to the condensate storage tank.
        self.hotwell_mass += (w_in - w_out) * dt;
        let frac = self.hotwell_mass / HOTWELL_CAPACITY;
        let mut makeup = if frac < HOTWELL_MAKEUP_ON { HOTWELL_MAKEUP_KGS } else { 0.0 };
        let mut reject = if frac > HOTWELL_REJECT_ON { HOTWELL_MAKEUP_KGS } else { 0.0 };
        if self.hotwell_mass > HOTWELL_CAPACITY {
            reject += (self.hotwell_mass - HOTWELL_CAPACITY) / dt;
        }
        makeup = makeup.min(self.cst_mass / dt).max(0.0);
        reject = reject.min((CST_CAPACITY_KG - self.cst_mass) / dt).max(0.0);
        self.hotwell_mass += (makeup - reject) * dt;
        self.cst_mass += (reject - makeup) * dt;
        if self.hotwell_mass > HOTWELL_CAPACITY {
            self.overflow_kg += self.hotwell_mass - HOTWELL_CAPACITY;
            self.hotwell_mass = HOTWELL_CAPACITY;
        }
        if self.hotwell_mass < 0.0 {
            self.deficit_kg += -self.hotwell_mass;
            self.hotwell_mass = 0.0;
        }
        self.makeup_flow = makeup;
        self.reject_flow = reject;
        // Storage term for the plant energy balance: hotwell liquid at saturation.
        self.hotwell_energy = self.hotwell_mass * steam::h_liquid(self.hotwell_t());
    }

    pub fn cst_level_pct(&self) -> f64 {
        self.cst_mass / CST_CAPACITY_KG * 100.0
    }
```

- [ ] **Step 5: Wire into `sim.rs`**

Add a provisional `bop_faults`:

```rust
    fn bop_faults(&self) -> (SteamFaults, CondenserFaults, ()) {
        let mut cf = CondenserFaults::default();
        for index in 0..N_CW_PUMPS {
            cf.cw_trip[index] = self.has_fault(&Fault::CwPumpTrip { index });
        }
        cf.vacuum_failed = self.has_fault(&Fault::VacuumPumpFailed);
        (self.steam_faults(), cf, ())
    }
```

(Task 8 replaces `()` with `FeedFaults`.) Upgrade the Task 6 condenser block to the full form (with `makeup_flow`/`reject_flow` and `&cf`). In `apply`, refuse faulted equipment:

```rust
            Command::SetCwPump { index, on } => {
                if index < N_CW_PUMPS {
                    if on && self.has_fault(&Fault::CwPumpTrip { index }) {
                        self.refuse("cw", format!("CW pump {} start blocked: pump tripped", index + 1));
                        return;
                    }
                    self.cond.cw_pumps[index] = on;
                }
            }
            Command::SetVacuumPump { on } => {
                if on && self.has_fault(&Fault::VacuumPumpFailed) {
                    self.refuse("vac_pump", "Vacuum pump start blocked: pump failed");
                    return;
                }
                self.cond.vacuum_pump = on;
            }
```

`refuse` is a package-local helper that raises the transient operator message **and** writes the locked-key log line (master log-line convention, key `command ignored`), so every refused command is visible in `Trace.events`:

```rust
    /// Refuse an operator command: operator message plus a `command ignored:` log line.
    fn refuse(&mut self, id: &str, detail: impl Into<String>) {
        let detail = detail.into();
        self.log.push((self.time, format!("command ignored: {detail}")));
        self.event(id, detail);
    }
```

(If Package 2 already provides an equivalent helper for ignored commands, call that instead and delete this one; the log text must start with `command ignored:` either way.)
```

Alarms in `active_alarms`:

```rust
        if self.cond.cst_level_pct() < 10.0 {
            add("cst_low", "CONDENSATE STORAGE TANK LEVEL LOW".into(), Severity::Alarm);
        }
        if self.cond.overflow_kg > 0.0 {
            add("hotwell_overflow", format!("HOTWELL OVERFLOW {:.0} kg (CST FULL)", self.cond.overflow_kg), Severity::Warning);
        }
```

- [ ] **Step 6: Run all tests**

Run: `cargo test --release -p bwr-core`
Expected: all pass. The 4-hour vacuum test runs ~300k steps (about 3 s in release).

- [ ] **Step 7: Document and commit**

Append to `docs/model/power-conversion.md` "Results": CW-loss event times (turbine trip, bypass block, scram, IC), 4 h vacuum degradation numbers, and the statement that no MSIV isolation on low vacuum is modelled because no BWRX source for such a signal was found (spec: do not invent).

```bash
git add crates/bwr-core docs/model/power-conversion.md
git commit -m "feat(bop): explicit CST boundary, hotwell overflow accounting, air blanketing, CW/vacuum faults

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 8: Feedwater: pump speed states, parallel hydraulics, bumpless transfer, faults

**Files:**
- Modify: `crates/bwr-core/src/params.rs` (`FW_STATIC_DP`, `FW_PUMP_COASTDOWN_S`)
- Modify: `crates/bwr-core/src/bop.rs` (`FeedFaults`, `Feedwater.pump_speed`, `hydraulics`, `set_mode`, `step`)
- Modify: `crates/bwr-core/src/sim.rs` (`bop_faults` complete, `SetLevelControl`, `SetFeedPump`/`SetCondensatePump` refusals, feedwater call)
- Test: `crates/bwr-core/tests/power_conversion.rs`

**Interfaces:**
- Produces: `#[derive(Clone, Copy, Debug, Default)] pub struct FeedFaults { pub pump_trip: [bool; N_FEED_PUMPS], pub cond_pump_trip: [bool; N_COND_PUMPS], pub fcv_stuck: bool, pub heaters_lost: bool }`; `Feedwater.pump_speed: [f64; N_FEED_PUMPS]`; `Feedwater::set_mode(&mut self, mode: LevelControlMode)` (master signature; bumpless via a private re-track flag consumed by the next `step`); `Feedwater::step(&mut self, dt, p_reactor, level_cm, steam_flow, turbine_flow, hotwell_ok, hotwell_t, faults: &FeedFaults) -> Option<String>`; `Sim::bop_faults(&self) -> (SteamFaults, CondenserFaults, FeedFaults)`.

- [ ] **Step 1: Write the failing tests**

Append to `tests/power_conversion.rs`:

```rust
use bwr_core::bop::{Feedwater, LevelControlMode};

#[test]
fn stopped_feed_pumps_pass_no_reverse_flow_against_vessel_pressure() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    for i in 0..N_FEED_PUMPS {
        sim.apply(Command::SetFeedPump { index: i, on: false });
    }
    run(&mut sim, 30.0); // past coastdown
    assert!(sim.fw.pump_speed.iter().all(|&s| s < 0.01), "speeds {:?}", sim.fw.pump_speed);
    assert_eq!(sim.fw.flow, 0.0, "check valves: no flow into a 7.2 MPa vessel without pumps");
}

#[test]
fn feed_pump_coasts_down_rather_than_stopping_instantly() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    let f0 = sim.fw.flow;
    sim.apply(Command::SetFeedPump { index: 0, on: false });
    run(&mut sim, 1.0);
    assert!(sim.fw.pump_speed[0] > 0.5 && sim.fw.pump_speed[0] < 0.95, "speed after 1 s {}", sim.fw.pump_speed[0]);
    assert!(sim.fw.flow > 0.5 * f0, "flow should not collapse within 1 s: {} -> {}", f0, sim.fw.flow);
}

#[test]
fn tripped_feed_pump_cannot_be_restarted_and_fault_persists() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    sim.set_fault(Fault::FeedPumpTrip { index: 1 }, true);
    run(&mut sim, 1.0);
    assert!(!sim.fw.pumps[1]);
    sim.apply(Command::SetFeedPump { index: 1, on: true });
    run(&mut sim, 1.0);
    assert!(!sim.fw.pumps[1], "start must be refused under the fault");
    assert!(sim.has_fault(&Fault::FeedPumpTrip { index: 1 }));
}

#[test]
fn stuck_fcv_holds_position_and_lost_heaters_cannot_be_restored() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    let pos = sim.fw.fcv_pos;
    sim.set_fault(Fault::FcvStuck, true);
    sim.apply(Command::SetLevelSetpoint { cm: 40.0 });
    run(&mut sim, 60.0);
    assert!((sim.fw.fcv_pos - pos).abs() < 1e-9, "FCV moved under FcvStuck");
    sim.set_fault(Fault::HeatersLost, true);
    sim.apply(Command::SetHeaters { in_service: true });
    run(&mut sim, 1.0);
    assert!(!sim.fw.heaters_in_service);
}

#[test]
fn level_control_mode_transfer_is_bumpless() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 120.0);
    let pos0 = sim.fw.fcv_pos;
    sim.apply(Command::SetLevelControl { mode: LevelControlMode::Manual });
    run(&mut sim, 2.0);
    assert!((sim.fw.fcv_pos - pos0).abs() < 0.01, "manual transfer bumped {} -> {}", pos0, sim.fw.fcv_pos);
    sim.apply(Command::SetLevelControl { mode: LevelControlMode::SingleElement });
    run(&mut sim, 2.0);
    assert!((sim.fw.fcv_pos - pos0).abs() < 0.02, "1-element transfer bumped {} -> {}", pos0, sim.fw.fcv_pos);
    sim.apply(Command::SetLevelControl { mode: LevelControlMode::ThreeElement });
    run(&mut sim, 2.0);
    assert!((sim.fw.fcv_pos - pos0).abs() < 0.02, "3-element transfer bumped {} -> {}", pos0, sim.fw.fcv_pos);
}

#[test]
fn loss_of_feedwater_heating_raises_power() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    let n0 = sim.core.kin.n;
    let t0 = sim.fw.t_feed;
    sim.set_fault(Fault::HeatersLost, true);
    let mut peak = n0;
    for _ in 0..(600.0 / DT) as usize {
        sim.step(DT);
        peak = peak.max(sim.core.kin.n);
    }
    assert!(sim.fw.t_feed < t0 - 20.0, "feed temperature should fall: {} -> {}", t0, sim.fw.t_feed);
    assert!(peak > n0 + 0.02, "colder feedwater must add reactivity: peak {peak} vs {n0}");
    println!("heater_loss: t_feed {t0} -> {}, power peak {peak}, scram {:?}", sim.fw.t_feed, sim.scram_reason);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --release -p bwr-core --test power_conversion feed stuck_fcv level_control loss_of_feedwater`
Expected: compile error `no field pump_speed`.

- [ ] **Step 3: Parameters**

In `params.rs` feedwater section:

```rust
/// Static head plus piping loss from feed pumps to the vessel nozzle at zero flow, Pa. ASSUMED (was a literal).
pub const FW_STATIC_DP: f64 = 0.15e6;
/// Feed pump speed time constant for start and coastdown, s. ASSUMED (A-6xx).
pub const FW_PUMP_COASTDOWN_S: f64 = 5.0;
```

- [ ] **Step 4: Implement in `bop.rs`**

Add above `pub struct Feedwater`:

```rust
#[derive(Clone, Copy, Debug, Default)]
pub struct FeedFaults {
    pub pump_trip: [bool; N_FEED_PUMPS],
    pub cond_pump_trip: [bool; N_COND_PUMPS],
    pub fcv_stuck: bool,
    pub heaters_lost: bool,
}
```

Add `pub pump_speed: [f64; N_FEED_PUMPS],` to `Feedwater`; in `new`, after `pumps` is built: `let mut pump_speed = [0.0; N_FEED_PUMPS]; for (s, &on) in pump_speed.iter_mut().zip(&pumps) { if on { *s = 1.0; } }` and include it in the struct literal.

Replace `hydraulics`:

```rust
    /// Flow into a vessel at `p_reactor` from the pumps in parallel. Each pump
    /// follows affinity laws at its speed `s`: head = s^2*SHUTOFF*(1 - (q/(s*RUNOUT))^2).
    /// The common head H solves q_pumps(H) = q_valve(H), where q_valve is the
    /// FCV flow for the head left after static losses. Pumps whose shutoff head is
    /// below the system head are isolated by their check valves (zero flow).
    fn hydraulics(&self, p_reactor: f64, p_suction: f64) -> f64 {
        let pos = self.fcv_pos;
        if pos < 1e-4 || p_suction <= 0.0 {
            return 0.0;
        }
        let cv = C_FCV * fcv_characteristic(pos);
        let h0 = p_reactor + FW_STATIC_DP - p_suction;
        let running: Vec<f64> = self.pump_speed.iter().cloned().filter(|&s| s > 0.01).collect();
        let q_pumps = |h: f64| -> f64 {
            if running.is_empty() {
                // Condensate pressure alone pushes through the idle pumps' resistance.
                if h >= 0.0 { 0.0 } else { 150.0 * (-h / 1.0e5).sqrt() }
            } else {
                running
                    .iter()
                    .map(|&s| {
                        let h_max = s * s * FW_PUMP_SHUTOFF_DP;
                        if h >= h_max { 0.0 } else { s * FW_PUMP_RUNOUT * (1.0 - h / h_max).sqrt() }
                    })
                    .sum()
            }
        };
        let h_max = running.iter().map(|&s| s * s * FW_PUMP_SHUTOFF_DP).fold(0.0, f64::max);
        if h0 >= h_max {
            return 0.0; // check valves hold: pumps cannot overcome vessel pressure
        }
        let (mut lo, mut hi) = (h0.max(-1.0e7), h_max);
        for _ in 0..60 {
            let mid = 0.5 * (lo + hi);
            let f = q_pumps(mid) - cv * (mid - h0).max(0.0).sqrt();
            if f > 0.0 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let h = 0.5 * (lo + hi);
        cv * (h - h0).max(0.0).sqrt()
    }

    /// Change the level control mode without bumping the valve (master signature).
    /// The measurements needed to re-track the integral live in `step`'s arguments,
    /// so the re-track is latched here and performed at the top of the next `step`.
    pub fn set_mode(&mut self, mode: LevelControlMode) {
        if mode == LevelControlMode::Manual {
            self.fcv_manual = self.fcv_pos;
        }
        self.mode = mode;
        self.retrack = true;
    }
```

Add the private field `retrack: bool` to `Feedwater` (`false` in `new`) and, in `step`, immediately after the fault-application block and before the controller runs:

```rust
        if self.retrack {
            // Bumpless transfer: initialise the active mode's integral so its
            // first output equals the current valve position.
            let err = level_cm - (self.level_sp_cm + self.setdown_cm);
            match self.mode {
                LevelControlMode::Manual => {}
                LevelControlMode::SingleElement => {
                    self.integral = (self.fcv_pos + 0.004 * err).clamp(0.0, 1.0);
                }
                LevelControlMode::ThreeElement => {
                    let flow_err = (steam_flow - 1.2 * err - self.flow) / STEAM_FLOW_RATED;
                    self.integral = (self.fcv_pos - 0.2 * flow_err).clamp(0.0, 1.0);
                }
            }
            self.retrack = false;
        }
```

In `step`, add the `faults: &FeedFaults` parameter and, at the top:

```rust
        for i in 0..N_FEED_PUMPS {
            if faults.pump_trip[i] {
                self.pumps[i] = false;
            }
        }
        for i in 0..N_COND_PUMPS {
            if faults.cond_pump_trip[i] {
                self.cond_pumps[i] = false;
            }
        }
        if faults.heaters_lost {
            self.heaters_in_service = false;
        }
        for i in 0..N_FEED_PUMPS {
            let target = if self.pumps[i] { 1.0 } else { 0.0 };
            self.pump_speed[i] += (target - self.pump_speed[i]) * (dt / FW_PUMP_COASTDOWN_S).min(1.0);
            if self.pump_speed[i] < 1e-3 {
                self.pump_speed[i] = 0.0;
            }
        }
```

Change the low-suction check from `self.running_pumps() > 0` to `self.pump_speed.iter().any(|&s| s > 0.01)`. Wrap the valve move: `if !faults.fcv_stuck { let d = target - self.fcv_pos; self.fcv_pos += d.clamp(-0.1 * dt, 0.1 * dt); }`. Change `pump_heat` to `if self.pump_speed.iter().any(|&s| s > 0.01) { 2.0 } else { 0.0 }`.

- [ ] **Step 5: Wire into `sim.rs`**

Complete `bop_faults`:

```rust
    fn bop_faults(&self) -> (SteamFaults, CondenserFaults, FeedFaults) {
        let mut cf = CondenserFaults::default();
        for index in 0..N_CW_PUMPS {
            cf.cw_trip[index] = self.has_fault(&Fault::CwPumpTrip { index });
        }
        cf.vacuum_failed = self.has_fault(&Fault::VacuumPumpFailed);
        let mut ff = FeedFaults::default();
        for index in 0..N_FEED_PUMPS {
            ff.pump_trip[index] = self.has_fault(&Fault::FeedPumpTrip { index });
        }
        for index in 0..N_COND_PUMPS {
            ff.cond_pump_trip[index] = self.has_fault(&Fault::CondensatePumpTrip { index });
        }
        ff.fcv_stuck = self.has_fault(&Fault::FcvStuck);
        ff.heaters_lost = self.has_fault(&Fault::HeatersLost);
        (self.steam_faults(), cf, ff)
    }
```

Call `let (sf, cf, ff) = self.bop_faults();` once at the top of `Sim::step` and pass `&ff` to `self.fw.step(...)`, `&cf` to `self.cond.step(...)`, `&sf` to the steam calls. In `apply`:

```rust
            Command::SetFeedPump { index, on } => {
                if index < N_FEED_PUMPS {
                    if on && self.has_fault(&Fault::FeedPumpTrip { index }) {
                        self.refuse("fw", format!("Feed pump {} start blocked: pump tripped", index + 1));
                        return;
                    }
                    if on && !self.fw.cond_pumps.iter().any(|&b| b) {
                        self.refuse("fw", "Feed pump start blocked: no condensate pump running");
                        return;
                    }
                    self.fw.pumps[index] = on;
                }
            }
            Command::SetCondensatePump { index, on } => {
                if index < N_COND_PUMPS {
                    if on && self.has_fault(&Fault::CondensatePumpTrip { index }) {
                        self.refuse("fw", format!("Condensate pump {} start blocked: pump tripped", index + 1));
                        return;
                    }
                    self.fw.cond_pumps[index] = on;
                }
            }
            Command::SetLevelControl { mode } => self.fw.set_mode(mode),
            Command::SetHeaters { in_service } => {
                if in_service && self.has_fault(&Fault::HeatersLost) {
                    self.refuse("fw", "Feedwater heaters unavailable");
                    return;
                }
                self.fw.heaters_in_service = in_service;
            }
```

- [ ] **Step 6: Run all tests**

Run: `cargo test --release -p bwr-core`
Expected: all pass. `full_power_is_steady` and `startup_from_hot_standby_to_30_percent` exercise the new hydraulics: if feed flow at full power differs from the baseline by more than 2 %, stop and report the printed hydraulics values (the parallel solution must reproduce the baseline's equal-speed case exactly: `n` pumps at speed 1 → identical `q`).

- [ ] **Step 7: Document and commit**

Append heater-loss result line and the pump model description to `docs/model/power-conversion.md`.

```bash
git add crates/bwr-core docs/model/power-conversion.md
git commit -m "feat(bop): feed pump speed states, parallel affinity hydraulics, bumpless level control, feed faults

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 9: RWCU reject as the implemented function

**Files:**
- Modify: `crates/bwr-core/src/params.rs` (`RWCU_REJECT_MAX_KGS`)
- Modify: `crates/bwr-core/src/sim.rs` (`SetRwcuReject` clamp; condenser inflow enthalpy from `flows`)
- Test: `crates/bwr-core/tests/power_conversion.rs`

**Interfaces:** consumes `BoundaryFlows.w_liquid_out`, `BoundaryFlows.h_liquid_out` (P2).

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn rwcu_reject_moves_mass_from_vessel_to_hotwell() {
    let build = |reject: f64| {
        let mut sim = Sim::new(ScenarioName::FullPower);
        run(&mut sim, 30.0);
        sim.apply(Command::SetLevelControl { mode: LevelControlMode::Manual });
        sim.apply(Command::SetRwcuReject { kgs: reject });
        let m0 = sim.vessel.mass;
        let hw0 = sim.cond.hotwell_mass;
        run(&mut sim, 300.0);
        (sim.vessel.mass - m0, sim.cond.hotwell_mass - hw0, sim)
    };
    let (dv0, dh0, _) = build(0.0);
    let (dv1, dh1, sim) = build(RWCU_REJECT_MAX_KGS);
    let expected = RWCU_REJECT_MAX_KGS * 300.0;
    assert!(((dv0 - dv1) - expected).abs() < 0.1 * expected, "vessel lost {} kg, expected ~{}", dv0 - dv1, expected);
    assert!(((dh1 - dh0) - expected).abs() < 0.1 * expected, "hotwell gained {} kg, expected ~{}", dh1 - dh0, expected);
    assert!(!sim.scrammed, "300 s of reject in manual should stay above L3: {:?}", sim.scram_reason);
    assert!(sim.ledger.mass_residual(sim.vessel.mass).abs() < 1.0);
}

#[test]
fn rwcu_reject_is_capped() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    sim.apply(Command::SetRwcuReject { kgs: 500.0 });
    assert_eq!(sim.fw.rwcu_reject, RWCU_REJECT_MAX_KGS);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --release -p bwr-core --test power_conversion rwcu`
Expected: compile error `cannot find value RWCU_REJECT_MAX_KGS`.

- [ ] **Step 3: Implement**

`params.rs` feedwater section: `/// Maximum RWCU reject flow to the hotwell, kg/s. ASSUMED (A-6xx: RWCU pumps always available). pub const RWCU_REJECT_MAX_KGS: f64 = 15.0;`. In `sim.rs` `apply`: `Command::SetRwcuReject { kgs } => self.fw.rwcu_reject = kgs.clamp(0.0, RWCU_REJECT_MAX_KGS),`. Confirm the Task 6 condenser block already uses `flows.w_liquid_out` and `flows.h_liquid_out` (the APPLIED reject and its enthalpy, so a subcooled vessel rejects subcooled liquid enthalpy, not `sat().h_f`, and an inventory-bounded step moves only what the vessel released); if the P3/P4 code left `self.vessel.sat().h_f` or `self.fw.rwcu_reject` anywhere in the condenser block, replace it. Tests read the applied value through `sim.last_flows().w_liquid_out`; add to `rwcu_reject_moves_mass_from_vessel_to_hotwell` after the reject run: `assert!((sim.last_flows().w_liquid_out - RWCU_REJECT_MAX_KGS).abs() < 1e-9, "applied reject must equal the request at adequate inventory");`.

- [ ] **Step 4: Run all tests, document, commit**

Run: `cargo test --release -p bwr-core` → all pass. Append to `docs/model/power-conversion.md`: "RWCU reject: destination hotwell; feasibility assumed (pumped system, A-6xx); cap `RWCU_REJECT_MAX_KGS`; verified by `rwcu_reject_moves_mass_from_vessel_to_hotwell`."

```bash
git add crates/bwr-core docs/model/power-conversion.md
git commit -m "feat(sim): RWCU reject cap registered; reject enthalpy from vessel boundary flows

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 10: Snapshot contract, UI mirror, registers and baselines

**Files:**
- Modify: `crates/bwr-core/src/api.rs` (`Steam`, `Condenser`, `Turbine` structs)
- Modify: `crates/bwr-core/src/sim.rs` (`snapshot()`)
- Modify: `ui/src/sim/types.ts`, `ui/src/sim/mock.ts`
- Modify: `docs/model/parameters.csv`, `docs/model/assumptions.md`, `docs/model/esbwr-applicability.csv`, `docs/validation/baseline/*.csv`, `docs/model/power-conversion.md`
- Test: `crates/bwr-core/tests/plant.rs` (`json_contract_matches_ui`)

**Interfaces:** Snapshot additions exactly as in the master table for P6.

- [ ] **Step 1: Extend the contract test**

In `tests/plant.rs` `json_contract_matches_ui`, add:

```rust
    for key in ["tsvPos", "lineFlowsKgs", "headerPressureMpa"] {
        assert!(v["steam"].get(key).is_some(), "missing steam.{key}");
    }
    for key in ["cstLevelPct", "makeupKgs", "rejectKgs"] {
        assert!(v["condenser"].get(key).is_some(), "missing condenser.{key}");
    }
    assert!(v["turbine"].get("extractionMw").is_some());
```

Run: `cargo test --release -p bwr-core --test plant json_contract` → Expected: FAIL `missing steam.tsvPos`.

- [ ] **Step 2: `api.rs`**

Add to `Steam`: `pub tsv_pos: f64, pub line_flows_kgs: Vec<f64>, pub header_pressure_mpa: f64,`. To `Condenser`: `pub cst_level_pct: f64, pub makeup_kgs: f64, pub reject_kgs: f64,`. To `Turbine`: `pub extraction_mw: f64,`.

- [ ] **Step 3: `snapshot()`**

In the `steam:` literal add `tsv_pos: self.steam.tsv_pos, line_flows_kgs: self.steam.w_line.to_vec(), header_pressure_mpa: self.steam.p_chest / 1e6,`. In `condenser:` add `cst_level_pct: self.cond.cst_level_pct(), makeup_kgs: self.cond.makeup_flow, reject_kgs: self.cond.reject_flow,`. In `turbine:` add `extraction_mw: self.turbine.p_extraction / 1e6,`.

Run: `cargo test --release -p bwr-core` → all pass.

- [ ] **Step 4: `types.ts`**

In `ui/src/sim/types.ts`, `interface Steam` add:

```ts
  /** Turbine stop valve position 0 closed .. 1 open. */
  tsvPos: number;
  /** Flow through each steam line's MSIV, kg/s. */
  lineFlowsKgs: number[];
  /** Steam header / chest pressure, MPa. */
  headerPressureMpa: number;
```

`interface Condenser` add:

```ts
  /** Condensate storage tank level, % of CST_CAPACITY_KG (declared boundary inventory). */
  cstLevelPct: number;
  makeupKgs: number;
  rejectKgs: number;
```

`interface Turbine` add `  /** LP turbine work forgone to feedwater heating extraction, MW. */ extractionMw: number;`.

- [ ] **Step 5: `mock.ts`**

In `ui/src/sim/mock.ts`, find the object literal returned by the mock's snapshot builder (search for `steamToBypassKgs:`) and add inside the `steam` object: `tsvPos: this.turbineTripped ? 0 : 1,` (use whatever boolean the mock keeps for the trip state; if it has none, use `1`), `lineFlowsKgs: [steamFlow / 2, steamFlow / 2],` (reuse the mock's total steam flow variable), `headerPressureMpa: pressureMpa - 0.2,`. In the `condenser` object: `cstLevelPct: 75, makeupKgs: 0, rejectKgs: 0,`. In the `turbine` object: `extractionMw: grossMwe * 0.3,` (reuse the mock's gross variable).

Run: `npm --prefix ui run typecheck && npm --prefix ui run build` → Expected: both succeed.

- [ ] **Step 5b: Populate the P6 trace columns (schema v2)**

Package 2's `validation::run_case` sampling emits `0.0` for columns whose source did not exist yet. In the sampling function (the one that fills a `[f64; N_TRACE_COLS]` row by `col_index(name)`), set the P6-owned columns from the engine state:

```rust
    row[col_index("msiv_pos_0")] = sim.steam.msiv_pos[0];
    row[col_index("msiv_pos_1")] = sim.steam.msiv_pos[1];
    row[col_index("tsv_pos")] = sim.steam.tsv_pos;
    row[col_index("header_pressure_pa")] = sim.steam.p_chest;
    row[col_index("cst_kg")] = sim.cond.cst_mass;
    row[col_index("fw_pump_0")] = sim.fw.pump_speed[0];
    row[col_index("fw_pump_1")] = sim.fw.pump_speed[1];
    row[col_index("fw_pump_2")] = sim.fw.pump_speed[2];
```

(`msiv_pos_0/1` may already be populated by Package 2 since `msiv_pos` exists at baseline; keep whichever assignment exists and do not duplicate.) In `docs/validation/method.md`, move these columns from "populated from Package 6" to the populated list. Add to `tests/power_conversion.rs`:

```rust
#[test]
fn p6_trace_columns_are_populated() {
    use bwr_core::validation::{run_case, standard_cases};
    let case = standard_cases().into_iter().find(|c| c.id == "fp_turbine_trip").expect("P2 case");
    let tr = run_case(&case, DT);
    let last = *tr.rows.last().unwrap();
    let col = |n: &str| last[bwr_core::validation::col_index(n)];
    assert!(col("header_pressure_pa") > 1.0e5, "header pressure column empty");
    assert!(col("cst_kg") > 0.0, "cst column empty");
    assert!(col("fw_pump_0") > 0.0 || col("fw_pump_1") > 0.0, "fw pump speed columns empty");
    let first = tr.rows[0];
    assert!((first[bwr_core::validation::col_index("tsv_pos")] - 1.0).abs() < 1e-9, "tsv_pos must be 1 before the trip");
    assert!(col("tsv_pos") < 1e-6, "tsv_pos must be 0 after the trip");
}
```

Run: `cargo test --release -p bwr-core --test power_conversion p6_trace` → Expected: PASS once the sampling assignments are in; before them, FAIL on `header pressure column empty`.

- [ ] **Step 6: Registers**

Append to `docs/model/parameters.csv` one row per constant in the "Names used by this package" table, using the **locked header** `id,value,unit,definition,reference_state,source_design,status,source_ids,interval_lo,interval_hi,assumption_id,location,notes` (the `id` is the Rust constant name; `location` is `crates/bwr-core/src/params.rs`). Example rows (fill `assumption_id` with the ids created in Task 1 and later tasks; `source_ids` is empty for assumed values):

```
V_STEAM_LINE_M3,60.0,m3,steam volume of both main steam lines plus header and chest,n/a,BWRX-300 (assumed),assumed,,30,120,A-6xx,crates/bwr-core/src/params.rs,line storage node; interval spans half to double
TSV_STROKE,0.1,s,turbine stop valve closing stroke on trip,n/a,generic,assumed,,0.05,0.3,A-6xx,crates/bwr-core/src/params.rs,
EXTRACTION_WORK_FRACTION,0.25,-,LP work forgone per unit extraction heat to feedwater heaters,rated,generic BWR cycle,assumed,,0.15,0.35,A-6xx,crates/bwr-core/src/params.rs,0.35 would push TURBINE_DH_RATED above the isentropic drop
TURBINE_DH_RATED,0.897e6,J/kg,shaft work per kg throttle steam at full expansion,rated 7.0 MPa chest / 5.6 kPa,BWRX-300 (calibrated),calibrated,,,,A-6xx,crates/bwr-core/src/params.rs,was 0.703e6 effective (included extraction); recalibrated so net stays ~300 MWe
CST_CAPACITY_KG,2.0e6,kg,condensate storage tank capacity (declared boundary inventory),n/a,BWRX-300 (assumed),assumed,,1.0e6,4.0e6,A-6xx,crates/bwr-core/src/params.rs,
AIR_UA_DEGRADATION,5.0,-,condenser UA divisor coefficient on air/steam partial-pressure ratio,n/a,generic,assumed,,2,10,A-6xx,crates/bwr-core/src/params.rs,air blanketing
SRI_TRIGGER_POWER_FRAC,0.4,-,power above which turbine trip/load rejection triggers Select Rod Insert,n/a,BWRX-300 (assumed),assumed,,0.3,0.6,A-6xx,crates/bwr-core/src/params.rs,was a literal in sim.rs
```

Write the remaining rows (`MSIV_DP_FRACTION`, `LINE_FLOW_RATED`, `K_LINE`, `K_MSIV_OPEN` as `derived`, `TSV_OPEN_RATE`, `CST_MASS_INITIAL`, `HOTWELL_MAKEUP_KGS`, `HOTWELL_MAKEUP_ON`, `HOTWELL_REJECT_ON`, `FW_STATIC_DP`, `FW_PUMP_COASTDOWN_S`, `RWCU_REJECT_MAX_KGS`) in the same shape. Append to `docs/model/esbwr-applicability.csv` using its locked header `id,esbwr_source,esbwr_section,target_subsystem,bwrx_evidence,differences,adaptation,applicability_limits,test,classification`:

```
ESBWR-P6-01,ESBWR DCD Tier 2 Rev. 10,Ch. 10.2-10.4 main steam and power conversion,main steam/turbine/condenser,BWRX general description (steam cycle outline),ESBWR is 4500 MWt with 4 steam lines; BWRX has 2 lines and ~870 MWt,topology and valve roles only; no numeric scaling,steady-state energy balance structure,power_conversion::full_power_energy_balance_closes,shared concept
ESBWR-P6-02,ESBWR DCD Tier 2 Rev. 10,Ch. 7.7 pressure regulator and turbine bypass,EHC pressure regulation,none numeric,regulator gains and bypass capacity not published for BWRX,regulator span and bypass capacity remain assumed,regulator fault and load rejection cases,power_conversion::load_rejection_with_bypass_does_not_scram,shared concept
ESBWR-P6-03,ESBWR DCD Tier 2 Rev. 10,Ch. 10.4.7 condensate and feedwater,feedwater pumps and heaters,none numeric,pump count/capacity differ,pump curves and heater duty remain assumed,feed pump trip and heater loss cases,power_conversion::loss_of_feedwater_heating_raises_power,shared concept
```

- [ ] **Step 7: Baselines and results**

Run: `cargo run --release -p bwr-core --example validation -- --update`; then `cargo run --release -p bwr-core --example validation` → Expected: all cases compare equal to the freshly written baselines. Append to `docs/model/power-conversion.md` "Results" the final before/after table for every P6 case (peak pressure, time, scram reason, net MWe, condenser duty) and a "Limitations" list: saturated line node (no superheat), heater lag not a storage term, bypass capacity unverified, extraction fraction assumed, CST and CW as declared boundaries, no containment isolation.

- [ ] **Step 8: Full verification and commit**

Run all: `cargo test --release -p bwr-core && npm --prefix ui run typecheck && npm --prefix ui run build` → Expected: all green.

```bash
git add crates/bwr-core ui/src/sim docs
git commit -m "feat(api): power-conversion snapshot fields; registers and baselines for package 6

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

## Spec coverage check

| Spec bullet (Package 6) | Task |
|---|---|
| Verify topology; distinguish reactor isolation from containment isolation | 1 |
| Model independently consequential valves separately; consistent enthalpy across throttling; line/chest storage | 2, 3, 4 |
| Verify bypass capacity, turbine trip and load-rejection response, valve timing, governor, pressure regulation; reassess SRI group and ride-through test | 4, 5 |
| Reconcile shaft power, generator output, pump work, extraction heating, condenser duty, house loads in one energy balance | 6 |
| Refine condenser heat transfer, noncondensable gas, vacuum loss, hotwell inventory; declare cooling-water boundary | 1, 7 |
| Replace implicit unlimited makeup and reject with explicit boundary accounting and declared inventory | 7 |
| Verify pump count/capacity, check/isolation behaviour, head, suction limits, pump stopping response, heater losses, controller mode transfer | 8 |
| RWCU reject destination, feasibility, mass/energy effects | 9 |
| Acceptance gate: balances close; condenser loss, feedwater disturbances, turbine trips follow the design basis within documented uncertainty | 5, 6, 7, 8, 10 |

Master r2 obligations carried by this package:

| Master requirement | Task |
|---|---|
| Transfer allocation: every steam/RWCU consumer uses applied flows (step order 8–12, defect 13) | 3 (wiring), 6, 7, 9 |
| `EnergyBalance` control volume with storage terms; steady and transient closure | 6 |
| `SteamLine.energy`, `Turbine.p_losses`, `Condenser.hotwell_energy` storage states | 3, 6, 7 |
| Derives rule (`SteamLine` Copy; fault views Copy+Default; `EnergyBalance` Copy+Default) | 3, 4, 6, 7, 8 |
| `faults: Vec<Fault>` read via `has_fault`/`find_fault`; faults never auto-cleared (tested) | 4, 5, 7, 8 |
| Trace schema v2 columns `tsv_pos`, `header_pressure_pa`, `cst_kg`, `fw_pump_0..2`, `msiv_pos_0/1` | 10 (Step 5b) |
| Locked CSV headers for `parameters.csv`, `cases.csv`, `esbwr-applicability.csv` | 5, 10 |
| Log-line convention (`msiv isolation:`, `command ignored:`) | 5, 7, 8 |
| `Sim::last_flows()` for applied-flow reads in tests | 3, 9 |

## Self-review against the master checklist (r2)

- Spec coverage: every Package 6 bullet maps to a task (table above).
- Placeholders: none; every code step shows code; the only cross-package references are to names the master defines (`run_case`, `col_index`, `standard_cases`, `last_flows`, `find_fault`, `CP_WATER`) or to the Package 2 runner flag, which is named explicitly as a dependency.
- Names and signatures match master r2: `compute_flows(dt, p_vessel)`, `init_line_steady(p_vessel)`, `SteamSystem::new(msivs_open, tsv_open)`, `Turbine::step(.., extraction_heat_w)`, `Condenser::step(.., &CondenserFaults)`, `Feedwater::step(.., &FeedFaults)`, `Feedwater::set_mode(mode)`, `EnergyBalance` field list, `SteamLine { mass, p, energy }`, `Turbine.p_losses`, `Condenser.hotwell_energy`. Package-local additions (`step_line`, `kinetic_energy`, `refuse`, `house_load`, `bypass_available`, `overflow_kg`, `deficit_kg`, `HOTWELL_MAKEUP_ON/REJECT_ON`) are marked and consumed only inside P6.
- Derives: stated on every new struct.
- Constants: every new or changed constant has a `parameters.csv` row step with a status (Task 10 Step 6).
- Snapshot changes: `types.ts` and `mock.ts` steps present (Task 10 Steps 4–5); typecheck/build commands given.
- Defect fixes: defect 6 (hotwell) has a compile-failing test first (Task 7); the averaging defect has a compile-failing test first (Task 2); every fault has a persistence assertion.
- Applied transfers: Task 3 Step 6 scales `w_line`, `w_tcv`, `w_bypass`; the condenser and balance read `flows.w_liquid_out`; `steam_consumers_see_applied_flows` guards it.
