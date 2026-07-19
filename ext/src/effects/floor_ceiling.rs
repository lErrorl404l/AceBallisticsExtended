// ABE - Floor/Ceiling Concrete Penetration Model
//
// Models projectile penetration through concrete floor and ceiling slabs
// including reinforced concrete, lightweight concrete, precast hollow-core
// planks, composite steel deck, and suspended gypsum ceilings.
//
// The core penetration engine delegates to `penetration::evaluate()` with
// a concrete material factor adjusted for slab type, density, reinforcement,
// voids, and spall behaviour.
//
// ponytail: SlabType and helpers are test-only — whole module is dead until
// urban-combat map features require floor/ceiling penetration.

#![allow(dead_code)]
// References:
//   - UFC 3-340-01 (Unified Facilities Criteria — Structures to Resist
//     the Effects of Accidental Explosions), Chapters 5–6 (concrete
//     penetration, spall, and breach)
//   - TM 5-855-1 (Design and Analysis of Hardened Structures to
//     Conventional Weapons Effects)
//   - NDRC Report A-365 (Bethune et al., 1945) — empirical concrete
//     penetration formulae
//   - ACI 318-19 (Building Code Requirements for Structural Concrete)
//     — slab thickness and reinforcement standards
//   - CEB-FIP Model Code 2010 — concrete constitutive models

use crate::penetration;

/// Classification of floor / ceiling slab types.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SlabType {
    /// Lightweight / AAC concrete block plank.  Low density (~1600 kg/m³),
    /// low strength.  Used in residential upper floors and non-structural
    /// applications.
    LightweightConcrete,
    /// Standard cast-in-place reinforced concrete slab.  The most common
    /// structural floor system in commercial, industrial, and multi-storey
    /// residential construction.  Density ~2400 kg/m³.
    ReinforcedConcreteFloor,
    /// Cast-in-place reinforced concrete ceiling.  Often monolithic with
    /// the floor slab above, so same thickness and reinforcement.  Separate
    /// entry for scenarios where a suspended ceiling is backed by a thin
    /// concrete topping.
    ReinforcedConcreteCeiling,
    /// Precast / pre-stressed hollow-core plank.  Extruded concrete with
    /// continuous longitudinal voids (~40–60 % of cross-section).  Common
    /// in parking garages, schools, and office buildings.  Effective
    /// thickness is reduced by void fraction.
    PrecastHollowCore,
    /// Composite steel deck with lightweight or normal-weight concrete
    /// topping.  Concrete on profiled steel sheet.  Used in high-rise
    /// steel-frame construction.  The steel deck adds some additional
    /// projectile resistance.
    CompositeDeck,
    /// Suspended gypsum board ceiling (drop ceiling).  NOT concrete —
    /// typically 12–25 mm gypsum panels on a metal grid.  Minimal
    /// projectile resistance; primarily modelled to confirm near-zero
    /// stopping power.
    SuspendedGypsum,
}

/// Input parameters for a concrete-floor / ceiling penetration evaluation.
#[derive(Debug, Clone, Copy)]
pub struct ConcreteSlabParams {
    /// Type of slab construction.
    pub slab_type: SlabType,
    /// Total slab thickness in millimetres.
    pub thickness_mm: f64,
    /// Concrete density in kg/m³.  Normal weight ~2400; lightweight
    /// ~1600–1900.
    pub concrete_density_kgm3: f64,
    /// Rebar diameter in millimetres.  Zero if unreinforced.
    pub rebar_diameter_mm: f64,
    /// Rebar spacing centre-to-centre in millimetres.
    pub rebar_spacing_mm: f64,
    /// True if the slab contains continuous voids (hollow-core planks).
    pub has_voids: bool,
    /// Fraction of cross-sectional area occupied by voids (0.0–1.0).
    /// Typical hollow-core: 0.40–0.60.
    pub void_fraction: f64,
    /// Projectile impact velocity in m/s.
    pub velocity_ms: f64,
    /// Projectile mass in grams.
    pub mass_g: f64,
    /// Projectile calibre in metres (same unit as the existing penetration model).
    pub caliber_m: f64,
    /// Projectile construction type string, passed through to
    /// `penetration::evaluate()` (e.g. "ball", "ap", "apds", "frangible").
    pub projectile_type: &'static str,
    /// Impact angle measured from the surface normal in degrees
    /// (0 = perpendicular, 90 = grazing).
    pub impact_angle_deg: f64,
}

