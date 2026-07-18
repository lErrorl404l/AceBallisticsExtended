# abe-24-gaps - Work Plan

## TL;DR (For humans)
<!-- Fill this LAST, after the detailed plan below is written, so it summarizes the REAL plan. -->
<!-- Plain English for a non-engineer: NO file paths, NO todo numbers, NO wave/agent/tool names. -->

**What you'll get:** All 24 identified ballistic-model and penetration-data gaps filled — the simulation gains Eötvös correction, transonic drag perturbation, velocity-bounded BC switching, barrel-length-tuned muzzle velocity, ammo temperature sensitivity, spin decay, configurable form factor and aero coefficients, boat-tail base drag, humidity/precipitation effects, and seeded MV variation. The penetration model gets KE/CE RHAe vehicle schemas, backing plate support, ERA zone configs, APFSDS sabot data, EFP/tandem/API projectile data, weakpoint zones, NIJ and STANAG 4569 protection levels, and populated fragmentation/ricochet data for all 62 round types.

**Why this approach:** ~70% of the gaps are independent so they run in parallel waves — config schema first because all data files depend on it, then physics and data tasks fan out simultaneously. The penetration model already exceeds ACE3 fidelity; the 12 "penetration gaps" are mostly new JSON data with minimal code wiring.

**What it will NOT do:** Change the C ABI, penetration physics algorithms, existing JSON file formats, or the core step loop signature. No new crate dependencies.

**Effort:** XL (24 gaps, ~60 files touched)
**Risk:** Medium — the ballistic physics changes touch the core `abe_step` loop in lib.rs, and data population for 62 ammo types needs domain accuracy
**Decisions to sanity-check:** KE-CE RHAe default multiplier values, NIJ/STANAG threat velocity thresholds, transonic perturbation polynomial shape, spin decay time constant

Your next move: Approve this plan, then run `$start-work` to execute. Full execution detail follows below.

---

