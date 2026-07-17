// ABE - Lot-to-Lot Variation Model
//
// Models ammunition lot-to-lot variation: different production lots of
// the same ammunition have slightly different ballistic coefficients,
// muzzle velocities, and propellant burn rates.
//
// Within-lot variation is small (MV ±2-5 m/s for match, ±5-15 for ball).
// Between-lot variation is larger (MV ±5-15 m/s typical, BC ±2-5%).
//
// Uses deterministic golden-angle-based sampling (like dispersion.rs)
// so each lot_index always produces the same (MV, BC) offsets.
//
// References:
//   - Bryan Litz "Applied Ballistics Precision" (lot variation data)
//   - NATO STANAG 4357 (ammunition lot acceptance)
//   - SAAMI Z299.4 (ammunition uniformity standards)

/// Classification of ammunition manufacturing quality.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AmmoGrade {
    /// Premium match ammunition — tightest QC, smallest variance.
    Match,
    /// Standard military ball / service ammunition.
    Service,
    /// Surplus ammunition or unknown provenance — widest variance.
    Surplus,
    /// Hand-loaded ammunition — depends on loader skill.
    Handload,
}

/// Input parameters for lot variation statistics.
#[derive(Debug, Clone)]
pub struct LotVariationParams {
    /// Ammunition grade (determines base sigma values).
    pub grade: AmmoGrade,
    /// Nominal muzzle velocity (m/s) for this cartridge.
    pub nominal_mv_ms: f64,
    /// Nominal ballistic coefficient (G7 standard).
    pub nominal_bc: f64,
    /// Ambient temperature in degrees Celsius.
    pub temperature_c: f64,
    /// Number of lots to generate statistics for.
    pub lot_count: i32,
}

/// Generated lot-variation statistics.
#[derive(Debug, Clone)]
pub struct LotStatistics {
    /// Mean muzzle velocity across all lots (m/s).
    pub mean_mv_ms: f64,
    /// Standard deviation of muzzle velocity across lots (m/s).
    pub std_mv_ms: f64,
    /// Mean ballistic coefficient across all lots.
    pub mean_bc: f64,
    /// Standard deviation of ballistic coefficient across lots.
    pub std_bc: f64,
    /// Per-lot (MV, BC) samples for downstream use.
    pub lot_samples: Vec<(f64, f64)>,
}

// ── Grade-specific sigma values ─────────────────────────────────────────────

/// Standard deviation for within-grade muzzle velocity dispersion (m/s).
fn grade_sigma_mv(grade: AmmoGrade) -> f64 {
    match grade {
        AmmoGrade::Match => 2.0,
        AmmoGrade::Service => 5.0,
        AmmoGrade::Surplus => 12.0,
        AmmoGrade::Handload => 3.0,
    }
}

/// Standard deviation for within-grade ballistic coefficient dispersion.
fn grade_sigma_bc(grade: AmmoGrade) -> f64 {
    match grade {
        AmmoGrade::Match => 0.003,
        AmmoGrade::Service => 0.008,
        AmmoGrade::Surplus => 0.015,
        AmmoGrade::Handload => 0.005,
    }
}

/// Between-lot MV sigma multiplier relative to within-grade sigma.
/// Between-lot spread is typically 2-3x the within-grade sigma.
fn between_lot_multiplier() -> f64 {
    2.5
}

// ── Golden-angle deterministic offset ──────────────────────────────────────

