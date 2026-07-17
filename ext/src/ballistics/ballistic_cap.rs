// ABE - Ballistic Cap & Piercing Cap Detachment Model
//
// Armour-piercing (AP) projectiles often carry a ballistic cap (windshield)
// that improves aerodynamics in flight but detaches on impact.  Some AP
// rounds also have a piercing cap (armour-piercing cap) that stays on during
// flight and detaches only during the initial phase of penetration, providing
// a ~5-15 % penetration improvement by distributing impact forces.
//
// References:
//   - Hogg, I.V., "The World's Great Artillery" — cap types and function
//   - US Army TM 9-1907 "Ballistic Data Performance of Ammunition" (1948)
//   - Okun, N., "Facehard vs. Decapping" (NavWeaps) — piercing cap mechanics
//   - Williams, A.G., "Rapid Fire" — AP cap development history
//   - M8 API (12.7 mm) data: ~42 g total projectile, cap ~4 g

#![allow(dead_code)]

/// When a cap detaches from the projectile.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CapDetachTiming {
    /// Ballistic cap detaches immediately on striking any surface.
    OnImpact,
    /// Piercing cap stays on through impact and detaches during the
    /// penetration phase as the core pushes through.
    OnPenetration,
    /// Detaches in flight due to aerodynamic forces (rare, usually
    /// indicates a defective round).
    InFlight,
    /// No cap present on this projectile.
    None,
}

/// Parameters for evaluating ballistic cap or piercing cap behaviour.
#[derive(Debug, Clone, Copy)]
pub struct BallisticCapParams {
    /// Whether the projectile has a ballistic cap (windshield).
    pub has_ballistic_cap: bool,
    /// Whether the projectile has a piercing cap (armour-piercing cap).
    pub has_piercing_cap: bool,
    /// Mass of the cap (grams). For ballistic caps this is the windshield
    /// mass; for piercing caps this is the hardened cap mass.
    pub cap_mass_g: f64,
    /// Mass of the projectile body without the cap (grams).
    pub projectile_mass_g: f64,
    /// Projectile calibre (mm).
    pub caliber_mm: f64,
    /// Impact velocity (m/s).
    pub impact_velocity_ms: f64,
    /// Impact angle from surface normal (degrees).
    pub impact_angle_deg: f64,
}

/// Result of a ballistic cap or piercing cap evaluation.
#[derive(Debug, Clone, Copy)]
pub struct CapDetachResult {
    /// Whether the cap detaches (separates from the projectile).
    pub detaches: bool,
    /// When the cap detaches relative to impact/penetration.
    pub detach_timing: CapDetachTiming,
    /// Projectile mass after cap detachment (grams).
    pub mass_after_detach_g: f64,
    /// Percentage change in ballistic coefficient (BC) due to cap loss.
    /// Negative means the BC decreases (worse aerodynamics).
    /// Typical range: -2 to -5 % for ballistic cap loss.
    pub bc_change_pct: f64,
    /// Percentage of velocity lost due to cap detachment (if any).
    /// The cap peeling off in flight or on impact may impart a small
    /// impulse. Typical range: 0-2 %.
    pub velocity_loss_pct: f64,
    /// Penetration penalty (percentage) for having a cap that does NOT
    /// detach — i.e., the ballistic cap crumpling on impact without
    /// contributing to penetration.  Only meaningful for ballistic cap
    /// that fails to detach.  Range: 0-15 %.
    pub penetration_penalty_pct: f64,
}

// ── Constants ──────────────────────────────────────────────────────────────────

/// Minimum velocity (m/s) for reliable ballistic cap detachment on impact.
/// Below this the cap may crumple rather than shear off cleanly.
const BALLISTIC_CAP_MIN_DETACH_VELOCITY_MS: f64 = 150.0;

/// Minimum velocity (m/s) for piercing cap detachment during penetration.
/// Below this the cap may not separate and could hinder penetration.
const PIERCING_CAP_MIN_DETACH_VELOCITY_MS: f64 = 300.0;

/// Typical BC change when a ballistic cap detaches (fraction, -0.02 to -0.05).
/// The exposed core has a different drag profile — typically blunter → higher
/// drag → lower BC.
const BALLISTIC_CAP_BC_CHANGE_MIN: f64 = -0.05;
const BALLISTIC_CAP_BC_CHANGE_MAX: f64 = -0.02;

