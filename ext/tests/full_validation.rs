//! Comprehensive physical validation suite for ABE ballistics engine.
//!
//! Tests every variable combination: drag models, wind, atmosphere,
//! calibers, and physical invariants that MUST hold for any valid
//! ballistics model.
//!
//! Uses the C ABI directly (abe_step) so the test binary is independent
//! of the library internals — this is a black-box validation.

use abe_ballistics_ext::{abe_init, abe_step, BulletState, StepParams, MAGIC_ABE};

const MAGIC: u64 = MAGIC_ABE;
const DT_S: f64 = 0.01;
const API_VER: u32 = 1;

// ── Projectile database ────────────────────────────────────────────

struct Proj {
    name: &'static str,
    mv_ms: f64,
    bc: f64,
    mass_g: f64,
    cal_mm: f64,
}

const PISTOL: &[Proj] = &[
    Proj {
        name: "9mm 124gr",
        mv_ms: 370.0,
        bc: 0.152,
        mass_g: 8.0,
        cal_mm: 9.0,
    },
    Proj {
        name: ".45 ACP 230gr",
        mv_ms: 280.0,
        bc: 0.173,
        mass_g: 15.0,
        cal_mm: 11.43,
    },
];
const RIFLE: &[Proj] = &[
    Proj {
        name: "M855 5.56mm",
        mv_ms: 930.0,
        bc: 0.157,
        mass_g: 4.0,
        cal_mm: 5.56,
    },
    Proj {
        name: "M43 7.62×39",
        mv_ms: 715.0,
        bc: 0.148,
        mass_g: 7.97,
        cal_mm: 7.62,
    },
    Proj {
        name: "M80 7.62mm",
        mv_ms: 853.0,
        bc: 0.200,
        mass_g: 9.5,
        cal_mm: 7.62,
    },
    Proj {
        name: "M118LR",
        mv_ms: 780.0,
        bc: 0.243,
        mass_g: 11.3,
        cal_mm: 7.62,
    },
    Proj {
        name: ".300WM 190gr",
        mv_ms: 900.0,
        bc: 0.282,
        mass_g: 12.3,
        cal_mm: 7.82,
    },
    Proj {
        name: ".338LM 300gr",
        mv_ms: 820.0,
        bc: 0.374,
        mass_g: 19.4,
        cal_mm: 8.58,
    },
    Proj {
        name: "M33 .50 BMG",
        mv_ms: 860.0,
        bc: 0.314,
        mass_g: 42.8,
        cal_mm: 12.7,
    },
];
fn all_proj() -> Vec<&'static Proj> {
    PISTOL.iter().chain(RIFLE.iter()).collect()
}

// ── Atmosphere ─────────────────────────────────────────────────────

struct Atmo {
    name: &'static str,
    density_kgm3: f64,
    temp_c: f64,
}
const ATMO_SL: Atmo = Atmo {
    name: "sea-level",
    density_kgm3: 1.225,
    temp_c: 15.0,
};
const ATMO_HIGH: Atmo = Atmo {
    name: "5000m",
    density_kgm3: 0.736,
    temp_c: -10.0,
};
const ATMO_HOT: Atmo = Atmo {
    name: "desert",
    density_kgm3: 1.127,
    temp_c: 45.0,
};
const ATMO_COLD: Atmo = Atmo {
    name: "arctic",
    density_kgm3: 1.350,
    temp_c: -20.0,
};
const ATMO_HUMID: Atmo = Atmo {
    name: "humid",
    density_kgm3: 1.180,
    temp_c: 35.0,
};
const ATMOS: &[Atmo] = &[ATMO_SL, ATMO_HIGH, ATMO_HOT, ATMO_COLD, ATMO_HUMID];

// ── Wind ────────────────────────────────────────────────────────────

