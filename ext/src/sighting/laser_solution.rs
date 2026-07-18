// ABE - Laser Rangefinder → Firing Solution Integration
//
// Integrates laser rangefinder (LRF) readings with ballistic data to
// compute a firing solution: corrected range (inclination-compensated),
// elevation, windage, time-of-flight, impact velocity, and first-round
// hit probability.
//
// # Concept
//
// A laser rangefinder measures the slant range to the target. When
// shooting uphill or downhill, the effective (horizontal) range is
// shorter than the slant range by the cosine of the inclination angle:
//
//     effective_range = measured_range × cos(angle)
//
// This module:
//   1. Corrects the LRF range for inclination (cosine rule).
//   2. Computes the required elevation and windage using either a
//      pre-computed ballistic table or a vacuum trajectory estimate.
//   3. Estimates first-round hit probability from shooter error, LRF
//      range uncertainty, and wind uncertainty.
//
// References:
//   - M855A1 5.56mm, 950 m/s MV, 63mm sight height, 100m zero:
//     at 300 m with 30° incline → effective range 300×cos(30°) ≈ 260 m
//     → aim correction ≈ 1.5 MOA

#![allow(dead_code)]

use std::f64::consts::PI;

/// Parameters measured or estimated by a laser rangefinder and
/// environmental sensors.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LRFParams {
    /// Slant range measured by the LRF (m).
    pub measured_range_m: f64,
    /// 1-sigma range error (m). Typical: ±1 m to 1000 m, ±3 m at 2000 m.
    pub range_error_m: f64,
    /// Inclination angle of the shot line (degrees positive = uphill,
    /// negative = downhill).
    pub inclination_angle_deg: f64,
    /// Azimuth of the target relative to the shooter (degrees).
    pub azimuth_deg: f64,
    /// Cross-range (horizontal) wind component perpendicular to the
    /// line of fire (m/s). Positive = wind from the right.
    pub wind_x_ms: f64,
    /// Vertical wind component (m/s). Usually small; included for
    /// completeness.
    pub wind_y_ms: f64,
    /// Ambient temperature (°C).
    pub temp_c: f64,
    /// Altitude above sea level (m).
    pub altitude_m: f64,
    /// Atmospheric pressure (kPa).
    pub pressure_kpa: f64,
}

/// The computed firing solution for a single shot.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FiringSolution {
    /// Inclination-corrected (horizontal) range to target (m).
    pub corrected_range_m: f64,
    /// Required elevation adjustment in MOA (positive = dial up).
    pub elevation_moa: f64,
    /// Required elevation adjustment in milliradians.
    pub elevation_mrad: f64,
    /// Required windage adjustment in MOA (positive = dial right).
    pub windage_moa: f64,
    /// Required windage adjustment in milliradians.
    pub windage_mrad: f64,
    /// Estimated projectile time of flight (s).
    pub time_of_flight_s: f64,
    /// Estimated projectile velocity at impact (m/s).
    pub impact_velocity_ms: f64,
    /// Probability of a first-round hit given all uncertainty sources,
    /// in [0, 1].
    pub first_round_hit_probability: f64,
}

/// Ballistic data for a specific ammunition type.
#[derive(Debug, Clone, Copy)]
pub struct AmmoBallisticData {
    /// Muzzle velocity (m/s).
    pub mv_ms: f64,
    /// G7 ballistic coefficient.
    pub bc_g7: f64,
    /// Projectile mass (g).
    pub mass_g: f64,
    /// Projectile calibre (mm).
    pub caliber_mm: f64,
    /// Drag model identifier (e.g. "g7", "g1").
    pub cdm_id: &'static str,
    /// Height of the sight axis above the bore axis (mm).
    pub sight_height_mm: f64,
}

// ── Constants ──────────────────────────────────────────────────────────────────

/// Gravitational acceleration (m/s²).
const GRAVITY: f64 = 9.806_65;

/// Radians per MOA.
const RAD_PER_MOA: f64 = PI / (180.0 * 60.0);

/// MOA per radian.
const MOA_PER_RAD: f64 = 180.0 * 60.0 / PI;

/// Radians per milliradian.
const RAD_PER_MIL: f64 = 0.001;

/// Milliradians per radian.
const MIL_PER_RAD: f64 = 1000.0;

// ── Helpers ────────────────────────────────────────────────────────────────────

fn rad_to_moa(rad: f64) -> f64 {
    rad * MOA_PER_RAD
}

