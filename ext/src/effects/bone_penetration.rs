// ── Bone model constants ───────────────────────────────────────────────────────

/// Cortical bone density (kg/m³).
const CORTICAL_BONE_DENSITY: f64 = 1900.0;

/// Trabecular bone density (kg/m³).
#[allow(dead_code)]
const TRABECULAR_BONE_DENSITY: f64 = 1000.0;

/// Compressive strength of cortical bone (MPa).
#[allow(dead_code)]
const CORTICAL_BONE_STRENGTH_MPA: f64 = 170.0;

/// Bone penetration calibration constant.
/// E_bone = K_BONE × t² × sqrt(density) × area
/// Calibrated so that:
///   - .22 LR (142 J) does NOT penetrate skull (7mm)
///   - 9mm (518 J) is borderline for sternum (6mm)
///   - M855 (1730 J) penetrates femur (20mm) with ~55% velocity retention
const K_BONE: f64 = 1.0e9;

// ── Bone types ─────────────────────────────────────────────────────────────────

/// Types of bone that a projectile may encounter in a wound track.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BoneType {
    Skull,
    Rib,
    Femur,
    Sternum,
    Pelvis,
    Tibia,
    Humerus,
    Spine,
    Generic { thickness_m: f64 },
}

impl BoneType {
    /// Estimated thickness of this bone (metres).
    pub fn thickness_m(&self) -> f64 {
        match self {
            BoneType::Skull => 0.007,
            BoneType::Rib => 0.002,
            BoneType::Femur => 0.020,
            BoneType::Sternum => 0.006,
            BoneType::Pelvis => 0.008,
            BoneType::Tibia => 0.015,
            BoneType::Humerus => 0.012,
            BoneType::Spine => 0.010,
            BoneType::Generic { thickness_m } => *thickness_m,
        }
    }

    /// Effective density used for penetration energy calculation (kg/m³).
    /// Uses cortical density as the primary resistance layer.
    pub fn effective_density(&self) -> f64 {
        CORTICAL_BONE_DENSITY
    }

    /// Estimated bone cross-sectional area presented to the projectile (m²).
    /// Roughly 3-8× the projectile presented area for most bones.
    pub fn effective_area_m2(&self, caliber_m: f64) -> f64 {
        let width_mult = match self {
            BoneType::Skull => 3.0,
            BoneType::Rib => 2.0,
            BoneType::Femur => 3.0,
            BoneType::Sternum => 4.0,
            BoneType::Pelvis => 6.0,
            BoneType::Tibia => 4.0,
            BoneType::Humerus => 4.0,
            BoneType::Spine => 8.0,
            BoneType::Generic { .. } => 4.0,
        };
        let proj_area = std::f64::consts::PI * (caliber_m / 2.0).powi(2);
        proj_area * width_mult
    }
}

// ── Bone impact result ─────────────────────────────────────────────────────────

/// Result of evaluating a projectile impact against bone.
#[derive(Debug, Clone, Copy)]
pub struct BoneImpactResult {
    /// Whether the projectile fully penetrated the bone.
    pub penetrated: bool,
    /// Number of bone fragments generated (0 if no fracture).
    pub bone_fragments: i32,
    /// Total estimated mass of bone fragments (grams).
    pub bone_fragment_mass_g: f64,
    /// Angle the projectile was deflected (degrees).
    pub deflection_angle_deg: f64,
    /// Projectile velocity after interacting with bone (m/s).
    pub velocity_after_bone_ms: f64,
    /// Energy deposited in the bone (J).
    pub energy_deposited_in_bone_j: f64,
}

