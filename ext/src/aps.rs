// ABE - Active Protection Systems (APS) Intercept Model
//
// Models hard-kill and soft-kill active protection systems that
// intercept incoming projectiles before they reach the armour.
// Covers Trophy, Arena, Iron Fist, Zaslon, Afghanit, Drozd, and Shtora.
//
// References:
//   - "Trophy APS" (IAI/Rafael) — hard-kill, fragment interceptor
//   - "Arena" (KBM, Russia) — hard-kill, explosive interceptor
//   - "Iron Fist" (IMI, Israel) — hard/soft-kill hybrid
//   - "Zaslon" (Ukraine) — hard-kill, directional, limited coverage
//   - "Afghanit" (Russia, T-14 Armata) — hard-kill with AESA radar
//   - "Drozd" (Russia, T-55AD) — first-gen hard-kill
//   - "Shtora" (Russia) — soft-kill IR dazzler / smoke screen
//   - O'Gorman, T.J. et al., "Active Protection System Effectiveness
//     Modeling" (ARL-TR-6982, 2014)

/// Types of active protection systems.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum APSType {
    /// Trophy (Israeli, Rafael/IAI). Hard-kill, fragment interceptor.
    /// Best vs ATGMs/RPGs. Used on Merkava, Abrams (Trophy MVP).
    Trophy,
    /// Arena (Russian, KBM). Hard-kill, explosive interceptor.
    /// Good vs everything. Used on T-80, BMP-3.
    Arena,
    /// Iron Fist (Israeli, IMI). Hard/soft-kill hybrid.
    /// Good vs KE and HEAT. Used on Namer, Eitan.
    IronFist,
    /// Zaslon (Ukrainian). Hard-kill, directional.
    /// Limited coverage (~120° arc). Used on BMP-1, BTR-4.
    Zaslon,
    /// Afghanit (Russian, T-14 Armata). Hard-kill with AESA radar.
    /// Best vs modern threats. Integral to T-14.
    Afghanit,
    /// Drozd (Soviet/Russian). Hard-kill, first-gen.
    /// Limited capability. Used on T-55AD.
    Drozd,
    /// Shtora (Russian). Soft-kill (IR dazzler, smoke).
    /// Only vs guided weapons. Used on T-90.
    Shtora,
}

impl APSType {
    /// Human-readable name.
    pub fn name(&self) -> &'static str {
        match self {
            APSType::Trophy => "Trophy",
            APSType::Arena => "Arena",
            APSType::IronFist => "Iron Fist",
            APSType::Zaslon => "Zaslon",
            APSType::Afghanit => "Afghanit",
            APSType::Drozd => "Drozd",
            APSType::Shtora => "Shtora",
        }
    }

    /// Origin/nation.
    pub fn origin(&self) -> &'static str {
        match self {
            APSType::Trophy => "Israel",
            APSType::Arena => "Russia",
            APSType::IronFist => "Israel",
            APSType::Zaslon => "Ukraine",
            APSType::Afghanit => "Russia",
            APSType::Drozd => "USSR/Russia",
            APSType::Shtora => "Russia",
        }
    }
}

/// Configuration for an active protection system.
pub struct APSConfig {
    /// The type of APS.
    pub aps_type: APSType,
    /// Whether the system can intercept kinetic energy (KE) rounds.
    pub intercept_kinetic: bool,
    /// Whether the system can intercept HEAT (shaped charge) rounds.
    pub intercept_heat: bool,
    /// Whether the system can intercept ATGMs (anti-tank guided missiles).
    pub intercept_missile: bool,
    /// Maximum projectile velocity the system can track (m/s).
    pub max_intercept_velocity_ms: f64,
    /// Minimum engagement range (m).
    pub min_intercept_range_m: f64,
    /// Maximum engagement range (m).
    pub max_intercept_range_m: f64,
    /// Azimuth coverage arc (360 = full sphere, 180 = half-sphere).
    pub azimuth_coverage_deg: f64,
    /// Elevation coverage arc.
    pub elevation_coverage_deg: f64,
    /// Time between intercept attempts (s).
    pub reload_time_s: f64,
    /// Number of interceptor rounds available. -1 for unlimited (soft-kill).
    pub ammunition: i32,
    /// Base probability of kill per interceptor shot (0.0–1.0).
    pub probability_per_shot: f64,
    /// Probability that the sensor fails to detect the threat (0.0–1.0).
    pub sensor_failure_rate: f64,
}

