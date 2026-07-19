// ABE - Combined Effects Model (API / HEI / Incendiary)
//
// Models the combined terminal effects of ammunition that pairs
// penetration/kinetic damage with incendiary and/or high-explosive
// payloads: Armor-Piercing Incendiary (API), High-Explosive Incendiary
// (HEI), and Semi-Armor-Piercing HEI (SAP-HEI) types.
//
// Physics summary:
//   API — hardened steel/tungsten core + incendiary filler. On
//         impact (soft target) or after armour perforation, the filler
//         ignites via compressive heating or friction (500–2000 °C).
//         Fire probability scales with residual kinetic energy and
//         incendiary mass fraction.
//   HEI — high-explosive filler + incendium material. Impact fuze
//         (instantaneous) or delay fuze (10–50 µs, through-armour
//         burst). Blast overpressure ∝ charge^⅓ · r^-1. Container
//         produces casing fragments; burning particles scatter over
//         5–15 m.
//   SAP-HEI — semi-armour-piercing body with delay fuze: penetrates
//         light armour then detonates behind it.
//   Tracer variants — pyrotechnic trace element extends burn
//         duration after primary ignition.
//
// References:
//   - Jane's Ammunition Handbook (2022)
//   - M8 API (.50 BMG, 42 g total, ~18 g incendiary, Zr/thermite)
//   - B32 API (7.62×54R, 10.0 g total, ~1.4 g incendiary, phosphorus)
//   - M1 HEI (20×102 mm, 102 g total, ~11 g explosive + incendium)
//   - TM 43-0001-27 (US Army Ammunition Data Sheets)
//   - Nennstiel, R., "Terminal Ballistics of API Projectiles" (1989)

// ── Physical constants ──────────────────────────────────────────────────────────

/// Minimum impact velocity (m/s) to reliably ignite incendiary filler
/// through compressive heating or friction.
const IGNITION_VELOCITY_THRESHOLD_MS: f64 = 300.0;

/// Minimum residual velocity (m/s) after armour penetration for
/// post-penetration incendiary ignition.
const IGNITION_POST_PEN_MS: f64 = 150.0;

/// Minimum mass fraction of incendiary filler (filler / total projectile)
/// required for reliable ignition.
const IGNITION_MIN_FILLER_FRACTION: f64 = 0.02;

/// Nominal combustion temperature range for common incendiary
/// materials (zirconium, thermite, phosphorus) in °C.
/// Used only for internal calibration; not exposed in the output struct.
const _INCENDIARY_TEMP_MIN_C: f64 = 800.0;
const _INCENDIARY_TEMP_MAX_C: f64 = 2000.0;

/// Fire radius scaling constant:  radius (m) = K · m_inc^⅓.
const FIRE_RADIUS_K: f64 = 0.75;

/// Maximum credible open-air fire radius from a single projectile (m).
const FIRE_RADIUS_MAX_M: f64 = 5.0;

/// Minimum fire radius when ignited (m).
const FIRE_RADIUS_MIN_M: f64 = 0.3;

/// Burn duration per gram of incendiary filler (s/g).
const BURN_DURATION_S_PER_G: f64 = 15.0;

/// Tracer extension to burn duration (s) — the trace pellet adds its
/// own burn time.
const TRACER_BURN_EXTENSION_S: f64 = 2.0;

// ── Explosive (HE) model constants ──────────────────────────────────────────────

/// Blast overpressure at 1 m reference distance, scaled by charge mass.
///   P_ref = K · m_ch^⅓   (kPa)
const BLAST_REF_K: f64 = 750.0;

/// Minimum overpressure for damage-causing blast (kPa).
const BLAST_DAMAGE_THRESHOLD_KPA: f64 = 15.0;

/// Maximum credible explosive filler in small-arms HEI rounds (g).
const HEI_MAX_CHARGE_G: f64 = 5.0;

/// Minimum velocity to avoid dud on impact for HEI impact fuze (m/s).
const HEI_FUZE_ARM_MS: f64 = 200.0;

/// Number of casing fragments per gram of casing mass^0.8.
const FRAGMENTS_PER_G_CASING: f64 = 1.8;

/// Base fragment spray cone half-angle for HEI detonation (degrees).
const HEI_FRAGMENT_SPRAY_BASE_DEG: f64 = 30.0;

/// Minimum fragment spray half-angle (degrees).
const HEI_FRAGMENT_SPRAY_MIN_DEG: f64 = 15.0;

