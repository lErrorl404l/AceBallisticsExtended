// ABE - Sequential Hit Model
//
// Multi-round sequential hit model. When multiple projectiles hit the same
// armor plate, the plate degrades cumulatively. Models armor degradation
// from sustained fire including zone proximity, spall liner wear, heat
// buildup, fatigue cracking, and edge effects.
//
// References:
//   - Miner's cumulative damage rule (Miner 1945)
//   - MIL-HDBK-5J (Metallic Materials and Elements for Aerospace Vehicle
//     Structures)
//   - ASM Handbook Vol 1: Properties and Selection: Irons, Steels, and
//     High-Performance Alloys
//   - RHA tempering behaviour (~300 °C softening threshold)

/// Record of a prior hit on an armor plate.
#[derive(Debug, Clone)]
pub struct HitRecord {
    /// X-coordinate of impact on plate (m).
    pub hit_x_m: f64,
    /// Y-coordinate of impact on plate (m).
    pub hit_y_m: f64,
    /// Impact kinetic energy (J).
    pub impact_energy_j: f64,
    /// Projectile type identifier.
    pub projectile_type: String,
    /// Assigned zone ID (populated by [`evaluate_sequential_hits`]).
    pub zone_id: i32,
}

/// Input parameters for the sequential hit model.
#[derive(Debug, Clone)]
pub struct SequentialHitParams {
    /// Material identifier (e.g. "steel_rha", "aluminum_5083").
    pub material: String,
    /// Plate width (m) — for edge proximity calculation.
    pub plate_width_m: f64,
    /// Plate height (m).
    pub plate_height_m: f64,
    /// Plate reference thickness (m).
    pub plate_thickness_m: f64,
    /// Projectile calibre (m) — for zone-proximity thresholds.
    pub caliber_m: f64,
    /// History of hits on this plate (including the latest).
    pub prior_hits: Vec<HitRecord>,
    /// Whether a spall liner is present on the plate.
    pub spall_liner_present: bool,
    /// Ambient temperature in °C.
    pub ambient_temp_c: f64,
}

/// Result of sequential hit evaluation.
#[derive(Debug, Clone)]
pub struct SequentialHitResult {
    /// Post-degradation equivalent thickness (m).
    pub effective_thickness_m: f64,
    /// Armor degradation factor (1.0 = pristine, 0.5 = 50 % remaining).
    pub armor_degradation_factor: f64,
    /// Whether the spall liner is still intact.
    pub spall_liner_intact: bool,
    /// Whether the plate has through-thickness cracking in the active zone.
    pub plate_cracked: bool,
    /// Degradation factor from hits in the same zone (1.0 = pristine).
    pub zone_degradation_factor: f64,
    /// Miner's cumulative damage (0-1+; ≥1.0 = structural failure).
    pub cumulative_damage: f64,
    /// Estimated remaining hits before structural failure.
    pub estimated_remaining_hits_to_failure: i32,
}

// ── Constants ────────────────────────────────────────────────────────────────────

/// Ratio of calibre for "same zone" threshold (≤2× calibre = full cumulative
/// effect).
const SAME_ZONE_CAL_RATIO: f64 = 2.0;

/// Ratio of calibre for "partial zone" threshold (2-6× = partial effect).
const PARTIAL_ZONE_CAL_RATIO: f64 = 6.0;

/// Ratio of calibre for edge effect threshold (≤3× calibre → edge multiplier
/// applies).
const EDGE_ZONE_CAL_RATIO: f64 = 3.0;

/// Number of hits in the same zone before the plate is considered to have
/// through-thickness cracking.
const ZONE_CRACK_THRESHOLD: i32 = 5;

/// Reference energy (J) for Miner's cumulative damage scaling.  Equivalent to
/// a 7.62 mm NATO AP round at ~100 m (~3.3 kJ).
const REFERENCE_ENERGY_J: f64 = 3300.0;

/// Fraction of impact energy converted to heat in the plate (non-penetrating
/// estimate).
const HEAT_CONVERSION_FRACTION: f64 = 0.7;

/// Specific heat capacity of steel (J·kg⁻¹·K⁻¹).
const SPECIFIC_HEAT_STEEL: f64 = 450.0;

/// Specific heat capacity of aluminium (J·kg⁻¹·K⁻¹).
const SPECIFIC_HEAT_ALUMINUM: f64 = 900.0;

/// Steel density (kg·m⁻³).
const STEEL_DENSITY: f64 = 7850.0;

/// Aluminium density (kg·m⁻³).
const ALUMINUM_DENSITY: f64 = 2700.0;

/// RHA tempering threshold (°C).  Above this temperature the steel begins to
/// lose temper, reducing hardness.
const RHA_TEMPER_THRESHOLD_C: f64 = 300.0;

/// Minimum impact energy (J) required to remove a spall liner.
const SPALL_LINER_ENERGY_J: f64 = 500.0;

/// Ratio of calibre for "tight cluster" threshold (≤3× calibre = grouped hit).
const TIGHT_CLUSTER_CAL_RATIO: f64 = 3.0;

// ── Public API ───────────────────────────────────────────────────────────────────

