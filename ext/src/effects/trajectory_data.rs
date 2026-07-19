// ── Reference trajectory data ───────────────────────────────────────────────────

/// Reference trajectory data for a single projectile/load combination.
/// Contains down-range velocity and drop sampled at standard intervals.
#[derive(Debug, Clone)]
pub struct TrajectorySample {
    pub range_m: f64,
    pub velocity_ms: f64,
    pub drop_m: f64,
}

/// Reference ammunition data: known BC, MV, and source.
#[derive(Debug, Clone)]
pub struct AmmoReference {
    pub name: &'static str,
    pub mv_ms: f64,
    pub bc_g7: f64,
    pub mass_g: f64,
    pub caliber_mm: f64,
    pub source: &'static str,
    pub trajectory_samples: &'static [TrajectorySample],
}

/// M855 (5.56×45mm) reference trajectory. G7 BC = 0.151 @ MV = 930 m/s.
/// Velocity and drop sampled from JBM/BRL trajectory tables (ICAO, sea level).
const M855_TRAJECTORY: &[TrajectorySample] = &[
    TrajectorySample {
        range_m: 0.0,
        velocity_ms: 930.0,
        drop_m: 0.000,
    },
    TrajectorySample {
        range_m: 100.0,
        velocity_ms: 842.0,
        drop_m: 0.069,
    },
    TrajectorySample {
        range_m: 200.0,
        velocity_ms: 759.0,
        drop_m: 0.301,
    },
    TrajectorySample {
        range_m: 300.0,
        velocity_ms: 679.0,
        drop_m: 0.738,
    },
    TrajectorySample {
        range_m: 400.0,
        velocity_ms: 603.0,
        drop_m: 1.428,
    },
    TrajectorySample {
        range_m: 500.0,
        velocity_ms: 530.0,
        drop_m: 2.433,
    },
    TrajectorySample {
        range_m: 600.0,
        velocity_ms: 462.0,
        drop_m: 3.829,
    },
    TrajectorySample {
        range_m: 800.0,
        velocity_ms: 337.0,
        drop_m: 8.477,
    },
    TrajectorySample {
        range_m: 1000.0,
        velocity_ms: 277.0,
        drop_m: 16.43,
    },
];

/// M80 (7.62×51mm) reference trajectory. G7 BC = 0.200 @ MV = 850 m/s.
const M80_TRAJECTORY: &[TrajectorySample] = &[
    TrajectorySample {
        range_m: 0.0,
        velocity_ms: 850.0,
        drop_m: 0.000,
    },
    TrajectorySample {
        range_m: 100.0,
        velocity_ms: 787.0,
        drop_m: 0.059,
    },
    TrajectorySample {
        range_m: 200.0,
        velocity_ms: 727.0,
        drop_m: 0.245,
    },
    TrajectorySample {
        range_m: 300.0,
        velocity_ms: 669.0,
        drop_m: 0.576,
    },
    TrajectorySample {
        range_m: 400.0,
        velocity_ms: 614.0,
        drop_m: 1.066,
    },
    TrajectorySample {
        range_m: 500.0,
        velocity_ms: 562.0,
        drop_m: 1.736,
    },
    TrajectorySample {
        range_m: 600.0,
        velocity_ms: 513.0,
        drop_m: 2.608,
    },
    TrajectorySample {
        range_m: 800.0,
        velocity_ms: 424.0,
        drop_m: 5.249,
    },
    TrajectorySample {
        range_m: 1000.0,
        velocity_ms: 349.0,
        drop_m: 9.676,
    },
];

/// M33 (7.62×51mm FMJ ball, also used as .308 Win match) reference.
/// G7 BC = 0.210 @ MV = 830 m/s.
const M33_TRAJECTORY: &[TrajectorySample] = &[
    TrajectorySample {
        range_m: 0.0,
        velocity_ms: 830.0,
        drop_m: 0.000,
    },
    TrajectorySample {
        range_m: 100.0,
        velocity_ms: 771.0,
        drop_m: 0.061,
    },
    TrajectorySample {
        range_m: 200.0,
        velocity_ms: 714.0,
        drop_m: 0.252,
    },
    TrajectorySample {
        range_m: 300.0,
        velocity_ms: 659.0,
        drop_m: 0.591,
    },
    TrajectorySample {
        range_m: 400.0,
        velocity_ms: 607.0,
        drop_m: 1.093,
    },
    TrajectorySample {
        range_m: 500.0,
        velocity_ms: 557.0,
        drop_m: 1.778,
    },
    TrajectorySample {
        range_m: 600.0,
        velocity_ms: 510.0,
        drop_m: 2.668,
    },
    TrajectorySample {
        range_m: 800.0,
        velocity_ms: 423.0,
        drop_m: 5.354,
    },
    TrajectorySample {
        range_m: 1000.0,
        velocity_ms: 359.0,
        drop_m: 9.786,
    },
];

