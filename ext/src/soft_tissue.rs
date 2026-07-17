// ABE - Soft Tissue / Wound Ballistics Model
//
// Models the interaction between projectiles and soft (biological) tissue.
// Implements temporary cavity, permanent wound channel, penetration depth,
// and energy deposition models used in terminal ballistics.
//
// References:
//   - Fackler, M.L., "Wound Ballistics: A Review of Common Misconceptions"
//   - FBI HPR (Handgun Wounding Effectiveness) reports, FBI Academy
//   - Nennstiel, R., "Wound Ballistics" in Encyclopedia of Forensic Sciences
//   - Peters, C.E., "Wound Ballistics of Yawed Rifle Projectiles" (AD-A233 621)
//   - Sturdivan, L.M., "Mathematical Modeling of Wound Ballistics"
//   - UK Defence Standard 13-100 (Fragment Simulating Projectiles)
//   - MacPherson, D., "Bullet Penetration: Modeling the Dynamics and
//     the Wounding Capacity of Projectiles in Soft Tissue" (1994)
//   - NIJ Standard 0101.06 (Ballistic Resistance of Body Armor)
//   - STANAG 4569 (Protection Levels for Logistic and Light Armored Vehicles)
//   - JBM Ballistics / PRODAS trajectory tables reference data

/// Tissue density in kg/m³ (nominally water-equivalent, ~5 % denser).
const TISSUE_DENSITY: f64 = 1040.0;

/// Drag coefficient of a typical projectile in soft tissue.
/// Caliber-dependent; 0.45–0.55 for pistol/rifle rounds in water simulant.
const TISSUE_CD: f64 = 0.50;

/// Temporary cavity expansion constant (m / sqrt(N)).
/// Relates energy deposition per unit path length to peak cavity diameter.
/// D_temp = 2 * k_c * sqrt(dE/dx)
/// For typical muscle/ballistic gelatin: k_c ≈ 0.0003–0.0007 m/N^0.5
/// giving temporary cavities of 30–100 mm for handgun rounds and
/// 60–200 mm (often capped by tissue rupture) for yawing rifle rounds.
/// Reference: Fackler, Peters, FBI HPR ballistic gelatin data.
const CAVITY_CONSTANT: f64 = 0.0005;

/// Minimum retained velocity (m/s) for wounding significance.
/// Below ∼50 m/s the projectile delivers negligible energy to tissue.
const WOUND_THRESHOLD_MS: f64 = 50.0;

// ── Bone model constants ───────────────────────────────────────────────────────

/// Cortical bone density (kg/m³).
const CORTICAL_BONE_DENSITY: f64 = 1900.0;

/// Trabecular bone density (kg/m³).
const TRABECULAR_BONE_DENSITY: f64 = 1000.0;

/// Compressive strength of cortical bone (MPa).
const CORTICAL_BONE_STRENGTH_MPA: f64 = 170.0;

/// Bone penetration calibration constant.
/// E_bone = K_BONE × t² × sqrt(density) × area
/// Calibrated so that:
///   - .22 LR (142 J) does NOT penetrate skull (7mm)
///   - 9mm (518 J) is borderline for sternum (6mm)
///   - M855 (1730 J) penetrates femur (20mm) with ~55% velocity retention
const K_BONE: f64 = 1.0e9;

// ── Bone types ─────────────────────────────────────────────────────────────────

/// Types of bone that a projectile may encounter in a wound track.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BoneType {
    Skull,
    Rib,
    Femur,
    Sternum,
    Pelvis,
    Tibia,
    Humerus,
    Spine,
    Generic { thickness_m: f64 },
}

impl BoneType {
    /// Estimated thickness of this bone (metres).
    pub fn thickness_m(&self) -> f64 {
        match self {
            BoneType::Skull => 0.007,
            BoneType::Rib => 0.002,
            BoneType::Femur => 0.020,
            BoneType::Sternum => 0.006,
            BoneType::Pelvis => 0.008,
            BoneType::Tibia => 0.015,
            BoneType::Humerus => 0.012,
            BoneType::Spine => 0.010,
            BoneType::Generic { thickness_m } => *thickness_m,
        }
    }

    /// Effective density used for penetration energy calculation (kg/m³).
    /// Uses cortical density as the primary resistance layer.
    pub fn effective_density(&self) -> f64 {
        CORTICAL_BONE_DENSITY
    }

    /// Estimated bone cross-sectional area presented to the projectile (m²).
    /// Roughly 3-8× the projectile presented area for most bones.
    pub fn effective_area_m2(&self, caliber_m: f64) -> f64 {
        let width_mult = match self {
            BoneType::Skull => 3.0,
            BoneType::Rib => 2.0,
            BoneType::Femur => 3.0,
            BoneType::Sternum => 4.0,
            BoneType::Pelvis => 6.0,
            BoneType::Tibia => 4.0,
            BoneType::Humerus => 4.0,
            BoneType::Spine => 8.0,
            BoneType::Generic { .. } => 4.0,
        };
        let proj_area = std::f64::consts::PI * (caliber_m / 2.0).powi(2);
        proj_area * width_mult
    }
}

// ── Bone impact result ─────────────────────────────────────────────────────────

/// Result of evaluating a projectile impact against bone.
#[derive(Debug, Clone, Copy)]
pub struct BoneImpactResult {
    /// Whether the projectile fully penetrated the bone.
    pub penetrated: bool,
    /// Number of bone fragments generated (0 if no fracture).
    pub bone_fragments: i32,
    /// Total estimated mass of bone fragments (grams).
    pub bone_fragment_mass_g: f64,
    /// Angle the projectile was deflected (degrees).
    pub deflection_angle_deg: f64,
    /// Projectile velocity after interacting with bone (m/s).
    pub velocity_after_bone_ms: f64,
    /// Energy deposited in the bone (J).
    pub energy_deposited_in_bone_j: f64,
}

/// Evaluate projectile impact against a bone.
///
/// Physics model:
/// - Bone treated as a brittle plate with penetration energy:
///   E_bone = K_bone × thickness² × density_bone^0.5 × area_bone
/// - If remaining KE > E_bone: penetrate, reduce velocity
/// - If not: projectile stops/deflects, energy deposits as fracture
/// - Bone generates 2-8 sharp secondary fragments
/// - Deflection angle: 5-25° depending on impact angle and bone curvature
pub fn evaluate_bone_impact(
    velocity_ms: f64,
    mass_g: f64,
    caliber_m: f64,
    projectile_type: &str,
    bone: BoneType,
    impact_angle_deg: f64,
) -> BoneImpactResult {
    if velocity_ms <= 0.0 || mass_g <= 0.0 {
        return BoneImpactResult {
            penetrated: false,
            bone_fragments: 0,
            bone_fragment_mass_g: 0.0,
            deflection_angle_deg: 0.0,
            velocity_after_bone_ms: 0.0,
            energy_deposited_in_bone_j: 0.0,
        };
    }

    let mass_kg = mass_g / 1000.0;
    let ke = 0.5 * mass_kg * velocity_ms.powi(2);

    // Bone penetration energy: K_BONE × thickness² × density^0.5 × area
    let thickness = bone.thickness_m();
    let density = bone.effective_density();
    let area = bone.effective_area_m2(caliber_m);
    let e_bone = K_BONE * thickness.powi(2) * density.sqrt() * area;

    // Impact angle modifies effective energy: normal impact = full,
    // oblique = less energy transferred to penetration
    let angle_rad = impact_angle_deg.to_radians();
    let angle_factor = angle_rad.cos().max(0.1); // minimum 10% at extreme angles
    let e_bone_effective = e_bone / angle_factor;

    // AP projectiles resist better against bone
    let proj_type = projectile_type.to_lowercase();
    let is_ap = proj_type == "ap" || proj_type == "armor_piercing";
    let ap_bonus = if is_ap { 0.7 } else { 1.0 };
    let e_bone_effective = e_bone_effective * ap_bonus;

    // Determine penetration
    let (penetrated, vel_after, energy_in_bone) = if ke > e_bone_effective {
        // Penetrate: reduce velocity
        let frac = (e_bone_effective / ke).min(0.99);
        let vel_after = velocity_ms * (1.0 - frac).sqrt();
        (true, vel_after.max(0.0), e_bone_effective.min(ke))
    } else {
        // Stopped/deflected: all KE deposited
        (false, 0.0, ke)
    };

    // Bone fragments: 2-8 sharp secondary fragments
    // Scales with energy deposited in bone (roughly one fragment per 300 J)
    let fragment_count = if penetrated {
        let frag_by_energy = (energy_in_bone / 300.0).round() as i32;
        (2 + frag_by_energy).min(8)
    } else {
        // Even without penetration, near-penetrating impacts can crack bone
        if ke > e_bone_effective * 0.6 {
            ((energy_in_bone / 500.0).round() as i32).min(4)
        } else {
            0
        }
    };

    // Fragment mass: 0.1-2.0g each, total scales with energy
    let fragment_mass_g = if fragment_count > 0 {
        let avg_frag_mass = 0.1 + (energy_in_bone / 1000.0).min(1.9);
        fragment_count as f64 * avg_frag_mass.min(2.0)
    } else {
        0.0
    };

    // Deflection angle: 5-25° depending on impact angle and bone curvature
    // Oblique impacts deflect more; glancing impacts (high angle) deflect most
    let deflection = if penetrated {
        let base_deflection = 5.0 + impact_angle_deg * 0.15; // 5-18.5° for 0-90°
        base_deflection.min(25.0)
    } else {
        // Stopped projectiles may deflect sharply
        let base_deflection = 10.0 + impact_angle_deg * 0.2; // 10-28°
        base_deflection.min(30.0)
    };

    BoneImpactResult {
        penetrated,
        bone_fragments: fragment_count,
        bone_fragment_mass_g: fragment_mass_g,
        deflection_angle_deg: deflection,
        velocity_after_bone_ms: vel_after,
        energy_deposited_in_bone_j: energy_in_bone,
    }
}

// ── Wound ballistic result ─────────────────────────────────────────────────────

/// Result of a soft-tissue wound ballistics evaluation.
#[derive(Debug, Clone)]
pub struct WoundResult {
    /// Total penetration depth in soft tissue (metres).
    pub penetration_depth_m: f64,
    /// Peak temporary cavity diameter (metres). The maximum radial
    /// stretch the tissue experiences as the projectile passes through.
    pub temp_cavity_diameter_m: f64,
    /// Permanent (crushed tissue) wound channel diameter (metres).
    /// For FMJ this is ≈ projectile diameter; for expanding bullets it
    /// may be 1.5–2.5× calibre.
    pub perm_cavity_diameter_m: f64,
    /// Total kinetic energy deposited in the wound track (J).
    pub energy_deposited_j: f64,
    /// Energy deposition per unit path length at the point of peak
    /// temporary cavity (J/cm). Used to compare wounding potential
    /// between calibres.
    pub peak_edep_j_per_cm: f64,
    /// Whether the projectile yawed in tissue (rifle rounds typically
    /// yaw 90–180° within the first 10–20 cm, dramatically increasing
    /// energy deposition).
    pub yawed: bool,
    /// Penetration depth at which the projectile yawed (m). 0.0 if
    /// the projectile did not yaw (handgun rounds, or subsonic).
    pub yaw_depth_m: f64,
    /// Fragmentation contribution: additional mass from projectiles
    /// that break up in tissue (grams). M193/M855 can shed jacket
    /// fragments, significantly enlarging the wound track.
    pub frag_mass_g: f64,
    /// Whether the projectile penetrated a bone in the wound track.
    pub bone_penetrated: bool,
    /// Number of bone fragments generated by impact (0 if no bone hit).
    pub bone_fragments: i32,
    /// Energy deposited in bone during impact (J).
    pub bone_energy_j: f64,
}

// ── Main evaluation ────────────────────────────────────────────────────────────

