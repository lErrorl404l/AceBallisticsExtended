// ABE — Refined Crew Injury Model from Behind-Armour Debris (BAD / Spall)
//
// Upgraded fragment-level injury model: distributes BAD fragments by body
// region, applies region-specific lethality curves (MIL-HDBK-799 / STANAG 4569),
// and returns granular wound counts by severity.
//
// References:
//   - MIL-HDBK-799 (Vehicle Vulnerability — Component Kill Criteria)
//   - NATO STANAG 4569 Annex D (Crew Vulnerability to Spall)
//   - Fairlie J.P., "Behind Armour Debris Modelling" (DERA)
//   - DMSS (Defence Modelling & Simulation) Handbook, UK MoD

use super::component_damage::{CREW_INJURY_THRESHOLD_J, CrewProtection, spall_protection_factor};

// ── Deterministic random (threshold) helper ───────────────────────────────────

/// Returns `true` when probability `p` exceeds 0.5.
///
/// This is a **deterministic** threshold: probabilities above 0.5 always
/// trigger, those below never do.  Unit tests with clear energy margins
/// are therefore stable and repeatable.
///
/// A game integration should replace this with a seeded PRNG
/// (e.g. oorandom) seeded once at mission start.  The 0.5 split provides
/// a useful default: moderate threats (p ≈ 0.3–0.7) are decided by the
/// caller's energy-threshold logic rather than by randomness, while
/// high-confidence events (p > 0.5) always fire.
pub(crate) fn rand_threshold(p: f64) -> bool {
    // ponytail: deterministic 0.5 split, not a true RNG.  Replace with
    // a seeded PRNG if gameplay-acceptable randomness is needed.
    p > 0.5
}

// ── Refined crew injury model ──────────────────────────────────────────────────

/// Category of injury severity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InjurySeverity {
    /// No injury.
    None,
    /// Minor wound (fragment lodged in non-critical area, soldier combat-effective with treatment).
    Minor,
    /// Serious wound (fragment hit vital area, requires evacuation, significant combat power loss).
    Serious,
    /// Fatal wound (immediately incapacitating or lethal).
    Fatal,
}

/// Body region hit by behind-armour debris.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BodyRegion {
    /// Head/neck — ~12% of presented area, high lethality.
    HeadNeck,
    /// Thorax/chest — ~16% of area, high lethality.
    Thorax,
    /// Abdomen — ~11% of area, moderate lethality.
    Abdomen,
    /// Arms — ~22% of area, low lethality.
    Arms,
    /// Legs — ~39% of area, low lethality.
    Legs,
}

impl BodyRegion {
    /// All body regions in order of descending lethality, for iteration.
    pub const ALL: [BodyRegion; 5] = [
        BodyRegion::HeadNeck,
        BodyRegion::Thorax,
        BodyRegion::Abdomen,
        BodyRegion::Arms,
        BodyRegion::Legs,
    ];
}

/// Result of a refined crew injury evaluation.
#[derive(Debug, Clone)]
pub struct CrewInjuryResult {
    /// Total number of fragments that struck the crew position.
    pub total_impacts: i32,
    /// Number of fatal wounds.
    pub fatal_wounds: i32,
    /// Number of serious wounds.
    pub serious_wounds: i32,
    /// Number of minor wounds.
    pub minor_wounds: i32,
    /// Number of crew members incapacitated.
    pub crew_incapacitated: i32,
    /// Detailed breakdown by body region.
    pub region_breakdown: Vec<BodyRegionInjury>,
    /// Probability of at least one fatal hit (>0.5 = likely fatality).
    pub fatal_hit_probability: f64,
}

/// Injury breakdown for a single body region.
#[derive(Debug, Clone)]
pub struct BodyRegionInjury {
    /// Which body region this breakdown covers.
    pub region: BodyRegion,
    /// Number of fragment impacts in this region.
    pub fragment_impacts: i32,
    /// Number of fatal hits in this region.
    pub fatal_hits: i32,
    /// Number of serious hits in this region.
    pub serious_hits: i32,
    /// Number of minor hits in this region.
    pub minor_hits: i32,
}

