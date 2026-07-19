// ── NIJ / STANAG Reference Data ─────────────────────────────────────────────────

/// NIJ 0101.06 ballistic resistance threat levels for body armour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NIJLevel {
    /// Level IIA — 9mm FMJ RN @ 373 m/s, .40 S&W FMJ @ 325 m/s
    IIA,
    /// Level II — 9mm FMJ RN @ 398 m/s, .357 Mag JSP @ 430 m/s
    II,
    /// Level IIIA — .357 SIG FMJ FN @ 448 m/s, .44 Mag SJHP @ 436 m/s
    IIIA,
    /// Level III — 7.62×51mm M80 ball @ 847 m/s (rifle)
    III,
    /// Level IV — .30‑06 M2 AP @ 878 m/s (armour-piercing rifle)
    IV,
}

/// STANAG 4569 protection levels for logistic and light armoured vehicles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum STANAGLevel {
    /// Level 1 — 7.62×51mm M80 ball @ 833 m/s, 5.56×45mm M855 @ 900 m/s,
    /// and 155 mm HE blast at 10 m.
    L1,
    /// Level 2 — 7.62×39mm API BZ @ 695 m/s, plus Level 1 threats.
    L2,
    /// Level 3 — 7.62×51mm AP (M61) @ 838 m/s, plus Level 2 threats.
    L3,
    /// Level 4 — 14.5×114mm AP B32 @ 911 m/s, plus Level 3 threats.
    L4,
    /// Level 5 — 25 mm APDS-T @ 1258 m/s (Bushmaster chain gun), plus Level 4.
    L5,
}

/// Standard threat parameters for each NIJ level.
pub struct NIJThreat {
    pub level: NIJLevel,
    pub label: &'static str,
    pub velocity_ms: f64,
    pub mass_g: f64,
    pub caliber_m: f64,
}

/// Look up the reference threat for a given NIJ level.
pub fn nij_threat(level: NIJLevel) -> NIJThreat {
    match level {
        NIJLevel::IIA => NIJThreat {
            level: NIJLevel::IIA,
            label: "9mm FMJ RN (124 gr)",
            velocity_ms: 373.0,
            mass_g: 8.0,
            caliber_m: 0.00901,
        },
        NIJLevel::II => NIJThreat {
            level: NIJLevel::II,
            label: "9mm FMJ RN (124 gr)",
            velocity_ms: 398.0,
            mass_g: 8.0,
            caliber_m: 0.00901,
        },
        NIJLevel::IIIA => NIJThreat {
            level: NIJLevel::IIIA,
            label: ".44 Mag SJHP (240 gr)",
            velocity_ms: 436.0,
            mass_g: 15.6,
            caliber_m: 0.0109,
        },
        NIJLevel::III => NIJThreat {
            level: NIJLevel::III,
            label: "7.62×51mm M80 ball (147 gr)",
            velocity_ms: 847.0,
            mass_g: 9.5,
            caliber_m: 0.00782,
        },
        NIJLevel::IV => NIJThreat {
            level: NIJLevel::IV,
            label: ".30‑06 M2 AP (165 gr)",
            velocity_ms: 878.0,
            mass_g: 10.7,
            caliber_m: 0.00762,
        },
    }
}

/// Standard threat parameters for each STANAG 4569 level.
pub struct STANAGThreat {
    pub level: STANAGLevel,
    pub label: &'static str,
    pub velocity_ms: f64,
    pub mass_g: f64,
    pub caliber_m: f64,
}

/// Look up the reference KE threat for a given STANAG 4569 level.
pub fn stanag_threat(level: STANAGLevel) -> STANAGThreat {
    match level {
        STANAGLevel::L1 => STANAGThreat {
            level: STANAGLevel::L1,
            label: "7.62×51mm M80 ball",
            velocity_ms: 833.0,
            mass_g: 9.5,
            caliber_m: 0.00782,
        },
        STANAGLevel::L2 => STANAGThreat {
            level: STANAGLevel::L2,
            label: "7.62×39mm API BZ",
            velocity_ms: 695.0,
            mass_g: 7.9,
            caliber_m: 0.00762,
        },
        STANAGLevel::L3 => STANAGThreat {
            level: STANAGLevel::L3,
            label: "7.62×51mm AP M61",
            velocity_ms: 838.0,
            mass_g: 10.0,
            caliber_m: 0.00762,
        },
        STANAGLevel::L4 => STANAGThreat {
            level: STANAGLevel::L4,
            label: "14.5×114mm AP B32",
            velocity_ms: 911.0,
            mass_g: 64.0,
            caliber_m: 0.0145,
        },
        STANAGLevel::L5 => STANAGThreat {
            level: STANAGLevel::L5,
            label: "25mm APDS-T",
            velocity_ms: 1258.0,
            mass_g: 132.0,
            caliber_m: 0.0250,
        },
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nij_levels_defined() {
        for level in &[
            NIJLevel::IIA,
            NIJLevel::II,
            NIJLevel::IIIA,
            NIJLevel::III,
            NIJLevel::IV,
        ] {
            let threat = nij_threat(*level);
            assert!(threat.velocity_ms > 300.0);
            assert!(threat.mass_g > 5.0);
            assert!(threat.caliber_m > 0.005);
        }
    }

    #[test]
    fn nij_level_iii_is_rifle() {
        let threat = nij_threat(NIJLevel::III);
        assert!(threat.velocity_ms > 800.0);
        assert!((threat.caliber_m - 0.00782).abs() < 0.001);
    }

    #[test]
    fn stanag_levels_defined() {
        for level in &[
            STANAGLevel::L1,
            STANAGLevel::L2,
            STANAGLevel::L3,
            STANAGLevel::L4,
            STANAGLevel::L5,
        ] {
            let threat = stanag_threat(*level);
            assert!(
                threat.velocity_ms > 600.0,
                "L{:?} velocity = {}",
                level,
                threat.velocity_ms
            );
            assert!(threat.mass_g > 5.0);
        }
    }

    #[test]
    fn stanag_l4_threat_is_heavy() {
        let threat = stanag_threat(STANAGLevel::L4);
        assert!(
            threat.mass_g > 50.0,
            "14.5mm should be heavy: {} g",
            threat.mass_g
        );
        assert!(threat.caliber_m > 0.012, "Caliber should be > 12 mm");
    }
}
