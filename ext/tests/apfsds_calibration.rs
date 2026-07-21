// ABE — APFSDS Lanz-Odermatt Calibration Tests
//
// Validates the long-rod penetration model against published reference data.
// NOTE: Model uses textbook k=2.0 default (uncalibrated) — over-predicts
// absolute penetration by ~1.7×. Tests verify monotonicity and relative
// behavior. Once k is calibrated to ~3.5 (from 41-shot Odermatt corpus),
// tighten tolerances to ±15%.
//
// References:
//   - Lanz & Odermatt, "Penetration Limits of Conventional Large Caliber
//     Anti Tank Guns / Kinetic Energy Projectiles", 1996
//   - Rosenberg & Dekel, "Terminal Ballistics", 2nd ed., Springer, 2016

use abe_ballistics_ext::penetration::long_rod;

fn rod_params(
    rod_length_mm: f64,
    rod_diameter_mm: f64,
    rod_density_kgm3: f64,
    velocity_ms: f64,
    angle_deg: f64,
) -> long_rod::LongRodParams {
    let ld = if rod_diameter_mm > 0.0 {
        rod_length_mm / rod_diameter_mm
    } else {
        10.0
    };
    long_rod::LongRodParams {
        rod_length_mm,
        rod_diameter_mm,
        rod_density_kgm3,
        impact_velocity_ms: velocity_ms,
        impact_angle_deg: angle_deg,
        target_density_kgm3: 7850.0,
        target_yield_strength_m_pa: 1000.0,
        rod_fineness_ratio: ld,
    }
}

// ── Invariant: penetration depth ≥ rod length for high-velocity impacts
#[test]
fn high_velocity_exceeds_rod_length() {
    let r = long_rod::evaluate_long_rod(&rod_params(500.0, 25.0, 17500.0, 1800.0, 0.0));
    assert!(
        r.penetration_depth_mm > 500.0,
        "1800 m/s rod should penetrate > its own length: {:.0}mm",
        r.penetration_depth_mm
    );
}

// ── Invariant: higher L/D → deeper penetration (L/D efficiency)
#[test]
fn higher_ld_deeper_penetration() {
    let low = long_rod::evaluate_long_rod(&rod_params(400.0, 40.0, 17500.0, 1600.0, 0.0));
    let high = long_rod::evaluate_long_rod(&rod_params(600.0, 20.0, 17500.0, 1600.0, 0.0));
    assert!(
        high.penetration_efficiency > low.penetration_efficiency,
        "Higher L/D should increase efficiency: L/D=30 ({:.4}) vs L/D=10 ({:.4})",
        high.penetration_efficiency,
        low.penetration_efficiency
    );
}

// ── Invariant: denser rod penetrates more
#[test]
fn denser_rod_penetrates_more() {
    let wha = long_rod::evaluate_long_rod(&rod_params(500.0, 25.0, 17500.0, 1600.0, 0.0));
    let du = long_rod::evaluate_long_rod(&rod_params(500.0, 25.0, 19000.0, 1600.0, 0.0));
    assert!(
        du.penetration_depth_mm > wha.penetration_depth_mm,
        "DU rod ({:.0}mm) should penetrate more than WHA ({:.0}mm)",
        du.penetration_depth_mm,
        wha.penetration_depth_mm
    );
}

// ── Invariant: higher velocity → more penetration
#[test]
fn higher_velocity_deeper_penetration() {
    let low = long_rod::evaluate_long_rod(&rod_params(500.0, 25.0, 17500.0, 1200.0, 0.0));
    let high = long_rod::evaluate_long_rod(&rod_params(500.0, 25.0, 17500.0, 1800.0, 0.0));
    assert!(
        high.penetration_depth_mm > low.penetration_depth_mm,
        "1800 m/s ({:.0}mm) should penetrate more than 1200 m/s ({:.0}mm)",
        high.penetration_depth_mm,
        low.penetration_depth_mm
    );
}

// ── Invariant: finite rod erodes at high velocity
#[test]
fn finite_rod_erodes() {
    let r = long_rod::evaluate_long_rod(&rod_params(200.0, 20.0, 17500.0, 900.0, 0.0));
    assert!(
        r.rod_eroded || r.residual_rod_length_mm < 50.0,
        "200mm rod at 900 m/s should be near-fully eroded: residual={:.0}mm",
        r.residual_rod_length_mm
    );
}

// ── Invariant: obliquity reduces net penetrative capability
// Lanz-Odermatt P/L gives penetration along the trajectory path, which
// increases with obliquity. Effective penetration perpendicular to the
// target face = P × cos(θ) and decreases with angle.
#[test]
fn obliquity_reduces_effective_penetration() {
    let r0 = long_rod::evaluate_long_rod(&rod_params(500.0, 25.0, 17500.0, 1600.0, 0.0));
    let r60 = long_rod::evaluate_long_rod(&rod_params(500.0, 25.0, 17500.0, 1600.0, 60.0));

    let eff_p0 = r0.penetration_depth_mm * 0.0_f64.to_radians().cos();
    let eff_p60 = r60.penetration_depth_mm * 60.0_f64.to_radians().cos();

    assert!(
        eff_p60 < eff_p0,
        "Effective penetration at 60° ({:.0}mm) should be less than at 0° ({:.0}mm)",
        eff_p60,
        eff_p0
    );
}

// ── Invariant: V3 correction balances efficiency for L/D > 30
#[test]
fn v3_correction_activates_at_high_ld() {
    let ld_32 = long_rod::evaluate_long_rod(&rod_params(800.0, 25.0, 17500.0, 1700.0, 0.0));
    let ld_20 = long_rod::evaluate_long_rod(&rod_params(500.0, 25.0, 17500.0, 1700.0, 0.0));
    let pl_32 = ld_32.penetration_depth_mm / 800.0;
    let pl_20 = ld_20.penetration_depth_mm / 500.0;
    assert!(
        (pl_32 - pl_20).abs() < 0.5,
        "P/L ratio similar across L/D: L/D=32 P/L={:.4}, L/D=20 P/L={:.4}",
        pl_32,
        pl_20
    );
}

// ── Crater diameter sanity check
#[test]
fn crater_diameter_reasonable() {
    let r = long_rod::evaluate_long_rod(&rod_params(500.0, 25.0, 17500.0, 1600.0, 0.0));
    assert!(
        r.crater_diameter_mm > 30.0 && r.crater_diameter_mm < 100.0,
        "Crater diameter {:.0}mm should be 1.5-4× rod dia (25mm)",
        r.crater_diameter_mm
    );
}

// ── Invalid input does not crash
#[test]
fn zero_velocity_no_penetration() {
    let r = long_rod::evaluate_long_rod(&rod_params(500.0, 25.0, 17500.0, 0.0, 0.0));
    assert_eq!(r.penetration_depth_mm, 0.0);
}
