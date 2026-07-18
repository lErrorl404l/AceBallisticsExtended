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
use crate::systems::config::get_data_registry;

static MATERIAL_CACHE: OnceLock<HashMap<&'static str, f64>> = OnceLock::new();

/// Build the material factor lookup table.
fn build_material_cache() -> HashMap<&'static str, f64> {
    let mut m = HashMap::new();
    m.insert("steel_rha", 1.0);
    m.insert("steel_hha", 1.25);
    m.insert("aluminum_5083", 0.35);
    m.insert("aluminum_7039", 0.45);
    m.insert("ceramic_al2o3", 2.5); // standalone Al2O3 tile
    m.insert("ceramic_sic", 3.5); // standalone SiC tile
    m.insert("ceramic_b4c", 4.5); // standalone B4C tile
    m.insert("esapi_al2o3", 3.0); // Al2O3 in ESAPI backing array
    m.insert("esapi_sic", 4.0); // SiC in ESAPI backing array
    m.insert("esapi_b4c", 5.5); // B4C in ESAPI backing array (higher)
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

/// Material hardness factor relative to RHA.
///
/// Ceramic multipliers are velocity-dependent — RHAe values decrease at higher
/// impact velocities (> 900 m/s) as ceramic fracture mechanisms shift from
/// dwell/interface defeat to erosion. The values here represent nominal
/// mid-velocity (500–900 m/s) performance.
pub fn material_factor(material: &str) -> f64 {
    // Prefer data-loaded material configs
    if let Some(registry) = get_data_registry() {
        if let Some(mat) = registry.materials.get(material) {
            return mat.rha_equivalent;
        }
    }

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
        "ceramic_al2o3" => 2.5,    // standalone Al2O3 tile RHAe ~2.5
        "ceramic_sic" => 3.5,      // standalone SiC tile RHAe ~3.0-4.0
        "ceramic_b4c" => 4.5,      // standalone B4C tile RHAe ~4.0-5.0; ESAPI array ~5.0
        "esapi_al2o3" => 3.0,      // Al2O3 in ESAPI backing array (higher than standalone)
        "esapi_sic" => 4.0,        // SiC in ESAPI backing array
        "esapi_b4c" => 5.5,        // B4C in ESAPI backing array (higher)
        "composite_kevlar" => 0.6, // Per unit thickness
        "composite_glass" => 0.55, // S2-glass/phenolic RHAe ~0.5-0.7 per ARL
        "spall_liner" => 0.1,      // Spall liner, minimal structural resistance
        "concrete" => 0.12,        // ASMRB 0.11 for 1:3:5 mix; harmonized with concrete_reinforced
        "wood" => 0.03,            // ASMRB 0.012 oak; DAAAM 2019: 100mm oak=3-4mm RHAe → 0.03-0.04
        "steel_structural" | "mild_steel" => 0.55, // vs AP ~0.50-0.55; vs ball ~0.70
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
        "spall_liner_kevlar" | "kevlar_liner" => 0.20, // M113 LFTE: 0.15-0.20 for spall-only role
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
        "dual_hardness_steel" | "mars_armor" => 1.25, // MIL-A-46099C 600+BHN face; ARL shows V50 15-25% above RHA
        "armor_tip_steel" => 1.15,
        "rubber_solid" | "hard_rubber" => 0.08,
        "ceramic_ad90" | "ad90" => 2.0, // AD90 (90% Al2O3) ~1.8-2.0 RHAe vs 7.62 AP
        "ceramic_ad95" | "ad95" => 2.4,
        "mar_ceramic" => 2.8,
        "perforated_armor" | "perf_steel" => 0.60,
        "slotted_armor" | "slotted_steel" => 0.55,
        "acrylic" | "acrylic_standalone" => 0.04,
        "polycarbonate" | "polycarbonate_standalone" => 0.06,
        "concrete_reinforced" => 0.12, // ASMRB 0.11 for 1:3:5; harmonized with concrete key
        "wood_hardwood" => 0.05,
        "hha_steel" => 1.25,
        "ceramic_plate" => 2.5,
        "rha_steel" => 1.0,
        // ── Grades missing code keys per IRL Validation Report ──────────────
        "armox_500" => 1.10,
        "armox_600" => 1.35,
        "hardox_450" => 0.80,
        "relikt_era" => 2.00,
        "malachite_era" => 2.00,
        "tungsten_carbide" => 2.75,
        "titanium_diboride" => 3.75,
        "aluminum_7075" => 0.48,
        _ => 1.0,
    }
}

/// Check whether an AP projectile catastrophically shatters against hard armor.
///
/// Shatter occurs when a hard but brittle AP projectile strikes very hard
/// ceramic / dual-hardness armor at high velocity. The penetrator fractures
/// and penetration drops drastically — the projectile fails before the armor does.
///
/// Returns `true` when ALL of the following hold:
/// - Projectile is AP or armor_piercing
/// - Armor material is a hard ceramic (ceramic, B4C, SiC, AD90, AD95, etc.)
///   or dual-hardness steel / mars_armor
/// - Impact velocity exceeds ~700 m/s
/// - Impact angle is within the shatter zone (< 60° from normal)
fn check_shatter(
    velocity_ms: f64,
    projectile_type: &str,
    armor_material: &str,
    impact_angle_deg: f64,
) -> bool {
    // Only AP projectiles are hard enough to shatter
    let lower_proj = projectile_type.to_lowercase();
    if lower_proj != "ap" && lower_proj != "armor_piercing" {
        return false;
    }

    // Check for hard ceramic / dual-hardness armor materials
    let lower_mat = armor_material.to_lowercase();
    let is_hard_armor = lower_mat.contains("ceramic")
        || lower_mat.contains("b4c")
        || lower_mat.contains("sic")
        || lower_mat.contains("ad90")
        || lower_mat.contains("ad95")
        || lower_mat.contains("dual_hardness")
        || lower_mat.contains("mars_armor");

    if !is_hard_armor {
        return false;
    }

    // Velocity threshold — AP vs ceramic shatter typically above ~700 m/s
    if velocity_ms <= 700.0 {
        return false;
    }

    // Very oblique impacts don't produce the shatter condition
    if impact_angle_deg >= 60.0 {
        return false;
    }

    true
}

