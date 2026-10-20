# Package 1: Reference Design and Evidence Register — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Master contract: `docs/superpowers/plans/2026-10-20-refinement-master.md`. Spec: `docs/superpowers/plans/2026-10-20-existing-systems-refinement.md`, section "Package 1".

**Goal:** Produce a traceable evidence register so that every consequential constant, interlock, and modelled arrangement in the engine and UI points at a public source or a numbered assumption, enforced by a test.

**Architecture:** Five documents and three CSV registers under `docs/model/` become the single source of truth for provenance. `parameters.csv` is machine-checked by a new integration test (`crates/bwr-core/tests/register.rs`) that scans `params.rs` for `pub const` names and verifies each has a row with an allowed status. Later packages add rows as they add or change constants; the test fails if they forget.

**Tech Stack:** Markdown, CSV (comma-separated, no commas inside fields; use semicolons for lists), Rust integration test using only `std` (no new dependencies), `shasum -a 256` for checksums, `curl -L -o` for retrieval.

## Global Constraints

See master plan Global Constraints; in addition:

- Apply evidence in the spec's order: (1) BWRX design descriptions and assessments; (2) ESBWR material for shared concepts; (3) established thermodynamics and component physics with BWRX boundary conditions; (4) documented engineering estimates with sensitivity ranges.
- "Start with a coherent North American 60 Hz reference configuration, matching the existing application. Identify differences before using UK or Canadian details."
- "Resolve conflicts by applicable design revision and configuration. Do not silently combine incompatible sources or infer hidden values from redactions."
- Every `parameters.csv` row has a `status` of exactly one of: `confirmed`, `derived`, `calibrated`, `esbwr-estimate`, `assumed`, `unavailable`.
- Do not promote any value to `confirmed` in this package beyond what `README.md` and `params.rs` already mark CONFIRMED. Promotion happens only when a `sources.csv` row with a real checksum and page/section exists.
- CSV fields never contain commas. Lists use `;`. Formulas use `^` and `*` with no commas.

---

## File structure

| Path | Responsibility |
|---|---|
| `docs/model/design-basis.md` | Reference configuration, jurisdiction policy, conflict-resolution rule, confirmed/unresolved list |
| `docs/model/sources.csv` | One row per public document with ID, revision, URL, section, retrieval date, SHA-256, kind |
| `docs/model/parameters.csv` | One row per constant (params.rs, hardcoded engine values, UI constants) |
| `docs/model/assumptions.md` | Numbered assumptions `A-nnn` with rationale and sensitivity |
| `docs/model/coverage.md` | Subsystem → sources, parameters, assumptions, tests |
| `docs/model/esbwr-applicability.csv` | One row per borrowed ESBWR concept with classification |
| `crates/bwr-core/tests/register.rs` | Enforces that `parameters.csv` covers every `pub const` in `params.rs` with a valid status |
| `README.md` | New "Model evidence" section linking the register |

### CSV schemas (locked)

`sources.csv` (12 columns):

```
source_id,document_id,title,revision,date,jurisdiction,url,section,retrieval_date,sha256,kind,notes
```

`kind` is one of `vendor-proposal`, `regulator-assessment`, `accepted-methodology`, `standard`, `textbook`, `index-page`.

`parameters.csv` (13 columns):

```
id,value,unit,definition,reference_state,source_design,status,source_ids,interval_lo,interval_hi,assumption_id,location,notes
```

Column meanings: `id` is the `pub const` name for `params.rs` constants, or `<file>.<name>` for hardcoded values elsewhere; `reference_state` is the plant state the value refers to (`rated`, `cold`, `HFP`, `-`); `source_design` is `BWRX-300`, `ESBWR`, `BWR-generic`, `physics`, `tuning`, `numerical`, or `ui-demo`; `interval_lo`/`interval_hi` give the sensitivity interval or `none` when the spec's "no defensible interval" case applies; `assumption_id` is required for `assumed` and `esbwr-estimate` rows.

`esbwr-applicability.csv` (10 columns):

```
id,esbwr_source,esbwr_section,target_subsystem,bwrx_evidence,differences,adaptation,applicability_limits,test,classification
```

`classification` is one of `shared-concept`, `adapted-model`, `provisional-estimate`, `inapplicable`, `pending`.

---

### Task 1: Design basis document

**Files:**
- Create: `docs/model/design-basis.md`

**Interfaces:**
- Produces: assumption IDs referenced by `assumptions.md` (Task 7) and the conflict rule referenced by every later package.

- [ ] **Step 1: Create the directory and write the document**

```bash
mkdir -p docs/model docs/validation
```

Write `docs/model/design-basis.md` with exactly this content:

```markdown
# BWRX-300 Simulator Design Basis

Planning date: 2026-10-20. Reviewed engine baseline: commit 9cac46ac0e5b255f88c3d5a4deb064edac30a52c.

## 1. Reference configuration

- Plant: GE Hitachi BWRX-300, single unit, North American 60 Hz grid.
- Reference jurisdiction: United States (NRC pre-application topical reports, Clinch River CPA/PSAR). Canadian (CNSC/OPG Darlington) and UK (GDA) documents are supplementary. Where a UK or Canadian document gives a value not available in a US document, the row in `parameters.csv` records `jurisdiction` via its `source_ids` and `notes`, and the value is flagged for configuration difference review before use.
- Turbine-generator: 1800 rpm synchronous speed is an assumption (4-pole machine at 60 Hz). See A-010.
- Scope of this program: refinement of systems already represented in `crates/bwr-core`. No containment, LOCA, electrical distribution, boron injection, spent fuel, or severe accident models.

## 2. Confirmed at baseline (public GE Hitachi / NRC documents)

| Item | Value | Where used | Source row |
|---|---|---|---|
| Rated thermal power | 870 MWt | `RATED_POWER_W` | S-06 |
| Electrical output | approx. 300 MWe net | calibration target for `TURBINE_DH_RATED` | S-06 |
| Rated dome pressure | 7.2 MPa | `P_RATED` | S-06 |
| Fuel | 240 GNF2 bundles | `N_BUNDLES` | S-06 |
| Control rods | 57 fine-motion control rod drives, bottom entry | `N_RODS` | S-06; S-08 |
| Vessel size | approx. 4 m inner diameter; approx. 26 to 27 m tall | `VESSEL_ID`; `VESSEL_HEIGHT` | S-06 |
| Circulation | natural circulation with a tall chimney; no recirculation pumps | `vessel.rs` loop model | S-06 |
| Decay heat removal | passive isolation condensers; public claim of 7 days of cooling without operator action or AC power | `ic.rs`; `N_IC_TRAINS` is NOT confirmed | S-06; S-10 |
| Overpressure protection | integral RPV isolation valves; no conventional safety relief valves | not yet modelled; see A-014 | S-07 |

"Confirmed" means a public document states the value. It does not mean the simulator reproduces the design exactly.

## 3. Unresolved at baseline (marked ASSUMED in params.rs or README)

| Item | Current value | Needed by | Register status |
|---|---|---|---|
| Rated core flow | 2000 kg/s (scaled from ESBWR per MWt) | Package 4 | esbwr-estimate; scaling by power violates the master constraint and must be re-derived |
| Rated feedwater temperature | 215 degC | Package 4; 6 | assumed |
| Number of IC trains | 3 | Package 7 | assumed |
| IC train duty | 33.75 MWt (ESBWR unit size) | Package 7 | esbwr-estimate |
| Turbine bypass capacity | 100 % of rated steam flow | Package 6 | assumed |
| Protection setpoints and level trip elevations | see A-014 | Package 8 | assumed |
| Vessel internals geometry (elevations; areas; volumes) | see A-006 | Package 4 | assumed |
| Reactivity coefficients and rod worth | see A-003 | Package 5 | calibrated/assumed |

## 4. Evidence precedence

1. BWRX-300 design descriptions and regulator assessments (US first).
2. ESBWR DCD Rev. 10, NUREG-1966 and supplement, for shared concepts, mechanisms, correlations, control principles, and test methodology. Record each use in `esbwr-applicability.csv` before borrowing.
3. Established thermodynamics and component physics applied with BWRX boundary conditions.
4. Documented engineering estimates with sensitivity intervals, including justified ESBWR-based estimates.

## 5. Conflict resolution rule

When two sources disagree:

1. Prefer the document that applies to the reference configuration (US, 60 Hz, current design revision).
2. Prefer the later revision of the same document series.
3. Prefer a regulator assessment that explicitly accepts a value over a vendor proposal that merely states it.
4. If still unresolved, record both values in `parameters.csv` `notes`, keep the status at `assumed` or `unavailable`, and give a sensitivity interval that spans both.
5. Never fill a redacted or withheld value by inference. Record `unavailable` with `interval_lo`/`interval_hi` from physical bounds, or `none`.

## 6. Document kinds

`vendor-proposal` (GE/GE Vernova documents), `regulator-assessment` (NRC/CNSC safety evaluations and reports), `accepted-methodology` (NRC-approved topical reports marked -A), `standard` (ANS/IAPWS/ASME), `textbook`, `index-page` (web index used only to locate documents).
```

- [ ] **Step 2: Verify the file exists and has the six sections**

Run: `grep -c '^## ' docs/model/design-basis.md`
Expected: `6`

- [ ] **Step 3: Commit**

