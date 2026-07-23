// ── IRL Weapon/Ammo Lookup ───────────────────────────────────────────────────
//
// Compile-time PHF map of real-world weapon and ammunition specifications.
// Resolution priority: PHF exact → PHF substring → type+caliber fallback.
// Override mechanism: SQF calls abe_register_override() at init to store
// mod-provided or ACE3 values above these built-in IRL specs.

use crate::generated::{I4L_ARMOR_PLATES, I4L_MATERIALS, IRL_AMMO, IRL_WEAPONS, IR_CLOTHING};
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

/// Parameters for a single armor plate, resolved from compile-time PHF data.
#[derive(Debug, Clone)]
pub struct ArmorPlateParams {
    pub material: String,
    pub thickness_mm: f64,
    pub angle_deg: f64,
    pub backing: String,
    pub backing_thickness_mm: f64,
    pub backing_angle_deg: f64,
}

/// Material properties resolved from compile-time PHF data.
#[derive(Debug, Clone)]
pub struct MaterialParams {
    pub density_gcm3: f64,
    pub hardness_bhn: f64,
    pub tensile_strength_mpa: f64,
    pub rha_equivalent: f64,
    pub ductility: f64,
    pub spall_coeff: f64,
}

/// Parameters for a clothing/headgear/vest item, resolved from compile-time PHF data.
#[derive(Debug, Clone)]
#[repr(C)]
pub struct ClothingParams {
    /// Item type category: Glasses, Headgear, Uniform, or Vest.
    pub item_type: [u8; 32],
    /// Functional clothing category (e.g. ballistic_helmet, plate_carrier, combat_uniform).
    pub clothing_category: [u8; 48],
    /// Primary construction material (e.g. ceramic_sic, composite_kevlar, cloth).
    pub primary_material: [u8; 32],
    /// Secondary/backing material (e.g. uhmwpe, spall_liner, empty = none).
    pub backing_material: [u8; 32],
    /// Areal thickness of the ballistic insert in mm (0.0 = non-ballistic).
    pub thickness_mm: f64,
    /// NIJ body armour rating as a string (e.g. "IIIA", "III", "IV", "N/A").
    pub nij_rating: [u8; 16],
    /// RHA equivalent thickness in mm from the combination of materials.
    pub rha_mm: f64,
    /// Confidence score for the assigned properties (0.0–1.0).
    pub confidence: f64,
    /// Manufacturer name string.
    pub manufacturer: [u8; 48],
    /// Specific model / product line name.
    pub model: [u8; 64],
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

// ── Armor plate resolution ──────────────────────────────────────────────────

/// Parse a pipe-delimited armor plate value string into `ArmorPlateParams`.
fn parse_armor_value(val: &str) -> ArmorPlateParams {
    let parts: Vec<&str> = val.split('|').collect();
    ArmorPlateParams {
        material: parts.first().unwrap_or(&"").to_string(),
        thickness_mm: parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0.0),
        angle_deg: parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0.0),
        backing: parts.get(3).unwrap_or(&"").to_string(),
        backing_thickness_mm: parts.get(4).and_then(|s| s.parse().ok()).unwrap_or(0.0),
        backing_angle_deg: parts.get(5).and_then(|s| s.parse().ok()).unwrap_or(0.0),
    }
}

/// Resolve armor plate parameters for a vehicle/plate combination.
///
/// Key format: `"{vehicle}/{plate_name}"` (e.g. `"rhs_t90a_tv/hull_front_upper"`).
/// Returns `None` if no matching plate is found.
pub fn resolve_armor(vehicle: &str, plate_name: &str) -> Option<ArmorPlateParams> {
    let key = format!("{}/{}", vehicle, plate_name);
    I4L_ARMOR_PLATES.get(&key).map(|val| parse_armor_value(val))
}

// ── Material resolution ─────────────────────────────────────────────────────

/// Parse a pipe-delimited material value string into `MaterialParams`.
fn parse_material_value(val: &str) -> MaterialParams {
    let parts: Vec<&str> = val.split('|').collect();
    // parts[0] = display_name (string, skipped for numeric parsing)
    MaterialParams {
        density_gcm3: parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0.0),
        hardness_bhn: parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0.0),
        tensile_strength_mpa: parts.get(3).and_then(|s| s.parse().ok()).unwrap_or(0.0),
        rha_equivalent: parts.get(4).and_then(|s| s.parse().ok()).unwrap_or(1.0),
        ductility: parts.get(5).and_then(|s| s.parse().ok()).unwrap_or(0.0),
        spall_coeff: parts.get(6).and_then(|s| s.parse().ok()).unwrap_or(0.0),
    }
}

/// Resolve material properties by material ID.
/// Returns `None` if the material is not found.
pub fn resolve_material(material_id: &str) -> Option<MaterialParams> {
    I4L_MATERIALS
        .get(material_id)
        .map(|val| parse_material_value(val))
}

/// Compute effective RHA thickness for a given vehicle armor plate.
///
/// Multiplies the plate's thickness by its material's RHA equivalence factor.
/// Returns 0.0 if either the plate or material is not found.
pub fn resolve_effective_rha(vehicle: &str, plate_name: &str) -> f64 {
    let plate = match resolve_armor(vehicle, plate_name) {
        Some(p) => p,
        None => return 0.0,
    };
    let mat = match resolve_material(&plate.material) {
        Some(m) => m,
        None => return 0.0,
    };
    plate.thickness_mm * mat.rha_equivalent
}

// ── Clothing / wearable resolution ──────────────────────────────────────────

/// Parse a pipe-delimited clothing value string into `ClothingParams`.
fn parse_clothing_value(val: &str) -> ClothingParams {
    let parts: Vec<&str> = val.split('|').collect();
    let str_to_arr = |s: &str, buf: &mut [u8]| {
        let bytes = s.as_bytes();
        let len = bytes.len().min(buf.len() - 1);
        buf[..len].copy_from_slice(bytes);
        buf[len] = 0;
    };

    let mut item_type = [0u8; 32];
    str_to_arr(parts.first().unwrap_or(&""), &mut item_type);

    let mut clothing_category = [0u8; 48];
    str_to_arr(parts.get(1).unwrap_or(&""), &mut clothing_category);

    let mut primary_material = [0u8; 32];
    str_to_arr(parts.get(2).unwrap_or(&""), &mut primary_material);

    let mut backing_material = [0u8; 32];
    str_to_arr(parts.get(3).unwrap_or(&""), &mut backing_material);

    let mut nij_rating = [0u8; 16];
    str_to_arr(parts.get(5).unwrap_or(&""), &mut nij_rating);

    let mut manufacturer = [0u8; 48];
    str_to_arr(parts.get(8).unwrap_or(&""), &mut manufacturer);

    let mut model = [0u8; 64];
    str_to_arr(parts.get(9).unwrap_or(&""), &mut model);

    ClothingParams {
        item_type,
        clothing_category,
        primary_material,
        backing_material,
        thickness_mm: parts.get(4).and_then(|s| s.parse().ok()).unwrap_or(0.0),
        nij_rating,
        rha_mm: parts.get(6).and_then(|s| s.parse().ok()).unwrap_or(0.0),
        confidence: parts.get(7).and_then(|s| s.parse().ok()).unwrap_or(0.0),
        manufacturer,
        model,
    }
}

/// Resolve clothing/headgear/vest parameters for an Arma class name.
///
/// Returns `Some(ClothingParams)` if the class is found in the
/// compile-time PHF map, `None` otherwise.
pub fn resolve_clothing(class: &str) -> Option<ClothingParams> {
    IR_CLOTHING.get(class).map(|val| parse_clothing_value(val))
}
