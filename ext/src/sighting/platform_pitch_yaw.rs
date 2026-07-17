// ABE - Platform Pitch & Yaw / Shooter Angular Motion Model
//
// Models the effect of shooter or platform angular motion during the
// projectile's time-of-flight on the point of impact.  When firing
// from a moving platform (vehicle, aircraft, ship, or unsupported
// standing/kneeling), the platform's pitch, yaw, and roll rates at
// the moment of firing impart a transverse velocity to the bullet via
// the barrel tip's rotation about the platform's pivot point.
//
// Physics summary:
//   The barrel tip rotates about the platform's centre of rotation
//   (pivot) at the platform's angular rate(s).  The tangential velocity
//   of the barrel tip is:
//     v_transverse = angular_rate × pivot_to_muzzle_distance
//   This velocity is inherited by the projectile at muzzle exit,
//   adding to the intended aim direction.  Over the projectile's
//   time-of-flight this transverse velocity produces an impact-point
//   displacement:
//     Δ = v_transverse × TOF = angular_rate × pivot_distance × TOF
//
//   Additionally, the platform's linear velocity (forward and lateral)
//   is also inherited by the bullet, producing lead/lag offsets when
//   firing at an angle to the platform's heading.
//
// Sign conventions:
//   Pitch rate: positive = nose UP  → barrel tip moves UP → impact HIGHER
//   Yaw rate:   positive = nose RIGHT → barrel tip moves RIGHT → impact RIGHT
//   Roll rate:  positive = roll RIGHT
//   Azimuth:    firing direction relative to platform heading,
//               0° = straight ahead, +90° = right, -90° = left
//
// References:
//   - Carlucci & Jacobson, "Ballistics: Theory and Design of Guns
//     and Ammunition" (3rd ed.), CRC Press, 2018.
//   - NATO STANAG 4355 (modified point-mass trajectory model)
//   - US Army AMC Pamphlet 706-242, "Engineering Design Handbook:
//     Ballistics Series"

use std::f64::consts::PI;

// ── Physical constants ──────────────────────────────────────────────────────────

/// Standard acceleration due to gravity (m/s²).
const G: f64 = 9.806_65;

/// Degrees to radians conversion factor.
const DEG_TO_RAD: f64 = PI / 180.0;

/// Estimated distance from the platform's pitch/yaw rotation centre to
/// the muzzle position, by platform type.  These are representative
/// values; the actual distance depends on the specific platform geometry.
const PIVOT_GROUND_VEHICLE_M: f64 = 2.5;
const PIVOT_WATERCRAFT_M: f64 = 5.0;
const PIVOT_AIRCRAFT_M: f64 = 4.0;
const PIVOT_HUMAN_STANDING_M: f64 = 1.5;
const PIVOT_HUMAN_KNEELING_M: f64 = 1.0;
const PIVOT_HUMAN_PRONE_M: f64 = 0.3;
const PIVOT_STATIONARY_M: f64 = 0.0;

/// Sea-state pitch-rate lookup coefficients.
/// pitch_rate_deg_per_s = A + B * ln(sea_state + 1)
const SEA_PITCH_A: f64 = 0.2;
const SEA_PITCH_B: f64 = 1.8;

/// Sea-state roll-rate lookup coefficients.
/// roll_rate_deg_per_s = A + B * ln(sea_state + 1)
const SEA_ROLL_A: f64 = 0.3;
const SEA_ROLL_B: f64 = 3.0;

/// Sea-state yaw-rate lookup coefficients (smaller than roll).
/// yaw_rate_deg_per_s = A + B * ln(sea_state + 1)
const SEA_YAW_A: f64 = 0.1;
const SEA_YAW_B: f64 = 0.6;

// ── Platform type ────────────────────────────────────────────────────────────────

/// Classification of the firing platform's dynamic state.
///
/// Determines default pivot-to-muzzle distance and typical angular
/// rate envelopes for the motion estimation functions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlatformType {
    /// Ground vehicle in motion.  Pitch from terrain undulation,
    /// yaw from steering.  `speed_kmh` is the vehicle's forward speed.
    GroundVehicle { speed_kmh: f64 },
    /// Watercraft (ship, boat).  Wave-induced motion dominated by
    /// sea state (0–9 Beaufort scale).  Higher sea states produce
    /// larger pitch, roll, and yaw rates.
    Watercraft { sea_state: i32 },
    /// Aircraft in flight.  `speed_kmh` is true airspeed,
    /// `bank_angle_deg` is the steady-state bank angle (0 = wings level).
    Aircraft { speed_kmh: f64, bank_angle_deg: f64 },
    /// Human shooter standing unsupported.  Natural body sway from
    /// breathing and posture adjustment.
    HumanStanding,
    /// Human shooter prone (lying down).  Very stable, minimal motion.
    HumanProne,
    /// Human shooter kneeling.  Moderately stable.
    HumanKneeling,
    /// Fixed platform (static tripod, bench rest).  No motion.
    Stationary,
}

// ── Input / output structs ──────────────────────────────────────────────────────

/// All parameters required to evaluate platform-motion effects on the
/// projectile's impact point.
///
/// Most fields describe the platform state at the instant of firing.
/// `time_of_flight_s` and `range_m` describe the projectile trajectory.
#[derive(Debug, Clone, Copy)]
pub struct PlatformMotionParams {
    /// Platform classification (determines pivot-distance estimate
    /// and rate envelopes).
    pub platform: PlatformType,

