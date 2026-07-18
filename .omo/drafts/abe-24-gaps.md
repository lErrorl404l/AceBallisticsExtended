---
slug: abe-24-gaps
status: awaiting-approval
intent: clear
pending-action: write .omo/plans/abe-24-gaps.md
approach: Waves of parallel deep agents — Phase A (data schema + JSON data), Phase B (ballistic physics code), Phase C (integration + verification)
---

# Draft: abe-24-gaps

## Components (topology ledger)
<!-- id | outcome (one line) | status: active|deferred | evidence path -->

| ID | Component | Status | Evidence |
|----|-----------|--------|----------|
| C1 | Ballistic cap / form factor / aero coeff / boat-tail | active | ext/src/ballistics/ballistic_cap.rs, exterior.rs |
| C2 | BC switching (velocity-bounded) | active | ext/src/ballistics/drag.rs |
| C3 | Transonic perturbation | active | ext/src/ballistics/drag.rs |
| C4 | Barrel-length MV lookup | active | ext/src/ballistics/interior.rs + data/schemas/weapon_schema.json |
| C5 | Ammo temp shift table | active | ext/src/ballistics/mv_temperature.rs |
| C6 | MV variation (seeded RNG) | active | ext/src/systems/lot_variation.rs |
| C7 | Spin decay | active | ext/src/ballistics/dof.rs |
| C8 | Eötvös effect | active | ext/src/ballistics/dof.rs |
| C9 | Humidity + precipitation in core step | active | ext/src/ballistics/atmosphere.rs + ext/src/lib.rs (abe_step) |
| D1 | LOS/KE-CE RHAe vehicle schema | active | ext/src/systems/config.rs + data/armor/ |
| D2 | Backing plate layer-stacking | active | ext/src/penetration/armor_array.rs + config.rs |
| D3 | ERA per-zone config | active | ext/src/systems/predictive_era.rs + config.rs |
| D4 | APFSDS sabot data | active | ext/src/systems/config.rs + data/ammo/ |
| D5 | EFP model data | active | config.rs + data/ammo/ |
| D6 | Tandem warhead timing | active | config.rs + data/ammo/ |
| D7 | API penetration | active | config.rs + data/ammo/ |
| D8 | Weakpoints per vehicle | active | config.rs + data/armor/ |
| D9 | NIJ body armor levels | active | ext/src/effects/body_armor.rs + config.rs |
| D10 | STANAG 4569 | active | ext/src/penetration/barrier_penetration.rs |
| D11 | Sparse ammo fragmentation data | active | data/ammo/ (62 JSON files) |
| D12 | Sparse ammo ricochet data | active | data/ammo/ (62 JSON files) |

## Open assumptions (announced defaults)
<!-- Record any default you adopt instead of asking, so the user can veto it at the gate. -->

| Assumption | Default | Rationale | Reversible? |
|------------|---------|-----------|-------------|
| Schema changes use `#[serde(default)]` | New fields are Option/with default | Backward compat with 62 existing ammo JSONs | Yes (can make required later) |
| KE-CE RHAe is f64 multiplier per plate | `ke_rhae: f64 = 1.0`, `ce_rhae: f64 = 1.0` | Neutral default that doesn't change existing behavior | Yes |
| NIJ levels use threat velocity thresholds | Enum mapping level→V50 | Directly implementable from NIJ std tables | Yes |
| STANAG 4569 uses level→KE/CE thresholds | Enum mapping | STANAG defines per-level protection KE/CE in MJ | Yes |
| BC switching uses velocity-breakpoint model | `switch_vel_ms: f64` per transition | Simple, predictable | Yes |
| Transonic perturbation uses polynomial CD boost | `drag::transonic_boost(mach)` | Established aerodynamics model | Yes |
| Eötvös = 2ω×v (horizontal coriolis on vertical) | Already have Coriolis, just adding vertical term | Physics standard | Yes |
| Spin decay = exponential relaxation | `spin *= exp(-k * t)` | Common approximation | Yes |

## Findings (cited - path:lines)

