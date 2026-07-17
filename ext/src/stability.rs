// ABE - Gyroscopic Stability Factor
//
// Computes the gyroscopic stability factor (Sg) for spin-stabilised
// projectiles.  Based on the Miller twist rule and McCoy's method from
// "Modern Exterior Ballistics".
//
// # Gyroscopic stability factor Sg
//
// ```text
// Sg = (I_x² × ω²) / (4 × I_y × Mα × q × S × d)
// ```
//
// where:
// - I_x   = axial moment of inertia (kg·m²)
// - I_y   = transverse moment of inertia (kg·m²)
// - ω     = spin rate (rad/s) = 2π × twist_rate_rev_per_m × velocity_ms
// - Mα    = overturning moment coefficient derivative (dimensionless)
// - q     = dynamic pressure = 0.5 × ρ × v² (Pa)
// - S     = cross-sectional area = π × d²/4 (m²)
// - d     = caliber (m)
//
// # Recommended Mα values
// - Spitzer boat-tail (rifle): 1.2 – 1.5
// - Blunt / round-nose (pistol): 0.8 – 1.0
// - Default: 1.2
//
// # Stability criteria
// - Sg > 1.3: stable flight
// - Sg > 3.0: over-stabilised (the projectile resists nutation damping,
//   degrading accuracy)
// - Sg < 1.0: unstable (tumbling)
//
// References:
//   - McCoy, "Modern Exterior Ballistics", Ch. 8
//   - Miller, "A Basic Guide to Understanding Twist Requirements"
//     (Precision Shooting, 1993)
//   - Litz, "Applied Ballistics for Long Range Shooting", Ch. 8

use std::f64::consts::PI;

/// Gyroscopic stability factor Sg.
///
/// Values above 1.3 indicate stable flight; above 3.0 means over-stabilised.
///
/// # Arguments
/// * `velocity_ms` - Projectile velocity relative to air (m/s)
/// * `twist_rate_rev_per_m` - Rifling twist (rev/m, positive for right-hand)
/// * `caliber_m` - Projectile diameter (m)
/// * `projectile_mass_kg` - Projectile mass (kg)
/// * `air_density_kgm3` - Ambient air density (kg/m³)
/// * `projectile_type` - Nose / shape hint ("spitzer", "blunt", or other)
pub fn gyroscopic_stability(
    velocity_ms: f64,
    twist_rate_rev_per_m: f64,
    caliber_m: f64,
    projectile_mass_kg: f64,
    air_density_kgm3: f64,
    projectile_type: &str,
) -> f64 {
    if velocity_ms <= 0.0
        || twist_rate_rev_per_m <= 0.0
        || caliber_m <= 0.0
        || projectile_mass_kg <= 0.0
        || air_density_kgm3 <= 0.0
    {
        return 0.0;
    }

    // Estimate moments of inertia
    let (i_x, i_y) = estimate_inertia(projectile_mass_kg, caliber_m);

    // Spin rate (rad/s)
    let omega = 2.0 * PI * twist_rate_rev_per_m * velocity_ms;

    // Overturning moment coefficient
    let m_alpha = match projectile_type {
        s if s.eq_ignore_ascii_case("spitzer") => 1.4,
        s if s.eq_ignore_ascii_case("blunt") => 0.9,
        _ => 1.2,
    };

    // Dynamic pressure
    let q = 0.5 * air_density_kgm3 * velocity_ms.powi(2);

    // Cross-sectional area
    let area = PI * caliber_m.powi(2) / 4.0;

    // Gyroscopic stability factor
    let numerator = i_x.powi(2) * omega.powi(2);
    let denominator = 4.0 * i_y * m_alpha * q * area * caliber_m;

    if denominator <= 0.0 {
        return 0.0;
    }
    numerator / denominator
}

/// Returns `true` when Sg indicates stable flight (Sg > 1.3).
///
/// The theoretical minimum is 1.0; a 30 % margin accounts for transient
/// aerodynamic disturbances.
pub fn is_stable(sg: f64) -> bool {
    sg > 1.3
}

/// Returns `true` when Sg exceeds the over-stabilisation threshold (Sg > 3.0).
///
/// Over-stabilised rounds resist nutation damping, which degrades long-range
/// accuracy.  The optimal range is 1.3 – 2.5.
pub fn is_over_stabilized(sg: f64) -> bool {
    sg > 3.0
}

