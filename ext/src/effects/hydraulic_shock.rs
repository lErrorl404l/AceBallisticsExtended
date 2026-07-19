use crate::effects::soft_tissue::{CAVITY_CONSTANT, TISSUE_CD, TISSUE_DENSITY};

// ── Hydraulic Shock Model ─────────────────────────────────────────────────────

/// Result of a hydraulic shock evaluation from a projectile passing through
/// fluid-filled soft tissue.
#[derive(Debug, Clone, Copy)]
pub struct HydraulicShockResult {
    /// Peak pressure in the temporary cavity (kPa).
    pub peak_pressure_kpa: f64,
    /// Duration of the pressure wave (ms).
    pub pressure_duration_ms: f64,
    /// Distance the pressure wave propagates (m).
    pub pressure_wave_distance_m: f64,
    /// Probability of remote organ damage (0.0–1.0).
    pub remote_organ_damage_probability: f64,
    /// Risk of vascular/nerve disruption at a distance.
    pub vascular_disruption_risk: f64,
    /// Whether the shock is sufficient to cause incapacitation.
    pub incapacitating: bool,
}

/// Evaluate hydraulic shock effects from a projectile passing through
/// fluid-filled soft tissue (muscle, organs).
///
/// The temporary cavity acts as an explosive expansion, generating a
/// pressure wave that propagates through tissue. This can cause damage
/// remote from the wound track.
///
/// Physics:
/// - Peak pressure proportional to dE/dx: P = k_p × (dE/dx) / A
///   where k_p ≈ 0.01–0.03 for muscle tissue
/// - Pressure wave velocity ≈ speed of sound in tissue (~1540 m/s)
/// - Duration scales with temporary cavity radius / sound speed
/// - Remote organ damage: significant when peak pressure > 100 kPa
///   or energy deposition > 30 J/cm
pub fn evaluate_hydraulic_shock(
    velocity_ms: f64,
    mass_g: f64,
    caliber_m: f64,
    projectile_type: &str,
    yawed: bool,
) -> HydraulicShockResult {
    if velocity_ms <= 50.0 || mass_g <= 0.0 || caliber_m <= 0.0 {
        return HydraulicShockResult {
            peak_pressure_kpa: 0.0,
            pressure_duration_ms: 0.0,
            pressure_wave_distance_m: 0.0,
            remote_organ_damage_probability: 0.0,
            vascular_disruption_risk: 0.0,
            incapacitating: false,
        };
    }

    let area = std::f64::consts::PI * (caliber_m / 2.0).powi(2);
    let proj_lower = projectile_type.to_lowercase();

    // ── dE/dx at surface ───────────────────────────────────────────────
    let edex = 0.5 * TISSUE_DENSITY * area * TISSUE_CD * velocity_ms.powi(2);

    // Yaw multiplies dE/dx by 3-6×
    let yaw_mult = if yawed { 4.0 } else { 1.0 };
    let effective_edex = edex * yaw_mult;

    // Fragmenting rounds amplify shock
    let frag_mult = match proj_lower.as_str() {
        "soft_point" | "hollow_point" | "varmint" | "sp" => {
            if velocity_ms > 700.0 {
                1.5
            } else {
                1.2
            }
        },
        _ => 1.0,
    };
    let effective_edex = effective_edex * frag_mult;

    // ── Peak pressure ──────────────────────────────────────────────────
    // P = k_p × (dE/dx) / A, converted to kPa
    // Calibrated so 9mm FMJ (no yaw) produces ~30-80 kPa at cavity wall
    // and yawed rifle produces ~200-800 kPa.
    let k_p = 0.002;
    let peak_pressure_pa = if area > 0.0 {
        k_p * effective_edex / area
    } else {
        0.0
    };
    let peak_pressure_kpa = (peak_pressure_pa / 1000.0).clamp(0.0, 2000.0);

    // ── Pressure duration ──────────────────────────────────────────────
    // Duration ~ 2 × temp_cavity_radius / sound_speed
    // temp cavity: D_temp = 2 × CAVITY_CONSTANT × sqrt(dE/dx)
    let temp_cavity_diameter = 2.0 * CAVITY_CONSTANT * effective_edex.sqrt();
    let temp_cavity_radius = temp_cavity_diameter / 2.0;
    let sound_speed_tissue = 1540.0; // m/s
    let pressure_duration_s = 2.0 * temp_cavity_radius / sound_speed_tissue;
    let pressure_duration_ms = (pressure_duration_s * 1000.0).min(100.0);

    // ── Pressure wave distance ─────────────────────────────────────────
    // Wave attenuates as 1/r² in tissue. Effective range when P > 50 kPa.
    // r_max = sqrt(P_0 / P_threshold) × r_0
    let pressure_wave_distance_m = if peak_pressure_kpa > 50.0 {
        (peak_pressure_kpa / 50.0).sqrt() * temp_cavity_radius
    } else {
        0.0
    };
    let pressure_wave_distance_m = pressure_wave_distance_m.min(0.5);

    // ── Remote organ damage probability ────────────────────────────────
    // Significant when P > 100 kPa or energy deposition > 30 J/cm
    let edep_j_per_m = effective_edex;
    let edep_j_per_cm = edep_j_per_m * 0.01;

    let prob_pressure = (peak_pressure_kpa / 200.0).min(1.0);
    let prob_edep = ((edep_j_per_cm - 15.0) / 40.0).clamp(0.0, 1.0);
    let remote_organ_damage_probability = prob_pressure.max(prob_edep).min(1.0);

    // ── Vascular disruption risk ──────────────────────────────────────
    let vascular_disruption_risk = (peak_pressure_kpa / 500.0).min(1.0) * yaw_mult * 0.3;

    // ── Incapacitation ─────────────────────────────────────────────────
    let incapacitating = peak_pressure_kpa > 300.0 && edep_j_per_cm > 30.0;

    HydraulicShockResult {
        peak_pressure_kpa,
        pressure_duration_ms,
        pressure_wave_distance_m,
        remote_organ_damage_probability,
        vascular_disruption_risk,
        incapacitating,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hydraulic_shock_rifle_yawed() {
        let r = evaluate_hydraulic_shock(930.0, 4.0, 0.00556, "fmj", true);
        assert!(
            r.peak_pressure_kpa > 100.0,
            "Yawed rifle should produce > 100 kPa: {:.0} kPa",
            r.peak_pressure_kpa
        );
        assert!(
            r.remote_organ_damage_probability > 0.0,
            "Should have some remote organ damage probability"
        );
    }

    #[test]
    fn hydraulic_shock_handgun_minimal() {
        let r = evaluate_hydraulic_shock(360.0, 8.0, 0.00901, "fmj", false);
        assert!(
            r.peak_pressure_kpa < 200.0,
            "Handgun shock should be mild: {:.0} kPa",
            r.peak_pressure_kpa
        );
    }

    #[test]
    fn hydraulic_shock_incapacitating_threshold() {
        let r = evaluate_hydraulic_shock(930.0, 4.0, 0.00556, "fmj", true);
        assert!(
            r.incapacitating || r.peak_pressure_kpa > 200.0,
            "Yawed rifle should cause significant shock"
        );
    }

    #[test]
    fn hydraulic_shock_zero_velocity() {
        let r = evaluate_hydraulic_shock(0.0, 8.0, 0.00901, "fmj", false);
        assert_eq!(r.peak_pressure_kpa, 0.0);
        assert!(!r.incapacitating);
    }

    #[test]
    fn hydraulic_shock_yaw_increases_pressure() {
        let no_yaw = evaluate_hydraulic_shock(800.0, 9.5, 0.00762, "fmj", false);
        let with_yaw = evaluate_hydraulic_shock(800.0, 9.5, 0.00762, "fmj", true);
        assert!(
            with_yaw.peak_pressure_kpa > no_yaw.peak_pressure_kpa,
            "Yaw should increase peak pressure"
        );
    }
}
