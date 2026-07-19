// ABE - Reticle Holdover / Windage Conversion
//
// Converts ballistic drop and deflection (from the ballistic solution table)
// into reticle-relative aiming points for various optical sight patterns.
//
// # Concept
//
// A ballistic solution table (solution_table.rs) computes drop and windage
// in linear units (centimetres) for a given range. This module converts
// those linear displacements into the angular units a shooter uses through
// the scope:
//
//   mrad = atan(drop_m / range_m) × 1000
//
// The result is then mapped onto the specific reticle pattern:
//   - Mil-Dot: holdover in mil spacings (1 mil = 1 mrad)
//   - Horus/TReMoR: fine grid subtensions
//   - BDC: stadia line matching
//   - Scope clicks: turret adjustment in mrad
//
// References:
//   - Mil relation: 1 mrad ≈ 10 cm at 100 m
//   - Horus Vision reticle manuals (H58, H59)
//   - TReMoR3 / TReMoR4 (Tremor3 / Tremor4) reticle specs
//   - SVD PSO-1: 0.1 mrad windage clicks, BDC to 1000 m
//   - Trijicon ACOG: BDC chevrons at 100–800 m

#![allow(dead_code)]

/// Optical reticle patterns supported for holdover/windage conversion.
#[derive(Debug, Clone, PartialEq)]
pub enum ReticlePattern {
    /// Mil-Dot: standard 1 mrad dot spacing, 0.2 mrad sub-tensions
    /// (Gen II Mil-Dot uses 0.2 mrad dots).
    MilDot,
    /// Horus H58: grid reticle with 0.1 mrad windage spacing and
    /// 0.2 mrad elevation increments.
    HorusH58,
    /// Horus H59: full 0.1 mrad grid for both windage and elevation.
    HorusH59,
    /// TReMoR3 (Tremor3): 0.1 mrad windage grid, 0.2 mrad elevation
    /// increments on the lower stadia.
    TReMoR3,
    /// TReMoR4 (Tremor4): full 0.1 mrad grid across the entire reticle,
    /// with ranging brackets.
    TReMoR4,
    /// Generic ballistic drop compensating reticle. Uses a built-in
    /// BDC table to find the stadia line matching the range.
    BDC,
    /// SVD PSO-1: Soviet-style BDC reticle calibrated for 7.62×54mmR,
    /// 0.1 mrad windage clicks, 50 m range increments to 1000 m.
    SVDPSO1,
    /// Trijicon ACOG: chevron-based BDC reticle with stadia lines at
    /// 100–800 m intervals (calibre-dependent).
    ACOG,
    /// Custom user-defined reticle parameters.
    Custom(ReticleCustom),
}

/// Parameters for a custom reticle pattern.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReticleCustom {
    /// Elevation interval between stadia lines (mrad).
    pub elevation_interval_mrad: f64,
    /// Windage interval between grid lines (mrad).
    pub windage_interval_mrad: f64,
    /// Sub-tension (fine aiming point size) in mrad.
    pub sub_tension_mrad: f64,
}

/// Computed reticle holdover and windage solution.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReticleSolution {
    /// Holdover (elevation) relative to the crosshair centre (mrad).
    /// Positive = aim above the crosshair centre.
    pub holdover_mil: f64,
    /// Windage relative to the crosshair centre (mrad).
    /// Positive = aim right.
    pub windage_mil: f64,
    /// Elevation turret clicks (positive = dial up).
    pub holdover_clicks: i32,
    /// Windage turret clicks (positive = dial right).
    pub windage_clicks: i32,
    /// Click value of the scope (mrad per click). Typical values:
    /// 0.1 mrad (standard), 0.05 mrad (precision target).
    pub click_value_mrad: f64,
    /// BDC aim point range — the closest BDC stadia line at or below the
    /// engagement range (metres). 0.0 if the reticle is not a BDC type
    /// or the range is below the first stadia.
    pub bdc_aim_point_m: f64,
}

// ── Constants ──────────────────────────────────────────────────────────────────

/// Milliradians per radian.
const MRAD_PER_RAD: f64 = 1000.0;

/// Default click value for modern scopes (0.1 mrad).
const DEFAULT_CLICK_MRAD: f64 = 0.1;

// ── Conversion helpers ─────────────────────────────────────────────────────────