fn rad_to_mil(rad: f64) -> f64 {
    rad * MIL_PER_RAD
}

/// Linear interpolation into a ballistic table.
///
/// Table format: `(range_m, drop_m, windage_m, tof_s)` sorted by range.
/// Extrapolates flat from the nearest endpoint for out-of-range values.
fn interpolate_ballistic_table(
    table: &[(f64, f64, f64, f64)],
    range_m: f64,
) -> (f64, f64, f64, f64) {
    if table.is_empty() {
        return (0.0, 0.0, 0.0, 0.0);
    }
    if range_m <= table[0].0 {
        return (table[0].1, table[0].2, table[0].3, 0.0);
    }
    if range_m >= table[table.len() - 1].0 {
        let last = table[table.len() - 1];
        return (last.1, last.2, last.3, 0.0);
    }

    for i in 1..table.len() {
        if table[i].0 >= range_m {
            let (r0, d0, w0, t0) = table[i - 1];
            let (r1, d1, w1, t1) = table[i];
            let frac = (range_m - r0) / (r1 - r0);
            let drop = d0 + frac * (d1 - d0);
            let windage = w0 + frac * (w1 - w0);
            let tof = t0 + frac * (t1 - t0);
            // Estimate impact velocity from adjacent entries
            let impact_v = if (t1 - t0).abs() > 1e-12 {
                (r1 - r0) / (t1 - t0)
            } else {
                0.0
            };
            return (drop, windage, tof, impact_v);
        }
    }
    (0.0, 0.0, 0.0, 0.0)
}

// ── Core API ───────────────────────────────────────────────────────────────────

/// Apply the cosine-rule inclination correction to a slant range.
///
/// For a shot at angle θ from horizontal:
///
/// ```text
/// effective_range = measured_range × cos(θ)
/// ```
///
/// At 30° incline this gives 0.866 × measured range, at 45° it gives
/// 0.707 × measured range. Level fire (0°) returns the same range.
pub fn inclined_range_correction(range_m: f64, angle_deg: f64) -> f64 {
    let cos_theta = angle_deg.to_radians().cos();
    range_m * cos_theta
}

/// Compute a full firing solution from LRF readings and ammunition data.
///
/// If a pre-computed ballistic table is provided (range → drop, windage,
/// TOF), it is interpolated for the corrected range.  Otherwise a vacuum
/// (no-drag) estimate is used, which is acceptable for short-to-medium
/// ranges but overestimates the drop at long range.
///
/// # Arguments
/// * `lrf` — LRF and environmental measurements.
/// * `ammo` — ballistic data for the loaded ammunition.
/// * `ballistic_table` — optional pre-computed table of
///   `(range, drop_m, windage_m, tof_s)` entries sorted by range.
///
/// # Returns
/// A `FiringSolution` with elevation, windage, TOF, impact velocity,
/// and first-round hit probability.
pub fn compute_firing_solution(
    lrf: &LRFParams,
    ammo: &AmmoBallisticData,
    ballistic_table: Option<&Vec<(f64, f64, f64, f64)>>,
    zero_range_m: f64,
) -> FiringSolution {
    let corrected_range =
        inclined_range_correction(lrf.measured_range_m, lrf.inclination_angle_deg).max(0.0);

    // Helper: compute drop at a given range using the ballistic table or vacuum model.
    let compute_drop_at_range = |range_m: f64| -> f64 {
        if range_m <= 0.0 {
            return 0.0;
        }
        if let Some(table) = ballistic_table {
            interpolate_ballistic_table(table, range_m).0
        } else {
            let mv = ammo.mv_ms;
            if mv > 0.0 {
                let tof = range_m / mv;
                0.5 * GRAVITY * tof * tof
            } else {
                0.0
            }
        }
    };

    // --- Ballistic solution for the target range ---
    let target_drop = compute_drop_at_range(corrected_range);

    // --- Zero-range drop: the rifle is zeroed at this distance ---
    let zero_drop = compute_drop_at_range(zero_range_m);

    // --- Ballistic solution (windage + TOF + impact V) ---
    let (windage_m, tof_s, impact_v_ms) = if let Some(table) = ballistic_table {
        let (_, w, t, v) = interpolate_ballistic_table(table, corrected_range);
        (w, t, v)
    } else {
        // Vacuum (no-drag) approximation
        let mv = ammo.mv_ms;
        if mv > 0.0 && corrected_range > 0.0 {
            let tof = corrected_range / mv;
            let windage = lrf.wind_x_ms * tof;
            (windage, tof, mv)
        } else {
            (0.0, 0.0, ammo.mv_ms)
        }
    };

    // --- Elevation (zero-range corrected) ---
    // The shooter has zeroed the rifle at `zero_range_m`, meaning the sight
    // elevation already compensates for the drop at that range.  We only
    // need to adjust for the DIFFERENCE in drop between target and zero
    // ranges.
    //
    //   θ = atan(h / R) + atan((drop_target - drop_zero) / R)
    //
    // At the zero range, elevation_adjustment ≈ 0 (the sights are already set).
    let sight_h_m = ammo.sight_height_mm / 1000.0;
    let range = corrected_range.max(1.0);

    let geo_angle = (sight_h_m / range).atan();
    let drop_adjustment = (target_drop - zero_drop) / range;
    let elevation_rad = geo_angle + drop_adjustment.atan();

    // --- Windage ---
    let windage_angle = (windage_m / range).atan();

    // --- Hit probability ---
    // Shooter MOA estimate: 1.5 MOA (typical trained shooter, prone supported)
    // Wind uncertainty from the measured wind components
    let wind_mag = (lrf.wind_x_ms.powi(2) + lrf.wind_y_ms.powi(2)).sqrt();
    let wind_uncertainty = (wind_mag * 0.3 + 0.5).max(0.3); // 30 % of wind + 0.5 m/s floor
    let hit_p = compute_hit_probability(lrf, 1.5, wind_uncertainty, 0.5);

    FiringSolution {
        corrected_range_m: corrected_range,
        elevation_moa: rad_to_moa(elevation_rad),
        elevation_mrad: rad_to_mil(elevation_rad),
        windage_moa: rad_to_moa(windage_angle),
        windage_mrad: rad_to_mil(windage_angle),
        time_of_flight_s: tof_s,
        impact_velocity_ms: impact_v_ms,
        first_round_hit_probability: hit_p,
    }
}

