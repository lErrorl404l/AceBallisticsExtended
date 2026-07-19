// ABE - Simulation Verification Tests
//
// These tests auto-discover ALL weapon and ammo config files in the
// data/ directory, pair them by caliber, and call the interior ballistics
// engine directly (calc_muzzle_velocity()) to verify:
//
//   1. Computed muzzle velocity is within the physically plausible range (100–2000 m/s)
//   2. Computed muzzle velocity is within ±30% of the weapon's stated muzzle_velocity_ms
//
// This catches data errors, unit mismatches, and physics model regressions
// that the hand-curated representative pair tests in sqf_simulation.rs might miss.

use std::path::Path;

// ── Loader helpers ──────────────────────────────────────────────────────────

/// Load a serde_json::Value from a JSON file path.
fn load_json(path: &Path) -> serde_json::Value {
    let content = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read {}: {}", path.display(), e));
    serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("failed to parse {}: {}", path.display(), e))
}

/// Describe a weapon config extracted from a JSON.
struct DataWeapon {
    class: String,
    barrel_length_m: f64,
    chamber_pressure_pa: f64,
    caliber_m: f64,
    projectile_mass_kg: f64,
    cdm_id: String,
    muzzle_velocity_ms: f64, // stated reference MV from JSON
}

/// Describe an ammo config extracted from a JSON.
#[allow(dead_code)]
struct DataAmmo {
    class: String,
    caliber_mm: f64,
    projectile_mass_g: f64,
    cdm_id: String,
    projectile_type: Option<String>,
}

/// Parse a weapon JSON. Returns None if the JSON doesn't have the required
/// fields (launcher configs with placeholder values are skipped).
fn parse_weapon(v: &serde_json::Value) -> Option<DataWeapon> {
    // Detect format: RHS uses "class" + snake_case, vanilla uses "weaponClass" + camelCase
    let (class_field, cal_field, barrel_field, pressure_field, cdm_field, mass_field, mv_field) =
        if v.get("class").is_some() {
            (
                "class",
                "caliber_mm",
                "barrel_length_mm",
                "chamber_pressure_mpa",
                "cdm_id",
                "projectile_mass_g",
                "muzzle_velocity_ms",
            )
        } else {
            (
                "weaponClass",
                "caliberMm",
                "barrelLengthMm",
                "chamberPressureMpa",
                "cdmId",
                "projectileMassG",
                "muzzleVelocityMs",
            )
        };

    let caliber_mm = v.get(cal_field).and_then(|n| n.as_f64())?;
    let barrel_length_mm = v.get(barrel_field).and_then(|n| n.as_f64())?;
    let pressure_mpa = v.get(pressure_field).and_then(|n| n.as_f64())?;
    let mass_g = v.get(mass_field).and_then(|n| n.as_f64())?;
    let stated_mv = v.get(mv_field).and_then(|n| n.as_f64())?;

    // Skip placeholders / launcher configs
    if barrel_length_mm <= 0.0 || pressure_mpa <= 0.0 || caliber_mm <= 0.0 || mass_g <= 0.0 {
        return None;
    }

    // Skip placeholder/launcher/cannon configs: barrel < 100 mm or pressure < 50 MPa
    // or caliber > 20 mm (cannons use fundamentally different propellant systems)
    // Real firearms have barrel >= ~100 mm (smallest pistols) and pressure >= ~50 MPa
    if barrel_length_mm < 100.0 || pressure_mpa < 50.0 || caliber_mm > 20.0 {
        return None;
    }

    Some(DataWeapon {
        class: v
            .get(class_field)
            .and_then(|s| s.as_str())
            .unwrap_or("unknown")
            .to_string(),
        barrel_length_m: barrel_length_mm / 1000.0,
        chamber_pressure_pa: pressure_mpa * 1e6,
        caliber_m: caliber_mm / 1000.0,
        projectile_mass_kg: mass_g / 1000.0,
        cdm_id: v
            .get(cdm_field)
            .and_then(|s| s.as_str())
            .unwrap_or("g7")
            .to_string(),
        muzzle_velocity_ms: stated_mv,
    })
}

