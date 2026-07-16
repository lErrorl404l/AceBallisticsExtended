// ABE - Performance Benchmarks
//
// Measures throughput (iter/s) for the core C ABI functions and
// the string-based RVExtensionArgs dispatch.
//
// Run:  cargo bench
// Quick: cargo bench -- --quick --warm-up-time 1 --measurement-time 2

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::ffi::CString;
use std::os::raw::c_char;

use abe_ballistics_ext::*;

// ── Helpers ─────────────────────────────────────────────────────────────────

const OUTPUT_BUF_SIZE: usize = 2048;

/// Pack a &str into a 32-byte fixed-size array (CDM ID / material / proj type).
fn pack_id(s: &str) -> [u8; 32] {
    let mut buf = [0u8; 32];
    let bytes = s.as_bytes();
    let len = bytes.len().min(31);
    buf[..len].copy_from_slice(&bytes[..len]);
    buf
}

/// String-mode ABI helper — replicates the test harness pattern.
fn rv_ext_args(func: &str, args: &[&str]) -> String {
    let mut buf = vec![0u8; OUTPUT_BUF_SIZE];
    let cfunc = CString::new(func).unwrap();
    let c_args: Vec<CString> = args.iter().map(|a| CString::new(*a).unwrap()).collect();
    let ptrs: Vec<*const c_char> = c_args.iter().map(|a| a.as_ptr()).collect();
    unsafe {
        RVExtensionArgs(
            buf.as_mut_ptr() as *mut c_char,
            OUTPUT_BUF_SIZE as i32,
            cfunc.as_ptr(),
            ptrs.as_ptr(),
            args.len() as i32,
        );
    }
    let end = buf.iter().position(|&b| b == 0).unwrap_or(0);
    std::str::from_utf8(&buf[..end]).unwrap().to_string()
}

// ── Benchmarks ──────────────────────────────────────────────────────────────

fn bench_fire(c: &mut Criterion) {
    abe_init(1, 0);

    let cdm = pack_id("g7");

    c.bench_function("fire/struct_abi", |b| {
        let params = FireParams {
            barrel_length_mm: 368.0,
            chamber_pressure_mpa: 380.0,
            caliber_mm: 5.56,
            projectile_mass_g: 4.0,
            cdm_id: cdm,
        };
        b.iter(|| {
            let mut result = FireResult::default();
            abe_fire(black_box(&params), &mut result);
            black_box(result)
        })
    });
}

fn bench_step(c: &mut Criterion) {
    abe_init(1, 0);

    let cdm = pack_id("g7");

    c.bench_function("step/struct_abi", |b| {
        let params = StepParams {
            pos_x: 500.0,
            pos_y: 0.0,
            pos_z: -2.0,
            vel_x: 800.0,
            vel_y: 0.0,
            vel_z: 0.0,
            dt_s: 0.01,
            wind_x: 0.0,
            wind_y: 0.0,
            wind_z: 0.0,
            density_kgm3: 1.225,
            temp_c: 15.0,
            altitude_m: 0.0,
            cdm_id: cdm,
            bc: 0.157,
            mass_g: 4.0,
            caliber_mm: 5.56,
        };
        b.iter(|| {
            let mut result = BulletState::default();
            abe_step(black_box(&params), &mut result);
            black_box(result)
        })
    });
}

fn bench_impact(c: &mut Criterion) {
    abe_init(1, 0);

    let mat = pack_id("steel_rha");
    let proj = pack_id("ball");

    c.bench_function("impact/struct_abi", |b| {
        let params = ImpactParams {
            vel_x: 850.0,
            vel_y: 0.0,
            vel_z: 0.0,
            mass_g: 9.5,
            caliber_mm: 7.62,
            armor_thickness_mm: 10.0,
            armor_material: mat,
            impact_angle_deg: 0.0,
            projectile_type: proj,
        };
        b.iter(|| {
            let mut result = ImpactResult::default();
            abe_impact(black_box(&params), &mut result);
            black_box(result)
        })
    });
}

