// ABE - C ABI Identity Tests
//
// Traffic-light tests: the Rust-native ballistics computation and the C FFI
// path MUST produce bit-identical results.  If they ever diverge, the test
// fails — regression in the ABI wrapper, struct layout, or unit conversion.
//
// Architecture
// ─────────────
// The crate exposes two API layers:
//   1. String-based ARMA 3 callExtension ABI  (RVExtension / RVExtensionArgs)
//   2. Struct-based C ABI (abe_fire, abe_step, abe_impact)
//
// Both call the same internal physics kernels (interior::calc_muzzle_velocity,
// penetration::evaluate, etc.).  The struct C ABI is the "internal FFI API for
// tests" (see comment in lib.rs).
//
// This test file compares:
//   "Rust native path"  — the physics kernel called directly (replicated or
//                          via public API like penetration::evaluate)
//   "C FFI path"        — the same computation via the extern "C" struct ABI
//
// If any field differs by even one ULP, the test suite fails.

use abe_ballistics_ext::{
    abe_fire, abe_health, abe_impact, abe_init, abe_step, drag, exterior, penetration, BulletState,
    FireParams, FireResult, ImpactParams, ImpactResult, StepParams, MAGIC_ABE,
};
use std::ffi::CStr;

const ABE_API_VERSION: u32 = 1;

// Physical constants matching lib.rs / atmosphere.rs
const GRAVITY_MS2: f64 = 9.80665;
const BC_CONV: f64 = 0.453592 / (0.0254 * 0.0254) * (4.0 / std::f64::consts::PI);

// ── Helpers ──────────────────────────────────────────────────────────────────

fn make_cdm(s: &str) -> [u8; 32] {
    let mut buf = [0u8; 32];
    let bytes = s.as_bytes();
    let len = bytes.len().min(31);
    buf[..len].copy_from_slice(bytes);
    buf
}

/// Read a null-terminated string from a 32-byte CDM buffer.
fn cdm_str(buf: &[u8; 32]) -> &str {
    CStr::from_bytes_until_nul(buf)
        .ok()
        .and_then(|s| s.to_str().ok())
        .unwrap_or("g7")
}

// ── Rust-native replica of interior::calc_muzzle_velocity ──────────────────
//
// Exact copy of the closed-form interior ballistics model from
// ext/src/interior.rs.  The C ABI (abe_fire) applies the same
// unit conversions then calls the same kernel.

/// Regime efficiency multiplier (mirrors interior.rs).
fn replica_regime_mult(chamber_pressure_pa: f64, caliber_m: f64) -> f64 {
    if chamber_pressure_pa < 100e6 && caliber_m > 0.015 {
        return 1.60;
    }
    if chamber_pressure_pa < 300e6 && caliber_m >= 0.007 && caliber_m <= 0.015 {
        return 0.80;
    }
    if chamber_pressure_pa >= 300e6 && caliber_m >= 0.010 {
        return 1.55;
    }
    if chamber_pressure_pa >= 300e6 && caliber_m < 0.010 {
        return 1.15;
    }
    1.0
}

/// Select char_length proportional to barrel length (mirrors interior.rs).
fn replica_char_length(barrel_m: f64) -> f64 {
    if barrel_m < 0.30 {
        0.17 // fast pistol powder
    } else if barrel_m < 0.50 {
        0.28 // medium rifle
    } else if barrel_m < 0.80 {
        0.35 // slow rifle
    } else {
        0.45 // magnum
    }
}