/// Maximum fragment spray half-angle (degrees).
const HEI_FRAGMENT_SPRAY_MAX_DEG: f64 = 60.0;

/// Maximum number of credible HEI casing fragments.
const HEI_FRAGMENTS_MAX: i32 = 200;

#[allow(dead_code)] // ponytail: HEI scatter scaling, wire when combined effects fully integrated
/// Incendiary particle scatter radius scaling for HEI.
///   scatter (m) = K · m_filler^⅓
const HEI_PARTICLE_SCATTER_K: f64 = 4.0;

// ── Fuze delays ─────────────────────────────────────────────────────────────────

const _FUZE_INSTANTANEOUS_US: f64 = 0.0;
const _FUZE_DELAY_MIN_US: f64 = 10.0;
const _FUZE_DELAY_MAX_US: f64 = 50.0;

// ── Types ───────────────────────────────────────────────────────────────────────

/// Types of combined-effect (incendiary / explosive) ammunition.
///
/// Variant names follow military ammunition designations (API, HEI, SAP)
/// which conventionally use SCREAMING_SNAKE_CASE in technical documentation.
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(non_camel_case_types)]
pub enum CombinedType {
    /// Armor-Piercing Incendiary. Hardened core + incendiary filler.
    /// Ignites on impact or after armour penetration.
    API,
    /// API with a pyrotechnic trace element that extends the visual
    /// burn signature.
    API_Tracer,
    /// High-Explosive Incendiary. Explosive filler + incendium
    /// material. Instantaneous impact fuze.
    HEI,
    /// HEI with a trace element.
    HEI_Tracer,
    /// Semi-Armor-Piercing HEI. Thinner hardened body + delay fuze
    /// for through-armour detonation.
    SAP_HEI,
}

// ── Input / output structs ──────────────────────────────────────────────────────

/// Input parameters describing the projectile, its filler, and the
/// armour-interaction outcome.
#[derive(Debug, Clone, Copy)]
pub struct CombinedAmmoParams {
    /// Which type of combined-effect ammunition.
    pub combined_type: CombinedType,
    /// Total projectile mass in grams (including filler and core).
    pub projectile_mass_g: f64,
    /// Projectile calibre in millimetres.
    pub caliber_mm: f64,
    /// Mass of the incendiary / explosive filler in grams.
    pub filler_mass_g: f64,
    /// Projectile velocity at impact in m/s.
    pub impact_velocity_ms: f64,
    /// Whether the projectile perforated the armour plate.
    pub armor_penetrated: bool,
    /// Projectile velocity remaining after armour penetration in m/s.
    pub residual_velocity_ms: f64,
    /// Impact angle from surface normal in degrees
    /// (0 = perpendicular, 90 = grazing).
    pub impact_angle_deg: f64,
}

/// The combined terminal effects of an API / HEI projectile impact.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CombinedEffectResult {
    /// Whether the incendiary filler ignited.
    pub incendiary_ignition: bool,
    /// Radius of the fireball / burning area in metres (0 if no ignition).
    pub fire_radius_m: f64,
    /// Duration of sustained burning in seconds (0 if no ignition).
    pub burn_duration_s: f64,
    /// Whether the high-explosive filler detonated.
    pub explosive_detonation: bool,
    /// Peak blast overpressure at 1 m reference in kPa (0 if no detonation).
    pub blast_overpressure_kpa: f64,
    /// Radius within which overpressure exceeds the damage threshold (m).
    pub blast_radius_m: f64,
    /// Number of casing/projectile fragments produced by the HE detonation.
    pub he_fragments: i32,
    /// Spray cone half-angle for HE fragments (degrees).
    pub he_fragment_spray_angle_deg: f64,
    /// Probability (0–1) that nearby flammable materials catch fire
    /// from hot fragments or blast.
    pub secondary_ignition_chance: f64,
}

impl Default for CombinedEffectResult {
    fn default() -> Self {
        Self {
            incendiary_ignition: false,
            fire_radius_m: 0.0,
            burn_duration_s: 0.0,
            explosive_detonation: false,
            blast_overpressure_kpa: 0.0,
            blast_radius_m: 0.0,
            he_fragments: 0,
            he_fragment_spray_angle_deg: 0.0,
            secondary_ignition_chance: 0.0,
        }
    }
}

// ── Core evaluation ─────────────────────────────────────────────────────────────

