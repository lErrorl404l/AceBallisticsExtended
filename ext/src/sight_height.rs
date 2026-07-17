// ABE - Sight Height / Zeroing Model
//
// Computes the bore-to-sight angular correction (sight line angle / zero
// angle) given the height of the sight axis above the bore axis and the
// desired zero range.
//
// # Concept
//
// When a rifle is zeroed, the bore axis is angled upward relative to the
// sight line so that the projectile's parabolic trajectory intersects the
// sight line at the zero range.  The zero angle decomposes into two parts:
//
//   1. Geometric offset — the angle between the bore axis and the line
//      from the sight to the target: `atan(sight_height / zero_range)`.
//
//   2. Drop compensation — the additional elevation to offset the
//      projectile's gravitational arc: `asin(g × zero_range / (2 × MV²))`.
//
// The model uses a vacuum trajectory (no drag) for the zeroing solution.
// This is the same approach used by most ballistics solvers for the
// first‑order zero‑angle estimate; drag corrections are applied separately
// in the exterior ballistics integrator.
//
// # Units
// All angles are in radians unless otherwise noted.  Conversion helpers
// for MOA and milliradians (mil) are provided.
//
// References:
//   - Litz, "Applied Ballistics for Long Range Shooting" (ch. 6)
//   - McCoy, "Modern Exterior Ballistics" (ch. 5)
//   - Hatcher's Notebook

use std::f64::consts::PI;

// ── Constants ─────────────────────────────────────────────────────────────────

/// Standard gravitational acceleration at sea level (m/s²).
pub const GRAVITY: f64 = 9.806_65;

/// Radians per MOA (1 MOA = 1/60 of one degree).
pub const RAD_PER_MOA: f64 = PI / (180.0 * 60.0);

/// MOA per radian.
pub const MOA_PER_RAD: f64 = 180.0 * 60.0 / PI;

/// Radians per milliradian (mil).  1 mil ≡ 1 milliradian = 1/1000 rad.
pub const RAD_PER_MIL: f64 = 0.001;

/// Milliradians per radian.
pub const MIL_PER_RAD: f64 = 1000.0;

// ── Angular conversions ───────────────────────────────────────────────────────

/// Convert radians to MOA.
///
/// 1 MOA ≈ 1.047" at 100 yd (the standard US/NATO shooter's MOA).
#[inline]
pub fn rad_to_moa(rad: f64) -> f64 {
    rad * MOA_PER_RAD
}

/// Convert MOA to radians.
#[inline]
pub fn moa_to_rad(moa: f64) -> f64 {
    moa * RAD_PER_MOA
}

/// Convert radians to milliradians (mil).
#[inline]
pub fn rad_to_mil(rad: f64) -> f64 {
    rad * MIL_PER_RAD
}

/// Convert milliradians (mil) to radians.
#[inline]
pub fn mil_to_rad(mil: f64) -> f64 {
    mil * RAD_PER_MIL
}

// ── Core zero-angle computation ───────────────────────────────────────────────

/// Compute the bore-to-sight zero angle (radians), using the full
/// small‑angle‑correct form.
///
/// ```text
/// θ = atan(h / R) + asin(g × R / (2 × V²))
/// ```
///
/// where:
/// - `h` = `sight_height_m` — height of the sight axis above the bore axis
///   (metres)
/// - `R` = `zero_range_m` — distance at which the projectile crosses the
///   sight line (metres)
/// - `V` = `mv_ms` — muzzle velocity (m/s)
/// - `g` = [`GRAVITY`] = 9.80665 m/s²
///
/// The first term is the geometric angle required to align the bore with
/// the sight‑line‑to‑target.  The second term compensates for the
/// projectile's gravitational drop over the zero range.
///
/// # Returns
/// `Some(θ)` in radians, or `None` if the drop‑compensation argument
/// `g·R / (2·V²)` exceeds 1.0 (indicating the projectile cannot reach
/// the zero range at this muzzle velocity — an impossible zero).
///
/// # Edge cases
/// - Returns `Some(0.0)` when `mv_ms ≤ 0` or `zero_range_m ≤ 0`.
///
/// # Example
/// ```
/// # use abe_ballistics_ext::sight_height::*;
/// let angle = zero_angle(0.040, 100.0, 900.0).unwrap();
/// // ≈ 0.00101 rad ≈ 3.5 MOA for a typical 5.56 mm rifle
/// assert!((rad_to_moa(angle) - 3.5).abs() < 0.2);
/// ```
pub fn zero_angle(sight_height_m: f64, zero_range_m: f64, mv_ms: f64) -> Option<f64> {
    if mv_ms <= 0.0 || zero_range_m <= 0.0 {
        return Some(0.0);
    }

    // Geometric term: θ₁ = atan(h / R)
    let geo = (sight_height_m / zero_range_m).atan();

    // Drop‑compensation term: θ₂ = asin(g × R / (2 × V²))
    let arg = GRAVITY * zero_range_m / (2.0 * mv_ms * mv_ms);
    if arg > 1.0 {
        return None; // physically impossible zero
    }
    let drop = arg.asin();

    Some(geo + drop)
}

