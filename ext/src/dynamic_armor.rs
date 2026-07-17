// ABE - Dynamic / Energy-Absorbing Armor Models
//
// Models armour materials whose effective resistance changes with
// projectile velocity, impact angle, or multi-hit state.
//
// Physics summary:
//   Viscoelastic layer — rubber/polymer interlayer between steel plates.
//     Energy absorption via hysteresis ∝ √v · thickness · area.
//     At low velocity (<200 m/s) behaves as soft material (minimal extra
//     protection); at high velocity (>600 m/s) absorbs significant KE.
//
//   Shear-thickening fluid (STF) / Non-Newtonian — liquid armour that
//     stiffens on impact.  Transition modelled as a sigmoid centred at
//     ~300 m/s.  Below threshold → flexible fabric; above → up to 5×
//     effective hardness.
//
//   Spaced / sloped dynamic array — multiple thin plates at angles.
//     Each plate induces 1–3° yaw; after 3–4 plates even AP projectiles
//     lose significant penetration.  Air gaps between plates reduce HEAT
//     jet efficiency.
//
// References:
//   - ARL-TR-5426: "Shear Thickening Fluid (STF) Body Armor" (US Army
//     Research Lab, 2006)
//   - UK MoD DSTL: "Viscoelastic Armour Interlayer Performance"
//   - De Marre ballistic formula (KE penetration baseline)
//   - Held / hydrodynamic shaped-charge jet model (HEAT baseline)
//   - NIJ 0101.06: Ballistic resistance of body armour (STF-enabled)

// ── Physical constants ──────────────────────────────────────────────────────────

/// Transition midpoint velocity for STF and viscoelastic stiffening (m/s).
const STF_V0_MS: f64 = 300.0;

/// Sigmoid steepness for STF / viscoelastic transition.
const STF_K: f64 = 0.02;

/// Maximum effective-thickness multiplier for pure STF.
const STF_MAX_MULT: f64 = 5.0;

/// Maximum effective-thickness multiplier for viscoelastic layers.
const VISCO_MAX_MULT: f64 = 2.5;

/// Below this impact velocity (m/s) viscoelastic layers contribute
/// negligible additional protection.
const LOW_VELOCITY_THRESHOLD_MS: f64 = 200.0;

/// Above this impact velocity (m/s) viscoelastic layers operate at
/// near-peak efficiency.
const HIGH_VELOCITY_THRESHOLD_MS: f64 = 600.0;

/// Energy absorption constant for viscoelastic hysteresis:
///   E_abs = K_ABS · √v · t_mm · A_m²
const VISCO_ABS_K: f64 = 250.0;

/// Default engaged area (m²) for the viscoelastic absorption model
/// when area is not explicitly provided.  Represents the approximate
/// region over which elastic waves distribute impact energy (~16 cm²).
const DEFAULT_VISCO_AREA_M2: f64 = 0.025;

/// Average yaw induced per plate in a spaced / sloped array (degrees).
/// Each plate imparts 1–3° depending on obliquity and projectile shape.
const YAW_PER_PLATE_DEG: f64 = 2.0;

/// Maximum credible cumulative yaw from a spaced array (degrees).
/// Beyond this the projectile is tumbling and additional plates have
/// little extra effect.
const MAX_CUMULATIVE_YAW_DEG: f64 = 18.0;

/// Number of plates after which the array's HEAT disruption saturates.
/// Each air gap disturbs the shaped-charge jet; beyond ~6 gaps the
/// jet is fully broken up.
const HEAT_SATURATION_PLATES: i32 = 6;

/// HEAT efficiency reduction per air gap in a spaced array.
/// Each gap reduces the jet's effective penetration by this fraction
/// (multiplicative).  Reference: spaced armour vs HEAT test data
/// (Held, "Penetration of Shaped Charge Jets", 1997).
const HEAT_GAP_EFFICIENCY: f64 = 0.80;

// ── Types ──────────────────────────────────────────────────────────────────────

/// Types of dynamic / energy-absorbing armour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DynamicArmorType {
    /// Viscoelastic rubber/polymer interlayer between rigid plates.
    /// Energy absorbed through hysteresis; effectiveness rises with
    /// strain rate (impact velocity).
    ViscoelasticLayer {
        /// Thickness of the viscoelastic layer in mm.
        thickness_mm: f64,
        /// Material identifier (e.g. `"polyurethane"`, `"natural_rubber"`).
        material: &'static str,
    },
    /// Shear-thickening (non-Newtonian) fluid-impregnated fabric.
    /// Remains flexible below a velocity threshold; above it stiffens
    /// dramatically and behaves as a solid with high hardness.
    ShearThickeningFluid {
        /// Thickness of the STF layer / pack in mm.
        thickness_mm: f64,
        /// Carrier fabric identifier (e.g. `"kevlar"`, `"twaron"`).
        carrier_fabric: &'static str,
    },
    /// Multiple thin plates separated by air gaps and mounted at
    /// oblique angles to induce projectile yaw / break-up.
    SpacedSlopedArray {
        /// Number of individual plates in the array.
        plate_count: i32,
        /// Air gap between successive plates in mm.
        spacing_mm: f64,
        /// Thickness of each individual plate in mm.
        individual_thickness_mm: f64,
    },
    /// Multi-layer composite that combines two or more of the above
    /// technologies in a defined layering sequence.
    MultiLayerComposite {
        /// Number of distinct layers.
        layers: i32,
        /// Opaque layering configuration string (interpreted internally).
        layer_config: &'static str,
    },
}

