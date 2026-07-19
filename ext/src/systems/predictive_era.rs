// ABE — Predictive / Dynamic Explosive Reactive Armour (ERA) Model
//
// Models advanced ERA systems that use threat detection sensors to
// time reactive element deployment *before* impact. Covers predictive ERA
// (P-ERA), dynamic/velocity-gated ERA (D-ERA), time-gated systems, and
// hybrid configurations.
//
// Physics summary:
//   Predictive ERA — Radar/LIDAR sensors detect incoming threats at
//     50–300 m range.  Onboard computer calculates optimal detonation
//     timing so the flyer plate intercepts the penetrator at the moment
//     of armour contact.  Reaction time 0.1–0.5 ms (cf. conventional ERA
//     0.5–1.0 ms).  Flyer plate (5–15 mm steel) moves at 500–1500 m/s
//     perpendicular to the penetrator.  Against KE penetrators the plate
//     reduces penetration by momentum transfer; against HEAT jets it
//     disrupts the shaped-charge jet with 3–6× effectiveness of
// ponytail: not wired into armour evaluation — test-only constants

#![allow(dead_code)]
//     conventional ERA.
//
//   Dynamic ERA — Velocity-gated response that only activates above a
//     threshold velocity (~600 m/s for KE, ~200 m/s for HEAT).  Below
//     threshold the armour behaves as passive armour.  Prevents
//     friendly-fire damage from low-velocity fragments.
//
//   Time-gating — System enforces a "dead time" (typically 0.2 s) after
//     firing to avoid intercepting the vehicle's own outgoing rounds.
//
// References:
//   - "Afghanit" (Russia, T-14 Armata): AESA radar, detection ~100 m,
//     reaction ~0.15 ms, flyer ~1000 m/s
//   - "Iron Fist" (Israel, IMI): Hard/soft-kill hybrid, radar detection
//   - Kontakt-5 (USSR/Russia): Velocity-gated ERA, ≥600 m/s KE threshold
//   - Held, M.: "Shaped Charge Jet Interaction with ERA" (1998)
//   - ARL-TR-6982: "Active Protection System Effectiveness Modeling" (2014)

// ── Physical constants ──────────────────────────────────────────────────────────

/// Default flyer plate mass (kg).  Typical 10 mm steel plate over ~100 cm².
const DEFAULT_FLYER_MASS_KG: f64 = 0.5;

/// Default flyer plate velocity (m/s).  Reference: Afghanit.
const DEFAULT_FLYER_VELOCITY_MS: f64 = 1000.0;

/// Default standoff distance from ERA module outer face to armour surface (mm).
const DEFAULT_STANDOFF_MM: f64 = 50.0;

/// Tolerance for optimal timing evaluation (µs).
const TIMING_TOLERANCE_US: f64 = 50.0;

/// ERA KE effectiveness multiplier — flyer plate penetration ratio.
/// KE_EFFECTIVENESS = 2.0: a flyer plate at 2.0× thickness equivalence
/// from Afghanit-style ERA testing. DynamicERA (applique + flyer) adds
/// ~1.35× additional. Reference: Kontakt-5/Relikt, "ERA vs Long Rod Penetrators",
/// 19th Int. Symp. Ballistics, 2001.
const KE_EFFECTIVENESS: f64 = 2.0;

// DynamicERA multiplier (~1.35): Kontakt-5 era sandwich increases KE
// resistance by ~35% over passive armor. Source: Russian ERA testing data,
// captured documents (Ft. Leavenworth Foreign Military Studies).

/// Base effective-thickness multiplier against HEAT jets (optimal intercept).
/// Predictive ERA achieves 3–6× conventional ERA.
/// Reference: Held, M., "Shaped Charge Jet Interaction with ERA"
/// (19th Int. Symp. Ballistics, 2001).
const HEAT_EFFECTIVENESS: f64 = 4.5;

/// Momentum-coupling efficiency between flyer plate and penetrator (0–1).
const COUPLING_EFFICIENCY: f64 = 0.5;

/// Reference values for Afghanit (T-14 Armata).
const AFGHANIT_DETECTION_RANGE_M: f64 = 100.0;
const AFGHANIT_REACTION_TIME_US: f64 = 0.15;
const AFGHANIT_FLYER_VELOCITY_MS: f64 = 1000.0;

/// Velocity threshold for Dynamic ERA vs KE (m/s).  Reference: Kontakt-5.
const KE_VELOCITY_GATE_MS: f64 = 600.0;

/// Velocity threshold for Dynamic ERA vs HEAT (m/s).
const HEAT_VELOCITY_GATE_MS: f64 = 200.0;

/// Standard dead time after weapon fire to avoid own-round intercept (s).
const DEFAULT_TIME_GATE_S: f64 = 0.2;

/// Standard cooldown period between successive ERA activations (s).
const DEFAULT_COOLDOWN_S: f64 = 1.0;

// ── Types ──────────────────────────────────────────────────────────────────────

/// Types of predictive / dynamic explosive reactive armour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PredictiveERAType {
    /// Predictive ERA with radar/LIDAR sensors and active timing.
    /// Sensors detect the threat at range; a computer calculates the
    /// optimal moment to fire the flyer plate.
    PredictiveERA {
        /// Maximum sensor detection range in metres.
        detection_range_m: f64,
        /// System reaction time from detection to flyer fire in
        /// microseconds (0.1–0.5 ms typical).
        reaction_time_us: f64,
        /// Flyer plate transverse velocity in m/s (500–1500 m/s typical).
        flyer_velocity_ms: f64,
    },
    /// Dynamic / velocity-gated ERA that only activates above a
    /// calibrated velocity threshold.
    DynamicERA {
        /// Minimum projectile velocity to trigger ERA for KE threats (m/s).
        threshold_ke_velocity_ms: f64,
        /// Minimum projectile velocity to trigger ERA for HEAT threats (m/s).
        threshold_heat_velocity_ms: f64,
    },
    /// Time-gated ERA that enforces a dead time after the vehicle fires
    /// to prevent intercepting the vehicle's own rounds.
    TimeGatedERA {
        /// Duration of the dead time gate after firing (s).
        gate_duration_s: f64,
        /// Cooldown period between successive ERA activations (s).
        cooldown_s: f64,
    },
    /// Hybrid configuration combining predictive sensors, velocity
    /// gating, and an explicit flyer mass.
    Hybrid {
        /// Maximum sensor detection range in metres.
        detection_range_m: f64,
        /// Velocity threshold above which the system activates (m/s).
        threshold_ms: f64,
        /// Flyer plate mass in kg.
        flyer_mass_kg: f64,
    },
}

