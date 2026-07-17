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
    let bc: f64 = args.get(14).and_then(|s| s.parse().ok()).unwrap_or(0.157);
    let mass_g: f64 = args.get(15).and_then(|s| s.parse().ok()).unwrap_or(4.0);
    let caliber_mm: f64 = args.get(16).and_then(|s| s.parse().ok()).unwrap_or(5.56);
    let _ = mass_g;
    let _ = caliber_mm;

    let speed = (vel_x.powi(2) + vel_y.powi(2) + vel_z.powi(2)).sqrt();
    let mach = exterior::calc_mach(speed, temp_c);
    let cd = drag::get_cd(cdm_id, mach);

    let air_density = if altitude_m > 0.0 && (temp_c - 15.0).abs() < 0.1 {
        atmosphere::density_from_altitude(altitude_m, temp_c)
    } else {
        density
    };

    // BC-based drag: a = 0.5 * ρ * v² * Cd / (BC * K)
    // K converts from BC in lb/in² to SI (kg/m²) and includes the π/4
    // cross-sectional area factor from a = 0.5·ρ·v²·Cd·(π·d²/4)/(BC_SI·m):
    //   K = (kg/lb) / (m/in)² * (4/π) = 0.453592 / 0.0254² * 4/π ≈ 895.3
    const BC_CONV: f64 = 0.453592 / (0.0254 * 0.0254) * (4.0 / std::f64::consts::PI);
    let bc_metric = bc * BC_CONV;
    let drag_decel = if speed > 0.001 && bc_metric > 0.001 {
        0.5 * air_density * speed * speed * cd / bc_metric
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

/// Initialise the ABE extension runtime.
///
/// Must be called exactly once before any other `abe_*` function.
/// Uses `OnceLock` internally, so duplicate calls are idempotent
/// (the first call's state is preserved).
///
/// # Arguments
/// * `api_version` — Expected ABE API version (`ABE_API_VERSION`, currently 1).
///   Returns -1 if this does not match the compiled version, indicating an
///   SQF/Rust version mismatch.
/// * `ace_present` — Non-zero if ACE3 is loaded in the current mission
///   environment. Controls whether ABE operates in standalone or ACE3
///   enhanced mode.
///
/// # Returns
/// 0 on success, -1 on version mismatch.
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

/// Return the ABE extension version as a null-terminated C string.
///
/// Format: `"MAJOR.MINOR.PATCH"` (semver). The returned pointer points
/// to a static `OnceLock<CString>` and remains valid for the lifetime of
/// the extension. The caller must not free it.
///
/// This function does not require initialisation and is safe to call
/// before `abe_init`.
#[unsafe(no_mangle)]
pub extern "C" fn abe_version() -> *const c_char {
    static VERSION: OnceLock<CString> = OnceLock::new();
    VERSION
        .get_or_init(|| CString::new(ABE_VERSION).unwrap())
        .as_ptr()
}

/// Input parameters for [`abe_fire`].
///
/// Describes the weapon and projectile combination for interior
/// ballistics computation. All dimensional values are in SI-related
/// units (mm, g, MPa) for convenient SQF interop.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FireParams {
    /// Barrel length in millimetres (e.g. 368.0 for an M4).
    pub barrel_length_mm: f64,

    /// Peak chamber pressure in MPa (SAAMI/CIP standard value).
    pub chamber_pressure_mpa: f64,

    /// Projectile diameter / bore calibre in millimetres.
    pub caliber_mm: f64,

    /// Projectile mass in grams.
    pub projectile_mass_g: f64,

    /// Drag model identifier as a null-terminated ASCII string
    /// (e.g. `b"g7\0"`, `b"g1\0"`, `b"g8\0"`). Padded to 32 bytes.
    pub cdm_id: [u8; 32],
}

/// Output from [`abe_fire`].
///
/// Contains the computed muzzle velocity, pressure, and barrel-time
/// estimates from the interior ballistics model.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FireResult {
    /// Computed muzzle velocity in m/s.
    pub muzzle_velocity_ms: f64,

    /// Peak chamber pressure in MPa (input value passed through;
    /// the model does not independently compute pressure).
    pub max_chamber_pressure_mpa: f64,

    /// Estimated fraction of propellant burned at projectile exit
    /// (range 0.25–1.0).
    pub propellant_burn_fraction: f64,

    /// Estimated barrel time from ignition to exit in milliseconds.
    pub barrel_time_ms: f64,
}

/// Compute muzzle velocity and interior ballistics for a given weapon and
/// projectile combination.
///
/// Uses a two-zone gas-expansion pressure curve model. Pressure rises as
/// propellant ignites (peak at ~12 % of projectile travel), then decays
/// exponentially as the projectile moves down the bore. The work integral
/// of bore pressure times area along the barrel gives kinetic energy,
/// reduced by friction, heat transfer, and rifling engraving losses.
///
/// # Input fields (FireParams)
/// * `barrel_length_mm` — barrel length in millimetres.
/// * `chamber_pressure_mpa` — peak chamber pressure in MPa (SAAMI/CIP
///   standard).
/// * `caliber_mm` — projectile diameter in millimetres.
/// * `projectile_mass_g` — projectile mass in grams.
/// * `cdm_id` — drag model identifier (currently unused in interior
///   ballistics, reserved for future coupled interior/exterior models).
///
/// # Output fields (FireResult)
/// * `muzzle_velocity_ms` — computed muzzle velocity in m/s.
/// * `max_chamber_pressure_mpa` — peak chamber pressure (input value
///   passed through, model does not independently compute pressure).
/// * `propellant_burn_fraction` — estimated fraction of propellant
///   burned at projectile exit (0.25–1.0).
/// * `barrel_time_ms` — estimated time from ignition to exit in
///   milliseconds.
///
/// # Validation
/// Returns -1 if the extension is not initialised, or if any input
/// dimension is zero or negative. Returns 0 on success.
///
/// # Thread safety
/// Pure function, no mutable global state. Safe to call concurrently.
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

