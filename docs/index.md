# Advanced Ballistics Extension (ABE)

ABE is a data-driven ballistics framework for ARMA 3. The physics engine runs in a Rust
extension (C ABI), configuration lives in JSON data tables, and the SQF layer is thin
orchestration. ACE3 compatibility is a deployment mode, not an architectural dependency.

## Table of Contents

- [Overview](#overview)
- [Key Features](#key-features)
- [Quick Start](#quick-start)
- [Project Structure](#project-structure)
- [Contributing](#contributing)

## Overview

Traditional ARMA 3 ballistics mods embed physics in SQF or C++ configs. ABE takes a
different approach: the computational core is a Rust library compiled to a shared object
that ARMA 3 loads via `callExtension`. This gives native-code performance, a full test
suite that runs without the game engine, and a data-driven design where adding a new
weapon or round is a JSON file and a class config, never a recompile.

The mod targets ARMA 3 with a Reforger-ready design. When ACE3 is loaded, ABE hooks into
ACE3's bullet tracking and enriches it. Without ACE3, ABE runs its own SQF framework.
Same extension binary, same physics, different dispatch.

## Key Features

- **Data-driven ballistics.** Weapon, ammo, and armor configuration in JSON. Community
  contributions are data PRs, not code changes. CI validates every entry against JSON
  schemas before merge.
- **Rust extension speed.** Physics kernels are compiled native code with no garbage
  collection and no scripting overhead. A single bullet step completes in under 5
  microseconds. One hundred bullets per frame costs under 0.5ms in the extension.
- **ACE3 standalone operation.** With ACE3 loaded, ABE enriches ACE3's bullet tracking.
  Without ACE3, ABE runs its own full SQF fire control layer. No hard dependency.
- **Toggleable modules.** Each ballistics domain (interior, terminal, penetration,
  ricochet, environment, degradation, and others) is an independent PBO that server hosts
  can enable or disable. Choose depth versus performance.
- **Headless testable.** The Rust extension is pure library code with no ARMA 3 dependency.
  `cargo test` covers all physics kernels. SQF unit tests run under HEMTT with a minimal
  engine context. Integration regression tests use ACT in CI only.
- **Verifiable by replay.** Every bullet flight can be logged and replayed against a
  reference trajectory to validate that no build regressed physics.
- **Five-phase build order.** The module dependency graph is arranged into build phases
  that each produce a testable artifact. Core and interior ballistics ship first,
  then external and environment, then penetration and terminal effects, then degradation,
  and finally ACE3 integration with compatibility data packs.

## Quick Start

### Prerequisites

- Rust toolchain (stable)
- HEMTT (Arma build tool)
- ARMA 3 (for runtime testing)

### Build and Test

```bash
# Clone the repository
git clone https://github.com/lErrorl404l/AceBallisticsExtended.git
cd AceBallisticsExtended

# Run Rust physics tests (no ARMA 3 required)
cargo test

# Lint Rust code
cargo clippy

# Build the mod
hemtt build

# Run SQF unit tests
hemtt test

# Integration regression tests (requires ARMA 3)
act run --headless
```

### Add a Weapon Config

Create a JSON file in `data/weapons/` following the weapon schema:

```json
{
  "class": "my_weapon",
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

CI validates the JSON against the schema and runs reference trajectory checks. If
validation passes, data is bundled into the next release's compatibility PBO and the
weapon appears in-game automatically. No code change required.

## Project Structure

```
AceBallisticsExtention/
  ├──   ext/                    — Rust extension (physics engine)
  │   ├── src/
  │   │   ├── lib.rs              — C ABI dispatcher
  │   │   ├── interior.rs         — Interior ballistics (AVG_PRESSURE_FACTOR=0.58)
  │   │   ├── exterior.rs         — External ballistics (RK4 integration)
  │   │   ├── penetration.rs      — De Marre + Lanz-Odermatt penetration models
  │   │   ├── heat_penetration.rs — HEAT shaped-charge jet penetration
  │   │   ├── fragmentation.rs    — Fragment mass distribution / spray cone
  │   │   ├── behind_armor_debris.rs — Spall and BAD generation
  │   │   ├── atmosphere.rs       — ICAO standard atmosphere model
  │   │   ├── drag.rs             — G1/G7/G8 drag curves / CDM lookup
  │   │   └── config.rs           — JSON data table loader
  │   └── Cargo.toml
  ├── data/                 — Weapon, ammo, armor, vehicle configuration (JSON)
  ├── ext/tests/            — Rust integration and compatibility tests
  ├── docs/                 — Documentation
  └── tools/                — Data population and validation scripts
```

## Contributing

Data contributions are the primary community engagement vector. The barrier to entry is
JSON literacy, not C++ or SQF experience. See the [Data Format](data-format.md) page
for the full schema reference.

1. Fork the repository.
2. Add weapon JSON to `data/weapons/`.
3. CI validates JSON against schema and runs reference trajectory check.
4. If validation passes, the PR is merged.
5. Data is bundled into the next release's compatibility PBO.
6. Weapon appears in-game automatically.

Architecture and design decisions are documented in [Architecture](architecture.md).
For a per-module breakdown, see the [Module Guide](module-guide.md).
