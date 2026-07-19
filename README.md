<p align="center">
  <h1 align="center">Advanced Ballistics Extension (ABE)</h1>
  <p align="center">
    Realistic interior, exterior, terminal, and penetration ballistics for ARMA 3
    <br />
    Native Rust extension · ACE3 enhancer or standalone
  </p>
  <p align="center">
    <a href="https://github.com/lErrorl404l/AceBallisticsExtended/actions/workflows/build.yml"><img src="https://img.shields.io/github/actions/workflow/status/lErrorl404l/AceBallisticsExtended/build.yml?logo=github&label=Build" alt="Build"></a>
    <a href="https://github.com/lErrorl404l/AceBallisticsExtended/actions/workflows/test.yml"><img src="https://img.shields.io/github/actions/workflow/status/lErrorl404l/AceBallisticsExtended/test.yml?logo=github&label=Tests" alt="Tests"></a>
    <img src="https://img.shields.io/badge/tests-1134%20passing-brightgreen?logo=rust" alt="Tests">
    <a href="LICENSE"><img src="https://img.shields.io/badge/license-GPL--3.0-blue" alt="License"></a>
    <img src="https://img.shields.io/badge/rust-2024%20edition-purple?logo=rust" alt="Rust">
    <img src="https://img.shields.io/badge/HEMTT-ready-orange" alt="HEMTT">
    <img src="https://img.shields.io/badge/status-alpha-yellow" alt="Status">
  </p>
</p>

---

## Overview

ABE replaces ARMA 3's arcade ballistics with data-driven physics. All simulation
runs in a native Rust extension — SQF is thin orchestration, JSON is the
configuration language. Community contributions are **data PRs**, not code.

ABE runs in two modes:
- **ACE3 Enhanced** — hooks into ACE3's bullet tracking, replaces its physics
- **Standalone** — full self-contained ballistics when ACE3 is absent

## Table of Contents