/// Type of incoming threat.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ThreatType {
    /// Kinetic energy penetrator (bullet, AP, APFSDS rod).
    KE,
    /// Shaped charge (HEAT warhead, RPG, ATGM).
    HEAT,
    /// Fragmentation (artillery fragments, bomblets, IED debris).
    Fragment,
}

/// Input parameters for dynamic armour evaluation.
#[derive(Debug, Clone, Copy)]
pub struct DynamicArmorParams {
    /// Type and configuration of the dynamic armour.
    pub armor_type: DynamicArmorType,
    /// Impact velocity of the threat in m/s.
    pub threat_velocity_ms: f64,
    /// Mass of the threat projectile in grams.
    pub threat_mass_g: f64,
    /// Calibre (diameter) of the threat in mm.
    pub threat_caliber_mm: f64,
    /// Classification of the threat (KE, HEAT, Fragment).
    pub threat_type: ThreatType,
    /// Impact angle measured from surface normal in degrees
    /// (0 = perpendicular, 90 = grazing).
    pub impact_angle_deg: f64,
}

/// Result of a dynamic / energy-absorbing armour evaluation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DynamicArmorResult {
    /// Factor by which the armour's effective thickness is multiplied
    /// due to dynamic effects (1.0 = no enhancement).
    pub effective_thickness_multiplier: f64,
    /// Kinetic energy absorbed by the dynamic armour mechanism in joules.
    pub energy_absorbed_j: f64,
    /// Projectile residual velocity after interacting with the armour (m/s).
    /// Zero if fully stopped.
    pub residual_velocity_ms: f64,
    /// Cumulative yaw angle induced in the projectile (degrees).
    /// Zero for non-yawing armour types.
    pub projectile_yawed_deg: f64,
    /// Whether the armour's dynamic mechanism is damaged / expended
    /// after this hit (e.g., polymer degraded, STF reverted, plates pierced).
    pub armor_damaged: bool,
    /// The RHA-equivalent thickness after applying all dynamic effects (mm).
    pub computed_effective_thickness_mm: f64,
}

// ── Helpers ────────────────────────────────────────────────────────────────────

/// Sigmoidal multiplier for STF armour.
///
/// Models the transition from flexible-fabric behaviour (multiplier ≈ 1.0)
/// at low velocity to solid-like behaviour (multiplier → `max_mult`) at
/// high velocity.
///
/// Formula:
///   M(v) = 1.0 + (max_mult - 1.0) / (1 + exp(-k · (v - v0)))
///
/// where v0 = 300 m/s (transition midpoint), k = 0.02 (steepness).
pub fn stf_multiplier(velocity_ms: f64, thickness_mm: f64) -> f64 {
    if velocity_ms <= 0.0 || thickness_mm <= 0.0 {
        return 1.0;
    }
    let exponent = STF_K * (velocity_ms - STF_V0_MS);
    let sigmoid = 1.0 / (1.0 + (-exponent).exp());
    1.0 + (STF_MAX_MULT - 1.0) * sigmoid
}

/// Velocity-dependent multiplier for viscoelastic armour thickening.
///
/// Same sigmoidal form as STF but with a lower ceiling (`VISCO_MAX_MULT`).
fn viscoelastic_multiplier(velocity_ms: f64) -> f64 {
    if velocity_ms <= 0.0 {
        return 1.0;
    }
    let exponent = STF_K * (velocity_ms - STF_V0_MS);
    let sigmoid = 1.0 / (1.0 + (-exponent).exp());
    1.0 + (VISCO_MAX_MULT - 1.0) * sigmoid
}

/// Compute kinetic energy absorbed by a viscoelastic hysteresis layer.
///
/// Formula (phenomenological, fitted to DSTL test data):
///   E_abs = K · √v · t · A
///
/// where K is the absorption constant (`VISCO_ABS_K`), v is impact velocity,
/// t is layer thickness in mm, and A is the engaged area in m².
///
/// # Arguments
/// * `velocity_ms` — Impact velocity (m/s).
/// * `thickness_mm` — Viscoelastic layer thickness (mm).
/// * `area_m2` — Engaged area of the layer (m²).  Use 0.0 or negative
///   to invoke the default engaged area (`DEFAULT_VISCO_AREA_M2`).
pub fn viscoelastic_absorption(velocity_ms: f64, thickness_mm: f64, area_m2: f64) -> f64 {
    if velocity_ms <= 0.0 || thickness_mm <= 0.0 {
        return 0.0;
    }
    let area = if area_m2 > 0.0 {
        area_m2
    } else {
        DEFAULT_VISCO_AREA_M2
    };
    VISCO_ABS_K * velocity_ms.sqrt() * thickness_mm * area
}