    /// Platform pitch rate at the muzzle instant.
    /// Positive = nose UP → barrel tip rising → impact goes HIGH.
    pub pitch_rate_deg_per_s: f64,

    /// Platform yaw rate at the muzzle instant.
    /// Positive = nose RIGHT → barrel tip moving right → impact goes RIGHT.
    pub yaw_rate_deg_per_s: f64,

    /// Platform roll rate at the muzzle instant.
    /// Positive = roll RIGHT.  Primarily affects aircraft where roll
    /// couples into yaw via the turn rate.
    pub roll_rate_deg_per_s: f64,

    /// Platform forward speed in the direction of its heading (m/s).
    pub forward_speed_ms: f64,

    /// Platform lateral (sideways) speed perpendicular to heading (m/s).
    /// Positive = moving to the right of the heading direction.
    pub lateral_speed_ms: f64,

    /// Projectile time-of-flight from muzzle to target (s).
    pub time_of_flight_s: f64,

    /// Slant range from muzzle to target (m).
    pub range_m: f64,

    /// Firing direction relative to platform heading (degrees).
    /// 0° = firing straight ahead in the heading direction.
    /// +90° = firing to the right, -90° (or 270°) = firing to the left.
    /// 180° = firing rearward.
    pub azimuth_deg: f64,
}

/// Result of evaluating platform-motion effects on the impact point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlatformMotionResult {
    /// Vertical displacement at the target from platform pitch rate (m).
    /// Positive = impact HIGHER than aim point.
    pub pitch_displacement_m: f64,

    /// Horizontal displacement at the target from platform yaw rate (m).
    /// Positive = impact to the RIGHT of aim point.
    pub yaw_displacement_m: f64,

    /// Combined offset magnitude from pitch + yaw (m).
    pub combined_offset_m: f64,

    /// Lateral offset from platform forward motion coupled with
    /// non-zero firing azimuth (m).  Positive = to the right.
    pub forward_motion_offset_m: f64,

    /// Lateral offset from platform lateral (sideways) motion (m).
    /// Positive = to the right.
    pub lateral_motion_offset_m: f64,

    /// Magnitude of the angular velocity vector at the muzzle (rad/s).
    /// This is the resultant pitch+yaw angular rate the bullet
    /// experiences as transverse velocity at the barrel tip.
    pub inherited_angular_velocity_rad_s: f64,

    /// Aircraft bank-induced horizontal drift (m).
    /// In a coordinated turn, the rotated gravity vector creates a
    /// lateral acceleration component during the projectile's flight.
    /// Zero for non-aircraft platforms.
    pub bank_drift_m: f64,
}

impl Default for PlatformMotionResult {
    fn default() -> Self {
        Self {
            pitch_displacement_m: 0.0,
            yaw_displacement_m: 0.0,
            combined_offset_m: 0.0,
            forward_motion_offset_m: 0.0,
            lateral_motion_offset_m: 0.0,
            inherited_angular_velocity_rad_s: 0.0,
            bank_drift_m: 0.0,
        }
    }
}

// ── Pivot distance estimation ───────────────────────────────────────────────────

/// Estimate the distance from the platform's rotation centre (pivot)
/// to the muzzle position, in metres.
///
/// This distance determines the tangential velocity the barrel tip
/// acquires from a given angular rate:  v = ω × r.
fn pivot_distance(platform: PlatformType) -> f64 {
    match platform {
        PlatformType::GroundVehicle { .. } => PIVOT_GROUND_VEHICLE_M,
        PlatformType::Watercraft { .. } => PIVOT_WATERCRAFT_M,
        PlatformType::Aircraft { .. } => PIVOT_AIRCRAFT_M,
        PlatformType::HumanStanding => PIVOT_HUMAN_STANDING_M,
        PlatformType::HumanKneeling => PIVOT_HUMAN_KNEELING_M,
        PlatformType::HumanProne => PIVOT_HUMAN_PRONE_M,
        PlatformType::Stationary => PIVOT_STATIONARY_M,
    }
}

// ── Sea-state rate estimators ───────────────────────────────────────────────────

/// Estimate typical pitch rate (°/s) for a given Beaufort sea state (0–9).
///
/// Scaling: logarithmic with sea state.  Calm (0) ≈ 0.2 °/s,
/// rough (5) ≈ 3.1 °/s, phenomenal (9) ≈ 4.3 °/s.
fn sea_state_pitch_rate(sea_state: i32) -> f64 {
    let ss = sea_state.clamp(0, 9) as f64;
    SEA_PITCH_A + SEA_PITCH_B * (ss + 1.0).ln()
}

/// Estimate typical roll rate (°/s) for a given Beaufort sea state (0–9).
///
/// Roll is typically larger than pitch for the same sea state.
/// Rough (5) ≈ 5.2 °/s, phenomenal (9) ≈ 7.2 °/s.
fn sea_state_roll_rate(sea_state: i32) -> f64 {
    let ss = sea_state.clamp(0, 9) as f64;
    SEA_ROLL_A + SEA_ROLL_B * (ss + 1.0).ln()
}

/// Estimate typical yaw rate (°/s) for a given Beaufort sea state (0–9).
///
/// Yaw from wave action is smaller than pitch or roll.
fn sea_state_yaw_rate(sea_state: i32) -> f64 {
    let ss = sea_state.clamp(0, 9) as f64;
    SEA_YAW_A + SEA_YAW_B * (ss + 1.0).ln()
}

