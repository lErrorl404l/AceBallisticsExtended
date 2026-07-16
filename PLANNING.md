# ABE — Advanced Ballistics Extension

**Project**: Advanced Ballistics Extension (ABE)  
**Repo**: `/home/matt/Development/AceBallisticsExtention`  
**Status**: Planning phase  
**Target**: ARMA 3 (Reforger-ready design consideration)  
**Build system**: HEMTT + Cargo  
**Testing**: Headless-first, Rust-native + HEMTT + ACT CI/CD

---

## 1. Philosophy

ABE is a **data-driven ballistics framework** — the physics lives in a Rust extension, the
configuration lives in JSON/HPP data tables, and the SQF layer is thin orchestration.
ACE3 compatibility is a deployment mode, not an architectural dependency.

### Design tenets

1. **Everything testable headless**. The Rust extension is pure library code with no ARMA 3
   dependency — `cargo test` covers all physics. SQF modules are unit-testable via HEMTT's
   test runner. Integration tests use ACT in CI only.
2. **Data over code**. Adding a new weapon, round, or armor array is a JSON file + one class
   config, never a recompile. Community contributions are data PRs.
3. **ACE3-first, standalone-always**. When ACE3 is loaded, ABE hooks into ACE3's bullet tracking
   and enriches it. Without ACE3, ABE runs its own SQF framework. Same extension binary, same
   physics, different dispatch.
4. **Toggleable modules**. Each ballistics domain (interior, terminal, penetration, ricochet,
   environment, degradation, etc.) is an independent PBO that can be disabled. Players/server
   hosts choose depth vs performance.
5. **Verifiable by replay**. Every bullet's flight can be logged and replayed against the
   reference trajectory to validate that a build didn't regress physics.

---

## 2. Module Architecture

### Extension layer (Rust)

```
abe_ballistics_ext.[dll/so]
  ├── init(api_version, ace_present) -> bool
  ├── fire(params: FireParams) -> FireResult        // Interior ballistics
  ├── step(state: BulletState, dt: f64, env: EnvState) -> BulletState  // External step
  ├── impact(state: BulletState, target: TargetData) -> ImpactResult    // Terminal/penetration
  └── get_stats() -> ExtensionStats                   // Performance telemetry
```

All physics kernels are pure functions with no global state beyond lazy-initialized data
tables. This makes them trivially testable — call with known inputs, assert known outputs.

### SQF framework (modules)

The SQF layer is per-frame orchestration, event handling, and data configuration. Each
module is a separate PBO under `addons/`:

```
@abe/
  addons/
    abo_core/          — Extension loader, version check, logging, common macros
    abo_interior/      — Barrel length→MV, chamber pressure, propellant model
    abo_external/      — Custom CDMs, wind gradient, BC-vs-Mach scaling
    abo_terminal/      — Fragmentation, yaw/tumbling, temporary cavity
    abo_penetration/   — De Marre/L-O penetration, angle curves, overmatch
    abo_ricochet/      — Energy-retaining ricochets, skip/bounce/tumble
    abo_armor/         — Material arrays, ERA/composite/spaced, spall liners
    abo_bad/           — Behind-armor debris, spalling fragment generation
    abo_damage/        — Yaw-dependant wounding, fragmentation damage channels
    abo_environment/   — ISA lapse rates, dynamic weather, density gradients
    abo_fcs/           — Ballistic reticles, cant error, altitude zero correction
    abo_degradation/   — Barrel heat, fouling, erosion, round-count tracking
    abo_ace3/          — ACE3 hook layer (dispatch to ACE3 or standalone)
    compat_rhs/        — RHS weapon/vehicle data tables
    compat_cup/        — CUP weapon/vehicle data tables
    compat_niarms/     — NIArms weapon data tables
    compat_ww2/        — WWII weapon data tables (IFA/FOW)
```

---

## 3. Module Dependency Graph

```
abo_core (no deps)
  |
  ├── abo_environment (no deps)       — ISA atmosphere, wind gradient
  ├── abo_interior (core)              — Barrel length, chamber pressure
  |     └── abo_external (interior + environment)
  |           └── abo_fcs (external + environment)
  |
  ├── abo_penetration (core)           — Penetration math
  |     ├── abo_ricochet (penetration) — Ricochet physics
  |     └── abo_armor (penetration)    — Armor array modeling
  |           └── abo_bad (armor)      — Behind-armor debris
  |
  ├── abo_terminal (external)          — Fragmentation, yaw in tissue
  |     └── abo_damage (terminal + bad) — Damage application
  |
  ├── abo_degradation (core)           — Barrel heat, fouling
  |
  └── abo_ace3 (core)                  — ACE3 integration layer
        └── combat_* PBOs              — Per-mod weapon/vehicle data

Independent: abo_environment, abo_degradation (can ship standalone)
```