/// Cumulative armor degradation state across sequential impacts.
///
/// Tracks hit count, total absorbed energy, and spatial clustering to
/// model the progressive weakening of armor under sustained fire.
#[derive(Debug, Clone)]
pub struct DegradationAccumulator {
    /// Total number of hits on the plate.
    pub total_hits: i32,
    /// Total kinetic energy absorbed by the plate (J).
    pub total_energy_absorbed_j: f64,
    /// Miner-style cumulative damage index in `[0.0, 1.0]`.
    /// 0.0 = pristine, 1.0 = fully degraded.
    pub cumulative_damage_index: f64,
    /// Multiplier for tightly grouped hits in `[0.0, 1.0]`.
    /// Higher values mean hits are more tightly clustered.
    pub zone_cluster_penalty: f64,
}

/// Compute proximity degradation factor between two hit locations.
///
/// Returns a factor in `[0.0, 1.0]`:
/// - ≤2× calibre apart → 1.0 (full cumulative effect, same zone)
/// - 2-6× calibre → linear taper from 1.0 down to 0.0
/// - >6× calibre → 0.0 (independent hits, no proximity interaction)
pub fn hit_proximity_factor(
    hit1_x: f64,
    hit1_y: f64,
    hit2_x: f64,
    hit2_y: f64,
    caliber_m: f64,
) -> f64 {
    let dist = ((hit1_x - hit2_x).powi(2) + (hit1_y - hit2_y).powi(2)).sqrt();
    let cal_ratio = dist / caliber_m.max(f64::MIN_POSITIVE);

    if cal_ratio <= SAME_ZONE_CAL_RATIO {
        1.0
    } else if cal_ratio <= PARTIAL_ZONE_CAL_RATIO {
        // Linear taper: 1.0 at 2× → 0.0 at 6×
        let t = (cal_ratio - SAME_ZONE_CAL_RATIO) / (PARTIAL_ZONE_CAL_RATIO - SAME_ZONE_CAL_RATIO);
        (1.0 - t).max(0.0)
    } else {
        0.0
    }
}

/// Compute edge proximity factor.
///
/// Returns a multiplier in `[0.7, 1.0]`:
/// - Within 3× calibre of any edge → 0.7 (reduced effective thickness from
///   edge breakout).
/// - Farther from edges → approaches 1.0 with a linear ramp.
pub fn edge_proximity_factor(x: f64, y: f64, width: f64, height: f64, caliber_m: f64) -> f64 {
    let edge_zone = EDGE_ZONE_CAL_RATIO * caliber_m;
    if edge_zone <= 0.0 {
        return 1.0;
    }

    let dist_left = x;
    let dist_right = width - x;
    let dist_bottom = y;
    let dist_top = height - y;

    let min_edge_dist = dist_left.min(dist_right).min(dist_bottom).min(dist_top);

    if min_edge_dist < edge_zone {
        // Linear ramp: 0.7 right at edge, 1.0 at edge_zone distance
        let t = (min_edge_dist / edge_zone).clamp(0.0, 1.0);
        0.7 + 0.3 * t
    } else {
        1.0
    }
}

/// Compute heat buildup from sequential impacts.
///
/// Returns the estimated bulk plate temperature in °C using a lumped thermal
/// model: each impact deposits a fraction of its energy as heat, raising the
/// plate temperature proportionally to `Q / (m · cₚ)`.
///
/// Cooling (conduction / convection to ambient) is ignored — this represents
/// the worst-case temperature during a rapid burst of fire.
pub fn heat_buildup(
    hits: &[HitRecord],
    ambient_c: f64,
    plate_area_m2: f64,
    plate_thickness_m: f64,
    material: &str,
) -> f64 {
    if hits.is_empty() || plate_area_m2 <= 0.0 || plate_thickness_m <= 0.0 {
        return ambient_c;
    }

    let (density, specific_heat) = if material.to_lowercase().contains("aluminum") {
        (ALUMINUM_DENSITY, SPECIFIC_HEAT_ALUMINUM)
    } else {
        (STEEL_DENSITY, SPECIFIC_HEAT_STEEL)
    };

    let plate_vol = plate_area_m2 * plate_thickness_m;
    let plate_mass_kg = density * plate_vol;

    if plate_mass_kg <= 0.0 {
        return ambient_c;
    }

    let total_heat_j: f64 = hits
        .iter()
        .map(|h| h.impact_energy_j * HEAT_CONVERSION_FRACTION)
        .sum();

    let delta_t = total_heat_j / (plate_mass_kg * specific_heat);
    ambient_c + delta_t
}

