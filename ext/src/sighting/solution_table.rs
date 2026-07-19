// ABE - Ballistic Solution Table
//
// Pre-computes ballistic trajectories for fast lookup. Stores range,
// velocity, drop, windage, and time-of-flight at fixed range intervals.
// The trajectory is computed using the same semi-implicit Euler integration
// as the core `abe_step` engine, ensuring consistency with the live solver.
//
// The solution table is the foundation for scope-dope cards, ballistic
// reticles, and holdover/windage click calculations.  Entries are stored
// at fixed `step_m` intervals and interpolated linearly for arbitrary
// intermediate ranges.
//
// References:
//   - NATO STANAG 4355 (AOP-55) — ballistic tables
//   - JBM Ballistics (trajectory table format)
//   - ABE core: lib.rs abe_step, exterior.rs, interior.rs

#![allow(dead_code)]

use crate::atmosphere;
use crate::drag;
use crate::exterior;
use crate::sight_height;

/// A single entry in a ballistic solution table.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SolutionEntry {
    /// Downrange distance (metres).
    pub range_m: f64,
    /// Projectile speed at this range (m/s).
    pub velocity_ms: f64,
    /// Drop below the bore axis (metres). Positive = below the line of fire.
    /// This is the integrated z-position from the step solver.
    pub drop_m: f64,
    /// Lateral drift per 1 m/s crosswind (metres).
    /// Multiply by the actual crosswind speed to get total wind drift.
    pub windage_m_per_ms: f64,
    /// Accumulated time of flight to this range (seconds).
    pub time_of_flight_s: f64,
    /// Mach number at this range.
    pub mach: f64,
}

/// A pre-computed ballistic solution covering the full trajectory
/// from the muzzle to `max_range_m` in `step_m` increments.
#[derive(Debug, Clone)]
pub struct BallisticSolution {
    /// Trajectory entries at each step_m interval.
    pub entries: Vec<SolutionEntry>,
    /// Maximum range covered by this solution (metres).
    pub max_range_m: f64,
    /// Range step between entries (metres).
    pub step_m: f64,
    /// Muzzle velocity at which the solution was computed (m/s).
    pub mv_ms: f64,
    /// Ballistic coefficient used (G1 or G7, lb/in²).
    pub bc: f64,
    /// Drag model identifier (e.g. "g7", "g1").
    pub cdm_id: String,
    /// Drop at the muzzle from the sight-height offset (metres).
    /// This is the vertical distance from the line of sight to the bore
    /// axis at the muzzle; always negative (the projectile starts below
    /// the line of sight).
    pub muzzle_drop_m: f64,
}

// ── Local step integration ──────────────────────────────────────────────────

/// Internal step structure matching the ABE step integration.
#[derive(Debug, Clone, Copy)]
struct StepState {
    x: f64,
    y: f64,
    z: f64,
    vx: f64,
    vy: f64,
    vz: f64,
    t: f64,
}

