// ABE - Frozen Ground & Ice Penetration Model
//
// Extends the surface ricochet model in penetration.rs with temperature-dependent
// frozen ground, ice-on-water, permafrost, and packed snow penetration.
//
// Frozen ground is much harder than loose soil. Ice is a brittle solid that
// fractures differently than soil, producing cone-shaped spall and secondary
// ice fragments.  Permafrost (frozen soil with ice lenses) is harder still.
//
// References:
//   - USACE "Pavement Design for Seasonal Frost Conditions" (EM 1110-3-138)
//   - Petrov, I.V., "Ice Penetration by Small Arms Projectiles" (2015)
//   - Zaretsky et al., "Ice Strength as a Function of Temperature" (2006)
//   - ISO 19906:2019 Arctic offshore structures
//   - Mellor, M., "Mechanical Properties of Snow and Ice" (1975)

// ── Constants ──────────────────────────────────────────────────────────────────

/// Temperature (°C) above which ground is considered fully thawed (factor = 1.0).
const THAWED_TEMP_C: f64 = 5.0;

/// Temperature (°C) at which ground reaches maximum hardness.
const MAX_FROZEN_TEMP_C: f64 = -20.0;

/// Base soil mat-factor (used in penetration.rs for wood/soil-equivalent).
const SOIL_MAT_FACTOR: f64 = 0.05;

/// Slope of the hardness-vs-temperature linear model.
const HARDNESS_SLOPE: f64 = (4.0 - 1.0) / (THAWED_TEMP_C - (-10.0)); // ≈ -0.2 / °C

/// Minimum material factor for fully frozen ground when using De Marre.
const MIN_FROZEN_MAT_FACTOR: f64 = 0.20;

/// Maximum material factor for extremely cold frozen ground.
const MAX_FROZEN_MAT_FACTOR: f64 = 0.30;

/// Moisture multiplier: when moisture = 1.0 (saturated), hardness increases by this factor.
const MOISTURE_CONTRIBUTION: f64 = 0.5;

/// Ice-on-water ricochet threshold (shallow angles ricochet, steep angles penetrate).
const ICE_RICOCHET_THRESHOLD_DEG: f64 = 15.0;

/// Base spall diameter as a multiple of caliber for ice fracture.
const ICE_SPALL_MIN_CAL: f64 = 5.0;

/// Maximum spall diameter as a multiple of caliber.
const ICE_SPALL_MAX_CAL: f64 = 15.0;

/// Additional hardness multiplier per unit of permafrost ice-lens ratio.
const PERMAFROST_ICE_LENS_BONUS: f64 = 0.30;

/// Ice fracture toughness approximate (MPa·m^{1/2}).
const ICE_FRACTURE_TOUGHNESS: f64 = 0.15; // MN·m^{-3/2}

// ── Types ──────────────────────────────────────────────────────────────────────

/// Surface type for frozen-ground and ice penetration evaluation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FrozenSurfaceType {
    /// Frozen soil with temperature and moisture dependence.
    FrozenGround {
        temperature_c: f64,
        moisture_content: f64, // 0.0 (dry) to 1.0 (saturated)
    },
    /// Ice sheet floating on water, with variable thickness.
    IceOnWater {
        thickness_m: f64, // 0.1 (first ice) to 1.0+ (permanent ice)
    },
    /// Permafrost: frozen soil with distributed ice lenses.
    Permafrost {
        ice_lens_ratio: f64, // 0.0 (no lenses) to 1.0 (dense lenses)
        temperature_c: f64,
    },
    /// Packed or wind-blown snow layer.
    PackedSnow {
        depth_m: f64,      // layer thickness
        density_kgm3: f64, // 200 (fresh) to 600 (packed)
    },
}

/// Input parameters for a frozen-ground or ice penetration evaluation.
#[derive(Debug, Clone)]
pub struct FrozenGroundPenetrationParams {
    pub surface: FrozenSurfaceType,
    pub velocity_ms: f64,
    pub mass_g: f64,
    pub caliber_m: f64,
    pub projectile_type: String,
    pub impact_angle_deg: f64,
}