/// Simplified vacuum zero angle using the linearised form.
///
/// ```text
/// θ = atan((½ × g × (R / V)² + h) / R)
/// ```
///
/// This version folds the drop into the numerator before applying `atan`,
/// which is algebraically equivalent to the full form [`zero_angle`] for
/// all practical small angles but avoids the `asin` entirely.  It never
/// returns `None`.
///
/// Prefer [`zero_angle`] for production use; this is provided as a
/// building‑block for cases where `Option` ergonomics are inconvenient
/// (e.g. in array initialisers) or for validating the full form.
#[inline]
pub fn zero_angle_linear(sight_height_m: f64, zero_range_m: f64, mv_ms: f64) -> f64 {
    if mv_ms <= 0.0 || zero_range_m <= 0.0 {
        return 0.0;
    }
    let drop = vacuum_drop(zero_range_m, mv_ms);
    ((drop + sight_height_m) / zero_range_m).atan()
}

/// Convenience: return [`zero_angle`] in MOA.
pub fn zero_moa(sight_height_m: f64, zero_range_m: f64, mv_ms: f64) -> Option<f64> {
    zero_angle(sight_height_m, zero_range_m, mv_ms).map(rad_to_moa)
}

// ── Sensitivity / error analysis ──────────────────────────────────────────────

/// Sensitivity of the zero angle to sight‑height error.
///
/// Returns `∂θ/∂h` (rad/m) — the change in the bore‑to‑sight angle per
/// metre of sight‑height measurement error.
///
/// ```text
/// ∂θ/∂h = 1 / (R × (1 + (h/R)²)) = R / (R² + h²)
/// ```
///
/// Multiply by the height error *in metres* to get the angular zero shift.
/// For example, a 1 mm sight‑height error at 100 m zero with 40 mm height
/// shifts the zero by roughly 0.01 mrad — negligible for most applications.
pub fn height_sensitivity(sight_height_m: f64, zero_range_m: f64) -> f64 {
    if zero_range_m <= 0.0 {
        return 0.0;
    }
    let r2 = zero_range_m * zero_range_m;
    let h2 = sight_height_m * sight_height_m;
    zero_range_m / (r2 + h2)
}

/// Sensitivity of the zero angle to range error.
///
/// Returns `∂θ/∂R` (rad/m) — the change in the bore‑to‑sight angle per
/// metre of zero‑range error.
///
/// ```text
/// ∂θ/∂R = -h / (R² + h²) + g / (2 × V² × √(1 - (g×R/(2×V²))²))
/// ```
///
/// The first term comes from the geometric angle (negative: increasing R
/// reduces the required angle).  The second term comes from the drop
/// compensation (positive: longer range needs more elevation).
///
/// Returns `None` when the drop‑compensation term is at its physical limit
/// (same condition as [`zero_angle`]).
pub fn range_sensitivity(sight_height_m: f64, zero_range_m: f64, mv_ms: f64) -> Option<f64> {
    if mv_ms <= 0.0 || zero_range_m <= 0.0 {
        return Some(0.0);
    }

    let r2 = zero_range_m * zero_range_m;
    let h2 = sight_height_m * sight_height_m;

    // ∂/∂R [atan(h/R)] = -h / (R² + h²)
    let d_geo = -sight_height_m / (r2 + h2);

    // ∂/∂R [asin(g·R / (2·V²))]
    let arg = GRAVITY * zero_range_m / (2.0 * mv_ms * mv_ms);
    if arg >= 1.0 {
        return None;
    }
    let d_drop = GRAVITY / (2.0 * mv_ms * mv_ms * (1.0 - arg * arg).sqrt());

    Some(d_geo + d_drop)
}

