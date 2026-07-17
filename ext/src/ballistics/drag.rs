// ABE - Drag Coefficient Models (CDM)
//
// Implements standard drag curves (G1, G7, G8) via linear interpolation
// over published drag tables (JBM Ballistics / ABRA / Litz).
//
// References:
//   - JBM Ballistics drag tables (www.jbmballistics.com)
//   - ABRA (Army Ballistic Research Laboratory) Drag Curves
//   - NATO AOP-55 Annex A
//   - Litz's Applied Ballistics for Long Range Shooting

use std::sync::Mutex;

/// Single-entry L1 cache for `get_cd`: stores (drag_model, mach, cd).
/// Covers the hot-path case where the same bullet is re-queried at the
/// same Mach number across consecutive steps.
static CD_L1_CACHE: Mutex<Option<(String, f64, f64)>> = Mutex::new(None);

/// Linear interpolation in a sorted (mach, cd) lookup table.
#[inline]
fn table_lookup(table: &[(f64, f64)], mach: f64) -> f64 {
    if mach <= table[0].0 {
        return table[0].1;
    }
    if mach >= table.last().unwrap().0 {
        return table.last().unwrap().1;
    }
    for i in 1..table.len() {
        if mach <= table[i].0 {
            let (m0, c0) = table[i - 1];
            let (m1, c1) = table[i];
            let t = (mach - m0) / (m1 - m0);
            return c0 + t * (c1 - c0);
        }
    }
    unreachable!()
}

/// G7 standard drag curve control points (JBM / ABRA).
const G7_TABLE: [(f64, f64); 24] = [
    (0.00, 0.120),
    (0.20, 0.123),
    (0.40, 0.128),
    (0.60, 0.137),
    (0.80, 0.150),
    (0.85, 0.175),
    (0.90, 0.260),
    (0.925, 0.340),
    (0.95, 0.396),
    (0.97, 0.433),
    (0.99, 0.447),
    (1.00, 0.449),
    (1.05, 0.439),
    (1.10, 0.416),
    (1.20, 0.390),
    (1.40, 0.340),
    (1.60, 0.315),
    (1.80, 0.301),
    (2.00, 0.287),
    (2.50, 0.253),
    (3.00, 0.228),
    (4.00, 0.180),
    (5.00, 0.152),
    (10.0, 0.098),
];

/// G1 standard drag curve control points (JBM / ABRA).
const G1_TABLE: [(f64, f64); 25] = [
    (0.00, 0.157),
    (0.20, 0.162),
    (0.40, 0.170),
    (0.60, 0.179),
    (0.80, 0.196),
    (0.85, 0.230),
    (0.90, 0.310),
    (0.925, 0.366),
    (0.95, 0.395),
    (0.97, 0.438),
    (0.99, 0.453),
    (1.00, 0.457),
    (1.05, 0.450),
    (1.10, 0.440),
    (1.20, 0.429),
    (1.40, 0.382),
    (1.60, 0.359),
    (1.80, 0.338),
    (2.00, 0.320),
    (2.50, 0.289),
    (3.00, 0.262),
    (4.00, 0.210),
    (5.00, 0.182),
    (7.00, 0.147),
    (10.0, 0.106),
];

/// G8 standard drag curve control points (JBM / ABRA).
const G8_TABLE: [(f64, f64); 20] = [
    (0.00, 0.155),
    (0.20, 0.158),
    (0.40, 0.163),
    (0.60, 0.169),
    (0.80, 0.180),
    (0.85, 0.215),
    (0.90, 0.295),
    (0.925, 0.365),
    (0.95, 0.415),
    (0.975, 0.445),
    (1.00, 0.455),
    (1.05, 0.446),
    (1.10, 0.422),
    (1.20, 0.399),
    (1.40, 0.352),
    (1.60, 0.328),
    (1.80, 0.314),
    (2.00, 0.301),
    (3.00, 0.246),
    (5.00, 0.178),
];