/// Evaluate the combined terminal effects (incendiary, explosive,
/// fragmentation) after a projectile impact or armour interaction.
///
/// # Arguments
/// * `params` — Combined ammunition parameters describing the projectile,
///   filler, impact conditions, and penetration outcome.
///
/// # Returns
/// A [`CombinedEffectResult`] with ignition status, fire spread, blast
/// parameters, fragmentation, and secondary ignition risk.  All
/// calculations are **deterministic** — the same input always produces
/// the same output.
///
/// # Physics
/// **API/AP-I** — incendiary ignition occurs when the impact (or
/// residual post-penetration) velocity exceeds a threshold and the
/// filler mass fraction is sufficient.  Fire radius scales with
/// `m_filler^⅓`; burn duration is proportional to filler mass.  Tracer
/// variants add a few seconds of additional burn.
///
/// **HEI** — explosive detonation is governed by fuze type:
///   - Impact fuze (HEI, HEI_Tracer): arms when velocity > ~200 m/s
///     and filler mass > threshold.
///   - Delay fuze (SAP_HEI): detonates after armour penetration, or
///     on impact if the projectile was stopped (self-destruct).
///   Blast overpressure ∝ `m_charge^⅓` · `r^{-1}`; the casing produces
///   fragments according to its mass.
///
/// **Secondary ignition** — hot fragments and blast thermal flux
/// contribute a 0–0.95 probability of igniting nearby flammable
/// materials.
pub fn evaluate_combined_effects(params: &CombinedAmmoParams) -> CombinedEffectResult {
    // ── Guard clauses ──────────────────────────────────────────────────────
    if params.filler_mass_g <= 0.0
        || params.projectile_mass_g <= 0.0
        || params.impact_velocity_ms <= 0.0
    {
        return CombinedEffectResult::default();
    }

    // ── Convenience aliases ────────────────────────────────────────────────
    let m_tot = params.projectile_mass_g;
    let m_fill = params.filler_mass_g;
    let filler_frac = m_fill / m_tot.max(1e-6);
    let v_impact = params.impact_velocity_ms;
    let v_residual = params.residual_velocity_ms;
    let pen = params.armor_penetrated;
    let angle_deg = params.impact_angle_deg;

    // Whether the combined type carries a tracer element.
    let is_tracer = matches!(
        params.combined_type,
        CombinedType::API_Tracer | CombinedType::HEI_Tracer
    );

    // ── Incendiary ignition (API and HEI both carry incendium) ────────────

    // Effective velocity for ignition: residual after penetration, else impact.
    let v_eff = if pen { v_residual } else { v_impact };
    let v_ok =
        v_eff >= IGNITION_VELOCITY_THRESHOLD_MS || (pen && v_residual >= IGNITION_POST_PEN_MS);

    // Angle penalty: glancing impacts reduce compressive heating.
    let angle_rad = angle_deg.to_radians();
    let angle_factor = angle_rad.cos().max(0.15); // minimum 15 % at extreme angles

    // The "fury" parameter — combined energy × filler mass × angle factor.
    // Impact KE available for ignition.
    let ke = if pen {
        0.5 * (m_tot / 1000.0) * v_residual.powi(2).max(50.0)
    } else {
        0.5 * (m_tot / 1000.0) * v_impact.powi(2)
    };
    let fury = ke * filler_frac * angle_factor;

    // Deterministic threshold: ignition when fury > 40 J·frac (empirical).
    let incendiary_ignition = v_ok && filler_frac >= IGNITION_MIN_FILLER_FRACTION && fury > 40.0;

    // ── Fire spread and burn duration ──────────────────────────────────────

    let (fire_radius_m, burn_duration_s) = if incendiary_ignition {
        // Radius scales with m_filler^⅓ — a 10 g filler gives ~1.6 m fire.
        let r = FIRE_RADIUS_K * m_fill.powf(1.0 / 3.0);
        let r = r.clamp(FIRE_RADIUS_MIN_M, FIRE_RADIUS_MAX_M);

        // Base burn duration proportional to filler mass.
        let mut dur = m_fill * BURN_DURATION_S_PER_G;
        // Tracer adds a sustained 2 s visual signature.
        if is_tracer {
            dur += TRACER_BURN_EXTENSION_S;
        }
        let dur = dur.clamp(3.0, 600.0);

        (r, dur)
    } else {
        (0.0, 0.0)
    };

    // ── HEI explosive detonation ───────────────────────────────────────────

    let is_hei = matches!(
        params.combined_type,
        CombinedType::HEI | CombinedType::HEI_Tracer | CombinedType::SAP_HEI
    );

    let explosive_detonation = if is_hei && m_fill >= 0.3 {
        // Minimum filler mass for a credible HE detonation (~0.3 g).
        match params.combined_type {
            CombinedType::SAP_HEI => {
                // Delay fuze: detonates after armour penetration, or on
                // impact at high velocity (self-destruct if stopped).
                pen || v_impact > 500.0
            },
            _ => {
                // Impact fuze: arm only above a minimum velocity to
                // prevent low-velocity duds.
                v_impact >= HEI_FUZE_ARM_MS
            },
        }
    } else {
        false
    };

    // ── Blast overpressure ─────────────────────────────────────────────────

    let (blast_overpressure_kpa, blast_radius_m) = if explosive_detonation {
        let chg = m_fill.min(HEI_MAX_CHARGE_G);
        // Peak overpressure at 1 m: P_ref = K · m_ch^⅓
        let p_ref = BLAST_REF_K * chg.powf(1.0 / 3.0);
        // Distance where overpressure decays to threshold.
        // Overpressure ∝ P_ref / r  (simplified Sachs scaling).
        let r_blast = (p_ref / BLAST_DAMAGE_THRESHOLD_KPA).max(1.0).min(20.0);
        (p_ref, r_blast)
    } else {
        (0.0, 0.0)
    };

    // ── HE fragmentation ───────────────────────────────────────────────────

    let (he_fragments, he_fragment_spray_angle_deg) = if explosive_detonation {
        // Estimate casing mass: total minus filler minus penetrator core.
        let core_frac = match params.combined_type {
            CombinedType::SAP_HEI => 0.30, // thinner semi-AP core
            _ => 0.0,                      // HEI — no penetrating core
        };
        let casing_mass = (m_tot - m_fill - m_tot * core_frac).max(1.0);

        // Fragment count: N = K · m_casing^0.8
        let n = (FRAGMENTS_PER_G_CASING * casing_mass.powf(0.8))
            .round()
            .max(1.0)
            .min(HEI_FRAGMENTS_MAX as f64) as i32;

        // Spray angle widens with charge-to-casing ratio.
        let chg_ratio = m_fill / casing_mass.max(1e-6);
        let spray = (HEI_FRAGMENT_SPRAY_BASE_DEG * (1.0 + 2.0 * chg_ratio))
            .clamp(HEI_FRAGMENT_SPRAY_MIN_DEG, HEI_FRAGMENT_SPRAY_MAX_DEG);

        (n, spray)
    } else {
        (0, 0.0)
    };

    // ── Secondary ignition chance ──────────────────────────────────────────

    // Hot fragments (HEI) and open flame (incendiary) contribute to
    // the probability of igniting nearby fuel / flammable materials.
    let secondary_ignition_chance = if explosive_detonation || incendiary_ignition {
        let mut prob = 0.0;

        if explosive_detonation {
            // Blast thermal contribution: saturated at 300 kPa.
            let blast_prob = (blast_overpressure_kpa / 300.0).min(1.0) * 0.45;
            // Fragment hot-mass contribution: more fragments → more ignition sources.
            let frag_prob = (he_fragments as f64 / 50.0).min(1.0) * 0.20;
            prob += blast_prob + frag_prob;
        }

        if incendiary_ignition {
            // Open flame directly ignites nearby materials.
            let flame_prob = (fire_radius_m / 3.0).min(1.0) * 0.30;
            prob += flame_prob;
        }

        prob.min(0.95)
    } else {
        0.0
    };

    CombinedEffectResult {
        incendiary_ignition,
        fire_radius_m,
        burn_duration_s,
        explosive_detonation,
        blast_overpressure_kpa,
        blast_radius_m,
        he_fragments,
        he_fragment_spray_angle_deg,
        secondary_ignition_chance,
    }
}