> TL;DR (machine): XL, Medium — 24 ballistic+penetration gaps in 6 parallel waves across config.rs, ballistics/*, penetration/*, systems/*, effects/*, data/ammo/, data/armor/. ~60 files touched. 1051 existing tests must pass.

## Scope
### Must have
- Ballistic variables: Eötvös, BC switching (v-bounded), transonic perturbation, barrel-length MV lookup, ammo temp shift table, MV variation (seeded RNG), spin decay, form factor i exposure, aero coeffs (CA/CY/CN), boat-tail base drag, humidity in step, precipitation in step
- Penetration data: LOS/KE-CE RHAe vehicle schema, backing plate layer-stacking, ERA per-zone config, APFSDS sabot data, EFP model data, tandem warhead timing data, API penetration data, weakpoints per vehicle, NIJ body armor levels, STANAG 4569 threat levels, populating ammo fragmentation data, populating ammo ricochet data
- All new struct fields use `#[serde(default)]` or `Option<>` for backward compat
- All ~1090 existing tests pass

### Must NOT have (guardrails, anti-slop, scope boundaries)
- No C ABI signature changes (abe_fire/abe_step/abe_impact)
- No new external crate dependencies
- No changes to existing JSON file format structures
- No penetration physics model rewrites
- No module refactors beyond targeted additions
- No breaking existing test behavior

## Verification strategy
> Zero human intervention - all verification is agent-executed.
- Test decision: tests-after (add test for each new feature inline in the module)
- Framework: built-in `#[test]` in each module
- Evidence: `cargo test 2>&1` must show all tests passing with zero failures
- Each agent task: write test that validates:
  - Happy path (new feature works with reasonable inputs)
  - Edge case (default/zero values don't crash — serde default guards)
  - Backward compat (existing data files still deserialize)

## Execution strategy
### Parallel execution waves
> Target 5-8 todos per wave. Wave 1 is sequential (schema first), then everything fans out.

**Wave 1: Config schema extensions** — must complete before Wave 2+5 (data files need schema). 6 parallel agents.
**Wave 2: Penetration data files** — depends on Wave 1. 5 parallel agents.
**Wave 3: Ballistic physics code** — no penetration deps. 10 parallel agents.
**Wave 4: Penetration code extensions** — depends on Wave 1. 5 parallel agents.
**Wave 5: Ammo data population** — depends on Wave 1. 2 parallel agents.
**Wave 6: Integration + verification** — all prior waves complete. 1 agent.

### Dependency matrix
| Todo | Depends on | Blocks | Can parallelize with |
| --- | --- | --- | --- |
| W1-1 through W1-6 | nothing | W2, W4, W5 | all W1 todos |
| W2-1 through W2-5 | W1 | W6 | all W2, W3, W4, W5 todos |
| W3-1 through W3-10 | nothing | W6 | all W1, W2, W3, W4, W5 todos |
| W4-1 through W4-5 | W1 | W6 | all W2, W3, W4, W5 todos |
| W5-1, W5-2 | W1 | W6 | all W2, W3, W4, W5 todos |
| W6 | W2+W3+W4+W5 | nothing | nothing |

## Todos
> Implementation + Test = ONE todo. Never separate.
<!-- APPEND TASK BATCHES BELOW THIS LINE WITH edit/apply_patch - never rewrite the headers above. -->

### Wave 1 — Config schema extensions (must run first, then all other waves parallel)

- [ ] 1. Extend ProjectileConfig with APFSDS, EFP, tandem warhead, API penetration fields
  What to do: Add Option<f64> fields to ProjectileConfig in ext/src/systems/config.rs for sabot_mass_g, sabot_discard_velocity_ms (APFSDS), efp_standoff_mm, efp_velocity_ms, efp_mass_g (EFP), tandem_charge_delay_us, tandem_charge_separation_mm (tandem), api_core_mass_g, api_core_caliber_mm (API). All `#[serde(default)]`.
  Must NOT do: No required fields. No changes to existing serde derives. No new modules.
  Parallelization: Wave 1 | Blocked by: — | Blocks: W2/W4/W5
  References: ext/src/systems/config.rs (ProjectileConfig struct ~line 30-60)
  Acceptance: `cargo build` succeeds, `cargo test` passes. New fields present in parse output.
  Commit: Y | feat(schema): add APFSDS/EFP/tandem/API fields to ProjectileConfig

- [ ] 2. Extend ArmorPlate with KE/CE RHAe multipliers, backing plate fields, weakpoints
  What to do: Add to ArmorPlate in config.rs: ke_rhae: f64 = 1.0, ce_rhae: f64 = 1.0, backing_material: Option<String>, backing_thickness_mm: Option<f64>. Add new struct VehicleWeakpoints { zones: Vec<WeakpointZone> } where WeakpointZone { name: String, relative_thickness: f64, area_ratio: f64 }. Add weakpoints: Option<VehicleWeakpoints> to ArmorConfig.
  Must NOT do: No required fields. No changes to existing armor JSONs.
  Parallelization: Wave 1 | Blocked by: — | Blocks: W2/W4/W5
  References: ext/src/systems/config.rs (ArmorPlate ~line 80, ArmorConfig ~line 70)
  Acceptance: `cargo build` succeeds. New struct compiles.
  Commit: Y | feat(schema): add KE/CE RHAe, backing plate, weakpoints to armor schema

- [ ] 3. Add EraZoneConfig struct and wire to predictive_era config
  What to do: Create EraZoneConfig { zone_name: String, coverage_angle_start_deg: f64, coverage_angle_end_deg: f64, era_thickness_mm: f64, era_material: String, era_density_gcc: f64 }. Add era_zones: Option<Vec<EraZoneConfig>> to a new or existing config. Wire to ext/src/systems/predictive_era.rs.
  Must NOT do: No changes to existing ERA calculation API.
  Parallelization: Wave 1 | Blocked by: — | Blocks: W2/W4/W5
  References: ext/src/systems/predictive_era.rs, ext/src/systems/config.rs
  Acceptance: `cargo build` succeeds.
  Commit: Y | feat(schema): add EraZoneConfig struct and wiring

- [ ] 4. Add NIJ body armor level enum + config struct
  What to do: Add enum NIJLevel { IIA, II, IIIA, III, IV } with serde_repr. Add BodyArmorConfig { nij_level: NIJLevel, threat_velocity_ms: f64, coverage: f64 }. Add to config loading.
  Must NOT do: No changes to effects/body_armor.rs function signatures.
  Parallelization: Wave 1 | Blocked by: — | Blocks: W2/W4/W5
  References: ext/src/effects/body_armor.rs, ext/src/systems/config.rs
  Acceptance: `cargo build` succeeds. NIJLevel round-trips through serde.
  Commit: Y | feat(schema): add NIJ body armor level enum

- [ ] 5. Add STANAG 4569 threat level enum + config struct
  What to do: Add enum StanagLevel { L1, L2, L3, L4, L5 } with KE/CE threshold table (hardcoded constants in barrier_penetration.rs). StanagProtectionConfig { level: StanagLevel, ke_mj: f64, ce_mj: f64 }.
  Must NOT do: No changes to penetration algorithm — just threshold data.
  Parallelization: Wave 1 | Blocked by: — | Blocks: W2/W4/W5
  References: ext/src/penetration/barrier_penetration.rs, Stanag 4569 std thresholds
  Acceptance: `cargo build` succeeds. Enum round-trips through serde.
  Commit: Y | feat(schema): add STANAG 4569 level enum

- [ ] 6. Add ballistic-specialized fields to config (form factor i, aero coeffs, BC switching, boat-tail)
  What to do: Add to ProjectileConfig: form_factor_i: Option<f64>, ca: Option<f64>, cy: Option<f64>, cn: Option<f64>, boat_tail_angle_deg: Option<f64>, boat_tail_length_mm: Option<f64>, bc_switch_velocity_ms: Option<f64>, bc_switch_value: Option<f64>. Add to WeaponConfig: mv_table: Option<Vec<MvTableEntry>> { temp_c: f64, mv_ms: f64 }, barrel_correction_mm_per_ms: Option<f64>. All `#[serde(default)]`.
  Must NOT do: No required fields, no existing field removal.
  Parallelization: Wave 1 | Blocked by: — | Blocks: W3/W5
  References: ext/src/systems/config.rs (ProjectileConfig, WeaponConfig structs)
  Acceptance: `cargo build` succeeds. `cargo test` passes.
  Commit: Y | feat(schema): add ballistic model fields to ammo/weapon configs

### Wave 2 — Penetration data files (parallel, depends on Wave 1)

- [ ] 7. Create KE/CE RHAe vehicle armor JSONs
  What to do: For each of ~20 vehicle JSONs in data/armor/, add ke_rhae: 1.0 and ce_rhae: 1.0 to each ArmorPlate entry (placeholder — real values from ACE3/REALREF). Add optional backing_material, backing_thickness_mm where appropriate (e.g., M1A1 composite = ceramic + RHA backing).
  Must NOT do: Don't change existing plate dimensions/angles — only add new fields.
  Parallelization: Wave 2 | Blocked by: W1-2 | Blocks: W6
  References: data/armor/*.json (20 vehicle files), ext/src/systems/config.rs ArmorPlate struct
  Acceptance: `cargo test all_armor_jsons_load` passes.
  Commit: Y | feat(data): add KE/CE RHAe and backing fields to vehicle armor JSONs

- [ ] 8. Create ERA zone config JSONs
  What to do: Create data/era_zones/ directory. For vehicles with ERA (e.g., T-90A, BMP-2), create JSON files with EraZoneConfig entries: angle coverage per zone (e.g., turret front 315-45°, hull front 315-45°), era material Kontakt-5/Relikt, thickness, density.
  Must NOT do: No changes to ERA calculation code yet — that's Wave 4.
  Parallelization: Wave 2 | Blocked by: W1-3 | Blocks: W6
  References: data/armor/*.json for existing vehicle dimensions
  Acceptance: New JSONs deserialize into EraZoneConfig struct.
  Commit: Y | feat(data): add ERA zone configs for ERA-equipped vehicles

- [ ] 9. Create NIJ body armor config JSONs
  What to do: Create data/armor/body_armor_nij.json with BodyArmorConfig entries for each NIJ level IIA through IV. Include per-level: V50 threat velocity, coverage factor, material composition reference.
  Must NOT do: No NIJ level I (obsolete), no special/specialized ratings.
  Parallelization: Wave 2 | Blocked by: W1-4 | Blocks: W6
  References: NIJ Standard-0101.06, ext/src/effects/body_armor.rs
  Acceptance: New JSON deserializes. `cargo test` passes.
  Commit: Y | feat(data): add NIJ body armor threat level configs

- [ ] 10. Create STANAG 4569 protection level config JSONs
  What to do: Create data/armor/stanag_4569.json with StanagProtectionConfig entries for levels L1-L5. Each level: KE threat (MJ at muzzle), CE threat (MJ), typical projectile, range.
  Must NOT do: No changes to barrier_penetration.rs thresholds — use hardcoded constants.
  Parallelization: Wave 2 | Blocked by: W1-5 | Blocks: W6
  References: STANAG 4569 Edition 3 tables, ext/src/penetration/barrier_penetration.rs
  Acceptance: New JSON deserializes.
  Commit: Y | feat(data): add STANAG 4569 protection level configs

- [ ] 11. Create ammo data JSONs for APFSDS, EFP, tandem, API projectiles
  What to do: Add sabot_mass_g etc. to APFSDS-type ammo JSONs (e.g., m829a1.json in data/armor/ or inline in ammo). Add efp_* to EFP-type (tank HEAT-MP, claymore). Add tandem_* to tandem HEAT (rpg-29vampir). Add api_* to API rounds (m993, m995). If these ammo types don't have individual JSONs yet — create them.
  Must NOT do: No modifications to existing non-APFSDS/EFP/tandem/API ammo files.
  Parallelization: Wave 2 | Blocked by: W1-1 | Blocks: W6
  References: data/ammo/*.json (existing APFSDS-adjacent rounds)
  Acceptance: New/existing JSONs deserialize with new fields present.
  Commit: Y | feat(data): add APFSDS/EFP/tandem/API penetration data to ammo configs

### Wave 3 — Ballistic physics code (fully parallel, no deps)

- [ ] 12. Add Eötvös effect to dof.rs
  What to do: In dof.rs, add eotvos_acceleration(lat_rad, vel_east_ms) -> Vec3 that computes 2 * omega_earth * vel_east_ms * (cos(lat), 0, -sin(lat)). Call from abe_step (lib.rs ~line 920) and add to total acceleration. Add test: at equator, 1000 m/s east = ~0.145 m/s² upward.
  Must NOT do: No Coriolis refactoring — Eötvös is a separate additive term.
  Parallelization: Wave 3 | Blocked by: — | Blocks: W6
  References: ext/src/ballistics/dof.rs (existing magnus_acceleration, Coriolis), ext/src/lib.rs (~line 920 acceleration sum)
  Acceptance: `cargo test` passes. Eötvös unit test validates magnitude at equator.
  Commit: Y | feat(ballistics): add Eötvös effect to dof acceleration

- [ ] 13. Add spin decay to dof.rs
  What to do: Add spin_decay(spin_rate, time_s, decay_constant) -> f64. Default decay constant from rifling twist + air density. Call in abe_step after Magnus calculation. Add test: decay over 1s at sea level reduces spin ~2-5%.
  Must NOT do: No change to existing Magnus calculation.
  Parallelization: Wave 3 | Blocked by: — | Blocks: W6
  References: ext/src/ballistics/dof.rs, ext/src/lib.rs (~line 902)
  Acceptance: `cargo test` passes. Spin decay within expected range.
  Commit: Y | feat(ballistics): add spin decay model to dof

- [ ] 14. Add transonic perturbation to drag.rs
  What to do: Add transonic_perturbation(mach) -> f64 that computes a CD boost factor centered near mach 0.9-1.1. Use polynomial model (CD_boost = a*(mach-1)^2 + b for 0.8 < mach < 1.2, else 0). Apply to drag coefficient in get_cd() when mach in transonic range. Add test: CD boost peaks at mach ~0.98 with ~30% increase.
  Must NOT do: No change to subsonic/supersonic CD tables.
  Parallelization: Wave 3 | Blocked by: — | Blocks: W6
  References: ext/src/ballistics/drag.rs (get_cd function)
  Acceptance: `cargo test` passes. Transonic test validates peak position and magnitude.
  Commit: Y | feat(ballistics): add transonic CD perturbation to drag model

- [ ] 15. Add velocity-bounded BC switching to drag.rs
  What to do: Add bc_at_velocity(mach, bc_base, switch_vel_ms, bc_switch_value) -> f64. When velocity crosses switch_vel_ms threshold, interpolate toward bc_switch_value. Wire to new bc_switch_velocity_ms / bc_switch_value fields on ProjectileConfig. Add test: BC transitions smoothly at switch velocity.
  Must NOT do: No change to existing bc_at_mach function — this is an additional correction.
  Parallelization: Wave 3 | Blocked by: — | Blocks: W6
  References: ext/src/ballistics/drag.rs (bc_at_mach), config.rs (new fields)
  Acceptance: `cargo test` passes. BC switch test validates interpolation.
  Commit: Y | feat(ballistics): add velocity-bounded BC switching

- [ ] 16. Add barrel-length MV lookup to interior.rs
  What to do: Add mv_from_barrel_length(barrel_length_mm, caliber_mm, chamber_pressure_mpa, ammo_caliber_mm) -> f64. Use powder burn model: MV proportional to sqrt(barrel_length) with charge-to-mass ratio adjustment. Wire to WeaponConfig.barrel_correction_mm_per_ms when set. Add test: 508mm vs 368mm M4 barrel gives ~80 m/s MV difference for M855.
  Must NOT do: No change to existing interior.rs API.
  Parallelization: Wave 3 | Blocked by: — | Blocks: W6
  References: ext/src/ballistics/interior.rs, ext/src/systems/config.rs (WeaponConfig)
  Acceptance: `cargo test` passes. Barrel-length correction within 5% of real data.
  Commit: Y | feat(ballistics): add barrel-length MV interpolation to interior ballistics

- [ ] 17. Add ammo temperature shift table to mv_temperature.rs
  What to do: Add temp_shift_table(temp_c, mv_ref_ms, table: &[(f64, f64)]) -> f64. If MvTableEntry data present in WeaponConfig, interpolate. Otherwise fall back to existing temp_coefficient model. Add test: table lookup with 4 entry points vs linear interpolation.
  Must NOT do: No removal of existing temp_coefficient model — table is additive first-class path.
  Parallelization: Wave 3 | Blocked by: — | Blocks: W6
  References: ext/src/ballistics/mv_temperature.rs, config.rs (WeaponConfig.mv_table)
  Acceptance: `cargo test` passes. Table-based correction matches interpolation math.
  Commit: Y | feat(ballistics): add ammo temperature shift table to MV model

- [ ] 18. Add seeded MV variation to lot_variation.rs
  What to do: Add mv_variation(baseline_mv_ms, lot_std_ms, seed: u64) -> f64. Use deterministic RNG (std::hash::Hasher or simple LCG seeded from seed). Wire to existing lot variation system. Add test: same seed produces same MV, different seed produces different MV within ±3σ.
  Must NOT do: No external RNG crate. No thread_rng — must be deterministic from seed.
  Parallelization: Wave 3 | Blocked by: — | Blocks: W6
  References: ext/src/systems/lot_variation.rs
  Acceptance: `cargo test` passes. Deterministic MV test validates reproducibility.
  Commit: Y | feat(systems): add seeded MV variation to lot_variation

- [ ] 19. Expose form factor i and add configurable aero coefficients (CA/CY/CN) to ballistic_cap.rs
  What to do: Add i_field(mass_g, caliber_mm, bc_g7) -> f64 in ballistic_cap.rs. Add aero_forces(ca, cy, cn, mach, dynamic_pressure, area) -> Vec3 in ballistic_cap.rs. Wire to ProjectileConfig.form_factor_i, .ca, .cy, .cn. When not set, use default i = 1.0 and skip aero force adjustment. Add test: i = 0.9 reduces drag by 10%.
  Must NOT do: No change to existing BC-based drag path when new fields are absent.
  Parallelization: Wave 3 | Blocked by: — | Blocks: W6
  References: ext/src/ballistics/ballistic_cap.rs, ext/src/systems/config.rs
  Acceptance: `cargo test` passes. Form factor and aero coef tests validate.
  Commit: Y | feat(ballistics): add form factor i exposure and configurable aero coefficients

- [ ] 20. Add boat-tail base drag to exterior.rs
  What to do: Add boat_tail_drag_correction(boat_tail_angle_deg, boat_tail_length_mm, caliber_mm, mach) -> f64. Base drag reduction = f(angle) * f(length/caliber). Wire to ProjectileConfig fields. Apply as multiplier to CD in abe_step. Add test: 7° boat tail on 5.56mm reduces base drag ~15% at mach 2.
  Must NOT do: No change to zero-angle (flat base) drag path.
  Parallelization: Wave 3 | Blocked by: — | Blocks: W6
  References: ext/src/ballistics/exterior.rs, ext/src/lib.rs (abe_step)
  Acceptance: `cargo test` passes. Boat-tail correction in expected range.
  Commit: Y | feat(ballistics): add boat-tail base drag correction to exterior ballistics

- [ ] 21. Add humidity and precipitation effects to atmosphere.rs + lib.rs step
  What to do: Add humidity_factor(rh_percent, temp_c, altitude_m) -> f64 (humidity reduces air density slightly). Add precipitation_drag(rain_rate_mmhr, caliber_mm, vel_ms) -> f64 (rain droplets impart drag impulse). Wire both into atmosphere module and call from abe_step. Add test: 100% humidity reduces density ~0.5% at 20°C. Heavy rain (50 mm/hr) adds ~1% drag at 800 m/s.
  Must NOT do: No change to existing density/pressure calculations — humidity is a correction factor.
  Parallelization: Wave 3 | Blocked by: — | Blocks: W6
  References: ext/src/ballistics/atmosphere.rs, ext/src/lib.rs (~line 835 density call)
  Acceptance: `cargo test` passes. Humidity and precipitation tests within expected magnitude.
  Commit: Y | feat(ballistics): add humidity and precipitation effects to atmosphere model

### Wave 4 — Penetration code extensions (parallel, depends on Wave 1 schema)

- [ ] 22. Wire backing plate layer-stacking in armor_array.rs
  What to do: In armor_array.rs, extend layer evaluation to recursively handle backing plates. When ArmorPlate has backing_material + backing_thickness_mm, evaluate as two layers with the backing reducing residual energy. Add test: 50mm RHA + 20mm Al backing stops same projectile 5% less deep than 70mm monolithic RHA.
  Must NOT do: No change to existing armor_array API — backing is additive logic.
  Parallelization: Wave 4 | Blocked by: W1-2 | Blocks: W6
  References: ext/src/penetration/armor_array.rs, config.rs (ArmorPlate backing field)
  Acceptance: `cargo test` passes. Backing plate test validates reduction vs monolithic.
  Commit: Y | feat(penetration): add backing plate layer-stacking to armor array evaluation

- [ ] 23. Wire ERA per-zone config in predictive_era.rs
  What to do: In predictive_era.rs, read EraZoneConfig list, match impact angle against zone coverage. When impact falls within a zone, use zone-specific ERA material/thickness instead of global default. Add test: hull-front vs turret-side ERA gives different disruption for same projectile.
  Must NOT do: No change to ERA disruption physics — only zone-based selection.
  Parallelization: Wave 4 | Blocked by: W1-3 | Blocks: W6
  References: ext/src/systems/predictive_era.rs, config.rs (EraZoneConfig struct)
  Acceptance: `cargo test` passes. ERA zone selection test validates angle matching.
  Commit: Y | feat(systems): wire per-zone ERA configuration into predictive_era

- [ ] 24. Wire NIJ body armor levels in body_armor.rs
  What to do: In body_armor.rs, add evaluate_nij_protection(nij_level, projectile_ke_j, velocity_ms) -> ProtectionResult. V50 threshold comparison: if velocity < V50 → stop, else partial/full penetration. Wire to BodyArmorConfig when present. Add test: NIJ IIIA stops .44 Magnum at 450 m/s (V50 table), fails for .308 at 850 m/s.
  Must NOT do: No change to existing body armor evaluation path.
  Parallelization: Wave 4 | Blocked by: W1-4 | Blocks: W6
  References: ext/src/effects/body_armor.rs, config.rs (NIJ Level + BodyArmorConfig)
  Acceptance: `cargo test` passes. NIJ level protection test matches standard thresholds.
  Commit: Y | feat(effects): wire NIJ body armor level protection evaluation

- [ ] 25. Wire STANAG 4569 protection in barrier_penetration.rs
  What to do: In barrier_penetration.rs, add evaluate_stanag_protection(stanag_level, projectile_ke_mj, projectile_type) -> bool. Compare projectile KE/CE at impact against STANAG per-level thresholds. Add test: 7.62x51 AP (3.5 kJ) penetrates L1 but not L2.
  Must NOT do: No change to existing barrier penetration algorithm — STANAG is an overlay check.
  Parallelization: Wave 4 | Blocked by: W1-5 | Blocks: W6
  References: ext/src/penetration/barrier_penetration.rs, STANAG 4569 thresholds
  Acceptance: `cargo test` passes. STANAG level test matches standard.
  Commit: Y | feat(penetration): add STANAG 4569 protection level evaluation

- [ ] 26. Wire weakpoint zone evaluation in penetration.rs
  What to do: In penetration.rs, after choosing impact point, check ImpactParams against VehicleWeakpoints zones. If impact falls within a weakpoint zone, apply relative_thickness modifier to armor plate. Add test: hull MG port weakpoint (0.3x relative thickness) sees 70% reduced effective armor.
  Must NOT do: No change to base penetration algorithm — thickness modifier only.
  Parallelization: Wave 4 | Blocked by: W1-2 | Blocks: W6
  References: ext/src/penetration/penetration.rs, config.rs (VehicleWeakpoints)
  Acceptance: `cargo test` passes. Weakpoint modifier test validates reduced effective thickness.
  Commit: Y | feat(penetration): add weakpoint zone evaluation with thickness modifiers

### Wave 5 — Ammo data population (parallel, depends on Wave 1 schema)

- [ ] 27. Populate fragmentation data for all 62 ammo JSONs
  What to do: For each of 62 ammo JSONs in data/ammo/, add or update fragmentation fields: frag_mass_mean (mean fragment mass in g), frag_mass_std (std dev). Use caliber-based estimates: pistol (~0.5g mean), rifle (~1.5g mean for M855), fragmenting rounds (higher). Estimate std = mean * 0.5.
  Must NOT do: No changes to schema or non-frag fields. No deletion of existing data.
  Parallelization: Wave 5 | Blocked by: W1 | Blocks: W6
  References: data/ammo/*.json (62 files), ext/src/systems/config.rs (ProjectileConfig frag fields)
  Acceptance: `cargo test all_ammo_jsons_load` passes.
  Commit: Y | feat(data): populate fragmentation mass data for all ammo configs

- [ ] 28. Populate ricochet angle data for all 62 ammo JSONs
  What to do: For each of 62 ammo JSONs, add ricochet_angle_deg: based on projectile type: FMJ ~20°, AP ~15°, hollow-point ~25°, shotgun slug ~30°, HEAT ~35°. Use type-class logic to assign reasonable defaults.
  Must NOT do: No changes to schema or non-ricochet fields.
  Parallelization: Wave 5 | Blocked by: W1 | Blocks: W6
  References: data/ammo/*.json (62 files), ext/src/systems/config.rs (ricochet_angle_deg)
  Acceptance: `cargo test all_ammo_jsons_load` passes.
  Commit: Y | feat(data): populate ricochet angle data for all ammo configs

### Wave 6 — Integration + verification (all prior waves complete)

- [ ] 29. Integration verification — full test pass, compile check, docs compile
  What to do: Run `cargo test`, verify all 1051+ tests pass. Run `cargo clippy -- -D warnings`, verify clean. Verify new JSON files in data/armor/ and data/ammo/ deserialize correctly. Check that all new fields in config.rs are accessible from their respective modules.
  Must NOT do: No code changes — verification only. If tests fail, report failures; do NOT fix in this step.
  Parallelization: Wave 6 | Blocked by: W2+W3+W4+W5 | Blocks: nothing
  References: entire codebase
  Acceptance: `cargo test` exits 0, `cargo clippy -- -D warnings` exits 0.
  Commit: N (verification pass — commits per-todo above)

## Final verification wave
> Runs in parallel after ALL todos. ALL must APPROVE. Surface results and wait for the user's explicit okay before declaring complete.
- [ ] F1. Plan compliance audit — all 24 gaps addressed, no omissions
- [ ] F2. Code quality review — no `as any`, unwrap, or suppressed errors
- [ ] F3. Full test pass — `cargo test` zero failures
- [ ] F4. Scope fidelity — no C ABI changes, no penetration algo rewrites, no crate deps

## Commit strategy
- One commit per todo above (29 commits total)
- All commits to main
- Commit messages follow semantic style: `feat(schema):`, `feat(data):`, `feat(ballistics):`, `feat(penetration):`, `feat(effects):`, `feat(systems):`
- Each commit includes `Ultraworked with [Sisyphus](https://github.com/code-yeongyu/oh-my-openagent)` footer

## Success criteria
- All 24 gaps implemented with associated tests
- `cargo test` passes (1051+ tests)
- `cargo clippy -- -D warnings` clean
- All new JSON files deserialize correctly
- No breaking changes to existing data or C ABI
- KE/CE RHAe, backing plate, ERA zones, APFSDS, EFP, tandem, API, weakpoints, NIJ, STANAG — all wired end-to-end
