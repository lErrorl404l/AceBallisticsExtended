// ABE - Component Kill Probability Model
//
// Statistical component kill probability model: given a projectile impact
// on a vehicle, what is the probability of killing specific components?
// Combines hit location distribution, penetration to depth, and component
// vulnerability.
//
// The model flows through three stages:
//   1. P(hit_component) = area_component / area_zone
//   2. P(penetrate_to_depth | hit) = f(residual_energy, depth, vehicle_density)
//   3. P(kill | hit ∧ pen) = f(residual_energy, component_vulnerability)
//
// Combined: P(kill) = P(hit) × P(pen) × P(kill|hit,pen)
//
// References:
//   - MIL-HDBK-799 (Vehicle Vulnerability — Component Kill Criteria)
//   - BRL CR 341 (Critical Energy Criteria for Vehicle Components)
//   - Fairlie J.P., "Behind Armour Debris Modelling" (DERA)
//   - Jane's Armour & Artillery (2022–2023)
//   - Jane's Land Warfare Platforms: Armoured Fighting Vehicles
//   - Osprey Publishing New Vanguard series (vehicle cutaway schematics)
//   - NATO STANAG 4569 Annex D (Crew Vulnerability)
//   - US Army FM 5-102 (Armored Vehicle Survivability)
//   - Held M., "Cookoff Thresholds for Stored Munitions"

// ── Vehicle & component types ──────────────────────────────────────────────────

/// Broad vehicle classification.  Each type has distinct internal layout,
/// armour density, and component vulnerability profiles.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VehicleType {
    /// Main Battle Tank — heavy armour, dense layout, well-protected components.
    MBT,
    /// Infantry Fighting Vehicle — medium armour, carries dismounts.
    IFV,
    /// Armoured Personnel Carrier — light-to-medium armour, troop transport.
    APC,
    /// Unarmoured cargo truck / utility vehicle.
    Truck,
    /// Rotary-wing aircraft (attack / transport helicopter).
    Helicopter,
    /// Light wheeled vehicle (SUV, jeep, technical).
    LightVehicle,
}

/// Vehicle hit zone — which face of the vehicle the projectile strikes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HitZone {
    Front,
    Side,
    Rear,
    Top,
    Bottom,
}

/// A specific vehicle component that can be damaged or destroyed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VehicleComponent {
    /// Engine / powerpack — mobility kill.
    Engine,
    /// Gearbox / final drive — mobility kill.
    Transmission,
    /// Fuel tank — fire / explosion hazard.
    FuelTank,
    /// Ready- or stowed ammunition — catastrophic cookoff.
    AmmoRack,
    /// Driver or gunner position (individual crew).
    DriverGunner,
    /// Crew compartment (passengers, dismounts, loaders).
    CrewCompartment,
    /// Radiator / cooling fan assembly — mobility degradation.
    Radiator,
    /// Drive train / suspension / roadwheels — mobility kill.
    DriveTrain,
    /// Stowage bins, external cargo — no direct kill potential.
    Stowage,
    /// Empty space — no critical component.
    Empty,
}

// ── Input / output structs ─────────────────────────────────────────────────────

/// All input parameters needed to evaluate component kill probabilities
/// for a single projectile impact on a vehicle.
#[derive(Debug, Clone)]
pub struct ComponentKillParams {
    /// Which type of vehicle was struck.
    pub vehicle_type: VehicleType,
    /// Which zone of the vehicle was hit.
    pub hit_zone: HitZone,
    /// Projectile calibre in millimetres.
    pub projectile_caliber_mm: f64,
    /// Projectile mass in grams.
    pub projectile_mass_g: f64,
    /// Velocity at the moment of impact (m/s).
    pub impact_velocity_ms: f64,
    /// Projectile construction type: "ball", "ap", "apds", "apfsds",
    /// "heat", "he", "incendiary", "api", "hei", "tracer", etc.
    pub projectile_type: &'static str,
    /// Angle of impact from surface normal (0 = perpendicular, degrees).
    pub impact_angle_deg: f64,
    /// Projectile velocity remaining after perforating the main armour (m/s).
    /// Set to `impact_velocity_ms` if the vehicle has no armour.
    pub residual_velocity_ms: f64,
    /// Residual kinetic energy after armour penetration (joules).
    /// Computed as `0.5 * mass_kg * residual_velocity_ms²`.
    pub energy_j: f64,
    /// Whether the main armour was perforated.
    pub armor_penetrated: bool,
}

/// Kill probabilities aggregated into standard military kill categories.
#[derive(Debug, Clone)]
pub struct ComponentKillResult {
    /// Per-component hit, penetration, and kill probabilities.
    pub hits: Vec<ComponentHitProbability>,
    /// Probability of mobility kill (engine, transmission, drive train, fuel).
    pub mobility_kill_probability: f64,
    /// Probability of firepower kill (ammo rack, crew incapacitation).
    pub firepower_kill_probability: f64,
    /// Probability of catastrophic kill (ammo cookoff → total loss).
    pub catastrophic_kill_probability: f64,
    /// Probability of crew kill (driver, gunner, crew compartment).
    pub crew_kill_probability: f64,
}

/// Detailed three-stage probabilities for a single vehicle component.
#[derive(Debug, Clone)]
pub struct ComponentHitProbability {
    /// Which component.
    pub component: VehicleComponent,
    /// P(hit) — probability the projectile hits this component given
    /// the hit zone (based on fractional area coverage).
    pub hit_probability: f64,
    /// P(penetrate_to_depth | hit) — probability the projectile reaches
    /// the component's depth with sufficient residual energy.
    pub penetration_probability: f64,
    /// P(kill | hit ∧ pen) — probability the component is killed given
    /// it is hit and reached.
    pub kill_given_hit_pen: f64,
    /// P(kill) = P(hit) × P(pen) × P(kill|hit,pen)
    pub combined_kill_probability: f64,
}

// ── Internal density model constants ───────────────────────────────────────────

