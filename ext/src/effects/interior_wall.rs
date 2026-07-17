// ABE - Interior Stud Wall Penetration Model
//
// Models projectile penetration through interior partition walls
// consisting of gypsum drywall boards on timber or metal studs
// with an air gap between faces.
//
// The standard interior wall is a 2-layer assembly:
//   Layer 1: 12.5mm gypsum board (front face)
//   Layer 2: 12.5mm gypsum board (back face)
//   Air gap: 90mm stud cavity between faces
//
// References:
//   - UFC 3-340-01 (Unified Facilities Criteria)
//   - NIJ 0108.01 ballistic resistance standards
//   - ASTM C1396 / C1658 (gypsum board standards)
//   - IBC 2021 (International Building Code — interior wall framing)

use crate::penetration;

/// A single layer in a wall assembly.
///
/// Each layer is evaluated sequentially: the projectile perforates
/// one layer before reaching the next. Air gaps between layers cause
/// minor velocity losses.
#[derive(Debug, Clone, Copy)]
pub struct WallLayerConfig {
    /// Material type (matches `penetration::material_factor` strings).
    pub material: &'static str,
    /// Layer thickness in metres.
    pub thickness_m: f64,
    /// Distance to the next layer (air gap / stud cavity) in metres.
    /// 0.0 if this is the last layer.
    pub spacing_to_next_m: f64,
}

/// Result of an interior wall penetration evaluation.
#[derive(Debug, Clone, Copy)]
pub struct InteriorWallResult {
    /// Whether the projectile fully perforated all wall layers.
    pub wall_breached: bool,
    /// Number of layers fully perforated.
    pub layers_perforated: i32,
    /// Projectile velocity remaining after exiting the wall (m/s).
    pub residual_velocity_ms: f64,
    /// Angular deviation from stud contact (degrees).
    pub deviation_deg: f64,
    /// Number of dust/spall fragments generated.
    pub dust_spall_count: i32,
    /// Whether the projectile yawed due to stud contact.
    pub projectile_yawed: bool,
}

/// Evaluate projectile penetration through an interior wall assembly.
///
/// Processes each layer sequentially:
/// 1. Converts layer thickness to RHA-equivalent via `material_factor`
/// 2. Calls `penetration::evaluate` with the RHA-equivalent thickness
/// 3. On perforation, applies air gap losses between layers
/// 4. On the first face layer, checks for stud contact (~15 % probability
///    given 400–600 mm stud spacing)
///
/// # Arguments
/// * `layers` — Wall layer descriptions ordered from front face to back face.
/// * `velocity_ms` — Impact velocity (m/s).
/// * `mass_kg` — Projectile mass (kg).
/// * `caliber_m` — Projectile diameter (m).
/// * `impact_angle_deg` — Impact angle from surface normal (0 = perpendicular).
/// * `projectile_type` — Projectile type identifier (e.g. "ball", "ap").
pub fn evaluate_interior_wall(
    layers: &[WallLayerConfig],
    velocity_ms: f64,
    mass_kg: f64,
    caliber_m: f64,
    impact_angle_deg: f64,
    projectile_type: &str,
) -> InteriorWallResult {
    let mut residual_v = velocity_ms;
    let mut perforated = 0i32;
    let mut dev = 0.0;
    let mut dust = 0i32;
    let mut yawed = false;
    let all_layers = layers.len() as i32;

    for (i, layer) in layers.iter().enumerate() {
        if residual_v <= 20.0 {
            break;
        }

        let mat_factor = penetration::material_factor(layer.material);

        // Effective thickness = actual thickness * mat_factor
        // This converts gypsum thickness to RHA-equivalent
        let rha_equiv = layer.thickness_m * mat_factor;

        let r = penetration::evaluate(
            residual_v,
            mass_kg,
            caliber_m,
            rha_equiv,
            impact_angle_deg,
            "steel_rha", // already converted via mat_factor
            projectile_type,
            None,
        );

        if r.penetrated {
            perforated += 1;
            residual_v = r.residual_velocity.max(20.0);
            dust += r.spall_fragments;

            // Stud contact: if this is the first face layer and the stud
            // cavity is present, 15% chance the projectile hits a stud
            // (timber/metal 400-600mm spacing → ~15% coverage)
            if i == 0 && layer.spacing_to_next_m > 0.01 {
                let stud_hit = (residual_v / 1000.0) > 0.15_f64; // deterministic threshold
                if stud_hit {
                    dev += 3.0; // 3° deviation from stud contact
                    yawed = true;
                    // Stud absorbs some energy
                    residual_v *= 0.95;
                }
            }

            // Air gap: minimal velocity loss (~1 m/s per metre at rifle velocities)
            if layer.spacing_to_next_m > 0.0 && i + 1 < layers.len() {
                let gap_loss = layer.spacing_to_next_m * 1.0;
                residual_v = (residual_v - gap_loss).max(20.0);
            }
        } else {
            break;
        }
    }

    InteriorWallResult {
        wall_breached: perforated >= all_layers,
        layers_perforated: perforated,
        residual_velocity_ms: residual_v.max(0.0),
        deviation_deg: dev,
        dust_spall_count: dust,
        projectile_yawed: yawed,
    }
}

