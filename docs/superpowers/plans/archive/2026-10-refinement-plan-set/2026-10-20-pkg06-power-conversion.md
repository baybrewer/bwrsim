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
- Every new or changed constant gets its `docs/model/parameters.csv` row (locked header, status `assumed`, `derived`, `calibrated` or `confirmed`) **in the same task that adds or changes the constant** (master register-row rule), so P1's `tests/register.rs` passes at every task boundary. Task 10 only verifies the register; it adds no rows.
- Master dt rule: every public step function in `bop.rs` validates `dt` and its numeric arguments before touching state (Task 2a); void-returning steps report through `last_error`, which `Sim::step` drains into `domain_errors` at master step 17.
- Master baseline regeneration protocol: compare first and save the report to `docs/validation/reports/2026-10-21-pkg06.md`, classify every changed metric, pass the package gates, then `--write` once per reviewed change set, and commit the report with the baselines. Tolerances are never adjusted after a failure.
- Run `cargo test --release -p bwr-core` after every task; all pre-existing tests stay green except where a task says a baseline assertion moves to `cases.csv`.

**Acceptance gate (verbatim from spec):** "Steady and transient power, water, and heat balances close. Condenser loss, feedwater disturbances, and turbine trips follow the selected design basis within documented uncertainty."

---

## Names used by this package

Constants `MSIV_DP_FRACTION` through `RWCU_REJECT_MAX_KGS`, the P6 struct fields (`SteamLine { mass, energy, p, q_wall_w }`, `Turbine.p_extraction`/`p_losses` with SIGNED `p_elec`, `Condenser { energy, q_in_w, q_cw_w, cst_mass, makeup_flow, reject_flow, makeup_h, reject_h, overflow_kg, deficit_kg }`, `Feedwater { pump_speed, heater_energy, q_extraction_applied_w, pump_work_w }`), `DomainError::HotwellExhausted { deficit_kg }` (added to `domain.rs` by this package), the fault views, and the P6 signatures are now **in the master contract (r2, amended 2026-10-21 for the cycle-2 review)**; the table below restates them for implementers and marks the few names that remain package-local (allowed: nothing outside P6 consumes them). The master wins on any conflict with this plan.

