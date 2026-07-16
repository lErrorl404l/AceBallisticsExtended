// ABE - Data Configuration & Validation
//
// Loads and validates JSON data tables for weapons, ammo, armor,
// and ballistic coefficients. Schemas enforce correct data at build
// time and in CI.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

// ── Data structures ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeaponConfig {
    pub class: String,
    pub caliber_mm: f64,
    pub barrel_length_mm: f64,
    #[serde(default)]
    pub rifling_twist_mm: f64,
    pub chamber_pressure_mpa: f64,
    #[serde(default = "default_cdm")]
    pub cdm_id: String,
    pub projectile_mass_g: f64,
    #[serde(default)]
    pub muzzle_velocity_ms: f64,
    #[serde(default = "default_zero")]
    pub zero_range_m: f64,
}

fn default_cdm() -> String {
    "g7".to_string()
}
fn default_zero() -> f64 {
    100.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmmoConfig {
    pub class: String,
    pub projectile: ProjectileConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectileConfig {
    pub model: String,
    pub mass_g: f64,
    pub caliber_mm: f64,
    pub bc_g7: f64,
    #[serde(default = "default_cdm")]
    pub cdm_id: String,
    #[serde(default)]
    pub fragmentation: Option<FragmentationConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FragmentationConfig {
    pub threshold_vel_ms: f64,
    pub avg_fragments: u32,
    pub mass_distribution: String,
    pub params: HashMap<String, f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArmorConfig {
    pub vehicle: String,
    pub plates: Vec<ArmorPlate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArmorPlate {
    pub name: String,
    pub material: String,
    pub thickness_mm: f64,
    pub angle_deg: f64,
    pub backing: Option<String>,
}

// ── Loading ───────────────────────────────────────────────────────────────────

/// Load a directory of JSON weapon configs.
pub fn load_weapon_configs(path: &Path) -> Result<Vec<WeaponConfig>, String> {
    let mut configs = Vec::new();
    let dir =
        std::fs::read_dir(path).map_err(|e| format!("Failed to read weapon config dir: {}", e))?;

    for entry in dir {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let content = std::fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
        let config: WeaponConfig = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse {}: {}", path.display(), e))?;
        configs.push(config);
    }

    Ok(configs)
}

/// Load a directory of JSON ammo configs.
pub fn load_ammo_configs(path: &Path) -> Result<Vec<AmmoConfig>, String> {
    let mut configs = Vec::new();
    let dir =
        std::fs::read_dir(path).map_err(|e| format!("Failed to read ammo config dir: {}", e))?;

    for entry in dir {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let content = std::fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
        let config: AmmoConfig = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse {}: {}", path.display(), e))?;
        configs.push(config);
    }

    Ok(configs)
}

/// Load a directory of JSON armor configs.
pub fn load_armor_configs(path: &Path) -> Result<Vec<ArmorConfig>, String> {
    let mut configs = Vec::new();
    let dir =
        std::fs::read_dir(path).map_err(|e| format!("Failed to read armor config dir: {}", e))?;

    for entry in dir {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let content = std::fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
        let config: ArmorConfig = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse {}: {}", path.display(), e))?;
        configs.push(config);
    }

    Ok(configs)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weapon_config_deserialize() {
        let json = r#"
        {
            "class": "rhs_weap_m4a1",
            "caliber_mm": 5.56,
            "barrel_length_mm": 368.0,
            "rifling_twist_mm": 178.0,
            "chamber_pressure_mpa": 380.0,
            "cdm_id": "g7",
            "projectile_mass_g": 4.0,
            "muzzle_velocity_ms": 948.0,
            "zero_range_m": 100.0
        }
        "#;

        let config: WeaponConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.class, "rhs_weap_m4a1");
        assert!((config.barrel_length_mm - 368.0).abs() < 0.01);
        assert_eq!(config.cdm_id, "g7");
    }

    #[test]
    fn weapon_config_defaults() {
        let json = r#"
        {
            "class": "test_weapon",
            "caliber_mm": 7.62,
            "barrel_length_mm": 500.0,
            "chamber_pressure_mpa": 360.0,
            "projectile_mass_g": 9.5
        }
        "#;

        let config: WeaponConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.cdm_id, "g7"); // Default
        assert!((config.zero_range_m - 100.0).abs() < 0.01); // Default
    }

    #[test]
    fn ammo_config_with_fragmentation() {
        let json = r#"
        {
            "class": "rhs_mag_30Rnd_556x45_M855_Stanag",
            "projectile": {
                "model": "m855",
                "mass_g": 4.0,
                "caliber_mm": 5.56,
                "bc_g7": 0.157,
                "cdm_id": "m855_custom",
                "fragmentation": {
                    "threshold_vel_ms": 762.0,
                    "avg_fragments": 12,
                    "mass_distribution": "log_normal",
                    "params": {"mean": 0.08, "std": 0.04}
                }
            }
        }
        "#;

        let config: AmmoConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.projectile.model, "m855");
        let frag = config.projectile.fragmentation.unwrap();
        assert_eq!(frag.avg_fragments, 12);
    }

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

    #[test]
    fn invalid_json_returns_error() {
        let result = serde_json::from_str::<WeaponConfig>("not valid json");
        assert!(result.is_err());
    }

    #[test]
    fn load_nonexistent_directory() {
        let result = load_weapon_configs(Path::new("/nonexistent/path"));
        assert!(result.is_err());
    }
}
