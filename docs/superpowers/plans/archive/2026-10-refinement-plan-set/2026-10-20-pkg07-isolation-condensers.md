# Package 7: Isolation Condensers and Shutdown Heat Removal — Implementation Plan (r2)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Names and signatures come from `docs/superpowers/plans/2026-10-20-refinement-master.md` r2 ("Shared interface contract", section P7, and the "Shared `Sim::step` order"); if this file and the master disagree, the master wins.

**Goal:** Replace the single-valve, energy-only isolation condenser (IC) model with a per-train model that moves steam and condensate through explicit valves, tube inventory and a pool whose energy is conserved, so that heat removal is physically supplied and bounded by what the vessel can give, endurance claims are tied to stated conditions, and the lower temperature limit of IC cooling is a declared boundary rather than an accident of vessel heat loss.

**Architecture:** `ic.rs` becomes a mass-and-energy component: steam in through a normally-open steam valve, condensate held in tubes (with its own enthalpy) until one of two parallel normally-closed return valves opens, condensate back to the vessel at the tube water's enthalpy. Each train's step is split in two: `request` (pure) states the steam it would take and the condensate it would return; the vessel is the transfer-allocation point (master step 8); `commit` (step 9) advances valves, tubes and pool with the flows the vessel actually applied, so the ledger, the energy balance, the trace and the snapshot all see one applied number and no state ever moves on a requested one. `Sim` maps `Fault`s onto per-train availability, valve freezes and UA degradation through a `Copy` fault view, raises `DomainError::IcModelEnvelope` through `Sim::domain_error` when a pool is exhausted while still loaded, and logs initiation for ordering tests. Long-duration validation cases use `Case.dt_after_s` with a convergence check.

**Tech Stack:** Rust 2021, serde, existing `bwr-core` modules from Packages 2–6 (`ledger`, `faults`, `validation`, `domain`, `vessel::Regime`, `bop::EnergyBalance`), TypeScript mirror in `ui/src/sim/types.ts`, `mock.ts`, `PlantMimic.tsx`.

**Spec:** Package 7 of `docs/superpowers/plans/2026-10-20-existing-systems-refinement.md`.

## Global Constraints

Everything in the master plan's "Global Constraints" section applies, including the Derives rule. Package-specific restatements from the spec:

- "Do not scale dimensions, inventory, flow, decay-heat removal capacity, or timing by electrical output alone."
- "Include a justified sensitivity treatment for noncondensable gas degradation. Do not invent exact design performance or purge behavior."
- "Run unattended cooling tests to 72 hours and seven days where these durations match the selected source claim. Specify initial operating history, credited trains, makeup, and external services."
- "If complete cold-shutdown cooling requires an unimplemented system, document that boundary instead of attributing its function to ICS or arbitrary vessel heat loss."
- "GE's public BWRX general description identifies integral RPV isolation valves, elimination of conventional safety relief valves, and dry containment as distinguishing features." and "The NRC/CNSC containment evaluation report also notes that IC function and reactor connections differ despite design heritage." ESBWR IC material is a shared-concept source only.
- No new crate or npm dependencies.
- Every new constant gets a `docs/model/parameters.csv` row (13 locked columns: `id,value,unit,definition,reference_state,source_design,status,source_ids,interval_lo,interval_hi,assumption_id,location,notes`); P1's `tests/register.rs` fails the build until the row exists.
- Every consumer of a transfer uses the applied (post-allocation) value.

**Acceptance gate (verbatim from the spec):** "Heat removal is physically supplied and conserved; endurance claims identify conditions and evidence. Shutdown scenarios remain valid through their declared endpoint."

## Assumed state at start of this package (check before Task 1)

```bash
cd /Users/jim/ai/bwrsim
grep -n "IcModelEnvelope" crates/bwr-core/src/domain.rs
grep -n "pub fn domain_error" crates/bwr-core/src/sim.rs
grep -n "pub fn find_fault\|pub faults: Vec<Fault>" crates/bwr-core/src/sim.rs
grep -n "pub fn request(&self, dt: f64, p_vessel: f64, w_steam_available: f64) -> IcRequest\|pub fn commit(&mut self, dt: f64, p_vessel: f64, applied: IcRequest) -> IcStep\|pub last_error: Option<DomainError>" crates/bwr-core/src/ic.rs
grep -n "pub ic_boundary: IcBoundary\|\.scaled(ic_ratio)\|t.last_error.take()" crates/bwr-core/src/sim.rs
grep -n "dt_after_s\|operator: Operator" crates/bwr-core/src/validation.rs | head -3
grep -n "ic_heat_w_0\|ic_pool_kg_0\|ic_return_pos_0\|t_coolant_c" crates/bwr-core/src/validation.rs | head -4
grep -n "w_ic_steam\|h_ic_return" crates/bwr-core/src/ledger.rs
grep -n "pub enum Regime" crates/bwr-core/src/vessel.rs
grep -n "storage_w\|q_ic_w\|q_line_wall_w" crates/bwr-core/src/bop.rs
grep -n "IC tube energy from Package 7\|q_ic_w: flows.q_ic\|self.turbine.kinetic_energy()\|self.fw.heater_energy\|self.cond.energy" crates/bwr-core/src/sim.rs
```

Every grep must hit. If one does not, the owning package (P2 ledger/validation, P3 domain/ic interim, P4 regime, P6 balance) is incomplete; fix it there in a one-line commit, not here. Known state from those plans: `BoundaryFlows` already carries `w_ic_steam`, `w_ic_return`, `h_ic_return` (zero before this package) and `q_ic`; `Ledger::record` already counts `w_ic_return` in `mass_in_kg`, `w_ic_steam` in `mass_out_kg`, `q_ic` in `energy_out_j`, and fills `integrated["ic_steam_kg"|"ic_return_kg"|"ic_j"]`; the interim `IcStep` has the master's seven fields incl. `pool_cooling_w`; P3's `Sim::step` already calls `IcTrain::request` at step 6 and `commit` at step 9 with `IcRequest::scaled(ic_ratio)`, accumulates every `IcStep` into `Sim.ic_boundary` (`IcBoundary::add`) and drains `IcTrain.last_error` at step 17; P6's balance block captures `stored_before`/`stored_after` sums (vessel, `steam.line.energy`, `cond.energy`, `fw.heater_energy`, `turbine.kinetic_energy()`) with a `// + IC tube energy from Package 7 (0 until then)` placeholder and sets `q_ic_w: flows.q_ic`; `Sim::domain_error` dedups by variant and logs `DOMAIN: <reason>`; the trace schema is v2 with 59 columns and the three IC per-train column triplets emitted as `0.0`/`valve_pos` until this package; the runner regenerates baselines with `--write`.

## Names introduced by this package (beyond the master contract)