/// Convert bullet drop in centimetres to milliradians at a given range.
///
/// Uses the exact trigonometric formula:
///
/// ```text
/// mrad = atan(drop_m / range_m) × 1000
/// ```
///
/// For small angles (< 300 mrad) the small-angle approximation
/// `(drop_m / range_m) × 1000` is within 1.5 % of the exact value.
///
/// # Arguments
/// * `drop_cm` — bullet drop below the line of sight (centimetres).
/// * `range_m` — range to target (metres).
///
/// # Returns
/// The equivalent angular holdover in milliradians.
pub fn drop_to_mrad(drop_cm: f64, range_m: f64) -> f64 {
    if range_m <= 0.0 {
        return 0.0;
    }
    let drop_m = drop_cm / 100.0;
    (drop_m / range_m).atan() * MRAD_PER_RAD
}

/// Convert windage deflection in centimetres to milliradians.
///
/// Same mathematical relationship as `drop_to_mrad`:
///
/// ```text
/// mrad = atan(deflection_m / range_m) × 1000
/// ```
///
/// # Arguments
/// * `deflection_cm` — lateral projectile deflection (centimetres).
/// * `range_m` — range to target (metres).
pub fn deflection_to_mrad(deflection_cm: f64, range_m: f64) -> f64 {
    drop_to_mrad(deflection_cm, range_m)
}

/// Convert milliradians to scope clicks.
///
/// Clicks are rounded to the nearest integer since most scopes have
/// detented turrets with discrete click positions.
///
/// # Arguments
/// * `angle_mrad` — angular correction in milliradians.
/// * `click_value_mrad` — angular value of one click (e.g. 0.1).
pub fn mrad_to_clicks(angle_mrad: f64, click_value_mrad: f64) -> i32 {
    if click_value_mrad <= 0.0 {
        return 0;
    }
    (angle_mrad / click_value_mrad).round() as i32
}

/// Convert a holdover in mrad to mil-dot spacings.
///
/// Returns `(full_mils, fractional_mrad)` where:
/// * `full_mils` — number of whole mil spacings (e.g. 3 mils).
/// * `fractional_mrad` — the remainder in mrad for sub-tension aiming
///   (fractions of a mil between dots).
///
/// For a standard Mil-Dot reticle, each dot is 1 mil apart and the
/// dot itself is 0.2 mrad (Gen II). The fractional part can be used
/// to aim between dots using the sub-tension marks.
pub fn mil_dot_holdover(holdover_mrad: f64) -> (f64, f64) {
    let full_mils = holdover_mrad.floor();
    let fractional_mrad = holdover_mrad - full_mils;
    (full_mils, fractional_mrad)
}

// ── BDC table matching ─────────────────────────────────────────────────────────

/// Find the best-matching BDC stadia line for a given range.
///
/// Takes a slice of `(stadia_range_m, hold_mrad)` pairs representing
/// the BDC stadia lines of the reticle. Returns the hold value in mrad
/// for the closest stadia at or below the given range.
///
/// Returns 0.0 if the range is below the first stadia or the table is
/// empty.
///
/// # Arguments
/// * `stadia_table` — slice of `(range_m, hold_mrad)` pairs, sorted
///   by increasing range.
/// * `range_m` — engagement range to match.
pub fn bdc_aim_point(stadia_table: &[(f64, f64)], range_m: f64) -> f64 {
    if stadia_table.is_empty() || range_m < stadia_table[0].0 {
        return 0.0;
    }
    let mut best = 0.0;
    for &(stadia_range, hold_mrad) in stadia_table {
        if stadia_range <= range_m {
            best = hold_mrad;
        } else {
            break;
        }
    }
    best
}

// ── Reticle-specific BDC tables ────────────────────────────────────────────────

/// Stadia table for a generic BDC reticle (100 m intervals, typical 5.56 mm).
fn generic_bdc_table() -> Vec<(f64, f64)> {
    vec![
        (100.0, 0.0),
        (200.0, 0.5),
        (300.0, 1.5),
        (400.0, 3.0),
        (500.0, 5.0),
        (600.0, 7.5),
        (700.0, 10.5),
        (800.0, 14.0),
    ]
}

/// Stadia table for the SVD PSO-1 reticle (7.62×54mmR, 50 m increments).
fn svd_pso1_table() -> Vec<(f64, f64)> {
    vec![
        (100.0, 0.0),
        (200.0, 0.5),
        (300.0, 1.5),
        (400.0, 3.0),
        (500.0, 5.0),
        (600.0, 7.5),
        (700.0, 10.5),
        (800.0, 14.0),
        (900.0, 18.0),
        (1000.0, 22.5),
    ]
}

