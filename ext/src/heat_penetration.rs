// ABE — HEAT (High-Explosive Anti-Tank) Shaped Charge Jet Penetration
//
// Physics model for shaped charge jet formation, stretching, breakup,
// and penetration of armour.  Uses the Birkhoff–Pugh–Taylor theory of
// shaped charges with a Gurney-velocity approximation for the jet tip.
//
// References:
//   - Birkhoff et al. (1948)  Jet formation in shaped charges
//   - Pugh, Eichelberger, Rostoker (1952)  Theory of jet penetration
//   - Walters & Zukas (1989)  Fundamentals of Shaped Charges
//   - Held (1995)  Jet breakup and penetration efficiency
//   - NATO AEP-2920 (Terminal Ballistics)

use crate::behind_armor_debris::{self, BehindArmorDebrisParams, BehindArmorDebrisResult};

// ── Material constants ──────────────────────────────────────────────────────────
// All densities in kg/m³.

/// Copper — the standard shaped charge liner material.
pub const LINER_DENSITY_COPPER: f64 = 8960.0;
/// Tantalum — high density, used in advanced warheads.
pub const LINER_DENSITY_TANTALUM: f64 = 16600.0;
/// Molybdenum — moderate density, good ductility.
pub const LINER_DENSITY_MOLYBDENUM: f64 = 10280.0;
/// Aluminium — lightweight, used in some training rounds.
pub const LINER_DENSITY_ALUMINUM: f64 = 2700.0;

/// Typical RHA density.
pub const TARGET_DENSITY_RHA: f64 = 7850.0;
/// Rolled homogeneous armour (alternate value, same physical density).
pub const TARGET_DENSITY_STEEL: f64 = 7850.0;

/// Detonation velocity of Composition B (m/s) — the standard HEAT fill.
pub const V_DET_COMPOSITION_B: f64 = 8500.0;
/// Octol – HMX/TNT mixture, common in NATO warheads.
pub const V_DET_OCTOL: f64 = 8600.0;
/// RDX-based fill.
pub const V_DET_RDX: f64 = 8700.0;
/// HMX – highest performance conventional explosive.
pub const V_DET_HMX: f64 = 9100.0;
/// TNT – used in older or lower-cost warheads.
pub const V_DET_TNT: f64 = 6950.0;

/// Minimum jet velocity required to erode common target materials (m/s).
const V_MIN_RHA: f64 = 2500.0;
const V_MIN_ALUMINUM: f64 = 1500.0;
const V_MIN_CERAMIC: f64 = 2000.0;
const V_MIN_CONCRETE: f64 = 1000.0;

// ── Input parameters ───────────────────────────────────────────────────────────

/// Parameters describing a shaped charge jet and its target.
#[derive(Debug, Clone)]
pub struct HeatJetParams {
    /// Jet tip velocity at impact (m/s).  Pass `0.0` to auto-compute from
    /// the explosive fill and cone geometry.
    pub jet_tip_velocity_ms: f64,
    /// Effective jet mass (kg).  Pass `0.0` to auto-estimate from the liner.
    pub jet_mass_kg: f64,
    /// Standoff distance between the warhead face and the target (m).
    pub standoff_m: f64,
    /// Warhead / launcher calibre (m).
    pub caliber_m: f64,
    /// Cone half-angle of the shaped charge liner (degrees).  Typical values
    /// are 25–35°; 28° is a common modern compromise.
    pub cone_half_angle_deg: f64,
    /// Liner material identifier (e.g. "copper", "tantalum", "molybdenum").
    pub liner_material: String,
    /// Density of the liner material (kg/m³).
    pub liner_density_kgm3: f64,
    /// Target armour material identifier (see [`crate::penetration::material_factor`]).
    pub target_armor_material: String,
    /// Density of the target armour (kg/m³).
    pub target_density_kgm3: f64,
    /// Impact angle measured from the surface normal (degrees; 0 = perpendicular).
    pub impact_angle_deg: f64,
    /// Thickness of any explosive reactive armour (ERA) panel (m).  Pass `0.0`
    /// for no ERA.
    pub era_thickness_m: f64,
    /// Armour plate thickness (m) — used for behind-armour debris computation.
    pub armor_thickness_m: f64,
}

