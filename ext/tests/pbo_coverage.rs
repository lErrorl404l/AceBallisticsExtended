// ABE - PBO Coverage Test
//
// Scans ALL Arma 3 PBOs for CfgWeapons/CfgAmmo class definitions, walks
// inheritance chains to find effective base weapons, and validates that
// the IRL PHF lookup system (resolve_weapon / resolve_ammo) covers them
// with ≥80% coverage.
//
// This is the closest test to in-game behavior: the extension receives raw
// class names from the game engine, and this test exercises that exact
// pipeline — armake extracts the live config.bin from each PBO, the test
// parses the derapified config.cpp, and feeds each class name through
// the same resolve_weapon path the extension uses at runtime.
//
// Usage:
//   ABE_PBO_DIR="/ext/SteamLibrary/steamapps/common/Arma 3/Addons" \
//     cargo test --test pbo_coverage -- --nocapture

use abe_ballistics_ext::ir_lookup;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

// ── Thresholds ────────────────────────────────────────────────────────────────

const MIN_WEAPON_COVERAGE: f64 = 80.0;
const MIN_AMMO_COVERAGE: f64 = 80.0;

const VEHICLE_NON_WEAPON_BASES: &[&str] = &[
    "FlareLauncherBase",
    "SmokeLauncherBase",
    "CMFlareLauncherBase",
    "MineDetectorBase",
    "SearchLightBase",
];

// ── Inheritance chain classifier ──────────────────────────────────────────────
//
// In Arma 3's config hierarchy, CfgWeapons contains both real firearms and
// non-weapon items (scopes, suppressors, bipods, uniforms). The only reliable
// way to distinguish them is to walk each class's inheritance chain and check
// whether an ancestor is a known weapon base (Rifle, Pistol, Launcher, etc.)
// vs a known non-weapon item base (ItemCore → optics/muzzles/flashlights).

/// Base classes whose descendants are real firearms.
const WEAPON_BASES: &[&str] = &[
    "Rifle",
    "RifleCore",
    "Rifle_Base_F",
    "Launcher",
    "Launcher_Base_F",
    "LauncherRPG_Base_F",
    "HandGun",
    "HandGunCore",
    "HandGunBase",
    "Pistol",
    "Pistol_Base_F",
    "Weapon",
    "Weapon_Base_F",
];

/// Base classes whose descendants are weapon systems mounted on vehicles
/// (machine guns, autocannons, mortars, launcher body).
const VEHICLE_WEAPON_BASES: &[&str] = &[
    "MGun",
    "MGunCore",
    "CannonCore",
    "MortarCore",
    "LauncherCore", // shared with Launcher_Base_F (handheld → WEAPON_BASES)
];

/// Base classes whose descendants are game items (accessories, scopes, etc.)
/// placed in CfgWeapons but NOT actual firearms.
const ACCESSORY_BASES: &[&str] = &[
    "ItemCore",
    "InventoryItem_Base_F",
    "InventoryMuzzleItem_Base_F",
    "InventoryOpticsItem_Base_F",
    "InventoryFlashLightItem_Base_F",
    "InventoryUnderBarrelItem_Base_F",
];

/// Non-weapon items also placed in CfgWeapons (grenades, mines, magazines).
const OTHER_NON_WEAPON_BASES: &[&str] = &[
    "MineBase",
    "Mine_Base_F",
    "GrenadeCore",
    "Grenade_Base_F",
    "CA_Magazine",
    "CA_LauncherMagazine",
];

