// ABE — Behind-Armour Debris (BAD) Model
//
// Physics-inspired model for behind-armour debris generation following
// a perforating or non-perforating impact.  Uses De Marre residual
// velocity to compute energy deposited into the armour, then derives:
//
//   - Spall fragment count and total mass
//   - Spall / debris spray cone angles (widening with obliquity)
//   - Temporary cavity diameter and volume
//   - Behind-armour lethality index (BALI)
//
// A post-processing calibration layer ([`ThreatLevel`] /
// [`classify_bad_threat`]) maps BALI to discrete threat categories
// aligned with STANAG 4569 Level I–V analogues.
//
// References:
//   - BRL Report 1662 (Behind-Armour Debris)
//   - NATO AEP-2920 (Terminal Ballistics)
//   - NATO AEP-55 (Protection Level Evaluation)
//   - STANAG 4569 (Ed. 3) — Protection Levels I–V
//   - Bless & Rosenberg (1985) — spall thresholds

use std::f64::consts::PI;

use crate::penetration::material_factor;

/// Parameters for a behind-armour debris evaluation.
///
/// These are the inputs describing the projectile–armour interaction
/// after a penetration/impact event.
#[derive(Debug, Clone)]
pub struct BehindArmorDebrisParams {
    /// Projectile impact velocity (m/s).
    pub impact_velocity_ms: f64,
    /// Projectile mass (kg).
    pub projectile_mass_kg: f64,
    /// Projectile calibre / diameter (m).
    pub caliber_m: f64,
    /// Armour plate thickness (m).
    pub armor_thickness_m: f64,
    /// Armour material identifier (see [`material_factor`]).
    pub armor_material: String,
    /// Impact angle from surface normal (degrees; 0 = perpendicular).
    pub impact_angle_deg: f64,
    /// Projectile type identifier (e.g. "ball", "ap", "apfsds").
    pub projectile_type: String,
    /// Number of projectile fragments from the fragmentation model.
    pub projectile_fragments: i32,
    /// Residual velocity after armour perforation (m/s).
    /// For ricochet, this is the outgoing ricochet velocity.
    /// For non-penetrating hits, this is a small fraction of impact velocity.
    pub residual_velocity_ms: f64,
    /// Whether the projectile fully perforated the plate.
    pub penetrated: bool,
}

/// Result of a behind-armour debris evaluation.
#[derive(Debug, Clone)]
pub struct BehindArmorDebrisResult {
    /// Total number of spall / armour fragments ejected behind the plate.
    pub num_spall_fragments: i32,
    /// Total mass of all spall fragments (grams).
    pub spall_mass_g: f64,
    /// Full cone angle of the spall spray (degrees).
    pub spall_cone_angle_deg: f64,
    /// Core debris spray half-cone angle (degrees); narrower than the
    /// full spall cone.
    pub debris_spray_cone_deg: f64,
    /// Average debris velocity (m/s).
    pub debris_velocity_ms: f64,
    /// Diameter of the temporary cavity formed in the armour (mm).
    pub temp_cavity_diameter_mm: f64,
    /// Volume of the temporary cavity (cm³ / cc).
    pub temp_cavity_volume_cc: f64,
    /// Behind-armour lethality index (dimensionless, higher = more
    /// lethal behind-armour effects).
    pub behind_armor_lethality_index: f64,
}

/// Classification of the threat posed by behind-armour debris.
///
/// Maps the continuous [`BehindArmorDebrisResult::behind_armor_lethality_index`]
/// to discrete categories following the severity scale used in
/// NATO AEP-2920 terminal ballistic assessments.
///
/// The categories are monotonic: each successive variant represents an
/// unambiguously higher threat level, mirroring the escalation in
/// STANAG 4569 protection levels (I → V).
///
/// # References
///
/// | Variant     | BALI range | STANAG 4569 analogue              |
/// |-------------|------------|-----------------------------------|
/// | `None`      | < 0.1      | Below Level I (no protection need) |
/// | `Minimal`   | [0.1, 1.0) | Level I — small arms suppressed    |
/// | `Moderate`  | [1.0, 5.0) | Level II — rifle-ball stopped      |
/// | `Substantial`| [5.0, 20.0)| Level III — rifle-AP defeated      |
/// | `Critical`  | ≥ 20.0     | Level IV–V — heavy / cannon threat |
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ThreatLevel {
    /// BALI < 0.1 — no significant behind-armour debris.
    ///
    /// Corresponds to impacts that do not perforate the armour or that
    /// produce negligible ejecta (e.g., small arms on thick plate).
    None,
    /// 0.1 ≤ BALI < 1.0 — limited spall; minor behind-armour effects.
    ///
    /// Typical of non-perforating impacts or spall-liner–controlled
    /// perforations.  Below the threshold for crew injury.
    Minimal,
    /// 1.0 ≤ BALI < 5.0 — moderate debris spray; casualty-producing
    /// potential.
    ///
    /// Corresponds to small-calibre AP or ball perforation of
    /// thin-to-moderate armour.  Fragments may cause soft-tissue
    /// wounds.
    Moderate,
    /// 5.0 ≤ BALI < 20.0 — substantial debris; high probability of
    /// crew injury or equipment damage.
    ///
    /// Typical of rifle-calibre AP perforation of structural armour
    /// or multiple fragment impacts.
    Substantial,
    /// BALI ≥ 20.0 — critical debris spray; catastrophic behind-armour
    /// effects.
    ///
    /// Corresponds to heavy machine-gun or cannon-calibre perforation.
    /// Vehicle-level damage or crew incapacitation is highly likely.
    Critical,
}

