# Package 8: Protection, Modes, Instruments, and Operator Controls — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Names and signatures follow `docs/superpowers/plans/2026-10-20-refinement-master.md`; the master wins on any conflict.

**Goal:** Give every implemented trip, isolation, permissive, block, reset, and alarm a sourced or explicitly assumed row in a cause-and-effect matrix, make protection act on sensed (lagged, ranged) values instead of model truth, fix the REFUEL one-rod interlock and the L8 feed-pump trip, verify reset and latch behaviour, and make displays say whether a number is actual, commanded, sensed, or model-calculated.

**Architecture:** Protection logic stays in `crates/bwr-core/src/sim.rs` but reads only `Sim.sensors` (a new `sensors.rs` module with first-order-lag, ranged sensors). The matrix in `docs/model/protection.csv` is the specification; `tests/protection.rs` has one positive, one negative, one boundary, and one transition test per row and per `Command`. The UI gains one small `ValueTag` component that suffixes readouts with ACT / CMD / SENSED / MODEL.

**Tech Stack:** Rust 2021, serde, TypeScript 5.6, React 18. No new dependencies.

**Prerequisites:** Packages 1–7 complete, master plan r2. This plan assumes: P2 `Sim::finalize_init()`, `Sim.log`, `Sim.faults: Vec<Fault>` with `set_fault`/`find_fault`, `tests/common/mod.rs` (`run`, `assert_finite`), `validation::run_case` with trace schema v2; P3 `Sim::step` order (`core.step_rods` first), `DomainError`, `Sim.domain_errors`; P5 `RodMotion { Idle, Motor { target }, Scram, Sri, MotorRunIn }`, `Rods { motion, stuck }`, `Rods::target(i)` (commanded endpoint; 0.0 for insertion modes), `Rods::targets()`, `Rods::set_motor_target(i, t)`, `Rods::scram_hydraulic()`, `Rods::motor_run_in()`, `Rods::scramming()`; `Snapshot.fuel_envelope_exceeded`; P6 `SteamSystem { tsv_pos, line: SteamLine { p, .. } }`, `SteamSystem::vessel_steam_flow()`, `RWCU_REJECT_MAX_KGS`, `SRI_TRIGGER_POWER_FRAC`; P7 `IcTrain { available, in_service(), cmd_open, ... }`.

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
docs/model/assumptions.md                 modify  add A-801..A-805 (Tasks 1, 9)
docs/model/parameters.csv                 modify  period trip, sensor, ARI constants (Tasks 2, 7, 9)
docs/model/esbwr-applicability.csv        modify  DCD Ch. 7 rows (Task 1)
crates/bwr-core/src/params.rs             modify  PERIOD_TRIP_*, ARI_*, sensor ranges and taus
crates/bwr-core/src/sensors.rs            create  LagSensor, SensorTruth, Sensors (five instruments)
crates/bwr-core/src/bop.rs                modify  Condenser::available_for_bypass(p_sensed)
crates/bwr-core/src/validation.rs         modify  sensed/model trace columns (schema v2 reserved slots)
crates/bwr-core/src/lib.rs                modify  pub mod sensors
crates/bwr-core/src/sim.rs                modify  scram_signals, withdraw_block, apply, protection, snapshot, finalize_init, sensors step (position 13), ARI
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
docs/validation/baseline/*.csv            regenerate (Task 12)
```

Test helper used throughout `tests/protection.rs`:

```rust
use bwr_core::api::{Command, ModeSwitch, ScenarioName};
use bwr_core::bop::LevelControlMode;
use bwr_core::params::*;
use bwr_core::sim::DT;
use bwr_core::Sim;

mod common;
use common::run; // shared helper from P2: steps `sim` by DT for `seconds`

/// True if any active alarm id equals `id`.
fn has_alarm(sim: &Sim, id: &str) -> bool {
    sim.snapshot().alarms.iter().any(|a| a.id == id)
}

/// True if the append-only log contains an entry starting with `prefix`.
/// Log lines follow the master's locked `"<key>: <detail>"` convention
/// (keys: scram, turbine trip, msiv isolation, ic auto-initiation, feed pump trip,
/// domain error, rod block, sri, ari, breaker open, command ignored, overload, ic model, fault);
/// Metrics.first_event_time keys on the text before the first ':'.
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
S01,scram,APRM high neutron flux,direct model input kin.n (APRM: fast instrument; lag below the 0.05 s physics step; assumed),RUN,1.18,fraction of rated,assumed (APRM_HIGH_RUN),0,yes,signal clear AND >=10 s since scram AND operator ResetScram,none,hydraulic scram all rods + motor run-in; scrammed latch,scram_aprm_high_run
S02,scram,MSIV closure,direct model input: average MSIV position over both lines (position switches; no lag modelled; assumed),RUN,0.85,fraction open (below),assumed (legacy BWR rule applied to a 2-line plant: one fully closed line gives average 0.5 and scrams; BWRX per-line logic unverified),0,yes,as S01,none,scram; the open line is not isolated (no isolation signal),single_msiv_closure_scrams_in_run_without_isolating_other_line
S03,scram,APRM high flux setdown,direct model input kin.n (APRM; assumed),SHUTDOWN/REFUEL/STARTUP,0.15,fraction of rated,assumed (APRM_HIGH_SETDOWN),0,yes,as S01,none,scram,run_to_startup_at_power_scrams_on_setdown
S04,scram,IRM high-high,direct model input kin.n / irm_full_scale (IRM; assumed),SHUTDOWN/REFUEL/STARTUP,120,% of range,assumed (IRM_HIGH_HIGH),0,yes,as S01,bypassed in RUN,scram,cmd_set_irm_range_transition
S05,scram,reactor high pressure,sensors.pressure (dome pressure instrument),all,7.50,MPa,assumed (SCRAM_HIGH_PRESSURE),sensor lag 0.2 s,yes,as S01,none,scram,scram_high_pressure
S06,scram,low water level L3,sensors.level_nr,all,-100,cm vs NWL,assumed (L3_CM),sensor lag 0.5 s,yes,as S01,none,scram,sensor_lag_delays_l3_scram / failed_level_sensor_blinds_l3_scram
S07,scram,manual scram,operator Command::ManualScram,all,n/a,n/a,design intent,0,yes,as S01,none,scram,cmd_manual_scram_positive
S08,scram,mode switch in SHUTDOWN,mode switch position (discrete; no lag),transition to SHUTDOWN and while in SHUTDOWN,n/a,n/a,assumed (legacy BWR; A-804),0,yes,mode switch moved out of SHUTDOWN AND as S01,none,scram; persistent signal blocks reset,scram_reset_blocked_in_shutdown
S09,scram,short period,direct model input kin.inv_period (period meter; assumed),STARTUP (if enabled),10,s,unavailable (PERIOD_TRIP_ENABLED=false),0,yes,as S01,disabled by constant,scram when enabled,period_trip_disabled_by_default
I01,isolation,low-low water level L2,sensors.level_wr,all,-400,cm vs NWL,assumed (L2_CM),sensor lag 0.5 s,yes (msiv_cmd false),operator SetMsiv open after signal clears,none,close all MSIVs; IC auto-initiation C02,isolation_l2_closes_both_lines_and_blocks_reopen
I02,isolation,low main steam line pressure,sensors.header_pressure (steam line node steam.line.p),RUN,5.9,MPa,assumed (MSIV_LOW_PRESSURE_RUN),sensor lag 0.2 s,yes,as I01,bypassed outside RUN,close all MSIVs; S02 follows,isolation_low_pressure_armed_in_run / isolation_uses_header_sensor_and_turbine_trip_uses_condenser_sensor
I03,isolation,high steam flow,direct model input steam.vessel_steam_flow() (flow instruments fast; lag below physics step; assumed),all,1.4,x rated steam flow,assumed,0,yes,as I01,none,close all MSIVs,isolation_high_steam_flow
C01,IC auto-initiation,reactor high pressure,sensors.pressure,all,7.60,MPa,assumed (IC_HIGH_PRESSURE),sensor lag 0.2 s,no (re-initiates while signal present),operator SetIcTrain close only after signal clears,none,all available IC trains cmd_open,ic_reinitiates_while_signal_persists / ic_stays_closed_after_signal_clears
C02,IC auto-initiation,low-low level L2,sensors.level_wr,all,-400,cm,assumed (L2_CM),sensor lag 0.5 s,as C01,as C01,none,all IC trains open,ic_init_on_l2
C03,IC auto-initiation,MSIV closure at power,direct model input: all MSIV pos <0.1 while vessel saturated and previously open (position switches; assumed),all,0.1,fraction open,assumed,0,one-shot per closure,n/a,none,all IC trains open,ic_init_on_isolation_at_power
T01,turbine trip,high water level L8,sensors.level_nr,all,100,cm,assumed (L8_CM),sensor lag 0.5 s,yes (turbine.tripped),ResetTurbine when sensed NR level < L8 and sensed condenser pressure ok,none,TSV/TCV close; breaker opens on reverse power,l8_trips_turbine
T02,feed pump trip,high water level L8,sensors.level_nr,all,100,cm,assumed (L8_CM),sensor lag 0.5 s,no (pumps may be restarted),operator SetFeedPump,none,all feed pumps stop; independent of turbine state,l8_trips_feed_pumps_when_turbine_already_tripped
T03,turbine trip,low condenser vacuum,sensors.condenser_pressure,all,25,kPa,assumed (CONDENSER_TRIP_P),sensor lag 0.2 s,yes,ResetTurbine when sensed pressure < 25 kPa,none,turbine trip,cmd_set_cw_pump_boundary / isolation_uses_header_sensor_and_turbine_trip_uses_condenser_sensor
T04,turbine trip,overspeed,direct model input turbine.speed_rpm (speed probe; no lag modelled; assumed),all,1980,rpm,assumed (OVERSPEED_TRIP_RPM),0,yes,ResetTurbine,none,turbine trip,cmd_open_breaker_boundary
T05,breaker open,reverse power,direct model input p_elec < 0 (electrical relay; assumed),breaker closed,3,s,assumed,3,no,CloseBreaker,none,breaker opens,cmd_trip_turbine_positive
T06,generator lockout,out-of-phase synchronisation,direct model input phase and slip at CloseBreaker (synchroscope; assumed),breaker open,15 deg or 0.25 Hz,deg / Hz,assumed,0,yes,ResetTurbine,none,turbine trip 'Out-of-phase synchronisation',cmd_close_breaker_transition
T07,permissive,breaker close speed window,direct model input turbine.speed_rpm (assumed),breaker open,20,rpm from 1800,assumed,0,no,n/a,none,CloseBreaker refused with event,cmd_close_breaker_negative
R01,select rod insert,generator offline at power,direct model input: breaker/trip transition with kin.n > 0.4 and not scrammed (assumed),all,0.4,fraction of rated,assumed (legacy SRI concept; SRI_TRIGGER_POWER_FRAC),0,one-shot,n/a,none,SRI group hydraulically inserted (RodMotion::Sri),cmd_open_breaker_transition
B01,rod withdrawal block,scram not reset,scrammed latch,all,n/a,n/a,design intent,0,while condition,ResetScram,none,withdraw refused; motor targets clipped to next notch,cmd_rod_set_target_transition
B02,rod withdrawal block,mode switch SHUTDOWN,mode,SHUTDOWN,n/a,n/a,assumed (legacy),0,while condition,mode change,none,withdraw refused,block_shutdown_mode
B03,rod withdrawal block,REFUEL one-rod interlock,count over the prospective set of rods with pos>0.01 or Rods::target(i)>0.01,REFUEL,1,rods,assumed (legacy),0,while condition,insert other rod fully,none,second rod refused with event rod_block,refuel_interlock_multi_index / refuel_interlock_successive / refuel_interlock_after_first_fully_out
B04,rod withdrawal block,IRM upscale,direct model input kin.n / irm_full_scale (assumed),STARTUP,108,% of range,assumed,0,while condition,range up or power down,none,withdraw refused,block_irm_upscale
B05,rod withdrawal block,APRM setdown upscale,direct model input kin.n (assumed),STARTUP,0.12,fraction,assumed,0,while condition,power down or mode RUN,none,withdraw refused,block_aprm_setdown_upscale
B06,rod withdrawal block,APRM upscale,direct model input kin.n (assumed),RUN,1.08,fraction,assumed,0,while condition,power down,none,withdraw refused,block_aprm_upscale_run
P01,permissive,scram reset time delay,time since scram,all,10,s,assumed,10,n/a,n/a,none,ResetScram refused with event scram_reset,scram_reset_time_delay
P02,permissive,scram reset signal clear,any S01..S09 active,all,n/a,n/a,design intent,0,n/a,n/a,none,ResetScram refused naming the signal,scram_reset_blocked_by_signal
P03,permissive,MSIV open,any I01..I03 active,all,n/a,n/a,design intent,0,n/a,n/a,none,SetMsiv open refused with event msiv,isolation_l2_closes_both_lines_and_blocks_reopen
P04,interlock,feed pump start,any condensate pump running (discrete),all,1,pumps,assumed,0,n/a,n/a,none,SetFeedPump on refused with event fw,cmd_set_feed_pump_negative
P05,permissive,turbine reset,sensors.level_nr < L8,all,100,cm,assumed,0,n/a,n/a,none,ResetTurbine refused,cmd_reset_turbine_transition
P06,permissive,turbine reset,sensors.condenser_pressure < 25 kPa,all,25,kPa,assumed,0,n/a,n/a,none,ResetTurbine refused,cmd_reset_turbine_negative
P07,feed pump trip,low suction pressure,direct model input: no condensate pump or hotwell < 5 % (pressure switch; assumed),all,3,s,assumed,3,no,operator restart,none,all feed pumps stop; event fw_trip,cmd_set_feed_pump_transition
P08,setpoint clamp,command range limits,command argument,all,"p 0.2..7.4 MPa; level -100..50 cm; RWCU 0..RWCU_REJECT_MAX_KGS; speed 0..1836 rpm; load 0..110 %; IRM 1..10; sim speed 0.1..100",various,assumed,0,n/a,n/a,none,argument clamped silently,cmd_*_boundary tests
F01,fuel envelope flag,fuel/clad temperature beyond model envelope,direct model input core.fuel_t / core.clad_t (calculated quantity),all,see P5 FUEL_MODEL_T_MAX_C,degC,per Package 5,0,yes,LoadScenario,none,fuelEnvelopeExceeded + alarm fuel_envelope,fuel_envelope_alarm (P5)
A01,alarm,REACTOR SCRAM,scrammed latch,all,n/a,n/a,design intent,0,while condition,ResetScram,ack,Trip annunciator,cmd_ack_alarms_positive
A02,alarm,FUEL THERMAL MODEL ENVELOPE EXCEEDED,fuel_envelope_exceeded (model flag),all,n/a,n/a,per Package 5,0,while condition,LoadScenario,ack,Trip annunciator,fuel_envelope_alarm (P5)
A03,alarm,VESSEL ABOVE DESIGN PRESSURE,sensors.pressure,all,8.62,MPa,assumed (VESSEL_DESIGN_PRESSURE),0,while condition,pressure falls,ack,Trip annunciator,scenario tests (P9)
A04,alarm,APRM HIGH,direct model input kin.n (assumed),all,1.05,fraction,assumed,0,while condition,power falls,ack,Alarm,scenario tests (P9)
A05,alarm,ROD WITHDRAWAL BLOCK,any B01..B06 except in SHUTDOWN or scrammed,STARTUP/REFUEL/RUN,n/a,n/a,design intent,0,while condition,block clears,ack,Warning,refuel_with_two_rods_out_blocks
A06,alarm,SHORT PERIOD,direct model input kin.inv_period (assumed),all,20,s,assumed,0,while condition,period lengthens,ack,Alarm,scenario tests (P9)
A07,alarm,IRM UPSCALE,direct model input kin.n / irm_full_scale (assumed),not RUN,108,% of range,assumed,0,while condition,range up,ack,Warning,scenario tests (P9)
A08,alarm,REACTOR WATER LEVEL HIGH,sensors.level_nr,all,50,cm (L7),assumed (L7_CM),0,while condition,level falls,ack,Alarm,cmd_set_fcv_manual_transition
A09,alarm,REACTOR WATER LEVEL LOW,sensors.level_nr,all,-50,cm (L4),assumed (L4_CM),0,while condition,level rises,ack,Alarm,sensor_lag_delays_l3_scram
A10,alarm,REACTOR WATER LEVEL LOW-LOW,sensors.level_wr,all,-400,cm (L2),assumed (L2_CM),0,while condition,level rises,ack,Trip,ic_init_on_l2
A11,alarm,CORE UNCOVERED,model core_coverage (mixture level; calculated quantity),all,0.999,fraction covered,model-derived,0,while condition,level rises,ack,Trip,scenario tests (P9)
A12,alarm,REACTOR PRESSURE HIGH,sensors.pressure,all,7.35,MPa,assumed,0,while condition,pressure falls,ack,Alarm,cmd_set_pressure_setpoint_positive
A13,alarm,HEATUP/COOLDOWN RATE,model heatup_rate (calculated quantity),all,55,degC/h,assumed (typical tech-spec value),0,while condition,rate falls,ack,Warning,scenario tests (P9)
A14,alarm,HEAD VENT OPEN ABOVE 0.2 MPa,vent state and sensors.pressure,all,0.2,MPa,assumed,0,while condition,vent closed,ack,Warning,cmd_set_head_vent_negative
A15,alarm,REACTOR VESSEL WATER SOLID,eos.regime == WaterSolid (model),all,n/a,n/a,model-derived,0,while condition,steam space forms,ack,Alarm,vessel_transients (P4)
A16,alarm,MAIN STEAM ISOLATED,direct model input: MSIV average < 0.9 while saturated (position switches; assumed),all,0.9,fraction open,assumed,0,while condition,MSIVs reopened,ack,Alarm,isolation_l2_closes_both_lines_and_blocks_reopen
A17,alarm,ISOLATION CONDENSER IN SERVICE,any train in_service() (valve position switches),all,n/a,n/a,model-derived,0,while condition,valves closed,ack,Warning,cmd_ack_alarms_transition
A18,alarm,IC POOL LEVEL LOW,pool_level_pct (model),all,50,%,assumed,0,while condition,n/a (no makeup modelled),ack,Alarm,ic_transients (P7)
A19,alarm,TURBINE TRIP,turbine.tripped and speed > 100 rpm (direct model input),all,100,rpm,assumed,0,while condition,ResetTurbine,ack,Alarm,cmd_trip_turbine_positive
A20,alarm,CONDENSER VACUUM LOW,sensors.condenser_pressure,turbine not tripped,15,kPa,assumed,0,while condition,pressure falls,ack,Alarm,cmd_set_vacuum_pump_transition
A21,alarm,TURBINE BYPASS UNAVAILABLE,Condenser::available_for_bypass(sensors.condenser_pressure) false while saturated and sensed p > 1 MPa,all,30,kPa condenser,assumed (CONDENSER_BYPASS_BLOCK_P),0,while condition,vacuum restored,ack,Warning,cmd_set_bypass_negative
A22,alarm,FEEDWATER TEMPERATURE LOW,t_feed with kin.n > 0.3 (model),all,150,degC,assumed,0,while condition,heating restored,ack,Warning,cmd_set_heaters_positive
A23,alarm,HOTWELL LEVEL LOW,hotwell_mass (model),all,30,% of capacity,assumed,0,while condition,makeup,ack,Warning,power_conversion (P6)
A24,alarm,FUEL TEMPERATURE HIGH,core.fuel_t (calculated quantity),all,1000,degC,assumed (indication only),0,while condition,cools,ack,Alarm,core_transients (P5)
A25,alarm,MODEL ENVELOPE EXCEEDED,domain_errors non-empty (model flag),all,n/a,n/a,per Package 3,0,yes,LoadScenario,ack,Trip,numerics (P3)
```

- [ ] **Step 2: Append ESBWR applicability rows**

Append to `docs/model/esbwr-applicability.csv` (locked header `id,esbwr_source,esbwr_section,target_subsystem,bwrx_evidence,differences,adaptation,applicability_limits,test,classification`; continue the `E-nnn` id sequence from the last existing row):

```csv
E-801,ESBWR DCD Rev.10 Tier 2,7.2 Reactor Trip System measurement principles and permissives,protection,NEDO-33912-A Rev.2 (reactivity control) and GDA Ch.7 to be cited by executor,BWRX has no voting architecture modelled; signals are single-channel in this engine,single-channel logic retained; matrix rows mark threshold_source=assumed,no claim of 2-out-of-4 or divisional logic,tests/protection.rs,shared concept
E-802,ESBWR DCD Rev.10 Tier 2,7.2 mode switch and rod block permissives,protection,none confirmed for BWRX,BWRX startup instrumentation arrangement unverified (Task 2),legacy SRM/IRM/APRM kept with ASSUMED label,no period trip without source,period_trip_disabled_by_default,provisional numerical estimate
E-803,ESBWR DCD Rev.10 Tier 2,7.3 sensor response time and range,instruments,none confirmed for BWRX,ranges and lags are generic,LagSensor with assumed tau and ranges (A-803),sensor failure limited to a frozen reading and out-of-range flag,sensor_lag_delays_l3_scram,adapted model
E-804,ESBWR DCD Rev.10 Tier 2,7.4 alternate rod insertion as a diverse path,protection,none confirmed for BWRX,signals and setpoints unknown,compiled out behind ARI_ENABLED (A-805),no automatic ARI claimed,ari_disabled_by_default,inapplicable until sourced
```

- [ ] **Step 3: Append assumptions**

Append to `docs/model/assumptions.md`:

```markdown
## Package 8

- **A-801** All protection setpoints in `params.rs` (`SCRAM_HIGH_PRESSURE`, `IC_HIGH_PRESSURE`, `APRM_HIGH_RUN`, `APRM_HIGH_SETDOWN`, `IRM_HIGH_HIGH`, `MSIV_LOW_PRESSURE_RUN`, `L1_CM`..`L8_CM`, `CONDENSER_TRIP_P`, `OVERSPEED_TRIP_RPM`) are engineering assumptions in the style of legacy BWR technical specifications. They are not BWRX-300 values. Each has `threshold_source = assumed` in `docs/model/protection.csv` until Package 1 supplies a public source.
- **A-802** Protection is single-channel. No divisional, 2-out-of-4, or voting architecture is modelled or claimed.
- **A-803** Sensor dynamics: narrow-range level −150..+150 cm with 0.5 s first-order lag; wide-range level −1200..+200 cm with 0.5 s lag; dome pressure 0..10 MPa with 0.2 s lag. Values are typical instrument-class assumptions, not BWRX data.
- **A-804** The mode switch in SHUTDOWN is treated as a persistent scram signal (reset blocked until the switch is moved), matching legacy BWR practice. BWRX mode-switch behaviour is unverified.
- **A-806** MSIV-closure scram (S02) uses the average position of the two lines against 0.85, so one fully closed line scrams while the open line is not isolated. The legacy multi-line (3-of-4) logic does not map onto a two-line plant and BWRX-300 per-line closure logic is unverified; this rule is an assumption until sourced.
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

Append to `docs/model/parameters.csv` (locked header `id,value,unit,definition,reference_state,source_design,status,source_ids,interval_lo,interval_hi,assumption_id,location,notes`; `id` is the constant name):

```csv
PERIOD_TRIP_ENABLED,false,bool,compile-time guard for a period scram,n/a,none,unavailable,,,,A-801,crates/bwr-core/src/params.rs,flip only with a source
PERIOD_TRIP_S,10,s,period threshold if enabled,n/a,none,assumed,,5,20,A-801,crates/bwr-core/src/params.rs,
irm_full_scale,0.4*10^((range-10)/2),fraction of rated,IRM range full scale; 10 half-decade ranges,n/a,legacy BWR,assumed,,,,A-801,crates/bwr-core/src/sim.rs,legacy arrangement; BWRX startup instrumentation unverified
srm_cps,min(n*2e10+3; 1e6),cps,SRM indication formula,n/a,none,assumed,,,,A-801,crates/bwr-core/src/sim.rs,ui-indication only; not used by protection
```

Append to `docs/model/coverage.md` under "Protection and instruments":

```markdown
| Startup neutron instrumentation | SRM / IRM (10 half-decade ranges) / APRM is the legacy BWR arrangement and is ASSUMED. The executor must check NEDO-33912-A Rev. 2 and GE UK GDA Chapter 7 for the BWRX-300 startup-range arrangement and record the finding in sources.csv. No period-based trip is enabled (`PERIOD_TRIP_ENABLED`). Neutron power rows in protection.csv are direct model inputs: the instruments are fast and their lag is below the 0.05 s physics step (assumed). | A-801; parameters PERIOD_TRIP_ENABLED, PERIOD_TRIP_S, irm_full_scale, srm_cps | period_trip_disabled_by_default |
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

### Task 3: SHUTDOWN mode as a persistent scram signal; scram reset latch tests

**Files:**
- Modify: `crates/bwr-core/src/sim.rs:scram_signals`, `apply` (`ResetScram`, `SetModeSwitch`)
- Test: `crates/bwr-core/tests/protection.rs`

**Interfaces:**
- Produces: scram signal text `"Mode switch in SHUTDOWN"` returned by `scram_signals()` whenever `self.mode == ModeSwitch::Shutdown`. The `SetModeSwitch` arm no longer calls `self.scram(..)` directly; `protection()` picks the signal up on the next step (same step as the command because `apply` is between steps).

- [ ] **Step 1: Write the failing tests**

```rust
// matrix: S08, P02
#[test]
fn scram_reset_blocked_in_shutdown() {
    let mut sim = Sim::new(ScenarioName::HotStandby);
    sim.apply(Command::SetModeSwitch { mode: ModeSwitch::Shutdown });
    sim.step(DT);
    assert!(sim.scrammed);
    run(&mut sim, 15.0);
    sim.apply(Command::ResetScram);
    assert!(sim.scrammed, "reset must be blocked while mode switch is in SHUTDOWN");
    assert!(has_alarm(&sim, "scram_reset"));
    sim.apply(Command::SetModeSwitch { mode: ModeSwitch::Startup });
    sim.step(DT);
    sim.apply(Command::ResetScram);
    assert!(!sim.scrammed, "reset allowed once the mode switch leaves SHUTDOWN");
}

// matrix: P01
#[test]
fn scram_reset_time_delay() {
    let mut sim = Sim::new(ScenarioName::HotStandby);
    sim.apply(Command::ManualScram);
    run(&mut sim, 5.0);
    sim.apply(Command::ResetScram);
    assert!(sim.scrammed, "reset within 10 s must be refused");
    run(&mut sim, 6.0);
    sim.apply(Command::ResetScram);
    assert!(!sim.scrammed);
}

// matrix: P02
#[test]
fn scram_reset_blocked_by_signal() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 2.0);
    sim.apply(Command::SetMsiv { line: 0, open: false });
    sim.apply(Command::SetMsiv { line: 1, open: false });
    run(&mut sim, 20.0);
    assert!(sim.scrammed);
    sim.apply(Command::ResetScram);
    assert!(sim.scrammed, "MSIV closure signal persists; reset must be refused");
    assert!(has_alarm(&sim, "scram_reset"));
}
```

- [ ] **Step 2: Run tests to verify the first fails**

Run: `cargo test --release -p bwr-core --test protection scram_reset`
Expected: `scram_reset_blocked_in_shutdown` FAILS at "reset must be blocked while mode switch is in SHUTDOWN" (baseline allows it because the SHUTDOWN scram is not in `scram_signals`). The other two pass at baseline and stay as regression evidence.

- [ ] **Step 3: Implement**

In `scram_signals`, before `match self.mode`:

```rust
        if self.mode == ModeSwitch::Shutdown {
            v.push("Mode switch in SHUTDOWN");
        }
```

Replace the `SetModeSwitch` arm in `apply`:

```rust
            Command::SetModeSwitch { mode } => {
                self.mode = mode;
                // SHUTDOWN is a persistent scram signal (matrix S08); protection() acts on it.
            }
```

In `active_alarms`, the rod-block warning already suppresses SHUTDOWN; no change.

- [ ] **Step 4: Run tests and the whole suite**

Run: `cargo test --release -p bwr-core`
Expected: all pass, including `manual_scram_shuts_down_without_isolation` in `plant.rs` (it switches to SHUTDOWN after a manual scram; the reason stays "Manual scram" because `scram()` keeps the first reason).

- [ ] **Step 5: Commit**

```bash
git add crates/bwr-core/src/sim.rs crates/bwr-core/tests/protection.rs
git commit -m "fix(protection): SHUTDOWN mode is a persistent scram signal; reset latch tests

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 4: REFUEL one-rod interlock using commanded targets

**Files:**
- Modify: `crates/bwr-core/src/sim.rs:withdraw_block`, `apply` (`RodSetTarget`)
- Test: `crates/bwr-core/tests/protection.rs`

**Interfaces:**
- Consumes: P5 `Rods::target(i) -> f64`, `Rods::set_motor_target(i, t)`, `RodMotion`.
- Produces: `fn rods_out_or_commanded(&self) -> usize` on `Sim` (private), counting rods with `pos > 0.01 || target(i) > 0.01`.

- [ ] **Step 1: Write the failing tests**

```rust
fn refuel_sim() -> Sim {
    let mut sim = Sim::new(ScenarioName::ColdShutdown);
    sim.apply(Command::SetModeSwitch { mode: ModeSwitch::Refuel });
    sim.step(DT);
    assert!(!sim.scrammed);
    sim
}

// matrix: B03 (negative, multi-index)
#[test]
fn refuel_interlock_multi_index() {
    let mut sim = refuel_sim();
    sim.apply(Command::RodSetTarget { indices: vec![0, 1], target: 2.0 });
    run(&mut sim, 10.0);
    assert!(sim.core.rods.pos[0] > 1.9, "first rod moves");
    assert!(sim.core.rods.pos[1] < 0.01, "second rod in the same command must be blocked");
    assert!(logged(&sim, "rod block: REFUEL"));
}

// matrix: B03 (negative, successive commands before motion)
#[test]
fn refuel_interlock_successive() {
    let mut sim = refuel_sim();
    sim.apply(Command::RodSetTarget { indices: vec![0], target: 2.0 });
    sim.apply(Command::RodSetTarget { indices: vec![1], target: 2.0 });
    run(&mut sim, 10.0);
    assert!(sim.core.rods.pos[0] > 1.9);
    assert!(sim.core.rods.pos[1] < 0.01, "second command before motion must be blocked");
}

// matrix: B03 (negative, defect 7 off-by-one: first rod fully withdrawn, second rod commanded)
#[test]
fn refuel_interlock_after_first_fully_out() {
    let mut sim = refuel_sim();
    sim.apply(Command::RodSetTarget { indices: vec![0], target: ROD_FULL_OUT });
    run(&mut sim, ROD_FULL_OUT / ROD_SPEED + 5.0);
    assert!(sim.core.rods.pos[0] > ROD_FULL_OUT - 0.1, "first rod fully out");
    sim.apply(Command::RodSetTarget { indices: vec![1], target: 2.0 });
    run(&mut sim, 10.0);
    assert!(sim.core.rods.pos[1] < 0.01, "baseline `out > 1` lets a second rod out once the first is at 48; must be blocked");
    assert!(logged(&sim, "rod block: REFUEL"));
}

// matrix: B03 (positive and transition)
#[test]
fn refuel_interlock_allows_next_rod_after_full_insert() {
    let mut sim = refuel_sim();
    sim.apply(Command::RodSetTarget { indices: vec![0], target: 4.0 });
    run(&mut sim, 15.0);
    assert!(sim.core.rods.pos[0] > 3.9);
    sim.apply(Command::RodSetTarget { indices: vec![0], target: 0.0 });
    run(&mut sim, 15.0);
    assert!(sim.core.rods.pos[0] < 0.01);
    sim.apply(Command::RodSetTarget { indices: vec![1], target: 2.0 });
    run(&mut sim, 10.0);
    assert!(sim.core.rods.pos[1] > 1.9, "a different rod may be withdrawn once the first is fully in");
}

// matrix: B03 (boundary: insertion of the withdrawn rod is never blocked)
#[test]
fn refuel_interlock_never_blocks_insertion() {
    let mut sim = refuel_sim();
    sim.apply(Command::RodSetTarget { indices: vec![0], target: 4.0 });
    run(&mut sim, 15.0);
    sim.apply(Command::RodSetTarget { indices: vec![0, 1], target: 0.0 });
    run(&mut sim, 15.0);
    assert!(sim.core.rods.pos[0] < 0.01);
    assert!(!logged(&sim, "rod block: REFUEL"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --release -p bwr-core --test protection refuel_interlock`
Expected: `refuel_interlock_multi_index`, `refuel_interlock_successive` and `refuel_interlock_after_first_fully_out` FAIL at "must be blocked"; the other two pass.

- [ ] **Step 3: Implement**

Add to `impl Sim`:

```rust
    /// Rods other than `except` that are out or commanded out: pos > 0.01 or
    /// Rods::target(i) > 0.01 (Motor targets count; Scram/Sri/MotorRunIn targets are 0.0).
    /// The REFUEL interlock evaluates this over the prospective set, i.e. after
    /// every rod already accepted in the same command has had its target set.
    fn other_rods_out_or_commanded(&self, except: usize) -> usize {
        (0..N_RODS)
            .filter(|&i| i != except)
            .filter(|&i| self.core.rods.pos[i] > 0.01 || self.core.rods.target(i) > 0.01)
            .count()
    }
```

In `withdraw_block`, replace the `Refuel` arm (used by the alarm A05 and by `protection()` to clip motor targets):

```rust
            ModeSwitch::Refuel => {
                let out = (0..N_RODS)
                    .filter(|&i| self.core.rods.pos[i] > 0.01 || self.core.rods.target(i) > 0.01)
                    .count();
                if out > 1 {
                    return Some("REFUEL mode: one rod withdrawn");
                }
            }
```

Replace the `RodSetTarget` arm so the block is evaluated per rod against the prospective set (defect 7: multi-index, successive commands, and the `out > 1` off-by-one):

```rust
            Command::RodSetTarget { indices, target } => {
                let target = ((target / 2.0).round() * 2.0).clamp(0.0, ROD_FULL_OUT);
                for i in indices.into_iter().filter(|&i| i < N_RODS) {
                    let withdrawing = target > self.core.rods.pos[i] + 1e-6;
                    if withdrawing {
                        let mut block = self.withdraw_block();
                        if block.is_none()
                            && self.mode == ModeSwitch::Refuel
                            && self.other_rods_out_or_commanded(i) >= 1
                        {
                            block = Some("REFUEL mode: one rod withdrawn");
                        }
                        if let Some(b) = block {
                            self.event("rod_block", format!("Rod withdrawal blocked: {b}"));
                            self.log.push((self.time, format!("rod block: {b}")));
                            continue;
                        }
                    }
                    if !self.core.rods.scramming() {
                        self.core.rods.set_motor_target(i, target);
                    }
                }
            }
```

- [ ] **Step 4: Run tests**

Run: `cargo test --release -p bwr-core`
Expected: all pass (`cold_shutdown_is_subcritical_and_rods_blocked` unaffected).

- [ ] **Step 5: Commit**

```bash
git add crates/bwr-core/src/sim.rs crates/bwr-core/tests/protection.rs
git commit -m "fix(protection): REFUEL one-rod interlock counts commanded rods and evaluates per rod

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 5: L8 feed-pump trip independent of turbine state

**Files:**
- Modify: `crates/bwr-core/src/sim.rs:protection`
- Test: `crates/bwr-core/tests/protection.rs`

**Interfaces:**
- Produces: log entries `"L8 high level: feed pumps tripped"` and (turbine) `"L8 high level: turbine tripped"`; event id `l8` unchanged.

- [ ] **Step 1: Write the failing test**

```rust
// matrix: T02 (negative at baseline: turbine already tripped)
#[test]
fn l8_trips_feed_pumps_when_turbine_already_tripped() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 2.0);
    sim.apply(Command::TripTurbine);
    sim.apply(Command::SetLevelControl { mode: LevelControlMode::Manual });
    sim.apply(Command::SetFcvManual { pos: 1.0 });
    let mut reached_l8 = false;
    for _ in 0..(300.0 / DT) as usize {
        sim.step(DT);
        if sim.vessel.level_nr_cm() > L8_CM + 5.0 {
            reached_l8 = true;
        }
        if reached_l8 && sim.vessel.level_nr_cm() > L8_CM + 20.0 {
            break;
        }
    }
    assert!(reached_l8, "test setup: level never reached L8");
    assert!(sim.fw.running_pumps() == 0, "feed pumps must trip on L8 even with the turbine already tripped");
    assert!(logged(&sim, "feed pump trip: L8 high level"));
}

// matrix: T01, T02 (positive)
#[test]
fn l8_trips_turbine() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 2.0);
    sim.apply(Command::SetLevelControl { mode: LevelControlMode::Manual });
    sim.apply(Command::SetFcvManual { pos: 1.0 });
    run(&mut sim, 300.0);
    assert!(sim.turbine.tripped);
    assert_eq!(sim.turbine.trip_reason.as_deref(), Some("Reactor high water level (L8)"));
    assert_eq!(sim.fw.running_pumps(), 0);
}
```

- [ ] **Step 2: Run tests to verify the first fails**

Run: `cargo test --release -p bwr-core --test protection l8_`
Expected: `l8_trips_feed_pumps_when_turbine_already_tripped` FAILS at "feed pumps must trip"; `l8_trips_turbine` passes.

- [ ] **Step 3: Implement**

In `protection()`, replace the "Turbine trips." block:

```rust
        // High water level L8: turbine trip and feed pump trip are independent functions (matrix T01, T02).
        let l8 = self.sensed_level_nr() > L8_CM;
        if l8 && self.fw.running_pumps() > 0 {
            self.fw.pumps = [false; N_FEED_PUMPS];
            self.event("l8", "L8 high level: feed pumps tripped");
            self.log.push((self.time, "feed pump trip: L8 high level".into()));
        }
        if !self.turbine.tripped {
            if l8 {
                self.turbine.trip("Reactor high water level (L8)");
                self.log.push((self.time, "turbine trip: L8 high level".into()));
            } else if self.cond.pressure() > CONDENSER_TRIP_P {
                self.turbine.trip("Low condenser vacuum");
                self.log.push((self.time, "turbine trip: low condenser vacuum".into()));
            }
        }
```

Until Task 7 adds sensors, define `fn sensed_level_nr(&self) -> f64 { self.vessel.level_nr_cm() }` on `Sim`; Task 7 replaces its body.

- [ ] **Step 4: Run the suite**

Run: `cargo test --release -p bwr-core`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/bwr-core/src/sim.rs crates/bwr-core/tests/protection.rs
git commit -m "fix(protection): L8 feed-pump trip no longer depends on turbine trip state

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 6: IC re-initiation and per-line MSIV behaviour

**MSIV single-line rule (one sentence, matrix S02, assumed):** in RUN the MSIV-closure scram acts when the average MSIV position over the two lines falls below 0.85, so a single fully closed line (average 0.5) scrams while the other line stays open and no isolation signal is generated; BWRX-300 per-line closure logic is unverified, so this is recorded as `assumed` until Package 1 supplies a source.

**Files:**
- Modify: `crates/bwr-core/src/sim.rs:protection`
- Test: `crates/bwr-core/tests/protection.rs`

**Interfaces:**
- Consumes: P7 `IcTrain.cmd_open`, `IcTrain.available`.
- Produces: `fn ic_initiation_signals(&self) -> Vec<&'static str>` on `Sim` (private) returning `"Reactor high pressure"`, `"Low-low water level (L2)"`; the one-shot isolation-at-power trigger stays separate.

- [ ] **Step 1: Write the failing tests**

```rust
// matrix: C01 (re-initiation while signal persists)
#[test]
fn ic_reinitiates_while_signal_persists() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 2.0);
    sim.apply(Command::SetMsiv { line: 0, open: false });
    sim.apply(Command::SetMsiv { line: 1, open: false });
    run(&mut sim, 10.0);
    assert!(sim.ic.iter().all(|t| t.cmd_open), "ICs auto-initiated");
    // Operator closes train 0 while pressure is still above IC_HIGH_PRESSURE.
    let t_hi = sim.vessel.eos.p > IC_HIGH_PRESSURE;
    sim.apply(Command::SetIcTrain { index: 0, open: false });
    sim.step(DT);
    if t_hi {
        assert!(sim.ic[0].cmd_open, "persisting high-pressure signal must re-open the train");
    }
}

// matrix: C01 (stays closed once signal clears)
#[test]
fn ic_stays_closed_after_signal_clears() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 2.0);
    sim.apply(Command::SetMsiv { line: 0, open: false });
    sim.apply(Command::SetMsiv { line: 1, open: false });
    run(&mut sim, 600.0);
    assert!(sim.vessel.eos.p < IC_HIGH_PRESSURE && sim.vessel.level_nr_cm() > L2_CM, "signals cleared");
    sim.apply(Command::SetIcTrain { index: 0, open: false });
    run(&mut sim, 5.0);
    assert!(!sim.ic[0].cmd_open);
}

// matrix: C02
#[test]
fn ic_init_on_l2() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 2.0);
    sim.apply(Command::SetLevelControl { mode: LevelControlMode::Manual });
    sim.apply(Command::SetFcvManual { pos: 0.0 });
    for _ in 0..(900.0 / DT) as usize {
        sim.step(DT);
        if sim.ic.iter().all(|t| t.cmd_open) {
            break;
        }
    }
    assert!(sim.ic.iter().all(|t| t.cmd_open), "ICs must initiate on L2");
    assert!(sim.vessel.level_nr_cm() < L2_CM + 50.0);
}

// matrix: C03
#[test]
fn ic_init_on_isolation_at_power() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 2.0);
    sim.apply(Command::SetMsiv { line: 0, open: false });
    sim.apply(Command::SetMsiv { line: 1, open: false });
    run(&mut sim, MSIV_STROKE + 1.0);
    assert!(sim.ic.iter().all(|t| t.cmd_open));
    assert!(logged(&sim, "ic auto-initiation"));
}

// matrix: S02 (single-line closure: scram on the 0.85 average rule; no isolation of the other line)
#[test]
fn single_msiv_closure_scrams_in_run_without_isolating_other_line() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 2.0);
    sim.apply(Command::SetMsiv { line: 0, open: false });
    run(&mut sim, MSIV_STROKE + 2.0);
    assert!(sim.steam.msiv_pos[0] < 0.01, "line 0 closed");
    assert!(sim.steam.msiv_pos[1] > 0.99 && sim.steam.msiv_cmd[1], "line 1 stays open: single-line closure is not an isolation signal");
    assert_eq!(sim.scram_reason.as_deref(), Some("MSIV closure"), "average position 0.5 < 0.85 scrams (assumed rule)");
    // STARTUP: S02 is not armed, so the same action does not scram.
    let mut sim = Sim::new(ScenarioName::HotStandby);
    sim.apply(Command::SetMsiv { line: 0, open: false });
    run(&mut sim, MSIV_STROKE + 2.0);
    assert!(!sim.scrammed, "scrammed: {:?}", sim.scram_reason);
}

// matrix: I01, P03
#[test]
fn isolation_l2_closes_both_lines_and_blocks_reopen() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 2.0);
    sim.apply(Command::SetLevelControl { mode: LevelControlMode::Manual });
    sim.apply(Command::SetFcvManual { pos: 0.0 });
    for _ in 0..(900.0 / DT) as usize {
        sim.step(DT);
        if !sim.steam.msiv_cmd.iter().any(|&c| c) {
            break;
        }
    }
    assert!(sim.steam.msiv_cmd.iter().all(|&c| !c), "both lines commanded closed on L2");
    sim.apply(Command::SetMsiv { line: 0, open: true });
    assert!(!sim.steam.msiv_cmd[0], "reopen blocked while L2 persists");
    assert!(has_alarm(&sim, "msiv"));
}
```

- [ ] **Step 2: Run tests to verify re-initiation fails**

Run: `cargo test --release -p bwr-core --test protection ic_`
Expected: `ic_reinitiates_while_signal_persists` FAILS (baseline only opens trains when `any(!cmd_open)` and the operator close is honoured once per step — verify by reading the failure; if the baseline happens to re-open because the `any` check fires every step, the test passes and is kept as regression evidence). The other IC tests pass or fail according to P7 behaviour; fix P7 regressions before continuing.

- [ ] **Step 3: Implement**

Add to `impl Sim`:

```rust
    fn ic_initiation_signals(&self) -> Vec<&'static str> {
        let mut v = Vec::new();
        if self.sensed_pressure() > IC_HIGH_PRESSURE {
            v.push("Reactor high pressure");
        }
        if self.sensed_level_wr() < L2_CM {
            v.push("Low-low water level (L2)");
        }
        v
    }
```

Until Task 7: `fn sensed_pressure(&self) -> f64 { self.vessel.eos.p }` and `fn sensed_level_wr(&self) -> f64 { self.vessel.level_nr_cm() }`.

Replace the IC auto-initiation block in `protection()`:

```rust
        // Isolation condenser auto-initiation (matrix C01-C03). Signals re-initiate every step
        // while present; the isolation-at-power trigger is one-shot per closure.
        let ic_sigs = self.ic_initiation_signals();
        if !ic_sigs.is_empty() || isolated_at_power {
            let mut opened = false;
            for t in self.ic.iter_mut().filter(|t| t.available && !t.cmd_open) {
                t.cmd_open = true;
                opened = true;
            }
            if opened {
                let why = ic_sigs.first().copied().unwrap_or("MSIV closure at power");
                self.event("ic_init", format!("Isolation condensers auto-initiated: {why}"));
                self.log.push((self.time, format!("ic auto-initiation: {why}")));
            }
        }
```

- [ ] **Step 4: Run the suite**

Run: `cargo test --release -p bwr-core`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/bwr-core/src/sim.rs crates/bwr-core/tests/protection.rs
git commit -m "fix(protection): IC auto-initiation re-asserts while signals persist; per-line MSIV tests

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 7: Sensors module; protection, level control, and displays read sensed values

**Files:**
- Create: `crates/bwr-core/src/sensors.rs`
- Modify: `crates/bwr-core/src/lib.rs` (`pub mod sensors;`), `params.rs`, `sim.rs` (struct, `finalize_init`, `step` position 13, `sensed_*`, `active_alarms`, `snapshot`), `api.rs`, `bop.rs` (`Condenser::available_for_bypass` takes the sensed pressure)
- Modify: `ui/src/sim/types.ts`, `ui/src/sim/mock.ts`
- Modify: `docs/model/parameters.csv`
- Test: `crates/bwr-core/src/sensors.rs` (unit), `crates/bwr-core/tests/protection.rs`

**Interfaces:**
- Produces (master r2 contract): `LagSensor { tau_s, lo, hi, state, reading, failed }` deriving `Clone, Copy, Debug`; `LagSensor::new(tau_s, lo, hi, initial)`, `step(dt, truth) -> f64`, `out_of_range() -> bool` (`state < lo || state > hi`), `fail()`; `Sensors { level_nr, level_wr, pressure, header_pressure, condenser_pressure, last_error }` deriving `Clone, Debug` (NOT `Copy`: `last_error: Option<DomainError>` holds a `String`; master amendment, see the note after Step 3); `Sensors::from_truth(sim: &Sim) -> Sensors`; `Sensors::step(&mut self, dt, truth: &SensorTruth)`; `SensorTruth { level_nr_cm, level_wr_cm, pressure_pa, header_pressure_pa, condenser_pressure_pa }` deriving `Clone, Copy, Debug`; `Sim.sensors: Sensors`; `Sim::sensor_truth(&self) -> SensorTruth` (`pub(crate)`: called from `sensors.rs`, a different module); `Sim::sensed_level_nr()`, `sensed_level_wr()`, `sensed_pressure()`, `sensed_header_pressure()`, `sensed_condenser_pressure()` (private).
- Signature change owned by this package: `Condenser::available_for_bypass(&self, p_sensed: f64) -> bool` (was `&self` only); the body compares `p_sensed` with `CONDENSER_BYPASS_BLOCK_P` and keeps the CW-pump condition. `Sim` is the only caller.
- Snapshot fields: `vessel.level_nr_cm` (sensed NR), `level_wr_cm`, `level_model_cm`, `level_nr_out_of_range`, `pressure_sensed_mpa`.
- Constants in `params.rs`: `LEVEL_NR_RANGE_CM: (f64, f64) = (-150.0, 150.0)`, `LEVEL_WR_RANGE_CM: (f64, f64) = (-1200.0, 200.0)`, `PRESSURE_SENSOR_RANGE_PA: (f64, f64) = (0.0, 10.0e6)`, `HEADER_PRESSURE_SENSOR_RANGE_PA: (f64, f64) = (0.0, 10.0e6)`, `CONDENSER_PRESSURE_SENSOR_RANGE_PA: (f64, f64) = (0.0, 120.0e3)`, `LEVEL_SENSOR_TAU_S: f64 = 0.5`, `PRESSURE_SENSOR_TAU_S: f64 = 0.2`.
- dt rule (master): `Sensors::step` is a public void-returning step function, so it guards `dt` and every truth value with P6's `crate::bop::check_step_args` (`pub(crate)`, not re-invented) as its first statement, sets `Sensors.last_error` on failure and changes nothing; `Sim::step` drains it at step 17. `LagSensor::step` (also public) validates the same way and returns the unchanged `reading` on failure; it has no error channel, `Sensors` reports for it.
- Truth sources (step order position 13): `level_nr_cm = level_wr_cm = vessel.level_nr_cm()`, `pressure_pa = vessel.eos.p`, `header_pressure_pa = steam.line.p` (P6 steam-line node), `condenser_pressure_pa = cond.pressure()`.

- [ ] **Step 1: Write the failing unit tests in `sensors.rs`**

Create `crates/bwr-core/src/sensors.rs`:

```rust
//! Measured plant variables: first-order lag plus instrument range. Protection,
//! the level controller and displays read these, never the model state
//! directly (assumption A-803; measurement column of docs/model/protection.csv).

use crate::bop::check_step_args;
use crate::domain::DomainError;
use crate::params::*;
use crate::sim::Sim;

#[derive(Clone, Copy, Debug)]
pub struct LagSensor {
    pub tau_s: f64,
    pub lo: f64,
    pub hi: f64,
    /// Lagged, UNCLAMPED signal.
    pub state: f64,
    /// `state` clamped to [lo, hi]; frozen when `failed`.
    pub reading: f64,
    pub failed: bool,
}

impl LagSensor {
    pub fn new(tau_s: f64, lo: f64, hi: f64, initial: f64) -> Self {
        Self { tau_s, lo, hi, state: initial, reading: initial.clamp(lo, hi), failed: false }
    }

    /// First-order lag on `state`; `reading` is the clamped state. A failed sensor
    /// holds its last reading and stops tracking.
    pub fn step(&mut self, dt: f64, truth: f64) -> f64 {
        // Master dt rule / non-finite inputs rule: no state change on an invalid argument.
        // The owning `Sensors::step` reports the error; this leaf has no error channel.
        if !(dt.is_finite() && dt > 0.0) || !truth.is_finite() || self.failed {
            return self.reading;
        }
        let a = if self.tau_s <= 0.0 { 1.0 } else { (dt / self.tau_s).min(1.0) };
        self.state += (truth - self.state) * a;
        self.reading = self.state.clamp(self.lo, self.hi);
        self.reading
    }

    /// True when the lagged signal lies outside the instrument span. A value
    /// exactly on a rail is in range.
    pub fn out_of_range(&self) -> bool {
        self.state < self.lo || self.state > self.hi
    }

    /// Freeze the reading at its current value.
    pub fn fail(&mut self) {
        self.failed = true;
    }
}

/// Model truths sampled once per step for the instruments.
#[derive(Clone, Copy, Debug)]
pub struct SensorTruth {
    pub level_nr_cm: f64,
    pub level_wr_cm: f64,
    pub pressure_pa: f64,
    pub header_pressure_pa: f64,
    pub condenser_pressure_pa: f64,
}

#[derive(Clone, Debug)]
pub struct Sensors {
    pub level_nr: LagSensor,
    pub level_wr: LagSensor,
    pub pressure: LagSensor,
    pub header_pressure: LagSensor,
    pub condenser_pressure: LagSensor,
    /// Error channel of the void-returning `step` (master dt rule); drained by `Sim::step` at step 17.
    pub last_error: Option<DomainError>,
}

impl Sensors {
    fn from_values(t: &SensorTruth) -> Self {
        Self {
            level_nr: LagSensor::new(LEVEL_SENSOR_TAU_S, LEVEL_NR_RANGE_CM.0, LEVEL_NR_RANGE_CM.1, t.level_nr_cm),
            level_wr: LagSensor::new(LEVEL_SENSOR_TAU_S, LEVEL_WR_RANGE_CM.0, LEVEL_WR_RANGE_CM.1, t.level_wr_cm),
            pressure: LagSensor::new(PRESSURE_SENSOR_TAU_S, PRESSURE_SENSOR_RANGE_PA.0, PRESSURE_SENSOR_RANGE_PA.1, t.pressure_pa),
            header_pressure: LagSensor::new(
                PRESSURE_SENSOR_TAU_S,
                HEADER_PRESSURE_SENSOR_RANGE_PA.0,
                HEADER_PRESSURE_SENSOR_RANGE_PA.1,
                t.header_pressure_pa,
            ),
            condenser_pressure: LagSensor::new(
                PRESSURE_SENSOR_TAU_S,
                CONDENSER_PRESSURE_SENSOR_RANGE_PA.0,
                CONDENSER_PRESSURE_SENSOR_RANGE_PA.1,
                t.condenser_pressure_pa,
            ),
            last_error: None,
        }
    }

    /// Instruments initialised at the model truth (no lag transient at scenario start).
    pub fn from_truth(sim: &Sim) -> Self {
        Self::from_values(&sim.sensor_truth())
    }

    pub fn step(&mut self, dt: f64, t: &SensorTruth) {
        // Master dt rule, via P6's shared guard (first statement; nothing changes on Err).
        if let Err(e) = check_step_args(
            dt,
            &[
                ("level_nr_cm", t.level_nr_cm),
                ("level_wr_cm", t.level_wr_cm),
                ("pressure_pa", t.pressure_pa),
                ("header_pressure_pa", t.header_pressure_pa),
                ("condenser_pressure_pa", t.condenser_pressure_pa),
            ],
        ) {
            self.last_error = Some(e);
            return;
        }
        self.level_nr.step(dt, t.level_nr_cm);
        self.level_wr.step(dt, t.level_wr_cm);
        self.pressure.step(dt, t.pressure_pa);
        self.header_pressure.step(dt, t.header_pressure_pa);
        self.condenser_pressure.step(dt, t.condenser_pressure_pa);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lag_reaches_63_percent_after_one_tau() {
        let mut s = LagSensor::new(1.0, -1e9, 1e9, 0.0);
        for _ in 0..1000 {
            s.step(0.001, 1.0);
        }
        assert!((s.reading - 0.632).abs() < 0.01, "reading {}", s.reading);
    }

    #[test]
    fn clamps_reading_but_state_keeps_tracking() {
        let mut s = LagSensor::new(0.1, -150.0, 150.0, 0.0);
        for _ in 0..100 {
            s.step(0.05, -400.0);
        }
        assert_eq!(s.reading, -150.0);
        assert!((s.state + 400.0).abs() < 1.0, "state {}", s.state);
        assert!(s.out_of_range());
    }

    #[test]
    fn exact_rail_value_is_in_range() {
        let mut s = LagSensor::new(0.0, -150.0, 150.0, 0.0);
        s.step(1.0, -150.0);
        assert_eq!(s.reading, -150.0);
        assert!(!s.out_of_range());
        s.step(1.0, -150.0001);
        assert!(s.out_of_range());
    }

    #[test]
    fn recovers_after_returning_in_range() {
        let mut s = LagSensor::new(0.1, -150.0, 150.0, 0.0);
        for _ in 0..100 {
            s.step(0.05, -400.0);
        }
        assert!(s.out_of_range());
        for _ in 0..100 {
            s.step(0.05, -100.0);
        }
        assert!(!s.out_of_range());
        assert!((s.reading + 100.0).abs() < 1.0);
    }

    #[test]
    fn failed_sensor_holds_reading() {
        let mut s = LagSensor::new(0.1, 0.0, 10.0, 5.0);
        s.fail();
        s.step(1.0, 9.0);
        assert_eq!(s.reading, 5.0);
        assert!(s.failed);
    }

    #[test]
    fn step_rejects_invalid_dt_and_truth_unchanged() {
        use crate::domain::DomainError;
        let t = SensorTruth { level_nr_cm: 12.0, level_wr_cm: 12.0, pressure_pa: 7.2e6, header_pressure_pa: 7.0e6, condenser_pressure_pa: 5.6e3 };
        let base = Sensors::from_values(&t);
        for (what, dt, truth) in [("dt", 0.0, t), ("dt", -1.0, t), ("dt", f64::NAN, t), ("level_nr_cm", 0.05, SensorTruth { level_nr_cm: f64::NAN, ..t })] {
            let mut s = base.clone();
            s.step(dt, &truth);
            assert!(matches!(s.last_error.take(), Some(DomainError::PropertyOutOfRange { what: w, .. }) if w == what), "{what}");
            assert_eq!(format!("{s:?}"), format!("{base:?}"), "{what}: state must be unchanged");
        }
        let mut leaf = LagSensor::new(0.1, 0.0, 10.0, 5.0);
        assert_eq!(leaf.step(0.0, 9.0), 5.0);
        assert_eq!(leaf.step(0.05, f64::INFINITY), 5.0);
        assert_eq!(leaf.state, 5.0);
    }

    #[test]
    fn from_values_has_no_initial_transient() {
        let t = SensorTruth { level_nr_cm: 12.0, level_wr_cm: 12.0, pressure_pa: 7.2e6, header_pressure_pa: 7.0e6, condenser_pressure_pa: 5.6e3 };
        let s = Sensors::from_values(&t);
        assert_eq!(s.level_nr.reading, 12.0);
        assert_eq!(s.pressure.reading, 7.2e6);
        assert_eq!(s.header_pressure.reading, 7.0e6);
        assert_eq!(s.condenser_pressure.reading, 5.6e3);
    }
}
```

- [ ] **Step 2: Run to verify compile failure**

Run: `cargo test --release -p bwr-core sensors`
Expected: FAIL to compile: module not registered, constants and `Sim::sensor_truth` missing.

- [ ] **Step 3: Register module and constants**

In `lib.rs` add `pub mod sensors;` (alphabetical, after `reactor`). In `params.rs`, after the `PERIOD_TRIP_S` block:

```rust
// ---------------------------------------------------------------- instruments (ASSUMED, A-803)
/// Narrow-range level instrument span, cm relative to NWL.
pub const LEVEL_NR_RANGE_CM: (f64, f64) = (-150.0, 150.0);
/// Wide-range level instrument span, cm relative to NWL.
pub const LEVEL_WR_RANGE_CM: (f64, f64) = (-1200.0, 200.0);
/// Dome pressure instrument span, Pa.
pub const PRESSURE_SENSOR_RANGE_PA: (f64, f64) = (0.0, 10.0e6);
/// Main steam header pressure instrument span, Pa.
pub const HEADER_PRESSURE_SENSOR_RANGE_PA: (f64, f64) = (0.0, 10.0e6);
/// Condenser pressure instrument span, Pa (absolute).
pub const CONDENSER_PRESSURE_SENSOR_RANGE_PA: (f64, f64) = (0.0, 120.0e3);
/// First-order lag of level instruments, s.
pub const LEVEL_SENSOR_TAU_S: f64 = 0.5;
/// First-order lag of pressure instruments, s.
pub const PRESSURE_SENSOR_TAU_S: f64 = 0.2;
```

In `sim.rs` add the truth sampler next to the `sensed_*` helpers. It must be `pub(crate)`: `Sensors::from_truth` in `sensors.rs` calls it across the module boundary (a private method would not compile). Every field it reads (`vessel`, `steam`, `cond`) is already `pub` on `Sim`; `vessel.level_nr_cm()`, `cond.pressure()` are `pub` methods and `steam.line.p` is a `pub` field (P6).

```rust
    pub(crate) fn sensor_truth(&self) -> SensorTruth {
        SensorTruth {
            level_nr_cm: self.vessel.level_nr_cm(),
            level_wr_cm: self.vessel.level_nr_cm(),
            pressure_pa: self.vessel.eos.p,
            header_pressure_pa: self.steam.line.p,
            condenser_pressure_pa: self.cond.pressure(),
        }
    }
```

Visibility audit (everything crossing a module boundary is explicit):

| Item | Declared in | Used from | Visibility |
|---|---|---|---|
| `LagSensor`, its fields, `new`, `step`, `out_of_range`, `fail` | `sensors.rs` | `sim.rs`, `tests/protection.rs` | `pub` |
| `SensorTruth` and all five fields | `sensors.rs` | `sim.rs` (`sensor_truth`) | `pub` |
| `Sensors` and its six fields (five instruments + `last_error`), `from_truth`, `step`, `from_truth_values_zero` | `sensors.rs` | `sim.rs`, tests | `pub` |
| `bop::check_step_args` | `bop.rs` (P6 Task 2a) | `sensors.rs` (`Sensors::step`) | `pub(crate)` already; `sensors.rs` adds `use crate::bop::check_step_args;` and `use crate::domain::DomainError;` |

**Master amendment (exact text, P8 section):** replace `#[derive(Clone, Copy, Debug)]\npub struct Sensors { pub level_nr: LagSensor, pub level_wr: LagSensor, pub pressure: LagSensor, pub header_pressure: LagSensor, pub condenser_pressure: LagSensor }` with `#[derive(Clone, Debug)]\npub struct Sensors { pub level_nr: LagSensor, pub level_wr: LagSensor, pub pressure: LagSensor, pub header_pressure: LagSensor, pub condenser_pressure: LagSensor, pub last_error: Option<DomainError> }` — the dt rule requires an error channel on a void-returning step, and `DomainError` is not `Copy`. `LagSensor` stays `Clone, Copy, Debug`.
| `Sensors::from_values` | `sensors.rs` | `sensors.rs` only (incl. its unit tests) | private |
| `Sim::sensor_truth` | `sim.rs` | `sensors.rs` | `pub(crate)` |
| `Sim.sensors` | `sim.rs` | `sensors.rs` is not a user; tests read and mutate it | `pub` |
| `Sim::sensed_level_nr/wr/pressure/header_pressure/condenser_pressure` | `sim.rs` | `sim.rs` only | private |
| `Sim.scram_time`, `Sim.log`, `Sim.domain_errors`, `Sim.irm_range`, `Sim.ari_latched` | `sim.rs` | tests | `pub` (`scram_time`/`irm_range` already are; `log`, `domain_errors` from P2/P3; `ari_latched` Task 9) |
| `Sim.ari_l2_timer` | `sim.rs` | `sim.rs` only | private |

`tests/protection.rs` needs no `use bwr_core::sensors::...` line: tests reach the instruments through `sim.sensors.<name>.<field or method>` and never name the types. The one test in Task 12 that uses the validation runner carries its own `use bwr_core::validation::{col_index, run_case, standard_cases};` inside the function body.

Run: `cargo test --release -p bwr-core sensors`
Expected: 7 unit tests pass (the `Sim` field is added in Step 5; `from_truth` compiles because `sensor_truth` exists and is `pub(crate)`).

- [ ] **Step 4: Write the failing integration tests**

```rust
// matrix: S06 (sensor lag)
#[test]
fn sensor_lag_delays_l3_scram() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 2.0);
    sim.apply(Command::SetLevelControl { mode: LevelControlMode::Manual });
    sim.apply(Command::SetFcvManual { pos: 0.0 });
    let mut t_model_cross = None;
    for _ in 0..(900.0 / DT) as usize {
        sim.step(DT);
        if t_model_cross.is_none() && sim.vessel.level_nr_cm() < L3_CM {
            t_model_cross = Some(sim.time);
        }
        if sim.scrammed {
            break;
        }
    }
    let t_model = t_model_cross.expect("level never reached L3");
    assert_eq!(sim.scram_reason.as_deref(), Some("Reactor low water level (L3)"));
    let lag = sim.scram_time - t_model;
    assert!(lag > 0.2 && lag < 1.5, "scram lagged model crossing by {lag} s (tau {LEVEL_SENSOR_TAU_S})");
}

// matrix: A-803 (range: NR rails, WR keeps reading, model value exposed)
#[test]
fn level_nr_out_of_range_flag() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 2.0);
    sim.apply(Command::SetLevelControl { mode: LevelControlMode::Manual });
    sim.apply(Command::SetFcvManual { pos: 0.0 });
    for _ in 0..(900.0 / DT) as usize {
        sim.step(DT);
        if sim.vessel.level_nr_cm() < LEVEL_NR_RANGE_CM.0 - 20.0 {
            break;
        }
    }
    let s = sim.snapshot();
    assert!(s.vessel.level_nr_out_of_range);
    assert_eq!(s.vessel.level_nr_cm, LEVEL_NR_RANGE_CM.0, "reading rails at the span limit");
    assert!(s.vessel.level_wr_cm < LEVEL_NR_RANGE_CM.0 - 10.0, "WR still reads");
    assert!(s.vessel.level_model_cm < LEVEL_NR_RANGE_CM.0 - 10.0);
}

// matrix: A-803 (exact boundary: a level sitting exactly at the rail is in range)
#[test]
fn level_exactly_at_rail_is_in_range() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    sim.sensors.level_nr.state = LEVEL_NR_RANGE_CM.1;
    sim.sensors.level_nr.reading = LEVEL_NR_RANGE_CM.1;
    assert!(!sim.sensors.level_nr.out_of_range());
    assert!(!sim.snapshot().vessel.level_nr_out_of_range);
}

// matrix: A-803 (recovery: flag clears when the model returns inside the span)
#[test]
fn level_nr_out_of_range_recovers() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 2.0);
    sim.apply(Command::SetLevelControl { mode: LevelControlMode::Manual });
    sim.apply(Command::SetFcvManual { pos: 0.0 });
    for _ in 0..(900.0 / DT) as usize {
        sim.step(DT);
        if sim.snapshot().vessel.level_nr_out_of_range {
            break;
        }
    }
    assert!(sim.snapshot().vessel.level_nr_out_of_range, "setup: never went out of range");
    sim.apply(Command::SetFcvManual { pos: 1.0 });
    for _ in 0..(900.0 / DT) as usize {
        sim.step(DT);
        if sim.vessel.level_nr_cm() > LEVEL_NR_RANGE_CM.0 + 20.0 {
            break;
        }
    }
    run(&mut sim, 5.0 * LEVEL_SENSOR_TAU_S);
    assert!(!sim.snapshot().vessel.level_nr_out_of_range);
}

// matrix: S06 (failed sensor: protection sees the frozen reading, not the model)
#[test]
fn failed_level_sensor_blinds_l3_scram() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 2.0);
    sim.sensors.level_nr.fail();
    sim.apply(Command::SetLevelControl { mode: LevelControlMode::Manual });
    sim.apply(Command::SetFcvManual { pos: 0.0 });
    for _ in 0..(900.0 / DT) as usize {
        sim.step(DT);
        if sim.vessel.level_nr_cm() < L3_CM - 50.0 {
            break;
        }
    }
    assert!(sim.vessel.level_nr_cm() < L3_CM - 50.0, "setup: level did not fall");
    assert_ne!(sim.scram_reason.as_deref(), Some("Reactor low water level (L3)"), "frozen NR instrument cannot see L3");
    // The wide-range instrument still works, so L2 isolation remains available.
    for _ in 0..(900.0 / DT) as usize {
        sim.step(DT);
        if !sim.steam.msiv_cmd.iter().any(|&c| c) {
            break;
        }
    }
    assert!(sim.steam.msiv_cmd.iter().all(|&c| !c), "L2 isolation on the WR instrument");
}

// matrix: I02 reads the header instrument; T03/P06 read the condenser instrument
#[test]
fn isolation_uses_header_sensor_and_turbine_trip_uses_condenser_sensor() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 2.0);
    sim.sensors.header_pressure.fail(); // frozen at ~7.0 MPa
    sim.apply(Command::SetPressureSetpoint { mpa: 4.0 });
    run(&mut sim, 300.0);
    assert!(sim.steam.line.p < MSIV_LOW_PRESSURE_RUN, "setup: model header pressure fell");
    assert!(sim.steam.msiv_cmd.iter().all(|&c| c), "frozen header instrument: no low-pressure isolation");

    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 2.0);
    sim.sensors.condenser_pressure.fail();
    sim.apply(Command::SetCwPump { index: 0, on: false });
    sim.apply(Command::SetCwPump { index: 1, on: false });
    run(&mut sim, 600.0);
    assert!(sim.cond.pressure() > CONDENSER_TRIP_P, "setup: model condenser pressure rose");
    assert_ne!(sim.turbine.trip_reason.as_deref(), Some("Low condenser vacuum"), "frozen condenser instrument: no vacuum trip");
}

#[test]
fn sensors_start_at_truth_on_load() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    assert!((sim.sensors.level_nr.reading - sim.vessel.level_nr_cm()).abs() < 1e-9);
    assert!((sim.sensors.header_pressure.reading - sim.steam.line.p).abs() < 1e-6);
    sim.apply(Command::LoadScenario { name: ScenarioName::HotStandby });
    assert!((sim.sensors.pressure.reading - sim.vessel.eos.p).abs() < 1e-6);
    assert!((sim.sensors.condenser_pressure.reading - sim.cond.pressure()).abs() < 1e-6);
}
```

Run: `cargo test --release -p bwr-core --test protection sensor`
Expected: compile failure (`sim.sensors`, `level_nr_out_of_range` missing).

- [ ] **Step 5: Wire sensors into `Sim`**

In `sim.rs`:

```rust
use crate::sensors::{SensorTruth, Sensors};
// struct Sim: add
    pub sensors: Sensors,
```

In `base()` the struct literal needs a value before the plant exists; use a zeroed placeholder and let `finalize_init` overwrite it:

```rust
            sensors: Sensors::from_truth_values_zero(),
```

Add to `sensors.rs` (public, documented as construction-only):

```rust
impl Sensors {
    /// All-zero instruments; only for struct construction before `finalize_init`.
    pub fn from_truth_values_zero() -> Self {
        Self::from_values(&SensorTruth { level_nr_cm: 0.0, level_wr_cm: 0.0, pressure_pa: 0.0, header_pressure_pa: 0.0, condenser_pressure_pa: 0.0 })
    }
}
```

In `finalize_init()` (P2), add as the last statement before `self.time = 0.0` bookkeeping completes:

```rust
        self.sensors = Sensors::from_truth(self);
```

Every scenario constructor already ends with `finalize_init()`, and `LoadScenario` goes through the constructor, so both inherit this.

In `step()`, at position 13 of the master order: after P6's `// --- plant energy balance` block (which follows `self.cond.step(..)`, step 12) and before `self.time += dt` (step 14):

```rust
        // 13. instruments sample the model
        let truth = self.sensor_truth();
        self.sensors.step(dt, &truth);
```

At step 17, next to P3's `ic_errs` drain and P6's BOP `last_error` drains, add `if let Some(e) = self.sensors.last_error.take() { self.domain_error(e); }`. The order of the rest of `Sim::step` is the master's 17-step list as refined by P3 Task 4 and P6 Tasks 2a/7 (`bound_feed_to_hotwell` at step 7, drains at step 17); nothing else moves.

Replace the interim helpers from Tasks 5 and 6 and delete `fn level_cm`:

```rust
    fn sensed_level_nr(&self) -> f64 {
        self.sensors.level_nr.reading
    }
    fn sensed_level_wr(&self) -> f64 {
        self.sensors.level_wr.reading
    }
    fn sensed_pressure(&self) -> f64 {
        self.sensors.pressure.reading
    }
    fn sensed_header_pressure(&self) -> f64 {
        self.sensors.header_pressure.reading
    }
    fn sensed_condenser_pressure(&self) -> f64 {
        self.sensors.condenser_pressure.reading
    }
```

Then apply the measurement column of `protection.csv` literally:

| Rows | Replace | With |
|---|---|---|
| S06, T01, T02, P05, A08, A09 | `self.level_cm()` | `self.sensed_level_nr()` |
| I01, C02, A10 | `self.level_cm()` | `self.sensed_level_wr()` |
| S05, C01, A03, A12, A14 | `self.vessel.eos.p` | `self.sensed_pressure()` |
| I02 | `self.steam.p_chest > 0.0 && self.steam.p_chest < MSIV_LOW_PRESSURE_RUN` | `self.sensed_header_pressure() > 0.0 && self.sensed_header_pressure() < MSIV_LOW_PRESSURE_RUN` |
| T03, P06, A20 | `self.cond.pressure()` | `self.sensed_condenser_pressure()` |
| A21 and the `byp_ok` call in `step()` | `self.cond.available_for_bypass()` | `self.cond.available_for_bypass(self.sensed_condenser_pressure())` |

In `bop.rs`:

```rust
    /// `p_sensed` is the condenser pressure instrument reading (Pa), not the model pressure.
    pub fn available_for_bypass(&self, p_sensed: f64) -> bool {
        p_sensed < CONDENSER_BYPASS_BLOCK_P && self.cw_pumps.iter().any(|&b| b)
    }
```

`core_coverage`, `heatup_rate`, `eos.regime`, `kin.n` (APRM/IRM/SRM rows), and `steam.vessel_steam_flow()` (I03) stay on model values, as the matrix declares.

Feed the level controller the instrument: in `step()` the level argument of `self.fw.step(...)` becomes `self.sensed_level_nr()`. Because sensors step at position 13 and feedwater at position 7, the controller sees the previous step's reading (one physics step older than the instrument, which is itself lagged by `LEVEL_SENSOR_TAU_S`). Note this in `docs/model/power-conversion.md`; Task 12's baseline table records the effect.

- [ ] **Step 6: API fields**

In `api.rs` `Vessel`, after `level_nr_cm`:

```rust
    /// Wide-range level instrument, cm relative to NWL.
    pub level_wr_cm: f64,
    /// Model (true) level, cm relative to NWL; level_nr_cm is the narrow-range instrument.
    pub level_model_cm: f64,
    pub level_nr_out_of_range: bool,
    /// Pressure instrument reading, MPa (pressure_mpa stays the model value).
    pub pressure_sensed_mpa: f64,
```

In `snapshot()`:

```rust
                level_nr_cm: self.sensors.level_nr.reading,
                level_wr_cm: self.sensors.level_wr.reading,
                level_model_cm: v.level_nr_cm(),
                level_nr_out_of_range: self.sensors.level_nr.out_of_range(),
                pressure_sensed_mpa: self.sensors.pressure.reading / 1e6,
```

In `ui/src/sim/types.ts` `Vessel`, after `levelNrCm`:

```ts
  /** Wide-range level instrument, cm relative to NWL. */
  levelWrCm: number;
  /** Model (true) level, cm relative to NWL; levelNrCm is the narrow-range instrument. */
  levelModelCm: number;
  levelNrOutOfRange: boolean;
  /** Pressure instrument reading, MPa; pressureMpa is the model value. */
  pressureSensedMpa: number;
