# Wave 2 Research Synthesis — 16 Parallel Librarian Agents

Generated: 2026-07-18
Sources: 16 parallel librarian agents across 8 ballistics domains

## 1. US Army Ballistics Manuals
**Agent bg_5a936da0** — 49 verified documents found

### Key Documents for ABE Validation

| Document | What It Contains | URL |
|----------|-----------------|-----|
| TM 43-0001-27 (1994+C1-8) | Army Ammo Data Sheets, all small calibers | ciehub.info/ref/TM/43-0001-27_1994_C1-8.pdf |
| FM 6-40 (1996) Ch.3 | Interior/transitional/exterior/dispersion ballistics | man.fas.org/dod-101/sys/land/docs/fm6-40-ch3.htm |
| AMCP 706-140 (Exterior Ballistics) | G1-G8 drag tables, trajectory methodology | apps.dtic.mil/sti/tr/pdf/AD0830264.pdf |
| AMCP 706-150 (Interior Ballistics) | Pressure/temperature models, propellant burn | apps.dtic.mil/sti/tr/pdf/AD0462060.pdf |
| ARBRL-TR-02293 (McCoy McDrag) | Drag coeff estimation FORTRAN, ±3% supersonic | apps.dtic.mil/sti/tr/pdf/ADA098110.pdf |
| BRL Report 1532 | Compendium of ballistic properties, L/D 3.5-20 | apps.dtic.mil/sti/pdfs/AD0882117.pdf |
| BRL-MR-3960 | KE terminal ballistics test guidelines | apps.dtic.mil/sti/pdfs/ADA246922.pdf |
| ARL-TR-5540 | M855 accuracy/jump measurements | apps.dtic.mil/sti/tr/pdf/ADA542434.pdf |
| ARSCD-TR-79001 | Interior ballistics modeling small arms | archive.org/stream/DTIC_ADA076175 |
| ADA953510 | De Marre vs empirical limit velocity (n~1.25) | apps.dtic.mil/sti/html/tr/ADA953510/ |
| AD0815788 | 7.62mm NATO aero (M59/M80/M61/M62) | apps.dtic.mil/sti/tr/pdf/AD0815788.pdf |
| ADA219106 | .50 cal M33/M8/M20 aero characterization | NTIS via DTIC |

### Key Finding: ICAO vs Army Standard Metro
ICAO: 29.9213 inHg, dry, ρ₀ = 1.225 kg/m³
Army Standard Metro: 29.5275 inHg, 78% RH, ρ₀ = 0.0751265 lb/ft³
Conversion: BC_ICAO ≈ BC_Metro × 1.018

---

## 2. British Army / NATO Ballistics
**Agent bg_c24a4b7c** — UK MOD, DSTL, UK proof data

### Critical Finding: NATO Standard Temperature
**NATO STANAG 4044 uses 15°C (ICAO/ISA)**
**NATO EPVAT testing uses 21°C (70°F) for service pressure reference**
ABE's 21°C standard may reflect US convention (70°F), not ISA standard (15°C)

### UK Service Ammunition MV

| Weapon | Ammo | MV | Source |
|--------|------|-----|--------|
| L85A2 (5.56mm) | L1A2 ball | 930-940 m/s | MOD |
| L1A1/L7 (7.62mm) | L2A2 ball | 838 m/s | MOD |
| L96A1 (7.62mm) | L42A1 | 838 m/s | MOD fact sheet |
| L115A3 (.338) | Lapua Scenar 250gr | **936 m/s** | MOD official |
| L115A1 (.338) | Lapua Scenar 250gr | ~900 m/s | Various |

### Drag Model Note
UK NABIS adopted **G7 drag models** for L42A1 .338 Lapua rounds. G1 systematically overpredicts drag beyond ~400m for spitzer boat-tail bullets.

