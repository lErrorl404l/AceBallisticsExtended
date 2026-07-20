// ── IRL Weapon/Ammo Lookup ───────────────────────────────────────────────────
//
// Compile-time PHF map of real-world weapon and ammunition specifications.
// Resolution priority: PHF exact → PHF substring → type+caliber fallback.
// Override mechanism: SQF calls abe_register_override() at init to store
// mod-provided or ACE3 values above these built-in IRL specs.

use crate::generated::{IRL_AMMO, IRL_WEAPONS};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

// ── Structs ──────────────────────────────────────────────────────────────────

/// Physical parameters that define a real-world firearm.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct WeaponParams {
    pub barrel_length_mm: f64,
    pub chamber_pressure_mpa: f64,
    pub caliber_mm: f64,
    pub projectile_mass_g: f64,
    pub barrel_twist_mm: f64,
}

/// Ballistic parameters that define a real-world cartridge.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct AmmoParams {
    pub bullet_diameter_mm: f64,
    pub projectile_mass_g: f64,
    pub bc_g1: f64,
    pub bc_g7: f64,
    pub drag_model: u8, // 1=G1, 7=G7, 8=G8
}

/// Result of a weapon resolution attempt.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct ResolveResult {
    pub params: WeaponParams,
    pub confidence: f64,
    pub matched_model: [u8; 64], // null-terminated IRL model name
}

// ── Override registry (runtime, populated by SQF at init) ────────────────────

static OVERRIDES: OnceLock<Mutex<HashMap<String, WeaponParams>>> = OnceLock::new();

fn overrides() -> &'static Mutex<HashMap<String, WeaponParams>> {
    OVERRIDES.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Register a weapon override at init time.
pub fn register_override(class: &str, params: WeaponParams) {
    overrides()
        .lock()
        .unwrap()
        .insert(class.to_string(), params);
}

/// Look up an override by class name.
fn find_override(class: &str) -> Option<WeaponParams> {
    overrides().lock().unwrap().get(class).copied()
}

/// Copy a byte string into a fixed-size array, null-padded.
fn str_to_array(s: &str, buf: &mut [u8]) {
    let bytes = s.as_bytes();
    let len = bytes.len().min(buf.len() - 1);
    buf[..len].copy_from_slice(&bytes[..len]);
    buf[len] = 0;
}

// ── Normalization ────────────────────────────────────────────────────────────

/// Strip Arma/mod prefixes from a weapon class name for matching.
fn normalize(class: &str) -> String {
    let mut s = class.to_lowercase();

    // Strip weapon type prefixes (Arma 3 standard)
    for prefix in &[
        "arifle_", "srifle_", "hgun_", "smg_", "lmg_", "mmg_", "sgun_", "launch_", "pdw_", "dmr_",
        "hmg_", "gmg_", "mortar_",
    ] {
        if let Some(rest) = s.strip_prefix(prefix) {
            s = rest.to_string();
            break;
        }
    }

    // Dynamic mod prefix stripping: strip leading `_`-delimited segments that
    // look like mod identifiers. Mod prefixes are alpha-only (rhs, cup, gm,
    // 3cb, ace, vn, spe, csla, ws, rf, etc.). Weapon model designations
    // (m4, ak47, p07, mk18) contain digits mixed with letters — keep those.
    while let Some(uscore) = s.find('_') {
        let (candidate, rest) = s.split_at(uscore);
        let rest = &rest[1..]; // skip '_'

        // Candidate contains a digit → weapon model number, not mod prefix
        if candidate.contains(|c: char| c.is_ascii_digit()) {
            break;
        }
        // Candidate too short (shouldn't happen given digit rule, but safety)
        if candidate.len() < 2 {
            break;
        }
        // Candidate too long for a typical mod prefix
        if candidate.len() > 8 {
            break;
        }
        // Remaining is a short suffix (< 3 chars like _F, _d, _w)
        if rest.len() < 3 {
            break;
        }
        // Remaining has no alphabetic chars (pure numeric variant)
        if !rest.contains(|c: char| c.is_ascii_alphabetic()) {
            break;
        }

        s = rest.to_string();
    }

    s
}

// ── Fallback defaults by weapon type ─────────────────────────────────────────

/// Classify a weapon class into a broad type for fallback defaults.
fn classify_type(class: &str) -> &'static str {
    let c = class.to_lowercase();
    if c.contains("pistol") || c.starts_with("hgun") {
        "pistol"
    } else if c.contains("smg") || c.contains("pdw") {
        "smg"
    } else if c.contains("sniper") || c.contains("srifle") || c.contains("dmr") || c.contains("lr")
    {
        "sniper"
    } else if c.contains("sgun") || c.contains("shotgun") {
        "shotgun"
    } else if c.contains("launch") || c.contains("launcher") || c.contains("rpg") {
        "launcher"
    // Vehicle weapon types — checked before generic "mg" below since
    // "hmg", "gmg" are subsets of "mg" and would be misclassified as "lmg".
    } else if c.contains("hmg") {
        "hmg"
    } else if c.contains("gmg") {
        "grenade_launcher"
    } else if c.contains("cannon") || c.contains("autocannon") || c.contains("gatling") {
        "cannon"
    } else if c.contains("mortar") {
        "mortar"
    } else if c.contains("lmg") || c.contains("mmg") || c.contains("mg") {
        "lmg"
    } else {
        "rifle"
    } // default to rifle
}