| Name | Where | Meaning |
|---|---|---|
| `IC_TUBE_CAPACITY_KG` | `params.rs` | Condensate that fills one train's tubes and drain line, kg. ASSUMED 5 000. |
| `IC_DRAIN_TAU_S` | `params.rs` | Drain time constant through an open return valve, s. ASSUMED 20. |
| `IC_POOL_MASS_TOTAL` | `params.rs` | Total pool water for all trains, kg. DERIVED from the 7-day claim (Task 1). |
| `IC_POOL_MASS` | `params.rs` | Per-train bay, `IC_POOL_MASS_TOTAL / N_IC_TRAINS as f64` (replaces the 1.5e6 ASSUMED value). |
| `IC_POOL_COOLING_W` | `params.rs` | Credited pool cooling system duty, W. ASSUMED 0. Replaces P3's `POOL_IDLE_COOLING_K_PER_S`, which is deleted. |
| `IC_POOL_T_AMBIENT` | `params.rs` | Floor for an idle pool's temperature, degC. ASSUMED 40. |
| `IcTrain.steam_valve_cmd_open: bool` | `ic.rs` | Set by `Sim` from `Fault::IcSteamValveClosed`. |
| `IcTrain.return_valve_stuck: [bool; N_IC_RETURN_VALVES]` | `ic.rs` | Set by `Sim` from `Fault::IcReturnValveStuck`. |
| `IcRequest { w_steam_in, w_condensate_return, h_condensate_return }` | `ic.rs` | `#[derive(Clone, Copy, Debug, Default)]`; what a train asks for this step. |
| `IcTrain::request(&self, dt, p_vessel, w_steam_available) -> IcRequest` | `ic.rs` | Pure; the master's request half (P3 interim form, signature kept). |
| `IcTrain::commit(&mut self, dt, p_vessel, applied: IcRequest) -> IcStep` | `ic.rs` | Advances valves, tubes and pool with the APPLIED flows (the request scaled by the vessel's allocation ratios). |
| `IcTrain::envelope_exceeded(&self, t_sat: f64) -> bool` | `ic.rs` | Pool empty while in service and steam hotter than the boiling pool. |
| `IcTrain::tube_h(&self) -> f64` | `ic.rs` | Specific enthalpy of the tube condensate, J/kg (`tube_energy / tube_mass`). |
| `IcFaults { unavailable, steam_closed, return_stuck, degradation }` | `sim.rs` | `#[derive(Clone, Copy, Debug, Default)]` fault view built per train by `Sim::ic_faults(&self, train) -> IcFaults` (mirrors P6's `bop_faults`). |
| `Sim.ic_envelope_raised: Vec<bool>` | `sim.rs` | Private, one per train; prevents re-raising after the first depletion. Reset in `Sim::base`. |
| `Sim.ic_tube_storage_w: f64` | `sim.rs` | Public; `Σ Δtube_energy / dt` of the last step, the IC storage term of the energy balance. |
| `Sim::cw_pumps_all_off(&self) -> bool` | `sim.rs` | Public test helper. |
| `DT_LONG` | `validation.rs` | 0.5 s, the coarse step used by `dt_after_s` for long cases. |
| `--long` | `examples/validation.rs` | Includes cases with `duration_s > 86400` in a runner invocation. |
| `docs/validation/results/{ic-availability,ic-degradation,ic-inadvertent,ic-endurance}.csv` | repo | Written by tests when `BWR_WRITE_RESULTS=1` (master file list). |

Master names used unchanged: `N_IC_RETURN_VALVES`, `IcTrain` fields, `IcStep { q_removed, w_steam_in, w_condensate_return, h_condensate_return, energy_to_pool, pool_boiloff_kg, pool_cooling_w }`, `IcTrain::in_service`, `VesselInputs { q_core, w_ic_steam, w_ic_return, h_ic_return, w_feed, h_feed, w_steam, w_liquid_out }`, `BoundaryFlows.{w_ic_steam, w_ic_return, h_ic_return, q_ic}`, `Case.dt_after_s`, `Sim::domain_error`, `Sim::find_fault`, trace columns `ic_heat_w_i`, `ic_pool_kg_i`, `ic_return_pos_i`, `t_coolant_c`.

**Master contract (r2, already carries the split; restated here for implementers):** there is no single-call `IcTrain::step`; P3 Task 5 landed the interim form and this package keeps the signatures unchanged:

```rust
pub fn request(&self, dt: f64, p_vessel: f64, w_steam_available: f64) -> IcRequest; // pure: no state change
pub fn commit(&mut self, dt: f64, p_vessel: f64, applied: IcRequest) -> IcStep;    // applied = request x vessel allocation ratios; the ONLY state change
```

`commit` is the only state change and carries the master dt rule through `IcTrain.last_error` (P3's guard, kept verbatim). `request` returns the return flow and its enthalpy as well as the steam request because the vessel consumes both at step 8 and `commit` must apply exactly what the vessel received. `EnergyBalance.q_ic_w` is the POOL-side absorption (`Σ IcStep.q_removed`); the vessel-side removal stays in `BoundaryFlows.q_ic`; `d/dt(tube_energy)` is storage.

---

### Task 1: Subsystem specification, evidence rows, and the derived pool inventory

**Files:**
- Create: `docs/model/isolation-condensers.md`
- Modify: `docs/model/parameters.csv`, `docs/model/assumptions.md`, `docs/model/esbwr-applicability.csv`, `docs/model/sources.csv`
- Modify: `crates/bwr-core/src/params.rs` (IC block)
- Test: `crates/bwr-core/src/ic.rs` (unit test `derived_pool_mass_covers_seven_days_to_100c`)

**Interfaces:**
- Consumes: `reactor::DecayHeat::after_history(n, irr_s, cooled_s)` (P5), `vessel::energy_for(mass, t)`, `Sim::new(ScenarioName::FullPower)`, `ic::H_FG_ATM` (P3).
- Produces: constants `N_IC_RETURN_VALVES`, `IC_TUBE_CAPACITY_KG`, `IC_DRAIN_TAU_S`, `IC_POOL_MASS_TOTAL`, `IC_POOL_MASS`, `IC_POOL_COOLING_W`, `IC_POOL_T_AMBIENT`; assumption ids `A-020`..`A-028`; applicability ids `E-07`..`E-09`; source id `S-ESBWR-05`.

- [ ] **Step 1: Record the sources**

```bash
cd /Users/jim/ai/bwrsim && mkdir -p /tmp/bwr-src && cd /tmp/bwr-src
curl -sSLo esbwr-dcd-r10-ch5.pdf "https://www.nrc.gov/docs/ML1410/ML14104A929.pdf"
curl -sSLo 005N9751.pdf "https://www.gevernova.com/content/dam/gevernova-nuclear/global/en_us/documents/carbon-free-power/005N9751_Rev_BWRX-300_General_Description.pdf"
curl -sSLo nedo-33934-r2.pdf "https://www.nrc.gov/docs/ML2607/ML26077A383.pdf"
curl -sSLo nrc-cnsc-containment.pdf "https://www.nrc.gov/docs/ML2209/ML22091A201.pdf"
shasum -a 256 *.pdf
```

If the DCD chapter URL has moved, locate "ESBWR DCD Revision 10 Tier 2 Chapter 5" from source `S-04` and record the URL actually used. In `docs/model/sources.csv` (12 columns `id,document_id,title,revision,date,jurisdiction,url,section,retrieval_date,sha256,kind,notes`):

- Add one row:

```
S-ESBWR-05,ESBWR-DCD-T2-CH5,ESBWR Design Control Document Tier 2 Chapter 5 (Reactor Coolant System and Connected Systems),Rev. 10,2014-04,US,<url used>,5.4.6 Isolation Condenser System,<today>,<sha256>,vendor-design-certified,shared-concept source for IC valve arrangement and pool boil-off; not a BWRX value source
```

- Fill `revision`, `date`, `retrieval_date`, `sha256` on the existing rows `S-06` (005N9751), `S-09` (NRC/CNSC containment), `S-10` (NEDO-33934) if they still read `UNRETRIEVED`, and set their `section` cells to the ICS section/page you read (for `S-10` record the page that states the number of IC trains and the passive cooling duration).

Run: `awk -F',' 'NF!=12{print NR": "NF" fields"}' docs/model/sources.csv`
Expected: no output.

- [ ] **Step 2: Write the subsystem specification**

Create `docs/model/isolation-condensers.md` with exactly this content (the two `<value>` cells are filled in Step 6):

````markdown
# Isolation Condenser System (ICS) — model specification

Package 7 of the refinement program. Status words follow `docs/model/parameters.csv`.
Assumptions are `A-020`..`A-028` in `docs/model/assumptions.md`.

## Function modelled

Passive removal of reactor steam energy to an elevated water pool, with condensate
returned to the vessel by gravity. Modelled uses: overpressure control after
isolation, decay-heat removal after scram with the main condenser unavailable,
and inadvertent initiation at power.

Not modelled (declared boundaries): pool makeup, pool cooling system (credited duty
`IC_POOL_COOLING_W` = 0 W), noncondensable purge lines, IC line break isolation,
containment heat removal by the ICs, and any shutdown cooling below the pool
boiling point.

## Arrangement

| Item | Value | Status | Source / assumption |
|---|---|---|---|
| Trains | `N_IC_TRAINS` = 3 | assumed (to verify) | S-06 / S-10 state the number of trains; executor records the page in `S-10.section` and promotes the row to `confirmed`, or leaves `assumed` with A-020 |
| Steam supply valve per train | 1, normally open | shared concept | S-ESBWR-05 5.4.6; E-07; A-021 |
| Condensate return valves per train | `N_IC_RETURN_VALVES` = 2 in parallel, normally closed | shared concept | S-ESBWR-05 5.4.6 describes parallel return valves; S-09 notes BWRX connections differ; E-07; A-021 |
| Rated duty per train | `IC_TRAIN_RATED_W` = 33.75 MW at 7.2 MPa with a boiling pool | esbwr-estimate | ESBWR IC unit duty (E-08). **Not a BWRX value.** Sensitivity ±30 % reported in `results/ic-availability.csv` |
| Pool water, all trains | `IC_POOL_MASS_TOTAL` = <value> kg | derived | 1.10 x (7-day decay energy after 2 y at rated + vessel stored energy 287.7 -> 100 degC) / h_fg(100 degC). A-022 |
| Pool water per train | `IC_POOL_MASS` = total / 3 | assumed split | separate bays assumed; a common pool would improve the 1- and 2-train results. A-023 |
| Tube/drain condensate capacity | `IC_TUBE_CAPACITY_KG` = 5 000 kg | assumed | A-024 |
| Drain time constant | `IC_DRAIN_TAU_S` = 20 s | assumed | A-025 |
| Valve stroke | `IC_VALVE_STROKE` = 15 s | assumed | A-026 |
| Idle pool floor | `IC_POOL_T_AMBIENT` = 40 degC | assumed | A-027 |

## Equations (per train, each step)

Symbols: `p` vessel pressure, `T_sat = T_sat(p)`, `h_f, h_g, h_fg` at `p`, `T_pool`,
`m_tube` and `E_tube` the tube condensate mass and enthalpy content, `h_tube = E_tube / m_tube`,
`T_tube = T(h_tube)` on the liquid line.

- Valves move linearly at `1/IC_VALVE_STROKE` per second toward their commands; a
  stuck valve or an unavailable train does not move.
- Condensing surface fraction `f_surf = 1 - m_tube / IC_TUBE_CAPACITY_KG`. With the
  return valves closed the tubes flood and condensation stops: that is what keeps a
  standby train idle.
- Pool coverage `cov = clamp(pool_level_pct / 40, 0, 1)`; `UA = IC_TRAIN_RATED_W / (T_sat(7.2 MPa) - 100)`;
  `UA_eff = UA * ua_factor * steam_valve_pos * cov`.
- Tube water cooling to the pool: `q_cool = min(UA_eff * (T_tube - T_pool)+, m_tube * CP_WATER * (T_tube - T_pool)+ / dt, pool capacity/dt)`.
  This is the only heat path when no steam is available; it is bounded by the
  energy stored in the tube water and never continues through a lag.
- **Request** (pure, from the state at the start of the step): condensation
  `w_in_req = min(UA_eff * f_surf * (T_sat - T_pool)+ / h_fg, w_steam_available, (pool capacity/dt - q_cool)/h_fg)`
  (zero when `w_steam_available = 0`); drain `w_return_req = min(sqrt(max(return_valve_pos)) * m_tube / IC_DRAIN_TAU_S, m_tube/dt)` at enthalpy `h_tube`.
- **Allocation** (vessel, master step 8): the vessel bounds all outflows together and
  reports `w_in` and `w_return` applied; `Sim` scales each train's request by the
  vessel's ratios (1.0 when inventory is adequate, the vessel's `scale` when exhausted,
  0.0 when the vessel applied nothing).
- **Commit** (with applied flows): `q_cond = w_in * h_fg`; `q_cool` re-bounded to
  `pool capacity/dt - q_cond`; tubes `dm_tube = w_in - w_return`,
  `dE_tube = w_in*h_f - w_return*h_tube - q_cool`; pool `q_removed = q_cond + q_cool`
  (sensible heating to 100 degC, then boil-off at `H_FG_ATM`, no overshoot); cooling
  to the uncredited system `IC_POOL_COOLING_W`. Valves move inside commit.
- Vessel (applied values): mass `-w_in + w_return`; energy `-w_in*h_g + w_return*h_tube`
  = `BoundaryFlows.q_ic` (vessel-side, DERIVED from the two mass flows).
- Identity, every step and for any applied flow: `q_ic(vessel side) = q_removed(pool side) + dE_tube/dt`.
  In the plant energy balance `q_ic_w` is the pool-side term (boundary) and
  `dE_tube/dt` is storage; the tube energy is counted once.
- Noncondensable degradation: `ua_factor = 1 - pct/100` from a test-level fault.
  No purge or accumulation dynamics (A-028).

## Initiation and control (ASSUMED; protection.csv rows in Package 8)

Automatic open of all available trains' return valves on `IC_HIGH_PRESSURE`, L2,
or MSIV isolation at power. Latched: valves stay commanded open until the operator
closes them. Manual `SetIcTrain` is honoured in both directions; a command to an
unavailable train is rejected with an operator message and a log line.

## Transfer allocation

The IC trains `request` at master step 6 without changing state. The vessel (step 8)
bounds all outflows together and returns the applied IC steam and return flows. At
step 9 `Sim` calls `commit` on every train with its request scaled by the vessel's
ratios, so tubes and pools only ever move with applied transfers. There is no step in
which a pool absorbs energy the vessel did not give up.

## Lower temperature limit (declared boundary)

An IC can only reject heat while `T_sat(p) > T_pool`. With a boiling pool the vessel
cannot be cooled below about 100 degC by the ICs; afterwards the pool cools only by
the uncredited cooling system. Therefore:

- Cold shutdown below 100 degC requires a shutdown cooling system that is not
  implemented. The `ColdShutdown` scenario starts at 40 degC as an **assumed
  initial condition**, not as a state reachable in the simulator.
- Vessel insulation loss (`HEAT_LOSS_HOT`) is reported in the ledger as `q_loss`;
  it is not credited for cooldown and no test accepts cooldown that depends on it.

## Endurance claims

Case `fp_scram_ic_7d`: scram and MSIV closure at t = 5 s from rated power with
equilibrium xenon after 2 y at rated power, 3 trains credited, no makeup, no pool
cooling, no AC power assumed or needed. Reported: pool fraction at 72 h and 7 d,
vessel temperature at 7 d, peak pressure. The 7-day figure is the simulator's own
result under these assumptions, not a reproduction of the vendor claim (S-10).

## Results tables

Written by `tests/ic_transients.rs` with `BWR_WRITE_RESULTS=1` into
`docs/validation/results/ic-availability.csv`, `ic-degradation.csv`,
`ic-inadvertent.csv`, `ic-endurance.csv`.
````

- [ ] **Step 3: Add assumption rows**

Append to the table in `docs/model/assumptions.md` (P1 columns `| id | title | content | sensitivity | package |`):

```markdown
| A-020 | IC train count | Three trains. Public GE descriptions state three; the executor records the page in S-10 or leaves this assumed. | 2..4 trains; see results/ic-availability.csv | Package 7 |
| A-021 | IC valve arrangement | One normally-open steam supply valve and two parallel normally-closed condensate return valves per train (ESBWR arrangement adopted as a shared concept, E-07; BWRX connection differences in S-09 do not change the valve function modelled). Either return valve passes full drain flow. | 1 vs 2 return valves | Package 7 |
| A-022 | IC pool inventory | Derived from 7-day decay energy (2 y at rated) plus vessel stored energy to 100 degC with 10 % margin, divided by h_fg at 100 degC. Sensible capacity of a cold pool (40 -> 100 degC, about +11 %) is not credited. | see results/ic-endurance.csv | Package 7 |
| A-023 | IC pool split | Pool water split equally into separate bays, one per train. A common pool would make 1- and 2-train endurance better than reported. | shared vs separate | Package 7 |
| A-024 | IC tube inventory | Tube and drain condensate capacity 5 000 kg per train; condensing surface falls linearly with flooding. | 2 000..10 000 kg | Package 7 |
| A-025 | IC drain time constant | 20 s through an open return valve. | 10..60 s | Package 7 |
| A-026 | IC valve stroke | 15 s for steam and return valves. | 5..30 s | Package 7 |
| A-027 | IC pool idle floor | Idle pool cools no lower than 40 degC and only via the uncredited cooling term IC_POOL_COOLING_W (0 W). | 20..50 degC; 0..2 MW | Package 7 |
| A-028 | IC noncondensable treatment | Degradation is a fixed UA multiplier set by a test fault; no accumulation or purge dynamics. | 0..50 % | Package 7 |
```

- [ ] **Step 4: Add applicability rows**

Append to `docs/model/esbwr-applicability.csv` (10 columns `id,esbwr_source,esbwr_section,target_subsystem,bwrx_evidence,differences,adaptation,applicability_limits,test,classification`):

```
E-07,DCD Rev 10 Tier 2,Ch 5.4.6 ICS valve arrangement,Isolation condensers,S-06;S-10,BWRX integral RPV isolation valves; IC connections differ (S-09),Valve function and count adopted; no containment heat-removal role,No containment or IC-line-break modelling,ic_transients.rs::stuck_return_valve_is_held_and_parallel_valve_carries_the_train,shared concept
E-08,DCD Rev 10 Tier 2,Ch 5.4.6 IC unit duty 33.75 MWt,Isolation condensers,none public for BWRX,BWRX thermal power 870 vs 4500 MWt; 3 vs 4 trains,Used as per-train duty with +/-30 % sensitivity,Not a BWRX performance claim,ic_transients.rs::train_availability_matrix,provisional numerical estimate
E-09,DCD Rev 10 Tier 2,Ch 5.4.6 pool boil-off and gravity return; Ch 6.2 PCCS pools,Isolation condensers,S-10 7-day passive cooling claim,Pool geometry unknown; BWRX dry containment: ICs serve no containment function,Pool mass derived from energy integral; containment role not used,Containment heat removal inapplicable,ic::tests::derived_pool_mass_covers_seven_days_to_100c,adapted model
```

Run: `awk -F',' 'NF!=10{print NR": "NF" fields"}' docs/model/esbwr-applicability.csv`
Expected: no output.

- [ ] **Step 5: Write the failing pool-inventory test and the constants**

Append inside the `#[cfg(test)] mod tests` block of `crates/bwr-core/src/ic.rs`:

```rust
    #[test]
    fn derived_pool_mass_covers_seven_days_to_100c() {
        use crate::api::ScenarioName;
        use crate::reactor::DecayHeat;
        use crate::sim::Sim;
        use crate::vessel::energy_for;
        let irr = 2.0 * 365.25 * 86400.0;
        let dt = 60.0;
        let mut e_decay = 0.0;
        let mut t = 0.0;
        while t < 7.0 * 86400.0 {
            let p0 = DecayHeat::after_history(1.0, irr, t).power();
            let p1 = DecayHeat::after_history(1.0, irr, t + dt).power();
            e_decay += 0.5 * (p0 + p1) * RATED_POWER_W * dt;
            t += dt;
        }
        let m = Sim::new(ScenarioName::FullPower).vessel.mass;
        let e_stored = energy_for(m, crate::steam::sat_p(P_RATED).t) - energy_for(m, 100.0);
        let m_required = (e_decay + e_stored) / H_FG_ATM;
        eprintln!(
            "decay {:.3e} J, stored {:.3e} J, boil-off {:.0} kg, 1.10x = {:.0} kg",
            e_decay, e_stored, m_required, 1.10 * m_required
        );
        assert!(
            IC_POOL_MASS_TOTAL >= 1.10 * m_required && IC_POOL_MASS_TOTAL <= 1.15 * m_required,
            "IC_POOL_MASS_TOTAL = {IC_POOL_MASS_TOTAL} must lie in 1.10..1.15 x {m_required:.0} kg"
        );
    }
```

Replace the IC block in `crates/bwr-core/src/params.rs` with (`0.0` for the total so the test fails and prints the required value):

```rust
// ---------------------------------------------------------------- isolation condensers
/// Number of isolation condenser trains. ASSUMED (to verify). A-020.
pub const N_IC_TRAINS: usize = 3;
/// Parallel condensate return valves per train. Shared concept (ESBWR, E-07). A-021.
pub const N_IC_RETURN_VALVES: usize = 2;
/// Heat removal per train at rated pressure with a boiling pool, W.
/// ESBWR-ESTIMATE (ESBWR IC unit 33.75 MWt, E-08). Not a BWRX value; sensitivity +/-30 %.
pub const IC_TRAIN_RATED_W: f64 = 33.75e6;
/// Valve stroke time for steam and return valves, s. ASSUMED. A-026.
pub const IC_VALVE_STROKE: f64 = 15.0;
/// Condensate held in one train's tubes and drain line when flooded, kg. ASSUMED. A-024.
pub const IC_TUBE_CAPACITY_KG: f64 = 5_000.0;
/// Drain time constant through an open return valve, s. ASSUMED. A-025.
pub const IC_DRAIN_TAU_S: f64 = 20.0;
/// Total IC pool water, all trains, kg. DERIVED by ic::tests::derived_pool_mass_covers_seven_days_to_100c. A-022.
pub const IC_POOL_MASS_TOTAL: f64 = 0.0;
/// Pool water per train (separate bays). ASSUMED split. A-023.
pub const IC_POOL_MASS: f64 = IC_POOL_MASS_TOTAL / N_IC_TRAINS as f64;
/// Credited pool cooling system duty, W. ASSUMED 0 (no system credited). A-027.
pub const IC_POOL_COOLING_W: f64 = 0.0;
/// Floor for an idle pool temperature, degC. ASSUMED. A-027.
pub const IC_POOL_T_AMBIENT: f64 = 40.0;
```

- [ ] **Step 6: Run the test, read the required mass, set the constant**

Run: `cargo test --release -p bwr-core --lib derived_pool_mass -- --nocapture`
Expected: FAIL with `IC_POOL_MASS_TOTAL = 0 must lie in 1.10..1.15 x <N> kg` and the `eprintln` line above it. (`--lib` because `sim.rs` still compiles against the interim `IcTrain`; nothing else is touched yet.)

Set `IC_POOL_MASS_TOTAL` to the printed `1.10x` value rounded up to the nearest 1 000 kg. Fill the same number into the `<value>` cell of `isolation-condensers.md`.

Run: `cargo test --release -p bwr-core --lib derived_pool_mass`
Expected: PASS.

- [ ] **Step 7: Parameter rows**

In `docs/model/parameters.csv` (13 columns), **update** the existing rows `N_IC_TRAINS`, `IC_TRAIN_RATED_W`, `IC_VALVE_STROKE`, `IC_POOL_MASS` and **append** the new ones. Final rows:

```
N_IC_TRAINS,3,-,Number of isolation condenser trains,-,BWRX-300,assumed,S-06;S-10,2,4,A-020,params.rs,README lists as unconfirmed; promote when S-10 page recorded
N_IC_RETURN_VALVES,2,-,Parallel condensate return valves per train,-,ESBWR,esbwr-estimate,S-ESBWR-05,1,2,A-021,params.rs,E-07 shared concept
IC_TRAIN_RATED_W,33.75e6,W,Heat removal per train with a boiling pool,7.2 MPa,ESBWR,esbwr-estimate,S-ESBWR-05,23.6e6,43.9e6,A-020,params.rs,E-08; +/-30 % sensitivity in results/ic-availability.csv
IC_VALVE_STROKE,15,s,IC steam and return valve full stroke,-,BWR-generic,assumed,,5,30,A-026,params.rs,
IC_TUBE_CAPACITY_KG,5000,kg,Condensate to flood one train's tubes and drain line,-,none,assumed,,2000,10000,A-024,params.rs,
IC_DRAIN_TAU_S,20,s,Drain time constant through an open return valve,-,none,assumed,,10,60,A-025,params.rs,
IC_POOL_MASS_TOTAL,<value>,kg,IC pool water all trains,-,derived,derived,S-10,none,none,A-022,params.rs,1.10 x (7 d decay energy + vessel stored energy to 100 degC)/h_fg(100 degC)
IC_POOL_MASS,<value>/3,kg,IC pool water per train,-,derived,assumed,,none,none,A-023,params.rs,equal split into separate bays
IC_POOL_COOLING_W,0,W,Credited IC pool cooling duty,-,none,assumed,,0,2e6,A-027,params.rs,replaces ic.POOL_IDLE_COOLING_K_PER_S
IC_POOL_T_AMBIENT,40,degC,Idle IC pool temperature floor,-,none,assumed,,20,50,A-027,params.rs,
```

Delete the `ic.POOL_IDLE_COOLING_K_PER_S` row P3 added (the constant is removed in Task 2). Keep `ic.ua_pool_ref`, `ic.CP_WATER`, `ic.H_FG_ATM`, `ic.POOL_BOIL_T` rows as P3 left them.

Run: `cargo test --release -p bwr-core --test register`
Expected: PASS (every `pub const` in `params.rs` has a row).

- [ ] **Step 8: Commit**

```bash
git add docs/model crates/bwr-core/src/params.rs crates/bwr-core/src/ic.rs
git commit -m "docs(ic): isolation condenser specification, evidence rows and derived pool inventory

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 2: Per-train valves, tube inventory with enthalpy, conserved pool, request/commit split

**Files:**
- Modify: `crates/bwr-core/src/ic.rs` (full rewrite below)
- Test: `crates/bwr-core/src/ic.rs` unit tests

**Interfaces:**
- Consumes: `steam::{sat_p, h_liquid, t_from_h_liquid, P_ATM}`, `params::*`.
- Produces: `IcTrain` (master fields plus `steam_valve_cmd_open`, `return_valve_stuck`), `IcRequest { w_steam_in, w_condensate_return, h_condensate_return }` (`Clone, Copy, Debug, Default, PartialEq`, with P3's `IcRequest::scaled(ratio)` kept so P3's `Sim::step` compiles until Task 3 rewires it), `IcStep` (master's seven fields, `Clone, Copy, Debug, Default, PartialEq`), `IcTrain.last_error: Option<DomainError>` (P3's error channel, kept: the master dt rule for the void-returning `commit`), `IcTrain::in_service()`, `IcTrain::request(&self, dt, p_vessel, w_steam_available) -> IcRequest` (pure), `IcTrain::commit(&mut self, dt, p_vessel, applied: IcRequest) -> IcStep`, `IcTrain::envelope_exceeded(t_sat)`, `IcTrain::tube_h()`, `IcTrain::pool_level_pct()`. There is no single-call `IcTrain::step` (P3 removed it). `POOL_IDLE_COOLING_K_PER_S` is deleted (its `parameters.csv` row goes in Task 1; P3's `tests/conservation.rs` use of it is replaced in Task 3 Step 6); `CP_WATER`, `H_FG_ATM`, `POOL_BOIL_T` stay.

Why two calls: the vessel is the transfer-allocation point (master step 8) and needs the train's requested steam and return flows as inputs; the train may only change its tubes and pool with the flows the vessel actually applied (step 9). `request` reads the state at the start of the step and changes nothing; `commit` receives the applied flows and advances valves, tubes and pool with them. The return flow is part of the request because the vessel must receive exactly the condensate the tubes give up.

- [ ] **Step 1: Write the failing unit tests**

Replace the test module in `crates/bwr-core/src/ic.rs` with the following (keep `derived_pool_mass_covers_seven_days_to_100c` from Task 1 inside it; P3's interim tests are replaced by the set below, which keeps P3's `commit_rejects_invalid_arguments_unchanged` for the dt rule):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::steam::{h_liquid, sat_p};

    const DT: f64 = 0.05;
    const INF: f64 = f64::INFINITY;
    fn rated_p() -> f64 {
        P_RATED
    }
    /// Request then commit with everything granted: what Sim does when inventory is adequate.
    fn step(tr: &mut IcTrain, dt: f64, p: f64, w_avail: f64) -> IcStep {
        let r = tr.request(dt, p, w_avail);
        tr.commit(dt, p, r)
    }
    fn run(tr: &mut IcTrain, seconds: f64, p: f64, w_avail: f64) -> IcStep {
        let mut last = IcStep::default();
        for _ in 0..(seconds / DT) as usize {
            last = step(tr, DT, p, w_avail);
        }
        last
    }

    #[test]
    fn request_does_not_change_state() {
        let mut tr = IcTrain::new();
        tr.pool_t = 60.0;
        tr.cmd_open = true;
        run(&mut tr, 30.0, rated_p(), INF);
        let before = format!("{tr:?}");
        let r1 = tr.request(DT, rated_p(), INF);
        let r2 = tr.request(DT, rated_p(), INF);
        assert_eq!(format!("{tr:?}"), before, "request must be pure");
        assert_eq!(r1.w_steam_in, r2.w_steam_in);
        assert_eq!(r1.w_condensate_return, r2.w_condensate_return);
        assert!(r1.w_steam_in > 0.0 && r1.w_condensate_return > 0.0);
    }

    #[test]
    fn commit_uses_applied_not_requested_flows() {
        let p = rated_p();
        let s = sat_p(p);
        let mut tr = IcTrain::new();
        tr.pool_t = POOL_BOIL_T;
        tr.cmd_open = true;
        run(&mut tr, 300.0, p, INF);
        let req = tr.request(DT, p, INF);
        assert!(req.w_steam_in > 5.0);
        let applied = IcRequest { w_steam_in: 0.25 * req.w_steam_in, w_condensate_return: 0.5 * req.w_condensate_return, ..req };
        let m_tube0 = tr.tube_mass;
        let m_pool0 = tr.pool_mass;
        let out = tr.commit(DT, p, applied);
        assert_eq!(out.w_steam_in, applied.w_steam_in);
        assert_eq!(out.w_condensate_return, applied.w_condensate_return);
        assert_eq!(tr.w_steam_in, applied.w_steam_in, "train field carries the APPLIED value");
        let q_cond = applied.w_steam_in * s.h_fg();
        assert!(out.q_removed >= q_cond - 1e-6 && out.q_removed < q_cond + 1.0e5, "pool absorbed applied condensation plus small tube cooling: {}", out.q_removed);
        assert!((tr.tube_mass - (m_tube0 + (applied.w_steam_in - applied.w_condensate_return) * DT)).abs() < 1e-9);
        assert!((out.energy_to_pool - (m_pool0 - tr.pool_mass) * H_FG_ATM).abs() < 1e-6 * out.energy_to_pool + 1.0);
    }

    #[test]
    fn standby_train_is_idle() {
        let mut tr = IcTrain::new();
        tr.pool_t = POOL_BOIL_T;
        assert!((tr.tube_mass - IC_TUBE_CAPACITY_KG).abs() < 1e-9, "standby tubes start flooded");
        // Tube water sits at pool temperature after hours of standby: no sensible heat path either.
        tr.tube_energy = tr.tube_mass * h_liquid(POOL_BOIL_T);
        let out = run(&mut tr, 60.0, rated_p(), INF);
        assert_eq!(out.q_removed, 0.0);
        assert_eq!(out.w_steam_in, 0.0);
        assert!(!tr.in_service());
    }

    #[test]
    fn opening_return_valves_drains_tubes_and_reaches_rated_duty() {
        let mut tr = IcTrain::new();
        tr.pool_t = POOL_BOIL_T;
        tr.cmd_open = true;
        let out = run(&mut tr, 300.0, rated_p(), INF);
        assert!(tr.in_service());
        assert!(tr.tube_mass < 0.2 * IC_TUBE_CAPACITY_KG, "tubes drained: {}", tr.tube_mass);
        let rel = out.q_removed / IC_TRAIN_RATED_W;
        assert!(rel > 0.8 && rel < 1.05, "duty {rel} of rated");
        assert!((out.w_condensate_return - out.w_steam_in).abs() < 0.05 * out.w_steam_in);
        let s = sat_p(rated_p());
        assert!(out.h_condensate_return <= s.h_f + 1.0 && out.h_condensate_return > h_liquid(POOL_BOIL_T));
    }

    #[test]
    fn requested_withdrawal_never_exceeds_available_steam() {
        let mut tr = IcTrain::new();
        tr.pool_t = POOL_BOIL_T;
        tr.cmd_open = true;
        run(&mut tr, 300.0, rated_p(), INF);
        let unlimited = tr.request(DT, rated_p(), INF).w_steam_in;
        assert!(unlimited > 5.0);
        let cap = 0.25 * unlimited;
        let r = tr.request(DT, rated_p(), cap);
        assert!((r.w_steam_in - cap).abs() < 1e-9, "requested {} vs available {cap}", r.w_steam_in);
        let r0 = tr.request(DT, rated_p(), 0.0);
        assert_eq!(r0.w_steam_in, 0.0);
    }

    #[test]
    fn no_steam_limits_heat_to_tube_water_energy_without_lag() {
        let p = rated_p();
        let s = sat_p(p);
        let mut tr = IcTrain::new();
        tr.pool_t = POOL_BOIL_T;
        tr.cmd_open = true;
        run(&mut tr, 300.0, p, INF);
        assert!(tr.heat > 0.8 * IC_TRAIN_RATED_W);
        // Steam supply disappears: refill the tubes with hot condensate to make the bound visible.
        tr.tube_mass = IC_TUBE_CAPACITY_KG;
        tr.tube_energy = tr.tube_mass * s.h_f;
        let releasable = tr.tube_mass * CP_WATER * (s.t - POOL_BOIL_T);
        let heat_before = tr.heat;
        let first = step(&mut tr, DT, p, 0.0);
        assert_eq!(first.w_steam_in, 0.0);
        assert!(first.q_removed < heat_before, "heat must drop at once, not decay through a lag");
        assert!(first.q_removed * DT <= releasable, "first step released more than the tube water holds");
        let mut total = first.energy_to_pool;
        for _ in 0..(600.0 / DT) as usize {
            total += step(&mut tr, DT, p, 0.0).energy_to_pool;
        }
        assert!(total <= releasable * (1.0 + 1e-9), "total {total} > releasable {releasable}");
        assert!(tr.heat < 1.0e3, "heat must die out: {}", tr.heat);
        assert!(tr.tube_h() <= h_liquid(POOL_BOIL_T) + 1.0);
    }

    #[test]
    fn parallel_return_valve_redundancy() {
        let mut both = IcTrain::new();
        both.pool_t = POOL_BOIL_T;
        both.cmd_open = true;
        let mut one = both.clone();
        one.return_valve_stuck[0] = true;
        let q_both = run(&mut both, 300.0, rated_p(), INF).q_removed;
        let q_one = run(&mut one, 300.0, rated_p(), INF).q_removed;
        assert_eq!(one.return_valve_pos[0], 0.0, "stuck valve never moved");
        assert!(one.return_valve_pos[1] > 0.99);
        assert!((q_one - q_both).abs() < 0.01 * q_both, "one valve {q_one} vs both {q_both}");
    }

    #[test]
    fn unavailable_train_valves_do_not_move() {
        let mut tr = IcTrain::new();
        tr.available = false;
        tr.cmd_open = true;
        tr.pool_t = POOL_BOIL_T;
        let out = run(&mut tr, 120.0, rated_p(), INF);
        assert_eq!(tr.return_valve_pos, [0.0; N_IC_RETURN_VALVES]);
        assert_eq!(out.q_removed, 0.0);
        assert_eq!(out.w_steam_in, 0.0);
    }

    #[test]
    fn closed_steam_valve_removes_nothing() {
        let mut tr = IcTrain::new();
        tr.pool_t = POOL_BOIL_T;
        tr.tube_energy = tr.tube_mass * h_liquid(POOL_BOIL_T);
        tr.cmd_open = true;
        tr.steam_valve_cmd_open = false;
        let out = run(&mut tr, 120.0, rated_p(), INF);
        assert!(tr.steam_valve_pos < 1e-9);
        assert_eq!(out.q_removed, 0.0);
        assert_eq!(out.w_steam_in, 0.0);
        assert!(!tr.in_service());
    }

    #[test]
    fn degradation_scales_duty() {
        let mut a = IcTrain::new();
        a.pool_t = POOL_BOIL_T;
        a.cmd_open = true;
        let mut b = a.clone();
        b.ua_factor = 0.5;
        let qa = run(&mut a, 300.0, rated_p(), INF).q_removed;
        let qb = run(&mut b, 300.0, rated_p(), INF).q_removed;
        assert!((qb / qa - 0.5).abs() < 0.03, "ratio {}", qb / qa);
    }

    #[test]
    fn pool_energy_closes_through_boiling() {
        let mut tr = IcTrain::new();
        tr.pool_t = IC_POOL_T_AMBIENT;
        tr.pool_mass = 20_000.0; // small pool so it boils within the test
        tr.cmd_open = true;
        let (m0, t0) = (tr.pool_mass, tr.pool_t);
        let (mut e_in, mut boiloff, mut e_cool) = (0.0, 0.0, 0.0);
        for _ in 0..(1200.0 / DT) as usize {
            let o = step(&mut tr, DT, rated_p(), INF);
            e_in += o.energy_to_pool;
            boiloff += o.pool_boiloff_kg;
            e_cool += o.pool_cooling_w * DT;
            assert!(tr.pool_t <= POOL_BOIL_T + 1e-9, "pool overshoot {}", tr.pool_t);
            assert!(tr.pool_mass >= 0.0);
        }
        assert!(tr.pool_t > POOL_BOIL_T - 1e-6, "pool should be boiling");
        assert!(boiloff > 0.0);
        let expected = m0 * CP_WATER * (POOL_BOIL_T - t0) + boiloff * H_FG_ATM + e_cool;
        assert!((e_in - expected).abs() < 1e-6 * expected, "e_in {e_in} vs {expected}");
        assert!((m0 - boiloff - tr.pool_mass).abs() < 1e-6);
    }

    #[test]
    fn vessel_tubes_pool_identity_holds_each_step() {
        // Vessel-side removal == pool-side absorption + tube storage, for ANY applied flow.
        let p = rated_p();
        let s = sat_p(p);
        let mut tr = IcTrain::new();
        tr.pool_t = 60.0;
        tr.cmd_open = true;
        for k in 0..(400.0 / DT) as usize {
            let req = tr.request(DT, p, INF);
            // Alternate between full and partial allocation to exercise both paths.
            let f = if k % 7 == 3 { 0.4 } else { 1.0 };
            let applied = IcRequest { w_steam_in: f * req.w_steam_in, w_condensate_return: f * req.w_condensate_return, ..req };
            let e_before = tr.tube_energy;
            let o = tr.commit(DT, p, applied);
            let vessel_side = (o.w_steam_in * s.h_g - o.w_condensate_return * o.h_condensate_return) * DT;
            let pool_side = o.energy_to_pool + (tr.tube_energy - e_before);
            assert!((vessel_side - pool_side).abs() < 1e-6 * vessel_side.abs().max(1.0), "step {k}: vessel {vessel_side} vs pool+tubes {pool_side}");
        }
    }

    #[test]
    fn empty_pool_stops_duty_and_flags_envelope() {
        let p = rated_p();
        let t_sat = sat_p(p).t;
        let mut tr = IcTrain::new();
        tr.pool_t = POOL_BOIL_T;
        tr.pool_mass = 50.0;
        tr.cmd_open = true;
        let mut last = IcStep::default();
        for _ in 0..(600.0 / DT) as usize {
            last = step(&mut tr, DT, p, INF);
            assert!(tr.pool_mass >= 0.0);
            assert!(last.q_removed * DT <= last.energy_to_pool + 1e-6);
        }
        assert_eq!(tr.pool_mass, 0.0);
        assert_eq!(last.q_removed, 0.0);
        assert!(tr.envelope_exceeded(t_sat));
        assert!(!tr.envelope_exceeded(90.0), "no demand below the pool boiling point");
    }

    #[test]
    fn below_pool_temperature_removes_nothing() {
        let mut tr = IcTrain::new();
        tr.cmd_open = true;
        tr.tube_mass = 0.0;
        tr.tube_energy = 0.0;
        tr.pool_t = POOL_BOIL_T;
        let o = step(&mut tr, DT, crate::steam::sat_t(90.0).p, INF);
        assert_eq!(o.q_removed, 0.0);
        assert_eq!(o.w_steam_in, 0.0);
    }

    /// Master dt rule and non-finite inputs rule (P3's test, kept): `commit` is the
    /// reporting path; `request` is pure and yields a zero request for the same inputs.
    #[test]
    fn commit_rejects_invalid_arguments_unchanged() {
        use crate::domain::DomainError;
        let mut base = IcTrain::new();
        base.cmd_open = true;
        run(&mut base, 120.0, rated_p(), INF);
        let good = base.request(DT, rated_p(), INF);
        let mut cases: Vec<(&str, f64, f64, IcRequest)> = vec![("dt", 0.0, rated_p(), good), ("dt", -1.0, rated_p(), good)];
        for v in [f64::NAN, f64::INFINITY] {
            cases.push(("dt", v, rated_p(), good));
            cases.push(("p_vessel", DT, v, good));
            cases.push(("applied w_steam_in", DT, rated_p(), IcRequest { w_steam_in: v, ..good }));
            cases.push(("applied w_condensate_return", DT, rated_p(), IcRequest { w_condensate_return: v, ..good }));
            cases.push(("applied h_condensate_return", DT, rated_p(), IcRequest { h_condensate_return: v, ..good }));
        }
        for (what, dt, p, applied) in cases {
            let mut t = base.clone();
            let out = t.commit(dt, p, applied);
            assert_eq!(out, IcStep::default(), "{what}");
            assert!(
                matches!(t.last_error.take(), Some(DomainError::PropertyOutOfRange { what: w, .. }) if w == what),
                "{what}: wrong or missing last_error"
            );
            assert_eq!(format!("{t:?}"), format!("{base:?}"), "{what}: state must be unchanged");
            if what == "dt" || what == "p_vessel" {
                assert_eq!(base.request(dt, p, INF), IcRequest::default(), "{what}: request must be zero");
            }
        }
        assert_eq!(base.request(DT, rated_p(), f64::NAN), IcRequest::default());
    }

    // derived_pool_mass_covers_seven_days_to_100c from Task 1 stays here.
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --release -p bwr-core --lib ic::tests`
Expected: compile errors (`steam_valve_cmd_open`, `return_valve_stuck`, `tube_energy`, `in_service`, `envelope_exceeded`, `tube_h`, `pool_level_pct`, `IC_POOL_COOLING_W` not found; `IcRequest`, `request`, `commit` and `last_error` already exist from P3).

- [ ] **Step 3: Rewrite `ic.rs`**

Replace everything above the test module in `crates/bwr-core/src/ic.rs` with:

```rust
//! Isolation condenser system: passive decay heat removal and overpressure protection.
//!
//! Each train takes reactor steam through a normally-open steam valve into tubes
//! submerged in an elevated water pool. Condensate collects in the tubes and drain
//! line; with the parallel condensate return valves closed the tubes flood and
//! condensation stops (standby). Opening either return valve drains the condensate
//! back to the vessel by gravity and exposes the condensing surface.
//!
//! Two-phase step: `request` (pure) says what the train would take and give this
//! step; the vessel allocates (master step 8); `commit` advances valves, tubes and
//! pool with the APPLIED flows (step 9). Specification: docs/model/isolation-condensers.md.

use crate::domain::DomainError;
use crate::params::*;
use crate::steam::{h_liquid, sat_p, t_from_h_liquid, P_ATM};

/// Liquid water specific heat used for the pool and tube water, J/kgK.
pub const CP_WATER: f64 = 4186.0;
/// Latent heat at atmospheric pressure (IAPWS, 100 degC), J/kg.
pub const H_FG_ATM: f64 = 2.257e6;
/// Pool boiling point at atmospheric pressure, degC (pool assumed vented).
pub const POOL_BOIL_T: f64 = 100.0;

/// Heat transfer UA per train, W/K (rated duty at 7.2 MPa with a boiling pool).
fn ua() -> f64 {
    IC_TRAIN_RATED_W / (sat_p(P_RATED).t - POOL_BOIL_T)
}

/// Transfers a train asks for this step, from the state at the start of the step.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct IcRequest {
    /// Steam requested from the vessel, kg/s.
    pub w_steam_in: f64,
    /// Condensate the tubes would drain to the vessel, kg/s.
    pub w_condensate_return: f64,
    /// Enthalpy of that condensate, J/kg (tube water enthalpy).
    pub h_condensate_return: f64,
}

impl IcRequest {
    /// The request after a single allocation ratio (P3's helper, kept for P3's
    /// `Sim::step` until Task 3 scales steam and return separately).
    pub fn scaled(self, ratio: f64) -> Self {
        Self { w_steam_in: self.w_steam_in * ratio, w_condensate_return: self.w_condensate_return * ratio, h_condensate_return: self.h_condensate_return }
    }
}

/// What one train did in one step, with APPLIED flows.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct IcStep {
    /// Heat delivered to the pool, W (condensation of applied steam + tube water cooling).
    pub q_removed: f64,
    /// Steam taken from the vessel, kg/s (applied).
    pub w_steam_in: f64,
    /// Condensate returned to the vessel, kg/s (applied).
    pub w_condensate_return: f64,
    /// Enthalpy of the returned condensate, J/kg.
    pub h_condensate_return: f64,
    /// Energy absorbed by the pool this step, J (= q_removed * dt).
    pub energy_to_pool: f64,
    /// Pool water boiled off this step, kg.
    pub pool_boiloff_kg: f64,
    /// Energy leaving the pool to the (uncredited) cooling system, W.
    pub pool_cooling_w: f64,
}

#[derive(Clone, Debug)]
pub struct IcTrain {
    /// Steam supply valve, 0..1, normally open.
    pub steam_valve_pos: f64,
    /// Command for the steam valve; false only via Fault::IcSteamValveClosed.
    pub steam_valve_cmd_open: bool,
    /// Parallel condensate return valves, 0..1 each, normally closed.
    pub return_valve_pos: [f64; N_IC_RETURN_VALVES],
    /// Set by Sim from Fault::IcReturnValveStuck; a stuck valve holds position.
    pub return_valve_stuck: [bool; N_IC_RETURN_VALVES],
    /// Operator / protection command to put the train in service.
    pub cmd_open: bool,
    /// False via Fault::IcTrainUnavailable: nothing moves, nothing is removed.
    pub available: bool,
    /// UA multiplier, 1.0 nominal; reduced by noncondensable degradation.
    pub ua_factor: f64,
    /// Condensate held in tubes and drain line, kg.
    pub tube_mass: f64,
    /// Enthalpy content of the tube condensate, J (energy-balance storage term).
    pub tube_energy: f64,
    /// Heat delivered to the pool in the last committed step, W.
    pub heat: f64,
    /// Steam taken from the vessel in the last committed step, kg/s (applied).
    pub w_steam_in: f64,
    /// Condensate returned to the vessel in the last committed step, kg/s (applied).
    pub w_condensate_return: f64,
    pub pool_t: f64,
    pub pool_mass: f64,
    /// Error channel of the void-returning `commit` (master dt rule); drained by `Sim::step` at step 17.
    pub last_error: Option<DomainError>,
}

impl IcTrain {
    /// Standby train at pressure: steam valve open, return valves closed, tubes
    /// flooded with condensate at pool temperature.
    pub fn new() -> Self {
        Self {
            steam_valve_pos: 1.0,
            steam_valve_cmd_open: true,
            return_valve_pos: [0.0; N_IC_RETURN_VALVES],
            return_valve_stuck: [false; N_IC_RETURN_VALVES],
            cmd_open: false,
            available: true,
            ua_factor: 1.0,
            tube_mass: IC_TUBE_CAPACITY_KG,
            tube_energy: IC_TUBE_CAPACITY_KG * h_liquid(IC_POOL_T_AMBIENT),
            heat: 0.0,
            w_steam_in: 0.0,
            w_condensate_return: 0.0,
            pool_t: IC_POOL_T_AMBIENT,
            pool_mass: IC_POOL_MASS,
            last_error: None,
        }
    }

    pub fn pool_level_pct(&self) -> f64 {
        self.pool_mass / IC_POOL_MASS * 100.0
    }

    /// Specific enthalpy of the tube condensate, J/kg.
    pub fn tube_h(&self) -> f64 {
        if self.tube_mass > 0.0 {
            self.tube_energy / self.tube_mass
        } else {
            h_liquid(self.pool_t)
        }
    }

    fn return_open(&self) -> f64 {
        self.return_valve_pos.iter().cloned().fold(0.0, f64::max)
    }

    /// Steam path open and at least one return valve open.
    pub fn in_service(&self) -> bool {
        self.steam_valve_pos > 0.01 && self.return_open() > 0.01
    }

    /// The model cannot supply the demanded heat removal: pool exhausted while
    /// the train is in service and the steam is hotter than the boiling pool.
    pub fn envelope_exceeded(&self, t_sat: f64) -> bool {
        self.pool_mass <= 0.0 && self.in_service() && t_sat > POOL_BOIL_T
    }

    /// Effective UA, W/K: nominal x degradation x steam valve x pool coverage.
    fn ua_eff(&self) -> f64 {
        if !self.available {
            return 0.0;
        }
        let coverage = (self.pool_level_pct() / 40.0).clamp(0.0, 1.0);
        ua() * self.ua_factor * self.steam_valve_pos * coverage
    }

    /// Energy the pool can still absorb, J: sensible to boiling plus latent heat of all of it.
    fn pool_capacity_j(&self) -> f64 {
        (self.pool_mass * CP_WATER * (POOL_BOIL_T - self.pool_t)).max(0.0) + self.pool_mass * H_FG_ATM
    }

    /// Tube-water cooling to the pool, W: the only heat path without steam; bounded by
    /// the energy stored in the tube water, never carried by a lag.
    fn q_cool(&self, dt: f64) -> f64 {
        if self.tube_mass <= 0.0 {
            return 0.0;
        }
        let dt_tube = (t_from_h_liquid(self.tube_h()) - self.pool_t).max(0.0);
        let cap = self.tube_mass * CP_WATER * dt_tube / dt;
        (self.ua_eff() * dt_tube).min(cap).min(self.pool_capacity_j() / dt).max(0.0)
    }

    /// Drain through the open return valves, bounded by the tube inventory: (kg/s, J/kg).
    fn drain(&self, dt: f64) -> (f64, f64) {
        let w = (self.return_open().sqrt() * self.tube_mass / IC_DRAIN_TAU_S).min(self.tube_mass / dt).max(0.0);
        (w, self.tube_h())
    }

    fn move_valves(&mut self, dt: f64) {
        if !self.available {
            return;
        }
        let stroke = dt / IC_VALVE_STROKE;
        let st = if self.steam_valve_cmd_open { 1.0 } else { 0.0 };
        self.steam_valve_pos += (st - self.steam_valve_pos).clamp(-stroke, stroke);
        let rt = if self.cmd_open { 1.0 } else { 0.0 };
        for v in 0..N_IC_RETURN_VALVES {
            if self.return_valve_stuck[v] {
                continue;
            }
            self.return_valve_pos[v] += (rt - self.return_valve_pos[v]).clamp(-stroke, stroke);
        }
    }

    /// Transfers this train asks for, from the state at the start of the step. Pure.
    /// `p_vessel` is reactor pressure (Pa); `w_steam_available` is the vessel's
    /// allocation hint (kg/s it can supply; `f64::INFINITY` when inventory is adequate,
    /// `0.0` when there is no steam space). The train never requests more. An invalid
    /// argument yields a zero request (pure, no error channel); `commit` receives the
    /// same `dt`/`p_vessel` and reports it (P3's rule, kept).
    pub fn request(&self, dt: f64, p_vessel: f64, w_steam_available: f64) -> IcRequest {
        if !(dt.is_finite() && dt > 0.0) || !p_vessel.is_finite() || w_steam_available.is_nan() {
            return IcRequest::default();
        }
        let s = sat_p(p_vessel.max(P_ATM));
        let h_fg = s.h_fg();
        let f_surf = (1.0 - self.tube_mass / IC_TUBE_CAPACITY_KG).clamp(0.0, 1.0);
        let dt_cond = (s.t - self.pool_t).max(0.0);
        let w_in = if w_steam_available > 0.0 && h_fg > 0.0 {
            let q_cond_max = (self.pool_capacity_j() / dt - self.q_cool(dt)).max(0.0);
            (self.ua_eff() * f_surf * dt_cond / h_fg).min(w_steam_available).min(q_cond_max / h_fg).max(0.0)
        } else {
            0.0
        };
        let (w_out, h_ret) = self.drain(dt);
        IcRequest { w_steam_in: w_in, w_condensate_return: w_out, h_condensate_return: h_ret }
    }

    /// Advance valves, tubes and pool with the flows the vessel APPLIED. `applied` is
    /// the request scaled by the vessel's allocation ratio (identical to the request
    /// when inventory was adequate; all zero when the vessel applied nothing).
    pub fn commit(&mut self, dt: f64, p_vessel: f64, applied: IcRequest) -> IcStep {
        // Master dt rule and non-finite inputs rule (P3's guard, kept verbatim): validate
        // everything before any mutation; on failure set last_error and change nothing.
        let checks = [
            ("dt", dt, dt.is_finite() && dt > 0.0),
            ("p_vessel", p_vessel, p_vessel.is_finite()),
            ("applied w_steam_in", applied.w_steam_in, applied.w_steam_in.is_finite()),
            ("applied w_condensate_return", applied.w_condensate_return, applied.w_condensate_return.is_finite()),
            ("applied h_condensate_return", applied.h_condensate_return, applied.h_condensate_return.is_finite()),
        ];
        if let Some((what, value, _)) = checks.iter().find(|(_, _, ok)| !ok) {
            self.last_error = Some(DomainError::PropertyOutOfRange { what: (*what).into(), value: *value });
            return IcStep::default();
        }
        let s = sat_p(p_vessel.max(P_ATM));
        let h_fg = s.h_fg();
        let w_in = applied.w_steam_in.max(0.0);
        let w_out = applied.w_condensate_return.max(0.0).min(self.tube_mass / dt + w_in);
        let h_ret = applied.h_condensate_return;
        let q_cond = w_in * h_fg;
        // Tube cooling is internal to the control volume; it is bounded by the pool's remaining capacity.
        let q_cool = self.q_cool(dt).min((self.pool_capacity_j() / dt - q_cond).max(0.0));
        self.move_valves(dt);

        // Tubes: condensate in at h_f, out at the tube enthalpy, sensible loss to the pool.
        self.tube_mass += (w_in - w_out) * dt;
        self.tube_energy += (w_in * s.h_f - w_out * h_ret - q_cool) * dt;
        if self.tube_mass <= 0.0 {
            self.tube_mass = 0.0;
            self.tube_energy = 0.0;
        }

        // Pool: sensible heating to boiling, then boil-off; no overshoot.
        let q = q_cond + q_cool;
        let cooling_w = if self.pool_t > IC_POOL_T_AMBIENT && self.pool_mass > 0.0 { IC_POOL_COOLING_W } else { 0.0 };
        let mut e = (q - cooling_w) * dt;
        let mut boiloff = 0.0;
        if e > 0.0 {
            if self.pool_t < POOL_BOIL_T {
                let to_boil = self.pool_mass * CP_WATER * (POOL_BOIL_T - self.pool_t);
                if e <= to_boil {
                    self.pool_t += e / (self.pool_mass.max(1e-9) * CP_WATER);
                    e = 0.0;
                } else {
                    self.pool_t = POOL_BOIL_T;
                    e -= to_boil;
                }
            }
            if e > 0.0 {
                boiloff = (e / H_FG_ATM).min(self.pool_mass);
                self.pool_mass -= boiloff;
                if self.pool_mass < 1e-9 {
                    self.pool_mass = 0.0;
                }
            }
        } else if e < 0.0 && self.pool_mass > 0.0 {
            self.pool_t = (self.pool_t + e / (self.pool_mass * CP_WATER)).max(IC_POOL_T_AMBIENT);
        }

        self.heat = q;
        self.w_steam_in = w_in;
        self.w_condensate_return = w_out;
        IcStep {
            q_removed: q,
            w_steam_in: w_in,
            w_condensate_return: w_out,
            h_condensate_return: h_ret,
            energy_to_pool: q * dt,
            pool_boiloff_kg: boiloff,
            pool_cooling_w: cooling_w,
        }
    }
}

impl Default for IcTrain {
    fn default() -> Self {
        Self::new()
    }
}
```

- [ ] **Step 4: Run the unit tests**

Run: `cargo test --release -p bwr-core --lib ic::tests`
Expected: all 16 `ic::tests::*` PASS. P3's `Sim::step` wiring (`request` at step 6, `commit(dt, p, req.scaled(ic_ratio))` at step 9, `ic_boundary.add`, `last_error` drain at step 17) compiles unchanged against this API; its allocation hint and single ratio are replaced in Task 3. `cargo test --release -p bwr-core` as a whole is not expected to pass yet: `tests/conservation.rs` still names `POOL_IDLE_COOLING_K_PER_S` (replaced in Task 3 Step 6).

If `vessel_tubes_pool_identity_holds_each_step` fails, the mismatch is in `commit`: `h_ret`, `s.h_f` and `q_cool` must be the same values that enter `tube_energy`; print `vessel_side - pool_side` and fix the order of operations, do not loosen the tolerance. The identity `w_in*h_g - w_out*h_ret = (q_cond + q_cool) + dE_tube/dt` holds by construction because `dE_tube/dt = w_in*h_f - w_out*h_ret - q_cool` and `h_g - h_f = h_fg`.

- [ ] **Step 5: Commit**

```bash
git add crates/bwr-core/src/ic.rs
git commit -m "feat(ic): per-train valves, tube inventory with enthalpy, conserved pool, request/commit split

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 3: Vessel IC ports, fault view, request/allocate/commit sequence and energy balance in `Sim`

**Files:**
- Modify: `crates/bwr-core/src/vessel.rs` (`VesselInputs`, `step`)
- Modify: `crates/bwr-core/src/ledger.rs` (doc comment only; fields already exist)
- Modify: `crates/bwr-core/src/sim.rs` (`IcFaults`, `ic_faults`, IC request at step 6, commit at step 9, energy balance, `cold_shutdown`, `base`, `SetIcTrain`, auto-initiation)
- Create: `crates/bwr-core/tests/ic_transients.rs`

**Interfaces:**
- Consumes: `Sim::find_fault(kind, equipment)`, `Fault::{IcTrainUnavailable, IcReturnValveStuck, IcSteamValveClosed, IcNoncondensableDegradation}`, `Sim::domain_error`, `DomainError::IcModelEnvelope`, `vessel::Regime`, `bop::EnergyBalance { q_ic_w, storage_w, .. }`, `Sim.log`, `IcTrain::{request, commit}`.
- Produces: `VesselInputs { q_core, w_ic_steam, w_ic_return, h_ic_return, w_feed, h_feed, w_steam, w_liquid_out }`; `BoundaryFlows.{w_ic_steam, w_ic_return, h_ic_return}` populated and `q_ic` DERIVED (vessel-side removal); `IcFaults`; `Sim::ic_faults`; `Sim.ic_envelope_raised`; `pub ic_tube_storage_w: f64` on `Sim` (rate of change of total tube energy in the last step, W); `EnergyBalance.q_ic_w` = energy delivered to the POOLS (`Σ IcStep.q_removed`).

**Two IC energy numbers, defined once:**

| Quantity | Where | Meaning |
|---|---|---|
| `flows.q_ic` | `BoundaryFlows` (vessel-side) | `w_ic_steam*h_steam - w_ic_return*h_ic_return`: energy leaving the VESSEL toward the IC trains (applied). Used by the vessel energy equation and the ledger. |
| `balance.q_ic_w` | `EnergyBalance` (pool-side) | `Σ IcStep.q_removed`: energy crossing the control-volume boundary into the POOLS. |
| `ic_tube_storage_w` | `Sim` | `Σ Δtube_energy / dt`: storage inside the control volume (master P6: `storage` includes `d/dt(ic tube energy)`). |

Identity, every step: `flows.q_ic == balance.q_ic_w + ic_tube_storage_w`. The tube term appears once (as storage), never as a boundary flow.

- [ ] **Step 1: Write the failing plant-level tests**

Create `crates/bwr-core/tests/ic_transients.rs`:

```rust
//! Isolation condenser transients: conservation, allocation, availability, faults,
//! initiation ordering, degradation sensitivity and long-duration endurance.
//! Specification: docs/model/isolation-condensers.md.

use bwr_core::api::{Command, ScenarioName};
use bwr_core::faults::Fault;
use bwr_core::ic::{CP_WATER, H_FG_ATM, POOL_BOIL_T};
use bwr_core::params::*;
use bwr_core::sim::DT;
use bwr_core::Sim;

fn run(sim: &mut Sim, seconds: f64) {
    for _ in 0..(seconds / DT).round() as usize {
        sim.step(DT);
    }
}

/// Scram and close both MSIVs: the ICs become the only heat sink.
fn isolate(sim: &mut Sim) {
    sim.apply(Command::ManualScram);
    sim.apply(Command::SetMsiv { line: 0, open: false });
    sim.apply(Command::SetMsiv { line: 1, open: false });
}

/// Remove every other mass path so vessel + IC tubes is a closed system.
fn seal(sim: &mut Sim) {
    for i in 0..N_FEED_PUMPS {
        sim.set_fault(Fault::FeedPumpTrip { index: i }, true);
    }
    sim.apply(Command::SetRwcuReject { kgs: 0.0 });
    sim.apply(Command::SetHeadVent { open: false });
}

fn results(name: &str, header: &str, rows: &[String]) {
    if std::env::var("BWR_WRITE_RESULTS").is_err() {
        return;
    }
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/validation/results");
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(format!("{dir}/{name}.csv"), format!("{header}\n{}\n", rows.join("\n"))).unwrap();
}

/// Energy the pools have absorbed since a cold start at IC_POOL_T_AMBIENT and IC_POOL_MASS.
fn pool_energy_absorbed(sim: &Sim) -> f64 {
    sim.ic
        .iter()
        .map(|t| {
            let sensible = IC_POOL_MASS * CP_WATER * (t.pool_t.min(POOL_BOIL_T) - IC_POOL_T_AMBIENT);
            let latent = (IC_POOL_MASS - t.pool_mass) * H_FG_ATM;
            sensible + latent
        })
        .sum()
}

/// Per-step identity: vessel-side removal == pool-side absorption + tube storage rate.
fn assert_ic_identity(sim: &Sim, label: &str) {
    let vessel_side = sim.last_flows().q_ic;
    let pool_plus_tubes = sim.balance.q_ic_w + sim.ic_tube_storage_w;
    assert!(
        (vessel_side - pool_plus_tubes).abs() < 1e-6 * vessel_side.abs().max(1.0) + 1.0,
        "{label}: vessel-side {vessel_side} W vs pool {} W + tubes {} W",
        sim.balance.q_ic_w,
        sim.ic_tube_storage_w
    );
}

#[test]
fn vessel_plus_tubes_is_closed_when_only_ics_act() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    isolate(&mut sim);
    seal(&mut sim);
    run(&mut sim, 120.0); // MSIVs shut, IC auto-initiated, feed stopped
    assert!(sim.ic.iter().any(|t| t.in_service()), "IC should be in service after isolation at power");
    assert_eq!(sim.fw.flow, 0.0, "feed must be zero for the closed-system check");
    let total = |s: &Sim| s.vessel.mass + s.ic.iter().map(|t| t.tube_mass).sum::<f64>();
    let m0 = total(&sim);
    let mut max_dev: f64 = 0.0;
    for _ in 0..(600.0 / DT) as usize {
        sim.step(DT);
        assert_eq!(sim.fw.flow, 0.0);
        assert_eq!(sim.vessel.vent_flow, 0.0);
        max_dev = max_dev.max((total(&sim) - m0).abs());
    }
    assert!(max_dev < 1.0, "vessel + tube inventory drifted by {max_dev} kg");
    assert!(sim.ic.iter().map(|t| t.w_steam_in).sum::<f64>() > 1.0, "ICs should be condensing");
    assert!(sim.ledger.mass_residual(sim.vessel.mass).abs() < 1.0e-3);
}

#[test]
fn ic_energy_identity_holds_at_plant_level() {
    // Per step: flows.q_ic (vessel side) == balance.q_ic_w (pool side) + tube storage.
    // Integrated: ledger ∫q_ic dt == pool energy absorbed + Δ(tube energy).
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    let e_pool0 = pool_energy_absorbed(&sim);
    let e_tube0: f64 = sim.ic.iter().map(|t| t.tube_energy).sum();
    let ic_j0 = *sim.ledger.integrated.get("ic_j").unwrap_or(&0.0);
    isolate(&mut sim);
    let mut e_pool_steps = 0.0;
    for k in 0..(600.0 / DT) as usize {
        sim.step(DT);
        assert_ic_identity(&sim, &format!("step {k}"));
        e_pool_steps += sim.balance.q_ic_w * DT;
    }
    let d_pool = pool_energy_absorbed(&sim) - e_pool0;
    let d_tube = sim.ic.iter().map(|t| t.tube_energy).sum::<f64>() - e_tube0;
    let ic_j = sim.ledger.integrated["ic_j"] - ic_j0;
    assert!(ic_j > 1.0e9, "ICs should have removed GJ by now: {ic_j}");
    assert!((ic_j - (d_pool + d_tube)).abs() < 1e-6 * ic_j + 1.0e3, "ledger {ic_j} vs pools {d_pool} + tubes {d_tube}");
    assert!((e_pool_steps - d_pool).abs() < 1e-6 * d_pool + 1.0e3, "balance.q_ic_w must be the pool-side number: {e_pool_steps} vs {d_pool}");
    let w_in: f64 = sim.ic.iter().map(|t| t.w_steam_in).sum();
    assert!((sim.last_flows().w_ic_steam - w_in).abs() < 1e-9, "trains must carry the APPLIED withdrawal");
}

#[test]
fn ic_commit_uses_applied_flows_when_the_vessel_runs_dry() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    isolate(&mut sim);
    seal(&mut sim);
    run(&mut sim, 120.0);
    let w_req: f64 = sim.ic.iter().map(|t| t.request(DT, sim.vessel.eos.p, f64::INFINITY).w_steam_in).sum();
    assert!(w_req > 1.0);
    let pool_before = pool_energy_absorbed(&sim);
    // Drain the vessel artificially to force allocation in the next step.
    sim.vessel.mass = 0.5 * w_req * DT;
    sim.vessel.energy = bwr_core::vessel::energy_for(sim.vessel.mass, sim.vessel.eos.t);
    sim.step(DT);
    let flows = sim.last_flows();
    assert!(sim.domain_errors.iter().any(|(_, e)| matches!(e, bwr_core::domain::DomainError::InventoryExhausted { .. })));
    let w_applied: f64 = sim.ic.iter().map(|t| t.w_steam_in).sum();
    assert!((w_applied - flows.w_ic_steam).abs() < 1e-9, "trains {w_applied} vs applied {}", flows.w_ic_steam);
    assert!(w_applied < w_req, "allocation must have scaled the IC request down");
    // The pools absorbed the APPLIED condensation (plus bounded tube cooling), not the request.
    let d_pool = pool_energy_absorbed(&sim) - pool_before;
    let h_fg = sim.vessel.sat().h_fg();
    assert!(d_pool < w_req * h_fg * DT, "pool absorbed the requested energy: {d_pool} J");
    assert_ic_identity(&sim, "exhausted step");
    assert!(sim.vessel.mass >= 0.0);
}
```

`Sim::last_flows()` is P2/P3's accessor for the `BoundaryFlows` of the most recent step (`grep -n "fn last_flows" crates/bwr-core/src/sim.rs`). If it does not exist, add `pub fn last_flows(&self) -> BoundaryFlows { self.last_flows }` with a private `last_flows: BoundaryFlows` field set right after `self.vessel.step(..)`, and list it in the commit message as a contract addition.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --release -p bwr-core --test ic_transients`
Expected: compile errors in the test (`ic_tube_storage_w` missing on `Sim`; `VesselInputs` has no `w_ic_steam` field) — the crate is not wired until Steps 3–4.