```bash
git add docs/model/design-basis.md
git commit -m "docs(model): add BWRX-300 simulator design basis

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 2: Seed the sources register

**Files:**
- Create: `docs/model/sources.csv`

**Interfaces:**
- Produces: `source_id` values `S-01`..`S-12`, `S-STD-01`..`S-STD-05`, `S-UK-01` referenced by `parameters.csv` and `esbwr-applicability.csv`.

- [ ] **Step 1: Write the header and seed rows**

Write `docs/model/sources.csv` with exactly this content (one line per row; `UNRETRIEVED` placeholders are replaced in Step 3):

```csv
source_id,document_id,title,revision,date,jurisdiction,url,section,retrieval_date,sha256,kind,notes
S-01,NRC-BWRX-INDEX,NRC BWRX-300 pre-application activities and topical report index,-,-,US,https://www.nrc.gov/facilities-safety/new-reactors/advanced-reactors/who-were-working-with/pre-application-activities/gvh-bwrx-300,-,UNRETRIEVED,UNRETRIEVED,index-page,entry point only; cite the documents it links
S-02,NRC-CRN-INDEX,NRC Clinch River construction permit application and PSAR index,-,-,US,https://www.nrc.gov/facilities-safety/new-reactors/advanced-reactors/who-were-working-with/advanced-reactor-application-projects/tva-clinch-river-cpa,-,UNRETRIEVED,UNRETRIEVED,index-page,PSAR chapters are the primary US plant-level source
S-03,GEV-GDA-LIB,GE Vernova BWRX-300 UK GDA document library,-,-,UK,https://www.gevernova.com/nuclear/uk-smr/documents-list,-,UNRETRIEVED,UNRETRIEVED,index-page,supplementary jurisdiction; check configuration differences before use
S-04,NRC-ESBWR-INDEX,NRC ESBWR design certification index (DCD Rev. 10 and safety evaluation),-,-,US,https://www.nrc.gov/facilities-safety/new-reactors/large-light-water-reactors/design-certification-applications-for-new-reactors/esbwr,-,UNRETRIEVED,UNRETRIEVED,index-page,locate DCD Tier 2 chapters 4 5 6 7 9 10 14 15 16
S-05,NUREG-1966-V2,Final Safety Evaluation Report related to the certification of the ESBWR Volume 2 Chapters 4 through 8,-,-,US,https://www.nrc.gov/regulations-legislation/nureg-series-publications/publications-prepared-by-nrc-staff/final-safety-evaluation-report-related-to-the-certification-of-the-economic-simplified-boiling-water/v2,-,UNRETRIEVED,UNRETRIEVED,regulator-assessment,also index the supplement
S-06,005N9751,BWRX-300 General Description,UNRETRIEVED,UNRETRIEVED,US,https://www.gevernova.com/content/dam/gevernova-nuclear/global/en_us/documents/carbon-free-power/005N9751_Rev_BWRX-300_General_Description.pdf,-,UNRETRIEVED,UNRETRIEVED,vendor-proposal,source of the baseline CONFIRMED values; includes comparison with ESBWR
S-07,NEDO-33910-A,BWRX-300 Reactor Pressure Vessel Isolation and Overpressure Protection,Rev. 2,UNRETRIEVED,US,https://www.nrc.gov/docs/ML2316/ML23167A086.pdf,-,UNRETRIEVED,UNRETRIEVED,accepted-methodology,integral isolation valves; no SRVs
S-08,NEDO-33912-A,BWRX-300 Reactivity Control,Rev. 2,UNRETRIEVED,US,https://www.nrc.gov/docs/ML2304/ML23048A018.pdf,-,UNRETRIEVED,UNRETRIEVED,accepted-methodology,FMCRD arrangement; shutdown margin methodology
S-09,NRC-CNSC-CONTAINMENT,NRC/CNSC joint report on BWRX-300 containment evaluation method,-,UNRETRIEVED,US;CA,https://www.nrc.gov/docs/ML2209/ML22091A201.pdf,-,UNRETRIEVED,UNRETRIEVED,regulator-assessment,notes IC function and reactor connections differ from ESBWR
S-10,NEDO-33934,BWRX-300 Safety Strategy,Rev. 2,UNRETRIEVED,US,https://www.nrc.gov/docs/ML2607/ML26077A383.pdf,-,UNRETRIEVED,UNRETRIEVED,vendor-proposal,ICS mission time and credited systems
S-11,NEDO-34270-A,BWRX-300 Stability Analysis,Rev. 2,UNRETRIEVED,US,https://www.nrc.gov/docs/ML2615/ML26159A164.pdf,-,UNRETRIEVED,UNRETRIEVED,accepted-methodology,natural-circulation stability claims
S-12,NRC-STABILITY-SE,NRC safety evaluation of BWRX-300 stability analysis,-,UNRETRIEVED,US,https://www.nrc.gov/docs/ML2607/ML26078A299.pdf,-,UNRETRIEVED,UNRETRIEVED,regulator-assessment,
S-UK-01,GDA-CH4,BWRX-300 UK Generic Design Assessment Chapter 4 (Reactor),UNRETRIEVED,UNRETRIEVED,UK,https://www.gevernova.com/nuclear/uk-smr/documents-list,-,UNRETRIEVED,UNRETRIEVED,vendor-proposal,README cites this; locate exact PDF via S-03
S-STD-01,CODATA-2018,CODATA recommended values of the fundamental physical constants 2018,2018,2019,-,https://physics.nist.gov/cuu/Constants/,g_n,UNRETRIEVED,UNRETRIEVED,standard,standard gravity 9.80665 m/s2; engine uses 9.81
S-STD-02,KEEPIN-1965,Physics of Nuclear Kinetics (Keepin),1965,1965,-,-,Table 4-7 U-235 thermal,UNRETRIEVED,none,textbook,six-group delayed neutron data
S-STD-03,NUCLEAR-DATA-XE,Xenon-135 and iodine-135 decay constants and yields (ENDF/B or Lamarsh),-,-,-,https://www.nndc.bnl.gov/,-,UNRETRIEVED,none,standard,cite the exact table used
S-STD-04,TODREAS-KAZIMI,Nuclear Systems Volume 1 (Todreas and Kazimi) decay heat approximation 0.066 t^-0.2,2nd ed.,2012,-,-,Chapter 3,UNRETRIEVED,none,textbook,current decay-heat fit target; replaced in Package 5 by a transcribed reference table
S-STD-05,IAPWS-IF97,IAPWS Industrial Formulation 1997 for the thermodynamic properties of water and steam (via CoolProp),2007,2007,-,http://www.iapws.org/relguide/IF97-Rev.html,-,UNRETRIEVED,none,standard,steam table generator tools/gen_steam_table.py
```

- [ ] **Step 2: Verify shape**

Run: `awk -F',' 'NF!=12{print NR": "NF" fields"}' docs/model/sources.csv`
Expected: no output (every line has 12 fields).

- [ ] **Step 3: Retrieve each PDF source and fill revision, date, retrieval date, and checksum**

For each of S-05, S-06, S-07, S-08, S-09, S-10, S-11, S-12 (and the DCD chapters located via S-04, added as new rows `S-ESBWR-04`, `S-ESBWR-05`, ... one per Tier 2 chapter):

```bash
mkdir -p ~/ai/bwrsim-sources
curl -L -o ~/ai/bwrsim-sources/005N9751.pdf "https://www.gevernova.com/content/dam/gevernova-nuclear/global/en_us/documents/carbon-free-power/005N9751_Rev_BWRX-300_General_Description.pdf"
shasum -a 256 ~/ai/bwrsim-sources/005N9751.pdf
date +%Y-%m-%d
```

Expected: a 64-hex-character digest followed by the path; today's date. Copy the digest into the `sha256` field, today's date into `retrieval_date`, and the revision and issue date printed on the document's cover into `revision` and `date`. Repeat per document. Source PDFs stay outside the repository (`~/ai/bwrsim-sources/` is not tracked); only the checksum is recorded. Index pages (S-01..S-04) keep `sha256` = `none` and get only `retrieval_date`.

- [ ] **Step 4: Verify no `UNRETRIEVED` remains in a PDF row**

Run: `grep -n UNRETRIEVED docs/model/sources.csv | grep -v index-page`
Expected: no output. (Rows S-STD-02..04 use `none` for `sha256` because they are print books; their `retrieval_date` is the date the citation was verified.)

- [ ] **Step 5: Commit**

```bash
git add docs/model/sources.csv
git commit -m "docs(model): seed public source register with checksums

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 3: Register enforcement test (written before the register exists)

**Files:**
- Create: `crates/bwr-core/tests/register.rs`

**Interfaces:**
- Consumes: `crates/bwr-core/src/params.rs` (via `include_str!`), `docs/model/parameters.csv` (read at test time).
- Produces: a failing test until Task 4 creates the CSV; later packages keep it green.

- [ ] **Step 1: Write the failing test**

Create `crates/bwr-core/tests/register.rs`:

```rust
//! Enforces the parameter register: every `pub const` in params.rs has a row in
//! docs/model/parameters.csv, every row has the locked 13-column shape, and
//! every status is one of the six allowed values.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

const PARAMS_RS: &str = include_str!("../src/params.rs");
const ALLOWED_STATUS: [&str; 6] = ["confirmed", "derived", "calibrated", "esbwr-estimate", "assumed", "unavailable"];
const COLUMNS: usize = 13;
const COL_ID: usize = 0;
const COL_STATUS: usize = 6;
const COL_INTERVAL_LO: usize = 8;
const COL_INTERVAL_HI: usize = 9;
const COL_ASSUMPTION: usize = 10;

fn register_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/model/parameters.csv")
}

/// Names of every `pub const NAME:` in params.rs, found by a plain line scan.
fn const_names() -> Vec<String> {
    PARAMS_RS
        .lines()
        .map(str::trim_start)
        .filter_map(|line| line.strip_prefix("pub const "))
        .filter_map(|rest| rest.split(':').next())
        .map(|name| name.trim().to_string())
        .collect()
}

/// Data rows of parameters.csv (header skipped, blank lines ignored), split on commas.
fn rows() -> Vec<Vec<String>> {
    let text = std::fs::read_to_string(register_path())
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", register_path().display()));
    text.lines()
        .skip(1)
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.split(',').map(|field| field.trim().to_string()).collect())
        .collect()
}

#[test]
fn params_rs_scan_finds_the_constants() {
    let names = const_names();
    assert!(names.len() >= 100, "expected at least 100 pub consts in params.rs, found {}", names.len());
    assert!(names.contains(&"RATED_POWER_W".to_string()));
    assert!(names.contains(&"L1_CM".to_string()));
}

#[test]
fn every_params_const_is_registered() {
    let ids: HashSet<String> = rows().into_iter().map(|r| r[COL_ID].clone()).collect();
    let missing: Vec<String> = const_names().into_iter().filter(|n| !ids.contains(n)).collect();
    assert!(missing.is_empty(), "constants in params.rs with no parameters.csv row: {missing:?}");
}

#[test]
fn every_row_has_valid_shape_and_status() {
    let rows = rows();
    assert!(!rows.is_empty(), "parameters.csv has no data rows");
    let mut seen = HashSet::new();
    for (i, r) in rows.iter().enumerate() {
        let line = i + 2;
        let id = r.get(COL_ID).cloned().unwrap_or_default();
        assert_eq!(r.len(), COLUMNS, "line {line} ({id}): {} fields; fields must not contain commas", r.len());
        assert!(seen.insert(id.clone()), "line {line}: duplicate id {id}");
        let status = r[COL_STATUS].as_str();
        assert!(ALLOWED_STATUS.contains(&status), "line {line} ({id}): status {status:?} not in {ALLOWED_STATUS:?}");
        if status == "assumed" || status == "esbwr-estimate" {
            assert!(r[COL_ASSUMPTION].starts_with("A-"), "line {line} ({id}): {status} rows need an assumption_id like A-001");
        }
        if status == "unavailable" {
            let lo = r[COL_INTERVAL_LO].as_str();
            let hi = r[COL_INTERVAL_HI].as_str();
            let both_none = lo == "none" && hi == "none";
            let both_numeric = lo.parse::<f64>().is_ok() && hi.parse::<f64>().is_ok();
            assert!(both_none || both_numeric, "line {line} ({id}): unavailable rows need a numeric interval or none/none");
        }
    }
}
```

- [ ] **Step 2: Run it and confirm it fails because the register does not exist yet**

Run: `cargo test --release -p bwr-core --test register 2>&1 | tail -20`
Expected: `params_rs_scan_finds_the_constants ... ok`; `every_params_const_is_registered ... FAILED` and `every_row_has_valid_shape_and_status ... FAILED` with a panic message containing `cannot read` and `docs/model/parameters.csv`.

- [ ] **Step 3: Commit the failing test**