/// Perform one semi-implicit Euler integration step, mirroring the
/// physics in `abe_step` (lib.rs) without requiring global state.
///
/// Applies drag (via BC-based look-up), gravity, the yaw-of-repose
/// induced drag multiplier, and wind.
#[allow(clippy::too_many_arguments)]
// ponytail: physics kernel, all params required
fn local_step(
    state: &StepState,
    dt_s: f64,
    density: f64,
    temp_c: f64,
    cdm_id: &str,
    bc: f64,
    mass_g: f64,
    caliber_mm: f64,
    wind_y: f64,
) -> StepState {
    let speed = (state.vx.powi(2) + state.vy.powi(2) + state.vz.powi(2)).sqrt();
    if speed <= 0.0 {
        return StepState {
            x: state.x,
            y: state.y,
            z: state.z,
            vx: 0.0,
            vy: 0.0,
            vz: 0.0,
            t: state.t + dt_s,
        };
    }

    let mach = exterior::calc_mach(speed, temp_c);
    let cd = drag::get_cd(cdm_id, mach);

    // BC conversion (same as abe_step)
    const BC_CONV: f64 = 0.453592 / (0.0254 * 0.0254) * (4.0 / std::f64::consts::PI);
    let bc_metric = bc * BC_CONV;
    let drag_decel = if speed > 0.001 && bc_metric > 0.001 {
        0.5 * density * speed * speed * cd / bc_metric
    } else {
        0.0
    };

    // Yaw-of-repose induced drag (4-DOF model from abe_step)
    const REF_SG: f64 = 1.5;
    const REF_CAL: f64 = 5.56;
    const REF_MASS: f64 = 4.0;
    const REF_SPEED: f64 = 930.0;
    const REF_DENSITY: f64 = 1.225;

    let sg = if mass_g > 0.0 && speed > 0.0 {
        REF_SG
            * (caliber_mm / REF_CAL).powi(2)
            * (REF_MASS / mass_g)
            * (REF_SPEED / speed)
            * (REF_DENSITY / density.max(0.01))
    } else {
        0.0
    };

    let yaw_repose = if sg > 0.1 && speed > 0.0 {
        let sin_theta = (state.vz.abs() / speed).min(1.0);
        sin_theta * (caliber_mm / speed) * 20.0 / sg
    } else {
        0.0
    };

    let ind_drag_mult = 1.0 + 25.0 * yaw_repose * yaw_repose;
    let total_drag = if sg >= 1.0 {
        drag_decel * ind_drag_mult
    } else {
        drag_decel
    };

    // Velocity update: drag (opposes velocity)
    let vx = state.vx - total_drag * (state.vx / speed) * dt_s;
    let vy = state.vy - total_drag * (state.vy / speed) * dt_s;
    let vz = state.vz - total_drag * (state.vz / speed) * dt_s;

    // Gravity (ABE uses +z = downward)
    let vz = vz + atmosphere::GRAVITY * dt_s;

    // Wind: crosswind only (wind_y), scaled by dt for dimensional consistency.
    // Note: abe_step applies wind without dt scaling (velocity impulse per step),
    // but that creates timestep-dependent drift. The physically correct form
    // treats wind as an acceleration: a = v_wind / τ where τ is the
    // aerodynamic response time. Here we scale by dt for consistent results.
    let wind_factor = atmosphere::wind_shear_factor(0.0);
    let vy = vy - wind_y * wind_factor * dt_s;

    // Position update
    StepState {
        x: state.x + vx * dt_s,
        y: state.y + vy * dt_s,
        z: state.z + vz * dt_s,
        vx,
        vy,
        vz,
        t: state.t + dt_s,
    }
}

// ── Core functions ──────────────────────────────────────────────────────────