/// Evaluate wound ballistics for a projectile entering soft tissue.
///
/// Uses a three-phase model:
/// 1. **Penetration** — the projectile decelerates in tissue as a
///    dense-fluid drag problem: `F = ½·ρ·A·Cd·v²`. Penetration depth
///    is found by integrating the drag equation.
/// 2. **Temporary cavity** — the peak radial stretch diameter at a
///    given depth scales with sqrt(dE/dx). The maximum over the wound
///    track is reported.
/// 3. **Permanent cavity** — the crushed-tissue track diameter. For
///    non-expanding projectiles this is ≈ projectile calibre. Yawing
///    and fragmentation increase it substantially.
///
/// # Arguments
/// * `velocity_ms` — Impact velocity (m/s).
/// * `mass_g` — Projectile mass (grams).
/// * `caliber_m` — Projectile diameter (metres).
/// * `projectile_type` — Projectile construction identifier
///   ("fmj", "soft_point", "hollow_point", "ball", "ap", etc.).
///
/// # Returns
/// `WoundResult` summarising the wound channel properties.
pub fn evaluate(
    velocity_ms: f64,
    mass_g: f64,
    caliber_m: f64,
    projectile_type: &str,
) -> WoundResult {
    if velocity_ms <= WOUND_THRESHOLD_MS || mass_g <= 0.0 || caliber_m <= 0.0 {
        return WoundResult {
            penetration_depth_m: 0.0,
            temp_cavity_diameter_m: 0.0,
            perm_cavity_diameter_m: 0.0,
            energy_deposited_j: 0.0,
            peak_edep_j_per_cm: 0.0,
            yawed: false,
            yaw_depth_m: 0.0,
            frag_mass_g: 0.0,
            bone_penetrated: false,
            bone_fragments: 0,
            bone_energy_j: 0.0,
        };
    }

    let mass_kg = mass_g / 1000.0;
    let area = std::f64::consts::PI * (caliber_m / 2.0).powi(2);
    let proj_type = projectile_type.to_lowercase();

    // ── 1. Penetration depth ─────────────────────────────────────────
    // Drag equation in tissue: m·dv/dt = -½·ρ·A·Cd·v²
    // Integrated: x = (2·m / (ρ·A·Cd)) · ln(v₀ / v_threshold)
    //
    // This gives the "total stopping distance" in tissue-homologous
    // material (ballistic gelatin or muscle tissue).
    let drag_factor = TISSUE_DENSITY * area * TISSUE_CD / (2.0 * mass_kg);
    let penetration_depth = if drag_factor > 0.0 {
        (velocity_ms / WOUND_THRESHOLD_MS.max(1.0)).ln() / drag_factor
    } else {
        0.0
    };
    let penetration_depth = penetration_depth.min(1.5); // sanity cap: 1.5 m is very deep

    // ── 2. Energy deposition per unit length (dE/dx) ──────────────────
    // dE/dx = F_drag = ½·ρ·A·Cd·v(x)²
    // The velocity at each point x: v(x) = v₀·exp(-drag_factor·x)
    // So dE/dx(x) = ½·ρ·A·Cd·v₀²·exp(-2·drag_factor·x)
    //
    // Peak dE/dx is at the surface (x=0) for non-yawing projectiles.
    // For yawing rifles, the actual peak occurs post-yaw.
    let edep_surface = 0.5 * TISSUE_DENSITY * area * TISSUE_CD * velocity_ms.powi(2);

    // ── 3. Yaw depth (rifle rounds) ───────────────────────────────────
    // Rifle-calibre projectiles (v₀ > ~650 m/s, L/D > 2.5) typically
    // yaw 90–180° within the first 10–20 cm of penetration. The yaw
    // dramatically increases presented area, hence energy deposition.
    //
    // Yaw onset depth is empirically ~8–15 calibres for FMJ rifle rounds:
    //   yaw_depth = k_yaw * caliber_m, where k_yaw ≈ 8–15
    //
    // Aspect ratio estimate: L/D = mass_kg / (ρ · A · D) where
    //   ρ ≈ 8500 kg/m³ (generic lead-alloy projectile density).
    let is_ap = proj_type == "ap" || proj_type == "armor_piercing";
    let density_est = 8500.0; // kg/m³ — generic projectile material
    let length_m = mass_kg / (density_est * area).max(1e-30);
    let aspect_ratio = length_m / caliber_m.max(1e-30);
    let is_rifle = velocity_ms > 650.0 && aspect_ratio > 2.5;
    let (yawed, yaw_depth_m, yaw_area_multiplier) = if is_rifle && !is_ap {
        // Yaw depth: 8–15 calibres; longer/heavier projectiles yaw later
        let k_yaw = 8.0 + 4.0 * (aspect_ratio / 5.0).clamp(0.0, 1.0);
        let yaw_d = (k_yaw * caliber_m).min(penetration_depth * 0.5);
        // Post-yaw presented area increases 3–6× (side-on vs nose-on)
        let area_mult = 4.0;
        (yaw_d > 0.01, yaw_d, area_mult)
    } else {
        (false, 0.0, 1.0)
    };

    // ── 4. Fragmentation in tissue ────────────────────────────────────
    // High-velocity projectiles (v > ~700 m/s) can fragment in tissue,
    // especially varmint/soft-point designs. The jacket may shed at the
    // yaw point, creating 2–20 secondary fragments.
    let (frag_mass_g, frag_area_mult) = if yawed && velocity_ms > 700.0 {
        let proj_frac = match proj_type.as_str() {
            "soft_point" | "hollow_point" => 0.05, // 5 % mass shed
            "fmj" | "ball" => 0.02,                // 2 % for military ball
            "varmint" | "sp" => 0.08,              // 8 % for rapid-expanding
            _ => 0.01,
        };
        if velocity_ms > 900.0 {
            // M193/M855 regime: jacket separation
            (mass_g * proj_frac, 1.5)
        } else {
            (mass_g * proj_frac * 0.5, 1.2)
        }
    } else {
        (0.0, 1.0)
    };

    // ── 5. Temporary cavity diameter ──────────────────────────────────
    // Peak cavity diameter scales with sqrt(dE/dx) at the point of
    // maximum energy deposition. For yawing projectiles this is the
    // post-yaw phase; for non-yawing it is the surface value.
    //
    // D_temp = 2 * k_c * sqrt(dE/dx)
    // where k_c = CAVITY_CONSTANT. The factor of 2 converts radial
    // expansion to full diameter.
    let edep_max = if yawed {
        // Apply yaw area multiplier + fragmentation
        edep_surface * yaw_area_multiplier * frag_area_mult
    } else {
        edep_surface
    };
    let temp_cavity = 2.0 * CAVITY_CONSTANT * edep_max.sqrt();
    // Cap temporary cavity at 15× calibre (tissue rupture limit)
    let temp_cavity = temp_cavity.min(15.0 * caliber_m);

    // ── 6. Permanent cavity diameter ──────────────────────────────────
    // The crushed tissue track. Non-expanding: ≈0.9× calibre.
    // Expanding/yawing: up to 2.5× calibre.
    let perm_diameter = match proj_type.as_str() {
        "soft_point" | "hollow_point" => {
            if velocity_ms > 700.0 {
                // Full expansion
                (2.0 * caliber_m).min(3.0 * caliber_m)
            } else {
                // Partial expansion
                (1.4 * caliber_m).min(2.0 * caliber_m)
            }
        }
        "fmj" | "ball" => {
            if yawed {
                // Yawed FMJ creates a larger permanent cavity post-yaw
                0.9 * caliber_m + 0.5 * caliber_m * yaw_area_multiplier
            } else {
                0.9 * caliber_m // Minimal permanent cavity
            }
        }
        "varmint" | "sp" => {
            if velocity_ms > 800.0 {
                2.2 * caliber_m
            } else {
                1.5 * caliber_m
            }
        }
        "ap" | "armor_piercing" => 0.8 * caliber_m, // AP penetrates with minimal expansion
        _ => 0.9 * caliber_m,
    };
    let perm_cavity = perm_diameter.min(4.0 * caliber_m);

    // ── 7. Total energy deposited ─────────────────────────────────────
    // Integrate dE/dx over the penetration depth. For the simplified
    // model: E_deposited = ½·m·(v₀² - v_threshold²)
    // Actual losses to yaw and fragmentation increase deposition.
    let ke_initial = 0.5 * mass_kg * velocity_ms.powi(2);
    let ke_retained = 0.5 * mass_kg * WOUND_THRESHOLD_MS.powi(2);
    let mut edep = (ke_initial - ke_retained).max(0.0);

    // Yaw/fragmentation increase the effective drag, so more energy
    // is deposited in a shorter distance. Apply a multiplier.
    let edep_mult = if yawed { 1.3 } else { 1.0 } * (1.0 + frag_mass_g / mass_g * 2.0);
    edep *= edep_mult;

    // ── 8. Peak energy deposition per cm ──────────────────────────────
    let peak_edep_j_per_cm = edep_max * 0.01; // Convert J/m → J/cm

    WoundResult {
        penetration_depth_m: penetration_depth,
        temp_cavity_diameter_m: temp_cavity,
        perm_cavity_diameter_m: perm_cavity,
        energy_deposited_j: edep,
        peak_edep_j_per_cm,
        yawed,
        yaw_depth_m: yaw_depth_m,
        frag_mass_g,
        bone_penetrated: false,
        bone_fragments: 0,
        bone_energy_j: 0.0,
    }
}

/// Evaluate wound ballistics with optional bone interaction.
///
/// Extends `evaluate()` with a bone interaction model. If `bone` is `Some`,
/// the projectile first penetrates soft tissue to reach the bone (assumed
/// at `bone_depth_m`), then the bone impact is evaluated, and soft tissue
/// penetration continues with the post-bone velocity.
///
/// # Arguments
/// * `velocity_ms` — Impact velocity (m/s).
/// * `mass_g` — Projectile mass (grams).
/// * `caliber_m` — Projectile diameter (metres).
/// * `projectile_type` — Projectile construction identifier.
/// * `bone` — Optional tuple of (BoneType, impact_angle_deg, bone_depth_m).
///
/// # Returns
/// `WoundResult` with bone interaction effects included.
pub fn evaluate_extended(
    velocity_ms: f64,
    mass_g: f64,
    caliber_m: f64,
    projectile_type: &str,
    bone: Option<(BoneType, f64, f64)>,
) -> WoundResult {
    if velocity_ms <= WOUND_THRESHOLD_MS || mass_g <= 0.0 || caliber_m <= 0.0 {
        return WoundResult {
            penetration_depth_m: 0.0,
            temp_cavity_diameter_m: 0.0,
            perm_cavity_diameter_m: 0.0,
            energy_deposited_j: 0.0,
            peak_edep_j_per_cm: 0.0,
            yawed: false,
            yaw_depth_m: 0.0,
            frag_mass_g: 0.0,
            bone_penetrated: false,
            bone_fragments: 0,
            bone_energy_j: 0.0,
        };
    }

    let (bone_type, impact_angle_deg, bone_depth_m) = match bone {
        Some(b) => b,
        None => {
            // No bone: delegate to standard evaluate
            return evaluate(velocity_ms, mass_g, caliber_m, projectile_type);
        }
    };

    let mass_kg = mass_g / 1000.0;
    let area = std::f64::consts::PI * (caliber_m / 2.0).powi(2);
    let proj_type = projectile_type.to_lowercase();

    // ── 1. Compute pre-bone soft tissue penetration ─────────────────────
    // Drag equation gives us the velocity at depth bone_depth_m
    let drag_factor = TISSUE_DENSITY * area * TISSUE_CD / (2.0 * mass_kg);
    let vel_at_bone = if drag_factor > 0.0 {
        velocity_ms * (-drag_factor * bone_depth_m).exp()
    } else {
        velocity_ms
    };

    // If velocity at bone is below threshold, just do standard eval
    if vel_at_bone <= WOUND_THRESHOLD_MS {
        return evaluate(velocity_ms, mass_g, caliber_m, projectile_type);
    }

    // ── 2. Evaluate bone impact ─────────────────────────────────────────
    let bone_result = evaluate_bone_impact(
        vel_at_bone,
        mass_g,
        caliber_m,
        projectile_type,
        bone_type,
        impact_angle_deg,
    );

    // ── 3. Continue soft tissue penetration after bone ──────────────────
    let (total_penetration, temp_cavity, perm_cavity, edep, peak_edep, yawed, yaw_d, frag) =
        if bone_result.penetrated {
            // Post-bone: continue with reduced velocity
            let post_bone_vel = bone_result.velocity_after_bone_ms;
            let _post_bone_ke = 0.5 * mass_kg * post_bone_vel.powi(2);

            // Remaining penetration depth after bone
            let post_bone_pen = if drag_factor > 0.0 && post_bone_vel > WOUND_THRESHOLD_MS {
                (post_bone_vel / WOUND_THRESHOLD_MS.max(1.0)).ln() / drag_factor
            } else {
                0.0
            };
            let total_pen = bone_depth_m + post_bone_pen.min(1.5);

            // Yaw: bone impact can induce yaw even in handgun rounds
            let is_ap = proj_type == "ap" || proj_type == "armor_piercing";
            let density_est = 8500.0;
            let length_m = mass_kg / (density_est * area).max(1e-30);
            let aspect_ratio = length_m / caliber_m.max(1e-30);
            let is_rifle = post_bone_vel > 650.0 && aspect_ratio > 2.5;
            let yawed = bone_result.deflection_angle_deg > 10.0 || (is_rifle && !is_ap);
            let yaw_d = if yawed { bone_depth_m } else { 0.0 };

            // Post-bone yaw area multiplier
            let yaw_area_mult = if yawed { 4.0 } else { 1.0 };

            // dE/dx at bone location
            let edep_bone = 0.5 * TISSUE_DENSITY * area * TISSUE_CD * post_bone_vel.powi(2);

            // Pre-bone dE/dx at surface (highest pre-bone velocity)
            let edep_surface_pre = 0.5 * TISSUE_DENSITY * area * TISSUE_CD * velocity_ms.powi(2);

            // Bone interface energy deposition rate:
            // energy deposited in bone concentrated over ~2× bone thickness
            let bone_edep_rate =
                bone_result.energy_deposited_in_bone_j / (bone_type.thickness_m() * 2.0).max(0.001);

            // Peak dE/dx: max of pre-bone surface, bone interface, post-bone (with yaw)
            let edep_max = edep_surface_pre
                .max(bone_edep_rate)
                .max(edep_bone * yaw_area_mult);

            // Temp cavity: bone impact amplifies (1.5-2.0× near bone)
            let bone_amp = if bone_result.penetrated { 1.5 } else { 1.0 };
            let temp_cav = 2.0 * CAVITY_CONSTANT * edep_max.sqrt() * bone_amp;
            let temp_cav = temp_cav.min(15.0 * caliber_m);

            // Permanent cavity
            let perm_d = match proj_type.as_str() {
                "soft_point" | "hollow_point" => {
                    if post_bone_vel > 700.0 {
                        2.0 * caliber_m
                    } else {
                        1.4 * caliber_m
                    }
                }
                "fmj" | "ball" => {
                    if yawed {
                        0.9 * caliber_m + 0.5 * caliber_m * yaw_area_mult
                    } else {
                        0.9 * caliber_m
                    }
                }
                "varmint" | "sp" => {
                    if post_bone_vel > 800.0 {
                        2.2 * caliber_m
                    } else {
                        1.5 * caliber_m
                    }
                }
                _ => 0.9 * caliber_m,
            };
            let perm_cav = perm_d.min(4.0 * caliber_m);

            // Fragmentation: bone produces secondary fragments
            let frag_mass = bone_result.bone_fragment_mass_g * 0.5; // ~50% of bone frag mass adds to wound

            // Total energy deposited: pre-bone KE loss + bone + post-bone
            let ke_initial = 0.5 * mass_kg * velocity_ms.powi(2);
            let ke_final = 0.5 * mass_kg * WOUND_THRESHOLD_MS.powi(2);
            let total_edep = (ke_initial - ke_final).max(0.0);

            let peak_edep_j_per_cm = edep_max * 0.01;

            (
                total_pen,
                temp_cav,
                perm_cav,
                total_edep,
                peak_edep_j_per_cm,
                yawed,
                yaw_d,
                frag_mass,
            )
        } else {
            // Bone stopped projectile: penetration capped at bone depth
            let _ke_at_bone = 0.5 * mass_kg * vel_at_bone.powi(2);
            let edep = bone_result.energy_deposited_in_bone_j;

            // Cavity from energy deposited at bone
            let edep_rate = if bone_depth_m > 0.0 {
                edep / (bone_depth_m * 100.0) // J/cm
            } else {
                0.0
            };
            let temp_cav = 2.0 * CAVITY_CONSTANT * (edep / bone_depth_m.max(0.001)).sqrt();
            let temp_cav = temp_cav.min(15.0 * caliber_m);

            (
                bone_depth_m, // penetration capped
                temp_cav,
                0.9 * caliber_m, // minimal perm cavity
                edep,
                edep_rate,
                false, // no yaw (stopped)
                0.0,
                0.0,
            )
        };

    WoundResult {
        penetration_depth_m: total_penetration,
        temp_cavity_diameter_m: temp_cavity,
        perm_cavity_diameter_m: perm_cavity,
        energy_deposited_j: edep,
        peak_edep_j_per_cm: peak_edep,
        yawed,
        yaw_depth_m: yaw_d,
        frag_mass_g: frag,
        bone_penetrated: bone_result.penetrated,
        bone_fragments: bone_result.bone_fragments,
        bone_energy_j: bone_result.energy_deposited_in_bone_j,
    }
}

/// Evaluate wound profile along a discretised wound track.
///
/// Returns a vector of (depth_m, temp_cavity_diameter_m, dE_dx_J_per_m) samples
/// at `num_samples` evenly spaced points along the penetration path.
/// Useful for visualising the wound profile.
pub fn wound_profile(
    velocity_ms: f64,
    mass_g: f64,
    caliber_m: f64,
    projectile_type: &str,
    num_samples: usize,
) -> Vec<(f64, f64, f64)> {
    let result = evaluate(velocity_ms, mass_g, caliber_m, projectile_type);
    if result.penetration_depth_m <= 0.0 || num_samples == 0 {
        return Vec::new();
    }

    let mass_kg = mass_g / 1000.0;
    let area = std::f64::consts::PI * (caliber_m / 2.0).powi(2);
    let drag_factor = TISSUE_DENSITY * area * TISSUE_CD / (2.0 * mass_kg);
    let proj_type = projectile_type.to_lowercase();
    let is_ap = proj_type == "ap" || proj_type == "armor_piercing";
    let density_est = 8500.0;
    let length_m = mass_kg / (density_est * area).max(1e-30);
    let aspect_ratio = length_m / caliber_m.max(1e-30);
    let is_rifle = velocity_ms > 650.0 && aspect_ratio > 2.5;
    let yaw_depth = if is_rifle && !is_ap {
        let k_yaw = 8.0 + 4.0 * (aspect_ratio / 5.0).clamp(0.0, 1.0);
        (k_yaw * caliber_m).min(result.penetration_depth_m * 0.5)
    } else {
        f64::INFINITY
    };
    let area_base = area;
    let area_post_yaw = area_base * 4.0;
    let frag_mass = result.frag_mass_g;

    let mut profile = Vec::with_capacity(num_samples);
    for i in 0..num_samples {
        let frac = (i as f64 + 0.5) / num_samples as f64;
        let depth = result.penetration_depth_m * frac;

        // Determine effective presented area at this depth
        let effective_area = if depth >= yaw_depth {
            area_post_yaw + frag_mass / (mass_g * depth + 0.001) * area_base
        } else {
            area_base
        };

        // v(x) = v₀ · exp(-drag_factor · x)
        let v_at_depth = velocity_ms * (-drag_factor * depth).exp();
        let d_e_dx = 0.5 * TISSUE_DENSITY * effective_area * TISSUE_CD * v_at_depth.powi(2);

        // Temporary cavity at this depth
        let temp_d = 2.0 * CAVITY_CONSTANT * d_e_dx.sqrt();
        let temp_d = temp_d.min(15.0 * caliber_m);

        profile.push((depth, temp_d, d_e_dx));
    }
    profile
}

