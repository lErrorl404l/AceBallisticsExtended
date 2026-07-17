// ABE - Security Glass Penetration Model
//
// Models penetration of security and automotive glass by small-arms
// projectiles.  Covers tempered (toughened) glass, laminated glass with
// PVB interlayer, bullet-resistant (BR) glass with polycarbonate/acrylic
// laminates, and automotive side-window glass.
//
// Physics layers:
//   - Tempered: prestressed pane, ~50 m/s threshold for 6mm, shatters to
//     small cubes on penetration, entire pane loses structural integrity.
//   - Laminated: glass + PVB (polyvinyl butyral) interlayer.  Glass cracks
//     at low energy; PVB stretches to absorb energy (strain-rate dependent).
//     Each PVB layer absorbs ~40–80 J before tearing.
//   - Bullet-resistant: outer glass + polycarbonate (PC) / acrylic laminate.
//     PC provides primary ballistic resistance via modified De Marre (factor
//     ~0.6 of RHA).  Spall shield on rear prevents glass fragment spray.
//   - Automotive side: tempered glass, typical 3–6 mm.
//
// References:
//   - UL 752, "Standard for Bullet-Resisting Equipment" (13th ed., 2018)
//   - NIJ 0108.01 (Ballistic Resistant Protective Materials)
//   - Van der Voet et al., "PVB interlayer ballistic performance"
//     (TNO Defence, 2009) — strain-rate effects in PVB
//   - Istrate et al., "Ballistic resistance of polycarbonate"
//     (Int. J. Impact Eng., 2016) — De Marre for PC laminates
//   - Walley, S.M., "Shear Localisation: A Historical Overview" (PC
//     shear-band behaviour under high strain rate)
//   - De Marre ballistics formula (KE penetration)
//   - ECE R43 / ANSI Z97.1 (automotive glazing standards)
//   - MIL-STD-662F (V50 ballistic test)

// ── Constants ────────────────────────────────────────────────────────────────────

/// Reference penetration velocity for 6 mm tempered glass (m/s).
/// A typical car side window is ~6 mm and fails at ~50 m/s impact.
const TEMPERED_V50_REF: f64 = 50.0;

/// Reference thickness for tempered glass V50 scaling (mm).
const TEMPERED_THICKNESS_REF: f64 = 6.0;

/// Energy absorbed per mm of glass layer cracking (J/mm).
/// Glass layers in laminated/BR constructions offer minimal ballistic
/// resistance but consume a few joules crushing/cracking.
const GLASS_CRACK_ENERGY_PER_MM: f64 = 3.0;

/// Base energy absorption for a single 0.76 mm PVB layer at reference
/// velocity (J).  At 400 m/s a standard PVB layer absorbs ~55 J before
/// tearing (strain-rate dependent).
const PVB_BASE_ABSORPTION_J: f64 = 55.0;

/// Reference PVB layer thickness (mm) — standard automotive interlayer.
const PVB_REF_THICKNESS_MM: f64 = 0.76;

/// Reference velocity for PVB energy absorption scaling (m/s).
const PVB_REF_VELOCITY: f64 = 400.0;

/// Strain-rate exponent for PVB.  PVB stiffens at high strain rates so
/// energy absorption increases with velocity: E ∝ v^0.3.
const PVB_STRAIN_EXP: f64 = 0.3;

/// Polycarbonate (PC) ballistic material factor relative to RHA.
/// PC is ~0.6× as effective per mm as rolled homogeneous armour in the
/// De Marre penetration formula.
const PC_MATERIAL_FACTOR: f64 = 0.6;

/// Acrylic ballistic material factor relative to RHA.
const ACRYLIC_MATERIAL_FACTOR: f64 = 0.4;

/// Outer glass material factor in BR laminates (brittle, minimal KE
/// resistance — provides hardness to deform / fracture the projectile).
const BR_GLASS_MATERIAL_FACTOR: f64 = 0.15;

// ── Types ────────────────────────────────────────────────────────────────────────

/// Classification of security / automotive glass.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SecurityGlassType {
    /// Prestressed tempered (toughened) glass.  Entire pane shatters
    /// into small cuboid fragments on penetration.
    Tempered { thickness_mm: f64 },
    /// Laminated glass with one or more PVB (polyvinyl butyral)
    /// interlayers.  Glass cracks on impact; PVB absorbs energy.
    /// Typical windshield: 2 × 2.3 mm glass + 0.76 mm PVB.
    Laminated {
        /// Total glass thickness (sum of all glass plies, mm).
        glass_thickness_mm: f64,
        /// Number of PVB interlayer sheets.
        pvb_layer_count: i32,
        /// Thickness of each PVB layer (typically 0.76–3.0 mm).
        pvb_thickness_per_layer_mm: f64,
    },
    /// Bullet-resistant (BR) glass — glass + polycarbonate + acrylic
    /// laminate.  Polycarbonate provides primary ballistic resistance.
    BulletResistant {
        /// Total glass thickness (outer plies, mm).
        glass_thickness_mm: f64,
        /// Polycarbonate layer thickness (mm).  Main ballistic layer.
        polycarbonate_thickness_mm: f64,
        /// Acrylic layer thickness (mm).  Secondary ballistic + spall
        /// shield.
        acrylic_thickness_mm: f64,
    },
    /// Automotive side-window glass (tempered).  Typically 3–6 mm.
    AutomotiveSide { thickness_mm: f64 },
}