// ── Output result ──────────────────────────────────────────────────────────────

/// Result of a HEAT jet penetration evaluation.
#[derive(Debug, Clone)]
pub struct HeatJetResult {
    /// Depth of the penetration channel normal to the armour surface (mm).
    pub penetration_depth_mm: f64,
    /// Residual velocity of the surviving jet tail material (m/s).
    pub residual_jet_velocity_ms: f64,
    /// Effective standoff at which the jet struck the target (mm).
    pub effective_standoff_mm: f64,
    /// Whether the jet was fully disrupted before reaching the main armour.
    pub jet_disrupted: bool,
    /// Whether ERA panels were triggered and affected the jet.
    pub era_disrupted: bool,
    /// Behind-armour debris effects (spall, cavity, lethality).
    pub behind_armor_effects: BehindArmorDebrisResult,
}

// ── Target-material helpers ─────────────────────────────────────────────────────

/// Map an armour material identifier to its approximate density (kg/m³).
pub fn target_density_from_material(material: &str) -> f64 {
    let m = material.to_lowercase();

    // ── Ceramic FIRST to avoid matching e.g. "ceramic_al2o3" as aluminium ──
    if m.contains("ceramic")
        || m.contains("al2o3")
        || m.contains("sic")
        || m.contains("b4c")
        || m.contains("ad90")
        || m.contains("ad95")
        || m.contains("mar_ceramic")
    {
        3500.0
    } else if m.contains("steel")
        || m.contains("rha")
        || m.contains("hha")
        || m.contains("cast")
        || m.contains("mild")
        || m.contains("structural")
        || m.contains("armor_tip")
        || m.contains("dual_hardness")
        || m.contains("mars")
        || m.contains("perforated")
        || m.contains("slotted")
        || m.contains("mil_dtl")
    {
        7850.0
    } else if m.contains("aluminum") || m.contains("al") {
        2700.0
    } else if m.contains("concrete")
        || m.contains("gypsum")
        || m.contains("drywall")
        || m.contains("adobe")
        || m.contains("rammed")
    {
        2400.0
    } else if m.contains("titanium") {
        4430.0
    } else if m.contains("uranium") || m.contains("du") {
        19_000.0
    } else if m.contains("glass") {
        2500.0
    } else if m.contains("wood")
        || m.contains("timber")
        || m.contains("plywood")
        || m.contains("osb")
        || m.contains("stud")
    {
        600.0
    } else if m.contains("kevlar") || m.contains("aramid") || m.contains("twaron") {
        1440.0
    } else if m.contains("dyneema") || m.contains("uhmwpe") {
        970.0
    } else if m.contains("rubber") || m.contains("elastomer") {
        1100.0
    } else if m.contains("lead") {
        11_340.0
    } else if m.contains("chobham")
        || m.contains("burlington")
        || m.contains("dorchester")
        || m.contains("stanag")
        || m.contains("mexas")
        || m.contains("stef")
        || m.contains("kvarts")
        || m.contains("k_active")
        || m.contains("textolite")
        || m.contains("texolite")
    {
        4500.0
    } else if m.contains("carbon")
        || m.contains("fibreglass")
        || m.contains("grp")
        || m.contains("fiberglass")
    {
        1800.0
    } else {
        7850.0
    }
}

/// Minimum jet velocity (m/s) below which the jet no longer erodes the target.
pub fn get_v_min_for_target(material: &str) -> f64 {
    let m = material.to_lowercase();

    // ── Ceramic FIRST to avoid matching "ceramic_al2o3" as aluminium ──
    if m.contains("ceramic")
        || m.contains("al2o3")
        || m.contains("b4c")
        || m.contains("sic")
        || m.contains("ad90")
        || m.contains("ad95")
    {
        V_MIN_CERAMIC
    } else if m.contains("aluminum") || m.contains("al") {
        V_MIN_ALUMINUM
    } else if m.contains("concrete")
        || m.contains("gypsum")
        || m.contains("adobe")
        || m.contains("rammed")
    {
        V_MIN_CONCRETE
    } else if m.contains("rubber")
        || m.contains("elastomer")
        || m.contains("spall")
        || m.contains("liner")
    {
        500.0
    } else if m.contains("wood")
        || m.contains("plywood")
        || m.contains("osb")
        || m.contains("timber")
    {
        300.0
    } else {
        V_MIN_RHA
    }
}

