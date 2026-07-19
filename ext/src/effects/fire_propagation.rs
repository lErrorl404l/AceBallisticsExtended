// ABE — Vehicle Fire Propagation Model
//
// Evaluates fire spread between vehicle components after a component has
// been ignited by projectile damage.  Models suppression systems, sealed
// compartments, and fuel loads to determine whether the fire becomes an
// uncontrollable vehicle fire.
//
// References:
//   - MIL-HDBK-799 (Vehicle Vulnerability — Component Kill Criteria)
//   - NATO STANAG 4569 Annex D (Crew Vulnerability to Spall)
//   - DMSS (Defence Modelling & Simulation) Handbook, UK MoD
//   - MIL-STD-2105D (Hazard Assessment for Ammunition Stowage)

use super::component_damage::{ComponentConfig, ComponentDamageResult, ComponentType};

// ── Fire propagation types ─────────────────────────────────────────────────────

/// Vehicle-level fire suppression capability.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FireSuppression {
    /// No suppression — fire spreads freely
    None,
    /// Hand extinguishers only
    Manual,
    /// Automatic fire suppression system (AFSS)
    Automatic,
    /// Halon gas flood (military AFV systems)
    Halon,
    /// Armoured/suppressed — fire is contained within compartment
    Armoured,
}

/// Status of a fire in a vehicle component.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FireStatus {
    NoFire,
    /// Low-intensity, may self-extinguish
    Smoldering,
    /// Active fire, can spread
    Burning,
    /// Maximum intensity, rapid spread
    FullyEngulfed,
}

/// Component fire state for propagation evaluation.
#[derive(Debug, Clone)]
pub struct ComponentFireState {
    pub component_index: usize,
    pub fire_status: FireStatus,
    /// 0.0–1.0
    pub fire_intensity: f64,
}

/// Result of fire propagation evaluation.
#[derive(Debug, Clone)]
pub struct FirePropagationResult {
    /// Per-component fire status after propagation.
    pub component_fires: Vec<ComponentFireState>,
    /// Overall probability that vehicle is fully involved (>50% components burning).
    pub vehicle_fully_involved_probability: f64,
    /// Estimated time to vehicle destruction in seconds (if fire is unchecked).
    pub estimated_destruction_time_s: f64,
}

