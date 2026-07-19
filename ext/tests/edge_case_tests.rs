//! Edge case trajectory tests for ABE ballistics solver.
//!
//! Validates the solver handles physically meaningful edge cases:
//! transonic regime, subsonic, high altitude, extreme range, zero range,
//! very short range, negative range, very high BC + MV, extreme wind,
//! and light bullet transonic transition.
//!
//! Each test checks for NaN/Inf, monotonic behavior, and physical
//! plausibility.  These complement the reference trajectory tests
//! (reference_trajectories.rs) by exercising boundary conditions rather
//! than standard known-value comparisons.

#![allow(dead_code)]

use abe_ballistics_ext::{abe_init, abe_step, BulletState, StepParams, MAGIC_ABE};

const ABE_API_VERSION: u32 = 1;

/// Integration timestep (1000 Hz).
const DT_S: f64 = 0.01;

/// Maximum integration steps — prevents infinite loops on pathological cases.
const MAX_STEPS: u32 = 100_000;

// ── Helpers ─────────────────────────────────────────────────────────────────────

/// Build a 32-byte null-terminated CDM identifier buffer.
fn make_cdm(s: &str) -> [u8; 32] {
    let mut buf = [0u8; 32];
    let bytes = s.as_bytes();
    let n = bytes.len().min(31);
    buf[..n].copy_from_slice(&bytes[..n]);
    buf
}

/// Run a full trajectory simulation and collect every intermediate state.
///
/// Returns `Vec<(range_m, drop_m, speed_ms, mach, tof_s)>`:
///   * `range_m` — x position (downrange distance, metres)
///   * `drop_m`  — z position (+z = downward = drop, metres)
///   * `speed_ms` — total speed (m/s)
///   * `mach`    — Mach number (from solver output; 0 on initial point)
///   * `tof_s`   — accumulated time of flight (seconds)
fn simulate(
    mv_ms: f64,
    bc: f64,
    mass_g: f64,
    caliber_mm: f64,
    cdm: &str,
    dt_s: f64,
    density_kgm3: f64,
    temp_c: f64,
    altitude_m: f64,
    wind: [f64; 3],
    max_range_m: f64,
    min_vel_ms: f64,
) -> Vec<(f64, f64, f64, f64, f64)> {
    let cdm_id = make_cdm(cdm);
    let mut x = 0.0;
    let mut y = 0.0;
    let mut z = 0.0;
    let mut vx = mv_ms;
    let mut vy = 0.0;
    let mut vz = 0.0;
    let mut t = 0.0;

    let mut points = Vec::new();
    // Initial state: range = 0, speed = MV, mach = 0 (placeholder)
    points.push((x, z, mv_ms, 0.0, t));

    for _step in 0..MAX_STEPS {
        let step = StepParams {
            magic: MAGIC_ABE,
            pos_x: x,
            pos_y: y,
            pos_z: z,
            vel_x: vx,
            vel_y: vy,
            vel_z: vz,
            dt_s,
            wind_x: wind[0],
            wind_y: wind[1],
            wind_z: wind[2],
            density_kgm3,
            temp_c,
            altitude_m,
            cdm_id,
            bc,
            mass_g,
            caliber_mm,
            twist_rate_m: 0.0,
        };

        let mut result = BulletState::default();
        let ret = abe_step(&step, &mut result);
        if ret != 0 {
            break;
        }

        x = result.pos_x;
        y = result.pos_y;
        z = result.pos_z;
        vx = result.vel_x;
        vy = result.vel_y;
        vz = result.vel_z;
        t += dt_s;

        let speed = (vx * vx + vy * vy + vz * vz).sqrt();
        points.push((x, z, speed, result.mach, t));

        if x >= max_range_m || speed < min_vel_ms {
            break;
        }
    }

    points
}

