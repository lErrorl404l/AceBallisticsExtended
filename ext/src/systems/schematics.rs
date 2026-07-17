// ABE - Vehicle Armor Schematics
//
// Multi-layer armor array definitions for combat vehicles.
// Loaded from data/schematics/vehicle_schematics.json.
//
// Each vehicle zone (turret_front, hull_front, etc.) is modelled as
// a multi-layer array with face plate, air gaps, composite inserts,
// backing plate, and spall liner — enabling detailed evaluation
// through armor_array::evaluate_armor_array().

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::armor_array::{ArmorArrayResult, ArrayPlate};

// ── Public API types ───────────────────────────────────────────────────────────

/// A single layer in a multi-layer armor stack.
#[derive(Debug, Clone)]
pub struct SchematicLayer {
    /// Material identifier (matches material_factor keys).
    pub material: &'static str,
    /// Plate thickness in millimetres.
    pub thickness_mm: f64,
    /// Plate angle from vertical in degrees.
    pub angle_deg: f64,
    /// Gap to the next layer in millimetres (0.0 = last layer or no gap).
    pub gap_to_next_mm: f64,
}

impl SchematicLayer {
    /// Convert to an ArrayPlate (metres, no open area).
    pub fn to_array_plate(&self) -> ArrayPlate {
        ArrayPlate {
            thickness_m: self.thickness_mm / 1000.0,
            material: self.material,
            angle_from_vertical_deg: self.angle_deg,
            gap_to_next_m: self.gap_to_next_mm / 1000.0,
            open_area_fraction: 0.0,
        }
    }
}

/// An armor zone (e.g. turret_front, hull_side) with multi-layer array.
#[derive(Debug, Clone)]
pub struct SchematicZone {
    /// Estimated total RHA-equivalent thickness in mm (reference value).
    pub total_rhae_mm: f64,
    /// Optional notes about the zone composition.
    pub notes: Option<&'static str>,
    /// Ordered layers from front (outer) to back (inner).
    pub layers: Vec<SchematicLayer>,
}

/// Full armor schematic for one vehicle.
#[derive(Debug, Clone)]
pub struct VehicleSchematic {
    /// Short identifier (e.g. "m1a2_abrams_sepv2").
    pub vehicle_id: &'static str,
    /// Human-readable name.
    pub display_name: &'static str,
    /// Nation of origin.
    pub nation: &'static str,
    /// Vehicle type (mbt, ifv, apc).
    pub vehicle_type: &'static str,
    /// Armor zones keyed by zone name ("turret_front", "hull_side", etc.).
    pub zones: HashMap<&'static str, SchematicZone>,
}

impl VehicleSchematic {
    /// Resolve a zone by name. Returns `None` if the zone does not exist.
    pub fn zone(&self, name: &str) -> Option<&SchematicZone> {
        self.zones.get(name)
    }

    /// Evaluate a named zone against a projectile.
    ///
    /// Converts the zone's layer array to `ArrayPlate` plates and evaluates
    /// them through `armor_array::evaluate_armor_array()`.
    pub fn evaluate_zone(
        &self,
        zone_name: &str,
        velocity_ms: f64,
        mass_kg: f64,
        caliber_m: f64,
        projectile_type: &str,
    ) -> Option<ArmorArrayResult> {
        let zone = self.zones.get(zone_name)?;
        let plates: Vec<ArrayPlate> = zone.layers.iter().map(|l| l.to_array_plate()).collect();
        Some(crate::armor_array::evaluate_armor_array(
            &plates,
            velocity_ms,
            mass_kg,
            caliber_m,
            projectile_type,
        ))
    }

    /// Compute the total geometric RHA-equivalent thickness for a zone
    /// independent of projectile penetration (sums all layers).
    /// Returns `None` if the zone does not exist.
    pub fn zone_geometric_rhae_mm(&self, zone_name: &str) -> Option<f64> {
        let zone = self.zones.get(zone_name)?;
        let total_m: f64 = zone
            .layers
            .iter()
            .map(|l| {
                let angle_rad = l.angle_deg.to_radians();
                let cos_factor = angle_rad.cos().max(0.087);
                let angle_mult = 1.0 / cos_factor;
                (l.thickness_mm / 1000.0)
                    * crate::penetration::material_factor(l.material)
                    * angle_mult
            })
            .sum();
        Some(total_m * 1000.0)
    }

