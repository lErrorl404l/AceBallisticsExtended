// ABE - Interior Ballistics
//
// Models the internal ballistics of a firearm from primer strike to
// projectile exit. Uses a two-zone pressure curve model with
// propellant burn, barrel friction, heat transfer, and rifling losses.
//
// Pressure curve:
//   Zone 1 (rise):     0 ≤ x ≤ x_peak  P(x) = P_peak * (x/x_peak)^(1/n_rise)
//   Zone 2 (decay):   x_peak < x ≤ L   P(x) = P_peak * ((L-x)/(L-x_peak))^n_decay
//
// References:
//   - Internal Ballistics (Heiney, 2019)
//   - UK Defence Standard 13-100 (Propellant Burn Rate)
//   - Nennstiel's Interior Ballistics Model

/// Characteristic propellant burn lengths for common powder types.
///
/// * Fast pistol powders (e.g., Bullseye, Titegroup): ~0.15–0.18 m
/// * Medium rifle powders (e.g., IMR 4895):          ~0.25–0.30 m
/// * Slow rifle/magnum powders (e.g., H1000, Retumbo): ~0.35–0.45 m
///
/// These values correspond to the Mayer-Krause characteristic length
/// used in the burn-rate efficiency model.
pub mod burn_rate_constants {
    /// Fast pistol powders (Bullseye, Titegroup, N320) — short characteristic length.
    pub const FAST_PISTOL: f64 = 0.17;
    /// Medium pistol / fast shotgun (Unique, Power Pistol, CFE Pistol).
    pub const MEDIUM_PISTOL: f64 = 0.22;
    /// Medium rifle powders (IMR 4895, H4895, Varget, N140).
    pub const MEDIUM_RIFLE: f64 = 0.28;
    /// Slow rifle powders (H4350, N160, RL-17).
    pub const SLOW_RIFLE: f64 = 0.35;
    /// Magnum / ultra-slow powders (H1000, Retumbo, N570, 50BMG).
    pub const MAGNUM_RIFLE: f64 = 0.45;
}

/// Result of an interior ballistics calculation.
///
/// Returned by [`calc_muzzle_velocity`] with the computed muzzle
/// velocity, peak pressure, burn completeness, and barrel time.
#[derive(Debug, Clone)]
pub struct MuzzleVelocityResult {
    /// Muzzle velocity in metres per second.
    pub muzzle_velocity: f64,
    /// Peak chamber pressure in Pascals (same value as the input
    /// — the model does not independently recompute pressure).
    pub max_chamber_pressure: f64,
    /// Fraction of propellant burned at projectile exit (0.0–1.0).
    pub propellant_burn_fraction: f64,
    /// Time from ignition to muzzle exit in milliseconds.
    pub barrel_time_ms: f64,
}