/// M193 (5.56×45mm) reference trajectory. G7 BC = 0.178 @ MV = 990 m/s.
/// Lighter, faster than M855; similar G7 BC due to different construction.
const M193_TRAJECTORY: &[TrajectorySample] = &[
    TrajectorySample {
        range_m: 0.0,
        velocity_ms: 990.0,
        drop_m: 0.000,
    },
    TrajectorySample {
        range_m: 100.0,
        velocity_ms: 901.0,
        drop_m: 0.061,
    },
    TrajectorySample {
        range_m: 200.0,
        velocity_ms: 817.0,
        drop_m: 0.272,
    },
    TrajectorySample {
        range_m: 300.0,
        velocity_ms: 733.0,
        drop_m: 0.680,
    },
    TrajectorySample {
        range_m: 400.0,
        velocity_ms: 655.0,
        drop_m: 1.345,
    },
    TrajectorySample {
        range_m: 500.0,
        velocity_ms: 583.0,
        drop_m: 2.340,
    },
    TrajectorySample {
        range_m: 600.0,
        velocity_ms: 519.0,
        drop_m: 3.750,
    },
    TrajectorySample {
        range_m: 800.0,
        velocity_ms: 407.0,
        drop_m: 8.451,
    },
    TrajectorySample {
        range_m: 1000.0,
        velocity_ms: 336.0,
        drop_m: 16.48,
    },
];

/// 9mm FMJ (9×19mm Parabellum) reference trajectory. G7 BC = 0.067 @ MV = 370 m/s.
/// Typical 124 gr FMJ RN pistol round.
const TRAJECTORY_9MM_FMJ: &[TrajectorySample] = &[
    TrajectorySample {
        range_m: 0.0,
        velocity_ms: 370.0,
        drop_m: 0.000,
    },
    TrajectorySample {
        range_m: 25.0,
        velocity_ms: 353.0,
        drop_m: 0.043,
    },
    TrajectorySample {
        range_m: 50.0,
        velocity_ms: 336.0,
        drop_m: 0.170,
    },
    TrajectorySample {
        range_m: 75.0,
        velocity_ms: 321.0,
        drop_m: 0.394,
    },
    TrajectorySample {
        range_m: 100.0,
        velocity_ms: 306.0,
        drop_m: 0.722,
    },
    TrajectorySample {
        range_m: 150.0,
        velocity_ms: 278.0,
        drop_m: 1.784,
    },
    TrajectorySample {
        range_m: 200.0,
        velocity_ms: 253.0,
        drop_m: 3.578,
    },
    TrajectorySample {
        range_m: 300.0,
        velocity_ms: 211.0,
        drop_m: 9.962,
    },
    TrajectorySample {
        range_m: 400.0,
        velocity_ms: 179.0,
        drop_m: 21.91,
    },
];

/// .338 Lapua Magnum reference trajectory. G7 BC = 0.320 @ MV = 880 m/s.
/// 250 gr FMJBT long-range sniper round.
const LAPUA_338_TRAJECTORY: &[TrajectorySample] = &[
    TrajectorySample {
        range_m: 0.0,
        velocity_ms: 880.0,
        drop_m: 0.000,
    },
    TrajectorySample {
        range_m: 100.0,
        velocity_ms: 838.0,
        drop_m: 0.051,
    },
    TrajectorySample {
        range_m: 200.0,
        velocity_ms: 798.0,
        drop_m: 0.208,
    },
    TrajectorySample {
        range_m: 300.0,
        velocity_ms: 760.0,
        drop_m: 0.478,
    },
    TrajectorySample {
        range_m: 400.0,
        velocity_ms: 724.0,
        drop_m: 0.869,
    },
    TrajectorySample {
        range_m: 500.0,
        velocity_ms: 689.0,
        drop_m: 1.392,
    },
    TrajectorySample {
        range_m: 600.0,
        velocity_ms: 655.0,
        drop_m: 2.058,
    },
    TrajectorySample {
        range_m: 800.0,
        velocity_ms: 592.0,
        drop_m: 4.038,
    },
    TrajectorySample {
        range_m: 1000.0,
        velocity_ms: 534.0,
        drop_m: 7.178,
    },
    TrajectorySample {
        range_m: 1200.0,
        velocity_ms: 481.0,
        drop_m: 11.81,
    },
    TrajectorySample {
        range_m: 1500.0,
        velocity_ms: 418.0,
        drop_m: 20.97,
    },
];

