# BWRX-300 Simulator: Reactor Startup Procedure

Step-by-step operating procedure for taking the simulated plant from **HOT STANDBY** to the generator on line and
power rising, without tripping any protection. Every limit quoted here is the simulator's own setpoint from
`crates/bwr-core/src/params.rs` and `sim.rs`, so the numbers are what will actually scram you. This is a game
procedure, not a real plant procedure.

Timings come from the scripted operator in `crates/bwr-core/examples/startup.rs`, which pulls rods as fast as the
period allows. A careful human run takes longer. Run at 1x while pulling rods near criticality and 5x to 10x while
waiting on the turbine or xenon.

---

## 1. Limits that will stop you

Know these before touching anything. The first column is the trigger, the second is what the simulator does.

### Scram (reactor trip)

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

| Trigger | Setpoint | Applies in |
|---|---|---|
| Scram not reset | | always |
| Mode switch in SHUTDOWN | all withdrawal blocked | SHUTDOWN |
| IRM upscale | > 108 % of range | STARTUP |
| APRM setdown upscale | > 12 % power | STARTUP |
| APRM upscale | > 108 % power | RUN |

### MSIV isolation (steam lines close, then a scram follows in RUN)

| Trigger | Setpoint |
|---|---|
| Low-low water level L2 | < -400 cm |
| Low steam line pressure | < 5.9 MPa at the turbine inlet, **only in RUN** |
| High steam flow | > 140 % rated |

### Turbine and feedwater trips

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
| SHORT PERIOD | period < 20 s (panel turns amber below 60 s, red below 30 s) |
| REACTOR WATER LEVEL HIGH L7 | > +50 cm |
| REACTOR WATER LEVEL LOW L4 | < -50 cm |
| REACTOR PRESSURE HIGH | > 7.35 MPa |
| HEATUP / COOLDOWN RATE | > 55 degC/h |
| FEEDWATER TEMPERATURE LOW | feed < 150 degC while above 30 % power |

---

## 2. Initial conditions: HOT STANDBY

Load the scenario from the top bar: select **HOT STANDBY**, press **LOAD**, then **CONFIRM**. Verify the plant looks
like this before starting. If anything differs, fix it first.

| Item | Expected |
|---|---|
| Mode switch | STARTUP |
| RPS | RPS NORMAL lit, SCRAM dark, ALL RODS IN lit |
| All 57 rods | position 00 (full in) |
| RX PRESS | about 6.90 MPa, coolant about 285 degC |
| P SETPT (EHC) | 6.95 MPa, bypass in AUTO |
| IRM | range 1, reading near zero; SRM a few cps |
| Period | infinite or very long |
| Xenon | about 27 % of equilibrium (two days after shutdown) |
| Turbine | TRIPPED lit, speed 0 rpm, breaker OPEN |
| Condenser | vacuum available, pressure well under 15 kPa, CW pumps and vacuum pump running |
| Feedwater | COND P A and FWP A running, level control 1-ELEM, setpoint 0 cm, FW HEATERS IN, feed temp 150 degC |
| LEVEL NR | near 0 cm, drifting slightly |
| MSIVs | both open |
| Isolation condensers | all three valves closed |

Set the sim speed to **1x** for the rod work that follows.

---

## 3. Phase A: prepare for swell

The core will swell the water level upward as it heats and voids form. Make room for that now, before it happens.

1. In the FEEDWATER panel, press **SETPT -5** repeatedly until the level setpoint reads **-40 cm**.
   The level controller will slowly lower level toward that value.
2. Leave **RWCU REJECT** at 0 for now. You will use it in Phase C.
3. Press **ACK** on the annunciator to clear any standing alarms so new ones stand out.

Why: L7 alarms at +50 cm and L8 at +100 cm trips the turbine and every feed pump. Starting from -40 cm gives you
140 cm of margin for swell.

---

## 4. Phase B: approach to criticality

Rods are bottom-entry and read 00 (full in) to 48 (full out) in notches of 2. **INSERT** and **WITHDRAW** move the
selected rod or group one notch. The simulator pulls rods in banks: every rod to 08, then every rod to 16, and so on.
That keeps the power shape flat and the worth per notch predictable.

1. Click a rod on the core map to select it, or use **GROUP SELECT ALL** to select all 57.
2. Press **WITHDRAW** once. Wait for the rods to stop moving (POS matches TGT). With all 57 selected this is one
   notch on every rod.
3. Watch three things after every notch, in this order:
   - **PERIOD**. Positive and shortening means you are adding reactivity. Do not pull again while the period is
     shorter than about **100 s**. If it drops under **60 s**, stop and wait. If it drops under **30 s**, press
     **INSERT** one notch.
   - **SRM** count rate. It climbs slowly at first, then faster as you approach critical. That acceleration is the
     signal that criticality is close.
   - **ROD WITHDRAWAL BLOCK** in the annunciator. If it appears, read the reason. Nothing moves until it clears.