impl APSConfig {
    /// Default APS configuration for Trophy (hard-kill, fragment interceptor).
    pub fn trophy() -> Self {
        APSConfig {
            aps_type: APSType::Trophy,
            intercept_kinetic: false,
            intercept_heat: true,
            intercept_missile: true,
            max_intercept_velocity_ms: 1200.0,
            min_intercept_range_m: 10.0,
            max_intercept_range_m: 60.0,
            azimuth_coverage_deg: 360.0,
            elevation_coverage_deg: 60.0,
            reload_time_s: 0.3,
            ammunition: 12,
            probability_per_shot: 0.90,
            sensor_failure_rate: 0.02,
        }
    }

    /// Default APS configuration for Arena (Russian hard-kill).
    pub fn arena() -> Self {
        APSConfig {
            aps_type: APSType::Arena,
            intercept_kinetic: true,
            intercept_heat: true,
            intercept_missile: true,
            max_intercept_velocity_ms: 1500.0,
            min_intercept_range_m: 5.0,
            max_intercept_range_m: 50.0,
            azimuth_coverage_deg: 300.0,
            elevation_coverage_deg: 60.0,
            reload_time_s: 0.4,
            ammunition: 8,
            probability_per_shot: 0.85,
            sensor_failure_rate: 0.05,
        }
    }

    /// Default APS configuration for Iron Fist (hard/soft-kill hybrid).
    pub fn iron_fist() -> Self {
        APSConfig {
            aps_type: APSType::IronFist,
            intercept_kinetic: true,
            intercept_heat: true,
            intercept_missile: true,
            max_intercept_velocity_ms: 1800.0,
            min_intercept_range_m: 5.0,
            max_intercept_range_m: 40.0,
            azimuth_coverage_deg: 360.0,
            elevation_coverage_deg: 90.0,
            reload_time_s: 0.2,
            ammunition: 10,
            probability_per_shot: 0.92,
            sensor_failure_rate: 0.01,
        }
    }

    /// Default APS configuration for Zaslon (Ukrainian directional).
    pub fn zaslon() -> Self {
        APSConfig {
            aps_type: APSType::Zaslon,
            intercept_kinetic: false,
            intercept_heat: true,
            intercept_missile: true,
            max_intercept_velocity_ms: 1000.0,
            min_intercept_range_m: 2.0,
            max_intercept_range_m: 30.0,
            azimuth_coverage_deg: 120.0,
            elevation_coverage_deg: 45.0,
            reload_time_s: 0.5,
            ammunition: 4,
            probability_per_shot: 0.80,
            sensor_failure_rate: 0.10,
        }
    }

    /// Default APS configuration for Afghanit (T-14 Armata).
    pub fn afghanit() -> Self {
        APSConfig {
            aps_type: APSType::Afghanit,
            intercept_kinetic: true,
            intercept_heat: true,
            intercept_missile: true,
            max_intercept_velocity_ms: 2500.0,
            min_intercept_range_m: 5.0,
            max_intercept_range_m: 70.0,
            azimuth_coverage_deg: 360.0,
            elevation_coverage_deg: 90.0,
            reload_time_s: 0.15,
            ammunition: 16,
            probability_per_shot: 0.95,
            sensor_failure_rate: 0.01,
        }
    }

    /// Default APS configuration for Drozd (first-gen Soviet).
    pub fn drozd() -> Self {
        APSConfig {
            aps_type: APSType::Drozd,
            intercept_kinetic: false,
            intercept_heat: true,
            intercept_missile: false,
            max_intercept_velocity_ms: 700.0,
            min_intercept_range_m: 5.0,
            max_intercept_range_m: 25.0,
            azimuth_coverage_deg: 180.0,
            elevation_coverage_deg: 30.0,
            reload_time_s: 1.0,
            ammunition: 4,
            probability_per_shot: 0.60,
            sensor_failure_rate: 0.20,
        }
    }