/// Evaluate projectile impact against a bone.
///
/// Physics model:
/// - Bone treated as a brittle plate with penetration energy:
///   E_bone = K_bone × thickness² × density_bone^0.5 × area_bone
/// - If remaining KE > E_bone: penetrate, reduce velocity
/// - If not: projectile stops/deflects, energy deposits as fracture
/// - Bone generates 2-8 sharp secondary fragments
/// - Deflection angle: 5-25° depending on impact angle and bone curvature
pub fn evaluate_bone_impact(
    velocity_ms: f64,
    mass_g: f64,
    caliber_m: f64,
    projectile_type: &str,
    bone: BoneType,
    impact_angle_deg: f64,
) -> BoneImpactResult {
    if velocity_ms <= 0.0 || mass_g <= 0.0 {
        return BoneImpactResult {
            penetrated: false,
            bone_fragments: 0,
            bone_fragment_mass_g: 0.0,
            deflection_angle_deg: 0.0,
            velocity_after_bone_ms: 0.0,
            energy_deposited_in_bone_j: 0.0,
        };
    }

    let mass_kg = mass_g / 1000.0;
    let ke = 0.5 * mass_kg * velocity_ms.powi(2);

    // Bone penetration energy: K_BONE × thickness² × density^0.5 × area
    let thickness = bone.thickness_m();
    let density = bone.effective_density();
    let area = bone.effective_area_m2(caliber_m);
    let e_bone = K_BONE * thickness.powi(2) * density.sqrt() * area;

    // Impact angle modifies effective energy: normal impact = full,
    // oblique = less energy transferred to penetration
    let angle_rad = impact_angle_deg.to_radians();
    let angle_factor = angle_rad.cos().max(0.1); // minimum 10% at extreme angles
    let e_bone_effective = e_bone / angle_factor;

    // AP projectiles resist better against bone
    let proj_type = projectile_type.to_lowercase();
    let is_ap = proj_type == "ap" || proj_type == "armor_piercing";
    let ap_bonus = if is_ap { 0.7 } else { 1.0 };
    let e_bone_effective = e_bone_effective * ap_bonus;

    // Determine penetration
    let (penetrated, vel_after, energy_in_bone) = if ke > e_bone_effective {
        // Penetrate: reduce velocity
        let frac = (e_bone_effective / ke).min(0.99);
        let vel_after = velocity_ms * (1.0 - frac).sqrt();
        (true, vel_after.max(0.0), e_bone_effective.min(ke))
    } else {
        // Stopped/deflected: all KE deposited
        (false, 0.0, ke)
    };

    // Bone fragments: 2-8 sharp secondary fragments
    // Scales with energy deposited in bone (roughly one fragment per 300 J)
    let fragment_count = if penetrated {
        let frag_by_energy = (energy_in_bone / 300.0).round() as i32;
        (2 + frag_by_energy).min(8)
    } else {
        // Even without penetration, near-penetrating impacts can crack bone
        if ke > e_bone_effective * 0.6 {
            ((energy_in_bone / 500.0).round() as i32).min(4)
        } else {
            0
        }
    };

    // Fragment mass: 0.1-2.0g each, total scales with energy
    let fragment_mass_g = if fragment_count > 0 {
        let avg_frag_mass = 0.1 + (energy_in_bone / 1000.0).min(1.9);
        fragment_count as f64 * avg_frag_mass.min(2.0)
    } else {
        0.0
    };

    // Deflection angle: 5-25° depending on impact angle and bone curvature
    // Oblique impacts deflect more; glancing impacts (high angle) deflect most
    let deflection = if penetrated {
        let base_deflection = 5.0 + impact_angle_deg * 0.15; // 5-18.5° for 0-90°
        base_deflection.min(25.0)
    } else {
        // Stopped projectiles may deflect sharply
        let base_deflection = 10.0 + impact_angle_deg * 0.2; // 10-28°
        base_deflection.min(30.0)
    };

    BoneImpactResult {
        penetrated,
        bone_fragments: fragment_count,
        bone_fragment_mass_g: fragment_mass_g,
        deflection_angle_deg: deflection,
        velocity_after_bone_ms: vel_after,
        energy_deposited_in_bone_j: energy_in_bone,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn m855_penetrates_femur() {
        let bone_result = evaluate_bone_impact(930.0, 4.0, 0.00556, "fmj", BoneType::Femur, 0.0);
        assert!(bone_result.penetrated, "M855 should penetrate femur");
        assert!(
            bone_result.velocity_after_bone_ms > 400.0
                && bone_result.velocity_after_bone_ms < 700.0,
            "M855 retains ~480 m/s after femur: {:.1} m/s",
            bone_result.velocity_after_bone_ms
        );
        assert!(
            bone_result.bone_fragments >= 2,
            "Femur impact should produce fragments: {}",
            bone_result.bone_fragments
        );
        assert!(
            bone_result.energy_deposited_in_bone_j > 200.0,
            "Bone should absorb significant energy: {:.1} J",
            bone_result.energy_deposited_in_bone_j
        );
    }

    #[test]
    fn nine_mm_borderline_sternum() {
        let bone_result = evaluate_bone_impact(360.0, 8.0, 0.00901, "fmj", BoneType::Sternum, 0.0);
        assert!(
            bone_result.energy_deposited_in_bone_j > 0.0,
            "Bone impact should deposit some energy"
        );
        assert!(
            bone_result.penetrated,
            "9mm at 360 m/s should penetrate sternum (~6mm)"
        );
    }

    #[test]
    fn twentytwo_lr_stopped_by_skull() {
        let bone_result = evaluate_bone_impact(330.0, 2.6, 0.00556, "fmj", BoneType::Skull, 0.0);
        assert!(!bone_result.penetrated, ".22 LR should be stopped by skull");
        assert!(
            (bone_result.velocity_after_bone_ms).abs() < 0.1,
            "Velocity after bone should be ~0 when stopped"
        );
    }

    #[test]
    fn lapua_destroys_femur() {
        let bone_result = evaluate_bone_impact(880.0, 16.2, 0.00858, "fmj", BoneType::Femur, 0.0);
        assert!(bone_result.penetrated, ".338 Lapua should penetrate femur");
        assert!(
            bone_result.bone_fragments >= 5,
            ".338 Lapua should produce massive fragmentation: {} fragments",
            bone_result.bone_fragments
        );
        assert!(
            bone_result.bone_fragment_mass_g > 2.0,
            "Massive bone fragmentation: {:.2} g",
            bone_result.bone_fragment_mass_g
        );
        assert!(
            bone_result.energy_deposited_in_bone_j > 500.0,
            ".338 Lapua deposits huge energy in bone: {:.0} J",
            bone_result.energy_deposited_in_bone_j
        );
    }

    #[test]
    fn bone_impact_at_oblique_angle() {
        let normal = evaluate_bone_impact(800.0, 9.5, 0.00762, "fmj", BoneType::Femur, 0.0);
        let oblique = evaluate_bone_impact(800.0, 9.5, 0.00762, "fmj", BoneType::Femur, 60.0);
        assert!(
            oblique.deflection_angle_deg >= normal.deflection_angle_deg,
            "Oblique angle should deflect more: normal={:.1}°, oblique={:.1}°",
            normal.deflection_angle_deg,
            oblique.deflection_angle_deg
        );
    }

    #[test]
    fn bone_type_thicknesses_plausible() {
        assert!((BoneType::Skull.thickness_m() - 0.007).abs() < 0.001);
        assert!((BoneType::Rib.thickness_m() - 0.002).abs() < 0.001);
        assert!((BoneType::Femur.thickness_m() - 0.020).abs() < 0.001);
        assert!((BoneType::Sternum.thickness_m() - 0.006).abs() < 0.001);
        assert!((BoneType::Pelvis.thickness_m() - 0.008).abs() < 0.001);
        assert!((BoneType::Tibia.thickness_m() - 0.015).abs() < 0.001);
        assert!((BoneType::Humerus.thickness_m() - 0.012).abs() < 0.001);
        assert!((BoneType::Spine.thickness_m() - 0.010).abs() < 0.001);
        let generic = BoneType::Generic { thickness_m: 0.005 };
        assert!((generic.thickness_m() - 0.005).abs() < 0.001);
    }

    #[test]
    fn bone_impact_zero_velocity() {
        let result = evaluate_bone_impact(0.0, 8.0, 0.00901, "fmj", BoneType::Skull, 0.0);
        assert!(!result.penetrated);
        assert_eq!(result.bone_fragments, 0);
    }

    #[test]
    fn ap_projectile_better_bone_penetration() {
        let fmj = evaluate_bone_impact(500.0, 10.0, 0.00762, "fmj", BoneType::Sternum, 0.0);
        let ap = evaluate_bone_impact(500.0, 10.0, 0.00762, "ap", BoneType::Sternum, 0.0);
        assert!(
            ap.penetrated
                || !fmj.penetrated
                || ap.velocity_after_bone_ms >= fmj.velocity_after_bone_ms - 0.01
        );
    }
}