// ── Core evaluation ─────────────────────────────────────────────────────────────

/// Evaluate all platform-motion effects on the projectile's impact point.
///
/// Computes the impact-point displacement caused by:
///
/// 1. **Pitch-rate displacement** — the barrel tip's vertical tangential
///    velocity from platform pitch angular velocity, integrated over TOF.
///
/// 2. **Yaw-rate displacement** — the barrel tip's horizontal tangential
///    velocity from platform yaw angular velocity, integrated over TOF.
///
/// 3. **Forward-motion offset** — when firing at an angle to the
///    platform's heading (azimuth ≠ 0°), the inherited forward velocity
///    of the platform contributes a lateral component perpendicular to
///    the bore line.
///
/// 4. **Lateral-motion offset** — the platform's sideways velocity
///    contribution perpendicular to the bore line.
///
/// 5. **Aircraft bank drift** — for aircraft in a coordinated turn, the
///    rotated effective gravity vector produces a small lateral
///    acceleration during the projectile's flight.
///
/// # Arguments
/// * `params` — platform state and trajectory parameters.
///
/// # Returns
/// A [`PlatformMotionResult`] with each displacement component in
/// metres, the combined offset magnitude, and the inherited angular
/// velocity magnitude.
///
/// # Determinism
/// All calculations are deterministic.  Same inputs always produce the
/// same outputs.
pub fn evaluate_platform_motion(params: &PlatformMotionParams) -> PlatformMotionResult {
    // Guard: zero TOF or range → no displacement.
    if params.time_of_flight_s <= 0.0 || params.range_m <= 0.0 {
        return PlatformMotionResult::default();
    }

    // ── Convenience conversions ────────────────────────────────────
    let pitch_rate_rad = params.pitch_rate_deg_per_s * DEG_TO_RAD;
    let yaw_rate_rad = params.yaw_rate_deg_per_s * DEG_TO_RAD;
    let az_rad = params.azimuth_deg * DEG_TO_RAD;
    let tof = params.time_of_flight_s;
    let r = pivot_distance(params.platform);

    // ── 1. Angular-rate displacement ────────────────────────────────
    // v_transverse = ω × r
    // Δ = v_transverse × TOF = ω × r × TOF
    //
    // Sign conventions:
    //   Positive pitch rate (nose UP) → barrel tip moves UP
    //     → upward transverse velocity → impact goes HIGH (+Δ)
    //   Positive yaw rate (nose RIGHT) → barrel tip moves RIGHT
    //     → rightward transverse velocity → impact goes RIGHT (+Δ)
    let pitch_displacement_m = pitch_rate_rad * r * tof;
    let yaw_displacement_m = yaw_rate_rad * r * tof;

    // Combined angular-offset magnitude (root-sum-square of pitch & yaw).
    let combined_offset_m = (pitch_displacement_m.powi(2) + yaw_displacement_m.powi(2)).sqrt();

    // Magnitude of the total angular velocity vector projected onto
    // the pitch–yaw plane.
    let inherited_angular_velocity_rad_s = (pitch_rate_rad.powi(2) + yaw_rate_rad.powi(2)).sqrt();

    // ── 2. Linear motion offsets ───────────────────────────────────
    // The platform's forward velocity contributes a component
    // perpendicular to the bore line when firing at a non-zero azimuth.
    //
    //   lateral_component = forward_speed × sin(azimuth)
    //
    // This lateral velocity is inherited by the bullet and integrated
    // over TOF.
    //
    //   Δ_forward = forward_speed × sin(azimuth) × TOF
    let forward_motion_offset_m = params.forward_speed_ms * az_rad.sin() * tof;

    // The platform's lateral (sideways) velocity contributes directly
    // perpendicular to the bore line (no azimuth projection needed;
    // it is already referenced to the platform's lateral axis, and the
    // bore line is defined relative to the platform's forward axis).
    //
    //   Δ_lateral = lateral_speed × TOF
    let lateral_motion_offset_m = params.lateral_speed_ms * tof;

    // ── 3. Aircraft bank drift ──────────────────────────────────────
    // For a platform in a coordinated turn (banked aircraft), the
    // effective gravity vector in the aircraft's frame has a horizontal
    // component:
    //   g_horizontal = g × sin(bank_angle)
    //
    // This horizontal acceleration acts on the projectile during its
    // flight, producing a lateral drift:
    //   Δ_bank = ½ × g × sin(bank_angle) × TOF²
    //
    // The drift direction is toward the lower wing (the direction of
    // the bank).  Positive bank angle (right wing down) → rightward
    // drift.
    let bank_drift_m = if let PlatformType::Aircraft { bank_angle_deg, .. } = params.platform {
        let bank_rad = bank_angle_deg * DEG_TO_RAD;
        0.5 * G * bank_rad.sin() * tof.powi(2)
    } else {
        0.0
    };

    PlatformMotionResult {
        pitch_displacement_m,
        yaw_displacement_m,
        combined_offset_m,
        forward_motion_offset_m,
        lateral_motion_offset_m,
        inherited_angular_velocity_rad_s,
        bank_drift_m,
    }
}

// ── Platform motion envelope ────────────────────────────────────────────────────

