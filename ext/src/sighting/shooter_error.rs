// ABE - Shooter Error / Human Precision Model
//
// Models shooter-dependent accuracy variables: stance, weapon support,
// heart rate, breathing, fatigue, and experience level effects on
// shot placement precision.
//
// Combines with weapon dispersion (dispersion.rs) as root-sum-square
// to produce total system dispersion.
//
// All factors use deterministic lookup tables (no RNG).
//
// References:
//   - US Army DMR / AMU marksmanship doctrine
//   - NATO STANAG 4655 (small arms accuracy)
//   - MIL-STD-810 (human factors engineering)

/// Shooter body position affects stability and natural point of aim.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShooterStance {
    Prone,
    Kneeling,
    Standing,
    Crouched,
    SittingSupported,
}

/// Mechanical support type helping stabilise the weapon.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SupportType {
    Bipod,
    Tripod,
    RestSandbag,
    Sling,
    Unsupported,
    VehicleMount,
}

/// Phase of the shooter's respiratory cycle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BreathPhase {
    Hold,
    Normal,
    Heavy,
}

/// Marksmanship experience / training level.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExperienceLevel {
    Novice,
    Intermediate,
    Advanced,
    Expert,
    Precision,
}

/// All shooter-dependent parameters influencing precision.
#[derive(Debug, Clone, Copy)]
pub struct ShooterParams {
    /// Shooting stance
    pub stance: ShooterStance,
    /// Weapon support type
    pub support: SupportType,
    /// Current heart rate in beats per minute
    pub heart_rate_bpm: f64,
    /// Current breathing phase
    pub breathing: BreathPhase,
    /// Fatigue level: 0.0 = rested, 1.0 = exhausted
    pub fatigue_fraction: f64,
    /// Shooter experience level
    pub experience: ExperienceLevel,
    /// Natural precision of the shooter in MOA (typical range 1–4)
    pub base_shooter_moa: f64,
}

// ── Factor lookup tables ───────────────────────────────────────────────────────

/// Stance stability multiplier (lower = more stable).
fn stance_factor(stance: ShooterStance) -> f64 {
    match stance {
        ShooterStance::Prone => 1.0,
        ShooterStance::Kneeling => 1.5,
        ShooterStance::Standing => 2.5,
        ShooterStance::Crouched => 2.0,
        ShooterStance::SittingSupported => 1.2,
    }
}

/// Support-equipment multiplier (lower = more stable).
fn support_factor(support: SupportType) -> f64 {
    match support {
        SupportType::Bipod => 0.7,
        SupportType::Tripod => 0.5,
        SupportType::RestSandbag => 0.6,
        SupportType::Sling => 0.8,
        SupportType::Unsupported => 1.0,
        SupportType::VehicleMount => 0.75,
    }
}

/// Heart-rate multiplier.
///
/// Piecewise-linear interpolation over the reference points:
///   60 bpm → 1.0x, 120 bpm → 1.5x, 180 bpm → 2.5x
/// Values outside [60, 180] are clamped to the nearest reference.
fn heart_rate_factor(heart_rate_bpm: f64) -> f64 {
    if heart_rate_bpm <= 60.0 {
        1.0
    } else if heart_rate_bpm <= 120.0 {
        let t = (heart_rate_bpm - 60.0) / 60.0;
        1.0 + t * 0.5
    } else if heart_rate_bpm <= 180.0 {
        let t = (heart_rate_bpm - 120.0) / 60.0;
        1.5 + t * 1.0
    } else {
        2.5
    }
}

/// Breathing-phase multiplier.
fn breath_factor(phase: BreathPhase) -> f64 {
    match phase {
        BreathPhase::Hold => 0.8,
        BreathPhase::Normal => 1.0,
        BreathPhase::Heavy => 1.8,
    }
}

/// Fatigue multiplier: linear degradation with exertion.
///
/// `fatigue_fraction` ∈ [0, 1]; clamped to that range.
/// Rested (0.0) → 1.0x, exhausted (1.0) → 2.0x.
fn fatigue_factor(fatigue_fraction: f64) -> f64 {
    let f = fatigue_fraction.clamp(0.0, 1.0);
    1.0 + f * 1.0
}

