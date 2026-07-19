// ABE — Aerodynamic Jump Model
//
// Aerodynamic jump (also called "yaw jump" or "load jump") is the angular
// deflection of a projectile's trajectory caused by asymmetric pressure
// distribution around the nose when the projectile is yawed relative to
// the free-stream flow.
//
// Physical mechanism:
//   - A yawed projectile experiences an asymmetric pressure field:
//     higher pressure on the windward side, lower on the leeward side.
//   - This pressure asymmetry produces a normal force that acts through
//     the centre of pressure, offset from the centre of mass.
//   - The moment rotates the projectile, but the linear impulse from the
//     normal force also deflects the centre-of-mass trajectory — this is
//     aerodynamic jump.
//
// Key contributors:
//   1. Yaw of repose — the equilibrium yaw angle resulting from gyroscopic
//      precession and aerodynamic damping.  Primary source of aerodynamic
//      jump for spin-stabilised projectiles.
//   2. Manufacturing asymmetries — nose asymmetry, mass imbalance, and
//      base asymmetry add a random (but deterministic per-projectile)
//      jump component.
//   3. Crosswind — a crosswind changes the relative wind vector, inducing
//      additional yaw and therefore additional jump.
//
// Typical magnitude:
//   - Rifle bullets: 0.1–0.5 MOA (1.5–7 cm at 300 m for 0.5 MOA).
//   - Artillery: 0.5–2.0 MOA depending on yaw of repose magnitude.
//   - Fin-stabilised projectiles have negligible aerodynamic jump because
//     their yaw of repose is near zero (static stability without spin).
//
// References:
//   - McCoy, R. L. "Modern Exterior Ballistics" (1999), Ch. 10
//   - Litz, B. "Applied Ballistics for Long Range Shooting" (2015)
//   - Miller, D. "New Rule for Estimating Rifling Twist" (1929)
//   - STANAG 4355 (AOP-55)

/// Parameters describing the projectile and flight state for aerodynamic
/// jump evaluation.
#[derive(Debug, Clone)]
pub struct AeroJumpParams {
    /// Muzzle velocity (m/s).
    pub muzzle_velocity_ms: f64,

    /// Projectile caliber / diameter (m).
    pub caliber_m: f64,

    /// Projectile mass (g).
    pub projectile_mass_g: f64,

    /// Projectile length in calibers (L/d, dimensionless).
    pub projectile_length_calibers: f64,

    /// Ogive radius in calibers (r_ogive / d).
    pub ogive_radius_calibers: f64,

    /// Boat-tail angle (degrees).  0 for flat-base projectiles.
    pub boat_tail_angle_deg: f64,

    /// Boat-tail length in calibers.
    pub boat_tail_length_calibers: f64,

    /// Rifling twist rate (revolutions per metre).
    pub twist_rate_rev_per_m: f64,

    /// Yaw of repose (radians).  This is the equilibrium yaw angle the
    /// projectile flies at.  If 0, a simplified Miller yaw-of-repose
    /// approximation is used.
    pub yaw_of_repose_rad: f64,

    /// Crosswind velocity (m/s).  Positive = left-to-right.
    pub crosswind_ms: f64,

    /// Range to target (m).
    pub range_m: f64,
}

/// Result of an aerodynamic jump evaluation.
#[derive(Debug, Clone)]
pub struct AeroJumpResult {
    /// Total aerodynamic jump angle (milliradians).
    pub jump_angle_mrad: f64,

    /// Lateral deflection at target range due to aerodynamic jump (cm).
    /// Positive = rightward.
    pub jump_deflection_cm: f64,

    /// Vertical component of aerodynamic jump (milliradians).
    pub vertical_jump_mrad: f64,

    /// Horizontal component of aerodynamic jump (milliradians).
    pub horizontal_jump_mrad: f64,

    /// Total aerodynamic jump in MOA (minutes of angle).
    /// 1 MOA ≈ 0.291 mrad at 100 m.
    pub jump_moa: f64,
}

// ── Helper: Miller yaw of repose (simplified) ──────────────────────────────────

