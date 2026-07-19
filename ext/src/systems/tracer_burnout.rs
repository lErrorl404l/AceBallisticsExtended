// ABE - Tracer Burnout Model
//
// Models the pyrotechnic trace element burnout in tracer ammunition.
// Different tracer types have different burn durations and burnout
// characteristics based on their chemical composition and element
// geometry.
//
// The trace element is typically a pressed mixture of strontium
// nitrate (Sr(NO₃)₂), magnesium powder, and a binder/oxidiser.
// Burn rate depends on velocity (faster flight = faster consumption),
// temperature (cold reduces burn rate), and altitude (reduced oxygen
// at altitude slows combustion).
// ponytail: not wired into the projectile state update — whole module is forward-looking

#![allow(dead_code)]
//
// References:
//   - US Army ARDEC Tracer Ammunition Specifications
//   - MIL-STD-652 (Tracer Ammunition Performance Requirements)
//   - TM 43-0001-27 (Army Ammunition Data Sheets)
//   - Nennstiel, R. "Behaviour of Tracer Projectiles" (1999)
//   - Federal Cartridge / ATK Tracer Technical Data

// ── Constants ──────────────────────────────────────────────────────────────────

/// Tracer element mass as fraction of total projectile mass (~2-5%).
/// 3.5% is typical for 5.56 mm and 7.62 mm tracer rounds.
const TRACER_MASS_FRACTION: f64 = 0.035;

/// Ballistic coefficient change after tracer element burnout (fraction).
/// The projectile is lighter and the CG has shifted, typically
/// reducing BC by 1-3%.
const BC_CHANGE_FRACTION: f64 = -0.02;

// ── Nominal burn times at 15 °C sea level, 800 m/s reference ───────────────────

/// M856 (5.56 mm short tracer): ~2.25 s burn time.
const M856_BURN_TIME_S: f64 = 2.25;

/// M62 (7.62 mm standard tracer): ~3.25 s burn time.
const M62_BURN_TIME_S: f64 = 3.25;

/// M196 (5.56 mm dim tracer): ~2.50 s burn time.
const M196_BURN_TIME_S: f64 = 2.50;

/// M17 (.50 cal standard tracer): ~3.50 s burn time.
const M17_BURN_TIME_S: f64 = 3.50;

/// Long-range tracer: ~4.50 s burn time.
const LONG_TRACER_BURN_TIME_S: f64 = 4.50;

/// Reduced-signature dim tracer: ~2.50 s burn time.
const DIM_TRACER_BURN_TIME_S: f64 = 2.50;

/// Reference velocity (m/s) at which nominal burn times are defined.
/// Tracer burn rate scales linearly with velocity: a projectile at
/// 1600 m/s consumes the element twice as fast (half the burn time).
const VELOCITY_REF_MS: f64 = 800.0;

/// Temperature coefficient: fractional change in burn time per °C
/// deviation from 15 °C reference.  Cold slows combustion (longer
/// burn), heat accelerates it (shorter burn).
const TEMP_COEFF_PER_C: f64 = 0.003;

/// Altitude coefficient: fractional change in burn time per km
/// above sea level.  Reduced oxygen at altitude slows combustion.
const ALT_COEFF_PER_KM: f64 = 0.02;

/// Fraction of burn duration at which dimming begins.
/// Full brightness for the first 70 % of burn life, then linear
/// dimming over the remaining 30 %.
const BRIGHTNESS_DIM_START: f64 = 0.70;

/// Hard clamp on the velocity burn-rate factor to prevent
/// division-by-zero and physically impossible values.
const MIN_VEL_FACTOR: f64 = 0.25;
const MAX_VEL_FACTOR: f64 = 3.00;

/// Clamp bounds for temperature and altitude factors.
const MIN_ENV_FACTOR: f64 = 0.50;
const MAX_ENV_FACTOR: f64 = 2.00;

// ── Types ──────────────────────────────────────────────────────────────────────

/// Tracer ammunition type / NATO designation.
///
/// Each type has a different pyrotechnic element length, composition,
/// and burn duration profile.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TracerType {
    /// 5.56 mm short tracer — visible to ~500-700 m (2.0-2.5 s).
    M856,
    /// 7.62 mm standard tracer — visible to ~800-1000 m (3.0-3.5 s).
    M62,
    /// 5.56 mm dim tracer — reduced signature, ~2.5 s.
    M196,
    /// .50 cal standard tracer — ~3.5 s.
    M17,
    /// Extended-range tracer — visible beyond 1200 m (4.0-5.0 s).
    LongTracer,
    /// Reduced-signature tracer — ~2.5 s.
    DimTracer,
}