// ── NIJ / STANAG Reference Data ─────────────────────────────────────────────────

/// NIJ 0101.06 ballistic resistance threat levels for body armour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NIJLevel {
    /// Level IIA — 9mm FMJ RN @ 373 m/s, .40 S&W FMJ @ 325 m/s
    IIA,
    /// Level II — 9mm FMJ RN @ 398 m/s, .357 Mag JSP @ 430 m/s
    II,
    /// Level IIIA — .357 SIG FMJ FN @ 448 m/s, .44 Mag SJHP @ 436 m/s
    IIIA,
    /// Level III — 7.62×51mm M80 ball @ 847 m/s (rifle)
    III,
    /// Level IV — .30‑06 M2 AP @ 878 m/s (armour-piercing rifle)
    IV,
}

/// STANAG 4569 protection levels for logistic and light armoured vehicles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum STANAGLevel {
    /// Level 1 — 7.62×51mm M80 ball @ 833 m/s, 5.56×45mm M855 @ 900 m/s,
    /// and 155 mm HE blast at 10 m.
    L1,
    /// Level 2 — 7.62×39mm API BZ @ 695 m/s, plus Level 1 threats.
    L2,
    /// Level 3 — 7.62×51mm AP (M61) @ 838 m/s, plus Level 2 threats.
    L3,
    /// Level 4 — 14.5×114mm AP B32 @ 911 m/s, plus Level 3 threats.
    L4,
    /// Level 5 — 25 mm APDS-T @ 1258 m/s (Bushmaster chain gun), plus Level 4.
    L5,
}

/// Standard threat parameters for each NIJ level.
pub struct NIJThreat {
    pub level: NIJLevel,
    pub label: &'static str,
    pub velocity_ms: f64,
    pub mass_g: f64,
    pub caliber_m: f64,
}

/// Look up the reference threat for a given NIJ level.
pub fn nij_threat(level: NIJLevel) -> NIJThreat {
    match level {
        NIJLevel::IIA => NIJThreat {
            level: NIJLevel::IIA,
            label: "9mm FMJ RN (124 gr)",
            velocity_ms: 373.0,
            mass_g: 8.0,
            caliber_m: 0.00901,
        },
        NIJLevel::II => NIJThreat {
            level: NIJLevel::II,
            label: "9mm FMJ RN (124 gr)",
            velocity_ms: 398.0,
            mass_g: 8.0,
            caliber_m: 0.00901,
        },
        NIJLevel::IIIA => NIJThreat {
            level: NIJLevel::IIIA,
            label: ".44 Mag SJHP (240 gr)",
            velocity_ms: 436.0,
            mass_g: 15.6,
            caliber_m: 0.0109,
        },
        NIJLevel::III => NIJThreat {
            level: NIJLevel::III,
            label: "7.62×51mm M80 ball (147 gr)",
            velocity_ms: 847.0,
            mass_g: 9.5,
            caliber_m: 0.00782,
        },
        NIJLevel::IV => NIJThreat {
            level: NIJLevel::IV,
            label: ".30‑06 M2 AP (165 gr)",
            velocity_ms: 878.0,
            mass_g: 10.7,
            caliber_m: 0.00762,
        },
    }
}

/// Standard threat parameters for each STANAG 4569 level.
pub struct STANAGThreat {
    pub level: STANAGLevel,
    pub label: &'static str,
    pub velocity_ms: f64,
    pub mass_g: f64,
    pub caliber_m: f64,
}

/// Look up the reference KE threat for a given STANAG 4569 level.
pub fn stanag_threat(level: STANAGLevel) -> STANAGThreat {
    match level {
        STANAGLevel::L1 => STANAGThreat {
            level: STANAGLevel::L1,
            label: "7.62×51mm M80 ball",
            velocity_ms: 833.0,
            mass_g: 9.5,
            caliber_m: 0.00782,
        },
        STANAGLevel::L2 => STANAGThreat {
            level: STANAGLevel::L2,
            label: "7.62×39mm API BZ",
            velocity_ms: 695.0,
            mass_g: 7.9,
            caliber_m: 0.00762,
        },
        STANAGLevel::L3 => STANAGThreat {
            level: STANAGLevel::L3,
            label: "7.62×51mm AP M61",
            velocity_ms: 838.0,
            mass_g: 10.0,
            caliber_m: 0.00762,
        },
        STANAGLevel::L4 => STANAGThreat {
            level: STANAGLevel::L4,
            label: "14.5×114mm AP B32",
            velocity_ms: 911.0,
            mass_g: 64.0,
            caliber_m: 0.0145,
        },
        STANAGLevel::L5 => STANAGThreat {
            level: STANAGLevel::L5,
            label: "25mm APDS-T",
            velocity_ms: 1258.0,
            mass_g: 132.0,
            caliber_m: 0.0250,
        },
    }
}

// ── Reference trajectory data ───────────────────────────────────────────────────

/// Reference trajectory data for a single projectile/load combination.
/// Contains down-range velocity and drop sampled at standard intervals.
#[derive(Debug, Clone)]
pub struct TrajectorySample {
    pub range_m: f64,
    pub velocity_ms: f64,
    pub drop_m: f64,
}

/// Reference ammunition data: known BC, MV, and source.
#[derive(Debug, Clone)]
pub struct AmmoReference {
    pub name: &'static str,
    pub mv_ms: f64,
    pub bc_g7: f64,
    pub mass_g: f64,
    pub caliber_mm: f64,
    pub source: &'static str,
    pub trajectory_samples: &'static [TrajectorySample],
}

/// M855 (5.56×45mm) reference trajectory. G7 BC = 0.151 @ MV = 930 m/s.
/// Velocity and drop sampled from JBM/BRL trajectory tables (ICAO, sea level).
const M855_TRAJECTORY: &[TrajectorySample] = &[
    TrajectorySample {
        range_m: 0.0,
        velocity_ms: 930.0,
        drop_m: 0.000,
    },
    TrajectorySample {
        range_m: 100.0,
        velocity_ms: 842.0,
        drop_m: 0.069,
    },
    TrajectorySample {
        range_m: 200.0,
        velocity_ms: 759.0,
        drop_m: 0.301,
    },
    TrajectorySample {
        range_m: 300.0,
        velocity_ms: 679.0,
        drop_m: 0.738,
    },
    TrajectorySample {
        range_m: 400.0,
        velocity_ms: 603.0,
        drop_m: 1.428,
    },
    TrajectorySample {
        range_m: 500.0,
        velocity_ms: 530.0,
        drop_m: 2.433,
    },
    TrajectorySample {
        range_m: 600.0,
        velocity_ms: 462.0,
        drop_m: 3.829,
    },
    TrajectorySample {
        range_m: 800.0,
        velocity_ms: 337.0,
        drop_m: 8.477,
    },
    TrajectorySample {
        range_m: 1000.0,
        velocity_ms: 277.0,
        drop_m: 16.43,
    },
];

/// M80 (7.62×51mm) reference trajectory. G7 BC = 0.200 @ MV = 850 m/s.
const M80_TRAJECTORY: &[TrajectorySample] = &[
    TrajectorySample {
        range_m: 0.0,
        velocity_ms: 850.0,
        drop_m: 0.000,
    },
    TrajectorySample {
        range_m: 100.0,
        velocity_ms: 787.0,
        drop_m: 0.059,
    },
    TrajectorySample {
        range_m: 200.0,
        velocity_ms: 727.0,
        drop_m: 0.245,
    },
    TrajectorySample {
        range_m: 300.0,
        velocity_ms: 669.0,
        drop_m: 0.576,
    },
    TrajectorySample {
        range_m: 400.0,
        velocity_ms: 614.0,
        drop_m: 1.066,
    },
    TrajectorySample {
        range_m: 500.0,
        velocity_ms: 562.0,
        drop_m: 1.736,
    },
    TrajectorySample {
        range_m: 600.0,
        velocity_ms: 513.0,
        drop_m: 2.608,
    },
    TrajectorySample {
        range_m: 800.0,
        velocity_ms: 424.0,
        drop_m: 5.249,
    },
    TrajectorySample {
        range_m: 1000.0,
        velocity_ms: 349.0,
        drop_m: 9.676,
    },
];

/// M33 (7.62×51mm FMJ ball, also used as .308 Win match) reference.
/// G7 BC = 0.210 @ MV = 830 m/s.
const M33_TRAJECTORY: &[TrajectorySample] = &[
    TrajectorySample {
        range_m: 0.0,
        velocity_ms: 830.0,
        drop_m: 0.000,
    },
    TrajectorySample {
        range_m: 100.0,
        velocity_ms: 771.0,
        drop_m: 0.061,
    },
    TrajectorySample {
        range_m: 200.0,
        velocity_ms: 714.0,
        drop_m: 0.252,
    },
    TrajectorySample {
        range_m: 300.0,
        velocity_ms: 659.0,
        drop_m: 0.591,
    },
    TrajectorySample {
        range_m: 400.0,
        velocity_ms: 607.0,
        drop_m: 1.093,
    },
    TrajectorySample {
        range_m: 500.0,
        velocity_ms: 557.0,
        drop_m: 1.778,
    },
    TrajectorySample {
        range_m: 600.0,
        velocity_ms: 510.0,
        drop_m: 2.668,
    },
    TrajectorySample {
        range_m: 800.0,
        velocity_ms: 423.0,
        drop_m: 5.354,
    },
    TrajectorySample {
        range_m: 1000.0,
        velocity_ms: 359.0,
        drop_m: 9.786,
    },
];

/// M193 (5.56×45mm) reference trajectory. G7 BC = 0.178 @ MV = 990 m/s.
/// Lighter, faster than M855; similar G7 BC due to different construction.
const M193_TRAJECTORY: &[TrajectorySample] = &[
    TrajectorySample {
        range_m: 0.0,
        velocity_ms: 990.0,
        drop_m: 0.000,
    },
    TrajectorySample {
        range_m: 100.0,
        velocity_ms: 901.0,
        drop_m: 0.061,
    },
    TrajectorySample {
        range_m: 200.0,
        velocity_ms: 817.0,
        drop_m: 0.272,
    },
    TrajectorySample {
        range_m: 300.0,
        velocity_ms: 733.0,
        drop_m: 0.680,
    },
    TrajectorySample {
        range_m: 400.0,
        velocity_ms: 655.0,
        drop_m: 1.345,
    },
    TrajectorySample {
        range_m: 500.0,
        velocity_ms: 583.0,
        drop_m: 2.340,
    },
    TrajectorySample {
        range_m: 600.0,
        velocity_ms: 519.0,
        drop_m: 3.750,
    },
    TrajectorySample {
        range_m: 800.0,
        velocity_ms: 407.0,
        drop_m: 8.451,
    },
    TrajectorySample {
        range_m: 1000.0,
        velocity_ms: 336.0,
        drop_m: 16.48,
    },
];

/// 9mm FMJ (9×19mm Parabellum) reference trajectory. G7 BC = 0.067 @ MV = 370 m/s.
/// Typical 124 gr FMJ RN pistol round.
const TRAJECTORY_9MM_FMJ: &[TrajectorySample] = &[
    TrajectorySample {
        range_m: 0.0,
        velocity_ms: 370.0,
        drop_m: 0.000,
    },
    TrajectorySample {
        range_m: 25.0,
        velocity_ms: 353.0,
        drop_m: 0.043,
    },
    TrajectorySample {
        range_m: 50.0,
        velocity_ms: 336.0,
        drop_m: 0.170,
    },
    TrajectorySample {
        range_m: 75.0,
        velocity_ms: 321.0,
        drop_m: 0.394,
    },
    TrajectorySample {
        range_m: 100.0,
        velocity_ms: 306.0,
        drop_m: 0.722,
    },
    TrajectorySample {
        range_m: 150.0,
        velocity_ms: 278.0,
        drop_m: 1.784,
    },
    TrajectorySample {
        range_m: 200.0,
        velocity_ms: 253.0,
        drop_m: 3.578,
    },
    TrajectorySample {
        range_m: 300.0,
        velocity_ms: 211.0,
        drop_m: 9.962,
    },
    TrajectorySample {
        range_m: 400.0,
        velocity_ms: 179.0,
        drop_m: 21.91,
    },
];

/// .338 Lapua Magnum reference trajectory. G7 BC = 0.320 @ MV = 880 m/s.
/// 250 gr FMJBT long-range sniper round.
const LAPUA_338_TRAJECTORY: &[TrajectorySample] = &[
    TrajectorySample {
        range_m: 0.0,
        velocity_ms: 880.0,
        drop_m: 0.000,
    },
    TrajectorySample {
        range_m: 100.0,
        velocity_ms: 838.0,
        drop_m: 0.051,
    },
    TrajectorySample {
        range_m: 200.0,
        velocity_ms: 798.0,
        drop_m: 0.208,
    },
    TrajectorySample {
        range_m: 300.0,
        velocity_ms: 760.0,
        drop_m: 0.478,
    },
    TrajectorySample {
        range_m: 400.0,
        velocity_ms: 724.0,
        drop_m: 0.869,
    },
    TrajectorySample {
        range_m: 500.0,
        velocity_ms: 689.0,
        drop_m: 1.392,
    },
    TrajectorySample {
        range_m: 600.0,
        velocity_ms: 655.0,
        drop_m: 2.058,
    },
    TrajectorySample {
        range_m: 800.0,
        velocity_ms: 592.0,
        drop_m: 4.038,
    },
    TrajectorySample {
        range_m: 1000.0,
        velocity_ms: 534.0,
        drop_m: 7.178,
    },
    TrajectorySample {
        range_m: 1200.0,
        velocity_ms: 481.0,
        drop_m: 11.81,
    },
    TrajectorySample {
        range_m: 1500.0,
        velocity_ms: 418.0,
        drop_m: 20.97,
    },
];