/// Body region lethality: probability a hit to this region is fatal,
/// given a fragment of at least threshold KE.
///
/// Returns `(fatal_prob, serious_prob, minor_prob)` summing to ≤ 1.0.
fn region_lethality(region: BodyRegion, fragment_ke_j: f64) -> (f64, f64, f64) {
    let region_params: (f64, f64, f64) = match region {
        BodyRegion::HeadNeck => (0.85, 0.10, 0.05),
        BodyRegion::Thorax => (0.60, 0.25, 0.15),
        BodyRegion::Abdomen => (0.30, 0.40, 0.30),
        BodyRegion::Arms => (0.05, 0.25, 0.70),
        BodyRegion::Legs => (0.02, 0.18, 0.80),
    };

    // Scale by KE: below CREW_INJURY_THRESHOLD_J (80 J), reduce severity
    let ke_factor = (fragment_ke_j / CREW_INJURY_THRESHOLD_J).clamp(0.0, 1.0);
    if ke_factor < 1.0 {
        // Below threshold: mostly minor or none
        (
            0.0,
            region_params.1 * ke_factor * 0.3,
            region_params.2 * ke_factor,
        )
    } else {
        // Above threshold: full lethality, scaled by log(KE/threshold)
        let severity = 1.0 + 0.5 * (fragment_ke_j / CREW_INJURY_THRESHOLD_J).ln().max(0.0);
        let fatal = region_params.0.min(1.0_f64);
        let serious = (region_params.1 * severity.min(2.0)).min(1.0 - fatal);
        (fatal, serious, region_params.2)
    }
}

/// Body region area fraction of total presented body area.
fn region_area_fraction(region: BodyRegion) -> f64 {
    match region {
        BodyRegion::HeadNeck => 0.12,
        BodyRegion::Thorax => 0.16,
        BodyRegion::Abdomen => 0.11,
        BodyRegion::Arms => 0.22,
        BodyRegion::Legs => 0.39,
    }
}

/// Generate a mass distribution of BAD fragments using a power-law model.
///
/// Real behind-armour debris follows approximately a Mott distribution:
///   dN/dm = C · m^(-β)  where β ≈ 2.0 for armour steel
///
/// Returns a vector of fragment masses in kg.
fn generate_fragment_masses(total_mass_g: f64, num_fragments: i32, is_apfsds: bool) -> Vec<f64> {
    let count = num_fragments.max(1);
    let total_kg = total_mass_g / 1000.0;

    if count <= 1 {
        return vec![total_kg];
    }

    // Power-law exponent: APFSDS produces finer fragments (β ≈ 2.2)
    // vs. conventional rounds (β ≈ 1.8)
    let beta = if is_apfsds { 2.2 } else { 1.8 };

    let mut masses: Vec<f64> = Vec::with_capacity(count as usize);

    // Uniformly spaced quantiles for deterministic results
    let mut total = 0.0;
    for i in 0..count {
        let u = (i as f64 + 0.5) / count as f64; // midpoint quantile
        // Inverse CDF for power law: m = m_min * (1 - u)^(-1/(β-1))
        // For β > 1, the CDF is: F(m) = 1 - (m/m_min)^(-(β-1))
        // Inverse: m = m_min * (1 - u)^(-1/(β-1))
        let m_min = total_kg / count as f64 * 0.1; // smallest fragment is 10% of average
        let m = m_min * (1.0 - u).powf(-1.0 / (beta - 1.0));
        masses.push(m);
        total += m;
    }

    // Normalize to total mass
    if total > 0.0 {
        let scale = total_kg / total;
        for m in &mut masses {
            *m *= scale;
        }
    }

    masses
}

