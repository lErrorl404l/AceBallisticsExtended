// ABE - Frangible Ammunition Model
//
// Models frangible projectile behaviour: rounds (e.g., M882 frangible, THV)
// designed to disintegrate on hard-surface impact, reducing ricochet and
// overpenetration.  They penetrate soft targets normally but shatter into
// fine dust when striking metal, concrete, ceramic, or bone above a
// velocity threshold.
//
// References:
//   - NIJ Standard 0108.01 (Ballistic Resistant Protective Materials) —
//     frangible ammunition classification and test methodology
//   - Nennstiel's "Fragmentation of Bullets" (1986)
//   - MacPherson, D., "Bullet Penetration" (1994) — frangible projectile
//     terminal effects in ballistic gelatin
//   - US DoD Frangible Ammunition Safety & Performance Specification
//   - FBI HPR terminal ballistics data on frangible vs. conventional rounds
//   - Capstick's terminal ballistics data (various calibers)

/// Minimum impact velocity (m/s) required to trigger any shatter event.
/// Below this, even hard surfaces behave as if struck by a soft lead slug.
const MIN_SHATTER_VELOCITY_MS: f64 = 50.0;

/// Fraction of pre-impact kinetic energy transferred into the surface
/// when frangible shatter occurs (≥ 95 %).
const ENERGY_TRANSFER_FRACTION: f64 = 0.95;

/// Base penetration depth into a hard surface before full breakup (mm).
const HARD_PENETRATION_BASE_MM: f64 = 3.0;

/// Fraction of projectile mass that remains as residual fragments after
/// a full shatter event.  The rest is pulverised and adheres to / is
/// embedded in the surface.
const RESIDUAL_MASS_FRACTION: f64 = 0.05;

/// Maximum speed any dust fragment can carry (m/s).  The highly inelastic
/// shatter event caps fragment velocity well below impact speed.
const MAX_FRAGMENT_VELOCITY_MS: f64 = 200.0;

// ── Types ──────────────────────────────────────────────────────────────────────

/// Type of frangible projectile construction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FrangibleType {
    /// M882 type, lead-based frangible (pressed powdered lead + binder).
    /// Fragmentation threshold ~300 m/s.
    /// Typical use: .22 / 5.56 mm training ammunition.
    LeadFrangible,
    /// THV type, sintered copper frangible (compressed copper powder).
    /// Fragmentation threshold ~200 m/s.
    /// Lower density than lead; wider fragment spread.
    SinteredCopper,
    /// High-performance match frangible.  Precision tolerances with
    /// controlled fragmentation at ~250 m/s.
    CompositeMatch,
}

/// Input parameters for a frangible projectile impact evaluation.
#[derive(Debug, Clone, Copy)]
pub struct FrangibleAmmoParams {
    /// Frangible projectile construction type.
    pub frangible_type: FrangibleType,
    /// Impact velocity in metres per second.
    pub impact_velocity_ms: f64,
    /// Projectile mass in grams.
    pub mass_g: f64,
    /// Projectile calibre in millimetres.
    pub caliber_mm: f64,
}

/// Result of a frangible projectile impact evaluation.
///
/// Describes whether the projectile shattered, how much energy was
/// transferred to the surface, residual fragment characteristics, and
/// the penetration distance into the target before full breakup.
#[derive(Debug, Clone, Copy)]
pub struct FrangibleImpactResult {
    /// Whether the projectile shattered (fragmented into fine dust).
    pub shatters: bool,
    /// Kinetic energy transferred into the surface (J).  When
    /// `shatters == true` this is ≥ 95 % of pre-impact KE.
    pub energy_transferred_j: f64,
    /// Total mass of the largest residual fragment after breakup (g).
    /// Zero when fully disintegrated.
    pub residual_mass_g: f64,
    /// Velocity of the residual fragment / dust cloud after breakup (m/s).
    pub residual_velocity_ms: f64,
    /// Maximum penetration into the target before full fragmentation
    /// stops the projectile (mm).  Zero if the round fully pulverises
    /// on contact.
    pub max_penetration_mm: f64,
    /// Number of fragments generated on shatter.
    pub fragment_count: i32,
    /// Average mass of individual fragments (g).  For a shatter event
    /// this is typically < 0.1 g.
    pub avg_fragment_mass_g: f64,
}