### Build order (5 phases)

| Phase | Modules | Deps | Testable headless? |
|---|---|---|---|
| **1 — Core + Interior** | abo_core, abo_interior | None | ✅ Full cargo test |
| **2 — External + Env** | abo_environment, abo_external, abo_fcs | Interior | ✅ Full cargo test |
| **3 — Terminal + Pen** | abo_penetration, abo_ricochet, abo_armor, abo_bad, abo_terminal, abo_damage | Core | ✅ Full cargo test |
| **4 — Degradation** | abo_degradation | Core | ✅ Full cargo test |
| **5 — ACE3 + Compat** | abo_ace3, compat_* | All | ✅ Integration via ACT |

---

## 4. Headless Testing Strategy

### 4.1 Rust extension tests (`cargo test`)

Every physics kernel ships with property-based and regression tests. This covers ~95% of
the mod's computational surface without any ARMA 3 involvement.

```rust
// Example: interior ballistics test
#[test]
fn barrel_length_increases_muzzle_velocity() {
    // Longer barrel → higher MV (within same chamber pressure)
    let short = interior::calc_muzzle_velocity(barrel: 10.1, chamber: 400.0, caliber: 5.56);
    let long  = interior::calc_muzzle_velocity(barrel: 20.0, chamber: 400.0, caliber: 5.56);
    assert!(long.mv > short.mv);
    assert!((long.mv - short.mv) - expected_delta < 1.0); // ~60 m/s per 10"
}

#[test]
fn drag_diverges_at_transonic() {
    // G1 drag coefficient changes measurably around Mach 0.8-1.2
    let subsonic = drag::cdm(Mach: 0.6, projectile: "M80");
    let transonic = drag::cdm(Mach: 0.95, projectile: "M80");
    assert!((transonic - subsonic).abs() > 0.05); // Significant drag rise
}

#[test]
fn penetration_drops_with_angle() {
    // 60° impact angle → ~2x effective thickness (cosine rule + material factor)
    let pen_0  = penetration::calc(velocity: 850, mass: 9.5, caliber: 7.62, angle_deg: 0);
    let pen_60 = penetration::calc(velocity: 850, mass: 9.5, caliber: 7.62, angle_deg: 60);
    assert!(pen_60.penetrated <= pen_0.penetrated);
    assert!(pen_60.effective_thickness_mm > pen_0.effective_thickness_mm);
}
```

Tests run on every `cargo test`, every `hemtt build`, and every CI push. No ARMA 3
required.

### 4.2 Data validation tests (Rust)

All JSON data tables (weapon configs, armor arrays, fragmentation profiles) are validated
at compile/test time against JSON schemas:

```rust
#[test]
fn all_weapon_configs_validate() {
    let dir = std::fs::read_dir("data/weapons/").unwrap();
    for entry in dir {
        let doc: serde_json::Value = serde_json::from_reader(entry).unwrap();
        let schema = load_schema("weapon.schema.json");
        assert!(jsonschema::validate(&schema, &doc).is_ok(),
                "{} failed validation", entry.path().display());
    }
}
```

Catches bad data before any ARMA 3 process starts.

### 4.3 SQF unit tests (HEMTT `hemtt test`)

SQF modules that orchestrate events but don't contain physics logic are tested with
HEMTT's built-in test runner:

```sqf
// test_core.sqf — Extension loader mock test
["ABE extension loads correctly", {
    private _result = "abe_ballistics_ext" callExtension ["init", [1, false]];
    assert_equal(_result select 0, "OK");
}] call test_framework;

// test_interior_mock.sqf — Verify fire event dispatches to extension
["Fire event calls extension with correct params", {
    private _weapon = "rhs_weap_m4a1";
    private _result = call_abe_interior(_weapon);
    assert_equal(typeName _result, "ARRAY");
    assert_equal(count _result, 4); // [muzzleVelocity, barrelLength, chamberPressure, propellant]
}] call test_framework;
```

HEMTT test runner starts a minimal ARMA 3 context (no renderer, no world) — much lighter
than a full client launch but still requires the engine binary. These are used for
orchestration-level tests only.

### 4.4 Integration / regression tests (ACT in CI)

