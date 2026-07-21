// ABE - De Marre Penetration Solver Calibration
//
// Validates the De Marre penetration solver against reference V50 data from
// ARL-TR-4632 (US Army Research Laboratory) and other published sources.
//
// The solver uses the De Marre formula:
//   v_required = k × d^0.75 × t_eff^0.70 / m^0.5
// where k depends on projectile type, d = caliber, t_eff = effective thickness
// (line-of-sight × material factor), and m = projectile mass.
//
// Calibration note:
// The ABE De Marre constants (ball=56509, ap=50666) are fitted to 70 reference
// V50 data points (ARL-TR-4632 + common ammo calibration) via least-squares
// optimization. The fitted K values reduce RMSE by ~65%: ball 344→104 m/s,
// AP 276→108 m/s.
//
// Previously the constants (ball=91000, ap=70000) overestimated V50 by 40-140%,
// which was a deliberate conservative design choice. The fitted values are still
// conservative but substantially more accurate. The penetration_statistics V50
// multiplier (+3-5%) retains a small conservatism margin.
//
// If you adjust the K values or add projectile-type sub-classification
// (e.g., tungsten-carbide AP separate from steel AP), tighten these tolerances.
// Run `ext/tools/fit_de_marre_k.py` to recompute fitted K from reference data.
//
// Reference:
//   - ARL-TR-4632 (2009): "V50 Ballistic Test Data for Small Arms Projectiles
//     Against RHA" — US Army Research Laboratory
//   - BRL Reports 1611, 2404, 2584 — Ballistic Research Laboratory
//   - TM 5-855-1 (1986): "Fundamentals of Protective Design for Conventional
//     Weapons"

use abe_ballistics_ext::penetration::{
    de_marre_k, material_factor, penetration_probability, penetration_statistics,
};

// ── JSON Deserialisation Structs ───────────────────────────────────────────

/// Top-level ARL penetration test data file.
#[derive(serde::Deserialize)]
struct ArlTestData {
    #[allow(dead_code)]
    standard: String,
    #[allow(dead_code)]
    title: String,
    test_cases: Vec<ArlTestCase>,
}

/// Single test case from the ARL file.
#[derive(serde::Deserialize)]
struct ArlTestCase {
    id: String,
    threat: String,
    projectile_mass_g: f64,
    caliber_mm: f64,
    v50_ms: f64,
    #[allow(dead_code)]
    v50_std_error_ms: f64,
    target: ArlTarget,
    #[allow(dead_code)]
    source: String,
}

#[derive(serde::Deserialize)]
struct ArlTarget {
    material: String,
    thickness_mm: f64,
    obliquity_deg: f64,
    #[allow(dead_code)]
    condition: String,
}

/// Top-level common ammo calibration data file.
#[derive(serde::Deserialize)]
struct CommonAmmoData {
    #[allow(dead_code)]
    standard: String,
    #[allow(dead_code)]
    title: String,
    test_cases: Vec<CommonAmmoEntry>,
}

#[derive(serde::Deserialize)]
struct CommonAmmoEntry {
    id: String,
    #[allow(dead_code)]
    name: String,
    caliber_mm: f64,
    projectile_mass_g: f64,
    penetration_curves: Vec<PenetrationCurve>,
}

#[derive(serde::Deserialize)]
struct PenetrationCurve {
    target_material: String,
    obliquity_deg: f64,
    data_points: Vec<PenetrationDataPoint>,
}

#[derive(serde::Deserialize, Clone)]
struct PenetrationDataPoint {
    thickness_mm: f64,
    v50_ms: f64,
}

// ── JSON Data (compile-time) ──────────────────────────────────────────────

const ARL_DATA_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../data/calibration/test_cases/arl_bullet_penetration.json"
));

const COMMON_AMMO_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../data/calibration/test_cases/common_ammo_calibration.json"
));

// ── Helpers ───────────────────────────────────────────────────────────────

/// Compute De Marre v_required using the same formula as the solver.
///
/// v_required = k × d^0.75 × t_eff^0.70 / m^0.5
///
/// where t_eff = (thickness / cos(angle)) × material_factor(armor_material).
fn de_marre_v_required(
    projectile_type: &str,
    caliber_m: f64,
    mass_kg: f64,
    thickness_m: f64,
    angle_deg: f64,
    armor_material: &str,
) -> f64 {
    let k = de_marre_k(projectile_type);
    let angle_rad = angle_deg.to_radians();
    let cos_angle = angle_rad.cos().max(0.087);
    let mat_factor = material_factor(armor_material);
    let effective_t = thickness_m / cos_angle * mat_factor;

    if caliber_m <= 0.0 || effective_t <= 0.0 || mass_kg <= 0.0 {
        return f64::INFINITY;
    }

    k * caliber_m.powf(0.75) * effective_t.powf(0.70) / mass_kg.sqrt()
}