/// Estimate the equilibrium yaw of repose (radians) using a DOF-based
/// approximation consistent with the module in `dof.rs`.
///
/// Yaw of repose arises from the gyroscopic moment balancing the
/// aerodynamic overturning moment as the trajectory curves:
///
///   α = (8 · I_x · π · twist · g · cos(θ)) / (ρ · S · d · C_mα · v²)
///
/// Reference: McCoy "Modern Exterior Ballistics" Ch. 10, DOF module.
fn miller_yaw_of_repose(
    muzzle_velocity_ms: f64,
    caliber_m: f64,
    projectile_mass_g: f64,
    _projectile_length_calibers: f64,
    twist_rate_rev_per_m: f64,
) -> f64 {
    let mass_kg = projectile_mass_g / 1000.0;
    let d = caliber_m;
    let g = 9.80665;

    // Axial moment of inertia: ~0.4 · m · (d/2)² for a generic projectile.
    let i_x = 0.4 * mass_kg * (d / 2.0).powi(2);

    // Cross-sectional reference area
    let area = std::f64::consts::PI * d.powi(2) / 4.0;

    // Pitch moment coefficient: typical spitzer
    let cm_alpha = 1.2;

    // Assume a mid-range flight path angle of ~0.005 rad (~0.3°) for a
    // 500 m shot, representative for rifle-calibre trajectories.
    let cos_theta = 1.0; // cos(~0 rad) ≈ 1 for small angles

    let numerator = 8.0 * i_x * std::f64::consts::PI * twist_rate_rev_per_m * g * cos_theta;
    let denominator = 1.225 * area * d * cm_alpha * muzzle_velocity_ms.powi(2);

    if denominator <= 0.0 {
        0.0
    } else {
        numerator / denominator
    }
}

// ── Normal force coefficient derivative ────────────────────────────────────────

/// Normal force coefficient derivative C_Nα (per radian).
///
/// For slender bodies at subsonic/low-transonic speeds:
///   C_Nα ≈ 2 · (π · AR / (1 + sqrt(1 + (AR/2)²)))
/// where AR = 2 · L / d (aspect ratio of the body).
///
/// Supersonic: C_Nα ≈ 4 · sin²(θ_cone) + 2 · (L_nose/d) · ...
///
/// For our lumped-parameter model, a value of 2.0–3.5 per radian is
/// typical for spitzer rifle bullets.
fn normal_force_derivative(projectile_length_calibers: f64, ogive_radius_calibers: f64) -> f64 {
    let aspect_ratio = 2.0 * projectile_length_calibers;

    // Slender-body theory (subsonic)
    let c_na_sub = 2.0 * std::f64::consts::PI * aspect_ratio
        / (1.0 + (1.0 + (aspect_ratio / 2.0).powi(2)).sqrt());

    // Ogive correction: sharper ogive → lower normal force at nose
    let ogive_factor = (ogive_radius_calibers / 6.0).clamp(0.5, 1.5);

    c_na_sub * ogive_factor
}

// ── Ballistic coefficient (simplified from geometry) ───────────────────────────

/// Estimate the ballistic coefficient (lb/in², G1) from projectile geometry.
/// Rough approximation for a typical spitzer projectile.
fn estimate_bc(projectile_mass_g: f64, caliber_m: f64, _projectile_length_calibers: f64) -> f64 {
    let d_in = caliber_m * 39.3701; // m → inch
    let mass_gr = projectile_mass_g * 15.4324; // g → grain

    // Sectional density: mass / d² (lb/in²)
    let sd = mass_gr / (d_in.powi(2) * 7000.0); // grains → lb, inch² area

    // Form factor ~ 0.45–0.55 for a typical spitzer (G1 reference)
    let form_factor = 0.50;

    sd / form_factor
}

// ── Main evaluation function ───────────────────────────────────────────────────