```

In `ui/src/sim/mock.ts` (identifiers as inspected on 2026-10-21: state lives in `this.s`, `blankSnapshot()` builds the `vessel` literal, and `step()` binds `const v = s.vessel`): in the `blankSnapshot()` `vessel` literal, after `levelNrCm: 0,` add

```ts
      levelWrCm: 0,
      levelModelCm: 0,
      levelNrOutOfRange: false,
      pressureSensedMpa: 0.1,
```

and in `step()`, directly after the line `v.levelNrCm = (v.levelAboveTafM - NWL_ABOVE_TAF_M) * 100;`, add

```ts
    v.levelWrCm = v.levelNrCm;
    v.levelModelCm = v.levelNrCm;
    v.levelNrOutOfRange = v.levelNrCm < -150 || v.levelNrCm > 150;
    v.pressureSensedMpa = v.pressureMpa;
```

(the mock has no sensor lag; it is a UI demonstration).

- [ ] **Step 7: Run everything**

Run: `cargo test --release -p bwr-core && npm --prefix ui run typecheck`
Expected: all Rust tests pass including the seven new integration tests; typecheck passes.

- [ ] **Step 8: Register the constants**

Append to `docs/model/parameters.csv` (locked header `id,value,unit,definition,reference_state,source_design,status,source_ids,interval_lo,interval_hi,assumption_id,location,notes`):

```csv
LEVEL_NR_RANGE_CM,-150..150,cm,narrow-range level instrument span relative to NWL,NWL,none,assumed,,-200,200,A-803,crates/bwr-core/src/params.rs,
LEVEL_WR_RANGE_CM,-1200..200,cm,wide-range level instrument span relative to NWL,NWL,none,assumed,,-1500,300,A-803,crates/bwr-core/src/params.rs,
PRESSURE_SENSOR_RANGE_PA,0..10e6,Pa,dome pressure instrument span,n/a,none,assumed,,,,A-803,crates/bwr-core/src/params.rs,
HEADER_PRESSURE_SENSOR_RANGE_PA,0..10e6,Pa,main steam header pressure instrument span,n/a,none,assumed,,,,A-803,crates/bwr-core/src/params.rs,
CONDENSER_PRESSURE_SENSOR_RANGE_PA,0..120e3,Pa,condenser absolute pressure instrument span,n/a,none,assumed,,,,A-803,crates/bwr-core/src/params.rs,
LEVEL_SENSOR_TAU_S,0.5,s,level instrument first-order lag,n/a,none,assumed,,0.2,2,A-803,crates/bwr-core/src/params.rs,
PRESSURE_SENSOR_TAU_S,0.2,s,pressure instrument first-order lag,n/a,none,assumed,,0.1,1,A-803,crates/bwr-core/src/params.rs,
```

- [ ] **Step 9: Commit**

```bash
git add crates/bwr-core/src/sensors.rs crates/bwr-core/src/lib.rs crates/bwr-core/src/params.rs crates/bwr-core/src/sim.rs crates/bwr-core/src/api.rs crates/bwr-core/src/bop.rs crates/bwr-core/tests/protection.rs ui/src/sim/types.ts ui/src/sim/mock.ts docs/model/parameters.csv
git commit -m "feat(protection): five lagged, ranged instruments; protection and level control read sensed values

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 8: Rod motion in the snapshot; ValueTag; display labelling; design audit

