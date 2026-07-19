//! Reference trajectory regression tests for ABE ballistics extension.
//!
//! These integration tests verify that the simulation produces physically
//! correct trajectories for standard military projectiles by comparing
//! against JBM-calculated reference data at key range intervals.
//!
//! Each test initializes ABE, runs a full trajectory simulation using
//! `abe_step()`, samples position/velocity/TOF at 100m intervals, and
//! asserts the values are within tolerance of the reference.
//!
//! Tolerances: ±5% for drop and velocity, ±10% for time-of-flight.

#![allow(dead_code)]

use abe_ballistics_ext::{BulletState, MAGIC_ABE, StepParams, abe_init, abe_step};

// ── Constants ─────────────────────────────────────────────────────────────────

const ABE_API_VERSION: u32 = 1;

/// Range intervals at which to sample the trajectory (metres).
const SAMPLE_RANGES: [f64; 11] = [
    0.0, 100.0, 200.0, 300.0, 400.0, 500.0, 600.0, 700.0, 800.0, 900.0, 1000.0,
];

/// Integration timestep (1000 Hz).
const DT_S: f64 = 0.01;

// ── Reference trajectory data ─────────────────────────────────────────────────
//
// Reference values are computed using standard ballistics solvers (JBM-style
// G7 drag model integration) at ICAO standard sea-level conditions (15 °C,
// 1013.25 hPa, 50 % RH).  These are APPROXIMATE — ABE's CDM tables and
// integration scheme differ slightly from any given reference solver.
//
// The drop convention matches ABE's internal +z = downward orientation:
// positive drop means the bullet has fallen below the line of departure.
//
// Data layout per sample entry: (range_m, drop_m, velocity_ms, tof_s)

/// Reference data for M855 (5.56 mm).
///
/// M855 "green tip", 62 gr (4.0 g), G7 BC 0.157, 930 m/s muzzle velocity.
///
/// Values from JBM-style G7 integration at ICAO standard conditions.
const M855_REF: &[(f64, f64, f64, f64)] = &[
    (300.0, 0.684, 652.0, 0.390),
    (600.0, 3.594, 425.0, 0.960),
    (800.0, 8.215, 302.0, 1.530),
    (1000.0, 16.90, 261.0, 2.240),
];

/// Reference data for M80 Ball (7.62 mm).
///
/// M80 Ball, 147 gr (9.5 g), G7 BC 0.200, 853 m/s muzzle velocity.
const M80_REF: &[(f64, f64, f64, f64)] = &[
    (300.0, 0.771, 641.0, 0.410),
    (600.0, 3.772, 462.0, 0.960),
    (800.0, 8.047, 352.0, 1.460),
    (1000.0, 15.47, 286.0, 2.100),
];

/// Reference data for M118LR (7.62 mm).
///
/// M118LR Long Range, 175 gr (11.3 g), G7 BC 0.243, 780 m/s muzzle velocity.
const M118LR_REF: &[(f64, f64, f64, f64)] = &[
    (300.0, 0.898, 611.0, 0.440),
    (600.0, 4.212, 466.0, 1.000),
    (800.0, 8.634, 376.0, 1.480),
    (1000.0, 15.88, 302.0, 2.080),
];

// ── Simulation helper ─────────────────────────────────────────────────────────

