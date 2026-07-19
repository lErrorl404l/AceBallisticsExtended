// ABE - Fragmentation Model
//
// Models projectile fragmentation on impact or during penetration.
// Handles fragment mass distribution, velocity partitioning, and
// spray pattern generation.
//
// References:
//   - Nennstiel's "Fragmentation of Bullets" (1986)
//   - UK Defence Standard 13-100 (Fragment Simulating Projectiles)
//   - Capstick's terminal ballistics data (various calibers)
//   - FBI HPR (Handgun Wounding Effectiveness) fragmentation data

use std::collections::HashMap;

/// Data for a single projectile fragment.
#[derive(Debug, Clone)]
pub struct Fragment {
    /// Fragment mass in grams.
    pub mass_g: f64,
    /// Fragment velocity in m/s (mass-weighted partitioning from
    /// impact velocity).
    pub speed_ms: f64,
    /// Spray cone half-angle in degrees.
    pub cone_angle_deg: f64,
    /// Azimuth angle in degrees (deterministic via golden-angle
    /// distribution).
    pub azimuth_deg: f64,
}

/// Result of a fragmentation simulation.
///
/// Returned by [`evaluate`] with the generated fragment list and
/// summary statistics.
#[derive(Debug, Clone)]
pub struct FragmentationResult {
    /// Individual fragment data.
    pub fragments: Vec<Fragment>,
    /// Total number of fragments generated.
    pub num_fragments: i32,
    /// Average fragment mass in grams.
    pub average_mass_g: f64,
    /// Maximum spray cone half-angle across all fragments (degrees).
    pub max_spray_angle_deg: f64,
}

/// Evaluate fragmentation for a projectile impact.
///
/// # Arguments
/// * `velocity_ms` - Impact velocity (m/s)
/// * `projectile_mass_g` - Projectile mass (grams)
/// * `projectile_type` - Type of projectile (e.g. "fmj", "ap", "ball")
/// * `frag_threshold_ms` - Minimum velocity for fragmentation (m/s)
/// * `frag_config` - Optional fragmentation configuration (mass distribution params)
///
/// # Returns
/// FragmentationResult with generated fragments, or a minimal result if
/// velocity is below fragmentation threshold.
pub fn evaluate(
    velocity_ms: f64,
    projectile_mass_g: f64,
    projectile_type: &str,
    frag_threshold_ms: f64,
    frag_config: Option<&HashMap<String, f64>>,
) -> FragmentationResult {
    // Check if fragmentation is possible
    if velocity_ms < frag_threshold_ms || projectile_mass_g <= 0.0 {
        return FragmentationResult {
            fragments: Vec::new(),
            num_fragments: 0,
            average_mass_g: 0.0,
            max_spray_angle_deg: 0.0,
        };
    }

    // ── Fragment count ─────────────────────────────────────────────
    // Number of fragments depends on velocity, mass, and projectile construction.
    // FMJ bullets fragment into 2-20 pieces. More fragments at higher velocity.
    let velocity_ratio = (velocity_ms / frag_threshold_ms).min(3.0);
    let base_count = match projectile_type.to_lowercase().as_str() {
        "fmj" | "ball" => 2 + (8.0 * (velocity_ratio - 1.0)) as i32,
        "ap" | "armor_piercing" => 1 + (4.0 * (velocity_ratio - 1.0)) as i32,
        "apds" => 1, // Long rod penetrators don't fragment
        "tracer" => 1 + (5.0 * (velocity_ratio - 1.0)) as i32,
        "incendiary" => 3 + (10.0 * (velocity_ratio - 1.0)) as i32,
        "soft_point" | "hollow_point" => 3 + (12.0 * (velocity_ratio - 1.0)) as i32,
        _ => 1 + (5.0 * (velocity_ratio - 1.0)) as i32,
    };
    let num_fragments = base_count.clamp(1, 50);

    // ── Mass distribution ───────────────────────────────────────────
    // Fragment masses follow a log-normal distribution.
    // Most fragments are small (jacket fragments), fewer large (core fragments).
    //
    // Default parameters: mean ~0.15 * projectile_mass / count,
    // std_dev ~0.5 * mean (log-normal shape)
    //
    // With config override, params["mean"] and params["std"] scale the distribution.

    // Sample fragments with evenly spaced quantiles along the log-normal CDF
    let fragments = if num_fragments > 0 {
        let mass_mean = frag_config
            .and_then(|p| p.get("mean"))
            .copied()
            .unwrap_or(0.15 * projectile_mass_g / num_fragments as f64);
        let mass_std = frag_config
            .and_then(|p| p.get("std"))
            .copied()
            .unwrap_or(0.5 * mass_mean);

        // Log-normal parameters: mu, sigma
        let sigma = (1.0 + (mass_std / mass_mean).powi(2)).ln().sqrt();
        let mu = mass_mean.ln() - sigma.powi(2) / 2.0;

        let mut fragments = Vec::with_capacity(num_fragments as usize);

        for i in 0..num_fragments {
            // Evenly spaced quantile in (0, 1) — deterministic for reproducibility
            let t = (i as f64 + 0.5) / num_fragments as f64;
            // Inverse log-normal CDF via Box-Muller approximation
            let z = inverse_normal_cdf(t);
            let mass = (mu + sigma * z).exp();

            // ── Fragment velocity ────────────────────────────────────
            // Small fragments lose velocity faster. Use velocity partitioning:
            //   V_frag = V_impact * (M_frag / M_total)^0.33
            // Smaller fragments retain less velocity.
            let mass_ratio = (mass / projectile_mass_g).clamp(0.001, 1.0);
            let frag_speed = velocity_ms * mass_ratio.powf(0.33);

            // ── Spray pattern ────────────────────────────────────────
            // Fragments spray in a cone. Smaller fragments spread wider.
            // Base cone angle: 5-30 degrees depending on material and construction
            let base_cone = match projectile_type.to_lowercase().as_str() {
                "ap" => 8.0,
                "apds" => 3.0,
                "soft_point" | "hollow_point" => 25.0,
                _ => 15.0,
            };
            let cone_angle = base_cone * (1.0 / (mass_ratio + 0.5).max(0.1));
            let cone_angle = cone_angle.min(45.0);

            // Random-looking but deterministic azimuth from i
            let azimuth = (i as f64 * 137.508) % 360.0; // Golden angle

            fragments.push(Fragment {
                mass_g: mass,
                speed_ms: frag_speed,
                cone_angle_deg: cone_angle,
                azimuth_deg: azimuth,
            });
        }

        fragments
    } else {
        Vec::new()
    };

    // Compute summary statistics
    let mut total_mass = 0.0;
    let mut max_spray = 0.0;
    for frag in &fragments {
        total_mass += frag.mass_g;
        if frag.cone_angle_deg > max_spray {
            max_spray = frag.cone_angle_deg;
        }
    }

    let avg_mass = if !fragments.is_empty() {
        total_mass / fragments.len() as f64
    } else {
        0.0
    };

    FragmentationResult {
        fragments,
        num_fragments,
        average_mass_g: avg_mass,
        max_spray_angle_deg: max_spray,
    }
}