/// Experience-level multiplier.
fn experience_factor(level: ExperienceLevel) -> f64 {
    match level {
        ExperienceLevel::Novice => 1.5,
        ExperienceLevel::Intermediate => 1.2,
        ExperienceLevel::Advanced => 1.0,
        ExperienceLevel::Expert => 0.85,
        ExperienceLevel::Precision => 0.7,
    }
}

/// Suggested base shooter MOA for a given experience level.
///
/// These are typical values; callers may override with
/// `ShooterParams.base_shooter_moa` for custom profiles.
pub fn default_base_moa(level: ExperienceLevel) -> f64 {
    match level {
        ExperienceLevel::Novice => 4.0,
        ExperienceLevel::Intermediate => 2.5,
        ExperienceLevel::Advanced => 1.5,
        ExperienceLevel::Expert => 1.0,
        ExperienceLevel::Precision => 0.7,
    }
}

// ── Core API ───────────────────────────────────────────────────────────────────

/// Compute the shooter's contribution to total system dispersion (MOA).
///
/// Shooter MOA = base_shooter_MOA × stance × support × HR × breath × fatigue × exp
pub fn shooter_dispersion_moa(params: &ShooterParams) -> f64 {
    let base = params.base_shooter_moa;
    let stance = stance_factor(params.stance);
    let support = support_factor(params.support);
    let hr = heart_rate_factor(params.heart_rate_bpm);
    let breath = breath_factor(params.breathing);
    let fatigue = fatigue_factor(params.fatigue_fraction);
    let exp = experience_factor(params.experience);

    base * stance * support * hr * breath * fatigue * exp
}

/// Combine weapon dispersion and shooter dispersion into total system MOA.
///
/// Uses root-sum-square: total = sqrt(weapon² + shooter²)
pub fn total_system_moa(weapon_moa: f64, shooter: &ShooterParams) -> f64 {
    let shooter_moa = shooter_dispersion_moa(shooter);
    (weapon_moa.powi(2) + shooter_moa.powi(2)).sqrt()
}

/// Convert total system MOA to a standard deviation in metres at a given range.
///
/// 1 MOA ≈ 0.291 mil at 100 m, where 1 mil = 0.001 rad.
/// Derived: σ_m = total_MOA × (π / (180 × 60)) × range_m
pub fn system_standard_deviation_m(total_moa: f64, range_m: f64) -> f64 {
    let moa_rad = std::f64::consts::PI / (180.0 * 60.0);
    total_moa * moa_rad * range_m
}

/// Hit probability on a circular target of given size.
///
/// Uses the Rayleigh distribution (radial error of a circular Gaussian):
///   P = 1 - exp(-R² / (2σ²))
///
/// where R = target_size_cm / 2 is the target radius (converted to metres),
/// and σ = system_sigma_m is the system standard deviation.
///
/// # Returns
/// Probability in [0, 1].
pub fn hit_probability(target_size_cm: f64, system_sigma_m: f64) -> f64 {
    if system_sigma_m <= 0.0 {
        return if target_size_cm > 0.0 { 1.0 } else { 0.0 };
    }
    let radius_m = target_size_cm / 200.0; // cm → m radius
    if radius_m <= 0.0 {
        return 0.0;
    }
    // P = 1 - exp(-R² / (2σ²))
    1.0 - (-(radius_m * radius_m) / (2.0 * system_sigma_m * system_sigma_m)).exp()
}

/// Error function via Abramowitz & Stegun 7.1.26 approximation.
///
/// Maximum error ~1.5×10⁻⁷. Valid for all real x.
fn erf_approx(x: f64) -> f64 {
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();

    // Constants from A&S 7.1.26
    let p = 0.327_591_1;
    let a1 = 0.254_829_592;
    let a2 = -0.284_496_736;
    let a3 = 1.421_413_741;
    let a4 = -1.453_152_027;
    let a5 = 1.061_405_429;

    let t = 1.0 / (1.0 + p * x);
    let y = 1.0 - ((((a5 * t + a4) * t + a3) * t + a2) * t + a1) * t * (-x * x).exp();
    sign * y
}

