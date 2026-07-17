// ABE — Body Armor Penetration & Trauma Model
//
// Evaluates projectile performance against multi-layer body armor
// (ceramic face + composite backing) per NIJ 0101.06 standards.
//
// Physics:
//   1. Ceramic face erodes / shatters the projectile, absorbing energy
//      via cone-crack fracture. Material factor ~2.5–3.5× RHA per mm.
//   2. Composite backing (UHMWPE / Kevlar) catches fragments and
//      absorbs remaining kinetic energy. Material factor ~0.25–0.6× RHA.
//   3. Backface deformation (BFD) models blunt trauma — exceeding
//      44 mm (NIJ limit) constitutes armor defeat even without penetration.
//   4. Multi-hit degradation: ceramic cracks on the first impact,
//      reducing effectiveness for subsequent hits to the same tile.
//
// References:
//   - NIJ Standard 0101.06 (Ballistic Resistance of Body Armor)
//   - NIJ Standard 0108.01 (Ballistic Resistant Protective Materials)
//   - Hetherington & Rajendran (1998) — ceramic/composite armour
//   - Florence (1969) — two-layer ceramic/metal impact model
//   - STANAG 2920 — ballistic test methodology
//
// This module uses penetration::material_factor() for base material
// resistance values.

use std::collections::HashMap;

use crate::penetration;

// ── NIJ 0101.06 reference threat velocities (m/s) ─────────────────────────────

const NIJ_IIIA_9MM_V: f64 = 436.0; // 9 mm FMJ RN @ 436 ± 10 m/s
const NIJ_IIIA_44_V: f64 = 436.0; // .44 Mag SJHP @ 436 ± 10 m/s
const NIJ_III_M80_V: f64 = 847.0; // 7.62×51 mm M80 ball @ 847 ± 10 m/s
const NIJ_IV_AP_V: f64 = 878.0; // .30-06 M2 AP @ 878 ± 10 m/s

/// Backface deformation limit per NIJ 0101.06 (44 mm).
const BFD_LIMIT_MM: f64 = 44.0;

// ── Configuration ─────────────────────────────────────────────────────────────

/// Configuration for a specific body armor package.
///
/// Built-in variants (LevelIIIA, LevelIII, LevelIV, ESAPI, SAPI,
/// GenericCeramicComposite) have predefined parameters. Use `Custom` to
/// specify arbitrary layer geometry and materials.
#[derive(Debug, Clone)]
pub enum BodyArmorConfiguration {
    /// NIJ Level IIIA (soft armor): stops 9 mm FMJ & .44 Mag SJHP.
    LevelIIIA,
    /// NIJ Level III (hard plate): stops 7.62×51 mm M80 ball.
    LevelIII,
    /// NIJ Level IV (hard plate): stops .30-06 M2 AP.
    LevelIV,
    /// ESAPI — US military Enhanced Small Arms Protective Insert.
    ESAPI,
    /// SAPI — US military standard.
    SAPI,
    /// Generic ceramic face + composite backing plate.
    GenericCeramicComposite,
    /// Custom layer geometry and materials.
    Custom(BodyArmorCustomConfig),
}

/// Detailed custom body armor parameters.
#[derive(Debug, Clone)]
pub struct BodyArmorCustomConfig {
    /// Ceramic face thickness in mm.
    pub ceramic_thickness_mm: f64,
    /// Composite backing thickness in mm.
    pub backing_thickness_mm: f64,
    /// Ceramic material identifier (see [`penetration::material_factor`]).
    pub ceramic_material: String,
    /// Backing material identifier (e.g. "uhmwpe", "composite_kevlar").
    pub backing_material: String,
    /// Total areal density in kg/m².
    pub areal_density_kgm2: f64,
    /// Plate dimensions (width, height) in mm.
    pub plate_dimensions_mm: (f64, f64),
    /// Backface deformation limit in mm (NIJ default: 44 mm).
    pub trauma_backface_limit_mm: f64,
    /// Number of hits the plate can survive at its rated threat level.
    pub multi_hit_capability: i32,
}

// ── Projectile + evaluation parameters ─────────────────────────────────────────

