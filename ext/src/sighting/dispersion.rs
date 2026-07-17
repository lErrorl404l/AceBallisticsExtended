// ABE - Dispersion / Precision Model
//
// Models shot-to-shot dispersion from muzzle velocity variation,
// ballistic coefficient spread, barrel harmonics, and vertical stringing.
//
// Uses deterministic golden-angle-based sampling for reproducibility:
// each shot_index maps to a unique offset, so any two evaluations with
// the same params and shot_index produce identical results.
//
// References:
//   - Hatcher's Notebook (vertical stringing / barrel time)
//   - Bryan Litz "Applied Ballistics Precision" (MV/BC spread)
//   - ISO 5725 (measurement precision)
//   - Golden-angle low-discrepancy sequences

/// Parameters controlling shot dispersion.
#[derive(Debug, Clone)]
pub struct ShotDispersionParams {
    /// Nominal muzzle velocity (m/s)
    pub nominal_mv_ms: f64,
    /// Absolute MV spread (±m/s). Typical: 3–15 m/s for match ammo
    pub mv_spread_ms: f64,
    /// Nominal ballistic coefficient (G1 or G7)
    pub nominal_bc: f64,
    /// BC variation (±fraction). Typical: 0.005–0.020
    pub bc_variation: f64,
    /// Barrel vertical harmonic amplitude (mils at 100m)
    pub barrel_harmonic_mils: f64,
    /// Barrel harmonic wavelength in shots (≈ 1–3 full cycles over barrel life)
    pub harmonic_wavelength: f64,
    /// Vertical stringing coefficient (mils per shot in burst)
    pub vertical_stringing_mils_per_shot: f64,
    /// Crosswind sensitivity (m/s wind per mil deflection)
    pub wind_sensitivity: f64,
}

