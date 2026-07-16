// ABE - Advanced Ballistics Extension
// C ABI dispatcher for ARMA 3 callExtension interface
//
// All physics kernels are pure functions. Two API layers:
//   1. RVExtension / RVExtensionArgs — string-based ARMA 3 callExtension ABI
//   2. Struct-based C ABI (abe_fire, abe_step, etc.) — internal FFI API for tests
//
// Both call the same physics kernels.

#![allow(dead_code)]

mod atmosphere;
mod config;
mod drag;
mod exterior;
mod fragmentation;
mod interior;
mod penetration;

use std::ffi::{CStr, CString};
use std::fmt::Write;
use std::os::raw::c_char;
use std::sync::OnceLock;

// ── Version contract ──────────────────────────────────────────────────────────

const ABE_API_VERSION: u32 = 1;
const ABE_VERSION: &str = "0.1.0";

// ── Global state ──────────────────────────────────────────────────────────────

struct AbeState {
    initialized: bool,
    ace_present: bool,
    data_loaded: bool,
}

static STATE: OnceLock<AbeState> = OnceLock::new();

fn get_state() -> &'static AbeState {
    STATE.get_or_init(|| AbeState {
        initialized: false,
        ace_present: false,
        data_loaded: false,
    })
}

// ── ARMA 3 callExtension API (string dispatch) ───────────────────────────────
//
// These are the functions ARMA 3 actually resolves when SQF calls:
//   "abe_ballistics_ext" callExtension "command"
//   "abe_ballistics_ext" callExtension ["command", [args...]]

const OUTPUT_BUF_SIZE: usize = 2048;

/// Write a string into ARMA 3's output buffer, safely truncated + null-terminated.
unsafe fn write_output(output: *mut c_char, output_size: i32, s: &str) {
    let cap = (output_size as usize).min(OUTPUT_BUF_SIZE);
    let bytes = s.as_bytes();
    let len = bytes.len().min(cap.saturating_sub(1));
    // SAFETY: output buffer is guaranteed valid by ARMA 3 contract
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), output as *mut u8, len);
        *output.add(len) = 0;
    }
}

/// Convert ARMA 3's const char **args into a Vec<&str>.
unsafe fn parse_args<'a>(args: *const *const c_char, cnt: i32) -> Vec<&'a str> {
    // SAFETY: args pointer + cnt are guaranteed valid by ARMA 3 contract
    let slice = unsafe { std::slice::from_raw_parts(args, cnt as usize) };
    slice
        .iter()
        .map(|&p| {
            // SAFETY: each pointer in the array is guaranteed null-terminated
            unsafe { CStr::from_ptr(p) }.to_str().unwrap_or("")
        })
        .collect()
}

/// Format an f64 to a compact string, avoiding trailing zeros.
fn fmt_f64(val: f64) -> String {
    if val.fract() == 0.0 && val.abs() < 1e12 {
        format!("{:.1}", val)
    } else {
        let s = format!("{:.6}", val);
        let trimmed = s.trim_end_matches('0');
        let trimmed = trimmed.trim_end_matches('.');
        if trimmed.is_empty() {
            "0".into()
        } else {
            trimmed.to_string()
        }
    }
}

// ── String-mode handlers (RVExtension) ────────────────────────────────────────

fn handle_string_command(function: &str, output: &mut String) {
    match function {
        "version" => *output = ABE_VERSION.to_string(),
        "health" => {
            *output = if get_state().initialized {
                "1".into()
            } else {
                "0".into()
            }
        }
        other => {
            let _ = write!(output, "{}", format!("unknown: {}", other));
        }
    }
}

// ── Array-mode handlers (RVExtensionArgs) ────────────────────────────────────

fn handle_init(args: &[&str]) -> String {
    let api_version: u32 = args.first().and_then(|s| s.parse().ok()).unwrap_or(0);
    let ace_present: u32 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);

    if api_version != ABE_API_VERSION {
        return "-1".into();
    }

    let state = AbeState {
        initialized: true,
        ace_present: ace_present != 0,
        data_loaded: false,
    };
    let _ = STATE.set(state);
    "0".into()
}