For tests that need the full pipeline — fire a virtual bullet, simulate flight, verify
impact — we use **ACT (Arma Continuous Testing)** in GitHub Actions CI:

```yaml
# .github/workflows/test.yml
jobs:
  integration:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: grupp-p/act-setup@v1
        with:
          version: latest
      - run: hemtt build
      - run: act run --mission tests/regression --headless
```

Regression missions fire weapons with known parameters and verify:
- Muzzle velocity within 2% of reference table
- Impact point within 0.1 MIL of reference trajectory
- Penetration verdict matches lookup table for 10+ calibrated shots

These run on PR merge to main and before releases only (2-5 min per run).

### 4.5 Test coverage targets

| Layer | Tool | Coverage target | ARMA 3 needed? |
|---|---|---|---|
| Physics kernels | `cargo test` | 100% of branch coverage | ❌ No |
| Data validation | `cargo test` | 100% of config schemas | ❌ No |
| SQF orchestration | `hemtt test` | 90%+ of module logic | ⚠️ Minimal engine context |
| Integration | ACT headless | Key regression scenarios | ✅ Yes (CI only) |
| Performance | Criterion (Rust) | All hot paths benchmarked | ❌ No |

---

## 5. Build System

### Project structure

```
AceBallisticsExtention/
  ├── .cargo/
  │   └── config.toml
  ├── .github/
  │   └── workflows/
  │       ├── test.yml         — cargo test + hemtt test + ACT integration
  │       ├── build.yml        — HEMTT build + Cargo cross-compile (win/linux)
  │       └── release.yml      — Tagged release with auto-changelog
  ├── ext/
  │   ├── Cargo.toml
  │   └── src/
  │       ├── lib.rs           — C ABI dispatcher
  │       ├── interior.rs      — Interior ballistics
  │       ├── exterior.rs      — External ballistics
  │       ├── terminal.rs      — Terminal ballistics
  │       ├── penetration.rs   — Penetration models
  │       ├── ricochet.rs      — Ricochet physics
  │       ├── atmosphere.rs    — Atmosphere model
  │       ├── drag.rs          — Drag functions / CDM
  │       ├── armor.rs         — Armor array model
  │       ├── damage.rs        — Fragmentation / BAD / yaw
  │       ├── environment.rs   — Weather / lapse rates
  │       ├── zeroing.rs       — Ballistic reticle / zero
  │       ├── degradation.rs   — Barrel heat / fouling
  │       ├── config.rs        — Data table loader
  │       └── data/            — Built-in weapon/ammo data
  │           ├── weapons/     — Weapon configs (JSON)
  │           ├── ammo/        — Ammo configs (JSON)
  │           ├── armor/       — Vehicle armor arrays (JSON)
  │           ├── schemas/     — JSON validation schemas
  │           └── reference/   — Reference trajectory tables (CSV)
  ├── addons/
  │   ├── abo_core/
  │   │   ├── config.cpp
  │   │   ├── fnc_init.sqf
  │   │   ├── fnc_fire.sqf
  │   │   └── fnc_step.sqf
  │   ├── abo_interior/
  │   ├── abo_external/
  │   └── ...
  ├── tests/
  │   ├── rust/                — Rust integration/regression tests
  │   ├── sqf/                 — SQF unit test missions
  │   └── regression/          — ACT regression missions
  ├── docs/
  │   ├── architecture.md
  │   ├── data-format.md       — JSON schema docs for data contributors
  │   ├── module-guide.md      — Per-module feature breakdown
  │   └── testing.md           — Full testing guide
  ├── data/                    — Community-contributed weapon data (pre-merge staging)
  │   ├── weapons/
  │   ├── ammo/
  │   └── armor/
  ├── PLANNING.md              — This file
  ├── .hemtt.json              — HEMTT config
  └── Cargo.toml               — Workspace root
```

### Build commands

```bash
# Development
cargo test                    # Run all Rust tests (< 2s)
cargo clippy                  # Lint Rust code
hemtt build                   # Build mod + copy extension binary
hemtt test                    # Run SQF unit tests

# CI
cargo test --release          # Release-mode tests
hemtt build --release         # Final build
act run --headless            # Integration regression tests

# Cross-compile extension for Windows from Linux
./scripts/build-cross.sh      # Targets x86_64-pc-windows-gnu + linux
```

---

## 6. Data Format (JSON Schemas)

The data contract makes ABE extensible by anyone. Example schemas:

