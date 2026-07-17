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

// ── Additional gas constants ──────────────────────────────────────────────────

/// Specific gas constant for dry air (J/(kg·K)). Alias of R_SPECIFIC.
pub const GAS_CONSTANT_DRY_AIR: f64 = 287.058;

/// Specific gas constant for water vapor (J/(kg·K)).
pub const GAS_CONSTANT_WET_AIR: f64 = 461.495;

// ── Power-law wind profile exponents ──────────────────────────────────────────

/// Power-law exponent for open terrain (grassland, farmland).
pub const POWER_LAW_EXPONENT_OPEN: f64 = 0.143;

/// Power-law exponent for urban / built-up terrain.
pub const POWER_LAW_EXPONENT_URBAN: f64 = 0.333;

/// Power-law exponent for open water / sea.
pub const POWER_LAW_EXPONENT_SEA: f64 = 0.100;

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

// ── Non-ISA Atmosphere ─────────────────────────────────────────────────────────

/// Non-ISA atmosphere parameters representing deviations from the standard model.
pub struct NonIsaAtmosphere {
    /// Temperature offset from ISA at any altitude (°C). Positive = warmer.
    pub delta_temp_c: f64,
    /// Pressure offset as a percentage of ISA pressure (%). Positive = higher pressure.
    pub delta_pressure_pct: f64,
    /// Relative humidity (0.0–100.0 %). Affects density (moist air is lighter).
    pub humidity_pct: f64,
}

/// Turbulence parameters for gust and eddy modeling.
pub struct TurbulenceParams {
    /// Turbulence intensity (0.0 = calm, 1.0 = severe).
    pub intensity: f64,
    /// Eddy scale length (m). Default ~100 m for open terrain.
    pub scale_length_m: f64,
    /// Gust amplitude (m/s) at reference conditions.
    pub gust_amplitude_ms: f64,
}

/// Wind profile defined by a power-law model.
///
/// U(z) = U_ref * (z / z_ref)^α
pub struct WindProfile {
    /// Wind speed at reference height (m/s).
    pub surface_wind_ms: f64,
    /// Wind direction in degrees from north (0 = from north, 90 = from east).
    pub wind_direction_deg: f64,
    /// Power-law exponent α (dimensionless). Default 0.143 for open terrain.
    pub profile_exponent: f64,
    /// Reference height (m) at which surface_wind_ms is measured. Default 10 m.
    pub reference_height_m: f64,
}

/// Combined weather state at a given altitude.
pub struct WeatherState {
    /// Altitude (m).
    pub altitude_m: f64,
    /// Temperature (°C).
    pub temperature_c: f64,
    /// Pressure (Pa).
    pub pressure_pa: f64,
    /// Density (kg/m³).
    pub density_kgm3: f64,
    /// Wind profile at this location.
    pub wind_profile: WindProfile,
    /// Turbulence parameters.
    pub turbulence: TurbulenceParams,
    /// Non-ISA atmosphere offsets.
    pub non_isa: NonIsaAtmosphere,
}

// ── Non-ISA Functions ─────────────────────────────────────────────────────────

const MAGNUS_A: f64 = 17.625;
const MAGNUS_B: f64 = 243.04;

/// Saturation vapour pressure (Pa) via the improved Magnus formula.
///
/// Reference: Alduchov & Eskridge (1996), J. Appl. Meteor.
fn saturation_vapor_pressure(temp_c: f64) -> f64 {
    610.94 * ((MAGNUS_A * temp_c) / (temp_c + MAGNUS_B)).exp()
}

/// Temperature at altitude with non-ISA offset (K).
///
/// T(h) = T_ISA(h) + ΔT
pub fn temperature_non_isa(altitude_m: f64, non_isa: &NonIsaAtmosphere) -> f64 {
    temperature_at_altitude(altitude_m) + non_isa.delta_temp_c
}

/// Pressure at altitude with non-ISA percentage offset (Pa).
///
/// P(h) = P_ISA(h) × (1 + ΔP%)
pub fn pressure_non_isa(altitude_m: f64, non_isa: &NonIsaAtmosphere) -> f64 {
    pressure_at_altitude(altitude_m) * (1.0 + non_isa.delta_pressure_pct / 100.0)
}

