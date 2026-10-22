//! Plant design parameters.
//!
//! Values marked CONFIRMED come from public GE Hitachi / NRC documents
//! (BWRX-300 General Description 005N9751, NRC safety evaluations).
//! Values marked ASSUMED are engineering estimates chosen to be plausible for
//! a 870 MWt natural-circulation BWR and should be revisited as better data
//! is found.

use std::f64::consts::PI;

pub const G: f64 = 9.81;

// ---------------------------------------------------------------- core
/// Rated thermal power, W. CONFIRMED (870 MWt).
pub const RATED_POWER_W: f64 = 870.0e6;
/// Number of control rods. CONFIRMED (57, bottom entry, fine-motion drives).
pub const N_RODS: usize = 57;
/// Number of fuel bundles. CONFIRMED (240 GNF2).
pub const N_BUNDLES: usize = 240;
/// Rod positions run 0 (full in) .. 48 (full out), notches are 2 positions.
pub const ROD_FULL_OUT: f64 = 48.0;
/// Active fuel length, m. ASSUMED (GNF2 ~3.81 m).
pub const CORE_HEIGHT: f64 = 3.81;
/// FMCRD normal drive speed, rod positions per second (~30 mm/s). ASSUMED.
pub const ROD_SPEED: f64 = 0.4;
/// Time for a hydraulic scram to fully insert rods, s. ASSUMED.
pub const SCRAM_TIME: f64 = 3.0;

// Neutronics. ASSUMED (typical BWR, U-235 delayed neutron data from Keepin).
pub const BETA: f64 = 0.0060;
pub const BETA_FRAC: [f64; 6] = [0.033, 0.219, 0.196, 0.395, 0.115, 0.042];
pub const LAMBDA_DN: [f64; 6] = [0.0124, 0.0305, 0.111, 0.301, 1.14, 3.01];
/// Prompt neutron generation time, s.
pub const GEN_TIME: f64 = 4.0e-5;
/// Neutron source strength (relative power units per second).
pub const SOURCE: f64 = 8.0e-6;

// Reactivity coefficients. ASSUMED, tuned so that the core is critical with
// ~12 % of rod worth inserted at hot full power with equilibrium xenon.
/// Excess reactivity of the cold, clean, all-rods-out core (dk/k).
pub const RHO_EXCESS_COLD: f64 = 0.130;
/// Total control rod worth (dk/k).
pub const ROD_WORTH_TOTAL: f64 = 0.200;
/// Doppler: rho = -A (sqrt(Tf[K]) - sqrt(293.15)).
pub const DOPPLER_A: f64 = 0.0024;
/// Void coefficient per unit core-average void fraction (dk/k).
pub const VOID_COEF: f64 = -0.080;
/// Moderator temperature coefficient (dk/k per K, relative to 20 degC).
pub const MOD_TEMP_COEF: f64 = -6.0e-5;
/// Equilibrium xenon worth at 100 % power (dk/k).
pub const XENON_WORTH_EQ: f64 = 0.028;

// Xenon / iodine.
pub const LAMBDA_I: f64 = 2.87e-5;
pub const LAMBDA_XE: f64 = 2.09e-5;
pub const GAMMA_I: f64 = 0.0639;
pub const GAMMA_XE: f64 = 0.00237;
/// sigma_a(Xe) * thermal flux at rated power, 1/s.
pub const SIGMA_PHI_XE: f64 = 2.65e-18 * 3.0e13;

/// Decay heat groups (fraction of rated power at equilibrium, decay constant 1/s).
/// Fitted to 0.066 t^-0.2 (infinite operation) by tools/fit_decay_heat.py.
pub const DECAY_GROUPS: [(f64, f64); 9] = [
    (2.604750e-02, 5.011872e-01),
    (1.712415e-02, 6.309573e-02),
    (1.135295e-02, 7.943282e-03),
    (7.440500e-03, 1.000000e-03),
    (5.015556e-03, 1.258925e-04),
    (3.145072e-03, 1.584893e-05),
    (2.396613e-03, 1.995262e-06),
    (6.616170e-04, 2.511886e-07),
    (3.379486e-03, 3.162278e-08),
];

