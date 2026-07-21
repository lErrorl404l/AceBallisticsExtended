// ABE - APFSDS Long-Rod Penetrator Model
//
// Extends the Lanz-Odermatt formula (see penetration.rs) with finite rod
// length, L/D ratio efficiency, rod erosion, and crater diameter estimation.
//
// References:
//   - Lanz & Odermatt, "Penetration of Long Rods" (1996, 1999)
//   - Rosenberg & Dekel, "Terminal Ballistics" (2nd ed., Springer, 2016)
//   - Anderson et al., "Long Rod Penetration" (Int. J. Impact Eng., 2006)

/// Parameters for an APFSDS long-rod penetrator impact.
#[derive(Debug, Clone, Copy)]
pub struct LongRodParams {
    /// Rod length (mm)
    pub rod_length_mm: f64,
    /// Rod diameter (mm)
    pub rod_diameter_mm: f64,
    /// Rod material density (kg/m³)
    pub rod_density_kgm3: f64,
    /// Impact velocity (m/s)
    pub impact_velocity_ms: f64,
    /// Impact angle from normal (degrees, 0 = perpendicular)
    pub impact_angle_deg: f64,
    /// Target material density (kg/m³)
    pub target_density_kgm3: f64,
    /// Target yield strength (MPa)
    pub target_yield_strength_m_pa: f64,
    /// Rod fineness ratio (L/D). Set to 0.0 for auto-compute from length/diameter.
    pub rod_fineness_ratio: f64,
}

/// Result of an APFSDS long-rod penetration evaluation.
#[derive(Debug, Clone, Copy)]
pub struct LongRodPenetrationResult {
    /// Total penetration depth (mm)
    pub penetration_depth_mm: f64,
    /// Residual rod length after erosion (mm). Zero if fully eroded.
    pub residual_rod_length_mm: f64,
    /// Whether the rod was fully consumed before the penetration stopped
    pub rod_eroded: bool,
    /// Penetration efficiency factor (0.0–1.0) based on L/D ratio
    pub penetration_efficiency: f64,
    /// Crater diameter at the impact surface (mm)
    pub crater_diameter_mm: f64,
}