| Name | Where | Value / meaning |
|---|---|---|
| `MSIV_DP_FRACTION` | `params.rs` | `0.10` — fraction of rated line dp across a fully open MSIV. ASSUMED (interval 0.05–0.25); `LINE_FLOW_RATED`, `K_LINE`, `K_MSIV_OPEN` are the DERIVED dependents |
| `K_LINE` | `params.rs` | `(1-MSIV_DP_FRACTION)*STEAM_LINE_DP/(STEAM_FLOW_RATED/2)^2` Pa/(kg/s)^2. Derived |
| `K_MSIV_OPEN` | `params.rs` | `MSIV_DP_FRACTION*STEAM_LINE_DP/(STEAM_FLOW_RATED/2)^2`. Derived |
| `V_STEAM_LINE_M3` | `params.rs` | `60.0` m3, both lines + header + chest. ASSUMED |
| `TSV_STROKE` | `params.rs` | `0.1` s closing stroke on trip. ASSUMED |
| `TSV_OPEN_RATE` | `params.rs` | `1.0` per second on reset. ASSUMED |
| `EXTRACTION_WORK_FRACTION` | `params.rs` | `0.25` W of LP work forgone per W of extraction heating. ASSUMED, interval 0.15–0.35 |
| `EXTRACTION_FLOW_FRACTION_MAX` | `params.rs` | `0.35` largest fraction of the turbine steam available for extraction heating: the steam-side bound on `q_extraction_applied_w`. ASSUMED, interval 0.25–0.45. Package-local |
| `TURBINE_DH_RATED` | `params.rs` | changed `0.703e6` → `0.897e6` J/kg. CALIBRATED (see Task 6); its existing register row is UPDATED, not duplicated |
| `FW_HEATER_CAPACITY_J_PER_K` | `params.rs` | `FW_HEATER_TAU_S * STEAM_FLOW_RATED * 4186` = 3.54e8 J/K thermal capacity of the feedwater heater train (metal, shells, drains). DERIVED (Task 6); interval 1e8–1e9. Package-local |
| `FW_HEATER_TAU_S` | `params.rs` | `180.0` s heater response time (was a literal). ASSUMED. Package-local |
| `CONDENSER_SHELL_CAPACITY_J_PER_K` | `params.rs` | `5.0e7` J/K tube-bundle and shell metal capacity lumped with the hotwell water. ASSUMED, interval 2e7–2e8. Package-local |
| `AIR_UA_DEGRADATION` | `params.rs` | `5.0` — UA divisor coefficient on air/steam partial pressure ratio. ASSUMED |
| `CST_CAPACITY_KG` | `params.rs` | `2.0e6` kg. ASSUMED |
| `CST_MASS_INITIAL` | `params.rs` | `1.5e6` kg. ASSUMED |
| `HOTWELL_MAKEUP_KGS` | `params.rs` | `30.0` kg/s (was a literal). ASSUMED |
| `HOTWELL_MAKEUP_ON` / `HOTWELL_REJECT_ON` | `params.rs` | `0.40` / `0.85` hotwell fraction (were literals). ASSUMED. Package-local |
| `FW_STATIC_DP` | `params.rs` | `0.15e6` Pa (was a literal). ASSUMED |
| `FW_PUMP_COASTDOWN_S` | `params.rs` | `5.0` s first-order pump speed time constant. ASSUMED |
| `SRI_TRIGGER_POWER_FRAC` | `params.rs` | `0.4` (was a literal in `sim.rs`). ASSUMED, interval 0.3–0.6 (Task 5a sensitivity) |
| `BYPASS_CAPACITY` | `params.rs` | existing `1.0` x rated steam flow. Status changed to `assumed` with interval 0.25–1.1 and a Task 5a sensitivity run; nominal value is NOT confirmed |
| `RWCU_REJECT_MAX_KGS` | `params.rs` | `15.0` kg/s (was a literal). ASSUMED |
| `line_flow(dp, pos)` | `bop.rs` | per-line flow through friction + MSIV (series resistance, master r2) |
| `SteamFaults`, `CondenserFaults`, `FeedFaults` | `bop.rs` | fault views; all derive `Clone, Copy, Debug, Default` |
| `SteamSystem::vessel_steam_flow()` | `bop.rs` | `w_line.iter().sum()` — the vessel's REQUESTED steam outflow |
| `SteamSystem::init_line_steady(p_vessel)` | `bop.rs` | sets `line` to the quasi-steady header state AND `p_chest = line.p` on every branch (scenario build; tested for all three scenarios before the first step) |
| `SteamLine::at_pressure(p)` | `bop.rs` | storage node constructor (scenario build only); `SteamLine` derives `Clone, Copy, Debug`; at construction the node holds saturated vapour at `p` (its `energy` is that mass's saturated internal energy) and `q_wall_w = 0`; afterwards `energy` is INTEGRATED from the applied enthalpy flows minus the explicit `q_wall_w` (master) |
| `SteamSystem::step_line(dt, w_in_applied, h_in) -> Result<(), DomainError>` | `bop.rs` | commits the applied inflow, re-solves and bounds the node's own outflow (`w_tcv`, `w_bypass`) against inventory plus applied inflow, integrates energy, sets `q_wall_w`; Err (transactional, node unchanged) outside the registered property bounds. Package-local name |
| `line_p_bounds()` | `bop.rs` | `(sat_t(T_TABLE_MIN).p, sat_t(T_TABLE_MAX).p)`: the registered steam-table domain (P4) used as the solve bracket. Package-local |
| `steam::sat_rho_g(rho) -> Result<Sat, DomainError>` | `steam.rs` | inverse of `rho_g` on the table; Err outside the table's density column (no silent clamp) |
| `check_step_args(dt, &[(name, value)])` | `bop.rs` | master dt rule + non-finite rule guard for every public step function; `pub(crate)`. Package-local |
| `SteamSystem.last_error`, `Turbine.last_error`, `Condenser.last_error`, `Feedwater.last_error` | `bop.rs` | `Option<DomainError>` set by a rejected step (state unchanged); drained by `Sim::step` at master step 17 |
| `Feedwater::set_mode(mode)` | `bop.rs` | bumpless transfer (master signature); re-tracks the integral on the next `step()` |
| `Turbine::kinetic_energy()` | `bop.rs` | shaft kinetic energy, J; the master's `d/dt(shaft kinetic energy)` storage term. Package-local name |
| `Feedwater::step_heaters(dt, w_feed_applied, w_turbine_applied, p_chest, hotwell_t)` | `bop.rs` | heater energy node (master step 11, before `Turbine::step`): sets `q_extraction_applied_w` (bounded by steam-side availability), `pump_work_w`, advances `heater_energy` and `t_feed`. Package-local |
| `Feedwater::hydraulic_solution(p_reactor, p_suction) -> (flow, head)` | `bop.rs` | public parallel-pump solve (replaces the private `hydraulics`), used by `step` and by the component tests; `pump_flow(speed, head)` is the single-pump affinity curve it sums. Package-local |
| `Feedwater::step(dt, p_reactor, level_cm, steam_flow, hotwell_ok, faults)` | `bop.rs` | final signature: the heater arguments moved to `step_heaters` (Task 6), `faults` added (Task 8); returns the trip detail for the `feed pump trip:` log line |
| `Condenser::set_hotwell_t(t)` | `bop.rs` | consistent `energy`/`t_sat`/`p_steam` initialiser (scenario build only; steps DERIVE the state from `energy`). Package-local |
| `SteamSystem.bypass_capacity`, `Sim.sri_trigger_power_frac`, `Sim::full_power_with_bop(cap, sri)` | `bop.rs`, `sim.rs` | Task 5a sensitivity hooks (defaults are the registered constants). Package-local |
| `Sim::bop_faults()` | `sim.rs` | builds the three fault views from `Sim.faults` via `has_fault`/`find_fault` |
| `Sim::house_load()` | `sim.rs` | extracted from `snapshot()`. Package-local |
| `Sim::bypass_available()` | `sim.rs` | condenser availability AND `!BypassUnavailable`. Package-local |
| `Condenser.overflow_kg`, `Condenser.deficit_kg` | `bop.rs` | master fields: water rejected over a full CST (counted in `reject_flow`/`reject_h`, never discarded) and feed shortfall against hotwell inventory (raised as `HotwellExhausted` at master step 7) |
| `DomainError::HotwellExhausted { deficit_kg }` | `domain.rs` | added by Task 7 (master P3 enum; `reason()` arm `hotwell exhausted: feed request exceeded inventory by <deficit> kg`) |
| `Sim::bound_feed_to_hotwell(dt)` | `sim.rs` | master step 7 bound `w_feed = min(request, hotwell_mass/dt)`; records the deficit and raises `HotwellExhausted`. Package-local |
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
- The lines, header and steam chest are one lumped storage node of volume `V_STEAM_LINE_M3` (A-6xx). Its mass and energy are integrated from the applied enthalpy flows; its pressure is solved from mass on the saturation line. The saturated-state constraint cannot hold the integrated energy exactly, so the mismatch is an explicit boundary term `q_wall_w` (assumed exchange with line metal, A-6xx), reported as `EnergyBalance.q_line_wall_w` and bounded in Task 6. Throttling superheat is neglected.
- The turbine stop valve (TSV) closes in `TSV_STROKE` on any turbine trip and is a separate state from the control valves (TCV).
- Bypass valves are one lumped bank of capacity `BYPASS_CAPACITY` x rated steam flow (status: assumed, interval 0.25–1.1; the spec marks this "to verify" and Task 5a records the evidence search and a sensitivity run; no source confirms the nominal value).
- The turbine is a lumped shaft: gross work `w_tcv * TURBINE_DH_RATED * pf` minus the LP work forgone by extraction steam, `EXTRACTION_WORK_FRACTION * (feedwater heating duty)`.
- The condenser is a single shell with air partial pressure, air-blanketing UA degradation, and a hotwell whose energy is a first-law state: `Condenser.energy` is advanced by the arriving thermal load `q_in_w`, the cooling-water rejection `q_cw_w = m_cw*cp*eff*(T_hw - T_in)` (zero with no CW pump), the condensate withdrawal and the CST exchange recorded at transfer-time enthalpies; temperature and pressure are derived from that energy, never lagged toward a target.
- Feedwater: two condensate pumps (suction pressure boundary `CONDENSATE_PRESSURE`), heaters as a stored-energy node (`heater_energy`, capacity `FW_HEATER_CAPACITY_J_PER_K`) charged by applied extraction heat bounded by steam-side availability and discharged into the feed stream, three 50 % feed pumps with affinity-law speed states, one equal-percentage FCV, manual / single-element / three-element level control. Pump work (`pump_work_w`, the 2 K term) is counted once as fluid work.

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
| Hotwell temperature | derived from `Condenser.energy` and inventory (perfect mixing: condensing steam in equilibrium with the hotwell water) | assumed |
| Auxiliary electrical load | `Sim::house_load()` is drawn from gross electrical power OUTSIDE the control volume; only the fluid pump work `pump_work_w` re-enters it; `house_load - pump_work_w` is motor/pump inefficiency and work done on external streams (CW, vacuum) | assumed |

## Energy balance definition

See Task 6 of the plan; the balance terms and the analytic residual are documented in the "Energy balance" section appended by that task.

## Faults honoured

`MsivStuck`, `TsvStuck`, `TcvStuck`, `BypassUnavailable`, `PressureRegulatorFailed`, `FeedPumpTrip`, `CondensatePumpTrip`, `FcvStuck`, `HeatersLost`, `CwPumpTrip`, `VacuumPumpFailed`. Each is tested in `crates/bwr-core/tests/power_conversion.rs`.

## Results

(appended by Tasks 3, 5, 5a, 6, 7, 8, 10)
```

- [ ] **Step 2: Replace the `A-6xx` / `S-xx` placeholders with real ids**

Open `docs/model/assumptions.md` (Package 1). Append numbered assumptions, continuing the existing sequence, for: MSIV stands in for RPV isolation; CST as boundary inventory; RWCU pumps always available; hotwell perfectly mixed; steam line node saturated with the energy mismatch as an explicit wall exchange (`q_wall_w`); feedwater heater train as a lumped stored-energy node; condenser shell metal lumped with the hotwell water. Use those ids in the document. Use the `sources.csv` id of the GE general description for `S-xx`.

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
- Produces: `pub fn line_flow(dp: f64, pos: f64) -> f64`; `SteamSystem.w_line: [f64; N_STEAM_LINES]`; `SteamSystem::vessel_steam_flow(&self) -> f64`; `SteamSystem.tsv_pos: f64`.
- Signatures are the master's FINAL ones from this task on, so no later task changes them and every test written here compiles at every later boundary: `SteamSystem::new(msivs_open: bool, tsv_open: bool)` and `SteamSystem::compute_flows(&mut self, dt: f64, p_vessel: f64)`. In this task `compute_flows` is still quasi-steady and does not use `dt` (Task 3 makes the solve implicit in `dt`), and `tsv_pos` is a TEMPORARY instantaneous stop valve set by `step` from `tripped` (Task 4 Step 4 replaces it with the stroke dynamics and the `TsvStuck` hold; that replacement is mandatory, not optional).
- Migrates every caller in the same step: `sim.rs` constructor sites and the `compute_flows` call (Step 4a).

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
    let mut a = SteamSystem::new(true, true);
    a.tcv_pos = 0.85;
    a.msiv_pos = [0.0, 1.0];
    a.compute_flows(DT, P_RATED);
    let mut b = SteamSystem::new(true, true);
    b.tcv_pos = 0.85;
    b.msiv_pos = [0.5, 0.5];
    b.compute_flows(DT, P_RATED);
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

- [ ] **Step 3: Add parameters and their register rows**

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

Append to `docs/model/parameters.csv` (locked header `id,value,unit,definition,reference_state,source_design,status,source_ids,interval_lo,interval_hi,assumption_id,location,notes`; `A-6xx` is the MSIV assumption id from Task 1):

```
MSIV_DP_FRACTION,0.10,-,fraction of rated steam-line pressure drop across a fully open MSIV,rated,generic BWR,assumed,,0.05,0.25,A-6xx,crates/bwr-core/src/params.rs,sets the line/valve split; dependents are derived
LINE_FLOW_RATED,235.0,kg/s,rated flow in one steam line (STEAM_FLOW_RATED / N_STEAM_LINES),rated,BWRX-300,derived,,,,,crates/bwr-core/src/params.rs,
K_LINE,,Pa/(kg/s)^2,friction resistance of one steam line excluding its MSIV,rated,BWRX-300,derived,,,,A-6xx,crates/bwr-core/src/params.rs,(1-MSIV_DP_FRACTION)*STEAM_LINE_DP/LINE_FLOW_RATED^2; value column left empty because it is a compile-time expression
K_MSIV_OPEN,,Pa/(kg/s)^2,resistance of one fully open MSIV,rated,BWRX-300,derived,,,,A-6xx,crates/bwr-core/src/params.rs,MSIV_DP_FRACTION*STEAM_LINE_DP/LINE_FLOW_RATED^2; scales with 1/pos^2
```

(If P1's `tests/register.rs` requires a numeric `value`, evaluate the two expressions with the current `STEAM_LINE_DP` and write the numbers; the test defines the rule.)

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
    /// Turbine stop valve, 0 closed .. 1 open (Task 4 adds the stroke dynamics).
    pub tsv_pos: f64,
}

impl SteamSystem {
    pub fn new(msivs_open: bool, tsv_open: bool) -> Self {
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
            tsv_pos: if tsv_open { 1.0 } else { 0.0 },
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
    /// `dt` is accepted now so the signature is the master's final one; Task 3 uses it
    /// for the implicit storage solve.
    pub fn compute_flows(&mut self, dt: f64, p: f64) {
        let _ = dt;
        let a_tcv = C_TCV * self.tcv_pos * self.tsv_pos;
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

Keep `total_flow`, `pressure_demand` and `step` as they are (they follow immediately), except for one TEMPORARY line at the top of `step`, which reproduces the baseline's `tsv_open = !tripped` gating until Task 4 lands:

```rust
    pub fn step(&mut self, dt: f64, p: f64, tcv_flow_demand: f64, bypass_available: bool, tripped: bool) {
        // TEMPORARY (replaced by Task 4 Step 4): instantaneous stop valve.
        self.tsv_pos = if tripped { 0.0 } else { 1.0 };
        // ...existing body unchanged...
```

Delete the now-unused `msiv_open_fraction` if the compiler warns.

- [ ] **Step 4a: Migrate every caller in `sim.rs`**

`SteamSystem::new(true)` in `cold_shutdown` and `hot_standby` becomes `SteamSystem::new(true, false)`; in `full_power` it becomes `SteamSystem::new(true, true)`. The one `compute_flows` call in `Sim::step` (P4 left it as `self.steam.compute_flows(p, !self.turbine.tripped);`) becomes `self.steam.compute_flows(dt, p);` (the trip gating now acts through `tsv_pos`, set in `step` just above). `grep -rn "SteamSystem::new(\|compute_flows(" crates/bwr-core` must show only two-argument calls before Step 5.

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

### Task 2a: Argument guards and error channels for every public BOP step function

**Files:**
- Modify: `crates/bwr-core/src/bop.rs` (`check_step_args`; `last_error` on `SteamSystem`, `Turbine`, `Condenser`, `Feedwater`; a guard as the FIRST statement of every public step function)
- Modify: `crates/bwr-core/src/sim.rs` (`Sim::step` step-17 drain)
- Test: `crates/bwr-core/tests/power_conversion.rs`

**Interfaces:**
- Produces: `pub(crate) fn check_step_args(dt: f64, args: &[(&str, f64)]) -> Result<(), DomainError>` (master dt rule + non-finite inputs rule); `pub last_error: Option<DomainError>` on `SteamSystem`, `Turbine`, `Condenser` and `Feedwater` (the error channel of their void-returning methods, drained by `Sim::step` at master step 17 exactly like `Core.last_error`, `Vessel.last_error` and `IcTrain.last_error` from P3).
- Guarded in this task, with the signatures as they stand after Task 2: `SteamSystem::step(dt, p, tcv_flow_demand, bypass_available, tripped)`, `SteamSystem::compute_flows(dt, p)`, `Turbine::tcv_demand(dt, pressure_demand)`, `Turbine::step(dt, w_turbine, p_chest, p_cond)`, `Condenser::step(dt, q, w_in, w_out)`, `Feedwater::step(dt, p_reactor, level_cm, steam_flow, turbine_flow, hotwell_ok, hotwell_t)`. Every later task that extends one of these argument lists (Tasks 3, 4, 6, 7, 8) extends the guard's table in the same step and keeps the guard as the first statement; `Result`-returning additions (`step_line`, Task 3) propagate the same error with `?`.
- Rejection semantics (master): `dt` not finite or `dt <= 0.0` (zero and negative `dt` are rejected exactly like NaN) → `PropertyOutOfRange { what: "dt", value: dt }`; any other numeric argument not finite → `PropertyOutOfRange { what: <argument name>, value }`. The method returns before touching any field, so the component is identical to its entry state. `tcv_demand` returns `0.0` on rejection (its integrator is untouched). `bool` arguments need no guard.

- [ ] **Step 1: Write the failing tests**

Append to `tests/power_conversion.rs`:

```rust
use bwr_core::bop::{Condenser, Feedwater, LevelControlMode, Turbine};
use bwr_core::domain::DomainError;

const BAD_DT: [f64; 4] = [0.0, -0.05, f64::NAN, f64::INFINITY];
const BAD_VAL: [f64; 3] = [f64::NAN, f64::INFINITY, f64::NEG_INFINITY];

fn rejects(what: &str, e: Option<DomainError>) -> bool {
    matches!(e, Some(DomainError::PropertyOutOfRange { what: w, .. }) if w == what)
}

/// Table-driven rejection: every row is (description, call) and the state snapshot
/// (Debug of the whole component; all fields derive Debug) must be unchanged after it.
#[test]
fn bop_step_functions_reject_invalid_dt_and_non_finite_arguments_unchanged() {
    let mut sys = SteamSystem::new(true, true);
    sys.tcv_pos = 0.85;
    let mut tb = Turbine::new(true);
    let mut cd = Condenser::new(true);
    let mut fw = Feedwater::new(2, T_FW_RATED, LevelControlMode::ThreeElement, 0.7);
    for &dt in &BAD_DT {
        let (s0, t0, c0, f0) = (format!("{sys:?}"), format!("{tb:?}"), format!("{cd:?}"), format!("{fw:?}"));
        sys.step(dt, P_RATED, 300.0, true, false);
        assert!(rejects("dt", sys.last_error.take()), "SteamSystem::step dt {dt}");
        sys.compute_flows(dt, P_RATED);
        assert!(rejects("dt", sys.last_error.take()), "compute_flows dt {dt}");
        assert_eq!(tb.tcv_demand(dt, 300.0), 0.0);
        assert!(rejects("dt", tb.last_error.take()), "tcv_demand dt {dt}");
        tb.step(dt, 400.0, 7.0e6, 5.6e3);
        assert!(rejects("dt", tb.last_error.take()), "Turbine::step dt {dt}");
        cd.step(dt, 5.0e8, 470.0, 470.0);
        assert!(rejects("dt", cd.last_error.take()), "Condenser::step dt {dt}");
        fw.step(dt, P_RATED, 0.0, 470.0, 470.0, true, 35.0);
        assert!(rejects("dt", fw.last_error.take()), "Feedwater::step dt {dt}");
        assert_eq!((format!("{sys:?}"), format!("{tb:?}"), format!("{cd:?}"), format!("{fw:?}")), (s0, t0, c0, f0), "state must be unchanged for dt {dt}");
    }
    for &v in &BAD_VAL {
        let (s0, t0, c0, f0) = (format!("{sys:?}"), format!("{tb:?}"), format!("{cd:?}"), format!("{fw:?}"));
        sys.step(DT, v, 300.0, true, false);
        assert!(rejects("p", sys.last_error.take()));
        sys.step(DT, P_RATED, v, true, false);
        assert!(rejects("tcv_flow_demand", sys.last_error.take()));
        sys.compute_flows(DT, v);
        assert!(rejects("p_vessel", sys.last_error.take()));
        tb.tcv_demand(DT, v);
        assert!(rejects("pressure_demand", tb.last_error.take()));
        tb.step(DT, v, 7.0e6, 5.6e3);
        assert!(rejects("w_turbine", tb.last_error.take()));
        tb.step(DT, 400.0, v, 5.6e3);
        assert!(rejects("p_chest", tb.last_error.take()));
        tb.step(DT, 400.0, 7.0e6, v);
        assert!(rejects("p_cond", tb.last_error.take()));
        cd.step(DT, v, 470.0, 470.0);
        assert!(rejects("q", cd.last_error.take()));
        cd.step(DT, 5.0e8, v, 470.0);
        assert!(rejects("w_in", cd.last_error.take()));
        cd.step(DT, 5.0e8, 470.0, v);
        assert!(rejects("w_out", cd.last_error.take()));
        fw.step(DT, v, 0.0, 470.0, 470.0, true, 35.0);
        assert!(rejects("p_reactor", fw.last_error.take()));
        fw.step(DT, P_RATED, v, 470.0, 470.0, true, 35.0);
        assert!(rejects("level_cm", fw.last_error.take()));
        fw.step(DT, P_RATED, 0.0, v, 470.0, true, 35.0);
        assert!(rejects("steam_flow", fw.last_error.take()));
        fw.step(DT, P_RATED, 0.0, 470.0, v, true, 35.0);
        assert!(rejects("turbine_flow", fw.last_error.take()));
        fw.step(DT, P_RATED, 0.0, 470.0, 470.0, true, v);
        assert!(rejects("hotwell_t", fw.last_error.take()));
        assert_eq!((format!("{sys:?}"), format!("{tb:?}"), format!("{cd:?}"), format!("{fw:?}")), (s0, t0, c0, f0), "state must be unchanged for value {v}");
    }
}

#[test]
fn sim_step_drains_bop_errors_at_step_17_with_the_post_step_time() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    sim.steam.last_error = Some(DomainError::PropertyOutOfRange { what: "probe".into(), value: f64::NAN });
    sim.step(DT);
    let (t, e) = sim.domain_errors.last().expect("drained");
    assert_eq!(*t, sim.time, "stamped with the post-step time (master step 17)");
    assert!(matches!(e, DomainError::PropertyOutOfRange { what, .. } if what == "probe"));
    assert!(sim.steam.last_error.is_none(), "drained, not retained");
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --release -p bwr-core --test power_conversion`
Expected: compile error `no field last_error on type SteamSystem`.

- [ ] **Step 3: Implement the guard and the error channels in `bop.rs`**

Add `use crate::domain::DomainError;` to the imports and, above `pub fn line_flow`:

```rust
/// Master dt rule and non-finite inputs rule for the BOP step functions: `dt` must be
/// finite and positive, every other numeric argument finite. Called before any mutation.
pub(crate) fn check_step_args(dt: f64, args: &[(&str, f64)]) -> Result<(), DomainError> {
    if !(dt.is_finite() && dt > 0.0) {
        return Err(DomainError::PropertyOutOfRange { what: "dt".into(), value: dt });
    }
    for &(name, value) in args {
        if !value.is_finite() {
            return Err(DomainError::PropertyOutOfRange { what: name.into(), value });
        }
    }
    Ok(())
}
```

Add to each of `SteamSystem`, `Turbine`, `Condenser`, `Feedwater` the field

```rust
    /// Error raised by a rejected void-returning step (master dt rule); drained by `Sim::step` at step 17.
    pub last_error: Option<DomainError>,
```

initialised `last_error: None` in each `new`. Insert the guard as the first statement of each method (the argument names are the ones the tests assert):

```rust
    // SteamSystem::step
        if let Err(e) = check_step_args(dt, &[("p", p), ("tcv_flow_demand", tcv_flow_demand)]) {
            self.last_error = Some(e);
            return;
        }
    // SteamSystem::compute_flows
        if let Err(e) = check_step_args(dt, &[("p_vessel", p)]) {
            self.last_error = Some(e);
            return;
        }
    // Turbine::tcv_demand
        if let Err(e) = check_step_args(dt, &[("pressure_demand", pressure_demand)]) {
            self.last_error = Some(e);
            return 0.0;
        }
    // Turbine::step
        if let Err(e) = check_step_args(dt, &[("w_turbine", w_turbine), ("p_chest", p_chest), ("p_cond", p_cond)]) {
            self.last_error = Some(e);
            return;
        }
    // Condenser::step
        if let Err(e) = check_step_args(dt, &[("q", q), ("w_in", w_in), ("w_out", w_out)]) {
            self.last_error = Some(e);
            return;
        }
    // Feedwater::step
        if let Err(e) = check_step_args(
            dt,
            &[("p_reactor", p_reactor), ("level_cm", level_cm), ("steam_flow", steam_flow), ("turbine_flow", turbine_flow), ("hotwell_t", hotwell_t)],
        ) {
            self.last_error = Some(e);
            return None;
        }
```

- [ ] **Step 4: Drain in `sim.rs`**

At master step 17 of `Sim::step` (P3 placed the `core`/`vessel`/`ic` drains there, after `ledger.record`), add:

```rust
        for e in [
            self.steam.last_error.take(),
            self.turbine.last_error.take(),
            self.cond.last_error.take(),
            self.fw.last_error.take(),
        ]
        .into_iter()
        .flatten()
        {
            self.domain_error(e);
        }
```

- [ ] **Step 5: Run all tests**

Run: `cargo test --release -p bwr-core`
Expected: all pass. No baseline changes (the guards never fire in the standard cases: `Sim::step` already validates `dt` and every scenario stays finite).

- [ ] **Step 6: Commit**

```bash
git add crates/bwr-core/src/bop.rs crates/bwr-core/src/sim.rs crates/bwr-core/tests/power_conversion.rs
git commit -m "feat(bop): dt and non-finite argument guards with last_error channels on every BOP step

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---
### Task 3: Steam line and chest storage node

**Files:**
- Modify: `crates/bwr-core/src/steam.rs` (`sat_rho_g` returning `Result`)
- Modify: `crates/bwr-core/src/params.rs` (`V_STEAM_LINE_M3`) and `docs/model/parameters.csv` (its row, same task: register-row rule)
- Modify: `crates/bwr-core/src/bop.rs` (`SteamLine`, `line_p_bounds`, `solve_line_pressure`, `SteamSystem.line`, `compute_flows` becomes the implicit request predictor, `init_line_steady`, `step_line`)
- Modify: `crates/bwr-core/src/sim.rs` (`base`, `full_power`, `step` order 5/8/9/10/11, `isolation_signals`, `snapshot`)
- Modify: `crates/bwr-core/tests/numerics.rs`, `crates/bwr-core/tests/conservation.rs` (P3 identity migration, Step 7)
- Test: `crates/bwr-core/tests/power_conversion.rs`

**Interfaces:**
- Produces (master): `#[derive(Clone, Copy, Debug)] pub struct SteamLine { pub mass: f64, pub energy: f64, pub p: f64, pub q_wall_w: f64 }`; `SteamLine::at_pressure(p) -> Self`; `pub fn line_p_bounds() -> (f64, f64)` (`(sat_t(T_TABLE_MIN).p, sat_t(T_TABLE_MAX).p)`, the P4 steam-table domain); `steam::sat_rho_g(rho_g: f64) -> Result<Sat, DomainError>`; `SteamSystem::init_line_steady(&mut self, p_vessel: f64)`; `SteamSystem::step_line(&mut self, dt: f64, w_in_applied: f64, h_in: f64) -> Result<(), DomainError>`. `SteamSystem::new(msivs_open, tsv_open)` and `compute_flows(dt, p)` keep the signatures Task 2 introduced; `compute_flows` now computes REQUESTED flows from an implicit storage solve. No later task changes any of these signatures.
- Allocation with the node (master step order 8-10): the vessel bounds vessel->line inflow; `Sim` scales ONLY `w_line` by `flows.w_steam / requested`; `step_line` then re-solves the node with the applied inflow and sets the node's own applied outflow `w_tcv`, `w_bypass`, bounded by inventory plus applied inflow. Outflows are never scaled by the vessel ratio, so a full line keeps discharging when the vessel supplies nothing. Consumers (turbine, condenser) read `w_tcv`/`w_bypass` AFTER `step_line`.
- Energy: `energy` is integrated from the applied enthalpy flows (`h_in` = `flows.h_steam`, the enthalpy the vessel released; `h_out` = `h_g` at the node's entry pressure) minus the explicit wall exchange `q_wall_w = (E_integrated - mass_new * u_g(p_new)) / dt` (A-6xx), which `Sim` reports as `EnergyBalance.q_line_wall_w` from Task 6. The stored `energy` therefore always equals the saturated internal energy of the stored mass, and `d(energy)/dt = w_in*h_in - w_out*h_out - q_wall_w` holds exactly every step.
- Rejection (transactional): a solve whose bracket `line_p_bounds()` does not contain the root, a converged residual above `1e-6 * (mass + 1)` kg, or a density outside the table column returns `Err(DomainError::PropertyOutOfRange { what: "line_p" | "rho_g", .. })` from `step_line` with the node, `w_tcv`, `w_bypass` and `p_chest` unchanged; `compute_flows` (void) zeroes the requested flows and sets `last_error`. `Sim` records the error at step 17 (Task 2a drain); conservation identities are not asserted on a rejected step (same rule as P3's EOS rejection). Non-finite arguments are rejected first (Task 2a).

- [ ] **Step 1: Write the failing tests**

Append to `crates/bwr-core/tests/power_conversion.rs`:

```rust
use bwr_core::bop::{line_p_bounds, SteamLine};
use bwr_core::steam::{sat_p, sat_rho_g, sat_t, T_TABLE_MAX, T_TABLE_MIN};

/// d(energy) - dt*(w_in*h_in - w_out*h_out - q_wall) for the step that produced `after`.
fn line_energy_residual(before: &SteamLine, after: &SteamLine, w_in: f64, h_in: f64, w_out: f64, h_out: f64) -> f64 {
    (after.energy - before.energy) - DT * (w_in * h_in - w_out * h_out - after.q_wall_w)
}

#[test]
fn steam_line_charging_and_discharge_close_mass_and_energy() {
    // Isolated node, valves open, charged at the steady inflow: the committed state is the predictor's.
    let mut sys = SteamSystem::new(true, true);
    sys.tcv_pos = 0.85;
    sys.init_line_steady(7.2e6);
    let p0 = sys.line.p;
    let h_in = sat_p(7.2e6).h_g;
    sys.compute_flows(DT, 7.2e6);
    let w_req = sys.vessel_steam_flow();
    let before = sys.line;
    let h_out = sat_p(before.p).h_g;
    sys.step_line(DT, w_req, h_in).unwrap();
    let w_out = sys.w_tcv + sys.w_bypass;
    assert!((sys.line.p - p0).abs() < 1.0e3, "steady line drifted {} -> {}", p0, sys.line.p);
    assert!((sys.line.mass - (before.mass + (w_req - w_out) * DT)).abs() < 1e-6);
    assert!(line_energy_residual(&before, &sys.line, w_req, h_in, w_out, h_out).abs() < 1.0, "energy identity");
    assert!(sys.line.q_wall_w.abs() < 1.0e6, "steady wall exchange must be small: {} W", sys.line.q_wall_w);
    assert_eq!(sys.p_chest, sys.line.p);
    // Discharge with zero applied inflow: outflow continues from inventory, pressure falls,
    // mass is exact and the energy identity holds with the explicit wall term.
    let before = sys.line;
    let h_out = sat_p(before.p).h_g;
    sys.step_line(DT, 0.0, h_in).unwrap();
    let w_out = sys.w_tcv + sys.w_bypass;
    assert!(w_out > 0.9 * w_req, "the node keeps discharging from its own inventory: {w_out}");
    assert!(sys.line.p < p0);
    assert!((sys.line.mass - (before.mass - w_out * DT)).abs() < 1e-6);
    assert!(line_energy_residual(&before, &sys.line, 0.0, h_in, w_out, h_out).abs() < 1.0);
    println!("discharge step: q_wall_w = {:.3} MW at w_out {:.1} kg/s", sys.line.q_wall_w / 1e6, w_out);
}

#[test]
fn closed_valve_charging_raises_line_pressure_and_conserves() {
    let mut sys = SteamSystem::new(true, false); // TSV shut, TCV and bypass shut
    sys.init_line_steady(6.0e6);
    let (m0, e0, p0) = (sys.line.mass, sys.line.energy, sys.line.p);
    let h_in = sat_p(6.5e6).h_g;
    sys.compute_flows(DT, 6.5e6);
    let w_in = sys.vessel_steam_flow();
    assert!(w_in > 0.0, "a 0.5 MPa head must drive inflow");
    sys.step_line(DT, w_in, h_in).unwrap();
    assert_eq!(sys.w_tcv + sys.w_bypass, 0.0, "closed valves pass nothing");
    assert!((sys.line.mass - (m0 + w_in * DT)).abs() < 1e-9);
    assert!(sys.line.p > p0);
    assert!(((sys.line.energy - e0) - DT * (w_in * h_in - sys.line.q_wall_w)).abs() < 1.0);
    assert_eq!(sys.p_chest, sys.line.p);
}

#[test]
fn line_outflow_is_bounded_by_inventory_plus_applied_inflow() {
    // Full line, zero vessel supply (e.g. the vessel rejected its step): the node still
    // discharges from inventory, never more than it holds, with no ratio scaling.
    let mut sys = SteamSystem::new(true, true);
    sys.tcv_pos = 0.85;
    sys.init_line_steady(7.2e6);
    sys.compute_flows(DT, 7.2e6);
    let w_full = sys.w_tcv + sys.w_bypass;
    let w_req = sys.vessel_steam_flow();
    let p_pred = sys.p_chest;
    let m0 = sys.line.mass;
    sys.step_line(DT, 0.0, sat_p(7.2e6).h_g).unwrap();
    let w_zero = sys.w_tcv + sys.w_bypass;
    assert!(w_zero > 0.0 && w_zero < w_full, "zero supply: {w_zero} vs full {w_full}");
    assert!(w_zero * DT <= m0, "outflow bounded by inventory");
    // Partially allocated inflow: outflow and pressure sit between the two cases and the
    // committed pressure re-solves below the predictor's.
    let mut sys2 = SteamSystem::new(true, true);
    sys2.tcv_pos = 0.85;
    sys2.init_line_steady(7.2e6);
    sys2.compute_flows(DT, 7.2e6);
    sys2.step_line(DT, 0.5 * w_req, sat_p(7.2e6).h_g).unwrap();
    let w_half = sys2.w_tcv + sys2.w_bypass;
    assert!(w_half > w_zero && w_half < w_full, "half supply: {w_half}");
    assert!(sys2.line.p < p_pred);
    assert!((sys2.line.mass - (m0 + (0.5 * w_req - w_half) * DT)).abs() < 1e-6);
}

#[test]
fn steam_line_rejects_out_of_table_states_transactionally() {
    let (p_lo, p_hi) = line_p_bounds();
    assert!((p_lo - sat_t(T_TABLE_MIN).p).abs() < 1e-6 && (p_hi - sat_t(T_TABLE_MAX).p).abs() < 1e-6);
    // Inverse density lookup errs outside the column instead of clamping.
    assert!(sat_rho_g(0.5 * sat_p(p_lo).rho_g).is_err());
    assert!(sat_rho_g(2.0 * sat_p(p_hi).rho_g).is_err());
    assert!(sat_rho_g(f64::NAN).is_err());
    for &p in &[0.1e6, 1.0e6, 7.2e6] {
        let s = sat_p(p);
        assert!((sat_rho_g(s.rho_g).unwrap().p - p).abs() / p < 1e-3, "roundtrip {p}");
    }
    // Low endpoint: a node emptier than the table can represent, asked to discharge.
    let mut sys = SteamSystem::new(true, true);
    sys.tcv_pos = 1.0;
    sys.bypass_pos = 1.0;
    sys.line = SteamLine::at_pressure(p_lo);
    sys.line.mass *= 0.5;
    let before = format!("{:?}", (sys.line, sys.w_tcv, sys.w_bypass, sys.p_chest));
    let r = sys.step_line(DT, 0.0, sat_p(p_lo).h_g);
    assert!(matches!(r, Err(DomainError::PropertyOutOfRange { .. })), "{r:?}");
    assert_eq!(format!("{:?}", (sys.line, sys.w_tcv, sys.w_bypass, sys.p_chest)), before, "node unchanged");
    // High endpoint: inflow that would push the node above the table.
    let mut sys = SteamSystem::new(true, false);
    sys.line = SteamLine::at_pressure(p_hi);
    let before = format!("{:?}", sys.line);
    assert!(sys.step_line(DT, 1.0e5, sat_p(p_hi).h_g).is_err());
    assert_eq!(format!("{:?}", sys.line), before);
    // Non-finite inputs are rejected before anything is touched (Task 2a rule).
    let mut sys = SteamSystem::new(true, true);
    sys.init_line_steady(7.0e6);
    let before = format!("{:?}", sys.line);
    for &v in &[f64::NAN, f64::INFINITY] {
        assert!(matches!(sys.step_line(DT, v, 2.77e6), Err(DomainError::PropertyOutOfRange { what, .. }) if what == "w_in_applied"));
        assert!(matches!(sys.step_line(DT, 100.0, v), Err(DomainError::PropertyOutOfRange { what, .. }) if what == "h_in"));
    }
    assert!(matches!(sys.step_line(0.0, 100.0, 2.77e6), Err(DomainError::PropertyOutOfRange { what, .. }) if what == "dt"));
    assert!(matches!(sys.step_line(DT, -1.0, 2.77e6), Err(DomainError::PropertyOutOfRange { what, .. }) if what == "w_in_applied"));
    assert_eq!(format!("{:?}", sys.line), before);
}

#[test]
fn init_line_steady_sets_header_pressure_before_the_first_step() {
    for scenario in [ScenarioName::ColdShutdown, ScenarioName::HotStandby, ScenarioName::FullPower] {
        let sim = Sim::new(scenario);
        assert_eq!(sim.steam.p_chest, sim.steam.line.p, "{scenario:?}: p_chest must mirror the node");
        let p_vessel = sim.vessel.eos.p;
        assert!(
            sim.steam.line.p <= p_vessel + 1.0 && sim.steam.line.p > 0.5 * p_vessel.max(P_ATM),
            "{scenario:?}: line {} vessel {p_vessel}",
            sim.steam.line.p
        );
    }
    let hs = Sim::new(ScenarioName::HotStandby);
    assert!(hs.steam.p_chest > 6.0e6, "hot standby header must not read zero: {}", hs.steam.p_chest);
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
    assert!(sim.domain_errors.is_empty(), "draining stays inside the table: {:?}", sim.domain_errors);
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
fn steam_consumers_see_applied_flows_with_line_storage() {
    // Master steps 9-10: sum(w_line) is the vessel's APPLIED outflow; the node's outflow
    // differs from it by the line mass derivative (the storage-aware identity).
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    sim.apply(Command::TripTurbine); // a real transient so the storage term is non-zero
    for _ in 0..(30.0 / DT) as usize {
        let m_before = sim.steam.line.mass;
        sim.step(DT);
        let f = sim.last_flows();
        let dm_dt = (sim.steam.line.mass - m_before) / DT;
        assert!((sim.steam.vessel_steam_flow() - f.w_steam).abs() < 1e-9, "sum(w_line) must be the applied vessel outflow");
        assert!(
            ((sim.steam.w_tcv + sim.steam.w_bypass) - (f.w_steam - dm_dt)).abs() < 1e-6,
            "consumers {} vs vessel {} - dm/dt {}",
            sim.steam.w_tcv + sim.steam.w_bypass,
            f.w_steam,
            dm_dt
        );
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --release -p bwr-core --test power_conversion`
Expected: compile error `cannot find SteamLine` / `no field line`.

- [ ] **Step 3: Add the inverse density lookup to `steam.rs`**

After `pub fn sat_p(p: f64) -> Sat { ... }` in `crates/bwr-core/src/steam.rs` add (with `use crate::domain::DomainError;` in the imports):

```rust
/// Saturation properties at saturated-vapour density `rho_g` (kg/m3): the inverse of
/// the table's `rho_g` column. Err for non-finite input or outside the column (no clamp),
/// and if the interpolated row does not reproduce `rho_g` (residual check).
pub fn sat_rho_g(rho_g: f64) -> Result<Sat, DomainError> {
    let (lo, hi) = (SAT_TABLE[0][3], SAT_TABLE[N - 1][3]);
    if !rho_g.is_finite() || rho_g < lo || rho_g > hi {
        return Err(DomainError::PropertyOutOfRange { what: "rho_g".into(), value: rho_g });
    }
    let (i, f) = locate(3, rho_g, true);
    let s = sat_t(T0 + i as f64 + f);
    if (s.rho_g - rho_g).abs() > 1e-6 * rho_g {
        return Err(DomainError::PropertyOutOfRange { what: "rho_g".into(), value: rho_g });
    }
    Ok(s)
}
```

Add a unit test in the existing `mod tests` of `steam.rs`:

```rust
    #[test]
    fn rho_g_roundtrip_and_bounds() {
        for &p in &[0.1e6, 1.0e6, 7.2e6] {
            let s = sat_p(p);
            assert!((sat_rho_g(s.rho_g).unwrap().p - p).abs() / p < 1e-3, "p {p}");
        }
        assert!(sat_rho_g(0.5 * SAT_TABLE[0][3]).is_err());
        assert!(sat_rho_g(2.0 * SAT_TABLE[N - 1][3]).is_err());
        assert!(sat_rho_g(f64::INFINITY).is_err());
    }
```

- [ ] **Step 4: Add the volume parameter and its register row**

In `params.rs` after `K_MSIV_OPEN` add:

```rust
/// Steam volume of both main steam lines, header and turbine steam chest, m3. ASSUMED (A-6xx).
pub const V_STEAM_LINE_M3: f64 = 60.0;
```

Append to `docs/model/parameters.csv` (locked header; `A-6xx` is the steam-line-node assumption id from Task 1):

```
V_STEAM_LINE_M3,60.0,m3,steam volume of both main steam lines plus header and chest,n/a,BWRX-300 (assumed),assumed,,30,120,A-6xx,crates/bwr-core/src/params.rs,line storage node; interval spans half to double
```

- [ ] **Step 5: Implement `SteamLine`, the bounded solve, and the request/commit split in `bop.rs`**

Add `use crate::steam::{sat_p, sat_rho_g, sat_t, Sat, P_ATM, T_TABLE_MAX, T_TABLE_MIN};` (merge with the existing `steam` imports). Before `pub struct SteamSystem`, add:

```rust
/// Registered steam-table pressure domain (P4 bounds): the bracket for every line solve.
pub fn line_p_bounds() -> (f64, f64) {
    (sat_t(T_TABLE_MIN).p, sat_t(T_TABLE_MAX).p)
}

/// Bisection for a line pressure on `line_p_bounds()`. `g` is increasing in p and returns
/// the mass residual, kg. Err (nothing committed by the caller) when the bracket does not
/// contain the root or the converged residual exceeds `tol`.
fn solve_line_pressure(g: impl Fn(f64) -> f64, tol: f64) -> Result<f64, DomainError> {
    let (mut lo, mut hi) = line_p_bounds();
    if g(lo) > 0.0 {
        return Err(DomainError::PropertyOutOfRange { what: "line_p".into(), value: lo });
    }
    if g(hi) < 0.0 {
        return Err(DomainError::PropertyOutOfRange { what: "line_p".into(), value: hi });
    }
    for _ in 0..80 {
        let mid = 0.5 * (lo + hi);
        if g(mid) < 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let p = 0.5 * (lo + hi);
    if g(p).abs() > tol {
        return Err(DomainError::PropertyOutOfRange { what: "line_p".into(), value: p });
    }
    Ok(p)
}

/// Lumped saturated-steam storage in the main steam lines, header and chest (master P6).
#[derive(Clone, Copy, Debug)]
pub struct SteamLine {
    pub mass: f64,
    /// J; integrated from the applied enthalpy flows minus `q_wall_w` (see `step_line`),
    /// which keeps it equal to the saturated internal energy of `mass` at `p`.
    pub energy: f64,
    pub p: f64,
    /// W; explicit exchange with the line metal that keeps the node on the saturation
    /// line (A-6xx). Reported as `EnergyBalance.q_line_wall_w`.
    pub q_wall_w: f64,
}

impl SteamLine {
    /// Saturated vapour filling the line volume at `p` (clamped to the registered bounds:
    /// a scenario-build initialiser, never called from a step function).
    pub fn at_pressure(p: f64) -> Self {
        let (lo, hi) = line_p_bounds();
        let s = sat_p(p.clamp(lo, hi));
        let mass = V_STEAM_LINE_M3 * s.rho_g;
        Self { mass, energy: mass * s.u_g(), p: s.p, q_wall_w: 0.0 }
    }

    fn mass_at(p: f64) -> f64 {
        V_STEAM_LINE_M3 * sat_p(p).rho_g
    }
}
```

Add `pub line: SteamLine,` to `SteamSystem` and `line: SteamLine::at_pressure(P_ATM),` in `new`. Replace the Task 2 `compute_flows` with the request predictor and add `init_line_steady` and `step_line` (the Task 2a guard stays first):

```rust
    /// Quasi-steady header state for dome pressure `p` (scenario build). Sets the node
    /// AND `p_chest` on every path so the first snapshot and P8's sensor seed are consistent.
    pub fn init_line_steady(&mut self, p: f64) {
        let a_tcv = C_TCV * self.tcv_pos * self.tsv_pos;
        let a = a_tcv + C_BYPASS * self.bypass_pos;
        let p_hdr = if !self.any_line_open() || p <= P_ATM {
            P_ATM
        } else if a < 1e-12 {
            p
        } else {
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
            0.5 * (lo + hi)
        };
        self.line = SteamLine::at_pressure(p_hdr);
        self.p_chest = self.line.p;
        self.w_line = [0.0; N_STEAM_LINES];
        self.w_tcv = 0.0;
        self.w_bypass = 0.0;
    }

    /// REQUESTED steam flows for this step (master step 5): an implicit predictor solves
    /// the end-of-step line pressure p' from mass(p') = mass + dt*(w_in(p') - w_out(p'))
    /// on the registered bracket, then evaluates every flow at p'. Nothing is committed
    /// here; `step_line` commits with the APPLIED inflow after vessel allocation.
    pub fn compute_flows(&mut self, dt: f64, p: f64) {
        if let Err(e) = check_step_args(dt, &[("p_vessel", p)]) {
            self.last_error = Some(e);
            return;
        }
        let a_tcv = C_TCV * self.tcv_pos * self.tsv_pos;
        let a_byp = C_BYPASS * self.bypass_pos;
        let a = a_tcv + a_byp;
        let msiv = self.msiv_pos;
        let m0 = self.line.mass;
        let g = |ph: f64| -> f64 {
            let w_in: f64 = msiv.iter().map(|&m| line_flow(p - ph, m)).sum();
            SteamLine::mass_at(ph) - m0 - dt * (w_in - a * (ph - P_ATM).max(0.0))
        };
        match solve_line_pressure(g, 1e-6 * (m0 + 1.0)) {
            Ok(ph) => {
                for i in 0..N_STEAM_LINES {
                    self.w_line[i] = line_flow(p - ph, msiv[i]);
                }
                let dp_out = (ph - P_ATM).max(0.0);
                self.w_tcv = a_tcv * dp_out;
                self.w_bypass = a_byp * dp_out;
                self.p_chest = ph;
            }
            Err(e) => {
                // Envelope rejection: nothing is requested this step; the node is untouched.
                self.w_line = [0.0; N_STEAM_LINES];
                self.w_tcv = 0.0;
                self.w_bypass = 0.0;
                self.last_error = Some(e);
            }
        }
    }

    /// Commit the APPLIED inflow (master step 10). `w_in_applied` is `vessel_steam_flow()`
    /// after `Sim` scaled `w_line`; `h_in` is the enthalpy the vessel released
    /// (`BoundaryFlows.h_steam`). The node re-solves its end-of-step pressure with that
    /// inflow fixed, which bounds its OWN outflow (`w_tcv`, `w_bypass`) against inventory
    /// plus inflow: discharge from existing inventory continues when the vessel supplies
    /// nothing. Energy is integrated from the enthalpy flows (outlet at the entry-state
    /// `h_g`); the mismatch against the saturated state is the explicit wall term.
    /// Transactional: on Err nothing (node, flows, `p_chest`) changes.
    pub fn step_line(&mut self, dt: f64, w_in_applied: f64, h_in: f64) -> Result<(), DomainError> {
        check_step_args(dt, &[("w_in_applied", w_in_applied), ("h_in", h_in)])?;
        if w_in_applied < 0.0 {
            return Err(DomainError::PropertyOutOfRange { what: "w_in_applied".into(), value: w_in_applied });
        }
        let a_tcv = C_TCV * self.tcv_pos * self.tsv_pos;
        let a_byp = C_BYPASS * self.bypass_pos;
        let a = a_tcv + a_byp;
        let m0 = self.line.mass;
        let g = |ph: f64| -> f64 { SteamLine::mass_at(ph) - m0 - dt * (w_in_applied - a * (ph - P_ATM).max(0.0)) };
        let p_new = solve_line_pressure(g, 1e-6 * (m0 + 1.0))?;
        let dp_out = (p_new - P_ATM).max(0.0);
        let (w_tcv, w_byp) = (a_tcv * dp_out, a_byp * dp_out);
        let w_out = w_tcv + w_byp;
        let mass_new = m0 + dt * (w_in_applied - w_out);
        let s_new = sat_rho_g(mass_new / V_STEAM_LINE_M3)?; // consistent with p_new to the residual check
        let h_out = sat_p(self.line.p).h_g;
        let e_integrated = self.line.energy + dt * (w_in_applied * h_in - w_out * h_out);
        let q_wall_w = (e_integrated - mass_new * s_new.u_g()) / dt;
        // Everything below is the commit.
        self.line.q_wall_w = q_wall_w;
        self.line.energy = e_integrated - q_wall_w * dt;
        self.line.mass = mass_new;
        self.line.p = s_new.p;
        self.w_tcv = w_tcv;
        self.w_bypass = w_byp;
        self.p_chest = self.line.p;
        Ok(())
    }
```

When the allocation ratio is 1.0 (every normal step) the committed state equals the predictor's implicit solution exactly, so the node never drifts from its own solve; `compute_flows` and `step_line` share one `g` shape so the two solves agree to the residual tolerance.

Update the Task 2 test `msiv_flow_is_per_line_not_averaged` to call `a.init_line_steady(P_RATED); a.compute_flows(DT, P_RATED);` (same for `b`): the quasi-steady state is now the initialiser followed by one request computation, and its `in == out` assertion holds because the predictor starts from the steady node.

- [ ] **Step 6: Wire into `sim.rs`**

Add `use crate::steam::{sat_p, P_ATM};` if not already imported. In `Sim::base`, after `s.prev_gen_online = ...;` add (P4's `Vessel::steam_available()` is the Regime-based steam-space test; without a steam space the dome supplies nothing and the line sees atmosphere):

```rust
        let p_dome = if s.vessel.steam_available() { s.vessel.eos.p_steam } else { P_ATM };
        s.steam.init_line_steady(p_dome);
```

In `Sim::full_power()`, just before `s.settle(600.0);` (the TCV position was set after `base`): `s.steam.init_line_steady(s.vessel.eos.p_steam);`.

In `Sim::step`, restructure the steam/vessel section to master steps 5 and 8-11. Replace the P4 call `self.steam.compute_flows(dt, p);` (Task 2 form) and the turbine call that follows it, and move the `VesselInputs`/`vessel.step` block up between them, so the section reads:

```rust
        // 5. requested steam flows at the dome steam pressure (P4 Regime: no steam space -> atmospheric)
        let p_dome = if self.vessel.steam_available() { self.vessel.eos.p_steam } else { P_ATM };
        self.steam.compute_flows(dt, p_dome);

        // ... 6 (IC request) and 7 (feedwater) stay where P3 left them, before the vessel ...

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

        // 9. scale ONLY the vessel->line inflow to the applied transfer; the node bounds its own outflow
        let requested = self.steam.vessel_steam_flow();
        let ratio = if requested > 0.0 { (flows.w_steam / requested).clamp(0.0, 1.0) } else { 0.0 };
        for w in self.steam.w_line.iter_mut() {
            *w *= ratio;
        }
        // RWCU consumers use flows.w_liquid_out (applied), never fw.rwcu_reject (request).

        // 10. line storage commits the applied inflow at the enthalpy the vessel released and
        //     re-solves w_tcv / w_bypass against its own inventory
        let h_line_out = sat_p(self.steam.line.p).h_g; // the node's outlet enthalpy for this step (entry state)
        if let Err(e) = self.steam.step_line(dt, self.steam.vessel_steam_flow(), flows.h_steam) {
            self.steam.last_error = Some(e);
        }

        // 11. turbine consumes the node's applied w_tcv
        let p_cond = self.cond.pressure();
        self.turbine.step(dt, self.steam.w_tcv, self.steam.p_chest, p_cond);
```

(The baseline computed `p_cond` and stepped the turbine before the vessel; both move after the allocation. Task 6 extends the turbine call with `extraction_heat_w`.) In the condenser block that follows (P3 form), replace `sat_p(self.steam.p_chest.max(P_ATM)).h_g` with `h_line_out` so the condenser values the steam at the same enthalpy the node discharged it. Change the feedwater call's `steam_flow` argument from `self.steam.total_flow()` to `self.steam.vessel_steam_flow()` (the three-element controller measures line flow; at step 7 this still holds the request, which is what a real flow sensor sees within one step). In `isolation_signals`, change `self.steam.total_flow() > 1.4 * STEAM_FLOW_RATED` to `self.steam.vessel_steam_flow() > 1.4 * STEAM_FLOW_RATED`. In `snapshot()`, change `steam_flow_kgs: self.steam.total_flow() + v.vent_flow` to `steam_flow_kgs: self.steam.vessel_steam_flow() + v.vent_flow` (post-step these are applied values).

- [ ] **Step 7: Migrate the P3 identities to the storage-aware form**

P3's `tests/numerics.rs::downstream_consumers_see_applied_flows` and `tests/conservation.rs::coupled_low_inventory_run_conserves_mass_end_to_end` assert `w_tcv + w_bypass == f.w_steam` every step. With a line node that equality is false by exactly the line mass derivative. In each test replace that assertion (do not touch anything else in P3's files) with the two master identities, tracking the line mass across the step:

```rust
        // before the step:
        let m_line_before = sim.steam.line.mass;
        sim.step(DT);
        let f = sim.last_flows();
        let dm_line_dt = (sim.steam.line.mass - m_line_before) / DT;
        // (a) summed applied line flow is the vessel's applied steam outflow
        assert!((sim.steam.vessel_steam_flow() - f.w_steam).abs() < 1e-9, "sum(w_line) {} vs applied {}", sim.steam.vessel_steam_flow(), f.w_steam);
        // (b) storage-aware identity: consumers see the vessel release minus line storage
        assert!(
            ((sim.steam.w_tcv + sim.steam.w_bypass) - (f.w_steam - dm_line_dt)).abs() < 1e-6,
            "turbine+bypass {} but vessel applied {} with dm_line/dt {}",
            sim.steam.w_tcv + sim.steam.w_bypass,
            f.w_steam,
            dm_line_dt
        );
```

In the coupled depletion test the receiver-side integral stays as P3 wrote it (`received_kg` from `f.w_steam + f.w_liquid_out` measures the vessel's releases); if P3's cycle-2 revision measures the hotwell directly instead, add `sim.steam.line.mass - m_line_at_start` to the receiver side so the steam parked in the line is counted. Run `cargo test --release -p bwr-core --test numerics --test conservation` → both pass.

- [ ] **Step 8: Run all tests**

Run: `cargo test --release -p bwr-core`
Expected: all pass. `turbine_trip_rides_through_on_bypass` and `raising_pressure_raises_power` may shift numerically; if any assertion fails, record the values in `docs/model/power-conversion.md` "Results" and report before touching thresholds (master constraint: no bulk retune).

- [ ] **Step 9: Compare, classify, then regenerate the baselines (master protocol)**

Run `cargo run --release -p bwr-core --example validation` (compare mode) and save the full output to `docs/validation/reports/2026-10-21-pkg06.md` under a heading `## Task 3: steam line storage node` (create the file with the P2 report header if it does not exist). Classify every changed metric: physical (line storage delays and softens the `fp_turbine_trip` pressure rise; MSIV closure now drains the line; the header pressure is no longer the quasi-steady value) or numerical; `header_pressure_pa` stays unpopulated until Task 10 Step 5b (never expect byte-identical traces for a column a later task populates). Physical gates: the Task 3 tests above plus `turbine_trip_rides_through_on_bypass`. Only after the report is written run `cargo run --release -p bwr-core --example validation -- --write` ONCE for this change set. Equality with the freshly written baselines is not evidence; the saved compare output is. Append to `docs/model/power-conversion.md` "Results" a table of `fp_turbine_trip` and the P2 MSIV-closure case's peak pressure and time before/after this task (from the saved compare output; use the exact ids `standard_cases()` defines).

- [ ] **Step 10: Commit**

```bash
git add crates/bwr-core/src crates/bwr-core/tests docs/model docs/validation
git commit -m "feat(bop): lumped steam line storage node with bounded saturation solve and explicit wall term

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---
### Task 4: Turbine stop valve and steam-side fault holds

**Files:**
- Modify: `crates/bwr-core/src/params.rs` (`TSV_STROKE`, `TSV_OPEN_RATE`)
- Modify: `crates/bwr-core/src/bop.rs` (`SteamFaults`, `step` stroke dynamics and fault holds, `pressure_demand`)
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
`SteamSystem::new(msivs_open, tsv_open)`, `SteamSystem.tsv_pos` and `compute_flows(dt, p)` exist with their final signatures since Task 2 and `init_line_steady(p)` since Task 3: this task changes NO signature of theirs, it only adds behaviour. `pressure_demand(&self, p: f64, faults: &SteamFaults) -> f64`; `step(&mut self, dt, p, tcv_flow_demand, bypass_available, tripped, faults: &SteamFaults)`; `Sim::bop_faults(&self) -> (SteamFaults, CondenserFaults, FeedFaults)` (the latter two are defined in Tasks 7 and 8; in this task return `SteamFaults` only via `fn steam_faults(&self) -> SteamFaults`, and Task 8 folds it into `bop_faults`).

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
    assert!(sim.has_fault(&Fault::TsvStuck) && sim.has_fault(&Fault::TcvStuck), "faults retained");
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
    assert!(sim.has_fault(&Fault::MsivStuck { line: 0 }), "fault retained");
}

#[test]
fn regulator_frozen_output_overrides_pressure_demand() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    sim.set_fault(Fault::PressureRegulatorFailed { demand_pct: 0 }, true);
    run(&mut sim, 60.0);
    assert!(sim.scrammed, "no steam demand: pressure must reach the high-pressure scram");
    assert_eq!(sim.scram_reason.as_deref(), Some("Reactor high pressure"));
    assert!(sim.has_fault(&Fault::PressureRegulatorFailed { demand_pct: 0 }), "fault retained");
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --release -p bwr-core --test power_conversion`
Expected: the tests compile (every name exists since Tasks 2-3); `tsv_closes_fast_on_trip_and_stuck_tsv_holds` fails on `stuck TSV must not move` (the Task 2 TEMPORARY stop valve ignores faults), `stuck_msiv_stays_open_during_isolation` fails on `line 0 must stay open`, and `regulator_frozen_output_overrides_pressure_demand` fails on `no steam demand` (the regulator ignores the fault).

- [ ] **Step 3: Parameters**

In `params.rs` after `MSIV_STROKE`:

```rust
/// Turbine stop valve closing stroke on trip, s. ASSUMED (typical fast-acting stop valve).
pub const TSV_STROKE: f64 = 0.1;
/// Turbine stop valve opening rate after reset, fraction per second. ASSUMED.
pub const TSV_OPEN_RATE: f64 = 1.0;
```

Append to `docs/model/parameters.csv` (same task, register-row rule):

```
TSV_STROKE,0.1,s,turbine stop valve closing stroke on trip,n/a,generic,assumed,,0.05,0.3,A-6xx,crates/bwr-core/src/params.rs,
TSV_OPEN_RATE,1.0,1/s,turbine stop valve opening rate after reset,n/a,generic,assumed,,0.2,2.0,A-6xx,crates/bwr-core/src/params.rs,
```

- [ ] **Step 4: Implement in `bop.rs`**

Add the `SteamFaults` struct (from Interfaces) above `SteamSystem`. `tsv_pos`, the two-argument `new` and the `self.tsv_pos` use inside `init_line_steady`/`compute_flows`/`step_line` are already in place (Tasks 2-3). Delete the Task 2 TEMPORARY line `self.tsv_pos = if tripped { 0.0 } else { 1.0 };` from `step`: the stroke dynamics below replace it (mandatory).

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
        if let Err(e) = check_step_args(dt, &[("p", p), ("tcv_flow_demand", tcv_flow_demand)]) {
            self.last_error = Some(e); // Task 2a guard stays first
            return;
        }
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

Add `use crate::bop::SteamFaults;` and `use crate::faults::Fault;` to the imports. There are no constructor or `init_line_steady` call-site changes: they have carried their final arguments since Tasks 2 and 3.

In `Sim::step` replace the steam block:

```rust
        // --- main steam, EHC, turbine
        let sf = self.steam_faults();
        let demand = self.steam.pressure_demand(p, &sf);
        let tcv_demand = self.turbine.tcv_demand(dt, demand);
        let byp_ok = self.bypass_available();
        self.steam.step(dt, p, tcv_demand, byp_ok, self.turbine.tripped, &sf);
        let p_dome = if self.vessel.steam_available() { self.vessel.eos.p_steam } else { P_ATM };
        self.steam.compute_flows(dt, p_dome);
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

Run: `cargo test --release -p bwr-core --test power_conversion` (one positional filter per `cargo test` invocation, or none)
Expected: `load_rejection_with_bypass_does_not_scram` and `turbine_trip_without_bypass...` compile and may already pass (verification tests); `excessive_steam_demand_in_run_isolates` passes only if the P2 log line for MSIV isolation carries the locked `msiv isolation:` key. Record which passed at this point in the commit message.

- [ ] **Step 3: Make the SRI trigger a registered parameter**

In `params.rs`, in the protection setpoints section:

```rust
/// Power above which a turbine trip / load rejection triggers Select Rod Insert. ASSUMED (A-6xx).
pub const SRI_TRIGGER_POWER_FRAC: f64 = 0.4;
```

In `sim.rs` `protection()` replace `self.core.kin.n > 0.4` with `self.core.kin.n > SRI_TRIGGER_POWER_FRAC`. Update the `BYPASS_CAPACITY` doc comment to: `/// Turbine bypass capacity as fraction of rated steam flow. ASSUMED (interval 0.25..1.1, Task 5a sensitivity); the spec marks it "to verify" and no BWRX source confirms the nominal value; the turbine_trip_no_bypass case bounds the zero-capacity end.`

Register rows (same task): append

```
SRI_TRIGGER_POWER_FRAC,0.4,-,power above which turbine trip/load rejection triggers Select Rod Insert,n/a,BWRX-300 (assumed),assumed,,0.3,0.6,A-6xx,crates/bwr-core/src/params.rs,was a literal in sim.rs; Task 5a sensitivity
```

and UPDATE the existing `BYPASS_CAPACITY` row in place: status `assumed`, `interval_lo` 0.25, `interval_hi` 1.1, notes `nominal value unconfirmed (spec: to verify); sensitivity in Task 5a`.

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

In `tests/plant.rs` `turbine_trip_rides_through_on_bypass`, make the criterion windowed and explicit (the case starts at power fraction ~1, so a whole-run peak can never be below 0.9): keep `run(&mut sim, 5.0); sim.apply(Command::TripTurbine);`, then replace `run(&mut sim, 120.0);` and the three assertions with a stepped loop over the next 120 s that asserts, at every step from 10 s after the trip onward, `sim.core.kin.n < 0.9` (message `"case fp_turbine_trip: SRI must hold power below 0.9 from t+10 s, got {} at {}"`) and `sim.vessel.eos.p < 7.4e6` at every step, and after the loop `!sim.scrammed` and `sim.snapshot().steam.steam_to_bypass_kgs > 200.0`. `fp_turbine_trip` is registered in `cases.csv` by Package 2; update its `purpose` cell to state exactly that criterion: `Turbine trip with bypass; SRI ride-through: power fraction < 0.9 at every sample from 10 s after the trip to the end of the window (t+120 s), bypass > 200 kg/s at the end, vessel pressure < 7.4 MPa throughout, no scram`.

- [ ] **Step 5: Run everything and regenerate baselines**

Run: `cargo test --release -p bwr-core` → all pass.
Baseline protocol (master): run `cargo run --release -p bwr-core --example validation` in compare mode FIRST and append its output under `## Task 5: bypass, load rejection and regulator cases` in `docs/validation/reports/2026-10-21-pkg06.md`; the four new cases have no baseline yet (classify as new), and any change to `fp_turbine_trip` is classified physical (SRI trigger now a constant with the same value: expect none) or numerical. Pass the gates (the Task 5 tests and the windowed `turbine_trip_rides_through_on_bypass`), then run `--write` ONCE and commit the report with the new baseline CSVs. Equality with freshly written baselines proves nothing; the saved compare output is the evidence.

- [ ] **Step 6: Append results**

In `docs/model/power-conversion.md` "Results", add a table with one row per new case: peak pressure, time of peak, scram reason, IC initiation time, taken from the runner output.

- [ ] **Step 7: Commit**

```bash
git add crates/bwr-core docs/validation docs/model/power-conversion.md
git commit -m "test(bop): bypass, load rejection and regulator fault cases; SRI trigger registered

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 5a: Bypass capacity, SRI group and ride-through evidence and sensitivity

**Files:**
- Modify: `docs/validation/findings.md` (evidence-search record), `docs/model/parameters.csv` (`BYPASS_CAPACITY` note), `docs/model/power-conversion.md` ("Results")
- Create: `docs/validation/reports/2026-10-21-pkg06-sensitivity.md`, `crates/bwr-core/examples/p6_sensitivity.rs`
- Modify: `crates/bwr-core/src/bop.rs` (`SteamSystem.bypass_capacity`, `c_bypass()`), `crates/bwr-core/src/sim.rs` (`Sim.sri_trigger_power_frac`, `Sim::full_power_with_bop`)
- Test: `crates/bwr-core/tests/power_conversion.rs`

**Interfaces:**
- Produces (package-local, mirroring P5's `Sim::full_power_with(coeffs)` sensitivity hook): `SteamSystem.bypass_capacity: f64` (default `BYPASS_CAPACITY`) with `fn c_bypass(&self) -> f64 { self.bypass_capacity * STEAM_FLOW_RATED / 7.0e6 }` replacing every `C_BYPASS` use inside `bop.rs` (`compute_flows`, `init_line_steady`, `step_line`, `step`); `Sim.sri_trigger_power_frac: f64` (default `SRI_TRIGGER_POWER_FRAC`) read by `protection()`; `pub fn Sim::full_power_with_bop(bypass_capacity: f64, sri_trigger_power_frac: f64) -> Sim` (builds `full_power()` then sets both and re-settles 120 s). The registered constants stay the SSOT defaults; the fields exist only so the sensitivity example and tests can vary them without recompiling.
- What is sourced versus plausible (recorded in `findings.md`, not asserted as design facts): no public BWRX-300 source found for the bypass capacity, the four-rod SRI group, its trigger or the ride-through criterion; ESBWR DCD values (110 % bypass) are ancestry, not confirmation (master global constraint). The criteria below are therefore PLAUSIBILITY tests of the implemented model, labelled as such in `cases.csv` and in the report.

- [ ] **Step 1: Record the evidence search**

Append to `docs/validation/findings.md` (P3 created it) a section `## P6: bypass capacity, SRI group and ride-through` with: the sources searched (`sources.csv` ids of the GE BWRX-300 general description and the ESBWR DCD Tier 2 chapters 7.7 and 10.4), what each states (ESBWR: 110 % bypass; BWRX: no published figure, no published SRI group), the resulting status (`BYPASS_CAPACITY` assumed 1.0 with interval 0.25–1.1; `SRI_TRIGGER_POWER_FRAC` assumed 0.4 with interval 0.3–0.6; the four-rod SRI group retained from the baseline as an assumption, A-6xx), and the plausibility criteria used by this package:

- ride-through: no scram within 300 s of a turbine trip or load rejection with bypass available and SRI acting; vessel pressure peak below `SCRAM_HIGH_PRESSURE`;
- governor: after a load rejection the shaft speed peak stays below `OVERSPEED_TRIP_RPM` and returns within 1 % of `RATED_RPM` within 60 s;
- SRI effectiveness: power fraction below 0.9 from 10 s after the trip to the end of the case.

- [ ] **Step 2: Write the failing tests**

Append to `tests/power_conversion.rs`:

```rust
#[test]
fn load_rejection_governor_holds_speed_below_overspeed_and_resettles() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    sim.apply(Command::OpenBreaker);
    let mut peak: f64 = 0.0;
    for _ in 0..(60.0 / DT) as usize {
        sim.step(DT);
        peak = peak.max(sim.turbine.speed_rpm);
    }
    assert!(peak < OVERSPEED_TRIP_RPM, "governor must hold the speed peak below the overspeed trip: {peak}");
    assert!(!sim.turbine.tripped, "no overspeed trip: {:?}", sim.turbine.trip_reason);
    assert!((sim.turbine.speed_rpm - RATED_RPM).abs() < 0.01 * RATED_RPM, "speed after 60 s {}", sim.turbine.speed_rpm);
}

#[test]
fn bypass_capacity_sensitivity_brackets_the_ride_through_threshold() {
    // Plausibility, not a sourced requirement: at the nominal (assumed) capacity the plant
    // rides through; at the low end of the interval it cannot.
    let ride_through = |cap: f64| {
        let mut sim = Sim::full_power_with_bop(cap, SRI_TRIGGER_POWER_FRAC);
        sim.apply(Command::TripTurbine);
        run(&mut sim, 300.0);
        !sim.scrammed
    };
    assert!(ride_through(BYPASS_CAPACITY), "nominal capacity must ride through");
    assert!(!ride_through(0.25), "a quarter-capacity bypass cannot absorb a full-power trip");
}
```

Run: `cargo test --release -p bwr-core --test power_conversion` → Expected: compile error `no function full_power_with_bop`.

- [ ] **Step 3: Implement the sensitivity hooks**

In `bop.rs` add `pub bypass_capacity: f64,` to `SteamSystem` (`BYPASS_CAPACITY` in `new`) and

```rust
    /// Bypass bank conductance for the configured capacity (the registered `C_BYPASS` at default).
    fn c_bypass(&self) -> f64 {
        self.bypass_capacity * STEAM_FLOW_RATED / 7.0e6
    }
```

then replace every `C_BYPASS` inside `bop.rs` with `self.c_bypass()` (`grep -n C_BYPASS crates/bwr-core/src/bop.rs` must return nothing afterwards; the constant stays in `params.rs` as the registered value). In `sim.rs` add `pub sri_trigger_power_frac: f64,` to `Sim` (`SRI_TRIGGER_POWER_FRAC` in `base`), use it in `protection()` where Task 5 put the constant, and add:

```rust
    /// Sensitivity build: full power with a different bypass capacity and SRI trigger (Task 5a).
    pub fn full_power_with_bop(bypass_capacity: f64, sri_trigger_power_frac: f64) -> Self {
        let mut s = Self::full_power();
        s.steam.bypass_capacity = bypass_capacity;
        s.sri_trigger_power_frac = sri_trigger_power_frac;
        s.settle(120.0);
        s.time = 0.0;
        s
    }
```

Create `crates/bwr-core/examples/p6_sensitivity.rs`: for `cap in [0.25, 0.5, 0.75, 1.0, 1.1]` x `sri in [0.3, 0.4, 0.5, 0.6]` build `Sim::full_power_with_bop(cap, sri)`, apply `Command::TripTurbine`, run 300 s, and print one markdown row `| cap | sri | scrammed (reason) | p_peak MPa | t_peak s | power at +30 s | power at +300 s |`. Then repeat the grid with `Command::OpenBreaker` (load rejection). Only `std` and the crate are used.

- [ ] **Step 4: Run the sensitivity and write the report**

Run: `cargo run --release -p bwr-core --example p6_sensitivity > docs/validation/reports/2026-10-21-pkg06-sensitivity.md`, then edit the file to add a header stating the purpose, the two intervals, the plausibility criteria from Step 1, and a short conclusion (the lowest capacity that rides through; whether the SRI trigger changes the outcome within 0.3–0.6). Append a two-line summary with a link to the report under "Results" in `docs/model/power-conversion.md`. Update the `BYPASS_CAPACITY` row note in `parameters.csv` to `sensitivity: docs/validation/reports/2026-10-21-pkg06-sensitivity.md`.

- [ ] **Step 5: Run all tests and commit**

Run: `cargo test --release -p bwr-core` → all pass (defaults unchanged: no baseline change).

```bash
git add crates/bwr-core docs/validation docs/model
git commit -m "test(bop): bypass capacity and SRI trigger sensitivity; evidence search recorded as plausibility criteria

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---
### Task 6: Control-volume plant energy balance with explicit extraction work and heater storage

**Files:**
- Modify: `crates/bwr-core/src/params.rs` (`EXTRACTION_WORK_FRACTION`, `EXTRACTION_FLOW_FRACTION_MAX`, `FW_HEATER_TAU_S`, `FW_HEATER_CAPACITY_J_PER_K`; `TURBINE_DH_RATED` updated) and `docs/model/parameters.csv` (rows in this task; the `TURBINE_DH_RATED` row is UPDATED)
- Modify: `crates/bwr-core/src/bop.rs` (`Turbine.p_extraction`, `Turbine.p_losses`, signed `p_elec`, `Turbine::kinetic_energy`, complete `Turbine::step`; `Feedwater { heater_energy, q_extraction_applied_w, pump_work_w }`, `Feedwater::step_heaters`, `Feedwater::step` loses the heater lag; `EnergyBalance`)
- Modify: `crates/bwr-core/src/sim.rs` (`Sim.balance`, `house_load`, step order 7-12 assembly)
- Modify: `docs/model/power-conversion.md` ("Energy balance" section)
- Test: `crates/bwr-core/tests/power_conversion.rs`

**Interfaces:**
- Produces (master): `Turbine.p_extraction: f64` (W of LP work forgone to the APPLIED extraction heat), `Turbine.p_losses: f64` (W dissipated this step, computed from entry energy + applied steam work − exit energy, never clipped), `Turbine.p_elec` SIGNED (negative = grid import while motoring; `Snapshot.gross_mwe` reports the signed value), `Turbine::step(&mut self, dt, w_turbine, p_chest, p_cond, extraction_heat_w: f64)`; `Feedwater { heater_energy, q_extraction_applied_w, pump_work_w }`; `Feedwater::step_heaters(&mut self, dt, w_feed_applied, w_turbine_applied, p_chest, hotwell_t)` (master step 11, before `Turbine::step`); `Feedwater::step(&mut self, dt, p_reactor, level_cm, steam_flow, hotwell_ok) -> Option<String>` (the heater arguments `turbine_flow`, `hotwell_t` move to `step_heaters`; Task 8 appends `faults`; update the Task 2a rejection table's `Feedwater::step` rows to this list); `EnergyBalance` with the master's field list `{ q_core_w, q_condenser_w, q_ic_w, q_loss_w, p_elec_gross_w, p_losses_w, p_extraction_w, house_pump_work_w, makeup_w, reject_w, vent_w, q_line_wall_w, storage_w, residual_w }`; `Sim.balance: EnergyBalance`; `Sim::house_load(&self) -> f64`. Package-local: `Turbine::kinetic_energy(&self) -> f64`, `Feedwater::heater_t(&self) -> f64`.
- The ONE extraction value: `fw.q_extraction_applied_w` is computed once per step by `step_heaters` as the heater demand bounded by steam-side availability (`EXTRACTION_FLOW_FRACTION_MAX` of the applied turbine steam, condensed and drained to hotwell temperature), then passed unchanged to `Turbine::step` (work forgone `EXTRACTION_WORK_FRACTION * q`) and subtracted from the condenser inlet load (steam energy that went to the heaters never reaches the condenser). The heater node stores it: `d(heater_energy)/dt = q_extraction_applied_w − [w_feed*(h_feed − h_hw) − pump_work_w]` and `t_feed = heater_t() + pump rise`, so the 180 s lag IS this storage and heat delivered after steam loss is drawn down from it. There is no negative-duty clamp anywhere: the condenser receives the signed `q_in`.
- Pump work is counted ONCE: `pump_work_w = w_feed_applied * CP_WATER * 2 K` (the baseline's 2 K rise) is fluid work entering the control volume (`house_pump_work_w`) and is excluded from extraction heat. `Sim::house_load()` (motor electrical input: `HOUSE_BASE` + pump motors) is drawn from gross power OUTSIDE the control volume; `house_load − pump_work_w` is motor/pump inefficiency plus work on external streams (CW, vacuum). Task 1's boundary table carries this row.
- Control volume: the master's definition, quoted verbatim in the `EnergyBalance` doc comment (Step 4). `storage_w` is the master's storage sum; P7 adds `d/dt(ic tube energy)` to that same sum.
- Temporaries in THIS task (mandatory replacements named): the condenser storage term uses `self.cond.hotwell_mass * steam::h_liquid(self.cond.hotwell_t())`, `q_condenser_w` uses the baseline `Condenser.heat_load`, and `makeup_w = reject_w = 0.0`, until Task 7 lands `Condenser { energy, q_cw_w, makeup_h, reject_h }` and replaces exactly those lines (Task 7 Step 5). The baseline `Condenser::step` still carries its non-negative clamp on `heat_load`; Task 7 Step 4 deletes it and P6 writes no new one. `step_heaters`' pump condition uses `running_pumps() > 0` until Task 8 Step 4 replaces it with `pump_speed` (mandatory). The transient closure gates (turbine trip, CW loss, vacuum degradation) are written in Task 7 Step 1 once the condenser is a first-law state; this task asserts steady-state closure and the turbine and heater energy identities.

- [ ] **Step 1: Write the failing tests**

Append to `tests/power_conversion.rs`:

```rust
fn print_balance(b: &bwr_core::bop::EnergyBalance) {
    println!(
        "core {:.1} cond {:.1} ic {:.1} loss {:.1} elec {:.1} tglosses {:.1} extraction {:.1} pumpwork {:.1} makeup {:.1} reject {:.1} vent {:.1} linewall {:.1} storage {:.1} residual {:.3} MW",
        b.q_core_w / 1e6, b.q_condenser_w / 1e6, b.q_ic_w / 1e6, b.q_loss_w / 1e6,
        b.p_elec_gross_w / 1e6, b.p_losses_w / 1e6, b.p_extraction_w / 1e6, b.house_pump_work_w / 1e6,
        b.makeup_w / 1e6, b.reject_w / 1e6, b.vent_w / 1e6, b.q_line_wall_w / 1e6, b.storage_w / 1e6, b.residual_w / 1e6
    );
}

#[test]
fn full_power_energy_balance_closes() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 300.0);
    let b = sim.balance;
    print_balance(&b);
    assert!(b.residual_w.abs() < 0.01 * RATED_POWER_W, "residual {} MW", b.residual_w / 1e6);
    // Physical closure at steady state (storage ~ 0): core heat plus pump work leaves as
    // electric power, machine losses and heat rejection, within 2 %.
    let out = b.p_elec_gross_w + b.p_losses_w + b.q_condenser_w + b.q_ic_w + b.q_loss_w + b.q_line_wall_w;
    assert!((b.q_core_w + b.house_pump_work_w - out).abs() < 0.02 * RATED_POWER_W, "core {} out {}", b.q_core_w / 1e6, out / 1e6);
    assert!(b.storage_w.abs() < 0.005 * RATED_POWER_W, "steady storage {}", b.storage_w / 1e6);
    assert!(b.q_line_wall_w.abs() < 2.0e6, "steady line wall exchange {} MW", b.q_line_wall_w / 1e6);
    assert!(b.p_extraction_w > 50.0e6 && b.p_extraction_w < 150.0e6, "extraction {}", b.p_extraction_w / 1e6);
    assert!(b.q_condenser_w > 480.0e6 && b.q_condenser_w < 600.0e6, "condenser {}", b.q_condenser_w / 1e6);
    assert!(b.house_pump_work_w > 1.0e6 && b.house_pump_work_w < 6.0e6, "pump work {}", b.house_pump_work_w / 1e6);
    // One extraction value, used by the turbine and the feedwater equations alike.
    assert!((sim.turbine.p_extraction - EXTRACTION_WORK_FRACTION * sim.fw.q_extraction_applied_w).abs() < 1.0);
    let s = sim.snapshot();
    assert!(s.turbine.net_mwe > 285.0 && s.turbine.net_mwe < 315.0, "net {}", s.turbine.net_mwe);
}

#[test]
fn turbine_coastdown_dissipates_exactly_the_entry_energy() {
    let mut tb = Turbine::new(false);
    tb.speed_rpm = RATED_RPM;
    let ke0 = tb.kinetic_energy();
    let mut dissipated = 0.0;
    let mut steps = 0usize;
    while tb.speed_rpm > 0.0 && steps < 400_000 {
        let ke_in = tb.kinetic_energy();
        tb.step(DT, 0.0, P_ATM, P_ATM, 0.0);
        assert!(tb.p_losses.is_finite() && tb.p_losses >= 0.0, "step {steps}: {}", tb.p_losses);
        // Every step, offline: entry KE + steam work (0) == exit KE + losses.
        assert!((ke_in - tb.kinetic_energy() - tb.p_losses * DT).abs() < 1.0, "step {steps}");
        dissipated += tb.p_losses * DT;
        steps += 1;
    }
    assert_eq!(tb.speed_rpm, 0.0, "coastdown must reach zero within the step budget");
    assert!((dissipated - ke0).abs() < 1e-6 * ke0, "dissipated {dissipated} J vs entry KE {ke0} J (the last step must not report zero loss)");
}

#[test]
fn reverse_power_is_reported_as_signed_grid_import() {
    let mut tb = Turbine::new(true); // on line at grid speed
    tb.step(DT, 0.0, 7.0e6, 5.6e3, 0.0);
    assert!(tb.p_elec < 0.0, "motoring must report import, not zero: {}", tb.p_elec);
    assert!(tb.breaker_closed, "the 3 s reverse-power timer has not expired");
    // Online: entry KE == exit KE (grid speed), so steam work == electric (signed) + losses exactly.
    assert!((tb.p_mech - tb.p_elec - tb.p_losses).abs() < 1.0);
    assert!(tb.p_losses > 0.0);
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    sim.apply(Command::TripTurbine);
    run(&mut sim, 1.0);
    assert!(sim.turbine.p_elec < 0.0 && sim.snapshot().turbine.gross_mwe < 0.0, "snapshot reports the signed value");
}

#[test]
fn synchronisation_energy_is_accounted_on_the_breaker_step() {
    let mut tb = Turbine::new(false);
    tb.tripped = false;
    tb.speed_rpm = RATED_RPM - 6.0; // inside the close window, below grid speed
    tb.close_breaker().unwrap();
    let ke_in = tb.kinetic_energy();
    tb.step(DT, 100.0, 7.0e6, 5.6e3, 0.0);
    let ke_out = tb.kinetic_energy();
    assert!((tb.speed_rpm - Turbine::grid_rpm()).abs() < 1e-9);
    // Pull-in energy comes from the grid: steam work + entry KE == exit KE + electric (signed) + losses.
    assert!((tb.p_mech * DT + ke_in - ke_out - (tb.p_elec + tb.p_losses) * DT).abs() < 1.0);
    assert!(tb.p_elec < 0.0, "pull-in from below grid speed is an import this step: {}", tb.p_elec);
}

#[test]
fn extraction_heat_is_bounded_by_steam_availability_and_stored_in_the_heaters() {
    use bwr_core::steam::h_liquid;
    let (w_feed, t_hw) = (470.0, 35.0);
    let h_hw = h_liquid(t_hw);
    // Rated turbine flow: the demand is met and the node settles at the design feed temperature.
    let mut fw = Feedwater::new(2, T_FW_RATED, LevelControlMode::ThreeElement, 0.7);
    for _ in 0..(600.0 / DT) as usize {
        fw.step_heaters(DT, w_feed, STEAM_FLOW_RATED, 7.0e6, t_hw);
    }
    let t_design = t_hw + (T_FW_RATED - 35.0) + 2.0; // baseline shape: hotwell + heating + pump rise
    assert!((fw.t_feed - t_design).abs() < 0.5, "t_feed {} vs {t_design}", fw.t_feed);
    assert!(fw.q_extraction_applied_w > 300.0e6 && fw.q_extraction_applied_w < 420.0e6, "{}", fw.q_extraction_applied_w);
    assert!((fw.pump_work_w - w_feed * 4186.0 * 2.0).abs() < 1.0);
    assert!(fw.q_extraction_applied_w < EXTRACTION_FLOW_FRACTION_MAX * STEAM_FLOW_RATED * (sat_p(7.0e6).h_g - h_hw), "within availability");
    // Steam gone (turbine tripped): no extraction; the node discharges into the feed stream
    // over ~FW_HEATER_TAU_S and every joule the stream gains came out of storage.
    let e0 = fw.heater_energy;
    let mut q_stream = 0.0;
    for _ in 0..(180.0 / DT) as usize {
        let h_feed_entry = fw.h_feed();
        fw.step_heaters(DT, w_feed, 0.0, 7.0e6, t_hw);
        assert_eq!(fw.q_extraction_applied_w, 0.0, "no steam, no extraction");
        q_stream += (w_feed * (h_feed_entry - h_hw) - fw.pump_work_w) * DT;
    }
    assert!(((e0 - fw.heater_energy) - q_stream).abs() < 1e-6 * e0, "storage discharge must equal the stream's gain");
    let expect = t_hw + 2.0 + (t_design - 2.0 - t_hw) * (-1.0f64).exp();
    assert!((fw.t_feed - expect).abs() < 8.0, "after one time constant t_feed {} ~ {expect}", fw.t_feed);
    // Low steam: demand exceeds availability, so the applied value IS the bound.
    let mut fw = Feedwater::new(2, 60.0, LevelControlMode::ThreeElement, 0.7);
    fw.step_heaters(DT, w_feed, 20.0, 7.0e6, t_hw);
    let avail = EXTRACTION_FLOW_FRACTION_MAX * 20.0 * (sat_p(7.0e6).h_g - h_hw);
    assert!((fw.q_extraction_applied_w - avail).abs() < 1.0, "applied {} must equal availability {avail}", fw.q_extraction_applied_w);
    // No feed flow: nothing is demanded and the stored heat stays put.
    let e1 = fw.heater_energy;
    fw.step_heaters(DT, 0.0, STEAM_FLOW_RATED, 7.0e6, t_hw);
    assert_eq!(fw.q_extraction_applied_w, 0.0);
    assert_eq!(fw.heater_energy, e1);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --release -p bwr-core --test power_conversion`
Expected: compile error `no field balance on type Sim` / `no method step_heaters`.

- [ ] **Step 3: Parameters and their register rows**

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
/// Largest fraction of the turbine steam that can be extracted for feedwater heating:
/// the steam-side availability bound on the applied extraction heat. ASSUMED (A-6xx), interval 0.25..0.45.
pub const EXTRACTION_FLOW_FRACTION_MAX: f64 = 0.35;
```

In the feedwater section add:

```rust
/// Feedwater heater train response time, s (was the literal 180.0 in Feedwater::step). ASSUMED (A-6xx).
pub const FW_HEATER_TAU_S: f64 = 180.0;
/// Lumped thermal capacity of the heater train (metal, shells, drains), J/K: the storage
/// whose discharge through the rated feed stream has the FW_HEATER_TAU_S time constant. DERIVED.
pub const FW_HEATER_CAPACITY_J_PER_K: f64 = FW_HEATER_TAU_S * STEAM_FLOW_RATED * 4186.0;
```

In `docs/model/parameters.csv` UPDATE the existing `TURBINE_DH_RATED` row in place (do not append a second one): value `0.897e6`, status `calibrated`, notes `was 0.703e6 effective (included extraction); recalibrated so net stays ~300 MWe with explicit extraction work`. Append:

```
EXTRACTION_WORK_FRACTION,0.25,-,LP work forgone per unit extraction heat to feedwater heaters,rated,generic BWR cycle,assumed,,0.15,0.35,A-6xx,crates/bwr-core/src/params.rs,0.35 would push TURBINE_DH_RATED above the isentropic drop
EXTRACTION_FLOW_FRACTION_MAX,0.35,-,maximum fraction of turbine steam available for extraction heating,rated,generic BWR cycle,assumed,,0.25,0.45,A-6xx,crates/bwr-core/src/params.rs,steam-side bound on q_extraction_applied_w
FW_HEATER_TAU_S,180.0,s,feedwater heater train response time,rated,generic,assumed,,90,360,A-6xx,crates/bwr-core/src/params.rs,was a literal in Feedwater::step
FW_HEATER_CAPACITY_J_PER_K,3.54e8,J/K,lumped thermal capacity of the feedwater heater train,rated,generic,derived,,1e8,1e9,A-6xx,crates/bwr-core/src/params.rs,FW_HEATER_TAU_S*STEAM_FLOW_RATED*4186
```

Run `cargo test --release -p bwr-core --test register` → passes (every constant above has its row).

- [ ] **Step 4: Turbine extraction, signed output, losses and shaft KE in `bop.rs`**

Add `pub p_extraction: f64,` and `pub p_losses: f64,` to `Turbine` (both `0.0` in `new`). Replace the whole `Turbine::step` (the Task 2a guard stays first and gains the new argument) and add `kinetic_energy`:

```rust
    /// Shaft kinetic energy, J: the master's `d/dt(shaft kinetic energy)` storage term.
    pub fn kinetic_energy(&self) -> f64 {
        let omega = self.speed_rpm * std::f64::consts::PI / 30.0;
        0.5 * TG_INERTIA * omega * omega
    }

    /// Advance shaft dynamics. `extraction_heat_w` is the APPLIED extraction heat
    /// (`Feedwater.q_extraction_applied_w`); the LP work that steam would have done is forgone.
    /// Energy identity every step, both branches:
    ///   p_mech*dt + KE_entry == KE_exit + p_elec*dt + p_losses*dt
    /// with p_elec SIGNED (negative = grid import) and p_losses never clipped.
    pub fn step(&mut self, dt: f64, w_turbine: f64, p_chest: f64, p_cond: f64, extraction_heat_w: f64) {
        if let Err(e) = check_step_args(
            dt,
            &[("w_turbine", w_turbine), ("p_chest", p_chest), ("p_cond", p_cond), ("extraction_heat_w", extraction_heat_w)],
        ) {
            self.last_error = Some(e);
            return;
        }
        let pf = if p_chest > p_cond && p_cond > 0.0 {
            ((p_chest / p_cond).ln() / (7.0e6f64 / 5.6e3).ln()).clamp(0.0, 1.2)
        } else {
            0.0
        };
        let gross = w_turbine * TURBINE_DH_RATED * pf;
        self.p_extraction = (EXTRACTION_WORK_FRACTION * extraction_heat_w).min(gross);
        self.p_mech = gross - self.p_extraction;
        let w_frac = self.speed_rpm / RATED_RPM;
        let friction = TG_LOSSES * w_frac * w_frac + 0.5e6 * w_frac.min(1.0) * (self.speed_rpm > 1.0) as i32 as f64;
        let ke_in = self.kinetic_energy();

        if self.breaker_closed {
            self.speed_rpm = Self::grid_rpm();
            self.phase_deg = 0.0;
            let ke_out = self.kinetic_energy();
            // Signed generator output: shaft power left after friction and the grid pull-in
            // (synchronisation) energy. Negative = the grid motors the set.
            self.p_elec = (self.p_mech - friction + (ke_in - ke_out) / dt) * GENERATOR_EFF;
            if self.p_elec < 0.0 {
                self.reverse_power_timer += dt;
                if self.reverse_power_timer > 3.0 {
                    self.breaker_closed = false;
                }
            } else {
                self.reverse_power_timer = 0.0;
            }
        } else {
            self.p_elec = 0.0;
            let e = (ke_in + (self.p_mech - friction) * dt).max(0.0);
            let omega = (2.0 * e / TG_INERTIA).sqrt();
            self.speed_rpm = omega * 30.0 / std::f64::consts::PI;
            if self.speed_rpm < 0.5 && self.p_mech <= 0.0 {
                self.speed_rpm = 0.0;
            }
            self.phase_deg += 360.0 * self.slip_hz() * dt;
            self.phase_deg = (self.phase_deg + 180.0).rem_euclid(360.0) - 180.0;
        }
        // Dissipation from the actual energy change: entry KE + steam work - exit KE - electric.
        // On the last coastdown step (e clamped at 0) this is all of the remaining entry energy.
        self.p_losses = self.p_mech - self.p_elec + (ke_in - self.kinetic_energy()) / dt;
        if self.speed_rpm > OVERSPEED_TRIP_RPM {
            self.trip("Turbine overspeed");
        }
    }
```

Add after the `Turbine` impl, with the master's control-volume comment quoted verbatim:

```rust
/// Whole-plant energy balance, W, evaluated every step. Control volume: vessel + steam lines + turbine + condenser/hotwell + feed train + IC tubes.
/// Sign convention: positive = into the control volume. residual = in - out - storage_rate.
///   in      = q_core + makeup_flow*makeup_h + house_pump_work (fluid pump work, counted once)
///   out     = q_condenser (= q_cw_w, to cooling water) + q_ic (pool-side absorption) + q_loss + p_elec_gross (signed) + p_losses
///             + reject_flow*reject_h (incl. overflow to a full CST) + w_vent*h_g + q_line_wall_w
///   storage = d/dt(vessel.energy) + d/dt(line.energy) + d/dt(condenser.energy) + d/dt(fw.heater_energy) + d/dt(shaft kinetic energy)
///             + d/dt(ic tube energy, P7)   (finite differences over the step)
/// p_extraction is NOT a boundary term (it is internal: steam -> feedwater); it is reported for reconciliation only.
/// Package 7 adds d/dt(ic tube energy) to this same storage sum; nothing else in the balance changes.
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
    pub q_line_wall_w: f64,
    pub storage_w: f64,
    pub residual_w: f64,
}
```

- [ ] **Step 5: Heater storage node in `bop.rs`**

Add `use crate::ic::CP_WATER;` (P3 constant). Add to `Feedwater`:

```rust
    /// J stored in the heater train (metal, shells, drains), datum 0 degC: the 180 s feed
    /// temperature lag is this storage, so heat delivered after steam loss is accounted.
    pub heater_energy: f64,
    /// W; the ONE extraction value used by the turbine, feedwater and condenser equations.
    pub q_extraction_applied_w: f64,
    /// W; the 2 K pump term as fluid work, counted once (excluded from extraction heat).
    pub pump_work_w: f64,
```

In `new`, with `pumps_running` already in scope: `heater_energy: FW_HEATER_CAPACITY_J_PER_K * (t_feed - if pumps_running > 0 { 2.0 } else { 0.0 }), q_extraction_applied_w: 0.0, pump_work_w: 0.0`. Add:

```rust
    /// Heater-node temperature, degC (the feed temperature before the pump rise).
    pub fn heater_t(&self) -> f64 {
        self.heater_energy / FW_HEATER_CAPACITY_J_PER_K
    }

    /// Feedwater heater train as a stored-energy node (master step 11, before `Turbine::step`).
    /// `w_feed_applied` is `flows.w_feed`; `w_turbine_applied` is the line node's applied `w_tcv`.
    /// Sets the ONE extraction value (demand bounded by steam-side availability), the pump
    /// work (counted once), and advances `heater_energy` and `t_feed`.
    pub fn step_heaters(&mut self, dt: f64, w_feed_applied: f64, w_turbine_applied: f64, p_chest: f64, hotwell_t: f64) {
        if let Err(e) = check_step_args(
            dt,
            &[("w_feed_applied", w_feed_applied), ("w_turbine_applied", w_turbine_applied), ("p_chest", p_chest), ("hotwell_t", hotwell_t)],
        ) {
            self.last_error = Some(e);
            return;
        }
        // TEMPORARY condition (Task 8 Step 4 replaces it with pump_speed.iter().any(|&s| s > 0.01)).
        let pumps_running = self.running_pumps() > 0;
        let pump_dt = if pumps_running { 2.0 } else { 0.0 };
        self.pump_work_w = w_feed_applied * CP_WATER * pump_dt;
        let h_hw = steam::h_liquid(hotwell_t);
        // Demand: heat needed beyond pump work to deliver feed at the design temperature for
        // the current turbine load (baseline shape: (T_FW_RATED - 35) * load^0.75). Extraction
        // cannot cool the feed, so the demand floor is zero.
        let load = (w_turbine_applied / STEAM_FLOW_RATED).clamp(0.0, 1.2);
        let full = (T_FW_RATED - 35.0) * load.powf(0.75);
        let rise = if self.heaters_in_service { full } else { 0.65 * full };
        let q_demand = (w_feed_applied * (steam::h_liquid(hotwell_t + rise + pump_dt) - h_hw) - self.pump_work_w).max(0.0);
        // Steam-side availability: at most EXTRACTION_FLOW_FRACTION_MAX of the turbine steam,
        // condensed and drained to hotwell temperature.
        let q_avail = EXTRACTION_FLOW_FRACTION_MAX * w_turbine_applied * (sat_p(p_chest.max(P_ATM)).h_g - h_hw);
        self.q_extraction_applied_w = q_demand.min(q_avail);
        // Stream: what the feed carries out of the node this step, valued at the enthalpies the
        // vessel (h_feed at entry) and the condenser (h_hw at entry) use, less the pump's share.
        let q_stream = w_feed_applied * (self.h_feed() - h_hw) - self.pump_work_w;
        self.heater_energy += dt * (self.q_extraction_applied_w - q_stream);
        self.t_feed = self.heater_t() + pump_dt;
    }
```

In `Feedwater::step`, delete the block from `// Feedwater heating from turbine extraction steam.` through `self.t_feed += ...;` (the lag now lives in the node), remove the `turbine_flow` and `hotwell_t` parameters, and shrink the Task 2a guard to `&[("p_reactor", p_reactor), ("level_cm", level_cm), ("steam_flow", steam_flow)]`. Update the Task 2a test's `fw.step(..)` calls and rejection rows to the five-argument form (drop the `turbine_flow`/`hotwell_t` rows).

- [ ] **Step 6: Assemble in `sim.rs`**

Add `pub balance: EnergyBalance,` to `Sim` (`EnergyBalance::default()` in `base`). Add:

```rust
    /// Auxiliary electrical load, W (motor input; outside the control volume).
    pub fn house_load(&self) -> f64 {
        HOUSE_BASE
            + self.fw.running_pumps() as f64 * FW_PUMP_POWER
            + self.fw.cond_pumps.iter().filter(|&&b| b).count() as f64 * COND_PUMP_POWER
            + self.cond.cw_pumps.iter().filter(|&&b| b).count() as f64 * CW_PUMP_POWER
    }
```

and in `snapshot()` replace the inline `let house = HOUSE_BASE + ...;` with `let house = self.house_load();` (`gross_mwe` reports the signed `p_elec`; `net_mwe = gross - house`).

At the top of `Sim::step`, right after P3's `dt` guard, capture the stored energy of the control volume (the condenser line is the Task 7 temporary):

```rust
        let stored_before = self.vessel.energy
            + self.steam.line.energy
            + self.cond.hotwell_mass * steam::h_liquid(self.cond.hotwell_t()) // TEMPORARY: Task 7 -> self.cond.energy
            + self.fw.heater_energy
            + self.turbine.kinetic_energy();
        // + IC tube energy from Package 7 (0 until then)
```

Step 7: the feedwater call becomes `self.fw.step(dt, p, self.level_cm(), self.steam.vessel_steam_flow(), hotwell_ok)` (Task 7 inserts `bound_feed_to_hotwell` right after it; Task 8 appends `&ff`). Steps 8-10 stay as Task 3 wrote them. Replace the Task 3 turbine call and the whole `// --- condenser` block with master steps 11-12 plus the balance:

```rust
        // 11. heaters (the ONE applied extraction value) then the turbine
        let hotwell_t = self.cond.hotwell_t(); // entry state: shared by the heater stream and the condenser withdrawal
        self.fw.step_heaters(dt, flows.w_feed, self.steam.w_tcv, self.steam.p_chest, hotwell_t);
        let q_x = self.fw.q_extraction_applied_w;
        let p_cond = self.cond.pressure();
        self.turbine.step(dt, self.steam.w_tcv, self.steam.p_chest, p_cond, q_x);

        // 12. condenser: applied flows only; signed inlet load (no clamp)
        let h_hw = steam::h_liquid(hotwell_t);
        let w_to_bop = self.steam.w_tcv + self.steam.w_bypass; // the node's applied outflow
        let w_in = w_to_bop + flows.w_liquid_out;              // plus the applied RWCU reject
        // Enthalpy the streams give up beyond shaft work and the heat that went to the heaters,
        // relative to the hotwell condition.
        let q_in = w_to_bop * h_line_out + flows.w_liquid_out * flows.h_liquid_out - self.turbine.p_mech - q_x - w_in * h_hw;
        self.cond.step(dt, q_in, w_in, flows.w_feed); // the condenser withdraws the APPLIED feed; Task 7 appends &cf

        // --- plant energy balance (master control volume; see EnergyBalance doc comment)
        let stored_after = self.vessel.energy
            + self.steam.line.energy
            + self.cond.hotwell_mass * steam::h_liquid(self.cond.hotwell_t()) // TEMPORARY: Task 7 -> self.cond.energy
            + self.fw.heater_energy
            + self.turbine.kinetic_energy();
        let mut b = EnergyBalance {
            q_core_w: flows.q_core,
            q_condenser_w: self.cond.heat_load, // TEMPORARY: Task 7 -> self.cond.q_cw_w
            q_ic_w: flows.q_ic,                 // P7 replaces with pool-side absorption + tube storage
            q_loss_w: flows.q_loss,
            p_elec_gross_w: self.turbine.p_elec, // signed: import is reported, not clipped
            p_losses_w: self.turbine.p_losses,
            p_extraction_w: self.turbine.p_extraction,
            house_pump_work_w: self.fw.pump_work_w,
            makeup_w: 0.0, // TEMPORARY: Task 7 -> self.cond.makeup_flow * self.cond.makeup_h
            reject_w: 0.0, // TEMPORARY: Task 7 -> self.cond.reject_flow * self.cond.reject_h
            vent_w: flows.w_vent * flows.h_steam,
            q_line_wall_w: self.steam.line.q_wall_w,
            storage_w: (stored_after - stored_before) / dt,
            residual_w: 0.0,
        };
        b.residual_w = (b.q_core_w + b.makeup_w + b.house_pump_work_w)
            - (b.q_condenser_w + b.q_ic_w + b.q_loss_w + b.p_elec_gross_w + b.p_losses_w + b.reject_w + b.vent_w + b.q_line_wall_w)
            - b.storage_w;
        self.balance = b;
```

Closure argument (write it into `docs/model/power-conversion.md` "Energy balance"): every internal stream cancels algebraically. The vessel gains `w_feed*h_feed` at the entry `t_feed` and the heater node is charged with the same `h_feed`; the condenser gives up `w_feed*h_hw` at its entry temperature and the heater stream is valued at the same `h_hw`; the line node discharges at `h_line_out` and the condenser load uses the same value; `q_x` leaves the turbine stream and enters the heater node; `p_mech` is split by the turbine identity into `p_elec`, `p_losses` and `d(KE)/dt`. What remains in the residual at Task 6 is only the Task 7 temporary (the baseline condenser integrates no energy, so its proxy storage carries the legacy +/-30 kg/s makeup as `30 kg/s * h_hw ~ 4 MW` whenever it runs); after Task 7 the residual is the rounding of the finite differences. Nothing in the balance is a tolerance to be adjusted: a closure failure is a missing transfer or storage term.

- [ ] **Step 7: Run all tests**

Run: `cargo test --release -p bwr-core`
Expected: all pass; `full_power_is_steady` net MWe within 285-315 (if not, the calibration arithmetic in Step 3 is wrong: recompute `TURBINE_DH_RATED` from the printed `extraction` value so that net returns to the baseline value, and record both numbers in the register row's notes). Then compare mode: `cargo run --release -p bwr-core --example validation`, append its output under `## Task 6: extraction work, heater storage, signed output` in `docs/validation/reports/2026-10-21-pkg06.md` with each changed metric classified (physical: `net_mwe` recalibration path, signed `gross_mwe` during trips), then `--write` once.

- [ ] **Step 8: Document and commit**

Append the "Energy balance" section (term definitions, the closure argument above, the printed full-power table, old and new `TURBINE_DH_RATED`, the house-load vs pump-work vs losses statement) to `docs/model/power-conversion.md`.

```bash
git add crates/bwr-core docs/model docs/validation
git commit -m "feat(sim): control-volume energy balance with heater storage, one applied extraction value, signed generator output

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---
### Task 7: Condenser as a first-law state: explicit CST boundary, air blanketing, cooling-water and vacuum faults, hotwell bound

**Files:**
- Modify: `crates/bwr-core/src/params.rs` (`CST_CAPACITY_KG`, `CST_MASS_INITIAL`, `HOTWELL_MAKEUP_KGS`, `HOTWELL_MAKEUP_ON`, `HOTWELL_REJECT_ON`, `AIR_UA_DEGRADATION`, `CONDENSER_SHELL_CAPACITY_J_PER_K`) and `docs/model/parameters.csv` (rows in this task)
- Modify: `crates/bwr-core/src/domain.rs` (`HotwellExhausted` variant and `reason()` arm)
- Modify: `crates/bwr-core/src/bop.rs` (`CondenserFaults`, `Condenser` fields, `solve_hotwell_t`, `set_hotwell_t`, `step`)
- Modify: `crates/bwr-core/src/sim.rs` (`bound_feed_to_hotwell`, `bop_faults` partial, balance lines, alarms, `SetCwPump`/`SetVacuumPump` refusals, `full_power` hotwell init, `snapshot` `heat_load_mw`)
- Test: `crates/bwr-core/tests/power_conversion.rs`

**Interfaces:**
- Produces (master): `#[derive(Clone, Copy, Debug, Default)] pub struct CondenserFaults { pub cw_trip: [bool; N_CW_PUMPS], pub vacuum_failed: bool }`; `Condenser { energy, q_in_w, q_cw_w, cst_mass, makeup_flow, reject_flow, makeup_h, reject_h, overflow_kg, deficit_kg }` (the baseline `heat_load` field is deleted; `Snapshot.heat_load_mw` reports `q_in_w`); `Condenser::step(&mut self, dt, q, w_in, w_out, faults: &CondenserFaults)`; `DomainError::HotwellExhausted { deficit_kg }` with `reason()` = `hotwell exhausted: feed request exceeded inventory by <deficit> kg`; `Sim::bound_feed_to_hotwell(dt) -> Option<DomainError>` (master step 7). Package-local: `Condenser::set_hotwell_t(&mut self, t)` (consistent `energy`/`t_sat`/`p_steam` initialiser, scenario build only), `Condenser::cst_level_pct()`, `solve_hotwell_t(energy, mass) -> Result<f64, DomainError>`.
- First law: `energy` (condenser + hotwell water + shell metal) advances as `dE = dt*(q_in + (w_in - w_out)*h_hw - q_cw + (makeup - reject)*h_hw)` with every transfer valued at the ENTRY hotwell enthalpy `h_hw = h_liquid(t_sat_entry)`, recorded as `makeup_h`/`reject_h` (A-6xx `H_CST_EXCHANGE`: the CST has no temperature state). `q_in_w = q` is the arriving load (the signed enthalpy the streams give up beyond shaft work and applied extraction, relative to the hotwell condition; no clamp). `q_cw_w = m_cw*cp*eff*(T_hw - CW_INLET_C)` is the actual rejection: signed, zero with no CW pumps. Temperature is DERIVED from `energy` and inventory by `solve_hotwell_t` (bisection of `mass*h_liquid(T) + CONDENSER_SHELL_CAPACITY_J_PER_K*T = energy` on the table bounds); condensing steam is in equilibrium with the hotwell water (perfect mixing, A-6xx), so `p_steam = sat_t(T_hw).p`. Nothing is lagged toward a target; with CW lost the energy accumulates and the pressure rises until the low-vacuum trip.
- Hotwell rule (master step 7): `Sim::bound_feed_to_hotwell(dt)` bounds `fw.flow = min(request, hotwell_mass/dt)` BEFORE the vessel allocation, records the shortfall in `cond.deficit_kg` and returns `HotwellExhausted` (held locally, drained at step 17 with the other errors), so the applied `w_feed` is the same everywhere (vessel, condenser withdrawal, heater stream). `Condenser::step` therefore never sees `w_out*dt > hotwell_mass + w_in*dt`; a direct caller that asks for it is rejected transactionally (`last_error = PropertyOutOfRange { what: "w_out" }`). The hotwell mass is never set to zero from a negative value. Overflow above `HOTWELL_CAPACITY` leaves with the reject stream: `reject_flow` includes it and `reject_h` values it, the CST takes what fits, and the remainder is counted in `overflow_kg` (a mass AND energy boundary loss, through `reject_w`). A depleted CST limits makeup to `cst_mass/dt`.
- `EnergyBalance` (replacing the Task 6 temporaries): `q_condenser_w = cond.q_cw_w`, `makeup_w = makeup_flow*makeup_h`, `reject_w = reject_flow*reject_h`, storage uses `cond.energy`.

- [ ] **Step 1: Write the failing tests**

Append to `tests/power_conversion.rs`:

```rust
use bwr_core::bop::CondenserFaults;

#[test]
fn hotwell_makeup_and_reject_are_explicit_and_balanced_at_steady_state() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 60.0);
    let cst0 = sim.cond.cst_mass;
    let mut net = 0.0;
    for _ in 0..(600.0 / DT) as usize {
        sim.step(DT);
        net += (sim.cond.makeup_flow - sim.cond.reject_flow) * DT;
        assert!(sim.cond.makeup_h >= bwr_core::steam::h_liquid(CW_INLET_C) && sim.cond.reject_h == sim.cond.makeup_h);
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
    let total = |s: &Sim| s.vessel.mass + s.steam.line.mass + s.cond.hotwell_mass + s.cond.cst_mass;
    let t0 = total(&sim);
    let overflow0 = sim.cond.overflow_kg;
    let mut vented = 0.0;
    sim.apply(Command::TripTurbine);
    for _ in 0..(600.0 / DT) as usize {
        sim.step(DT);
        vented += sim.last_flows().w_vent * DT;
    }
    // Physical closure: the inventory change equals what left through the two declared exits
    // (head vent, overflow past a full CST). No fictitious supplied-water counter.
    let left = vented + (sim.cond.overflow_kg - overflow0);
    assert!(((total(&sim) - t0) + left).abs() < 1.0, "plant water inventory drifted by {} kg", total(&sim) - t0 + left);
    assert_eq!(sim.cond.deficit_kg, 0.0, "a bypass trip must not run the hotwell dry");
    assert!(!sim.domain_errors.iter().any(|(_, e)| matches!(e, DomainError::HotwellExhausted { .. })));
}

#[test]
fn hotwell_exhaustion_bounds_feed_and_is_reported() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    let request = sim.fw.flow;
    sim.cond.hotwell_mass = 0.5 * request * DT; // half a step of feed left
    sim.cond.set_hotwell_t(sim.cond.hotwell_t()); // keep energy consistent with the new mass
    let v_mass0 = sim.vessel.mass;
    sim.step(DT);
    let f = sim.last_flows();
    assert!(f.w_feed <= 0.5 * request + 1e-9, "feed must be bounded by the hotwell: {} vs request {request}", f.w_feed);
    assert!(sim.cond.hotwell_mass >= 0.0, "never negative, never clamped from negative");
    assert!(sim.cond.deficit_kg > 0.0);
    let (t, e) = sim.domain_errors.last().expect("HotwellExhausted drained at step 17");
    assert!(matches!(e, DomainError::HotwellExhausted { deficit_kg } if *deficit_kg > 0.0), "{e:?}");
    assert_eq!(*t, sim.time);
    assert!(e.reason().starts_with("hotwell exhausted:"));
    assert!(sim.snapshot().unsupported.is_some());
    // The vessel received exactly the bounded flow: no mass was created to cover the shortfall.
    let gained = sim.vessel.mass - v_mass0;
    let expect = (f.w_feed - f.w_steam - f.w_vent - f.w_liquid_out) * DT;
    assert!((gained - expect).abs() < 1e-6, "vessel gained {gained} vs applied {expect}");
}

#[test]
fn hotwell_inventory_tracks_applied_transfers_every_step() {
    // Master steps 9-12: the condenser withdraws the APPLIED feed and receives the node's
    // applied steam and the vessel's applied RWCU reject, nothing else.
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    sim.apply(Command::SetRwcuReject { kgs: 10.0 });
    sim.apply(Command::TripTurbine);
    for _ in 0..(60.0 / DT) as usize {
        let hw0 = sim.cond.hotwell_mass;
        sim.step(DT);
        let f = sim.last_flows();
        let expect = (sim.steam.w_tcv + sim.steam.w_bypass + f.w_liquid_out - f.w_feed + sim.cond.makeup_flow - sim.cond.reject_flow) * DT;
        assert!((sim.cond.hotwell_mass - hw0 - expect).abs() < 1e-6, "hotwell moved {} vs applied {}", sim.cond.hotwell_mass - hw0, expect);
    }
}

#[test]
fn condenser_energy_closes_during_cw_loss_and_vacuum_degradation() {
    let none = CondenserFaults::default();
    let identity = |c0: &Condenser, c1: &Condenser, q: f64, w_in: f64, w_out: f64| -> f64 {
        let h = bwr_core::steam::h_liquid(c0.hotwell_t());
        (c1.energy - c0.energy) - DT * (q + (w_in - w_out) * h - c1.q_cw_w + (c1.makeup_flow - c1.reject_flow) * h)
    };
    // CW lost: no rejection; energy accumulates; temperature and pressure rise monotonically.
    let mut c = Condenser::new(true);
    c.set_hotwell_t(35.0);
    let e0 = c.energy;
    c.cw_pumps = [false; N_CW_PUMPS];
    let (mut t_prev, mut p_prev) = (c.hotwell_t(), c.pressure());
    for _ in 0..(60.0 / DT) as usize {
        let c0 = c.clone();
        c.step(DT, 5.0e8, 470.0, 470.0, &none);
        assert_eq!(c.q_cw_w, 0.0, "no cooling water, no rejection");
        assert!(identity(&c0, &c, 5.0e8, 470.0, 470.0).abs() < 1.0);
        assert!(c.hotwell_t() > t_prev && c.pressure() >= p_prev, "monotone rise");
        t_prev = c.hotwell_t();
        p_prev = c.pressure();
        assert!(c.last_error.is_none());
    }
    assert!((c.energy - e0 - 60.0 * 5.0e8).abs() < 1.0e3, "60 s x 500 MW stored, nothing else");
    // Vacuum pump failed: air accumulates, UA degrades, the identity still closes.
    let mut c = Condenser::new(true);
    c.set_hotwell_t(35.0);
    c.step(DT, 5.0e8, 470.0, 470.0, &none);
    let (q_cw_clean, m_air0) = (c.q_cw_w, c.m_air);
    let faulted = CondenserFaults { cw_trip: [false; N_CW_PUMPS], vacuum_failed: true };
    for _ in 0..(600.0 / DT) as usize {
        let c0 = c.clone();
        c.step(DT, 5.0e8, 470.0, 470.0, &faulted);
        assert!(identity(&c0, &c, 5.0e8, 470.0, 470.0).abs() < 1.0);
    }
    assert!(!c.vacuum_pump && c.m_air > m_air0);
    assert!(c.pressure() > sat_t(c.hotwell_t()).p, "air adds to the total pressure");
    println!("vacuum degradation: q_cw {:.1} -> {:.1} MW at T {:.1}", q_cw_clean / 1e6, c.q_cw_w / 1e6, c.hotwell_t());
}

#[test]
fn energy_balance_closes_through_turbine_trip_cw_loss_and_vacuum_degradation() {
    // Master closure gate: |residual| < 1 % of rated through the first 60 s of each transient.
    // The storage terms (line, condenser energy, heater node, shaft KE, vessel) absorb the swing.
    let cases: [(&str, fn(&mut Sim)); 3] = [
        ("fp_turbine_trip", |s| s.apply(Command::TripTurbine)),
        ("cw_loss", |s| {
            s.set_fault(Fault::CwPumpTrip { index: 0 }, true);
            s.set_fault(Fault::CwPumpTrip { index: 1 }, true);
        }),
        ("vacuum_degradation", |s| s.set_fault(Fault::VacuumPumpFailed, true)),
    ];
    for (name, start) in cases {
        let mut sim = Sim::new(ScenarioName::FullPower);
        run(&mut sim, 5.0);
        start(&mut sim);
        let mut worst = 0.0f64;
        for _ in 0..(60.0 / DT) as usize {
            sim.step(DT);
            if sim.balance.residual_w.abs() > worst.abs() {
                worst = sim.balance.residual_w;
                print_balance(&sim.balance);
            }
        }
        assert!(worst.abs() < 0.01 * RATED_POWER_W, "{name}: worst transient residual {} MW", worst / 1e6);
    }
}

#[test]
fn full_cst_overflow_is_counted_as_a_boundary_loss() {
    let mut c = Condenser::new(true);
    c.cst_mass = CST_CAPACITY_KG;
    c.hotwell_mass = 0.9 * HOTWELL_CAPACITY; // above HOTWELL_REJECT_ON
    c.set_hotwell_t(35.0);
    let (e0, h) = (c.energy, bwr_core::steam::h_liquid(35.0));
    c.step(DT, 0.0, 600.0, 0.0, &CondenserFaults::default()); // 600 kg/s in, nothing withdrawn
    assert_eq!(c.cst_mass, CST_CAPACITY_KG, "a full CST takes nothing");
    assert!(c.reject_flow >= HOTWELL_MAKEUP_KGS, "reject runs: {}", c.reject_flow);
    assert!((c.overflow_kg - c.reject_flow * DT).abs() < 1e-9, "everything rejected overflowed");
    assert_eq!(c.reject_h, h, "recorded at the transfer-time enthalpy");
    assert!(c.hotwell_mass <= HOTWELL_CAPACITY + 1e-9);
    // The overflow's energy left the control volume at reject_h.
    let e_expect = e0 + DT * (600.0 * h - c.q_cw_w - c.reject_flow * c.reject_h);
    assert!((c.energy - e_expect).abs() < 1.0);
    let mut sim = Sim::new(ScenarioName::FullPower);
    sim.cond.cst_mass = CST_CAPACITY_KG;
    sim.cond.hotwell_mass = 0.99 * HOTWELL_CAPACITY;
    sim.cond.set_hotwell_t(sim.cond.hotwell_t());
    sim.apply(Command::SetRwcuReject { kgs: RWCU_REJECT_MAX_KGS });
    run(&mut sim, 60.0);
    assert!(sim.cond.overflow_kg > 0.0);
    assert!(sim.snapshot().alarms.iter().any(|a| a.id == "hotwell_overflow"));
}

#[test]
fn depleted_cst_limits_makeup() {
    let mut c = Condenser::new(true);
    c.hotwell_mass = 0.2 * HOTWELL_CAPACITY; // below HOTWELL_MAKEUP_ON
    c.set_hotwell_t(35.0);
    c.cst_mass = 0.5 * HOTWELL_MAKEUP_KGS * DT; // half a step of makeup left
    c.step(DT, 0.0, 0.0, 0.0, &CondenserFaults::default());
    assert!((c.makeup_flow - 0.5 * HOTWELL_MAKEUP_KGS).abs() < 1e-9, "makeup bounded by the CST: {}", c.makeup_flow);
    assert!(c.cst_mass.abs() < 1e-9, "CST empty, not negative");
    c.step(DT, 0.0, 0.0, 0.0, &CondenserFaults::default());
    assert_eq!(c.makeup_flow, 0.0);
    let mut sim = Sim::new(ScenarioName::FullPower);
    sim.cond.cst_mass = 0.05 * CST_CAPACITY_KG;
    sim.step(DT);
    assert!(sim.snapshot().alarms.iter().any(|a| a.id == "cst_low"), "cst_low alarm");
}

#[test]
fn direct_overdraw_of_the_hotwell_is_rejected_not_clamped() {
    let mut c = Condenser::new(true);
    c.set_hotwell_t(35.0);
    let before = format!("{c:?}");
    c.step(DT, 0.0, 0.0, 2.0 * c.hotwell_mass / DT, &CondenserFaults::default());
    assert!(matches!(c.last_error.take(), Some(DomainError::PropertyOutOfRange { what, .. }) if what == "w_out"));
    assert_eq!(format!("{c:?}"), before, "state unchanged");
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
    assert!(sim.has_fault(&Fault::CwPumpTrip { index: 0 }) && sim.has_fault(&Fault::CwPumpTrip { index: 1 }), "faults retained");
    assert!(sim.log.iter().any(|(_, t)| t.starts_with("command ignored:") && t.contains("CW pump")), "refusal logged: {:?}", sim.log);
    assert_eq!(sim.cond.q_cw_w, 0.0, "no heat sink without cooling water");
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
    assert!(sim.has_fault(&Fault::VacuumPumpFailed), "fault retained");
    assert!(sim.log.iter().any(|(_, t)| t.starts_with("command ignored:") && t.contains("Vacuum pump")));
    assert!(last > p0 + 1.0e3, "4 h of in-leakage should raise condenser pressure by > 1 kPa: {} -> {}", p0, last);
    println!("vacuum_pump_failed: {} -> {} Pa over 4 h", p0, last);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --release -p bwr-core --test power_conversion`
Expected: compile error `no field cst_mass` / `no variant HotwellExhausted`.

- [ ] **Step 3: Parameters and their register rows**

In `params.rs` condenser section, after `pub const HOTWELL_CAPACITY: f64 = 120_000.0;` add:

```rust
/// Hotwell makeup / reject pump rate to and from the condensate storage tank, kg/s (was a literal). ASSUMED.
pub const HOTWELL_MAKEUP_KGS: f64 = 30.0;
/// Hotwell fraction below which makeup runs and above which reject runs (were literals). ASSUMED.
pub const HOTWELL_MAKEUP_ON: f64 = 0.40;
pub const HOTWELL_REJECT_ON: f64 = 0.85;
/// Condensate storage tank: declared boundary inventory, kg. ASSUMED (A-6xx).
pub const CST_CAPACITY_KG: f64 = 2.0e6;
pub const CST_MASS_INITIAL: f64 = 1.5e6;
/// Condenser UA is divided by (1 + AIR_UA_DEGRADATION * p_air / p_steam): air blanketing. ASSUMED (A-6xx), sensitivity 2..10.
pub const AIR_UA_DEGRADATION: f64 = 5.0;
/// Tube-bundle and shell metal capacity lumped with the hotwell water, J/K. ASSUMED (A-6xx), interval 2e7..2e8.
pub const CONDENSER_SHELL_CAPACITY_J_PER_K: f64 = 5.0e7;
```

Append to `docs/model/parameters.csv`:

```
HOTWELL_MAKEUP_KGS,30.0,kg/s,hotwell makeup / reject rate to the CST,n/a,generic,assumed,,10,60,A-6xx,crates/bwr-core/src/params.rs,was a literal
HOTWELL_MAKEUP_ON,0.40,-,hotwell fraction below which makeup runs,n/a,generic,assumed,,0.3,0.5,A-6xx,crates/bwr-core/src/params.rs,was a literal
HOTWELL_REJECT_ON,0.85,-,hotwell fraction above which reject runs,n/a,generic,assumed,,0.7,0.95,A-6xx,crates/bwr-core/src/params.rs,was a literal
CST_CAPACITY_KG,2.0e6,kg,condensate storage tank capacity (declared boundary inventory),n/a,BWRX-300 (assumed),assumed,,1.0e6,4.0e6,A-6xx,crates/bwr-core/src/params.rs,
CST_MASS_INITIAL,1.5e6,kg,initial condensate storage tank inventory,n/a,BWRX-300 (assumed),assumed,,0.5e6,2.0e6,A-6xx,crates/bwr-core/src/params.rs,
AIR_UA_DEGRADATION,5.0,-,condenser UA divisor coefficient on air/steam partial-pressure ratio,n/a,generic,assumed,,2,10,A-6xx,crates/bwr-core/src/params.rs,air blanketing
CONDENSER_SHELL_CAPACITY_J_PER_K,5.0e7,J/K,tube-bundle and shell metal capacity lumped with the hotwell water,n/a,generic,assumed,,2e7,2e8,A-6xx,crates/bwr-core/src/params.rs,first-law condenser state
```

- [ ] **Step 4: `domain.rs` and `bop.rs`**

In `crates/bwr-core/src/domain.rs` add the master variant `HotwellExhausted { deficit_kg: f64 },   // raised by P6 (feed request exceeds hotwell inventory)` to `DomainError` and the arm `DomainError::HotwellExhausted { deficit_kg } => format!("hotwell exhausted: feed request exceeded inventory by {deficit_kg:.1} kg"),` to `reason()`.

In `bop.rs` add above `pub struct Condenser`:

```rust
#[derive(Clone, Copy, Debug, Default)]
pub struct CondenserFaults {
    pub cw_trip: [bool; N_CW_PUMPS],
    pub vacuum_failed: bool,
}

/// Hotwell temperature consistent with `energy` and `mass`: the root of
/// mass*h_liquid(T) + CONDENSER_SHELL_CAPACITY_J_PER_K*T = energy on the table bounds.
fn solve_hotwell_t(energy: f64, mass: f64) -> Result<f64, DomainError> {
    let f = |t: f64| mass * steam::h_liquid(t) + CONDENSER_SHELL_CAPACITY_J_PER_K * t - energy;
    let (mut lo, mut hi) = (T_TABLE_MIN, T_TABLE_MAX);
    if f(lo) > 0.0 || f(hi) < 0.0 {
        return Err(DomainError::PropertyOutOfRange { what: "hotwell_t".into(), value: energy });
    }
    for _ in 0..60 {
        let mid = 0.5 * (lo + hi);
        if f(mid) < 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Ok(0.5 * (lo + hi))
}
```

Replace the `Condenser` struct, `new`, `hotwell_t` and `step` with (keep `p_air`, `pressure`, `available_for_bypass` as they are):

```rust
#[derive(Clone, Debug)]
pub struct Condenser {
    pub m_air: f64,
    /// Steam partial pressure, Pa: sat_t(t_sat).p, DERIVED each step (condensing steam in
    /// equilibrium with the hotwell water, A-6xx).
    pub p_steam: f64,
    pub cw_pumps: [bool; N_CW_PUMPS],
    pub vacuum_pump: bool,
    pub hotwell_mass: f64,
    /// Hotwell / condensing temperature, degC, DERIVED from `energy` and inventory each step.
    pub t_sat: f64,
    /// J: condenser + hotwell water + shell metal, evolved by the first law every step.
    pub energy: f64,
    /// W: thermal load arriving (steam + drains, relative to the hotwell condition); NOT the rejection.
    pub q_in_w: f64,
    /// W: actually rejected to cooling water this step; zero with no CW pumps.
    pub q_cw_w: f64,
    pub cst_mass: f64,
    pub makeup_flow: f64,
    pub reject_flow: f64,
    /// J/kg, recorded at transfer time (A-6xx: CST exchanged at hotwell enthalpy).
    pub makeup_h: f64,
    pub reject_h: f64,
    /// kg rejected past a full CST (boundary loss, counted in reject_flow / reject_h).
    pub overflow_kg: f64,
    /// kg of feed the hotwell could not supply (recorded by Sim::bound_feed_to_hotwell).
    pub deficit_kg: f64,
    pub last_error: Option<DomainError>,
}

impl Condenser {
    pub fn new(vacuum: bool) -> Self {
        let p_air = if vacuum { 0.3e3 } else { P_ATM - 2.3e3 };
        let mut c = Self {
            m_air: p_air * CONDENSER_VOLUME / (287.0 * 303.0),
            p_steam: 0.0,
            cw_pumps: [vacuum; N_CW_PUMPS],
            vacuum_pump: vacuum,
            hotwell_mass: 0.6 * HOTWELL_CAPACITY,
            t_sat: 0.0,
            energy: 0.0,
            q_in_w: 0.0,
            q_cw_w: 0.0,
            cst_mass: CST_MASS_INITIAL,
            makeup_flow: 0.0,
            reject_flow: 0.0,
            makeup_h: 0.0,
            reject_h: 0.0,
            overflow_kg: 0.0,
            deficit_kg: 0.0,
            last_error: None,
        };
        c.set_hotwell_t(CW_INLET_C);
        c
    }

    /// Consistent hotwell state at temperature `t` (scenario build only; steps derive it).
    pub fn set_hotwell_t(&mut self, t: f64) {
        self.t_sat = t;
        self.p_steam = sat_t(t).p;
        self.energy = self.hotwell_mass * steam::h_liquid(t) + CONDENSER_SHELL_CAPACITY_J_PER_K * t;
    }

    pub fn hotwell_t(&self) -> f64 {
        self.t_sat
    }

    pub fn cst_level_pct(&self) -> f64 {
        self.cst_mass / CST_CAPACITY_KG * 100.0
    }

    /// Advance one step. `q` is the arriving thermal load (W, signed, relative to the hotwell
    /// condition), `w_in` water returned to the hotwell, `w_out` condensate withdrawn (already
    /// bounded by `Sim::bound_feed_to_hotwell`), kg/s. Every transfer this step is valued at the
    /// ENTRY hotwell enthalpy; temperature and pressure are then DERIVED from the energy.
    pub fn step(&mut self, dt: f64, q: f64, w_in: f64, w_out: f64, faults: &CondenserFaults) {
        if let Err(e) = check_step_args(dt, &[("q", q), ("w_in", w_in), ("w_out", w_out)]) {
            self.last_error = Some(e);
            return;
        }
        if w_out * dt > self.hotwell_mass + w_in * dt {
            // A direct caller asked for more than the hotwell holds: reject, never clamp to zero.
            self.last_error = Some(DomainError::PropertyOutOfRange { what: "w_out".into(), value: w_out });
            return;
        }
        for i in 0..N_CW_PUMPS {
            if faults.cw_trip[i] {
                self.cw_pumps[i] = false;
            }
        }
        if faults.vacuum_failed {
            self.vacuum_pump = false;
        }
        let t_entry = self.t_sat;
        let h_hw = steam::h_liquid(t_entry);
        self.q_in_w = q;

        // Cooling water is the only heat sink. Air blanketing lowers the effective UA.
        let n_cw = self.cw_pumps.iter().filter(|&&b| b).count() as f64;
        let m_cw = n_cw * CW_FLOW_PER_PUMP;
        let ua = CONDENSER_UA / (1.0 + AIR_UA_DEGRADATION * self.p_air() / self.p_steam.max(700.0));
        self.q_cw_w = if m_cw > 0.0 {
            let mcp = m_cw * CP_WATER;
            let eff = 1.0 - (-ua / mcp).exp();
            mcp * eff * (t_entry - CW_INLET_C)
        } else {
            0.0
        };

        // Hotwell inventory: condensate in/out, then the CST exchange and overflow.
        let mut mass = self.hotwell_mass + (w_in - w_out) * dt;
        let frac = mass / HOTWELL_CAPACITY;
        let mut makeup = if frac < HOTWELL_MAKEUP_ON { HOTWELL_MAKEUP_KGS } else { 0.0 };
        let mut reject = if frac > HOTWELL_REJECT_ON { HOTWELL_MAKEUP_KGS } else { 0.0 };
        makeup = makeup.min(self.cst_mass / dt); // a depleted CST limits makeup
        let excess = mass + (makeup - reject) * dt - HOTWELL_CAPACITY;
        if excess > 0.0 {
            reject += excess / dt; // overflow leaves with the reject stream
        }
        let to_cst = reject.min(((CST_CAPACITY_KG - self.cst_mass) / dt).max(0.0));
        let overflow = reject - to_cst; // past a full CST: a counted boundary loss
        mass += (makeup - reject) * dt;
        self.cst_mass += (to_cst - makeup) * dt;
        self.overflow_kg += overflow * dt;
        self.hotwell_mass = mass;
        self.makeup_flow = makeup;
        self.reject_flow = reject;
        self.makeup_h = h_hw;
        self.reject_h = h_hw;

        // First law for condenser + hotwell water + shell metal.
        self.energy += dt * (q + (w_in - w_out) * h_hw - self.q_cw_w + (makeup - reject) * h_hw);
        match solve_hotwell_t(self.energy, self.hotwell_mass) {
            Ok(t) => {
                self.t_sat = t;
                self.p_steam = sat_t(t).p;
            }
            // Derived state kept; the plant is flagged unsupported through the step-17 drain.
            Err(e) => self.last_error = Some(e),
        }

        // Noncondensables: in-leakage against vacuum, removal by the vacuum pump.
        let p = self.pressure();
        let leak = AIR_INLEAKAGE * ((P_ATM - p) / P_ATM).clamp(0.0, 1.0);
        let removal = if self.vacuum_pump { self.m_air / VACUUM_PUMP_TAU } else { 0.0 };
        self.m_air = (self.m_air + (leak - removal) * dt).max(0.0);
        let max_air = (P_ATM - self.p_steam).max(0.0) * CONDENSER_VOLUME / (287.0 * 303.0);
        self.m_air = self.m_air.min(max_air.max(0.0));
    }
```

(The baseline's `heat_load` field, its non-negative clamp, the lagged `p_steam` relaxation and the `hotwell_t().max(CW_INLET_C)` floor are all gone with this replacement; nothing in P6 reintroduces a clamp on the condenser duty.)

- [ ] **Step 5: Wire into `sim.rs`**

Add a provisional `bop_faults` (Task 8 replaces `()` with `FeedFaults`):

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

Add the master step-7 bound:

```rust
    /// Master step 7: the hotwell can only give what it holds. Bounds the feed REQUEST before
    /// the vessel allocation so the applied w_feed is the same everywhere (vessel, condenser
    /// withdrawal, heater stream). Returns the error to drain at step 17.
    fn bound_feed_to_hotwell(&mut self, dt: f64) -> Option<DomainError> {
        let available = self.cond.hotwell_mass / dt;
        if self.fw.flow <= available {
            return None;
        }
        let deficit_kg = (self.fw.flow - available) * dt;
        self.cond.deficit_kg += deficit_kg;
        self.fw.flow = available;
        Some(DomainError::HotwellExhausted { deficit_kg })
    }
```

In `Sim::step`: `let (sf, cf, _) = self.bop_faults();` at the top (Task 4's `steam_faults()` call becomes this); right after the `fw.step(..)` call add `let hotwell_err = self.bound_feed_to_hotwell(dt);`; the condenser call becomes `self.cond.step(dt, q_in, w_in, flows.w_feed, &cf);`; at step 17 add `if let Some(e) = hotwell_err { self.domain_error(e); }` next to the Task 2a drain. Replace the three Task 6 temporaries: both `self.cond.hotwell_mass * steam::h_liquid(self.cond.hotwell_t())` storage lines become `self.cond.energy`; `q_condenser_w: self.cond.q_cw_w`; `makeup_w: self.cond.makeup_flow * self.cond.makeup_h`; `reject_w: self.cond.reject_flow * self.cond.reject_h`. In `full_power()` replace `s.cond.p_steam = 5.6e3; s.cond.t_sat = 35.0;` with `s.cond.set_hotwell_t(35.0);`. In `snapshot()` change `heat_load_mw: self.cond.heat_load / 1e6` to `heat_load_mw: self.cond.q_in_w / 1e6`.

In `apply`, refuse faulted equipment:

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

`refuse` is a package-local helper that raises the transient operator message AND writes the locked-key log line (master log-line convention, key `command ignored`), so every refused command is visible in `Trace.events`:

```rust
    /// Refuse an operator command: operator message plus a `command ignored:` log line.
    fn refuse(&mut self, id: &str, detail: impl Into<String>) {
        let detail = detail.into();
        self.log.push((self.time, format!("command ignored: {detail}")));
        self.event(id, detail);
    }
```

(If Package 2 or 3 already provides an equivalent helper for ignored commands, call that instead and delete this one; the log text must start with `command ignored:` either way.)

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
Expected: all pass, including the Task 6 steady-state closure now without the legacy-makeup term, and the three transient closure cases. The 4-hour vacuum test runs ~300k steps (about 3 s in release). Then compare mode: `cargo run --release -p bwr-core --example validation`, append the classified output under `## Task 7: first-law condenser, CST boundary` in `docs/validation/reports/2026-10-21-pkg06.md` (physical: condenser pressure now follows energy rather than a 15 s lag; `cst_kg` stays unpopulated until Task 10 Step 5b), then `--write` once.

- [ ] **Step 7: Document and commit**

Append to `docs/model/power-conversion.md` "Results": CW-loss event times (turbine trip, bypass block, scram, IC), 4 h vacuum degradation numbers, the condenser energy time constant at rated CW flow, and the statement that no MSIV isolation on low vacuum is modelled because no BWRX source for such a signal was found (spec: do not invent).

```bash
git add crates/bwr-core docs/model docs/validation
git commit -m "feat(bop): first-law condenser with derived vacuum, explicit CST boundary, hotwell bound, CW/vacuum faults

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---
### Task 8: Feedwater: pump speed states, parallel hydraulics, bumpless transfer, faults

**Files:**
- Modify: `crates/bwr-core/src/params.rs` (`FW_STATIC_DP`, `FW_PUMP_COASTDOWN_S`) and `docs/model/parameters.csv` (rows in this task)
- Modify: `crates/bwr-core/src/bop.rs` (`FeedFaults`, `Feedwater.pump_speed`, `pump_flow`, `hydraulic_solution` replacing `hydraulics`, `set_mode`, `step`, the `step_heaters` pump condition)
- Modify: `crates/bwr-core/src/sim.rs` (`bop_faults` complete, `SetLevelControl`, `SetFeedPump`/`SetCondensatePump` refusals, feedwater call)
- Test: `crates/bwr-core/tests/power_conversion.rs`

**Interfaces:**
- Produces: `#[derive(Clone, Copy, Debug, Default)] pub struct FeedFaults { pub pump_trip: [bool; N_FEED_PUMPS], pub cond_pump_trip: [bool; N_COND_PUMPS], pub fcv_stuck: bool, pub heaters_lost: bool }`; `Feedwater.pump_speed: [f64; N_FEED_PUMPS]`; `Feedwater::set_mode(&mut self, mode: LevelControlMode)` (master signature; bumpless via a private re-track flag consumed by the next `step`); `Feedwater::step(&mut self, dt, p_reactor, level_cm, steam_flow, hotwell_ok, faults: &FeedFaults) -> Option<String>` (the Task 6 list plus `faults`; the returned detail is logged by `Sim` as `feed pump trip: <detail>` for a low-suction trip AND for a fault-forced stop); `pub fn pump_flow(speed: f64, head: f64) -> f64` and `Feedwater::hydraulic_solution(&self, p_reactor, p_suction) -> (f64, f64)` (public, so the component tests can check them against independent calculations); `Sim::bop_faults(&self) -> (SteamFaults, CondenserFaults, FeedFaults)`. The Task 6 TEMPORARY pump condition in `step_heaters` is replaced here (mandatory).

- [ ] **Step 1: Write the failing tests**

Append to `tests/power_conversion.rs`:

```rust
use bwr_core::bop::{pump_flow, FeedFaults};

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
    assert!(sim.log.iter().any(|(_, t)| t.starts_with("feed pump trip:")), "fault-forced stop must be logged with the locked key: {:?}", sim.log);
    sim.apply(Command::SetFeedPump { index: 1, on: true });
    run(&mut sim, 1.0);
    assert!(!sim.fw.pumps[1], "start must be refused under the fault");
    assert!(sim.log.iter().any(|(_, t)| t.starts_with("command ignored:") && t.contains("Feed pump 2")), "refusal logged");
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
    assert!(sim.has_fault(&Fault::FcvStuck) && sim.has_fault(&Fault::HeatersLost), "faults retained");
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
    assert!(sim.has_fault(&Fault::HeatersLost));
}

// ---- component tests: the parallel hydraulics checked against independent calculations

#[test]
fn equal_speed_pumps_reproduce_the_single_curve_solution() {
    // n identical pumps at speed 1, closed form:
    // SHUTOFF*(1-(q/(n*RUNOUT))^2) = h0 + (q/cv)^2  =>  q = sqrt((SHUTOFF-h0)/(SHUTOFF/(n*RUNOUT)^2 + 1/cv^2)).
    let h0 = P_RATED + FW_STATIC_DP - CONDENSATE_PRESSURE;
    let cv = C_FCV * bwr_core::bop::fcv_characteristic(0.7);
    for n in 1..=N_FEED_PUMPS {
        let fw = Feedwater::new(n, T_FW_RATED, LevelControlMode::Manual, 0.7);
        let expect = ((FW_PUMP_SHUTOFF_DP - h0) / (FW_PUMP_SHUTOFF_DP / (n as f64 * FW_PUMP_RUNOUT).powi(2) + 1.0 / (cv * cv))).sqrt();
        let (q, h) = fw.hydraulic_solution(P_RATED, CONDENSATE_PRESSURE);
        assert!((q - expect).abs() < 1e-6 * expect, "{n} pumps: {q} vs {expect}");
        assert!((h - (h0 + (q / cv).powi(2))).abs() < 1.0, "head consistent with the valve");
    }
}

#[test]
fn unequal_speed_pumps_share_head_and_sum_their_curve_flows() {
    let mut fw = Feedwater::new(3, T_FW_RATED, LevelControlMode::Manual, 0.7);
    fw.pump_speed = [1.0, 0.8, 0.0];
    let (q, h) = fw.hydraulic_solution(P_RATED, CONDENSATE_PRESSURE);
    let contributions: Vec<f64> = fw.pump_speed.iter().map(|&s| pump_flow(s, h)).collect();
    assert!((contributions.iter().sum::<f64>() - q).abs() < 1e-6 * q, "per-pump flows at the common head sum to the valve flow");
    assert!(contributions[0] > contributions[1] && contributions[1] > 0.0, "{contributions:?}");
    assert_eq!(contributions[2], 0.0, "stopped pump");
    let cv = C_FCV * bwr_core::bop::fcv_characteristic(0.7);
    assert!((q - cv * (h - (P_RATED + FW_STATIC_DP - CONDENSATE_PRESSURE)).sqrt()).abs() < 1e-6 * q);
    // Not the equal-speed answer at the mean speed: the parallel solution is what the model uses.
    let mut eq = Feedwater::new(3, T_FW_RATED, LevelControlMode::Manual, 0.7);
    eq.pump_speed = [0.9, 0.9, 0.0];
    assert!((eq.hydraulic_solution(P_RATED, CONDENSATE_PRESSURE).0 - q).abs() > 1.0);
}

#[test]
fn slow_pump_check_valve_closes_at_shutoff_crossover() {
    let h0 = P_RATED + FW_STATIC_DP - CONDENSATE_PRESSURE;
    let s_cross = (h0 / FW_PUMP_SHUTOFF_DP).sqrt(); // speed whose shutoff head equals the static head
    let mut fw = Feedwater::new(2, T_FW_RATED, LevelControlMode::Manual, 0.7);
    fw.pump_speed = [1.0, 0.9 * s_cross, 0.0];
    let (q_pair, h) = fw.hydraulic_solution(P_RATED, CONDENSATE_PRESSURE);
    assert_eq!(pump_flow(0.9 * s_cross, h), 0.0, "below crossover the slow pump's check valve is shut");
    fw.pump_speed = [1.0, 0.0, 0.0];
    let (q_single, _) = fw.hydraulic_solution(P_RATED, CONDENSATE_PRESSURE);
    assert!((q_pair - q_single).abs() < 1e-9, "a closed check valve contributes nothing");
    fw.pump_speed = [1.0, 1.1 * s_cross, 0.0];
    let (q_above, h_above) = fw.hydraulic_solution(P_RATED, CONDENSATE_PRESSURE);
    assert!(pump_flow(1.1 * s_cross, h_above) > 0.0 && q_above > q_single, "above crossover the slow pump opens and adds flow");
}

#[test]
fn all_pump_coastdown_reaches_zero_flow_when_shutoff_head_crosses_the_static_head() {
    let h0 = P_RATED + FW_STATIC_DP - CONDENSATE_PRESSURE;
    let s_cross = (h0 / FW_PUMP_SHUTOFF_DP).sqrt();
    let mut fw = Feedwater::new(3, T_FW_RATED, LevelControlMode::Manual, 0.7);
    let mut last_q = f64::INFINITY;
    let mut s = 1.0;
    while s > 0.0 {
        fw.pump_speed = [s; N_FEED_PUMPS];
        let (q, _) = fw.hydraulic_solution(P_RATED, CONDENSATE_PRESSURE);
        assert!(q <= last_q + 1e-9, "flow must fall monotonically with speed");
        if s > 1.02 * s_cross {
            assert!(q > 0.0, "speed {s} above crossover must still feed");
        }
        if s < 0.98 * s_cross {
            assert_eq!(q, 0.0, "speed {s} below crossover: check valves hold");
        }
        last_q = q;
        s -= 0.01;
    }
    // Through Sim: stopping every pump coasts the flow to zero when the first-order speed
    // decay reaches the crossover speed, not instantly.
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    for i in 0..N_FEED_PUMPS {
        sim.apply(Command::SetFeedPump { index: i, on: false });
    }
    let mut t_zero = None;
    for _ in 0..(30.0 / DT) as usize {
        sim.step(DT);
        if t_zero.is_none() && sim.fw.flow == 0.0 {
            t_zero = Some(sim.time);
        }
    }
    let t_zero = t_zero.expect("flow must reach zero");
    let expect = 5.0 + FW_PUMP_COASTDOWN_S * (1.0 / s_cross).ln();
    assert!((t_zero - expect).abs() < 1.0, "flow stopped at {t_zero}, expected ~{expect} from the speed time constant");
}

#[test]
fn condensate_only_operation_feeds_a_depressurised_vessel() {
    let mut fw = Feedwater::new(0, 40.0, LevelControlMode::Manual, 0.5);
    fw.cond_pumps = [true, false];
    let (q_low, _) = fw.hydraulic_solution(0.5e6, CONDENSATE_PRESSURE);
    assert!(q_low > 0.0, "condensate pressure alone feeds a 0.5 MPa vessel");
    let (q_high, _) = fw.hydraulic_solution(P_RATED, CONDENSATE_PRESSURE);
    assert_eq!(q_high, 0.0, "not a 7.2 MPa vessel");
    // Independent check of the baseline condensate-only term at the solution.
    let cv = C_FCV * bwr_core::bop::fcv_characteristic(0.5);
    let h0 = 0.5e6 + FW_STATIC_DP - CONDENSATE_PRESSURE;
    assert!((-(q_low / 150.0).powi(2) * 1.0e5 - h0 - (q_low / cv).powi(2)).abs() < 1.0);
}

#[test]
fn suction_loss_stops_flow_and_trips_the_feed_pumps() {
    let mut fw = Feedwater::new(2, T_FW_RATED, LevelControlMode::Manual, 0.7);
    let ff = FeedFaults::default();
    assert!(fw.step(DT, P_RATED, 0.0, STEAM_FLOW_RATED, true, &ff).is_none());
    assert!(fw.flow > 0.0);
    let mut msg = None;
    for _ in 0..(4.0 / DT) as usize {
        if let Some(m) = fw.step(DT, P_RATED, 0.0, STEAM_FLOW_RATED, false, &ff) {
            msg = Some(m);
        }
        assert_eq!(fw.flow, 0.0, "no suction, no flow");
    }
    assert!(msg.as_deref().map_or(false, |m| m.contains("low suction")), "{msg:?}");
    assert!(fw.pumps.iter().all(|&p| !p), "pumps tripped after 3 s");
}

#[test]
fn condensate_pump_trip_starves_suction_and_feed_flow() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    let f0 = sim.fw.flow;
    for index in 0..N_COND_PUMPS {
        sim.set_fault(Fault::CondensatePumpTrip { index }, true);
    }
    sim.step(DT);
    assert!(sim.fw.cond_pumps.iter().all(|&b| !b), "faulted condensate pumps stop");
    assert_eq!(sim.fw.flow, 0.0, "no suction pressure: feed flow collapses from {f0}");
    run(&mut sim, 4.0);
    assert!(sim.fw.pumps.iter().all(|&p| !p), "feed pumps trip on low suction");
    assert!(sim.log.iter().any(|(_, t)| t.starts_with("feed pump trip:")), "locked log key: {:?}", sim.log);
    sim.apply(Command::SetCondensatePump { index: 0, on: true });
    assert!(!sim.fw.cond_pumps[0], "restart refused under the fault");
    assert!(sim.log.iter().any(|(_, t)| t.starts_with("command ignored:") && t.contains("Condensate pump")));
    assert!((0..N_COND_PUMPS).all(|index| sim.has_fault(&Fault::CondensatePumpTrip { index })), "faults retained");
}
```

Fault coverage matrix (one row per fault in Task 1's "Faults honoured"; every row names the test that asserts the behaviour, the retention assertion (`has_fault` after the run), the blocked restart where one exists, and the logged event):

| Fault | Behaviour test | Retention | Blocked restart | Logged event |
|---|---|---|---|---|
| `MsivStuck { line }` | `stuck_msiv_stays_open_during_isolation` (Task 4) | yes | n/a (valve) | `msiv isolation:` from the unfaulted line (P2) |
| `TsvStuck` | `tsv_closes_fast_on_trip_and_stuck_tsv_holds`, `stuck_tsv_and_tcv_keep_feeding_the_turbine_after_trip` (Task 4) | yes | n/a | `turbine trip:` (P2) |
| `TcvStuck` | `stuck_tsv_and_tcv_keep_feeding_the_turbine_after_trip` (Task 4) | yes | n/a | `turbine trip:` (P2) |
| `BypassUnavailable` | `turbine_trip_without_bypass_scrams_on_pressure_and_starts_ics` (Task 5) | yes | n/a | `scram:`, `ic auto-initiation:` (P2) |
| `PressureRegulatorFailed { demand_pct }` | `regulator_frozen_output_overrides_pressure_demand` (Task 4) | yes | n/a | `scram:` (P2) |
| `FeedPumpTrip { index }` | `tripped_feed_pump_cannot_be_restarted_and_fault_persists` | yes | `SetFeedPump` refused, `command ignored:` | `feed pump trip:` on the fault-forced stop |
| `CondensatePumpTrip { index }` | `condensate_pump_trip_starves_suction_and_feed_flow` | yes | `SetCondensatePump` refused, `command ignored:` | `feed pump trip:` (low suction) |
| `FcvStuck` | `stuck_fcv_holds_position_and_lost_heaters_cannot_be_restored` | yes | n/a | none (position hold) |
| `HeatersLost` | `stuck_fcv_holds_position_and_lost_heaters_cannot_be_restored`, `loss_of_feedwater_heating_raises_power` | yes | `SetHeaters` refused, `command ignored:` | none |
| `CwPumpTrip { index }` | `loss_of_circulating_water_trips_turbine_then_scrams_on_pressure` (Task 7) | yes | `SetCwPump` refused, `command ignored:` | `turbine trip:`, `scram:` (P2) |
| `VacuumPumpFailed` | `vacuum_pump_failure_degrades_vacuum_monotonically` (Task 7) | yes | `SetVacuumPump` refused, `command ignored:` | none |

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --release -p bwr-core --test power_conversion`
Expected: compile error `no field pump_speed` / `no function pump_flow`.

- [ ] **Step 3: Parameters**

In `params.rs` feedwater section:

```rust
/// Static head plus piping loss from feed pumps to the vessel nozzle at zero flow, Pa. ASSUMED (was a literal).
pub const FW_STATIC_DP: f64 = 0.15e6;
/// Feed pump speed time constant for start and coastdown, s. ASSUMED (A-6xx).
pub const FW_PUMP_COASTDOWN_S: f64 = 5.0;
```

Append to `docs/model/parameters.csv` (same task):

```
FW_STATIC_DP,0.15e6,Pa,static head plus piping loss from feed pumps to the vessel nozzle at zero flow,n/a,generic,assumed,,0.05e6,0.4e6,A-6xx,crates/bwr-core/src/params.rs,was a literal in hydraulics
FW_PUMP_COASTDOWN_S,5.0,s,feed pump speed time constant for start and coastdown,n/a,generic,assumed,,2,15,A-6xx,crates/bwr-core/src/params.rs,
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

Replace the private `hydraulics` with a free pump curve plus a public parallel solution:

```rust
/// One feed pump's flow (kg/s) at speed `s` (0..1) against head `h` (Pa), affinity laws:
/// head = s^2 * FW_PUMP_SHUTOFF_DP * (1 - (q / (s * FW_PUMP_RUNOUT))^2). Zero for a stopped
/// pump and at or above its shutoff head (its check valve closes).
pub fn pump_flow(s: f64, h: f64) -> f64 {
    if s <= 0.01 {
        return 0.0;
    }
    let h_max = s * s * FW_PUMP_SHUTOFF_DP;
    if h >= h_max {
        0.0
    } else {
        s * FW_PUMP_RUNOUT * (1.0 - h / h_max).sqrt()
    }
}
```

```rust
    /// Parallel-pump solution for a vessel at `p_reactor` with suction `p_suction`: returns
    /// (flow into the vessel kg/s, common pump head Pa). The common head H solves
    /// sum_i pump_flow(s_i, H) = q_valve(H), where q_valve is the FCV flow for the head left
    /// after the static loss; pumps whose shutoff head is below H contribute nothing (check
    /// valve). With no pump running, condensate pressure alone pushes through the idle pumps'
    /// resistance (the baseline term). Public so the component tests can check it against
    /// independent calculations.
    pub fn hydraulic_solution(&self, p_reactor: f64, p_suction: f64) -> (f64, f64) {
        let pos = self.fcv_pos;
        if pos < 1e-4 || p_suction <= 0.0 {
            return (0.0, 0.0);
        }
        let cv = C_FCV * fcv_characteristic(pos);
        let h0 = p_reactor + FW_STATIC_DP - p_suction; // head the pumps must supply at zero flow
        if !self.pump_speed.iter().any(|&s| s > 0.01) {
            // Condensate-only: flow only into a depressurised vessel, through the idle pumps.
            if h0 >= 0.0 {
                return (0.0, 0.0);
            }
            let f = |q: f64| -(q / 150.0).powi(2) * 1.0e5 - h0 - (q / cv).powi(2);
            let (mut lo, mut hi) = (0.0, 150.0 * (-h0 / 1.0e5).sqrt());
            for _ in 0..60 {
                let mid = 0.5 * (lo + hi);
                if f(mid) > 0.0 {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            let q = 0.5 * (lo + hi);
            return (q, h0 + (q / cv).powi(2));
        }
        let h_max = self.pump_speed.iter().map(|&s| s * s * FW_PUMP_SHUTOFF_DP).fold(0.0, f64::max);
        if h0 >= h_max {
            return (0.0, h0); // every check valve holds: no pump can overcome vessel pressure
        }
        let q_pumps = |h: f64| -> f64 { self.pump_speed.iter().map(|&s| pump_flow(s, h)).sum() };
        let (mut lo, mut hi) = (h0, h_max);
        for _ in 0..60 {
            let mid = 0.5 * (lo + hi);
            if q_pumps(mid) > cv * (mid - h0).sqrt() {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let h = 0.5 * (lo + hi);
        (cv * (h - h0).max(0.0).sqrt(), h)
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

In `step` (now `step(&mut self, dt, p_reactor, level_cm, steam_flow, hotwell_ok, faults: &FeedFaults)`), keep the Task 2a guard as the first statement, move `let mut msg = None;` directly after it, and then add (a fault-forced stop of a running pump returns a message so `Sim` logs it with the locked key):

```rust
        for i in 0..N_FEED_PUMPS {
            if faults.pump_trip[i] && self.pumps[i] {
                self.pumps[i] = false;
                msg = Some(format!("Feed pump {} tripped (fault)", i + 1));
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

Change the low-suction check from `self.running_pumps() > 0` to `self.pump_speed.iter().any(|&s| s > 0.01)` and its message to `"Feed pumps tripped on low suction pressure"` (unchanged text; `Sim` prefixes the key). Wrap the valve move: `if !faults.fcv_stuck { let d = target - self.fcv_pos; self.fcv_pos += d.clamp(-0.1 * dt, 0.1 * dt); }`. `self.flow = self.hydraulics(p_reactor, p_s);` becomes `self.flow = self.hydraulic_solution(p_reactor, p_s).0;`. In `step_heaters` replace the Task 6 TEMPORARY line `let pumps_running = self.running_pumps() > 0;` with `let pumps_running = self.pump_speed.iter().any(|&s| s > 0.01);` (mandatory: a coasting pump still does fluid work).

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

Call `let (sf, cf, ff) = self.bop_faults();` once at the top of `Sim::step` and pass `&ff` to the feedwater call, which becomes `self.fw.step(dt, p, self.level_cm(), self.steam.vessel_steam_flow(), hotwell_ok, &ff)`, `&cf` to `self.cond.step(...)`, `&sf` to the steam calls. Every `Some(m)` the feedwater step returns is logged with the locked key in addition to the operator event: `self.log.push((self.time, format!("feed pump trip: {m}"))); self.event("fw_trip", m);` (grep `feed pump trip:` first: P2 logs the L8 trip in `protection()` with this key, and the low-suction and fault-forced trips must use the same one). In `apply`:

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
Expected: all pass. `full_power_is_steady` and `startup_from_hot_standby_to_30_percent` exercise the new hydraulics: if feed flow at full power differs from the baseline by more than 2 %, stop and report the printed hydraulics values (the parallel solution must reproduce the baseline's equal-speed case exactly: `n` pumps at speed 1 → identical `q`, which `equal_speed_pumps_reproduce_the_single_curve_solution` checks in closed form). Then compare mode: `cargo run --release -p bwr-core --example validation`, append the classified output under `## Task 8: feed pump speed states` in `docs/validation/reports/2026-10-21-pkg06.md` (physical: coastdown replaces the instantaneous stop in any case that trips a feed pump; `fw_pump_0..2` stay unpopulated until Task 10 Step 5b), then `--write` once.

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

`params.rs` feedwater section: `/// Maximum RWCU reject flow to the hotwell, kg/s. ASSUMED (A-6xx: RWCU pumps always available). pub const RWCU_REJECT_MAX_KGS: f64 = 15.0;` and its register row in the same task: `RWCU_REJECT_MAX_KGS,15.0,kg/s,maximum RWCU reject flow to the hotwell,n/a,generic BWR,assumed,,5,30,A-6xx,crates/bwr-core/src/params.rs,pumped and feasible at any vessel pressure (assumption); only the cap applies`. In `sim.rs` `apply`: `Command::SetRwcuReject { kgs } => self.fw.rwcu_reject = kgs.clamp(0.0, RWCU_REJECT_MAX_KGS),`. Confirm the Task 6 condenser block already uses `flows.w_liquid_out` and `flows.h_liquid_out` (the APPLIED reject and its enthalpy, so a subcooled vessel rejects subcooled liquid enthalpy, not `sat().h_f`, and an inventory-bounded step moves only what the vessel released); if the P3/P4 code left `self.vessel.sat().h_f` or `self.fw.rwcu_reject` anywhere in the condenser block, replace it. Tests read the applied value through `sim.last_flows().w_liquid_out`; add to `rwcu_reject_moves_mass_from_vessel_to_hotwell` after the reject run: `assert!((sim.last_flows().w_liquid_out - RWCU_REJECT_MAX_KGS).abs() < 1e-9, "applied reject must equal the request at adequate inventory");`.

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

`ui/src/sim/mock.ts` keeps its state in `this.s` and `snapshot()` returns `structuredClone(this.s)`, so the new fields are added to the initial-state literal and advanced in `step()` (identifiers as inspected on 2026-10-21):

1. In the initial-state literal, inside `steam: { ... pressureSetpointMpa: 7.2, }` add `tsvPos: 0, lineFlowsKgs: [0, 0], headerPressureMpa: 0,`; inside `condenser: { ... heatLoadMw: 0, }` add `cstLevelPct: 75, makeupKgs: 0, rejectKgs: 0,`; inside `turbine: { ... loadLimitPct: 100, }` add `extractionMw: 0,`.
2. In `private step(dt)` (locals `st = s.steam`, `tb = s.turbine`, `cd = s.condenser`, `v = s.vessel`), directly after `v.steamFlowKgs = st.steamToTurbineKgs + st.steamToBypassKgs + ventFlow;` add:

```ts
    st.tsvPos = tb.tripped ? 0 : 1;
    st.lineFlowsKgs = [v.steamFlowKgs / 2, v.steamFlowKgs / 2];
    st.headerPressureMpa = Math.max(v.pressureMpa - 0.2, 0);
```

   and directly after `tb.netMwe = tb.grossMwe - houseLoad;` add `tb.extractionMw = tb.grossMwe * 0.3;`. `cd.cstLevelPct`, `cd.makeupKgs` and `cd.rejectKgs` stay at their initial values (the mock has no CST).
3. In the full-power preset branch (the block that sets `s.steam.tcvPos = 0.94;` and `s.turbine.grossMwe = 312;`) add `s.steam.tsvPos = 1; s.steam.lineFlowsKgs = [237.5, 237.5]; s.steam.headerPressureMpa = 7.0; s.turbine.extractionMw = 94;`.

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

Verify only (register-row rule): `cargo test --release -p bwr-core --test register` passes with NO new rows from this task; every P6 constant was registered in the task that introduced or changed it (Tasks 2, 3, 4, 5, 6, 7, 8, 9; `TURBINE_DH_RATED` and `BYPASS_CAPACITY` were UPDATED in place, never duplicated). If the register test reports a missing row, the fix goes into that task's commit history, not here. Append to `docs/model/esbwr-applicability.csv` using its locked header `id,esbwr_source,esbwr_section,target_subsystem,bwrx_evidence,differences,adaptation,applicability_limits,test,classification`:

```
ESBWR-P6-01,ESBWR DCD Tier 2 Rev. 10,Ch. 10.2-10.4 main steam and power conversion,main steam/turbine/condenser,BWRX general description (steam cycle outline),ESBWR is 4500 MWt with 4 steam lines; BWRX has 2 lines and ~870 MWt,topology and valve roles only; no numeric scaling,steady-state energy balance structure,power_conversion::full_power_energy_balance_closes,shared concept
ESBWR-P6-02,ESBWR DCD Tier 2 Rev. 10,Ch. 7.7 pressure regulator and turbine bypass,EHC pressure regulation,none numeric,regulator gains and bypass capacity not published for BWRX,regulator span and bypass capacity remain assumed,regulator fault and load rejection cases,power_conversion::load_rejection_with_bypass_does_not_scram,shared concept
ESBWR-P6-03,ESBWR DCD Tier 2 Rev. 10,Ch. 10.4.7 condensate and feedwater,feedwater pumps and heaters,none numeric,pump count/capacity differ,pump curves and heater duty remain assumed,feed pump trip and heater loss cases,power_conversion::loss_of_feedwater_heating_raises_power,shared concept
```

- [ ] **Step 7: Baselines and results**

Master baseline regeneration protocol, final pass: (1) `cargo run --release -p bwr-core --example validation` in compare mode; append the output under `## Task 10: trace columns populated` in `docs/validation/reports/2026-10-21-pkg06.md`; (2) classify every change: schema-population for `tsv_pos`, `header_pressure_pa`, `cst_kg`, `fw_pump_0..2` (newly populated columns; byte-identical traces are NOT expected), physical or numerical for anything else (expect none from this task); (3) the package gates are `cargo test --release -p bwr-core` green including `p6_trace_columns_are_populated`; (4) run `--write` ONCE; (5) commit the report with the baselines. A second compare run after writing only shows the runner is deterministic; it is not validation evidence and is not claimed as such. Append to `docs/model/power-conversion.md` "Results" the final before/after table for every P6 case (peak pressure, time, scram reason, net MWe, condenser duty, worst 60 s residual) and a "Limitations" list: saturated line node (no superheat; the wall term `q_line_wall_w` is the explicit closure of that constraint), heater train as a single lumped node, bypass capacity and SRI group unconfirmed (Task 5a), extraction fractions assumed, CST and CW as declared boundaries, condenser perfectly mixed, no containment isolation.

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
| Verify bypass capacity, turbine trip and load-rejection response, valve timing, governor, pressure regulation; reassess SRI group and ride-through test | 4, 5, 5a (evidence search recorded; sensitivity run; criteria labelled plausibility, not sourced requirements) |
| Reconcile shaft power, generator output, pump work, extraction heating, condenser duty, house loads in one energy balance | 6 |
| Refine condenser heat transfer, noncondensable gas, vacuum loss, hotwell inventory; declare cooling-water boundary | 1, 7 |
| Replace implicit unlimited makeup and reject with explicit boundary accounting and declared inventory | 7 |
| Verify pump count/capacity, check/isolation behaviour, head, suction limits, pump stopping response, heater losses, controller mode transfer | 8 |
| RWCU reject destination, feasibility, mass/energy effects | 9 |
| Acceptance gate: balances close; condenser loss, feedwater disturbances, turbine trips follow the design basis within documented uncertainty | 5, 6, 7, 8, 10 |

Master r2 obligations carried by this package:

| Master requirement | Task |
|---|---|
| Transfer allocation: every steam/RWCU/feed consumer uses applied flows (step order 7–12, defect 13); P3 identities migrated to the storage-aware form | 3 (wiring and migration), 6, 7, 9 |
| `EnergyBalance` control volume with storage terms; steady and transient closure | 6 |
| `SteamLine.energy`/`q_wall_w`, `Turbine.p_losses`, `Condenser.energy`, `Feedwater.heater_energy` storage states | 3, 6, 7 |
| dt rule / non-finite inputs rule with `last_error` channels on every BOP step | 2a |
| `HotwellExhausted` and the step-7 hotwell bound; applied `w_feed` everywhere | 7 |
| Register-row rule (row in the task that adds or changes the constant) | 2, 3, 4, 5, 6, 7, 8, 9 |
| Baseline regeneration protocol (compare, save report, classify, write once) | 3, 5, 6, 7, 8, 10 |
| Derives rule (`SteamLine` Copy; fault views Copy+Default; `EnergyBalance` Copy+Default) | 3, 4, 6, 7, 8 |
| `faults: Vec<Fault>` read via `has_fault`/`find_fault`; faults never auto-cleared (tested) | 4, 5, 7, 8 |
| Trace schema v2 columns `tsv_pos`, `header_pressure_pa`, `cst_kg`, `fw_pump_0..2`, `msiv_pos_0/1` | 10 (Step 5b) |
| Locked CSV headers for `parameters.csv`, `cases.csv`, `esbwr-applicability.csv` | 5, 10 |
| Log-line convention (`msiv isolation:`, `command ignored:`) | 5, 7, 8 |
| `Sim::last_flows()` for applied-flow reads in tests | 3, 9 |

## Self-review against the master checklist (r2)

- Spec coverage: every Package 6 bullet maps to a task (table above).
- Executability: every code step shows code against inspected identifiers (P4's `Vessel::steam_available()` and `eos.p_steam` for the dome steam pressure; the mock's `st`/`tb`/`cd`/`v` locals and `structuredClone` snapshot; `BoundaryFlows.h_steam`/`h_liquid_out`). Cross-package names used are those the master defines (`run_case`, `col_index`, `standard_cases`, `last_flows`, `find_fault`, `CP_WATER`, `T_TABLE_MIN/MAX`, `steam_available`). Where a package-local temporary is unavoidable (Task 6's condenser lines, the `step_heaters` pump condition) the replacing task and step are named.
- Names and signatures match the master: `SteamSystem::new(msivs_open, tsv_open)` and `compute_flows(dt, p_vessel)` from Task 2, `init_line_steady(p_vessel)` and `step_line(dt, w_in_applied, h_in) -> Result` from Task 3, `Turbine::step(.., extraction_heat_w)`, `Condenser::step(.., &CondenserFaults)`, `Feedwater::step(.., &FeedFaults)`, `Feedwater::set_mode(mode)`, `SteamLine { mass, energy, p, q_wall_w }`, `Turbine.p_extraction`/`p_losses` with signed `p_elec`, `Condenser { energy, q_in_w, q_cw_w, cst_mass, makeup_flow, reject_flow, makeup_h, reject_h, overflow_kg, deficit_kg }`, `Feedwater { pump_speed, heater_energy, q_extraction_applied_w, pump_work_w }`, `DomainError::HotwellExhausted`, the `EnergyBalance` field list and its doc comment quoted from the master. Every `cargo test` compiles at every task boundary (no deferred signature migrations).
- Derives: stated on every new struct; `last_error` on the four BOP components.
- Constants: every new or changed constant has its `parameters.csv` row in the task that adds or changes it; Task 10 only verifies.
- Snapshot changes: `types.ts` and `mock.ts` steps present (Task 10 Steps 4–5) with inspected identifiers; typecheck/build commands given.
- Defect fixes: defect 6 (hotwell) has a compile-failing test first (Task 7); the averaging defect has a compile-failing test first (Task 2); the fault coverage matrix (Task 8) names, per fault, the behaviour test, the retention assertion, the blocked restart and the logged event.
- Applied transfers: Task 3 scales only `w_line` and lets the node bound its own outflow; `flows.w_feed` is withdrawn from the condenser and charged to the heaters; the condenser and balance read `flows.w_liquid_out`/`h_liquid_out`; `steam_consumers_see_applied_flows_with_line_storage` and `hotwell_inventory_tracks_applied_transfers_every_step` guard it.
- Energy: one control volume (master definition), one applied extraction value, pump work once, signed electric output, explicit line-wall and overflow boundary terms, no clamp on the condenser duty, no post-failure tolerance adjustment anywhere.