/// Evaluate armor plate condition after sequential hits.
///
/// Analyzes the cumulative effect of multiple hits on the same armor plate,
/// accounting for:
/// - **Zone proximity** — hits clustered together degrade the plate more
///   than hits spread apart.
/// - **Edge effects** — hits near plate edges can cause earlier edge
///   breakout.
/// - **Spall liner wear** — the first hit in a zone with sufficient energy
///   removes the spall liner; subsequent hits see bare armor.
/// - **Heat buildup** — sustained fire heats the plate, reducing hardness
///   above the tempering threshold.
/// - **Through-thickness cracking** — N concentrated hits in the same zone
///   may crack the plate.
/// - **Miner's cumulative fatigue damage** — each hit contributes
///   `ΔD = E_impact / E_ref`; when `D_cum >= 1.0` the plate is structurally
///   degraded.
pub fn evaluate_sequential_hits(params: &SequentialHitParams) -> SequentialHitResult {
    let hits = &params.prior_hits;
    let plate_area = params.plate_width_m * params.plate_height_m;
    let cal = params.caliber_m;

    // ── 1. Assign hits to spatial zones ─────────────────────────────────
    let zones = assign_zones(hits, cal);

    // Count hits per zone (for crack detection)
    let max_zone = zones.iter().max().copied().unwrap_or(0);
    let mut hits_per_zone = vec![0i32; (max_zone + 1) as usize];
    for &z in &zones {
        hits_per_zone[z as usize] += 1;
    }

    // Zone of the latest hit
    let latest_zone = if zones.is_empty() {
        0
    } else {
        zones[zones.len() - 1]
    };
    let latest_zone_hits = *hits_per_zone.get(latest_zone as usize).unwrap_or(&0);

    // ── 2. Zone degradation factor ──────────────────────────────────────
    let zone_degradation_factor = zone_degradation(latest_zone_hits);

    // ── 3. Proximity degradation for the latest hit ─────────────────────
    // Average proximity factor between the latest hit and all other hits in
    // the same zone.
    let latest_idx = hits.len().saturating_sub(1);
    let mut prox_sum = 0.0;
    let mut prox_count = 0;
    if !hits.is_empty() {
        for (i, hit) in hits.iter().enumerate() {
            if i != latest_idx && zones[i] == latest_zone {
                let pf = hit_proximity_factor(
                    hits[latest_idx].hit_x_m,
                    hits[latest_idx].hit_y_m,
                    hit.hit_x_m,
                    hit.hit_y_m,
                    cal,
                );
                prox_sum += pf;
                prox_count += 1;
            }
        }
    }
    let _avg_proximity = if prox_count > 0 {
        prox_sum / prox_count as f64
    } else {
        0.0
    };

    // ── 4. Edge effect (latest hit) ─────────────────────────────────────
    let edge_factor = if !hits.is_empty() {
        edge_proximity_factor(
            hits[latest_idx].hit_x_m,
            hits[latest_idx].hit_y_m,
            params.plate_width_m,
            params.plate_height_m,
            cal,
        )
    } else {
        1.0
    };

    // ── 5. Miner's cumulative damage ────────────────────────────────────
    let d_cum = compute_cumulative_damage(hits);
    let cum_damage_factor = if d_cum >= 1.0 { 0.5 } else { 1.0 };

    // ── 6. Plate crack check ────────────────────────────────────────────
    let plate_cracked = zone_has_cracked(latest_zone_hits);

    // ── 7. Material factor (from penetration.rs) ────────────────────────
    let mat_factor = crate::penetration::material_factor(&params.material);

    // ── 8. Heat buildup → hardness reduction ────────────────────────────
    let plate_temp = heat_buildup(
        hits,
        params.ambient_temp_c,
        plate_area,
        params.plate_thickness_m,
        &params.material,
    );
    let heat_degradation = if plate_temp > RHA_TEMPER_THRESHOLD_C {
        // Linear degradation from 1.0 at 300 °C to 0.5 at 600 °C
        let over_temp = (plate_temp - RHA_TEMPER_THRESHOLD_C).min(300.0);
        1.0 - 0.5 * (over_temp / 300.0)
    } else {
        1.0
    };

    // ── 9. Spall liner status ───────────────────────────────────────────
    let spall_liner_intact = compute_spall_liner_status(hits, params.spall_liner_present);

    // ── 10. Combined armor degradation factor ───────────────────────────
    // Start with zone degradation clamped by cumulative damage floor.
    let mut adf = zone_degradation_factor.min(cum_damage_factor);
    adf *= edge_factor;
    adf *= heat_degradation;

    // Bare-armor penalty when spall liner is removed.
    if !spall_liner_intact && params.spall_liner_present {
        adf *= 0.95;
    }

    // Through-thickness cracking penalty.
    if plate_cracked {
        adf *= 0.85;
    }

    adf = adf.clamp(0.1, 1.0);

    // ── 11. Effective thickness ─────────────────────────────────────────
    let effective_thickness_m = params.plate_thickness_m * mat_factor * adf;

    // ── 12. Remaining hits estimate ─────────────────────────────────────
    let remaining = estimate_remaining_hits(d_cum, hits);

    SequentialHitResult {
        effective_thickness_m,
        armor_degradation_factor: adf,
        spall_liner_intact,
        plate_cracked,
        zone_degradation_factor,
        cumulative_damage: d_cum,
        estimated_remaining_hits_to_failure: remaining,
    }
}

/// Sigmoid function: `1.0 / (1.0 + exp(-x))`.
///
/// Maps any real input to `(0.0, 1.0)`, centred at `x=0` → `0.5`.
fn sigmoid(x: f64) -> f64 {
    1.0 / (1.0 + (-x).exp())
}

/// Compute the armor degradation factor from a [`DegradationAccumulator`].
///
/// Returns a multiplier in `[0.70, 1.0]` applied to effective armor thickness:
/// - `1.0` = full strength
/// - `0.70` = maximum degradation from cumulative damage alone
///
/// The reduction scales with the cumulative damage index (up to 15 %) and
/// the zone cluster penalty (up to 10 %).  The floor prevents complete
/// armor negation from degradation alone — a cracked plate still provides
/// some protection.
pub fn armor_degradation_factor(
    accumulator: &DegradationAccumulator,
    _params: &SequentialHitParams,
) -> f64 {
    let mut factor = 1.0;

    // Miner's cumulative damage reduces effective thickness by up to 15 %.
    factor -= 0.15 * accumulator.cumulative_damage_index;

    // Tightly grouped hits degrade locally by up to 10 %.
    factor -= 0.1 * accumulator.zone_cluster_penalty;

    factor.clamp(0.70, 1.0)
}