    /// Check whether the schematic has all required zones.
    pub fn has_required_zones(&self) -> bool {
        const REQUIRED: &[&str] = &[
            "turret_front",
            "turret_side",
            "hull_front",
            "hull_side",
            "turret_top",
        ];
        REQUIRED.iter().all(|z| self.zones.contains_key(*z))
    }
}

// ── Deserialization intermediates ──────────────────────────────────────────────

#[derive(Deserialize)]
struct SchematicDatabaseRaw {
    #[allow(dead_code)]
    metadata: Option<serde_json::Value>,
    vehicles: Vec<VehicleRaw>,
}

#[derive(Deserialize)]
struct VehicleRaw {
    vehicle_id: String,
    display_name: String,
    nation: String,
    #[serde(rename = "type")]
    vehicle_type: String,
    zones: HashMap<String, ZoneRaw>,
}

#[derive(Deserialize)]
struct ZoneRaw {
    total_rhae_mm: f64,
    #[serde(default)]
    notes: Option<String>,
    layers: Vec<LayerRaw>,
}

#[derive(Deserialize)]
struct LayerRaw {
    material: String,
    thickness_mm: f64,
    angle_deg: f64,
    #[serde(default)]
    gap_to_next_mm: Option<f64>,
}

// ── Helpers ────────────────────────────────────────────────────────────────────

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

fn convert(raw: SchematicDatabaseRaw) -> Vec<VehicleSchematic> {
    raw.vehicles
        .into_iter()
        .map(|v| {
            let zones: HashMap<&'static str, SchematicZone> = v
                .zones
                .into_iter()
                .map(|(key, z)| {
                    let layers: Vec<SchematicLayer> = z
                        .layers
                        .into_iter()
                        .map(|l| SchematicLayer {
                            material: leak(l.material),
                            thickness_mm: l.thickness_mm,
                            angle_deg: l.angle_deg,
                            gap_to_next_mm: l.gap_to_next_mm.unwrap_or(0.0),
                        })
                        .collect();
                    let zone = SchematicZone {
                        total_rhae_mm: z.total_rhae_mm,
                        notes: z.notes.map(leak),
                        layers,
                    };
                    (leak(key), zone)
                })
                .collect();

            VehicleSchematic {
                vehicle_id: leak(v.vehicle_id),
                display_name: leak(v.display_name),
                nation: leak(v.nation),
                vehicle_type: leak(v.vehicle_type),
                zones,
            }
        })
        .collect()
}

// ── Static database ────────────────────────────────────────────────────────────

static DB_PATH: OnceLock<String> = OnceLock::new();
static SCHEMATIC_DB: OnceLock<Option<Vec<VehicleSchematic>>> = OnceLock::new();

/// Set the path to the schematics JSON file (called once during init).
/// Defaults to `"data/schematics/vehicle_schematics.json"` relative to CWD
/// if not called explicitly.
pub fn set_schematics_path(path: &str) {
    let _ = DB_PATH.set(path.to_string());
}

fn db_path() -> &'static str {
    DB_PATH
        .get()
        .map(|s| s.as_str())
        .unwrap_or("../data/schematics/vehicle_schematics.json")
}

fn load_database() -> Option<&'static Vec<VehicleSchematic>> {
    SCHEMATIC_DB
        .get_or_init(|| {
            let path = db_path();
            match std::fs::read_to_string(path) {
                Ok(content) => match serde_json::from_str::<SchematicDatabaseRaw>(&content) {
                    Ok(raw) => {
                        eprintln!(
                            "[schematics] Loaded {} vehicles from {}",
                            raw.vehicles.len(),
                            path
                        );
                        Some(convert(raw))
                    }
                    Err(e) => {
                        eprintln!("[schematics] Warning: failed to parse {}: {}", path, e);
                        None
                    }
                },
                Err(e) => {
                    eprintln!("[schematics] Warning: failed to read {}: {}", path, e);
                    None
                }
            }
        })
        .as_ref()
}

// ── Public API ─────────────────────────────────────────────────────────────────

/// Find a vehicle schematic by its ID (e.g. "m1a2_abrams_sepv2").
///
/// Returns `None` if the database cannot be loaded or the vehicle is
/// not found.
pub fn load_schematic(vehicle_id: &str) -> Option<VehicleSchematic> {
    let db = load_database()?;
    db.iter().find(|v| v.vehicle_id == vehicle_id).cloned()
}

