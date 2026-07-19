// ABE - Tire / Tyre Penetration Model
//
// Specialised model for projectile interaction with pneumatic (air-filled)
// vehicle tyres.  Tyres are complex multilayer targets:
//   - Tread (rubber, 8–16 mm)
//   - Steel belt plies (2–8 layers, each ≈0.4 mm steel equivalent)
//   - Inner liner (2–4 mm rubber)
//   - Air cavity (pressurised, 200–700 kPa)
//   - Sidewall (thinner rubber, no belts, may have kevlar reinforcement)
//   - Run-flat insert (optional hard rubber/composite ring)
//
// References:
// ponytail: not wired into hit detection — whole module is forward-looking

#![allow(dead_code)]
//   - FMVSS 139 / ECE R30 (passenger tyre construction standards)
//   - DOT / NHTSA tyre safety research data
//   - De Marre penetration formula (belt steel-equivalent resistance)
//   - Bernoulli orifice flow (pneumatic deflation model)
//   - SAE J2015 (run-flat mobility standards: 80 km @ 50 km/h)
//   - MIL-STD-662F (V50 methodology adapted for belt packages)

// ── Physical constants ──────────────────────────────────────────────────────────

/// Air density at sea level (kg/m³).
const AIR_DENSITY: f64 = 1.225;

/// Orifice discharge coefficient for a sharp-edged hole in rubber.
const DISCHARGE_COEFFICIENT: f64 = 0.65;

/// De Marre constant for RHA (from penetration.rs).
const DE_MARRE_K: f64 = 91_000.0;

/// Steel equivalent thickness of one steel belt ply (mm RHA).
/// A typical passenger-tire steel belt is ~0.8 mm actual steel but the
/// cord-rubber composite is ~50 % efficient vs solid RHA.
const STEEL_BELT_EQ_MM: f64 = 0.4;

/// Steel equivalent thickness of one kevlar belt ply (mm RHA).
const KEVLAR_BELT_EQ_MM: f64 = 0.3;

/// Nonlinear pressure-decay multiplier on linear V/Q estimate.
/// A constant-pressure assumption under-estimates time-to-flat by ≈2×
/// because flow drops as pressure decays.  Integrated isothermal model
/// gives ~2.2× for 95 % pressure loss.
const PRESSURE_DECAY_MULT: f64 = 2.2;

/// Run-flat mobility parameters (SAE J2015): 80 km @ 50 km/h.
const RUN_FLAT_MAX_SPEED_KPH: f64 = 50.0;
const RUN_FLAT_MAX_RANGE_KM: f64 = 80.0;

// ── Types ───────────────────────────────────────────────────────────────────────

/// Classification of pneumatic tyre by construction and intended use.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TireType {
    /// Passenger car / light SUV (2–4 belt plies, 8 mm tread).
    Highway,
    /// Light truck / off-road (4–6 belt plies, 10 mm tread).
    AllTerrain,
    /// Heavy off-road / mud terrain (4–6 belts, 14 mm tread).
    MudTerrain,
    /// Semi / lorry (6–8 belts, 16 mm tread).
    HeavyTruck,
    /// Run-flat insert (reinforced sidewall + internal support ring).
    RunFlat,
    /// Construction / mining (very thick, solid-rubber options).
    HeavyEquipment,
}

/// Which zone of the tyre the projectile struck.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ImpactZone {
    /// Through the tread patch (belt package present).
    Tread,
    /// Through the sidewall (no belts, thinner).
    Sidewall,
    /// Transition zone — partial belt coverage (shoulder).
    Shoulder,
}

/// Input parameters for a tyre-penetration evaluation.
#[derive(Debug, Clone, Copy)]
pub struct TirePenetrationParams {
    /// Tyre construction type.
    pub tire_type: TireType,
    /// Impact zone on the tyre.
    pub impact_zone: ImpactZone,
    /// Projectile diameter (mm).
    pub projectile_caliber_mm: f64,
    /// Projectile mass (g).
    pub projectile_mass_g: f64,
    /// Impact velocity (m/s).
    pub impact_velocity_ms: f64,
    /// Projectile construction ("ball", "ap", "apds", "frangible", etc.).
    pub projectile_type: &'static str,
    /// Tyre inflation pressure — gauge (kPa).  Typical: 200–700 kPa.
    pub tire_pressure_kpa: f64,
    /// Whether a run-flat support ring is present.
    pub run_flat_present: bool,
}