### Weapon config
```json
{
  "class": "rhs_weap_m4a1",
  "caliber_mm": 5.56,
  "barrel_length_mm": 368,
  "rifling_twist_mm": 178,
  "chamber_pressure_mpa": 380,
  "cdm_id": "m855",
  "projectile_mass_g": 4.0,
  "muzzle_velocity_ms": 948,
  "zero_range_m": 100
}
```

### Ammo config
```json
{
  "class": "rhs_mag_30Rnd_556x45_M855_Stanag",
  "projectile": {
    "model": "m855",
    "mass_g": 4.0,
    "caliber_mm": 5.56,
    "bc_g7": 0.157,
    "cdm_id": "m855",
    "fragmentation": {
      "threshold_vel_ms": 762,
      "avg_fragments": 12,
      "mass_distribution": "log_normal",
      "params": {"mean": 0.08, "std": 0.04}
    }
  }
}
```

### Armor config
```json
{
  "vehicle": "rhs_btr80a_msv",
  "plates": [
    {
      "name": "hull_front",
      "material": "steel_rha",
      "thickness_mm": 10,
      "angle_deg": 45,
      "backing": null
    },
    {
      "name": "hull_side",
      "material": "steel_rha",
      "thickness_mm": 7,
      "angle_deg": 0,
      "backing": "spall_liner_kevlar"
    }
  ]
}
```

---

## 7. Implementation Roadmap

### Phase 1 — Foundation (Months 1-3)

- [ ] Rust extension skeleton: C ABI, init/fire/step/impact dispatch
- [ ] `abo_core` SQF: extension loader, init, logging, config parser
- [ ] Interior ballistics: barrel length→MV, chamber pressure model
- [ ] HEMTT project config, Cargo workspace
- [ ] CI/CD pipeline: build + test on push
- [ ] Data schemas: weapon, ammo, armor JSON schemas
- [ ] Headless testing framework: Rust unit tests for all interior functions
- [ ] First working test: `cargo test` + `hemtt test` pass

### Phase 2 — External Ballistics (Months 3-6)

- [ ] Custom CDM drag tables (mach-vs-drag for 10 projectile families)
- [ ] G1/G7/G8 standard drag curve implementation (reference)
- [ ] Wind gradient model
- [ ] ICAO/ISA standard atmosphere with altitude lapse rates
- [ ] BC-vs-Mach scaling (non-constant BC)
- [ ] Transonic drag divergence
- [ ] Coriolis + spin drift + Magnus (reference ACE3 impl)
- [ ] FCS: ballistic reticles, cant error, altitude zero correction
- [ ] Full external regression test suite (against reference trajectories)

### Phase 3 — Penetration & Armor (Months 6-9)

- [ ] De Marre penetration formula with material modifiers
- [ ] L-O (Lanz-Odermatt) formula for long-rod penetrators
- [ ] Angle-dependent penetration curves (non-linear)
- [ ] Caliber-to-thickness ratio effects
- [ ] Overmatch mechanics
- [ ] Ricochet physics with energy retention
- [ ] Material database: RHA, HHA, aluminum, ceramic, composite, ERA
- [ ] Layered armor arrays (ERA → steel → spall liner)
- [ ] Multi-hit armor degradation
- [ ] Armor data collection: 50+ vehicles

### Phase 4 — Terminal Effects (Months 9-12)

- [ ] Fragmentation model: mass distribution, spray pattern, velocity dependence
- [ ] Yaw/tumbling in tissue (yaw-growth differential equation)
- [ ] Behind-armor debris: fragment generation from penetrated armor
- [ ] Spalling: fragment generation from non-penetrated armor
- [ ] Temporary cavity modeling (simplified pressure field)
- [ ] Damage channels: fragment vs penetrator, yaw-dependent wounding
- [ ] Integration with ACE3 medical / standalone damage application

### Phase 5 — Environment & Degradation (Months 12-15)

- [ ] Dynamic weather → air density changes
- [ ] Barrel heat model: round-count + firing rate → temperature
- [ ] Barrel fouling: accuracy degradation over sustained fire
- [ ] Zeroing drift from thermal effects
- [ ] Erosion model: round-count → velocity loss
- [ ] Environmental effects on optics (lens fog, mirage)
- [ ] Light-level effects on zeroing (twilight refraction)

### Phase 6 — ACE3 Integration & Data (Months 15-21)

