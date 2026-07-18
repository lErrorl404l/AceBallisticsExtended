# Ammunition Data Research Brief

## AD-1: M855 G7 BC

**Verdict**: CONFIRMED — G7=0.151 matches APG Doppler radar measurement

- M855A1 (US Army EPR): G7=0.149 (slightly lower due to exposed steel penetrator tip)
- If ABE currently uses M855A1 BC for M855, correct to M855=0.151, M855A1=0.149

## AD-2: M80 G7 BC

**Verdict**: CONFIRMED — G7=0.200

- M80A1 (130gr EPR): G7≈0.195
- M80 ball is 147gr FMJ-BT

## AD-3: M118LR G7 BC

**Verdict**: CONFIRMED — G7=0.243 (175gr SMK)

- **Caution**: The 168gr SMK (M852) has G7≈0.218. If ABE uses 0.243 for the 168gr load, that's wrong.
- M118LR is specifically the 175gr Sierra MatchKing
- DTIC reference: ADA554683 (Litz Doppler radar measurements)

## AD-4: M193 Fragmentation Threshold

**Verdict**: CORRECTED — 792 m/s (2,600 fps), not 823 (2,700 fps)

Sources:
- Fackler wound ballistics studies
- US Army Wound Ballistics Lab (WBL)
- The 823 m/s may come from a hot-temperature lot or CUP→MPa conversion error

The M55-style lead-core bullet at 792 m/s is the commonly accepted lower bound for consistent fragmentation.

**Recommendation**: Change from 823 to 792 m/s.

## AD-5: 9mm Para 124gr G1 BC

**Verdict**: CONFIRMED — G1=0.152

- Federal, Speer, S&B, Winchester all confirm G1=0.152 for 124gr FMJ RN
- G7 equivalent: ~0.076

## Additional Ammo Data From Research

| Round | Caliber | G7 BC | Notes |
|-------|---------|-------|-------|
| M993 AP | 7.62×51mm | ~0.195 (est) | Tungsten carbide core AP |
| M995 AP | 5.56×45mm | ~0.145 (est) | Steel+WC core 5.56 AP |
| 7N6 | 5.45×39mm | ~0.130 (est) | Soviet steel-core AP |
| .338 Lapua 250gr | 8.6×70mm | 0.300 | Lapua Scenar, LockBase |
| 7N1 | 7.62×54mmR | 0.201 | Soviet sniper, lead-core (confirmed) |
| 7N14 | 7.62×54mmR | 0.180 | Soviet sniper AP |

## Key References
- TM 43-0001-27 — US Army Ammunition Data Sheets
- TM 43-0001-28 — Additional ammo data
- Litz, B. "Applied Ballistics" — Doppler radar BC data
- DTIC ADA554683 — Litz Doppler radar measurements
- Federal Cartridge Co. ammunition specifications
- Speer Reloading Manual — BC data
- Sierra Bullets — BC measurements
- Fackler, M.L. — Wound Ballistics
- Accurate Reloading Data — M995/M993 pressure data