/// Output from [`abe_step`].
///
/// Contains the updated projectile position, velocity, Mach number,
/// and step time after one integration step.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct BulletState {
    /// New x-position in ARMA 3 world coordinates (metres).
    pub pos_x: f64,
    /// New y-position (metres).
    pub pos_y: f64,
    /// New z-position — ABE uses +z for downward (gravity) direction.
    pub pos_z: f64,
    /// New x-velocity component after drag, gravity, and wind (m/s).
    pub vel_x: f64,
    /// New y-velocity component (m/s).
    pub vel_y: f64,
    /// New z-velocity component (m/s).
    pub vel_z: f64,
    /// Mach number at the new speed and current temperature.
    pub mach: f64,
    /// Integration timestep (delta time in seconds); the caller
    /// accumulates total time-of-flight externally.
    pub time_s: f64,
}

/// Input parameters for [`abe_step`].
///
/// Describes the current projectile state, environment, and
/// projectile properties for one semi-implicit Euler integration
/// step.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct StepParams {
    /// Current x-position (metres).
    pub pos_x: f64,
    /// Current y-position (metres).
    pub pos_y: f64,
    /// Current z-position (metres, +z = downward).
    pub pos_z: f64,
    /// Current x-velocity (m/s).
    pub vel_x: f64,
    /// Current y-velocity (m/s).
    pub vel_y: f64,
    /// Current z-velocity (m/s).
    pub vel_z: f64,
    /// Integration timestep in seconds (typically 0.01–0.1).
    pub dt_s: f64,
    /// Wind velocity x-component at reference height (m/s).
    pub wind_x: f64,
    /// Wind velocity y-component (crosswind) at reference height (m/s).
    pub wind_y: f64,
    /// Wind velocity z-component at reference height (m/s).
    pub wind_z: f64,
    /// Air density in kg/m³ (overridden when `altitude_m > 0` and
    /// `temp_c ≈ 15` triggers ICAO atmosphere lookup).
    pub density_kgm3: f64,
    /// Air temperature in degrees Celsius.
    pub temp_c: f64,
    /// Altitude above sea level in metres (0 = sea level).
    pub altitude_m: f64,
    /// Drag model identifier as a null-terminated ASCII string
    /// (e.g. `b"g7\0"`, `b"g1\0"`, `b"g8\0"`). Padded to 32 bytes.
    pub cdm_id: [u8; 32],
    /// Ballistic coefficient in lb/in² (G1 or G7 standard). The
    /// extension converts internally to SI via `K ≈ 895.3`.
    pub bc: f64,
    /// Projectile mass in grams (reserved for future use).
    pub mass_g: f64,
    /// Projectile calibre in millimetres (reserved for future use).
    pub caliber_mm: f64,
}

/// Step a projectile forward by `dt_s` seconds using semi-implicit Euler
/// integration.
///
/// The velocity is updated first (drag, gravity, wind), then the position
/// is advanced using the new velocity. This gives first-order accuracy
/// with better energy behaviour than explicit Euler.
///
/// # Physics applied
/// * Drag deceleration: `0.5 * rho * v^2 * Cd / (BC * K)` where
///   `K = 0.453592 / 0.0254^2 * 4/pi ~ 895.3` converts BC from imperial
///   (lb/in^2) to SI (kg/m^2) including cross-sectional area.
/// * Gravity: constant `g = 9.80665 m/s^2` in +z direction.
/// * Wind: relative velocity subtracted, scaled by altitude-dependent
///   wind shear factor (log-wind-profile, surface layer 0–200 m).
/// * Air density: from altitude via ICAO atmosphere when `altitude_m > 0`
///   and temperature is near ISA (15 deg C); otherwise uses the provided
///   `density_kgm3`.
/// * Drag coefficient: linear interpolation over JBM/ABRA lookup tables
///   (G1, G7, G8) keyed by Mach number.
///
/// # Input fields (StepParams)
/// * `pos_x/y/z` — projectile position in metres (ARMA 3 world coords).
/// * `vel_x/y/z` — projectile velocity in m/s.
/// * `dt_s` — integration timestep in seconds (typically 0.01–0.1).
/// * `wind_x/y/z` — wind velocity in m/s at reference height.
/// * `density_kgm3` — air density in kg/m^3 (overridden when altitude
///   triggers ICAO lookup).
/// * `temp_c` — air temperature in Celsius.
/// * `altitude_m` — altitude ASL in metres.
/// * `cdm_id` — drag model identifier (G1, G7, G8, or custom).
/// * `bc` — ballistic coefficient in lb/in^2 (G1 or G7 standard).
/// * `mass_g` — projectile mass in grams (reserved for future use).
/// * `caliber_mm` — projectile diameter in mm (reserved for future use).
///
/// # Output fields (BulletState)
/// * `pos_x/y/z` — new position after `dt_s`.
/// * `vel_x/y/z` — new velocity after drag/gravity/wind.
/// * `mach` — Mach number at the new speed and temperature.
/// * `time_s` — delta time (`dt_s`); the caller accumulates total TOF.
///
/// # Validation
/// Returns -1 if the extension is not initialised (safety guard).
/// Returns 0 on success. The caller should check that velocity remains
/// positive and position is within the expected domain.
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

    // Apply BC-based drag: a = 0.5 * ρ * v² * Cd / (BC * K)
    // K includes π/4 area factor: 0.453592 / 0.0254² * 4/π ≈ 895.3
    let speed = (params.vel_x.powi(2) + params.vel_y.powi(2) + params.vel_z.powi(2)).sqrt();
    const BC_CONV: f64 = 0.453592 / (0.0254 * 0.0254) * (4.0 / std::f64::consts::PI);
    let bc_metric = params.bc * BC_CONV;
    let drag_decel = if speed > 0.001 && bc_metric > 0.001 {
        0.5 * density * speed * speed * cd / bc_metric
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

/// Input parameters for [`abe_impact`].
///
/// Describes a projectile impact against an armour plate for
/// terminal ballistics evaluation.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ImpactParams {
    /// Impact velocity x-component (m/s).
    pub vel_x: f64,
    /// Impact velocity y-component (m/s).
    pub vel_y: f64,
    /// Impact velocity z-component (m/s).
    pub vel_z: f64,
    /// Projectile mass in grams.
    pub mass_g: f64,
    /// Projectile calibre in millimetres.
    pub caliber_mm: f64,
    /// Armour plate thickness in millimetres.
    pub armor_thickness_mm: f64,
    /// Armour material identifier as a null-terminated ASCII string
    /// (e.g. `b"steel_rha\0"`, `b"aluminum_5083\0"`, `b"ceramic_b4c\0"`).
    /// Padded to 32 bytes.
    pub armor_material: [u8; 32],
    /// Impact angle from surface normal in degrees
    /// (0 = perpendicular, 90 = grazing).
    pub impact_angle_deg: f64,
    /// Projectile type identifier as a null-terminated ASCII string
    /// (e.g. `b"ball\0"`, `b"ap\0"`, `b"apds\0"`, `b"soft_point\0"`).
    /// Padded to 32 bytes.
    pub projectile_type: [u8; 32],
}

