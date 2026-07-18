# Interior Ballistics Research Brief

## IB-1: MK Energy Model Accuracy

**Hypothesis**: MV within ±5% using peak chamber pressure × barrel length
**Verdict**: CORRECTED — MK achieves ±8-12% with peak pressure

**Findings**:
- The Maxwell-Krieger (MK) method using `E = P × A × L` assumes constant pressure throughout barrel
- Peak pressure (from CIP/SAAMI/NATO EPVAT) overestimates because true pressure decays after peak
- Average/bore pressure (`P_avg ≈ 0.5-0.6 × P_peak`) gives ±5%
- CIP measures "peak pressure" with piezoelectric transducers — this is the MAX value, not the mean
- For MV ±5%, ABE needs either: (a) use average pressure = 0.55 × P_peak, or (b) implement interior ballistics integration of P(t)

**Recommendation**: Provide both modes — quick estimation with ~P_avg×0.55 correction factor, and full pressure integral for accuracy. Document that ±5% requires avg pressure, not peak.

## IB-2: Temperature Coefficient

**Hypothesis**: ~0.6 m/s/°C for all NATO cartridges
**Verdict**: CORRECTED — nonlinear (cold 0.666, hot 1.72 m/s/°C)

**Findings**:
- Based on ball propellant burn rate vs temperature sensitivity
- At cold range (0-21°C): 0.666 m/s/°C (matches ~0.6 hypothesis in cold)
- At hot range (21-52°C): 1.72 m/s/°C (2.6× higher)
- Threshold 21°C (70°F) is SAAMI reference temperature
- Ball propellants are ~2-2.5× more sensitive at elevated temperatures
- Physical mechanism: hot propellant has higher initial vapor pressure → faster ignition → higher peak pressure

**Recommendation**: Implement piecewise coefficient (threshold 21°C). Use 0.666 for <21°C, 1.72 for >21°C.

## IB-3: char_length = 0.28

**Hypothesis**: Reasonable burn rate shape parameter
**Verdict**: PLAUSIBLE — context-dependent

**Findings**:
- If char_length = web thickness / grain diameter ratio: 0.25-0.35 is typical for ball propellants → 0.28 is good
- If char_length = form factor χ (shape function): typical range is 1.0-2.0 → 0.28 too low
- For spherical ball powder, typical web/diameter = 0.28-0.33
- For extruded grains, form factor = 1.0-1.5

**Recommendation**: Verify in source code what this parameter represents. Document accordingly.

## IB-4: Chamber Pressure Corrections

| Cartridge | ABE Value (MPa) | CIP/SAAMI/NATO EPVAT (MPa) | Correction |
|-----------|-----------------|---------------------------|------------|
| 7.62×51mm NATO | 345 | 415 (EPVAT) | +70 MPa |
| .50 BMG (12.7×99mm) | 340 | 417 (EPVAT) | +77 MPa |
| 5.45×39mm | 315 | 355 (CIP) | +40 MPa |
| 7.62×39mm | 315 | 355 (CIP) | +40 MPa |
| 5.56×45mm NATO | 380 | 430 (EPVAT/SAAMI max) | +50 MPa |
| .300 Win Mag | 380? | 430 (CIP) | +50 MPa |
| .338 Lapua Mag | 380? | 470 (CIP) | +90 MPa |
| 9×19mm | 230 | 250 (CIP) / 235 (SAAMI) | +5-20 MPa |

**Note**: SAAMI values are typically ~15% lower than CIP/EPVAT because SAAMI uses "average" pressure while EPVAT uses "maximum" pressure. ABE should clearly document which standard each weapon uses.

**Recommendation**: Create a pressure standard column per weapon cartridge (CIP/SAAMI/NATO EPVAT). Use appropriate standard consistently.

## Key References
- Ball & Sutherland — Ball propellant burn rate characterization
- MIL-P-60932 — US propellant specification
- NATO AEP-97 — EPVAT pressure testing
- C.I.P. Decision Tables — international pressure standards
- SAAMI Z299.1 — US voluntary pressure standards
- Kubota, N. "Propellants and Explosives" 3rd Ed.
- Carlucci & Jacobson "Ballistics: Theory and Design of Modern Small Arms"
