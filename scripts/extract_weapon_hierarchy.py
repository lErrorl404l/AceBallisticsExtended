#!/usr/bin/env python3
"""
Extract Arma 3 weapon class hierarchy from derapified config files.

Parses config.hpp and individual config.bin files from Arma 3 PBOs to build:
  - A JSON file with the full weapon class tree
  - Weapon type classification (arifle, srifle, hgun, smg, etc.)
  - Parent-child relationships between weapon classes

Usage:
    python scripts/extract_weapon_hierarchy.py
"""

import json
import os
import re
import subprocess
import sys
from collections import defaultdict

# ── Paths ────────────────────────────────────────────────────────────
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
PROJECT_DIR = os.path.normpath(os.path.join(SCRIPT_DIR, ".."))
DATA_DIR = os.path.join(PROJECT_DIR, "data")
REFERENCE_DIR = os.path.join(DATA_DIR, "reference")

MAIN_CONFIG = "/tmp/pbo_all/weapons_f_main.hpp"
PBO_ALL_DIR = "/tmp/pbo_all/"
DERAPIFIED_DIR = "/tmp/derapified/"
OUTPUT_JSON = os.path.join(REFERENCE_DIR, "weapon_hierarchy.json")
IR_WEAPONS_TSV = os.path.join(DATA_DIR, "ir_weapons.tsv")

ARMAGE_BIN = "armake"  # should be on PATH

# ── Weapon type classification by name prefix ────────────────────────
PREFIX_TO_TYPE = [
    ("arifle_", "arifle"),
    ("srifle_", "srifle"),
    ("hgun_", "hgun"),
    ("smg_", "smg"),
    ("SMG_", "smg"),
    ("lmg_", "lmg"),
    ("LMG_", "lmg"),
    ("mmg_", "mmg"),
    ("MMG_", "mmg"),
    ("HMG_", "lmg"),  # HMGs are mounted MGs in A3 CfgWeapons
    ("dmr_", "dmr"),
    ("DMR_", "dmr"),
    ("sgun_", "sgun"),
    ("launch_", "launch"),
    ("pdw_", "pdw"),
]

# Parent-based type inference (fallback when prefix is ambiguous)
PARENT_TO_TYPE = {
    "Rifle_Base_F": "arifle",
    "Rifle_Short_Base_F": "smg",
    "Rifle_Long_Base_F": "srifle",
    "HandGunBase": "hgun",
    "Launcher_Base_F": "launch",
    "Pistol_Base_F": "hgun",
    "Pistol": "hgun",
}

# Config sections that indicate a top-level CfgWeapons block
CFG_WEAPONS_OPEN = re.compile(r"^\s*class\s+CfgWeapons\s*\{")
CFG_WEAPONS_CLOSE = re.compile(r"^\s*\};")

# Class definition pattern: class ClassName: ParentClass {
CLASS_DEF = re.compile(r"^\s*class\s+(\w+)\s*:\s*(\w+)\s*\{")

# Forward declaration: class ClassName;  or  class ClassName: ParentClass;
CLASS_FWD = re.compile(r"^\s*class\s+(\w+)\s*(?::\s*(\w+))?\s*;")

# Class close: };
CLASS_CLOSE = re.compile(r"^\s*\};")

# Class open with body: class ClassName { ...  (no parent)
CLASS_OPEN_NO_PARENT = re.compile(r"^\s*class\s+(\w+)\s*\{")

# Extract class names from Weapon_* containers (e.g., class arifle_Katiba_F { weapon = "arifle_Katiba_F"; };)
# weapon = "ClassName";
WEAPON_ASSIGN = re.compile(r'weapon\s*=\s*"(\w+)";')
# parenthesized path
MODEL_PATH = re.compile(r'model\s*=\s*"[^"]*";')
# magazine reference
MAGAZINE_REF = re.compile(r"magazines\[\]\s*=\s*\{[^}]*\};", re.DOTALL)

# ── Helpers ──────────────────────────────────────────────────────────


