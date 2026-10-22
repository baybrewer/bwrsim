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
- Do not promote any value to `confirmed` in this package beyond what `README.md` and `params.rs` already mark CONFIRMED. Promotion happens only when a `sources.csv` row with a real checksum and page/section exists. The reverse is required: a `confirmed` status that the retrieved evidence does not support is downgraded with a note (Task 4 Step 1b).
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

These headers are quoted verbatim in the master plan section "Locked CSV headers". Every reader (tests, later packages) looks columns up **by header name**, never by position, so a later package may append a column at the end without breaking anything. A later package may never rename, reorder, or remove one of these columns.

`sources.csv` (12 columns; first column `id` as the master requires):

```
id,document_id,title,revision,date,jurisdiction,url,section,retrieval_date,sha256,kind,notes
```

`kind` is one of `vendor-proposal`, `regulator-assessment`, `accepted-methodology`, `standard`, `textbook`, `index-page`.

`parameters.csv` (13 columns):

```
id,value,unit,definition,reference_state,source_design,status,source_ids,interval_lo,interval_hi,assumption_id,location,notes
```

Column meanings: `id` is the `pub const` name for `params.rs` constants, or `<file>.<name>` for hardcoded values elsewhere; `reference_state` is the plant state the value refers to (`rated`, `cold`, `HFP`, `-`); `source_design` is `BWRX-300`, `ESBWR`, `BWR-generic`, `physics`, `tuning`, `numerical`, `shell` (Tauri shell timing), or `ui-demo`; `interval_lo`/`interval_hi` give the sensitivity interval or `none` when the spec's "no defensible interval" case applies; `assumption_id` is required for `assumed` and `esbwr-estimate` rows.

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
- Produces: `id` values `S-01`..`S-13`, `S-05S`, `S-STD-01`..`S-STD-06`, `S-UK-01`, and the per-chapter ids `S-CRN-<ch>`, `S-ESBWR-<ch>`, `S-UK-<ch>` referenced by `parameters.csv` and `esbwr-applicability.csv`.

- [ ] **Step 1: Write the header and seed rows**

Write `docs/model/sources.csv` with exactly this content (one line per row; `UNRETRIEVED` placeholders are replaced in Steps 3 to 4b):

```csv
id,document_id,title,revision,date,jurisdiction,url,section,retrieval_date,sha256,kind,notes
S-01,NRC-BWRX-INDEX,NRC BWRX-300 pre-application activities and topical report index,-,-,US,https://www.nrc.gov/facilities-safety/new-reactors/advanced-reactors/who-were-working-with/pre-application-activities/gvh-bwrx-300,-,UNRETRIEVED,UNRETRIEVED,index-page,entry point only; cite the documents it links
S-02,NRC-CRN-INDEX,NRC Clinch River construction permit application and PSAR index,-,-,US,https://www.nrc.gov/facilities-safety/new-reactors/advanced-reactors/who-were-working-with/advanced-reactor-application-projects/tva-clinch-river-cpa,-,UNRETRIEVED,UNRETRIEVED,index-page,PSAR chapters are the primary US plant-level source
S-03,GEV-GDA-LIB,GE Vernova BWRX-300 UK GDA document library,-,-,UK,https://www.gevernova.com/nuclear/uk-smr/documents-list,-,UNRETRIEVED,UNRETRIEVED,index-page,supplementary jurisdiction; check configuration differences before use
S-04,NRC-ESBWR-INDEX,NRC ESBWR design certification index (DCD Rev. 10 and safety evaluation),-,-,US,https://www.nrc.gov/facilities-safety/new-reactors/large-light-water-reactors/design-certification-applications-for-new-reactors/esbwr,-,UNRETRIEVED,UNRETRIEVED,index-page,locate DCD Tier 2 chapters 4 5 6 7 9 10 14 15 16
S-05,NUREG-1966-V2,Final Safety Evaluation Report related to the certification of the ESBWR Volume 2 Chapters 4 through 8,-,-,US,https://www.nrc.gov/regulations-legislation/nureg-series-publications/publications-prepared-by-nrc-staff/final-safety-evaluation-report-related-to-the-certification-of-the-economic-simplified-boiling-water/v2,-,UNRETRIEVED,UNRETRIEVED,regulator-assessment,volume 2 PDF
S-05S,NUREG-1966-S1,Final Safety Evaluation Report related to the certification of the ESBWR Supplement 1,-,-,US,https://www.nrc.gov/facilities-safety/new-reactors/large-light-water-reactors/design-certification-applications-for-new-reactors/esbwr,-,UNRETRIEVED,UNRETRIEVED,regulator-assessment,locate the supplement PDF via S-04 and replace this URL with the direct link
S-06,005N9751,BWRX-300 General Description,UNRETRIEVED,UNRETRIEVED,US,https://www.gevernova.com/content/dam/gevernova-nuclear/global/en_us/documents/carbon-free-power/005N9751_Rev_BWRX-300_General_Description.pdf,-,UNRETRIEVED,UNRETRIEVED,vendor-proposal,source of the baseline CONFIRMED values; includes comparison with ESBWR
S-07,NEDO-33910-A,BWRX-300 Reactor Pressure Vessel Isolation and Overpressure Protection,Rev. 2,UNRETRIEVED,US,https://www.nrc.gov/docs/ML2316/ML23167A086.pdf,-,UNRETRIEVED,UNRETRIEVED,accepted-methodology,integral isolation valves; no SRVs
S-08,NEDO-33912-A,BWRX-300 Reactivity Control,Rev. 2,UNRETRIEVED,US,https://www.nrc.gov/docs/ML2304/ML23048A018.pdf,-,UNRETRIEVED,UNRETRIEVED,accepted-methodology,FMCRD arrangement; shutdown margin methodology
S-09,NRC-CNSC-CONTAINMENT,NRC/CNSC joint report on BWRX-300 containment evaluation method,-,UNRETRIEVED,US;CA,https://www.nrc.gov/docs/ML2209/ML22091A201.pdf,-,UNRETRIEVED,UNRETRIEVED,regulator-assessment,notes IC function and reactor connections differ from ESBWR
S-10,NEDO-33934,BWRX-300 Safety Strategy,Rev. 2,UNRETRIEVED,US,https://www.nrc.gov/docs/ML2607/ML26077A383.pdf,-,UNRETRIEVED,UNRETRIEVED,vendor-proposal,ICS mission time and credited systems
S-11,NEDO-34270-A,BWRX-300 Stability Analysis,Rev. 2,UNRETRIEVED,US,https://www.nrc.gov/docs/ML2615/ML26159A164.pdf,-,UNRETRIEVED,UNRETRIEVED,accepted-methodology,natural-circulation stability claims
S-12,NRC-STABILITY-SE,NRC safety evaluation of BWRX-300 stability analysis,-,UNRETRIEVED,US,https://www.nrc.gov/docs/ML2607/ML26078A299.pdf,-,UNRETRIEVED,UNRETRIEVED,regulator-assessment,
S-13,NUREG-1434-V1,Standard Technical Specifications General Electric BWR/6 Plants Volume 1 Specifications,UNRETRIEVED,UNRETRIEVED,US,https://www.nrc.gov/reading-rm/doc-collections/nuregs/staff/sr1434/,UNRETRIEVED,UNRETRIEVED,UNRETRIEVED,regulator-assessment,legacy BWR/6 allowable values used only as typical-BWR interval bounds (Task 7); never a BWRX value; replace URL with the direct Volume 1 PDF link in Step 4a
S-UK-01,GDA-CH4,BWRX-300 UK Generic Design Assessment Chapter 4 (Reactor),UNRETRIEVED,UNRETRIEVED,UK,https://www.gevernova.com/nuclear/uk-smr/documents-list,-,UNRETRIEVED,UNRETRIEVED,vendor-proposal,README cites this; locate exact PDF via S-03
S-STD-01,CODATA-2018,CODATA recommended values of the fundamental physical constants 2018,2018,2019,-,https://physics.nist.gov/cuu/Constants/,g_n,UNRETRIEVED,UNRETRIEVED,standard,standard gravity 9.80665 m/s2; engine uses 9.81
S-STD-02,KEEPIN-1965,Physics of Nuclear Kinetics (Keepin),1965,1965,-,-,Table 4-7 U-235 thermal,UNRETRIEVED,none,textbook,six-group delayed neutron data
S-STD-03,ENDF-B-VIII.0-XE,ENDF/B-VIII.0 decay data and U-235 thermal fission product yields for I-135 and Xe-135 (via NNDC),VIII.0,2018,-,https://www.nndc.bnl.gov/endf-b8.0/,UNRETRIEVED,UNRETRIEVED,UNRETRIEVED,standard,resolved in Step 4a; Lamarsh and Baratta 3rd ed. is the fallback print citation
S-STD-04,TODREAS-KAZIMI,Nuclear Systems Volume 1 (Todreas and Kazimi) decay heat approximation 0.066 t^-0.2,2nd ed.,2012,-,-,Chapter 3,UNRETRIEVED,none,textbook,current decay-heat fit target; replaced in Package 5 by a transcribed reference table
S-STD-05,IAPWS-IF97,IAPWS Industrial Formulation 1997 for the thermodynamic properties of water and steam (via CoolProp),2007,2007,-,http://www.iapws.org/relguide/IF97-Rev.html,UNRETRIEVED,UNRETRIEVED,UNRETRIEVED,standard,steam table generator tools/gen_steam_table.py; resolved in Step 4a
S-STD-06,IAPWS-R1-76,IAPWS Revised Release on Surface Tension of Ordinary Water Substance (R1-76 2014),2014,2014,-,http://www.iapws.org/relguide/Surf-H2O.html,UNRETRIEVED,UNRETRIEVED,UNRETRIEVED,standard,surface tension correlation sigma = B*tau^mu*(1+b*tau) used by steam.rs Sat::sigma(); resolved in Step 4a
```

- [ ] **Step 2: Verify shape**

Run: `awk -F',' 'NF!=12{print NR": "NF" fields"}' docs/model/sources.csv`
Expected: no output (every line has 12 fields).

- [ ] **Step 3: Add one row per chapter-level document located through the index pages**

The spec asks for the latest applicable revisions of the Clinch River PSAR chapters, the GE UK GDA chapters, and the ESBWR DCD Tier 2 chapters. Index pages are not citable, so add one row per chapter PDF, with these fixed ids and the chapter list fixed here:

| Id pattern | Located via | Chapters to add (one row each) | `kind` | `jurisdiction` |
|---|---|---|---|---|
| `S-CRN-04` .. `S-CRN-15` | S-02 | PSAR chapters 4 (reactor), 5 (RCS and connected systems), 6 (ESF), 7 (I&C), 9 (auxiliaries), 10 (steam and power conversion), 15 (transient analyses) | `vendor-proposal` | `US` |
| `S-ESBWR-04` .. `S-ESBWR-16` | S-04 | DCD Rev. 10 Tier 2 chapters 4, 5, 6, 7, 9, 10, 14, 15, 16 | `vendor-proposal` (DCD) | `US` |
| `S-UK-04` .. `S-UK-15` | S-03 | GDA chapters matching the PSAR list above where they exist; replace the seeded `S-UK-01` row with `S-UK-04` once the exact PDF is located | `vendor-proposal` | `UK` |

Each new row uses the direct PDF URL (not the index URL), the revision and date printed on the cover, and gets its checksum in Step 4. A chapter that does not exist publicly gets `url` = `not-public`, `sha256` = `none`, and `notes` = `not available publicly on <date>`; it is never left out silently and is never cited in a `parameters.csv` `source_ids` field.

- [ ] **Step 4: Retrieve each PDF source and fill revision, date, retrieval date, and checksum**

For each of S-05, S-05S, S-06, S-07, S-08, S-09, S-10, S-11, S-12, S-13 and every `S-CRN-*`, `S-ESBWR-*`, `S-UK-*` row added in Step 3 (the standard and textbook rows follow in Step 4a):

```bash
mkdir -p ~/ai/bwrsim-sources
curl -L -o ~/ai/bwrsim-sources/005N9751.pdf "https://www.gevernova.com/content/dam/gevernova-nuclear/global/en_us/documents/carbon-free-power/005N9751_Rev_BWRX-300_General_Description.pdf"
shasum -a 256 ~/ai/bwrsim-sources/005N9751.pdf
date +%Y-%m-%d
```

Expected: a 64-hex-character digest followed by the path; today's date. Copy the digest into the `sha256` field, today's date into `retrieval_date`, and the revision and issue date printed on the document's cover into `revision` and `date`. Repeat per document. Source PDFs stay outside the repository (`~/ai/bwrsim-sources/` is not tracked); only the checksum is recorded. Index pages (S-01..S-04) keep `sha256` = `none` and get only `retrieval_date`.

- [ ] **Step 4a: Resolve the standard and textbook rows S-STD-01..S-STD-06 and S-13**