fn native_muzzle_velocity(
    barrel_length_mm: f64,
    chamber_pressure_mpa: f64,
    caliber_mm: f64,
    projectile_mass_g: f64,
) -> Option<NativeMuzzleVelocity> {
    let barrel_m = barrel_length_mm / 1000.0;
    let pressure_pa = chamber_pressure_mpa * 1e6;
    let caliber_m = caliber_mm / 1000.0;
    let mass_kg = projectile_mass_g / 1000.0;

    if barrel_m <= 0.0 || pressure_pa <= 0.0 || caliber_m <= 0.0 || mass_kg <= 0.0 {
        return None;
    }

    let bore_area = std::f64::consts::PI * (caliber_m / 2.0).powi(2);

    // ── Gas expansion pressure curve model ─────────────────────────────────
    let char_length = replica_char_length(barrel_m);
    let work_integral = char_length * (1.0 - (-barrel_m / char_length).exp());

    // ── Energy losses ──────────────────────────────────────────────────────
    let base_efficiency = 0.87;
    let length_efficiency = (-0.30 * barrel_m).exp();
    let regime_mult = replica_regime_mult(pressure_pa, caliber_m);
    let efficiency = (base_efficiency * length_efficiency * regime_mult).clamp(0.1, 1.0);

    // ── Muzzle velocity ────────────────────────────────────────────────────
    // AVG_PRESSURE_FACTOR (0.58) converts peak chamber pressure to effective
    // average pressure over the projectile travel. Must match interior.rs.
    const AVG_PRESSURE_FACTOR: f64 = 0.58;
    let ke = pressure_pa * bore_area * work_integral * efficiency * AVG_PRESSURE_FACTOR;
    let muzzle_velocity = (2.0 * ke / mass_kg).sqrt();

    // ── Derived quantities ─────────────────────────────────────────────────
    let burn_char_length = 0.25;
    let burn_fraction = (1.0 - (-barrel_m / burn_char_length).exp()).clamp(0.25, 1.0);

    let barrel_time_ms = if muzzle_velocity > 1.0 {
        2.0 * barrel_m / muzzle_velocity * 1000.0
    } else {
        0.0
    };

    Some(NativeMuzzleVelocity {
        muzzle_velocity_ms: muzzle_velocity,
        max_chamber_pressure_pa: pressure_pa,
        propellant_burn_fraction: burn_fraction,
        barrel_time_ms,
    })
}

struct NativeMuzzleVelocity {
    muzzle_velocity_ms: f64,
    max_chamber_pressure_pa: f64,
    propellant_burn_fraction: f64,
    barrel_time_ms: f64,
}

// ── Rust-native replica of abe_step physics ────────────────────────────────
//
// Replicates the exact step computation from lib.rs (abe_step body, lines
// 842–937) using ONLY public APIs (exterior::calc_mach, drag::get_cd).
//
// Yaw-of-repose emulation: when the caller passes caliber_mm = 0, the
// gyroscopic stability factor sg becomes 0, which disables all yaw-related
// drag (sg ≥ 1.0 is false, sg > 0.1 is false).  This test file relies on
// that invariant.
//
// Atmosphere: when altitude_m = 0, the code uses params.density_kgm3 directly
// (no call to private atmosphere::density_from_altitude), and wind shear
// returns 1.0 (WIND_REF_HEIGHT = 2 m guard).  This test file uses altitude_m
// = 0 to keep the comparison entirely in terms of the public API.