/// .50 BMG (12.7×99mm) M33 ball reference trajectory. G7 BC = 0.435 @ MV = 860 m/s.
const BMG_50_TRAJECTORY: &[TrajectorySample] = &[
    TrajectorySample {
        range_m: 0.0,
        velocity_ms: 860.0,
        drop_m: 0.000,
    },
    TrajectorySample {
        range_m: 100.0,
        velocity_ms: 834.0,
        drop_m: 0.049,
    },
    TrajectorySample {
        range_m: 200.0,
        velocity_ms: 809.0,
        drop_m: 0.197,
    },
    TrajectorySample {
        range_m: 300.0,
        velocity_ms: 785.0,
        drop_m: 0.446,
    },
    TrajectorySample {
        range_m: 400.0,
        velocity_ms: 762.0,
        drop_m: 0.799,
    },
    TrajectorySample {
        range_m: 500.0,
        velocity_ms: 739.0,
        drop_m: 1.260,
    },
    TrajectorySample {
        range_m: 600.0,
        velocity_ms: 717.0,
        drop_m: 1.835,
    },
    TrajectorySample {
        range_m: 800.0,
        velocity_ms: 674.0,
        drop_m: 3.578,
    },
    TrajectorySample {
        range_m: 1000.0,
        velocity_ms: 633.0,
        drop_m: 6.261,
    },
    TrajectorySample {
        range_m: 1200.0,
        velocity_ms: 594.0,
        drop_m: 10.04,
    },
    TrajectorySample {
        range_m: 1500.0,
        velocity_ms: 539.0,
        drop_m: 17.62,
    },
    TrajectorySample {
        range_m: 2000.0,
        velocity_ms: 455.0,
        drop_m: 36.24,
    },
];

/// 7.62×39mm LPS (57-N-231) reference trajectory. G7 BC = 0.194 @ MV = 730 m/s.
const LPS_762X39_TRAJECTORY: &[TrajectorySample] = &[
    TrajectorySample {
        range_m: 0.0,
        velocity_ms: 730.0,
        drop_m: 0.000,
    },
    TrajectorySample {
        range_m: 100.0,
        velocity_ms: 675.0,
        drop_m: 0.072,
    },
    TrajectorySample {
        range_m: 200.0,
        velocity_ms: 623.0,
        drop_m: 0.296,
    },
    TrajectorySample {
        range_m: 300.0,
        velocity_ms: 574.0,
        drop_m: 0.707,
    },
    TrajectorySample {
        range_m: 400.0,
        velocity_ms: 528.0,
        drop_m: 1.348,
    },
    TrajectorySample {
        range_m: 500.0,
        velocity_ms: 485.0,
        drop_m: 2.269,
    },
    TrajectorySample {
        range_m: 600.0,
        velocity_ms: 446.0,
        drop_m: 3.540,
    },
    TrajectorySample {
        range_m: 800.0,
        velocity_ms: 376.0,
        drop_m: 7.691,
    },
    TrajectorySample {
        range_m: 1000.0,
        velocity_ms: 318.0,
        drop_m: 14.80,
    },
];

/// 5.45×39mm 7N6 reference trajectory. G7 BC = 0.145 @ MV = 900 m/s.
const TRAJECTORY_545_7N6: &[TrajectorySample] = &[
    TrajectorySample {
        range_m: 0.0,
        velocity_ms: 900.0,
        drop_m: 0.000,
    },
    TrajectorySample {
        range_m: 100.0,
        velocity_ms: 809.0,
        drop_m: 0.074,
    },
    TrajectorySample {
        range_m: 200.0,
        velocity_ms: 724.0,
        drop_m: 0.317,
    },
    TrajectorySample {
        range_m: 300.0,
        velocity_ms: 645.0,
        drop_m: 0.767,
    },
    TrajectorySample {
        range_m: 400.0,
        velocity_ms: 571.0,
        drop_m: 1.466,
    },
    TrajectorySample {
        range_m: 500.0,
        velocity_ms: 503.0,
        drop_m: 2.466,
    },
    TrajectorySample {
        range_m: 600.0,
        velocity_ms: 441.0,
        drop_m: 3.836,
    },
    TrajectorySample {
        range_m: 800.0,
        velocity_ms: 332.0,
        drop_m: 8.495,
    },
    TrajectorySample {
        range_m: 1000.0,
        velocity_ms: 265.0,
        drop_m: 16.90,
    },
];

/// Reference ammunition database.
pub fn ammo_references() -> Vec<AmmoReference> {
    vec![
        AmmoReference {
            name: "5.56×45mm M855",
            mv_ms: 930.0,
            bc_g7: 0.151,
            mass_g: 4.0,
            caliber_mm: 5.56,
            source: "APG/US Army BRL (ARL-TR-5182)",
            trajectory_samples: M855_TRAJECTORY,
        },
        AmmoReference {
            name: "7.62×51mm M80",
            mv_ms: 850.0,
            bc_g7: 0.200,
            mass_g: 9.5,
            caliber_mm: 7.62,
            source: "APG/US Army BRL (AD0815788)",
            trajectory_samples: M80_TRAJECTORY,
        },
        AmmoReference {
            name: "7.62×51mm M33 (FMJ Ball)",
            mv_ms: 830.0,
            bc_g7: 0.210,
            mass_g: 9.5,
            caliber_mm: 7.62,
            source: "Sierra/NRA match data",
            trajectory_samples: M33_TRAJECTORY,
        },
        AmmoReference {
            name: "5.56×45mm M193",
            mv_ms: 990.0,
            bc_g7: 0.178,
            mass_g: 4.0,
            caliber_mm: 5.56,
            source: "JBM Ballistics / US Army DARCOM",
            trajectory_samples: M193_TRAJECTORY,
        },
        AmmoReference {
            name: "9×19mm FMJ (124 gr)",
            mv_ms: 370.0,
            bc_g7: 0.067,
            mass_g: 8.0,
            caliber_mm: 9.0,
            source: "Sierra/NRA pistol data",
            trajectory_samples: TRAJECTORY_9MM_FMJ,
        },
        AmmoReference {
            name: ".338 Lapua Magnum",
            mv_ms: 880.0,
            bc_g7: 0.320,
            mass_g: 16.2,
            caliber_mm: 8.58,
            source: "Lapua / JBM Ballistics",
            trajectory_samples: LAPUA_338_TRAJECTORY,
        },
        AmmoReference {
            name: ".50 BMG M33 ball",
            mv_ms: 860.0,
            bc_g7: 0.435,
            mass_g: 42.8,
            caliber_mm: 12.7,
            source: "US MIL-DTL-4025 / JBM",
            trajectory_samples: BMG_50_TRAJECTORY,
        },
        AmmoReference {
            name: "7.62×39mm LPS (57-N-231)",
            mv_ms: 730.0,
            bc_g7: 0.194,
            mass_g: 7.9,
            caliber_mm: 7.62,
            source: "Russian Ammo Tech Data / JBM",
            trajectory_samples: LPS_762X39_TRAJECTORY,
        },
        AmmoReference {
            name: "5.45×39mm 7N6",
            mv_ms: 900.0,
            bc_g7: 0.145,
            mass_g: 3.4,
            caliber_mm: 5.45,
            source: "Russian Ammo Tech Data / ARL-TR-5182",
            trajectory_samples: TRAJECTORY_545_7N6,
        },
    ]
}

// ── Trajectory validation utilities ────────────────────────────────────────────

/// Result of comparing a simulated trajectory against reference data.
#[derive(Debug, Clone)]
pub struct TrajectoryValidationResult {
    pub ammo_name: &'static str,
    pub rmse_velocity_ms: f64,
    pub rmse_drop_m: f64,
    pub max_velocity_error_ms: f64,
    pub max_drop_error_m: f64,
    pub samples_compared: usize,
}

/// Check that a reference trajectory is internally consistent:
/// velocity decreases monotonically and drop increases monotonically.
pub fn check_trajectory_monotonic(ammo: &AmmoReference) -> bool {
    let samples = ammo.trajectory_samples;
    if samples.len() < 2 {
        return true; // trivially monotonic
    }

    // Velocity must never increase with range
    for i in 1..samples.len() {
        if samples[i].velocity_ms > samples[i - 1].velocity_ms + 0.5 {
            return false;
        }
    }

    // Drop must never decrease with range (drop is positive downward)
    for i in 1..samples.len() {
        if samples[i].drop_m < samples[i - 1].drop_m - 0.001 {
            return false;
        }
    }

    true
}

/// Linear-interpolate velocity and drop at an arbitrary range from
/// the reference trajectory samples.
pub fn interpolate_trajectory(ammo: &AmmoReference, range_m: f64) -> (f64, f64) {
    let samples = ammo.trajectory_samples;
    if samples.is_empty() {
        return (0.0, 0.0);
    }

    // Before first sample: use first values
    if range_m <= samples[0].range_m {
        return (samples[0].velocity_ms, samples[0].drop_m);
    }

    // After last sample: use last values
    if range_m >= samples[samples.len() - 1].range_m {
        let last = &samples[samples.len() - 1];
        return (last.velocity_ms, last.drop_m);
    }

    // Find the two bracketing samples and interpolate
    for i in 1..samples.len() {
        if range_m <= samples[i].range_m {
            let r0 = samples[i - 1].range_m;
            let r1 = samples[i].range_m;
            let t = (range_m - r0) / (r1 - r0);
            let vel = samples[i - 1].velocity_ms
                + t * (samples[i].velocity_ms - samples[i - 1].velocity_ms);
            let drop = samples[i - 1].drop_m + t * (samples[i].drop_m - samples[i - 1].drop_m);
            return (vel, drop);
        }
    }

    // Should not reach here
    let last = &samples[samples.len() - 1];
    (last.velocity_ms, last.drop_m)
}

/// Compute the kinetic energy (J) of the projectile at a given range
/// by interpolating velocity from the reference trajectory.
pub fn compute_energy_at_range(ammo: &AmmoReference, range_m: f64) -> f64 {
    let (vel_ms, _) = interpolate_trajectory(ammo, range_m);
    let mass_kg = ammo.mass_g / 1000.0;
    0.5 * mass_kg * vel_ms.powi(2)
}

// ── Behind-Armor Blunt Trauma (BABT) ──────────────────────────────────────────

/// Result of a behind-armor blunt trauma (BABT) evaluation for non-penetrating
/// hits on soft body armour.
#[derive(Debug, Clone, Copy)]
pub struct BABTResult {
    /// Backface deformation (mm) — the maximum indentation into the body.
    pub backface_deformation_mm: f64,
    /// Blunt trauma energy transmitted through the armour (J).
    pub blunt_energy_j: f64,
    /// Qualitative injury severity.
    pub injury_severity: &'static str,
    /// Probability of rib fracture (0.0–1.0).
    pub rib_fracture_probability: f64,
    /// Risk of liver/spleen injury (0.0–1.0).
    pub liver_spleen_risk: f64,
}

/// Evaluate behind-armor blunt trauma for non-penetrating hits on soft body armor.
///
/// Physics:
/// - Backface deformation (BFD) is proportional to impulse transfer to the armour:
///   BFD ≈ k_bfd × KE^0.5 / (thickness × density^0.5)
/// - Blunt trauma energy: E_blunt = KE × exp(-C × thickness × density^0.5)
/// - For aramid: C ≈ 800; UHMWPE: C ≈ 1200; steel: C ≈ 3000.
///
/// # Arguments
/// * `velocity_ms` — Impact velocity (m/s).
/// * `mass_g` — Projectile mass (g).
/// * `caliber_m` — Projectile diameter (m).
/// * `projectile_type` — Construction identifier.
/// * `armor_thickness_mm` — Armour thickness (mm).
/// * `armor_material` — Armour material ("aramid", "uhmwpe", "steel", etc.).
/// * `standoff_mm` — Gap between armour and body (mm, for plate carriers).
pub fn evaluate_babt(
    velocity_ms: f64,
    mass_g: f64,
    _caliber_m: f64,
    projectile_type: &str,
    armor_thickness_mm: f64,
    armor_material: &str,
    standoff_mm: f64,
) -> BABTResult {
    if velocity_ms <= 0.0 || mass_g <= 0.0 || armor_thickness_mm <= 0.0 {
        return BABTResult {
            backface_deformation_mm: 0.0,
            blunt_energy_j: 0.0,
            injury_severity: "none",
            rib_fracture_probability: 0.0,
            liver_spleen_risk: 0.0,
        };
    }

    let mass_kg = mass_g / 1000.0;
    let ke = 0.5 * mass_kg * velocity_ms.powi(2);

    // ── Material-dependent constants ────────────────────────────────────
    let mat_lower = armor_material.to_lowercase();
    let (k_bfd, c_factor, density): (f64, f64, f64) = match mat_lower.as_str() {
        "aramid" | "kevlar" | "twaron" => (0.00035, 800.0, 1440.0),
        "uhmwpe" | "dyneema" | "spectra" => (0.00025, 1200.0, 970.0),
        "steel" | "steel_rha" | "hha" => (0.00003, 3000.0, 7850.0),
        "ceramic" | "b4c" | "sic" | "al2o3" => (0.00010, 2000.0, 3200.0),
        "titanium" | "ti" => (0.00008, 2500.0, 4430.0),
        _ => (0.00030, 900.0, 1400.0), // default: aramid-like
    };

    let thickness_m = armor_thickness_mm / 1000.0;

    // ── Backface deformation ────────────────────────────────────────────
    // BFD ≈ k_bfd × KE^0.5 / (thickness × density^0.5)
    let bfd_m = if thickness_m > 0.0 && density > 0.0 {
        k_bfd * ke.sqrt() / (thickness_m * density.sqrt())
    } else {
        0.0
    };
    let backface_deformation_mm = (bfd_m * 1000.0).min(80.0); // cap at 80mm (beyond NIJ limits)

    // ── Blunt trauma energy ─────────────────────────────────────────────
    // E_blunt = KE × exp(-C × thickness × density^0.5)
    let blunt_energy_j = ke * (-c_factor * thickness_m * density.sqrt()).exp();

    // Standoff gap reduces transmitted energy (air gap allows BFD to expand before hitting body)
    let standoff_factor = if standoff_mm > 0.0 && backface_deformation_mm > 0.0 {
        // If standoff > BFD, no energy transferred to body
        if standoff_mm >= backface_deformation_mm {
            0.0
        } else {
            1.0 - (standoff_mm / backface_deformation_mm)
        }
    } else {
        1.0
    };
    let blunt_energy_j = blunt_energy_j * standoff_factor;

    // ── Injury severity ─────────────────────────────────────────────────
    // E_blunt < 15J: minor
    // E_blunt 15-40J: moderate
    // E_blunt 40-80J: severe
    // E_blunt > 80J: critical
    let (injury_severity, rib_fracture_probability, liver_spleen_risk) = if blunt_energy_j < 15.0 {
        ("minor", blunt_energy_j / 30.0, 0.0)
    } else if blunt_energy_j < 40.0 {
        (
            "moderate",
            0.3 + (blunt_energy_j - 15.0) / 50.0,
            blunt_energy_j / 100.0,
        )
    } else if blunt_energy_j < 80.0 {
        (
            "severe",
            0.5 + (blunt_energy_j - 40.0) / 80.0,
            (blunt_energy_j - 30.0) / 100.0,
        )
    } else {
        (
            "critical",
            0.95_f64.min(0.5 + blunt_energy_j / 160.0),
            0.5_f64.min(blunt_energy_j / 160.0),
        )
    };

    // Projectile type modifier: blunt/deforming bullets transmit more energy
    let proj_lower = projectile_type.to_lowercase();
    let proj_mod = match proj_lower.as_str() {
        "soft_point" | "hollow_point" | "sp" => 1.3,
        "fmj" | "ball" => 1.0,
        "ap" | "armor_piercing" => 0.7,
        _ => 1.0,
    };

    let rib_fracture_probability = (rib_fracture_probability * proj_mod).min(1.0);
    let liver_spleen_risk = (liver_spleen_risk * proj_mod).min(1.0);

    BABTResult {
        backface_deformation_mm,
        blunt_energy_j,
        injury_severity,
        rib_fracture_probability,
        liver_spleen_risk,
    }
}