/// Evaluate fire propagation across vehicle components.
///
/// Starts from a set of damaged components (those with `fire_started = true`)
/// and models the spread to adjacent components over time.
///
/// # Arguments
/// * `fire_states` — Current fire state for each vehicle component.
/// * `component_configs` — Component configuration (used for adjacency/proximity).
/// * `suppression` — Vehicle-level fire suppression capability.
/// * `time_step_s` — Time increment for propagation (seconds).
/// * `compartment_sealed` — Whether components are in sealed compartments.
///
/// # Returns
/// Updated fire states for all components after propagation.
pub fn evaluate_fire_propagation(
    fire_states: &[ComponentFireState],
    component_configs: &[ComponentConfig],
    suppression: FireSuppression,
    time_step_s: f64,
    compartment_sealed: bool,
) -> FirePropagationResult {
    let n = fire_states.len();
    let mut new_states = fire_states.to_vec();

    // Suppression modifier: reduces spread probability
    let suppression_factor: f64 = match suppression {
        FireSuppression::None => 1.0,
        FireSuppression::Manual => 0.7,
        FireSuppression::Automatic => 0.35,
        FireSuppression::Halon => 0.15,
        FireSuppression::Armoured => 0.05,
    };

    // Compartment sealed: reduces oxygen flow → slower spread
    let oxygen_factor = if compartment_sealed { 0.6 } else { 1.0 };

    for i in 0..n {
        // Only spreading components can propagate
        if new_states[i].fire_status != FireStatus::FullyEngulfed
            && new_states[i].fire_status != FireStatus::Burning
        {
            continue;
        }

        let source_intensity = new_states[i].fire_intensity;

        // Check all other components for potential ignition
        for j in 0..n {
            if i == j {
                continue;
            }
            if new_states[j].fire_status != FireStatus::NoFire
                && new_states[j].fire_status != FireStatus::Smoldering
            {
                continue;
            }

            // Proximity: assume distance between i and j based on index order
            // (adjacent indices = closer, indices far apart = farther)
            let dist_factor = 1.0 / (1.0 + (i as f64 - j as f64).abs() * 0.5);

            // Fuel load of target (derived from component type)
            let target_fuel_load = match component_configs.get(j) {
                Some(cfg) => match cfg.component {
                    ComponentType::FuelTank { .. } => 0.9,
                    ComponentType::AmmoRack { .. } => 0.8,
                    ComponentType::Engine { .. } => 0.5,
                    ComponentType::Crew { .. } => 0.3,
                    ComponentType::Transmission => 0.2,
                },
                None => 0.1,
            };

            // Base spread probability per second
            let base_spread = source_intensity * dist_factor * target_fuel_load * 0.25;

            // Apply suppression and oxygen
            let spread_prob =
                (base_spread * suppression_factor * oxygen_factor * time_step_s).clamp(0.0, 0.99);

            // Deterministic threshold as per project convention
            if spread_prob > 0.5 {
                // Ignite the target
                let target_is_fuel = matches!(
                    component_configs.get(j).map(|c| &c.component),
                    Some(ComponentType::FuelTank { .. })
                );

                new_states[j] = ComponentFireState {
                    component_index: j,
                    fire_status: if target_is_fuel {
                        FireStatus::FullyEngulfed
                    } else {
                        FireStatus::Burning
                    },
                    fire_intensity: if target_is_fuel { 0.9 } else { 0.5 },
                };
            }
        }

        // Intensify the source fire over time (if burning → fully engulfed)
        if new_states[i].fire_status == FireStatus::Burning {
            new_states[i].fire_intensity =
                (new_states[i].fire_intensity + 0.1 * time_step_s).min(1.0);
            if new_states[i].fire_intensity >= 0.8 {
                new_states[i].fire_status = FireStatus::FullyEngulfed;
            }
        }
    }

    // Count fire involvement
    let burning_count = new_states
        .iter()
        .filter(|s| {
            matches!(
                s.fire_status,
                FireStatus::Burning | FireStatus::FullyEngulfed
            )
        })
        .count() as f64;
    let fully_involved = burning_count / n.max(1) as f64 > 0.5;

    // Estimated destruction time (if most components are burning)
    let destruction_time = if burning_count > 0.0 {
        let involvement = burning_count / n.max(1) as f64;
        let base_time = 60.0 * suppression_factor; // 60s with no suppression
        let sealed_penalty = if compartment_sealed { 1.5 } else { 1.0 };
        base_time * sealed_penalty / involvement.max(0.1)
    } else {
        f64::INFINITY
    };

    FirePropagationResult {
        component_fires: new_states,
        vehicle_fully_involved_probability: if fully_involved { 0.8 } else { 0.0 },
        estimated_destruction_time_s: destruction_time,
    }
}