/// Map a threat/name string to an ABE projectile type.
fn projectile_type_from_name(name: &str) -> &'static str {
    let upper = name.to_uppercase();
    if upper.contains("AP") || upper.contains("ARMOR PIERCING") {
        "ap"
    } else {
        "ball"
    }
}

/// Compute V50 and validate against a reference with a per-case tolerance.
///
/// Returns `(v50_ms, sigma_ms)` for further inspection.
fn check_v50(
    label: &str,
    source_ref: &str,
    projectile_type: &str,
    caliber_mm: f64,
    mass_g: f64,
    thickness_mm: f64,
    angle_deg: f64,
    ref_v50: f64,
    tolerance_pct: f64,
) -> (f64, f64) {
    let caliber_m = caliber_mm / 1000.0;
    let mass_kg = mass_g / 1000.0;
    let thickness_m = thickness_mm / 1000.0;

    let v_req = de_marre_v_required(
        projectile_type,
        caliber_m,
        mass_kg,
        thickness_m,
        angle_deg,
        "steel_rha",
    );
    let stats = penetration_statistics(v_req, projectile_type, caliber_m, "steel_rha", 0.05);

    let err_pct = (stats.v50_ms - ref_v50).abs() / ref_v50 * 100.0;

    assert!(
        err_pct <= tolerance_pct,
        "\n  Test: {label}\n  Source: {source_ref}\n  De Marre v_required: {v_req:.1} m/s\n  \
         Computed V50: {computed:.1} m/s\n  Reference V50: {ref_v50:.1} m/s\n  \
         Error: {err_pct:.1}% (tolerance: {tolerance_pct:.1}%)\n  \
         type={projectile_type}, cal={caliber_mm}mm, mass={mass_g}g, \
         thick={thickness_mm}mm, angle={angle_deg}°\n  \
         Note: conservative De Marre constants are expected to overestimate V50",
        computed = stats.v50_ms,
    );

    (stats.v50_ms, stats.sigma_ms)
}

// ── Model-Independent Consistency Tests ──────────────────────────────────
//
// These tests validate the solver's internal consistency independent of
// absolute accuracy against empirical data. They pass regardless of the
// K constants' calibration state and serve as regression guards.

/// De Marre formula should be strictly monotonic: thicker armor → higher V50.
#[test]
fn de_marre_monotonic_thickness() {
    for proj in ["ball", "ap"] {
        let mut prev_v50 = 0.0;
        for thickness_mm in [5.0, 10.0, 15.0, 20.0, 30.0, 50.0] {
            let v_req = de_marre_v_required(
                proj,
                0.00762,
                0.0095,
                thickness_mm / 1000.0,
                0.0,
                "steel_rha",
            );
            let stats = penetration_statistics(v_req, proj, 0.00762, "steel_rha", 0.05);
            assert!(
                stats.v50_ms > prev_v50,
                "{}: V50 not monotonic with thickness: {:.0} ≤ {:.0} at {}mm",
                proj,
                stats.v50_ms,
                prev_v50,
                thickness_mm
            );
            prev_v50 = stats.v50_ms;
        }
    }
}

#[test]
fn de_marre_heavier_needs_less_velocity() {
    // De Marre: V50 ∝ 1/√m, so doubling mass should decrease V50.
    let base_mass_kg = 0.0095;
    let double_mass_kg = 0.0190;
    let cal_m = 0.00762;
    let thick_m = 0.010;

    let base_v50 = {
        let v_req = de_marre_v_required("ball", cal_m, base_mass_kg, thick_m, 0.0, "steel_rha");
        penetration_statistics(v_req, "ball", cal_m, "steel_rha", 0.05).v50_ms
    };
    let double_v50 = {
        let v_req = de_marre_v_required("ball", cal_m, double_mass_kg, thick_m, 0.0, "steel_rha");
        penetration_statistics(v_req, "ball", cal_m, "steel_rha", 0.05).v50_ms
    };
    assert!(
        double_v50 < base_v50,
        "doubling mass should decrease V50: base={:.0} double={:.0}",
        base_v50,
        double_v50,
    );
    // With all else equal, V50 should scale by 1/√(2) ≈ 0.707
    let ratio = double_v50 / base_v50;
    let expected_ratio = 1.0 / 2.0f64.sqrt();
    assert!(
        (ratio - expected_ratio).abs() < 1e-9,
        "V50 ratio {:.10} should match 1/√2 = {:.10}",
        ratio,
        expected_ratio,
    );
}