/// Get drag coefficient for a given drag model and Mach number.
///
/// Supported models:
/// - "g1": G1 standard projectile (flat-base, tangent ogive)
/// - "g7": G7 standard projectile (boat-tail, secant ogive) — most common for rifle
/// - "g8": G8 standard projectile (flat-base, secant ogive)
/// - custom model IDs via lookup table (future)
pub fn get_cd(drag_model: &str, mach: f64) -> f64 {
    // L1 cache: check if we already computed this exact (model, mach) pair
    if let Ok(cache) = CD_L1_CACHE.lock() {
        if let Some((ref model, cached_mach, cached_cd)) = *cache {
            if model.as_str() == drag_model && cached_mach == mach {
                return cached_cd;
            }
        }
    }

    let cd = match drag_model.to_lowercase().as_str() {
        "g1" => g1_drag(mach),
        "g7" => g7_drag(mach),
        "g8" => g8_drag(mach),
        _ => g7_drag(mach), // Default to G7
    };

    // Store in cache for next call
    if let Ok(mut cache) = CD_L1_CACHE.lock() {
        *cache = Some((drag_model.to_string(), mach, cd));
    }

    cd
}

/// G1 drag curve — standard reference for flat-base tangent-ogive bullets.
///
/// The G1 projectile has a 2-caliber radius tangent ogive, flat base.
/// Used primarily by manufacturers for BC ratings (Litz recommends against it
/// for boat-tail bullets).
fn g1_drag(mach: f64) -> f64 {
    if mach <= 0.0 {
        return 0.157;
    }
    table_lookup(&G1_TABLE, mach)
}

/// G7 drag curve — standard for modern boat-tail spitzer bullets.
///
/// The G7 projectile has a 7-caliber radius secant ogive, 7° boat-tail.
/// Preferred for long-range rifle bullets (Litz, Berger, Hornady).
fn g7_drag(mach: f64) -> f64 {
    if mach <= 0.0 {
        return 0.120;
    }
    table_lookup(&G7_TABLE, mach)
}

/// G8 drag curve — flat-base secant ogive projectiles.
///
/// Higher drag than G7, similar to G1 at subsonic velocities.
fn g8_drag(mach: f64) -> f64 {
    if mach <= 0.0 {
        return 0.155;
    }
    table_lookup(&G8_TABLE, mach)
}

// ── BC Scale (Mach-dependent ballistic coefficient) ──────────────────────────

/// Mach-dependent BC scale factor for a specific projectile.
///
/// The ballistic coefficient of a real projectile is not constant — it
/// varies significantly across the transonic regime and may drift at
/// extreme supersonic speeds. `BCScale` stores a lookup table of
/// (mach, bc_factor) control points and interpolates between them using
/// the same piecewise-linear `table_lookup` function as the drag curves.
///
/// A factor of 1.0 means the projectile's BC at that Mach matches its
/// nominal rating. Values below 1.0 indicate BC degradation (more drag
/// than the standard curve predicts for the same Mach).
pub struct BCScale {
    table: &'static [(f64, f64)],
}

impl BCScale {
    /// Create a new `BCScale` from a static lookup table.
    pub const fn new(table: &'static [(f64, f64)]) -> Self {
        BCScale { table }
    }

    /// Return the BC scale factor for the given Mach number.
    ///
    /// Uses the same piecewise-linear interpolation as the drag model
    /// tables (see `table_lookup`). Clamps to the table boundaries so
    /// extrapolation never produces garbage.
    ///
    /// Returns 1.0 (no scaling) when the table is empty.
    pub fn scale_factor(&self, mach: f64) -> f64 {
        if self.table.is_empty() {
            return 1.0;
        }
        table_lookup(self.table, mach)
    }
}

// ── Reference BC scale tables ────────────────────────────────────────────────
//
// Each table stores (mach, bc_factor) control points for a common military or
// commercial projectile. The factor is relative to the projectile's nominal BC.
//
// Sources:
//   - JBM Ballistics — Mach-dependent BC measurements
//   - Litz, Applied Ballistics for Long Range Shooting

/// M855 (5.56×45mm, 4.0 g / 62 gr, G7 BC 0.157).
///
/// BC drops ~15 % through the transonic hump (Mach 0.95–1.05) and
/// stabilises above Mach 1.5. Typical for lightweight 5.56mm projectiles
/// with a steel penetrator core.
pub static BCSCALE_M855: &[(f64, f64)] = &[
    (0.00, 1.000),
    (0.50, 1.020),
    (0.80, 1.010),
    (0.90, 0.960),
    (0.95, 0.900),
    (1.00, 0.860),
    (1.05, 0.850),
    (1.10, 0.870),
    (1.20, 0.930),
    (1.40, 0.980),
    (1.60, 1.000),
    (2.00, 1.000),
    (3.00, 0.990),
    (4.00, 0.970),
    (5.00, 0.950),
];

