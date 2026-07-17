// ABE - Muzzle Velocity Temperature Sensitivity
//
// Models the effect of ambient/cartridge temperature on muzzle velocity.
// Propellant burn rate is temperature-dependent: cold propellant burns slower
// (lower peak pressure → lower MV), hot propellant burns faster (higher MV).
//
// Reference:
//   - NATO STANAG 4110 (Muzzle Velocity Temperature Sensitivity)
//   - Bryan Litz "Applied Ballistics Precision" Ch. 12 (Temperature Effects)
//   - US Army TDPs for M80, M855, M118LR (publish temp coefficients)
//
// Standard reference temperature for most ammunition: 21 °C (70 °F).
// Typical temp coefficient: 0.6–1.2 m/s per °C (varies by propellant).

/// Default reference (standard) temperature in °C.
pub const STANDARD_TEMP_C: f64 = 21.0;

/// Default temperature coefficient for NATO propellants (m/s per °C).
pub const DEFAULT_TEMP_COEFF: f64 = 0.9;

/// Temperature coefficient in m/s per °C for a given propellant type.
///
/// Single-base (nitrocellulose only): lower sensitivity
/// Double-base (NC + NG): moderate sensitivity
/// Ball propellant: higher sensitivity (more surface area per mass)
///
/// # Arguments
/// * `propellant_type` — Identifier string. Supported: "nato_single_base",
///   "nato_double_base", "ball", "smokeless_pistol", "high_temp_stable".
///   Unknown strings return the default 0.9 m/s/°C.
pub fn cartridge_temp_sensitivity(propellant_type: &str) -> f64 {
    match propellant_type {
        "nato_single_base" => 0.6,      // IMR 4895, HXP — stable, low sensitivity
        "nato_double_base" => 0.8,      // WC844, WC846 — M855 / M80 class
        "ball" => 1.0,                  // Ball powders: fast surface-area increase
        "smokeless_pistol" => 0.7,      // Fast pistol powders (Bullseye, Titegroup)
        "high_temp_stable" => 0.4,      // Temperature-stable (Norma, some match ammo)
        "nato_single_base_cold" => 0.5, // Cold-bore optimized single base
        _ => DEFAULT_TEMP_COEFF,        // Fallback for unknown types
    }
}

/// Correct muzzle velocity for cartridge/ambient temperature.
///
/// The correction is linear in the normal range (−20 °C to 50 °C), with
/// degressive behaviour in extreme cold (propellant burns less efficiently
/// as temperature drops below −20 °C) and progressive behaviour in extreme
/// heat (burn rate accelerates as temperature approaches cook-off near
/// 70 °C+).
///
/// # Arguments
/// * `temp_celsius` — Current cartridge/ambient temperature in °C.
/// * `standard_temp_c` — Reference (standard) temperature in °C.
///   Use `STANDARD_TEMP_C` (21 °C) for most NATO ammunition.
/// * `reference_mv_ms` — Published muzzle velocity at standard_temp_c
///   (m/s).
/// * `temp_coeff` — Temperature coefficient in m/s per °C. Use
///   `DEFAULT_TEMP_COEFF` (0.9) or the result of
///   `cartridge_temp_sensitivity()`.
///
/// # Returns
/// Corrected muzzle velocity in m/s.
pub fn mv_temperature_correction(
    temp_celsius: f64,
    standard_temp_c: f64,
    reference_mv_ms: f64,
    temp_coeff: f64,
) -> f64 {
    let delta_t = temp_celsius - standard_temp_c;

    // Apply non-linear scaling factor based on temperature regime.
    // Linear coefficient applies in the normal range (−20 °C to 50 °C).
    // Extreme cold (< −20 °C): degressive — rate drops to ~0.4 m/s/°C.
    // Extreme heat (> 50 °C): progressive — rate increases to ~1.4 m/s/°C.
    let effective_coeff = if temp_celsius < -20.0 {
        // Degressive below −20 °C: interpolate from `temp_coeff` down to 0.4
        let depth = (-20.0 - temp_celsius) / 40.0; // how far below −20 (cap at 40 °C delta)
        let depth = depth.min(1.0);
        temp_coeff - (temp_coeff - 0.4) * depth
    } else if temp_celsius > 50.0 {
        // Progressive above 50 °C: interpolate from `temp_coeff` up to 1.4
        // but never exceed 2× the base coefficient or 1.6 (cook-off proximity).
        let rise = (temp_celsius - 50.0) / 30.0; // how far above 50 (cap at 30 °C delta)
        let rise = rise.min(1.0);
        let max_coeff = (temp_coeff * 2.0).min(1.6);
        temp_coeff + (max_coeff - temp_coeff) * rise
    } else {
        temp_coeff
    };

    reference_mv_ms + delta_t * effective_coeff
}