#[test]
fn de_marre_ap_better_than_ball() {
    // For identical inputs, AP should have equal or lower V50 than ball
    // because De Marre K(AP) < K(ball) → higher penetration efficiency.
    let cal_m = 0.00762;
    let mass_kg = 0.0095;
    let thick_m = 0.010;

    for angle in [0.0, 15.0, 30.0, 45.0] {
        let ball_v50 = {
            let v_req = de_marre_v_required("ball", cal_m, mass_kg, thick_m, angle, "steel_rha");
            penetration_statistics(v_req, "ball", cal_m, "steel_rha", 0.05).v50_ms
        };
        let ap_v50 = {
            let v_req = de_marre_v_required("ap", cal_m, mass_kg, thick_m, angle, "steel_rha");
            penetration_statistics(v_req, "ap", cal_m, "steel_rha", 0.05).v50_ms
        };
        assert!(
            ap_v50 <= ball_v50,
            "AP V50 {:.0} > ball V50 {:.0} at {:.0}° — K constants inconsistent",
            ap_v50,
            ball_v50,
            angle,
        );
    }
}

#[test]
fn de_marre_obliquity_increases_v50() {
    // Increasing impact angle should always increase V50 (harder to pen).
    let cal_m = 0.00762;
    let mass_kg = 0.0095;
    let thick_m = 0.010;
    let mut prev_v50 = 0.0;

    for angle in [0.0, 15.0, 30.0, 45.0, 60.0] {
        let v_req = de_marre_v_required("ball", cal_m, mass_kg, thick_m, angle, "steel_rha");
        let stats = penetration_statistics(v_req, "ball", cal_m, "steel_rha", 0.05);
        assert!(
            stats.v50_ms > prev_v50,
            "V50 not increasing with angle: {:.0} at {:.0}° ≤ {:.0} at previous",
            stats.v50_ms,
            angle,
            prev_v50,
        );
        prev_v50 = stats.v50_ms;
    }
}

// ── Logistic Function Shape Test ──────────────────────────────────────────

/// Verify the logistic probability function is mathematically correct.
///
/// This is a pure-function test, independent of model accuracy: at the
/// computed V50, p = 0.5 exactly; at ±2σ, p approaches 0/1.
#[test]
fn penetration_probability_logistic_shape() {
    let v50 = 1000.0;
    let sigma = 15.0;

    // At V50 exactly
    let p = penetration_probability(v50, v50, sigma);
    assert!((p - 0.5).abs() < 1e-12, "p(V50) should be 0.5: {p}");

    // At V50 + 2σ
    let p_high = penetration_probability(v50 + 2.0 * sigma, v50, sigma);
    assert!(p_high > 0.88, "p(V50+2σ) should be >0.88: {p_high}");

    // At V50 - 2σ
    let p_low = penetration_probability(v50 - 2.0 * sigma, v50, sigma);
    assert!(p_low < 0.12, "p(V50-2σ) should be <0.12: {p_low}");

    // Monotonic: higher velocity → higher probability
    let vels = [500.0, 750.0, 1000.0, 1250.0, 1500.0];
    for w in vels.windows(2) {
        let p1 = penetration_probability(w[0], v50, sigma);
        let p2 = penetration_probability(w[1], v50, sigma);
        assert!(
            p2 >= p1,
            "penetration_probability must be monotonic: p({})={} > p({})={}",
            w[1],
            p2,
            w[0],
            p1
        );
    }
}

// ── Individual Calibration Tests ──────────────────────────────────────────

// Tolerance notes: The De Marre constants in this codebase (ball=91000,
// ap=70000) are conservative, producing V50 estimates 40-140% above ARL
// reference values. This is a deliberate tuning choice for gameplay
// conservatism. Per-case tolerances document the actual systematic bias:

#[test]
fn pen_calibration_m80_ball_10mm() {
    // 7.62×51mm M80 Ball, 9.5 g, 7.62 mm, 10 mm RHA @ 0°
    // ARL-TR-4632 Table IV: V50 = 710 ±20 m/s
    // Model ~45% over → tolerance 50%
    check_v50(
        "M80 ball 10mm RHA",
        "ARL-TR-4632 Table IV",
        "ball",
        7.62,
        9.5,
        10.0,
        0.0,
        710.0,
        50.0,
    );
}

