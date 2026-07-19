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
            twist_rate_m: 0.0,
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
            yaw_angle_deg: 0.0,
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
                    twist_rate_m: 0.0,
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
                yaw_angle_deg: 0.0,
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
                        twist_rate_m: 0.0,
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
                        twist_rate_m: 0.0,
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
                    twist_rate_m: 0.0,
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
                yaw_angle_deg: 0.0,
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
            yaw_angle_deg: 0.0,
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
            yaw_angle_deg: 0.0,
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
            yaw_angle_deg: 0.0,
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
            yaw_angle_deg: 0.0,
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

// ── New module benchmarks ─────────────────────────────────────────────────────

use abe_ballistics_ext::combined_effects::{CombinedAmmoParams, CombinedType};
use abe_ballistics_ext::component_damage::{
    AmmoStoredType, ComponentConfig, ComponentFireState, ComponentType, CrewProtection, EngineType,
    FireStatus, FireSuppression, FuelType,
};
use abe_ballistics_ext::component_kill_prob::{ComponentKillParams, HitZone, VehicleType};
use abe_ballistics_ext::frangible_ammo::{FrangibleAmmoParams, FrangibleType};
use abe_ballistics_ext::lot_variation::{AmmoGrade, LotVariationParams};
use abe_ballistics_ext::predictive_era::{PredictiveERAParams, PredictiveERAType};
use abe_ballistics_ext::sequential_hits::{HitRecord, SequentialHitParams};
use abe_ballistics_ext::shooter_error::{
    BreathPhase, ExperienceLevel, ShooterParams, ShooterStance, SupportType,
};
use abe_ballistics_ext::tire_penetration::{ImpactZone, TirePenetrationParams, TireType};

fn bench_sight_height(c: &mut Criterion) {
    c.bench_function("sight_height/zero_angle_linear", |b| {
        b.iter(|| {
            black_box(sight_height::zero_angle_linear(
                black_box(0.05),
                black_box(100.0),
                black_box(900.0),
            ))
        })
    });
}

fn bench_shooter_error(c: &mut Criterion) {
    let shooter = ShooterParams {
        stance: ShooterStance::Prone,
        support: SupportType::Bipod,
        heart_rate_bpm: 80.0,
        breathing: BreathPhase::Hold,
        fatigue_fraction: 0.1,
        experience: ExperienceLevel::Expert,
        base_shooter_moa: 1.0,
    };
    c.bench_function("shooter_error/total_system_moa", |b| {
        b.iter(|| {
            black_box(shooter_error::total_system_moa(
                black_box(1.5),
                black_box(&shooter),
            ))
        })
    });
}

fn bench_material_factor(c: &mut Criterion) {
    let materials = [
        "steel_rha",
        "aluminum_5083",
        "ceramic_b4c",
        "concrete",
        "glass",
    ];
    c.bench_function("penetration/material_factor_5", |b| {
        b.iter(|| {
            let mut sum = 0.0f64;
            for m in &materials {
                sum += black_box(penetration::material_factor(black_box(m)));
            }
            black_box(sum)
        })
    });
}

fn bench_spin_drift(c: &mut Criterion) {
    c.bench_function("exterior/spin_drift", |b| {
        b.iter(|| {
            black_box(exterior::spin_drift(
                black_box(12.0),
                black_box(900.0),
                black_box(0.5),
                black_box(450.0),
            ))
        })
    });
}

fn bench_get_cd(c: &mut Criterion) {
    let machs = [0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0, 2.25, 2.5, 3.0];
    c.bench_function("drag/get_cd_g7_10mach", |b| {
        b.iter(|| {
            let mut sum = 0.0f64;
            for &m in &machs {
                sum += black_box(drag::get_cd(black_box("g7"), black_box(m)));
            }
            black_box(sum)
        })
    });
}

fn bench_penetration_multilayer(c: &mut Criterion) {
    c.bench_function("penetration/evaluate_3plate", |b| {
        b.iter(|| {
            let mut v = 850.0f64;
            let mass_kg = 0.0095;
            let cal_m = 0.00762;
            let plates = [
                ("steel_rha", 0.010),
                ("aluminum_5083", 0.020),
                ("steel_rha", 0.005),
            ];
            for (mat, thick) in plates {
                let r = penetration::evaluate(
                    black_box(v),
                    black_box(mass_kg),
                    black_box(cal_m),
                    black_box(thick),
                    black_box(0.0),
                    black_box(mat),
                    black_box("ap"),
                    None,
                );
                v = r.residual_velocity;
            }
            black_box(v)
        })
    });
}

