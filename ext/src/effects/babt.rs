// ── Behind-Armor Blunt Trauma (BABT) ──────────────────────────────────────────

/// Result of a behind-armor blunt trauma (BABT) evaluation for non-penetrating
/// hits on soft body armour.
#[derive(Debug, Clone, Copy)]
pub struct BABTResult {
    /// Backface deformation (mm) — the maximum indentation into the body.
    pub backface_deformation_mm: f64,
    /// Blunt trauma energy transmitted through the armour (J).
    pub blunt_energy_j: f64,
    /// Qualitative injury severity.
    pub injury_severity: &'static str,
    /// Probability of rib fracture (0.0–1.0).
    pub rib_fracture_probability: f64,
    /// Risk of liver/spleen injury (0.0–1.0).
    pub liver_spleen_risk: f64,
}

/// Evaluate behind-armor blunt trauma for non-penetrating hits on soft body armor.
///
/// Physics:
/// - Backface deformation (BFD) is proportional to impulse transfer to the armour:
///   BFD ≈ k_bfd × KE^0.5 / (thickness × density^0.5)
/// - Blunt trauma energy: E_blunt = KE × exp(-C × thickness × density^0.5)
/// - For aramid: C ≈ 800; UHMWPE: C ≈ 1200; steel: C ≈ 3000.
///
/// # Arguments
/// * `velocity_ms` — Impact velocity (m/s).
/// * `mass_g` — Projectile mass (g).
/// * `caliber_m` — Projectile diameter (m).
/// * `projectile_type` — Construction identifier.
/// * `armor_thickness_mm` — Armour thickness (mm).
/// * `armor_material` — Armour material ("aramid", "uhmwpe", "steel", etc.).
/// * `standoff_mm` — Gap between armour and body (mm, for plate carriers).
pub fn evaluate_babt(
    velocity_ms: f64,
    mass_g: f64,
    _caliber_m: f64,
    projectile_type: &str,
    armor_thickness_mm: f64,
    armor_material: &str,
    standoff_mm: f64,
) -> BABTResult {
    if velocity_ms <= 0.0 || mass_g <= 0.0 || armor_thickness_mm <= 0.0 {
        return BABTResult {
            backface_deformation_mm: 0.0,
            blunt_energy_j: 0.0,
            injury_severity: "none",
            rib_fracture_probability: 0.0,
            liver_spleen_risk: 0.0,
        };
    }

    let mass_kg = mass_g / 1000.0;
    let ke = 0.5 * mass_kg * velocity_ms.powi(2);

    // ── Material-dependent constants ────────────────────────────────────
    let mat_lower = armor_material.to_lowercase();
    let (k_bfd, c_factor, density): (f64, f64, f64) = match mat_lower.as_str() {
        "aramid" | "kevlar" | "twaron" => (0.00035, 800.0, 1440.0),
        "uhmwpe" | "dyneema" | "spectra" => (0.00025, 1200.0, 970.0),
        "steel" | "steel_rha" | "hha" => (0.00003, 3000.0, 7850.0),
        "ceramic" | "b4c" | "sic" | "al2o3" => (0.00010, 2000.0, 3200.0),
        "titanium" | "ti" => (0.00008, 2500.0, 4430.0),
        _ => (0.00030, 900.0, 1400.0), // default: aramid-like
    };

    let thickness_m = armor_thickness_mm / 1000.0;

    // ── Backface deformation ────────────────────────────────────────────
    // BFD ≈ k_bfd × KE^0.5 / (thickness × density^0.5)
    let bfd_m = if thickness_m > 0.0 && density > 0.0 {
        k_bfd * ke.sqrt() / (thickness_m * density.sqrt())
    } else {
        0.0
    };
    let backface_deformation_mm = (bfd_m * 1000.0).min(80.0); // cap at 80mm (beyond NIJ limits)

    // ── Blunt trauma energy ─────────────────────────────────────────────
    // E_blunt = KE × exp(-C × thickness × density^0.5)
    let blunt_energy_j = ke * (-c_factor * thickness_m * density.sqrt()).exp();

    // Standoff gap reduces transmitted energy (air gap allows BFD to expand before hitting body)
    let standoff_factor = if standoff_mm > 0.0 && backface_deformation_mm > 0.0 {
        // If standoff > BFD, no energy transferred to body
        if standoff_mm >= backface_deformation_mm {
            0.0
        } else {
            1.0 - (standoff_mm / backface_deformation_mm)
        }
    } else {
        1.0
    };
    let blunt_energy_j = blunt_energy_j * standoff_factor;

    // ── Injury severity ─────────────────────────────────────────────────
    // E_blunt < 15J: minor
    // E_blunt 15-40J: moderate
    // E_blunt 40-80J: severe
    // E_blunt > 80J: critical
    let (injury_severity, rib_fracture_probability, liver_spleen_risk) = if blunt_energy_j < 15.0 {
        ("minor", blunt_energy_j / 30.0, 0.0)
    } else if blunt_energy_j < 40.0 {
        (
            "moderate",
            0.3 + (blunt_energy_j - 15.0) / 50.0,
            blunt_energy_j / 100.0,
        )
    } else if blunt_energy_j < 80.0 {
        (
            "severe",
            0.5 + (blunt_energy_j - 40.0) / 80.0,
            (blunt_energy_j - 30.0) / 100.0,
        )
    } else {
        (
            "critical",
            0.95_f64.min(0.5 + blunt_energy_j / 160.0),
            0.5_f64.min(blunt_energy_j / 160.0),
        )
    };

    // Projectile type modifier: blunt/deforming bullets transmit more energy
    let proj_lower = projectile_type.to_lowercase();
    let proj_mod = match proj_lower.as_str() {
        "soft_point" | "hollow_point" | "sp" => 1.3,
        "fmj" | "ball" => 1.0,
        "ap" | "armor_piercing" => 0.7,
        _ => 1.0,
    };

    let rib_fracture_probability = (rib_fracture_probability * proj_mod).min(1.0);
    let liver_spleen_risk = (liver_spleen_risk * proj_mod).min(1.0);

    BABTResult {
        backface_deformation_mm,
        blunt_energy_j,
        injury_severity,
        rib_fracture_probability,
        liver_spleen_risk,
    }
}