/// Sensitivity of the zero angle to muzzle‑velocity error.
///
/// Returns `∂θ/∂V` (rad/(m/s)) — the change in the bore‑to‑sight angle per
/// m/s of muzzle‑velocity error.
///
/// Only the drop‑compensation term depends on MV:
///
/// ```text
/// ∂θ/∂V = -g × R / (V³ × √(1 - (g×R/(2×V²))²))
/// ```
///
/// Returns `None` when the drop‑compensation term is at its physical limit.
pub fn mv_sensitivity(_sight_height_m: f64, zero_range_m: f64, mv_ms: f64) -> Option<f64> {
    if mv_ms <= 0.0 || zero_range_m <= 0.0 {
        return Some(0.0);
    }

    let arg = GRAVITY * zero_range_m / (2.0 * mv_ms * mv_ms);
    if arg >= 1.0 {
        return None;
    }
    let denom = mv_ms * mv_ms * mv_ms * (1.0 - arg * arg).sqrt();
    Some(-GRAVITY * zero_range_m / denom)
}

// ── Trajectory helpers (vacuum) ───────────────────────────────────────────────

/// Gravitational drop at a given range in a vacuum (metres).
///
/// ```text
/// drop = ½ × g × (range / V)²
/// ```
///
/// The drop is the vertical distance the projectile has fallen below the
/// bore line due to gravity (positive = below the bore line).
#[inline]
pub fn vacuum_drop(range_m: f64, mv_ms: f64) -> f64 {
    if mv_ms <= 0.0 || range_m <= 0.0 {
        return 0.0;
    }
    let tof = range_m / mv_ms;
    0.5 * GRAVITY * tof * tof
}

/// Height of the vacuum trajectory above the bore axis at a given range.
///
/// ```text
/// y(x) = x × tan(θ) - ½ × g × (x / V)²
/// ```
///
/// Negative values mean the projectile is below the bore axis.
///
/// # Arguments
/// * `range_m` — down‑range distance (metres)
/// * `angle_rad` — launch angle above the bore axis (radians)
/// * `mv_ms` — muzzle velocity (m/s)
#[inline]
pub fn trajectory_height(range_m: f64, angle_rad: f64, mv_ms: f64) -> f64 {
    if mv_ms <= 0.0 || range_m <= 0.0 {
        return 0.0;
    }
    let rise = range_m * angle_rad.tan();
    let drop = vacuum_drop(range_m, mv_ms);
    rise - drop
}

// ── Click / sight‑adjustment utilities ────────────────────────────────────────

/// Compute the number of sight clicks for a given angular adjustment.
///
/// `moa_per_click` is the scope's click increment (e.g. 0.25 for a
/// ¼‑MOA scope, 0.1 for a 0.1 mrad scope).  The result is rounded to the
/// nearest whole click.
///
/// Returns `None` when `moa_per_click` is not positive.
pub fn clicks_for_adjustment(adjustment_moa: f64, moa_per_click: f64) -> Option<i32> {
    if moa_per_click <= 0.0 {
        return None;
    }
    Some((adjustment_moa / moa_per_click).round() as i32)
}

