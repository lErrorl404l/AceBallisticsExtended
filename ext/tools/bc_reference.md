# ABE Ballistic Coefficient Reference & Inference Methodology

## 1. What is a Ballistic Coefficient?

A ballistic coefficient (BC) quantifies a projectile's ability to overcome air
drag.  In the standard small-arms drag model:

```
BC = SD / i
```

where **SD** (sectional density) = mass / cross-sectional area, and **i** is a
dimensionless *form factor* relating the projectile's drag to a **reference
curve**.

| Quantity | Imperial                      | Metric                          |
|----------|-------------------------------|----------------------------------|
| SD       | (weight_gr / 7000) / (d_in²)  | mass_kg / (π·d²/4)              |
| BC       | lb/in² (conventional)         | kg/m² (seldom used directly)    |

Higher BC = less drag = better long-range performance = less wind drift and
drop.

---

## 2. Drag Models: G1 vs G7

There are several standard drag models; the two most common in ballistics
software are **G1** and **G7**.

### G1 (Ingalls)

- Reference projectile: flat-base, 1-inch diameter, 3-inch long, 1-pound
  bullet (archaic — 1880s era design).
- Best suited for **flat-base handgun and slow rifle bullets**.
- Overestimates drag at long range for modern boat-tail projectiles.
- Historically the most common BC published by ammunition manufacturers.

### G7 (Sierra / Litz)

- Reference projectile: 1-inch diameter, 3.7-inch long, boat-tail design
  (~20° included angle at the tail).
- Best suited for **modern boat-tail rifle bullets** (most military and match
  ammunition made after ~1950).
- More accurate across the transonic region.
- The primary drag model for ABE.

### When to use which

| Ammunition type             | Recommended model |
|-----------------------------|-------------------|
| Modern boat-tail rifle      | **G7**            |
| Handgun / pistol            | G1 (or G1→G7 ×0.50) |
| Subsonic rifle (9×39, .300 BLK sub) | G1 is more reliable; G7 is acceptable with conversion |
| .22 LR rimfire              | G1                |
| Flat-base rifle (M43 7.62×39) | G7 acceptable, note higher form factor |

### G1 → G7 conversion

There is **no universal conversion factor** — it depends on velocity and
projectile shape.  A commonly used approximation for handgun bullets and
blunt rifle bullets is:

```
G7 ≈ G1 × 0.50   (±0.05)
```

For modern boat-tail bullets the factor is closer to 0.48–0.52.  ABE always
prefers **measured G7 values** from Doppler radar or published references.

---

## 3. BC Estimation Methodology

### 3.1 Formula approach

When no published BC is available, ABE estimates G7 BC using:

```
BC_G7 = mass_kg / (A · CD_ref · ρ₀ · i)
```

where:

| Symbol | Value     | Description                        |
|--------|-----------|------------------------------------|
| A      | —         | Cross-sectional area (π·d²/4, m²)  |
| CD_ref | 0.259     | G7 drag coefficient at ~Mach 2     |
| ρ₀     | 1.225     | Sea-level air density (kg/m³)      |
| i      | varies    | Form factor (projectile shape)     |

The form factor **i** is chosen from lookup tables based on projectile type:

| Projectile type | Form factor (i) | Notes                         |
|-----------------|-----------------|-------------------------------|
| FMJ flat-base   | 0.95            | Standard ball ammunition      |
| FMJ boat-tail   | 0.92            | Modern rifle ball             |
| Hollow-point    | 0.88            | Match / open-tip HP           |
| Soft-point      | 0.90            | Hunting bullet                |
| Armour-piercing | 0.96            | Steel/heavy core              |
| API             | 0.96            | AP incendiary                 |
| Tracer          | 1.00            | Usually less streamlined      |
| JHP             | 0.90            | Jacketed hollow-point         |

If projectile **length** is available, the L/D ratio refines the form factor:

| L/D ratio | i      | Example                          |
|-----------|--------|----------------------------------|
| > 5.0     | 0.85   | VLD match bullets (Berger 168+)  |
| 4.0–5.0   | 0.90   | Long boat-tail (M118LR, Mk262)   |
| 3.0–4.0   | 0.95   | Medium rifle (M80, M855)         |
| < 3.0     | 1.00   | Short / pistol (.45 ACP, 9 mm)   |

### 3.2 Confidence bounds

Estimated BC values carry **confidence intervals** computed from the form-factor
uncertainty:

- **±0.05** when L/D ratio is known (length provided).
- **±0.08** when only projectile type is known (no length).

The confidence label follows this scheme:

| Label       | Meaning                                                  |
|-------------|----------------------------------------------------------|
| **high**    | BC is a published reference measurement (±3% error).     |
| **moderate**| BC estimated from L/D ratio or well-known type family.   |
| **low**     | BC estimated from projectile type only — ±10–15% spread. |
| **very_low**| BC estimated with minimal data — ±20% spread.            |

ABE outputs `bc_low` and `bc_high` fields alongside the nominal `bc_g7` for
every estimate.

---

## 4. Cross-referencing Methodology