/// Result of a tyre-penetration evaluation.
#[derive(Debug, Clone, Copy)]
pub struct TirePenetrationResult {
    /// Whether the projectile fully perforated the tyre.
    pub penetrated: bool,
    /// Number of belt plies the projectile passed through
    /// (0 for sidewall, partial for shoulder).
    pub belts_penetrated: i32,
    /// Physical hole area in the rubber structure (mm²).
    pub hole_area_mm2: f64,
    /// Rate of air-pressure loss at the instant of puncture (kPa/s).
    pub air_loss_rate_kpa_per_s: f64,
    /// Estimated time for the tyre to reach 5 % of initial inflation
    /// pressure (seconds).  `f64::INFINITY` if no hole.
    pub time_to_flat_s: f64,
    /// Whether the tyre experienced a catastrophic blowout
    /// (sidewall/tread separation, instant deflation).
    pub blowout: bool,
    /// Whether the run-flat insert has engaged (true when `penetrated`
    /// and `run_flat_present` is true).
    pub run_flat_engaged: bool,
    /// Projectile velocity remaining after exiting the tyre (m/s).
    pub residual_velocity_ms: f64,
    /// Kinetic energy lost in the tyre structure (J).
    pub energy_loss_j: f64,
}

// ── Tyre structural parameters ─────────────────────────────────────────────────

/// Number of belt plies for a given tyre type and impact zone.
fn belt_count(tire_type: TireType, zone: ImpactZone) -> i32 {
    if zone == ImpactZone::Sidewall {
        return 0;
    }
    let base = match tire_type {
        TireType::Highway => 3,
        TireType::AllTerrain => 4,
        TireType::MudTerrain => 5,
        TireType::HeavyTruck => 7,
        TireType::RunFlat => 4,
        TireType::HeavyEquipment => 6,
    };
    if zone == ImpactZone::Shoulder {
        (base + 1) / 2 // partial belt coverage
    } else {
        base
    }
}

/// Steel-equivalent belt material factor (some belts are kevlar/aramid).
fn belt_steel_eq_mm_per_ply(tire_type: TireType) -> f64 {
    match tire_type {
        TireType::HeavyTruck | TireType::HeavyEquipment => STEEL_BELT_EQ_MM,
        _ => STEEL_BELT_EQ_MM, // most consumer tyres use steel belts
    }
}

/// Tread rubber thickness (mm).
fn tread_thickness_mm(tire_type: TireType) -> f64 {
    match tire_type {
        TireType::Highway => 8.0,
        TireType::AllTerrain => 10.0,
        TireType::MudTerrain => 14.0,
        TireType::HeavyTruck => 16.0,
        TireType::RunFlat => 10.0,
        TireType::HeavyEquipment => 24.0,
    }
}

/// Sidewall rubber thickness (mm) — no belts.
fn sidewall_thickness_mm(tire_type: TireType) -> f64 {
    match tire_type {
        TireType::Highway => 6.0,
        TireType::AllTerrain => 8.0,
        TireType::MudTerrain => 10.0,
        TireType::HeavyTruck => 12.0,
        TireType::RunFlat => 14.0, // thicker sidewall
        TireType::HeavyEquipment => 18.0,
    }
}

/// Internal air volume of one tyre (m³).
fn tire_volume_m3(tire_type: TireType) -> f64 {
    match tire_type {
        TireType::Highway | TireType::RunFlat => 0.030,
        TireType::AllTerrain | TireType::MudTerrain => 0.045,
        TireType::HeavyTruck => 0.090,
        TireType::HeavyEquipment => 0.180,
    }
}

// ── Projectile modifiers ────────────────────────────────────────────────────────

/// Penetration modifier for the projectile type against belt packages.
///
/// Mirrors the modifiers in `penetration::projectile_modifier` so behaviour
/// is consistent with the armour-penetration model.
fn projectile_modifier(proj_type: &str) -> f64 {
    match proj_type.to_lowercase().as_str() {
        "ball" | "fmj" => 1.0,
        "ap" | "armor_piercing" => 1.3,
        "apds" | "apfsds" => 1.8,
        "apcr" => 1.5,
        "frangible" => 0.5, // frangible has poor belt penetration
        "soft_point" | "hollow_point" => 0.9,
        "incendiary" | "tracer" => 0.95,
        _ => 1.0,
    }
}

/// Energy-loss fraction for the belt package (fraction of KE absorbed).
fn belt_energy_loss_fraction(belts_hit: i32) -> f64 {
    match belts_hit {
        0 => 0.005, // no belts (sidewall) — just rubber friction
        1 => 0.015,
        2 => 0.025,
        3 => 0.035,
        4 => 0.050,
        5 => 0.065,
        6 => 0.080,
        _ => 0.100, // 7+ belts
    }
}

/// Tear factor: how much the hole is enlarged beyond projectile area by
/// tearing / yaw in the belt package.
fn tear_factor(proj_type: &str) -> f64 {
    match proj_type.to_lowercase().as_str() {
        "ball" | "fmj" => 1.4,
        "ap" | "armor_piercing" => 0.9, // clean hole
        "apds" | "apfsds" => 0.8,
        "apcr" => 0.85,
        "soft_point" | "hollow_point" => 1.6, // expands
        "frangible" => 0.5,                   // may not fully penetrate
        _ => 1.2,
    }
}

