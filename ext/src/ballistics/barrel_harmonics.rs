// ABE - Barrel Harmonics / Vertical Stringing Model
//
// Models the transverse vibrations of a rifle barrel during projectile
// transit. The barrel acts as a cantilevered beam; the travelling bullet
// excites bending modes whose amplitude depends on barrel profile
// (stiffness, mass distribution), projectile mass, and the pressure
// curve timing.
//
// When the bullet exits at a point where the barrel tip is moving
// vertically, it imparts a vertical velocity component that changes the
// point of impact. Shot-to-shot variation in this exit timing creates
// vertical stringing — the characteristic elliptical dispersion pattern
// of a cold-bore vs. fouled barrel.
//
// References:
//   - Bryan Litz, Applied Ballistics Precision (ch. 11 — barrel harmonics)
//   - McCoy, Modern Exterior Ballistics (appendix — vibration analysis)
//   - Timoshenko, Vibration Problems in Engineering (beam theory)
//   - Hatcher's Notebook (barrel whip / vertical stringing observations)
//   - ANSI/SAAMI barrel dimension standards

#![allow(dead_code)]

/// Parameters describing the barrel and projectile for harmonic analysis.
pub struct BarrelHarmonicsParams {
    /// Barrel length in millimetres (muzzle to breech face).
    pub barrel_length_mm: f64,
    /// Barrel profile identifier. Controls stiffness and mass distribution.
    ///   "pencil"  — thin, lightweight barrel; maximum whip
    ///   "contour" — medium profile; moderate whip
    ///   "heavy"   — stiffer profile; reduced whip
    ///   "bull"    — maximum stiffness; minimal whip
    pub barrel_profile: &'static str,
    /// Projectile mass in grams.
    pub projectile_mass_g: f64,
    /// Propellant charge mass in grams.
    pub charge_mass_g: f64,
    /// Muzzle velocity in m/s.
    pub muzzle_velocity_ms: f64,
    /// Rifling twist rate in rev/m.
    pub twist_rate_rev_per_m: f64,
    /// Number of shots fired since the last bore cleaning.
    /// Fouling increases effective barrel mass, lowering resonance.
    pub round_count_since_clean: i32,
    /// Barrel temperature in degrees Celsius.
    /// Heat softens the barrel and causes thermal expansion.
    pub barrel_temp_c: f64,
}

/// Result of the barrel harmonics evaluation.
pub struct BarrelHarmonicsResult {
    /// Vertical velocity imparted to the projectile by the barrel tip
    /// at the moment of exit (m/s). Positive = upward.
    pub muzzle_vertical_velocity_ms: f64,
    /// Additional vertical dispersion in MOA from barrel whip.
    pub vertical_dispersion_moa: f64,
    /// Ratio of vertical-to-horizontal dispersion.
    /// 1.0 = circular dispersion, > 1.0 = vertical stringing.
    pub vertical_stringing_ratio: f64,
    /// Primary (fundamental) transverse vibration frequency of the
    /// barrel in Hz.
    pub dominant_frequency_hz: f64,
    /// Barrel tip deflection at the moment of bullet exit (mm).
    /// Positive = upward.
    pub tip_deflection_mm: f64,
    /// Alert flag indicating that the charge mass is close to a
    /// resonant node where barrel whip is minimised (i.e. the
    /// bullet exits at a vibration node).
    pub resonant_charge_alert: bool,
}