/// Return fallback WeaponParams for an unrecognised weapon class.
fn fallback_for(class: &str, caliber_hint: f64) -> (WeaponParams, f64) {
    let wtype = classify_type(class);
    let cal = if caliber_hint > 0.0 {
        caliber_hint
    } else {
        7.62
    };

    let (barrel, twist, pressure, mass) = match wtype {
        "pistol" => (120.0, 250.0, 240.0, 8.0),
        "smg" => (200.0, 250.0, 240.0, 8.0),
        "sniper" => (610.0, 305.0, 380.0, 10.0),
        "lmg" => (508.0, 305.0, 380.0, 9.5),
        "shotgun" => (470.0, 760.0, 70.0, 32.0),
        "launcher" => (0.0, 0.0, 0.0, 0.0),
        "cannon" => (5000.0, 0.0, 500.0, 20000.0),
        "autocannon" => (2400.0, 0.0, 380.0, 280.0),
        "mortar" => (1200.0, 0.0, 100.0, 3200.0),
        "grenade_launcher" => (300.0, 0.0, 200.0, 230.0),
        "hmg" => (1143.0, 381.0, 400.0, 42.0),
        _ => (368.0, 254.0, 380.0, 8.0), // rifle default (7.62mm NATO ~147gr)
    };

    (
        WeaponParams {
            barrel_length_mm: barrel,
            chamber_pressure_mpa: pressure,
            caliber_mm: cal,
            projectile_mass_g: mass,
            barrel_twist_mm: twist,
        },
        0.3,
    )
}

// ── Main resolution ──────────────────────────────────────────────────────────

/// Resolve weapon parameters for an Arma weapon class.
///
/// Returns the best-match IRL weapon specs and a confidence score.
/// Caller should prefer ACE3/ABO config values over this result;
/// use this as the final fallback when config has no values.
pub fn resolve_weapon(class: &str, caliber_hint: f64) -> ResolveResult {
    // 1. Check overrides first (registered by SQF at init)
    if let Some(override_params) = find_override(class) {
        let mut model = [0u8; 64];
        str_to_array("override", &mut model);
        return ResolveResult {
            params: override_params,
            confidence: 1.0,
            matched_model: model,
        };
    }

    let normalized = normalize(class);
    let clean = normalized.replace(['_', '-'], "");
    let class_lower = class.to_lowercase();

    // 2. PHF map substring match — find longest matching IRL name
    let mut best_match: Option<(&'static WeaponParams, usize)> = None;
    let mut best_key: &str = "";

    // ponytail: linear scan over ~200 entries; < 1µs per weapon, only
    // called at init per unique class. HashMap if this ever shows up on
    // a profile, but it won't.
    for (key, params) in IRL_WEAPONS.entries() {
        // Match against clean (underscore-free), then normalized (prefix-stripped),
        // then raw class name (catches weapons where normalize over-strips model prefix).
        if !clean.contains(key) && !normalized.contains(key) && !class_lower.contains(key) {
            continue;
        }
        // For short keys (≤4 chars), require word-boundary match to prevent
        // false positives like "g3" matching inside "rpg32" or "m2" inside "hmmwv".
        if key.len() <= 4
            && !normalized.starts_with(key)
            && !normalized.contains(&format!("_{}", key))
        {
            continue;
        }
        if best_match.is_none_or(|(_, len)| key.len() > len) {
            best_match = Some((params, key.len()));
            best_key = key;
        }
    }

    if let Some((params, _)) = best_match {
        let caliber_ok = caliber_hint <= 0.0 || (caliber_hint - params.caliber_mm).abs() < 2.0;
        let confidence = if caliber_ok { 0.95 } else { 0.75 };

        let model_bytes = best_key.as_bytes();
        let mut model_arr = [0u8; 64];
        let len = model_bytes.len().min(63);
        model_arr[..len].copy_from_slice(&model_bytes[..len]);

        return ResolveResult {
            params: *params,
            confidence,
            matched_model: model_arr,
        };
    }

    // 3. Fallback
    let (fallback_params, confidence) = fallback_for(class, caliber_hint);
    let model = format!("fallback_{}", classify_type(class));
    let model_bytes = model.as_bytes();
    let mut model_arr = [0u8; 64];
    let len = model_bytes.len().min(63);
    model_arr[..len].copy_from_slice(&model_bytes[..len]);

    ResolveResult {
        params: fallback_params,
        confidence,
        matched_model: model_arr,
    }
}

/// Resolve ammo parameters for an Arma ammo class.
pub fn resolve_ammo(class: &str) -> (AmmoParams, f64) {
    let normalized = class.to_lowercase();
    let clean = normalized.replace(['_', '-'], "");

    // Substring match against PHF map
    for (key, params) in IRL_AMMO.entries() {
        if clean.contains(key) {
            return (*params, 0.95);
        }
    }

    // Fallback
    let cal = if clean.contains("556") || clean.contains("5_56") {
        5.69
    } else if clean.contains("762") || clean.contains("7_62") {
        7.82
    } else if clean.contains("9mm") || clean.contains("9_") {
        9.01
    } else if clean.contains("545") || clean.contains("5_45") {
        5.60
    } else if clean.contains("127") || clean.contains("12_7") || clean.contains("50cal") {
        12.7
    } else if clean.contains("338") {
        8.58
    } else if clean.contains("408") {
        10.36
    } else {
        7.62
    };

    let (mass, bc_g1, bc_g7, dm) = match cal as i32 {
        5 => (4.0, 0.151, 0.075, 7),
        7 => (9.5, 0.200, 0.100, 7),
        9 => (8.0, 0.080, 0.040, 1),
        12 => (42.0, 0.350, 0.175, 7),
        _ => (9.5, 0.200, 0.100, 7),
    };

    (
        AmmoParams {
            bullet_diameter_mm: cal,
            projectile_mass_g: mass,
            bc_g1,
            bc_g7,
            drag_model: dm,
        },
        0.3,
    )
}
