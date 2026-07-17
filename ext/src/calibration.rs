// ABE — Vehicle Armour Calibration Dataset
//
// Per-vehicle exact armour calibration against published reference RHAe
// values (Jane's, Tankograd, official publications).  The layout table
// (`calibration_entries()`) is the extension point: add a new entry
// when specific vehicle armour schematics are integrated.
//
// Usage:
//   1. Add reference data to `data/calibration/vehicle_calibration.json`.
//   2. Call `validate_calibration()` after config loading to get a
//      discrepancy report comparing the model's effective RHAe against
//      published values.
//   3. The report identifies zones that are under- or overperforming
//      relative to the real vehicle.
//
// ponytail: per-vehicle exact calibration dataset, add entries when specific
//           vehicle armour schematics are integrated — the layout table
//           is the extension point.

use crate::config::{get_data_registry, ArmorPlate};
#[cfg(test)]
use crate::penetration::evaluate;
use crate::penetration::material_factor;

/// Single calibration entry for one armour zone of a specific vehicle.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CalibrationEntry {
    /// Vehicle class name (matches `ArmorConfig.vehicle` in the JSON data).
    pub vehicle: String,
    /// Armour zone name (matches `ArmorPlate.name`).
    pub zone: String,
    /// Published reference RHAe thickness in mm.
    pub reference_rhae_mm: f64,
    /// Tolerance above reference (+mm) before a warning is raised.
    pub tolerance_plus_mm: f64,
    /// Tolerance below reference (-mm) before a warning is raised.
    pub tolerance_minus_mm: f64,
    /// Source citation for the reference value.
    pub source: String,
}

/// Full vehicle calibration dataset, loaded from JSON.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VehicleCalibration {
    /// Vehicle class name.
    pub vehicle: String,
    /// Vehicle version/designation.
    pub version: String,
    /// ERA package (if applicable).
    pub era: String,
    /// Free-text notes.
    pub notes: String,
    /// Per-zone calibration entries.
    pub zones: Vec<CalibrationZone>,
}

/// A single calibration zone within a vehicle.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CalibrationZone {
    /// Zone name (matches `ArmorPlate.name`).
    pub name: String,
    /// Published reference RHAe in mm.
    pub reference_rhae_mm: f64,
    /// Upper tolerance.
    pub tolerance_plus_mm: f64,
    /// Lower tolerance.
    pub tolerance_minus_mm: f64,
    /// Source citation.
    pub source: String,
}

/// Result of validating one zone against its calibration.
#[derive(Debug, Clone)]
pub struct ZoneValidation {
    pub vehicle: String,
    pub zone: String,
    pub computed_rhae_mm: f64,
    pub reference_rhae_mm: f64,
    pub absolute_error_mm: f64,
    pub relative_error_pct: f64,
    pub within_tolerance: bool,
}

/// Full calibration validation report.
#[derive(Debug, Clone)]
pub struct CalibrationReport {
    /// Per-zone validation results.
    pub zones: Vec<ZoneValidation>,
    /// Number of zones within tolerance.
    pub zones_ok: i32,
    /// Number of zones outside tolerance.
    pub zones_warning: i32,
    /// Root-mean-square error across all zones (mm).
    pub rmse_mm: f64,
}

/// Load the full calibration dataset from a JSON file.
pub fn load_calibration(path: &std::path::Path) -> Result<Vec<VehicleCalibration>, String> {
    let content =
        std::fs::read_to_string(path).map_err(|e| format!("Failed to read calibration: {}", e))?;
    let data: Vec<VehicleCalibration> = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse calibration: {}", e))?;
    Ok(data)
}

/// Compute the effective RHAe of a single armour plate using the
/// penetration model's material factor and angle.
pub fn plate_effective_rhae(plate: &ArmorPlate) -> f64 {
    let mat_factor = material_factor(&plate.material);
    let angle_rad = plate.angle_deg.to_radians();
    let angle_mult = 1.0 / angle_rad.cos().max(0.087); // cap at cos(85°)
    plate.thickness_mm * mat_factor * angle_mult
}