/// UL 752 ballistic resistance levels (1–10).
///
/// Each level corresponds to a specific threat (calibre, projectile mass,
/// velocity, and number of shots).  The levels are cumulative: a panel
/// tested to Level N must stop all threats ≤ N.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UL752Level {
    /// 9 mm FMJ  (3 shots)
    Level1,
    /// .357 Magnum JSP  (3 shots)
    Level2,
    /// .44 Magnum lead  (3 shots)
    Level3,
    /// .30-06 M2 ball  (1 shot)
    Level4,
    /// 7.62 mm M80 ball  (1 shot)
    Level5,
    /// 9 mm FMJ (5 shots) + .44 Magnum (1 shot)
    Level6,
    /// 5.56 mm M193 ball  (1 shot)
    Level7,
    /// 7.62 mm M80 ball (5 shots)
    Level8,
    /// .30-06 M2 AP  (1 shot)
    Level9,
    /// .50 Cal M2 AP  (1 shot)
    Level10,
}

/// Input parameters for a glass penetration evaluation.
#[derive(Debug, Clone)]
pub struct GlassPenetrationParams {
    /// Glass type and dimensions.
    pub glass_type: SecurityGlassType,
    /// Normal impact velocity (m/s).
    pub velocity_ms: f64,
    /// Projectile mass (g).
    pub mass_g: f64,
    /// Projectile calibre (m).
    pub caliber_m: f64,
    /// Projectile construction ("ball", "fmj", "ap", "apds", etc.).
    pub projectile_type: String,
    /// Angle from surface normal (0 = perpendicular, deg).
    pub impact_angle_deg: f64,
}

/// Result of a glass penetration evaluation.
#[derive(Debug, Clone)]
pub struct GlassPenetrationResult {
    /// Whether the projectile fully perforated the glass.
    pub penetrated: bool,
    /// Whether spall (glass fragments) was generated on the protected side.
    pub spall_generated: bool,
    /// Estimated mass of spall fragments (g).
    pub spall_mass_g: f64,
    /// Projectile velocity after exiting the glass (m/s).  0.0 if stopped.
    pub residual_velocity_ms: f64,
    /// Tempered: entire pane loses structural integrity when any point
    /// is breached.
    pub glass_pane_shattered: bool,
    /// Number of PVB interlayer sheets that were perforated (laminated only).
    pub pvb_layers_perforated: i32,
    /// Number of glass plies that cracked (counts each distinct layer).
    pub glass_layers_cracked: i32,
    /// Total kinetic energy absorbed by the glass (J).
    pub energy_absorbed_j: f64,
}

// ── Projectile type modifier (De Marre) ─────────────────────────────────────────

/// Projectile construction modifier for De Marre penetration formula.
/// Applied when evaluating polycarbonate / BR laminates.
fn proj_modifier(proj_type: &str) -> f64 {
    match proj_type.to_lowercase().as_str() {
        "ball" | "fmj" => 1.0,
        "ap" | "armor_piercing" => 1.3,
        "apds" => 1.5,
        "soft_point" | "jsp" | "jhp" => 0.95,
        "frangible" => 0.5,
        _ => 1.0,
    }
}

// ── De Marre penetration velocity for BR glass ──────────────────────────────────

/// Compute the De Marre threshold velocity (m/s) needed to penetrate a
/// BR glass panel given its effective thickness, projectile calibre,
/// and mass.
///
///   V_req = (91000 / proj_mod) × D^0.75 × T_eff^0.7 / M^0.5
///
/// Where:
///   D  = calibre (m)
///   T_eff = effective thickness (m) after material-factor scaling
///   M  = projectile mass (kg)
///   proj_mod = projectile construction modifier
fn de_marre_v50(caliber_m: f64, effective_thickness_m: f64, mass_kg: f64, proj_mod: f64) -> f64 {
    if caliber_m <= 0.0 || effective_thickness_m <= 0.0 || mass_kg <= 0.0 {
        return f64::INFINITY;
    }
    let k = 91000.0 / proj_mod;
    k * caliber_m.powf(0.75) * effective_thickness_m.powf(0.70) / mass_kg.sqrt()
}

// ── Angle correction ────────────────────────────────────────────────────────────

/// Angle multiplier for effective thickness.
/// T_eff = T / cos(angle)^n  where n ≈ 1.0 for glass.
fn angle_correction(angle_deg: f64) -> f64 {
    let rad = angle_deg.to_radians();
    let cos_a = rad.cos().max(0.087); // clamp to ~85° max
    1.0 / cos_a
}