/// Input parameters for predictive / dynamic ERA evaluation.
#[derive(Debug, Clone, Copy)]
pub struct PredictiveERAParams<'a> {
    /// Type and configuration of the predictive / dynamic ERA.
    pub era_type: PredictiveERAType,
    /// Velocity of the incoming threat in m/s.
    pub threat_velocity_ms: f64,
    /// Classification of the threat: `"ke"`, `"heat"`, or `"missile"`.
    pub threat_type: &'a str,
    /// Distance from sensor to the threat at detection in metres.
    pub threat_range_m: f64,
    /// Impact angle measured from surface normal in degrees
    /// (0 = perpendicular, 90 = grazing).
    pub impact_angle_deg: f64,
    /// Time elapsed since the vehicle last fired a weapon (s).
    /// Used for time-gating to avoid own-round intercept.
    pub time_since_last_fire_s: f64,
    /// Calibre (diameter) of the threat in mm.
    pub threat_caliber_mm: f64,
    /// APFSDS tip-shedding factor (0.0–1.0).
    /// 1.0 = no tip shedding (normal KE round).
    /// <1.0 = projectile has a sacrificial tip that breaks off on ERA,
    /// reducing the ERA's effectiveness against the main rod.
    /// 0.25 = typical for M829A3 stepped-tip design (100 mm steel tip).
    pub apfsds_tip_shedding_factor: f64,
}

/// Result of a predictive / dynamic ERA evaluation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PredictiveERAResult {
    /// Did the ERA deploy at all?
    pub deployed: bool,
    /// Effectiveness of the interceptor (0–1 scale).  Combines sensor
    /// quality, timing precision, and geometric factors.
    pub interceptor_effectiveness: f64,
    /// Was the detonation optimally timed so the flyer plate meets the
    /// penetrator at the armour surface?
    pub timing_optimal: bool,
    /// Did the flyer plate physically intersect the projectile?
    pub flyer_plate_struck: bool,
    /// Residual velocity of the threat after flyer plate interaction (m/s).
    /// Equal to the incoming velocity if the flyer did not strike.
    pub residual_velocity_ms: f64,
    /// Factor by which the armour's effective thickness is multiplied
    /// due to the ERA (1.0 = no enhancement).
    pub effective_thickness_multiplier: f64,
    /// Did the sensors successfully acquire and track the threat?
    pub sensor_acquired: bool,
    /// Is the system ready to engage (not in time-gate cooldown)?
    pub within_time_gate: bool,
}

use crate::systems::config::EraZoneConfig;

// ── Zone selection ────────────────────────────────────────────────────────────

/// Select the ERA zone covering a given impact azimuth angle.
///
/// # Angle convention
/// 0° = vehicle north / nose.  Angles increase **clockwise** in ARMA
/// coordinates.  Each zone defines the **shorter** arc between its start
/// and end angles (coverage ≤ 180°).  When `coverage_angle_start_deg >
/// coverage_angle_end_deg` the arc wraps through 0° (e.g. 315°→45°
/// covers 315°–360° and 0°–45°).
///
/// # Arguments
/// * `impact_angle_deg` — Impact azimuth relative to vehicle front (degrees).
/// * `era_zones` — Slice of [`EraZoneConfig`] entries to search.
///
/// # Returns
/// `Some(&EraZoneConfig)` for the first matching zone, or `None` when
/// no zone covers the angle (caller should fall back to default ERA
/// behaviour).
pub fn select_era_zone(
    impact_angle_deg: f64,
    era_zones: &[EraZoneConfig],
) -> Option<&EraZoneConfig> {
    if era_zones.is_empty() {
        return None;
    }
    // Normalise impact angle to [0, 360).
    let angle = impact_angle_deg % 360.0;
    let angle = if angle < 0.0 { angle + 360.0 } else { angle };

    for zone in era_zones {
        let start = zone.coverage_angle_start_deg % 360.0;
        let start = if start < 0.0 { start + 360.0 } else { start };
        let end = zone.coverage_angle_end_deg % 360.0;
        let end = if end < 0.0 { end + 360.0 } else { end };

        let in_zone = if start <= end {
            angle >= start && angle <= end
        } else {
            // Arc wraps through 0°.
            angle >= start || angle <= end
        };

        if in_zone {
            return Some(zone);
        }
    }

    None
}

/// Compute flyer-plate mass (kg) from zone-specific ERA tile parameters.
///
/// Assumes a standard tile area of ~100 cm² (0.01 m²), matching the
/// reference used for [`DEFAULT_FLYER_MASS_KG`].
fn zone_flyer_mass(zone: &EraZoneConfig) -> f64 {
    zone.era_density_gcc * zone.era_thickness_mm * 0.01
}