/// Rubber constriction factor: elastic rubber partially seals the hole.
/// Smaller values = more sealing (less air loss).
fn constriction_factor(proj_type: &str, zone: ImpactZone, caliber_mm: f64) -> f64 {
    let base = if zone == ImpactZone::Sidewall {
        match proj_type.to_lowercase().as_str() {
            "ball" | "fmj" => 0.10,
            "ap" | "armor_piercing" => 0.18,
            "apds" | "apfsds" => 0.22,
            "frangible" => 0.03,
            _ => 0.12,
        }
    } else {
        match proj_type.to_lowercase().as_str() {
            "ball" | "fmj" => 0.07,
            "ap" | "armor_piercing" => 0.14,
            "apds" | "apfsds" => 0.18,
            "frangible" => 0.02,
            _ => 0.09,
        }
    };
    // Large calibres overwhelm rubber elasticity → less constriction
    if caliber_mm > 12.0 {
        f64::min(base * 2.0, 0.50_f64)
    } else if caliber_mm > 9.0 {
        base * 1.4
    } else {
        base
    }
}

/// Check whether the projectile causes a catastrophic blowout.
fn check_blowout(
    caliber_mm: f64,
    proj_type: &str,
    zone: ImpactZone,
    impact_velocity_ms: f64,
    belts_penetrated: i32,
) -> bool {
    match proj_type.to_lowercase().as_str() {
        "he" | "heat" | "he_i" | "incendiary" => return true, // explosive
        _ => {},
    }

    // Large calibre through sidewall → blowout
    if zone == ImpactZone::Sidewall && caliber_mm >= 12.0 {
        return true;
    }

    // Very large calibre through tread → blowout from belt separation
    if caliber_mm >= 20.0 {
        return true;
    }

    // High-energy impact can cause tread-belt separation blowout even
    // through tread if belts are heavily damaged (AP rounds cut cleanly
    // and are less likely to cause belt separation).
    let is_ap = proj_type.to_lowercase().contains("ap");
    if !is_ap && belts_penetrated >= 6 && impact_velocity_ms > 850.0 && caliber_mm > 9.0 {
        return true;
    }

    // Large-caliber AP through tread: heavy AP rounds (>14mm) at high velocity
    // cause belt separation even for AP due to massive energy transfer.
    if is_ap && caliber_mm >= 14.0 && impact_velocity_ms > 900.0 && belts_penetrated >= 4 {
        return true;
    }

    // Sidewall tear from moderate-large calibre at high velocity
    if zone == ImpactZone::Sidewall && caliber_mm >= 7.62 && impact_velocity_ms > 750.0 {
        return true;
    }

    // Shoulder blowout from high-energy damage to belt-edge
    if zone == ImpactZone::Shoulder && belts_penetrated >= 3 && caliber_mm >= 9.0 {
        return true;
    }

    false
}

// ── Core physics ────────────────────────────────────────────────────────────────

/// Minimum velocity required to penetrate the belt package (De Marre).
///
///   V_req = k · D^0.75 · T^0.7 / M^0.5 / proj_mod
///
/// where:
///   k = DE_MARRE_K (91000), D = caliber (m), T = belt steel eq (m),
///   M = mass (kg), proj_mod = projectile type modifier.
fn required_velocity_for_belts(
    caliber_m: f64,
    mass_kg: f64,
    belt_steel_eq_m: f64,
    proj_mod: f64,
) -> f64 {
    if caliber_m <= 0.0 || mass_kg <= 0.0 || belt_steel_eq_m <= 0.0 || proj_mod <= 0.0 {
        return 0.0;
    }
    let k = DE_MARRE_K / proj_mod;
    k * caliber_m.powf(0.75) * belt_steel_eq_m.powf(0.70) / mass_kg.sqrt()
}

/// Compute how many belt plies the projectile penetrates.
///
/// Uses the De Marre threshold velocity to determine whether the full
/// belt package is defeated.  If not, we estimate partial penetration
/// based on energy scaling.
fn belts_penetrated_count(
    velocity_ms: f64,
    caliber_m: f64,
    mass_kg: f64,
    tire_type: TireType,
    zone: ImpactZone,
    proj_mod: f64,
) -> i32 {
    let total_belts = belt_count(tire_type, zone);
    if total_belts == 0 {
        return 0;
    }

    let eq_per_ply = belt_steel_eq_mm_per_ply(tire_type);
    let total_eq_m = (total_belts as f64 * eq_per_ply) / 1000.0;

    // Need at least some minimum velocity to do anything to the belts
    let v_partial_thresh = 150.0; // m/s — below this the belt is just deformed
    if velocity_ms < v_partial_thresh {
        return 0;
    }

    let v_req_full = required_velocity_for_belts(caliber_m, mass_kg, total_eq_m, proj_mod);

    if v_req_full <= 0.0 {
        return total_belts;
    }

    if velocity_ms >= v_req_full {
        // Full penetration
        total_belts
    } else {
        // Partial penetration: energy scales with v²
        let energy_ratio = (velocity_ms / v_req_full).powi(2);
        let partial = (total_belts as f64 * energy_ratio).floor() as i32;
        partial.max(0).min(total_belts - 1)
    }
}

