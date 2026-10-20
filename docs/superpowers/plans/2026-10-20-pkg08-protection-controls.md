# Package 8: Protection, Modes, Instruments, and Operator Controls — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Names and signatures follow `docs/superpowers/plans/2026-10-20-refinement-master.md`; the master wins on any conflict.

**Goal:** Give every implemented trip, isolation, permissive, block, reset, and alarm a sourced or explicitly assumed row in a cause-and-effect matrix, make protection act on sensed (lagged, ranged) values instead of model truth, fix the REFUEL one-rod interlock and the L8 feed-pump trip, verify reset and latch behaviour, and make displays say whether a number is actual, commanded, sensed, or model-calculated.

**Architecture:** Protection logic stays in `crates/bwr-core/src/sim.rs` but reads only `Sim.sensors` (a new `sensors.rs` module with first-order-lag, ranged sensors). The matrix in `docs/model/protection.csv` is the specification; `tests/protection.rs` has one positive, one negative, one boundary, and one transition test per row and per `Command`. The UI gains one small `ValueTag` component that suffixes readouts with ACT / CMD / SENSED / MODEL.

**Tech Stack:** Rust 2021, serde, TypeScript 5.6, React 18. No new dependencies.

**Prerequisites:** Packages 1–7 complete. This plan assumes: P3 `Sim::step` order (`core.step_rods` first), `DomainError`, `Sim.log`; P5 `Rods { motion: Vec<RodMotion>, stuck }`, `Rods::target(i)`, `Rods::targets()`, `Rods::set_motor_target(i, t)`, `Rods::scram_hydraulic()`, `Rods::scramming()`; `Snapshot.fuel_envelope_exceeded`; P6 `SteamSystem.tsv_pos`; P7 `IcTrain { available, in_service(), cmd_open, ... }`; P2 `validation::standard_cases()` and `docs/validation/baseline/`.

## Global Constraints

See the master plan "Global Constraints" (verbatim from the spec). In addition, for this package:

- "Do not assume the legacy SRM/IRM/APRM arrangement is the final BWRX arrangement."
- "No new claim of plant-specific voting architecture is made without evidence."
- Every setpoint in `params.rs` is ASSUMED today; the matrix must say so (`threshold_source = assumed`) until Package 1's register supplies a source.
- No `Command` variants are added. Faults stay engine-level.
- After any UI change, run the `frontend-design-audit` skill (`/fda:quick`) before calling the task done (Jim's CLAUDE.md).

**Acceptance gate (verbatim from the spec):** "Every control action and protective action has positive, negative, boundary, and transition tests. No new claim of plant-specific voting architecture is made without evidence."

---

## File structure

```
docs/model/protection.csv                 create  cause-and-effect matrix (Task 1)
docs/model/assumptions.md                 modify  add A-8xx rows (Tasks 1, 2, 7)
docs/model/parameters.csv                 modify  sensor constants, period trip constants (Tasks 2, 7)
docs/model/esbwr-applicability.csv        modify  DCD Ch. 7 rows (Task 1)
crates/bwr-core/src/params.rs             modify  PERIOD_TRIP_*, sensor ranges and taus
crates/bwr-core/src/sensors.rs            create  LagSensor, Sensors
crates/bwr-core/src/lib.rs                modify  pub mod sensors
crates/bwr-core/src/sim.rs                modify  scram_signals, withdraw_block, apply, protection, snapshot, sensors step
crates/bwr-core/src/api.rs                modify  Vessel sensed fields, Rods.motion
crates/bwr-core/tests/protection.rs       create  all tests in this package
ui/src/sim/types.ts                       modify  mirror api.rs
ui/src/sim/mock.ts                        modify  emit new fields
ui/src/components/common/ValueTag.tsx     create
ui/src/components/common/Readout.tsx      modify  optional `kind` prop
ui/src/styles.css                         modify  .vtag styles
ui/src/components/RodControls.tsx         modify  ACT / CMD tags, motion text
ui/src/components/FeedwaterPanel.tsx      modify  LEVEL NR sensed + OOR, LEVEL MODEL
ui/src/components/TurbinePanel.tsx        modify  RX PRESS sensed/model
ui/src/components/NuclearInstruments.tsx  modify  MODEL tags on k-eff, fuel temp, reactivity
ui/src/components/mimic/PlantMimic.tsx    modify  MSIV pos vs cmd lamp, level/pressure tags
docs/validation/baseline/*.csv            regenerate (Task 11)
```

Test helper used throughout `tests/protection.rs`:

```rust
use bwr_core::api::{Command, ModeSwitch, ScenarioName};
use bwr_core::bop::LevelControlMode;
use bwr_core::params::*;
use bwr_core::sim::DT;
use bwr_core::Sim;

fn run(sim: &mut Sim, seconds: f64) {
    let steps = (seconds / DT).round() as usize;
    for _ in 0..steps {
        sim.step(DT);
    }
}

/// True if any active alarm id equals `id`.
fn has_alarm(sim: &Sim, id: &str) -> bool {
    sim.snapshot().alarms.iter().any(|a| a.id == id)
}

/// True if the append-only log contains an entry starting with `prefix`.
fn logged(sim: &Sim, prefix: &str) -> bool {
    sim.log.iter().any(|(_, text)| text.starts_with(prefix))
}
```

---

### Task 1: Cause-and-effect matrix `docs/model/protection.csv`

**Files:**
- Create: `docs/model/protection.csv`
- Modify: `docs/model/esbwr-applicability.csv` (append rows)
- Modify: `docs/model/assumptions.md` (append A-801..A-804)

**Interfaces:**
- Consumes: current logic in `crates/bwr-core/src/sim.rs` (`scram_signals`, `isolation_signals`, `withdraw_block`, `protection`, `apply`, `active_alarms`) and thresholds in `params.rs`.
- Produces: row ids (`S01`.., `I01`.., `C01`.., `T01`.., `R01`, `B01`.., `P01`.., `A01`..) referenced by every test name in this package as `// matrix: S06`.

- [ ] **Step 1: Write the matrix**

Create `docs/model/protection.csv` with exactly this content. `threshold_source` is `assumed` for every numeric setpoint because `params.rs` marks all of them ASSUMED; Package 1's `parameters.csv` row id is given where one exists (`P-xxx` ids follow the Package 1 plan; use the id that plan assigned, or `assumed` if none).

```csv
id,function,signal,measurement,mode_applicability,threshold,threshold_unit,threshold_source,delay_s,latch,reset_condition,override,consequence,test
S01,scram,APRM high neutron flux,sensed APRM (kin.n),RUN,1.18,fraction of rated,assumed (APRM_HIGH_RUN),0,yes,signal clear AND >=10 s since scram AND operator ResetScram,none,hydraulic scram all rods; scrammed latch,scram_aprm_high_run
S02,scram,MSIV closure,average MSIV position over lines,RUN,0.85,fraction open (below),assumed (legacy BWR rule; per-line closure does not scram),0,yes,as S01,none,scram,scram_msiv_closure_run
S03,scram,APRM high flux setdown,sensed APRM,SHUTDOWN/REFUEL/STARTUP,0.15,fraction of rated,assumed (APRM_HIGH_SETDOWN),0,yes,as S01,none,scram,scram_aprm_setdown
S04,scram,IRM high-high,IRM reading on selected range,SHUTDOWN/REFUEL/STARTUP,120,% of range,assumed (IRM_HIGH_HIGH),0,yes,as S01,bypassed in RUN,scram,scram_irm_high_high
S05,scram,reactor high pressure,sensed dome pressure (sensors.pressure),all,7.50,MPa,assumed (SCRAM_HIGH_PRESSURE),sensor lag 0.2 s,yes,as S01,none,scram,scram_high_pressure
S06,scram,low water level L3,sensed NR level (sensors.level_nr),all,-100,cm vs NWL,assumed (L3_CM),sensor lag 0.5 s,yes,as S01,none,scram,scram_l3_uses_sensed_level
S07,scram,manual scram,operator Command::ManualScram,all,n/a,n/a,design intent,0,yes,as S01,none,scram,cmd_manual_scram
S08,scram,mode switch in SHUTDOWN,mode switch position,transition to SHUTDOWN and while in SHUTDOWN,n/a,n/a,assumed (legacy BWR),0,yes,mode switch moved out of SHUTDOWN AND as S01,none,scram; persistent signal blocks reset,scram_reset_blocked_in_shutdown
S09,scram,short period,inverse period (kin.inv_period),STARTUP (if enabled),10,s,unavailable (PERIOD_TRIP_ENABLED=false),0,yes,as S01,disabled by constant,scram when enabled,period_trip_disabled_by_default
I01,isolation,low-low water level L2,sensed WR level (sensors.level_wr),all,-400,cm vs NWL,assumed (L2_CM),sensor lag 0.5 s,yes (msiv_cmd false),operator SetMsiv open after signal clears,none,close all MSIVs; IC auto-initiation C02,isolation_l2_closes_both_lines
I02,isolation,low main steam line pressure,p_chest (steam header),RUN,5.9,MPa,assumed (MSIV_LOW_PRESSURE_RUN),0,yes,as I01,bypassed outside RUN,close all MSIVs; S02 follows,isolation_low_pressure_armed_in_run
I03,isolation,high steam flow,total steam flow,all,1.4,x rated steam flow,assumed,0,yes,as I01,none,close all MSIVs,isolation_high_steam_flow
C01,IC auto-initiation,reactor high pressure,sensed dome pressure,all,7.60,MPa,assumed (IC_HIGH_PRESSURE),sensor lag 0.2 s,no (re-initiates while signal present),operator SetIcTrain close only after signal clears,none,all available IC trains cmd_open,ic_reinitiates_while_signal_persists
C02,IC auto-initiation,low-low level L2,sensed WR level,all,-400,cm,assumed (L2_CM),sensor lag 0.5 s,as C01,as C01,none,all IC trains open,ic_init_on_l2
C03,IC auto-initiation,MSIV closure at power,all MSIV pos <0.1 while vessel saturated and previously open,all,0.1,fraction open,assumed,0,one-shot per closure,n/a,none,all IC trains open,ic_init_on_isolation_at_power
T01,turbine trip,high water level L8,sensed NR level,all,100,cm,assumed (L8_CM),sensor lag 0.5 s,yes (turbine.tripped),ResetTurbine when level < L8 and vacuum ok,none,TSV/TCV close; breaker opens on reverse power,l8_trips_turbine
T02,feed pump trip,high water level L8,sensed NR level,all,100,cm,assumed (L8_CM),sensor lag 0.5 s,no (pumps may be restarted),operator SetFeedPump,none,all feed pumps stop; independent of turbine state,l8_trips_feed_pumps_when_turbine_already_tripped
T03,turbine trip,low condenser vacuum,condenser pressure,all,25,kPa,assumed (CONDENSER_TRIP_P),0,yes,ResetTurbine when pressure < 25 kPa,none,turbine trip,turbine_trip_low_vacuum
T04,turbine trip,overspeed,shaft speed,all,1980,rpm,assumed (OVERSPEED_TRIP_RPM),0,yes,ResetTurbine,none,turbine trip,turbine_trip_overspeed
T05,breaker open,reverse power,p_elec < 0,breaker closed,3,s,assumed,3,no,CloseBreaker,none,breaker opens,breaker_opens_on_reverse_power
T06,generator lockout,out-of-phase synchronisation,phase and slip at CloseBreaker,breaker open,15 deg or 0.25 Hz,deg / Hz,assumed,0,yes,ResetTurbine,none,turbine trip 'Out-of-phase synchronisation',breaker_close_out_of_phase_locks_out
T07,permissive,breaker close speed window,shaft speed,breaker open,20,rpm from 1800,assumed,0,no,n/a,none,CloseBreaker refused with event,breaker_close_speed_window
R01,select rod insert,generator offline at power,breaker/trip transition with kin.n > 0.4 and not scrammed,all,0.4,fraction of rated,assumed (legacy SRI concept),0,one-shot,n/a,none,SRI group hydraulically inserted,sri_on_load_rejection
B01,rod withdrawal block,scram not reset,scrammed latch,all,n/a,n/a,design intent,0,while condition,ResetScram,none,withdraw refused; motor targets clipped to next notch,block_scram_not_reset
B02,rod withdrawal block,mode switch SHUTDOWN,mode,SHUTDOWN,n/a,n/a,assumed (legacy),0,while condition,mode change,none,withdraw refused,block_shutdown_mode
B03,rod withdrawal block,REFUEL one-rod interlock,count of rods with pos>0.01 or commanded target>0.01,REFUEL,1,rods,assumed (legacy),0,while condition,insert other rod fully,none,second rod refused with event rod_block,refuel_interlock_multi_index / refuel_interlock_successive
B04,rod withdrawal block,IRM upscale,IRM reading,STARTUP,108,% of range,assumed,0,while condition,range up or power down,none,withdraw refused,block_irm_upscale
B05,rod withdrawal block,APRM setdown upscale,kin.n,STARTUP,0.12,fraction,assumed,0,while condition,power down or mode RUN,none,withdraw refused,block_aprm_setdown_upscale
B06,rod withdrawal block,APRM upscale,kin.n,RUN,1.08,fraction,assumed,0,while condition,power down,none,withdraw refused,block_aprm_upscale_run
P01,permissive,scram reset time delay,time since scram,all,10,s,assumed,10,n/a,n/a,none,ResetScram refused with event scram_reset,scram_reset_time_delay
P02,permissive,scram reset signal clear,any S01..S09 active,all,n/a,n/a,design intent,0,n/a,n/a,none,ResetScram refused naming the signal,scram_reset_blocked_by_signal
P03,permissive,MSIV open,any I01..I03 active,all,n/a,n/a,design intent,0,n/a,n/a,none,SetMsiv open refused with event msiv,msiv_open_blocked_while_signal
P04,interlock,feed pump start,any condensate pump running,all,1,pumps,assumed,0,n/a,n/a,none,SetFeedPump on refused with event fw,feed_pump_start_requires_condensate
P05,permissive,turbine reset,sensed NR level < L8,all,100,cm,assumed,0,n/a,n/a,none,ResetTurbine refused,turbine_reset_blocked_high_level
P06,permissive,turbine reset,condenser pressure < 25 kPa,all,25,kPa,assumed,0,n/a,n/a,none,ResetTurbine refused,turbine_reset_blocked_low_vacuum
P07,feed pump trip,low suction pressure,no condensate pump or hotwell < 5 %,all,3,s,assumed,3,no,operator restart,none,all feed pumps stop; event fw_trip,feed_pump_low_suction_trip
P08,setpoint clamp,command range limits,command argument,all,"p 0.2..7.4 MPa; level -100..50 cm; RWCU 0..15 kg/s; speed 0..1836 rpm; load 0..110 %; IRM 1..10; sim speed 0.1..100",various,assumed,0,n/a,n/a,none,argument clamped silently,cmd_boundary_* tests
F01,fuel envelope flag,fuel/clad temperature beyond model envelope,core.fuel_t / core.clad_t,all,see P5,degC,per Package 5,0,yes,LoadScenario,none,fuelEnvelopeExceeded + alarm fuel_envelope,fuel_envelope_alarm (P5)
A01,alarm,REACTOR SCRAM,scrammed latch,all,n/a,n/a,design intent,0,while condition,ResetScram,ack,Trip annunciator,alarm_scram
A02,alarm,FUEL THERMAL MODEL ENVELOPE EXCEEDED,fuel_envelope_exceeded,all,n/a,n/a,per Package 5,0,while condition,LoadScenario,ack,Trip annunciator,alarm_fuel_envelope
A03,alarm,VESSEL ABOVE DESIGN PRESSURE,sensed pressure,all,8.62,MPa,assumed (VESSEL_DESIGN_PRESSURE),0,while condition,pressure falls,ack,Trip annunciator,alarm_overpressure
A04,alarm,APRM HIGH,kin.n,all,1.05,fraction,assumed,0,while condition,power falls,ack,Alarm,alarm_aprm_hi
A05,alarm,ROD WITHDRAWAL BLOCK,any B01..B06 except in SHUTDOWN or scrammed,STARTUP/REFUEL/RUN,n/a,n/a,design intent,0,while condition,block clears,ack,Warning,alarm_rod_block
A06,alarm,SHORT PERIOD,inverse period,all,20,s,assumed,0,while condition,period lengthens,ack,Alarm,alarm_period
A07,alarm,IRM UPSCALE,IRM reading,not RUN,108,% of range,assumed,0,while condition,range up,ack,Warning,alarm_irm_up
A08,alarm,REACTOR WATER LEVEL HIGH,sensed NR level,all,50,cm (L7),assumed (L7_CM),0,while condition,level falls,ack,Alarm,alarm_lvl_hi
A09,alarm,REACTOR WATER LEVEL LOW,sensed NR level,all,-50,cm (L4),assumed (L4_CM),0,while condition,level rises,ack,Alarm,alarm_lvl_lo
A10,alarm,REACTOR WATER LEVEL LOW-LOW,sensed WR level,all,-400,cm (L2),assumed (L2_CM),0,while condition,level rises,ack,Trip,alarm_lvl_lolo
A11,alarm,CORE UNCOVERED,model core_coverage (mixture level),all,0.999,fraction covered,model-derived,0,while condition,level rises,ack,Trip,alarm_core_uncovered
A12,alarm,REACTOR PRESSURE HIGH,sensed pressure,all,7.35,MPa,assumed,0,while condition,pressure falls,ack,Alarm,alarm_p_hi
A13,alarm,HEATUP/COOLDOWN RATE,model heatup_rate,all,55,degC/h,assumed (typical tech-spec value),0,while condition,rate falls,ack,Warning,alarm_heatup
A14,alarm,HEAD VENT OPEN ABOVE 0.2 MPa,vent state and sensed pressure,all,0.2,MPa,assumed,0,while condition,vent closed,ack,Warning,alarm_vent
A15,alarm,REACTOR VESSEL WATER SOLID,eos.regime == WaterSolid,all,n/a,n/a,model-derived,0,while condition,steam space forms,ack,Alarm,alarm_solid
A16,alarm,MAIN STEAM ISOLATED,MSIV average < 0.9 while saturated,all,0.9,fraction open,assumed,0,while condition,MSIVs reopened,ack,Alarm,alarm_msiv
A17,alarm,ISOLATION CONDENSER IN SERVICE,any train in_service(),all,0.05,return valve fraction,model-derived,0,while condition,valves closed,ack,Warning,alarm_ic
A18,alarm,IC POOL LEVEL LOW,pool_level_pct,all,50,%,assumed,0,while condition,n/a (no makeup modelled),ack,Alarm,alarm_ic_pool
A19,alarm,TURBINE TRIP,turbine.tripped and speed > 100 rpm,all,100,rpm,assumed,0,while condition,ResetTurbine,ack,Alarm,alarm_turb_trip
A20,alarm,CONDENSER VACUUM LOW,condenser pressure,turbine not tripped,15,kPa,assumed,0,while condition,pressure falls,ack,Alarm,alarm_vac
A21,alarm,TURBINE BYPASS UNAVAILABLE,available_for_bypass false while saturated and p > 1 MPa,all,30,kPa condenser,assumed (CONDENSER_BYPASS_BLOCK_P),0,while condition,vacuum restored,ack,Warning,alarm_byp_unavail
A22,alarm,FEEDWATER TEMPERATURE LOW,t_feed with kin.n > 0.3,all,150,degC,assumed,0,while condition,heating restored,ack,Warning,alarm_fw_temp
A23,alarm,HOTWELL LEVEL LOW,hotwell_mass,all,30,% of capacity,assumed,0,while condition,makeup,ack,Warning,alarm_hotwell
A24,alarm,FUEL TEMPERATURE HIGH,core.fuel_t,all,1000,degC,assumed (indication only),0,while condition,cools,ack,Alarm,alarm_fuel_hot
A25,alarm,MODEL ENVELOPE EXCEEDED,domain_errors non-empty,all,n/a,n/a,per Package 3,0,yes,LoadScenario,ack,Trip,alarm_unsupported
```

- [ ] **Step 2: Append ESBWR applicability rows**

Append to `docs/model/esbwr-applicability.csv` (columns per Package 1: `esbwr_source,section,target_subsystem,bwrx_evidence,differences,adaptation,limits,test,classification`):

```csv
ESBWR DCD Rev.10 Tier 2 Ch.7,7.2 Reactor Trip System measurement principles and permissives,protection,NEDO-33912-A Rev.2 (reactivity control) and GDA Ch.7 to be cited by executor,BWRX has no voting architecture modelled; signals are single-channel in this engine,single-channel logic retained; matrix rows mark threshold_source=assumed,no claim of 2-out-of-4 or divisional logic,tests/protection.rs,shared concept
ESBWR DCD Rev.10 Tier 2 Ch.7,7.2 mode switch and rod block permissives,protection,none confirmed for BWRX,BWRX startup instrumentation arrangement unverified (Task 2),legacy SRM/IRM/APRM kept with ASSUMED label,no period trip without source,period_trip_disabled_by_default,provisional numerical estimate
ESBWR DCD Rev.10 Tier 2 Ch.7,7.3 sensor response time and range,instruments,none confirmed for BWRX,ranges and lags are generic,LagSensor with assumed tau and ranges (A-803),sensor failure modes limited to out-of-range flag,sensor_lag_delays_l3_scram,adapted model
```

- [ ] **Step 3: Append assumptions**

Append to `docs/model/assumptions.md`:

```markdown
## Package 8

- **A-801** All protection setpoints in `params.rs` (`SCRAM_HIGH_PRESSURE`, `IC_HIGH_PRESSURE`, `APRM_HIGH_RUN`, `APRM_HIGH_SETDOWN`, `IRM_HIGH_HIGH`, `MSIV_LOW_PRESSURE_RUN`, `L1_CM`..`L8_CM`, `CONDENSER_TRIP_P`, `OVERSPEED_TRIP_RPM`) are engineering assumptions in the style of legacy BWR technical specifications. They are not BWRX-300 values. Each has `threshold_source = assumed` in `docs/model/protection.csv` until Package 1 supplies a public source.
- **A-802** Protection is single-channel. No divisional, 2-out-of-4, or voting architecture is modelled or claimed.
- **A-803** Sensor dynamics: narrow-range level −150..+150 cm with 0.5 s first-order lag; wide-range level −1200..+200 cm with 0.5 s lag; dome pressure 0..10 MPa with 0.2 s lag. Values are typical instrument-class assumptions, not BWRX data.
- **A-804** The mode switch in SHUTDOWN is treated as a persistent scram signal (reset blocked until the switch is moved), matching legacy BWR practice. BWRX mode-switch behaviour is unverified.
```

- [ ] **Step 4: Commit**

```bash
git add docs/model/protection.csv docs/model/esbwr-applicability.csv docs/model/assumptions.md
git commit -m "docs(model): protection cause-and-effect matrix and Package 8 assumptions

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 2: Startup instrumentation verification and guarded period trip

**Files:**
- Modify: `crates/bwr-core/src/params.rs` (append after the protection setpoints block)
- Modify: `crates/bwr-core/src/sim.rs:scram_signals`
- Modify: `docs/model/parameters.csv`, `docs/model/coverage.md`
- Test: `crates/bwr-core/tests/protection.rs`

**Interfaces:**
- Produces: `pub const PERIOD_TRIP_ENABLED: bool = false;` and `pub const PERIOD_TRIP_S: f64 = 10.0;` in `params.rs`. The scram signal text is `"Short period"`.

- [ ] **Step 1: Write the failing test**

Append to `crates/bwr-core/tests/protection.rs`:

```rust
// matrix: S09
#[test]
fn period_trip_disabled_by_default() {
    assert!(!PERIOD_TRIP_ENABLED, "period trip must stay disabled until a public source is recorded");
    // A 5 s positive period in STARTUP must not scram while the trip is disabled.
    let mut sim = Sim::new(ScenarioName::HotStandby);
    sim.core.kin.inv_period = 1.0 / 5.0;
    sim.core.kin.n = 1e-4;
    sim.step(DT);
    assert!(!sim.scrammed, "scrammed: {:?}", sim.scram_reason);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --release -p bwr-core --test protection period_trip_disabled_by_default`
Expected: FAIL with `cannot find value PERIOD_TRIP_ENABLED`.

- [ ] **Step 3: Add the constants and the guarded signal**

In `params.rs`, after `pub const L1_CM: f64 = -600.0;`:

```rust
/// Short-period scram. UNAVAILABLE: no public BWRX-300 source identifies a
/// period-based trip, so the signal is compiled out. Flip only with a source
/// recorded in docs/model/sources.csv and a parameters.csv row.
pub const PERIOD_TRIP_ENABLED: bool = false;
/// Period below which the trip would act when enabled, s. ASSUMED placeholder.
pub const PERIOD_TRIP_S: f64 = 10.0;
```

In `sim.rs` `scram_signals`, inside the `_ =>` (non-RUN) arm after the IRM check:

```rust
                if PERIOD_TRIP_ENABLED && n > 1e-7 && self.core.kin.inv_period > 1.0 / PERIOD_TRIP_S {
                    v.push("Short period");
                }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --release -p bwr-core --test protection period_trip_disabled_by_default`
Expected: `test period_trip_disabled_by_default ... ok`

- [ ] **Step 5: Record the instrumentation status**

Append to `docs/model/parameters.csv` (Package 1 columns `id,name,file,value,unit,definition,reference_state,source_design,status,source_id,sensitivity`):

```csv
P-801,PERIOD_TRIP_ENABLED,crates/bwr-core/src/params.rs,false,bool,compile-time guard for a period scram,n/a,none,unavailable,,n/a
P-802,PERIOD_TRIP_S,crates/bwr-core/src/params.rs,10,s,period threshold if enabled,n/a,none,assumed,,5..20 s
P-803,irm_full_scale,crates/bwr-core/src/sim.rs,0.4*10^((range-10)/2),fraction of rated,IRM range full scale; 10 ranges half-decade,n/a,legacy BWR,assumed,,n/a
P-804,srm_cps,crates/bwr-core/src/sim.rs,min(n*2e10+3; 1e6),cps,SRM indication formula,n/a,none,ui-indication assumed,,n/a
```

Append to `docs/model/coverage.md` under "Protection and instruments":

```markdown
| Startup neutron instrumentation | SRM / IRM (10 half-decade ranges) / APRM is the legacy BWR arrangement and is ASSUMED. The executor must check NEDO-33912-A Rev. 2 and GE UK GDA Chapter 7 for the BWRX-300 startup-range arrangement and record the finding in sources.csv. No period-based trip is enabled (P-801). | A-801, P-801..P-804 | period_trip_disabled_by_default |
```

- [ ] **Step 6: Run the full suite and commit**

Run: `cargo test --release -p bwr-core`
Expected: all tests pass.

```bash
git add crates/bwr-core/src/params.rs crates/bwr-core/src/sim.rs crates/bwr-core/tests/protection.rs docs/model/parameters.csv docs/model/coverage.md
git commit -m "feat(protection): guarded period trip constant; record startup instrumentation status

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---