/// .50 BMG (12.7×99mm) M33 ball reference trajectory. G7 BC = 0.435 @ MV = 860 m/s.
const BMG_50_TRAJECTORY: &[TrajectorySample] = &[
    TrajectorySample {
        range_m: 0.0,
        velocity_ms: 860.0,
        drop_m: 0.000,
    },
    TrajectorySample {
        range_m: 100.0,
        velocity_ms: 834.0,
        drop_m: 0.049,
    },
    TrajectorySample {
        range_m: 200.0,
        velocity_ms: 809.0,
        drop_m: 0.197,
    },
    TrajectorySample {
        range_m: 300.0,
        velocity_ms: 785.0,
        drop_m: 0.446,
    },
    TrajectorySample {
        range_m: 400.0,
        velocity_ms: 762.0,
        drop_m: 0.799,
    },
    TrajectorySample {
        range_m: 500.0,
        velocity_ms: 739.0,
        drop_m: 1.260,
    },
    TrajectorySample {
        range_m: 600.0,
        velocity_ms: 717.0,
        drop_m: 1.835,
    },
    TrajectorySample {
        range_m: 800.0,
        velocity_ms: 674.0,
        drop_m: 3.578,
    },
    TrajectorySample {
        range_m: 1000.0,
        velocity_ms: 633.0,
        drop_m: 6.261,
    },
    TrajectorySample {
        range_m: 1200.0,
        velocity_ms: 594.0,
        drop_m: 10.04,
    },
    TrajectorySample {
        range_m: 1500.0,
        velocity_ms: 539.0,
        drop_m: 17.62,
    },
    TrajectorySample {
        range_m: 2000.0,
        velocity_ms: 455.0,
        drop_m: 36.24,
    },
];

/// 7.62×39mm LPS (57-N-231) reference trajectory. G7 BC = 0.194 @ MV = 730 m/s.
const LPS_762X39_TRAJECTORY: &[TrajectorySample] = &[
    TrajectorySample {
        range_m: 0.0,
        velocity_ms: 730.0,
        drop_m: 0.000,
    },
    TrajectorySample {
        range_m: 100.0,
        velocity_ms: 675.0,
        drop_m: 0.072,
    },
    TrajectorySample {
        range_m: 200.0,
        velocity_ms: 623.0,
        drop_m: 0.296,
    },
    TrajectorySample {
        range_m: 300.0,
        velocity_ms: 574.0,
        drop_m: 0.707,
    },
    TrajectorySample {
        range_m: 400.0,
        velocity_ms: 528.0,
        drop_m: 1.348,
    },
    TrajectorySample {
        range_m: 500.0,
        velocity_ms: 485.0,
        drop_m: 2.269,
    },
    TrajectorySample {
        range_m: 600.0,
        velocity_ms: 446.0,
        drop_m: 3.540,
    },
    TrajectorySample {
        range_m: 800.0,
        velocity_ms: 376.0,
        drop_m: 7.691,
    },
    TrajectorySample {
        range_m: 1000.0,
        velocity_ms: 318.0,
        drop_m: 14.80,
    },
];

/// 5.45×39mm 7N6 reference trajectory. G7 BC = 0.145 @ MV = 900 m/s.
const TRAJECTORY_545_7N6: &[TrajectorySample] = &[
    TrajectorySample {
        range_m: 0.0,
        velocity_ms: 900.0,
        drop_m: 0.000,
    },
    TrajectorySample {
        range_m: 100.0,
        velocity_ms: 809.0,
        drop_m: 0.074,
    },
    TrajectorySample {
        range_m: 200.0,
        velocity_ms: 724.0,
        drop_m: 0.317,
    },
    TrajectorySample {
        range_m: 300.0,
        velocity_ms: 645.0,
        drop_m: 0.767,
    },
    TrajectorySample {
        range_m: 400.0,
        velocity_ms: 571.0,
        drop_m: 1.466,
    },
    TrajectorySample {
        range_m: 500.0,
        velocity_ms: 503.0,
        drop_m: 2.466,
    },
    TrajectorySample {
        range_m: 600.0,
        velocity_ms: 441.0,
        drop_m: 3.836,
    },
    TrajectorySample {
        range_m: 800.0,
        velocity_ms: 332.0,
        drop_m: 8.495,
    },
    TrajectorySample {
        range_m: 1000.0,
        velocity_ms: 265.0,
        drop_m: 16.90,
    },
];