/// Energy loss per metre of vehicle interior penetration (J/m).
///
/// Represents the average energy dissipated by internal structure,
/// bulkheads, and secondary armour as the projectile travels to reach
/// a component at a given depth.  Higher values reflect denser,
/// better-armoured vehicle interiors.
fn vehicle_density_energy_cost(vehicle: VehicleType) -> f64 {
    match vehicle {
        VehicleType::MBT => 30_000.0,         // armoured bulkheads, packed
        VehicleType::IFV => 15_000.0,         // some internal armour
        VehicleType::APC => 8_000.0,          // light internal partitions
        VehicleType::Truck => 1_000.0,        // thin sheet metal, empty voids
        VehicleType::Helicopter => 300.0,     // airframe skin, very little
        VehicleType::LightVehicle => 1_500.0, // unibody construction
    }
}

/// Scale factor applied to component vulnerability thresholds by vehicle
/// type.  Heavier vehicles house components behind more robust casings
/// and sub-assemblies, raising the effective kill threshold.
#[allow(dead_code)] // ponytail: test-only, wire when component kill linked to vehicle damage model
fn vehicle_kill_modifier(vehicle: VehicleType) -> f64 {
    match vehicle {
        VehicleType::MBT => 1.4,
        VehicleType::IFV => 1.15,
        VehicleType::APC => 0.9,
        VehicleType::Truck => 0.5,
        VehicleType::Helicopter => 0.4,
        VehicleType::LightVehicle => 0.6,
    }
}

// ── Component layout tables ────────────────────────────────────────────────────
//
// Each entry: (component, area_fraction, depth_m)
//
// Area fractions within a zone sum to 1.0 (Empty fills the remainder).
// Depth is the approximate straight-line distance from the outer skin
// to the component centroid.
//
// Layouts are synthesised from Jane's armour cutaway drawings, Osprey
// New Vanguard series schematics, and public-domain vehicle technical
// manuals.  They represent representative values; exact per-vehicle
// data would replace these in a game-integration layer.