/// Typical BC change when a piercing cap detaches (fraction).
/// Piercing caps are usually smaller relative to the core, so the
/// aerodynamic change is less pronounced.
const PIERCING_CAP_BC_CHANGE_MIN: f64 = -0.01;
const PIERCING_CAP_BC_CHANGE_MAX: f64 = 0.005;

/// Penetration improvement factor from a piercing cap (multiplicative).
/// A piercing cap provides ~5-15 % improvement in penetration depth.
const PIERCING_CAP_PEN_IMPROVEMENT_MIN: f64 = 1.05;
const PIERCING_CAP_PEN_IMPROVEMENT_MAX: f64 = 1.15;

/// If the ballistic cap fails to detach (low velocity), the crumpling
/// cap imposes a penetration penalty of up to 15 %.
const BALLISTIC_CAP_PENALTY_MAX_PCT: f64 = 15.0;

// ── Ballistic cap (windshield) evaluation ──────────────────────────────────────

/// Evaluate ballistic cap (windshield) detachment on impact.
///
/// The ballistic cap is an aerodynamic fairing fitted over the projectile
/// nose.  It improves flight characteristics (reduces drag) but is weak
/// and detaches immediately on striking any hard surface at typical impact
/// velocities.
///
/// On detachment:
/// - The exposed core has a higher drag profile → BC drops 2-5 %.
/// - The cap mass is shed, reducing total projectile mass slightly.
/// - Detachment may impart a tiny velocity perturbation (< 2 %).
/// - If the cap fails to detach (low velocity), it crumples and can
///   reduce penetration by up to 15 %.
pub fn evaluate_ballistic_cap(params: &BallisticCapParams) -> CapDetachResult {
    if !params.has_ballistic_cap || params.cap_mass_g <= 0.0 {
        return CapDetachResult {
            detaches: false,
            detach_timing: CapDetachTiming::None,
            mass_after_detach_g: params.projectile_mass_g,
            bc_change_pct: 0.0,
            velocity_loss_pct: 0.0,
            penetration_penalty_pct: 0.0,
        };
    }

    let total_mass = params.projectile_mass_g + params.cap_mass_g;

    // Check detachment conditions
    let detaches = params.impact_velocity_ms >= BALLISTIC_CAP_MIN_DETACH_VELOCITY_MS;
    let detach_timing = if detaches {
        CapDetachTiming::OnImpact
    } else {
        CapDetachTiming::None
    };

    // BC change: interpolate between min and max based on velocity
    let bc_change = if detaches {
        let vel_fraction =
            ((params.impact_velocity_ms - BALLISTIC_CAP_MIN_DETACH_VELOCITY_MS) / 1000.0).min(1.0);
        let range = BALLISTIC_CAP_BC_CHANGE_MAX - BALLISTIC_CAP_BC_CHANGE_MIN;
        BALLISTIC_CAP_BC_CHANGE_MIN + range * (1.0 - vel_fraction)
    } else {
        0.0
    };

    // Velocity loss: cap peeling off imparts a tiny impulse opposite
    // to the direction of flight. More prominent at higher velocities.
    let velocity_loss_pct = if detaches {
        let cap_mass_ratio = params.cap_mass_g / total_mass.max(1.0);
        // The cap shears off at an angle, producing a small retarding impulse
        cap_mass_ratio * 2.0 * (params.impact_velocity_ms / 1000.0).min(3.0)
    } else {
        0.0
    };

    // Penetration penalty: if the cap doesn't detach (low velocity),
    // it crumples and can hinder penetration.
    let penetration_penalty_pct = if !detaches && params.has_ballistic_cap {
        let vel_ratio = (BALLISTIC_CAP_MIN_DETACH_VELOCITY_MS - params.impact_velocity_ms)
            / BALLISTIC_CAP_MIN_DETACH_VELOCITY_MS;
        BALLISTIC_CAP_PENALTY_MAX_PCT * vel_ratio.min(1.0)
    } else {
        0.0
    };

    CapDetachResult {
        detaches,
        detach_timing,
        mass_after_detach_g: if detaches {
            params.projectile_mass_g
        } else {
            total_mass
        },
        bc_change_pct: bc_change * 100.0,
        velocity_loss_pct: velocity_loss_pct.min(2.0),
        penetration_penalty_pct: penetration_penalty_pct.min(15.0),
    }
}

// ── Piercing cap evaluation ────────────────────────────────────────────────────

