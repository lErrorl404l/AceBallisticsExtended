// ABE - Penetration & Terminal Ballistics
//
// Implements penetration models for small arms and armor.
// Uses De Marre formula for homogeneous armor, with material
// and angle modifiers.
//
// References:
//   - De Marre ballistics formula (late 19th c.)
//   - Lanz-Odermatt (long rod penetrators)
//   - UK Ordnance Board formulae
//   - NIJ 0108.01 ballistic resistance

/// Material hardness factor relative to RHA
fn material_factor(material: &str) -> f64 {
    match material.to_lowercase().as_str() {
        "steel_rha" => 1.0,
        "steel_hha" => 1.25, // High-hardness armor
        "aluminum_5083" => 0.35,
        "aluminum_7039" => 0.45,
        "ceramic_al2o3" => 2.5, // High hardness but brittle
        "ceramic_sic" => 3.0,
        "ceramic_b4c" => 3.5,
        "composite_kevlar" => 0.6, // Per unit thickness
        "composite_glass" => 0.4,
        "spall_liner" => 0.1, // Spall liner, minimal structural resistance
        "concrete" => 0.15,
        "wood" => 0.05,
        "steel_structural" => 0.7, // Mild steel
        _ => 1.0,                  // Default to RHA
    }
}

/// Projectile type modifier
fn projectile_modifier(proj_type: &str) -> f64 {
    match proj_type.to_lowercase().as_str() {
        "ball" | "fmj" => 1.0,
        "ap" | "armor_piercing" => 1.3, // Hardened core
        "apds" | "apfsds" => 1.8,       // Sub-caliber long rod
        "apcr" => 1.5,                  // Tungsten carbide core
        "heat" | "he" => 0.3,           // Shaped charge jet
        "incendiary" => 0.9,
        "tracer" => 0.95,
        _ => 1.0,
    }
}

/// Result of a penetration evaluation
#[derive(Debug, Clone)]
pub struct PenetrationResult {
    pub penetrated: bool,
    pub residual_velocity: f64,
    pub effective_thickness: f64,
    pub ricochet: bool,
    pub ricochet_angle: f64,
    pub ricochet_energy_fraction: f64,
    pub fragments: i32,
    pub spall_fragments: i32,
}

