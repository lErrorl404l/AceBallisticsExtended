// ABE - External Ballistics Utilities
//
// Common utility functions for external ballistics calculations.
// The primary integration happens in lib.rs (abe_step).
//
// References:
//   - McCoy's Modern Exterior Ballistics
//   - NATO STANAG 4355 (AOP-55)

/// Speed of sound at given temperature (m/s).
/// Uses the standard formula: c = 331.3 * sqrt(1 + T/273.15)
pub fn speed_of_sound(temp_c: f64) -> f64 {
    331.3 * (1.0 + temp_c / 273.15).sqrt()
}

/// Mach number from velocity and temperature.
pub fn calc_mach(velocity_ms: f64, temp_c: f64) -> f64 {
    if velocity_ms <= 0.0 {
        return 0.0;
    }
    let sos = speed_of_sound(temp_c);
    velocity_ms / sos
}

/// Crosswind deflection (simplified).
/// Uses the standard formula: drift = wind * (TOF - range/mv)
pub fn wind_drift(
    crosswind_ms: f64,
    time_of_flight_s: f64,
    range_m: f64,
    muzzle_velocity_ms: f64,
) -> f64 {
    if muzzle_velocity_ms <= 0.0 {
        return 0.0;
    }
    crosswind_ms * (time_of_flight_s - range_m / muzzle_velocity_ms)
}

/// Spin drift (Magnus effect) approximation.
/// Formula from McCoy: SD = k * TOF * (spin_rate/v)^2 * range
pub fn spin_drift(
    twist_rate_rev_per_m: f64,
    muzzle_velocity_ms: f64,
    time_of_flight_s: f64,
    range_m: f64,
) -> f64 {
    let spin_rate = twist_rate_rev_per_m * muzzle_velocity_ms; // rev/s
    let k = 1.25e-4; // Empirical constant
    k * time_of_flight_s * spin_rate.powi(2) / muzzle_velocity_ms * range_m
}

/// Coriolis deflection (horizontal component).
/// Δx = 2 * Ω * v * t² * sin(latitude)  (simplified eastward deflection)
pub fn coriolis_horizontal(
    latitude_deg: f64,
    muzzle_velocity_ms: f64,
    time_of_flight_s: f64,
    azimuth_deg: f64,
) -> f64 {
    let omega = 7.2921e-5; // Earth's angular velocity (rad/s)
    let lat_rad = latitude_deg.to_radians();
    let az_rad = azimuth_deg.to_radians();

    // Eastward deflection: positive east, negative west
    // Depends on azimuth direction
    2.0 * omega * muzzle_velocity_ms * time_of_flight_s.powi(2) * lat_rad.sin() * az_rad.cos()
}

/// Range at which a bullet transitions through the transonic region
/// (Mach 0.8-1.2). Returns None if never transonic within 2000m.
pub fn transonic_range(muzzle_velocity_ms: f64, bc: f64, temp_c: f64) -> Option<f64> {
    if muzzle_velocity_ms < speed_of_sound(temp_c) * 1.2 {
        return Some(0.0); // Already subsonic/subsonic-bound
    }

    // Crude estimate: drag slows bullet ~80 m/s per 100m for M80 ball at G7 BC
    // Transonic threshold: below Mach 1.2
    let mach1 = speed_of_sound(temp_c);
    let dv = muzzle_velocity_ms - mach1 * 1.2;
    let range_estimate = 5.0 * dv * bc * 10.0;

    if range_estimate > 2000.0 {
        None
    } else {
        Some(range_estimate)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speed_of_sound_at_15c() {
        let sos = speed_of_sound(15.0);
        // ~340.3 m/s at 15°C
        assert!((sos - 340.3).abs() < 1.0);
    }

    #[test]
    fn speed_of_sound_at_0c() {
        let sos = speed_of_sound(0.0);
        assert!((sos - 331.3).abs() < 0.1);
    }

    #[test]
    fn mach_1_at_sea_level() {
        let m = calc_mach(340.3, 15.0);
        assert!((m - 1.0).abs() < 0.01);
    }

    #[test]
    fn mach_2_point_5() {
        let m = calc_mach(850.0, 15.0);
        assert!((m - 2.5).abs() < 0.05);
    }

    #[test]
    fn wind_drift_correct_direction() {
        let drift = wind_drift(5.0, 1.5, 800.0, 850.0);
        // Crosswind from left (positive) → drift right (positive)
        assert!(drift > 0.0);
    }

    #[test]
    fn spin_drift_positive() {
        // Right-hand twist (positive) → spin drift right (positive in northern hemisphere)
        let drift = spin_drift(1.0 / 0.178, 850.0, 1.5, 800.0); // 1:7" twist
        assert!(drift > 0.0);
    }
}