/// Input parameters for a body armor penetration evaluation.
#[derive(Debug, Clone)]
pub struct BodyArmorParams {
    /// Projectile calibre in mm.
    pub projectile_caliber_mm: f64,
    /// Projectile mass in grams.
    pub projectile_mass_g: f64,
    /// Impact velocity in m/s.
    pub impact_velocity_ms: f64,
    /// Projectile type identifier (e.g. "ball", "ap", "fmj").
    pub projectile_type: String,
    /// Body armor package configuration.
    pub armor_package: BodyArmorConfiguration,
}

/// Result of a body armor penetration evaluation.
#[derive(Debug, Clone)]
pub struct BodyArmorResult {
    /// Whether the projectile fully perforated the armor.
    pub penetrated: bool,
    /// Residual velocity after perforation in m/s (0 if stopped).
    pub residual_velocity_ms: f64,
    /// Backface deformation in mm (0 if penetrated).
    pub backface_deformation_mm: f64,
    /// Whether the ceramic face cracked (even if not fully penetrated).
    pub plate_cracked: bool,
    /// Estimated blunt trauma depth behind the plate in mm.
    pub estimated_trauma_depth_mm: f64,
    /// Kinetic energy deposited into the armor in joules.
    pub energy_deposited_j: f64,
    /// Whether the armor is functionally defeated (perforated or
    /// BFD exceeds limit, or multi-hit capacity exhausted).
    pub armor_defeated: bool,
    /// Total number of shots at this energy level required to defeat
    /// a fresh plate. 1 = current shot defeats the armor.
    pub shots_to_defeat: i32,
}

// ── Built-in armor configurations ─────────────────────────────────────────────

impl BodyArmorConfiguration {
    /// Resolve a built-in or custom configuration to its detailed parameters.
    pub fn resolve(&self) -> BodyArmorCustomConfig {
        match self {
            BodyArmorConfiguration::LevelIIIA => BodyArmorCustomConfig {
                // Soft armor: no ceramic face, thick aramid backing
                ceramic_thickness_mm: 0.0,
                backing_thickness_mm: 10.0,
                ceramic_material: "air".to_string(),
                backing_material: "composite_kevlar".to_string(),
                areal_density_kgm2: 5.3,
                plate_dimensions_mm: (250.0, 300.0),
                trauma_backface_limit_mm: BFD_LIMIT_MM,
                multi_hit_capability: 20,
            },
            BodyArmorConfiguration::LevelIII => BodyArmorCustomConfig {
                ceramic_thickness_mm: 8.0,
                backing_thickness_mm: 12.0,
                ceramic_material: "ceramic_sic".to_string(),
                backing_material: "uhmwpe".to_string(),
                areal_density_kgm2: 22.0,
                plate_dimensions_mm: (250.0, 300.0),
                trauma_backface_limit_mm: BFD_LIMIT_MM,
                multi_hit_capability: 6,
            },
            BodyArmorConfiguration::LevelIV => BodyArmorCustomConfig {
                ceramic_thickness_mm: 10.0,
                backing_thickness_mm: 14.0,
                ceramic_material: "ceramic_b4c".to_string(),
                backing_material: "uhmwpe".to_string(),
                areal_density_kgm2: 28.0,
                plate_dimensions_mm: (250.0, 300.0),
                trauma_backface_limit_mm: BFD_LIMIT_MM,
                multi_hit_capability: 5,
            },
            BodyArmorConfiguration::ESAPI => BodyArmorCustomConfig {
                ceramic_thickness_mm: 9.0,
                backing_thickness_mm: 16.0,
                ceramic_material: "ceramic_b4c".to_string(),
                backing_material: "uhmwpe".to_string(),
                areal_density_kgm2: 29.0,
                plate_dimensions_mm: (254.0, 330.0),
                trauma_backface_limit_mm: BFD_LIMIT_MM,
                multi_hit_capability: 4,
            },
            BodyArmorConfiguration::SAPI => BodyArmorCustomConfig {
                ceramic_thickness_mm: 7.0,
                backing_thickness_mm: 14.0,
                ceramic_material: "ceramic_al2o3".to_string(),
                backing_material: "composite_kevlar".to_string(),
                areal_density_kgm2: 25.0,
                plate_dimensions_mm: (254.0, 330.0),
                trauma_backface_limit_mm: BFD_LIMIT_MM,
                multi_hit_capability: 5,
            },
            BodyArmorConfiguration::GenericCeramicComposite => BodyArmorCustomConfig {
                ceramic_thickness_mm: 8.0,
                backing_thickness_mm: 10.0,
                ceramic_material: "ceramic_al2o3".to_string(),
                backing_material: "uhmwpe".to_string(),
                areal_density_kgm2: 20.0,
                plate_dimensions_mm: (250.0, 300.0),
                trauma_backface_limit_mm: BFD_LIMIT_MM,
                multi_hit_capability: 4,
            },
            BodyArmorConfiguration::Custom(cfg) => cfg.clone(),
        }
    }
}

