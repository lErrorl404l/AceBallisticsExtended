//! Print trajectory tables for all calibrated ABE ammunition types.
//!
//! Run: cargo run --example trajectory_table

use abe_ballistics_ext::{abe_init, abe_step, BulletState, StepParams};

const ABE_API_VERSION: u32 = 1;
const DT_S: f64 = 0.01;

const SAMPLE_RANGES: [f64; 11] = [
    0.0, 100.0, 200.0, 300.0, 400.0, 500.0, 600.0, 700.0, 800.0, 900.0, 1000.0,
];

fn simulate_trajectory(
    mv_ms: f64,
    bc: f64,
    mass_g: f64,
    caliber_mm: f64,
    cdm: &str,
) -> Vec<(f64, f64, f64, f64)> {
    let (mut x, mut y, mut z) = (0.0, 0.0, 0.0);
    let (mut vx, mut vy, mut vz) = (mv_ms, 0.0, 0.0);
    let mut t = 0.0;
    let mut cdm_buf = [0u8; 32];
    let bytes = cdm.as_bytes();
    cdm_buf[..bytes.len().min(31)].copy_from_slice(&bytes[..bytes.len().min(31)]);

    let mut samples = Vec::new();
    samples.push((x, z, mv_ms, t));
    let mut next_idx = 1;

    while x < 1050.0 && vx > 50.0 && next_idx < SAMPLE_RANGES.len() {
        let step = StepParams {
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
        let ret = abe_step(&step, &mut result);
        assert_eq!(ret, 0);
        x = result.pos_x;
        y = result.pos_y;
        z = result.pos_z;
        vx = result.vel_x;
        vy = result.vel_y;
        vz = result.vel_z;
        t += DT_S;
        let speed = (vx * vx + vy * vy + vz * vz).sqrt();
        while next_idx < SAMPLE_RANGES.len() && x >= SAMPLE_RANGES[next_idx] {
            samples.push((x, z, speed, t));
            next_idx += 1;
        }
    }
    samples
}

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

struct Round {
    name: &'static str,
    mv_ms: f64,
    bc: f64,
    mass_g: f64,
    cal_mm: f64,
    cdm: &'static str,
}

fn main() {
    abe_init(ABE_API_VERSION, 0);

    let rounds = [
        Round {
            name: "M193 5.56mm (20\")",
            mv_ms: 993.0,
            bc: 0.126,
            mass_g: 3.56,
            cal_mm: 5.56,
            cdm: "g7",
        },
        Round {
            name: "M855 5.56mm (20\")",
            mv_ms: 948.0,
            bc: 0.151,
            mass_g: 4.0,
            cal_mm: 5.56,
            cdm: "g7",
        },
        Round {
            name: "M855A1 5.56mm (20\")",
            mv_ms: 961.0,
            bc: 0.152,
            mass_g: 4.02,
            cal_mm: 5.69,
            cdm: "g7",
        },
        Round {
            name: "Mk262 5.56mm (16\")",
            mv_ms: 870.0,
            bc: 0.197,
            mass_g: 5.0,
            cal_mm: 5.56,
            cdm: "g7",
        },
        Round {
            name: "7N6 5.45mm (16.3\")",
            mv_ms: 880.0,
            bc: 0.168,
            mass_g: 3.43,
            cal_mm: 5.45,
            cdm: "g7",
        },
        Round {
            name: "7N22 5.45mm AP",
            mv_ms: 880.0,
            bc: 0.174,
            mass_g: 3.69,
            cal_mm: 5.45,
            cdm: "g7",
        },
        Round {
            name: "M43 7.62×39 (16.3\")",
            mv_ms: 715.0,
            bc: 0.148,
            mass_g: 7.97,
            cal_mm: 7.62,
            cdm: "g7",
        },
        Round {
            name: "M80 7.62mm (22\")",
            mv_ms: 853.0,
            bc: 0.200,
            mass_g: 9.5,
            cal_mm: 7.62,
            cdm: "g7",
        },
        Round {
            name: "M118LR 7.62mm (24\")",
            mv_ms: 780.0,
            bc: 0.243,
            mass_g: 11.34,
            cal_mm: 7.62,
            cdm: "g7",
        },
        Round {
            name: "M61 AP 7.62mm (22\")",
            mv_ms: 830.0,
            bc: 0.218,
            mass_g: 10.0,
            cal_mm: 7.62,
            cdm: "g7",
        },
        Round {
            name: "LPS 7.62×54R (PKM)",
            mv_ms: 825.0,
            bc: 0.210,
            mass_g: 9.6,
            cal_mm: 7.62,
            cdm: "g7",
        },
        Round {
            name: "7N1 7.62×54R (SVD)",
            mv_ms: 823.0,
            bc: 0.216,
            mass_g: 9.85,
            cal_mm: 7.62,
            cdm: "g7",
        },
        Round {
            name: ".277 Fury (16\")",
            mv_ms: 915.0,
            bc: 0.206,
            mass_g: 8.75,
            cal_mm: 7.04,
            cdm: "g7",
        },
        Round {
            name: ".338 Lapua (26\")",
            mv_ms: 880.0,
            bc: 0.310,
            mass_g: 16.2,
            cal_mm: 8.6,
            cdm: "g7",
        },
        Round {
            name: ".408 CheyTac (27\")",
            mv_ms: 830.0,
            bc: 0.420,
            mass_g: 27.0,
            cal_mm: 10.36,
            cdm: "g7",
        },
        Round {
            name: ".50 BMG M33 (29\")",
            mv_ms: 860.0,
            bc: 0.340,
            mass_g: 42.0,
            cal_mm: 12.7,
            cdm: "g7",
        },
        Round {
            name: ".300 BLK Sub (16\")",
            mv_ms: 305.0,
            bc: 0.313,
            mass_g: 14.3,
            cal_mm: 7.82,
            cdm: "g7",
        },
        Round {
            name: ".300 BLK Sup (16\")",
            mv_ms: 610.0,
            bc: 0.139,
            mass_g: 7.1,
            cal_mm: 7.82,
            cdm: "g7",
        },
        Round {
            name: "9mm 124gr FMJ (4.7\")",
            mv_ms: 370.0,
            bc: 0.152,
            mass_g: 8.0,
            cal_mm: 9.0,
            cdm: "g1",
        },
        Round {
            name: ".45 ACP 230gr (5\")",
            mv_ms: 280.0,
            bc: 0.173,
            mass_g: 15.0,
            cal_mm: 11.43,
            cdm: "g1",
        },
    ];

    println!("=== ABE Trajectory Validation Table ===");
    println!("ICAO std atmosphere, 100m zero, G7/G1 drag model per round\n");
    println!(
        "{:<24} {:>6} {:>5} {:>7} {:>6} {:>7} {:>6} {:>7} {:>6} {:>7} {:>6}",
        "Round", "MV", "BC", "D300", "V300", "D600", "V600", "D800", "V800", "D1000", "V1000"
    );
    println!("{:-<94}", "");

    for r in &rounds {
        let s = simulate_trajectory(r.mv_ms, r.bc, r.mass_g, r.cal_mm, r.cdm);
        let f = |rm: f64| sample_at(&s, rm).unwrap_or((rm, -1.0, 0.0, 0.0));
        let (_, d3, v3, _) = f(300.0);
        let (_, d6, v6, _) = f(600.0);
        let (_, d8, v8, _) = f(800.0);
        let (_, d10, v10, _) = f(1000.0);
        println!("{:<24} {:>6.0} {:>5.3} {:>7.3} {:>6.0} {:>7.3} {:>6.0} {:>7.3} {:>6.0} {:>7.3} {:>6.0}",
            r.name, r.mv_ms, r.bc, d3, v3, d6, v6, d8, v8, d10, v10);
    }

    println!("=== End ===\n");
}