/// Get deterministic lot offset for a given lot index.
///
/// Uses golden-angle low-discrepancy sequence for reproducibility.
/// The golden ratio φ = (1 + √5) / 2 provides uniform coverage of
/// the parameter space — same lot_index always gives the same offset.
///
/// # Returns
/// `(mv_delta, bc_delta)` — offsets to apply to nominal MV and BC.
pub fn lot_offset(lot_index: i32, grade: AmmoGrade, temperature_c: f64) -> (f64, f64) {
    let phi = 1.618_033_988_749_895; // golden ratio
    let index = lot_index as f64;

    // ── MV offset: Irwin-Hall(3) approximation of Gaussian ──────────
    // Sum of 3 uniform[-1, 1] → Irwin-Hall(3), mean 0, variance 1.
    let ih_seed = index * phi;
    let u1 = (ih_seed.fract() - 0.5) * 2.0;
    let u2 = ((ih_seed * 1.3).fract() - 0.5) * 2.0;
    let u3 = ((ih_seed * 2.1).fract() - 0.5) * 2.0;
    let ih_sum = (u1 + u2 + u3) / 3.0; // Irwin-Hall(3), scaled ≈ N(0,1)

    // Between-lot MV variation scales with grade sigma
    let mv_sigma = grade_sigma_mv(grade) * between_lot_multiplier();
    let mv_delta = ih_sum * mv_sigma;

    // Temperature effect on MV baked into the offset
    // ponytail: 20°C reference; upgrade to configurable if needed
    let temp_effect = temperature_velocity_effect(temperature_c, 20.0);
    let mv_delta = mv_delta + temp_effect;

    // ── BC offset: golden-angle uniform in ±grade_sigma_bc ──────────
    let bc_phase = (index * phi * 0.7).fract();
    let bc_uniform = (bc_phase - 0.5) * 2.0;
    let bc_sigma = grade_sigma_bc(grade) * between_lot_multiplier();
    // Scale so ±1 → ±2σ span (~95% within ±2σ)
    let bc_delta = bc_uniform * bc_sigma * 2.0;

    (mv_delta, bc_delta)
}

// ── Temperature effects ────────────────────────────────────────────────────

/// Temperature effect on muzzle velocity.
///
/// Propellant burns faster at higher temperature. For double-base powders
/// the rule of thumb is ~1 m/s per 10°C deviation from reference.
///
/// Returns an offset to add to muzzle velocity (m/s). Positive at
/// temperatures above reference (hotter → faster burn → higher MV).
pub fn temperature_velocity_effect(temp_c: f64, reference_temp_c: f64) -> f64 {
    (temp_c - reference_temp_c) / 10.0
}

// ── Hot-lot probability ────────────────────────────────────────────────────

/// Probability that a randomly selected lot is "hot" — i.e., its mean MV
/// exceeds nominal + 2σ (the upper tail of the between-lot distribution).
///
/// Under a Gaussian model this would be ~2.3%, but real ammunition lots
/// exhibit heavier tails. The returned values are empirical estimates.
pub fn hot_lot_probability(grade: AmmoGrade) -> f64 {
    match grade {
        AmmoGrade::Match => 0.035,
        AmmoGrade::Service => 0.080,
        AmmoGrade::Surplus => 0.180,
        AmmoGrade::Handload => 0.060,
    }
}

// ── Statistics generation ──────────────────────────────────────────────────

