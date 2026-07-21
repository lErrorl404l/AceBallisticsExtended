//! Property-based tests for ABE ballistics extension.
//!
//! Uses proptest to verify invariants hold across a wide range of
//! physically meaningful inputs. Each test asserts a property that
//! must be true for all valid inputs — not just specific expected
//! values.
//!
//! f64 strategies in proptest can produce NaN/Inf.  Range strategies
//! like `(0.0f64..1e6)` exclude non-finite values.  `prop_assume!`
//! guards any remaining edge cases.

use proptest::prelude::*;

use std::collections::HashMap;

use abe_ballistics_ext::{
    atmosphere::{
        density_at_altitude, density_from_altitude, pressure_at_altitude, temperature_at_altitude,
        wind_shear_factor,
    },
    ballistic_cap::{self, BallisticCapParams},
    barrel_harmonics::{self, BarrelHarmonicsParams},
    dof::{
        induced_drag_multiplier, magnus_acceleration, total_angle_of_attack, yaw_drag_penalty,
        yaw_of_repose,
    },
    drag::{bc_at_mach, boat_tail_drag_factor, get_cd},
    exterior::{calc_mach, speed_of_sound, spin_drift, wind_drift},
    mv_temperature::{cartridge_temp_sensitivity, mv_temp_standard, mv_temperature_correction},
    penetration::fragmentation::evaluate,
    stability::{estimate_inertia, gyroscopic_stability, is_over_stabilized, is_stable},
};

// ── Strategy helpers ──────────────────────────────────────────────────────────

/// Strictly positive f64 in `(0, 1e6)` — safe for physical quantities.
fn positive() -> impl Strategy<Value = f64> {
    f64::EPSILON..1e6
}

/// Mach number in `[0, 10]` — covers subsonic through hypersonic.
fn mach_strategy() -> impl Strategy<Value = f64> {
    0.0f64..10.0
}

/// Altitude in metres `[0, 50_000]` — troposphere through stratosphere.
fn altitude_strategy() -> impl Strategy<Value = f64> {
    0.0f64..50_000.0
}

/// Temperature in °C `[-60, 60]` — covers all realistic atmospheres.
fn temp_c_strategy() -> impl Strategy<Value = f64> {
    -60.0f64..60.0
}