/// Fraction of fission power deposited directly in the coolant.
pub const DIRECT_COOLANT_HEAT: f64 = 0.025;
/// Fuel heat capacity (UO2 ~49 t x 300 J/kgK), J/K. ASSUMED.
pub const FUEL_HEAT_CAP: f64 = 1.47e7;
/// Fuel-to-coolant thermal resistance, K/W (avg fuel ~550 degC at rated). ASSUMED.
pub const FUEL_RESISTANCE: f64 = 264.0 / (RATED_POWER_W * (1.0 - DIRECT_COOLANT_HEAT));

// ---------------------------------------------------------------- vessel
/// Nominal reactor pressure at rated power, Pa. CONFIRMED (7.2 MPa).
pub const P_RATED: f64 = 7.2e6;
// Vessel geometry. CONFIRMED for design revision GE Hitachi 005N9751 Rev. J
// (BWRX-300 General Description), section 3.1.1, printed p. 31:
// inner diameter ~4.2 m, inner height ~27.36 m, bottom of active fuel ~5.12 m
// above the vessel bottom. vessel.rs derives the free-volume/elevation model
// (liquid_volume_below, level_from_volume, driving head) from these constants;
// the internal layout below is ASSUMED and kept relative to Z_CORE_BOTTOM so
// it moves with the fuel elevation.
/// Vessel inner diameter, m. CONFIRMED (Rev. J).
pub const VESSEL_ID: f64 = 4.2;
/// Vessel inner height, m. CONFIRMED (Rev. J).
pub const VESSEL_HEIGHT: f64 = 27.36;
/// Bottom of active fuel above vessel bottom, m. CONFIRMED (Rev. J).
pub const Z_CORE_BOTTOM: f64 = 5.12;
/// Elevations above vessel bottom, m. ASSUMED (relative to the fuel).
pub const Z_TAF: f64 = Z_CORE_BOTTOM + CORE_HEIGHT;
pub const CHIMNEY_HEIGHT: f64 = 9.0;
pub const Z_CHIMNEY_TOP: f64 = Z_TAF + CHIMNEY_HEIGHT;
/// Dryer skirt elevation, m. ASSUMED (separator region 5.7 m above the chimney).
pub const Z_DRYER: f64 = Z_CHIMNEY_TOP + 5.7;
/// Normal water level elevation, m. ASSUMED.
pub const Z_NWL: f64 = Z_CHIMNEY_TOP + 3.0;

pub const A_VESSEL: f64 = PI * VESSEL_ID * VESSEL_ID / 4.0;
/// Core flow area (240 bundles + bypass), m2. ASSUMED.
pub const A_CORE: f64 = 2.6;
/// Chimney inner area, m2. ASSUMED.
pub const A_CHIMNEY: f64 = 8.55;
/// Shroud / chimney outer diameter, m. ASSUMED.
pub const SHROUD_OD: f64 = 3.5;
/// Downcomer annulus area, m2. Derived from the vessel ID and shroud OD.
pub const A_DOWNCOMER: f64 = A_VESSEL - PI * SHROUD_OD * SHROUD_OD / 4.0;
/// Free area for water level between chimney top and dryer skirt, m2. ASSUMED.
pub const A_SEPARATOR_REGION: f64 = 9.0;
/// Free area above the dryer skirt, m2. ASSUMED.
pub const A_DOME: f64 = 10.0;
/// Lower plenum fluid volume, m3. ASSUMED.
pub const V_LOWER_PLENUM: f64 = 22.0;
pub const V_CORE: f64 = A_CORE * CORE_HEIGHT;
pub const V_CHIMNEY: f64 = A_CHIMNEY * CHIMNEY_HEIGHT;
pub const V_DOWNCOMER: f64 = A_DOWNCOMER * (Z_CHIMNEY_TOP - Z_CORE_BOTTOM);
pub const V_SEP_REGION: f64 = A_SEPARATOR_REGION * (Z_DRYER - Z_CHIMNEY_TOP);
pub const V_DOME: f64 = A_DOME * (VESSEL_HEIGHT - Z_DRYER);
pub const V_VESSEL_FLUID: f64 =
    V_LOWER_PLENUM + V_CORE + V_CHIMNEY + V_DOWNCOMER + V_SEP_REGION + V_DOME;