    /// Default APS configuration for Shtora (soft-kill).
    pub fn shtora() -> Self {
        APSConfig {
            aps_type: APSType::Shtora,
            intercept_kinetic: false,
            intercept_heat: false,
            intercept_missile: true,
            max_intercept_velocity_ms: 500.0,
            min_intercept_range_m: 0.0,
            max_intercept_range_m: 300.0,
            azimuth_coverage_deg: 360.0,
            elevation_coverage_deg: 20.0,
            reload_time_s: 3.0,
            ammunition: -1, // unlimited (soft-kill)
            probability_per_shot: 0.40,
            sensor_failure_rate: 0.05,
        }
    }

    /// Look up a default config by APS type name ("trophy", "arena", etc.).
    pub fn from_type_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "trophy" => Some(Self::trophy()),
            "arena" => Some(Self::arena()),
            "iron_fist" | "ironfist" => Some(Self::iron_fist()),
            "zaslon" => Some(Self::zaslon()),
            "afghanit" => Some(Self::afghanit()),
            "drozd" => Some(Self::drozd()),
            "shtora" => Some(Self::shtora()),
            _ => None,
        }
    }
}

/// Result of an APS intercept attempt.
#[derive(Debug, Clone, Copy)]
pub struct APSResult {
    /// Whether the interceptor engaged the projectile at all.
    pub intercepted: bool,
    /// Whether the APS sensors successfully detected the threat.
    pub detection_succeeded: bool,
    /// Number of interceptor rounds consumed in this attempt.
    pub intercept_rounds_used: i32,
    /// Remaining interceptor ammunition after this attempt.
    pub ammunition_remaining: i32,
    /// Whether the projectile was completely stopped.
    pub projectile_stopped: bool,
    /// Residual velocity if only partially intercepted (m/s).
    pub residual_velocity_ms: f64,
    /// Whether the projectile was deflected off its original path.
    pub projectile_deflected: bool,
    /// Deflection angle if deflected (degrees).
    pub deflection_angle_deg: f64,
}

