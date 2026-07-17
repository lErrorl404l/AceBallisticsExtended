// ABE - Wind Measurement Uncertainty Model
//
// Real wind measurement is never exact.  Handheld anemometers have
// instrument error (0.5-2 m/s RMS typical), gusts add unpredictable
// turbulence, and wind at the shooter's position differs from wind
// downrange.  This module models the uncertainty in wind readings
// and its effect on trajectory / impact-point dispersion.
//
// References:
//   - Litz, B., "Applied Ballistics for Long Range Shooting" (2011)
//       — wind uncertainty and probability of hit
//   - McCoy, R.L., "Modern Exterior Ballistics" (1999)
//       — crosswind deflection models
//   - WMO Guide to Meteorological Instruments and Methods of Observation
//       — anemometer accuracy classes
//   - MIL-STD-810H — wind gust profiles
//   - ICAO Standard Atmosphere — density-altitude relationships

#![allow(dead_code)]

/// Parameters for evaluating wind measurement uncertainty.
#[derive(Debug, Clone, Copy)]
pub struct WindUncertaintyParams {
    /// Measured (read) wind velocity in the x-direction (m/s).
    /// Typically downrange wind.
    pub measured_wind_x_ms: f64,
    /// Measured (read) wind velocity in the y-direction (m/s).
    /// Typically cross-range wind (the one that matters most for
    /// deflection).
    pub measured_wind_y_ms: f64,
    /// Instrument error as a percentage of reading (0–100).
    /// A value of 10 means the instrument is accurate to ±10 % of
    /// the measured value (1-sigma).  Typical handheld anemometers
    /// have 5-15 % error.
    pub instrument_error_pct: f64,
    /// Range to target (m).
    pub range_m: f64,
    /// Total time of flight to target (s).
    pub time_of_flight_s: f64,
    /// Turbulence intensity factor (0.0 = perfectly calm, 1.0 = severe
    /// turbulence).  Scales the stochastic uncertainty contribution.
    pub turbulence_intensity: f64,
    /// Altitude above sea level (m).  Higher altitude → thinner air →
    /// same wind speed imparts less force on the projectile.
    pub altitude_m: f64,
}

/// Result of a wind measurement uncertainty evaluation.
#[derive(Debug, Clone, Copy)]
pub struct WindUncertaintyResult {
    /// Most likely actual wind velocity in the x-direction (m/s).
    /// The measured value corrected for systematic instrument bias.
    pub effective_wind_x_ms: f64,
    /// Most likely actual wind velocity in the y-direction (m/s).
    pub effective_wind_y_ms: f64,
    /// 1-sigma uncertainty in the x-wind estimate (m/s).
    pub wind_uncertainty_x_ms: f64,
    /// 1-sigma uncertainty in the y-wind estimate (m/s).
    pub wind_uncertainty_y_ms: f64,
    /// 1-sigma uncertainty in impact-point deflection due to wind
    /// uncertainty (m).  How much the round may drift left/right
    /// beyond the expected wind drift.
    pub drift_uncertainty_m: f64,
    /// 95 % confidence interval horizontal spread (m) — the width
    /// of the band that contains 95 % of probable impact points
    /// given wind uncertainty alone.
    pub probability_band_width_m: f64,
}

// ── Constants ──────────────────────────────────────────────────────────────────

/// Reference air density at sea level (kg/m³), ICAO standard.
const REF_DENSITY_KGM3: f64 = 1.225;

/// Air density at 1000 m altitude, ICAO standard (kg/m³).
const DENSITY_AT_1000M_KGM3: f64 = 1.112;

/// Density scale height (m) for exponential decay approximation.
const DENSITY_SCALE_HEIGHT_M: f64 = 8500.0;

/// Minimum instrument error as a fraction of reading (cannot be zero —
/// even the best instruments have some noise).
const MIN_INSTRUMENT_ERROR_FRAC: f64 = 0.005; // 0.5 % minimum

/// Turbulence scaling factor (m^{-1/2}) — connects turbulence intensity
/// to position-dependent uncertainty.
const TURBULENCE_SCALE: f64 = 0.1;

/// Systematic bias fraction (instrument tends to read low or high).
/// Typical bias for cup/vane anemometers: 1-3 % of reading.
const INSTRUMENT_BIAS_FRAC: f64 = 0.02;

// ── Core evaluation ────────────────────────────────────────────────────────────