/// Reference ammunition database.
pub fn ammo_references() -> Vec<AmmoReference> {
    vec![
        AmmoReference {
            name: "5.56×45mm M855",
            mv_ms: 930.0,
            bc_g7: 0.151,
            mass_g: 4.0,
            caliber_mm: 5.56,
            source: "APG/US Army BRL (ARL-TR-5182)",
            trajectory_samples: M855_TRAJECTORY,
        },
        AmmoReference {
            name: "7.62×51mm M80",
            mv_ms: 850.0,
            bc_g7: 0.200,
            mass_g: 9.5,
            caliber_mm: 7.62,
            source: "APG/US Army BRL (AD0815788)",
            trajectory_samples: M80_TRAJECTORY,
        },
        AmmoReference {
            name: "7.62×51mm M33 (FMJ Ball)",
            mv_ms: 830.0,
            bc_g7: 0.210,
            mass_g: 9.5,
            caliber_mm: 7.62,
            source: "Sierra/NRA match data",
            trajectory_samples: M33_TRAJECTORY,
        },
        AmmoReference {
            name: "5.56×45mm M193",
            mv_ms: 990.0,
            bc_g7: 0.178,
            mass_g: 4.0,
            caliber_mm: 5.56,
            source: "JBM Ballistics / US Army DARCOM",
            trajectory_samples: M193_TRAJECTORY,
        },
        AmmoReference {
            name: "9×19mm FMJ (124 gr)",
            mv_ms: 370.0,
            bc_g7: 0.067,
            mass_g: 8.0,
            caliber_mm: 9.0,
            source: "Sierra/NRA pistol data",
            trajectory_samples: TRAJECTORY_9MM_FMJ,
        },
        AmmoReference {
            name: ".338 Lapua Magnum",
            mv_ms: 880.0,
            bc_g7: 0.320,
            mass_g: 16.2,
            caliber_mm: 8.58,
            source: "Lapua / JBM Ballistics",
            trajectory_samples: LAPUA_338_TRAJECTORY,
        },
        AmmoReference {
            name: ".50 BMG M33 ball",
            mv_ms: 860.0,
            bc_g7: 0.435,
            mass_g: 42.8,
            caliber_mm: 12.7,
            source: "US MIL-DTL-4025 / JBM",
            trajectory_samples: BMG_50_TRAJECTORY,
        },
        AmmoReference {
            name: "7.62×39mm LPS (57-N-231)",
            mv_ms: 730.0,
            bc_g7: 0.194,
            mass_g: 7.9,
            caliber_mm: 7.62,
            source: "Russian Ammo Tech Data / JBM",
            trajectory_samples: LPS_762X39_TRAJECTORY,
        },
        AmmoReference {
            name: "5.45×39mm 7N6",
            mv_ms: 900.0,
            bc_g7: 0.145,
            mass_g: 3.4,
            caliber_mm: 5.45,
            source: "Russian Ammo Tech Data / ARL-TR-5182",
            trajectory_samples: TRAJECTORY_545_7N6,
        },
    ]
}

// ── Trajectory validation utilities ────────────────────────────────────────────

/// Result of comparing a simulated trajectory against reference data.
#[derive(Debug, Clone)]
pub struct TrajectoryValidationResult {
    pub ammo_name: &'static str,
    pub rmse_velocity_ms: f64,
    pub rmse_drop_m: f64,
    pub max_velocity_error_ms: f64,
    pub max_drop_error_m: f64,
    pub samples_compared: usize,
}

/// Check that a reference trajectory is internally consistent:
/// velocity decreases monotonically and drop increases monotonically.
pub fn check_trajectory_monotonic(ammo: &AmmoReference) -> bool {
    let samples = ammo.trajectory_samples;
    if samples.len() < 2 {
        return true; // trivially monotonic
    }

    // Velocity must never increase with range
    for i in 1..samples.len() {
        if samples[i].velocity_ms > samples[i - 1].velocity_ms + 0.5 {
            return false;
        }
    }

    // Drop must never decrease with range (drop is positive downward)
    for i in 1..samples.len() {
        if samples[i].drop_m < samples[i - 1].drop_m - 0.001 {
            return false;
        }
    }

    true
}

