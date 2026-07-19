# HEAT Penetration Test Coverage Gap Analysis

**Date:** 2026-07-18  
**Scope:** `ext/src/penetration/heat_penetration.rs` vs `data/armor/materials/`  
**Test target field:** `HeatJetParams.target_armor_material`  
**Source data referenced:** `arl_penetration_data.md` (ARL penetration test reports, ceramic DOP data), `armox_armor_data.md` (SSAB ARMOX specs, MIL-DTL-46100E HHA, MIL-DTL-12560J RHA), `saami_cip_pressures.md` (chamber pressure specs for warhead reference)

---

## Summary

Of **72** armor materials defined in `data/armor/materials/`, only **1** (`steel_rha`) has any end-to-end HEAT penetration test coverage. That means **71 materials (99%) are untested** in the HEAT penetration model.

The `get_v_min_for_target()` function dispatches materially different physics constants per category (ceramic→2000 m/s, aluminum→1500 m/s, rubber/spall→500 m/s, wood→300 m/s, default steel→2500 m/s), but only the default steel path is exercised by tests.

---

## Test Inventory

### Test helper parameter builders (all use `"steel_rha"`)

| Helper | `target_armor_material` | Warhead |
|---|---|---|
| `rpg7_heat_params()` | `steel_rha` | RPG-7 (85 mm) |
| `maaws_heat_params()` | `steel_rha` | MAAWS (84 mm) |
| `heavy_heat_params()` | `steel_rha` | Heavy (105 mm) |

### End-to-end penetration tests

All 8 E2E tests use one of the three helpers above → all use `"steel_rha"`:

| Test | Uses helper | Target material |
|---|---|---|
| `basic_heat_penetrates_rha` | `rpg7_heat_params()` | `steel_rha` |
| `standoff_affects_penetration_peak_at_optimal` | `rpg7_heat_params()` | `steel_rha` |
| `era_significantly_reduces_penetration` | `rpg7_heat_params()` | `steel_rha` |
| `era_reduces_penetration_substantially` | `heavy_heat_params()` | `steel_rha` |
| `tandem_charge_defeats_era` | `heavy_heat_params()` | `steel_rha` |
| `jet_disrupted_by_thick_era` | `heavy_heat_params()` | `steel_rha` |
| `deterministic_output` | `rpg7_heat_params()` | `steel_rha` |
| `angled_impact_reduces_penetration` | `rpg7_heat_params()` | `steel_rha` |

### Unit tests (not end-to-end penetration)

| Test | What it tests | Material |
|---|---|---|
| `copper_cone_jet_velocity` | Jet tip calc | N/A |
| `tantalum_slightly_faster_than_copper` | Liner material factor | N/A |
| `penetration_ratio_plausible` | P/L ratio math | N/A |
| `no_pen_below_min_velocity` | V_min threshold | N/A |
| `standoff_peaks_at_two_to_four_calibers` | Standoff efficiency | N/A |
| `target_density_lookup` | Density lookup (only) | Various strings (not actual penetration) |

The `target_density_lookup` test exercises `target_density_from_material()` with string inputs but does **not** test actual HEAT penetration against those materials.

---

## Full Material-vs-Test Matrix

### CERAMICS (14 materials — ALL untested)

| materialId | Category | V_min path | HEAT test? |
|---|---|---|---|
| `ad90` | ceramics | V_MIN_CERAMIC (2000) | ✗ |
| `ad95` | ceramics | V_MIN_CERAMIC (2000) | ✗ |
| `boron_carbide` | ceramics | V_MIN_CERAMIC (2000) | ✗ |
| `ceramic_ad90` | ceramics | V_MIN_CERAMIC (2000) | ✗ |
| `ceramic_ad95` | ceramics | V_MIN_CERAMIC (2000) | ✗ |
| `ceramic_al2o3` | ceramics | V_MIN_CERAMIC (2000) | ✗ |
| `ceramic_b4c` | ceramics | V_MIN_CERAMIC (2000) | ✗ |
| `ceramic_plate` | ceramics | V_MIN_CERAMIC (2000) | ✗ |
| `ceramic_sic` | ceramics | V_MIN_CERAMIC (2000) | ✗ |
| `esapi_al2o3` | ceramics | V_MIN_CERAMIC (2000) | ✗ |
| `esapi_b4c` | ceramics | V_MIN_CERAMIC (2000) | ✗ |
| `esapi_sic` | ceramics | V_MIN_CERAMIC (2000) | ✗ |
| `mar_ceramic` | ceramics | V_MIN_CERAMIC (2000) | ✗ |
| `silicon_carbide` | ceramics | V_MIN_CERAMIC (2000) | ✗ |

