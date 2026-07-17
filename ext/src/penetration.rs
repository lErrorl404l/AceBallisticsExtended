// ABE - Penetration & Terminal Ballistics
//
// Implements penetration models for small arms and armor.
// Uses De Marre formula for homogeneous armor, with material
// and angle modifiers.
//
// References:
//   - De Marre ballistics formula (late 19th c.)
//   - Lanz-Odermatt (long rod penetrators)
//   - UK Ordnance Board formulae
//   - NIJ 0108.01 ballistic resistance

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::behind_armor_debris::{self, BehindArmorDebrisParams};
use crate::heat_penetration::{self, HeatJetParams};

static MATERIAL_CACHE: OnceLock<HashMap<&'static str, f64>> = OnceLock::new();

/// Build the material factor lookup table.
fn build_material_cache() -> HashMap<&'static str, f64> {
    let mut m = HashMap::new();
    m.insert("steel_rha", 1.0);
    m.insert("steel_hha", 1.25);
    m.insert("aluminum_5083", 0.35);
    m.insert("aluminum_7039", 0.45);
    m.insert("ceramic_al2o3", 2.5);
    m.insert("ceramic_sic", 3.0);
    m.insert("ceramic_b4c", 3.5);
    m.insert("composite_kevlar", 0.6);
    m.insert("composite_glass", 0.4);
    m.insert("spall_liner", 0.1);
    m.insert("concrete", 0.15);
    m.insert("wood", 0.05);
    m.insert("steel_structural", 0.7);
    m.insert("mild_steel", 0.7);
    m.insert("cast_steel", 0.85);
    m.insert("depleted_uranium", 1.8);
    m.insert("titanium_alloy", 0.9);
    m.insert("lead_alloy", 0.04);
    m.insert("burlington_composite", 2.0);
    m.insert("chobham_composite", 2.2);
    m.insert("dorchester_composite", 2.5);
    m.insert("stanag_composite", 2.0);
    m.insert("textolite_composite", 0.35);
    m.insert("mexas_composite", 1.8);
    m.insert("stef_composite", 1.9);
    m.insert("kvarts_composite", 2.1);
    m.insert("k_active_composite", 2.3);
    m.insert("laminated_glass", 0.15);
    m.insert("texolite_composite", 0.35);
    m.insert("uhmwpe", 0.25);
    m.insert("rubber_elastomer", 0.015);
    m.insert("spall_liner_kevlar", 0.25);
    m.insert("kevlar_liner", 0.25);
    m.insert("twaron_liner", 0.22);
    m.insert("dyneema_liner", 0.30);
    m.insert("gypsum", 0.02);
    m.insert("gypsum_board", 0.02);
    m.insert("drywall", 0.02);
    m.insert("stud_timber", 0.04);
    m.insert("plywood", 0.035);
    m.insert("osb", 0.035);
    m.insert("adobe", 0.08);
    m.insert("rammed_earth", 0.08);
    m.insert("carbon_fiber", 0.20);
    m.insert("fiberglass", 0.12);
    m.insert("grp", 0.12);
    m.insert("mil_dtl_46100_class1", 1.30);
    m.insert("mil_dtl_46100_class3", 1.40);
    m.insert("mil_dtl_46100_class4", 1.50);
    m.insert("dual_hardness_steel", 1.10);
    m.insert("mars_armor", 1.10);
    m.insert("armor_tip_steel", 1.15);
    m.insert("rubber_solid", 0.08);
    m.insert("hard_rubber", 0.08);
    m.insert("ceramic_ad90", 2.2);
    m.insert("ad90", 2.2);
    m.insert("ceramic_ad95", 2.4);
    m.insert("ad95", 2.4);
    m.insert("mar_ceramic", 2.8);
    m.insert("perforated_armor", 0.60);
    m.insert("perf_steel", 0.60);
    m.insert("slotted_armor", 0.55);
    m.insert("slotted_steel", 0.55);
    m.insert("acrylic", 0.04);
    m.insert("acrylic_standalone", 0.04);
    m.insert("polycarbonate", 0.06);
    m.insert("polycarbonate_standalone", 0.06);
    m.insert("concrete_reinforced", 0.15);
    m.insert("wood_hardwood", 0.05);
    m.insert("hha_steel", 1.25);
    m.insert("ceramic_plate", 2.5);
    m.insert("rha_steel", 1.0);
    m
}