/// Density at altitude adjusted for temperature offset and humidity (kg/m³).
///
/// Uses the virtual-temperature correction for water vapour:
///   ρ = P / (R_dry × T_v)
/// where T_v = T / (1 - 0.379 × e / P)  and  e = RH × e_sat(T)
pub fn density_non_isa(altitude_m: f64, non_isa: &NonIsaAtmosphere) -> f64 {
    let t_k = temperature_non_isa(altitude_m, non_isa);
    let p = pressure_non_isa(altitude_m, non_isa);
    let t_c = t_k - 273.15;

    // Saturation vapour pressure → actual vapour pressure from RH
    let e_sat = saturation_vapor_pressure(t_c);
    let e = (non_isa.humidity_pct / 100.0) * e_sat;

    // Virtual temperature correction: moist air is lighter
    let t_v = t_k / (1.0 - 0.379 * e / p.max(1.0));

    p / (GAS_CONSTANT_DRY_AIR * t_v)
}

// ── Wind & Turbulence ─────────────────────────────────────────────────────────

/// Wind speed at altitude using the power-law profile (m/s).
///
/// U(z) = U_ref × (z / z_ref)^α
///
/// Returns 0 when altitude or reference_height_m is ≤ 0.
pub fn wind_velocity(wind_profile: &WindProfile, altitude_m: f64) -> f64 {
    let z_ref = wind_profile.reference_height_m.max(0.01);
    let z = altitude_m.max(0.0);
    if z <= 0.0 || z_ref <= 0.0 {
        return 0.0;
    }
    wind_profile.surface_wind_ms * (z / z_ref).powf(wind_profile.profile_exponent)
}

/// Deterministic gust velocity (m/s) from turbulence parameters.
///
/// Uses a seeded calculation based on intensity and scale length — no randomness.
///   U_gust = A × I × sqrt(L / L₀)
/// where A = gust_amplitude_ms, I = intensity, L = scale_length_m, L₀ = 100 m.
pub fn gust_velocity(turbulence: &TurbulenceParams) -> f64 {
    let scale_norm = (turbulence.scale_length_m / 100.0).sqrt();
    turbulence.gust_amplitude_ms * turbulence.intensity * scale_norm
}

/// Wind ratio between two altitudes using power-law shear.
///
/// Returns U(target) / U(source) = (z₂ / z₁)^α.
/// Both altitudes are clamped to ≥ 0.01 m to avoid division issues.
pub fn wind_shear_power_law(
    wind_profile: &WindProfile,
    altitude_m: f64,
    target_altitude_m: f64,
) -> f64 {
    let z1 = altitude_m.max(0.01);
    let z2 = target_altitude_m.max(0.01);
    (z2 / z1).powf(wind_profile.profile_exponent)
}