/// Evaluate refined crew injury with fragment distribution and body region detail.
///
/// This is an UPGRADED version of [`evaluate_crew`] with more granular fragment
/// mass distribution and body region hit modelling.
///
/// # Arguments
/// * `protection_level` — Crew protection (None/Light/SpallLiner/Heavy).
/// * `residual_velocity_ms` — Projectile velocity after penetrating armour (m/s).
/// * `spall_fragments` — Number of BAD fragments (from PenetrationResult).
/// * `bad_spray_angle_deg` — Spray cone half-angle (degrees).
/// * `bad_total_mass_g` — Total mass of all BAD fragments (grams).
/// * `projectile_type` — "ball", "ap", "apfsds", etc.
/// * `num_crew` — Number of crew members in this position (1–3).
pub fn evaluate_crew_refined(
    protection_level: CrewProtection,
    residual_velocity_ms: f64,
    spall_fragments: i32,
    bad_spray_angle_deg: f64,
    bad_total_mass_g: f64,
    projectile_type: &str,
    num_crew: i32,
) -> CrewInjuryResult {
    let protection = spall_protection_factor(protection_level);
    let is_apfsds = matches!(projectile_type.to_lowercase().as_str(), "apfsds");

    if spall_fragments <= 0 || bad_total_mass_g <= 0.0 || residual_velocity_ms < 50.0 {
        let empty_regions: Vec<BodyRegionInjury> = BodyRegion::ALL
            .iter()
            .map(|&region| BodyRegionInjury {
                region,
                fragment_impacts: 0,
                fatal_hits: 0,
                serious_hits: 0,
                minor_hits: 0,
            })
            .collect();
        return CrewInjuryResult {
            total_impacts: 0,
            fatal_wounds: 0,
            serious_wounds: 0,
            minor_wounds: 0,
            crew_incapacitated: 0,
            region_breakdown: empty_regions,
            fatal_hit_probability: 0.0,
        };
    }

    // Generate fragment mass distribution
    let masses = generate_fragment_masses(bad_total_mass_g, spall_fragments, is_apfsds);
    let num_fragments = masses.len() as i32;

    // Fragment velocity (reduced by spray angle)
    let vel_factor = (bad_spray_angle_deg.max(5.0) / 45.0).clamp(0.2, 1.0);
    let frag_vel = (residual_velocity_ms * vel_factor).max(100.0);

    let regions = [
        BodyRegion::HeadNeck,
        BodyRegion::Thorax,
        BodyRegion::Abdomen,
        BodyRegion::Arms,
        BodyRegion::Legs,
    ];

    // Distribute fragments across body regions by area fraction.
    // Earlier regions (head/thorax) get first pick of the mass distribution
    // (worst-case: the largest fragments hit the most vital areas).
    let mut next_idx = 0;
    let mut region_injuries: Vec<BodyRegionInjury> = regions
        .iter()
        .map(|&region| {
            let area_frac = region_area_fraction(region);
            let hits_in_region =
                ((num_fragments as f64 * area_frac).round() as i32).min(num_fragments - next_idx);

            let mut fatal = 0;
            let mut serious = 0;
            let mut minor = 0;

            for j in next_idx..(next_idx + hits_in_region).min(num_fragments) {
                let mass_kg = masses[j as usize];
                let frag_ke_j = 0.5 * mass_kg * frag_vel.powi(2) / protection;
                let (fp, sp, mip) = region_lethality(region, frag_ke_j);

                if fp > 0.5 {
                    fatal += 1;
                } else if sp > 0.5 {
                    serious += 1;
                } else if mip > 0.5 {
                    minor += 1;
                }
            }
            next_idx += hits_in_region;

            BodyRegionInjury {
                region,
                fragment_impacts: hits_in_region.max(0),
                fatal_hits: fatal,
                serious_hits: serious,
                minor_hits: minor,
            }
        })
        .collect();

    // Remaining fragments (rounding loss) go to legs (largest area)
    let remaining = num_fragments - next_idx;
    if remaining > 0 {
        if let Some(legs) = region_injuries.last_mut() {
            for j in next_idx..num_fragments {
                let mass_kg = masses[j as usize];
                let frag_ke_j = 0.5 * mass_kg * frag_vel.powi(2) / protection;
                let (fp, sp, mip) = region_lethality(BodyRegion::Legs, frag_ke_j);
                if fp > 0.5 {
                    legs.fatal_hits += 1;
                } else if sp > 0.5 {
                    legs.serious_hits += 1;
                } else if mip > 0.5 {
                    legs.minor_hits += 1;
                }
                legs.fragment_impacts += 1;
            }
        }
    }

    // Sum totals
    let total_fatal: i32 = region_injuries.iter().map(|r| r.fatal_hits).sum();
    let total_serious: i32 = region_injuries.iter().map(|r| r.serious_hits).sum();
    let total_minor: i32 = region_injuries.iter().map(|r| r.minor_hits).sum();

    // Fatal hit probability: any fatal hit is very serious
    let fatal_prob = if total_fatal > 0 { 0.95 } else { 0.0 };

    // Crew incapacitation: count crew affected by fatal/serious wounds
    let wounded = total_fatal + total_serious + total_minor.min(total_minor);
    let incapacitated = wounded.min(num_crew).max(0);

    CrewInjuryResult {
        total_impacts: num_fragments,
        fatal_wounds: total_fatal,
        serious_wounds: total_serious,
        minor_wounds: total_minor,
        crew_incapacitated: incapacitated,
        region_breakdown: region_injuries,
        fatal_hit_probability: fatal_prob,
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::super::component_damage::CrewProtection;
    use super::*;

    #[test]
    fn crew_refined_no_spall_no_injury() {
        let r = evaluate_crew_refined(CrewProtection::None, 50.0, 0, 0.0, 0.0, "ball", 1);
        assert_eq!(r.total_impacts, 0);
        assert_eq!(r.fatal_wounds, 0);
        assert_eq!(r.crew_incapacitated, 0);
        assert_eq!(r.region_breakdown.len(), 5);
    }

    #[test]
    fn crew_refined_apfsds_high_spall() {
        // APFSDS produces fine, high-velocity fragments
        let r = evaluate_crew_refined(CrewProtection::None, 800.0, 200, 15.0, 100.0, "apfsds", 3);
        assert!(r.total_impacts > 0, "should produce fragment impacts");
        let has_casualties = r.crew_incapacitated > 0 || r.fatal_wounds > 0 || r.serious_wounds > 0;
        assert!(
            has_casualties,
            "high-velocity APFSDS should cause crew casualties"
        );
    }

    #[test]
    fn crew_refined_spall_liner_reduces_severity() {
        let r_none = evaluate_crew_refined(CrewProtection::None, 600.0, 50, 20.0, 200.0, "ball", 2);
        let r_liner = evaluate_crew_refined(
            CrewProtection::SpallLiner,
            600.0,
            50,
            20.0,
            200.0,
            "ball",
            2,
        );
        // Liner should not increase fatal wounds
        assert!(
            r_liner.fatal_wounds <= r_none.fatal_wounds,
            "spall liner should not increase fatal wounds (liner={} vs none={})",
            r_liner.fatal_wounds,
            r_none.fatal_wounds
        );
        // Total impact count is the same (same input fragments)
        assert_eq!(
            r_liner.total_impacts, r_none.total_impacts,
            "spall liner does not change fragment count"
        );
    }

    #[test]
    fn crew_refined_region_breakdown_sums() {
        let r = evaluate_crew_refined(CrewProtection::None, 700.0, 30, 10.0, 80.0, "ap", 2);
        let region_total: i32 = r
            .region_breakdown
            .iter()
            .map(|rb| rb.fragment_impacts)
            .sum();
        assert!(region_total > 0, "region breakdown should have fragments");
        assert!(
            region_total >= r.total_impacts || r.total_impacts == 0,
            "region fragment sum ({}) should cover total impacts ({})",
            region_total,
            r.total_impacts
        );
        // All 5 regions present
        assert_eq!(r.region_breakdown.len(), 5);
    }

    #[test]
    fn crew_refined_heavy_protection_prevents_fatal() {
        let r = evaluate_crew_refined(CrewProtection::Heavy, 500.0, 20, 25.0, 50.0, "ball", 1);
        // Heavy protection (factor 10) — few if any fatal wounds
        assert!(
            r.fatal_wounds <= 1,
            "heavy protection should prevent most fatal wounds (got {})",
            r.fatal_wounds
        );
    }

    #[test]
    fn fragment_mass_distribution_power_law() {
        let masses = generate_fragment_masses(100.0, 50, false);
        assert_eq!(masses.len(), 50);
        let total: f64 = masses.iter().sum();
        // Total should approximate 100 g = 0.1 kg
        assert!(
            (total - 0.1).abs() < 0.02,
            "total mass should be ~0.1 kg, got {}",
            total
        );
        // Power law means small fragments are abundant, large fragments are few.
        // First fragment (lowest quantile) should be the smallest, last the largest.
        assert!(
            masses[0] < masses[masses.len() - 1],
            "power law: first fragment ({:.6}) should be smaller than last ({:.6})",
            masses[0],
            masses[masses.len() - 1]
        );
    }

    #[test]
    fn fragment_mass_apfsds_finer_than_conventional() {
        let masses_conv = generate_fragment_masses(100.0, 50, false);
        let masses_apfsds = generate_fragment_masses(100.0, 50, true);
        // APFSDS (β=2.2) produces more uniform (finer) fragments than
        // conventional (β=1.8). The ratio of smallest to largest fragment
        // should be larger for APFSDS (closer to 1 = more uniform).
        let conv_ratio = masses_conv[0] / masses_conv[masses_conv.len() - 1];
        let apfsds_ratio = masses_apfsds[0] / masses_apfsds[masses_apfsds.len() - 1];
        assert!(
            apfsds_ratio > conv_ratio,
            "APFSDS should have more uniform fragment distribution (ratio {:.4} vs {:.4})",
            apfsds_ratio,
            conv_ratio
        );
    }

    #[test]
    fn region_lethality_ordering() {
        // Head/neck should always be more lethal than legs for the same KE
        let (fp_head, _, _) = region_lethality(BodyRegion::HeadNeck, 200.0);
        let (fp_legs, _, _) = region_lethality(BodyRegion::Legs, 200.0);
        assert!(
            fp_head > fp_legs,
            "head/neck lethality ({}) should exceed leg lethality ({})",
            fp_head,
            fp_legs
        );
    }
}