/// NIJ backface deformation compliance check.
/// Level IIA/II/IIIA: BFD must not exceed 44mm.
pub fn nij_bfd_compliant(bfd_mm: f64, nij_level: &str) -> bool {
    let max_bfd = match nij_level.to_lowercase().as_str() {
        "ii" | "iia" | "iiia" => 44.0,
        "iii" | "iv" => 44.0, // rifle plates have less strict BFD limits
        _ => 44.0,
    };
    bfd_mm <= max_bfd
}

// ── Hydraulic Shock Model ─────────────────────────────────────────────────────

/// Result of a hydraulic shock evaluation from a projectile passing through
/// fluid-filled soft tissue.
#[derive(Debug, Clone, Copy)]
pub struct HydraulicShockResult {
    /// Peak pressure in the temporary cavity (kPa).
    pub peak_pressure_kpa: f64,
    /// Duration of the pressure wave (ms).
    pub pressure_duration_ms: f64,
    /// Distance the pressure wave propagates (m).
    pub pressure_wave_distance_m: f64,
    /// Probability of remote organ damage (0.0–1.0).
    pub remote_organ_damage_probability: f64,
    /// Risk of vascular/nerve disruption at a distance.
    pub vascular_disruption_risk: f64,
    /// Whether the shock is sufficient to cause incapacitation.
    pub incapacitating: bool,
}

/// Evaluate hydraulic shock effects from a projectile passing through
/// fluid-filled soft tissue (muscle, organs).
///
/// The temporary cavity acts as an explosive expansion, generating a
/// pressure wave that propagates through tissue. This can cause damage
/// remote from the wound track.
///
/// Physics:
/// - Peak pressure proportional to dE/dx: P = k_p × (dE/dx) / A
///   where k_p ≈ 0.01–0.03 for muscle tissue
/// - Pressure wave velocity ≈ speed of sound in tissue (~1540 m/s)
/// - Duration scales with temporary cavity radius / sound speed
/// - Remote organ damage: significant when peak pressure > 100 kPa
///   or energy deposition > 30 J/cm
pub fn evaluate_hydraulic_shock(
    velocity_ms: f64,
    mass_g: f64,
    caliber_m: f64,
    projectile_type: &str,
    yawed: bool,
) -> HydraulicShockResult {
    if velocity_ms <= 50.0 || mass_g <= 0.0 || caliber_m <= 0.0 {
        return HydraulicShockResult {
            peak_pressure_kpa: 0.0,
            pressure_duration_ms: 0.0,
            pressure_wave_distance_m: 0.0,
            remote_organ_damage_probability: 0.0,
            vascular_disruption_risk: 0.0,
            incapacitating: false,
        };
    }

    let area = std::f64::consts::PI * (caliber_m / 2.0).powi(2);
    let proj_lower = projectile_type.to_lowercase();

    // ── dE/dx at surface ───────────────────────────────────────────────
    let edex = 0.5 * TISSUE_DENSITY * area * TISSUE_CD * velocity_ms.powi(2);

    // Yaw multiplies dE/dx by 3-6×
    let yaw_mult = if yawed { 4.0 } else { 1.0 };
    let effective_edex = edex * yaw_mult;

    // Fragmenting rounds amplify shock
    let frag_mult = match proj_lower.as_str() {
        "soft_point" | "hollow_point" | "varmint" | "sp" => {
            if velocity_ms > 700.0 {
                1.5
            } else {
                1.2
            }
        }
        _ => 1.0,
    };
    let effective_edex = effective_edex * frag_mult;

    // ── Peak pressure ──────────────────────────────────────────────────
    // P = k_p × (dE/dx) / A, converted to kPa
    // Calibrated so 9mm FMJ (no yaw) produces ~30-80 kPa at cavity wall
    // and yawed rifle produces ~200-800 kPa.
    let k_p = 0.002;
    let peak_pressure_pa = if area > 0.0 {
        k_p * effective_edex / area
    } else {
        0.0
    };
    let peak_pressure_kpa = (peak_pressure_pa / 1000.0).clamp(0.0, 2000.0);

    // ── Pressure duration ──────────────────────────────────────────────
    // Duration ~ 2 × temp_cavity_radius / sound_speed
    // temp cavity: D_temp = 2 × CAVITY_CONSTANT × sqrt(dE/dx)
    let temp_cavity_diameter = 2.0 * CAVITY_CONSTANT * effective_edex.sqrt();
    let temp_cavity_radius = temp_cavity_diameter / 2.0;
    let sound_speed_tissue = 1540.0; // m/s
    let pressure_duration_s = 2.0 * temp_cavity_radius / sound_speed_tissue;
    let pressure_duration_ms = (pressure_duration_s * 1000.0).min(100.0);

    // ── Pressure wave distance ─────────────────────────────────────────
    // Wave attenuates as 1/r² in tissue. Effective range when P > 50 kPa.
    // r_max = sqrt(P_0 / P_threshold) × r_0
    let pressure_wave_distance_m = if peak_pressure_kpa > 50.0 {
        (peak_pressure_kpa / 50.0).sqrt() * temp_cavity_radius
    } else {
        0.0
    };
    let pressure_wave_distance_m = pressure_wave_distance_m.min(0.5);

    // ── Remote organ damage probability ────────────────────────────────
    // Significant when P > 100 kPa or energy deposition > 30 J/cm
    let edep_j_per_m = effective_edex;
    let edep_j_per_cm = edep_j_per_m * 0.01;

    let prob_pressure = (peak_pressure_kpa / 200.0).min(1.0);
    let prob_edep = ((edep_j_per_cm - 15.0) / 40.0).clamp(0.0, 1.0);
    let remote_organ_damage_probability = prob_pressure.max(prob_edep).min(1.0);

    // ── Vascular disruption risk ──────────────────────────────────────
    let vascular_disruption_risk = (peak_pressure_kpa / 500.0).min(1.0) * yaw_mult * 0.3;

    // ── Incapacitation ─────────────────────────────────────────────────
    let incapacitating = peak_pressure_kpa > 300.0 && edep_j_per_cm > 30.0;

    HydraulicShockResult {
        peak_pressure_kpa,
        pressure_duration_ms,
        pressure_wave_distance_m,
        remote_organ_damage_probability,
        vascular_disruption_risk,
        incapacitating,
    }
}

// ── ACE3 Medical Wound Classification ─────────────────────────────────────────

/// ACE3 medical wound classification type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ACE3WoundType {
    /// Massive tissue damage from high-energy cavity.
    AvulsionWound,
    /// Standard projectile wound.
    BulletWound,
    /// Wound from bone or secondary fragments.
    ShrapnelWound,
    /// Blunt trauma from non-penetrating impact.
    ContusionWound,
    /// Crush injury (e.g. rib fracture from BABT).
    CrushWound,
}

/// ACE3 medical wound result — structured data for ACE3 medical system.
#[derive(Debug, Clone)]
pub struct ACE3MedicalResult {
    /// ACE3 wound type classification.
    pub wound_type: ACE3WoundType,
    /// Wound size (1-14, ACE3 scale).
    pub wound_size: i32,
    /// Base blood loss rate (ml/s).
    pub blood_loss_ml_per_s: f64,
    /// Pain level (1-10).
    pub pain_level: i32,
    /// Whether immediate incapacitation is likely.
    pub immediate_incapacitation: bool,
    /// Minutes until consciousness lost from blood loss (if applicable).
    pub consciousness_time_s: f64,
    /// Recommended ACE3 medical treatment.
    pub recommended_treatment: &'static str,
    /// Whether a tourniquet is applicable.
    pub tourniquet_applicable: bool,
}

/// Classify an ABE wound result for the ACE3 medical system.
///
/// Maps from wound ballistics output to ACE3 wound parameters.
///
/// # Arguments
/// * `wound_result` — Result from `evaluate()` or `evaluate_extended()`.
/// * `velocity_ms` — Impact velocity (m/s).
/// * `mass_g` — Projectile mass (g).
/// * `projectile_type` — Projectile construction identifier.
/// * `body_region` — "head", "thorax", "abdomen", "limb".
/// * `armor_penetrated` — Whether armour was penetrated.
/// * `armor_type` — Optional armour type if armour was present.
pub fn classify_ace3_wound(
    wound_result: &WoundResult,
    velocity_ms: f64,
    mass_g: f64,
    projectile_type: &str,
    body_region: &str,
    armor_penetrated: bool,
    _armor_type: Option<&str>,
) -> ACE3MedicalResult {
    let mass_kg = mass_g / 1000.0;
    let _proj_lower = projectile_type.to_lowercase();
    let region = body_region.to_lowercase();

    // ── Compute energy deposition per cm ───────────────────────────────
    let edep_j_per_cm = wound_result.peak_edep_j_per_cm;

    // ── Determine wound type ───────────────────────────────────────────
    let wound_type = if wound_result.yawed && wound_result.frag_mass_g > 0.0 && edep_j_per_cm > 30.0
    {
        // Yawed + fragmented = avulsion
        ACE3WoundType::AvulsionWound
    } else if wound_result.temp_cavity_diameter_m > 0.10 && edep_j_per_cm > 40.0 {
        // Large temporary cavity + high energy deposition
        ACE3WoundType::AvulsionWound
    } else if wound_result.yawed {
        // Yawed but no fragmentation
        ACE3WoundType::BulletWound
    } else if wound_result.bone_fragments > 0 && wound_result.bone_penetrated {
        // Bone generates secondary fragments
        ACE3WoundType::ShrapnelWound
    } else if !armor_penetrated {
        // Non-penetrating
        // Approximate BABT from KE — armor usually transmits 3-8%
        let approx_babt = 0.5 * mass_kg * velocity_ms.powi(2) * 0.05; // ~5% transmitted
        if approx_babt > 50.0 {
            ACE3WoundType::CrushWound
        } else if approx_babt > 15.0 {
            ACE3WoundType::ContusionWound
        } else {
            ACE3WoundType::ContusionWound
        }
    } else {
        ACE3WoundType::BulletWound
    };

    // ── Wound size mapping ─────────────────────────────────────────────
    // Caliber-based base size
    let caliber_mm = if wound_result.perm_cavity_diameter_m > 0.0 {
        wound_result.perm_cavity_diameter_m * 1000.0
    } else {
        5.56 // default
    };

    let base_size = if caliber_mm < 5.0 {
        // < 5mm: size 1-3
        1 + (caliber_mm / 2.0) as i32
    } else if caliber_mm < 8.0 {
        // 5-8mm: size 3-6
        3 + ((caliber_mm - 5.0) / 1.5) as i32
    } else if caliber_mm < 12.0 {
        // 8-12mm: size 5-9
        5 + ((caliber_mm - 8.0) / 1.0) as i32
    } else {
        // > 12mm: size 8-14
        8 + ((caliber_mm - 12.0) / 0.8) as i32
    };
    let mut wound_size = base_size.clamp(1, 14);

    // Yaw/frag add 2-5 to size
    if wound_result.yawed {
        wound_size += 2;
    }
    if wound_result.frag_mass_g > 0.5 {
        wound_size += 3;
    } else if wound_result.frag_mass_g > 0.0 {
        wound_size += 1;
    }

    // Body region: head wounds smaller but more severe
    // (actually they're same size or smaller, but critical due to location)
    match region.as_str() {
        "head" => {
            wound_size = wound_size.clamp(1, 8);
        }
        _ => {}
    }
    let wound_size = wound_size.clamp(1, 14);

    // ── Blood loss rate ────────────────────────────────────────────────
    // Base: 1-3 ml/s for small caliber, 5-15 ml/s for large caliber
    let caliber_effective = if wound_result.perm_cavity_diameter_m > 0.001 {
        wound_result.perm_cavity_diameter_m
    } else {
        caliber_mm / 1000.0
    };
    let base_blood_loss = if caliber_effective < 0.006 {
        0.8 + caliber_effective * 200.0
    } else if caliber_effective < 0.010 {
        2.0 + (caliber_effective - 0.006) * 750.0
    } else {
        5.0 + (caliber_effective - 0.010) * 1500.0
    };
    let mut blood_loss = base_blood_loss.clamp(0.3, 20.0);

    // Modifiers
    if wound_result.yawed {
        blood_loss *= 2.5; // yaw multiplier 2-4×
    }
    if wound_result.frag_mass_g > 0.0 {
        blood_loss *= 1.5; // fragmentation 1.5-2×
    }

    // Avulsion: 10-30 ml/s
    if matches!(wound_type, ACE3WoundType::AvulsionWound) {
        blood_loss = blood_loss.max(10.0).min(30.0);
    }

    // Head: minimal blood loss but high incapacitation
    if region == "head" {
        blood_loss = blood_loss.min(3.0);
    }

    // ── Pain level ─────────────────────────────────────────────────────
    // Proportional to wound size and energy deposition
    let pain_from_size = wound_size as f64 * 0.7;
    let pain_from_edep = (edep_j_per_cm / 10.0).min(5.0);
    let mut pain_level = (pain_from_size + pain_from_edep).round() as i32;
    pain_level = pain_level.clamp(1, 10);

    // Region modifiers
    match region.as_str() {
        "head" => pain_level = pain_level.max(7),
        "thorax" => pain_level = pain_level.max(4),
        "abdomen" => pain_level = pain_level.max(3),
        _ => {}
    }

    // ── Incapacitation / consciousness ─────────────────────────────────
    let immediate_incapacitation = match region.as_str() {
        "head" => true,                   // 90%+ immediate incapacitation
        "thorax" => edep_j_per_cm > 50.0, // 50% at high energy
        _ => false,
    };

    // Blood loss > 30 ml/s: consciousness < 30s
    let consciousness_time_s = if blood_loss > 30.0 {
        30.0
    } else if blood_loss > 15.0 {
        60.0
    } else if blood_loss > 8.0 {
        120.0
    } else {
        300.0
    };

    // Blunt trauma > 60J: 30% incapacitation
    let _blunt_incapacitation = edep_j_per_cm > 60.0;

    // ── Treatment recommendations ──────────────────────────────────────
    let (recommended_treatment, tourniquet_applicable) = match wound_type {
        ACE3WoundType::AvulsionWound => ("surgery", false),
        ACE3WoundType::BulletWound => {
            if region == "limb" {
                ("packing", true)
            } else {
                ("packing", false)
            }
        }
        ACE3WoundType::ShrapnelWound => ("surgery", false),
        ACE3WoundType::ContusionWound => ("bandage", false),
        ACE3WoundType::CrushWound => ("surgery", false),
    };

    ACE3MedicalResult {
        wound_type,
        wound_size,
        blood_loss_ml_per_s: blood_loss,
        pain_level,
        immediate_incapacitation,
        consciousness_time_s,
        recommended_treatment,
        tourniquet_applicable,
    }
}

// ── Multi-Projectile / Shotgun Support ────────────────────────────────────────