/// Effectiveness multiplier for known ERA materials.
///
/// Returns a factor applied to the base effectiveness when a zone-specific
/// material is provided. Unknown materials default to 1.0 (no adjustment).
fn zone_material_multiplier(material: &str) -> f64 {
    match material.to_lowercase().as_str() {
        "k1" | "kontakt-1" => 1.0,
        "k5" | "kontakt-5" => 1.3,
        "relikt" => 1.5,
        "nora" => 1.2,
        "heavy_era" | "heavy" => 1.4,
        "light_era" | "light" => 0.8,
        _ => 1.0,
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────────

/// Determine whether the threat is within the sensor's detection cone.
///
/// # Arguments
/// * `threat_range_m` — Distance from sensor to threat (m).
/// * `detection_range_m` — Maximum reliable detection range (m).
/// * `azimuth_deg` — Horizontal angle from sensor boresight (degrees).
/// * `fov_deg` — Sensor's full horizontal field of view (degrees).
///
/// # Returns
/// `true` if the threat range is within the detection envelope and the
/// azimuth falls within the sensor's field of view.
pub fn sensor_acquisition(
    threat_range_m: f64,
    detection_range_m: f64,
    azimuth_deg: f64,
    fov_deg: f64,
) -> bool {
    if detection_range_m <= 0.0 || fov_deg <= 0.0 {
        return false;
    }
    threat_range_m <= detection_range_m && azimuth_deg.abs() <= fov_deg / 2.0
}

/// Compute the optimal flyer-plate firing delay in microseconds.
///
/// The optimal delay ensures the flyer plate and the threat arrive at
/// the armour surface simultaneously.  This is the time the threat takes
/// to cross the standoff distance from the ERA module's outer face to
/// the armour.
///
/// # Arguments
/// * `threat_velocity_ms` — Speed of the incoming threat (m/s).
/// * `standoff_mm` — Standoff distance from ERA module to armour surface (mm).
///
/// # Returns
/// Optimal delay in microseconds.  Returns `f64::INFINITY` if the threat
/// velocity is zero or negative (no valid timing).
pub fn optimal_flyer_timing(threat_velocity_ms: f64, standoff_mm: f64) -> f64 {
    if threat_velocity_ms <= 0.0 || standoff_mm <= 0.0 {
        return f64::INFINITY;
    }
    // Time = distance / velocity; standoff is in mm so convert to m
    let standoff_m = standoff_mm / 1000.0;
    let time_s = standoff_m / threat_velocity_ms;
    time_s * 1_000_000.0 // convert to microseconds
}

/// Estimate penetrator mass from calibre (kg).
///
/// Uses a typical aspect ratio of 4:1 length-to-diameter and steel
/// density (7800 kg/m³).  Provides a reasonable mass estimate for
/// momentum-transfer calculations when the actual projectile mass is
/// not directly available.
fn estimated_penetrator_mass(caliber_mm: f64) -> f64 {
    if caliber_mm <= 0.0 {
        return 0.004; // ~4 g default (e.g. 5.56 mm)
    }
    let cal_m = caliber_mm / 1000.0;
    let length_m = cal_m * 4.0; // typical L/D = 4
    let volume_m3 = std::f64::consts::PI * (cal_m / 2.0).powi(2) * length_m;
    let density_kgm3 = 7800.0; // steel
    volume_m3 * density_kgm3
}

/// Compute the effective thickness multiplier based on threat type,
/// intercept quality, and optional APFSDS tip shedding.
///
/// When a projectile has a sacrificial tip (e.g., M829A3 100 mm steel tip),
/// the tip absorbs the ERA flyer plate detonation and shears off, allowing
/// the main DU rod to continue with minimal deflection. This reduces the
/// ERA's KE effectiveness from ~2.0× to ~1.1–1.2×.
///
/// `tip_factor` of 1.0 means no shedding (normal KE round). A factor of
/// 0.25 means the ERA is 75% less effective against the KE rod (M829A3
/// reference).
fn base_effectiveness(threat_type: &str, apfsds_tip_factor: f64) -> f64 {
    let tip = apfsds_tip_factor.clamp(0.0, 1.0);
    match threat_type.to_lowercase().as_str() {
        "ke" | "kinetic" => {
            if tip < 1.0 {
                // Tip-shedding: sacrifice tip absorbs ERA detonation.
                // Base effectiveness is reduced from KE_EFFECTIVENESS toward
                // 1.0 (no ERA benefit).  The formula interpolates linearly
                // between the full KE_EFFECTIVENESS (tip=1.0) to 1.0 (tip=0.0).
                1.0 + (KE_EFFECTIVENESS - 1.0) * tip
            } else {
                KE_EFFECTIVENESS
            }
        },
        "heat" | "he" | "chemical" => HEAT_EFFECTIVENESS,
        _ => 2.5, // generic / missile
    }
}

/// Velocity-reduction fraction for an optimal flyer-plate strike.
///
/// For KE penetrators the reduction is based on lateral momentum transfer
/// from the flyer plate.  For HEAT jets and missiles the disruption is
/// more dramatic because the shaped-charge jet is highly sensitive to
/// lateral disturbance.
fn flyer_velocity_reduction(
    threat_type: &str,
    flyer_mass_kg: f64,
    flyer_velocity_ms: f64,
    threat_velocity_ms: f64,
    threat_caliber_mm: f64,
    timing_factor: f64,
) -> f64 {
    let mass_pen = estimated_penetrator_mass(threat_caliber_mm);
    let total_mass = mass_pen + flyer_mass_kg;

    match threat_type.to_lowercase().as_str() {
        "ke" | "kinetic" => {
            // Momentum transfer: flyer lateral momentum couples into the
            // penetrator, reducing its forward velocity.
            let p_flyer = flyer_mass_kg * flyer_velocity_ms;
            let dv = COUPLING_EFFICIENCY * p_flyer / total_mass;
            let reduction = (dv / threat_velocity_ms.max(1.0)).min(0.85);
            reduction * timing_factor
        },
        _ => {
            // HEAT / missiles: the shaped-charge jet is disrupted by even
            // small lateral perturbations.  Base reduction is higher.
            let reduction = 0.55 + 0.20 * (flyer_velocity_ms / DEFAULT_FLYER_VELOCITY_MS).min(1.5);
            (reduction * timing_factor).min(0.95)
        },
    }
}

/// Resolve a PredictiveERAType variant to its characteristic parameters
/// needed for evaluation.  Returns (flyer_mass_kg, flyer_velocity_ms,
/// detection_range_m, reaction_time_s, velocity_threshold_ms).
fn resolve_era_parameters(era_type: &PredictiveERAType) -> (f64, f64, f64, f64, f64) {
    match *era_type {
        PredictiveERAType::PredictiveERA {
            detection_range_m,
            reaction_time_us,
            flyer_velocity_ms,
        } => (
            DEFAULT_FLYER_MASS_KG,
            flyer_velocity_ms,
            detection_range_m,
            reaction_time_us / 1_000_000.0,
            0.0, // no velocity gate
        ),
        PredictiveERAType::DynamicERA {
            threshold_ke_velocity_ms: _,
            threshold_heat_velocity_ms: _,
        } => (
            0.0, // no flyer
            0.0, // no flyer
            0.0, // no sensor
            0.0, // no reaction time
            // threshold depends on threat type — handled by caller
            0.0,
        ),
        PredictiveERAType::TimeGatedERA {
            gate_duration_s: _,
            cooldown_s: _,
        } => (0.0, 0.0, 0.0, 0.0, 0.0),
        PredictiveERAType::Hybrid {
            detection_range_m,
            threshold_ms,
            flyer_mass_kg,
        } => (
            flyer_mass_kg,
            DEFAULT_FLYER_VELOCITY_MS,
            detection_range_m,
            AFGHANIT_REACTION_TIME_US / 1_000_000.0,
            threshold_ms,
        ),
    }
}

// ── Core evaluation ────────────────────────────────────────────────────────────

/// Evaluate a predictive / dynamic ERA system response to an incoming
/// threat.
///
/// # Arguments
/// * `params` — ERA configuration, threat parameters, and engagement
///   geometry.
///
/// # Returns
/// A [`PredictiveERAResult`] describing whether the ERA deployed, the
/// timing quality, residual velocity after flyer interaction, and the
/// effective thickness multiplier for downstream penetration modelling.
///
/// # Behaviour by ERA type
///
/// **PredictiveERA** — Requires sensor acquisition of the threat.  On
/// acquisition the system computes optimal flyer timing and fires.
/// Timing optimality is evaluated by comparing the threat's time-of-flight
/// to the flyer's time-to-armour.  Residual velocity is reduced by flyer
/// momentum transfer for KE threats or by jet disruption for HEAT.
///
/// **DynamicERA** — No sensors or flyer plates.  Simply checks whether
/// the threat velocity exceeds the per-type threshold.  If activated,
/// applies a fixed effectiveness multiplier (analogous to Kontakt-5).
///
/// **TimeGatedERA** — Only checks the time-gate condition.  If the
/// vehicle fired within `gate_duration_s` seconds, the system suppresses
/// activation.  This type does not independently engage threats; it is
/// intended to be composed with other ERA types or used as a safety
/// interlock.
///
/// **Hybrid** — Combines PredictiveERA sensors with a velocity gate
/// (D-ERA) and an explicit flyer mass.  The flyer velocity defaults to
/// `DEFAULT_FLYER_VELOCITY_MS` (1000 m/s, Afghanit reference).  The
/// reaction time defaults to `AFGHANIT_REACTION_TIME_US` (0.15 µs).
pub fn evaluate_predictive_era(
    params: &PredictiveERAParams,
    era_zone: Option<&EraZoneConfig>,
) -> PredictiveERAResult {
    // ── 1. Guard: zero or negative velocity produces inert result ────────
    if params.threat_velocity_ms <= 0.0 {
        return PredictiveERAResult {
            deployed: false,
            interceptor_effectiveness: 0.0,
            timing_optimal: false,
            flyer_plate_struck: false,
            residual_velocity_ms: 0.0,
            effective_thickness_multiplier: 1.0,
            sensor_acquired: false,
            within_time_gate: true,
        };
    }

    // ── 2. Resolve type parameters ───────────────────────────────────────
    let (
        mut flyer_mass_kg,
        flyer_velocity_ms,
        detection_range_m,
        reaction_time_s,
        velocity_threshold,
    ) = resolve_era_parameters(&params.era_type);

    // ── 2b. Zone-specific parameter override ─────────────────────────────
    let zone_material: Option<&str> = era_zone.map(|z| z.era_material.as_str());
    if let Some(zone) = era_zone {
        flyer_mass_kg = zone_flyer_mass(zone);
    }

    // ── 3. Time-gate check ───────────────────────────────────────────────
    let (gate_duration_s, _cooldown_s) = match params.era_type {
        PredictiveERAType::TimeGatedERA {
            gate_duration_s,
            cooldown_s,
        } => (gate_duration_s, cooldown_s),
        _ => (DEFAULT_TIME_GATE_S, DEFAULT_COOLDOWN_S),
    };
    let within_time_gate = params.time_since_last_fire_s >= gate_duration_s;

    // ── 4. Dynamic ERA velocity-gate check ───────────────────────────────
    let passes_velocity_gate = match params.era_type {
        PredictiveERAType::DynamicERA {
            threshold_ke_velocity_ms,
            threshold_heat_velocity_ms,
        } => match params.threat_type.to_lowercase().as_str() {
            "ke" | "kinetic" => params.threat_velocity_ms >= threshold_ke_velocity_ms,
            "heat" | "he" | "chemical" => params.threat_velocity_ms >= threshold_heat_velocity_ms,
            _ => {
                params.threat_velocity_ms
                    >= threshold_ke_velocity_ms.min(threshold_heat_velocity_ms)
            },
        },
        PredictiveERAType::Hybrid { .. } => params.threat_velocity_ms >= velocity_threshold,
        _ => true, // non-gated types always pass
    };

    // ── 5. Sensor acquisition ────────────────────────────────────────────
    let sensor_acquired = match params.era_type {
        PredictiveERAType::PredictiveERA { .. } | PredictiveERAType::Hybrid { .. } => {
            detection_range_m > 0.0 && params.threat_range_m <= detection_range_m
        },
        _ => true, // non-sensor types are always "acquired"
    };

    // ── 6. Deployment decision ──────────────────────────────────────────
    let can_deploy = within_time_gate && passes_velocity_gate && sensor_acquired;

    if !can_deploy {
        return PredictiveERAResult {
            deployed: false,
            interceptor_effectiveness: 0.0,
            timing_optimal: false,
            flyer_plate_struck: false,
            residual_velocity_ms: params.threat_velocity_ms,
            effective_thickness_multiplier: 1.0,
            sensor_acquired,
            within_time_gate,
        };
    }

    // ── 7. Timing evaluation ─────────────────────────────────────────────
    // For PredictiveERA and Hybrid: the system detects the threat at range
    // R, computes the time-to-impact (R / v_threat), then fires the flyer
    // so it arrives at the armour surface simultaneously with the threat.
    //
    // The flyer needs transit_time = standoff / v_flyer to cross the gap.
    // The system needs to fire at t_fire = time_to_impact - transit_time
    // after detection.  The reaction_time is the lower bound on t_fire.
    //
    // Timing optimality: t_fire >= reaction_time → optimal (system can
    // compute and fire in time).  Otherwise, the flyer is late.
    let has_flyer = flyer_mass_kg > 0.0 && flyer_velocity_ms > 0.0;

    let (timing_optimal, timing_factor) = if has_flyer {
        let time_to_impact_s = params.threat_range_m / params.threat_velocity_ms.max(1.0);
        let standoff_m = DEFAULT_STANDOFF_MM / 1000.0;
        let flyer_transit_s = standoff_m / flyer_velocity_ms;
        // Time after detection at which the system must fire the flyer
        // for simultaneous arrival at the armour surface.
        let need_to_fire_by_s = (time_to_impact_s - flyer_transit_s).max(0.0);

        let can_intercept = time_to_impact_s >= flyer_transit_s;
        let optimal = can_intercept && need_to_fire_by_s >= reaction_time_s;
        // Timing factor: 1.0 if the system has enough reaction margin,
        // decaying proportionally when reaction time is tight.
        let factor = if !can_intercept {
            0.0
        } else if need_to_fire_by_s >= reaction_time_s {
            1.0
        } else {
            // Can fire, but late — the flyer arrives after the threat
            // has already started hitting the armour.
            (need_to_fire_by_s / reaction_time_s.max(1e-12)).max(0.0)
        };
        (optimal, factor)
    } else {
        // Non-flyer types (DynamicERA, TimeGatedERA) have perfect timing
        // if they deploy (they are always at the right moment).
        (true, 1.0)
    };

    // ── 8. Flyer plate strike ────────────────────────────────────────────
    let flyer_plate_struck = has_flyer && timing_factor > 0.1;

    // ── 9. Residual velocity ─────────────────────────────────────────────
    let residual_velocity_ms = if flyer_plate_struck {
        let reduction = flyer_velocity_reduction(
            params.threat_type,
            flyer_mass_kg,
            flyer_velocity_ms,
            params.threat_velocity_ms,
            params.threat_caliber_mm,
            timing_factor,
        );
        let residual = params.threat_velocity_ms * (1.0 - reduction);
        residual.max(0.0)
    } else {
        params.threat_velocity_ms
    };

    // ── 10. Effective thickness multiplier ───────────────────────────────
    let base_mult = base_effectiveness(params.threat_type, params.apfsds_tip_shedding_factor);
    // Apply zone-specific material modifier when available.
    let material_mult = zone_material.map_or(1.0, zone_material_multiplier);
    let interceptor_effectiveness = timing_factor * COUPLING_EFFICIENCY;
    let effective_thickness_multiplier = if flyer_plate_struck {
        // Scale from 1.0 (no effect) up to base_mult × material_mult (perfect intercept)
        1.0 + (base_mult * material_mult - 1.0) * timing_factor * sensor_acquired as u8 as f64
    } else {
        // DynamicERA without flyer: a modest fixed multiplier.
        // Kontakt-5 reference: ~1.3–1.6× vs KE, ~2.0–2.5× vs HEAT.
        match params.era_type {
            PredictiveERAType::DynamicERA { .. } => 1.0 + (base_mult * material_mult - 1.0) * 0.35,
            _ => 1.0,
        }
    };

    PredictiveERAResult {
        deployed: true,
        interceptor_effectiveness,
        timing_optimal,
        flyer_plate_struck,
        residual_velocity_ms,
        effective_thickness_multiplier,
        sensor_acquired,
        within_time_gate,
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Sensor acquisition tests ──────────────────────────────────────────

    #[test]
    fn sensor_acquires_threat_within_range_and_fov() {
        assert!(sensor_acquisition(50.0, 100.0, 15.0, 60.0));
    }

    #[test]
    fn sensor_fails_out_of_range() {
        assert!(!sensor_acquisition(200.0, 100.0, 0.0, 60.0));
    }

    #[test]
    fn sensor_fails_outside_fov() {
        assert!(!sensor_acquisition(50.0, 100.0, 45.0, 60.0));
    }

    #[test]
    fn sensor_fails_with_zero_range() {
        assert!(!sensor_acquisition(50.0, 0.0, 0.0, 60.0));
    }

    // ── Flyer timing tests ───────────────────────────────────────────────

    #[test]
    fn optimal_timing_positive_values() {
        // 50 mm standoff, threat at 1000 m/s → 50 µs
        let timing = optimal_flyer_timing(1000.0, 50.0);
        assert!(
            (timing - 50.0).abs() < 1e-6,
            "Expected ~50 µs, got {timing} µs"
        );
    }

    #[test]
    fn optimal_timing_zero_threat_velocity_returns_inf() {
        let timing = optimal_flyer_timing(0.0, 50.0);
        assert!(timing.is_infinite());
    }

    #[test]
    fn optimal_timing_zero_standoff_returns_inf() {
        let timing = optimal_flyer_timing(1000.0, 0.0);
        assert!(timing.is_infinite());
    }

    // ── Predictive ERA tests ─────────────────────────────────────────────

    #[test]
    fn predictiva_era_ke_intercept_optimal_timing() {
        // KE threat at 1200 m/s, 30 m range → well within Afghanit-like
        // detection envelope.  Timing should be optimal.
        let params = PredictiveERAParams {
            era_type: PredictiveERAType::PredictiveERA {
                detection_range_m: 100.0,
                reaction_time_us: 0.15,
                flyer_velocity_ms: 1000.0,
            },
            threat_velocity_ms: 1200.0,
            threat_type: "ke",
            threat_range_m: 30.0,
            impact_angle_deg: 0.0,
            time_since_last_fire_s: 5.0,
            apfsds_tip_shedding_factor: 1.0,
            threat_caliber_mm: 7.62,
        };
        let r = evaluate_predictive_era(&params, None);
        assert!(r.deployed, "P-ERA should deploy against KE threat");
        assert!(r.sensor_acquired, "Sensor should acquire at 30 m");
        assert!(
            r.within_time_gate,
            "5 s since last fire should be within gate"
        );
        assert!(
            r.residual_velocity_ms < params.threat_velocity_ms,
            "Flyer should reduce residual velocity"
        );
        assert!(
            r.effective_thickness_multiplier > 1.0,
            "ERA should provide thickness enhancement"
        );
    }

    #[test]
    fn predictive_era_heat_intercept_high_effectiveness() {
        // HEAT threat at 300 m/s, 20 m range.
        let params = PredictiveERAParams {
            era_type: PredictiveERAType::PredictiveERA {
                detection_range_m: 100.0,
                reaction_time_us: 0.15,
                flyer_velocity_ms: 800.0,
            },
            threat_velocity_ms: 300.0,
            threat_type: "heat",
            threat_range_m: 20.0,
            impact_angle_deg: 0.0,
            time_since_last_fire_s: 2.0,
            apfsds_tip_shedding_factor: 1.0,
            threat_caliber_mm: 30.0,
        };
        let r = evaluate_predictive_era(&params, None);
        assert!(r.deployed, "P-ERA should deploy against HEAT");
        assert!(
            r.effective_thickness_multiplier > 3.0,
            "P-ERA vs HEAT should give >3x multiplier, got {}",
            r.effective_thickness_multiplier
        );
        // HEAT jet should be significantly disrupted
        assert!(
            r.residual_velocity_ms < params.threat_velocity_ms * 0.5,
            "HEAT residual velocity should be <50% of incoming"
        );
    }

    #[test]
    fn predictive_era_out_of_sensor_range_no_deploy() {
        let params = PredictiveERAParams {
            era_type: PredictiveERAType::PredictiveERA {
                detection_range_m: 50.0,
                reaction_time_us: 0.15,
                flyer_velocity_ms: 1000.0,
            },
            threat_velocity_ms: 900.0,
            threat_type: "ke",
            threat_range_m: 200.0, // well beyond 50 m detection range
            impact_angle_deg: 0.0,
            time_since_last_fire_s: 5.0,
            apfsds_tip_shedding_factor: 1.0,
            threat_caliber_mm: 7.62,
        };
        let r = evaluate_predictive_era(&params, None);
        assert!(!r.deployed, "P-ERA should not deploy out of sensor range");
        assert!(!r.sensor_acquired, "Sensor should not acquire at 200 m");
        assert_eq!(
            r.residual_velocity_ms, params.threat_velocity_ms,
            "No intercept means full residual velocity"
        );
        assert_eq!(r.effective_thickness_multiplier, 1.0);
    }

    // ── Dynamic ERA tests ────────────────────────────────────────────────

    #[test]
    fn dynamic_era_ke_below_threshold_no_activation() {
        // Kontakt-5: KE threshold = 600 m/s.  A 300 m/s fragment should
        // not trigger the ERA.
        let params = PredictiveERAParams {
            era_type: PredictiveERAType::DynamicERA {
                threshold_ke_velocity_ms: KE_VELOCITY_GATE_MS,
                threshold_heat_velocity_ms: HEAT_VELOCITY_GATE_MS,
            },
            threat_velocity_ms: 300.0,
            threat_type: "ke",
            threat_range_m: 0.0, // irrelevant for D-ERA
            impact_angle_deg: 0.0,
            time_since_last_fire_s: 5.0,
            apfsds_tip_shedding_factor: 1.0,
            threat_caliber_mm: 7.62,
        };
        let r = evaluate_predictive_era(&params, None);
        assert!(!r.deployed, "D-ERA should not deploy below KE threshold");
        assert_eq!(
            r.effective_thickness_multiplier, 1.0,
            "Below threshold: no thickness enhancement"
        );
    }

    #[test]
    fn dynamic_era_ke_above_threshold_activates() {
        let params = PredictiveERAParams {
            era_type: PredictiveERAType::DynamicERA {
                threshold_ke_velocity_ms: KE_VELOCITY_GATE_MS,
                threshold_heat_velocity_ms: HEAT_VELOCITY_GATE_MS,
            },
            threat_velocity_ms: 1000.0,
            threat_type: "ke",
            threat_range_m: 0.0,
            impact_angle_deg: 0.0,
            time_since_last_fire_s: 5.0,
            apfsds_tip_shedding_factor: 1.0,
            threat_caliber_mm: 7.62,
        };
        let r = evaluate_predictive_era(&params, None);
        assert!(r.deployed, "D-ERA should deploy above KE threshold");
        assert!(
            r.effective_thickness_multiplier > 1.0,
            "D-ERA should enhance thickness"
        );
        // D-ERA has no flyer, so residual velocity is unchanged
        assert_eq!(
            r.residual_velocity_ms, params.threat_velocity_ms,
            "D-ERA (non-flyer) does not change residual velocity directly"
        );
    }

    #[test]
    fn dynamic_era_heat_below_threshold_no_activation() {
        let params = PredictiveERAParams {
            era_type: PredictiveERAType::DynamicERA {
                threshold_ke_velocity_ms: KE_VELOCITY_GATE_MS,
                threshold_heat_velocity_ms: HEAT_VELOCITY_GATE_MS,
            },
            threat_velocity_ms: 100.0,
            threat_type: "heat",
            threat_range_m: 0.0,
            impact_angle_deg: 0.0,
            time_since_last_fire_s: 5.0,
            apfsds_tip_shedding_factor: 1.0,
            threat_caliber_mm: 30.0,
        };
        let r = evaluate_predictive_era(&params, None);
        assert!(!r.deployed, "D-ERA should not deploy below HEAT threshold");
    }

    #[test]
    fn dynamic_era_heat_above_threshold_activates() {
        let params = PredictiveERAParams {
            era_type: PredictiveERAType::DynamicERA {
                threshold_ke_velocity_ms: KE_VELOCITY_GATE_MS,
                threshold_heat_velocity_ms: HEAT_VELOCITY_GATE_MS,
            },
            threat_velocity_ms: 500.0,
            threat_type: "heat",
            threat_range_m: 0.0,
            impact_angle_deg: 0.0,
            time_since_last_fire_s: 5.0,
            apfsds_tip_shedding_factor: 1.0,
            threat_caliber_mm: 30.0,
        };
        let r = evaluate_predictive_era(&params, None);
        assert!(r.deployed, "D-ERA should deploy above HEAT threshold");
    }

    // ── Time-gating tests ────────────────────────────────────────────────

    #[test]
    fn time_gate_blocks_own_round() {
        // Vehicle fired 0.05 s ago; gate is 0.2 s → should block.
        let params = PredictiveERAParams {
            era_type: PredictiveERAType::TimeGatedERA {
                gate_duration_s: 0.2,
                cooldown_s: 1.0,
            },
            threat_velocity_ms: 900.0,
            threat_type: "ke",
            threat_range_m: 30.0,
            impact_angle_deg: 0.0,
            time_since_last_fire_s: 0.05,
            apfsds_tip_shedding_factor: 1.0,
            threat_caliber_mm: 7.62,
        };
        let r = evaluate_predictive_era(&params, None);
        assert!(!r.within_time_gate, "Should be outside time gate");
        assert!(!r.deployed, "Time gate should block deployment");
    }

    #[test]
    fn time_gate_allows_after_dead_time() {
        // Vehicle fired 0.5 s ago; gate is 0.2 s → should allow.
        let params = PredictiveERAParams {
            era_type: PredictiveERAType::TimeGatedERA {
                gate_duration_s: 0.2,
                cooldown_s: 1.0,
            },
            threat_velocity_ms: 900.0,
            threat_type: "ke",
            threat_range_m: 30.0,
            impact_angle_deg: 0.0,
            time_since_last_fire_s: 0.5,
            apfsds_tip_shedding_factor: 1.0,
            threat_caliber_mm: 7.62,
        };
        let r = evaluate_predictive_era(&params, None);
        assert!(r.within_time_gate, "Should be within time gate");
    }

    // ── Hybrid ERA tests ─────────────────────────────────────────────────

    #[test]
    fn hybrid_era_above_threshold_deploys() {
        let params = PredictiveERAParams {
            era_type: PredictiveERAType::Hybrid {
                detection_range_m: 80.0,
                threshold_ms: 500.0,
                flyer_mass_kg: 0.8,
            },
            threat_velocity_ms: 900.0,
            threat_type: "ke",
            threat_range_m: 30.0,
            impact_angle_deg: 0.0,
            time_since_last_fire_s: 5.0,
            apfsds_tip_shedding_factor: 1.0,
            threat_caliber_mm: 12.7,
        };
        let r = evaluate_predictive_era(&params, None);
        assert!(r.deployed, "Hybrid should deploy above threshold");
        assert!(
            r.sensor_acquired,
            "Hybrid should acquire at 30 m / 80 m range"
        );
        assert!(
            r.flyer_plate_struck,
            "Hybrid flyer should strike the threat"
        );
        assert!(
            r.residual_velocity_ms < params.threat_velocity_ms,
            "Hybrid flyer should reduce residual velocity"
        );
    }

    #[test]
    fn hybrid_era_below_threshold_no_deploy() {
        let params = PredictiveERAParams {
            era_type: PredictiveERAType::Hybrid {
                detection_range_m: 80.0,
                threshold_ms: 500.0,
                flyer_mass_kg: 0.8,
            },
            threat_velocity_ms: 300.0, // below threshold
            threat_type: "ke",
            threat_range_m: 30.0,
            impact_angle_deg: 0.0,
            time_since_last_fire_s: 5.0,
            apfsds_tip_shedding_factor: 1.0,
            threat_caliber_mm: 12.7,
        };
        let r = evaluate_predictive_era(&params, None);
        assert!(!r.deployed, "Hybrid should not deploy below threshold");
        assert_eq!(r.effective_thickness_multiplier, 1.0);
    }

    // ── Timing optimality tests ──────────────────────────────────────────

    #[test]
    fn predictive_era_timing_optimal_at_close_range() {
        // At 30 m range with a 1200 m/s threat the system has ~25 ms
        // to react — far more than the ~50 µs the flyer needs to transit
        // the standoff.  The reaction-time margin is enormous, so timing
        // should be optimal and the flyer should strike.
        let params = PredictiveERAParams {
            era_type: PredictiveERAType::PredictiveERA {
                detection_range_m: 100.0,
                reaction_time_us: 0.15,
                flyer_velocity_ms: 1000.0,
            },
            threat_velocity_ms: 1200.0,
            threat_type: "ke",
            threat_range_m: 30.0,
            impact_angle_deg: 0.0,
            time_since_last_fire_s: 5.0,
            apfsds_tip_shedding_factor: 1.0,
            threat_caliber_mm: 7.62,
        };
        let r = evaluate_predictive_era(&params, None);
        assert!(r.deployed, "P-ERA should deploy");
        assert!(
            r.timing_optimal,
            "Timing should be optimal with ample reaction margin"
        );
        assert!(r.flyer_plate_struck, "Flyer should strike the threat");
        assert!(
            r.interceptor_effectiveness > 0.3,
            "Optimal timing should give interceptor_effectiveness > 0.3, got {}",
            r.interceptor_effectiveness
        );
    }

    #[test]
    fn predictive_era_timing_marginal_at_extreme_close_range() {
        // At sub-metre range the threat arrives before the flyer can
        // cross the standoff.  The flyer is too late, so it doesn't
        // strike and timing is non-optimal, even though the system
        // detects the threat.
        let params = PredictiveERAParams {
            era_type: PredictiveERAType::PredictiveERA {
                detection_range_m: 100.0,
                reaction_time_us: 0.15,
                flyer_velocity_ms: 1000.0,
            },
            threat_velocity_ms: 1200.0,
            threat_type: "ke",
            threat_range_m: 0.03, // 3 cm — threat arrives before flyer crosses standoff
            impact_angle_deg: 0.0,
            time_since_last_fire_s: 5.0,
            apfsds_tip_shedding_factor: 1.0,
            threat_caliber_mm: 7.62,
        };
        let r = evaluate_predictive_era(&params, None);
        assert!(r.deployed, "P-ERA should deploy (sensor acquired)");
        assert!(
            !r.timing_optimal,
            "Timing should be non-optimal at extreme close range"
        );
        assert!(
            !r.flyer_plate_struck,
            "Flyer should NOT strike — threat arrives too fast"
        );
    }

    #[test]
    fn predictive_era_timing_non_optimal_threat_too_fast_or_close() {
        // At very close range where the threat arrives before the flyer
        // can cross the standoff, timing is suboptimal.
        let params = PredictiveERAParams {
            era_type: PredictiveERAType::PredictiveERA {
                detection_range_m: 100.0,
                reaction_time_us: 0.15,
                flyer_velocity_ms: 1000.0,
            },
            threat_velocity_ms: 2000.0, // very fast threat
            threat_type: "ke",
            threat_range_m: 0.05, // only 5 cm — flyer can't cross standoff in time
            impact_angle_deg: 0.0,
            time_since_last_fire_s: 5.0,
            apfsds_tip_shedding_factor: 1.0,
            threat_caliber_mm: 7.62,
        };
        let r = evaluate_predictive_era(&params, None);
        // The system deploys (sensor acquired) but the flyer arrives at
        // the armour after the threat has already struck.
        if r.deployed {
            assert!(
                !r.flyer_plate_struck,
                "Flyer should not strike — threat arrives too fast"
            );
            assert!(!r.timing_optimal, "Timing should be non-optimal");
        }
    }

    // ── Determinism ───────────────────────────────────────────────────────

    #[test]
    fn evaluate_is_deterministic() {
        let params = PredictiveERAParams {
            era_type: PredictiveERAType::PredictiveERA {
                detection_range_m: 100.0,
                reaction_time_us: 0.15,
                flyer_velocity_ms: 1000.0,
            },
            threat_velocity_ms: 900.0,
            threat_type: "ke",
            threat_range_m: 30.0,
            impact_angle_deg: 5.0,
            time_since_last_fire_s: 3.0,
            apfsds_tip_shedding_factor: 1.0,
            threat_caliber_mm: 7.62,
        };
        let a = evaluate_predictive_era(&params, None);
        let b = evaluate_predictive_era(&params, None);
        assert_eq!(a, b, "predictive ERA evaluation must be deterministic");
    }

    // ── Edge cases ────────────────────────────────────────────────────────

    #[test]
    fn zero_velocity_returns_safe_default() {
        let params = PredictiveERAParams {
            era_type: PredictiveERAType::PredictiveERA {
                detection_range_m: 100.0,
                reaction_time_us: 0.15,
                flyer_velocity_ms: 1000.0,
            },
            threat_velocity_ms: 0.0,
            threat_type: "ke",
            threat_range_m: 30.0,
            impact_angle_deg: 0.0,
            time_since_last_fire_s: 5.0,
            apfsds_tip_shedding_factor: 1.0,
            threat_caliber_mm: 7.62,
        };
        let r = evaluate_predictive_era(&params, None);
        assert!(!r.deployed);
        assert!(!r.flyer_plate_struck);
        assert_eq!(r.effective_thickness_multiplier, 1.0);
        assert_eq!(r.residual_velocity_ms, 0.0);
    }

    #[test]
    fn missile_threat_against_predictive_era() {
        // A missile (e.g. ATGM) should be engaged by P-ERA with
        // intermediate effectiveness (between KE and HEAT).
        let params = PredictiveERAParams {
            era_type: PredictiveERAType::PredictiveERA {
                detection_range_m: 100.0,
                reaction_time_us: 0.15,
                flyer_velocity_ms: 1000.0,
            },
            threat_velocity_ms: 250.0,
            threat_type: "missile",
            threat_range_m: 40.0,
            impact_angle_deg: 0.0,
            time_since_last_fire_s: 5.0,
            apfsds_tip_shedding_factor: 1.0,
            threat_caliber_mm: 100.0,
        };
        let r = evaluate_predictive_era(&params, None);
        assert!(r.deployed, "P-ERA should engage missile threats");
        assert!(r.flyer_plate_struck, "Flyer should strike missile");
    }

    #[test]
    fn time_gate_combined_with_predictive_era_blocks_own_round() {
        // Simulate a PredictiveERA system enhanced with time-gating.
        // If the vehicle just fired, the ERA should not deploy even if
        // the sensor detects a threat (the detected object is likely
        // the vehicle's own round).
        let params = PredictiveERAParams {
            era_type: PredictiveERAType::PredictiveERA {
                detection_range_m: 100.0,
                reaction_time_us: 0.15,
                flyer_velocity_ms: 1000.0,
            },
            threat_velocity_ms: 900.0,
            threat_type: "ke",
            threat_range_m: 20.0, // well within detection range
            impact_angle_deg: 0.0,
            time_since_last_fire_s: 0.05, // just fired 50 ms ago
            apfsds_tip_shedding_factor: 1.0,
            threat_caliber_mm: 7.62,
        };
        let r = evaluate_predictive_era(&params, None);
        // Time-gate check uses DEFAULT_TIME_GATE_S (0.2 s) for non-TimeGatedERA types.
        // 0.05 s < 0.2 s → not within time gate
        assert!(
            !r.within_time_gate,
            "Own round should be blocked by time gate"
        );
        assert!(!r.deployed, "ERA should not deploy on own round");
    }

    // ── ERA zone selection tests ──────────────────────────────────────────

    #[test]
    fn select_zone_direct_hit_matches_hull_front() {
        let zones = vec![
            EraZoneConfig {
                zone_name: "hull_front".into(),
                coverage_angle_start_deg: 315.0,
                coverage_angle_end_deg: 45.0,
                era_material: "k5".into(),
                era_thickness_mm: 15.0,
                era_density_gcc: 3.0,
            },
            EraZoneConfig {
                zone_name: "side_right".into(),
                coverage_angle_start_deg: 45.0,
                coverage_angle_end_deg: 135.0,
                era_material: "k1".into(),
                era_thickness_mm: 10.0,
                era_density_gcc: 2.5,
            },
        ];

        let zone = select_era_zone(0.0, &zones);
        assert!(zone.is_some(), "0° should match a zone");
        assert_eq!(zone.unwrap().zone_name, "hull_front");
    }

    #[test]
    fn select_zone_side_hit_matches_side_right() {
        let zones = vec![
            EraZoneConfig {
                zone_name: "hull_front".into(),
                coverage_angle_start_deg: 315.0,
                coverage_angle_end_deg: 45.0,
                era_material: "k5".into(),
                era_thickness_mm: 15.0,
                era_density_gcc: 3.0,
            },
            EraZoneConfig {
                zone_name: "side_right".into(),
                coverage_angle_start_deg: 45.0,
                coverage_angle_end_deg: 135.0,
                era_material: "k1".into(),
                era_thickness_mm: 10.0,
                era_density_gcc: 2.5,
            },
        ];

        let zone = select_era_zone(90.0, &zones);
        assert!(zone.is_some(), "90° should match a zone");
        assert_eq!(zone.unwrap().zone_name, "side_right");
    }

    #[test]
    fn select_zone_no_match_returns_none() {
        let zones = vec![
            EraZoneConfig {
                zone_name: "hull_front".into(),
                coverage_angle_start_deg: 315.0,
                coverage_angle_end_deg: 45.0,
                era_material: "k5".into(),
                era_thickness_mm: 15.0,
                era_density_gcc: 3.0,
            },
            EraZoneConfig {
                zone_name: "side_right".into(),
                coverage_angle_start_deg: 45.0,
                coverage_angle_end_deg: 135.0,
                era_material: "k1".into(),
                era_thickness_mm: 10.0,
                era_density_gcc: 2.5,
            },
        ];

        assert!(select_era_zone(180.0, &zones).is_none());
    }

    #[test]
    fn select_zone_empty_slice_returns_none() {
        assert!(select_era_zone(0.0, &[]).is_none());
    }

    #[test]
    fn select_zone_rollover_boundary() {
        let zones = vec![EraZoneConfig {
            zone_name: "hull_front".into(),
            coverage_angle_start_deg: 315.0,
            coverage_angle_end_deg: 45.0,
            era_material: "k5".into(),
            era_thickness_mm: 15.0,
            era_density_gcc: 3.0,
        }];

        assert_eq!(
            select_era_zone(315.0, &zones).unwrap().zone_name,
            "hull_front"
        );
        assert_eq!(
            select_era_zone(359.0, &zones).unwrap().zone_name,
            "hull_front"
        );
        assert_eq!(
            select_era_zone(0.0, &zones).unwrap().zone_name,
            "hull_front"
        );
        assert_eq!(
            select_era_zone(45.0, &zones).unwrap().zone_name,
            "hull_front"
        );
        assert!(select_era_zone(90.0, &zones).is_none());
    }

    #[test]
    fn select_zone_negative_angle_normalises() {
        let zones = vec![EraZoneConfig {
            zone_name: "hull_front".into(),
            coverage_angle_start_deg: 315.0,
            coverage_angle_end_deg: 45.0,
            era_material: "k5".into(),
            era_thickness_mm: 15.0,
            era_density_gcc: 3.0,
        }];

        assert_eq!(
            select_era_zone(-45.0, &zones).unwrap().zone_name,
            "hull_front"
        );
        // -45° normalises to 315°, which is the start boundary.
        assert!(select_era_zone(-46.0, &zones).is_none());
    }

    #[test]
    fn select_zone_negative_start_angle_normalises() {
        // Zone config with negative start angle (e.g. -45° → 315°).
        let zones = vec![EraZoneConfig {
            zone_name: "hull_front".into(),
            coverage_angle_start_deg: -45.0,
            coverage_angle_end_deg: 45.0,
            era_material: "k5".into(),
            era_thickness_mm: 15.0,
            era_density_gcc: 3.0,
        }];

        assert_eq!(
            select_era_zone(0.0, &zones).unwrap().zone_name,
            "hull_front"
        );
        assert_eq!(
            select_era_zone(330.0, &zones).unwrap().zone_name,
            "hull_front"
        );
        assert!(select_era_zone(90.0, &zones).is_none());
        assert!(select_era_zone(180.0, &zones).is_none());
    }

    // ── Zone flyer mass tests ─────────────────────────────────────────────

    #[test]
    fn zone_flyer_mass_computed_from_density_and_thickness() {
        let zone = EraZoneConfig {
            zone_name: "test".into(),
            coverage_angle_start_deg: 0.0,
            coverage_angle_end_deg: 90.0,
            era_material: "k5".into(),
            era_thickness_mm: 15.0,
            era_density_gcc: 3.0,
        };
        let mass = zone_flyer_mass(&zone);
        let expected = 3.0 * 15.0 * 0.01;
        assert!(
            (mass - expected).abs() < 1e-12,
            "Expected {expected} kg, got {mass} kg"
        );
    }

    #[test]
    fn zone_flyer_mass_default_thickness() {
        let zone = EraZoneConfig {
            zone_name: "test".into(),
            coverage_angle_start_deg: 0.0,
            coverage_angle_end_deg: 90.0,
            era_material: "k1".into(),
            era_thickness_mm: 10.0,
            era_density_gcc: 2.5,
        };
        let mass = zone_flyer_mass(&zone);
        let expected = 2.5 * 10.0 * 0.01;
        assert!(
            (mass - expected).abs() < 1e-12,
            "Expected {expected} kg, got {mass} kg"
        );
    }

    // ── Zone material multiplier tests ────────────────────────────────────

    #[test]
    fn zone_material_multiplier_known_materials() {
        assert!((zone_material_multiplier("k1") - 1.0).abs() < 1e-12);
        assert!((zone_material_multiplier("K5") - 1.3).abs() < 1e-12);
        assert!((zone_material_multiplier("relikt") - 1.5).abs() < 1e-12);
        assert!((zone_material_multiplier("nora") - 1.2).abs() < 1e-12);
        assert!((zone_material_multiplier("heavy_era") - 1.4).abs() < 1e-12);
        assert!((zone_material_multiplier("LIGHT") - 0.8).abs() < 1e-12);
    }

    #[test]
    fn zone_material_multiplier_unknown_defaults_to_one() {
        assert!((zone_material_multiplier("unknown").abs() - 1.0).abs() < 1e-12);
        assert!((zone_material_multiplier("").abs() - 1.0).abs() < 1e-12);
    }
}