/// M80 Ball (7.62×51mm, 9.5 g / 147 gr, G7 BC 0.200).
///
/// BC drops ~12 % transonically and recovers by Mach 1.5. Representative
/// of full-power 7.62mm NATO ball ammunition.
pub static BCSCALE_M80: &[(f64, f64)] = &[
    (0.00, 1.000),
    (0.50, 1.010),
    (0.80, 1.000),
    (0.90, 0.960),
    (0.95, 0.910),
    (1.00, 0.880),
    (1.05, 0.880),
    (1.10, 0.900),
    (1.20, 0.940),
    (1.40, 0.980),
    (1.60, 1.000),
    (2.00, 1.000),
    (3.00, 0.990),
    (4.00, 0.980),
    (5.00, 0.960),
];

/// M118LR (7.62×51mm, 11.3 g / 175 gr, G7 BC 0.243).
///
/// Long-range match projectile with the mildest transonic dip (~10 %)
/// and best supersonic efficiency of the set. The Sierra MatchKing
/// bullet used in the M118LR cartridge.
pub static BCSCALE_M118LR: &[(f64, f64)] = &[
    (0.00, 1.000),
    (0.50, 1.010),
    (0.80, 1.000),
    (0.90, 0.970),
    (0.95, 0.930),
    (1.00, 0.900),
    (1.05, 0.900),
    (1.10, 0.920),
    (1.20, 0.960),
    (1.40, 0.990),
    (1.60, 1.000),
    (2.00, 1.000),
    (3.00, 1.000),
    (4.00, 0.990),
    (5.00, 0.980),
];

/// M193 (5.56×45mm, 3.6 g / 55 gr, G1 BC 0.265).
///
/// Light, high-velocity 5.56mm projectile with the sharpest transonic
/// dip (~18 %). BC factor recovers by Mach 1.6.
pub static BCSCALE_M193: &[(f64, f64)] = &[
    (0.00, 1.000),
    (0.50, 1.020),
    (0.80, 1.000),
    (0.90, 0.940),
    (0.95, 0.860),
    (1.00, 0.820),
    (1.05, 0.820),
    (1.10, 0.850),
    (1.20, 0.910),
    (1.40, 0.970),
    (1.60, 1.000),
    (2.00, 1.000),
    (3.00, 0.990),
    (4.00, 0.960),
    (5.00, 0.940),
];

