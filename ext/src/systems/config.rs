// ABE - Configuration Types
//
// Compile-time configuration types for ERA zones and vehicle armor.
// Weapons and ammo are served from the compile-time PHF map (ir_weapons.tsv / ir_ammo.tsv).
// Armor and material data at data/armor/ serves as the IRL reference source;
// these may be migrated to compile-time PHF maps in a future pass.

use serde::{Deserialize, Serialize};

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