4. Continue one notch at a time. Expect criticality (k-eff reaching 1.00000 and a steady positive period) with the
   rods averaging about **24 of 48** withdrawn. The scripted operator reaches it in roughly **55 minutes** of sim time.
5. Once the period is steady and positive, stop pulling. Power will rise on its own. Let it coast up.

**IRM ranging.** Each IRM range covers a factor of about 3.2 in power. Range 10 is 40 % full scale. When the IRM
reads above **80 %**, click the next range up. Above 108 % you lose rod withdrawal; above 120 % you scram. Ranging
is the single most common way to lose a startup. Do not let the IRM pass 100 %.

| IRM range | Full scale (% power) | Reads 80 % at |
|---|---|---|
| 5 | 0.13 | 0.1 % |
| 6 | 0.4 | 0.32 % |
| 7 | 1.3 | 1.0 % |
| 8 | 4.0 | 3.2 % |
| 9 | 12.6 | 10 % |
| 10 | 40 | 32 % |

Range 10 is the top. At 6 % power on range 9 the IRM reads about 50 %, which is where the scripted operator rolls
the turbine. In STARTUP the APRM setdown limits (12 % block, 15 % scram) bite before the IRM does on range 10, so
the mode switch to RUN in Phase E is what actually lets you climb past 12 %.

---

## 5. Phase C: heat up and hold pressure

Power is now a few tenths of a percent and rising. Reactivity feedback changes character here: Doppler and voids
turn negative as power climbs, so the period will lengthen by itself and you will need to pull more rods to keep
climbing. Hold power at about **6 %** for the turbine roll.

1. Keep pulling single notches when the period lengthens beyond about 150 s and power is below 6 %. Stop when
   power reaches 6 %.
2. Watch **RX PRESS**. The EHC pressure regulator holds 6.95 MPa by opening the **BYPASS** valves to the condenser as
   steam production grows. Bypass position creeping up from 0 is normal and means the regulator is working.
3. Watch **LEVEL NR**. As voids form the level **swells** upward. Hold it below **+20 cm**:
   - Move the **RWCU REJECT** slider up (the panel slider goes to 10 kg/s) when level climbs past +20 cm.
   - Bring it back to 0 when level is below +20 cm. The scripted operator toggles between full reject and 0 using
     that threshold.
   - If level reaches **+50 cm** you will get the L7 alarm. Act on it. At **+100 cm** you lose the turbine and all
     feed pumps and the startup is effectively over.
4. Do not raise the pressure setpoint. Raising pressure collapses voids, which adds reactivity and raises power.
5. Keep **FW HEATERS IN**. Cold feedwater adds reactivity and is a source of power swings.

---

## 6. Phase D: roll the turbine and synchronise

Conditions to start: power above about **3.5 %** and reactor pressure above **6.9 MPa**. The turbine can only take
steam the regulator is already passing through the bypass, so rolling it will not drag pressure down.

1. Confirm **CONDENSER** pressure is below **15 kPa**. RESET is blocked above 25 kPa and bypass is blocked above
   30 kPa.
2. Confirm LEVEL NR is below +100 cm. RESET is blocked above L8.
3. Press **RESET** in the TURBINE group. TRIPPED goes dark and LATCHED lights.
4. Press **SPEED SP 1800**. The governor ramps the reference at 5 rpm/s, so the shaft takes about **6 minutes** to
   reach 1800 rpm. Steam diverts from the bypass to the turbine as it accelerates. Sim speed 5x is fine here.
5. Watch level during the roll. The scripted run saw a swell to **+55 cm** at this point. Use RWCU reject early.
6. The synchroscope wakes up above 1500 rpm, but the breaker will only close within 20 rpm of 1800. Watch the
   needle:
   - It must be turning **slowly** (slip under 0.25 Hz) and pointing **near 12 o'clock** (within 15 degrees).
   - Aim for within 5 degrees and almost stationary. Press **CLOSE** on the generator breaker as the needle
     passes 12 o'clock.
7. **BKR CLOSED** lights and NET MWe goes positive. If you get an out-of-phase message, the turbine has tripped
   on generator lockout. Go back to step 3 and wait for a better moment next time.

After the breaker closes the plant is in "reactor leads" mode. The pressure regulator positions the turbine
control valves, and electrical load follows reactor power. You do not set load directly. LOAD LIM at 105 % is the
ceiling and needs no change.

---

## 7. Phase E: mode switch to RUN (time critical)