/// NIJ backface deformation compliance check.
/// Level IIA/II/IIIA: BFD must not exceed 44mm.
pub fn nij_bfd_compliant(bfd_mm: f64, nij_level: &str) -> bool {
    let max_bfd = match nij_level.to_lowercase().as_str() {
        "ii" | "iia" | "iiia" => 44.0,
        "iii" | "iv" => 44.0, // rifle plates have less strict BFD limits
        _ => 44.0,
    };
    bfd_mm <= max_bfd
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn babt_9mm_vs_niijiiia_aramid() {
        let r = evaluate_babt(360.0, 8.0, 0.00901, "fmj", 6.0, "aramid", 0.0);
        assert!(
            r.backface_deformation_mm > 10.0 && r.backface_deformation_mm < 44.0,
            "9mm vs IIIA aramid BFD should be within NIJ limits: {:.1} mm",
            r.backface_deformation_mm
        );
        assert!(
            r.blunt_energy_j > 0.0,
            "Should have some blunt trauma energy"
        );
    }

    #[test]
    fn babt_44mag_vs_niijiiia() {
        let r = evaluate_babt(436.0, 15.6, 0.0109, "fmj", 6.0, "aramid", 0.0);
        assert!(
            r.backface_deformation_mm > 30.0,
            ".44 Mag BFD should be significant: {:.1} mm",
            r.backface_deformation_mm
        );
    }

    #[test]
    fn babt_9mm_vs_steel_plate() {
        let r = evaluate_babt(360.0, 8.0, 0.00901, "fmj", 3.0, "steel", 0.0);
        assert!(
            r.backface_deformation_mm < 10.0,
            "Steel plate BFD should be minimal: {:.3} mm",
            r.backface_deformation_mm
        );
        assert!(
            r.blunt_energy_j < 20.0,
            "Steel transmits minimal blunt energy"
        );
    }

    #[test]
    fn babt_standoff_reduces_injury() {
        let no_standoff = evaluate_babt(400.0, 10.0, 0.00901, "fmj", 6.0, "aramid", 0.0);
        let with_standoff = evaluate_babt(400.0, 10.0, 0.00901, "fmj", 6.0, "aramid", 20.0);
        assert!(
            with_standoff.blunt_energy_j <= no_standoff.blunt_energy_j,
            "Standoff should reduce blunt energy"
        );
    }

    #[test]
    fn nij_bfd_compliant_check() {
        assert!(nij_bfd_compliant(30.0, "iiia"));
        assert!(nij_bfd_compliant(44.0, "iiia"));
        assert!(!nij_bfd_compliant(45.0, "iiia"));
        assert!(nij_bfd_compliant(20.0, "II"));
    }

    #[test]
    fn babt_zero_velocity_no_injury() {
        let r = evaluate_babt(0.0, 8.0, 0.00901, "fmj", 6.0, "aramid", 0.0);
        assert_eq!(r.backface_deformation_mm, 0.0);
        assert_eq!(r.blunt_energy_j, 0.0);
        assert_eq!(r.injury_severity, "none");
    }

    #[test]
    fn babt_injury_severity_scales_with_energy() {
        let low = evaluate_babt(200.0, 4.0, 0.00556, "fmj", 6.0, "aramid", 0.0);
        let high = evaluate_babt(800.0, 10.0, 0.00762, "fmj", 6.0, "aramid", 0.0);
        let severity_order: Vec<&str> = vec!["minor", "moderate", "severe", "critical"];
        let low_idx = severity_order
            .iter()
            .position(|&s| s == low.injury_severity)
            .unwrap_or(0);
        let high_idx = severity_order
            .iter()
            .position(|&s| s == high.injury_severity)
            .unwrap_or(0);
        assert!(
            high_idx >= low_idx,
            "Higher energy should have equal or greater severity"
        );
    }

    #[test]
    fn uhmwpe_lower_bfd_than_aramid() {
        let aramid = evaluate_babt(400.0, 10.0, 0.00901, "fmj", 6.0, "aramid", 0.0);
        let uhmwpe = evaluate_babt(400.0, 10.0, 0.00901, "fmj", 6.0, "uhmwpe", 0.0);
        assert!(
            uhmwpe.backface_deformation_mm <= aramid.backface_deformation_mm + 1.0,
            "UHMWPE BFD ({:.1}) should be <= aramid BFD ({:.1})",
            uhmwpe.backface_deformation_mm,
            aramid.backface_deformation_mm
        );
    }
}