**Files:**
- Modify: `crates/bwr-core/src/api.rs` (`Rods.motion`), `sim.rs:snapshot`
- Modify: `ui/src/sim/types.ts`, `ui/src/sim/mock.ts`
- Create: `ui/src/components/common/ValueTag.tsx`
- Modify: `ui/src/components/common/Readout.tsx`, `ui/src/styles.css`, `ui/src/components/RodControls.tsx`, `ui/src/components/FeedwaterPanel.tsx`, `ui/src/components/TurbinePanel.tsx`, `ui/src/components/NuclearInstruments.tsx`, `ui/src/components/mimic/PlantMimic.tsx`
- Test: `crates/bwr-core/tests/protection.rs` (snapshot field), `npm --prefix ui run typecheck`

**Interfaces:**
- Consumes: P5 `RodMotion`.
- Produces: `Snapshot.rods.motion: Vec<String>` with values `"idle" | "motor" | "scram" | "sri" | "motorRunIn"`; TS `export type RodMotion = "idle" | "motor" | "scram" | "sri" | "motorRunIn";`; `ValueTag` component with `export type ValueKind = "actual" | "cmd" | "sensed" | "model";`; `Readout` gains optional `kind?: ValueKind`.

- [ ] **Step 1: Failing Rust test**

```rust
#[test]
fn snapshot_reports_rod_motion() {
    let mut sim = Sim::new(ScenarioName::HotStandby);
    sim.apply(Command::RodSetTarget { indices: vec![0], target: 2.0 });
    sim.step(DT);
    let s = sim.snapshot();
    assert_eq!(s.rods.motion.len(), N_RODS);
    assert_eq!(s.rods.motion[0], "motor");
    assert_eq!(s.rods.motion[1], "idle");
    sim.apply(Command::ManualScram);
    sim.step(DT);
    assert_eq!(sim.snapshot().rods.motion[5], "scram");
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 2.0);
    sim.apply(Command::TripTurbine);
    sim.step(DT);
    let sri = sim.core.rods.sri_group();
    assert_eq!(sim.snapshot().rods.motion[sri[0]], "sri");
}
```

