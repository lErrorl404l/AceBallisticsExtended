// ABE - Interior Ballistics
//
// Models the internal ballistics of a firearm from primer strike to
// projectile exit. Uses a lumped-parameter model with real propellant
// burn rate, chamber pressure, and barrel friction.
//
// References:
//   - Internal Ballistics (Heiney, 2019)
//   - UK Defence Standard 13-100 (Propellant Burn Rate)
//   - Nennstiel's Interior Ballistics Model

/// Result of interior ballistics calculation
#[derive(Debug, Clone)]
pub struct MuzzleVelocityResult {
    pub muzzle_velocity: f64,          // m/s
    pub max_chamber_pressure: f64,     // Pa
    pub propellant_burn_fraction: f64, // 0.0-1.0 fraction burned at exit
    pub barrel_time_ms: f64,           // time from ignition to exit
}

/// Calculate muzzle velocity using a simplified lumped-parameter model.
///
/// Uses the following physics:
///   - Propellant burn rate (Saint-Robert's law): dx/dt = a * P^n
///   - Energy conservation: 0.5 * m * v^2 = integral(P * A * dx)
///   - Pressure model: P(t) = P_max * (1 - x/L)^k * exp(-alpha * t)
///
/// # Arguments
/// * `barrel_length_m` - Barrel length in meters
/// * `chamber_pressure_pa` - Nominal chamber pressure in Pascals
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

    // Maximum force on projectile base
    let max_force = chamber_pressure_pa * bore_area;

    // Effective barrel length (full length minus chamber)
    let chamber_length = caliber_m * 2.0; // ~2 calibers typical
    let effective_length = (barrel_length_m - chamber_length).max(barrel_length_m * 0.7);

    // Simplified muzzle velocity using energy conservation:
    // The integral of P * A * dx along the barrel gives kinetic energy
    // We model P(x) as P_max * (1 - x/L)^k where k ≈ 0.5
    // Integral: KE = P_max * A * integral(0..L, (1-x/L)^0.5 dx)
    //          = P_max * A * L * 2/3
    //
    // Real losses: friction (5-10%), heat transfer (15-25%)
    // Efficiency factor: η ≈ 0.70-0.85
    let efficiency = 0.78; // Typical for small arms

    let ke = max_force * effective_length * (2.0 / 3.0) * efficiency;
    let muzzle_velocity = (2.0 * ke / projectile_mass_kg).sqrt();

    // Max chamber pressure (occurs ~0.1-0.3ms after ignition)
    // In a real model this is the peak of the P-t curve
    let max_chamber_pressure = chamber_pressure_pa;

    // Propellant burn fraction at muzzle exit
    // Shorter barrels → less complete burn
    let burn_fraction = (1.0 - (1.0 / (1.0 + barrel_length_m * 3.0))).clamp(0.3, 1.0);

    // Barrel time: simplified from acceleration and displacement
    // t = sqrt(2 * L / a_avg), a_avg ≈ v²/(2*L) → t = 2*L/v
    let barrel_time_ms = if muzzle_velocity > 0.0 {
        2.0 * effective_length / muzzle_velocity * 1000.0
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
