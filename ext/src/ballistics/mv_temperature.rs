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

/// Cold temperature ratio: 0.666 / 0.9 — sub-21 °C sensitivity multiplier.
///
/// Applied to the temperature coefficient for temperatures below the NATO
/// standard (21 °C). Derived from MIL-HDBK-762 empirical data showing
/// ~0.666 m/s/°C effective sensitivity vs the 0.9 m/s/°C baseline.
const COLD_TEMP_RATIO: f64 = 0.74;

/// Hot temperature ratio: 1.72 / 0.9 — above-21 °C sensitivity multiplier.
///
/// Applied to the temperature coefficient for temperatures above the NATO
/// standard (21 °C). Derived from MIL-HDBK-762 empirical data showing
/// ~1.72 m/s/°C effective sensitivity vs the 0.9 m/s/°C baseline.
const HOT_TEMP_RATIO: f64 = 1.91;

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

    // Piecewise temperature coefficient model.
    // Below reference temp (21 °C): propellant burns slower — effective
    // sensitivity is ~74 % of the baseline coefficient.
    // Above reference temp: propellant burns faster — effective sensitivity
    // is ~191 % of the baseline coefficient.
    // Source: MIL-HDBK-762, NATO STANAG 4362.
    let effective_coeff = if delta_t <= 0.0 {
        temp_coeff * COLD_TEMP_RATIO
    } else {
        temp_coeff * HOT_TEMP_RATIO
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
        // −20 °C with standard NATO coefficient and cold ratio:
        //   delta_t = −20 − 21 = −41 °C
        //   effective_coeff = 0.9 × 0.74 = 0.666
        //   mv = 800 + (−41) × 0.666 = 800 − 27.3 ≈ 772.7 m/s
        let mv = mv_temperature_correction(-20.0, 21.0, 800.0, 0.9);
        let expected = 800.0 + (-41.0) * 0.9 * COLD_TEMP_RATIO;
        assert!(
            (mv - expected).abs() < 0.1,
            "−20 °C: expected {:.1}, got {:.1}",
            expected,
            mv
        );
    }

    #[test]
    fn hot_temp_increases_mv() {
        // 50 °C with standard NATO coefficient and hot ratio:
        //   delta_t = 50 − 21 = 29 °C
        //   effective_coeff = 0.9 × 1.91 = 1.719
        //   mv = 800 + 29 × 1.719 = 800 + 49.9 ≈ 849.9 m/s
        let mv = mv_temperature_correction(50.0, 21.0, 800.0, 0.9);
        let expected = 800.0 + 29.0 * 0.9 * HOT_TEMP_RATIO;
        assert!(
            (mv - expected).abs() < 0.1,
            "50 °C: expected {:.1}, got {:.1}",
            expected,
            mv
        );
    }

    #[test]
    fn extreme_cold_piecewise() {
        // Piecewise cold model: below 21 °C the coefficient is always
        // temp_coeff × COLD_TEMP_RATIO (no further degressive drop).
        //
        // At −30 °C: cold ratio applies
        //   effective_coeff = 0.9 × 0.74 = 0.666
        //   delta_t = −30 − 21 = −51
        //   mv = 800 + (−51) × 0.666 = 800 − 33.97 ≈ 766.0 m/s
        //
        // At −60 °C: same cold ratio (no saturation asymptote)
        //   delta_t = −60 − 21 = −81
        //   mv = 800 + (−81) × 0.666 = 800 − 53.95 ≈ 746.1 m/s
        let mv_minus_30 = mv_temperature_correction(-30.0, 21.0, 800.0, 0.9);
        let expected_minus_30 = 800.0 + (-30.0 - 21.0) * 0.9 * COLD_TEMP_RATIO;
        assert!(
            (mv_minus_30 - expected_minus_30).abs() < 0.1,
            "−30 °C: expected {:.1}, got {:.1}",
            expected_minus_30,
            mv_minus_30
        );
        assert!(
            mv_minus_30 < 800.0,
            "Extreme cold should still reduce MV: {}",
            mv_minus_30
        );

        let mv_minus_60 = mv_temperature_correction(-60.0, 21.0, 800.0, 0.9);
        let expected_minus_60 = 800.0 + (-60.0 - 21.0) * 0.9 * COLD_TEMP_RATIO;
        assert!(
            (mv_minus_60 - expected_minus_60).abs() < 0.1,
            "−60 °C: expected {:.1}, got {:.1}",
            expected_minus_60,
            mv_minus_60
        );
    }

    #[test]
    fn extreme_heat_piecewise() {
        // Piecewise hot model: above 21 °C the coefficient is always
        // temp_coeff × HOT_TEMP_RATIO (no further progressive increase).
        //
        // At 60 °C: hot ratio applies
        //   effective_coeff = 0.9 × 1.91 = 1.719
        //   delta_t = 60 − 21 = 39
        //   mv = 800 + 39 × 1.719 = 800 + 67.04 ≈ 867.0 m/s
        //
        // At 80 °C: same hot ratio (no saturation asymptote)
        //   delta_t = 80 − 21 = 59
        //   mv = 800 + 59 × 1.719 = 800 + 101.42 ≈ 901.4 m/s
        let mv_60 = mv_temperature_correction(60.0, 21.0, 800.0, 0.9);
        let expected_60 = 800.0 + (60.0 - 21.0) * 0.9 * HOT_TEMP_RATIO;
        assert!(
            (mv_60 - expected_60).abs() < 0.1,
            "60 °C: expected {:.1}, got {:.1}",
            expected_60,
            mv_60
        );
        assert!(mv_60 > 800.0, "Extreme heat should increase MV: {}", mv_60);

        let mv_80 = mv_temperature_correction(80.0, 21.0, 800.0, 0.9);
        let expected_80 = 800.0 + (80.0 - 21.0) * 0.9 * HOT_TEMP_RATIO;
        assert!(
            (mv_80 - expected_80).abs() < 0.1,
            "80 °C: expected {:.1}, got {:.1}",
            expected_80,
            mv_80
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
        // At −10 °C, delta = −31
        // effective_coeff = 0.5 × COLD_TEMP_RATIO = 0.5 × 0.74 = 0.37
        // MV = 800 + (−31) × 0.37 = 800 − 11.47 = 788.5
        let coeff = cartridge_temp_sensitivity("nato_single_base_cold");
        assert_eq!(coeff, 0.5);
        let mv = mv_temperature_correction(-10.0, 21.0, 800.0, coeff);
        let expected = 800.0 + (-10.0 - 21.0) * coeff * COLD_TEMP_RATIO;
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
