# Architecture

ABE is split into three layers: the Rust extension (physics computation), the SQF
framework (in-game orchestration), and the JSON data tables (configuration). This page
describes each layer and how they connect.

## Table of Contents

- [Extension Layer (Rust)](#extension-layer-rust)
- [SQF Framework](#sqf-framework)
- [Data Tables](#data-tables)
- [Module Dependency Graph](#module-dependency-graph)
- [Build Order](#build-order)
- [Communication Protocol](#communication-protocol)
- [Performance Budget](#performance-budget)

## Extension Layer (Rust)

The extension is a shared library (`abe_ballistics_ext.dll` or `abe_ballistics_ext.so`)
that ARMA 3 loads via `callExtension`. It exposes five C ABI entry points. All physics
kernels are pure functions with no global state beyond lazy-initialized data tables,
making them trivially testable.

```
abe_ballistics_ext.[dll/so]
  ├── init(api_version, ace_present) -> bool
  ├── fire(params: FireParams) -> FireResult        // Interior ballistics
  ├── step(state: BulletState, dt: f64, env: EnvState) -> BulletState  // External step
  ├── impact(state: BulletState, target: TargetData) -> ImpactResult    // Terminal/penetration
  └── get_stats() -> ExtensionStats                   // Performance telemetry
```

### Source Layout

```
ext/src/
  ├── lib.rs           — C ABI dispatcher, FFI boundary
  ├── interior.rs      — Barrel length to MV, chamber pressure, propellant model
  ├── exterior.rs      — Custom CDM, wind gradient, BC-vs-Mach scaling
  ├── terminal.rs      — Fragmentation, yaw/tumbling, temporary cavity
  ├── penetration.rs   — De Marre and L-O formulas, angle curves, overmatch
  ├── ricochet.rs      — Energy-retaining ricochets, skip/bounce/tumble
  ├── atmosphere.rs    — ICAO/ISA standard atmosphere with altitude lapse rates
  ├── drag.rs          — Drag functions, G1/G7/G8 standard curves, CDM lookup
  ├── armor.rs         — Material database, layered armor array evaluation
  ├── damage.rs        — Yaw-dependent wounding, fragmentation damage channels
  ├── environment.rs   — Dynamic weather, air density gradients
  ├── zeroing.rs       — Ballistic reticles, cant error, altitude zero correction
  ├── degradation.rs   — Barrel heat, fouling, erosion, round-count tracking
  ├── config.rs        — JSON data table loader with schema validation
  └── data/            — Built-in weapon/ammo/armor reference data
```

## SQF Framework

The SQF layer handles per-frame orchestration, event handling, and data configuration.
Each module is a separate PBO under `addons/`. Modules call into the extension for
computation and read JSON data tables for configuration values.

Modules are organized into a dependency tree rooted at `abo_core`. The SQF code is kept
thin: it dispatches parameters to the extension, receives results, and applies them to
the game state. No physics logic lives in SQF.

The framework supports two dispatch modes:

- **ACE3 mode.** When ACE3 is detected, ABE hooks into ACE3's bullet tracking and
  enriches it with ABE data. ACE3 manages the per-frame bullet iteration; ABE provides
  the physics results.
- **Standalone mode.** Without ACE3, ABE runs its own per-frame handler that iterates
  over tracked bullets, calls the extension for each step, and applies the results.

The same extension binary and the same physics run in both modes. Only the dispatch
layer changes.

## Data Tables

Configuration data lives in JSON files under `ext/src/data/` (built-in reference data)
and `data/` (community-contributed data). Each file is validated against a JSON schema
at compile and test time.

```
data/
  ├── weapons/      — Weapon configs (caliber, barrel length, chamber pressure, CDM)
  ├── ammo/         — Ammo configs (projectile mass, BC, fragmentation params)
  ├── armor/        — Vehicle armor arrays (plate materials, thickness, angle)
  ├── schemas/      — JSON validation schemas
  └── reference/    — Reference trajectory tables (CSV)
```

The data pipeline works as follows:

1. JSON file is added to the appropriate directory.
2. CI validates the file against the JSON schema.
3. At build time, data is bundled into the mod's compatibility PBO.
4. At runtime, the SQF config parser reads the data and passes it to the extension.
5. The extension caches data tables after lazy initialization.

## Module Dependency Graph

The following ASCII diagram shows the dependency graph for all ABE modules. Arrows point
from a module to its dependencies.

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
        └── compat_* PBOs              — Per-mod weapon/vehicle data
```

Independent modules: `abo_environment` and `abo_degradation` have no dependencies beyond
core and can ship standalone.

## Build Order

The modules are built in five phases, each producing a testable artifact. This allows
incremental delivery and validation.

| Phase | Modules | Deps | Testable Headless |
|---|---|---|---|
| 1 - Core + Interior | abo_core, abo_interior | None | Full cargo test |
| 2 - External + Env | abo_environment, abo_external, abo_fcs | Interior | Full cargo test |
| 3 - Terminal + Pen | abo_penetration, abo_ricochet, abo_armor, abo_bad, abo_terminal, abo_damage | Core | Full cargo test |
| 4 - Degradation | abo_degradation | Core | Full cargo test |
| 5 - ACE3 + Compat | abo_ace3, compat_* | All | Integration via ACT |

## Communication Protocol

### C ABI

The extension exposes a stable C ABI. The `api_version` parameter enables graceful
handling of future ABI changes. Each function returns a status code (0 for success) and
writes results into a caller-allocated struct.

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

### SQF Invocation

From SQF, the extension is called via `callExtension` with command and parameter array.

```sqf
// Interior - calculate muzzle velocity
private _result = "abe_ballistics_ext" callExtension [
    "fire", [
        getNumber(configFile >> "CfgWeapons" >> _weapon >> "ABO_barrelLength"),
        getNumber(configFile >> "CfgWeapons" >> _weapon >> "ABO_chamberPressure"),
        getNumber(configFile >> "CfgAmmo" >> _ammo >> "caliber"),
        getNumber(configFile >> "CfgAmmo" >> _ammo >> "ABO_projectileMass"),
        getText(configFile >> "CfgAmmo" >> _ammo >> "ABO_cdmId")
    ]
];

// External - step bullet forward
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

## Performance Budget

| Operation | Budget | Extension Cost | Headroom |
|---|---|---|---|
| Init | < 50ms | < 1ms (load data tables) | 98% |
| Fire (single) | < 0.5ms | < 0.01ms (lookup + calc) | 98% |
| Step (per bullet) | < 0.1ms | < 0.005ms (few flops) | 95% |
| Impact (per hit) | < 0.5ms | < 0.02ms (pen model) | 96% |
| 100 bullets/frame | < 3.4ms | < 0.5ms (100 steps) | 85% |

ACE3's existing `advanced_ballistics` tracks roughly 20 bullets per frame with a 0.1ms
extension cost. ABE modules are additive, each adding 0.01-0.05ms per bullet per active
module. Even with all 14 modules enabled, 50 bullets should stay under 1ms in the
extension. The SQF iteration overhead is the actual bottleneck.