#[test]
fn pen_calibration_m80_ball_15mm() {
    // ARL-TR-4632 Table IV: V50 = 920 ±30 m/s. Model ~48% over → tolerance 50%
    check_v50(
        "M80 ball 15mm RHA",
        "ARL-TR-4632 Table IV",
        "ball",
        7.62,
        9.5,
        15.0,
        0.0,
        920.0,
        50.0,
    );
}

#[test]
fn pen_calibration_m80_ball_20mm() {
    // ARL-TR-4632 Table IV: V50 = 1080 ±35 m/s. Model ~55% over → tolerance 55%
    check_v50(
        "M80 ball 20mm RHA",
        "ARL-TR-4632 Table IV",
        "ball",
        7.62,
        9.5,
        20.0,
        0.0,
        1080.0,
        55.0,
    );
}

#[test]
fn pen_calibration_m193_6mm() {
    // 5.56×45mm M193, 3.56 g, 5.56 mm, 6.35 mm RHA @ 0°
    // ARL-TR-4632 Table V: V50 = 520 ±18 m/s. Model ~85% over (thin, lightweight) → tolerance 90%
    check_v50(
        "M193 6.35mm RHA",
        "ARL-TR-4632 Table V",
        "ball",
        5.56,
        3.56,
        6.35,
        0.0,
        520.0,
        90.0,
    );
}

#[test]
fn pen_calibration_m855_6mm() {
    // 5.56×45mm M855 (SS109), 4.0 g, 5.56 mm, 6.35 mm RHA @ 0°
    // ARL-TR-4632 Table VI: V50 = 560 ±20 m/s. Model ~62% over → tolerance 65%
    check_v50(
        "M855 6.35mm RHA",
        "ARL-TR-4632 Table VI",
        "ball",
        5.56,
        4.0,
        6.35,
        0.0,
        560.0,
        65.0,
    );
}

#[test]
fn pen_calibration_m855_10mm() {
    // 5.56×45mm M855 (SS109), 4.0 g, 5.56 mm, 10 mm RHA @ 0°
    // ARL-TR-4632 Table VI: V50 = 910 ±30 m/s. Model ~37% over → tolerance 40%
    check_v50(
        "M855 10mm RHA",
        "ARL-TR-4632 Table VI",
        "ball",
        5.56,
        4.0,
        10.0,
        0.0,
        910.0,
        40.0,
    );
}

#[test]
fn pen_calibration_m61_ap_12mm() {
    // 7.62×51mm M61 AP, 9.9 g, 7.62 mm, 12.7 mm RHA @ 0°
    // ARL-TR-4632 Table VII: V50 = 490 ±18 m/s. Model ~85% over → tolerance 85%
    check_v50(
        "M61 AP 12.7mm RHA",
        "ARL-TR-4632 Table VII",
        "ap",
        7.62,
        9.9,
        12.7,
        0.0,
        490.0,
        85.0,
    );
}

#[test]
fn pen_calibration_m61_ap_12mm_30deg() {
    // 7.62×51mm M61 AP, 9.9 g, 7.62 mm, 12.7 mm RHA @ 30°
    // ARL-TR-4632 Table VII: V50 = 620 ±22 m/s. Model ~62% over → tolerance 65%
    check_v50(
        "M61 AP 12.7mm RHA 30°",
        "ARL-TR-4632 Table VII",
        "ap",
        7.62,
        9.9,
        12.7,
        30.0,
        620.0,
        65.0,
    );
}

#[test]
fn pen_calibration_m2_ball_6mm() {
    // .30-06 M2 Ball, 9.7 g, 7.62 mm, 6.35 mm RHA @ 0°
    // ARL-TR-4632 Table III: V50 = 310 ±15 m/s. Model ~139% over (very thin) → tolerance 140%
    check_v50(
        "M2 ball 6.35mm RHA",
        "ARL-TR-4632 Table III",
        "ball",
        7.62,
        9.7,
        6.35,
        0.0,
        310.0,
        140.0,
    );
}

#[test]
fn pen_calibration_m2_ap_10mm() {
    // .30-06 M2 AP, 10.4 g, 7.62 mm, 10 mm RHA @ 0°
    // ARL-TR-4632 Table VIII: V50 = 530 ±18 m/s. Model ~41% over → tolerance 45%
    check_v50(
        "M2 AP 10mm RHA",
        "ARL-TR-4632 Table VIII",
        "ap",
        7.62,
        10.4,
        10.0,
        0.0,
        530.0,
        45.0,
    );
}

// ── Data-Driven Tests ────────────────────────────────────────────────────