def classify_weapon(class_name: str, parent: str | None) -> str:
    """Classify a weapon class name into a type."""
    # Skip known base classes
    if class_name in (
        "Rifle_Base_F",
        "Rifle_Short_Base_F",
        "Rifle_Long_Base_F",
        "Launcher_Base_F",
        "HandGunBase",
        "Pistol_Base_F",
        "Pistol",
        "Rifle",
        "RifleCore",
        "Launcher",
        "LauncherCore",
        "Default",
        "Weapon_Base_F",
        "PistolCore",
        "GrenadeLauncher",
        "UGL_F",
        "GrenadeCore",
        "WeaponHolder",
        "SlotInfo",
    ):
        return "base"
    # Skip Weapon_* container classes (editor placement proxies)
    if (
        class_name.startswith("Weapon_")
        or class_name.startswith("Item_")
        or class_name.startswith("Headgear_")
        or class_name.startswith("Vest_")
    ):
        return "base"
    for prefix, wtype in PREFIX_TO_TYPE:
        if class_name.startswith(prefix):
            return wtype
    if parent:
        for base, wtype in PARENT_TO_TYPE.items():
            if parent == base or parent.endswith(base):
                return wtype
    return "unknown"


def find_all_config_bins(base_dir: str) -> list[str]:
    """Find all config.bin files under weapons_f directories."""
    bins = []
    for root, dirs, files in os.walk(base_dir):
        for f in files:
            if f == "config.bin":
                # Skip top-level config.bin that's already in the main merge
                full = os.path.join(root, f)
                # Only include sub-directory configs (individual PBO weapons)
                relative = os.path.relpath(full, base_dir)
                if "/" in relative:
                    bins.append(full)
    return sorted(bins)


def derapify_config(bin_path: str, out_dir: str) -> str | None:
    """Derapify a config.bin to a .hpp file. Returns path to output or None."""
    parts = bin_path.replace(PBO_ALL_DIR, "").split("/")
    safe_name = "_".join(parts).replace(".bin", ".hpp")
    out_path = os.path.join(out_dir, safe_name)

    if os.path.exists(out_path):
        return out_path  # Already derapified

    os.makedirs(os.path.dirname(out_path), exist_ok=True)

    try:
        result = subprocess.run(
            [ARMAGE_BIN, "derapify", bin_path, out_path],
            capture_output=True,
            text=True,
            timeout=30,
        )
        if result.returncode == 0 and os.path.exists(out_path):
            return out_path
        else:
            print(
                f"  WARNING: armake derapify failed for {bin_path}: {result.stderr.strip()}"
            )
            return None
    except (subprocess.TimeoutExpired, FileNotFoundError) as e:
        print(f"  WARNING: armake error for {bin_path}: {e}")
        return None


def parse_config_for_classes(content: str) -> dict[str, dict]:
    """
    Parse a derapified config file for class definitions in CfgWeapons blocks.

    Returns dict of class_name -> {"parent": parent_name or None, "file": source}
    """
    classes = {}
    lines = content.split("\n")

    in_cfg_weapons = False
    brace_depth = 0
    current_class = None
    current_parent = None
    current_body_depth = 0

    for i, line in enumerate(lines):
        stripped = line.strip()

        # Detect CfgWeapons blocks
        if CFG_WEAPONS_OPEN.match(stripped):
            in_cfg_weapons = True
            brace_depth = 1
            continue

        if not in_cfg_weapons:
            # Also scan outside CfgWeapons for class definitions
            m = CLASS_DEF.match(stripped)
            if m:
                cls_name = m.group(1)
                parent = m.group(2)
                classes[cls_name] = {"parent": parent}
            continue

        # Inside CfgWeapons - track brace depth
        for ch in stripped:
            if ch == "{":
                brace_depth += 1
            elif ch == "}":
                brace_depth -= 1

        if brace_depth <= 0:
            in_cfg_weapons = False
            continue

        # Class definition: class ClassName: ParentClass {
        m = CLASS_DEF.match(stripped)
        if m:
            cls_name = m.group(1)
            parent = m.group(2)
            # Skip non-weapon class types
            if cls_name.startswith("Mode_") or cls_name in (
                "SlotInfo",
                "MuzzleSlot",
                "CowsSlot",
                "PointerSlot",
                "MuzzleSlot_Rail",
                "CowsSlot_Rail",
                "PointerSlot_Rail",
                "UnderBarrelSlot_rail",
            ):
                continue
            classes[cls_name] = {"parent": parent}
            continue

        # Forward declaration: class ClassName;
        m = CLASS_FWD.match(stripped)
        if m:
            cls_name = m.group(1)
            parent = m.group(2)
            classes[cls_name] = {"parent": parent}
            continue

    return classes