fn handle_fire(args: &[&str]) -> String {
    if !get_state().initialized {
        return "-1".into();
    }

    let barrel_length_mm: f64 = args.first().and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let chamber_pressure_mpa: f64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let caliber_mm: f64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let projectile_mass_g: f64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let cdm_id = args.get(4).copied().unwrap_or("g7");

    let r = interior::calc_muzzle_velocity(
        barrel_length_mm / 1000.0,
        chamber_pressure_mpa * 1e6,
        caliber_mm / 1000.0,
        projectile_mass_g / 1000.0,
        cdm_id,
    );

    match r {
        Some(mv) => {
            format!(
                "[{},{},{},{}]",
                fmt_f64(mv.muzzle_velocity),
                fmt_f64(mv.max_chamber_pressure / 1e6),
                fmt_f64(mv.propellant_burn_fraction),
                fmt_f64(mv.barrel_time_ms),
            )
        }
        None => "-1".into(),
    }
}

fn handle_step(args: &[&str]) -> String {
    if !get_state().initialized {
        return "-1".into();
    }

    let pos_x: f64 = args.first().and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let pos_y: f64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let pos_z: f64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let vel_x: f64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let vel_y: f64 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let vel_z: f64 = args.get(5).and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let dt_s: f64 = args.get(6).and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let wind_x: f64 = args.get(7).and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let wind_y: f64 = args.get(8).and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let wind_z: f64 = args.get(9).and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let density: f64 = args.get(10).and_then(|s| s.parse().ok()).unwrap_or(1.225);
    let temp_c: f64 = args.get(11).and_then(|s| s.parse().ok()).unwrap_or(15.0);
    let altitude_m: f64 = args.get(12).and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let cdm_id = args.get(13).copied().unwrap_or("g7");
    let _bc: f64 = args.get(14).and_then(|s| s.parse().ok()).unwrap_or(0.157);
    let mass_g: f64 = args.get(15).and_then(|s| s.parse().ok()).unwrap_or(4.0);
    let caliber_mm: f64 = args.get(16).and_then(|s| s.parse().ok()).unwrap_or(5.56);

    let speed = (vel_x.powi(2) + vel_y.powi(2) + vel_z.powi(2)).sqrt();
    let mach = exterior::calc_mach(speed, temp_c);
    let cd = drag::get_cd(cdm_id, mach);

    let air_density = if altitude_m > 0.0 && (temp_c - 15.0).abs() < 0.1 {
        atmosphere::density_from_altitude(altitude_m, temp_c)
    } else {
        density
    };

    let cross_section = std::f64::consts::PI * (caliber_mm / 2000.0).powi(2);
    let drag_force = 0.5 * air_density * speed * speed * cd * cross_section;
    let mass_kg = mass_g / 1000.0;

    let drag_decel = if speed > 0.001 {
        drag_force / mass_kg
    } else {
        0.0
    };

    let wind_factor = atmosphere::wind_shear_factor(altitude_m);
    let vx = vel_x - drag_decel * (vel_x / speed.max(0.001)) * dt_s - wind_x * wind_factor;
    let vy = vel_y - drag_decel * (vel_y / speed.max(0.001)) * dt_s - wind_y * wind_factor;
    let vz = vel_z - drag_decel * (vel_z / speed.max(0.001)) * dt_s + atmosphere::GRAVITY * dt_s
        - wind_z * wind_factor;

    let new_speed = (vx.powi(2) + vy.powi(2) + vz.powi(2)).sqrt();
    let new_mach = exterior::calc_mach(new_speed, temp_c);

    format!(
        "[{},{},{},{},{},{},{},{}]",
        fmt_f64(pos_x + vx * dt_s),
        fmt_f64(pos_y + vy * dt_s),
        fmt_f64(pos_z + vz * dt_s),
        fmt_f64(vx),
        fmt_f64(vy),
        fmt_f64(vz),
        fmt_f64(new_mach),
        fmt_f64(dt_s),
    )
}