/// Replica of `penetration::yaw_penalty` for use in this module.
///
/// Yaw increases the projectile's presented cross-section, reducing
/// penetration efficiency.  Returns a factor in [0.1, 1.0] where
/// 1.0 = no yaw and lower values = degraded penetration.
fn yaw_penalty(yaw_angle_deg: f64) -> f64 {
    let yaw_rad = yaw_angle_deg.to_radians();
    // penalty = cos(yaw)^0.5 / (1 + 0.5·sin(yaw))
    let penalty = yaw_rad.cos().sqrt() / (1.0 + 0.5 * yaw_rad.sin());
    penalty.max(0.1).min(1.0)
}

/// Kinetic energy of a projectile in joules.
fn kinetic_energy(mass_kg: f64, velocity_ms: f64) -> f64 {
    0.5 * mass_kg * velocity_ms.powi(2)
}

// ── Core evaluation ────────────────────────────────────────────────────────────

/// Evaluate a dynamic / energy-absorbing armour response to a threat.
///
/// # Arguments
/// * `params` — Armour configuration, threat parameters, and engagement
///   geometry.
///
/// # Returns
/// A [`DynamicArmorResult`] with the effective thickness multiplier,
/// absorbed energy, residual velocity, induced yaw, damage state, and
/// RHA-equivalent thickness.
///
/// # Behaviour by armour type
///
/// **ViscoelasticLayer** — The effective thickness multiplier follows the
/// same sigmoid as the absorption curve, rising from ~1.0 at <200 m/s to
/// `VISCO_MAX_MULT` (2.5×) at >600 m/s.  Kinetic energy is absorbed via
/// the hysteresis term and subtracted from the projectile's KE.
///
/// **ShearThickeningFluid** — The effective thickness multiplier follows
/// the STF sigmoid: 1.0 at low velocity, up to 5.0× at high velocity.
/// Energy absorption is the difference between post-stiffening effective
/// resistance and the baseline (below-threshold) resistance.
///
/// **SpacedSlopedArray** — Each plate induces ~2° of yaw on KE
/// projectiles.  Cumulative yaw reduces penetration efficiency via the
/// yaw penalty (lower penalty → armour is effectively thicker).  Air
/// gaps between plates additionally disrupt HEAT jets (each gap
/// multiply reduces HEAT efficiency by `HEAT_GAP_EFFICIENCY`).
///
/// **MultiLayerComposite** — Delegates to a config-string-based layer
/// model (see internal helper).
pub fn evaluate_dynamic_armor(params: &DynamicArmorParams) -> DynamicArmorResult {
    // Guard: zero or negative velocity / mass → inert result
    if params.threat_velocity_ms <= 0.0 || params.threat_mass_g <= 0.0 {
        return DynamicArmorResult {
            effective_thickness_multiplier: 1.0,
            energy_absorbed_j: 0.0,
            residual_velocity_ms: params.threat_velocity_ms.max(0.0),
            projectile_yawed_deg: 0.0,
            armor_damaged: false,
            computed_effective_thickness_mm: 0.0,
        };
    }

    let mass_kg = params.threat_mass_g / 1000.0;
    let ke = kinetic_energy(mass_kg, params.threat_velocity_ms);

    match params.armor_type {
        DynamicArmorType::ViscoelasticLayer {
            thickness_mm,
            material: _,
        } => {
            let mult = viscoelastic_multiplier(params.threat_velocity_ms);
            let e_abs = viscoelastic_absorption(
                params.threat_velocity_ms,
                thickness_mm,
                DEFAULT_VISCO_AREA_M2,
            );
            let e_abs = e_abs.min(ke);

            let residual_ke = ke - e_abs;
            let residual_v = if residual_ke > 0.0 {
                (2.0 * residual_ke / mass_kg).sqrt()
            } else {
                0.0
            };

            let effective = thickness_mm * mult;
            let damaged = params.threat_velocity_ms > LOW_VELOCITY_THRESHOLD_MS && e_abs > 0.0;

            DynamicArmorResult {
                effective_thickness_multiplier: mult,
                energy_absorbed_j: e_abs,
                residual_velocity_ms: residual_v,
                projectile_yawed_deg: 0.0,
                armor_damaged: damaged,
                computed_effective_thickness_mm: effective,
            }
        }

        DynamicArmorType::ShearThickeningFluid {
            thickness_mm,
            carrier_fabric: _,
        } => {
            let mult = stf_multiplier(params.threat_velocity_ms, thickness_mm);

            // Energy absorption modelled as the work done by the
            // difference between stiffened and baseline resistance:
            // above threshold the armour resists more, absorbing
            // extra energy from the projectile.
            let vel_ratio = (params.threat_velocity_ms / STF_V0_MS).min(3.0);
            let frac_abs = if params.threat_velocity_ms > STF_V0_MS {
                // Above threshold: absorption fraction grows with sigmoid overshoot
                let saturation = (mult - 1.0) / (STF_MAX_MULT - 1.0); // 0-1
                0.05 + 0.40 * saturation
            } else {
                // Below threshold: minimal absorption (fluid is compliant)
                0.02 * vel_ratio
            };
            let e_abs = (ke * frac_abs).min(ke);

            let residual_ke = ke - e_abs;
            let residual_v = if residual_ke > 0.0 {
                (2.0 * residual_ke / mass_kg).sqrt()
            } else {
                0.0
            };

            let effective = thickness_mm * mult;
            let damaged = params.threat_velocity_ms > STF_V0_MS && e_abs > ke * 0.05;

            DynamicArmorResult {
                effective_thickness_multiplier: mult,
                energy_absorbed_j: e_abs,
                residual_velocity_ms: residual_v,
                projectile_yawed_deg: 0.0,
                armor_damaged: damaged,
                computed_effective_thickness_mm: effective,
            }
        }

        DynamicArmorType::SpacedSlopedArray {
            plate_count,
            spacing_mm: _,
            individual_thickness_mm,
        } => {
            let n = plate_count.max(1);

            match params.threat_type {
                ThreatType::HEAT => {
                    // HEAT: air gaps between plates disrupt the shaped-charge jet.
                    // Each gap reduces effective penetration multiplicatively.
                    // The plates themselves offer some armour thickness too.
                    let saturated_n = n.min(HEAT_SATURATION_PLATES);
                    let gap_mult = HEAT_GAP_EFFICIENCY.powi(saturated_n);
                    let base_thickness = n as f64 * individual_thickness_mm;

                    // HEAT jets are disrupted by spacing; total disruption
                    // factor combines gap efficiency with a spacing bonus.
                    let spacing_bonus = 1.0 + 0.02 * (saturated_n as f64);
                    let total_mult = (1.0 / gap_mult.max(0.01)) * spacing_bonus;

                    let e_abs = ke * 0.60; // HEAT jets lose ~60 % energy to disruption
                    let residual_ke = ke - e_abs;
                    let residual_v = if residual_ke > 0.0 {
                        (2.0 * residual_ke / mass_kg).sqrt()
                    } else {
                        0.0
                    };

                    DynamicArmorResult {
                        effective_thickness_multiplier: total_mult,
                        energy_absorbed_j: e_abs,
                        residual_velocity_ms: residual_v,
                        projectile_yawed_deg: 0.0,
                        armor_damaged: true,
                        computed_effective_thickness_mm: base_thickness * total_mult,
                    }
                }

                _ => {
                    // KE / Fragment: each plate induces cumulative yaw.
                    let total_yaw = (n as f64 * YAW_PER_PLATE_DEG).min(MAX_CUMULATIVE_YAW_DEG);
                    let penalty = yaw_penalty(total_yaw);

                    // The yaw penalty divides the effective thickness in the
                    // penetration model (lower penalty = more effective armour).
                    // Here we express this as a multiplier ≥ 1.0.
                    let yaw_mult = 1.0 / penalty.max(0.1);
                    let base_thickness = n as f64 * individual_thickness_mm;

                    // Additional effect: oblique plate array induces
                    // tumbling and energy loss through plate impacts.
                    let plate_energy_loss = ke * 0.05 * (n as f64).min(5.0);
                    let e_abs = plate_energy_loss.min(ke);
                    let residual_ke = ke - e_abs;
                    let residual_v = if residual_ke > 0.0 {
                        (2.0 * residual_ke / mass_kg).sqrt()
                    } else {
                        0.0
                    };

                    DynamicArmorResult {
                        effective_thickness_multiplier: yaw_mult,
                        energy_absorbed_j: e_abs,
                        residual_velocity_ms: residual_v,
                        projectile_yawed_deg: total_yaw,
                        armor_damaged: total_yaw > 3.0,
                        computed_effective_thickness_mm: base_thickness * yaw_mult,
                    }
                }
            }
        }

        DynamicArmorType::MultiLayerComposite {
            layers,
            layer_config,
        } => evaluate_mlc(params, layers, layer_config),
    }
}