With the generator on line, move the mode switch **immediately**, before power passes 12 %.

1. Click **RUN** on the mode switch.
2. Confirm the IRM bar now reads **(BYPASSED IN RUN)**.

Why the hurry: in STARTUP the APRM setdown blocks rods at 12 % and scrams at 15 %, and the IRM is pinned above
100 % on range 10. In RUN those go away and the APRM limits become 108 % block and 118 % scram.

What RUN arms against you: the **low steam line pressure MSIV isolation at 5.9 MPa**. If pressure sags below that,
both MSIVs close, which scrams the reactor and auto-starts the isolation condensers. Keep pressure up. Never lower
the pressure setpoint below about 6.2 MPa in RUN.

---

## 8. Phase F: power ascent

1. Resume pulling rods a notch at a time with the same period discipline. Reactor power drives turbine load.
2. At about **20 % power** (the scripted operator's point; one feed pump runs out at 400 kg/s, which is roughly
   85 % power, so there is no rush beyond having margin):
   - Press **COND P B RUN**.
   - Press **FWP B RUN**. A feed pump will not start without a condensate pump running.
   - Switch the level controller to **3-ELEM**. Three-element control uses steam flow and feed flow and tracks
     level far better through transients.
3. Raise the level setpoint back toward **0 cm** in +5 steps once level is stable and well under L7.
4. Keep an eye on **FEED TEMP**. Feedwater heating comes from turbine extraction and rises with load. If it reads
   under 150 degC above 30 % power you get a low temperature warning, and the cold feed is adding reactivity.
5. Bring **RWCU REJECT** back to 0 once level control is steady in 3-element.
6. Continue to your target power. Full power is 870 MWt, about 300 MWe net, 7.2 MPa dome pressure.

The scripted operator reaches 40 % power and 82 MWe net about **75 minutes** after starting from hot standby.

---

## 9. Phase G: xenon

Hot standby starts with about 27 % of equilibrium xenon. As the core runs at power, xenon builds toward equilibrium
over roughly the next day of sim time and eats reactivity (equilibrium worth is 2.8 % dk/k, the largest single
poison in the model). Power will sag unless you keep withdrawing rods. Check every 30 to 60 minutes of sim time at
10x or 30x and pull a notch when power drifts down. Watch the XENON readout climb toward 100 %.

If you reduce power, xenon first **rises** (iodine decays into it faster than it burns out) before it falls. That
is the xenon peak. Expect to need extra rod withdrawal for several hours after any power reduction, and extra
insertion later when it burns off.

---

## 10. If you scram

1. Rods insert hydraulically in 3 s. The level setpoint sets down by 100 cm automatically.
2. Find the cause in the RPS readout under the SCRAM lamp and in the annunciator. Fix it (re-range the IRM, lower
   pressure, restore level).
3. Wait at least **10 s**, then press **SCRAM RESET**. Reset is blocked while any scram signal is still active.
4. If the MSIVs closed, clear the isolation cause and reopen them. If the isolation condensers started, they will
   keep cooling the vessel until you close their valves.
5. If the turbine tripped, you must RESET and re-synchronise. If it tripped on L8, all feed pumps also tripped:
   restart COND P A, then FWP A, before level falls to L3.
6. Return to Phase B. Xenon will be higher than it was, so expect to need more rods out.

---

## 11. Starting from COLD SHUTDOWN (not exercised by the tests)

The cold shutdown scenario is 30 days after shutdown, 40 degC, vessel vented to atmosphere with the RPV head vent
open, mode switch in SHUTDOWN, no condenser vacuum, no pumps running. The integration tests only cover the hot
standby startup, so treat this as a sketch rather than a validated path.

1. Start **CW PUMP A** and **CW PUMP B**, then the **VACUUM PUMP**. Vacuum pulls in with a time constant of about
   10 minutes. Wait for condenser pressure under 15 kPa.
2. Start **COND P A**, then **FWP A**. Put the level controller in **1-ELEM** with setpoint 0 cm.
3. Turn the mode switch to **STARTUP**. Withdrawal is blocked in SHUTDOWN. Do not turn it to SHUTDOWN again
   later without meaning to: that is itself a scram signal.
4. Pull rods per Phase B to reach criticality and a low power level. Nuclear heat does the heating; there is no
   external heating source in the model.
5. Hold the heatup rate under **55 degC/h** by limiting power. Watch the HEATUP RATE readout.
6. **Close the RPV HEAD VENT** before pressure rises above 0.2 MPa, or you get the vent-open warning and keep
   venting inventory to the condenser.
7. Once pressure reaches the 6.95 MPa setpoint the bypass valves take over. Continue from Phase C.