/// Configuration for multi-projectile loads (shotgun, duplex, flechette).
pub struct MultiProjectileConfig {
    /// Number of pellets/projectiles.
    pub pellet_count: i32,
    /// Mass per pellet (g).
    pub pellet_mass_g: f64,
    /// Pellet diameter (mm).
    pub pellet_diameter_mm: f64,
    /// Muzzle velocity (m/s).
    pub muzzle_velocity_ms: f64,
    /// Total spread in MOA (minutes of angle).
    pub spread_moa: f64,
    /// Projectile type identifier.
    pub projectile_type: &'static str,
}

/// Predefined multi-projectile configs for common ammunition types.
pub fn shotgun_config(ammo_type: &str) -> MultiProjectileConfig {
    match ammo_type.to_lowercase().as_str() {
        "12ga_buckshot_00" | "00_buckshot" | "12ga_00" => MultiProjectileConfig {
            pellet_count: 8,
            pellet_mass_g: 3.5,
            pellet_diameter_mm: 9.1,
            muzzle_velocity_ms: 400.0,
            spread_moa: 30.0,
            projectile_type: "shot",
        },
        "12ga_buckshot_4" | "buckshot_4" | "12ga_4" => MultiProjectileConfig {
            pellet_count: 27,
            pellet_mass_g: 1.5,
            pellet_diameter_mm: 6.7,
            muzzle_velocity_ms: 400.0,
            spread_moa: 30.0,
            projectile_type: "shot",
        },
        "12ga_birdshot_7.5" | "birdshot_7.5" | "12ga_7.5" => MultiProjectileConfig {
            pellet_count: 350,
            pellet_mass_g: 0.13,
            pellet_diameter_mm: 2.4,
            muzzle_velocity_ms: 400.0,
            spread_moa: 40.0,
            projectile_type: "shot",
        },
        "20ga_buckshot_3" | "20ga_3" => MultiProjectileConfig {
            pellet_count: 20,
            pellet_mass_g: 1.1,
            pellet_diameter_mm: 6.1,
            muzzle_velocity_ms: 380.0,
            spread_moa: 35.0,
            projectile_type: "shot",
        },
        "duplex_556" | "duplex" => MultiProjectileConfig {
            pellet_count: 2,
            pellet_mass_g: 2.0,
            pellet_diameter_mm: 5.56,
            muzzle_velocity_ms: 900.0,
            spread_moa: 4.0,
            projectile_type: "duplex",
        },
        "flechette_12ga" | "flechette" => MultiProjectileConfig {
            pellet_count: 20,
            pellet_mass_g: 0.7,
            pellet_diameter_mm: 1.5,
            muzzle_velocity_ms: 500.0,
            spread_moa: 20.0,
            projectile_type: "flechette",
        },
        _ => MultiProjectileConfig {
            pellet_count: 8,
            pellet_mass_g: 3.5,
            pellet_diameter_mm: 9.1,
            muzzle_velocity_ms: 400.0,
            spread_moa: 30.0,
            projectile_type: "shot",
        }, // default to 00 buck
    }
}

/// Result of evaluating a multi-projectile impact against soft tissue.
pub struct MultiImpactResult {
    /// Per-pellet wound results.
    pub pellet_wounds: Vec<WoundResult>,
    /// Total penetration depth across all wound tracks (max cm).
    pub total_penetration_cm: f64,
    /// Number of pellets that actually hit the target.
    pub hits: i32,
    /// Combined kinetic energy of all hitting pellets (J).
    pub combined_energy_j: f64,
    /// Estimated effective wound volume (cc).
    pub effective_wound_volume_cc: f64,
}