### Key Documents
- AEP-55 Vol.1 (KE/Artillery) — englands1.com/site/wp-content/uploads/AEP-55.pdf
- DefStan 13-33/3 (7.62mm NATO) — fcmls.org.uk
- AEP-97 M-CMOPI — diweb.hq.nato.int/naag/Public%20Release%20Documents/
- STANAG 4569 Ed.4 (Feb 2022) — standards.globalspec.com

---

## 3. Modern Ballistics Research Papers
**Agent bg_6ed6a14a** — Academic literature across 12 domains

### Critical Academic Sources

| Author(s) | Year | Title | DOI / Link |
|-----------|------|-------|------------|
| Litz, B. | 2023 | Applied Ballistics for Long Range Shooting (4th Ed) | ISBN 978-0-9909206-6-3 |
| Litz, B. | 2025 | The Evolution of Ballistic Calibration (CDF) | appliedballisticsllc.com |
| Carlucci & Jacobson | 2025 | Ballistics: Theory and Design (4th Ed) | 10.1201/9781003560661 |
| Lundberg et al. | 2000 | Transition between interface defeat and penetration (SiC/B₄C) | 10.1016/S0734-743X(99)00152-9 |
| Behner, Wickert et al. | 2016 | Dwell and penetration of WHA rods impacting SiC | 10.1016/j.ijimpeng.2016.04.008 |
| Anderson & Walker | 2004 | Analytical model for dwell and interface defeat | 10.1016/j.ijimpeng.2004.07.013 |
| Wei et al. | 2024 | Held distribution best for BAD mass distribution | 10.1088/1742-6596/2891/5/052007 |
| Qi, Zu, Huang | 2025 | Review of development and key technologies of reactive armor | 10.1134/S0025654425601612 |
| Odermatt, W. | 2009 | Odermatt Equation V3 (full coefficients) | N/A (proprietary) |
| Ćatović | 2024 | Influence of explosive type on Mott's stochastic model | 10.1177/15485129241289136 |
| Chiriac et al. | 2023 | Mott distribution for steel coaxial cylinders | 10.3390/ma16175783 |
| Kong, Li, Fang | 2016 | Critical impact yaw for long-rod penetrators | 10.1115/1.4034620 |
| Vayig & Rosenberg | 2020 | Effect of yaw on rigid rod penetration | 10.1016/j.ijimpeng.2020.103748 |
| Boulkadid et al. | 2015 | Temperature sensitivity of propellants (5.56mm NATO) | 10.17265/1934-7375/2015.06.005 |
| Fu, Zhu et al. | 2021 | Temperature sensitivity of RDX-based propellants | 10.1002/prep.202100095 |
| Becker et al. | 2022 | Data-driven prediction of ERA plate velocities | 10.1016/j.dt.2022.07.001 |
| Moldtmann et al. | 2024 | Adaptive optimisation of ERA for KE/CE threats | 10.1016/j.dt.2024.05.007 |

### Key Numerical Values

**Interface Defeat Thresholds (bare SiC)**:
- Unconfined bare SiC: ~900 m/s (Behner 2016)
- Buffered SiC (Cu disc ~0.5D): ~1700 m/s (Behner 2016)
- Confined SiC, 372-899 MPa pre-stress: ~1200 m/s (Çellek 2026)
- Cover plate + confinement: higher (Sun 2020)

**Lanz-Odermatt V3 Coefficients**:
- WHA: a=0.921, c0=138, c1=-0.10
- DU: a=0.825, c0=90.0, c1=-0.0849
- L/D val: L/D 4-36, v 1.0-2.3 km/s, BHN 250-470

**Transonic Cd Change**: ~70% from supersonic to subsonic
**G7 BC variation**: 15% across velocity range for typical boattail
**CDM eliminates**: 12% variance at Mach 1 vs G7

---

## 4. Kestrel/Applied Ballistics Solver Data
**Agent bg_6d37924c** — Commercial solver methodology

### Solver Types Comparison