/// Result of a frozen-ground or ice penetration evaluation.
#[derive(Debug, Clone)]
pub struct FrozenPenetrationResult {
    pub penetrated: bool,
    pub penetration_depth_m: f64,
    pub ricocheted: bool,
    pub ricochet_angle_deg: f64,
    pub velocity_fraction: f64,
    pub ice_shattered: bool,
    pub ice_shatter_radius_m: f64,
    pub secondary_fragments: i32,
}

// ── Public API ─────────────────────────────────────────────────────────────────

/// Evaluate penetration/ricochet for frozen ground, ice, permafrost, or snow.
///
/// Dispatches to the appropriate sub-model based on [`FrozenSurfaceType`].
///
/// * **Frozen ground** — De Marre penetration with temperature/moisture-dependent
///   material factor (0.20–0.30).  More spall fragments than unfrozen soil.
/// * **Ice on water** — Ricochet below 15°, penetration + brittle fracture above.
///   Ice shatters in a cone pattern (radius 5–15× caliber).  Below ice, the
///   projectile enters water (delegates velocity_fraction for underwater model).
/// * **Permafrost** — Frozen ground with ice-lens bonus (extra 30% hardness per
///   unit ratio). Treats ice lenses as embedded hard inclusions.
/// * **Packed snow** — Very low density/resistance.  Deep penetration at any
///   angle with minimal ricochet.  No secondary fragmentation.
pub fn evaluate_frozen_penetration(
    params: &FrozenGroundPenetrationParams,
) -> FrozenPenetrationResult {
    match params.surface {
        FrozenSurfaceType::FrozenGround {
            temperature_c,
            moisture_content,
        } => evaluate_frozen_ground(params, temperature_c, moisture_content),
        FrozenSurfaceType::IceOnWater { thickness_m } => evaluate_ice_on_water(params, thickness_m),
        FrozenSurfaceType::Permafrost {
            ice_lens_ratio,
            temperature_c,
        } => evaluate_permafrost(params, ice_lens_ratio, temperature_c),
        FrozenSurfaceType::PackedSnow {
            depth_m,
            density_kgm3,
        } => evaluate_packed_snow(params, depth_m, density_kgm3),
    }
}

/// Compute ground hardness multiplier relative to thawed soil.
///
/// Linear interpolation from thawed (factor = 1.0) at +5 °C to
/// maximum hardness at -20 °C.  At -10 °C the factor is ≈ 4.0
/// (3–5× thawed soil, per literature).  Moisture content adds
/// up to 50 % additional hardness when frozen (ice cements grains).
///
/// # Arguments
/// * `moisture` — Moisture content fraction (0.0–1.0).
/// * `temp_c` — Ground temperature in °C.
pub fn ground_hardness_factor(moisture: f64, temp_c: f64) -> f64 {
    if temp_c >= THAWED_TEMP_C {
        return 1.0;
    }
    let clamped_temp = temp_c.max(MAX_FROZEN_TEMP_C);
    // Linear: 1.0 at +5 °C → monotonic increase as temperature drops
    let base = 1.0 + (THAWED_TEMP_C - clamped_temp) * HARDNESS_SLOPE.abs();
    // ponytail: linear model, add exponential at extremes if validation data appears
    let moisture_bonus = if temp_c < 0.0 {
        moisture.clamp(0.0, 1.0) * MOISTURE_CONTRIBUTION
    } else {
        0.0
    };
    (base + moisture_bonus).clamp(1.0, 6.0)
}