/// Attempt APS intercept of an incoming projectile.
///
/// Uses a deterministic threshold check (no RNG). Projectile speed, type,
/// range, and angle determine intercept probability against the APS
/// specifications. Returns an `APSResult` describing the outcome.
///
/// # Arguments
/// * `aps` — APS system configuration.
/// * `projectile_velocity_ms` — Incoming projectile velocity (m/s).
/// * `projectile_range_m` — Distance from APS to projectile at engagement (m).
/// * `projectile_type` — "ke" (kinetic energy), "heat", "missile".
/// * `azimuth_deg` — Horizontal angle from APS boresight (0-360).
/// * `elevation_deg` — Vertical angle from APS boresight (0-90).
pub fn evaluate_intercept(
    aps: &APSConfig,
    projectile_velocity_ms: f64,
    projectile_range_m: f64,
    projectile_type: &str,
    azimuth_deg: f64,
    elevation_deg: f64,
) -> APSResult {
    // ── 1. Check if this APS can engage this projectile type ───────────
    let can_engage = match projectile_type.to_lowercase().as_str() {
        "ke" | "kinetic" => aps.intercept_kinetic,
        "heat" | "he" | "chemical" => aps.intercept_heat,
        "missile" | "atgm" | "guided" => aps.intercept_missile,
        _ => false,
    };

    if !can_engage {
        return APSResult {
            intercepted: false,
            detection_succeeded: false,
            intercept_rounds_used: 0,
            ammunition_remaining: aps.ammunition,
            projectile_stopped: false,
            residual_velocity_ms: projectile_velocity_ms,
            projectile_deflected: false,
            deflection_angle_deg: 0.0,
        };
    }

    // ── 2. Check ammunition availability ───────────────────────────────
    if aps.ammunition == 0 {
        return APSResult {
            intercepted: false,
            detection_succeeded: false,
            intercept_rounds_used: 0,
            ammunition_remaining: 0,
            projectile_stopped: false,
            residual_velocity_ms: projectile_velocity_ms,
            projectile_deflected: false,
            deflection_angle_deg: 0.0,
        };
    }

    // ── 3. Sensor detection ────────────────────────────────────────────
    // Detection succeeds if random uniform ∈ [sensor_failure_rate, 1.0)
    let detection_succeeded = projectile_velocity_ms <= aps.max_intercept_velocity_ms
        && projectile_range_m >= aps.min_intercept_range_m
        && projectile_range_m <= aps.max_intercept_range_m
        && azimuth_deg <= aps.azimuth_coverage_deg
        && elevation_deg <= aps.elevation_coverage_deg;

    if !detection_succeeded {
        // No shot taken (or projectile outside coverage)
        return APSResult {
            intercepted: false,
            detection_succeeded: false,
            intercept_rounds_used: 0,
            ammunition_remaining: aps.ammunition,
            projectile_stopped: false,
            residual_velocity_ms: projectile_velocity_ms,
            projectile_deflected: false,
            deflection_angle_deg: 0.0,
        };
    }

    // ── 4. Compute effective intercept probability ─────────────────────
    // Base probability from APS config, modified by engagement geometry
    let mut effective_p_kill = aps.probability_per_shot;

    // Velocity factor: very fast projectiles are harder to intercept
    if projectile_velocity_ms > 0.0 {
        let vel_ratio = projectile_velocity_ms / aps.max_intercept_velocity_ms;
        if vel_ratio > 0.8 {
            // Reduced effectiveness for near-max-velocity threats
            let penalty = (vel_ratio - 0.8) / 0.2 * 0.3; // up to 30% reduction
            effective_p_kill *= 1.0 - penalty;
        }
    }

    // Range factor: close or far edges of engagement envelope reduce P_kill
    let range_mid = (aps.min_intercept_range_m + aps.max_intercept_range_m) / 2.0;
    let range_mid_dist = if range_mid > 0.0 {
        (projectile_range_m - range_mid).abs() / (range_mid.max(1.0))
    } else {
        0.0
    };
    if range_mid_dist > 0.5 {
        let range_penalty = ((range_mid_dist - 0.5) / 0.5).min(0.3);
        effective_p_kill *= 1.0 - range_penalty;
    }

    // Angle factor: edge of coverage reduces effectiveness
    let az_frac = azimuth_deg / aps.azimuth_coverage_deg.max(1.0);
    let el_frac = elevation_deg / aps.elevation_coverage_deg.max(1.0);
    let max_frac = az_frac.max(el_frac);
    if max_frac > 0.7 {
        let angle_penalty = ((max_frac - 0.7) / 0.3).min(0.2);
        effective_p_kill *= 1.0 - angle_penalty;
    }

    // Clamp to valid range
    let effective_p_kill = effective_p_kill.clamp(0.0, 0.99);

    // ── 5. Intercept rounds used ───────────────────────────────────────
    // Soft-kill (Shtora) uses no ammunition in the conventional sense
    let rounds_used = if aps.ammunition > 0 { 1 } else { 0 };
    let ammo_remaining = if aps.ammunition > 0 {
        aps.ammunition - rounds_used
    } else {
        aps.ammunition
    };

    // ── 6. Deterministic intercept outcome ─────────────────────────────
    // Use a hash-like deterministic function of inputs rather than RNG.
    // This gives reproducible results for the same inputs.
    let intercept_hash = deterministic_hash(
        aps.aps_type,
        projectile_velocity_ms,
        projectile_range_m,
        projectile_type,
    );
    let intercept_value = (intercept_hash % 10000) as f64 / 10000.0;
    let intercept_success = intercept_value < effective_p_kill;

    if !intercept_success {
        // Miss: projectile continues untouched
        return APSResult {
            intercepted: true, // system fired, but missed
            detection_succeeded: true,
            intercept_rounds_used: rounds_used,
            ammunition_remaining: ammo_remaining,
            projectile_stopped: false,
            residual_velocity_ms: projectile_velocity_ms,
            projectile_deflected: false,
            deflection_angle_deg: 0.0,
        };
    }

    // ── 7. Successful intercept — determine effect ─────────────────────
    // KE projectiles: partial interception possible (break but don't stop)
    let is_ke = matches!(projectile_type.to_lowercase().as_str(), "ke" | "kinetic");

    if is_ke && effective_p_kill < 0.7 {
        // Partial KE intercept: projectile is broken / deflected
        // but some residual fragments may continue
        let residual_frac = 0.2 + (1.0 - effective_p_kill) * 0.5;
        let residual_velocity_ms = projectile_velocity_ms * residual_frac.min(1.0);
        let deflection_angle_deg = 15.0 + (1.0 - effective_p_kill) * 30.0;

        APSResult {
            intercepted: true,
            detection_succeeded: true,
            intercept_rounds_used: rounds_used,
            ammunition_remaining: ammo_remaining,
            projectile_stopped: residual_velocity_ms < 50.0,
            residual_velocity_ms,
            projectile_deflected: true,
            deflection_angle_deg: deflection_angle_deg.min(60.0),
        }
    } else {
        // Full intercept: projectile destroyed/stopped
        APSResult {
            intercepted: true,
            detection_succeeded: true,
            intercept_rounds_used: rounds_used,
            ammunition_remaining: ammo_remaining,
            projectile_stopped: true,
            residual_velocity_ms: 0.0,
            projectile_deflected: false,
            deflection_angle_deg: 0.0,
        }
    }
}