// ── NIJ threat-level velocity thresholds ──────────────────────────────────────

/// Return a map of NIJ threat level → minimum velocity (m/s) for which this
/// armor configuration is rated.
///
/// The returned velocities are the *test* velocities from NIJ 0101.06. A
/// projectile at or below these values should be stopped by undamaged armor
/// of that level.
///
/// # Keys
/// - `"9mm_fmj"` — 9 mm FMJ RN (Level IIIA test round)
/// - `"44_mag_sjhp"` — .44 Mag SJHP (Level IIIA test round)
/// - `"m80_ball"` — 7.62×51 mm M80 ball (Level III test round)
/// - `"m2_ap"` — .30-06 M2 AP (Level IV test round)
pub fn nij_threshold(config: &BodyArmorCustomConfig) -> HashMap<String, f64> {
    use penetration::material_factor;

    let ceramic_factor = material_factor(&config.ceramic_material);
    let backing_factor = material_factor(&config.backing_material);

    // Combined RHA-equivalent thickness in mm
    let ceramic_rha_mm = config.ceramic_thickness_mm * ceramic_factor;
    let backing_rha_mm = config.backing_thickness_mm * backing_factor;
    let total_rha_mm = ceramic_rha_mm + backing_rha_mm;

    let mut map = HashMap::new();

    // De Marre: V_req = k * D^0.75 * T^0.7 / M^0.5
    // k ~ 91000 calibrated for steel RHA (matches penetration.rs)
    // We compute threshold velocity for each standard test round
    let k = 91000.0;

    // 9 mm FMJ RN (124 gr, 8.0 g, 9.05 mm dia)
    let v_9mm =
        k * (0.00905_f64).powf(0.75) * (total_rha_mm / 1000.0).powf(0.7) / (0.0080_f64).sqrt();
    map.insert("9mm_fmj".to_string(), v_9mm);

    // .44 Mag SJHP (240 gr, 15.6 g, 10.9 mm dia)
    let v_44 =
        k * (0.0109_f64).powf(0.75) * (total_rha_mm / 1000.0).powf(0.7) / (0.0156_f64).sqrt();
    map.insert("44_mag_sjhp".to_string(), v_44);

    // 7.62×51 mm M80 ball (147 gr, 9.5 g, 7.62 mm dia)
    let v_m80 =
        k * (0.00762_f64).powf(0.75) * (total_rha_mm / 1000.0).powf(0.7) / (0.0095_f64).sqrt();
    map.insert("m80_ball".to_string(), v_m80);

    // .30-06 M2 AP (166 gr, 10.8 g, 7.62 mm dia, AP projectile)
    let v_ap =
        k * (0.00762_f64).powf(0.75) * (total_rha_mm / 1000.0).powf(0.7) / (0.0108_f64).sqrt();
    map.insert("m2_ap".to_string(), v_ap);

    map
}

// ── Ceramic spall efficiency ──────────────────────────────────────────────────