/// Compute the [`DegradationAccumulator`] from prior hit history.
///
/// Analyzes hit count, total absorbed energy, and spatial clustering to
/// quantify how much cumulative damage the armor has sustained.
///
/// - `cumulative_damage_index` uses a sigmoid of `(hits − 3) / 3` to
///   capture non-linear fatigue progression.
/// - `zone_cluster_penalty` grows with the fraction of hits within 3×
///   calibre of another hit, saturating via `1 − exp(−ratio × 2)`.
pub fn compute_degradation_accumulator(
    prior_hits: &[HitRecord],
    params: &SequentialHitParams,
) -> DegradationAccumulator {
    let total_hits = prior_hits.len() as i32;
    let total_energy_absorbed_j: f64 = prior_hits.iter().map(|h| h.impact_energy_j).sum();

    if total_hits == 0 {
        return DegradationAccumulator {
            total_hits: 0,
            total_energy_absorbed_j: 0.0,
            cumulative_damage_index: 0.0,
            zone_cluster_penalty: 0.0,
        };
    }

    // Sigmoid cumulative damage index: near 0 at 0-2 hits, ~0.5 at 3,
    // approaching 1.0 as hits increase.
    let x = (total_hits as f64 - 3.0) / 3.0;
    let cumulative_damage_index = sigmoid(x);

    // Zone cluster penalty: fraction of hits within 3× calibre of another
    // hit.  Tightly grouped impacts cause more localised degradation.
    let tight_threshold = TIGHT_CLUSTER_CAL_RATIO * params.caliber_m;
    let mut tight_count = 0;
    for i in 0..prior_hits.len() {
        let has_nearby = prior_hits.iter().enumerate().any(|(j, other)| {
            if i == j {
                return false;
            }
            let dx = prior_hits[i].hit_x_m - other.hit_x_m;
            let dy = prior_hits[i].hit_y_m - other.hit_y_m;
            (dx * dx + dy * dy).sqrt() <= tight_threshold
        });
        if has_nearby {
            tight_count += 1;
        }
    }
    let tight_hit_ratio = tight_count as f64 / total_hits as f64;
    let zone_cluster_penalty = 1.0 - (-tight_hit_ratio * 2.0).exp();

    DegradationAccumulator {
        total_hits,
        total_energy_absorbed_j,
        cumulative_damage_index,
        zone_cluster_penalty,
    }
}

// ── Internal helpers ──────────────────────────────────────────────────────────────

/// Assign zone IDs to hits based on spatial proximity.
///
/// - Hits within 2× calibre of any existing hit in a zone join that zone
///   (full cumulative effect).
/// - Hits within 2-6× calibre of the nearest hit share the zone with
///   partial effect.
/// - Hits >6× calibre from all existing hits start a new zone.
fn assign_zones(hits: &[HitRecord], caliber_m: f64) -> Vec<i32> {
    if hits.is_empty() {
        return vec![];
    }
    let mut zones = vec![0i32; hits.len()];
    let mut next_zone = 1;

    // First hit is always zone 0.
    zones[0] = 0;

    for i in 1..hits.len() {
        let mut assigned = false;
        // Walk backwards: assign to the zone of the *closest* prior hit
        // within range.
        for j in (0..i).rev() {
            let dist = ((hits[i].hit_x_m - hits[j].hit_x_m).powi(2)
                + (hits[i].hit_y_m - hits[j].hit_y_m).powi(2))
            .sqrt();
            let cal_ratio = dist / caliber_m.max(f64::MIN_POSITIVE);

            if cal_ratio <= PARTIAL_ZONE_CAL_RATIO {
                zones[i] = zones[j];
                assigned = true;
                break;
            }
        }
        if !assigned {
            zones[i] = next_zone;
            next_zone += 1;
        }
    }
    zones
}

/// Degradation factor for a single zone based on number of hits.
///
/// Each additional hit reduces effective strength with diminishing returns:
/// - 1 hit → 1.0 (pristine)
/// - 2 hits → ~0.87
/// - 3 hits → ~0.78
/// - 5+ hits → approaches 0.5 asymptotically
fn zone_degradation(hit_count: i32) -> f64 {
    if hit_count <= 1 {
        1.0
    } else {
        // Exponential decay toward 0.5 floor
        let factor = 1.0 - 0.5 * (1.0 - (-0.3 * (hit_count - 1) as f64).exp());
        factor.clamp(0.5, 1.0)
    }
}

/// Whether a zone's hit count exceeds the through-thickness crack threshold.
fn zone_has_cracked(hit_count: i32) -> bool {
    hit_count >= ZONE_CRACK_THRESHOLD
}

/// Compute Miner's cumulative damage from all hits.
///
/// Each hit contributes `ΔD = E_impact / E_ref`.  When `D_cum >= 1.0` the
/// plate has structurally failed (effective thickness halved).
fn compute_cumulative_damage(hits: &[HitRecord]) -> f64 {
    if hits.is_empty() {
        return 0.0;
    }
    hits.iter()
        .map(|h| h.impact_energy_j / REFERENCE_ENERGY_J)
        .sum()
}

/// Determine whether the spall liner remains intact.
///
/// The first hit with impact energy ≥ `SPALL_LINER_ENERGY_J` removes the
/// liner globally.
fn compute_spall_liner_status(hits: &[HitRecord], spall_liner_present: bool) -> bool {
    if !spall_liner_present {
        return false;
    }
    for hit in hits {
        if hit.impact_energy_j >= SPALL_LINER_ENERGY_J {
            return false;
        }
    }
    true
}