/// Framework base classes — neither weapons nor items, just config scaffolding.
const FRAMEWORK_BASES: &[&str] = &["Default", "WeaponSlotInfo"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClassKind {
    /// A real firearm — inherits from Rifle / Pistol / Launcher / etc.
    Weapon,
    /// A vehicle-mounted weapon — MG, autocannon, mortar.
    VehicleWeapon,
    /// An accessory / scope / muzzle / item — inherits from ItemCore etc.
    Accessory,
    /// A framework base class like Default, Rifle_Base_F itself, etc.
    Framework,
    /// Unclassifiable — chain doesn't reach a recognized base.
    Unknown,
}

/// Walk the full inheritance chain of a class and classify it.
///
/// Classification distinguishes real firearms from items that share the same
/// CfgWeapons section but aren't ballistic weapons (aircraft ordnance, flares,
/// searchlights, mine detectors, etc.) by tracking the PATH through the
/// inheritance tree — not just the deepest base class.
fn classify_class(class: &str, parents: &HashMap<String, Option<String>>) -> ClassKind {
    // Quick checks first for known names
    if FRAMEWORK_BASES.contains(&class) || class.ends_with("_Base_F") || class.ends_with("_base_F")
    {
        return ClassKind::Framework;
    }
    if WEAPON_BASES.contains(&class) || VEHICLE_WEAPON_BASES.contains(&class) {
        return ClassKind::Framework; // the base itself, not an instance
    }
    if ACCESSORY_BASES.contains(&class)
        || OTHER_NON_WEAPON_BASES.contains(&class)
        || VEHICLE_NON_WEAPON_BASES.contains(&class)
    {
        return ClassKind::Framework;
    }

    // Path tracking — these intermediates tell us which sub-tree of a shared
    // base class we pass through.
    let mut passed_rocketpods = false;
    let mut passed_missilelauncher = false;
    let mut passed_smokelauncher = false;

    // Walk the chain
    let mut current = class;
    let mut visited = HashSet::new();
    visited.insert(current.to_string());

    loop {
        // Known fake/special items that aren't ballistic weapons
        if matches!(
            current,
            "cannon_railgun"
                | "cannon_railgun_fake"
                | "H_FakeHeadgear"
                | "MineDetector"
                | "DetectorCore"
                | "SEARCHLIGHT"
        ) {
            return ClassKind::Accessory;
        }

        // Track path through the Launcher tree
        if current == "RocketPods" {
            passed_rocketpods = true;
        }
        if current == "MissileLauncher" {
            passed_missilelauncher = true;
        }
        if current == "SmokeLauncher" {
            passed_smokelauncher = true;
        }

        // Does this ancestor make us a small arm?
        if WEAPON_BASES.contains(&current) {
            return ClassKind::Weapon;
        }

        // Vehicle weapon? Check path first for sub-tree disambiguation.
        if VEHICLE_WEAPON_BASES.contains(&current) {
            // RocketPods / MissileLauncher path → aircraft ordnance
            if passed_rocketpods || passed_missilelauncher {
                return ClassKind::Accessory;
            }
            // SmokeLauncher path → flares, smoke, searchlights
            if passed_smokelauncher {
                return ClassKind::Accessory;
            }
            return ClassKind::VehicleWeapon;
        }

        // Does this ancestor make us an accessory/item?
        if ACCESSORY_BASES.contains(&current) || OTHER_NON_WEAPON_BASES.contains(&current) {
            return ClassKind::Accessory;
        }
        // Vehicle non-weapon (flares, smoke, searchlight, mine detector)?
        if VEHICLE_NON_WEAPON_BASES.contains(&current) {
            return ClassKind::Accessory;
        }
        // Framework base? (Only explicit FRAMEWORK_BASES during walk — _Base_F
        // suffix is too broad: weapon_LGBLauncherBase is a real weapon component,
        // not scaffolding like Rifle_Base_F.)
        if FRAMEWORK_BASES.contains(&current) {
            return ClassKind::Framework;
        }

        match parents.get(current) {
            Some(Some(parent)) => {
                if !visited.insert(parent.clone()) {
                    return ClassKind::Unknown; // cycle
                }
                current = parent.as_str();
            },
            _ => return ClassKind::Unknown, // reached root without classification
        }
    }
}

// ── Config parser ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct ClassDef {
    name: String,
    parent: Option<String>,
}