**Relevance:** Very high. Ceramics are the backbone of modern vehicle armor arrays (Challenger 2, Leopard 2, M1 Abrams, T-90M). HEAT jets encounter ceramic tiles in composite armor packs. The V_min is 2000 vs 2500 for RHA, meaning the model predicts 25% more penetration against ceramics at equal density — this code path has zero tests. Ceramic armor performance data from ARL DOP tests [source: arl_penetration_data.md §11 (ADA362926); §15 (ARL-TR-7263)] shows Al₂O₃ RHAe=2.2–2.8, SiC=3.0–4.0, B₄C=4.0–5.0 vs KE penetrators [source: arl_penetration_data.md §14 (ARL-TR-3590 interface defeat)].

### COMPOSITES (14 materials — ALL untested)

| materialId | Category | V_min path | HEAT test? |
|---|---|---|---|
| `arma_glass` | composites | V_MIN_RHA (2500) | ✗ |
| `burlington_composite` | composites | V_MIN_RHA (2500) | ✗ |
| `carbon_fiber` | composites | V_MIN_RHA (2500) | ✗ |
| `chobham_composite` | composites | V_MIN_RHA (2500) | ✗ |
| `composite_glass` | composites | V_MIN_RHA (2500) | ✗ |
| `composite_kevlar` | composites | V_MIN_RHA (2500) | ✗ |
| `dorchester_composite` | composites | V_MIN_RHA (2500) | ✗ |
| `k_active_composite` | composites | V_MIN_RHA (2500) | ✗ |
| `kvarts_composite` | composites | V_MIN_RHA (2500) | ✗ |
| `laminated_glass` | composites | V_MIN_RHA (2500) | ✗ |
| `mexas_composite` | composites | V_MIN_RHA (2500) | ✗ |
| `stanag_composite` | composites | V_MIN_RHA (2500) | ✗ |
| `stef_composite` | composites | V_MIN_RHA (2500) | ✗ |
| `textolite_composite` | composites | V_MIN_RHA (2500) | ✗ |

**Relevance:** High. Chobham, Burlington, Dorchester, K-Active, Kvarts, STEF are real composite arrays on major MBTs. All currently fall through to V_MIN_RHA despite being physically different materials — this is a modelling gap the tests would expose. Chobham-class ceramic/steel/rubber composites are documented in ARL ceramic armor research [source: arl_penetration_data.md §11 (ADA362926); §14 (ARL-TR-3590 interface defeat)], with estimated RHAe multipliers of 2.0–3.0 vs KE [source: armox_armor_data.md §9 (VPAM protection levels for reference)].

### ERA (5 materials — ALL untested)

| materialId | Category | V_min path | HEAT test? |
|---|---|---|---|
| `era_kontakt5` | era | V_MIN_RHA (2500) | ✗ |
| `era_light` | era | V_MIN_RHA (2500) | ✗ |
| `era_relikt` | era | V_MIN_RHA (2500) | ✗ |
| `malachite_era` | era | V_MIN_RHA (2500) | ✗ |
| `relikt_era` | era | V_MIN_RHA (2500) | ✗ |

**Relevance:** Medium. ERA interaction is tested in the model (via `era_thickness_m` parameter), but the ERA panel as a target material is never tested. The `era_interaction()` function is tested indirectly, but no test verifies HEAT penetration against an actual ERA material ID as the target. Relikt ERA (NII Stali) is rated ~1.80–2.20 vs KE and ~2.50–3.00 vs HEAT per public estimates [source: arl_penetration_data.md §1 (ARL-TR-4632 for KE reference); armox_armor_data.md §9 for armor equivalency methodology].

### METALS (28 materials — 1 tested, 27 untested)

