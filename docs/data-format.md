# Data Format

ABE uses JSON for all configuration data. Every data file is validated against a JSON
schema at compile and test time, which catches invalid entries before any runtime
execution. This page documents the three core schemas: weapon configs, ammo configs, and
armor configs.

## Table of Contents

- [Weapon Config](#weapon-config)
- [Ammo Config](#ammo-config)
- [Armor Config](#armor-config)
- [Schema Validation](#schema-validation)
- [Data Contribution Workflow](#data-contribution-workflow)

## Weapon Config

Weapon configs define the physical properties of a firearm that affect interior and
exterior ballistics. Each file in `data/weapons/` represents one weapon class.

### Schema

| Field | Type | Required | Description |
|---|---|---|---|
| `class` | string | yes | ARMA 3 class name of the weapon (e.g. `rhs_weap_m4a1`) |
| `caliber_mm` | number | yes | Bullet caliber in millimeters |
| `barrel_length_mm` | number | yes | Barrel length in millimeters |
| `rifling_twist_mm` | number | yes | Rifling twist rate as millimeters per revolution |
| `chamber_pressure_mpa` | number | yes | Maximum chamber pressure in megapascals |
| `cdm_id` | string | no | CDM (Coefficient of Drag Model) identifier. Falls back to caliber-based default if omitted |
| `projectile_mass_g` | number | yes | Standard projectile mass in grams |
| `muzzle_velocity_ms` | number | yes | Reference muzzle velocity in meters per second |
| `zero_range_m` | number | no | Factory zero range in meters. Defaults to 100 if omitted |

### Example

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

### Field Details

**class.** Must match the `CfgWeapons` entry in ARMA 3. This is the key used by SQF to
look up the weapon's ballistics data at runtime. Convention uses the full Arma class path
(e.g. `rhs_weap_m4a1`, `CUP_arifle_M4A1`).

**barrel_length_mm.** Measured from breech face to muzzle crown. ABE uses this to
calculate muzzle velocity scaling when the in-game barrel length differs from the
reference length. The interior ballistics model applies a pressure-integral approximation
based on the ratio of actual to reference barrel length.

**rifling_twist_mm.** One complete rifling rotation in millimeters. Used for gyroscopic
stability calculations and spin drift correction. A shorter twist rate (tighter spiral)
gives more spin, which stabilizes longer bullets but increases spin drift.

**chamber_pressure_mpa.** SAAMI or CIP peak pressure for the cartridge. ABE uses this
in the propellant burn model to compute the pressure curve and muzzle velocity for
barrel lengths different from the reference.

**cdm_id.** References a drag coefficient table in the extension's built-in CDM library.
Built-in IDs include common military projectile profiles (M855, M80, M118LR, 7N6,
9x19_FMJ). If omitted, the extension selects a CDM based on the caliber-to-mass ratio.
Community contributors can request new CDM entries for unusual projectiles.

## Ammo Config

Ammo configs describe the projectile properties for a specific ammunition type. Each file
in `data/ammo/` represents one magazine or ammunition entry.

### Schema

| Field | Type | Required | Description |
|---|---|---|---|
| `class` | string | yes | ARMA 3 class name of the magazine or ammo (e.g. `rhs_mag_30Rnd_556x45_M855_Stanag`) |
| `projectile.model` | string | yes | Projectile model identifier (e.g. `m855`) |
| `projectile.mass_g` | number | yes | Projectile mass in grams |
| `projectile.caliber_mm` | number | yes | Projectile caliber in millimeters |
| `projectile.bc_g7` | number | no | G7 ballistic coefficient. If omitted, derived from mass, caliber, and CDM |
| `projectile.cdm_id` | string | no | Override CDM identifier for this specific projectile. Inherits from weapon config if omitted |
| `projectile.fragmentation.threshold_vel_ms` | number | no | Minimum velocity in m/s at which the projectile fragments on impact. Omit for non-fragmenting rounds |
| `projectile.fragmentation.avg_fragments` | integer | no | Average number of fragments produced on fragmentation |
| `projectile.fragmentation.mass_distribution` | string | no | Statistical distribution of fragment masses. Valid values: `log_normal`, `uniform` |
| `projectile.fragmentation.params.mean` | number | no | Mean fragment mass in grams (for log-normal distribution) |
| `projectile.fragmentation.params.std` | number | no | Standard deviation of fragment mass in grams (for log-normal distribution) |

### Example

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
      "params": {
        "mean": 0.08,
        "std": 0.04
      }
    }
  }
}
```

### Field Details

**bc_g7.** The G7 ballistic coefficient is the preferred form for ABE because G7 models
modern spitzer boat-tail projectiles better than G1. If you only have a G1 BC, ABE can
convert it internally, but G7 is preferred for accuracy. BC values are velocity-dependent
in the extension; the value here is the reference BC at Mach 2.

**fragmentation.** The fragmentation block is optional. Non-fragmenting rounds (solid
core, AP, frangible that powderizes rather than fragments) omit this block entirely. The
`threshold_vel_ms` field is the velocity below which the projectile deforms rather than
fragments. Above this threshold, the `avg_fragments` and mass distribution parameters
control the fragment spray.

**mass_distribution.** Two distribution models are supported:

- `log_normal`: Realistic fragment mass distribution. The `mean` and `std` parameters
  define the log-normal distribution of fragment masses. This is the default and
  recommended model for most projectiles.
- `uniform`: Simplified equal-mass fragments. Only `avg_fragments` is used. Useful for
  performance-constrained scenarios or when fragment data is unavailable.

## Armor Config

Armor configs define the armor protection arrays for vehicles. Each file in
`data/armor/` represents one vehicle.

### Schema

| Field | Type | Required | Description |
|---|---|---|---|
| `vehicle` | string | yes | ARMA 3 class name of the vehicle (e.g. `rhs_btr80a_msv`) |
| `plates[].name` | string | yes | Identifier for this armor plate (e.g. `hull_front`, `turret_side`) |
| `plates[].material` | string | yes | Material type. See material database below |
| `plates[].thickness_mm` | number | yes | Plate thickness in millimeters (RHA equivalent for composite materials) |
| `plates[].angle_deg` | number | yes | Plate angle from vertical in degrees. 0 is vertical, 90 is horizontal |
| `plates[].backing` | string | no | Backing material or spall liner identifier. `null` for no backing |

### Example

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

### Material Database

The extension includes a built-in material properties table. Each material has density,
strength coefficient, and spall characteristics.

| Material ID | Description | Density (g/cm3) | Strength Factor |
|---|---|---|---|
| `steel_rha` | Rolled homogeneous armor steel | 7.85 | 1.0 |
| `steel_hha` | High hardness armor steel | 7.85 | 1.3 |
| `aluminum_5083` | Marine-grade aluminum | 2.66 | 0.4 |
| `aluminum_7075` | High-strength aluminum alloy | 2.81 | 0.55 |
| `ceramic_al2o3` | Alumina ceramic tile | 3.95 | 2.5 |
| `ceramic_sic` | Silicon carbide ceramic | 3.21 | 3.0 |
| `composite_kevlar` | Kevlar/epoxy composite | 1.44 | 0.6 |
| `composite_uhmwpe` | Ultra-high molecular weight polyethylene | 0.97 | 0.7 |
| `era_heavy` | Heavy explosive reactive armor (Russian-style) | - | 3.5 |
| `era_light` | Light explosive reactive armor (Western tile) | - | 2.0 |
| `spall_liner_kevlar` | Kevlar spall liner | 1.44 | 0.3 |
| `spall_liner_rhino` | Rhino liner / anti-spall coating | 1.1 | 0.2 |
| `concrete` | Structural concrete | 2.4 | 0.15 |
| `glass_armored` | Armored glass laminate | 2.5 | 0.4 |

### Field Details

**plates[].material.** Must be one of the material IDs from the material database.
Custom materials can be registered in the extension's material table. The strength factor
multiplies the baseline penetration resistance calculated by the De Marre or L-O formula.

**plates[].angle_deg.** The angle is measured from vertical. A hull front plate angled
at 45 degrees presents roughly 1.4x the line-of-sight thickness (cosine of 45 is 0.707).
The penetration model applies this geometric effect plus a material-specific
angle-decoupling factor.

**plates[].backing.** When a backing material is specified, the armor array is evaluated
as a layered system. The projectile must penetrate each layer in sequence, with energy
reduced after each layer. This models spaced armor, composite arrays with ceramic strike
faces, and spall liners.

### Layered Armor Arrays

For complex armor arrays with multiple layers, use multiple entries in the `plates`
array for the same face. The penetration model evaluates each layer sequentially.

```json
{
  "vehicle": "rhs_t72ba",
  "plates": [
    { "name": "hull_front_era", "material": "era_heavy", "thickness_mm": 10, "angle_deg": 68, "backing": null },
    { "name": "hull_front_steel", "material": "steel_rha", "thickness_mm": 80, "angle_deg": 68, "backing": null },
    { "name": "hull_front_liner", "material": "spall_liner_kevlar", "thickness_mm": 10, "angle_deg": 68, "backing": null }
  ]
}
```

## Schema Validation

Every JSON data file is validated against a JSON Schema at compile and test time. The
schemas live in `ext/src/data/schemas/`. A Rust test loads every file in each data
directory and validates it:

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

This catches bad data before any ARMA 3 process starts. Contributors should run
`cargo test` locally to validate their data files before submitting a PR.

## Data Contribution Workflow

1. Fork the repository and create a branch.
2. Add weapon JSON to `data/weapons/`, ammo JSON to `data/ammo/`, or armor JSON to
   `data/armor/`.
3. Run `cargo test` locally to validate against schemas.
4. Submit a PR. CI runs the same validation plus reference trajectory checks.
5. If validation passes, the PR is merged. Data is bundled into the next release's
   compatibility PBO.
6. The weapon or vehicle appears in-game automatically. No code change required.