/// Stadia table for the ACOG reticle (chevrons at 100–800 m).
fn acog_table() -> Vec<(f64, f64)> {
    vec![
        (100.0, 0.0),
        (200.0, 0.5),
        (300.0, 1.5),
        (400.0, 3.0),
        (500.0, 5.0),
        (600.0, 7.5),
        (700.0, 10.5),
        (800.0, 14.0),
    ]
}

// ── Pattern-based conversion ───────────────────────────────────────────────────

/// Determine the click value in mrad for a given reticle pattern.
fn click_value_for_pattern(pattern: &ReticlePattern) -> f64 {
    match pattern {
        ReticlePattern::SVDPSO1 => 0.1, // PSO-1 uses 0.1 mrad per click
        ReticlePattern::ACOG => 0.1,    // ACOG typically 0.1 mrad
        _ => DEFAULT_CLICK_MRAD,
    }
}

/// Look up the BDC aim point (range in metres) for a reticle pattern.
fn bdc_aim_point_for_pattern(pattern: &ReticlePattern, range_m: f64) -> f64 {
    match pattern {
        ReticlePattern::BDC => {
            let table = generic_bdc_table();
            bdc_aim_point(&table, range_m)
        },
        ReticlePattern::SVDPSO1 => {
            let table = svd_pso1_table();
            bdc_aim_point(&table, range_m)
        },
        ReticlePattern::ACOG => {
            let table = acog_table();
            bdc_aim_point(&table, range_m)
        },
        // Non-BDC reticles have no stadia-table aim point
        _ => 0.0,
    }
}

// ── Core conversion API ────────────────────────────────────────────────────────