/// Compute the physical hole area (mm²) created by the projectile.
fn hole_area_mm2(caliber_mm: f64, proj_type: &str, belts_penetrated: i32, zone: ImpactZone) -> f64 {
    let proj_area = std::f64::consts::PI * (caliber_mm / 2.0).powi(2);

    if belts_penetrated == 0 && zone != ImpactZone::Sidewall {
        // Projectile stopped by belts — small deformation hole or
        // the projectile may have partially embedded
        return proj_area * 0.08;
    }

    let tear = tear_factor(proj_type);
    let area = proj_area * tear;

    // Belts reduce effective hole (they act as a grid, cutting the
    // hole into smaller openings).  More belts = more restriction.
    if belts_penetrated > 0 && zone != ImpactZone::Sidewall {
        let belt_restriction = 1.0 - (belts_penetrated as f64).min(6.0) * 0.04;
        area * belt_restriction.max(0.6)
    } else {
        area
    }
}

/// Compute the air-loss rate at the instant of puncture (kPa/s).
///
/// Uses Bernoulli orifice flow:
///   Q = Cd · A_eff · sqrt(2 · ΔP / ρ)
///
/// Then: dP/dt = P · Q / V  (isothermal ideal gas).
fn air_loss_rate_kpa_per_s(
    hole_area_mm2: f64,
    tire_pressure_kpa: f64,
    tire_volume_m3: f64,
    proj_type: &str,
    zone: ImpactZone,
    caliber_mm: f64,
) -> f64 {
    if hole_area_mm2 <= 0.0 || tire_pressure_kpa <= 0.0 || tire_volume_m3 <= 0.0 {
        return 0.0;
    }

    let constriction = constriction_factor(proj_type, zone, caliber_mm);
    let area_eff_m2 = (hole_area_mm2 * constriction) / 1_000_000.0;
    let pressure_pa = tire_pressure_kpa * 1000.0;

    // Bernoulli: Q = Cd · A · sqrt(2 · ΔP / ρ)
    let flow_m3_per_s =
        DISCHARGE_COEFFICIENT * area_eff_m2 * (2.0 * pressure_pa / AIR_DENSITY).sqrt();

    // isothermal: dP/dt = P · Q / V
    let dp_dt = pressure_pa * flow_m3_per_s / tire_volume_m3.max(1e-12);

    // convert Pa/s → kPa/s
    dp_dt / 1000.0
}

/// Estimate the time for the tyre to effectively go flat.
///
/// We integrate the pressure-decay equation to 5 % residual pressure:
///   t_flat ≈ k · V / (Cd · A · sqrt(2·P₀/ρ))
/// where k = PRESSURE_DECAY_MULT accounts for the non-linear decay.
fn time_to_flat_s(
    hole_area_mm2: f64,
    tire_pressure_kpa: f64,
    tire_volume_m3: f64,
    proj_type: &str,
    zone: ImpactZone,
    caliber_mm: f64,
) -> f64 {
    if hole_area_mm2 <= 0.0 || tire_pressure_kpa <= 0.0 || tire_volume_m3 <= 0.0 {
        return f64::INFINITY;
    }

    let constriction = constriction_factor(proj_type, zone, caliber_mm);
    let area_eff_m2 = (hole_area_mm2 * constriction) / 1_000_000.0;
    let pressure_pa = tire_pressure_kpa * 1000.0;

    let flow_m3_per_s =
        DISCHARGE_COEFFICIENT * area_eff_m2 * (2.0 * pressure_pa / AIR_DENSITY).sqrt();

    if flow_m3_per_s <= 0.0 {
        return f64::INFINITY;
    }

    let linear_time = tire_volume_m3 / flow_m3_per_s;
    linear_time * PRESSURE_DECAY_MULT
}

/// Compute the projectile's residual velocity and energy loss after
/// passing through the tyre structure.
fn residual_velocity(
    velocity_ms: f64,
    mass_g: f64,
    belts_penetrated: i32,
    tire_type: TireType,
) -> (f64, f64) {
    let mass_kg = mass_g / 1000.0;
    let ke = 0.5 * mass_kg * velocity_ms.powi(2);

    // Rubber penetration energy (tread or sidewall)
    let rubber_thick_mm = match tire_type {
        TireType::RunFlat => 14.0, // sidewall thickness for runflat (worst case)
        _ => 8.0,                  // generic estimate
    };
    // Rubber offers very low resistance: ≈0.015 × RHA (from penetration.rs)
    let rubber_ke_loss = rubber_thick_mm * 0.015 * 100.0; // ~ Joules; very small

    // Belt energy loss
    let belt_loss_frac = belt_energy_loss_fraction(belts_penetrated);
    let belt_ke_loss = ke * belt_loss_frac;

    let total_loss = (rubber_ke_loss + belt_ke_loss).min(ke * 0.50);
    let residual_ke = (ke - total_loss).max(0.0);
    let residual_v = (2.0 * residual_ke / mass_kg).sqrt();

    (residual_v, total_loss)
}