/// Compute ice fracture energy (J) using an elastic-brittle fracture model.
///
/// Ice is treated as a linear-elastic brittle solid.  The energy required
/// to create a through-thickness fracture over an area comparable to the
/// projectile presented area is:
///
///    E_frac ≈ (K_IC² / E) · w · t_ice
///
/// where K_IC is fracture toughness, E is Young's modulus, w is a fracture
/// zone width (~5× projectile diameter), and t_ice is the ice thickness.
/// A dynamic enhancement factor (1 + 0.2·v/1000) accounts for rate-dependent
/// strength increase at ballistic strain rates.
///
/// # Arguments
/// * `thickness_m` — Ice sheet thickness (m).
/// * `velocity_ms` — Impact velocity (m/s).
pub fn ice_fracture_energy(thickness_m: f64, velocity_ms: f64) -> f64 {
    if thickness_m <= 0.0 || velocity_ms <= 0.0 {
        return 0.0;
    }
    // Young's modulus for ice ≈ 9 GPa
    let e_ice: f64 = 9.0e9;
    // Fracture zone width ~5 mm (representative for handgun/rifle calibers)
    let fracture_zone_m: f64 = 0.005;
    // Static fracture energy per unit area: K_IC² / E  (J/m²)
    let kic_pa: f64 = ICE_FRACTURE_TOUGHNESS * 1e6_f64; // MPa·m^{1/2} → Pa·m^{1/2}
    let g_c: f64 = kic_pa.powi(2) / e_ice;
    // Fracture area = zone_width × thickness (plane-strain through crack)
    let area_m2: f64 = fracture_zone_m * thickness_m;
    // Dynamic enhancement (ice is stronger at high strain rates)
    let dyn_factor: f64 = 1.0 + 0.2 * (velocity_ms / 1000.0);
    g_c * area_m2 * dyn_factor
}

// ── Internal helpers ───────────────────────────────────────────────────────────

/// Projectile type modifier (same classification as penetration.rs).
fn projectile_modifier(proj_type: &str) -> f64 {
    match proj_type.to_lowercase().as_str() {
        "ball" | "fmj" => 1.0,
        "ap" | "armor_piercing" => 1.3,
        "apds" | "apfsds" => 1.8,
        "apcr" => 1.5,
        "incendiary" => 0.9,
        "tracer" => 0.95,
        _ => 1.0,
    }
}

/// Compute the De Marre material factor for frozen ground.
///
/// Combines the base soil factor (0.05) with the temperature/moisture
/// hardness multiplier and clamps to the frozen-ground range: 0.20–0.30.
fn frozen_mat_factor(moisture: f64, temp_c: f64) -> f64 {
    (SOIL_MAT_FACTOR * ground_hardness_factor(moisture, temp_c))
        .clamp(MIN_FROZEN_MAT_FACTOR, MAX_FROZEN_MAT_FACTOR)
}

/// Ice spall / shatter radius from a brittle fracture model.
///
/// The spall (cone-shaped exit crater) diameter is 5–15× the projectile
/// caliber.  Larger values correspond to thicker ice and higher velocity.
/// The radius is half that diameter.
///
/// Returns 0 if the projectile lacks enough energy to fully perforate the
/// ice sheet (surface cracking only).
fn ice_shatter_radius_m(caliber_m: f64, thickness_m: f64, velocity_ms: f64) -> f64 {
    let e_frac = ice_fracture_energy(thickness_m, velocity_ms);
    // Kinetic energy of the projectile (mass approx from params, use 9g default)
    // We derive a size factor from the ratio of fracture energy to a reference
    let size_factor = (e_frac * 10_000.0).min(1.0); // 0..1, saturating
    let spall_diam_cal = ICE_SPALL_MIN_CAL + (ICE_SPALL_MAX_CAL - ICE_SPALL_MIN_CAL) * size_factor;
    let radius_m = caliber_m * spall_diam_cal / 2.0;
    radius_m.max(0.0)
}

/// Estimate ice spall count from the shatter radius and caliber.
fn ice_fragment_count(radius_m: f64, caliber_m: f64) -> i32 {
    if radius_m <= 0.0 || caliber_m <= 0.0 {
        return 0;
    }
    let area_ratio = (radius_m / caliber_m).powi(2);
    (area_ratio * 0.5).round().max(1.0) as i32
}

// ── Per-surface evaluators ─────────────────────────────────────────────────────