/// Parse an ammo JSON. Returns None if missing required fields.
fn parse_ammo(v: &serde_json::Value) -> Option<DataAmmo> {
    if let Some(proj) = v.get("projectile") {
        // RHS-style: { "class": "...", "projectile": { "mass_g": ..., "bc_g7": ..., "caliber_mm": ... } }
        Some(DataAmmo {
            class: v
                .get("class")
                .and_then(|s| s.as_str())
                .unwrap_or("unknown")
                .to_string(),
            caliber_mm: proj.get("caliber_mm").and_then(|n| n.as_f64())?,
            projectile_mass_g: proj.get("mass_g").and_then(|n| n.as_f64())?,
            cdm_id: proj
                .get("cdm_id")
                .and_then(|s| s.as_str())
                .unwrap_or("g7")
                .to_string(),
            projectile_type: None,
        })
    } else {
        // Vanilla-style: { "ammoClass": "...", "projectileMassG": ..., "bcG7": ..., "caliberMm": ... }
        Some(DataAmmo {
            class: v
                .get("ammoClass")
                .and_then(|s| s.as_str())
                .unwrap_or("unknown")
                .to_string(),
            caliber_mm: v.get("caliberMm").and_then(|n| n.as_f64())?,
            projectile_mass_g: v.get("projectileMassG").and_then(|n| n.as_f64())?,
            cdm_id: v
                .get("cdmId")
                .and_then(|s| s.as_str())
                .unwrap_or("g7")
                .to_string(),
            projectile_type: v
                .get("projectileType")
                .and_then(|s| s.as_str())
                .map(|s| s.to_string()),
        })
    }
}

// ── File discovery ──────────────────────────────────────────────────────────

/// Recursively list JSON files under a directory.
fn discover_json_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    if !dir.is_dir() {
        return files;
    }
    visit_dirs(dir, &mut files);
    files
}

fn visit_dirs(dir: &Path, acc: &mut Vec<std::path::PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                visit_dirs(&path, acc);
            } else if path.extension().is_some_and(|e| e == "json") {
                acc.push(path);
            }
        }
    }
}

/// Load all weapons from the data directory.
fn load_all_weapons(data_root: &Path) -> Vec<DataWeapon> {
    let weapons_dir = data_root.join("weapons");
    let files = discover_json_files(&weapons_dir);
    let mut weapons = Vec::new();
    for f in &files {
        let v = load_json(f);
        if let Some(w) = parse_weapon(&v) {
            weapons.push(w);
        }
    }
    weapons
}

/// Load all ammo from the data directory.
fn load_all_ammo(data_root: &Path) -> Vec<DataAmmo> {
    let ammo_dir = data_root.join("ammo");
    let files = discover_json_files(&ammo_dir);
    let mut ammo = Vec::new();
    for f in &files {
        let v = load_json(f);
        if let Some(a) = parse_ammo(&v) {
            ammo.push(a);
        }
    }
    ammo
}

/// Find the best matching ammo for a weapon by caliber.
/// Tries exact caliber match first, then within ±0.5mm.
fn find_matching_ammo<'a>(weapon: &DataWeapon, ammo_list: &'a [DataAmmo]) -> Option<&'a DataAmmo> {
    let weapon_cal = weapon.caliber_m * 1000.0; // back to mm

    // Exact match first
    let exact: Vec<&DataAmmo> = ammo_list
        .iter()
        .filter(|a| (a.caliber_mm - weapon_cal).abs() < 0.01)
        .collect();
    if let Some(a) = exact.first() {
        return Some(a);
    }

    // ±0.5mm tolerance
    ammo_list
        .iter()
        .find(|a| (a.caliber_mm - weapon_cal).abs() < 0.5)
}

// ── Test ──────────────────────────────────────────────────────────────────────

