// ABE - Spin Drift Direction Model
//
// Computes spin drift magnitude and direction for both right-hand and
// left-hand barrel twist. Right-hand twist (clockwise from breech, NATO
// standard) produces drift to the right in the Northern Hemisphere.
// Left-hand twist (counter-clockwise, some Soviet/Russian and 9mm
// pistol barrels) produces drift to the left.
//
// The Coriolis coupling factor captures the cross-term interaction
// between the bullet's precession axis (determined by twist direction)
// and the Earth's rotation vector.
//
// References:
//   - McCoy's Modern Exterior Ballistics (ch. 9 — Magnus effect / spin drift)
//   - Litz, Applied Ballistics for Long Range Shooting (ch. 8)
//   - Miller stability criterion (yaw-of-repose coupling)
//   - Carlucci & Jacobson, Ballistics (ch. 10)

#![allow(dead_code)]

/// Direction of rifling twist as viewed from the breech.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TwistDirection {
    /// Clockwise rotation (NATO standard, most bolt-action rifles).
    /// Bullet drifts right in the Northern Hemisphere.
    RightHand,
    /// Counter-clockwise rotation (some Soviet/Russian weapons,
    /// some 9mm pistol barrels like the CZ-75).
    /// Bullet drifts left in the Northern Hemisphere.
    LeftHand,
}

/// Parameters for the spin drift evaluation.
pub struct TwistDriftParams {
    /// Twist rate in revolutions per metre of barrel travel.
    /// e.g. 1:7" = 1/(7*0.0254) ≈ 5.62 rev/m
    pub twist_rate_rev_per_m: f64,
    /// Direction of the rifling twist.
    pub twist_direction: TwistDirection,
    /// Current projectile velocity in m/s.
    pub velocity_ms: f64,
    /// Projectile calibre in millimetres.
    pub caliber_mm: f64,
    /// Latitude in degrees. Sign determines hemisphere:
    /// positive = Northern, negative = Southern.
    pub latitude_deg: f64,
    /// Time of flight in seconds.
    pub time_of_flight_s: f64,
}

/// Result of the spin drift computation.
pub struct TwistDriftResult {
    /// Absolute magnitude of spin drift at the target range (metres).
    pub drift_magnitude_m: f64,
    /// Direction of drift in degrees from the line of fire.
    /// 0 = right, 180 = left, 90 = up, -90 = down.
    pub drift_direction_deg: f64,
    /// Signed lateral deflection. Positive = right for RH twist
    /// at the equator. Negative for LH twist (drifts left).
    pub lateral_deflection_m: f64,
    /// Dimensionless factor capturing the interaction between
    /// spin-drift precession and the Coriolis effect. Positive
    /// when both effects push in the same direction, negative
    /// when they oppose.
    pub coriolis_coupling_factor: f64,
}

/// Compute the spin drift coefficient based on velocity, calibre,
/// and twist rate. This is the dimensionless proportionality
/// constant used in the drift formula.
///
/// From McCoy: the Magnus effect scales as:
///   SD_coeff ∝ (π · d · n / v)²
/// where d = diameter, n = spin rate (rev/s), v = velocity.
pub fn twist_drift_coefficient(v_ms: f64, caliber_mm: f64, rate_rev_per_m: f64) -> f64 {
    if v_ms <= 0.0 || caliber_mm <= 0.0 || rate_rev_per_m <= 0.0 {
        return 0.0;
    }
    let d = caliber_mm / 1000.0; // convert to metres
    let spin_rate_hz = rate_rev_per_m * v_ms; // rev/s
                                              // Empirical coefficient from McCoy / Litz:
                                              //   SD_coeff = k_coeff · (π · d · n / v)² · (L / d)^(1/3)
                                              // For typical spitzer bullets L/d ≈ 4, so (L/d)^(1/3) ≈ 1.587
    let aspect_ratio_factor = 4.0_f64.cbrt(); // ≈ 1.587
    let k_coeff = 0.082; // empirical constant
    let arg = std::f64::consts::PI * d * spin_rate_hz / v_ms;
    k_coeff * arg * arg * aspect_ratio_factor
}