Run: `cargo test --release -p bwr-core --test protection snapshot_reports_rod_motion`
Expected: compile failure, `motion` missing.

- [ ] **Step 2: Implement in Rust**

`api.rs` `Rods`: add `pub motion: Vec<String>,` after `targets`. In `snapshot()`:

```rust
                motion: c.rods.motion.iter().map(|m| match m {
                    crate::reactor::RodMotion::Idle => "idle",
                    crate::reactor::RodMotion::Motor { .. } => "motor",
                    crate::reactor::RodMotion::Scram => "scram",
                    crate::reactor::RodMotion::Sri => "sri",
                    crate::reactor::RodMotion::MotorRunIn => "motorRunIn",
                }.to_string()).collect(),
```

Run the test: PASS.

- [ ] **Step 3: TypeScript contract and mock**

`types.ts`: add `export type RodMotion = "idle" | "motor" | "scram" | "sri" | "motorRunIn";` and in `Rods`: `/** Motion mechanism per rod. */ motion: RodMotion[];`. In `mock.ts` (identifiers as inspected on 2026-10-21): in the `blankSnapshot()` `rods` literal add `motion: LAYOUT.map(() => "idle" as RodMotion),` (import `RodMotion` from `./types`), and in `step()` right after the line `if (s.rods.scramInProgress && !anyMoving) s.rods.scramInProgress = false;` add `s.rods.motion = s.rods.positions.map((p, i) => (s.rods.scramInProgress ? "scram" : Math.abs(s.rods.targets[i] - p) > 1e-6 ? "motor" : "idle"));` (the mock never shows `sri` or `motorRunIn`; it is a UI demonstration).