### Module structure (from explore bg_8398c9d4)
- `ext/src/ballistics/` (10 files): ballistic_cap.rs, barrel_harmonics.rs, mv_temperature.rs, stability.rs, drag.rs, exterior.rs, interior.rs, atmosphere.rs, dof.rs, mod.rs
- `ext/src/penetration/` (11 files): armor_array.rs, barrier_penetration.rs, behind_armor_debris.rs, fragmentation.rs, heat_penetration.rs, hesh_penetration.rs, long_rod.rs, multi_bounce.rs, penetration.rs, sequential_hits.rs, mod.rs
- `ext/src/systems/config.rs` (415 lines) — Rust structs: AmmoConfig, ProjectileConfig, FragmentationConfig, ArmorConfig, ArmorPlate, WeaponConfig
- `ext/src/systems/lot_variation.rs` — RNG variation exists, can extend for MV
- `ext/src/systems/predictive_era.rs` — ERA model exists, can extend for per-zone config
- `ext/src/effects/body_armor.rs` — body armor exists, can extend with NIJ levels
- `ext/src/penetration/armor_array.rs` — armor array stacking exists, can extend for backing + LOS
- `ext/src/ballistics/dof.rs` — magnus_acceleration, Coriolis exist; need spin decay + Eötvös
- `ext/src/ballistics/drag.rs` — get_cd, bc_at_mach exist; need transonic perturbation + BC switching
- `ext/src/ballistics/ballistic_cap.rs` — form factor exists but not exposed; need i field + CA/CY/CN
- `ext/src/ballistics/exterior.rs` — calc_mach, speed_of_sound exist; boat-tail drag not handled
- `ext/src/ballistics/mv_temperature.rs` — temp shift exists; need table-based lookup
- `ext/src/ballistics/interior.rs` — interior ballistics; need barrel-length MV lookup
- `ext/src/ballistics/atmosphere.rs` — density, pressure, temp, wind_shear exist; need humidity
- `ext/src/lib.rs` — abe_step (lines 815-943) = main physics loop; step params struct; bullet state

### Test infrastructure (from explore bg_06f3899c)
- ~1090 total tests (1051 unit + 39 integration)
- 5 test files under ext/tests/ including cabi, edge_case, sqf_compat
- CI: `cargo test` on push, also `cargo clippy -- -D warnings`
- Test command: `cd /ext/Development/AceBallisticsExtention/ext && cargo test`
- No cached pass/fail — compile succeeds (debug artifacts at Jul 17 23:02)

### Data structure (from explore bg_db17be23)
- `data/schemas/*.json` are STALE — real schema = Rust structs in config.rs
- 62 ammo JSONs in `data/ammo/` each with `class` + `projectile` sub-object
- 48 armor JSONs in `data/armor/` (20 vehicles + 28 materials)
- No `data/vehicles/` directory
- All existing ammo JSONs deserialize with `#[serde(default)]` on optional fields
- WeaponConfig has: class, caliber_mm, barrel_length_mm, rifling_twist_mm, chamber_pressure_mpa, cdm_id, muzzle_velocity_ms, zero_range_m
- ProjectileConfig has: model, mass_g, caliber_mm, bc_g7, cdm_id, fragmentation(opt), frag_mass_mean(opt), frag_mass_std(opt), ricochet_angle_deg(opt), tracer_burn_time_s(opt), incendiary(opt), incendiary_ignition_temp_k(opt)

### Penetration model insight
- ABE already exceeds ACE3 penetration fidelity (from librarian bg_bdfd8fd8)
- The 12 "penetration data gaps" are DATA gaps, not model gaps — the penetration model handles LOS, ERA, backing plates, etc. It just lacks the JSON config entries
- APFSDS, EFP, tandem warhead are ammo-level properties that modify how penetration.rs evaluates impact — small code changes but mostly data

## Decisions (with rationale)

1. **Group into 6 parallel waves** — based on dependency analysis:
   - Wave 1: Config struct extensions (all new fields with `#[serde(default)]`)
   - Wave 2: Penetration data JSON files (independent — can start right after schema)
   - Wave 3: Ballistic physics code (no penetration deps)
   - Wave 4: Penetration code extensions (backing plate, ERA config, NIJ, STANAG)
   - Wave 5: Ammo data population (frag + ricochet values for all 62 rounds)
   - Wave 6: Integration + test pass

2. **Each gap = one agent task** — parallelizable within waves

3. **Core physics step lib.rs changes are NOT auto-delegated** — they touch the critical integration point and need review

4. **New fields are `Option<T>` or `#[serde(default)]`** — zero breakage of existing 62 ammo JSONs and 48 armor JSONs

## Scope IN

All 24 gaps (12 ballistic variable + 12 penetration data):
- Ballistic: Eötvös, BC switching, transonic perturbation, barrel-length MV, temp table, MV RNG, spin decay, form factor i, aero coeff CA/CY/CN, boat-tail drag, humidity, precipitation
- Penetration data: LOS/KE-CE RHAe, backing plate, ERA zone config, APFSDS sabot, EFP, tandem warhead, API, weakpoints, NIJ levels, STANAG 4569, ammo frag data, ammo ricochet data

## Scope OUT (Must NOT have)

- No new penetration physics models (only data schema + wiring)
- No refactoring of existing modules (only additions)
- No changes to existing JSON file formats (only new fields with defaults)
- No changes to C ABI (abe_fire/abe_step/abe_impact signatures stay)
- No new external crate dependencies (use std/Core math only)
- No breaking existing test behavior (all ~1090 tests must pass)

## Open questions

None — all exploration is complete. Defaults adopted and recorded above.

## Approval gate
status: drafting