```bash
git add crates/bwr-core/tests/register.rs
git commit -m "test: add parameter register enforcement (fails until register exists)

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 4: Parameter register — every constant in params.rs

**Files:**
- Create: `docs/model/parameters.csv`

**Interfaces:**
- Consumes: `source_id`s from Task 2; `A-nnn` ids defined in Task 7 (ids are fixed here; Task 7 writes their text).
- Produces: the register that `tests/register.rs` checks and that every later package appends to.

- [ ] **Step 1: Write the header and the 111 params.rs rows**

Write `docs/model/parameters.csv` with the header line followed by these rows, exactly as given (the executor may correct a typo in a value only after checking `params.rs`; do not change statuses):

```csv
id,value,unit,definition,reference_state,source_design,status,source_ids,interval_lo,interval_hi,assumption_id,location,notes
G,9.81,m/s2,Gravitational acceleration,-,physics,confirmed,S-STD-01,,,,params.rs,standard gravity 9.80665 rounded
RATED_POWER_W,8.70e8,W,Rated core thermal power,rated,BWRX-300,confirmed,S-06,,,,params.rs,
N_RODS,57,-,Number of fine-motion control rod drives (bottom entry),-,BWRX-300,confirmed,S-06;S-08,,,,params.rs,
N_BUNDLES,240,-,Number of GNF2 fuel bundles,-,BWRX-300,confirmed,S-06,,,,params.rs,
ROD_FULL_OUT,48,position,Rod position scale: 0 full in to 48 full out; one notch = 2 positions,-,BWR-generic,assumed,,none,none,A-001,params.rs,legacy BWR convention; verify FMCRD travel convention in Package 5
CORE_HEIGHT,3.81,m,Active fuel length,-,BWR-generic,assumed,,3.6,3.9,A-001,params.rs,GNF2 typical
ROD_SPEED,0.4,position/s,FMCRD motor drive speed (about 30 mm/s),-,BWR-generic,assumed,,0.2,0.8,A-001,params.rs,
SCRAM_TIME,3.0,s,Hydraulic scram full insertion time,-,BWR-generic,assumed,,2.0,5.0,A-001,params.rs,
BETA,0.0060,dk/k,Effective delayed neutron fraction,-,physics,assumed,S-STD-02,0.0050,0.0070,A-002,params.rs,U-235 thermal; exposure dependence ignored
BETA_FRAC,0.033;0.219;0.196;0.395;0.115;0.042,-,Six-group delayed neutron fractions (sum 1),-,physics,assumed,S-STD-02,none,none,A-002,params.rs,promote to confirmed once S-STD-02 table is cited
LAMBDA_DN,0.0124;0.0305;0.111;0.301;1.14;3.01,1/s,Six-group precursor decay constants,-,physics,assumed,S-STD-02,none,none,A-002,params.rs,
GEN_TIME,4.0e-5,s,Prompt neutron generation time,-,BWR-generic,assumed,,2e-5,6e-5,A-002,params.rs,
SOURCE,8.0e-6,rel-power/s,Neutron source strength in relative power units per second,-,tuning,assumed,,1e-6,1e-4,A-002,params.rs,sets subcritical SRM count rate
RHO_EXCESS_COLD,0.130,dk/k,Excess reactivity of the cold clean all-rods-out core,cold xenon-free,tuning,calibrated,,,,A-003,params.rs,tuned so HFP is critical with about 12 % rod worth inserted
ROD_WORTH_TOTAL,0.200,dk/k,Total control rod worth,-,tuning,calibrated,,,,A-003,params.rs,
DOPPLER_A,0.0024,dk/k per sqrt(K),Doppler form constant rho = -A*(sqrt(Tf) - sqrt(293.15)),-,BWR-generic,assumed,,0.0015,0.0035,A-003,params.rs,
VOID_COEF,-0.080,dk/k per unit void,Core-average void coefficient,HFP,BWR-generic,assumed,,-0.12,-0.05,A-003,params.rs,
MOD_TEMP_COEF,-6.0e-5,dk/k per K,Moderator temperature coefficient relative to 20 degC,-,BWR-generic,assumed,,-1.5e-4,0,A-003,params.rs,
XENON_WORTH_EQ,0.028,dk/k,Equilibrium xenon worth at rated power,HFP,BWR-generic,assumed,,0.02,0.035,A-003,params.rs,
LAMBDA_I,2.87e-5,1/s,I-135 decay constant,-,physics,assumed,S-STD-03,none,none,A-004,params.rs,promote to confirmed once table cited
LAMBDA_XE,2.09e-5,1/s,Xe-135 decay constant,-,physics,assumed,S-STD-03,none,none,A-004,params.rs,
GAMMA_I,0.0639,-,I-135 cumulative fission yield,-,physics,assumed,S-STD-03,none,none,A-004,params.rs,
GAMMA_XE,0.00237,-,Xe-135 direct fission yield,-,physics,assumed,S-STD-03,none,none,A-004,params.rs,
SIGMA_PHI_XE,7.95e-5,1/s,Xe-135 absorption rate sigma_a*phi at rated flux (2.65e-18 cm2 x 3.0e13),HFP,BWR-generic,assumed,,5e-5,1.2e-4,A-004,params.rs,
DECAY_GROUPS,9 pairs (see params.rs),fraction;1/s,Decay-heat group amplitudes and decay constants,infinite operation,tuning,calibrated,S-STD-04,,,A-005,params.rs,fit to 0.066 t^-0.2 by tools/fit_decay_heat.py; refit in Package 5
DIRECT_COOLANT_HEAT,0.025,-,Fraction of fission power deposited directly in coolant,-,BWR-generic,assumed,,0.02,0.04,A-005,params.rs,
FUEL_HEAT_CAP,1.47e7,J/K,Lumped fuel heat capacity (about 49 t UO2 x 300 J/kgK),-,BWR-generic,assumed,,1.2e7,1.8e7,A-005,params.rs,
FUEL_RESISTANCE,264/(RATED_POWER_W*(1-DIRECT_COOLANT_HEAT)),K/W,Fuel-to-coolant thermal resistance,rated,tuning,derived,,,,A-005,params.rs,derived from assumed 264 K average fuel-to-coolant rise
P_RATED,7.2e6,Pa,Rated dome pressure,rated,BWRX-300,confirmed,S-06,,,,params.rs,
VESSEL_ID,4.0,m,Vessel inner diameter,-,BWRX-300,confirmed,S-06,,,,params.rs,public figure is approximate
VESSEL_HEIGHT,26.7,m,Vessel inner height,-,BWRX-300,confirmed,S-06,26,27,,params.rs,public figure is 26 to 27 m
Z_CORE_BOTTOM,2.5,m,Elevation of bottom of active fuel above vessel bottom,-,BWR-generic,assumed,,2.0,3.5,A-006,params.rs,
Z_TAF,Z_CORE_BOTTOM+CORE_HEIGHT,m,Top of active fuel elevation,-,-,derived,,,,A-006,params.rs,
CHIMNEY_HEIGHT,9.0,m,Chimney height above TAF,-,BWR-generic,assumed,,7,11,A-006,params.rs,
Z_CHIMNEY_TOP,Z_TAF+CHIMNEY_HEIGHT,m,Chimney top elevation,-,-,derived,,,,A-006,params.rs,
Z_DRYER,21.0,m,Dryer skirt elevation,-,BWR-generic,assumed,,19,23,A-006,params.rs,
Z_NWL,Z_CHIMNEY_TOP+3.0,m,Normal water level elevation,-,BWR-generic,assumed,,none,none,A-006,params.rs,3 m above chimney top is assumed
A_VESSEL,PI*VESSEL_ID^2/4,m2,Vessel cross-sectional area,-,-,derived,,,,A-006,params.rs,
A_CORE,2.6,m2,Core flow area including bypass,-,BWR-generic,assumed,,2.2,3.2,A-006,params.rs,
A_CHIMNEY,8.55,m2,Chimney inner flow area,-,BWR-generic,assumed,,7,10,A-006,params.rs,
A_DOWNCOMER,A_VESSEL-PI*3.5^2/4,m2,Downcomer annulus area (3.5 m shroud OD assumed),-,BWR-generic,assumed,,none,none,A-006,params.rs,derived from an assumed shroud OD
A_SEPARATOR_REGION,9.0,m2,Free area for water level between chimney top and dryer skirt,-,BWR-generic,assumed,,7,11,A-006,params.rs,
A_DOME,10.0,m2,Free area above the dryer skirt,-,BWR-generic,assumed,,8,12,A-006,params.rs,
V_LOWER_PLENUM,22.0,m3,Lower plenum fluid volume,-,BWR-generic,assumed,,15,30,A-006,params.rs,
V_CORE,A_CORE*CORE_HEIGHT,m3,Core fluid volume,-,-,derived,,,,A-006,params.rs,
V_CHIMNEY,A_CHIMNEY*CHIMNEY_HEIGHT,m3,Chimney fluid volume,-,-,derived,,,,A-006,params.rs,
V_DOWNCOMER,A_DOWNCOMER*(Z_CHIMNEY_TOP-Z_CORE_BOTTOM),m3,Downcomer fluid volume,-,-,derived,,,,A-006,params.rs,
V_SEP_REGION,A_SEPARATOR_REGION*(Z_DRYER-Z_CHIMNEY_TOP),m3,Separator region fluid volume,-,-,derived,,,,A-006,params.rs,
V_DOME,A_DOME*(VESSEL_HEIGHT-Z_DRYER),m3,Dome volume,-,-,derived,,,,A-006,params.rs,
V_VESSEL_FLUID,sum of region volumes,m3,Total vessel fluid volume,-,-,derived,,,,A-006,params.rs,
METAL_HEAT_CAP,2.75e8,J/K,Vessel and internals heat capacity (550 t x 500 J/kgK),-,BWR-generic,assumed,,2e8,3.5e8,A-006,params.rs,
HEAT_LOSS_HOT,1.0e6,W,Insulation heat loss at hot conditions,hot,BWR-generic,assumed,,0.3e6,2e6,A-006,params.rs,
CORE_FLOW_RATED,2000,kg/s,Natural-circulation core flow at rated conditions,rated,ESBWR,esbwr-estimate,S-04,1500,2500,A-007,params.rs,scaled from ESBWR per MWt; violates no-power-scaling rule; re-derive from geometry in Package 4
LOOP_INERTIA,7.3,1/m,Natural-circulation loop inertia sum(L/A),-,BWR-generic,assumed,,4,12,A-007,params.rs,
C0,1.13,-,Drift-flux distribution parameter,-,physics,assumed,,1.0,1.2,A-007,params.rs,Zuber-Findlay typical
N_AXIAL,12,-,Number of axial nodes in the core thermal model,-,numerical,assumed,,6,24,A-016,params.rs,convergence check in Package 3
T_FW_RATED,215,degC,Rated feedwater temperature,rated,BWR-generic,assumed,,200,230,A-008,params.rs,
STEAM_FLOW_RATED,470,kg/s,Rated steam flow = 870 MW / (h_g(7.2 MPa) - h_f(215 degC)),rated,-,derived,,,,A-008,params.rs,depends on T_FW_RATED
STEAM_LINE_DP,0.2e6,Pa,Main steam line pressure drop at rated flow,rated,BWR-generic,assumed,,0.1e6,0.4e6,A-008,params.rs,
N_STEAM_LINES,2,-,Number of main steam lines,-,BWR-generic,assumed,,none,none,A-008,params.rs,no CONFIRMED/ASSUMED marker in code; verify in Package 6
MSIV_STROKE,4.0,s,MSIV stroke time,-,BWR-generic,assumed,,3,5,A-008,params.rs,
C_TCV,1.10*STEAM_FLOW_RATED/7.0e6,kg/s/Pa,Turbine control valve flow coefficient at full open,-,-,derived,,,,A-008,params.rs,
BYPASS_CAPACITY,1.0,-,Turbine bypass capacity as fraction of rated steam flow,-,BWR-generic,assumed,,0.3,1.0,A-008,params.rs,README lists as unconfirmed
C_BYPASS,BYPASS_CAPACITY*STEAM_FLOW_RATED/7.0e6,kg/s/Pa,Bypass valve flow coefficient at full open,-,-,derived,,,,A-008,params.rs,
PRESSURE_REG_SPAN,0.25e6,Pa,Pressure regulator proportional span (100 % demand per span above setpoint),-,tuning,assumed,,0.1e6,0.5e6,A-009,params.rs,no marker in code
PRESSURE_SETPOINT_DEFAULT,6.95e6,Pa,Default pressure setpoint giving 7.2 MPa dome at rated steam flow,rated,tuning,calibrated,,,,A-009,params.rs,
TURBINE_DH_RATED,0.703e6,J/kg,Effective turbine enthalpy drop to shaft at rated conditions,rated,tuning,calibrated,,,,A-010,params.rs,gives about 315 MWe gross / 300 MWe net
GENERATOR_EFF,0.985,-,Generator efficiency,-,BWR-generic,assumed,,0.98,0.99,A-010,params.rs,
RATED_RPM,1800,rpm,Turbine-generator synchronous speed,-,BWR-generic,assumed,,none,none,A-010,params.rs,4-pole machine at 60 Hz assumed
GRID_HZ,60,Hz,Grid frequency of the reference configuration,-,BWRX-300,confirmed,S-02,,,,params.rs,North American reference configuration decision (design-basis.md section 1)
TG_INERTIA,74300,kg m2,Turbine-generator rotational inertia (H about 4 s on 330 MVA),-,BWR-generic,assumed,,5e4,1e5,A-010,params.rs,
TG_LOSSES,8.0e6,W,No-load windage and friction losses at rated speed,rated,BWR-generic,assumed,,4e6,12e6,A-010,params.rs,
OVERSPEED_TRIP_RPM,1980,rpm,Mechanical overspeed trip (110 %),-,BWR-generic,assumed,,1960,2000,A-010,params.rs,
SPEED_RAMP,5.0,rpm/s,Speed reference ramp rate,-,tuning,assumed,,2,10,A-010,params.rs,
CW_INLET_C,20,degC,Circulating water inlet temperature (external boundary condition),-,BWR-generic,assumed,,5,30,A-011,params.rs,declared boundary; site dependent
CW_FLOW_PER_PUMP,7500,kg/s,Circulating water flow per pump,-,BWR-generic,assumed,,5000,10000,A-011,params.rs,
N_CW_PUMPS,2,-,Number of circulating water pumps,-,BWR-generic,assumed,,none,none,A-011,params.rs,
CONDENSER_UA,5.6e7,W/K,Condenser overall heat transfer conductance,rated,tuning,calibrated,,,,A-011,params.rs,gives about 5.6 kPa at rated load
CONDENSER_VOLUME,3000,m3,Condenser steam-space volume,-,BWR-generic,assumed,,2000,5000,A-011,params.rs,
AIR_INLEAKAGE,0.004,kg/s,Air in-leakage at full vacuum,-,BWR-generic,assumed,,0.001,0.01,A-011,params.rs,
VACUUM_PUMP_TAU,600,s,Air removal time constant,-,BWR-generic,assumed,,300,1200,A-011,params.rs,
CONDENSER_TRIP_P,25.0e3,Pa,Turbine trip on high condenser pressure,-,BWR-generic,assumed,,15e3,35e3,A-014,params.rs,
CONDENSER_BYPASS_BLOCK_P,30.0e3,Pa,Bypass valves blocked above this condenser pressure,-,BWR-generic,assumed,,20e3,40e3,A-014,params.rs,
HOTWELL_CAPACITY,120000,kg,Hotwell capacity,-,BWR-generic,assumed,,80000,200000,A-012,params.rs,
N_FEED_PUMPS,3,-,Number of feedwater pumps (3 x 50 %),-,BWR-generic,assumed,,none,none,A-012,params.rs,
N_COND_PUMPS,2,-,Number of condensate pumps,-,BWR-generic,assumed,,none,none,A-012,params.rs,
FW_PUMP_SHUTOFF_DP,9.5e6,Pa,Feed pump shutoff differential pressure,-,BWR-generic,assumed,,8.5e6,11e6,A-012,params.rs,
FW_PUMP_RUNOUT,400,kg/s,Feed pump runout flow,-,BWR-generic,assumed,,300,500,A-012,params.rs,
CONDENSATE_PRESSURE,1.5e6,Pa,Condensate booster pressure at feed pump suction,-,BWR-generic,assumed,,1.0e6,2.5e6,A-012,params.rs,
C_FCV,1.75,kg/s/sqrt(Pa),Feedwater control valve coefficient at full open,-,tuning,assumed,,1.2,2.5,A-012,params.rs,equal-percentage trim
N_IC_TRAINS,3,-,Number of isolation condenser trains,-,BWR-generic,assumed,,2,4,A-013,params.rs,README lists as unconfirmed; ESBWR has 4
IC_TRAIN_RATED_W,33.75e6,W,Heat removal per train at rated pressure with a 100 degC pool,rated,ESBWR,esbwr-estimate,S-04,20e6,45e6,A-013,params.rs,ESBWR unit size; BWRX duty to be resolved in Package 7
IC_VALVE_STROKE,15.0,s,IC condensate return valve stroke time,-,BWR-generic,assumed,,10,30,A-013,params.rs,
IC_POOL_MASS,1.5e6,kg,IC pool water mass per train,-,BWR-generic,assumed,,0.5e6,3e6,A-013,params.rs,re-derive from the 7-day claim in Package 7
SCRAM_HIGH_PRESSURE,7.50e6,Pa,Reactor high pressure scram setpoint,-,BWR-generic,assumed,,7.4e6,7.8e6,A-014,params.rs,
IC_HIGH_PRESSURE,7.60e6,Pa,IC auto-initiation high pressure setpoint,-,BWR-generic,assumed,,7.5e6,8.0e6,A-014,params.rs,
VESSEL_DESIGN_PRESSURE,8.62e6,Pa,Vessel design pressure,-,BWR-generic,assumed,,8.3e6,9.0e6,A-014,params.rs,
APRM_HIGH_RUN,1.18,rel,APRM high flux scram in RUN,-,BWR-generic,assumed,,1.12,1.20,A-014,params.rs,
APRM_HIGH_SETDOWN,0.15,rel,APRM high flux scram outside RUN (setdown),-,BWR-generic,assumed,,0.12,0.20,A-014,params.rs,
IRM_HIGH_HIGH,120,% of range,IRM high-high scram,-,BWR-generic,assumed,,110,125,A-014,params.rs,legacy IRM arrangement not confirmed for BWRX
MSIV_LOW_PRESSURE_RUN,5.9e6,Pa,MSIV isolation on low steam line pressure in RUN,-,BWR-generic,assumed,,5.5e6,6.2e6,A-014,params.rs,
L8_CM,100,cm,Level 8 high level trip relative to NWL,-,BWR-generic,assumed,,none,none,A-014,params.rs,
L7_CM,50,cm,Level 7 high level alarm relative to NWL,-,BWR-generic,assumed,,none,none,A-014,params.rs,
L4_CM,-50,cm,Level 4 low level alarm relative to NWL,-,BWR-generic,assumed,,none,none,A-014,params.rs,
L3_CM,-100,cm,Level 3 low level scram relative to NWL,-,BWR-generic,assumed,,none,none,A-014,params.rs,
L2_CM,-400,cm,Level 2 low-low isolation and IC initiation relative to NWL,-,BWR-generic,assumed,,none,none,A-014,params.rs,
L1_CM,-600,cm,Level 1 relative to NWL,-,BWR-generic,assumed,,none,none,A-014,params.rs,not used by any trip at baseline
HOUSE_BASE,4.0e6,W,Base house load,-,BWR-generic,assumed,,2e6,8e6,A-015,params.rs,
FW_PUMP_POWER,3.5e6,W,Electrical power per running feed pump,-,BWR-generic,assumed,,2e6,5e6,A-015,params.rs,
COND_PUMP_POWER,1.0e6,W,Electrical power per running condensate pump,-,BWR-generic,assumed,,0.5e6,2e6,A-015,params.rs,
CW_PUMP_POWER,2.0e6,W,Electrical power per running circulating water pump,-,BWR-generic,assumed,,1e6,4e6,A-015,params.rs,
```

- [ ] **Step 2: Run the register tests**

Run: `cargo test --release -p bwr-core --test register 2>&1 | tail -8`
Expected: `test result: ok. 3 passed; 0 failed`.

If `every_params_const_is_registered` lists a missing name, add that row (the scan is authoritative; the table above was transcribed by hand from `params.rs`).

- [ ] **Step 3: Commit**

```bash
git add docs/model/parameters.csv
git commit -m "docs(model): register every params.rs constant with status and interval

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 5: Parameter register — hardcoded values outside params.rs

