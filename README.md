# Ace Ballistics Extension (ABE)

Realistic interior/exterior/terminal ballistics for ARMA 3 as an ACE3 enhancer or standalone mod. Uses a native Rust extension for all physics kernels with SQF glue and JSON configuration.

## Quick Start

### Build Requirements
- **Rust** 1.85+ (edition 2024) with `x86_64-pc-windows-gnu` target for cross-compilation
- **HEMTT** (Arma 3 build toolchain)
- **MinGW-w64** (Windows cross-compilation)

### Build

```bash
git clone https://github.com/lErrorl404l/AceBallisticsExtended
cd AceBallisticsExtended

# Build the Rust physics extension (Linux .so)
cd ext && cargo build --release && cd ..

# Cross-compile Windows DLL
cd ext && cargo build --release --target x86_64-pc-windows-gnu && cd ..

# Build PBO mod package
hemtt build
```

Drop the resulting `@AceBallisticsExtended` directory into your ARMA 3 installation and launch with:

```
-mod=@AceBallisticsExtended
```

## Architecture

ABE has three layers:

```
Rust Extension (ext/)          — All physics kernels
  ├── lib.rs                   — C ABI dispatcher (abe_init, abe_fire, abe_step, abe_impact, abe_health, abe_version, abe_free)
  ├── interior.rs              — Two-zone gas-expansion interior ballistics model
  ├── exterior.rs              — Speed of sound, Mach, wind drift, spin drift, Coriolis
  ├── drag.rs                  — G1/G2/G5/G6/G7/G8/GL drag coefficient lookup tables with linear interpolation
  ├── atmosphere.rs            — ICAO standard atmosphere: temperature/pressure/density vs altitude, wind shear
  ├── penetration.rs           — De Marre + Lanz-Odermatt penetration, ricochet, spall
  ├── fragmentation.rs         — Projectile fragmentation: log-normal mass distribution, spray cone
  └── config.rs                — JSON data loading and deserialization

SQF Glue (addons/abo_core/)   — Thin event dispatch
  ├── fnc_init.sqf             — Extension init, CBA/ACE3 event hook registration
  ├── fnc_fire.sqf             — Fire event handler, bullet state initialization
  ├── fnc_step.sqf             — Per-frame trajectory update, ACE3 hashmap purge
  ├── fnc_impact.sqf           — HitPart handler, armor lookup, damage application
  ├── fnc_health.sqf           — Health check diagnostic
  └── fnc_ace3_compat.sqf      — ACE3 advanced ballistics override (layers A–C)

JSON Config (data/)            — Community-contributable without a compiler
  ├── weapons/                 — 25+ weapon configurations (barrel length, chamber pressure, caliber)
  ├── ammo/                    — 12+ ammunition configurations (mass, BC, drag model, fragmentation params)
  ├── armor/                   — 3 armor material definitions (RHA, aluminum, ceramic)
  └── schemas/                 — JSON Schema validation files
```

## Physics Models

### Interior Ballistics (`interior.rs`)

Two-zone gas-expansion pressure curve model. Pressure rises as propellant ignites (peak at approximately 12% of projectile travel), then decays exponentially as the projectile moves down the bore. The work integral `integral P(x) dx` along the barrel length is reduced by friction, heat transfer, and rifling engraving losses. Burn fraction follows an exponential approach to completion with barrel length. Barrel time derived from the average-velocity approximation `t = 2L / MV`.

References: Heiney (Internal Ballistics), UK Defence Standard 13-100, Nennstiel's Interior Ballistics Model.

### Exterior Ballistics (`exterior.rs`, `drag.rs`)

Semi-implicit Euler integration of projectile trajectory. Drag deceleration computed as `0.5 * rho * v^2 * Cd / (BC * K)` where `K ≈ 895.3` converts ballistic coefficient from imperial (lb/in^2) to SI (kg/m^2) including cross-sectional area factor.

Drag coefficients drawn from JBM Ballistics / ABRA lookup tables with linear interpolation between control points. Supported standard drag models: G1 (flat-base tangent ogive), G7 (boat-tail secant ogive, preferred for modern rifle bullets), G8 (flat-base secant ogive). The bullet's ballistic coefficient (BC) scales the drag curve.

Coriolis and spin drift (Magnus effect) approximations are available in `exterior.rs` for azimuth computations, following McCoy's Modern Exterior Ballistics and NATO STANAG 4355 (AOP-55).

### Atmosphere Model (`atmosphere.rs`)

ICAO/ISA standard atmosphere. Temperature follows the tropospheric lapse rate of -6.5 K/km to the tropopause at 11 km, then isothermal to 20 km. Pressure computed via the barometric formula `P = P0 * (T/T0)^(-g/(R*L))` in the troposphere and isothermal exponential decay above. Density from the ideal gas law `rho = P/(R*T)`. Wind shear follows the log-wind-profile (von Karman-Prandtl) in the surface layer (0–200 m).

References: ICAO Doc 7488, ISO 2533:1975, MIL-STD-210C.

### Penetration Model (`penetration.rs`)