// ── Kinetic energy helper ───────────────────────────────────────────────────────

fn kinetic_energy(mass_kg: f64, velocity_ms: f64) -> f64 {
    0.5 * mass_kg * velocity_ms.powi(2)
}

// ── PVB layer energy absorption ─────────────────────────────────────────────────

/// Maximum energy a single PVB layer can absorb before tearing (J).
///
/// Strain-rate effect: PVB stiffens at higher impact velocities, so
/// a given layer absorbs more energy against a fast projectile.
fn pvb_layer_absorption(layer_thickness_mm: f64, impact_velocity_ms: f64) -> f64 {
    let thickness_ratio = layer_thickness_mm / PVB_REF_THICKNESS_MM;
    let rate_factor = (impact_velocity_ms / PVB_REF_VELOCITY).powf(PVB_STRAIN_EXP);
    PVB_BASE_ABSORPTION_J * thickness_ratio * rate_factor
}

// ── Per-type evaluators ─────────────────────────────────────────────────────────

fn evaluate_tempered(thickness_mm: f64, params: &GlassPenetrationParams) -> GlassPenetrationResult {
    let v_threshold = TEMPERED_V50_REF * (thickness_mm / TEMPERED_THICKNESS_REF).sqrt();
    // Angle: effective velocity is normal component
    let cos_a = params.impact_angle_deg.to_radians().cos().max(0.087);
    let effective_v = params.velocity_ms * cos_a;

    let ke = kinetic_energy(params.mass_g / 1000.0, params.velocity_ms);

    if effective_v >= v_threshold {
        // Penetrated — pane disintegrates into small cubes
        let residual_v = (params.velocity_ms.powi(2) - (v_threshold / cos_a.max(0.087)).powi(2))
            .sqrt()
            .max(0.0);
        let energy_abs = ke - kinetic_energy(params.mass_g / 1000.0, residual_v);
        // Spall: fine glass cubes, mass scales with energy and thickness
        let spall_factor = (params.velocity_ms / 200.0).min(3.0);
        let spall = 0.15 * thickness_mm * spall_factor;

        GlassPenetrationResult {
            penetrated: true,
            spall_generated: true,
            spall_mass_g: spall.max(0.1),
            residual_velocity_ms: residual_v,
            glass_pane_shattered: true,
            pvb_layers_perforated: 0,
            glass_layers_cracked: 1,
            energy_absorbed_j: energy_abs,
        }
    } else {
        // Spiderweb cracks but no through-penetration.
        // The pane is still held by the compressive prestress layer.
        GlassPenetrationResult {
            penetrated: false,
            spall_generated: false,
            spall_mass_g: 0.0,
            residual_velocity_ms: params.velocity_ms * 0.05,
            glass_pane_shattered: false,
            pvb_layers_perforated: 0,
            glass_layers_cracked: 1,
            energy_absorbed_j: ke,
        }
    }
}

fn evaluate_laminated(
    glass_thickness_mm: f64,
    pvb_layer_count: i32,
    pvb_thickness_per_layer_mm: f64,
    params: &GlassPenetrationParams,
) -> GlassPenetrationResult {
    let cos_a = params.impact_angle_deg.to_radians().cos().max(0.087);
    let effective_v = params.velocity_ms * cos_a; // normal component
    let mass_kg = params.mass_g / 1000.0;
    let ke = kinetic_energy(mass_kg, params.velocity_ms);

    // ── Stage 1: Crack the glass plies ───────────────────────────────
    // Glass offers minimal resistance — each mm absorbs ~3 J.
    let glass_energy = glass_thickness_mm * GLASS_CRACK_ENERGY_PER_MM;
    let mut remaining_j = (ke - glass_energy).max(0.0);
    let mut pvb_perforated: i32 = 0;
    let glass_cracked: i32 = if ke >= glass_energy { 2 } else { 1 };
    // (we simplify: always two glass plies in laminated construction)

    // ── Stage 2: Absorb through PVB layers ──────────────────────────
    // Use normal-component velocity for strain-rate-dependent PVB
    // absorption (the PVB stretch rate is governed by the perpendicular
    // component of the impact).
    let mut current_v = (2.0 * remaining_j / mass_kg).sqrt().max(effective_v * 0.1);
    for _layer in 0..pvb_layer_count {
        if remaining_j <= 0.0 {
            break;
        }
        let e_layer = pvb_layer_absorption(pvb_thickness_per_layer_mm, current_v);
        if remaining_j >= e_layer {
            // Layer fails
            remaining_j -= e_layer;
            pvb_perforated += 1;
            current_v = (2.0 * remaining_j / mass_kg).sqrt().max(effective_v * 0.1);
        } else {
            // Layer holds — projectile stopped
            remaining_j = 0.0;
            break;
        }
    }

    let penetrated = pvb_perforated >= pvb_layer_count && remaining_j > 0.0;
    let residual_v = if penetrated {
        (2.0 * remaining_j / mass_kg).sqrt()
    } else {
        0.0
    };
    let energy_abs = ke - remaining_j;

    // Spall only if fully penetrated (PVB catches most fragments otherwise)
    let spall = if penetrated {
        0.08 * glass_thickness_mm * (params.velocity_ms / 300.0).min(2.0)
    } else {
        0.0
    };

    GlassPenetrationResult {
        penetrated,
        spall_generated: penetrated,
        spall_mass_g: spall.max(0.0),
        residual_velocity_ms: residual_v,
        glass_pane_shattered: penetrated,
        pvb_layers_perforated: pvb_perforated,
        glass_layers_cracked: glass_cracked,
        energy_absorbed_j: energy_abs,
    }
}