// ── Calibration helpers ─────────────────────────────────────────────────────────

/// Parameters for the M8 API cartridge (.50 BMG / 12.7×99 mm).
///
///   Total mass:   42.0 g
///   Filler mass: ~18.0 g  (zirconium / thermite incendiary)
///   Calibre:      12.7 mm
///   Typical MV:   890 m/s
pub const M8_API_PARAMS: CombinedAmmoParams = CombinedAmmoParams {
    combined_type: CombinedType::API,
    projectile_mass_g: 42.0,
    caliber_mm: 12.7,
    filler_mass_g: 18.0,
    impact_velocity_ms: 890.0,
    armor_penetrated: false,
    residual_velocity_ms: 0.0,
    impact_angle_deg: 0.0,
};

/// Parameters for the B32 API cartridge (7.62×54R).
///
///   Total mass:   10.0 g
///   Filler mass: ~1.4 g   (phosphorus incendiary)
///   Calibre:       7.62 mm
///   Typical MV:   830 m/s
pub const B32_API_PARAMS: CombinedAmmoParams = CombinedAmmoParams {
    combined_type: CombinedType::API,
    projectile_mass_g: 10.0,
    caliber_mm: 7.62,
    filler_mass_g: 1.4,
    impact_velocity_ms: 830.0,
    armor_penetrated: false,
    residual_velocity_ms: 0.0,
    impact_angle_deg: 0.0,
};