Three-stage terminal ballistics model:
1. **Ricochet check** — if impact angle exceeds the velocity- and caliber-dependent threshold, the projectile ricochets with energy retention scaled by angle
2. **Effective thickness** — plate thickness divided by cosine of impact angle, scaled by material factor and caliber-to-thickness ratio
3. **De Marre penetration** — threshold velocity `V_req = k * D^0.75 * T^0.7 / M^0.5`, with residual velocity `V_res = sqrt(V^2 - V_req^2)` on penetration

Material factors: RHA = 1.0, HHA = 1.25, aluminum ≈ 0.35–0.45, ceramic ≈ 2.5–3.5, kevlar = 0.6. Projectile type modifiers: ball/FMJ = 1.0, AP = 1.3, APDS/APFSDS = 1.8.

References: De Marre ballistics formula, Lanz-Odermatt (long rod penetrators), NIJ 0108.01.

### Fragmentation (`fragmentation.rs`)

Velocity-threshold gated fragmentation with log-normal mass distribution. Fragment count scales with velocity ratio above threshold (762 m/s for M855-style rounds). Small fragments retain less velocity via power-law partitioning `V_frag = V_impact * (M_frag / M_total)^0.33`. Spray pattern uses base cone angle (15 deg FMJ, 8 deg AP, 25 deg soft/hollow point) with golden-angle azimuth for deterministic distribution.

References: Nennstiel (1986), UK Defence Standard 13-100, FBI HPR fragmentation data.

## Configuration Data

### Weapon Schema (`data/weapons/`)

| Field                | Type     | Description                                        |
|----------------------|----------|----------------------------------------------------|
| `weaponClass`        | string   | CfgWeapons class name                              |
| `barrelLengthMm`     | number   | Barrel length in millimeters                       |
| `caliberMm`          | number   | Bullet caliber in millimeters                      |
| `chamberPressureMpa` | number   | SAAMI/CIP peak chamber pressure in MPa             |
| `riflingTwistMm`     | number   | Rifling twist rate in mm per revolution (optional) |
| `projectileMassG`    | number   | Projectile mass in grams (overrides ammo)          |
| `cdmId`              | string   | Drag model curve ID (default: g7)                  |
| `zeroRangeM`         | number   | Zero range in meters (default: 100)                |
| `effectiveRangeM`    | number   | Maximum effective range (optional)                 |
| `notes`              | string   | Data source or assumptions (optional)              |

### Ammo Schema (`data/ammo/`)

| Field                   | Type    | Description                                      |
|-------------------------|---------|--------------------------------------------------|
| `ammoClass`             | string  | CfgAmmo class name                               |
| `projectile.mass_g`     | number  | Projectile mass in grams                         |
| `projectile.caliber_mm` | number  | Projectile caliber in millimeters                |
| `projectile.bc_g7`      | number  | Ballistic coefficient (G7 model)                 |
| `projectile.cdm_id`     | string  | Drag model curve ID                              |
| `projectile.fragmentation.*` | object | Fragmentation parameters (threshold, count, distribution) |

### Armor Schema (`data/armor/`)

| Field                | Type   | Description                                   |
|----------------------|--------|-----------------------------------------------|
| `materialId`         | string | Unique material identifier                    |
| `densityGcm3`        | number | Material density in g/cm^3                    |
| `hardnessBHN`        | number | Brinell hardness number                       |
| `tensileStrengthMpa` | number | Ultimate tensile strength in MPa (optional)   |
| `rhaEquivalent`      | number | RHA equivalency multiplier (default: 1.0)     |
| `ductility`          | number | Ductility factor 0–1 (default: 0.5)           |
| `spallCoeff`         | number | Spall generation coefficient 0–1 (default: 0.5) |

## ACE3 Integration

ABE detects whether ACE3 is loaded at init and switches between two modes:

### Standalone Mode (no ACE3)
Hooks into CBA_fired event for weapon firing, uses the HitPart event system for impact detection. ABE handles all ballistics independently.

### ACE3 Enhanced Mode
Overrides ACE3's advanced_ballistics module via a three-layer strategy:

- **Layer A** — Sets `ace_advanced_ballistics_enabled = false` in missionNamespace, preventing ACE3's setting handler from registering its ballistic event handlers
- **Layer B** — Replaces ACE3's `ace_advanced_ballistics_allBullets` hashmap with an empty map, making the per-frame handler a no-op
- **Layer C** — Per-frame purge removes any ACE3-tracked bullets that were added after Layers A and B were applied (edge case handling)

The override is reversible on mission end, restoring ACE3's setting for subsequent missions without ABE.

### Public API (C ABI)

All functions are `extern "C"` and thread-safe via `OnceLock` global state:

| Function       | Description                                            |
|----------------|--------------------------------------------------------|
| `abe_init()`   | Initialize extension with API version and ACE3 flag    |
| `abe_version()`| Return semver string                                   |
| `abe_health()` | Return 1 if initialized, 0 otherwise                   |
| `abe_fire()`   | Compute muzzle velocity from barrel/chamber/ammo params|
| `abe_step()`   | Integrate projectile state forward by delta time       |
| `abe_impact()` | Evaluate penetration, ricochet, spall, fragmentation   |
| `abe_free()`   | Release extension resources                            |

## Testing

```bash
# Rust unit tests (95 tests covering all physics modules)
cargo test

# Data validation (230 Python validation checks)
python tests/validate_data.py

# SQF integration tests (via HEMTT)
hemtt test
```

## License

MIT. See LICENSE file.