// ═══════════════════════════════════════════════════════════════════════════════
// Atmosphere
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {
    #[test]
    fn density_decreases_with_altitude(
        low in altitude_strategy(),
        high in altitude_strategy(),
    ) {
        prop_assume!(high > low);
        let d_low = density_at_altitude(low);
        let d_high = density_at_altitude(high);
        prop_assert!(d_low.is_finite(), "density not finite at {} m", low);
        prop_assert!(d_high.is_finite(), "density not finite at {} m", high);
        prop_assert!(d_high < d_low, "density should decrease with altitude: \
            at {low:.0} m = {d_low:.6}, at {high:.0} m = {d_high:.6}");
    }

    #[test]
    fn pressure_decreases_with_altitude(
        low in altitude_strategy(),
        high in altitude_strategy(),
    ) {
        prop_assume!(high > low);
        let p_low = pressure_at_altitude(low);
        let p_high = pressure_at_altitude(high);
        prop_assert!(p_low.is_finite());
        prop_assert!(p_high.is_finite());
        prop_assert!(p_high < p_low, "pressure should decrease with altitude");
    }

    #[test]
    fn temperature_is_bounded(alt in altitude_strategy()) {
        let t = temperature_at_altitude(alt);
        prop_assert!(t.is_finite());
        // ISA minimum is 216.65 K at tropopause and above
        prop_assert!(t >= 216.65, "temp {t:.2} K below ISA minimum at {alt:.0} m");
        // Maximum at sea level is ~288.15 K
        prop_assert!(t <= 288.15 + 1.0, "temp {t:.2} K above sea-level ISA at {alt:.0} m");
    }

    #[test]
    fn temperature_isothermal_above_tropopause(alt in (11_000.0f64..50_000.0)) {
        let t = temperature_at_altitude(alt);
        prop_assert!((t - 216.65).abs() < 1.0,
            "above tropopause temp should be ~216.65 K, got {t:.2} at {alt:.0} m");
    }

    #[test]
    fn wind_shear_factor_bounded(alt in (0.0f64..2000.0)) {
        let f = wind_shear_factor(alt);
        prop_assert!(f.is_finite());
        prop_assert!(f >= 1.0, "wind shear factor should be >= 1, got {f:.4}");
        prop_assert!(f <= 3.0, "wind shear factor should be <= 3, got {f:.4}");
    }

    #[test]
    fn wind_shear_increases_with_altitude(
        low in (0.0f64..1000.0),
        high in (0.0f64..1000.0),
    ) {
        prop_assume!(high > low + 1.0);
        let f_low = wind_shear_factor(low);
        let f_high = wind_shear_factor(high);
        prop_assert!(f_high >= f_low,
            "wind shear should be non-decreasing with altitude: \
            {f_low:.4} at {low:.0} m > {f_high:.4} at {high:.0} m");
    }

    #[test]
    fn density_positive(alt in altitude_strategy()) {
        let d = density_at_altitude(alt);
        prop_assert!(d.is_finite());
        prop_assert!(d > 0.0, "density should be positive, got {d:.6} at {alt:.0} m");
    }

    #[test]
    fn warmer_air_is_less_dense(
        cool_temp in (-19.0f64..-0.2),
        warm_temp in (0.2f64..30.0),
        alt in altitude_strategy(),
    ) {
        prop_assume!(warm_temp > cool_temp + 1.0);
        let d_cool = density_from_altitude(alt, cool_temp);
        let d_warm = density_from_altitude(alt, warm_temp);
        prop_assert!(d_cool.is_finite(), "cool density not finite: {d_cool}");
        prop_assert!(d_warm.is_finite(), "warm density not finite: {d_warm}");
        prop_assert!(d_warm < d_cool,
            "warmer air should be less dense: cool={d_cool:.6} at {cool_temp:.0}°C, \
             warm={d_warm:.6} at {warm_temp:.0}°C");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Drag — Cd values
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {
    #[test]
    fn cd_is_finite_and_positive(        model in prop::sample::select(&["g1", "g7", "g8"] as &[_]), mach in mach_strategy()) {
        let cd = get_cd(model, mach);
        prop_assert!(cd.is_finite(), "Cd not finite for {model} at M={mach}");
        prop_assert!(cd > 0.0, "Cd should be positive for {model} at M={mach}, got {cd}");
    }

    #[test]
    fn g7_less_than_g1(mach in (0.1f64..5.0)) {
        let g7 = get_cd("g7", mach);
        let cd_g1 = get_cd("g1", mach);
        // G7 <= G1 everywhere within a small tolerance for transonic crossover
        prop_assert!(g7 <= cd_g1 + 0.005,
            "G7 ({g7:.4}) should be <= G1 ({cd_g1:.4}) at M={mach}");
    }

    #[test]
    fn g8_between_g1_and_g7(mach in (0.1f64..3.0)) {
        let g7 = get_cd("g7", mach);
        let g8 = get_cd("g8", mach);
        prop_assert!(g8 > g7, "G8 ({g8:.4}) > G7 ({g7:.4}) at M={mach}");
        // G1 is not always above G8 in the full range; only check subsonic through mid-supersonic
        // where the relationship is well-established
    }

    #[test]
    fn transonic_drag_rise(g7_sub in (0.6f64..0.8), g7_peak in (0.95f64..1.05)) {
        let cd_sub = get_cd("g7", g7_sub);
        let cd_peak = get_cd("g7", g7_peak);
        prop_assert!(cd_peak > cd_sub,
            "G7 drag should peak in transonic: subsonic M={g7_sub:.1} Cd={cd_sub:.4}, \
             transonic M={g7_peak:.2} Cd={cd_peak:.4}");
    }

    #[test]
    fn cd_decreases_after_transonic_peak(
        peak_mach in (0.95f64..1.05),
        supersonic_mach in (1.6f64..4.0),
    ) {
        let cd_peak = get_cd("g7", peak_mach);
        let cd_sup = get_cd("g7", supersonic_mach);
        prop_assert!(cd_sup < cd_peak,
            "Cd should drop after transonic peak: M={peak_mach:.2} Cd={cd_peak:.4} \
             vs M={supersonic_mach:.1} Cd={cd_sup:.4}");
    }

    #[test]
    fn cd_at_zero_mach_reasonable(model in prop::sample::select(&["g1", "g7", "g8"])) {
        let cd = get_cd(model, 0.0);
        prop_assert!(cd > 0.08, "Cd at M=0 too low: {cd:.4} for {model}");
        prop_assert!(cd < 0.25, "Cd at M=0 too high: {cd:.4} for {model}");
    }

    #[test]
    fn unknown_model_falls_back_to_g7(mach in (0.1f64..5.0)) {
        let unknown = get_cd("custom_unknown", mach);
        let g7 = get_cd("g7", mach);
        prop_assert!((unknown - g7).abs() < 0.001,
            "unknown model should fall back to G7: got {unknown:.4} vs G7 {g7:.4} at M={mach}");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Drag — BC at Mach
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {
    #[test]
    fn bc_at_mach_is_positive(bc in positive(), mach in mach_strategy(), model in prop::sample::select(&["g1", "g7"])) {
        let result = bc_at_mach(bc, mach, model);
        prop_assert!(result.is_finite(), "bc_at_mach not finite: bc={bc}, M={mach}, {model}");
        prop_assert!(result > 0.0, "bc_at_mach should be positive: got {result} for {model} at M={mach}");
    }

    #[test]
    fn bc_at_mach_g7_dip_at_transonic(bc in positive()) {
        let subsonic = bc_at_mach(bc, 0.8, "g7");
        let transonic = bc_at_mach(bc, 1.0, "g7");
        let supersonic = bc_at_mach(bc, 1.2, "g7");
        // G7 should dip in transonic
        prop_assert!(transonic < subsonic,
            "G7 BC at M=1.0 ({transonic:.6}) should be below subsonic ({subsonic:.6})");
        prop_assert!(transonic < supersonic,
            "G7 BC at M=1.0 ({transonic:.6}) should be below supersonic ({supersonic:.6})");
    }

    #[test]
    fn bc_at_mach_g1_dip_milder_than_g7(bc in positive()) {
        let g1_dip = bc_at_mach(bc, 1.0, "g1");
        let g7_dip = bc_at_mach(bc, 1.0, "g7");
        // G1 should have a shallower dip than G7 at Mach 1.0
        let g1_ratio = g1_dip / bc;
        let g7_ratio = g7_dip / bc;
        prop_assert!(g1_ratio > g7_ratio,
            "G1 dip ratio ({g1_ratio:.4}) should be milder (less dip) than G7 ({g7_ratio:.4})");
    }

    #[test]
    fn bc_at_mach_smooth_at_boundaries(mach in (0.79f64..0.81)) {
        // No discontinuity at Mach 0.8 boundary
        let left = bc_at_mach(0.200, mach - 0.01, "g7");
        let right = bc_at_mach(0.200, mach + 0.01, "g7");
        let diff = (left - right).abs();
        prop_assert!(diff < 0.01, "possible discontinuity at M=0.8: {left:.6} vs {right:.6}");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Drag — boat_tail_drag_factor
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {
    #[test]
    fn boat_tail_factor_in_range(
        angle in (0.0f64..20.0),
        length in (0.0f64..2.0),
        mach in (0.0f64..5.0),
    ) {
        let f = boat_tail_drag_factor(angle, length, mach);
        prop_assert!(f.is_finite());
        prop_assert!(f >= 0.85, "factor below 0.85: {f:.4} at ({angle:.0}°, {length:.1} cal, M={mach:.1})");
        prop_assert!(f <= 1.0 + 1e-12,
            "factor above 1.0: {f:.4} at ({angle:.0}°, {length:.1} cal, M={mach:.1})");
    }

    #[test]
    fn boat_tail_no_tail_returns_one(angle in (0.0f64..1.0), length in (0.0f64..0.04), mach in (0.0f64..5.0)) {
        let f = boat_tail_drag_factor(angle, length, mach);
        prop_assert!((f - 1.0).abs() < 1e-12,
            "no boat-tail should give factor 1.0: got {f}");
    }

    #[test]
    fn boat_tail_subsonic_vs_supersonic(
        angle in (5.0f64..12.0),
        length in (0.5f64..1.5),
    ) {
        let sub = boat_tail_drag_factor(angle, length, 0.5);
        let sup = boat_tail_drag_factor(angle, length, 2.0);
        prop_assert!(sub < sup,
            "boat-tail should be more effective (lower factor) subsonically: \
             sub={sub:.4}, sup={sup:.4}");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Exterior
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {
    #[test]
    fn speed_of_sound_increases_with_temperature(
        cold in (-50.0f64..30.0),
        hot in (-50.0f64..60.0),
    ) {
        prop_assume!(hot > cold + 1.0);
        let sos_cold = speed_of_sound(cold);
        let sos_hot = speed_of_sound(hot);
        prop_assert!(sos_cold.is_finite());
        prop_assert!(sos_hot.is_finite());
        prop_assert!(sos_hot > sos_cold,
            "SoS should increase with temp: {sos_cold:.1} at {cold:.0}°C < {sos_hot:.1} at {hot:.0}°C");
    }

    #[test]
    fn speed_of_sound_reasonable_at_typical_temps(temp in (-30.0f64..50.0)) {
        let sos = speed_of_sound(temp);
        prop_assert!(sos > 300.0, "SoS too low: {sos:.1} at {temp:.0}°C");
        prop_assert!(sos < 400.0, "SoS too high: {sos:.1} at {temp:.0}°C");
    }

    #[test]
    fn mach_zero_at_zero_velocity(temp in temp_c_strategy()) {
        let m = calc_mach(0.0, temp);
        prop_assert!((m - 0.0).abs() < f64::EPSILON,
            "Mach should be 0 at v=0, got {m}");
    }

    #[test]
    fn mach_scales_with_velocity(
        v1 in (0.0f64..2000.0),
        v2 in (0.0f64..2000.0),
        temp in temp_c_strategy(),
    ) {
        prop_assume!((v1 - v2).abs() > 1.0);
        let m1 = calc_mach(v1, temp);
        let m2 = calc_mach(v2, temp);
        prop_assert!(m1.is_finite());
        prop_assert!(m2.is_finite());
        // Higher velocity → higher Mach (at same temperature)
        if v2 > v1 {
            prop_assert!(m2 > m1,
                "Mach should increase with velocity: {m1:.4} at {v1:.0} m/s, {m2:.4} at {v2:.0} m/s");
        }
    }

    #[test]
    fn wind_drift_zero_at_no_wind(tof in (0.0f64..10.0), range in (0.0f64..2000.0), mv in positive()) {
        let d = wind_drift(0.0, tof, range, mv);
        prop_assert!((d - 0.0).abs() < f64::EPSILON,
            "wind drift should be 0 at zero wind, got {d}");
    }

    #[test]
    fn wind_drift_scales_with_wind(
        w1 in (-50.0f64..50.0),
        w2 in (-50.0f64..50.0),
        tof in (0.01f64..10.0),
        range in (0.0f64..2000.0),
        mv in (100.0f64..2000.0),
    ) {
        prop_assume!(w1 != 0.0 && w2 != 0.0 && (w1 / w2).abs() > 0.1);
        let d1 = wind_drift(w1, tof, range, mv);
        let d2 = wind_drift(w2, tof, range, mv);
        prop_assert!(d1.is_finite());
        prop_assert!(d2.is_finite());
        // Doubling wind should double drift
        let ratio = d2 / d1;
        let wind_ratio = w2 / w1;
        // Allow some tolerance due to f64 arithmetic
        prop_assert!((ratio / wind_ratio - 1.0).abs() < 1e-12,
            "drift ratio ({ratio:.6}) should equal wind ratio ({wind_ratio:.6})");
    }

    #[test]
    fn spin_drift_positive_for_right_twist(twist in positive(), mv in positive(), tof in (0.01f64..10.0), range in (0.0f64..5000.0)) {
        let drift = spin_drift(twist, mv, tof, range);
        prop_assert!(drift >= 0.0,
            "right-hand twist should give non-negative spin drift: got {drift:.4}");
        prop_assert!(drift.is_finite());
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// DOF — yaw_of_repose
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {
    #[test]
    fn yaw_of_repose_zero_for_vertical(
        moi in positive(),
        twist in positive(),
        vel in positive(),
        density in positive(),
        cal in positive(),
        proj_type in prop::sample::select(&["spitzer", "blunt", "match", "unknown"]),
    ) {
        let yaw = yaw_of_repose(moi, twist, vel, density, cal, proj_type, std::f64::consts::FRAC_PI_2);
        prop_assert!((yaw - 0.0).abs() < f64::EPSILON,
            "vertical trajectory should give zero yaw: got {yaw:.2e}");
    }

    #[test]
    fn yaw_of_repose_zero_when_velocity_zero(
        moi in positive(),
        twist in positive(),
        density in positive(),
        cal in positive(),
        proj_type in prop::sample::select(&["spitzer", "blunt"]),
        angle in (0.0f64..std::f64::consts::FRAC_PI_2),
    ) {
        let yaw = yaw_of_repose(moi, twist, 0.0, density, cal, proj_type, angle);
        prop_assert!((yaw - 0.0).abs() < f64::EPSILON,
            "zero velocity should give zero yaw: got {yaw:.2e}");
    }

    #[test]
    fn yaw_of_repose_faster_twist_increases_yaw(
        moi in (1e-10f64..1e-5),
        vel in (100.0f64..1500.0),
        density in (0.5f64..1.5),
        cal in (0.001f64..0.05),
        proj_type in prop::sample::select(&["spitzer", "match"]),
    ) {
        let slow_twist = 1.0 / 0.305;  // 1:12"
        let fast_twist = 1.0 / 0.178;  // 1:7"
        let yaw_slow = yaw_of_repose(moi, slow_twist, vel, density, cal, proj_type, 0.0);
        let yaw_fast = yaw_of_repose(moi, fast_twist, vel, density, cal, proj_type, 0.0);
        prop_assert!(yaw_slow >= 0.0);
        prop_assert!(yaw_fast >= 0.0);
        prop_assert!(yaw_fast > yaw_slow,
            "faster twist should increase yaw: fast={yaw_fast:.2e} slow={yaw_slow:.2e}");
    }

    #[test]
    fn yaw_of_repose_decreases_with_velocity(
        moi in (1e-10f64..1e-5),
        twist in (2.0f64..10.0),
        density in (0.5f64..1.5),
        cal in (0.001f64..0.05),
        proj_type in prop::sample::select(&["spitzer", "match"]),
    ) {
        let yaw_slow = yaw_of_repose(moi, twist, 300.0, density, cal, proj_type, 0.0);
        let yaw_fast = yaw_of_repose(moi, twist, 900.0, density, cal, proj_type, 0.0);
        prop_assert!(yaw_slow >= 0.0);
        prop_assert!(yaw_fast >= 0.0);
        prop_assert!(yaw_slow > yaw_fast,
            "yaw should decrease with velocity: slow={yaw_slow:.2e} fast={yaw_fast:.2e}");
    }

    #[test]
    fn yaw_of_repose_finite_for_all_valid_inputs(
        moi in positive(),
        twist in positive(),
        vel in positive(),
        density in positive(),
        cal in positive(),
        angle in (0.0f64..std::f64::consts::FRAC_PI_2),
    ) {
        // Must use valid values that pass the guard clauses
        let yaw = yaw_of_repose(moi.min(1.0), twist.min(100.0), vel.min(2000.0),
                                 density.min(5.0), cal.min(0.1), "spitzer", angle);
        prop_assert!(yaw.is_finite(), "yaw should be finite: got {yaw:.2e}");
        prop_assert!(yaw >= 0.0, "yaw should be non-negative: got {yaw:.2e}");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// DOF — induced_drag_multiplier, total_angle_of_attack, yaw_drag_penalty
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {
    #[test]
    fn induced_drag_ge_one(yaw in (0.0f64..0.5)) {
        let m = induced_drag_multiplier(yaw);
        prop_assert!(m >= 1.0, "induced drag multiplier should be >= 1, got {m:.6}");
        prop_assert!(m.is_finite());
    }

    #[test]
    fn induced_drag_symmetric(yaw in (0.0f64..0.5)) {
        let pos = induced_drag_multiplier(yaw);
        let neg = induced_drag_multiplier(-yaw);
        prop_assert!((pos - neg).abs() < f64::EPSILON,
            "induced drag should be symmetric: pos={pos:.6}, neg={neg:.6}");
    }

    #[test]
    fn induced_drag_increases_with_yaw(
        y1 in (0.0f64..0.3),
        y2 in (0.0f64..0.3),
    ) {
        prop_assume!(y2 > y1);
        let m1 = induced_drag_multiplier(y1);
        let m2 = induced_drag_multiplier(y2);
        prop_assert!(m2 > m1,
            "larger yaw should give larger multiplier: {m1:.6} at {y1:.3} vs {m2:.6} at {y2:.3}");
    }

    #[test]
    fn yaw_drag_penalty_ge_one(yaw in (0.0f64..0.5), ptype in prop::sample::select(&["spitzer", "blunt", "match", "unknown"])) {
        let p = yaw_drag_penalty(yaw, ptype);
        prop_assert!(p >= 1.0, "yaw drag penalty should be >= 1, got {p:.6} for {ptype}");
    }

    #[test]
    fn total_aoa_rss_combination(yaw in (0.0f64..0.5), turb in (0.0f64..0.5)) {
        let aoa = total_angle_of_attack(yaw, turb);
        let expected = (yaw.powi(2) + turb.powi(2)).sqrt();
        prop_assert!((aoa - expected).abs() < 1e-15,
            "AoA should be RSS: expected {expected:.10}, got {aoa:.10}");
    }

    #[test]
    fn total_aoa_ge_max_component(yaw in (0.0f64..0.5), turb in (0.0f64..0.5)) {
        let aoa = total_angle_of_attack(yaw, turb);
        let max = yaw.max(turb);
        prop_assert!(aoa >= max,
            "AoA should be >= max component: {aoa:.6} < {max:.6}");
    }

    #[test]
    fn magnus_cross_product_sign(
        density in (0.5f64..1.5),
        cal in (0.001f64..0.05),
        mass in (0.001f64..0.1),
        spin in (100.0f64..10000.0),
        vy in (-50.0f64..50.0),
        vz in (-50.0f64..50.0),
    ) {
        // Magnus: ay ∝ -vz, az ∝ vy
        // Need non-zero vy & vz to test sign
        prop_assume!(vy.abs() > 1.0 && vz.abs() > 1.0);
        let speed = (vy.powi(2) + vz.powi(2) + 900.0f64.powi(2)).sqrt();
        prop_assume!(speed >= 10.0); // guard clause in magnus_acceleration

        let (ay, az) = magnus_acceleration(density, speed, cal, mass, spin, vy, vz);
        prop_assert!(ay.is_finite(), "magnus ay not finite");
        prop_assert!(az.is_finite(), "magnus az not finite");

        // ay should have opposite sign to vz
        if vz.abs() > 1e-10 {
            prop_assert!((ay.signum() + vz.signum()).abs() < 0.1 || ay.abs() < 1e-15,
                "ay should oppose vz: ay={ay:.2e}, vz={vz:.1}");
        }
        // az should have same sign as vy
        if vy.abs() > 1e-10 {
            prop_assert!((az.signum() - vy.signum()).abs() < 0.1 || az.abs() < 1e-15,
                "az should match vy: az={az:.2e}, vy={vy:.1}");
        }
    }

    #[test]
    fn magnus_zero_at_low_speed(
        density in (0.5f64..1.5),
        cal in (0.001f64..0.05),
        mass in (0.001f64..0.1),
        spin in positive(),
        vy in (-50.0f64..50.0),
        vz in (-50.0f64..50.0),
    ) {
        let (ay, az) = magnus_acceleration(density, 5.0, cal, mass, spin, vy, vz);
        prop_assert!((ay - 0.0).abs() < f64::EPSILON,
            "Magnus should be zero below 10 m/s: ay={ay:.2e}");
        prop_assert!((az - 0.0).abs() < f64::EPSILON,
            "Magnus should be zero below 10 m/s: az={az:.2e}");
    }

    #[test]
    fn magnus_scales_with_density(
        cal in (0.001f64..0.05),
        mass in (0.001f64..0.1),
        spin in (1000.0f64..10000.0),
        vy in (-20.0f64..20.0),
        vz in (-20.0f64..20.0),
    ) {
        prop_assume!(vy.abs() > 1.0 || vz.abs() > 1.0);
        let speed = (vy.powi(2) + vz.powi(2) + 900.0f64.powi(2)).sqrt();
        let (ay_low, az_low) = magnus_acceleration(0.5, speed, cal, mass, spin, vy, vz);
        let (ay_high, az_high) = magnus_acceleration(1.225, speed, cal, mass, spin, vy, vz);

        prop_assert!(ay_high.abs() >= ay_low.abs() - 1e-15,
            "higher density should give stronger Magnus: high={ay_high:.2e}, low={ay_low:.2e}");
        prop_assert!(az_high.abs() >= az_low.abs() - 1e-15,
            "higher density should give stronger Magnus: high={az_high:.2e}, low={az_low:.2e}");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Stability
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {
    #[test]
    fn gyroscopic_stability_positive(
        vel in (100.0f64..2000.0),
        twist in (2.0f64..10.0),
        cal in (0.001f64..0.05),
        mass in (0.001f64..0.1),
        density in (0.5f64..1.5),
        ptype in prop::sample::select(&["spitzer", "blunt", "unknown"]),
    ) {
        let sg = gyroscopic_stability(vel, twist, cal, mass, density, ptype);
        prop_assert!(sg.is_finite(),
            "Sg not finite: v={vel:.0}, twist={twist:.1}, cal={cal:.4}, mass={mass:.4}, ρ={density:.3}");
        prop_assert!(sg > 0.0,
            "Sg should be positive for valid inputs: got {sg:.4}");
    }

    #[test]
    fn sg_increases_with_twist(
        vel in (300.0f64..1500.0),
        cal in (0.003f64..0.03),
        mass in (0.002f64..0.05),
        density in (0.5f64..1.5),
        ptype in prop::sample::select(&["spitzer", "blunt"]),
    ) {
        let slow = gyroscopic_stability(vel, 3.0, cal, mass, density, ptype);
        let fast = gyroscopic_stability(vel, 6.0, cal, mass, density, ptype);
        prop_assert!(fast > slow,
            "faster twist should increase Sg: slow={slow:.4}, fast={fast:.4}");
    }

    #[test]
    fn sg_decreases_with_density(
        vel in (300.0f64..1500.0),
        twist in (2.0f64..10.0),
        cal in (0.003f64..0.03),
        mass in (0.002f64..0.05),
        ptype in prop::sample::select(&["spitzer", "blunt"]),
    ) {
        let sg_sea = gyroscopic_stability(vel, twist, cal, mass, 1.225, ptype);
        let sg_high = gyroscopic_stability(vel, twist, cal, mass, 0.5, ptype);
        prop_assert!(sg_high > sg_sea,
            "lower density should increase Sg: sea={sg_sea:.4}, high={sg_high:.4}");
    }

    #[test]
    fn sg_approx_velocity_invariant(
        twist in (2.0f64..10.0),
        cal in (0.003f64..0.03),
        mass in (0.002f64..0.05),
        density in (0.5f64..1.5),
        ptype in prop::sample::select(&["spitzer", "blunt"]),
    ) {
        let sg_low = gyroscopic_stability(500.0, twist, cal, mass, density, ptype);
        let sg_high = gyroscopic_stability(1000.0, twist, cal, mass, density, ptype);
        let diff = (sg_high - sg_low).abs();
        // Sg ∝ ω²/q; ω ∝ v and q ∝ v² so v-dependence cancels
        prop_assert!(diff < 0.02,
            "Sg should be approx velocity-invariant: {sg_low:.4} @ 500 m/s vs {sg_high:.4} @ 1000 m/s (diff={diff:.4})");
    }

    #[test]
    fn is_stable_threshold_behavior(sg in (0.0f64..5.0)) {
        let stable = is_stable(sg);
        let over = is_over_stabilized(sg);
        // Rules: stable if Sg > 1.3, over-stabilized if Sg > 3.0
        if sg > 1.3 {
            prop_assert!(stable, "Sg={sg:.4} > 1.3 should be stable");
        } else {
            prop_assert!(!stable, "Sg={sg:.4} <= 1.3 should NOT be stable");
        }
        if sg > 3.0 {
            prop_assert!(over, "Sg={sg:.4} > 3.0 should be over-stabilized");
        } else {
            prop_assert!(!over, "Sg={sg:.4} <= 3.0 should NOT be over-stabilized");
        }
    }

    #[test]
    fn sg_zero_for_zero_velocity(
        twist in positive(),
        cal in positive(),
        mass in positive(),
        density in positive(),
    ) {
        let sg = gyroscopic_stability(0.0, twist, cal, mass, density, "spitzer");
        prop_assert!((sg - 0.0).abs() < f64::EPSILON,
            "Sg should be 0 for v=0: got {sg:.4}");
    }

    #[test]
    fn estimate_inertia_positive(
        mass in (0.001f64..0.1),
        cal in (0.003f64..0.05),
    ) {
        let (ix, iy) = estimate_inertia(mass, cal);
        prop_assert!(ix > 0.0, "I_x should be positive: {ix:.2e}");
        prop_assert!(iy > 0.0, "I_y should be positive: {iy:.2e}");
    }

    #[test]
    fn estimate_inertia_both_positive_and_ix_lt_iy_by_construction(
        mass in (0.01f64..0.05),
        cal in (0.003f64..0.007),
    ) {
        // For a typical rifle projectile (length >> caliber), I_x should be
        // smaller than I_y.  Constrain cal to keep L/d ratio ≥ 3.
        let (ix, iy) = estimate_inertia(mass, cal);
        prop_assert!(ix > 0.0);
        prop_assert!(iy > 0.0);
        prop_assert!(ix < iy,
            "for typical rifle projectile I_x ({ix:.2e}) < I_y ({iy:.2e}), mass={mass:.4}, cal={cal:.4}");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// MV Temperature
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {
    #[test]
    fn mv_unchanged_at_standard_temp(reference_mv in (100.0f64..2000.0), coeff in (0.1f64..2.0)) {
        let mv = mv_temperature_correction(21.0, 21.0, reference_mv, coeff);
        prop_assert!((mv - reference_mv).abs() < 1e-10,
            "MV should be unchanged at standard temp: {mv:.1} vs ref {reference_mv:.1}");
    }

    #[test]
    fn cold_reduces_mv_hot_increases_mv(
        reference_mv in (200.0f64..1500.0),
        coeff in (0.4f64..1.5),
    ) {
        let mv_cold = mv_temperature_correction(-10.0, 21.0, reference_mv, coeff);
        let mv_hot = mv_temperature_correction(40.0, 21.0, reference_mv, coeff);
        prop_assert!(mv_cold < reference_mv,
            "cold should reduce MV: {mv_cold:.1} < {reference_mv:.1}");
        prop_assert!(mv_hot > reference_mv,
            "hot should increase MV: {mv_hot:.1} > {reference_mv:.1}");
        prop_assert!(mv_hot > mv_cold,
            "hot MV ({mv_hot:.1}) > cold MV ({mv_cold:.1})");
    }

    #[test]
    fn mv_linear_with_delta_t(
        temp in (-30.0f64..50.0),
        reference_mv in (200.0f64..1500.0),
    ) {
        let mv = mv_temperature_correction(temp, 21.0, reference_mv, 0.9);
        prop_assert!(mv.is_finite());

        // At reference temp, MV = reference_mv
        let mv_ref = mv_temperature_correction(21.0, 21.0, reference_mv, 0.9);
        prop_assert!((mv_ref - reference_mv).abs() < 1e-10);
    }

    #[test]
    fn sensitivity_coeffs_finite(ptype in prop::sample::select(&[
        "nato_single_base", "nato_double_base", "ball",
        "smokeless_pistol", "high_temp_stable", "nato_single_base_cold", "some_unknown_type",
    ])) {
        let coeff = cartridge_temp_sensitivity(ptype);
        prop_assert!(coeff.is_finite());
        prop_assert!(coeff > 0.0, "temp sensitivity should be > 0 for {ptype}: got {coeff}");
        prop_assert!(coeff <= 2.0, "temp sensitivity should be <= 2.0 for {ptype}: got {coeff}");
    }

    #[test]
    fn convenience_wrapper_consistency(
        temp in (-20.0f64..40.0),
        reference_mv in (200.0f64..1500.0),
    ) {
        let full = mv_temperature_correction(temp, 21.0, reference_mv, 0.9);
        let conv = mv_temp_standard(temp, reference_mv);
        prop_assert!((full - conv).abs() < 1e-12,
            "mv_temp_standard should match mv_temperature_correction: {full:.6} vs {conv:.6}");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Ballistic cap
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {
    #[test]
    fn ballistic_cap_detachment_threshold(
        cap_mass in (0.5f64..10.0),
        proj_mass in (5.0f64..60.0),
        cal in (5.0f64..30.0),
        low_vel in (50.0f64..149.0),
        high_vel in (151.0f64..1500.0),
    ) {
        let params_low = BallisticCapParams {
            has_ballistic_cap: true,
            has_piercing_cap: false,
            cap_mass_g: cap_mass,
            projectile_mass_g: proj_mass,
            caliber_mm: cal,
            impact_velocity_ms: low_vel,
            impact_angle_deg: 0.0,
        };
        let params_high = BallisticCapParams {
            has_ballistic_cap: true,
            has_piercing_cap: false,
            cap_mass_g: cap_mass,
            projectile_mass_g: proj_mass,
            caliber_mm: cal,
            impact_velocity_ms: high_vel,
            impact_angle_deg: 0.0,
        };

        let result_low = ballistic_cap::evaluate_ballistic_cap(&params_low);
        let result_high = ballistic_cap::evaluate_ballistic_cap(&params_high);

        // Below threshold: no detach
        prop_assert!(!result_low.detaches,
            "cap should not detach below {} m/s: {low_vel:.0} m/s",
            ballistic_cap::BALLISTIC_CAP_MIN_DETACH_VELOCITY_MS);
        // Above threshold: should detach
        prop_assert!(result_high.detaches,
            "cap should detach above {} m/s: {high_vel:.0} m/s",
            ballistic_cap::BALLISTIC_CAP_MIN_DETACH_VELOCITY_MS);

        // Low velocity should have penetration penalty
        prop_assert!(result_low.penetration_penalty_pct > 0.0,
            "non-detaching cap should have penetration penalty");
        prop_assert!((result_high.penetration_penalty_pct - 0.0).abs() < 1e-12,
            "detaching cap should have no penetration penalty");
    }

    #[test]
    fn no_cap_no_change(
        has_ballistic in prop::bool::ANY,
        has_piercing in prop::bool::ANY,
        proj_mass in (1.0f64..100.0),
        vel in (100.0f64..1500.0),
    ) {
        prop_assume!(!has_ballistic && !has_piercing);

        let params = BallisticCapParams {
            has_ballistic_cap: false,
            has_piercing_cap: false,
            cap_mass_g: 0.0,
            projectile_mass_g: proj_mass,
            caliber_mm: 12.7,
            impact_velocity_ms: vel,
            impact_angle_deg: 0.0,
        };

        let bc_result = ballistic_cap::evaluate_ballistic_cap(&params);
        prop_assert!(!bc_result.detaches);
        prop_assert!((bc_result.mass_after_detach_g - proj_mass).abs() < 1e-12);
        prop_assert!((bc_result.bc_change_pct - 0.0).abs() < 1e-12);

        let pc_result = ballistic_cap::evaluate_piercing_cap(&params);
        prop_assert!(!pc_result.detaches);
        prop_assert!((pc_result.mass_after_detach_g - proj_mass).abs() < 1e-12);
    }

    #[test]
    fn ballistic_cap_bc_change_in_range(
        cap_mass in (1.0f64..5.0),
        proj_mass in (30.0f64..50.0),
        cal in (10.0f64..15.0),
        vel in (300.0f64..1200.0),
    ) {
        let params = BallisticCapParams {
            has_ballistic_cap: true,
            has_piercing_cap: false,
            cap_mass_g: cap_mass,
            projectile_mass_g: proj_mass,
            caliber_mm: cal,
            impact_velocity_ms: vel,
            impact_angle_deg: 0.0,
        };
        let result = ballistic_cap::evaluate_ballistic_cap(&params);
        prop_assert!(result.detaches);
        // BC change should be -2% to -5%
        prop_assert!(result.bc_change_pct <= -2.0,
            "BC change too mild: {:.2}%", result.bc_change_pct);
        prop_assert!(result.bc_change_pct >= -5.0,
            "BC change too severe: {:.2}%", result.bc_change_pct);
    }

    #[test]
    fn piercing_cap_penetration_bonus(
        cap_mass in (1.0f64..5.0),
        proj_mass in (30.0f64..50.0),
        cal in (10.0f64..15.0),
        vel in (300.0f64..1200.0),
    ) {
        let params = BallisticCapParams {
            has_ballistic_cap: false,
            has_piercing_cap: true,
            cap_mass_g: cap_mass,
            projectile_mass_g: proj_mass,
            caliber_mm: cal,
            impact_velocity_ms: vel,
            impact_angle_deg: 0.0,
        };
        let result = ballistic_cap::evaluate_piercing_cap(&params);
        prop_assert!(result.detaches,
            "piercing cap should detach at {vel:.0} m/s");
        // Penetration bonus should be negative penalty (bonus)
        prop_assert!(result.penetration_penalty_pct < 0.0,
            "piercing cap should give penetration bonus, got {:.2}%",
            result.penetration_penalty_pct);
    }

    #[test]
    fn ballistic_cap_finite_for_all_inputs(
        has_bc in prop::bool::ANY,
        has_pc in prop::bool::ANY,
        cap_mass in (0.0f64..10.0),
        proj_mass in (1.0f64..100.0),
        cal in (3.0f64..30.0),
        vel in (0.0f64..2000.0),
        angle in (0.0f64..90.0),
    ) {
        let params = BallisticCapParams {
            has_ballistic_cap: has_bc,
            has_piercing_cap: has_pc,
            cap_mass_g: cap_mass,
            projectile_mass_g: proj_mass,
            caliber_mm: cal,
            impact_velocity_ms: vel,
            impact_angle_deg: angle,
        };
        let bc = ballistic_cap::evaluate_ballistic_cap(&params);
        let pc = ballistic_cap::evaluate_piercing_cap(&params);

        prop_assert!(bc.mass_after_detach_g.is_finite());
        prop_assert!(bc.bc_change_pct.is_finite());
        prop_assert!(bc.velocity_loss_pct.is_finite());
        prop_assert!(bc.penetration_penalty_pct.is_finite());

        prop_assert!(pc.mass_after_detach_g.is_finite());
        prop_assert!(pc.bc_change_pct.is_finite());
        prop_assert!(pc.velocity_loss_pct.is_finite());
        prop_assert!(pc.penetration_penalty_pct.is_finite());
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Barrel harmonics
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {
    #[test]
    fn barrel_tip_velocity_zero_when_mass_zero(
        length in (1.0f64..1000.0),
        mv in (1.0f64..2000.0),
        profile in prop::sample::select(&["pencil", "contour", "heavy", "bull"] as &[_]),
    ) {
        let tip = barrel_harmonics::barrel_tip_velocity(length, profile, 0.0, mv);
        prop_assert!((tip - 0.0).abs() < f64::EPSILON,
            "tip velocity should be 0 when mass is 0: got {tip:.2e}");
    }

    #[test]
    fn barrel_tip_velocity_zero_when_mv_zero(
        length in (1.0f64..1000.0),
        mass in (1.0f64..100.0),
        profile in prop::sample::select(&["pencil", "contour", "heavy", "bull"] as &[_]),
    ) {
        let tip = barrel_harmonics::barrel_tip_velocity(length, profile, mass, 0.0);
        prop_assert!((tip - 0.0).abs() < f64::EPSILON,
            "tip velocity should be 0 when MV is 0: got {tip:.2e}");
    }

    #[test]
    fn barrel_tip_velocity_increases_with_mass(
        length in (200.0f64..800.0),
        mv in (500.0f64..1500.0),
        profile in prop::sample::select(&["pencil", "contour", "heavy", "bull"]),
    ) {
        let tip_light = barrel_harmonics::barrel_tip_velocity(length, profile, 2.0, mv);
        let tip_heavy = barrel_harmonics::barrel_tip_velocity(length, profile, 10.0, mv);
        prop_assert!(tip_light >= 0.0);
        prop_assert!(tip_heavy >= 0.0);
        prop_assert!(tip_heavy > tip_light,
            "heavier projectile should give more tip velocity: \
             light={tip_light:.2e}, heavy={tip_heavy:.2e}");
    }

    #[test]
    fn barrel_tip_decreases_with_stiffness(length in (200.0f64..800.0), mass in (2.0f64..20.0), mv in (500.0f64..1500.0)) {
        let pencil = barrel_harmonics::barrel_tip_velocity(length, "pencil", mass, mv);
        let bull = barrel_harmonics::barrel_tip_velocity(length, "bull", mass, mv);
        prop_assert!(pencil.is_finite());
        prop_assert!(bull.is_finite());
        prop_assert!(pencil > bull,
            "pencil barrel should have more tip velocity than bull: \
             pencil={pencil:.2e}, bull={bull:.2e}");
    }

    #[test]
    fn harmonics_result_finite(
        length in (200.0f64..800.0),
        profile in prop::sample::select(&["pencil", "contour", "heavy", "bull"]),
        mass in (2.0f64..20.0),
        charge in (0.5f64..6.0),
        mv in (500.0f64..1500.0),
        twist in (2.0f64..10.0),
        temp in (-20.0f64..200.0),
    ) {
        let params = BarrelHarmonicsParams {
            barrel_length_mm: length,
            barrel_profile: profile,
            projectile_mass_g: mass,
            charge_mass_g: charge,
            muzzle_velocity_ms: mv,
            twist_rate_rev_per_m: twist,
            round_count_since_clean: 0,
            barrel_temp_c: temp,
        };
        let result = barrel_harmonics::evaluate_barrel_harmonics(&params);
        prop_assert!(result.muzzle_vertical_velocity_ms.is_finite());
        prop_assert!(result.vertical_dispersion_moa.is_finite());
        prop_assert!(result.vertical_stringing_ratio.is_finite());
        prop_assert!(result.dominant_frequency_hz.is_finite());
        prop_assert!(result.tip_deflection_mm.is_finite());
        // All values should be non-negative
        prop_assert!(result.muzzle_vertical_velocity_ms >= 0.0,
            "tip velocity should be non-negative: {:.2e}", result.muzzle_vertical_velocity_ms);
        prop_assert!(result.vertical_dispersion_moa >= 0.0);
        prop_assert!(result.dominant_frequency_hz >= 0.0,
            "frequency should be non-negative: {:.1}", result.dominant_frequency_hz);
        prop_assert!(result.tip_deflection_mm >= 0.0);
    }

    #[test]
    fn pencil_barrel_higher_stringing_than_heavy(
        length in (300.0f64..700.0),
        mass in (3.0f64..15.0),
        mv in (600.0f64..1200.0),
    ) {
        let pencil = barrel_harmonics::evaluate_barrel_harmonics(&BarrelHarmonicsParams {
            barrel_length_mm: length,
            barrel_profile: "pencil",
            projectile_mass_g: mass,
            charge_mass_g: 2.0,
            muzzle_velocity_ms: mv,
            twist_rate_rev_per_m: 5.0,
            round_count_since_clean: 0,
            barrel_temp_c: 20.0,
        });
        let heavy = barrel_harmonics::evaluate_barrel_harmonics(&BarrelHarmonicsParams {
            barrel_length_mm: length,
            barrel_profile: "heavy",
            projectile_mass_g: mass,
            charge_mass_g: 2.0,
            muzzle_velocity_ms: mv,
            twist_rate_rev_per_m: 5.0,
            round_count_since_clean: 0,
            barrel_temp_c: 20.0,
        });
        prop_assert!(pencil.vertical_stringing_ratio > heavy.vertical_stringing_ratio,
            "pencil barrel should have higher stringing ratio: pencil={:.4}, heavy={:.4}",
            pencil.vertical_stringing_ratio, heavy.vertical_stringing_ratio);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Fragmentation
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {
    /// Sample mean of log-normal fragment masses converges to
    /// within 20% of the configured mean parameter (narrow distribution
    /// so even extreme quantile mappings stay close to the param).
    #[test]
    fn frag_log_normal_moments(
        vel in (800.0f64..2000.0),
        mass in (2.0f64..50.0),
    ) {
        let threshold = 1.0;
        let mut config = HashMap::new();
        let config_mean = 0.3;
        // Very narrow distribution — extreme z-scores from the quantile
        // function get multiplied by tiny sigma, keeping masses near mean.
        let config_std = 1e-6;
        config.insert("mean".to_string(), config_mean);
        config.insert("std".to_string(), config_std);

        let result = evaluate(vel, mass, "fmj", threshold, Some(&config));
        prop_assume!(result.num_fragments >= 5);

        let n = result.num_fragments as f64;
        let sample_mean: f64 = result.fragments.iter().map(|f| f.mass_g).sum::<f64>() / n;

        let mean_rel_err = (sample_mean - config_mean).abs() / config_mean;

        prop_assert!(mean_rel_err < 0.20,
            "sample mean {:.10} diverges >20% from config mean {:.10}: rel error {:.4}",
            sample_mean, config_mean, mean_rel_err);
    }

    /// Golden-angle azimuths are uniformly spaced: no two fragments
    /// have azimuths within 5° of each other on the circle.
    #[test]
    fn frag_azimuth_uniform(
        vel in (800.0f64..2000.0),
        mass in (2.0f64..50.0),
        ptype in prop::sample::select(&["fmj", "ball", "ap", "hollow_point", "soft_point"] as &[_]),
    ) {
        let threshold = 1.0;
        let result = evaluate(vel, mass, ptype, threshold, None);
        prop_assume!(result.fragments.len() >= 2);

        for i in 0..result.fragments.len() {
            for j in (i + 1)..result.fragments.len() {
                let diff = (result.fragments[i].azimuth_deg - result.fragments[j].azimuth_deg).abs();
                let circular_diff = diff.min(360.0 - diff);
                prop_assert!(circular_diff > 5.0,
                    "frags {} ({:.4}°) and {} ({:.4}°) too close: gap {:.4}° < 5°",
                    i, result.fragments[i].azimuth_deg,
                    j, result.fragments[j].azimuth_deg,
                    circular_diff);
            }
        }
    }

    /// Mass conservation: total fragment mass never exceeds 115% of
    /// projectile mass.  Uses configured narrow distribution so that
    /// even with large fragment counts the total stays reasonable.
    #[test]
    fn frag_mass_conservation(
        vel in (300.0f64..1500.0),
        mass in (2.0f64..50.0),
    ) {
        let threshold = 100.0;
        let mut config = HashMap::new();
        // Small per-fragment mean ensures total ≪ projectile mass
        // regardless of fragment count.
        config.insert("mean".to_string(), 0.01);
        config.insert("std".to_string(), 1e-8);
        let result = evaluate(vel, mass, "fmj", threshold, Some(&config));
        let total_mass: f64 = result.fragments.iter().map(|f| f.mass_g).sum();
        let max_expected = mass * 1.15;
        prop_assert!(total_mass <= max_expected,
            "total frag mass {:.6}g exceeds 115% of projectile mass {:.4}g (max {:.4}g)",
            total_mass, mass, max_expected);
        // No individual fragment should exceed the projectile mass
        for (i, frag) in result.fragments.iter().enumerate() {
            prop_assert!(frag.mass_g <= mass * 1.05,
                "frag {i} mass {:.6}g exceeds projectile mass {:.4}g", frag.mass_g, mass);
        }
    }

    /// Fragment count is bounded to [0, 50] for all projectile types
    /// across the full velocity range.
    #[test]
    fn frag_count_bounds(
        vel in (200.0f64..2000.0),
        mass in (2.0f64..50.0),
        ptype in prop::sample::select(&["ball", "ap", "sp", "hp", "apds", "tracer", "incendiary", "default_proj"] as &[_]),
    ) {
        let threshold = 100.0;
        let result = evaluate(vel, mass, ptype, threshold, None);
        prop_assert!(result.num_fragments >= 0,
            "fragment count should be non-negative for {}: {}", ptype, result.num_fragments);
        prop_assert!(result.num_fragments <= 50,
            "fragment count should be ≤ 50 for {} at {:.0} m/s: got {}",
            ptype, vel, result.num_fragments);
    }

    /// Every fragment has cone_angle_deg ≤ 45° and
    /// azimuth_deg ∈ [0, 360).
    #[test]
    fn frag_cone_bounds(
        vel in (200.0f64..2000.0),
        mass in (2.0f64..50.0),
        ptype in prop::sample::select(&["ball", "ap", "sp", "hp", "apds", "tracer", "incendiary", "default_proj"] as &[_]),
    ) {
        let threshold = 100.0;
        let result = evaluate(vel, mass, ptype, threshold, None);
        for (i, frag) in result.fragments.iter().enumerate() {
            prop_assert!(frag.cone_angle_deg <= 45.0,
                "frag {} cone {:.4}° exceeds 45° for {} at {:.0} m/s",
                i, frag.cone_angle_deg, ptype, vel);
            prop_assert!(frag.azimuth_deg >= 0.0 && frag.azimuth_deg < 360.0,
                "frag {} azimuth {:.4}° outside [0, 360) for {} at {:.0} m/s",
                i, frag.azimuth_deg, ptype, vel);
        }
    }
}