| materialId | Category | V_min path | HEAT test? |
|---|---|---|---|
| `aluminum_5083` | metals | V_MIN_ALUMINUM (1500) | ✗ |
| `aluminum_7039` | metals | V_MIN_ALUMINUM (1500) | ✗ |
| `aluminum_7075` | metals | V_MIN_ALUMINUM (1500) | ✗ |
| `arma_aluminum` | metals | V_MIN_ALUMINUM (1500) | ✗ |
| `arma_default` | metals | V_MIN_RHA (2500) | ✗ |
| `arma_steel` | metals | V_MIN_RHA (2500) | ✗ |
| `armor_tip_steel` | metals | V_MIN_RHA (2500) | ✗ |
| `armox_500` | metals | V_MIN_RHA (2500) | ✗ |
| `armox_600` | metals | V_MIN_RHA (2500) | ✗ |
| `cage_armor` | metals | V_MIN_RHA (2500) | ✗ |
| `cast_steel` | metals | V_MIN_RHA (2500) | ✗ |
| `depleted_uranium` | metals | V_MIN_RHA (2500) | ✗ |
| `dual_hardness_steel` | metals | V_MIN_RHA (2500) | ✗ |
| `hardox_450` | metals | V_MIN_RHA (2500) | ✗ |
| `hha_steel` | metals | V_MIN_RHA (2500) | ✗ |
| `lead_alloy` | metals | V_MIN_RHA (2500) | ✗ |
| `mars_armor` | metals | V_MIN_RHA (2500) | ✗ |
| `mil_dtl_46100_class1` | metals | V_MIN_RHA (2500) | ✗ |
| `mil_dtl_46100_class3` | metals | V_MIN_RHA (2500) | ✗ |
| `mil_dtl_46100_class4` | metals | V_MIN_RHA (2500) | ✗ |
| `mild_steel` | metals | V_MIN_RHA (2500) | ✗ |
| `perforated_armor` | metals | V_MIN_RHA (2500) | ✗ |
| **`steel_rha`** | metals | V_MIN_RHA (2500) | **✓** |
| `slat_armor` | metals | V_MIN_RHA (2500) | ✗ |
| `slotted_armor` | metals | V_MIN_RHA (2500) | ✗ |
| `titanium_alloy` | metals | V_MIN_RHA (2500) | ✗ |
| `titanium_diboride` | metals | V_MIN_RHA (2500) | ✗ |
| `tungsten_carbide` | metals | V_MIN_RHA (2500) | ✗ |

**Relevance:**
- **Aluminum alloys** (5083, 7039, 7075): High — light armor vehicles (BMP, Stryker, LAV-25). Gets V_MIN_ALUMINUM (1500) which is 40% lower than RHA. Drastically different penetration behavior. ARL-TR-4427 documents V50 for AA5059 and AA5083 [source: arl_penetration_data.md §6].
- **Depleted uranium**: Medium — M1A1HA/M1A2 turret inserts. Falls through to V_MIN_RHA despite being 2.4× denser than steel. DU armor RHAe=1.5–2.0 vs KE [source: arl_penetration_data.md §9 (ARL-TR-1146)].
- **Titanium alloy**: Medium — aircraft armor. Falls through to V_MIN_RHA despite being 56% the density of steel. Ti-6Al-4V RHAe=0.85–1.00 vs 7.62 AP [source: arl_penetration_data.md §7 (ARL-TR-4996)].
- **MIL-DTL-46100 / HHA / Armox / Hardox**: Low-medium — high-hardness steels that may behave differently vs HEAT than RHA. SSAB ARMOX specifications [source: armox_armor_data.md §3–§8]; ARL-TR-4632 V50 data [source: arl_penetration_data.md §1].
- **Tungsten carbide, titanium diboride**: Low — these are AP core / advanced ceramic-metal materials, not typical armor structural materials.

### POLYMERS (11 materials — ALL untested)