**Files:**
- Modify: `docs/model/parameters.csv` (append rows)

**Interfaces:**
- Produces: ids of the form `<file>.<name>` that Packages 3 to 8 cite when they move a value into `params.rs` or remove it.

- [ ] **Step 1: Append the engine rows**

Append these rows (grouped by file) to `docs/model/parameters.csv`:

```csv
vessel.C_HEAD_VENT,5.0/7.0e6,kg/s/Pa,Head vent line flow coefficient above atmospheric,-,tuning,assumed,,none,none,A-016,vessel.rs:18,
vessel.W_MIN,5.0,kg/s,Minimum core flow (local recirculation inside the shroud),-,numerical,assumed,,1,20,A-016,vessel.rs:20,numerical floor
vessel.vent_sensible_term,0.02*(T-100),kg/s,Extra vent flow term proportional to superheat above 100 degC,-,tuning,assumed,,none,none,A-016,vessel.rs step(),no physical basis; review in Package 4
vessel.drift_flux_vgj_coef,1.41,-,Drift velocity coefficient in vgj = 1.41*(sigma*g*drho/rho_f^2)^0.25,-,physics,assumed,,none,none,A-007,vessel.rs drift_flux_void(),Zuber-Findlay churn-turbulent form
vessel.void_clamp,0.999,-,Upper clamp on nodal void fraction,-,numerical,assumed,,none,none,A-016,vessel.rs drift_flux_void(),
vessel.chopped_cosine_extrapolation,0.3,m,Extrapolation length at each end of the axial power shape,-,BWR-generic,assumed,,0.1,0.5,A-005,vessel.rs core_th(),
vessel.phi2_form,1+0.5*x*(rho_f/rho_g-1),-,Two-phase friction multiplier form (damped homogeneous),-,physics,assumed,,none,none,A-007,vessel.rs phi2(),
vessel.eos_t_bounds,1..349.5,degC,Bisection bounds of the equation-of-state solve; silently clamps outside,-,numerical,assumed,,none,none,A-016,vessel.rs solve_eos(),Package 3 replaces silent clamp with DomainError
vessel.subcooled_threshold,100,degC,Below this the vessel is treated as vented at atmospheric pressure,-,BWR-generic,assumed,,none,none,A-016,vessel.rs energy_at()/solve_eos(),Package 4 replaces with regime model
vessel.tau_dc_rho,750,kg/m3,Fixed liquid density used in the downcomer mixing time constant,-,numerical,assumed,,none,none,A-016,vessel.rs step(),should use actual downcomer density
vessel.tau_dc_clamp,2..600,s,Clamp on downcomer mixing time constant,-,numerical,assumed,,none,none,A-016,vessel.rs step(),
vessel.void_lag_core,0.5,s,First-order lag on core void fraction,-,tuning,assumed,,0.2,2.0,A-016,vessel.rs step(),
vessel.void_lag_chimney,2.0,s,First-order lag on chimney void fraction,-,tuning,assumed,,0.5,5.0,A-016,vessel.rs step(),
vessel.mass_floor,1000,kg,Inventory floor applied after each mass update,-,numerical,assumed,,none,none,A-016,vessel.rs step(),creates mass; removed in Package 3
vessel.spill_band,0.5,m,Smoothing band below chimney top over which loop driving head is cut off,-,tuning,assumed,,0.1,1.0,A-007,vessel.rs drive(),
vessel.heatup_rate_lag,30,s,Filter time constant on the displayed heatup rate,-,tuning,assumed,,10,60,A-017,vessel.rs step(),
vessel.heat_loss_ref_span,268,K,Temperature span normalising heat loss (T-20)/268,-,tuning,assumed,,none,none,A-006,vessel.rs step(),
vessel.core_coverage_void_floor,0.2,-,Floor on (1-alpha) in the core coverage calculation,-,numerical,assumed,,none,none,A-016,vessel.rs core_coverage(),double-corrects void; removed in Package 3
vessel.init_relaxation,0.7/0.3,-,Under-relaxation weights in Vessel::new flow iteration (200 iterations),-,numerical,assumed,,none,none,A-016,vessel.rs new(),
reactor.rod_weight_r_ext,5.3,grid units,Extrapolated radius for J0 rod worth weighting,-,tuning,assumed,,4.5,6.0,A-003,reactor.rs rod_weights(),
reactor.rod_layout_radius2,17,grid units squared,Rod layout radius squared on the 9x9 grid (57 cells),-,tuning,assumed,,none,none,A-001,reactor.rs rod_layout(),
reactor.s_curve_form,f-sin(2*pi*f)/(2*pi),-,Integral rod worth shape (cosine flux),-,physics,assumed,,none,none,A-003,reactor.rs s_curve(),
reactor.sri_group,rods at |dx|=2 and |dy|=2,-,Select Rod Insert group (4 symmetric rods about 1.4 % dk/k),-,tuning,assumed,,none,none,A-003,reactor.rs sri_group(),reassess in Package 6
reactor.kinetics_substep,1.0e-3,s,Fixed kinetics substep,-,numerical,assumed,,none,none,A-016,reactor.rs step(),Package 3 makes adaptive
reactor.htc_factor_floor,0.02,-,Floor on fuel-to-coolant heat transfer factor when uncovered,-,tuning,assumed,,none,none,A-005,reactor.rs step(),
reactor.period_meter_tau,1.0,s,Period meter smoothing time constant,-,tuning,assumed,,0.5,3.0,A-017,kinetics.rs step(),
kinetics.n_floor,1e-14,rel,Floor applied to neutron population after each implicit step,-,numerical,assumed,,none,none,A-016,kinetics.rs step(),hides nonpositive denominator; Package 3
ic.ua_pool_ref,100,degC,Pool temperature at which IC_TRAIN_RATED_W is defined,rated,ESBWR,esbwr-estimate,S-04,none,none,A-013,ic.rs ua(),
ic.tube_lag,20,s,Tube drain/fill time constant,-,tuning,assumed,,5,60,A-013,ic.rs step(),
ic.coverage_threshold,40,% pool level,Pool level below which tube coverage falls linearly to zero,-,tuning,assumed,,20,60,A-013,ic.rs step(),
ic.idle_pool_cooling,0.0005,K/s,Idle pool cooling rate when heat < 0.1 MW and pool > 40 degC,-,tuning,assumed,,none,none,A-013,ic.rs step(),unaccounted energy removal; Package 7
ic.idle_heat_threshold,1.0e5,W,Heat below which idle pool cooling applies,-,tuning,assumed,,none,none,A-013,ic.rs step(),
ic.pool_initial_t,40,degC,Initial pool temperature,-,tuning,assumed,,20,50,A-013,ic.rs new(),
ic.pool_cp,4186,J/kgK,Pool water specific heat,-,physics,confirmed,S-STD-05,,,,ic.rs step(),
ic.pool_hfg,2.257e6,J/kg,Pool latent heat at atmospheric,-,physics,confirmed,S-STD-05,,,,ic.rs step(),
bop.steam_line_k_form,STEAM_LINE_DP/STEAM_FLOW_RATED^2/m^2,Pa s2/kg2,Steam line loss coefficient scaled by averaged MSIV opening,-,tuning,assumed,,none,none,A-008,bop.rs compute_flows(),averages line positions; Package 6
bop.msiv_min_fraction,1e-4,-,Averaged MSIV opening below which flow is zero,-,numerical,assumed,,none,none,A-016,bop.rs compute_flows(),
bop.p_ref_chest_threshold,0.5e6,Pa,Chest pressure above which it is used as the valve sizing reference,-,tuning,assumed,,none,none,A-009,bop.rs SteamSystem::step(),
bop.p_ref_dome_offset,0.05e6,Pa,Offset subtracted from dome pressure for valve sizing when chest pressure is low,-,tuning,assumed,,none,none,A-009,bop.rs SteamSystem::step(),
bop.p_ref_floor,0.2e6,Pa,Floor on the valve sizing reference pressure,-,tuning,assumed,,none,none,A-009,bop.rs SteamSystem::step(),
bop.tcv_open_rate,0.10,1/s,TCV opening rate,-,BWR-generic,assumed,,0.05,0.5,A-009,bop.rs SteamSystem::step(),
bop.tcv_close_rate,0.5,1/s,TCV closing rate (normal),-,BWR-generic,assumed,,0.2,2.0,A-009,bop.rs SteamSystem::step(),
bop.tcv_trip_close_rate,5.0,1/s,TCV closing rate on trip (0.2 s stroke),-,BWR-generic,assumed,,2,20,A-009,bop.rs SteamSystem::step(),
bop.bypass_auto_rate,2.0,1/s,Bypass valve rate in automatic,-,BWR-generic,assumed,,0.5,5.0,A-009,bop.rs SteamSystem::step(),
bop.bypass_manual_rate,0.2,1/s,Bypass valve rate in manual,-,tuning,assumed,,none,none,A-009,bop.rs SteamSystem::step(),
bop.load_limit_default,1.05,-,Default load limiter,-,tuning,assumed,,none,none,A-010,bop.rs Turbine::new(),
bop.governor_kp,10.0,rel flow per rel speed error,Speed governor proportional gain,-,tuning,assumed,,none,none,A-010,bop.rs tcv_demand(),
bop.governor_ki,0.5,1/s,Speed governor integral gain,-,tuning,assumed,,none,none,A-010,bop.rs tcv_demand(),
bop.governor_integral_clamp,-0.05..0.10,-,Governor integral clamp,-,tuning,assumed,,none,none,A-010,bop.rs tcv_demand(),
bop.roll_margin,0.01,rel flow,Extra steam the turbine may take above the pressure-regulator demand while rolling,-,tuning,assumed,,none,none,A-010,bop.rs tcv_demand(),
bop.turbine_pf_form,ln(p_chest/p_cond)/ln(7.0e6/5.6e3) clamped 0..1.2,-,Pressure-ratio factor on turbine work,-,tuning,assumed,,none,none,A-010,bop.rs Turbine::step(),
bop.turbine_extra_loss,0.5e6,W,Fixed loss while rotating,-,tuning,assumed,,none,none,A-010,bop.rs Turbine::step(),
bop.reverse_power_delay,3.0,s,Reverse-power breaker opening delay,-,BWR-generic,assumed,,1,10,A-010,bop.rs Turbine::step(),
bop.sync_speed_window,20,rpm,Breaker close permissive speed window,-,BWR-generic,assumed,,none,none,A-010,bop.rs close_breaker(),
bop.sync_phase_window,15,deg,Breaker close phase window (outside = out-of-phase trip),-,BWR-generic,assumed,,none,none,A-010,bop.rs close_breaker(),
bop.sync_slip_window,0.25,Hz,Breaker close slip window,-,BWR-generic,assumed,,none,none,A-010,bop.rs close_breaker(),
bop.condenser_r_air,287,J/kgK,Gas constant of air in the condenser air model,-,physics,confirmed,S-STD-01,,,,bop.rs Condenser,
bop.condenser_t_air,303,K,Fixed air temperature in the condenser air model,-,tuning,assumed,,290,320,A-011,bop.rs Condenser,
bop.condenser_p_steam_init,2.3e3,Pa,Initial condenser steam partial pressure,-,tuning,assumed,,none,none,A-011,bop.rs Condenser::new(),
bop.condenser_p_air_init_vacuum,0.3e3,Pa,Initial air partial pressure with vacuum,-,tuning,assumed,,none,none,A-011,bop.rs Condenser::new(),
bop.condenser_p_cap,1.2*P_ATM,Pa,Cap on condenser pressure,-,numerical,assumed,,none,none,A-011,bop.rs pressure(),
bop.condenser_tau_cw,15,s,Condenser pressure time constant with CW flow,-,tuning,assumed,,5,60,A-011,bop.rs Condenser::step(),
bop.condenser_tau_no_cw,120,s,Condenser pressure time constant without CW flow,-,tuning,assumed,,30,600,A-011,bop.rs Condenser::step(),
bop.condenser_no_cw_floor_t,30,degC,Steam saturation temperature assumed with no CW and no heat load,-,tuning,assumed,,none,none,A-011,bop.rs Condenser::step(),
bop.condenser_p_steam_floor,700,Pa,Floor on steam partial pressure for saturation lookup,-,numerical,assumed,,none,none,A-011,bop.rs Condenser::step(),
bop.hotwell_initial_fraction,0.6,-,Initial hotwell fill fraction,-,tuning,assumed,,none,none,A-012,bop.rs Condenser::new(),
bop.hotwell_makeup_flow,30,kg/s,Automatic makeup/reject flow,-,tuning,assumed,,none,none,A-012,bop.rs Condenser::step(),unaccounted boundary flow; Package 6
bop.hotwell_makeup_band,0.40/0.85,-,Hotwell fractions triggering makeup/reject,-,tuning,assumed,,none,none,A-012,bop.rs Condenser::step(),
bop.hotwell_ok_fraction,0.05,-,Hotwell fraction below which feed suction is lost,-,tuning,assumed,,none,none,A-012,sim.rs step(),
bop.fcv_rangeability,30,-,Equal-percentage valve rangeability,-,BWR-generic,assumed,,20,50,A-012,bop.rs fcv_characteristic(),
bop.fcv_rate,0.1,1/s,FCV stroke rate,-,BWR-generic,assumed,,0.05,0.2,A-012,bop.rs Feedwater::step(),
bop.fw_static_dp,0.15e6,Pa,Static head and piping loss in the feed line,-,tuning,assumed,,none,none,A-012,bop.rs hydraulics(),
bop.fw_pump_curve_form,SHUTOFF_DP*(1-(q/RUNOUT)^2),Pa,Feed pump head curve form,-,BWR-generic,assumed,,none,none,A-012,bop.rs hydraulics(),
bop.fw_idle_pump_loss,(q/150)^2*1.0e5,Pa,Flow loss through idle feed pumps on condensate pressure alone,-,tuning,assumed,,none,none,A-012,bop.rs hydraulics(),
bop.fw_low_suction_delay,3.0,s,Feed pump trip delay on low suction pressure,-,BWR-generic,assumed,,1,10,A-012,bop.rs Feedwater::step(),
bop.level_1e_ki,0.0002,1/(cm s),Single-element level controller integral gain,-,tuning,assumed,,none,none,A-012,bop.rs Feedwater::step(),
bop.level_1e_kp,0.004,1/cm,Single-element level controller proportional gain,-,tuning,assumed,,none,none,A-012,bop.rs Feedwater::step(),
bop.level_3e_level_gain,1.2,kg/s per cm,Three-element level-to-flow-demand gain,-,tuning,assumed,,none,none,A-012,bop.rs Feedwater::step(),
bop.level_3e_ki,0.3,1/s,Three-element flow-error integral gain,-,tuning,assumed,,none,none,A-012,bop.rs Feedwater::step(),
bop.level_3e_kp,0.2,-,Three-element flow-error proportional gain,-,tuning,assumed,,none,none,A-012,bop.rs Feedwater::step(),
bop.fw_heating_form,(T_FW_RATED-35)*load^0.75,K,Feedwater temperature rise from extraction heating versus turbine load,-,tuning,assumed,,none,none,A-012,bop.rs Feedwater::step(),
bop.fw_heater_out_factor,0.65,-,Fraction of heating retained with HP heaters out of service,-,BWR-generic,assumed,,0.5,0.8,A-012,bop.rs Feedwater::step(),
bop.fw_temp_lag,180,s,Feedwater temperature time constant,-,tuning,assumed,,60,600,A-012,bop.rs Feedwater::step(),
bop.fw_pump_heat,2.0,K,Temperature rise across running feed pumps,-,BWR-generic,assumed,,1,4,A-012,bop.rs Feedwater::step(),
sim.DT,0.05,s,Fixed plant time step,-,numerical,assumed,,0.0125,0.1,A-016,sim.rs,convergence study in Package 3
sim.max_steps_per_advance,4000,-,Step cap per advance call (accumulator zeroed when hit),-,numerical,assumed,,none,none,A-016,sim.rs advance(),silently drops time; Package 10
sim.speed_clamp,0.1..100,-,Simulation speed clamp,-,ui-demo,assumed,,none,none,A-019,sim.rs apply(),
sim.scram_reset_delay,10,s,Minimum time after scram before reset is accepted,-,BWR-generic,assumed,,none,none,A-014,sim.rs apply(),
sim.sri_power_threshold,0.4,rel,Power above which turbine trip / load rejection triggers Select Rod Insert,-,BWR-generic,assumed,,0.3,0.6,A-014,sim.rs protection(),
sim.setdown_cm,-100,cm,Level setpoint setdown after scram,-,BWR-generic,assumed,,none,none,A-014,sim.rs step(),
sim.fuel_damage_t,1200,degC,Average fuel temperature declared as core damage,-,tuning,assumed,,none,none,A-005,sim.rs protection(),unjustified; replaced in Package 5
sim.fuel_hot_alarm_t,1000,degC,Fuel temperature high alarm,-,tuning,assumed,,none,none,A-005,sim.rs active_alarms(),
sim.srm_cps_form,n*2.0e10+3 capped 1e6,cps,SRM count rate indication formula,-,tuning,assumed,,none,none,A-017,sim.rs snapshot(),
sim.irm_full_scale_form,0.4*10^((range-10)/2),rel,IRM full-scale power for range 1..10,-,BWR-generic,assumed,,none,none,A-017,sim.rs irm_full_scale(),legacy arrangement
sim.irm_upscale_block,108,% of range,IRM upscale rod block and alarm,-,BWR-generic,assumed,,none,none,A-014,sim.rs withdraw_block(),
sim.aprm_setdown_block,0.12,rel,APRM setdown rod block outside RUN,-,BWR-generic,assumed,,none,none,A-014,sim.rs withdraw_block(),
sim.aprm_run_block,1.08,rel,APRM upscale rod block in RUN,-,BWR-generic,assumed,,none,none,A-014,sim.rs withdraw_block(),
sim.aprm_high_alarm,1.05,rel,APRM high alarm,-,BWR-generic,assumed,,none,none,A-014,sim.rs active_alarms(),
sim.msiv_closure_scram_fraction,0.85,-,Averaged MSIV opening below which MSIV closure scram occurs in RUN,-,BWR-generic,assumed,,none,none,A-014,sim.rs scram_signals(),
sim.msivs_open_fraction,0.9,-,Averaged MSIV opening above which lines are considered open,-,tuning,assumed,,none,none,A-014,sim.rs msivs_open(),
sim.high_steam_flow_isolation,1.4,x rated,High steam flow isolation,-,BWR-generic,assumed,,none,none,A-014,sim.rs isolation_signals(),
sim.period_alarm,20,s,Short period alarm,-,BWR-generic,assumed,,none,none,A-014,sim.rs active_alarms(),
sim.heatup_alarm,55,degC/h,Heatup/cooldown rate alarm,-,BWR-generic,assumed,,none,none,A-014,sim.rs active_alarms(),
sim.p_hi_alarm,7.35e6,Pa,Reactor pressure high alarm,-,BWR-generic,assumed,,none,none,A-014,sim.rs active_alarms(),
sim.vent_open_alarm_p,0.2e6,Pa,Head vent open above this pressure alarm,-,tuning,assumed,,none,none,A-014,sim.rs active_alarms(),
sim.vacuum_low_alarm,15.0e3,Pa,Condenser vacuum low alarm,-,BWR-generic,assumed,,none,none,A-014,sim.rs active_alarms(),
sim.bypass_unavail_alarm_p,1.0e6,Pa,Reactor pressure above which bypass-unavailable warns,-,tuning,assumed,,none,none,A-014,sim.rs active_alarms(),
sim.fw_temp_low_alarm,150,degC,Feedwater temperature low alarm (above 30 % power),-,tuning,assumed,,none,none,A-014,sim.rs active_alarms(),
sim.hotwell_low_alarm,0.3,-,Hotwell level low alarm fraction,-,tuning,assumed,,none,none,A-014,sim.rs active_alarms(),
sim.ic_pool_low_alarm,50,%,IC pool level low alarm,-,tuning,assumed,,none,none,A-014,sim.rs active_alarms(),
sim.ic_in_service_fraction,0.05,-,Return valve position above which IC in service alarm shows,-,tuning,assumed,,none,none,A-014,sim.rs active_alarms(),
sim.core_uncovered_alarm,0.999,-,Core coverage below which CORE UNCOVERED alarm shows,-,tuning,assumed,,none,none,A-014,sim.rs active_alarms(),
sim.rwcu_reject_max,15,kg/s,Maximum RWCU reject flow,-,BWR-generic,assumed,,5,30,A-012,sim.rs apply(),
sim.level_setpoint_clamp,-100..50,cm,Operator level setpoint clamp,-,tuning,assumed,,none,none,A-014,sim.rs apply(),
sim.pressure_setpoint_clamp,0.2e6..7.4e6,Pa,Operator pressure setpoint clamp,-,tuning,assumed,,none,none,A-009,sim.rs apply(),
sim.speed_setpoint_clamp,0..1.02*RATED_RPM,rpm,Operator speed setpoint clamp,-,tuning,assumed,,none,none,A-010,sim.rs apply(),
sim.load_limit_clamp,0..1.1,-,Load limiter clamp,-,tuning,assumed,,none,none,A-010,sim.rs apply(),
sim.event_ttl,30,s,Lifetime of transient operator messages,-,ui-demo,assumed,,none,none,A-019,sim.rs event(),
sim.cold_shutdown_ic,40 degC;30 days;vented,-,Cold shutdown initial condition,cold,tuning,assumed,,none,none,A-018,sim.rs cold_shutdown(),cooling to 40 degC requires an unimplemented system
sim.hot_standby_ic,6.9 MPa;2 days;180 degC feed;1 feed pump,-,Hot standby initial condition,hot,tuning,assumed,,none,none,A-018,sim.rs hot_standby(),
sim.full_power_ic,rods inserted to critical;600 s settle;xenon reset;120 s settle,-,Full power initial condition construction,rated,tuning,assumed,,none,none,A-018,sim.rs full_power(),
sim.full_power_tcv_init,0.85,-,Initial TCV position at full power,rated,tuning,assumed,,none,none,A-018,sim.rs full_power(),
sim.full_power_fcv_init,0.7,-,Initial FCV position at full power,rated,tuning,assumed,,none,none,A-018,sim.rs full_power(),
sim.full_power_cond_init,5.6e3 Pa;35 degC,-,Initial condenser state at full power,rated,tuning,assumed,,none,none,A-018,sim.rs full_power(),
steam.P_ATM,101325,Pa,Standard atmospheric pressure,-,physics,confirmed,S-STD-01,,,,steam.rs,
steam.table_range,1..350,degC,Saturation table range (clamped outside),-,numerical,assumed,,none,none,A-016,steam.rs sat_t(),Package 4 declares domain
steam.sigma_form,0.2358*tau^1.256*(1-0.625*tau),N/m,Surface tension correlation,-,physics,confirmed,S-STD-05,,,,steam.rs Sat::sigma(),IAPWS 2014
```