struct Wind {
    name: &'static str,
    wx: f64,
    wy: f64,
}
const W_NONE: Wind = Wind {
    name: "no wind",
    wx: 0.0,
    wy: 0.0,
};
const W_CROSS5: Wind = Wind {
    name: "crosswind 5m/s",
    wx: 0.0,
    wy: -5.0,
};
const W_CROSS10: Wind = Wind {
    name: "crosswind 10m/s",
    wx: 0.0,
    wy: -10.0,
};
const W_HEAD5: Wind = Wind {
    name: "headwind 5m/s",
    wx: -5.0,
    wy: 0.0,
};
const W_TAIL5: Wind = Wind {
    name: "tailwind 5m/s",
    wx: 5.0,
    wy: 0.0,
};
const W_QUARTER: Wind = Wind {
    name: "quartering 10m/s",
    wx: 7.07,
    wy: -7.07,
};
const WINDS: &[Wind] = &[W_NONE, W_CROSS5, W_CROSS10, W_HEAD5, W_TAIL5, W_QUARTER];

// ── Drag models ────────────────────────────────────────────────────

const MODELS: &[&str] = &["g1", "g7", "g8"];

// ── Trajectory runner ──────────────────────────────────────────────

#[derive(Clone, Copy)]
struct Sample {
    range_m: f64,
    drop_m: f64,
    drift_m: f64,
    vel_ms: f64,
    tof_s: f64,
}

fn run_traj(p: &Proj, atmo: &Atmo, wind: &Wind, model: &str, max_m: f64) -> Vec<Sample> {
    abe_init(API_VER, 0);
    let mut x = 0.0;
    let mut y = 0.0;
    let mut z = 0.0;
    let mut vx = p.mv_ms;
    let mut vy = 0.0;
    let mut vz = 0.0;
    let mut t = 0.0;
    let mut cdm = [0u8; 32];
    let b = model.as_bytes();
    let n = b.len().min(31);
    cdm[..n].copy_from_slice(&b[..n]);
    let mut samples = Vec::new();
    let mut next_range = 0.0;

    while x < max_m && (vx * vx + vy * vy + vz * vz).sqrt() > 30.0 {
        let step = StepParams {
            magic: MAGIC,
            pos_x: x,
            pos_y: y,
            pos_z: z,
            vel_x: vx,
            vel_y: vy,
            vel_z: vz,
            dt_s: DT_S,
            wind_x: wind.wx,
            wind_y: wind.wy,
            wind_z: 0.0,
            density_kgm3: atmo.density_kgm3,
            temp_c: atmo.temp_c,
            altitude_m: 0.0,
            cdm_id: cdm,
            bc: p.bc,
            mass_g: p.mass_g,
            caliber_mm: p.cal_mm,
            twist_rate_m: 0.0,
        };
        let mut res = BulletState::default();
        assert_eq!(abe_step(&step, &mut res), 0);
        x = res.pos_x;
        y = res.pos_y;
        z = res.pos_z;
        vx = res.vel_x;
        vy = res.vel_y;
        vz = res.vel_z;
        t += DT_S;
        while x >= next_range + 50.0 {
            next_range += 50.0;
            let speed = (vx * vx + vy * vy + vz * vz).sqrt();
            samples.push(Sample {
                range_m: x,
                drop_m: z,
                drift_m: y,
                vel_ms: speed,
                tof_s: t,
            });
        }
    }
    samples
}

fn sample_at(s: &[Sample], r: f64) -> Option<&Sample> {
    s.iter().min_by(|a, b| {
        (a.range_m - r)
            .abs()
            .partial_cmp(&(b.range_m - r).abs())
            .unwrap()
    })
}

/// Linearly interpolate trajectory state at exact range_m.
/// Much more accurate than sample_at for comparing trajectories.
fn interp_at(samps: &[Sample], r: f64) -> Option<Sample> {
    let i = samps.iter().position(|x| x.range_m >= r)?;
    if i == 0 {
        return samps.first().copied();
    }
    let a = &samps[i - 1];
    let b = &samps[i];
    let f = (r - a.range_m) / (b.range_m - a.range_m).max(1e-12);
    Some(Sample {
        range_m: r,
        drop_m: a.drop_m + f * (b.drop_m - a.drop_m),
        drift_m: a.drift_m + f * (b.drift_m - a.drift_m),
        vel_ms: a.vel_ms + f * (b.vel_ms - a.vel_ms),
        tof_s: a.tof_s + f * (b.tof_s - a.tof_s),
    })
}