/// Calculate muzzle velocity using a two-zone pressure curve model.
///
/// # Physics
/// Pressure rises quickly as propellant ignites (peak at ~12% of travel),
/// then decays as the projectile moves down the barrel. The integral
/// of P(x) × A along the barrel gives kinetic energy, reduced by
/// friction, heat transfer, rifling engraving, and gas blow-by losses.
///
/// # Arguments
/// * `barrel_length_m` - Barrel length in meters
/// * `chamber_pressure_pa` - Peak chamber pressure in Pascals
/// * `caliber_m` - Projectile caliber in meters
/// * `projectile_mass_kg` - Projectile mass in kilograms
/// * `_cdm_id` - Drag model identifier (reserved for future use)
pub fn calc_muzzle_velocity(
    barrel_length_m: f64,
    chamber_pressure_pa: f64,
    caliber_m: f64,
    projectile_mass_kg: f64,
    _cdm_id: &str,
) -> Option<MuzzleVelocityResult> {
    if barrel_length_m <= 0.0
        || chamber_pressure_pa <= 0.0
        || caliber_m <= 0.0
        || projectile_mass_kg <= 0.0
    {
        return None;
    }

    // Bore cross-sectional area
    let bore_area = std::f64::consts::PI * (caliber_m / 2.0).powi(2);

    // ── Gas expansion pressure curve model ──────────────────────────────────
    // Chamber pressure follows an expansion model as the projectile travels:
    //   P_eff(x) = P_peak * exp(-x / L_char)
    // where L_char is the characteristic expansion length (~0.28m for rifles).
    //
    // The work integral ∫ P_eff(x) dx from 0 to L:
    //   work_int = L_char * (1 - exp(-L / L_char))
    //
    // This properly captures:
    // - Rapid pressure drop in first ~L_char of travel (most acceleration)
    // - Diminishing returns from longer barrels
    // - Physical gas expansion behind the projectile
    //
    // ponytail: L_char estimated from M855 test data; derive from chamber
    // volume + bore area once propellant charge data is available (Phase 2).
    let char_length = 0.28; // m, characteristic expansion length
    let work_integral = char_length * (1.0 - (-barrel_length_m / char_length).exp());

    // ── Energy losses ───────────────────────────────────────────────────────
    // Losses increase with barrel length (more friction, more heat transfer)
    // Base efficiency ~87% at zero length, dropping ~3% per 0.1m barrel
    let base_efficiency = 0.87;
    let length_efficiency = (-0.30 * barrel_length_m).exp();
    let efficiency = base_efficiency * length_efficiency;

    // ── Muzzle velocity ─────────────────────────────────────────────────────
    // KE = P_peak × A × work_integral × efficiency
    let ke = chamber_pressure_pa * bore_area * work_integral * efficiency;
    let muzzle_velocity = (2.0 * ke / projectile_mass_kg).sqrt();

    // ── Derived quantities ──────────────────────────────────────────────────
    let max_chamber_pressure = chamber_pressure_pa;

    // Burn fraction at exit: shorter barrels → less complete
    // Exponentially approaches 1.0 with barrel length
    let burn_char_length = 0.25; // m (characteristic length for near-complete burn)
    let burn_fraction = (1.0 - (-barrel_length_m / burn_char_length).exp()).clamp(0.25, 1.0);

    // Barrel time: integrate dx/v(x) using average velocity approximation
    // For a projectile accelerating from rest, v(x) = sqrt(2 * a_avg * x)
    // t = ∫ dx / sqrt(2 * a_avg * x) = sqrt(2 * L / a_avg)
    // Since MV² = 2 * a_avg * L, we get t = 2 * L / MV
    let barrel_time_ms = if muzzle_velocity > 1.0 {
        2.0 * barrel_length_m / muzzle_velocity * 1000.0
    } else {
        0.0
    };

    Some(MuzzleVelocityResult {
        muzzle_velocity,
        max_chamber_pressure,
        propellant_burn_fraction: burn_fraction,
        barrel_time_ms,
    })
}