/// Parse a derapified config.cpp string and extract all class definitions
/// at depth 1 within a named section.
fn extract_section_classes(config: &str, section: &str) -> Vec<ClassDef> {
    let mut classes = Vec::new();
    let mut in_target = false;
    let mut brace_depth: i32 = -1;
    let target_prefix = format!("class {} ", section);
    let target_prefix2 = format!("class {}", section);

    for line in config.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }

        if !in_target {
            if trimmed.starts_with(&target_prefix) || trimmed.starts_with(&target_prefix2) {
                if trimmed.ends_with('{') || trimmed.contains('{') {
                    in_target = true;
                    brace_depth = 0;
                }
            }
            continue;
        }

        let opens: usize = trimmed.matches('{').count();
        let closes: usize = trimmed.matches('}').count();
        let depth_before = brace_depth;
        brace_depth += opens as i32;

        // Check for class definitions at depth 0 (immediate child of section)
        if depth_before <= 0 && brace_depth >= 0 && opens > 0 {
            // `class Name;` forward declaration
            if let Some(name) = trimmed.strip_prefix("class ").and_then(|s| {
                if s.ends_with(';') && !s.contains('{') {
                    Some(s[..s.len() - 1].trim().to_string())
                } else {
                    None
                }
            }) {
                classes.push(ClassDef { name, parent: None });
            } else if let Some(body) = trimmed.strip_prefix("class ") {
                // `class Name : Parent {` or `class Name {`
                let name = if body.contains(':') {
                    body.split(':').next().unwrap_or("").trim().to_string()
                } else {
                    body.split('{').next().unwrap_or("").trim().to_string()
                };
                let parent = if body.contains(':') {
                    let after = body.split(':').nth(1).unwrap_or("");
                    let p = after.split('{').next().unwrap_or("").trim();
                    if p.is_empty() {
                        None
                    } else {
                        Some(p.to_string())
                    }
                } else {
                    None
                };
                if !name.is_empty() {
                    classes.push(ClassDef { name, parent });
                }
            }
        }

        brace_depth -= closes as i32;
        if brace_depth < 0 {
            break;
        }
    }

    classes
}

// ── MagazineWell extraction ─────────────────────────────────────────────────────

/// Extract magazineWell[] values from a class body in config text.
/// Handles both `magazineWell[] = {X};` and `magazineWell[] += {X};`.
fn extract_magwells_from_config(config: &str, class_name: &str) -> Option<Vec<String>> {
    let mut in_class = false;
    let mut brace_depth: i32 = -1;
    let mut magwells: Option<Vec<String>> = None;
    let search1 = format!("class {} ", class_name);
    let search2 = format!("class {}", class_name);
    // Also accept forward decls as empty
    for line in config.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }

        if !in_class {
            if trimmed.starts_with(&search1) || trimmed.starts_with(&search2) {
                // Skip forward declarations
                if trimmed.ends_with(';') && !trimmed.contains('{') {
                    // Found forward decl → no definition body
                    // Mark as known but empty
                    continue;
                }
                if trimmed.ends_with('{') || trimmed.contains('{') {
                    in_class = true;
                    let opens: usize = trimmed.matches('{').count();
                    let closes: usize = trimmed.matches('}').count();
                    brace_depth = (opens - closes) as i32;
                    magwells = Some(Vec::new());
                }
            }
            continue;
        }

        // Track brace depth
        let opens: usize = trimmed.matches('{').count();
        let closes: usize = trimmed.matches('}').count();
        let depth_before = brace_depth;
        brace_depth += opens as i32;

        // Check for magazineWell[] assignment at the class body depth
        if depth_before >= 0 && trimmed.contains("magazineWell[]") {
            // Extract values between { and }
            if let Some(vals_start) = trimmed.find('{') {
                let after = &trimmed[vals_start + 1..];
                if let Some(vals_end) = after.find('}') {
                    let contents = &after[..vals_end];
                    for token in contents.split(',') {
                        let val = token.trim().trim_matches('"');
                        if !val.is_empty() {
                            if let Some(ref mut mw) = magwells {
                                mw.push(val.to_string());
                            }
                        }
                    }
                }
            }
        }

        brace_depth -= closes as i32;
        if brace_depth <= 0 || in_class && brace_depth < 0 {
            break;
        }
    }

    magwells
}

/// Detect weapons where a child class adds magazineWell values not present
/// in its parent chain (indicating multi-caliber capability).
fn detect_multi_caliber(
    configs: &[&String],
    parents: &HashMap<String, Option<String>>,
) -> Vec<(String, String, Vec<String>, Vec<String>)> {
    // result: (child, parent, child_magwells, parent_magwells)
    let mut results = Vec::new();

    // Collect all child classes that have a parent in our map
    let children: Vec<(String, String)> = parents
        .iter()
        .filter_map(|(name, parent_opt)| parent_opt.as_ref().map(|p| (name.clone(), p.clone())))
        .collect();

    // For each child-parent pair, extract and compare magwells
    for (child, parent) in &children {
        let mut child_mws: Option<Vec<String>> = None;
        let mut parent_mws: Option<Vec<String>> = None;

        for cfg in configs {
            if child_mws.is_none() {
                child_mws = extract_magwells_from_config(cfg, child);
            }
            if parent_mws.is_none() {
                parent_mws = extract_magwells_from_config(cfg, &parent);
            }
            if child_mws.is_some() && parent_mws.is_some() {
                break;
            }
        }

        let child_set: HashSet<String> = child_mws
            .as_ref()
            .map(|v| v.iter().cloned().collect())
            .unwrap_or_default();
        let parent_set: HashSet<String> = parent_mws
            .as_ref()
            .map(|v| v.iter().cloned().collect())
            .unwrap_or_default();

        // Child adds magwells the parent doesn't have → multi-caliber capable
        let added: Vec<String> = child_set.difference(&parent_set).cloned().collect();
        if !added.is_empty() {
            results.push((
                child.clone(),
                parent.clone(),
                added,
                parent_set.into_iter().collect(),
            ));
        }
    }

    results
}