fn handle_impact(args: &[&str]) -> String {
    if !get_state().initialized {
        return "-1".into();
    }

    let vel_x: f64 = args.first().and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let vel_y: f64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let vel_z: f64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let mass_g: f64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let caliber_mm: f64 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let armor_thickness_mm: f64 = args.get(5).and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let armor_material = args.get(6).copied().unwrap_or("steel_rha");
    let impact_angle_deg: f64 = args.get(7).and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let projectile_type = args.get(8).copied().unwrap_or("ball");

    let speed = (vel_x.powi(2) + vel_y.powi(2) + vel_z.powi(2)).sqrt();
    let energy = 0.5 * (mass_g / 1000.0) * speed * speed;

    let pen = penetration::evaluate(
        speed,
        mass_g / 1000.0,
        caliber_mm / 1000.0,
        armor_thickness_mm / 1000.0,
        impact_angle_deg,
        armor_material,
        projectile_type,
    );

    format!(
        "[{},{},{},{},{},{},{},{},{}]",
        pen.penetrated as i32,
        fmt_f64(pen.residual_velocity),
        fmt_f64(energy),
        fmt_f64(pen.effective_thickness * 1000.0),
        pen.ricochet as i32,
        fmt_f64(pen.ricochet_angle),
        fmt_f64(pen.ricochet_energy_fraction),
        pen.fragments,
        pen.spall_fragments,
    )
}

// ── ARMA 3 entry points ───────────────────────────────────────────────────────

/// String-mode callExtension: "ext" callExtension "command"
#[unsafe(no_mangle)]
pub unsafe extern "C" fn RVExtension(
    output: *mut c_char,
    output_size: i32,
    function: *const c_char,
) {
    // SAFETY: function pointer is guaranteed null-terminated by ARMA 3 contract
    let func = unsafe { CStr::from_ptr(function) }.to_str().unwrap_or("");
    let mut result = String::with_capacity(256);
    handle_string_command(func, &mut result);
    // SAFETY: output buffer is valid per ARMA 3 contract
    unsafe { write_output(output, output_size, &result) };
}

/// Array-mode callExtension: "ext" callExtension ["command", [args...]]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn RVExtensionArgs(
    output: *mut c_char,
    output_size: i32,
    function: *const c_char,
    args: *const *const c_char,
    args_cnt: i32,
) {
    // SAFETY: all pointers + count are guaranteed valid by ARMA 3 contract
    let func = unsafe { CStr::from_ptr(function) }.to_str().unwrap_or("");
    let parsed = unsafe { parse_args(args, args_cnt) };

    let result = match func {
        "init" => handle_init(&parsed),
        "fire" => handle_fire(&parsed),
        "step" => handle_step(&parsed),
        "impact" => handle_impact(&parsed),
        other => format!("unknown: {}", other),
    };

    // SAFETY: output buffer is valid per ARMA 3 contract
    unsafe { write_output(output, output_size, &result) };
}