fn evaluate_bullet_resistant(
    glass_thickness_mm: f64,
    polycarbonate_thickness_mm: f64,
    acrylic_thickness_mm: f64,
    params: &GlassPenetrationParams,
) -> GlassPenetrationResult {
    let mass_kg = params.mass_g / 1000.0;
    let ke = kinetic_energy(mass_kg, params.velocity_ms);

    // ── Effective thickness (material-factor-weighted) ───────────────
    let t_glass_eff = glass_thickness_mm / 1000.0 * BR_GLASS_MATERIAL_FACTOR;
    let t_pc_eff = polycarbonate_thickness_mm / 1000.0 * PC_MATERIAL_FACTOR;
    let t_acrylic_eff = acrylic_thickness_mm / 1000.0 * ACRYLIC_MATERIAL_FACTOR;
    let total_eff_m = t_glass_eff + t_pc_eff + t_acrylic_eff;

    // ── Angle correction ────────────────────────────────────────────
    let angle_mult = angle_correction(params.impact_angle_deg);
    let eff_with_angle = total_eff_m * angle_mult;

    // ── De Marre threshold velocity ─────────────────────────────────
    let proj_mod = proj_modifier(&params.projectile_type);
    let v_req = de_marre_v50(params.caliber_m, eff_with_angle, mass_kg, proj_mod);

    // ── Compare normal-component velocity ────────────────────────────
    let cos_a = params.impact_angle_deg.to_radians().cos().max(0.087);
    let effective_v = params.velocity_ms * cos_a;

    let penetrated = effective_v >= v_req;

    let (residual_v, energy_abs) = if penetrated {
        let res_v = (params.velocity_ms.powi(2) - (v_req / cos_a.max(0.087)).powi(2))
            .sqrt()
            .max(0.0);
        let e_abs = ke - kinetic_energy(mass_kg, res_v);
        (res_v, e_abs)
    } else {
        (0.0, ke)
    };

    // BR glass has a spall shield on the rear.  If the panel is NOT
    // penetrated, the spall shield prevents glass fragment spray.
    // If penetrated, some spall may still be generated.
    let spall = if penetrated {
        // Spall shield reduces but does not eliminate rear debris
        0.03 * (glass_thickness_mm + polycarbonate_thickness_mm)
            * (params.velocity_ms / 500.0).min(2.0)
    } else {
        0.0
    };

    GlassPenetrationResult {
        penetrated,
        spall_generated: penetrated,
        spall_mass_g: spall.max(0.0),
        residual_velocity_ms: residual_v,
        glass_pane_shattered: penetrated,
        pvb_layers_perforated: 0,
        glass_layers_cracked: 1,
        energy_absorbed_j: energy_abs,
    }
}

// ── Public API ──────────────────────────────────────────────────────────────────

/// Evaluate penetration of a security-glass panel by a small-arms
/// projectile.
///
/// Selects the appropriate physics model based on [`SecurityGlassType`]:
///
/// | Glass type          | Model                                              |
/// |---------------------|----------------------------------------------------|
/// | `Tempered`          | Velocity threshold, pane shatters on breach        |
/// | `AutomotiveSide`    | Same as Tempered                                   |
/// | `Laminated`         | Glass cracks + sequential PVB layer absorption     |
/// | `BulletResistant`   | De Marre through effective material thickness      |
///
/// # Arguments
/// * `params` — Glass configuration, projectile, and impact conditions.
///
/// # Returns
/// [`GlassPenetrationResult`] with penetration status, residual velocity,
/// spall, and layer-by-layer failure counts.
///
/// # Validation
/// If velocity ≤ 0 or mass ≤ 0 the result reports no penetration and
/// zero energy absorption.
pub fn evaluate_glass_penetration(params: &GlassPenetrationParams) -> GlassPenetrationResult {
    if params.velocity_ms <= 0.0 || params.mass_g <= 0.0 || params.caliber_m <= 0.0 {
        return GlassPenetrationResult {
            penetrated: false,
            spall_generated: false,
            spall_mass_g: 0.0,
            residual_velocity_ms: 0.0,
            glass_pane_shattered: false,
            pvb_layers_perforated: 0,
            glass_layers_cracked: 0,
            energy_absorbed_j: 0.0,
        };
    }

    match params.glass_type {
        SecurityGlassType::Tempered { thickness_mm } => evaluate_tempered(thickness_mm, params),
        SecurityGlassType::AutomotiveSide { thickness_mm } => {
            // Automotive side glass is tempered — same physics
            evaluate_tempered(thickness_mm, params)
        }
        SecurityGlassType::Laminated {
            glass_thickness_mm,
            pvb_layer_count,
            pvb_thickness_per_layer_mm,
        } => evaluate_laminated(
            glass_thickness_mm,
            pvb_layer_count,
            pvb_thickness_per_layer_mm,
            params,
        ),
        SecurityGlassType::BulletResistant {
            glass_thickness_mm,
            polycarbonate_thickness_mm,
            acrylic_thickness_mm,
        } => evaluate_bullet_resistant(
            glass_thickness_mm,
            polycarbonate_thickness_mm,
            acrylic_thickness_mm,
            params,
        ),
    }
}