// ── String table (stringtable.xml) parser ─────────────────────────────────────

/// Extract stringtable.xml from a PBO if it exists.
fn pbo_stringtable(pbo: &Path) -> Option<HashMap<String, String>> {
    let output = Command::new("armake")
        .arg("cat")
        .arg(pbo)
        .arg("stringtable.xml")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    let xml = String::from_utf8(output.stdout).ok()?;
    if xml.trim().is_empty() || !xml.starts_with("<?xml") {
        return None;
    }
    Some(parse_stringtable(&xml))
}

// ── Display name extraction from config.cpp ─────────────────────────────────

/// Extract `displayName` value for a specific weapon class from config text.
/// Returns the raw value (literal string or $STR_* reference).
/// Extract the body of a named section (e.g. `class CfgWeapons { ... }`)
/// from a derapified config string.
fn extract_section_body<'a>(config: &'a str, section: &'a str) -> Option<&'a str> {
    let search_prefix = format!("class {} ", section);
    let search_prefix2 = format!("class {}", section);
    let mut start: Option<usize> = None;
    let mut brace_depth: i32 = -1;

    for (i, line) in config.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }
        if start.is_none() {
            if trimmed.starts_with(&search_prefix) || trimmed.starts_with(&search_prefix2) {
                if trimmed.ends_with('{') || trimmed.contains('{') {
                    start = Some(i);
                    let opens: usize = trimmed.matches('{').count();
                    let closes: usize = trimmed.matches('}').count();
                    brace_depth = (opens - closes) as i32;
                    continue;
                }
            }
        } else {
            let opens: usize = trimmed.matches('{').count();
            let closes: usize = trimmed.matches('}').count();
            brace_depth += opens as i32;
            brace_depth -= closes as i32;
            if brace_depth < 0 {
                return None; // premature close
            }
            if brace_depth == 0 && closes > 0 {
                // End of section
                let whole = config.lines().collect::<Vec<&str>>();
                // Find the byte range for lines [start..=i]
                let byte_start = whole[..start.unwrap()]
                    .iter()
                    .map(|l| l.as_bytes().len() + 1) // +1 for \n
                    .sum::<usize>();
                let byte_end = whole[..=i]
                    .iter()
                    .map(|l| l.as_bytes().len() + 1)
                    .sum::<usize>();
                return Some(&config[byte_start..byte_end]);
            }
        }
    }
    None
}

/// Extract displayName for a class from within the CfgWeapons section body.
fn extract_display_name(cfgweapons_body: &str, class_name: &str) -> Option<String> {
    let search_patterns = [
        format!("class {} :", class_name),
        format!("class {} {{", class_name),
        format!("class {}", class_name),
    ];

    let mut start_pos = None;
    for pat in &search_patterns {
        if let Some(pos) = cfgweapons_body.find(pat.as_str()) {
            start_pos = Some(pos);
            break;
        }
    }
    let pos = start_pos?;

    let remaining = &cfgweapons_body[pos..];
    let mut depth: i32 = -1;
    let mut seen_open = false;

    for line in remaining.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }

        let opens: usize = trimmed.matches('{').count();
        let closes: usize = trimmed.matches('}').count();

        if !seen_open && opens > 0 {
            seen_open = true;
            depth = (opens - closes) as i32;
            continue;
        }
        if !seen_open {
            continue;
        }

        // At depth 1 (direct field of this class, not nested)
        if depth == 1 {
            if let Some(val) = trimmed
                .strip_prefix("displayName")
                .and_then(|s| s.trim().strip_prefix('='))
            {
                let val = val.trim().trim_end_matches(';').trim();
                if (val.starts_with('"') && val.ends_with('"')) || val.starts_with("$STR_") {
                    return Some(val.trim_matches('"').to_string());
                }
            }
            // Also check for scope = 1; (non-public inherit-only classes)
            // Skip class forward declarations (; termination, no {)
        }

        depth -= closes as i32;
        if depth < 0 {
            break;
        }
        depth += opens as i32;
    }

    None
}