/// Estimate remaining hits before Miner failure using the average energy of
/// prior hits.
fn estimate_remaining_hits(d_cum: f64, hits: &[HitRecord]) -> i32 {
    if hits.is_empty() {
        return 10;
    }
    if d_cum >= 1.0 {
        return 0;
    }

    let avg_energy: f64 = hits.iter().map(|h| h.impact_energy_j).sum::<f64>() / hits.len() as f64;

    if avg_energy <= 0.0 {
        return 10;
    }

    let d_per_hit = avg_energy / REFERENCE_ENERGY_J;
    let remaining = ((1.0 - d_per_hit) / d_per_hit).ceil() as i32;
    remaining.max(0)
}

// ── Tests ─────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Helper: make a hit record ────────────────────────────────────────
    fn hit(x: f64, y: f64, energy_j: f64, proj: &str) -> HitRecord {
        HitRecord {
            hit_x_m: x,
            hit_y_m: y,
            impact_energy_j: energy_j,
            projectile_type: proj.to_string(),
            zone_id: -1,
        }
    }

    fn default_params() -> SequentialHitParams {
        SequentialHitParams {
            material: "steel_rha".into(),
            plate_width_m: 1.0,
            plate_height_m: 1.0,
            plate_thickness_m: 0.025, // 25 mm
            caliber_m: 0.00762,       // 7.62 mm
            prior_hits: vec![],
            spall_liner_present: true,
            ambient_temp_c: 20.0,
        }
    }

    // ── hit_proximity_factor ─────────────────────────────────────────────

    #[test]
    fn proximity_same_location_full_effect() {
        // Zero distance → same point → full effect
        let f = hit_proximity_factor(0.0, 0.0, 0.0, 0.0, 0.00762);
        assert!((f - 1.0).abs() < 1e-10, "same point should be 1.0, got {f}");
    }

    #[test]
    fn proximity_within_2x_caliber_full() {
        // 0.015 m apart with 0.00762 calibre → ratio ~1.97 → within 2×
        let f = hit_proximity_factor(0.0, 0.0, 0.015, 0.0, 0.00762);
        assert!(
            (f - 1.0).abs() < 1e-10,
            "within 2× cal should be 1.0, got {f}"
        );
    }

    #[test]
    fn proximity_2x_to_6x_partial() {
        // 0.03 m apart, 0.00762 cal → ratio ~3.94 → partial taper
        let f = hit_proximity_factor(0.0, 0.0, 0.030, 0.0, 0.00762);
        assert!(f > 0.0 && f < 1.0, "2-6× cal should taper: got {f}");
        // Expected: t = (3.94 - 2) / 4 = 0.485 → 1 - 0.485 = 0.515
        let expected = 1.0 - (0.030 / 0.00762 - 2.0) / 4.0;
        assert!(
            (f - expected).abs() < 0.01,
            "expected {expected:.4}, got {f:.4}"
        );
    }

    #[test]
    fn proximity_beyond_6x_independent() {
        // 0.05 m apart, 0.00762 cal → ratio ~6.56 → >6× → independent
        let f = hit_proximity_factor(0.0, 0.0, 0.050, 0.0, 0.00762);
        assert!((f - 0.0).abs() < 1e-10, "beyond 6× should be 0.0, got {f}");
    }

    #[test]
    fn proximity_exact_boundaries() {
        let cal = 0.01;
        // Exactly 2× calibre
        let f2 = hit_proximity_factor(0.0, 0.0, 0.02, 0.0, cal);
        assert!((f2 - 1.0).abs() < 1e-10, "at 2× should be 1.0, got {f2}");
        // Exactly 6× calibre
        let f6 = hit_proximity_factor(0.0, 0.0, 0.06, 0.0, cal);
        assert!((f6 - 0.0).abs() < 1e-10, "at 6× should be 0.0, got {f6}");
    }

    // ── edge_proximity_factor ────────────────────────────────────────────

    #[test]
    fn edge_at_center_no_penalty() {
        // Centre of a 1×1 plate, 7.62 mm calibre → edge zone = 0.02286 m
        // Centre is 0.5 m from each edge → well outside zone → 1.0
        let f = edge_proximity_factor(0.5, 0.5, 1.0, 1.0, 0.00762);
        assert!((f - 1.0).abs() < 1e-10, "centre should be 1.0, got {f}");
    }

    #[test]
    fn edge_at_boundary_minimum() {
        // Right at the edge
        let cal = 0.01;
        let f = edge_proximity_factor(0.0, 0.5, 1.0, 1.0, cal);
        assert!((f - 0.7).abs() < 1e-10, "at edge should be 0.7, got {f}");
    }

    #[test]
    fn edge_within_zone_partial_ramp() {
        // At half the edge-zone distance → halfway on ramp
        let cal = 0.01;
        let edge_zone = 3.0 * cal; // 0.03
        let f = edge_proximity_factor(edge_zone / 2.0, 0.5, 1.0, 1.0, cal);
        let expected = 0.7 + 0.3 * 0.5; // 0.85
        assert!(
            (f - expected).abs() < 1e-10,
            "at half zone should be {expected}, got {f}"
        );
    }

    // ── heat_buildup ─────────────────────────────────────────────────────

    #[test]
    fn heat_no_hits_ambient() {
        let temp = heat_buildup(&[], 20.0, 1.0, 0.025, "steel_rha");
        assert!(
            (temp - 20.0).abs() < 1e-10,
            "no hits should be ambient, got {temp}"
        );
    }

    #[test]
    fn heat_single_hit_rise() {
        // 1 × 3.3 kJ hit on a 1 m² × 25 mm steel plate
        let hits = vec![hit(0.0, 0.0, 3300.0, "ap")];
        let temp = heat_buildup(&hits, 20.0, 1.0, 0.025, "steel_rha");
        // Plate mass = 7850 * 1 * 0.025 = 196.25 kg
        // Q = 3300 * 0.7 = 2310 J
        // ΔT = 2310 / (196.25 * 450) ≈ 0.026 °C
        assert!(temp > 20.0, "single hit should raise temp, got {temp}");
        assert!(temp < 21.0, "single hit should be modest, got {temp}");
    }

    #[test]
    fn heat_aluminum_higher_rise() {
        // Aluminium has higher specific heat and lower density → different ΔT
        let hits = vec![hit(0.0, 0.0, 3300.0, "ap")];
        let temp = heat_buildup(&hits, 20.0, 1.0, 0.025, "aluminum_5083");
        assert!(temp > 20.0, "should heat above ambient, got {temp}");
    }

    #[test]
    fn heat_sustained_burst_significant() {
        // 20 hits of 7.62 mm NATO (~3.3 kJ each) on a small plate
        let hits: Vec<_> = (0..20)
            .map(|i| hit(0.05 * i as f64, 0.0, 3300.0, "ball"))
            .collect();
        let temp = heat_buildup(&hits, 20.0, 0.5, 0.010, "steel_rha");
        // Plate: 0.5 m² × 10 mm → mass ≈ 39.25 kg
        // Q_total = 20 × 3300 × 0.7 ≈ 46.2 kJ → ΔT ≈ 2.6 °C
        assert!(
            temp > 21.0,
            "20 hits should show measurable rise, got {temp}"
        );
        assert!(
            temp < 50.0,
            "20 hits shouldn't heat excessively, got {temp}"
        );
    }

    // ── evaluate_sequential_hits — single hit ────────────────────────────

    #[test]
    fn single_hit_no_degradation() {
        // Use a low-energy hit so cumulative damage stays below 1.0
        // and the spall liner is NOT removed (< 500 J).
        let params = SequentialHitParams {
            prior_hits: vec![hit(0.5, 0.5, 100.0, "ball")],
            ..default_params()
        };
        let r = evaluate_sequential_hits(&params);
        // Single hit → zone_degradation = 1.0, no crack, no edge penalty,
        // D_cum < 1.0, liner intact, no heat effect.
        assert!(
            (r.armor_degradation_factor - 1.0).abs() < 0.01,
            "single low-energy hit should have minimal degradation, got {}",
            r.armor_degradation_factor
        );
        assert!(!r.plate_cracked, "single hit should not crack plate");
        assert!(r.spall_liner_intact, "100 J should not remove liner");
        assert!(r.estimated_remaining_hits_to_failure > 0);
    }

    // ── Multi-hit: same zone ─────────────────────────────────────────────

    #[test]
    fn two_hits_same_zone_increases_degradation() {
        // Use moderate energies so D_cum stays < 1.0 for 1 hit but > 1.0
        // for 2 hits, allowing degradation comparison.
        let one_hit = SequentialHitParams {
            prior_hits: vec![hit(0.5, 0.5, 1000.0, "ap")],
            ..default_params()
        };
        let two_hits = SequentialHitParams {
            prior_hits: vec![
                hit(0.5, 0.5, 1000.0, "ap"),
                hit(0.508, 0.5, 1000.0, "ap"), // 8 mm apart → same zone (7.62 mm cal)
            ],
            ..default_params()
        };
        let r1 = evaluate_sequential_hits(&one_hit);
        let r2 = evaluate_sequential_hits(&two_hits);
        assert!(
            r2.armor_degradation_factor < r1.armor_degradation_factor,
            "two same-zone hits should degrade more than one: {:.4} vs {:.4}",
            r2.armor_degradation_factor,
            r1.armor_degradation_factor,
        );
        assert!(
            r2.cumulative_damage > r1.cumulative_damage,
            "cumulative damage should increase with more hits"
        );
    }

    // ── Multi-hit: different zone ────────────────────────────────────────

    #[test]
    fn two_hits_different_zones_less_degradation_than_same_zone() {
        let same_zone = SequentialHitParams {
            prior_hits: vec![
                hit(0.5, 0.5, 3300.0, "ap"),
                hit(0.508, 0.5, 3300.0, "ap"), // 8 mm → same zone
            ],
            ..default_params()
        };
        let diff_zone = SequentialHitParams {
            prior_hits: vec![
                hit(0.1, 0.1, 3300.0, "ap"),
                hit(0.9, 0.9, 3300.0, "ap"), // far apart → different zone
            ],
            ..default_params()
        };
        let r_same = evaluate_sequential_hits(&same_zone);
        let r_diff = evaluate_sequential_hits(&diff_zone);
        assert!(
            r_same.zone_degradation_factor < r_diff.zone_degradation_factor,
            "same-zone zone_degradation ({:.4}) should be lower than diff-zone ({:.4})",
            r_same.zone_degradation_factor,
            r_diff.zone_degradation_factor,
        );
    }

    // ── Edge effect ──────────────────────────────────────────────────────

    #[test]
    fn edge_hit_reduces_effective_thickness() {
        let center_hit = SequentialHitParams {
            prior_hits: vec![hit(0.5, 0.5, 3300.0, "ap")],
            ..default_params()
        };
        let edge_hit = SequentialHitParams {
            prior_hits: vec![hit(0.001, 0.5, 3300.0, "ap")], // 1 mm from edge
            ..default_params()
        };
        let r_center = evaluate_sequential_hits(&center_hit);
        let r_edge = evaluate_sequential_hits(&edge_hit);
        assert!(
            r_edge.armor_degradation_factor < r_center.armor_degradation_factor,
            "edge hit should degrade more than centre: {:.4} vs {:.4}",
            r_edge.armor_degradation_factor,
            r_center.armor_degradation_factor,
        );
    }

    // ── Spall liner ──────────────────────────────────────────────────────

    #[test]
    fn spall_liner_removed_by_sufficient_energy() {
        // Small energy hit → liner survives
        let low = SequentialHitParams {
            prior_hits: vec![hit(0.5, 0.5, 100.0, "ball")],
            ..default_params()
        };
        let r_low = evaluate_sequential_hits(&low);
        assert!(r_low.spall_liner_intact, "100 J should not remove liner");

        // Large energy hit → liner removed
        let high = SequentialHitParams {
            prior_hits: vec![hit(0.5, 0.5, 1000.0, "ap")],
            ..default_params()
        };
        let r_high = evaluate_sequential_hits(&high);
        assert!(!r_high.spall_liner_intact, "1000 J should remove liner");
    }

    #[test]
    fn no_spall_liner_stays_false() {
        let params = SequentialHitParams {
            prior_hits: vec![hit(0.5, 0.5, 100.0, "ball")],
            spall_liner_present: false,
            ..default_params()
        };
        let r = evaluate_sequential_hits(&params);
        assert!(!r.spall_liner_intact, "no liner present → always false");
    }

    // ── Plate crack ──────────────────────────────────────────────────────

    #[test]
    fn plate_cracks_after_threshold_hits_in_zone() {
        // 5 hits all in the same small cluster
        let hits_in_zone: Vec<_> = (0..ZONE_CRACK_THRESHOLD)
            .map(|i| hit(0.5 + 0.001 * i as f64, 0.5, 3300.0, "ap"))
            .collect();
        let params = SequentialHitParams {
            prior_hits: hits_in_zone,
            ..default_params()
        };
        let r = evaluate_sequential_hits(&params);
        assert!(
            r.plate_cracked,
            "plate should crack after {ZONE_CRACK_THRESHOLD} hits"
        );

        // 4 hits → not cracked
        let hits_below: Vec<_> = (0..ZONE_CRACK_THRESHOLD - 1)
            .map(|i| hit(0.5 + 0.001 * i as f64, 0.5, 3300.0, "ap"))
            .collect();
        let params_below = SequentialHitParams {
            prior_hits: hits_below,
            ..default_params()
        };
        let r_below = evaluate_sequential_hits(&params_below);
        assert!(
            !r_below.plate_cracked,
            "plate should NOT crack below threshold"
        );
    }

    // ── Cumulative damage → failure ──────────────────────────────────────

    #[test]
    fn cumulative_damage_halves_thickness_at_unity() {
        // Enough energy to push D_cum >= 1.0
        // REFERENCE_ENERGY_J = 3300, so 1 hit >= 3300 J → D_cum >= 1.0
        let params = SequentialHitParams {
            prior_hits: vec![hit(0.5, 0.5, REFERENCE_ENERGY_J, "ap")],
            ..default_params()
        };
        let r = evaluate_sequential_hits(&params);
        // cumulative damage should be >= 1.0
        assert!(
            r.cumulative_damage >= 1.0,
            "D_cum should be >= 1.0, got {}",
            r.cumulative_damage
        );
        // With the various penalties, adf should reflect the 0.5 floor
        assert!(r.armor_degradation_factor <= 1.0);
    }

    #[test]
    fn cumulative_damage_increases_with_more_hits() {
        let hits_1 = vec![hit(0.5, 0.5, 1650.0, "ball")]; // D = 0.5
        let hits_2 = vec![
            hit(0.5, 0.5, 1650.0, "ball"),
            hit(0.51, 0.5, 1650.0, "ball"),
        ]; // D = 1.0
        let r1 = evaluate_sequential_hits(&SequentialHitParams {
            prior_hits: hits_1,
            ..default_params()
        });
        let r2 = evaluate_sequential_hits(&SequentialHitParams {
            prior_hits: hits_2,
            ..default_params()
        });
        assert!(
            r2.cumulative_damage > r1.cumulative_damage,
            "more hits should increase D_cum"
        );
    }

    // ── Progressive degradation ──────────────────────────────────────────

    #[test]
    fn degradation_increases_with_hit_count_in_zone() {
        let mut prev_adf = 1.0;
        for n in 1..=4 {
            let hits: Vec<_> = (0..n)
                .map(|i| hit(0.5 + 0.002 * i as f64, 0.5, 3300.0, "ap"))
                .collect();
            let r = evaluate_sequential_hits(&SequentialHitParams {
                prior_hits: hits,
                ..default_params()
            });
            assert!(
                r.armor_degradation_factor <= prev_adf,
                "hit {n} should not improve degradation ({} vs {})",
                r.armor_degradation_factor,
                prev_adf,
            );
            prev_adf = r.armor_degradation_factor;
        }
    }

    // ── Remaining hits estimate ──────────────────────────────────────────

    #[test]
    fn remaining_hits_decreases_with_damage() {
        let low = SequentialHitParams {
            prior_hits: vec![hit(0.5, 0.5, 500.0, "ball")],
            ..default_params()
        };
        let high = SequentialHitParams {
            prior_hits: vec![hit(0.5, 0.5, 3000.0, "ap"), hit(0.51, 0.5, 3000.0, "ap")],
            ..default_params()
        };
        let r_low = evaluate_sequential_hits(&low);
        let r_high = evaluate_sequential_hits(&high);
        assert!(
            r_low.estimated_remaining_hits_to_failure >= r_high.estimated_remaining_hits_to_failure,
            "more damaged plate should have fewer remaining hits"
        );
    }

    // ── Empty hits ───────────────────────────────────────────────────────

    #[test]
    fn no_hits_pristine() {
        let r = evaluate_sequential_hits(&default_params());
        assert!(
            (r.armor_degradation_factor - 1.0).abs() < 1e-10,
            "pristine plate should have factor 1.0"
        );
        assert!(!r.plate_cracked);
        assert!(r.spall_liner_intact);
        assert!((r.cumulative_damage - 0.0).abs() < 1e-10);
        assert!(r.estimated_remaining_hits_to_failure > 0);
    }

    // ── assign_zones ─────────────────────────────────────────────────────

    #[test]
    fn zone_assignment_clustered_hits_same_zone() {
        let hits = vec![
            hit(0.0, 0.0, 0.0, "ball"),
            hit(0.005, 0.0, 0.0, "ball"), // 5 mm apart, 7.62 mm cal → same zone
            hit(0.010, 0.0, 0.0, "ball"), // 10 mm from first → still same zone (ratio 1.31)
        ];
        let zones = assign_zones(&hits, 0.00762);
        assert_eq!(zones.len(), 3);
        assert_eq!(zones[0], zones[1], "first two should share a zone");
        assert_eq!(zones[0], zones[2], "all three should share a zone");
    }

    #[test]
    fn zone_assignment_distant_hits_separate_zones() {
        let hits = vec![
            hit(0.0, 0.0, 0.0, "ball"),
            hit(0.5, 0.5, 0.0, "ball"), // far apart → new zone
        ];
        let zones = assign_zones(&hits, 0.00762);
        assert_eq!(zones.len(), 2);
        assert_ne!(
            zones[0], zones[1],
            "distant hits should be in different zones"
        );
    }

    // ── zone_degradation ─────────────────────────────────────────────────

    #[test]
    fn zone_degradation_one_hit_is_pristine() {
        assert!((zone_degradation(1) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn zone_degradation_never_below_half() {
        for n in 1..50 {
            let d = zone_degradation(n);
            assert!(
                d >= 0.49,
                "degradation should not drop below 0.5 at n={n}, got {d}"
            );
            assert!(d <= 1.0);
        }
    }

    // ── edge_proximity_factor additional ─────────────────────────────────

    #[test]
    fn edge_factor_corner_minimum() {
        let cal = 0.01;
        let f = edge_proximity_factor(0.0, 0.0, 1.0, 1.0, cal);
        assert!((f - 0.7).abs() < 1e-10, "corner should be 0.7, got {f}");
    }

    // ── Armor degradation (Gap 6) ────────────────────────────────────────

    #[test]
    fn degradation_no_hits_factor_one() {
        let params = default_params();
        let acc = compute_degradation_accumulator(&[], &params);
        let factor = armor_degradation_factor(&acc, &params);
        assert!(
            (factor - 1.0).abs() < 0.05,
            "no hits should give factor near 1.0, got {factor}",
        );
    }

    #[test]
    fn degradation_single_hit_no_meaningful_degradation() {
        let hits = vec![hit(0.5, 0.5, 1000.0, "ap")];
        let params = default_params();
        let acc = compute_degradation_accumulator(&hits, &params);
        let factor = armor_degradation_factor(&acc, &params);
        assert!(
            factor > 0.90,
            "single hit should not meaningfully degrade armor, got {factor}",
        );
    }

    #[test]
    fn degradation_few_hits_scattered_factor_near_one() {
        // 3 scattered hits in different regions → low cluster penalty
        let hits = vec![
            hit(0.1, 0.1, 1000.0, "ap"),
            hit(0.5, 0.1, 1000.0, "ap"),
            hit(0.9, 0.9, 1000.0, "ap"),
        ];
        let params = default_params();
        let acc = compute_degradation_accumulator(&hits, &params);
        let factor = armor_degradation_factor(&acc, &params);
        assert!(
            factor > 0.85,
            "scattered hits should have factor near 1.0, got {factor}",
        );
    }

    #[test]
    fn degradation_many_hits_clustered_significantly_reduced() {
        // 6 tightly clustered hits → high CDI + cluster penalty
        let hits: Vec<_> = (0..6)
            .map(|i| hit(0.5 + 0.001 * i as f64, 0.5, 3300.0, "ap"))
            .collect();
        let params = default_params();
        let acc = compute_degradation_accumulator(&hits, &params);
        let factor = armor_degradation_factor(&acc, &params);
        assert!(
            factor < 0.90,
            "clustered hits should significantly reduce factor, got {factor}",
        );
        assert!(
            factor >= 0.70,
            "factor should not drop below 0.70, got {factor}",
        );
    }
}