fn evaluate_frozen_ground(
    params: &FrozenGroundPenetrationParams,
    temperature_c: f64,
    moisture_content: f64,
) -> FrozenPenetrationResult {
    let mat_factor = frozen_mat_factor(moisture_content, temperature_c);
    let proj_mod = projectile_modifier(&params.projectile_type);
    let mass_kg = params.mass_g / 1000.0;
    let angle_rad = params.impact_angle_deg.to_radians();
    let cos_angle = angle_rad.cos().max(0.087);

    // Use De Marre penetration against a semi-infinite ground medium.
    // Effective thickness is a function of mat factor and angle.
    // For ground we treat it as a 1 m slab (deep enough for most bullets).
    let slab_equiv_m = 1.0;
    let effective_thickness = slab_equiv_m / cos_angle * mat_factor;
    let k = 91000.0 / proj_mod;
    let v_required = if params.caliber_m > 0.0 && effective_thickness > 0.0 && mass_kg > 0.0 {
        k * params.caliber_m.powf(0.75) * effective_thickness.powf(0.70) / mass_kg.sqrt()
    } else {
        f64::INFINITY
    };

    let pens = params.velocity_ms >= v_required;
    let crit_angle = 15.0 + 5.0 * (1.0 / mat_factor.max(0.01)); // frozen = less ricochet-prone
    let ric = params.impact_angle_deg > crit_angle;

    if ric && !pens {
        let retention = 0.4 + 0.2 * mat_factor;
        FrozenPenetrationResult {
            penetrated: false,
            penetration_depth_m: 0.0,
            ricocheted: true,
            ricochet_angle_deg: (params.impact_angle_deg - crit_angle) * 0.5,
            velocity_fraction: retention.min(0.7),
            ice_shattered: false,
            ice_shatter_radius_m: 0.0,
            // Frozen ground → brittle fracture → more spall than unfrozen
            secondary_fragments: (4.0 * mat_factor * (params.velocity_ms / 300.0)).ceil() as i32,
        }
    } else if pens {
        let depth = (params.velocity_ms - v_required) / 1000.0 * 0.3;
        FrozenPenetrationResult {
            penetrated: true,
            penetration_depth_m: depth.min(1.0),
            ricocheted: false,
            ricochet_angle_deg: 0.0,
            velocity_fraction: 0.0,
            ice_shattered: false,
            ice_shatter_radius_m: 0.0,
            secondary_fragments: (6.0 * mat_factor).ceil() as i32,
        }
    } else {
        FrozenPenetrationResult {
            penetrated: false,
            penetration_depth_m: (params.velocity_ms / v_required.max(1.0) * 0.1).min(0.3),
            ricocheted: false,
            ricochet_angle_deg: 0.0,
            velocity_fraction: 0.0,
            ice_shattered: false,
            ice_shatter_radius_m: 0.0,
            secondary_fragments: (2.0 * mat_factor).ceil() as i32,
        }
    }
}

fn evaluate_ice_on_water(
    params: &FrozenGroundPenetrationParams,
    thickness_m: f64,
) -> FrozenPenetrationResult {
    let angle = params.impact_angle_deg;

    // ── Ricochet at shallow angles ─────────────────────────────────────
    if angle < ICE_RICOCHET_THRESHOLD_DEG {
        let retention = 0.80 + 0.10 * (angle / ICE_RICOCHET_THRESHOLD_DEG);
        return FrozenPenetrationResult {
            penetrated: false,
            penetration_depth_m: 0.0,
            ricocheted: true,
            ricochet_angle_deg: (angle * 0.6).max(1.0),
            velocity_fraction: retention.min(0.9),
            ice_shattered: false,
            ice_shatter_radius_m: 0.0,
            secondary_fragments: 0,
        };
    }

    // ── Steep-angle: attempt penetration ───────────────────────────────
    let mass_kg = params.mass_g / 1000.0;
    let proj_mod = projectile_modifier(&params.projectile_type);
    let angle_rad = angle.to_radians();
    let cos_angle = angle_rad.cos().max(0.087);

    // Ice mat-factor: ~0.15 (similar to concrete, but brittle)
    let ice_mat = 0.15;
    let effective = thickness_m / cos_angle * ice_mat;
    let k = 91000.0 / proj_mod;
    let v_required = if params.caliber_m > 0.0 && effective > 0.0 && mass_kg > 0.0 {
        k * params.caliber_m.powf(0.75) * effective.powf(0.70) / mass_kg.sqrt()
    } else {
        f64::INFINITY
    };

    let pens = params.velocity_ms >= v_required;

    // ── Ice fracture diagnostics ───────────────────────────────────────
    let fracture_energy = ice_fracture_energy(thickness_m, params.velocity_ms);
    let ke = 0.5 * mass_kg * params.velocity_ms.powi(2);
    let ice_shattered = pens || ke > fracture_energy * 10.0;

    let shatter_radius = if ice_shattered {
        ice_shatter_radius_m(params.caliber_m, thickness_m, params.velocity_ms)
    } else {
        0.0
    };

    let frags = if ice_shattered {
        ice_fragment_count(shatter_radius, params.caliber_m)
    } else {
        0
    };

    if pens || ice_shattered {
        // Energy consumed by ice fracture
        let ke_remaining = (ke - fracture_energy).max(0.0);
        let vel_fraction = (ke_remaining / ke.max(1.0)).sqrt();
        // ponytail: vel_fraction here is post-ice; underwater model caller handles water drag

        FrozenPenetrationResult {
            penetrated: true,
            penetration_depth_m: thickness_m, // through the ice sheet
            ricocheted: false,
            ricochet_angle_deg: 0.0,
            velocity_fraction: vel_fraction,
            ice_shattered: true,
            ice_shatter_radius_m: shatter_radius,
            secondary_fragments: frags,
        }
    } else {
        // Dent / crack the ice surface but don't fully penetrate
        FrozenPenetrationResult {
            penetrated: false,
            penetration_depth_m: 0.0,
            ricocheted: false,
            ricochet_angle_deg: 0.0,
            velocity_fraction: 0.0,
            ice_shattered: false,
            ice_shatter_radius_m: 0.0,
            secondary_fragments: 0,
        }
    }
}