/// Vessel + internals heat capacity, J/K (~550 t steel). ASSUMED.
pub const METAL_HEAT_CAP: f64 = 550.0e3 * 500.0;
/// Insulation heat loss at hot conditions, W. ASSUMED.
pub const HEAT_LOSS_HOT: f64 = 1.0e6;

/// Natural circulation core flow at rated conditions, kg/s. ASSUMED
/// (scaled from ESBWR ~2.2 kg/s per MWt).
pub const CORE_FLOW_RATED: f64 = 2000.0;
/// Loop inertia sum(L/A), 1/m. ASSUMED.
pub const LOOP_INERTIA: f64 = 7.3;
/// Drift flux parameters.
pub const C0: f64 = 1.13;
pub const N_AXIAL: usize = 12;

// ---------------------------------------------------------------- steam & turbine
/// Rated feedwater temperature, degC. ASSUMED (typical BWR ~215 degC).
pub const T_FW_RATED: f64 = 215.0;
/// Rated steam flow, kg/s (870 MW / (h_g(7.2 MPa) - h_f(215 degC))).
pub const STEAM_FLOW_RATED: f64 = 470.0;
/// Main steam line pressure drop at rated flow, Pa. ASSUMED.
pub const STEAM_LINE_DP: f64 = 0.2e6;
pub const N_STEAM_LINES: usize = 2;
/// MSIV stroke time, s. ASSUMED.
pub const MSIV_STROKE: f64 = 4.0;
/// Turbine control valve flow coefficient (kg/s per Pa at full open). ASSUMED.
pub const C_TCV: f64 = 1.10 * STEAM_FLOW_RATED / 7.0e6;
/// Turbine bypass capacity as fraction of rated steam flow. ASSUMED (to verify).
pub const BYPASS_CAPACITY: f64 = 1.0;
pub const C_BYPASS: f64 = BYPASS_CAPACITY * STEAM_FLOW_RATED / 7.0e6;
/// Bypass fast-opening time on a turbine trip / load rejection signal, s.
/// CONFIRMED: the bypass valves complete fast opening within 300 ms of the
/// valid trip or load-rejection signal (GE Hitachi 005N9751 Rev. J, section
/// 5.1.4, printed p. 56). Modelled as a linear full stroke, so the fast-open
/// stroke rate is 1 / BYPASS_FAST_OPEN_TIME full strokes per second.
pub const BYPASS_FAST_OPEN_TIME: f64 = 0.3;
pub const BYPASS_FAST_OPEN_RATE: f64 = 1.0 / BYPASS_FAST_OPEN_TIME;
/// Bypass stroke rate under ordinary pressure-regulating motion, strokes/s. ASSUMED.
pub const BYPASS_REG_RATE: f64 = 2.0;
/// Bypass stroke rate under manual control, strokes/s. ASSUMED.
pub const BYPASS_MANUAL_RATE: f64 = 0.2;
/// Pressure regulator: steam demand rises 100 % per this many Pa above setpoint.
pub const PRESSURE_REG_SPAN: f64 = 0.25e6;
/// Default pressure setpoint, Pa (gives 7.2 MPa dome pressure at rated steam flow).
pub const PRESSURE_SETPOINT_DEFAULT: f64 = 6.95e6;
/// Effective turbine enthalpy drop to shaft at rated conditions, J/kg. ASSUMED
/// (gives ~315 MWe gross / ~300 MWe net).
pub const TURBINE_DH_RATED: f64 = 0.703e6;
pub const GENERATOR_EFF: f64 = 0.985;
// Turbine-generator: 3,000/3,600 rpm turbine with a direct-driven 50/60 Hz
// generator. CONFIRMED (GE Hitachi 005N9751 Rev. J, section 5.1.2, printed
// p. 55). The 60 Hz North American configuration is modelled: 3,600 rpm,
// two-pole generator, electrical frequency f = rpm * poles / 120 = rpm / 60.
pub const GRID_HZ: f64 = 60.0;
pub const GENERATOR_POLES: f64 = 2.0;
pub const RATED_RPM: f64 = 120.0 * GRID_HZ / GENERATOR_POLES;
/// Rated angular speed, rad/s.
pub const OMEGA_RATED: f64 = RATED_RPM * 2.0 * PI / 60.0;
/// Generator apparent power rating, VA. ASSUMED.
pub const TG_MVA: f64 = 330.0e6;
/// Turbine-generator inertia constant H, s. ASSUMED.
pub const TG_H_S: f64 = 4.0;
/// Turbine-generator inertia, kg m2, derived as J = 2 H S / omega_rated^2
/// (~18,576 kg m2 for H = 4 s on 330 MVA at 3,600 rpm).
pub const TG_INERTIA: f64 = 2.0 * TG_H_S * TG_MVA / (OMEGA_RATED * OMEGA_RATED);
/// No-load windage and friction losses at rated speed, W. ASSUMED.
pub const TG_LOSSES: f64 = 8.0e6;
/// Mechanical overspeed trip ratio. ASSUMED (110 % of rated, 3,960 rpm).
pub const OVERSPEED_TRIP_RATIO: f64 = 1.10;
pub const OVERSPEED_TRIP_RPM: f64 = OVERSPEED_TRIP_RATIO * RATED_RPM;
/// Speed reference ramp, rpm per second.
pub const SPEED_RAMP: f64 = 10.0;