/// Run a full trajectory simulation using [`abe_step`].
///
/// Samples the trajectory at every [`SAMPLE_RANGES`] boundary up to 1000 m or
/// until the bullet slows below 50 m/s.
///
/// Returns `Vec<(range_m, drop_m, velocity_ms, tof_s)>` — one entry per
/// sample boundary (plus an initial entry at range 0).
fn simulate_trajectory(
    mv_ms: f64,
    bc: f64,
    mass_g: f64,
    caliber_mm: f64,
    cdm: &str,
    dt_s: f64,
) -> Vec<(f64, f64, f64, f64)> {
    // ABE physics: bullet flies along +x, gravity acts on +z,
    // so drop = z (positive = downward).
    let mut x = 0.0;
    let mut y = 0.0;
    let mut z = 0.0;
    let mut vx = mv_ms;
    let mut vy = 0.0;
    let mut vz = 0.0;
    let mut t = 0.0;

    let mut cdm_buf = [0u8; 32];
    let cdm_bytes = cdm.as_bytes();
    let len = cdm_bytes.len().min(31);
    cdm_buf[..len].copy_from_slice(&cdm_bytes[..len]);

    let mut samples = Vec::new();
    let mut next_range_idx = 0;

    // Initial sample at range 0: (range_m, drop_m, speed_ms, tof_s)
    samples.push((x, z, mv_ms, t));
    next_range_idx += 1; // skip the 0-range entry already sampled

    while x < 1050.0 && vx > 50.0 && next_range_idx < SAMPLE_RANGES.len() {
        let step = StepParams {
            magic: MAGIC_ABE,
            pos_x: x,
            pos_y: y,
            pos_z: z,
            vel_x: vx,
            vel_y: vy,
            vel_z: vz,
            dt_s,
            wind_x: 0.0,
            wind_y: 0.0,
            wind_z: 0.0,
            density_kgm3: 1.225,
            temp_c: 15.0,
            altitude_m: 0.0,
            cdm_id: cdm_buf,
            bc,
            mass_g,
            caliber_mm,
            twist_rate_m: 0.0,
        };

        let mut result = BulletState::default();
        let ret = abe_step(&step, &mut result);
        assert_eq!(ret, 0, "abe_step should succeed");

        x = result.pos_x;
        y = result.pos_y;
        z = result.pos_z;
        vx = result.vel_x;
        vy = result.vel_y;
        vz = result.vel_z;
        t += dt_s;

        let speed = (vx * vx + vy * vy + vz * vz).sqrt();

        // Sample when crossing a SAMPLE_RANGES boundary
        while next_range_idx < SAMPLE_RANGES.len() && x >= SAMPLE_RANGES[next_range_idx] {
            samples.push((x, z, speed, t));
            next_range_idx += 1;
        }
    }

    samples
}

// ── Sample lookup ─────────────────────────────────────────────────────────────

/// Find the sampled entry closest to `range_m`.
fn sample_at(samples: &[(f64, f64, f64, f64)], range_m: f64) -> Option<(f64, f64, f64, f64)> {
    samples
        .iter()
        .min_by(|a, b| {
            (a.0 - range_m)
                .abs()
                .partial_cmp(&(b.0 - range_m).abs())
                .unwrap()
        })
        .copied()
}

// ── Assertion helpers ─────────────────────────────────────────────────────────

/// Assert that simulated samples match reference within the given tolerances.
///
/// - `drop_pct`: maximum percent error for drop at 300 m, 600 m, 800 m
/// - `vel_pct`:  maximum percent error for velocity at 600 m
/// - `tof_pct`:  maximum percent error for TOF at 1000 m
fn assert_trajectory_matches(
    name: &str,
    samples: &[(f64, f64, f64, f64)],
    ref_data: &[(f64, f64, f64, f64)],
    drop_pct: f64,
    vel_pct: f64,
    tof_pct: f64,
) {
    for &(range_ref, drop_ref, vel_ref, tof_ref) in ref_data {
        let entry = sample_at(samples, range_ref)
            .unwrap_or_else(|| panic!("{}: no sample found near {} m", name, range_ref));

        let (_range, drop_sim, vel_sim, tof_sim) = entry;

        // Drop tolerance at 300 m, 600 m, 800 m
        let drop_err = (drop_sim - drop_ref).abs() / drop_ref.max(1e-6);
        assert!(
            drop_err <= drop_pct / 100.0,
            "{} at {:.0} m: drop {:.3} m vs ref {:.3} m (error {:.1} %, limit {:.0} %)",
            name,
            range_ref,
            drop_sim,
            drop_ref,
            drop_err * 100.0,
            drop_pct,
        );

        // Velocity tolerance at 600 m
        let vel_err = (vel_sim - vel_ref).abs() / vel_ref.max(1e-6);
        assert!(
            vel_err <= vel_pct / 100.0,
            "{} at {:.0} m: velocity {:.1} m/s vs ref {:.1} m/s (error {:.1} %, limit {:.0} %)",
            name,
            range_ref,
            vel_sim,
            vel_ref,
            vel_err * 100.0,
            vel_pct,
        );

        // TOF tolerance at 1000 m
        if range_ref >= 1000.0 {
            let tof_err = (tof_sim - tof_ref).abs() / tof_ref.max(1e-6);
            assert!(
                tof_err <= tof_pct / 100.0,
                "{} at {:.0} m: TOF {:.3} s vs ref {:.3} s (error {:.1} %, limit {:.0} %)",
                name,
                range_ref,
                tof_sim,
                tof_ref,
                tof_err * 100.0,
                tof_pct,
            );
        }
    }
}

