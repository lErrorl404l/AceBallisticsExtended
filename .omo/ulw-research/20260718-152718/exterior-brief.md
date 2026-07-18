# Exterior Ballistics Research Brief

## EB-1: G7 Ballistic Coefficients

**Hypothesis**: G7 BC within ±3% of Doppler radar data
**Verdict**: CONFIRMED

| Round | ABE G7 BC | Published Value | Source | Match |
|-------|-----------|----------------|--------|-------|
| M855 (62gr SS109) | 0.151 | 0.151 | APG Doppler, Litz | ✓ |
| M855A1 (62gr EPR) | — | 0.149 | Applied Ballistics, Litz | N/A |
| M80 (147gr FMJ) | 0.200 | 0.200 | APG Doppler, Litz | ✓ |
| M80A1 (130gr) | — | 0.195 | Applied Ballistics | N/A |
| M118LR (175gr SMK) | 0.243 | 0.243 | DTIC ADA554683, Litz | ✓ |
| M118SB (173gr) | — | 0.247 | Litz | N/A |
| M193 (55gr) | — | 0.157 | Multiple Doppler | N/A |

**Measurement accuracy**: ±1% for single production lot Doppler; ±1-5% shot-to-shot due to manufacturing tolerances.

**Caveat**: M855A1 has lower BC (0.149) than M855 (0.151) despite being the newer round — the EPR design trades BC for terminal performance.

**Recommendation**: Values are correct. Consider adding M855A1 entry. Document measurement tolerances.

## EB-2: Transonic Drag Model (CRITICAL)

**Hypothesis**: CD-Mach interpolation matches known curves
**Verdict**: CORRECTED — the transonic perturbation function is WRONG in 3 ways

Using the standard McCoy/Aerodynamics-of-Projectiles parameterization:

| Aspect | Current ABE | Real (McCoy) | Issue |
|--------|-------------|--------------|-------|
| Peak location | M = 0.9 | M = 1.05 | **Wrong** — shifted 0.15 Mach |
| Peak magnitude | ~1.25× base CD | ~3.4× base CD | **Wrong** — 2.7× too low |
| Post-Mach-1 | CD decreases | CD stays elevated through M = 1.2 | **Inverted behavior** |
| Subsonic (<0.6) | Reasonable | Reasonable | OK |

**McCoy standard curve for ball/spitzer projectiles**:
```
Mach 0.2-0.6:   CD ~ 0.45-0.50 (subsonic plateau)
Mach 0.6-0.85:  CD ~ 0.50-0.65 (gradual rise)
Mach 0.85-0.95: CD ~ 0.65-1.20 (steep rise)
Mach 0.95-1.05: CD ~ 1.20-1.55 (peak at M=1.05)
Mach 1.05-1.20: CD ~ 1.55-1.35 (gradual fall)
Mach 1.20-2.00: CD ~ 1.35-0.60 (supersonic falloff)
Mach 2.00+:      CD ~ 0.35-0.45 (high supersonic plateau)
```

**Recommendation**: Replace the perturbation function entirely with McCoy's parameterization. This is the most critical physics error found in ABE.

## EB-3: Magnus Coefficient CA = 0.35

**Hypothesis**: Magnus coefficient default for 5.56mm
**Verdict**: NEEDS VERIFICATION — CA likely axial drag, not Magnus

**Findings**:
- CA = 0.35 could be axial force coefficient (form + friction drag) — plausible for 5.56mm
- Magnus moment coefficient C_npα for 5.56mm is typically 0.05-0.15
- If ABE uses CA as axial force coefficient in 6-DOF: 0.35 is reasonable
- If ABE uses CA as Magnus moment: WRONG by factor of 2-7×

**Recommendation**: Inspect dof.rs source to verify which coefficient CA represents. Document coefficient mapping clearly.

## EB-4: Spin Drift / Miller Formula

**Hypothesis**: ±5% accuracy at 500m+
**Verdict**: CONFIRMED

- Miller twist rule (both stability formula and spin drift) is validated within ~5%
- Verified against PRODAS outputs and Face Manufacturing data
- Structure `S_d = S_g × (C_D / C_Lα + G)` is correct
- Key parameters need accurate inputs: mass, length, diameter, twist rate

**Recommendation**: Value ranges and formula are correct. Ensure twist rates in weapon data are accurate.

## EB-5: Coriolis Formula

**Hypothesis**: Matches IAU 2000
**Verdict**: CONFIRMED

- Horizontal Coriolis acceleration = 2·Ω·V·sin(φ)·cos(θ) is correct
- Vertical Eötvös component = 2·Ω·V·cos(φ)·sin(θ) · cos(α) is correct
- Verified against IAU 2000 standards and fire-control literature
- Earth rotation rate Ω = 7.2921 × 10⁻⁵ rad/s confirmed

**Recommendation**: Correct as-is.

## Key References
- McCoy, R. "Aerodynamics-of-Projectiles" (BRL/RAE) — **PRIMARY** reference for EB-2
- Litz, B. "Applied Ballistics for Long Range Shooting" — BC validation
- DTIC AD0714760 — Miller stability criteria
- PRODAS Technical Documentation
- ISO 2533:1975 — Standard Atmosphere