/// Evaluate piercing cap (armour-piercing cap) behaviour.
///
/// The piercing cap is a hardened cap that covers the projectile nose
/// during flight and is designed to stay on through impact.  During the
/// initial stage of penetration it distributes impact forces over a wider
/// area, protecting the core from shattering and improving penetration
/// by 5-15 %.
///
/// On penetration:
/// - The cap cracks / detaches as the core pushes through.
/// - The exposed core continues with slightly lower drag (the cap is
///   generally larger than the core).
/// - Penetration is improved by 5-15 % while the cap is present.
///
/// If the cap fails to detach (very low velocity), it may act as a
/// blunt nose, reducing penetration.
pub fn evaluate_piercing_cap(params: &BallisticCapParams) -> CapDetachResult {
    if !params.has_piercing_cap || params.cap_mass_g <= 0.0 {
        return CapDetachResult {
            detaches: false,
            detach_timing: CapDetachTiming::None,
            mass_after_detach_g: params.projectile_mass_g,
            bc_change_pct: 0.0,
            velocity_loss_pct: 0.0,
            penetration_penalty_pct: 0.0,
        };
    }

    let total_mass = params.projectile_mass_g + params.cap_mass_g;

    // Piercing cap detaches during penetration (not on impact like BC).
    // Needs sufficient velocity to initiate the penetration process.
    let detaches = params.impact_velocity_ms >= PIERCING_CAP_MIN_DETACH_VELOCITY_MS;
    let detach_timing = if detaches {
        CapDetachTiming::OnPenetration
    } else {
        CapDetachTiming::None
    };

    // BC change: piercing caps are typically removed during penetration,
    // so the post-penetration projectile has slightly different aerodynamics.
    // The effect is small (< 1 %) since the cap was already in-flight.
    let bc_change = if detaches {
        let vel_fraction =
            ((params.impact_velocity_ms - PIERCING_CAP_MIN_DETACH_VELOCITY_MS) / 1000.0).min(1.0);
        let range = PIERCING_CAP_BC_CHANGE_MAX - PIERCING_CAP_BC_CHANGE_MIN;
        PIERCING_CAP_BC_CHANGE_MIN + range * vel_fraction
    } else {
        0.0
    };

    // Velocity loss from cap detachment during penetration is minimal
    // (the cap is shed into the armour, not backward).
    let velocity_loss_pct = if detaches {
        params.cap_mass_g / total_mass.max(1.0) * 0.5
    } else {
        0.0
    };

    // Penetration improvement: while the cap is on, penetration is better.
    // We express this as a *negative penalty* (i.e., a bonus).
    // The benefit is proportional to velocity and calibre.
    let pen_bonus_pct = if detaches {
        let vel_factor = (params.impact_velocity_ms / 1000.0).min(2.0);
        let cal_factor = (params.caliber_mm / 12.7).min(2.0);
        let improvement = PIERCING_CAP_PEN_IMPROVEMENT_MIN
            + (PIERCING_CAP_PEN_IMPROVEMENT_MAX - PIERCING_CAP_PEN_IMPROVEMENT_MIN)
                * ((vel_factor + cal_factor) / 4.0);
        // Return as a negative penalty (bonus)
        -(improvement - 1.0) * 100.0
    } else {
        // No bonus without detachment
        0.0
    };

    CapDetachResult {
        detaches,
        detach_timing,
        mass_after_detach_g: if detaches {
            params.projectile_mass_g
        } else {
            total_mass
        },
        bc_change_pct: bc_change * 100.0,
        velocity_loss_pct: velocity_loss_pct.min(1.0),
        penetration_penalty_pct: pen_bonus_pct,
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Ballistic cap tests ────────────────────────────────────────────────

    #[test]
    fn ballistic_cap_detaches_on_impact_at_high_velocity() {
        let params = BallisticCapParams {
            has_ballistic_cap: true,
            has_piercing_cap: false,
            cap_mass_g: 2.0,
            projectile_mass_g: 40.0,
            caliber_mm: 12.7,
            impact_velocity_ms: 850.0,
            impact_angle_deg: 0.0,
        };
        let result = evaluate_ballistic_cap(&params);
        assert!(result.detaches, "Ballistic cap should detach at 850 m/s");
        assert_eq!(
            result.detach_timing,
            CapDetachTiming::OnImpact,
            "Ballistic cap detaches on impact"
        );
        assert!(
            result.mass_after_detach_g < 42.0,
            "Mass should decrease after cap detachment"
        );
        assert!(
            result.bc_change_pct <= -2.0,
            "BC should drop by at least 2% after cap loss, got {:.2}%",
            result.bc_change_pct
        );
    }

    #[test]
    fn ballistic_cap_below_threshold_no_detach() {
        let params = BallisticCapParams {
            has_ballistic_cap: true,
            has_piercing_cap: false,
            cap_mass_g: 2.0,
            projectile_mass_g: 40.0,
            caliber_mm: 12.7,
            impact_velocity_ms: 100.0, // below min detach velocity
            impact_angle_deg: 0.0,
        };
        let result = evaluate_ballistic_cap(&params);
        assert!(
            !result.detaches,
            "Ballistic cap should NOT detach at 100 m/s"
        );
        assert!(
            result.penetration_penalty_pct > 0.0,
            "Non-detaching cap should impose a penetration penalty"
        );
    }

    #[test]
    fn no_ballistic_cap_returns_no_change() {
        let params = BallisticCapParams {
            has_ballistic_cap: false,
            has_piercing_cap: false,
            cap_mass_g: 0.0,
            projectile_mass_g: 42.0,
            caliber_mm: 12.7,
            impact_velocity_ms: 850.0,
            impact_angle_deg: 0.0,
        };
        let result = evaluate_ballistic_cap(&params);
        assert!(!result.detaches);
        assert_eq!(result.mass_after_detach_g, 42.0);
        assert_eq!(result.bc_change_pct, 0.0);
        assert_eq!(result.penetration_penalty_pct, 0.0);
    }

    // ── Piercing cap tests ─────────────────────────────────────────────────

    #[test]
    fn piercing_cap_detaches_during_penetration() {
        let params = BallisticCapParams {
            has_ballistic_cap: false,
            has_piercing_cap: true,
            cap_mass_g: 3.0,
            projectile_mass_g: 39.0,
            caliber_mm: 12.7,
            impact_velocity_ms: 850.0,
            impact_angle_deg: 0.0,
        };
        let result = evaluate_piercing_cap(&params);
        assert!(result.detaches, "Piercing cap should detach during pen");
        assert_eq!(
            result.detach_timing,
            CapDetachTiming::OnPenetration,
            "Piercing cap detaches during penetration"
        );
    }

    #[test]
    fn piercing_cap_improves_penetration() {
        let params = BallisticCapParams {
            has_ballistic_cap: false,
            has_piercing_cap: true,
            cap_mass_g: 3.0,
            projectile_mass_g: 39.0,
            caliber_mm: 12.7,
            impact_velocity_ms: 850.0,
            impact_angle_deg: 0.0,
        };
        let result = evaluate_piercing_cap(&params);
        // Penetration penalty is negative (a bonus)
        assert!(
            result.penetration_penalty_pct < 0.0,
            "Piercing cap should give a penetration bonus, got {:.2}%",
            result.penetration_penalty_pct
        );
        // The bonus should be between -5% and -15%
        assert!(
            result.penetration_penalty_pct >= -15.0,
            "Piercing cap bonus should not exceed 15%"
        );
        assert!(
            result.penetration_penalty_pct <= -3.0,
            "Piercing cap bonus should be at least 3%"
        );
    }

    #[test]
    fn no_piercing_cap_returns_no_change() {
        let params = BallisticCapParams {
            has_ballistic_cap: false,
            has_piercing_cap: false,
            cap_mass_g: 0.0,
            projectile_mass_g: 42.0,
            caliber_mm: 12.7,
            impact_velocity_ms: 850.0,
            impact_angle_deg: 0.0,
        };
        let result = evaluate_piercing_cap(&params);
        assert!(!result.detaches);
        assert_eq!(result.mass_after_detach_g, 42.0);
        assert_eq!(result.penetration_penalty_pct, 0.0);
    }

    // ── M8 API calibration (12.7 mm, ~42 g total, cap ~4 g) ────────────────

    #[test]
    fn m8_api_ballistic_cap_calibration() {
        // M8 API: 12.7 mm projectile, ~42 g total, cap ~4 g, MV ~890 m/s
        let params = BallisticCapParams {
            has_ballistic_cap: true,
            has_piercing_cap: false,
            cap_mass_g: 4.0,
            projectile_mass_g: 38.0,
            caliber_mm: 12.7,
            impact_velocity_ms: 890.0,
            impact_angle_deg: 0.0,
        };
        let result = evaluate_ballistic_cap(&params);
        assert!(result.detaches, "M8 API ballistic cap should detach");
        // Mass after detach should be 38 g (projectile without cap)
        assert!(
            (result.mass_after_detach_g - 38.0).abs() < 0.01,
            "M8 API mass after detach should be ~38 g, got {}",
            result.mass_after_detach_g
        );
        // BC drop should be in the 2-5 % range
        assert!(
            result.bc_change_pct <= -2.0 && result.bc_change_pct >= -5.0,
            "M8 API BC change should be -2 to -5%, got {:.2}%",
            result.bc_change_pct
        );
    }

    // ── High velocity detachment ───────────────────────────────────────────

    #[test]
    fn high_velocity_ballistic_cap_detach_reliable() {
        let params = BallisticCapParams {
            has_ballistic_cap: true,
            has_piercing_cap: false,
            cap_mass_g: 1.5,
            projectile_mass_g: 8.0,
            caliber_mm: 7.62,
            impact_velocity_ms: 1200.0, // very high
            impact_angle_deg: 0.0,
        };
        let result = evaluate_ballistic_cap(&params);
        assert!(result.detaches);
        // Higher velocity → more certain detachment → penalty should be 0
        assert_eq!(result.penetration_penalty_pct, 0.0);
    }

    // ── Cap mass effect ────────────────────────────────────────────────────

    #[test]
    fn heavier_cap_greater_bc_effect() {
        let light = evaluate_ballistic_cap(&BallisticCapParams {
            has_ballistic_cap: true,
            has_piercing_cap: false,
            cap_mass_g: 1.0,
            projectile_mass_g: 41.0,
            caliber_mm: 12.7,
            impact_velocity_ms: 850.0,
            impact_angle_deg: 0.0,
        });
        let heavy = evaluate_ballistic_cap(&BallisticCapParams {
            has_ballistic_cap: true,
            has_piercing_cap: false,
            cap_mass_g: 5.0,
            projectile_mass_g: 37.0,
            caliber_mm: 12.7,
            impact_velocity_ms: 850.0,
            impact_angle_deg: 0.0,
        });
        // Velocity loss from cap shedding is proportional to cap mass ratio
        assert!(
            heavy.velocity_loss_pct >= light.velocity_loss_pct,
            "Heavier cap should cause more velocity loss on detach"
        );
    }

    // ── Both caps present ──────────────────────────────────────────────────

    #[test]
    fn both_caps_evaluate_independently() {
        let params = BallisticCapParams {
            has_ballistic_cap: true,
            has_piercing_cap: true,
            cap_mass_g: 4.0,
            projectile_mass_g: 38.0,
            caliber_mm: 12.7,
            impact_velocity_ms: 850.0,
            impact_angle_deg: 0.0,
        };
        let bc_result = evaluate_ballistic_cap(&params);
        let pc_result = evaluate_piercing_cap(&params);
        assert!(bc_result.detaches, "Ballistic cap should detach on impact");
        assert!(
            pc_result.detaches,
            "Piercing cap should detach during penetration"
        );
        assert_eq!(bc_result.detach_timing, CapDetachTiming::OnImpact);
        assert_eq!(pc_result.detach_timing, CapDetachTiming::OnPenetration);
    }

    // ── Deterministic ──────────────────────────────────────────────────────

    #[test]
    fn deterministic_output() {
        let params = BallisticCapParams {
            has_ballistic_cap: true,
            has_piercing_cap: true,
            cap_mass_g: 3.0,
            projectile_mass_g: 39.0,
            caliber_mm: 12.7,
            impact_velocity_ms: 800.0,
            impact_angle_deg: 0.0,
        };
        let a = evaluate_ballistic_cap(&params);
        let b = evaluate_ballistic_cap(&params);
        assert_eq!(a.detaches, b.detaches);
        assert!((a.bc_change_pct - b.bc_change_pct).abs() < 1e-12);
        assert!((a.velocity_loss_pct - b.velocity_loss_pct).abs() < 1e-12);

        let c = evaluate_piercing_cap(&params);
        let d = evaluate_piercing_cap(&params);
        assert_eq!(c.detaches, d.detaches);
        assert!((c.penetration_penalty_pct - d.penetration_penalty_pct).abs() < 1e-12);
    }
}