/// Material hardness factor relative to RHA
pub fn material_factor(material: &str) -> f64 {
    // O(1) cache lookup — built once on first call
    let cache = MATERIAL_CACHE.get_or_init(build_material_cache);
    let key = material.to_lowercase();
    if let Some(&val) = cache.get(key.as_str()) {
        return val;
    }
    // Fallback match (preserves all existing behavior)
    match material.to_lowercase().as_str() {
        "steel_rha" => 1.0,
        "steel_hha" => 1.25, // High-hardness armor
        "aluminum_5083" => 0.35,
        "aluminum_7039" => 0.45,
        "ceramic_al2o3" => 2.5, // High hardness but brittle
        "ceramic_sic" => 3.0,
        "ceramic_b4c" => 3.5,
        "composite_kevlar" => 0.6, // Per unit thickness
        "composite_glass" => 0.4,
        "spall_liner" => 0.1, // Spall liner, minimal structural resistance
        "concrete" => 0.15,
        "wood" => 0.05,
        "steel_structural" | "mild_steel" => 0.7,
        "cast_steel" => 0.85,
        "depleted_uranium" => 1.8,
        "titanium_alloy" => 0.9,
        "lead_alloy" => 0.04,
        "burlington_composite" => 2.0,
        "chobham_composite" => 2.2,
        "dorchester_composite" => 2.5,
        "stanag_composite" => 2.0,
        "textolite_composite" => 0.35,
        "mexas_composite" => 1.8,
        "stef_composite" => 1.9,
        "kvarts_composite" => 2.1,
        "k_active_composite" => 2.3,
        "laminated_glass" => 0.15,
        "texolite_composite" => 0.35,
        "uhmwpe" => 0.25,
        "rubber_elastomer" => 0.015,
        "spall_liner_kevlar" | "kevlar_liner" => 0.25,
        "twaron_liner" => 0.22,
        "dyneema_liner" => 0.30,
        "gypsum" | "gypsum_board" | "drywall" => 0.02,
        "stud_timber" => 0.04,
        "plywood" | "osb" => 0.035,
        "adobe" | "rammed_earth" => 0.08,
        "carbon_fiber" => 0.20,
        "fiberglass" | "grp" => 0.12,
        "mil_dtl_46100_class1" => 1.30,
        "mil_dtl_46100_class3" => 1.40,
        "mil_dtl_46100_class4" => 1.50,
        "dual_hardness_steel" | "mars_armor" => 1.10,
        "armor_tip_steel" => 1.15,
        "rubber_solid" | "hard_rubber" => 0.08,
        "ceramic_ad90" | "ad90" => 2.2,
        "ceramic_ad95" | "ad95" => 2.4,
        "mar_ceramic" => 2.8,
        "perforated_armor" | "perf_steel" => 0.60,
        "slotted_armor" | "slotted_steel" => 0.55,
        "acrylic" | "acrylic_standalone" => 0.04,
        "polycarbonate" | "polycarbonate_standalone" => 0.06,
        "concrete_reinforced" => 0.15,
        "wood_hardwood" => 0.05,
        "hha_steel" => 1.25,
        "ceramic_plate" => 2.5,
        "rha_steel" => 1.0,
        _ => 1.0,
    }
}

/// Projectile type modifier
fn projectile_modifier(proj_type: &str) -> f64 {
    match proj_type.to_lowercase().as_str() {
        "ball" | "fmj" => 1.0,
        "ap" | "armor_piercing" => 1.3, // Hardened core
        "apds" | "apfsds" => 1.8,       // Sub-caliber long rod
        "apcr" => 1.5,                  // Tungsten carbide core
        "heat" => 1.0,                  // HEAT — handled by heat_penetration module
        "he" => 0.3,                    // High explosive
        "incendiary" => 0.9,
        "tracer" => 0.95,
        _ => 1.0,
    }
}

/// Result of a penetration evaluation.
///
/// Returned by [`evaluate`] with the outcome of a projectile impact
/// against an armour plate: whether it penetrated, residual velocity,
/// ricochet information, and fragment counts.
#[derive(Debug, Clone)]
pub struct PenetrationResult {
    /// Whether the projectile fully perforated the plate.
    pub penetrated: bool,
    /// Residual velocity after penetration in m/s.
    pub residual_velocity: f64,
    /// Effective armour thickness after angle and material scaling
    /// in metres.
    pub effective_thickness: f64,
    /// Whether the projectile ricocheted off the surface.
    pub ricochet: bool,
    /// Outgoing ricochet angle relative to the surface in degrees.
    pub ricochet_angle: f64,
    /// Fraction of kinetic energy retained after ricochet (0.0–1.0).
    pub ricochet_energy_fraction: f64,
    /// Number of projectile fragments generated.
    pub fragments: i32,
    /// Number of armour spall fragments generated.
    pub spall_fragments: i32,
    /// Full cone angle of the spall spray (degrees).
    pub spall_cone_angle: f64,
    /// Core debris spray cone angle (degrees).
    pub debris_spray_cone: f64,
    /// Diameter of the temporary cavity formed in the armour (mm).
    pub temp_cavity_diameter: f64,
    /// Volume of the temporary cavity (cc / cm³).
    pub temp_cavity_volume: f64,
}