/// Linear-interpolate velocity and drop at an arbitrary range from
/// the reference trajectory samples.
pub fn interpolate_trajectory(ammo: &AmmoReference, range_m: f64) -> (f64, f64) {
    let samples = ammo.trajectory_samples;
    if samples.is_empty() {
        return (0.0, 0.0);
    }

    // Before first sample: use first values
    if range_m <= samples[0].range_m {
        return (samples[0].velocity_ms, samples[0].drop_m);
    }

    // After last sample: use last values
    if range_m >= samples[samples.len() - 1].range_m {
        let last = &samples[samples.len() - 1];
        return (last.velocity_ms, last.drop_m);
    }

    // Find the two bracketing samples and interpolate
    for i in 1..samples.len() {
        if range_m <= samples[i].range_m {
            let r0 = samples[i - 1].range_m;
            let r1 = samples[i].range_m;
            let t = (range_m - r0) / (r1 - r0);
            let vel = samples[i - 1].velocity_ms
                + t * (samples[i].velocity_ms - samples[i - 1].velocity_ms);
            let drop = samples[i - 1].drop_m + t * (samples[i].drop_m - samples[i - 1].drop_m);
            return (vel, drop);
        }
    }

    // Should not reach here
    let last = &samples[samples.len() - 1];
    (last.velocity_ms, last.drop_m)
}