/// Classify a behind-armour lethality index into a discrete threat level.
///
/// Maps the continuous BALI score produced by [`evaluate_bad`] to one of
/// five threat categories (see [`ThreatLevel`]).
///
/// The mapping is **monotonic** — higher BALI always yields a higher
/// threat category — and applies a noise floor at BALI < 0.1 to suppress
/// false positives from numerical noise.
///
/// # Reference
///
/// The threat categories are informed by:
/// - **STANAG 4569** (Ed. 3) — Protection levels for logistic and
///   armoured vehicles, Levels I–V.
/// - **NATO AEP-2920** — Terminal Ballistics Methodology.
/// - **NATO AEP-55** — Procedures for evaluating the protection level
///   of logistic vehicles.
///
/// # Example
///
/// ```
/// use abe::penetration::behind_armor_debris::classify_bad_threat;
///
/// let level = classify_bad_threat(3.2);
/// assert_eq!(level, ThreatLevel::Moderate);
/// ```
pub fn classify_bad_threat(bali: f64) -> ThreatLevel {
    match bali {
        b if b < 0.1 => ThreatLevel::None,
        b if b < 1.0 => ThreatLevel::Minimal,
        b if b < 5.0 => ThreatLevel::Moderate,
        b if b < 20.0 => ThreatLevel::Substantial,
        _ => ThreatLevel::Critical,
    }
}

/// Mott distribution fragment mass for BAD sampling.
/// Behind-armor fragment sizes follow a Mott distribution (1943):
///   p(m) = (1/μ) * exp(-(m/μ)^0.5) / (2*(m/μ)^0.5)
/// where μ = avg_mass / k, k ≈ 1.0 for steel projectiles.
/// This enables stochastic fragment mass variation around the config avg.
///
/// Reference: Mott, N.F., "A Theory of Fragmentation of Shells and Bombs",
/// Ministry of Supply AC 3642, 1943. Validated against WW2 fragment data.
///
/// Current BAD evaluation uses avg_frag_mass as a deterministic value;
/// this function provides the distribution for future stochastic sampling.
pub fn mott_fragment_mass(avg_mass: f64, percentile: f64) -> f64 {
    // Inverse CDF of the Mott distribution
    // The ν-th percentile fragment mass = μ * (ln(1/(1-ν)))²
    // where μ = avg_mass (for k=1 steel assumption)
    avg_mass * (-(1.0 - percentile.clamp(0.0, 0.999))).ln().powi(2)
}

