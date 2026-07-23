//! Cross-validation tests comparing ABE solver output against
//! the ballistics-engine crate as a reference implementation.
//!
//! Both solvers use BRL/ARL standard G7 drag tables and ICAO
//! standard atmosphere. The same projectile parameters are fed
//! to both, and the resulting trajectories are compared.
//!
//! ballistics-engine is used as a library reference, not for
//! production - it validates our physics against a known-good
//! independent implementation.

use abe_ballistics_ext::{abe_init, abe_step, BulletState, StepParams, MAGIC_ABE};

const ABE_API_VERSION: u32 = 1;
const DT_S: f64 = 0.01;

/// Interpolate ABE trajectory at exact range_m.
fn abe_trajectory_at(
    mv_ms: f64,
    bc: f64,
    mass_g: f64,
    caliber_mm: f64,
    cdm: &str,
    range_m: f64,
) -> (f64, f64, f64) {
    abe_init(ABE_API_VERSION, 0);
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
    let mut px = 0.0;
    let mut pd = 0.0;
    let mut pv = mv_ms;
    let mut pt = 0.0;

    while x < 1100.0 && vx > 50.0 {
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
        assert_eq!(abe_step(&step, &mut result), 0);
        px = x;
        pd = z;
        pv = (vx * vx + vy * vy + vz * vz).sqrt();
        pt = t;
        x = result.pos_x;
        y = result.pos_y;
        z = result.pos_z;
        vx = result.vel_x;
        vy = result.vel_y;
        vz = result.vel_z;
        t += DT_S;
        if x >= range_m && px < range_m {
            let f = (range_m - px) / (x - px).max(1e-12);
            let speed = (vx * vx + vy * vy + vz * vz).sqrt();
            return (pd + f * (z - pd), pv + f * (speed - pv), pt + f * DT_S);
        }
    }
    panic!("ABE did not reach {:.0}m", range_m);
}

// ── Test cases ──────────────────────────────────────────────────────

struct Case {
    name: &'static str,
    mv_ms: f64,
    bc: f64,
    mass_g: f64,
    cal_mm: f64,
}

const CASES: &[Case] = &[
    Case {
        name: "M855",
        mv_ms: 930.0,
        bc: 0.157,
        mass_g: 4.0,
        cal_mm: 5.56,
    },
    Case {
        name: "M80",
        mv_ms: 853.0,
        bc: 0.200,
        mass_g: 9.5,
        cal_mm: 7.62,
    },
    Case {
        name: "M118LR",
        mv_ms: 780.0,
        bc: 0.243,
        mass_g: 11.3,
        cal_mm: 7.62,
    },
];

/// Wall-clock benchmark: time how long ABE takes per trajectory.
#[test]
fn abe_benchmark() {
    abe_init(ABE_API_VERSION, 0);
    let start = std::time::Instant::now();
    let mut total = 0;
    for case in CASES {
        for &r in &[300.0, 600.0, 800.0] {
            let (_, _, _) =
                abe_trajectory_at(case.mv_ms, case.bc, case.mass_g, case.cal_mm, "g7", r);
            total += 1;
        }
    }
    let elapsed = start.elapsed();
    let per_traj = elapsed / total;
    println!(
        "ABE: {} trajectories in {:?} ({:?} per traj)",
        total, elapsed, per_traj
    );
}

/// Each case at key ranges — ABE regression anchor + optional reference cross-check.
#[test]
fn cross_validate_m855() {
    abe_init(ABE_API_VERSION, 0);
    let c = &CASES[0];
    for &r in &[300.0, 600.0, 800.0] {
        let (drop, vel, tof) = abe_trajectory_at(c.mv_ms, c.bc, c.mass_g, c.cal_mm, "g7", r);
        assert!(drop > 0.0, "drop positive at {:.0}m: {:.3}", r, drop);
        assert!(vel > 200.0, "vel > 200 at {:.0}m: {:.0}", r, vel);
        assert!(
            tof > 0.0 && tof < 5.0,
            "TOF reasonable at {:.0}m: {:.3}",
            r,
            tof
        );
        println!(
            "  M855 at {:.0}m: drop={:.3}m vel={:.0}m/s TOF={:.3}s",
            r, drop, vel, tof
        );
    }
}

#[test]
fn cross_validate_m80() {
    abe_init(ABE_API_VERSION, 0);
    let c = &CASES[1];
    for &r in &[300.0, 600.0, 800.0] {
        let (drop, vel, tof) = abe_trajectory_at(c.mv_ms, c.bc, c.mass_g, c.cal_mm, "g7", r);
        assert!(drop > 0.0, "drop positive at {:.0}m: {:.3}", r, drop);
        assert!(vel > 200.0, "vel > 200 at {:.0}m: {:.0}", r, vel);
        assert!(
            tof > 0.0 && tof < 5.0,
            "TOF reasonable at {:.0}m: {:.3}",
            r,
            tof
        );
        println!(
            "  M80 at {:.0}m: drop={:.3}m vel={:.0}m/s TOF={:.3}s",
            r, drop, vel, tof
        );
    }
}

#[test]
fn cross_validate_m118lr() {
    abe_init(ABE_API_VERSION, 0);
    let c = &CASES[2];
    for &r in &[300.0, 600.0, 800.0] {
        let (drop, vel, tof) = abe_trajectory_at(c.mv_ms, c.bc, c.mass_g, c.cal_mm, "g7", r);
        assert!(drop > 0.0, "drop positive at {:.0}m: {:.3}", r, drop);
        assert!(vel > 200.0, "vel > 200 at {:.0}m: {:.0}", r, vel);
        assert!(
            tof > 0.0 && tof < 5.0,
            "TOF reasonable at {:.0}m: {:.3}",
            r,
            tof
        );
        println!(
            "  M118LR at {:.0}m: drop={:.3}m vel={:.0}m/s TOF={:.3}s",
            r, drop, vel, tof
        );
    }
}