| Solver | Method | Drag Models | Transonic | Accuracy |
|--------|--------|-------------|-----------|----------|
| AB Point Mass | RK4, 1000Hz | G1, G7, CDM/PDM | Native via CDM | 0.2 mil @ 1000m |
| Hornady 4DOF | Modified PM + yaw | Cd (Doppler), G1/G7 | Native via Cd curve | Very high |
| Lapua 6DOF | Full 6-DOF | Cd tables per bullet | Native | Highest |
| Sierra Infinity | Siacci/PM | G1 only | Piecewise | Limited |
| JBM Ballistics | PM/MPM | G1-G8, GI, GL, RA4 | Variable | Good |
| Patagonia ColdBore | Kalman filter | G1/G7 + DC function | Continuous | Proprietary |

### CDM/PDM Methodology
- CDM: Doppler radar measured Cd(Mach) — eliminates BC concept
- Accuracy: ±1% per shot
- Transonic: 0.2 mil error at 1323yd (un-trued)
- PDM: Same methodology, user's own rifle

### Atmospheric Standards
- ICAO: 15°C, 1013.25 hPa, dry (solver default)
- ASM: 15°C, 29.5275 inHg, 78% RH
- Error from wrong standard: ~3% density

---

## 5. Modern Ammo Specifications
**Agent bg_a538ca09** — Military and commercial ammunition data

### Military Ammo Key Values

| Cartridge | Projectile | MV | Source Barrel | Penetration |
|-----------|-----------|-----|---------------|-------------|
| M855 (5.56mm) | 62gr SS109 | 922 m/s | 20" | — |
| M855A1 | 62gr lead-free | 961 m/s | 20" | 9.5mm MS at 350m |
| M80 (7.62mm) | 144gr | 833 m/s | 22" EPVAT | — |
| M993 (7.62mm AP) | 126.6gr WC | 910 m/s | — | 18mm RHA at 100m |
| M995 (5.56mm AP) | 52gr WC | 1013 m/s | — | 12mm RHA at 100m |
| MK211 (.50) | 665gr multipurpose | 903 m/s | 24" | 11mm RHA at 45° at 1000m |
| M118LR (7.62mm) | 175gr SMK | 786 m/s | 24" | — |
| Mk316 Mod 0 | 175gr SMK (improved) | 786 m/s | 24" | SD ≤ 15 fps |

### M829 Series (120mm APFSDS)

| Variant | Year | MV | Penetrator | Pen (est RHA @2km) |
|---------|------|-----|------------|-------------------|
| M829 | 1988 | 1670 m/s | DU, L/D 23:1 | ~550mm |
| M829A1 | 1988 | 1575 m/s | DU, L/D 35:1 | ~650mm |
| M829A2 | 1994 | ~1675 m/s | DU improved | ~700mm |
| M829A3 | 2003 | 1555 m/s | Steel tip + DU, 31:1 | ~700mm |
| M829A4 | 2016 | ~1650 m/s | Enhanced DU | ~750mm+ |

### SAAMI MAP Values (key)
- 5.56mm NATO: ~62,000 psi (430 MPa) EPVAT
- 7.62mm NATO: ~60,000 psi (415 MPa) EPVAT
- .308 Win: 62,000 psi (SAAMI)
- .338 Lapua: 65,000 psi (SAAMI)
- .50 BMG: 55,000 psi
- .300 PRC: 65,000 psi (SAAMI 2025 new)
- 7mm Backcountry: 80,000 psi (SAAMI 2025 new)

### Lapua .338 BC Values

| Bullet | Weight | G1 BC | G7 BC (>830 m/s) |
|--------|--------|-------|--------------------|
| Scenar | 250gr | 0.646 | 0.320 |
| Lock Base | 250gr | 0.621 | 0.310 |
| Scenar | 300gr | 0.745 | 0.368 |
| AP | 248gr | 0.564 | 0.289 |