// ── Thresholds ─────────────────────────────────────────────────────────────────

/// Fragmentation threshold velocity (m/s) for a given frangible type.
///
/// Below this velocity the projectile behaves as a normal soft lead slug
/// even against hard surfaces.  Above it the projectile shatters on hard
/// impact.
fn fragmentation_threshold(ft: FrangibleType) -> f64 {
    match ft {
        FrangibleType::LeadFrangible => 300.0,
        FrangibleType::SinteredCopper => 200.0,
        FrangibleType::CompositeMatch => 250.0,
    }
}

// ── Surface classification ─────────────────────────────────────────────────────

/// Categorise a surface material for frangible shatter logic.
///
/// Returns one of:
/// * `"hard"` — metal, concrete, ceramic, armour → full shatter when
///   above velocity threshold.
/// * `"bone"` — bone (cortical / trabecular) → shatter with partial
///   energy deposit.
/// * `"soft"` — tissue, gelatin, water, wood, soil, polymers → no
///   shatter; normal penetration behaviour.
pub fn surface_category(surface: &str) -> &str {
    let s = surface.to_lowercase();

    // ── Hard materials (trigger full shatter) ────────────────────────
    // These are materials with compressive strength >> soft tissue:
    // metals, ceramics, concrete, brick, glass, armour composites.
    let hard_keywords = [
        "steel",
        "rha",
        "hha",
        "armor",
        "armour",
        "ceramic",
        "aluminum",
        "aluminium",
        "titanium",
        "concrete",
        "brick",
        "glass",
        "iron",
        "uranium",
        "era_",
        "chobham",
        "burlington",
        "dorchester",
        "stef_",
        "kvarts",
        "gost_",
        "cage_",
        "slat_",
    ];
    for kw in &hard_keywords {
        if s.contains(kw) {
            return "hard";
        }
    }

    // ── Bone (partial shatter) ───────────────────────────────────────
    let bone_keywords = ["bone", "rib", "skull", "cortical", "trabecular"];
    for kw in &bone_keywords {
        if s.contains(kw) {
            return "bone";
        }
    }

    // ── Soft (everything else — tissue, water, wood, soil, polymers) ─
    "soft"
}

// ── Core evaluation ────────────────────────────────────────────────────────────