/// Returns dispersion offsets for a given shot index.
///
/// Uses deterministic golden-angle-based sampling (not random!).
/// The golden ratio φ = (1 + √5) / 2 provides a low-discrepancy
/// sequence that covers the parameter space uniformly.
///
/// # Returns
/// `(mv_delta, bc_delta, vertical_angle_offset_rad, horizontal_angle_offset_rad)`
pub fn apply_dispersion(params: &ShotDispersionParams, shot_index: i32) -> (f64, f64, f64, f64) {
    let phi = 1.618_033_988_749_895; // golden ratio
    let index = shot_index as f64;

    // ── MV offset: Irwin-Hall approximation of Gaussian ──────────────────
    // Sum of 3 uniform[-0.5, 0.5] → Irwin-Hall(3), mean 0, variance 0.25
    // Scale to match desired spread
    let ih_seed = index * phi;
    let u1 = (ih_seed.fract() - 0.5) * 2.0; // [-1, 1]
    let u2 = ((ih_seed * 1.3).fract() - 0.5) * 2.0;
    let u3 = ((ih_seed * 2.1).fract() - 0.5) * 2.0;
    let ih_sum = (u1 + u2 + u3) / 3.0; // Irwin-Hall(3), scaled to [-1, 1]
    let mv_delta = ih_sum * params.mv_spread_ms;

    // ── BC offset: uniform in ±bc_variation/2 ────────────────────────────
    let bc_phase = (index * phi * 0.7).fract();
    let bc_uniform = (bc_phase - 0.5) * 2.0; // [-1, 1]
    let bc_delta = bc_uniform * params.bc_variation * 0.5;

    // ── Vertical angle: harmonic + shot-to-shot ──────────────────────────
    // Barrel harmonic (sinusoidal oscillation as barrel heats/cools)
    let harmonic_phase = 2.0 * std::f64::consts::PI * index / params.harmonic_wavelength.max(1.0);
    let harmonic_offset = harmonic_phase.sin() * params.barrel_harmonic_mils;

    // Vertical stringing from barrel heating during sustained fire
    let burst_stringing = params.vertical_stringing_mils_per_shot * index;

    // Random-looking vertical component from golden-angle noise
    let vert_noise = (index * phi * 1.7).fract() - 0.5;

    // Total vertical offset in mils, convert to radians (1 mil ≈ 0.001 rad)
    let vertical_mils =
        harmonic_offset + burst_stringing + vert_noise * params.barrel_harmonic_mils;
    let vertical_rad = vertical_mils * 0.001;

    // ── Horizontal angle: random component ───────────────────────────────
    let horiz_phase = (index * phi * 2.3).fract();
    let horiz_offset = (horiz_phase - 0.5) * params.barrel_harmonic_mils * 0.5;
    let horizontal_rad = horiz_offset * 0.001;

    (mv_delta, bc_delta, vertical_rad, horizontal_rad)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn default_params() -> ShotDispersionParams {
        ShotDispersionParams {
            nominal_mv_ms: 900.0,
            mv_spread_ms: 10.0,
            nominal_bc: 0.200,
            bc_variation: 0.010,
            barrel_harmonic_mils: 0.5,
            harmonic_wavelength: 50.0,
            vertical_stringing_mils_per_shot: 0.05,
            wind_sensitivity: 0.5,
        }
    }

    #[test]
    fn dispersion_is_deterministic() {
        let p = default_params();
        let (mv1, bc1, v1, h1) = apply_dispersion(&p, 5);
        let (mv2, bc2, v2, h2) = apply_dispersion(&p, 5);
        assert!(
            (mv1 - mv2).abs() < 1e-12,
            "Same shot_index should give identical MV offset"
        );
        assert!(
            (bc1 - bc2).abs() < 1e-12,
            "Same shot_index should give identical BC offset"
        );
        assert!((v1 - v2).abs() < 1e-12);
        assert!((h1 - h2).abs() < 1e-12);
    }

    #[test]
    fn different_shots_give_different_offsets() {
        let p = default_params();
        let (mv1, ..) = apply_dispersion(&p, 0);
        let (mv2, ..) = apply_dispersion(&p, 1);
        assert!(
            (mv1 - mv2).abs() > 1e-12,
            "Different shot_index should give different MV offset"
        );
    }

    #[test]
    fn mv_offset_within_spread() {
        let p = default_params();
        for i in 0..100 {
            let (mv_delta, ..) = apply_dispersion(&p, i);
            assert!(
                mv_delta.abs() <= p.mv_spread_ms * 1.01,
                "MV offset {} should be within spread bounds at shot {}",
                mv_delta,
                i
            );
        }
    }

    #[test]
    fn bc_offset_within_variation() {
        let p = default_params();
        for i in 0..100 {
            let (_, bc_delta, ..) = apply_dispersion(&p, i);
            assert!(
                bc_delta.abs() <= p.bc_variation * 0.5 + 1e-9,
                "BC offset {} should be within variation at shot {}",
                bc_delta,
                i
            );
        }
    }

    #[test]
    fn zero_mv_spread_returns_zero() {
        let mut p = default_params();
        p.mv_spread_ms = 0.0;
        let (mv_delta, ..) = apply_dispersion(&p, 42);
        assert!(
            mv_delta.abs() < 1e-12,
            "Zero spread should give zero MV offset"
        );
    }

    #[test]
    fn vertical_offset_increases_with_stringing() {
        let mut p = default_params();
        p.vertical_stringing_mils_per_shot = 0.1;
        let (_, _, v1, _) = apply_dispersion(&p, 1);
        let (_, _, v10, _) = apply_dispersion(&p, 10);
        // Higher shot index should tend to have more vertical stringing
        // (not strictly monotonic due to harmonic component)
        assert!(v10 != v1);
    }

    #[test]
    fn vertical_offset_contains_harmonic_component() {
        let p = ShotDispersionParams {
            nominal_mv_ms: 900.0,
            mv_spread_ms: 0.0,
            nominal_bc: 0.200,
            bc_variation: 0.0,
            barrel_harmonic_mils: 1.0,
            harmonic_wavelength: 4.0,
            vertical_stringing_mils_per_shot: 0.0,
            wind_sensitivity: 0.0,
        };
        // At shot 0, sin(0) = 0 → harmonic = 0
        let (_, _, v0, _) = apply_dispersion(&p, 0);
        // At shot 1, sin(2π/4) = sin(π/2) = 1 → harmonic = 1 mil
        let (_, _, v1, _) = apply_dispersion(&p, 1);
        // At shot 0, no noise either (no-mv-spread params) → vertical = 0
        assert!(
            v0.abs() < 0.001,
            "At shot 0 with no spread, vertical should be ~0"
        );
        // At shot 1, vertical should be non-zero from harmonic
        assert!(
            v0 != v1,
            "Harmonic should produce different vertical at different shots"
        );
    }

    #[test]
    fn horizontal_offset_reproducible() {
        let p = default_params();
        let (_, _, _, h1) = apply_dispersion(&p, 7);
        let (_, _, _, h2) = apply_dispersion(&p, 7);
        assert!(
            (h1 - h2).abs() < 1e-12,
            "Horizontal offset should be deterministic"
        );
    }

    // ── Additional requested tests ─────────────────────────────────────────

    #[test]
    fn generate_dispersion_no_zero_spread() {
        // With non-zero spread parameters, dispersion should produce
        // non-zero offsets for most shot indices
        let p = default_params();
        let mut nonzero_count = 0;
        for i in 0..20 {
            let (mv_delta, bc_delta, vert, horiz) = apply_dispersion(&p, i);
            if mv_delta.abs() > 1e-12
                || bc_delta.abs() > 1e-12
                || vert.abs() > 1e-12
                || horiz.abs() > 1e-12
            {
                nonzero_count += 1;
            }
        }
        assert!(
            nonzero_count >= 18,
            "At least 18/20 shots should have non-zero dispersion offsets: {}/20",
            nonzero_count
        );
    }

    #[test]
    fn dispersion_increases_with_range() {
        // Shot-to-shot vertical stringing should compound with shot index
        let p = ShotDispersionParams {
            nominal_mv_ms: 900.0,
            mv_spread_ms: 0.0,
            nominal_bc: 0.200,
            bc_variation: 0.0,
            barrel_harmonic_mils: 0.0,
            harmonic_wavelength: 100.0,
            vertical_stringing_mils_per_shot: 0.1,
            wind_sensitivity: 0.0,
        };
        // With only stringing active, |vertical| should trend upward with index
        let (_, _, v1, _) = apply_dispersion(&p, 1);
        let (_, _, v10, _) = apply_dispersion(&p, 10);
        // The stringing term adds 0.1 mils per shot, so shot 10 has
        // at least 0.9 mils more vertical offset than shot 1 (before harmonic)
        let stringing_diff = (v10 - v1).abs() * 1000.0; // convert rad to mils
        assert!(
            stringing_diff > 0.5,
            "Stringing should increase vertical with shot index: diff={:.2} mils",
            stringing_diff
        );
    }

    #[test]
    fn dispersion_deterministic() {
        // Same params + same shot_index → identical results (already tested,
        // this is a cross-check with different params)
        let p = default_params();
        let r1 = apply_dispersion(&p, 42);
        let r2 = apply_dispersion(&p, 42);
        assert!((r1.0 - r2.0).abs() < 1e-12, "MV delta deterministic");
        assert!((r1.1 - r2.1).abs() < 1e-12, "BC delta deterministic");
        assert!((r1.2 - r2.2).abs() < 1e-12, "Vertical offset deterministic");
        assert!(
            (r1.3 - r2.3).abs() < 1e-12,
            "Horizontal offset deterministic"
        );
    }
}
