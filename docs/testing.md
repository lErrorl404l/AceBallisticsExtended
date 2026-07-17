# Testing

ABE is designed for headless testability. The Rust extension is pure library code with no
ARMA 3 dependency. SQF modules are unit-testable via HEMTT's test runner. Integration
tests use ACT in CI only. This page documents the testing strategy, commands, and
coverage targets.

## Table of Contents

- [Testing Philosophy](#testing-philosophy)
- [Rust Extension Tests](#rust-extension-tests)
- [Data Validation Tests](#data-validation-tests)
- [SQF Unit Tests](#sqf-unit-tests)
- [Integration Tests](#integration-tests)
- [Coverage Targets](#coverage-targets)
- [CI Pipeline](#ci-pipeline)

## Testing Philosophy

ABE's test strategy follows a layered approach. The bulk of computational logic lives in
the Rust extension, so the bulk of tests live there too. Each layer has its own tooling
and runs at different points in the development workflow.

1. **Physics correctness.** Every physics kernel is a pure function: call it with known
   inputs, assert known outputs. Property-based tests generate random valid inputs and
   assert invariants (drag always reduces velocity, penetration never increases
   projectile energy).
2. **Data integrity.** Every JSON data file is validated against its schema at test
   time. Invalid data is caught before any runtime execution.
3. **Orchestration correctness.** SQF tests verify that events fire, data is passed to
   the extension correctly, and results are applied to the game state. These run under
   HEMTT with a minimal engine context.
4. **Regression prevention.** Integration tests fire virtual weapons with known
   parameters and verify muzzle velocity, impact point, and penetration verdict against
   reference tables. These run in CI only.

## Rust Extension Tests

### Running Tests

```bash
# Run all Rust tests (covers physics kernels + data validation)
cargo test

# Run tests with output
cargo test -- --nocapture

# Run a specific test
cargo test barrel_length_increases_muzzle_velocity

# Release-mode tests (slower compile, runs benchmarks)
cargo test --release

# Lint
cargo clippy
```

### Physics Kernel Tests

Every physics kernel ships with property-based and regression tests. This covers
roughly 95% of the mod's computational surface without any ARMA 3 involvement.

```rust
// Interior ballistics: longer barrel gives higher MV
#[test]
fn barrel_length_increases_muzzle_velocity() {
    let short = interior::calc_muzzle_velocity(barrel: 10.1, chamber: 400.0, caliber: 5.56);
    let long  = interior::calc_muzzle_velocity(barrel: 20.0, chamber: 400.0, caliber: 5.56);
    assert!(long.mv > short.mv);
    assert!((long.mv - short.mv) - expected_delta < 1.0);
}

// Drag: coefficient rises measurably through transonic
#[test]
fn drag_diverges_at_transonic() {
    let subsonic = drag::cdm(Mach: 0.6, projectile: "M80");
    let transonic = drag::cdm(Mach: 0.95, projectile: "M80");
    assert!((transonic - subsonic).abs() > 0.05);
}

// Penetration: angle reduces penetration capability
#[test]
fn penetration_drops_with_angle() {
    let pen_0  = penetration::calc(velocity: 850, mass: 9.5, caliber: 7.62, angle_deg: 0);
    let pen_60 = penetration::calc(velocity: 850, mass: 9.5, caliber: 7.62, angle_deg: 60);
    assert!(pen_60.penetrated <= pen_0.penetrated);
    assert!(pen_60.effective_thickness_mm > pen_0.effective_thickness_mm);
}
```

### Test Patterns

**Property-based tests.** For each physics kernel, tests assert invariant properties
that must hold for any valid input:

- Drag force always opposes velocity (deceleration, never acceleration).
- Penetration depth monotonically increases with impact velocity.
- Ricochet probability monotonically increases with impact angle.
- Muzzle velocity monotonically increases with barrel length (for a given chamber
  pressure).
- Air density monotonically decreases with altitude.

**Regression tests.** For each kernel, tests compare output against reference values
from published ballistics tables or empirical data:

- M855 trajectory at 100m increments compared to published M855 data.
- 7.62x51mm NATO penetration of 10mm RHA at 100m compared to NATO test data.
- Muzzle velocity for standard military cartridges compared to manufacturer specs.

**Benchmark tests.** Hot-path functions use Criterion benchmarks to track performance
regressions:

```bash
cargo bench
```

All hot paths are benchmarked. The CI pipeline fails if a benchmark regresses more
than 5% from the baseline.

## Data Validation Tests

All JSON data tables are validated against JSON schemas at test time. A test loads every
file in each data directory and validates it.

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

Similar tests exist for ammo and armor data. These tests run as part of `cargo test`
and on every CI push. They catch malformed data before any ARMA 3 process starts.

## SQF Unit Tests

SQF modules are tested with HEMTT's built-in test runner. These tests verify
orchestration logic: that events are dispatched correctly, data is passed to the
extension, and results are handled.

```bash
hemtt test
```

```sqf
// Test that extension loader works
["ABE extension loads correctly", {
    private _result = "abe_ballistics_ext" callExtension ["init", [1, false]];
    assert_equal(_result select 0, "OK");
}] call test_framework;

// Test that fire event dispatches correct parameters
["Fire event calls extension with correct params", {
    private _weapon = "rhs_weap_m4a1";
    private _result = call_abe_interior(_weapon);
    assert_equal(typeName _result, "ARRAY");
    assert_equal(count _result, 4);
}] call test_framework;
```

HEMTT's test runner starts a minimal ARMA 3 context with no renderer and no world
simulation. It is much lighter than a full client launch but still requires the engine
binary. SQF tests are used for orchestration-level verification only. Physics logic is
never tested in SQF.

## Integration Tests

Integration tests fire virtual weapons with known parameters and verify the full pipeline
from muzzle to impact. These use ACT (Arma Continuous Testing) and run in GitHub Actions
CI only.

```bash
act run --headless
```

Regression missions verify:

- Muzzle velocity within 2% of reference table.
- Impact point within 0.1 MIL of reference trajectory.
- Penetration verdict matches lookup table for 10 or more calibrated shots.
- Ricochet angle matches empirical data for standard impact scenarios.
- Fragmentation pattern statistics match reference distributions.

These tests run on PR merge to main and before releases only, because each run takes
2-5 minutes.

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

## Coverage Targets

| Layer | Tool | Coverage Target | ARMA 3 Needed |
|---|---|---|---|
| Physics kernels | `cargo test` | 100% branch coverage | No |
| Data validation | `cargo test` | 100% config schemas | No |
| SQF orchestration | `hemtt test` | 90%+ module logic | Minimal engine context |
| Integration | ACT headless | Key regression scenarios | Yes (CI only) |
| Performance | Criterion (Rust) | All hot paths benchmarked | No |

## CI Pipeline

The GitHub Actions CI runs three workflows:

### test.yml (on push and PR)

```yaml
jobs:
  rust:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo test
      - run: cargo clippy -- -D warnings
  sqf:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: grupp-p/hemtt-setup@v1
      - run: hemtt test
```

### build.yml (on push to main)

```yaml
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo test --release
      - run: hemtt build --release
      - run: ./scripts/build-cross.sh
```

### release.yml (on tag)

Triggered by version tags (e.g. `v0.1.0`). Runs all tests, builds all targets, generates
changelog, and publishes the release artifact.

## Quick Reference

```bash
# Most common workflow
cargo test                    # Physics + data validation (< 2s)
cargo clippy                  # Lint

# Before merge
cargo test --release          # Full physics suite
hemtt build                   # Build mod
hemtt test                    # SQF orchestration tests

# Before release
act run --headless            # Integration regression (CI only)
./scripts/build-cross.sh      # Cross-compile for Windows + Linux
```