/// Evaluate penetration of a long-rod APFSDS penetrator using an enhanced
/// Lanz-Odermatt model with finite-length effects.
///
/// The model extends the base P/L formula with:
///
/// * **L/D ratio efficiency** — longer, thinner rods penetrate more
///   efficiently. The base Lanz-Odermatt material constant `k` is scaled
///   inversely with an efficiency factor `η(L/D)`.
///
/// * **Finite rod erosion** — the rod erodes (shortens) as it penetrates.
///   If the rod is consumed before reaching the Lanz-Odermatt penetration
///   depth, the actual depth is limited by the erosion model.
///
/// * **Crater diameter** — estimated from rod diameter and velocity ratio.
pub fn evaluate_long_rod(params: &LongRodParams) -> LongRodPenetrationResult {
    // Resolve L/D ratio
    let ld_ratio = if params.rod_fineness_ratio > 0.0 {
        params.rod_fineness_ratio
    } else if params.rod_diameter_mm > 0.0 {
        params.rod_length_mm / params.rod_diameter_mm
    } else {
        10.0 // sensible default
    };

    // Minimum eroding velocity from target strength
    // Dynamic flow stress ≈ 2× static yield; V_min = sqrt(2 · Y_dyn / ρₚ)
    let flow_stress_pa = params.target_yield_strength_m_pa * 2.0e6;
    let v_min_ms = if params.rod_density_kgm3 > 0.0 {
        (2.0 * flow_stress_pa / params.rod_density_kgm3).sqrt()
    } else {
        700.0
    };

    // L/D efficiency: η = 1.0 − 0.15 · exp(−L/D · 0.15)
    //   L/D = 10  → η ≈ 0.867
    //   L/D = 20  → η ≈ 0.926
    //   L/D = 30  → η → 0.951
    //   L/D → ∞   → η → 1.000
    let efficiency = 1.0 - 0.15 * (-ld_ratio * 0.15_f64).exp();

    // Adjust k inversely with efficiency so longer rods (higher η)
    // yield a lower effective k → more penetration
    let k = 2.0 / efficiency;

    // P/L from the enhanced Lanz-Odermatt formula
    let p_over_l = lanz_odermatt_depth_with_ld(
        params.impact_velocity_ms,
        v_min_ms,
        params.rod_density_kgm3,
        params.target_density_kgm3,
        k,
        params.impact_angle_deg,
        2.0, // angle exponent n for long rods
        ld_ratio,
    );

    // Rod length in metres for internal computation
    let rod_length_m = params.rod_length_mm / 1000.0;

    // Un-capped Lanz-Odermatt penetration depth (metres).
    // Apply the V3 L/D correction factor for high-fineness-ratio rods
    // (L/D > 30). For conventional L/D ≤ 30 the factor is 1.0 (no change).
    let v3_factor = crate::penetration::lanz_odermatt_v3_factor(ld_ratio);
    let max_penetration_m = p_over_l * rod_length_m * v3_factor;

    // Erosion-limited penetration depth using the simplified erosion model
    // k_erosion = 1.0 gives comparable magnitude to P/L for typical APFSDS;
    // higher values make erosion more restrictive.
    let k_erosion = 1.0;
    let erosion_depth_m = rod_erosion_depth(
        params.impact_velocity_ms,
        params.rod_density_kgm3,
        params.target_density_kgm3,
        rod_length_m,
        k_erosion,
    );

    // Determine actual penetration and residual rod length
    let (penetration_m, rod_eroded, residual_m) = if p_over_l <= 0.0 {
        (0.0, false, rod_length_m)
    } else if erosion_depth_m >= max_penetration_m {
        // Rod not fully eroded → achieves full Lanz-Odermatt depth
        let consumed_fraction = max_penetration_m / erosion_depth_m.max(1e-12);
        (
            max_penetration_m,
            false,
            rod_length_m * (1.0 - consumed_fraction).max(0.0),
        )
    } else {
        // Rod fully consumed before reaching Lanz-Odermatt depth
        (erosion_depth_m, true, 0.0)
    };

    // Crater diameter at the impact surface
    let crater_mm = crater_diameter(params.rod_diameter_mm, params.impact_velocity_ms, v_min_ms);

    LongRodPenetrationResult {
        penetration_depth_mm: penetration_m * 1000.0,
        residual_rod_length_mm: residual_m * 1000.0,
        rod_eroded,
        penetration_efficiency: efficiency,
        crater_diameter_mm: crater_mm,
    }
}

/// Compute the normalised penetration ratio P/L using the Lanz-Odermatt
/// formula, accepting an L/D ratio parameter for interface compatibility
/// with the enhanced model.
///
/// This function delegates to the same core formula as `lanz_odermatt_depth`
/// in `penetration.rs`. The `ld_ratio` parameter is accepted but not used
/// directly in the formula body — the caller (`evaluate_long_rod`) adjusts
/// the material constant `k` via the efficiency factor before calling.
///
/// # Formula
/// ```text
/// P/L = sqrt(ρₚ / ρₜ) · (v² − vₘᵢₙ²) / (k · cos(θ)ⁿ)
/// ```
///
/// where dimensions are in km/s internally.
#[allow(clippy::too_many_arguments)]
// ponytail: physics kernel, all params required
pub fn lanz_odermatt_depth_with_ld(
    velocity_ms: f64,
    v_min_ms: f64,
    rho_p: f64,
    rho_t: f64,
    k: f64,
    angle_deg: f64,
    n: f64,
    _ld_ratio: f64,
) -> f64 {
    if velocity_ms <= 0.0 || rho_t <= 0.0 || k <= 0.0 {
        return 0.0;
    }

    let cos_angle = angle_deg.to_radians().cos().max(0.087); // clamp at ~85°
    let v_km = velocity_ms / 1000.0;
    let v_min_km = v_min_ms / 1000.0;

    (rho_p / rho_t).sqrt() * (v_km.powi(2) - v_min_km.powi(2)) / (k * cos_angle.powf(n))
}