/// Return the component layout for a given vehicle type and hit zone.
///
/// # Returns
/// A vector of `(component, area_fraction, depth_m)` tuples sorted by
/// descending area fraction.
pub fn component_layout(vehicle: VehicleType, zone: HitZone) -> Vec<(VehicleComponent, f64, f64)> {
    use HitZone::*;
    use VehicleComponent::*;
    use VehicleType::*;

    match (vehicle, zone) {
        // ── MBT ─────────────────────────────────────────────────────────────
        (MBT, Front) => vec![
            (Engine, 0.40, 2.0),
            (Transmission, 0.15, 1.5),
            (DriverGunner, 0.10, 1.2),
            (AmmoRack, 0.05, 2.5),
            (FuelTank, 0.05, 2.0),
            (Empty, 0.25, 3.0),
        ],
        (MBT, Side) => vec![
            (FuelTank, 0.20, 1.0),
            (AmmoRack, 0.15, 1.5),
            (CrewCompartment, 0.15, 0.8),
            (Engine, 0.15, 2.0),
            (Stowage, 0.35, 0.5),
        ],
        (MBT, Rear) => vec![
            (Engine, 0.30, 1.0),
            (FuelTank, 0.20, 0.8),
            (Transmission, 0.10, 1.5),
            (Empty, 0.40, 2.0),
        ],
        (MBT, Top) => vec![
            (CrewCompartment, 0.25, 0.5),
            (AmmoRack, 0.15, 0.8),
            (Engine, 0.10, 1.0),
            (Radiator, 0.20, 0.4),
            (Empty, 0.30, 1.5),
        ],
        (MBT, Bottom) => vec![
            (DriveTrain, 0.15, 0.3),
            (Engine, 0.10, 0.5),
            (Transmission, 0.10, 0.4),
            (FuelTank, 0.10, 0.3),
            (Empty, 0.55, 0.8),
        ],

        // ── IFV ─────────────────────────────────────────────────────────────
        (IFV, Front) => vec![
            (Engine, 0.35, 1.2),
            (AmmoRack, 0.15, 1.5),
            (Transmission, 0.10, 1.0),
            (DriverGunner, 0.10, 0.8),
            (Empty, 0.30, 2.0),
        ],
        (IFV, Side) => vec![
            (CrewCompartment, 0.25, 0.6),
            (AmmoRack, 0.20, 1.0),
            (FuelTank, 0.15, 0.5),
            (Engine, 0.10, 1.5),
            (Stowage, 0.30, 0.4),
        ],
        (IFV, Rear) => vec![
            (Engine, 0.25, 0.8),
            (CrewCompartment, 0.20, 0.6),
            (FuelTank, 0.15, 0.5),
            (Empty, 0.40, 1.5),
        ],
        (IFV, Top) => vec![
            (CrewCompartment, 0.30, 0.4),
            (AmmoRack, 0.15, 0.6),
            (Radiator, 0.15, 0.3),
            (Engine, 0.10, 0.8),
            (Empty, 0.30, 1.0),
        ],
        (IFV, Bottom) => vec![
            (DriveTrain, 0.20, 0.2),
            (FuelTank, 0.10, 0.2),
            (Empty, 0.70, 0.5),
        ],

        // ── APC ─────────────────────────────────────────────────────────────
        (APC, Front) => vec![
            (Engine, 0.35, 1.0),
            (Transmission, 0.10, 0.8),
            (DriverGunner, 0.10, 0.5),
            (FuelTank, 0.10, 0.8),
            (Empty, 0.35, 1.5),
        ],
        (APC, Side) => vec![
            (CrewCompartment, 0.25, 0.5),
            (FuelTank, 0.15, 0.4),
            (Engine, 0.10, 1.0),
            (Stowage, 0.25, 0.3),
            (Empty, 0.25, 1.0),
        ],
        (APC, Rear) => vec![
            (Engine, 0.25, 0.6),
            (CrewCompartment, 0.20, 0.5),
            (FuelTank, 0.10, 0.4),
            (Empty, 0.45, 1.2),
        ],
        (APC, Top) => vec![
            (CrewCompartment, 0.30, 0.3),
            (Radiator, 0.15, 0.2),
            (Engine, 0.10, 0.5),
            (Empty, 0.45, 0.8),
        ],
        (APC, Bottom) => vec![
            (DriveTrain, 0.15, 0.2),
            (FuelTank, 0.10, 0.2),
            (Empty, 0.75, 0.4),
        ],

        // ── Truck ───────────────────────────────────────────────────────────
        (Truck, Front) => vec![
            (Engine, 0.30, 0.5),
            (DriverGunner, 0.10, 0.3),
            (FuelTank, 0.10, 0.4),
            (Empty, 0.50, 1.0),
        ],
        (Truck, Side) => vec![
            (CrewCompartment, 0.15, 0.3),
            (FuelTank, 0.10, 0.2),
            (Stowage, 0.30, 0.3),
            (Empty, 0.45, 0.8),
        ],
        (Truck, Rear) => vec![
            (Stowage, 0.40, 0.4),
            (FuelTank, 0.10, 0.2),
            (Engine, 0.10, 0.3),
            (Empty, 0.40, 0.8),
        ],
        (Truck, Top) => vec![
            (CrewCompartment, 0.15, 0.2),
            (Stowage, 0.30, 0.3),
            (Empty, 0.55, 0.6),
        ],
        (Truck, Bottom) => vec![
            (DriveTrain, 0.20, 0.2),
            (FuelTank, 0.10, 0.15),
            (Empty, 0.70, 0.3),
        ],

        // ── Helicopter ──────────────────────────────────────────────────────
        (Helicopter, Front) => vec![
            (CrewCompartment, 0.30, 0.2),
            (Engine, 0.20, 0.5),
            (Transmission, 0.15, 0.4),
            (Empty, 0.35, 0.8),
        ],
        (Helicopter, Side) => vec![
            (CrewCompartment, 0.25, 0.15),
            (FuelTank, 0.20, 0.3),
            (Engine, 0.10, 0.4),
            (Stowage, 0.20, 0.2),
            (Empty, 0.25, 0.6),
        ],
        (Helicopter, Rear) => vec![
            (Transmission, 0.20, 0.3),
            (Engine, 0.15, 0.3),
            (FuelTank, 0.15, 0.2),
            (Empty, 0.50, 0.6),
        ],
        (Helicopter, Top) => vec![
            (Transmission, 0.25, 0.1),
            (Engine, 0.20, 0.15),
            (FuelTank, 0.10, 0.1),
            (Empty, 0.45, 0.3),
        ],
        (Helicopter, Bottom) => vec![
            (CrewCompartment, 0.15, 0.1),
            (FuelTank, 0.10, 0.15),
            (Stowage, 0.20, 0.1),
            (Empty, 0.55, 0.3),
        ],

        // ── LightVehicle ────────────────────────────────────────────────────
        (LightVehicle, Front) => vec![
            (Engine, 0.35, 0.4),
            (DriverGunner, 0.15, 0.3),
            (FuelTank, 0.10, 0.3),
            (Empty, 0.40, 0.8),
        ],
        (LightVehicle, Side) => vec![
            (CrewCompartment, 0.20, 0.2),
            (FuelTank, 0.10, 0.15),
            (Stowage, 0.30, 0.2),
            (Empty, 0.40, 0.6),
        ],
        (LightVehicle, Rear) => vec![
            (FuelTank, 0.10, 0.2),
            (Stowage, 0.35, 0.3),
            (Empty, 0.55, 0.6),
        ],
        (LightVehicle, Top) => vec![
            (CrewCompartment, 0.15, 0.1),
            (Stowage, 0.20, 0.15),
            (Empty, 0.65, 0.4),
        ],
        (LightVehicle, Bottom) => vec![
            (DriveTrain, 0.20, 0.15),
            (FuelTank, 0.10, 0.1),
            (Empty, 0.70, 0.25),
        ],
    }
}

// ── Vulnerability thresholds ──────────────────────────────────────────────────

/// Base vulnerability threshold (joules) — the energy level at which
/// P(kill|hit,pen) = 0.5 for this component.
///
/// Derived from BRL CR 341 critical energy criteria, adjusted for
/// behind-armour debris and component sensitivity.
fn component_vulnerability_threshold_j(component: VehicleComponent) -> f64 {
    use VehicleComponent::*;
    match component {
        AmmoRack => 5_000.0,        // cookoff threshold
        FuelTank => 12_000.0,       // fuel ignition / explosion
        CrewCompartment => 8_000.0, // spall vulnerability
        DriverGunner => 8_000.0,    // same as crew
        Radiator => 15_000.0,       // fragile fins and coolant
        Engine => 50_000.0,         // heavy block
        Transmission => 100_000.0,  // robust gear cluster
        DriveTrain => 120_000.0,    // suspension / roadwheels
        Stowage => 500_000.0,       // external cargo, hard to "kill"
        Empty => 1_000_000.0,       // no component to kill
    }
}

// ── Projectile-type modifiers ─────────────────────────────────────────────────

/// Multiplier applied to residual energy for kill computation.
///
/// Rounds with enhanced behind-armour effects (HEAT, HE, HEI) or
/// incendiary content have higher effective energy for component
/// damage than their kinetic energy alone suggests.
fn projectile_kill_modifier(proj_type: &str) -> f64 {
    match proj_type.to_lowercase().as_str() {
        "ball" | "fmj" => 1.0,
        "ap" | "armor_piercing" => 1.0,
        "apds" => 0.9,
        "apfsds" => 0.85,
        "apcr" => 0.95,
        "heat" => 1.8,
        "he" => 2.0,
        "incendiary" | "i" => 1.3,
        "api" | "ap_i" => 1.4,
        "hei" | "he_i" => 2.2,
        "hei_t" | "he_i_t" => 2.2,
        "sap_hei" => 1.9,
        "tracer" | "t" => 1.1,
        _ => 1.0,
    }
}