/// Convert ballistic drop and deflection to a reticle-relative aiming
/// solution for the specified reticle pattern.
///
/// # Arguments
/// * `drop_cm` — bullet drop at the target range (centimetres). Positive
///   = bullet lands below the point of aim.
/// * `deflection_cm` — windage deflection at the target range (centimetres).
///   Positive = bullet lands to the right of the point of aim.
/// * `range_m` — range to the target (metres).
/// * `pattern` — the optical reticle pattern to use.
/// * `zero_range_m` — the zero range of the weapon (metres). Used by BDC
///   reticles to select the correct stadia line.
///
/// # Returns
/// A `ReticleSolution` with holdover/windage in milliradians, turret
/// clicks, and the BDC aim point if applicable.
pub fn convert_to_reticle(
    drop_cm: f64,
    deflection_cm: f64,
    range_m: f64,
    pattern: ReticlePattern,
    zero_range_m: f64,
) -> ReticleSolution {
    // Convert linear displacement to milliradians
    let holdover_mrad = drop_to_mrad(drop_cm, range_m);
    let windage_mrad = deflection_to_mrad(deflection_cm, range_m);

    // Scope click values
    let click_value_mrad = click_value_for_pattern(&pattern);

    // Convert to clicks
    let holdover_clicks = mrad_to_clicks(holdover_mrad, click_value_mrad);
    let windage_clicks = mrad_to_clicks(windage_mrad, click_value_mrad);

    // BDC aim point
    let bdc_aim = if range_m <= zero_range_m {
        // At or below the zero range, holdover is at the crosshair
        0.0
    } else {
        bdc_aim_point_for_pattern(&pattern, range_m)
    };

    ReticleSolution {
        holdover_mil: holdover_mrad,
        windage_mil: windage_mrad,
        holdover_clicks,
        windage_clicks,
        click_value_mrad,
        bdc_aim_point_m: bdc_aim,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── drop_to_mrad tests ─────────────────────────────────────────────

    #[test]
    fn drop_to_mrad_known_value() {
        // 30 cm drop at 300 m
        // mrad = atan(0.3 / 300) × 1000 ≈ 1.0 mrad
        let mrad = drop_to_mrad(30.0, 300.0);
        let expected = (0.3_f64 / 300.0).atan() * 1000.0;
        assert!(
            (mrad - expected).abs() < 1e-6,
            "drop_to_mrad: expected {} mrad, got {}",
            expected,
            mrad
        );
    }

    #[test]
    fn drop_to_mrad_small_angle_approximation() {
        // 10 cm drop at 500 m: atan(0.1/500) × 1000 ≈ 0.2 mrad
        let exact = drop_to_mrad(10.0, 500.0);
        let approx = (0.1 / 500.0) * 1000.0; // small-angle approx
        let diff = (exact - approx).abs();
        assert!(
            diff < 0.001,
            "Small-angle approx should be close: exact={}, approx={}, diff={}",
            exact,
            approx,
            diff
        );
    }

    #[test]
    fn drop_to_mrad_zero_range_returns_zero() {
        assert_eq!(drop_to_mrad(30.0, 0.0), 0.0);
    }

    #[test]
    fn deflection_to_mrad_matches_drop_to_mrad() {
        let d = deflection_to_mrad(15.0, 200.0);
        let expected = drop_to_mrad(15.0, 200.0);
        assert!(
            (d - expected).abs() < 1e-12,
            "deflection_to_mrad should match drop_to_mrad"
        );
    }

    // ── mrad_to_clicks tests ───────────────────────────────────────────

    #[test]
    fn mrad_to_clicks_standard_conversion() {
        // 1.5 mrad at 0.1 mrad/click = 15 clicks
        let clicks = mrad_to_clicks(1.5, 0.1);
        assert_eq!(clicks, 15, "1.5 mrad / 0.1 = 15 clicks");
    }

    #[test]
    fn mrad_to_clicks_rounds_to_nearest() {
        // 1.55 mrad at 0.1 mrad/click = 16 clicks (rounded up)
        let clicks = mrad_to_clicks(1.55, 0.1);
        assert_eq!(clicks, 16, "1.55 mrad should round to 16 clicks");
    }

    #[test]
    fn mrad_to_clicks_fine_click_value() {
        // 0.25 mrad at 0.05 mrad/click = 5 clicks
        let clicks = mrad_to_clicks(0.25, 0.05);
        assert_eq!(clicks, 5, "0.25 mrad / 0.05 = 5 clicks");
    }

    #[test]
    fn mrad_to_clicks_zero_click_value_returns_zero() {
        assert_eq!(mrad_to_clicks(1.0, 0.0), 0);
    }

    // ── mil_dot_holdover tests ─────────────────────────────────────────

    #[test]
    fn mil_dot_holdover_whole_mils() {
        let (full, frac) = mil_dot_holdover(3.0);
        assert!(
            (full - 3.0).abs() < 1e-10,
            "Should be 3 full mils, got {}",
            full
        );
        assert!(
            frac.abs() < 1e-10,
            "Fractional part should be 0 for whole mils, got {}",
            frac
        );
    }

    #[test]
    fn mil_dot_holdover_fractional() {
        let (full, frac) = mil_dot_holdover(3.7);
        assert!(
            (full - 3.0).abs() < 1e-10,
            "Should be 3 full mils, got {}",
            full
        );
        assert!(
            (frac - 0.7).abs() < 1e-10,
            "Fractional part should be 0.7, got {}",
            frac
        );
    }

    // ── bdc_aim_point tests ────────────────────────────────────────────

    #[test]
    fn bdc_aim_point_exact_match() {
        let table = vec![(100.0, 0.0), (300.0, 1.5), (500.0, 5.0)];
        let hold = bdc_aim_point(&table, 300.0);
        assert!(
            (hold - 1.5).abs() < 1e-10,
            "BDC at 300 m should give 1.5 mrad hold, got {}",
            hold
        );
    }

    #[test]
    fn bdc_aim_point_between_stadia() {
        let table = vec![(100.0, 0.0), (300.0, 1.5), (500.0, 5.0)];
        // At 400 m, the closest stadia at or below is 300 m → hold = 1.5 mrad
        let hold = bdc_aim_point(&table, 400.0);
        assert!(
            (hold - 1.5).abs() < 1e-10,
            "BDC between stadia: at 400 m should give 300 m stadia hold (1.5), got {}",
            hold
        );
    }

    #[test]
    fn bdc_aim_point_below_first_stadia() {
        let table = vec![(100.0, 0.0), (300.0, 1.5)];
        let hold = bdc_aim_point(&table, 50.0);
        assert!(
            hold.abs() < 1e-10,
            "BDC below first stadia should return 0, got {}",
            hold
        );
    }

    #[test]
    fn bdc_aim_point_empty_table() {
        let table: Vec<(f64, f64)> = vec![];
        let hold = bdc_aim_point(&table, 300.0);
        assert!(
            hold.abs() < 1e-10,
            "Empty table should return 0, got {}",
            hold
        );
    }

    // ── convert_to_reticle full conversion tests ───────────────────────

    #[test]
    fn convert_to_reticle_mil_dot_known_values() {
        // At 300 m with 30 cm drop and 10 cm windage deflection:
        //   holdover = atan(0.3/300) × 1000 ≈ 1.0 mrad → ~10 clicks
        //   windage  = atan(0.1/300) × 1000 ≈ 0.333 mrad → ~3 clicks
        let sol = convert_to_reticle(30.0, 10.0, 300.0, ReticlePattern::MilDot, 100.0);

        assert!(
            (sol.holdover_mil - 1.0).abs() < 0.01,
            "Holdover should be ~1.0 mrad, got {}",
            sol.holdover_mil
        );
        assert!(
            sol.holdover_clicks > 0,
            "Should have positive holdover clicks, got {}",
            sol.holdover_clicks
        );
        assert!(
            sol.windage_clicks > 0,
            "Should have positive windage clicks"
        );
        assert!(
            (sol.click_value_mrad - 0.1).abs() < 1e-10,
            "Default click value should be 0.1 mrad"
        );
        // Mil-Dot is not BDC
        assert!(
            sol.bdc_aim_point_m.abs() < 1e-10,
            "Mil-Dot should have no BDC aim point"
        );
    }

    #[test]
    fn convert_to_reticle_horus_h58() {
        let sol = convert_to_reticle(50.0, 15.0, 400.0, ReticlePattern::HorusH58, 100.0);
        // Should produce finite, non-negative values
        assert!(sol.holdover_mil.is_finite(), "Holdover should be finite");
        assert!(sol.windage_mil.is_finite(), "Windage should be finite");
        assert!(sol.holdover_clicks >= 0, "Holdover clicks should be >= 0");
    }

    #[test]
    fn convert_to_reticle_bdc_range_matching() {
        // BDC reticle at 350 m: should match 300 m stadia
        let sol = convert_to_reticle(45.0, 10.0, 350.0, ReticlePattern::BDC, 100.0);
        assert!(
            sol.bdc_aim_point_m.abs() > 0.0,
            "BDC should return a non-zero aim point at 350 m"
        );
    }

    #[test]
    fn convert_to_reticle_svd_pso1() {
        let sol = convert_to_reticle(70.0, 5.0, 500.0, ReticlePattern::SVDPSO1, 100.0);
        assert!(
            sol.click_value_mrad == 0.1,
            "SVD PSO-1 click value should be 0.1 mrad"
        );
        assert!(
            sol.bdc_aim_point_m > 0.0,
            "SVD PSO-1 should have BDC aim point at 500 m"
        );
    }

    #[test]
    fn convert_to_reticle_acog() {
        let sol = convert_to_reticle(30.0, 0.0, 300.0, ReticlePattern::ACOG, 100.0);
        assert!(sol.holdover_mil > 0.0, "ACOG should have positive holdover");
        assert!(sol.bdc_aim_point_m > 0.0, "ACOG should match BDC at 300 m");
    }

    #[test]
    fn convert_to_reticle_custom() {
        let custom = ReticlePattern::Custom(ReticleCustom {
            elevation_interval_mrad: 0.2,
            windage_interval_mrad: 0.1,
            sub_tension_mrad: 0.05,
        });
        let sol = convert_to_reticle(30.0, 10.0, 300.0, custom, 100.0);
        assert!(
            sol.holdover_mil > 0.0,
            "Custom reticle should have holdover"
        );
        assert!(
            sol.bdc_aim_point_m.abs() < 1e-10,
            "Custom reticle should have no BDC"
        );
    }

    // ── Edge cases ─────────────────────────────────────────────────────

    #[test]
    fn convert_to_reticle_zero_range() {
        // Zero range → should not panic, return sensible values
        let sol = convert_to_reticle(0.0, 0.0, 0.0, ReticlePattern::MilDot, 100.0);
        assert_eq!(sol.holdover_mil, 0.0, "Zero drop at zero range");
        assert_eq!(sol.windage_mil, 0.0, "Zero deflection at zero range");
        assert_eq!(sol.holdover_clicks, 0, "Zero clicks");
    }

    #[test]
    fn convert_to_reticle_very_long_range() {
        // Very long range should not overflow or produce NaN
        let sol = convert_to_reticle(500.0, 100.0, 1500.0, ReticlePattern::MilDot, 100.0);
        assert!(
            sol.holdover_mil.is_finite(),
            "Holdover should be finite at long range"
        );
        assert!(
            sol.holdover_mil > 0.0,
            "Holdover should be positive at 1500 m"
        );
    }

    #[test]
    fn convert_to_reticle_bdc_below_zero_range() {
        // At or below zero range, BDC aim point should be 0
        let sol = convert_to_reticle(5.0, 0.0, 100.0, ReticlePattern::BDC, 100.0);
        assert!(
            sol.bdc_aim_point_m.abs() < 1e-10,
            "BDC at zero range should be 0"
        );
    }
}