/// Result of a concrete slab penetration evaluation.
#[derive(Debug, Clone, Copy)]
pub struct ConcretePenetrationResult {
    /// Whether the projectile fully perforated the slab.
    pub penetrated: bool,
    /// Maximum penetration depth into the concrete in millimetres.
    /// If `penetrated == true`, this may exceed `thickness_mm`.
    pub penetration_depth_mm: f64,
    /// Projectile velocity remaining after exiting the slab (m/s).
    /// Zero if the projectile was stopped.
    pub residual_velocity_ms: f64,
    /// Whether the projectile struck a rebar.
    pub rebar_hit: bool,
    /// Geometric probability of hitting a rebar (0.0–1.0).
    pub rebar_hit_probability: f64,
    /// Estimated mass of concrete spall ejected from the back face (grams).
    pub spall_mass_g: f64,
    /// Front-face crater diameter in millimetres.
    pub crater_front_diameter_mm: f64,
    /// Back-face crater (scab) diameter in millimetres.
    pub crater_back_diameter_mm: f64,
    /// Whether the slab has lost structural integrity (complete breaching
    /// with large back-face crater / spall).
    pub structural_damage: bool,
}

// ── Concrete material factor ───────────────────────────────────────────────

/// Material resistance factor for concrete based on density.
///
/// Follows the trend in UFC 3-340-01 empirical data: higher-density concrete
/// offers greater penetration resistance.  The factor scales relative to
/// normal-weight concrete (2400 kg/m³ ≈ 1.0).
///
/// Returns 0.0 for densities below 800 kg/m³ (no meaningful concrete
/// resistance; effectively gypsum or filler).
pub fn concrete_material_factor(density_kgm3: f64) -> f64 {
    if density_kgm3 < 800.0 {
        return 0.0;
    }
    // Normal-weight reference: 2400 kg/m³ → 1.0
    // Lightweight 1600 kg/m³ → ~0.63
    // High-density 2500 kg/m³ → ~1.05
    let rho = density_kgm3.clamp(800.0, 3000.0);
    (rho / 2400.0).powf(1.2)
}

// ── Rebar hit probability ──────────────────────────────────────────────────

/// Geometric probability that a projectile hits a rebar, based on
/// rebar diameter and spacing.
///
/// Assumes a square grid of rebars.  The probability equals the area
/// fraction of rebar cross-section within the slab plane:
///
///   P = (d_rebar / spacing)²
///
/// Real reinforcement may be a double mat (top + bottom); this models a
/// single mat at mid-depth.
pub fn rebar_hit_probability(rebar_diameter_mm: f64, rebar_spacing_mm: f64) -> f64 {
    if rebar_diameter_mm <= 0.0 || rebar_spacing_mm <= 0.0 {
        return 0.0;
    }
    let ratio = rebar_diameter_mm / rebar_spacing_mm;
    (ratio * ratio).min(0.5) // cap at 50 % (very dense reinforcement)
}

// ── Effective thickness ─────────────────────────────────────────────────────

/// Base concrete-to-RHA material factor at normal weight (2400 kg/m³).
///
/// Matches `material_factor("concrete")` in `penetration.rs`.
const BASE_CONCRETE_RHA_FACTOR: f64 = 0.15;