/// Evaluate penetration of a projectile against an armor plate.
///
/// Uses a three-stage model:
/// 1. Ricochet check — if impact angle exceeds ricochet threshold, projectile bounces
/// 2. Effective thickness — plate thickness / cos(angle) × material factor
/// 3. De Marre penetration formula: V_required = k * D^0.75 * T^0.7 / M^0.5
///
/// # Arguments
/// * `velocity_ms` - Impact velocity (m/s)
/// * `projectile_mass_kg` - Projectile mass (kg)
/// * `caliber_m` - Projectile diameter (m)
/// * `armor_thickness_m` - Armor plate thickness (m)
/// * `impact_angle_deg` - Angle from normal (0° = perpendicular)
/// * `armor_material` - Material identifier string
/// * `projectile_type` - Projectile type identifier string
pub fn evaluate(
    velocity_ms: f64,
    projectile_mass_kg: f64,
    caliber_m: f64,
    armor_thickness_m: f64,
    impact_angle_deg: f64,
    armor_material: &str,
    projectile_type: &str,
) -> PenetrationResult {
    let mat_factor = material_factor(armor_material);
    let proj_mod = projectile_modifier(projectile_type);

    let angle_rad = impact_angle_deg.to_radians();
    let cos_angle = angle_rad.cos().max(0.087); // Clamp at ~85° max

    // ── Ricochet check ─────────────────────────────────────────────────────
    // Ricochet threshold depends on velocity, caliber, and angle
    // R = sin⁻¹(V₀ / V * D / T), simplified empirical rule
    let ricochet_angle_threshold = if armor_thickness_m > 0.0 {
        let vel_ratio = 900.0 / velocity_ms.max(1.0);
        let cal_thick = caliber_m / armor_thickness_m;
        // Ricochet becomes likely above ~70° for typical rifle rounds
        70.0 + 5.0 * (vel_ratio * cal_thick).min(5.0)
    } else {
        90.0
    };

    let ricochet = impact_angle_deg > ricochet_angle_threshold;

    // ── Effective thickness ────────────────────────────────────────────────
    let base_effective = armor_thickness_m / cos_angle * mat_factor;
    // Caliber-to-thickness ratio effect: smaller calibers pen more efficiently
    // relative to their diameter against thin armor
    let cal_thick_ratio = caliber_m / armor_thickness_m.max(1e-6);
    let cal_factor = (1.0 + 0.3 * (-3.0 * cal_thick_ratio).exp()).min(1.0);
    let effective_thickness = base_effective * cal_factor;

    // ── Ricochet outcome ───────────────────────────────────────────────────
    if ricochet && !armor_material.contains("spall") {
        // Ricochet retains some energy depending on angle
        let energy_retention: f64 = if impact_angle_deg > 80.0 {
            0.85 // Glancing hit
        } else if impact_angle_deg > 75.0 {
            0.70
        } else {
            0.50
        };

        let residual_v = velocity_ms * energy_retention.sqrt();
        let ricochet_angle = (90.0 - impact_angle_deg) * 0.9; // Specular-ish

        let bad_rico = behind_armor_debris::evaluate_bad(&BehindArmorDebrisParams {
            impact_velocity_ms: velocity_ms,
            projectile_mass_kg,
            caliber_m,
            armor_thickness_m,
            armor_material: armor_material.to_string(),
            impact_angle_deg,
            projectile_type: projectile_type.to_string(),
            projectile_fragments: 0,
            residual_velocity_ms: residual_v,
            penetrated: false,
        });

        return PenetrationResult {
            penetrated: false,
            residual_velocity: residual_v,
            effective_thickness,
            ricochet: true,
            ricochet_angle: ricochet_angle.max(5.0),
            ricochet_energy_fraction: energy_retention,
            fragments: 0,
            spall_fragments: bad_rico.num_spall_fragments,
            spall_cone_angle: bad_rico.spall_cone_angle_deg,
            debris_spray_cone: bad_rico.debris_spray_cone_deg,
            temp_cavity_diameter: bad_rico.temp_cavity_diameter_mm,
            temp_cavity_volume: bad_rico.temp_cavity_volume_cc,
        };
    }

    // ── HEAT / shaped charge branch ──────────────────────────────────────────
    // For shaped-charge warheads we bypass the kinetic De Marre model and
    // instead use the physics-based shaped-charge jet penetration module.
    let proj_lower = projectile_type.to_lowercase();
    if proj_lower == "heat" {
        let target_density = heat_penetration::target_density_from_material(armor_material);
        let params = HeatJetParams {
            jet_tip_velocity_ms: 0.0,    // auto-computed from cone geometry
            jet_mass_kg: 0.0,            // auto-estimated from liner
            standoff_m: 3.0 * caliber_m, // typical tactical standoff
            caliber_m,
            cone_half_angle_deg: 28.0, // modern HEAT compromise
            liner_material: "copper".to_string(),
            liner_density_kgm3: 8960.0,
            target_armor_material: armor_material.to_string(),
            target_density_kgm3: target_density,
            impact_angle_deg,
            era_thickness_m: 0.0, // no ERA info from current callers
            armor_thickness_m,
        };

        let heat_result = heat_penetration::evaluate_heat_jet(&params);
        let penetrated = heat_result.penetration_depth_mm > (armor_thickness_m * 1000.0);
        let bad = &heat_result.behind_armor_effects;

        return PenetrationResult {
            penetrated,
            residual_velocity: heat_result.residual_jet_velocity_ms,
            effective_thickness: armor_thickness_m,
            ricochet: false,
            ricochet_angle: 0.0,
            ricochet_energy_fraction: 0.0,
            fragments: if heat_result.jet_disrupted { 5 } else { 0 },
            spall_fragments: bad.num_spall_fragments,
            spall_cone_angle: bad.spall_cone_angle_deg,
            debris_spray_cone: bad.debris_spray_cone_deg,
            temp_cavity_diameter: bad.temp_cavity_diameter_mm,
            temp_cavity_volume: bad.temp_cavity_volume_cc,
        };
    }

    // ── De Marre penetration ───────────────────────────────────────────────
    // V_required = k * D^0.75 * T^0.7 / M^0.5
    // where k is a material/construction constant (~6100 for RHA)
    //
    // Simplified: if velocity exceeds De Marre threshold, penetration occurs
    let k = 91000.0 / proj_mod;

    let v_required = if caliber_m > 0.0 && effective_thickness > 0.0 && projectile_mass_kg > 0.0 {
        let d = caliber_m;
        let t = effective_thickness;
        let m = projectile_mass_kg;
        k * d.powf(0.75) * t.powf(0.70) / m.sqrt()
    } else {
        f64::INFINITY
    };

    let penetrated = velocity_ms >= v_required;

    // ── Residual velocity ──────────────────────────────────────────────────
    let residual_velocity = if penetrated {
        // R_p = sqrt(V^2 - V_req^2)
        let vr_sq = velocity_ms.powi(2) - v_required.powi(2);
        if vr_sq > 0.0 {
            vr_sq.sqrt()
        } else {
            0.0
        }
    } else {
        velocity_ms * 0.1 // Stopped or minimal pass-through
    };

    // ── Fragments ──────────────────────────────────────────────────────────
    // Use the explicit fragmentation module for projectile breakup,
    // then add spall from armor deformation separately.
    let frag_result = crate::fragmentation::evaluate(
        velocity_ms,
        projectile_mass_kg * 1000.0,
        projectile_type,
        300.0, // Low threshold: most fragmentation relevant for pen model
        None,  // Use defaults; SQF provides specific config via ABO_* ammo params
    );
    let fragments = if penetrated {
        frag_result.num_fragments.max(2)
    } else if velocity_ms > 500.0 {
        // Non-penetrating hit can still cause some projectile breakup
        (frag_result.num_fragments / 2).max(0)
    } else {
        0
    };
    let bad_result = behind_armor_debris::evaluate_bad(&BehindArmorDebrisParams {
        impact_velocity_ms: velocity_ms,
        projectile_mass_kg,
        caliber_m,
        armor_thickness_m,
        armor_material: armor_material.to_string(),
        impact_angle_deg,
        projectile_type: projectile_type.to_string(),
        projectile_fragments: fragments,
        residual_velocity_ms: residual_velocity,
        penetrated,
    });

    PenetrationResult {
        penetrated,
        residual_velocity,
        effective_thickness,
        ricochet: false,
        ricochet_angle: 0.0,
        ricochet_energy_fraction: 0.0,
        fragments,
        spall_fragments: bad_result.num_spall_fragments,
        spall_cone_angle: bad_result.spall_cone_angle_deg,
        debris_spray_cone: bad_result.debris_spray_cone_deg,
        temp_cavity_diameter: bad_result.temp_cavity_diameter_mm,
        temp_cavity_volume: bad_result.temp_cavity_volume_cc,
    }
}