fn bench_full_pipeline(c: &mut Criterion) {
    abe_init(1, 0);

    let cdm = pack_id("g7");
    let mat = pack_id("steel_rha");
    let proj = pack_id("ball");

    c.bench_function("pipeline/fire_500step_impact", |b| {
        b.iter(|| {
            // ── Fire ─────────────────────────────────────────────────
            let fire = FireParams {
                barrel_length_mm: 368.0,
                chamber_pressure_mpa: 380.0,
                caliber_mm: 5.56,
                projectile_mass_g: 4.0,
                cdm_id: cdm,
            };
            let mut fr = FireResult::default();
            abe_fire(&fire, &mut fr);
            let mv = fr.muzzle_velocity_ms;

            // ── 500 steps @ dt=0.01 ─────────────────────────────────
            let (mut x, mut y, mut z, mut vx, mut vy, mut vz) = (0.0, 0.0, 0.0, mv, 0.0, 0.0);

            for _ in 0..500 {
                let step = StepParams {
                    pos_x: x,
                    pos_y: y,
                    pos_z: z,
                    vel_x: vx,
                    vel_y: vy,
                    vel_z: vz,
                    dt_s: 0.01,
                    wind_x: 0.0,
                    wind_y: 0.0,
                    wind_z: 0.0,
                    density_kgm3: 1.225,
                    temp_c: 15.0,
                    altitude_m: 0.0,
                    cdm_id: cdm,
                    bc: 0.157,
                    mass_g: 4.0,
                    caliber_mm: 5.56,
                };
                let mut sr = BulletState::default();
                abe_step(&step, &mut sr);
                x = sr.pos_x;
                y = sr.pos_y;
                z = sr.pos_z;
                vx = sr.vel_x;
                vy = sr.vel_y;
                vz = sr.vel_z;
            }

            // ── Impact ──────────────────────────────────────────────
            let impact = ImpactParams {
                vel_x: vx,
                vel_y: vy,
                vel_z: vz,
                mass_g: 4.0,
                caliber_mm: 5.56,
                armor_thickness_mm: 3.0,
                armor_material: mat,
                impact_angle_deg: 0.0,
                projectile_type: proj,
            };
            let mut ir = ImpactResult::default();
            abe_impact(&impact, &mut ir);
            black_box(ir)
        })
    });
}

fn bench_step_string_abi(c: &mut Criterion) {
    abe_init(1, 0);

    let args: [&str; 17] = [
        "500", "0", "-2", // pos_x, pos_y, pos_z
        "800", "0", "0",    // vel_x, vel_y, vel_z
        "0.01", // dt
        "0", "0", "0",     // wind
        "1.225", // density
        "15",    // temp_c
        "0",     // altitude
        "g7",    // cdm
        "0.157", // bc
        "4.0",   // mass_g
        "5.56",  // caliber_mm
    ];

    c.bench_function("step/string_abi", |b| {
        b.iter(|| black_box(rv_ext_args("step", &args)))
    });
}

fn bench_multi_bullet(c: &mut Criterion) {
    abe_init(1, 0);

    let cdm = pack_id("g7");
    const NUM_BULLETS: usize = 10;
    const STEPS_EACH: usize = 30;

    c.bench_function("multi_bullet/10x30_interleaved", |b| {
        b.iter(|| {
            // ── Initialize 10 bullet states ──────────────────────────
            let mut states: Vec<(f64, f64, f64, f64, f64, f64, f64)> =
                Vec::with_capacity(NUM_BULLETS);
            for i in 0..NUM_BULLETS {
                let mv = 930.0 - i as f64 * 60.0;
                let bc = 0.157 - i as f64 * 0.008;
                states.push((0.0, 0.0, 0.0, mv, 0.0, 0.0, bc));
            }

            // ── Interleaved stepping ─────────────────────────────────
            for _step_idx in 0..STEPS_EACH {
                for s in states.iter_mut() {
                    let (x, y, z, vx, vy, vz, bc) = *s;
                    let step = StepParams {
                        pos_x: x,
                        pos_y: y,
                        pos_z: z,
                        vel_x: vx,
                        vel_y: vy,
                        vel_z: vz,
                        dt_s: 0.01,
                        wind_x: 0.0,
                        wind_y: 0.0,
                        wind_z: 0.0,
                        density_kgm3: 1.225,
                        temp_c: 15.0,
                        altitude_m: 0.0,
                        cdm_id: cdm,
                        bc,
                        mass_g: 4.0,
                        caliber_mm: 5.56,
                    };
                    let mut sr = BulletState::default();
                    abe_step(&step, &mut sr);
                    *s = (
                        sr.pos_x, sr.pos_y, sr.pos_z, sr.vel_x, sr.vel_y, sr.vel_z, bc,
                    );
                }
            }

            black_box(states.len())
        })
    });
}

// ── Criterion harness ───────────────────────────────────────────────────────

criterion_group!(
    benches,
    bench_fire,
    bench_step,
    bench_impact,
    bench_full_pipeline,
    bench_step_string_abi,
    bench_multi_bullet,
);
criterion_main!(benches);