fn bench_interior_wall(c: &mut Criterion) {
    use abe_ballistics_ext::interior_wall::{evaluate_interior_wall, standard_stud_wall};
    let layers = standard_stud_wall();
    c.bench_function("interior_wall/standard_stud", |b| {
        b.iter(|| {
            black_box(evaluate_interior_wall(
                black_box(&layers),
                black_box(800.0),
                black_box(0.0095),
                black_box(0.00762),
                black_box(0.0),
                black_box("ball"),
            ))
        })
    });
}

fn bench_armor_array(c: &mut Criterion) {
    use abe_ballistics_ext::armor_array::{evaluate_armor_array, ArrayPlate};
    let plates = [
        ArrayPlate {
            thickness_m: 0.010,
            material: "steel_rha",
            angle_from_vertical_deg: 0.0,
            gap_to_next_m: 0.050,
            open_area_fraction: 0.0,
        },
        ArrayPlate {
            thickness_m: 0.005,
            material: "ceramic_b4c",
            angle_from_vertical_deg: 10.0,
            gap_to_next_m: 0.030,
            open_area_fraction: 0.0,
        },
        ArrayPlate {
            thickness_m: 0.008,
            material: "steel_rha",
            angle_from_vertical_deg: 0.0,
            gap_to_next_m: 0.020,
            open_area_fraction: 0.0,
        },
        ArrayPlate {
            thickness_m: 0.004,
            material: "aluminum_5083",
            angle_from_vertical_deg: 5.0,
            gap_to_next_m: 0.0,
            open_area_fraction: 0.0,
        },
    ];
    c.bench_function("armor_array/4plate_spaced", |b| {
        b.iter(|| {
            black_box(evaluate_armor_array(
                black_box(&plates),
                black_box(1200.0),
                black_box(0.0045),
                black_box(0.00556),
                black_box("apds"),
            ))
        })
    });
}

fn bench_frangible(c: &mut Criterion) {
    let params = FrangibleAmmoParams {
        frangible_type: FrangibleType::LeadFrangible,
        impact_velocity_ms: 900.0,
        mass_g: 4.0,
        caliber_mm: 5.56,
    };
    c.bench_function("frangible/impact_steel", |b| {
        b.iter(|| {
            black_box(frangible_ammo::evaluate_frangible_impact(
                black_box(&params),
                black_box("steel_rha"),
            ))
        })
    });
}

fn bench_tire_penetration(c: &mut Criterion) {
    let params = TirePenetrationParams {
        tire_type: TireType::Highway,
        impact_zone: ImpactZone::Tread,
        projectile_caliber_mm: 7.62,
        projectile_mass_g: 9.5,
        impact_velocity_ms: 850.0,
        projectile_type: "ball",
        tire_pressure_kpa: 220.0,
        run_flat_present: false,
    };
    c.bench_function("tire/penetration", |b| {
        b.iter(|| {
            black_box(tire_penetration::evaluate_tire_penetration(black_box(
                &params,
            )))
        })
    });
}

fn bench_sequential_hits(c: &mut Criterion) {
    let params = SequentialHitParams {
        material: "steel_rha".to_string(),
        plate_width_m: 0.5,
        plate_height_m: 0.5,
        plate_thickness_m: 0.010,
        caliber_m: 0.00762,
        prior_hits: vec![
            HitRecord {
                hit_x_m: 0.25,
                hit_y_m: 0.25,
                impact_energy_j: 3400.0,
                projectile_type: "ap".to_string(),
                zone_id: 0,
            },
            HitRecord {
                hit_x_m: 0.26,
                hit_y_m: 0.24,
                impact_energy_j: 3200.0,
                projectile_type: "ap".to_string(),
                zone_id: 0,
            },
            HitRecord {
                hit_x_m: 0.27,
                hit_y_m: 0.26,
                impact_energy_j: 3100.0,
                projectile_type: "ball".to_string(),
                zone_id: 0,
            },
        ],
        spall_liner_present: true,
        ambient_temp_c: 20.0,
    };
    c.bench_function("sequential_hits/3hit", |b| {
        b.iter(|| {
            black_box(sequential_hits::evaluate_sequential_hits(black_box(
                &params,
            )))
        })
    });
}

fn bench_lot_variation(c: &mut Criterion) {
    let params = LotVariationParams {
        grade: AmmoGrade::Service,
        nominal_mv_ms: 830.0,
        nominal_bc: 0.200,
        temperature_c: 15.0,
        lot_count: 50,
    };
    c.bench_function("lot_variation/stats_50", |b| {
        b.iter(|| black_box(lot_variation::lot_variation_statistics(black_box(&params))))
    });
}