/// Input parameters for a tracer burnout evaluation.
#[derive(Debug, Clone, Copy)]
pub struct TracerBurnoutParams {
    /// Tracer ammunition type.
    pub tracer_type: TracerType,

    /// Current projectile time-of-flight (seconds from muzzle).
    pub time_of_flight_s: f64,

    /// Current projectile velocity (m/s).
    pub velocity_ms: f64,

    /// Ambient air temperature (°C).  15 °C is the reference.
    pub temperature_c: f64,

    /// Altitude above sea level (metres).  0 = sea level.
    pub altitude_m: f64,

    /// Current down-range distance (metres).
    pub range_m: f64,

    /// Total projectile mass in grams (needed for mass-reduction
    /// and BC-change computation).
    pub projectile_mass_g: f64,
}

/// Result of a tracer burnout evaluation.
#[derive(Debug, Clone, Copy)]
pub struct TracerBurnoutResult {
    /// Whether the tracer element has been fully consumed by the
    /// current time-of-flight.
    pub burned_out: bool,

    /// Expected burnout time (seconds from muzzle), considering
    /// velocity, temperature, and altitude effects.
    pub burnout_time_s: f64,

    /// Approximate down-range distance at which burnout occurs
    /// (metres).  A simple estimate using current velocity as a
    /// proxy for average velocity over the remaining flight.
    pub burnout_range_m: f64,

    /// Current brightness of the tracer pyrotechnic flame.
    ///   1.0 = full brightness
    ///   0.0 = fully burned out
    /// Values between 0.0 and 1.0 during the dimming phase.
    pub brightness_fraction: f64,

    /// Mass of the tracer element consumed so far (grams).  The
    /// tracer element is typically 2-5 % of total projectile mass.
    pub mass_reduction_g: f64,

    /// Percentage change in ballistic coefficient after burnout.
    /// Typical range: -1.0 to -3.0 (percent, negative = worse BC).
    /// Zero before burnout occurs.
    pub bc_change_pct: f64,

    /// Projectile velocity at the burnout point (m/s).
    /// When `burned_out == true` this is the current velocity
    /// (the best estimate available without a full trajectory).
    /// Zero when burnout has not yet occurred.
    pub after_burnout_velocity_ms: f64,
}

// ── Nominal burn times ─────────────────────────────────────────────────────────

/// Return the nominal burn time (seconds) at reference conditions
/// (15 °C, sea level, 800 m/s) for the given tracer type.
fn nominal_burn_time(tracer_type: TracerType) -> f64 {
    match tracer_type {
        TracerType::M856 => M856_BURN_TIME_S,
        TracerType::M62 => M62_BURN_TIME_S,
        TracerType::M196 => M196_BURN_TIME_S,
        TracerType::M17 => M17_BURN_TIME_S,
        TracerType::LongTracer => LONG_TRACER_BURN_TIME_S,
        TracerType::DimTracer => DIM_TRACER_BURN_TIME_S,
    }
}

/// Tracer element mass in grams for a given total projectile mass.
fn tracer_mass_g(projectile_mass_g: f64) -> f64 {
    (projectile_mass_g * TRACER_MASS_FRACTION).max(0.0)
}

// ── Environment scaling factors ────────────────────────────────────────────────

/// Velocity-dependent burn-time multiplier.
///
/// Faster projectiles consume the tracer element more quickly
/// (factor < 1.0 → shorter burn time).
fn velocity_factor(velocity_ms: f64) -> f64 {
    if velocity_ms <= 0.0 {
        return MAX_VEL_FACTOR;
    }
    (VELOCITY_REF_MS / velocity_ms).clamp(MIN_VEL_FACTOR, MAX_VEL_FACTOR)
}

/// Temperature-dependent burn-time multiplier.
///
/// Cold slows combustion (factor > 1.0 → longer burn time),
/// heat accelerates (factor < 1.0 → shorter).
fn temperature_factor(temp_c: f64) -> f64 {
    let f = 1.0 - TEMP_COEFF_PER_C * (temp_c - 15.0);
    f.clamp(MIN_ENV_FACTOR, MAX_ENV_FACTOR)
}

