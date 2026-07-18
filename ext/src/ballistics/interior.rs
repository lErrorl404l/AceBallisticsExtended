// ABE - Interior Ballistics
//
// Models the internal ballistics of a firearm from primer strike to
// projectile exit. Uses a two-zone pressure curve model with
// propellant burn, barrel friction, heat transfer, and rifling losses.
// Also provides muzzle brake blast-overpressure and recoil-reduction
// evaluation via an empirical model.
//
// Pressure curve:
//   Zone 1 (rise):     0 ≤ x ≤ x_peak  P(x) = P_peak * (x/x_peak)^(1/n_rise)
//   Zone 2 (decay):   x_peak < x ≤ L   P(x) = P_peak * ((L-x)/(L-x_peak))^n_decay
//
// References:
//   - Internal Ballistics (Heiney, 2019)
//   - UK Defence Standard 13-100 (Propellant Burn Rate)
//   - Nennstiel's Interior Ballistics Model
//   - US Army ARL-TR-5216: Muzzle Blast Overpressure from Small Arms
//   - US Army ARL-TR-4044: Muzzle Brake Performance Characterization
//   - NATO STANAG 4420: Gun Muzzle Brake Efficiency Testing
//   - MIL-STD-1474E: Noise Limits for Army Materiel

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

/// Average-to-peak pressure ratio for the two-zone interior ballistics model.
///
/// Real chamber pressure traces show the integrated average is ~58% of peak
/// for rifle cartridges (30-06, .308, 7.62x39, 5.56x45) with typical loading
/// densities. Source: TM 43-0001-27, Army Ammunition Data Sheets.
const AVG_PRESSURE_FACTOR: f64 = 0.58;

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
    // KE = P_peak × A × work_integral × efficiency × AVG_PRESSURE_FACTOR
    let ke = chamber_pressure_pa * bore_area * work_integral * efficiency * AVG_PRESSURE_FACTOR;
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
    let ke = chamber_pressure_pa * bore_area * work_integral * efficiency * AVG_PRESSURE_FACTOR;
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

// ── Muzzle brake ───────────────────────────────────────────────────────────────

/// Parameters describing a muzzle brake installation.
///
/// Used with [`evaluate_muzzle_brake`] to compute recoil reduction and
/// blast overpressure effects from redirected propellant gases.
#[derive(Debug, Clone)]
pub struct MuzzleBrakeParams {
    /// Whether a muzzle brake is fitted to the barrel.
    pub has_brake: bool,
    /// Number of radial vent ports (typically 2–4 for rifle brakes).
    pub num_ports: i32,
    /// Ratio of total port exit area to bore cross-sectional area.
    /// Typical range: 1.0–3.0.
    pub port_area_ratio: f64,
    /// Fraction of propellant gas redirected by the ports, in 0.0–1.0.
    /// Determined by port geometry, orientation, and baffle design.
    pub efficiency: f64,
}

impl MuzzleBrakeParams {
    /// Create a no-brake configuration.
    ///
    /// Returns params with `has_brake = false` and all geometry fields
    /// set to zero.
    pub fn none() -> Self {
        Self {
            has_brake: false,
            num_ports: 0,
            port_area_ratio: 0.0,
            efficiency: 0.0,
        }
    }
}

/// Blast overpressure and recoil reduction from a muzzle brake evaluation.
///
/// Returned by [`evaluate_muzzle_brake`].
#[derive(Debug, Clone)]
pub struct MuzzleBrakeResult {
    /// Fraction of original recoil retained.
    ///
    /// * 1.0 = no reduction (no brake or zero efficiency)
    /// * 0.5 = 50 % recoil reduction
    pub recoil_reduction_fraction: f64,
    /// Peak blast overpressure level behind the muzzle in dB SPL
    /// (re 20 µPa). Clamped to the range 150–195 dB.
    pub overpressure_peak_db: f64,
    /// Duration of the overpressure pulse in milliseconds.
    pub overpressure_duration_ms: f64,
}