/// Parse a Stringtable.xml into a HashMap of STR_ID → English text.
fn parse_stringtable(xml: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    // Simple parser: find `<Key ID="...">` and `<English>...</English>`
    let mut current_key: Option<String> = None;
    for line in xml.lines() {
        let trimmed = line.trim();
        if let Some(key) = trimmed.strip_prefix("<Key ID=\"") {
            if let Some(end) = key.find('"') {
                current_key = Some(key[..end].to_string());
            }
        } else if let Some(key) = trimmed.strip_prefix("<Key ID='") {
            if let Some(end) = key.find('\'') {
                current_key = Some(key[..end].to_string());
            }
        }
        if let Some(ref key) = current_key {
            if let Some(val) = trimmed.strip_prefix("<English>") {
                if let Some(end) = val.find("</English>") {
                    let text = &val[..end];
                    map.insert(key.clone(), text.to_string());
                }
            }
        }
        // Reset key on closing Key tag
        if trimmed.starts_with("</Key") || trimmed.contains("/>") {
            current_key = None;
        }
    }
    map
}

/// Resolve a displayName value: literal strings pass through, $STR_* refs
/// are looked up in the string table map. Unresolvable $STR_* refs
/// (e.g. vanilla Arma `$STR_A3_*` — tables compiled into the engine)
/// fall back to the class name.
fn resolve_display_name(raw: &str, strtable: &HashMap<String, String>, class_name: &str) -> String {
    if raw.starts_with("$STR_") {
        let key = raw.trim_matches('"');
        strtable
            .get(key)
            .cloned()
            .unwrap_or_else(|| class_name.to_string())
    } else {
        raw.to_string()
    }
}

// ── Armake interface ──────────────────────────────────────────────────────────

fn armake_available() -> bool {
    Command::new("armake")
        .arg("--help")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn extract_pbo_config(pbo: &Path) -> Option<String> {
    let cat_output = Command::new("armake")
        .arg("cat")
        .arg(pbo)
        .arg("config.bin")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())?;

    let mut derapify = Command::new("armake")
        .arg("derapify")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;

    use std::io::Write;
    let stdin = derapify.stdin.as_mut().unwrap();
    stdin.write_all(&cat_output.stdout).ok()?;
    drop(derapify.stdin.take());

    let output = derapify.wait_with_output().ok()?;
    if output.status.success() {
        String::from_utf8(output.stdout).ok()
    } else {
        None
    }
}

fn find_pbos(dir: &Path) -> Vec<PathBuf> {
    let mut pbos = Vec::new();
    if !dir.is_dir() {
        return pbos;
    }
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        if let Ok(entries) = std::fs::read_dir(&current) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().is_some_and(|e| e == "pbo") {
                    pbos.push(path);
                }
            }
        }
    }
    pbos.sort();
    pbos
}

// ── Test ──────────────────────────────────────────────────────────────────────