/// Compute elevation and windage clicks to zero a given impact offset.
///
/// Given the observed point‑of‑impact offset at a known range, returns
/// the number of clicks needed in (elevation, windage) to bring the
/// impact to the point‑of‑aim.
///
/// # Arguments
/// * `vertical_offset_m` — observed vertical offset (positive = high,
///   negative = low), in metres
/// * `horizontal_offset_m` — observed horizontal offset (positive = right),
///   in metres
/// * `target_range_m` — range at which the offset was measured (metres)
/// * `moa_per_click` — scope click increment in MOA
///
/// Returns `None` if any argument is invalid (zero or negative range or
/// click value).
pub fn correction_clicks(
    vertical_offset_m: f64,
    horizontal_offset_m: f64,
    target_range_m: f64,
    moa_per_click: f64,
) -> Option<(i32, i32)> {
    if target_range_m <= 0.0 || moa_per_click <= 0.0 {
        return None;
    }

    let elev_rad = (vertical_offset_m / target_range_m).atan();
    let wind_rad = (horizontal_offset_m / target_range_m).atan();

    let elev_moa = rad_to_moa(elev_rad);
    let wind_moa = rad_to_moa(wind_rad);

    Some((
        (elev_moa / moa_per_click).round() as i32,
        (wind_moa / moa_per_click).round() as i32,
    ))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Angular conversions ────────────────────────────────────────────

    #[test]
    fn moa_roundtrip() {
        let rad = 0.001;
        let moa = rad_to_moa(rad);
        let back = moa_to_rad(moa);
        assert!((rad - back).abs() < 1e-15);
    }

    #[test]
    fn mil_roundtrip() {
        let rad = 0.001;
        let mil = rad_to_mil(rad);
        let back = mil_to_rad(mil);
        assert!((rad - back).abs() < 1e-15);
    }

    #[test]
    fn moa_constant_is_plausible() {
        // 1 MOA ≈ 1.047" at 100 yd → ~1.0 MOA per 1.047" ≈ 0.0291 rad per MOA
        // More precisely: 1 MOA = π/(180×60) ≈ 0.000290888 rad
        let one_moa_rad = moa_to_rad(1.0);
        assert!((one_moa_rad - PI / 10800.0).abs() < 1e-15);
    }

    // ── Zero angle ─────────────────────────────────────────────────────

    #[test]
    fn typical_rifle_zero() {
        // 5.56 mm, 40 mm sight height, 100 m zero, 900 m/s
        let angle = zero_angle(0.040, 100.0, 900.0).unwrap();
        let moa = rad_to_moa(angle);
        // Expect ~3.46 MOA
        assert!((moa - 3.46).abs() < 0.1, "got {} MOA", moa);
    }

    #[test]
    fn zero_angle_increases_with_sight_height() {
        let low = zero_angle(0.020, 100.0, 900.0).unwrap();
        let high = zero_angle(0.080, 100.0, 900.0).unwrap();
        assert!(high > low, "higher sight needs more angle");
    }

    #[test]
    fn zero_angle_decreases_with_mv() {
        let slow = zero_angle(0.040, 100.0, 600.0).unwrap();
        let fast = zero_angle(0.040, 100.0, 1000.0).unwrap();
        assert!(fast < slow, "faster MV needs less angle");
    }

    #[test]
    fn zero_angle_increases_with_range() {
        let near = zero_angle(0.040, 50.0, 900.0).unwrap();
        let far = zero_angle(0.040, 300.0, 900.0).unwrap();
        assert!(far > near, "longer zero range needs more angle");
    }

    #[test]
    fn zero_angle_zero_range_returns_zero() {
        assert_eq!(zero_angle(0.040, 0.0, 900.0), Some(0.0));
    }

    #[test]
    fn zero_angle_zero_mv_returns_zero() {
        assert_eq!(zero_angle(0.040, 100.0, 0.0), Some(0.0));
    }

    #[test]
    fn zero_angle_negative_mv_returns_zero() {
        assert_eq!(zero_angle(0.040, 100.0, -1.0), Some(0.0));
    }

    #[test]
    fn zero_angle_impossible_range() {
        // g × R / (2 × V²) > 1 → impossible
        // At 100 m/s, R = 2500 m → 9.8 × 2500 / (2 × 10000) = 24500/20000 = 1.225 > 1
        let result = zero_angle(0.040, 2500.0, 100.0);
        assert!(result.is_none(), "should be impossible to zero");
    }

    #[test]
    fn zero_angle_linear_matches_full() {
        let h = 0.040;
        let r = 100.0;
        let v = 900.0;
        let full = zero_angle(h, r, v).unwrap();
        let linear = zero_angle_linear(h, r, v);
        let diff = (full - linear).abs();
        // The two forms are algebraically equivalent at small angles
        assert!(diff < 1e-6, "full={} linear={}", full, linear);
    }

    #[test]
    fn zero_moa_roundtrip() {
        let moa = zero_moa(0.040, 100.0, 900.0).unwrap();
        assert!(moa > 0.0);
        let back = zero_angle(0.040, 100.0, 900.0).unwrap();
        assert!((moa_to_rad(moa) - back).abs() < 1e-15);
    }

    // ── Sensitivity ────────────────────────────────────────────────────

    #[test]
    fn height_sensitivity_positive() {
        let s = height_sensitivity(0.040, 100.0);
        assert!(s > 0.0, "sensitivity should be positive");
        // ~9.99 × 10⁻³ rad/m ≈ 0.01 rad/m
        assert!((s - 0.01).abs() < 0.002, "got {:.6}", s);
    }

    #[test]
    fn height_sensitivity_zero_range() {
        assert_eq!(height_sensitivity(0.040, 0.0), 0.0);
    }

    #[test]
    fn range_sensitivity_negative_at_close_range() {
        // At very short ranges the geometric term dominates → ∂θ/∂R < 0
        let s = range_sensitivity(0.040, 10.0, 900.0).unwrap();
        assert!(s < 0.0, "short-range sensitivity should be negative");
    }

    #[test]
    fn mv_sensitivity_negative() {
        // ∂θ/∂V should always be negative (faster MV → less angle needed)
        let s = mv_sensitivity(0.040, 100.0, 900.0).unwrap();
        assert!(s < 0.0, "MV sensitivity should be negative");
    }

    #[test]
    fn mv_sensitivity_zero_range() {
        assert_eq!(mv_sensitivity(0.040, 0.0, 900.0), Some(0.0));
    }

    // ── Vacuum trajectory ──────────────────────────────────────────────

    #[test]
    fn vacuum_drop_at_100m() {
        let drop = vacuum_drop(100.0, 900.0);
        // t = 100/900 ≈ 0.111 s, drop = 0.5 × 9.807 × 0.111² ≈ 0.0605 m
        assert!((drop - 0.0605).abs() < 0.001, "got {} m", drop);
    }

    #[test]
    fn vacuum_drop_scales_quadratically() {
        let d100 = vacuum_drop(100.0, 900.0);
        let d200 = vacuum_drop(200.0, 900.0);
        // Drop scales with range²: d(2R) / d(R) ≈ 4
        let ratio = d200 / d100;
        assert!((ratio - 4.0).abs() < 0.01, "ratio={}", ratio);
    }

    #[test]
    fn vacuum_drop_zero_range() {
        assert_eq!(vacuum_drop(0.0, 900.0), 0.0);
    }

    #[test]
    fn projectile_intersects_sight_line_at_zero_range() {
        // At zero_range_m, the projectile's vacuum trajectory should
        // intersect the sight line (i.e. height ~0 above the target plane).
        // bore_horizontal_angle = zero_angle - atan(h/R) = asin(g·R/(2·V²))
        // y(R) = R·tan(bore_horiz_angle) - drop = 0 (target height)
        let h = 0.040;
        let r = 100.0;
        let v = 900.0;
        let zero_off = zero_angle(h, r, v).unwrap();
        let sight_angle = (h / r).atan();
        let bore_horiz_angle = zero_off - sight_angle;
        let y = r * bore_horiz_angle.tan() - vacuum_drop(r, v);
        assert!(y.abs() < 1e-6, "trajectory height at zero range: {:.2e}", y);
    }

    // ── Click utilities ────────────────────────────────────────────────

    #[test]
    fn clicks_for_adjustment_typical() {
        // 1 MOA adjustment, 0.25 MOA per click → 4 clicks
        let clicks = clicks_for_adjustment(1.0, 0.25);
        assert_eq!(clicks, Some(4));
    }

    #[test]
    fn clicks_for_adjustment_rounding() {
        // 1.3 MOA / 0.25 = 5.2 → 5 clicks
        let clicks = clicks_for_adjustment(1.3, 0.25);
        assert_eq!(clicks, Some(5));
    }

    #[test]
    fn clicks_for_adjustment_zero_click_value() {
        assert_eq!(clicks_for_adjustment(1.0, 0.0), None);
        assert_eq!(clicks_for_adjustment(1.0, -0.1), None);
    }

    #[test]
    fn correction_clicks_both_axes() {
        // 10 cm high, 5 cm right, 100 m, 0.25 MOA per click
        let (elev, wind) = correction_clicks(0.10, 0.05, 100.0, 0.25).unwrap();
        // 10 cm at 100 m → atan(0.1/100) ≈ 0.001 rad ≈ 3.44 MOA → 14 clicks
        // 5 cm at 100 m → atan(0.05/100) ≈ 0.0005 rad ≈ 1.72 MOA → 7 clicks
        assert!(elev > 0, "elevation clicks should be positive: {}", elev);
        assert!(wind > 0, "windage clicks should be positive: {}", wind);
    }

    #[test]
    fn correction_clicks_negative_offsets() {
        // 10 cm low → negative elevation
        let (elev, _) = correction_clicks(-0.10, 0.0, 100.0, 0.25).unwrap();
        assert!(elev < 0, "low offset should give negative clicks: {}", elev);
    }

    #[test]
    fn correction_clicks_invalid_range() {
        assert_eq!(correction_clicks(0.1, 0.0, 0.0, 0.25), None);
    }

    #[test]
    fn correction_clicks_invalid_click_value() {
        assert_eq!(correction_clicks(0.1, 0.0, 100.0, 0.0), None);
    }

    // ── Constants ──────────────────────────────────────────────────────

    #[test]
    fn constants_are_reasonable() {
        assert!((GRAVITY - 9.80665).abs() < 1e-10);
        assert!((RAD_PER_MOA - PI / 10800.0).abs() < 1e-15);
        assert!((MOA_PER_RAD - 10800.0 / PI).abs() < 1e-12);
        assert_eq!(RAD_PER_MIL, 0.001);
        assert_eq!(MIL_PER_RAD, 1000.0);
    }
}