/// Convenience: evaluate a named zone on a vehicle in one call.
///
/// Returns `None` if the vehicle or zone is not found.
pub fn evaluate_vehicle_zone(
    vehicle_id: &str,
    zone_name: &str,
    velocity_ms: f64,
    mass_kg: f64,
    caliber_m: f64,
    projectile_type: &str,
) -> Option<ArmorArrayResult> {
    let schematic = load_schematic(vehicle_id)?;
    schematic.evaluate_zone(zone_name, velocity_ms, mass_kg, caliber_m, projectile_type)
}

/// List all available vehicle schematic IDs.
pub fn all_available_schematics() -> Vec<&'static str> {
    match load_database() {
        Some(db) => db.iter().map(|v| v.vehicle_id).collect(),
        None => vec![],
    }
}

/// Return the display name for a vehicle schematic.
pub fn schematic_display_name(vehicle_id: &str) -> Option<&'static str> {
    let db = load_database()?;
    db.iter()
        .find(|v| v.vehicle_id == vehicle_id)
        .map(|v| v.display_name)
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() {
        // The default path is "../data/schematics/vehicle_schematics.json"
        // relative to the ext/ crate root. Force load.
        let _ = load_database();
    }

    fn db() -> &'static Vec<VehicleSchematic> {
        load_database().expect("Database should be loaded in setup")
    }

    #[test]
    fn all_vehicles_load_correctly() {
        setup();
        let ids = all_available_schematics();
        // We should have at least 8 vehicles
        assert!(ids.len() >= 8, "Expected >= 8 vehicles, got {}", ids.len());
        println!("Loaded {} vehicles:", ids.len());
        for id in &ids {
            let name = schematic_display_name(id).unwrap_or("?");
            println!("  - {} ({})", id, name);
        }
    }

    #[test]
    fn each_vehicle_has_required_zones() {
        setup();
        for v in db() {
            assert!(
                v.has_required_zones(),
                "Vehicle {} is missing one or more required zones (turret_front, turret_side, hull_front, hull_side, turret_top)",
                v.vehicle_id,
            );
        }
    }

    #[test]
    fn every_zone_has_at_least_one_layer() {
        setup();
        for v in db() {
            for (zone_name, zone) in &v.zones {
                assert!(
                    !zone.layers.is_empty(),
                    "Vehicle {} zone {} has no layers",
                    v.vehicle_id,
                    zone_name,
                );
            }
        }
    }

    #[test]
    fn m1a2_turret_front_rhae_exceeds_t72b3() {
        setup();
        let m1 = load_schematic("m1a2_abrams_sepv2").expect("M1A2 not found");
        let t72 = load_schematic("t_72b3").expect("T-72B3 not found");

        // Use geometric RHAe (sum of all layers, projectile-independent)
        let m1_rhae = m1.zone_geometric_rhae_mm("turret_front").unwrap();
        let t72_rhae = t72.zone_geometric_rhae_mm("turret_front").unwrap();

        // M1A2 turret front should have substantially higher total RHAe
        assert!(
            m1_rhae > t72_rhae * 1.2,
            "M1A2 TF geometric RHAe ({:.0}mm) should exceed T-72B3 TF RHAe ({:.0}mm) by >20%",
            m1_rhae,
            t72_rhae,
        );

        println!("M1A2 TF geometric RHAe: {:.1} mm", m1_rhae);
        println!("T-72B3 TF geometric RHAe: {:.1} mm", t72_rhae);
    }

    #[test]
    fn rifle_round_does_not_penetrate_m1a2_turret_front() {
        setup();
        let m1 = load_schematic("m1a2_abrams_sepv2").expect("M1A2 not found");
        // 7.62x51mm M80 ball at 853 m/s
        let result = m1
            .evaluate_zone("turret_front", 853.0, 0.0095, 0.00762, "ball")
            .expect("M1A2 turret_front zone");
        assert!(
            !result.array_perforated,
            "7.62mm ball should NOT penetrate M1A2 turret front"
        );
    }

    #[test]
    fn rifle_ap_penetrates_btr82a_hull_side() {
        setup();
        let btr = load_schematic("btr_82a").expect("BTR-82A not found");
        // 7.62x51mm AP at 930 m/s - AP modifier ensures reliable penetration of 7mm RHA
        let result = btr
            .evaluate_zone("hull_side", 930.0, 0.0095, 0.00762, "ap")
            .expect("BTR-82A hull_side zone");
        assert!(
            result.array_perforated,
            "7.62mm AP should penetrate BTR-82A hull side (7mm RHA)"
        );
    }

    #[test]
    fn light_ap_penetrates_bmp3_hull_side() {
        setup();
        let bmp = load_schematic("bmp_3").expect("BMP-3 not found");
        // 7.62x51mm AP at 900 m/s
        let result = bmp
            .evaluate_zone("hull_side", 900.0, 0.0095, 0.00762, "ap")
            .expect("BMP-3 hull_side zone");
        assert!(
            result.array_perforated,
            "7.62mm AP should penetrate BMP-3 hull side"
        );
    }

    #[test]
    fn heavy_apfsds_may_not_penetrate_t72b3_tf() {
        setup();
        let t72 = load_schematic("t_72b3").expect("T-72B3 not found");
        // The geometric RHAe of T-72B3 turret front should be > 200mm
        let rhae = t72.zone_geometric_rhae_mm("turret_front").unwrap();
        assert!(rhae > 200.0, "T-72B3 TF geometric RHAe should exceed 200mm");
        println!("T-72B3 TF geometric RHAe: {:.1} mm", rhae);

        // A heavy APFSDS will penetrate some layers - count how many
        let result = t72
            .evaluate_zone("turret_front", 1670.0, 8.5, 0.027, "apfsds")
            .expect("T-72B3 turret_front zone");
        println!(
            "T-72B3 TF vs DM53: perforated={}, plates={}/{}",
            result.array_perforated,
            result.plates_perforated,
            t72.zones.get("turret_front").map_or(0, |z| z.layers.len()),
        );

        // At minimum, the first plate should be perforated by a tank round
        assert!(result.plates_perforated >= 1);
    }

    #[test]
    fn known_vehicle_count() {
        setup();
        let ids = all_available_schematics();
        assert!(ids.contains(&"m1a2_abrams_sepv2"), "Should contain M1A2");
        assert!(ids.contains(&"leopard_2a6"), "Should contain Leopard 2A6");
        assert!(ids.contains(&"challenger_2"), "Should contain Challenger 2");
        assert!(ids.contains(&"t_90a"), "Should contain T-90A");
        assert!(ids.contains(&"t_72b3"), "Should contain T-72B3");
        assert!(ids.contains(&"t_80bvm"), "Should contain T-80BVM");
        assert!(ids.contains(&"m2a3_bradley"), "Should contain M2A3 Bradley");
        assert!(ids.contains(&"bmp_3"), "Should contain BMP-3");
        assert!(ids.contains(&"btr_82a"), "Should contain BTR-82A");
        assert!(ids.contains(&"stryker_m1126"), "Should contain Stryker");
        assert!(ids.contains(&"t90m"), "Should contain T-90M");
        assert!(
            ids.contains(&"leopard_2a7plus"),
            "Should contain Leopard 2A7+"
        );
        assert!(
            ids.contains(&"challenger_2_tes"),
            "Should contain Challenger 2 TES"
        );
        assert!(ids.contains(&"btr82a"), "Should contain BTR-82A (btr82a)");
    }

    #[test]
    fn new_vehicles_have_detailed_zones() {
        setup();
        let detailed_ids = &[
            "t90m",
            "leopard_2a7plus",
            "challenger_2_tes",
            "m2a3_bradley",
            "btr82a",
        ];
        let detailed_zones = &[
            "turret_front",
            "turret_side",
            "turret_top",
            "hull_front",
            "hull_side",
            "hull_side_upper",
            "hull_side_lower",
        ];
        for id in detailed_ids {
            let schem = load_schematic(id)
                .unwrap_or_else(|| panic!("Vehicle '{}' not found in schematic database", id));
            for zone in detailed_zones {
                assert!(
                    schem.zones.contains_key(*zone),
                    "Vehicle '{}' is missing required zone '{}'",
                    id,
                    zone,
                );
                let z = schem.zones.get(*zone).unwrap();
                assert!(
                    z.layers.len() >= 2,
                    "Vehicle '{}' zone '{}' has only {} layers (min 2)",
                    id,
                    zone,
                    z.layers.len(),
                );
                assert!(
                    z.layers.len() <= 6,
                    "Vehicle '{}' zone '{}' has {} layers (max 6)",
                    id,
                    zone,
                    z.layers.len(),
                );
            }
        }
    }

    #[test]
    fn non_existent_vehicle_returns_none() {
        setup();
        assert!(load_schematic("nonexistent_vehicle").is_none());
    }

    #[test]
    fn non_existent_zone_returns_none() {
        setup();
        let m1 = load_schematic("m1a2_abrams_sepv2").expect("M1A2 not found");
        assert!(m1
            .evaluate_zone("non_existent_zone", 853.0, 0.0095, 0.00762, "ball")
            .is_none());
    }
}