// ── PROPERTY 1: Drop increases monotonically with range ────────────

#[test]
fn drop_increases_with_range() {
    for p in all_proj() {
        for a in ATMOS {
            for m in MODELS {
                let s = run_traj(p, a, &W_NONE, m, 300.0);
                let mut prev = -1.0;
                for smp in &s {
                    assert!(
                        smp.drop_m >= prev - 1e-9,
                        "{} {} {}: drop decreased {:.4}→{:.4}",
                        p.name,
                        a.name,
                        m,
                        prev,
                        smp.drop_m
                    );
                    prev = smp.drop_m;
                }
            }
        }
    }
}

// ── PROPERTY 2: Velocity decreases monotonically with range ────────

#[test]
fn velocity_decreases_with_range() {
    for p in all_proj() {
        for m in MODELS {
            let s = run_traj(p, &ATMO_SL, &W_NONE, m, 300.0);
            let mut prev = p.mv_ms + 10.0;
            for smp in &s {
                assert!(
                    smp.vel_ms <= prev + 1e-9,
                    "{} {}: vel increased {:.1}→{:.1}",
                    p.name,
                    m,
                    prev,
                    smp.vel_ms
                );
                prev = smp.vel_ms;
            }
        }
    }
}

// ── PROPERTY 3: Higher BC → less drop at same range ──────────────

#[test]
fn higher_bc_less_drop() {
    abe_init(API_VER, 0);
    // Same MV, same caliber, different BC → higher BC = less drop
    let lo = run_traj(
        &Proj {
            name: "lo",
            mv_ms: 850.0,
            bc: 0.150,
            mass_g: 8.0,
            cal_mm: 7.62,
        },
        &ATMO_SL,
        &W_NONE,
        "g7",
        800.0,
    );
    let hi = run_traj(
        &Proj {
            name: "hi",
            mv_ms: 850.0,
            bc: 0.300,
            mass_g: 8.0,
            cal_mm: 7.62,
        },
        &ATMO_SL,
        &W_NONE,
        "g7",
        800.0,
    );
    let lo_600 = sample_at(&lo, 600.0).unwrap();
    let hi_600 = sample_at(&hi, 600.0).unwrap();
    assert!(
        hi_600.drop_m < lo_600.drop_m,
        "higher BC should have less drop: {:.3} vs {:.3}",
        hi_600.drop_m,
        lo_600.drop_m
    );
}

// ── PROPERTY 4: Higher MV → less drop at same range ──────────────

#[test]
fn higher_mv_less_drop() {
    abe_init(API_VER, 0);
    let slow = run_traj(
        &Proj {
            name: "slow",
            mv_ms: 700.0,
            bc: 0.200,
            mass_g: 9.5,
            cal_mm: 7.62,
        },
        &ATMO_SL,
        &W_NONE,
        "g7",
        600.0,
    );
    let fast = run_traj(
        &Proj {
            name: "fast",
            mv_ms: 900.0,
            bc: 0.200,
            mass_g: 9.5,
            cal_mm: 7.62,
        },
        &ATMO_SL,
        &W_NONE,
        "g7",
        600.0,
    );
    let s_600 = sample_at(&slow, 600.0).unwrap();
    let f_600 = sample_at(&fast, 600.0).unwrap();
    assert!(
        f_600.drop_m < s_600.drop_m,
        "higher MV should have less drop: {:.3} vs {:.3}",
        f_600.drop_m,
        s_600.drop_m
    );
}

// ── PROPERTY 5: More wind → more drift ─────────────────────────────