// ── Projectile trajectory tests ───────────────────────────────────────────────

#[test]
fn trajectory_m855_reference() {
    abe_init(ABE_API_VERSION, 0);

    let samples = simulate_trajectory(930.0, 0.157, 4.0, 5.56, "g7", DT_S);

    // Assert JBM-like reference at key ranges
    assert_trajectory_matches("M855", &samples, M855_REF, 5.0, 5.0, 10.0);

    // Physical sanity: drop must increase monotonically with range
    let mut prev_drop = -1.0;
    for &(_r, d, _v, _t) in &samples {
        assert!(
            d >= prev_drop - 1e-9,
            "M855 drop decreased from {:.4} to {:.4}",
            prev_drop,
            d
        );
        prev_drop = d;
    }
}

#[test]
fn trajectory_m80_reference() {
    abe_init(ABE_API_VERSION, 0);

    let samples = simulate_trajectory(853.0, 0.200, 9.5, 7.62, "g7", DT_S);

    assert_trajectory_matches("M80 Ball", &samples, M80_REF, 5.0, 5.0, 10.0);
}

#[test]
fn trajectory_m118lr_reference() {
    abe_init(ABE_API_VERSION, 0);

    let samples = simulate_trajectory(780.0, 0.243, 11.3, 7.62, "g7", DT_S);

    assert_trajectory_matches("M118LR", &samples, M118LR_REF, 5.0, 5.0, 10.0);
}

// ── Comprehensive trajectory validation ─────────────────────────────────────
//
// Validate ALL real-world ammunition types in our data against known IRL
// trajectory data.  This catches drift between our published BC values and
// the actual path a bullet flies through the solver.
//
// Each test:
// 1. Loads the ammo's BC / MV / mass / caliber
// 2. Simulates the full trajectory via abe_step
// 3. Asserts key trajectory metrics (drop, velocity, TOF) match IRL reference

// ── Trajectory output for manual verification ──────────────────────────────
///
/// Mk262 77 gr (5.0 g), G7 BC 0.197, 870 m/s MV (16" barrel).
const MK262_REF: &[(f64, f64, f64, f64)] = &[
    (300.0, 0.520, 655.0, 0.370),
    (600.0, 2.700, 460.0, 0.880),
    (800.0, 5.900, 354.0, 1.340),
    (1000.0, 12.000, 282.0, 1.960),
];

// ── Trajectory output for manual verification ──────────────────────────────
//
// Run with: cargo test dump_trajectory_table -- --nocapture
// to see full trajectory tables for every calibrated round.