/// Altitude-dependent burn-time multiplier.
///
/// Reduced oxygen at altitude slows combustion
/// (factor > 1.0 → longer burn time).
fn altitude_factor(altitude_m: f64) -> f64 {
    let f = 1.0 + ALT_COEFF_PER_KM * (altitude_m / 1000.0);
    f.clamp(MIN_ENV_FACTOR, MAX_ENV_FACTOR)
}

/// Overall effective burn time combining all environmental factors.
fn effective_burn_time(
    tracer_type: TracerType,
    velocity_ms: f64,
    temp_c: f64,
    altitude_m: f64,
) -> f64 {
    let nominal = nominal_burn_time(tracer_type);
    nominal
        * velocity_factor(velocity_ms)
        * temperature_factor(temp_c)
        * altitude_factor(altitude_m)
}

// ── Brightness computation ─────────────────────────────────────────────────────

/// Compute brightness fraction at a given point in the burn cycle.
///
/// Full brightness (1.0) for the first `BRIGHTNESS_DIM_START` fraction
/// of total burn time, then linear dimming to zero at burnout.
fn compute_brightness(burn_fraction: f64) -> f64 {
    if burn_fraction <= 0.0 {
        return 1.0;
    }
    if burn_fraction >= 1.0 {
        return 0.0;
    }
    if burn_fraction < BRIGHTNESS_DIM_START {
        1.0
    } else {
        let dim_progress = (burn_fraction - BRIGHTNESS_DIM_START) / (1.0 - BRIGHTNESS_DIM_START);
        (1.0 - dim_progress).max(0.0)
    }
}

// ── Estimate burnout range ─────────────────────────────────────────────────────

/// Estimate the down-range distance at which burnout occurs.
///
/// Uses the current velocity to project forward from current range.
/// Simple linear extrapolation: assumes velocity stays approximately
/// constant for the remaining burn duration.
fn estimate_burnout_range(
    range_m: f64,
    velocity_ms: f64,
    time_of_flight_s: f64,
    burnout_time_s: f64,
) -> f64 {
    let remaining = burnout_time_s - time_of_flight_s;
    if remaining <= 0.0 {
        range_m
    } else {
        range_m + velocity_ms * remaining
    }
}

/// Estimate projectile velocity at the burnout point.
///
/// When burnout has already occurred the current velocity is the
/// best available estimate.  Before burnout we project forward
/// using a simple linear deceleration model derived from the
/// average velocity so far.
fn estimate_burnout_velocity(
    velocity_ms: f64,
    time_of_flight_s: f64,
    range_m: f64,
    burnout_time_s: f64,
) -> f64 {
    if time_of_flight_s >= burnout_time_s {
        return velocity_ms;
    }
    if time_of_flight_s <= 0.0 || range_m <= 0.0 {
        return velocity_ms; // insufficient data for better estimate
    }
    // Average velocity so far
    let avg_v = range_m / time_of_flight_s;
    // Deceleration estimate
    let decel = (avg_v - velocity_ms) / time_of_flight_s;
    let remaining = burnout_time_s - time_of_flight_s;
    (velocity_ms - decel * remaining).max(0.0)
}

// ── Public API ─────────────────────────────────────────────────────────────────

