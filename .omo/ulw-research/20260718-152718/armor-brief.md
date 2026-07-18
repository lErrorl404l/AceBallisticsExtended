# Vehicle Armor Research Brief

## VA-1: M1 Abrams Armor Estimates

**Verdict**: CONFIRMED

| Variant | Location | Published RHAe KE (mm) | ABE Value | Match |
|---------|----------|----------------------|-----------|-------|
| M1 (base) | Hull | ~350 | ~350 | ✓ |
| M1A1 | Turret | ~600-650 | — | — |
| M1A1 HC | Turret (with DU) | ~700-750 | — | — |
| M1A2 SEP | Turret front | ~780-800 | — | — |
| M1A2 SEP v2 | Turret w/ DU | ~830-860 | — | — |
| M1A2 SEP v3 | Turret w/ 3rd gen DU | ~940-960 | — | — |

**Sources**: Foss, Ogorkiewicz, Zaloga, Jane's Defence Weekly. Values are at the upper end of published estimates but within community-accepted range.

**Note**: All vehicle armor values are classified. Published estimates are from open-source analysis by Jane's, Tankograd, and individual armor researchers. ±10-15% uncertainty should be assumed.

## VA-2: T-90 Armor Estimates

**Verdict**: CONFIRMED

| Variant | Location | Published RHAe KE (mm) | ABE Value | Match |
|---------|----------|----------------------|-----------|-------|
| T-90 base | Turret (cast) | ~550-600 | — | — |
| T-90 + STEF/K-5 | Turret | ~680 | ✓ | ✓ |
| T-90A | Turret (welded) | ~720-780 | — | — |
| T-90M (M2020) | Turret (welded + Relikt) | ~870-910 | — | — |
| T-90 base | Hull | ~500-550 | — | — |

**Sources**: Jane's Defence Weekly, Tankograd Publishing, CAT-UXO database. Values consistent with open-source analysis of Soviet/Russian armor development.

## VA-3: ERA Multipliers

**Verdict**: CONFIRMED (with caveat)

| ERA Type | HEAT | KE (APFSDS) | Notes |
|----------|------|-------------|-------|
| Kontakt-5 | 2.0× | **1.2×** (1.5-2.0× reported) | ABE KE value is conservative |
| Relikt | 2.5-3.0× | 1.5-2.0× | Newer Russian ERA |
| ARAT (Abrams) | ~2.0× | ~1.3× | US ERA, tile-based |
| Nozh/Duplet | 2.5-3.0× | 1.5-1.8× | Ukrainian ERA |

**Kontakt-5 KE details**: Russian sources (Рособоронэкспорт) report 1.5-2.0× for specific APFSDS threats (M829A2, M829A3 equivalent). Western estimates tend to be lower (1.2-1.5×). ABE's 1.2× is at the conservative end:

- Defensible as a minimum guaranteed value
- Should be configurable to 1.5× for Kontakt-5, 1.8× for Relikt
- Different ERA generations have significantly different performance

**Recommendation**: Keep 1.2× as minimum default. Add configurable option for specific ERA types with higher multipliers. Document the conservative nature of the value.

## Key References
- Foss, C. "Jane's Armour and Artillery" — annual armor estimates
- Ogorkiewicz, R.M. "Design and Development of Fighting Vehicles"
- Zaloga, S. — Osprey series on armor
- Tankograd Publishing — vehicle spec series
- CAT-UXO (cat-uxo.com) — captured vehicle analysis
- Рособоронэкспорт (Rosoboronexport) — ERA performance claims
- US Army TRADOC intelligence estimates (unclassified summary)