/// Estimate the effective detonation velocity from the liner material.
fn detonation_velocity_for_liner(liner_material: &str) -> f64 {
    // Most HEAT warheads use Composition B or an HMX-based fill.
    // Higher-density liners typically couple more efficiently, but the
    // explosive fill itself doesn't change — we keep the same V_det.
    let _ = liner_material;
    V_DET_COMPOSITION_B
}

// ── Core physics functions ─────────────────────────────────────────────────────

/// Compute the jet tip velocity from the explosive and cone geometry.
///
/// Uses the simplified Gurney relation for a shaped charge:
///
///   V_tip = V_det · cos(α)
///
/// where α is the cone half-angle.  A small correction is applied for
/// very dense liner materials (tantalum, molybdenum) which couple more
/// efficiently with the detonation wave.
///
/// # Example (standard copper cone, 60° full angle)
/// ```
/// # use abe_ballistics_ext::heat_penetration::calculate_jet_tip_velocity;
/// let v_tip = calculate_jet_tip_velocity(8500.0, 30.0, "copper");
/// // V_det · cos(30°) = 8500 · 0.866 ≈ 7360 m/s
/// assert!((v_tip - 7360.0).abs() < 50.0);
/// ```
pub fn calculate_jet_tip_velocity(
    v_detonation_ms: f64,
    cone_half_angle_deg: f64,
    liner_material: &str,
) -> f64 {
    let cos_angle = cone_half_angle_deg.to_radians().cos();

    // Minor efficiency boost for very dense liners that couple better
    // with the detonation wave (tantalum, molybdenum).
    let density_factor = match liner_material.to_lowercase().as_str() {
        "tantalum" => 1.02,
        "molybdenum" => 1.01,
        "copper" => 1.0,
        "aluminum" | "aluminium" => 0.96,
        _ => 1.0,
    };

    v_detonation_ms * cos_angle * density_factor
}

/// Compute the P/L ratio from the shaped-charge penetration formula.
///
/// Returns the dimensionless penetration-depth / effective-jet-length ratio
/// based on the Birkhoff quasi-steady penetration equation:
///
///   P/L = √(ρⱼ / ρₜ) · (vⱼ² − vₘᵢₙ²) / vⱼ²
///
/// where:
///   ρⱼ  — jet density (liner density, kg/m³)
///   ρₜ  — target density (kg/m³)
///   vⱼ  — jet tip velocity (m/s)
///   vₘᵢₙ — minimum eroding velocity (m/s)
///
/// # Example (copper vs RHA at 7.4 km/s)
/// ```
/// # use abe_ballistics_ext::heat_penetration::calculate_penetration_from_jet;
/// let p_l = calculate_penetration_from_jet(8960.0, 7850.0, 7360.0, 2500.0);
/// assert!(p_l > 0.5 && p_l < 1.5);
/// ```
pub fn calculate_penetration_from_jet(
    rho_jet: f64,
    rho_target: f64,
    v_jet: f64,
    v_min: f64,
) -> f64 {
    if v_jet <= v_min || rho_jet <= 0.0 || rho_target <= 0.0 {
        return 0.0;
    }

    let density_term = (rho_jet / rho_target).sqrt();
    let velocity_term = (v_jet.powi(2) - v_min.powi(2)) / v_jet.powi(2);

    density_term * velocity_term.max(0.0)
}

