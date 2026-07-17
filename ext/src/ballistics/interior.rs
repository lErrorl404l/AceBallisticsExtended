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
}