/// Compute the first-round hit probability given all uncertainty sources.
///
/// Combines shooter precision (MOA), LRF range error, and wind uncertainty
/// into a total circular dispersion, then uses the Rayleigh distribution
/// to estimate the probability of hitting a target of the given size.
///
/// # Arguments
/// * `lrf` — LRF parameters (range error is used for vertical uncertainty).
/// * `shooter_moa` — shooter's precision in MOA (e.g. 1.5 for a trained
///   marksman).
/// * `wind_uncertainty_ms` — 1-sigma uncertainty in cross-wind speed (m/s).
/// * `target_size_m` — characteristic target dimension (m) used as the
///   diameter of an equivalent circular target.
///
/// # Returns
/// Probability in [0, 1].
pub fn compute_hit_probability(
    lrf: &LRFParams,
    shooter_moa: f64,
    wind_uncertainty_ms: f64,
    target_size_m: f64,
) -> f64 {
    let range_m = lrf.measured_range_m.max(1.0);

    // 1. Shooter dispersion: convert MOA to standard deviation in metres
    //    σ_shooter = MOA × (π / (180 × 60)) × range
    let shooter_sigma = shooter_moa * RAD_PER_MOA * range_m;

    // 2. LRF range error converted to vertical dispersion
    //    The range error maps to a vertical offset through the trajectory
    //    angle. A rough estimate: σ_vertical ≈ range_error × (drop / range)
    //    For simplicity, use a fixed fraction.
    let range_error_vertical = lrf.range_error_m * 0.5;

    // 3. Wind uncertainty: lateral displacement uncertainty
    //    σ_wind ≈ V_wind_uncertainty × TOF
    let approx_tof = range_m / 800.0; // crude TOF estimate (800 m/s average)
    let wind_sigma = wind_uncertainty_ms * approx_tof;

    // Total circular sigma (RSS of all components)
    let total_sigma =
        (shooter_sigma.powi(2) + range_error_vertical.powi(2) + wind_sigma.powi(2)).sqrt();

    if total_sigma <= 0.0 || target_size_m <= 0.0 {
        return if target_size_m > 0.0 { 1.0 } else { 0.0 };
    }

    // Rayleigh distribution: P(hit) = 1 - exp(-R² / (2σ²))
    // where R = target_size / 2 (radius of equivalent circular target)
    let radius = target_size_m / 2.0;
    1.0 - (-(radius * radius) / (2.0 * total_sigma * total_sigma)).exp()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn default_lrf() -> LRFParams {
        LRFParams {
            measured_range_m: 300.0,
            range_error_m: 1.0,
            inclination_angle_deg: 0.0,
            azimuth_deg: 0.0,
            wind_x_ms: 0.0,
            wind_y_ms: 0.0,
            temp_c: 15.0,
            altitude_m: 0.0,
            pressure_kpa: 101.325,
        }
    }

    fn m855a1_ammo() -> AmmoBallisticData {
        AmmoBallisticData {
            mv_ms: 950.0,
            bc_g7: 0.157,
            mass_g: 4.0,
            caliber_mm: 5.56,
            cdm_id: "g7",
            sight_height_mm: 63.0,
        }
    }

    // ── Inclination correction tests ────────────────────────────────────

    #[test]
    fn level_fire_returns_same_range() {
        let corrected = inclined_range_correction(300.0, 0.0);
        assert!(
            (corrected - 300.0).abs() < 1e-10,
            "Level fire should return same range, got {}",
            corrected
        );
    }

    #[test]
    fn inclined_30_deg_reduces_range() {
        let corrected = inclined_range_correction(300.0, 30.0);
        let expected = 300.0 * (30.0_f64).to_radians().cos(); // ~259.8 m
        assert!(
            (corrected - expected).abs() < 0.1,
            "30° incline: expected {}, got {}",
            expected,
            corrected
        );
        // Verify it's approximately 260 m
        assert!(
            (corrected - 259.8).abs() < 0.1,
            "30° at 300m should give ~259.8m, got {}",
            corrected
        );
    }

    #[test]
    fn inclined_45_deg_reduces_range() {
        let corrected = inclined_range_correction(500.0, 45.0);
        let expected = 500.0 * (45.0_f64).to_radians().cos(); // ~353.6 m
        assert!(
            (corrected - expected).abs() < 0.1,
            "45° incline: expected {}, got {}",
            expected,
            corrected
        );
    }

    #[test]
    fn downhill_incline_same_as_uphill() {
        let uphill = inclined_range_correction(300.0, 30.0);
        let downhill = inclined_range_correction(300.0, -30.0);
        assert!(
            (uphill - downhill).abs() < 1e-10,
            "cosine is even, uphill={}, downhill={}",
            uphill,
            downhill
        );
    }

    // ── Firing solution tests ───────────────────────────────────────────

    #[test]
    fn known_firing_solution_m855a1() {
        // M855A1 5.56mm, 950 m/s MV, 63mm sight height, 100m zero
        // at 300m with 30° incline → effective range = 300×cos(30°) ≈ 260m
        let lrf = LRFParams {
            measured_range_m: 300.0,
            range_error_m: 1.0,
            inclination_angle_deg: 30.0,
            ..default_lrf()
        };
        let ammo = m855a1_ammo();
        let sol = compute_firing_solution(&lrf, &ammo, None, 100.0);

        // Corrected range should be ~260 m
        assert!(
            (sol.corrected_range_m - 259.8).abs() < 1.0,
            "Corrected range should be ~260m, got {}",
            sol.corrected_range_m
        );

        // Elevation should be positive (need to dial up)
        assert!(
            sol.elevation_moa > 0.0,
            "Elevation should be positive, got {} MOA",
            sol.elevation_moa
        );

        // The aim correction vs level fire (300m) should be ~1.5 MOA
        let level_sol = compute_firing_solution(&default_lrf(), &ammo, None, 100.0);
        let aim_correction = (sol.elevation_moa - level_sol.elevation_moa).abs();
        assert!(
            (aim_correction - 1.5).abs() < 1.0,
            "Incline aim correction should be ~1.5 MOA, got {} MOA",
            aim_correction
        );

        // TOF should be positive
        assert!(sol.time_of_flight_s > 0.0, "TOF should be positive");

        // Impact velocity should be positive
        assert!(
            sol.impact_velocity_ms > 0.0,
            "Impact velocity should be positive"
        );
    }

    #[test]
    fn firing_solution_level_fire() {
        let lrf = default_lrf();
        let ammo = m855a1_ammo();
        let sol = compute_firing_solution(&lrf, &ammo, None, 100.0);

        // With no wind, windage should be ~0
        assert!(
            sol.windage_moa.abs() < 0.01,
            "No wind should give ~0 windage, got {} MOA",
            sol.windage_moa
        );

        // Corrected range should equal measured range (level fire)
        assert!(
            (sol.corrected_range_m - 300.0).abs() < 1e-10,
            "Level fire corrected range should be 300m"
        );
    }

    #[test]
    fn firing_solution_with_table() {
        // Create a simple ballistic table (vacuum-based for testing)
        let table = vec![
            (0.0, 0.0, 0.0, 0.0),
            (100.0, 0.054, 0.0, 0.105),
            (200.0, 0.217, 0.0, 0.211),
            (300.0, 0.489, 0.0, 0.316),
        ];
        let lrf = default_lrf();
        let ammo = m855a1_ammo();
        let sol = compute_firing_solution(&lrf, &ammo, Some(&table), 100.0);

        assert!(
            (sol.corrected_range_m - 300.0).abs() < 1e-10,
            "Corrected range should be 300m"
        );
        assert!(
            sol.time_of_flight_s > 0.0,
            "TOF should be positive, got {}",
            sol.time_of_flight_s
        );
    }

    // ── Hit probability tests ───────────────────────────────────────────

    #[test]
    fn hit_probability_less_than_one() {
        let lrf = LRFParams {
            measured_range_m: 500.0,
            ..default_lrf()
        };
        let p = compute_hit_probability(&lrf, 2.0, 1.0, 0.5);
        assert!(
            p > 0.0 && p < 1.0,
            "Hit probability should be in (0,1) with uncertainty, got {}",
            p
        );
    }

    #[test]
    fn hit_probability_zero_error() {
        let lrf = LRFParams {
            measured_range_m: 100.0,
            range_error_m: 0.0,
            ..default_lrf()
        };
        // Zero shooter error, zero wind uncertainty → should hit guaranteed
        let p = compute_hit_probability(&lrf, 0.0, 0.0, 1.0);
        assert!(
            (p - 1.0).abs() < 1e-12,
            "Zero error at close range on large target should give P=1, got {}",
            p
        );
    }

    #[test]
    fn wind_uncertainty_reduces_hit_probability() {
        let lrf = LRFParams {
            measured_range_m: 500.0,
            ..default_lrf()
        };
        let p_low_wind = compute_hit_probability(&lrf, 1.0, 0.5, 0.5);
        let p_high_wind = compute_hit_probability(&lrf, 1.0, 5.0, 0.5);
        assert!(
            p_low_wind > p_high_wind,
            "Higher wind uncertainty should reduce P(hit): low={}, high={}",
            p_low_wind,
            p_high_wind
        );
    }

    #[test]
    fn shooter_moa_reduces_hit_probability() {
        let lrf = LRFParams {
            measured_range_m: 500.0,
            ..default_lrf()
        };
        let p_good = compute_hit_probability(&lrf, 0.5, 0.5, 0.5);
        let p_bad = compute_hit_probability(&lrf, 5.0, 0.5, 0.5);
        assert!(
            p_good > p_bad,
            "Better shooter (lower MOA) should have higher P(hit): good={}, bad={}",
            p_good,
            p_bad
        );
    }

    // ── Additional edge-case tests ──────────────────────────────────────

    #[test]
    fn hit_probability_larger_target_higher() {
        let lrf = LRFParams {
            measured_range_m: 300.0,
            ..default_lrf()
        };
        let p_small = compute_hit_probability(&lrf, 2.0, 1.0, 0.3);
        let p_large = compute_hit_probability(&lrf, 2.0, 1.0, 1.0);
        assert!(p_large > p_small, "Larger target should have higher P(hit)");
    }

    #[test]
    fn windage_nonzero_with_crosswind() {
        let lrf = LRFParams {
            measured_range_m: 300.0,
            wind_x_ms: 5.0, // 5 m/s crosswind from the right
            ..default_lrf()
        };
        let ammo = m855a1_ammo();
        let sol = compute_firing_solution(&lrf, &ammo, None, 100.0);

        // Crosswind should produce non-zero windage
        assert!(
            sol.windage_moa.abs() > 0.01,
            "Crosswind should produce windage, got {} MOA",
            sol.windage_moa
        );

        // Wind from right (positive wind_x) should need windage adjustment
        // Wind pushes bullet left → need right windage → positive
        assert!(
            sol.windage_moa > 0.0,
            "Wind from right should need positive windage, got {}",
            sol.windage_moa
        );
    }
}
