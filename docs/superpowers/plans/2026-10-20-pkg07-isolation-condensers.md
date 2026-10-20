# Package 7: Isolation Condensers and Shutdown Heat Removal — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Names and signatures come from `docs/superpowers/plans/2026-10-20-refinement-master.md` ("Shared interface contract", section P7); if this file and the master disagree, the master wins.

**Goal:** Replace the single-valve, energy-only isolation condenser (IC) model with a per-train model that moves steam and condensate through explicit valves, tube inventory and a pool whose energy is conserved, so that heat removal is physically supplied, endurance claims are tied to stated conditions, and the lower temperature limit of IC cooling is a declared boundary rather than an accident of vessel heat loss.

**Architecture:** `ic.rs` becomes a mass-and-energy component: steam in through a normally-open steam valve, condensate held in tubes until one of two parallel normally-closed return valves opens, condensate back to the vessel at saturated-liquid enthalpy. The vessel sees two mass flows instead of a heat sink. `Sim` maps `Fault`s onto per-train availability, valve freezes and UA degradation every step, raises `DomainError::IcModelEnvelope` when a pool is exhausted while still loaded, and logs initiation events for ordering tests. Long-duration validation cases use a coarser time step after the first ten minutes with a convergence check.

**Tech Stack:** Rust 2021, serde, existing `bwr-core` modules from Packages 2–6 (`ledger`, `faults`, `validation`, `domain`, `vessel::Regime`), TypeScript mirror in `ui/src/sim/types.ts`, `mock.ts`, `PlantMimic.tsx`.

**Spec:** Package 7 of `docs/superpowers/plans/2026-10-20-existing-systems-refinement.md`.

## Global Constraints

Everything in the master plan's "Global Constraints" section applies. Package-specific restatements from the spec:

- "Do not scale dimensions, inventory, flow, decay-heat removal capacity, or timing by electrical output alone."
- "Include a justified sensitivity treatment for noncondensable gas degradation. Do not invent exact design performance or purge behavior."
- "Run unattended cooling tests to 72 hours and seven days where these durations match the selected source claim. Specify initial operating history, credited trains, makeup, and external services."
- "If complete cold-shutdown cooling requires an unimplemented system, document that boundary instead of attributing its function to ICS or arbitrary vessel heat loss."
- "GE's public BWRX general description identifies integral RPV isolation valves, elimination of conventional safety relief valves, and dry containment as distinguishing features." and "The NRC/CNSC containment evaluation report also notes that IC function and reactor connections differ despite design heritage." — ESBWR IC material is a shared-concept source only.
- No new crate or npm dependencies.
- Every new constant gets a `docs/model/parameters.csv` row.

**Acceptance gate (verbatim from the spec):** "Heat removal is physically supplied and conserved; endurance claims identify conditions and evidence. Shutdown scenarios remain valid through their declared endpoint."

## Assumed state at start of this package

- P2: `Sim.ledger: Ledger`, `Vessel::step(..) -> BoundaryFlows`, `Sim.log: Vec<(f64, String)>`, `Sim.faults`, `Sim::set_fault`, `validation::{Case, run_case, standard_cases, Trace, Tolerance}`, `examples/validation.rs` with `--write-baseline` and compare modes, `.github/workflows/validate.yml`.
- P3: `domain::DomainError` with `IcModelEnvelope { train, reason }` variant declared; `Sim.domain_errors: Vec<(f64, DomainError)>`; `IcTrain::step` currently returns `IcStep { q_removed, w_condensate_return, energy_to_pool, pool_boiloff_kg }` and takes `(dt, t_sat, steam_available, h_fg)`; pool overshoot bug fixed.
- P4: `vessel::Regime`, `Eos.regime`, `Vessel::sat()`.
- P6: `Fault::CwPumpTrip`, `Fault::FeedPumpTrip`, `Fault::BypassUnavailable` honoured; `Sim.balance: EnergyBalance` with `q_ic_w`.

Check these with `grep -n "IcModelEnvelope" crates/bwr-core/src/domain.rs` and `grep -n "pub fn step" crates/bwr-core/src/ic.rs` before Task 1; if a name differs, fix it in that package's file first (one-line commit), not here.

## Names introduced by this package (beyond the master contract)

| Name | Where | Meaning |
|---|---|---|
| `IC_TUBE_CAPACITY_KG` | `params.rs` | Condensate that fills one train's tubes and drain line, kg. ASSUMED 5 000. |
| `IC_DRAIN_TAU_S` | `params.rs` | Drain time constant through an open return valve, s. ASSUMED 20. |
| `IC_POOL_MASS_TOTAL` | `params.rs` | Total pool water for all trains, kg. DERIVED from the 7-day claim (Task 1). |
| `IC_POOL_MASS` | `params.rs` | Per-train bay, `IC_POOL_MASS_TOTAL / N_IC_TRAINS as f64`. |
| `IC_POOL_COOLING_W` | `params.rs` | Credited pool cooling system duty, W. ASSUMED 0. |
| `IC_POOL_T_AMBIENT` | `params.rs` | Floor for an idle pool's temperature, degC. ASSUMED 40. |
| `CP_WATER`, `H_FG_ATM` | `ic.rs` | 4186 J/kgK, 2.257e6 J/kg (IAPWS at 100 degC). |
| `IcTrain.steam_valve_cmd_open: bool` | `ic.rs` | Set by `Sim` from `Fault::IcSteamValveClosed`. |
| `IcTrain.return_valve_stuck: [bool; N_IC_RETURN_VALVES]` | `ic.rs` | Set by `Sim` from `Fault::IcReturnValveStuck`. |
| `IcTrain::envelope_exceeded(&self, t_sat: f64) -> bool` | `ic.rs` | Pool empty while in service and steam is hotter than 100 degC. |
| `IcStep.pool_cooling_w: f64` | `ic.rs` | Fifth field: energy leaving the pool to the (uncredited) cooling system this step. |
| `VesselInputs.{w_ic_steam, w_ic_return, h_ic_return}` | `vessel.rs` | Replace `VesselInputs.q_ic`. |
| `BoundaryFlows.{w_ic_steam, w_ic_return}` | `ledger.rs` | Added; `q_ic` kept and now equals `w_ic_steam*h_g - w_ic_return*h_ic_return`. |
| `Case.dt_after_s: Option<(f64, f64)>` | `validation.rs` | `(from_t, dt)`: after `from_t` seconds of sim time use `dt` instead of the run dt. |
| `DT_LONG` | `validation.rs` | 0.5 s, the coarse step for long cases. |
| `docs/validation/results/*.csv` | repo | Tables written by tests when `BWR_WRITE_RESULTS=1`. |

---

### Task 1: Subsystem specification, evidence rows, and the derived pool inventory

**Files:**
- Create: `docs/model/isolation-condensers.md`
- Modify: `docs/model/parameters.csv`, `docs/model/assumptions.md`, `docs/model/esbwr-applicability.csv`, `docs/model/sources.csv`
- Modify: `crates/bwr-core/src/params.rs` (IC block)
- Test: `crates/bwr-core/src/ic.rs` (unit test `derived_pool_mass_covers_seven_days_to_100c`)

**Interfaces:**
- Consumes: `reactor::DecayHeat::after_history(n, irr_s, cooled_s)` (P5), `vessel::energy_for(mass, t)`, `Sim::new(ScenarioName::FullPower)`.
- Produces: constants `N_IC_RETURN_VALVES`, `IC_TUBE_CAPACITY_KG`, `IC_DRAIN_TAU_S`, `IC_POOL_MASS_TOTAL`, `IC_POOL_MASS`, `IC_POOL_COOLING_W`, `IC_POOL_T_AMBIENT` used by every later task.

- [ ] **Step 1: Record the sources this package relies on**

Run and paste the checksums into `docs/model/sources.csv` using the column order Package 1 created:

```bash
cd /Users/jim/ai/bwrsim && mkdir -p /tmp/bwr-src && cd /tmp/bwr-src
curl -sSLo esbwr-dcd-r10-ch5.pdf "https://www.nrc.gov/docs/ML1410/ML14104A929.pdf"
curl -sSLo bwrx-general-description.pdf "https://www.gevernova.com/content/dam/gevernova-nuclear/global/en_us/documents/carbon-free-power/005N9751_Rev_BWRX-300_General_Description.pdf"
curl -sSLo nedo-33934-r2.pdf "https://www.nrc.gov/docs/ML2607/ML26077A383.pdf"
curl -sSLo nrc-cnsc-containment.pdf "https://www.nrc.gov/docs/ML2209/ML22091A201.pdf"
shasum -a 256 *.pdf
```

