// ABE - Zero Management Module
//
// Manages zero profiles per weapon×ammunition×shooter combination,
// computes zero corrections when environmental conditions change,
// and provides Maximum Point-Blank Range (MPBR) estimates.
//
// # Concept
//
// Each combination of weapon, ammunition, and shooter has a unique
// zero setting stored at specific environmental conditions. When
// temperature or altitude changes, the zero point shifts:
//
//   - Temperature: +1 °C → effective MV increases by ~0.5 m/s
//     (propellant burns hotter, higher muzzle velocity)
//   - Altitude (500 m): effective MV changes by ~0.7 m/s relative
//     to sea level (thinner air → altered drag + propellant effects)
//
// The module reconciles stored zero data with current conditions
// to produce the angular correction needed to re-zero the weapon.

#![allow(dead_code)]

use crate::sight_height;

/// Broad weapon classification.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WeaponClass {
    Carbine,
    Rifle,
    DMR,
    Sniper,
    LMG,
    Pistol,
    Shotgun,
    Launcher,
}

/// A complete zero profile linking a weapon, its ammunition, and the
/// shooter's precision to the environmental conditions at the time of zeroing.
#[derive(Debug, Clone)]
pub struct ZeroProfile {
    pub profile_id: String,
    pub weapon_class: WeaponClass,
    pub weapon_barrel_length_mm: f64,
    pub ammo_mass_g: f64,
    pub ammo_mv_ms: f64,
    pub ammo_bc: f64,
    pub ammo_cdm: String,
    pub sight_height_mm: f64,
    pub zero_range_m: f64,
    pub shooter_base_moa: f64,
    pub environmental_temp_c: f64,
    pub environmental_altitude_m: f64,
    pub environmental_pressure_kpa: f64,
}

/// The correction required to return the point of aim to the point of
/// impact when environmental conditions differ from the stored zero.
///
/// Positive corrections mean "dial up / dial right":
///   - `correction_moa_vertical > 0` → increase elevation
///   - `correction_moa_horizontal > 0` → add windage (right)
#[derive(Debug, Clone, PartialEq)]
pub struct ZeroCorrection {
    pub profile_id: String,
    /// The effective zero range under current conditions (m).
    pub new_zero_range_m: f64,
    pub correction_moa_vertical: f64,
    pub correction_moa_horizontal: f64,
    pub correction_mrad_vertical: f64,
    pub correction_mrad_horizontal: f64,
    /// Maximum Point-Blank Range under the stored profile (m).
    pub mpbr_m: f64,
}

/// An in-memory store of zero profiles.
#[derive(Debug, Clone)]
pub struct ZeroDatabase {
    pub profiles: Vec<ZeroProfile>,
}

// ── Environmental helpers ──────────────────────────────────────────────────────

/// Temperature coefficient: muzzle velocity change per °C (m/s).
const TEMP_MV_COEFF: f64 = 0.5;

/// Altitude coefficient: effective MV change per 500 m (m/s).
const ALT_MV_COEFF: f64 = 0.7;

/// Gravitational acceleration (m/s²).
const GRAVITY: f64 = 9.806_65;

/// Compute the effective muzzle-velocity shift due to a change in
/// temperature and altitude relative to a reference condition.
///
/// Positive Δtemp → higher MV (hotter propellant).
/// Positive Δalt  → lower effective MV (thinner air reduces propellant
/// efficiency and alters drag; net effect is more apparent drop).
fn effective_mv_shift(delta_temp_c: f64, delta_altitude_m: f64) -> f64 {
    delta_temp_c * TEMP_MV_COEFF - (delta_altitude_m / 500.0) * ALT_MV_COEFF
}

// ── Core API ───────────────────────────────────────────────────────────────────