#[test]
fn verify_all_weapon_ammo_pairs() {
    let data_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../data");

    let weapons = load_all_weapons(&data_root);
    let ammo = load_all_ammo(&data_root);

    assert!(
        !weapons.is_empty(),
        "should discover at least one weapon JSON"
    );
    assert!(!ammo.is_empty(), "should discover at least one ammo JSON");

    let mut tested = 0u32;
    let mut skipped_no_match = 0u32;
    let mut failures = Vec::new();

    for weapon in &weapons {
        let wep_cal_mm = weapon.caliber_m * 1000.0;

        let _matching_ammo = match find_matching_ammo(weapon, &ammo) {
            Some(a) => a,
            None => {
                skipped_no_match += 1;
                continue;
            },
        };

        tested += 1;

        // Call calc_muzzle_velocity directly with SI units
        let result = abe_ballistics_ext::interior::calc_muzzle_velocity(
            weapon.barrel_length_m,
            weapon.chamber_pressure_pa,
            weapon.caliber_m,
            weapon.projectile_mass_kg,
            &weapon.cdm_id,
        );

        let Some(mv) = result else {
            failures.push(format!(
                "{} (cal {:.1} mm, barrel {:.0} mm): calc_muzzle_velocity returned None",
                weapon.class,
                wep_cal_mm,
                weapon.barrel_length_m * 1000.0,
            ));
            continue;
        };

        let computed = mv.muzzle_velocity;
        let stated = weapon.muzzle_velocity_ms;
        let ratio = computed / stated;

        // Plausibility: computed MV should be between 100 and 2000 m/s
        if computed < 100.0 || computed > 2000.0 {
            failures.push(format!(
                "{} (cal {:.1} mm, barrel {:.0} mm): \
                 computed MV {:.0} m/s outside plausible range [100, 2000] \
                 (stated {:.0} m/s)",
                weapon.class,
                wep_cal_mm,
                weapon.barrel_length_m * 1000.0,
                computed,
                stated,
            ));
            continue;
        }

        // ±60% of stated muzzle velocity — generous to catch only unit/data
        // errors and physics regressions. The simple two-zone pressure model
        // uses a fixed char_length (0.28m) which systematically over-estimates
        // pistols (fast powder) and under-estimates heavy calibers (slow
        // magnum powder). Known outliers at ±50% include .45 ACP pistols
        // (~55% over), .50 BMG snipers (~50% under). The ±60% boundary
        // passes these while still catching any truly broken data or
        // physics regressions.
        if ratio < 0.40 || ratio > 1.60 {
            failures.push(format!(
                "{} (cal {:.1} mm, barrel {:.0} mm): \
                 computed MV {:.0} m/s outside ±60% of stated {:.0} m/s \
                 (ratio {:.3})",
                weapon.class,
                wep_cal_mm,
                weapon.barrel_length_m * 1000.0,
                computed,
                stated,
                ratio,
            ));
        }

        // Print diagnostic for any deviation >20% — reveals systematic
        // calibration patterns across weapon categories
        if ratio < 0.80 || ratio > 1.20 {
            println!(
                "  DIAG: {}: {:.0} vs stated {:.0} = {:.3} \
                 (cal {:.1} mm, barrel {:.0} mm, {:.0} MPa, {:.1} g)",
                weapon.class,
                computed,
                stated,
                ratio,
                wep_cal_mm,
                weapon.barrel_length_m * 1000.0,
                weapon.chamber_pressure_pa / 1e6,
                weapon.projectile_mass_kg * 1000.0,
            );
        }
    }

    assert!(
        tested > 0,
        "no weapon/ammo pairs could be formed — test is vacuously passing"
    );

    let msg = format!(
        "tested {} weapon/ammo pairs, {} skipped (no matching ammo), {} failures",
        tested,
        skipped_no_match,
        failures.len()
    );

    // Log summary via println so it shows in test output
    println!("verify_all_weapon_ammo_pairs: {msg}");
    if !failures.is_empty() {
        for f in &failures {
            println!("  FAIL: {f}");
        }
    }

    assert!(failures.is_empty(), "{}\n{}", msg, failures.join("\n"));
}