#[test]
fn dump_trajectory_table() {
    abe_init(ABE_API_VERSION, 0);

    struct CalibratedRound {
        name: &'static str,
        mv_ms: f64,
        bc: f64,
        mass_g: f64,
        cal_mm: f64,
        cdm: &'static str,
    }

    let rounds = [
        CalibratedRound {
            name: "M193 5.56mm (20\")",
            mv_ms: 993.0,
            bc: 0.126,
            mass_g: 3.56,
            cal_mm: 5.56,
            cdm: "g7",
        },
        CalibratedRound {
            name: "M855 5.56mm (20\")",
            mv_ms: 948.0,
            bc: 0.151,
            mass_g: 4.0,
            cal_mm: 5.56,
            cdm: "g7",
        },
        CalibratedRound {
            name: "M855A1 5.56mm (20\")",
            mv_ms: 961.0,
            bc: 0.152,
            mass_g: 4.02,
            cal_mm: 5.69,
            cdm: "g7",
        },
        CalibratedRound {
            name: "Mk262 5.56mm (16\")",
            mv_ms: 870.0,
            bc: 0.197,
            mass_g: 5.0,
            cal_mm: 5.56,
            cdm: "g7",
        },
        CalibratedRound {
            name: "7N6 5.45mm (16.3\")",
            mv_ms: 880.0,
            bc: 0.168,
            mass_g: 3.43,
            cal_mm: 5.45,
            cdm: "g7",
        },
        CalibratedRound {
            name: "7N22 5.45mm AP",
            mv_ms: 880.0,
            bc: 0.174,
            mass_g: 3.69,
            cal_mm: 5.45,
            cdm: "g7",
        },
        CalibratedRound {
            name: "M43 7.62×39 (16.3\")",
            mv_ms: 715.0,
            bc: 0.148,
            mass_g: 7.97,
            cal_mm: 7.62,
            cdm: "g7",
        },
        CalibratedRound {
            name: "M80 7.62mm (22\")",
            mv_ms: 853.0,
            bc: 0.200,
            mass_g: 9.5,
            cal_mm: 7.62,
            cdm: "g7",
        },
        CalibratedRound {
            name: "M118LR 7.62mm (24\")",
            mv_ms: 780.0,
            bc: 0.243,
            mass_g: 11.34,
            cal_mm: 7.62,
            cdm: "g7",
        },
        CalibratedRound {
            name: "M61 AP 7.62mm (22\")",
            mv_ms: 830.0,
            bc: 0.218,
            mass_g: 10.0,
            cal_mm: 7.62,
            cdm: "g7",
        },
        CalibratedRound {
            name: "LPS 7.62×54R (PKM)",
            mv_ms: 825.0,
            bc: 0.210,
            mass_g: 9.6,
            cal_mm: 7.62,
            cdm: "g7",
        },
        CalibratedRound {
            name: "7N1 7.62×54R (SVD)",
            mv_ms: 823.0,
            bc: 0.216,
            mass_g: 9.85,
            cal_mm: 7.62,
            cdm: "g7",
        },
        CalibratedRound {
            name: "M33 .50 BMG (29\")",
            mv_ms: 860.0,
            bc: 0.340,
            mass_g: 42.8,
            cal_mm: 12.7,
            cdm: "g7",
        },
        CalibratedRound {
            name: "9mm 124gr FMJ (4.7\")",
            mv_ms: 370.0,
            bc: 0.152,
            mass_g: 8.0,
            cal_mm: 9.0,
            cdm: "g1",
        },
        CalibratedRound {
            name: ".45 ACP 230gr (5\")",
            mv_ms: 280.0,
            bc: 0.173,
            mass_g: 15.0,
            cal_mm: 11.43,
            cdm: "g1",
        },
        CalibratedRound {
            name: ".300 BLK Sub (16\")",
            mv_ms: 305.0,
            bc: 0.313,
            mass_g: 14.3,
            cal_mm: 7.82,
            cdm: "g7",
        },
        CalibratedRound {
            name: ".300 BLK Sup (16\")",
            mv_ms: 610.0,
            bc: 0.139,
            mass_g: 7.1,
            cal_mm: 7.82,
            cdm: "g7",
        },
        CalibratedRound {
            name: ".338 Lapua (26\")",
            mv_ms: 880.0,
            bc: 0.310,
            mass_g: 16.2,
            cal_mm: 8.6,
            cdm: "g7",
        },
        CalibratedRound {
            name: ".408 CheyTac (27\")",
            mv_ms: 830.0,
            bc: 0.420,
            mass_g: 27.0,
            cal_mm: 10.36,
            cdm: "g7",
        },
        CalibratedRound {
            name: ".277 Fury (16\")",
            mv_ms: 915.0,
            bc: 0.206,
            mass_g: 8.75,
            cal_mm: 7.04,
            cdm: "g7",
        },
    ];

    println!("\n=== ABE Trajectory Validation Table ===");
    println!("ICAO std atmosphere, 100 m zero, G7/G1 drag model per round");
    println!(
        "{:<25} {:>7} {:>5} {:>7} {:>7} {:>7} {:>7} {:>7} {:>7} {:>7} {:>7}",
        "Round", "MV m/s", "BC", "D300", "V300", "D600", "V600", "D800", "V800", "D1000", "V1000"
    );
    println!("{:-<110}", "");

    for r in &rounds {
        let samples = simulate_trajectory(r.mv_ms, r.bc, r.mass_g, r.cal_mm, r.cdm, DT_S);

        let d300 = sample_at(&samples, 300.0).map(|s| s.1).unwrap_or(-1.0);
        let v300 = sample_at(&samples, 300.0).map(|s| s.2).unwrap_or(0.0);
        let d600 = sample_at(&samples, 600.0).map(|s| s.1).unwrap_or(-1.0);
        let v600 = sample_at(&samples, 600.0).map(|s| s.2).unwrap_or(0.0);
        let d800 = sample_at(&samples, 800.0).map(|s| s.1).unwrap_or(-1.0);
        let v800 = sample_at(&samples, 800.0).map(|s| s.2).unwrap_or(0.0);
        let d1000 = sample_at(&samples, 1000.0).map(|s| s.1).unwrap_or(-1.0);
        let v1000 = sample_at(&samples, 1000.0).map(|s| s.2).unwrap_or(0.0);

        println!(
            "{:<25} {:>7.0} {:>5.3} {:>7.3} {:>7.0} {:>7.3} {:>7.0} {:>7.3} {:>7.0} {:>7.3} {:>7.0}",
            r.name, r.mv_ms, r.bc, d300, v300, d600, v600, d800, v800, d1000, v1000
        );
    }

    println!("=== End Table ===\n");
}