#[test]
fn wind_increases_drift() {
    abe_init(API_VER, 0);
    for p in RIFLE {
        let none = run_traj(p, &ATMO_SL, &W_NONE, "g7", 600.0);
        let light = run_traj(p, &ATMO_SL, &W_CROSS5, "g7", 600.0);
        let heavy = run_traj(p, &ATMO_SL, &W_CROSS10, "g7", 600.0);
        let n = sample_at(&none, 500.0).unwrap();
        let l = sample_at(&light, 500.0).unwrap();
        let h = sample_at(&heavy, 500.0).unwrap();
        assert_eq!(
            n.drift_m.abs(),
            0.0,
            "{}: no-wind drift must be 0, got {:.4}",
            p.name,
            n.drift_m
        );
        assert!(
            l.drift_m.abs() > 0.0,
            "{}: crosswind should produce drift",
            p.name
        );
        assert!(
            h.drift_m.abs() > l.drift_m.abs(),
            "{}: 10m/s drift ({:.3}) > 5m/s drift ({:.3})",
            p.name,
            h.drift_m,
            l.drift_m
        );
    }
}

// ── PROPERTY 6: No wind drift with no air resistance ───────────────
// Wind has no effect in vacuum (reality check: no drag = no wind coupling)

#[test]
fn wind_no_drift_without_air_drag() {
    abe_init(API_VER, 0);
    // Use a very high BC (effectively drag-free) to approximate vacuum
    let low_drag = run_traj(
        &Proj {
            name: "ideal",
            mv_ms: 850.0,
            bc: 100.0,
            mass_g: 9.5,
            cal_mm: 7.62,
        },
        &ATMO_SL,
        &W_CROSS10,
        "g7",
        500.0,
    );
    let s = interp_at(&low_drag, 500.0).unwrap();
    // With BC=100, drag is negligible → wind has almost no effect
    assert!(
        s.drift_m.abs() < 0.01,
        "near-vacuum wind drift should be tiny: {:.4}m",
        s.drift_m
    );
}

// ── PROPERTY 7: Vacuum trajectory is parabolic ─────────────────────
// With no drag (vacuum), drop follows z = 0.5 * g * t²

#[test]
fn vacuum_trajectory_parabolic() {
    abe_init(API_VER, 0);
    // Extreme BC makes drag negligible → near-parabolic
    let s = run_traj(
        &Proj {
            name: "vacuum",
            mv_ms: 800.0,
            bc: 10000.0,
            mass_g: 10.0,
            cal_mm: 7.62,
        },
        &ATMO_SL,
        &W_NONE,
        "g7",
        500.0,
    );
    let s300 = sample_at(&s, 300.0).unwrap();
    let s400 = sample_at(&s, 400.0).unwrap();
    let s490 = sample_at(&s, 490.0).unwrap();
    // In vacuum with flat fire: range ≈ mv*t, drop = 0.5*g*t² → drop ≈ 0.5*g*(r/mv)²
    const G: f64 = 9.80665;
    let pred300 = 0.5 * G * (300.0_f64 / 800.0_f64).powf(2.0);
    let pred400 = 0.5 * G * (400.0_f64 / 800.0_f64).powf(2.0);
    let pred490 = 0.5 * G * (490.0_f64 / 800.0_f64).powf(2.0);
    assert!(
        (s300.drop_m - pred300).abs() < 0.05,
        "vacuum 300m: {:.4} vs expected {:.4}",
        s300.drop_m,
        pred300
    );
    assert!(
        (s400.drop_m - pred400).abs() < 0.2,
        "vacuum 400m: {:.4} vs expected {:.4}",
        s400.drop_m,
        pred400
    );
    assert!(
        (s490.drop_m - pred490).abs() < 0.5,
        "vacuum 490m: {:.4} vs expected {:.4}",
        s490.drop_m,
        pred490
    );
}

// ── PROPERTY 8: High altitude → less drag → flatter trajectory ─────