/// Compute the zero correction needed when the environmental conditions
/// differ from the stored zero profile's conditions.
///
/// The correction is the angular difference between the zero angle at
/// the stored muzzle velocity and the zero angle at the effective MV
/// adjusted for the current temperature and altitude.
///
/// # Returns
/// A `ZeroCorrection` with the angular adjustment to re-zero.
pub fn compute_zero_correction(
    stored: &ZeroProfile,
    current_temp_c: f64,
    current_altitude_m: f64,
) -> ZeroCorrection {
    let delta_temp = current_temp_c - stored.environmental_temp_c;
    let delta_alt = current_altitude_m - stored.environmental_altitude_m;
    let delta_mv = effective_mv_shift(delta_temp, delta_alt);
    let adjusted_mv = stored.ammo_mv_ms + delta_mv;
    let sight_h_m = stored.sight_height_mm / 1000.0;
    let zero_r = stored.zero_range_m;

    let stored_angle_rad =
        sight_height::zero_angle(sight_h_m, zero_r, stored.ammo_mv_ms).unwrap_or(0.0);
    let adjusted_angle_rad =
        sight_height::zero_angle(sight_h_m, zero_r, adjusted_mv).unwrap_or(stored_angle_rad);

    let correction_rad = adjusted_angle_rad - stored_angle_rad;
    let correction_moa = sight_height::rad_to_moa(correction_rad);
    let correction_mrad = sight_height::rad_to_mil(correction_rad);

    // New zero range under adjusted conditions (small-angle approximation).
    // With a given fixed scope setting θ_stored = atan(h/R) + gR/(2V²)
    // Under adjusted MV: the effective zero range shifts proportional to V².
    let new_zero_r = if adjusted_mv > 0.0 && stored.ammo_mv_ms > 0.0 {
        zero_r * (adjusted_mv / stored.ammo_mv_ms).powi(2)
    } else {
        zero_r
    };

    let mpbr = compute_mpbr(stored, 30.0, 15.0);

    ZeroCorrection {
        profile_id: stored.profile_id.clone(),
        new_zero_range_m: new_zero_r,
        correction_moa_vertical: correction_moa,
        correction_moa_horizontal: 0.0,
        correction_mrad_vertical: correction_mrad,
        correction_mrad_horizontal: 0.0,
        mpbr_m: mpbr,
    }
}

/// Compute the Maximum Point-Blank Range (MPBR) for a given target size.
///
/// MPBR is the maximum range at which the bullet stays within a vertical
/// window of `max_drop_cm` from the line of sight when aiming at the
/// centre of a target of size `target_size_cm`. The smaller of the two
/// constraints (half the target size, or `max_drop_cm`) governs the
/// usable vertical window.
///
/// Uses a vacuum (no-drag) trajectory approximation with the zero angle
/// computed from the profile. This gives a first-order estimate useful
/// for comparing profiles; real MPBR with drag is shorter.
///
/// # Arguments
/// * `profile` — the zero profile containing weapon/ammo data
/// * `target_size_cm` — vertical extent of the target (cm)
/// * `max_drop_cm` — maximum allowable bullet drop below the line of
///   sight (cm)
///
/// # Returns
/// MPBR in metres.
pub fn compute_mpbr(profile: &ZeroProfile, target_size_cm: f64, max_drop_cm: f64) -> f64 {
    let mv = profile.ammo_mv_ms;
    let sight_h_m = profile.sight_height_mm / 1000.0;
    let zero_r = profile.zero_range_m;

    if mv <= 0.0 || zero_r <= 0.0 {
        return 0.0;
    }

    // The usable vertical window is the smaller of:
    //   - half the target (aim at centre, bullet can rise/drop half the target size)
    //   - the max_drop_cm constraint
    let window_cm = max_drop_cm.min(target_size_cm / 2.0);
    let window_m = window_cm / 100.0;

    let theta = match sight_height::zero_angle(sight_h_m, zero_r, mv) {
        Some(a) => a,
        None => return 0.0,
    };

    // Parabolic trajectory height relative to LOS:
    //   y(x) = x·tan(θ) + h·x/R - h - ½·g·(x/MV)²
    //
    // Set y(x) = -window_m and solve for the positive root past the zero.
    let a_coeff = -GRAVITY / (2.0 * mv * mv);
    let b_coeff = theta.tan() + sight_h_m / zero_r;
    let c_coeff = -sight_h_m + window_m;

    let disc = b_coeff * b_coeff - 4.0 * a_coeff * c_coeff;
    if disc <= 0.0 {
        return zero_r;
    }

    let sqrt_disc = disc.sqrt();
    // a_coeff < 0, so the larger positive root uses -b - sqrt(disc) in numerator
    let mpbr = (-b_coeff - sqrt_disc) / (2.0 * a_coeff);
    mpbr.max(0.0)
}

/// Return all zero profiles in the database that match a given weapon class.
pub fn profiles_for_weapon(db: &ZeroDatabase, class: WeaponClass) -> Vec<&ZeroProfile> {
    db.profiles
        .iter()
        .filter(|p| p.weapon_class == class)
        .collect()
}