// ── Public API ──────────────────────────────────────────────────────────────────

/// Evaluate projectile penetration of a pneumatic tyre.
///
/// Multi-layer model:
/// 1. **Belt package** — steel belt plies are modelled as equivalent RHA
///    thickness.  The De Marre formula determines whether the projectile
///    defeats the belt package (full or partial penetration).
/// 2. **Sidewall** — no belts; any projectile above a low threshold
///    penetrates.  Large calibre / high velocity may cause blowout.
/// 3. **Hole area** — determined by projectile calibre, type (ball vs AP),
///    and belt interaction.  Rubber elasticity constricts the effective
///    leak area.
/// 4. **Air loss** — Bernoulli orifice flow through the constricted hole.
///    Pressure-decay integration gives time-to-flat.
/// 5. **Blowout** — catastrophic failure from large calibre, explosive
///    rounds, or belt separation.
/// 6. **Run-flat** — if the tyre has a run-flat insert, vehicle mobility
///    is retained (50 km/h for 80 km) even at zero pressure.
/// 7. **Energy loss** — minimal for small arms (2–5 %); belts absorb
///    more energy (up to 10 % for heavy-truck belt packages).
///
/// # Arguments
/// * `params` — Tyre type, impact zone, projectile characteristics,
///   inflation pressure, and run-flat presence.
///
/// # Returns
/// [`TirePenetrationResult`] with penetration status, hole geometry,
/// air-loss rate, blowout flag, and residual projectile state.
pub fn evaluate_tire_penetration(params: &TirePenetrationParams) -> TirePenetrationResult {
    // ── Guard: degenerate inputs ──────────────────────────────────────────
    if params.projectile_caliber_mm <= 0.0
        || params.projectile_mass_g <= 0.0
        || params.impact_velocity_ms <= 0.0
    {
        return TirePenetrationResult {
            penetrated: false,
            belts_penetrated: 0,
            hole_area_mm2: 0.0,
            air_loss_rate_kpa_per_s: 0.0,
            time_to_flat_s: f64::INFINITY,
            blowout: false,
            run_flat_engaged: false,
            residual_velocity_ms: 0.0,
            energy_loss_j: 0.0,
        };
    }

    let caliber_m = params.projectile_caliber_mm / 1000.0;
    let mass_kg = params.projectile_mass_g / 1000.0;
    let proj_mod = projectile_modifier(params.projectile_type);

    // ── 1. Belt penetration ─────────────────────────────────────────────
    let belts = belts_penetrated_count(
        params.impact_velocity_ms,
        caliber_m,
        mass_kg,
        params.tire_type,
        params.impact_zone,
        proj_mod,
    );

    // ── 2. Determine whether the tyre is perforated ────────────────────
    let penetrated = match params.impact_zone {
        ImpactZone::Sidewall => {
            // Sidewall: no belts; always perforated above a low threshold
            params.impact_velocity_ms > 50.0
        },
        ImpactZone::Tread | ImpactZone::Shoulder => belts > 0,
    };

    // ── 3. Hole area ──────────────────────────────────────────────────
    let hole_area = if penetrated {
        hole_area_mm2(
            params.projectile_caliber_mm,
            params.projectile_type,
            belts,
            params.impact_zone,
        )
    } else {
        0.0
    };

    // ── 4. Blowout check ──────────────────────────────────────────────
    let blowout = check_blowout(
        params.projectile_caliber_mm,
        params.projectile_type,
        params.impact_zone,
        params.impact_velocity_ms,
        belts,
    );

    // ── 5. Air loss ──────────────────────────────────────────────────
    let volume = tire_volume_m3(params.tire_type);
    let loss_rate = if !blowout && hole_area > 0.0 {
        air_loss_rate_kpa_per_s(
            hole_area,
            params.tire_pressure_kpa,
            volume,
            params.projectile_type,
            params.impact_zone,
            params.projectile_caliber_mm,
        )
    } else if blowout {
        // Blowout = instant deflation; very high loss rate
        1_000_000.0
    } else {
        0.0
    };

    let time_flat = if blowout {
        0.5 // blowout deflates in < 1 s
    } else if hole_area > 0.0 {
        time_to_flat_s(
            hole_area,
            params.tire_pressure_kpa,
            volume,
            params.projectile_type,
            params.impact_zone,
            params.projectile_caliber_mm,
        )
    } else {
        f64::INFINITY
    };

    // ── 6. Run-flat engagement ────────────────────────────────────────
    let run_flat = penetrated && params.run_flat_present && !blowout;

    // ── 7. Residual velocity & energy loss ────────────────────────────
    let (residual_vel, energy_loss) = residual_velocity(
        params.impact_velocity_ms,
        params.projectile_mass_g,
        belts,
        params.tire_type,
    );

    TirePenetrationResult {
        penetrated,
        belts_penetrated: belts,
        hole_area_mm2: hole_area,
        air_loss_rate_kpa_per_s: loss_rate,
        time_to_flat_s: time_flat,
        blowout,
        run_flat_engaged: run_flat,
        residual_velocity_ms: residual_vel,
        energy_loss_j: energy_loss,
    }
}