/// Evaluate behind-armour debris for a given impact event.
///
/// Computes spall properties from the energy deposited in the armour
/// (`ΔE = ½·m·(v_impact² – v_residual²)`), scaled by the t/d ratio,
/// material properties, and impact obliquity.
///
/// Fragment mass follows a Mott distribution in reality (see
/// [`mott_fragment_mass`]); the current model uses a deterministic
/// average fragment mass tied to material hardness as a pragmatic
/// simplification for game-context BAD evaluation.
pub fn evaluate_bad(params: &BehindArmorDebrisParams) -> BehindArmorDebrisResult {
    let t_d_ratio = if params.caliber_m > 0.0 {
        params.armor_thickness_m / params.caliber_m
    } else {
        0.5
    };

    let angle_rad = params.impact_angle_deg.to_radians();

    // ── Energy deposited in the armour ────────────────────────────────────
    // General formula works for penetration, ricochet, and non-penetration
    // by using the actual residual velocity.
    let energy_deposited = if params.residual_velocity_ms < params.impact_velocity_ms {
        0.5 * params.projectile_mass_kg
            * (params.impact_velocity_ms.powi(2) - params.residual_velocity_ms.powi(2))
    } else {
        0.5 * params.projectile_mass_kg * params.impact_velocity_ms.powi(2) * 0.95
    };

    // ── Material properties ───────────────────────────────────────────────
    let mat_factor = material_factor(&params.armor_material);
    let mat_lower = params.armor_material.to_lowercase();
    let is_spall_liner = mat_lower.contains("spall") || mat_lower.contains("liner");

    // ── Spall efficiency vs thickness/calibre ratio ───────────────────────
    // Gaussian-shaped efficiency centred at t/d ≈ 0.6 (where ductile hole
    // enlargement produces the most ejecta).  Falls off for very thin
    // plates (petaling, less spall) and thick plates (plugging, less
    // ejecta).
    let spall_efficiency = (-((t_d_ratio - 0.6).powi(2)) / (2.0 * 0.28_f64.powi(2))).exp();

    // ── Spall mass ───────────────────────────────────────────────────────
    // Base spall mass: fraction of deposited energy converted to ejecta,
    // scaled inversely with material hardness (softer → more spall).
    // Spall liners get a large explicit reduction.
    let spall_factor = if is_spall_liner {
        0.05 // liners are ~20× more effective at containing spall than RHA
    } else {
        1.0 / mat_factor.max(0.01)
    };

    // Energy-to-spall-mass conversion: ~5 % of deposited energy goes into
    // spall fragment mass (fragment KE + surface energy), with the specific
    // energy per kg varying by material.  5 × 10⁻⁶ gives ~7 g of spall per
    // 1 600 J deposited (typical 7.62 mm M80 vs 5 mm RHA).
    let mut spall_mass_kg = 5e-6 * spall_efficiency * energy_deposited * spall_factor;

    // Volume bound: limit spall to the armour material in the impact zone
    // (rough cylinder 2 calibres diameter × plate thickness).
    let max_spall_vol = params.armor_thickness_m * (params.caliber_m * 2.0).powi(2) * 0.785;
    let max_spall_mass_kg = max_spall_vol * 7850.0; // steel density as upper bound
    spall_mass_kg = spall_mass_kg.min(max_spall_mass_kg);

    // ── Fragment count ───────────────────────────────────────────────────
    // Average fragment mass depends on material hardness.  Harder → smaller
    // fragments → higher count for the same mass.  Base ~0.3 g for RHA.
    let avg_frag_mass_kg = 0.0003 / mat_factor.max(0.01).sqrt().max(0.1);

    let num_fragments = if spall_mass_kg > 0.0 && avg_frag_mass_kg > 0.0 {
        (spall_mass_kg / avg_frag_mass_kg).round() as i32
    } else {
        0
    };
    let num_fragments = num_fragments.clamp(0, 200);

    // ── Cone angles ──────────────────────────────────────────────────────
    // Base cone 15° for normal impact, widening with obliquity and t/d.
    let spall_cone_angle_deg = 15.0 + 25.0 * angle_rad.sin() + 5.0 * t_d_ratio.min(2.0);
    let debris_spray_cone_deg = 10.0 + 20.0 * angle_rad.sin() + 3.0 * t_d_ratio.min(1.5);

    // ── Debris velocity ──────────────────────────────────────────────────
    // Verolme angle-dependent model: V_debris(θ) = V_residual × cos(1.92θ)
    // At 0° (normal): full residual → more energetic BAD at normal impact.
    // At 45°: drops to ~0.06× → less BAD at oblique impacts.
    let debris_velocity_ms = params.residual_velocity_ms * (1.92 * angle_rad).cos();

    // ── Temporary cavity ─────────────────────────────────────────────────
    // Energy density (J/m³) in the impact zone drives cavity expansion.
    let cavity_vol_m3 = (params.armor_thickness_m * params.caliber_m.powi(2) * 0.785).max(1e-12);
    let edens = energy_deposited / cavity_vol_m3;

    // Cavity diameter scaling: 2–5× calibre for RHA at typical rifle energies.
    let cav_diam_ratio = (2.0 + 2.5 * (edens / 1e9).sqrt().min(3.0)) / mat_factor.powf(0.15);

    let temp_cavity_diameter_mm = params.caliber_m * 1000.0 * cav_diam_ratio;
    let cav_r_mm = temp_cavity_diameter_mm / 2.0;
    let temp_cavity_volume_cc =
        PI * cav_r_mm.powi(2) * (params.armor_thickness_m * 1000.0) / 1000.0;

    // ── Lethality index ──────────────────────────────────────────────────
    // Scaled combination: 70 % debris kinetic energy, 30 % fragment count.
    let debris_ke = 0.5 * spall_mass_kg * debris_velocity_ms.powi(2);
    let lethality = if debris_ke > 0.0 {
        (num_fragments as f64).powf(0.3) * (debris_ke / 1000.0).powf(0.7)
    } else {
        0.0
    };

    BehindArmorDebrisResult {
        num_spall_fragments: num_fragments,
        spall_mass_g: spall_mass_kg * 1000.0,
        spall_cone_angle_deg,
        debris_spray_cone_deg,
        debris_velocity_ms,
        temp_cavity_diameter_mm,
        temp_cavity_volume_cc,
        behind_armor_lethality_index: lethality,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_params(
        impact_velocity_ms: f64,
        armor_thickness_m: f64,
        armor_material: &str,
        impact_angle_deg: f64,
        residual_velocity_ms: f64,
        penetrated: bool,
    ) -> BehindArmorDebrisParams {
        BehindArmorDebrisParams {
            impact_velocity_ms,
            projectile_mass_kg: 0.0095, // 7.62×51 mm M80 ball
            caliber_m: 0.00762,
            armor_thickness_m,
            armor_material: armor_material.to_string(),
            impact_angle_deg,
            projectile_type: "ball".to_string(),
            projectile_fragments: 5,
            residual_velocity_ms,
            penetrated,
        }
    }

    /// Thin armour produces abundant spall on perforation.
    #[test]
    fn thin_armor_produces_lots_of_spall() {
        // 7.62 mm M80 ball at 853 m/s vs 5 mm RHA (t/d ≈ 0.66 → near peak efficiency)
        let result = evaluate_bad(&make_params(853.0, 0.005, "steel_rha", 0.0, 380.0, true));
        assert!(
            result.num_spall_fragments >= 8,
            "thin RHA should produce ≥8 spall fragments: {}",
            result.num_spall_fragments
        );
        assert!(
            result.spall_mass_g > 0.5,
            "spall mass should be meaningful: {} g",
            result.spall_mass_g
        );
    }

    /// Thick armour that stops the projectile produces fewer spall fragments.
    #[test]
    fn thick_armor_produces_less_spall() {
        let thin = evaluate_bad(&make_params(853.0, 0.005, "steel_rha", 0.0, 380.0, true));
        let thick = evaluate_bad(&make_params(
            853.0,
            0.050,
            "steel_rha",
            0.0,
            85.0, // residual = 853*0.1 ≈ 85 m/s for non-penetrating
            false,
        ));
        // Thick plate that is not perforated should produce less spall
        // because the t/d ratio is far from the efficiency peak.
        assert!(
            thick.num_spall_fragments <= thin.num_spall_fragments,
            "thick armour should spall no more than thin: thin={}, thick={}",
            thin.num_spall_fragments,
            thick.num_spall_fragments
        );
    }

    /// Spall liners significantly reduce behind-armour debris.
    #[test]
    fn spall_liner_reduces_spall() {
        let rha = evaluate_bad(&BehindArmorDebrisParams {
            armor_material: "steel_rha".to_string(),
            ..make_params(853.0, 0.005, "steel_rha", 0.0, 380.0, true)
        });
        let liner = evaluate_bad(&BehindArmorDebrisParams {
            armor_material: "spall_liner_kevlar".to_string(),
            ..make_params(853.0, 0.005, "steel_rha", 0.0, 380.0, true)
        });
        assert!(
            liner.num_spall_fragments < rha.num_spall_fragments,
            "spall liner should reduce fragment count: rha={}, liner={}",
            rha.num_spall_fragments,
            liner.num_spall_fragments
        );
        assert!(
            liner.spall_mass_g < rha.spall_mass_g,
            "spall liner should reduce spall mass: rha={:.2}g, liner={:.2}g",
            rha.spall_mass_g,
            liner.spall_mass_g
        );
    }

    /// Oblique impact widens the debris cone angle.
    #[test]
    fn angled_impact_widens_spall_cone() {
        let normal = evaluate_bad(&make_params(853.0, 0.005, "steel_rha", 0.0, 380.0, true));
        let oblique = evaluate_bad(&make_params(853.0, 0.005, "steel_rha", 45.0, 380.0, true));

        assert!(
            oblique.spall_cone_angle_deg > normal.spall_cone_angle_deg,
            "oblique impact should widen spall cone: normal={:.1}°, oblique={:.1}°",
            normal.spall_cone_angle_deg,
            oblique.spall_cone_angle_deg
        );
        assert!(
            oblique.debris_spray_cone_deg > normal.debris_spray_cone_deg,
            "oblique impact should widen debris cone"
        );
    }

    /// The BAD model is deterministic — same inputs → same outputs.
    #[test]
    fn deterministic_output() {
        let params = make_params(853.0, 0.005, "steel_rha", 0.0, 380.0, true);
        let a = evaluate_bad(&params);
        let b = evaluate_bad(&params);
        assert_eq!(a.num_spall_fragments, b.num_spall_fragments);
        assert!((a.spall_cone_angle_deg - b.spall_cone_angle_deg).abs() < 1e-12);
        assert!((a.behind_armor_lethality_index - b.behind_armor_lethality_index).abs() < 1e-12);
    }

    // ── Threat-level calibration tests ───────────────────────────────────────

    /// M80 ball perforating 5 mm RHA (t/d ≈ 0.66, near peak efficiency)
    /// produces at least Moderate behind-armour threat.
    #[test]
    fn thin_rha_classified_at_least_moderate() {
        let result = evaluate_bad(&make_params(853.0, 0.005, "steel_rha", 0.0, 380.0, true));
        let level = classify_bad_threat(result.behind_armor_lethality_index);
        assert!(
            level >= ThreatLevel::Moderate,
            "thin RHA perforation should be ≥ Moderate (BALI={:.3}): got {:?}",
            result.behind_armor_lethality_index,
            level,
        );
    }

    /// 50 mm RHA stopping M80 ball produces negligible debris → None.
    #[test]
    fn thick_rha_classified_none() {
        let result = evaluate_bad(&make_params(
            853.0,
            0.050,
            "steel_rha",
            0.0,
            85.0, // residual ≈ 10 % impact velocity, non-penetrating
            false,
        ));
        let level = classify_bad_threat(result.behind_armor_lethality_index);
        assert_eq!(
            level,
            ThreatLevel::None,
            "thick RHA (non-perforating) should be None (BALI={:.3})",
            result.behind_armor_lethality_index,
        );
    }

    /// Spall-liner–controlled perforation suppresses debris to Minimal or
    /// below, even when the geometry (5 mm, normal) would otherwise be
    /// efficient at generating spall.
    #[test]
    fn spall_liner_classified_at_most_minimal() {
        let result = evaluate_bad(&BehindArmorDebrisParams {
            armor_material: "spall_liner_kevlar".to_string(),
            ..make_params(853.0, 0.005, "steel_rha", 0.0, 380.0, true)
        });
        let level = classify_bad_threat(result.behind_armor_lethality_index);
        assert!(
            level <= ThreatLevel::Minimal,
            "spall liner should keep threat ≤ Minimal (BALI={:.3}): got {:?}",
            result.behind_armor_lethality_index,
            level,
        );
    }

    /// The ThreatLevel ordering is verified with explicit boundary values
    /// to ensure monotonicity.
    #[test]
    fn threat_level_monotonic() {
        assert!(ThreatLevel::None < ThreatLevel::Minimal);
        assert!(ThreatLevel::Minimal < ThreatLevel::Moderate);
        assert!(ThreatLevel::Moderate < ThreatLevel::Substantial);
        assert!(ThreatLevel::Substantial < ThreatLevel::Critical);
    }

    /// Edge-case BALI values at each boundary produce the expected level.
    #[test]
    fn classification_boundaries() {
        assert_eq!(classify_bad_threat(0.0), ThreatLevel::None);
        assert_eq!(classify_bad_threat(0.0999), ThreatLevel::None);
        assert_eq!(classify_bad_threat(0.1), ThreatLevel::Minimal);
        assert_eq!(classify_bad_threat(0.999), ThreatLevel::Minimal);
        assert_eq!(classify_bad_threat(1.0), ThreatLevel::Moderate);
        assert_eq!(classify_bad_threat(4.999), ThreatLevel::Moderate);
        assert_eq!(classify_bad_threat(5.0), ThreatLevel::Substantial);
        assert_eq!(classify_bad_threat(19.999), ThreatLevel::Substantial);
        assert_eq!(classify_bad_threat(20.0), ThreatLevel::Critical);
        assert_eq!(classify_bad_threat(100.0), ThreatLevel::Critical);
    }
}