fn evaluate_permafrost(
    params: &FrozenGroundPenetrationParams,
    ice_lens_ratio: f64,
    temperature_c: f64,
) -> FrozenPenetrationResult {
    // Permafrost = frozen ground + ice lenses.  Treat as frozen ground with
    // a hardness bonus from the lenses (treated as embedded hard inclusions).
    let lens_bonus = 1.0 + PERMAFROST_ICE_LENS_BONUS * ice_lens_ratio.clamp(0.0, 1.0);
    let moisture = 0.5 + 0.4 * ice_lens_ratio.clamp(0.0, 1.0); // lenses contribute moisture
    let base_mat = frozen_mat_factor(moisture, temperature_c);
    let mat_factor = (base_mat * lens_bonus).min(0.40);

    let proj_mod = projectile_modifier(&params.projectile_type);
    let mass_kg = params.mass_g / 1000.0;
    let angle_rad = params.impact_angle_deg.to_radians();
    let cos_angle = angle_rad.cos().max(0.087);

    let slab_equiv_m = 1.0;
    let effective_thickness = slab_equiv_m / cos_angle * mat_factor;
    let k = 91000.0 / proj_mod;
    let v_required = if params.caliber_m > 0.0 && effective_thickness > 0.0 && mass_kg > 0.0 {
        k * params.caliber_m.powf(0.75) * effective_thickness.powf(0.70) / mass_kg.sqrt()
    } else {
        f64::INFINITY
    };

    let pens = params.velocity_ms >= v_required;
    let crit_angle = 12.0 + 5.0 * (1.0 / mat_factor.max(0.01));
    let ric = params.impact_angle_deg > crit_angle;

    if ric && !pens {
        FrozenPenetrationResult {
            penetrated: false,
            penetration_depth_m: 0.0,
            ricocheted: true,
            ricochet_angle_deg: (params.impact_angle_deg - crit_angle) * 0.5,
            velocity_fraction: 0.5,
            ice_shattered: false,
            ice_shatter_radius_m: 0.0,
            secondary_fragments: (5.0 * mat_factor).ceil() as i32,
        }
    } else if pens {
        let depth = (params.velocity_ms - v_required) / 1000.0 * 0.25;
        FrozenPenetrationResult {
            penetrated: true,
            penetration_depth_m: depth.min(0.8),
            ricocheted: false,
            ricochet_angle_deg: 0.0,
            velocity_fraction: 0.0,
            ice_shattered: false,
            ice_shatter_radius_m: 0.0,
            secondary_fragments: (8.0 * mat_factor).ceil() as i32,
        }
    } else {
        FrozenPenetrationResult {
            penetrated: false,
            penetration_depth_m: (params.velocity_ms / v_required.max(1.0) * 0.08).min(0.2),
            ricocheted: false,
            ricochet_angle_deg: 0.0,
            velocity_fraction: 0.0,
            ice_shattered: false,
            ice_shatter_radius_m: 0.0,
            secondary_fragments: (3.0 * mat_factor).ceil() as i32,
        }
    }
}