- [ ] **Step 2: Run the register tests**

Run: `cargo test --release -p bwr-core --test register 2>&1 | tail -4`
Expected: `test result: ok. 3 passed; 0 failed`.

- [ ] **Step 3: Commit**

```bash
git add docs/model/parameters.csv
git commit -m "docs(model): register hardcoded engine values outside params.rs

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 6: Parameter register — UI constants (demo-only)

**Files:**
- Modify: `docs/model/parameters.csv` (append rows)

**Interfaces:**
- Produces: `ui-demo` rows that Package 10 cites when relabelling the mock.

- [ ] **Step 1: Append the UI rows**

```csv
ui.history.rated_mwt,870,MW,Rated thermal power used to normalise trend power percent,rated,BWRX-300,confirmed,S-06,,,,ui/src/sim/history.ts,duplicates RATED_POWER_W; Package 10 removes by sending powerPct from Rust
ui.mock.RATED_MWT,870,MW,Mock rated thermal power,-,ui-demo,assumed,,none,none,A-019,ui/src/sim/mock.ts,demo only
ui.mock.BETA,650,pcm,Mock one-group delayed fraction,-,ui-demo,assumed,,none,none,A-019,ui/src/sim/mock.ts,demo only
ui.mock.LAMBDA,0.0767,1/s,Mock one-group precursor decay constant,-,ui-demo,assumed,,none,none,A-019,ui/src/sim/mock.ts,demo only
ui.mock.SOURCE_PCM,1.2e-5,-,Mock source term,-,ui-demo,assumed,,none,none,A-019,ui/src/sim/mock.ts,demo only
ui.mock.NWL_ABOVE_TAF_M,9.5,m,Mock normal water level above TAF,-,ui-demo,assumed,,none,none,A-019,ui/src/sim/mock.ts,demo only
ui.mock.ROD_SPEED,0.667,notch units/s,Mock rod speed,-,ui-demo,assumed,,none,none,A-019,ui/src/sim/mock.ts,demo only
ui.mock.SCRAM_SPEED,16,units/s,Mock scram speed,-,ui-demo,assumed,,none,none,A-019,ui/src/sim/mock.ts,demo only
ui.mock.FEED_PUMP_CAP_KGS,420,kg/s,Mock feed pump capacity,-,ui-demo,assumed,,none,none,A-019,ui/src/sim/mock.ts,demo only
ui.mock.STEAM_K_TCV,72,kg/s per MPa,Mock TCV coefficient,-,ui-demo,assumed,,none,none,A-019,ui/src/sim/mock.ts,demo only
ui.mock.STEAM_K_BYP,36,kg/s per MPa,Mock bypass coefficient,-,ui-demo,assumed,,none,none,A-019,ui/src/sim/mock.ts,demo only
ui.mock.IC_MW,33,MW,Mock IC train duty,-,ui-demo,assumed,,none,none,A-019,ui/src/sim/mock.ts,demo only
ui.mock.XE_BURN,2,-,Mock xenon burnout factor,-,ui-demo,assumed,,none,none,A-019,ui/src/sim/mock.ts,demo only
ui.mock.tsat_form,179.9*p^0.24,degC,Mock saturation temperature fit,-,ui-demo,assumed,,none,none,A-019,ui/src/sim/mock.ts,demo only
ui.history.TREND_WINDOW_S,600,s,Trend window,-,ui-demo,assumed,,none,none,A-019,ui/src/sim/history.ts,
ui.history.MIN_SAMPLE_DT,1,s,Trend sampling interval (decimation),-,ui-demo,assumed,,none,none,A-019,ui/src/sim/history.ts,can hide sub-second excursions; Package 10
ui.topbar.SPEEDS,1;2;5;10;30,x,Available simulation speeds,-,ui-demo,assumed,,none,none,A-019,ui/src/components/TopBar.tsx,
tauri.tick_ms,20,ms,Background physics loop sleep,-,numerical,assumed,,none,none,A-016,src-tauri/src/lib.rs,
tauri.emit_ms,100,ms,Snapshot emit interval,-,numerical,assumed,,none,none,A-016,src-tauri/src/lib.rs,
tauri.max_real_dt,0.5,s,Cap on wall-clock dt per loop iteration,-,numerical,assumed,,none,none,A-016,src-tauri/src/lib.rs,
```

- [ ] **Step 2: Run the register tests and count rows**

Run: `cargo test --release -p bwr-core --test register 2>&1 | tail -4 && wc -l docs/model/parameters.csv`
Expected: `3 passed`; line count of at least `260` (header + 111 + 128 + 20).

- [ ] **Step 3: Commit**

```bash
git add docs/model/parameters.csv
git commit -m "docs(model): register UI and shell constants as demo-only

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 7: Assumptions register

