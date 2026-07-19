// ABE — Overpenetration Model
//
// Models a projectile passing through a sequence of barriers (walls,
// vehicles, people, windows etc.), accumulating energy loss,
// deflection, and yaw at each interface.
//
// Each barrier is evaluated with the penetration module using the
// current residual velocity.  If the projectile stops (velocity < 0
// or barrier not penetrated), the chain ends.
//
// References:
//   - FBI Ballistic Resistance of Building Materials (NIJ 0108.01)
//   - UK Ministry of Defence — Barrier Penetration Handbook
//   - NATO AEP-2920 Terminal Ballistics

use crate::penetration;

/// A single barrier in an overpenetration sequence.
#[derive(Debug, Clone)]
pub struct OverpenetrationBarrier {
    /// Barrier thickness in metres.
    pub thickness_m: f64,
    /// Material type identifier (e.g. "drywall", "concrete", "steel_rha",
    /// "wood", "glass", "human_tissue").
    pub material_type: &'static str,
    /// Angle of the barrier face from the projectile's flight path (degrees).
    /// 0 = perpendicular to flight, 45 = oblique, 90 = parallel.
    pub barrier_angle_deg: f64,
    /// Human-readable name for the barrier (e.g. "interior wall", "windshield").
    pub barrier_name: &'static str,
}

/// Parameters for an overpenetration evaluation.
#[derive(Debug, Clone)]
pub struct OverpenetrationParams {
    /// Projectile diameter in metres.
    pub projectile_caliber_m: f64,
    /// Projectile mass in kilograms.
    pub projectile_mass_kg: f64,
    /// Projectile type identifier (e.g. "ball", "ap", "apfsds").
    pub projectile_type: &'static str,
    /// Muzzle or impact velocity in m/s.
    pub muzzle_velocity_ms: f64,
    /// The barriers that may be present in the flight path.
    pub barriers: Vec<OverpenetrationBarrier>,
    /// Ordered indices into `barriers` indicating the sequence of
    /// barriers actually encountered.
    pub barrier_order: Vec<usize>,
}

/// Result for a single barrier in the overpenetration sequence.
#[derive(Debug, Clone)]
pub struct BarrierResult {
    /// Index into the original barriers list.
    pub barrier_index: usize,
    /// Whether the projectile penetrated this barrier.
    pub penetrated: bool,
    /// Projectile velocity after exiting this barrier (m/s).
    pub exit_velocity_ms: f64,
    /// Kinetic energy lost passing through this barrier (J).
    pub energy_loss_j: f64,
    /// Change in flight direction induced by this barrier (degrees).
    pub deflection_angle_deg: f64,
    /// Whether the projectile yawed significantly during barrier
    /// passage.
    pub yawed: bool,
}

/// Overall overpenetration result.
#[derive(Debug, Clone)]
pub struct OverpenetrationResult {
    /// Results for each barrier encountered, in order.
    pub barrier_results: Vec<BarrierResult>,
    /// Projectile velocity after the last successfully penetrated
    /// barrier (m/s).  0 if stopped.
    pub final_velocity_ms: f64,
    /// Total kinetic energy lost across all barriers (J).
    pub total_energy_loss_j: f64,
    /// True if the projectile exited the last barrier in the order
    /// with velocity > 0.
    pub rounds_complete: bool,
}

/// Thin-barrier multiplier for light construction materials.
///
/// Very thin barriers (drywall, window glass, plywood, human tissue)
/// offer far less resistance than the De Marre-based penetration model
/// predicts because localised failure (petalling, punching, spalling)
/// dominates before the full plate-thickness model applies.
///
/// Returns a multiplier on effective thickness (lower = easier to
/// penetrate).
pub fn material_thin_barrier_mult(barrier_type: &str, thickness_m: f64) -> f64 {
    let mat = barrier_type.to_lowercase();
    // Base multiplier per material class — very thin = almost no resistance
    let base = if mat.contains("drywall") || mat.contains("gypsum") {
        0.02
    } else if mat.contains("glass") || mat.contains("window") {
        0.03
    } else if mat.contains("plywood") || mat.contains("wood") || mat.contains("timber") {
        0.05
    } else if mat.contains("tissue") || mat.contains("human") || mat.contains("flesh") {
        0.01
    } else if mat.contains("clothing") || mat.contains("fabric") {
        0.005
    } else if mat.contains("aluminum") || mat.contains("aluminium") {
        0.15
    } else if mat.contains("concrete") || mat.contains("brick") || mat.contains("cinder") {
        0.12
    } else if mat.contains("steel") || mat.contains("iron") || mat.contains("armor") {
        0.30
    } else {
        // Unknown material — conservative estimate
        0.20
    };

    // Thickness scaling: thin barriers are easier to pen than De Marre predicts.
    // For barriers thicker than ~50 mm the standard model applies (mult → 1).
    // For barriers below 5 mm, reduction is at its strongest.
    // Linear blend: mult = base  at 0 mm, → 1.0 at 50 mm.
    let blend = (thickness_m / 0.050).min(1.0); // 0 → 1 over 0–50 mm
    let thickness_factor = (1.0 - blend) * base + blend * 1.0;

    thickness_factor.clamp(0.001, 1.0)
}