def extract_cfg_patches_weapons(content: str) -> set[str]:
    """Extract weapon class names from CfgPatches weapons[] array."""
    weapons = set()
    # Find weapons[] array
    m = re.search(r"weapons\[\]\s*=\s*\{(.*?)\};", content, re.DOTALL)
    if m:
        arr_content = m.group(1)
        # Extract string literals
        for w in re.findall(r'"(\w+)"', arr_content):
            weapons.add(w)
    return weapons


def parse_individual_config(file_path: str) -> dict[str, dict]:
    """
    Parse a derapified individual weapon config file.
    Returns dict of class_name -> {"parent": parent_name}.
    Focused on weapon-specific classes, not config infrastructure.
    """
    with open(file_path) as f:
        content = f.read()

    classes = {}
    lines = content.split("\n")

    in_cfg_weapons = False
    brace_depth = 0

    for line in lines:
        stripped = line.strip()

        if CFG_WEAPONS_OPEN.match(stripped):
            in_cfg_weapons = True
            brace_depth = 1
            continue

        if not in_cfg_weapons:
            continue

        # Track braces
        for ch in stripped:
            if ch == "{":
                brace_depth += 1
            elif ch == "}":
                brace_depth -= 1

        if brace_depth <= 0:
            in_cfg_weapons = False
            continue

        # Class definition: class ClassName: ParentClass {
        m = CLASS_DEF.match(stripped)
        if m:
            cls_name = m.group(1)
            parent = m.group(2)
            # Skip slot/UI infrastructure
            if any(
                skip in cls_name for skip in ("Slot", "Muzzle", "Rail", "UnderBarrel")
            ):
                continue
            classes[cls_name] = {"parent": parent}
            continue

        # Forward declaration
        m = CLASS_FWD.match(stripped)
        if m:
            cls_name = m.group(1)
            parent = m.group(2)
            if cls_name not in classes:
                classes[cls_name] = {"parent": parent}
            continue

    # Also extract from CfgPatches weapons[] array
    patch_weapons = extract_cfg_patches_weapons(content)
    for w in patch_weapons:
        if w not in classes:
            classes[w] = {"parent": None}

    return classes


def build_full_hierarchy(
    classes: dict[str, dict],
) -> tuple[dict, dict, list, set, dict]:
    """
    Build the full weapon hierarchy.

    Returns:
        inheritance: dict of class -> {"parent": parent, "type": weapon_type}
        by_type: dict of type -> [class_names]
        all_weapon_classes: list of all known weapon class names
        base_classes: set of base class names
    """
    # Build parent lookup
    parent_of = {}
    for cls_name, info in classes.items():
        parent_of[cls_name] = info.get("parent")

    # Classify each class
    inheritance = {}
    by_type = defaultdict(list)
    all_weapon_classes = []
    base_classes = set()

    for cls_name, info in classes.items():
        parent = info.get("parent")
        wtype = classify_weapon(cls_name, parent)

        inheritance[cls_name] = {
            "parent": parent,
            "type": wtype,
        }

        if wtype != "unknown":
            all_weapon_classes.append(cls_name)
            by_type[wtype].append(cls_name)
        else:
            # Could be a base class
            if parent is None:
                base_classes.add(cls_name)
            else:
                # Check if parent chain leads to a known weapon type
                chain = resolve_inheritance_chain(cls_name, parent_of)
                inferred_type = None
                for ancestor in chain:
                    atype = classify_weapon(ancestor, parent_of.get(ancestor))
                    if atype != "unknown":
                        inferred_type = atype
                        break

                if inferred_type:
                    inheritance[cls_name]["type"] = inferred_type
                    all_weapon_classes.append(cls_name)
                    by_type[inferred_type].append(cls_name)
                else:
                    base_classes.add(cls_name)

    return inheritance, dict(by_type), all_weapon_classes, base_classes, parent_of


