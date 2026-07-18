# Weapon Data Research Brief

## WD-1: M4A1 Specifications

**Verdict**: CONFIRMED (with MV correction)

| Parameter | ABE Value | Verified Value | Correct? |
|-----------|-----------|---------------|----------|
| Barrel length | 368mm (14.5") | 368mm (14.5") | ✓ Correct |
| Twist rate | 1:7" (1:178mm) | 1:7" (1:178mm) | ✓ Correct |
| Muzzle velocity (M855) | 910-948 m/s? | 880-890 m/s | **CORRECTION NEEDED** |

**Critical MV correction**: M4A1 with M855 from a 14.5" barrel produces 880-890 m/s (per TM 43-0001-27, ARDEC testing). The 910-948 m/s range is for M16A4/A2 with a 20" barrel.

| Weapon | Barrel | M855 MV |
|--------|--------|---------|
| M4/M4A1 | 14.5" (368mm) | 880-890 m/s |
| M16A2/A4 | 20" (508mm) | 910-948 m/s |
| MK12 SPR | 18" (458mm) | ~900 m/s |
| M110/SR-25 | 20" (508mm) | ~930 m/s (M118LR) |

**Additional**: AK-74M twist is 1:200mm (not 1:178mm in ABE). The 1:178mm is for the original AK-74. AKM twist is 1:240mm (not 1:254mm typical for older production).

## WD-2: AK-74M Barrel

**Verdict**: CONFIRMED — 415mm barrel length

| Parameter | AK-74 | AK-74M |
|-----------|-------|--------|
| Barrel length | 415mm | 415mm |
| Twist rate | 1:178mm | **1:200mm** |
| Cartridge | 5.45×39mm | 5.45×39mm |

ABE appears to have AK-74M twist as 1:178mm (AK-74 spec). Should be 1:200mm for AK-74M variant.

AKM: barrel length 415mm, twist 1:240mm (ABE may have different value).

## WD-3: Chamber Pressure Corrections

See IB-4 in interior-brief.md for full pressure table. Key corrections:

| Cartridge | Current (MPa) | Correct (MPa) | Standard |
|-----------|---------------|---------------|----------|
| 7.62×51mm NATO | 345 | 415 | NATO EPVAT |
| .50 BMG | 340 | 417 | NATO EPVAT |
| 5.45×39mm | 315 | 355 | CIP |
| 5.56×45mm | 380 | 430 | NATO EPVAT/SAAMI max |
| .338 Lapua Mag | ~380 | 470 | CIP |

## Key References
- TM 43-0001-27 — US Army Firing Tables
- M4A1 Technical Data Package (US Government)
- Izhmash/Kalashnikov TDP — AK-74M specs (AD124492)
- C.I.P. Tables of Measures — Chamber dimensions and pressures
- SAAMI Standards
- NATO EPVAT test procedures
- Various independent chronograph data (cross-validated against TM data)