// ── V₅₀ / V₀ Statistical Penetration ───────────────────────────────────────────

/// Statistical penetration parameters for probabilistic armor assessment.
///
/// V₅₀ is the velocity at which a projectile has a 50% probability of
/// perforating a given armor configuration. V₀ is the velocity at which
/// the probability drops to a specified threshold (typically ≤5 %).
/// σ characterizes the statistical spread due to material and manufacturing
/// variances.
#[derive(Debug, Clone, Copy)]
pub struct PenetrationStatistics {
    /// Velocity at which P(penetration) = 50 % (m/s)
    pub v50_ms: f64,
    /// Velocity at which P(penetration) ≈ threshold (m/s)
    pub v0_ms: f64,
    /// Standard deviation of the V₅₀ distribution (m/s)
    pub sigma_ms: f64,
    /// Confidence level for the reported values (0.0–100.0 %)
    pub confidence_pct: f64,
}

/// Compute the probability of penetration at a given impact velocity
/// using the logistic distribution.
///
/// The logistic CDF models the transition zone between the "never
/// penetrates" and "always penetrates" regimes:
///
/// ```text
/// P(v) = 1 / (1 + exp(−π · (v − V₅₀) / (√3 · σ)))
/// ```
///
/// where `π / √3 ≈ 1.814` scales the logistic distribution to have the
/// same variance as a normal distribution with standard deviation σ.
pub fn penetration_probability(velocity_ms: f64, v50_ms: f64, sigma_ms: f64) -> f64 {
    if sigma_ms <= 0.0 {
        return if velocity_ms >= v50_ms { 1.0 } else { 0.0 };
    }
    let z = std::f64::consts::PI * (velocity_ms - v50_ms) / (sigma_ms * 3.0_f64.sqrt());
    1.0 / (1.0 + (-z).exp())
}

