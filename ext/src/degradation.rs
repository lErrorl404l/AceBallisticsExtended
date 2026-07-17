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