- [ ] **Step 4: ValueTag component and styles**

Create `ui/src/components/common/ValueTag.tsx`:

```tsx
/** Marks a displayed number as actual plant state, operator command, instrument reading, or model calculation. */
export type ValueKind = "actual" | "cmd" | "sensed" | "model";

const TEXT: Record<ValueKind, string> = { actual: "ACT", cmd: "CMD", sensed: "SENSED", model: "MODEL" };
const TITLE: Record<ValueKind, string> = {
  actual: "Actual equipment state in the simulator",
  cmd: "Operator-commanded value; equipment may still be moving",
  sensed: "Instrument reading: lagged and range-limited",
  model: "Calculated model quantity; not an instrument in the plant",
};

export function ValueTag(props: { kind: ValueKind; oor?: boolean }) {
  const text = props.oor ? "OOR" : TEXT[props.kind];
  const title = props.oor ? "Instrument out of range" : TITLE[props.kind];
  return (
    <span className={`vtag vtag-${props.kind}${props.oor ? " vtag-oor" : ""}`} title={title} aria-label={title}>
      {text}
    </span>
  );
}
```

Modify `Readout.tsx` to accept `kind?: ValueKind` and `oor?: boolean`:

```tsx
import { ValueTag, type ValueKind } from "./ValueTag";

export function Readout(props: {
  label: string;
  value: string;
  unit?: string;
  tone?: "normal" | "warn" | "alarm" | "dim";
  wide?: boolean;
  kind?: ValueKind;
  oor?: boolean;
}) {
  return (
    <div className={`readout ${props.wide ? "wide" : ""}`}>
      <div className="readout-label">
        {props.label}
        {props.kind && <ValueTag kind={props.kind} oor={props.oor} />}
      </div>
      <div className={`readout-value tone-${props.oor ? "alarm" : props.tone ?? "normal"}`}>
        {props.value}
        {props.unit && <span className="readout-unit">{props.unit}</span>}
      </div>
    </div>
  );
}
```

Append to `ui/src/styles.css`:

```css
/* ------------------------------------------------------------------ value provenance tags */
.vtag {
  display: inline-block;
  margin-left: 6px;
  padding: 0 4px;
  border-radius: 2px;
  font: 600 9px/14px var(--mono);
  letter-spacing: 0.04em;
  vertical-align: middle;
  border: 1px solid var(--panel-edge);
  color: var(--ink-dim);
}
.vtag-actual { color: var(--green); border-color: var(--green); }
.vtag-cmd { color: var(--amber); border-color: var(--amber); }
.vtag-sensed { color: var(--blue); border-color: var(--blue); }
.vtag-model { color: var(--ink-dim); border-style: dashed; }
.vtag-oor { color: var(--red); border-color: var(--red); animation: blink 1s steps(2) infinite; }
```

(`blink` keyframes already exist for `.lamp.blink`; if the name differs, reuse that name.)

- [ ] **Step 5: Label the panels**

`RodControls.tsx`: replace the POS / TGT block:

```tsx
        <div>
          <div className="tb-label">
            POS <ValueTag kind="actual" /> / TGT <ValueTag kind="cmd" />
          </div>
          <div className="digit big">
            {sel !== null ? `${fmtRodPos(rods.positions[sel])}/${fmtRodPos(rods.targets[sel])}` : "--/--"}
          </div>
          <div className="hint">{sel !== null ? rods.motion[sel].toUpperCase() : ""}</div>
        </div>
```

and add `import { ValueTag } from "./common/ValueTag";`.

`FeedwaterPanel.tsx`: replace the `LEVEL NR` readout and add a model readout:

```tsx
        <Readout label="LEVEL NR" value={fx(v.levelNrCm, 0)} unit="cm" tone={lvlTone} kind="sensed" oor={v.levelNrOutOfRange} />
        <Readout label="LEVEL WR" value={fx(v.levelWrCm, 0)} unit="cm" kind="sensed" />
        <Readout label="LEVEL" value={fx(v.levelModelCm, 0)} unit="cm" tone="dim" kind="model" />
```

Add `kind="cmd"` to the `FCV MAN` slider label (`<span className="tb-label">FCV MAN <ValueTag kind="cmd" /></span>`) and `kind="actual"` to the `FCV POS` readout; `SETPT` label gets `<ValueTag kind="cmd" />`. Import `ValueTag`.

`TurbinePanel.tsx`: open the file; change the `RX PRESS` readout to `value={fx(s.vessel.pressureSensedMpa, 3)} ... kind="sensed"` and add `<Readout label="RX PRESS" value={fx(s.vessel.pressureMpa, 3)} unit="MPa" tone="dim" kind="model" />` beside it; tag `SPEED SP` / load limit / pressure setpoint readouts with `kind="cmd"` and speed / MWe readouts with `kind="actual"`.

`NuclearInstruments.tsx`: add `kind="model"` to `K-EFF`, `FUEL TEMP`, `XENON`, `IODINE`, `NEUTRON PWR`, `THERMAL`, `DECAY HEAT`; keep SRM/IRM/APRM bar meters unlabelled (they are instruments); change the `REACTIVITY (pcm)` header cell to `REACTIVITY (pcm) <ValueTag kind="model" />`. The `FUEL TEMP` tone threshold `> 1200` becomes `n.fuelTempC > 1000` (A24; the 1200 damage rule was removed in P5).

`PlantMimic.tsx`: at the MSIV symbols (around the `msivPos`/`msivCommandOpen` lines), add a second small lamp per line showing the command: open the file, find the MSIV `<Valve>` call, and beside it render `<Tag ... text={st.msivCommandOpen[i] ? "CMD OPEN" : "CMD CLOSE"} tone={st.msivCommandOpen[i] !== (pos > 0.5) ? "warn" : undefined} />` using the mimic's existing `Tag` helper (position offsets: x + 0, y + 14 from the valve label). Change the level tag to `L ${fx(v.levelNrCm, 0)} cm NR${v.levelNrOutOfRange ? " OOR" : ""}` and the pressure tag to `P ${fx(v.pressureSensedMpa, 2)} MPa`.

- [ ] **Step 6: Typecheck and build**

Run: `npm --prefix ui run typecheck && npm --prefix ui run build`
Expected: both succeed.

- [ ] **Step 7: Design audit**

Run the `frontend-design-audit` skill in quick mode (`/fda:quick`) on `ui/src/components`. Fix every severity 3–4 finding it reports and practical severity 2 findings; re-run `npm --prefix ui run typecheck`.

- [ ] **Step 8: Desktop smoke check**

Run: `npm run dev`. Load FULL POWER; confirm the rod POS/TGT tags, LEVEL NR SENSED with OOR behaviour after setting FCV MAN 0 %, and MSIV CMD tags. Close the app.

- [ ] **Step 9: Commit**