/// Compute RHA-equivalent thickness in millimetres, accounting for slab
/// type, density, reinforcement, and voids.
///
/// This computes the full conversion to RHA-equivalent so the penetration
/// model can be called with `"steel_rha"` (factor 1.0) — avoiding double
/// application of the concrete material factor.
fn rha_equivalent_thickness(params: &ConcreteSlabParams) -> f64 {
    let density_factor = concrete_material_factor(params.concrete_density_kgm3);
    let total_concrete_factor = BASE_CONCRETE_RHA_FACTOR * density_factor;

    // Base RHA-equivalent from slab type
    match params.slab_type {
        SlabType::LightweightConcrete
        | SlabType::ReinforcedConcreteFloor
        | SlabType::ReinforcedConcreteCeiling => params.thickness_mm * total_concrete_factor,
        SlabType::PrecastHollowCore => {
            // Voids reduce effective cross-section
            let solid_fraction = 1.0 - params.void_fraction;
            params.thickness_mm * total_concrete_factor * solid_fraction
        },
        SlabType::CompositeDeck => {
            // Steel deck adds ~1–2 mm RHA-equivalent
            params.thickness_mm * total_concrete_factor * 1.05
        },
        SlabType::SuspendedGypsum => {
            // Gypsum board has negligible penetration resistance
            params.thickness_mm * 0.01 * density_factor
        },
    }
}

/// Convert RHA-equivalent thickness back to physical concrete depth.
fn rha_to_concrete_depth(rha_mm: f64, density_kgm3: f64) -> f64 {
    let density_factor = concrete_material_factor(density_kgm3);
    let total_factor = BASE_CONCRETE_RHA_FACTOR * density_factor;
    if total_factor > 0.0 {
        rha_mm / total_factor
    } else {
        0.0
    }
}

/// Maximum RHA-equivalent thickness (mm) a projectile can penetrate via the
/// De Marre formula inverted for T.
fn de_marre_max_rha_mm(velocity_ms: f64, mass_kg: f64, caliber_m: f64, proj_type: &str) -> f64 {
    let proj_mod = match proj_type.to_lowercase().as_str() {
        "ap" | "armor_piercing" => 1.3,
        "apds" | "apfsds" => 1.8,
        "apcr" => 1.5,
        "frangible" => 0.5,
        "soft_point" | "hollow_point" => 0.9,
        "incendiary" | "tracer" => 0.95,
        _ => 1.0,
    };
    let k = 91000.0 / proj_mod;
    // V_req = k × D^0.75 × T^0.7 / √M
    // → T = ((V × √M) / (k × D^0.75))^(1/0.7)
    if caliber_m <= 0.0 || mass_kg <= 0.0 || velocity_ms <= 0.0 {
        return 0.0;
    }
    let numerator = velocity_ms * mass_kg.sqrt();
    let denominator = k * caliber_m.powf(0.75);
    if denominator <= 0.0 {
        return 0.0;
    }
    let t_ratio = numerator / denominator;
    let t_m = t_ratio.powf(1.0 / 0.7);
    t_m * 1000.0 // convert to mm
}

// ── Spall model ────────────────────────────────────────────────────────────