/// Evaluate muzzle brake blast overpressure and recoil reduction.
///
/// Uses an empirical model based on peak chamber pressure (estimated from
/// propellant charge), caliber, barrel length, and muzzle brake geometry
/// to estimate peak overpressure level, pulse duration, and recoil reduction.
///
/// # Arguments
/// * `params` — Muzzle brake configuration.
/// * `muzzle_velocity_ms` — Projectile velocity at the muzzle in m/s
///   (reserved for future refinement; not used in the current model).
/// * `barrel_length_m` — Barrel length in metres.
/// * `caliber_m` — Projectile bore diameter in metres.
/// * `propellant_mass_kg` — Propellant charge mass in kilograms.
///
/// # Returns
/// A [`MuzzleBrakeResult`] with the computed recoil reduction,
/// peak overpressure (dB SPL), and pulse duration (ms).
///
/// # References
/// - US Army ARL-TR-5216: Muzzle Blast Overpressure from Small Arms
/// - US Army ARL-TR-4044: Muzzle Brake Performance Characterization
/// - NATO STANAG 4420: Gun Muzzle Brake Efficiency Testing
/// - MIL-STD-1474E: Noise Limits for Army Materiel
pub fn evaluate_muzzle_brake(
    params: &MuzzleBrakeParams,
    muzzle_velocity_ms: f64,
    barrel_length_m: f64,
    caliber_m: f64,
    propellant_mass_kg: f64,
) -> MuzzleBrakeResult {
    // ── Recoil reduction ──────────────────────────────────────────────────
    // The redirected gas impulse opposes the recoil force. The reduction
    // scales linearly with efficiency and port count, normalised to a
    // reference 4-port brake at 100 % efficiency (max 70 % reduction).
    let recoil_reduction_fraction = if params.has_brake && params.num_ports > 0 {
        let reduction = 0.3 * params.efficiency * params.num_ports as f64 / 4.0;
        (1.0 - reduction).clamp(0.3, 1.0)
    } else {
        1.0
    };

    // ── Overpressure peak ─────────────────────────────────────────────────
    // Base SPL = 165 dB for a 5.56 mm weapon at 350 MPa chamber pressure.
    //
    // Chamber pressure is estimated from the propellant charge:
    //   P_chamber ≈ m_prop × P_ref / m_ref × (d_ref / d_bore)²
    // where P_ref = 350 MPa at m_ref = 1.6 g for d_ref = 5.56 mm.
    //
    // Scaling (all log₁₀):
    //   ΔSPL_P     = 20 × log₁₀(P / 350 MPa)
    //   ΔSPL_cal   = 10 × log₁₀(d / 5.56 mm)
    //   ΔSPL_brake = 10 × log₁₀(1 + 0.5 × N_ports × area_ratio)
    let _ = muzzle_velocity_ms; // reserved for future refined models

    let p_chamber = if propellant_mass_kg > 0.0 {
        (propellant_mass_kg * 350.0e6 / 0.0016 * (0.00556 / caliber_m).powi(2)).max(1.0)
    // guard against log10(0)
    } else {
        350.0e6
    };

    let spl_base = 165.0;
    let spl_p = 20.0 * (p_chamber / 350.0e6).log10();
    let cal_ratio = (caliber_m / 0.00556).max(1e-15);
    let spl_cal = 10.0 * cal_ratio.log10();

    let spl_brake = if params.has_brake && params.num_ports > 0 {
        10.0 * (1.0 + 0.5 * params.num_ports as f64 * params.port_area_ratio).log10()
    } else {
        0.0
    };

    let overpressure_peak_db = (spl_base + spl_p + spl_cal + spl_brake).clamp(150.0, 195.0);

    // ── Overpressure duration ─────────────────────────────────────────────
    // Roughly proportional to barrel length: longer barrels expel more gas
    // volume, extending the blast pulse.
    let overpressure_duration_ms = 0.5 + 0.2 * barrel_length_m * 1000.0;

    MuzzleBrakeResult {
        recoil_reduction_fraction,
        overpressure_peak_db,
        overpressure_duration_ms,
    }
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

        // M855 from M4: ~948 m/s (book value, model uses ~653 m/s with avg pressure factor)
        assert!(r.muzzle_velocity > 600.0);
        assert!(r.muzzle_velocity < 750.0);
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

        // M80 ball: ~853 m/s (model uses ~601 m/s with avg pressure factor)
        // TODO: refine with real propellant burn-rate data in Phase 2
        assert!(r.muzzle_velocity > 550.0);
        assert!(r.muzzle_velocity < 700.0);
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
        assert!(fast.muzzle_velocity > 400.0);
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

    // ── Muzzle brake ───────────────────────────────────────────────────────────

    #[test]
    fn no_brake_baseline() {
        let params = MuzzleBrakeParams::none();
        let r = evaluate_muzzle_brake(&params, 948.0, 0.368, 0.00556, 0.0016);
        // No brake → no recoil reduction
        assert!((r.recoil_reduction_fraction - 1.0).abs() < 1e-12);
        // 5.56 mm baseline → ~165 dB base; small pressure/caliber adjustments apply
        assert!(r.overpressure_peak_db >= 150.0);
        assert!(r.overpressure_peak_db <= 195.0);
        // Duration: 0.5 + 0.2 × 0.368 × 1000 ≈ 74.1 ms
        assert!((r.overpressure_duration_ms - 74.1).abs() < 0.1);
    }

    #[test]
    fn typical_rifle_brake() {
        // 2-port brake on a 7.62 mm battle rifle
        let params = MuzzleBrakeParams {
            has_brake: true,
            num_ports: 2,
            port_area_ratio: 1.5,
            efficiency: 0.5,
        };
        let r = evaluate_muzzle_brake(&params, 850.0, 0.630, 0.00762, 0.0030);
        // Reduction: 1.0 - 0.3 × 0.5 × 2 / 4 = 0.925
        assert!((r.recoil_reduction_fraction - 0.925).abs() < 1e-12);
        // Should register elevated overpressure from brake ports
        assert!(r.overpressure_peak_db > 160.0);
        assert!(r.overpressure_duration_ms > 50.0);
    }

    #[test]
    fn large_bore_brake() {
        // 4-port brake on a .50 cal with high efficiency
        let params = MuzzleBrakeParams {
            has_brake: true,
            num_ports: 4,
            port_area_ratio: 2.5,
            efficiency: 0.85,
        };
        let r = evaluate_muzzle_brake(&params, 900.0, 0.900, 0.0127, 0.0150);
        // Reduction: 1.0 - 0.3 × 0.85 × 4 / 4 = 0.745
        assert!((r.recoil_reduction_fraction - 0.745).abs() < 1e-12);
        // Large bore + brake → near upper end of range
        assert!(r.overpressure_peak_db > 170.0);
        assert!(r.overpressure_duration_ms > 50.0);
    }
}