/// Compute ceramic fracture/efficiency factor based on ceramic type and
/// thickness-to-diameter ratio (t/d).
///
/// The efficiency follows a Gaussian-like curve peaked at t/d ≈ 0.4–0.6
/// (where cone-crack formation optimally erodes the projectile). Very thin
/// ceramic (t/d < 0.2) fails to fully disrupt the projectile; very thick
/// ceramic (t/d > 1.5) may shatter prematurely.
///
/// # Arguments
/// * `ceramic_type` — Ceramic material identifier (e.g. `"ceramic_b4c"`).
/// * `t_over_d` — Ceramic thickness divided by projectile diameter (t/d).
///
/// # Returns
/// Efficiency multiplier [0.0, 1.0] applied to the ceramic's effective
/// material factor.
pub fn ceramic_spall_efficiency(ceramic_type: &str, t_over_d: f64) -> f64 {
    // Material-dependent peak t/d ratio (brittler ceramics peak higher)
    let mat = ceramic_type.to_lowercase();
    let peak_td = if mat.contains("b4c") || mat.contains("boron_carbide") {
        0.45
    } else if mat.contains("sic") || mat.contains("silicon_carbide") {
        0.50
    } else if mat.contains("al2o3") || mat.contains("ad90") || mat.contains("ad95") {
        0.55
    } else {
        0.50
    };
    // ponytail: width=0.7 gives ~73% efficiency at t/d=1.0, ~25% at t/d=1.6
    // wider = more conservative falloff for thick ceramic
    let width: f64 = 0.7;

    if t_over_d <= 0.0 {
        return 0.0;
    }

    // Gaussian efficiency curve
    let eff = (-((t_over_d - peak_td).powi(2)) / (2.0 * width.powi(2))).exp();

    // Scale such that peak ≈ 0.95 (ceramic never 100 % efficient)
    eff * 0.95
}

// ── Backface deformation ──────────────────────────────────────────────────────

/// Compute backface deformation (BFD) in mm from the kinetic energy
/// transferred to the armor backing.
///
/// # Formula
/// ```text
/// BFD = k * E_deposited / (ρ_a * cos(θ))
/// ```
/// where:
/// - `E_deposited` — energy in joules absorbed by the backing layer
/// - `ρ_a` — backing areal density in kg/m²
/// - `θ` — impact angle from surface normal (degrees)
/// - `k` — empirical calibration constant (~0.32 mm·kg/J·m² for
///   UHMWPE/Kevlar-backed plates)
///
/// # Arguments
/// * `energy_deposited_j` — Kinetic energy (J) transferred to the backing.
/// * `backing_areal_density` — Backing areal density (kg/m²).
/// * `angle_deg` — Impact angle from normal (0° = perpendicular).
///
/// # Returns
/// Backface deformation in mm.
pub fn backface_deformation(
    energy_deposited_j: f64,
    backing_areal_density: f64,
    angle_deg: f64,
) -> f64 {
    if energy_deposited_j <= 0.0 || backing_areal_density <= 0.0 {
        return 0.0;
    }
    let cos_angle = angle_deg.to_radians().cos().max(0.087); // clamp ~85°
    let k = 0.32; // mm·kg/J·m² — calibrated for typical composite backings
    k * energy_deposited_j / (backing_areal_density * cos_angle)
}

// ── Main evaluation ───────────────────────────────────────────────────────────

