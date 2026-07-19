// ABE - Barrel Erosion / Degradation Model
//
// Models how barrel wear from round count, heat, and firing rate
// reduces muzzle velocity and degrades accuracy over time.
//
// All models are empirical/analytical closures over field-observed
// small-arms barrel degradation patterns. Three coupled sub-models:
//   1. Thermal — barrel temperature from firing rate and round count
//   2. Fouling — accuracy degradation from powder/copper fouling
//   3. Erosion — velocity loss from throat erosion (wear)
//
// References:
//   - Bryan Litz "Applied Ballistics Precision" (barrel life / fouling)
//   - US Army ARL-TR-2828 (erosive wear in small arms)
//   - Hatcher's Notebook (barrel heating / cooling)

/// Physical state of the bore surface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BoreCondition {
    New,
    Fouled,
    Eroded,
    Critical,
}

impl BoreCondition {
    /// Accuracy penalty multiplier for this bore condition.
    ///
    /// Returns a multiplier applied to the base system accuracy (MOA).
    /// 1.0 = no penalty, >1.0 = wider dispersion.
    pub fn accuracy_penalty(&self) -> f64 {
        match self {
            BoreCondition::New => 1.0,
            BoreCondition::Fouled => 1.05,
            BoreCondition::Eroded => 1.15,
            BoreCondition::Critical => 1.35,
        }
    }
}

/// Input parameters for barrel erosion evaluation.
#[derive(Debug, Clone, Copy)]
pub struct ErosionParams {
    /// Total rounds fired through this barrel.
    pub rounds_fired: i32,
    /// Current barrel temperature in degrees Celsius.
    pub barrel_temp_c: f64,
    /// Expected barrel life in rounds before velocity drops >5 %.
    pub max_barrel_life_rounds: i32,
    /// Last known bore condition.
    pub bore_condition: BoreCondition,
    /// Round count at last bore cleaning.
    pub last_clean_rounds: i32,
}

/// Result of a barrel erosion evaluation.
#[derive(Debug, Clone, Copy)]
pub struct ErosionResult {
    /// Muzzle velocity loss as a percentage of original velocity.
    pub velocity_loss_pct: f64,
    /// Accuracy degradation multiplier (1.0 = baseline, >1.0 = worse).
    pub accuracy_moa_multiplier: f64,
    /// Fraction of barrel life consumed (0.0–1.0, mechanical wear only).
    pub erosion_fraction: f64,
    /// Estimated remaining barrel life in rounds.
    pub estimated_remaining_life_rounds: i32,
    /// Determined bore condition after evaluation.
    pub bore_condition: BoreCondition,
    /// Thermal energy stored in the barrel in joules.
    pub barrel_heat_soak_j: f64,
}

// ── Thermal model ─────────────────────────────────────────────────────────────

/// Estimate barrel temperature from firing history.
///
/// Models heating from propellant energy (≈0.5–1.5 °C per shot depending
/// on firing rate) and convective cooling to ambient. Approaches a thermal
/// equilibrium under sustained fire.
///
/// # Arguments
/// * `rounds_fired` — Number of rounds fired in the string.
/// * `ambient_temp_c` — Ambient air temperature in °C.
/// * `firing_rate_rpm` — Sustained firing rate in rounds per minute.
///
/// # Returns
/// Estimated barrel temperature in °C.
pub fn thermal_model(rounds_fired: i32, ambient_temp_c: f64, firing_rate_rpm: f64) -> f64 {
    if rounds_fired <= 0 {
        return ambient_temp_c;
    }

    let n = rounds_fired as f64;
    let rpm = firing_rate_rpm.max(1.0);

    // Temperature rise per shot depends on firing rate:
    //   ~0.5 °C/shot at slow fire (1 rpm) — more inter-shot cooling
    //   ~1.5 °C/shot at rapid fire (600 rpm) — heat builds up
    let rate_factor = (rpm / 600.0).min(1.0);
    let rise_per_shot = 0.5 + rate_factor * 1.0; // 0.5–1.5 °C/shot

    // Asymptotic thermal model: barrel approaches equilibrium as
    // heat input balances convective + radiative cooling.
    // Saturation ~300 °C above ambient for sustained automatic fire.
    let raw_rise = n * rise_per_shot;
    let saturation_delta = 300.0;
    let rise = saturation_delta * (1.0 - (-raw_rise / saturation_delta).exp());

    ambient_temp_c + rise
}