// ---------------------------------------------------------------- condenser & feedwater
pub const CW_INLET_C: f64 = 20.0;
/// Circulating water flow per pump, kg/s. ASSUMED.
pub const CW_FLOW_PER_PUMP: f64 = 7_500.0;
pub const N_CW_PUMPS: usize = 2;
/// Condenser UA, W/K. ASSUMED (gives ~5.6 kPa at rated load).
pub const CONDENSER_UA: f64 = 5.6e7;
pub const CONDENSER_VOLUME: f64 = 3000.0;
pub const AIR_INLEAKAGE: f64 = 0.004;
pub const VACUUM_PUMP_TAU: f64 = 600.0;
/// Turbine trips above this condenser pressure, Pa.
pub const CONDENSER_TRIP_P: f64 = 25.0e3;
/// Bypass valves are blocked above this condenser pressure, Pa.
pub const CONDENSER_BYPASS_BLOCK_P: f64 = 30.0e3;
pub const HOTWELL_CAPACITY: f64 = 120_000.0;
/// Condenser shell, tube and structure heat capacity lumped with the hotwell
/// water, J/K. ASSUMED (~400 t of steel/copper alloy).
pub const CONDENSER_METAL_HEAT_CAP: f64 = 400.0e3 * 450.0;
/// Specific heat of cooling water / condensate, J/(kg K).
pub const CP_WATER: f64 = 4186.0;
/// Condensate storage tank temperature (makeup water), degC. ASSUMED.
pub const CST_TEMP_C: f64 = 30.0;
/// Hotwell makeup / reject transfer rate to the condensate storage tank, kg/s. ASSUMED.
pub const HOTWELL_TRANSFER_RATE: f64 = 30.0;
/// Hotwell level band (fraction of capacity) outside which makeup / reject runs.
pub const HOTWELL_MAKEUP_FRAC: f64 = 0.40;
pub const HOTWELL_REJECT_FRAC: f64 = 0.85;
/// Condenser shell relief pressure, Pa (rupture discs). ASSUMED.
pub const CONDENSER_RELIEF_P: f64 = 1.2 * 101_325.0;