/// Evaluate a projectile impact against a body armor plate.
///
/// Models the armor as a **combined** RHA-equivalent system (ceramic face +
/// composite backing act together, not sequentially):
///
/// 1. **Combined RHAe** — ceramic thickness × material factor × spall
///    efficiency + backing thickness × material factor. A single De Marre
///    threshold velocity is computed from this total.
///
/// 2. **Penetration** — if impact velocity ≥ threshold, the projectile
///    perforates the plate. Residual velocity follows `√(V² − V_req²)`.
///
/// 3. **Blunt trauma (BFD)** — if stopped, the energy deposited into the
///    backing layer (pro-rata by layer RHAe fraction) drives backface
///    deformation. Exceeding the NIJ 44 mm limit constitutes armor defeat.
///
/// 4. **Multi-hit** — each impact at ≥80 % of the threshold velocity
///    cracks the ceramic and consumes one "life" from the armour's
///    multi-hit capability. Lower-energy hits cause negligible degradation.
///
/// # Arguments
/// * `params` — Projectile and armor configuration.
///
/// # Returns
/// A [`BodyArmorResult`] describing penetration, trauma, and
/// multi-hit status.
pub fn evaluate_body_armor(params: &BodyArmorParams) -> BodyArmorResult {
    let config = params.armor_package.resolve();
    let mass_kg = params.projectile_mass_g / 1000.0;
    let cal_m = params.projectile_caliber_mm / 1000.0;
    let energy_j = 0.5 * mass_kg * params.impact_velocity_ms.powi(2);

    let ceramic_mat_factor = penetration::material_factor(&config.ceramic_material);
    let backing_mat_factor = penetration::material_factor(&config.backing_material);

    // ── Combined RHA-equivalent thickness ────────────────────────────────
    // Ceramic spall efficiency vs t/d — the ceramic erodes the projectile
    // via cone-crack fracture; efficiency peaks at t/d ≈ 0.4–0.6.
    let t_over_d = if params.projectile_caliber_mm > 0.0 {
        config.ceramic_thickness_mm / params.projectile_caliber_mm
    } else {
        0.0
    };
    let spall_eff = ceramic_spall_efficiency(&config.ceramic_material, t_over_d);

    // Both layers together form a single penetration barrier
    let ceramic_rha_m = config.ceramic_thickness_mm / 1000.0 * ceramic_mat_factor * spall_eff;
    let backing_rha_m = config.backing_thickness_mm / 1000.0 * backing_mat_factor;
    let total_rha_m = ceramic_rha_m + backing_rha_m;

    // ── De Marre threshold for the total armor package ──────────────────
    // V_req = k * D^0.75 * T^0.7 / M^0.5  (k ≈ 91000 for RHA steel)
    let k = 91000.0;
    let v_threshold = if total_rha_m > 0.0 && cal_m > 0.0 && mass_kg > 0.0 {
        k * cal_m.powf(0.75) * total_rha_m.powf(0.70) / mass_kg.sqrt()
    } else {
        f64::INFINITY
    };

    let penetrated = params.impact_velocity_ms >= v_threshold;

    // ── Residual velocity and energy deposition ─────────────────────────
    let residual_velocity_ms = if penetrated {
        let vr_sq = params.impact_velocity_ms.powi(2) - v_threshold.powi(2);
        if vr_sq > 0.0 {
            vr_sq.sqrt()
        } else {
            0.0
        }
    } else {
        0.0
    };

    let energy_deposited_j = energy_j - 0.5 * mass_kg * residual_velocity_ms.powi(2);

    // ── Backface deformation (blunt trauma) ─────────────────────────────
    // Only meaningful when the projectile stops inside the armor.
    // The backing absorbs a fraction of the deposited energy proportional
    // to its share of the total RHAe.
    let bfd_mm = if !penetrated && energy_deposited_j > 0.0 {
        let backing_fraction = if total_rha_m > 0.0 {
            backing_rha_m / total_rha_m
        } else {
            1.0
        };
        let bfd_energy = energy_deposited_j * backing_fraction;

        // Hard plate (ceramic present) vs soft armor (all backing)
        let is_hard_plate = ceramic_rha_m > 0.0;
        if is_hard_plate {
            // k ≈ 0.20 for hard ceramic-backed plates (stiffer, less localised)
            0.20 * bfd_energy / (config.areal_density_kgm2 * 1.0)
        } else {
            // k ≈ 0.12 for soft armor (Kevlar flex distributes energy)
            0.12 * bfd_energy / (config.areal_density_kgm2 * 1.0)
        }
    } else {
        0.0
    };

    // ── Estimated trauma depth ──────────────────────────────────────────
    // Behind the plate: ~70 % of BFD for blunt trauma; for perforations,
    // scale by residual energy per areal density.
    let estimated_trauma_depth_mm = if penetrated {
        0.3 * energy_deposited_j / (config.areal_density_kgm2.max(1.0))
    } else {
        bfd_mm * 0.7
    };

    // ── Multi-hit degradation ────────────────────────────────────────────
    // High-energy hits (≥80 % of threshold) crack the ceramic and use up
    // one "life" from the plate's multi-hit capability.
    let plate_cracked = !penetrated && energy_deposited_j > 0.0;

    let hit_strength = if v_threshold.is_finite() && v_threshold > 0.0 {
        params.impact_velocity_ms / v_threshold
    } else {
        0.0
    };

    let shots_to_defeat = if penetrated {
        1
    } else if hit_strength >= 0.8 {
        // Consumes a life — remaining shots = multi_hit_capability − 1
        config.multi_hit_capability
    } else if hit_strength >= 0.5 {
        // Moderate hit — consumes a life but plate survives longer
        config.multi_hit_capability
    } else {
        // Weak hit — negligible degradation
        (config.multi_hit_capability * 2).max(4)
    };

    let bfd_exceeded = bfd_mm > config.trauma_backface_limit_mm;
    let armor_defeated = penetrated || bfd_exceeded;

    BodyArmorResult {
        penetrated,
        residual_velocity_ms,
        backface_deformation_mm: bfd_mm,
        plate_cracked,
        estimated_trauma_depth_mm,
        energy_deposited_j,
        armor_defeated,
        shots_to_defeat,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: level IIIA soft armor vs 9 mm FMJ at 436 m/s (NIJ test spec).
    #[test]
    fn level_iiia_stops_9mm_fmj() {
        let params = BodyArmorParams {
            projectile_caliber_mm: 9.05,
            projectile_mass_g: 8.0,
            impact_velocity_ms: NIJ_IIIA_9MM_V,
            projectile_type: "fmj".to_string(),
            armor_package: BodyArmorConfiguration::LevelIIIA,
        };
        let result = evaluate_body_armor(&params);
        assert!(
            !result.penetrated,
            "Level IIIA must stop 9 mm FMJ @ 436 m/s"
        );
        assert!(
            result.backface_deformation_mm < BFD_LIMIT_MM,
            "BFD ({:.1} mm) must be under 44 mm limit",
            result.backface_deformation_mm
        );
    }

    /// Level III hard plate stops 7.62×51 mm M80 ball at 847 m/s.
    #[test]
    fn level_iii_stops_m80_ball() {
        let params = BodyArmorParams {
            projectile_caliber_mm: 7.62,
            projectile_mass_g: 9.5,
            impact_velocity_ms: NIJ_III_M80_V,
            projectile_type: "ball".to_string(),
            armor_package: BodyArmorConfiguration::LevelIII,
        };
        let result = evaluate_body_armor(&params);
        assert!(!result.penetrated, "Level III must stop M80 ball @ 847 m/s");
        assert!(
            result.backface_deformation_mm < BFD_LIMIT_MM,
            "BFD ({:.1} mm) must be under 44 mm limit",
            result.backface_deformation_mm
        );
    }

    /// Level IV hard plate stops .30-06 M2 AP at 878 m/s.
    #[test]
    fn level_iv_stops_m2_ap() {
        let params = BodyArmorParams {
            projectile_caliber_mm: 7.62,
            projectile_mass_g: 10.8,
            impact_velocity_ms: NIJ_IV_AP_V,
            projectile_type: "ap".to_string(),
            armor_package: BodyArmorConfiguration::LevelIV,
        };
        let result = evaluate_body_armor(&params);
        assert!(!result.penetrated, "Level IV must stop M2 AP @ 878 m/s");
        assert!(
            result.backface_deformation_mm < BFD_LIMIT_MM,
            "BFD ({:.1} mm) must be under 44 mm limit",
            result.backface_deformation_mm
        );
    }

    /// ESAPI plate stops M855A1 (5.56 mm, ~945 m/s).
    #[test]
    fn esapi_stops_m855a1() {
        let params = BodyArmorParams {
            projectile_caliber_mm: 5.56,
            projectile_mass_g: 4.0,
            impact_velocity_ms: 945.0,
            projectile_type: "ball".to_string(),
            armor_package: BodyArmorConfiguration::ESAPI,
        };
        let result = evaluate_body_armor(&params);
        assert!(!result.penetrated, "ESAPI must stop M855A1 @ 945 m/s");
        assert!(
            result.backface_deformation_mm < BFD_LIMIT_MM,
            "BFD ({:.1} mm) must be under 44 mm limit",
            result.backface_deformation_mm
        );
    }

    /// Ceramic spall efficiency degrades between first and subsequent hits.
    #[test]
    fn multi_hit_degradation() {
        // First hit cracks ceramic, reducing multi-hit capability
        let params = BodyArmorParams {
            projectile_caliber_mm: 7.62,
            projectile_mass_g: 9.5,
            impact_velocity_ms: 847.0,
            projectile_type: "ball".to_string(),
            armor_package: BodyArmorConfiguration::LevelIII,
        };
        let result = evaluate_body_armor(&params);
        assert!(!result.penetrated, "First hit should not penetrate");

        // Level III has multi_hit_capability = 6, shots_to_defeat should be
        // at least 2 (indicating this hit uses a "life")
        assert!(
            result.shots_to_defeat >= 2,
            "Fresh plate should survive multiple hits: shots_to_defeat={}",
            result.shots_to_defeat
        );
        // Plate should crack from the first impact
        assert!(
            result.plate_cracked,
            "Ceramic should crack on first significant impact"
        );
    }

    /// The evaluation is deterministic: same inputs → same outputs.
    #[test]
    fn deterministic_output() {
        let params = BodyArmorParams {
            projectile_caliber_mm: 7.62,
            projectile_mass_g: 9.5,
            impact_velocity_ms: 847.0,
            projectile_type: "ball".to_string(),
            armor_package: BodyArmorConfiguration::LevelIII,
        };
        let a = evaluate_body_armor(&params);
        let b = evaluate_body_armor(&params);
        assert_eq!(a.penetrated, b.penetrated);
        assert!((a.residual_velocity_ms - b.residual_velocity_ms).abs() < 1e-12);
        assert!((a.backface_deformation_mm - b.backface_deformation_mm).abs() < 1e-12);
        assert_eq!(a.plate_cracked, b.plate_cracked);
        assert_eq!(a.shots_to_defeat, b.shots_to_defeat);
    }

    /// NIJ thresholds map contains all four standard threat levels.
    #[test]
    fn nij_threshold_keys() {
        let config = BodyArmorConfiguration::LevelIII.resolve();
        let thresholds = nij_threshold(&config);
        assert!(thresholds.contains_key("9mm_fmj"));
        assert!(thresholds.contains_key("44_mag_sjhp"));
        assert!(thresholds.contains_key("m80_ball"));
        assert!(thresholds.contains_key("m2_ap"));
    }

    /// Custom configuration works the same as built-in.
    #[test]
    fn custom_config_equivalent_to_level_iii() {
        let custom = BodyArmorConfiguration::Custom(BodyArmorCustomConfig {
            ceramic_thickness_mm: 8.0,
            backing_thickness_mm: 12.0,
            ceramic_material: "ceramic_sic".to_string(),
            backing_material: "uhmwpe".to_string(),
            areal_density_kgm2: 22.0,
            plate_dimensions_mm: (250.0, 300.0),
            trauma_backface_limit_mm: BFD_LIMIT_MM,
            multi_hit_capability: 6,
        });
        let builtin = BodyArmorConfiguration::LevelIII;

        let p = BodyArmorParams {
            projectile_caliber_mm: 7.62,
            projectile_mass_g: 9.5,
            impact_velocity_ms: 847.0,
            projectile_type: "ball".to_string(),
            armor_package: builtin,
        };
        let r_builtin = evaluate_body_armor(&p);

        let p2 = BodyArmorParams {
            armor_package: custom,
            ..p
        };
        let r_custom = evaluate_body_armor(&p2);

        assert_eq!(r_builtin.penetrated, r_custom.penetrated);
        assert!((r_builtin.residual_velocity_ms - r_custom.residual_velocity_ms).abs() < 1e-9);
    }

    /// Ceramic spall efficiency peaks at t/d ≈ 0.5 and drops at extremes.
    #[test]
    fn ceramic_spall_efficiency_curve() {
        // Thin ceramic (low t/d) → poor efficiency
        let thin = ceramic_spall_efficiency("ceramic_sic", 0.1);
        // Optimal t/d
        let optimal = ceramic_spall_efficiency("ceramic_sic", 0.5);
        // Thick ceramic (high t/d) → reduced efficiency
        let thick = ceramic_spall_efficiency("ceramic_sic", 2.0);

        assert!(
            optimal > thin,
            "Optimal t/d should be better than thin: optimal={:.3}, thin={:.3}",
            optimal,
            thin
        );
        assert!(
            optimal > thick,
            "Optimal t/d should be better than thick: optimal={:.3}, thick={:.3}",
            optimal,
            thick
        );
    }

    /// Backface deformation increases with energy and decreases with areal density.
    #[test]
    fn backface_deformation_monotonic() {
        // Higher energy → larger BFD
        let low_e = backface_deformation(100.0, 22.0, 0.0);
        let high_e = backface_deformation(500.0, 22.0, 0.0);
        assert!(
            high_e > low_e,
            "Higher energy should produce larger BFD: {:.2} vs {:.2}",
            low_e,
            high_e
        );

        // Higher areal density → smaller BFD
        let light = backface_deformation(300.0, 15.0, 0.0);
        let heavy = backface_deformation(300.0, 30.0, 0.0);
        assert!(
            heavy < light,
            "Higher areal density should reduce BFD: {:.2} vs {:.2}",
            light,
            heavy
        );
    }
}
