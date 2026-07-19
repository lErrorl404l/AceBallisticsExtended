# Contributing to Ace Ballistics Extension

## Adding Weapons, Ammo, or Armor

### 1. Create a JSON File

Follow the schemas in `data/schemas/`:

- **Weapons**: `data/weapons/<class_name>.json` — requires `class`, `caliber_mm`, `barrel_length_mm`
- **Ammo**: `data/ammo/<caliber_or_model>.json` — requires `class`, `projectile.mass_g`, `projectile.caliber_mm`
- **Armor**: `data/armor/plates/<vehicle_id>.json` — requires `vehicle`, `plates[].material`, `plates[].thickness_mm`

Use `snake_case` for all JSON keys.

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

### Using `infer_bc.py`

Automates BC lookup/estimation from `ext/tools/infer_bc.py`:

```bash
# Diagnostic table of all 40+ reference entries
python ext/tools/infer_bc.py
# Infer BCs from JSON input, write ammo configs
python ext/tools/infer_bc.py --input ammo.json --output-dir data/ammo
# Skip reference DB, always estimate
python ext/tools/infer_bc.py --input ammo.json --mode formula
```

Input: `{"ammo": [{"class": "B_556x45_Ball", "model": "M855", "mass_g": 4.0, "caliber_mm": 5.56, "type": "fmj", "cdm_id": "g7"}]}`

Two modes: **infer** (default) matches against a built-in DB of published G7 BC values (Litz, US Army BRL, Hornady, Sierra, Lapua, JBM; 40+ projectiles across 13 calibres). **formula** estimates via form-factor heuristics when no reference exists.

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

- All functions are pure (no global state, no I/O)
- Interior: convergence with barrel length, sane MV ranges
- Exterior: energy non-increasing, monotonic position, free-fall
- Drag: G7 < G1 at all Mach, transonic peak near M=1
- Penetration: AP > ball, ricochet at grazing angles
- Fragmentation: no fragmentation below threshold, mass conservation within 10%

The Python validation harness (`tests/validate_data.py`) checks JSON schema compliance, field ranges, naming conventions, and cross-references. All 230+ checks must pass.

## PR Process

### Format Rules

| Context        | Convention      | Example                          |
|----------------|-----------------|----------------------------------|
| JSON keys      | snake_case      | `barrel_length_mm`, `chamber_pressure_mpa` |
| Rust code      | snake_case      | `calc_muzzle_velocity()`, `wind_shear_factor()` |
| SQF functions  | camelCase       | `ABE_fnc_init`, `ABE_fnc_fire`   |
| Rust types     | PascalCase      | `FireParams`, `BulletState`, `ImpactResult` |

### PR Workflow Checklist

Before opening a PR:

- [ ] `cargo test` — 1134+ tests pass
- [ ] `python tests/validate_data.py` — 230+ checks pass
- [ ] `cargo clippy` — no new warnings
- [ ] `cargo doc --no-deps` — clean
- [ ] New JSON configs have a `notes` field with BC source
- [ ] All empirical constants cite their source
- [ ] Branch targets `main`, only relevant changes

### Review Process

1. Automated: `cargo test` + `python tests/validate_data.py` + `cargo clippy`
2. Manual: physics correctness, data quality, edge cases
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

# Documentation
cargo doc --no-deps --open
```

For SQF changes, use `hemtt dev` which enables file patching for instant iteration without rebuilding PBOs.

## License

By contributing, you agree that your contributions will be licensed under the
**GNU General Public License v3.0** — see [`LICENSE`](LICENSE).