/// Estimate typical pitch, yaw, and roll rates (°/s) for a given
/// platform type.
///
/// These rates represent **typical** angular motion magnitudes.  Actual
/// rates depend on specific platform dynamics, speed, sea state, or
/// shooter physiology.  The returned values are suitable as defaults
/// when the exact rates are unknown.
///
/// # Returns
/// `(pitch_rate_deg_per_s, yaw_rate_deg_per_s, roll_rate_deg_per_s)`
///
/// | Platform | Pitch (°/s) | Yaw (°/s) | Roll (°/s) |
/// |---|---|---|---|
/// | GroundVehicle (30 km/h) | 1.5 | 2.0 | 0.5 |
/// | GroundVehicle (80 km/h) | 3.0 | 5.0 | 1.0 |
/// | Watercraft (sea state 3) | 1.5 | 0.5 | 2.5 |
/// | Watercraft (sea state 5) | 3.1 | 1.1 | 5.2 |
/// | Watercraft (sea state 7) | 3.8 | 1.3 | 6.3 |
/// | Aircraft (manoeuvring) | 20.0 | 15.0 | 10.0 |
/// | HumanStanding | 1.5 | 1.5 | 0.5 |
/// | HumanProne | 0.3 | 0.2 | 0.1 |
/// | HumanKneeling | 0.8 | 0.6 | 0.3 |
/// | Stationary | 0.0 | 0.0 | 0.0 |
pub fn platform_motion_envelope(platform: PlatformType) -> (f64, f64, f64) {
    match platform {
        PlatformType::GroundVehicle { speed_kmh } => {
            // Pitch from terrain scales with speed: higher speed → more
            // aggressive suspension response to bumps.
            let speed_factor = (speed_kmh / 50.0).clamp(0.5, 2.0);
            let pitch = 1.5 * speed_factor;
            let yaw = {
                // Steering-induced yaw: higher speed → gentler steering →
                // slightly lower yaw rate in practice, but we model
                // typical evasive-manoeuvre rates.
                if speed_kmh < 20.0 {
                    3.0 // low-speed turning (e.g. urban)
                } else if speed_kmh < 60.0 {
                    2.0 // moderate speed
                } else {
                    1.5 // high speed, gentle curves
                }
            };
            let roll = 0.5 * speed_factor;
            (pitch, yaw, roll)
        }
        PlatformType::Watercraft { sea_state } => {
            let pitch = sea_state_pitch_rate(sea_state);
            let yaw = sea_state_yaw_rate(sea_state);
            let roll = sea_state_roll_rate(sea_state);
            (pitch, yaw, roll)
        }
        PlatformType::Aircraft { speed_kmh, .. } => {
            // Manoeuvring aircraft: faster = tighter possible turns.
            // At low speeds the aircraft must bank harder to turn,
            // producing higher pitch/yaw rates per manoeuvre.
            let speed_factor = (300.0 / speed_kmh.max(50.0)).clamp(0.5, 2.0);
            let pitch = 20.0 * speed_factor;
            let yaw = 15.0 * speed_factor;
            let roll = 10.0 * speed_factor;
            (pitch, yaw, roll)
        }
        PlatformType::HumanStanding => (1.5, 1.5, 0.5),
        PlatformType::HumanProne => (0.3, 0.2, 0.1),
        PlatformType::HumanKneeling => (0.8, 0.6, 0.3),
        PlatformType::Stationary => (0.0, 0.0, 0.0),
    }
}

/// Estimate typical forward speed (m/s) for a given platform type.
///
/// Returns 0 for stationary or human platforms.  For vehicles and
/// aircraft this is the speed value embedded in the enum variant.
fn platform_forward_speed(platform: PlatformType) -> f64 {
    match platform {
        PlatformType::GroundVehicle { speed_kmh } => speed_kmh / 3.6,
        PlatformType::Watercraft { .. } => {
            // Ships/boats typically move at 5–15 m/s (10–30 kts).
            10.0
        }
        PlatformType::Aircraft { speed_kmh, .. } => speed_kmh / 3.6,
        PlatformType::HumanStanding
        | PlatformType::HumanProne
        | PlatformType::HumanKneeling
        | PlatformType::Stationary => 0.0,
    }
}

/// Estimate typical lateral speed (m/s) for a given platform type.
///
/// Returns 0 for most platforms; drifting watercraft may have small
/// lateral drift.
fn platform_lateral_speed(platform: PlatformType) -> f64 {
    match platform {
        PlatformType::Watercraft { sea_state } => {
            // Lateral drift from wind/current, scales with sea state.
            0.5 * (sea_state.clamp(0, 9) as f64)
        }
        PlatformType::Aircraft { .. } => {
            // Side-slip during crosswind landing / evasive manoeuvres;
            // usually small.
            1.0
        }
        _ => 0.0,
    }
}

// ── Convenience combined offset ─────────────────────────────────────────────────