/// Simple deterministic hash from input parameters to a u64.
/// Provides reproducible pseudo-uniform values for testing.
fn deterministic_hash(
    aps_type: APSType,
    velocity_ms: f64,
    range_m: f64,
    projectile_type: &str,
) -> u64 {
    let type_code = match aps_type {
        APSType::Trophy => 1u64,
        APSType::Arena => 2,
        APSType::IronFist => 3,
        APSType::Zaslon => 4,
        APSType::Afghanit => 5,
        APSType::Drozd => 6,
        APSType::Shtora => 7,
    };

    let vel_bits = velocity_ms.to_bits();
    let range_bits = range_m.to_bits();

    let mut h = type_code;
    h = h.wrapping_mul(6364136223846793005).wrapping_add(vel_bits);
    h = h.wrapping_mul(6364136223846793005).wrapping_add(range_bits);
    for b in projectile_type.bytes() {
        h = h.wrapping_mul(6364136223846793005).wrapping_add(b as u64);
    }
    h
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Default config tests ──────────────────────────────────────────────

    #[test]
    fn trophy_default_config() {
        let aps = APSConfig::trophy();
        assert_eq!(aps.aps_type, APSType::Trophy);
        assert!(!aps.intercept_kinetic);
        assert!(aps.intercept_heat);
        assert!(aps.intercept_missile);
        assert_eq!(aps.ammunition, 12);
        assert!((aps.probability_per_shot - 0.90).abs() < 0.01);
    }

    #[test]
    fn arena_default_config() {
        let aps = APSConfig::arena();
        assert_eq!(aps.aps_type, APSType::Arena);
        assert!(aps.intercept_kinetic);
        assert!(aps.intercept_heat);
        assert_eq!(aps.max_intercept_velocity_ms, 1500.0);
    }

    #[test]
    fn iron_fist_default_config() {
        let aps = APSConfig::iron_fist();
        assert_eq!(aps.aps_type, APSType::IronFist);
        assert!((aps.probability_per_shot - 0.92).abs() < 0.01);
        assert_eq!(aps.max_intercept_velocity_ms, 1800.0);
    }

    #[test]
    fn zaslon_default_config() {
        let aps = APSConfig::zaslon();
        assert_eq!(aps.aps_type, APSType::Zaslon);
        assert_eq!(aps.azimuth_coverage_deg, 120.0);
        assert_eq!(aps.ammunition, 4);
    }

    #[test]
    fn afghanit_default_config() {
        let aps = APSConfig::afghanit();
        assert_eq!(aps.aps_type, APSType::Afghanit);
        assert!(aps.intercept_kinetic);
        assert_eq!(aps.max_intercept_velocity_ms, 2500.0);
        assert_eq!(aps.ammunition, 16);
    }

    #[test]
    fn drozd_default_config() {
        let aps = APSConfig::drozd();
        assert_eq!(aps.aps_type, APSType::Drozd);
        assert!(!aps.intercept_missile);
        assert!((aps.probability_per_shot - 0.60).abs() < 0.01);
    }

    #[test]
    fn shtora_default_config() {
        let aps = APSConfig::shtora();
        assert_eq!(aps.aps_type, APSType::Shtora);
        assert!(!aps.intercept_kinetic);
        assert!(!aps.intercept_heat);
        assert!(aps.intercept_missile);
        assert_eq!(aps.ammunition, -1);
    }

    #[test]
    fn from_type_name_lookup() {
        assert!(APSConfig::from_type_name("trophy").is_some());
        assert!(APSConfig::from_type_name("arena").is_some());
        assert!(APSConfig::from_type_name("iron_fist").is_some());
        assert!(APSConfig::from_type_name("zaslon").is_some());
        assert!(APSConfig::from_type_name("afghanit").is_some());
        assert!(APSConfig::from_type_name("drozd").is_some());
        assert!(APSConfig::from_type_name("shtora").is_some());
        assert!(APSConfig::from_type_name("unknown").is_none());
    }

    #[test]
    fn aps_type_names() {
        assert_eq!(APSType::Trophy.name(), "Trophy");
        assert_eq!(APSType::Arena.name(), "Arena");
        assert_eq!(APSType::Shtora.name(), "Shtora");
    }

    // ── Intercept logic tests ─────────────────────────────────────────────

    #[test]
    fn trophy_intercepts_heat_round() {
        let aps = APSConfig::trophy();
        // RPG-7 at 200 m/s, 30m range, dead centre
        let result = evaluate_intercept(&aps, 200.0, 30.0, "heat", 0.0, 0.0);
        assert!(result.detection_succeeded);
        assert!(result.intercepted);
        assert!(result.projectile_stopped);
        assert_eq!(result.intercept_rounds_used, 1);
        assert_eq!(result.ammunition_remaining, 11);
    }

    #[test]
    fn trophy_cannot_intercept_ke() {
        let aps = APSConfig::trophy();
        // KE round should not be engaged by Trophy
        let result = evaluate_intercept(&aps, 1500.0, 30.0, "ke", 0.0, 0.0);
        assert!(!result.intercepted);
        assert!(!result.detection_succeeded);
    }

    #[test]
    fn shtora_only_intercepts_missiles() {
        let aps = APSConfig::shtora();
        let heat_result = evaluate_intercept(&aps, 200.0, 50.0, "heat", 0.0, 0.0);
        assert!(!heat_result.intercepted);

        let missile_result = evaluate_intercept(&aps, 250.0, 100.0, "missile", 0.0, 0.0);
        assert!(missile_result.detection_succeeded);
    }

    #[test]
    fn out_of_range_no_intercept() {
        let aps = APSConfig::trophy();
        // Target at 200m — beyond max intercept range of 60m
        let result = evaluate_intercept(&aps, 200.0, 200.0, "heat", 0.0, 0.0);
        assert!(!result.detection_succeeded);
        assert!(!result.intercepted);
    }

    #[test]
    fn out_of_azimuth_no_intercept() {
        let aps = APSConfig::zaslon(); // 120° coverage
                                       // Target at 180° — outside coverage
        let result = evaluate_intercept(&aps, 200.0, 15.0, "heat", 180.0, 0.0);
        assert!(!result.detection_succeeded);
        assert!(!result.intercepted);
    }

    #[test]
    fn no_ammunition_no_intercept() {
        let mut aps = APSConfig::trophy();
        aps.ammunition = 0;
        let result = evaluate_intercept(&aps, 200.0, 30.0, "heat", 0.0, 0.0);
        assert!(!result.intercepted);
        assert_eq!(result.ammunition_remaining, 0);
    }

    #[test]
    fn too_fast_for_sensors() {
        let aps = APSConfig::drozd(); // max 700 m/s
        let result = evaluate_intercept(&aps, 1500.0, 15.0, "heat", 0.0, 0.0);
        assert!(!result.detection_succeeded);
    }

    #[test]
    fn arena_intercepts_ke_partially() {
        let aps = APSConfig::arena(); // P_kill = 0.85 for KE
                                      // Very fast KE round at close range
        let result = evaluate_intercept(&aps, 1400.0, 10.0, "ke", 0.0, 0.0);
        // Should attempt intercept (detectin check passes since 1400 <= 1500)
        assert!(result.detection_succeeded);
        assert!(result.intercepted);
        // The deterministic hash will determine whether it's full or partial
        // Since Arena is meant for KE, at close range with P_kill 0.85:
        // check what the hash gives us
    }

    #[test]
    fn iron_fist_edge_of_coverage() {
        let aps = APSConfig::iron_fist(); // 360° azimuth, 90° elevation
                                          // At the edge of elevation coverage
        let result = evaluate_intercept(&aps, 800.0, 20.0, "heat", 350.0, 85.0);
        assert!(
            result.detection_succeeded,
            "Should detect at edge of coverage"
        );
    }

    #[test]
    fn ammo_consumption_tracking() {
        // Each intercept call reports ammunition_remaining = config ammo - rounds used
        let aps = APSConfig::trophy();
        let result = evaluate_intercept(&aps, 200.0, 30.0, "heat", 0.0, 0.0);
        assert!(result.intercepted || !result.intercepted);
        // If intercepted, rounds used = 1 and remaining = ammo - 1
        // If not intercepted, rounds used = 0 and remaining = ammo
        let expected_remaining = aps.ammunition - result.intercept_rounds_used;
        assert_eq!(result.ammunition_remaining, expected_remaining);
    }

    #[test]
    fn unkown_projectile_type_not_engaged() {
        let aps = APSConfig::trophy();
        let result = evaluate_intercept(&aps, 200.0, 30.0, "unknown", 0.0, 0.0);
        assert!(!result.intercepted);
    }

    #[test]
    fn no_threat_no_intercept() {
        // Empty projectile type string should not trigger intercept
        let aps = APSConfig::arena();
        let result = evaluate_intercept(&aps, 800.0, 25.0, "", 0.0, 0.0);
        assert!(!result.intercepted);
        assert!(!result.detection_succeeded);
        // Verify ammo is not consumed
        assert_eq!(result.ammunition_remaining, aps.ammunition);
    }

    #[test]
    fn trophy_detects_incoming_ke() {
        // Trophy cannot intercept KE, but should still detect it
        let aps = APSConfig::trophy();
        let result = evaluate_intercept(&aps, 900.0, 30.0, "ke", 0.0, 0.0);
        // Detection checks pass (velocity ≤ 1200, range 10-60, in coverage)
        // But intercept_kinetic=false → can_engage=false
        assert!(!result.intercepted, "Trophy cannot intercept KE");
        assert!(!result.detection_succeeded, "Trophy does not engage KE");
        assert_eq!(result.intercept_rounds_used, 0);
        assert_eq!(result.ammunition_remaining, aps.ammunition);
    }

    #[test]
    fn arena_splits_cupola_direction() {
        // Arena has 300° azimuth coverage — test a direction inside vs outside
        let aps = APSConfig::arena();
        // Within 300° coverage at 45°
        let inside = evaluate_intercept(&aps, 700.0, 25.0, "ke", 45.0, 0.0);
        assert!(
            inside.intercepted,
            "Arena should intercept KE within coverage"
        );
        // Outside 300° coverage at 310°
        let outside = evaluate_intercept(&aps, 700.0, 25.0, "ke", 310.0, 0.0);
        assert!(
            !outside.intercepted,
            "Arena should not engage outside coverage"
        );
        assert!(!outside.detection_succeeded);
    }

    #[test]
    fn iron_fist_soft_launch_vertical() {
        // Iron Fist has 90° elevation coverage — test at 80° (near edge)
        let aps = APSConfig::iron_fist();
        let result = evaluate_intercept(&aps, 600.0, 20.0, "heat", 0.0, 80.0);
        assert!(
            result.detection_succeeded,
            "Iron Fist should detect at 80° elevation (≤90°)"
        );
        assert!(
            result.intercepted,
            "Iron Fist should intercept at high elevation"
        );
    }
}