```bash
git add crates/bwr-core/src/api.rs crates/bwr-core/src/sim.rs crates/bwr-core/tests/protection.rs ui/src/sim/types.ts ui/src/sim/mock.ts ui/src/components/common/ValueTag.tsx ui/src/components/common/Readout.tsx ui/src/styles.css ui/src/components/RodControls.tsx ui/src/components/FeedwaterPanel.tsx ui/src/components/TurbinePanel.tsx ui/src/components/NuclearInstruments.tsx ui/src/components/mimic/PlantMimic.tsx
git commit -m "feat(ui): label readouts as actual / commanded / sensed / model; rod motion in snapshot

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 9: Alternate rod insertion path behind `ARI_ENABLED`

**Files:**
- Modify: `crates/bwr-core/src/params.rs`, `crates/bwr-core/src/sim.rs` (field, `finalize_init`, `protection`, `apply` `ResetScram`)
- Modify: `docs/model/parameters.csv`, `docs/model/protection.csv`, `docs/model/assumptions.md`
- Test: `crates/bwr-core/tests/protection.rs`

**Interfaces:**
- Consumes: P5 `Rods::scram_hydraulic()`, `Rods::motor_run_in()`.
- Produces (master r2): `pub const ARI_ENABLED: bool = false;`, `pub const ARI_HIGH_PRESSURE: f64 = 7.70e6;`, `pub const ARI_L2_DELAY_S: f64 = 9.0;` in `params.rs`; `Sim.ari_latched: bool`; `Sim.ari_l2_timer: f64` (private); `pub fn alternate_rod_insertion(&mut self)`; log line `"ari: <signal>"` (master log-key convention); matrix row `R02`.

- [ ] **Step 1: Write the failing test**

```rust
// matrix: R02 (disabled by default: nothing happens even when the signals are present)
#[test]
fn ari_disabled_by_default() {
    assert!(!ARI_ENABLED, "ARI must stay disabled until a public source is recorded");
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 2.0);
    sim.sensors.pressure.fail();
    sim.sensors.pressure.reading = ARI_HIGH_PRESSURE + 0.1e6; // ARI signal present on the instrument
    sim.sensors.level_wr.fail();
    sim.sensors.level_wr.reading = L2_CM - 10.0;
    run(&mut sim, ARI_L2_DELAY_S + 5.0);
    assert!(!sim.ari_latched);
    assert!(!logged(&sim, "ari:"));
}

// matrix: R02 (manual path is wired and layered on the RPS latch)
#[test]
fn alternate_rod_insertion_inserts_without_rps_latch() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 2.0);
    sim.alternate_rod_insertion();
    run(&mut sim, SCRAM_TIME + 2.0);
    assert!(sim.ari_latched);
    assert!(sim.core.rods.all_in());
    assert!(!sim.scrammed, "ARI is not the RPS scram latch");
    assert!(logged(&sim, "ari: manual"));
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --release -p bwr-core --test protection ari`
Expected (both `ari_disabled_by_default` and `alternate_rod_insertion_inserts_without_rps_latch` match the filter): compile failure (`ARI_ENABLED`, `ari_latched`, `alternate_rod_insertion` missing).

- [ ] **Step 3: Implement**

`params.rs`, after `PERIOD_TRIP_S`:

```rust
/// Alternate rod insertion (diverse path). UNAVAILABLE: no public BWRX-300 source
/// identifies ARI signals or setpoints; the automatic signals are compiled out.
/// Flip only with a source recorded in docs/model/sources.csv.
pub const ARI_ENABLED: bool = false;
/// ARI on reactor high pressure, Pa. ASSUMED placeholder (above the RPS setpoint).
pub const ARI_HIGH_PRESSURE: f64 = 7.70e6;
/// ARI on L2 after this delay, s. ASSUMED placeholder.
pub const ARI_L2_DELAY_S: f64 = 9.0;
```

`sim.rs`: add fields `pub ari_latched: bool,` and `ari_l2_timer: f64,` (both `false`/`0.0` in `base()`, reset in `finalize_init`). Add:

```rust
    /// Alternate rod insertion: the same two rod mechanisms as the RPS scram,
    /// through a separate latch (matrix R02). Manual call or, when ARI_ENABLED,
    /// automatic on ARI_HIGH_PRESSURE or L2 persisting for ARI_L2_DELAY_S.
    pub fn alternate_rod_insertion(&mut self) {
        self.ari_with_reason("manual");
    }

    fn ari_with_reason(&mut self, reason: &str) {
        if !self.ari_latched {
            self.log.push((self.time, format!("ari: {reason}")));
            self.event("ari", format!("Alternate rod insertion: {reason}"));
        }
        self.ari_latched = true;
        self.core.rods.scram_hydraulic();
        self.core.rods.motor_run_in();
    }
```

In `protection()`, after the scram-signal block:

```rust
        // Alternate rod insertion signals (matrix R02); compiled out until sourced.
        if ARI_ENABLED && !self.ari_latched {
            if self.sensed_pressure() > ARI_HIGH_PRESSURE {
                self.ari_with_reason("Reactor high pressure");
            }
            if self.sensed_level_wr() < L2_CM {
                self.ari_l2_timer += DT;
                if self.ari_l2_timer >= ARI_L2_DELAY_S {
                    self.ari_with_reason("Low-low water level (L2) sustained");
                }
            } else {
                self.ari_l2_timer = 0.0;
            }
        }
```

In `apply`, `ResetScram`: when the reset succeeds also set `self.ari_latched = false; self.ari_l2_timer = 0.0;` (a reset clears both latches; the rods-at-zero condition from P5 already governs returning Scram/MotorRunIn rods to Idle).

- [ ] **Step 4: Run**

Run: `cargo test --release -p bwr-core`
Expected: all pass.

- [ ] **Step 5: Register and matrix rows**

Append to `docs/model/parameters.csv`:

```csv
ARI_ENABLED,false,bool,compile-time guard for alternate rod insertion signals,n/a,none,unavailable,,,,A-805,crates/bwr-core/src/params.rs,flip only with a source
ARI_HIGH_PRESSURE,7.70e6,Pa,ARI reactor high pressure setpoint if enabled,n/a,none,assumed,,7.6e6,7.9e6,A-805,crates/bwr-core/src/params.rs,
ARI_L2_DELAY_S,9,s,ARI L2 time delay if enabled,n/a,none,assumed,,5,15,A-805,crates/bwr-core/src/params.rs,
```

Append to `docs/model/protection.csv`:

```csv
R02,alternate rod insertion,reactor high pressure or sustained L2 (or manual),sensors.pressure / sensors.level_wr,all (if ARI_ENABLED),7.70 MPa; L2 for 9 s,Pa / s,unavailable (ARI_ENABLED=false; placeholders assumed),0 / 9,yes (ari_latched),ResetScram,disabled by constant,hydraulic scram + motor run-in through the ARI latch; RPS latch untouched,ari_disabled_by_default / alternate_rod_insertion_inserts_without_rps_latch
```

Append to `docs/model/assumptions.md`:

```markdown
- **A-805** Alternate rod insertion exists as a diverse initiation path in the engine (`Sim::alternate_rod_insertion`) but its automatic signals are compiled out (`ARI_ENABLED = false`). Setpoints are placeholders. Enabling requires a public BWRX-300 source for the signals and values; record it in sources.csv, set the constant, and change the `protection.csv` R02 `threshold_source` in the same commit.
```

- [ ] **Step 6: Commit**

```bash
git add crates/bwr-core/src/params.rs crates/bwr-core/src/sim.rs crates/bwr-core/tests/protection.rs docs/model/parameters.csv docs/model/protection.csv docs/model/assumptions.md
git commit -m "feat(protection): alternate rod insertion path behind ARI_ENABLED guard

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 10: Mode transition tests

**Files:**
- Test: `crates/bwr-core/tests/protection.rs`
- Modify (only if a test exposes a defect): `crates/bwr-core/src/sim.rs`

**Interfaces:**
- Consumes: matrix rows I02, S03, B03, S08.

- [ ] **Step 1: Write the tests**

```rust
// matrix: I02 (transition STARTUP -> RUN arms low-pressure isolation)
#[test]
fn isolation_low_pressure_armed_in_run() {
    let mut sim = Sim::new(ScenarioName::HotStandby); // STARTUP, ~6.9 MPa, condenser on line
    sim.apply(Command::SetPressureSetpoint { mpa: 5.0 });
    run(&mut sim, 600.0);
    assert!(sim.steam.line.p < MSIV_LOW_PRESSURE_RUN, "setup: header pressure {}", sim.steam.line.p);
    run(&mut sim, 5.0 * PRESSURE_SENSOR_TAU_S); // let the header instrument follow
    assert!(sim.steam.msiv_cmd.iter().all(|&c| c), "no isolation in STARTUP");
    sim.apply(Command::SetModeSwitch { mode: ModeSwitch::Run });
    sim.step(DT);
    assert!(sim.steam.msiv_cmd.iter().all(|&c| !c), "RUN arms the low steam pressure isolation");
    assert!(logged(&sim, "msiv isolation"));
}

// matrix: S03 (transition RUN -> STARTUP at power scrams on the setdown)
#[test]
fn run_to_startup_at_power_scrams_on_setdown() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 2.0);
    sim.apply(Command::SetModeSwitch { mode: ModeSwitch::Startup });
    sim.step(DT);
    assert!(sim.scrammed);
    assert_eq!(sim.scram_reason.as_deref(), Some("APRM high flux (setdown)"));
}

// matrix: S08 (transition any -> SHUTDOWN scrams; reason recorded)
#[test]
fn shutdown_mode_scrams_from_run() {
    let mut sim = Sim::new(ScenarioName::FullPower);
    run(&mut sim, 2.0);
    sim.apply(Command::SetModeSwitch { mode: ModeSwitch::Shutdown });
    sim.step(DT);
    assert_eq!(sim.scram_reason.as_deref(), Some("Mode switch in SHUTDOWN"));
}

// matrix: B03 (transition STARTUP -> REFUEL with two rods out blocks further withdrawal)
#[test]
fn refuel_with_two_rods_out_blocks() {
    let mut sim = Sim::new(ScenarioName::HotStandby);
    sim.apply(Command::RodSetTarget { indices: vec![0, 1], target: 2.0 });
    run(&mut sim, 10.0);
    assert!(sim.core.rods.pos[0] > 1.9 && sim.core.rods.pos[1] > 1.9);
    sim.apply(Command::SetModeSwitch { mode: ModeSwitch::Refuel });
    sim.step(DT);
    sim.apply(Command::RodSetTarget { indices: vec![2], target: 2.0 });
    run(&mut sim, 10.0);
    assert!(sim.core.rods.pos[2] < 0.01);
    assert!(has_alarm(&sim, "rod_block"));
}

// matrix: B02 (SHUTDOWN blocks withdrawal; insertion allowed)
#[test]
fn block_shutdown_mode() {
    let mut sim = Sim::new(ScenarioName::HotStandby);
    sim.apply(Command::RodSetTarget { indices: vec![0], target: 4.0 });
    run(&mut sim, 15.0);
    sim.apply(Command::SetModeSwitch { mode: ModeSwitch::Shutdown });
    run(&mut sim, 15.0); // scram drives it in
    sim.apply(Command::ResetScram); // blocked: still in SHUTDOWN
    sim.apply(Command::RodSetTarget { indices: vec![0], target: 2.0 });
    run(&mut sim, 10.0);
    assert!(sim.core.rods.pos[0] < 0.01);
}
```

- [ ] **Step 2: Run**

Run, one filter per invocation:
```bash
cargo test --release -p bwr-core --test protection isolation_low_pressure
cargo test --release -p bwr-core --test protection run_to_startup
cargo test --release -p bwr-core --test protection shutdown_mode_scrams
cargo test --release -p bwr-core --test protection refuel_with_two
cargo test --release -p bwr-core --test protection block_shutdown
```
Expected: all pass. If `isolation_low_pressure_armed_in_run` fails because the hot-standby bypass cannot lower header pressure to 5.9 MPa within 600 s, lower the setpoint to 4.0 MPa and extend to 1200 s; do not change the isolation threshold.

- [ ] **Step 3: Commit**

```bash
git add crates/bwr-core/tests/protection.rs
git commit -m "test(protection): mode transition tests for RUN arming, setdown scram, SHUTDOWN, REFUEL

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 11: Command matrix: positive, negative, boundary, transition for all 31 commands

`api.rs` has 31 `Command` variants (the spec's "every control action"): SetPaused, SetSpeed, LoadScenario, ManualScram, ResetScram, SetModeSwitch, SelectRod, RodSetTarget, RodStop, SetIrmRange, SetHeadVent, SetMsiv, SetPressureSetpoint, SetBypass, SetSpeedSetpoint, ResetTurbine, TripTurbine, CloseBreaker, OpenBreaker, SetLoadLimit, SetCwPump, SetVacuumPump, SetFeedPump, SetCondensatePump, SetLevelControl, SetFcvManual, SetLevelSetpoint, SetHeaters, SetRwcuReject, SetIcTrain, AckAlarms.

**Files:**
- Test: `crates/bwr-core/tests/protection.rs`

**Interfaces:**
- Produces: `fn assert_cmd<F: Fn(&Sim) -> bool>(scenario: ScenarioName, setup: Vec<Command>, cmd: Command, settle_s: f64, check: F)` test helper.

- [ ] **Step 1: Add the helper**

```rust
fn neutronics_power(s: &Sim) -> f64 {
    s.core.kin.n
}

/// Build `scenario`, apply `setup`, run 1 s, apply `cmd`, run `settle_s`, assert `check`.
fn assert_cmd<F: Fn(&Sim) -> bool>(scenario: ScenarioName, setup: Vec<Command>, cmd: Command, settle_s: f64, check: F) {
    let mut sim = Sim::new(scenario);
    for c in setup {
        sim.apply(c);
    }
    run(&mut sim, 1.0);
    let label = format!("{cmd:?}");
    sim.apply(cmd);
    run(&mut sim, settle_s);
    assert!(check(&sim), "command {label} in {scenario:?} did not produce the expected state");
}
```

- [ ] **Step 2: Add the cases**

Each command gets four tests named `cmd_<name>_{positive,negative,boundary,transition}`. Where a command has no blocking condition, the negative test asserts the command is a no-op on an unrelated state (documented in the test comment). Full code:

```rust
use bwr_core::api::ScenarioName::*;
use bwr_core::api::ModeSwitch as M;
use bwr_core::sim::irm_full_scale;

#[test] fn cmd_set_paused_positive() { assert_cmd(HotStandby, vec![], Command::SetPaused { paused: true }, 0.0, |s| s.paused); }
#[test] fn cmd_set_paused_negative() { let mut s = Sim::new(HotStandby); s.apply(Command::SetPaused { paused: true }); let t = s.time; s.advance(1.0); assert_eq!(s.time, t, "paused sim must not advance"); }
#[test] fn cmd_set_paused_boundary() { assert_cmd(HotStandby, vec![Command::SetPaused { paused: true }], Command::SetPaused { paused: true }, 0.0, |s| s.paused); }
#[test] fn cmd_set_paused_transition() { assert_cmd(HotStandby, vec![Command::SetPaused { paused: true }], Command::SetPaused { paused: false }, 0.0, |s| !s.paused); }

#[test] fn cmd_set_speed_positive() { assert_cmd(HotStandby, vec![], Command::SetSpeed { speed: 10.0 }, 0.0, |s| s.speed == 10.0); }
#[test] fn cmd_set_speed_negative() { let mut s = Sim::new(HotStandby); s.apply(Command::SetSpeed { speed: 10.0 }); assert_eq!(s.time, 0.0, "speed alone does not advance time"); }
#[test] fn cmd_set_speed_boundary() { assert_cmd(HotStandby, vec![], Command::SetSpeed { speed: 1e9 }, 0.0, |s| s.speed == 100.0); assert_cmd(HotStandby, vec![], Command::SetSpeed { speed: 0.0 }, 0.0, |s| s.speed == 0.1); }
#[test] fn cmd_set_speed_transition() { let mut s = Sim::new(HotStandby); s.apply(Command::SetSpeed { speed: 5.0 }); s.apply(Command::LoadScenario { name: FullPower }); assert_eq!(s.speed, 5.0, "speed survives scenario load"); }

#[test] fn cmd_load_scenario_positive() { assert_cmd(HotStandby, vec![], Command::LoadScenario { name: FullPower }, 0.0, |s| s.mode == M::Run && s.time == 0.0); }
#[test] fn cmd_load_scenario_negative() { assert_cmd(FullPower, vec![Command::ManualScram], Command::LoadScenario { name: FullPower }, 0.0, |s| !s.scrammed && s.log.is_empty() && s.domain_errors.is_empty()); }
#[test] fn cmd_load_scenario_boundary() { assert_cmd(ColdShutdown, vec![], Command::LoadScenario { name: ColdShutdown }, 1.0, |s| s.mode == M::Shutdown && s.sensors.level_nr.reading.abs() < 5.0); }
#[test] fn cmd_load_scenario_transition() { assert_cmd(FullPower, vec![Command::SetPaused { paused: true }], Command::LoadScenario { name: HotStandby }, 0.0, |s| s.paused && s.mode == M::Startup); }

#[test] fn cmd_manual_scram_positive() { assert_cmd(FullPower, vec![], Command::ManualScram, 10.0, |s| s.scrammed && s.core.rods.all_in() && s.scram_reason.as_deref() == Some("Manual scram")); }
#[test] fn cmd_manual_scram_negative() { assert_cmd(FullPower, vec![Command::SetMsiv { line: 0, open: false }, Command::SetMsiv { line: 1, open: false }], Command::ManualScram, 30.0, |s| s.scram_reason.as_deref() != Some("Manual scram"), ); }
#[test] fn cmd_manual_scram_boundary() { assert_cmd(ColdShutdown, vec![], Command::ManualScram, 1.0, |s| s.scrammed && s.core.rods.all_in()); }
#[test] fn cmd_manual_scram_transition() { assert_cmd(FullPower, vec![], Command::ManualScram, 60.0, |s| s.ic.iter().all(|t| !t.cmd_open) && !s.turbine.tripped || s.turbine.trip_reason.as_deref() == Some("Reactor high water level (L8)")); }