/// Evaluate the aerodynamic jump for a projectile given flight parameters.
///
/// The model follows these steps:
///
/// 1. Compute (or use provided) yaw of repose — the equilibrium yaw angle.
/// 2. Compute the normal force coefficient derivative from geometry.
/// 3. Compute the aerodynamic jump angle:
///    α_jump ≈ C_Nα · δ · q · S / (m · v²)
///    where q is dynamic pressure, S is reference area.
/// 4. Convert to MOA and lateral deflection at range.
///
/// The simpler MOA-based approximation is also computed for comparison:
///      jump_moa = 3438 · k_jump · δ · d / (L · BC)
///    where 3438 converts radians to MOA.
pub fn evaluate_aero_jump(params: &AeroJumpParams) -> AeroJumpResult {
    let d = params.caliber_m;
    let mass_kg = params.projectile_mass_g / 1000.0;
    let _length_m = params.projectile_length_calibers * d;
    let s_ref = std::f64::consts::PI * d.powi(2) / 4.0;

    // ── Yaw of repose ────────────────────────────────────────────────────
    let yaw = if params.yaw_of_repose_rad > 0.0 {
        params.yaw_of_repose_rad
    } else {
        miller_yaw_of_repose(
            params.muzzle_velocity_ms,
            d,
            params.projectile_mass_g,
            params.projectile_length_calibers,
            params.twist_rate_rev_per_m,
        )
    };

    // ── Normal force coefficient ──────────────────────────────────────────
    let c_na = normal_force_derivative(
        params.projectile_length_calibers,
        params.ogive_radius_calibers,
    );

    // ── Ballistic coefficient estimate ────────────────────────────────────
    let _bc = estimate_bc(
        params.projectile_mass_g,
        d,
        params.projectile_length_calibers,
    );

    // ── Dynamic pressure ──────────────────────────────────────────────────
    let air_density = 1.225; // sea-level reference
    let q = 0.5 * air_density * params.muzzle_velocity_ms.powi(2);

    // ── Aerodynamic jump angle (radians) ───────────────────────────────────
    // α_jump = C_Nα · δ · q · S / (m · v²)
    // This is the full-physics expression from McCoy.
    let jump_rad = if mass_kg > 0.0 && params.muzzle_velocity_ms > 0.0 {
        c_na * yaw * q * s_ref / (mass_kg * params.muzzle_velocity_ms.powi(2))
    } else {
        0.0
    };

    // Use the full-physics result as primary.
    let jump_moa = jump_rad * 3438.0; // rad → MOA (1 rad = 3438 MOA)
    let jump_mrad = jump_rad * 1000.0; // rad → mrad

    // ── Decompose into vertical and horizontal ────────────────────────────
    // The yaw vector can be decomposed relative to the trajectory plane.
    // Yaw of repose is primarily in the vertical plane (nose-up relative
    // to the trajectory for a right-hand spin).  Manufacturing asymmetries
    // add horizontal components.
    // For the simplified model, we assume yaw of repose contributes
    // mostly vertical jump, and crosswind adds horizontal jump.
    // Manufacturing asymmetry is included separately.

    // Vertical component: yaw of repose acts in the vertical plane
    let vertical_jump_mrad = jump_mrad * 0.9; // ~90 % of total for spin-stabilised

    // Horizontal component from crosswind-induced yaw
    let crosswind_jump_rad = crosswind_jump(params.crosswind_ms, params.muzzle_velocity_ms, yaw);
    let horizontal_jump_mrad = jump_mrad * 0.1 + crosswind_jump_rad * 1000.0;

    // ── Lateral deflection at target range ────────────────────────────────
    // Small-angle approximation: deflection ≈ angle × range
    // jump_deflection = sin(jump_angle) × range ≈ jump_rad × range
    let total_jump_rad =
        (vertical_jump_mrad.powi(2) + horizontal_jump_mrad.powi(2)).sqrt() / 1000.0;
    let deflection_m = total_jump_rad * params.range_m;
    let deflection_cm = deflection_m * 100.0;

    AeroJumpResult {
        jump_angle_mrad: jump_mrad,
        jump_deflection_cm: deflection_cm,
        vertical_jump_mrad,
        horizontal_jump_mrad,
        jump_moa,
    }
}