// ── Core evaluation ───────────────────────────────────────────────────────────

/// Logistic sigmoid centred at `threshold` with steepness `k`.
///
///   P = 1 / (1 + exp(-k · (E / threshold - 1)))
///
/// Returns 0.0–1.0 probability.  At E = threshold, P = 0.5.
fn logistic_kill_probability(energy_j: f64, threshold_j: f64, k: f64) -> f64 {
    if threshold_j <= 0.0 || energy_j <= 0.0 {
        return 0.0;
    }
    let z = k * (energy_j / threshold_j - 1.0);
    // Numerically stable logistic
    let p = if z >= 0.0 {
        1.0 / (1.0 + (-z).exp())
    } else {
        let ez = z.exp();
        ez / (1.0 + ez)
    };
    p.clamp(0.0, 1.0)
}

/// Compute probability that the projectile penetrates to a given depth
/// inside the vehicle after armour perforation.
///
/// The projectile's residual energy is consumed by internal structure.
/// The probability is the fraction of the energy required to reach the
/// component that the projectile still possesses, clamped to [0, 1].
///
/// If the vehicle armour was not perforated, the probability is 0
/// regardless of residual energy (the projectile did not enter).
fn penetration_probability_to_depth(
    residual_energy_j: f64,
    depth_m: f64,
    vehicle: VehicleType,
    armor_penetrated: bool,
) -> f64 {
    if !armor_penetrated || depth_m <= 0.0 {
        return 0.0;
    }
    let cost_per_m = vehicle_density_energy_cost(vehicle);
    let required_j = cost_per_m * depth_m;
    if required_j <= 0.0 {
        return 1.0;
    }
    (residual_energy_j / required_j).clamp(0.0, 1.0)
}

/// Compute the probability of killing a specific vehicle component given
/// that the projectile has hit it and has residual energy available at
/// that depth.
///
/// # Arguments
/// * `component` — The vehicle component.
/// * `residual_energy_j` — Kinetic energy remaining after armour
///   penetration and after travelling to the component depth (J).
/// * `caliber_mm` — Projectile calibre (mm) — larger calibre transfers
///   energy more efficiently.
/// * `projectile_type` — Construction type string.
pub fn component_kill_given_hit(
    component: VehicleComponent,
    residual_energy_j: f64,
    caliber_mm: f64,
    projectile_type: &str,
) -> f64 {
    if residual_energy_j <= 0.0 {
        return 0.0;
    }

    let base_threshold = component_vulnerability_threshold_j(component);
    let proj_mod = projectile_kill_modifier(projectile_type);

    // Calibre bonus: larger rounds transfer energy more efficiently
    // to internal components.  A 7.62 mm round is baseline (1.0);
    // 30 mm rounds get ~1.4×, 120 mm get ~1.8×.
    let cal_bonus = 1.0 + 0.25 * (caliber_mm / 7.62 - 1.0).max(0.0).sqrt();
    let cal_bonus = cal_bonus.min(2.5);

    let effective_energy = residual_energy_j * proj_mod * cal_bonus;

    // Steepness: k=4 gives a reasonably sharp transition around the
    // threshold — 12% at 0.5×, 50% at 1.0×, 88% at 1.5×, 98% at 2.0×.
    logistic_kill_probability(effective_energy, base_threshold, 4.0)
}

/// Compute the complete component kill probability matrix for a single
/// projectile impact on a vehicle.
///
/// The model combines:
///   1. Geometric hit probability (area coverage within zone)
///   2. Penetration to depth (residual energy vs. internal density)
///   3. Component kill given hit and penetration (vulnerability threshold)
///
/// # Arguments
/// * `params` — Impact and projectile parameters.
///
/// # Returns
/// `ComponentKillResult` with per-component probabilities and aggregated
/// kill category probabilities.
pub fn evaluate_component_kill_probability(params: &ComponentKillParams) -> ComponentKillResult {
    let layout = component_layout(params.vehicle_type, params.hit_zone);

    let mut hits: Vec<ComponentHitProbability> = Vec::with_capacity(layout.len());

    for &(component, area_fraction, depth_m) in &layout {
        // Stage 1: P(hit)
        let hit_prob = area_fraction;

        // Stage 2: P(penetrate_to_depth)
        let pen_prob = penetration_probability_to_depth(
            params.energy_j,
            depth_m,
            params.vehicle_type,
            params.armor_penetrated,
        );

        // Residual energy after travelling to depth:
        // E_remaining = E_residual - vehicle_density_cost * depth
        let density_cost = vehicle_density_energy_cost(params.vehicle_type);
        let energy_at_depth = (params.energy_j - density_cost * depth_m).max(0.0);

        // Stage 3: P(kill | hit, pen)
        let kill_given = if pen_prob > 0.0 {
            component_kill_given_hit(
                component,
                energy_at_depth,
                params.projectile_caliber_mm,
                params.projectile_type,
            )
        } else {
            0.0
        };

        let combined = hit_prob * pen_prob * kill_given;

        hits.push(ComponentHitProbability {
            component,
            hit_probability: hit_prob,
            penetration_probability: pen_prob,
            kill_given_hit_pen: kill_given,
            combined_kill_probability: combined,
        });
    }

    // ── Aggregate kill category probabilities ───────────────────────────
    // Use the "union" formula: P = 1 - ∏(1 - P_i) for independent events.
    // This is conservative (assumes independence) and avoids double-counting.

    let mob_components = [
        VehicleComponent::Engine,
        VehicleComponent::Transmission,
        VehicleComponent::DriveTrain,
        VehicleComponent::FuelTank,
    ];
    let fp_components = [VehicleComponent::AmmoRack, VehicleComponent::DriverGunner];
    let cat_components = [VehicleComponent::AmmoRack];
    let crew_components = [
        VehicleComponent::CrewCompartment,
        VehicleComponent::DriverGunner,
    ];

    let mobility_kill = union_probability(&hits, &mob_components);
    let firepower_kill = union_probability(&hits, &fp_components);
    let catastrophic_kill = union_probability(&hits, &cat_components);
    let crew_kill = union_probability(&hits, &crew_components);

    ComponentKillResult {
        hits,
        mobility_kill_probability: mobility_kill,
        firepower_kill_probability: firepower_kill,
        catastrophic_kill_probability: catastrophic_kill,
        crew_kill_probability: crew_kill,
    }
}