// ── Tests ───────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Incendiary (API) tests ─────────────────────────────────────────────

    #[test]
    fn api_ignition_high_velocity_impact() {
        // .50 BMG M8 API at 890 m/s → should ignite
        let params = CombinedAmmoParams {
            combined_type: CombinedType::API,
            projectile_mass_g: 42.0,
            caliber_mm: 12.7,
            filler_mass_g: 18.0,
            impact_velocity_ms: 890.0,
            armor_penetrated: false,
            residual_velocity_ms: 0.0,
            impact_angle_deg: 0.0,
        };
        let r = evaluate_combined_effects(&params);
        assert!(r.incendiary_ignition, "M8 should ignite at 890 m/s");
        assert!(r.fire_radius_m > 0.3, "fire radius should be > 0.3 m");
        assert!(r.burn_duration_s > 10.0, "burn duration should be > 10 s");
        assert!(!r.explosive_detonation, "API has no explosive filler");
    }

    #[test]
    fn api_no_ignition_below_threshold() {
        // Low-velocity impact should not ignite the filler.
        let params = CombinedAmmoParams {
            combined_type: CombinedType::API,
            projectile_mass_g: 42.0,
            caliber_mm: 12.7,
            filler_mass_g: 18.0,
            impact_velocity_ms: 50.0,
            armor_penetrated: false,
            residual_velocity_ms: 0.0,
            impact_angle_deg: 0.0,
        };
        let r = evaluate_combined_effects(&params);
        assert!(!r.incendiary_ignition, "no ignition at 50 m/s");
        assert_eq!(r.fire_radius_m, 0.0);
        assert_eq!(r.burn_duration_s, 0.0);
    }

    #[test]
    fn api_no_ignition_without_filler() {
        // No filler → no incendiary effect.
        let params = CombinedAmmoParams {
            combined_type: CombinedType::API,
            projectile_mass_g: 10.0,
            caliber_mm: 7.62,
            filler_mass_g: 0.0,
            impact_velocity_ms: 850.0,
            armor_penetrated: false,
            residual_velocity_ms: 0.0,
            impact_angle_deg: 0.0,
        };
        let r = evaluate_combined_effects(&params);
        assert!(!r.incendiary_ignition, "no filler → no ignition");
        assert_eq!(r.secondary_ignition_chance, 0.0);
    }

    #[test]
    fn api_ignition_after_armor_penetration() {
        // After perforating armour, residual velocity remaining should
        // still ignite the filler.
        let params = CombinedAmmoParams {
            combined_type: CombinedType::API,
            projectile_mass_g: 42.0,
            caliber_mm: 12.7,
            filler_mass_g: 18.0,
            impact_velocity_ms: 890.0,
            armor_penetrated: true,
            residual_velocity_ms: 400.0,
            impact_angle_deg: 0.0,
        };
        let r = evaluate_combined_effects(&params);
        assert!(
            r.incendiary_ignition,
            "post-penetration ignition at 400 m/s residual"
        );
        assert!(r.fire_radius_m > 0.0);
    }

    #[test]
    fn api_no_ignition_low_residual_after_pen() {
        // Residual velocity so low that the filler does not ignite.
        let params = CombinedAmmoParams {
            combined_type: CombinedType::API,
            projectile_mass_g: 42.0,
            caliber_mm: 12.7,
            filler_mass_g: 18.0,
            impact_velocity_ms: 890.0,
            armor_penetrated: true,
            residual_velocity_ms: 30.0,
            impact_angle_deg: 0.0,
        };
        let r = evaluate_combined_effects(&params);
        assert!(!r.incendiary_ignition, "too slow after penetration");
    }

    #[test]
    fn api_grazing_angle_suppresses_marginal_ignition() {
        // A marginal round (low filler mass) should ignite at normal
        // but fail to ignite at an extreme grazing angle where the
        // angle factor (cos θ) reduces compressive heating.
        let normal = CombinedAmmoParams {
            combined_type: CombinedType::API,
            projectile_mass_g: 5.0,
            caliber_mm: 5.56,
            filler_mass_g: 0.25,
            impact_velocity_ms: 800.0,
            armor_penetrated: false,
            residual_velocity_ms: 0.0,
            impact_angle_deg: 0.0,
        };
        let grazing = CombinedAmmoParams {
            impact_angle_deg: 85.0,
            ..normal
        };
        let r_n = evaluate_combined_effects(&normal);
        let r_g = evaluate_combined_effects(&grazing);
        assert!(
            r_n.incendiary_ignition,
            "normal impact must ignite (fury > threshold)"
        );
        assert!(
            !r_g.incendiary_ignition,
            "grazing at 85° should fail ignition (fury < threshold)"
        );
        assert_eq!(r_g.fire_radius_m, 0.0);
    }

    // ── M8 / B32 calibration point tests ───────────────────────────────────

    #[test]
    fn m8_api_calibration_soft_target() {
        // M8 API against a soft target: should ignite with
        // a substantial fire radius (~1.5–2.5 m for 18 g filler).
        let r = evaluate_combined_effects(&M8_API_PARAMS);
        assert!(r.incendiary_ignition, "M8 API must ignite on impact");
        assert!(
            r.fire_radius_m >= 1.0 && r.fire_radius_m <= 5.0,
            "M8 fire radius {:.2} m out of expected range [1.0, 5.0]",
            r.fire_radius_m
        );
        assert!(
            r.burn_duration_s >= 30.0,
            "M8 burn duration {:.0} s too short",
            r.burn_duration_s
        );
        assert!(!r.explosive_detonation);
        assert_eq!(r.he_fragments, 0);
    }

    #[test]
    fn b32_api_calibration_soft_target() {
        // B32 API (7.62×54R) has ~1.4 g phosphorus filler.
        // Should ignite but with a smaller fire radius than M8.
        let r = evaluate_combined_effects(&B32_API_PARAMS);
        assert!(r.incendiary_ignition, "B32 API must ignite on impact");
        assert!(
            r.fire_radius_m >= 0.3 && r.fire_radius_m <= 2.0,
            "B32 fire radius {:.2} m — expected 0.3–2.0 m",
            r.fire_radius_m
        );
        assert!(
            r.burn_duration_s >= 5.0,
            "B32 burn duration {:.0} s too short",
            r.burn_duration_s
        );
    }

    #[test]
    fn m8_vs_b32_fire_radius_scales_with_filler() {
        // M8 (18 g filler) should have a larger fire radius than
        // B32 (1.4 g filler).
        let m8 = evaluate_combined_effects(&M8_API_PARAMS);
        let b32 = evaluate_combined_effects(&B32_API_PARAMS);
        assert!(
            m8.fire_radius_m > b32.fire_radius_m * 1.5,
            "M8 fire radius ({:.2} m) should be > 1.5× B32 ({:.2} m)",
            m8.fire_radius_m,
            b32.fire_radius_m
        );
    }

    // ── HEI tests ──────────────────────────────────────────────────────────

    #[test]
    fn hei_detonation_on_impact() {
        // Typical 20 mm HEI impact.
        let params = CombinedAmmoParams {
            combined_type: CombinedType::HEI,
            projectile_mass_g: 102.0,
            caliber_mm: 20.0,
            filler_mass_g: 11.0,
            impact_velocity_ms: 800.0,
            armor_penetrated: false,
            residual_velocity_ms: 0.0,
            impact_angle_deg: 0.0,
        };
        let r = evaluate_combined_effects(&params);
        assert!(r.explosive_detonation, "HEI should detonate on impact");
        assert!(r.blast_overpressure_kpa > 100.0, "blast > 100 kPa at 1 m");
        assert!(r.blast_radius_m > 2.0, "blast radius > 2 m");
        assert!(r.he_fragments > 10, "HEI should produce casing fragments");
        assert!(r.he_fragment_spray_angle_deg >= 15.0, "spray angle >= 15°");
        assert!(r.incendiary_ignition, "HEI incendium should also ignite");
    }

    #[test]
    fn hei_impact_fuze_does_not_arm_at_low_velocity() {
        // Low-velocity impact should be a dud.
        let params = CombinedAmmoParams {
            combined_type: CombinedType::HEI,
            projectile_mass_g: 102.0,
            caliber_mm: 20.0,
            filler_mass_g: 11.0,
            impact_velocity_ms: 50.0,
            armor_penetrated: false,
            residual_velocity_ms: 0.0,
            impact_angle_deg: 0.0,
        };
        let r = evaluate_combined_effects(&params);
        assert!(!r.explosive_detonation, "low-velocity dud");
        assert!(!r.incendiary_ignition, "no incendiary at low velocity");
    }

    #[test]
    fn hei_blast_scales_with_charge_mass() {
        // More charge → higher overpressure.
        let small = CombinedAmmoParams {
            combined_type: CombinedType::HEI,
            projectile_mass_g: 50.0,
            caliber_mm: 20.0,
            filler_mass_g: 2.0,
            impact_velocity_ms: 800.0,
            armor_penetrated: false,
            residual_velocity_ms: 0.0,
            impact_angle_deg: 0.0,
        };
        let large = CombinedAmmoParams {
            combined_type: CombinedType::HEI,
            projectile_mass_g: 102.0,
            caliber_mm: 20.0,
            filler_mass_g: 11.0,
            impact_velocity_ms: 800.0,
            armor_penetrated: false,
            residual_velocity_ms: 0.0,
            impact_angle_deg: 0.0,
        };
        let r_s = evaluate_combined_effects(&small);
        let r_l = evaluate_combined_effects(&large);
        assert!(
            r_l.blast_overpressure_kpa > r_s.blast_overpressure_kpa,
            "more charge → higher overpressure"
        );
        assert!(
            r_l.he_fragments >= r_s.he_fragments,
            "more charge → at least as many fragments"
        );
    }

    // ── SAP-HEI delay fuze tests ───────────────────────────────────────────

    #[test]
    fn sap_hei_detonates_after_penetration() {
        // SAP-HEI: delay fuze — detonates after armour perforation.
        let params = CombinedAmmoParams {
            combined_type: CombinedType::SAP_HEI,
            projectile_mass_g: 50.0,
            caliber_mm: 14.5,
            filler_mass_g: 4.0,
            impact_velocity_ms: 900.0,
            armor_penetrated: true,
            residual_velocity_ms: 300.0,
            impact_angle_deg: 0.0,
        };
        let r = evaluate_combined_effects(&params);
        assert!(r.explosive_detonation, "SAP-HEI should detonate after pen");
        assert!(r.blast_overpressure_kpa > 50.0);
        assert!(r.he_fragments > 5);
        // Incendiary may or may not ignite post-penetration depending on KE.
    }

    #[test]
    fn sap_hei_detonates_at_high_impact_velocity() {
        // SAP-HEI with delay fuze: even without penetration, high-
        // velocity impact triggers self-destruct detonation.
        let params = CombinedAmmoParams {
            combined_type: CombinedType::SAP_HEI,
            projectile_mass_g: 50.0,
            caliber_mm: 14.5,
            filler_mass_g: 4.0,
            impact_velocity_ms: 900.0,
            armor_penetrated: false,
            residual_velocity_ms: 0.0,
            impact_angle_deg: 0.0,
        };
        let r = evaluate_combined_effects(&params);
        assert!(
            r.explosive_detonation,
            "SAP-HEI should detonate on high-velocity impact (self-destruct)"
        );
    }

    // ── Tracer variant tests ───────────────────────────────────────────────

    #[test]
    fn api_tracer_extends_burn_duration() {
        // Tracer variant should have a longer burn duration than plain API.
        let plain = CombinedAmmoParams {
            combined_type: CombinedType::API,
            projectile_mass_g: 10.0,
            caliber_mm: 7.62,
            filler_mass_g: 1.4,
            impact_velocity_ms: 830.0,
            armor_penetrated: false,
            residual_velocity_ms: 0.0,
            impact_angle_deg: 0.0,
        };
        let tracer = CombinedAmmoParams {
            combined_type: CombinedType::API_Tracer,
            ..plain
        };
        let r_p = evaluate_combined_effects(&plain);
        let r_t = evaluate_combined_effects(&tracer);
        assert!(r_p.incendiary_ignition);
        assert!(r_t.incendiary_ignition);
        assert!(
            r_t.burn_duration_s > r_p.burn_duration_s,
            "tracer burn ({:.1} s) should exceed plain ({:.1} s)",
            r_t.burn_duration_s,
            r_p.burn_duration_s
        );
    }

    // ── Secondary ignition tests ───────────────────────────────────────────

    #[test]
    fn secondary_ignition_chance_non_zero_with_explosion() {
        // A strong HEI detonation should produce a secondary ignition risk.
        let params = CombinedAmmoParams {
            combined_type: CombinedType::HEI,
            projectile_mass_g: 102.0,
            caliber_mm: 20.0,
            filler_mass_g: 11.0,
            impact_velocity_ms: 800.0,
            armor_penetrated: false,
            residual_velocity_ms: 0.0,
            impact_angle_deg: 0.0,
        };
        let r = evaluate_combined_effects(&params);
        assert!(
            r.secondary_ignition_chance > 0.1,
            "secondary ignition chance should be > 0.1 with 11 g HEI"
        );
        assert!(r.secondary_ignition_chance <= 0.95, "capped at 0.95");
    }

    #[test]
    fn secondary_ignition_chance_zero_with_no_effect() {
        // No filler → no secondary ignition.
        let params = CombinedAmmoParams {
            combined_type: CombinedType::API,
            projectile_mass_g: 10.0,
            caliber_mm: 7.62,
            filler_mass_g: 0.0,
            impact_velocity_ms: 50.0,
            armor_penetrated: false,
            residual_velocity_ms: 0.0,
            impact_angle_deg: 0.0,
        };
        let r = evaluate_combined_effects(&params);
        assert_eq!(r.secondary_ignition_chance, 0.0);
    }

    // ── Determinism test ───────────────────────────────────────────────────

    #[test]
    fn evaluate_is_deterministic() {
        // Same input → same output every time.
        let params = CombinedAmmoParams {
            combined_type: CombinedType::API,
            projectile_mass_g: 42.0,
            caliber_mm: 12.7,
            filler_mass_g: 18.0,
            impact_velocity_ms: 890.0,
            armor_penetrated: true,
            residual_velocity_ms: 350.0,
            impact_angle_deg: 15.0,
        };
        let a = evaluate_combined_effects(&params);
        let b = evaluate_combined_effects(&params);
        assert_eq!(a.incendiary_ignition, b.incendiary_ignition);
        assert_eq!(a.fire_radius_m, b.fire_radius_m);
        assert_eq!(a.burn_duration_s, b.burn_duration_s);
        assert_eq!(a.explosive_detonation, b.explosive_detonation);
        assert_eq!(a.blast_overpressure_kpa, b.blast_overpressure_kpa);
        assert_eq!(a.blast_radius_m, b.blast_radius_m);
        assert_eq!(a.he_fragments, b.he_fragments);
        assert_eq!(a.he_fragment_spray_angle_deg, b.he_fragment_spray_angle_deg);
        assert_eq!(a.secondary_ignition_chance, b.secondary_ignition_chance);
    }

    // ── Edge case tests ────────────────────────────────────────────────────

    #[test]
    fn zero_velocity_returns_default() {
        let params = CombinedAmmoParams {
            combined_type: CombinedType::HEI,
            projectile_mass_g: 100.0,
            caliber_mm: 20.0,
            filler_mass_g: 10.0,
            impact_velocity_ms: 0.0,
            armor_penetrated: false,
            residual_velocity_ms: 0.0,
            impact_angle_deg: 0.0,
        };
        let r = evaluate_combined_effects(&params);
        assert_eq!(r, CombinedEffectResult::default());
    }

    #[test]
    fn tiny_filler_does_not_detonate() {
        // Sub-gram filler lacks enough HE to count as a detonation.
        let params = CombinedAmmoParams {
            combined_type: CombinedType::HEI,
            projectile_mass_g: 5.0,
            caliber_mm: 5.56,
            filler_mass_g: 0.1,
            impact_velocity_ms: 900.0,
            armor_penetrated: false,
            residual_velocity_ms: 0.0,
            impact_angle_deg: 0.0,
        };
        let r = evaluate_combined_effects(&params);
        assert!(!r.explosive_detonation, "< 0.3 g filler → no detonation");
    }
}