/// Estimate the vertical velocity of the barrel tip at the moment
/// the projectile exits the muzzle.
///
/// The barrel is modelled as a cantilevered beam whose fundamental
/// bending frequency depends on its geometry and material. The tip
/// velocity scales with projectile mass and muzzle velocity (which
/// together determine the forcing impulse) and decreases with
/// increasing barrel stiffness.
///
/// Profile stiffness multipliers (relative to a pencil barrel):
///   pencil  → 1.0
///   contour → 1.6
///   heavy  → 2.5
///   bull   → 3.5
pub fn barrel_tip_velocity(length_mm: f64, profile: &str, mass_g: f64, mv_ms: f64) -> f64 {
    if length_mm <= 0.0 || mass_g <= 0.0 || mv_ms <= 0.0 {
        return 0.0;
    }

    // Stiffness multiplier based on barrel profile
    let stiffness: f64 = match profile {
        "bull" => 3.5,
        "heavy" => 2.5,
        "contour" => 1.6,
        _ => 1.0, // pencil or unknown = baseline
    };

    // The barrel tip velocity is proportional to:
    //   v_tip = k · (m_proj · MV) / (stiffness · L²)
    // where the numerator is the impulse and the denominator is
    // the beam bending stiffness (E·I) ~ stiffness · L².
    // Length in metres for dimensional consistency.
    let l_m = length_mm / 1000.0;
    let mass_kg = mass_g / 1000.0;
    let impulse = mass_kg * mv_ms;

    // Empirical constant (calibrated from typical 5.56 mm data:
    // M4 with pencil barrel: v_tip ≈ 0.3–0.5 m/s).
    const K_VEL: f64 = 3.2e-3;
    let v_tip = K_VEL * impulse / (stiffness * l_m * l_m);

    // Sign: positive = upward at the muzzle
    v_tip
}