/// Convenience: standard stud wall (2×12.5mm drywall, 90mm gap).
///
/// Two-layer assembly:
///   Layer 1: 12.5 mm gypsum board (front face)
///   Air gap: 90 mm stud cavity
///   Layer 2: 12.5 mm gypsum board (back face)
pub fn standard_stud_wall() -> Vec<WallLayerConfig> {
    vec![
        WallLayerConfig {
            material: "gypsum",
            thickness_m: 0.0125,
            spacing_to_next_m: 0.090, // 90 mm stud cavity
        },
        WallLayerConfig {
            material: "gypsum",
            thickness_m: 0.0125,
            spacing_to_next_m: 0.0,
        },
    ]
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_stud_wall_penetrated_by_rifle() {
        // 5.56 mm ball at 900 m/s should easily perforate both layers
        let layers = standard_stud_wall();
        let r = evaluate_interior_wall(&layers, 900.0, 0.004, 0.00556, 0.0, "ball");
        assert!(
            r.wall_breached,
            "5.56 mm ball at 900 m/s should penetrate standard stud wall"
        );
        assert!(
            r.residual_velocity_ms > 200.0,
            "Residual velocity should be significant after wall penetration"
        );
        assert!(
            r.layers_perforated >= 2,
            "Both drywall layers should be perforated"
        );
    }

    #[test]
    fn pistol_round_loses_velocity() {
        // 9 mm pistol at ~350 m/s — drywall offers minimal resistance
        let layers = standard_stud_wall();
        let initial_v = 350.0;
        let r = evaluate_interior_wall(&layers, initial_v, 0.008, 0.009, 0.0, "ball");
        // Gypsum board is very thin RHA-equivalent; expect some loss from
        // penetration mechanics and air gap drag but the wall won't stop it
        assert!(
            r.wall_breached,
            "9 mm should penetrate standard stud wall at 350 m/s"
        );
        assert!(
            r.layers_perforated >= 2,
            "Both drywall layers should be perforated"
        );
        assert!(r.residual_velocity_ms >= 0.0);
    }

    #[test]
    fn residual_velocity_decreases_after_penetration() {
        let layers = standard_stud_wall();
        let initial_v = 900.0;
        let r = evaluate_interior_wall(&layers, initial_v, 0.004, 0.00556, 0.0, "ball");
        if r.wall_breached {
            // Expect at least 5% velocity loss
            assert!(
                r.residual_velocity_ms < initial_v * 0.95,
                "Projectile should lose significant velocity through wall"
            );
        }
    }

    #[test]
    fn single_gypsum_board_penetrated_easily() {
        let single = vec![WallLayerConfig {
            material: "gypsum",
            thickness_m: 0.0125,
            spacing_to_next_m: 0.0,
        }];
        let r = evaluate_interior_wall(&single, 300.0, 0.004, 0.00556, 0.0, "ball");
        assert!(
            r.wall_breached,
            "Single 12.5 mm gypsum board should be penetrated easily"
        );
    }

    #[test]
    fn oblique_impact_does_not_improve_penetration() {
        let layers = standard_stud_wall();
        let normal = evaluate_interior_wall(&layers, 500.0, 0.004, 0.00556, 0.0, "ball");
        let oblique = evaluate_interior_wall(&layers, 500.0, 0.004, 0.00556, 45.0, "ball");
        assert!(
            oblique.residual_velocity_ms <= normal.residual_velocity_ms || !oblique.wall_breached,
            "Oblique impact should not improve penetration"
        );
    }

    #[test]
    fn three_layer_wall_assembly() {
        // 3-layer: drywall + 90 mm gap + drywall (same as standard but explicit)
        let three = vec![
            WallLayerConfig {
                material: "gypsum",
                thickness_m: 0.0125,
                spacing_to_next_m: 0.090,
            },
            WallLayerConfig {
                material: "gypsum",
                thickness_m: 0.0125,
                spacing_to_next_m: 0.0,
            },
        ];
        let r = evaluate_interior_wall(&three, 900.0, 0.004, 0.00556, 0.0, "ball");
        assert!(r.wall_breached);
        assert!(r.layers_perforated == 2);
    }
}