### Barrel Length vs MV
- 5.56mm: ~22-25 fps/in in 16-22" range
- .308: ~20-30 fps/in in 16-26" range
- .338 Lapua: ~26-30 fps/in in 17-30" range
- General: 20-30 fps/in for rifle cartridges 16-22"

### Temperature Sensitivity
- Military spec (MIL-C-46931F): +150/-250 fps from 70°F at +125°F/-65°F
- Hodgdon Extreme: 0.4-0.7 m/s/°C
- Standard powders: 0.8-1.5 m/s/°C
- Ball powders (Winchester/Ramshot): 1.2-2.0 m/s/°C
- Mk316 Mod 0: 10x improvement over M118LR (21 fps vs 227 fps across -25°F to +165°F)

### Lot Variation
- FGMM 168gr: SD 14 fps
- FGMM 175gr: SD 20.6 fps
- Black Hills 175gr: SD 14 fps
- M118 legacy: SD 28 fps
- Mk316 Mod 0 spec: SD ≤ 15 fps

---

## 6. Hunter/Competitive Ballistics Data
**Agent bg_6d7b9bdd** — PRS, ELR, sniper field data

### Transonic Cd Profile — Lapua 300gr Scenar (.338)
Complete Doppler radar table:

| Mach | Cd | Regime |
|------|-----|--------|
| 1.200 | 0.348 | Upper transonic |
| 1.150 | 0.348 | Peak shockwave |
| 1.075 | 0.345 | Max overturning moment |
| 1.000 | 0.306 | Shockwave dissipation |
| 0.975 | 0.236 | Rapid drag reduction |
| 0.950 | 0.177 | Wake shedding |
| 0.900 | 0.142 | Subsonic approach |
| 0.850 | 0.137 | Minimum Cd trough |
| 0.800 | 0.144 | Stable base drag |

### ELR Velocity Retention

| Distance | .50 BMG 750gr | .408 CheyTac | .375 CheyTac | .375 EnABELR |
|----------|--------------|-------------|--------------|--------------|
| Muzzle | 2820 fps | 2850 | 2950 | 2900 |
| 1000y | 1960 | 2280 | 2460 | 2410 |
| 2000y | 1280 | 1780 | 2010 | 1960 |
| 2500y | 1050 | 1560 | 1800 | 1750 |

### Spin Drift / Coriolis Field Data

| Effect | 1000yd | Source |
|--------|--------|--------|
| Spin drift (RH twist) | ~9" right | Litz |
| Coriolis horizontal (45°N) | 2.5-3" right | Litz |
| Coriolis vertical (E fire) | 2.5-3" high | Litz |
| Combined (RH, NH) | ~11.5" right | Litz |

### Harrison 2475m Shot Reconstruction
- Hornady 4DOF: total aim-off ~3.68m lateral, ~21.03m vertical
- Within 5% of Harrison's debrief

### Field Truing Example
6.5 Creedmoor 140gr ELD-M @ 2669 fps, G7 BC 0.312:
- 800yd: AB 6.31 mil → actual 6.0 mil (-0.31 mil error)
- Updated BC to 0.326 → error resolved

---

## 7. NATO Standardization Docs
**Agent bg_cbe1b040** — STANAGs and AEPs

### STANAG Pressure Specs

| STANAG | Caliber | Pmax (MPa) | Proof (MPa) |
|--------|---------|------------|-------------|
| 4172 | 5.56×45mm | 430.00 | 537.50 |
| 2310 | 7.62×51mm | 415.00 | 519.00 |
| 4383 | 12.7×99mm | 417.00 | 521.30 |
| 4090 | 9×19mm | 252.00 | 315.00 |

### STANAG 4569 KE Protection Levels

| Level | Threat | Velocity | Range |
|-------|--------|----------|-------|
| I | 7.62mm M80 Ball | 833 m/s | 30m |
| I | 5.56mm SS109 | 900 m/s | 30m |
| II | 7.62×39 API | 695 m/s | 30m |
| III | 7.62mm AP (WC) | 930 m/s | 30m |
| IV | 14.5×114 API | 911 m/s | 200m |
| V | 25mm APDS-T | 1258 m/s | 500m |
| VI | 30mm APFSDS | — | 500m |

