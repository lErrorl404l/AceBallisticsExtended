// ABE - Data Configuration & Validation
//
// Loads and validates JSON data tables for weapons, ammo, armor,
// and ballistic coefficients. Schemas enforce correct data at build
// time and in CI.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

// ── Data structures ───────────────────────────────────────────────────────────

/// Armour configuration for a vehicle.
///
/// Deserialised from the JSON armour config files in `data/armor/`.
/// Contains a list of armour plates with material and thickness data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArmorConfig {
    /// Vehicle class name (e.g. `"rhs_btr80a_msv"`).
    pub vehicle: String,
    /// List of armour plates covering the vehicle.
    pub plates: Vec<ArmorPlate>,
}

/// A single armour plate with position, material, and thickness.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArmorPlate {
    /// Plate identifier (e.g. `"hull_front"`, `"hull_side"`).
    pub name: String,
    /// Material identifier (e.g. `"steel_rha"`, `"aluminum_5083"`).
    pub material: String,
    /// Plate thickness in millimetres.
    pub thickness_mm: f64,
    /// Plate angle from vertical in degrees.
    pub angle_deg: f64,
    /// Optional backing material (e.g. `"spall_liner_kevlar"`).
    pub backing: Option<String>,
}

// ── Loading ───────────────────────────────────────────────────────────────────

/// Load a directory of JSON weapon configs.
/// Collect all `.json` file paths under `path`, recursing into subdirectories.
fn collect_json_files(root: &Path) -> Result<Vec<std::path::PathBuf>, String> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let read_dir = std::fs::read_dir(&dir)
            .map_err(|e| format!("Failed to read dir {}: {}", dir.display(), e))?;
        for entry in read_dir {
            let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|s| s.to_str()) == Some("json") {
                files.push(path);
            }
        }
    }

    Ok(files)
}

/// Collect only `.json` files directly in `root` (no subdirectory recursion).
fn collect_json_files_top_level(root: &Path) -> Result<Vec<std::path::PathBuf>, String> {
    let mut files = Vec::new();
    let read_dir = std::fs::read_dir(root)
        .map_err(|e| format!("Failed to read dir {}: {}", root.display(), e))?;
    for entry in read_dir {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("json") {
            files.push(path);
        }
    }
    Ok(files)
}

/// Load JSON armor configs from `root` only (no subdirectory recursion).
pub fn load_armor_configs(path: &Path) -> Result<Vec<ArmorConfig>, String> {
    let mut configs = Vec::new();
    for path in collect_json_files_top_level(path)? {
        let content = std::fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
        let config: ArmorConfig = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse {}: {}", path.display(), e))?;
        configs.push(config);
    }
    Ok(configs)
}

// ── Material configs ──────────────────────────────────────────────────────────

/// Configuration for a material type.
///
/// Deserialised from the JSON material config files in `data/armor/materials/`
/// and `data/materials/`. Provides physics properties for armour materials
/// used in penetration calculations.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialConfig {
    pub material_id: String,
    pub display_name: String,
    pub density_gcm3: f64,
    pub hardness_bhn: f64,
    pub tensile_strength_mpa: f64,
    pub rha_equivalent: f64,
    pub ductility: f64,
    pub spall_coeff: f64,
    pub notes: Option<String>,
}

/// Load material configs from `data/armor/materials/` and `data/materials/`.
pub fn load_material_configs(data_dir: &Path) -> Result<HashMap<String, MaterialConfig>, String> {
    let mut map = HashMap::new();

    let armor_mat_path = data_dir.join("armor").join("materials");
    if armor_mat_path.exists() {
        for path in collect_json_files(&armor_mat_path)? {
            let content = std::fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
            let config: MaterialConfig = serde_json::from_str(&content)
                .map_err(|e| format!("Failed to parse {}: {}", path.display(), e))?;
            map.insert(config.material_id.clone(), config);
        }
    }

    let struct_mat_path = data_dir.join("materials");
    if struct_mat_path.exists() {
        for path in collect_json_files(&struct_mat_path)? {
            let content = std::fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
            let config: MaterialConfig = serde_json::from_str(&content)
                .map_err(|e| format!("Failed to parse {}: {}", path.display(), e))?;
            map.insert(config.material_id.clone(), config);
        }
    }

    Ok(map)
}

// ── Global data registry ───────────────────────────────────────────────────────

/// Registry of loaded configuration data.
pub struct DataRegistry {
    pub armor: Vec<ArmorConfig>,
    pub materials: HashMap<String, MaterialConfig>,
}

static DATA_REGISTRY: OnceLock<DataRegistry> = OnceLock::new();

/// Get a reference to the global data registry.
pub fn get_data_registry() -> Option<&'static DataRegistry> {
    DATA_REGISTRY.get()
}

/// Initialize the global data registry.
pub fn initialize_data(data_dir: &Path) -> Result<(), String> {
    if DATA_REGISTRY.get().is_some() {
        return Ok(());
    }
    // Weapons and ammo are served from the compile-time PHF map (ir_weapons.tsv / ir_ammo.tsv).
    // Armor/material configs are loaded from JSON at runtime.
    let armor = load_armor_configs(&data_dir.join("armor/plates"))?;
    let materials = load_material_configs(data_dir)?;
    DATA_REGISTRY
        .set(DataRegistry { armor, materials })
        .map_err(|_| "Data already initialized".to_string())
}

/// Configuration for a single ERA zone on a vehicle.
///
/// Each zone covers an angular arc around the vehicle's azimuth,
/// with a specific ERA tile material and thickness.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EraZoneConfig {
    /// Human-readable zone name (e.g. `"front_arc"`, `"side_left"`).
    pub zone_name: String,
    /// Start angle of coverage in degrees (0 = vehicle front).
    pub coverage_angle_start_deg: f64,
    /// End angle of coverage in degrees.
    pub coverage_angle_end_deg: f64,
    /// ERA material identifier (e.g. `"k1"`, `"k5"`).
    pub era_material: String,
    /// ERA tile thickness in millimetres.
    pub era_thickness_mm: f64,
    /// ERA explosive layer density in g/cm³.
    pub era_density_gcc: f64,
}

/// Per-vehicle ERA zone configuration.
///
/// Deserialised from the JSON ERA config files in `data/era/`.
/// Holds a list of named zones, each defining an angular coverage
/// arc with material and thickness parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EraConfiguration {
    /// Ordered list of ERA coverage zones.
    pub era_zones: Vec<EraZoneConfig>,
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn armor_config_deserialize() {
        let json = r#"
        {
            "vehicle": "rhs_btr80a_msv",
            "plates": [
                {
                    "name": "hull_front",
                    "material": "steel_rha",
                    "thickness_mm": 10.0,
                    "angle_deg": 45.0,
                    "backing": null
                },
                {
                    "name": "hull_side",
                    "material": "steel_rha",
                    "thickness_mm": 7.0,
                    "angle_deg": 0.0,
                    "backing": "spall_liner_kevlar"
                }
            ]
        }
        "#;

        let config: ArmorConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.vehicle, "rhs_btr80a_msv");
        assert_eq!(config.plates.len(), 2);
        assert_eq!(
            config.plates[1].backing.as_deref(),
            Some("spall_liner_kevlar")
        );
    }
}