**Files:**
- Create: `docs/model/assumptions.md`

**Interfaces:**
- Produces: `A-001`..`A-019`, referenced by `parameters.csv` and by code comments in later packages (`// A-007`).

- [ ] **Step 1: Write the file**

```markdown
# Modelling Assumptions

Rule: every `assumed` or `esbwr-estimate` row in `parameters.csv` cites one of these ids. When a package changes or removes a value, update the entry here in the same commit. Code comments reference an assumption as `// A-007` next to the value or logic. Ids are never reused; a retired assumption keeps its number and gets a `Status: retired (Package N)` line.

| Id | Title | Statement | Sensitivity treatment | Resolved by |
|---|---|---|---|---|
| A-001 | Rod geometry and drive kinematics | 57 FMCRDs on a 9x9 grid within radius sqrt(17); position 0..48 with 2-position notches; 0.4 position/s motor speed; 3 s hydraulic scram; active length 3.81 m. | Scram time 2..5 s; speed 0.2..0.8; length 3.6..3.9 m. | Package 5 |
| A-002 | Point-kinetics data | Six-group U-235 thermal data (Keepin); beta 0.006; generation time 40 us; fixed source 8e-6. Exposure dependence of beta ignored. | beta 0.005..0.007; generation time 20..60 us. | Package 5 |
| A-003 | Reactivity coefficients and rod worth | Cold excess 0.130 and total rod worth 0.200 are tuned for HFP criticality with about 12 % rod worth inserted; Doppler, void, moderator, xenon coefficients are BWR-generic; rod worth shape is J0^2 radial and cosine axial; SRI group is four symmetric rods. | Intervals in parameters.csv; sensitivity study in Package 5. | Package 5 |
| A-004 | Xenon data and flux | Standard I/Xe constants; sigma_a*phi = 7.95e-5 1/s at rated flux. | Flux 5e-5..1.2e-4 1/s. | Package 5 |
| A-005 | Decay heat and fuel thermal model | Nine-group fit to 0.066 t^-0.2 for infinite operation; lumped fuel node; 264 K average fuel-to-coolant rise; 2.5 % direct coolant heating; 1200 degC average fuel declared "core damage" without basis. | Refit to a transcribed public table with uncertainty band; separate clad node. | Package 5 |
| A-006 | Vessel geometry, elevations, metal mass | All internal elevations, areas, and volumes beyond the published 4 m x 26.7 m envelope are engineering estimates; metal heat capacity 2.75e8 J/K; hot heat loss 1 MW. | Intervals in parameters.csv; level and pressure sensitivity in Package 4. | Package 4 |
| A-007 | Natural circulation model | Rated flow 2000 kg/s scaled from ESBWR per MWt (violates the no-power-scaling rule and must be re-derived); friction calibrated at the rated point; drift-flux C0 1.13 with churn-turbulent vgj; homogeneous-like two-phase multiplier; loop head cut off over 0.5 m below chimney top. | Re-derive from geometry and head balance; flow 1500..2500 kg/s. | Package 4 |
| A-008 | Main steam system | Two steam lines; 215 degC feedwater; 0.2 MPa line drop; 4 s MSIV stroke; 100 % bypass. | Bypass 30..100 %; feed temp 200..230 degC. | Package 6 |
| A-009 | Pressure control | Proportional regulator with 0.25 MPa span; 6.95 MPa default setpoint; valve rates as registered. | Controller retune only after physics corrections. | Package 6 |
| A-010 | Turbine-generator | 1800 rpm 4-pole machine; effective enthalpy drop calibrated to 300 MWe net; inertia H about 4 s; fixed loss model; governor gains tuned. | Intervals in parameters.csv. | Package 6 |
| A-011 | Condenser and circulating water | CW inlet 20 degC is an external boundary; two CW pumps at 7500 kg/s; UA calibrated to 5.6 kPa; air model at fixed 303 K. | CW inlet 5..30 degC. | Package 6 |
| A-012 | Condensate and feedwater | 2 condensate + 3 x 50 % feed pumps; pump curve form; equal-percentage FCV; controller gains tuned; extraction heating fit; hotwell makeup/reject unaccounted. | Explicit CST boundary in Package 6. | Package 6 |
| A-013 | Isolation condensers | 3 trains at 33.75 MWt each (ESBWR unit size); 1.5e6 kg pool per train; single lumped valve; 20 s tube lag; idle pool cooling without accounting. | Duty 20..45 MWt; pool mass re-derived from the 7-day claim. | Package 7 |
| A-014 | Protection setpoints and alarms | All scram, isolation, trip, block, and alarm setpoints are BWR-generic; level setpoints L1..L8 relative to NWL; legacy SRM/IRM/APRM arrangement. | Cause-and-effect matrix with provenance. | Package 8 |
| A-015 | House loads | 4 MW base plus per-pump loads. | Intervals in parameters.csv. | Package 6 |
| A-016 | Numerical choices | 0.05 s plant step; 1 ms kinetics substep; 12 axial nodes; void lags 0.5/2 s; 1000 kg inventory floor; EOS bisection 1..349.5 degC; 4000-step advance cap. | Convergence at 0.05/0.025/0.0125 s; floors replaced by domain errors. | Package 3; 10 |
| A-017 | Instrument indication models | SRM cps = n*2e10+3; IRM full scale 0.4*10^((range-10)/2); APRM = n; period meter 1 s filter; heatup-rate 30 s filter; no sensor lag, range, or failure. | Sensor models in Package 8. | Package 8 |
| A-018 | Scenario initial conditions | Cold shutdown: 30 days decay; 40 degC; vented (reaching 40 degC needs a system not modelled). Hot standby: 2 days decay; 6.9 MPa; one feed pump. Full power: rods inserted to criticality then settled; xenon reset to equilibrium. | Verified in Package 9. | Package 9 |
| A-019 | UI demonstration mock | Browser mock physics are not the simulator; constants are demo-only and never cited as plant behaviour. | None; mock is relabelled in Package 10. | Package 10 |
```

- [ ] **Step 2: Verify every assumption id used in parameters.csv exists here**

Run: `awk -F',' 'NR>1 && $11!="" {print $11}' docs/model/parameters.csv | sort -u | while read id; do grep -q "| $id |" docs/model/assumptions.md || echo "missing $id"; done`
Expected: no output.

- [ ] **Step 3: Commit**

```bash
git add docs/model/assumptions.md
git commit -m "docs(model): add numbered assumptions register A-001..A-019

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 8: ESBWR applicability register