If the DCD chapter URL has moved, find "ESBWR DCD Revision 10 Tier 2 Chapter 5" from the NRC ESBWR index linked in the spec and record the URL you actually used. Rows to add (fill `retrieved` with today's date and `sha256` from the command):

```
SRC-IC-01,ESBWR DCD Tier 2 Rev. 10 Ch. 5 (5.4.6 Isolation Condenser System),10,2014-04,US NRC,<url>,5.4.6,<retrieved>,<sha256>,vendor design document accepted in certification
SRC-IC-02,BWRX-300 General Description 005N9751,Rev. per file,<date on cover>,GE Hitachi,<url>,ICS section,<retrieved>,<sha256>,vendor description
SRC-IC-03,NEDO-33934 Safety Strategy Rev. 2,2,<date on cover>,US NRC ADAMS ML26077A383,<url>,ICS and decay heat removal,<retrieved>,<sha256>,vendor topical report
SRC-IC-04,NRC/CNSC joint report on BWRX-300 containment evaluation method,-,2022-03,US NRC ADAMS ML22091A201,<url>,IC function differences,<retrieved>,<sha256>,regulator assessment
```

- [ ] **Step 2: Write the subsystem specification**

Create `docs/model/isolation-condensers.md` with exactly this content (fill the two `<...>` numbers in Step 6):

```markdown
# Isolation Condenser System (ICS) — model specification

Package 7 of the refinement program. Status words follow `docs/model/parameters.csv`.

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
| Trains | `N_IC_TRAINS` = 3 | assumed (to verify) | SRC-IC-02 / SRC-IC-03 state the number of trains; executor confirms and updates status to `confirmed` with the page reference, or leaves `assumed` with A-IC-01 |
| Steam supply valve per train | 1, normally open | shared concept | SRC-IC-01 5.4.6; A-IC-02 |
| Condensate return valves per train | `N_IC_RETURN_VALVES` = 2 in parallel, normally closed | shared concept | SRC-IC-01 5.4.6 describes parallel return valves; SRC-IC-04 notes BWRX connections differ; A-IC-02 |
| Rated duty per train | `IC_TRAIN_RATED_W` = 33.75 MW at 7.2 MPa with a boiling pool | esbwr-estimate | ESBWR IC unit duty. **Not a BWRX value.** Sensitivity ±30 % (Task 7 results) |
| Pool water, all trains | `IC_POOL_MASS_TOTAL` = <value> kg | derived | 1.10 x (7-day decay energy after 2 y at rated + vessel stored energy 287.7 -> 100 degC) / h_fg(100 degC). A-IC-03 |
| Pool water per train | `IC_POOL_MASS` = total / 3 | assumed split | separate bays assumed; a shared pool would change the 1- and 2-train results. A-IC-04 |
| Tube/drain condensate capacity | `IC_TUBE_CAPACITY_KG` = 5 000 kg | assumed | A-IC-05 |
| Drain time constant | `IC_DRAIN_TAU_S` = 20 s | assumed | A-IC-06 |
| Valve stroke | `IC_VALVE_STROKE` = 15 s | assumed | A-IC-07 |
| Idle pool floor | `IC_POOL_T_AMBIENT` = 40 degC | assumed | A-IC-08 |

## Equations (per train, each step)

- Valves move linearly at `1/IC_VALVE_STROKE` per second toward their commands; a
  stuck valve or an unavailable train does not move.
- Condensing surface fraction `f_surf = 1 - tube_mass / IC_TUBE_CAPACITY_KG`.
  With return valves closed the tubes flood and condensation stops; this is the
  mechanism that keeps a standby train idle.
- Pool coverage `cov = clamp(pool_level_pct / 40, 0, 1)`.
- Duty `q = UA * ua_factor * max(T_sat - T_pool, 0) * steam_valve_pos * f_surf * cov`,
  bounded by what the pool can absorb this step. `UA = IC_TRAIN_RATED_W / (T_sat(7.2 MPa) - 100)`.
- Steam in `w_steam_in = q / h_fg(p_vessel)`; condensate drains at
  `w_return = sqrt(max(return_valve_pos)) * tube_mass / IC_DRAIN_TAU_S`.
- Vessel: mass `-w_steam_in + w_return`; energy `-w_steam_in*h_g + w_return*h_f`
  (condensate subcooling neglected, A-IC-09). Vessel + tubes + pool close exactly.
- Pool: sensible heating to 100 degC, then boil-off at `H_FG_ATM`; no overshoot.
  Energy leaving to the cooling system is `IC_POOL_COOLING_W` (0).
- Noncondensable degradation: `ua_factor = 1 - pct/100` from a test-level fault.
  No purge or accumulation dynamics (A-IC-10).

## Initiation and control (ASSUMED, see protection.csv when Package 8 lands)

Automatic open of all available trains' return valves on `IC_HIGH_PRESSURE`, L2,
or MSIV isolation at power. Latched: valves stay commanded open until the operator
closes them. Manual `SetIcTrain` is honoured in both directions; a command to an
unavailable train is rejected with an operator message.

## Lower temperature limit (declared boundary)

An IC can only reject heat while `T_sat(p_vessel) > T_pool`. With a boiling pool
the vessel cannot be cooled below about 100 degC by the ICs; afterwards the pool
cools only by the uncredited cooling system. Therefore:

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
result under these assumptions, not a reproduction of the vendor claim.

## Results tables

Written by `tests/ic_transients.rs` with `BWR_WRITE_RESULTS=1` into
`docs/validation/results/ic-availability.csv`, `ic-degradation.csv`, `ic-endurance.csv`.
```

- [ ] **Step 3: Add assumption entries**

Append to `docs/model/assumptions.md`:

```markdown
- A-IC-01 Three IC trains. Public GE descriptions state three; executor records the page or leaves assumed.
- A-IC-02 One normally-open steam supply valve and two parallel normally-closed return valves per train (ESBWR arrangement adopted as a shared concept; BWRX connection differences noted in SRC-IC-04 do not change the valve function modelled).
- A-IC-03 Pool inventory derived from 7-day decay energy plus vessel stored energy to 100 degC with 10 % margin. Sensible capacity of a cold pool (40 -> 100 degC, about +11 %) is not credited.
- A-IC-04 Pool water split equally into separate bays, one per train. A common pool would make 1- and 2-train endurance better than reported.
- A-IC-05 Tube and drain condensate capacity 5 000 kg per train.
- A-IC-06 Drain time constant 20 s through an open return valve; either parallel valve passes full drain flow.
- A-IC-07 Valve stroke 15 s.
- A-IC-08 Idle pool cools no lower than 40 degC and only via the uncredited cooling term.
- A-IC-09 Condensate returns at saturated-liquid enthalpy at vessel pressure (subcooling neglected: conservative for vessel energy, slightly optimistic for return water reactivity).
- A-IC-10 Noncondensable degradation is a fixed UA multiplier set by a test fault; no accumulation or purge dynamics.
```

- [ ] **Step 4: Add applicability rows**

Append to `docs/model/esbwr-applicability.csv` (Package 1 column order: esbwr_source, target_subsystem, bwrx_evidence, differences, adaptation, limits, test, classification):

```
"DCD Tier 2 5.4.6 ICS arrangement",isolation condensers,"SRC-IC-02 ICS description","BWRX integral RPV isolation valves; IC connections differ (SRC-IC-04)","valve function and count adopted; no containment heat-removal role","no containment or IC-line-break modelling",ic_transients::parallel_return_valve_redundancy,shared concept
"DCD Tier 2 5.4.6 IC unit duty 33.75 MWt",isolation condensers,"none public for BWRX","BWRX thermal power 870 vs 4500 MWt; train count 3 vs 4","used as per-train duty with +/-30 % sensitivity","not a BWRX performance claim",ic_transients::capacity_sensitivity,provisional numerical estimate
"DCD Tier 2 5.4.6 pool boil-off and gravity return",isolation condensers,"SRC-IC-03 7-day passive cooling claim","pool geometry unknown; inventory derived not copied","pool mass derived from energy integral",none,ic::derived_pool_mass_covers_seven_days_to_100c,adapted model
"DCD Tier 2 6.2 containment heat removal by PCCS/IC pools",isolation condensers,"SRC-IC-04","BWRX dry containment; ICs do not serve containment","not used",n/a,n/a,inapplicable
```

- [ ] **Step 5: Write the failing pool-inventory test**

Append to the `#[cfg(test)] mod tests` block in `crates/bwr-core/src/ic.rs` (create the block if absent):

```rust
    #[test]
    fn derived_pool_mass_covers_seven_days_to_100c() {
        use crate::api::ScenarioName;
        use crate::reactor::DecayHeat;
        use crate::vessel::energy_for;
        use crate::sim::Sim;
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

Add the constants to `crates/bwr-core/src/params.rs`, replacing the IC block. Use `0.0` for the total so the test fails and prints the required value:

```rust
// ---------------------------------------------------------------- isolation condensers
/// Number of isolation condenser trains. ASSUMED (to verify). A-IC-01.
pub const N_IC_TRAINS: usize = 3;
/// Parallel condensate return valves per train. Shared concept (ESBWR). A-IC-02.
pub const N_IC_RETURN_VALVES: usize = 2;
/// Heat removal per train at rated pressure with a boiling pool, W.
/// ESBWR-ESTIMATE (ESBWR IC unit 33.75 MWt). Not a BWRX value; sensitivity +/-30 %.
pub const IC_TRAIN_RATED_W: f64 = 33.75e6;
/// Valve stroke time, s. ASSUMED. A-IC-07.
pub const IC_VALVE_STROKE: f64 = 15.0;
/// Condensate held in one train's tubes and drain line when flooded, kg. ASSUMED. A-IC-05.
pub const IC_TUBE_CAPACITY_KG: f64 = 5_000.0;
/// Drain time constant through an open return valve, s. ASSUMED. A-IC-06.
pub const IC_DRAIN_TAU_S: f64 = 20.0;
/// Total IC pool water, all trains, kg. DERIVED by ic::tests::derived_pool_mass_covers_seven_days_to_100c. A-IC-03.
pub const IC_POOL_MASS_TOTAL: f64 = 0.0;
/// Pool water per train (separate bays). ASSUMED split. A-IC-04.
pub const IC_POOL_MASS: f64 = IC_POOL_MASS_TOTAL / N_IC_TRAINS as f64;
/// Credited pool cooling system duty, W. ASSUMED 0 (no system credited).
pub const IC_POOL_COOLING_W: f64 = 0.0;
/// Floor for an idle pool temperature, degC. ASSUMED. A-IC-08.
pub const IC_POOL_T_AMBIENT: f64 = 40.0;
```

Add at the top of `ic.rs` (outside the test module):

```rust
/// Liquid water specific heat used for the pool, J/kgK.
pub const CP_WATER: f64 = 4186.0;
/// Latent heat at atmospheric pressure (IAPWS, 100 degC), J/kg.
pub const H_FG_ATM: f64 = 2.257e6;
```

- [ ] **Step 6: Run the test, read the required mass, set the constant**

Run: `cargo test --release -p bwr-core derived_pool_mass -- --nocapture`
Expected: FAIL with `IC_POOL_MASS_TOTAL = 0 must lie in 1.10..1.15 x <N> kg` and the `eprintln` line above it.

Set `IC_POOL_MASS_TOTAL` to the printed `1.10x` value rounded up to the nearest 1 000 kg. Fill the same number into the `<value>` cell of `isolation-condensers.md`.

Run: `cargo test --release -p bwr-core derived_pool_mass`
Expected: PASS.

- [ ] **Step 7: Add parameter rows**

Append to `docs/model/parameters.csv` (Package 1 column order: name, value, unit, definition, reference_state, source_design, status, source_id, assumption_id, sensitivity):

```
N_IC_TRAINS,3,-,isolation condenser trains,-,BWRX-300,assumed,SRC-IC-02,A-IC-01,2..4
N_IC_RETURN_VALVES,2,-,parallel condensate return valves per train,-,ESBWR,esbwr-estimate,SRC-IC-01,A-IC-02,1..2
IC_TRAIN_RATED_W,33.75e6,W,heat removal per train with boiling pool,7.2 MPa,ESBWR,esbwr-estimate,SRC-IC-01,-,+/-30 %
IC_VALVE_STROKE,15,s,IC valve full stroke,-,none,assumed,-,A-IC-07,5..30
IC_TUBE_CAPACITY_KG,5000,kg,condensate to flood tubes and drain line,-,none,assumed,-,A-IC-05,2000..10000
IC_DRAIN_TAU_S,20,s,drain time constant,-,none,assumed,-,A-IC-06,10..60
IC_POOL_MASS_TOTAL,<value>,kg,pool water all trains,-,derived,derived,SRC-IC-03,A-IC-03,see ic-endurance.csv
IC_POOL_MASS,<value>/3,kg,pool water per train,-,derived,assumed split,-,A-IC-04,-
IC_POOL_COOLING_W,0,W,credited pool cooling,-,none,assumed,-,-,0..2e6
IC_POOL_T_AMBIENT,40,degC,idle pool floor,-,none,assumed,-,A-IC-08,20..50
```

- [ ] **Step 8: Commit**

```bash
git add docs/model crates/bwr-core/src/params.rs crates/bwr-core/src/ic.rs
git commit -m "docs(ic): isolation condenser specification, evidence rows and derived pool inventory

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 2: Per-train valves, tube inventory and conserved pool in `ic.rs`

**Files:**
- Modify: `crates/bwr-core/src/ic.rs` (full rewrite below)
- Test: `crates/bwr-core/src/ic.rs` unit tests

**Interfaces:**
- Consumes: `steam::sat_p`, `steam::P_ATM`, `params::*`.
- Produces: `IcTrain` (master fields plus `steam_valve_cmd_open`, `return_valve_stuck`), `IcStep { q_removed, w_condensate_return, energy_to_pool, pool_boiloff_kg, pool_cooling_w }`, `IcTrain::in_service()`, `IcTrain::step(dt, p_vessel, t_sat, steam_available) -> IcStep`, `IcTrain::envelope_exceeded(t_sat)`, `IcTrain::pool_level_pct()`.

- [ ] **Step 1: Write the failing unit tests**

Replace the test module in `crates/bwr-core/src/ic.rs` with (keep `derived_pool_mass_covers_seven_days_to_100c` from Task 1 inside it):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::steam::sat_p;

    const DT: f64 = 0.05;
    fn rated() -> (f64, f64) {
        let s = sat_p(P_RATED);
        (s.p, s.t)
    }
    fn run(tr: &mut IcTrain, seconds: f64, p: f64, t: f64) -> IcStep {
        let mut last = IcStep::default();
        for _ in 0..(seconds / DT) as usize {
            last = tr.step(DT, p, t, true);
        }
        last
    }

    #[test]
    fn flooded_tubes_with_closed_return_valves_remove_nothing() {
        let (p, t) = rated();
        let mut tr = IcTrain::new();
        tr.pool_t = 100.0;
        assert!((tr.tube_mass - IC_TUBE_CAPACITY_KG).abs() < 1e-9, "standby tubes start flooded");
        let out = run(&mut tr, 60.0, p, t);
        assert_eq!(out.q_removed, 0.0);
        assert_eq!(tr.w_steam_in, 0.0);
        assert!(!tr.in_service());
    }

    #[test]
    fn opening_return_valves_drains_tubes_and_reaches_rated_duty() {
        let (p, t) = rated();
        let mut tr = IcTrain::new();
        tr.pool_t = 100.0;
        tr.cmd_open = true;
        let out = run(&mut tr, 300.0, p, t);
        assert!(tr.in_service());
        assert!(tr.tube_mass < 0.2 * IC_TUBE_CAPACITY_KG, "tubes drained: {}", tr.tube_mass);
        let rel = out.q_removed / IC_TRAIN_RATED_W;
        assert!(rel > 0.8 && rel < 1.05, "duty {rel} of rated");
        let w_in = tr.w_steam_in;
        assert!((out.w_condensate_return - w_in).abs() < 0.05 * w_in, "return {} vs in {}", out.w_condensate_return, w_in);
    }

    #[test]
    fn parallel_return_valve_redundancy() {
        let (p, t) = rated();
        let mut both = IcTrain::new();
        both.pool_t = 100.0;
        both.cmd_open = true;
        let mut one = both.clone();
        one.return_valve_stuck[0] = true;
        let q_both = run(&mut both, 300.0, p, t).q_removed;
        let q_one = run(&mut one, 300.0, p, t).q_removed;
        assert_eq!(one.return_valve_pos[0], 0.0, "stuck valve never moved");
        assert!(one.return_valve_pos[1] > 0.99);
        assert!((q_one - q_both).abs() < 0.01 * q_both, "one valve {q_one} vs both {q_both}");
    }

    #[test]
    fn unavailable_train_valves_do_not_move() {
        let (p, t) = rated();
        let mut tr = IcTrain::new();
        tr.available = false;
        tr.cmd_open = true;
        tr.pool_t = 100.0;
        let out = run(&mut tr, 120.0, p, t);
        assert_eq!(tr.return_valve_pos, [0.0; N_IC_RETURN_VALVES]);
        assert_eq!(out.q_removed, 0.0);
    }

    #[test]
    fn closed_steam_valve_removes_nothing() {
        let (p, t) = rated();
        let mut tr = IcTrain::new();
        tr.pool_t = 100.0;
        tr.cmd_open = true;
        tr.steam_valve_cmd_open = false;
        let out = run(&mut tr, 120.0, p, t);
        assert!(tr.steam_valve_pos < 1e-9);
        assert_eq!(out.q_removed, 0.0);
        assert!(!tr.in_service());
    }

    #[test]
    fn degradation_scales_duty() {
        let (p, t) = rated();
        let mut a = IcTrain::new();
        a.pool_t = 100.0;
        a.cmd_open = true;
        let mut b = a.clone();
        b.ua_factor = 0.5;
        let qa = run(&mut a, 300.0, p, t).q_removed;
        let qb = run(&mut b, 300.0, p, t).q_removed;
        assert!((qb / qa - 0.5).abs() < 0.03, "ratio {}", qb / qa);
    }

    #[test]
    fn pool_energy_closes_through_boiling() {
        let (p, t) = rated();
        let mut tr = IcTrain::new();
        tr.pool_t = 40.0;
        tr.pool_mass = 20_000.0; // small pool so it boils within the test
        tr.cmd_open = true;
        let m0 = tr.pool_mass;
        let t0 = tr.pool_t;
        let mut e_in = 0.0;
        let mut boiloff = 0.0;
        let mut e_cool = 0.0;
        for _ in 0..(1200.0 / DT) as usize {
            let o = tr.step(DT, p, t, true);
            e_in += o.energy_to_pool;
            boiloff += o.pool_boiloff_kg;
            e_cool += o.pool_cooling_w * DT;
            assert!(tr.pool_t <= 100.0 + 1e-9, "pool overshoot {}", tr.pool_t);
            assert!(tr.pool_mass >= 0.0);
        }
        assert!(tr.pool_t > 99.99, "pool should be boiling");
        assert!(boiloff > 0.0);
        let sensible = m0 * CP_WATER * (100.0 - t0);
        let expected = sensible + boiloff * H_FG_ATM + e_cool;
        assert!((e_in - expected).abs() < 1e-6 * expected, "e_in {e_in} vs {expected}");
        assert!((m0 - boiloff - tr.pool_mass).abs() < 1e-6);
    }

    #[test]
    fn empty_pool_stops_duty_and_flags_envelope() {
        let (p, t) = rated();
        let mut tr = IcTrain::new();
        tr.pool_t = 100.0;
        tr.pool_mass = 50.0;
        tr.cmd_open = true;
        let mut last = IcStep::default();
        for _ in 0..(600.0 / DT) as usize {
            last = tr.step(DT, p, t, true);
            assert!(tr.pool_mass >= 0.0);
            assert!(last.q_removed * DT <= last.energy_to_pool + 1e-6);
        }
        assert_eq!(tr.pool_mass, 0.0);
        assert_eq!(last.q_removed, 0.0);
        assert!(tr.envelope_exceeded(t));
        assert!(!tr.envelope_exceeded(90.0), "no demand below pool boiling point");
    }

    #[test]
    fn no_removal_without_steam_or_below_pool_temperature() {
        let mut tr = IcTrain::new();
        tr.cmd_open = true;
        tr.tube_mass = 0.0;
        let o = tr.step(DT, 101_325.0, 60.0, false);
        assert_eq!(o.q_removed, 0.0);
        tr.pool_t = 100.0;
        let o = tr.step(DT, sat_p(90.0_f64.max(1.0)).p, 90.0, true);
        assert_eq!(o.q_removed, 0.0);
    }

    // derived_pool_mass_covers_seven_days_to_100c from Task 1 stays here.
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --release -p bwr-core ic::tests`
Expected: compile errors (`IcStep: Default` missing, `steam_valve_cmd_open`, `return_valve_stuck`, `in_service`, `envelope_exceeded` not found, `step` arity).

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
//! Specification: docs/model/isolation-condensers.md.

use crate::params::*;
use crate::steam::{sat_p, P_ATM};

/// Liquid water specific heat used for the pool, J/kgK.
pub const CP_WATER: f64 = 4186.0;
/// Latent heat at atmospheric pressure (IAPWS, 100 degC), J/kg.
pub const H_FG_ATM: f64 = 2.257e6;

/// Heat transfer UA per train, W/K (rated duty at 7.2 MPa with a boiling pool).
fn ua() -> f64 {
    IC_TRAIN_RATED_W / (sat_p(P_RATED).t - 100.0)
}

/// What one train did in one step.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct IcStep {
    /// Heat taken from reactor steam, W (= w_steam_in * h_fg).
    pub q_removed: f64,
    /// Condensate returned to the vessel, kg/s.
    pub w_condensate_return: f64,
    /// Energy absorbed by the pool this step, J (sensible + latent + cooling).
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
    /// Heat removed from reactor steam, W.
    pub heat: f64,
    /// Steam drawn from the vessel, kg/s.
    pub w_steam_in: f64,
    /// Condensate returned to the vessel, kg/s.
    pub w_condensate_return: f64,
    pub pool_t: f64,
    pub pool_mass: f64,
}

impl IcTrain {
    /// Standby train at pressure: steam valve open, return valves closed, tubes flooded.
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
            heat: 0.0,
            w_steam_in: 0.0,
            w_condensate_return: 0.0,
            pool_t: IC_POOL_T_AMBIENT,
            pool_mass: IC_POOL_MASS,
        }
    }

    pub fn pool_level_pct(&self) -> f64 {
        self.pool_mass / IC_POOL_MASS * 100.0
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
        self.pool_mass <= 0.0 && self.in_service() && t_sat > 100.0
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

    /// Advance one step. `p_vessel` and `t_sat` are the reactor pressure (Pa) and
    /// steam temperature (degC); `steam_available` is false when the vessel has no
    /// steam space. Returns what the train did.
    pub fn step(&mut self, dt: f64, p_vessel: f64, t_sat: f64, steam_available: bool) -> IcStep {
        self.move_valves(dt);
        let s = sat_p(p_vessel.max(P_ATM));
        let h_fg = s.h_fg();

        // Condensing duty, limited by flooded tubes, pool coverage and pool capacity.
        let f_surf = (1.0 - self.tube_mass / IC_TUBE_CAPACITY_KG).clamp(0.0, 1.0);
        let coverage = (self.pool_level_pct() / 40.0).clamp(0.0, 1.0);
        let dt_drive = if steam_available && self.available { (t_sat - self.pool_t).max(0.0) } else { 0.0 };
        let mut q = ua() * self.ua_factor * dt_drive * self.steam_valve_pos * f_surf * coverage;
        let pool_capacity_j = if self.pool_t < 100.0 {
            self.pool_mass * CP_WATER * (100.0 - self.pool_t) + self.pool_mass * H_FG_ATM
        } else {
            self.pool_mass * H_FG_ATM
        };
        q = q.min(pool_capacity_j / dt).max(0.0);

        // Tube inventory: condensate in from the steam side, out through the return valves.
        let w_in = q / h_fg;
        let w_out = (self.return_open().sqrt() * self.tube_mass / IC_DRAIN_TAU_S).min(self.tube_mass / dt + w_in);
        self.tube_mass = (self.tube_mass + (w_in - w_out) * dt).clamp(0.0, IC_TUBE_CAPACITY_KG);
        self.heat = q;
        self.w_steam_in = w_in;
        self.w_condensate_return = w_out;

        // Pool: sensible heating to 100 degC, then boil-off; no overshoot.
        let cooling_w = if self.pool_t > IC_POOL_T_AMBIENT { IC_POOL_COOLING_W } else { 0.0 };
        let mut e = (q - cooling_w) * dt;
        let mut boiloff = 0.0;
        if e > 0.0 {
            if self.pool_t < 100.0 {
                let to_boil = self.pool_mass * CP_WATER * (100.0 - self.pool_t);
                if e <= to_boil {
                    self.pool_t += e / (self.pool_mass.max(1e-9) * CP_WATER);
                    e = 0.0;
                } else {
                    self.pool_t = 100.0;
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

        IcStep {
            q_removed: q,
            w_condensate_return: w_out,
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

Run: `cargo test --release -p bwr-core ic::tests`
Expected: all 10 `ic::tests::*` PASS. (`sim.rs` will not compile yet because it still calls the old `step` signature and `VesselInputs.q_ic`; run with `--lib` only if the integration tests block: `cargo test --release -p bwr-core --lib ic::tests`.)

If `pool_energy_closes_through_boiling` fails on the closure assertion by more than `1e-6`, the cause is `q.min(pool_capacity_j / dt)` interacting with the `.max(1e-9)` guard; print `e_in - expected` and fix the arithmetic, do not loosen the tolerance.

- [ ] **Step 5: Commit**

```bash
git add crates/bwr-core/src/ic.rs
git commit -m "feat(ic): per-train steam/return valves, tube inventory and conserved pool

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 3: Couple the vessel to IC mass flows and wire faults in `Sim`

**Files:**
- Modify: `crates/bwr-core/src/vessel.rs` (`VesselInputs`, `step`)
- Modify: `crates/bwr-core/src/ledger.rs` (`BoundaryFlows`)
- Modify: `crates/bwr-core/src/sim.rs` (IC section of `step`, `cold_shutdown`, `SetIcTrain`)
- Create: `crates/bwr-core/tests/ic_transients.rs`

**Interfaces:**
- Consumes: `Fault::{IcTrainUnavailable, IcReturnValveStuck, IcSteamValveClosed, IcNoncondensableDegradation}`, `DomainError::IcModelEnvelope`, `Sim.domain_errors`, `Sim.log`, `vessel::Regime`.
- Produces: `VesselInputs { q_core, w_ic_steam, w_ic_return, h_ic_return, w_feed, h_feed, w_steam, w_liquid_out }`; `BoundaryFlows.w_ic_steam`, `BoundaryFlows.w_ic_return`; `q_ic` in `BoundaryFlows` and `EnergyBalance` equal to `w_ic_steam*h_g - w_ic_return*h_ic_return`.

- [ ] **Step 1: Write the failing closed-loop conservation test**

Create `crates/bwr-core/tests/ic_transients.rs`:

```rust
//! Isolation condenser transients: conservation, availability, faults, initiation
//! ordering, degradation sensitivity and long-duration endurance.
//! Specification: docs/model/isolation-condensers.md.

use bwr_core::api::{Command, ScenarioName};
use bwr_core::faults::Fault;
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
}

#[test]
fn ic_energy_removal_is_reported_consistently() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    isolate(&mut sim);
    run(&mut sim, 300.0);
    let q_trains: f64 = sim.ic.iter().map(|t| t.heat).sum();
    let s = sim.vessel.sat();
    let w_in: f64 = sim.ic.iter().map(|t| t.w_steam_in).sum();
    let w_ret: f64 = sim.ic.iter().map(|t| t.w_condensate_return).sum();
    let q_vessel = w_in * s.h_g - w_ret * s.h_f;
    assert!(q_trains > 1.0e6);
    // In quasi-steady drain, w_ret ~ w_in and q_vessel ~ q_trains within the tube storage term.
    assert!((q_vessel - q_trains).abs() < 0.1 * q_trains, "vessel sees {q_vessel}, trains report {q_trains}");
    assert!((sim.balance.q_ic_w - q_vessel).abs() < 1.0, "energy balance q_ic {}", sim.balance.q_ic_w);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --release -p bwr-core --test ic_transients`
Expected: compile error in `sim.rs` (old `IcTrain::step` arity, `VesselInputs.q_ic`) — the crate does not build until Step 3.

- [ ] **Step 3: Change `VesselInputs` and the vessel step**

In `crates/bwr-core/src/vessel.rs` replace the `VesselInputs` struct:

```rust
#[derive(Clone, Debug)]
pub struct VesselInputs {
    /// Heat from the core into the coolant, W.
    pub q_core: f64,
    /// Steam drawn by the isolation condensers, kg/s.
    pub w_ic_steam: f64,
    /// Condensate returned by the isolation condensers, kg/s.
    pub w_ic_return: f64,
    /// Enthalpy of the returned condensate, J/kg.
    pub h_ic_return: f64,
    pub w_feed: f64,
    pub h_feed: f64,
    /// Steam leaving through the main steam lines, kg/s.
    pub w_steam: f64,
    /// Liquid removed (RWCU reject), kg/s.
    pub w_liquid_out: f64,
}
```

In `Vessel::step`, where P3 bounds outflows and builds `BoundaryFlows`, treat IC steam as a steam outflow and IC return as an inflow. The mass and energy lines become (keeping P3's bounding factor `scale` applied to every outflow):

```rust
        let w_steam = if steam_space { inp.w_steam } else { 0.0 };
        let w_ic_steam = if steam_space { inp.w_ic_steam } else { 0.0 };
        let out_total = w_steam + w_ic_steam + inp.w_liquid_out + self.vent_flow;
        let scale = if out_total * dt > self.mass + (inp.w_feed + inp.w_ic_return) * dt {
            ((self.mass + (inp.w_feed + inp.w_ic_return) * dt) / (out_total * dt)).max(0.0)
        } else {
            1.0
        };
        let (w_steam, w_ic_steam, w_liq, w_vent) =
            (w_steam * scale, w_ic_steam * scale, inp.w_liquid_out * scale, self.vent_flow * scale);
        let dm = inp.w_feed + inp.w_ic_return - w_steam - w_ic_steam - w_liq - w_vent;
        let de = inp.q_core - heat_loss + inp.w_feed * inp.h_feed + inp.w_ic_return * inp.h_ic_return
            - (w_steam + w_ic_steam + w_vent) * h_steam
            - w_liq * h_liq;
```

(`steam_space` is P4's `matches!(self.eos.regime, Regime::Saturated | Regime::GasBlanketed)`; P3 named its exhaustion handling — keep it, just include `w_ic_steam` in the scaled set.) Build the returned flows:

```rust
        BoundaryFlows {
            w_feed: inp.w_feed,
            h_feed: inp.h_feed,
            w_steam,
            h_steam,
            w_vent,
            w_liquid_out: w_liq,
            h_liquid_out: h_liq,
            w_ic_steam,
            w_ic_return: inp.w_ic_return,
            q_core: inp.q_core,
            q_ic: w_ic_steam * h_steam - inp.w_ic_return * inp.h_ic_return,
            q_loss: heat_loss,
            mass_clipped: 0.0,
            energy_clipped: 0.0,
        }
```

In `crates/bwr-core/src/ledger.rs` add the two fields to `BoundaryFlows` after `w_liquid_out`/`h_liquid_out`:

```rust
    /// Steam drawn by the isolation condensers, kg/s (out).
    pub w_ic_steam: f64,
    /// Condensate returned by the isolation condensers, kg/s (in).
    pub w_ic_return: f64,
```

and in `Ledger::record` count them: `mass_in += f.w_ic_return * dt; mass_out += f.w_ic_steam * dt;` and leave the energy terms as they are (`q_ic` already carries the net IC energy; do not double count: `energy_out += f.q_ic * dt` stays, and the `w_ic_*` enthalpies must NOT be added separately). Add a doc line on `q_ic`: `/// Net energy removed by the ICs, W: w_ic_steam*h_steam - w_ic_return*h_ic_return.`

- [ ] **Step 4: Wire the trains in `Sim::step`**

Replace the `// --- isolation condensers` block in `crates/bwr-core/src/sim.rs`:

```rust
        // --- isolation condensers
        let steam_avail = matches!(self.vessel.eos.regime, Regime::Saturated | Regime::GasBlanketed);
        let (mut w_ic_steam, mut w_ic_return) = (0.0, 0.0);
        for (i, tr) in self.ic.iter_mut().enumerate() {
            tr.available = !self.faults.contains(&Fault::IcTrainUnavailable { train: i });
            tr.steam_valve_cmd_open = !self.faults.contains(&Fault::IcSteamValveClosed { train: i });
            for v in 0..N_IC_RETURN_VALVES {
                tr.return_valve_stuck[v] = self.faults.contains(&Fault::IcReturnValveStuck { train: i, valve: v });
            }
            let degradation = self
                .faults
                .iter()
                .filter_map(|f| match f {
                    Fault::IcNoncondensableDegradation { train, pct } if *train == i => Some(*pct as f64 / 100.0),
                    _ => None,
                })
                .fold(0.0, f64::max);
            tr.ua_factor = (1.0 - degradation).clamp(0.0, 1.0);
            let out = tr.step(dt, p, t_cool, steam_avail);
            w_ic_steam += tr.w_steam_in;
            w_ic_return += out.w_condensate_return;
        }
        let h_ic_return = self.vessel.sat().h_f;
        for i in 0..self.ic.len() {
            if self.ic[i].envelope_exceeded(t_cool)
                && !self.domain_errors.iter().any(|(_, e)| matches!(e, DomainError::IcModelEnvelope { train, .. } if *train == i))
            {
                self.domain_errors.push((self.time, DomainError::IcModelEnvelope { train: i, reason: "pool depleted".into() }));
                self.log.push((self.time, format!("MODEL ENVELOPE: IC train {} pool depleted", i + 1)));
            }
        }
```

Add `use crate::domain::DomainError; use crate::faults::Fault; use crate::vessel::Regime;` to the imports if not already present. Build the vessel inputs with the new fields:

```rust
        let inputs = VesselInputs {
            q_core: self.q_core,
            w_ic_steam,
            w_ic_return,
            h_ic_return,
            w_feed: self.fw.flow,
            h_feed: self.fw.h_feed(),
            w_steam: self.steam.total_flow(),
            w_liquid_out: self.fw.rwcu_reject,
        };
        let flows = self.vessel.step(dt, &inputs);
```

Where P6 fills `self.balance.q_ic_w`, set it from `flows.q_ic`. Where P6/P3 record the ledger, nothing changes (`self.ledger.record(dt, &flows)`).

In `Sim::cold_shutdown()` after `Self::base(...)` add:

```rust
        for t in s.ic.iter_mut() {
            t.tube_mass = 0.0; // no steam for 30 days: tubes drained
        }
```

In `apply`, replace the `SetIcTrain` arm:

```rust
            Command::SetIcTrain { index, open } => {
                if index < self.ic.len() {
                    if !self.ic[index].available {
                        self.event("ic_cmd", format!("IC train {} unavailable", index + 1));
                        return;
                    }
                    self.ic[index].cmd_open = open;
                    self.log.push((self.time, format!("IC train {} {}", index + 1, if open { "manual open" } else { "manual close" })));
                }
            }
```

In `protection()`, replace the auto-initiation block so unavailable trains are skipped and the log records the event:

```rust
        if self.vessel.eos.p > IC_HIGH_PRESSURE || self.level_cm() < L2_CM || isolated_at_power {
            let mut started = false;
            for t in self.ic.iter_mut().filter(|t| t.available && !t.cmd_open) {
                t.cmd_open = true;
                started = true;
            }
            if started {
                self.event("ic_init", "Isolation condensers auto-initiated");
                self.log.push((self.time, "IC auto-initiation".into()));
            }
        }
```

- [ ] **Step 5: Fix compile errors in the rest of the crate**

Run: `cargo build --release -p bwr-core 2>&1 | grep -E "^error" -A5`
Expected: errors only where `VesselInputs { q_ic, .. }` is constructed (examples and tests from earlier packages, e.g. `tests/conservation.rs`, `tests/vessel_transients.rs`). Update each to the new fields with `w_ic_steam: 0.0, w_ic_return: 0.0, h_ic_return: 0.0` unless that test exercises ICs.

Run: `cargo test --release -p bwr-core --test ic_transients`
Expected: both tests PASS.

Run: `cargo test --release -p bwr-core`
Expected: everything passes except possibly `plant::msiv_closure_scrams_and_isolation_condensers_cool` (it asserts `t.heat > 1.0e6` for any train, which still holds) and P2 baseline comparisons, which are regenerated in Task 10. If any other test fails, read the failure; do not change its expected value in this task.

- [ ] **Step 6: Commit**

```bash
git add crates/bwr-core/src/vessel.rs crates/bwr-core/src/ledger.rs crates/bwr-core/src/sim.rs crates/bwr-core/tests
git commit -m "feat(ic): vessel sees IC steam and condensate flows; faults and envelope wired in Sim

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 4: Initiation, latching, manual control and fault behaviour at plant level

**Files:**
- Modify: `crates/bwr-core/tests/ic_transients.rs`
- Modify: `crates/bwr-core/src/sim.rs` only if a test exposes a defect

**Interfaces:**
- Consumes: Task 3 wiring, `Sim.log`, `Sim.domain_errors`.
- Produces: nothing new; verifies behaviour the spec requires ("automatic initiation, manual control, reset/latch behavior, available trains").

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
    assert!(sim.log.iter().any(|(_, t)| t == "IC auto-initiation"));
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
    run(&mut sim, 30.0);
    assert!(!sim.ic[1].cmd_open);
    let snap = sim.snapshot();
    assert!(snap.alarms.iter().any(|a| a.text.contains("IC train 2 unavailable")), "{:?}", snap.alarms);
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
}

#[test]
fn closed_steam_valve_train_removes_nothing_at_plant_level() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    sim.set_fault(Fault::IcSteamValveClosed { train: 1 }, true);
    isolate(&mut sim);
    run(&mut sim, 300.0);
    assert!(sim.ic[1].steam_valve_pos < 1e-9);
    assert_eq!(sim.ic[1].heat, 0.0);
    assert!(sim.ic[0].heat > 1.0e6 && sim.ic[2].heat > 1.0e6);
    assert!(sim.domain_errors.is_empty(), "a closed valve is a fault, not a model-envelope breach");
}

#[test]
fn pool_depletion_raises_model_envelope_error_once() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    for t in sim.ic.iter_mut() {
        t.pool_mass = 2_000.0; // tiny pools so depletion happens inside the test
        t.pool_t = 100.0;
    }
    isolate(&mut sim);
    run(&mut sim, 1800.0);
    let ic_errors: Vec<_> = sim
        .domain_errors
        .iter()
        .filter(|(_, e)| matches!(e, bwr_core::domain::DomainError::IcModelEnvelope { .. }))
        .collect();
    assert_eq!(ic_errors.len(), 3, "one entry per depleted train: {:?}", sim.domain_errors);
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
Expected: all PASS. If `manual_command_to_unavailable_train_is_rejected` fails because `event` ids expire after 30 s, that is fine at 30 s of run; the test runs 30 s exactly — if it is flaky at the boundary, change the run to `run(&mut sim, 10.0)`. If `auto_initiation_opens_only_available_trains_and_latches` fails at the `in_service()` assertion after 600 s, check that `Sim` is not re-closing valves anywhere (grep `cmd_open = false` in `sim.rs`); the only writer must be `SetIcTrain`.

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
- Modify: `docs/model/isolation-condensers.md` (results pointer already present)

**Interfaces:**
- Consumes: `Fault::IcNoncondensableDegradation { train, pct }`, `isolate`, `results`.
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
Expected: PASS; `docs/validation/results/ic-degradation.csv` created with three rows. If the 50 % case exceeds design pressure, that is a finding: keep the test failing, report it in the commit message and in `isolation-condensers.md` under "Results tables", and stop this task for Jim's decision (the spec forbids inventing performance).

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

**Interfaces:**
- Consumes: `Fault::CwPumpTrip` (P6), turbine trip on low vacuum, bypass block on condenser pressure, high-pressure scram, IC auto-initiation; `Sim.log` entries written by P2–P6 (`"Turbine trip: ..."`, scram text, `"IC auto-initiation"`).

- [ ] **Step 1: Write the test**

Append:

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
    let t_turbine = find("turbine").expect("turbine trip logged");
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

Add to `Sim` in `sim.rs` (public helper used only by tests):

```rust
    pub fn cw_pumps_all_off(&self) -> bool {
        self.cond.cw_pumps.iter().all(|&b| !b)
    }
```

- [ ] **Step 2: Run**

Run: `cargo test --release -p bwr-core --test ic_transients loss_of_circulating`
Expected: PASS. If the scram is logged before the turbine trip, inspect `sim.log` with `--nocapture`; the first signal after vacuum loss must be the condenser-pressure turbine trip (`CONDENSER_TRIP_P` = 25 kPa) and the scram should follow from pressure. If instead the scram comes from MSIV closure (low condenser vacuum MSIV isolation is not implemented), the ordering assertion still holds; record the actual order in the commit message.

- [ ] **Step 3: Commit**

```bash
git add crates/bwr-core/tests/ic_transients.rs crates/bwr-core/src/sim.rs
git commit -m "test(ic): loss of circulating water ordering ends in IC cooling

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 7: Train availability matrix, inadvertent initiation, hot and low pools

**Files:**
- Modify: `crates/bwr-core/tests/ic_transients.rs`

**Interfaces:**
- Produces: `docs/validation/results/ic-availability.csv` rows `case,trains,pool_t0_c,pool_frac0,peak_pressure_pa,peak_time_s,above_design,p_600s_pa,t_600s_c`.

- [ ] **Step 1: Write the tests**

Append:

```rust
struct IcCase {
    name: &'static str,
    trains: usize,
    pool_t0: f64,
    pool_frac0: f64,
}

fn isolation_with(case: &IcCase) -> (f64, f64, f64, f64) {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 5.0);
    for i in case.trains..N_IC_TRAINS {
        sim.set_fault(Fault::IcTrainUnavailable { train: i }, true);
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
    let cases = [
        IcCase { name: "three_trains", trains: 3, pool_t0: IC_POOL_T_AMBIENT, pool_frac0: 1.0 },
        IcCase { name: "two_trains", trains: 2, pool_t0: IC_POOL_T_AMBIENT, pool_frac0: 1.0 },
        IcCase { name: "one_train", trains: 1, pool_t0: IC_POOL_T_AMBIENT, pool_frac0: 1.0 },
        IcCase { name: "three_trains_hot_pool", trains: 3, pool_t0: 100.0, pool_frac0: 1.0 },
        IcCase { name: "three_trains_low_pool", trains: 3, pool_t0: IC_POOL_T_AMBIENT, pool_frac0: 0.3 },
    ];
    let mut rows = Vec::new();
    for c in &cases {
        let (peak, t_peak, p600, t600) = isolation_with(c);
        let above = peak > VESSEL_DESIGN_PRESSURE;
        rows.push(format!("{},{},{},{},{:.0},{:.1},{},{:.0},{:.1}", c.name, c.trains, c.pool_t0, c.pool_frac0, peak, t_peak, above, p600, t600));
        if c.trains >= 2 {
            assert!(!above, "{}: peak {peak} Pa above design pressure", c.name);
            assert!(p600 < IC_HIGH_PRESSURE, "{}: pressure not falling after 600 s ({p600} Pa)", c.name);
        }
        // One train: recorded only. The spec forbids inventing a performance target.
    }
    results("ic-availability", "case,trains,pool_t0_c,pool_frac0,peak_pressure_pa,peak_time_s,above_design,p_600s_pa,t_600s_c", &rows);
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

Run: `BWR_WRITE_RESULTS=1 cargo test --release -p bwr-core --test ic_transients -- train_availability inadvertent --nocapture`
Expected: PASS; two CSV files written. If `two_trains` exceeds design pressure, that is a finding against `IC_TRAIN_RATED_W` or the pool split (A-IC-04): leave the test failing, add the row to `isolation-condensers.md` "Results tables", and stop for Jim's decision.

- [ ] **Step 3: Commit**

```bash
git add crates/bwr-core/tests/ic_transients.rs docs/validation/results/ic-availability.csv docs/validation/results/ic-inadvertent.csv
git commit -m "test(ic): availability matrix, inadvertent initiation, hot and low pools

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 8: 72-hour and 7-day endurance cases with coarse-step convergence

**Files:**
- Modify: `crates/bwr-core/src/validation.rs` (`Case.dt_after_s`, `DT_LONG`, `run_case`, `standard_cases`)
- Modify: `docs/validation/cases.csv`
- Modify: `crates/bwr-core/tests/ic_transients.rs`
- Modify: `.github/workflows/validate.yml`
- Modify: every place that constructs a `Case` literal (grep `Case {`) to add `dt_after_s: None`

**Interfaces:**
- Produces: `validation::DT_LONG = 0.5`, `Case.dt_after_s: Option<(f64, f64)>`, cases `fp_scram_ic_72h` and `fp_scram_ic_7d`.

- [ ] **Step 1: Write the failing convergence test**

Append to `ic_transients.rs`:

```rust
use bwr_core::validation::{run_case, standard_cases, Tolerance, DT_LONG};

fn case(id: &str) -> bwr_core::validation::Case {
    standard_cases().into_iter().find(|c| c.id == id).unwrap_or_else(|| panic!("case {id} missing"))
}

#[test]
fn coarse_step_matches_fine_step_over_the_first_hour() {
    let mut c = case("fp_scram_ic_72h");
    c.duration_s = 3600.0;
    let mut fine = c.clone();
    fine.dt_after_s = None;
    let a = run_case(&fine, DT).metrics();
    let b = run_case(&c, DT).metrics(); // DT for 600 s, then DT_LONG
    let violations = bwr_core::validation::compare(&a, &b, &Tolerance::physical());
    assert!(violations.is_empty(), "coarse step diverges: {violations:?}");
}

#[test]
fn seventy_two_hour_cooling_with_three_trains() {
    let tr = run_case(&case("fp_scram_ic_72h"), DT);
    let pool = tr.column("ic_pool_kg");
    let temp_end = *tr.column("t_coolant_c").last().unwrap();
    let p_peak = tr.column("pressure_pa").iter().cloned().fold(0.0, f64::max);
    assert!(*pool.last().unwrap() > 0.0, "pool depleted before 72 h");
    assert!(p_peak < VESSEL_DESIGN_PRESSURE);
    assert!(temp_end >= 100.0 - 0.5, "ICs cannot cool below the boiling pool: {temp_end} degC");
    assert!(!tr.events.iter().any(|(_, e)| e.contains("MODEL ENVELOPE")), "{:?}", tr.events);
    results(
        "ic-endurance",
        "case,hours,pool_fraction,t_coolant_c,peak_pressure_pa",
        &[format!("fp_scram_ic_72h,72,{:.3},{:.1},{:.0}", pool.last().unwrap() / IC_POOL_MASS_TOTAL, temp_end, p_peak)],
    );
}

#[test]
#[ignore = "7-day run: executed by validate.yml with --ignored"]
fn seven_day_cooling_with_three_trains() {
    let tr = run_case(&case("fp_scram_ic_7d"), DT);
    let pool = tr.column("ic_pool_kg");
    let temp = tr.column("t_coolant_c");
    let p_peak = tr.column("pressure_pa").iter().cloned().fold(0.0, f64::max);
    let at = |h: f64| {
        let i = tr.column("t").iter().position(|&t| t >= h * 3600.0).unwrap();
        (pool[i], temp[i])
    };
    let (pool72, t72) = at(72.0);
    let (pool7d, t7d) = (*pool.last().unwrap(), *temp.last().unwrap());
    assert!(pool72 > 0.0);
    assert!(t7d >= 100.0 - 0.5, "{t7d}");
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

`TRACE_COLUMNS` in the master has `ic_pool_kg` but no coolant temperature column. Add `"t_coolant_c"` as a 25th column: change `TRACE_COLUMNS: [&str; 25]`, `rows: Vec<[f64; 25]>`, and the row builder in `run_case` to include `sim.vessel.eos.t` after `ic_pool_kg`. Note this in the master "Shared interface contract" P2 section as part of this commit (one-line edit: `[&str; 24]` → `[&str; 25]` plus the column name).

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --release -p bwr-core --test ic_transients coarse_step`
Expected: compile error: `dt_after_s` not a field of `Case`, `DT_LONG` not found.

- [ ] **Step 3: Extend `validation.rs`**

In `crates/bwr-core/src/validation.rs`:

```rust
/// Coarse step for long unattended-cooling cases, s. Convergence against `sim::DT`
/// over the first hour is checked by ic_transients::coarse_step_matches_fine_step_over_the_first_hour.
pub const DT_LONG: f64 = 0.5;
```

Add to `Case`:

```rust
    /// `(from_t, dt)`: after `from_t` seconds of simulation time, step with `dt`.
    pub dt_after_s: Option<(f64, f64)>,
```

In `run_case`, where the loop currently does `sim.step(dt)`, select the step:

```rust
        let h = match case.dt_after_s {
            Some((from_t, big)) if sim.time >= from_t => big,
            _ => dt,
        };
        sim.step(h);
```

and make the sampling test `sim.time + 1e-9 >= next_sample` (it already compares against `sample_dt_s`; with a 0.5 s step and `sample_dt_s` of 60 s sampling stays on multiples of 60 s). Add the two cases to `standard_cases()`:

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
        },
```

Add `dt_after_s: None` to every other `Case {` literal (`grep -rn "Case {" crates/bwr-core`).

In `examples/validation.rs`, skip cases with `duration_s > 24.0 * 3600.0` unless the `--long` flag is given (so the default baseline run stays short):

```rust
    let long = std::env::args().any(|a| a == "--long");
    for case in standard_cases().into_iter().filter(|c| long || c.duration_s <= 24.0 * 3600.0) {
```

- [ ] **Step 4: Register the cases**

Append to `docs/validation/cases.csv` (Package 2 column order: id, scenario, purpose, reference, tolerance_class):

```
fp_scram_ic_72h,fullPower,"72 h unattended cooling, 3 IC trains, no makeup, no pool cooling, 2 y at rated before scram","SRC-IC-03 passive cooling claim (conditions differ; simulator result only)",physical
fp_scram_ic_7d,fullPower,"7 d unattended cooling, same conditions; pool fraction reported","SRC-IC-03 7-day claim (not reproduced; reported)",physical
```

- [ ] **Step 5: Add the CI step**

In `.github/workflows/validate.yml`, after the existing validation run step add:

```yaml
      - name: Long-duration IC endurance (7 d)
        run: cargo test --release -p bwr-core --test ic_transients -- --ignored seven_day
```

- [ ] **Step 6: Run**

Run: `cargo test --release -p bwr-core --test ic_transients`
Expected: PASS (the 72 h case takes on the order of 30 s in release; if it exceeds 120 s, raise `DT_LONG` is NOT the fix — profile with `cargo build --release --example trace` and report).

Run: `BWR_WRITE_RESULTS=1 cargo test --release -p bwr-core --test ic_transients -- --ignored seven_day --nocapture`
Expected: PASS with the printed 7-day pool fraction; `docs/validation/results/ic-endurance.csv` written (the two tests both write this file; the 7-day run last, so run the 72 h test first, then the 7-day one, and merge the rows by hand into one file with three rows).

- [ ] **Step 7: Commit**

```bash
git add crates/bwr-core/src/validation.rs crates/bwr-core/examples/validation.rs crates/bwr-core/tests/ic_transients.rs docs/validation/cases.csv docs/validation/results/ic-endurance.csv .github/workflows/validate.yml docs/superpowers/plans/2026-10-20-refinement-master.md
git commit -m "feat(validation): 72 h and 7 d IC endurance cases with coarse-step convergence check

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 9: Snapshot, TypeScript contract, mock and mimic

**Files:**
- Modify: `crates/bwr-core/src/api.rs` (`IcTrain`)
- Modify: `crates/bwr-core/src/sim.rs` (`snapshot()` IC mapping)
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

Also update the `ic` alarm to use `in_service()`: `if self.ic.iter().any(|t| t.in_service())`.

Run: `cargo test --release -p bwr-core --test plant json_contract`
Expected: PASS.

- [ ] **Step 3: TypeScript side**

`ui/src/sim/types.ts`:

```ts
export interface IcTrain {
  /** max(returnValvePos) — kept for the mimic. */
  valvePos: number;
  /** Steam supply valve 0..1, normally open; closed only by a test fault. */
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

`ui/src/components/mimic/PlantMimic.tsx`: in the IC `Valve` element change the `title` and add a dim tone for an unavailable train:

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
- Consumes: `examples/validation.rs` compare and `--write-baseline` modes (P2).

- [ ] **Step 1: Compare against the pre-package baselines and record the differences**

Run: `cargo run --release -p bwr-core --example validation 2>&1 | tee /tmp/ic-before-after.txt`
Expected: violations only in cases where ICs act (`fp_msiv_closure`, `fp_loss_of_cw` and any case whose events include "IC"). For each violated metric, add a row to a "Baseline changes from Package 7" table at the end of `docs/model/isolation-condensers.md`:

```markdown
## Baseline changes from Package 7

| Case | Metric | Before | After | Cause |
|---|---|---|---|---|
| fp_msiv_closure | pressure_pa peak | <before> | <after> | tube drain lag (~60 s to full duty) replaces 20 s heat lag; return water now adds mass |
```

(one row per violation printed, copied verbatim from the runner output). Cases that do not touch the ICs must show no violations; if one does, stop and find out why before regenerating anything.

- [ ] **Step 2: Regenerate**

Run: `cargo run --release -p bwr-core --example validation -- --write-baseline`
Then: `cargo run --release -p bwr-core --example validation`
Expected: no violations.

- [ ] **Step 3: Coverage and README**

In `docs/model/coverage.md`, set the isolation condensers row to:

```
| Isolation condensers | docs/model/isolation-condensers.md; A-IC-01..10; SRC-IC-01..04 | tests/ic_transients.rs (12), ic::tests (10), cases fp_scram_ic_72h/7d | 1-train endurance and absolute duty are reported, not validated; cooling below 100 degC out of envelope |
```

In `README.md`, change the Isolation condensers row of the status table to:

```
| Isolation condensers | 3 passive trains, each with a steam valve and two parallel return valves; tubes flood in standby and drain into service; pools heat, boil off and are conserved; endurance to 7 days reported under stated assumptions (cannot cool below the boiling pool) |
```

and in "Data sources" append `- US NRC, *ESBWR Design Control Document* Tier 2 Rev. 10, chapter 5 (isolation condenser arrangement, used as a shared concept only)`.

- [ ] **Step 4: Full verification**

Run: `cargo test --release -p bwr-core && npm --prefix ui run typecheck && npm --prefix ui run build && cargo run --release -p bwr-core --example validation`
Expected: all green; the validation runner reports no violations.

- [ ] **Step 5: Commit**

```bash
git add docs/validation/baseline docs/model/coverage.md docs/model/isolation-condensers.md README.md
git commit -m "docs(ic): regenerate validation baselines after IC refinement; record before/after and coverage

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

## Package self-review (run before handing off)

- [ ] Spec bullets → tasks: train arrangement and parallel return valves (T1, T2); unsupported capacity/pool/timing replaced or quantified (T1, T7); steam supply / condensate / tube inventory / pressure difference / pool coupling (T2, T3); noncondensable sensitivity (T5); auto initiation, manual, reset/latch, available trains, loss of normal heat rejection (T4, T6); 1/2/3 trains, inadvertent, valve failure, hot and low pools, depletion (T4, T7); 72 h and 7 d with stated conditions (T8); lower temperature limit and unimplemented shutdown cooling boundary (T1 doc, T8 assertions).
- [ ] No placeholders: the only `<...>` cells are the derived pool mass and checksums produced by commands in Task 1 and the before/after table copied from runner output in Task 10.
- [ ] Names match the master: `IcTrain` fields, `IcStep`, `in_service`, `step(dt, p_vessel, t_sat, steam_available)`, `DomainError::IcModelEnvelope`, `Fault::Ic*`, Snapshot additions. Additions beyond the master are listed in the table at the top and must be appended to the master's P7 section in the Task 2 commit.
- [ ] Every new constant has a `parameters.csv` row (Task 1 Step 7).
- [ ] Every Snapshot change has `types.ts` and `mock.ts` steps and keeps `npm --prefix ui run typecheck` green (Task 9).
- [ ] Every defect fix has a failing test first (Task 2 Step 1, Task 3 Step 1, Task 8 Step 1, Task 9 Step 1).