/// Generate expected lot variation statistics for the given parameters.
///
/// Produces `lot_count` samples, each with a deterministic (MV, BC) offset
/// computed via golden-angle sampling. Returns mean, std, and the sample
/// vector.
pub fn lot_variation_statistics(params: &LotVariationParams) -> LotStatistics {
    let count = params.lot_count.max(1);

    let samples: Vec<(f64, f64)> = (0..count)
        .map(|i| {
            let (mv_delta, bc_delta) = lot_offset(i, params.grade, params.temperature_c);
            let mv = params.nominal_mv_ms + mv_delta;
            let bc = (params.nominal_bc + bc_delta).max(0.001); // BC cannot be negative
            (mv, bc)
        })
        .collect();

    // Compute statistics
    let n = samples.len() as f64;
    let mean_mv = samples.iter().map(|s| s.0).sum::<f64>() / n;
    let mean_bc = samples.iter().map(|s| s.1).sum::<f64>() / n;

    let variance_mv = samples.iter().map(|s| (s.0 - mean_mv).powi(2)).sum::<f64>() / n;
    let variance_bc = samples.iter().map(|s| (s.1 - mean_bc).powi(2)).sum::<f64>() / n;

    let std_mv = variance_mv.sqrt();
    let std_bc = variance_bc.sqrt();

    LotStatistics {
        mean_mv_ms: mean_mv,
        std_mv_ms: std_mv,
        mean_bc: mean_bc,
        std_bc: std_bc,
        lot_samples: samples,
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn match_params() -> LotVariationParams {
        LotVariationParams {
            grade: AmmoGrade::Match,
            nominal_mv_ms: 900.0,
            nominal_bc: 0.250,
            temperature_c: 20.0,
            lot_count: 50,
        }
    }

    fn service_params() -> LotVariationParams {
        LotVariationParams {
            grade: AmmoGrade::Service,
            nominal_mv_ms: 850.0,
            nominal_bc: 0.200,
            temperature_c: 20.0,
            lot_count: 50,
        }
    }

    // ── Determinism ─────────────────────────────────────────────────────

    #[test]
    fn lot_offset_is_deterministic() {
        let (mv1, bc1) = lot_offset(7, AmmoGrade::Match, 20.0);
        let (mv2, bc2) = lot_offset(7, AmmoGrade::Match, 20.0);
        assert!((mv1 - mv2).abs() < 1e-12, "Same lot_index → same MV");
        assert!((bc1 - bc2).abs() < 1e-12, "Same lot_index → same BC");
    }

    #[test]
    fn different_lots_give_different_offsets() {
        let (mv0, _) = lot_offset(0, AmmoGrade::Service, 20.0);
        let (mv1, _) = lot_offset(1, AmmoGrade::Service, 20.0);
        assert!(
            (mv0 - mv1).abs() > 1e-12,
            "Different lot_index should give different MV"
        );
    }

    #[test]
    fn statistics_are_deterministic() {
        let p = match_params();
        let s1 = lot_variation_statistics(&p);
        let s2 = lot_variation_statistics(&p);
        assert_eq!(s1.mean_mv_ms, s2.mean_mv_ms);
        assert_eq!(s1.std_mv_ms, s2.std_mv_ms);
        assert_eq!(s1.lot_samples.len(), s2.lot_samples.len());
        for (a, b) in s1.lot_samples.iter().zip(s2.lot_samples.iter()) {
            assert!((a.0 - b.0).abs() < 1e-12);
            assert!((a.1 - b.1).abs() < 1e-12);
        }
    }

    // ── Grade spreads ──────────────────────────────────────────────────

    #[test]
    fn match_grade_has_smallest_spread() {
        let match_s = lot_variation_statistics(&match_params());
        let service_s = lot_variation_statistics(&service_params());

        assert!(
            match_s.std_mv_ms < service_s.std_mv_ms,
            "Match grade MV std {:.3} should be smaller than service {:.3}",
            match_s.std_mv_ms,
            service_s.std_mv_ms
        );
        assert!(
            match_s.std_bc < service_s.std_bc,
            "Match grade BC std {:.5} should be smaller than service {:.5}",
            match_s.std_bc,
            service_s.std_bc
        );
    }

    #[test]
    fn surplus_grade_has_largest_spread() {
        let surplus_params = LotVariationParams {
            grade: AmmoGrade::Surplus,
            ..match_params()
        };
        let match_s = lot_variation_statistics(&match_params());
        let surplus_s = lot_variation_statistics(&surplus_params);

        assert!(
            surplus_s.std_mv_ms > match_s.std_mv_ms,
            "Surplus MV std {:.3} should be larger than match {:.3}",
            surplus_s.std_mv_ms,
            match_s.std_mv_ms
        );
        assert!(
            surplus_s.std_bc > match_s.std_bc,
            "Surplus BC std {:.5} should be larger than match {:.5}",
            surplus_s.std_bc,
            match_s.std_bc
        );
    }

    #[test]
    fn handload_spread_between_match_and_service() {
        let handload_params = LotVariationParams {
            grade: AmmoGrade::Handload,
            ..match_params()
        };
        let match_s = lot_variation_statistics(&match_params());
        let service_s = lot_variation_statistics(&service_params());
        let handload_s = lot_variation_statistics(&handload_params);

        assert!(
            handload_s.std_mv_ms >= match_s.std_mv_ms,
            "Handload MV std should be >= match"
        );
        assert!(
            handload_s.std_mv_ms <= service_s.std_mv_ms,
            "Handload MV std should be <= service"
        );
    }

    // ── Temperature effects ────────────────────────────────────────────

    #[test]
    fn temperature_effect_zero_at_reference() {
        let effect = temperature_velocity_effect(20.0, 20.0);
        assert!(
            effect.abs() < 1e-12,
            "Temperature effect should be zero at reference temperature"
        );
    }

    #[test]
    fn temperature_effect_increases_mv_at_hotter_temp() {
        let cold = temperature_velocity_effect(0.0, 20.0);
        let hot = temperature_velocity_effect(40.0, 20.0);
        assert!(hot > cold, "Hotter temp should give higher MV offset");
        // ~1 m/s per 10°C: 20°C swing → ~2 m/s
        assert!(
            (hot - cold - 4.0).abs() < 0.01,
            "40° swing should give ~4 m/s delta"
        );
    }

    #[test]
    fn hot_temperature_shifts_mean_mv_up() {
        let cold_params = LotVariationParams {
            temperature_c: 0.0,
            ..match_params()
        };
        let hot_params = LotVariationParams {
            temperature_c: 40.0,
            ..match_params()
        };
        let cold_s = lot_variation_statistics(&cold_params);
        let hot_s = lot_variation_statistics(&hot_params);

        assert!(
            hot_s.mean_mv_ms > cold_s.mean_mv_ms,
            "Hot temp should raise mean MV: {:.3} vs {:.3}",
            hot_s.mean_mv_ms,
            cold_s.mean_mv_ms
        );
    }

    // ── Hot-lot probability ────────────────────────────────────────────

    #[test]
    fn hot_lot_probability_ordering() {
        assert!(
            hot_lot_probability(AmmoGrade::Match) < hot_lot_probability(AmmoGrade::Service),
            "Match should have lower hot-lot probability than Service"
        );
        assert!(
            hot_lot_probability(AmmoGrade::Service) < hot_lot_probability(AmmoGrade::Surplus),
            "Service should have lower hot-lot probability than Surplus"
        );
    }

    #[test]
    fn hot_lot_probability_in_range() {
        for grade in &[
            AmmoGrade::Match,
            AmmoGrade::Service,
            AmmoGrade::Surplus,
            AmmoGrade::Handload,
        ] {
            let p = hot_lot_probability(*grade);
            assert!(
                p > 0.0 && p < 1.0,
                "Hot-lot probability for {:?} ({:.4}) not in (0,1)",
                grade,
                p
            );
        }
    }

    // ── Statistics invariants ──────────────────────────────────────────

    #[test]
    fn lot_statistics_have_correct_sample_count() {
        let p = LotVariationParams {
            lot_count: 100,
            ..match_params()
        };
        let s = lot_variation_statistics(&p);
        assert_eq!(s.lot_samples.len(), 100);
    }

    #[test]
    fn mean_mv_is_near_nominal() {
        // With 5000 lots the sample mean should converge close to nominal
        let p = LotVariationParams {
            lot_count: 5000,
            ..match_params()
        };
        let s = lot_variation_statistics(&p);
        let deviation = (s.mean_mv_ms - p.nominal_mv_ms).abs();
        assert!(
            deviation < 1.0,
            "Mean MV {:.3} should be within 1 m/s of nominal {:.3} (dev={:.3})",
            s.mean_mv_ms,
            p.nominal_mv_ms,
            deviation
        );
    }

    #[test]
    fn bc_values_remain_positive() {
        let p = LotVariationParams {
            grade: AmmoGrade::Surplus,
            nominal_bc: 0.050,
            lot_count: 200,
            ..match_params()
        };
        let s = lot_variation_statistics(&p);
        for (_, bc) in &s.lot_samples {
            assert!(*bc > 0.0, "BC must be positive, got {}", bc);
        }
    }

    #[test]
    fn single_lot_produces_one_sample() {
        let p = LotVariationParams {
            lot_count: 1,
            ..match_params()
        };
        let s = lot_variation_statistics(&p);
        assert_eq!(s.lot_samples.len(), 1);
        // Single lot should have meaningful values
        assert!(s.lot_samples[0].0 > 0.0);
        assert!(s.lot_samples[0].1 > 0.0);
    }

    #[test]
    fn lot_offset_differs_by_grade() {
        let (mv_match, bc_match) = lot_offset(5, AmmoGrade::Match, 20.0);
        let (mv_surplus, bc_surplus) = lot_offset(5, AmmoGrade::Surplus, 20.0);
        // Different grade sigmas should produce different offsets
        // (same lot_index, same temp → different grade → different offset)
        assert!(
            (mv_match - mv_surplus).abs() > 1e-12 || (bc_match - bc_surplus).abs() > 1e-12,
            "Same lot_index different grade should give different offsets"
        );
    }
}