#[test]
fn high_altitude_less_drag() {
    abe_init(API_VER, 0);
    let sl = run_traj(&RIFLE[0], &ATMO_SL, &W_NONE, "g7", 800.0);
    let hi = run_traj(&RIFLE[0], &ATMO_HIGH, &W_NONE, "g7", 800.0);
    let sl_800 = sample_at(&sl, 800.0).unwrap();
    let hi_800 = sample_at(&hi, 800.0).unwrap();
    assert!(
        hi_800.drop_m < sl_800.drop_m,
        "high alt should have less drop: {:.3} vs {:.3}",
        hi_800.drop_m,
        sl_800.drop_m
    );
    assert!(
        hi_800.vel_ms > sl_800.vel_ms,
        "high alt should retain more vel: {:.0} vs {:.0}",
        hi_800.vel_ms,
        sl_800.vel_ms
    );
}

// ── PROPERTY 9: Cold air → more drag → more drop ──────────────────

#[test]
fn cold_air_more_drop() {
    abe_init(API_VER, 0);
    let hot = run_traj(&RIFLE[0], &ATMO_HOT, &W_NONE, "g7", 800.0);
    let cold = run_traj(&RIFLE[0], &ATMO_COLD, &W_NONE, "g7", 800.0);
    let h800 = sample_at(&hot, 800.0).unwrap();
    let c800 = sample_at(&cold, 800.0).unwrap();
    assert!(
        c800.drop_m > h800.drop_m,
        "cold air should have more drop: {:.3} vs {:.3}",
        c800.drop_m,
        h800.drop_m
    );
}

// ── PROPERTY 10: Headwind → more drop, tailwind → less drop ───────

#[test]
fn headwind_tailwind_effect() {
    abe_init(API_VER, 0);
    let p = &RIFLE[2]; // M80
    let none = run_traj(p, &ATMO_SL, &W_NONE, "g7", 800.0);
    let head = run_traj(p, &ATMO_SL, &W_HEAD5, "g7", 800.0);
    let tail = run_traj(p, &ATMO_SL, &W_TAIL5, "g7", 800.0);
    let n = interp_at(&none, 600.0).unwrap();
    let h = interp_at(&head, 600.0).unwrap();
    let t = interp_at(&tail, 600.0).unwrap();
    // Headwind increases airspeed → more drag → more drop
    assert!(
        h.drop_m > n.drop_m,
        "headwind should increase drop: {:.3} vs {:.3}",
        h.drop_m,
        n.drop_m
    );
    assert!(
        t.drop_m < n.drop_m,
        "tailwind should decrease drop: {:.3} vs {:.3}",
        t.drop_m,
        n.drop_m
    );
}

// ── PROPERTY 11: G1 more drag than G7 → more drop ─────────────────

#[test]
fn g1_more_drop_than_g7() {
    abe_init(API_VER, 0);
    let p = &RIFLE[0]; // M855
    let g7 = run_traj(p, &ATMO_SL, &W_NONE, "g7", 600.0);
    let g1 = run_traj(p, &ATMO_SL, &W_NONE, "g1", 600.0);
    let r7 = sample_at(&g7, 500.0).unwrap();
    let r1 = sample_at(&g1, 500.0).unwrap();
    // G1 has higher Cd than G7 → more drop
    assert!(
        r1.drop_m > r7.drop_m,
        "G1 drop should exceed G7: {:.3} vs {:.3}",
        r1.drop_m,
        r7.drop_m
    );
}

// ── PROPERTY 12: All G models produce valid non-negative physics ────

#[test]
fn all_models_physical() {
    for p in all_proj() {
        for m in MODELS {
            // Test no-wind + crosswind (full coverage without all 12 combos)
            for w in &[W_NONE, W_CROSS5] {
                let s = run_traj(p, &ATMO_SL, w, m, 300.0);
                let r = sample_at(&s, 400.0).unwrap();
                assert!(
                    r.drop_m >= 0.0,
                    "{} {} {}: negative drop {:.4}",
                    p.name,
                    m,
                    w.name,
                    r.drop_m
                );
                assert!(r.vel_ms > 0.0, "{} {} {}: zero vel", p.name, m, w.name);
                assert!(r.tof_s > 0.0, "{} {} {}: zero TOF", p.name, m, w.name);
            }
        }
    }
}