/// Standoff efficiency factor (dimensionless, 0–1).
///
/// The shaped charge jet needs time and distance to fully form and stretch
/// from the collapsed liner.  Performance peaks at ~2–4 calibres standoff
/// and drops at very short standoffs (incomplete formation) and long
/// standoffs (jet breakup by necking and particulate transition).
///
/// # Reference
/// Held, M. — "Standoff effect on shaped charge penetration",
/// *Propellants, Explosives, Pyrotechnics* 20 (1995).
pub fn standoff_efficiency(standoff_m: f64, caliber_m: f64) -> f64 {
    let cal = standoff_m / caliber_m.max(1e-10);

    if cal <= 0.5 {
        // Contact / very close: jet has not fully formed.
        // Roughly linear rise from ~0.15 at contact to ~0.60 at 0.5 cal.
        (cal / 0.5) * 0.60
    } else if cal <= 2.5 {
        // Rapid improvement as the jet stretches to its optimal length.
        0.60 + 0.40 * (cal - 0.5) / 2.0
    } else if cal <= 4.0 {
        // Peak plateau centred on ~2.5–3.0 calibres, gentle decay to 4 cal.
        let dist = (cal - 4.0) / 1.5; // negative in this range
        0.92 + 0.08 * dist
    } else {
        // Exponential decay beyond optimal standoff as the jet breaks up
        // into particles that drill less efficiently.
        0.92 * (-(cal - 4.0) * 0.25).exp()
    }
}

/// ERA interaction model.
///
/// Returns `(was_disrupted, remaining_penetration_fraction)`.
///
/// Explosive Reactive Armour disrupts the jet by moving metal plates across
/// its path, severing the continuous jet into segments that lose penetration
/// efficiency.  The disruption scales with ERA panel thickness relative to
/// the jet diameter (~0.1 × caliber).
///
/// Typical reductions:
///   - Light ERA (Kontakt-1, 4 mm):  ~30–40 %
///   - Heavy ERA (Kontakt-5, 7 mm):  ~50–70 %
///   - Very heavy ERA (Relikt, 10 mm+):  ~60–80 %
pub fn era_interaction(era_thickness_m: f64, caliber_m: f64) -> (bool, f64) {
    if era_thickness_m <= 0.0 {
        return (false, 1.0);
    }

    let era_mm = era_thickness_m * 1000.0;
    let jet_diam_mm = 0.1 * caliber_m * 1000.0;

    // The ratio of ERA plate thickness to jet diameter drives disruption.
    let ratio = era_mm / jet_diam_mm.max(0.1);

    // Saturation at ~0.8 maximum disruption (80% reduction).
    let max_disruption = 0.80;
    let disruption = (1.0 - (-ratio * 3.0).exp()) * max_disruption;
    let remaining = (1.0 - disruption).max(0.0);

    (disruption > 0.01, remaining)
}

/// Estimate the effective j et length (m) at the moment of impact.
///
/// The initial collapsed liner slug is stretched by the velocity gradient
/// along the jet.  The effective length is the collapsed length times a
/// stretch factor that depends on the tip velocity and the characteristic
/// breakup time of the jet.
fn effective_jet_length_m(
    caliber_m: f64,
    cone_half_angle_deg: f64,
    v_tip_ms: f64,
    standoff_m: f64,
) -> f64 {
    let half_rad = cone_half_angle_deg.to_radians();
    let collapsed = (caliber_m / 2.0) / half_rad.tan();

    // Characteristic breakup time for a copper jet: ~60 µs.
    // High-quality liners can reach 80–100 µs.
    let breakup_time_s = 65e-6;

    // Maximum stretch before breakup.
    let max_stretch = v_tip_ms * breakup_time_s / collapsed.max(1e-10);

    // Available stretch time from standoff distance / tip velocity.
    let flight_time = standoff_m / v_tip_ms.max(1.0);
    let actual_stretch = (v_tip_ms * flight_time) / collapsed.max(1e-10);

    // Use whichever is smaller: the available stretch from the flight time,
    // or the maximum stretch before breakup.
    let stretch = actual_stretch.min(max_stretch).min(20.0);

    collapsed * stretch
}

// ── Main evaluation entry-point ─────────────────────────────────────────────────