fn native_step(params: &StepParams) -> BulletState {
    let speed = (params.vel_x.powi(2) + params.vel_y.powi(2) + params.vel_z.powi(2)).sqrt();
    let cd = drag::get_cd(
        cdm_str(&params.cdm_id),
        exterior::calc_mach(speed, params.temp_c),
    );

    // Density: at altitude_m = 0 the function body uses params.density_kgm3
    // (no call to private atmosphere::density_from_altitude).
    let density = params.density_kgm3;

    // ── Drag deceleration ──────────────────────────────────────────────────
    let bc_metric = params.bc * BC_CONV;
    let drag_decel = if speed > 0.001 && bc_metric > 0.001 {
        0.5 * density * speed * speed * cd / bc_metric
    } else {
        0.0
    };

    // ── Yaw-of-repose (inactive: sg = 0 when caliber_mm = 0) ─────────────
    // With caliber_mm = 0:  sg = 0.0, sg ≥ 1.0 = false → no yaw drag.
    //                        sg > 0.1  = false → yaw_repose = 0.0.

    // ── Velocity update: drag (semi-implicit Euler) ───────────────────────
    let vx = params.vel_x - drag_decel * (params.vel_x / speed) * params.dt_s;
    let vy = params.vel_y - drag_decel * (params.vel_y / speed) * params.dt_s;
    let vz = params.vel_z - drag_decel * (params.vel_z / speed) * params.dt_s;

    // ── Gravity (+z = downward) ───────────────────────────────────────────
    let vz = vz + GRAVITY_MS2 * params.dt_s;

    // ── Wind (shear factor = 1.0 at altitude ≤ 2 m) ──────────────────────
    // atmosphere::wind_shear_factor(0.0) returns 1.0 because the constant
    // WIND_REF_HEIGHT = 2.0 triggers the early-return guard.
    const WIND_FACTOR_AT_SEA_LEVEL: f64 = 1.0;
    let vx = vx - params.wind_x * WIND_FACTOR_AT_SEA_LEVEL;
    let vy = vy - params.wind_y * WIND_FACTOR_AT_SEA_LEVEL;
    let vz = vz - params.wind_z * WIND_FACTOR_AT_SEA_LEVEL;

    // ── Position update ───────────────────────────────────────────────────
    let new_speed = (vx.powi(2) + vy.powi(2) + vz.powi(2)).sqrt();

    BulletState {
        pos_x: params.pos_x + vx * params.dt_s,
        pos_y: params.pos_y + vy * params.dt_s,
        pos_z: params.pos_z + vz * params.dt_s,
        vel_x: vx,
        vel_y: vy,
        vel_z: vz,
        mach: exterior::calc_mach(new_speed, params.temp_c),
        time_s: params.dt_s,
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// Tests
// ═════════════════════════════════════════════════════════════════════════════

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Initialize the extension state.  Safe to call from every test because
/// OnceLock::set is idempotent — tests run in parallel within the binary
/// and the first one to arrive initialises STATE, the rest are no-ops.
fn ensure_initialized() {
    assert_eq!(abe_init(ABE_API_VERSION, 0), 0);
    assert_eq!(abe_health(), 1);
}

// ── Struct validation ──────────────────────────────────────────────────────
//
// These tests verify that the C ABI rejects malformed inputs before touching
// any physics kernel, preventing undefined behaviour from misaligned callers.

#[test]
fn cabi_fire_rejects_bad_magic() {
    ensure_initialized();
    let mut params = FireParams {
        magic: MAGIC_ABE,
        ..unsafe { std::mem::zeroed() }
    };
    params.magic = 0xDEAD_BEEF_DEAD_BEEF; // invalid magic
    let mut result = FireResult::default();
    assert_eq!(
        abe_fire(&params, &mut result),
        -2,
        "abe_fire should return -2 on bad magic"
    );
}

#[test]
fn cabi_step_rejects_bad_magic() {
    ensure_initialized();
    let mut params: StepParams = unsafe { std::mem::zeroed() };
    params.magic = 0x0; // invalid magic (zero)
    let mut result = BulletState::default();
    assert_eq!(
        abe_step(&params, &mut result),
        -2,
        "abe_step should return -2 on bad magic"
    );
}

#[test]
fn cabi_impact_rejects_bad_magic() {
    ensure_initialized();
    let mut params: ImpactParams = unsafe { std::mem::zeroed() };
    params.magic = u64::MAX; // invalid magic
    let mut result = ImpactResult::default();
    assert_eq!(
        abe_impact(&params, &mut result),
        -2,
        "abe_impact should return -2 on bad magic"
    );
}

#[test]
fn cabi_fire_zero_magic() {
    ensure_initialized();
    let params = FireParams {
        magic: MAGIC_ABE,
        ..unsafe { std::mem::zeroed() }
    };
    let mut result = FireResult::default();
    // Zeroed params should not crash — sanity check on structural soundness
    let rc = abe_fire(&params, &mut result);
    assert!(rc == -1 || rc == 0, "abe_fire with zeroed fields: {rc}");
}

#[test]
fn cabi_step_zero_params() {
    ensure_initialized();
    // All-zero StepParams with valid magic: should not crash, should return
    // a well-defined state (no movement, no drag)
    let params = StepParams {
        magic: MAGIC_ABE,
        ..unsafe { std::mem::zeroed() }
    };
    let mut result = BulletState::default();
    let rc = abe_step(&params, &mut result);
    assert_eq!(rc, 0, "abe_step with all-zero fields should succeed");
    // With velocity = 0, the NaN guard (speed.max(MIN_POSITIVE)) should
    // prevent NaN propagation in drag divisions.
    assert!(
        result.vel_x.is_finite() && result.vel_y.is_finite() && result.vel_z.is_finite(),
        "zero-velocity step produced non-finite velocity: {:?}",
        (result.vel_x, result.vel_y, result.vel_z)
    );
    // Position should remain at origin (no velocity to move)
    assert_eq!(result.pos_x, 0.0, "pos_x should remain 0");
    assert_eq!(result.pos_y, 0.0, "pos_y should remain 0");
    assert_eq!(result.pos_z, 0.0, "pos_z should remain 0");
}

#[test]
fn cabi_step_zero_speed_const_velocity() {
    ensure_initialized();
    // bc = 0 (no drag), wind = 0, altitude = 0, small non-zero horizontal
    // velocity so the drag term doesn't hit 0/0 in the unguarded division.
    //
    // The body should fall under gravity alone, with no NaN from speed=0
    // in the drag deceleration path.
    let cdm = make_cdm("g7");
    let params = StepParams {
        magic: MAGIC_ABE,
        pos_x: 0.0,
        pos_y: 0.0,
        pos_z: 0.0,
        vel_x: 1e-9, // tiny but non-zero to avoid 0/0 in native_step replica
        vel_y: 0.0,
        vel_z: 0.0,
        dt_s: 0.1,
        wind_x: 0.0,
        wind_y: 0.0,
        wind_z: 0.0,
        density_kgm3: 0.0, // no drag medium
        temp_c: 15.0,
        altitude_m: 0.0,
        cdm_id: cdm,
        bc: 0.0, // no drag
        mass_g: 0.0,
        caliber_mm: 0.0,
        twist_rate_m: 0.0,
    };
    let mut result = BulletState::default();
    let rc = abe_step(&params, &mut result);
    assert_eq!(rc, 0);
    assert!(
        result.vel_z > 0.0,
        "gravity should pull +z: {:?}",
        result.vel_z
    );
    assert!(
        result.vel_x.is_finite() && result.vel_y.is_finite(),
        "non-finite velocity with bc=0: {:?}",
        (result.vel_x, result.vel_y)
    );
}

// ── Fire (interior ballistics) ─────────────────────────────────────────────

#[test]
fn cabi_fire_identity_m855_m4() {
    ensure_initialized();

    let barrel_length_mm = 368.0;
    let chamber_pressure_mpa = 380.0;
    let caliber_mm = 5.56;
    let projectile_mass_g = 4.0;
    let cdm = make_cdm("g7");

    // C FFI path
    let params = FireParams {
        magic: MAGIC_ABE,
        barrel_length_mm,
        chamber_pressure_mpa,
        caliber_mm,
        projectile_mass_g,
        cdm_id: cdm,
    };
    let mut c_result = FireResult::default();
    assert_eq!(abe_fire(&params, &mut c_result), 0);

    // Rust native path
    let native = native_muzzle_velocity(
        barrel_length_mm,
        chamber_pressure_mpa,
        caliber_mm,
        projectile_mass_g,
    )
    .expect("native calcmv");

    assert_eq!(
        c_result.muzzle_velocity_ms, native.muzzle_velocity_ms,
        "MV not bit-identical: C ABI={} native={}",
        c_result.muzzle_velocity_ms, native.muzzle_velocity_ms,
    );
    assert_eq!(
        c_result.max_chamber_pressure_mpa,
        native.max_chamber_pressure_pa / 1e6,
        "pressure not bit-identical: C ABI={} native={}",
        c_result.max_chamber_pressure_mpa,
        native.max_chamber_pressure_pa / 1e6,
    );
    assert_eq!(
        c_result.propellant_burn_fraction, native.propellant_burn_fraction,
        "burn fraction not bit-identical: C ABI={} native={}",
        c_result.propellant_burn_fraction, native.propellant_burn_fraction,
    );
    assert_eq!(
        c_result.barrel_time_ms, native.barrel_time_ms,
        "barrel time not bit-identical: C ABI={} native={}",
        c_result.barrel_time_ms, native.barrel_time_ms,
    );
}

#[test]
fn cabi_fire_identity_nato_762() {
    ensure_initialized();
    // 7.62mm NATO (M80 ball) through a 508 mm barrel
    let barrel_length_mm = 508.0;
    let chamber_pressure_mpa = 360.0;
    let caliber_mm = 7.62;
    let projectile_mass_g = 9.5;
    let cdm = make_cdm("g7");

    let params = FireParams {
        magic: MAGIC_ABE,
        barrel_length_mm,
        chamber_pressure_mpa,
        caliber_mm,
        projectile_mass_g,
        cdm_id: cdm,
    };
    let mut c_result = FireResult::default();
    assert_eq!(abe_fire(&params, &mut c_result), 0);

    let native = native_muzzle_velocity(
        barrel_length_mm,
        chamber_pressure_mpa,
        caliber_mm,
        projectile_mass_g,
    )
    .expect("native calcmv");

    assert_eq!(c_result.muzzle_velocity_ms, native.muzzle_velocity_ms);
    assert_eq!(
        c_result.max_chamber_pressure_mpa,
        native.max_chamber_pressure_pa / 1e6
    );
    assert_eq!(
        c_result.propellant_burn_fraction,
        native.propellant_burn_fraction
    );
    assert_eq!(c_result.barrel_time_ms, native.barrel_time_ms);
}

#[test]
fn cabi_fire_identity_multiple_barrels() {
    ensure_initialized();
    // Sweep barrel lengths to ensure identity holds across the input domain
    let chamber_pressure_mpa = 380.0;
    let caliber_mm = 5.56;
    let projectile_mass_g = 4.0;
    let cdm = make_cdm("g7");

    for barrel_length_mm in [200.0, 254.0, 368.0, 508.0, 610.0] {
        let params = FireParams {
            magic: MAGIC_ABE,
            barrel_length_mm,
            chamber_pressure_mpa,
            caliber_mm,
            projectile_mass_g,
            cdm_id: cdm,
        };
        let mut c_result = FireResult::default();
        assert_eq!(abe_fire(&params, &mut c_result), 0);

        let native = native_muzzle_velocity(
            barrel_length_mm,
            chamber_pressure_mpa,
            caliber_mm,
            projectile_mass_g,
        )
        .expect("native calcmv");

        assert_eq!(
            c_result.muzzle_velocity_ms, native.muzzle_velocity_ms,
            "barrel={}: MV not bit-identical: C ABI={} native={}",
            barrel_length_mm, c_result.muzzle_velocity_ms, native.muzzle_velocity_ms,
        );
    }
}

// ── Step (exterior ballistics integration) ─────────────────────────────────

#[test]
fn cabi_step_identity_single() {
    ensure_initialized();
    // Single step at Mach ~2.7 (930 m/s, sea level, 15 °C).
    // caliber_mm=0 → sg=0 → yaw-of-repose disabled.
    // altitude_m=0 → no atmosphere library calls, wind shear = 1.0.
    let cdm = make_cdm("g7");

    let params = StepParams {
        magic: MAGIC_ABE,
        pos_x: 0.0,
        pos_y: 0.0,
        pos_z: 0.0,
        vel_x: 930.0,
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
        mass_g: 0.0,
        caliber_mm: 0.0,
        twist_rate_m: 0.0,
    };

    // C FFI path
    let mut c_result = BulletState::default();
    assert_eq!(abe_step(&params, &mut c_result), 0);

    // Rust native path
    let native = native_step(&params);

    assert_eq!(c_result.pos_x, native.pos_x, "pos_x");
    assert_eq!(c_result.pos_y, native.pos_y, "pos_y");
    assert_eq!(c_result.pos_z, native.pos_z, "pos_z");
    assert_eq!(c_result.vel_x, native.vel_x, "vel_x");
    assert_eq!(c_result.vel_y, native.vel_y, "vel_y");
    assert_eq!(c_result.vel_z, native.vel_z, "vel_z");
    assert_eq!(c_result.mach, native.mach, "mach");
    assert_eq!(c_result.time_s, native.time_s, "time_s");
}

#[test]
fn cabi_step_identity_multi_step_with_wind() {
    ensure_initialized();
    // 20 steps with 3 m/s crosswind, checking identity at every step.
    let cdm = make_cdm("g7");

    // C-ABI trajectory state
    let (mut cx, mut cy, mut cz) = (0.0, 0.0, 0.0);
    let (mut cvx, mut cvy, mut cvz) = (930.0, 0.0, 0.0);

    // Native trajectory state (runs in lockstep)
    let (mut nx, mut ny, mut nz) = (0.0, 0.0, 0.0);
    let (mut nvx, mut nvy, mut nvz) = (930.0, 0.0, 0.0);

    let base_params = StepParams {
        magic: MAGIC_ABE,
        pos_x: 0.0,
        pos_y: 0.0,
        pos_z: 0.0,
        vel_x: 930.0,
        vel_y: 0.0,
        vel_z: 0.0,
        dt_s: 0.05,
        wind_x: 0.0,
        wind_y: 3.0, // 3 m/s crosswind
        wind_z: 0.0,
        density_kgm3: 1.225,
        temp_c: 15.0,
        altitude_m: 0.0,
        cdm_id: cdm,
        bc: 0.157,
        mass_g: 0.0,
        caliber_mm: 0.0,
        twist_rate_m: 0.0,
    };

    for step_idx in 0..20 {
        // C ABI path
        let mut c_params = base_params;
        c_params.pos_x = cx;
        c_params.pos_y = cy;
        c_params.pos_z = cz;
        c_params.vel_x = cvx;
        c_params.vel_y = cvy;
        c_params.vel_z = cvz;

        let mut c_result = BulletState::default();
        assert_eq!(abe_step(&c_params, &mut c_result), 0, "step {step_idx}");

        cx = c_result.pos_x;
        cy = c_result.pos_y;
        cz = c_result.pos_z;
        cvx = c_result.vel_x;
        cvy = c_result.vel_y;
        cvz = c_result.vel_z;

        // Rust native path
        let mut n_params = base_params;
        n_params.pos_x = nx;
        n_params.pos_y = ny;
        n_params.pos_z = nz;
        n_params.vel_x = nvx;
        n_params.vel_y = nvy;
        n_params.vel_z = nvz;

        let native = native_step(&n_params);
        nx = native.pos_x;
        ny = native.pos_y;
        nz = native.pos_z;
        nvx = native.vel_x;
        nvy = native.vel_y;
        nvz = native.vel_z;

        // Identity at every step
        assert_eq!(cx, nx, "pos_x at step {step_idx}");
        assert_eq!(cy, ny, "pos_y at step {step_idx}");
        assert_eq!(cz, nz, "pos_z at step {step_idx}");
        assert_eq!(cvx, nvx, "vel_x at step {step_idx}");
        assert_eq!(cvy, nvy, "vel_y at step {step_idx}");
        assert_eq!(cvz, nvz, "vel_z at step {step_idx}");
        assert_eq!(c_result.mach, native.mach, "mach at step {step_idx}");

        // Step count (time_s is the delta time, not cumulative)
        assert_eq!(c_result.time_s, native.time_s, "time_s at step {step_idx}");
    }
}

#[test]
fn cabi_step_identity_zero_wind_gravity_only() {
    ensure_initialized();
    // Gravity-only regime: bc = 0 → no drag, vx = 0 → no speed guard edge case,
    // wind = 0 → no wind, altitude = 0 → uses given density.
    //
    // vz after N steps should equal GRAVITY * N * dt exactly (semi-implicit
    // Euler is exact for constant acceleration).
    //
    // NOTE: abe_step divides by speed (without .max(0.001)) in the drag
    // update term.  When bc = 0, drag_decel is already 0, so the 0 × NaN
    // product becomes NaN in IEEE 754.  We therefore use a tiny non-zero
    // horizontal velocity (1.0 m/s) to avoid the NaN while keeping drag
    // negligible.
    let cdm = make_cdm("g7");

    let params = StepParams {
        magic: MAGIC_ABE,
        pos_x: 0.0,
        pos_y: 0.0,
        pos_z: 0.0,
        vel_x: 1.0, // tiny but non-zero to avoid 0/0 in abe_step drag term
        vel_y: 0.0,
        vel_z: 0.0,
        dt_s: 0.1,
        wind_x: 0.0,
        wind_y: 0.0,
        wind_z: 0.0,
        density_kgm3: 0.0,
        temp_c: 15.0,
        altitude_m: 0.0,
        cdm_id: cdm,
        bc: 0.0, // no drag
        mass_g: 0.0,
        caliber_mm: 0.0,
        twist_rate_m: 0.0,
    };

    // C ABI path
    let mut c_result = BulletState::default();
    assert_eq!(abe_step(&params, &mut c_result), 0);

    // Rust native path
    let native = native_step(&params);

    assert_eq!(c_result.pos_x, native.pos_x, "pos_x");
    assert_eq!(c_result.pos_z, native.pos_z, "pos_z");
    assert_eq!(c_result.vel_z, native.vel_z, "vel_z");
    assert_eq!(c_result.vel_x, native.vel_x, "vel_x");
    assert_eq!(c_result.vel_y, native.vel_y, "vel_y");

    // Analytical check: v = g * dt = 0.980665  (bc=0 → no drag, no wind)
    assert!((c_result.vel_z - 0.980665).abs() < 1e-12, "vz ≈ g·dt");
}

// ── Impact (terminal ballistics) ───────────────────────────────────────────
//
// The Rust native path calls penetration::evaluate() directly.
// The C ABI path calls abe_impact() which wraps the same evaluate().

#[test]
fn cabi_impact_identity_penetration() {
    ensure_initialized();
    // 7.62mm ball at 900 m/s vs 5 mm RHA at 0° → should penetrate
    let mut mat = [0u8; 32];
    mat[..10].copy_from_slice(b"steel_rha\0");
    let mut proj = [0u8; 32];
    proj[..5].copy_from_slice(b"ball\0");

    let params = ImpactParams {
        magic: MAGIC_ABE,
        vel_x: 900.0,
        vel_y: 0.0,
        vel_z: 0.0,
        mass_g: 9.5,
        caliber_mm: 7.62,
        armor_thickness_mm: 5.0,
        armor_material: mat,
        impact_angle_deg: 0.0,
        projectile_type: proj,
        yaw_angle_deg: 0.0,
    };

    // C FFI path
    let mut c_result = ImpactResult::default();
    assert_eq!(abe_impact(&params, &mut c_result), 0);

    // Rust native path
    let speed = 900.0;
    let native = penetration::evaluate(
        speed,
        9.5 / 1000.0,  // mass in kg
        7.62 / 1000.0, // caliber in m
        5.0 / 1000.0,  // armor thickness in m
        0.0,           // impact angle normal
        "steel_rha",
        "ball",
        None,
    );

    assert!(native.penetrated, "7.62mm ball should pen 5mm RHA");
    assert_eq!(c_result.penetrated, 1, "C ABI should also pen");

    assert_eq!(c_result.penetrated, native.penetrated as i32, "penetrated");
    assert_eq!(
        c_result.residual_vel_ms, native.residual_velocity,
        "residual_vel"
    );
    assert_eq!(
        c_result.effective_thickness_mm,
        native.effective_thickness * 1000.0,
        "effective_thickness",
    );
    assert_eq!(c_result.ricochet, native.ricochet as i32, "ricochet");
    assert_eq!(
        c_result.ricochet_angle_deg, native.ricochet_angle,
        "ricochet_angle",
    );
    assert_eq!(
        c_result.ricochet_energy_fraction, native.ricochet_energy_fraction,
        "ricochet_energy_fraction",
    );
    assert_eq!(c_result.fragments, native.fragments, "fragments");
    assert_eq!(
        c_result.spall_fragments, native.spall_fragments,
        "spall_fragments"
    );
}

#[test]
fn cabi_impact_identity_ricochet() {
    ensure_initialized();
    // 7.62mm ball at 900 m/s vs 10 mm RHA at 85° → SHOULD ricochet
    let mut mat = [0u8; 32];
    mat[..10].copy_from_slice(b"steel_rha\0");
    let mut proj = [0u8; 32];
    proj[..5].copy_from_slice(b"ball\0");

    let params = ImpactParams {
        magic: MAGIC_ABE,
        vel_x: 900.0,
        vel_y: 0.0,
        vel_z: 0.0,
        mass_g: 9.5,
        caliber_mm: 7.62,
        armor_thickness_mm: 10.0,
        armor_material: mat,
        impact_angle_deg: 85.0,
        projectile_type: proj,
        yaw_angle_deg: 0.0,
    };

    // C FFI path
    let mut c_result = ImpactResult::default();
    assert_eq!(abe_impact(&params, &mut c_result), 0);

    // Rust native path
    let native = penetration::evaluate(
        900.0,
        9.5 / 1000.0,
        7.62 / 1000.0,
        10.0 / 1000.0,
        85.0,
        "steel_rha",
        "ball",
        None,
    );

    assert!(native.ricochet, "native: 85° should ricochet");
    assert_eq!(c_result.ricochet, 1, "C ABI: 85° should ricochet");

    assert_eq!(c_result.penetrated, native.penetrated as i32, "penetrated");
    assert_eq!(
        c_result.residual_vel_ms, native.residual_velocity,
        "residual_vel"
    );
    assert_eq!(
        c_result.effective_thickness_mm,
        native.effective_thickness * 1000.0,
        "effective_thickness",
    );
    assert_eq!(c_result.ricochet, native.ricochet as i32, "ricochet");
    assert_eq!(
        c_result.ricochet_angle_deg, native.ricochet_angle,
        "ricochet_angle",
    );
    assert_eq!(
        c_result.ricochet_energy_fraction, native.ricochet_energy_fraction,
        "ricochet_energy_fraction",
    );
}

#[test]
fn cabi_impact_identity_ap_over_ball() {
    ensure_initialized();
    // AP projectile at 10 mm RHA: verify identity even when pen threshold
    // is borderline.
    let mut mat = [0u8; 32];
    mat[..10].copy_from_slice(b"steel_rha\0");
    let mut p_ball = [0u8; 32];
    p_ball[..5].copy_from_slice(b"ball\0");
    let mut p_ap = [0u8; 32];
    p_ap[..3].copy_from_slice(b"ap\0");

    let build_impact = |proj: &[u8; 32]| -> (i32, f64, f64) {
        let params = ImpactParams {
            magic: MAGIC_ABE,
            vel_x: 900.0,
            vel_y: 0.0,
            vel_z: 0.0,
            mass_g: 9.5,
            caliber_mm: 7.62,
            armor_thickness_mm: 10.0,
            armor_material: mat,
            impact_angle_deg: 0.0,
            projectile_type: *proj,
            yaw_angle_deg: 0.0,
        };
        let mut result = ImpactResult::default();
        assert_eq!(abe_impact(&params, &mut result), 0);
        (
            result.penetrated,
            result.residual_vel_ms,
            result.effective_thickness_mm,
        )
    };

    let (c_ball_pen, c_ball_rv, c_ball_et) = build_impact(&p_ball);
    let (c_ap_pen, c_ap_rv, c_ap_et) = build_impact(&p_ap);

    let n_ball = penetration::evaluate(
        900.0,
        9.5 / 1000.0,
        7.62 / 1000.0,
        10.0 / 1000.0,
        0.0,
        "steel_rha",
        "ball",
        None,
    );
    let n_ap = penetration::evaluate(
        900.0,
        9.5 / 1000.0,
        7.62 / 1000.0,
        10.0 / 1000.0,
        0.0,
        "steel_rha",
        "ap",
        None,
    );

    // Ball identity
    assert_eq!(c_ball_pen, n_ball.penetrated as i32, "ball:penetrated");
    assert_eq!(c_ball_rv, n_ball.residual_velocity, "ball:residual_vel");
    assert_eq!(
        c_ball_et,
        n_ball.effective_thickness * 1000.0,
        "ball:effective_thickness"
    );

    // AP identity
    assert_eq!(c_ap_pen, n_ap.penetrated as i32, "ap:penetrated");
    assert_eq!(c_ap_rv, n_ap.residual_velocity, "ap:residual_vel");
    assert_eq!(
        c_ap_et,
        n_ap.effective_thickness * 1000.0,
        "ap:effective_thickness"
    );

    // AP penetrates at least as well as ball
    assert!(
        c_ap_pen >= c_ball_pen,
        "AP should pen at least as well as ball"
    );
}