- [ ] **Step 3: `VesselInputs` and the vessel step**

In `crates/bwr-core/src/vessel.rs` replace the `VesselInputs` struct:

```rust
#[derive(Clone, Debug)]
pub struct VesselInputs {
    /// Heat from the core into the coolant, W.
    pub q_core: f64,
    /// Steam requested by the isolation condensers, kg/s.
    pub w_ic_steam: f64,
    /// Condensate the isolation condensers would return, kg/s.
    pub w_ic_return: f64,
    /// Enthalpy of the returned condensate, J/kg.
    pub h_ic_return: f64,
    pub w_feed: f64,
    pub h_feed: f64,
    /// Steam requested through the main steam lines, kg/s.
    pub w_steam: f64,
    /// Liquid removed (RWCU reject), kg/s.
    pub w_liquid_out: f64,
}
```

In `Vessel::step` (P3's bounded form), treat IC steam as a steam outflow and IC return as an inflow. The block from `let steam_demand = ...` to the `flows` literal becomes:

```rust
        let steam_space = matches!(self.eos.regime, Regime::Saturated | Regime::GasBlanketed);
        let steam_demand = if steam_space { inp.w_steam } else { 0.0 };
        let ic_demand = if steam_space { inp.w_ic_steam } else { 0.0 };
        let out_demand = steam_demand + ic_demand + inp.w_liquid_out + vent_demand;
        let available = self.mass + (inp.w_feed + inp.w_ic_return) * dt;
        let scale = if out_demand * dt > available {
            (available / (out_demand * dt)).clamp(0.0, 1.0)
        } else {
            1.0
        };
        if scale < 1.0 {
            self.exhausted = Some(out_demand * dt - available);
        }
        let w_steam = steam_demand * scale;
        let w_ic_steam = ic_demand * scale;
        let w_liquid_out = inp.w_liquid_out * scale;
        self.vent_flow = vent_demand * scale;

        let dm = inp.w_feed + inp.w_ic_return - w_steam - w_ic_steam - w_liquid_out - self.vent_flow;
        let q_ic = w_ic_steam * h_steam - inp.w_ic_return * inp.h_ic_return;
        let de = inp.q_core - q_ic - heat_loss + inp.w_feed * inp.h_feed
            - (w_steam + self.vent_flow) * h_steam
            - w_liquid_out * h_liq;
        let flows = BoundaryFlows {
            w_feed: inp.w_feed,
            h_feed: inp.h_feed,
            w_steam,
            h_steam,
            w_vent: self.vent_flow,
            w_liquid_out,
            h_liquid_out: h_liq,
            w_ic_steam,
            w_ic_return: inp.w_ic_return,
            h_ic_return: inp.h_ic_return,
            q_core: inp.q_core,
            q_ic,
            q_loss: heat_loss,
            mass_clipped: 0.0,
            energy_clipped: 0.0,
        };
```

(`steam_space` replaces P3's `!self.eos.subcooled` after P4; keep whichever predicate P4 left, but it must gate `ic_demand` too.) `de` uses the derived `q_ic`, so the vessel energy and the ledger agree by construction. On the `solve_eos` error path the vessel already returns `BoundaryFlows::default()` (nothing applied, including IC flows); Step 4 commits the trains with zero applied flows in that case.

In `crates/bwr-core/src/ledger.rs` change the `q_ic` doc comment to:

```rust
    /// W, vessel-side: DERIVED as w_ic_steam*h_steam - w_ic_return*h_ic_return (never added separately).
    pub q_ic: f64,
```

and confirm `Ledger::record` does not add `w_ic_steam*h_steam` or `w_ic_return*h_ic_return` to the energy sums (it must count `q_ic` only; the master's P2 record already does this: `energy_out_j += (q_ic + q_loss + (w_steam + w_vent)*h_steam + w_liquid_out*h_liquid_out)*dt`). If it does, delete those terms.

Update every other `VesselInputs { .. }` literal in the crate (examples, `tests/conservation.rs`, `tests/vessel_transients.rs`): `grep -rn "VesselInputs {" crates/bwr-core` and replace `q_ic: <x>` with `w_ic_steam: 0.0, w_ic_return: 0.0, h_ic_return: 0.0` unless the test exercises ICs.

- [ ] **Step 4: Fault view, request (step 6), commit (step 9) and energy balance in `Sim::step`**

Add to `crates/bwr-core/src/sim.rs` (near `bop_faults`):

```rust
/// Per-train fault view, built from `Sim.faults` once per step.
#[derive(Clone, Copy, Debug, Default)]
pub struct IcFaults {
    pub unavailable: bool,
    pub steam_closed: bool,
    pub return_stuck: [bool; N_IC_RETURN_VALVES],
    /// 0.0..1.0 fraction of UA lost to noncondensables.
    pub degradation: f64,
}

impl Sim {
    pub fn ic_faults(&self, train: usize) -> IcFaults {
        let mut f = IcFaults::default();
        f.unavailable = self.find_fault("IcTrainUnavailable", Some(train)).is_some();
        f.steam_closed = self.find_fault("IcSteamValveClosed", Some(train)).is_some();
        for v in 0..N_IC_RETURN_VALVES {
            f.return_stuck[v] = self.find_fault("IcReturnValveStuck", Some(train * N_IC_RETURN_VALVES + v)).is_some();
        }
        if let Some(Fault::IcNoncondensableDegradation { pct, .. }) = self.find_fault("IcNoncondensableDegradation", Some(train)) {
            f.degradation = (*pct as f64 / 100.0).clamp(0.0, 1.0);
        }
        f
    }
}
```

Add to `Sim`: the private field `ic_envelope_raised: Vec<bool>` (initialised in `Sim::base` as `vec![false; N_IC_TRAINS]`) and the public field `pub ic_tube_storage_w: f64` (initialised `0.0`; it is per-step state, so `finalize_init` leaves both alone).

Replace P3's step-6 block in `Sim::step` (from `// 6: isolation condensers: pure requests` through the `let q_ic: f64 = ic_requests.iter()...` line; its `h_fg`/`w_steam_available` hint is superseded) with:

```rust
        // --- isolation condensers (step 6): fault view, then REQUESTED transfers; no train state changes here
        let steam_space = matches!(self.vessel.eos.regime, Regime::Saturated | Regime::GasBlanketed);
        let other_out = self.steam.vessel_steam_flow() + self.fw.rwcu_reject + self.vessel.vent_flow;
        let w_ic_avail = if steam_space { (self.vessel.mass / dt + self.fw.flow - other_out).max(0.0) } else { 0.0 };
        let views: Vec<IcFaults> = (0..self.ic.len()).map(|i| self.ic_faults(i)).collect();
        for (tr, f) in self.ic.iter_mut().zip(&views) {
            tr.available = !f.unavailable;
            tr.steam_valve_cmd_open = !f.steam_closed;
            tr.return_valve_stuck = f.return_stuck;
            tr.ua_factor = 1.0 - f.degradation;
        }
        let requests: Vec<IcRequest> = self.ic.iter().map(|tr| tr.request(dt, p, w_ic_avail)).collect();
        let w_ic_req: f64 = requests.iter().map(|r| r.w_steam_in).sum();
        let w_ic_ret_req: f64 = requests.iter().map(|r| r.w_condensate_return).sum();
        let h_ic_return = if w_ic_ret_req > 0.0 {
            requests.iter().map(|r| r.w_condensate_return * r.h_condensate_return).sum::<f64>() / w_ic_ret_req
        } else {
            self.vessel.sat().h_f
        };
```

Add `use crate::ic::IcRequest;`, `use crate::vessel::Regime;` and `use crate::faults::Fault;` if missing. Build the vessel inputs (step 8):

```rust
        let inputs = VesselInputs {
            q_core: self.q_core,
            w_ic_steam: w_ic_req,
            w_ic_return: w_ic_ret_req,
            h_ic_return,
            w_feed: self.fw.flow,
            h_feed: self.fw.h_feed(),
            w_steam: self.steam.vessel_steam_flow(),
            w_liquid_out: self.fw.rwcu_reject,
        };
        let flows = self.vessel.step(dt, &inputs);
```

Directly after it, replace P3's `// 9 (IC): commit with the ratio-scaled request` block (single `ic_ratio` from `flows.q_ic / q_ic`) in the step-9 allocation block (where `steam.w_tcv`, `steam.w_bypass`, `fw.rwcu_reject` are scaled by the allocation ratio) with separate steam and return ratios; `Sim.ic_boundary` (P3) keeps integrating every `IcStep`:

```rust
        // step 9: commit each train with what the vessel actually applied
        let steam_ratio = if w_ic_req > 0.0 { flows.w_ic_steam / w_ic_req } else { 0.0 };
        let return_ratio = if w_ic_ret_req > 0.0 { flows.w_ic_return / w_ic_ret_req } else { 0.0 };
        let e_tube0: f64 = self.ic.iter().map(|t| t.tube_energy).sum();
        let mut q_ic_pool = 0.0;
        self.ic_boundary.q_removed_w = 0.0;
        for (tr, r) in self.ic.iter_mut().zip(&requests) {
            let applied = IcRequest {
                w_steam_in: r.w_steam_in * steam_ratio,
                w_condensate_return: r.w_condensate_return * return_ratio,
                h_condensate_return: r.h_condensate_return,
            };
            let out = tr.commit(dt, p, applied);
            q_ic_pool += out.q_removed;
            self.ic_boundary.add(&out, dt);
        }
        self.ic_tube_storage_w = (self.ic.iter().map(|t| t.tube_energy).sum::<f64>() - e_tube0) / dt;
```

(`IcRequest::scaled` is now unused by `Sim`; leave it, it is `pub` and harmless.)

Both ratios are 1.0 whenever inventory is adequate, equal to the vessel's `scale` in an exhausted step, and 0.0 when the vessel applied nothing (EOS error path), so the trains never move mass or energy the vessel did not. The tube cooling term `q_cool` still runs inside `commit` in that case; it is internal (tubes → pool) and shows up as negative storage plus positive `q_ic_w`, which keeps the identity at `0 = q_cool - q_cool`.

In P6's energy-balance code (final form after P6 Task 7), make exactly these edits so the master's storage sum gains `d/dt(ic tube energy)` and `q_ic_w` becomes the pool-side term. The `stored_before` sum at the top of `Sim::step` (after P3's `dt` guard) and the `stored_after` sum in the `// --- plant energy balance` block both read

```rust
        let stored_before = self.vessel.energy
            + self.steam.line.energy
            + self.cond.energy
            + self.fw.heater_energy
            + self.turbine.kinetic_energy();
        // + IC tube energy from Package 7 (0 until then)
```

(and the same five terms for `stored_after`); replace the placeholder comment on `stored_before` and append the same term to `stored_after`:

```rust
            + self.turbine.kinetic_energy()
            + self.ic.iter().map(|t| t.tube_energy).sum::<f64>(); // P7: d/dt(ic tube energy) is storage
```

In the `let mut b = EnergyBalance { .. }` literal replace the line `q_ic_w: flows.q_ic,                 // P7 replaces with pool-side absorption + tube storage` with

```rust
            q_ic_w: q_ic_pool,                  // pool-side absorption: energy crossing the control-volume boundary
```

Leave `q_line_wall_w: self.steam.line.q_wall_w`, `storage_w: (stored_after - stored_before) / dt` and the `b.residual_w = ...` line as P6 wrote them: the tube term now enters `storage_w` through the finite difference, and `self.ic_tube_storage_w` (same quantity, computed at step 9) is the value the tests read. `b.q_ic_w` is NOT `flows.q_ic`; the vessel-side number stays in the ledger. The debug assertion `(flows.q_ic - (q_ic_pool + self.ic_tube_storage_w)).abs() < 1e-6 * flows.q_ic.abs().max(1.0) + 1.0` may be added under `debug_assert!` here; the integration test enforces it in release.

At master step 17, keep P3's `ic_errs` drain (`self.ic.iter_mut().filter_map(|t| t.last_error.take())` into `domain_error`) and P6's `last_error` drains, and add after them:

```rust
        let t_sat_now = self.vessel.eos.t;
        for i in 0..self.ic.len() {
            if !self.ic_envelope_raised[i] && self.ic[i].envelope_exceeded(t_sat_now) {
                self.ic_envelope_raised[i] = true;
                self.domain_error(DomainError::IcModelEnvelope { train: i, reason: format!("train {} pool depleted", i + 1) });
            }
        }
```

In `Sim::cold_shutdown()` after `Self::base(...)` and before `finalize_init`:

```rust
        for t in s.ic.iter_mut() {
            t.tube_mass = 0.0; // no steam for 30 days: tubes drained
            t.tube_energy = 0.0;
        }
```

In `apply`, replace the `SetIcTrain` arm:

```rust
            Command::SetIcTrain { index, open } => {
                if index < self.ic.len() {
                    if !self.ic[index].available {
                        self.event("ic_cmd", format!("IC train {} unavailable", index + 1));
                        self.log.push((self.time, format!("command ignored: SetIcTrain {} (unavailable)", index + 1)));
                        return;
                    }
                    self.ic[index].cmd_open = open;
                    self.log.push((self.time, format!("ic manual: train {} {}", index + 1, if open { "open" } else { "close" })));
                }
            }
```

(Master log-line convention: `command ignored` is a locked key; `ic manual` is NEW and is appended to the master's locked key list in this task's commit, as the convention requires.)

In `protection()`, replace the auto-initiation block so unavailable trains are skipped and the log records the event with the master's event prefix:

```rust
        if self.vessel.eos.p > IC_HIGH_PRESSURE || self.level_cm() < L2_CM || isolated_at_power {
            let mut started = false;
            for t in self.ic.iter_mut().filter(|t| t.available && !t.cmd_open) {
                t.cmd_open = true;
                started = true;
            }
            if started {
                self.event("ic_init", "Isolation condensers auto-initiated");
                self.log.push((self.time, "ic auto-initiation: all available trains".into()));
            }
        }
```

(The master's `Metrics.first_event_time` keys on the text before the first `:`; the key is therefore `ic auto-initiation`.)

- [ ] **Step 5: Populate the trace columns**

In `crates/bwr-core/src/validation.rs` `sample(sim)`, replace the three IC triplets' placeholder assignments with:

```rust
    for i in 0..N_IC_TRAINS.min(3) {
        let tr = &sim.ic[i];
        r[col_index(&format!("ic_heat_w_{i}"))] = tr.heat;
        r[col_index(&format!("ic_pool_kg_{i}"))] = tr.pool_mass;
        r[col_index(&format!("ic_return_pos_{i}"))] = tr.return_valve_pos.iter().cloned().fold(0.0, f64::max);
    }
```

(`t_coolant_c` is already sampled from `sim.vessel.eos.t` by P2.) `heat` is the committed (applied) value because `sample` runs after `step`.

- [ ] **Step 6: Build, fix call sites, run**

Run: `cargo build --release -p bwr-core` (read the `error[...]` blocks directly; no pipe, so the exit status is cargo's)
Expected: errors only at remaining `VesselInputs { q_ic, .. }` literals (fix per Step 3), at `POOL_IDLE_COOLING_K_PER_S` in `tests/conservation.rs`, and in P3's IC identity tests, which assumed no tube storage. Update those two P3 tests (they are the only tests whose expectations change, and the change is a declared model change, not a concealed failure):
- `tests/numerics.rs::ic_pool_energy_equals_vessel_side_removal_every_step`: the per-step identity becomes `f.q_ic == sim.ic_boundary.q_removed_w + sim.ic_tube_storage_w` (same tolerance), and the run-total identity becomes `ledger ic_j == ic_boundary.energy_to_pool_j + (Σ tube_energy at end − Σ tube_energy at start)`; keep the `energy_to_pool_j > 0` and `condensate_return_kg > 0` assertions.
- `tests/conservation.rs` EOS-rejection test: replace `POOL_IDLE_COOLING_K_PER_S * DT` with `IC_POOL_COOLING_W * DT / (pool_m * CP_WATER)`; the pool may still gain exactly the energy the tubes released on the rejected step (internal `q_cool`), so replace `assert_eq!(sim.ic_boundary.energy_to_pool_j, e_pool, ..)` with `assert!(((sim.ic_boundary.energy_to_pool_j - e_pool) - (tube_e_before - tube_e_after)).abs() < 1.0)` (capture `Σ tube_energy` before and after like the pool values) and keep `pool_boiloff_kg`/`condensate_return_kg` unchanged (nothing applied).

Run: `cargo test --release -p bwr-core --test ic_transients`
Expected: three tests PASS.

Run: `cargo test --release -p bwr-core`
Expected: everything passes except the P2 baseline comparison (regenerated in Task 10) and possibly `register` if a `params.rs` row is missing. `plant::msiv_closure_scrams_and_isolation_condensers_cool` asserts `t.heat > 1.0e6` for any train and still holds. Do not change any other test's expected value in this task.

- [ ] **Step 7: Commit**

```bash
git add crates/bwr-core/src/vessel.rs crates/bwr-core/src/ledger.rs crates/bwr-core/src/sim.rs crates/bwr-core/src/validation.rs crates/bwr-core/tests crates/bwr-core/examples
git commit -m "feat(ic): vessel IC mass ports, derived q_ic, fault view, request/allocate/commit, pool-side energy balance, trace columns

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 4: Initiation, latching, manual control and fault behaviour at plant level

**Files:**
- Modify: `crates/bwr-core/tests/ic_transients.rs`
- Modify: `crates/bwr-core/src/sim.rs` only if a test exposes a defect

**Interfaces:**
- Consumes: Task 3 wiring, `Sim.log`, `Sim.domain_errors`, `Sim::has_fault`.

- [ ] **Step 1: Write the tests**

Append to `crates/bwr-core/tests/ic_transients.rs`:

```rust
#[test]
fn auto_initiation_opens_only_available_trains_and_latches() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    sim.set_fault(Fault::IcTrainUnavailable { train: 2 }, true);
    isolate(&mut sim);
    run(&mut sim, 120.0);
    assert!(sim.ic[0].cmd_open && sim.ic[1].cmd_open, "available trains initiated");
    assert!(!sim.ic[2].cmd_open, "unavailable train must not be commanded");
    assert_eq!(sim.ic[2].return_valve_pos, [0.0; N_IC_RETURN_VALVES]);
    assert!(sim.log.iter().any(|(_, t)| t.starts_with("ic auto-initiation")));
    // Latch: the initiating condition clears as pressure falls, valves stay open.
    run(&mut sim, 600.0);
    assert!(sim.vessel.eos.p < IC_HIGH_PRESSURE);
    assert!(sim.ic[0].cmd_open && sim.ic[0].in_service());
    // Operator close is honoured; the signal is gone so it does not re-initiate.
    sim.apply(Command::SetIcTrain { index: 0, open: false });
    run(&mut sim, 60.0);
    assert!(!sim.ic[0].cmd_open);
    assert!(sim.ic[0].return_valve_pos.iter().all(|&p| p < 0.01));
    // The automatic logic must not have cleared the fault.
    assert!(sim.has_fault(&Fault::IcTrainUnavailable { train: 2 }));
}

#[test]
fn manual_command_to_unavailable_train_is_rejected() {
    let mut sim = Sim::new(ScenarioName::HotStandby);
    sim.set_fault(Fault::IcTrainUnavailable { train: 1 }, true);
    sim.apply(Command::SetIcTrain { index: 1, open: true });
    run(&mut sim, 10.0);
    assert!(!sim.ic[1].cmd_open);
    let snap = sim.snapshot();
    assert!(snap.alarms.iter().any(|a| a.text.contains("IC train 2 unavailable")), "{:?}", snap.alarms);
    assert!(sim.log.iter().any(|(_, t)| t.starts_with("command ignored: SetIcTrain 2")));
}

#[test]
fn stuck_return_valve_is_held_and_parallel_valve_carries_the_train() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    sim.set_fault(Fault::IcReturnValveStuck { train: 0, valve: 0 }, true);
    isolate(&mut sim);
    run(&mut sim, 300.0);
    assert_eq!(sim.ic[0].return_valve_pos[0], 0.0);
    assert!(sim.ic[0].return_valve_pos[1] > 0.99);
    assert!(sim.ic[0].in_service());
    let q0 = sim.ic[0].heat;
    let q1 = sim.ic[1].heat;
    assert!((q0 - q1).abs() < 0.02 * q1, "train with one stuck valve {q0} vs healthy {q1}");
    // The fault lookup is by (kind, equipment): train 0 valve 1 must be unaffected.
    assert!(sim.find_fault("IcReturnValveStuck", Some(1)).is_none());
}

#[test]
fn closed_steam_valve_train_removes_nothing_at_plant_level() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    sim.set_fault(Fault::IcSteamValveClosed { train: 1 }, true);
    isolate(&mut sim);
    run(&mut sim, 300.0);
    assert!(sim.ic[1].steam_valve_pos < 1e-9);
    assert!(sim.ic[1].heat < 1.0, "a closed steam path may only release residual tube heat: {}", sim.ic[1].heat);
    assert!(sim.ic[0].heat > 1.0e6 && sim.ic[2].heat > 1.0e6);
    assert!(sim.domain_errors.is_empty(), "a closed valve is a fault, not a model-envelope breach");
}

#[test]
fn pool_depletion_raises_model_envelope_error_once() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    for t in sim.ic.iter_mut() {
        t.pool_mass = 2_000.0; // tiny pools so depletion happens inside the test
        t.pool_t = POOL_BOIL_T;
    }
    isolate(&mut sim);
    run(&mut sim, 1800.0);
    let ic_errors = sim
        .domain_errors
        .iter()
        .filter(|(_, e)| matches!(e, bwr_core::domain::DomainError::IcModelEnvelope { .. }))
        .count();
    assert_eq!(ic_errors, 1, "domain_error dedups by variant: {:?}", sim.domain_errors);
    let depletion_logs = sim.log.iter().filter(|(_, t)| t.contains("pool depleted")).count();
    assert_eq!(depletion_logs, 1, "no re-raising per step: {:?}", sim.log);
    let snap = sim.snapshot();
    assert!(snap.unsupported.is_some());
    assert!(snap.alarms.iter().any(|a| a.id == "unsupported"));
    assert!(sim.ic.iter().all(|t| t.heat == 0.0 && t.pool_mass == 0.0));
}

#[test]
fn shutdown_mode_does_not_reset_ic_initiation() {
    use bwr_core::api::ModeSwitch;
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    isolate(&mut sim);
    run(&mut sim, 60.0);
    assert!(sim.ic.iter().all(|t| t.cmd_open));
    sim.apply(Command::SetModeSwitch { mode: ModeSwitch::Shutdown });
    sim.apply(Command::ResetScram);
    run(&mut sim, 60.0);
    assert!(sim.ic.iter().all(|t| t.cmd_open), "IC latch survives mode change and scram reset attempt");
}
```

- [ ] **Step 2: Run**

Run: `cargo test --release -p bwr-core --test ic_transients`
Expected: all PASS. If `auto_initiation_opens_only_available_trains_and_latches` fails at the `in_service()` assertion after 600 s, some other code is writing `cmd_open = false` (`grep -n "cmd_open = false" crates/bwr-core/src/sim.rs`); the only writer must be `SetIcTrain`.

- [ ] **Step 3: Commit**

```bash
git add crates/bwr-core/tests/ic_transients.rs crates/bwr-core/src/sim.rs
git commit -m "test(ic): initiation, latching, manual control and fault behaviour

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 5: Noncondensable degradation sensitivity

**Files:**
- Modify: `crates/bwr-core/tests/ic_transients.rs`

**Interfaces:**
- Consumes: `Fault::IcNoncondensableDegradation { train, pct }` via `Sim::ic_faults`, `isolate`, `results`.
- Produces: `docs/validation/results/ic-degradation.csv` when `BWR_WRITE_RESULTS=1`.

- [ ] **Step 1: Write the test**

Append:

```rust
/// Peak pressure and time for the vessel to cool to 150 degC after isolation,
/// with every train's UA degraded by `pct` %.
fn degraded_response(pct: u8) -> (f64, Option<f64>) {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    for train in 0..N_IC_TRAINS {
        sim.set_fault(Fault::IcNoncondensableDegradation { train, pct }, true);
    }
    isolate(&mut sim);
    let mut peak: f64 = 0.0;
    let mut t150 = None;
    while sim.time < 4.0 * 3600.0 {
        sim.step(DT);
        peak = peak.max(sim.vessel.eos.p);
        if t150.is_none() && sim.vessel.eos.t < 150.0 {
            t150 = Some(sim.time);
        }
    }
    assert!((sim.ic[0].ua_factor - (1.0 - pct as f64 / 100.0)).abs() < 1e-12, "fault view applied");
    (peak, t150)
}

#[test]
fn noncondensable_degradation_monotonically_worsens_response() {
    let mut rows = Vec::new();
    let mut prev_peak = 0.0;
    let mut prev_t150 = 0.0;
    for pct in [0u8, 25, 50] {
        let (peak, t150) = degraded_response(pct);
        let t150_s = t150.unwrap_or(f64::INFINITY);
        rows.push(format!("{pct},{:.0},{}", peak, if t150_s.is_finite() { format!("{t150_s:.0}") } else { "not reached in 4 h".into() }));
        assert!(peak >= prev_peak - 1.0, "peak pressure fell with more degradation: {peak} < {prev_peak}");
        assert!(t150_s >= prev_t150 - 1.0, "cooldown got faster with more degradation");
        assert!(peak < VESSEL_DESIGN_PRESSURE, "{pct} % degradation: peak {peak} Pa above design pressure");
        prev_peak = peak;
        prev_t150 = t150_s;
    }
    results("ic-degradation", "degradation_pct,peak_pressure_pa,time_to_150c_s", &rows);
}
```

- [ ] **Step 2: Run and write results**

Run: `BWR_WRITE_RESULTS=1 cargo test --release -p bwr-core --test ic_transients noncondensable -- --nocapture`
Expected: PASS; `docs/validation/results/ic-degradation.csv` created with three rows. If the 50 % case exceeds design pressure, that is a finding: leave the test failing, report it in the commit message and under "Results tables" in `isolation-condensers.md`, and stop this task for Jim's decision (the spec forbids inventing performance).

- [ ] **Step 3: Commit**

```bash
git add crates/bwr-core/tests/ic_transients.rs docs/validation/results/ic-degradation.csv
git commit -m "test(ic): noncondensable degradation sensitivity 0/25/50 %

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 6: Loss of normal heat rejection ordering

**Files:**
- Modify: `crates/bwr-core/tests/ic_transients.rs`
- Modify: `crates/bwr-core/src/sim.rs` (`cw_pumps_all_off`)

**Interfaces:**
- Consumes: `Fault::CwPumpTrip` honoured by P6, turbine trip on low vacuum, bypass block on condenser pressure, high-pressure scram, IC auto-initiation; `Sim.log` entries from P2–P6 (turbine trip, scram, `ic auto-initiation`).

- [ ] **Step 1: Write the test and helper**

Append to the test file:

```rust
#[test]
fn loss_of_circulating_water_ends_with_ic_cooling_in_the_right_order() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    for i in 0..N_CW_PUMPS {
        sim.set_fault(Fault::CwPumpTrip { index: i }, true);
    }
    let mut peak: f64 = 0.0;
    while sim.time < 1800.0 {
        sim.step(DT);
        peak = peak.max(sim.vessel.eos.p);
    }
    let find = |needle: &str| sim.log.iter().find(|(_, t)| t.to_lowercase().contains(needle)).map(|(t, _)| *t);
    let t_turbine = find("turbine trip").expect("turbine trip logged");
    let t_scram = find("scram").expect("scram logged");
    let t_ic = find("ic auto-initiation").expect("IC initiation logged");
    assert!(t_turbine <= t_scram, "turbine trip ({t_turbine}) before scram ({t_scram})");
    assert!(t_scram <= t_ic, "scram ({t_scram}) before IC initiation ({t_ic})");
    assert!(!sim.cond.available_for_bypass(), "bypass must be blocked without circulating water");
    assert!(peak < VESSEL_DESIGN_PRESSURE, "peak {peak} Pa");
    assert!(sim.ic.iter().any(|t| t.in_service()));
    assert!(sim.vessel.eos.p < IC_HIGH_PRESSURE, "ICs should have brought pressure down within 30 min");
    assert!(sim.cw_pumps_all_off(), "automatic logic must not restart faulted pumps");
}
```

Add to `impl Sim` in `sim.rs`:

```rust
    /// Test helper: no circulating water pump is running.
    pub fn cw_pumps_all_off(&self) -> bool {
        self.cond.cw_pumps.iter().all(|&b| !b)
    }
```

- [ ] **Step 2: Run**

Run: `cargo test --release -p bwr-core --test ic_transients loss_of_circulating`
Expected: PASS. If the scram is logged before the turbine trip, inspect `sim.log` with `--nocapture`; the first signal after vacuum loss must be the condenser-pressure turbine trip (`CONDENSER_TRIP_P` = 25 kPa) and the scram should follow from pressure. Record the actual order in the commit message.

- [ ] **Step 3: Commit**

```bash
git add crates/bwr-core/tests/ic_transients.rs crates/bwr-core/src/sim.rs
git commit -m "test(ic): loss of circulating water ordering ends in IC cooling

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 7: Train availability matrix, capacity sensitivity, inadvertent initiation, hot and low pools

**Files:**
- Modify: `crates/bwr-core/tests/ic_transients.rs`

**Interfaces:**
- Produces: `docs/validation/results/ic-availability.csv` and `ic-inadvertent.csv`.

- [ ] **Step 1: Write the tests**

Append:

```rust
struct IcCase {
    name: &'static str,
    trains: usize,
    pool_t0: f64,
    pool_frac0: f64,
    /// UA multiplier applied through the degradation fault (1.0 - pct/100); 1.0 = nominal.
    ua_pct_loss: u8,
}

fn isolation_with(case: &IcCase) -> (f64, f64, f64, f64) {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    for i in case.trains..N_IC_TRAINS {
        sim.set_fault(Fault::IcTrainUnavailable { train: i }, true);
    }
    if case.ua_pct_loss > 0 {
        for train in 0..N_IC_TRAINS {
            sim.set_fault(Fault::IcNoncondensableDegradation { train, pct: case.ua_pct_loss }, true);
        }
    }
    for t in sim.ic.iter_mut() {
        t.pool_t = case.pool_t0;
        t.pool_mass = case.pool_frac0 * IC_POOL_MASS;
    }
    isolate(&mut sim);
    let (mut peak, mut t_peak) = (0.0f64, 0.0);
    while sim.time < 605.0 {
        sim.step(DT);
        if sim.vessel.eos.p > peak {
            peak = sim.vessel.eos.p;
            t_peak = sim.time;
        }
    }
    (peak, t_peak, sim.vessel.eos.p, sim.vessel.eos.t)
}

#[test]
fn train_availability_matrix() {
    let nominal = |name, trains| IcCase { name, trains, pool_t0: IC_POOL_T_AMBIENT, pool_frac0: 1.0, ua_pct_loss: 0 };
    let cases = [
        nominal("three_trains", 3),
        nominal("two_trains", 2),
        nominal("one_train", 1),
        IcCase { name: "three_trains_hot_pool", trains: 3, pool_t0: POOL_BOIL_T, pool_frac0: 1.0, ua_pct_loss: 0 },
        IcCase { name: "three_trains_low_pool", trains: 3, pool_t0: IC_POOL_T_AMBIENT, pool_frac0: 0.3, ua_pct_loss: 0 },
        // Capacity sensitivity for the ESBWR-derived duty (E-08): -30 % via the UA fault.
        IcCase { name: "three_trains_minus_30pct_duty", trains: 3, pool_t0: IC_POOL_T_AMBIENT, pool_frac0: 1.0, ua_pct_loss: 30 },
    ];
    let mut rows = Vec::new();
    for c in &cases {
        let (peak, t_peak, p600, t600) = isolation_with(c);
        let above = peak > VESSEL_DESIGN_PRESSURE;
        rows.push(format!(
            "{},{},{},{},{},{:.0},{:.1},{},{:.0},{:.1}",
            c.name, c.trains, c.pool_t0, c.pool_frac0, c.ua_pct_loss, peak, t_peak, above, p600, t600
        ));
        if c.trains >= 2 && c.ua_pct_loss == 0 {
            assert!(!above, "{}: peak {peak} Pa above design pressure", c.name);
            assert!(p600 < IC_HIGH_PRESSURE, "{}: pressure not falling after 600 s ({p600} Pa)", c.name);
        }
        // One train and -30 % duty: recorded only. The spec forbids inventing a performance target.
    }
    results(
        "ic-availability",
        "case,trains,pool_t0_c,pool_frac0,ua_pct_loss,peak_pressure_pa,peak_time_s,above_design,p_600s_pa,t_600s_c",
        &rows,
    );
}

#[test]
fn inadvertent_initiation_at_full_power_does_not_scram() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    let n0 = sim.core.kin.n;
    let p0 = sim.vessel.eos.p;
    sim.apply(Command::SetIcTrain { index: 0, open: true });
    let (mut n_peak, mut p_min) = (n0, p0);
    while sim.time < 605.0 {
        sim.step(DT);
        n_peak = n_peak.max(sim.core.kin.n);
        p_min = p_min.min(sim.vessel.eos.p);
    }
    assert!(!sim.scrammed, "scram: {:?}", sim.scram_reason);
    assert!(sim.ic[0].in_service() && sim.ic[0].heat > 1.0e6);
    assert!(n_peak < APRM_HIGH_RUN, "power peak {n_peak}");
    results(
        "ic-inadvertent",
        "n0,n_peak,p0_pa,p_min_pa,q_ic_w",
        &[format!("{n0:.4},{n_peak:.4},{p0:.0},{p_min:.0},{:.0}", sim.ic[0].heat)],
    );
}
```

- [ ] **Step 2: Run and write results**

Run: `BWR_WRITE_RESULTS=1 cargo test --release -p bwr-core --test ic_transients train_availability -- --nocapture`
Then: `BWR_WRITE_RESULTS=1 cargo test --release -p bwr-core --test ic_transients inadvertent -- --nocapture`
Expected: both PASS; two CSV files written. If `two_trains` exceeds design pressure, that is a finding against `IC_TRAIN_RATED_W` or the pool split (A-023): leave the test failing, add the row to `isolation-condensers.md` "Results tables", and stop for Jim's decision.

- [ ] **Step 3: Commit**

```bash
git add crates/bwr-core/tests/ic_transients.rs docs/validation/results/ic-availability.csv docs/validation/results/ic-inadvertent.csv
git commit -m "test(ic): availability matrix, capacity sensitivity, inadvertent initiation, hot and low pools

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 8: 72-hour and 7-day endurance cases with coarse-step convergence

**Files:**
- Modify: `crates/bwr-core/src/validation.rs` (`DT_LONG`, `run_case` honours `dt_after_s`, `standard_cases`)
- Modify: `crates/bwr-core/examples/validation.rs` (`--long`)
- Modify: `docs/validation/cases.csv`
- Modify: `crates/bwr-core/tests/ic_transients.rs`
- Modify: `.github/workflows/validate.yml`

**Interfaces:**
- Consumes: `Case.dt_after_s` (master field; P2 may have left it unused by `run_case`), `Operator::None`, `Trace::column`, `Tolerance::physical`, `compare`.
- Produces: `validation::DT_LONG = 0.5`; cases `fp_scram_ic_72h`, `fp_scram_ic_7d`; `docs/validation/results/ic-endurance.csv`.

- [ ] **Step 1: Write the failing tests**

Append to `ic_transients.rs`:

```rust
use bwr_core::validation::{compare, run_case, standard_cases, Case, Tolerance, DT_LONG};

fn case(id: &str) -> Case {
    standard_cases().into_iter().find(|c| c.id == id).unwrap_or_else(|| panic!("case {id} missing"))
}

fn pool_total(tr: &bwr_core::validation::Trace, row: usize) -> f64 {
    (0..3).map(|i| tr.column(&format!("ic_pool_kg_{i}"))[row]).sum()
}

#[test]
fn coarse_step_matches_fine_step_over_the_first_hour() {
    let mut c = case("fp_scram_ic_72h");
    c.duration_s = 3600.0;
    let mut fine = c.clone();
    fine.dt_after_s = None;
    let a = run_case(&fine, DT).metrics();
    let b = run_case(&c, DT).metrics(); // DT for 600 s, then DT_LONG
    let violations = compare(&a, &b, &Tolerance::physical());
    assert!(violations.is_empty(), "coarse step diverges: {violations:?}");
}

#[test]
fn seventy_two_hour_cooling_with_three_trains() {
    let tr = run_case(&case("fp_scram_ic_72h"), DT);
    let last = tr.rows.len() - 1;
    let pool_end = pool_total(&tr, last);
    let temp_end = tr.column("t_coolant_c")[last];
    let p_peak = tr.column("pressure_pa_max_int").iter().cloned().fold(0.0, f64::max);
    assert!(pool_end > 0.0, "pool depleted before 72 h");
    assert!(p_peak < VESSEL_DESIGN_PRESSURE);
    assert!(temp_end >= POOL_BOIL_T - 0.5, "ICs cannot cool below the boiling pool: {temp_end} degC");
    assert!(!tr.events.iter().any(|(_, e)| e.starts_with("DOMAIN")), "{:?}", tr.events);
    assert!(tr.column("unsupported").iter().all(|&u| u == 0.0));
    results(
        "ic-endurance",
        "case,hours,pool_fraction,t_coolant_c,peak_pressure_pa",
        &[format!("fp_scram_ic_72h,72,{:.3},{:.1},{:.0}", pool_end / IC_POOL_MASS_TOTAL, temp_end, p_peak)],
    );
}

#[test]
#[ignore = "7-day run: executed by validate.yml with --ignored"]
fn seven_day_cooling_with_three_trains() {
    let tr = run_case(&case("fp_scram_ic_7d"), DT);
    let t = tr.column("t");
    let temp = tr.column("t_coolant_c");
    let p_peak = tr.column("pressure_pa_max_int").iter().cloned().fold(0.0, f64::max);
    let i72 = t.iter().position(|&x| x >= 72.0 * 3600.0).unwrap();
    let last = t.len() - 1;
    let (pool72, t72) = (pool_total(&tr, i72), temp[i72]);
    let (pool7d, t7d) = (pool_total(&tr, last), temp[last]);
    assert!(pool72 > 0.0);
    assert!(t7d >= POOL_BOIL_T - 0.5, "{t7d}");
    results(
        "ic-endurance",
        "case,hours,pool_fraction,t_coolant_c,peak_pressure_pa",
        &[
            format!("fp_scram_ic_7d,72,{:.3},{:.1},{:.0}", pool72 / IC_POOL_MASS_TOTAL, t72, p_peak),
            format!("fp_scram_ic_7d,168,{:.3},{:.1},{:.0}", pool7d / IC_POOL_MASS_TOTAL, t7d, p_peak),
        ],
    );
    // Reported, not asserted: whether the derived pool lasts 7 days with the assumed per-train split.
    eprintln!("7 d pool fraction {:.3}", pool7d / IC_POOL_MASS_TOTAL);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --release -p bwr-core --test ic_transients coarse_step`
Expected: FAIL: `DT_LONG` not found and/or `case fp_scram_ic_72h missing`.

- [ ] **Step 3: Extend `validation.rs` and the runner**

In `crates/bwr-core/src/validation.rs`:

```rust
/// Coarse step for long unattended-cooling cases, s. Convergence against `sim::DT` over the
/// first hour is checked by ic_transients::coarse_step_matches_fine_step_over_the_first_hour.
pub const DT_LONG: f64 = 0.5;
```

In `run_case`, if P2 left `dt_after_s` unused, replace the single `sim.step(dt)` and the fixed `sample_every` with a time-based sampler so the coarse phase samples on the same multiples:

```rust
    let mut next_sample = 0.0;
    loop {
        // commands/faults due before this step (unchanged from P2)
        if sim.time + 1e-9 >= next_sample {
            trace.rows.push(sample(&sim));
            next_sample += case.sample_dt_s;
        }
        if sim.time + 1e-9 >= case.duration_s {
            break;
        }
        let h = match case.dt_after_s {
            Some((from_t, big)) if sim.time + 1e-9 >= from_t => big,
            _ => dt,
        };
        sim.step(h);
        trace.events.extend(sim.log.drain(..));
    }
```

Keep P2's command/fault application (`due = sim.time + 0.5 * h`). Interval-extrema columns (`*_max_int`, `*_min_int`) are accumulated per physics step by P2's sampler and are unaffected by `h`.

Add to `standard_cases()`:

```rust
        Case {
            id: "fp_scram_ic_72h",
            scenario: ScenarioName::FullPower,
            duration_s: 72.0 * 3600.0,
            sample_dt_s: 60.0,
            commands: vec![
                ScriptedCommand { at_s: 5.0, cmd: Command::ManualScram },
                ScriptedCommand { at_s: 5.0, cmd: Command::SetMsiv { line: 0, open: false } },
                ScriptedCommand { at_s: 5.0, cmd: Command::SetMsiv { line: 1, open: false } },
            ],
            faults: vec![],
            dt_after_s: Some((600.0, DT_LONG)),
            operator: Operator::None,
        },
        Case {
            id: "fp_scram_ic_7d",
            scenario: ScenarioName::FullPower,
            duration_s: 7.0 * 86400.0,
            sample_dt_s: 600.0,
            commands: vec![
                ScriptedCommand { at_s: 5.0, cmd: Command::ManualScram },
                ScriptedCommand { at_s: 5.0, cmd: Command::SetMsiv { line: 0, open: false } },
                ScriptedCommand { at_s: 5.0, cmd: Command::SetMsiv { line: 1, open: false } },
            ],
            faults: vec![],
            dt_after_s: Some((600.0, DT_LONG)),
            operator: Operator::None,
        },
```

In `crates/bwr-core/examples/validation.rs`, after `let write = ...`:

```rust
    let long = args.iter().any(|a| a == "--long");
```

and filter the case loop: `for case in standard_cases().into_iter().filter(|c| long || c.duration_s <= 86_400.0)`. Document the flag in the example's header comment: `//! --long   include cases longer than 24 h (fp_scram_ic_72h, fp_scram_ic_7d)`.

- [ ] **Step 4: Register the cases**

Append to `docs/validation/cases.csv` (master header `id,scenario,duration_s,purpose,reference,tolerance_class,role,status`; if the file still has P2's 7-column header without `role`, add the `role` column to the header and `plausibility` to every existing row in this commit, since the master locks 8 columns):

```
fp_scram_ic_72h,fullPower,259200,72 h unattended cooling with 3 IC trains; no makeup; no pool cooling; 2 y at rated before scram,S-10 passive cooling claim (conditions differ; simulator result only),physical,plausibility,baseline
fp_scram_ic_7d,fullPower,604800,7 d unattended cooling under the same conditions; pool fraction reported,S-10 7-day claim (not reproduced; reported),physical,plausibility,long-run-only
```

- [ ] **Step 5: CI**

In `.github/workflows/validate.yml`, in the `engine` job after the "Validation cases against baselines" step:

```yaml
      - name: Long-duration IC endurance (7 d, ignored test)
        run: cargo test --release -p bwr-core --test ic_transients -- --ignored seven_day
```

- [ ] **Step 6: Run**

Run: `cargo test --release -p bwr-core --test ic_transients`
Expected: PASS. The 72 h case runs about 520 000 coarse steps; if it takes longer than 120 s in release, profile (`cargo build --release --example trace`, run `fp_scram_ic_72h` under `time`) and report; do not raise `DT_LONG`.

Run: `BWR_WRITE_RESULTS=1 cargo test --release -p bwr-core --test ic_transients seventy_two`
Then: `BWR_WRITE_RESULTS=1 cargo test --release -p bwr-core --test ic_transients -- --ignored seven_day --nocapture`
Expected: both PASS. Both write `ic-endurance.csv`; after the second run, merge the 72 h row from the first run back in so the file has three rows and the header once.

Run: `cargo run --release -p bwr-core --example validation -- --long --write --case fp_scram_ic_72h`
Expected: baseline `docs/validation/baseline/fp_scram_ic_72h.csv` written (the 7 d case gets no baseline; it is a long-run-only report). This is the FIRST baseline of a new case, so there is nothing to compare against; it is listed as `new case` in the Task 10 report and is not a second `--write` of an existing baseline.

- [ ] **Step 7: Commit**

```bash
git add crates/bwr-core/src/validation.rs crates/bwr-core/examples/validation.rs crates/bwr-core/tests/ic_transients.rs docs/validation/cases.csv docs/validation/results/ic-endurance.csv docs/validation/baseline/fp_scram_ic_72h.csv .github/workflows/validate.yml
git commit -m "feat(validation): 72 h and 7 d IC endurance cases with coarse-step convergence check

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 9: Snapshot, TypeScript contract, mock and mimic

**Files:**
- Modify: `crates/bwr-core/src/api.rs` (`IcTrain`)
- Modify: `crates/bwr-core/src/sim.rs` (`snapshot()` IC mapping, `ic` alarm)
- Modify: `crates/bwr-core/tests/plant.rs` (`json_contract_matches_ui`)
- Modify: `ui/src/sim/types.ts`, `ui/src/sim/mock.ts`, `ui/src/components/mimic/PlantMimic.tsx`

**Interfaces:**
- Produces: `Snapshot.ic[i]` fields `steamValvePos`, `returnValvePos: number[]`, `condensateReturnKgs`, `available`; `valvePos` kept as `max(returnValvePos)`.

- [ ] **Step 1: Extend the contract test**

In `crates/bwr-core/tests/plant.rs` `json_contract_matches_ui`, after the existing key checks add:

```rust
    let ic0 = &v["ic"][0];
    for key in ["valvePos", "steamValvePos", "returnValvePos", "condensateReturnKgs", "available", "commandOpen", "heatMw", "poolTempC", "poolLevelPct"] {
        assert!(ic0.get(key).is_some(), "ic missing {key}");
    }
    assert_eq!(ic0["returnValvePos"].as_array().unwrap().len(), N_IC_RETURN_VALVES);
```

Run: `cargo test --release -p bwr-core --test plant json_contract`
Expected: FAIL `ic missing steamValvePos`.

- [ ] **Step 2: Rust side**

In `api.rs`:

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IcTrain {
    /// max(return_valve_pos): kept for the mimic.
    pub valve_pos: f64,
    pub steam_valve_pos: f64,
    pub return_valve_pos: Vec<f64>,
    pub command_open: bool,
    pub available: bool,
    pub heat_mw: f64,
    pub condensate_return_kgs: f64,
    pub pool_temp_c: f64,
    pub pool_level_pct: f64,
}
```

In `sim.rs` `snapshot()`:

```rust
            ic: self
                .ic
                .iter()
                .map(|t| api::IcTrain {
                    valve_pos: t.return_valve_pos.iter().cloned().fold(0.0, f64::max),
                    steam_valve_pos: t.steam_valve_pos,
                    return_valve_pos: t.return_valve_pos.to_vec(),
                    command_open: t.cmd_open,
                    available: t.available,
                    heat_mw: t.heat / 1e6,
                    condensate_return_kgs: t.w_condensate_return,
                    pool_temp_c: t.pool_t,
                    pool_level_pct: t.pool_level_pct(),
                })
                .collect(),
```

Change the `ic` alarm condition to `if self.ic.iter().any(|t| t.in_service())`.

Run: `cargo test --release -p bwr-core --test plant json_contract`
Expected: PASS.

- [ ] **Step 3: TypeScript side**

`ui/src/sim/types.ts`:

```ts
export interface IcTrain {
  /** max(returnValvePos) — kept for the mimic. */
  valvePos: number;
  /** Steam supply valve 0..1, normally open; closed only by an engine-level test fault. */
  steamValvePos: number;
  /** Two parallel condensate return valves 0..1; opening either puts the train in service. */
  returnValvePos: number[];
  commandOpen: boolean;
  /** False when the train is faulted unavailable (engine-level fault). */
  available: boolean;
  heatMw: number;
  condensateReturnKgs: number;
  poolTempC: number;
  poolLevelPct: number;
}
```

`ui/src/sim/mock.ts` blank snapshot:

```ts
    ic: Array.from({ length: 3 }, () => ({
      valvePos: 0,
      steamValvePos: 1,
      returnValvePos: [0, 0],
      commandOpen: false,
      available: true,
      heatMw: 0,
      condensateReturnKgs: 0,
      poolTempC: 30,
      poolLevelPct: 100,
    })),
```

and in the mock step loop replace the valve line:

```ts
    for (const ic of s.ic) {
      ic.returnValvePos = ic.returnValvePos.map((p) => approach(p, ic.commandOpen ? 1 : 0, 1 / 20, dt));
      ic.valvePos = Math.max(...ic.returnValvePos);
      ic.heatMw = ic.valvePos * ic.steamValvePos * IC_MW * clamp(v.pressureMpa / 7.2, 0, 1) * (v.coolantTempC > 110 ? 1 : 0);
      ic.condensateReturnKgs = (ic.heatMw * 1e6) / 1.5e6;
```

`ui/src/components/mimic/PlantMimic.tsx`, IC valve and tag:

```tsx
              <Valve
                x={160}
                y={yRet}
                pos={ic.valvePos}
                size={6}
                onClick={() => send({ type: "setIcTrain", index: i, open: !ic.commandOpen })}
                title={`IC train ${String.fromCharCode(65 + i)} return valves (${ic.returnValvePos.map((p) => Math.round(p * 100)).join("/")} %)${ic.available ? "" : " — UNAVAILABLE"} — click to ${ic.commandOpen ? "close" : "open"}`}
              />
              <Tag
                x={x}
                y={200}
                text={`${String.fromCharCode(65 + i)} ${ic.available ? fx(ic.heatMw, 0) : "N/A"}`}
                tone={ic.available && ic.heatMw > 0.5 ? undefined : "dim"}
              />
```

- [ ] **Step 4: Typecheck and build**

Run: `npm --prefix ui run typecheck && npm --prefix ui run build`
Expected: both succeed with no errors.

- [ ] **Step 5: Commit**

```bash
git add crates/bwr-core/src/api.rs crates/bwr-core/src/sim.rs crates/bwr-core/tests/plant.rs ui/src/sim/types.ts ui/src/sim/mock.ts ui/src/components/mimic/PlantMimic.tsx
git commit -m "feat(api): expose IC steam/return valves, availability and condensate return

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 10: Regenerate baselines, record before/after, update coverage

**Files:**
- Modify: `docs/validation/baseline/*.csv`, `docs/model/coverage.md`, `docs/model/isolation-condensers.md`, `README.md`

**Interfaces:**
- Consumes: `examples/validation.rs` compare mode (the default; no flag) and `--write` (P2).

This task follows the master baseline regeneration protocol: (1) compare and save the report, (2) classify every change, (3) pass the package gates, (4) `--write` exactly once, (5) commit report and baselines together. No tolerance is adjusted at any point.

- [ ] **Step 1: Compare against the pre-package baselines and record the differences**

Run: `set -o pipefail; cargo run --release -p bwr-core --example validation 2>&1 | tee /tmp/ic-before-after.txt`
Expected: exit 1 (the runner reports violations) only in cases where ICs act (`fp_msiv_closure` and any case whose events include `ic auto-initiation`). Create `docs/validation/reports/2026-10-21-pkg07.md` with the runner output verbatim under `## Compare run (before --write)` and a `## Classification` table with one row per changed metric and a class of `physical` (reason: tube drain replaces the heat lag; condensate return adds mass; derived `q_ic`), `schema-population` (the `ic_heat_w_i`, `ic_pool_kg_i`, `ic_return_pos_i` columns are newly populated, so byte-identical traces are NOT expected for any case with an IC in service), or `numerical`. For each violated metric, also add a row to a "Baseline changes from Package 7" table at the end of `docs/model/isolation-condensers.md`:

```markdown
## Baseline changes from Package 7

| Case | Metric | Before | After | Cause |
|---|---|---|---|---|
| fp_msiv_closure | pressure_pa peak | <before> | <after> | tube drain (about 60 s to full duty) replaces the 20 s heat lag; return water now adds mass |
```

(one row per violation printed, copied verbatim from the runner output). Cases that do not touch the ICs must show no violations; if one does, stop and find out why before regenerating anything.

- [ ] **Step 2: Gates, then regenerate once**

The package's independent physical gates are `cargo test --release -p bwr-core --test ic_transients` and `cargo test --release -p bwr-core --lib ic::tests`; both must be green before writing (they are the Task 2–8 tests, not the baselines).

Run: `cargo run --release -p bwr-core --example validation -- --write` (exactly once for this change set)
Then: `cargo run --release -p bwr-core --example validation`
Expected: exit 0, no violations. `git diff --stat docs/validation/baseline` lists only the cases named in the report; if another case changed, stop and find out why. The second run only shows the runner is deterministic; it is not validation evidence.

- [ ] **Step 3: Coverage and README**

In `docs/model/coverage.md`, set the isolation condensers row to:

```
| Isolation condensers | ic.rs | S-ESBWR-05 (shared concept); S-06; S-09; S-10 | N_IC_TRAINS; N_IC_RETURN_VALVES; IC_*; ic.* | A-020..A-028 | ic::tests (16); ic_transients.rs (15); cases fp_scram_ic_72h, fp_scram_ic_7d | 1-train endurance, -30 % duty and absolute duty are reported, not validated; cooling below 100 degC out of envelope |
```

In `README.md`, change the Isolation condensers row of the status table to:

```
| Isolation condensers | 3 passive trains, each with a steam valve and two parallel return valves; tubes flood in standby and drain into service; the vessel supplies the steam they condense; pools heat, boil off and are conserved; endurance to 7 days reported under stated assumptions (cannot cool below the boiling pool) |
```

and in "Data sources" append `- US NRC, *ESBWR Design Control Document* Tier 2 Rev. 10, chapter 5 (isolation condenser arrangement, used as a shared concept only)`.

- [ ] **Step 4: Full verification**

Run: `cargo test --release -p bwr-core && npm --prefix ui run typecheck && npm --prefix ui run build && cargo run --release -p bwr-core --example validation`
Expected: all green; the validation runner reports no violations.

- [ ] **Step 5: Commit**

```bash
git add docs/validation/baseline docs/validation/reports/2026-10-21-pkg07.md docs/model/coverage.md docs/model/isolation-condensers.md README.md
git commit -m "docs(ic): regenerate validation baselines after IC refinement; record before/after and coverage

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

## Package self-review (master r2 checklist, run before handing off)

- [x] **Spec bullets → tasks:** train arrangement and parallel return valves (T1, T2); unsupported capacity/pool/timing replaced or quantified (T1 derivation, T7 capacity sensitivity); steam supply / condensate / tube inventory / pressure difference / pool coupling (T2 equations, T3 vessel ports); noncondensable sensitivity (T5); auto initiation, manual, reset/latch, available trains, loss of normal heat rejection (T4, T6); 1/2/3 trains, inadvertent, valve failure, hot and low pools, depletion (T4, T7); 72 h and 7 d with stated conditions (T8); lower temperature limit and unimplemented shutdown cooling boundary (T1 doc, T8 assertions).
- [x] **No placeholders:** the only `<...>` cells are the derived pool mass, the source checksums and the before/after table, each produced by a command in the same step.
- [x] **Names match master r2:** seven-field `IcStep`, `IcTrain::request`/`commit` with `IcTrain.last_error` (the master's split, P3's interim signatures kept unchanged), `tube_energy`, `VesselInputs`/`BoundaryFlows` IC ports, derived `q_ic`, `faults: Vec<Fault>` via `find_fault` with `train*N_IC_RETURN_VALVES + valve`, `Sim::domain_error`, `Case.dt_after_s` + `operator`, trace columns `ic_heat_w_i`/`ic_pool_kg_i`/`ic_return_pos_i`/`t_coolant_c` (no column-count change), `parameters.csv` 13 columns with `id`, results under `docs/validation/results/`. Additions beyond the master are in the table at the top; the executor appends them to the master's P7 section in the Task 2 commit.
- [x] **Derives rule:** `IcStep` and `IcRequest` `Clone, Copy, Debug, Default, PartialEq` (P3's derives kept); `IcTrain` `Clone, Debug` + `impl Default`; `IcFaults` `Clone, Copy, Debug, Default`; `api::IcTrain` `Clone, Debug, Serialize, Deserialize` camelCase.
- [x] **Every new constant has a `parameters.csv` row** (Task 1 Step 7; `register.rs` gates it).
- [x] **Every Snapshot change has `types.ts` and `mock.ts` steps** and keeps `npm --prefix ui run typecheck` green (Task 9).
- [x] **Every defect fix has a failing test first:** no-steam lag (T2 `no_steam_limits_heat_to_tube_water_energy_without_lag`), pool overshoot/depletion (T2), allocation (T2 `commit_uses_applied_not_requested_flows`, T3 `ic_commit_uses_applied_flows_when_the_vessel_runs_dry`), energy double count (T2 `vessel_tubes_pool_identity_holds_each_step`, T3 `ic_energy_identity_holds_at_plant_level` asserting `flows.q_ic == balance.q_ic_w + ic_tube_storage_w` every step), trace population (T8 `pool_total` reads the per-train columns), contract (T9).
- [x] **Every consumer of a transfer uses the applied value:** `request` is pure; tubes, pools, `heat`, `w_steam_in`, the trace sample, the snapshot and `balance.q_ic_w` are all produced by `commit` with the vessel-applied flows (T3 Step 4; asserted per step in `ic_energy_identity_holds_at_plant_level` and in the exhausted step by `ic_commit_uses_applied_flows_when_the_vessel_runs_dry`). No assumption row excuses a requested-value consumer.