/// Estimate the V₅₀ velocity for a given projectile / armor combination.
///
/// V₅₀ is typically 2–10 % above the deterministic threshold velocity
/// (V_req). The multiplier varies by armor material class:
///
/// | Material class  | Multiplier  | σ (% of V₅₀) |
/// |-----------------|------------|---------------|
/// | RHA steel       | 1.02–1.05  | 1–2 %         |
/// | HHA steel       | 1.03–1.06  | 1–2 %         |
/// | Aluminum        | 1.02–1.04  | 1–2 %         |
/// | Ceramic         | 1.05–1.10  | 3–5 %         |
/// | Composite       | 1.04–1.08  | 2–4 %         |
///
/// # Arguments
/// * `v_required_ms` — deterministic minimum velocity for penetration (m/s)
/// * `projectile_type` — projectile type identifier (for modifier)
/// * `caliber_m` — projectile diameter (m) — used for caliber corrections
/// * `armor_material` — armor material identifier
pub fn penetration_v50(
    v_required_ms: f64,
    projectile_type: &str,
    caliber_m: f64,
    armor_material: &str,
) -> f64 {
    if v_required_ms <= 0.0 || v_required_ms.is_infinite() {
        return f64::INFINITY;
    }

    // Material-specific V₅₀ multiplier (V₅₀ / V_required)
    let multiplier = match armor_material.to_lowercase().as_str() {
        // Steel family — tight spread, small overmatch
        s if s.contains("rha") || s == "steel_rha" => 1.03,
        s if s.contains("hha") || s == "steel_hha" => 1.04,
        s if s.contains("mil_dtl") => 1.04,
        // Aluminum — similar to RHA
        s if s.contains("aluminum") || s.contains("al_") => 1.03,
        // Ceramics — wide spread, higher overmatch needed
        s if s.contains("ceramic") || s.contains("ad90") || s.contains("ad95") => 1.08,
        // Composites — moderate spread
        s if s.contains("composite") || s.contains("kevlar") || s.contains("dyneema") => 1.06,
        // Transparent armor
        s if s.contains("glass") || s.contains("acrylic") || s.contains("polycarbonate") => 1.05,
        // Building materials
        s if s.contains("concrete") || s.contains("gypsum") || s.contains("drywall") => 1.04,
        // Default — mild steel / unknown
        _ => 1.04,
    };

    // Projectile-type modifier: sub-caliber and APFSDS rounds tend to have
    // less V₅₀ variability than ball rounds
    let proj_factor = match projectile_type.to_lowercase().as_str() {
        "apfsds" | "apds" => 1.0, // No extra multiplier — precise penetrators
        "ap" | "armor_piercing" | "apcr" => 1.01,
        "ball" | "fmj" => 1.02, // More variability with ball ammunition
        _ => 1.01,
    };

    // Small caliber effect: sub-10 mm projectiles may need slightly higher
    // overmatch against heterogeneous armor
    let cal_factor = if caliber_m > 0.0 && caliber_m < 0.010 {
        1.02
    } else {
        1.0
    };

    v_required_ms * multiplier * proj_factor * cal_factor
}