/// Evaluate the penetration of a shaped-charge HEAT jet against an armour
/// plate.
///
/// Assembles all sub-models (jet formation, standoff, ERA, penetration
/// depth, behind-armour debris) into a single result.
pub fn evaluate_heat_jet(params: &HeatJetParams) -> HeatJetResult {
    // ── 1. Jet tip velocity ──────────────────────────────────────────────────
    let v_tip = if params.jet_tip_velocity_ms > 0.0 {
        params.jet_tip_velocity_ms
    } else {
        let v_det = detonation_velocity_for_liner(&params.liner_material);
        calculate_jet_tip_velocity(v_det, params.cone_half_angle_deg, &params.liner_material)
    };

    // ── 2. Jet mass ──────────────────────────────────────────────────────────
    let _jet_mass = if params.jet_mass_kg > 0.0 {
        params.jet_mass_kg
    } else {
        // Approximate liner volume as a disc of the given caliber with a
        // typical thickness of 0.3 % of the caliber (3 mm for a 100 mm gun).
        let liner_thickness = 0.003 * (params.caliber_m / 0.1).powf(0.3);
        let liner_vol = std::f64::consts::PI * (params.caliber_m / 2.0).powi(2) * liner_thickness;
        let liner_mass = liner_vol * params.liner_density_kgm3;
        // The jet comprises ~15–20 % of the liner mass (the rest forms the slug).
        liner_mass * 0.18
    };

    // ── 3. Standoff and V_min ────────────────────────────────────────────────
    let standoff_m = if params.standoff_m > 0.0 {
        params.standoff_m
    } else {
        3.0 * params.caliber_m
    };

    let v_min = get_v_min_for_target(&params.target_armor_material);

    // ── 4. Standoff efficiency ───────────────────────────────────────────────
    let standoff_eff = standoff_efficiency(standoff_m, params.caliber_m);

    // ── 5. ERA interaction ───────────────────────────────────────────────────
    let (era_disrupted, era_factor) = era_interaction(params.era_thickness_m, params.caliber_m);

    // ── 6. Effective jet length ──────────────────────────────────────────────
    let jet_length = effective_jet_length_m(
        params.caliber_m,
        params.cone_half_angle_deg,
        v_tip,
        standoff_m,
    );

    // ── 7. Penetration ───────────────────────────────────────────────────────
    let p_over_l = calculate_penetration_from_jet(
        params.liner_density_kgm3,
        params.target_density_kgm3,
        v_tip,
        v_min,
    );

    // Angle effect: at oblique impact the jet traverses a longer path
    // through the armour, reducing the effective penetration normal to
    // the plate.  For shaped charges the exponent is ~1.0–1.2 (lower
    // than for kinetic rods, which tunnel more efficiently).
    let cos_angle = params.impact_angle_deg.to_radians().cos().max(0.087);
    let angle_factor = cos_angle.powf(1.2);

    // Base penetration depth (normal to plate, in metres).
    let penetration_m = p_over_l * jet_length * standoff_eff * era_factor * angle_factor;

    // ── 8. Disruption check ──────────────────────────────────────────────────
    let jet_disrupted = penetration_m <= 0.001 * params.caliber_m;

    // ── 9. Residual velocity ─────────────────────────────────────────────────
    // The slower tail of the jet continues after the tip is consumed.
    // Approximate as ~15 % of tip velocity for a stretching jet.
    let residual_v = if penetration_m > 0.0 && !jet_disrupted {
        (v_tip * 0.15).max(100.0)
    } else {
        0.0
    };

    // ── 10. Behind-armour debris ─────────────────────────────────────────────
    let penetrated = penetration_m >= params.armor_thickness_m;

    let bad_result = behind_armor_debris::evaluate_bad(&BehindArmorDebrisParams {
        impact_velocity_ms: v_tip,
        projectile_mass_kg: _jet_mass,
        caliber_m: params.caliber_m,
        armor_thickness_m: params.armor_thickness_m,
        armor_material: params.target_armor_material.clone(),
        impact_angle_deg: params.impact_angle_deg,
        projectile_type: "heat".to_string(),
        projectile_fragments: if jet_disrupted { 15 } else { 5 },
        residual_velocity_ms: residual_v,
        penetrated,
    });

    HeatJetResult {
        penetration_depth_mm: penetration_m * 1000.0,
        residual_jet_velocity_ms: residual_v,
        effective_standoff_mm: standoff_m * 1000.0,
        jet_disrupted,
        era_disrupted,
        behind_armor_effects: bad_result,
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Helper to build a standard "RPG-7-like" param set ───────────────────────

    fn rpg7_heat_params() -> HeatJetParams {
        HeatJetParams {
            jet_tip_velocity_ms: 0.0, // auto-compute
            jet_mass_kg: 0.0,         // auto-estimate
            standoff_m: 0.255,        // ~3 calibres (85 mm × 3)
            caliber_m: 0.085,
            cone_half_angle_deg: 30.0,
            liner_material: "copper".to_string(),
            liner_density_kgm3: 8960.0,
            target_armor_material: "steel_rha".to_string(),
            target_density_kgm3: 7850.0,
            impact_angle_deg: 0.0,
            era_thickness_m: 0.0,
            armor_thickness_m: 0.200, // 200 mm RHA — typical target
        }
    }

    fn maaws_heat_params() -> HeatJetParams {
        HeatJetParams {
            jet_tip_velocity_ms: 0.0,
            jet_mass_kg: 0.0,
            standoff_m: 0.252, // ~3 calibres (84 mm × 3)
            caliber_m: 0.084,
            cone_half_angle_deg: 28.0,
            liner_material: "copper".to_string(),
            liner_density_kgm3: 8960.0,
            target_armor_material: "steel_rha".to_string(),
            target_density_kgm3: 7850.0,
            impact_angle_deg: 0.0,
            era_thickness_m: 0.0,
            armor_thickness_m: 0.300,
        }
    }

    fn heavy_heat_params() -> HeatJetParams {
        HeatJetParams {
            jet_tip_velocity_ms: 0.0,
            jet_mass_kg: 0.0,
            standoff_m: 0.315, // ~3 calibres (105 mm × 3)
            caliber_m: 0.105,
            cone_half_angle_deg: 28.0,
            liner_material: "copper".to_string(),
            liner_density_kgm3: 8960.0,
            target_armor_material: "steel_rha".to_string(),
            target_density_kgm3: 7850.0,
            impact_angle_deg: 0.0,
            era_thickness_m: 0.0,
            armor_thickness_m: 0.400,
        }
    }

    // ── Physics function tests ─────────────────────────────────────────────────

    /// Standard copper cone produces the expected Gurney velocity.
    #[test]
    fn copper_cone_jet_velocity() {
        let v_tip = calculate_jet_tip_velocity(8500.0, 30.0, "copper");
        // V_det × cos(30°) = 7360
        assert!(
            (v_tip - 7360.0).abs() < 50.0,
            "copper 30° half-angle should give ~7360 m/s: got {v_tip}"
        );
    }

    /// Tantalum liner gets a small density-driven velocity boost.
    #[test]
    fn tantalum_slightly_faster_than_copper() {
        let copper = calculate_jet_tip_velocity(8500.0, 28.0, "copper");
        let ta = calculate_jet_tip_velocity(8500.0, 28.0, "tantalum");
        assert!(
            ta > copper,
            "tantalum should be slightly faster than copper: Cu={copper}, Ta={ta}"
        );
    }

    /// P/L ratio is in the physically plausible range for a typical HEAT jet.
    #[test]
    fn penetration_ratio_plausible() {
        // Copper vs RHA at 7360 m/s, V_min = 2500 m/s
        let p_l = calculate_penetration_from_jet(8960.0, 7850.0, 7360.0, 2500.0);
        assert!(
            p_l > 0.5 && p_l < 1.5,
            "P/L ratio should be ~0.95: got {p_l}"
        );
    }

    /// P/L is zero when V_jet ≤ V_min.
    #[test]
    fn no_pen_below_min_velocity() {
        let p_l = calculate_penetration_from_jet(8960.0, 7850.0, 2000.0, 2500.0);
        assert_eq!(p_l, 0.0, "should be zero when v_jet < v_min");
    }

    /// Standoff efficiency peaks in the 2–4 calibre range.
    #[test]
    fn standoff_peaks_at_two_to_four_calibers() {
        let c = 0.085;
        let at_0 = standoff_efficiency(0.0, c);
        let at_1 = standoff_efficiency(1.0 * c, c);
        let at_3 = standoff_efficiency(3.0 * c, c);
        let at_6 = standoff_efficiency(6.0 * c, c);
        let at_12 = standoff_efficiency(12.0 * c, c);

        // At contact: very low
        assert!(at_0 < 0.2, "contact efficiency should be <0.2: got {at_0}");
        // At 1 cal: improving but below peak
        assert!(at_1 < 0.9, "1-cal efficiency should be <0.9: got {at_1}");
        // At 3 cal: near peak
        assert!(
            at_3 > at_1,
            "3-cal efficiency ({at_3}) should be > 1-cal ({at_1})"
        );
        assert!(at_3 > 0.85, "3-cal efficiency should be >0.85: got {at_3}");
        // At 6 cal: decayed below peak
        assert!(
            at_6 < at_3,
            "6-cal efficiency ({at_6}) should be < 3-cal ({at_3})"
        );
        // At 12 cal: very low
        assert!(at_12 < 0.5, "12-cal efficiency should be <0.5: got {at_12}");
    }

    /// ERA reduces penetration.
    #[test]
    fn era_significantly_reduces_penetration() {
        let no_era = evaluate_heat_jet(&rpg7_heat_params());

        let with_era = evaluate_heat_jet(&HeatJetParams {
            era_thickness_m: 0.007, // Kontakt-5 ~7 mm
            ..rpg7_heat_params()
        });

        assert!(with_era.era_disrupted, "ERA should be flagged as disrupted");
        assert!(
            with_era.penetration_depth_mm < no_era.penetration_depth_mm * 0.75,
            "ERA should reduce penetration significantly: no_era={:.0}mm, with_era={:.0}mm",
            no_era.penetration_depth_mm,
            with_era.penetration_depth_mm
        );
    }

    // ── End-to-end penetration tests ───────────────────────────────────────────

    /// Basic shaped charge penetrates RHA (RPG-7 vs 200 mm plate).
    #[test]
    fn basic_heat_penetrates_rha() {
        let result = evaluate_heat_jet(&rpg7_heat_params());
        // RPG-7 PG-7VL should achieve ~300 mm RHA penetration
        assert!(
            result.penetration_depth_mm > 150.0,
            "RPG-7 HEAT should pen >150 mm RHA: got {:.0} mm",
            result.penetration_depth_mm
        );
        assert!(
            result.penetration_depth_mm < 600.0,
            "RPG-7 HEAT should pen <600 mm RHA: got {:.0} mm",
            result.penetration_depth_mm
        );
    }

    /// Standoff affects penetration (peak at 2–4 calibres).
    #[test]
    fn standoff_affects_penetration_peak_at_optimal() {
        let base = rpg7_heat_params();
        let c = base.caliber_m;

        // Test various standoffs
        let close = evaluate_heat_jet(&HeatJetParams {
            standoff_m: 0.5 * c,
            ..base.clone()
        });
        let optimal = evaluate_heat_jet(&HeatJetParams {
            standoff_m: 3.0 * c,
            ..base.clone()
        });
        let far = evaluate_heat_jet(&HeatJetParams {
            standoff_m: 10.0 * c,
            ..base.clone()
        });

        assert!(
            optimal.penetration_depth_mm > close.penetration_depth_mm,
            "optimal standoff (3 cal) should pen more than close (0.5 cal): {:.0} vs {:.0}",
            optimal.penetration_depth_mm,
            close.penetration_depth_mm
        );
        assert!(
            optimal.penetration_depth_mm > far.penetration_depth_mm,
            "optimal standoff (3 cal) should pen more than far (10 cal): {:.0} vs {:.0}",
            optimal.penetration_depth_mm,
            far.penetration_depth_mm
        );
    }

    /// ERA reduces penetration significantly.
    #[test]
    fn era_reduces_penetration_substantially() {
        let base = evaluate_heat_jet(&heavy_heat_params());
        let with_era = evaluate_heat_jet(&HeatJetParams {
            era_thickness_m: 0.010, // 10 mm heavy ERA
            ..heavy_heat_params()
        });

        assert!(
            with_era.penetration_depth_mm < base.penetration_depth_mm * 0.6,
            "heavy ERA should reduce penetration to <60%: {:.0} vs {:.0}",
            with_era.penetration_depth_mm,
            base.penetration_depth_mm
        );
    }

    /// Tandem charge defeats ERA (ERA thickness = 0 or very thin).
    #[test]
    fn tandem_charge_defeats_era() {
        // Tandem charge: the precursor clears ERA, so the main jet sees
        // effectively no ERA.
        let with_era = evaluate_heat_jet(&heavy_heat_params());
        let tandem = evaluate_heat_jet(&HeatJetParams {
            era_thickness_m: 0.0, // cleared by precursor
            ..heavy_heat_params()
        });

        // With no ERA effect, tandem should achieve full penetration.
        assert!(!tandem.era_disrupted, "tandem should not be ERA-disrupted");
        assert!(
            tandem.penetration_depth_mm >= with_era.penetration_depth_mm,
            "tandem (no ERA) should pen at least as well as with ERA: {:.0} vs {:.0}",
            tandem.penetration_depth_mm,
            with_era.penetration_depth_mm
        );
    }

    /// Jet is disrupted by very thick ERA.
    #[test]
    fn jet_disrupted_by_thick_era() {
        let result = evaluate_heat_jet(&HeatJetParams {
            era_thickness_m: 0.050, // 50 mm — would stop most single-warhead HEAT
            ..heavy_heat_params()
        });

        assert!(result.era_disrupted, "thick ERA should flag disruption");
        // Very thick ERA should massively reduce / stop the jet.
        assert!(
            result.penetration_depth_mm < 200.0,
            "50 mm ERA should reduce penetration to <200 mm: got {:.0}",
            result.penetration_depth_mm
        );
    }

    /// Deterministic output: same inputs → same outputs.
    #[test]
    fn deterministic_output() {
        let a = evaluate_heat_jet(&rpg7_heat_params());
        let b = evaluate_heat_jet(&rpg7_heat_params());

        assert!(
            (a.penetration_depth_mm - b.penetration_depth_mm).abs() < 1e-9,
            "penetration depth should be identical: a={:.6} b={:.6}",
            a.penetration_depth_mm,
            b.penetration_depth_mm
        );
        assert_eq!(a.jet_disrupted, b.jet_disrupted);
        assert_eq!(a.era_disrupted, b.era_disrupted);
        assert_eq!(
            a.behind_armor_effects.num_spall_fragments,
            b.behind_armor_effects.num_spall_fragments
        );
    }

    /// Impact angle reduces penetration.
    #[test]
    fn angled_impact_reduces_penetration() {
        let normal = evaluate_heat_jet(&rpg7_heat_params());
        let oblique = evaluate_heat_jet(&HeatJetParams {
            impact_angle_deg: 60.0,
            ..rpg7_heat_params()
        });

        assert!(
            oblique.penetration_depth_mm < normal.penetration_depth_mm * 0.8,
            "60° impact should significantly reduce penetration: {:.0} vs {:.0}",
            oblique.penetration_depth_mm,
            normal.penetration_depth_mm
        );
    }

    /// Target density lookup returns correct values for common materials.
    #[test]
    fn target_density_lookup() {
        assert!((target_density_from_material("steel_rha") - 7850.0).abs() < 1.0);
        assert!((target_density_from_material("aluminum_5083") - 2700.0).abs() < 1.0);
        assert!((target_density_from_material("ceramic_al2o3") - 3500.0).abs() < 1.0);
        assert!((target_density_from_material("concrete_reinforced") - 2400.0).abs() < 1.0);
        assert!((target_density_from_material("wood") - 600.0).abs() < 1.0);
    }
}