/// The ideal Sg range for accurate fire: (1.3, 2.5).
pub fn optimal_sg_range() -> (f64, f64) {
    (1.3, 2.5)
}

/// Estimate axial (I_x) and transverse (I_y) moments of inertia.
///
/// Uses simple geometric proxies when CAD data is unavailable:
/// - **I_x**: thin-cylinder (solid cylinder) approximation `½ m r²`
/// - **I_y**: uniform-rod approximation `m L² / 12`
/// - **Length**: derived from mass and a generic density of 8500 kg/m³
///   (roughly halfway between lead and copper).
///
/// These approximations tend to over-estimate I_x (and therefore Sg) for
/// real jacketed projectiles, which have their mass concentrated closer
/// to the axis than a uniform solid cylinder.
pub fn estimate_inertia(mass_kg: f64, caliber_m: f64) -> (f64, f64) {
    let radius = caliber_m / 2.0;

    // Axial MOI – solid cylinder
    let i_x = 0.5 * mass_kg * radius.powi(2);

    // Estimate projectile length from mass and material density
    let density = 8500.0; // kg/m³ (generic jacketed-bullet density)
    let volume = mass_kg / density;
    let cross_section = PI * radius.powi(2);
    let length_m = volume / cross_section;

    // Transverse MOI – uniform rod about its centre
    let i_y = mass_kg * length_m.powi(2) / 12.0;

    (i_x, i_y)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Unit tests for stability predicates ──────────────────────────────

    #[test]
    fn is_stable_above_threshold() {
        assert!(is_stable(2.0));
        assert!(is_stable(1.5));
        assert!(is_stable(1.31));
    }

    #[test]
    fn is_stable_below_threshold() {
        assert!(!is_stable(0.9));
        assert!(!is_stable(1.0));
        assert!(!is_stable(1.29));
    }

    #[test]
    fn is_stable_at_exactly_one_point_three() {
        // Sg = 1.3 is marginal — NOT stable (rule is "strictly greater")
        assert!(!is_stable(1.3));
    }

    #[test]
    fn is_over_stabilized_above() {
        assert!(is_over_stabilized(3.1));
        assert!(is_over_stabilized(4.0));
    }

    #[test]
    fn is_over_stabilized_below() {
        assert!(!is_over_stabilized(1.5));
        assert!(!is_over_stabilized(2.9));
        assert!(!is_over_stabilized(3.0));
    }

    #[test]
    fn optimal_sg_range_returns_expected() {
        let (lo, hi) = optimal_sg_range();
        assert!((lo - 1.3).abs() < f64::EPSILON);
        assert!((hi - 2.5).abs() < f64::EPSILON);
    }

    // ── Inertia estimation ────────────────────────────────────────────────

    #[test]
    fn estimate_inertia_returns_positive() {
        let (ix, iy) = estimate_inertia(0.004, 0.00556);
        assert!(ix > 0.0, "I_x should be positive: {}", ix);
        assert!(iy > 0.0, "I_y should be positive: {}", iy);
    }

    #[test]
    fn estimate_inertia_ix_less_than_iy() {
        // For a long thin projectile I_x < I_y
        let (ix, iy) = estimate_inertia(0.004, 0.00556);
        // 5.56mm is relatively slender → I_x should be smaller than I_y
        assert!(ix < iy, "I_x ({}) should be < I_y ({})", ix, iy);
    }

    // ── Integration tests ─────────────────────────────────────────────────

    #[test]
    fn m855_sg_range() {
        // M855 from M4 (1:7" twist) at sea level
        let sg = gyroscopic_stability(
            930.0,       // muzzle velocity (m/s)
            1.0 / 0.178, // 1:7" twist → 5.618 rev/m
            0.00556,     // caliber (m)
            0.004,       // mass (kg)
            1.225,       // sea-level air density (kg/m³)
            "spitzer",
        );
        assert!(sg > 0.0, "Sg should be positive: {}", sg);
        // The thin-cylinder / uniform-rod MOI model over-estimates I_x,
        // so the computed Sg is higher than real-world measurements.
        // The key check is that M855 is firmly stable.
        assert!(sg > 1.3, "M855 should be stable: Sg={}", sg);
    }

    #[test]
    fn m80_sg_range() {
        // 7.62mm M80 ball from M240 (1:12" twist) at sea level
        let sg = gyroscopic_stability(
            850.0,       // muzzle velocity (m/s)
            1.0 / 0.305, // 1:12" twist → 3.279 rev/m
            0.00762,     // caliber (m)
            0.0095,      // mass (kg)
            1.225,       // sea-level air density
            "spitzer",
        );
        assert!(sg > 0.0, "Sg should be positive: {}", sg);
        assert!(
            sg > 1.0,
            "M80 should be at least marginally stable: Sg={}",
            sg
        );
    }

    #[test]
    fn nine_mm_sg_range() {
        // 9mm FMJ from typical service pistol (1:10" twist)
        let sg = gyroscopic_stability(
            360.0,       // muzzle velocity (m/s)
            1.0 / 0.254, // 1:10" twist → 3.937 rev/m
            0.00901,     // caliber (m)
            0.008,       // mass (kg)
            1.225,
            "blunt",
        );
        // 9mm should at least be computed as positive
        assert!(sg > 0.0, "Sg should be positive: {}", sg);
    }

    #[test]
    fn stability_increases_with_altitude() {
        // Sg ∝ 1/q ∝ 1/ρ.  Lower density at altitude → higher Sg
        // (less aerodynamic destabilising force → more stable).
        let sg_sea = gyroscopic_stability(930.0, 1.0 / 0.178, 0.00556, 0.004, 1.225, "spitzer");
        let sg_high = gyroscopic_stability(930.0, 1.0 / 0.178, 0.00556, 0.004, 0.736, "spitzer");

        assert!(
            sg_high > sg_sea,
            "Sg at altitude ({}) should be > Sg at sea level ({})",
            sg_high,
            sg_sea
        );
    }

    #[test]
    fn fifty_bmg_sg() {
        // .50 BMG from M2 (1:15" twist) at sea level
        let sg = gyroscopic_stability(
            850.0,
            1.0 / 0.381, // 1:15" twist → 2.625 rev/m
            0.0127,      // caliber (m)
            0.042,       // mass (kg)
            1.225,
            "spitzer",
        );
        assert!(sg > 0.0, "Sg should be positive: {}", sg);
    }

    #[test]
    fn sg_approximately_invariant_with_velocity() {
        // Sg ∝ ω² / q.  Since ω ∝ v and q ∝ v², the velocity dependence
        // cancels in the simple model — Sg is approximately velocity-invariant.
        let low_v = gyroscopic_stability(500.0, 1.0 / 0.178, 0.00556, 0.004, 1.225, "spitzer");
        let high_v = gyroscopic_stability(1000.0, 1.0 / 0.178, 0.00556, 0.004, 1.225, "spitzer");
        let diff = (high_v - low_v).abs();
        assert!(
            diff < 0.01,
            "Sg should be approx velocity-invariant: {} vs {} (diff={})",
            low_v,
            high_v,
            diff
        );
    }

    #[test]
    fn sg_increases_with_twist() {
        let slow_twist = gyroscopic_stability(930.0, 1.0 / 0.305, 0.00556, 0.004, 1.225, "spitzer"); // 1:12"
        let fast_twist = gyroscopic_stability(930.0, 1.0 / 0.178, 0.00556, 0.004, 1.225, "spitzer"); // 1:7"
        assert!(
            fast_twist > slow_twist,
            "Faster twist should increase Sg: {} vs {}",
            slow_twist,
            fast_twist
        );
    }

    #[test]
    fn sg_zero_for_invalid_inputs() {
        assert_eq!(
            gyroscopic_stability(0.0, 5.0, 0.00556, 0.004, 1.225, "spitzer"),
            0.0
        );
        assert_eq!(
            gyroscopic_stability(930.0, 0.0, 0.00556, 0.004, 1.225, "spitzer"),
            0.0
        );
        assert_eq!(
            gyroscopic_stability(930.0, 5.0, 0.0, 0.004, 1.225, "spitzer"),
            0.0
        );
    }

    #[test]
    fn default_projectile_type_works() {
        let sg = gyroscopic_stability(930.0, 1.0 / 0.178, 0.00556, 0.004, 1.225, "unknown");
        assert!(sg > 0.0, "Sg should be positive for default type: {}", sg);
    }

    #[test]
    fn estimate_inertia_consistent_units() {
        let (ix, iy) = estimate_inertia(0.004, 0.00556);
        // SI units: kg·m²
        assert!(ix > 0.0 && iy > 0.0);
        // Both should be reasonable magnitudes for a rifle bullet
        assert!(ix > 1e-10, "I_x implausibly small: {}", ix);
        assert!(iy > 1e-10, "I_y implausibly small: {}", iy);
    }
}