/// Evaluate frangible projectile behaviour on impact.
///
/// # Arguments
/// * `params` — Frangible projectile and impact parameters (type,
///   velocity, mass, calibre).
/// * `surface` — Surface material identifier string
///   (e.g. `"steel_rha"`, `"concrete"`, `"bone"`, `"soft_tissue"`).
///   The surface category is determined by [`surface_category`].
///
/// # Returns
/// [`FrangibleImpactResult`] with shatter status, energy transfer,
/// residual mass / velocity, and fragment statistics.
///
/// # Behaviour summary
/// | Surface  | v ≥ threshold | v < threshold |
/// |----------|--------------|--------------|
/// | Hard     | Shatters, ≥ 95 % KE dumped | Normal lead-slug behaviour |
/// | Bone     | Shatters, ~60 % KE dumped | Partial fragmentation |
/// | Soft     | Normal penetration | Normal penetration |
pub fn evaluate_frangible_impact(
    params: &FrangibleAmmoParams,
    surface: &str,
) -> FrangibleImpactResult {
    let ke = 0.5 * (params.mass_g / 1000.0) * params.impact_velocity_ms.powi(2);
    let threshold = fragmentation_threshold(params.frangible_type);
    let category = surface_category(surface);

    // Zero-mass or zero-velocity projectiles cannot shatter
    if params.mass_g <= 0.0 || params.impact_velocity_ms <= 0.0 {
        return FrangibleImpactResult {
            shatters: false,
            energy_transferred_j: 0.0,
            residual_mass_g: 0.0,
            residual_velocity_ms: 0.0,
            max_penetration_mm: 0.0,
            fragment_count: 0,
            avg_fragment_mass_g: 0.0,
        };
    }

    match category {
        "hard" => {
            if params.impact_velocity_ms >= threshold
                && params.impact_velocity_ms >= MIN_SHATTER_VELOCITY_MS
            {
                // ── Full shatter ──────────────────────────────────────
                // Projectile disintegrates into fine dust.  ≥ 95 % KE
                // goes into the surface over < 5 cm.
                let vel_ratio = (params.impact_velocity_ms / threshold - 1.0).min(3.0);
                let max_pen = HARD_PENETRATION_BASE_MM * (1.0 + vel_ratio * 0.5);

                let energy_transferred = ke * ENERGY_TRANSFER_FRACTION;
                let residual_ke = ke * (1.0 - ENERGY_TRANSFER_FRACTION);

                // Total fragment mass is a small fraction of the projectile
                let total_frag_mass_g = params.mass_g * RESIDUAL_MASS_FRACTION;

                // Fragment count grows with velocity above threshold
                let fragment_count = (10.0 + 25.0 * vel_ratio).round().max(1.0) as i32;

                let avg_fragment_mass_g = total_frag_mass_g / fragment_count as f64;

                // Residual velocity is low — the shatter is highly inelastic
                let residual_vel = residual_fragment_velocity(residual_ke, total_frag_mass_g);

                FrangibleImpactResult {
                    shatters: true,
                    energy_transferred_j: energy_transferred,
                    residual_mass_g: total_frag_mass_g,
                    residual_velocity_ms: residual_vel,
                    max_penetration_mm: max_pen,
                    fragment_count,
                    avg_fragment_mass_g: avg_fragment_mass_g,
                }
            } else {
                // ── Below threshold: soft-lead behaviour ──────────────
                // No shatter.  The round behaves like a normal lead slug,
                // transferring only a modest fraction of KE to the surface
                // and retaining most of its mass.
                below_threshold_result(ke, params)
            }
        }

        "bone" => {
            if params.impact_velocity_ms >= threshold
                && params.impact_velocity_ms >= MIN_SHATTER_VELOCITY_MS
            {
                // ── Bone shatter (partial energy deposit) ─────────────
                // Bone is hard but less dense than metal/concrete; the
                // projectile deposits ~60 % of its KE and fragments into
                // larger pieces.
                let bone_energy_fraction = 0.60;
                let energy_transferred = ke * bone_energy_fraction;
                let residual_ke = ke * (1.0 - bone_energy_fraction);
                let total_frag_mass_g = params.mass_g * 0.10; // 10 % residual
                let vel_ratio = (params.impact_velocity_ms / threshold - 1.0).min(3.0);
                let max_pen = HARD_PENETRATION_BASE_MM * 2.5 * (1.0 + vel_ratio * 0.3);
                let fragment_count = (5.0 + 15.0 * vel_ratio).round().max(1.0) as i32;
                let avg_fragment_mass_g = total_frag_mass_g / fragment_count as f64;
                let residual_vel = residual_fragment_velocity(residual_ke, total_frag_mass_g);

                FrangibleImpactResult {
                    shatters: true,
                    energy_transferred_j: energy_transferred,
                    residual_mass_g: total_frag_mass_g,
                    residual_velocity_ms: residual_vel,
                    max_penetration_mm: max_pen,
                    fragment_count,
                    avg_fragment_mass_g: avg_fragment_mass_g,
                }
            } else {
                // ── Below threshold against bone: partial penetration ─
                below_threshold_result(ke, params)
            }
        }

        _ => {
            // ── Soft target: no shatter, normal behaviour ─────────────
            below_threshold_result(ke, params)
        }
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────────

/// Compute a plausible residual fragment velocity given residual kinetic
/// energy and fragment mass.
///
/// The shatter event is highly inelastic, so the actual velocity is
/// capped to a physically reasonable maximum regardless of what the
/// kinetic-energy formula alone would suggest (since the fragments are
/// decelerated by the surface and the explosion-like expansion of the
/// pulverised projectile).
fn residual_fragment_velocity(residual_ke: f64, fragment_mass_g: f64) -> f64 {
    if fragment_mass_g <= 0.0 || residual_ke <= 0.0 {
        return 0.0;
    }
    let mass_kg = fragment_mass_g / 1000.0;
    let vel_from_ke = (2.0 * residual_ke / mass_kg).sqrt();
    vel_from_ke.min(MAX_FRAGMENT_VELOCITY_MS)
}

/// Produce a non-shatter result for impacts below the fragmentation
/// threshold or against soft surfaces.
///
/// The round retains most of its mass and energy, behaving like a normal
/// soft lead projectile.
fn below_threshold_result(ke: f64, params: &FrangibleAmmoParams) -> FrangibleImpactResult {
    // Soft lead slug: ~10–20 % energy transfer, most mass retained
    let energy_transferred = ke * 0.15;
    let residual_mass = params.mass_g * 0.95;
    let residual_ke = ke - energy_transferred;
    let residual_vel = (2.0 * residual_ke / (residual_mass / 1000.0)).sqrt();

    FrangibleImpactResult {
        shatters: false,
        energy_transferred_j: energy_transferred,
        residual_mass_g: residual_mass,
        residual_velocity_ms: residual_vel,
        // Lead slug penetrates several calibres into the surface
        max_penetration_mm: params.caliber_mm * 8.0,
        fragment_count: 1,
        avg_fragment_mass_g: params.mass_g,
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Hard-surface shatter tests ─────────────────────────────────────────

    #[test]
    fn lead_frangible_shatters_on_steel_above_threshold() {
        let params = FrangibleAmmoParams {
            frangible_type: FrangibleType::LeadFrangible,
            impact_velocity_ms: 400.0, // > 300 threshold
            mass_g: 4.0,
            caliber_mm: 5.56,
        };
        let result = evaluate_frangible_impact(&params, "steel_rha");
        assert!(
            result.shatters,
            "Lead frangible should shatter on steel above 300 m/s"
        );
    }

    #[test]
    fn sintered_copper_shatters_on_concrete_above_threshold() {
        let params = FrangibleAmmoParams {
            frangible_type: FrangibleType::SinteredCopper,
            impact_velocity_ms: 350.0, // > 200 threshold
            mass_g: 3.0,
            caliber_mm: 5.56,
        };
        let result = evaluate_frangible_impact(&params, "concrete");
        assert!(
            result.shatters,
            "Sintered copper should shatter on concrete above 200 m/s"
        );
    }

    #[test]
    fn composite_match_shatters_on_ceramic() {
        let params = FrangibleAmmoParams {
            frangible_type: FrangibleType::CompositeMatch,
            impact_velocity_ms: 500.0, // > 250 threshold
            mass_g: 4.5,
            caliber_mm: 7.62,
        };
        let result = evaluate_frangible_impact(&params, "ceramic_b4c");
        assert!(
            result.shatters,
            "Composite match should shatter on ceramic above 250 m/s"
        );
    }

    // ── Below-threshold — no shatter ──────────────────────────────────────

    #[test]
    fn lead_frangible_below_threshold_does_not_shatter() {
        let params = FrangibleAmmoParams {
            frangible_type: FrangibleType::LeadFrangible,
            impact_velocity_ms: 200.0, // < 300 threshold
            mass_g: 4.0,
            caliber_mm: 5.56,
        };
        let result = evaluate_frangible_impact(&params, "steel_rha");
        assert!(
            !result.shatters,
            "Lead frangible should NOT shatter below 300 m/s on steel"
        );
        assert!(
            result.residual_mass_g > 3.5,
            "Most mass should be retained below threshold"
        );
    }

    #[test]
    fn sintered_copper_below_threshold_does_not_shatter() {
        let params = FrangibleAmmoParams {
            frangible_type: FrangibleType::SinteredCopper,
            impact_velocity_ms: 100.0, // < 200 threshold
            mass_g: 3.0,
            caliber_mm: 5.56,
        };
        let result = evaluate_frangible_impact(&params, "aluminum_5083");
        assert!(
            !result.shatters,
            "Sintered copper should NOT shatter below 200 m/s"
        );
    }

    // ── Soft target — never shatters ──────────────────────────────────────

    #[test]
    fn frangible_does_not_shatter_in_soft_tissue() {
        let params = FrangibleAmmoParams {
            frangible_type: FrangibleType::LeadFrangible,
            impact_velocity_ms: 500.0, // well above threshold
            mass_g: 4.0,
            caliber_mm: 5.56,
        };
        let result = evaluate_frangible_impact(&params, "soft_tissue");
        assert!(
            !result.shatters,
            "Frangible should NOT shatter in soft tissue regardless of velocity"
        );
    }

    #[test]
    fn frangible_does_not_shatter_in_water() {
        let params = FrangibleAmmoParams {
            frangible_type: FrangibleType::SinteredCopper,
            impact_velocity_ms: 600.0,
            mass_g: 3.0,
            caliber_mm: 5.56,
        };
        let result = evaluate_frangible_impact(&params, "water");
        assert!(!result.shatters, "Frangible should NOT shatter in water");
    }

    // ── Bone impact ───────────────────────────────────────────────────────

    #[test]
    fn frangible_shatters_on_bone_above_threshold() {
        let params = FrangibleAmmoParams {
            frangible_type: FrangibleType::LeadFrangible,
            impact_velocity_ms: 400.0, // > 300 threshold
            mass_g: 4.0,
            caliber_mm: 5.56,
        };
        let result = evaluate_frangible_impact(&params, "cortical_bone");
        assert!(
            result.shatters,
            "Frangible should shatter on bone above threshold"
        );
        assert!(
            result.energy_transferred_j < 0.95 * 0.5 * 0.004 * 400.0 * 400.0,
            "Bone shatter should transfer less energy than full hard shatter"
        );
    }

    #[test]
    fn frangible_on_bone_below_threshold_no_shatter() {
        let params = FrangibleAmmoParams {
            frangible_type: FrangibleType::LeadFrangible,
            impact_velocity_ms: 200.0, // < 300 threshold
            mass_g: 4.0,
            caliber_mm: 5.56,
        };
        let result = evaluate_frangible_impact(&params, "bone");
        assert!(
            !result.shatters,
            "Frangible should NOT shatter on bone below threshold"
        );
    }

    // ── Energy transfer verification ──────────────────────────────────────

    #[test]
    fn shatter_transfers_at_least_95_percent_energy_to_hard_surface() {
        let params = FrangibleAmmoParams {
            frangible_type: FrangibleType::LeadFrangible,
            impact_velocity_ms: 500.0,
            mass_g: 4.0,
            caliber_mm: 5.56,
        };
        let ke = 0.5 * (params.mass_g / 1000.0) * params.impact_velocity_ms.powi(2);
        let result = evaluate_frangible_impact(&params, "steel_rha");
        assert!(result.shatters);
        assert!(
            result.energy_transferred_j >= ke * 0.95,
            "Shatter should transfer ≥ 95 % KE: transferred={:.1} J, 95 % of KE={:.1} J",
            result.energy_transferred_j,
            ke * 0.95
        );
    }

    #[test]
    fn fragment_mass_per_piece_below_0_1g_on_shatter() {
        let params = FrangibleAmmoParams {
            frangible_type: FrangibleType::LeadFrangible,
            impact_velocity_ms: 500.0,
            mass_g: 4.0,
            caliber_mm: 5.56,
        };
        let result = evaluate_frangible_impact(&params, "steel_rha");
        assert!(result.shatters);
        assert!(
            result.avg_fragment_mass_g < 0.1,
            "Average fragment mass should be < 0.1 g on shatter: {} g",
            result.avg_fragment_mass_g
        );
    }

    #[test]
    fn shatter_drastically_reduces_penetration() {
        let params = FrangibleAmmoParams {
            frangible_type: FrangibleType::SinteredCopper,
            impact_velocity_ms: 500.0,
            mass_g: 3.0,
            caliber_mm: 5.56,
        };
        let result = evaluate_frangible_impact(&params, "concrete");
        assert!(result.shatters);
        // Penetration should be very limited compared to non-frangible round
        assert!(
            result.max_penetration_mm < 15.0,
            "Shatter penetration should be < 15 mm: {} mm",
            result.max_penetration_mm
        );
    }

    // ── Different frangible type thresholds ───────────────────────────────

    #[test]
    fn different_frangible_types_have_different_thresholds() {
        let t_lead = fragmentation_threshold(FrangibleType::LeadFrangible);
        let t_copper = fragmentation_threshold(FrangibleType::SinteredCopper);
        let t_match = fragmentation_threshold(FrangibleType::CompositeMatch);
        assert!(
            t_copper < t_lead,
            "Sintered copper threshold ({}) should be lower than lead ({})",
            t_copper,
            t_lead
        );
        assert!(
            t_match > t_copper && t_match < t_lead,
            "Composite match threshold ({}) should be between copper ({}) and lead ({})",
            t_match,
            t_copper,
            t_lead
        );
    }

    // ── Surface classification ────────────────────────────────────────────

    #[test]
    fn surface_category_classifies_correctly() {
        assert_eq!(surface_category("steel_rha"), "hard");
        assert_eq!(surface_category("concrete"), "hard");
        assert_eq!(surface_category("ceramic_al2o3"), "hard");
        assert_eq!(surface_category("aluminum_5083"), "hard");
        assert_eq!(surface_category("era_kontakt5"), "hard");
        assert_eq!(surface_category("cortical_bone"), "bone");
        assert_eq!(surface_category("bone"), "bone");
        assert_eq!(surface_category("skull"), "bone");
        assert_eq!(surface_category("soft_tissue"), "soft");
        assert_eq!(surface_category("water"), "soft");
        assert_eq!(surface_category("ballistic_gelatin"), "soft");
        assert_eq!(surface_category("wood"), "soft");
        assert_eq!(surface_category("rubber"), "soft");
    }

    // ── Edge cases ────────────────────────────────────────────────────────

    #[test]
    fn very_low_velocity_never_shatters() {
        let params = FrangibleAmmoParams {
            frangible_type: FrangibleType::LeadFrangible,
            impact_velocity_ms: 10.0, // << threshold
            mass_g: 4.0,
            caliber_mm: 5.56,
        };
        for surface in &["steel_rha", "concrete", "ceramic_b4c", "bone"] {
            let result = evaluate_frangible_impact(&params, surface);
            assert!(
                !result.shatters,
                "10 m/s should never shatter on {}",
                surface
            );
        }
    }

    #[test]
    fn zero_mass_returns_sensible_result() {
        let params = FrangibleAmmoParams {
            frangible_type: FrangibleType::LeadFrangible,
            impact_velocity_ms: 400.0,
            mass_g: 0.0,
            caliber_mm: 5.56,
        };
        let result = evaluate_frangible_impact(&params, "steel_rha");
        assert!(!result.shatters, "Zero-mass projectile cannot shatter");
        assert_eq!(result.energy_transferred_j, 0.0);
        assert_eq!(result.residual_mass_g, 0.0);
        assert_eq!(result.residual_velocity_ms, 0.0);
    }

    #[test]
    fn deterministic_output() {
        // Two identical evaluations must produce identical results
        let params = FrangibleAmmoParams {
            frangible_type: FrangibleType::SinteredCopper,
            impact_velocity_ms: 450.0,
            mass_g: 3.5,
            caliber_mm: 7.62,
        };
        let a = evaluate_frangible_impact(&params, "steel_rha");
        let b = evaluate_frangible_impact(&params, "steel_rha");
        assert_eq!(a.shatters, b.shatters);
        assert!((a.energy_transferred_j - b.energy_transferred_j).abs() < 1e-12);
        assert!((a.residual_mass_g - b.residual_mass_g).abs() < 1e-12);
        assert!((a.avg_fragment_mass_g - b.avg_fragment_mass_g).abs() < 1e-12);
    }

    // ── Helper function tests ─────────────────────────────────────────────

    #[test]
    fn residual_fragment_velocity_capped() {
        // Very high KE + tiny mass → should be capped, not follow sqrt(2KE/m)
        let v = residual_fragment_velocity(10_000.0, 0.001);
        assert!(
            v <= MAX_FRAGMENT_VELOCITY_MS,
            "Residual velocity should be capped at {} m/s, got {}",
            MAX_FRAGMENT_VELOCITY_MS,
            v
        );
    }

    #[test]
    fn residual_fragment_velocity_zero_for_no_mass() {
        assert_eq!(residual_fragment_velocity(100.0, 0.0), 0.0);
        assert_eq!(residual_fragment_velocity(0.0, 0.1), 0.0);
    }

    #[test]
    fn below_threshold_penetration_scales_with_caliber() {
        let small = FrangibleAmmoParams {
            frangible_type: FrangibleType::LeadFrangible,
            impact_velocity_ms: 200.0,
            mass_g: 2.0,
            caliber_mm: 5.56,
        };
        let large = FrangibleAmmoParams {
            frangible_type: FrangibleType::LeadFrangible,
            impact_velocity_ms: 200.0,
            mass_g: 8.0,
            caliber_mm: 9.0,
        };
        let r_s = evaluate_frangible_impact(&small, "steel_rha");
        let r_l = evaluate_frangible_impact(&large, "steel_rha");
        assert!(
            r_l.max_penetration_mm > r_s.max_penetration_mm,
            "Larger calibre should penetrate deeper below threshold"
        );
    }
}