| materialId | Category | V_min path | HEAT test? |
|---|---|---|---|
| `arma_plastic` | polymers | V_MIN_RHA (2500) | ✗ |
| `arma_rubber` | polymers | V_MIN_RHA (2500) | ✗ |
| `dyneema_liner` | polymers | V_MIN_RHA (2500) | ✗ |
| `fiberglass` | polymers | V_MIN_RHA (2500) | ✗ |
| `kevlar_liner` | polymers | V_MIN_RHA (2500) | ✗ |
| `spall_liner_kevlar` | polymers | V_MIN_RHA (2500) | ✗ |
| `rubber_elastomer` | polymers | 500.0 (explicit) | ✗ |
| `rubber_solid` | polymers | 500.0 (explicit) | ✗ |
| `spall_liner` | polymers | 500.0 (explicit) | ✗ |
| `twaron_liner` | polymers | V_MIN_RHA (2500) | ✗ |
| `uhmwpe` | polymers | V_MIN_RHA (2500) | ✗ |

**Relevance:** Low for primary HEAT armor (polymers are backers/spall liners, not primary armor). But `rubber_elastomer`, `rubber_solid`, and `spall_liner` have an explicit V_min of 500 m/s (vs 2500 for RHA) — this is a very different code path that's never tested.

---

## Code Paths Without Coverage

The following dispatch branches in `get_v_min_for_target()` have **zero** test coverage:

| V_min branch | Triggering materials | Tests |
|---|---|---|
| `V_MIN_CERAMIC` (2000) | Any `ceramic_*`, `*_al2o3`, `*_b4c`, `*_sic`, `ad90`, `ad95`, `mar_ceramic` | 0 |
| `V_MIN_ALUMINUM` (1500) | `aluminum_*`, `arma_aluminum` | 0 |
| `V_MIN_CONCRETE` (1000) | `concrete_*`, `gypsum`, etc. | 0 |
| 500 m/s branch | `rubber_*`, `elastomer`, `spall_*`, `*_liner` | 0 |
| 300 m/s branch | `wood`, `plywood`, etc. | 0 |

The `target_density_from_material()` function has broader dispatch with 19 branches, but only 5 are touched by the `target_density_lookup` test (steel, aluminum, ceramic, concrete, wood). The following density branches are also untested:

- titanium (4430)
- uranium/DU (19000)
- glass (2500)
- kevlar/aramid/twaron (1440)
- dyneema/UHMWPE (970)
- rubber/elastomer (1100)
- lead (11340)
- chobham/burlington/stanag/mexas/etc. (4500)
- carbon/fiberglass (1800)

---

## Recommendations

### Tier 1 — Critical (add immediately)

These materials have their **own V_min constant** producing meaningfully different physics. Tests should validate penetration depth, residual velocity, and jet disruption behavior.

1. **`ceramic_al2o3`** — V_MIN_CERAMIC (2000). The most common armor ceramic. Test penetration vs equivalent-thickness RHA.
2. **`ceramic_sic`** — V_MIN_CERAMIC (2000). Common in modern arrays. Verify V_min branch fires correctly.
3. **`ceramic_b4c`** — V_MIN_CERAMIC (2000). Lightest armor ceramic.
4. **`aluminum_5083`** — V_MIN_ALUMINUM (1500). Common light armor. Test that >250 mm RHA-equivalent penetration is not claimed.
5. **`aluminum_7075`** — V_MIN_ALUMINUM (1500). Higher-strength variant. Verify V_min=1500 branch.

### Tier 2 — High (add soon)

These materials match RHA's V_MIN_RHA (2500) but have different densities, affecting P/L ratio via the `√(ρⱼ/ρₜ)` term. Tests should validate density-driven penetration differences.

6. **`chobham_composite`** — density 2700 (vs 7850 for RHA). √(8960/2700) ≈ 1.82 vs √(8960/7850) ≈ 1.07 → ~70% more penetration predicted. This should be verified.
7. **`burlington_composite`** — M1 Abrams base armor.
8. **`depleted_uranium`** — density 19000. √(8960/19000) ≈ 0.69 vs 1.07 → ~36% less penetration predicted.
9. **`titanium_alloy`** — density 4430. Important for aircraft vulnerability models.

### Tier 3 — Medium (add when extending)

These exercise either edge-case code paths or are less commonly targeted by HEAT.

10. **`steel_high_hardness`** or `mil_dtl_46100_class1` — high-hardness steel with same V_min but same density.
11. **`rubber_elastomer`** — V_min=500 path, very different from RHA.
12. **`spall_liner`** — V_min=500 path.

### Recommended test structure

Each new test should follow the existing pattern — override `target_armor_material` and `target_density_kgm3` on a standard param set:

```rust
#[test]
fn heat_penetrates_ceramic_al2o3() {
    let result = evaluate_heat_jet(&HeatJetParams {
        target_armor_material: "ceramic_al2o3".to_string(),
        target_density_kgm3: 3500.0,
        ..rpg7_heat_params()
    });
    // Ceramic has lower V_min (2000 vs 2500) → more penetration at same velocity
    assert!(result.penetration_depth_mm > rpg7_baseline().penetration_depth_mm);
}
```

Suggested test count: **10–12 new tests** covering:
- 3 ceramic variations (Al2O3, SiC, B4C)
- 2 aluminum alloys (5083, 7075)
- 2 special armor composites (Chobham, Burlington)
- 1 DU insert
- 1 titanium alloy
- 1 rubber/elastomer (for V_min=500 path)
- 1 concrete (for V_min=1000 path)
- Possibly 1 composite verification (vs density lookup)

---

## Appendix: Complete Untested Material List

All material IDs with zero HEAT penetration test coverage:

```
ad90, ad95, boron_carbide, ceramic_ad90, ceramic_ad95, ceramic_al2o3, ceramic_b4c,
ceramic_plate, ceramic_sic, esapi_al2o3, esapi_b4c, esapi_sic, mar_ceramic,
silicon_carbide, arma_glass, burlington_composite, carbon_fiber, chobham_composite,
composite_glass, composite_kevlar, dorchester_composite, k_active_composite,
kvarts_composite, laminated_glass, mexas_composite, stanag_composite, stef_composite,
textolite_composite, era_kontakt5, era_light, era_relikt, malachite_era, relikt_era,
aluminum_5083, aluminum_7039, aluminum_7075, arma_aluminum, arma_default, arma_steel,
armor_tip_steel, armox_500, armox_600, cage_armor, cast_steel, depleted_uranium,
dual_hardness_steel, hardox_450, hha_steel, lead_alloy, mars_armor,
mil_dtl_46100_class1, mil_dtl_46100_class3, mil_dtl_46100_class4, mild_steel,
perforated_armor, slat_armor, slotted_armor, titanium_alloy, titanium_diboride,
tungsten_carbide, arma_plastic, arma_rubber, dyneema_liner, fiberglass, kevlar_liner,
spall_liner_kevlar, rubber_elastomer, rubber_solid, spall_liner, twaron_liner, uhmwpe
```

**Total: 71 materials untested (98.6%)**

---

## References

| Source File | Description |
|-------------|-------------|
| `arl_penetration_data.md` | 16 cataloged ARL/BRL test reports from DTIC: ARL-TR-4632 (UHH steel V50), ARL-TR-5182 (M855 aerodynamics), ADA362926 (ceramic armor database — DOP test data for Al₂O₃, SiC, B₄C, TiB₂), ARL-TR-3590 (interface defeat of long rods by ceramic armor), ARL-TR-7263 (KE characterization of advanced ceramics vs 12.7mm APM2), ARL-TR-4427 (AA5059 aluminum armor), ARL-TR-4996 (Ti-5553 titanium armor), ARL-TR-1146 (Ti-6Al-4V vs long rod), Stewart & Netherton 2019 (7.62mm M61 AP vs steel and HHA) |
| `armox_armor_data.md` | SSAB ARMOX product specifications (370T, 440T, 500T, 520T, 560T, 600T, 620T, Advance); MIL-DTL-46100E HHA (477–534 BHN); MIL-DTL-12560J RHA (Classes 1–4); VPAM/EN 1522 ballistic protection level thickness tables per threat; V50 test results from ARL-TR-4632; material density summary |
| `hornady_ballistic_data.md` | Hornady Manufacturing published BCs for all bullet lines (ELD Match, ELD-X, A-Tip, InterLock, SST, V-MAX); ICAO standard atmosphere reference conditions |
| `lapua_ballistic_data.md` | Nammo Lapua Doppler radar-measured G1/G7 BCs and trajectory tables for .338LM, .300WM, .308W, 6.5CM, 6.5×47 |
| `saami_cip_pressures.md` | SAAMI MAP, CIP Pmax, and NATO EPVAT chamber pressure specifications for 12 calibers; conversion factors and critical notes on pressure measurement methodology |