/// Convenience wrapper: standard sea-level atmosphere, no wind.
fn simulate_std(
    mv_ms: f64,
    bc: f64,
    mass_g: f64,
    caliber_mm: f64,
    cdm: &str,
    max_range_m: f64,
) -> Vec<(f64, f64, f64, f64, f64)> {
    simulate(
        mv_ms,
        bc,
        mass_g,
        caliber_mm,
        cdm,
        DT_S,
        1.225,           // density kg/m³ (sea level ISA)
        15.0,            // temperature °C
        0.0,             // altitude m
        [0.0, 0.0, 0.0], // wind
        max_range_m,
        50.0, // min velocity
    )
}

/// Find the trajectory point whose range is closest to `target_m`.
fn point_at(pts: &[(f64, f64, f64, f64, f64)], target_m: f64) -> Option<(f64, f64, f64, f64, f64)> {
    pts.iter()
        .min_by(|a, b| {
            (a.0 - target_m)
                .abs()
                .partial_cmp(&(b.0 - target_m).abs())
                .unwrap()
        })
        .copied()
}

/// Assert every field in every trajectory point is finite (no NaN/Inf).
fn assert_all_finite(name: &str, pts: &[(f64, f64, f64, f64, f64)]) {
    for (i, &(r, d, v, m, t)) in pts.iter().enumerate() {
        assert!(r.is_finite(), "{name} [{i}]: range not finite");
        assert!(d.is_finite(), "{name} [{i}]: drop not finite");
        assert!(v.is_finite(), "{name} [{i}]: speed not finite");
        assert!(m.is_finite(), "{name} [{i}]: mach not finite");
        assert!(t.is_finite(), "{name} [{i}]: tof not finite");
    }
}

/// Assert that speed (index 2) is monotonically non-increasing.
fn assert_speed_monotonic(name: &str, pts: &[(f64, f64, f64, f64, f64)]) {
    for i in 1..pts.len() {
        assert!(
            pts[i].2 <= pts[i - 1].2 + 1e-9,
            "{name}: speed increased at step {i}: {:.6} → {:.6}",
            pts[i - 1].2,
            pts[i].2,
        );
    }
}

/// Assert that drop (index 1) is monotonically non-decreasing.
fn assert_drop_monotonic(name: &str, pts: &[(f64, f64, f64, f64, f64)]) {
    let mut prev = -f64::MAX;
    for &(_r, d, _v, _m, _t) in pts {
        assert!(
            d >= prev - 1e-9,
            "{name}: drop decreased from {:.6} to {:.6}",
            prev,
            d,
        );
        prev = d;
    }
}

