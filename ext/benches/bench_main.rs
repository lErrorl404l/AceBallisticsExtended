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

// ── New benchmark groups ────────────────────────────────────────────────────

fn bench_init_health(c: &mut Criterion) {
    // Ensure clean state for bench
    let _ = abe_init(1, 0);

    c.bench_function("init/health_check", |b| {
        b.iter(|| {
            // get_state().initialized — the OnceLock::get check on every ABI call
            black_box(abe_health())
        })
    });

    c.bench_function("init/version_string", |b| {
        b.iter(|| {
            // OnceLock<CString> — cached on first call, then just .get()
            black_box(abe_version())
        })
    });

    c.bench_function("init/full_init", |b| {
        // Re-init: OnceLock::set returns Err on second call, handle guards return "0"
        b.iter(|| black_box(abe_init(1, 0)))
    });
}

fn bench_multi_bullet_100(c: &mut Criterion) {
    abe_init(1, 0);

    let cdm = pack_id("g7");
    const NUM_BULLETS: usize = 100;
    const STEPS_EACH: usize = 30;

    c.bench_function("multi_bullet/100x30_interleaved", |b| {
        b.iter(|| {
            let mut states: Vec<(f64, f64, f64, f64, f64, f64, f64)> =
                Vec::with_capacity(NUM_BULLETS);
            for i in 0..NUM_BULLETS {
                let mv = 930.0 - i as f64 * 5.0; // spread: 930→430 m/s
                let bc = 0.200 - i as f64 * 0.0015; // spread: 0.200→0.050
                states.push((0.0, 0.0, 0.0, mv, 0.0, 0.0, bc));
            }

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

fn bench_full_pipeline_realistic(c: &mut Criterion) {
    abe_init(1, 0);

    let cdm = pack_id("g7");
    let mat = pack_id("steel_rha");
    let proj = pack_id("ap");

    c.bench_function("pipeline/fire_500step_impact_realistic", |b| {
        b.iter(|| {
            // ── Fire: 7.62mm NATO M80 from M240 (24.8" barrel) ──────
            let fire = FireParams {
                barrel_length_mm: 630.0,
                chamber_pressure_mpa: 360.0,
                caliber_mm: 7.62,
                projectile_mass_g: 9.5,
                cdm_id: cdm,
            };
            let mut fr = FireResult::default();
            abe_fire(&fire, &mut fr);
            let mv = fr.muzzle_velocity_ms;

            // ── 500 steps @ dt=0.01 with crosswind at altitude ──────
            let mut x = 0.0;
            let mut y = 0.0;
            let mut z = 0.0;
            let mut vx = mv;
            let mut vy = 0.0;
            let mut vz = 0.0;

            for _ in 0..500 {
                let step = StepParams {
                    pos_x: x,
                    pos_y: y,
                    pos_z: z,
                    vel_x: vx,
                    vel_y: vy,
                    vel_z: vz,
                    dt_s: 0.01,
                    wind_x: 5.0,
                    wind_y: 3.0,
                    wind_z: 0.0, // crosswind
                    density_kgm3: 1.225,
                    temp_c: 15.0,
                    altitude_m: 500.0, // 500m ASL → ICAO density
                    cdm_id: cdm,
                    bc: 0.200, // G7 BC for M80 ball
                    mass_g: 9.5,
                    caliber_mm: 7.62,
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

            // ── Impact: AP vs 10mm RHA at 30° ──────────────────────
            let impact = ImpactParams {
                vel_x: vx,
                vel_y: vy,
                vel_z: vz,
                mass_g: 9.5,
                caliber_mm: 7.62,
                armor_thickness_mm: 10.0,
                armor_material: mat,
                impact_angle_deg: 30.0,
                projectile_type: proj,
            };
            let mut ir = ImpactResult::default();
            abe_impact(&impact, &mut ir);
            black_box(ir)
        })
    });
}

fn bench_impact_variants(c: &mut Criterion) {
    abe_init(1, 0);

    let steel = pack_id("steel_rha");
    let alum = pack_id("aluminum_5083");
    let ceramic = pack_id("ceramic_b4c");
    let ball = pack_id("ball");
    let ap = pack_id("ap");
    let apds = pack_id("apds");

    c.bench_function("impact/ap_vs_10mm_rha", |b| {
        let params = ImpactParams {
            vel_x: 880.0,
            vel_y: 0.0,
            vel_z: 0.0,
            mass_g: 9.5,
            caliber_mm: 7.62,
            armor_thickness_mm: 10.0,
            armor_material: steel,
            impact_angle_deg: 0.0,
            projectile_type: ap,
        };
        b.iter(|| {
            let mut result = ImpactResult::default();
            abe_impact(black_box(&params), &mut result);
            black_box(result)
        })
    });

    c.bench_function("impact/ball_vs_aluminum_thin", |b| {
        let params = ImpactParams {
            vel_x: 650.0,
            vel_y: 0.0,
            vel_z: 0.0,
            mass_g: 4.0,
            caliber_mm: 5.56,
            armor_thickness_mm: 3.0,
            armor_material: alum,
            impact_angle_deg: 0.0,
            projectile_type: ball,
        };
        b.iter(|| {
            let mut result = ImpactResult::default();
            abe_impact(black_box(&params), &mut result);
            black_box(result)
        })
    });

    c.bench_function("impact/ricochet_at_80deg", |b| {
        let params = ImpactParams {
            vel_x: 750.0,
            vel_y: 0.0,
            vel_z: 0.0,
            mass_g: 9.5,
            caliber_mm: 7.62,
            armor_thickness_mm: 5.0,
            armor_material: steel,
            impact_angle_deg: 80.0,
            projectile_type: ball,
        };
        b.iter(|| {
            let mut result = ImpactResult::default();
            abe_impact(black_box(&params), &mut result);
            black_box(result)
        })
    });

    c.bench_function("impact/apds_vs_ceramic", |b| {
        let params = ImpactParams {
            vel_x: 1500.0,
            vel_y: 0.0,
            vel_z: 0.0,
            mass_g: 4.5,
            caliber_mm: 5.56,
            armor_thickness_mm: 30.0,
            armor_material: ceramic,
            impact_angle_deg: 15.0,
            projectile_type: apds,
        };
        b.iter(|| {
            let mut result = ImpactResult::default();
            abe_impact(black_box(&params), &mut result);
            black_box(result)
        })
    });
}

fn bench_fire_variants(c: &mut Criterion) {
    abe_init(1, 0);

    let cdm = pack_id("g7");

    c.bench_function("fire/5.56mm_m855", |b| {
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

    c.bench_function("fire/7.62mm_m80", |b| {
        let params = FireParams {
            barrel_length_mm: 630.0,
            chamber_pressure_mpa: 360.0,
            caliber_mm: 7.62,
            projectile_mass_g: 9.5,
            cdm_id: cdm,
        };
        b.iter(|| {
            let mut result = FireResult::default();
            abe_fire(black_box(&params), &mut result);
            black_box(result)
        })
    });

    c.bench_function("fire/9mm_parabellum", |b| {
        let params = FireParams {
            barrel_length_mm: 127.0,
            chamber_pressure_mpa: 240.0,
            caliber_mm: 9.01,
            projectile_mass_g: 8.0,
            cdm_id: cdm,
        };
        b.iter(|| {
            let mut result = FireResult::default();
            abe_fire(black_box(&params), &mut result);
            black_box(result)
        })
    });

    c.bench_function("fire/338_lapua", |b| {
        let params = FireParams {
            barrel_length_mm: 690.0,
            chamber_pressure_mpa: 420.0,
            caliber_mm: 8.58,
            projectile_mass_g: 16.2,
            cdm_id: cdm,
        };
        b.iter(|| {
            let mut result = FireResult::default();
            abe_fire(black_box(&params), &mut result);
            black_box(result)
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
    bench_init_health,
    bench_multi_bullet_100,
    bench_full_pipeline_realistic,
    bench_impact_variants,
    bench_fire_variants,
);
criterion_main!(benches);