/// Output from [`abe_impact`].
///
/// Contains the terminal effects of a projectile impact against
/// armour: penetration status, residual velocity, ricochet
/// information, and fragmentation counts.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ImpactResult {
    /// 1 if the plate was perforated, 0 otherwise.
    pub penetrated: i32,
    /// Projectile velocity remaining after penetrating the plate (m/s).
    pub residual_vel_ms: f64,
    /// Impact kinetic energy (joules).
    pub energy_j: f64,
    /// Effective armour thickness the projectile had to defeat after
    /// angle and material scaling (millimetres).
    pub effective_thickness_mm: f64,
    /// 1 if the projectile ricocheted, 0 otherwise.
    pub ricochet: i32,
    /// Outgoing ricochet angle relative to the surface (degrees).
    pub ricochet_angle_deg: f64,
    /// Fraction of kinetic energy retained after ricochet (0.0–1.0).
    pub ricochet_energy_fraction: f64,
    /// Number of projectile fragments generated.
    pub fragments: i32,
    /// Number of armour spall fragments generated.
    pub spall_fragments: i32,
}

/// Evaluate the terminal effects of a projectile impact against an armour
/// plate: penetration, ricochet, spall, and fragmentation.
///
/// Uses a three-stage model:
/// 1. Ricochet check — if the impact angle exceeds the velocity- and
///    calibre-dependent threshold, the projectile ricochets. Energy
///    retention is scaled by angle (85 % at glancing, 50 % near the
///    threshold).
/// 2. Effective thickness — plate thickness divided by cos(angle) and
///    scaled by the material factor (RHA = 1.0, HHA = 1.25, aluminium
///    ~0.35–0.45, ceramic ~2.5–3.5) plus a calibre-to-thickness ratio
///    correction.
/// 3. De Marre penetration — threshold velocity solved from
///    `V_req = k * D^0.75 * T^0.7 / M^0.5`. On penetration, residual
///    velocity is `sqrt(V^2 - V_req^2)`.
///
/// Fragmentation is delegated to `fragmentation::evaluate()`. Spall
/// count scales with effective thickness and velocity.
///
/// # Input fields (ImpactParams)
/// * `vel_x/y/z` — impact velocity vector in m/s.
/// * `mass_g` — projectile mass in grams.
/// * `caliber_mm` — projectile diameter in mm.
/// * `armor_thickness_mm` — armour plate thickness in mm.
/// * `armor_material` — material identifier (e.g. "steel_rha",
///   "aluminum_5083", "ceramic_b4c").
/// * `impact_angle_deg` — angle from surface normal in degrees
///   (0 = perpendicular, 90 = grazing).
/// * `projectile_type` — projectile construction identifier
///   (e.g. "ball", "ap", "apds", "soft_point").
///
/// # Output fields (ImpactResult)
/// * `penetrated` — 1 if the plate was perforated, 0 otherwise.
/// * `residual_vel_ms` — projectile velocity after penetrating (m/s).
/// * `energy_j` — impact kinetic energy (J).
/// * `effective_thickness_mm` — the thickness the projectile had to
///   defeat after angle and material scaling.
/// * `ricochet` — 1 if the projectile ricocheted, 0 otherwise.
/// * `ricochet_angle_deg` — outgoing ricochet angle relative to the
///   surface.
/// * `ricochet_energy_fraction` — fraction of energy retained after
///   ricochet (0.0–1.0).
/// * `fragments` — number of projectile fragments generated.
/// * `spall_fragments` — number of armour spall fragments.
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