/// Compute the kinetic energy (J) of the projectile at a given range
/// by interpolating velocity from the reference trajectory.
pub fn compute_energy_at_range(ammo: &AmmoReference, range_m: f64) -> f64 {
    let (vel_ms, _) = interpolate_trajectory(ammo, range_m);
    let mass_kg = ammo.mass_g / 1000.0;
    0.5 * mass_kg * vel_ms.powi(2)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ammo_references_available() {
        let refs = ammo_references();
        assert!(!refs.is_empty(), "Should have at least one reference");
        for ammo in &refs {
            assert!(
                ammo.mv_ms > 200.0,
                "All ammo should have MV > 200 m/s: {}",
                ammo.mv_ms
            );
            assert!(ammo.bc_g7 > 0.05);
            assert!(!ammo.trajectory_samples.is_empty());
            assert!(
                (ammo.trajectory_samples[0].range_m).abs() < 0.001,
                "First sample should be at range 0"
            );
            for i in 1..ammo.trajectory_samples.len() {
                assert!(
                    ammo.trajectory_samples[i].velocity_ms
                        <= ammo.trajectory_samples[i - 1].velocity_ms + 1.0,
                    "Velocity should not increase with range"
                );
            }
        }
    }

    #[test]
    fn m855_trajectory_data_plausible() {
        let refs = ammo_references();
        let m855 = refs.iter().find(|a| a.name.contains("M855")).unwrap();
        let at_500 = m855
            .trajectory_samples
            .iter()
            .find(|s| (s.range_m - 500.0).abs() < 1.0)
            .unwrap();
        assert!(
            at_500.velocity_ms > 400.0 && at_500.velocity_ms < 650.0,
            "M855 at 500 m should be ~530 m/s: {}",
            at_500.velocity_ms
        );
    }

    #[test]
    fn new_ammo_references_available() {
        let refs = ammo_references();
        assert!(
            refs.len() >= 9,
            "Should have at least 9 ammo references: {}",
            refs.len()
        );
        let names: Vec<&str> = refs.iter().map(|a| a.name).collect();
        assert!(
            names.iter().any(|n| n.contains("M193")),
            "M193 should be in references"
        );
        assert!(
            names.iter().any(|n| n.contains("9×19mm")),
            "9mm should be in references"
        );
        assert!(
            names.iter().any(|n| n.contains(".338 Lapua")),
            ".338 Lapua should be in references"
        );
        assert!(
            names.iter().any(|n| n.contains(".50 BMG")),
            ".50 BMG should be in references"
        );
        assert!(
            names.iter().any(|n| n.contains("7.62×39mm")),
            "7.62×39mm LPS should be in references"
        );
        assert!(
            names.iter().any(|n| n.contains("5.45×39mm")),
            "5.45×39mm 7N6 should be in references"
        );
    }

    #[test]
    fn new_ammo_trajectories_plausible() {
        let refs = ammo_references();
        for ammo in &refs {
            assert!(!ammo.trajectory_samples.is_empty());
            assert!((ammo.trajectory_samples[0].range_m).abs() < 0.001);
            assert!(
                (ammo.trajectory_samples[0].velocity_ms - ammo.mv_ms).abs() < 1.0,
                "{}: first sample vel {} ≈ MV {}",
                ammo.name,
                ammo.trajectory_samples[0].velocity_ms,
                ammo.mv_ms
            );
        }
    }

    #[test]
    fn m193_trajectory_at_500m() {
        let refs = ammo_references();
        let m193 = refs.iter().find(|a| a.name.contains("M193")).unwrap();
        let at_500 = m193
            .trajectory_samples
            .iter()
            .find(|s| (s.range_m - 500.0).abs() < 1.0)
            .unwrap();
        assert!(
            at_500.velocity_ms > 500.0 && at_500.velocity_ms < 650.0,
            "M193 at 500 m should be ~583 m/s: {}",
            at_500.velocity_ms
        );
    }

    #[test]
    fn lapua_338_trajectory_retains_high_velocity() {
        let refs = ammo_references();
        let lapua = refs.iter().find(|a| a.name.contains(".338 Lapua")).unwrap();
        let at_1000 = lapua
            .trajectory_samples
            .iter()
            .find(|s| (s.range_m - 1000.0).abs() < 1.0)
            .unwrap();
        assert!(
            at_1000.velocity_ms > 500.0,
            ".338 Lapua at 1000m should be supersonic: {} m/s",
            at_1000.velocity_ms
        );
    }

    #[test]
    fn check_trajectory_monotonic_all_ammo() {
        let refs = ammo_references();
        for ammo in &refs {
            assert!(
                check_trajectory_monotonic(ammo),
                "{} trajectory should be monotonic",
                ammo.name
            );
        }
    }

    #[test]
    fn interpolate_m855_at_150m() {
        let refs = ammo_references();
        let m855 = refs.iter().find(|a| a.name.contains("M855")).unwrap();
        let (vel, drop) = interpolate_trajectory(m855, 150.0);
        assert!(
            (vel - 801.0).abs() < 5.0,
            "M855 at 150m should be ~801 m/s: {}",
            vel
        );
        assert!(
            drop > 0.05 && drop < 0.35,
            "M855 at 150m drop should be interpolated: {}",
            drop
        );
    }

    #[test]
    fn energy_at_range_m855_muzzle() {
        let refs = ammo_references();
        let m855 = refs.iter().find(|a| a.name.contains("M855")).unwrap();
        let energy = compute_energy_at_range(m855, 0.0);
        assert!(
            (energy - 1725.0).abs() < 20.0,
            "M855 muzzle energy ~1725 J: {}",
            energy
        );
    }

    #[test]
    fn interpolate_before_first_sample() {
        let refs = ammo_references();
        let m855 = refs.iter().find(|a| a.name.contains("M855")).unwrap();
        let (vel, drop) = interpolate_trajectory(m855, -10.0);
        assert!(
            (vel - 930.0).abs() < 1.0,
            "Should use first sample velocity"
        );
        assert!((drop).abs() < 0.001, "Drop should be 0 at/before 0m");
    }

    #[test]
    fn interpolate_after_last_sample() {
        let refs = ammo_references();
        let m855 = refs.iter().find(|a| a.name.contains("M855")).unwrap();
        let (vel, drop) = interpolate_trajectory(m855, 2000.0);
        assert!(vel > 0.0, "Should return last valid velocity");
        assert!(drop > 16.0, "Drop should exceed last sample drop");
    }

    #[test]
    fn energy_decreases_with_range() {
        let refs = ammo_references();
        for ammo in &refs {
            let e0 = compute_energy_at_range(ammo, 0.0);
            let e_mid = compute_energy_at_range(ammo, 100.0);
            assert!(
                e_mid <= e0 + 1.0,
                "{} energy should decrease with range: {:.1} → {:.1}",
                ammo.name,
                e0,
                e_mid
            );
        }
    }

    #[test]
    fn compute_energy_all_ammo_types() {
        let refs = ammo_references();
        for ammo in &refs {
            let e = compute_energy_at_range(ammo, 0.0);
            assert!(
                e > 100.0,
                "{} muzzle energy should be > 100 J: {:.0} J",
                ammo.name,
                e
            );
        }
    }
}