fn bench_predictive_era(c: &mut Criterion) {
    let params = PredictiveERAParams {
        era_type: PredictiveERAType::PredictiveERA {
            detection_range_m: 100.0,
            reaction_time_us: 0.15,
            flyer_velocity_ms: 1000.0,
        },
        threat_velocity_ms: 1500.0,
        threat_type: "ke",
        threat_range_m: 80.0,
        impact_angle_deg: 0.0,
        time_since_last_fire_s: 5.0,
        threat_caliber_mm: 125.0,
        apfsds_tip_shedding_factor: 1.0,
    };
    c.bench_function("predictive_era/ke_threat", |b| {
        b.iter(|| {
            black_box(predictive_era::evaluate_predictive_era(
                black_box(&params),
                None,
            ))
        })
    });
}

fn bench_component_kill_prob(c: &mut Criterion) {
    let params = ComponentKillParams {
        vehicle_type: VehicleType::MBT,
        hit_zone: HitZone::Front,
        projectile_caliber_mm: 125.0,
        projectile_mass_g: 7000.0,
        impact_velocity_ms: 1500.0,
        projectile_type: "apfsds",
        impact_angle_deg: 0.0,
        residual_velocity_ms: 1200.0,
        energy_j: 5_040_000.0,
        armor_penetrated: true,
    };
    c.bench_function("component_kill_prob/mbt_front", |b| {
        b.iter(|| {
            black_box(component_kill_prob::evaluate_component_kill_probability(
                black_box(&params),
            ))
        })
    });
}

fn bench_combined_effects(c: &mut Criterion) {
    let params = CombinedAmmoParams {
        combined_type: CombinedType::API,
        projectile_mass_g: 42.0,
        caliber_mm: 12.7,
        filler_mass_g: 18.0,
        impact_velocity_ms: 900.0,
        armor_penetrated: true,
        residual_velocity_ms: 600.0,
        impact_angle_deg: 0.0,
    };
    c.bench_function("combined_effects/api_50cal", |b| {
        b.iter(|| {
            black_box(combined_effects::evaluate_combined_effects(black_box(
                &params,
            )))
        })
    });
}

fn bench_fire_propagation(c: &mut Criterion) {
    let fire_states = vec![
        ComponentFireState {
            component_index: 0,
            fire_status: FireStatus::Burning,
            fire_intensity: 0.7,
        },
        ComponentFireState {
            component_index: 1,
            fire_status: FireStatus::NoFire,
            fire_intensity: 0.0,
        },
        ComponentFireState {
            component_index: 2,
            fire_status: FireStatus::NoFire,
            fire_intensity: 0.0,
        },
        ComponentFireState {
            component_index: 3,
            fire_status: FireStatus::NoFire,
            fire_intensity: 0.0,
        },
    ];
    let configs = vec![
        ComponentConfig {
            component: ComponentType::FuelTank {
                fuel_type: FuelType::Diesel,
            },
            local_armor_thickness_mm: 5.0,
            local_armor_material: "steel_rha",
            local_angle_deg: 0.0,
        },
        ComponentConfig {
            component: ComponentType::Engine {
                engine_type: EngineType::Diesel,
            },
            local_armor_thickness_mm: 10.0,
            local_armor_material: "steel_rha",
            local_angle_deg: 0.0,
        },
        ComponentConfig {
            component: ComponentType::AmmoRack {
                ammo_type: AmmoStoredType::SmallArms,
            },
            local_armor_thickness_mm: 8.0,
            local_armor_material: "steel_rha",
            local_angle_deg: 0.0,
        },
        ComponentConfig {
            component: ComponentType::Crew {
                protection_level: CrewProtection::Light,
            },
            local_armor_thickness_mm: 5.0,
            local_armor_material: "steel_rha",
            local_angle_deg: 0.0,
        },
    ];
    c.bench_function("component_damage/fire_propagation", |b| {
        b.iter(|| {
            black_box(component_damage::evaluate_fire_propagation(
                black_box(&fire_states),
                black_box(&configs),
                black_box(FireSuppression::Automatic),
                black_box(0.5),
                black_box(false),
            ))
        })
    });
}

fn bench_crew_refined(c: &mut Criterion) {
    c.bench_function("component_damage/crew_refined", |b| {
        b.iter(|| {
            black_box(component_damage::evaluate_crew_refined(
                black_box(CrewProtection::SpallLiner),
                black_box(800.0),
                black_box(40),
                black_box(30.0),
                black_box(15.0),
                black_box("ap"),
                black_box(3),
            ))
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
    bench_sight_height,
    bench_shooter_error,
    bench_material_factor,
    bench_spin_drift,
    bench_get_cd,
    bench_penetration_multilayer,
    bench_interior_wall,
    bench_armor_array,
    bench_frangible,
    bench_tire_penetration,
    bench_sequential_hits,
    bench_lot_variation,
    bench_predictive_era,
    bench_component_kill_prob,
    bench_combined_effects,
    bench_fire_propagation,
    bench_crew_refined,
);
criterion_main!(benches);