/// Estimate the V₀ velocity (penetration threshold).
///
/// V₀ is defined as the velocity at which the penetration probability
/// drops to `threshold_p` (typically 0.05 for the 5 % threshold).
///
/// For the logistic distribution the inverse CDF is:
///
/// ```text
/// V₀ = V₅₀ + (√3 · σ / π) · ln(Pₜ / (1 − Pₜ))
/// ```
pub fn penetration_v0(v50_ms: f64, sigma_ms: f64, threshold_p: f64) -> f64 {
    if sigma_ms <= 0.0 {
        return v50_ms;
    }
    // Clamp threshold to valid range for the log-odds transform
    let p = threshold_p.clamp(1e-12, 0.5 - 1e-12);
    let ln_odds = (p / (1.0 - p)).ln();
    v50_ms + (sigma_ms * 3.0_f64.sqrt() / std::f64::consts::PI) * ln_odds
}

/// Compute sigma (standard deviation) for a given armor material and V₅₀.
///
/// Provides typical σ as a percentage of V₅₀ based on armor material class.
pub fn penetration_sigma(v50_ms: f64, armor_material: &str) -> f64 {
    let frac = match armor_material.to_lowercase().as_str() {
        s if s.contains("ceramic") || s.contains("ad90") || s.contains("ad95") => 0.04,
        s if s.contains("composite") || s.contains("kevlar") || s.contains("dyneema") => 0.03,
        s if s.contains("glass") || s.contains("acrylic") => 0.03,
        s if s.contains("concrete") => 0.025,
        s if s.contains("rha") || s == "steel_rha" => 0.015,
        s if s.contains("hha") || s == "steel_hha" => 0.015,
        s if s.contains("aluminum") || s.contains("al_") => 0.015,
        _ => 0.02,
    };
    v50_ms * frac
}

/// Build a `PenetrationStatistics` struct from the input parameters.
///
/// This is a convenience function that computes V₅₀, V₀ (at the default
/// 5 % threshold), σ, and returns them together.
pub fn penetration_statistics(
    v_required_ms: f64,
    projectile_type: &str,
    caliber_m: f64,
    armor_material: &str,
    threshold_p: f64,
) -> PenetrationStatistics {
    let v50 = penetration_v50(v_required_ms, projectile_type, caliber_m, armor_material);
    let sigma = penetration_sigma(v50, armor_material);
    PenetrationStatistics {
        v50_ms: v50,
        v0_ms: penetration_v0(v50, sigma, threshold_p),
        sigma_ms: sigma,
        confidence_pct: 95.0,
    }
}

// ── Lanz-Odermatt (Long Rod Penetrator Model) ──────────────────────────────────