/// Compute the total impact-point offset from all motion sources.
///
/// Returns `(vertical_m, horizontal_m)` where:
/// * `vertical_m`   — total vertical displacement, combining pitch
///   displacement and any vertical component from aircraft bank
///   (positive = higher than aim point).
/// * `horizontal_m` — total horizontal displacement, combining yaw
///   displacement, forward-motion offset, lateral-motion offset, and
///   bank drift (positive = to the right of aim point).
///
/// For a quick single-call interface when the full breakdown is not
/// needed.
pub fn total_motion_offset(params: &PlatformMotionParams) -> (f64, f64) {
    let result = evaluate_platform_motion(params);

    // Vertical: pitch displacement.
    // (Bank drift is horizontal, not vertical.)
    let vertical_m = result.pitch_displacement_m;

    // Horizontal: yaw displacement + forward-motion lateral +
    // lateral-motion + bank drift.
    let horizontal_m = result.yaw_displacement_m
        + result.forward_motion_offset_m
        + result.lateral_motion_offset_m
        + result.bank_drift_m;

    (vertical_m, horizontal_m)
}

/// Build a fully-populated [`PlatformMotionParams`] from a platform
/// type and trajectory parameters.
///
/// Fills in the angular rates, forward speed, and lateral speed from
/// the platform's typical motion envelope.  This is useful for quick
/// scenario evaluation without manually specifying every rate.
///
/// # Arguments
/// * `platform` — the platform type (provides rates + speeds).
/// * `azimuth_deg` — firing direction relative to heading.
/// * `time_of_flight_s` — projectile time of flight.
/// * `range_m` — slant range to target.
pub fn params_from_platform(
    platform: PlatformType,
    azimuth_deg: f64,
    time_of_flight_s: f64,
    range_m: f64,
) -> PlatformMotionParams {
    let (pitch_rate, yaw_rate, roll_rate) = platform_motion_envelope(platform);
    let forward_speed = platform_forward_speed(platform);
    let lateral_speed = platform_lateral_speed(platform);

    PlatformMotionParams {
        platform,
        pitch_rate_deg_per_s: pitch_rate,
        yaw_rate_deg_per_s: yaw_rate,
        roll_rate_deg_per_s: roll_rate,
        forward_speed_ms: forward_speed,
        lateral_speed_ms: lateral_speed,
        time_of_flight_s,
        range_m,
        azimuth_deg,
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a default PlatformMotionParams with sensible
    /// trajectory values.  The caller overrides specific fields.
    fn test_params() -> PlatformMotionParams {
        PlatformMotionParams {
            platform: PlatformType::Stationary,
            pitch_rate_deg_per_s: 0.0,
            yaw_rate_deg_per_s: 0.0,
            roll_rate_deg_per_s: 0.0,
            forward_speed_ms: 0.0,
            lateral_speed_ms: 0.0,
            time_of_flight_s: 1.5,
            range_m: 500.0,
            azimuth_deg: 0.0,
        }
    }

    // ── Stationary platform ─────────────────────────────────────────

    #[test]
    fn stationary_platform_zero_offset() {
        // A stationary platform with no angular or linear motion
        // should produce zero displacement everywhere.
        let params = test_params();
        let r = evaluate_platform_motion(&params);
        assert_eq!(r.pitch_displacement_m, 0.0);
        assert_eq!(r.yaw_displacement_m, 0.0);
        assert_eq!(r.combined_offset_m, 0.0);
        assert_eq!(r.forward_motion_offset_m, 0.0);
        assert_eq!(r.lateral_motion_offset_m, 0.0);
        assert_eq!(r.inherited_angular_velocity_rad_s, 0.0);
        assert_eq!(r.bank_drift_m, 0.0);

        let (v, h) = total_motion_offset(&params);
        assert_eq!(v, 0.0);
        assert_eq!(h, 0.0);
    }

    #[test]
    fn stationary_explicit_type_zero_rates() {
        // Even with Stationary type, specifying rates should still
        // produce offset because the pivot distance for Stationary is
        // zero.  This checks that the pivot-distance lookup works.
        let mut p = test_params();
        p.platform = PlatformType::Stationary;
        p.pitch_rate_deg_per_s = 10.0;
        let r = evaluate_platform_motion(&p);
        // Pivot distance for Stationary = 0 → no displacement despite
        // having a pitch rate.
        assert_eq!(r.pitch_displacement_m, 0.0);
        assert_eq!(r.inherited_angular_velocity_rad_s, (10.0 * DEG_TO_RAD));
    }

    // ── Ground vehicle ──────────────────────────────────────────────

    #[test]
    fn ground_vehicle_pitch_displacement() {
        let mut p = test_params();
        p.platform = PlatformType::GroundVehicle { speed_kmh: 50.0 };
        p.pitch_rate_deg_per_s = 2.0; // 2 °/s terrain pitch
                                      // Pivot = 2.5 m, TOF = 1.5 s
                                      // Expected: (2° × π/180) × 2.5 × 1.5 = 0.1309 m
        let r = evaluate_platform_motion(&p);
        let expected = 2.0 * DEG_TO_RAD * 2.5 * 1.5;
        assert!(
            (r.pitch_displacement_m - expected).abs() < 1e-10,
            "pitch disp: expected {:.6} m, got {:.6} m",
            expected,
            r.pitch_displacement_m
        );
        assert_eq!(r.yaw_displacement_m, 0.0);
    }

    #[test]
    fn ground_vehicle_yaw_displacement() {
        let mut p = test_params();
        p.platform = PlatformType::GroundVehicle { speed_kmh: 50.0 };
        p.yaw_rate_deg_per_s = 3.0; // 3 °/s steering yaw
                                    // Pivot = 2.5 m, TOF = 1.5 s
                                    // Expected: (3° × π/180) × 2.5 × 1.5 = 0.19635 m
        let r = evaluate_platform_motion(&p);
        let expected = 3.0 * DEG_TO_RAD * 2.5 * 1.5;
        assert!(
            (r.yaw_displacement_m - expected).abs() < 1e-10,
            "yaw disp: expected {:.6} m, got {:.6} m",
            expected,
            r.yaw_displacement_m
        );
        assert_eq!(r.pitch_displacement_m, 0.0);
    }

    #[test]
    fn ground_vehicle_forward_motion_offset() {
        // Firing at 45° off heading from a moving vehicle.
        let mut p = test_params();
        p.platform = PlatformType::GroundVehicle { speed_kmh: 72.0 };
        p.forward_speed_ms = 20.0; // 72 km/h
        p.azimuth_deg = 45.0;
        // Expected: 20 × sin(45°) × 1.5 = 21.213 m  (large offset!)
        let r = evaluate_platform_motion(&p);
        let expected = 20.0 * (45.0_f64 * DEG_TO_RAD).sin() * 1.5;
        assert!(
            (r.forward_motion_offset_m - expected).abs() < 1e-10,
            "forward offset: expected {:.6} m, got {:.6} m",
            expected,
            r.forward_motion_offset_m
        );
    }

    #[test]
    fn ground_vehicle_lateral_motion_offset() {
        let mut p = test_params();
        p.platform = PlatformType::GroundVehicle { speed_kmh: 50.0 };
        p.lateral_speed_ms = 2.0; // sliding sideways
                                  // Expected: 2.0 × 1.5 = 3.0 m
        let r = evaluate_platform_motion(&p);
        assert!(
            (r.lateral_motion_offset_m - 3.0).abs() < 1e-10,
            "lateral offset: expected 3.0 m, got {:.6} m",
            r.lateral_motion_offset_m
        );
    }

    // ── Watercraft / sea state ──────────────────────────────────────

    #[test]
    fn watercraft_sea_state_pitch_and_roll() {
        // Sea state 5 (rough): estimate pitch ≈ 3.1 °/s, roll ≈ 5.2 °/s.
        let p = PlatformMotionParams {
            platform: PlatformType::Watercraft { sea_state: 5 },
            pitch_rate_deg_per_s: sea_state_pitch_rate(5),
            yaw_rate_deg_per_s: sea_state_yaw_rate(5),
            roll_rate_deg_per_s: sea_state_roll_rate(5),
            forward_speed_ms: 10.0,
            lateral_speed_ms: platform_lateral_speed(PlatformType::Watercraft { sea_state: 5 }),
            time_of_flight_s: 2.0,
            range_m: 800.0,
            azimuth_deg: 0.0,
        };
        let r = evaluate_platform_motion(&p);
        // Pivot = 5.0 m.
        let expected_pitch = sea_state_pitch_rate(5) * DEG_TO_RAD * 5.0 * 2.0;
        let expected_yaw = sea_state_yaw_rate(5) * DEG_TO_RAD * 5.0 * 2.0;
        assert!(
            (r.pitch_displacement_m - expected_pitch).abs() < 1e-10,
            "sea state 5 pitch: expected {:.6} m, got {:.6} m",
            expected_pitch,
            r.pitch_displacement_m
        );
        assert!(
            (r.yaw_displacement_m - expected_yaw).abs() < 1e-10,
            "sea state 5 yaw: expected {:.6} m, got {:.6} m",
            expected_yaw,
            r.yaw_displacement_m
        );
        // Combined offset should be RSS of pitch and yaw.
        let expected_combined = (expected_pitch.powi(2) + expected_yaw.powi(2)).sqrt();
        assert!(
            (r.combined_offset_m - expected_combined).abs() < 1e-10,
            "combined: expected {:.6} m, got {:.6} m",
            expected_combined,
            r.combined_offset_m
        );
    }

    #[test]
    fn sea_state_0_is_nearly_calm() {
        // Sea state 0: very low rates.
        let pitch = sea_state_pitch_rate(0);
        let roll = sea_state_roll_rate(0);
        let yaw = sea_state_yaw_rate(0);
        assert!(pitch < 0.5, "sea state 0 pitch should be < 0.5 °/s");
        assert!(roll < 0.5, "sea state 0 roll should be < 0.5 °/s");
        assert!(yaw < 0.3, "sea state 0 yaw should be < 0.3 °/s");
    }

    #[test]
    fn sea_state_monotonically_increasing() {
        // Higher sea states should have higher or equal rates.
        let mut prev_pitch = 0.0;
        let mut prev_roll = 0.0;
        let mut prev_yaw = 0.0;
        for ss in 0..=9 {
            let p = sea_state_pitch_rate(ss);
            let r = sea_state_roll_rate(ss);
            let y = sea_state_yaw_rate(ss);
            assert!(p >= prev_pitch, "pitch not monotonic at ss={}", ss);
            assert!(r >= prev_roll, "roll not monotonic at ss={}", ss);
            assert!(y >= prev_yaw, "yaw not monotonic at ss={}", ss);
            prev_pitch = p;
            prev_roll = r;
            prev_yaw = y;
        }
    }

    // ── Aircraft ────────────────────────────────────────────────────

    #[test]
    fn aircraft_bank_drift_non_zero() {
        // Banked aircraft produces horizontal drift.
        let p = PlatformMotionParams {
            platform: PlatformType::Aircraft {
                speed_kmh: 500.0,
                bank_angle_deg: 30.0,
            },
            pitch_rate_deg_per_s: 5.0,
            yaw_rate_deg_per_s: 0.0,
            roll_rate_deg_per_s: 0.0,
            forward_speed_ms: 500.0 / 3.6,
            lateral_speed_ms: 0.0,
            time_of_flight_s: 1.0,
            range_m: 600.0,
            azimuth_deg: 0.0,
        };
        let r = evaluate_platform_motion(&p);
        // Expected bank drift: 0.5 × 9.80665 × sin(30°) × 1.0²
        let expected_drift = 0.5 * G * (30.0_f64 * DEG_TO_RAD).sin() * 1.0_f64.powi(2);
        assert!(
            (r.bank_drift_m - expected_drift).abs() < 1e-10,
            "bank drift: expected {:.6} m, got {:.6} m",
            expected_drift,
            r.bank_drift_m
        );
        assert!(
            r.bank_drift_m > 0.0,
            "30° bank should produce positive drift"
        );
    }

    #[test]
    fn aircraft_wings_level_zero_bank_drift() {
        // Wings-level aircraft: no bank drift.
        let p = PlatformMotionParams {
            platform: PlatformType::Aircraft {
                speed_kmh: 500.0,
                bank_angle_deg: 0.0,
            },
            pitch_rate_deg_per_s: 10.0,
            yaw_rate_deg_per_s: 10.0,
            roll_rate_deg_per_s: 5.0,
            forward_speed_ms: 500.0 / 3.6,
            lateral_speed_ms: 0.0,
            time_of_flight_s: 0.5,
            range_m: 300.0,
            azimuth_deg: 0.0,
        };
        let r = evaluate_platform_motion(&p);
        assert_eq!(r.bank_drift_m, 0.0, "wings-level → zero bank drift");
        // Pitch and yaw displacements should still be present.
        assert!(
            r.pitch_displacement_m > 0.0,
            "pitch displacement should be > 0"
        );
        assert!(r.yaw_displacement_m > 0.0, "yaw displacement should be > 0");
    }

    #[test]
    fn aircraft_manoeuvre_high_rates() {
        let (pitch, yaw, roll) = platform_motion_envelope(PlatformType::Aircraft {
            speed_kmh: 250.0,
            bank_angle_deg: 45.0,
        });
        // At 250 km/h (slow, high-manoeuvrability regime), rates
        // should be well above the human-scale values.
        assert!(pitch > 10.0, "aircraft pitch rate should be > 10 °/s");
        assert!(yaw > 5.0, "aircraft yaw rate should be > 5 °/s");
        assert!(roll > 5.0, "aircraft roll rate should be > 5 °/s");
    }

    // ── Human stances ───────────────────────────────────────────────

    #[test]
    fn human_standing_pitch_and_yaw() {
        let mut p = test_params();
        p.platform = PlatformType::HumanStanding;
        p.pitch_rate_deg_per_s = 1.5;
        p.yaw_rate_deg_per_s = 1.5;
        p.time_of_flight_s = 1.0;
        // Pivot = 1.5 m.
        // Expected pitch: 1.5° × π/180 × 1.5 × 1.0 = 0.0393 m
        // Expected yaw:   same = 0.0393 m
        // Combined: sqrt(2) × 0.0393 = 0.0555 m
        let r = evaluate_platform_motion(&p);
        let expected_py = 1.5 * DEG_TO_RAD * 1.5 * 1.0;
        assert!(
            (r.pitch_displacement_m - expected_py).abs() < 1e-10,
            "standing pitch: expected {:.6} m, got {:.6} m",
            expected_py,
            r.pitch_displacement_m
        );
        assert!(
            (r.yaw_displacement_m - expected_py).abs() < 1e-10,
            "standing yaw: expected {:.6} m, got {:.6} m",
            expected_py,
            r.yaw_displacement_m
        );
        let expected_combined = (2.0_f64).sqrt() * expected_py;
        assert!(
            (r.combined_offset_m - expected_combined).abs() < 1e-10,
            "standing combined: expected {:.6} m, got {:.6} m",
            expected_combined,
            r.combined_offset_m
        );
    }

    #[test]
    fn human_prone_is_more_stable_than_standing() {
        let (pitch_p, _, _) = platform_motion_envelope(PlatformType::HumanProne);
        let (pitch_s, _, _) = platform_motion_envelope(PlatformType::HumanStanding);
        assert!(
            pitch_p < pitch_s,
            "Prone pitch ({}) should be < standing pitch ({})",
            pitch_p,
            pitch_s
        );
    }

    #[test]
    fn human_kneeling_between_prone_and_standing() {
        let (pitch_pr, _, _) = platform_motion_envelope(PlatformType::HumanProne);
        let (pitch_k, _, _) = platform_motion_envelope(PlatformType::HumanKneeling);
        let (pitch_s, _, _) = platform_motion_envelope(PlatformType::HumanStanding);
        assert!(
            pitch_pr < pitch_k,
            "Prone ({}) < Kneeling ({})",
            pitch_pr,
            pitch_k
        );
        assert!(
            pitch_k < pitch_s,
            "Kneeling ({}) < Standing ({})",
            pitch_k,
            pitch_s
        );
    }

    // ── Zero TOF / zero range guards ────────────────────────────────

    #[test]
    fn zero_tof_returns_zero_displacement() {
        let mut p = test_params();
        p.time_of_flight_s = 0.0;
        p.pitch_rate_deg_per_s = 10.0;
        let r = evaluate_platform_motion(&p);
        assert_eq!(r.pitch_displacement_m, 0.0);
        assert_eq!(r.combined_offset_m, 0.0);
    }

    #[test]
    fn zero_range_returns_zero_displacement() {
        let mut p = test_params();
        p.range_m = 0.0;
        p.yaw_rate_deg_per_s = 10.0;
        let r = evaluate_platform_motion(&p);
        assert_eq!(r.yaw_displacement_m, 0.0);
        assert_eq!(r.combined_offset_m, 0.0);
    }

    // ── total_motion_offset convenience ─────────────────────────────

    #[test]
    fn total_motion_offset_combines_all_components() {
        // All motion sources active at once.
        let mut p = test_params();
        p.platform = PlatformType::GroundVehicle { speed_kmh: 50.0 };
        p.pitch_rate_deg_per_s = 2.0;
        p.yaw_rate_deg_per_s = 3.0;
        p.forward_speed_ms = 15.0;
        p.lateral_speed_ms = 1.0;
        p.azimuth_deg = 30.0;
        p.time_of_flight_s = 2.0;

        let r = evaluate_platform_motion(&p);
        let (v, h) = total_motion_offset(&p);

        // Vertical should equal pitch displacement only.
        assert!((v - r.pitch_displacement_m).abs() < 1e-12);

        // Horizontal = yaw + forward_motion + lateral_motion.
        let h_expected =
            r.yaw_displacement_m + r.forward_motion_offset_m + r.lateral_motion_offset_m;
        assert!(
            (h - h_expected).abs() < 1e-10,
            "total horizontal: expected {:.6} m, got {:.6} m",
            h_expected,
            h
        );
    }

    // ── params_from_platform convenience ────────────────────────────

    #[test]
    fn params_from_platform_ground_vehicle() {
        let params = params_from_platform(
            PlatformType::GroundVehicle { speed_kmh: 60.0 },
            15.0,
            1.2,
            400.0,
        );
        // Should have non-zero rates, forward speed = 60/3.6 ≈ 16.67 m/s
        assert!(params.pitch_rate_deg_per_s > 0.0);
        assert!(params.yaw_rate_deg_per_s > 0.0);
        assert!((params.forward_speed_ms - 60.0 / 3.6).abs() < 1e-10);
    }

    #[test]
    fn params_from_platform_stationary() {
        let params = params_from_platform(PlatformType::Stationary, 0.0, 1.0, 300.0);
        assert_eq!(params.pitch_rate_deg_per_s, 0.0);
        assert_eq!(params.yaw_rate_deg_per_s, 0.0);
        assert_eq!(params.forward_speed_ms, 0.0);
        assert_eq!(params.lateral_speed_ms, 0.0);

        let r = evaluate_platform_motion(&params);
        assert_eq!(r, PlatformMotionResult::default());
    }

    // ── Determinism ─────────────────────────────────────────────────

    #[test]
    fn evaluate_is_deterministic() {
        let p = PlatformMotionParams {
            platform: PlatformType::Watercraft { sea_state: 4 },
            pitch_rate_deg_per_s: 2.5,
            yaw_rate_deg_per_s: 1.2,
            roll_rate_deg_per_s: 4.0,
            forward_speed_ms: 8.0,
            lateral_speed_ms: 1.5,
            time_of_flight_s: 1.8,
            range_m: 650.0,
            azimuth_deg: -20.0,
        };
        let a = evaluate_platform_motion(&p);
        let b = evaluate_platform_motion(&p);
        assert_eq!(a, b);
    }

    // ── Azimuth sign symmetry ───────────────────────────────────────

    #[test]
    fn azimuth_sign_symmetry() {
        // Firing at +45° and -45° with the same forward speed should
        // produce opposite forward-motion offsets.
        let mut p_right = test_params();
        p_right.forward_speed_ms = 20.0;
        p_right.azimuth_deg = 45.0;

        let mut p_left = test_params();
        p_left.forward_speed_ms = 20.0;
        p_left.azimuth_deg = -45.0;

        let r_right = evaluate_platform_motion(&p_right);
        let r_left = evaluate_platform_motion(&p_left);

        assert!(
            r_right.forward_motion_offset_m > 0.0,
            "right azimuth should give positive offset"
        );
        assert!(
            r_left.forward_motion_offset_m < 0.0,
            "left azimuth should give negative offset"
        );
        assert!(
            (r_right.forward_motion_offset_m + r_left.forward_motion_offset_m).abs() < 1e-10,
            "±45° should be symmetric: {} vs {}",
            r_right.forward_motion_offset_m,
            r_left.forward_motion_offset_m
        );
    }

    // ── Edge case: rearward firing ──────────────────────────────────

    #[test]
    fn rearward_firing_reverses_forward_offset() {
        // Firing directly rearward (180°) from a forward-moving platform
        // should give zero forward-motion offset (sin(180°) = 0).
        let mut p = test_params();
        p.forward_speed_ms = 20.0;
        p.azimuth_deg = 180.0;
        let r = evaluate_platform_motion(&p);
        assert!(
            r.forward_motion_offset_m.abs() < 1e-12,
            "rearward firing should give zero forward offset, got {}",
            r.forward_motion_offset_m
        );
    }
}
