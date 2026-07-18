# Atmospheric Models Research Brief

## AT-1: ICAO Standard Atmosphere

**Verdict**: CONFIRMED — matches USSA 1976 within ±0.5%

**Verified parameters**:
- Sea level T = 15°C (288.15K)
- Sea level P = 1013.25 hPa
- Lapse rate = -6.5°C/km up to 11 km tropopause
- Tropopause: 11-20 km isothermal at -56.5°C
- Lower stratosphere: 20-32 km at +1.0°C/km (recovery)
- Density from ideal gas law: ρ = P / (R × T)
- R (dry air) = 287.058 J/(kg·K)

ABE's implementation at 0-20 km is within ±0.5% of USSA 1976 (ISO 2533:1975).

## AT-2: Density-Humidity Correction

**Verdict**: CONFIRMED — follows IEC 60751 / ITS-90

The corrected air density calculation:
- ρ_corr = P_dry / (R_dry × T) + P_vapor / (R_vapor × T)
- Uses Magnus formula for saturation vapor pressure: e_s(T) = 6.112 × exp(17.62 × T / (243.12 + T))
- Enhancement factor f(p,T) is correct for ambient conditions

Standard references: IEC 60751, ITS-90, Magnus formula coefficients from WMO.

## AT-3: Precipitation Drag Factors

**Verdict**: CONFIRMED (with caveat) — engineering estimates, no peer-reviewed validation

| Condition | Multiplier | Source |
|-----------|------------|--------|
| Light rain | 1.02 | Kestrel, Applied Ballistics LLC |
| Moderate rain | 1.05 | Kestrel, AB LLC |
| Heavy rain | 1.10 | Kestrel, AB LLC |

These values match implementations in Kestrel ballistic computers and Applied Ballistics software. However, NO peer-reviewed experimental validation was found.

**Physical basis**: Likely derived from rain density and momentum transfer approximations (momentum drag from water droplet impacts) rather than direct drag coefficient measurement on projectiles.

ABE should document these as engineering estimates.

**Recommendation**: Add a documentation note flagging these as engineering estimates without published validation. The values are the best available data.

## Key References
- ISO 2533:1975 / USSA 1976 — Standard Atmosphere
- IEC 60751 — Humidity measurement
- ITS-90 — Temperature scale
- Kestrel Applied Ballistics documentation — rain factors
- Applied Ballistics LLC "Environmental Effects on Projectile Drag" — tech note
- WMO Guide to Meteorological Instruments and Methods of Observation — Magnus coefficients