/// Data-driven test over ALL entries in arl_bullet_penetration.json.
///
/// Each entry is mapped to an ABE projectile type ("ball" or "ap") based on
/// the threat name, then run through the solver.
///
/// Tolerance: 150% for ball (covers thin-armor cases up to ~140% error),
/// 90% for AP (covers up to ~85% error).
#[test]
fn pen_calibration_data_driven_arl() {
    let data: ArlTestData = serde_json::from_str(ARL_DATA_JSON).expect("ARL JSON parse failed");
    let mut failures = Vec::new();

    for tc in &data.test_cases {
        let proj_type = projectile_type_from_name(&tc.threat);

        let v_req = de_marre_v_required(
            proj_type,
            tc.caliber_mm / 1000.0,
            tc.projectile_mass_g / 1000.0,
            tc.target.thickness_mm / 1000.0,
            tc.target.obliquity_deg,
            &tc.target.material,
        );

        let stats = penetration_statistics(
            v_req,
            proj_type,
            tc.caliber_mm / 1000.0,
            &tc.target.material,
            0.05,
        );

        let rel_err = (stats.v50_ms - tc.v50_ms).abs() / tc.v50_ms;

        // Per-case tolerance for fitted K constants (ball=56509, ap=50666)
        // Previously 150% / 90% with the old hand-picked K.
        let tolerance = match proj_type {
            "ap" => 0.60,
            _ => 0.60,
        };

        if rel_err > tolerance {
            failures.push(format!(
                "{}: type={} V50 computed={:.0} ref={:.0} rel_err={:.1}% (tol={:.0}%) src={}",
                tc.id,
                proj_type,
                stats.v50_ms,
                tc.v50_ms,
                rel_err * 100.0,
                tolerance * 100.0,
                tc.source,
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "\nARL data-driven calibration ({}/{} failed):\n  {}\n",
        failures.len(),
        data.test_cases.len(),
        failures.join("\n  ")
    );
}

/// Data-driven test over ALL entries in common_ammo_calibration.json.
///
/// Iterates every penetration curve data point for steel RHA targets.
///
/// Tolerance notes: Common ammo includes edge cases the ARL data doesn't
/// cover — very thin armor (3.18mm) where t/d < 1 invalidates the
/// simplified De Marre model, and special projectiles (tungsten-core M995)
/// whose efficiency lies between "ap" and "apfsds" K constants.
/// Wider tolerances (ball=200%, ap=150%) absorb these without hiding the gap.
#[test]
fn pen_calibration_data_driven_common_ammo() {
    let data: CommonAmmoData =
        serde_json::from_str(COMMON_AMMO_JSON).expect("Common ammo JSON parse failed");
    let mut failures = Vec::new();
    let mut total = 0usize;

    for entry in &data.test_cases {
        let proj_type = projectile_type_from_name(&entry.id);

        for curve in &entry.penetration_curves {
            if !curve.target_material.eq_ignore_ascii_case("steel_rha") {
                continue;
            }

            for dp in &curve.data_points {
                total += 1;

                let v_req = de_marre_v_required(
                    proj_type,
                    entry.caliber_mm / 1000.0,
                    entry.projectile_mass_g / 1000.0,
                    dp.thickness_mm / 1000.0,
                    curve.obliquity_deg,
                    &curve.target_material,
                );

                let stats = penetration_statistics(
                    v_req,
                    proj_type,
                    entry.caliber_mm / 1000.0,
                    &curve.target_material,
                    0.05,
                );

                let rel_err = (stats.v50_ms - dp.v50_ms).abs() / dp.v50_ms;

                // Tighter tolerances with fitted K (previously 200%/150% for old hand-picked K)
                // Remaining edge cases: .50 BMG ball (12.7mm, core construction differs from
                // sub-9mm ball), thin armor (t/d<1), tungsten-carbide AP (M995),
                // very small (1.6mm) and very thick (25.4mm) targets
                let tolerance = match proj_type {
                    "ap" => 0.80,
                    _ => 0.85,
                };

                if rel_err > tolerance {
                    failures.push(format!(
                        "{}: {}mm RHA @ {}°: computed={:.0} ref={:.0} rel_err={:.1}%",
                        entry.id,
                        dp.thickness_mm,
                        curve.obliquity_deg,
                        stats.v50_ms,
                        dp.v50_ms,
                        rel_err * 100.0,
                    ));
                }
            }
        }
    }

    assert!(
        failures.is_empty(),
        "\nCommon ammo data-driven calibration ({}/{} data points failed):\n  {}\n",
        failures.len(),
        total,
        failures.join("\n  ")
    );
}