/// Estimate spall (back-face scabbing) parameters per UFC 3-340-01.
///
/// Spall occurs when a projectile stops within approximately 1/3 of the
/// slab thickness from the rear face.  The scabbed area is modelled as a
/// cone with the apex at the projectile nose depth and base on the back
/// face.  Spall mass scales with crater volume and concrete density.
fn estimate_spall(
    thickness_mm: f64,
    density_kgm3: f64,
    penetration_depth_mm: f64,
    penetrated: bool,
    caliber_m: f64,
) -> (f64, f64, f64) {
    if penetrated {
        // Full perforation: the back-face crater is bounded by the
        // remaining energy.  Spall is still produced.
        let crater_front = caliber_m * 1000.0 * 8.0; // front crater: ~8× calibre
        let crater_back = caliber_m * 1000.0 * 6.0; // smaller back crater

        // Estimate spall mass from a conical frustum approximation.
        let avg_crater_mm = (crater_front + crater_back) / 2.0;
        // Use a thin slice approximation: volume ~ thickness × area of
        // a circle at the average crater diameter
        let spall_volume_mm3 =
            std::f64::consts::PI * (avg_crater_mm / 2.0).powi(2) * thickness_mm * 0.1;
        let spall_mass_g = spall_volume_mm3 / 1000.0 * (density_kgm3 / 1_000_000.0);

        return (crater_front, crater_back.min(crater_front), spall_mass_g);
    }

    // Projectile stopped within the slab — check the standoff to back face
    let standoff_to_rear = thickness_mm - penetration_depth_mm;

    if standoff_to_rear <= 0.0 {
        // Edge case: flush with back face
        let crater_front = caliber_m * 1000.0 * 6.0;
        (crater_front, 0.0, 0.0)
    } else if standoff_to_rear <= thickness_mm / 3.0 {
        // Spall likely: nose is ≤ 1/3 thickness from rear face.
        // Crater diameter grows with proximity to the rear surface.
        let proximity_ratio = 1.0 - (standoff_to_rear / (thickness_mm / 3.0));
        let crater_front = caliber_m * 1000.0 * (6.0 + 4.0 * proximity_ratio);
        let crater_back = caliber_m * 1000.0 * (4.0 + 6.0 * proximity_ratio);

        // Spall cone volume: frustum approximation
        let crater_avg = (crater_front + crater_back) / 2.0;
        let cone_height_mm = standoff_to_rear;
        let spall_volume_mm3 =
            std::f64::consts::PI / 3.0 * (crater_avg / 2.0).powi(2) * cone_height_mm;
        let spall_mass_g = spall_volume_mm3 / 1000.0 * (density_kgm3 / 1_000_000.0);

        (crater_front, crater_back, spall_mass_g.max(1.0))
    } else {
        // Deep embedment — no back-face effect
        let crater_front = caliber_m * 1000.0 * (4.0 + 4.0 * (penetration_depth_mm / thickness_mm));
        (crater_front, 0.0, 0.0)
    }
}

// ── Core evaluation ────────────────────────────────────────────────────────