/// Convenience wrapper: temperature correction using 21 °C standard and
/// default NATO coefficient (0.9 m/s/°C).
pub fn mv_temp_standard(temp_celsius: f64, reference_mv_ms: f64) -> f64 {
    mv_temperature_correction(
        temp_celsius,
        STANDARD_TEMP_C,
        reference_mv_ms,
        DEFAULT_TEMP_COEFF,
    )
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: round to 1 decimal for readable assertions
    fn r1(v: f64) -> f64 {
        (v * 10.0).round() / 10.0
    }

    #[test]
    fn standard_temp_no_change() {
        // At 21 °C reference, MV should be unchanged
        let mv = mv_temperature_correction(21.0, 21.0, 800.0, 0.9);
        assert!((mv - 800.0).abs() < 1e-10, "21 °C → no change: {}", mv);
    }

    #[test]
    fn cold_temp_reduces_mv() {
        // −20 °C with standard NATO coefficient:
        //   delta_t = −20 − 21 = −41 °C
        //   mv = 800 + (−41) × 0.9 = 800 − 36.9 ≈ 763.1 m/s
        let mv = mv_temperature_correction(-20.0, 21.0, 800.0, 0.9);
        let expected = 800.0 + (-41.0) * 0.9;
        assert!(
            (mv - expected).abs() < 0.1,
            "−20 °C: expected {:.1}, got {:.1}",
            expected,
            mv
        );
    }

    #[test]
    fn hot_temp_increases_mv() {
        // 50 °C with standard NATO coefficient:
        //   delta_t = 50 − 21 = 29 °C
        //   mv = 800 + 29 × 0.9 = 800 + 26.1 = 826.1 m/s
        let mv = mv_temperature_correction(50.0, 21.0, 800.0, 0.9);
        let expected = 800.0 + 29.0 * 0.9;
        assert!(
            (mv - expected).abs() < 0.1,
            "50 °C: expected {:.1}, got {:.1}",
            expected,
            mv
        );
    }

    #[test]
    fn extreme_cold_degressive() {
        // −30 °C: below −20 °C threshold, coefficient should drop
        // toward 0.4. At −30 °C (10 °C below threshold), with baseline 0.9:
        //   depth = (−20 − (−30)) / 40 = 0.25
        //   effective_coeff = 0.9 − (0.9 − 0.4) × 0.25 = 0.9 − 0.125 = 0.775
        //   delta_t = −30 − 21 = −51
        //   mv = 800 + (−51) × 0.775 = 800 − 39.525 = 760.475
        //
        // With a purely linear model: mv = 800 + (−51) × 0.9 = 754.1
        // The degressive model should give a HIGHER MV (less loss) in extreme cold
        let linear = 800.0 + (-51.0) * 0.9; // 754.1
        let degressive = mv_temperature_correction(-30.0, 21.0, 800.0, 0.9);

        assert!(
            degressive > linear,
            "Extreme cold degressive: MV should be higher than linear model ({} vs {})",
            degressive,
            linear
        );
        assert!(
            degressive < 800.0,
            "Extreme cold should still reduce MV: {}",
            degressive
        );

        // At −60 °C (saturated), effective coeff ≈ 0.4
        let saturated = mv_temperature_correction(-60.0, 21.0, 800.0, 0.9);
        let sat_delta = -60.0 - 21.0; // −81
        let sat_expected = 800.0 + sat_delta * 0.4; // 800 − 32.4 = 767.6
        assert!(
            (saturated - sat_expected).abs() < 1.0,
            "−60 °C saturated degressive: expected ~{:.1}, got {:.1}",
            sat_expected,
            saturated
        );
    }

    #[test]
    fn extreme_heat_progressive() {
        // 60 °C: above 50 °C threshold, coefficient should increase.
        //   rise = (60 − 50) / 30 = 0.333...
        //   max_coeff = min(0.9 × 2, 1.6) = 1.6
        //   effective_coeff = 0.9 + (1.6 − 0.9) × 0.333 = 0.9 + 0.233 = 1.133
        //   delta_t = 60 − 21 = 39
        //   mv = 800 + 39 × 1.133 = 800 + 44.2 = 844.2
        //
        // With a purely linear model: mv = 800 + 39 × 0.9 = 835.1
        // The progressive model should give a HIGHER MV (more boost) in extreme heat
        let linear = 800.0 + 39.0 * 0.9; // 835.1
        let progressive = mv_temperature_correction(60.0, 21.0, 800.0, 0.9);

        assert!(
            progressive > linear,
            "Extreme heat progressive: MV should be higher than linear model ({} vs {})",
            progressive,
            linear
        );
        assert!(
            progressive > 800.0,
            "Extreme heat should increase MV: {}",
            progressive
        );

        // At 80 °C (saturated), effective coeff ≈ 1.6
        let saturated = mv_temperature_correction(80.0, 21.0, 800.0, 0.9);
        let sat_delta = 80.0 - 21.0; // 59
        let sat_expected = 800.0 + sat_delta * 1.6; // 800 + 94.4 = 894.4
        assert!(
            (saturated - sat_expected).abs() < 1.0,
            "80 °C saturated progressive: expected ~{:.1}, got {:.1}",
            sat_expected,
            saturated
        );
    }

    #[test]
    fn cartridge_temp_sensitivity_variation() {
        // Verify different propellant types give different coefficients
        let sb = cartridge_temp_sensitivity("nato_single_base");
        let db = cartridge_temp_sensitivity("nato_double_base");
        let ball = cartridge_temp_sensitivity("ball");
        let stable = cartridge_temp_sensitivity("high_temp_stable");
        let unknown = cartridge_temp_sensitivity("unknown_type");

        assert_eq!(sb, 0.6);
        assert_eq!(db, 0.8);
        assert_eq!(ball, 1.0);
        assert_eq!(stable, 0.4);
        assert_eq!(unknown, DEFAULT_TEMP_COEFF);

        // Ball propellant has higher sensitivity than single base
        assert!(
            ball > sb,
            "Ball powder should be more temp-sensitive than single base"
        );
    }

    #[test]
    fn cold_bore_single_base_propellant() {
        // Single-base cold variant: coefficient = 0.5
        // At −10 °C, delta = −31, MV = 800 + (−31) × 0.5 = 784.5
        let coeff = cartridge_temp_sensitivity("nato_single_base_cold");
        assert_eq!(coeff, 0.5);
        let mv = mv_temperature_correction(-10.0, 21.0, 800.0, coeff);
        let expected = 800.0 + (-10.0 - 21.0) * 0.5;
        assert!(
            (mv - expected).abs() < 0.1,
            "Cold-bore single base at −10 °C: expected {:.1}, got {:.1}",
            expected,
            mv
        );
    }

    #[test]
    fn deterministic_output() {
        let a = mv_temperature_correction(-15.0, 21.0, 853.0, 0.9);
        let b = mv_temperature_correction(-15.0, 21.0, 853.0, 0.9);
        assert!(
            (a - b).abs() < 1e-12,
            "mv_temperature_correction deterministic"
        );
    }

    #[test]
    fn convenience_wrapper_matches() {
        let full = mv_temperature_correction(35.0, 21.0, 800.0, 0.9);
        let conv = mv_temp_standard(35.0, 800.0);
        assert!(
            (full - conv).abs() < 1e-12,
            "mv_temp_standard should match full call"
        );
    }

    #[test]
    fn temp_sensitivity_via_correction() {
        // Use different propellant coefficients and verify MV changes accordingly
        let coeff = cartridge_temp_sensitivity("ball"); // 1.0
        let mv_ball = mv_temperature_correction(0.0, 21.0, 800.0, coeff);

        let coeff_sb = cartridge_temp_sensitivity("nato_single_base"); // 0.6
        let mv_sb = mv_temperature_correction(0.0, 21.0, 800.0, coeff_sb);

        // Ball powder loses MORE velocity in the cold (higher coefficient)
        assert!(
            mv_ball < mv_sb,
            "Ball powder should lose more MV at 0 °C than single base"
        );
    }
}