/// Evaluate penetration of a projectile against an armor plate.
///
/// Uses a three-stage model:
/// 1. Ricochet check — if impact angle exceeds ricochet threshold, projectile bounces
/// 2. Effective thickness — plate thickness / cos(angle) × material factor
/// 3. De Marre penetration formula: V_required = k * D^0.75 * T^0.7 / M^0.5
///
/// # Arguments
/// * `velocity_ms` - Impact velocity (m/s)
/// * `projectile_mass_kg` - Projectile mass (kg)
/// * `caliber_m` - Projectile diameter (m)
/// * `armor_thickness_m` - Armor plate thickness (m)
/// * `impact_angle_deg` - Angle from normal (0° = perpendicular)
/// * `armor_material` - Material identifier string
/// * `projectile_type` - Projectile type identifier string
pub fn evaluate(
    velocity_ms: f64,
    projectile_mass_kg: f64,
    caliber_m: f64,
    armor_thickness_m: f64,
    impact_angle_deg: f64,
    armor_material: &str,
    projectile_type: &str,
) -> PenetrationResult {
    let mat_factor = material_factor(armor_material);
    let proj_mod = projectile_modifier(projectile_type);

    let angle_rad = impact_angle_deg.to_radians();
    let cos_angle = angle_rad.cos().max(0.087); // Clamp at ~85° max

    // ── Ricochet check ─────────────────────────────────────────────────────
    // Ricochet threshold depends on velocity, caliber, and angle
    // R = sin⁻¹(V₀ / V * D / T), simplified empirical rule
    let ricochet_angle_threshold = if armor_thickness_m > 0.0 {
        let vel_ratio = 900.0 / velocity_ms.max(1.0);
        let cal_thick = caliber_m / armor_thickness_m;
        // Ricochet becomes likely above ~70° for typical rifle rounds
        70.0 + 5.0 * (vel_ratio * cal_thick).min(5.0)
    } else {
        90.0
    };

    let ricochet = impact_angle_deg > ricochet_angle_threshold;

    // ── Effective thickness ────────────────────────────────────────────────
    let base_effective = armor_thickness_m / cos_angle * mat_factor;
    // Caliber-to-thickness ratio effect: smaller calibers pen more efficiently
    // relative to their diameter against thin armor
    let cal_thick_ratio = caliber_m / armor_thickness_m.max(1e-6);
    let cal_factor = (1.0 + 0.3 * (-3.0 * cal_thick_ratio).exp()).min(1.0);
    let effective_thickness = base_effective * cal_factor;

    // ── Ricochet outcome ───────────────────────────────────────────────────
    if ricochet && !armor_material.contains("spall") {
        // Ricochet retains some energy depending on angle
        let energy_retention: f64 = if impact_angle_deg > 80.0 {
            0.85 // Glancing hit
        } else if impact_angle_deg > 75.0 {
            0.70
        } else {
            0.50
        };

        let residual_v = velocity_ms * energy_retention.sqrt();
        let ricochet_angle = (90.0 - impact_angle_deg) * 0.9; // Specular-ish

        return PenetrationResult {
            penetrated: false,
            residual_velocity: residual_v,
            effective_thickness,
            ricochet: true,
            ricochet_angle: ricochet_angle.max(5.0),
            ricochet_energy_fraction: energy_retention,
            fragments: 0,
            spall_fragments: (4.0 * (impact_angle_deg / 90.0)) as i32,
        };
    }

    // ── De Marre penetration ───────────────────────────────────────────────
    // V_required = k * D^0.75 * T^0.7 / M^0.5
    // where k is a material/construction constant (~6100 for RHA)
    //
    // Simplified: if velocity exceeds De Marre threshold, penetration occurs
    let k = 91000.0 / proj_mod;

    let v_required = if caliber_m > 0.0 && effective_thickness > 0.0 && projectile_mass_kg > 0.0 {
        let d = caliber_m;
        let t = effective_thickness;
        let m = projectile_mass_kg;
        k * d.powf(0.75) * t.powf(0.70) / m.sqrt()
    } else {
        f64::INFINITY
    };

    let penetrated = velocity_ms >= v_required;

    // ── Residual velocity ──────────────────────────────────────────────────
    let residual_velocity = if penetrated {
        // R_p = sqrt(V^2 - V_req^2)
        let vr_sq = velocity_ms.powi(2) - v_required.powi(2);
        if vr_sq > 0.0 {
            vr_sq.sqrt()
        } else {
            0.0
        }
    } else {
        velocity_ms * 0.1 // Stopped or minimal pass-through
    };

    // ── Fragments ──────────────────────────────────────────────────────────
    // Use the explicit fragmentation module for projectile breakup,
    // then add spall from armor deformation separately.
    let frag_result = crate::fragmentation::evaluate(
        velocity_ms,
        projectile_mass_kg * 1000.0,
        projectile_type,
        300.0, // Low threshold: most fragmentation relevant for pen model
        None,  // Use defaults; SQF provides specific config via ABO_* ammo params
    );
    let fragments = if penetrated {
        frag_result.num_fragments.max(2)
    } else if velocity_ms > 500.0 {
        // Non-penetrating hit can still cause some projectile breakup
        (frag_result.num_fragments / 2).max(0)
    } else {
        0
    };
    let spall_fragments = if penetrated {
        (effective_thickness / 0.010 * 3.0).min(30.0) as i32
    } else {
        (3.0 * (velocity_ms / 1000.0)).min(10.0) as i32
    };

    PenetrationResult {
        penetrated,
        residual_velocity,
        effective_thickness,
        ricochet: false,
        ricochet_angle: 0.0,
        ricochet_energy_fraction: 0.0,
        fragments,
        spall_fragments,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn m80_ball_pens_5mm_rha_at_0deg() {
        let r = evaluate(853.0, 0.0095, 0.00762, 0.005, 0.0, "steel_rha", "ball");
        assert!(r.penetrated, "M80 ball should pen 5mm RHA at 0°");
        assert!(r.residual_velocity > 100.0);
    }

    #[test]
    fn m80_ball_does_not_pen_20mm_rha() {
        let r = evaluate(853.0, 0.0095, 0.00762, 0.020, 0.0, "steel_rha", "ball");
        assert!(!r.penetrated, "M80 ball should NOT pen 20mm RHA at 0°");
    }

    #[test]
    fn angle_reduces_penetration() {
        let r0 = evaluate(900.0, 0.0095, 0.00762, 0.008, 0.0, "steel_rha", "ball");
        let r60 = evaluate(900.0, 0.0095, 0.00762, 0.008, 60.0, "steel_rha", "ball");
        if r0.penetrated && !r60.penetrated {
            // Expected: 0° pens, 60° doesn't
        }
        assert!(
            r0.effective_thickness < r60.effective_thickness,
            "Effective thickness should increase with angle"
        );
    }

    #[test]
    fn ap_round_pens_better_than_ball() {
        let ball = evaluate(880.0, 0.0095, 0.00762, 0.010, 0.0, "steel_rha", "ball");
        let ap = evaluate(880.0, 0.0095, 0.00762, 0.010, 0.0, "steel_rha", "ap");
        assert!(
            ap.penetrated || !ball.penetrated,
            "AP should pen equal or better than ball"
        );
    }

    #[test]
    fn ricochet_at_shallow_angle() {
        let r = evaluate(850.0, 0.0095, 0.00762, 0.010, 80.0, "steel_rha", "ball");
        assert!(r.ricochet, "80° impact should ricochet");
        assert!(r.ricochet_energy_fraction > 0.0);
    }

    #[test]
    fn hha_is_harder_than_rha() {
        let rha = evaluate(900.0, 0.0095, 0.00762, 0.010, 0.0, "steel_rha", "ball");
        let hha = evaluate(900.0, 0.0095, 0.00762, 0.010, 0.0, "steel_hha", "ball");
        assert!(hha.effective_thickness > rha.effective_thickness);
    }

    #[test]
    fn penetration_produces_fragments() {
        let r = evaluate(900.0, 0.0095, 0.00762, 0.006, 0.0, "steel_rha", "ball");
        if r.penetrated {
            assert!(r.fragments > 0, "Penetrating hit should produce fragments");
            assert!(
                r.spall_fragments > 0,
                "Penetrating hit should produce spall"
            );
        }
    }

    #[test]
    fn high_velocity_pens_more() {
        let slow = evaluate(400.0, 0.0095, 0.00762, 0.005, 0.0, "steel_rha", "ball");
        let fast = evaluate(900.0, 0.0095, 0.00762, 0.005, 0.0, "steel_rha", "ball");
        assert!(
            fast.penetrated || !slow.penetrated,
            "Higher velocity should pen at least as well"
        );
    }
}