- [Quick Start](#quick-start)
- [Physics Models](#physics-models)
- [Architecture](#architecture)
- [Configuration Data](#configuration-data)
- [ACE3 Integration](#ace3-integration)
- [Public API](#public-api)
- [Testing](#testing)
- [Benchmarks](#benchmarks)
- [IRL Source Data](#irl-source-data)
- [Project Status](#project-status)
- [Contributing](#contributing)
- [License](#license)

---

## Quick Start

### Requirements

- **Rust** 1.85+ (edition 2024) — MSRV policy: latest stable
- **HEMTT** ([install](https://hemtt.dev/)) — Arma 3 mod toolchain
- **MinGW-w64** — Windows cross-compilation

### Build

```bash
git clone https://github.com/lErrorl404l/AceBallisticsExtended
cd AceBallisticsExtended

# Build the Rust extension (Linux .so)
cd ext && cargo build --release && cd ..

# Cross-compile Windows DLL
cd ext && cargo build --release --target x86_64-pc-windows-gnu && cd ..

# Package the mod
hemtt build
```

Drop `@AceBallisticsExtended` into your ARMA 3 installation and launch with
`-mod=@AceBallisticsExtended`.

### Quick Iteration

```bash
./build.sh                          # Full build: Rust → copy binary → HEMTT check
cd ext && cargo test                # Run all 1134+ physics tests
cargo doc --no-deps --open          # Build and open API docs
```

---

## Physics Models

All models are implemented as pure functions in `ext/src/` — no global state,
no I/O, trivially testable.

| Module | File | Model |
|--------|------|-------|
| Interior | `interior.rs` | Pressure-integral exponential decay (Heiney, UK DefStan 13-100). `AVG_PRESSURE_FACTOR = 0.58`, efficiency `0.87 × e^(-0.30 × L)`, burn fraction, barrel time. |
| Exterior | `exterior.rs` | Semi-implicit Euler integration. Mach, wind drift, spin drift, Coriolis (McCoy, NATO STANAG 4355). |
| Drag | `drag.rs` | G1/G7/G8 lookup tables (JBM/ABRA) with linear interpolation. |
| Atmosphere | `atmosphere.rs` | ICAO/ISA standard atmosphere (ICAO Doc 7488, ISO 2533). Tropospheric lapse −6.5 K/km, barometric formula, log-wind-profile shear. |
| Penetration | `penetration.rs` | Three-stage: ricochet → effective thickness → De Marre threshold velocity. Material factors (RHA=1.0, HHA=1.25, ceramic=2.5–3.5, kevlar=0.6). Lanz-Odermatt for APDS/APFSDS. |
| Fragmentation | `fragmentation.rs` | Velocity-threshold gated (762 m/s M855). Log-normal mass distribution. Cone angles: FMJ=15°, AP=8°, HP=25°. Golden-angle azimuth (Nennstiel, UK DefStan 13-100). |
| HEAT | `heat_penetration.rs` | Shaped-charge jet penetration (Birkhoff, Eichelberger). Standoff efficiency, jet stretch. |
| Behind-armour debris | `behind_armor_debris.rs` | Spall generation, temporary cavity, secondary fragment spray. |

Full technical descriptions: [`PLANNING.md`](PLANNING.md), or the [online docs](https://lerrorl404l.github.io/AceBallisticsExtended).

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    Rust Extension (ext/)                         │
│                                                                  │
│  C ABI (extern "C")                                             │
│  ┌──────────────────────────────────────────────────────────┐    │
│  │ abe_init  abe_fire  abe_step  abe_impact                │    │
│  │ abe_health  abe_version  abe_free                       │    │
│  └──────────────────────────────────────────────────────────┘    │
│                                                                  │
│  ┌──────────┬───────────┬───────────┬───────────┬──────────┐    │
│  │ interior │ exterior  │ drag      │ atmosphere│ config   │    │
│  ├──────────┼───────────┼───────────┼───────────┼──────────┤    │
│  │ penetration │ frag   │ heat_pen  │ behind_armor_debris  │    │
│  └──────────┴───────────┴───────────┴───────────┴──────────┘    │
│                                                                  │
│  Targets: cdylib (.so/.dll) + rlib                         │
└─────────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│                    SQF Layer (addons/)                            │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐    │
│  │ abe_core:  Init, ACE3 detection, mode switching          │    │
│  │ abe_tracking:  Per-frame bullet iteration                │    │
│  │ abe_events:  Fired + HitPart event handlers              │    │
│  └──────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│                    JSON Data (data/)                              │
│                                                                  │
│  weapons/  ammo/  calibers/  armor/  vehicles/  schemas/         │
│  materials/  calibration/  scripts/  sources/                    │
│                                                                  │
│  Community-contributable — no compiler required                  │
└─────────────────────────────────────────────────────────────────┘
```

Two calling conventions:
- **String ABI** — `callExtension` from SQF, ~5 M calls/s
- **Struct C ABI** — `#[repr(C)]` parameter structs, ~6× faster (~30–50 M calls/s)

---

## Configuration Data

All data lives in `data/` as JSON files. Adding a new weapon, round, or vehicle
is a **data PR** — no Rust recompilation needed.

| Directory | Contents |
|-----------|----------|
| `data/weapons/` | 89+ weapon configs (barrel length, chamber pressure, rifling twist) |
| `data/ammo/` | 477+ ammunition configs (mass, BC, drag model, fragmentation params) |
| `data/calibers/` | 19 caliber definitions (case capacity, max pressure) |
| `data/armor/` | Material properties (density, hardness, RHA equivalence, spall coeff) |
| `data/armor/plates/` | Vehicle armour arrays (M1A2, T-72, T-80, T-90, BMP-2, Bradley) |
| `data/vehicles/` | Full vehicle armour layouts |
| `data/materials/` | 70+ ballistic material entries |
| `data/schemas/` | JSON Schema validation files |
| `data/sources/` | Manufacturer and defence-source reference documents |

### IRL Source Data

ABE is data-driven, not speculative. `data/sources/` contains curated source
documents from:

- **Hornady** — ballistic coefficient tables and reloading data
- **Lapua** — cartridge specification PDFs and velocity tables
- **SAAMI / CIP / NATO** — chamber pressure standards (SAAMI Z299.3/4, CIP TDCC, EPVAT)
- **ARL / BRL** — US Army ballistic research penetration test reports (DTIC.mil)
- **SSAB** — ARMOX armor plate datasheets and MIL-DTL specifications

Every config value should trace to one of these sources.

---

## ACE3 Integration

ABE detects ACE3 at init and switches behaviour:

- **No ACE3** → hooks `CBA_fired`, `HitPart` for standalone ballistics
- **ACE3 loaded** → three-layer ACE3 override:
  1. Disables ACE3's advanced ballistics setting
  2. Replaces its bullet tracking hashmap
  3. Per-frame purge for edge cases

The override is fully reversible — ACE3's setting is restored on mission end.

---

## Public API

All functions `extern "C"`, thread-safe via `OnceLock`.

| String Call | Signature | Description |
|-------------|-----------|-------------|
| `"init"` | `[api_version, ace_present]` → `"0"` / `"-1"` | Initialise extension |
| `"version"` | — → `"MAJOR.MINOR.PATCH"` | Extension version |
| `"health"` | — → `"1"` / `"0"` | Health check |
| `"fire"` | `[barrelMm, pressureMpa, calMm, massG, cdmId]` → `[mv, pressure, burn, t_ms]` | Interior ballistics |
| `"step"` | `[posX/Y/Z, velX/Y/Z, dt, windX/Y/Z, density, tempC, alt, cdmId, bc, mass, cal]` → `[pos, vel, mach, dt]` | External step |
| `"impact"` | `[velX/Y/Z, mass, cal, armourThick, material, angle, projType]` → `[pen, resVel, energy, effThick, ric, ...]` | Terminal ballistics |

Struct equivalents (`abe_fire`, `abe_step`, `abe_impact`) accept `#[repr(C)]`
parameter structs — ~6× faster, recommended for per-frame use. See
[`ext/src/lib.rs`](ext/src/lib.rs) for struct layouts.

---

## Testing

```bash
cargo test                          # 1134+ unit + integration tests
cargo test --lib                    # Library unit tests
cargo test --test sqf_compat_test   # SQF compatibility
python tests/validate_data.py       # 230+ JSON validation checks
```

Physics invariants enforced across all modules:
- Energy non-increasing
- Monotonic position
- Free-fall convergence
- Transonic drag consistency (G7 < G1 at all Mach, peak near M=1)
- Mass conservation in fragmentation (±10%)

---

## Benchmarks

```bash
cargo bench
```

Tested on reference hardware (criterion, HTML reports):

| Benchmark | Throughput |
|-----------|-----------|
| `fire/struct_abi` | ~50 M calls/s |
| `step/struct_abi` | ~30 M calls/s |
| `impact/struct_abi` | ~40 M calls/s |
| `step/string_abi` | ~5 M calls/s |
| `pipeline/fire_500step_impact` | ~100k pipelines/s |

Struct ABI ≈ 6× faster than string dispatch. Use `abe_*` calls in per-frame loops.

---

## Project Status

**Alpha** — physics kernels are feature-complete and tested. SQF layer and
in-game integration are in development. Current focus:

- [x] Interior ballistics (pressure-integral model)
- [x] Exterior ballistics (drag, Coriolis, wind, spin drift)
- [x] Penetration (De Marre, Lanz-Odermatt, ricochet)
- [x] Fragmentation and behind-armour debris
- [x] HEAT shaped-charge jet penetration
- [x] Atmosphere model (ICAO/ISA)
- [x] 250+ data files across weapons, ammo, armour
- [x] IRL source data from manufacturers and defence labs
- [ ] SQF event system and ACE3 integration complete
- [ ] In-game validation and tuning
- [ ] Reforger compatibility evaluation

---

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for:

- Adding weapons, ammo, or armour (JSON data PRs)
- Ballistic coefficient sourcing guidelines
- Physics model improvements
- PR workflow and commit conventions
- Development environment setup

All contributions are welcome — data additions, physics refinements,
documentation, and testing. This project uses **conventional commits** and
requires all 1134+ tests to pass before merging.

---

## License

**GNU General Public License v3.0** — see [`LICENSE`](LICENSE).

ABE is free software: you can redistribute and modify it under the terms of
the GPL-3.0. This ensures the mod remains open for the ARMA community.