- [ ] ACE3 hook layer: intercept ACE3 bullet tracking, enrich with ABE data
- [ ] Standalone SQF fire control (no ACE3 dependency)
- [ ] Compatibility data: RHS weapons (100+), CUP (150+), NIArms (80+)
- [ ] Compatibility data: RHS vehicles (50+), CUP (80+)
- [ ] Performance optimization: data table caching, LUT vs computation balancing
- [ ] Public data contribution guide + CI validation for data PRs
- [ ] Community beta release
- [ ] Documentation site

---

## 8. Communication Protocol (Extension ↔ SQF)

### C ABI

```rust
#[no_mangle]
pub extern "C" fn abe_init(api_version: u32, ace_present: u32) -> u32 { ... }

#[no_mangle]
pub extern "C" fn abe_fire(
    barrel_length_mm: f64,
    chamber_pressure_mpa: f64,
    caliber_mm: f64,
    projectile_mass_g: f64,
    cdm_id: *const c_char,
    result: *mut FireResult,
) -> u32 { ... }

#[no_mangle]
pub extern "C" fn abe_step(
    pos_x: f64, pos_y: f64, pos_z: f64,
    vel_x: f64, vel_y: f64, vel_z: f64,
    dt_s: f64,
    wind_x: f64, wind_y: f64,
    density_kgm3: f64,
    temp_c: f64,
    result: *mut BulletState,
) -> u32 { ... }

#[no_mangle]
pub extern "C" fn abe_impact(
    vel_x: f64, vel_y: f64, vel_z: f64,
    mass_g: f64,
    caliber_mm: f64,
    armor_thickness_mm: f64,
    armor_material: *const c_char,
    impact_angle_deg: f64,
    result: *mut ImpactResult,
) -> u32 { ... }
```

### SQF invocation

```sqf
// Interior — calculate muzzle velocity
private _result = "abe_ballistics_ext" callExtension [
    "fire", [
        getNumber(configFile >> "CfgWeapons" >> _weapon >> "ABO_barrelLength"),
        getNumber(configFile >> "CfgWeapons" >> _weapon >> "ABO_chamberPressure"),
        getNumber(configFile >> "CfgAmmo" >> _ammo >> "caliber"),
        getNumber(configFile >> "CfgAmmo" >> _ammo >> "ABO_projectileMass"),
        getText(configFile >> "CfgAmmo" >> _ammo >> "ABO_cdmId")
    ]
];

// External — step bullet forward
private _stepResult = "abe_ballistics_ext" callExtension [
    "step", [
        _pos select 0, _pos select 1, _pos select 2,
        _vel select 0, _vel select 1, _vel select 2,
        _deltaTime,
        _wind select 0, _wind select 1,
        _airDensity,
        _temperature
    ]
];
```

---

## 9. Performance Budget

| Operation | Budget | Extension cost | Headroom |
|---|---|---|---|
| Init | < 50ms | < 1ms (load data tables) | 98% |
| Fire (single) | < 0.5ms | < 0.01ms (lookup + calc) | 98% |
| Step (per bullet) | < 0.1ms | < 0.005ms (few flops) | 95% |
| Impact (per hit) | < 0.5ms | < 0.02ms (pen model) | 96% |
| 100 bullets/frame | < 3.4ms | < 0.5ms (100 steps) | 85% |

ACE3's existing `advanced_ballistics` tracks ~20 bullets/frame with a ~0.1ms extension
cost. ABE's modules are additive — each adds 0.01-0.05ms per bullet per active module.
Even with all 14 modules enabled, 50 bullets should stay under 1ms in the extension
(the SQF iteration overhead is the actual bottleneck).

---

## 10. Key Risks

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Extension API changes (Arma patch) | Low | High | Contract versioning, graceful fallback |
| ACE3 internal API changes | Medium | Medium | Hook at stable points (handleFire, per-frame PFH) |
| Data quality for 500+ weapons | Medium | High | Community data contributions, CI validation, phased rollout |
| Multiplayer sync desync | Low | High | Deterministic extension (same inputs → same outputs), seed-based RNG |
| HEMTT/ACT compatibility | Low | Medium | Track upstream changes, pin versions |
| Performance on low-end servers | Low | Medium | Toggleable modules, dynamic bullet cap |

---

## 11. Contribution Model

```
1. Contributor forks, adds weapon JSON to data/weapons/
2. CI validates JSON against schema, runs reference trajectory check
3. If validation passes, PR is merged
4. Data is bundled into next release's compat PBO
5. Weapon appears in-game automatically (no code change)
```

Data contributions are the primary community engagement vector. The barrier to entry is
JSON literacy, not C++ or SQF experience.

---

*This document is a living plan. Update as scope, constraints, and understanding evolve.*
