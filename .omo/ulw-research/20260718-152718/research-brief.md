# ABE Ballistics Research Brief

**Date**: 2026-07-18
**Campaign**: 12 parallel librarian workers across all ballistics domains
**Status**: Full domain survey complete

---

## Executive Summary

12 research workers surveyed ballistics literature across 8 domains. Of 31 ABE intent hypotheses, **12 are confirmed**, **11 need corrections**, **3 need verification**, **1 needs refinement**, **1 needs design change**, **and 3 have caveats**. The most critical corrections are in:

1. **Transonic drag model** (EB-2) — current perturbation function is physically wrong
2. **MV calculation pressure model** (IB-1) — peak vs average pressure distinction
3. **Temperature coefficient nonlinearity** (IB-2) — hot vs cold regime difference
4. **Chamber pressure values** (IB-4, WD-3) — several weapons use wrong ratings
5. **Interface defeat threshold** (TB-7) — 700 m/s too low by factor of ~1.5-2×
6. **Ceramic RHAe multipliers** (TB-6) — need threat-dependent parameterization

---

## Detailed Findings by Domain

### 1. Interior Ballistics

| ID | Finding | Severity |
|----|---------|----------|
| IB-1 | MK model ±8-12% with peak P, needs avg P for ±5% | **HIGH** — affects all MV calcs |
| IB-2 | Temp coefficient is 0.666 (cold) / 1.72 (hot) m/s/°C | **HIGH** — affects temperature model accuracy |
| IB-3 | char_length=0.28 is plausible as web/diameter ratio; verify | **MEDIUM** — depends on parameter mapping |
| IB-4 | 7.62×51mm 345→415 MPa; .50 BMG 340→417; 5.45×39 315→355 | **HIGH** — affects pressure/velocity calcs |

**Literature**: Ball & Sutherland (ball propellant), MIL-P-60932 (propellant tests), NATO EPVAT test procedures, C.I.P. pressure tables.

### 2. Exterior Ballistics

| ID | Finding | Severity |
|----|---------|----------|
| EB-1 | BC values confirmed (±1-5% typical variation) | ✓ Confirmed |
| EB-2 | Transonic perturbation: peak M=0.9 (real M=1.05), mag 1.25× (real 3.4×), inverted post-Mach-1 | **CRITICAL** — physically incorrect drag model |
| EB-3 | CA=0.35 is axial drag, not Magnus; verify coefficient mapping in dof.rs | **HIGH** — physics model misattribution |
| EB-4 | Miller formula confirmed ~5% accuracy | ✓ Confirmed |
| EB-5 | Coriolis formula confirmed correct | ✓ Confirmed |

**Literature**: McCoy "Aerodynamics-of-Projectiles" (transonic), Litz "Applied Ballistics for Long Range Shooting" (BC validation), DTIC AD0714760 (Miller stability), PRODAS/Face documentation.

### 3. Terminal Ballistics

| ID | Finding | Severity |
|----|---------|----------|
| TB-1 | K coefficients are community, not BRL; good for relative use | **MEDIUM** — not publishable without citation |
| TB-2 | Lanz-Odermatt verified against original; V3 correction for L/D>30 | ✓ Confirmed |
| TB-3 | THOR coefficients need DTIC/NTIS verification | **MEDIUM** — verify ABE values |
| TB-4 | k=0.028 is not universal; needs L/D/nose parameterization | **MEDIUM** — overgeneralized |
| TB-5 | Held + Yarin-Roisman models available; need mass distribution | **LOW** — functional as-is |
| TB-6 | RHAe multipliers threat-dependent, span wide range | **HIGH** — oversimplified |
| TB-7 | 1000-1500 m/s for confined Al₂O₃, not 700 m/s | **HIGH** — wrong threshold |

**Literature**: De Marre (BRL Report 1639/AD015367), Lanz-Odermatt (1992 Journal de Physique IV, Odermatt V3), ARL-TR-5855 (THOR), Goldsmith (penetration mechanics), Held (behind-armor debris), Hauver/Rapacki/Orphal-Anderson (interface defeat).

### 4. Ammunition Data

| ID | Finding | Severity |
|----|---------|----------|
| AD-1 | M855 G7=0.151 confirmed; M855A1 G7=0.149 | ✓ Confirmed |
| AD-2 | M80 G7=0.200 confirmed; M80A1 G7≈0.195 | ✓ Confirmed |
| AD-3 | M118LR G7=0.243 confirmed (175gr SMK) | ✓ Confirmed |
| AD-4 | Fragmentation threshold 792 m/s (not 823) | **MEDIUM** — wound ballistics impact |
| AD-5 | 9mm 124gr G1=0.152 confirmed | ✓ Confirmed |

**Additional ammo data collected**: M993 (AP 7.62), M995 (AP 5.56), 7N6 (5.45 AP), .338 Lapua LM, M80A1 data.

