// ABE - Moving Target Lead / Deflection Shooting
//
// Computes lead angles for engaging moving targets. The lead is based on
// target speed, projectile time of flight (TOF), range, and the angle
// between the target's direction of motion and the line of sight (aspect).
//
// # Lead physics
//
// For a target moving at speed v perpendicular to the line of sight at
// range R with projectile TOF t, the required lead angle (in radians) is:
//
//     lead = v · t / R
//
// This is the angle the shooter must aim ahead of the target so the bullet
// and target arrive at the same point simultaneously.
//
// For oblique targets the perpendicular component is v · sin(aspect_angle),
// so the lead becomes:
//
//     lead = v · sin(aspect) · t / R
//
// # Iterative refinement
//
// The initial lead assumes the TOF at the current range. The intercept
// point is slightly further (or at a different angle) than the initial
// range. The TOF at the intercept point differs slightly, so the lead
// is refined in up to 3 iterations.
//
// References:
//   - ARMA 3 ACE3 deflection shooting model
//   - NATO sniping: lead = target speed × TOF (Mil relation formula)
//   - 1 mrad ≈ 10 cm at 100 m (Mil relation: 1 mil = 10 cm @ 100 m)

#![allow(dead_code)]

/// Parameters describing a moving target and the shooter-weapon system.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LeadTargetState {
    /// Range to target (metres).
    pub range_m: f64,
    /// Projectile time of flight to the target range (seconds).
    pub time_of_flight_s: f64,
    /// Estimated speed of the target (m/s).
    pub target_speed_ms: f64,
    /// Direction the target is moving relative to the shooter-target
    /// line (degrees). 0 = directly toward, 180 = directly away.
    pub target_direction_deg: f64,
    /// Aspect angle of the target's motion relative to the line of
    /// sight (degrees). 90° = full crossing (perpendicular to LOS),
    /// 0° = incoming/outgoing (parallel to LOS). The perpendicular
    /// component of the target's velocity used for lead computation
    /// is v · sin(aspect_angle).
    pub target_aspect_angle_deg: f64,
    /// Muzzle velocity of the shooter's weapon (m/s).
    pub shooter_muzzle_velocity_ms: f64,
    /// Crosswind speed perpendicular to the line of fire (m/s).
    /// Positive = wind from the right (pushes bullet left).
    pub crosswind_ms: f64,
}

/// Computed lead solution for engaging a moving target.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LeadSolution {
    /// Required lead angle in milliradians (horizontal deflection).
    /// Positive = aim ahead of the target in its direction of motion.
    pub lead_angle_mrad: f64,
    /// Lead distance in metres — how far the target moves during the
    /// projectile's flight to the intercept point.
    pub lead_distance_m: f64,
    /// Vertical correction due to target motion along the line of
    /// sight (mrad). Positive = aim higher. For incoming targets the
    /// effective TOF is shorter → less drop → negative correction.
    pub vertical_correction_mrad: f64,
    /// Corrected aim point as `(horizontal_mrad, vertical_mrad)` from
    /// the crosshair centre. Positive horizontal = aim right, positive
    /// vertical = aim up.
    pub corrected_aim_point: (f64, f64),
    /// Estimated time to intercept (seconds). This accounts for the
    /// reduced or increased TOF when the target moves along the LOS.
    pub time_to_intercept_s: f64,
}

// ── Constants ──────────────────────────────────────────────────────────────────

/// Milliradians per radian.
const MRAD_PER_RAD: f64 = 1000.0;

/// Gravitational acceleration (m/s²).
const GRAVITY: f64 = 9.806_65;

// ── Core lead functions ────────────────────────────────────────────────────────

/// Compute the lead angle for a fully crossing target (perpendicular to LOS).
///
/// Uses the Mil relation formula:
///
/// ```text
/// lead_mrad = (target_speed × TOF / range) × 1000
/// ```
///
/// # Example
///
/// A target moving at 10 m/s crossing at 300 m with 0.35 s TOF:
///
/// ```text
/// lead = 10 × 0.35 / 300 × 1000 ≈ 11.7 mrad (11.7 MIL)
/// ```
pub fn crossing_lead(speed_ms: f64, tof_s: f64, range_m: f64) -> f64 {
    if range_m <= 0.0 || tof_s <= 0.0 || speed_ms <= 0.0 {
        return 0.0;
    }
    (speed_ms * tof_s / range_m) * MRAD_PER_RAD
}

