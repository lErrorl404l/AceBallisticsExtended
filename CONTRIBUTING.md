# Contributing to Ace Ballistics Extension

## Adding Weapons, Ammo, or Armor

### 1. Create a JSON File

Follow the schemas in `data/schemas/`:

- **Weapons**: `data/weapons/<class_name>.json` — requires `weaponClass`, `barrelLengthMm`, `caliberMm`
- **Ammo**: `data/ammo/<caliber_or_model>.json` — requires `ammoClass`, `projectileMassG`, `caliberMm`
- **Armor**: `data/armor/<material_id>.json` — requires `materialId`, `densityGcm3`, `hardnessBHN`

Use `camelCase` for all JSON keys.

### 2. Validate

```bash
# Rust deserialization + schema validation
cargo test

# Python validation harness (230+ checks)
python tests/validate_data.py
```

### 3. Add Ballistic Coefficient References

Your ammo entry needs a `bc_g7` value. Sources for BC data:

- **Applied Ballistics** (Litz) — preferred for modern boat-tail spitzer bullets, G7 BC
- **JBM Ballistics** (jbmballistics.com) — free drag curve tables and BC calculator
- **Army Ballistic Research Laboratory** (ARL/APG) — BRL drag curves and BC data for military ammunition
- **Manufacturer data** — Hornady, Berger, Lapua, Federal publish G1/G7 BC

When adding a BC, include the source in the `notes` field:

```json
{
    "projectile": {
        "model": "m80",
        "bc_g7": 0.200,
        "source": "APG G7 BC=0.200 (7.62x51mm M80 ball, ARL-TR-5182)"
    }
}
```

If measured BC is unavailable, estimate from projectile shape and mass using Litz's empirical correlations or the JBM BC calculator.

### Drag Model Selection

| Drag Model | Use Case                                      |
|------------|-----------------------------------------------|
| G1         | Flat-base tangent-ogive bullets (legacy)      |
| G7         | Modern boat-tail spitzer bullets (preferred)  |
| G8         | Flat-base secant-ogive bullets                |
| GL         | Custom drag curves (uncommon)                 |

Most military rifle ammunition uses G7. Pistol ammunition often uses G1.

## Physics Improvements

### Module Locations

| Module          | File                          | Key Functions                   |
|-----------------|-------------------------------|---------------------------------|
| Interior        | `ext/src/interior.rs`         | `calc_muzzle_velocity()`        |
| Exterior        | `ext/src/exterior.rs`         | `calc_mach()`, `wind_drift()`   |
| Drag            | `ext/src/drag.rs`             | `get_cd()`, `table_lookup()`    |
| Atmosphere      | `ext/src/atmosphere.rs`       | `density_at_altitude()`, `wind_shear_factor()` |
| Penetration     | `ext/src/penetration.rs`      | `evaluate()`, `material_factor()` |
| Fragmentation   | `ext/src/fragmentation.rs`    | `evaluate()`, `inverse_normal_cdf()` |

### Adding a Drag Model

1. Add a const lookup table in `ext/src/drag.rs` following the `G1_TABLE`/`G7_TABLE` pattern
2. Add a dispatch arm in `get_cd()`
3. Add tests verifying the curve shape, transonic behavior, and smoothness
4. Run `cargo test`

Reference data: JBM Ballistics (jbmballistics.com), ABRA drag curves, NATO AOP-55 Annex A.

### Testing Expectations

- All physics functions are pure — no global state, no I/O
- Interior: convergence with barrel length, sane MV ranges for known cartridges
- Exterior: energy non-increasing, monotonic position, free-fall consistency
- Drag: G7 < G1 at all Mach, transonic peak near M=1, smooth interpolation
- Penetration: AP > ball against same target, ricochet at grazing angles
- Fragmentation: no fragmentation below threshold, mass conservation within 10%

## Data Validation

The Python validation harness in `tests/validate_data.py` checks:

- JSON Schema compliance against `data/schemas/*.json`
- File naming conventions (lowercase, underscores)
- Field ranges (barrel length 50–2000 mm, chamber pressure 50–700 MPa, etc.)
- BC value plausibility (0.01–10.0)
- Cross-references between weapon caliber and ammo caliber

```bash
python tests/validate_data.py
```

All 230+ validation checks must pass for a data PR to be accepted.

## PR Process

### Format Rules

| Context        | Convention      | Example                          |
|----------------|-----------------|----------------------------------|
| JSON keys      | camelCase       | `barrelLengthMm`, `chamberPressureMpa` |
| Rust code      | snake_case      | `calc_muzzle_velocity()`, `wind_shear_factor()` |
| SQF functions  | camelCase       | `ABE_fnc_init`, `ABE_fnc_fire`   |
| Rust types     | PascalCase      | `FireParams`, `BulletState`, `ImpactResult` |

### Requirements

Every PR must include:

1. **Tests** — Rust `#[test]` functions for any new code paths. Physics changes require trajectory integration tests.
2. **No magic numbers** — All empirical constants must reference their source in a comment or the module header.
3. **Schema validation** — New JSON configs must pass `cargo test` (deserialization) and `python tests/validate_data.py`.
4. **LSP clean** — No new Rust compiler warnings or clippy lints. Run `cargo clippy` before pushing.

### Review Process

1. Automated checks: `cargo test`, `python tests/validate_data.py`, `cargo clippy`
2. Manual review: physics model correctness, data source quality, edge case handling
3. Merge after one approving review from a maintainer

### Commit Messages

Follow conventional commits:

```
feat(data): add M118LR 7.62x51mm ammo config
fix(penetration): correct De Marre constant for AP projectiles
docs(contributing): add PR process section
test(exterior): add energy conservation trajectory test
```

## Development Environment

```bash
# Full build
./build.sh

# Iteration loop
cd ext && cargo build && cp target/debug/libabe_ballistics_ext.so . && cd .. && hemtt dev

# Run all tests
cargo test && python tests/validate_data.py

# Lint
cargo clippy
```

For SQF changes, use `hemtt dev` which enables file patching for instant iteration without rebuilding PBOs.