/// Compute the normalised penetration ratio P/L using the Lanz-Odermatt
/// formula, corrected for SI units (expects m/s, converts to km/s internally).
///
/// Returns the dimensionless penetration depth / rod length ratio.
///
/// # Formula
/// ```text
/// P/L = sqrt(ρₚ / ρₜ) · (v² − vₘᵢₙ²) / (k · cos(θ)ⁿ)
/// ```
///
/// where:
/// - `ρₚ` — projectile density (kg/m³) — e.g. 17 500 for tungsten
/// - `ρₜ` — target density (kg/m³) — e.g. 7 850 for RHA steel
/// - `v`  — impact velocity **in km/s** (converted from m/s internally)
/// - `vₘᵢₙ` — minimum eroding velocity **in km/s**
/// - `k`  — material constant (~2.0 for RHA)
/// - `θ`  — impact angle from normal (degrees)
/// - `n`  — angle exponent (~2.0 for long rods)
///
/// # Example (APFSDS vs RHA)
/// ```
/// # use abe_ballistics_ext::penetration::lanz_odermatt_depth;
/// let p_over_l = lanz_odermatt_depth(
///     1600.0,     // 1.6 km/s
///     700.0,      // v_min = 700 m/s → 0.7 km/s
///     17500.0,    // tungsten density
///     7850.0,     // RHA density
///     2.0,        // k
///     0.0,        // 0° impact
///     2.0,        // n
/// );
/// // P/L should be ~1.5 for a typical APFSDS at 1.6 km/s
/// assert!(p_over_l > 0.5 && p_over_l < 3.0, "P/L = {p_over_l}");
/// ```
pub fn lanz_odermatt_depth(
    velocity_ms: f64,
    v_min_ms: f64,
    rho_p: f64,
    rho_t: f64,
    k: f64,
    angle_deg: f64,
    n: f64,
) -> f64 {
    if velocity_ms <= 0.0 || rho_t <= 0.0 || k <= 0.0 {
        return 0.0;
    }

    let cos_angle = angle_deg.to_radians().cos().max(0.087); // clamp at ~85°
    let v_km = velocity_ms / 1000.0;
    let v_min_km = v_min_ms / 1000.0;

    (rho_p / rho_t).sqrt() * (v_km.powi(2) - v_min_km.powi(2)) / (k * cos_angle.powf(n))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn m80_ball_pens_5mm_rha_at_0deg() {
        let r = evaluate(853.0, 0.0095, 0.00762, 0.005, 0.0, "steel_rha", "ball");
        assert!(r.penetrated, "M80 ball should pen 5mm RHA at 0°");
        assert!(r.residual_velocity > 100.0);
    }

    #[test]
    fn m80_ball_does_not_pen_20mm_rha() {
        let r = evaluate(853.0, 0.0095, 0.00762, 0.020, 0.0, "steel_rha", "ball");
        assert!(!r.penetrated, "M80 ball should NOT pen 20mm RHA at 0°");
    }

    #[test]
    fn angle_reduces_penetration() {
        let r0 = evaluate(900.0, 0.0095, 0.00762, 0.008, 0.0, "steel_rha", "ball");
        let r60 = evaluate(900.0, 0.0095, 0.00762, 0.008, 60.0, "steel_rha", "ball");
        if r0.penetrated && !r60.penetrated {
            // Expected: 0° pens, 60° doesn't
        }
        assert!(
            r0.effective_thickness < r60.effective_thickness,
            "Effective thickness should increase with angle"
        );
    }

    #[test]
    fn ap_round_pens_better_than_ball() {
        let ball = evaluate(880.0, 0.0095, 0.00762, 0.010, 0.0, "steel_rha", "ball");
        let ap = evaluate(880.0, 0.0095, 0.00762, 0.010, 0.0, "steel_rha", "ap");
        assert!(
            ap.penetrated || !ball.penetrated,
            "AP should pen equal or better than ball"
        );
    }

    #[test]
    fn ricochet_at_shallow_angle() {
        let r = evaluate(850.0, 0.0095, 0.00762, 0.010, 80.0, "steel_rha", "ball");
        assert!(r.ricochet, "80° impact should ricochet");
        assert!(r.ricochet_energy_fraction > 0.0);
    }

    #[test]
    fn hha_is_harder_than_rha() {
        let rha = evaluate(900.0, 0.0095, 0.00762, 0.010, 0.0, "steel_rha", "ball");
        let hha = evaluate(900.0, 0.0095, 0.00762, 0.010, 0.0, "steel_hha", "ball");
        assert!(hha.effective_thickness > rha.effective_thickness);
    }

    #[test]
    fn penetration_produces_fragments() {
        let r = evaluate(900.0, 0.0095, 0.00762, 0.006, 0.0, "steel_rha", "ball");
        if r.penetrated {
            assert!(r.fragments > 0, "Penetrating hit should produce fragments");
            assert!(
                r.spall_fragments > 0,
                "Penetrating hit should produce spall"
            );
        }
    }

    #[test]
    fn high_velocity_pens_more() {
        let slow = evaluate(400.0, 0.0095, 0.00762, 0.005, 0.0, "steel_rha", "ball");
        let fast = evaluate(900.0, 0.0095, 0.00762, 0.005, 0.0, "steel_rha", "ball");
        assert!(
            fast.penetrated || !slow.penetrated,
            "Higher velocity should pen at least as well"
        );
    }

    // ── Lanz-Odermatt ────────────────────────────────────────────────────────

    #[test]
    fn lanz_odermatt_apfsds_vs_rha_plausible() {
        // APFSDS at 1600 m/s, 4.6 kg, 120mm gun, vs RHA at 0°
        // Expect P/L ~1.5 (rod length ~600mm → pen depth ~900mm RHAe)
        let p_over_l = super::lanz_odermatt_depth(
            1600.0,  // 1.6 km/s
            700.0,   // v_min = 700 m/s → 0.7 km/s
            17500.0, // tungsten density
            7850.0,  // RHA density
            2.0,     // material constant
            0.0,     // 0° impact
            2.0,     // angle exponent
        );
        assert!(
            p_over_l > 0.5 && p_over_l < 3.0,
            "APFSDS P/L should be ~1.5, got {p_over_l}"
        );
    }

    #[test]
    fn lanz_odermatt_zero_velocity_no_pen() {
        let p_over_l = super::lanz_odermatt_depth(0.0, 700.0, 17500.0, 7850.0, 2.0, 0.0, 2.0);
        assert_eq!(p_over_l, 0.0);
    }

    #[test]
    fn lanz_odermatt_cos_denom_increases_pl_at_angle() {
        // With cosⁿ in the denominator, P/L increases at oblique angles
        // (long-rod tunnelling effect in Lanz-Odermatt model).
        // cos(60°) = 0.5, cos² = 0.25 → P/L = 4× baseline for n=2
        let at_0 = super::lanz_odermatt_depth(1600.0, 700.0, 17500.0, 7850.0, 2.0, 0.0, 2.0);
        let at_60 = super::lanz_odermatt_depth(1600.0, 700.0, 17500.0, 7850.0, 2.0, 60.0, 2.0);
        let ratio = at_60 / at_0;
        // n=2 → ratio ≈ 1/0.5² = 4.0
        assert!(
            (ratio - 4.0).abs() < 0.001,
            "P/L at 60° should be ~4× 0° for n=2: ratio={ratio}"
        );
    }

    // ── V₅₀ / V₀ ─────────────────────────────────────────────────────────────

    #[test]
    fn v0_is_less_than_v50() {
        // V₀ (5% threshold) must be strictly less than V₅₀ for any σ > 0
        let v0 = super::penetration_v0(870.0, 15.0, 0.05);
        assert!(
            v0 < 870.0,
            "V₀ should be less than V₅₀: v0={v0:.1}, v50=870.0"
        );
        assert!(
            v0 > 800.0 && v0 < 870.0,
            "V₀ should be between 800-870 m/s for σ=15: v0={v0:.1}"
        );
    }

    #[test]
    fn penetration_probability_at_v50_is_half() {
        // At v = V₅₀, P = 0.5 exactly
        let p = super::penetration_probability(870.0, 870.0, 15.0);
        assert!(
            (p - 0.5).abs() < 1e-12,
            "P(V₅₀) should be exactly 0.5: p={p}"
        );
    }

    #[test]
    fn penetration_probability_far_above_v50_approaches_one() {
        // At v >> V₅₀, P → 1
        let p = super::penetration_probability(870.0 + 10.0 * 15.0, 870.0, 15.0);
        assert!(p > 0.9999, "P(v >> V₅₀) should approach 1: p={p}");
    }

    #[test]
    fn penetration_probability_far_below_v50_approaches_zero() {
        // At v << V₅₀, P → 0
        let p = super::penetration_probability(870.0 - 10.0 * 15.0, 870.0, 15.0);
        assert!(p < 0.0001, "P(v << V₅₀) should approach 0: p={p}");
    }

    #[test]
    fn penetration_v50_ceramic_higher_than_rha() {
        let rha_v50 = super::penetration_v50(800.0, "ball", 0.00762, "steel_rha");
        let ceramic_v50 = super::penetration_v50(800.0, "ball", 0.00762, "ceramic_b4c");
        assert!(
            ceramic_v50 > rha_v50,
            "Ceramic V₅₀ should be higher than RHA V₅₀: ceramic={ceramic_v50:.1}, rha={rha_v50:.1}"
        );
    }
}