def resolve_inheritance_chain(class_name: str, parent_of: dict) -> list[str]:
    """Resolve the full inheritance chain for a class."""
    chain = [class_name]
    current = class_name
    seen = set()
    while current in parent_of and parent_of[current]:
        current = parent_of[current]
        if current in seen:
            break  # avoid cycles
        seen.add(current)
        chain.append(current)
    return chain


def build_tree(inheritance: dict, parent_of: dict) -> dict:
    """Build a nested tree structure from the flat inheritance."""
    tree = {}

    # Find root classes (no parent or parent not in our data)
    roots = set()
    for cls_name, info in inheritance.items():
        parent = info.get("parent")
        if not parent or parent not in inheritance:
            roots.add(cls_name)

    # Build children lookup
    children_of = defaultdict(list)
    for cls_name, info in inheritance.items():
        parent = info.get("parent")
        if parent:
            children_of[parent].append(cls_name)

    def build_subtree(node_name: str) -> dict:
        node = {
            "class": node_name,
            "type": inheritance.get(node_name, {}).get("type", "unknown"),
        }
        kids = children_of.get(node_name, [])
        if kids:
            node["children"] = [build_subtree(k) for k in sorted(kids)]
        return node

    # Build tree from each root
    result = {}
    for root in sorted(roots):
        if root in children_of or root in inheritance:
            result[root] = build_subtree(root)

    return result


def check_mod_compatibility(
    weapon_class: str, inheritance: dict, parent_of: dict
) -> str | None:
    """
    Check if a TSV weapon model can be matched to an Arma class.
    Returns the matched class name or None.
    """
    # Direct match
    if weapon_class in inheritance:
        return weapon_class

    # Try stripping common suffixes that denote attachment variants
    for suffix in [
        "_snds",
        "_pointer",
        "_ACO",
        "_Holo",
        "_Hamr",
        "_ARCO",
        "_DMS",
        "_RCO",
        "_SOS",
        "_LRPS",
        "_NVS",
        "_IR",
        "_FL",
        "_FL_pointer",
        "_FL_ACO",
        "_FL_snds",
        "_point_snds",
        "_pointer_snds",
        "_ACO_pointer",
        "_ACO_pointer_snds",
        "_Holo_pointer",
        "_Holo_pointer_snds",
        "_Hamr_pointer",
        "_RCO_pointer_snds",
        "_SOS_pointer",
        "_DMS_pointer",
        "_DMS_F",
        "_pointer_F",
        "_ACO_F",
        "_Holo_F",
        "_Hamr_F",
        "_ARCO_F",
    ]:
        if weapon_class.endswith(suffix):
            base = weapon_class[: -len(suffix)]
            if base in inheritance:
                return base

    return None


def parse_main_config(config_path: str) -> dict[str, dict]:
    """Parse the main combined config file for all class definitions."""
    print(f"Parsing main config: {config_path}")
    with open(config_path) as f:
        content = f.read()

    classes = parse_config_for_classes(content)

    # Also extract weapon names from CfgPatches
    patch_weapons = extract_cfg_patches_weapons(content)
    for w in patch_weapons:
        if w not in classes:
            classes[w] = {"parent": None}

    print(f"  Found {len(classes)} class definitions in main config")
    return classes