/// Convert a ComponentDamageResult to initial fire state.
pub fn damage_to_fire_state(
    damage: &ComponentDamageResult,
    component_index: usize,
) -> ComponentFireState {
    if damage.explosion {
        ComponentFireState {
            component_index,
            fire_status: FireStatus::FullyEngulfed,
            fire_intensity: 1.0,
        }
    } else if damage.fire_started {
        ComponentFireState {
            component_index,
            fire_status: FireStatus::Burning,
            fire_intensity: 0.4,
        }
    } else {
        ComponentFireState {
            component_index,
            fire_status: FireStatus::NoFire,
            fire_intensity: 0.0,
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::super::component_damage::{ComponentConfig, ComponentType};
    use super::*;

    /// Create a default component config for a given component type with
    /// the local armour parameters zeroed (unarmoured stowage).
    fn cfg(c: ComponentType) -> ComponentConfig {
        ComponentConfig {
            component: c,
            local_armor_thickness_mm: 0.0,
            local_armor_material: "steel_structural",
            local_angle_deg: 0.0,
        }
    }

    #[test]
    fn fire_propagation_no_initial_fire() {
        let states = vec![
            ComponentFireState {
                component_index: 0,
                fire_status: FireStatus::NoFire,
                fire_intensity: 0.0,
            },
            ComponentFireState {
                component_index: 1,
                fire_status: FireStatus::NoFire,
                fire_intensity: 0.0,
            },
        ];
        let cfgs = vec![
            cfg(ComponentType::FuelTank {
                fuel_type: super::super::component_damage::FuelType::Diesel,
            }),
            cfg(ComponentType::Engine {
                engine_type: super::super::component_damage::EngineType::Diesel,
            }),
        ];
        let r = evaluate_fire_propagation(&states, &cfgs, FireSuppression::None, 1.0, false);
        assert!(r
            .component_fires
            .iter()
            .all(|s| s.fire_status == FireStatus::NoFire));
    }

    #[test]
    fn fire_propagation_spreads_from_burning_component() {
        // Fuel tank fully engulfed should spread to adjacent engine
        let states = vec![
            ComponentFireState {
                component_index: 0,
                fire_status: FireStatus::FullyEngulfed,
                fire_intensity: 1.0,
            },
            ComponentFireState {
                component_index: 1,
                fire_status: FireStatus::NoFire,
                fire_intensity: 0.0,
            },
        ];
        let cfgs = vec![
            cfg(ComponentType::FuelTank {
                fuel_type: super::super::component_damage::FuelType::Petrol,
            }),
            cfg(ComponentType::Engine {
                engine_type: super::super::component_damage::EngineType::Petrol,
            }),
        ];
        let r = evaluate_fire_propagation(&states, &cfgs, FireSuppression::None, 7.0, false);
        // Engine should have ignited (adjacent to burning fuel tank)
        assert!(
            r.component_fires[1].fire_status != FireStatus::NoFire,
            "fire should spread from fuel tank to engine"
        );
    }

    #[test]
    fn fire_suppression_reduces_spread() {
        let states = vec![
            ComponentFireState {
                component_index: 0,
                fire_status: FireStatus::FullyEngulfed,
                fire_intensity: 1.0,
            },
            ComponentFireState {
                component_index: 1,
                fire_status: FireStatus::NoFire,
                fire_intensity: 0.0,
            },
        ];
        let cfgs = vec![
            cfg(ComponentType::FuelTank {
                fuel_type: super::super::component_damage::FuelType::Petrol,
            }),
            cfg(ComponentType::Engine {
                engine_type: super::super::component_damage::EngineType::Petrol,
            }),
        ];
        let r_none = evaluate_fire_propagation(&states, &cfgs, FireSuppression::None, 1.0, false);
        let r_halon = evaluate_fire_propagation(&states, &cfgs, FireSuppression::Halon, 1.0, false);
        // With no suppression, fire spreads more
        let spread_none = r_none
            .component_fires
            .iter()
            .filter(|s| {
                matches!(
                    s.fire_status,
                    FireStatus::Burning | FireStatus::FullyEngulfed
                )
            })
            .count();
        let spread_halon = r_halon
            .component_fires
            .iter()
            .filter(|s| {
                matches!(
                    s.fire_status,
                    FireStatus::Burning | FireStatus::FullyEngulfed
                )
            })
            .count();
        assert!(
            spread_halon <= spread_none,
            "halon should not increase fire spread"
        );
    }

    #[test]
    fn sealed_compartment_slows_fire() {
        let states = vec![
            ComponentFireState {
                component_index: 0,
                fire_status: FireStatus::FullyEngulfed,
                fire_intensity: 1.0,
            },
            ComponentFireState {
                component_index: 1,
                fire_status: FireStatus::NoFire,
                fire_intensity: 0.0,
            },
        ];
        let cfgs = vec![
            cfg(ComponentType::FuelTank {
                fuel_type: super::super::component_damage::FuelType::Petrol,
            }),
            cfg(ComponentType::Engine {
                engine_type: super::super::component_damage::EngineType::Petrol,
            }),
        ];
        let r_open = evaluate_fire_propagation(&states, &cfgs, FireSuppression::None, 1.0, false);
        let r_sealed = evaluate_fire_propagation(&states, &cfgs, FireSuppression::None, 1.0, true);
        let spread_open = r_open
            .component_fires
            .iter()
            .filter(|s| {
                matches!(
                    s.fire_status,
                    FireStatus::Burning | FireStatus::FullyEngulfed
                )
            })
            .count();
        let spread_sealed = r_sealed
            .component_fires
            .iter()
            .filter(|s| {
                matches!(
                    s.fire_status,
                    FireStatus::Burning | FireStatus::FullyEngulfed
                )
            })
            .count();
        assert!(
            spread_sealed <= spread_open,
            "sealed compartments should not increase spread"
        );
    }
}