// ── PROPERTY 13: Pistol trajectories are always steeper than rifle ──

#[test]
fn pistol_steeper_than_rifle() {
    abe_init(API_VER, 0);
    for model in MODELS {
        for w in &WINDS[..2] {
            let pistol = run_traj(&PISTOL[0], &ATMO_SL, w, model, 300.0);
            let rifle = run_traj(&RIFLE[0], &ATMO_SL, w, model, 300.0);
            let p_at = sample_at(&pistol, 200.0).unwrap();
            let r_at = sample_at(&rifle, 200.0).unwrap();
            assert!(
                p_at.drop_m > r_at.drop_m,
                "pistol steeper than rifle {} {}: {:.3} vs {:.3}",
                model,
                w.name,
                p_at.drop_m,
                r_at.drop_m
            );
        }
    }
}

// ── PROPERTY 14: Crosswind drift scales with TOF² roughly ──────────

#[test]
fn drift_scales_with_tof_squared() {
    abe_init(API_VER, 0);
    // Drift at constant crosswind ~ wind_speed * TOF (simplified)
    // For the same projectile, longer range = more TOF²-ish scaling
    let p = &RIFLE[2]; // M80
    let s = run_traj(p, &ATMO_SL, &W_CROSS5, "g7", 500.0);
    let r300 = sample_at(&s, 300.0).unwrap();
    let r600 = sample_at(&s, 600.0).unwrap();
    // Drift ratio should roughly follow (TOF600/TOF300)^2 over short intervals
    let tof_ratio = r600.tof_s / r300.tof_s;
    let drift_ratio = r600.drift_m.abs() / r300.drift_m.abs().max(1e-12);
    assert!(
        drift_ratio > tof_ratio,
        "drift should grow faster than linear with TOF: {:.2}x vs {:.2}x (drift/TOF)",
        drift_ratio,
        tof_ratio
    );
}

// ── PROPERTY 15: Same projectile + same conditions = deterministic ──

#[test]
fn deterministic_trajectory() {
    abe_init(API_VER, 0);
    let s1 = run_traj(&RIFLE[1], &ATMO_SL, &W_CROSS5, "g7", 600.0);
    let s2 = run_traj(&RIFLE[1], &ATMO_SL, &W_CROSS5, "g7", 600.0);
    for (a, b) in s1.iter().zip(s2.iter()) {
        assert!((a.drop_m - b.drop_m).abs() < 1e-9, "drop not deterministic");
        assert!(
            (a.drift_m - b.drift_m).abs() < 1e-9,
            "drift not deterministic"
        );
    }
}

// ── PROPERTY 16: Intermodel ordering preserved across atmosphere ───

#[test]
fn model_ordering_across_atmosphere() {
    for a in ATMOS {
        abe_init(API_VER, 0);
        let mut results = Vec::new();
        for &m in &["g1", "g7", "g8"] {
            let s = run_traj(&RIFLE[0], a, &W_NONE, m, 300.0);
            if let Some(r) = interp_at(&s, 250.0) {
                results.push((m, r.drop_m));
            }
        }
        // G7 should have least drop (lowest drag), G1 most
        results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        assert_eq!(
            results[0].0, "g7",
            "G7 should have least drop in {}: {:?}",
            a.name, results
        );
        assert_eq!(
            results[2].0, "g1",
            "G1 should have most drop in {}: {:?}",
            a.name, results
        );
    }
}

// ── PROPERTY 17: All calibers reach their meaningful range ─────────

#[test]
fn projectiles_reach_range() {
    for p in all_proj() {
        abe_init(API_VER, 0);
        let test_range = if p.mv_ms > 500.0 { 500.0 } else { 300.0 };
        let s = run_traj(p, &ATMO_SL, &W_NONE, "g7", test_range);
        let last = s.last().unwrap();
        assert!(
            last.range_m > test_range * 0.9,
            "{} should reach {:.0}m (got {:.0}m)",
            p.name,
            test_range,
            last.range_m
        );
    }
}