// Reactor feed pumps: two 100 % capacity pumps with adjustable-speed drives.
// CONFIRMED (GE Hitachi 005N9751 Rev. J, section 5.1.6, printed p. 57; also
// NEDO-34173 Rev. B, section 10.2.2). Pump curve coefficients are ASSUMED.
pub const N_FEED_PUMPS: usize = 2;
pub const N_COND_PUMPS: usize = 2;
/// Feed pump shutoff differential pressure at full speed, Pa. ASSUMED.
pub const FW_PUMP_SHUTOFF_DP: f64 = 9.5e6;
/// Feed pump runout flow at full speed, kg/s. ASSUMED: one pump delivers
/// rated feed flow against rated vessel pressure at ~96 % speed.
pub const FW_PUMP_RUNOUT: f64 = 800.0;
/// Adjustable-speed drive minimum speed, per unit of rated. ASSUMED.
pub const FW_PUMP_MIN_SPEED: f64 = 0.80;
/// Drive speed ramp rate, per unit per second. ASSUMED.
pub const FW_PUMP_SPEED_RATE: f64 = 0.05;
/// Coast-down rate of a stopped pump, per unit per second. ASSUMED.
pub const FW_PUMP_COASTDOWN_RATE: f64 = 0.20;
/// Condensate (booster) pressure at feed pump suction, Pa.
pub const CONDENSATE_PRESSURE: f64 = 1.5e6;
/// Feedwater heater train (shells, tubes, water) lumped heat capacity, J/K.
/// ASSUMED: gives a ~180 s outlet-temperature time constant at rated flow.
pub const FW_HEATER_HEAT_CAP: f64 = 3.5e8;
/// Extraction heat available per kg of turbine steam, J/kg. ASSUMED: ~10 %
/// above the rated feedwater heating demand (h_f(215 degC) - h_f(35 degC)).
pub const FW_EXTRACTION_HEAT_PER_KG: f64 = 0.85e6;
/// Feed pump temperature rise from pumping work, degC. ASSUMED.
pub const FW_PUMP_TEMP_RISE: f64 = 2.0;
/// Heater train ambient heat loss conductance, W/K. ASSUMED (~1 MW at rated).
pub const FW_HEATER_LOSS_UA: f64 = 5.0e3;
/// Feedwater control valve coefficient at full open, kg/s per sqrt(Pa) (equal-percentage trim).
pub const C_FCV: f64 = 1.75;

// ---------------------------------------------------------------- isolation condensers
/// Number of isolation condenser trains. ASSUMED (to verify; ESBWR-derived).
pub const N_IC_TRAINS: usize = 3;
/// Heat removal per train at rated pressure with a 100 degC pool, W. ASSUMED
/// (ESBWR-size unit, 33.75 MWt).
pub const IC_TRAIN_RATED_W: f64 = 33.75e6;
pub const IC_VALVE_STROKE: f64 = 15.0;
pub const IC_POOL_MASS: f64 = 1.5e6;

// ---------------------------------------------------------------- protection setpoints (ASSUMED)
pub const SCRAM_HIGH_PRESSURE: f64 = 7.50e6;
pub const IC_HIGH_PRESSURE: f64 = 7.60e6;
pub const VESSEL_DESIGN_PRESSURE: f64 = 8.62e6;
pub const APRM_HIGH_RUN: f64 = 1.18;
pub const APRM_HIGH_SETDOWN: f64 = 0.15;
pub const IRM_HIGH_HIGH: f64 = 120.0;
pub const MSIV_LOW_PRESSURE_RUN: f64 = 5.9e6;
/// Water level setpoints, cm relative to NWL.
pub const L8_CM: f64 = 100.0;
pub const L7_CM: f64 = 50.0;
pub const L4_CM: f64 = -50.0;
pub const L3_CM: f64 = -100.0;
pub const L2_CM: f64 = -400.0;
pub const L1_CM: f64 = -600.0;

// ---------------------------------------------------------------- house loads, W (ASSUMED)
pub const HOUSE_BASE: f64 = 4.0e6;
pub const FW_PUMP_POWER: f64 = 3.5e6;
pub const COND_PUMP_POWER: f64 = 1.0e6;
pub const CW_PUMP_POWER: f64 = 2.0e6;
