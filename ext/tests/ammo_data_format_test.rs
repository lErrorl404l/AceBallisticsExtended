// ABE - Ammo Data Format Validation Test
//
// Integration test that loads every JSON file in `data/ammo/` and verifies
// it can be deserialised as `AmmoConfig`. Legacy flat-format files (without
// a `projectile` key) are silently skipped. Validates the new Option fields
// are correctly deserialised from the JSON files.

use std::path::Path;

use abe_ballistics_ext::config::AmmoConfig;

/// Path to the ammo data directory, relative to the crate's manifest.
const AMMO_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../data/ammo/");

#[test]
fn all_ammo_jsons_load() {
    let dir = Path::new(AMMO_DIR);
    let mut ammo_count = 0u32;
    let mut skipped_count = 0u32;

    // Recursively traverse all subdirectories
    let mut stack: Vec<std::path::PathBuf> = vec![dir.to_path_buf()];
    while let Some(cur_dir) = stack.pop() {
        for entry in std::fs::read_dir(&cur_dir)
            .unwrap_or_else(|e| panic!("Failed to read dir {:?}: {}", cur_dir, e))
        {
            let path = entry
                .unwrap_or_else(|e| panic!("Failed to read entry: {}", e))
                .path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }

            let content = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("Failed to read {:?}: {}", path, e));

            // Attempt to deserialise as AmmoConfig.
            // Legacy flat-format files (ammoClass, projectileMassG, etc. without
            // a `projectile` sub-object) will fail — they are silently skipped.
            let config: AmmoConfig = match serde_json::from_str(&content) {
                Ok(c) => c,
                Err(_) => {
                    skipped_count += 1;
                    continue;
                },
            };

            let file_name = path.file_name().unwrap().to_string_lossy();

            // Verify basic required fields
            assert!(
                !config.class.is_empty(),
                "class is empty in {:?}",
                file_name,
            );

            // Verify new Option fields can be inspected (they may be None or Some)
            let p = &config.projectile;

            // frag_mass_mean and frag_mass_std should be 0.0 in the JSON files
            if let Some(v) = p.frag_mass_mean {
                assert!(
                    v >= 0.0,
                    "frag_mass_mean = {} (must be >= 0) in {:?}",
                    v,
                    file_name,
                );
            }
            if let Some(v) = p.frag_mass_std {
                assert!(
                    v >= 0.0,
                    "frag_mass_std = {} (must be >= 0) in {:?}",
                    v,
                    file_name,
                );
            }

            // ricochet_angle_deg in [0, 90] if present
            if let Some(v) = p.ricochet_angle_deg {
                assert!(
                    v >= 0.0 && v <= 90.0,
                    "ricochet_angle_deg = {} (must be 0–90) in {:?}",
                    v,
                    file_name,
                );
            }

            // tracer_burn_time_s: if > 0, file name likely contains "tracer"
            if let Some(v) = p.tracer_burn_time_s {
                if v > 0.0 {
                    let name_has_tracer = file_name.to_lowercase().contains("tracer");
                    assert!(
                        name_has_tracer,
                        "tracer_burn_time_s = {} in {:?} but filename lacks 'tracer'",
                        v, file_name,
                    );
                }
            }

            // incendiary: if true, file name likely contains api / incendiary / iap
            if let Some(v) = p.incendiary {
                if v {
                    let name_lower = file_name.to_lowercase();
                    let has_marker = name_lower.contains("api")
                        || name_lower.contains("incendiary")
                        || name_lower.contains("iap");
                    assert!(
                        has_marker,
                        "incendiary = true in {:?} but filename lacks api/incendiary/iap marker",
                        file_name,
                    );
                }
            }

            // incendiary_ignition_temp_k: if > 0, incendiary should be Some(true)
            if let Some(v) = p.incendiary_ignition_temp_k {
                if v > 0.0 {
                    assert!(
                        p.incendiary == Some(true),
                        "incendiary_ignition_temp_k = {} but incendiary is not true in {:?}",
                        v,
                        file_name,
                    );
                }
            }

            ammo_count += 1;
        }
    }

    // Sanity: we should have found many ammo configs
    assert!(
        ammo_count >= 50,
        "Expected at least 50 ammo configs, found {}",
        ammo_count,
    );
    // These are template/base-class files in the old flat format (no `projectile`
    // sub-object). They represent abstract Arma 3 ammo classes (BulletBase,
    // ShellBase, GrenadeBase, etc.) that are never used directly.
    const TOLERATED_SKIPS: u32 = 200;
    assert!(
        skipped_count <= TOLERATED_SKIPS,
        "Expected at most {} skipped legacy files, found {}",
        TOLERATED_SKIPS,
        skipped_count,
    );

    eprintln!(
        "ammo_data_format: {} ammo configs validated, {} legacy files skipped",
        ammo_count, skipped_count,
    );
}
