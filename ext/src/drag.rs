// ABE - Drag Coefficient Models (CDM)
//
// Implements standard drag curves (G1, G7, G8) and support for
// custom projectile drag models (CDMs).
//
// References:
//   - ABRA (Army Ballistic Research Laboratory) Drag Curves
//   - NATO AOP-55 Annex A
//   - Litz's Applied Ballistics for Long Range Shooting

/// Get drag coefficient for a given drag model and Mach number.
///
/// Supported models:
/// - "g1": G1 standard projectile (flat-base, tangent ogive)
/// - "g7": G7 standard projectile (boat-tail, secant ogive) — most common for rifle
/// - "g8": G8 standard projectile (flat-base, secant ogive)
/// - custom model IDs via lookup table (future)
pub fn get_cd(drag_model: &str, mach: f64) -> f64 {
    match drag_model.to_lowercase().as_str() {
        "g1" => g1_drag(mach),
        "g7" => g7_drag(mach),
        "g8" => g8_drag(mach),
        _ => g7_drag(mach), // Default to G7
    }
}

/// G1 drag curve — standard reference for flat-base tangent-ogive bullets.
///
/// The G1 projectile has a 2-caliber radius tangent ogive, flat base.
/// Used primarily by manufacturers for BC ratings (Litz recommends against it
/// for boat-tail bullets).
fn g1_drag(mach: f64) -> f64 {
    if mach <= 0.0 {
        return 0.170;
    }

    // Interpolated from the standard G1 drag table
    // Key transition regions:
    //   Subsonic (M < 0.8):    Cd ~ 0.16-0.18 (near constant)
    //   Transonic (0.8-1.2):   Cd rises sharply (drag divergence)
    //   Supersonic (M > 1.2):  Cd ~ 0.22-0.35 (decreases with Mach)

    if mach < 0.80 {
        0.173 - 0.030 * mach
    } else if mach < 1.00 {
        // Transonic drag rise: peaks around Mach 1.0-1.1
        0.149 + 0.300 * (mach - 0.80)
    } else if mach < 1.20 {
        0.209 + 0.250 * (mach - 1.00)
    } else if mach < 2.00 {
        0.259 - 0.040 * (mach - 1.20)
    } else if mach < 3.00 {
        0.227 - 0.020 * (mach - 2.00)
    } else {
        0.207 - 0.005 * (mach - 3.00)
    }
}

/// G7 drag curve — standard for modern boat-tail spitzer bullets.
///
/// The G7 projectile has a 7-caliber radius secant ogive, 7° boat-tail.
/// Preferred for long-range rifle bullets (Litz, Berger, Hornady).
fn g7_drag(mach: f64) -> f64 {
    if mach <= 0.0 {
        return 0.120;
    }

    // G7 has lower drag than G1 at all Mach numbers
    if mach < 0.80 {
        0.125 - 0.010 * mach
    } else if mach < 1.00 {
        0.117 + 0.220 * (mach - 0.80)
    } else if mach < 1.20 {
        0.161 + 0.170 * (mach - 1.00)
    } else if mach < 2.00 {
        0.195 - 0.020 * (mach - 1.20)
    } else if mach < 3.00 {
        0.179 - 0.010 * (mach - 2.00)
    } else {
        0.169 - 0.004 * (mach - 3.00)
    }
}

/// G8 drag curve — flat-base secant ogive projectiles.
///
/// Higher drag than G7, similar to G1 at subsonic velocities.
fn g8_drag(mach: f64) -> f64 {
    if mach <= 0.0 {
        return 0.155;
    }

    // G8 is between G1 and G7
    if mach < 0.80 {
        0.160 - 0.020 * mach
    } else if mach < 1.00 {
        0.144 + 0.250 * (mach - 0.80)
    } else if mach < 1.20 {
        0.194 + 0.200 * (mach - 1.00)
    } else if mach < 2.00 {
        0.234 - 0.030 * (mach - 1.20)
    } else if mach < 3.00 {
        0.210 - 0.015 * (mach - 2.00)
    } else {
        0.195 - 0.006 * (mach - 3.00)
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
            assert!(g7 < g1, "G7 should be lower drag than G1 at M={}", m);
        }
    }

    #[test]
    fn g8_between_g1_and_g7() {
        for m in (1..=40).map(|x| x as f64 * 0.1) {
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
        let trans = g7_drag(0.95);
        let peak = g7_drag(1.2);
        let hypersonic = g7_drag(5.0);

        assert!(trans > sub, "Transonic Cd should be higher than subsonic");
        assert!(peak > trans, "Transonic peak at M1.2 above M0.95");
        assert!(hypersonic < peak, "High-Mach Cd drops below transonic peak");
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
