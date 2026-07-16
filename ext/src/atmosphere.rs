// ABE - Atmosphere & Environment Model
//
// Implements ICAO/ISA standard atmosphere with altitude-based
// temperature, pressure, and density gradients.
//
// References:
//   - ICAO Standard Atmosphere (Doc 7488)
//   - ISO 2533:1975
//   - MIL-STD-210C

pub const GRAVITY: f64 = 9.80665; // m/s²
pub const R_SPECIFIC: f64 = 287.058; // J/(kg·K) - specific gas constant for air
pub const LAPSE_RATE: f64 = -0.0065; // K/m - temperature lapse rate in troposphere
pub const SEA_LEVEL_TEMP: f64 = 288.15; // K (15°C)
pub const SEA_LEVEL_PRESSURE: f64 = 101325.0; // Pa
pub const SEA_LEVEL_DENSITY: f64 = 1.225; // kg/m³
pub const TROPOPAUSE_ALT: f64 = 11000.0; // m

/// Temperature at altitude (K) in ISA standard atmosphere.
/// Troposphere (0-11km): T = T₀ + L * h
/// Minimum: 216.65 K (-56.5°C)
pub fn temperature_at_altitude(altitude_m: f64) -> f64 {
    if altitude_m <= TROPOPAUSE_ALT {
        (SEA_LEVEL_TEMP + LAPSE_RATE * altitude_m).max(216.65)
    } else {
        216.65 // Isothermal at tropopause
    }
}

/// Pressure at altitude (Pa) in ISA standard atmosphere.
/// Troposphere: P = P₀ * (T/T₀)^(-g/(R*L))
/// Stratosphere: P = P₁₁ * exp(-g*(h-h₁₁)/(R*T₁₁))
pub fn pressure_at_altitude(altitude_m: f64) -> f64 {
    let t = temperature_at_altitude(altitude_m);

    if altitude_m <= TROPOPAUSE_ALT {
        let exponent = -GRAVITY / (R_SPECIFIC * LAPSE_RATE);
        SEA_LEVEL_PRESSURE * (t / SEA_LEVEL_TEMP).powf(exponent)
    } else {
        let p_trop = pressure_at_altitude(TROPOPAUSE_ALT);
        let delta_h = altitude_m - TROPOPAUSE_ALT;
        p_trop * (-GRAVITY * delta_h / (R_SPECIFIC * t)).exp()
    }
}

/// Air density at altitude (kg/m³) using ideal gas law.
pub fn density_at_altitude(altitude_m: f64) -> f64 {
    let p = pressure_at_altitude(altitude_m);
    let t = temperature_at_altitude(altitude_m);
    p / (R_SPECIFIC * t)
}

/// Surface roughness length for wind profile (m).
/// Typical values: 0.03 (open grassland), 0.25 (hedgerows), 1.0 (suburban).
pub const SURFACE_ROUGHNESS: f64 = 0.03;

/// Reference height for input wind measurement (m).
/// ARMA 3's wind is typically at ~2m above ground.
pub const WIND_REF_HEIGHT: f64 = 2.0;

/// Wind shear factor at a given altitude.
///
/// Uses the log-wind-profile law (von Kármán-Prandtl):
///   U(z) = U_ref * ln(z / z₀) / ln(z_ref / z₀)
///
/// where:
///   z  = altitude above ground (m)
///   z₀ = surface roughness length (m)
///   z_ref = reference height where U_ref is measured (2m default)
///
/// Returns the multiplier to apply to reference wind.
/// Formula is valid in the surface layer (0-200m). Above 200m, the multiplier
/// approaches a realistic upper bound (~2-3x surface wind).
pub fn wind_shear_factor(altitude_m: f64) -> f64 {
    if altitude_m <= WIND_REF_HEIGHT {
        return 1.0; // At or below reference height, no adjustment
    }

    let z = altitude_m.min(200.0); // Cap at surface layer limit
    let ln_z = z.ln();
    let ln_z0 = SURFACE_ROUGHNESS.ln();
    let ln_ref = WIND_REF_HEIGHT.ln();

    let factor = (ln_z - ln_z0) / (ln_ref - ln_z0);
    factor.max(1.0).min(3.0) // Sanity bounds
}

/// Convenience: density from altitude with temperature override
pub fn density_from_altitude(altitude_m: f64, temp_c: f64) -> f64 {
    // Use given temperature if non-zero, otherwise ISA
    if temp_c.abs() < 0.1 {
        density_at_altitude(altitude_m)
    } else {
        let temp_k = temp_c + 273.15;
        let p = pressure_at_altitude(altitude_m);
        p / (R_SPECIFIC * temp_k)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sea_level_values() {
        let t = temperature_at_altitude(0.0);
        let p = pressure_at_altitude(0.0);
        let d = density_at_altitude(0.0);

        assert!((t - SEA_LEVEL_TEMP).abs() < 0.1);
        assert!((p - SEA_LEVEL_PRESSURE).abs() < 10.0);
        assert!((d - SEA_LEVEL_DENSITY).abs() < 0.01);
    }

    #[test]
    fn density_decreases_with_altitude() {
        let d_0 = density_at_altitude(0.0);
        let d_1000 = density_at_altitude(1000.0);
        let d_5000 = density_at_altitude(5000.0);

        assert!(d_1000 < d_0);
        assert!(d_5000 < d_1000);
    }

    #[test]
    fn pressure_at_1000m() {
        let p = pressure_at_altitude(1000.0);
        // ~89,874 Pa at 1000m ISA
        assert!((p - 89874.0).abs() < 200.0);
    }

    #[test]
    fn tropopause_isothermal() {
        let t_11k = temperature_at_altitude(11000.0);
        let t_15k = temperature_at_altitude(15000.0);
        // Both should be ~216.65 K in the simple model
        assert!((t_11k - 216.65).abs() < 1.0);
        assert!((t_15k - 216.65).abs() < 1.0);
    }

    #[test]
    fn temperature_cold_at_high_alt() {
        let t_20k = temperature_at_altitude(20000.0);
        assert!(t_20k < 230.0); // Well below freezing
    }

    #[test]
    fn wind_shear_at_reference_height() {
        let f = wind_shear_factor(2.0);
        assert!(
            (f - 1.0).abs() < 0.01,
            "At reference height, factor should be 1.0: {}",
            f
        );
    }

    #[test]
    fn wind_increases_with_altitude() {
        let f_10 = wind_shear_factor(10.0);
        let f_50 = wind_shear_factor(50.0);
        assert!(f_10 > 1.0, "Wind at 10m should be > reference");
        assert!(f_50 > f_10, "Wind at 50m should be stronger than at 10m");
    }

    #[test]
    fn wind_shear_bounded() {
        let f_0 = wind_shear_factor(0.0);
        assert!((f_0 - 1.0).abs() < 0.01, "At 0m, factor = 1.0: {}", f_0);
        let f_300 = wind_shear_factor(300.0);
        assert!(f_300 <= 3.0, "Wind factor capped at 3.0: {}", f_300);
    }

    #[test]
    fn density_from_temp_override() {
        // At sea level, warmer air → lower density
        let d_standard = density_from_altitude(0.0, 15.0);
        let d_hot = density_from_altitude(0.0, 40.0);
        assert!(d_hot < d_standard);
    }
}