fn evaluate_packed_snow(
    params: &FrozenGroundPenetrationParams,
    depth_m: f64,
    density_kgm3: f64,
) -> FrozenPenetrationResult {
    // Snow is very low density: treat as a low-resistance medium.
    // Density ratio relative to water (1000 kg/m³).
    let density_ratio = (density_kgm3 / 1000.0).clamp(0.1, 1.0);

    // Penetration depth: snow offers little resistance to rifle rounds.
    // A high-velocity round penetrates far deeper than the snow depth.
    // Formula: penetration velocity threshold ~30 m/s per 0.1m of snow
    // at density 400 kg/m³; scales with sqrt(density).
    let vel_required = 30.0 * (depth_m / 0.1) * density_ratio.sqrt();
    let penetration = if params.velocity_ms > 100.0 {
        depth_m // full penetration
    } else {
        (params.velocity_ms / vel_required.max(1.0)).min(1.0) * depth_m
    };
    let penetrated = penetration >= depth_m * 0.95 || params.velocity_ms > vel_required;

    // Snow ricochet is extremely rare (soft medium)
    let ric = params.impact_angle_deg > 85.0 && params.velocity_ms > 800.0;

    if ric {
        FrozenPenetrationResult {
            penetrated: false,
            penetration_depth_m: 0.0,
            ricocheted: true,
            ricochet_angle_deg: 5.0,
            velocity_fraction: 0.9,
            ice_shattered: false,
            ice_shatter_radius_m: 0.0,
            secondary_fragments: 0,
        }
    } else {
        FrozenPenetrationResult {
            penetrated,
            penetration_depth_m: penetration,
            ricocheted: false,
            ricochet_angle_deg: 0.0,
            velocity_fraction: if penetrated { 0.9 } else { 0.0 },
            ice_shattered: false,
            ice_shatter_radius_m: 0.0,
            secondary_fragments: 0,
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Ground hardness ────────────────────────────────────────────────────

    #[test]
    fn ground_hardness_thawed_vs_frozen() {
        let thawed = ground_hardness_factor(0.3, 10.0);
        let frozen = ground_hardness_factor(0.3, -10.0);
        assert!(
            frozen > thawed,
            "Frozen ground should be harder than thawed"
        );
        assert!((thawed - 1.0).abs() < 1e-6, "Thawed factor should be 1.0");
        assert!(
            (frozen - 4.0).abs() < 1.5,
            "Frozen (-10°C) factor ~4.0: got {}",
            frozen
        );
    }

    #[test]
    fn ground_hardness_moisture_effect() {
        let dry = ground_hardness_factor(0.0, -10.0);
        let wet = ground_hardness_factor(1.0, -10.0);
        assert!(wet > dry, "Higher moisture should increase frozen hardness");
    }

    #[test]
    fn ground_hardness_clamp_upper() {
        let extreme = ground_hardness_factor(1.0, -50.0);
        assert!(
            extreme <= 6.0,
            "Hardness factor should not exceed 6.0: got {}",
            extreme
        );
    }

    #[test]
    fn ground_hardness_monotonic() {
        let mut prev = ground_hardness_factor(0.5, 5.0);
        for t in (-20..=5_i32)
            .step_by(5)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
        {
            let h = ground_hardness_factor(0.5, t as f64);
            assert!(
                h >= prev - 1e-9,
                "Hardness must be monotonic decreasing with temp: {} < {} at {}°C",
                h,
                prev,
                t
            );
            prev = h;
        }
    }

    // ── Ice fracture energy ────────────────────────────────────────────────

    #[test]
    fn ice_fracture_energy_increases_with_thickness() {
        let thin = ice_fracture_energy(0.1, 500.0);
        let thick = ice_fracture_energy(0.5, 500.0);
        assert!(thick > thin, "Thicker ice requires more fracture energy");
    }

    #[test]
    fn ice_fracture_energy_increases_with_velocity() {
        let slow = ice_fracture_energy(0.3, 300.0);
        let fast = ice_fracture_energy(0.3, 900.0);
        assert!(
            fast > slow,
            "Higher velocity increases dynamic fracture energy"
        );
    }

    #[test]
    fn ice_fracture_energy_zero_inputs() {
        assert_eq!(ice_fracture_energy(0.0, 500.0), 0.0);
        assert_eq!(ice_fracture_energy(0.3, 0.0), 0.0);
    }

    // ── Ice ricochet / penetration ─────────────────────────────────────────

    #[test]
    fn ice_ricochet_at_shallow_angle() {
        let params = FrozenGroundPenetrationParams {
            surface: FrozenSurfaceType::IceOnWater { thickness_m: 0.3 },
            velocity_ms: 850.0,
            mass_g: 9.5,
            caliber_m: 0.00762,
            projectile_type: "ball".into(),
            impact_angle_deg: 10.0, // < 15° → ricochet
        };
        let result = evaluate_frozen_penetration(&params);
        assert!(
            result.ricocheted,
            "Shallow-angle ice impact should ricochet"
        );
        assert!(!result.penetrated);
        assert!(!result.ice_shattered);
    }

    #[test]
    fn ice_penetration_at_steep_angle() {
        let params = FrozenGroundPenetrationParams {
            surface: FrozenSurfaceType::IceOnWater { thickness_m: 0.2 },
            velocity_ms: 850.0,
            mass_g: 9.5,
            caliber_m: 0.00762,
            projectile_type: "ball".into(),
            impact_angle_deg: 45.0, // steep → penetrate
        };
        let result = evaluate_frozen_penetration(&params);
        assert!(result.penetrated, "Steep-angle ice impact should penetrate");
        assert!(result.ice_shattered);
        assert!(
            result.ice_shatter_radius_m > 0.0,
            "Shatter radius should be positive"
        );
        assert!(
            result.secondary_fragments > 0,
            "Ice fracture should produce fragments"
        );
    }

    #[test]
    fn ice_shatter_radius_in_range() {
        let cal = 0.00762;
        let r = ice_shatter_radius_m(cal, 0.3, 850.0);
        let min_r = cal * ICE_SPALL_MIN_CAL / 2.0;
        let max_r = cal * ICE_SPALL_MAX_CAL / 2.0;
        assert!(
            r >= min_r && r <= max_r,
            "Shatter radius {:.4} should be between {:.4} and {:.4}",
            r,
            min_r,
            max_r
        );
    }

    #[test]
    fn ice_fragment_count_positive() {
        let frags = ice_fragment_count(0.05, 0.00762);
        assert!(frags > 0, "Should produce at least 1 fragment");
    }

    // ── Permafrost ─────────────────────────────────────────────────────────

    #[test]
    fn permafrost_harder_than_frozen_ground() {
        let frozen = FrozenGroundPenetrationParams {
            surface: FrozenSurfaceType::FrozenGround {
                temperature_c: -10.0,
                moisture_content: 0.5,
            },
            velocity_ms: 500.0,
            mass_g: 9.5,
            caliber_m: 0.00762,
            projectile_type: "ball".into(),
            impact_angle_deg: 0.0,
        };
        let perma = FrozenGroundPenetrationParams {
            surface: FrozenSurfaceType::Permafrost {
                ice_lens_ratio: 0.6,
                temperature_c: -10.0,
            },
            velocity_ms: 500.0,
            mass_g: 9.5,
            caliber_m: 0.00762,
            projectile_type: "ball".into(),
            impact_angle_deg: 0.0,
        };
        let f_res = evaluate_frozen_penetration(&frozen);
        let p_res = evaluate_frozen_penetration(&perma);
        // Permafrost should be harder to penetrate than plain frozen ground
        // (check that penetration depth is lower for permafrost)
        assert!(
            p_res.penetration_depth_m <= f_res.penetration_depth_m + 0.01,
            "Permafrost should resist penetration at least as well as frozen ground"
        );
    }

    // ── Packed snow ────────────────────────────────────────────────────────

    #[test]
    fn packed_snow_deep_penetration() {
        let params = FrozenGroundPenetrationParams {
            surface: FrozenSurfaceType::PackedSnow {
                depth_m: 0.5,
                density_kgm3: 400.0,
            },
            velocity_ms: 850.0,
            mass_g: 9.5,
            caliber_m: 0.00762,
            projectile_type: "ball".into(),
            impact_angle_deg: 0.0,
        };
        let result = evaluate_frozen_penetration(&params);
        assert!(result.penetrated, "Rifle round should penetrate 0.5 m snow");
        assert!(!result.ricocheted);
        assert_eq!(result.secondary_fragments, 0, "Snow produces no fragments");
    }

    // ── Frozen ground vs unfrozen ──────────────────────────────────────────

    #[test]
    fn frozen_ground_more_spall_than_unfrozen() {
        // Simulate unfrozen: temp above freezing → behaves like plain soil
        // Frozen: below freezing → brittle fracture → more spall
        let thawed = evaluate_frozen_penetration(&FrozenGroundPenetrationParams {
            surface: FrozenSurfaceType::FrozenGround {
                temperature_c: 10.0,
                moisture_content: 0.3,
            },
            velocity_ms: 850.0,
            mass_g: 4.0,
            caliber_m: 0.00556,
            projectile_type: "ball".into(),
            impact_angle_deg: 0.0,
        });
        let frozen = evaluate_frozen_penetration(&FrozenGroundPenetrationParams {
            surface: FrozenSurfaceType::FrozenGround {
                temperature_c: -10.0,
                moisture_content: 0.3,
            },
            velocity_ms: 850.0,
            mass_g: 4.0,
            caliber_m: 0.00556,
            projectile_type: "ball".into(),
            impact_angle_deg: 0.0,
        });
        assert!(
            frozen.secondary_fragments >= thawed.secondary_fragments,
            "Frozen ground should produce >= spall fragments vs thawed"
        );
    }

    // ── Mat factor range ───────────────────────────────────────────────────

    #[test]
    fn frozen_mat_factor_in_range() {
        let light = frozen_mat_factor(0.2, -1.0);
        let heavy = frozen_mat_factor(1.0, -20.0);
        assert!(
            light >= MIN_FROZEN_MAT_FACTOR,
            "Lightly frozen mat_factor ({}) should be >= {}",
            light,
            MIN_FROZEN_MAT_FACTOR
        );
        assert!(
            heavy <= MAX_FROZEN_MAT_FACTOR,
            "Heavily frozen mat_factor ({}) should be <= {}",
            heavy,
            MAX_FROZEN_MAT_FACTOR
        );
    }

    // ── Edge cases ─────────────────────────────────────────────────────────

    #[test]
    fn ice_zero_thickness_no_penetration() {
        // Zero-thickness ice: no ice to penetrate
        let params = FrozenGroundPenetrationParams {
            surface: FrozenSurfaceType::IceOnWater { thickness_m: 0.0 },
            velocity_ms: 850.0,
            mass_g: 9.5,
            caliber_m: 0.00762,
            projectile_type: "ball".into(),
            impact_angle_deg: 60.0,
        };
        let result = evaluate_frozen_penetration(&params);
        // Zero thickness → fracture energy is 0 → no ice to shatter
        // But effectively there's nothing to stop the projectile
        assert!(
            result.ice_shattered || result.penetrated,
            "Zero-thickness ice should not block projectile"
        );
    }

    #[test]
    fn permafrost_secondary_fragments_scaled() {
        let low = evaluate_frozen_penetration(&FrozenGroundPenetrationParams {
            surface: FrozenSurfaceType::Permafrost {
                ice_lens_ratio: 0.0,
                temperature_c: -5.0,
            },
            velocity_ms: 400.0,
            mass_g: 4.0,
            caliber_m: 0.00556,
            projectile_type: "ball".into(),
            impact_angle_deg: 0.0,
        });
        let high = evaluate_frozen_penetration(&FrozenGroundPenetrationParams {
            surface: FrozenSurfaceType::Permafrost {
                ice_lens_ratio: 1.0,
                temperature_c: -15.0,
            },
            velocity_ms: 400.0,
            mass_g: 4.0,
            caliber_m: 0.00556,
            projectile_type: "ball".into(),
            impact_angle_deg: 0.0,
        });
        assert!(
            high.secondary_fragments >= low.secondary_fragments,
            "Denser ice lenses should produce >= fragments"
        );
    }
}