// ── Multi-layer composite helper ───────────────────────────────────────────────

/// Evaluate a multi-layer composite armour with a layered configuration.
///
/// The `layer_config` string encodes a layering sequence using single-letter
/// codes: `V` = viscoelastic, `S` = STF, `A` = spaced/sloped array, with
/// optional repeat count suffixes.
///
/// Examples:
/// - `"V1A2"` — one viscoelastic layer on a two-plate array
/// - `"S2V1"` — two STF layers backed by one viscoelastic
/// - `"V1S1V1"` — viscoelastic–STF–viscoelastic sandwich
///
/// When the config string is empty or unrecognised the function falls
/// back to treating all layers as a generic homogeneous composite with
/// a mild multiplier (1.8×).
fn evaluate_mlc(
    params: &DynamicArmorParams,
    _layers: i32,
    layer_config: &str,
) -> DynamicArmorResult {
    let mass_kg = params.threat_mass_g / 1000.0;
    let ke = kinetic_energy(mass_kg, params.threat_velocity_ms);
    let v = params.threat_velocity_ms;

    // Parse the layer config string into component types with repeat counts.
    let components = parse_mlc_config(layer_config);
    let total_layers = components.iter().map(|(_, c)| *c).sum::<i32>().max(1);

    // Compute aggregate multiplier as weighted average of component multipliers.
    let mut weighted_mult_sum = 0.0_f64;
    let mut total_weight = 0.0_f64;
    let mut total_energy_abs = 0.0_f64;
    let mut max_yaw = 0.0_f64;

    for &(code, count) in &components {
        let weight = count as f64;
        total_weight += weight;

        match code {
            'V' => {
                let mult = viscoelastic_multiplier(v);
                let abs = viscoelastic_absorption(v, 5.0 * weight, DEFAULT_VISCO_AREA_M2);
                weighted_mult_sum += mult * weight;
                total_energy_abs += abs;
            }
            'S' => {
                let mult = stf_multiplier(v, 3.0 * weight);
                let vel_ratio = (v / STF_V0_MS).min(3.0);
                let frac_abs = if v > STF_V0_MS {
                    0.05 + 0.40 * ((mult - 1.0) / (STF_MAX_MULT - 1.0))
                } else {
                    0.02 * vel_ratio
                };
                total_energy_abs += ke * frac_abs * weight / total_layers as f64;
                weighted_mult_sum += mult * weight;
            }
            'A' => {
                let n = (count * 2).max(1); // each 'A' unit approximates 2 plates
                let total_yaw = (n as f64 * YAW_PER_PLATE_DEG * 0.7).min(MAX_CUMULATIVE_YAW_DEG);
                let penalty = yaw_penalty(total_yaw);
                let mult = 1.0 / penalty.max(0.1);
                max_yaw = max_yaw.max(total_yaw);
                weighted_mult_sum += mult * weight;
                total_energy_abs += ke * 0.05 * (n as f64).min(5.0) * weight / total_layers as f64;
            }
            _ => {
                // Unknown component: treat as mild homogeneous composite (~1.8×)
                weighted_mult_sum += 1.8 * weight;
            }
        }
    }

    let avg_mult = weighted_mult_sum / total_weight;

    // Cap absorbed energy at available KE
    let e_abs = total_energy_abs.min(ke);
    let residual_ke = ke - e_abs;
    let residual_v = if residual_ke > 0.0 {
        (2.0 * residual_ke / mass_kg).sqrt()
    } else {
        0.0
    };

    // Baseline thickness: assume average 4 mm per layer unit
    let base_thickness_mm = total_layers as f64 * 4.0;

    DynamicArmorResult {
        effective_thickness_multiplier: avg_mult,
        energy_absorbed_j: e_abs,
        residual_velocity_ms: residual_v,
        projectile_yawed_deg: max_yaw,
        armor_damaged: e_abs > ke * 0.05,
        computed_effective_thickness_mm: base_thickness_mm * avg_mult,
    }
}