/// Return 1 if the extension has been initialised and is ready for use, 0
/// otherwise.
///
/// "Initialised" means that `abe_init` was called with a matching API
/// version and the global `OnceLock<AbeState>` has been set. This does
/// not guarantee data files are loaded (data loading is lazy or handled
/// by the SQF layer).
///
/// Safe to call before `abe_init` (will return 0).
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

    // ── Trajectory integration validation ──────────────────────────────────────
    // Runs a full trajectory loop through abe_step and samples at key ranges.
    // These values can be compared against py-ballisticcalc and ballistics-engine.

    const SAMPLE_RANGES: [f64; 7] = [0.0, 100.0, 200.0, 300.0, 500.0, 800.0, 1000.0];

    fn run_trajectory(
        mv_ms: f64,
        bc: f64,
        mass_g: f64,
        caliber_mm: f64,
        cdm: &str,
        dt_s: f64,
    ) -> Vec<(f64, f64, f64, f64)> {
        // ABE physics: bullet flies along +x, gravity acts on +z,
        // so drop = z, lateral = y (always 0 in this setup)
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

        // Initial sample at range 0: (range_m, drop_m, speed_ms, time_s)
        samples.push((x, z, mv_ms, t));

        while x < 1050.0 && vx > 50.0 && next_range_idx < SAMPLE_RANGES.len() {
            let step = StepParams {
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

    #[test]
    fn trajectory_m855_at_930ms() {
        abe_init(ABE_API_VERSION, 0);
        let samples = run_trajectory(930.0, 0.157, 4.0, 5.56, "g7", 0.01);

        // Expected: at 500m, drop ~+2m (gravity is +z in ABE); at 1000m drop ~+16m
        // These are sanity checks — not exact comparisons yet
        // (the reference libs disagree by ~0.5m at 500m)
        for &(x_pt, drop, v, _t) in &samples {
            let x_rounded = (x_pt / 50.0).round() * 50.0;
            if x_rounded == 500.0 {
                assert!(
                    drop > 0.0 && drop < 4.0,
                    "M855 at 500m: drop should be ~+2m, got {}",
                    drop
                );
                assert!(
                    v > 400.0 && v < 600.0,
                    "M855 at 500m: v should be ~480, got {}",
                    v
                );
            }
            if x_rounded == 1000.0 {
                assert!(
                    drop > 10.0 && drop < 22.0,
                    "M855 at 1000m: drop should be ~+16m, got {}",
                    drop
                );
                assert!(
                    v > 200.0 && v < 350.0,
                    "M855 at 1000m: v should be ~277, got {}",
                    v
                );
            }
        }
    }

    // ── State management tests ──────────────────────────────────────────────

    #[test]
    fn init_twice_returns_ok() {
        // First init always succeeds
        let r1 = rv_ext_args("init", &["1", "0"]);
        assert_eq!(r1, "0");
        // Second init: OnceLock::set returns Err (discarded), handle_init still returns "0"
        let r2 = rv_ext_args("init", &["1", "0"]);
        assert_eq!(r2, "0", "calling init twice should still return success");
    }

    #[test]
    fn health_before_init_returns_0() {
        if !get_state().initialized {
            let h = rv_ext("health");
            assert_eq!(h, "0");
        }
    }

    // ── Fire command edge cases ─────────────────────────────────────────────

    #[test]
    fn rv_ext_fire_zero_barrel() {
        rv_ext_args("init", &["1", "0"]);
        let r = rv_ext_args("fire", &["0", "380", "5.56", "4.0", "g7"]);
        assert_eq!(r, "-1", "barrel_length=0 should fail");
    }

    #[test]
    fn rv_ext_fire_negative_pressure() {
        rv_ext_args("init", &["1", "0"]);
        let r = rv_ext_args("fire", &["368", "-10", "5.56", "4.0", "g7"]);
        assert_eq!(r, "-1", "negative chamber pressure should fail");
    }

    #[test]
    fn rv_ext_fire_missing_args() {
        rv_ext_args("init", &["1", "0"]);
        // Only 2 args: barrel_length + chamber_pressure
        // caliber and mass default to 0 → calc_muzzle_velocity returns None
        let r = rv_ext_args("fire", &["368", "380"]);
        assert_eq!(r, "-1", "missing caliber/mass should fail gracefully");
    }

    #[test]
    fn rv_ext_fire_empty_cdm() {
        rv_ext_args("init", &["1", "0"]);
        // Empty cdm string — calc_muzzle_velocity takes _cdm_id (unused)
        // So fire should still succeed with valid numeric args
        let r = rv_ext_args("fire", &["368", "380", "5.56", "4.0", ""]);
        assert_ne!(
            r, "-1",
            "empty cdm should still succeed (cdm unused in interior)"
        );
        assert!(r.starts_with('['), "should return valid array: {}", r);
    }

    #[test]
    fn rv_ext_fire_non_numeric() {
        rv_ext_args("init", &["1", "0"]);
        // All positional args parse-fail → defaults to 0 → calc returns None
        let r = rv_ext_args("fire", &["abc", "def", "xyz", "ghi", "g7"]);
        assert_eq!(r, "-1", "non-numeric args should fail gracefully");
    }

    // ── Step command edge cases ─────────────────────────────────────────────

    #[test]
    fn rv_ext_step_zero_dt() {
        rv_ext_args("init", &["1", "0"]);
        let r = rv_ext_args(
            "step",
            &[
                "0", "0", "0", "900", "0", "0", "0", // dt = 0
                "0", "0", "0", "1.225", "15", "0", "g7", "0.157", "4.0", "5.56",
            ],
        );
        assert_ne!(r, "-1", "step with zero dt should succeed");
        let trimmed = r.trim_start_matches('[').trim_end_matches(']');
        let parts: Vec<&str> = trimmed.split(',').collect();
        let pos_x: f64 = parts[0].parse().unwrap();
        assert!(
            (pos_x).abs() < 0.001,
            "pos_x should be ~0 with dt=0: {}",
            pos_x
        );
    }

    #[test]
    fn rv_ext_step_negative_dt() {
        rv_ext_args("init", &["1", "0"]);
        let r = rv_ext_args(
            "step",
            &[
                "100", "0", "0", "900", "0", "0", "-0.01", // negative dt
                "0", "0", "0", "1.225", "15", "0", "g7", "0.157", "4.0", "5.56",
            ],
        );
        assert_ne!(r, "-1", "step with negative dt should not crash");
    }

    #[test]
    fn rv_ext_step_extreme_speed() {
        rv_ext_args("init", &["1", "0"]);
        // M=5 at 15°C → ~1700 m/s. Should produce valid output.
        let r = rv_ext_args(
            "step",
            &[
                "0", "0", "0", "1700", "0", "0", "0.01", "0", "0", "0", "1.225", "15", "0", "g7",
                "0.157", "4.0", "5.56",
            ],
        );
        assert_ne!(r, "-1", "step at M=5 should succeed");
        assert!(r.starts_with('['), "should return valid array: {}", r);
        let trimmed = r.trim_start_matches('[').trim_end_matches(']');
        let parts: Vec<&str> = trimmed.split(',').collect();
        assert_eq!(parts.len(), 8);
        let pos_x: f64 = parts[0].parse().unwrap();
        assert!(
            pos_x > 0.0,
            "supersonic bullet should move forward: {}",
            pos_x
        );
    }

    #[test]
    fn rv_ext_step_stationary() {
        rv_ext_args("init", &["1", "0"]);
        // v=0, should fall straight down (gravity only, drag_decel = 0)
        let r = rv_ext_args(
            "step",
            &[
                "0", "0", "0", "0", "0", "0", // v=0
                "0.01", "0", "0", "0", "1.225", "15", "0", "g7", "0.157", "4.0", "5.56",
            ],
        );
        assert_ne!(r, "-1", "step with v=0 should succeed");
        let trimmed = r.trim_start_matches('[').trim_end_matches(']');
        let parts: Vec<&str> = trimmed.split(',').collect();
        let pos_z: f64 = parts[2].parse().unwrap();
        let vel_z: f64 = parts[5].parse().unwrap();
        assert!(pos_z > 0.0, "bullet should fall: pos_z={}", pos_z);
        assert!(
            vel_z > 0.0,
            "bullet should have downward velocity: vel_z={}",
            vel_z
        );
    }

    #[test]
    fn rv_ext_step_missing_wind() {
        rv_ext_args("init", &["1", "0"]);
        // Only 10 args (pos + vel + dt + wind). No density/temp/altitude/cdm/bc/mass/caliber.
        // All missing fields get defaults via unwrap_or, should not crash.
        let r = rv_ext_args(
            "step",
            &["0", "0", "0", "900", "0", "0", "0.01", "0", "0", "0"],
        );
        assert_ne!(r, "-1", "step with 10 args should not crash");
        assert!(r.starts_with('['), "should return array: {}", r);
        let trimmed = r.trim_start_matches('[').trim_end_matches(']');
        let parts: Vec<&str> = trimmed.split(',').collect();
        assert_eq!(parts.len(), 8);
    }

    #[test]
    fn rv_ext_step_at_altitude() {
        rv_ext_args("init", &["1", "0"]);
        // altitude=5000, temp=15.0 → triggers density_from_altitude(5000, 15.0)
        let r = rv_ext_args(
            "step",
            &[
                "0", "0", "0", "900", "0", "0", "0.1", "0", "0", "0",
                "1.225", // density (ignored when altitude>0 and temp≈15)
                "15.0",  // temp (close enough to 15 to trigger ISA density)
                "5000",  // altitude (m)
                "g7", "0.157", "4.0", "5.56",
            ],
        );
        assert_ne!(r, "-1", "step at altitude should succeed");
        let trimmed = r.trim_start_matches('[').trim_end_matches(']');
        let parts: Vec<&str> = trimmed.split(',').collect();
        let vel_x: f64 = parts[3].parse().unwrap();
        assert!(
            vel_x > 0.0,
            "bullet should move forward at altitude: {}",
            vel_x
        );
    }

    // ── Impact command edge cases ───────────────────────────────────────────

    #[test]
    fn rv_ext_impact_thick_armor() {
        rv_ext_args("init", &["1", "0"]);
        // 7.62mm ball at 900 m/s vs 50mm RHA → should NOT penetrate
        let r = rv_ext_args(
            "impact",
            &[
                "900",
                "0",
                "0",
                "9.5",
                "7.62",
                "50",
                "steel_rha",
                "0",
                "ball",
            ],
        );
        assert_ne!(r, "-1");
        let trimmed = r.trim_start_matches('[').trim_end_matches(']');
        let parts: Vec<&str> = trimmed.split(',').collect();
        let penetrated: i32 = parts[0].parse().unwrap();
        assert_eq!(penetrated, 0, "7.62mm ball should NOT pen 50mm RHA");
    }

    #[test]
    fn rv_ext_impact_grazing_angle() {
        rv_ext_args("init", &["1", "0"]);
        // 7.62mm ball at 900 m/s vs 10mm RHA at 85° → ricochet
        let r = rv_ext_args(
            "impact",
            &[
                "900",
                "0",
                "0",
                "9.5",
                "7.62",
                "10",
                "steel_rha",
                "85", // shallow angle
                "ball",
            ],
        );
        assert_ne!(r, "-1");
        let trimmed = r.trim_start_matches('[').trim_end_matches(']');
        let parts: Vec<&str> = trimmed.split(',').collect();
        let ricochet: i32 = parts[4].parse().unwrap();
        assert_eq!(ricochet, 1, "85° angle should cause ricochet");
    }

    #[test]
    fn rv_ext_impact_zero_mass() {
        rv_ext_args("init", &["1", "0"]);
        // mass=0 → evaluate guards against zero mass (v_required = INF)
        let r = rv_ext_args(
            "impact",
            &[
                "900",
                "0",
                "0",
                "0", // mass = 0
                "7.62",
                "5",
                "steel_rha",
                "0",
                "ball",
            ],
        );
        assert_ne!(r, "-1", "impact with zero mass should not crash");
    }

    #[test]
    fn rv_ext_impact_ap_projectile() {
        rv_ext_args("init", &["1", "0"]);
        // AP has projectile_modifier=1.3 vs ball=1.0 → penetration threshold is lower
        let ball = rv_ext_args(
            "impact",
            &[
                "880",
                "0",
                "0",
                "9.5",
                "7.62",
                "10",
                "steel_rha",
                "0",
                "ball",
            ],
        );
        let ap = rv_ext_args(
            "impact",
            &["880", "0", "0", "9.5", "7.62", "10", "steel_rha", "0", "ap"],
        );
        assert_ne!(ball, "-1");
        assert_ne!(ap, "-1");

        let parse_pen = |s: &str| -> i32 {
            s.trim_start_matches('[')
                .split(',')
                .next()
                .unwrap()
                .parse()
                .unwrap()
        };
        let ball_pen = parse_pen(&ball);
        let ap_pen = parse_pen(&ap);
        assert!(
            ap_pen >= ball_pen,
            "AP should pen as well or better than ball (AP={}, ball={})",
            ap_pen,
            ball_pen
        );
    }

    #[test]
    fn rv_ext_impact_unknown_material() {
        rv_ext_args("init", &["1", "0"]);
        // Unknown material → material_factor defaults to 1.0 (RHA equivalent)
        let r = rv_ext_args(
            "impact",
            &[
                "900",
                "0",
                "0",
                "9.5",
                "7.62",
                "5",
                "nonexistent_material",
                "0",
                "ball",
            ],
        );
        assert_ne!(r, "-1");
        let trimmed = r.trim_start_matches('[').trim_end_matches(']');
        let parts: Vec<&str> = trimmed.split(',').collect();
        let penetrated: i32 = parts[0].parse().unwrap();
        assert_eq!(
            penetrated, 1,
            "unknown material should default to RHA and pen 5mm"
        );
    }

    // ── String command edge cases ───────────────────────────────────────────

    #[test]
    fn rv_ext_empty_string() {
        let r = rv_ext("");
        assert_eq!(r, "unknown: ", "empty command should return unknown");
    }

    #[test]
    fn rv_ext_very_long_command() {
        let long_cmd = "a".repeat(500);
        let r = rv_ext(&long_cmd);
        assert!(r.starts_with("unknown: "));
        // Output buffer is 2048, but our result is "unknown: " + 500 chars = ~509
        assert!(r.len() <= OUTPUT_BUF_SIZE, "output should fit in buffer");
    }

    // ── Pipeline tests: fire → step → impact ──────────────────────────────

    #[test]
    fn fire_step_impact_pipeline() {
        abe_init(ABE_API_VERSION, 0);

        let mut cdm = [0u8; 32];
        cdm[..3].copy_from_slice(b"g7\0");

        // 1. Fire with 368mm barrel, 380 MPa, 5.56mm, 4.0g
        let fire = FireParams {
            barrel_length_mm: 368.0,
            chamber_pressure_mpa: 380.0,
            caliber_mm: 5.56,
            projectile_mass_g: 4.0,
            cdm_id: cdm,
        };
        let mut fr = FireResult::default();
        assert_eq!(abe_fire(&fire, &mut fr), 0);
        let mv = fr.muzzle_velocity_ms;
        assert!(mv > 800.0 && mv < 1100.0, "MV should be reasonable: {mv}");

        // 2. Step 200 times (2 s at dt = 0.01)
        let mut x = 0.0;
        let mut y = 0.0;
        let mut z = 0.0;
        let mut vx = mv;
        let mut vy = 0.0;
        let mut vz = 0.0;

        for _ in 0..200 {
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
            assert_eq!(abe_step(&step, &mut sr), 0);
            x = sr.pos_x;
            y = sr.pos_y;
            z = sr.pos_z;
            vx = sr.vel_x;
            vy = sr.vel_y;
            vz = sr.vel_z;
        }

        // 3. Impact the bullet at its final velocity against 3mm RHA
        let mut mat = [0u8; 32];
        mat[..10].copy_from_slice(b"steel_rha\0");
        let mut proj = [0u8; 32];
        proj[..5].copy_from_slice(b"ball\0");

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
        assert_eq!(abe_impact(&impact, &mut ir), 0);
        assert!(
            ir.residual_vel_ms > 0.0,
            "subsonic 5.56mm at 3mm RHA should retain some energy: rv={}",
            ir.residual_vel_ms
        );
    }

    #[test]
    fn rv_ext_fire_step_impact_pipeline() {
        rv_ext_args("init", &["1", "0"]);

        // 1. Fire via string ABI
        let fire = rv_ext_args("fire", &["508", "380", "5.56", "4.0", "g7"]);
        assert_ne!(fire, "-1");
        let trimmed = fire.trim_start_matches('[').trim_end_matches(']');
        let parts: Vec<&str> = trimmed.split(',').collect();
        let mv: f64 = match parts.first().and_then(|s| s.parse().ok()) {
            Some(v) => v,
            None => panic!("fire result should have numeric MV: {fire}"),
        };

        // 2. Step 200 times via string ABI
        let mut x = 0.0_f64;
        let mut y = 0.0_f64;
        let mut z = 0.0_f64;
        let mut vx = mv;
        let mut vy = 0.0;
        let mut vz = 0.0;

        for _ in 0..200 {
            let s = format!("{x},{y},{z},{vx},{vy},{vz},0.01,0,0,0,1.225,15,0,g7,0.157,4.0,5.56");
            let args: Vec<&str> = s.split(',').collect();
            let r = rv_ext_args("step", &args);
            assert_ne!(r, "-1");
            let trimmed = r.trim_start_matches('[').trim_end_matches(']');
            let parts: Vec<&str> = trimmed.split(',').collect();
            x = parts[0].parse().unwrap();
            y = parts[1].parse().unwrap();
            z = parts[2].parse().unwrap();
            vx = parts[3].parse().unwrap();
            vy = parts[4].parse().unwrap();
            vz = parts[5].parse().unwrap();
        }

        // 3. Impact via string ABI
        let impact = rv_ext_args(
            "impact",
            &[
                &format!("{vx:.1}"),
                &format!("{vy:.1}"),
                &format!("{vz:.1}"),
                "4.0",
                "5.56",
                "3",
                "steel_rha",
                "0",
                "ball",
            ],
        );
        assert_ne!(impact, "-1");
        let trimmed = impact.trim_start_matches('[').trim_end_matches(']');
        let parts: Vec<&str> = trimmed.split(',').collect();
        let residual_vel: f64 = parts[1].parse().unwrap();
        assert!(
            residual_vel > 0.0,
            "should retain some energy: rv={residual_vel}"
        );
    }

    // ── Multi-bullet interleaving (ACE3 pattern) ──────────────────────────

    #[test]
    fn multi_bullet_interleaved() {
        abe_init(ABE_API_VERSION, 0);

        let mut cdm = [0u8; 32];
        cdm[..3].copy_from_slice(b"g7\0");

        // Bullet A: fast M855-like (930 m/s, 0.157 G7)
        // Bullet B: slower subsonic pistol-like (320 m/s, 0.075 G7)
        let (mut ax, mut ay, mut az, mut avx, mut avy, mut avz) = (0.0, 0.0, 0.0, 930.0, 0.0, 0.0);
        let (mut bx, mut by, mut bz, mut bvx, mut bvy, mut bvz) = (0.0, 0.0, 0.0, 320.0, 0.0, 0.0);

        let base = StepParams {
            pos_x: 0.0,
            pos_y: 0.0,
            pos_z: 0.0,
            vel_x: 0.0,
            vel_y: 0.0,
            vel_z: 0.0,
            wind_x: 0.0,
            wind_y: 0.0,
            wind_z: 0.0,
            density_kgm3: 1.225,
            temp_c: 15.0,
            altitude_m: 0.0,
            cdm_id: cdm,
            mass_g: 4.0,
            caliber_mm: 5.56,
            dt_s: 0.02,
            bc: 0.157,
        };

        // Interleave A×5, B×5 for 8 rounds = 40 steps each
        for round in 0..8 {
            let bullet_a = round % 2 == 0;
            let (x, y, z, vx, vy, vz, bc, label) = if bullet_a {
                (
                    &mut ax, &mut ay, &mut az, &mut avx, &mut avy, &mut avz, 0.157, "A",
                )
            } else {
                (
                    &mut bx, &mut by, &mut bz, &mut bvx, &mut bvy, &mut bvz, 0.075, "B",
                )
            };

            for _ in 0..5 {
                let step = StepParams {
                    pos_x: *x,
                    pos_y: *y,
                    pos_z: *z,
                    vel_x: *vx,
                    vel_y: *vy,
                    vel_z: *vz,
                    dt_s: base.dt_s,
                    wind_x: base.wind_x,
                    wind_y: base.wind_y,
                    wind_z: base.wind_z,
                    density_kgm3: base.density_kgm3,
                    temp_c: base.temp_c,
                    altitude_m: base.altitude_m,
                    cdm_id: base.cdm_id,
                    bc,
                    mass_g: base.mass_g,
                    caliber_mm: base.caliber_mm,
                };
                let mut result = BulletState::default();
                assert_eq!(abe_step(&step, &mut result), 0);
                *x = result.pos_x;
                *y = result.pos_y;
                *z = result.pos_z;
                *vx = result.vel_x;
                *vy = result.vel_y;
                *vz = result.vel_z;
            }

            assert!(*x > 0.0, "Bullet {label} should move forward: x={}", *x);
        }

        // After 40 interleaved steps each, faster bullet A should be further
        assert!(
            ax > bx,
            "Faster bullet should lead: A.x={ax:.1} B.x={bx:.1}"
        );
    }

    // ── Wind / drift tests ──────────────────────────────────────────────────

    #[test]
    fn crosswind_deflects_bullet() {
        abe_init(ABE_API_VERSION, 0);

        let mut cdm = [0u8; 32];
        cdm[..3].copy_from_slice(b"g7\0");

        let run_wind = |wind_y: f64| -> f64 {
            let mut x = 0.0;
            let mut y = 0.0;
            let mut z = 0.0;
            let mut vx = 930.0;
            let mut vy = 0.0;
            let mut vz = 0.0;

            for _ in 0..50 {
                let step = StepParams {
                    pos_x: x,
                    pos_y: y,
                    pos_z: z,
                    vel_x: vx,
                    vel_y: vy,
                    vel_z: vz,
                    dt_s: 0.1,
                    wind_x: 0.0,
                    wind_y,
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
                abe_step(&step, &mut result);
                x = result.pos_x;
                y = result.pos_y;
                z = result.pos_z;
                vx = result.vel_x;
                vy = result.vel_y;
                vz = result.vel_z;
            }
            y
        };

        let y_nowind = run_wind(0.0);
        let y_cross = run_wind(5.0);

        assert!(
            y_nowind.abs() < 0.001,
            "Without crosswind, y should be ~0: got {y_nowind}"
        );
        assert!(
            (y_cross - y_nowind).abs() > 0.1,
            "Crosswind should deflect bullet: nowind={y_nowind}, wind={y_cross}"
        );
    }

    // ── Trajectory quality tests ────────────────────────────────────────────

    #[test]
    fn trajectory_energy_conservation() {
        abe_init(ABE_API_VERSION, 0);

        let mut cdm = [0u8; 32];
        cdm[..3].copy_from_slice(b"g7\0");

        let mass_g = 4.0;
        let mut x = 0.0;
        let mut y = 0.0;
        let mut z = 0.0;
        let mut vx = 930.0;
        let mut vy = 0.0;
        let mut vz = 0.0;
        let dt = 0.01;

        let mut prev_energy: Option<f64> = None;

        for _ in 0..200 {
            let params = StepParams {
                pos_x: x,
                pos_y: y,
                pos_z: z,
                vel_x: vx,
                vel_y: vy,
                vel_z: vz,
                dt_s: dt,
                wind_x: 0.0,
                wind_y: 0.0,
                wind_z: 0.0,
                density_kgm3: 1.225,
                temp_c: 15.0,
                altitude_m: 0.0,
                cdm_id: cdm,
                bc: 0.157,
                mass_g,
                caliber_mm: 5.56,
            };
            let mut result = BulletState::default();
            assert_eq!(abe_step(&params, &mut result), 0);

            x = result.pos_x;
            y = result.pos_y;
            z = result.pos_z;
            vx = result.vel_x;
            vy = result.vel_y;
            vz = result.vel_z;

            let speed = (vx * vx + vy * vy + vz * vz).sqrt();
            // Specific total mechanical energy: KE + PE
            // ABE uses +z = down, so specific PE at height z = -g*z
            let energy = 0.5 * speed * speed - atmosphere::GRAVITY * z;

            if let Some(prev) = prev_energy {
                assert!(
                    energy <= prev + 1e-6,
                    "Total mechanical energy should not increase: prev={:.6}, now={:.6}",
                    prev,
                    energy
                );
            }
            prev_energy = Some(energy);
        }
    }

    #[test]
    fn trajectory_monotonic_position() {
        abe_init(ABE_API_VERSION, 0);

        let mut cdm = [0u8; 32];
        cdm[..3].copy_from_slice(b"g7\0");

        let mut x = 0.0;
        let mut y = 0.0;
        let mut z = 0.0;
        let mut vx = 930.0;
        let mut vy = 0.0;
        let mut vz = 0.0;

        for _ in 0..200 {
            let params = StepParams {
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
            let mut result = BulletState::default();
            abe_step(&params, &mut result);

            assert!(
                result.pos_x > x,
                "x should increase monotonically: {} -> {}",
                x,
                result.pos_x
            );
            assert!(
                result.pos_z >= z,
                "z should increase monotonically (gravity pulls down): {} -> {}",
                z,
                result.pos_z
            );

            x = result.pos_x;
            y = result.pos_y;
            z = result.pos_z;
            vx = result.vel_x;
            vy = result.vel_y;
            vz = result.vel_z;
        }
    }

    #[test]
    fn trajectory_gravity_consistency() {
        abe_init(ABE_API_VERSION, 0);

        let mut cdm = [0u8; 32];
        cdm[..3].copy_from_slice(b"g7\0");

        // Note: abe_step divides by speed without a .max() guard (handle_step has it).
        // Starting with v=0 would produce 0/0=NaN, so we use a tiny horizontal velocity.
        // With bc=0 there is no drag, so horizontal motion does not affect vertical.
        let mut x = 0.0;
        let mut y = 0.0;
        let mut z = 0.0;
        let mut vx = 1.0; // tiny horizontal velocity to avoid division-by-zero in abe_step
        let mut vy = 0.0;
        let mut vz = 0.0;
        let dt = 0.1;

        for step_num in 0..20 {
            let params = StepParams {
                pos_x: x,
                pos_y: y,
                pos_z: z,
                vel_x: vx,
                vel_y: vy,
                vel_z: vz,
                dt_s: dt,
                wind_x: 0.0,
                wind_y: 0.0,
                wind_z: 0.0,
                density_kgm3: 1.225,
                temp_c: 15.0,
                altitude_m: 0.0,
                cdm_id: cdm,
                bc: 0.0, // No drag → free fall
                mass_g: 4.0,
                caliber_mm: 5.56,
            };
            let mut result = BulletState::default();
            assert_eq!(abe_step(&params, &mut result), 0);

            x = result.pos_x;
            y = result.pos_y;
            z = result.pos_z;
            vx = result.vel_x;
            vy = result.vel_y;
            vz = result.vel_z;

            let t = (step_num + 1) as f64 * dt;
            // Semi-implicit Euler: velocity update is exact for constant acceleration
            // v(t) = g * t
            let expected_vz = atmosphere::GRAVITY * t;
            assert!(
                (vz - expected_vz).abs() < 0.001,
                "vz should match free-fall at t={}: expected={}, actual={}",
                t,
                expected_vz,
                vz
            );

            // Position from semi-implicit Euler overestimates analytical:
            // z(t) = g * dt² * N(N+1)/2 vs analytical g*t²/2
            // So position ratio = (N+1)/N, which tends to 1 as N grows.
            // With N=20 at t=2.0, ratio = 1.05 → verify z bounds bracket the truth.
            let z_lower = 0.5 * atmosphere::GRAVITY * t * t; // analytical min
            let z_upper = z_lower * (t / dt + 1.0) / (t / dt); // semi-implicit Euler max
            assert!(
                z >= z_lower * 0.95 && z <= z_upper * 1.05,
                "z should be near free-fall at t={}: analytical={:.4}, bounds=[{:.4},{:.4}], actual={:.4}",
                t,
                z_lower,
                z_lower * 0.95,
                z_upper * 1.05,
                z
            );
        }
    }

    #[test]
    fn trajectory_high_altitude_less_drag() {
        abe_init(ABE_API_VERSION, 0);

        let mut cdm = [0u8; 32];
        cdm[..3].copy_from_slice(b"g7\0");

        let run = |altitude_m: f64| -> f64 {
            let mut x = 0.0;
            let mut y = 0.0;
            let mut z = 0.0;
            let mut vx = 930.0;
            let mut vy = 0.0;
            let mut vz = 0.0;

            for _ in 0..50 {
                let params = StepParams {
                    pos_x: x,
                    pos_y: y,
                    pos_z: z,
                    vel_x: vx,
                    vel_y: vy,
                    vel_z: vz,
                    dt_s: 0.1,
                    wind_x: 0.0,
                    wind_y: 0.0,
                    wind_z: 0.0,
                    density_kgm3: 1.225,
                    temp_c: 15.0,
                    altitude_m,
                    cdm_id: cdm,
                    bc: 0.157,
                    mass_g: 4.0,
                    caliber_mm: 5.56,
                };
                let mut result = BulletState::default();
                abe_step(&params, &mut result);

                x = result.pos_x;
                y = result.pos_y;
                z = result.pos_z;
                vx = result.vel_x;
                vy = result.vel_y;
                vz = result.vel_z;
            }
            vx
        };

        let sea_level_v = run(0.0);
        let high_alt_v = run(5000.0);

        assert!(
            high_alt_v > sea_level_v,
            "At altitude (lower density) bullet should slow less: sea={:.1}, alt={:.1}",
            sea_level_v,
            high_alt_v
        );
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