Completion rules (exactly one applies to every non-index row; a row that satisfies none of them is incomplete):

- *Retrieved file* (PDF or data file): `sha256` = digest of the retrieved file; `retrieval_date` = download date; `revision` and `date` as printed in the document; `section` = the printed section, table, or equation that states the cited value (never `-`).
- *Verified print citation* (no public file): `url` = `print`; `sha256` = `none`; `revision` = edition; `section` = chapter, page, and table or equation number read from a physical or library copy; `retrieval_date` = the date the executor read that page; `notes` begins `ISBN <isbn>` (or `pre-ISBN` for books printed before 1970). A print citation whose page the executor has not seen is not verified; use the alternative named for that row instead.
- *Not public*: `url` = `not-public`; `sha256` = `none`; `section` = `-`; `retrieval_date` = the date searched; `notes` = `not available publicly on <date>`. A `not-public` row is never cited in `parameters.csv` `source_ids`.

Row by row:

- S-STD-01 (CODATA 2018): `curl -L -o ~/ai/bwrsim-sources/codata2018_allascii.txt https://physics.nist.gov/cuu/Constants/Table/allascii.txt`, then `grep -E "standard acceleration of gravity|standard atmosphere|molar gas constant" ~/ai/bwrsim-sources/codata2018_allascii.txt` must print `9.806 65`, `101 325`, and `8.314 462 618`. Retrieved-file rule (checksum the text file); `url` becomes the allascii.txt link; `section` = `rows: standard acceleration of gravity; standard atmosphere; molar gas constant`.
- S-STD-02 (Keepin 1965): verified print citation of the U-235 thermal six-group table (Addison-Wesley 1965; `pre-ISBN`); record chapter, page, and table number from the copy. If no copy can be read, replace the row (same id) with the ENDF/B-VIII.0 six-group delayed-neutron data for U-235 thermal fission retrieved from the same NNDC archive as S-STD-03 (title `ENDF/B-VIII.0 U-235 delayed neutron group data (MF=1 MT=455 and MF=5 MT=455) via NNDC`; `section` names those MF/MT sections), and in Task 4 write the difference between the ENDF/B-VIII.0 group constants and the Keepin values the engine uses in the `BETA_FRAC` and `LAMBDA_DN` notes.
- S-STD-03 (I-135 / Xe-135 data): retrieved-file rule. From https://www.nndc.bnl.gov/endf-b8.0/ download the `decay` sublibrary archive and the `nfy` (neutron-induced fission product yield) sublibrary archive into `~/ai/bwrsim-sources/`; checksum each; write both digests in `sha256` separated by `;` and both download links in `url` separated by `;`. `section` = `decay: I-135 (ZA 53135) and Xe-135 (ZA 54135) half-lives; nfy: U-235 thermal cumulative yield of I-135 and independent yield of Xe-135`. In Task 4 write the half-life-derived decay constants (`ln 2 / T_half`) and the yields next to the engine values in the notes of `LAMBDA_I`, `LAMBDA_XE`, `GAMMA_I`, `GAMMA_XE`. Fallback if NNDC is unreachable on the retrieval date: verified print citation of Lamarsh and Baratta *Introduction to Nuclear Engineering* 3rd ed., the fission-product-poisoning section of Chapter 7 (section, page, and ISBN from the copy), with `title` and `document_id` changed to match.
- S-STD-04 (Todreas and Kazimi): verified print citation; record the Chapter 3 equation number and page where the `0.066*t^-0.2` decay-heat approximation appears and the ISBN from the copyright page. If no copy can be read, cite instead the textbook that can be read (Glasstone and Sesonske *Nuclear Reactor Engineering* or Lamarsh and Baratta) with its printed coefficient; when that coefficient differs from 0.066, write both coefficients in the `DECAY_GROUPS` notes in Task 4 and keep its status `calibrated`.
- S-STD-05 (IAPWS-IF97): retrieved-file rule. `curl -L -o ~/ai/bwrsim-sources/IF97-Rev.pdf http://www.iapws.org/relguide/IF97-Rev.pdf`; checksum; `url` becomes the PDF link; `section` = `Regions 1 2 and 4 basic and saturation equations as evaluated by the CoolProp IF97 backend in tools/gen_steam_table.py; Region 4 hfg(101.325 kPa); Region 1 cp of saturated liquid 40 to 100 degC`.
- S-STD-06 (IAPWS surface tension): retrieved-file rule. Open http://www.iapws.org/relguide/Surf-H2O.html, download the linked release PDF (IAPWS R1-76(2014)) to `~/ai/bwrsim-sources/`, checksum, and replace `url` with the PDF link. `section` = `Eq. (1) sigma = B*tau^mu*(1+b*tau) with B = 235.8 mN/m; b = -0.625; mu = 1.256; Tc = 647.096 K`. These are the constants `steam.rs Sat::sigma()` uses (0.2358 N/m; 1.256; -0.625; 647.096 K).
- S-13 (NUREG-1434 Volume 1): retrieved-file rule. Locate the current revision's Volume 1 PDF through the index URL, download, checksum, replace `url` with the direct PDF link, and record `revision` and `date` from the cover. `section` = `Table 3.1.4-1; SR 3.6.1.3; Table 3.3.1.1-1; Table 3.3.6.1-1` (the tables Task 7 cites for interval bounds).

- [ ] **Step 4b: Fill `section` for every source cited by a `parameters.csv` row**

Index pages are never cited for a value. Replace `section` = `-` in each row below with the printed location that states the value; where one document is cited for several values, list the locations separated by `;`. For `confirmed` rows the same location goes at the start of the `notes` field of the citing `parameters.csv` row in Task 4/5/6 (`S-06 p.<n>; <existing notes>`), so every `confirmed` row carries value-specific evidence. `tests/register.rs::every_cited_source_exists_with_a_section` fails on any cited row whose `section` is still `-`.

| `parameters.csv` row (Task) | Cites | `section` to record | Must state |
|---|---|---|---|
| G; steam.P_ATM; bop.condenser_r_air (4; 5) | S-STD-01 | see Step 4a | 9.806 65 m/s2; 101 325 Pa; 8.314 462 618 J/(mol K) |
| RATED_POWER_W; N_BUNDLES; P_RATED; VESSEL_ID; VESSEL_HEIGHT; ui.history.rated_mwt (4; 6) | S-06 | `p.<n> <table or heading>` for each value | 870 MWt; 240 GNF2 bundles; 7.2 MPa; about 4 m diameter; about 26 to 27 m height |
| N_RODS (4) | S-06; S-08 | S-06 `p.<n>`; S-08 `Section <n.n>` (FMCRD count and arrangement) | 57 FMCRDs |
| GRID_HZ (4) | S-CRN-10 (replaces the index S-02) | `Section <n.n>` of PSAR Chapter 10 describing the turbine-generator and grid connection | 60 Hz or 1800 rpm synchronous speed; if Chapter 10 does not state it see Task 4 Step 1b |
| ic.pool_hfg; ic.pool_cp (5) | S-STD-05 | see Step 4a | hfg(101.325 kPa) = 2256.5 kJ/kg; cp of saturated liquid 4.18 to 4.22 kJ/kgK over 40 to 100 degC |
| steam.sigma_form (5) | S-STD-06 (replaces S-STD-05) | see Step 4a | B = 235.8 mN/m; mu = 1.256; b = -0.625 |
| BETA; BETA_FRAC; LAMBDA_DN (4) | S-STD-02 | see Step 4a | six-group U-235 thermal fractions and decay constants |
| LAMBDA_I; LAMBDA_XE; GAMMA_I; GAMMA_XE (4) | S-STD-03 | see Step 4a | half-lives and yields |
| DECAY_GROUPS (4) | S-STD-04 | see Step 4a | the 0.066*t^-0.2 approximation |
| CORE_FLOW_RATED (4) | S-ESBWR-04 | `Section 4.4; <thermal-hydraulic parameter table id as printed> (p.<n>)` | ESBWR rated core flow and rated thermal power |
| IC_TRAIN_RATED_W; ic.ua_pool_ref (4; 5) | S-ESBWR-05 | `Section 5.4.6; <ICS design parameter table id as printed> (p.<n>)` | per-train heat removal capacity and pool reference temperature |

Run: `grep -n ',-,[0-9]\{4\}-[0-9][0-9]-[0-9][0-9],' docs/model/sources.csv | grep -v 'index-page\|not-public'`
Expected: no output (no retrieved non-index row still has `section` = `-`).

- [ ] **Step 5: Verify no `UNRETRIEVED` remains outside index rows and the shape still holds**

Run: `grep -n UNRETRIEVED docs/model/sources.csv | grep -v index-page; awk -F',' 'NF!=12{print NR": "NF" fields"}' docs/model/sources.csv`
Expected: no output. Every non-index row now satisfies one of the Step 4a completion rules (print rows carry `print`/`none` and a verified page; `not-public` rows carry the dated note); `tests/register.rs` (Task 3) re-checks this on every run.

- [ ] **Step 6: Commit**

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
//! Enforces the evidence register: every `pub const` in params.rs has a row in
//! docs/model/parameters.csv, every row has the same number of fields as the
//! header, the locked columns exist, and every status is one of the six
//! allowed values. It also ties the registers together: every `source_ids`
//! entry exists in sources.csv (with a section), every `assumption_id` exists in
//! assumptions.md, no non-index sources row is still `UNRETRIEVED`, and every
//! value already borrowed from ESBWR has a non-`pending` applicability row.
//! Columns are looked up by header name, never by position, so later packages
//! may append columns without touching this test.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

const PARAMS_RS: &str = include_str!("../src/params.rs");
const ALLOWED_STATUS: [&str; 6] = ["confirmed", "derived", "calibrated", "esbwr-estimate", "assumed", "unavailable"];
const ALLOWED_CLASS: [&str; 5] = ["shared-concept", "adapted-model", "provisional-estimate", "inapplicable", "pending"];
/// Values already borrowed from ESBWR at the reviewed baseline and the
/// esbwr-applicability.csv row that must assess them. A later package that
/// borrows a new concept appends its pair here in the same commit.
const BORROWED: [(&str, &str); 3] = [
    ("CORE_FLOW_RATED", "E-02"),
    ("IC_TRAIN_RATED_W", "E-03"),
    ("ic.ua_pool_ref", "E-03"),
];
/// Locked header (master plan "Locked CSV headers"); columns may be appended after these, never renamed.
const LOCKED_COLUMNS: [&str; 13] = [
    "id", "value", "unit", "definition", "reference_state", "source_design", "status",
    "source_ids", "interval_lo", "interval_hi", "assumption_id", "location", "notes",
];

fn model_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/model").join(name)
}

fn register_path() -> PathBuf {
    model_path("parameters.csv")
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

/// A CSV register parsed by header name. Fields never contain commas (register rule).
struct Register {
    header: Vec<String>,
    rows: Vec<Vec<String>>,
}

impl Register {
    fn load(path: &Path) -> Self {
        let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        let mut lines = text.lines().filter(|line| !line.trim().is_empty());
        let header: Vec<String> = lines
            .next()
            .expect("register has no header line")
            .split(',')
            .map(|f| f.trim().to_string())
            .collect();
        let rows = lines.map(|line| line.split(',').map(|f| f.trim().to_string()).collect()).collect();
        Self { header, rows }
    }

    /// Column index by header name; panics with the header listed if absent.
    fn col(&self, name: &str) -> usize {
        self.header
            .iter()
            .position(|h| h == name)
            .unwrap_or_else(|| panic!("column {name:?} missing from header {:?}", self.header))
    }

    fn get<'a>(&self, row: &'a [String], name: &str) -> &'a str {
        row.get(self.col(name)).map(String::as_str).unwrap_or("")
    }
}

fn parameters() -> Register {
    Register::load(&register_path())
}

fn sources() -> Register {
    Register::load(&model_path("sources.csv"))
}

fn applicability() -> Register {
    Register::load(&model_path("esbwr-applicability.csv"))
}