/// Convenience: combined density from a full WeatherState (kg/m³).
///
/// Uses the non-ISA functions internally, returning a density that accounts for
/// temperature offset, pressure offset, and humidity.
pub fn density_from_weather(weather: &WeatherState) -> f64 {
    density_non_isa(weather.altitude_m, &weather.non_isa)
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

    // ── Non-ISA / Dynamic Weather Tests ────────────────────────────────────────

    #[test]
    fn non_isa_temperature_offset_affects_density() {
        let isa = NonIsaAtmosphere {
            delta_temp_c: 0.0,
            delta_pressure_pct: 0.0,
            humidity_pct: 0.0,
        };
        let warm = NonIsaAtmosphere {
            delta_temp_c: 15.0,
            delta_pressure_pct: 0.0,
            humidity_pct: 0.0,
        };

        let d_isa = density_non_isa(0.0, &isa);
        let d_warm = density_non_isa(0.0, &warm);
        assert!(
            d_warm < d_isa,
            "Warmer air should be less dense: ISA={}, warm={}",
            d_isa,
            d_warm
        );
    }

    #[test]
    fn humidity_reduces_density() {
        let dry = NonIsaAtmosphere {
            delta_temp_c: 0.0,
            delta_pressure_pct: 0.0,
            humidity_pct: 0.0,
        };
        let humid = NonIsaAtmosphere {
            delta_temp_c: 0.0,
            delta_pressure_pct: 0.0,
            humidity_pct: 80.0,
        };

        let d_dry = density_non_isa(0.0, &dry);
        let d_humid = density_non_isa(0.0, &humid);
        assert!(
            d_humid < d_dry,
            "Moist air should be less dense: dry={}, humid={}",
            d_dry,
            d_humid
        );
    }

    #[test]
    fn power_law_wind_increases_with_altitude() {
        let profile = WindProfile {
            surface_wind_ms: 5.0,
            wind_direction_deg: 270.0,
            profile_exponent: POWER_LAW_EXPONENT_OPEN,
            reference_height_m: 10.0,
        };

        let w_10 = wind_velocity(&profile, 10.0);
        let w_50 = wind_velocity(&profile, 50.0);
        let w_100 = wind_velocity(&profile, 100.0);

        assert!((w_10 - 5.0).abs() < 0.001, "At ref height, wind = U_ref");
        assert!(
            w_50 > w_10,
            "Wind at 50m > wind at 10m: {} > {}",
            w_50,
            w_10
        );
        assert!(
            w_100 > w_50,
            "Wind at 100m > wind at 50m: {} > {}",
            w_100,
            w_50
        );
    }

    #[test]
    fn gust_amplitude_scales_with_intensity() {
        let calm = TurbulenceParams {
            intensity: 0.0,
            scale_length_m: 100.0,
            gust_amplitude_ms: 10.0,
        };
        let moderate = TurbulenceParams {
            intensity: 0.5,
            scale_length_m: 100.0,
            gust_amplitude_ms: 10.0,
        };
        let severe = TurbulenceParams {
            intensity: 1.0,
            scale_length_m: 100.0,
            gust_amplitude_ms: 10.0,
        };

        assert!((gust_velocity(&calm) - 0.0).abs() < 1e-12, "Calm = 0 gust");
        assert!(
            gust_velocity(&moderate) > 0.0,
            "Moderate > 0: {}",
            gust_velocity(&moderate)
        );
        assert!(
            gust_velocity(&severe) > gust_velocity(&moderate),
            "Severe > moderate: {} > {}",
            gust_velocity(&severe),
            gust_velocity(&moderate)
        );
    }

    #[test]
    fn non_isa_pressure_offset_shifts_pressure() {
        let no_offset = NonIsaAtmosphere {
            delta_temp_c: 0.0,
            delta_pressure_pct: 0.0,
            humidity_pct: 0.0,
        };
        let high = NonIsaAtmosphere {
            delta_temp_c: 0.0,
            delta_pressure_pct: 5.0,
            humidity_pct: 0.0,
        };

        let p_isa = pressure_non_isa(0.0, &no_offset);
        let p_high = pressure_non_isa(0.0, &high);
        assert!(
            (p_isa - SEA_LEVEL_PRESSURE).abs() < 1.0,
            "Zero offset = ISA: {}",
            p_isa
        );
        assert!(
            p_high > p_isa,
            "Positive offset increases pressure: {} > {}",
            p_high,
            p_isa
        );
        // 5% increase at sea level
        assert!((p_high - SEA_LEVEL_PRESSURE * 1.05).abs() < 1.0);
    }

    #[test]
    fn sea_level_standard_density_with_isa_defaults() {
        let isa = NonIsaAtmosphere {
            delta_temp_c: 0.0,
            delta_pressure_pct: 0.0,
            humidity_pct: 0.0,
        };
        let d = density_non_isa(0.0, &isa);
        assert!(
            (d - SEA_LEVEL_DENSITY).abs() < 0.01,
            "ISA defaults give sea-level density: {}",
            d
        );
    }

    #[test]
    fn deterministic_output() {
        let non_isa = NonIsaAtmosphere {
            delta_temp_c: 10.0,
            delta_pressure_pct: -2.0,
            humidity_pct: 50.0,
        };
        let profile = WindProfile {
            surface_wind_ms: 7.5,
            wind_direction_deg: 180.0,
            profile_exponent: POWER_LAW_EXPONENT_OPEN,
            reference_height_m: 10.0,
        };
        let turb = TurbulenceParams {
            intensity: 0.6,
            scale_length_m: 120.0,
            gust_amplitude_ms: 8.0,
        };

        // Call each new function twice — results must match exactly
        let d1 = density_non_isa(500.0, &non_isa);
        let d2 = density_non_isa(500.0, &non_isa);
        assert!((d1 - d2).abs() < 1e-15, "density_non_isa deterministic");

        let p1 = pressure_non_isa(500.0, &non_isa);
        let p2 = pressure_non_isa(500.0, &non_isa);
        assert!((p1 - p2).abs() < 1e-15, "pressure_non_isa deterministic");

        let t1 = temperature_non_isa(500.0, &non_isa);
        let t2 = temperature_non_isa(500.0, &non_isa);
        assert!((t1 - t2).abs() < 1e-15, "temperature_non_isa deterministic");

        let w1 = wind_velocity(&profile, 100.0);
        let w2 = wind_velocity(&profile, 100.0);
        assert!((w1 - w2).abs() < 1e-15, "wind_velocity deterministic");

        let g1 = gust_velocity(&turb);
        let g2 = gust_velocity(&turb);
        assert!((g1 - g2).abs() < 1e-15, "gust_velocity deterministic");

        let s1 = wind_shear_power_law(&profile, 10.0, 100.0);
        let s2 = wind_shear_power_law(&profile, 10.0, 100.0);
        assert!(
            (s1 - s2).abs() < 1e-15,
            "wind_shear_power_law deterministic"
        );
    }
}
