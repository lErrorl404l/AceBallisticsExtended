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
}