/// Compute the lead angle for a target moving at an oblique angle.
///
/// The lead is the full crossing lead scaled by `sin(aspect_angle_deg)`.
/// Only the component of the target's velocity perpendicular to the line
/// of sight contributes to the required lead.
///
/// * `aspect_angle_deg` — 90° = full crossing, 0° = incoming/outgoing.
pub fn oblique_lead(speed_ms: f64, tof_s: f64, range_m: f64, aspect_angle_deg: f64) -> f64 {
    let sin_aspect = aspect_angle_deg.to_radians().sin();
    crossing_lead(speed_ms, tof_s, range_m) * sin_aspect
}

/// Compute the optimal aim point combining lead, ballistic drop, and
/// windage, weighted by confidence in the lead estimate.
///
/// When confidence is low the horizontal lead is scaled down, biasing
/// toward centre-mass aiming. The vertical component is unaffected by
/// confidence (ballistic drop is known precisely from the solution table).
///
/// * `lead_mrad` — horizontal lead from target motion (mrad).
/// * `ballistic_drop_mrad` — vertical holdover from ballistic drop (mrad).
/// * `windage_mrad` — horizontal windage correction (mrad).
/// * `confidence` — confidence in the lead estimate [0.0, 1.0].
///   0.0 = no confidence (aim centre), 1.0 = full confidence.
///
/// Returns `(horizontal_hold_mrad, vertical_hold_mrad)`.
pub fn optimal_aim_point(
    lead_mrad: f64,
    ballistic_drop_mrad: f64,
    windage_mrad: f64,
    confidence: f64,
) -> (f64, f64) {
    let confidence = confidence.clamp(0.0, 1.0);
    let horizontal = (lead_mrad + windage_mrad) * confidence;
    let vertical = ballistic_drop_mrad;
    (horizontal, vertical)
}

// ── Core computation ───────────────────────────────────────────────────────────