/// Evaluate whether a tracer projectile has burned out at the given
/// time-of-flight and environmental conditions.
///
/// # Arguments
/// * `params` — Tracer type, current flight state, and environmental
///   conditions.
///
/// # Returns
/// [`TracerBurnoutResult`] with burnout status, timing, range,
/// brightness, mass reduction, and BC change.
///
/// # Physics summary
/// | Factor | Effect on burn time |
/// |--------|---------------------|
/// | Velocity | Faster → shorter burn (inversely proportional) |
/// | Temperature | Hotter → shorter burn (~0.3 % / °C) |
/// | Altitude | Higher → longer burn (~2 % / km) |
/// | Brightness | Full for first 70 %, dims over last 30 % |
/// | Mass reduction | ~3.5 % of projectile mass at full burnout |
/// | BC change | -2 % after burnout (lighter, CG shift) |
pub fn evaluate_tracer_burnout(params: &TracerBurnoutParams) -> TracerBurnoutResult {
    // ── Guard: degenerate inputs ──────────────────────────────────────
    let clamped_tof = params.time_of_flight_s.max(0.0);

    // ── Effective burn time ──────────────────────────────────────────
    let burnout_time_s = effective_burn_time(
        params.tracer_type,
        params.velocity_ms,
        params.temperature_c,
        params.altitude_m,
    );

    // ── Burn fraction ───────────────────────────────────────────────
    let burn_fraction = if burnout_time_s > 0.0 {
        (clamped_tof / burnout_time_s).min(1.0)
    } else {
        1.0
    };
    let burned_out = clamped_tof >= burnout_time_s;

    // ── Brightness ──────────────────────────────────────────────────
    let brightness = compute_brightness(burn_fraction);

    // ── Mass reduction ──────────────────────────────────────────────
    let tracer_mass = tracer_mass_g(params.projectile_mass_g);
    let mass_reduction_g = tracer_mass * burn_fraction;

    // ── BC change ───────────────────────────────────────────────────
    let bc_change_pct = if burned_out {
        BC_CHANGE_FRACTION * 100.0
    } else {
        0.0
    };

    // ── Burnout range ───────────────────────────────────────────────
    let burnout_range_m = estimate_burnout_range(
        params.range_m,
        params.velocity_ms,
        clamped_tof,
        burnout_time_s,
    );

    // ── Velocity at burnout ─────────────────────────────────────────
    let after_burnout_velocity_ms = if burned_out {
        params.velocity_ms
    } else {
        estimate_burnout_velocity(
            params.velocity_ms,
            clamped_tof,
            params.range_m,
            burnout_time_s,
        )
    };

    TracerBurnoutResult {
        burned_out,
        burnout_time_s,
        burnout_range_m,
        brightness_fraction: brightness,
        mass_reduction_g,
        bc_change_pct,
        after_burnout_velocity_ms,
    }
}