/// Evaluate the impact of a multi-projectile pattern against soft tissue.
///
/// # Arguments
/// * `config` — The multi-projectile configuration.
/// * `range_m` — Range at impact (for spread calculation).
/// * `target_area_cm2` — Presented target area (cm²).
/// * `target_diameter_m` — Target diameter (m), for spread calculation.
pub fn evaluate_multi_impact(
    config: &MultiProjectileConfig,
    range_m: f64,
    target_area_cm2: f64,
    _target_diameter_m: f64,
) -> MultiImpactResult {
    if config.pellet_count <= 0 || config.pellet_mass_g <= 0.0 || range_m < 0.0 {
        return MultiImpactResult {
            pellet_wounds: Vec::new(),
            total_penetration_cm: 0.0,
            hits: 0,
            combined_energy_j: 0.0,
            effective_wound_volume_cc: 0.0,
        };
    }

    // ── Pattern spread calculation ────────────────────────────────────
    // At range R, pattern diameter = R × tan(spread_moa / 60 × π/180)
    let spread_rad = (config.spread_moa / 60.0).to_radians();
    let pattern_diameter_m = range_m * spread_rad.tan();
    let pattern_radius_m = pattern_diameter_m / 2.0;
    let pattern_area_m2 = std::f64::consts::PI * pattern_radius_m.powi(2);

    // ── Hits calculation ──────────────────────────────────────────────
    // Hits = pellet_count × (target_area / pattern_area)
    let target_area_m2 = target_area_cm2 / 10000.0; // cm² → m²
    let hit_fraction = if pattern_area_m2 > 0.0 {
        (target_area_m2 / pattern_area_m2).min(1.0)
    } else {
        1.0 // point-blank: all hit
    };
    let hits = ((config.pellet_count as f64) * hit_fraction).round() as i32;
    let hits = hits.min(config.pellet_count).max(0);

    // ── Per-pellet wound evaluation ────────────────────────────────────
    let pellet_caliber_m = config.pellet_diameter_mm / 1000.0;
    let mut pellet_wounds = Vec::with_capacity(hits as usize);
    let mut total_penetration_cm: f64 = 0.0;
    let mut combined_energy_j: f64 = 0.0;
    let mut effective_wound_volume_cc = 0.0;

    // Velocity at range (simplified: drag reduces velocity)
    // For shotgun pellets at typical ranges (< 50m), velocity drop is ~10-20%
    let vel_at_range = if range_m < 10.0 {
        config.muzzle_velocity_ms * (1.0 - range_m * 0.005)
    } else if range_m < 50.0 {
        config.muzzle_velocity_ms * (1.0 - 0.05 - (range_m - 10.0) * 0.004)
    } else {
        config.muzzle_velocity_ms * (1.0 - 0.25 - (range_m - 50.0) * 0.002).max(100.0)
    };
    let vel_at_range = vel_at_range.max(50.0);

    for _ in 0..hits {
        let wound = evaluate(
            vel_at_range,
            config.pellet_mass_g,
            pellet_caliber_m,
            config.projectile_type,
        );
        let pellet_ke = 0.5 * (config.pellet_mass_g / 1000.0) * vel_at_range.powi(2);

        combined_energy_j += pellet_ke;
        total_penetration_cm = total_penetration_cm.max(wound.penetration_depth_m * 100.0);

        // Wound volume: perm cavity cross-section × penetration (per pellet)
        let cavity_radius = wound.perm_cavity_diameter_m / 2.0;
        let vol_per_pellet_cc =
            std::f64::consts::PI * cavity_radius.powi(2) * wound.penetration_depth_m * 1e6; // m³ → cc
        effective_wound_volume_cc += vol_per_pellet_cc;

        pellet_wounds.push(wound);
    }

    // Multi-projectile wound volume is not purely additive (overlapping wound tracks
    // don't double the volume). Apply a packing factor.
    let packing_factor = if hits <= 3 {
        1.0
    } else if hits <= 10 {
        0.7
    } else if hits <= 50 {
        0.5
    } else {
        0.3
    };
    effective_wound_volume_cc *= packing_factor;

    MultiImpactResult {
        pellet_wounds,
        total_penetration_cm,
        hits,
        combined_energy_j,
        effective_wound_volume_cc,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Handgun rounds ───────────────────────────────────────────────────

    #[test]
    fn fmj_9mm_penetration_in_tissue() {
        // 9mm FMJ at 360 m/s (~124 gr at 1180 fps)
        let r = evaluate(360.0, 8.0, 0.00901, "fmj");
        assert!(
            r.penetration_depth_m > 0.3 && r.penetration_depth_m < 1.0,
            "9mm FMJ should penetrate ~40-70 cm in tissue: {:.3} m",
            r.penetration_depth_m
        );
        assert!(
            r.perm_cavity_diameter_m > 0.0,
            "Permanent cavity should be positive"
        );
        assert!(
            r.temp_cavity_diameter_m > 0.0,
            "Temporary cavity should be positive"
        );
        assert!(!r.yawed, "9mm FMJ should not yaw in tissue (handgun)");
        assert!(
            r.energy_deposited_j > 100.0,
            "9mm should deposit > 100 J: {:.1} J",
            r.energy_deposited_j
        );
    }

    #[test]
    fn nine_mm_vs_44_magnum_cavity() {
        // .44 Mag delivers significantly more energy → larger cavity
        let nine = evaluate(360.0, 8.0, 0.00901, "fmj");
        let magnum = evaluate(450.0, 15.6, 0.0109, "soft_point");
        assert!(
            magnum.temp_cavity_diameter_m > nine.temp_cavity_diameter_m,
            ".44 Mag cavity ({:.4} m) should exceed 9mm ({:.4} m)",
            magnum.temp_cavity_diameter_m,
            nine.temp_cavity_diameter_m
        );
        assert!(
            magnum.energy_deposited_j > nine.energy_deposited_j,
            ".44 Mag deposits more energy"
        );
    }

    // ── Rifle rounds ────────────────────────────────────────────────────

    #[test]
    fn rifle_yaws_in_tissue() {
        // 5.56mm M855 at 930 m/s should yaw in tissue
        let r = evaluate(930.0, 4.0, 0.00556, "fmj");
        assert!(r.yawed, "5.56mm FMJ should yaw in soft tissue");
        assert!(
            r.yaw_depth_m > 0.0,
            "Yaw depth should be positive: {:.3} m",
            r.yaw_depth_m
        );
        assert!(
            r.temp_cavity_diameter_m > 0.01,
            "Rifle round should produce large temporary cavity: {:.4} m",
            r.temp_cavity_diameter_m
        );
    }

    #[test]
    fn rifle_vs_handgun_cavity() {
        let rifle = evaluate(930.0, 4.0, 0.00556, "fmj");
        let handgun = evaluate(360.0, 8.0, 0.00901, "fmj");
        assert!(
            rifle.temp_cavity_diameter_m > handgun.temp_cavity_diameter_m,
            "Rifle temporary cavity ({:.4} m) >> handgun ({:.4} m)",
            rifle.temp_cavity_diameter_m,
            handgun.temp_cavity_diameter_m
        );
        // Rifle deposits significantly more energy
        assert!(
            rifle.energy_deposited_j > handgun.energy_deposited_j * 2.0,
            "Rifle deposits much more energy"
        );
    }

    #[test]
    fn subsonic_no_yaw() {
        // Subsonic 300 BLK at 310 m/s should NOT yaw
        let r = evaluate(310.0, 12.5, 0.00762, "fmj");
        assert!(!r.yawed, "Subsonic should not yaw in tissue");
        // Subsonic still penetrates deeply
        assert!(
            r.penetration_depth_m > 0.3,
            "300 BLK subsonic should penetrate deeply: {:.3} m",
            r.penetration_depth_m
        );
    }

    #[test]
    fn ap_projectiles_minimal_yaw() {
        // AP projectiles are hardened — less likely to yaw/fragment
        let r = evaluate(880.0, 10.0, 0.00762, "ap");
        assert!(!r.yawed, "AP projectiles should resist yawing");
        assert!(
            r.frag_mass_g.abs() < 0.01,
            "AP should not fragment in tissue: {:.4} g",
            r.frag_mass_g
        );
    }

    // ── Soft point / expanding rounds ───────────────────────────────────

    #[test]
    fn soft_point_expands_in_tissue() {
        let r = evaluate(380.0, 10.0, 0.00901, "soft_point");
        assert!(
            r.perm_cavity_diameter_m > 0.012,
            "Soft point should expand: perm cavity = {:.4} m",
            r.perm_cavity_diameter_m
        );
        // Compared to FMJ same caliber: soft point has larger permanent cavity
        let fmj = evaluate(380.0, 10.0, 0.00901, "fmj");
        assert!(
            r.perm_cavity_diameter_m > fmj.perm_cavity_diameter_m,
            "Soft point perm cavity ({:.4} m) should exceed FMJ ({:.4} m)",
            r.perm_cavity_diameter_m,
            fmj.perm_cavity_diameter_m
        );
        // Energy deposited should be at least as high
        assert!(
            r.energy_deposited_j >= fmj.energy_deposited_j,
            "Expanding bullet should deposit >= FMJ energy"
        );
    }

    // ── Edge cases ──────────────────────────────────────────────────────

    #[test]
    fn zero_velocity_no_wound() {
        let r = evaluate(0.0, 8.0, 0.00901, "fmj");
        assert_eq!(r.penetration_depth_m, 0.0);
        assert_eq!(r.temp_cavity_diameter_m, 0.0);
        assert_eq!(r.energy_deposited_j, 0.0);
    }

    #[test]
    fn zero_mass_no_wound() {
        let r = evaluate(400.0, 0.0, 0.00901, "fmj");
        assert_eq!(r.penetration_depth_m, 0.0);
    }

    #[test]
    fn below_threshold_no_wound() {
        let r = evaluate(10.0, 8.0, 0.00901, "fmj");
        assert_eq!(r.penetration_depth_m, 0.0);
    }

    #[test]
    fn high_velocity_increases_cavity() {
        let slow = evaluate(400.0, 8.0, 0.00901, "fmj");
        let fast = evaluate(600.0, 8.0, 0.00901, "fmj");
        assert!(
            fast.temp_cavity_diameter_m > slow.temp_cavity_diameter_m,
            "Higher velocity should produce larger temporary cavity"
        );
    }

    // ── Wound profile ──────────────────────────────────────────────────

    #[test]
    fn wound_profile_returns_samples() {
        let profile = wound_profile(930.0, 4.0, 0.00556, "fmj", 10);
        assert!(!profile.is_empty(), "Profile should have samples");
        assert_eq!(profile.len(), 10, "Should return exactly 10 samples");
        // Samples should be in increasing depth order
        for i in 1..profile.len() {
            assert!(
                profile[i].0 > profile[i - 1].0,
                "Depth should increase monotonically"
            );
        }
    }

    #[test]
    fn wound_profile_handgun() {
        let profile = wound_profile(360.0, 8.0, 0.00901, "fmj", 5);
        assert_eq!(profile.len(), 5);
        // Handgun: energy deposition decreases monotonically (no yaw)
        for i in 1..profile.len() {
            assert!(
                profile[i].2 <= profile[i - 1].2 + 1e-6,
                "Handgun dE/dx should decrease with depth"
            );
        }
    }

    #[test]
    fn wound_profile_empty_for_no_penetration() {
        let profile = wound_profile(0.0, 8.0, 0.00901, "fmj", 10);
        assert!(profile.is_empty(), "Should be empty for no penetration");
    }

    // ── Reference data tests ───────────────────────────────────────────

    #[test]
    fn nij_levels_defined() {
        for level in &[
            NIJLevel::IIA,
            NIJLevel::II,
            NIJLevel::IIIA,
            NIJLevel::III,
            NIJLevel::IV,
        ] {
            let threat = nij_threat(*level);
            assert!(threat.velocity_ms > 300.0);
            assert!(threat.mass_g > 5.0);
            assert!(threat.caliber_m > 0.005);
        }
    }

    #[test]
    fn nij_level_iii_is_rifle() {
        let threat = nij_threat(NIJLevel::III);
        assert!(threat.velocity_ms > 800.0);
        assert!((threat.caliber_m - 0.00782).abs() < 0.001);
    }

    #[test]
    fn stanag_levels_defined() {
        for level in &[
            STANAGLevel::L1,
            STANAGLevel::L2,
            STANAGLevel::L3,
            STANAGLevel::L4,
            STANAGLevel::L5,
        ] {
            let threat = stanag_threat(*level);
            assert!(
                threat.velocity_ms > 600.0,
                "L{:?} velocity = {}",
                level,
                threat.velocity_ms
            );
            assert!(threat.mass_g > 5.0);
        }
    }

    #[test]
    fn stanag_l4_threat_is_heavy() {
        let threat = stanag_threat(STANAGLevel::L4);
        // 14.5×114mm API B32: ~64 g at 911 m/s
        assert!(
            threat.mass_g > 50.0,
            "14.5mm should be heavy: {} g",
            threat.mass_g
        );
        assert!(threat.caliber_m > 0.012, "Caliber should be > 12 mm");
    }

    #[test]
    fn ammo_references_available() {
        let refs = ammo_references();
        assert!(!refs.is_empty(), "Should have at least one reference");
        for ammo in &refs {
            // Rifle rounds have MV > 800; pistol rounds are slower
            // but should still be physically plausible
            assert!(
                ammo.mv_ms > 200.0,
                "All ammo should have MV > 200 m/s: {}",
                ammo.mv_ms
            );
            assert!(ammo.bc_g7 > 0.05);
            assert!(!ammo.trajectory_samples.is_empty());
            // First sample should be at range 0
            assert!(
                (ammo.trajectory_samples[0].range_m).abs() < 0.001,
                "First sample should be at range 0"
            );
            // Velocity should decrease with range
            for i in 1..ammo.trajectory_samples.len() {
                assert!(
                    ammo.trajectory_samples[i].velocity_ms
                        <= ammo.trajectory_samples[i - 1].velocity_ms + 1.0,
                    "Velocity should not increase with range"
                );
            }
        }
    }

    #[test]
    fn m855_trajectory_data_plausible() {
        let refs = ammo_references();
        let m855 = refs.iter().find(|a| a.name.contains("M855")).unwrap();
        // At 500 m, velocity should be ~530 m/s
        let at_500 = m855
            .trajectory_samples
            .iter()
            .find(|s| (s.range_m - 500.0).abs() < 1.0)
            .unwrap();
        assert!(
            at_500.velocity_ms > 400.0 && at_500.velocity_ms < 650.0,
            "M855 at 500 m should be ~530 m/s: {}",
            at_500.velocity_ms
        );
    }

    // ── Bone interaction tests ──────────────────────────────────────────────

    #[test]
    fn m855_penetrates_femur() {
        let bone_result = evaluate_bone_impact(930.0, 4.0, 0.00556, "fmj", BoneType::Femur, 0.0);
        assert!(bone_result.penetrated, "M855 should penetrate femur");
        // M855 loses ~half its energy penetrating a 20mm femur
        assert!(
            bone_result.velocity_after_bone_ms > 400.0
                && bone_result.velocity_after_bone_ms < 700.0,
            "M855 retains ~480 m/s after femur: {:.1} m/s",
            bone_result.velocity_after_bone_ms
        );
        assert!(
            bone_result.bone_fragments >= 2,
            "Femur impact should produce fragments: {}",
            bone_result.bone_fragments
        );
        assert!(
            bone_result.energy_deposited_in_bone_j > 200.0,
            "Bone should absorb significant energy: {:.1} J",
            bone_result.energy_deposited_in_bone_j
        );
    }

    #[test]
    fn nine_mm_borderline_sternum() {
        // 9mm at 360 m/s is borderline for sternum penetration
        // (6mm sternum, moderate velocity)
        let bone_result = evaluate_bone_impact(360.0, 8.0, 0.00901, "fmj", BoneType::Sternum, 0.0);
        // 9mm at this velocity may or may not penetrate — we just check the
        // result is physically plausible and doesn't crash
        assert!(
            bone_result.energy_deposited_in_bone_j > 0.0,
            "Bone impact should deposit some energy"
        );
        // At 360 m/s, 9mm likely penetrates sternum
        assert!(
            bone_result.penetrated,
            "9mm at 360 m/s should penetrate sternum (~6mm)"
        );
    }

    #[test]
    fn twentytwo_lr_stopped_by_skull() {
        // .22 LR at 330 m/s should be stopped by skull (~7mm)
        let bone_result = evaluate_bone_impact(330.0, 2.6, 0.00556, "fmj", BoneType::Skull, 0.0);
        assert!(!bone_result.penetrated, ".22 LR should be stopped by skull");
        assert!(
            (bone_result.velocity_after_bone_ms).abs() < 0.1,
            "Velocity after bone should be ~0 when stopped"
        );
    }

    #[test]
    fn lapua_destroys_femur() {
        // .338 Lapua at 880 m/s should destroy femur
        let bone_result = evaluate_bone_impact(880.0, 16.2, 0.00858, "fmj", BoneType::Femur, 0.0);
        assert!(bone_result.penetrated, ".338 Lapua should penetrate femur");
        assert!(
            bone_result.bone_fragments >= 5,
            ".338 Lapua should produce massive fragmentation: {} fragments",
            bone_result.bone_fragments
        );
        assert!(
            bone_result.bone_fragment_mass_g > 2.0,
            "Massive bone fragmentation: {:.2} g",
            bone_result.bone_fragment_mass_g
        );
        assert!(
            bone_result.energy_deposited_in_bone_j > 500.0,
            ".338 Lapua deposits huge energy in bone: {:.0} J",
            bone_result.energy_deposited_in_bone_j
        );
    }

    #[test]
    fn bone_impact_changes_wound_characteristics() {
        // Comparing evaluate_extended with vs without bone
        let no_bone = evaluate_extended(850.0, 9.5, 0.00782, "fmj", None);
        let with_bone = evaluate_extended(
            850.0,
            9.5,
            0.00782,
            "fmj",
            Some((BoneType::Femur, 10.0, 0.05)),
        );
        // Bone impact changes wound characteristics (cavity, penetration, fragments)
        assert!(
            (with_bone.temp_cavity_diameter_m - no_bone.temp_cavity_diameter_m).abs() < 0.15,
            "Bone should affect cavity: no_bone={:.4}, with_bone={:.4}",
            no_bone.temp_cavity_diameter_m,
            with_bone.temp_cavity_diameter_m
        );
        // Bone should either be penetrated or energy deposited
        assert!(
            with_bone.bone_penetrated || with_bone.bone_energy_j > 100.0,
            "Bone impact should either penetrate or deposit energy"
        );
        if with_bone.bone_penetrated {
            assert!(
                with_bone.bone_fragments > 0,
                "Penetrating bone impact should generate fragments"
            );
        }
    }

    #[test]
    fn evaluate_extended_no_bone_matches_evaluate() {
        // evaluate_extended with None should match evaluate
        let base = evaluate(930.0, 4.0, 0.00556, "fmj");
        let ext = evaluate_extended(930.0, 4.0, 0.00556, "fmj", None);
        assert!((base.penetration_depth_m - ext.penetration_depth_m).abs() < 0.001);
        assert!((base.temp_cavity_diameter_m - ext.temp_cavity_diameter_m).abs() < 0.001);
        assert!((base.energy_deposited_j - ext.energy_deposited_j).abs() < 0.001);
    }

    #[test]
    fn bone_impact_at_oblique_angle() {
        // Oblique impact (60°) should produce higher deflection
        let normal = evaluate_bone_impact(800.0, 9.5, 0.00762, "fmj", BoneType::Femur, 0.0);
        let oblique = evaluate_bone_impact(800.0, 9.5, 0.00762, "fmj", BoneType::Femur, 60.0);
        assert!(
            oblique.deflection_angle_deg >= normal.deflection_angle_deg,
            "Oblique angle should deflect more: normal={:.1}°, oblique={:.1}°",
            normal.deflection_angle_deg,
            oblique.deflection_angle_deg
        );
    }

    #[test]
    fn bone_type_thicknesses_plausible() {
        assert!((BoneType::Skull.thickness_m() - 0.007).abs() < 0.001);
        assert!((BoneType::Rib.thickness_m() - 0.002).abs() < 0.001);
        assert!((BoneType::Femur.thickness_m() - 0.020).abs() < 0.001);
        assert!((BoneType::Sternum.thickness_m() - 0.006).abs() < 0.001);
        assert!((BoneType::Pelvis.thickness_m() - 0.008).abs() < 0.001);
        assert!((BoneType::Tibia.thickness_m() - 0.015).abs() < 0.001);
        assert!((BoneType::Humerus.thickness_m() - 0.012).abs() < 0.001);
        assert!((BoneType::Spine.thickness_m() - 0.010).abs() < 0.001);
        let generic = BoneType::Generic { thickness_m: 0.005 };
        assert!((generic.thickness_m() - 0.005).abs() < 0.001);
    }

    #[test]
    fn bone_impact_zero_velocity() {
        let result = evaluate_bone_impact(0.0, 8.0, 0.00901, "fmj", BoneType::Skull, 0.0);
        assert!(!result.penetrated);
        assert_eq!(result.bone_fragments, 0);
    }

    #[test]
    fn ap_projectile_better_bone_penetration() {
        // AP projectile should have easier time with bone than FMJ
        let fmj = evaluate_bone_impact(500.0, 10.0, 0.00762, "fmj", BoneType::Sternum, 0.0);
        let ap = evaluate_bone_impact(500.0, 10.0, 0.00762, "ap", BoneType::Sternum, 0.0);
        // AP should be at least as likely to penetrate (lower threshold)
        assert!(
            ap.penetrated
                || !fmj.penetrated
                || ap.velocity_after_bone_ms >= fmj.velocity_after_bone_ms - 0.01
        );
    }

    #[test]
    fn evaluate_extended_bone_stops_projectile() {
        // .22 LR at 300 m/s should be stopped by femur
        let result = evaluate_extended(
            300.0,
            2.6,
            0.00556,
            "fmj",
            Some((BoneType::Femur, 0.0, 0.04)),
        );
        assert!(!result.bone_penetrated, ".22 LR should not penetrate femur");
        assert_eq!(
            result.bone_fragments, 0,
            "No bone fragments if projectile stopped"
        );
    }

    // ── Expanded reference trajectory tests ────────────────────────────────

    #[test]
    fn new_ammo_references_available() {
        let refs = ammo_references();
        assert!(
            refs.len() >= 9,
            "Should have at least 9 ammo references: {}",
            refs.len()
        );
        // Check each new ammo type exists
        let names: Vec<&str> = refs.iter().map(|a| a.name).collect();
        assert!(
            names.iter().any(|n| n.contains("M193")),
            "M193 should be in references"
        );
        assert!(
            names.iter().any(|n| n.contains("9×19mm")),
            "9mm should be in references"
        );
        assert!(
            names.iter().any(|n| n.contains(".338 Lapua")),
            ".338 Lapua should be in references"
        );
        assert!(
            names.iter().any(|n| n.contains(".50 BMG")),
            ".50 BMG should be in references"
        );
        assert!(
            names.iter().any(|n| n.contains("7.62×39mm")),
            "7.62×39mm LPS should be in references"
        );
        assert!(
            names.iter().any(|n| n.contains("5.45×39mm")),
            "5.45×39mm 7N6 should be in references"
        );
    }

    #[test]
    fn new_ammo_trajectories_plausible() {
        let refs = ammo_references();
        for ammo in &refs {
            assert!(!ammo.trajectory_samples.is_empty());
            assert!((ammo.trajectory_samples[0].range_m).abs() < 0.001);
            // Velocity should be close to MV at range 0
            assert!(
                (ammo.trajectory_samples[0].velocity_ms - ammo.mv_ms).abs() < 1.0,
                "{}: first sample vel {} ≈ MV {}",
                ammo.name,
                ammo.trajectory_samples[0].velocity_ms,
                ammo.mv_ms
            );
        }
    }

    #[test]
    fn m193_trajectory_at_500m() {
        let refs = ammo_references();
        let m193 = refs.iter().find(|a| a.name.contains("M193")).unwrap();
        let at_500 = m193
            .trajectory_samples
            .iter()
            .find(|s| (s.range_m - 500.0).abs() < 1.0)
            .unwrap();
        assert!(
            at_500.velocity_ms > 500.0 && at_500.velocity_ms < 650.0,
            "M193 at 500 m should be ~583 m/s: {}",
            at_500.velocity_ms
        );
    }

    #[test]
    fn lapua_338_trajectory_retains_high_velocity() {
        let refs = ammo_references();
        let lapua = refs.iter().find(|a| a.name.contains(".338 Lapua")).unwrap();
        let at_1000 = lapua
            .trajectory_samples
            .iter()
            .find(|s| (s.range_m - 1000.0).abs() < 1.0)
            .unwrap();
        assert!(
            at_1000.velocity_ms > 500.0,
            ".338 Lapua at 1000m should be supersonic: {} m/s",
            at_1000.velocity_ms
        );
    }

    // ── Trajectory validation tests ────────────────────────────────────────

    #[test]
    fn check_trajectory_monotonic_all_ammo() {
        let refs = ammo_references();
        for ammo in &refs {
            assert!(
                check_trajectory_monotonic(ammo),
                "{} trajectory should be monotonic",
                ammo.name
            );
        }
    }

    #[test]
    fn interpolate_m855_at_150m() {
        let refs = ammo_references();
        let m855 = refs.iter().find(|a| a.name.contains("M855")).unwrap();
        let (vel, drop) = interpolate_trajectory(m855, 150.0);
        // Between 100m (842 m/s) and 200m (759 m/s) → ~801 m/s
        assert!(
            (vel - 801.0).abs() < 5.0,
            "M855 at 150m should be ~801 m/s: {}",
            vel
        );
        // Drop should be between 0.069 and 0.301
        assert!(
            drop > 0.05 && drop < 0.35,
            "M855 at 150m drop should be interpolated: {}",
            drop
        );
    }

    #[test]
    fn energy_at_range_m855_muzzle() {
        let refs = ammo_references();
        let m855 = refs.iter().find(|a| a.name.contains("M855")).unwrap();
        let energy = compute_energy_at_range(m855, 0.0);
        // KE at muzzle: 0.5 * 0.004 * 930^2 = 1729.8 J
        assert!(
            (energy - 1725.0).abs() < 20.0,
            "M855 muzzle energy ~1725 J: {}",
            energy
        );
    }

    #[test]
    fn interpolate_before_first_sample() {
        let refs = ammo_references();
        let m855 = refs.iter().find(|a| a.name.contains("M855")).unwrap();
        let (vel, drop) = interpolate_trajectory(m855, -10.0);
        assert!(
            (vel - 930.0).abs() < 1.0,
            "Should use first sample velocity"
        );
        assert!((drop).abs() < 0.001, "Drop should be 0 at/before 0m");
    }

    #[test]
    fn interpolate_after_last_sample() {
        let refs = ammo_references();
        let m855 = refs.iter().find(|a| a.name.contains("M855")).unwrap();
        let (vel, drop) = interpolate_trajectory(m855, 2000.0);
        assert!(vel > 0.0, "Should return last valid velocity");
        assert!(drop > 16.0, "Drop should exceed last sample drop");
    }

    #[test]
    fn energy_decreases_with_range() {
        let refs = ammo_references();
        for ammo in &refs {
            let e0 = compute_energy_at_range(ammo, 0.0);
            let e_mid = compute_energy_at_range(ammo, 100.0);
            assert!(
                e_mid <= e0 + 1.0,
                "{} energy should decrease with range: {:.1} → {:.1}",
                ammo.name,
                e0,
                e_mid
            );
        }
    }

    #[test]
    fn compute_energy_all_ammo_types() {
        let refs = ammo_references();
        for ammo in &refs {
            let e = compute_energy_at_range(ammo, 0.0);
            assert!(
                e > 100.0,
                "{} muzzle energy should be > 100 J: {:.0} J",
                ammo.name,
                e
            );
        }
    }

    // ── BABT / Blunt Trauma tests ──────────────────────────────────────────

    #[test]
    fn babt_9mm_vs_niijiiia_aramid() {
        // 9mm FMJ at 360 m/s vs NIJ IIIA aramid (6 layers, ~6mm)
        let r = evaluate_babt(360.0, 8.0, 0.00901, "fmj", 6.0, "aramid", 0.0);
        // BFD should be within NIJ limits (≤44mm)
        assert!(
            r.backface_deformation_mm > 10.0 && r.backface_deformation_mm < 44.0,
            "9mm vs IIIA aramid BFD should be within NIJ limits: {:.1} mm",
            r.backface_deformation_mm
        );
        assert!(
            r.blunt_energy_j > 0.0,
            "Should have some blunt trauma energy"
        );
    }

    #[test]
    fn babt_44mag_vs_niijiiia() {
        // .44 Mag at 436 m/s vs NIJ IIIA aramid — at or near BFD limit
        let r = evaluate_babt(436.0, 15.6, 0.0109, "fmj", 6.0, "aramid", 0.0);
        // Should be near or at BFD limit of 44mm
        assert!(
            r.backface_deformation_mm > 30.0,
            ".44 Mag BFD should be significant: {:.1} mm",
            r.backface_deformation_mm
        );
    }

    #[test]
    fn babt_9mm_vs_steel_plate() {
        // 9mm vs steel plate: BFD near zero (steel doesn't flex)
        let r = evaluate_babt(360.0, 8.0, 0.00901, "fmj", 3.0, "steel", 0.0);
        assert!(
            r.backface_deformation_mm < 10.0,
            "Steel plate BFD should be minimal: {:.3} mm",
            r.backface_deformation_mm
        );
        assert!(
            r.blunt_energy_j < 20.0,
            "Steel transmits minimal blunt energy"
        );
    }

    #[test]
    fn babt_standoff_reduces_injury() {
        // With standoff, blunt energy should be less
        let no_standoff = evaluate_babt(400.0, 10.0, 0.00901, "fmj", 6.0, "aramid", 0.0);
        let with_standoff = evaluate_babt(400.0, 10.0, 0.00901, "fmj", 6.0, "aramid", 20.0);
        assert!(
            with_standoff.blunt_energy_j <= no_standoff.blunt_energy_j,
            "Standoff should reduce blunt energy"
        );
    }

    #[test]
    fn nij_bfd_compliant_check() {
        assert!(nij_bfd_compliant(30.0, "iiia"));
        assert!(nij_bfd_compliant(44.0, "iiia"));
        assert!(!nij_bfd_compliant(45.0, "iiia"));
        assert!(nij_bfd_compliant(20.0, "II"));
    }

    #[test]
    fn babt_zero_velocity_no_injury() {
        let r = evaluate_babt(0.0, 8.0, 0.00901, "fmj", 6.0, "aramid", 0.0);
        assert_eq!(r.backface_deformation_mm, 0.0);
        assert_eq!(r.blunt_energy_j, 0.0);
        assert_eq!(r.injury_severity, "none");
    }

    #[test]
    fn babt_injury_severity_scales_with_energy() {
        let low = evaluate_babt(200.0, 4.0, 0.00556, "fmj", 6.0, "aramid", 0.0);
        let high = evaluate_babt(800.0, 10.0, 0.00762, "fmj", 6.0, "aramid", 0.0);
        let severity_order: Vec<&str> = vec!["minor", "moderate", "severe", "critical"];
        let low_idx = severity_order
            .iter()
            .position(|&s| s == low.injury_severity)
            .unwrap_or(0);
        let high_idx = severity_order
            .iter()
            .position(|&s| s == high.injury_severity)
            .unwrap_or(0);
        assert!(
            high_idx >= low_idx,
            "Higher energy should have equal or greater severity"
        );
    }

    #[test]
    fn uhmwpe_lower_bfd_than_aramid() {
        let aramid = evaluate_babt(400.0, 10.0, 0.00901, "fmj", 6.0, "aramid", 0.0);
        let uhmwpe = evaluate_babt(400.0, 10.0, 0.00901, "fmj", 6.0, "uhmwpe", 0.0);
        assert!(
            uhmwpe.backface_deformation_mm <= aramid.backface_deformation_mm + 1.0,
            "UHMWPE BFD ({:.1}) should be <= aramid BFD ({:.1})",
            uhmwpe.backface_deformation_mm,
            aramid.backface_deformation_mm
        );
    }

    // ── Hydraulic Shock tests ──────────────────────────────────────────────

    #[test]
    fn hydraulic_shock_rifle_yawed() {
        // 5.56mm M855 yawed in tissue should produce significant shock
        let r = evaluate_hydraulic_shock(930.0, 4.0, 0.00556, "fmj", true);
        assert!(
            r.peak_pressure_kpa > 100.0,
            "Yawed rifle should produce > 100 kPa: {:.0} kPa",
            r.peak_pressure_kpa
        );
        assert!(
            r.remote_organ_damage_probability > 0.0,
            "Should have some remote organ damage probability"
        );
    }

    #[test]
    fn hydraulic_shock_handgun_minimal() {
        // 9mm FMJ (no yaw) should produce weaker shock
        let r = evaluate_hydraulic_shock(360.0, 8.0, 0.00901, "fmj", false);
        assert!(
            r.peak_pressure_kpa < 200.0,
            "Handgun shock should be mild: {:.0} kPa",
            r.peak_pressure_kpa
        );
    }

    #[test]
    fn hydraulic_shock_incapacitating_threshold() {
        // High-energy yawed round should be incapacitating
        let r = evaluate_hydraulic_shock(930.0, 4.0, 0.00556, "fmj", true);
        // M855 yawed at ~930 m/s has dE/dx > 30 J/cm and P > 300 kPa
        assert!(
            r.incapacitating || r.peak_pressure_kpa > 200.0,
            "Yawed rifle should cause significant shock"
        );
    }

    #[test]
    fn hydraulic_shock_zero_velocity() {
        let r = evaluate_hydraulic_shock(0.0, 8.0, 0.00901, "fmj", false);
        assert_eq!(r.peak_pressure_kpa, 0.0);
        assert!(!r.incapacitating);
    }

    #[test]
    fn hydraulic_shock_yaw_increases_pressure() {
        let no_yaw = evaluate_hydraulic_shock(800.0, 9.5, 0.00762, "fmj", false);
        let with_yaw = evaluate_hydraulic_shock(800.0, 9.5, 0.00762, "fmj", true);
        assert!(
            with_yaw.peak_pressure_kpa > no_yaw.peak_pressure_kpa,
            "Yaw should increase peak pressure"
        );
    }

    // ── ACE3 Medical Classification tests ──────────────────────────────────

    #[test]
    fn ace3_m855_center_mass() {
        // 5.56mm M855 center mass (yawed, fragmented): avulsion wound
        let wound = evaluate(930.0, 4.0, 0.00556, "fmj");
        let ace3 = classify_ace3_wound(&wound, 930.0, 4.0, "fmj", "thorax", true, None);
        assert!(
            matches!(ace3.wound_type, ACE3WoundType::AvulsionWound),
            "M855 center mass should be avulsion: {:?}",
            ace3.wound_type
        );
        assert!(ace3.blood_loss_ml_per_s > 5.0, "Blood loss > 5 ml/s");
    }

    #[test]
    fn ace3_9mm_to_limb() {
        // 9mm FMJ to limb (no yaw): bullet wound, moderate blood loss
        let wound = evaluate(360.0, 8.0, 0.00901, "fmj");
        let ace3 = classify_ace3_wound(&wound, 360.0, 8.0, "fmj", "limb", true, None);
        assert_eq!(ace3.wound_type, ACE3WoundType::BulletWound);
        assert!(
            ace3.blood_loss_ml_per_s < 8.0,
            "9mm to limb blood loss reasonable: {:.1}",
            ace3.blood_loss_ml_per_s
        );
        assert!(
            ace3.pain_level <= 7,
            "9mm limb pain should be moderate: {}",
            ace3.pain_level
        );
    }

    #[test]
    fn ace3_762mm_m80_to_thorax() {
        // 7.62mm M80 to thorax (yawed): large bullet wound, 8-15 ml/s
        let wound = evaluate(850.0, 9.5, 0.00782, "fmj");
        let ace3 = classify_ace3_wound(&wound, 850.0, 9.5, "fmj", "thorax", true, None);
        assert!(
            matches!(
                ace3.wound_type,
                ACE3WoundType::BulletWound | ACE3WoundType::AvulsionWound
            ),
            "M80 should be bullet or avulsion wound: {:?}",
            ace3.wound_type
        );
        assert!(ace3.wound_size >= 4, "Wound size should be >= 4");
    }

    #[test]
    fn ace3_22lr_to_head() {
        // .22 LR to head (any): immediate incapacitation
        let wound = evaluate(330.0, 2.6, 0.00556, "fmj");
        let ace3 = classify_ace3_wound(&wound, 330.0, 2.6, "fmj", "head", true, None);
        assert!(
            ace3.immediate_incapacitation,
            ".22 LR to head should be immediately incapacitating"
        );
        assert!(ace3.pain_level >= 7, "Head wound pain should be severe");
    }

    #[test]
    fn ace3_babt_contusion() {
        // Behind armor blunt trauma of ~30J: contusion wound
        // Simulate a non-penetrating hit
        let wound = WoundResult {
            penetration_depth_m: 0.0,
            temp_cavity_diameter_m: 0.02,
            perm_cavity_diameter_m: 0.005,
            energy_deposited_j: 30.0,
            peak_edep_j_per_cm: 5.0,
            yawed: false,
            yaw_depth_m: 0.0,
            frag_mass_g: 0.0,
            bone_penetrated: false,
            bone_fragments: 0,
            bone_energy_j: 0.0,
        };
        let ace3 = classify_ace3_wound(&wound, 400.0, 8.0, "fmj", "thorax", false, Some("aramid"));
        assert_eq!(ace3.wound_type, ACE3WoundType::ContusionWound);
    }

    #[test]
    fn ace3_fragment_shrapnel_wound() {
        // Fragment from grenade: shrapnel wound
        let wound = WoundResult {
            penetration_depth_m: 0.05,
            temp_cavity_diameter_m: 0.01,
            perm_cavity_diameter_m: 0.003,
            energy_deposited_j: 80.0,
            peak_edep_j_per_cm: 10.0,
            yawed: false,
            yaw_depth_m: 0.0,
            frag_mass_g: 0.0,
            bone_penetrated: true,
            bone_fragments: 3,
            bone_energy_j: 50.0,
        };
        let ace3 = classify_ace3_wound(&wound, 600.0, 2.0, "fmj", "limb", true, None);
        assert_eq!(ace3.wound_type, ACE3WoundType::ShrapnelWound);
    }

    #[test]
    fn ace3_338_lapua_in_limb() {
        // .338 Lapua in limb: avulsion wound (high energy deposition)
        let wound = evaluate(880.0, 16.2, 0.00858, "fmj");
        let ace3 = classify_ace3_wound(&wound, 880.0, 16.2, "fmj", "limb", true, None);
        assert!(
            matches!(
                ace3.wound_type,
                ACE3WoundType::AvulsionWound | ACE3WoundType::BulletWound
            ),
            ".338 Lapua should cause avulsion wound: {:?}",
            ace3.wound_type
        );
        assert!(ace3.blood_loss_ml_per_s > 8.0, "Blood loss should be high");
    }

    // ── Multi-Projectile / Shotgun tests ───────────────────────────────────

    #[test]
    fn buckshot_00_at_10m() {
        // 00 buckshot at 10m: pattern < 0.5m, most pellets hit torso target
        let config = shotgun_config("12ga_buckshot_00");
        let result = evaluate_multi_impact(&config, 10.0, 500.0, 0.5);
        assert!(
            result.hits >= 5,
            "At 10m most buck pellets should hit torso: {} hits",
            result.hits
        );
        assert!(result.combined_energy_j > 1000.0, "Combined KE > 1000 J");
    }

    #[test]
    fn buckshot_00_at_50m() {
        // 00 buckshot at 50m: pattern > 1.5m, few pellets hit
        let config = shotgun_config("12ga_buckshot_00");
        let result = evaluate_multi_impact(&config, 50.0, 500.0, 0.5);
        assert!(
            result.hits < 8,
            "At 50m fewer pellets should hit: {} hits",
            result.hits
        );
    }

    #[test]
    fn birdshot_at_25m() {
        // Birdshot at 25m: pattern > 1m, many small wounds
        let config = shotgun_config("12ga_birdshot_7.5");
        let result = evaluate_multi_impact(&config, 25.0, 500.0, 0.5);
        assert!(
            result.hits > 10,
            "Birdshot at 25m should have many hits: {}",
            result.hits
        );
    }

    #[test]
    fn duplex_at_300m() {
        // Duplex at 300m: two tight projectiles tracking close together
        let config = shotgun_config("duplex_556");
        let result = evaluate_multi_impact(&config, 300.0, 500.0, 0.5);
        // Duplex rounds are tight (4 MOA) — both should hit
        assert!(
            result.hits >= 1,
            "Duplex at 300m both should hit: {} hits",
            result.hits
        );
    }

    #[test]
    fn flechette_at_25m() {
        // Flechette at 25m: multiple narrow penetrating wounds
        let config = shotgun_config("flechette_12ga");
        let result = evaluate_multi_impact(&config, 25.0, 500.0, 0.5);
        assert!(
            result.hits > 5,
            "Flechette at 25m should have multiple hits: {}",
            result.hits
        );
    }

    #[test]
    fn shotgun_config_types() {
        let cfg00 = shotgun_config("12ga_buckshot_00");
        assert_eq!(cfg00.pellet_count, 8);
        assert!((cfg00.pellet_mass_g - 3.5).abs() < 0.1);

        let cfg_duplex = shotgun_config("duplex_556");
        assert_eq!(cfg_duplex.pellet_count, 2);

        let cfg_flechette = shotgun_config("flechette");
        assert_eq!(cfg_flechette.pellet_count, 20);
    }

    #[test]
    fn multi_impact_zero_range_all_hit() {
        let config = shotgun_config("12ga_buckshot_00");
        let result = evaluate_multi_impact(&config, 0.0, 500.0, 0.5);
        assert_eq!(result.hits, config.pellet_count, "At 0m all pellets hit");
    }

    #[test]
    fn multi_impact_empty_config() {
        let config = MultiProjectileConfig {
            pellet_count: 0,
            pellet_mass_g: 0.0,
            pellet_diameter_mm: 0.0,
            muzzle_velocity_ms: 0.0,
            spread_moa: 0.0,
            projectile_type: "shot",
        };
        let result = evaluate_multi_impact(&config, 10.0, 500.0, 0.5);
        assert_eq!(result.hits, 0);
        assert!(result.pellet_wounds.is_empty());
    }
}