#[test]
fn pbo_weapon_and_ammo_coverage() {
    // ── Prerequisites ─────────────────────────────────────────────────────────
    if !armake_available() {
        eprintln!("SKIP: armake not found in PATH");
        return;
    }

    let pbo_dir = std::env::var("ABE_PBO_DIR")
        .unwrap_or_else(|_| "/ext/SteamLibrary/steamapps/common/Arma 3/Addons".to_string());
    let pbo_path = Path::new(&pbo_dir);

    let pbos = find_pbos(pbo_path);
    if pbos.is_empty() {
        eprintln!("SKIP: no .pbo files found in {pbo_dir}");
        return;
    }

    eprintln!(
        "PBO Coverage Test: scanning {} PBOs in {}",
        pbos.len(),
        pbo_dir
    );

    // ── Extraction ────────────────────────────────────────────────────────────
    let mut all_weapon_parents: HashMap<String, Option<String>> = HashMap::new();
    let mut all_ammo_classes: HashSet<String> = HashSet::new();
    let mut total_pbos_with_data = 0u32;
    // Collect config texts for multi-caliber scanning and display name lookup
    let mut weapon_configs: Vec<String> = Vec::new();
    // Collect display names per class (resolved from $STR_* vs literal)
    // (populated inline in the PER-WEAPON section from config texts + strtable)
    // Global string table from all PBOs
    let mut strtable: HashMap<String, String> = HashMap::new();

    for pbo in &pbos {
        let pbo_name = pbo
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let config = match extract_pbo_config(pbo) {
            Some(c) => c,
            None => {
                continue;
            },
        };

        let has_weapons = config.contains("class CfgWeapons ");
        let has_ammo = config.contains("class CfgAmmo ");
        let has_vehicles = config.contains("class CfgVehicles ");
        let has_section = has_weapons || has_ammo;

        let mut weap_count = 0u32;
        let mut ammo_count = 0u32;

        if has_weapons {
            let defs = extract_section_classes(&config, "CfgWeapons");
            for d in &defs {
                if !d.name.is_empty() {
                    let prev = all_weapon_parents.insert(d.name.clone(), d.parent.clone());
                    if prev.is_none() {
                        weap_count += 1;
                    }
                }
            }
            weapon_configs.push(config.clone());
        }

        if has_ammo {
            let defs = extract_section_classes(&config, "CfgAmmo");
            for d in &defs {
                if !d.name.is_empty() && all_ammo_classes.insert(d.name.clone()) {
                    ammo_count += 1;
                }
            }
        }

        // Collect string table for display name resolution
        if let Some(tables) = pbo_stringtable(pbo) {
            let before = strtable.len();
            strtable.extend(tables);
            if strtable.len() > before {
                eprintln!("  [    +{} str] {pbo_name}", strtable.len() - before);
            }
        }

        if has_section {
            eprintln!("  [{weap_count:3}W/{ammo_count:3}A] {pbo_name}");
            total_pbos_with_data += 1;
        } else if has_vehicles {
            eprintln!("  [  vehicles] {pbo_name}");
        }
    }

    eprintln!("{} PBOs with CfgWeapons/CfgAmmo data", total_pbos_with_data);

    // ── Classify each class by walking its inheritance chain ──────────────
    let all_weapon_names: Vec<String> = all_weapon_parents.keys().cloned().collect();

    let mut classified_weapons: Vec<String> = Vec::new();
    let mut vehicle_weapons: Vec<String> = Vec::new();
    let mut accessory_count = 0u32;
    let mut framework_count = 0u32;
    let mut unknown_count = 0u32;

    for w in &all_weapon_names {
        match classify_class(w, &all_weapon_parents) {
            ClassKind::Weapon => classified_weapons.push(w.clone()),
            ClassKind::VehicleWeapon => vehicle_weapons.push(w.clone()),
            ClassKind::Accessory => accessory_count += 1,
            ClassKind::Framework => framework_count += 1,
            ClassKind::Unknown => {
                unknown_count += 1;
                let mut chain = w.clone();
                let mut cur: &str = w;
                let mut count = 0usize;
                while let Some(Some(parent)) = all_weapon_parents.get(cur) {
                    if count > 10 {
                        chain += &format!(" → ...");
                        break;
                    }
                    chain += &format!(" → {parent}");
                    cur = parent;
                    count += 1;
                }
                eprintln!("  [?] {chain}");
            },
        }
    }

    // ── Print full classification lists for human review ────────────────
    classified_weapons.sort();
    vehicle_weapons.sort();

    eprintln!();
    eprintln!("── Small arms (Rifle/Pistol/Launcher chain) ──");
    for w in &classified_weapons {
        eprintln!("  {w}");
    }

    eprintln!("\n── Vehicle weapons (MGun/Cannon/Mortar chain) ──");
    for w in &vehicle_weapons {
        eprintln!("  {w}");
    }

    // Print first 30 accessories so the user can verify they're not weapons
    eprintln!("\n── First 30 accessories/items (of {accessory_count}) ──");
    let mut acc_list: Vec<&String> = all_weapon_names
        .iter()
        .filter(|w| matches!(classify_class(w, &all_weapon_parents), ClassKind::Accessory))
        .collect();
    acc_list.sort();
    for w in acc_list.iter().take(30) {
        eprintln!("  {w}");
    }

    eprintln!(
        "\nClassified: {} small arms, {} vehicle weapons, {} accessories/items, {} framework bases, {} unknown",
        classified_weapons.len(),
        vehicle_weapons.len(),
        accessory_count,
        framework_count,
        unknown_count,
    );
    let strtable_ref = &strtable; // borrow for closure

    // ── Resolve display names for all weapons ───────────────────────────
    // Search each config text for displayName field.
    let mut display_for: HashMap<String, String> = HashMap::new();
    for w in classified_weapons.iter().chain(vehicle_weapons.iter()) {
        for cfg in &weapon_configs {
            let cfgw_body = extract_section_body(cfg, "CfgWeapons");
            let body = cfgw_body.unwrap_or(cfg);
            if let Some(raw) = extract_display_name(body, w) {
                let resolved = resolve_display_name(&raw, strtable_ref, w);
                display_for.insert(w.clone(), resolved);
                break;
            }
        }
    }

    // ── Print per-weapon verification report ────────────────────────────
    eprintln!();
    eprintln!("═══ Per-Weapon PHF Verification ═══");
    for w in classified_weapons.iter() {
        let result = ir_lookup::resolve_weapon(w, 0.0);
        let dn = display_for.get(w).map(|s| s.as_str()).unwrap_or("?");
        let matched = std::str::from_utf8(&result.matched_model)
            .unwrap_or("?")
            .trim_end_matches('\0');
        let p = &result.params;
        let flag = if matched == "override" {
            "*"
        } else if result.confidence >= 0.9 {
            "✓"
        } else if result.confidence >= 0.7 {
            "~"
        } else {
            "✗"
        };
        let barrel_match = if (100.0..600.0).contains(&p.barrel_length_mm) {
            format!("{}mm", p.barrel_length_mm as u32)
        } else {
            format!("{:.0}mm", p.barrel_length_mm)
        };
        let cal_str = if p.caliber_mm > 0.0 {
            format!("{:.1}mm", p.caliber_mm)
        } else {
            "?".to_string()
        };
        eprintln!(
            "  {flag} {dn:<35} ({w:<45}) → PHF {matched:<16} ({barrel_match}, {cal_str}, c={:.2})",
            result.confidence
        );
    }
    for w in vehicle_weapons.iter() {
        let result = ir_lookup::resolve_weapon(w, 0.0);
        let dn = display_for.get(w).map(|s| s.as_str()).unwrap_or("?");
        let matched = std::str::from_utf8(&result.matched_model)
            .unwrap_or("?")
            .trim_end_matches('\0');
        let p = &result.params;
        let flag = if matched == "override" {
            "*"
        } else if result.confidence >= 0.9 {
            "✓"
        } else if result.confidence >= 0.7 {
            "~"
        } else {
            "✗"
        };
        let barrel_str = if p.barrel_length_mm > 0.0 {
            format!("{:.0}mm", p.barrel_length_mm)
        } else {
            "N/A".to_string()
        };
        eprintln!(
            "  {flag} {dn:<35} ({w:<45}) → PHF {matched:<16} ({barrel_str}, c={:.2})",
            result.confidence
        );
    }
    eprintln!("(✓ exact match, ~ caliber mismatch, ✗ low confidence)");
    eprintln!("(* SQF override — ACE3 or mod-provided values)");

    // ── Run resolve_weapon on small arms + vehicle weapons ──────────────
    let all_weapons_to_test: Vec<&String> = classified_weapons
        .iter()
        .chain(vehicle_weapons.iter())
        .collect();
    let total_weapons_to_test = all_weapons_to_test.len();

    let mut weapon_hits = 0usize;
    let mut weapon_misses: Vec<String> = Vec::new();

    for w in &all_weapons_to_test {
        let result = ir_lookup::resolve_weapon(w, 0.0);
        if result.confidence > 0.0 {
            weapon_hits += 1;
        } else {
            weapon_misses.push(w.to_string());
        }
    }

    // ── Run resolve_ammo ────────────────────────────────────────────────
    let ammo_names: Vec<&String> = all_ammo_classes.iter().collect();
    let mut ammo_hits = 0usize;
    let mut ammo_misses: Vec<String> = Vec::new();

    for a in &ammo_names {
        let (result, _conf) = ir_lookup::resolve_ammo(a);
        if result.bullet_diameter_mm > 0.0 && result.projectile_mass_g > 0.0 {
            ammo_hits += 1;
        } else {
            ammo_misses.push((*a).clone());
        }
    }

    // ── Per-category PHF hit counts ─────────────────────────────────────
    let sa_hits = classified_weapons
        .iter()
        .filter(|w| ir_lookup::resolve_weapon(w, 0.0).confidence > 0.0)
        .count();
    let vw_hits = vehicle_weapons
        .iter()
        .filter(|w| ir_lookup::resolve_weapon(w, 0.0).confidence > 0.0)
        .count();

    // ── Report ──────────────────────────────────────────────────────────
    let weapon_coverage = if total_weapons_to_test == 0 {
        100.0
    } else {
        weapon_hits as f64 / total_weapons_to_test as f64 * 100.0
    };
    let ammo_coverage = if all_ammo_classes.is_empty() {
        100.0
    } else {
        ammo_hits as f64 / all_ammo_classes.len() as f64 * 100.0
    };

    eprintln!();
    eprintln!("═══ PBO Coverage Report ═══");
    eprintln!("PBOs scanned:                {}", pbos.len());
    eprintln!("PBOs with weapon/ammo data:  {}", total_pbos_with_data);
    eprintln!("Raw classes in CfgWeapons:   {}", all_weapon_names.len());
    eprintln!(
        "  Small arms (Rifle/Pistol/Launcher): {}",
        classified_weapons.len()
    );
    eprintln!(
        "  Vehicle weapons (MGun/Cannon/Mortar): {}",
        vehicle_weapons.len()
    );
    eprintln!("  Accessories/items:         {accessory_count}");
    eprintln!("  Framework bases:           {framework_count}");
    eprintln!("  Unclassifiable:            {unknown_count}");
    eprintln!();
    eprintln!(
        "PHF total: {weapon_hits}/{total_weapons_to_test} ({:.1}%)",
        weapon_coverage
    );
    eprintln!(
        "  Small arms:   {sa_hits}/{} ({:.1}%)",
        classified_weapons.len(),
        if classified_weapons.is_empty() {
            100.0
        } else {
            sa_hits as f64 / classified_weapons.len() as f64 * 100.0
        }
    );
    eprintln!(
        "  Vehicle wpns: {vw_hits}/{} ({:.1}%)",
        vehicle_weapons.len(),
        if vehicle_weapons.is_empty() {
            100.0
        } else {
            vw_hits as f64 / vehicle_weapons.len() as f64 * 100.0
        }
    );
    eprintln!(
        "Ammo:           {ammo_hits}/{} ({:.1}%)",
        all_ammo_classes.len(),
        ammo_coverage
    );
    eprintln!(
        "PHF misses: {} weapons / {} ammo",
        weapon_misses.len(),
        ammo_misses.len()
    );

    if !weapon_misses.is_empty() {
        eprintln!("\n── Missed weapons (first 50) ──");
        for w in weapon_misses.iter().take(50) {
            let mut chain = w.clone();
            let mut cur: &str = w;
            let mut count = 0usize;
            while let Some(Some(parent)) = all_weapon_parents.get(cur) {
                if count > 20 {
                    chain += " → ...";
                    break;
                }
                chain += &format!(" → {parent}");
                cur = parent;
                count += 1;
            }
            eprintln!("  {chain}");
        }
    }

    if !ammo_misses.is_empty() {
        eprintln!("\n── Missed ammo (first 30) ──");
        for a in ammo_misses.iter().take(30) {
            eprintln!("  {a}");
        }
    }

    // ── Multi-caliber detection ───────────────────────────────────────────────
    let config_refs: Vec<&String> = weapon_configs.iter().collect();
    let multi_cal = detect_multi_caliber(&config_refs, &all_weapon_parents);

    if !multi_cal.is_empty() {
        eprintln!("\n── Multi-caliber capable weapons (child adds magwells) ──");
        for (child, parent, added, parent_mws) in &multi_cal {
            eprintln!(
                "  {child} (→ {parent}) adds: [{}] over parent [{}]",
                added.join(", "),
                parent_mws.join(", ")
            );
        }
    }

    assert!(
        weapon_coverage >= MIN_WEAPON_COVERAGE,
        "Weapon coverage {:.1}% < {:.0}% threshold — {} PHF misses ({} small arms + {} vehicle weapons)",
        weapon_coverage,
        MIN_WEAPON_COVERAGE,
        weapon_misses.len(),
        classified_weapons.len(),
        vehicle_weapons.len(),
    );

    assert!(
        ammo_coverage >= MIN_AMMO_COVERAGE,
        "Ammo coverage {:.1}% < {:.0}% threshold — {} PHF misses",
        ammo_coverage,
        MIN_AMMO_COVERAGE,
        ammo_misses.len()
    );

    eprintln!(
        "\nCoverage OK: weapons {:.1}%, ammo {:.1}%",
        weapon_coverage, ammo_coverage
    );
}