#[test] fn cmd_reset_scram_positive() { assert_cmd(HotStandby, vec![Command::ManualScram], Command::ResetScram, 0.0, |s| s.scrammed); /* within 1 s: refused */ }
#[test] fn cmd_reset_scram_negative() { let mut s = Sim::new(HotStandby); s.apply(Command::ManualScram); run(&mut s, 11.0); s.apply(Command::ResetScram); assert!(!s.scrammed); }
#[test] fn cmd_reset_scram_boundary() { assert_cmd(HotStandby, vec![], Command::ResetScram, 0.0, |s| !s.scrammed && !has_alarm(s, "scram_reset")); }
#[test] fn cmd_reset_scram_transition() { let mut s = Sim::new(HotStandby); s.apply(Command::SetModeSwitch { mode: M::Shutdown }); run(&mut s, 11.0); s.apply(Command::ResetScram); assert!(s.scrammed); s.apply(Command::SetModeSwitch { mode: M::Startup }); s.step(DT); s.apply(Command::ResetScram); assert!(!s.scrammed); }

#[test] fn cmd_set_mode_switch_positive() { assert_cmd(HotStandby, vec![], Command::SetModeSwitch { mode: M::Run }, 0.0, |s| s.mode == M::Run); }
#[test] fn cmd_set_mode_switch_negative() { assert_cmd(HotStandby, vec![], Command::SetModeSwitch { mode: M::Startup }, 1.0, |s| !s.scrammed); }
#[test] fn cmd_set_mode_switch_boundary() { assert_cmd(HotStandby, vec![], Command::SetModeSwitch { mode: M::Shutdown }, 1.0, |s| s.scrammed); }
#[test] fn cmd_set_mode_switch_transition() { assert_cmd(FullPower, vec![], Command::SetModeSwitch { mode: M::Startup }, 1.0, |s| s.scram_reason.as_deref() == Some("APRM high flux (setdown)")); }

#[test] fn cmd_select_rod_positive() { assert_cmd(HotStandby, vec![], Command::SelectRod { index: Some(3) }, 0.0, |s| s.core.rods.selected == Some(3)); }
#[test] fn cmd_select_rod_negative() { assert_cmd(HotStandby, vec![], Command::SelectRod { index: Some(99) }, 0.0, |s| s.core.rods.selected.is_none()); }
#[test] fn cmd_select_rod_boundary() { assert_cmd(HotStandby, vec![], Command::SelectRod { index: Some(N_RODS - 1) }, 0.0, |s| s.core.rods.selected == Some(N_RODS - 1)); }
#[test] fn cmd_select_rod_transition() { assert_cmd(HotStandby, vec![Command::SelectRod { index: Some(3) }], Command::SelectRod { index: None }, 0.0, |s| s.core.rods.selected.is_none()); }

#[test] fn cmd_rod_set_target_positive() { assert_cmd(HotStandby, vec![], Command::RodSetTarget { indices: vec![0], target: 4.0 }, 15.0, |s| s.core.rods.pos[0] > 3.9); }
#[test] fn cmd_rod_set_target_negative() { assert_cmd(ColdShutdown, vec![], Command::RodSetTarget { indices: vec![0], target: 4.0 }, 15.0, |s| s.core.rods.pos[0] < 0.01 && has_alarm(s, "rod_block") == false /* SHUTDOWN suppresses the warning (A05) */); }
#[test] fn cmd_rod_set_target_boundary() { assert_cmd(HotStandby, vec![], Command::RodSetTarget { indices: vec![0], target: 3.0 }, 15.0, |s| (s.core.rods.pos[0] - 4.0).abs() < 0.01 /* rounds to even notch */); assert_cmd(HotStandby, vec![], Command::RodSetTarget { indices: vec![0], target: 99.0 }, 1.0, |s| s.core.rods.target(0) == ROD_FULL_OUT); }
#[test] fn cmd_rod_set_target_transition() { assert_cmd(HotStandby, vec![Command::ManualScram], Command::RodSetTarget { indices: vec![0], target: 4.0 }, 15.0, |s| s.core.rods.pos[0] < 0.01); }

#[test] fn cmd_rod_stop_positive() { let mut s = Sim::new(HotStandby); s.apply(Command::RodSetTarget { indices: vec![0], target: 20.0 }); run(&mut s, 5.0); s.apply(Command::RodStop); let p = s.core.rods.pos[0]; run(&mut s, 10.0); assert!((s.core.rods.pos[0] - p).abs() <= 2.0 && s.core.rods.pos[0] % 2.0 < 1e-6); }
#[test] fn cmd_rod_stop_negative() { assert_cmd(HotStandby, vec![], Command::RodStop, 1.0, |s| !s.core.rods.moving()); }
#[test] fn cmd_rod_stop_boundary() { let mut s = Sim::new(HotStandby); s.apply(Command::RodSetTarget { indices: vec![0], target: 2.0 }); s.apply(Command::RodStop); run(&mut s, 10.0); assert!(s.core.rods.pos[0] == 0.0 || (s.core.rods.pos[0] - 2.0).abs() < 1e-6); }
#[test] fn cmd_rod_stop_transition() { assert_cmd(FullPower, vec![Command::ManualScram], Command::RodStop, 10.0, |s| s.core.rods.all_in(), ); }

#[test] fn cmd_set_irm_range_positive() { assert_cmd(HotStandby, vec![], Command::SetIrmRange { range: 5 }, 0.0, |s| s.irm_range == 5); }
#[test] fn cmd_set_irm_range_negative() { assert_cmd(HotStandby, vec![], Command::SetIrmRange { range: 5 }, 0.0, |s| !s.scrammed); }
#[test] fn cmd_set_irm_range_boundary() { assert_cmd(HotStandby, vec![], Command::SetIrmRange { range: 0 }, 0.0, |s| s.irm_range == 1); assert_cmd(HotStandby, vec![], Command::SetIrmRange { range: 200 }, 0.0, |s| s.irm_range == 10); }
#[test] fn cmd_set_irm_range_transition() { let mut s = Sim::new(FullPower); s.apply(Command::SetIrmRange { range: 1 }); assert!(s.core.kin.n / irm_full_scale(1) > 1.2); s.step(DT); assert!(!s.scrammed, "IRM bypassed in RUN"); }

#[test] fn cmd_set_head_vent_positive() { assert_cmd(ColdShutdown, vec![], Command::SetHeadVent { open: false }, 0.0, |s| !s.vessel.head_vent_open); }
#[test] fn cmd_set_head_vent_negative() { assert_cmd(FullPower, vec![], Command::SetHeadVent { open: true }, 1.0, |s| has_alarm(s, "vent")); }
#[test] fn cmd_set_head_vent_boundary() { assert_cmd(ColdShutdown, vec![], Command::SetHeadVent { open: true }, 0.0, |s| s.vessel.head_vent_open); }
#[test] fn cmd_set_head_vent_transition() { assert_cmd(HotStandby, vec![Command::SetHeadVent { open: true }], Command::SetHeadVent { open: false }, 1.0, |s| s.vessel.vent_flow == 0.0); }

#[test] fn cmd_set_msiv_positive() { assert_cmd(FullPower, vec![], Command::SetMsiv { line: 0, open: false }, MSIV_STROKE + 1.0, |s| s.steam.msiv_pos[0] < 0.01 && s.steam.msiv_cmd[1] && s.scram_reason.as_deref() == Some("MSIV closure") /* S02 single-line rule */); }
#[test] fn cmd_set_msiv_negative() { assert_cmd(FullPower, vec![], Command::SetMsiv { line: 7, open: false }, 1.0, |s| s.steam.msiv_cmd.iter().all(|&c| c)); }
#[test] fn cmd_set_msiv_boundary() { assert_cmd(HotStandby, vec![Command::SetMsiv { line: 0, open: false }], Command::SetMsiv { line: 0, open: true }, MSIV_STROKE + 1.0, |s| s.steam.msiv_pos[0] > 0.99 && !s.scrammed /* STARTUP: S02 not armed, reopen allowed with no isolation signal */); }
#[test] fn cmd_set_msiv_transition() { let mut s = Sim::new(FullPower); s.apply(Command::SetLevelControl { mode: LevelControlMode::Manual }); s.apply(Command::SetFcvManual { pos: 0.0 }); for _ in 0..(900.0 / DT) as usize { s.step(DT); if !s.steam.msiv_cmd[0] { break; } } s.apply(Command::SetMsiv { line: 0, open: true }); assert!(!s.steam.msiv_cmd[0], "reopen blocked by L2"); }

#[test] fn cmd_set_pressure_setpoint_positive() { assert_cmd(FullPower, vec![], Command::SetPressureSetpoint { mpa: 7.05 }, 60.0, |s| (s.vessel.eos.p - 7.3e6).abs() < 0.15e6); }
#[test] fn cmd_set_pressure_setpoint_negative() { assert_cmd(ColdShutdown, vec![], Command::SetPressureSetpoint { mpa: 7.0 }, 10.0, |s| s.vessel.eos.p < 0.2e6, ); }
#[test] fn cmd_set_pressure_setpoint_boundary() { assert_cmd(FullPower, vec![], Command::SetPressureSetpoint { mpa: 99.0 }, 0.0, |s| s.steam.p_setpoint == 7.4e6); assert_cmd(FullPower, vec![], Command::SetPressureSetpoint { mpa: -1.0 }, 0.0, |s| s.steam.p_setpoint == 0.2e6); }
#[test] fn cmd_set_pressure_setpoint_transition() { assert_cmd(FullPower, vec![], Command::SetPressureSetpoint { mpa: 6.5 }, 120.0, |s| s.steam.bypass_pos < 0.01 && s.vessel.eos.p < 7.0e6); }

#[test] fn cmd_set_bypass_positive() { assert_cmd(HotStandby, vec![], Command::SetBypass { auto: false, pos: 0.3 }, 10.0, |s| (s.steam.bypass_pos - 0.3).abs() < 0.02); }
#[test] fn cmd_set_bypass_negative() { assert_cmd(HotStandby, vec![Command::SetCwPump { index: 0, on: false }, Command::SetCwPump { index: 1, on: false }], Command::SetBypass { auto: false, pos: 1.0 }, 10.0, |s| s.steam.bypass_pos < 0.01, ); }
#[test] fn cmd_set_bypass_boundary() { assert_cmd(HotStandby, vec![], Command::SetBypass { auto: false, pos: 5.0 }, 0.0, |s| s.steam.bypass_manual_pos == 1.0); }
#[test] fn cmd_set_bypass_transition() { assert_cmd(HotStandby, vec![Command::SetBypass { auto: false, pos: 0.5 }], Command::SetBypass { auto: true, pos: 0.0 }, 60.0, |s| s.steam.bypass_auto); }

#[test] fn cmd_set_speed_setpoint_positive() { assert_cmd(HotStandby, vec![Command::ResetTurbine], Command::SetSpeedSetpoint { rpm: 600.0 }, 300.0, |s| (s.turbine.speed_rpm - 600.0).abs() < 30.0); }
#[test] fn cmd_set_speed_setpoint_negative() { assert_cmd(HotStandby, vec![], Command::SetSpeedSetpoint { rpm: 600.0 }, 60.0, |s| s.turbine.speed_rpm < 1.0, ); }
#[test] fn cmd_set_speed_setpoint_boundary() { assert_cmd(HotStandby, vec![], Command::SetSpeedSetpoint { rpm: 5000.0 }, 0.0, |s| s.turbine.speed_target == RATED_RPM * 1.02); }
#[test] fn cmd_set_speed_setpoint_transition() { assert_cmd(FullPower, vec![], Command::SetSpeedSetpoint { rpm: 1000.0 }, 10.0, |s| s.turbine.speed_rpm == RATED_RPM, ); }

#[test] fn cmd_reset_turbine_positive() { assert_cmd(HotStandby, vec![], Command::ResetTurbine, 0.0, |s| !s.turbine.tripped); }
#[test] fn cmd_reset_turbine_negative() { assert_cmd(HotStandby, vec![Command::SetCwPump { index: 0, on: false }, Command::SetCwPump { index: 1, on: false }, Command::SetVacuumPump { on: false }], Command::ResetTurbine, 0.0, |s| s.turbine.tripped || s.cond.pressure() <= CONDENSER_TRIP_P); }
#[test] fn cmd_reset_turbine_boundary() { assert_cmd(FullPower, vec![], Command::ResetTurbine, 0.0, |s| !s.turbine.tripped); }
#[test] fn cmd_reset_turbine_transition() { let mut s = Sim::new(FullPower); s.apply(Command::SetLevelControl { mode: LevelControlMode::Manual }); s.apply(Command::SetFcvManual { pos: 1.0 }); run(&mut s, 300.0); assert!(s.turbine.tripped); s.apply(Command::ResetTurbine); assert!(s.turbine.tripped, "reset blocked while level > L8"); assert!(has_alarm(&s, "turbine")); }

#[test] fn cmd_trip_turbine_positive() { assert_cmd(FullPower, vec![], Command::TripTurbine, 5.0, |s| s.turbine.tripped && s.steam.tcv_pos < 0.01 && !s.turbine.breaker_closed); }
#[test] fn cmd_trip_turbine_negative() { assert_cmd(HotStandby, vec![], Command::TripTurbine, 1.0, |s| s.turbine.trip_reason.as_deref() == Some("Turbine not latched")); }
#[test] fn cmd_trip_turbine_boundary() { assert_cmd(FullPower, vec![Command::TripTurbine], Command::TripTurbine, 1.0, |s| s.turbine.trip_reason.as_deref() == Some("Manual turbine trip")); }
#[test] fn cmd_trip_turbine_transition() { assert_cmd(FullPower, vec![], Command::TripTurbine, 120.0, |s| !s.scrammed && neutronics_power(s) < 0.9); }

#[test] fn cmd_close_breaker_positive() { assert_cmd(FullPower, vec![Command::OpenBreaker], Command::CloseBreaker, 0.0, |s| s.turbine.breaker_closed); }
#[test] fn cmd_close_breaker_negative() { assert_cmd(HotStandby, vec![], Command::CloseBreaker, 0.0, |s| !s.turbine.breaker_closed && has_alarm(s, "breaker")); }
#[test] fn cmd_close_breaker_boundary() { let mut s = Sim::new(FullPower); s.apply(Command::OpenBreaker); s.turbine.phase_deg = 14.0; s.apply(Command::CloseBreaker); assert!(s.turbine.breaker_closed); }
#[test] fn cmd_close_breaker_transition() { let mut s = Sim::new(FullPower); s.apply(Command::OpenBreaker); s.turbine.phase_deg = 90.0; s.apply(Command::CloseBreaker); assert!(s.turbine.tripped && s.turbine.trip_reason.as_deref() == Some("Out-of-phase synchronisation")); }

#[test] fn cmd_open_breaker_positive() { assert_cmd(FullPower, vec![], Command::OpenBreaker, 0.0, |s| !s.turbine.breaker_closed); }
#[test] fn cmd_open_breaker_negative() { assert_cmd(HotStandby, vec![], Command::OpenBreaker, 0.0, |s| !s.turbine.breaker_closed); }
#[test] fn cmd_open_breaker_boundary() { assert_cmd(FullPower, vec![], Command::OpenBreaker, 2.0, |s| s.turbine.speed_rpm > RATED_RPM, ); }
#[test] fn cmd_open_breaker_transition() { assert_cmd(FullPower, vec![], Command::OpenBreaker, 60.0, |s| logged(s, "sri:") || s.turbine.tripped); }

#[test] fn cmd_set_load_limit_positive() { assert_cmd(FullPower, vec![], Command::SetLoadLimit { pct: 50.0 }, 60.0, |s| s.steam.w_tcv < 0.55 * STEAM_FLOW_RATED); }
#[test] fn cmd_set_load_limit_negative() { assert_cmd(HotStandby, vec![], Command::SetLoadLimit { pct: 50.0 }, 1.0, |s| s.steam.w_tcv == 0.0); }
#[test] fn cmd_set_load_limit_boundary() { assert_cmd(FullPower, vec![], Command::SetLoadLimit { pct: 500.0 }, 0.0, |s| (s.turbine.load_limit - 1.1).abs() < 1e-9); }
#[test] fn cmd_set_load_limit_transition() { assert_cmd(FullPower, vec![Command::SetLoadLimit { pct: 50.0 }], Command::SetLoadLimit { pct: 105.0 }, 120.0, |s| s.steam.w_tcv > 0.9 * STEAM_FLOW_RATED); }

#[test] fn cmd_set_cw_pump_positive() { assert_cmd(FullPower, vec![], Command::SetCwPump { index: 0, on: false }, 0.0, |s| !s.cond.cw_pumps[0]); }
#[test] fn cmd_set_cw_pump_negative() { assert_cmd(FullPower, vec![], Command::SetCwPump { index: 9, on: false }, 0.0, |s| s.cond.cw_pumps.iter().all(|&b| b)); }
#[test] fn cmd_set_cw_pump_boundary() { assert_cmd(FullPower, vec![Command::SetCwPump { index: 0, on: false }], Command::SetCwPump { index: 1, on: false }, 300.0, |s| s.turbine.tripped); }
#[test] fn cmd_set_cw_pump_transition() { assert_cmd(ColdShutdown, vec![], Command::SetCwPump { index: 0, on: true }, 0.0, |s| s.cond.cw_pumps[0]); }