def process_individual_configs(pbo_dir: str, out_dir: str) -> dict[str, dict]:
    """Derapify and parse all individual weapon config.bin files."""
    print(f"\nScanning for config.bin files under {pbo_dir}...")
    config_bins = find_all_config_bins(pbo_dir)
    print(f"  Found {len(config_bins)} config.bin files")

    all_classes = {}
    derapified_count = 0

    for bin_path in config_bins:
        hpp_path = derapify_config(bin_path, out_dir)
        if hpp_path:
            derapified_count += 1
            file_classes = parse_individual_config(hpp_path)
            rel_path = os.path.relpath(hpp_path, out_dir)

            # Merge - individual configs are more authoritative for weapon classes
            for cls_name, info in file_classes.items():
                if cls_name not in all_classes:
                    all_classes[cls_name] = info
                elif info.get("parent") and not all_classes[cls_name].get("parent"):
                    all_classes[cls_name]["parent"] = info["parent"]

    print(f"  Derapified {derapified_count} files")
    print(f"  Found {len(all_classes)} class definitions from individual configs")
    return all_classes


def main():
    print("=" * 60)
    print("  Arma 3 Weapon Hierarchy Extractor")
    print("=" * 60)
    print()

    # Ensure reference directory exists
    os.makedirs(REFERENCE_DIR, exist_ok=True)

    # Step 1: Parse the main merged config
    main_classes = parse_main_config(MAIN_CONFIG)

    # Step 2: Process individual weapon configs
    ind_classes = process_individual_configs(PBO_ALL_DIR, DERAPIFIED_DIR)

    # Also extract and process DLC weapon PBOs
    dlc_pbo_map = {
        "Contact": "/ext/SteamLibrary/steamapps/common/Arma 3/Contact/Addons/weapons_f_contact.pbo",
        "Enoch": "/ext/SteamLibrary/steamapps/common/Arma 3/Enoch/Addons/weapons_f_enoch.pbo",
        "Jets": "/ext/SteamLibrary/steamapps/common/Arma 3/Jets/Addons/weapons_f_jets.pbo",
        "Mark": "/ext/SteamLibrary/steamapps/common/Arma 3/Mark/Addons/weapons_f_mark.pbo",
        "Tank": "/ext/SteamLibrary/steamapps/common/Arma 3/Tank/Addons/weapons_f_tank.pbo",
        "Orange": "/ext/SteamLibrary/steamapps/common/Arma 3/Orange/Addons/weapons_f_orange.pbo",
    }
    dlc_extract_dir = os.path.join(DERAPIFIED_DIR, "dlc_pbo_extract")
    for dlc_name, pbo_path in dlc_pbo_map.items():
        if not os.path.exists(pbo_path):
            print(f"\n  DLC PBO not found: {dlc_name} ({pbo_path})")
            continue
        extract_dir = os.path.join(dlc_extract_dir, dlc_name)
        if not os.path.exists(extract_dir):
            print(f"\n  Unpacking DLC PBO: {dlc_name}...")
            os.makedirs(extract_dir, exist_ok=True)
            try:
                result = subprocess.run(
                    ["armake", "unpack", pbo_path, extract_dir],
                    capture_output=True,
                    text=True,
                    timeout=120,
                )
                if result.returncode == 0:
                    print(f"    Extracted to {extract_dir}")
                else:
                    print(
                        f"    WARNING: armake extract failed: {result.stderr.strip()[:100]}"
                    )
                    continue
            except (subprocess.TimeoutExpired, FileNotFoundError) as e:
                print(f"    WARNING: {e}")
                continue
        else:
            print(f"\n  Already extracted: {dlc_name}")

        dlc_classes = process_individual_configs(extract_dir, DERAPIFIED_DIR)
        for cls_name, info in dlc_classes.items():
            if cls_name not in ind_classes:
                ind_classes[cls_name] = info
        print(f"  Added {len(dlc_classes)} classes from {dlc_name}")

    # Step 3: Merge (main provides base hierarchy, individuals provide weapon classes)
    merged = {}

    # Start with individual config results (weapon-specific)
    for cls_name, info in ind_classes.items():
        merged[cls_name] = info

    # Add main config results (but don't overwrite weapon-specific info)
    for cls_name, info in main_classes.items():
        if cls_name not in merged:
            merged[cls_name] = info
        elif info.get("parent") and not merged[cls_name].get("parent"):
            merged[cls_name]["parent"] = info["parent"]

    print(f"\n  Merged: {len(merged)} unique class definitions")

    # Step 4: Build the hierarchy
    inheritance, by_type, all_weapon_classes, base_classes, parent_of = (
        build_full_hierarchy(merged)
    )

    # Separate base classes from unknown
    base_class_names = sorted(base_classes)

    print(f"\n  Weapon classes by type:")
    for wtype in sorted(by_type.keys()):
        count = len(by_type[wtype])
        examples = ", ".join(sorted(by_type[wtype])[:5])
        print(f"    {wtype}: {count} ({examples}{',...' if count > 5 else ''})")

    print(f"\n  Base classes: {len(base_class_names)}")
    print(
        f"    {', '.join(base_class_names[:20])}{'...' if len(base_class_names) > 20 else ''}"
    )

    # Step 5: Build the tree
    tree = build_tree(inheritance, parent_of)

    # Step 6: Build output
    output = {
        "base_classes": base_class_names,
        "inheritance": inheritance,
        "by_type": by_type,
        "all_weapon_classes": sorted(all_weapon_classes),
        "tree": tree,
        "total_weapon_classes": len(all_weapon_classes),
        "source": "Derived from weapons_f PBO configs via armake derapify",
    }

    with open(OUTPUT_JSON, "w") as f:
        json.dump(output, f, indent=2)

    print(f"\n  Output written to: {OUTPUT_JSON}")
    print(f"  Total weapon classes: {len(all_weapon_classes)}")

    # Step 7: Optional analysis - match against ir_weapons.tsv
    if os.path.exists(IR_WEAPONS_TSV):
        print("\n" + "─" * 60)
        print("  Cross-reference with ir_weapons.tsv")
        print("─" * 60)

        import csv

        tsv_models = []
        with open(IR_WEAPONS_TSV, newline="") as f:
            first = f.readline().strip()
            if not first.startswith("#"):
                f.seek(0)
            reader = csv.DictReader(f, delimiter="\t")
            for row in reader:
                model = row.get("model", "").strip()
                if model:
                    tsv_models.append(model)

        # ir_weapons.tsv uses CLEANED names: stripped of arifle_/hgun_/etc prefix,
        # _F suffix removed, lowercase. Build reverse lookup from our weapon classes.
        # Build cleaned-name -> Arma class mapping
        cleaned_to_arma: dict[str, str] = {}
        for cls_name in inheritance:
            wtype = inheritance[cls_name].get("type", "unknown")
            if wtype in ("base", "unknown"):
                continue
            cleaned = cls_name
            for prefix, _ in PREFIX_TO_TYPE:
                if cleaned.startswith(prefix):
                    cleaned = cleaned[len(prefix) :]
                    break
            if cleaned.endswith("_F"):
                cleaned = cleaned[:-2]
            if cleaned.endswith("_Base"):
                cleaned = cleaned[:-5]
            cleaned_lower = cleaned.lower()
            if cleaned_lower not in cleaned_to_arma:
                cleaned_to_arma[cleaned_lower] = cls_name

        # Manual mappings for common weapons where cleaned names differ
        manual_mapping: dict[str, str] = {
            "mx": "arifle_MX_F",
            "mxc": "arifle_MXC_F",
            "mxm": "arifle_MXM_F",
            "mx_sw": "arifle_MX_SW_F",
            "mx_gl": "arifle_MX_GL_F",
            "katiba": "arifle_Katiba_F",
            "katiba_c": "arifle_Katiba_C_F",
            "katiba_gl": "arifle_Katiba_GL_F",
            "trg21": "arifle_TRG21_F",
            "trg20": "arifle_TRG20_F",
            "mk20": "arifle_Mk20_F",
            "mk20c": "arifle_Mk20C_F",
            "sdar": "arifle_SDAR_F",
            "p07": "hgun_P07_F",
            "rook40": "hgun_Rook40_F",
            "acpc2": "hgun_ACPC2_F",
            "pdw2000": "hgun_PDW2000_F",
            "ebr": "srifle_EBR_F",
            "gm6": "srifle_GM6_F",
            "lrr": "srifle_LRR_F",
            "m320": "srifle_LRR_F",
            "zafir": "LMG_Zafir_F",
            "mk200": "LMG_Mk200_F",
            "nlaw": "launch_NLAW_F",
            "rpg32": "launch_RPG32_F",
            "smg_01": "SMG_01_F",
            "smg_02": "SMG_02_F",
            "smg_03": "SMG_03_black",
            "dmr_01": "srifle_DMR_01_F",
        }
        for cleaned, arma_cls in manual_mapping.items():
            if cleaned not in cleaned_to_arma and arma_cls in inheritance:
                cleaned_to_arma[cleaned] = arma_cls

        direct_matches: set[str] = set()
        inferred_matches: set[str] = set()
        for model in tsv_models:
            model_lower = model.lower().strip()
            # Direct cleaned-name lookup
            if model_lower in cleaned_to_arma:
                direct_matches.add(model)
                continue
            # TSV names often have a trailing 'f' (from _F suffix) appended
            # e.g. "acpc2f" -> "acpc2" -> hgun_ACPC2_F
            if model_lower.endswith("f") and len(model_lower) > 3:
                no_f = model_lower[:-1]
                if no_f in cleaned_to_arma:
                    direct_matches.add(model)
                    continue
            # stripping variant suffixes
            found = False
            var_suffixes = [
                "sndsf",
                "snds",
                "pointerf",
                "pointer",
                "acof",
                "aco",
                "holof",
                "holo",
                "hamrf",
                "hamr",
                "arcosightf",
                "arco",
                "dmsf",
                "dms",
                "rco",
                "sos",
                "lrps",
                "nvs",
                "ir",
                "flf",
                "fl",
            ]
            for suffix in var_suffixes:
                if model_lower.endswith(suffix):
                    base_lower = model_lower[: -len(suffix)]
                    for candidate in (base_lower, base_lower.rstrip("_").rstrip("f")):
                        if candidate in cleaned_to_arma:
                            inferred_matches.add(model)
                            found = True
                            break
                    if found:
                        break
            if not found:
                stripped = model_lower.rstrip("0123456789")
                if stripped != model_lower and stripped in cleaned_to_arma:
                    inferred_matches.add(model)
                    continue
                if "base" in model_lower:
                    base_try = model_lower.split("base")[0].rstrip("_")
                    if base_try in cleaned_to_arma:
                        inferred_matches.add(model)

        total = len(tsv_models)
        matched = len(direct_matches)
        inferred = len(inferred_matches)
        unknown = total - matched - inferred

        print(f"\n    Total TSV entries:      {total:5d}")
        print(
            f"    Direct Arma class match: {matched:5d} ({matched / total * 100:.1f}%)"
        )
        print(
            f"    Variant-inferred match:  {inferred:5d} ({inferred / total * 100:.1f}%)"
        )
        print(
            f"    Unknown/junk:           {unknown:5d} ({unknown / total * 100:.1f}%)"
        )

        if unknown > 0:
            unknown_models = [
                m
                for m in tsv_models
                if m not in direct_matches and m not in inferred_matches
            ]
            print(f"\n    Unknown models (first 50):")
            for m in unknown_models[:50]:
                print(f"      - {m}")
            if len(unknown_models) > 50:
                print(f"      ... and {len(unknown_models) - 50} more")

    print("\nDone.")


if __name__ == "__main__":
    main()