Every entry in the reference database (REFERENCE_DB in `infer_bc.py`) comes
from at least one published source:

| Source code | Full reference                                           |
|-------------|----------------------------------------------------------|
| [LITZ]      | Bryan Litz, *Applied Ballistics for Long Range Shooting*, 4th Ed., 2021. Doppler radar measurements. |
| [LITZ-B]    | Bryan Litz, *Ballistic Performance of Rifle Bullets*, 3rd Ed., 2019. |
| [APG]       | US Army Aberdeen Proving Ground / ARL test reports.      |
| [HORN]      | Hornady 11th Edition Handbook of Cartridge Reloading.     |
| [SIERRA]    | Sierra Bullets Reloading Manual, 6th Ed.                  |
| [LAPUA]     | Lapua / Vihtavuori Reloading Guide, 2023.                 |
| [JBM]       | JBM Ballistics Library — jbmballistics.com               |
| [MIL]       | NATO / US Army DODIC ammunition specifications.           |
| [FED]       | Federal Premium Ammunition ballistic specs.                |
| [NOS]       | Nosler Reloading Guide 10th Ed.                           |

### Cross-reference procedure

1. **Prefer G7 measurements**: Doppler radar G7 BC is the gold standard
   (traceable to Litz, APG/ARL).  Where a G7 value is published it is used
   directly.

2. **G1→G7 conversion**: Where only G1 BC is available (common for handgun,
   Russian, and older military ammunition), the G1 value is converted using
   the projectile-specific factor.  For modern boat-tails: ×0.505; for
   handgun/blunt: ×0.50.

3. **Corroboration**: Every value is checked against at least two independent
   sources where possible.  Single-source entries are marked "est." in the
   source field.

4. **Confidence tagging**: Reference entries are tagged "high" confidence by
   default (measured data).  Entries based on estimation or conversion are
   noted in the source string.

### Known limitations

- **Velocity dependence**: BC is not truly constant — it changes slightly
  across the velocity regime (especially through transonic ~Mach 0.8–1.2).
  The published BC is usually an average over the useful velocity range.
- **Subsonic rounds**: G7 is calibrated for supersonic projectiles.  Purely
  subsonic ammunition (9×39, 12.7×55, .300 BLK subsonic) may not match the
  G7 curve well.  For these, either use G1 directly or accept wider error.
- **Weather**: All BC values assume ICAO standard sea-level conditions
  (15 °C, 1013.25 hPa, 50% RH).  Extreme temperatures or altitude affect
  effective BC.

---

## 5. AmmoConfig Schema

Generated JSON files follow the Rust `AmmoConfig` / `ProjectileConfig` structs
(in `ext/src/config.rs`):

```json
{
    "class": "ammo_magazine_class_name",
    "projectile": {
        "model": "projectile_model_name",
        "mass_g": 4.0,
        "caliber_mm": 5.56,
        "bc_g7": 0.151,
        "cdm_id": "g7",
        "fragmentation": {
            "threshold_vel_ms": 762.0,
            "avg_fragments": 12,
            "mass_distribution": "log_normal",
            "params": {"mean": 0.08, "std": 0.04}
        },
        "_source": "Published reference or estimation note",
        "_meta": {
            "bc_low": 0.146,
            "bc_high": 0.156,
            "confidence": "high",
            "method": "reference"
        }
    }
}
```

The optional `_meta` block carries inference metadata; the Rust physics engine
ignores unknown fields via serde.

---

## 6. Using the Inference Tool

```bash
# Diagnostic table of all reference entries
python infer_bc.py

# Infer BCs from an ammo input JSON
python infer_bc.py --input my_ammo.json

# Generate ammo JSONs from a weapon config
python infer_bc.py --weapon data/weapons/rhs_weap_m4a1.json

# Generate one ammo config directly from specs
python infer_bc.py --generate-ammo 7.62 9.5 fmj M80

# Override output dir
python infer_bc.py --input ammo.json --output-dir /tmp/ammo_out

# Force overwrite existing files
python infer_bc.py --weapon weapon.json --force
```

See the tool's built-in `--help` for the full option list.

---

## 7. Current Coverage

As of this writing the reference database covers **~55 entries across
~15 calibre groups**:

| Calibre group               | Entries |
|-----------------------------|---------|
| 5.56×45 / .223              | 7       |
| 5.45×39                     | 4       |
| 7.62×51 / .308              | 7       |
| 7.62×54R                    | 3       |
| 7.62×39                     | 3       |
| 6.5 mm (Creedmoor, Grendel) | 5       |
| .338 / 8.6 mm               | 4       |
| 9.3 mm (Brenneke, 9.3×62)   | 3       |
| .408 CheyTac                | 2       |
| .50 BMG / 12.7×99           | 2       |
| 9×19 / 9 mm                 | 5       |
| .45 ACP                     | 4       |
| 9×39 (subsonic)             | 2       |
| 12.7×55 (subsonic)          | 1       |
| .22 LR                      | 3       |
| 6.8×51 (.277 Sig Fury)      | 1       |

Calibre groups with fewer than 2 entries should be considered placeholder
values.  Community contributions of measured G7 BCs are welcome.