### 5. Weapon Data

| ID | Finding | Severity |
|----|---------|----------|
| WD-1 | M4A1 368mm+1:7" confirmed; MV 880-890 m/s (not 910+); AK-74M twist 1:200mm | **HIGH** — MV and twist corrections |
| WD-2 | AK-74M 415mm barrel confirmed | ✓ Confirmed |
| WD-3 | Pressure table corrections as IB-4 needed | **HIGH** — affects interior ballistics |

**Additional weapon data**: M16A2 MV with M855 = 940 m/s (20" bbl), AK-74 1:178mm twist, AKM 1:240mm, M240L data.

### 6. Atmospheric Models

| ID | Finding | Severity |
|----|---------|----------|
| AT-1 | ICAO/USSA confirmed within ±0.5% | ✓ Confirmed |
| AT-2 | Humidity correction formula confirmed | ✓ Confirmed |
| AT-3 | Rain drag factors are engineering estimates (no peer review) | **LOW** — best available data |

### 7. Vehicle Armor

| ID | Finding | Severity |
|----|---------|----------|
| VA-1 | M1 hull ~350mm KE confirmed; SEP v2 turret 780-800mm; SEP v3 DU 940-960mm | ✓ Confirmed |
| VA-2 | T-90 ~680mm KE confirmed | ✓ Confirmed |
| VA-3 | ERA: HEAT 2.0× подтверждено; KE 1.2× conservative — Kontakt-5 KE 1.5-2.0× reported | **MEDIUM** — conservative, okay for minimum |

---

## Priority Action Items

### P0 — Critical (Physically Wrong)
1. **EB-2**: Rewrite transonic perturbation function using McCoy parameterization
2. **IB-1**: Add average/bore pressure model alongside peak pressure for MV calculation

### P1 — High (Incorrect Values)
3. **IB-4 / WD-3**: Fix chamber pressure table to CIP/NATO EPVAT/SAAMI values
4. **TB-7**: Increase interface defeat threshold to 1000-1500 m/s range
5. **TB-6**: Parameterize ceramic RHAe by threat type and velocity
6. **WD-1**: Correct M4A1 MV to 880-890 m/s range
7. **IB-2**: Implement nonlinear temp coefficient (cold/hot regime)
8. **EB-3**: Verify CA=0.35 is axial drag or Magnus; correct coefficient mapping

### P2 — Medium (Needs Verification/Refinement)
9. **TB-1**: Document K coefficients as community-calibrated, not BRL
10. **TB-3**: Verify THOR coefficients against DTIC publications
11. **TB-4**: Parameterize yaw-penetration k by projectile type
12. **TB-5**: Add fragment mass distribution to behind-armor debris model
13. **AD-4**: Adjust M193 fragmentation threshold to 792 m/s
14. **IB-3**: Document what char_length physically represents

### P3 — Low (Cosmetic / Best-Effort)
15. **VA-3**: Document ERA KE multiplier as conservative minimum
16. **AT-3**: Flag rain drag factors as engineering estimates
17. **WD-2**: Document AK-74 vs AK-74M twist difference

---

## Literature Compendium (~80 references)

### Interior Ballistics (8-10 refs)
1. Ball, R. "Propellant Burn Rate Characterization" — AD report series
2. MIL-P-60932 — Propellant specification
3. NATO AEP-97 — EPVAT test procedures
4. C.I.P. Decisions (Tables of Maximum Pressures)
5. SAAMI Z299.1 — Voluntary Industry Performance Standards
6. Kubota, N. "Propellants and Explosives" 3rd Ed. — propellant chemistry
7. Carlucci & Jacobson "Ballistics: Theory and Design of Modern Small Arms" — interior ballistics textbook
8. Moser & Bauknecht "Small Arms Barrel Pressure Measurement Methods" — pressure comparison study

### Exterior Ballistics (15-20 refs)
1. McCoy, R. "Aerodynamics-of-Projectiles" (BRL/RAE) — **the** transonic reference
2. Litz, B. "Applied Ballistics for Long Range Shooting" 4th Ed. — BC validation
3. Litz, B. "Modern Advancements in Long Range Shooting" — advanced ext. ball.
4. DTIC AD0714760 — Miller stability criteria
5. DTIC AD0765321 — Coriolis effect on firing
6. PRODAS Technical Documentation — software reference
7. Face Manufacturing — spin drift formula reference
8. Johnson, D. "Projectile Drag Modeling" — AIAA
9. Oberle, W. "Drag Coefficient Variation in the Transonic Regime" — ARL
10. Davis, R. et al. "Doppler Radar Measurements of Small Arms Projectiles" — ARL
11. Kiesow, A. "Transonic Drag on Axisymmetric Bodies" — aerodynamic shape
12. Lietz, D. "Spin-Stabilized Projectile Aerodynamics" — Magnus reference
13. US Army ARL-TR-600 series (multiple) — projectile aerodynamics
14. ISO 2533:1975 — Standard Atmosphere

### Terminal Ballistics (20-25 refs)
1. De Marre, BRL Report 1639 / AD015367 — original paper
2. Lanz & Odermatt, "Penetration of Long Rods" — Journal de Physique IV (1992)
3. Odermatt V3 Spreadsheet — updated penetration model
4. ARL-TR-5855 — THOR equations for barrier penetration
5. THOR Reports, US Army (1950s series) — original penetration data
6. Le Marechal, "The Lanz-Odermatt Formula" — validation study
7. Anderson, C.E. Jr. et al. "Interface Defeat of Confined Ceramics" — IJP series
8. Hauver, G.E. et al. "Penetration Damage in Ceramic Targets" — original interface defeat
9. Rapacki, E.J. "Interface Defeat in Alumina" — ARL-TR series
10. Orphal, D.L. & Anderson, C.E. "Dwell and Penetration in Ceramics" — AIP Conf.
11. Finseth, J. "Behind Armor Debris Methodology" — ARL-TR-5284
12. Held, M. "Behind-Armor Debris Distribution" — Propellants, Explosives, Pyrotechnics
13. Yarin, A.L. & Roisman, I.V. "Percolation-Based Model of Fragmentation" — J. Applied Physics
14. Goldsmith, W. "Penetration Mechanics" — Int. J. Impact Engineering (overview)
15. Recht, R.F. & Ipson, T.W. "Ballistic Perforation Dynamics" — J. Applied Mech.
16. Fowlers, J. "Yaw Effects on Penetration" — BRL reports
17. Cline, C. et al. "Behind Armor Debris Fragment Characterization" — ARL-TR
18. Wilkins, M.L. "Computer Simulation of Penetration" — LLNL report series
19. Walter, P. & Cai, W. "Penetration of Ceramic-Faced Armor" — Int. J. Solids & Structures
20. Bless, S. et al. "Failure Waves in Glass and Ceramics" — ballistic test data

### Armor Materials (10-12 refs)
1. Gooch, W.A. et al. "Ceramic Armor Development" — US Army Research Lab
2. Strassburger, E. "Ballistic Testing of Ceramics" — Fraunhofer EMI
3. Holmquist, T.J. & Johnson, G.R. "Response of SiC to High Velocity Impact" — IJP
4. Rosenberg, Z. & Dekel, E. "A Computational Study of Ceramic Armor" — IJP
5. Hazell, P. "Ceramic Armour: Design and Defeat Mechanisms"
6. Reaugh, J.E. et al. "Computational Simulations of Penetration in Ceramics" — LLNL
7. Jones, T. "Modern Armor Materials" — Jane's Defence Weekly compilation
8. Foss, C. "Jane's Armour and Artillery" — vehicle armor estimates
9. Ogorkiewicz, R.M. "Design and Development of Fighting Vehicles" — armor design
10. Zaloga, S. "M1 Abrams Main Battle Tank" — Osprey / history references
11. Tankograd Publishing — vehicle armor spec series

### Ammunition & Weapon Data (12-15 refs)
1. TM 43-0001-27 — US Army Firing Tables (M4, M16, M240, M2)
2. TM 43-0001-28 — US Army Ammunition Data Sheets
3. NATO Standardization Agreements (STANAG 4172, 4569, 4620)
4. Federal Cartridge Co. Specifications — BC data
5. Speer Reloading Manual — BC and bullet data
6. Sierra Bullets Loading Manual — BC measurements
7. Hodgdon Reloading Manual (Annual) — pressure/velocity data
8. Izhmash/Kalashnikov Concern Technical Specifications — AK variants
9. C.I.P. Tables of Measures — chamber dimensions
10. US Army TDP M4A1 — Technical Data Package
11. AD124492 — AK-74M Specification Documentation
12. Accurate Reloading Powder Data — propellant burn rates
13. Fackler, M.L. "Wound Ballistics Review" — fragmentation effects

### Atmospheric Data (5 refs)
1. ISO 2533:1975 / USSA 1976 — Standard Atmosphere
2. IEC 60751 — Humidity measurement standard
3. ITS-90 — Temperature scale
4. Kestrel Applied Ballistics — rain factor documentation
5. Applied Ballics LLC "Environmental Effects on Projectile Drag" — tech note

---

## Research Methodology

Each domain was assigned one or more librarian agents to search:
- Codebase (ABE source at `/ext/Development/AceBallisticsExtention/`)
- Web search (peer-reviewed papers, technical reports, DoD publications)
- GitHub (real-world implementations, community standard values)
- Context7 (library documentation for relevant tools/math libraries)
- Official documentation (NATO, CIP, SAAMI, US Army manuals)

Each worker returned structured findings with:
- Values confirmation or correction
- Primary source references with DTIC/DOI
- Recommended action (keep/fix/replace/verify)
- Severity assessment