const INTERFACE_DEFEAT_VELOCITY_THRESHOLD: f64 = 1500.0; // modern B4C/SiC ceramics sustain dwell to ~1500 m/s

/// Check whether an AP projectile experiences interface defeat against
/// hard ceramic-backed armor.
///
/// Interface defeat occurs when a hard ceramic face (e.g. B4C, SiC, AD95)
/// backed by a ductile material (UHMWPE, aluminum, steel) erodes the
/// projectile at the ceramic interface. The ceramic causes the projectile
/// tip to deform/erode, and the ductile backing traps the debris,
/// significantly reducing penetration.
///
/// # Conditions
/// - Projectile is AP, APDS, APFSDS, or APCR
/// - Armor face is a hard ceramic (b4c, sic, ad95 in material name)
/// - Backing material is ductile (UHMWPE, aluminum, steel, dyneema, kevlar)
/// - Impact velocity < 1500 m/s (modern silicon carbide/boron carbide ceramic
///   systems with ductile backing can sustain interface defeat (dwell) to
///   ~1500 m/s, above which the penetrator erodes through before full dwell
///   can be established. Reference: Lundberg et al., "Interface Defeat in
///   B4C Ceramics", Int. J. Impact Eng. 2000.)
/// - Ceramic thickness > 0.5 × projectile caliber (ensures dwell time)
///
/// # Returns
/// `true` if interface defeat conditions are met. Callers should multiply
/// the penetration by [`interface_defeat_penetration_multiplier`] when
/// this returns `true`.
pub fn check_interface_defeat(
    velocity_ms: f64,
    projectile_type: &str,
    armor_material: &str,
    backing_material: &str,
    caliber_m: f64,
    armor_thickness_m: f64,
) -> bool {
    // Only hard-core projectiles can be eroded at the interface
    let lower_proj = projectile_type.to_lowercase();
    let is_ap = lower_proj == "ap"
        || lower_proj == "armor_piercing"
        || lower_proj == "apds"
        || lower_proj == "apfsds"
        || lower_proj == "apcr";
    if !is_ap {
        return false;
    }

    // Check for hard ceramic face material
    let lower_armor = armor_material.to_lowercase();
    let is_hard_ceramic =
        lower_armor.contains("b4c") || lower_armor.contains("sic") || lower_armor.contains("ad95");
    if !is_hard_ceramic {
        return false;
    }

    // Interface defeat requires a ductile backing to trap debris
    let lower_back = backing_material.to_lowercase();
    let is_ductile_backing = lower_back.contains("uhmwpe")
        || lower_back.contains("aluminum")
        || lower_back.contains("steel")
        || lower_back.contains("dyneema")
        || lower_back.contains("kevlar");
    if !is_ductile_backing {
        return false;
    }

    // Interface defeat only works below critical velocity
    if velocity_ms >= INTERFACE_DEFEAT_VELOCITY_THRESHOLD {
        return false;
    }

    // Ceramic must be thick enough to sustain dwell
    if armor_thickness_m <= 0.5 * caliber_m {
        return false;
    }

    true
}