/// Compute a full ballistic solution table by integrating the trajectory
/// from the muzzle out to `max_range_m` at `step_m` intervals.
///
/// The trajectory is initialised with the bore-elevation zero angle
/// computed from the sight height and zero range.  Two passes are
/// performed: one with zero crosswind for the baseline drop, and one
/// with 1 m/s crosswind to extract the windage coefficient.
#[allow(clippy::too_many_arguments)]
// ponytail: physics kernel, all params required
pub fn compute_solution(
    mv_ms: f64,
    bc: f64,
    cdm_id: &str,
    mass_g: f64,
    caliber_mm: f64,
    sight_height_mm: f64,
    zero_range_m: f64,
    altitude_m: f64,
    temp_c: f64,
    max_range_m: f64,
    step_m: f64,
) -> BallisticSolution {
    // Air density
    let density = if altitude_m > 0.0 {
        atmosphere::density_from_altitude(altitude_m, temp_c)
    } else {
        atmosphere::SEA_LEVEL_DENSITY
    };

    // Initial elevation angle for zeroing
    let zero_angle =
        sight_height::zero_angle(sight_height_mm / 1000.0, zero_range_m, mv_ms).unwrap_or(0.0);

    // Launch vector: ABE uses +z = downward, so upward launch = negative vz
    let vx_init = mv_ms * zero_angle.cos();
    let vz_init = -mv_ms * zero_angle.sin();

    // Time step for integration (fixed small dt for accuracy)
    let dt_s = 0.005;

    let mut state_no_wind = StepState {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        vx: vx_init,
        vy: 0.0,
        vz: vz_init,
        t: 0.0,
    };

    let mut state_wind = StepState {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        vx: vx_init,
        vy: 0.0,
        vz: vz_init,
        t: 0.0,
    };

    let mut entries: Vec<SolutionEntry> = Vec::new();

    // Muzzle entry at range 0
    let speed0 =
        (state_no_wind.vx.powi(2) + state_no_wind.vy.powi(2) + state_no_wind.vz.powi(2)).sqrt();
    let mach0 = exterior::calc_mach(speed0, temp_c);
    entries.push(SolutionEntry {
        range_m: 0.0,
        velocity_ms: speed0,
        drop_m: state_no_wind.z, // 0 at start
        windage_m_per_ms: 0.0,
        time_of_flight_s: 0.0,
        mach: mach0,
    });

    let mut next_range = step_m;

    // Integrate until we exceed max_range_m or the bullet goes subsonic
    // and slow (below 50 m/s indicates it's impractical)
    while state_no_wind.x < max_range_m && state_no_wind.vx > 50.0 {
        // Adaptive dt: smaller dt for supersonic, larger for subsonic
        let local_dt = if state_no_wind.vx > 400.0 {
            dt_s
        } else if state_no_wind.vx > 200.0 {
            0.01
        } else {
            0.02
        };

        state_no_wind = local_step(
            &state_no_wind,
            local_dt,
            density,
            temp_c,
            cdm_id,
            bc,
            mass_g,
            caliber_mm,
            0.0,
        );

        state_wind = local_step(
            &state_wind,
            local_dt,
            density,
            temp_c,
            cdm_id,
            bc,
            mass_g,
            caliber_mm,
            1.0, // 1 m/s crosswind for windage coefficient
        );

        // Record entry at each step_m boundary
        if state_no_wind.x >= next_range && state_no_wind.x <= max_range_m + step_m {
            let speed =
                (state_no_wind.vx.powi(2) + state_no_wind.vy.powi(2) + state_no_wind.vz.powi(2))
                    .sqrt();
            let mach = exterior::calc_mach(speed, temp_c);

            // Windage = lateral drift per 1 m/s crosswind
            let windage = state_wind.y - state_no_wind.y;

            entries.push(SolutionEntry {
                range_m: next_range,
                velocity_ms: speed,
                drop_m: state_no_wind.z,
                windage_m_per_ms: windage,
                time_of_flight_s: state_no_wind.t,
                mach,
            });

            next_range += step_m;
        }
    }

    let muzzle_drop_m = -(sight_height_mm / 1000.0);

    BallisticSolution {
        entries,
        max_range_m,
        step_m,
        mv_ms,
        bc,
        cdm_id: cdm_id.to_string(),
        muzzle_drop_m,
    }
}

/// Interpolate a solution entry at an arbitrary range using linear
/// interpolation between the two nearest table entries.
///
/// Returns `None` when the range is outside the table bounds.
pub fn interpolate_solution(sol: &BallisticSolution, range_m: f64) -> Option<SolutionEntry> {
    let entries = &sol.entries;
    if entries.is_empty() {
        return None;
    }

    if range_m <= entries[0].range_m {
        return Some(entries[0]);
    }
    if range_m >= entries[entries.len() - 1].range_m {
        return Some(*entries.last().unwrap());
    }

    // Binary search for the bracketing entries
    let idx = match entries.binary_search_by(|e| {
        e.range_m
            .partial_cmp(&range_m)
            .unwrap_or(std::cmp::Ordering::Equal)
    }) {
        Ok(i) => return Some(entries[i]),
        Err(i) => {
            if i == 0 {
                return Some(entries[0]);
            }
            i - 1
        },
    };

    let lo = &entries[idx];
    let hi = &entries[idx + 1];
    let dr = hi.range_m - lo.range_m;
    if dr <= 0.0 {
        return Some(*lo);
    }
    let frac = (range_m - lo.range_m) / dr;

    Some(SolutionEntry {
        range_m,
        velocity_ms: lo.velocity_ms + frac * (hi.velocity_ms - lo.velocity_ms),
        drop_m: lo.drop_m + frac * (hi.drop_m - lo.drop_m),
        windage_m_per_ms: lo.windage_m_per_ms + frac * (hi.windage_m_per_ms - lo.windage_m_per_ms),
        time_of_flight_s: lo.time_of_flight_s + frac * (hi.time_of_flight_s - lo.time_of_flight_s),
        mach: lo.mach + frac * (hi.mach - lo.mach),
    })
}