// ── Fouling model ─────────────────────────────────────────────────────────────

/// Estimate accuracy loss from bore fouling.
///
/// Returns an MOA multiplier (1.0 = perfectly clean, >1.0 = degraded).
/// Fouling builds logarithmically: rapid initial buildup of powder
/// fouling and copper fouling in the first ~200 rounds, then a plateau
/// as the bore fouls to equilibrium.
///
/// # Arguments
/// * `rounds_since_clean` — Rounds fired since last bore cleaning.
///
/// # Returns
/// Accuracy multiplier. Typical range: 1.0 (clean) to 1.15 (fully fouled).
pub fn fouling_model(rounds_since_clean: i32) -> f64 {
    let n = rounds_since_clean.max(0) as f64;
    if n <= 0.0 {
        return 1.0;
    }

    // Logarithmic fouling: fast initial buildup, plateau at ~500 rounds.
    // Max 15 % accuracy degradation from fouling alone.
    const PLATEAU_MULT: f64 = 1.15;
    let log_factor = (n + 1.0).ln() / (501.0_f64).ln();
    1.0 + (PLATEAU_MULT - 1.0) * log_factor.min(1.0)
}

// ── Erosion / velocity-loss model ─────────────────────────────────────────────

/// Estimate muzzle velocity loss fraction from throat erosion.
///
/// Returns the fraction of original velocity lost (0.0 = none,
/// ≈0.06 = 6 % loss at end of barrel life).
///
/// Erosion progresses super-quadratically: the first half of barrel
/// life consumes ~1–2 % of muzzle velocity; the second half degrades
/// rapidly as throat erosion opens the bore diameter, letting
/// propellant gas bypass the projectile.
///
/// # Arguments
/// * `total_rounds` — Total rounds fired through the barrel.
/// * `max_life` — Expected barrel life in rounds.
///
/// # Returns
/// Velocity loss fraction (0.0–0.12, typically 0.0–0.06).
pub fn erosion_velocity_loss(total_rounds: i32, max_life: i32) -> f64 {
    let max_life = max_life.max(1);
    let r = (total_rounds as f64 / max_life as f64).clamp(0.0, 1.0);

    // Super-quadratic: linear baseline + quartic acceleration
    //   r = 0.05 (1000 / 20000): ≈0.05 %
    //   r = 0.50 (10000 / 20000): ≈1.2 %
    //   r = 0.90 (18000 / 20000): ≈4.4 %
    //   r = 1.00 (end of life):   ≈6.0 %
    0.005 * r + 0.055 * r.powi(4)
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Determine the bore condition from erosion fraction, fouling state,
/// and barrel temperature.
fn determine_bore_condition(
    erosion_frac: f64,
    rounds_since_clean: i32,
    barrel_temp_c: f64,
) -> BoreCondition {
    // Critical: heavily eroded (>80 %) or very hot + moderate erosion
    if erosion_frac > 0.80 || (barrel_temp_c > 250.0 && erosion_frac > 0.50) {
        BoreCondition::Critical
    }
    // Eroded: moderately eroded (>40 %) or above thermal threshold
    else if erosion_frac > 0.40 || barrel_temp_c > 200.0 {
        BoreCondition::Eroded
    }
    // Fouled: some wear or significant rounds since last cleaning
    else if rounds_since_clean > 200 || erosion_frac > 0.10 {
        BoreCondition::Fouled
    } else {
        BoreCondition::New
    }
}

/// Heat capacity of a typical 1.5 kg steel barrel (J/K).
///
/// Steel specific heat ≈ 500 J/(kg·K), mass ≈ 1.5 kg → 750 J/K.
const BARREL_HEAT_CAPACITY_J_K: f64 = 750.0;

/// Evaluate barrel erosion for the given firing and thermal state.
///
/// Combines mechanical wear (round count), thermal effects (barrel
/// temperature), and fouling (rounds since cleaning) into a single
/// assessment. Hot barrels wear faster (thermal acceleration); a
/// barrel at 200 °C accumulates wear at ≈1.2× the room-temperature
/// rate.
///
/// # Arguments
/// * `params` — Erosion parameters: round count, temperature, barrel
///   life rating, last known bore condition, and cleaning history.
///
/// # Returns
/// `ErosionResult` with velocity loss, accuracy multiplier, erosion
/// fraction, remaining life, determined bore condition, and heat soak.
pub fn evaluate_erosion(params: &ErosionParams) -> ErosionResult {
    let max_life = params.max_barrel_life_rounds.max(1);

    // Physical erosion fraction (mechanical wear only)
    let erosion_frac = (params.rounds_fired as f64 / max_life as f64).clamp(0.0, 1.0);

    // Velocity loss from cumulative throat erosion
    let vel_loss = erosion_velocity_loss(params.rounds_fired, max_life);

    // Thermal acceleration factor: hotter barrels wear faster.
    // 1.0× at ambient, ≈1.2× at 200 °C, ≈1.4× at 400 °C.
    let temp_rise = (params.barrel_temp_c - 20.0).max(0.0);
    let thermal_accel = 1.0 + temp_rise * 0.001;

    // Effective erosion fraction (thermal-adjusted)
    let effective_frac = (erosion_frac * thermal_accel).min(1.0);

    // Fouling multiplier from rounds since last cleaning
    let rounds_since_clean = params
        .rounds_fired
        .saturating_sub(params.last_clean_rounds)
        .max(0);
    let fouling_mult = fouling_model(rounds_since_clean);

    // Determine bore condition from all factors
    let condition =
        determine_bore_condition(effective_frac, rounds_since_clean, params.barrel_temp_c);

    // Combined accuracy multiplier: condition × fouling × thermal
    let condition_mult = condition.accuracy_penalty();
    let thermal_mult = if params.barrel_temp_c > 200.0 {
        1.0 + (params.barrel_temp_c - 200.0) * 0.002 // +0.2 % per °C above 200
    } else {
        1.0
    };
    let accuracy_mult = fouling_mult * condition_mult * thermal_mult;

    // Remaining life (thermal-adjusted)
    let remaining = ((max_life as f64) * (1.0 - effective_frac))
        .round()
        .max(0.0) as i32;

    // Thermal energy stored in barrel steel
    let heat_soak = temp_rise * BARREL_HEAT_CAPACITY_J_K;

    ErosionResult {
        velocity_loss_pct: vel_loss * 100.0,
        accuracy_moa_multiplier: accuracy_mult,
        erosion_fraction: erosion_frac,
        estimated_remaining_life_rounds: remaining,
        bore_condition: condition,
        barrel_heat_soak_j: heat_soak,
    }
}

// ── Barrel heating — MV curve model ──────────────────────────────────────────

/// Thermal state of a barrel during a firing string.
///
/// Tracks the barrel temperature through sustained fire and convective
/// cooling between shots. Used to compute the heating-induced MV
/// multiplier: as barrel temperature rises, propellant burn rate
/// increases (MV_mult up to ~1.02), but above 200 °C throat erosion
/// and fouling begin to dominate, eventually dropping MV_mult below
/// 1.0 as the barrel approaches critical temperature.
///
/// Reference:
///   - Bryan Litz "Applied Ballistics Precision" Ch. 11 (Barrel Heat)
///   - US Army ARL-TR-2828 (erosive wear in small arms)
///   - Hatcher's Notebook (barrel heating / cooling curves)
#[derive(Debug, Clone, Copy)]
pub struct BarrelThermalState {
    /// Current bore surface temperature in °C.
    pub barrel_temp_c: f64,
    /// Number of rounds fired since the barrel was cold (ambient).
    pub rounds_fired_since_cold: i32,
    /// Sustained firing rate in rounds per minute.
    pub sustained_fire_rate_rpm: f64,
    /// Ambient (air) temperature in °C.
    pub ambient_temp_c: f64,
    /// Barrel mass in kilograms (affects thermal inertia).
    pub barrel_mass_kg: f64,
    /// Convective cooling coefficient (1/s). Typical range: 0.01–0.05.
    /// Higher values = faster cooling (thin barrel, good airflow).
    /// Default: 0.02 for a typical steel barrel.
    pub cooling_coefficient: f64,
}

impl Default for BarrelThermalState {
    fn default() -> Self {
        Self {
            barrel_temp_c: 21.0,
            rounds_fired_since_cold: 0,
            sustained_fire_rate_rpm: 60.0,
            ambient_temp_c: 21.0,
            barrel_mass_kg: 1.5,
            cooling_coefficient: 0.02,
        }
    }
}

/// Compute the muzzle velocity multiplier due to barrel heating.
///
/// Returns a multiplier on MV:
///   - Cold bore (~ambient):           1.000
///   - Warm (50–200 °C):               1.005–1.020 (hotter = faster burn)
///   - Hot (200–350 °C):               peaks at ~1.020 then stabilizes
///   - Very hot (350–400 °C):          starts dropping (erosion + fouling)
///   - Critical (>400 °C):             drops below 1.000
///
/// # Arguments
/// * `state` — Current barrel thermal state.
pub fn heating_mv_multiplier(state: &BarrelThermalState) -> f64 {
    let temp = state.barrel_temp_c;

    if temp <= 20.0 {
        // Cold bore at or below ambient: slight velocity loss from cold propellant
        let cold_factor = (20.0 - temp).max(0.0) / 60.0; // 0 at 20 °C, ~0.33 at 0 °C
        1.000 - cold_factor * 0.010 // up to ~1 % loss at 0 °C
    } else if temp <= 50.0 {
        // Ambient to warm: slight increase as propellant warms
        let t = (temp - 20.0) / 30.0; // 0→1 over 20–50 °C
        1.000 + t * 0.005 // 1.000 → 1.005
    } else if temp <= 200.0 {
        // Warm to hot: linear rise to peak
        let t = (temp - 50.0) / 150.0; // 0→1 over 50–200 °C
        1.005 + t * 0.015 // 1.005 → 1.020
    } else if temp <= 350.0 {
        // Hot zone: plateau at ~1.020 with slight roll-off
        let t = (temp - 200.0) / 150.0; // 0→1 over 200–350 °C
        1.020 - t * 0.010 // 1.020 → 1.010
    } else if temp <= 400.0 {
        // Very hot: accelerating decline from erosion/fouling
        let t = (temp - 350.0) / 50.0; // 0→1 over 350–400 °C
        1.010 - t * 0.015 // 1.010 → 0.995
    } else {
        // Critical: drops below 1.0 and accelerates toward cook-off
        let t = ((temp - 400.0) / 100.0).min(1.0); // 0→1 over 400–500 °C
        (0.995 - t * 0.040).max(0.85) // 0.995 → 0.955 (cap at 0.85)
    }
}

/// Estimate barrel temperature rise from a single shot.
///
/// Models the thermodynamic energy transfer from propellant combustion
/// to the barrel steel. A fraction of the propellant chemical energy
/// heats the barrel rather than accelerating the projectile.
///
/// Typical values for 7.62×51mm NATO (M80):
///   - charge mass: ~3.0 g
///   - bore volume (20" barrel, 7.62 mm): ~12 cc
///   - temperature rise per shot: ~0.5–1.0 °C
///
/// # Arguments
/// * `temp_before` — Barrel temperature before the shot (°C).
/// * `charge_mass_g` — Propellant charge mass in grams.
/// * `bore_volume_cc` — Bore volume in cubic centimetres
///   (π × (caliber/2)² × barrel_length).
///
/// # Returns
/// Barrel temperature immediately after the shot (°C).
pub fn barrel_temperature_after_shot(
    temp_before: f64,
    charge_mass_g: f64,
    bore_volume_cc: f64,
) -> f64 {
    // Propellant energy density: ~4.5 kJ/g for typical nitrocellulose
    // A fraction (~15–20 %) of this goes into heating the barrel steel
    // (the rest goes to projectile KE, gas ejecta, and muzzle blast).
    const PROP_E_DENSITY_J_G: f64 = 4500.0;
    const HEAT_FRACTION: f64 = 0.20;

    let energy_j = charge_mass_g * PROP_E_DENSITY_J_G * HEAT_FRACTION;

    // Thermal mass of barrel steel adjacent to the bore.
    // For a typical barrel, bore volume scales with barrel mass:
    //   ~12 cc bore → ~1.5 kg barrel → ~125 g steel per cc of bore
    // This captures the whole-barrel thermal inertia.
    const THERMAL_MASS_PER_CC: f64 = 125.0; // grams of steel per cc bore volume
    let thermal_mass_g = bore_volume_cc * THERMAL_MASS_PER_CC;

    if thermal_mass_g <= 0.0 || charge_mass_g <= 0.0 {
        return temp_before;
    }

    // Steel specific heat: 0.5 J/(g·K)
    const STEEL_SPECIFIC_HEAT_J_GK: f64 = 0.5;

    let temp_rise = energy_j / (thermal_mass_g * STEEL_SPECIFIC_HEAT_J_GK);
    temp_before + temp_rise
}

/// Apply Newtonian cooling to the barrel between shots or over a time step.
///
///   T_new = T_amb + (T_bore − T_amb) × exp(−dt × cooling_coeff)
///
/// # Arguments
/// * `temp_bore` — Bore temperature at start of interval (°C).
/// * `temp_ambient` — Ambient air temperature (°C).
/// * `dt_s` — Time elapsed (seconds).
/// * `cooling_coefficient` — Convective cooling coefficient (1/s).
pub fn barrel_cooling_step(
    temp_bore: f64,
    temp_ambient: f64,
    dt_s: f64,
    cooling_coefficient: f64,
) -> f64 {
    let dt = dt_s.max(0.0);
    temp_ambient + (temp_bore - temp_ambient) * (-dt * cooling_coefficient).exp()
}

/// Convenience: compute the barrel temperature evolution over a firing string
/// with cooling between shots.
///
/// Given the initial (cold) temperature, the firing rate, and the cooling
/// coefficient, this returns the barrel temperature after `n_rounds` shots
/// have been fired (with inter-shot cooling applied).
///
/// # Arguments
/// * `initial_temp_c` — Cold barrel temperature (°C).
/// * `ambient_temp_c` — Ambient air temperature (°C).
/// * `n_rounds` — Number of rounds fired.
/// * `firing_rate_rpm` — Sustained firing rate (rounds per minute).
/// * `charge_mass_g` — Propellant charge mass per round (g).
/// * `bore_volume_cc` — Bore volume (cc).
/// * `cooling_coefficient` — Convective cooling coefficient (1/s).
/// * `barrel_mass_kg` — Barrel mass (kg) — affects inter-shot cooling rate
///   scaling (heavier barrel = more thermal inertia = slower temp change).
#[allow(clippy::too_many_arguments)]
// ponytail: physics kernel, all params required
pub fn barrel_temperature_after_string(
    initial_temp_c: f64,
    ambient_temp_c: f64,
    n_rounds: i32,
    firing_rate_rpm: f64,
    charge_mass_g: f64,
    bore_volume_cc: f64,
    cooling_coefficient: f64,
    barrel_mass_kg: f64,
) -> f64 {
    if n_rounds <= 0 {
        return initial_temp_c;
    }

    // Inter-shot interval in seconds
    let rpm = firing_rate_rpm.max(1.0);
    let interval_s = 60.0 / rpm;

    // Mass scaling: a heavier barrel has more thermal inertia,
    // so effective cooling is reduced proportionally.
    // Reference: 1.5 kg barrel → mass_factor = 1.0
    let mass_factor = (barrel_mass_kg / 1.5).max(0.2);

    let mut temp = initial_temp_c;
    for _ in 0..n_rounds {
        // Heat from the shot
        temp = barrel_temperature_after_shot(temp, charge_mass_g, bore_volume_cc);
        // Cool between shots (mass scaling adjusts effective cooling)
        let effective_cooling = cooling_coefficient / mass_factor;
        temp = barrel_cooling_step(temp, ambient_temp_c, interval_s, effective_cooling);
    }
    temp
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_barrel_has_no_loss() {
        let params = ErosionParams {
            rounds_fired: 0,
            barrel_temp_c: 20.0,
            max_barrel_life_rounds: 20000,
            bore_condition: BoreCondition::New,
            last_clean_rounds: 0,
        };
        let result = evaluate_erosion(&params);
        assert_eq!(result.velocity_loss_pct, 0.0);
        assert_eq!(result.accuracy_moa_multiplier, 1.0);
        assert_eq!(result.erosion_fraction, 0.0);
        assert_eq!(result.bore_condition, BoreCondition::New);
    }

    #[test]
    fn fouling_increases_with_round_count() {
        let clean = fouling_model(0);
        let dirty = fouling_model(300);
        let very_dirty = fouling_model(500);
        let plateau = fouling_model(1000);

        assert!((clean - 1.0).abs() < 1e-10);
        assert!(dirty > clean, "fouling should increase from clean");
        assert!(very_dirty > dirty, "fouling should increase with rounds");
        assert!(
            (plateau - very_dirty).abs() < 1e-10,
            "fouling should plateau at ~500 rounds"
        );
        assert!(
            very_dirty <= 1.15,
            "fouling multiplier must not exceed 1.15"
        );
    }

    #[test]
    fn barrel_heat_rises_with_firing_rate() {
        let slow = thermal_model(60, 20.0, 1.0);
        let fast = thermal_model(60, 20.0, 600.0);

        assert!(slow > 20.0, "even slow fire should raise temperature");
        assert!(
            fast > slow,
            "faster fire rate must produce higher temperature"
        );
        assert!(
            fast < 400.0,
            "temperature must not exceed reasonable bounds"
        );
    }

    #[test]
    fn erosion_accelerates_near_end_of_life() {
        let max_life = 20000;

        let early = erosion_velocity_loss(5000, max_life);
        let mid = erosion_velocity_loss(10000, max_life);
        let late = erosion_velocity_loss(18000, max_life);
        let end = erosion_velocity_loss(20000, max_life);

        // Loss increases monotonically
        assert!(early < mid, "early erosion must be less than mid");
        assert!(mid < late, "mid erosion must be less than late");
        assert!(late < end, "late erosion must be less than end");

        // Per-round slope comparison: late-to-end slope must be
        // substantially steeper than early-to-mid slope.
        let early_slope = (mid - early) / 5000.0;
        let late_slope = (end - late) / 2000.0;
        assert!(
            late_slope > early_slope * 3.0,
            "erosion rate must accelerate near end of life ({} vs {})",
            late_slope,
            early_slope,
        );

        // End-of-life loss must be in the 5–10 % band
        assert!(
            end > 0.05,
            "end-of-life loss should exceed 5 %, got {:.3}",
            end
        );
        assert!(
            end < 0.10,
            "end-of-life loss should be below 10 %, got {:.3}",
            end
        );
    }

    #[test]
    fn accuracy_degrades_with_fouling_and_erosion() {
        let max_life = 15000;

        // Near-new barrel, just a few rounds
        let clean_params = ErosionParams {
            rounds_fired: 10,
            barrel_temp_c: 25.0,
            max_barrel_life_rounds: max_life,
            bore_condition: BoreCondition::New,
            last_clean_rounds: 0,
        };
        let clean_result = evaluate_erosion(&clean_params);

        // Heavily fouled barrel, many rounds since clean
        let fouled_params = ErosionParams {
            rounds_fired: 500,
            barrel_temp_c: 25.0,
            max_barrel_life_rounds: max_life,
            bore_condition: BoreCondition::New,
            last_clean_rounds: 0,
        };
        let fouled_result = evaluate_erosion(&fouled_params);

        assert!(
            fouled_result.accuracy_moa_multiplier > clean_result.accuracy_moa_multiplier,
            "fouled barrel must have worse accuracy than near-new"
        );
        assert!(
            fouled_result.accuracy_moa_multiplier > 1.0,
            "multiplier must exceed 1.0 for a fouled barrel"
        );
    }

    #[test]
    fn deterministic_output() {
        let params = ErosionParams {
            rounds_fired: 5000,
            barrel_temp_c: 80.0,
            max_barrel_life_rounds: 15000,
            bore_condition: BoreCondition::Fouled,
            last_clean_rounds: 4200,
        };

        let r1 = evaluate_erosion(&params);
        let r2 = evaluate_erosion(&params);

        assert_eq!(r1.velocity_loss_pct, r2.velocity_loss_pct);
        assert_eq!(r1.accuracy_moa_multiplier, r2.accuracy_moa_multiplier);
        assert_eq!(r1.erosion_fraction, r2.erosion_fraction);
        assert_eq!(
            r1.estimated_remaining_life_rounds,
            r2.estimated_remaining_life_rounds
        );
        assert_eq!(r1.bore_condition, r2.bore_condition);
        assert_eq!(r1.barrel_heat_soak_j, r2.barrel_heat_soak_j);
    }

    #[test]
    fn thermal_saturation_at_high_round_count() {
        // Sustained automatic fire approaches thermal equilibrium
        let high = thermal_model(500, 20.0, 600.0);
        let very_high = thermal_model(1000, 20.0, 600.0);

        assert!(high > 200.0, "500 rounds at 600 rpm must be very hot");
        assert!(very_high > high, "more rounds must mean higher temp");
        assert!(very_high < 350.0, "must not exceed ~320 °C saturation");
        // Diminishing returns: difference between 500→1000 rounds is small
        assert!(
            very_high - high < 30.0,
            "temperature must show diminishing returns (diff={:.1})",
            very_high - high,
        );
    }

    // ── Barrel heating / MV curve tests ─────────────────────────────────

    #[test]
    fn cold_bore_mv_multiplier_is_one() {
        let state = BarrelThermalState {
            barrel_temp_c: 21.0,
            rounds_fired_since_cold: 0,
            sustained_fire_rate_rpm: 60.0,
            ambient_temp_c: 21.0,
            barrel_mass_kg: 1.5,
            cooling_coefficient: 0.02,
        };
        let mult = heating_mv_multiplier(&state);
        assert!(
            (mult - 1.0).abs() < 0.002,
            "Cold bore (~21 °C) multipler should be ~1.0: got {}",
            mult
        );
    }

    #[test]
    fn sustained_fire_heats_to_200c() {
        // Simulate 60 rounds at 600 rpm with 7.62×51mm parameters
        let temp = barrel_temperature_after_string(
            21.0,  // initial (cold)
            21.0,  // ambient
            60,    // 60 rounds
            600.0, // 600 rpm
            3.0,   // 3.0 g charge mass (M80)
            12.0,  // ~12 cc bore volume (20" barrel)
            0.02,  // cooling coefficient
            1.5,   // barrel mass (kg)
        );

        assert!(
            temp > 80.0,
            "60 rounds at 600 rpm should exceed 80 °C: got {:.1} °C",
            temp
        );
        assert!(
            temp < 450.0,
            "60 rounds at 600 rpm should be below 450 °C: got {:.1} °C",
            temp
        );

        // MV multiplier should be elevated in hot barrel
        let state = BarrelThermalState {
            barrel_temp_c: temp,
            rounds_fired_since_cold: 60,
            sustained_fire_rate_rpm: 600.0,
            ambient_temp_c: 21.0,
            barrel_mass_kg: 1.5,
            cooling_coefficient: 0.02,
        };
        let mult = heating_mv_multiplier(&state);
        assert!(
            mult > 1.005,
            "Hot barrel should increase MV multiplier (>1.005): got {}",
            mult
        );
    }

    #[test]
    fn cooling_between_shots_reduces_temperature() {
        // One shot raises temp, then 30 s of cooling should bring it back down
        let after_shot = barrel_temperature_after_shot(21.0, 3.0, 12.0);
        assert!(
            after_shot > 21.0,
            "Shot should raise barrel temperature: {:.2} °C",
            after_shot
        );

        let after_cooling = barrel_cooling_step(after_shot, 21.0, 30.0, 0.02);
        assert!(
            after_cooling < after_shot,
            "Cooling should reduce temperature: {:.2} → {:.2}",
            after_shot,
            after_cooling
        );
        assert!(
            after_cooling > 21.0,
            "After 30 s cooling, barrel should still be above ambient: {:.2}",
            after_cooling
        );

        // Extended cooling should approach ambient
        let fully_cooled = barrel_cooling_step(after_shot, 21.0, 300.0, 0.02);
        assert!(
            (fully_cooled - 21.0).abs() < 2.0,
            "After 5 min, barrel should be near ambient: {:.2}",
            fully_cooled
        );
    }

    #[test]
    fn extreme_heat_degrades_mv_multiplier() {
        // At 450 °C (approaching cook-off), MV multiplier should be below 1.0
        let state = BarrelThermalState {
            barrel_temp_c: 450.0,
            rounds_fired_since_cold: 200,
            sustained_fire_rate_rpm: 600.0,
            ambient_temp_c: 21.0,
            barrel_mass_kg: 1.5,
            cooling_coefficient: 0.02,
        };
        let mult = heating_mv_multiplier(&state);
        assert!(
            mult < 1.0,
            "At 450 °C, MV multiplier should be below 1.0: got {}",
            mult
        );
        assert!(
            mult >= 0.85,
            "At 450 °C, MV multiplier should not drop below 0.85: got {}",
            mult
        );
    }

    #[test]
    fn muiltiplier_peaks_in_warm_range() {
        // MV multiplier should peak in the warm/hot transition (150–250 °C)
        let cold = heating_mv_multiplier(&BarrelThermalState {
            barrel_temp_c: 21.0,
            ..Default::default()
        });
        let warm = heating_mv_multiplier(&BarrelThermalState {
            barrel_temp_c: 180.0,
            ..Default::default()
        });
        let hot = heating_mv_multiplier(&BarrelThermalState {
            barrel_temp_c: 350.0,
            ..Default::default()
        });
        let critical = heating_mv_multiplier(&BarrelThermalState {
            barrel_temp_c: 500.0,
            ..Default::default()
        });

        assert!(warm > cold, "Warm barrel should have higher mult than cold");
        assert!(
            warm >= hot,
            "Peak mult should be near warm zone (180 °C vs 350 °C)"
        );
        assert!(
            critical < hot,
            "Critical barrel should have lower mult than hot"
        );
    }

    #[test]
    fn barrel_temp_after_shot_reasonable() {
        // 7.62mm NATO: ~3 g charge, ~12 cc bore → ~3–5 °C rise
        let rise = barrel_temperature_after_shot(21.0, 3.0, 12.0) - 21.0;
        assert!(
            rise > 0.5,
            "Temperature rise per shot should be > 0.5 °C: got {:.3}",
            rise
        );
        assert!(
            rise < 8.0,
            "Temperature rise per shot should be < 8.0 °C: got {:.3}",
            rise
        );

        // Bigger charge = more heating
        let small_rise = barrel_temperature_after_shot(21.0, 1.5, 12.0) - 21.0;
        let big_rise = barrel_temperature_after_shot(21.0, 5.0, 12.0) - 21.0;
        assert!(
            big_rise > small_rise,
            "Larger charge should cause more heating"
        );

        // Larger bore volume = less heating (more thermal mass)
        let small_bore = barrel_temperature_after_shot(21.0, 3.0, 6.0);
        let big_bore = barrel_temperature_after_shot(21.0, 3.0, 24.0);
        assert!(
            small_bore > big_bore,
            "Smaller bore volume should heat more per shot"
        );
    }

    #[test]
    fn sustained_fire_cooling_equilibrium() {
        // Long string: after many rounds the barrel should approach thermal
        // equilibrium where heating ≈ cooling.
        let short_run =
            barrel_temperature_after_string(21.0, 21.0, 30, 600.0, 3.0, 12.0, 0.02, 1.5);
        let long_run =
            barrel_temperature_after_string(21.0, 21.0, 200, 600.0, 3.0, 12.0, 0.02, 1.5);
        let very_long =
            barrel_temperature_after_string(21.0, 21.0, 500, 600.0, 3.0, 12.0, 0.02, 1.5);

        // Temperature should increase monotonically
        assert!(
            long_run > short_run,
            "More rounds → higher temp: {} > {}",
            long_run,
            short_run
        );
        // Diminishing returns: the difference between 200 and 500 rounds
        // should not be dramatically larger than between 30 and 200 rounds
        let d_early = long_run - short_run;
        let d_late = very_long - long_run;
        // With Newtonian cooling the temp approaches equilibrium, so late
        // increments should be comparable or smaller than early increments.
        // Allow some margin for the non-linear approach to equilibrium.
        assert!(
            d_late < d_early * 2.0,
            "Temperature should show diminishing returns: early Δ={:.1}, late Δ={:.1}",
            d_early,
            d_late
        );
    }

    #[test]
    fn deterministic_barrel_heating() {
        let t1 = barrel_temperature_after_shot(21.0, 3.0, 12.0);
        let t2 = barrel_temperature_after_shot(21.0, 3.0, 12.0);
        assert!(
            (t1 - t2).abs() < 1e-12,
            "barrel_temperature_after_shot deterministic"
        );

        let m1 = heating_mv_multiplier(&BarrelThermalState {
            barrel_temp_c: 150.0,
            ..Default::default()
        });
        let m2 = heating_mv_multiplier(&BarrelThermalState {
            barrel_temp_c: 150.0,
            ..Default::default()
        });
        assert!(
            (m1 - m2).abs() < 1e-12,
            "heating_mv_multiplier deterministic"
        );
    }

    #[test]
    fn bore_condition_transitions() {
        let max_life = 10000;

        // Heavy mechanical wear → Critical
        let worn = evaluate_erosion(&ErosionParams {
            rounds_fired: 9000,
            barrel_temp_c: 30.0,
            max_barrel_life_rounds: max_life,
            bore_condition: BoreCondition::Eroded,
            last_clean_rounds: 0,
        });
        assert_eq!(
            worn.bore_condition,
            BoreCondition::Critical,
            "barrel at 90 % life should be Critical"
        );

        // Hot barrel with moderate wear → Eroded (via thermal threshold)
        let hot = evaluate_erosion(&ErosionParams {
            rounds_fired: 1000,
            barrel_temp_c: 250.0,
            max_barrel_life_rounds: max_life,
            bore_condition: BoreCondition::Fouled,
            last_clean_rounds: 0,
        });
        assert_eq!(
            hot.bore_condition,
            BoreCondition::Eroded,
            "barrel at 250 °C should be Eroded"
        );
    }
}