/// Compute the penetration multiplier when interface defeat occurs.
///
/// Returns a value in [0.4, 0.7] corresponding to 30–60% reduction in
/// penetration depth. Higher velocities near the critical threshold
/// (1500 m/s) produce less reduction (multiplier ≈ 0.7), while lower
/// velocities produce greater erosion (multiplier ≈ 0.4).
pub fn interface_defeat_penetration_multiplier(velocity_ms: f64) -> f64 {
    let v = velocity_ms.clamp(0.0, INTERFACE_DEFEAT_VELOCITY_THRESHOLD);
    let mult = 0.4 + 0.3 * (v / INTERFACE_DEFEAT_VELOCITY_THRESHOLD);
    mult.clamp(0.4, 0.7)
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

/// Compute the mass lost by a projectile during armour penetration.
///
/// Uses a Poncelet-form erosion model where the mass-loss fraction scales
/// with the square of impact velocity, material hardness, and relative
/// penetration depth. Harder armour (ceramic/B₄C) produces 15–40 % mass
/// loss depending on velocity; steel/RHA produces 5–20 %.
///
/// # Arguments
/// * `velocity_ms` - Impact velocity (m/s)
/// * `mass_kg` - Projectile mass before penetration (kg)
/// * `caliber_m` - Projectile caliber (m)
/// * `armor_material` - Armour material identifier string
/// * `penetration_depth_m` - Total penetration depth through armour (m)
///
/// # Returns
/// Eroded mass in kilograms (≤ `mass_kg`). Returns 0.0 for invalid inputs.
///
/// # Formula
/// ```text
/// Δm / m = k_mat × (v / 800)² × sqrt(P / D)
/// ```
/// where `k_mat` is the material erosion coefficient derived from
/// [`material_factor`], `v` is impact velocity, `P` is penetration depth,
/// and `D` is caliber.
pub fn erode_projectile_mass(
    velocity_ms: f64,
    mass_kg: f64,
    caliber_m: f64,
    armor_material: &str,
    penetration_depth_m: f64,
) -> f64 {
    if mass_kg <= 0.0 || penetration_depth_m <= 0.0 || velocity_ms <= 0.0 || caliber_m <= 0.0 {
        return 0.0;
    }

    let mat_factor = material_factor(armor_material);

    // Poncelet form: erosion rate ∝ dynamic pressure (ρ·v²) and material hardness.
    // Normalise to 800 m/s reference velocity.
    let vel_factor = (velocity_ms / 800.0).powi(2);

    // Depth factor: cumulative erosion grows with sqrt of relative penetration depth.
    let depth_factor = (penetration_depth_m / caliber_m).sqrt().min(5.0);

    // Base erosion fraction: calibrated so RHA (mat_factor=1.0) at 800 m/s
    // through 1-caliber depth gives ~10 % mass loss.
    let raw_fraction = 0.10 * mat_factor * vel_factor * depth_factor;

    // Clamp to material-specific ranges
    let lower_mat = armor_material.to_lowercase();
    let is_ceramic = lower_mat.contains("ceramic")
        || lower_mat.contains("b4c")
        || lower_mat.contains("sic")
        || lower_mat.contains("ad90")
        || lower_mat.contains("ad95")
        || lower_mat.contains("mar_ceramic");
    let is_steel = lower_mat.contains("steel")
        || lower_mat.contains("rha")
        || lower_mat.contains("hha")
        || lower_mat.contains("armox")
        || lower_mat.contains("hardox")
        || lower_mat.contains("mil_dtl")
        || lower_mat.contains("dual_hardness");

    let clamped = if is_ceramic {
        raw_fraction.clamp(0.15, 0.40)
    } else if is_steel {
        raw_fraction.clamp(0.05, 0.20)
    } else {
        raw_fraction.clamp(0.03, 0.35)
    };

    (clamped * mass_kg).min(mass_kg)
}

/// De Marre calibration constant `k` for a given projectile type.
///
/// Returns the De Marre coefficient `k` for the given projectile type.
/// Lower `k` = more efficient penetration (lower required velocity).
/// These values are calibrated for the De Marre formula with D and T in metres
/// and M in kilograms:
///
/// ```text
/// V_required = k * D^0.75 * T^0.7 / M^0.5
/// ```
///
/// # Provenance
/// - **ball / fmj (91000)**: Standard full metal jacket — De Marre's original 1890s experiments
/// - **AP (hard steel, ~70000)**: Calibrated against WWI-WWII AP data (De Marre, Krupp, US Army test data)  
/// - **APFSDS (tungsten long rod, ~50500)**: Community standard from BALI/Litz extended De Marre fits
///
/// These are community-calibrated empirical constants, not physically derived.
/// Sources: TM 43-0001-27, De Marre (1893), BALI Technical Notes.
///
/// | Projectile type  | k       | Notes                           |
/// |------------------|---------|---------------------------------|
/// | ball / fmj       | 91000   | Standard full metal jacket      |
/// | ap / armor_piercing | 70000 | Hardened steel core            |
/// | apds / apfsds    | 50500   | Sub-calibre long rod, most eff. |
/// | apcr             | 60700   | Tungsten carbide core           |
/// | heat             | 100000  | Inefficient by KE (shaped chrg) |
/// | soft_point / hollow_point | 95000 | Deforms on impact, less eff. |
pub fn de_marre_k(projectile_type: &str) -> f64 {
    match projectile_type.to_lowercase().as_str() {
        "ball" | "fmj" => 91000.0,
        "ap" | "armor_piercing" => 70000.0,
        "apds" | "apfsds" => 50500.0,
        "apcr" => 60700.0,
        "heat" => 100000.0,
        "soft_point" | "hollow_point" => 95000.0,
        _ => 91000.0,
    }
}

// TODO: Add THOR equation alternative penetration model for high-hardness
// rolled homogenous armor. THOR provides better accuracy for RHA targets
// with hardness > 350 BHN at velocities below 1000 m/s. Reference:
// THOR Program Report No. 61-76 (1961), BRL Memorandum Report 1385.

/// Mott distribution fragment mass sampling.
///
/// Behind-armor debris does not have a single average mass — fragments
/// follow a Mott distribution:
///
/// ```text
/// p(m) = (2 / √π) · exp(-m / μ) / √(m · μ)
/// ```
///
/// where μ is the characteristic fragment mass parameter.
///
/// This returns the expected distribution of fragment masses rather than
/// using avg_frag_mass as a single value. The BAD model can use this to
/// assign statistical variation to fragment sizes.
///
/// # Arguments
/// * `avg_mass` — Mean fragment mass (kg)
/// * `percentile` — Cumulative probability [0, 1) to sample, e.g. 0.5 for median
///
/// # Formula
/// ```text
/// m(ν) = μ · (erf⁻¹(ν))²   →   m(ν) ≈ μ · (−ln(1 − ν))
/// ```
/// where `μ = avg_mass / k` and `k ≈ 1.0` for steel fragments.
/// The approximation uses the relationship: `erf⁻¹(ν) ≈ sqrt(−ln(1 − ν))`
/// which gives the simplified CDF: `m(ν) = avg_mass · (−ln(1 − ν))``
///
/// Reference: Mott, "A Theory of Fragmentation", Ministry of Supply AC 3642, 1943.
pub fn mott_fragment_mass(avg_mass: f64, percentile: f64) -> f64 {
    if avg_mass <= 0.0 || percentile <= 0.0 || percentile >= 1.0 {
        return avg_mass;
    }
    // Mott distribution inverse CDF approximation: m(ν) = μ * (-ln(1-ν))
    // where μ ≈ avg_mass for steel fragments (k ≈ 1.0)
    avg_mass * (-(1.0 - percentile).ln())
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
/// Backward-compatible wrapper that calls [`evaluate_yaw`] with `yaw_angle_deg = 0.0`.
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
/// * `ricochet_angle_deg` - Optional maximum ricochet angle in degrees.
///   Pass `None` to use the default formula.
pub fn evaluate(
    velocity_ms: f64,
    projectile_mass_kg: f64,
    caliber_m: f64,
    armor_thickness_m: f64,
    impact_angle_deg: f64,
    armor_material: &str,
    projectile_type: &str,
    ricochet_angle_deg: Option<f64>,
) -> PenetrationResult {
    evaluate_yaw(
        velocity_ms,
        projectile_mass_kg,
        caliber_m,
        armor_thickness_m,
        impact_angle_deg,
        armor_material,
        projectile_type,
        0.0, // no yaw
        ricochet_angle_deg,
    )
}

/// Yaw coefficient K_YAW by projectile type.
///
/// K_YAW represents the effect of yaw on penetration efficiency — a yawed
/// projectile presents a larger cross-section, reducing effective penetration.
/// Values calibrated from Bless et al., "Yawed Impact of Long Rods", 1987
/// and subsequent BRL/MIL-DTL test series.
///
/// | Projectile type | K_YAW | Characteristics                          |
/// |-----------------|-------|------------------------------------------|
/// | APFSDS/long_rod | 0.015 | Slender, self-sharpening — least yaw sens.|
/// | AP/APHE/APCR    | 0.022 | Hard core, moderate aspect ratio         |
/// | Ball/FMJ        | 0.028 | Round-nose ogive, reference baseline     |
/// | Blunt/HP        | 0.040 | Flat meplat — most yaw-sensitive         |
pub fn yaw_coefficient(projectile_type: &str) -> f64 {
    match projectile_type.to_lowercase().as_str() {
        "apfsds" | "long_rod" => 0.015,
        "ap" | "armor_piercing" | "aphe" | "apcr" => 0.022,
        "ball" | "fmj" => 0.028,
        "blunt" | "hp" | "hollow_point" | "soft_point" => 0.040,
        _ => 0.028,
    }
}

/// Evaluate penetration of a projectile against an armor plate,
/// including yaw-angle effects at impact.
///
/// Uses a four-stage model:
/// 1. Ricochet check — if impact angle exceeds ricochet threshold, projectile bounces
/// 2. Yaw multiplier — yaw reduces effective penetration by increasing presented cross-section
/// 3. Effective thickness — plate thickness / cos(angle) × material factor / yaw_mult
/// 4. De Marre penetration formula: V_required = k * D^0.75 * T^0.7 / M^0.5
///
/// Yaw effect: `yaw_mult = exp(-k_yaw × yaw_deg)` where `k_yaw` is determined
/// by [`yaw_coefficient`] based on projectile type, capped at 50 % reduction
/// (multiplier clamped to [0.5, 1.0]).
/// Small yaw (< 5°) has minimal effect; large yaw (10–20°) reduces penetration
/// by 30–50 %.
///
/// # Arguments
/// * `velocity_ms` - Impact velocity (m/s)
/// * `projectile_mass_kg` - Projectile mass (kg)
/// * `caliber_m` - Projectile diameter (m)
/// * `armor_thickness_m` - Armor plate thickness (m)
/// * `impact_angle_deg` - Angle from normal (0° = perpendicular)
/// * `armor_material` - Material identifier string
/// * `projectile_type` - Projectile type identifier string
/// * `yaw_angle_deg` - Yaw angle at impact (0 = perfectly aligned)
/// * `ricochet_angle_deg` - Optional maximum ricochet angle in degrees.
///   When `Some(angle)`, the ricochet angle is capped at this value.
///   When `None`, the default formula `(90 - impact_angle) * 0.9` is used.
pub fn evaluate_yaw(
    velocity_ms: f64,
    projectile_mass_kg: f64,
    caliber_m: f64,
    armor_thickness_m: f64,
    impact_angle_deg: f64,
    armor_material: &str,
    projectile_type: &str,
    yaw_angle_deg: f64,
    ricochet_angle_deg: Option<f64>,
) -> PenetrationResult {
    let mat_factor = material_factor(armor_material);

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

    // ── Yaw multiplier ──────────────────────────────────────────────────────
    // Yaw at impact increases the presented cross-section, reducing penetration
    // effectiveness. Small yaw (< 5°) has minimal effect; large yaw (10–20°)
    // can reduce penetration by 30–50 %.
    // yaw_mult = exp(-k_yaw * yaw_deg), clamped to [0.5, 1.0]
    let k_yaw = yaw_coefficient(projectile_type);
    let yaw_mult = (-k_yaw * yaw_angle_deg).exp().clamp(0.5, 1.0);

    // ── Effective thickness ────────────────────────────────────────────────
    let base_effective = armor_thickness_m / cos_angle * mat_factor;
    // Caliber-to-thickness ratio effect: smaller calibers pen more efficiently
    // relative to their diameter against thin armor
    let cal_thick_ratio = caliber_m / armor_thickness_m.max(1e-6);
    let cal_factor = (1.0 + 0.3 * (-3.0 * cal_thick_ratio).exp()).min(1.0);
    // Yaw increases effective thickness: a yawed projectile presents a wider
    // cross-section, so the armour appears thicker.
    let effective_thickness = base_effective * cal_factor / yaw_mult;

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
        // Ricochet angle: use computed specular-ish angle, capped at per-projectile maximum if provided
        let computed_rico = (90.0 - impact_angle_deg) * 0.9;
        let ricochet_angle = match ricochet_angle_deg {
            Some(max_angle) if max_angle > 0.0 => computed_rico.min(max_angle),
            _ => computed_rico,
        };

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

    // ── Shatter check (AP vs hard ceramic/dual-hardness armor) ──────────────
    // AP projectiles can catastrophically shatter against hard armor at high
    // velocity. When shatter occurs the penetrator fractures and penetration
    // drops drastically — the projectile fails before the armor does.
    if check_shatter(
        velocity_ms,
        projectile_type,
        armor_material,
        impact_angle_deg,
    ) {
        let frag_result = crate::fragmentation::evaluate(
            velocity_ms,
            projectile_mass_kg * 1000.0,
            projectile_type,
            300.0,
            None,
        );
        let bad_shatter = behind_armor_debris::evaluate_bad(&BehindArmorDebrisParams {
            impact_velocity_ms: velocity_ms,
            projectile_mass_kg,
            caliber_m,
            armor_thickness_m,
            armor_material: armor_material.to_string(),
            impact_angle_deg,
            projectile_type: projectile_type.to_string(),
            projectile_fragments: frag_result.num_fragments.max(4),
            residual_velocity_ms: 0.0,
            penetrated: false,
        });
        return PenetrationResult {
            penetrated: false,
            residual_velocity: 0.0,
            effective_thickness,
            ricochet: false,
            ricochet_angle: 0.0,
            ricochet_energy_fraction: 0.0,
            fragments: (frag_result.num_fragments * 3).max(6), // catastrophic breakup
            spall_fragments: bad_shatter.num_spall_fragments,
            spall_cone_angle: bad_shatter.spall_cone_angle_deg,
            debris_spray_cone: bad_shatter.debris_spray_cone_deg,
            temp_cavity_diameter: bad_shatter.temp_cavity_diameter_mm,
            temp_cavity_volume: bad_shatter.temp_cavity_volume_cc,
        };
    }

    // ── De Marre penetration ───────────────────────────────────────────────
    // V_required = k * D^0.75 * T^0.7 / M^0.5
    // where k is the De Marre coefficient, calibrated per projectile type.
    // Lower k = more efficient penetration (requires less velocity).
    //
    // Simplified: if velocity exceeds De Marre threshold, penetration occurs
    let k = de_marre_k(projectile_type);

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

/// Lanz-Odermatt V3 correction for high L/D (>30) APFSDS penetrators.
///
/// Standard Lanz-Odermatt (1990) is calibrated for L/D 10–30. For modern
/// rods (M829A3: L/D ≈ 32, M829A4: L/D ≈ 34), the V3 correction applies
/// a reduced effective density factor:
///
/// ```text
/// V3_factor = (L/D)^(-0.05)
/// ```
///
/// which reduces penetration ~3–5% for L/D 30–35 rods by accounting for
/// reduced transverse confinement at extreme aspect ratios.
///
/// Returns `1.0` (no correction) for L/D ≤ 30.
///
/// Source: Odermatt, "Lanz-Odermatt Extended for High L/D", 2001.
pub fn lanz_odermatt_v3_factor(l_over_d: f64) -> f64 {
    if l_over_d > 30.0 {
        (l_over_d).powf(-0.05)
    } else {
        1.0
    }
}

// TODO: Integrate lanz_odermatt_v3_factor into the main penetration flow.
// Multiply the Lanz-Odermatt P/L result by the V3 factor when evaluating
// APFSDS rounds with known L/D. Currently lanz_odermatt_depth is a standalone
// helper; a future wrapper should accept rod L/D and apply the correction.

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn m80_ball_pens_5mm_rha_at_0deg() {
        let r = evaluate(
            853.0,
            0.0095,
            0.00762,
            0.005,
            0.0,
            "steel_rha",
            "ball",
            None,
        );
        assert!(r.penetrated, "M80 ball should pen 5mm RHA at 0°");
        assert!(r.residual_velocity > 100.0);
    }

    #[test]
    fn m80_ball_does_not_pen_20mm_rha() {
        let r = evaluate(
            853.0,
            0.0095,
            0.00762,
            0.020,
            0.0,
            "steel_rha",
            "ball",
            None,
        );
        assert!(!r.penetrated, "M80 ball should NOT pen 20mm RHA at 0°");
    }

    #[test]
    fn angle_reduces_penetration() {
        let r0 = evaluate(
            900.0,
            0.0095,
            0.00762,
            0.008,
            0.0,
            "steel_rha",
            "ball",
            None,
        );
        let r60 = evaluate(
            900.0,
            0.0095,
            0.00762,
            0.008,
            60.0,
            "steel_rha",
            "ball",
            None,
        );
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
        let ball = evaluate(
            880.0,
            0.0095,
            0.00762,
            0.010,
            0.0,
            "steel_rha",
            "ball",
            None,
        );
        let ap = evaluate(880.0, 0.0095, 0.00762, 0.010, 0.0, "steel_rha", "ap", None);
        assert!(
            ap.penetrated || !ball.penetrated,
            "AP should pen equal or better than ball"
        );
    }

    #[test]
    fn ricochet_at_shallow_angle() {
        let r = evaluate(
            850.0,
            0.0095,
            0.00762,
            0.010,
            80.0,
            "steel_rha",
            "ball",
            None,
        );
        assert!(r.ricochet, "80° impact should ricochet");
        assert!(r.ricochet_energy_fraction > 0.0);
    }

    #[test]
    fn custom_ricochet_angle_caps_default() {
        // At 80° impact, default ricochet angle = (90-80)*0.9 = 9.0°
        let default = evaluate_yaw(
            850.0,
            0.0095,
            0.00762,
            0.010,
            80.0,
            "steel_rha",
            "ball",
            0.0,
            None,
        );
        assert!(default.ricochet);
        let default_angle = default.ricochet_angle;
        assert!(
            (default_angle - 9.0).abs() < 1e-6,
            "Default ricochet angle should be 9.0°, got {default_angle}"
        );

        // With custom max of 5.0°, the angle should be capped
        let capped = evaluate_yaw(
            850.0,
            0.0095,
            0.00762,
            0.010,
            80.0,
            "steel_rha",
            "ball",
            0.0,
            Some(5.0),
        );
        assert!(capped.ricochet);
        assert!(
            (capped.ricochet_angle - 5.0).abs() < 1e-6,
            "Capped ricochet angle should be 5.0°, got {}",
            capped.ricochet_angle
        );
        assert!(
            capped.ricochet_angle < default_angle,
            "Capped angle ({}) should be less than default ({})",
            capped.ricochet_angle,
            default_angle
        );
    }

    #[test]
    fn hha_is_harder_than_rha() {
        let rha = evaluate(
            900.0,
            0.0095,
            0.00762,
            0.010,
            0.0,
            "steel_rha",
            "ball",
            None,
        );
        let hha = evaluate(
            900.0,
            0.0095,
            0.00762,
            0.010,
            0.0,
            "steel_hha",
            "ball",
            None,
        );
        assert!(hha.effective_thickness > rha.effective_thickness);
    }

    #[test]
    fn penetration_produces_fragments() {
        let r = evaluate(
            900.0,
            0.0095,
            0.00762,
            0.006,
            0.0,
            "steel_rha",
            "ball",
            None,
        );
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
        let slow = evaluate(
            400.0,
            0.0095,
            0.00762,
            0.005,
            0.0,
            "steel_rha",
            "ball",
            None,
        );
        let fast = evaluate(
            900.0,
            0.0095,
            0.00762,
            0.005,
            0.0,
            "steel_rha",
            "ball",
            None,
        );
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

    // ── Yaw at impact ─────────────────────────────────────────────────────────

    #[test]
    fn zero_yaw_matches_evaluate() {
        let base = super::evaluate(
            853.0,
            0.0095,
            0.00762,
            0.005,
            0.0,
            "steel_rha",
            "ball",
            None,
        );
        let yaw = super::evaluate_yaw(
            853.0,
            0.0095,
            0.00762,
            0.005,
            0.0,
            "steel_rha",
            "ball",
            0.0,
            None,
        );
        assert_eq!(base.penetrated, yaw.penetrated);
        assert!((base.residual_velocity - yaw.residual_velocity).abs() < 1e-6);
        assert!((base.effective_thickness - yaw.effective_thickness).abs() < 1e-6);
    }

    #[test]
    fn yaw_10deg_reduces_penetration() {
        let r0 = super::evaluate_yaw(
            853.0,
            0.0095,
            0.00762,
            0.005,
            0.0,
            "steel_rha",
            "ball",
            0.0,
            None,
        );
        let r10 = super::evaluate_yaw(
            853.0,
            0.0095,
            0.00762,
            0.005,
            0.0,
            "steel_rha",
            "ball",
            10.0,
            None,
        );
        // 10° yaw reduces residual velocity (more energy spent overcoming yaw)
        assert!(
            r10.residual_velocity <= r0.residual_velocity + 1e-6,
            "10° yaw should not increase residual velocity: {} vs {}",
            r10.residual_velocity,
            r0.residual_velocity
        );
        // Effective thickness should be higher with yaw (projectile sees more armour)
        assert!(
            r10.effective_thickness > r0.effective_thickness,
            "10° yaw should increase effective thickness: {} vs {}",
            r10.effective_thickness,
            r0.effective_thickness
        );
    }

    #[test]
    fn yaw_20deg_reduces_pen_more_than_10deg() {
        let r10 = super::evaluate_yaw(
            900.0,
            0.0095,
            0.00762,
            0.008,
            0.0,
            "steel_rha",
            "ball",
            10.0,
            None,
        );
        let r20 = super::evaluate_yaw(
            900.0,
            0.0095,
            0.00762,
            0.008,
            0.0,
            "steel_rha",
            "ball",
            20.0,
            None,
        );
        assert!(
            r20.effective_thickness > r10.effective_thickness,
            "20° yaw should give thicker effective thickness than 10°: {} vs {}",
            r20.effective_thickness,
            r10.effective_thickness
        );
    }

    #[test]
    fn yaw_alone_does_not_cause_ricochet() {
        // High yaw but normal impact angle — yaw alone shouldn't cause ricochet
        let r = super::evaluate_yaw(
            853.0,
            0.0095,
            0.00762,
            0.010,
            0.0,
            "steel_rha",
            "ball",
            20.0,
            None,
        );
        assert!(
            !r.ricochet,
            "Yaw alone (20°) at 0° impact angle should not cause ricochet"
        );
    }

    // ── Shatter ───────────────────────────────────────────────────────────────

    #[test]
    fn ap_round_shatters_on_ceramic_at_high_vel() {
        assert!(super::check_shatter(850.0, "ap", "ceramic_b4c", 0.0));
    }

    #[test]
    fn ball_round_does_not_shatter_on_ceramic() {
        assert!(!super::check_shatter(850.0, "ball", "ceramic_b4c", 0.0));
    }

    #[test]
    fn ap_round_does_not_shatter_on_rha() {
        assert!(!super::check_shatter(850.0, "ap", "steel_rha", 0.0));
    }

    #[test]
    fn ap_round_does_not_shatter_below_threshold() {
        assert!(!super::check_shatter(600.0, "ap", "ceramic_b4c", 0.0));
    }

    #[test]
    fn ap_round_does_not_shatter_at_oblique_angle() {
        assert!(!super::check_shatter(850.0, "ap", "ceramic_b4c", 70.0));
    }

    #[test]
    fn ap_round_shatters_on_dual_hardness_steel() {
        assert!(super::check_shatter(
            850.0,
            "ap",
            "dual_hardness_steel",
            0.0
        ));
    }

    #[test]
    fn ap_round_shatters_on_mars_armor() {
        assert!(super::check_shatter(
            850.0,
            "armor_piercing",
            "mars_armor",
            0.0
        ));
    }

    #[test]
    fn ap_round_shatters_on_sic() {
        assert!(super::check_shatter(800.0, "ap", "ceramic_sic", 30.0));
    }

    #[test]
    fn ap_round_shatters_on_ad95() {
        assert!(super::check_shatter(750.0, "ap", "ad95", 0.0));
    }

    #[test]
    fn evaluate_yaw_returns_shatter_result() {
        // AP vs ceramic at 900 m/s, 0° → should shatter (penetrated=false)
        let r = super::evaluate_yaw(
            900.0,
            0.0095,
            0.00762,
            0.010,
            0.0,
            "ceramic_b4c",
            "ap",
            0.0,
            None,
        );
        assert!(
            !r.penetrated,
            "AP vs ceramic at 900 m/s should shatter and not penetrate"
        );
        // Shatter produces lots of fragments
        assert!(r.fragments >= 6, "Shatter should produce >= 6 fragments");
        assert!(
            (r.residual_velocity - 0.0).abs() < 1e-6,
            "Shatter should leave zero residual velocity"
        );
    }

    #[test]
    fn evaluate_yaw_ap_does_not_shatter_on_rha() {
        // AP vs RHA at 900 m/s — no shatter, normal penetration logic
        let r = super::evaluate_yaw(
            900.0,
            0.0095,
            0.00762,
            0.010,
            0.0,
            "steel_rha",
            "ap",
            0.0,
            None,
        );
        // Whether it pens or not is up to De Marre, but shatter should not intervene.
        // Against RHA (non-ceramic) the shatter check returns false, so we
        // should NOT see the shatter signature: residual velocity > 0.
        assert!(
            r.residual_velocity > 0.0,
            "Non-shatter AP should retain residual velocity: got {}",
            r.residual_velocity
        );
    }

    #[test]
    fn check_shatter_armor_piercing_string() {
        assert!(super::check_shatter(
            800.0,
            "armor_piercing",
            "ceramic_al2o3",
            0.0
        ));
    }

    // ── Interface Defeat ─────────────────────────────────────────────────────

    #[test]
    fn interface_defeat_ap_vs_b4c_with_uhmwpe_backing() {
        assert!(super::check_interface_defeat(
            800.0,
            "ap",
            "ceramic_b4c",
            "uhmwpe",
            0.00762,
            0.010
        ));
    }

    #[test]
    fn interface_defeat_apds_vs_sic_with_aluminum_backing() {
        assert!(super::check_interface_defeat(
            700.0,
            "apds",
            "ceramic_sic",
            "aluminum_5083",
            0.00762,
            0.010
        ));
    }

    #[test]
    fn interface_defeat_fails_for_ball_ammo() {
        assert!(!super::check_interface_defeat(
            800.0,
            "ball",
            "ceramic_b4c",
            "uhmwpe",
            0.00762,
            0.010
        ));
    }

    #[test]
    fn interface_defeat_fails_without_ductile_backing() {
        assert!(!super::check_interface_defeat(
            800.0,
            "ap",
            "ceramic_b4c",
            "concrete",
            0.00762,
            0.010
        ));
    }

    #[test]
    fn interface_defeat_fails_above_critical_velocity() {
        assert!(!super::check_interface_defeat(
            1600.0,
            "ap",
            "ceramic_b4c",
            "uhmwpe",
            0.00762,
            0.010
        ));
    }

    #[test]
    fn interface_defeat_fails_when_ceramic_too_thin() {
        // caliber = 7.62mm, thickness = 3mm < 0.5 * 7.62 = 3.81mm
        assert!(!super::check_interface_defeat(
            800.0,
            "ap",
            "ceramic_b4c",
            "uhmwpe",
            0.00762,
            0.003
        ));
    }

    #[test]
    fn interface_defeat_works_with_ad95() {
        assert!(super::check_interface_defeat(
            750.0,
            "armor_piercing",
            "ad95",
            "steel_rha",
            0.0127,
            0.012
        ));
    }

    #[test]
    fn interface_defeat_apcr_vs_b4c() {
        assert!(super::check_interface_defeat(
            600.0,
            "apcr",
            "b4c_tile",
            "aluminum_7039",
            0.00762,
            0.006
        ));
    }

    #[test]
    fn interface_defeat_fails_on_rha_face() {
        assert!(!super::check_interface_defeat(
            800.0,
            "ap",
            "steel_rha",
            "uhmwpe",
            0.00762,
            0.010
        ));
    }

    #[test]
    fn interface_defeat_multiplier_range() {
        let m_low = super::interface_defeat_penetration_multiplier(300.0);
        let m_high = super::interface_defeat_penetration_multiplier(900.0);
        assert!(m_low >= 0.4 && m_low <= 0.7);
        assert!(m_high >= 0.4 && m_high <= 0.7);
        assert!(
            m_low < m_high,
            "Lower velocity should give stronger reduction"
        );
    }

    #[test]
    fn interface_defeat_multiplier_clamps() {
        let below = super::interface_defeat_penetration_multiplier(-100.0);
        let above = super::interface_defeat_penetration_multiplier(2000.0);
        assert!((below - 0.4).abs() < 1e-12, "below={below}");
        assert!((above - 0.7).abs() < 1e-12, "above={above}");
    }

    #[test]
    fn interface_defeat_apfsds_with_dyneema_backing() {
        assert!(super::check_interface_defeat(
            850.0,
            "apfsds",
            "sic_ceramic",
            "dyneema_liner",
            0.00762,
            0.010
        ));
    }

    #[test]
    fn interface_defeat_kevlar_backing() {
        assert!(super::check_interface_defeat(
            800.0,
            "armor_piercing",
            "ceramic_b4c",
            "kevlar_liner",
            0.00762,
            0.010
        ));
    }

    #[test]
    fn interface_defeat_velocity_boundary_just_below() {
        assert!(super::check_interface_defeat(
            899.999,
            "ap",
            "b4c",
            "steel_rha",
            0.00762,
            0.010
        ));
    }

    #[test]
    fn yaw_multiplier_bounds() {
        // Verify the yaw multiplier is in [0.5, 1.0]
        let r0 = super::evaluate_yaw(
            853.0,
            0.0095,
            0.00762,
            0.005,
            0.0,
            "steel_rha",
            "ball",
            0.0,
            None,
        );
        let r50 = super::evaluate_yaw(
            853.0,
            0.0095,
            0.00762,
            0.005,
            0.0,
            "steel_rha",
            "ball",
            50.0,
            None,
        );
        // At 50° yaw, the 50% cap should apply — effective thickness should be at
        // most 2× the 0-yaw value (1/0.5 = 2).
        let ratio = r50.effective_thickness / r0.effective_thickness;
        assert!(
            (ratio - 2.0).abs() < 0.01,
            "50° yaw should cap at 2× effective thickness, got ratio={}",
            ratio
        );
    }

    // ── erode_projectile_mass ─────────────────────────────────────────────────

    #[test]
    fn erosion_ceramic_more_than_steel() {
        let steel_loss = super::erode_projectile_mass(850.0, 0.0095, 0.00762, "steel_rha", 0.010);
        let ceramic_loss =
            super::erode_projectile_mass(850.0, 0.0095, 0.00762, "ceramic_b4c", 0.010);
        assert!(
            ceramic_loss > steel_loss,
            "Ceramic should erode more projectile than steel: {ceramic_loss} vs {steel_loss}"
        );
    }

    #[test]
    fn erosion_fraction_in_expected_range() {
        // M80 ball through 10mm RHA at 850 m/s → 5–20 %
        let loss = super::erode_projectile_mass(850.0, 0.0095, 0.00762, "steel_rha", 0.010);
        let fraction = loss / 0.0095;
        assert!(
            fraction >= 0.05 && fraction <= 0.20,
            "Steel erosion fraction should be 0.05–0.20, got {fraction:.3}"
        );
    }

    #[test]
    fn erosion_ceramic_fraction_in_expected_range() {
        // AP through 15mm B4C at 900 m/s → 15–40 %
        let loss = super::erode_projectile_mass(900.0, 0.0040, 0.00556, "ceramic_b4c", 0.015);
        let fraction = loss / 0.0040;
        assert!(
            fraction >= 0.15 && fraction <= 0.40,
            "Ceramic erosion fraction should be 0.15–0.40, got {fraction:.3}"
        );
    }

    #[test]
    fn erosion_higher_velocity_more_mass_loss() {
        let slow = super::erode_projectile_mass(400.0, 0.0095, 0.00762, "steel_rha", 0.010);
        let fast = super::erode_projectile_mass(900.0, 0.0095, 0.00762, "steel_rha", 0.010);
        assert!(
            fast > slow,
            "Higher velocity should cause more erosion: {fast} vs {slow}"
        );
    }

    #[test]
    fn erosion_zero_inputs_return_zero() {
        assert_eq!(
            super::erode_projectile_mass(0.0, 0.0095, 0.00762, "steel_rha", 0.010),
            0.0
        );
        assert_eq!(
            super::erode_projectile_mass(850.0, 0.0, 0.00762, "steel_rha", 0.010),
            0.0
        );
        assert_eq!(
            super::erode_projectile_mass(850.0, 0.0095, 0.00762, "steel_rha", 0.0),
            0.0
        );
    }

    #[test]
    fn erosion_never_exceeds_mass() {
        let loss = super::erode_projectile_mass(2000.0, 0.001, 0.00556, "ceramic_b4c", 0.050);
        assert!(
            loss <= 0.001 + 1e-12,
            "Erosion should not exceed projectile mass: {loss} > 0.001"
        );
    }

    #[test]
    fn erosion_deeper_pen_more_loss() {
        let shallow = super::erode_projectile_mass(850.0, 0.0095, 0.00762, "steel_rha", 0.005);
        let deep = super::erode_projectile_mass(850.0, 0.0095, 0.00762, "steel_rha", 0.020);
        assert!(
            deep >= shallow,
            "Deeper penetration should cause at least as much erosion"
        );
    }
}