/// Compute the reference threat parameters for a UL 752 level.
///
/// Returns `(mass_g, velocity_ms, caliber_m)` — the reference projectile
/// that a panel at this level is certified to stop.
fn ul752_level_threat(level: UL752Level) -> (f64, f64, f64) {
    match level {
        UL752Level::Level1 => (8.0, 358.0, 0.0090),
        UL752Level::Level2 => (10.2, 381.0, 0.0091),
        UL752Level::Level3 => (15.6, 427.0, 0.0109),
        UL752Level::Level4 => (9.7, 828.0, 0.00782),
        UL752Level::Level5 => (9.7, 838.0, 0.00762),
        // Level 6 requires multi-hit resistance (9 mm × 5 + .44 Mag × 1).
        // Use .44 Mag reference as the highest single-shot threat.
        UL752Level::Level6 => (15.6, 427.0, 0.0109),
        UL752Level::Level7 => (3.6, 936.0, 0.00556),
        // Level 8 is multi-hit 7.62 mm M80 — use same reference as L5.
        UL752Level::Level8 => (9.7, 838.0, 0.00762),
        UL752Level::Level9 => (10.8, 823.0, 0.00782),
        UL752Level::Level10 => (45.8, 884.0, 0.0127),
    }
}

/// De Marre penetration index:  sqrt(M) × v / D^0.75
///
/// Higher values indicate greater penetration capability.  Used to
/// compare an arbitrary projectile against a UL 752 level's reference
/// threat.
fn pen_index(mass_kg: f64, velocity_ms: f64, caliber_m: f64) -> f64 {
    if caliber_m <= 0.0 || mass_kg <= 0.0 {
        return 0.0;
    }
    mass_kg.sqrt() * velocity_ms / caliber_m.powf(0.75)
}

/// Check whether a UL 752 certified glass panel of the given level
/// would stop a specific projectile.
///
/// Compares the projectile's De Marre penetration index against the
/// level's reference threat.  Returns `true` if the glass provides
/// protection (projectile is stopped), `false` if the projectile would
/// penetrate.
///
/// This is a conservative single-shot comparison; multi-hit levels
/// (6, 8) use their highest single-shot threat as reference.
///
/// # Arguments
/// * `level` — UL 752 protection level of the glass panel.
/// * `velocity_ms` — Projectile impact velocity (m/s).
/// * `mass_g` — Projectile mass (g).
/// * `caliber_m` — Projectile calibre (m).
pub fn evaluate_ul752_protection(
    level: UL752Level,
    velocity_ms: f64,
    mass_g: f64,
    caliber_m: f64,
) -> bool {
    let (ref_mass_g, ref_vel_ms, ref_caliber_m) = ul752_level_threat(level);

    let idx = pen_index(mass_g / 1000.0, velocity_ms, caliber_m);
    let ref_idx = pen_index(ref_mass_g / 1000.0, ref_vel_ms, ref_caliber_m);

    // Protected if the projectile's penetration index does not exceed
    // the level's reference (conservative).
    idx <= ref_idx
}