/// Compute the depth at which a finite-length rod is fully eroded.
///
/// Simplified model based on the density ratio and velocity above
/// the erosion threshold:
///
/// ```text
/// p_erode = L · sqrt(ρₚ / ρₜ) · (v / v_ref − 1) / k_erosion
/// ```
///
/// where `v_ref = 700 m/s` is a reference minimum eroding velocity.
/// Returns zero when v ≤ v_ref.
pub fn rod_erosion_depth(
    velocity_ms: f64,
    rho_p: f64,
    rho_t: f64,
    rod_length_m: f64,
    k_erosion: f64,
) -> f64 {
    if velocity_ms <= 0.0 || rho_t <= 0.0 || rod_length_m <= 0.0 || k_erosion <= 0.0 {
        return 0.0;
    }

    let v_above_threshold = (velocity_ms / 700.0 - 1.0).max(0.0);
    rod_length_m * (rho_p / rho_t).sqrt() * v_above_threshold / k_erosion
}

/// Estimate the crater diameter at the impact surface.
///
/// The crater scales linearly with rod diameter and with the square
/// root of the normalised velocity ratio.  Typical craters range from
/// 1.5× rod diameter (near threshold) to ~4.5× (high velocity).
fn crater_diameter(rod_diameter_mm: f64, velocity_ms: f64, v_min_ms: f64) -> f64 {
    if rod_diameter_mm <= 0.0 {
        return 0.0;
    }
    let v_ratio = (velocity_ms / v_min_ms.max(1.0)).sqrt();
    rod_diameter_mm * 1.5 * v_ratio.min(3.0)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn high_ld_penetrates_more_than_low_ld() {
        // L/D efficiency: η = 1.0 − 0.15·exp(−L/D·0.15)
        // L/D=10 → η ≈ 0.967, L/D=30 → η ≈ 0.998 → effective k differs
        let p_low = lanz_odermatt_depth_with_ld(
            1600.0,
            428.0,
            17500.0,
            7850.0,
            2.0 / 0.9665,
            0.0,
            2.0,
            10.0,
        );
        let p_high = lanz_odermatt_depth_with_ld(
            1600.0,
            428.0,
            17500.0,
            7850.0,
            2.0 / 0.9983,
            0.0,
            2.0,
            30.0,
        );
        assert!(
            p_high > p_low,
            "Higher L/D should give larger P/L: low={p_low:.4}, high={p_high:.4}",
        );

        // Efficiency also differs through evaluate_long_rod
        let low_rod = evaluate_long_rod(&LongRodParams {
            rod_length_mm: 400.0,
            rod_diameter_mm: 40.0,
            rod_density_kgm3: 17500.0,
            impact_velocity_ms: 1600.0,
            impact_angle_deg: 0.0,
            target_density_kgm3: 7850.0,
            target_yield_strength_m_pa: 800.0,
            rod_fineness_ratio: 10.0,
        });
        let high_rod = evaluate_long_rod(&LongRodParams {
            rod_length_mm: 400.0,
            rod_diameter_mm: 400.0 / 30.0,
            rod_density_kgm3: 17500.0,
            impact_velocity_ms: 1600.0,
            impact_angle_deg: 0.0,
            target_density_kgm3: 7850.0,
            target_yield_strength_m_pa: 800.0,
            rod_fineness_ratio: 30.0,
        });
        assert!(
            high_rod.penetration_efficiency > low_rod.penetration_efficiency,
            "Higher L/D should increase efficiency: low={:.4}, high={:.4}",
            low_rod.penetration_efficiency,
            high_rod.penetration_efficiency,
        );
        assert!(
            high_rod.penetration_depth_mm > low_rod.penetration_depth_mm,
            "Higher L/D should penetrate deeper: low={:.1}mm, high={:.1}mm",
            low_rod.penetration_depth_mm,
            high_rod.penetration_depth_mm,
        );
    }

    #[test]
    fn finite_rod_fully_erodes() {
        // Short rod at low velocity vs strong target → fully eroded
        let result = evaluate_long_rod(&LongRodParams {
            rod_length_mm: 200.0,
            rod_diameter_mm: 30.0,
            rod_density_kgm3: 17500.0,
            impact_velocity_ms: 800.0,
            impact_angle_deg: 0.0,
            target_density_kgm3: 7850.0,
            target_yield_strength_m_pa: 1000.0,
            rod_fineness_ratio: 0.0,
        });

        assert!(
            result.rod_eroded,
            "Short rod at 800 m/s should fully erode against strong target"
        );
        assert!(
            result.residual_rod_length_mm < 1.0,
            "Fully eroded rod should have near-zero residual: {:.1}mm",
            result.residual_rod_length_mm,
        );
    }

    #[test]
    fn long_rod_not_fully_eroded() {
        // Long, high-density rod at high velocity → not fully eroded
        let result = evaluate_long_rod(&LongRodParams {
            rod_length_mm: 800.0,
            rod_diameter_mm: 20.0,
            rod_density_kgm3: 19100.0,
            impact_velocity_ms: 1700.0,
            impact_angle_deg: 0.0,
            target_density_kgm3: 7850.0,
            target_yield_strength_m_pa: 800.0,
            rod_fineness_ratio: 40.0,
        });

        assert!(
            !result.rod_eroded,
            "Long rod at 1700 m/s should NOT fully erode"
        );
        assert!(
            result.residual_rod_length_mm > 0.0,
            "Residual rod should exist: {:.1}mm",
            result.residual_rod_length_mm
        );
        assert!(
            result.penetration_depth_mm > 100.0,
            "Long rod at high velocity should penetrate significantly: {:.1}mm",
            result.penetration_depth_mm
        );
    }

    #[test]
    fn lanz_odermatt_with_ld_follows_angle_trend() {
        // The Lanz-Odermatt formula has cosⁿ in the denominator, so P/L
        // increases at oblique angles (the long-rod tunnelling effect).
        // Verify this holds in the enhanced formula as well.
        let at_0 = lanz_odermatt_depth_with_ld(1600.0, 700.0, 17500.0, 7850.0, 2.0, 0.0, 2.0, 20.0);
        let at_60 =
            lanz_odermatt_depth_with_ld(1600.0, 700.0, 17500.0, 7850.0, 2.0, 60.0, 2.0, 20.0);

        // cos(60°) = 0.5, cos² = 0.25 → ratio ≈ 4.0
        assert!(
            at_60 > at_0,
            "P/L at 60° should be greater than at 0° (tunnelling effect): \
             at_0={at_0:.4}, at_60={at_60:.4}"
        );
        let ratio = at_60 / at_0;
        assert!(
            (ratio - 4.0).abs() < 0.001,
            "P/L at 60° should be ~4× 0° for n=2: ratio={ratio:.4}"
        );
    }

    #[test]
    fn rod_erosion_depth_scales_with_velocity() {
        // Higher velocity → greater erosion depth
        let low_v = rod_erosion_depth(1000.0, 17500.0, 7850.0, 0.6, 2.5);
        let high_v = rod_erosion_depth(1600.0, 17500.0, 7850.0, 0.6, 2.5);

        assert!(
            high_v > low_v,
            "Higher velocity should increase erosion depth: low={low_v:.4}m, high={high_v:.4}m"
        );
        assert!(
            low_v > 0.0,
            "Erosion depth should be positive above threshold"
        );
    }

    #[test]
    fn crater_diameter_increases_with_velocity() {
        let low_v = crater_diameter(30.0, 800.0, 700.0);
        let high_v = crater_diameter(30.0, 1600.0, 700.0);

        assert!(
            high_v > low_v,
            "Higher velocity should increase crater diameter: low={low_v:.1}mm, high={high_v:.1}mm"
        );
        assert!(
            low_v >= 30.0 * 1.5,
            "Crater should be at least 1.5× rod diameter at threshold"
        );
    }
}