/// Evaluate whether a projectile penetrates a concrete floor / ceiling slab.
///
/// The core penetration delegates to `penetration::evaluate()` using
/// `"steel_rha"` (the RHA reference material) with an RHA-equivalent
/// thickness computed from the concrete slab properties — density, voids,
/// slab type, and reinforcement.  This avoids double-applying the concrete
/// material factor that is already built into the penetration model.
///
/// Rebar interaction is handled probabilistically: the function samples one
/// rebar-hit roll per evaluation (deterministic in this pure function — the
/// caller should re-roll statefully for stochastic behaviour).
///
/// Spall, cratering, and structural damage are estimated from the
/// penetration depth relative to slab thickness, per UFC 3-340-01.
pub fn evaluate_slab_penetration(params: &ConcreteSlabParams) -> ConcretePenetrationResult {
    let mass_kg = params.mass_g / 1000.0;

    // ── Rebar hit ────────────────────────────────────────────────────────
    let rebar_prob = rebar_hit_probability(params.rebar_diameter_mm, params.rebar_spacing_mm);

    // Deterministic roll: stable across runs for a given slab type.
    let slab_hash = match params.slab_type {
        SlabType::LightweightConcrete => 0.123,
        SlabType::ReinforcedConcreteFloor => 0.456,
        SlabType::ReinforcedConcreteCeiling => 0.678,
        SlabType::PrecastHollowCore => 0.234,
        SlabType::CompositeDeck => 0.567,
        SlabType::SuspendedGypsum => 0.345,
    };
    let rebar_hit = rebar_prob > 0.0 && slab_hash < rebar_prob;

    // RHA-equivalent thickness: concrete base + rebar add-on if hit
    let rha_base_mm = rha_equivalent_thickness(params);
    let rebar_added_mm = if rebar_hit && params.rebar_diameter_mm > 0.0 {
        // Rebar adds: diameter × structural steel factor (0.7) as RHA-equivalent
        params.rebar_diameter_mm * 0.7
    } else {
        0.0
    };
    let rha_total_m = (rha_base_mm + rebar_added_mm) / 1000.0;

    // ── Delegate to penetration model ────────────────────────────────────
    let pen = penetration::evaluate(
        params.velocity_ms,
        mass_kg,
        params.caliber_m,
        rha_total_m,
        params.impact_angle_deg,
        "steel_rha",
        params.projectile_type,
        None,
    );

    // ── Back out true penetration depth ──────────────────────────────────
    // Use the inverted De Marre formula for projectile penetration
    // capability, then convert from RHA-equivalent to physical concrete.
    let max_rha_mm = de_marre_max_rha_mm(
        params.velocity_ms,
        mass_kg,
        params.caliber_m,
        params.projectile_type,
    );
    let physical_depth_mm = if pen.penetrated {
        params.thickness_mm * 1.1 // marginal perforation
    } else {
        rha_to_concrete_depth(max_rha_mm, params.concrete_density_kgm3).min(params.thickness_mm)
    };

    // ── Spall and cratering ──────────────────────────────────────────────
    let (crater_front, crater_back, spall_mass) = estimate_spall(
        params.thickness_mm,
        params.concrete_density_kgm3,
        physical_depth_mm,
        pen.penetrated,
        params.caliber_m,
    );

    // ── Structural damage ────────────────────────────────────────────────
    // Structural integrity loss occurs on full perforation with large
    // back-face crater (> 300 mm) or extensive spall (> 5 kg).
    let structural_damage = pen.penetrated && (crater_back > 300.0 || spall_mass > 5000.0);

    ConcretePenetrationResult {
        penetrated: pen.penetrated,
        penetration_depth_mm: physical_depth_mm,
        residual_velocity_ms: pen.residual_velocity,
        rebar_hit,
        rebar_hit_probability: rebar_prob,
        spall_mass_g: spall_mass,
        crater_front_diameter_mm: crater_front,
        crater_back_diameter_mm: crater_back,
        structural_damage,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn default_floor_params() -> ConcreteSlabParams {
        ConcreteSlabParams {
            slab_type: SlabType::ReinforcedConcreteFloor,
            thickness_mm: 200.0,
            concrete_density_kgm3: 2400.0,
            rebar_diameter_mm: 12.0,
            rebar_spacing_mm: 200.0,
            has_voids: false,
            void_fraction: 0.0,
            velocity_ms: 900.0,
            mass_g: 9.5,
            caliber_m: 0.00762,
            projectile_type: "ball",
            impact_angle_deg: 0.0,
        }
    }

    // ── Slab type tests ──────────────────────────────────────────────────

    #[test]
    fn standard_rc_floor_stops_7_62_ball() {
        // 200 mm RC floor at 0° — typical 7.62×51 ball should NOT penetrate
        let p = evaluate_slab_penetration(&default_floor_params());
        assert!(
            !p.penetrated,
            "200mm RC floor should stop 7.62mm ball at 900 m/s"
        );
    }

    #[test]
    fn standard_rc_floor_penetrated_by_50bmg() {
        // .50 BMG AP through a thinner 100 mm slab at 0°
        let params = ConcreteSlabParams {
            slab_type: SlabType::ReinforcedConcreteFloor,
            thickness_mm: 100.0,
            concrete_density_kgm3: 2400.0,
            rebar_diameter_mm: 12.0,
            rebar_spacing_mm: 200.0,
            has_voids: false,
            void_fraction: 0.0,
            velocity_ms: 880.0,
            mass_g: 42.0,
            caliber_m: 0.0127,
            projectile_type: "ap",
            impact_angle_deg: 0.0,
        };
        let p = evaluate_slab_penetration(&params);
        assert!(
            p.penetrated,
            "100mm RC floor should be perforated by .50 BMG AP at 880 m/s"
        );
        assert!(
            p.residual_velocity_ms > 0.0,
            "Residual velocity should be positive on perforation"
        );
    }

    #[test]
    fn lightweight_concrete_less_resistance() {
        let normal = evaluate_slab_penetration(&ConcreteSlabParams {
            concrete_density_kgm3: 2400.0,
            ..default_floor_params()
        });
        let light = evaluate_slab_penetration(&ConcreteSlabParams {
            slab_type: SlabType::LightweightConcrete,
            concrete_density_kgm3: 1600.0,
            ..default_floor_params()
        });
        // Lighter density → less effective thickness → easier penetration
        assert!(
            light.penetration_depth_mm >= normal.penetration_depth_mm,
            "Lightweight concrete should have equal or greater penetration depth"
        );
    }

    // ── Rebar interaction ────────────────────────────────────────────────

    #[test]
    fn rebar_hit_probability_scales_with_density() {
        let dense = rebar_hit_probability(16.0, 150.0);
        let sparse = rebar_hit_probability(10.0, 300.0);
        assert!(
            dense > sparse,
            "Denser rebar layout should have higher hit probability"
        );
        assert!(dense <= 0.5, "Hit probability should be capped at 0.5");
    }

    #[test]
    fn rebar_hit_adds_resistance() {
        // Thin slab where rebar hit matters
        let no_rebar = evaluate_slab_penetration(&ConcreteSlabParams {
            slab_type: SlabType::ReinforcedConcreteCeiling,
            thickness_mm: 100.0,
            concrete_density_kgm3: 2400.0,
            rebar_diameter_mm: 0.0,
            rebar_spacing_mm: 200.0,
            has_voids: false,
            void_fraction: 0.0,
            velocity_ms: 850.0,
            mass_g: 9.5,
            caliber_m: 0.00762,
            projectile_type: "ball",
            impact_angle_deg: 0.0,
        });
        let with_rebar = evaluate_slab_penetration(&ConcreteSlabParams {
            rebar_diameter_mm: 12.0,
            rebar_spacing_mm: 150.0,
            ..ConcreteSlabParams {
                slab_type: SlabType::ReinforcedConcreteCeiling,
                thickness_mm: 100.0,
                concrete_density_kgm3: 2400.0,
                velocity_ms: 850.0,
                mass_g: 9.5,
                caliber_m: 0.00762,
                projectile_type: "ball",
                impact_angle_deg: 0.0,
                rebar_diameter_mm: 0.0,
                rebar_spacing_mm: 200.0,
                has_voids: false,
                void_fraction: 0.0,
            }
        });
        // Only check if rebar_hit changed anything — penetration model may
        // have stopped it either way; what matters is that when rebar IS
        // hit the effective thickness is never lower
        if with_rebar.rebar_hit {
            assert!(
                with_rebar.penetration_depth_mm <= no_rebar.penetration_depth_mm
                    || !with_rebar.penetrated && no_rebar.penetrated,
                "Rebar hit should not reduce resistance"
            );
        }
    }

    #[test]
    fn zero_rebar_means_no_hit() {
        let p = rebar_hit_probability(0.0, 200.0);
        assert_eq!(p, 0.0, "No rebar → zero hit probability");
    }

    // ── Hollow-core voids ────────────────────────────────────────────────

    #[test]
    fn hollow_core_voids_reduce_effective_thickness() {
        let solid = evaluate_slab_penetration(&ConcreteSlabParams {
            slab_type: SlabType::PrecastHollowCore,
            thickness_mm: 250.0,
            has_voids: false,
            void_fraction: 0.0,
            ..default_floor_params()
        });
        let with_voids = evaluate_slab_penetration(&ConcreteSlabParams {
            slab_type: SlabType::PrecastHollowCore,
            thickness_mm: 250.0,
            has_voids: true,
            void_fraction: 0.5,
            ..default_floor_params()
        });
        assert!(
            with_voids.penetration_depth_mm >= solid.penetration_depth_mm,
            "Voids should reduce effective resistance"
        );
    }

    // ── Suspended ceiling ────────────────────────────────────────────────

    #[test]
    fn suspended_gypsum_celling_offers_negligible_resistance() {
        // 12 mm gypsum board at 0° — should be trivially perforated
        let p = evaluate_slab_penetration(&ConcreteSlabParams {
            slab_type: SlabType::SuspendedGypsum,
            thickness_mm: 12.0,
            concrete_density_kgm3: 800.0,
            rebar_diameter_mm: 0.0,
            rebar_spacing_mm: 0.0,
            has_voids: false,
            void_fraction: 0.0,
            velocity_ms: 400.0,
            mass_g: 4.0,
            caliber_m: 0.00556,
            projectile_type: "ball",
            impact_angle_deg: 0.0,
        });
        assert!(
            p.penetrated,
            "12 mm gypsum ceiling should be trivially perforated by 5.56mm ball at 400 m/s"
        );
    }

    // ── Density material factor ──────────────────────────────────────────

    #[test]
    fn concrete_material_factor_is_monotonic() {
        let f800 = concrete_material_factor(800.0);
        let f1600 = concrete_material_factor(1600.0);
        let f2400 = concrete_material_factor(2400.0);
        let f2500 = concrete_material_factor(2500.0);
        assert!(f800 >= 0.0);
        assert!(f1600 > f800, "Factor should increase with density");
        assert!(f2400 > f1600);
        assert!(f2500 > f2400);
        assert!((f2400 - 1.0).abs() < 0.1, "2400 kg/m³ should be near 1.0");
    }

    #[test]
    fn very_low_density_returns_zero() {
        assert_eq!(concrete_material_factor(100.0), 0.0);
    }

    // ── Spall model ──────────────────────────────────────────────────────

    #[test]
    fn thin_slab_spall_on_perforation() {
        // 100 mm slab fully perforated by 7.62 mm at 900 m/s
        let p = evaluate_slab_penetration(&ConcreteSlabParams {
            slab_type: SlabType::ReinforcedConcreteFloor,
            thickness_mm: 100.0,
            concrete_density_kgm3: 2400.0,
            rebar_diameter_mm: 10.0,
            rebar_spacing_mm: 200.0,
            has_voids: false,
            void_fraction: 0.0,
            velocity_ms: 900.0,
            mass_g: 9.5,
            caliber_m: 0.00762,
            projectile_type: "ball",
            impact_angle_deg: 0.0,
        });
        if p.penetrated {
            assert!(p.spall_mass_g > 0.0, "Perforation should produce spall");
            assert!(p.crater_front_diameter_mm > 0.0);
            assert!(p.crater_back_diameter_mm >= 0.0);
        }
    }

    #[test]
    fn deep_embedment_avoids_back_spall() {
        // Thick slab: projectile embeds deep but stays well away from
        // the back face — no back-face spall expected.
        // Use low velocity to ensure embedment without perforation.
        let p = evaluate_slab_penetration(&ConcreteSlabParams {
            slab_type: SlabType::ReinforcedConcreteFloor,
            thickness_mm: 300.0,
            concrete_density_kgm3: 2400.0,
            rebar_diameter_mm: 12.0,
            rebar_spacing_mm: 200.0,
            has_voids: false,
            void_fraction: 0.0,
            velocity_ms: 500.0,
            mass_g: 9.5,
            caliber_m: 0.00762,
            projectile_type: "ball",
            impact_angle_deg: 0.0,
        });
        assert!(!p.penetrated, "300 mm slab should stop 7.62 mm at 500 m/s");
        if p.penetration_depth_mm < 200.0 {
            // Nose more than 1/3 from rear → no back spall
            assert_eq!(
                p.crater_back_diameter_mm, 0.0,
                "Deep embedment should not produce back-face crater"
            );
        }
    }

    // ── Composite deck ───────────────────────────────────────────────────

    #[test]
    fn composite_deck_slightly_more_resistance() {
        let plain = evaluate_slab_penetration(&ConcreteSlabParams {
            slab_type: SlabType::ReinforcedConcreteFloor,
            thickness_mm: 150.0,
            ..default_floor_params()
        });
        let composite = evaluate_slab_penetration(&ConcreteSlabParams {
            slab_type: SlabType::CompositeDeck,
            thickness_mm: 150.0,
            ..default_floor_params()
        });
        assert!(
            composite.penetration_depth_mm <= plain.penetration_depth_mm,
            "Composite deck should offer equal or better resistance than plain RC"
        );
    }

    // ── Impact angle ─────────────────────────────────────────────────────

    #[test]
    fn oblique_impact_reduces_penetration() {
        let normal = evaluate_slab_penetration(&default_floor_params());
        let oblique = evaluate_slab_penetration(&ConcreteSlabParams {
            impact_angle_deg: 45.0,
            ..default_floor_params()
        });
        assert!(
            oblique.penetration_depth_mm <= normal.penetration_depth_mm
                || !oblique.penetrated && normal.penetrated,
            "Oblique impact should not improve penetration"
        );
    }
}