/// Compute the tracer brightness fraction at a given time for visual
/// effects, independent of a full projectile state.
///
/// Uses the nominal burn time for the given tracer type at 800 m/s
/// reference velocity, adjusted for temperature.  This is a simpler
/// query than [`evaluate_tracer_burnout`] when only the visual
/// brightness is needed.
///
/// # Arguments
/// * `time_s` — Seconds from muzzle (time-of-flight).
/// * `tracer_type` — Tracer ammunition type.
/// * `temp_c` — Ambient temperature (°C); affects burn rate.
///
/// # Returns
/// Brightness fraction: 1.0 = full, 0.0 = burned out.
pub fn tracer_brightness_at_time(time_s: f64, tracer_type: TracerType, temp_c: f64) -> f64 {
    if time_s <= 0.0 {
        return 1.0;
    }

    let nominal = nominal_burn_time(tracer_type);
    let temp_factor = temperature_factor(temp_c);
    let adjusted_burn_time = nominal * temp_factor;

    if adjusted_burn_time <= 0.0 {
        return 0.0;
    }

    let fraction = time_s / adjusted_burn_time;
    compute_brightness(fraction)
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── All tracer types ─────────────────────────────────────────────────

    #[test]
    fn m856_short_tracer_burns_out_by_2_5s() {
        let params = TracerBurnoutParams {
            tracer_type: TracerType::M856,
            time_of_flight_s: 2.5,
            velocity_ms: 850.0,
            temperature_c: 15.0,
            altitude_m: 0.0,
            range_m: 1500.0,
            projectile_mass_g: 4.0,
        };
        let r = evaluate_tracer_burnout(&params);
        assert!(r.burned_out, "M856 should burn out by 2.5 s");
        assert!(r.burnout_time_s >= 1.5 && r.burnout_time_s <= 3.0);
        assert!(r.brightness_fraction < 0.01);
    }

    #[test]
    fn m62_standard_tracer_burns_out_by_4_0s() {
        let params = TracerBurnoutParams {
            tracer_type: TracerType::M62,
            time_of_flight_s: 4.0,
            velocity_ms: 800.0,
            temperature_c: 15.0,
            altitude_m: 0.0,
            range_m: 2500.0,
            projectile_mass_g: 9.5,
        };
        let r = evaluate_tracer_burnout(&params);
        assert!(r.burned_out, "M62 should burn out by 4.0 s");
        assert!(r.burnout_time_s >= 2.5 && r.burnout_time_s <= 4.0);
    }

    #[test]
    fn m196_dim_tracer_burns_out_by_3_0s() {
        let params = TracerBurnoutParams {
            tracer_type: TracerType::M196,
            time_of_flight_s: 3.0,
            velocity_ms: 820.0,
            temperature_c: 15.0,
            altitude_m: 0.0,
            range_m: 1800.0,
            projectile_mass_g: 4.0,
        };
        let r = evaluate_tracer_burnout(&params);
        assert!(r.burned_out, "M196 should burn out by 3.0 s");
        assert!(r.burnout_time_s >= 1.8 && r.burnout_time_s <= 3.2);
    }

    #[test]
    fn m17_50cal_tracer_burns_out_by_4_5s() {
        let params = TracerBurnoutParams {
            tracer_type: TracerType::M17,
            time_of_flight_s: 4.5,
            velocity_ms: 850.0,
            temperature_c: 15.0,
            altitude_m: 0.0,
            range_m: 3000.0,
            projectile_mass_g: 42.0,
        };
        let r = evaluate_tracer_burnout(&params);
        assert!(r.burned_out, "M17 should burn out by 4.5 s");
        assert!(r.burnout_time_s >= 2.5 && r.burnout_time_s <= 4.5);
    }

    #[test]
    fn long_tracer_burns_out_by_5_5s() {
        let params = TracerBurnoutParams {
            tracer_type: TracerType::LongTracer,
            time_of_flight_s: 5.5,
            velocity_ms: 800.0,
            temperature_c: 15.0,
            altitude_m: 0.0,
            range_m: 3500.0,
            projectile_mass_g: 10.0,
        };
        let r = evaluate_tracer_burnout(&params);
        assert!(r.burned_out, "LongTracer should burn out by 5.5 s");
        assert!(r.burnout_time_s >= 3.5 && r.burnout_time_s <= 5.5);
    }

    #[test]
    fn dim_tracer_burns_out_by_3_5s() {
        let params = TracerBurnoutParams {
            tracer_type: TracerType::DimTracer,
            time_of_flight_s: 3.5,
            velocity_ms: 800.0,
            temperature_c: 15.0,
            altitude_m: 0.0,
            range_m: 2000.0,
            projectile_mass_g: 4.0,
        };
        let r = evaluate_tracer_burnout(&params);
        assert!(r.burned_out, "DimTracer should burn out by 3.5 s");
        assert!(r.brightness_fraction < 0.01);
    }

    // ── Burnout before given TOF ─────────────────────────────────────────

    #[test]
    fn tracer_alive_before_burnout_time() {
        let params = TracerBurnoutParams {
            tracer_type: TracerType::M856,
            time_of_flight_s: 1.0, // well before nominal 2.25 s burnout
            velocity_ms: 850.0,
            temperature_c: 15.0,
            altitude_m: 0.0,
            range_m: 800.0,
            projectile_mass_g: 4.0,
        };
        let r = evaluate_tracer_burnout(&params);
        assert!(!r.burned_out, "M856 should NOT be burned out at 1.0 s");
        assert!(
            r.brightness_fraction > 0.99,
            "Brightness should be near 1.0 early in flight"
        );
        assert_eq!(r.bc_change_pct, 0.0, "BC should not change before burnout");
    }

    #[test]
    fn tracer_burns_out_exactly_at_threshold() {
        let params = TracerBurnoutParams {
            tracer_type: TracerType::M62,
            time_of_flight_s: 3.25, // exactly at nominal (800 m/s)
            velocity_ms: 800.0,
            temperature_c: 15.0,
            altitude_m: 0.0,
            range_m: 2000.0,
            projectile_mass_g: 9.5,
        };
        let r = evaluate_tracer_burnout(&params);
        assert!(
            r.burned_out || (r.brightness_fraction < 0.001),
            "M62 should be at or near burnout at 3.25 s at 800 m/s"
        );
    }

    // ── Brightness over time ─────────────────────────────────────────────

    #[test]
    fn brightness_full_during_first_70_percent() {
        let params = TracerBurnoutParams {
            tracer_type: TracerType::M62,
            time_of_flight_s: 1.5, // well before 70 % of 3.25 s
            velocity_ms: 800.0,
            temperature_c: 15.0,
            altitude_m: 0.0,
            range_m: 1000.0,
            projectile_mass_g: 9.5,
        };
        let r = evaluate_tracer_burnout(&params);
        assert!(
            r.brightness_fraction > 0.99,
            "Brightness should be full during first 70 %"
        );
    }

    #[test]
    fn brightness_dims_linearly_during_last_30_percent() {
        // At ~80 % of burn time, brightness should be ~67 %
        // (dim progress = (0.80 - 0.70) / 0.30 = 0.33 → brightness = 0.67)
        let params = TracerBurnoutParams {
            tracer_type: TracerType::M62,
            time_of_flight_s: 2.6, // 80 % of 3.25
            velocity_ms: 800.0,
            temperature_c: 15.0,
            altitude_m: 0.0,
            range_m: 1800.0,
            projectile_mass_g: 9.5,
        };
        let r = evaluate_tracer_burnout(&params);
        assert!(!r.burned_out, "Should not be fully burned out at 80 %");
        assert!(
            (r.brightness_fraction - 0.67).abs() < 0.05,
            "Brightness should be ~0.67 at 80 % burn, got {}",
            r.brightness_fraction
        );
    }

    #[test]
    fn brightness_zero_at_burnout() {
        let params = TracerBurnoutParams {
            tracer_type: TracerType::M856,
            time_of_flight_s: 3.0,
            velocity_ms: 850.0,
            temperature_c: 15.0,
            altitude_m: 0.0,
            range_m: 1800.0,
            projectile_mass_g: 4.0,
        };
        let r = evaluate_tracer_burnout(&params);
        assert!(r.burned_out);
        assert!(
            r.brightness_fraction < 0.001,
            "Brightness should be ~0 after burnout"
        );
    }

    #[test]
    fn tracer_brightness_at_time_standalone() {
        // At 0 s: full brightness
        let b0 = tracer_brightness_at_time(0.0, TracerType::M62, 15.0);
        assert!((b0 - 1.0).abs() < 0.01, "At t=0 brightness should be 1.0");

        // At 4.0 s (past nominal 3.25): burned out
        let b4 = tracer_brightness_at_time(4.0, TracerType::M62, 15.0);
        assert!(b4 < 0.01, "M62 should be burned out by 4 s");

        // At 2.0 s (≈62 %): full brightness
        let b2 = tracer_brightness_at_time(2.0, TracerType::M62, 15.0);
        assert!(b2 > 0.99, "M62 should still be full at 2 s");

        // Dimming phase (3.0 s ≈ 92 %)
        let b3 = tracer_brightness_at_time(3.0, TracerType::M62, 15.0);
        assert!(b3 > 0.0 && b3 < 1.0, "Should be dimming at 3.0 s: {}", b3);
    }

    // ── Temperature effect ────────────────────────────────────────────────

    #[test]
    fn cold_temperature_increases_burn_time() {
        let cold = TracerBurnoutParams {
            tracer_type: TracerType::M62,
            time_of_flight_s: 3.5,
            velocity_ms: 800.0,
            temperature_c: -10.0, // 25 °C below reference
            altitude_m: 0.0,
            range_m: 2200.0,
            projectile_mass_g: 9.5,
        };
        let hot = TracerBurnoutParams {
            tracer_type: TracerType::M62,
            time_of_flight_s: 3.5,
            velocity_ms: 800.0,
            temperature_c: 40.0, // 25 °C above reference
            altitude_m: 0.0,
            range_m: 2200.0,
            projectile_mass_g: 9.5,
        };
        let r_cold = evaluate_tracer_burnout(&cold);
        let r_hot = evaluate_tracer_burnout(&hot);

        assert!(
            r_cold.burnout_time_s > r_hot.burnout_time_s,
            "Cold ({} s) should have longer burn time than hot ({} s)",
            r_cold.burnout_time_s,
            r_hot.burnout_time_s
        );
    }

    #[test]
    fn hot_temperature_reduces_burn_time() {
        let params_15c = TracerBurnoutParams {
            tracer_type: TracerType::M856,
            time_of_flight_s: 2.5,
            velocity_ms: 850.0,
            temperature_c: 15.0,
            altitude_m: 0.0,
            range_m: 1500.0,
            projectile_mass_g: 4.0,
        };
        let params_50c = TracerBurnoutParams {
            tracer_type: TracerType::M856,
            time_of_flight_s: 2.5,
            velocity_ms: 850.0,
            temperature_c: 50.0,
            altitude_m: 0.0,
            range_m: 1500.0,
            projectile_mass_g: 4.0,
        };
        let r_15 = evaluate_tracer_burnout(&params_15c);
        let r_50 = evaluate_tracer_burnout(&params_50c);

        assert!(
            r_50.burnout_time_s < r_15.burnout_time_s,
            "50 °C ({:.3} s) should burn faster than 15 °C ({:.3} s)",
            r_50.burnout_time_s,
            r_15.burnout_time_s,
        );
    }

    // ── Altitude effect ──────────────────────────────────────────────────

    #[test]
    fn high_altitude_increases_burn_time() {
        let sea = TracerBurnoutParams {
            tracer_type: TracerType::M62,
            time_of_flight_s: 3.5,
            velocity_ms: 800.0,
            temperature_c: 15.0,
            altitude_m: 0.0,
            range_m: 2200.0,
            projectile_mass_g: 9.5,
        };
        let high = TracerBurnoutParams {
            tracer_type: TracerType::M62,
            time_of_flight_s: 3.5,
            velocity_ms: 800.0,
            temperature_c: 15.0,
            altitude_m: 3000.0,
            range_m: 2200.0,
            projectile_mass_g: 9.5,
        };
        let r_sea = evaluate_tracer_burnout(&sea);
        let r_high = evaluate_tracer_burnout(&high);

        assert!(
            r_high.burnout_time_s > r_sea.burnout_time_s,
            "Altitude should increase burn time: sea={:.3} s, 3 km={:.3} s",
            r_sea.burnout_time_s,
            r_high.burnout_time_s,
        );
    }

    // ── Velocity effect ──────────────────────────────────────────────────

    #[test]
    fn higher_velocity_reduces_burn_time() {
        let slow = TracerBurnoutParams {
            tracer_type: TracerType::M62,
            time_of_flight_s: 3.5,
            velocity_ms: 600.0,
            temperature_c: 15.0,
            altitude_m: 0.0,
            range_m: 1500.0,
            projectile_mass_g: 9.5,
        };
        let fast = TracerBurnoutParams {
            tracer_type: TracerType::M62,
            time_of_flight_s: 3.5,
            velocity_ms: 1000.0,
            temperature_c: 15.0,
            altitude_m: 0.0,
            range_m: 2500.0,
            projectile_mass_g: 9.5,
        };
        let r_slow = evaluate_tracer_burnout(&slow);
        let r_fast = evaluate_tracer_burnout(&fast);

        assert!(
            r_fast.burnout_time_s < r_slow.burnout_time_s,
            "Faster projectile should have shorter burn time: fast={:.3} s, slow={:.3} s",
            r_fast.burnout_time_s,
            r_slow.burnout_time_s,
        );
    }

    // ── Mass reduction ───────────────────────────────────────────────────

    #[test]
    fn mass_reduction_proportional_to_burn_fraction() {
        let params = TracerBurnoutParams {
            tracer_type: TracerType::M62,
            time_of_flight_s: 1.625, // ~50 % of 3.25 s
            velocity_ms: 800.0,
            temperature_c: 15.0,
            altitude_m: 0.0,
            range_m: 1000.0,
            projectile_mass_g: 10.0,
        };
        let r = evaluate_tracer_burnout(&params);

        let expected_tracer_mass = 10.0 * TRACER_MASS_FRACTION; // 0.35 g
        let expected_mass_loss = expected_tracer_mass * 0.50; // ~0.175 g at 50 %
        let diff = (r.mass_reduction_g - expected_mass_loss).abs();
        assert!(
            diff < 0.02,
            "Mass reduction at 50 % burn: expected ~{:.4} g, got {:.4} g (diff={:.4})",
            expected_mass_loss,
            r.mass_reduction_g,
            diff
        );
    }

    #[test]
    fn full_burnout_mass_reduction_equals_tracer_mass() {
        let params = TracerBurnoutParams {
            tracer_type: TracerType::M62,
            time_of_flight_s: 5.0, // past burnout
            velocity_ms: 800.0,
            temperature_c: 15.0,
            altitude_m: 0.0,
            range_m: 3000.0,
            projectile_mass_g: 9.5,
        };
        let r = evaluate_tracer_burnout(&params);
        assert!(r.burned_out);
        let expected = 9.5 * TRACER_MASS_FRACTION;
        let diff = (r.mass_reduction_g - expected).abs();
        assert!(
            diff < 0.001,
            "Full burnout mass reduction should be {:.4} g, got {:.4} g",
            expected,
            r.mass_reduction_g
        );
    }

    // ── BC change ─────────────────────────────────────────────────────────

    #[test]
    fn bc_change_only_after_burnout() {
        let before = TracerBurnoutParams {
            tracer_type: TracerType::M856,
            time_of_flight_s: 1.0, // before burnout
            velocity_ms: 850.0,
            temperature_c: 15.0,
            altitude_m: 0.0,
            range_m: 800.0,
            projectile_mass_g: 4.0,
        };
        let after = TracerBurnoutParams {
            tracer_type: TracerType::M856,
            time_of_flight_s: 3.0, // past burnout
            velocity_ms: 600.0,
            temperature_c: 15.0,
            altitude_m: 0.0,
            range_m: 1800.0,
            projectile_mass_g: 4.0,
        };
        let r_before = evaluate_tracer_burnout(&before);
        let r_after = evaluate_tracer_burnout(&after);

        assert_eq!(r_before.bc_change_pct, 0.0, "No BC change before burnout");
        assert!(
            r_after.bc_change_pct < 0.0,
            "BC should decrease after burnout: {}",
            r_after.bc_change_pct
        );
        assert!(
            (r_after.bc_change_pct - BC_CHANGE_FRACTION * 100.0).abs() < 0.01,
            "BC change should be {:.1} %, got {:.1} %",
            BC_CHANGE_FRACTION * 100.0,
            r_after.bc_change_pct
        );
    }

    // ── Edge cases ────────────────────────────────────────────────────────

    #[test]
    fn zero_time_of_flight_not_burned_out() {
        let params = TracerBurnoutParams {
            tracer_type: TracerType::M62,
            time_of_flight_s: 0.0,
            velocity_ms: 800.0,
            temperature_c: 15.0,
            altitude_m: 0.0,
            range_m: 0.0,
            projectile_mass_g: 9.5,
        };
        let r = evaluate_tracer_burnout(&params);
        assert!(!r.burned_out, "Tracer should not be burned out at t=0");
        assert!(
            (r.brightness_fraction - 1.0).abs() < 0.01,
            "Full brightness at t=0"
        );
        assert_eq!(r.mass_reduction_g, 0.0, "No mass reduction at t=0");
    }

    #[test]
    fn zero_velocity_safe_defaults() {
        let params = TracerBurnoutParams {
            tracer_type: TracerType::M62,
            time_of_flight_s: 1.0,
            velocity_ms: 0.0,
            temperature_c: 15.0,
            altitude_m: 0.0,
            range_m: 0.0,
            projectile_mass_g: 9.5,
        };
        // Should not panic
        let r = evaluate_tracer_burnout(&params);
        assert!(!r.burned_out);
    }

    #[test]
    fn negative_temperature_does_not_cause_panic() {
        let params = TracerBurnoutParams {
            tracer_type: TracerType::M62,
            time_of_flight_s: 2.0,
            velocity_ms: 800.0,
            temperature_c: -50.0,
            altitude_m: 0.0,
            range_m: 1400.0,
            projectile_mass_g: 9.5,
        };
        let r = evaluate_tracer_burnout(&params);
        // Burn time should be clamped to a sensible maximum
        assert!(r.burnout_time_s > 0.0);
    }

    #[test]
    fn deterministic_output() {
        let params = TracerBurnoutParams {
            tracer_type: TracerType::M62,
            time_of_flight_s: 2.0,
            velocity_ms: 850.0,
            temperature_c: 20.0,
            altitude_m: 500.0,
            range_m: 1500.0,
            projectile_mass_g: 9.5,
        };
        let a = evaluate_tracer_burnout(&params);
        let b = evaluate_tracer_burnout(&params);
        assert_eq!(a.burned_out, b.burned_out);
        assert!((a.burnout_time_s - b.burnout_time_s).abs() < 1e-12);
        assert!((a.brightness_fraction - b.brightness_fraction).abs() < 1e-12);
        assert!((a.mass_reduction_g - b.mass_reduction_g).abs() < 1e-12);
    }
}