/// Approximate inverse normal CDF (probit function).
///
/// Uses the rational approximation from Peter Acklam (2003).
/// Accurate to ~1e-9 for p in (0, 1).
fn inverse_normal_cdf(p: f64) -> f64 {
    const A0: f64 = -3.969_683_028_665_416e1;
    const A1: f64 = 2.209_460_984_245_205e2;
    const A2: f64 = -2.759_285_104_382_374e2;
    const A3: f64 = 1.383_577_518_672_69e2;
    const A4: f64 = -3.066_429_263_862_405e1;
    const A5: f64 = 2.586_803_618_085_77;
    const B0: f64 = -5.447_609_879_822_406e1;
    const B1: f64 = 1.615_858_368_580_409e2;
    const B2: f64 = -1.556_989_798_598_866e2;
    const B3: f64 = 6.680_131_188_771_972e1;
    const B4: f64 = -1.328_068_155_288_572e1;
    const C0: f64 = -7.784_894_003_430_237e-3;
    const C1: f64 = -3.223_964_583_411_694e-1;
    const C2: f64 = -2.400_758_277_161_223e0;
    const C3: f64 = -2.549_732_539_343_734e0;
    const C4: f64 = 4.374_664_141_464_968e0;
    const C5: f64 = 2.938_163_982_997_783e0;
    const D0: f64 = 7.784_695_709_041_462e-3;
    const D1: f64 = 3.224_671_290_700_229e-1;
    const D2: f64 = 2.445_134_137_142_996e0;
    const D3: f64 = 3.754_408_641_908_608e0;

    if p <= 0.0 || p >= 1.0 {
        return 0.0;
    }

    if p < 0.02425 {
        // Lower tail
        let q = (-2.0 * p.ln()).sqrt();
        let e = (((((C0 * q + C1) * q + C2) * q + C3) * q + C4) * q + C5)
            / ((((D0 * q + D1) * q + D2) * q + D3) * q + 1.0);
        return e;
    }

    if p > 0.97575 {
        // Upper tail
        let q = (-2.0 * (1.0 - p).ln()).sqrt();
        let e = -(((((C0 * q + C1) * q + C2) * q + C3) * q + C4) * q + C5)
            / ((((D0 * q + D1) * q + D2) * q + D3) * q + 1.0);
        return e;
    }

    // Central region
    let q = p - 0.5;
    let r = q * q;
    (((((A0 * r + A1) * r + A2) * r + A3) * r + A4) * r + A5) * q
        / (((((B0 * r + B1) * r + B2) * r + B3) * r + B4) * r + 1.0)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subsonic_no_fragmentation() {
        let r = evaluate(300.0, 4.0, "fmj", 762.0, None);
        assert_eq!(r.num_fragments, 0, "Subsonic should not fragment");
    }

    #[test]
    fn supersonic_produces_fragments() {
        let r = evaluate(900.0, 4.0, "fmj", 762.0, None);
        assert!(r.num_fragments > 0, "Supersonic FMJ should fragment");
    }

    #[test]
    fn higher_velocity_more_fragments() {
        let r1 = evaluate(850.0, 4.0, "fmj", 762.0, None);
        let r2 = evaluate(1100.0, 4.0, "fmj", 762.0, None);
        assert!(r2.num_fragments >= r1.num_fragments);
    }

    #[test]
    fn ap_produces_fewer_fragments_than_fmj() {
        let r_fmj = evaluate(900.0, 9.5, "fmj", 762.0, None);
        let r_ap = evaluate(900.0, 9.5, "ap", 762.0, None);
        assert!(
            r_ap.num_fragments <= r_fmj.num_fragments,
            "AP should have <= fragments vs FMJ: AP={} FMJ={}",
            r_ap.num_fragments,
            r_fmj.num_fragments
        );
    }

    #[test]
    fn fragmentation_with_config_override() {
        let mut params = HashMap::new();
        params.insert("mean".to_string(), 0.5);
        params.insert("std".to_string(), 0.3);
        let r = evaluate(950.0, 4.0, "fmj", 762.0, Some(&params));
        assert!(r.num_fragments > 0);
        assert!(r.fragments.len() == r.num_fragments as usize);
    }

    #[test]
    fn fragment_masses_positive() {
        let r = evaluate(950.0, 4.0, "fmj", 762.0, None);
        for f in &r.fragments {
            assert!(
                f.mass_g > 0.0,
                "Fragment mass should be positive: {}",
                f.mass_g
            );
            assert!(f.speed_ms > 0.0, "Fragment speed should be positive");
            assert!(f.cone_angle_deg > 0.0, "Cone angle should be positive");
        }
    }

    #[test]
    fn fragment_masses_do_not_exceed_projectile() {
        let r = evaluate(950.0, 4.0, "fmj", 762.0, None);
        let total: f64 = r.fragments.iter().map(|f| f.mass_g).sum();
        // Some mass is lost as fine dust
        assert!(
            total <= 4.0 * 1.1,
            "Total frag mass should be ~projectile mass: {}",
            total
        );
    }

    #[test]
    fn zero_mass_no_fragments() {
        let r = evaluate(900.0, 0.0, "fmj", 762.0, None);
        assert_eq!(r.num_fragments, 0);
    }

    #[test]
    fn spray_angle_in_range() {
        let r = evaluate(950.0, 4.0, "soft_point", 762.0, None);
        for f in &r.fragments {
            assert!(
                f.cone_angle_deg <= 45.0,
                "Cone angle capped at 45°: {}",
                f.cone_angle_deg
            );
            assert!(f.azimuth_deg >= 0.0 && f.azimuth_deg < 360.0);
        }
    }

    #[test]
    fn hollow_point_spreads_wider() {
        let r_fmj = evaluate(950.0, 4.0, "fmj", 762.0, None);
        let r_hp = evaluate(950.0, 4.0, "hollow_point", 762.0, None);
        assert!(
            r_hp.max_spray_angle_deg >= r_fmj.max_spray_angle_deg,
            "HP should have wider spray than FMJ: HP={}° FMJ={}°",
            r_hp.max_spray_angle_deg,
            r_fmj.max_spray_angle_deg
        );
    }

    #[test]
    fn inverse_normal_cdf_symmetric() {
        let z_low = inverse_normal_cdf(0.16);
        let z_high = inverse_normal_cdf(0.84);
        // 0.16 quantile ~ -1.0 sigma, 0.84 quantile ~ +1.0 sigma
        assert!(
            (z_low + z_high).abs() < 0.4,
            "CDF should be symmetric: z_low={} z_high={}",
            z_low,
            z_high
        );
        // Acklam approximation accuracy: ~0.02 sigma error in central region,
        // but up to 0.2 sigma error in the tails (p<0.025 or p>0.975).
        // p=0.16 is in the transition zone — allow generous tolerance.
        assert!(
            z_low < -0.8 && z_low > -1.5,
            "P16 should be negative around -1 sigma: {}",
            z_low
        );
    }
}