/// Evaluate the barrel harmonics for the given weapon and ammunition
/// parameters.
///
/// The model computes:
///   1. Barrel tip deflection and velocity at bullet exit, based on
///      a cantilever beam model with profile-dependent stiffness.
///   2. The fundamental bending frequency from beam theory.
///   3. Vertical dispersion and stringing ratio from the whip
///      amplitude and shot-to-shot variation in exit timing.
///   4. A resonance check: when the charge mass corresponds to a
///      bullet exit time that aligns with a vibration node,
///      muzzle whip is minimised.
pub fn evaluate_barrel_harmonics(params: &BarrelHarmonicsParams) -> BarrelHarmonicsResult {
    let tip_vel = barrel_tip_velocity(
        params.barrel_length_mm,
        params.barrel_profile,
        params.projectile_mass_g,
        params.muzzle_velocity_ms,
    );

    // Barrel transit time (approximate: the bullet accelerates, so
    // the average velocity is less than muzzle velocity).
    // Average velocity ≈ MV / sqrt(2) for uniform acceleration.
    let avg_vel = params.muzzle_velocity_ms / 2.0_f64.sqrt();
    let barrel_time_s = if avg_vel > 0.0 {
        (params.barrel_length_mm / 1000.0) / avg_vel
    } else {
        0.0
    };

    // ── Dominant frequency ──────────────────────────────────────────────
    // Fundamental frequency of a cantilever beam:
    //   f₁ = (1.875² / (2π · L²)) · sqrt(E·I / (ρ·A))
    // For a steel barrel (E ≈ 200 GPa, ρ ≈ 7800 kg/m³) with the
    // profile approximated as a uniform beam of average diameter.
    let l_m = params.barrel_length_mm / 1000.0;
    let stiffness: f64 = match params.barrel_profile {
        "bull" => 3.5,
        "heavy" => 2.5,
        "contour" => 1.6,
        _ => 1.0,
    };
    // Base frequency for a pencil barrel ≈ 100 / L² (empirical fit
    // to Timoshenko beam for typical barrel cross-sections).
    // Derived: for a 0.368 m pencil barrel, f ≈ 100/0.135 ≈ 740 Hz.
    const BASE_FREQ_COEFF: f64 = 100.0;

    // Temperature correction: Young's modulus decreases with temperature.
    // For steel: E(T) = E₂₀ · (1 - α_E · (T - 20))
    // α_E ≈ 3.5e-4 /°C for carbon steel.
    let temp_diff = params.barrel_temp_c - 20.0;
    let e_correction: f64 = 1.0 - 3.5e-4 * temp_diff.clamp(-50.0, 300.0);

    // Thermal expansion: L(T) = L₂₀ · (1 + α_L · (T - 20))
    // α_L ≈ 1.2e-5 /°C for steel.
    let l_correction: f64 = 1.0 + 1.2e-5 * temp_diff.clamp(-50.0, 300.0);

    // Fouling adds effective mass, lowering resonance:
    // fouling_correction ≈ 1 - 0.001 · rounds / (1 + 0.001 · rounds)
    // Starts negligible, saturates at ~3-5% reduction.
    let fouling_mass_ratio: f64 = (params.round_count_since_clean as f64) * 0.001;
    let fouling_correction = (1.0_f64 / (1.0 + fouling_mass_ratio)).sqrt();

    let dom_freq = BASE_FREQ_COEFF / (l_m * l_m) * e_correction.sqrt() / l_correction
        * f64::sqrt(stiffness)
        * fouling_correction;

    // ── Tip deflection at bullet exit ────────────────────────────────────
    // For a sinusoidal vibration, the tip displacement is:
    //   δ(t) = A · sin(2π · f · t)
    // and the tip velocity is the time derivative:
    //   v(t) = A · 2π · f · cos(2π · f · t)
    // At the moment of bullet exit (t = barrel_time_s), we have the
    // tip velocity from barrel_tip_velocity. We can estimate the
    // amplitude from the velocity:
    //   A = v_tip / (2π · f)   (assuming exit at a velocity antinode)
    let amplitude_m = if dom_freq > 1.0 {
        (tip_vel.abs() / (2.0 * std::f64::consts::PI * dom_freq)).max(1e-12)
    } else {
        1e-12
    };

    // The actual deflection at exit depends on the phase angle.
    // For a worst-case estimate (exit at maximum displacement):
    let tip_deflection_m = amplitude_m
        * (2.0 * std::f64::consts::PI * dom_freq * barrel_time_s)
            .sin()
            .abs();
    let tip_deflection_mm = tip_deflection_m * 1000.0;

    // ── Vertical dispersion (MOA) ────────────────────────────────────────
    // The vertical velocity imparted by the barrel tip translates
    // to an angular dispersion at the target:
    //   θ ≈ v_tip / MV   (small angle approximation)
    // Convert to MOA: MOA = θ_rad · 3437.75
    let vertical_angle_rad = tip_vel.abs() / params.muzzle_velocity_ms.max(1.0);
    const RAD_TO_MOA: f64 = 3437.746770784939; // 180/π × 60

    // Phase factor at bullet exit: 0 = node (minimal whip), 1 = antinode.
    // Fouling and heat shift the frequency, changing the exit phase and
    // therefore the effective whip amplitude.
    let phase = (2.0 * std::f64::consts::PI * dom_freq.max(1.0) * barrel_time_s).sin();
    let phase_factor = phase.abs();

    // Dispersion is modulated by the phase: when bullet exits at an
    // antinode (phase_factor ≈ 1), the full tip velocity transfers.
    // At a node (phase_factor ≈ 0), whip is suppressed.
    // The 0.5 floor prevents the dispersion from going to zero entirely
    // (there is always some residual from non-ideal behavior).
    let vertical_dispersion_moa = vertical_angle_rad * RAD_TO_MOA * (0.5 + 0.5 * phase_factor);

    // ── Vertical stringing ratio ────────────────────────────────────────
    // Ratio of vertical to horizontal dispersion. Pencil barrels
    // have high ratios; bull barrels approach 1.0.
    let stringing_ratio = match params.barrel_profile {
        "bull" => 1.05 + vertical_dispersion_moa * 0.02,
        "heavy" => 1.15 + vertical_dispersion_moa * 0.05,
        "contour" => 1.4 + vertical_dispersion_moa * 0.10,
        _ => 1.8 + vertical_dispersion_moa * 0.15,
    };

    // ── Resonant charge check ────────────────────────────────────────────
    // When the barrel transit time equals an integer multiple of
    // the vibration half-period, the bullet exits at a node (minimal
    // whip). This occurs at specific charge masses that produce the
    // right muzzle velocity for the transit time to align.
    let half_period_s = 1.0 / (2.0 * dom_freq.max(1.0));
    let transit_in_half_periods = barrel_time_s / half_period_s;
    let distance_to_node = (transit_in_half_periods - transit_in_half_periods.round()).abs();
    // Within 5 % of a half-period → at a node
    let resonant_charge_alert = distance_to_node < 0.05;

    BarrelHarmonicsResult {
        muzzle_vertical_velocity_ms: tip_vel,
        vertical_dispersion_moa,
        vertical_stringing_ratio: stringing_ratio,
        dominant_frequency_hz: dom_freq,
        tip_deflection_mm,
        resonant_charge_alert,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_params(
        length_mm: f64,
        profile: &'static str,
        mass_g: f64,
        charge_g: f64,
        mv_ms: f64,
        twist: f64,
        rounds: i32,
        temp_c: f64,
    ) -> BarrelHarmonicsParams {
        BarrelHarmonicsParams {
            barrel_length_mm: length_mm,
            barrel_profile: profile,
            projectile_mass_g: mass_g,
            charge_mass_g: charge_g,
            muzzle_velocity_ms: mv_ms,
            twist_rate_rev_per_m: twist,
            round_count_since_clean: rounds,
            barrel_temp_c: temp_c,
        }
    }

    // ── Profile comparison ──────────────────────────────────────────────

    #[test]
    fn pencil_barrel_more_whip_than_heavy() {
        let pencil = evaluate_barrel_harmonics(&make_params(
            368.0, "pencil", 4.0, 1.8, 930.0, 5.62, 0, 20.0,
        ));
        let heavy =
            evaluate_barrel_harmonics(&make_params(368.0, "heavy", 4.0, 1.8, 930.0, 5.62, 0, 20.0));
        assert!(
            pencil.muzzle_vertical_velocity_ms.abs() > heavy.muzzle_vertical_velocity_ms.abs(),
            "pencil barrel should whip more: pencil={}, heavy={}",
            pencil.muzzle_vertical_velocity_ms,
            heavy.muzzle_vertical_velocity_ms
        );
    }

    #[test]
    fn pencil_barrel_higher_stringing_ratio() {
        let pencil = evaluate_barrel_harmonics(&make_params(
            368.0, "pencil", 4.0, 1.8, 930.0, 5.62, 0, 20.0,
        ));
        let bull =
            evaluate_barrel_harmonics(&make_params(368.0, "bull", 4.0, 1.8, 930.0, 5.62, 0, 20.0));
        assert!(
            pencil.vertical_stringing_ratio > bull.vertical_stringing_ratio,
            "pencil should have higher stringing ratio: pencil={}, bull={}",
            pencil.vertical_stringing_ratio,
            bull.vertical_stringing_ratio
        );
    }

    #[test]
    fn bull_barrel_near_circular_dispersion() {
        let bull =
            evaluate_barrel_harmonics(&make_params(660.0, "bull", 10.0, 3.5, 850.0, 3.0, 0, 20.0));
        assert!(
            bull.vertical_stringing_ratio < 1.3,
            "bull barrel should be near circular: {}",
            bull.vertical_stringing_ratio
        );
    }

    // ── Fouling shifts resonance ────────────────────────────────────────

    #[test]
    fn fouling_reduces_dominant_frequency() {
        let clean = evaluate_barrel_harmonics(&make_params(
            508.0, "contour", 4.0, 1.8, 930.0, 5.62, 0, 20.0,
        ));
        let fouled = evaluate_barrel_harmonics(&make_params(
            508.0, "contour", 4.0, 1.8, 930.0, 5.62, 2000, 20.0,
        ));
        assert!(
            fouled.dominant_frequency_hz < clean.dominant_frequency_hz,
            "fouling should reduce frequency: clean={}, fouled={}",
            clean.dominant_frequency_hz,
            fouled.dominant_frequency_hz
        );
    }

    #[test]
    fn fouling_increases_vertical_dispersion() {
        let clean = evaluate_barrel_harmonics(&make_params(
            368.0, "pencil", 4.0, 1.8, 930.0, 5.62, 0, 20.0,
        ));
        let fouled = evaluate_barrel_harmonics(&make_params(
            368.0, "pencil", 4.0, 1.8, 930.0, 5.62, 2000, 20.0,
        ));
        assert!(
            fouled.vertical_dispersion_moa > clean.vertical_dispersion_moa,
            "fouling should increase dispersion: clean={}, fouled={}",
            clean.vertical_dispersion_moa,
            fouled.vertical_dispersion_moa
        );
    }

    // ── Heat changes harmonics ──────────────────────────────────────────

    #[test]
    fn heat_reduces_dominant_frequency() {
        let cold = evaluate_barrel_harmonics(&make_params(
            508.0, "contour", 4.0, 1.8, 930.0, 5.62, 0, -10.0,
        ));
        let hot = evaluate_barrel_harmonics(&make_params(
            508.0, "contour", 4.0, 1.8, 930.0, 5.62, 0, 150.0,
        ));
        assert!(
            hot.dominant_frequency_hz < cold.dominant_frequency_hz,
            "heat should reduce frequency: cold={}, hot={}",
            cold.dominant_frequency_hz,
            hot.dominant_frequency_hz
        );
    }

    #[test]
    fn heat_increases_vertical_dispersion() {
        let cold = evaluate_barrel_harmonics(&make_params(
            368.0, "pencil", 4.0, 1.8, 930.0, 5.62, 0, -10.0,
        ));
        let hot = evaluate_barrel_harmonics(&make_params(
            368.0, "pencil", 4.0, 1.8, 930.0, 5.62, 0, 150.0,
        ));
        assert!(
            hot.vertical_dispersion_moa > cold.vertical_dispersion_moa,
            "heat should increase dispersion: cold={}, hot={}",
            cold.vertical_dispersion_moa,
            hot.vertical_dispersion_moa
        );
    }

    // ── Edge cases ──────────────────────────────────────────────────────

    #[test]
    fn zero_charge_no_whip() {
        // No projectile mass → no impulse → no whip
        let result =
            evaluate_barrel_harmonics(&make_params(368.0, "pencil", 0.0, 0.0, 0.0, 5.62, 0, 20.0));
        assert!(
            result.muzzle_vertical_velocity_ms.abs() < 1e-12,
            "zero mass should have zero whip: {}",
            result.muzzle_vertical_velocity_ms
        );
    }

    #[test]
    fn muzzle_vertical_velocity_sign_is_reasonable() {
        // The sign of the tip velocity depends on the phase at exit;
        // it should be non-zero for a realistic test case.
        let result = evaluate_barrel_harmonics(&make_params(
            368.0, "contour", 4.0, 1.8, 930.0, 5.62, 0, 20.0,
        ));
        assert!(
            result.muzzle_vertical_velocity_ms != 0.0,
            "non-zero whip expected"
        );
    }

    // ── Typical M4 vs. M24 ──────────────────────────────────────────────

    #[test]
    fn typical_m4_carbine_harmonics() {
        // M4 carbine: 368 mm pencil barrel, 5.56 mm, 4.0 g, 930 m/s
        let result = evaluate_barrel_harmonics(&make_params(
            368.0, "pencil", 4.0, 1.8, 930.0, 5.62, 0, 20.0,
        ));
        // Vertical dispersion should be measurable but reasonable
        assert!(
            result.vertical_dispersion_moa > 0.01,
            "M4 should have measurable dispersion: {} MOA",
            result.vertical_dispersion_moa
        );
        assert!(
            result.dominant_frequency_hz > 100.0,
            "M4 should have fundamental > 100 Hz: {}",
            result.dominant_frequency_hz
        );
    }

    #[test]
    fn typical_m24_harmonics() {
        // M24 SWS: 660 mm heavy barrel, 7.62 mm, 10.0 g, 850 m/s
        let result =
            evaluate_barrel_harmonics(&make_params(660.0, "heavy", 10.0, 3.5, 850.0, 3.0, 0, 20.0));
        // Heavy barrel → less whip than pencil
        assert!(
            result.vertical_stringing_ratio < 1.8,
            "M24 heavy barrel should have modest stringing: {}",
            result.vertical_stringing_ratio
        );
        // Longer barrel → lower fundamental frequency
        assert!(
            result.dominant_frequency_hz < 600.0,
            "M24 dominant frequency < 600 Hz: {}",
            result.dominant_frequency_hz
        );
    }

    #[test]
    fn resonant_charge_detected() {
        // Specific charge masses can align transit time with vibration
        // nodes. We verify the alert flag is triggered for some params.
        // Use a fine grid over a wide charge range to hit a node.
        let mut found_resonant = false;
        for charge_idx in 0..200 {
            let charge = 0.25 + charge_idx as f64 * 0.05;
            // Compute approximate MV from charge mass (linear approx)
            let mv = 200.0 + charge * 180.0;
            let result = evaluate_barrel_harmonics(&make_params(
                508.0, "contour", 4.0, charge, mv, 5.62, 0, 20.0,
            ));
            if result.resonant_charge_alert {
                found_resonant = true;
                break;
            }
        }
        assert!(
            found_resonant,
            "at least one charge mass should be near resonance"
        );
    }
}