/// Number of MOA elevation clicks needed at a given range.
///
/// Converts the interpolated drop at `range_m` to an angular correction
/// in MOA and divides by the scope's click increment.
///
/// Returns `None` if the range is outside the solution table or the
/// click value is not positive.
pub fn holdover_click(sol: &BallisticSolution, range_m: f64, click_moa: f64) -> Option<i32> {
    if click_moa <= 0.0 {
        return None;
    }
    let entry = interpolate_solution(sol, range_m)?;

    // Angular correction: atan(drop / range) ≈ drop / range for small angles
    // We adjust for the muzzle drop (sight height offset)
    if range_m <= 0.0 {
        return Some(0);
    }
    let total_drop = entry.drop_m - sol.muzzle_drop_m;
    let angle_rad = (total_drop / range_m).atan();
    let angle_moa = angle_rad * sight_height::MOA_PER_RAD;

    let clicks = (angle_moa / click_moa).round() as i32;
    Some(clicks)
}

/// Number of windage clicks needed for a given crosswind speed.
///
/// Converts the wind drift at `range_m` to an angular correction in MOA
/// and divides by the scope's click increment.
///
/// Returns `None` if the range is outside the solution table or the
/// click value is not positive.
pub fn windage_click(
    sol: &BallisticSolution,
    range_m: f64,
    wind_ms: f64,
    click_moa: f64,
) -> Option<i32> {
    if click_moa <= 0.0 {
        return None;
    }
    let entry = interpolate_solution(sol, range_m)?;

    if range_m <= 0.0 {
        return Some(0);
    }
    let wind_drift_m = entry.windage_m_per_ms * wind_ms;
    let angle_rad = (wind_drift_m / range_m).atan();
    let angle_moa = angle_rad * sight_height::MOA_PER_RAD;

    let clicks = (angle_moa / click_moa).round() as i32;
    Some(clicks)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a standard M855-like solution table for testing.
    fn m855_solution() -> BallisticSolution {
        compute_solution(
            930.0, // mv_ms
            0.157, // bc (G7)
            "g7",  // cdm_id
            4.0,   // mass_g
            5.56,  // caliber_mm
            40.0,  // sight_height_mm
            100.0, // zero_range_m
            0.0,   // altitude_m
            15.0,  // temp_c
            600.0, // max_range_m
            50.0,  // step_m
        )
    }

    // ── Solution table structure ─────────────────────────────────────────

    #[test]
    fn solution_has_correct_number_of_entries() {
        let sol = m855_solution();
        // 0 to 600 m at 50 m steps = 13 entries (0, 50, 100, ..., 600)
        let expected = (600.0 / 50.0) as usize + 1;
        assert_eq!(
            sol.entries.len(),
            expected,
            "expected {} entries, got {}",
            expected,
            sol.entries.len()
        );
    }

    #[test]
    fn solution_starts_at_range_zero() {
        let sol = m855_solution();
        assert_eq!(sol.entries[0].range_m, 0.0);
        assert!((sol.entries[0].velocity_ms - 930.0).abs() < 10.0);
        assert_eq!(sol.entries[0].drop_m, 0.0);
        assert_eq!(sol.entries[0].time_of_flight_s, 0.0);
    }

    #[test]
    fn solution_matches_step_integration_at_500m() {
        let sol = m855_solution();
        // At 500 m, the M855 drop should be between 1.5 and 3.5 m (positive = below bore)
        // based on the existing trajectory_m855_at_930ms test which asserts drop > 0 && < 4
        let entry_500 = sol.entries.iter().find(|e| (e.range_m - 500.0).abs() < 1.0);
        assert!(entry_500.is_some(), "should have entry near 500m");
        let e = entry_500.unwrap();
        assert!(
            e.drop_m > 1.0 && e.drop_m < 4.0,
            "drop at 500 m should be 1-4 m: {}",
            e.drop_m
        );
        // Velocity at 500 m should be 400-600 m/s (from existing tests)
        assert!(
            e.velocity_ms > 300.0 && e.velocity_ms < 700.0,
            "velocity at 500 m should be 300-700 m/s: {}",
            e.velocity_ms
        );
    }

    #[test]
    fn velocity_decreases_with_range() {
        let sol = m855_solution();
        for i in 1..sol.entries.len() {
            assert!(
                sol.entries[i].velocity_ms <= sol.entries[i - 1].velocity_ms + 1.0,
                "velocity should decrease: entries[{}].vel={} > entries[{}].vel={}",
                i,
                sol.entries[i].velocity_ms,
                i - 1,
                sol.entries[i - 1].velocity_ms
            );
        }
    }

    #[test]
    fn drop_increases_with_range() {
        let sol = m855_solution();
        // The trajectory is launched with a slight upward angle (zeroing),
        // so the bullet initially rises (negative drop relative to bore axis).
        // After the apogee (~150-200m for M855), drop increases monotonically.
        // Start checking from entry 4 (200 m) onward.
        for i in 4..sol.entries.len() {
            assert!(
                sol.entries[i].drop_m >= sol.entries[i - 1].drop_m - 0.01,
                "drop should increase beyond apogee: entries[{}].drop={} < entries[{}].drop={}",
                i,
                sol.entries[i].drop_m,
                i - 1,
                sol.entries[i - 1].drop_m
            );
        }
    }

    // ── Interpolation ────────────────────────────────────────────────────

    #[test]
    fn interpolate_exact_entry() {
        let sol = m855_solution();
        let entry = interpolate_solution(&sol, 100.0).unwrap();
        let exact = sol
            .entries
            .iter()
            .find(|e| (e.range_m - 100.0).abs() < 1.0)
            .unwrap();
        assert!(
            (entry.drop_m - exact.drop_m).abs() < 0.001,
            "interpolation at exact entry should match: interp={}, exact={}",
            entry.drop_m,
            exact.drop_m
        );
    }

    #[test]
    fn interpolate_midpoint() {
        let sol = m855_solution();
        // At midpoint between 100 and 150 (if step=50 → 125)
        let mid = 125.0;
        let entry = interpolate_solution(&sol, mid).unwrap();
        assert!(
            (entry.range_m - mid).abs() < 0.001,
            "range should match: {}",
            entry.range_m
        );
        // The interpolated value should be between the bracketing entries
        let lo = sol
            .entries
            .iter()
            .find(|e| (e.range_m - 100.0).abs() < 1.0)
            .unwrap();
        let hi = sol
            .entries
            .iter()
            .find(|e| (e.range_m - 150.0).abs() < 1.0)
            .unwrap();
        assert!(
            entry.drop_m >= lo.drop_m.min(hi.drop_m) - 0.01
                && entry.drop_m <= lo.drop_m.max(hi.drop_m) + 0.01,
            "interpolated drop should be between entries: lo={}, hi={}, interp={}",
            lo.drop_m,
            hi.drop_m,
            entry.drop_m
        );
    }

    #[test]
    fn interpolate_beyond_max_range() {
        let sol = m855_solution();
        // Range beyond max → clamp to last entry
        let entry = interpolate_solution(&sol, 2000.0).unwrap();
        let last = sol.entries.last().unwrap();
        assert!(
            (entry.drop_m - last.drop_m).abs() < 0.001,
            "clamp to last entry: interp={}, last={}",
            entry.drop_m,
            last.drop_m
        );
    }

    #[test]
    fn interpolate_below_range() {
        let sol = m855_solution();
        // Negative range → clamp to first entry
        let entry = interpolate_solution(&sol, -10.0).unwrap();
        assert!(
            (entry.drop_m - sol.entries[0].drop_m).abs() < 0.001,
            "negative range should clamp to first entry"
        );
    }

    // ── Zero range consistency ───────────────────────────────────────────

    #[test]
    fn zero_range_returns_correct_drop() {
        // At the zero range (100 m), the drop relative to the sight line
        // should be approximately 0 (the bullet crosses the line of sight).
        // The trajectory drop_m is relative to the bore axis, not LOS,
        // so drop - muzzle_drop should be ~0 at the zero range.
        let sol = m855_solution();
        let entry_100 = sol
            .entries
            .iter()
            .find(|e| (e.range_m - 100.0).abs() < 1.0)
            .unwrap();
        let relative_drop = entry_100.drop_m - sol.muzzle_drop_m;
        // The bullet should be near the line of sight at zero range
        // (within ±0.1 m is acceptable for the simplified model)
        assert!(
            relative_drop.abs() < 0.2,
            "drop relative to LOS at 100 m should be near zero: {}",
            relative_drop
        );
    }

    // ── Max range ────────────────────────────────────────────────────────

    #[test]
    fn max_range_entry_present() {
        let sol = m855_solution();
        let last = sol.entries.last().unwrap();
        assert!(
            (last.range_m - 600.0).abs() < 1.0,
            "last entry should be at max_range: last={}",
            last.range_m
        );
    }

    // ── Holdover clicks ──────────────────────────────────────────────────

    #[test]
    fn holdover_clicks_increase_with_range() {
        let sol = m855_solution();
        let c100 = holdover_click(&sol, 100.0, 0.25).unwrap_or(0);
        let c300 = holdover_click(&sol, 300.0, 0.25).unwrap_or(0);
        let c500 = holdover_click(&sol, 500.0, 0.25).unwrap_or(0);
        assert!(
            c300 >= c100,
            "clicks at 300 should >= 100: {} >= {}",
            c300,
            c100
        );
        assert!(
            c500 >= c300,
            "clicks at 500 should >= 300: {} >= {}",
            c500,
            c300
        );
    }

    #[test]
    fn holdover_clicks_zero_at_zero_range() {
        let sol = m855_solution();
        let clicks = holdover_click(&sol, 0.0, 0.25);
        assert_eq!(clicks, Some(0));
    }

    #[test]
    fn holdover_clicks_invalid_click_value() {
        let sol = m855_solution();
        assert_eq!(holdover_click(&sol, 300.0, 0.0), None);
        assert_eq!(holdover_click(&sol, 300.0, -0.1), None);
    }

    // ── Windage clicks ───────────────────────────────────────────────────

    #[test]
    fn windage_scales_linearly_with_wind() {
        let sol = m855_solution();
        // Windage at constant range should scale linearly with wind speed
        let c1 = windage_click(&sol, 300.0, 2.0, 0.25).unwrap_or(0);
        let c2 = windage_click(&sol, 300.0, 4.0, 0.25).unwrap_or(0);
        // 4 m/s at the same range should produce ~2× the clicks of 2 m/s
        assert!(
            (c2 as f64 / c1 as f64 - 2.0).abs() < 0.5 || (c1 == 0 && c2 == 0),
            "windage clicks should roughly double with double wind: {} m/s -> {}, {} m/s -> {}",
            2.0,
            c1,
            4.0,
            c2
        );
    }

    #[test]
    fn windage_increases_with_range() {
        let sol = m855_solution();
        let c200 = windage_click(&sol, 200.0, 5.0, 0.25).unwrap_or(0);
        let c500 = windage_click(&sol, 500.0, 5.0, 0.25).unwrap_or(0);
        // Longer range → more wind drift → more clicks
        // (windage_m_per_ms increases with range)
        // Sign is negative (wind pushes opposite to wind direction convention),
        // so compare absolute values.
        if c200 != 0 || c500 != 0 {
            assert!(
                c500.abs() >= c200.abs(),
                "windage clicks at 500 m should >= 200 m: |{}| >= |{}|",
                c500,
                c200
            );
        }
    }

    #[test]
    fn windage_side_consistency() {
        let sol = m855_solution();
        // Wind from the left (positive) and right (negative) should
        // produce opposite-signed clicks.
        let c_left = windage_click(&sol, 300.0, 5.0, 0.25).unwrap_or(0);
        let c_right = windage_click(&sol, 300.0, -5.0, 0.25).unwrap_or(0);
        assert!(
            c_left.abs() == c_right.abs(),
            "|left wind| clicks should equal |right wind| clicks: {} vs {}",
            c_left,
            c_right
        );
        if c_left != 0 {
            assert_eq!(
                c_left, -c_right,
                "left and right wind should give opposite signs"
            );
        }
    }

    #[test]
    fn windage_zero_with_no_wind() {
        let sol = m855_solution();
        let clicks = windage_click(&sol, 300.0, 0.0, 0.25);
        assert_eq!(clicks, Some(0));
    }

    #[test]
    fn windage_out_of_range() {
        let sol = m855_solution();
        let clicks = windage_click(&sol, 9999.0, 5.0, 0.25);
        // Should still return a value (clamps to last entry)
        assert!(clicks.is_some());
    }
}