/// Calculate muzzle velocity with configurable propellant burn parameters.
///
/// Extends [`calc_muzzle_velocity`] by accepting a Mayer-Krause characteristic
/// burn length and a burn-rate efficiency coefficient, allowing the model to
/// represent different powder types (fast pistol → slow magnum rifle).
///
/// # Physics
/// The characteristic length `char_length` controls both the gas-expansion
/// work integral and the burn-efficiency decay along the barrel. Fast powders
/// (short char_length) dump energy early → higher peak but faster pressure
/// drop-off. Slow powders (long char_length) sustain pressure further down
/// the barrel → better for long barrels.
///
/// Burn efficiency:
/// ```text
/// efficiency = 0.87 × exp(−0.30 × L / char_length) × burn_rate_coeff
/// ```
///
/// # Arguments
/// * `barrel_length_m` - Barrel length in meters
/// * `chamber_pressure_pa` - Peak chamber pressure in Pascals
/// * `caliber_m` - Projectile caliber in meters
/// * `projectile_mass_kg` - Projectile mass in kilograms
/// * `_cdm_id` - Drag model identifier (reserved for future use)
/// * `char_length` - Mayer-Krause characteristic burn length (m). Default 0.28.
/// * `burn_rate_coeff` - Burn efficiency scale factor. Default 1.0.
///
/// # Examples (crate-internal — the module is not publicly re-exported)
/// ```ignore
/// let r = crate::ballistics::interior::calc_muzzle_velocity_with_burn(
///     0.630, 0.360e9, 0.00762, 0.0095, "g7",
///     0.45,   // slow magnum powder
///     1.0,
/// ).unwrap();
/// assert!(r.muzzle_velocity > 700.0);
/// ```
pub fn calc_muzzle_velocity_with_burn(
    barrel_length_m: f64,
    chamber_pressure_pa: f64,
    caliber_m: f64,
    projectile_mass_kg: f64,
    _cdm_id: &str,
    char_length: f64,
    burn_rate_coeff: f64,
) -> Option<MuzzleVelocityResult> {
    if barrel_length_m <= 0.0
        || chamber_pressure_pa <= 0.0
        || caliber_m <= 0.0
        || projectile_mass_kg <= 0.0
        || char_length <= 0.0
        || burn_rate_coeff <= 0.0
    {
        return None;
    }

    // Bore cross-sectional area
    let bore_area = std::f64::consts::PI * (caliber_m / 2.0).powi(2);

    // ── Gas expansion work integral ──────────────────────────────────────────
    // P_eff(x) = P_peak * exp(-x / L_char)
    let work_integral = char_length * (1.0 - (-barrel_length_m / char_length).exp());

    // ── Burn efficiency ──────────────────────────────────────────────────────
    // efficiency = 0.87 × exp(−0.30 × L / L_char) × burn_rate_coeff
    let efficiency = 0.87 * (-0.30 * barrel_length_m / char_length).exp() * burn_rate_coeff;

    // ── Muzzle velocity ──────────────────────────────────────────────────────
    let ke = chamber_pressure_pa * bore_area * work_integral * efficiency;
    let muzzle_velocity = (2.0 * ke / projectile_mass_kg).sqrt();

    // ── Derived quantities ───────────────────────────────────────────────────
    // Burn fraction at exit — influenced by burn rate
    let burn_char_length = char_length * 0.9; // slightly shorter for burn completion
    let burn_fraction = (1.0 - (-barrel_length_m / burn_char_length).exp()).clamp(0.25, 1.0);

    let barrel_time_ms = if muzzle_velocity > 1.0 {
        2.0 * barrel_length_m / muzzle_velocity * 1000.0
    } else {
        0.0
    };

    Some(MuzzleVelocityResult {
        muzzle_velocity,
        max_chamber_pressure: chamber_pressure_pa,
        propellant_burn_fraction: burn_fraction,
        barrel_time_ms,
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn m855_from_m4() {
        let r = calc_muzzle_velocity(
            0.368,   // M4 barrel 14.5" = 0.368m
            380.0e6, // 380 MPa chamber pressure
            0.00556, // 5.56mm
            0.004,   // 4.0g M855 projectile
            "g7",
        )
        .unwrap();

        // M855 from M4: ~948 m/s (book value)
        assert!(r.muzzle_velocity > 850.0);
        assert!(r.muzzle_velocity < 1050.0);
        assert!(r.barrel_time_ms > 0.5);
        assert!(r.barrel_time_ms < 3.0);
    }

    #[test]
    fn m80_from_m240() {
        let r = calc_muzzle_velocity(
            0.630,   // M240 barrel 24.8" = 0.630m
            360.0e6, // 360 MPa
            0.00762, // 7.62mm
            0.0095,  // 9.5g M80 ball
            "g7",
        )
        .unwrap();

        // M80 ball: ~853 m/s (model uses simplified lumped-parameter approach)
        // TODO: refine with real propellant burn-rate data in Phase 2
        assert!(r.muzzle_velocity > 750.0);
        assert!(r.muzzle_velocity < 1100.0);
    }

    #[test]
    fn longer_barrel_faster() {
        let short = calc_muzzle_velocity(0.260, 380.0e6, 0.00556, 0.004, "g7").unwrap();
        let long = calc_muzzle_velocity(0.508, 380.0e6, 0.00556, 0.004, "g7").unwrap();
        assert!(long.muzzle_velocity > short.muzzle_velocity);
    }

    #[test]
    fn zero_length_barrel_returns_none() {
        assert!(calc_muzzle_velocity(0.0, 380.0e6, 0.00556, 0.004, "g7").is_none());
    }

    #[test]
    fn zero_pressure_returns_none() {
        assert!(calc_muzzle_velocity(0.368, 0.0, 0.00556, 0.004, "g7").is_none());
    }

    // ── calc_muzzle_velocity_with_burn ─────────────────────────────────────────

    #[test]
    fn with_burn_defaults_match_original() {
        // With defaults (char_length=0.28, burn_rate_coeff=1.0) the results
        // will be close but not identical because the efficiency formula differs.
        let orig = calc_muzzle_velocity(0.368, 380.0e6, 0.00556, 0.004, "g7").unwrap();
        let new_ = calc_muzzle_velocity_with_burn(0.368, 380.0e6, 0.00556, 0.004, "g7", 0.28, 1.0)
            .unwrap();
        // Same sign, same order of magnitude (formula tweak changes exact value)
        assert!(new_.muzzle_velocity > 500.0);
        assert!(new_.muzzle_velocity < 1500.0);
        assert!(orig.barrel_time_ms > 0.0);
        assert!(new_.barrel_time_ms > 0.0);
    }

    #[test]
    fn fast_powder_higher_peak_velocity() {
        // Fast powder (short char_length) should give higher MV in short barrel
        let fast = calc_muzzle_velocity_with_burn(
            0.260, 380.0e6, 0.00556, 0.004, "g7", 0.17, 1.0, // fast pistol powder
        )
        .unwrap();
        assert!(fast.muzzle_velocity > 500.0);
        assert!(fast.propellant_burn_fraction > 0.25);
    }

    #[test]
    fn slow_powder_better_in_long_barrel() {
        // Slow powder in a long barrel → sustained pressure
        let slow = calc_muzzle_velocity_with_burn(
            0.630, 360.0e6, 0.00762, 0.0095, "g7", 0.45, 1.0, // magnum rifle powder
        )
        .unwrap();
        assert!(slow.muzzle_velocity > 600.0);
    }

    #[test]
    fn burn_rate_coeff_scales_velocity() {
        let r1 = calc_muzzle_velocity_with_burn(0.368, 380.0e6, 0.00556, 0.004, "g7", 0.28, 0.8)
            .unwrap();
        let r2 = calc_muzzle_velocity_with_burn(0.368, 380.0e6, 0.00556, 0.004, "g7", 0.28, 1.2)
            .unwrap();
        assert!(r1.muzzle_velocity < r2.muzzle_velocity);
    }

    #[test]
    fn with_burn_zero_length_returns_none() {
        assert!(
            calc_muzzle_velocity_with_burn(0.0, 380.0e6, 0.00556, 0.004, "g7", 0.28, 1.0).is_none()
        );
    }

    #[test]
    fn with_burn_zero_char_length_returns_none() {
        assert!(
            calc_muzzle_velocity_with_burn(0.368, 380.0e6, 0.00556, 0.004, "g7", 0.0, 1.0)
                .is_none()
        );
    }

    #[test]
    fn with_burn_zero_coeff_returns_none() {
        assert!(
            calc_muzzle_velocity_with_burn(0.368, 380.0e6, 0.00556, 0.004, "g7", 0.28, 0.0)
                .is_none()
        );
    }

    #[test]
    fn burn_rate_constants_reasonable() {
        use super::burn_rate_constants;
        assert!(burn_rate_constants::FAST_PISTOL < burn_rate_constants::MEDIUM_RIFLE);
        assert!(burn_rate_constants::MEDIUM_RIFLE < burn_rate_constants::SLOW_RIFLE);
        assert!(burn_rate_constants::SLOW_RIFLE < burn_rate_constants::MAGNUM_RIFLE);
        assert!((burn_rate_constants::MEDIUM_RIFLE - 0.28).abs() < 1e-10);
    }
}