**Files:**
- Create: `docs/model/esbwr-applicability.csv`

**Interfaces:**
- Produces: ids `E-01`..`E-06` that Packages 4 to 9 update (classification changes from `pending`) before borrowing any ESBWR concept.

- [ ] **Step 1: Write the header and six seeded rows**

```csv
id,esbwr_source,esbwr_section,target_subsystem,bwrx_evidence,differences,adaptation,applicability_limits,test,classification
E-01,DCD Rev 10 Tier 2;NUREG-1966 V2,Ch 4 (4.3 nuclear design; 4.4 thermal-hydraulic; 4.6 FMCRD),Core fuel and rods,S-06;S-08,Fuel geometry; core size (240 vs 1132 bundles); exposure; FMCRD count and arrangement; insertion response,Borrow feedback mechanisms and kinetics methods and heat-transfer forms and shutdown-margin methodology only; re-derive all numbers for 240-bundle core,No ESBWR worth or coefficient value is a BWRX value,core_transients.rs (Package 5),pending
E-02,DCD Rev 10 Tier 2;NUREG-1966 V2,Ch 4.4 and Ch 5 (RPV and internals; chimney),Vessel and circulation,S-06,Elevations; areas; inventory; flow paths; pressure losses; chimney partitioning,Borrow inventory/energy balance forms and boiling and separator and natural-circulation head/loss formulation; use BWRX geometry,Correlation validity must be checked at BWRX flows and voids,vessel_transients.rs (Package 4),pending
E-03,DCD Rev 10 Tier 2;NUREG-1966 V2,Ch 5.4.6 (ICS) and Ch 6.2 (containment interfaces),Isolation condensers,S-06;S-09;S-10,BWRX connections; train count (3 vs 4); duties; initiation logic; pool inventories; no SRVs,Borrow condensation and gravity return and pool boiloff and gas accumulation and valve/failure concepts; BWRX duty and pool from BWRX claims,S-09 notes IC function and connections differ,ic_transients.rs (Package 7),pending
E-04,DCD Rev 10 Tier 2;NUREG-1966 V2,Ch 10 (steam and power conversion) and Ch 7.7 (pressure and level control),Steam and power conversion,S-06,Actual equipment; capacities; control responses; operating points,Borrow pressure regulation and bypass and turbine/condenser thermodynamics and feedwater heating and level control principles,Equipment numbers are BWRX-specific or assumed,power_conversion.rs (Package 6),pending
E-05,DCD Rev 10 Tier 2;NUREG-1966 V2,Ch 7 (I&C; RPS; NMS) and Ch 16 (technical specifications),Protection and instruments,S-06;S-07;S-08,Applicable BWRX signals; architecture; modes; setpoints; actuation sequence; RPV isolation valves replace SRVs,Borrow measurement principles and permissives and latches and redundancy concepts and response testing; no voting architecture claim without BWRX evidence,Do not assume legacy SRM/IRM/APRM or ESBWR SRNM/PRNM is the final BWRX arrangement,protection.rs (Package 8),pending
E-06,DCD Rev 10 Tier 2;NUREG-1966 V2,Ch 14 (initial test program) and Ch 15 (transient and accident analyses),Validation,S-10;S-11;S-12,Reference configuration; initial conditions; credited systems; numerical uncertainty,Borrow component qualification methods and test structure and transient mechanisms; ESBWR traces are mechanism tests not BWRX acceptance curves,No ESBWR numerical trace is a BWRX acceptance curve,scenarios.rs (Package 9),pending
```