#[test] fn cmd_set_vacuum_pump_positive() { assert_cmd(ColdShutdown, vec![Command::SetCwPump { index: 0, on: true }], Command::SetVacuumPump { on: true }, 1800.0, |s| s.cond.pressure() < 50.0e3); }
#[test] fn cmd_set_vacuum_pump_negative() { assert_cmd(FullPower, vec![], Command::SetVacuumPump { on: false }, 10.0, |s| !s.turbine.tripped); }
#[test] fn cmd_set_vacuum_pump_boundary() { assert_cmd(FullPower, vec![], Command::SetVacuumPump { on: true }, 0.0, |s| s.cond.vacuum_pump); }
#[test] fn cmd_set_vacuum_pump_transition() { assert_cmd(FullPower, vec![], Command::SetVacuumPump { on: false }, 3600.0, |s| s.cond.pressure() > 5.6e3); }

#[test] fn cmd_set_feed_pump_positive() { assert_cmd(FullPower, vec![], Command::SetFeedPump { index: 2, on: true }, 0.0, |s| s.fw.running_pumps() == 3); }
#[test] fn cmd_set_feed_pump_negative() { assert_cmd(ColdShutdown, vec![], Command::SetFeedPump { index: 0, on: true }, 0.0, |s| s.fw.running_pumps() == 0 && has_alarm(s, "fw")); }
#[test] fn cmd_set_feed_pump_boundary() { assert_cmd(FullPower, vec![], Command::SetFeedPump { index: 5, on: false }, 0.0, |s| s.fw.running_pumps() == 2); }
#[test] fn cmd_set_feed_pump_transition() { assert_cmd(FullPower, vec![Command::SetCondensatePump { index: 0, on: false }, Command::SetCondensatePump { index: 1, on: false }], Command::SetFeedPump { index: 0, on: true }, 5.0, |s| s.fw.running_pumps() == 0 && logged(s, "feed pump trip: low suction")); }

#[test] fn cmd_set_condensate_pump_positive() { assert_cmd(HotStandby, vec![], Command::SetCondensatePump { index: 1, on: true }, 0.0, |s| s.fw.cond_pumps[1]); }
#[test] fn cmd_set_condensate_pump_negative() { assert_cmd(HotStandby, vec![], Command::SetCondensatePump { index: 9, on: true }, 0.0, |s| !s.fw.cond_pumps[1]); }
#[test] fn cmd_set_condensate_pump_boundary() { assert_cmd(HotStandby, vec![], Command::SetCondensatePump { index: 0, on: false }, 5.0, |s| s.fw.running_pumps() == 0); }
#[test] fn cmd_set_condensate_pump_transition() { assert_cmd(HotStandby, vec![Command::SetCondensatePump { index: 0, on: false }], Command::SetCondensatePump { index: 0, on: true }, 1.0, |s| s.fw.cond_pumps[0]); }

#[test] fn cmd_set_level_control_positive() { assert_cmd(FullPower, vec![], Command::SetLevelControl { mode: LevelControlMode::SingleElement }, 0.0, |s| s.fw.mode == LevelControlMode::SingleElement); }
#[test] fn cmd_set_level_control_negative() { assert_cmd(FullPower, vec![], Command::SetLevelControl { mode: LevelControlMode::Manual }, 0.0, |s| (s.fw.fcv_manual - s.fw.fcv_pos).abs() < 1e-9, ); }
#[test] fn cmd_set_level_control_boundary() { assert_cmd(FullPower, vec![], Command::SetLevelControl { mode: LevelControlMode::ThreeElement }, 300.0, |s| s.sensors.level_nr.reading.abs() < 10.0); }
#[test] fn cmd_set_level_control_transition() { assert_cmd(FullPower, vec![Command::SetLevelControl { mode: LevelControlMode::Manual }], Command::SetLevelControl { mode: LevelControlMode::ThreeElement }, 300.0, |s| s.sensors.level_nr.reading.abs() < 15.0); }

#[test] fn cmd_set_fcv_manual_positive() { assert_cmd(FullPower, vec![Command::SetLevelControl { mode: LevelControlMode::Manual }], Command::SetFcvManual { pos: 0.0 }, 30.0, |s| s.fw.fcv_pos < 0.01 && s.fw.flow < 1.0); }
#[test] fn cmd_set_fcv_manual_negative() { assert_cmd(FullPower, vec![], Command::SetFcvManual { pos: 0.0 }, 30.0, |s| s.fw.flow > 100.0, ); }
#[test] fn cmd_set_fcv_manual_boundary() { assert_cmd(FullPower, vec![], Command::SetFcvManual { pos: 7.0 }, 0.0, |s| s.fw.fcv_manual == 1.0); }
#[test] fn cmd_set_fcv_manual_transition() { assert_cmd(FullPower, vec![Command::SetLevelControl { mode: LevelControlMode::Manual }, Command::SetFcvManual { pos: 1.0 }], Command::SetLevelControl { mode: LevelControlMode::ThreeElement }, 1.0, |s| s.fw.mode == LevelControlMode::ThreeElement); }

#[test] fn cmd_set_level_setpoint_positive() { assert_cmd(FullPower, vec![], Command::SetLevelSetpoint { cm: 30.0 }, 600.0, |s| (s.sensors.level_nr.reading - 30.0).abs() < 10.0); }
#[test] fn cmd_set_level_setpoint_negative() { assert_cmd(FullPower, vec![Command::SetLevelControl { mode: LevelControlMode::Manual }], Command::SetLevelSetpoint { cm: 30.0 }, 60.0, |s| s.fw.level_sp_cm == 30.0, ); }
#[test] fn cmd_set_level_setpoint_boundary() { assert_cmd(FullPower, vec![], Command::SetLevelSetpoint { cm: 500.0 }, 0.0, |s| s.fw.level_sp_cm == 50.0); assert_cmd(FullPower, vec![], Command::SetLevelSetpoint { cm: -500.0 }, 0.0, |s| s.fw.level_sp_cm == -100.0); }
#[test] fn cmd_set_level_setpoint_transition() { assert_cmd(FullPower, vec![Command::ManualScram], Command::SetLevelSetpoint { cm: 0.0 }, 1.0, |s| s.fw.setdown_cm == -100.0); }

#[test] fn cmd_set_heaters_positive() { assert_cmd(FullPower, vec![], Command::SetHeaters { in_service: false }, 600.0, |s| s.fw.t_feed < T_FW_RATED - 20.0); }
#[test] fn cmd_set_heaters_negative() { assert_cmd(HotStandby, vec![], Command::SetHeaters { in_service: false }, 60.0, |s| !s.scrammed); }
#[test] fn cmd_set_heaters_boundary() { assert_cmd(FullPower, vec![], Command::SetHeaters { in_service: true }, 0.0, |s| s.fw.heaters_in_service); }
#[test] fn cmd_set_heaters_transition() { assert_cmd(FullPower, vec![Command::SetHeaters { in_service: false }], Command::SetHeaters { in_service: true }, 900.0, |s| (s.fw.t_feed - T_FW_RATED).abs() < 10.0); }

#[test] fn cmd_set_rwcu_reject_positive() { assert_cmd(HotStandby, vec![], Command::SetRwcuReject { kgs: 10.0_f64.min(RWCU_REJECT_MAX_KGS) }, 0.0, |s| s.fw.rwcu_reject == 10.0_f64.min(RWCU_REJECT_MAX_KGS)); }
#[test] fn cmd_set_rwcu_reject_negative() { assert_cmd(ColdShutdown, vec![], Command::SetRwcuReject { kgs: 10.0 }, 60.0, |s| s.vessel.mass > 0.0 && s.domain_errors.is_empty()); }
#[test] fn cmd_set_rwcu_reject_boundary() { assert_cmd(HotStandby, vec![], Command::SetRwcuReject { kgs: 99.0 }, 0.0, |s| s.fw.rwcu_reject == RWCU_REJECT_MAX_KGS); }
#[test] fn cmd_set_rwcu_reject_transition() { assert_cmd(HotStandby, vec![Command::SetRwcuReject { kgs: 15.0 }], Command::SetRwcuReject { kgs: 0.0 }, 1.0, |s| s.fw.rwcu_reject == 0.0); }

#[test] fn cmd_set_ic_train_positive() { assert_cmd(FullPower, vec![], Command::SetIcTrain { index: 0, open: true }, 60.0, |s| s.ic[0].heat > 1.0e6); }
#[test] fn cmd_set_ic_train_negative() { assert_cmd(FullPower, vec![], Command::SetIcTrain { index: 9, open: true }, 1.0, |s| s.ic.iter().all(|t| !t.cmd_open)); }
#[test] fn cmd_set_ic_train_boundary() { assert_cmd(ColdShutdown, vec![], Command::SetIcTrain { index: 0, open: true }, 60.0, |s| s.ic[0].heat < 1.0e3, ); }
#[test] fn cmd_set_ic_train_transition() { assert_cmd(FullPower, vec![Command::SetIcTrain { index: 0, open: true }], Command::SetIcTrain { index: 0, open: false }, 60.0, |s| !s.ic[0].cmd_open && s.ic[0].heat < 1.0e6); }

#[test] fn cmd_ack_alarms_positive() { assert_cmd(FullPower, vec![Command::ManualScram], Command::AckAlarms, 0.0, |s| s.snapshot().alarms.iter().all(|a| a.acked)); }
#[test] fn cmd_ack_alarms_negative() { assert_cmd(FullPower, vec![], Command::AckAlarms, 0.0, |s| s.snapshot().alarms.iter().all(|a| a.acked)); }
#[test] fn cmd_ack_alarms_boundary() { let mut s = Sim::new(FullPower); s.apply(Command::ManualScram); s.apply(Command::AckAlarms); s.apply(Command::SetMsiv { line: 0, open: false }); s.apply(Command::SetMsiv { line: 1, open: false }); run(&mut s, 10.0); assert!(s.snapshot().alarms.iter().any(|a| !a.acked), "new alarms arrive unacked"); }
#[test] fn cmd_ack_alarms_transition() { let mut s = Sim::new(FullPower); s.apply(Command::SetIcTrain { index: 0, open: true }); run(&mut s, 30.0); s.apply(Command::AckAlarms); s.apply(Command::SetIcTrain { index: 0, open: false }); s.advance(60.0); s.apply(Command::SetIcTrain { index: 0, open: true }); s.advance(30.0); assert!(s.snapshot().alarms.iter().any(|a| a.id == "ic" && !a.acked), "cleared-then-returned alarm flashes again"); }
```

`neutronics_power` is a free helper in `tests/protection.rs`: `fn neutronics_power(s: &Sim) -> f64 { s.core.kin.n }`.

- [ ] **Step 3: Run**

Run: `cargo test --release -p bwr-core --test protection cmd_`
Expected: 124 tests pass. Any failure is either a test-setup timing problem (adjust `settle_s`, never thresholds) or a defect in an earlier package: open an issue note in `docs/model/coverage.md` and fix it in that package's files with its own failing test before continuing.

- [ ] **Step 4: Commit**

```bash
git add crates/bwr-core/tests/protection.rs
git commit -m "test(protection): positive/negative/boundary/transition coverage for all 31 commands

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 12: Register rows, trace columns, coverage, and baseline regeneration

**Files:**
- Modify: `docs/model/coverage.md`, `docs/validation/cases.csv`, `docs/validation/baseline/*.csv`, `docs/validation/method.md`
- Test: `crates/bwr-core/examples/validation.rs` (existing, P2)

This task follows the master baseline regeneration protocol: (1) compare and save the report under `docs/validation/reports/`, (2) classify every change, (3) pass the package gates, (4) `--write` exactly once, (5) commit report and baselines together. No tolerance is adjusted at any point.

- [ ] **Step 1: Record the before state**

Run: `set -o pipefail; cargo run --release -p bwr-core --example validation 2>&1 | tee /tmp/p8-before.txt` (compare mode is the runner's default; there is no `--compare` flag)
Expected: exit 1 with violations listed for every case whose event timing moved because protection now sees lagged sensors (scram and trip times later by about `LEVEL_SENSOR_TAU_S` or `PRESSURE_SENSOR_TAU_S`). Create `docs/validation/reports/2026-10-21-pkg08.md` with the runner output verbatim under `## Compare run (before --write)` and a `## Classification` table; also summarise it in `docs/validation/method.md` under `## Package 8 baseline shift`:

```markdown
| case | metric | before | after | class | cause |
|---|---|---|---|---|---|
| (one row per violation, filled from the compare output) | | | | physical | sensor lag A-803 / L8 independence T02 / SHUTDOWN signal S08 |
| (every case) | pressure_sensed_pa, level_nr_cm, level_model_cm | 0.0 | populated | schema-population | Step 3 populates the reserved columns; byte-identical traces are not expected |
```

Every row is `physical` (with the reason), `schema-population` or `numerical`; a change that fits none of these is a defect to find, not a tolerance to widen.

- [ ] **Step 2: Gates, then regenerate the baselines once**

The package's independent gates are `cargo test --release -p bwr-core --test protection` (Tasks 2–11) green BEFORE writing. Do Step 3 (the sensed trace columns) before writing as well, so the baselines are written once with the populated columns.

Run: `cargo run --release -p bwr-core --example validation -- --write` (exactly once for this change set)
Then: `cargo run --release -p bwr-core --example validation`
Expected: exit 0, no violations (the second run only shows the runner is deterministic; it is not validation evidence). `git diff --stat docs/validation/baseline` shows only the cases named in the Step 1 table; if another case changed, stop and find out why before committing.

- [ ] **Step 3: Populate the sensed trace columns**

Trace schema v2 (P2) reserves `pressure_sensed_pa`, `level_model_cm` and defines `level_nr_cm` as the sensed value "populated from Package 8". In `crates/bwr-core/src/validation.rs`, in the sampling function that fills a `[f64; N_TRACE_COLS]` row from a `&Sim`, set:

```rust
    row[col_index("pressure_sensed_pa")] = sim.sensors.pressure.reading;
    row[col_index("level_nr_cm")] = sim.sensors.level_nr.reading;
    row[col_index("level_model_cm")] = sim.vessel.level_nr_cm();
```

and make the per-step interval extrema `level_nr_cm_min_int` / `level_nr_cm_max_int` accumulate `sim.sensors.level_nr.reading` (the instrument the trips act on), not the model level. Add to `tests/protection.rs`:

```rust
#[test]
fn trace_carries_sensed_and_model_level() {
    use bwr_core::validation::{col_index, run_case, standard_cases};
    let case = standard_cases().into_iter().find(|c| c.id == "full_power_steady").expect("P2 standard case");
    let tr = run_case(&case, DT);
    let last = tr.rows.last().unwrap();
    assert!(last[col_index("pressure_sensed_pa")] > 6.0e6);
    assert!((last[col_index("level_nr_cm")] - last[col_index("level_model_cm")]).abs() < 5.0);
}
```

Remove the "populated from Package 8" note for these three columns from `docs/validation/method.md`.

Run: `cargo test --release -p bwr-core --test protection trace_carries`
Expected: PASS.

- [ ] **Step 4: Coverage**

Append to `docs/model/coverage.md` under "Protection and instruments": one row per matrix id group:

```markdown
| Scram signals S01–S09 | protection.csv | A-801, A-804, A-806 | tests/protection.rs scram_*, single_msiv_closure_scrams_in_run_without_isolating_other_line |
| Alternate rod insertion R02 | protection.csv | A-805 | ari_disabled_by_default, alternate_rod_insertion_inserts_without_rps_latch |
| Isolation I01–I03 | protection.csv | A-801 | isolation_* |
| IC initiation C01–C03 | protection.csv | A-801 | ic_* |
| Turbine/generator T01–T07 | protection.csv | A-801 | l8_*, turbine_*, breaker_* |
| Rod blocks B01–B06 | protection.csv | A-801 | block_*, refuel_interlock_* |
| Permissives P01–P08 | protection.csv | A-801 | scram_reset_*, msiv_open_*, feed_pump_*, turbine_reset_*, cmd_*_boundary |
| Alarms A01–A25 | protection.csv | A-801 | cmd_* and alarm checks in scenario tests (P9) |
| Sensors (5 instruments) | A-803 | parameters LEVEL_NR_RANGE_CM, LEVEL_WR_RANGE_CM, PRESSURE_SENSOR_RANGE_PA, HEADER_PRESSURE_SENSOR_RANGE_PA, CONDENSER_PRESSURE_SENSOR_RANGE_PA, LEVEL_SENSOR_TAU_S, PRESSURE_SENSOR_TAU_S | sensor_lag_delays_l3_scram, level_nr_out_of_range_flag, level_exactly_at_rail_is_in_range, level_nr_out_of_range_recovers, failed_level_sensor_blinds_l3_scram, isolation_uses_header_sensor_and_turbine_trip_uses_condenser_sensor |
```

- [ ] **Step 5: Full verification**

Run: `cargo test --release -p bwr-core && npm --prefix ui run typecheck && npm --prefix ui run build`
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add docs/model/coverage.md docs/validation docs/validation/reports/2026-10-21-pkg08.md crates/bwr-core/src/validation.rs crates/bwr-core/tests/protection.rs
git commit -m "docs(validation): regenerate baselines after sensor-based protection; sensed trace columns; record shifts

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

## Spec coverage check (Package 8 bullets → tasks)

| Spec bullet | Task |
|---|---|
| Cause-and-effect matrix for every trip, isolation, permissive, block, reset, alarm | 1 |
| Verify startup neutron instrumentation; no legacy assumption; period-based protection only if documented | 2 |
| Correct REFUEL interlock using commanded targets; multiple rods in one command; successive commands | 4 |
| High-water protection when turbine already tripped | 5 |
| Shutdown-mode reset, persistent scram signals, changing modes, IC reinitiation, independent steam lines | 3, 6, 10 |
| Sensor range, lag, saturation, failure where it affects a trip or indication | 7 |
| Displays distinguish actual, commanded, measured, model, out-of-range | 8 |
| Distinct scram initiation paths (master r2: ARI behind `ARI_ENABLED`) | 9 |
| Positive, negative, boundary, transition tests for every control and protective action | 4, 5, 6, 9, 10, 11 |
| No voting architecture claim | 1 (A-802), 12 |

**Acceptance gate (verbatim):** "Every control action and protective action has positive, negative, boundary, and transition tests. No new claim of plant-specific voting architecture is made without evidence."