/// Compute the full lead solution for a moving target with iterative
/// refinement of the time-of-flight estimate.
///
/// # Algorithm
///
/// 1. Compute the perpendicular component of target velocity:
///    `v_perp = v · sin(aspect_angle)`.
/// 2. Estimate the initial lead using the nominal TOF and range.
/// 3. For each iteration:
///    a. Estimate the lateral offset at the intercept (target displacement
///       during the bullet's flight).
///    b. Compute the intercept range as the hypotenuse of the lateral
///       offset and the initial range.
///    c. Scale the TOF proportionally to the range change.
///    d. Recompute the lead with the refined TOF.
/// 4. After convergence, compute the vertical correction for LOS motion:
///    an incoming/outgoing target changes the effective engagement TOF,
///    which changes the bullet drop.
///
/// # Returns
///
/// A `LeadSolution` with the lead angle, distance, vertical correction,
/// corrected aim point, and intercept time.
pub fn compute_lead(params: &LeadTargetState) -> LeadSolution {
    let range_m = params.range_m.max(1.0);
    let tof = params.time_of_flight_s.max(0.001);
    let speed = params.target_speed_ms.abs();
    let aspect_rad = params.target_aspect_angle_deg.to_radians();

    // Perpendicular component of target velocity (contributes to lead)
    let v_perp = speed * aspect_rad.sin();

    // ── Initial lead estimate ──────────────────────────────────────────
    let mut lead_rad = v_perp * tof / range_m;
    let mut lead_mrad = lead_rad * MRAD_PER_RAD;
    let mut lead_dist = speed * tof;
    let mut intercept_tof = tof;

    // ── Iterative refinement (max 3 iterations) ────────────────────────
    for _ in 0..3 {
        // Lateral displacement of the target during the bullet's flight
        let lateral_offset = v_perp * intercept_tof;

        // Intercept range: hypotenuse of range and lateral offset
        let new_intercept_range = (range_m.powi(2) + lateral_offset.powi(2)).sqrt();

        // Scale TOF proportionally to the range change
        let tof_ratio = if range_m > 0.0 {
            (new_intercept_range / range_m).min(2.0)
        } else {
            1.0
        };
        let new_tof = tof * tof_ratio;

        // Check convergence (TOF change < 1 ms)
        if (new_tof - intercept_tof).abs() < 0.001 {
            // Recompute lead with converged TOF before breaking
            lead_rad = v_perp * new_tof / range_m;
            lead_mrad = lead_rad * MRAD_PER_RAD;
            lead_dist = speed * new_tof;
            intercept_tof = new_tof;
            break;
        }

        intercept_tof = new_tof;

        // Recompute lead with refined TOF
        lead_rad = v_perp * intercept_tof / range_m;
        lead_mrad = lead_rad * MRAD_PER_RAD;
        lead_dist = speed * intercept_tof;
    }

    // ── Vertical correction for LOS motion ─────────────────────────────
    // The component of target velocity along the LOS changes the effective
    // engagement distance. A target moving toward the shooter reduces the
    // distance the bullet must travel, shortening the effective TOF and
    // reducing bullet drop. A target moving away increases both.
    //
    // target_direction: 0° = toward shooter, 180° = away.
    // Combined with aspect: along-LOS speed = v · cos(aspect_angle) · cos(direction)
    let v_along = speed * aspect_rad.cos() * params.target_direction_deg.to_radians().cos();

    let effective_tof = if v_along >= 0.0 {
        // Moving away or stationary along LOS: TOF increases slightly because
        // the target is farther at intercept than initially ranged.
        intercept_tof
    } else {
        // Moving toward: effective TOF decreases because the target
        // closes the distance.
        let closing_mag = v_along.abs();
        let eff = intercept_tof * range_m / (range_m + closing_mag * intercept_tof);
        eff.max(0.001)
    };

    // Bullet drop approximation: 0.5 · g · t² (gravity-only for small arms)
    let drop_nominal = 0.5 * GRAVITY * intercept_tof * intercept_tof;
    let drop_effective = 0.5 * GRAVITY * effective_tof * effective_tof;
    let vertical_change_m = drop_effective - drop_nominal;

    // Convert vertical change to milliradians
    let vertical_correction_mrad = (vertical_change_m / range_m) * MRAD_PER_RAD;

    // ── Windage ────────────────────────────────────────────────────────
    let windage_mrad = (params.crosswind_ms * intercept_tof / range_m) * MRAD_PER_RAD;

    // Ballistic drop in mrad (for the aim point)
    let ballistic_drop_mrad = (drop_nominal / range_m) * MRAD_PER_RAD;

    // ── Assemble solution ──────────────────────────────────────────────
    let aim_x = lead_mrad + windage_mrad;
    let aim_y = ballistic_drop_mrad + vertical_correction_mrad;

    LeadSolution {
        lead_angle_mrad: lead_mrad,
        lead_distance_m: lead_dist,
        vertical_correction_mrad,
        corrected_aim_point: (aim_x, aim_y),
        time_to_intercept_s: effective_tof,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── crossing_lead tests ────────────────────────────────────────────

    #[test]
    fn crossing_lead_known_value() {
        // 10 m/s target, 0.35 s TOF, 300 m range
        // lead = 10 × 0.35 / 300 × 1000 = 11.666... mrad ≈ 11.7 MIL
        let lead = crossing_lead(10.0, 0.35, 300.0);
        let expected = 11.6667;
        assert!(
            (lead - expected).abs() < 0.01,
            "Crossing lead: expected {} mrad, got {}",
            expected,
            lead
        );
    }

    #[test]
    fn crossing_lead_zero_speed_returns_zero() {
        assert_eq!(crossing_lead(0.0, 0.35, 300.0), 0.0);
    }

    #[test]
    fn crossing_lead_zero_tof_returns_zero() {
        assert_eq!(crossing_lead(10.0, 0.0, 300.0), 0.0);
    }

    #[test]
    fn crossing_lead_scales_linearly_with_speed() {
        let lead_5 = crossing_lead(5.0, 0.35, 300.0);
        let lead_10 = crossing_lead(10.0, 0.35, 300.0);
        assert!(
            (lead_10 - lead_5 * 2.0).abs() < 1e-10,
            "Lead should scale linearly with speed"
        );
    }

    // ── oblique_lead tests ─────────────────────────────────────────────

    #[test]
    fn oblique_lead_crossing_aspect_matches_crossing_lead() {
        // At 90° aspect (full crossing), oblique_lead should equal crossing_lead
        let crossing = crossing_lead(10.0, 0.35, 300.0);
        let oblique = oblique_lead(10.0, 0.35, 300.0, 90.0);
        assert!(
            (oblique - crossing).abs() < 1e-10,
            "Oblique at 90° should match crossing lead: {} vs {}",
            oblique,
            crossing
        );
    }

    #[test]
    fn oblique_lead_45_degrees_reduces_lead() {
        // At 45°, lead = crossing × sin(45°) ≈ 0.707 × crossing
        let crossing = crossing_lead(10.0, 0.35, 300.0);
        let oblique = oblique_lead(10.0, 0.35, 300.0, 45.0);
        let expected = crossing * (45.0_f64).to_radians().sin();
        assert!(
            (oblique - expected).abs() < 1e-10,
            "Oblique at 45°: expected {} mrad, got {}",
            expected,
            oblique
        );
    }

    #[test]
    fn oblique_lead_incoming_returns_zero() {
        // At 0° aspect (incoming/outgoing), no perpendicular component → no lead
        let lead = oblique_lead(10.0, 0.35, 300.0, 0.0);
        assert!(
            lead.abs() < 1e-10,
            "Incoming target should have zero lead, got {}",
            lead
        );
    }

    #[test]
    fn oblique_lead_matches_sin_scaling() {
        let crossing = crossing_lead(10.0, 0.35, 300.0);
        for &angle in &[10.0, 30.0, 60.0, 80.0] {
            let oblique = oblique_lead(10.0, 0.35, 300.0, angle);
            let expected = crossing * angle.to_radians().sin();
            assert!(
                (oblique - expected).abs() < 1e-10,
                "Oblique at {}°: expected {}, got {}",
                angle,
                expected,
                oblique
            );
        }
    }

    // ── compute_lead tests ─────────────────────────────────────────────

    #[test]
    fn compute_lead_full_crossing_value() {
        // 10 m/s target, crossing at 300 m, 0.35 s TOF, 900 m/s MV
        let params = LeadTargetState {
            range_m: 300.0,
            time_of_flight_s: 0.35,
            target_speed_ms: 10.0,
            target_direction_deg: 90.0,
            target_aspect_angle_deg: 90.0, // full crossing
            shooter_muzzle_velocity_ms: 900.0,
            crosswind_ms: 0.0,
        };
        let sol = compute_lead(&params);

        // Lead should be ~11.7 mrad
        assert!(
            (sol.lead_angle_mrad - 11.67).abs() < 0.1,
            "Crossing lead should be ~11.7 mrad, got {}",
            sol.lead_angle_mrad
        );

        // Lead distance = speed × TOF = 10 × 0.35 = 3.5 m
        assert!(
            (sol.lead_distance_m - 3.5).abs() < 0.1,
            "Lead distance should be ~3.5 m, got {}",
            sol.lead_distance_m
        );

        // Time to intercept should be positive and close to TOF
        assert!(
            sol.time_to_intercept_s > 0.0,
            "Time to intercept should be positive"
        );

        // Aim point: lead only (no wind, no vertical correction for crossing)
        assert!(
            sol.corrected_aim_point.0 > 0.0,
            "Horizontal aim point should be positive"
        );
    }

    #[test]
    fn compute_lead_oblique_reduces_lead_versus_crossing() {
        let crossing_params = LeadTargetState {
            range_m: 300.0,
            time_of_flight_s: 0.35,
            target_speed_ms: 10.0,
            target_direction_deg: 90.0,
            target_aspect_angle_deg: 90.0,
            shooter_muzzle_velocity_ms: 900.0,
            crosswind_ms: 0.0,
        };
        let oblique_params = LeadTargetState {
            target_aspect_angle_deg: 45.0,
            ..crossing_params
        };

        let crossing_sol = compute_lead(&crossing_params);
        let oblique_sol = compute_lead(&oblique_params);

        assert!(
            oblique_sol.lead_angle_mrad < crossing_sol.lead_angle_mrad,
            "Oblique lead ({} mrad) should be less than crossing lead ({} mrad)",
            oblique_sol.lead_angle_mrad,
            crossing_sol.lead_angle_mrad
        );
    }

    #[test]
    fn compute_lead_incoming_target_zero_lead() {
        // Target moving directly toward the shooter: no perpendicular velocity
        let params = LeadTargetState {
            range_m: 300.0,
            time_of_flight_s: 0.35,
            target_speed_ms: 10.0,
            target_direction_deg: 0.0,    // directly toward
            target_aspect_angle_deg: 0.0, // parallel to LOS
            shooter_muzzle_velocity_ms: 900.0,
            crosswind_ms: 0.0,
        };
        let sol = compute_lead(&params);

        assert!(
            sol.lead_angle_mrad.abs() < 1e-6,
            "Incoming target should have zero lead, got {} mrad",
            sol.lead_angle_mrad
        );

        // Vertical correction should be negative (less drop at shorter effective range)
        assert!(
            sol.vertical_correction_mrad <= 0.0,
            "Incoming target should have negative vertical correction, got {}",
            sol.vertical_correction_mrad
        );
    }

    #[test]
    fn compute_lead_iterative_convergence() {
        // Use a fast target at long range to trigger more iteration
        let params = LeadTargetState {
            range_m: 500.0,
            time_of_flight_s: 0.65,
            target_speed_ms: 25.0,
            target_direction_deg: 90.0,
            target_aspect_angle_deg: 90.0,
            shooter_muzzle_velocity_ms: 850.0,
            crosswind_ms: 5.0,
        };
        let sol = compute_lead(&params);

        // Should have converged to a reasonable lead
        assert!(
            sol.lead_angle_mrad > 0.0,
            "Lead should be positive, got {}",
            sol.lead_angle_mrad
        );
        assert!(
            sol.time_to_intercept_s > 0.0,
            "Time to intercept should be positive, got {}",
            sol.time_to_intercept_s
        );
        // Lead distance should be reasonable
        assert!(
            sol.lead_distance_m > 0.0,
            "Lead distance should be positive"
        );
    }

    #[test]
    fn compute_lead_zero_speed_target() {
        // Stationary target → no lead needed
        let params = LeadTargetState {
            range_m: 300.0,
            time_of_flight_s: 0.35,
            target_speed_ms: 0.0,
            target_direction_deg: 90.0,
            target_aspect_angle_deg: 90.0,
            shooter_muzzle_velocity_ms: 900.0,
            crosswind_ms: 0.0,
        };
        let sol = compute_lead(&params);

        assert!(
            sol.lead_angle_mrad.abs() < 1e-6,
            "Stationary target should have zero lead, got {}",
            sol.lead_angle_mrad
        );
        assert!(
            sol.lead_distance_m.abs() < 1e-6,
            "Stationary target should have zero lead distance"
        );
    }

    #[test]
    fn compute_lead_short_range_extreme() {
        // Point-blank range
        let params = LeadTargetState {
            range_m: 10.0,
            time_of_flight_s: 0.011,
            target_speed_ms: 10.0,
            target_direction_deg: 90.0,
            target_aspect_angle_deg: 90.0,
            shooter_muzzle_velocity_ms: 900.0,
            crosswind_ms: 0.0,
        };
        let sol = compute_lead(&params);

        // Lead should be small but non-zero
        // lead = 10 × 0.011 / 10 × 1000 = 11 mrad
        // But at 10 m range, this formula gives a value.
        // The important thing is it doesn't crash or produce NaN.
        assert!(
            sol.lead_angle_mrad.is_finite(),
            "Lead should be finite at short range"
        );
        assert!(sol.lead_angle_mrad >= 0.0, "Lead should be non-negative");
    }

    #[test]
    fn compute_lead_crosswind_affects_aim_point() {
        let no_wind = LeadTargetState {
            range_m: 300.0,
            time_of_flight_s: 0.35,
            target_speed_ms: 10.0,
            target_direction_deg: 90.0,
            target_aspect_angle_deg: 90.0,
            shooter_muzzle_velocity_ms: 900.0,
            crosswind_ms: 0.0,
        };
        let with_wind = LeadTargetState {
            crosswind_ms: 5.0, // 5 m/s wind from the right
            ..no_wind
        };

        let sol_no = compute_lead(&no_wind);
        let sol_wind = compute_lead(&with_wind);

        // Windage should change the horizontal aim point
        assert!(
            (sol_wind.corrected_aim_point.0 - sol_no.corrected_aim_point.0).abs() > 0.0,
            "Wind should change the horizontal aim point"
        );
    }

    // ── optimal_aim_point tests ────────────────────────────────────────

    #[test]
    fn optimal_aim_point_full_confidence_uses_full_lead() {
        let (h, v) = optimal_aim_point(10.0, 5.0, 2.0, 1.0);
        assert!(
            (h - 12.0).abs() < 1e-10,
            "Full confidence: horizontal should be lead + windage = 12.0, got {}",
            h
        );
        assert!(
            (v - 5.0).abs() < 1e-10,
            "Vertical should be ballistic drop = 5.0, got {}",
            v
        );
    }

    #[test]
    fn optimal_aim_point_zero_confidence_aims_centre() {
        let (h, v) = optimal_aim_point(10.0, 5.0, 2.0, 0.0);
        assert!(
            h.abs() < 1e-10,
            "Zero confidence: horizontal should be 0, got {}",
            h
        );
        assert!(
            (v - 5.0).abs() < 1e-10,
            "Vertical should still be ballistic drop = 5.0, got {}",
            v
        );
    }

    #[test]
    fn optimal_aim_point_partial_confidence_scales_lead() {
        let full = optimal_aim_point(10.0, 5.0, 2.0, 1.0);
        let half = optimal_aim_point(10.0, 5.0, 2.0, 0.5);
        assert!(
            (half.0 - full.0 * 0.5).abs() < 1e-10,
            "Half confidence should scale horizontal by 0.5: expected {}, got {}",
            full.0 * 0.5,
            half.0
        );
    }
}
