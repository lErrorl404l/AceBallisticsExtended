// ABE - Armour Data Format Validation Test
//
// Integration test that loads every JSON file in `data/armor/` and verifies
// it can be deserialised as `ArmorConfig`.  Material-definition files
// (which don't have `vehicle` + `plates` keys) are silently skipped.
// All vehicle configs must have non-empty plate names, non-empty materials,
// and positive thickness values.

use std::path::Path;

use abe_ballistics_ext::config::ArmorConfig;

/// Path to the armour data directory, relative to the crate's manifest.
const ARMOR_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../data/armor/");

#[test]
fn all_armor_jsons_load() {
    let dir = Path::new(ARMOR_DIR);
    let mut vehicle_count = 0u32;
    let mut skipped_count = 0u32;

    for entry in std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("Failed to read armor dir {:?}: {}", dir, e))
    {
        let path = entry
            .unwrap_or_else(|e| panic!("Failed to read entry: {}", e))
            .path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }

        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("Failed to read {:?}: {}", path, e));

        // Attempt to deserialise as ArmorConfig.
        // Material-definition files (aluminum_5083.json, rha_steel.json, etc.)
        // lack the `vehicle` field and will fail — that is expected.
        let config: ArmorConfig = match serde_json::from_str(&content) {
            Ok(c) => c,
            Err(_) => {
                skipped_count += 1;
                continue;
            }
        };

        // Verify every plate has non-empty name, non-empty material, positive thickness
        let file_name = path.file_name().unwrap().to_string_lossy();
        assert!(
            !config.vehicle.is_empty(),
            "Vehicle name is empty in {:?}",
            file_name,
        );

        assert!(
            !config.plates.is_empty(),
            "Vehicle {:?} in {:?} has zero plates",
            config.vehicle,
            file_name,
        );

        for plate in &config.plates {
            assert!(
                !plate.name.is_empty(),
                "Plate has empty name in {:?} (vehicle: {:?})",
                file_name,
                config.vehicle,
            );
            assert!(
                !plate.material.is_empty(),
                "Plate {:?} has empty material in {:?} (vehicle: {:?})",
                plate.name,
                file_name,
                config.vehicle,
            );
            assert!(
                plate.thickness_mm > 0.0,
                "Plate {:?} has thickness_mm = {} (must be > 0) in {:?} (vehicle: {:?})",
                plate.name,
                plate.thickness_mm,
                file_name,
                config.vehicle,
            );
            assert!(
                plate.angle_deg >= 0.0 && plate.angle_deg <= 90.0,
                "Plate {:?} has angle_deg = {} (must be 0-90) in {:?}",
                plate.name,
                plate.angle_deg,
                file_name,
            );
        }

        vehicle_count += 1;
    }

    // Sanity: we should have found several vehicle configs and several skipped material files
    assert!(
        vehicle_count >= 20,
        "Expected at least 20 vehicle configs, found {}",
        vehicle_count,
    );
    assert!(
        skipped_count >= 10,
        "Expected at least 10 skipped material files, found {}",
        skipped_count,
    );

    eprintln!(
        "armor_data_format: {} vehicle configs validated, {} material files skipped",
        vehicle_count, skipped_count,
    );
}