/// Evaluate the spin drift for the given parameters, returning
/// a signed lateral deflection, drift magnitude, direction, and
/// the Coriolis coupling factor.
///
/// The base drift formula (McCoy, simplified) is:
///   SD = SD_coeff · TOF · v · sin(latitude)
///
/// The sign is determined by the twist direction combined with
/// the hemisphere (latitude sign).
pub fn evaluate_twist_drift(params: &TwistDriftParams) -> TwistDriftResult {
    let sd_coeff = twist_drift_coefficient(
        params.velocity_ms,
        params.caliber_mm,
        params.twist_rate_rev_per_m,
    );

    let lat_rad = params.latitude_deg.to_radians();
    let sin_lat = lat_rad.sin();

    // Base drift distance: SD_coeff · TOF · v · sin(latitude)
    // The latitude dependence captures the Coriolis-aligned component.
    let base_drift = sd_coeff * params.time_of_flight_s * params.velocity_ms * sin_lat;

    // Twist direction sign: +1 for RH, -1 for LH
    let twist_sign = match params.twist_direction {
        TwistDirection::RightHand => 1.0,
        TwistDirection::LeftHand => -1.0,
    };

    // Hemisphere sign: +1 for Northern, -1 for Southern
    let hemi_sign = if sin_lat >= 0.0 { 1.0 } else { -1.0 };

    // Lateral deflection: magnitude is positive; direction from twist × hemisphere
    // RH twist + Northern hemisphere → positive (right)
    // LH twist + Northern hemisphere → negative (left)
    // Either twist + equatorial (latitude=0) → zero
    let lateral_deflection_m = base_drift * twist_sign;
    let drift_magnitude_m = base_drift.abs();

    // Drift direction in degrees:
    // RH in NH → 0° (right), LH in NH → 180° (left)
    // RH in SH → 180° (left in NH terms), LH in SH → 0° (right)
    let combined_sign = twist_sign * hemi_sign;
    let drift_direction_deg = if combined_sign >= 0.0 { 0.0 } else { 180.0 };

    // Coriolis coupling factor: how much the spin-drift precession axis
    // aligns with Earth's rotation. Positive when they reinforce.
    // Formulation: coupling = twist_sign · sin(latitude)
    // This means in the Northern Hemisphere with RH twist, the
    // precession adds to the Coriolis deflection (positive coupling).
    // LH twist in NH gives negative coupling (precession opposes Coriolis).
    let coriolis_coupling_factor = twist_sign * sin_lat;

    TwistDriftResult {
        drift_magnitude_m,
        drift_direction_deg,
        lateral_deflection_m,
        coriolis_coupling_factor,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_params(
        twist_rate_rev_per_m: f64,
        direction: TwistDirection,
        velocity_ms: f64,
        caliber_mm: f64,
        latitude_deg: f64,
        tof_s: f64,
    ) -> TwistDriftParams {
        TwistDriftParams {
            twist_rate_rev_per_m,
            twist_direction: direction,
            velocity_ms,
            caliber_mm,
            latitude_deg,
            time_of_flight_s: tof_s,
        }
    }

    // ── Direction tests ──────────────────────────────────────────────────

    #[test]
    fn right_vs_left_twist_opposite_direction() {
        // Same parameters, opposite twist directions
        let rh = evaluate_twist_drift(&make_params(
            5.62,
            TwistDirection::RightHand,
            930.0,
            5.56,
            45.0,
            1.5,
        ));
        let lh = evaluate_twist_drift(&make_params(
            5.62,
            TwistDirection::LeftHand,
            930.0,
            5.56,
            45.0,
            1.5,
        ));
        // Same magnitude
        assert!(
            (rh.drift_magnitude_m - lh.drift_magnitude_m).abs() < 1e-12,
            "magnitude should be equal: rh={}, lh={}",
            rh.drift_magnitude_m,
            lh.drift_magnitude_m
        );
        // Opposite signs
        assert!(
            rh.lateral_deflection_m > 0.0,
            "RH twist should drift right (positive): {}",
            rh.lateral_deflection_m
        );
        assert!(
            lh.lateral_deflection_m < 0.0,
            "LH twist should drift left (negative): {}",
            lh.lateral_deflection_m
        );
        // Opposite directions
        assert_eq!(rh.drift_direction_deg, 0.0);
        assert_eq!(lh.drift_direction_deg, 180.0);
    }

    #[test]
    fn zero_twist_means_no_drift() {
        let result = evaluate_twist_drift(&make_params(
            0.0,
            TwistDirection::RightHand,
            930.0,
            5.56,
            45.0,
            1.5,
        ));
        assert!(
            result.drift_magnitude_m.abs() < 1e-12,
            "zero twist should give zero drift: {}",
            result.drift_magnitude_m
        );
        assert!(
            result.lateral_deflection_m.abs() < 1e-12,
            "zero twist should give zero lateral: {}",
            result.lateral_deflection_m
        );
    }

    // ── Latitude sign flips coupling ─────────────────────────────────────

    #[test]
    fn latitude_sign_flips_coupling() {
        let nh = evaluate_twist_drift(&make_params(
            5.62,
            TwistDirection::RightHand,
            930.0,
            5.56,
            45.0,
            1.5,
        ));
        let sh = evaluate_twist_drift(&make_params(
            5.62,
            TwistDirection::RightHand,
            930.0,
            5.56,
            -45.0,
            1.5,
        ));
        // Coriolis coupling should have opposite signs
        assert!(
            nh.coriolis_coupling_factor > 0.0,
            "NH should have positive coupling: {}",
            nh.coriolis_coupling_factor
        );
        assert!(
            sh.coriolis_coupling_factor < 0.0,
            "SH should have negative coupling: {}",
            sh.coriolis_coupling_factor
        );
        // Drift direction flips in SH
        assert_eq!(nh.drift_direction_deg, 0.0);
        assert_eq!(sh.drift_direction_deg, 180.0);
    }

    #[test]
    fn equatorial_latitude_gives_no_direction_bias() {
        // At the equator, sin(lat) ≈ 0 → no drift from the latitude term
        let result = evaluate_twist_drift(&make_params(
            5.62,
            TwistDirection::RightHand,
            930.0,
            5.56,
            0.0,
            1.5,
        ));
        assert!(
            result.lateral_deflection_m.abs() < 1e-12,
            "equator should give negligible drift: {}",
            result.lateral_deflection_m
        );
        assert!(
            result.coriolis_coupling_factor.abs() < 1e-12,
            "equator coupling should be zero: {}",
            result.coriolis_coupling_factor
        );
    }

    // ── Typical values ───────────────────────────────────────────────────

    #[test]
    fn typical_556_nato_rh_twist() {
        // M4A1, 1:7" twist, M855 ball at 930 m/s, 45°N, 500 m range ~1.0s TOF
        // 1:7" → 1/(7*0.0254) ≈ 5.62 rev/m
        let result = evaluate_twist_drift(&make_params(
            5.62,
            TwistDirection::RightHand,
            930.0,
            5.56,
            45.0,
            1.0,
        ));
        // Should drift right (positive) with small but non-zero magnitude
        assert!(
            result.lateral_deflection_m > 0.0,
            "RH twist NH should drift right: {}",
            result.lateral_deflection_m
        );
        assert!(
            result.drift_magnitude_m > 0.001,
            "drift should be measurable: {} m",
            result.drift_magnitude_m
        );
        assert_eq!(result.drift_direction_deg, 0.0);
    }

    #[test]
    fn typical_ak74_rh_twist_545mm() {
        // AK-74, 1:25.6" twist (≈ 1.54 rev/m), 5.45×39 mm at ~900 m/s
        // Slower twist → less spin drift
        let slow_twist = evaluate_twist_drift(&make_params(
            1.54,
            TwistDirection::RightHand,
            900.0,
            5.45,
            45.0,
            1.2,
        ));
        let fast_twist = evaluate_twist_drift(&make_params(
            5.62,
            TwistDirection::RightHand,
            900.0,
            5.45,
            45.0,
            1.2,
        ));
        assert!(
            fast_twist.drift_magnitude_m > slow_twist.drift_magnitude_m,
            "faster twist should produce more drift: fast={}, slow={}",
            fast_twist.drift_magnitude_m,
            slow_twist.drift_magnitude_m
        );
    }

    #[test]
    fn typical_9mm_lh_twist_pistol() {
        // Some 9 mm pistol barrels use LH twist (e.g. CZ-75, some S&W).
        // 1:9.84" twist ≈ 4.0 rev/m, 9mm at ~360 m/s, 100 m
        let result = evaluate_twist_drift(&make_params(
            4.0,
            TwistDirection::LeftHand,
            360.0,
            9.0,
            45.0,
            0.3,
        ));
        // LH twist in NH → drift left (negative)
        assert!(
            result.lateral_deflection_m < 0.0,
            "LH twist should drift left: {}",
            result.lateral_deflection_m
        );
        assert_eq!(result.drift_direction_deg, 180.0);
    }

    #[test]
    fn longer_tof_increases_drift() {
        let short = evaluate_twist_drift(&make_params(
            5.62,
            TwistDirection::RightHand,
            930.0,
            5.56,
            45.0,
            0.5,
        ));
        let long = evaluate_twist_drift(&make_params(
            5.62,
            TwistDirection::RightHand,
            930.0,
            5.56,
            45.0,
            2.0,
        ));
        assert!(
            long.drift_magnitude_m > short.drift_magnitude_m,
            "longer TOF should produce more drift: long={}, short={}",
            long.drift_magnitude_m,
            short.drift_magnitude_m
        );
    }

    #[test]
    fn southern_hemisphere_rh_twist_reverses() {
        // RH twist in the Southern Hemisphere → drifts left
        let result = evaluate_twist_drift(&make_params(
            5.62,
            TwistDirection::RightHand,
            930.0,
            5.56,
            -30.0,
            1.5,
        ));
        assert!(
            result.lateral_deflection_m < 0.0,
            "RH twist in SH should drift left: {}",
            result.lateral_deflection_m
        );
        assert_eq!(
            result.drift_direction_deg, 180.0,
            "direction should be 180° (left)"
        );
    }

    #[test]
    fn south_hemisphere_lh_twist_drifts_right() {
        // LH twist in the Southern Hemisphere → drifts right
        let result = evaluate_twist_drift(&make_params(
            5.62,
            TwistDirection::LeftHand,
            930.0,
            5.56,
            -30.0,
            1.5,
        ));
        assert!(
            result.lateral_deflection_m > 0.0,
            "LH twist in SH should drift right: {}",
            result.lateral_deflection_m
        );
        assert_eq!(
            result.drift_direction_deg, 0.0,
            "direction should be 0° (right)"
        );
    }

    // ── Coefficient tests ────────────────────────────────────────────────

    #[test]
    fn twist_drift_coefficient_zero_velocity() {
        assert_eq!(twist_drift_coefficient(0.0, 5.56, 5.62), 0.0);
    }

    #[test]
    fn twist_drift_coefficient_increases_with_twist() {
        let slow = twist_drift_coefficient(930.0, 5.56, 1.54);
        let fast = twist_drift_coefficient(930.0, 5.56, 5.62);
        assert!(
            fast > slow,
            "faster twist → larger coefficient: fast={}, slow={}",
            fast,
            slow
        );
    }
}