/// Ids of every `| A-nnn |` table row in assumptions.md.
fn assumption_ids() -> HashSet<String> {
    let path = model_path("assumptions.md");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    text.lines()
        .filter(|line| line.starts_with("| A-"))
        .filter_map(|line| line.split('|').nth(1))
        .map(|cell| cell.trim().to_string())
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
fn header_contains_every_locked_column() {
    let reg = parameters();
    for name in LOCKED_COLUMNS {
        assert!(reg.header.iter().any(|h| h == name), "locked column {name:?} missing from header {:?}", reg.header);
    }
    // Locked columns keep their relative order so hand-edited rows stay readable.
    let positions: Vec<usize> = LOCKED_COLUMNS.iter().map(|n| reg.col(n)).collect();
    assert!(positions.windows(2).all(|w| w[0] < w[1]), "locked columns were reordered: {positions:?}");
}

#[test]
fn every_params_const_is_registered() {
    let reg = parameters();
    let ids: HashSet<&str> = reg.rows.iter().map(|r| reg.get(r, "id")).collect();
    let missing: Vec<String> = const_names().into_iter().filter(|n| !ids.contains(n.as_str())).collect();
    assert!(missing.is_empty(), "constants in params.rs with no parameters.csv row: {missing:?}");
}

#[test]
fn every_row_has_valid_shape_and_status() {
    let reg = parameters();
    assert!(!reg.rows.is_empty(), "parameters.csv has no data rows");
    let width = reg.header.len();
    let mut seen = HashSet::new();
    for (i, r) in reg.rows.iter().enumerate() {
        let line = i + 2;
        let id = reg.get(r, "id").to_string();
        assert_eq!(r.len(), width, "line {line} ({id}): {} fields but header has {width}; fields must not contain commas", r.len());
        assert!(seen.insert(id.clone()), "line {line}: duplicate id {id}");
        let status = reg.get(r, "status");
        assert!(ALLOWED_STATUS.contains(&status), "line {line} ({id}): status {status:?} not in {ALLOWED_STATUS:?}");
        if status == "assumed" || status == "esbwr-estimate" {
            assert!(reg.get(r, "assumption_id").starts_with("A-"), "line {line} ({id}): {status} rows need an assumption_id like A-001");
        }
        if status == "unavailable" {
            let lo = reg.get(r, "interval_lo");
            let hi = reg.get(r, "interval_hi");
            let both_none = lo == "none" && hi == "none";
            let both_numeric = lo.parse::<f64>().is_ok() && hi.parse::<f64>().is_ok();
            assert!(both_none || both_numeric, "line {line} ({id}): unavailable rows need a numeric interval or none/none");
        }
    }
}

#[test]
fn every_cited_source_exists_with_a_section() {
    let reg = parameters();
    let src = sources();
    let mut bad = Vec::new();
    for r in &reg.rows {
        for cited in reg.get(r, "source_ids").split(';').map(str::trim).filter(|c| !c.is_empty()) {
            match src.rows.iter().find(|s| src.get(s, "id") == cited) {
                None => bad.push(format!("{} cites {cited}: no sources.csv row", reg.get(r, "id"))),
                Some(s) if src.get(s, "kind") == "index-page" => bad.push(format!("{} cites index page {cited}", reg.get(r, "id"))),
                Some(s) if src.get(s, "url") == "not-public" => bad.push(format!("{} cites not-public row {cited}", reg.get(r, "id"))),
                Some(s) if src.get(s, "section") == "-" => bad.push(format!("{} cites {cited} which has no section", reg.get(r, "id"))),
                Some(_) => {}
            }
        }
    }
    assert!(bad.is_empty(), "source_ids problems: {bad:?}");
}

#[test]
fn every_assumption_id_exists_in_assumptions_md() {
    let reg = parameters();
    let known = assumption_ids();
    assert!(!known.is_empty(), "assumptions.md has no `| A-nnn |` rows");
    let missing: Vec<String> = reg
        .rows
        .iter()
        .map(|r| (reg.get(r, "id"), reg.get(r, "assumption_id")))
        .filter(|(_, a)| !a.is_empty() && !known.contains(*a))
        .map(|(id, a)| format!("{id} -> {a}"))
        .collect();
    assert!(missing.is_empty(), "assumption_id with no assumptions.md row: {missing:?}");
}

#[test]
fn no_unretrieved_source_outside_index_pages() {
    let src = sources();
    let mut bad = Vec::new();
    for r in &src.rows {
        let id = src.get(r, "id");
        if src.get(r, "kind") == "index-page" {
            continue;
        }
        if r.iter().any(|f| f == "UNRETRIEVED") {
            bad.push(format!("{id}: UNRETRIEVED field"));
        }
        // sha256 is `none` (print or not-public) or one or more 64-hex digests separated by `;`.
        let sha_ok = src.get(r, "sha256").split(';').all(|d| d == "none" || (d.len() == 64 && d.chars().all(|c| c.is_ascii_hexdigit())));
        if !sha_ok {
            bad.push(format!("{id}: sha256 {:?} is neither none nor 64 hex characters", src.get(r, "sha256")));
        }
    }
    assert!(bad.is_empty(), "sources.csv rows not retrieved: {bad:?}");
}

#[test]
fn existing_borrowed_values_are_assessed() {
    let reg = parameters();
    let app = applicability();
    for r in &app.rows {
        let class = app.get(r, "classification");
        assert!(ALLOWED_CLASS.contains(&class), "{}: classification {class:?} not in {ALLOWED_CLASS:?}", app.get(r, "id"));
    }
    for (param, e_id) in BORROWED {
        let row = reg.rows.iter().find(|r| reg.get(r, "id") == param).unwrap_or_else(|| panic!("borrowed value {param} has no parameters.csv row"));
        assert_eq!(reg.get(row, "status"), "esbwr-estimate", "{param}: a borrowed value is an esbwr-estimate row");
        let e = app.rows.iter().find(|r| app.get(r, "id") == e_id).unwrap_or_else(|| panic!("{param}: esbwr-applicability.csv has no row {e_id}"));
        assert_ne!(app.get(e, "classification"), "pending", "{e_id} assesses {param}, which is already borrowed; classification must not be pending");
        assert!(app.get(e, "esbwr_section").contains("S-ESBWR-"), "{e_id}: esbwr_section must cite a retrieved S-ESBWR-<ch> row");
    }
}
```

Later packages that read this register (P5 shutdown-margin test, P9 scenarios) copy the `Register` helper pattern and look rows up by `id` and columns by name, exactly as above.

- [ ] **Step 2: Run it and confirm it fails because the register does not exist yet**

Run: `cargo test --release -p bwr-core --test register 2>&1 | tail -20`
Expected: `params_rs_scan_finds_the_constants ... ok` and `no_unretrieved_source_outside_index_pages ... ok` (sources.csv was completed in Task 2); the other six tests (`header_contains_every_locked_column`, `every_params_const_is_registered`, `every_row_has_valid_shape_and_status`, `every_cited_source_exists_with_a_section`, `every_assumption_id_exists_in_assumptions_md`, `existing_borrowed_values_are_assessed`) `FAILED`, each with a panic message containing `cannot read` and `docs/model/parameters.csv`; summary `test result: FAILED. 2 passed; 6 failed`.

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
- Consumes: source `id`s (`S-*`) from Task 2; `A-nnn` ids defined in Task 7 (ids are fixed here; Task 7 writes their text).
- Produces: the register that `tests/register.rs` checks and that every later package appends to.

- [ ] **Step 1: Write the header and the 111 params.rs rows**

This register covers only constants that exist at the reviewed baseline. **No placeholder rows are seeded for constants that later packages will introduce** (for example `KIN_EXCESS_MAX`, `SDM_REQUIRED`, `MAX_BACKLOG_S`, `N_IC_RETURN_VALVES`). When a package adds a `pub const` to `params.rs`, `every_params_const_is_registered` fails until that package appends the row in the same commit; that failure is the intended gate, not something to suppress. Hardcoded values that a package introduces outside `params.rs` are appended as `<file>.<name>` rows by that package (not test-enforced; checked at package review).

Write `docs/model/parameters.csv` with the header line followed by these rows, exactly as given (the executor may correct a typo in a value only after checking `params.rs`; statuses are not changed while transcribing; Step 1b is the only place a status changes in this package):

```csv
id,value,unit,definition,reference_state,source_design,status,source_ids,interval_lo,interval_hi,assumption_id,location,notes
G,9.81,m/s2,Gravitational acceleration,-,physics,confirmed,S-STD-01,,,,params.rs,standard gravity 9.80665 rounded
RATED_POWER_W,8.70e8,W,Rated core thermal power,rated,BWRX-300,confirmed,S-06,,,,params.rs,
N_RODS,57,-,Number of fine-motion control rod drives (bottom entry),-,BWRX-300,confirmed,S-06;S-08,,,,params.rs,
N_BUNDLES,240,-,Number of GNF2 fuel bundles,-,BWRX-300,confirmed,S-06,,,,params.rs,
ROD_FULL_OUT,48,position,Rod position scale: 0 full in to 48 full out; one notch = 2 positions,-,BWR-generic,assumed,,none,none,A-001,params.rs,legacy BWR convention; verify FMCRD travel convention in Package 5
CORE_HEIGHT,3.81,m,Active fuel length,-,BWR-generic,assumed,,3.6,3.9,A-001,params.rs,GNF2 typical
ROD_SPEED,0.4,position/s,FMCRD motor drive speed (about 30 mm/s),-,BWR-generic,assumed,,none,none,A-001,params.rs,no defensible interval
SCRAM_TIME,3.0,s,Hydraulic scram full insertion time,-,BWR-generic,assumed,,2.0,5.0,A-001,params.rs,
BETA,0.0060,dk/k,Effective delayed neutron fraction,-,physics,assumed,S-STD-02,0.0050,0.0070,A-002,params.rs,U-235 thermal; exposure dependence ignored
BETA_FRAC,0.033;0.219;0.196;0.395;0.115;0.042,-,Six-group delayed neutron fractions (sum 1),-,physics,assumed,S-STD-02,none,none,A-002,params.rs,promote to confirmed once S-STD-02 table is cited
LAMBDA_DN,0.0124;0.0305;0.111;0.301;1.14;3.01,1/s,Six-group precursor decay constants,-,physics,assumed,S-STD-02,none,none,A-002,params.rs,
GEN_TIME,4.0e-5,s,Prompt neutron generation time,-,BWR-generic,assumed,,none,none,A-002,params.rs,no defensible interval
SOURCE,8.0e-6,rel-power/s,Neutron source strength in relative power units per second,-,tuning,assumed,,none,none,A-002,params.rs,sets subcritical SRM count rate; no defensible interval
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
HEAT_LOSS_HOT,1.0e6,W,Insulation heat loss at hot conditions,hot,BWR-generic,assumed,,none,none,A-006,params.rs,no defensible interval
CORE_FLOW_RATED,2000,kg/s,Natural-circulation core flow at rated conditions,rated,ESBWR,esbwr-estimate,S-ESBWR-04,none,none,A-007,params.rs,scaled from ESBWR per MWt; violates no-power-scaling rule; re-derive from geometry in Package 4; no defensible interval
LOOP_INERTIA,7.3,1/m,Natural-circulation loop inertia sum(L/A),-,BWR-generic,assumed,,4,12,A-007,params.rs,
C0,1.13,-,Drift-flux distribution parameter,-,physics,assumed,,1.0,1.2,A-007,params.rs,Zuber-Findlay typical
N_AXIAL,12,-,Number of axial nodes in the core thermal model,-,numerical,assumed,,6,24,A-016,params.rs,convergence check in Package 3
T_FW_RATED,215,degC,Rated feedwater temperature,rated,BWR-generic,assumed,,200,230,A-008,params.rs,
STEAM_FLOW_RATED,470,kg/s,Rated steam flow = 870 MW / (h_g(7.2 MPa) - h_f(215 degC)),rated,-,derived,,,,A-008,params.rs,depends on T_FW_RATED
STEAM_LINE_DP,0.2e6,Pa,Main steam line pressure drop at rated flow,rated,BWR-generic,assumed,,none,none,A-008,params.rs,no defensible interval
N_STEAM_LINES,2,-,Number of main steam lines,-,BWR-generic,assumed,,none,none,A-008,params.rs,no CONFIRMED/ASSUMED marker in code; verify in Package 6
MSIV_STROKE,4.0,s,MSIV stroke time,-,BWR-generic,assumed,,3,5,A-008,params.rs,
C_TCV,1.10*STEAM_FLOW_RATED/7.0e6,kg/s/Pa,Turbine control valve flow coefficient at full open,-,-,derived,,,,A-008,params.rs,
BYPASS_CAPACITY,1.0,-,Turbine bypass capacity as fraction of rated steam flow,-,BWR-generic,assumed,,none,none,A-008,params.rs,README lists as unconfirmed; no defensible interval
C_BYPASS,BYPASS_CAPACITY*STEAM_FLOW_RATED/7.0e6,kg/s/Pa,Bypass valve flow coefficient at full open,-,-,derived,,,,A-008,params.rs,
PRESSURE_REG_SPAN,0.25e6,Pa,Pressure regulator proportional span (100 % demand per span above setpoint),-,tuning,assumed,,none,none,A-009,params.rs,no marker in code; no defensible interval
PRESSURE_SETPOINT_DEFAULT,6.95e6,Pa,Default pressure setpoint giving 7.2 MPa dome at rated steam flow,rated,tuning,calibrated,,,,A-009,params.rs,
TURBINE_DH_RATED,0.703e6,J/kg,Effective turbine enthalpy drop to shaft at rated conditions,rated,tuning,calibrated,,,,A-010,params.rs,gives about 315 MWe gross / 300 MWe net
GENERATOR_EFF,0.985,-,Generator efficiency,-,BWR-generic,assumed,,none,none,A-010,params.rs,no defensible interval
RATED_RPM,1800,rpm,Turbine-generator synchronous speed,-,BWR-generic,assumed,,none,none,A-010,params.rs,4-pole machine at 60 Hz assumed
GRID_HZ,60,Hz,Grid frequency of the reference configuration,-,BWRX-300,confirmed,S-CRN-10,,,,params.rs,North American reference configuration decision (design-basis.md section 1)
TG_INERTIA,74300,kg m2,Turbine-generator rotational inertia (H about 4 s on 330 MVA),-,BWR-generic,assumed,,none,none,A-010,params.rs,no defensible interval
TG_LOSSES,8.0e6,W,No-load windage and friction losses at rated speed,rated,BWR-generic,assumed,,none,none,A-010,params.rs,no defensible interval
OVERSPEED_TRIP_RPM,1980,rpm,Mechanical overspeed trip (110 %),-,BWR-generic,assumed,,none,none,A-010,params.rs,no defensible interval
SPEED_RAMP,5.0,rpm/s,Speed reference ramp rate,-,tuning,assumed,,none,none,A-010,params.rs,no defensible interval
CW_INLET_C,20,degC,Circulating water inlet temperature (external boundary condition),-,BWR-generic,assumed,,5,30,A-011,params.rs,declared boundary; site dependent
CW_FLOW_PER_PUMP,7500,kg/s,Circulating water flow per pump,-,BWR-generic,assumed,,5000,10000,A-011,params.rs,
N_CW_PUMPS,2,-,Number of circulating water pumps,-,BWR-generic,assumed,,none,none,A-011,params.rs,
CONDENSER_UA,5.6e7,W/K,Condenser overall heat transfer conductance,rated,tuning,calibrated,,,,A-011,params.rs,gives about 5.6 kPa at rated load
CONDENSER_VOLUME,3000,m3,Condenser steam-space volume,-,BWR-generic,assumed,,none,none,A-011,params.rs,no defensible interval
AIR_INLEAKAGE,0.004,kg/s,Air in-leakage at full vacuum,-,BWR-generic,assumed,,none,none,A-011,params.rs,no defensible interval
VACUUM_PUMP_TAU,600,s,Air removal time constant,-,BWR-generic,assumed,,none,none,A-011,params.rs,no defensible interval
CONDENSER_TRIP_P,25.0e3,Pa,Turbine trip on high condenser pressure,-,BWR-generic,assumed,,15e3,35e3,A-014,params.rs,
CONDENSER_BYPASS_BLOCK_P,30.0e3,Pa,Bypass valves blocked above this condenser pressure,-,BWR-generic,assumed,,none,none,A-014,params.rs,no defensible interval
HOTWELL_CAPACITY,120000,kg,Hotwell capacity,-,BWR-generic,assumed,,none,none,A-012,params.rs,no defensible interval
N_FEED_PUMPS,3,-,Number of feedwater pumps (3 x 50 %),-,BWR-generic,assumed,,none,none,A-012,params.rs,
N_COND_PUMPS,2,-,Number of condensate pumps,-,BWR-generic,assumed,,none,none,A-012,params.rs,
FW_PUMP_SHUTOFF_DP,9.5e6,Pa,Feed pump shutoff differential pressure,-,BWR-generic,assumed,,none,none,A-012,params.rs,no defensible interval
FW_PUMP_RUNOUT,400,kg/s,Feed pump runout flow,-,BWR-generic,assumed,,none,none,A-012,params.rs,no defensible interval
CONDENSATE_PRESSURE,1.5e6,Pa,Condensate booster pressure at feed pump suction,-,BWR-generic,assumed,,none,none,A-012,params.rs,no defensible interval
C_FCV,1.75,kg/s/sqrt(Pa),Feedwater control valve coefficient at full open,-,tuning,assumed,,none,none,A-012,params.rs,equal-percentage trim; no defensible interval
N_IC_TRAINS,3,-,Number of isolation condenser trains,-,BWR-generic,assumed,,2,4,A-013,params.rs,README lists as unconfirmed; ESBWR has 4
IC_TRAIN_RATED_W,33.75e6,W,Heat removal per train at rated pressure with a 100 degC pool,rated,ESBWR,esbwr-estimate,S-ESBWR-05,none,none,A-013,params.rs,ESBWR unit size (S-ESBWR-05 Section 5.4.6); BWRX duty to be resolved in Package 7; no defensible interval
IC_VALVE_STROKE,15.0,s,IC condensate return valve stroke time,-,BWR-generic,assumed,,none,none,A-013,params.rs,no defensible interval
IC_POOL_MASS,1.5e6,kg,IC pool water mass per train,-,BWR-generic,assumed,,0.4e6,1.3e6,A-013,params.rs,interval derived from the 7-day claim (assumptions.md A-013); baseline exceeds the single-train 7-day mass by about 20 %; Package 7 re-derives
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
HOUSE_BASE,4.0e6,W,Base house load,-,BWR-generic,assumed,,none,none,A-015,params.rs,no defensible interval
FW_PUMP_POWER,3.5e6,W,Electrical power per running feed pump,-,BWR-generic,assumed,,none,none,A-015,params.rs,no defensible interval
COND_PUMP_POWER,1.0e6,W,Electrical power per running condensate pump,-,BWR-generic,assumed,,none,none,A-015,params.rs,no defensible interval
CW_PUMP_POWER,2.0e6,W,Electrical power per running circulating water pump,-,BWR-generic,assumed,,none,none,A-015,params.rs,no defensible interval
```

- [ ] **Step 1b: Reconcile every `confirmed` row against the retrieved evidence**

Statuses are transcribed unchanged in Step 1; this step is the only place in the package where a status changes, and only downward. For each `confirmed` row, open the source and section recorded in Task 2 Step 4b and prepend the printed location to `notes` (`S-06 p.<n>; ...`). Then apply:

- The document states the value at the registered reference state (a value rounded to the engine's precision counts; note the rounding): keep `confirmed`.
- The document states inputs from which the value follows arithmetically: set `status` = `derived`, write the formula in `notes`, keep `source_ids`.
- The document does not state the value, states a different value, or the public copy redacts it: set `status` = `assumed`, add an `assumption_id` and `none`/`none` intervals (or `unavailable` with physical bounds when the value is withheld), and keep the citation in `notes` as `downgraded from confirmed: <reason>`.

Outcomes fixed by this plan after checking the baseline code: `GRID_HZ` keeps `confirmed` only if S-CRN-10 states 60 Hz or 1800 rpm synchronous speed; otherwise it becomes `assumed` with `A-010` and `notes` = `downgraded from confirmed: reference-configuration decision (design-basis.md section 1); S-CRN-10 does not state grid frequency`. The Task 5 rows `bop.condenser_r_air` (287 = 8.314462618/0.0289647 with a non-CODATA molar mass of dry air) and `ic.pool_cp` (4186 is a constant approximation of the IF97 saturated-liquid cp 4.18 to 4.22 kJ/kgK over 40 to 100 degC) are already written as `derived` for this reason.

Run: `awk -F',' 'NR==1{for(i=1;i<=NF;i++){if($i=="status")s=i;if($i=="notes")n=i}} NR>1 && $s=="confirmed" && $n !~ /^S-/ {print "confirmed without a source location: "$1}' docs/model/parameters.csv`
Expected: no output.

- [ ] **Step 2: Run the register tests**

Run: `cargo test --release -p bwr-core --test register 2>&1 | tail -8`
Expected: `test result: FAILED. 6 passed; 2 failed`; the two failures are `every_assumption_id_exists_in_assumptions_md` and `existing_borrowed_values_are_assessed`, each panicking with `cannot read` (assumptions.md and esbwr-applicability.csv do not exist until Tasks 7 and 8). Any other failure is a transcription error to fix here.

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
vessel.W_MIN,5.0,kg/s,Minimum core flow (local recirculation inside the shroud),-,numerical,assumed,,none,none,A-016,vessel.rs:20,numerical floor; no defensible interval
vessel.vent_sensible_term,0.02*(T-100),kg/s,Extra vent flow term proportional to superheat above 100 degC,-,tuning,assumed,,none,none,A-016,vessel.rs step(),no physical basis; review in Package 4
vessel.drift_flux_vgj_coef,1.41,-,Drift velocity coefficient in vgj = 1.41*(sigma*g*drho/rho_f^2)^0.25,-,physics,assumed,,none,none,A-007,vessel.rs drift_flux_void(),Zuber-Findlay churn-turbulent form
vessel.void_clamp,0.999,-,Upper clamp on nodal void fraction,-,numerical,assumed,,none,none,A-016,vessel.rs drift_flux_void(),
vessel.chopped_cosine_extrapolation,0.3,m,Extrapolation length at each end of the axial power shape,-,BWR-generic,assumed,,none,none,A-005,vessel.rs core_th(),no defensible interval
vessel.phi2_form,1+0.5*x*(rho_f/rho_g-1),-,Two-phase friction multiplier form (damped homogeneous),-,physics,assumed,,none,none,A-007,vessel.rs phi2(),
vessel.eos_t_bounds,1..349.5,degC,Bisection bounds of the equation-of-state solve; silently clamps outside,-,numerical,assumed,,none,none,A-016,vessel.rs solve_eos(),Package 3 replaces silent clamp with DomainError
vessel.subcooled_threshold,100,degC,Below this the vessel is treated as vented at atmospheric pressure,-,BWR-generic,assumed,,none,none,A-016,vessel.rs energy_at()/solve_eos(),Package 4 replaces with regime model
vessel.tau_dc_rho,750,kg/m3,Fixed liquid density used in the downcomer mixing time constant,-,numerical,assumed,,none,none,A-016,vessel.rs step(),should use actual downcomer density
vessel.tau_dc_clamp,2..600,s,Clamp on downcomer mixing time constant,-,numerical,assumed,,none,none,A-016,vessel.rs step(),
vessel.void_lag_core,0.5,s,First-order lag on core void fraction,-,tuning,assumed,,none,none,A-016,vessel.rs step(),no defensible interval
vessel.void_lag_chimney,2.0,s,First-order lag on chimney void fraction,-,tuning,assumed,,none,none,A-016,vessel.rs step(),no defensible interval
vessel.mass_floor,1000,kg,Inventory floor applied after each mass update,-,numerical,assumed,,none,none,A-016,vessel.rs step(),creates mass; removed in Package 3
vessel.spill_band,0.5,m,Smoothing band below chimney top over which loop driving head is cut off,-,tuning,assumed,,none,none,A-007,vessel.rs drive(),no defensible interval
vessel.heatup_rate_lag,30,s,Filter time constant on the displayed heatup rate,-,tuning,assumed,,none,none,A-017,vessel.rs step(),no defensible interval
vessel.heat_loss_ref_span,268,K,Temperature span normalising heat loss (T-20)/268,-,tuning,assumed,,none,none,A-006,vessel.rs step(),
vessel.core_coverage_void_floor,0.2,-,Floor on (1-alpha) in the core coverage calculation,-,numerical,assumed,,none,none,A-016,vessel.rs core_coverage(),double-corrects void; removed in Package 3
vessel.init_relaxation,0.7/0.3,-,Under-relaxation weights in Vessel::new flow iteration (200 iterations),-,numerical,assumed,,none,none,A-016,vessel.rs new(),
reactor.rod_weight_r_ext,5.3,grid units,Extrapolated radius for J0 rod worth weighting,-,tuning,assumed,,4.5,6.0,A-003,reactor.rs rod_weights(),
reactor.rod_layout_radius2,17,grid units squared,Rod layout radius squared on the 9x9 grid (57 cells),-,tuning,assumed,,none,none,A-001,reactor.rs rod_layout(),
reactor.s_curve_form,f-sin(2*pi*f)/(2*pi),-,Integral rod worth shape (cosine flux),-,physics,assumed,,none,none,A-003,reactor.rs s_curve(),
reactor.sri_group,rods at |dx|=2 and |dy|=2,-,Select Rod Insert group (4 symmetric rods about 1.4 % dk/k),-,tuning,assumed,,none,none,A-003,reactor.rs sri_group(),reassess in Package 6
reactor.kinetics_substep,1.0e-3,s,Fixed kinetics substep,-,numerical,assumed,,none,none,A-016,reactor.rs step(),Package 3 makes adaptive
reactor.htc_factor_floor,0.02,-,Floor on fuel-to-coolant heat transfer factor when uncovered,-,tuning,assumed,,none,none,A-005,reactor.rs step(),
reactor.period_meter_tau,1.0,s,Period meter smoothing time constant,-,tuning,assumed,,none,none,A-017,kinetics.rs step(),no defensible interval
kinetics.n_floor,1e-14,rel,Floor applied to neutron population after each implicit step,-,numerical,assumed,,none,none,A-016,kinetics.rs step(),hides nonpositive denominator (master defect 2); Package 3
kinetics.period_from_zero,ln(n1/n0) with n0=0,1/s,Period estimator divides by the previous population with no floor,-,numerical,assumed,,none,none,A-016,kinetics.rs step(),Kinetics::equilibrium(0.0) then step() gives infinite inv_period then NaN (master defect 12); Package 3
ic.ua_pool_ref,100,degC,Pool temperature at which IC_TRAIN_RATED_W is defined,rated,ESBWR,esbwr-estimate,S-ESBWR-05,none,none,A-013,ic.rs ua(),ESBWR pool reference state (S-ESBWR-05 Section 5.4.6)
ic.tube_lag,20,s,Tube drain/fill time constant,-,tuning,assumed,,none,none,A-013,ic.rs step(),no defensible interval
ic.coverage_threshold,40,% pool level,Pool level below which tube coverage falls linearly to zero,-,tuning,assumed,,none,none,A-013,ic.rs step(),no defensible interval
ic.idle_pool_cooling,0.0005,K/s,Idle pool cooling rate when heat < 0.1 MW and pool > 40 degC,-,tuning,assumed,,none,none,A-013,ic.rs step(),unaccounted energy removal; Package 7
ic.idle_heat_threshold,1.0e5,W,Heat below which idle pool cooling applies,-,tuning,assumed,,none,none,A-013,ic.rs step(),
ic.pool_initial_t,40,degC,Initial pool temperature,-,tuning,assumed,,none,none,A-013,ic.rs new(),no defensible interval
ic.pool_cp,4186,J/kgK,Pool water specific heat,-,physics,derived,S-STD-05,,,,ic.rs step(),constant approximation of the IF97 saturated-liquid cp (4.18 to 4.22 kJ/kgK over 40 to 100 degC); not confirmed at the pool reference state
ic.pool_hfg,2.257e6,J/kg,Pool latent heat at atmospheric,-,physics,confirmed,S-STD-05,,,,ic.rs step(),
bop.steam_line_k_form,STEAM_LINE_DP/STEAM_FLOW_RATED^2/m^2,Pa s2/kg2,Steam line loss coefficient scaled by averaged MSIV opening,-,tuning,assumed,,none,none,A-008,bop.rs compute_flows(),averages line positions; Package 6
bop.msiv_min_fraction,1e-4,-,Averaged MSIV opening below which flow is zero,-,numerical,assumed,,none,none,A-016,bop.rs compute_flows(),
bop.p_ref_chest_threshold,0.5e6,Pa,Chest pressure above which it is used as the valve sizing reference,-,tuning,assumed,,none,none,A-009,bop.rs SteamSystem::step(),
bop.p_ref_dome_offset,0.05e6,Pa,Offset subtracted from dome pressure for valve sizing when chest pressure is low,-,tuning,assumed,,none,none,A-009,bop.rs SteamSystem::step(),
bop.p_ref_floor,0.2e6,Pa,Floor on the valve sizing reference pressure,-,tuning,assumed,,none,none,A-009,bop.rs SteamSystem::step(),
bop.tcv_open_rate,0.10,1/s,TCV opening rate,-,BWR-generic,assumed,,none,none,A-009,bop.rs SteamSystem::step(),no defensible interval
bop.tcv_close_rate,0.5,1/s,TCV closing rate (normal),-,BWR-generic,assumed,,none,none,A-009,bop.rs SteamSystem::step(),no defensible interval
bop.tcv_trip_close_rate,5.0,1/s,TCV closing rate on trip (0.2 s stroke),-,BWR-generic,assumed,,none,none,A-009,bop.rs SteamSystem::step(),no defensible interval
bop.bypass_auto_rate,2.0,1/s,Bypass valve rate in automatic,-,BWR-generic,assumed,,none,none,A-009,bop.rs SteamSystem::step(),no defensible interval
bop.bypass_manual_rate,0.2,1/s,Bypass valve rate in manual,-,tuning,assumed,,none,none,A-009,bop.rs SteamSystem::step(),
bop.load_limit_default,1.05,-,Default load limiter,-,tuning,assumed,,none,none,A-010,bop.rs Turbine::new(),
bop.governor_kp,10.0,rel flow per rel speed error,Speed governor proportional gain,-,tuning,assumed,,none,none,A-010,bop.rs tcv_demand(),
bop.governor_ki,0.5,1/s,Speed governor integral gain,-,tuning,assumed,,none,none,A-010,bop.rs tcv_demand(),
bop.governor_integral_clamp,-0.05..0.10,-,Governor integral clamp,-,tuning,assumed,,none,none,A-010,bop.rs tcv_demand(),
bop.roll_margin,0.01,rel flow,Extra steam the turbine may take above the pressure-regulator demand while rolling,-,tuning,assumed,,none,none,A-010,bop.rs tcv_demand(),
bop.turbine_pf_form,ln(p_chest/p_cond)/ln(7.0e6/5.6e3) clamped 0..1.2,-,Pressure-ratio factor on turbine work,-,tuning,assumed,,none,none,A-010,bop.rs Turbine::step(),
bop.turbine_extra_loss,0.5e6,W,Fixed loss while rotating,-,tuning,assumed,,none,none,A-010,bop.rs Turbine::step(),
bop.reverse_power_delay,3.0,s,Reverse-power breaker opening delay,-,BWR-generic,assumed,,none,none,A-010,bop.rs Turbine::step(),no defensible interval
bop.sync_speed_window,20,rpm,Breaker close permissive speed window,-,BWR-generic,assumed,,none,none,A-010,bop.rs close_breaker(),
bop.sync_phase_window,15,deg,Breaker close phase window (outside = out-of-phase trip),-,BWR-generic,assumed,,none,none,A-010,bop.rs close_breaker(),
bop.sync_slip_window,0.25,Hz,Breaker close slip window,-,BWR-generic,assumed,,none,none,A-010,bop.rs close_breaker(),
bop.condenser_r_air,287,J/kgK,Gas constant of air in the condenser air model,-,physics,derived,S-STD-01,,,,bop.rs Condenser,R/M_air = 8.314462618/0.0289647; M_air of dry air is not a CODATA value
bop.condenser_t_air,303,K,Fixed air temperature in the condenser air model,-,tuning,assumed,,none,none,A-011,bop.rs Condenser,no defensible interval
bop.condenser_p_steam_init,2.3e3,Pa,Initial condenser steam partial pressure,-,tuning,assumed,,none,none,A-011,bop.rs Condenser::new(),
bop.condenser_p_air_init_vacuum,0.3e3,Pa,Initial air partial pressure with vacuum,-,tuning,assumed,,none,none,A-011,bop.rs Condenser::new(),
bop.condenser_p_cap,1.2*P_ATM,Pa,Cap on condenser pressure,-,numerical,assumed,,none,none,A-011,bop.rs pressure(),
bop.condenser_tau_cw,15,s,Condenser pressure time constant with CW flow,-,tuning,assumed,,none,none,A-011,bop.rs Condenser::step(),no defensible interval
bop.condenser_tau_no_cw,120,s,Condenser pressure time constant without CW flow,-,tuning,assumed,,none,none,A-011,bop.rs Condenser::step(),no defensible interval
bop.condenser_no_cw_floor_t,30,degC,Steam saturation temperature assumed with no CW and no heat load,-,tuning,assumed,,none,none,A-011,bop.rs Condenser::step(),
bop.condenser_p_steam_floor,700,Pa,Floor on steam partial pressure for saturation lookup,-,numerical,assumed,,none,none,A-011,bop.rs Condenser::step(),
bop.hotwell_initial_fraction,0.6,-,Initial hotwell fill fraction,-,tuning,assumed,,none,none,A-012,bop.rs Condenser::new(),
bop.hotwell_makeup_flow,30,kg/s,Automatic makeup/reject flow,-,tuning,assumed,,none,none,A-012,bop.rs Condenser::step(),unaccounted boundary flow; Package 6
bop.hotwell_makeup_band,0.40/0.85,-,Hotwell fractions triggering makeup/reject,-,tuning,assumed,,none,none,A-012,bop.rs Condenser::step(),
bop.hotwell_ok_fraction,0.05,-,Hotwell fraction below which feed suction is lost,-,tuning,assumed,,none,none,A-012,sim.rs step(),
bop.fcv_rangeability,30,-,Equal-percentage valve rangeability,-,BWR-generic,assumed,,none,none,A-012,bop.rs fcv_characteristic(),no defensible interval
bop.fcv_rate,0.1,1/s,FCV stroke rate,-,BWR-generic,assumed,,none,none,A-012,bop.rs Feedwater::step(),no defensible interval
bop.fw_static_dp,0.15e6,Pa,Static head and piping loss in the feed line,-,tuning,assumed,,none,none,A-012,bop.rs hydraulics(),
bop.fw_pump_curve_form,SHUTOFF_DP*(1-(q/RUNOUT)^2),Pa,Feed pump head curve form,-,BWR-generic,assumed,,none,none,A-012,bop.rs hydraulics(),
bop.fw_idle_pump_loss,(q/150)^2*1.0e5,Pa,Flow loss through idle feed pumps on condensate pressure alone,-,tuning,assumed,,none,none,A-012,bop.rs hydraulics(),
bop.fw_low_suction_delay,3.0,s,Feed pump trip delay on low suction pressure,-,BWR-generic,assumed,,none,none,A-012,bop.rs Feedwater::step(),no defensible interval
bop.level_1e_ki,0.0002,1/(cm s),Single-element level controller integral gain,-,tuning,assumed,,none,none,A-012,bop.rs Feedwater::step(),
bop.level_1e_kp,0.004,1/cm,Single-element level controller proportional gain,-,tuning,assumed,,none,none,A-012,bop.rs Feedwater::step(),
bop.level_3e_level_gain,1.2,kg/s per cm,Three-element level-to-flow-demand gain,-,tuning,assumed,,none,none,A-012,bop.rs Feedwater::step(),
bop.level_3e_ki,0.3,1/s,Three-element flow-error integral gain,-,tuning,assumed,,none,none,A-012,bop.rs Feedwater::step(),
bop.level_3e_kp,0.2,-,Three-element flow-error proportional gain,-,tuning,assumed,,none,none,A-012,bop.rs Feedwater::step(),
bop.fw_heating_form,(T_FW_RATED-35)*load^0.75,K,Feedwater temperature rise from extraction heating versus turbine load,-,tuning,assumed,,none,none,A-012,bop.rs Feedwater::step(),
bop.fw_heater_out_factor,0.65,-,Fraction of heating retained with HP heaters out of service,-,BWR-generic,assumed,,none,none,A-012,bop.rs Feedwater::step(),no defensible interval
bop.fw_temp_lag,180,s,Feedwater temperature time constant,-,tuning,assumed,,none,none,A-012,bop.rs Feedwater::step(),no defensible interval
bop.fw_pump_heat,2.0,K,Temperature rise across running feed pumps,-,BWR-generic,assumed,,1,4,A-012,bop.rs Feedwater::step(),
sim.DT,0.05,s,Fixed plant time step,-,numerical,assumed,,0.0125,0.1,A-016,sim.rs,convergence study in Package 3
sim.max_steps_per_advance,4000,-,Step cap per advance call (accumulator zeroed when hit),-,numerical,assumed,,none,none,A-016,sim.rs advance(),silently drops time (master defect 9); becomes MAX_STEPS_PER_ADVANCE with MAX_BACKLOG_S accounting in Package 10
sim.downstream_requested_flows,steam and reject flows passed unbounded,-,Turbine condenser and feedwater consume requested not applied vessel outflows,-,numerical,assumed,,none,none,A-016,sim.rs step()/protection(),no transfer allocation (master defect 13); Package 3
sim.speed_clamp,0.1..100,-,Simulation speed clamp,-,ui-demo,assumed,,none,none,A-019,sim.rs apply(),
sim.scram_reset_delay,10,s,Minimum time after scram before reset is accepted,-,BWR-generic,assumed,,none,none,A-014,sim.rs apply(),
sim.sri_power_threshold,0.4,rel,Power above which turbine trip / load rejection triggers Select Rod Insert,-,BWR-generic,assumed,,0.3,0.6,A-014,sim.rs protection(),
sim.l8_feed_pump_trip_placement,inside if !turbine.tripped,-,L8 high level trips the turbine and all feed pumps only while the turbine is not already tripped; with the turbine already tripped a later L8 trips nothing,-,BWR-generic,assumed,,none,none,A-014,sim.rs protection(),control-flow placement recorded not fixed; coverage.md lists it; Package 8
sim.setdown_cm,-100,cm,Level setpoint setdown after scram,-,BWR-generic,assumed,,none,none,A-014,sim.rs step(),
sim.fuel_damage_t,1200,degC,Average fuel temperature declared as core damage,-,tuning,assumed,,none,none,A-005,sim.rs protection(),unjustified; replaced in Package 5
sim.fuel_hot_alarm_t,1000,degC,Fuel temperature high alarm,-,tuning,assumed,,none,none,A-005,sim.rs active_alarms(),
sim.srm_cps_form,n*2.0e10+3 capped 1e6,cps,SRM count rate indication formula,-,tuning,assumed,,none,none,A-017,sim.rs snapshot(),
sim.irm_full_scale_form,0.4*10^((range-10)/2),rel,IRM full-scale power for range 1..10,-,BWR-generic,assumed,,none,none,A-017,sim.rs irm_full_scale(),legacy arrangement
sim.irm_upscale_block,108,% of range,IRM upscale rod block and alarm,-,BWR-generic,assumed,,none,none,A-014,sim.rs withdraw_block(),
sim.aprm_setdown_block,0.12,rel,APRM setdown rod block outside RUN,-,BWR-generic,assumed,,none,none,A-014,sim.rs withdraw_block(),
sim.aprm_run_block,1.08,rel,APRM upscale rod block in RUN,-,BWR-generic,assumed,,none,none,A-014,sim.rs withdraw_block(),
sim.refuel_withdraw_block,out > 1 counting rods with pos > 0.01,-,REFUEL mode rod withdraw block: blocks only when more than one rod is already withdrawn; counts actual rod positions and ignores demanded positions and rods in motion,-,BWR-generic,assumed,,none,none,A-014,sim.rs withdraw_block(),legacy one-rod-out interlock; a second rod can be selected and moved until its actual position exceeds 0.01; recorded not fixed; Package 8
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
sim.rwcu_reject_max,15,kg/s,Maximum RWCU reject flow,-,BWR-generic,assumed,,none,none,A-012,sim.rs apply(),no defensible interval
sim.level_setpoint_clamp,-100..50,cm,Operator level setpoint clamp,-,tuning,assumed,,none,none,A-014,sim.rs apply(),
sim.pressure_setpoint_clamp,0.2e6..7.4e6,Pa,Operator pressure setpoint clamp,-,tuning,assumed,,none,none,A-009,sim.rs apply(),
sim.speed_setpoint_clamp,0..1.02*RATED_RPM,rpm,Operator speed setpoint clamp,-,tuning,assumed,,none,none,A-010,sim.rs apply(),
sim.load_limit_clamp,0..1.1,-,Load limiter clamp,-,tuning,assumed,,none,none,A-010,sim.rs apply(),
sim.event_ttl,30,s,Lifetime of transient operator messages,-,ui-demo,assumed,,none,none,A-019,sim.rs event(),
sim.cold_shutdown_ic,40 degC;30 days;vented,-,Cold shutdown initial condition,cold,tuning,assumed,,none,none,A-018,sim.rs cold_shutdown(),cooling to 40 degC requires an unimplemented system
sim.hot_standby_ic,6.9 MPa;2 days decay;1 feed pump;1 condensate pump,-,Hot standby initial condition,hot,tuning,assumed,,none,none,A-018,sim.rs hot_standby(),feedwater temperatures are the two rows below
sim.hot_standby_vessel_t_feed,180,degC,Feedwater temperature passed to Vessel::new in hot standby,hot,tuning,assumed,,none,none,A-018,sim.rs hot_standby(),differs from the 150 degC passed to Feedwater::new in the same constructor; recorded not reconciled; Package 9
sim.hot_standby_feedwater_t_feed,150,degC,Initial feedwater temperature passed to Feedwater::new in hot standby,hot,tuning,assumed,,none,none,A-018,sim.rs hot_standby(),differs from the 180 degC passed to Vessel::new; equals the sim.fw_temp_low_alarm threshold; Package 9
sim.full_power_ic,rods inserted to critical;600 s settle;xenon reset;120 s settle,-,Full power initial condition construction,rated,tuning,assumed,,none,none,A-018,sim.rs full_power(),
sim.full_power_tcv_init,0.85,-,Initial TCV position at full power,rated,tuning,assumed,,none,none,A-018,sim.rs full_power(),
sim.full_power_fcv_init,0.7,-,Initial FCV position at full power,rated,tuning,assumed,,none,none,A-018,sim.rs full_power(),
sim.full_power_cond_init,5.6e3 Pa;35 degC,-,Initial condenser state at full power,rated,tuning,assumed,,none,none,A-018,sim.rs full_power(),
steam.P_ATM,101325,Pa,Standard atmospheric pressure,-,physics,confirmed,S-STD-01,,,,steam.rs,
steam.table_range,1..350,degC,Saturation table range (clamped outside),-,numerical,assumed,,none,none,A-016,steam.rs sat_t(),Package 4 declares domain
steam.sigma_form,0.2358*tau^1.256*(1-0.625*tau),N/m,Surface tension correlation,-,physics,confirmed,S-STD-06,,,,steam.rs Sat::sigma(),IAPWS R1-76(2014) Eq. (1); B = 0.2358 N/m; mu = 1.256; b = -0.625
```

- [ ] **Step 1b: File-by-file reconciliation of hardcoded engine behaviour**

The rows above were transcribed by hand; the code is authoritative. List every numeric literal and every behaviour-selecting comparison in the engine and check each against the rows (the `location` column names the function):

```bash
for f in kinetics.rs reactor.rs vessel.rs steam.rs bop.rs ic.rs sim.rs api.rs; do
  echo "== $f"
  grep -nE '[0-9]+\.[0-9]+|[0-9]e-?[0-9]|if .*[<>]' crates/bwr-core/src/$f | grep -vE '^[0-9]+:\s*//'
done > /tmp/engine-literals.txt; wc -l /tmp/engine-literals.txt
```

Walk `/tmp/engine-literals.txt` top to bottom. An item is *consequential* when changing it changes a trip, block, alarm, mode, rate, time constant, clamp, count, initial condition, or control-flow placement; pure arithmetic (`0.5` halving; `1e6` and `86400` unit conversions; array sizes; the 200-iteration loop count in `Vessel::new`) is exempt. Every consequential item either appears in an existing row's `value` or `definition` (tick it off in the file) or gets a new `<file>.<name>` row appended in the same style with `notes` describing current behaviour; nothing is fixed in this package. A control-flow placement that changes behaviour is registered with the branch condition as its `value` (the REFUEL `out > 1` actual-position rule and the L8 feed-pump trip inside the turbine-not-tripped branch, both in `sim.rs`, were found at review and are already rows above). Record the number of rows this step appended in the Step 3 commit message.

- [ ] **Step 2: Run the register tests**

Run: `cargo test --release -p bwr-core --test register 2>&1 | tail -4`
Expected: `test result: FAILED. 6 passed; 2 failed` (the same two `cannot read` failures as Task 4 Step 2).

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
ui.mock.N_FEED,3,-,Mock feed pump count,-,ui-demo,assumed,,none,none,A-019,ui/src/sim/mock.ts,demo only; equals engine N_FEED_PUMPS 3
ui.mock.LAMBDA_I,2.87e-5,1/s,Mock I-135 decay constant,-,ui-demo,assumed,,none,none,A-019,ui/src/sim/mock.ts,demo only; equals engine LAMBDA_I
ui.mock.LAMBDA_X,2.09e-5,1/s,Mock Xe-135 decay constant,-,ui-demo,assumed,,none,none,A-019,ui/src/sim/mock.ts,demo only; equals engine LAMBDA_XE
ui.mock.scram_high_pressure,7.8,MPa,Mock reactor high pressure scram threshold,-,ui-demo,assumed,,none,none,A-019,ui/src/sim/mock.ts scram checks,demo only; engine SCRAM_HIGH_PRESSURE is 7.50 MPa; mock differs
ui.mock.scram_low_level,-150,cm,Mock low level (L3) scram and alarm threshold,-,ui-demo,assumed,,none,none,A-019,ui/src/sim/mock.ts scram checks and alarms,demo only; engine L3_CM is -100 cm; mock differs
ui.mock.pressure_setpoint_clamp,0.2..7.8,MPa,Mock operator pressure setpoint clamp,-,ui-demo,assumed,,none,none,A-019,ui/src/sim/mock.ts setPressureSetpoint,demo only; engine sim.pressure_setpoint_clamp is 0.2..7.4 MPa; mock differs
ui.mock.XE_BURN,2,-,Mock xenon burnout factor,-,ui-demo,assumed,,none,none,A-019,ui/src/sim/mock.ts,demo only
ui.mock.tsat_form,179.9*p^0.24,degC,Mock saturation temperature fit,-,ui-demo,assumed,,none,none,A-019,ui/src/sim/mock.ts,demo only
ui.history.TREND_WINDOW_S,600,s,Trend window,-,ui-demo,assumed,,none,none,A-019,ui/src/sim/history.ts,
ui.history.MIN_SAMPLE_DT,1,s,Trend sampling interval (decimation),-,ui-demo,assumed,,none,none,A-019,ui/src/sim/history.ts,can hide sub-second excursions; Package 10
ui.topbar.SPEEDS,1;2;5;10;30,x,Available simulation speeds,-,ui-demo,assumed,,none,none,A-019,ui/src/components/TopBar.tsx,
tauri.tick_ms,20,ms,Background physics loop sleep between advance calls,-,shell,assumed,,10,50,A-016,src-tauri/src/lib.rs run(),commands apply only between advance calls; Package 10 defines the boundary rule
tauri.emit_ms,100,ms,Snapshot emit cadence (10 Hz of 20 Hz physics),-,shell,assumed,,50,200,A-016,src-tauri/src/lib.rs run(),can hide sub-second excursions (master defect 10); Package 10 adds extrema
tauri.max_real_dt,0.5,s,Wall-clock dt clamp dt.min(0.5) per loop iteration,-,shell,assumed,,none,none,A-016,src-tauri/src/lib.rs run(),discards wall-clock time after any stall over 0.5 s (master defect 11); removed in Package 10
```

- [ ] **Step 1b: File-by-file reconciliation of UI and shell behaviour**

Repeat the Task 5 Step 1b procedure for `ui/src/sim/mock.ts`, `ui/src/sim/history.ts`, `ui/src/components/TopBar.tsx`, and `src-tauri/src/lib.rs`:

```bash
for f in ui/src/sim/mock.ts ui/src/sim/history.ts ui/src/components/TopBar.tsx src-tauri/src/lib.rs; do
  echo "== $f"
  grep -nE '[0-9]+\.[0-9]+|[0-9]e-?[0-9]|if .*[<>]|^(export )?const ' $f | grep -vE '^[0-9]+:\s*//'
done > /tmp/ui-literals.txt; wc -l /tmp/ui-literals.txt
```

Same consequential rule; every mock threshold, rate, or count that is not yet a row gets a `ui.mock.<name>` row with `ui-demo`/`assumed`/`A-019`. Where the mock and the engine disagree, the `notes` field states the engine value and ends with `mock differs` (the rows above already do this for the 7.8 MPa pressure scram, the -150 cm low-level scram, and the 0.2..7.8 MPa setpoint clamp; the mock's 10 s short-period scram and its turbine-trip scram above 40 % APRM are two more to register here). Nothing in the mock is changed in this package.

- [ ] **Step 2: Run the register tests and count rows**

Run: `cargo test --release -p bwr-core --test register 2>&1 | tail -4 && wc -l docs/model/parameters.csv`
Expected: `6 passed; 2 failed` (same two `cannot read` failures as Task 4); line count of at least `271` (header + 111 + 133 + 26 as written in this plan). The reconciliation steps (Task 5 Step 1b and Task 6 Step 1b) append rows, so recompute the sum from the counts recorded in those commit messages and write the actual count in this commit message.

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

Interval rule: a numeric `interval_lo`/`interval_hi` on an `assumed`, `esbwr-estimate`, or `unavailable` row is allowed only when the `Interval basis` column of its assumption names that parameter with one of three basis kinds: *physical* (the bounds follow from a physical or ordering constraint stated there), *derived* (the bounds are computed from other registered values by the formula stated there), or *sourced* (a `sources.csv` row and printed section state the range or the two bounding values; the executor records the stated values in the parameter's `notes`). A parameter not named, or named as *none*, has `none`/`none` and `no defensible interval` in `notes`. If a *sourced* section turns out not to state the quantity (or is redacted), the interval becomes `none`/`none` and `notes` gets `no defensible interval; <source> <section> does not state it`; nothing is inferred.

| Id | Title | Statement | Sensitivity treatment | Interval basis | Resolved by |
|---|---|---|---|---|---|
| A-001 | Rod geometry and drive kinematics | 57 FMCRDs on a 9x9 grid within radius sqrt(17); position 0..48 with 2-position notches; 0.4 position/s motor speed; 3 s hydraulic scram; active length 3.81 m. | Scram time 2..5 s; length 3.6..3.9 m; rod speed has no defensible interval. | SCRAM_TIME sourced: S-13 Table 3.1.4-1 scram-time limits and S-ESBWR-04 Section 4.6 FMCRD hydraulic scram insertion time (record both; widen or narrow the interval to span them); CORE_HEIGHT sourced: S-CRN-04 Section 4.2 GNF2 active fuel length; ROD_SPEED none (30 mm/s anchor from S-ESBWR-04 Section 4.6 recorded in notes) | Package 5 |
| A-002 | Point-kinetics data | Six-group U-235 thermal data (Keepin); beta 0.006; generation time 40 us; fixed source 8e-6. Exposure dependence of beta ignored. | beta 0.005..0.007; generation time and source strength have no defensible interval. | BETA sourced: S-STD-02 U-235 thermal total (upper bound) and Pu-239 total (direction of the exposure trend) with the core-average value from S-ESBWR-04 Section 4.3; GEN_TIME and SOURCE none | Package 5 |
| A-003 | Reactivity coefficients and rod worth | Cold excess 0.130 and total rod worth 0.200 are tuned for HFP criticality with about 12 % rod worth inserted; Doppler, void, moderator, xenon coefficients are BWR-generic; rod worth shape is J0^2 radial and cosine axial; SRI group is four symmetric rods. | Intervals in parameters.csv; sensitivity study in Package 5. | DOPPLER_A; VOID_COEF; MOD_TEMP_COEF; XENON_WORTH_EQ sourced: S-ESBWR-04 Section 4.3 reactivity coefficients and xenon worth (typical-BWR; never BWRX values; widen the interval if the DCD value lies outside it); reactor.rod_weight_r_ext physical: above the layout radius sqrt(17) = 4.12 and below the 9x9 half-diagonal 6.36 | Package 5 |
| A-004 | Xenon data and flux | Standard I/Xe constants; sigma_a*phi = 7.95e-5 1/s at rated flux. | Flux 5e-5..1.2e-4 1/s. | SIGMA_PHI_XE derived: sigma_a 2.65e-18 cm2 (S-STD-03) times the core-average thermal flux range stated in S-ESBWR-04 Section 4.3 (sourced) | Package 5 |
| A-005 | Decay heat and fuel thermal model | Nine-group fit to 0.066 t^-0.2 for infinite operation; lumped fuel node; 264 K average fuel-to-coolant rise; 2.5 % direct coolant heating; 1200 degC average fuel declared "core damage" without basis. | Refit to a transcribed public table with uncertainty band; separate clad node. | DIRECT_COOLANT_HEAT sourced: S-ESBWR-04 Section 4.4 fraction of fission power deposited outside the fuel; FUEL_HEAT_CAP derived: 240 bundles times the GNF2 bundle UO2 mass (S-CRN-04 Section 4.2) times UO2 cp 240..320 J/kgK over 300..1000 K; vessel.chopped_cosine_extrapolation none | Package 5 |
| A-006 | Vessel geometry, elevations, metal mass | All internal elevations, areas, and volumes beyond the published 4 m x 26.7 m envelope are engineering estimates; metal heat capacity 2.75e8 J/K; hot heat loss 1 MW. | Intervals in parameters.csv; level and pressure sensitivity in Package 4. | all geometry intervals derived from the S-06 envelope (VESSEL_ID 4.0 m gives A_VESSEL 12.57 m2; VESSEL_HEIGHT 26.7 m) with the ordering 0 < Z_CORE_BOTTOM < Z_TAF < Z_CHIMNEY_TOP < Z_DRYER < VESSEL_HEIGHT and A_CORE <= A_CHIMNEY < A_VESSEL - A_DOWNCOMER; V_LOWER_PLENUM below A_VESSEL*Z_CORE_BOTTOM; METAL_HEAT_CAP derived from 400..700 t of steel at 500 J/kgK; HEAT_LOSS_HOT none | Package 4 |
| A-007 | Natural circulation model | Rated flow 2000 kg/s scaled from ESBWR per MWt (violates the no-power-scaling rule and must be re-derived); friction calibrated at the rated point; drift-flux C0 1.13 with churn-turbulent vgj; homogeneous-like two-phase multiplier; loop head cut off over 0.5 m below chimney top. | Re-derive from geometry and head balance; the flow interval is established by that derivation in Package 4. | LOOP_INERTIA derived: sum(L/A) over the A-006 interval geometry; C0 physical: 1.0 for a flat profile up to the 1.2 slug-flow value of the Zuber-Findlay form; CORE_FLOW_RATED none until the Package 4 head-balance derivation; vessel.spill_band none | Package 4 |
| A-008 | Main steam system | Two steam lines; 215 degC feedwater; 0.2 MPa line drop; 4 s MSIV stroke; 100 % bypass. | Feed temp 200..230 degC; bypass capacity has no defensible interval (ESBWR 110 % anchor). | T_FW_RATED sourced: S-ESBWR-10 Section 10.1 heat balance (215.6 degC) and S-CRN-10 heat balance; MSIV_STROKE sourced: S-13 SR 3.6.1.3 isolation time 3 to 5 s; STEAM_LINE_DP and BYPASS_CAPACITY none (S-ESBWR-10 Section 10.4.4 bypass capacity recorded in notes as anchor) | Package 6 |
| A-009 | Pressure control | Proportional regulator with 0.25 MPa span; 6.95 MPa default setpoint; valve rates as registered. | Controller retune only after physics corrections. | none for every interval (PRESSURE_REG_SPAN; TCV and bypass valve rates) | Package 6 |
| A-010 | Turbine-generator | 1800 rpm 4-pole machine; effective enthalpy drop calibrated to 300 MWe net; inertia H about 4 s; fixed loss model; governor gains tuned. | Intervals in parameters.csv. | none for every interval (GENERATOR_EFF; TG_INERTIA; TG_LOSSES; OVERSPEED_TRIP_RPM; SPEED_RAMP; bop.reverse_power_delay) | Package 6 |
| A-011 | Condenser and circulating water | CW inlet 20 degC is an external boundary; two CW pumps at 7500 kg/s; UA calibrated to 5.6 kPa; air model at fixed 303 K. | CW inlet 5..30 degC. | CW_INLET_C physical: site cooling water between near-freezing and 30 degC; CW_FLOW_PER_PUMP derived: condenser duty (RATED_POWER_W minus gross electrical output) over cp times an 8..12 K rise divided by N_CW_PUMPS; CONDENSER_VOLUME; AIR_INLEAKAGE; VACUUM_PUMP_TAU; bop.condenser_t_air; bop.condenser_tau_cw; bop.condenser_tau_no_cw none | Package 6 |
| A-012 | Condensate and feedwater | 2 condensate + 3 x 50 % feed pumps; pump curve form; equal-percentage FCV; controller gains tuned; extraction heating fit; hotwell makeup/reject unaccounted. | Explicit CST boundary in Package 6. | bop.fw_pump_heat derived: FW_PUMP_SHUTOFF_DP/(rho*cp*eta) with eta 0.7..1.0; every other interval none | Package 6 |
| A-013 | Isolation condensers | 3 trains at 33.75 MWt each (ESBWR unit size); 1.5e6 kg pool per train; single lumped valve; 20 s tube lag; idle pool cooling without accounting. | Pool mass 0.4e6..1.3e6 kg from the 7-day claim; train duty has no defensible interval (33.75 MWt anchor). | IC_POOL_MASS derived: the 7-day integral of the 0.066*t^-0.2 fit (S-STD-04) at 870 MWt is 0.0825*604800^0.8*8.7e8 = 3.0e12 J; at 2.5e6 J/kg (40 degC to boiling plus hfg) that is 1.2e6 kg; the lower bound shares it over N_IC_TRAINS (0.4e6 kg) and the upper bound puts it in one credited train (1.3e6 kg; recompute with Python and record); N_IC_TRAINS sourced: S-ESBWR-05 Section 5.4.6 (4 trains) and S-10 (BWRX train count); IC_TRAIN_RATED_W none (33.75 MWt anchor per S-ESBWR-05 Section 5.4.6); IC_VALVE_STROKE; ic.tube_lag; ic.coverage_threshold; ic.pool_initial_t none | Package 7 |
| A-014 | Protection setpoints and alarms | All scram, isolation, trip, block, and alarm setpoints are BWR-generic; level setpoints L1..L8 relative to NWL; legacy SRM/IRM/APRM arrangement; the REFUEL withdraw block counts actual positions only (`out > 1`); the L8 feed-pump trip sits inside the turbine-not-tripped branch so it does not act once the turbine is tripped. | Cause-and-effect matrix with provenance. | pressure setpoints physical ordering P_RATED < SCRAM_HIGH_PRESSURE < IC_HIGH_PRESSURE < VESSEL_DESIGN_PRESSURE and sourced: S-13 Table 3.3.1.1-1 allowable values (dome pressure high; APRM; IRM) and Table 3.3.6.1-1 (main steam line pressure low; condenser vacuum low) and S-ESBWR-05 Section 5.3 RPV design pressure; sim.sri_power_threshold sourced: S-13 Table 3.3.1.1-1 turbine stop valve closure scram bypass power; CONDENSER_BYPASS_BLOCK_P none | Package 8 |
| A-015 | House loads | 4 MW base plus per-pump loads. | Intervals in parameters.csv. | none for every interval | Package 6 |
| A-016 | Numerical and shell timing choices | 0.05 s plant step; 1 ms kinetics substep; 12 axial nodes; void lags 0.5/2 s; 1000 kg inventory floor; EOS bisection 1..349.5 degC; 4000-step advance cap; downstream components consume requested rather than applied vessel outflows; Tauri loop 20 ms tick, 100 ms emit, 0.5 s wall-clock clamp. | Convergence at 0.05/0.025/0.0125 s; floors replaced by domain errors; transfer allocation in Package 3; backlog accounting replaces the clamp in Package 10. | N_AXIAL and sim.DT derived: convergence-study range (half to double the baseline; Package 3); tauri.tick_ms and tauri.emit_ms derived: tick no longer than sim.DT and emit within the 5..20 Hz display band; vessel.W_MIN and the void lags none | Package 3; 10 |
| A-017 | Instrument indication models | SRM cps = n*2e10+3; IRM full scale 0.4*10^((range-10)/2); APRM = n; period meter 1 s filter; heatup-rate 30 s filter; no sensor lag, range, or failure. | Sensor models in Package 8. | none for every interval | Package 8 |
| A-018 | Scenario initial conditions | Cold shutdown: 30 days decay; 40 degC; vented (reaching 40 degC needs a system not modelled). Hot standby: 2 days decay; 6.9 MPa; one feed pump. Full power: rods inserted to criticality then settled; xenon reset to equilibrium. | Verified in Package 9. | no intervals | Package 9 |
| A-019 | UI demonstration mock | Browser mock physics are not the simulator; constants are demo-only and never cited as plant behaviour; its scram thresholds (7.8 MPa; -150 cm) and setpoint clamp differ from the engine (7.50 MPa; -100 cm; 7.4 MPa) and are registered as `ui.mock.*` rows with `mock differs`. | None; mock is relabelled in Package 10. | no intervals | Package 10 |
```

- [ ] **Step 2: Verify every assumption id used in parameters.csv exists here**

Run (resolves the column by header name; exits nonzero on the first missing id):

```bash
awk -F',' 'NR==1{for(i=1;i<=NF;i++) if($i=="assumption_id") c=i; if(!c){print "no assumption_id column"; exit 1}; next} $c!="" {print $c}' docs/model/parameters.csv \
  | sort -u | while read id; do grep -q "^| $id |" docs/model/assumptions.md || { echo "missing $id"; exit 1; }; done; echo "exit $?"
```

Expected: `exit 0` and nothing else. `tests/register.rs::every_assumption_id_exists_in_assumptions_md` performs the same check.

- [ ] **Step 2b: Run the register tests**

Run: `cargo test --release -p bwr-core --test register 2>&1 | tail -4`
Expected: `test result: FAILED. 7 passed; 1 failed`; the remaining failure is `existing_borrowed_values_are_assessed` with `cannot read` (esbwr-applicability.csv is created in Task 8).

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
- Produces: ids `E-01`..`E-06`. E-02 and E-03 are assessed in this package because the baseline already borrows their values (Step 1b); E-01, E-04, E-05, E-06 stay `pending` and are updated by Packages 5, 6, 8, 9 before those packages borrow anything.

- [ ] **Step 1: Write the header and six rows (E-02 and E-03 already assessed)**

```csv
id,esbwr_source,esbwr_section,target_subsystem,bwrx_evidence,differences,adaptation,applicability_limits,test,classification
E-01,DCD Rev 10 Tier 2;NUREG-1966 V2,Ch 4 (4.3 nuclear design; 4.4 thermal-hydraulic; 4.6 FMCRD),Core fuel and rods,S-06;S-08,Fuel geometry; core size (240 vs 1132 bundles); exposure; FMCRD count and arrangement; insertion response,Borrow feedback mechanisms and kinetics methods and heat-transfer forms and shutdown-margin methodology only; re-derive all numbers for 240-bundle core,No ESBWR worth or coefficient value is a BWRX value,core_transients.rs (Package 5),pending
E-02,DCD Rev 10 Tier 2;NUREG-1966 V2,S-ESBWR-04 Section 4.4 (thermal-hydraulic design; rated core flow and void tables); S-ESBWR-05 Section 5.3 (RPV) and Section 5.4 (chimney and internals),Vessel and circulation,S-06 (natural circulation; chimney; 4 m x 26.7 m envelope),Elevations; areas; inventory; flow paths; pressure losses; chimney partitioning; 240 vs 1132 bundles,CORE_FLOW_RATED 2000 kg/s is the ESBWR rated core flow scaled by thermal power (870 over the ESBWR rating) which the master constraint forbids; kept as a provisional magnitude until Package 4 replaces it with a head-balance derivation on BWRX geometry; inventory/energy balance forms and boiling and separator and natural-circulation formulations are borrowed as concepts only,No ESBWR flow or void number is BWRX evidence; correlation validity must be checked at BWRX flows and voids,vessel_transients.rs (Package 4),provisional-estimate
E-03,DCD Rev 10 Tier 2;NUREG-1966 V2,S-ESBWR-05 Section 5.4.6 (Isolation Condenser System: 4 trains; per-train duty; pool inventory) and S-ESBWR-06 Section 6.2 (containment interfaces),Isolation condensers,S-06 (passive ICs; 7-day claim); S-09; S-10 (ICS mission time),BWRX connections; train count (3 modelled vs 4 ESBWR); duties; initiation logic; pool inventories; no SRVs; S-09 notes IC function and connections differ,IC_TRAIN_RATED_W 33.75e6 W and ic.ua_pool_ref 100 degC are the ESBWR per-train unit size and pool reference state used as a provisional magnitude; condensation and gravity return and pool boil-off and valve/failure concepts are borrowed; BWRX duty and pool mass come from BWRX claims in Package 7,No ESBWR duty or inventory number is BWRX evidence; N_IC_TRAINS stays assumed until a BWRX document states it,ic_transients.rs (Package 7),provisional-estimate
E-04,DCD Rev 10 Tier 2;NUREG-1966 V2,Ch 10 (steam and power conversion) and Ch 7.7 (pressure and level control),Steam and power conversion,S-06,Actual equipment; capacities; control responses; operating points,Borrow pressure regulation and bypass and turbine/condenser thermodynamics and feedwater heating and level control principles,Equipment numbers are BWRX-specific or assumed,power_conversion.rs (Package 6),pending
E-05,DCD Rev 10 Tier 2;NUREG-1966 V2,Ch 7 (I&C; RPS; NMS) and Ch 16 (technical specifications),Protection and instruments,S-06;S-07;S-08,Applicable BWRX signals; architecture; modes; setpoints; actuation sequence; RPV isolation valves replace SRVs,Borrow measurement principles and permissives and latches and redundancy concepts and response testing; no voting architecture claim without BWRX evidence,Do not assume legacy SRM/IRM/APRM or ESBWR SRNM/PRNM is the final BWRX arrangement,protection.rs (Package 8),pending
E-06,DCD Rev 10 Tier 2;NUREG-1966 V2,Ch 14 (initial test program) and Ch 15 (transient and accident analyses),Validation,S-10;S-11;S-12,Reference configuration; initial conditions; credited systems; numerical uncertainty,Borrow component qualification methods and test structure and transient mechanisms; ESBWR traces are mechanism tests not BWRX acceptance curves,No ESBWR numerical trace is a BWRX acceptance curve,scenarios.rs (Package 9),pending
```

- [ ] **Step 1b: Complete the assessments for values already borrowed at baseline**

The spec requires the applicability register to be populated *before* borrowing, and the baseline already borrows three values. Their rows are completed in this package, not left `pending`:

| Borrowed value (parameters.csv) | Row | Chapter rows to read (Task 2 Step 3) | What to confirm and record |
|---|---|---|---|
| CORE_FLOW_RATED | E-02 | S-ESBWR-04 (Section 4.4 and its thermal-hydraulic parameter table); S-ESBWR-05 (Sections 5.3 and 5.4) | the ESBWR rated core flow and rated thermal power printed in the table; write `ESBWR <flow> kg/s at <power> MWt; scaled by 870/<power>` in the CORE_FLOW_RATED notes |
| IC_TRAIN_RATED_W; ic.ua_pool_ref | E-03 | S-ESBWR-05 (Section 5.4.6 and its ICS design parameter table); S-ESBWR-06 (Section 6.2) | the per-train heat removal capacity and the pool reference temperature printed there; write them in the IC_TRAIN_RATED_W and ic.ua_pool_ref notes |

Open each chapter PDF; where the section numbers or table ids printed in the retrieved Rev. 10 PDF differ from those written in `esbwr_section`, replace them and add the printed page in parentheses. `classification` for E-02 and E-03 is `provisional-estimate` (an ESBWR magnitude used until the BWRX derivation exists). If a cited section is redacted in the public PDF, write `redacted in public PDF` in `differences`, keep `provisional-estimate`, and never infer the redacted number. `pending` is reserved for E-01, E-04, E-05, and E-06, whose concepts are not borrowed until Packages 5, 6, 8, and 9. `tests/register.rs::existing_borrowed_values_are_assessed` enforces this.

- [ ] **Step 2: Verify shape and run the register tests**

Run: `awk -F',' 'NF!=10{print NR": "NF" fields"}' docs/model/esbwr-applicability.csv; cargo test --release -p bwr-core --test register 2>&1 | tail -3`
Expected: no awk output; `test result: ok. 8 passed; 0 failed`.

- [ ] **Step 3: Record the reading plan in design-basis.md**

Append to `docs/model/design-basis.md`:

```markdown
## 7. ESBWR reading plan

Before any package borrows an ESBWR concept, the executor reads the DCD Rev. 10 Tier 2 chapter(s) listed for that row in `esbwr-applicability.csv`, reads the referenced tables and figures (not only summaries), fills `bwrx_evidence`, `differences`, `adaptation`, and `applicability_limits` with specific section numbers, and changes `classification` from `pending` to `shared-concept`, `adapted-model`, `provisional-estimate`, or `inapplicable`. Priority order: Ch 4, 5, 6, 7, 9, 10, 14, 15, 16. Each chapter PDF gets its own `sources.csv` row (`S-ESBWR-<chapter>`) with checksum. E-02 and E-03 were assessed in Package 1 because the baseline already borrowed `CORE_FLOW_RATED`, `IC_TRAIN_RATED_W`, and `ic.ua_pool_ref`; `pending` is reserved for concepts not yet borrowed, and `tests/register.rs` fails if a borrowed value's row is `pending`.
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
| Vessel inventory and EOS | vessel.rs; steam.rs; steam_table.rs | S-06; S-STD-05; S-STD-06 | P_RATED; VESSEL_*; Z_*; A_*; V_*; METAL_HEAT_CAP; HEAT_LOSS_HOT; vessel.*; steam.* | A-006; A-016 | vessel::eos_roundtrip; vessel::level_volume_inverse; steam::known_points | 100 degC atmospheric assumption; 1000 kg floor; collapsed vs mixture level; no property reference test beyond 2 points |
| Natural circulation | vessel.rs | S-ESBWR-04; S-ESBWR-05 (E-02) | CORE_FLOW_RATED; LOOP_INERTIA; C0; N_AXIAL; vessel.drift_*; vessel.phi2_form; vessel.spill_band | A-007 | vessel::rated_flow_and_voids_are_plausible | calibrated at one point only; spill cutoff unverified |
| Main steam and pressure control | bop.rs | - | STEAM_*; N_STEAM_LINES; MSIV_STROKE; C_TCV; BYPASS_CAPACITY; C_BYPASS; PRESSURE_*; bop.tcv_*; bop.bypass_*; bop.p_ref_* | A-008; A-009 | plant::raising_pressure_raises_power; plant::msiv_closure_scrams_and_isolation_condensers_cool | averaged MSIV positions; no TSV; no line storage |
| Turbine-generator | bop.rs | - | TURBINE_DH_RATED; GENERATOR_EFF; RATED_RPM; GRID_HZ; TG_*; OVERSPEED_TRIP_RPM; SPEED_RAMP; bop.governor_*; bop.sync_*; bop.turbine_* | A-010 | plant::turbine_trip_rides_through_on_bypass; plant::startup_from_hot_standby_to_30_percent | no energy balance reconciliation; extraction not debited from shaft |
| Condenser | bop.rs | - | CW_*; N_CW_PUMPS; CONDENSER_*; AIR_INLEAKAGE; VACUUM_PUMP_TAU; HOTWELL_CAPACITY; bop.condenser_*; bop.hotwell_* | A-011; A-012 | none directly | unaccounted makeup/reject; no vacuum-loss test |
| Feedwater | bop.rs | - | N_FEED_PUMPS; N_COND_PUMPS; FW_*; CONDENSATE_PRESSURE; C_FCV; bop.fcv_*; bop.fw_*; bop.level_* | A-012 | plant::full_power_is_steady (indirect) | no pump-trip or controller-transfer test |
| Isolation condensers | ic.rs | S-ESBWR-05 (E-03); S-09; S-10 | N_IC_TRAINS; IC_*; ic.* | A-013 | plant::msiv_closure_scrams_and_isolation_condensers_cool | energy overshoot and depletion discard; single valve; no train-availability tests; no 72 h / 7 d run |
| Protection and modes | sim.rs | S-13 (interval bounds only) | SCRAM_HIGH_PRESSURE; IC_HIGH_PRESSURE; VESSEL_DESIGN_PRESSURE; APRM_*; IRM_HIGH_HIGH; MSIV_LOW_PRESSURE_RUN; L*_CM; sim.* | A-014; A-017 | plant::manual_scram_shuts_down_without_isolation; plant::cold_shutdown_is_subcritical_and_rods_blocked | REFUEL interlock uses actual positions only; L8 feed-pump trip inside turbine-not-tripped branch; no cause-and-effect matrix |
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

- [ ] **Step 4: Prove the acceptance gate (hard barrier for Package 3)**

The master dependency table makes Package 3 depend on **both** Package 1 and Package 2 acceptance gates: no physics change is made before the register exists and is enforced. The proof that Package 1's gate is passed is exactly this command on the committed tree:

Run: `git status --porcelain docs/model crates/bwr-core/tests/register.rs && cargo test --release -p bwr-core --test register 2>&1 | tail -3`
Expected: the `git status` line prints nothing (everything committed) and the test output ends with `test result: ok. 8 passed; 0 failed; 0 ignored`.

Record the commit hash that produced this output in `docs/model/coverage.md` under a final line `Package 1 acceptance: <hash> on <date>`; Package 3's executor checks that line and re-runs the command before starting.

```bash
git add docs/model/coverage.md
git commit -m "docs(model): record Package 1 acceptance gate

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

## Acceptance gate (verbatim from the spec)

"A reviewer can trace every consequential behavior to evidence or an explicit assumption. Public descriptions, accepted methods, and confirmed numerical plant data are distinguished."

This gate is a hard barrier: Package 3 (and everything after it) may not start until it passes together with Package 2's gate.

Checklist for the reviewer:

- [ ] `cargo test --release -p bwr-core --test register` passes (8 tests) on the committed tree, and `coverage.md` records the acceptance hash. The tree cannot pass with `sources.csv`, `assumptions.md`, or `esbwr-applicability.csv` absent.
- [ ] `register.rs` looks up every column by header name (`Register::col`), never by index.
- [ ] Every chapter-level source (`S-CRN-*`, `S-ESBWR-*`, `S-UK-*`) has a direct PDF URL and checksum, or an explicit `not-public` note.
- [ ] No `UNRETRIEVED` remains in a non-index row of `sources.csv`; every PDF row has a 64-character `sha256`.
- [ ] Every `assumed`/`esbwr-estimate` row in `parameters.csv` cites an `A-nnn` that exists in `assumptions.md`.
- [ ] No row was promoted to `confirmed` without a `source_ids` entry whose `sources.csv` row has a section and checksum; every `confirmed` row's `notes` begins with the source id and printed location (Task 2 Step 4b), and any row the evidence did not support carries `downgraded from confirmed:` (Task 4 Step 1b).
- [ ] Every numeric interval on an `assumed`/`esbwr-estimate`/`unavailable` row is named in an `Interval basis` entry of `assumptions.md` with its basis kind; every other such row has `none`/`none` and `no defensible interval` in `notes`.
- [ ] The REFUEL `out > 1` actual-position rule, the L8 feed-pump trip placement, both hot-standby feedwater temperatures (180 degC to `Vessel::new`; 150 degC to `Feedwater::new`), and the mock's 7.8 MPa / -150 cm thresholds have rows, and the Task 5/6 reconciliation steps recorded their appended-row counts.
- [ ] `esbwr-applicability.csv` rows E-02 and E-03 are `provisional-estimate` with `S-ESBWR-*` sections and printed pages; E-01, E-04, E-05, E-06 remain `pending` unless the named DCD chapter was read and its `S-ESBWR-*` source row exists.