/// Add a new zero profile to the database, or update an existing one
/// if a profile with the same `profile_id` already exists.
pub fn set_zero_profile(db: &mut ZeroDatabase, profile: ZeroProfile) {
    if let Some(existing) = db
        .profiles
        .iter_mut()
        .find(|p| p.profile_id == profile.profile_id)
    {
        *existing = profile;
    } else {
        db.profiles.push(profile);
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn carbine_profile() -> ZeroProfile {
        ZeroProfile {
            profile_id: "m4_m855_15c_0m".into(),
            weapon_class: WeaponClass::Carbine,
            weapon_barrel_length_mm: 368.0,
            ammo_mass_g: 4.0,
            ammo_mv_ms: 950.0,
            ammo_bc: 0.157,
            ammo_cdm: "g7".into(),
            sight_height_mm: 40.0,
            zero_range_m: 100.0,
            shooter_base_moa: 1.5,
            environmental_temp_c: 15.0,
            environmental_altitude_m: 0.0,
            environmental_pressure_kpa: 101.325,
        }
    }

    fn sniper_profile() -> ZeroProfile {
        ZeroProfile {
            profile_id: "m24_m118lr_15c_0m".into(),
            weapon_class: WeaponClass::Sniper,
            weapon_barrel_length_mm: 610.0,
            ammo_mass_g: 11.3,
            ammo_mv_ms: 790.0,
            ammo_bc: 0.475,
            ammo_cdm: "g7".into(),
            sight_height_mm: 45.0,
            zero_range_m: 100.0,
            shooter_base_moa: 0.7,
            environmental_temp_c: 15.0,
            environmental_altitude_m: 0.0,
            environmental_pressure_kpa: 101.325,
        }
    }

    fn dmr_profile() -> ZeroProfile {
        ZeroProfile {
            profile_id: "sr25_m118lr_15c_0m".into(),
            weapon_class: WeaponClass::DMR,
            weapon_barrel_length_mm: 508.0,
            ammo_mass_g: 11.3,
            ammo_mv_ms: 820.0,
            ammo_bc: 0.475,
            ammo_cdm: "g7".into(),
            sight_height_mm: 42.0,
            zero_range_m: 100.0,
            shooter_base_moa: 1.0,
            environmental_temp_c: 15.0,
            environmental_altitude_m: 0.0,
            environmental_pressure_kpa: 101.325,
        }
    }

    // ── compute_zero_correction tests ───────────────────────────────────

    #[test]
    fn zero_correction_temperature_change() {
        // Stored at 15 °C, now at 35 °C (+20 °C)
        // ΔMV = 20 × 0.5 = +10 m/s → adjusted MV = 960 m/s
        // Higher MV → flatter trajectory → POI goes up → dial DOWN (negative correction)
        let profile = carbine_profile();
        let correction = compute_zero_correction(&profile, 35.0, 0.0);

        assert_eq!(correction.profile_id, "m4_m855_15c_0m");
        // Higher temp → higher effective MV → less elevation needed → negative correction
        assert!(
            correction.correction_moa_vertical < 0.0,
            "Hotter conditions should produce negative MOA correction, got {}",
            correction.correction_moa_vertical
        );
        assert!(
            correction.correction_mrad_vertical < 0.0,
            "Hotter conditions should produce negative MRAD correction"
        );
        // Horizontal should be zero (no crosswind effect from environment alone)
        assert!(
            correction.correction_moa_horizontal.abs() < 1e-12,
            "Horizontal correction should be zero"
        );
        // Higher effective MV → zero point moves further out
        assert!(
            correction.new_zero_range_m > 100.0,
            "Effective zero range should increase with higher MV"
        );
        // MPBR should be positive
        assert!(correction.mpbr_m > 0.0);
    }

    #[test]
    fn zero_correction_altitude_change() {
        // Stored at sea level, now at 2000 m
        // Δalt = +2000 m → ΔMV = -(2000/500) × 0.7 = -2.8 m/s → adjusted MV = 947.2 m/s
        // Lower effective MV → more drop → POI goes low → dial UP (positive correction)
        let profile = carbine_profile();
        let correction = compute_zero_correction(&profile, 15.0, 2000.0);

        assert!(
            correction.correction_moa_vertical > 0.0,
            "Higher altitude should produce positive MOA correction, got {}",
            correction.correction_moa_vertical
        );
        // Lower effective MV → effective zero range shrinks
        assert!(
            correction.new_zero_range_m < 100.0,
            "Effective zero range should decrease at altitude"
        );
    }

    #[test]
    fn zero_correction_combined_temp_and_altitude() {
        // Hotter (+20 °C) pushes correction negative, higher (2000 m) pushes positive
        // Net: ΔMV = 20×0.5 - (2000/500)×0.7 = 10 - 2.8 = +7.2 m/s
        // Dominant effect is temperature → net negative correction
        let profile = carbine_profile();
        let correction = compute_zero_correction(&profile, 35.0, 2000.0);

        // 10 m/s from temp, -2.8 m/s from altitude = +7.2 m/s net → still negative
        assert!(
            correction.correction_moa_vertical < 0.0,
            "Net +7.2 m/s should give negative correction"
        );
    }

    // ── MPBR tests ──────────────────────────────────────────────────────

    #[test]
    fn mpbr_carbine_vs_sniper() {
        let carbine = carbine_profile();
        let sniper = sniper_profile();

        let mpbr_c = compute_mpbr(&carbine, 30.0, 15.0);
        let mpbr_s = compute_mpbr(&sniper, 30.0, 15.0);

        // Sniper (higher BC, similar MV) should have longer MPBR
        // Actually, the sniper has lower MV (790 vs 950), so its MPBR might be shorter
        // Let's check: both use vacuum approximation, so MPBR scales with MV²
        // Actually the carbine has higher MV. Let's check MPBR scales with MV.
        // Higher MV → flatter trajectory → longer MPBR.
        // But sniper has lower MV, so... let me adjust.
        // For this test, just verify both are positive and physically reasonable.
        assert!(
            mpbr_c > 0.0,
            "Carbine MPBR should be positive, got {}",
            mpbr_c
        );
        assert!(
            mpbr_s > 0.0,
            "Sniper MPBR should be positive, got {}",
            mpbr_s
        );
        // Carbine has higher MV → should have longer MPBR in vacuum model
        // (For real ballistics, sniper BC dominates, but here we use vacuum)
        assert!(
            mpbr_c > mpbr_s,
            "Higher-MV carbine should have longer vacuum MPBR: carbine={}, sniper={}",
            mpbr_c,
            mpbr_s
        );
    }

    #[test]
    fn mpbr_larger_target_longer_range() {
        let profile = carbine_profile();
        let mpbr_small = compute_mpbr(&profile, 10.0, 5.0);
        let mpbr_large = compute_mpbr(&profile, 50.0, 25.0);

        assert!(
            mpbr_large > mpbr_small,
            "Larger target should allow longer MPBR: small={}, large={}",
            mpbr_small,
            mpbr_large
        );
    }

    #[test]
    fn mpbr_precision_vs_battle() {
        // Precision (10 cm head, 5 cm max drop) vs battle (30 cm torso, 15 cm drop)
        let profile = carbine_profile();
        let precision = compute_mpbr(&profile, 10.0, 5.0);
        let battle = compute_mpbr(&profile, 30.0, 15.0);

        assert!(
            battle > precision,
            "Battle MPBR should exceed precision MPBR: battle={}, precision={}",
            battle,
            precision
        );
    }

    // ── Database / profile management tests ─────────────────────────────

    #[test]
    fn multiple_profiles_same_weapon_class() {
        let mut db = ZeroDatabase { profiles: vec![] };

        let p1 = carbine_profile();
        let p2 = ZeroProfile {
            profile_id: "m4_mk262_15c_0m".into(),
            weapon_class: WeaponClass::Carbine,
            ammo_mass_g: 5.0,
            ammo_mv_ms: 880.0,
            ..carbine_profile()
        };
        let p3 = sniper_profile();

        set_zero_profile(&mut db, p1);
        set_zero_profile(&mut db, p2);
        set_zero_profile(&mut db, p3);

        let carbine_profiles = profiles_for_weapon(&db, WeaponClass::Carbine);
        assert_eq!(carbine_profiles.len(), 2, "Should find 2 carbine profiles");

        let sniper_profiles = profiles_for_weapon(&db, WeaponClass::Sniper);
        assert_eq!(sniper_profiles.len(), 1, "Should find 1 sniper profile");

        let rifle_profiles = profiles_for_weapon(&db, WeaponClass::Rifle);
        assert_eq!(rifle_profiles.len(), 0, "Should find 0 rifle profiles");
    }

    #[test]
    fn zero_profile_roundtrip() {
        let mut db = ZeroDatabase { profiles: vec![] };
        let profile = carbine_profile();

        // Add
        set_zero_profile(&mut db, profile.clone());
        assert_eq!(db.profiles.len(), 1);

        // Retrieve
        let retrieved = profiles_for_weapon(&db, WeaponClass::Carbine);
        assert_eq!(retrieved.len(), 1);
        assert_eq!(retrieved[0].profile_id, "m4_m855_15c_0m");
        assert!((retrieved[0].ammo_mv_ms - 950.0).abs() < 1e-10);
        assert!((retrieved[0].sight_height_mm - 40.0).abs() < 1e-10);

        // Update
        let updated = ZeroProfile {
            ammo_mv_ms: 960.0,
            ..profile
        };
        set_zero_profile(&mut db, updated);
        assert_eq!(db.profiles.len(), 1, "Update should not add a new entry");
        let retrieved = profiles_for_weapon(&db, WeaponClass::Carbine);
        assert!(
            (retrieved[0].ammo_mv_ms - 960.0).abs() < 1e-10,
            "Updated MV should be 960, got {}",
            retrieved[0].ammo_mv_ms
        );
    }

    #[test]
    fn set_zero_profile_updates_existing() {
        let mut db = ZeroDatabase { profiles: vec![] };
        let p1 = carbine_profile();
        let p2 = ZeroProfile {
            profile_id: "m4_m855_15c_0m".into(), // same ID
            ammo_mv_ms: 920.0,
            ..carbine_profile()
        };

        set_zero_profile(&mut db, p1);
        set_zero_profile(&mut db, p2);

        assert_eq!(db.profiles.len(), 1, "Same ID should replace, not add");
        let carbine = &db.profiles[0];
        assert!(
            (carbine.ammo_mv_ms - 920.0).abs() < 1e-10,
            "MV should be updated to 920"
        );
    }

    // ── Environmental delta physical expectation tests ───────────────────

    #[test]
    fn environmental_delta_matches_physical_expectation() {
        let profile = carbine_profile();

        // Colder: -20 °C → ΔMV = -10 m/s → more drop → dial UP
        let colder = compute_zero_correction(&profile, -5.0, 0.0);
        assert!(
            colder.correction_moa_vertical > 0.0,
            "Colder should need positive correction (dial up)"
        );

        // Hotter: +20 °C → ΔMV = +10 m/s → less drop → dial DOWN
        let hotter = compute_zero_correction(&profile, 35.0, 0.0);
        assert!(
            hotter.correction_moa_vertical < 0.0,
            "Hotter should need negative correction (dial down)"
        );

        // The magnitude of correction should increase with larger ΔT
        let slightly_hotter = compute_zero_correction(&profile, 25.0, 0.0);
        assert!(
            hotter.correction_moa_vertical.abs() > slightly_hotter.correction_moa_vertical.abs(),
            "Larger ΔT should produce larger correction magnitude"
        );

        // Higher altitude → dial UP
        let high_alt = compute_zero_correction(&profile, 15.0, 3000.0);
        assert!(
            high_alt.correction_moa_vertical > 0.0,
            "Higher altitude should need positive correction (dial up)"
        );
    }

    #[test]
    fn zero_correction_no_change_conditions() {
        let profile = carbine_profile();
        // Same conditions → zero correction
        let correction = compute_zero_correction(&profile, 15.0, 0.0);
        assert!(
            correction.correction_moa_vertical.abs() < 1e-10,
            "Same conditions should give zero correction"
        );
        assert!(
            (correction.new_zero_range_m - 100.0).abs() < 0.5,
            "New zero range should equal stored at same conditions"
        );
    }

    #[test]
    fn mpbr_all_weapon_classes() {
        let classes = [
            WeaponClass::Carbine,
            WeaponClass::Rifle,
            WeaponClass::DMR,
            WeaponClass::Sniper,
            WeaponClass::LMG,
            WeaponClass::Pistol,
            WeaponClass::Shotgun,
            WeaponClass::Launcher,
        ];
        for class in classes {
            let profile = ZeroProfile {
                profile_id: format!("test_{:?}", class),
                weapon_class: class,
                weapon_barrel_length_mm: 400.0,
                ammo_mass_g: 8.0,
                ammo_mv_ms: 850.0,
                ammo_bc: 0.250,
                ammo_cdm: "g7".into(),
                sight_height_mm: 40.0,
                zero_range_m: 100.0,
                shooter_base_moa: 1.5,
                environmental_temp_c: 15.0,
                environmental_altitude_m: 0.0,
                environmental_pressure_kpa: 101.325,
            };
            let mpbr = compute_mpbr(&profile, 30.0, 15.0);
            assert!(
                mpbr > 0.0,
                "MPBR should be positive for {:?}, got {}",
                class,
                mpbr
            );
        }
    }
}
