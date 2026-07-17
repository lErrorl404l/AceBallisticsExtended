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

// ── Humidity Correction Functions ──────────────────────────────────────────────

/// Compute air density corrected for water vapour (kg/m³).
///
/// Moist air is lighter than dry air at the same temperature and pressure
/// because water molecules (M ≈ 18 g/mol) displace heavier nitrogen and
/// oxygen molecules (M ≈ 29 g/mol).
///
/// Uses the virtual-temperature correction:
///   ρ = P / (R_dry · T_v)
///   T_v = T / (1 - 0.379 · e / P)
///   e  = (RH / 100) · e_sat(T)
///
/// where e_sat(T) is the saturation vapour pressure from the Magnus
/// formula (Alduchov & Eskridge 1996).
///
/// # Arguments
/// * `altitude_m` — Altitude above sea level (m).  Pressure is computed
///   from the ISA standard atmosphere at this altitude.
/// * `temp_c` — Ambient temperature (°C).
/// * `humidity_pct` — Relative humidity (0–100 %).
///
/// # Returns
/// Air density in kg/m³.
pub fn density_with_humidity(altitude_m: f64, temp_c: f64, humidity_pct: f64) -> f64 {
    let temp_k = temp_c + 273.15;
    let pressure_pa = pressure_at_altitude(altitude_m);

    // Saturation vapour pressure → actual vapour pressure
    let e_sat = saturation_vapor_pressure(temp_c);
    let e = (humidity_pct / 100.0) * e_sat;

    // Virtual temperature: moist air is lighter
    let t_v = temp_k / (1.0 - 0.379 * e / pressure_pa.max(1.0));

    pressure_pa / (GAS_CONSTANT_DRY_AIR * t_v)
}