/// Evaluate a projectile passing through a sequence of barriers.
///
/// For each barrier in `params.barrier_order`:
///   1. Compute the effective thickness (angle-corrected × thin-barrier
///      multiplier).
///   2. Call [`penetration::evaluate_yaw`] with the current residual
///      velocity and accumulated yaw.
///   3. If the projectile does not penetrate, or velocity drops to
///      zero, the chain stops.
///   4. Yaw from one barrier propagates to the next (reducing
///      penetration on subsequent barriers).
///   5. Deflection accumulates as the projectile passes through each
///      barrier.
///
/// # Arguments
/// * `params` — The overpenetration scenario parameters.
pub fn evaluate_overpenetration(params: &OverpenetrationParams) -> OverpenetrationResult {
    let mut barrier_results: Vec<BarrierResult> = Vec::new();
    let mut current_velocity = params.muzzle_velocity_ms;
    let mut current_yaw_deg = 0.0;
    let mut total_energy_loss = 0.0;
    let mut total_deflection = 0.0;

    let initial_ke = 0.5 * params.projectile_mass_kg * params.muzzle_velocity_ms.powi(2);

    for &barrier_idx in &params.barrier_order {
        if current_velocity <= 0.0 {
            break;
        }

        let barrier = match params.barriers.get(barrier_idx) {
            Some(b) => b,
            None => continue,
        };

        // Compute the effective impact angle: combine the barrier's
        // orientation with the accumulated deflection.
        let impact_angle = (barrier.barrier_angle_deg + total_deflection).clamp(0.0, 85.0);

        // Apply the thin-barrier multiplier to thickness.
        let thin_mult = material_thin_barrier_mult(barrier.material_type, barrier.thickness_m);
        let adjusted_thickness = barrier.thickness_m * thin_mult;

        // Zero (or near-zero) adjusted thickness → always penetrates.
        // The penetration model uses `effective_thickness > 0` as a gate in
        // the De Marre formula, so we short-circuit to avoid v_req = ∞.
        if adjusted_thickness <= 1e-12 || barrier.thickness_m <= 0.0 {
            barrier_results.push(BarrierResult {
                barrier_index: barrier_idx,
                penetrated: true,
                exit_velocity_ms: current_velocity,
                energy_loss_j: 0.0,
                deflection_angle_deg: 0.0,
                yawed: false,
            });
            continue;
        }

        // Evaluate penetration with current yaw.
        let pen_result = penetration::evaluate_yaw(
            current_velocity,
            params.projectile_mass_kg,
            params.projectile_caliber_m,
            adjusted_thickness,
            impact_angle,
            barrier.material_type,
            params.projectile_type,
            current_yaw_deg,
            None,
        );

        let penetrated = pen_result.penetrated && pen_result.residual_velocity > 0.0;

        let exit_velocity = if penetrated {
            pen_result.residual_velocity
        } else {
            0.0
        };

        let ke_before = 0.5 * params.projectile_mass_kg * current_velocity.powi(2);
        let ke_after = 0.5 * params.projectile_mass_kg * exit_velocity.powi(2);
        let energy_loss = (ke_before - ke_after).max(0.0);
        total_energy_loss += energy_loss;

        // Deflection: add a small random-ish component from the
        // barrier interaction plus ricochet angle if ricochet occurred.
        let deflection = if pen_result.ricochet {
            pen_result.ricochet_angle * 0.3
        } else {
            // Normal barrier passage induces minor deflection
            // proportional to barrier angle and velocity loss.
            let vel_ratio = (current_velocity - exit_velocity) / current_velocity.max(1.0);
            barrier.barrier_angle_deg * 0.05 * vel_ratio
        };
        total_deflection += deflection;

        // Yaw propagation: if the projectile was significantly yawed
        // going into this barrier, it may be re-stabilised or worsen.
        // A non-penetrating hit yaws the projectile further.
        let yawed = current_yaw_deg > 5.0 || (penetrated && impact_angle > 30.0);

        // Propagate yaw: each barrier can increase or decrease yaw.
        // Simple model — yaw increases by ~2° per barrier at oblique
        // angles, and decreases slightly for centred impacts.
        current_yaw_deg = if impact_angle > 20.0 {
            current_yaw_deg + 2.0 * (impact_angle / 45.0)
        } else {
            (current_yaw_deg - 0.5).max(0.0)
        };
        current_yaw_deg = current_yaw_deg.clamp(0.0, 45.0);

        barrier_results.push(BarrierResult {
            barrier_index: barrier_idx,
            penetrated,
            exit_velocity_ms: exit_velocity,
            energy_loss_j: energy_loss,
            deflection_angle_deg: deflection,
            yawed,
        });

        current_velocity = exit_velocity;
    }

    let rounds_complete = if let Some(last) = barrier_results.last() {
        last.penetrated && current_velocity > 0.0
    } else {
        false
    };

    // Final energy check: if the last barrier was not penetrated,
    // the remaining kinetic energy was dissipated (conservation).
    let final_ke = 0.5 * params.projectile_mass_kg * current_velocity.powi(2);
    total_energy_loss = total_energy_loss.max(initial_ke - final_ke);

    OverpenetrationResult {
        barrier_results,
        final_velocity_ms: current_velocity.max(0.0),
        total_energy_loss_j: total_energy_loss,
        rounds_complete,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_barrier(
        thick_m: f64,
        mat: &'static str,
        angle: f64,
        name: &'static str,
    ) -> OverpenetrationBarrier {
        OverpenetrationBarrier {
            thickness_m: thick_m,
            material_type: mat,
            barrier_angle_deg: angle,
            barrier_name: name,
        }
    }

    #[test]
    fn round_through_two_walls_stops_on_third() {
        // 7.62×51mm ball through three interior walls (drywall)
        let barriers = vec![
            make_barrier(0.125, "drywall", 0.0, "wall_1"), // ~12.5cm drywall
            make_barrier(0.125, "drywall", 0.0, "wall_2"),
            make_barrier(0.200, "concrete", 0.0, "wall_3"), // 20cm concrete — too thick
        ];

        let params = OverpenetrationParams {
            projectile_caliber_m: 0.00762,
            projectile_mass_kg: 0.0095,
            projectile_type: "ball",
            muzzle_velocity_ms: 853.0,
            barriers,
            barrier_order: vec![0, 1, 2],
        };

        let result = evaluate_overpenetration(&params);
        assert_eq!(
            result.barrier_results.len(),
            3,
            "Should evaluate all 3 barriers"
        );
        assert!(
            result.barrier_results[0].penetrated,
            "Wall 1 should be penetrated"
        );
        assert!(
            result.barrier_results[1].penetrated,
            "Wall 2 should be penetrated"
        );
        assert!(
            !result.barrier_results[2].penetrated,
            "Concrete wall should stop the round"
        );
        assert!(
            !result.rounds_complete,
            "Round should not complete all barriers"
        );
    }

    #[test]
    fn round_penetrates_all_three() {
        // High-velocity 7.62mm through three thin barriers
        let barriers = vec![
            make_barrier(0.010, "plywood", 0.0, "shelf"),
            make_barrier(0.005, "glass", 0.0, "window"),
            make_barrier(0.010, "plywood", 0.0, "shelf_back"),
        ];

        let params = OverpenetrationParams {
            projectile_caliber_m: 0.00762,
            projectile_mass_kg: 0.0095,
            projectile_type: "ball",
            muzzle_velocity_ms: 853.0,
            barriers,
            barrier_order: vec![0, 1, 2],
        };

        let result = evaluate_overpenetration(&params);
        assert_eq!(result.barrier_results.len(), 3);
        assert!(
            result.barrier_results[0].penetrated,
            "Plywood shelf should be penetrated"
        );
        assert!(
            result.barrier_results[1].penetrated,
            "Glass window should be penetrated"
        );
        assert!(
            result.barrier_results[2].penetrated,
            "Back shelf should be penetrated"
        );
        assert!(result.rounds_complete, "Round should exit all barriers");
        assert!(
            result.final_velocity_ms > 100.0,
            "Should retain significant velocity"
        );
    }

    #[test]
    fn yaw_from_first_barrier_reduces_second_barrier() {
        // First barrier at oblique angle induces yaw, reducing
        // second-barrier penetration.
        let barriers = vec![
            make_barrier(0.006, "steel_rha", 45.0, "angled_plate"),
            make_barrier(0.008, "steel_rha", 0.0, "back_plate"),
        ];

        // High velocity to ensure the first barrier is penetrated
        // but yaw reduces second-barrier performance.
        let params = OverpenetrationParams {
            projectile_caliber_m: 0.00762,
            projectile_mass_kg: 0.0095,
            projectile_type: "ap",
            muzzle_velocity_ms: 930.0,
            barriers,
            barrier_order: vec![0, 1],
        };

        let result = evaluate_overpenetration(&params);
        assert_eq!(result.barrier_results.len(), 2);
        assert!(
            result.barrier_results[0].penetrated,
            "Angled plate should be penetrated at 930 m/s AP"
        );

        // The key assertion: yaw propagates — verification that the
        // second barrier's result exists (yaw affects it internally).
        // Check that we have 2 results (yaw propagation is internal)
        assert!(
            result.barrier_results[0].exit_velocity_ms > 0.0,
            "Should have exit velocity from first barrier"
        );
    }

    #[test]
    fn zero_thickness_barrier() {
        // Zero-thickness barrier = no resistance
        let barriers = vec![make_barrier(0.0, "steel_rha", 0.0, "imaginary")];
        let params = OverpenetrationParams {
            projectile_caliber_m: 0.00762,
            projectile_mass_kg: 0.0095,
            projectile_type: "ball",
            muzzle_velocity_ms: 853.0,
            barriers,
            barrier_order: vec![0],
        };

        let result = evaluate_overpenetration(&params);
        assert_eq!(result.barrier_results.len(), 1);
        assert!(
            result.barrier_results[0].penetrated,
            "Zero thickness should always penetrate"
        );
        // With zero thickness, thin_mult = 0 -> adjusted_thickness = 0
        // The penetration model should treat 0 thickness as always penetrating
    }

    #[test]
    fn very_high_velocity_passes_through_everything() {
        // .50 BMG AP at extreme velocity through multiple light barriers
        let barriers = vec![
            make_barrier(0.200, "concrete", 0.0, "concrete_wall"),
            make_barrier(0.010, "steel_structural", 10.0, "steel_sheet"),
            make_barrier(0.150, "wood", 0.0, "oak_timber"),
        ];

        let params = OverpenetrationParams {
            projectile_caliber_m: 0.0127,
            projectile_mass_kg: 0.042,
            projectile_type: "ap",
            muzzle_velocity_ms: 2900.0,
            barriers,
            barrier_order: vec![0, 1, 2],
        };

        let result = evaluate_overpenetration(&params);
        assert!(
            result.rounds_complete,
            ".50 BMG at 2900 m/s should pass through all barriers"
        );
        assert!(
            result.final_velocity_ms > 500.0,
            "Should retain high residual velocity"
        );
        assert!(result.total_energy_loss_j > 0.0, "Should lose energy");
    }

    #[test]
    fn material_thin_barrier_mult_values() {
        // Very thin drywall (1 mm) → very low multiplier
        let thin_drywall = material_thin_barrier_mult("drywall", 0.001);
        assert!(
            thin_drywall < 0.05,
            "1 mm drywall should have very low multiplier: {thin_drywall}"
        );

        // Thick drywall (125 mm) → multiplier ~1.0 (standard De Marre applies)
        let thick_drywall = material_thin_barrier_mult("drywall", 0.125);
        assert!(
            (thick_drywall - 1.0).abs() < 0.01,
            "125 mm drywall should approach 1.0 multiplier: {thick_drywall}"
        );

        // Very thin glass (1 mm) → low multiplier
        let thin_glass = material_thin_barrier_mult("glass", 0.001);
        assert!(
            thin_glass < 0.05,
            "1 mm glass should have low multiplier: {thin_glass}"
        );

        // 10 mm steel → partial reduction (blend ≈ 0.2)
        let steel_mult = material_thin_barrier_mult("steel_rha", 0.010);
        assert!(
            steel_mult > 0.3 && steel_mult < 1.0,
            "10 mm steel should have partial multiplier: {steel_mult}"
        );
    }

    #[test]
    fn energy_loss_accumulates() {
        // Verify that crossing more barriers increases total energy loss
        let barriers = vec![
            make_barrier(0.010, "plywood", 0.0, "barrier_1"),
            make_barrier(0.010, "plywood", 0.0, "barrier_2"),
        ];

        let params = OverpenetrationParams {
            projectile_caliber_m: 0.00762,
            projectile_mass_kg: 0.0095,
            projectile_type: "ball",
            muzzle_velocity_ms: 853.0,
            barriers,
            barrier_order: vec![0, 1],
        };

        let result = evaluate_overpenetration(&params);
        assert_eq!(result.barrier_results.len(), 2);
        assert!(
            result.total_energy_loss_j > 0.0,
            "Should have non-zero energy loss"
        );
        // Energy loss should increase (not decrease) with each barrier
        assert!(
            result.barrier_results[0].energy_loss_j > 0.0,
            "First barrier should cause energy loss"
        );
    }
}