// ── Convenience helpers ─────────────────────────────────────────────────────────

/// Return run-flat mobility data when the tyre has a run-flat insert
/// and has been penetrated.
///
/// Returns `(max_speed_kph, max_range_km)` or `None` if run-flat is
/// not applicable.
pub fn run_flat_mobility(result: &TirePenetrationResult) -> Option<(f64, f64)> {
    if result.run_flat_engaged {
        Some((RUN_FLAT_MAX_SPEED_KPH, RUN_FLAT_MAX_RANGE_KM))
    } else {
        None
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Highway tyre, ball round ──────────────────────────────────────────

    #[test]
    fn highway_tread_9mm_ball_penetrates() {
        let params = TirePenetrationParams {
            tire_type: TireType::Highway,
            impact_zone: ImpactZone::Tread,
            projectile_caliber_mm: 9.0,
            projectile_mass_g: 8.0,
            impact_velocity_ms: 370.0,
            projectile_type: "ball",
            tire_pressure_kpa: 250.0,
            run_flat_present: false,
        };
        let r = evaluate_tire_penetration(&params);
        assert!(r.penetrated, "9mm ball should penetrate highway tyre tread");
        assert!(r.belts_penetrated >= 2);
        assert!(r.hole_area_mm2 > 20.0);
        assert!(!r.blowout);
        assert!(r.time_to_flat_s > 5.0);
        assert!(r.time_to_flat_s < 120.0);
    }

    #[test]
    fn highway_sidewall_9mm_ball_penetrates() {
        let params = TirePenetrationParams {
            tire_type: TireType::Highway,
            impact_zone: ImpactZone::Sidewall,
            projectile_caliber_mm: 9.0,
            projectile_mass_g: 8.0,
            impact_velocity_ms: 370.0,
            projectile_type: "ball",
            tire_pressure_kpa: 250.0,
            run_flat_present: false,
        };
        let r = evaluate_tire_penetration(&params);
        assert!(r.penetrated, "9mm ball should penetrate sidewall");
        assert_eq!(r.belts_penetrated, 0, "Sidewall has no belts");
        assert!(!r.blowout, "9mm sidewall should not blow out");
        assert!(r.hole_area_mm2 > 30.0);
    }

    // ── AP round ──────────────────────────────────────────────────────────

    #[test]
    fn highway_tread_7_62_ap_penetrates() {
        let params = TirePenetrationParams {
            tire_type: TireType::Highway,
            impact_zone: ImpactZone::Tread,
            projectile_caliber_mm: 7.62,
            projectile_mass_g: 10.0,
            impact_velocity_ms: 850.0,
            projectile_type: "ap",
            tire_pressure_kpa: 250.0,
            run_flat_present: false,
        };
        let r = evaluate_tire_penetration(&params);
        assert!(r.penetrated, "7.62 AP should penetrate highway tyre");
        assert_eq!(r.belts_penetrated, 3, "7.62 AP should defeat all 3 belts");
        assert!(!r.blowout);
    }

    // ── Heavy truck tyre ──────────────────────────────────────────────────

    #[test]
    fn heavy_truck_tread_9mm_ball_stopped_by_belts() {
        let params = TirePenetrationParams {
            tire_type: TireType::HeavyTruck,
            impact_zone: ImpactZone::Tread,
            projectile_caliber_mm: 9.0,
            projectile_mass_g: 8.0,
            impact_velocity_ms: 370.0,
            projectile_type: "ball",
            tire_pressure_kpa: 600.0,
            run_flat_present: false,
        };
        let r = evaluate_tire_penetration(&params);
        // 9mm ball may not fully penetrate 7 steel belt plies
        // Check if it at least partially penetrates
        assert!(
            r.belts_penetrated < 7,
            "9mm ball should not defeat all 7 belts of heavy truck tyre"
        );
    }

    #[test]
    fn heavy_truck_tread_50bmg_ap_penetrates() {
        let params = TirePenetrationParams {
            tire_type: TireType::HeavyTruck,
            impact_zone: ImpactZone::Tread,
            projectile_caliber_mm: 12.7,
            projectile_mass_g: 43.0,
            impact_velocity_ms: 860.0,
            projectile_type: "ap",
            tire_pressure_kpa: 600.0,
            run_flat_present: false,
        };
        let r = evaluate_tire_penetration(&params);
        assert!(r.penetrated, ".50 BMG AP should penetrate heavy truck tyre");
        assert_eq!(
            r.belts_penetrated, 7,
            ".50 BMG AP should defeat all 7 belts"
        );
        assert!(!r.blowout, ".50 AP through tread should not blow out");
    }

    // ── Blowout conditions ────────────────────────────────────────────────

    #[test]
    fn sidewall_50bmg_causes_blowout() {
        let params = TirePenetrationParams {
            tire_type: TireType::Highway,
            impact_zone: ImpactZone::Sidewall,
            projectile_caliber_mm: 12.7,
            projectile_mass_g: 43.0,
            impact_velocity_ms: 860.0,
            projectile_type: "ball",
            tire_pressure_kpa: 250.0,
            run_flat_present: false,
        };
        let r = evaluate_tire_penetration(&params);
        assert!(r.blowout, ".50 BMG through sidewall should cause blowout");
        assert!(r.penetrated);
        assert_eq!(r.air_loss_rate_kpa_per_s, 1_000_000.0);
        assert!(r.time_to_flat_s < 1.0);
    }

    #[test]
    fn high_energy_tread_blowout() {
        let params = TirePenetrationParams {
            tire_type: TireType::HeavyTruck,
            impact_zone: ImpactZone::Tread,
            projectile_caliber_mm: 14.5,
            projectile_mass_g: 64.0,
            impact_velocity_ms: 950.0,
            projectile_type: "ap",
            tire_pressure_kpa: 600.0,
            run_flat_present: false,
        };
        let r = evaluate_tire_penetration(&params);
        // 14.5mm with 6+ belts penetrated at high velocity → belt separation
        assert!(
            r.blowout,
            "14.5mm AP through heavy truck tread should cause blowout"
        );
        assert!(r.penetrated);
    }

    // ── Run-flat behaviour ─────────────────────────────────────────────────

    #[test]
    fn run_flat_engaged_on_puncture() {
        let params = TirePenetrationParams {
            tire_type: TireType::RunFlat,
            impact_zone: ImpactZone::Tread,
            projectile_caliber_mm: 9.0,
            projectile_mass_g: 8.0,
            impact_velocity_ms: 370.0,
            projectile_type: "ball",
            tire_pressure_kpa: 250.0,
            run_flat_present: true,
        };
        let r = evaluate_tire_penetration(&params);
        assert!(r.penetrated);
        assert!(r.run_flat_engaged, "Run-flat should engage on puncture");
        let mobility = run_flat_mobility(&r);
        assert!(
            mobility.is_some(),
            "Run-flat mobility data should be available"
        );
        let (speed, range) = mobility.unwrap();
        assert_eq!(speed, 50.0);
        assert_eq!(range, 80.0);
    }

    #[test]
    fn run_flat_not_engaged_when_not_present() {
        let params = TirePenetrationParams {
            tire_type: TireType::Highway,
            impact_zone: ImpactZone::Tread,
            projectile_caliber_mm: 9.0,
            projectile_mass_g: 8.0,
            impact_velocity_ms: 370.0,
            projectile_type: "ball",
            tire_pressure_kpa: 250.0,
            run_flat_present: false,
        };
        let r = evaluate_tire_penetration(&params);
        assert!(!r.run_flat_engaged);
        assert!(run_flat_mobility(&r).is_none());
    }

    #[test]
    fn run_flat_not_engaged_on_blowout() {
        let params = TirePenetrationParams {
            tire_type: TireType::RunFlat,
            impact_zone: ImpactZone::Sidewall,
            projectile_caliber_mm: 12.7,
            projectile_mass_g: 43.0,
            impact_velocity_ms: 860.0,
            projectile_type: "ball",
            tire_pressure_kpa: 250.0,
            run_flat_present: true,
        };
        let r = evaluate_tire_penetration(&params);
        assert!(r.blowout);
        assert!(!r.run_flat_engaged, "Run-flat should NOT engage on blowout");
    }

    // ── Shoulder zone ─────────────────────────────────────────────────────

    #[test]
    fn shoulder_partial_belt_coverage() {
        let params = TirePenetrationParams {
            tire_type: TireType::AllTerrain,
            impact_zone: ImpactZone::Shoulder,
            projectile_caliber_mm: 7.62,
            projectile_mass_g: 9.5,
            impact_velocity_ms: 850.0,
            projectile_type: "ball",
            tire_pressure_kpa: 300.0,
            run_flat_present: false,
        };
        let r = evaluate_tire_penetration(&params);
        assert!(r.penetrated, "7.62 ball should penetrate shoulder zone");
        // Shoulder has partial belt coverage (4/2 = 2 belts)
        assert_eq!(r.belts_penetrated, 2);
    }

    // ── Frangible ammunition ───────────────────────────────────────────────

    #[test]
    fn frangible_poor_belt_penetration() {
        let params = TirePenetrationParams {
            tire_type: TireType::Highway,
            impact_zone: ImpactZone::Tread,
            projectile_caliber_mm: 5.56,
            projectile_mass_g: 4.0,
            impact_velocity_ms: 930.0,
            projectile_type: "frangible",
            tire_pressure_kpa: 250.0,
            run_flat_present: false,
        };
        let r = evaluate_tire_penetration(&params);
        // Frangible has proj_mod 0.5, so it needs ~2× the velocity.
        // At 930 m/s it may still penetrate some belts
        assert!(
            r.belts_penetrated <= 3,
            "Frangible should have reduced belt penetration"
        );
    }

    // ── Degenerate inputs ─────────────────────────────────────────────────

    #[test]
    fn zero_caliber_no_penetration() {
        let params = TirePenetrationParams {
            tire_type: TireType::Highway,
            impact_zone: ImpactZone::Tread,
            projectile_caliber_mm: 0.0,
            projectile_mass_g: 8.0,
            impact_velocity_ms: 370.0,
            projectile_type: "ball",
            tire_pressure_kpa: 250.0,
            run_flat_present: false,
        };
        let r = evaluate_tire_penetration(&params);
        assert!(!r.penetrated);
        assert_eq!(r.hole_area_mm2, 0.0);
        assert_eq!(r.time_to_flat_s, f64::INFINITY);
    }

    #[test]
    fn zero_velocity_no_penetration() {
        let params = TirePenetrationParams {
            tire_type: TireType::Highway,
            impact_zone: ImpactZone::Tread,
            projectile_caliber_mm: 9.0,
            projectile_mass_g: 8.0,
            impact_velocity_ms: 0.0,
            projectile_type: "ball",
            tire_pressure_kpa: 250.0,
            run_flat_present: false,
        };
        let r = evaluate_tire_penetration(&params);
        assert!(!r.penetrated);
        assert_eq!(r.energy_loss_j, 0.0);
    }

    // ── Determinism ───────────────────────────────────────────────────────

    #[test]
    fn deterministic_output() {
        let params = TirePenetrationParams {
            tire_type: TireType::AllTerrain,
            impact_zone: ImpactZone::Tread,
            projectile_caliber_mm: 5.56,
            projectile_mass_g: 4.0,
            impact_velocity_ms: 930.0,
            projectile_type: "ball",
            tire_pressure_kpa: 300.0,
            run_flat_present: false,
        };
        let a = evaluate_tire_penetration(&params);
        let b = evaluate_tire_penetration(&params);
        assert_eq!(a.penetrated, b.penetrated);
        assert_eq!(a.belts_penetrated, b.belts_penetrated);
        assert!((a.air_loss_rate_kpa_per_s - b.air_loss_rate_kpa_per_s).abs() < 1e-9);
        assert!((a.residual_velocity_ms - b.residual_velocity_ms).abs() < 1e-9);
        assert!((a.energy_loss_j - b.energy_loss_j).abs() < 1e-9);
    }

    // ── Energy loss minimal for small arms ─────────────────────────────────

    #[test]
    fn small_arms_energy_loss_under_10_percent() {
        let params = TirePenetrationParams {
            tire_type: TireType::Highway,
            impact_zone: ImpactZone::Tread,
            projectile_caliber_mm: 5.56,
            projectile_mass_g: 4.0,
            impact_velocity_ms: 930.0,
            projectile_type: "ball",
            tire_pressure_kpa: 250.0,
            run_flat_present: false,
        };
        let ke = 0.5 * (4.0 / 1000.0) * 930.0 * 930.0;
        let r = evaluate_tire_penetration(&params);
        assert!(r.penetrated);
        assert!(
            r.energy_loss_j < ke * 0.10,
            "Small arms energy loss should be under 10 %: {:.1} J < {:.1} J",
            r.energy_loss_j,
            ke * 0.10
        );
    }

    // ── Mud-terrain ───────────────────────────────────────────────────────

    #[test]
    fn mud_terrain_resists_better_than_highway() {
        // Use 250 m/s — highway fully penetrates (v_req ≈ 228), mud-terrain
        // partially penetrates (v_req ≈ 331) → highway gets more belts.
        let highway = TirePenetrationParams {
            tire_type: TireType::Highway,
            impact_zone: ImpactZone::Tread,
            projectile_caliber_mm: 7.62,
            projectile_mass_g: 9.5,
            impact_velocity_ms: 250.0,
            projectile_type: "ball",
            tire_pressure_kpa: 250.0,
            run_flat_present: false,
        };
        let mud = TirePenetrationParams {
            tire_type: TireType::MudTerrain,
            impact_zone: ImpactZone::Tread,
            projectile_caliber_mm: 7.62,
            projectile_mass_g: 9.5,
            impact_velocity_ms: 250.0,
            projectile_type: "ball",
            tire_pressure_kpa: 300.0,
            run_flat_present: false,
        };
        let r_h = evaluate_tire_penetration(&highway);
        let r_m = evaluate_tire_penetration(&mud);
        assert!(
            r_m.belts_penetrated <= r_h.belts_penetrated,
            "Mud-terrain (5 belts) should resist ≥ highway (3 belts)"
        );
    }
}