/// Compute the union probability P = 1 - ∏(1 - p_i) for the given
/// component subset.
fn union_probability(hits: &[ComponentHitProbability], components: &[VehicleComponent]) -> f64 {
    let mut p_not = 1.0;
    for comp in components {
        if let Some(h) = hits.iter().find(|h| h.component == *comp) {
            p_not *= (1.0 - h.combined_kill_probability).max(0.0);
        }
    }
    (1.0 - p_not).clamp(0.0, 1.0)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Helper: quick params builder ─────────────────────────────────────────

    /// Build a `ComponentKillParams` from the minimum required fields.
    fn params(
        vtype: VehicleType,
        zone: HitZone,
        cal_mm: f64,
        mass_g: f64,
        vel_ms: f64,
        proj_type: &'static str,
        res_vel_ms: f64,
        pen: bool,
    ) -> ComponentKillParams {
        let mass_kg = mass_g / 1000.0;
        let energy_j = 0.5 * mass_kg * res_vel_ms.powi(2);
        ComponentKillParams {
            vehicle_type: vtype,
            hit_zone: zone,
            projectile_caliber_mm: cal_mm,
            projectile_mass_g: mass_g,
            impact_velocity_ms: vel_ms,
            projectile_type: proj_type,
            impact_angle_deg: 0.0,
            residual_velocity_ms: res_vel_ms,
            energy_j,
            armor_penetrated: pen,
        }
    }

    // ── 1. MBT frontal, engine kill probability ──────────────────────────────
    //
    // 120 mm APFSDS at 1 650 m/s (~6.5 MJ) vs MBT front.
    // After penetrating ~600 mm RHA at 0°, residual ≈ 900 m/s → ~1.9 MJ.
    // Engine is at 2.0 m depth, area fraction 0.40.
    // Expected: high combined probability (~0.8–1.0).

    #[test]
    fn mbt_frontal_engine_kill_apfsds() {
        let p = params(
            VehicleType::MBT,
            HitZone::Front,
            120.0,
            4_500.0, // 4.5 kg APFSDS rod
            1_650.0, // impact velocity
            "apfsds",
            900.0, // residual after heavy armour
            true,  // armour perforated
        );
        // 0.5 * 4.5 * 900² = 1 822 500 J
        let r = evaluate_component_kill_probability(&p);

        // Engine occupies 40 % of frontal area
        let engine_hit = r
            .hits
            .iter()
            .find(|h| h.component == VehicleComponent::Engine)
            .expect("engine should be in MBT frontal layout");

        assert!(
            engine_hit.hit_probability >= 0.39 && engine_hit.hit_probability <= 0.41,
            "engine hit prob should be ~0.40 (area fraction), got {:.4}",
            engine_hit.hit_probability
        );
        assert!(
            engine_hit.penetration_probability > 0.5,
            "engine pen prob should be high with 1.8 MJ residual, got {:.4}",
            engine_hit.penetration_probability
        );
        assert!(
            engine_hit.combined_kill_probability > 0.3,
            "engine combined kill should be > 0.3 with large APFSDS, got {:.4}",
            engine_hit.combined_kill_probability
        );
    }

    // ── 2. IFV side, ammo rack hit / catastrophic kill ───────────────────────
    //
    // 30 mm AP (BMP-2 / Bradley chain gun) at 1 100 m/s, side hit.
    // 30 mm AP penetrates IFV side armour (~30 mm aluminium) easily.
    // Ammo rack at 1.0 m depth, area 0.20.

    #[test]
    fn ifv_side_ammo_rack_kill() {
        let p = params(
            VehicleType::IFV,
            HitZone::Side,
            30.0,
            350.0, // 350 g projectile
            1_100.0,
            "ap",
            950.0, // residual after light side armour
            true,
        );
        // 0.5 * 0.35 * 950² ≈ 158 kJ
        let r = evaluate_component_kill_probability(&p);

        let ammo = r
            .hits
            .iter()
            .find(|h| h.component == VehicleComponent::AmmoRack)
            .expect("ammo rack should be in IFV side layout");

        assert!(
            ammo.hit_probability >= 0.19 && ammo.hit_probability <= 0.21,
            "ammo rack area fraction ~0.20, got {:.4}",
            ammo.hit_probability
        );
        // 158 kJ is far above the 5 kJ ammo threshold, so P(kill|hit,pen) should be ~1.0.
        assert!(
            ammo.kill_given_hit_pen > 0.95,
            "ammo kill given hit should be near 1.0 at 158 kJ, got {:.4}",
            ammo.kill_given_hit_pen
        );
        // Combined P = 0.20 × P(pen) × ~1.0.
        // 158 kJ residual, IFV density cost 15 kJ/m, depth 1.0 m → P(pen) ≈ 158/15 ≈ 10.5 → 1.0.
        assert!(
            ammo.combined_kill_probability > 0.15,
            "ammo combined kill should be > 0.15, got {:.4}",
            ammo.combined_kill_probability
        );
        // Catastrophic kill probability should be non-trivial.
        assert!(
            r.catastrophic_kill_probability > 0.1,
            "catastrophic kill prob should be > 0.1 for IFV side ammo hit, got {:.4}",
            r.catastrophic_kill_probability
        );
    }

    // ── 3. Truck, no armour → high kill probabilities ────────────────────────
    //
    // 7.62×39 mm ball at 720 m/s (~2 kJ).  Truck has no armour,
    // so armor_penetrated = true at full velocity.
    // Components at shallow depths → most components reachable.
    // Combined probabilities should be moderate but non-zero across
    // the board.

    #[test]
    fn truck_no_armour_high_kill_probs() {
        let p = params(
            VehicleType::Truck,
            HitZone::Side,
            7.62,
            8.0, // 8 g
            720.0,
            "ball",
            720.0, // no armour → full velocity retained
            true,  // "perforated" (no armour to stop it)
        );
        // 0.5 * 0.008 * 720² = 2 074 J
        let r = evaluate_component_kill_probability(&p);

        // Fuel tank: 0.10 area, 0.2 m depth.
        // Density cost = 1 kJ/m, so 2074 J easily reaches 0.2 m.
        let fuel = r
            .hits
            .iter()
            .find(|h| h.component == VehicleComponent::FuelTank)
            .expect("fuel tank in truck side layout");
        assert!(
            fuel.penetration_probability > 0.9,
            "fuel tank pen prob should be near 1.0 at 2 kJ in a truck, got {:.4}",
            fuel.penetration_probability
        );
        // Crew compartment: 0.15 area, 0.3 m depth.
        // Truck density 1 kJ/m → energy_at_depth = 2074 - 1000*0.3 = 1774 J.
        // calibre 7.62 mm → cal_bonus = 1.0, ball modifier = 1.0.
        // effective = 1774 J.  Crew threshold 8 kJ.
        // logistic(1774/8000) ≈ 0.043.
        let crew = r
            .hits
            .iter()
            .find(|h| h.component == VehicleComponent::CrewCompartment)
            .expect("crew compartment in truck side layout");
        assert!(
            crew.kill_given_hit_pen > 0.02 && crew.kill_given_hit_pen < 0.20,
            "crew kill given hit at 1.8 kJ effective vs 8 kJ threshold should be ~0.043, got {:.4}",
            crew.kill_given_hit_pen
        );

        // Mobility kill from fuel tank (area 0.10, depth 0.2 m).
        // Truck side has no engine/transmission/drive train in its layout,
        // so only fuel contributes to mobility kill.
        // energy_at_depth = 2074 - 1000*0.2 = 1874 J, effective = 1874 J (ball, 7.62 mm).
        // logistic(1874/12000) ≈ 0.033 → combined = 0.10 * 1.0 * 0.033 = 0.0033.
        // This is low because a 7.62 mm ball has limited energy to kill a fuel tank.
        assert!(
            r.mobility_kill_probability > 0.001 && r.mobility_kill_probability < 0.02,
            "mobility kill prob should be ~0.0033 for 7.62 mm ball on truck side, got {:.4}",
            r.mobility_kill_probability
        );
    }

    // ── 4. Crew kill from spall (side hit, MBT) ──────────────────────────────
    //
    // RPG-7 (HEAT) hitting MBT side.  Even if it doesn't fully penetrate
    // the main armour, the shaped charge jet produces copious behind-armour
    // debris.  Crew compartment at 0.8 m depth, area 0.15.
    //
    // We model the HEAT jet's residual as high behind-armour effect
    // (projectile_kill_modifier = 1.8).  The crew vulnerability threshold
    // is 8 kJ — moderate spall easily exceeds this.

    #[test]
    fn mbt_side_crew_kill_from_spall() {
        let p = params(
            VehicleType::MBT,
            HitZone::Side,
            85.0,    // RPG-7 warhead diameter
            2_500.0, // 2.5 kg projectile
            300.0,   // relatively slow
            "heat",
            150.0, // residual jet velocity (converted)
            true,  // armour perforated by HEAT jet
        );
        // 0.5 * 2.5 * 150² = 28 125 J
        let r = evaluate_component_kill_probability(&p);

        let crew = r
            .hits
            .iter()
            .find(|h| h.component == VehicleComponent::CrewCompartment)
            .expect("crew compartment in MBT side layout");

        // Crew at 0.8 m depth, MBT density cost 30 kJ/m.
        // P(pen) to 0.8 m = 28125 / (30000 * 0.8) = 1.17 → capped at 1.0.
        assert!(
            crew.penetration_probability > 0.9,
            "crew pen prob should be near 1.0, got {:.4}",
            crew.penetration_probability
        );

        // HEAT has kill modifier 1.8, calibre 85 mm gives cal_bonus
        // = 1 + 0.25 * sqrt(85/7.62 - 1) = 1 + 0.25 * sqrt(10.15) = 1 + 0.796 = 1.796
        // effective_energy ≈ 28125 * 1.8 * 1.796 ≈ 90 900 J
        // Compared to 8 kJ threshold → very high kill prob.
        assert!(
            crew.kill_given_hit_pen > 0.8,
            "crew kill given hit should be high from HEAT spall, got {:.4}",
            crew.kill_given_hit_pen
        );

        // Crew kill aggregate should be significant.
        assert!(
            r.crew_kill_probability > 0.1,
            "crew kill prob should be > 0.1 for MBT side HEAT hit, got {:.4}",
            r.crew_kill_probability
        );
    }

    // ── 5. Light vehicle, small calibre → low probabilities ───────────────────
    //
    // 5.56 mm ball at 900 m/s hitting a LightVehicle side.
    // Energy = 0.5 * 0.004 * 900² = 1 620 J.
    // LightVehicle density = 1.5 kJ/m.  Crew at 0.2 m: P(pen) ≈ 1.0.
    // But 1.6 kJ is well below crew kill threshold of 8 kJ → low P(kill).

    #[test]
    fn light_vehicle_small_calibre_low_kill() {
        let p = params(
            VehicleType::LightVehicle,
            HitZone::Side,
            5.56,
            4.0,
            900.0,
            "ball",
            900.0,
            true,
        );
        let r = evaluate_component_kill_probability(&p);

        let crew = r
            .hits
            .iter()
            .find(|h| h.component == VehicleComponent::CrewCompartment)
            .expect("crew compartment in light vehicle side layout");

        // 1620 J reaches 0.2 m easily (cost = 1.5 kJ/m × 0.2 = 300 J)
        assert!(
            crew.penetration_probability > 0.9,
            "pen prob should be high at shallow depth"
        );
        // 1620 J vs 8 kJ threshold → logistic(1620/8000) ≈ 0.055
        assert!(
            crew.kill_given_hit_pen < 0.20,
            "crew kill given hit should be low with 1.6 kJ vs 8 kJ threshold, got {:.4}",
            crew.kill_given_hit_pen
        );
        // Combined = 0.20 * 1.0 * ~0.055 ≈ 0.011
        assert!(
            crew.combined_kill_probability < 0.10,
            "combined crew kill should be < 0.10, got {:.4}",
            crew.combined_kill_probability
        );
    }

    // ── 6. Helicopter top attack, transmission kill ───────────────────────────
    //
    // 12.7 mm API from above at 890 m/s.  Helicopter top has
    // transmission at 0.1 m, area 0.25.
    // Energy = 0.5 * 0.042 * 890² = 16 636 J.
    // Helo density = 0.3 kJ/m → P(pen) to 0.1 m ≈ 1.0.
    // Transmission threshold = 100 kJ → well below → low kill prob.

    #[test]
    fn helicopter_top_transmission_low_kill() {
        let p = params(
            VehicleType::Helicopter,
            HitZone::Top,
            12.7,
            42.0,
            890.0,
            "api",
            890.0,
            true,
        );
        let r = evaluate_component_kill_probability(&p);

        let trans = r
            .hits
            .iter()
            .find(|h| h.component == VehicleComponent::Transmission)
            .expect("transmission in helo top layout");

        assert!(
            trans.penetration_probability > 0.9,
            "helo transmission pen should be near 1.0 at 0.1 m depth"
        );
        // 16.6 kJ vs 100 kJ threshold → logistic(16.6/100) ≈ 0.026
        assert!(
            trans.kill_given_hit_pen < 0.15,
            "transmission kill given hit should be low, got {:.4}",
            trans.kill_given_hit_pen
        );
    }

    // ── 7. No armour penetration → all probabilities zero ────────────────────
    //
    // If the projectile fails to perforate the armour, P(pen) = 0 for
    // every component regardless of depth.

    #[test]
    fn no_penetration_all_probs_zero() {
        let p = params(
            VehicleType::MBT,
            HitZone::Front,
            7.62,
            9.5,
            800.0,
            "ball",
            0.0,   // no residual — round stopped
            false, // armour NOT perforated
        );
        let r = evaluate_component_kill_probability(&p);

        for h in &r.hits {
            assert_eq!(
                h.penetration_probability, 0.0,
                "{:?}: pen prob should be 0 when armour not perforated",
                h.component
            );
            assert_eq!(
                h.combined_kill_probability, 0.0,
                "{:?}: combined kill should be 0",
                h.component
            );
        }
        assert_eq!(r.mobility_kill_probability, 0.0);
        assert_eq!(r.firepower_kill_probability, 0.0);
        assert_eq!(r.catastrophic_kill_probability, 0.0);
        assert_eq!(r.crew_kill_probability, 0.0);
    }

    // ── 8. APC rear, engine kill, medium calibre ─────────────────────────────
    //
    // 14.5 mm AP (KPVT) at 1 000 m/s vs APC rear.
    // Res ~800 m/s after light rear armour → 0.5 * 0.064 * 800² = 20 480 J.
    // Engine at 0.6 m, area 0.25.  APC density 8 kJ/m.
    // P(pen) to 0.6 m = 20480 / (8000 * 0.6) = 4.27 → 1.0.
    // Engine threshold 50 kJ → 20.5 kJ vs 50 kJ → logistic(0.41) ≈ 0.20.

    #[test]
    fn apc_rear_engine_kill_medium_calibre() {
        let p = params(
            VehicleType::APC,
            HitZone::Rear,
            14.5,
            64.0, // 64 g
            1_000.0,
            "ap",
            800.0, // residual after light armour
            true,
        );
        let r = evaluate_component_kill_probability(&p);

        let engine = r
            .hits
            .iter()
            .find(|h| h.component == VehicleComponent::Engine)
            .expect("engine in APC rear layout");

        assert!(
            engine.hit_probability >= 0.24 && engine.hit_probability <= 0.26,
            "engine area fraction ~0.25, got {:.4}",
            engine.hit_probability
        );
        assert!(
            engine.penetration_probability > 0.9,
            "engine pen prob should be near 1.0, got {:.4}",
            engine.penetration_probability
        );
        // 20.5 kJ vs 50 kJ threshold, AP modifier 1.0, cal 14.5 mm bonus ≈ 1.24.
        // energy_at_depth = 20480 - 8000*0.6 = 15680 J.
        // effective = 15680 * 1.0 * 1.24 ≈ 19 412 J.
        // logistic(19412/50000) ≈ 0.080.
        assert!(
            engine.kill_given_hit_pen > 0.04 && engine.kill_given_hit_pen < 0.20,
            "engine kill given hit at ~19.4 kJ effective vs 50 kJ threshold should be ~0.080, got {:.4}",
            engine.kill_given_hit_pen
        );
        // Combined = 0.25 * 1.0 * ~0.080 ≈ 0.020
        assert!(
            engine.combined_kill_probability > 0.005 && engine.combined_kill_probability < 0.10,
            "engine combined kill should be in a sensible range, got {:.4}",
            engine.combined_kill_probability
        );
    }

    // ── 9. LightVehicle frontal, driver kill from small arms ──────────────────
    //
    // 7.62×51 mm NATO ball at 850 m/s, LightVehicle front.
    // Energy = 0.5 * 0.0095 * 850² = 3 432 J.  No armour.
    // Driver at 0.3 m, area 0.15.  Density = 1.5 kJ/m.
    // P(pen) ≈ 1.0.  Driver threshold 8 kJ → 3.4 kJ → marginal.

    #[test]
    fn light_vehicle_frontal_driver_kill() {
        let p = params(
            VehicleType::LightVehicle,
            HitZone::Front,
            7.62,
            9.5,
            850.0,
            "ball",
            850.0,
            true,
        );
        let r = evaluate_component_kill_probability(&p);

        let driver = r
            .hits
            .iter()
            .find(|h| h.component == VehicleComponent::DriverGunner)
            .expect("driver in light vehicle front layout");

        assert!(
            driver.hit_probability >= 0.14 && driver.hit_probability <= 0.16,
            "driver area ~0.15, got {:.4}",
            driver.hit_probability
        );
        // 3.4 kJ vs 8 kJ threshold → ~15 % kill given hit.
        assert!(
            driver.kill_given_hit_pen > 0.05 && driver.kill_given_hit_pen < 0.5,
            "driver kill given hit should be marginal (~0.15), got {:.4}",
            driver.kill_given_hit_pen
        );
    }

    // ── 10. Determinism ──────────────────────────────────────────────────────
    //
    // Same input → identical output every time.

    #[test]
    fn evaluate_is_deterministic() {
        let p = params(
            VehicleType::MBT,
            HitZone::Front,
            120.0,
            4_500.0,
            1_650.0,
            "apfsds",
            900.0,
            true,
        );
        let a = evaluate_component_kill_probability(&p);
        let b = evaluate_component_kill_probability(&p);

        assert_eq!(a.hits.len(), b.hits.len());
        for (ha, hb) in a.hits.iter().zip(b.hits.iter()) {
            assert_eq!(ha.component, hb.component);
            assert!((ha.combined_kill_probability - hb.combined_kill_probability).abs() < 1e-12);
        }
        assert!((a.mobility_kill_probability - b.mobility_kill_probability).abs() < 1e-12);
        assert!((a.firepower_kill_probability - b.firepower_kill_probability).abs() < 1e-12);
        assert!((a.catastrophic_kill_probability - b.catastrophic_kill_probability).abs() < 1e-12);
        assert!((a.crew_kill_probability - b.crew_kill_probability).abs() < 1e-12);
    }

    // ── 11. HEAT round: high projectile kill modifier ────────────────────────
    //
    // 84 mm Carl-Gustaf HEAT vs MBT side.  HEAT has kill modifier 1.8.
    // Fuel tank at 1.0 m depth, area 0.20.

    #[test]
    fn heat_high_modifier_increases_kill_prob() {
        let p = params(
            VehicleType::MBT,
            HitZone::Side,
            84.0,
            1_700.0, // 1.7 kg
            290.0,   // slow
            "heat",
            200.0, // HEAT jet residual (converted)
            true,
        );
        // 0.5 * 1.7 * 200² = 34 000 J
        let r = evaluate_component_kill_probability(&p);

        let fuel = r
            .hits
            .iter()
            .find(|h| h.component == VehicleComponent::FuelTank)
            .expect("fuel tank in MBT side layout");

        // HEAT modifier = 1.8, calibre 84 mm bonus ≈ 1.79.
        // MBT density cost = 30 kJ/m, fuel at 1.0 m → energy_at_depth = 34000 - 30000 = 4000 J.
        // effective ≈ 4000 * 1.8 * 1.79 ≈ 12 895 J.
        // vs fuel threshold 12 kJ → logistic(12895/12000) ≈ 0.57.
        assert!(
            fuel.kill_given_hit_pen > 0.4 && fuel.kill_given_hit_pen < 0.8,
            "fuel kill given hit with HEAT (effective ~12.9 kJ vs 12 kJ threshold) should be ~0.57, got {:.4}",
            fuel.kill_given_hit_pen
        );
    }

    // ── 12. APC bottom mine blast ────────────────────────────────────────────
    //
    // Simulates a mine/IED blast under an APC.  High energy but directed
    // upward.  Drive train at 0.2 m (area 0.15) takes the brunt.

    #[test]
    fn apc_bottom_drive_train_kill() {
        let p = params(
            VehicleType::APC,
            HitZone::Bottom,
            100.0, // large "effective calibre" for blast
            500.0, // 500 g equivalent
            800.0,
            "he",  // HE blast
            600.0, // residual blast energy
            true,
        );
        // 0.5 * 0.5 * 600² = 90 000 J
        let r = evaluate_component_kill_probability(&p);

        let dt = r
            .hits
            .iter()
            .find(|h| h.component == VehicleComponent::DriveTrain)
            .expect("drive train in APC bottom layout");

        assert!(
            dt.hit_probability >= 0.14 && dt.hit_probability <= 0.16,
            "drive train area fraction ~0.15, got {:.4}",
            dt.hit_probability
        );
        // 90 kJ at 0.2 m depth, APC density 8 kJ/m → energy_at_depth = 90000 - 8000*0.2 = 88400 J.
        // HE modifier = 2.0, calibre 100 mm bonus ≈ 1.87.
        // effective ≈ 88400 * 2.0 * 1.87 ≈ 330 616 J.
        // Drive train threshold 120 kJ → 330 kJ >> 120 kJ → nearly certain kill.
        assert!(
            dt.kill_given_hit_pen > 0.9,
            "drive train kill given hit should be near 1.0 with HE blast (effective ~331 kJ vs 120 kJ threshold), got {:.4}",
            dt.kill_given_hit_pen
        );
        // Combined = 0.15 * ~1.0 * ~1.0 ≈ 0.15
        assert!(
            dt.combined_kill_probability > 0.05 && dt.combined_kill_probability < 0.30,
            "drive train combined kill should be ~0.15, got {:.4}",
            dt.combined_kill_probability
        );
        // Mobility kill should reflect drive train + fuel damage.
        assert!(
            r.mobility_kill_probability > 0.01,
            "mobility kill prob should be > 0 for bottom blast, got {:.4}",
            r.mobility_kill_probability
        );
    }
}