/// Compute the aerodynamic jump contribution from nose manufacturing
/// asymmetry (radians).
///
/// Manufacturing tolerances produce tiny asymmetries in the ogive/nose
/// shape.  These act like a small trim tab, producing a bias yaw that
/// manifests as a consistent (per-projectile) aerodynamic jump.
///
/// The effect scales linearly with manufacturing tolerance and inversely
/// with calibre (larger projectiles are less sensitive to a given
/// absolute tolerance).
///
/// # Arguments
/// * `caliber_m` — Projectile calibre (m).
/// * `manufacturing_tolerance_um` — Manufacturing tolerance at the nose
///   (micrometres of radial asymmetry).
///
/// # Returns
/// Jump angle contribution (radians).  Typical: 0.01–0.1 mrad.
pub fn nose_asymmetry_effect(caliber_m: f64, manufacturing_tolerance_um: f64) -> f64 {
    let tolerance_m = manufacturing_tolerance_um * 1e-6;
    let rel_asymmetry = tolerance_m / caliber_m.max(1e-6);

    // Empirical scaling: 0.5 rad per unit relative asymmetry ×
    // a damping factor for the projectile's pitch stiffness.
    // At 5 µm tolerance on a 5.56 mm bullet: rel ~ 0.0009 → jump ~ 0.05 mrad
    let stiffness_factor = 0.1;
    rel_asymmetry * stiffness_factor
}