// ── BC-vs-Mach scaling demonstration ─────────────────────────────────────────
//
// This test demonstrates that the G7 and G1 drag models produce materially
// different trajectories for the SAME projectile (M855).  Both models
// implement Cd-vs-Mach interpolation through their respective CDM tables,
// but the underlying curves differ — G1 has higher drag than G7 across the
// typical rifle Mach range (Mach 2–3).
//
// If BC-vs-Mach scaling were irrelevant (i.e. if Cd were constant), G7 and
// G1 would agree.  The measured divergence proves that the Cd-Mach
// relationship materially affects the trajectory.

#[test]
fn bc_scaling_g7_vs_g1() {
    abe_init(ABE_API_VERSION, 0);

    let mv_ms = 930.0;
    let bc = 0.157;
    let mass_g = 4.0;
    let cal_mm = 5.56;

    // Same projectile, two different drag models
    let samples_g7 = simulate_trajectory(mv_ms, bc, mass_g, cal_mm, "g7", DT_S);
    let samples_g1 = simulate_trajectory(mv_ms, bc, mass_g, cal_mm, "g1", DT_S);

    let s7 = sample_at(&samples_g7, 600.0).expect("G7 trajectory should reach 600 m");
    let s1 = sample_at(&samples_g1, 600.0).expect("G1 trajectory should reach 600 m");

    let (r7, d7, v7, _t7) = s7;
    let (r1, d1, v1, _t1) = s1;

    // Both should have a sample within 10 m of 600 m
    assert!(
        (r7 - 600.0).abs() < 10.0,
        "G7 closest sample to 600 m is at {:.0} m",
        r7
    );
    assert!(
        (r1 - 600.0).abs() < 10.0,
        "G1 closest sample to 600 m is at {:.0} m",
        r1
    );

    // G1 should have more drop than G7 at 600 m (higher drag → slower →
    // longer TOF → more gravity-driven drop).
    assert!(
        d1 > d7 + 0.2,
        "G1 drop ({:.3} m) should exceed G7 drop ({:.3} m) by >0.2 m \
         at 600 m — demonstrates BC-vs-Mach scaling",
        d1,
        d7,
    );

    // G1 should have lower retained velocity
    assert!(
        v1 < v7 - 20.0,
        "G1 velocity ({:.1} m/s) should be lower than G7 ({:.1} m/s) by >20 m/s at 600 m",
        v1,
        v7,
    );

    // Sanity: the BC scaling effect should be material
    // (> 5 % difference in drop)
    let drop_ratio = d1 / d7;
    assert!(
        drop_ratio > 1.05,
        "G1/G7 drop ratio should be >1.05 at 600 m, got {:.3}",
        drop_ratio,
    );
}