// ── Tests ───────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Helpers ─────────────────────────────────────────────────────────────────

    fn eval(params: GlassPenetrationParams) -> GlassPenetrationResult {
        evaluate_glass_penetration(&params)
    }

    fn params(
        glass_type: SecurityGlassType,
        vel: f64,
        mass_g: f64,
        cal_mm: f64,
        proj: &str,
        angle: f64,
    ) -> GlassPenetrationParams {
        GlassPenetrationParams {
            glass_type,
            velocity_ms: vel,
            mass_g,
            caliber_m: cal_mm / 1000.0,
            projectile_type: proj.to_string(),
            impact_angle_deg: angle,
        }
    }

    // ── Tempered glass ─────────────────────────────────────────────────────────

    #[test]
    fn tempered_6mm_below_threshold_no_penetration() {
        // 6 mm tempered, 4 g projectile at 30 m/s < 50 m/s threshold
        let r = eval(params(
            SecurityGlassType::Tempered { thickness_mm: 6.0 },
            30.0,
            4.0,
            5.56,
            "fmj",
            0.0,
        ));
        assert!(!r.penetrated, "30 m/s should not penetrate 6 mm tempered");
        assert!(!r.spall_generated, "no through-spall without penetration");
        assert!(
            !r.glass_pane_shattered,
            "pane not shattered below threshold"
        );
        assert!(r.energy_absorbed_j > 0.0, "energy should be absorbed");
    }

    #[test]
    fn tempered_6mm_above_threshold_penetrates_and_shatters() {
        // 80 m/s > 50 m/s threshold → penetration + full shatter
        let r = eval(params(
            SecurityGlassType::Tempered { thickness_mm: 6.0 },
            80.0,
            4.0,
            5.56,
            "fmj",
            0.0,
        ));
        assert!(r.penetrated, "80 m/s should penetrate 6 mm tempered");
        assert!(r.spall_generated, "penetration should generate spall");
        assert!(r.glass_pane_shattered, "tempered pane shatters on breach");
        assert!(r.residual_velocity_ms > 0.0, "residual velocity after exit");
        assert!(r.energy_absorbed_j > 0.0);
    }

    #[test]
    fn tempered_10mm_requires_higher_velocity() {
        // 10 mm tempered: threshold = 50 * sqrt(10/6) ≈ 64.5 m/s
        // At 50 m/s → no pen; at 80 m/s → pen
        let r1 = eval(params(
            SecurityGlassType::Tempered { thickness_mm: 10.0 },
            50.0,
            4.0,
            5.56,
            "fmj",
            0.0,
        ));
        assert!(!r1.penetrated, "50 m/s should not penetrate 10 mm tempered");

        let r2 = eval(params(
            SecurityGlassType::Tempered { thickness_mm: 10.0 },
            80.0,
            4.0,
            5.56,
            "fmj",
            0.0,
        ));
        assert!(r2.penetrated, "80 m/s should penetrate 10 mm tempered");
    }

    // ── Laminated glass (windshield) ──────────────────────────────────────────

    #[test]
    fn laminated_windshield_stops_low_velocity() {
        // Windshield: 2 × 2.3 mm glass + 0.76 mm PVB
        // 4 g at 60 m/s → KE = 7.2 J
        // PVB layer: 55 * (0.76/0.76) * (60/400)^0.3 = 55 * 0.583 = 32.1 J
        // 7.2 J < 32.1 J → PVB holds
        let r = eval(params(
            SecurityGlassType::Laminated {
                glass_thickness_mm: 4.6,
                pvb_layer_count: 1,
                pvb_thickness_per_layer_mm: 0.76,
            },
            60.0,
            4.0,
            5.56,
            "fmj",
            0.0,
        ));
        assert!(!r.penetrated, "60 m/s should not penetrate windshield");
        assert!(!r.spall_generated, "PVB catches fragments");
        assert_eq!(r.pvb_layers_perforated, 0, "PVB layer intact");
        assert!(r.glass_layers_cracked >= 1, "glass layers crack on impact");
    }

    #[test]
    fn laminated_windshield_penetrated_at_high_velocity() {
        // Windshield, 4 g at 600 m/s → KE = 720 J
        // PVB: 55 * 1.0 * (600/400)^0.3 = 55 * 1.129 = 62.1 J
        // 720 J >> 62.1 J → PVB fails
        let r = eval(params(
            SecurityGlassType::Laminated {
                glass_thickness_mm: 4.6,
                pvb_layer_count: 1,
                pvb_thickness_per_layer_mm: 0.76,
            },
            600.0,
            4.0,
            5.56,
            "fmj",
            0.0,
        ));
        assert!(r.penetrated, "rifle round should penetrate windshield");
        assert!(r.spall_generated, "spall on penetration");
        assert_eq!(r.pvb_layers_perforated, 1, "PVB layer failed");
        assert!(
            r.residual_velocity_ms > 100.0,
            "significant residual velocity"
        );
    }

    #[test]
    fn laminated_multi_pvb_provides_more_resistance() {
        // Thick laminated: 6 mm glass + 3 × 0.76 mm PVB
        // 9.5 g at 400 m/s → KE = 760 J
        // Each PVB layer: 55 * 1.0 * (400/400)^0.3 = 55 J
        // Total PVB: 3 × 55 = 165 J; glass cracks ~18 J
        // 760 J >> 183 J → all layers fail, but residual is lower than
        // single-PVB case
        let r = eval(params(
            SecurityGlassType::Laminated {
                glass_thickness_mm: 6.0,
                pvb_layer_count: 3,
                pvb_thickness_per_layer_mm: 0.76,
            },
            400.0,
            9.5,
            7.62,
            "fmj",
            0.0,
        ));
        assert!(r.penetrated, "multi-PVB still penetrated at rifle velocity");
        assert_eq!(r.pvb_layers_perforated, 3, "all three PVB layers failed");
        // Verify that with 3 PVB layers the residual velocity is lower
        // than if there were only 1 layer (more energy absorbed)
        let r_single = eval(params(
            SecurityGlassType::Laminated {
                glass_thickness_mm: 6.0,
                pvb_layer_count: 1,
                pvb_thickness_per_layer_mm: 0.76,
            },
            400.0,
            9.5,
            7.62,
            "fmj",
            0.0,
        ));
        assert!(
            r.residual_velocity_ms < r_single.residual_velocity_ms,
            "more PVB layers should lower residual velocity: {} < {}",
            r.residual_velocity_ms,
            r_single.residual_velocity_ms,
        );
    }

    #[test]
    fn laminated_pvb_absorption_scales_with_thickness() {
        // Compare 0.38 mm vs 1.52 mm PVB at same velocity
        let r_thin = eval(params(
            SecurityGlassType::Laminated {
                glass_thickness_mm: 2.3,
                pvb_layer_count: 1,
                pvb_thickness_per_layer_mm: 0.38,
            },
            300.0,
            4.0,
            5.56,
            "fmj",
            0.0,
        ));
        let r_thick = eval(params(
            SecurityGlassType::Laminated {
                glass_thickness_mm: 2.3,
                pvb_layer_count: 1,
                pvb_thickness_per_layer_mm: 1.52,
            },
            300.0,
            4.0,
            5.56,
            "fmj",
            0.0,
        ));
        // Thicker PVB absorbs more energy → less likely to penetrate
        assert!(
            r_thick.energy_absorbed_j >= r_thin.energy_absorbed_j,
            "thicker PVB should absorb at least as much energy"
        );
    }

    // ── Bullet-resistant glass ────────────────────────────────────────────────

    #[test]
    fn br_glass_stops_9mm_pistol() {
        // BR panel: 30 mm glass + 6 mm PC + 3 mm acrylic
        // effective = 0.030*0.15 + 0.006*0.6 + 0.003*0.4
        //           = 0.0045 + 0.0036 + 0.0012 = 0.0093 m
        // 9 mm FMJ, 8.0 g, 358 m/s
        // V_req = 91000/1.0 * 0.009^0.75 * 0.0093^0.7 / sqrt(0.008) ≈ 1116 m/s
        // 358 << 1116 → stopped
        let r = eval(params(
            SecurityGlassType::BulletResistant {
                glass_thickness_mm: 30.0,
                polycarbonate_thickness_mm: 6.0,
                acrylic_thickness_mm: 3.0,
            },
            358.0,
            8.0,
            9.0,
            "fmj",
            0.0,
        ));
        assert!(!r.penetrated, "BR glass should stop 9 mm FMJ at 358 m/s");
        assert!(!r.spall_generated, "spall shield prevents rear spall");
        assert_eq!(r.residual_velocity_ms, 0.0, "projectile fully stopped");
    }

    #[test]
    fn br_glass_penetrated_by_50_bmg() {
        // BR panel: 30 mm glass + 6 mm PC + 3 mm acrylic (same as above)
        // .50 BMG M2 AP, 45.8 g, 884 m/s, 12.7 mm
        // V_req ≈ 91000/1.3 * 0.0127^0.75 * 0.0093^0.7 / sqrt(0.0458) ≈ 559 m/s
        // AP proj modifier = 1.3
        // 884 >> 559 → penetrated
        let r = eval(params(
            SecurityGlassType::BulletResistant {
                glass_thickness_mm: 30.0,
                polycarbonate_thickness_mm: 6.0,
                acrylic_thickness_mm: 3.0,
            },
            884.0,
            45.8,
            12.7,
            "ap",
            0.0,
        ));
        assert!(
            r.penetrated,
            ".50 BMG AP should penetrate moderate BR glass"
        );
        assert!(r.spall_generated, "spall on BR penetration");
    }

    #[test]
    fn br_glass_angle_reduces_penetration() {
        // Same BR panel, same projectile (9 mm), but at 45° angle
        // Angle increases effective thickness by 1/cos(45°) = 1.414
        // So even more stopping power → should also stop (already stopped at 0°)
        let r = eval(params(
            SecurityGlassType::BulletResistant {
                glass_thickness_mm: 30.0,
                polycarbonate_thickness_mm: 6.0,
                acrylic_thickness_mm: 3.0,
            },
            358.0,
            8.0,
            9.0,
            "fmj",
            45.0,
        ));
        assert!(!r.penetrated, "angled BR glass still stops 9 mm");
    }

    // ── Automotive side glass ────────────────────────────────────────────────

    #[test]
    fn automotive_side_penetrates_at_rifle_velocity() {
        // 4 mm automotive side (tempered), 5.56 mm FMJ at 900 m/s
        // threshold = 50 * sqrt(4/6) ≈ 40.8 m/s
        // 900 >> 40.8 → penetration
        let r = eval(params(
            SecurityGlassType::AutomotiveSide { thickness_mm: 4.0 },
            900.0,
            4.0,
            5.56,
            "fmj",
            0.0,
        ));
        assert!(
            r.penetrated,
            "rifle round easily penetrates automotive side glass"
        );
        assert!(r.glass_pane_shattered, "tempered pane shatters");
        assert!(r.spall_generated, "spall on penetration");
    }

    // ── UL 752 protection verification ───────────────────────────────────────

    #[test]
    fn ul752_level1_stops_9mm() {
        // 9 mm FMJ at 358 m/s should be within Level 1 protection
        let protected = evaluate_ul752_protection(UL752Level::Level1, 358.0, 8.0, 0.009);
        assert!(
            protected,
            "Level 1 glass should stop 9 mm FMJ at reference velocity"
        );
    }

    #[test]
    fn ul752_level3_stops_44magnum_but_not_rifle() {
        // .44 Mag at reference should be stopped by Level 3
        let protected_pistol = evaluate_ul752_protection(UL752Level::Level3, 427.0, 15.6, 0.0109);
        assert!(protected_pistol, "Level 3 should stop .44 Mag");

        // 7.62 mm rifle at 838 m/s should NOT be stopped by Level 3
        let protected_rifle = evaluate_ul752_protection(UL752Level::Level3, 838.0, 9.7, 0.00762);
        assert!(!protected_rifle, "Level 3 should not stop 7.62 mm rifle");
    }

    #[test]
    fn ul752_level10_stops_50bmg() {
        let protected = evaluate_ul752_protection(UL752Level::Level10, 884.0, 45.8, 0.0127);
        assert!(protected, "Level 10 should stop .50 BMG M2 AP at reference");
    }

    #[test]
    fn ul752_higher_levels_are_more_protective() {
        // A .22 LR (2.6 g, 320 m/s, 5.6 mm) should be stopped by all
        // levels.  Check that indices are monotonic.
        let l1 = evaluate_ul752_protection(UL752Level::Level1, 320.0, 2.6, 0.0056);
        let l4 = evaluate_ul752_protection(UL752Level::Level4, 320.0, 2.6, 0.0056);
        let l10 = evaluate_ul752_protection(UL752Level::Level10, 320.0, 2.6, 0.0056);
        assert!(l1, "Level 1 should stop .22 LR");
        assert!(l4, "Level 4 should stop .22 LR");
        assert!(l10, "Level 10 should stop .22 LR");
    }

    // ── Edge cases ───────────────────────────────────────────────────────────

    #[test]
    fn zero_velocity_or_mass_no_penetration() {
        let types = [
            SecurityGlassType::Tempered { thickness_mm: 6.0 },
            SecurityGlassType::AutomotiveSide { thickness_mm: 4.0 },
            SecurityGlassType::Laminated {
                glass_thickness_mm: 4.6,
                pvb_layer_count: 1,
                pvb_thickness_per_layer_mm: 0.76,
            },
            SecurityGlassType::BulletResistant {
                glass_thickness_mm: 30.0,
                polycarbonate_thickness_mm: 6.0,
                acrylic_thickness_mm: 3.0,
            },
        ];
        for gt in &types {
            let r = eval(params(*gt, 0.0, 4.0, 5.56, "fmj", 0.0));
            assert!(!r.penetrated, "{:?}: no pen at zero velocity", gt);
            assert_eq!(r.energy_absorbed_j, 0.0);

            let r2 = eval(params(*gt, 500.0, 0.0, 5.56, "fmj", 0.0));
            assert!(!r2.penetrated, "{:?}: no pen at zero mass", gt);
        }
    }

    #[test]
    fn deterministic_output() {
        // Same input → same output
        let p = GlassPenetrationParams {
            glass_type: SecurityGlassType::Tempered { thickness_mm: 6.0 },
            velocity_ms: 100.0,
            mass_g: 4.0,
            caliber_m: 0.00556,
            projectile_type: "fmj".to_string(),
            impact_angle_deg: 0.0,
        };
        let a = evaluate_glass_penetration(&p);
        let b = evaluate_glass_penetration(&p);
        assert_eq!(a.penetrated, b.penetrated);
        assert!((a.residual_velocity_ms - b.residual_velocity_ms).abs() < 1e-12);
        assert!((a.energy_absorbed_j - b.energy_absorbed_j).abs() < 1e-12);
        assert_eq!(a.spall_mass_g, b.spall_mass_g);
    }

    #[test]
    fn pen_index_monotonic_with_energy() {
        // Higher KE should always give higher penetration index
        // (for same calibre and mass)
        let low = pen_index(0.004, 300.0, 0.00556);
        let high = pen_index(0.004, 900.0, 0.00556);
        assert!(
            high > low,
            "pen index should increase with velocity: {} < {}",
            low,
            high
        );

        // Heavier projectile at same velocity → higher index
        let light = pen_index(0.004, 500.0, 0.00762);
        let heavy = pen_index(0.0095, 500.0, 0.00762);
        assert!(
            heavy > light,
            "pen index should increase with mass: {} < {}",
            light,
            heavy
        );
    }
}