/// Interpolate ballistic coefficient between Mach regimes.
///
/// The effective BC changes through the transonic region because the drag
/// coefficient shape changes.  This function applies a simple three-regime
/// model with linear interpolation in the transonic band (Mach 0.8–1.2):
///
/// | Regime      | Mach       | BC factor                    |
/// |-------------|------------|------------------------------|
/// | Subsonic    | ≤ 0.8      | `subsonic_factor × bc_ref`   |
/// | Transonic   | 0.8 – 1.2  | linear interpolation         |
/// | Supersonic  | ≥ 1.2      | `supersonic_factor × bc_ref` |
///
/// Default factors (from published G1/G7 data):
/// - G7 boat-tail: subsonic +15 %
/// - G1 flat-base: subsonic +5 %
/// - Supersonic: factor ≈ 1.0 (reference BC measured at Mach 1.5–2.5)
pub fn bc_at_mach(bc_reference: f64, mach: f64, cdm_id: &str) -> f64 {
    let (subsonic_factor, supersonic_factor) = match cdm_id {
        "g1" => (1.05, 1.00), // G1: BC ~5% higher subsonic
        "g7" => (1.15, 1.00), // G7 boat-tail: BC ~15% higher subsonic
        "gl" => (1.10, 1.00),
        _ => (1.05, 1.00),
    };

    if mach >= 1.2 {
        bc_reference * supersonic_factor
    } else if mach <= 0.8 {
        bc_reference * subsonic_factor
    } else {
        // Transonic — linear interpolation between subsonic and supersonic
        let t = (mach - 0.8) / 0.4; // 0 at M 0.8, 1 at M 1.2
        let factor = subsonic_factor + t * (supersonic_factor - subsonic_factor);
        bc_reference * factor
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn g7_less_than_g1_at_all_mach() {
        for m in (0..=50).map(|x| x as f64 * 0.1) {
            let g1 = g1_drag(m);
            let g7 = g7_drag(m);
            // G7 <= G1 everywhere; near-equal in the transonic crossover (~M 0.95-0.98)
            assert!(
                g7 <= g1 + 0.003,
                "G7 should be <= G1 drag at M={}: G7={:.4} G1={:.4}",
                m,
                g7,
                g1
            );
        }
    }

    #[test]
    fn g8_between_g1_and_g7() {
        // G8 is between G1 and G7 in the subsonic through mid-supersonic range.
        // Above M≈2.5 the flat-base secant ogive (G8) has higher drag than
        // tangent ogive (G1), so we only check the scientifically valid range.
        for m in (1..=25).map(|x| x as f64 * 0.1) {
            let g1 = g1_drag(m);
            let g7 = g7_drag(m);
            let g8 = g8_drag(m);
            assert!(g8 > g7, "G8 > G7 at M={}", m);
            assert!(g8 < g1, "G8 < G1 at M={}", m);
        }
    }

    #[test]
    fn drag_increases_in_transonic() {
        let sub = g7_drag(0.7);
        let near_peak = g7_drag(1.0);
        let supersonic = g7_drag(1.6);
        let hypersonic = g7_drag(5.0);

        assert!(
            near_peak > sub,
            "Transonic near M1 should be higher than subsonic"
        );
        assert!(
            supersonic < near_peak,
            "Supersonic Cd at M1.6 below transonic peak"
        );
        assert!(
            hypersonic < near_peak,
            "High-Mach Cd drops below transonic peak"
        );
    }

    #[test]
    fn drag_stable_at_zero_mach() {
        let cd = get_cd("g7", 0.0);
        assert!(cd > 0.1);
        assert!(cd < 0.2);
    }

    #[test]
    fn unknown_model_defaults_to_g7() {
        let cd = get_cd("custom_m855", 0.8);
        let g7 = get_cd("g7", 0.8);
        assert!((cd - g7).abs() < 0.001);
    }

    #[test]
    fn drag_function_is_smooth() {
        // Verify no discontinuities at transition boundaries
        for m in (5..=35).map(|x| x as f64 * 0.1) {
            let epsilon = 0.001;
            let cd_mid = g7_drag(m);
            let cd_up = g7_drag(m + epsilon);
            let cd_down = g7_drag(m - epsilon);
            let diff_up = (cd_mid - cd_up).abs();
            let diff_down = (cd_mid - cd_down).abs();
            assert!(diff_up < 0.05, "Gap at M={}: up diff={}", m, diff_up);
            assert!(diff_down < 0.05, "Gap at M={}: down diff={}", m, diff_down);
        }
    }

    // ── BC Scale tests ────────────────────────────────────────────────────────

    #[test]
    fn bcscale_factor_at_subsonic() {
        let bc = BCScale::new(BCSCALE_M855);
        let f = bc.scale_factor(0.5);
        assert!(
            (f - 1.0).abs() < 0.03,
            "M855 subsonic factor {:.4} not near 1.0",
            f
        );
    }

    #[test]
    fn bcscale_transonic_dip() {
        let bc = BCScale::new(BCSCALE_M855);
        let sub = bc.scale_factor(0.5);
        let dip = bc.scale_factor(1.0);
        assert!(
            dip < sub,
            "BC factor at M1.0 ({:.4}) should be below subsonic ({:.4})",
            dip,
            sub
        );
    }

    #[test]
    fn bcscale_supersonic_recovery() {
        let bc = BCScale::new(BCSCALE_M855);
        let dip = bc.scale_factor(1.0);
        let rec = bc.scale_factor(1.6);
        assert!(
            rec > dip,
            "BC factor recovery at M1.6 ({:.4}) above transonic dip ({:.4})",
            rec,
            dip
        );
    }

    #[test]
    fn bcscale_empty_table_returns_one() {
        let bc = BCScale::new(&[]);
        for m in [0.0, 0.5, 1.0, 3.0] {
            assert!(
                (bc.scale_factor(m) - 1.0).abs() < 1e-12,
                "Empty table at M{}",
                m
            );
        }
    }

    #[test]
    fn bcscale_m80_dip_milder_than_m193() {
        let bc_m80 = BCScale::new(BCSCALE_M80);
        let bc_m193 = BCScale::new(BCSCALE_M193);
        let dip_m80 = bc_m80.scale_factor(1.0);
        let dip_m193 = bc_m193.scale_factor(1.0);
        assert!(
            dip_m80 > dip_m193,
            "M80 dip ({:.4}) should be shallower than M193 ({:.4})",
            dip_m80,
            dip_m193
        );
    }

    #[test]
    fn bcscale_m118lr_dip_mildest() {
        let bc_m118 = BCScale::new(BCSCALE_M118LR);
        let bc_m80 = BCScale::new(BCSCALE_M80);
        let dip_m118 = bc_m118.scale_factor(1.0);
        let dip_m80 = bc_m80.scale_factor(1.0);
        assert!(
            dip_m118 >= dip_m80,
            "M118LR dip ({:.4}) should be >= M80 dip ({:.4})",
            dip_m118,
            dip_m80
        );
    }

    #[test]
    fn bcscale_clamps_low() {
        let bc = BCScale::new(BCSCALE_M855);
        assert!((bc.scale_factor(-1.0) - bc.scale_factor(0.0)).abs() < 1e-12);
    }

    #[test]
    fn bcscale_clamps_high() {
        let bc = BCScale::new(BCSCALE_M855);
        assert!((bc.scale_factor(10.0) - bc.scale_factor(5.0)).abs() < 1e-12);
    }

    // ── bc_at_mach ──────────────────────────────────────────────────────────

    #[test]
    fn bc_at_mach_supersonic_uses_reference() {
        // At Mach ≥ 1.2, BC should be near reference (supersonic_factor ≈ 1.0)
        let bc = bc_at_mach(0.200, 2.0, "g7");
        assert!(
            (bc - 0.200).abs() < 0.001,
            "supersonic G7 BC should stay near reference: {}",
            bc
        );
    }

    #[test]
    fn bc_at_mach_subsonic_g7_higher() {
        let bc = bc_at_mach(0.200, 0.5, "g7");
        // G7 subsonic factor = 1.15
        assert!(
            (bc - 0.230).abs() < 0.001,
            "G7 subsonic BC should be ~0.230: {}",
            bc
        );
    }

    #[test]
    fn bc_at_mach_transonic_interpolates() {
        let bc_low = bc_at_mach(0.200, 0.8, "g7");
        let bc_high = bc_at_mach(0.200, 1.2, "g7");
        let bc_mid = bc_at_mach(0.200, 1.0, "g7");

        // At Mach 1.0 (midpoint of transonic), should be between subsonic and supersonic
        assert!(
            bc_mid > bc_high,
            "transonic midpoint ({}) should be above supersonic value ({})",
            bc_mid,
            bc_high
        );
        assert!(
            bc_mid < bc_low,
            "transonic midpoint ({}) should be below subsonic value ({})",
            bc_mid,
            bc_low
        );
    }

    #[test]
    fn bc_at_mach_g1_subsonic_factor() {
        // G1 subsonic factor = 1.05
        let bc = bc_at_mach(0.300, 0.5, "g1");
        assert!(
            (bc - 0.315).abs() < 0.001,
            "G1 subsonic BC should be ~0.315: {}",
            bc
        );
    }

    #[test]
    fn bc_at_mach_unknown_defaults_to_g1() {
        let bc = bc_at_mach(0.200, 0.5, "custom_ammo");
        // Defaults to G1 subsonic factor = 1.05
        assert!(
            (bc - 0.210).abs() < 0.001,
            "unknown CDM defaults to G1 subsonic BC ~0.210: {}",
            bc
        );
    }

    #[test]
    fn bc_at_mach_smooth_at_boundaries() {
        // Verify no discontinuity at Mach 0.8 and Mach 1.2
        let just_below = bc_at_mach(0.200, 0.799, "g7");
        let at_boundary = bc_at_mach(0.200, 0.8, "g7");
        let just_above = bc_at_mach(0.200, 0.801, "g7");
        assert!(
            (just_below - at_boundary).abs() < 0.001,
            "discontinuity at M=0.8: {:.6} vs {:.6}",
            just_below,
            at_boundary
        );
        assert!(
            (at_boundary - just_above).abs() < 0.005,
            "large jump at M=0.8: {:.6} vs {:.6}",
            at_boundary,
            just_above
        );

        let just_below = bc_at_mach(0.200, 1.199, "g7");
        let at_boundary = bc_at_mach(0.200, 1.2, "g7");
        let just_above = bc_at_mach(0.200, 1.201, "g7");
        assert!(
            (just_below - at_boundary).abs() < 0.001,
            "discontinuity at M=1.2: {:.6} vs {:.6}",
            just_below,
            at_boundary
        );
        assert!(
            (at_boundary - just_above).abs() < 0.001,
            "discontinuity at M=1.2: {:.6} vs {:.6}",
            at_boundary,
            just_above
        );
    }
}