- [ ] **Step 2: Verify shape**

Run: `awk -F',' 'NF!=10{print NR": "NF" fields"}' docs/model/esbwr-applicability.csv`
Expected: no output.

- [ ] **Step 3: Record the reading plan in design-basis.md**

Append to `docs/model/design-basis.md`:

```markdown
## 7. ESBWR reading plan

Before any package borrows an ESBWR concept, the executor reads the DCD Rev. 10 Tier 2 chapter(s) listed for that row in `esbwr-applicability.csv`, reads the referenced tables and figures (not only summaries), fills `bwrx_evidence`, `differences`, `adaptation`, and `applicability_limits` with specific section numbers, and changes `classification` from `pending` to `shared-concept`, `adapted-model`, `provisional-estimate`, or `inapplicable`. Priority order: Ch 4, 5, 6, 7, 9, 10, 14, 15, 16. Each chapter PDF gets its own `sources.csv` row (`S-ESBWR-<chapter>`) with checksum.
```

- [ ] **Step 4: Commit**

```bash
git add docs/model/esbwr-applicability.csv docs/model/design-basis.md
git commit -m "docs(model): seed ESBWR applicability register

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 9: Coverage map

**Files:**
- Create: `docs/model/coverage.md`

**Interfaces:**
- Consumes: test names from `crates/bwr-core/tests/plant.rs` and unit tests in `src/*.rs`.
- Produces: the table each later package updates when it adds tests.

- [ ] **Step 1: Write the file**

```markdown
# Evidence and Test Coverage by Subsystem

Updated by every package. A subsystem row is "covered" when it has at least one source or assumption for every consequential value and at least one test per behaviour listed in the spec.

| Subsystem | Engine files | Source rows | Parameter rows (prefix) | Assumptions | Tests at baseline | Gaps at baseline |
|---|---|---|---|---|---|---|
| Core kinetics | kinetics.rs | S-STD-02 | BETA; BETA_FRAC; LAMBDA_DN; GEN_TIME; SOURCE; kinetics.* | A-002; A-016 | kinetics::steady_state_stays_put; kinetics::small_step_gives_textbook_period; kinetics::prompt_drop_on_scram | nonpositive denominator untested; no convergence test |
| Reactivity and rods | reactor.rs | S-06; S-08 | N_RODS; ROD_*; RHO_EXCESS_COLD; ROD_WORTH_TOTAL; DOPPLER_A; VOID_COEF; MOD_TEMP_COEF; reactor.* | A-001; A-003 | reactor::layout_has_57_rods; reactor::rod_worths; plant::cold_shutdown_is_subcritical_and_rods_blocked | no shutdown-margin test; no rod-mechanism distinction; rod reactivity lag |
| Xenon | reactor.rs | S-STD-03 | LAMBDA_I; LAMBDA_XE; GAMMA_*; SIGMA_PHI_XE; XENON_WORTH_EQ | A-004 | reactor::xenon_peaks_after_shutdown | no restart or power-history test |
| Decay heat and fuel | reactor.rs | S-STD-04 | DECAY_GROUPS; DIRECT_COOLANT_HEAT; FUEL_HEAT_CAP; FUEL_RESISTANCE; sim.fuel_* | A-005 | reactor::decay_heat_curve | infinite-operation fit only; no clad node; unjustified 1200 degC rule |
| Vessel inventory and EOS | vessel.rs; steam.rs; steam_table.rs | S-06; S-STD-05 | P_RATED; VESSEL_*; Z_*; A_*; V_*; METAL_HEAT_CAP; HEAT_LOSS_HOT; vessel.*; steam.* | A-006; A-016 | vessel::eos_roundtrip; vessel::level_volume_inverse; steam::known_points | 100 degC atmospheric assumption; 1000 kg floor; collapsed vs mixture level; no property reference test beyond 2 points |
| Natural circulation | vessel.rs | S-04 (ESBWR) | CORE_FLOW_RATED; LOOP_INERTIA; C0; N_AXIAL; vessel.drift_*; vessel.phi2_form; vessel.spill_band | A-007 | vessel::rated_flow_and_voids_are_plausible | calibrated at one point only; spill cutoff unverified |
| Main steam and pressure control | bop.rs | - | STEAM_*; N_STEAM_LINES; MSIV_STROKE; C_TCV; BYPASS_CAPACITY; C_BYPASS; PRESSURE_*; bop.tcv_*; bop.bypass_*; bop.p_ref_* | A-008; A-009 | plant::raising_pressure_raises_power; plant::msiv_closure_scrams_and_isolation_condensers_cool | averaged MSIV positions; no TSV; no line storage |
| Turbine-generator | bop.rs | - | TURBINE_DH_RATED; GENERATOR_EFF; RATED_RPM; GRID_HZ; TG_*; OVERSPEED_TRIP_RPM; SPEED_RAMP; bop.governor_*; bop.sync_*; bop.turbine_* | A-010 | plant::turbine_trip_rides_through_on_bypass; plant::startup_from_hot_standby_to_30_percent | no energy balance reconciliation; extraction not debited from shaft |
| Condenser | bop.rs | - | CW_*; N_CW_PUMPS; CONDENSER_*; AIR_INLEAKAGE; VACUUM_PUMP_TAU; HOTWELL_CAPACITY; bop.condenser_*; bop.hotwell_* | A-011; A-012 | none directly | unaccounted makeup/reject; no vacuum-loss test |
| Feedwater | bop.rs | - | N_FEED_PUMPS; N_COND_PUMPS; FW_*; CONDENSATE_PRESSURE; C_FCV; bop.fcv_*; bop.fw_*; bop.level_* | A-012 | plant::full_power_is_steady (indirect) | no pump-trip or controller-transfer test |
| Isolation condensers | ic.rs | S-04 (ESBWR); S-10 | N_IC_TRAINS; IC_*; ic.* | A-013 | plant::msiv_closure_scrams_and_isolation_condensers_cool | energy overshoot and depletion discard; single valve; no train-availability tests; no 72 h / 7 d run |
| Protection and modes | sim.rs | - | SCRAM_HIGH_PRESSURE; IC_HIGH_PRESSURE; VESSEL_DESIGN_PRESSURE; APRM_*; IRM_HIGH_HIGH; MSIV_LOW_PRESSURE_RUN; L*_CM; sim.* | A-014; A-017 | plant::manual_scram_shuts_down_without_isolation; plant::cold_shutdown_is_subcritical_and_rods_blocked | REFUEL interlock uses actual positions only; L8 feed-pump trip inside turbine-not-tripped branch; no cause-and-effect matrix |
| Initial conditions | sim.rs | - | sim.*_ic | A-018 | plant::full_power_is_steady; plant::json_contract_matches_ui | cold shutdown relies on an unmodelled cooling system |
| Runtime and UI contract | sim.rs; src-tauri/src/lib.rs; ui/src/sim/*.ts | - | sim.max_steps_per_advance; tauri.*; ui.* | A-016; A-019 | plant::json_contract_matches_ui | partial key check only; dropped time; trend decimation |
```

- [ ] **Step 2: Verify every test name listed exists**

Run: `for t in steady_state_stays_put small_step_gives_textbook_period prompt_drop_on_scram layout_has_57_rods rod_worths xenon_peaks_after_shutdown decay_heat_curve eos_roundtrip level_volume_inverse rated_flow_and_voids_are_plausible known_points full_power_is_steady manual_scram_shuts_down_without_isolation turbine_trip_rides_through_on_bypass msiv_closure_scrams_and_isolation_condensers_cool raising_pressure_raises_power startup_from_hot_standby_to_30_percent json_contract_matches_ui cold_shutdown_is_subcritical_and_rods_blocked; do grep -rq "fn $t" crates/bwr-core || echo "missing $t"; done`
Expected: no output.

- [ ] **Step 3: Commit**

```bash
git add docs/model/coverage.md
git commit -m "docs(model): add subsystem evidence and test coverage map

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 10: Link the register from the README and verify the full suite

**Files:**
- Modify: `README.md` (replace the "Data sources" section)

**Interfaces:**
- Produces: the public entry point to the register.

- [ ] **Step 1: Replace the "Data sources" section**

In `README.md`, replace the block from `## Data sources` up to (not including) `## License` with:

```markdown
## Model evidence

Every plant parameter and hardcoded modelling value is registered under `docs/model/`:

- `design-basis.md`: reference configuration (North American, 60 Hz), evidence precedence, and the conflict-resolution rule.
- `sources.csv`: every public document used, with revision, section, retrieval date, and SHA-256 checksum.
- `parameters.csv`: every constant with unit, definition, reference state, source design, status (`confirmed`, `derived`, `calibrated`, `esbwr-estimate`, `assumed`, `unavailable`), sensitivity interval, and location. `crates/bwr-core/tests/register.rs` fails if a `pub const` in `params.rs` has no row.
- `assumptions.md`: numbered assumptions `A-nnn` cited by the register and by code comments.
- `esbwr-applicability.csv`: each ESBWR concept borrowed, with the BWRX evidence, differences, adaptation, and limits.
- `coverage.md`: which sources, parameters, assumptions, and tests cover each subsystem.

Confirmed public figures: 870 MWt, about 300 MWe, 7.2 MPa, 240 GNF2 bundles, 57 fine-motion control rods, a vessel about 4 m wide and 26 to 27 m tall, natural circulation with a tall chimney, and passive isolation condensers with a published 7-day cooling claim. Everything else is marked `assumed`, `calibrated`, or `esbwr-estimate` in the register.

This is a game, not an engineering tool. It makes no claim of exact design reproduction, regulatory approval, or validation against plant data. It is not affiliated with GE Vernova, Hitachi or the makers of Nucleares.
```

- [ ] **Step 2: Run the full engine suite and UI typecheck**

Run: `cargo test --release -p bwr-core 2>&1 | grep -E "test result|FAILED"`
Expected: four `test result: ok.` lines (unit, plant, register, doc-tests) and no `FAILED`.

Run: `npm --prefix ui run typecheck`
Expected: exits 0 with no output (no UI code changed in this package).

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "docs: link the model evidence register from the README

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

## Acceptance gate (verbatim from the spec)

"A reviewer can trace every consequential behavior to evidence or an explicit assumption. Public descriptions, accepted methods, and confirmed numerical plant data are distinguished."

Checklist for the reviewer:

- [ ] `cargo test --release -p bwr-core --test register` passes.
- [ ] No `UNRETRIEVED` remains in a non-index row of `sources.csv`; every PDF row has a 64-character `sha256`.
- [ ] Every `assumed`/`esbwr-estimate` row in `parameters.csv` cites an `A-nnn` that exists in `assumptions.md`.
- [ ] No row was promoted to `confirmed` without a `source_ids` entry whose `sources.csv` row has a section and checksum.
- [ ] `esbwr-applicability.csv` rows remain `pending` unless the named DCD chapter was read and its `S-ESBWR-*` source row exists.