/// Compute Mach number with humidity correction for speed of sound.
///
/// Water vapour increases the speed of sound because it reduces the
/// average molecular weight of the air mixture, increasing the adiabatic
/// speed of sound for a given temperature:
///   c_moist = c_dry · sqrt(1 + 0.51 · q)
///
/// where q is the specific humidity (kg water vapour per kg moist air):
///   q ≈ 0.622 · e / P
///
/// The dry speed of sound uses the standard formula:
///   c_dry = 331.3 · sqrt(1 + T/273.15)
///
/// # Arguments
/// * `velocity_ms` — Projectile velocity (m/s).
/// * `temp_c` — Ambient temperature (°C).
/// * `humidity_pct` — Relative humidity (0–100 %).
///
/// # Returns
/// Mach number (dimensionless).
pub fn mach_with_humidity(velocity_ms: f64, temp_c: f64, humidity_pct: f64) -> f64 {
    if velocity_ms <= 0.0 {
        return 0.0;
    }

    let pressure_pa = pressure_at_altitude(0.0); // sea-level reference

    // Specific humidity
    let e_sat = saturation_vapor_pressure(temp_c);
    let e = (humidity_pct / 100.0) * e_sat;
    let q = 0.622 * e / pressure_pa.max(1.0);

    // Speed of sound in dry air
    let c_dry = 331.3 * (1.0 + temp_c / 273.15).sqrt();

    // Moisture correction
    let c_moist = c_dry * (1.0 + 0.51 * q).sqrt();

    velocity_ms / c_moist
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

// ── Precipitation Effects ─────────────────────────────────────────────────────

/// Broad caliber classification for rain drag coupling.
pub enum CaliberClass {
    /// Rifle and intermediate cartridges (typical BC > 0.2).
    /// Coupling coefficient k ≈ 0.15.
    Rifle,
    /// Pistol and subsonic cartridges (typical BC < 0.2).
    /// Coupling coefficient k ≈ 0.25.
    Pistol,
}

/// Precipitation parameters affecting ballistic trajectories.
///
/// Models the influence of rain and snow on projectile drag and
/// lateral drift.  The physics involves:
///   - Momentum transfer from impacting hydrometeors
///   - Boundary-layer disruption on the projectile surface
///   - Slight density change from airborne water (negligible below
///     cloud base — the virtual-temperature correction already
///     handles water vapour)
pub struct PrecipitationParams {
    /// Rainfall intensity (mm/hour).  Range: 0–150 mm/hr.
    ///
    /// Empirical intensity classes:
    ///   - 0.0:         No rain
    ///   - 0.1–0.5:    Drizzle
    ///   - 0.5–2.5:    Light rain
    ///   - 2.5–10:     Moderate rain
    ///   - 10–50:      Heavy rain
    ///   - 50–150+:    Extreme / torrential
    pub rain_rate_mm_per_hour: f64,
    /// `true` when the precipitation is snowfall (affects terminal
    /// velocity and drag differently from rain).
    pub is_snowfall: bool,
    /// Altitude of the cloud base above sea level (m).  Below this
    /// altitude the air is sub-saturated; above, saturated.
    pub cloud_base_altitude_m: f64,
    /// Ambient temperature (°C).  Used together with `is_snowfall`
    /// to distinguish rain vs freezing rain vs snow.
    pub temperature_c: f64,
}

/// Drag-coefficient adjustment caused by precipitation.
///
/// Rain and snow increase aerodynamic drag by roughening the
/// projectile surface and altering the boundary layer.  The ratio
/// is relative to dry-air conditions (1.0 = no effect).
pub struct PrecipitationDragCoefficient {
    /// Drag multiplier relative to dry conditions.
    /// Typical: 1.0 (none) … 1.15 (moderate rain) … 1.3 (heavy rain).
    pub cd_ratio: f64,
    /// Human-readable validity note (e.g. "moderate rain, rifle").
    pub validity_note: &'static str,
}

/// Estimate the terminal velocity of falling hydrometeors (m/s).
///
/// Rain drops: larger drops (higher rain rates) fall faster.
/// Empirical logarithmic fit:
///
///   Vt = 4.0 + 2.5 × ln(1 + R)
///
/// where R = rain rate (mm/hr).  Snowflakes are modelled at a
/// constant 1.5 m/s.
///
/// # Arguments
/// * `rain_params` — Current precipitation parameters.
///
/// # Returns
/// Terminal velocity in m/s, capped at 9.5 m/s for rain.
fn rain_drop_terminal_velocity(rain_params: &PrecipitationParams) -> f64 {
    if rain_params.is_snowfall {
        return 1.5;
    }
    let r = rain_params.rain_rate_mm_per_hour.max(0.0);
    let vt = 4.0 + 2.5 * (1.0 + r).ln();
    vt.min(9.5)
}

/// Effective ballistic coefficient reduced by rain or snowfall.
///
/// Hydrometeor impacts increase drag by disturbing the boundary
/// layer and transferring momentum.  The empirical exponential
/// model is:
///
///   BC_eff = BC × exp(-k × R / 1000)
///
/// where:
///   R = rain rate (mm/hour)
///   k = caliber-dependent coupling coefficient
///       - Rifle:  k ≈ 0.15 (larger BC, less relative effect)
///       - Pistol: k ≈ 0.25 (smaller BC, more relative effect)
///
/// The divisor 1000 normalises rain rate into a dimensionless
/// intensity scale.
///
/// # Arguments
/// * `rain_params` — Current precipitation parameters.
/// * `base_bc` — Ballistic coefficient under dry conditions (G1 or G7).
/// * `caliber` — Classification into Rifle or Pistol class.
///
/// # Returns
/// Effective ballistic coefficient accounting for precipitation drag.
pub fn precipitation_unadjusted_bc(
    rain_params: &PrecipitationParams,
    base_bc: f64,
    caliber: CaliberClass,
) -> f64 {
    let k = match caliber {
        CaliberClass::Rifle => 0.15,
        CaliberClass::Pistol => 0.25,
    };
    let intensity = rain_params.rain_rate_mm_per_hour.max(0.0);
    base_bc * (-k * intensity / 1000.0).exp()
}

/// Additional lateral drift velocity imparted by rain droplets (m/s).
///
/// Rain droplets carry horizontal momentum from the ambient crosswind.
/// When a projectile passes through falling rain, glancing impacts
/// transfer a small fraction of that momentum laterally:
///
///   v_drift = v_wind × (Vt_drop / V_bullet) × η
///
/// where:
///   v_wind    = crosswind speed (m/s)
///   Vt_drop   = hydrometeor terminal velocity (m/s)
///   V_bullet  = projectile velocity at the range point (m/s)
///   η         = empirical coupling efficiency (≈ 0.01 = 1 %)
///
/// The effect is tiny (typically < 0.01 m/s) and only matters at
/// very long ranges or extreme rain rates.
///
/// # Arguments
/// * `rain_params` — Current precipitation parameters.
/// * `crosswind_speed_ms` — Crosswind component perpendicular to
///   the line of fire (m/s).
/// * `bullet_velocity_ms` — Projectile velocity at this range
///   point (m/s).
///
/// # Returns
/// Additional drift velocity (m/s) from precipitation momentum
/// transfer.  Returns zero when rain rate is zero or bullet
/// velocity is non-positive.
pub fn rain_drift_velocity(
    rain_params: &PrecipitationParams,
    crosswind_speed_ms: f64,
    bullet_velocity_ms: f64,
) -> f64 {
    if rain_params.rain_rate_mm_per_hour <= 0.0 || bullet_velocity_ms <= 0.0 {
        return 0.0;
    }
    let vt = rain_drop_terminal_velocity(rain_params);
    crosswind_speed_ms * (vt / bullet_velocity_ms) * 0.01
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

    // ── Humidity Correction Tests ─────────────────────────────────────────

    #[test]
    fn humidity_reduces_density_direct() {
        // At sea level, 20 °C: dry vs humid
        let d_dry = density_with_humidity(0.0, 20.0, 0.0);
        let d_humid = density_with_humidity(0.0, 20.0, 80.0);
        assert!(
            d_humid < d_dry,
            "humid air should be less dense: dry={:.6}, humid={:.6}",
            d_dry,
            d_humid
        );
        // Effect should be small but measurable: at 20 °C / 80 % RH,
        // density reduction is ~1–2 %
        let reduction_pct = (d_dry - d_humid) / d_dry * 100.0;
        assert!(
            reduction_pct > 0.1,
            "humidity should reduce density by > 0.1 % @ 20 °C / 80 %: {:.3}%",
            reduction_pct
        );
        assert!(
            reduction_pct < 5.0,
            "humidity should not reduce density by > 5 % @ 20 °C: {:.3}%",
            reduction_pct
        );
    }

    #[test]
    fn humidity_increases_speed_of_sound() {
        let mach_dry = mach_with_humidity(340.0, 20.0, 0.0);
        let mach_humid = mach_with_humidity(340.0, 20.0, 80.0);
        // At the same velocity, higher SoS → lower Mach
        assert!(
            mach_humid < mach_dry,
            "humid air increases SoS → lower Mach: dry={:.6}, humid={:.6}",
            mach_dry,
            mach_humid
        );
        // Speed of sound increase should be small (~0.1–1 %)
        let sos_dry = 340.0 / mach_dry;
        let sos_humid = 340.0 / mach_humid;
        let sos_increase_pct = (sos_humid - sos_dry) / sos_dry * 100.0;
        assert!(
            sos_increase_pct > 0.02,
            "humidity should increase SoS measurably: {:.3}%",
            sos_increase_pct
        );
        assert!(
            sos_increase_pct < 2.0,
            "humidity should increase SoS by < 2 %: {:.3}%",
            sos_increase_pct
        );
    }

    #[test]
    fn high_humidity_effect_is_small_at_low_temp() {
        // At 5 °C, the saturation vapour pressure is low, so even 90 % RH
        // has a very small effect on density.
        let d_dry = density_with_humidity(0.0, 5.0, 0.0);
        let d_humid = density_with_humidity(0.0, 5.0, 90.0);

        let reduction_pct = (d_dry - d_humid) / d_dry * 100.0;
        assert!(
            reduction_pct < 1.0,
            "at 5 °C / 90 % RH, density reduction should be < 1 %: {:.4}%",
            reduction_pct
        );
    }

    #[test]
    fn humidity_effect_grows_with_temperature() {
        // At high temperature, e_sat is much larger → humidity matters more.
        let d_cool = density_with_humidity(0.0, 10.0, 80.0);
        let d_hot = density_with_humidity(0.0, 35.0, 80.0);

        // The density difference between dry and humid should be larger
        // at 35 °C than at 10 °C (holding RH constant).
        let dry = density_with_humidity(0.0, 10.0, 0.0);
        let diff_cool = (dry - d_cool) / dry;
        let diff_hot = (dry - d_hot) / dry;

        assert!(
            diff_hot > diff_cool,
            "humidity effect should be larger at higher temperature: cool={:.6}, hot={:.6}",
            diff_cool,
            diff_hot
        );
    }

    #[test]
    fn density_with_humidity_deterministic() {
        let a = density_with_humidity(500.0, 20.0, 50.0);
        let b = density_with_humidity(500.0, 20.0, 50.0);
        assert!((a - b).abs() < 1e-15, "density_with_humidity deterministic");
    }

    #[test]
    fn mach_with_humidity_deterministic() {
        let a = mach_with_humidity(800.0, 20.0, 50.0);
        let b = mach_with_humidity(800.0, 20.0, 50.0);
        assert!((a - b).abs() < 1e-15, "mach_with_humidity deterministic");
    }

    #[test]
    fn mach_is_reasonable() {
        // At 15 °C, dry, 800 m/s → Mach ~ 2.35
        let m = mach_with_humidity(800.0, 15.0, 0.0);
        assert!(m > 2.0, "Mach should be > 2 at 800 m/s: {}", m);
        assert!(m < 3.0, "Mach should be < 3 at 800 m/s: {}", m);
    }

    // ── Precipitation Tests ───────────────────────────────────────────────

    #[test]
    fn no_rain_leaves_bc_unchanged() {
        let dry = PrecipitationParams {
            rain_rate_mm_per_hour: 0.0,
            is_snowfall: false,
            cloud_base_altitude_m: 1000.0,
            temperature_c: 20.0,
        };
        let bc = precipitation_unadjusted_bc(&dry, 0.500, CaliberClass::Rifle);
        assert!((bc - 0.500).abs() < 1e-12, "No rain → BC unchanged: {}", bc);
    }

    #[test]
    fn light_rain_reduces_bc_slightly() {
        let light_rain = PrecipitationParams {
            rain_rate_mm_per_hour: 2.0,
            is_snowfall: false,
            cloud_base_altitude_m: 800.0,
            temperature_c: 15.0,
        };
        let bc = precipitation_unadjusted_bc(&light_rain, 0.500, CaliberClass::Rifle);
        assert!(bc < 0.500, "Light rain should reduce BC: {}", bc);
        assert!(
            (0.500 - bc) < 0.001,
            "Light rain reduction should be tiny (< 0.001): {}",
            0.500 - bc
        );
    }

    #[test]
    fn heavy_rain_reduces_bc_significantly() {
        let heavy_rain = PrecipitationParams {
            rain_rate_mm_per_hour: 50.0,
            is_snowfall: false,
            cloud_base_altitude_m: 600.0,
            temperature_c: 25.0,
        };
        let bc_rifle = precipitation_unadjusted_bc(&heavy_rain, 0.500, CaliberClass::Rifle);
        let expected_rifle = 0.500 * f64::exp(-0.15 * 50.0 / 1000.0);
        assert!(
            (bc_rifle - expected_rifle).abs() < 1e-12,
            "Rifle BC in heavy rain: expected {:.6}, got {:.6}",
            expected_rifle,
            bc_rifle
        );

        let bc_pistol = precipitation_unadjusted_bc(&heavy_rain, 0.150, CaliberClass::Pistol);
        let expected_pistol = 0.150 * f64::exp(-0.25 * 50.0 / 1000.0);
        assert!(
            (bc_pistol - expected_pistol).abs() < 1e-12,
            "Pistol BC in heavy rain: expected {:.6}, got {:.6}",
            expected_pistol,
            bc_pistol
        );
    }

    #[test]
    fn snowfall_uses_reduced_terminal_velocity() {
        let snow = PrecipitationParams {
            rain_rate_mm_per_hour: 10.0,
            is_snowfall: true,
            cloud_base_altitude_m: 500.0,
            temperature_c: -5.0,
        };
        let drift = rain_drift_velocity(&snow, 5.0, 800.0);
        // Snow vt ≈ 1.5 m/s → drift = 5 × (1.5/800) × 0.01 ≈ 9.375e-5
        let expected = 5.0 * (1.5 / 800.0) * 0.01;
        assert!(
            (drift - expected).abs() < 1e-12,
            "Snow drift: expected {:.10}, got {:.10}",
            expected,
            drift
        );
    }

    #[test]
    fn rain_drift_scales_with_crosswind() {
        let rain = PrecipitationParams {
            rain_rate_mm_per_hour: 25.0,
            is_snowfall: false,
            cloud_base_altitude_m: 700.0,
            temperature_c: 18.0,
        };
        let d1 = rain_drift_velocity(&rain, 2.0, 800.0);
        let d2 = rain_drift_velocity(&rain, 10.0, 800.0);
        assert!(d2 > d1, "Higher crosswind → higher drift: {} > {}", d2, d1);
        assert!(
            (d2 / d1 - 5.0).abs() < 1e-9,
            "5× crosswind → 5× drift (ratio={})",
            d2 / d1
        );
    }

    #[test]
    fn zero_rain_rate_gives_zero_drift() {
        let dry = PrecipitationParams {
            rain_rate_mm_per_hour: 0.0,
            is_snowfall: false,
            cloud_base_altitude_m: 1000.0,
            temperature_c: 20.0,
        };
        let d = rain_drift_velocity(&dry, 10.0, 800.0);
        assert!((d - 0.0).abs() < 1e-12, "No rain → zero drift: {}", d);
    }

    #[test]
    fn precipitation_functions_deterministic() {
        let rain = PrecipitationParams {
            rain_rate_mm_per_hour: 15.0,
            is_snowfall: false,
            cloud_base_altitude_m: 800.0,
            temperature_c: 20.0,
        };
        let bc1 = precipitation_unadjusted_bc(&rain, 0.350, CaliberClass::Rifle);
        let bc2 = precipitation_unadjusted_bc(&rain, 0.350, CaliberClass::Rifle);
        assert!(
            (bc1 - bc2).abs() < 1e-15,
            "precipitation_unadjusted_bc deterministic"
        );

        let d1 = rain_drift_velocity(&rain, 5.0, 750.0);
        let d2 = rain_drift_velocity(&rain, 5.0, 750.0);
        assert!((d1 - d2).abs() < 1e-15, "rain_drift_velocity deterministic");
    }
}
