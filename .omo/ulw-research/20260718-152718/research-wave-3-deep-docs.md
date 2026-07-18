# Wave 3 Research — Deep Document Mining

Generated: 2026-07-18
Sources: Web fetches, targeted searches, DTIC/Scribd/kestrel sources

## Key M829 Series APFSDS Data (from Wikipedia + FAS)

| Variant | MV | Penetrator L×D | L/D | Mass | Propellant | Pen @2km (est RHA) |
|---------|-----|-----------------|-----|------|------------|-------------------|
| M829 | 1,670 m/s | 627 × 27 mm | 23.2:1 | 4.0 kg DU | 8.1 kg JA-2 | ~540 mm |
| M829A1 | 1,575 m/s | 684 × ~22 mm | ~31:1 | 4.6 kg DU | 7.9 kg JA-19 | ~650 mm |
| M829A2 | ~1,675 m/s | 780 × ~22 mm | ~35:1 | ~4.6 kg DU | Improved | ~700 mm |
| M829A3 | ~1,555 m/s | ~670+100mm tip × 25mm | ~31:1 | ~5 kg | JA-2 variant | ~750 mm+ |
| M829A4 | ~1,650 m/s | Classified | — | ~5 kg | Enhanced | ~750-800 mm+ |

Key: M829A3 uses stepped-tip design (steel tip ~100mm that breaks on ERA, main DU rod ~670mm). M829A2 uses carbon-epoxy composite sabot (world first).

## Applied Ballistics Profile Loader (Freely Downloadable)
- **URL**: kestrelballistics.com → Applied Ballistics Profile Loader → Version 090 (9 MB ZIP)
- Contains the full CDM library for 815+ bullets
- Windows-only. Kestrel 5700 requires USB cable for transfer
- Can generate gun profile XMLs with BC, CDM, and all ballistic parameters
- Kestrel Elite stores 30 guns, 30 stages × 10 targets
- Custom curves, aerodynamic jump, spin drift, Coriolis all computed

## Source: AB AB Quantum Free App
- Free tier has basic G1/G7 solver
- Elite tier unlocks CDM library, WEZ analysis, CDF calibration
- AB Learn + AB Spotter (AI) included in Quantum
- Connects to 50+ Bluetooth devices

## CDM Methodology (NRA Article / AB Technical Paper)
- **15% BC variation** across velocity range for typical boat-tail bullets
- G7 CDM variance: 5.2% supersonic, 12% at Mach 1 transonic
- CDM eliminates BC concept entirely — uses raw Cd(Mach) tables
- Personal Drag Models (PDM) available for individual rifle-bullet combos via radar
- 2019 Kestrel Fall Classic: 49 competitors got PDMs

## frfrogspad BC Tables
- **Commercial G1 BC table** (Excel): 2012 update, all major manufacturers
- **Military G7/G8 BC table** (Excel): Aberdeen Proving Ground data + Litz data
- Both freely downloadable directly from frfrogspad.com

## De Briey PhD (2021) — Full PDF Available
- **Direct download**: orbi.uliege.be/bitstream/2268/260317/1/PhD_BE_deBRIEY_Sep21_final_version.pdf (87 MB)
- 257 pages, 6DoF trajectory model in LabVIEW (VTraj)
- Magnus extraction: Cmag = 0.00010-0.00020 for M855-like shapes at Mach 0.5-2.0
- Pitch damping: Cmq = -20 to -45 (stabilizing) for small-caliber spitzers
- Critical Re: 2×10^5 to 5×10^5 for rifle projectiles, M855 Re_d ≈ 3.5×10^5
- Paper won Prix General Vanvreckom 2023 and Jack Riegel Student Award 2019

## Litz Book Series
| Title | Edition | Year | ISBN | Pages |
|-------|---------|------|------|-------|
| Applied Ballistics for Long Range Shooting | 4th | 2023 | 9780990920663 | 432 |
| Ballistic Performance of Rifle Bullets | 3rd | 2017 | 9780990920601 | 720 bullets |
| Modern Advancements in Long Range Shooting | Vol 1 | 2014 | 9780692208434 | 339 |
| Modern Advancements in Long Range Shooting | Vol 2 | 2016 | 9780990920632 | 358 |
| Modern Advancements in Long Range Shooting | Vol 3 | — | 9780990920656 | — |
| Accuracy and Precision for Long Range Shooting | 2nd | 2011 | 9780615672557 | 578 |

## Kestrel 5700 Elite Specifications
- AB library: G1/G7 BC for 500+ bullets, CDM for 815+ bullets (Elite only)
- Coriolis correction (latitude-dependent via compass)
- Spin drift, aerodynamic jump, drop scale factoring (DSF)
- CDF (Custom Drag Factor) tune — 2025 feature
- 15 environmental parameters measured
- 30 gun profiles, 30 stages with 10 targets each
- LiNK (Bluetooth) to iOS/Android
- Firmware updates free

## Scribd Documents Identified
- FM 6-40 1945 (467 pages, field artillery gunnery + elementary ballistics)
- FM 6-40 1967 (401 pages)
- FM 6-40 1981 (available on CGSC)
- FM 6-40 1939 (older edition)
- All available on Archive.org for free download