// ── Struct-based C ABI (internal API for tests and FFI) ───────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn abe_init(api_version: u32, ace_present: u32) -> i32 {
    if api_version != ABE_API_VERSION {
        return -1;
    }
    let state = AbeState {
        initialized: true,
        ace_present: ace_present != 0,
        data_loaded: false,
    };
    let _ = STATE.set(state);
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn abe_version() -> *const c_char {
    static VERSION: OnceLock<CString> = OnceLock::new();
    VERSION
        .get_or_init(|| CString::new(ABE_VERSION).unwrap())
        .as_ptr()
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FireParams {
    pub barrel_length_mm: f64,
    pub chamber_pressure_mpa: f64,
    pub caliber_mm: f64,
    pub projectile_mass_g: f64,
    pub cdm_id: [u8; 32],
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FireResult {
    pub muzzle_velocity_ms: f64,
    pub max_chamber_pressure_mpa: f64,
    pub propellant_burn_fraction: f64,
    pub barrel_time_ms: f64,
}

#[unsafe(no_mangle)]
pub extern "C" fn abe_fire(params: &FireParams, result: &mut FireResult) -> i32 {
    if !get_state().initialized {
        return -1;
    }

    let cdm_str = match CStr::from_bytes_until_nul(&params.cdm_id) {
        Ok(s) => s.to_str().unwrap_or("g7"),
        Err(_) => "g7",
    };

    let r = interior::calc_muzzle_velocity(
        params.barrel_length_mm / 1000.0,
        params.chamber_pressure_mpa * 1e6,
        params.caliber_mm / 1000.0,
        params.projectile_mass_g / 1000.0,
        cdm_str,
    );

    match r {
        Some(mv) => {
            *result = FireResult {
                muzzle_velocity_ms: mv.muzzle_velocity,
                max_chamber_pressure_mpa: mv.max_chamber_pressure / 1e6,
                propellant_burn_fraction: mv.propellant_burn_fraction,
                barrel_time_ms: mv.barrel_time_ms,
            };
            0
        }
        None => -1,
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BulletState {
    pub pos_x: f64,
    pub pos_y: f64,
    pub pos_z: f64,
    pub vel_x: f64,
    pub vel_y: f64,
    pub vel_z: f64,
    pub mach: f64,
    pub time_s: f64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct StepParams {
    pub pos_x: f64,
    pub pos_y: f64,
    pub pos_z: f64,
    pub vel_x: f64,
    pub vel_y: f64,
    pub vel_z: f64,
    pub dt_s: f64,
    pub wind_x: f64,
    pub wind_y: f64,
    pub wind_z: f64,
    pub density_kgm3: f64,
    pub temp_c: f64,
    pub altitude_m: f64,
    pub cdm_id: [u8; 32],
    pub bc: f64,
    pub mass_g: f64,
    pub caliber_mm: f64,
}

/// Step a bullet forward by dt seconds.
/// Returns 0 on success, -1 on error.
#[unsafe(no_mangle)]
pub extern "C" fn abe_step(params: &StepParams, result: &mut BulletState) -> i32 {
    if !get_state().initialized {
        return -1;
    }

    let cdm_str = match CStr::from_bytes_until_nul(&params.cdm_id) {
        Ok(s) => s.to_str().unwrap_or("g7"),
        Err(_) => "g7",
    };

    // Get drag coefficient at current Mach
    let mach = exterior::calc_mach(
        (params.vel_x.powi(2) + params.vel_y.powi(2) + params.vel_z.powi(2)).sqrt(),
        params.temp_c,
    );

    let cd = drag::get_cd(cdm_str, mach);

    // Air density from altitude if provided, otherwise use given density
    let density = if params.altitude_m > 0.0 {
        atmosphere::density_from_altitude(params.altitude_m, params.temp_c)
    } else {
        params.density_kgm3
    };

    // Apply drag: F_drag = 0.5 * ρ * v² * Cd * A
    let speed = (params.vel_x.powi(2) + params.vel_y.powi(2) + params.vel_z.powi(2)).sqrt();
    let cross_section = std::f64::consts::PI * (params.caliber_mm / 2000.0).powi(2);
    let drag_force = 0.5 * density * speed * speed * cd * cross_section;
    let mass_kg = params.mass_g / 1000.0;

    // Drag deceleration (opposite velocity direction)
    let drag_decel = if speed > 0.001 {
        drag_force / mass_kg
    } else {
        0.0
    };

    let vx = params.vel_x - drag_decel * (params.vel_x / speed) * params.dt_s;
    let vy = params.vel_y - drag_decel * (params.vel_y / speed) * params.dt_s;
    let vz = params.vel_z - drag_decel * (params.vel_z / speed) * params.dt_s;

    // Gravity
    let vz = vz + atmosphere::GRAVITY * params.dt_s;

    // Wind (relative velocity) with altitude-based wind shear
    let wind_factor = atmosphere::wind_shear_factor(params.altitude_m);
    let vx = vx - params.wind_x * wind_factor;
    let vy = vy - params.wind_y * wind_factor;
    let vz = vz - params.wind_z * wind_factor;

    // Position update
    let new_speed = (vx.powi(2) + vy.powi(2) + vz.powi(2)).sqrt();
    let new_mach = exterior::calc_mach(new_speed, params.temp_c);

    *result = BulletState {
        pos_x: params.pos_x + vx * params.dt_s,
        pos_y: params.pos_y + vy * params.dt_s,
        pos_z: params.pos_z + vz * params.dt_s,
        vel_x: vx,
        vel_y: vy,
        vel_z: vz,
        mach: new_mach,
        time_s: params.dt_s, // delta time, caller accumulates
    };

    0
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ImpactParams {
    pub vel_x: f64,
    pub vel_y: f64,
    pub vel_z: f64,
    pub mass_g: f64,
    pub caliber_mm: f64,
    pub armor_thickness_mm: f64,
    pub armor_material: [u8; 32],
    pub impact_angle_deg: f64,
    pub projectile_type: [u8; 32],
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ImpactResult {
    pub penetrated: i32,
    pub residual_vel_ms: f64,
    pub energy_j: f64,
    pub effective_thickness_mm: f64,
    pub ricochet: i32,
    pub ricochet_angle_deg: f64,
    pub ricochet_energy_fraction: f64,
    pub fragments: i32,
    pub spall_fragments: i32,
}

/// Calculate impact effects for a bullet vs armor.
/// Returns 0 on success, -1 on error.
#[unsafe(no_mangle)]
pub extern "C" fn abe_impact(params: &ImpactParams, result: &mut ImpactResult) -> i32 {
    if !get_state().initialized {
        return -1;
    }

    let speed = (params.vel_x.powi(2) + params.vel_y.powi(2) + params.vel_z.powi(2)).sqrt();
    let energy = 0.5 * (params.mass_g / 1000.0) * speed * speed;

    let material_str = match CStr::from_bytes_until_nul(&params.armor_material) {
        Ok(s) => s.to_str().unwrap_or("steel_rha"),
        Err(_) => "steel_rha",
    };

    let proj_str = match CStr::from_bytes_until_nul(&params.projectile_type) {
        Ok(s) => s.to_str().unwrap_or("ball"),
        Err(_) => "ball",
    };

    let pen_result = penetration::evaluate(
        speed,
        params.mass_g / 1000.0,
        params.caliber_mm / 1000.0,
        params.armor_thickness_mm / 1000.0,
        params.impact_angle_deg,
        material_str,
        proj_str,
    );

    *result = ImpactResult {
        penetrated: pen_result.penetrated as i32,
        residual_vel_ms: pen_result.residual_velocity,
        energy_j: energy,
        effective_thickness_mm: pen_result.effective_thickness * 1000.0,
        ricochet: pen_result.ricochet as i32,
        ricochet_angle_deg: pen_result.ricochet_angle,
        ricochet_energy_fraction: pen_result.ricochet_energy_fraction,
        fragments: pen_result.fragments,
        spall_fragments: pen_result.spall_fragments,
    };

    0
}

/// Health check — returns 1 if extension is initialized and functional.
#[unsafe(no_mangle)]
pub extern "C" fn abe_health() -> i32 {
    if get_state().initialized {
        1
    } else {
        0
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // Helpers to call RVExtension / RVExtensionArgs as strings
    fn rv_ext(func: &str) -> String {
        let mut buf = vec![0u8; OUTPUT_BUF_SIZE];
        let cfunc = CString::new(func).unwrap();
        unsafe {
            RVExtension(
                buf.as_mut_ptr() as *mut c_char,
                OUTPUT_BUF_SIZE as i32,
                cfunc.as_ptr(),
            );
        }
        let end = buf.iter().position(|&b| b == 0).unwrap_or(0);
        std::str::from_utf8(&buf[..end]).unwrap().to_string()
    }

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

    // ── Struct-based C ABI tests ─────────────────────────────────────────────

    #[test]
    fn init_and_health() {
        assert_eq!(abe_init(ABE_API_VERSION, 0), 0);
        assert_eq!(abe_health(), 1);
    }

    #[test]
    fn version_string() {
        let ptr = abe_version();
        let cstr = unsafe { CStr::from_ptr(ptr) };
        assert_eq!(cstr.to_str().unwrap(), "0.1.0");
    }

    #[test]
    fn fire_interior_ballistics() {
        abe_init(ABE_API_VERSION, 0);

        let mut cdm = [0u8; 32];
        cdm[..3].copy_from_slice(b"g7\0");

        let params = FireParams {
            barrel_length_mm: 368.0,
            chamber_pressure_mpa: 380.0,
            caliber_mm: 5.56,
            projectile_mass_g: 4.0,
            cdm_id: cdm,
        };

        let mut result = FireResult::default();
        let ret = abe_fire(&params, &mut result);
        assert_eq!(ret, 0);
        assert!(result.muzzle_velocity_ms > 800.0);
        assert!(result.muzzle_velocity_ms < 1100.0);
        assert!(result.max_chamber_pressure_mpa > 200.0);
    }

    #[test]
    fn longer_barrel_increases_mv() {
        abe_init(ABE_API_VERSION, 0);

        let mut cdm = [0u8; 32];
        cdm[..3].copy_from_slice(b"g7\0");

        let short = FireParams {
            barrel_length_mm: 254.0,
            chamber_pressure_mpa: 380.0,
            caliber_mm: 5.56,
            projectile_mass_g: 4.0,
            cdm_id: cdm.clone(),
        };
        let long = FireParams {
            barrel_length_mm: 508.0,
            chamber_pressure_mpa: 380.0,
            caliber_mm: 5.56,
            projectile_mass_g: 4.0,
            cdm_id: cdm,
        };

        let mut s_result = FireResult::default();
        let mut l_result = FireResult::default();
        abe_fire(&short, &mut s_result);
        abe_fire(&long, &mut l_result);
        assert!(l_result.muzzle_velocity_ms > s_result.muzzle_velocity_ms);
    }

    #[test]
    fn step_moves_bullet_forward() {
        abe_init(ABE_API_VERSION, 0);

        let mut cdm = [0u8; 32];
        cdm[..3].copy_from_slice(b"g7\0");

        let params = StepParams {
            pos_x: 0.0,
            pos_y: 0.0,
            pos_z: 0.0,
            vel_x: 900.0,
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

        let mut result = BulletState::default();
        let ret = abe_step(&params, &mut result);
        assert_eq!(ret, 0);
        assert!(result.pos_x > 0.0);
        assert!(result.vel_x < 900.0);
    }

    #[test]
    fn impact_penetration_struct() {
        abe_init(ABE_API_VERSION, 0);

        let mut mat = [0u8; 32];
        mat[..10].copy_from_slice(b"steel_rha\0");
        let mut proj = [0u8; 32];
        proj[..5].copy_from_slice(b"ball\0");

        let params = ImpactParams {
            vel_x: 900.0,
            vel_y: 0.0,
            vel_z: 0.0,
            mass_g: 9.5,
            caliber_mm: 7.62,
            armor_thickness_mm: 5.0,
            armor_material: mat,
            impact_angle_deg: 0.0,
            projectile_type: proj,
        };

        let mut result = ImpactResult::default();
        let ret = abe_impact(&params, &mut result);
        assert_eq!(ret, 0);
        assert_eq!(
            result.penetrated, 1,
            "7.62x51mm at 900 m/s should penetrate 5mm RHA at 0°"
        );
    }

    // ── RVExtension string dispatch tests ────────────────────────────────────

    #[test]
    fn rv_ext_version_via_string() {
        let ver = rv_ext("version");
        assert_eq!(ver, "0.1.0");
    }

    #[test]
    fn rv_ext_health_uninitialized() {
        // Skip if already initialized by another test
        if !get_state().initialized {
            let h = rv_ext("health");
            assert_eq!(h, "0");
        }
    }

    #[test]
    fn rv_ext_init_and_health() {
        let r = rv_ext_args("init", &["1", "1"]);
        assert_eq!(r, "0");
        let h = rv_ext("health");
        assert_eq!(h, "1");
    }

    #[test]
    fn rv_ext_fire_via_string() {
        rv_ext_args("init", &["1", "0"]);

        let r = rv_ext_args("fire", &["368", "380", "5.56", "4.0", "g7"]);
        assert_ne!(r, "-1", "fire should succeed");
        assert!(r.starts_with('['), "fire result should be array: {}", r);

        // Parse: [mv, pressure, burn, time_ms]
        let trimmed = r.trim_start_matches('[').trim_end_matches(']');
        let parts: Vec<&str> = trimmed.split(',').collect();
        assert_eq!(parts.len(), 4, "fire result should have 4 fields: {}", r);
        let mv: f64 = parts[0].parse().unwrap();
        assert!(mv > 800.0 && mv < 1100.0, "MV should be in range: {}", mv);
    }

    #[test]
    fn rv_ext_fire_longer_barrel_faster() {
        rv_ext_args("init", &["1", "0"]);

        let short = rv_ext_args("fire", &["254", "380", "5.56", "4.0", "g7"]);
        let long = rv_ext_args("fire", &["508", "380", "5.56", "4.0", "g7"]);

        fn parse_mv(s: &str) -> f64 {
            s.trim_start_matches('[')
                .split(',')
                .next()
                .unwrap()
                .parse()
                .unwrap()
        }
        let mv_s = parse_mv(&short);
        let mv_l = parse_mv(&long);
        assert!(
            mv_l > mv_s,
            "Longer barrel should give higher MV: {} vs {}",
            mv_l,
            mv_s
        );
    }

    #[test]
    fn rv_ext_step_via_string() {
        rv_ext_args("init", &["1", "0"]);

        let r = rv_ext_args(
            "step",
            &[
                "0", "0", "0", // pos
                "900", "0", "0",    // vel
                "0.01", // dt
                "0", "0", "0",     // wind
                "1.225", // density
                "15",    // temp_c
                "0",     // altitude
                "g7",    // cdm
                "0.157", // bc
                "4.0",   // mass_g
                "5.56",  // caliber_mm
            ],
        );
        assert_ne!(r, "-1", "step should succeed");
        assert!(r.starts_with('['), "step result should be array: {}", r);

        let trimmed = r.trim_start_matches('[').trim_end_matches(']');
        let parts: Vec<&str> = trimmed.split(',').collect();
        assert_eq!(parts.len(), 8, "step result should have 8 fields");
        let pos_x: f64 = parts[0].parse().unwrap();
        let vel_x: f64 = parts[3].parse().unwrap();
        assert!(pos_x > 0.0, "Bullet should move forward: pos_x={}", pos_x);
        assert!(vel_x < 900.0, "Bullet should slow down: vel_x={}", vel_x);
    }

    #[test]
    fn rv_ext_impact_via_string() {
        rv_ext_args("init", &["1", "0"]);

        let r = rv_ext_args(
            "impact",
            &[
                "900",
                "0",
                "0",         // vel
                "9.5",       // mass_g
                "7.62",      // caliber_mm
                "5",         // armor_thickness_mm
                "steel_rha", // armor_material
                "0",         // impact_angle
                "ball",      // projectile_type
            ],
        );
        assert_ne!(r, "-1", "impact should succeed");
        assert!(r.starts_with('['), "impact result should be array: {}", r);

        let trimmed = r.trim_start_matches('[').trim_end_matches(']');
        let parts: Vec<&str> = trimmed.split(',').collect();
        assert_eq!(parts.len(), 9, "impact result should have 9 fields");
        let penetrated: i32 = parts[0].parse().unwrap();
        assert_eq!(penetrated, 1, "7.62mm at 900 m/s should pen 5mm RHA");
    }

    #[test]
    fn rv_ext_unknown_command() {
        let r = rv_ext("nonsense");
        assert_eq!(r, "unknown: nonsense");
    }

    #[test]
    fn rv_ext_args_unknown_command() {
        let r = rv_ext_args("nonsense", &["a"]);
        assert_eq!(r, "unknown: nonsense");
    }
}

// ── Default impls ─────────────────────────────────────────────────────────────

impl Default for FireResult {
    fn default() -> Self {
        Self {
            muzzle_velocity_ms: 0.0,
            max_chamber_pressure_mpa: 0.0,
            propellant_burn_fraction: 0.0,
            barrel_time_ms: 0.0,
        }
    }
}

impl Default for BulletState {
    fn default() -> Self {
        Self {
            pos_x: 0.0,
            pos_y: 0.0,
            pos_z: 0.0,
            vel_x: 0.0,
            vel_y: 0.0,
            vel_z: 0.0,
            mach: 0.0,
            time_s: 0.0,
        }
    }
}

impl Default for ImpactResult {
    fn default() -> Self {
        Self {
            penetrated: 0,
            residual_vel_ms: 0.0,
            energy_j: 0.0,
            effective_thickness_mm: 0.0,
            ricochet: 0,
            ricochet_angle_deg: 0.0,
            ricochet_energy_fraction: 0.0,
            fragments: 0,
            spall_fragments: 0,
        }
    }
}