### Corrections to ABE Assumptions
- **STANAG 4626**: Modular avionics, NOT small arms ammo interchangeability
- **NATO temp**: 21°C for EPVAT service pressure, 15°C for ISA ballistics
- **AEP-55** Vol.1 (KE/Artillery) available expired but not current

---

## 8. Exterior Ballistics Math
**Agent bg_22309133** — ballistics-engine crate reference

### ICAO Atmosphere (7 layers to 84 km)
Sea level: 288.15K, 101325 Pa, 1.225 kg/m³
Troposphere lapse: -6.5 K/km
Tropopause: 216.65K at 11km (isothermal)

### Coriolis Vector
Ω_earth = 7.2921159e-5 rad/s
Ω_x = ω·cos(lat)·cos(az)
Ω_y = ω·sin(lat)
Ω_z = -ω·cos(lat)·sin(az)

### Magnus C_Nα vs Mach
| Regime | Value |
|--------|-------|
| Subsonic (M < 0.8) | 0.030 |
| Transonic (0.8 ≤ M < 1.2) | 0.030 → 0.015 linear |
| Supersonic (M ≥ 1.2) | 0.015 + 0.0044·min((M-1.2)/1.8, 1) |

### Litz Spin Drift (post-process)
drift_inches = 1.25 × (Sg + 1.2) × TOF^1.83
Exponent 1.83, coeff 1.25

### Miller Stability
Sg = 30 × m_gr × (v/2800)^(1/3) × (T/T0) × (P0/P) / (t_cal² × d_in³ × L_cal × (1 + L_cal²))

---

## 9. Penetration/Armour Science
**Agent bg_f114af4e** — Ceramic, ERA, De Marre, L-O validation

### RHAe Multipliers (Validated)

| Ceramic | Standalone | ESAPI array | Source |
|---------|-----------|-------------|--------|
| Al₂O₃ | 2.2-2.8 | ~3.0 | Anderson & Royal ARL |
| SiC | 3.0-4.5 | ~4.0 | Lundberg 2004 |
| B₄C | 4.0-5.5 | ~5.5 | Lundberg, MIL-DTL-32383 |

### Lanz-Odermatt Validation
ABE values are correct per literature:
- k=2.0 (RHA), v_min=700 (RHA), L/D 4-36
- V₃ factor for L/D > 30 per Odermatt 2001

### De Marre Constants
- Ball/FMJ: 91,000 (classical)
- AP: 70,000 (23% lower)
- APDS/APFSDS: 50,500
- HEAT: 100,000

### ERA Effectiveness
| ERA | KE reduction | CE reduction |
|-----|-------------|-------------|
| Kontakt-1 | ~20-30% | ~50-60% |
| Kontakt-5 | ~50-60% | ~80-90% |
| Relikt | ~50%+ | ~90%+ |
| ARAT-2 | ~40-50% | ~70-80% |

### Issue: ABE era_interaction same curve for KE/HEAT
Should split: KE max ~0.50-0.60, HEAT max ~0.80

---

## 10. Weapons Specs
**Agent bg_c6ecd832** — 14 weapon systems verified

### Key Corrections
- **M249**: MV should be 915 m/s (not 925 m/s)
- **M40A5**: MV should be 777 m/s (not 790 m/s)
- **SVD twist**: post-1975 = 1:9.4" (not 1:12.6")

### Complete Verification
All ABE values within acceptable tolerance for:
M4/M4A1, M16A4, M249, M240B, M2HB, M110, M24, M40, L85A2, L115A3, L129A1, AK-74M, PKM, SVD

---

## 11. Propellant Science
**Agent bg_fd1068e9** — Interior ballistics data

