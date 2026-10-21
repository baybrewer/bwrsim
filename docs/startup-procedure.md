# BWRX-300 Simulator: Reactor Startup Procedure

Step-by-step operating procedure for taking the simulated plant from **HOT STANDBY** to the generator on line and
power rising, without tripping any protection. Every limit quoted here is the simulator's own setpoint from
[`params.rs`](../crates/bwr-core/src/params.rs) and [`sim.rs`](../crates/bwr-core/src/sim.rs), so the numbers are what
will actually scram you. This is a game procedure, not a real plant procedure.

Every acronym and panel label is defined in the [Glossary](#12-glossary-of-acronyms-and-panel-labels). Every system is
described in [Systems](#13-plant-systems-explained) and every trip or transient in [Events](#14-events-explained), with
links to public references for the real thing. Plant data comes from the
[GE Hitachi BWRX-300 General Description](https://www.gevernova.com/content/dam/gevernova-nuclear/global/en_us/documents/carbon-free-power/005N9751-BWRX-300-General-Description.pdf)
and the sources listed in the [README](../README.md#data-sources).

Timings come from the scripted operator in [`examples/startup.rs`](../crates/bwr-core/examples/startup.rs), which pulls
rods as fast as the period allows. A careful human run takes longer. Run at 1x while pulling rods near criticality and
5x to 10x while waiting on the turbine or xenon.

**Contents**

1. [Limits that will stop you](#1-limits-that-will-stop-you)
2. [Initial conditions: HOT STANDBY](#2-initial-conditions-hot-standby)
3. [Phase A: prepare for swell](#3-phase-a-prepare-for-swell)
4. [Phase B: approach to criticality](#4-phase-b-approach-to-criticality)
5. [Phase C: heat up and hold pressure](#5-phase-c-heat-up-and-hold-pressure)
6. [Phase D: roll the turbine and synchronise](#6-phase-d-roll-the-turbine-and-synchronise)
7. [Phase E: mode switch to RUN](#7-phase-e-mode-switch-to-run-time-critical)
8. [Phase F: power ascent](#8-phase-f-power-ascent)
9. [Phase G: xenon](#9-phase-g-xenon)
10. [If you scram](#10-if-you-scram)
11. [Starting from COLD SHUTDOWN](#11-starting-from-cold-shutdown-not-exercised-by-the-tests)
12. [Glossary of acronyms and panel labels](#12-glossary-of-acronyms-and-panel-labels)
13. [Plant systems explained](#13-plant-systems-explained)
14. [Events explained](#14-events-explained)
15. [References](#15-references)

---

## 1. Limits that will stop you

Know these before touching anything. The first column is the trigger, the second is what the simulator does.
Terms: [APRM](#aprm), [IRM](#irm), [MSIV](#msiv), [L2 to L8](#l1-l2-l3-l4-l7-l8), [RUN / STARTUP](#mode-switch).

### Scram (reactor trip)

A [scram](#scram) drives all 57 rods fully in within 3 s. See [Events: Scram](#scram-1).

| Trigger | Setpoint | Applies in |
|---|---|---|
| APRM high flux, setdown | > 15 % power | STARTUP (and SHUTDOWN / REFUEL) |
| IRM high-high | > 120 % of the selected IRM range | STARTUP |
| APRM high flux | > 118 % power | RUN |
| MSIV closure | either line < 85 % open | RUN |
| Reactor high pressure | > 7.50 MPa | always |
| Reactor low water level L3 | < -100 cm | always |
| Mode switch turned to SHUTDOWN | immediate | always |

### Rod withdrawal blocks (rods stop, no scram)

See [Events: Rod withdrawal block](#rod-withdrawal-block).

| Trigger | Setpoint | Applies in |
|---|---|---|
| Scram not reset | | always |
| Mode switch in SHUTDOWN | all withdrawal blocked | SHUTDOWN |
| IRM upscale | > 108 % of range | STARTUP |
| APRM setdown upscale | > 12 % power | STARTUP |
| APRM upscale | > 108 % power | RUN |

### MSIV isolation (steam lines close, then a scram follows in RUN)

See [Events: MSIV isolation](#msiv-isolation).

| Trigger | Setpoint |
|---|---|
| Low-low water level L2 | < -400 cm |
| Low steam line pressure | < 5.9 MPa at the turbine inlet, **only in RUN** |
| High steam flow | > 140 % rated |

### Turbine and feedwater trips

See [Events: L8 trip](#l8-high-level-trip), [Turbine trip](#turbine-trip), [Overspeed](#turbine-overspeed),
[Out-of-phase synchronisation](#out-of-phase-synchronisation), [Select Rod Insert](#select-rod-insert-sri).

| Trigger | Setpoint | Effect |
|---|---|---|
| High water level L8 | > +100 cm | Turbine trips **and all three feed pumps trip** |
| Low condenser vacuum | > 25 kPa | Turbine trips; bypass valves blocked above 30 kPa |
| Overspeed | > 1980 rpm | Turbine trips |
| Out-of-phase breaker close | phase > 15 deg or slip > 0.25 Hz | Generator lockout, turbine trip |
| Turbine trip or load rejection above 40 % power | | Select Rod Insert fires, a rod group drops in |

### Alarms worth reacting to

| Alarm | Setpoint |
|---|---|
| SHORT PERIOD | [period](#period) < 20 s (panel turns amber below 60 s, red below 30 s) |
| REACTOR WATER LEVEL HIGH L7 | > +50 cm |
| REACTOR WATER LEVEL LOW L4 | < -50 cm |
| REACTOR PRESSURE HIGH | > 7.35 MPa |
| HEATUP / COOLDOWN RATE | > 55 degC/h |
| FEEDWATER TEMPERATURE LOW | feed < 150 degC while above 30 % power |

---

## 2. Initial conditions: HOT STANDBY

[Hot standby](#hot-standby) means the vessel is hot and pressurised but the reactor is shut down. Load the scenario
from the top bar: select **HOT STANDBY**, press **LOAD**, then **CONFIRM**. Verify the plant looks like this before
starting. If anything differs, fix it first.

| Item | Expected |
|---|---|
| [Mode switch](#mode-switch) | STARTUP |
| [RPS](#rps) | RPS NORMAL lit, SCRAM dark, ALL RODS IN lit |
| All 57 [rods](#control-rods-and-fmcrds) | position 00 (full in) |
| RX PRESS | about 6.90 MPa, coolant about 285 degC |
| P SETPT ([EHC](#ehc)) | 6.95 MPa, bypass in AUTO |
| [IRM](#irm) | range 1, reading near zero; [SRM](#srm) a few cps |
| [Period](#period) | infinite or very long |
| [Xenon](#xenon-135-and-iodine-135) | about 27 % of equilibrium (two days after shutdown) |
| Turbine | TRIPPED lit, speed 0 rpm, breaker OPEN |
| [Condenser](#condenser-and-vacuum) | vacuum available, pressure well under 15 kPa, CW pumps and vacuum pump running |
| [Feedwater](#condensate-feedwater-and-level-control) | COND P A and FWP A running, level control 1-ELEM, setpoint 0 cm, FW HEATERS IN, feed temp 150 degC |
| LEVEL [NR](#nr) | near 0 cm, drifting slightly |
| [MSIVs](#msiv) | both open |
| [Isolation condensers](#isolation-condensers-ic) | all three valves closed |

Set the sim speed to **1x** for the rod work that follows.

---

## 3. Phase A: prepare for swell

The core will [swell](#swell-and-shrink) the water level upward as it heats and voids form. Make room for that now,
before it happens.

1. In the FEEDWATER panel, press **SETPT -5** repeatedly until the level setpoint reads **-40 cm**.
   The [level controller](#condensate-feedwater-and-level-control) will slowly lower level toward that value.
2. Leave **RWCU REJECT** at 0 for now. You will use it in Phase C. See [RWCU](#rwcu).
3. Press **ACK** on the [annunciator](#annunciator) to clear any standing alarms so new ones stand out.

Why: L7 alarms at +50 cm and [L8](#l8-high-level-trip) at +100 cm trips the turbine and every feed pump. Starting
from -40 cm gives you 140 cm of margin for swell.

---

## 4. Phase B: approach to criticality

Rods are bottom-entry and read 00 (full in) to 48 (full out) in [notches](#notch) of 2. **INSERT** and **WITHDRAW**
move the selected rod or group one notch. The simulator pulls rods in [banks](#bank): every rod to 08, then every rod
to 16, and so on. That keeps the power shape flat and the [worth](#rod-worth) per notch predictable. Background:
[Events: Criticality](#criticality).

1. Click a rod on the core map to select it, or use **GROUP SELECT ALL** to select all 57.
2. Press **WITHDRAW** once. Wait for the rods to stop moving (POS matches TGT). With all 57 selected this is one
   notch on every rod.
3. Watch three things after every notch, in this order:
   - **PERIOD**. Positive and shortening means you are adding [reactivity](#reactivity). Do not pull again while the
     period is shorter than about **100 s**. If it drops under **60 s**, stop and wait. If it drops under **30 s**,
     press **INSERT** one notch.
   - **SRM** count rate. It climbs slowly at first, then faster as you approach critical. That acceleration is the
     signal that criticality is close ([subcritical multiplication](#criticality)).
   - **ROD WITHDRAWAL BLOCK** in the annunciator. If it appears, read the reason. Nothing moves until it clears.
4. Continue one notch at a time. Expect criticality ([k-eff](#k-eff) reaching 1.00000 and a steady positive period)
   with the rods averaging about **24 of 48** withdrawn. The scripted operator reaches it in roughly **55 minutes** of
   sim time.
5. Once the period is steady and positive, stop pulling. Power will rise on its own. Let it coast up.

**IRM ranging.** Each [IRM](#irm) range covers a factor of about 3.2 in power. When the IRM reads above **80 %**,
click the next range up. Above 108 % you lose rod withdrawal; above 120 % you scram. Ranging is the single most
common way to lose a startup. Do not let the IRM pass 100 %.

| IRM range | Full scale (% power) | Reads 80 % at |
|---|---|---|
| 5 | 0.13 | 0.1 % |
| 6 | 0.4 | 0.32 % |
| 7 | 1.3 | 1.0 % |
| 8 | 4.0 | 3.2 % |
| 9 | 12.6 | 10 % |
| 10 | 40 | 32 % |

Range 10 is the top. At 6 % power on range 9 the IRM reads about 50 %, which is where the scripted operator rolls
the turbine. In STARTUP the [APRM](#aprm) setdown limits (12 % block, 15 % scram) bite before the IRM does on
range 10, so the mode switch to RUN in Phase E is what actually lets you climb past 12 %.

---

## 5. Phase C: heat up and hold pressure

Power is now a few tenths of a percent and rising. [Reactivity feedback](#reactivity-feedback-doppler-void-moderator)
changes character here: Doppler and voids turn negative as power climbs, so the period will lengthen by itself and
you will need to pull more rods to keep climbing. Hold power at about **6 %** for the turbine roll.

1. Keep pulling single notches when the period lengthens beyond about 150 s and power is below 6 %. Stop when
   power reaches 6 %.
2. Watch **RX PRESS**. The [EHC pressure regulator](#ehc-pressure-regulator-and-turbine-bypass) holds 6.95 MPa by
   opening the **BYPASS** valves to the condenser as steam production grows. Bypass position creeping up from 0 is
   normal and means the regulator is working.
3. Watch **LEVEL NR**. As voids form the level [swells](#swell-and-shrink) upward. Hold it below **+20 cm**:
   - Move the **RWCU REJECT** slider up (the panel slider goes to 10 kg/s) when level climbs past +20 cm.
   - Bring it back to 0 when level is below +20 cm. The scripted operator toggles between full reject and 0 using
     that threshold.
   - If level reaches **+50 cm** you will get the L7 alarm. Act on it. At **+100 cm** you lose the turbine and all
     feed pumps and the startup is effectively over.
4. Do not raise the pressure setpoint. Raising pressure collapses voids, which adds reactivity and raises power.
   See [Events: Pressure rise and void collapse](#pressure-rise-and-void-collapse).
5. Keep **FW HEATERS IN**. Cold feedwater adds reactivity and is a source of power swings. See
   [Events: Loss of feedwater heating](#loss-of-feedwater-heating).

---

## 6. Phase D: roll the turbine and synchronise

Conditions to start: power above about **3.5 %** and reactor pressure above **6.9 MPa**. The turbine can only take
steam the regulator is already passing through the bypass, so rolling it will not drag pressure down. Background:
[Turbine-generator](#turbine-generator-and-synchronisation).

1. Confirm **CONDENSER** pressure is below **15 kPa**. RESET is blocked above 25 kPa and bypass is blocked above
   30 kPa. See [Condenser and vacuum](#condenser-and-vacuum).
2. Confirm LEVEL NR is below +100 cm. RESET is blocked above L8.
3. Press **RESET** in the TURBINE group. TRIPPED goes dark and LATCHED lights.
4. Press **SPEED SP 1800**. The [governor](#turbine-generator-and-synchronisation) ramps the reference at 5 rpm/s,
   so the shaft takes about **6 minutes** to reach 1800 rpm. Steam diverts from the bypass to the turbine as it
   accelerates. Sim speed 5x is fine here.
5. Watch level during the roll. The scripted run saw a swell to **+55 cm** at this point. Use RWCU reject early.
6. The [synchroscope](#synchroscope) wakes up above 1500 rpm, but the breaker will only close within 20 rpm of 1800.
   Watch the needle:
   - It must be turning **slowly** ([slip](#slip) under 0.25 Hz) and pointing **near 12 o'clock** (within 15 degrees).
   - Aim for within 5 degrees and almost stationary. Press **CLOSE** on the generator breaker ([BKR](#bkr)) as the
     needle passes 12 o'clock.
7. **BKR CLOSED** lights and NET MWe goes positive. If you get an out-of-phase message, the turbine has tripped
   on generator lockout. Go back to step 3 and wait for a better moment next time. See
   [Events: Out-of-phase synchronisation](#out-of-phase-synchronisation).

After the breaker closes the plant is in [reactor leads](#reactor-leads) mode. The pressure regulator positions the
turbine control valves ([TCV](#tcv)), and electrical load follows reactor power. You do not set load directly.
LOAD LIM at 105 % is the ceiling and needs no change.

---

## 7. Phase E: mode switch to RUN (time critical)

With the generator on line, move the [mode switch](#mode-switch) **immediately**, before power passes 12 %.

1. Click **RUN** on the mode switch.
2. Confirm the IRM bar now reads **(BYPASSED IN RUN)**.

Why the hurry: in STARTUP the APRM setdown blocks rods at 12 % and scrams at 15 %, and the IRM is pinned above
100 % on range 10. In RUN those go away and the APRM limits become 108 % block and 118 % scram.

What RUN arms against you: the **low steam line pressure MSIV isolation at 5.9 MPa**. If pressure sags below that,
both MSIVs close, which scrams the reactor and auto-starts the isolation condensers. Keep pressure up. Never lower
the pressure setpoint below about 6.2 MPa in RUN. See [Events: MSIV isolation](#msiv-isolation).

---

## 8. Phase F: power ascent

1. Resume pulling rods a notch at a time with the same period discipline. Reactor power drives turbine load.
2. At about **20 % power** (the scripted operator's point; one feed pump [runs out](#runout) at 400 kg/s, which is
   roughly 85 % power, so there is no rush beyond having margin):
   - Press **COND P B RUN**.
   - Press **FWP B RUN**. A feed pump will not start without a condensate pump running.
   - Switch the level controller to **3-ELEM**. [Three-element control](#condensate-feedwater-and-level-control)
     uses steam flow and feed flow and tracks level far better through transients.
3. Raise the level setpoint back toward **0 cm** in +5 steps once level is stable and well under L7.
4. Keep an eye on **FEED TEMP**. [Feedwater heating](#feedwater-heaters) comes from turbine extraction and rises
   with load. If it reads under 150 degC above 30 % power you get a low temperature warning, and the cold feed is
   adding reactivity.
5. Bring **RWCU REJECT** back to 0 once level control is steady in 3-element.
6. Continue to your target power. Full power is 870 [MWt](#mwt-and-mwe), about 300 MWe net, 7.2 MPa dome pressure.

The scripted operator reaches 40 % power and 82 MWe net about **75 minutes** after starting from hot standby.

---

## 9. Phase G: xenon

Hot standby starts with about 27 % of equilibrium [xenon](#xenon-135-and-iodine-135). As the core runs at power,
xenon builds toward equilibrium over roughly the next day of sim time and eats reactivity (equilibrium worth is
2.8 % [dk/k](#dkk-and-pcm), the largest single poison in the model). Power will sag unless you keep withdrawing rods.
Check every 30 to 60 minutes of sim time at 10x or 30x and pull a notch when power drifts down. Watch the XENON
readout climb toward 100 %.

If you reduce power, xenon first **rises** (iodine decays into it faster than it burns out) before it falls. That
is the xenon peak, also called the [iodine pit](https://en.wikipedia.org/wiki/Iodine_pit). Expect to need extra rod
withdrawal for several hours after any power reduction, and extra insertion later when it burns off. See
[Events: Xenon transient](#xenon-transient).

---

## 10. If you scram

1. Rods insert hydraulically in 3 s. The level setpoint sets down by 100 cm automatically ([setdown](#setdown)).
2. Find the cause in the RPS readout under the SCRAM lamp and in the annunciator. Fix it (re-range the IRM, lower
   pressure, restore level).
3. Wait at least **10 s**, then press **SCRAM RESET**. Reset is blocked while any scram signal is still active.
4. If the MSIVs closed, clear the isolation cause and reopen them. If the [isolation condensers](#isolation-condensers-ic)
   started, they will keep cooling the vessel until you close their valves.
5. If the turbine tripped, you must RESET and re-synchronise. If it tripped on L8, all feed pumps also tripped:
   restart COND P A, then FWP A, before level falls to L3.
6. Return to Phase B. Xenon will be higher than it was, so expect to need more rods out.

---

## 11. Starting from COLD SHUTDOWN (not exercised by the tests)

The [cold shutdown](#cold-shutdown) scenario is 30 days after shutdown, 40 degC, vessel vented to atmosphere with the
[RPV](#rpv) head vent open, mode switch in SHUTDOWN, no condenser vacuum, no pumps running. The integration tests
only cover the hot standby startup, so treat this as a sketch rather than a validated path.

1. Start **CW PUMP A** and **CW PUMP B**, then the **VACUUM PUMP**. Vacuum pulls in with a time constant of about
   10 minutes. Wait for condenser pressure under 15 kPa.
2. Start **COND P A**, then **FWP A**. Put the level controller in **1-ELEM** with setpoint 0 cm.
3. Turn the mode switch to **STARTUP**. Withdrawal is blocked in SHUTDOWN. Do not turn it to SHUTDOWN again
   later without meaning to: that is itself a scram signal.
4. Pull rods per Phase B to reach criticality and a low power level. Nuclear heat does the heating; there is no
   external heating source in the model.
5. Hold the [heatup rate](#heatup-rate) under **55 degC/h** by limiting power. Watch the HEATUP RATE readout.
6. **Close the RPV HEAD VENT** before pressure rises above 0.2 MPa, or you get the vent-open warning and keep
   venting inventory to the condenser.
7. Once pressure reaches the 6.95 MPa setpoint the bypass valves take over. Continue from Phase C.

---

## 12. Glossary of acronyms and panel labels

Alphabetical. Items marked **(sim)** are names this simulator uses that have no exact public counterpart.

### ACK
Acknowledge. The [annunciator](#annunciator) button that stops alarm tiles flashing. Alarms stay lit until their
cause clears. [Annunciator panel (Wikipedia)](https://en.wikipedia.org/wiki/Annunciator_panel).

### APRM
Average Power Range Monitor. The neutron instrument that reads core power from about 1 % to 125 % by averaging many
in-core detectors. Drives the high-flux scram and the rod blocks. See [Nuclear instrumentation](#nuclear-instrumentation-srm-irm-aprm).
[NRC: SRM, IRM and APRM systems (PDF)](https://www.nrc.gov/docs/ML1705/ML17056A740.pdf).

### BANK
A group of rods moved together to the same position so the core power shape stays flat. The simulator's scripted
operator pulls every rod to 08, then every rod to 16, and so on. [Control rod (Wikipedia)](https://en.wikipedia.org/wiki/Control_rod).

### BKR
Generator output circuit breaker. CLOSED ties the generator to the grid; OPEN disconnects it.
[Circuit breaker (Wikipedia)](https://en.wikipedia.org/wiki/Circuit_breaker).

### BWR
Boiling Water Reactor. A reactor whose coolant boils in the core and whose steam goes directly to the turbine.
[NRC: Boiling Water Reactors](https://www.nrc.gov/reactors/power/bwrs.html),
[Wikipedia](https://en.wikipedia.org/wiki/Boiling_water_reactor).

### BWRX-300
GE Hitachi's 300 MWe natural-circulation small modular BWR, the plant this simulator models.
[GE Vernova product page](https://www.gevernova.com/nuclear/carbon-free-power/bwrx-300-small-modular-reactor),
[General Description 005N9751 (PDF)](https://www.gevernova.com/content/dam/gevernova-nuclear/global/en_us/documents/carbon-free-power/005N9751-BWRX-300-General-Description.pdf),
[Wikipedia](https://en.wikipedia.org/wiki/BWRX-300).

### BYP AUTO / MAN
Turbine bypass valve mode. AUTO lets the [EHC](#ehc) pressure regulator position the bypass; MAN holds the slider
position. See [EHC pressure regulator and turbine bypass](#ehc-pressure-regulator-and-turbine-bypass).

### COLD SHUTDOWN
Reactor shut down, depressurised and below 100 degC. In the simulator: 30 days after shutdown, 40 degC, vented.
[NRC glossary: cold shutdown](https://www.nrc.gov/reading-rm/basic-ref/glossary/cold-shutdown.html).

### COND P A / B
Condensate pumps. They take water from the condenser [hotwell](#hotwell) and feed the suction of the feed pumps. A
feed pump will not start unless a condensate pump is running.
[Boiler feedwater pump (Wikipedia)](https://en.wikipedia.org/wiki/Boiler_feedwater_pump).

### cps
Counts per second, the unit on the [SRM](#srm) meter. [Counts per minute (Wikipedia)](https://en.wikipedia.org/wiki/Counts_per_minute).

### CW PUMP A / B
Circulating water pumps. They push cooling water through the condenser tubes. Both are needed for full vacuum.
[Surface condenser (Wikipedia)](https://en.wikipedia.org/wiki/Surface_condenser).

### degC/h
Degrees Celsius per hour, the unit of the [heatup rate](#heatup-rate) readout.

### dk/k and pcm
Reactivity units. dk/k is the fractional change in [k-eff](#k-eff); 1 % dk/k = 1000 pcm (per cent mille). The panel's
REACTIVITY table is in pcm. [Reactivity (Wikipedia)](https://en.wikipedia.org/wiki/Reactivity_(nuclear)).

### EHC
Electro-Hydraulic Control. The turbine control system that holds reactor pressure by positioning the turbine control
valves and bypass valves, and governs turbine speed before synchronisation. See
[EHC pressure regulator and turbine bypass](#ehc-pressure-regulator-and-turbine-bypass).
[NRC: Process instrumentation and control, EHC and feedwater control (PDF)](https://www.nrc.gov/docs/ML0228/ML022830833.pdf).

### 1-ELEM / 3-ELEM
Single-element and three-element reactor water level control. See
[Condensate, feedwater and level control](#condensate-feedwater-and-level-control).

### ESBWR
Economic Simplified Boiling Water Reactor, GE's 1500 MWe natural-circulation design that the BWRX-300 descends from.
Several ASSUMED parameters in the simulator are scaled from it.
[Wikipedia](https://en.wikipedia.org/wiki/Economic_Simplified_Boiling_Water_Reactor).

### FCV
Feedwater control valve. The valve the level controller moves to set feed flow. FCV POS is its position, FCV MAN is
the manual demand used in MANUAL mode. See [Condensate, feedwater and level control](#condensate-feedwater-and-level-control).

### FMCRD
Fine Motion Control Rod Drive. The electric drive under each rod that moves it in small steps, with a hydraulic
backup for [scram](#scram). The BWRX-300 has 57. [Control rod (Wikipedia)](https://en.wikipedia.org/wiki/Control_rod).

### FW HEATERS IN / OUT
Feedwater heaters in or out of service. See [Feedwater heaters](#feedwater-heaters).

### FWP A / B / C
Feedwater pumps. Three 50 % pumps that push condensate into the reactor vessel against reactor pressure.
[Boiler feedwater pump (Wikipedia)](https://en.wikipedia.org/wiki/Boiler_feedwater_pump).

### GE / GEH / GE Vernova
General Electric, GE Hitachi Nuclear Energy, and GE's energy spin-off, the designers of the BWRX-300.
[GE Hitachi Nuclear Energy (Wikipedia)](https://en.wikipedia.org/wiki/GE_Hitachi_Nuclear_Energy).

### GNF2
Global Nuclear Fuel's 10x10 BWR fuel bundle. The BWRX-300 core holds 240 of them.
[Nuclear fuel (Wikipedia)](https://en.wikipedia.org/wiki/Nuclear_fuel).

### HOT STANDBY
Reactor shut down but hot and at operating pressure, ready to restart. In the simulator: 6.9 MPa, two days after
shutdown, condenser on line. Term defined in the [NRC glossary](https://www.nrc.gov/reading-rm/basic-ref/glossary.html).

### HOTWELL
The water reservoir at the bottom of the condenser that collects condensed steam. Condensate pumps draw from it.
[Hotwell (Wikipedia)](https://en.wikipedia.org/wiki/Hotwell).

### Hz
Hertz. Grid frequency is 60 Hz, which is 1800 rpm for this four-pole generator.
[Utility frequency (Wikipedia)](https://en.wikipedia.org/wiki/Utility_frequency).

### IC
Isolation condenser. See [Isolation condensers (IC)](#isolation-condensers-ic).

### IRM
Intermediate Range Monitor. The ten-range neutron instrument that covers the gap between the [SRM](#srm) and the
[APRM](#aprm). Its range switch is on the panel. See [Nuclear instrumentation](#nuclear-instrumentation-srm-irm-aprm).
[NRC GE Systems Technology Manual, chapter 5.2 Intermediate Range Monitor (PDF)](https://www.nrc.gov/docs/ML1125/ML11258A331.pdf).

### k-eff
Effective neutron multiplication factor. Below 1 the chain reaction dies out, at 1 it is self-sustaining (critical),
above 1 power rises. [Nuclear chain reaction (Wikipedia)](https://en.wikipedia.org/wiki/Nuclear_chain_reaction),
[Six factor formula (Wikipedia)](https://en.wikipedia.org/wiki/Six_factor_formula).

### kg/s
Kilograms per second, the unit for every flow readout (steam, feed, RWCU reject). Rated steam flow is 470 kg/s.

### L1, L2, L3, L4, L7, L8
Reactor water level trip points, numbered from lowest to highest, measured in cm from [NWL](#nwl). Standard BWR
convention. In the simulator: L8 +100 (turbine and feed pump trip), L7 +50 (high alarm), L4 -50 (low alarm),
L3 -100 (scram), L2 -400 (isolation and IC start), L1 -600 (unused). See [Reactor vessel and water level](#reactor-vessel-and-water-level).

### LOAD LIM
Generator load limiter, a ceiling on turbine steam demand as a percentage of rated. Default 105 %.
[Governor (Wikipedia)](https://en.wikipedia.org/wiki/Governor_(device)).

### MODE SWITCH
The reactor mode switch with four positions, SHUTDOWN, REFUEL, STARTUP and RUN, which selects which protection
and interlock set is active. See [Mode switch](#mode-switch-1).

### MPa
Megapascal, the pressure unit. 7.2 MPa is rated dome pressure (about 1044 psi).
[Pascal (Wikipedia)](https://en.wikipedia.org/wiki/Pascal_(unit)).

### MSIV
Main Steam Isolation Valve. One fast-closing valve on each of the two steam lines that isolates the reactor from
the turbine building. See [Main steam and MSIVs](#main-steam-and-msivs).

### MWt and MWe
Megawatts thermal (heat from the core, 870 at rated) and megawatts electric (generator output, about 300 net).
GROSS is the generator terminals; NET subtracts house loads such as the pumps.
[Watt (Wikipedia)](https://en.wikipedia.org/wiki/Watt).

### NOTCH
One step of rod travel, 2 positions out of 48. INSERT and WITHDRAW move one notch.

### NR
Narrow range. LEVEL NR is the narrow-range water level instrument, in cm relative to [NWL](#nwl). It is the level
the control systems use.

### NRC
United States Nuclear Regulatory Commission, whose public training manuals and glossary are linked throughout.
[NRC glossary](https://www.nrc.gov/reading-rm/basic-ref/glossary.html).

### NWL
Normal water level, the zero point of the LEVEL NR scale. In the simulator it is 3 m above the chimney top.

### ONR
UK Office for Nuclear Regulation, which is assessing the BWRX-300.
[ONR BWRX-300 GDA Step 2 summary report (PDF)](https://www.onr.org.uk/media/taufoxsv/bwrx-300-gda-step-2-summary-report.pdf).

### P SETPT
EHC pressure setpoint, the reactor pressure the regulator holds. Default 6.95 MPa, which gives 7.2 MPa in the dome at
rated steam flow after the steam line pressure drop.

### PERIOD
Reactor period, the time for power to change by a factor of e (2.718). Shorter is faster. Positive means rising,
negative means falling. See [Nuclear instrumentation](#nuclear-instrumentation-srm-irm-aprm).
[Reactor period (Wikipedia)](https://en.wikipedia.org/wiki/Reactor_period).

### POS / TGT
Rod position and rod target. Rods move toward TGT at 0.4 positions per second.

### RPS
Reactor Protection System. The logic that detects unsafe conditions and scrams the reactor. See
[Reactor protection system (RPS)](#reactor-protection-system-rps).
[Wikipedia](https://en.wikipedia.org/wiki/Reactor_protection_system).

### RPV
Reactor Pressure Vessel. The steel vessel holding the core, chimney, separators and dryer. See
[Reactor vessel and water level](#reactor-vessel-and-water-level).
[Wikipedia](https://en.wikipedia.org/wiki/Reactor_pressure_vessel).

### RPV HEAD VENT
A vent at the top of the vessel used to release air and steam when the vessel is depressurised. Open it only below
0.2 MPa.

### rpm
Revolutions per minute. The turbine runs at 1800 rpm synchronised to a 60 Hz grid.

### RUNOUT
The maximum flow a pump can deliver before its head collapses. One feed pump runs out at 400 kg/s.

### RWCU
Reactor Water Cleanup. A system that draws water from the vessel, cleans it, and normally returns it. Its reject
line sends water to the condenser instead, which is how the operator lowers vessel inventory. See [RWCU](#rwcu-1).

### RX PRESS
Reactor pressure, measured in the steam dome.

### SCRAM
Rapid automatic or manual shutdown by full rod insertion.
[NRC glossary: scram](https://www.nrc.gov/reading-rm/basic-ref/glossary/scram.html),
[Wikipedia](https://en.wikipedia.org/wiki/Scram).

### SETDOWN
An automatic lowering of a setpoint. The APRM scram setpoint sets down to 15 % outside RUN. The level setpoint sets
down 100 cm after a scram to counter the post-scram level swing.

### SETPT / SP
Setpoint, the value a controller is told to hold.

### SMR
Small Modular Reactor. [Wikipedia](https://en.wikipedia.org/wiki/Small_modular_reactor).

### SRI
Select Rod Insert. See [Events: Select Rod Insert](#select-rod-insert-sri).

### SRM
Source Range Monitor. The counting-type neutron detector that sees the shutdown source level and the approach to
critical. Reads in cps on a log scale. See [Nuclear instrumentation](#nuclear-instrumentation-srm-irm-aprm).

### SWELL / SHRINK
Water level rising or falling because the amount of steam in the water changes, not because mass changed. See
[Events: Swell and shrink](#swell-and-shrink).

### TAF
Top of Active Fuel. ABOVE TAF is how much water sits over the fuel. If it reaches zero the core is uncovered.

### TCV
Turbine control valves. The valves the EHC positions to admit steam to the turbine. [TCV](#tcv) readout is their
position. See [EHC pressure regulator and turbine bypass](#ehc-pressure-regulator-and-turbine-bypass).

### Xe-135 and I-135
Xenon-135 and iodine-135, the fission-product poison and its parent. See [Xenon-135 and iodine-135](#xenon-135-and-iodine-135).

---

## 13. Plant systems explained

Each entry says what the real system does, what the simulator models, and where it shows up on the panels. For a
general BWR primer, read the [NRC Reactor Concepts Manual: Boiling Water Reactor Systems (PDF)](https://www.nrc.gov/docs/ML1209/ML120970422.pdf)
and the [NRC GE Systems Technology Manual, chapter 2: BWR primary and auxiliary systems (PDF)](https://www.nrc.gov/docs/ML1127/ML11271A021.pdf).

### Reactor core, fuel and reactivity
The core is 240 GNF2 bundles, 3.81 m of active fuel, generating 870 MWt at rated power. The simulator uses
[point kinetics](https://en.wikipedia.org/wiki/Nuclear_reactor_physics) with six
[delayed neutron](https://en.wikipedia.org/wiki/Delayed_neutron) groups, a neutron source, and a lumped fuel
temperature. Thermal power lags neutron power through the fuel heat capacity, and
[decay heat](https://www.nrc.gov/reading-rm/basic-ref/glossary/decay-heat.html) from fission products continues after
shutdown (7.7 % at the moment of scram, 1.3 % after an hour). Panel: NEUTRON PWR, THERMAL, DECAY HEAT, FUEL TEMP,
K-EFF, REACTIVITY table. Reference: [Nuclear reactor core (Wikipedia)](https://en.wikipedia.org/wiki/Nuclear_reactor_core).

### Control rods and FMCRDs
57 bottom-entry cruciform rods driven by [fine-motion drives](#fmcrd). Each rod has a radial weight (centre rods are
worth more) and an axial worth curve, and total worth is 20 % dk/k. Withdrawal is interlocked by the
[rod blocks](#rod-withdrawal-block). The core map shows each rod coloured by position; click to select, shift-click to
build a group. [Control rod (Wikipedia)](https://en.wikipedia.org/wiki/Control_rod),
[NRC glossary: control rod](https://www.nrc.gov/reading-rm/basic-ref/glossary/control-rod.html).

### Reactivity feedback: Doppler, void, moderator
Three feedbacks push reactivity around without anyone touching a rod:
- **Doppler.** Hotter fuel absorbs more neutrons in U-238 resonances. Negative, fast, and the thing that makes
  the period lengthen as power rises. [Doppler broadening (Wikipedia)](https://en.wikipedia.org/wiki/Doppler_broadening).
- **Void.** Steam bubbles moderate less than water, so more voids means less reactivity. Negative and strong in a
  BWR (-8 % dk/k per unit void fraction here). Anything that changes voids, such as pressure, feedwater temperature or
  flow, moves power. [Void coefficient (Wikipedia)](https://en.wikipedia.org/wiki/Void_coefficient).
- **Moderator temperature.** Hotter water is less dense and moderates less. Negative and slow.
  [Temperature coefficient (Wikipedia)](https://en.wikipedia.org/wiki/Temperature_coefficient).
The REACTIVITY table shows each term in pcm. [Reactivity (NRC glossary)](https://www.nrc.gov/reading-rm/basic-ref/glossary/reactivity.html).

### Xenon-135 and iodine-135
Xe-135 is a fission product with an enormous neutron absorption cross-section. It is produced mostly from the decay
of I-135 (6.6 h half-life) and removed by burning out in the flux and by its own decay (9.2 h half-life). At steady
power the two balance; after a power drop production continues while burnout stops, so xenon peaks several hours
later. Equilibrium worth here is 2.8 % dk/k. Panel: XENON and IODINE readouts as a percentage of full-power
equilibrium. [Xenon-135 (Wikipedia)](https://en.wikipedia.org/wiki/Xenon-135),
[Iodine pit (Wikipedia)](https://en.wikipedia.org/wiki/Iodine_pit),
[Neutron poison (Wikipedia)](https://en.wikipedia.org/wiki/Neutron_poison).

### Nuclear instrumentation: SRM, IRM, APRM
Three overlapping instruments cover eleven decades of power:
- **SRM** counts individual neutrons near the shutdown source level. Log scale in cps.
- **IRM** has ten ranges, each 3.2 times the last, from well below 0.001 % to 40 % of rated. The operator must
  up-range by hand as power rises. Its trips (108 % block, 120 % scram) are bypassed in RUN.
- **APRM** reads percent power. Its scram is 15 % outside RUN (setdown) and 118 % in RUN.
The **PERIOD** meter is derived from the rate of change of neutron power. The simulator's SRM is a simple function
of neutron power and does not model detector withdrawal. [Neutron detection (Wikipedia)](https://en.wikipedia.org/wiki/Neutron_detection),
[NRC: SRM, IRM and APRM (PDF)](https://www.nrc.gov/docs/ML1705/ML17056A740.pdf),
[NRC: IRM chapter (PDF)](https://www.nrc.gov/docs/ML1125/ML11258A331.pdf).

### Reactor protection system (RPS)
The RPS compares plant signals against trip setpoints and scrams the reactor when any one is exceeded. It also
arms and disarms trips with the mode switch. The simulator's RPS lamp block is on the top bar; the scram reason
shows under it. Scram reset requires a 10 s delay and all signals clear. See [section 1](#1-limits-that-will-stop-you).
[Reactor protection system (Wikipedia)](https://en.wikipedia.org/wiki/Reactor_protection_system),
[Nuclear reactor safety system (Wikipedia)](https://en.wikipedia.org/wiki/Nuclear_reactor_safety_system).

### Mode switch
A keylock switch that defines the plant state and which interlocks apply:
- **SHUTDOWN**: all rod withdrawal blocked; turning the switch here is itself a scram.
- **REFUEL**: only one rod may be withdrawn at a time.
- **STARTUP**: IRM trips and APRM setdown active; low steam pressure isolation not armed.
- **RUN**: IRM bypassed, APRM at full setpoints, low steam pressure MSIV isolation armed.
Described in the [NRC Reactor Concepts Manual (PDF)](https://www.nrc.gov/docs/ML1209/ML120970422.pdf).

### Reactor vessel and water level
The [RPV](#rpv) is about 4 m wide and 26.7 m tall. From the bottom: lower plenum, core, a 9 m chimney that drives
natural circulation, the separator region where the water surface sits, the dryer, and the steam dome. The simulator
runs a full mass and energy balance with [IAPWS-IF97 steam properties](https://en.wikipedia.org/wiki/IAPWS), so
pressure, flashing, swell and shrink come out of the physics. Water level is measured in cm from [NWL](#nwl) and
compared against the [L1 to L8](#l1-l2-l3-l4-l7-l8) setpoints. Panel: RX PRESS, LEVEL NR, ABOVE TAF, HEATUP RATE,
core flow, voids, exit quality. [Reactor pressure vessel (Wikipedia)](https://en.wikipedia.org/wiki/Reactor_pressure_vessel),
[Steam separator (Wikipedia)](https://en.wikipedia.org/wiki/Steam_separator),
[Steam dryer (Wikipedia)](https://en.wikipedia.org/wiki/Steam_dryer).

### Natural circulation
The BWRX-300 has no recirculation pumps. Water circulates because the steam-water mixture in the core and chimney is
lighter than the solid water in the downcomer. The tall chimney makes that density head large enough for about
2000 kg/s at rated power. Flow follows power with a lag, and stops if the water level falls below the chimney top.
There is no flow control: you cannot change power with pumps, only with rods, pressure, level and feed temperature.
[Natural circulation (Wikipedia)](https://en.wikipedia.org/wiki/Natural_circulation),
[Passive nuclear safety (Wikipedia)](https://en.wikipedia.org/wiki/Passive_nuclear_safety).

### Main steam and MSIVs
Two steam lines carry steam from the dome to the turbine building through the [MSIVs](#msiv), which stroke closed in
4 s on an isolation signal. Steam line pressure drop is 0.2 MPa at rated flow, which is why the EHC setpoint of
6.95 MPa produces 7.2 MPa in the dome. Closed MSIVs are latched until the operator reopens them, and reopening is
blocked while an isolation signal persists. Panel: MSIV position on the plant mimic.
[NRC GE Systems Technology Manual, chapter 2 (PDF)](https://www.nrc.gov/docs/ML1127/ML11271A021.pdf).

### EHC pressure regulator and turbine bypass
In a BWR the turbine does not set reactor pressure; the pressure regulator does. It measures pressure, compares it
to P SETPT, and generates a steam demand that rises 100 % per 0.25 MPa of error. That demand goes first to the
turbine control valves ([TCV](#tcv)) and, when the turbine cannot take it (tripped, rolling, or load-limited), to the
bypass valves, which dump steam straight to the condenser. Bypass capacity here is 100 % of rated steam flow, so a
turbine trip at full power can be ridden through. The bypass is blocked if condenser pressure exceeds 30 kPa.
Panel: P SETPT, BYP AUTO/MAN, TCV and BYPASS positions, STM->TURB.
[NRC: EHC system description (PDF)](https://www.nrc.gov/docs/ML0228/ML022830833.pdf).

### Turbine-generator and synchronisation
A single 1800 rpm steam turbine drives a four-pole synchronous generator on a 60 Hz grid. Before the breaker closes,
the speed governor ramps the speed reference at 5 rpm/s and admits only as much steam as needed to accelerate the
shaft. The generator breaker may close only when speed is within 20 rpm of 1800, the phase angle between generator
and grid is within 15 degrees, and the slip is under 0.25 Hz. After closing, the generator is locked to grid
frequency and the governor hands over to the pressure regulator ([reactor leads](#reactor-leads)). Electrical output
equals turbine steam power minus losses. Reverse power for 3 s opens the breaker. Panel: TURBINE group, GENERATOR
group, [synchroscope](#synchroscope). [Steam turbine (Wikipedia)](https://en.wikipedia.org/wiki/Steam_turbine),
[Governor (Wikipedia)](https://en.wikipedia.org/wiki/Governor_(device)),
[Synchronous generator (Wikipedia)](https://en.wikipedia.org/wiki/Synchronous_generator),
[Synchronization, AC (Wikipedia)](https://en.wikipedia.org/wiki/Synchronization_(alternating_current)),
[Electrical grid (Wikipedia)](https://en.wikipedia.org/wiki/Electrical_grid).

### Synchroscope
A dial whose needle shows the phase angle between generator and grid voltage. Rotation speed is the [slip](#slip);
12 o'clock is zero phase. Close the breaker when the needle is near 12 and barely moving.
[Synchroscope (Wikipedia)](https://en.wikipedia.org/wiki/Synchroscope).

### Slip
The frequency difference between generator and grid, in Hz. Speed of 1800 rpm is 60 Hz; every 30 rpm off is 1 Hz
of slip. [Utility frequency (Wikipedia)](https://en.wikipedia.org/wiki/Utility_frequency).

### Reactor leads
The BWR control philosophy once on line: the reactor sets the power, the pressure regulator opens the turbine valves
to hold pressure, and the generator output follows. Compare "turbine leads" in a pressurised water reactor, where the
operator sets turbine load and the reactor follows. Described in the
[NRC Reactor Concepts Manual (PDF)](https://www.nrc.gov/docs/ML1209/ML120970422.pdf).

### Condenser and vacuum
Exhaust steam from the turbine and the bypass condenses on tubes cooled by circulating water from the CW pumps. The
condensed water collects in the [hotwell](#hotwell). Low pressure (vacuum) in the condenser is what makes the turbine
efficient; a vacuum pump removes air that leaks in. The simulator trips the turbine above 25 kPa and blocks the bypass
above 30 kPa. Rated condenser pressure is about 5.6 kPa. Panel: CONDENSER kPa, HOTWELL %, CW PUMP A/B, VACUUM PUMP.
[Surface condenser (Wikipedia)](https://en.wikipedia.org/wiki/Surface_condenser),
[Vacuum pump (Wikipedia)](https://en.wikipedia.org/wiki/Vacuum_pump).

### Condensate, feedwater and level control
Two condensate pumps lift water from the hotwell; three 50 % feed pumps push it into the vessel; the feedwater
control valve ([FCV](#fcv)) meters it. The level controller has three modes:
- **MANUAL**: the operator sets FCV position directly with the FCV MAN slider.
- **1-ELEM**: the controller moves the FCV on level error alone. Simple, but it chases swell and shrink.
- **3-ELEM**: the controller also uses steam flow and feed flow, so it corrects a mismatch before level moves.
  Use it at power.
Feed pumps trip on low suction pressure if condensate cannot keep up, and all trip on [L8](#l8-high-level-trip).
Panel: FEEDWATER panel. [Boiler feedwater (Wikipedia)](https://en.wikipedia.org/wiki/Boiler_feedwater),
[NRC: feedwater control system (PDF)](https://www.nrc.gov/docs/ML0228/ML022830833.pdf),
[NRC glossary: feedwater](https://www.nrc.gov/reading-rm/basic-ref/glossary/feedwater.html).

### Feedwater heaters
Steam extracted from the turbine heats the feedwater before it enters the vessel (215 degC at rated). Hot feedwater
means less subcooling at the core inlet, more voids, and lower reactivity. Taking the heaters OUT drops feed
temperature, collapses voids and raises power, which is a classic BWR transient. Heating only exists once the turbine
is carrying load, so feed temperature is low during startup. [Feedwater heater (Wikipedia)](https://en.wikipedia.org/wiki/Feedwater_heater).

### RWCU
Reactor Water Cleanup takes a bleed from the vessel, filters and demineralises it, and returns it. Its reject valve
instead sends the bleed to the condenser, reducing vessel inventory. In the simulator this is the operator's tool for
holding level down during the startup swell. RWCU reject water is at vessel saturation temperature, so rejecting
also removes some energy. Panel: RWCU REJECT slider and readout.
[NRC GE Systems Technology Manual, chapter 2 (PDF)](https://www.nrc.gov/docs/ML1127/ML11271A021.pdf).

### Isolation condensers (IC)
Three passive heat exchangers sitting in pools of water above the vessel. When a train's valve opens, reactor steam
flows up into the tubes, condenses, and drains back by gravity: no pumps, no power. They start automatically on high
pressure (7.6 MPa), L2 low level, or MSIV closure at power, and the operator can open them by hand. The pools heat
up and boil off over hours (rated 7 days on the real plant). Panel: IC trains on the mimic, pool temperature and
level. [Isolation condenser (Wikipedia)](https://en.wikipedia.org/wiki/Isolation_condenser),
[BWR safety systems (Wikipedia)](https://en.wikipedia.org/wiki/Boiling_water_reactor_safety_systems).

### Annunciator
The grid of alarm tiles. New alarms flash until acknowledged with ACK; severity is colour-coded Info, Warning, Alarm,
Trip. Operator events such as "Rod withdrawal blocked" show as Info tiles for 30 s.
[Annunciator panel (Wikipedia)](https://en.wikipedia.org/wiki/Annunciator_panel).

---

## 14. Events explained

What each transient is, why it happens in a BWR, and what the simulator does about it.

### Criticality
The point at which [k-eff](#k-eff) reaches 1 and the chain reaction sustains itself. Below critical, a neutron source
keeps a small population alive and the SRM count rate equals source divided by (1 - k-eff), so the count rate
climbs faster and faster as k-eff approaches 1 (subcritical multiplication). The simulator sets the initial neutron
population in equilibrium with that source. [Criticality (Wikipedia)](https://en.wikipedia.org/wiki/Criticality_(status)),
[Neutron source (Wikipedia)](https://en.wikipedia.org/wiki/Neutron_source),
[Nuclear chain reaction (Wikipedia)](https://en.wikipedia.org/wiki/Nuclear_chain_reaction).

### Period
After criticality the reactor period tells you how fast power is rising. A 60 s period doubles power every 42 s.
The six delayed neutron groups are what make a period of tens of seconds possible; without them any positive
reactivity would run away in milliseconds ([prompt criticality](https://en.wikipedia.org/wiki/Prompt_criticality)).
The simulator alarms below 20 s. [Reactor period (Wikipedia)](https://en.wikipedia.org/wiki/Reactor_period),
[Delayed neutron (Wikipedia)](https://en.wikipedia.org/wiki/Delayed_neutron).

### Scram
Full rod insertion in 3 s by the hydraulic backup of the FMCRDs. Neutron power collapses within seconds; thermal
power falls to decay heat (7.7 % and falling). Steam production drops, so level shrinks and the pressure regulator
closes the bypass and turbine valves. The level setpoint sets down 100 cm. The scram stays latched until reset.
[NRC glossary: scram](https://www.nrc.gov/reading-rm/basic-ref/glossary/scram.html),
[Scram (Wikipedia)](https://en.wikipedia.org/wiki/Scram).

### Rod withdrawal block
An interlock that stops rods from moving out while a limit is approached, to give the operator a chance before a
scram. Rods already moving stop at the next notch. It clears on its own when the condition clears. See the table in
[section 1](#rod-withdrawal-blocks-rods-stop-no-scram).

### MSIV isolation
Both MSIVs close to protect against a steam line break, loss of steam pressure, or low level. With the steam path
gone, pressure rises quickly; in RUN the closure itself scrams the reactor, and the isolation condensers start if the
plant was at power. Reopening requires the cause to clear. The low steam pressure trip at 5.9 MPa exists to prevent
the pressure regulator from lowering pressure, collapsing voids, and pushing power up uncontrolled; it is only armed
in RUN because startup is done at lower pressure. [NRC GE Systems Technology Manual, chapter 2 (PDF)](https://www.nrc.gov/docs/ML1127/ML11271A021.pdf).

### Swell and shrink
Water level in a BWR moves with the amount of steam in the water, not just mass. A power rise makes more voids and
pushes the surface up (swell); a pressure rise collapses bubbles and drops it (shrink). A scram causes a large
shrink. Single-element level control reacts to the level and can make it worse, which is why three-element control
is used at power. [Boiling water reactor (Wikipedia)](https://en.wikipedia.org/wiki/Boiling_water_reactor).

### L8 high level trip
If level reaches L8 (+100 cm) water could carry over into the steam lines and damage the turbine. The turbine trips
and the feed pumps trip to stop the rise. In the simulator this is the usual end of a careless startup because the
feed pumps must be restarted before level falls to L3. See [Reactor vessel and water level](#reactor-vessel-and-water-level).

### Turbine trip
Turbine stop valves slam shut on a trip signal (overspeed, low vacuum, L8, manual, or generator lockout). The pressure
regulator diverts steam to the bypass; with 100 % bypass the reactor need not scram. Breaker opens on reverse power.
RESET is blocked while L8 or low vacuum persists. [Steam turbine (Wikipedia)](https://en.wikipedia.org/wiki/Steam_turbine).

### Turbine overspeed
Above 1980 rpm (110 %) the turbine trips to protect the blading. It happens if the governor loses control of an
unloaded machine. [Overspeed (Wikipedia)](https://en.wikipedia.org/wiki/Overspeed_(engine)).

### Out-of-phase synchronisation
Closing the breaker with the generator more than 15 degrees out of phase or slipping more than 0.25 Hz would slam
the rotor into step with the grid. The simulator treats it as a generator lockout, which trips the turbine. Reset and
try again. [Synchronization, AC (Wikipedia)](https://en.wikipedia.org/wiki/Synchronization_(alternating_current)).

### Load rejection
The generator breaker opening while loaded. The turbine is suddenly unloaded, speed rises, and the governor closes
the control valves; steam goes to the bypass. Above 40 % power the simulator fires [Select Rod Insert](#select-rod-insert-sri).
[Load rejection (Wikipedia)](https://en.wikipedia.org/wiki/Load_rejection).

### Select Rod Insert (SRI)
On a turbine trip or load rejection at high power, a preselected group of rods is driven in hydraulically. Power
drops to a level the bypass can handle without a full scram. This is a feature of GE's later BWR designs with full
bypass capacity.

### Pressure rise and void collapse
Raising reactor pressure raises the saturation temperature, so some steam in the core condenses, voids shrink, and
the void feedback adds reactivity. Power rises. The simulator's test `raising_pressure_raises_power` checks this.
The reverse happens on a pressure drop. It is the reason the low steam pressure isolation exists and the reason
the procedure says never to raise P SETPT during startup. [Void coefficient (Wikipedia)](https://en.wikipedia.org/wiki/Void_coefficient).

### Loss of feedwater heating
Feedwater colder than normal enters the core with more subcooling, delays boiling, reduces voids and adds
reactivity. Power rises until Doppler and the new void distribution balance it. The simulator warns when feed is
below 150 degC above 30 % power. [Feedwater heater (Wikipedia)](https://en.wikipedia.org/wiki/Feedwater_heater).

### Xenon transient
After a power reduction, xenon-135 keeps being produced from the iodine already in the fuel while its burnout rate
has fallen, so its concentration rises for about 8 to 10 hours before decaying away. During the peak the reactor can
lack enough rod worth to return to power (the "iodine pit"). After a power increase the opposite happens: xenon
burns out faster than iodine supplies it, so reactivity rises for hours. Plan rod moves around it.
[Iodine pit (Wikipedia)](https://en.wikipedia.org/wiki/Iodine_pit), [Xenon-135 (Wikipedia)](https://en.wikipedia.org/wiki/Xenon-135).

### Heatup rate
Thick vessel steel is stressed by temperature gradients, so plants limit heatup and cooldown to about 55 degC/h.
The simulator alarms above that. It only matters when starting cold.
[Reactor pressure vessel (Wikipedia)](https://en.wikipedia.org/wiki/Reactor_pressure_vessel).

### Isolation condenser auto-initiation
Pressure above 7.6 MPa, level below L2, or MSIV closure at power opens all three IC trains. They remove up to about
100 MWt and will cool and depressurise the vessel until closed. Fine as a safety response, a nuisance if you are
trying to hold pressure for a restart. [Isolation condenser (Wikipedia)](https://en.wikipedia.org/wiki/Isolation_condenser).

### Core uncovery and core damage
If level falls below the top of active fuel, cooling degrades, fuel temperature climbs, and above 1200 degC the
simulator declares core damage. Getting there requires losing all feedwater and the isolation condensers for a long
time. [Loss-of-coolant accident (Wikipedia)](https://en.wikipedia.org/wiki/Loss-of-coolant_accident),
[Nuclear meltdown (Wikipedia)](https://en.wikipedia.org/wiki/Nuclear_meltdown).

### Low condenser vacuum
Air in-leakage or loss of circulating water raises condenser pressure. Above 15 kPa the alarm sounds, above 25 kPa
the turbine trips, and above 30 kPa the bypass is blocked, at which point the only steam path is the safety
function of the isolation condensers. [Surface condenser (Wikipedia)](https://en.wikipedia.org/wiki/Surface_condenser).

---

## 15. References

Plant design:
- [GE Hitachi BWRX-300 General Description, 005N9751 (PDF)](https://www.gevernova.com/content/dam/gevernova-nuclear/global/en_us/documents/carbon-free-power/005N9751-BWRX-300-General-Description.pdf)
- [GE Vernova BWRX-300 product page](https://www.gevernova.com/nuclear/carbon-free-power/bwrx-300-small-modular-reactor)
- [ONR BWRX-300 Generic Design Assessment, Step 2 summary report (PDF)](https://www.onr.org.uk/media/taufoxsv/bwrx-300-gda-step-2-summary-report.pdf)
- [BWRX-300 (Wikipedia)](https://en.wikipedia.org/wiki/BWRX-300)

BWR operation and systems (US NRC public training material):
- [NRC: Boiling Water Reactors overview](https://www.nrc.gov/reactors/power/bwrs.html)
- [NRC Reactor Concepts Manual: Boiling Water Reactor Systems (PDF)](https://www.nrc.gov/docs/ML1209/ML120970422.pdf)
- [NRC GE Systems Technology Manual, chapter 2: BWR primary and auxiliary systems (PDF)](https://www.nrc.gov/docs/ML1127/ML11271A021.pdf)
- [NRC GE Systems Technology Manual, chapter 5.2: Intermediate Range Monitor (PDF)](https://www.nrc.gov/docs/ML1125/ML11258A331.pdf)
- [NRC: SRM, IRM and APRM neutron monitoring (PDF)](https://www.nrc.gov/docs/ML1705/ML17056A740.pdf)
- [NRC: Process instrumentation and control, EHC and feedwater control (PDF)](https://www.nrc.gov/docs/ML0228/ML022830833.pdf)
- [NRC glossary](https://www.nrc.gov/reading-rm/basic-ref/glossary.html)

Note: nrc.gov refuses automated downloads, so open those PDFs in a browser.

Simulator sources:
- [README](../README.md), [`params.rs`](../crates/bwr-core/src/params.rs) (every setpoint, marked CONFIRMED or ASSUMED),
  [`sim.rs`](../crates/bwr-core/src/sim.rs) (protection logic), [`examples/startup.rs`](../crates/bwr-core/examples/startup.rs)
  (scripted operator), [`tests/plant.rs`](../crates/bwr-core/tests/plant.rs) (whole-plant behaviour tests).
