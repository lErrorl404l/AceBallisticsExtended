# Ace Ballistics Extension (ABE)

Realistic interior/exterior/terminal ballistics for ARMA 3 as an ACE3 enhancer or standalone mod. Uses a native Rust extension for all physics kernels with SQF glue and JSON configuration.

## Build Status

HEMTT ✅ • cargo test 121/121 ✅ • Python validation 230/230 ✅ • SQF 20/20 ✅ • clippy clean ✅ • cargo doc clean ✅

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

Two-zone gas-expansion pressure curve model (Heiney, UK DefStan 13-100, Nennstiel). Pressure peaks at ~12 % of projectile travel, then decays exponentially. Work integral along the bore gives kinetic energy, reduced by friction, heat transfer, and rifling losses. Burn fraction exponential with barrel length; barrel time from `t = 2L / MV`.

### Exterior Ballistics (`exterior.rs`, `drag.rs`)

Semi-implicit Euler integration. Drag: `0.5 * ρ * v² * Cd / (BC * K)` where `K ≈ 895.3` (lb/in²→kg/m² conversion). G1/G7/G8 drag tables from JBM/ABRA with linear interpolation. Coriolis and spin drift per McCoy and NATO STANAG 4355.

### Atmosphere Model (`atmosphere.rs`)

ICAO/ISA standard atmosphere (ICAO Doc 7488, ISO 2533, MIL-STD-210C). Tropospheric lapse -6.5 K/km, isothermal above 11 km. Barometric formula + ideal gas law. Log-wind-profile wind shear (von Kármán-Prandtl, surface layer 0–200 m).

### Penetration Model (`penetration.rs`)

Three-stage terminal model (De Marre, Lanz-Odermatt, NIJ 0108.01): ricochet check → effective thickness (angle + material scaling) → threshold velocity `V_req = k·D^0.75·T^0.7 / M^0.5`. Material factors: RHA=1.0, HHA=1.25, Al=0.35–0.45, ceramic=2.5–3.5, kevlar=0.6.

### Fragmentation (`fragmentation.rs`)

Velocity-threshold gated (762 m/s M855). Log-normal mass distribution; fragment count scales with velocity ratio. Velocity partition `V_frag = V_impact·(M_frag/M_total)^0.33`. Cone angles: FMJ=15°, AP=8°, HP=25°. Golden-angle azimuth (Nennstiel, UK DefStan 13-100, FBI HPR).

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

All functions are `extern "C"` and thread-safe via `OnceLock` global state.
Two calling conventions — **string ABI** (SQF `callExtension`) and **struct C ABI** (fast path for native code):

| Call | Signature |
|------|-----------|
| `"init"` | `[api_version, ace_present]` → `"0"` \| `"-1"` |
| `"version"` | (none) → `"MAJOR.MINOR.PATCH"` |
| `"health"` | (none) → `"1"` \| `"0"` |
| `"fire"` | `[barrelLengthMm, chamberPressureMpa, caliberMm, massG, cdmId]` → `[mv, pressure, burn, t_ms]` |
| `"step"` | `[posX/Y/Z, velX/Y/Z, dt, windX/Y/Z, density, tempC, alt, cdmId, bc, mass, cal]` → `[posX/Y/Z, velX/Y/Z, mach, dt]` |
| `"impact"` | `[velX/Y/Z, mass, cal, armorThick, material, angle, projType]` → `[pen, resVel, energy, effThick, ric, ricAngle, ricEner, frags, spall]` |

Struct equivalents (`abe_init`, `abe_version`, `abe_health`, `abe_fire`, `abe_step`, `abe_impact`) accept `&FireParams` / `&StepParams` / `&ImpactParams` and are ~6× faster. All struct types are `#[repr(C)]` with `[u8; 32]` arrays for string fields. See [`ext/src/lib.rs`](ext/src/lib.rs).

## Testing

```bash
cargo test                  # 121 unit + integration tests
cargo test --lib            # 95 unit tests
cargo test --test sqf_compat_test  # 26 integration tests
python tests/validate_data.py  # 230 data validation checks
hemtt test                     # SQF headless runtime (20/20)
```

Energy non-increasing, monotonic position, free-fall, and transonic
consistency are enforced across all physics modules.

## Benchmarks

`cargo bench` (criterion):

| Benchmark | Throughput |
|-----------|-----------|
| `fire/struct_abi` | ~50 M calls/s |
| `step/struct_abi` | ~30 M calls/s |
| `impact/struct_abi` | ~40 M calls/s |
| `step/string_abi` | ~5 M calls/s |
| `pipeline/fire_500step_impact` | ~100k pipelines/s |

Struct ABI ≈ 6× faster than string dispatch. Prefer `abe_*` for per-frame loops.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) — adding data, inferring BCs, physics
improvements, PR workflow, and commit conventions.

## License

MIT. See LICENSE file.