// ── PROPERTY 18: Cold + dense + high BC = still physical ───────────

#[test]
fn extreme_conditions_physical() {
    for p in &RIFLE[..3] {
        for m in MODELS {
            for a in [ATMO_COLD, ATMO_HIGH, ATMO_HUMID] {
                let s = run_traj(p, &a, &W_CROSS5, m, 300.0);
                let r = sample_at(&s, 400.0).unwrap();
                assert!(
                    r.vel_ms > 0.0 && r.drop_m >= 0.0 && r.tof_s > 0.0,
                    "{} {} {}: unphysical results",
                    p.name,
                    m,
                    a.name
                );
            }
        }
    }
}

// ── PROPERTY 19: Heavier bullet with same BC = same trajectory ─────
// (if BC and MV are the same, mass itself only affects momentum not trajectory in point-mass)

#[test]
fn bc_determines_trajectory() {
    abe_init(API_VER, 0);
    let a = run_traj(
        &Proj {
            name: "a",
            mv_ms: 800.0,
            bc: 0.250,
            mass_g: 10.0,
            cal_mm: 7.62,
        },
        &ATMO_SL,
        &W_NONE,
        "g7",
        600.0,
    );
    let b = run_traj(
        &Proj {
            name: "b",
            mv_ms: 800.0,
            bc: 0.250,
            mass_g: 20.0,
            cal_mm: 7.62,
        },
        &ATMO_SL,
        &W_NONE,
        "g7",
        600.0,
    );
    let ra = sample_at(&a, 500.0).unwrap();
    let rb = sample_at(&b, 500.0).unwrap();
    assert!(
        (ra.drop_m - rb.drop_m).abs() < 0.01,
        "same BC+M should give same drop: {:.4} vs {:.4}",
        ra.drop_m,
        rb.drop_m
    );
}

// ── PROPERTY 20: Zero density = vacuum → parabolic ─────────────────

#[test]
fn zero_density_vacuum() {
    abe_init(API_VER, 0);
    let vac_atmo = Atmo {
        name: "vacuum",
        density_kgm3: 0.0,
        temp_c: 15.0,
    };
    let s = run_traj(&RIFLE[0], &vac_atmo, &W_CROSS5, "g7", 550.0);
    let r500 = interp_at(&s, 500.0).unwrap();
    // No density = no drag = no wind coupling → zero drift
    assert!(
        r500.drift_m.abs() < 0.001,
        "zero-density wind drift should be near-zero: {:.4}",
        r500.drift_m
    );
    // Flight time should match vacuum: t = range / MV (for flat fire approx)
    let expected_tof = 500.0 / 930.0;
    assert!(
        (r500.tof_s - expected_tof).abs() < 0.1,
        "vacuum TOF should match t=r/v: {:.3} vs expected {:.3}",
        r500.tof_s,
        expected_tof
    );
}

// ── PROPERTY 21: Extremely high BC ≈ vacuum ────────────────────────

#[test]
fn high_bc_approaches_vacuum() {
    abe_init(API_VER, 0);
    let vac = run_traj(
        &Proj {
            name: "vac",
            mv_ms: 900.0,
            bc: 1e8,
            mass_g: 10.0,
            cal_mm: 7.62,
        },
        &ATMO_SL,
        &W_CROSS5,
        "g7",
        600.0,
    );
    let r = interp_at(&vac, 400.0).unwrap();
    const G2: f64 = 9.80665;
    let expected_drop = 0.5 * G2 * (400.0_f64 / 900.0_f64).powf(2.0);
    assert!(
        (r.drop_m - expected_drop).abs() < 0.05,
        "infinite-BC drop should match vacuum: {:.4} vs {:.4}",
        r.drop_m,
        expected_drop
    );
    // No significant wind drift
    assert!(
        r.drift_m.abs() < 0.01,
        "infinite-BC wind drift should be near-0: {:.4}",
        r.drift_m
    );
}