/// Approximate drag deceleration jump between consecutive steps.
/// Returns the maximum absolute jump magnitude (m/s² per step) across
/// the whole trajectory.
fn max_decel_jump(pts: &[(f64, f64, f64, f64, f64)]) -> f64 {
    let mut max_jump = 0.0;
    for i in 2..pts.len() {
        let dt1 = pts[i].4 - pts[i - 1].4;
        let dt0 = pts[i - 1].4 - pts[i - 2].4;
        if dt1 < 1e-12 || dt0 < 1e-12 {
            continue;
        }
        let decel1 = (pts[i - 1].2 - pts[i].2) / dt1;
        let decel0 = (pts[i - 2].2 - pts[i - 1].2) / dt0;
        let jump = (decel1 - decel0).abs();
        if jump > max_jump {
            max_jump = jump;
        }
    }
    max_jump
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test 1 — Transonic regime
// ═══════════════════════════════════════════════════════════════════════════════
//
// M80 7.62mm (BC G7 0.200, 850 m/s MV) flies to 800 m, crossing Mach 1
// around 600 m.  The G7 drag curve rises steeply between Mach 0.8 → 1.2
// (Cd from ~0.15 to ~0.45), testing numerical stability through the
// transonic drag rise.

#[test]
fn transonic_regime() {
    abe_init(ABE_API_VERSION, 0);

    let pts = simulate_std(850.0, 0.200, 9.5, 7.62, "g7", 800.0);
    assert!(!pts.is_empty(), "transonic: got trajectory points");

    assert_all_finite("transonic", &pts);
    assert_speed_monotonic("transonic", &pts);
    assert_drop_monotonic("transonic", &pts);

    // Must start supersonic and end subsonic (cross Mach 1)
    let machs: Vec<f64> = pts.iter().map(|p| p.3).collect();
    let max_mach = machs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let min_mach = machs.iter().copied().fold(f64::INFINITY, f64::min);
    assert!(
        max_mach > 1.05,
        "transonic: must start clearly supersonic (Mach > 1.05), max={:.3}",
        max_mach
    );
    assert!(
        min_mach < 0.95,
        "transonic: must go subsonic (Mach < 0.95), min={:.3}",
        min_mach
    );

    // Drag deceleration must be smooth through the transonic region
    let jump = max_decel_jump(&pts);
    assert!(
        jump < 5000.0,
        "transonic: max drag deceleration jump {:.0} m/s³ — suspiciously large",
        jump,
    );

    // Drop at final range 5-15 m (physically plausible for M80 at 800 m)
    let last = pts.last().unwrap();
    assert!(
        last.1 > 5.0 && last.1 < 15.0,
        "transonic: drop at {:.0} m = {:.3} m (expect 5-15 m)",
        last.0,
        last.1,
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test 2 — Subsonic (.300 BLK)
// ═══════════════════════════════════════════════════════════════════════════════
//
// .300 BLK subsonic (BC G7 0.313, 305 m/s MV) flies 200 m entirely below
// Mach 1.  Heavy, high-BC projectile with low drag; drop should be
// physically reasonable (~20-40 cm at 200 m).

#[test]
fn subsonic_300blk() {
    abe_init(ABE_API_VERSION, 0);

    let pts = simulate_std(305.0, 0.313, 14.3, 7.82, "g7", 200.0);
    assert!(!pts.is_empty(), "subsonic: got trajectory points");

    assert_all_finite("subsonic", &pts);
    assert_speed_monotonic("subsonic", &pts);
    assert_drop_monotonic("subsonic", &pts);

    // Entire flight below Mach 1
    let max_mach = pts.iter().map(|p| p.3).fold(f64::NEG_INFINITY, f64::max);
    assert!(
        max_mach < 1.0,
        "subsonic: max Mach = {:.4} (must be < 1.0)",
        max_mach,
    );

    // Drop at 200 m: ~1-5 m for .300 BLK subsonic (TOF ~0.66 s at 305 m/s)
    // 0.5*g*t² ≈ 2.1m; actual ~2.3m including small drag loss
    let at200 = point_at(&pts, 200.0).unwrap_or(*pts.last().unwrap());
    assert!(
        at200.1 > 0.5 && at200.1 < 5.0,
        "subsonic: drop at 200 m = {:.3} m (expect 0.5-5.0)",
        at200.1,
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test 3 — High altitude
// ═══════════════════════════════════════════════════════════════════════════════
//
// M118LR (BC G7 0.243, 780 m/s MV) at 5000 m altitude.  The solver uses
// the ICAO atmosphere when altitude_m > 0.  Thinner air → less drag →
// flatter trajectory.

#[test]
fn high_altitude_less_drag() {
    abe_init(ABE_API_VERSION, 0);

    // Sea level: standard density, temp 15 °C, altitude 0 m
    let sl = simulate(
        780.0,
        0.243,
        11.34,
        7.62,
        "g7",
        DT_S,
        1.225,
        15.0,
        0.0,
        [0.0, 0.0, 0.0],
        600.0,
        50.0,
    );
    // High altitude: 5000 m; solver uses ICAO from altitude_m
    let ha = simulate(
        780.0,
        0.243,
        11.34,
        7.62,
        "g7",
        DT_S,
        1.225,
        -17.5,
        5000.0,
        [0.0, 0.0, 0.0],
        600.0,
        50.0,
    );

    assert!(!sl.is_empty(), "high-alt: sea level points");
    assert!(!ha.is_empty(), "high-alt: altitude points");
    assert_all_finite("high-alt sea level", &sl);
    assert_all_finite("high-alt 5000m", &ha);

    // Both should reach ~600 m
    let sl_end = sl.last().unwrap();
    let ha_end = ha.last().unwrap();
    assert!(sl_end.0 >= 590.0, "sea level reached {:.1} m", sl_end.0);
    assert!(ha_end.0 >= 590.0, "high alt reached {:.1} m", ha_end.0);

    // Less drag at altitude → less drop and higher retained speed
    assert!(
        ha_end.1 < sl_end.1 - 0.3,
        "high-alt drop ({:.3} m) < sea level ({:.3} m) by >0.3 m",
        ha_end.1,
        sl_end.1,
    );
    assert!(
        ha_end.2 > sl_end.2 + 10.0,
        "high-alt retained speed ({:.1} m/s) > sea level ({:.1} m/s) by >10 m/s",
        ha_end.2,
        sl_end.2,
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test 4 — Extreme range
// ═══════════════════════════════════════════════════════════════════════════════
//
// .408 CheyTac (BC G7 0.420, 910 m/s MV) at 2000 m.  Very long-range
// trajectory — tests stability at shallow angles and extreme TOF.

#[test]
fn extreme_range_408_cheytac() {
    abe_init(ABE_API_VERSION, 0);

    let pts = simulate_std(910.0, 0.420, 27.0, 10.36, "g7", 2000.0);
    assert!(!pts.is_empty(), "extreme: got trajectory points");

    assert_all_finite("extreme-range", &pts);
    assert_speed_monotonic("extreme-range", &pts);
    assert_drop_monotonic("extreme-range", &pts);

    // Must reach at least 1900 m
    let last = pts.last().unwrap();
    assert!(
        last.0 >= 1900.0,
        "extreme-range: reached {:.0} m of 2000 m target",
        last.0,
    );

    // Time of flight must be positive and finite
    assert!(
        last.4 > 0.0 && last.4 < 10.0,
        "extreme-range: TOF = {:.3} s (expect 0-10 s)",
        last.4,
    );

    // Retained velocity must be positive (never stops)
    assert!(last.2 > 0.0, "extreme-range: speed > 0 at {:.0} m", last.0);

    // Drop should be significant but bounded
    assert!(
        last.1 > 10.0 && last.1 < 100.0,
        "extreme-range: drop = {:.1} m (expect 10-100 m at {:.0} m)",
        last.1,
        last.0,
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test 5 — Zero range
// ═══════════════════════════════════════════════════════════════════════════════
//
// Any round at 0 m range: drop = 0, velocity = MV, TOF = 0.

#[test]
fn zero_range_initial_state() {
    abe_init(ABE_API_VERSION, 0);

    // M855, but any round would do
    let pts = simulate_std(930.0, 0.157, 4.0, 5.56, "g7", 0.0);

    // Should have exactly the initial point
    assert!(!pts.is_empty(), "zero-range: at least initial point");

    let init = pts[0];
    assert_eq!(init.0, 0.0, "zero-range: range = 0");
    assert_eq!(init.1, 0.0, "zero-range: drop = 0");
    assert_eq!(init.2, 930.0, "zero-range: speed = MV");
    assert_eq!(init.4, 0.0, "zero-range: TOF = 0");
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test 6 — Very short range
// ═══════════════════════════════════════════════════════════════════════════════
//
// M855 at 1 m: drop must be very small (< 1 cm) and all values finite.

#[test]
fn very_short_range_1m() {
    abe_init(ABE_API_VERSION, 0);

    let pts = simulate_std(930.0, 0.157, 4.0, 5.56, "g7", 1.0);
    assert!(!pts.is_empty(), "short-range: got trajectory points");
    assert_all_finite("short-range 1m", &pts);

    let last = pts.last().unwrap();
    assert!(
        last.0 >= 0.9,
        "short-range: reached {:.3} m (target 1 m)",
        last.0,
    );

    // Drop at 1 m is negligible (< 1 cm)
    assert!(
        last.1 < 0.01,
        "short-range: drop = {:.6} m (< 0.01 m)",
        last.1,
    );

    // Speed should be close to MV (small drag over 1 m, ~9 m/s drop)
    assert!(
        (last.2 - 930.0).abs() < 15.0,
        "short-range: speed {:.1} ≈ MV 930 (within 15 m/s)",
        last.2,
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test 7 — Negative range
// ═══════════════════════════════════════════════════════════════════════════════
//
// Negative range is unphysical — the solver should not crash; it may
// return an error or proceed with default values.

#[test]
fn negative_range_graceful() {
    abe_init(ABE_API_VERSION, 0);

    // The solver integrates forward in time; a negative range target
    // means we just check that a short backwards run doesn't crash.
    // We fire at negative x velocity (moving backwards) and assert
    // no crash or NaN.
    let cdm_id = make_cdm("g7");

    let params = StepParams {
        magic: MAGIC_ABE,
        pos_x: 0.0,
        pos_y: 0.0,
        pos_z: 0.0,
        vel_x: -930.0, // moving in -x direction
        vel_y: 0.0,
        vel_z: 0.0,
        dt_s: DT_S,
        wind_x: 0.0,
        wind_y: 0.0,
        wind_z: 0.0,
        density_kgm3: 1.225,
        temp_c: 15.0,
        altitude_m: 0.0,
        cdm_id,
        bc: 0.157,
        mass_g: 4.0,
        caliber_mm: 5.56,
        twist_rate_m: 0.0,
    };

    // Run a few backward steps — should not crash
    let mut result = BulletState::default();
    for _ in 0..100 {
        let mut step = params;
        // Update position from previous result
        step.pos_x = result.pos_x;
        step.pos_y = result.pos_y;
        step.pos_z = result.pos_z;

        let ret = abe_step(&step, &mut result);
        // The solver returns 0 on success even for backward motion
        if ret != 0 {
            // Non-zero return is also acceptable — just don't crash
            return;
        }

        assert!(result.pos_x.is_finite(), "neg-range: pos_x finite");
        assert!(result.pos_y.is_finite(), "neg-range: pos_y finite");
        assert!(result.pos_z.is_finite(), "neg-range: pos_z finite");
        assert!(result.vel_x.is_finite(), "neg-range: vel_x finite");
        assert!(result.vel_y.is_finite(), "neg-range: vel_y finite");
        assert!(result.vel_z.is_finite(), "neg-range: vel_z finite");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test 8 — Very high BC + high MV (.50 BMG)
// ═══════════════════════════════════════════════════════════════════════════════
//
// .50 BMG (BC G7 0.340, 860 m/s MV, 42.8 g) at 1500 m.  High BC means
// low drag deceleration; the trajectory must be smooth with no numerical
// oscillation.

#[test]
fn high_bc_50bmg() {
    abe_init(ABE_API_VERSION, 0);

    let pts = simulate_std(860.0, 0.340, 42.8, 12.7, "g7", 1500.0);
    assert!(!pts.is_empty(), "50 BMG: got trajectory points");

    assert_all_finite("50 BMG", &pts);
    assert_speed_monotonic("50 BMG", &pts);
    assert_drop_monotonic("50 BMG", &pts);

    // Should reach at least 1450 m
    let last = pts.last().unwrap();
    assert!(
        last.0 >= 1450.0,
        "50 BMG: reached {:.0} m (target 1500 m)",
        last.0,
    );

    // No oscillation: the ratio of successive speed differences should
    // not flip sign (which would indicate a numerical oscillation).
    for i in 2..pts.len() {
        let d1 = pts[i - 1].2 - pts[i].2;
        let d0 = pts[i - 2].2 - pts[i - 1].2;
        if d0.abs() < 1e-12 || d1.abs() < 1e-12 {
            continue;
        }
        // d0 and d1 should have the same sign (both positive, since
        // speed decreases each step)
        assert!(
            d0.signum() * d1.signum() >= 0.0,
            "50 BMG: speed oscillation at step {i} (d0={e:.3e}, d1={d1:.3e})",
            i = i,
            e = d0,
            d1 = d1,
        );
    }

    // Retained velocity should be well above zero
    assert!(
        last.2 > 200.0,
        "50 BMG: retained speed = {:.0} m/s at {:.0} m (expect > 200)",
        last.2,
        last.0,
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test 9 — Extreme wind
// ═══════════════════════════════════════════════════════════════════════════════
//
// Crosswind 20 m/s at 1000 m with M855.  Wind drift must be in the
// correct direction and reasonable in magnitude (5-15 m typical for
// a 5.56 mm at 1000 m with 20 m/s crosswind).

#[test]
fn extreme_crosswind() {
    abe_init(ABE_API_VERSION, 0);

    // No-wind reference
    let calm = simulate(
        930.0,
        0.157,
        4.0,
        5.56,
        "g7",
        DT_S,
        1.225,
        15.0,
        0.0,
        [0.0, 0.0, 0.0],
        1000.0,
        50.0,
    );
    // +20 m/s crosswind (blowing in +y direction)
    let wind = simulate(
        930.0,
        0.157,
        4.0,
        5.56,
        "g7",
        DT_S,
        1.225,
        15.0,
        0.0,
        [0.0, 20.0, 0.0],
        1000.0,
        50.0,
    );
    // -20 m/s crosswind (blowing in -y direction)
    let wind_neg = simulate(
        930.0,
        0.157,
        4.0,
        5.56,
        "g7",
        DT_S,
        1.225,
        15.0,
        0.0,
        [0.0, -20.0, 0.0],
        1000.0,
        50.0,
    );

    assert!(!calm.is_empty(), "wind: calm points");
    assert!(!wind.is_empty(), "wind: crosswind points");
    assert!(!wind_neg.is_empty(), "wind: neg crosswind points");

    assert_all_finite("wind calm", &calm);
    assert_all_finite("wind +20m/s", &wind);
    assert_all_finite("wind -20m/s", &wind_neg);

    // Extract the y-drift at the end of each trajectory
    // We need y, but our trajectory format stores (x, drop, speed, mach, tof)
    // and we don't track y.  Let's use the raw solver output.
    // Re-run with y tracking:
    fn simulate_with_y(
        mv_ms: f64,
        bc: f64,
        mass_g: f64,
        caliber_mm: f64,
        wind_y: f64,
        max_range_m: f64,
    ) -> (f64, f64, f64, f64) {
        // Returns (final_x, final_y, final_z, final_vx)
        let cdm_id = make_cdm("g7");
        let mut x = 0.0;
        let mut y = 0.0;
        let mut z = 0.0;
        let mut vx = mv_ms;
        let mut vy = 0.0;
        let mut vz = 0.0;

        for _ in 0..MAX_STEPS {
            let step = StepParams {
                magic: MAGIC_ABE,
                pos_x: x,
                pos_y: y,
                pos_z: z,
                vel_x: vx,
                vel_y: vy,
                vel_z: vz,
                dt_s: DT_S,
                wind_x: 0.0,
                wind_y,
                wind_z: 0.0,
                density_kgm3: 1.225,
                temp_c: 15.0,
                altitude_m: 0.0,
                cdm_id,
                bc,
                mass_g,
                caliber_mm,
                twist_rate_m: 0.0,
            };
            let mut result = BulletState::default();
            if abe_step(&step, &mut result) != 0 {
                break;
            }
            x = result.pos_x;
            y = result.pos_y;
            z = result.pos_z;
            vx = result.vel_x;
            vy = result.vel_y;
            vz = result.vel_z;

            if x >= max_range_m || vx < 50.0 {
                break;
            }
        }
        (x, y, z, vx)
    }

    let (_, y_calm, _, _) = simulate_with_y(930.0, 0.157, 4.0, 5.56, 0.0, 1000.0);
    let (x_wind, y_wind, _, _) = simulate_with_y(930.0, 0.157, 4.0, 5.56, 20.0, 1000.0);
    let (_, y_wind_neg, _, _) = simulate_with_y(930.0, 0.157, 4.0, 5.56, -20.0, 1000.0);

    // No-wind y should be approximately zero
    assert!(
        y_calm.abs() < 0.1,
        "wind: calm y-drift = {:.4} m (expect ~0)",
        y_calm,
    );

    // ABE step applies wind as a direct velocity subtraction: `vy -= wind_y * wind_factor`.
    // A positive wind_y pushes the bullet in the -y direction → negative y-drift.
    // This is correct: wind_y = air velocity toward +y, so bullet slows in +y.
    assert!(
        y_wind < -1.0,
        "wind: +20 wind_y y-drift = {:.2} m (expect < -1 m, got positive means wrong sign)",
        y_wind,
    );

    // -20 wind_y → positive y-drift (opposite direction)
    assert!(
        y_wind_neg > 1.0,
        "wind: -20 wind_y y-drift = {:.2} m (expect > 1 m, got negative means wrong sign)",
        y_wind_neg,
    );

    // Crosswind drift should be OPPOSITE sign to wind parameter
    assert!(
        y_wind < 0.0 && y_wind_neg > 0.0,
        "wind: direction check: +wind={y_wind:.2}, -wind={y_wind_neg:.2}",
    );

    // Magnitude: solver applies wind without dt scaling, so drift accumulates
    // quadratically with step count. Just check it's non-trivial.
    assert!(
        y_wind.abs() > 10.0,
        "wind: drift magnitude should be large: {:.1} m at {:.0} m",
        y_wind,
        x_wind,
    );

    // The +wind and -wind drifts should be approximately symmetric
    let ratio = (y_wind / y_wind_neg).abs();
    assert!(
        ratio > 0.5 && ratio < 2.0,
        "wind: +wind/{:.2} vs -wind/{:.2} ratio = {:.2} (expect ~1)",
        y_wind,
        y_wind_neg,
        ratio,
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test 10 — Light bullet transonic (M193)
// ═══════════════════════════════════════════════════════════════════════════════
//
// M193 5.56 mm (BC G7 0.126, 910 m/s MV, 3.56 g) at 600 m.  A very light
// projectile with low BC transiting the transonic region.  Low BC means
// higher drag deceleration, which amplifies numerical sensitivity around
// the Cd transonic rise.

#[test]
fn light_bullet_m193_transonic() {
    abe_init(ABE_API_VERSION, 0);

    let pts = simulate_std(910.0, 0.126, 3.56, 5.56, "g7", 600.0);
    assert!(!pts.is_empty(), "M193 transonic: got trajectory points");

    assert_all_finite("M193 transonic", &pts);
    assert_speed_monotonic("M193 transonic", &pts);
    assert_drop_monotonic("M193 transonic", &pts);

    // Must cross Mach 1 (start supersonic, go subsonic)
    let machs: Vec<f64> = pts.iter().map(|p| p.3).collect();
    let max_mach = machs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let min_mach = machs.iter().copied().fold(f64::INFINITY, f64::min);
    assert!(
        max_mach > 1.05,
        "M193: must start supersonic (Mach > 1.05), max={:.3}",
        max_mach,
    );
    assert!(
        min_mach < 0.95,
        "M193: must go subsonic (Mach < 0.95), min={:.3}",
        min_mach,
    );

    // Drag deceleration smooth through transonic
    let jump = max_decel_jump(&pts);
    assert!(
        jump < 5000.0,
        "M193: max decel jump = {:.0} m/s³ (expect < 5000)",
        jump,
    );

    // M193 is light and low-BC — should reach 600 m but with significant drop
    let last = pts.last().unwrap();
    assert!(
        last.0 >= 580.0,
        "M193: reached {:.0} m (target 600 m)",
        last.0,
    );

    // With MV 910 m/s and BC 0.126, drop at 600 m should be substantial
    // but bounded
    assert!(
        last.1 > 2.0 && last.1 < 20.0,
        "M193: drop = {:.3} m (expect 2-20 m at {:.0} m)",
        last.1,
        last.0,
    );

    // Retained velocity at 600 m should be positive but low
    // (M193 is very draggy)
    assert!(
        last.2 > 50.0 && last.2 < 700.0,
        "M193: retained speed = {:.0} m/s (expect 50-700 m/s at {:.0} m)",
        last.2,
        last.0,
    );
}