/// Standard normal CDF: Φ(z) = 0.5 × (1 + erf(z / √2))
fn normal_cdf(z: f64) -> f64 {
    0.5 * (1.0 + erf_approx(z / std::f64::consts::SQRT_2))
}

/// Probability of hit on a rectangular target given separate X/Y dispersion
/// and aim-point offset.
///
/// Uses the product of two independent 1D Gaussian CDFs:
///   P = [Φ((X/2 - ox)/σx) - Φ((-X/2 - ox)/σx)]
///     × [Φ((Y/2 - oy)/σy) - Φ((-Y/2 - oy)/σy)]
///
/// # Arguments
/// * `target_width_m`  — target width in metres (horizontal)
/// * `target_height_m` — target height in metres (vertical)
/// * `sigma_horizontal_m` — system standard deviation in horizontal (metres)
/// * `sigma_vertical_m`   — system standard deviation in vertical (metres)
/// * `aim_offset_x_m` — horizontal aim-point offset from centre (metres)
/// * `aim_offset_y_m` — vertical aim-point offset from centre (metres)
///
/// # Returns
/// Probability in [0, 1].
pub fn hit_probability_rect(
    target_width_m: f64,
    target_height_m: f64,
    sigma_horizontal_m: f64,
    sigma_vertical_m: f64,
    aim_offset_x_m: f64,
    aim_offset_y_m: f64,
) -> f64 {
    if target_width_m <= 0.0 || target_height_m <= 0.0 {
        return 0.0;
    }

    let prob_x = if sigma_horizontal_m > 0.0 {
        let half = target_width_m / 2.0;
        let z_upper = (half - aim_offset_x_m) / sigma_horizontal_m;
        let z_lower = (-half - aim_offset_x_m) / sigma_horizontal_m;
        normal_cdf(z_upper) - normal_cdf(z_lower)
    } else {
        // No horizontal dispersion: perfect aim or miss based on offset
        if aim_offset_x_m.abs() < target_width_m / 2.0 {
            1.0
        } else {
            0.0
        }
    };

    let prob_y = if sigma_vertical_m > 0.0 {
        let half = target_height_m / 2.0;
        let z_upper = (half - aim_offset_y_m) / sigma_vertical_m;
        let z_lower = (-half - aim_offset_y_m) / sigma_vertical_m;
        normal_cdf(z_upper) - normal_cdf(z_lower)
    } else {
        if aim_offset_y_m.abs() < target_height_m / 2.0 {
            1.0
        } else {
            0.0
        }
    };

    prob_x * prob_y
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn default_shooter() -> ShooterParams {
        ShooterParams {
            stance: ShooterStance::Prone,
            support: SupportType::Unsupported,
            heart_rate_bpm: 60.0,
            breathing: BreathPhase::Normal,
            fatigue_fraction: 0.0,
            experience: ExperienceLevel::Advanced,
            base_shooter_moa: 1.5,
        }
    }

    // ── Stance factors ──────────────────────────────────────────────────────

    #[test]
    fn stance_prone_is_baseline() {
        let mut p = default_shooter();
        p.stance = ShooterStance::Prone;
        // Prone factor = 1.0, so with Advanced (1.0) and base 1.5 → 1.5
        let moa = shooter_dispersion_moa(&p);
        assert!(
            (moa - 1.5).abs() < 1e-12,
            "Prone should be baseline 1.5, got {}",
            moa
        );
    }

    #[test]
    fn stance_standing_is_least_stable() {
        let mut p = default_shooter();
        p.stance = ShooterStance::Standing;
        let standing = shooter_dispersion_moa(&p);
        p.stance = ShooterStance::Prone;
        let prone = shooter_dispersion_moa(&p);
        assert!(
            standing > prone,
            "Standing ({}) should be less stable than prone ({})",
            standing,
            prone
        );
    }

    #[test]
    fn stance_kneeling_factor() {
        let mut p = default_shooter();
        p.stance = ShooterStance::Kneeling;
        let moa = shooter_dispersion_moa(&p);
        // 1.5 * 1.5 = 2.25
        assert!(
            (moa - 2.25).abs() < 1e-12,
            "Kneeling should give 2.25 MOA, got {}",
            moa
        );
    }

    #[test]
    fn stance_crouched_factor() {
        let mut p = default_shooter();
        p.stance = ShooterStance::Crouched;
        let moa = shooter_dispersion_moa(&p);
        // 1.5 * 2.0 = 3.0
        assert!(
            (moa - 3.0).abs() < 1e-12,
            "Crouched should give 3.0 MOA, got {}",
            moa
        );
    }

    #[test]
    fn stance_sitting_supported_factor() {
        let mut p = default_shooter();
        p.stance = ShooterStance::SittingSupported;
        let moa = shooter_dispersion_moa(&p);
        // 1.5 * 1.2 = 1.8
        assert!(
            (moa - 1.8).abs() < 1e-12,
            "SittingSupported should give 1.8 MOA, got {}",
            moa
        );
    }

    // ── Support types ───────────────────────────────────────────────────────

    #[test]
    fn support_tripod_is_most_stable() {
        let mut p = default_shooter();
        p.support = SupportType::Tripod;
        let tripod = shooter_dispersion_moa(&p);
        p.support = SupportType::Bipod;
        let bipod = shooter_dispersion_moa(&p);
        assert!(
            tripod < bipod,
            "Tripod ({}) should be more stable than bipod ({})",
            tripod,
            bipod
        );
    }

    #[test]
    fn support_unsupported_is_worst() {
        let mut p = default_shooter();
        p.support = SupportType::Unsupported;
        let unsup = shooter_dispersion_moa(&p);
        p.support = SupportType::RestSandbag;
        let sandbag = shooter_dispersion_moa(&p);
        assert!(
            unsup > sandbag,
            "Unsupported ({}) should be worse than sandbag ({})",
            unsup,
            sandbag
        );
    }

    #[test]
    fn support_vehicle_mount_is_between_tripod_and_bipod() {
        let mut p = default_shooter();
        p.support = SupportType::VehicleMount;
        let vm = shooter_dispersion_moa(&p);
        p.support = SupportType::Tripod;
        let tripod = shooter_dispersion_moa(&p);
        p.support = SupportType::Bipod;
        let bipod = shooter_dispersion_moa(&p);
        // Vehicle mount (0.75) should be less stable than tripod (0.5)
        // but more stable than unsupported
        p.support = SupportType::Unsupported;
        let unsupported = shooter_dispersion_moa(&p);
        assert!(
            vm > tripod && vm < unsupported,
            "VehicleMount ({}) should be between tripod ({}) and unsupported ({})",
            vm,
            tripod,
            unsupported
        );
        assert!(
            (vm - bipod).abs() < 0.2,
            "VehicleMount ({}) should be close to bipod ({})",
            vm,
            bipod
        );
    }

    // ── Experience levels ───────────────────────────────────────────────────

    #[test]
    fn experience_novice_is_worst() {
        let mut p = default_shooter();
        p.experience = ExperienceLevel::Novice;
        p.base_shooter_moa = default_base_moa(ExperienceLevel::Novice);
        let novice = shooter_dispersion_moa(&p);
        p.experience = ExperienceLevel::Precision;
        p.base_shooter_moa = default_base_moa(ExperienceLevel::Precision);
        let precision = shooter_dispersion_moa(&p);
        assert!(
            novice > precision,
            "Novice ({}) should be worse than Precision ({})",
            novice,
            precision
        );
    }

    #[test]
    fn default_base_moa_monotonic() {
        let levels = [
            ExperienceLevel::Novice,
            ExperienceLevel::Intermediate,
            ExperienceLevel::Advanced,
            ExperienceLevel::Expert,
            ExperienceLevel::Precision,
        ];
        for i in 1..levels.len() {
            let a = default_base_moa(levels[i - 1]);
            let b = default_base_moa(levels[i]);
            assert!(
                a >= b,
                "Base MOA should be non-increasing: {:?}={} < {:?}={}",
                levels[i - 1],
                a,
                levels[i],
                b
            );
        }
    }

    // ── Heart rate ──────────────────────────────────────────────────────────

    #[test]
    fn heart_rate_at_rest_is_baseline() {
        let mut p = default_shooter();
        p.heart_rate_bpm = 60.0;
        let moa1 = shooter_dispersion_moa(&p);
        p.heart_rate_bpm = 45.0; // below clamp
        let moa2 = shooter_dispersion_moa(&p);
        assert!(
            (moa1 - moa2).abs() < 1e-12,
            "HR below 60 should clamp: {} vs {}",
            moa1,
            moa2
        );
    }

    #[test]
    fn heart_rate_elevated_increases_dispersion() {
        let mut p = default_shooter();
        p.heart_rate_bpm = 60.0;
        let rest = shooter_dispersion_moa(&p);
        p.heart_rate_bpm = 120.0;
        let elevated = shooter_dispersion_moa(&p);
        assert!(
            elevated > rest * 1.49 && elevated < rest * 1.51,
            "120 bpm should be ~1.5x rest: rest={}, elevated={}",
            rest,
            elevated
        );
    }

    #[test]
    fn heart_rate_max_capped() {
        let mut p = default_shooter();
        p.heart_rate_bpm = 180.0;
        let at_max = shooter_dispersion_moa(&p);
        p.heart_rate_bpm = 220.0;
        let above_max = shooter_dispersion_moa(&p);
        assert!(
            (at_max - above_max).abs() < 1e-12,
            "HR above 180 should clamp: {} vs {}",
            at_max,
            above_max
        );
    }

    // ── Breathing ───────────────────────────────────────────────────────────

    #[test]
    fn breath_hold_is_best() {
        let mut p = default_shooter();
        p.breathing = BreathPhase::Hold;
        let hold = shooter_dispersion_moa(&p);
        p.breathing = BreathPhase::Normal;
        let normal = shooter_dispersion_moa(&p);
        p.breathing = BreathPhase::Heavy;
        let heavy = shooter_dispersion_moa(&p);
        assert!(
            hold < normal,
            "Hold ({}) should beat normal ({})",
            hold,
            normal
        );
        assert!(
            normal < heavy,
            "Normal ({}) should beat heavy ({})",
            normal,
            heavy
        );
    }

    // ── Fatigue ─────────────────────────────────────────────────────────────

    #[test]
    fn fatigue_increases_dispersion() {
        let mut p = default_shooter();
        p.fatigue_fraction = 0.0;
        let rested = shooter_dispersion_moa(&p);
        p.fatigue_fraction = 1.0;
        let exhausted = shooter_dispersion_moa(&p);
        assert!(
            exhausted > rested,
            "Exhausted ({}) should be worse than rested ({})",
            exhausted,
            rested
        );
    }

    #[test]
    fn fatigue_clamped() {
        let mut p = default_shooter();
        p.fatigue_fraction = -0.1;
        let below = shooter_dispersion_moa(&p);
        p.fatigue_fraction = 0.0;
        let at_zero = shooter_dispersion_moa(&p);
        assert!(
            (below - at_zero).abs() < 1e-12,
            "Negative fatigue should clamp: {} vs {}",
            below,
            at_zero
        );
    }

    // ── Total system MOA ────────────────────────────────────────────────────

    #[test]
    fn total_system_moa_greater_than_weapon_alone() {
        let weapon_moa = 1.0;
        let shooter = default_shooter();
        let total = total_system_moa(weapon_moa, &shooter);
        let shooter_moa = shooter_dispersion_moa(&shooter);
        assert!(
            total > weapon_moa,
            "Total ({}) should exceed weapon ({})",
            total,
            weapon_moa
        );
        // root-sum-square: total = sqrt(1² + 1.5²) ≈ 1.803
        let expected = (weapon_moa.powi(2) + shooter_moa.powi(2)).sqrt();
        assert!(
            (total - expected).abs() < 1e-12,
            "Total should be RSS: expected {}, got {}",
            expected,
            total
        );
    }

    #[test]
    fn total_system_moa_zero_shooter_error() {
        let weapon_moa = 2.0;
        let mut shooter = default_shooter();
        shooter.base_shooter_moa = 0.0;
        let total = total_system_moa(weapon_moa, &shooter);
        assert!(
            (total - weapon_moa).abs() < 1e-12,
            "Zero shooter error: total ({}) should equal weapon ({})",
            total,
            weapon_moa
        );
    }

    #[test]
    fn total_system_moa_symmetric() {
        let mut shooter = default_shooter();
        shooter.base_shooter_moa = 0.0;
        let zero_shooter = total_system_moa(1.0, &shooter);
        // Weapon MOA = 0 → total = shooter only
        let weapon_only = total_system_moa(0.0, &default_shooter());
        let shooter_moa = shooter_dispersion_moa(&default_shooter());
        assert!(
            (weapon_only - shooter_moa).abs() < 1e-12,
            "Zero weapon should give shooter MOA: {} vs {}",
            weapon_only,
            shooter_moa
        );
        assert!(
            (zero_shooter - 1.0).abs() < 1e-12,
            "Zero shooter should give weapon MOA: {} vs 1.0",
            zero_shooter
        );
    }

    // ── standard_deviation ──────────────────────────────────────────────────

    #[test]
    fn sd_scales_with_range() {
        let sigma_100 = system_standard_deviation_m(1.0, 100.0);
        let sigma_200 = system_standard_deviation_m(1.0, 200.0);
        assert!(
            (sigma_200 - 2.0 * sigma_100).abs() < 1e-12,
            "SD should scale linearly with range: {} vs 2×{}",
            sigma_200,
            sigma_100
        );
    }

    #[test]
    fn sd_zero_moa_zero_sigma() {
        let sigma = system_standard_deviation_m(0.0, 500.0);
        assert!(
            sigma.abs() < 1e-12,
            "Zero MOA should give zero sigma, got {}",
            sigma
        );
    }

    // ── Hit probability (circular) ──────────────────────────────────────────

    #[test]
    fn hit_probability_is_between_zero_and_one() {
        for moa in [0.5, 1.0, 2.0, 5.0] {
            for range in [100.0, 300.0, 600.0] {
                for target in [10.0, 30.0, 50.0] {
                    let sigma = system_standard_deviation_m(moa, range);
                    let p = hit_probability(target, sigma);
                    assert!(
                        p >= 0.0 && p <= 1.0,
                        "P={} out of [0,1] for MOA={}, range={}, target={}",
                        p,
                        moa,
                        range,
                        target
                    );
                }
            }
        }
    }

    #[test]
    fn hit_probability_larger_target_higher_chance() {
        let sigma = system_standard_deviation_m(2.0, 300.0);
        let p_small = hit_probability(20.0, sigma);
        let p_large = hit_probability(60.0, sigma);
        assert!(
            p_large > p_small,
            "Larger target ({}) should have higher P than smaller ({})",
            p_large,
            p_small
        );
    }

    #[test]
    fn hit_probability_larger_dispersion_lower_chance() {
        let sigma_bad = system_standard_deviation_m(5.0, 300.0);
        let sigma_good = system_standard_deviation_m(1.0, 300.0);
        let p_bad = hit_probability(30.0, sigma_bad);
        let p_good = hit_probability(30.0, sigma_good);
        assert!(
            p_good > p_bad,
            "Good dispersion ({}) should give higher P than bad ({})",
            p_good,
            p_bad
        );
    }

    #[test]
    fn hit_probability_zero_sigma_deterministic_hit() {
        let p = hit_probability(30.0, 0.0);
        assert!(
            (p - 1.0).abs() < 1e-12,
            "Zero sigma should guarantee a hit if target > 0, got {}",
            p
        );
    }

    #[test]
    fn hit_probability_zero_target_no_hit() {
        let sigma = system_standard_deviation_m(1.0, 100.0);
        let p = hit_probability(0.0, sigma);
        assert!(
            p.abs() < 1e-12,
            "Zero target size should give zero probability, got {}",
            p
        );
    }

    // ── Hit probability (rectangular) ───────────────────────────────────────

    #[test]
    fn hit_probability_rect_centered_symmetric() {
        let p = hit_probability_rect(1.0, 1.0, 0.5, 0.5, 0.0, 0.0);
        assert!(
            p > 0.0 && p <= 1.0,
            "Centered aim on 1m target with 0.5m sigma should give reasonable P, got {}",
            p
        );
    }

    #[test]
    fn hit_probability_rect_offset_reduces_chance() {
        let centered = hit_probability_rect(1.0, 1.0, 0.5, 0.5, 0.0, 0.0);
        let offset = hit_probability_rect(1.0, 1.0, 0.5, 0.5, 2.0, 0.0);
        assert!(
            centered > offset,
            "Centered aim ({}) should beat offset aim ({})",
            centered,
            offset
        );
    }

    #[test]
    fn hit_probability_rect_zero_target() {
        let p = hit_probability_rect(0.0, 1.0, 0.5, 0.5, 0.0, 0.0);
        assert!(
            p.abs() < 1e-12,
            "Zero-width target should give zero probability, got {}",
            p
        );
    }

    #[test]
    fn hit_probability_rect_zero_sigma_perfect_aim() {
        // With zero dispersion and aim at centre, should always hit
        let p = hit_probability_rect(1.0, 1.0, 0.0, 0.0, 0.0, 0.0);
        assert!(
            (p - 1.0).abs() < 1e-12,
            "Zero sigma + centred aim should give 1.0, got {}",
            p
        );
    }

    #[test]
    fn hit_probability_rect_zero_sigma_off_target() {
        // With zero dispersion and aim outside target, should never hit
        let p = hit_probability_rect(1.0, 1.0, 0.0, 0.0, 10.0, 0.0);
        assert!(
            p.abs() < 1e-12,
            "Zero sigma + offset aim should give 0.0, got {}",
            p
        );
    }

    // ── Determinism ─────────────────────────────────────────────────────────

    #[test]
    fn shooter_dispersion_is_deterministic() {
        let p = default_shooter();
        let a = shooter_dispersion_moa(&p);
        let b = shooter_dispersion_moa(&p);
        assert!(
            (a - b).abs() < 1e-12,
            "Same params should give same result: {} vs {}",
            a,
            b
        );
    }

    // ── Full end-to-end scenario ────────────────────────────────────────────

    #[test]
    fn prone_bipod_expert_clear_shot() {
        let shooter = ShooterParams {
            stance: ShooterStance::Prone,
            support: SupportType::Bipod,
            heart_rate_bpm: 65.0,
            breathing: BreathPhase::Hold,
            fatigue_fraction: 0.1,
            experience: ExperienceLevel::Expert,
            base_shooter_moa: default_base_moa(ExperienceLevel::Expert),
        };
        let shooter_moa = shooter_dispersion_moa(&shooter);
        // 1.0 * 1.0 * 0.7 * ~1.04 * 0.8 * 1.1 * 0.85 ≈ 0.545
        assert!(
            shooter_moa > 0.3 && shooter_moa < 1.5,
            "Prone bipod expert should be tight MOA, got {}",
            shooter_moa
        );

        let total = total_system_moa(0.8, &shooter);
        assert!(
            total > shooter_moa,
            "Total ({}) should exceed shooter alone ({})",
            total,
            shooter_moa
        );

        let sigma = system_standard_deviation_m(total, 500.0);
        assert!(
            sigma > 0.0 && sigma < 5.0,
            "Sigma at 500m should be reasonable, got {}",
            sigma
        );

        let p = hit_probability(50.0, sigma);
        assert!(
            p > 0.0 && p <= 1.0,
            "Hit probability on 50cm target should be in [0,1], got {}",
            p
        );
    }
}
