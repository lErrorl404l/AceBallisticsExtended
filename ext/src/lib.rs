// ABE - Advanced Ballistics Extension
// C ABI dispatcher for ARMA 3 callExtension interface
//
// All physics kernels are pure functions. The C ABI layer handles
// serialization, dispatch, and error reporting.

// Skeleton: many physics API functions not yet wired to C ABI dispatchers.
// Remove per-item allows as modules get wired in Phase 1+.
#![allow(dead_code)]

mod atmosphere;
mod config;
mod drag;
mod exterior;
mod interior;
mod penetration;

use std::ffi::{CStr, CString};
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

// ── C ABI ─────────────────────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn abe_init(api_version: u32, ace_present: u32) -> i32 {
    if api_version != ABE_API_VERSION {
        return -1; // Version mismatch
    }
    let state = AbeState {
        initialized: true,
        ace_present: ace_present != 0,
        data_loaded: false,
    };
    let _ = STATE.set(state);
    0 // OK
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
    pub cdm_id: [u8; 32], // null-terminated string
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FireResult {
    pub muzzle_velocity_ms: f64,
    pub max_chamber_pressure_mpa: f64,
    pub propellant_burn_fraction: f64,
    pub barrel_time_ms: f64,
}

/// Calculate interior ballistics for a given weapon/ammo combination.
/// Returns 0 on success, -1 on error.
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
        params.barrel_length_mm / 1000.0,  // mm → m
        params.chamber_pressure_mpa * 1e6, // MPa → Pa
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

    // Wind (relative velocity)
    let vx = vx - params.wind_x;
    let vy = vy - params.wind_y;
    let vz = vz - params.wind_z;

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
            barrel_length_mm: 368.0,     // M4A1 carbine
            chamber_pressure_mpa: 380.0, // 5.56 NATO
            caliber_mm: 5.56,
            projectile_mass_g: 4.0, // M855
            cdm_id: cdm,
        };

        let mut result = FireResult {
            muzzle_velocity_ms: 0.0,
            max_chamber_pressure_mpa: 0.0,
            propellant_burn_fraction: 0.0,
            barrel_time_ms: 0.0,
        };

        let ret = abe_fire(&params, &mut result);
        assert_eq!(ret, 0);
        assert!(result.muzzle_velocity_ms > 800.0); // M855 ~948 m/s
        assert!(result.muzzle_velocity_ms < 1100.0);
        assert!(result.max_chamber_pressure_mpa > 200.0);
    }

    #[test]
    fn longer_barrel_increases_mv() {
        abe_init(ABE_API_VERSION, 0);

        let mut cdm = [0u8; 32];
        cdm[..3].copy_from_slice(b"g7\0");

        let short = FireParams {
            barrel_length_mm: 254.0, // 10" barrel
            chamber_pressure_mpa: 380.0,
            caliber_mm: 5.56,
            projectile_mass_g: 4.0,
            cdm_id: cdm.clone(),
        };

        let long = FireParams {
            barrel_length_mm: 508.0, // 20" barrel
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

        let mut result = BulletState {
            pos_x: 0.0,
            pos_y: 0.0,
            pos_z: 0.0,
            vel_x: 0.0,
            vel_y: 0.0,
            vel_z: 0.0,
            mach: 0.0,
            time_s: 0.0,
        };

        let ret = abe_step(&params, &mut result);
        assert_eq!(ret, 0);
        assert!(result.pos_x > 0.0); // Moves forward
        assert!(result.vel_x < 900.0); // Slowed by drag
    }

    #[test]
    fn impact_penetration() {
        abe_init(ABE_API_VERSION, 0);

        let mut mat = [0u8; 32];
        mat[..10].copy_from_slice(b"steel_rha\0");
        let mut proj = [0u8; 32];
        proj[..5].copy_from_slice(b"ball\0");

        let params = ImpactParams {
            vel_x: 900.0,
            vel_y: 0.0,
            vel_z: 0.0,
            mass_g: 9.5, // 7.62x51mm
            caliber_mm: 7.62,
            armor_thickness_mm: 5.0, // Thin steel
            armor_material: mat,
            impact_angle_deg: 0.0,
            projectile_type: proj,
        };

        let mut result = ImpactResult {
            penetrated: 0,
            residual_vel_ms: 0.0,
            energy_j: 0.0,
            effective_thickness_mm: 0.0,
            ricochet: 0,
            ricochet_angle_deg: 0.0,
            ricochet_energy_fraction: 0.0,
            fragments: 0,
            spall_fragments: 0,
        };

        let ret = abe_impact(&params, &mut result);
        assert_eq!(ret, 0);
        // 7.62mm at 900 m/s should pen 5mm RHA at 0°
        assert_eq!(
            result.penetrated, 1,
            "7.62x51mm at 900 m/s should penetrate 5mm RHA at 0°"
        );
    }
}

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