/// Parse an MLC config string into a list of (component_code, repeat_count).
///
/// Format: single uppercase letter optionally followed by a repeat count
/// digit.  Repeated letters are accumulated.
///
/// Example: `"V1A2"` → `[('V', 1), ('A', 2)]`
/// `"VVS"` → `[('V', 2), ('S', 1)]`
fn parse_mlc_config(config: &str) -> Vec<(char, i32)> {
    if config.is_empty() {
        return vec![('V', 1)]; // default fallback
    }

    let mut result: Vec<(char, i32)> = Vec::new();
    let chars: Vec<char> = config.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let code = chars[i];
        if !code.is_ascii_uppercase() {
            i += 1;
            continue;
        }

        // Read optional repeat count (single digit)
        let count = if i + 1 < chars.len() && chars[i + 1].is_ascii_digit() {
            let d = chars[i + 1].to_digit(10).unwrap_or(1) as i32;
            i += 2;
            d
        } else {
            i += 1;
            1
        };

        // Merge with last component if same code
        if let Some(last) = result.last_mut() {
            if last.0 == code {
                last.1 += count;
                continue;
            }
        }
        result.push((code, count));
    }

    if result.is_empty() {
        result.push(('V', 1));
    }
    result
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── STF multiplier tests ──────────────────────────────────────────────

    #[test]
    fn stf_below_threshold_near_unity() {
        let m = stf_multiplier(100.0, 5.0);
        // At 100 m/s (well below 300 m/s midpoint) the multiplier
        // should be close to 1.0 (flexible fabric behaviour).
        assert!(
            (m - 1.0).abs() < 0.15,
            "STF multiplier at 100 m/s should be near 1.0, got {:.4}",
            m
        );
    }

    #[test]
    fn stf_above_threshold_approaches_max() {
        let m = stf_multiplier(900.0, 5.0);
        // At 900 m/s the sigmoid should be saturated near STF_MAX_MULT (5.0).
        assert!(
            m > STF_MAX_MULT * 0.90,
            "STF multiplier at 900 m/s should be near {:.1}, got {:.4}",
            STF_MAX_MULT,
            m
        );
    }

    #[test]
    fn stf_at_midpoint_halfway() {
        let m = stf_multiplier(STF_V0_MS, 5.0);
        // At the sigmoid midpoint (300 m/s) the multiplier should be
        // halfway between 1.0 and STF_MAX_MULT.
        let expected = 1.0 + (STF_MAX_MULT - 1.0) * 0.5;
        assert!(
            (m - expected).abs() < 0.02,
            "STF multiplier at midpoint should be ~{:.2}, got {:.4}",
            expected,
            m
        );
    }

    #[test]
    fn stf_multiplier_zero_velocity_returns_one() {
        assert_eq!(stf_multiplier(0.0, 5.0), 1.0);
        assert_eq!(stf_multiplier(-1.0, 5.0), 1.0);
    }

    // ── Viscoelastic helper tests ─────────────────────────────────────────

    #[test]
    fn viscoelastic_absorption_scales_with_velocity() {
        let low = viscoelastic_absorption(100.0, 5.0, 0.025);
        let high = viscoelastic_absorption(800.0, 5.0, 0.025);
        assert!(
            high > low * 2.0,
            "High-velocity absorption ({:.1} J) should exceed low-velocity ({:.1} J) by >2x",
            high,
            low
        );
    }

    #[test]
    fn viscoelastic_absorption_scales_with_thickness() {
        let thin = viscoelastic_absorption(500.0, 3.0, 0.025);
        let thick = viscoelastic_absorption(500.0, 10.0, 0.025);
        assert!(
            thick > thin * 2.5,
            "Thicker layer ({:.1} J) should absorb >2.5x thinner ({:.1} J)",
            thick,
            thin
        );
    }

    #[test]
    fn viscoelastic_absorption_zero_inputs() {
        assert_eq!(viscoelastic_absorption(0.0, 5.0, 0.025), 0.0);
        assert_eq!(viscoelastic_absorption(500.0, 0.0, 0.025), 0.0);
        assert_eq!(viscoelastic_absorption(0.0, 0.0, 0.0), 0.0);
    }

    // ── Viscoelastic layer evaluation tests ───────────────────────────────

    #[test]
    fn viscoelastic_low_velocity_soft_behaviour() {
        // Below 200 m/s the viscoelastic layer contributes minimal
        // additional protection.
        let params = DynamicArmorParams {
            armor_type: DynamicArmorType::ViscoelasticLayer {
                thickness_mm: 10.0,
                material: "polyurethane",
            },
            threat_velocity_ms: 100.0,
            threat_mass_g: 9.5,
            threat_caliber_mm: 7.62,
            threat_type: ThreatType::KE,
            impact_angle_deg: 0.0,
        };
        let r = evaluate_dynamic_armor(&params);
        assert!(
            r.effective_thickness_multiplier < 1.3,
            "Low-velocity viscoelastic multiplier ({:.3}) should be modest (< 1.3)",
            r.effective_thickness_multiplier
        );
        assert!(
            r.energy_absorbed_j < 50.0,
            "Low-velocity absorption ({:.1} J) should be minimal (< 50 J)",
            r.energy_absorbed_j
        );
        assert!(
            !r.armor_damaged,
            "Low-velocity should not damage viscoelastic"
        );
    }

    #[test]
    fn viscoelastic_high_velocity_significant_absorption() {
        // Above 600 m/s the viscoelastic layer should absorb a meaningful
        // fraction of the projectile's KE.
        let params = DynamicArmorParams {
            armor_type: DynamicArmorType::ViscoelasticLayer {
                thickness_mm: 10.0,
                material: "polyurethane",
            },
            threat_velocity_ms: 850.0,
            threat_mass_g: 9.5,
            threat_caliber_mm: 7.62,
            threat_type: ThreatType::KE,
            impact_angle_deg: 0.0,
        };
        let r = evaluate_dynamic_armor(&params);
        let ke = 0.5 * (9.5 / 1000.0) * 850.0_f64.powi(2);
        assert!(
            r.effective_thickness_multiplier > 1.8,
            "High-velocity viscoelastic multiplier ({:.3}) should exceed 1.8",
            r.effective_thickness_multiplier
        );
        assert!(
            r.energy_absorbed_j > ke * 0.15,
            "High-velocity absorption ({:.1} J) should exceed 15% of KE ({:.1} J)",
            r.energy_absorbed_j,
            ke * 0.15
        );
        assert!(
            r.armor_damaged,
            "High-velocity impact should damage viscoelastic layer"
        );
    }

    // ── STF evaluation tests ──────────────────────────────────────────────

    #[test]
    fn stf_low_velocity_minimal_effect() {
        let params = DynamicArmorParams {
            armor_type: DynamicArmorType::ShearThickeningFluid {
                thickness_mm: 5.0,
                carrier_fabric: "kevlar",
            },
            threat_velocity_ms: 100.0,
            threat_mass_g: 4.0,
            threat_caliber_mm: 5.56,
            threat_type: ThreatType::KE,
            impact_angle_deg: 0.0,
        };
        let r = evaluate_dynamic_armor(&params);
        assert!(
            r.effective_thickness_multiplier < 1.3,
            "STF at low v: multiplier ({:.3}) should be < 1.3",
            r.effective_thickness_multiplier
        );
    }

    #[test]
    fn stf_high_velocity_approaches_max() {
        let params = DynamicArmorParams {
            armor_type: DynamicArmorType::ShearThickeningFluid {
                thickness_mm: 5.0,
                carrier_fabric: "kevlar",
            },
            threat_velocity_ms: 1000.0,
            threat_mass_g: 4.0,
            threat_caliber_mm: 5.56,
            threat_type: ThreatType::KE,
            impact_angle_deg: 0.0,
        };
        let r = evaluate_dynamic_armor(&params);
        assert!(
            r.effective_thickness_multiplier > 4.0,
            "STF at high v: multiplier ({:.3}) should exceed 4.0",
            r.effective_thickness_multiplier
        );
        assert!(
            r.energy_absorbed_j > 0.0,
            "STF at high v should absorb energy"
        );
    }

    // ── Spaced / sloped array tests ───────────────────────────────────────

    #[test]
    fn spaced_array_induces_yaw_on_ke() {
        let params = DynamicArmorParams {
            armor_type: DynamicArmorType::SpacedSlopedArray {
                plate_count: 4,
                spacing_mm: 50.0,
                individual_thickness_mm: 3.0,
            },
            threat_velocity_ms: 850.0,
            threat_mass_g: 9.5,
            threat_caliber_mm: 7.62,
            threat_type: ThreatType::KE,
            impact_angle_deg: 0.0,
        };
        let r = evaluate_dynamic_armor(&params);
        assert!(
            r.projectile_yawed_deg > 4.0,
            "Spaced array should induce yaw ({:.1}°) > 4°",
            r.projectile_yawed_deg
        );
        assert!(
            r.effective_thickness_multiplier > 1.0,
            "Spaced array multiplier ({:.3}) should exceed 1.0",
            r.effective_thickness_multiplier
        );
    }

    #[test]
    fn spaced_array_heat_disruption() {
        let params = DynamicArmorParams {
            armor_type: DynamicArmorType::SpacedSlopedArray {
                plate_count: 4,
                spacing_mm: 100.0,
                individual_thickness_mm: 2.0,
            },
            threat_velocity_ms: 800.0,
            threat_mass_g: 200.0,
            threat_caliber_mm: 40.0,
            threat_type: ThreatType::HEAT,
            impact_angle_deg: 0.0,
        };
        let r = evaluate_dynamic_armor(&params);
        // HEAT should see a high effective multiplier from gap disruption
        assert!(
            r.effective_thickness_multiplier > 2.0,
            "Spaced array vs HEAT: multiplier ({:.3}) should exceed 2.0",
            r.effective_thickness_multiplier
        );
        assert!(
            r.energy_absorbed_j > 0.0,
            "Spaced array should absorb HEAT jet energy"
        );
    }

    #[test]
    fn spaced_array_single_plate_minimal_yaw() {
        let params = DynamicArmorParams {
            armor_type: DynamicArmorType::SpacedSlopedArray {
                plate_count: 1,
                spacing_mm: 20.0,
                individual_thickness_mm: 5.0,
            },
            threat_velocity_ms: 850.0,
            threat_mass_g: 9.5,
            threat_caliber_mm: 7.62,
            threat_type: ThreatType::KE,
            impact_angle_deg: 0.0,
        };
        let r = evaluate_dynamic_armor(&params);
        assert!(
            r.projectile_yawed_deg <= 3.0,
            "Single plate: yaw ({:.1}°) should be ≤ 3°",
            r.projectile_yawed_deg
        );
    }

    // ── Multi-layer composite tests ───────────────────────────────────────

    #[test]
    fn mlc_viscoelastic_stf_sandwich() {
        // V1S1V1: viscoelastic–STF–viscoelastic sandwich
        let params = DynamicArmorParams {
            armor_type: DynamicArmorType::MultiLayerComposite {
                layers: 3,
                layer_config: "V1S1V1",
            },
            threat_velocity_ms: 850.0,
            threat_mass_g: 9.5,
            threat_caliber_mm: 7.62,
            threat_type: ThreatType::KE,
            impact_angle_deg: 0.0,
        };
        let r = evaluate_dynamic_armor(&params);
        // A three-layer composite should provide meaningful enhancement
        assert!(
            r.effective_thickness_multiplier > 1.5,
            "MLC sandwich multiplier ({:.3}) should exceed 1.5",
            r.effective_thickness_multiplier
        );
        assert!(r.energy_absorbed_j > 0.0, "MLC should absorb energy");
        assert!(
            r.computed_effective_thickness_mm > 10.0,
            "MLC effective thickness ({:.1} mm) should exceed 10 mm",
            r.computed_effective_thickness_mm
        );
    }

    // ── Threat-type sensitivity tests ─────────────────────────────────────

    #[test]
    fn stf_ke_vs_heat_same_params() {
        // KE and HEAT threats with same velocity against STF should
        // produce the same effective multiplier (STF response is
        // velocity-dependent, not threat-type-dependent).
        let ke_params = DynamicArmorParams {
            armor_type: DynamicArmorType::ShearThickeningFluid {
                thickness_mm: 5.0,
                carrier_fabric: "kevlar",
            },
            threat_velocity_ms: 600.0,
            threat_mass_g: 10.0,
            threat_caliber_mm: 10.0,
            threat_type: ThreatType::KE,
            impact_angle_deg: 0.0,
        };
        let heat_params = DynamicArmorParams {
            threat_type: ThreatType::HEAT,
            ..ke_params
        };
        let r_ke = evaluate_dynamic_armor(&ke_params);
        let r_heat = evaluate_dynamic_armor(&heat_params);
        assert!(
            (r_ke.effective_thickness_multiplier - r_heat.effective_thickness_multiplier).abs()
                < 0.01,
            "STF multiplier should be same for KE ({:.4}) and HEAT ({:.4})",
            r_ke.effective_thickness_multiplier,
            r_heat.effective_thickness_multiplier
        );
    }

    #[test]
    fn spaced_array_ke_vs_heat_different_response() {
        // Spaced array responds very differently to KE vs HEAT:
        // KE → yaw-based disruption, HEAT → gap-based disruption.
        let ke_params = DynamicArmorParams {
            armor_type: DynamicArmorType::SpacedSlopedArray {
                plate_count: 4,
                spacing_mm: 50.0,
                individual_thickness_mm: 3.0,
            },
            threat_velocity_ms: 800.0,
            threat_mass_g: 100.0,
            threat_caliber_mm: 20.0,
            threat_type: ThreatType::KE,
            impact_angle_deg: 0.0,
        };
        let heat_params = DynamicArmorParams {
            threat_type: ThreatType::HEAT,
            ..ke_params
        };
        let r_ke = evaluate_dynamic_armor(&ke_params);
        let r_heat = evaluate_dynamic_armor(&heat_params);
        // Both should provide some enhancement
        assert!(r_ke.effective_thickness_multiplier > 1.0);
        assert!(r_heat.effective_thickness_multiplier > 1.0);
        // At minimum, they should differ (different mechanisms)
        assert!(
            (r_ke.effective_thickness_multiplier - r_heat.effective_thickness_multiplier).abs()
                > 0.01,
            "KE ({:.3}) and HEAT ({:.3}) multipliers should differ for spaced array",
            r_ke.effective_thickness_multiplier,
            r_heat.effective_thickness_multiplier
        );
    }

    // ── Determinism ───────────────────────────────────────────────────────

    #[test]
    fn evaluate_is_deterministic() {
        let params = DynamicArmorParams {
            armor_type: DynamicArmorType::ViscoelasticLayer {
                thickness_mm: 8.0,
                material: "polyurethane",
            },
            threat_velocity_ms: 650.0,
            threat_mass_g: 9.5,
            threat_caliber_mm: 7.62,
            threat_type: ThreatType::KE,
            impact_angle_deg: 5.0,
        };
        let a = evaluate_dynamic_armor(&params);
        let b = evaluate_dynamic_armor(&params);
        assert_eq!(a, b, "Dynamic armor evaluation must be deterministic");
    }

    // ── Edge cases ────────────────────────────────────────────────────────

    #[test]
    fn zero_velocity_returns_safe_default() {
        let params = DynamicArmorParams {
            armor_type: DynamicArmorType::ViscoelasticLayer {
                thickness_mm: 10.0,
                material: "polyurethane",
            },
            threat_velocity_ms: 0.0,
            threat_mass_g: 9.5,
            threat_caliber_mm: 7.62,
            threat_type: ThreatType::KE,
            impact_angle_deg: 0.0,
        };
        let r = evaluate_dynamic_armor(&params);
        assert_eq!(r.effective_thickness_multiplier, 1.0);
        assert_eq!(r.energy_absorbed_j, 0.0);
        assert_eq!(r.residual_velocity_ms, 0.0);
        assert!(!r.armor_damaged);
    }

    #[test]
    fn zero_mass_returns_safe_default() {
        let params = DynamicArmorParams {
            armor_type: DynamicArmorType::ShearThickeningFluid {
                thickness_mm: 5.0,
                carrier_fabric: "kevlar",
            },
            threat_velocity_ms: 500.0,
            threat_mass_g: 0.0,
            threat_caliber_mm: 5.56,
            threat_type: ThreatType::KE,
            impact_angle_deg: 0.0,
        };
        let r = evaluate_dynamic_armor(&params);
        assert_eq!(r.effective_thickness_multiplier, 1.0);
        assert_eq!(r.energy_absorbed_j, 0.0);
    }

    #[test]
    fn mlc_empty_config_falls_back() {
        let params = DynamicArmorParams {
            armor_type: DynamicArmorType::MultiLayerComposite {
                layers: 1,
                layer_config: "",
            },
            threat_velocity_ms: 700.0,
            threat_mass_g: 9.5,
            threat_caliber_mm: 7.62,
            threat_type: ThreatType::KE,
            impact_angle_deg: 0.0,
        };
        let r = evaluate_dynamic_armor(&params);
        // Fallback should produce a mild multiplier (viscoelastic default)
        assert!(
            r.effective_thickness_multiplier >= 1.0,
            "Empty MLC config should fall back to valid multiplier: {:.3}",
            r.effective_thickness_multiplier
        );
    }

    // ── Parser tests ──────────────────────────────────────────────────────

    #[test]
    fn parse_mlc_v1a2() {
        let result = parse_mlc_config("V1A2");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], ('V', 1));
        assert_eq!(result[1], ('A', 2));
    }

    #[test]
    fn parse_mlc_empty_fallback() {
        let result = parse_mlc_config("");
        assert_eq!(result, vec![('V', 1)]);
    }

    // ── Multi-layer composite: STF-array sandwich ─────────────────────────

    #[test]
    fn mlc_stf_array_sandwich_vs_heat() {
        // S2A2: two STF layers + two-plate array; effective vs HEAT
        let params = DynamicArmorParams {
            armor_type: DynamicArmorType::MultiLayerComposite {
                layers: 4,
                layer_config: "S2A2",
            },
            threat_velocity_ms: 800.0,
            threat_mass_g: 200.0,
            threat_caliber_mm: 30.0,
            threat_type: ThreatType::HEAT,
            impact_angle_deg: 0.0,
        };
        let r = evaluate_dynamic_armor(&params);
        assert!(
            r.effective_thickness_multiplier > 1.5,
            "MLC S2A2 vs HEAT: multiplier ({:.3}) should exceed 1.5",
            r.effective_thickness_multiplier
        );
    }
}