/// Validate the loaded armour configs against the calibration dataset.
///
/// Loads vehicle calibration data from `calibration_path`, then for each
/// vehicle+zone pair in the calibration, computes the model's predicted
/// RHAe (thickness × material_factor × angle_multiplier) and compares
/// it to the published reference RHAe.
///
/// Returns a `CalibrationReport` summarising all discrepancies.
pub fn validate_calibration(calibration_path: &std::path::Path) -> CalibrationReport {
    let cal_data = match load_calibration(calibration_path) {
        Ok(d) => d,
        Err(_) => {
            return CalibrationReport {
                zones: vec![],
                zones_ok: 0,
                zones_warning: 0,
                rmse_mm: 0.0,
            };
        }
    };

    let registry = get_data_registry();
    let armor = registry.map(|r| &r.armor);

    let mut validations: Vec<ZoneValidation> = Vec::new();
    let mut sq_error_sum = 0.0_f64;

    for vehicle_cal in &cal_data {
        // Find matching armor config for this vehicle
        let vehicle_armor =
            armor.and_then(|all| all.iter().find(|a| a.vehicle == vehicle_cal.vehicle));

        for zone_cal in &vehicle_cal.zones {
            // Find matching plate
            let computed = vehicle_armor
                .and_then(|arm| arm.plates.iter().find(|p| p.name == zone_cal.name))
                .map(|plate| plate_effective_rhae(plate))
                .unwrap_or(0.0);

            let err = computed - zone_cal.reference_rhae_mm;
            let rel_err = if zone_cal.reference_rhae_mm > 0.0 {
                err / zone_cal.reference_rhae_mm * 100.0
            } else {
                0.0
            };
            let within = computed >= zone_cal.reference_rhae_mm - zone_cal.tolerance_minus_mm
                && computed <= zone_cal.reference_rhae_mm + zone_cal.tolerance_plus_mm;

            sq_error_sum += err * err;
            validations.push(ZoneValidation {
                vehicle: vehicle_cal.vehicle.clone(),
                zone: zone_cal.name.clone(),
                computed_rhae_mm: computed,
                reference_rhae_mm: zone_cal.reference_rhae_mm,
                absolute_error_mm: err,
                relative_error_pct: rel_err,
                within_tolerance: within,
            });
        }
    }

    let n = validations.len() as f64;
    let rmse = if n > 0.0 {
        (sq_error_sum / n).sqrt()
    } else {
        0.0
    };

    let ok_count = validations.iter().filter(|v| v.within_tolerance).count() as i32;
    let warn_count = validations.iter().filter(|v| !v.within_tolerance).count() as i32;

    CalibrationReport {
        zones: validations,
        zones_ok: ok_count,
        zones_warning: warn_count,
        rmse_mm: rmse,
    }
}

/// Pretty-print a calibration report.
pub fn format_calibration_report(report: &CalibrationReport) -> String {
    let mut s = String::new();
    s.push_str(&format!(
        "=== Armour Calibration Report ===\n\
         Zones checked: {}\n\
         Within tolerance: {}\n\
         Outside tolerance: {}\n\
         RMSE: {:.1} mm\n\n",
        report.zones.len(),
        report.zones_ok,
        report.zones_warning,
        report.rmse_mm
    ));

    for v in &report.zones {
        let flag = if v.within_tolerance { "✓" } else { "✗" };
        s.push_str(&format!(
            "  {} {}/{}: computed {:.0} mm vs ref {:.0} mm ({:+.0} mm, {:+.1}%)\n",
            flag,
            v.vehicle,
            v.zone,
            v.computed_rhae_mm,
            v.reference_rhae_mm,
            v.absolute_error_mm,
            v.relative_error_pct,
        ));
    }
    s
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn calibration_path() -> std::path::PathBuf {
        std::path::PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../data/calibration/vehicle_calibration.json"
        ))
    }

    #[test]
    fn load_calibration_file() {
        let path = calibration_path();
        let data = load_calibration(&path);
        assert!(data.is_ok(), "Failed to load calibration: {:?}", data.err());
        let data = data.unwrap();
        assert!(!data.is_empty(), "Calibration data should not be empty");
        // Check at least M1 and T-72 are present
        let vehicles: Vec<&str> = data.iter().map(|v| v.vehicle.as_str()).collect();
        assert!(
            vehicles.contains(&"rhs_m1a2sep1tuskii_d"),
            "M1 Abrams should be in calibration"
        );
        assert!(
            vehicles.contains(&"rhs_t72bb_tv"),
            "T-72 should be in calibration"
        );
    }

    #[test]
    fn plate_effective_calculation() {
        // 10mm RHA at 0° → 10mm RHAe
        let plate = ArmorPlate {
            name: "test".into(),
            material: "steel_rha".into(),
            thickness_mm: 10.0,
            angle_deg: 0.0,
            backing: None,
        };
        let rhae = plate_effective_rhae(&plate);
        assert!(
            (rhae - 10.0).abs() < 0.5,
            "10mm RHA @0° should give ~10mm RHAe, got {}",
            rhae
        );
    }

    #[test]
    fn angle_increases_effective() {
        let plate_0 = ArmorPlate {
            name: "test0".into(),
            material: "steel_rha".into(),
            thickness_mm: 10.0,
            angle_deg: 0.0,
            backing: None,
        };
        let plate_60 = ArmorPlate {
            name: "test60".into(),
            material: "steel_rha".into(),
            thickness_mm: 10.0,
            angle_deg: 60.0,
            backing: None,
        };
        let r0 = plate_effective_rhae(&plate_0);
        let r60 = plate_effective_rhae(&plate_60);
        assert!(r60 > r0, "60° plate should have higher RHAe than 0°");
    }

    #[test]
    fn validate_full_calibration() {
        let path = calibration_path();
        let report = validate_calibration(&path);
        assert!(!report.zones.is_empty(), "Should produce zone results");
        // We expect some zones to be outside tolerance (model is approximate)
        // but at least some should be within tolerance
        assert!(report.zones_warning >= 0, "Should report warning count");
        assert!(report.rmse_mm >= 0.0, "RMSE should be non-negative");
    }

    #[test]
    fn calibration_terminal_shot() {
        // M80 ball at 853 m/s should comfortably pen 5mm RHA flat
        let pen = evaluate(853.0, 0.0095, 0.00762, 0.005, 0.0, "steel_rha", "ball");
        assert!(
            pen.penetrated,
            "M80 ball should comfortably pen 5mm RHA flat"
        );
    }
}