### Key Values for ABE Validation
- Average/Peak pressure ratio: 0.55-0.62 for rifles (ABE 0.58 ✓)
- Burn rate n: 0.5-0.9 for small arms
- Force constant f: 950-1050 J/g (single-base), 1050-1200 J/g (double-base)
- WC 844 (M855): energy 3740 J/g, flame temp 2850K, n≈0.78
- WC 846 (M80): energy 3760 J/g, flame temp 2890K, n≈0.82

### Temperature Sensitivity Verified
- ABE 0.9 m/s/°C: reasonable average
- Hodgdon Extreme: 0.4-0.7 m/s/°C
- Ball powders: 1.2-2.0 m/s/°C
- Piecewise at 21°C threshold confirmed (Boulkadid 2015)

### Loading Density Effect
- P ∝ (density)^k where k ≈ 1.5-2.0
- Rifle optimal: 85-100% fill

---

## 12. Vehicle Armour Specs
**Agent bg_ec6b085d** — MBT and AFV estimates

### Key RHAe Ranges (KE/CE)

| Vehicle | Turret KE (mm) | Turret CE (mm) | Hull KE (mm) |
|---------|---------------|---------------|--------------|
| M1A2 SEPv3 | 650-750 | 1300-1500+ | 500-550 |
| Leopard 2A7V | 700-750 | 1400-1500+ | 450-500 |
| Challenger 2 TES | ~700 (est) | ~1500 (est) | ~500 (est) |
| T-72B3 + K-5 | ~700 (w/ K-5) | ~1100 (w/ K-5) | ~550 (w/ K-5) |
| T-90M | ~600-700 | ~900-1200 | ~400-450 |
| T-14 Armata | ~700-800 (est) | ~1200-1500 (est) | — |

### ERA RHAe Bonuses
| ERA | KE bonus | CE bonus |
|-----|---------|---------|
| Kontakt-1 | ~0-50 | ~300-400 |
| Kontakt-5 | ~200-250 | ~500-600 |
| Relikt | ~350-450 | ~800-1000 |
| Malachit | ~400-500 | ~1000-1200 |
| ARAT-2 | ~100-150 | ~400-500 |

### M2 Bradley
Base hull: 25.4mm 5083/7039 Al
RHAe: ~25-35mm KE (base), ~50-60mm A3, ~60-70mm A4

---

## 13. Environment/Weather Ballistics
**Agent bg_e702e71e** — Atmospheric correction

### ISA Layer Structure (Validated)
| Layer | Base Alt (m) | Base Temp (K) | Lapse (K/km) |
|-------|-------------|---------------|-------------|
| Troposphere | 0 | 288.15 | -6.5 |
| Tropopause | 11000 | 216.65 | 0 |
| Stratosphere 1 | 20000 | 216.65 | +1.0 |
| Stratosphere 2 | 32000 | 228.65 | +2.8 |
| Stratopause | 47000 | 270.65 | 0 |
| Mesosphere 1 | 51000 | 270.65 | -2.8 |
| Mesosphere 2 | 71000 | 214.65 | -2.0 |

### Density Altitude
Exact: DA = (T_SL/Γ)[1 - (P/P_SL / T/T_SL)^((gM/ΓR - 1)^-1)]
Approx: DA ≈ PA + 118.8 ft/°C × (T_OA - T_ISA)

### Humidity Effect on Density
- 15°C, 80% RH: -0.25% density
- 35°C, 80% RH: -1.7% density
- Effect on BC small but measurable

### Rain Effect on BC
- Light (0.5mm/hr): negligible
- Moderate (2.5mm/hr): ~0.1-0.3% BC decrease
- Heavy (12.5mm/hr): ~0.5-1% BC decrease

### Wind Averaging Models
- Pejsa: W_eff = (W_FL + 3W_⅓ + 2W_⅔)/6 (time-weighted)
- Weihe: W_eff = 0.3W_near + 0.6W_mid + 0.1W_far

---

## 14. Scope/Optronics Data
**Agent bg_ec71a1cb** — Riflescope specifications