/// Evaluate wind measurement uncertainty and its effect on trajectory.
///
/// The model combines three sources of uncertainty:
///
/// 1. **Instrument error** — the anemometer has a systematic bias and
///    a random (Gaussian) error, both proportional to the reading.
///    Typical handheld anemometers: 5-15 % of reading RMS.
///
/// 2. **Turbulence / gust uncertainty** — even if the average wind is
///    known, gusts and eddies cause the instantaneous wind at any
///    point along the trajectory to differ from the measured value.
///    This scales with sqrt(range) and turbulence intensity.
///
/// 3. **Altitude correction** — thinner air at altitude reduces the
///    force imparted by a given wind speed.  The effective wind
///    uncertainty is scaled by sqrt(density_ratio).
///
/// The effective (most likely) wind is the measured wind adjusted
/// for the systematic instrument bias.  The 1-sigma uncertainty combines
/// instrument error and turbulence in quadrature.  The drift uncertainty
/// is then wind_uncertainty × time_of_flight (approximating the
/// cross-range deflection from a constant erroneous wind).
///
/// The 95 % probability band width is approximately 3.92 × drift_uncertainty
/// (2 × 1.96 σ for the two-sided horizontal spread).
pub fn evaluate_wind_uncertainty(params: &WindUncertaintyParams) -> WindUncertaintyResult {
    // ── Clamp inputs ───────────────────────────────────────────────────
    let instrument_err = params.instrument_error_pct.clamp(0.0, 100.0) / 100.0;
    let instrument_err = instrument_err.max(MIN_INSTRUMENT_ERROR_FRAC);
    let turbulence = params.turbulence_intensity.clamp(0.0, 1.0);
    let range = params.range_m.max(0.0);
    let tof = params.time_of_flight_s.max(0.0);
    let altitude = params.altitude_m.max(0.0);

    // ── Density ratio (altitude correction) ────────────────────────────
    // Exponential density decay: ρ(h) = ρ₀ · exp(-h / H)
    let density_ratio = (-altitude / DENSITY_SCALE_HEIGHT_M).exp();
    // Density ratio^0.5 → force on projectile scales with sqrt(ρ)
    let alt_factor = density_ratio.sqrt();

    // ── Effective wind (corrected for systematic bias) ─────────────────
    let wind_magnitude =
        (params.measured_wind_x_ms.powi(2) + params.measured_wind_y_ms.powi(2)).sqrt();

    let (effective_x, effective_y) = if wind_magnitude > 0.001 {
        // Systematic bias: instrument reads 1 + bias fraction of true.
        // True = measured / (1 + bias). At low wind this correction
        // is negligible.
        let bias_correction = 1.0 / (1.0 + INSTRUMENT_BIAS_FRAC);
        let scale = bias_correction;
        (
            params.measured_wind_x_ms * scale,
            params.measured_wind_y_ms * scale,
        )
    } else {
        (0.0, 0.0)
    };

    // ── Instrument error contribution (1-sigma) ────────────────────────
    // Proportional to the measured wind magnitude.
    let instrument_sigma = instrument_err * wind_magnitude;

    // Apportion to x and y components proportional to their contribution.
    let (inst_sigma_x, inst_sigma_y) = if wind_magnitude > 0.001 {
        let frac_x = (params.measured_wind_x_ms / wind_magnitude).abs();
        let frac_y = (params.measured_wind_y_ms / wind_magnitude).abs();
        // The quadrature sum should equal instrument_sigma, but for
        // simplicity we split by fraction.
        (instrument_sigma * frac_x, instrument_sigma * frac_y)
    } else {
        // Zero wind → instrument error is a small absolute floor
        let floor = 0.2; // 0.2 m/s minimum instrument noise
        (floor, floor)
    };

    // ── Turbulence contribution (1-sigma) ──────────────────────────────
    // Scales with sqrt(range) and turbulence intensity.
    // Even in calm conditions, there is a residual air motion floor.
    let turb_base = 0.2 * (1.0 + 3.0 * turbulence); // 0.2 to 0.8 m/s
    let turb_range_factor = (range / 100.0).sqrt().min(5.0); // sqrt scaling
    let turb_sigma = turb_base * turb_range_factor;

    // Turbulence affects both axes equally (isotropic assumption).
    let turb_sigma_x = turb_sigma;
    let turb_sigma_y = turb_sigma;

    // ── Combined uncertainty (quadrature) ──────────────────────────────
    let wind_unc_x = (inst_sigma_x.powi(2) + turb_sigma_x.powi(2)).sqrt() * alt_factor;
    let wind_unc_y = (inst_sigma_y.powi(2) + turb_sigma_y.powi(2)).sqrt() * alt_factor;

    // ── Drift uncertainty ─────────────────────────────────────────────
    // Crosswind deflection = wind_error × time_of_flight (first-order
    // approximation; a full trajectory integration would use the
    // bullet's time-varying cross-range acceleration).
    // The drift uncertainty is dominated by the cross-range (y) component.
    let drift_uncertainty_m = wind_unc_y * tof;

    // ── 95 % probability band ──────────────────────────────────────────
    // For a Gaussian distribution, 95 % of impacts lie within 1.96 σ
    // on each side → total spread = 2 × 1.96 × σ ≈ 3.92 σ.
    let probability_band_width_m = 3.92 * drift_uncertainty_m;

    WindUncertaintyResult {
        effective_wind_x_ms: effective_x,
        effective_wind_y_ms: effective_y,
        wind_uncertainty_x_ms: wind_unc_x,
        wind_uncertainty_y_ms: wind_unc_y,
        drift_uncertainty_m,
        probability_band_width_m,
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Zero instrument error but still has turbulence ─────────────────────

    #[test]
    fn zero_instrument_error_still_has_turbulence() {
        let params = WindUncertaintyParams {
            measured_wind_x_ms: 5.0,
            measured_wind_y_ms: 5.0,
            instrument_error_pct: 0.0,
            range_m: 500.0,
            time_of_flight_s: 0.8,
            turbulence_intensity: 0.5,
            altitude_m: 0.0,
        };
        let result = evaluate_wind_uncertainty(&params);
        // Even with 0% instrument error, turbulence gives some uncertainty
        assert!(
            result.wind_uncertainty_y_ms > 0.0,
            "There should be some wind uncertainty from turbulence"
        );
        assert!(
            result.drift_uncertainty_m > 0.0,
            "Drift uncertainty should be positive"
        );
        assert!(
            result.probability_band_width_m > 0.0,
            "Probability band should be positive"
        );
    }

    // ── High turbulence increases uncertainty ─────────────────────────────

    #[test]
    fn high_turbulence_increases_uncertainty() {
        let calm = evaluate_wind_uncertainty(&WindUncertaintyParams {
            measured_wind_x_ms: 4.0,
            measured_wind_y_ms: 3.0,
            instrument_error_pct: 5.0,
            range_m: 500.0,
            time_of_flight_s: 0.8,
            turbulence_intensity: 0.0,
            altitude_m: 0.0,
        });
        let storm = evaluate_wind_uncertainty(&WindUncertaintyParams {
            measured_wind_x_ms: 4.0,
            measured_wind_y_ms: 3.0,
            instrument_error_pct: 5.0,
            range_m: 500.0,
            time_of_flight_s: 0.8,
            turbulence_intensity: 1.0,
            altitude_m: 0.0,
        });
        assert!(
            storm.wind_uncertainty_y_ms > calm.wind_uncertainty_y_ms,
            "High turbulence should increase wind uncertainty: {:.4} vs {:.4}",
            storm.wind_uncertainty_y_ms,
            calm.wind_uncertainty_y_ms
        );
        assert!(
            storm.drift_uncertainty_m > calm.drift_uncertainty_m,
            "Drift uncertainty should increase with turbulence"
        );
    }

    // ── High altitude reduces uncertainty ─────────────────────────────────

    #[test]
    fn high_altitude_reduces_effective_uncertainty() {
        let sea_level = evaluate_wind_uncertainty(&WindUncertaintyParams {
            measured_wind_x_ms: 5.0,
            measured_wind_y_ms: 5.0,
            instrument_error_pct: 10.0,
            range_m: 500.0,
            time_of_flight_s: 0.8,
            turbulence_intensity: 0.3,
            altitude_m: 0.0,
        });
        let high = evaluate_wind_uncertainty(&WindUncertaintyParams {
            measured_wind_x_ms: 5.0,
            measured_wind_y_ms: 5.0,
            instrument_error_pct: 10.0,
            range_m: 500.0,
            time_of_flight_s: 0.8,
            turbulence_intensity: 0.3,
            altitude_m: 3000.0,
        });
        assert!(
            high.wind_uncertainty_y_ms < sea_level.wind_uncertainty_y_ms,
            "High altitude should reduce wind uncertainty: {:.4} vs {:.4}",
            high.wind_uncertainty_y_ms,
            sea_level.wind_uncertainty_y_ms
        );
    }

    // ── Long range increases uncertainty ──────────────────────────────────

    #[test]
    fn longer_range_increases_drift_uncertainty() {
        let close = evaluate_wind_uncertainty(&WindUncertaintyParams {
            measured_wind_x_ms: 3.0,
            measured_wind_y_ms: 4.0,
            instrument_error_pct: 10.0,
            range_m: 100.0,
            time_of_flight_s: 0.15,
            turbulence_intensity: 0.3,
            altitude_m: 0.0,
        });
        let far = evaluate_wind_uncertainty(&WindUncertaintyParams {
            measured_wind_x_ms: 3.0,
            measured_wind_y_ms: 4.0,
            instrument_error_pct: 10.0,
            range_m: 800.0,
            time_of_flight_s: 1.5,
            turbulence_intensity: 0.3,
            altitude_m: 0.0,
        });
        assert!(
            far.drift_uncertainty_m > close.drift_uncertainty_m,
            "Longer range should increase drift uncertainty: {:.4} vs {:.4}",
            far.drift_uncertainty_m,
            close.drift_uncertainty_m
        );
        assert!(
            far.probability_band_width_m > close.probability_band_width_m,
            "Probability band should widen with range"
        );
    }

    // ── Sanity bounds ─────────────────────────────────────────────────────

    #[test]
    fn no_wind_has_minimal_uncertainty() {
        let result = evaluate_wind_uncertainty(&WindUncertaintyParams {
            measured_wind_x_ms: 0.0,
            measured_wind_y_ms: 0.0,
            instrument_error_pct: 10.0,
            range_m: 500.0,
            time_of_flight_s: 0.8,
            turbulence_intensity: 0.5,
            altitude_m: 0.0,
        });
        // With zero measured wind, effective wind should be (near) zero
        assert!(result.effective_wind_x_ms.abs() < 0.01);
        assert!(result.effective_wind_y_ms.abs() < 0.01);
        // Uncertainty should be from turbulence floor only (small, non-zero)
        assert!(result.wind_uncertainty_y_ms > 0.0);
        assert!(result.wind_uncertainty_y_ms < 5.0); // should be modest
    }

    #[test]
    fn probability_band_scales_with_drift_uncertainty() {
        let result = evaluate_wind_uncertainty(&WindUncertaintyParams {
            measured_wind_x_ms: 4.0,
            measured_wind_y_ms: 6.0,
            instrument_error_pct: 10.0,
            range_m: 500.0,
            time_of_flight_s: 0.8,
            turbulence_intensity: 0.4,
            altitude_m: 0.0,
        });
        // 95% band = 3.92 * drift_uncertainty
        let expected_band = 3.92 * result.drift_uncertainty_m;
        assert!(
            (result.probability_band_width_m - expected_band).abs() < 1e-9,
            "Probability band should be 3.92 × drift uncertainty: {:.4} vs {:.4}",
            result.probability_band_width_m,
            expected_band
        );
    }

    // ── Deterministic ─────────────────────────────────────────────────────

    #[test]
    fn deterministic_output() {
        let params = WindUncertaintyParams {
            measured_wind_x_ms: 2.5,
            measured_wind_y_ms: 4.0,
            instrument_error_pct: 8.0,
            range_m: 600.0,
            time_of_flight_s: 1.0,
            turbulence_intensity: 0.3,
            altitude_m: 500.0,
        };
        let a = evaluate_wind_uncertainty(&params);
        let b = evaluate_wind_uncertainty(&params);
        assert!((a.effective_wind_x_ms - b.effective_wind_x_ms).abs() < 1e-12);
        assert!((a.wind_uncertainty_y_ms - b.wind_uncertainty_y_ms).abs() < 1e-12);
        assert!((a.drift_uncertainty_m - b.drift_uncertainty_m).abs() < 1e-12);
        assert!((a.probability_band_width_m - b.probability_band_width_m).abs() < 1e-12);
    }

    // ── Long range + high turbulence produces wide band ────────────────────

    #[test]
    fn long_range_high_turbulence_wide_band() {
        let result = evaluate_wind_uncertainty(&WindUncertaintyParams {
            measured_wind_x_ms: 2.0,
            measured_wind_y_ms: 8.0,
            instrument_error_pct: 15.0,
            range_m: 1000.0,
            time_of_flight_s: 2.0,
            turbulence_intensity: 0.8,
            altitude_m: 0.0,
        });
        // Should produce a noticeably wide probability band
        assert!(
            result.probability_band_width_m > 0.5,
            "Worst-case wind should produce wide band, got {:.2} m",
            result.probability_band_width_m
        );
        // Effective wind should be bias-corrected (lower than measured)
        assert!(
            result.effective_wind_y_ms < 8.0,
            "Effective wind should be bias-corrected lower than measured"
        );
    }
}