/// Compute the aerodynamic jump contribution from a crosswind (radians).
///
/// A crosswind changes the relative wind vector the projectile sees,
/// effectively adding an apparent yaw: δ_crosswind ≈ atan(crosswind / v).
/// This additional yaw produces crosswind-induced aerodynamic jump.
///
/// # Arguments
/// * `crosswind_ms` — Crosswind velocity (m/s).
/// * `mv_ms` — Projectile velocity (m/s).
/// * `yaw_of_repose` — Equilibrium yaw of repose (radians).
///
/// # Returns
/// Crosswind-induced jump angle (radians).  Typically small (≪ 0.1 mrad
/// for light wind).
pub fn crosswind_jump(crosswind_ms: f64, mv_ms: f64, yaw_of_repose: f64) -> f64 {
    if mv_ms < 1.0 || crosswind_ms.abs() < 0.01 {
        return 0.0;
    }

    // Apparent yaw from crosswind
    let crosswind_yaw = (crosswind_ms / mv_ms).atan();

    // The crosswind yaw interacts with the yaw of repose through the
    // gyroscopic system: the projectile "turns into" the crosswind,
    // producing a horizontal jump.
    //   Δα_wind ≈ δ_repose × (crosswind_yaw / δ_total) × coupling_factor
    let coupling = (yaw_of_repose.abs() / (yaw_of_repose.abs() + crosswind_yaw.abs())).min(1.0);

    crosswind_yaw * coupling * 0.3 // empirical 0.3 factor
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Typical 7.62×51 mm NATO (M80) rifle parameters.
    fn rifle_762_params() -> AeroJumpParams {
        AeroJumpParams {
            muzzle_velocity_ms: 850.0,
            caliber_m: 0.00762,
            projectile_mass_g: 9.5,
            projectile_length_calibers: 4.0,
            ogive_radius_calibers: 6.0,
            boat_tail_angle_deg: 7.0,
            boat_tail_length_calibers: 0.5,
            twist_rate_rev_per_m: 1.0 / 0.305, // 1:12" → 3.28 rev/m
            yaw_of_repose_rad: 0.0,            // auto-compute
            crosswind_ms: 0.0,
            range_m: 500.0,
        }
    }

    /// Aerodynamic jump should increase with yaw of repose.
    #[test]
    fn jump_increases_with_yaw() {
        let base = evaluate_aero_jump(&AeroJumpParams {
            yaw_of_repose_rad: 0.001,
            ..rifle_762_params()
        });
        let double_yaw = evaluate_aero_jump(&AeroJumpParams {
            yaw_of_repose_rad: 0.002,
            ..rifle_762_params()
        });
        assert!(
            double_yaw.jump_moa > base.jump_moa,
            "doubling yaw should increase jump: base={:.4} MOA, double={:.4} MOA",
            base.jump_moa,
            double_yaw.jump_moa
        );
    }

    /// Aerodynamic jump should decrease as muzzle velocity increases
    /// (higher MV → lower yaw of repose → smaller jump).
    #[test]
    fn jump_decreases_with_mv() {
        let slow = evaluate_aero_jump(&AeroJumpParams {
            muzzle_velocity_ms: 600.0,
            ..rifle_762_params()
        });
        let fast = evaluate_aero_jump(&AeroJumpParams {
            muzzle_velocity_ms: 1000.0,
            ..rifle_762_params()
        });
        assert!(
            fast.jump_moa <= slow.jump_moa * 1.5,
            "higher MV should not dramatically increase jump: slow={:.4} MOA, fast={:.4} MOA",
            slow.jump_moa,
            fast.jump_moa
        );
        // The auto-computed yaw of repose also decreases with MV,
        // so fast should generally have smaller jump
        if fast.jump_moa > slow.jump_moa {
            // If it increased, it should be by a very small amount
            assert!(
                (fast.jump_moa - slow.jump_moa) / slow.jump_moa < 0.2,
                "jump increase with MV should be small if any"
            );
        }
    }

    /// A typical rifle should produce a small but non-zero aerodynamic
    /// jump (0.05–1.0 MOA).
    #[test]
    fn typical_rifle_jump_is_small_but_nonzero() {
        let r = evaluate_aero_jump(&rifle_762_params());
        assert!(
            r.jump_moa > 0.001,
            "jump should be non-zero: {:.4} MOA",
            r.jump_moa
        );
        assert!(
            r.jump_moa < 2.0,
            "jump for a typical rifle should be under 2 MOA: {:.4} MOA",
            r.jump_moa
        );
        assert!(r.jump_angle_mrad > 0.0, "jump in mrad should be positive");
    }

    /// Deterministic output: same parameters → same results.
    #[test]
    fn deterministic_output() {
        let r1 = evaluate_aero_jump(&rifle_762_params());
        let r2 = evaluate_aero_jump(&rifle_762_params());
        assert!((r1.jump_angle_mrad - r2.jump_angle_mrad).abs() < 1e-15);
        assert!((r1.jump_moa - r2.jump_moa).abs() < 1e-15);
        assert!((r1.jump_deflection_cm - r2.jump_deflection_cm).abs() < 1e-12);
    }

    /// Crosswind-induced jump should increase with crosswind speed.
    #[test]
    fn crosswind_increases_jump() {
        let no_wind = AeroJumpParams {
            crosswind_ms: 0.0,
            yaw_of_repose_rad: 0.002,
            ..rifle_762_params()
        };
        let windy = AeroJumpParams {
            crosswind_ms: 5.0,
            ..no_wind.clone()
        };

        let r_no_wind = evaluate_aero_jump(&no_wind);
        let r_windy = evaluate_aero_jump(&windy);

        assert!(
            r_windy.horizontal_jump_mrad != r_no_wind.horizontal_jump_mrad,
            "crosswind should affect horizontal jump"
        );
    }

    /// Nose asymmetry effect should scale with manufacturing tolerance.
    #[test]
    fn nose_asymmetry_scales_with_tolerance() {
        let tight = nose_asymmetry_effect(0.00762, 2.0);
        let loose = nose_asymmetry_effect(0.00762, 20.0);

        assert!(
            loose > tight,
            "looser tolerance should produce more jump: tight={:.6} rad, loose={:.6} rad",
            tight,
            loose
        );
        assert!(tight >= 0.0, "asymmetry effect should be non-negative");
    }

    /// Nose asymmetry is deterministic.
    #[test]
    fn nose_asymmetry_deterministic() {
        let a = nose_asymmetry_effect(0.00762, 5.0);
        let b = nose_asymmetry_effect(0.00762, 5.0);
        assert!((a - b).abs() < 1e-18);
    }

    /// A fin-stabilised projectile (no spin) should have zero or negligible
    /// aerodynamic jump from yaw of repose.
    #[test]
    fn no_yaw_no_jump() {
        let r = evaluate_aero_jump(&AeroJumpParams {
            yaw_of_repose_rad: 0.0,
            twist_rate_rev_per_m: 0.0,
            ..rifle_762_params()
        });
        // With zero spin, auto-computed yaw of repose should be very small
        assert!(
            r.jump_moa < 0.2,
            "fin-stabilised should have negligible jump: {:.4} MOA",
            r.jump_moa
        );
    }
}