### Key Scope Elevation Limits

| Manufacturer | Model | Total Elevation | Working Elevation |
|-------------|-------|----------------|-------------------|
| Tangent Theta | TT315P | 34 mrad (117 MOA) | 28 mrad (96 MOA) |
| Tangent Theta | TT525P | 30 mrad (103 MOA) | 28 mrad (96 MOA) |
| Nightforce | ATACR 5-25×56 | 35 mrad (120 MOA) | — |
| Schmidt & Bender | PM II 5-25×56 | 27 mrad (92.5 MOA) | — |
| S&B | PM II 3-27×56 | 30 mrad (103 MOA) | — |
| Leupold | Mark 5HD 5-25 | 35 mrad (120 MOA) | — |
| Vortex | Razor Gen III 6-36 | 30 mrad (102 MOA) | — |

All standard click values: 0.1 mrad or 0.25 MOA

### AB Solver Accuracy Benchmark
- Custom Curve: 0.1-0.15 mil vertical error at 1000m
- G7 generic: 0.3-0.5 mil error at 1000m
- Scope adjustment precision: 0.1 mrad per click

---

## 15. CFD / Projectile Aero
**Agent bg_5fbee940** — Academic CFD data

### Master Resource
**De Briey, V. (2021)** PhD Thesis — "Small-Caliber Exterior Ballistics: Aerodynamic Coefficients Determination by CFD"
257 pages, free PDF: orbi.uliege.be/bitstream/2268/260317/1/PhD_BE_deBRIEY_Sep21_final_version.pdf

Covers: 6DoF model, Magnus extraction, pitch damping (reduced frequency dependency), transonic domain, boundary layer resolution, mass unbalance effects

### Key CFD Papers

| Paper | What It Validates | DOI |
|-------|------------------|-----|
| Sabanovic (2021) | 5.56mm M855/SS109/ L110/M856 drag via CFD | 10.37868/dss.v2.id172 |
| Piasta et al. (2025) | M193 experimental CFD validation | 10.1088/1742-6596/3027/1/012048 |
| Cucchi (2023) | External ballistics flight simulation | Polimi thesis |
| Ferfouri et al. (2025) | Grid topology & RANS for artillery CD | 10.47176/jafm.18.4.3052 |
| Du (2021) | 6DOF vs Modified Point Mass | ERAU thesis |

### Magnus Coefficient
Cmag = 0.00010-0.00020 for M855-like shapes (Mach 0.5-2.0) from De Briey extraction
ABE Cmag = 0.00015: ✓ plausible for M855 in transonic regime

### Pitch Damping
Cm_q values: -20 to -45 (stabilizing) for small-caliber spitzers
Strong dependency on reduced pitch frequency (critical for ABE's 6DOF model)

### Boundary Layer Transition
Critical Re: 2×10⁵ to 5×10⁵ for rifle projectiles
M855 Re_d ≈ 3.5×10⁵ (transitional on ogive)
Transition-sensitive turbulence models recommended

### Form Factor i (G7)
M855: 0.90-0.95, M118LR: 0.92-0.97, .338 Lapua: 0.93-0.98

---

## Summary of Corrections for ABE

| Parameter | Old Value | New Value | Source |
|-----------|----------|-----------|--------|
| NATO standard temp | 21°C (presumed) | 15°C ISA / 21°C EPVAT | STANAG 4044 |
| M249 MV | 925 m/s | 915 m/s | Wikipedia |
| M40 MV | 790 m/s | 777 m/s | USMC spec |
| SVD twist | 1:12.6" | 1:9.4" (post-1975) | Wikipedia |
| Ceramic interface defeat | 1500 m/s uniform | 900-1700 m/s material-dep | Lundberg, Behner |
| ERA KE/HEAT common | same curve | split KE ~0.55, HEAT ~0.80 | Qi 2025 |
| STANAG 4626 | (assumed ammo) | "Modular Avionics" | NATO std list |
