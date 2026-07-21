#!/usr/bin/env python3
"""
One-shot gap analyzer for a single Arma 3 PBO.

Reads a PBO in-memory, derapifies its config, extracts weapons,
clothing/gear, vehicle references, and ammunition — then cross-references
each class against the project's IRL databases and reports what's covered
vs what's missing.

Usage:
    python data/scripts/gap_analyzer.py /path/to/mod.pbo
    python data/scripts/gap_analyzer.py /path/to/mod.pbo --verbose

By default shows a summary. Pass --verbose to see every unmatched class.
"""

import json
import os
import re
import struct
import subprocess
import sys
from collections import defaultdict
from pathlib import Path

# ── IRL database paths ─────────────────────────────────────────────────────
PROJECT_ROOT = Path(__file__).resolve().parent.parent.parent
DATA_DIR = PROJECT_ROOT / "data"

IRL_WEAPONS_PATH = DATA_DIR / "ir_weapons.tsv"
IRL_AMMO_PATH = DATA_DIR / "ir_ammo.tsv"
IRL_CLOTHING_PATH = DATA_DIR / "ir_clothing.tsv"
IRL_ARMOR_PATH = DATA_DIR / "ir_armor.tsv"
IRL_MATERIALS_PATH = DATA_DIR / "ir_materials.tsv"


# ── IRL database loaders ──────────────────────────────────────────────────


def load_tsv_map(path: Path, key_col=0) -> set[str]:
    """Load column *key_col* from a TSV into a set."""
    if not path.exists():
        return set()
    keys = set()
    with open(path) as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            parts = line.split("\t")
            if len(parts) > key_col:
                keys.add(parts[key_col].strip())
    return keys


def load_weapons() -> set[str]:
    """Load weapon model keys from ir_weapons.tsv."""
    return load_tsv_map(IRL_WEAPONS_PATH, 0)


def load_clothing() -> set[str]:
    """Load classname keys from ir_clothing.tsv."""
    return load_tsv_map(IRL_CLOTHING_PATH, 0)


def load_armor_vehicles() -> set[str]:
    """Load vehicle names from ir_armor.tsv (first column)."""
    return load_tsv_map(IRL_ARMOR_PATH, 0)


def load_materials() -> set[str]:
    """Load material IDs from ir_materials.tsv."""
    return load_tsv_map(IRL_MATERIALS_PATH, 0)


def load_ammo() -> set[str]:
    """Load ammo model keys from ir_ammo.tsv."""
    return load_tsv_map(IRL_AMMO_PATH, 0)


# ── PBO reader (from extract_gear_from_pbo.py) ────────────────────────────


def read_pbo(pbo_path: str) -> dict:
    """Read a PBO file into memory. Returns {files: {name: bytes}, prefix: str}."""
    with open(pbo_path, "rb") as f:
        data = f.read()

    n = len(data)
    pos = 0
    result = {}

    def read_asciiz(offset):
        end = data.find(b"\0", offset)
        if end < 0:
            return "", offset
        return data[offset:end].decode("ascii", errors="replace"), end + 1

    # ── Step 1: Version entry ──
    if pos + 21 > n:
        raise ValueError(f"Not a valid PBO file (too small): {pbo_path}")
    filename, pos = read_asciiz(pos)
    (packing, orig_size, reserved, timestamp, data_size) = struct.unpack_from(
        "<IIIII", data, pos
    )
    pos += 20

    srev_magic = struct.unpack("<I", b"sreV")[0]
    if packing != srev_magic:
        found = False
        for scan_pos in range(min(16, n)):
            p = data[scan_pos : scan_pos + 4]
            if len(p) == 4 and struct.unpack("<I", p)[0] == srev_magic:
                return _read_pbo_raw(data, scan_pos, pbo_path)
            if scan_pos + 4 > n:
                break
        raise ValueError(f"Not a PBO file (bad version magic): {pbo_path}")

    # ── Step 2: Header properties ──
    properties = {}
    while pos < n:
        key, pos = read_asciiz(pos)
        if not key:
            break
        value, pos = read_asciiz(pos)
        properties[key] = value

    # ── Step 3: File entry headers ──
    file_entries = []
    while pos < n:
        filename, pos = read_asciiz(pos)
        if not filename:
            break
        if pos + 20 > n:
            break
        (packing, orig_size, reserved, timestamp, data_size) = struct.unpack_from(
            "<IIIII", data, pos
        )
        pos += 20
        file_entries.append(
            {
                "name": filename,
                "packing": packing,
                "orig_size": orig_size,
                "data_size": data_size,
            }
        )

    # ── Step 3.5: Terminator ──
    pos += 20

    # ── Step 4: File data ──
    for entry in file_entries:
        if pos + entry["data_size"] > n:
            break
        result[entry["name"]] = data[pos : pos + entry["data_size"]]
        pos += entry["data_size"]

    return {
        "files": result,
        "properties": properties,
        "prefix": properties.get("prefix", ""),
    }


def _read_pbo_raw(data: bytes, offset: int, pbo_path: str) -> dict:
    """Read PBO data starting from a known offset (Mikero variant)."""
    n = len(data)
    pos = offset

    def ra(off):
        end = data.find(b"\0", off)
        if end < 0:
            return "", off
        return data[off:end].decode("ascii", errors="replace"), end + 1

    filename, pos = ra(pos)
    (packing, _orig_sz, _rsv, _ts, _ds) = struct.unpack_from("<IIIII", data, pos)
    pos += 20
    if struct.unpack("<I", b"sreV")[0] != packing:
        raise ValueError(f"Bad PBO version magic at offset {offset}: {pbo_path}")

    properties = {}
    while pos < n:
        key, pos = ra(pos)
        if not key:
            break
        val, pos = ra(pos)
        properties[key] = val

    file_entries = []
    while pos < n:
        fn, pos = ra(pos)
        if not fn:
            break
        if pos + 20 > n:
            break
        (p, osz, rsv, ts, dsz) = struct.unpack_from("<IIIII", data, pos)
        pos += 20
        file_entries.append(
            {"name": fn, "packing": p, "orig_size": osz, "data_size": dsz}
        )

    result = {}
    for entry in file_entries:
        if pos + entry["data_size"] > n:
            break
        result[entry["name"]] = data[pos : pos + entry["data_size"]]
        pos += entry["data_size"]

    return {
        "files": result,
        "properties": properties,
        "prefix": properties.get("prefix", ""),
    }


# ── Config derapification ─────────────────────────────────────────────────


def derapify_bytes(bin_data: bytes) -> str | None:
    """Derapify binary config to HPP text via armake."""
    rap_pos = -1
    search_limit = min(4096, len(bin_data) - 4)
    for off in range(search_limit):
        if bin_data[off] == 0 and bin_data[off + 1 : off + 4] == b"raP":
            rap_pos = off
            break

    if rap_pos < 0:
        return None

    clean_data = bin_data[rap_pos:]

    result = subprocess.run(
        ["armake", "derapify"],
        input=clean_data,
        capture_output=True,
        timeout=30,
    )
    if result.returncode != 0:
        return None
    return result.stdout.decode("utf-8", errors="replace")


# ── Config parsers ────────────────────────────────────────────────────────


def find_cfg_block(text: str, class_name: str) -> str | None:
    """Find the full block text for 'class X ... { ... }' at top level."""
    pattern = re.compile(rf"class\s+{re.escape(class_name)}\s*{{", re.IGNORECASE)
    m = pattern.search(text)
    if not m:
        return None
    start = m.start()
    depth = 0
    pos = start
    started = False
    while pos < len(text):
        if text[pos] == "{":
            depth += 1
            started = True
        elif text[pos] == "}":
            depth -= 1
        if started and depth == 0:
            return text[start : pos + 1]
        pos += 1
    return None


def extract_top_level_classes(text: str, parent: str = "") -> list[dict]:
    """Extract all top-level class definitions and their parent from a config block.

    Returns list of {class_name, parent_class, offset, block_text}.
    """
    classes = []
    pos = 0
    while pos < len(text):
        m = re.search(r"class\s+(\S+)\s*(?::\s*(\S+))?\s*\{", text[pos:])
        if not m:
            break
        abs_start = pos + m.start()
        match_end = pos + m.end()
        # Check depth: count braces before this position
        prefix = text[:abs_start]
        depth = prefix.count("{") - prefix.count("}")
        if depth == 0:  # top-level
            cls_name = m.group(1)
            parent_cls = m.group(2) or ""
            # Find matching close brace
            d = 1
            scan = match_end
            while d > 0 and scan < len(text):
                if text[scan] == "{":
                    d += 1
                elif text[scan] == "}":
                    d -= 1
                scan += 1
            block_text = text[abs_start:scan]
            classes.append(
                {
                    "class_name": cls_name,
                    "parent": parent_cls,
                    "block": block_text,
                }
            )
            pos = scan
        else:
            pos = abs_start + 1
    return classes


def find_class_inherited(text: str, class_name: str) -> dict | None:
    """Find a class definition by walking up inheritance chain."""
    # Direct lookup
    classes = extract_top_level_classes(text)
    by_name = {c["class_name"]: c for c in classes}

    visited = set()
    current = class_name
    while current and current not in visited:
        visited.add(current)
        if current in by_name:
            return by_name[current]
        current = None
    return None


def get_field_value(block: str, field: str) -> str | None:
    """Extract a field value from a config block (e.g. field=123; or field="str").

    Case-insensitive field matching since Arma configs can use any casing.
    """
    # Try quoted string first
    m = re.search(rf'\b{re.escape(field)}\s*=\s*"([^"]*)"', block, re.IGNORECASE)
    if m:
        return m.group(1)
    # Try numeric
    m = re.search(rf"\b{re.escape(field)}\s*=\s*([^;]+);", block, re.IGNORECASE)
    if m:
        return m.group(1).strip()
    return None


def get_armor_value(block: str) -> float:
    """Get armor/armorStructural value from a class block."""
    val = get_field_value(block, "armor")
    if val:
        try:
            return float(val)
        except ValueError:
            pass
    return 0.0


def get_pass_through(block: str) -> float:
    val = get_field_value(block, "passThrough")
    if val:
        try:
            return float(val)
        except ValueError:
            pass
    return 1.0


# ── IRL cross-reference ───────────────────────────────────────────────────


def normalize_weapon_key(name: str) -> str:
    """Normalize a weapon classname to IRL_WEAPONS key format."""
    s = name.lower()
    prefixes = [
        "arifle_",
        "srifle_",
        "hgun_",
        "smg_",
        "lmg_",
        "mmg_",
        "sgun_",
        "launch_",
        "pdw_",
        "dmr_",
        "hmg_",
        "gmg_",
        "mortar_",
    ]
    for p in prefixes:
        if s.startswith(p):
            s = s[len(p) :]
            break
    return s


def normalize_clothing_key(name: str) -> str:
    """Clothing is matched by exact classname."""
    return name


# ── Main analyzer ─────────────────────────────────────────────────────────


def analyze_pbo(pbo_path: str, verbose: bool = False):
    pbo_name = Path(pbo_path).stem
    print(f"\n{'=' * 60}")
    print(f"Gap Analysis: {pbo_name}")
    print(f"{'=' * 60}")

    # ── Load IRL databases ────────────────────────────────────────────────
    irl_weapons = load_weapons()
    irl_clothing = load_clothing()
    irl_armor_vehicles = load_armor_vehicles()
    irl_materials = load_materials()
    irl_ammo = load_ammo()

    print(
        f"IRL databases: {len(irl_weapons)} weapons, {len(irl_clothing)} clothing, "
        f"{len(irl_armor_vehicles)} vehicle armor, {len(irl_materials)} materials, "
        f"{len(irl_ammo)} ammo types"
    )

    # ── Read PBO ──────────────────────────────────────────────────────────
    try:
        pbo_data = read_pbo(pbo_path)
    except Exception as e:
        print(f"  ERROR: PBO read failed: {e}")
        return

    entries = pbo_data["files"]

    # ── Find config ───────────────────────────────────────────────────────
    config_key = None
    for name in (
        "config.bin",
        "config.cpp",
        "config.hpp",
        "Config.bin",
        "Config.cpp",
        "Config.hpp",
    ):
        if name in entries:
            config_key = name
            break
    if not config_key:
        for name in entries:
            if "config" in name.lower():
                config_key = name
                break
    if not config_key:
        print("  No config found in PBO")
        return

    config_data = entries[config_key]

    # Detect plain-text config — skip armake, parse directly.
    is_text_config = False
    if len(config_data) > 16:
        peek = config_data[:16]
        if (peek[0:1] != b"\x00" and peek[4:7] != b"raP") and (
            peek[:5] == b"class" or peek[:2] == b"/*" or peek[:2] == b"//"
        ):
            is_text_config = True

    try:
        if is_text_config:
            config_text = config_data.decode("utf-8", errors="replace")
        else:
            config_text = derapify_bytes(config_data)
    except Exception as e:
        print(f"  Config parse failed: {e}")
        return

    if not config_text:
        print("  Empty config or derapify failed")
        return

    # ── Find CfgWeapons block ─────────────────────────────────────────────
    cfg_weapons = find_cfg_block(config_text, "CfgWeapons")
    if not cfg_weapons:
        print("  No CfgWeapons found in config")

    # ── Find CfgVehicles block ────────────────────────────────────────────
    cfg_vehicles = find_cfg_block(config_text, "CfgVehicles")
    if not cfg_vehicles:
        print("  No CfgVehicles found in config")

    # ── Find CfgAmmo block ────────────────────────────────────────────────
    cfg_ammo = find_cfg_block(config_text, "CfgAmmo")
    if not cfg_ammo:
        print("  No CfgAmmo found in config")

    # ── Find CfgPatches for version info ──────────────────────────────────
    cfg_patches = find_cfg_block(config_text, "CfgPatches")
    mod_version = ""
    if cfg_patches:
        version_str = get_field_value(cfg_patches, "version")
        if version_str:
            mod_version = version_str

    if mod_version:
        print(f"  Mod version: {mod_version}")

    # ── Parse weapons from CfgWeapons ─────────────────────────────────────
    weapon_classes: list[dict] = []
    gear_classes: list[dict] = []

    if cfg_weapons:
        # Get all top-level classes in CfgWeapons (case-insensitive — some mods use cfgWeapons)
        inner = re.search(r"class\s+cfgWeapons\s*\{", config_text, re.IGNORECASE)
        if inner:
            # Find the content inside CfgWeapons { ... }
            start = inner.end()
            depth = 1
            pos = start
            while depth > 0 and pos < len(config_text):
                if config_text[pos] == "{":
                    depth += 1
                elif config_text[pos] == "}":
                    depth -= 1
                pos += 1
            cfg_body = config_text[start : pos - 1]

            # Extract classes
            for cls in extract_top_level_classes(cfg_body):
                name = cls["class_name"]
                block = cls["block"]

                scope = get_field_value(block, "scope")
                parent = cls["parent"]
                block_lower = block.lower()

                # ── Known base class sets ──
                weapon_base_lower = {
                    "rifle_base_f",
                    "rifle_short_base_f",
                    "rifle_long_base_f",
                    "pistol_base_f",
                    "launcher_base_f",
                    "smg_base_f",
                    "sniper_base_f",
                    "lmg_base_f",
                    "mgun_base_f",
                    "dmr_base_f",
                    "shotgun_base_f",
                    "grenadelauncher_base_f",
                    "defaultweapon",
                    "weaponslot",
                    "mortar_base_f",
                    "hmg_base_f",
                    "gmg_base_f",
                }
                gear_parents_lower = {
                    "vest_base_f",
                    "vest_camo_base",
                    "vest_nocamo_base",
                    "headgear_base_f",
                    "h_helmet_base_f",
                    "uniform_base",
                    "uniform_base_f",
                    "glasses_base_f",
                    "g_glasses_base_f",
                    "itemcore",
                    "nvgoggles",
                }

                block_lower = block.lower()
                parent_lower = parent.lower()

                has_item_info = "iteminfo" in block_lower
                has_init_speed = "initspeed" in block_lower
                has_magazines = "magazines[]" in block_lower
                has_ace_ballistics = (
                    "ace_barrellength" in block_lower
                    or "ace_barreltwist" in block_lower
                )
                parent_is_weapon_base = parent_lower in weapon_base_lower
                parent_is_gear_base = parent_lower in gear_parents_lower
                is_nvgs = "nvgoggles" in parent_lower or "nvg" in name.lower()

                # ── Classification priority: gear > weapon ──
                is_gear = has_item_info or parent_is_gear_base or is_nvgs
                is_weapon = (
                    has_init_speed
                    or has_magazines
                    or has_ace_ballistics
                    or parent_is_weapon_base
                )

                if is_gear and (scope == "2" or scope is None) and not is_weapon:
                    item_type = None
                    if "vestitem" in block_lower:
                        item_type = "VestItem"
                    elif "headgearitem" in block_lower:
                        item_type = "HeadgearItem"
                    elif "uniformitem" in block_lower:
                        item_type = "UniformItem"
                    elif "glassesitem" in block_lower:
                        item_type = "GlassesItem"
                    elif "vest" in parent_lower:
                        item_type = "VestItem"
                    elif "helmet" in parent_lower or "headgear" in parent_lower:
                        item_type = "HeadgearItem"
                    elif "uniform" in parent_lower:
                        item_type = "UniformItem"
                    elif "glasses" in parent_lower or "nvgoggles" in parent_lower:
                        item_type = "GlassesItem"

                    armor_val = get_armor_value(block)
                    pt_val = get_pass_through(block)
                    gear_classes.append(
                        {
                            "class_name": name,
                            "parent": parent,
                            "item_type": item_type or "GearItem",
                            "armor": armor_val,
                            "pass_through": pt_val,
                        }
                    )

                elif is_weapon and (scope == "2" or scope is None):
                    caliber = get_field_value(block, "caliber")
                    init_speed = get_field_value(block, "initSpeed")
                    ace_bl = get_field_value(block, "ace_barrelLength")
                    weapon_classes.append(
                        {
                            "class_name": name,
                            "parent": parent,
                            "caliber_str": caliber or "?",
                            "init_speed": init_speed or "?",
                            "ace_barrel_length": ace_bl or "?",
                        }
                    )

    # ── Parse vehicles from CfgVehicles ───────────────────────────────────
    vehicle_classes: list[str] = []
    if cfg_vehicles:
        for cls in extract_top_level_classes(cfg_vehicles):
            # Skip the block header itself (CfgVehicles class)
            if cls["class_name"] in ("CfgVehicles",):
                continue
            vehicle_classes.append(cls["class_name"])

    # ── Parse ammo from CfgAmmo ───────────────────────────────────────────
    ammo_classes: list[dict] = []
    if cfg_ammo:
        for cls in extract_top_level_classes(cfg_ammo):
            cal = get_field_value(cls["block"], "caliber")
            mass = get_field_value(cls["block"], "mass")
            hit = get_field_value(cls["block"], "hit")
            ammo_classes.append(
                {
                    "class_name": cls["class_name"],
                    "caliber": cal or "?",
                    "mass": mass or "?",
                    "hit": hit or "?",
                }
            )

    # ── Cross-reference ───────────────────────────────────────────────────
    print(f"\n  File entries in PBO: {len(entries)}")

    # Weapons
    weapon_covered = 0
    weapon_gaps = []
    for w in weapon_classes:
        key = normalize_weapon_key(w["class_name"])
        # Also try the raw name
        if key in irl_weapons or w["class_name"].lower() in {
            k.lower() for k in irl_weapons
        }:
            weapon_covered += 1
        else:
            weapon_gaps.append(w)

    print(f"\n  ── WEAPONS ──")
    print(f"  Found: {len(weapon_classes)} scope-2 classes")
    print(f"  Covered by IRL DB: {weapon_covered}")
    print(f"  Gaps: {len(weapon_gaps)}")
    if weapon_gaps and verbose:
        for w in weapon_gaps:
            print(
                f"    {w['class_name']:45s} cal={w['caliber_str']:10s} speed={w['init_speed']}"
            )

    # Gear / clothing
    gear_covered = 0
    gear_gaps = []
    for g in gear_classes:
        if g["class_name"] in irl_clothing:
            gear_covered += 1
        else:
            gear_gaps.append(g)

    # Also check if some gear items have armor values that should be in the DB
    gear_with_armor_not_covered = [g for g in gear_gaps if g["armor"] > 0]

    print(f"\n  ── CLOTHING / GEAR ──")
    print(f"  Found: {len(gear_classes)} wearables")
    print(f"  Covered by IRL DB: {gear_covered}")
    print(f"  Gaps: {len(gear_gaps)}")
    if gear_with_armor_not_covered:
        print(
            f"  Gaps WITH armor>0: {len(gear_with_armor_not_covered)} (most valuable)"
        )
        if verbose:
            for g in gear_with_armor_not_covered[:20]:
                print(
                    f"    {g['class_name']:45s} type={g['item_type']:15s} armor={g['armor']:.1f} pt={g['pass_through']:.2f}"
                )
    if verbose and gear_gaps:
        for g in gear_gaps:
            print(
                f"    {g['class_name']:45s} type={g['item_type']:15s} armor={g['armor']:.1f}"
            )

    # Vehicle armor
    vehicle_covered = 0
    vehicle_gaps = []
    for v in vehicle_classes:
        if v in irl_armor_vehicles or v.lower() in {
            k.lower() for k in irl_armor_vehicles
        }:
            vehicle_covered += 1
        else:
            vehicle_gaps.append(v)

    print(f"\n  ── VEHICLES ──")
    print(f"  Found: {len(vehicle_classes)} unique classes")
    print(f"  Covered by IRL armor DB: {vehicle_covered}")
    print(f"  Gaps: {len(vehicle_gaps)}")
    if vehicle_gaps and verbose:
        for v in vehicle_gaps[:20]:
            print(f"    {v}")

    # Ammo
    ammo_covered = 0
    ammo_gaps = []
    for a in ammo_classes:
        if a["class_name"].lower() in {k.lower() for k in irl_ammo}:
            ammo_covered += 1
        else:
            ammo_gaps.append(a)

    print(f"\n  ── AMMUNITION ──")
    print(f"  Found: {len(ammo_classes)} classes")
    print(f"  Covered by IRL DB: {ammo_covered}")
    print(f"  Gaps: {len(ammo_gaps)}")
    if ammo_gaps and verbose:
        for a in ammo_gaps[:20]:
            print(
                f"    {a['class_name']:45s} cal={a['caliber']:10s} mass={a['mass']:10s} hit={a['hit']}"
            )

    # Materials used but missing from materials DB
    # Collect all unique material-like field values from config
    material_refs = set()
    m = re.findall(
        r'\b(material|armorMaterial|materialId)\s*=\s*"([^"]+)"', config_text
    )
    for _, val in m:
        material_refs.add(val)
    m = re.findall(r"\b(material|armorMaterial|materialId)\s*=\s*(\w+)", config_text)
    for _, val in m:
        if val not in ("", "0", "1") and not val[0].isdigit():
            material_refs.add(val)

    known_materials_lower = {m.lower() for m in irl_materials}
    material_gaps = [m for m in material_refs if m.lower() not in known_materials_lower]

    print(f"\n  ── MATERIALS ──")
    print(f"  Material refs in config: {len(material_refs)}")
    print(f"  Covered by IRL DB: {len(material_refs) - len(material_gaps)}")
    print(f"  Gaps: {len(material_gaps)}")
    if material_gaps and verbose:
        for m in sorted(material_gaps)[:20]:
            print(f"    {m}")

    # ── Overall score ─────────────────────────────────────────────────────
    total_found = (
        len(weapon_classes)
        + len(gear_classes)
        + len(vehicle_classes)
        + len(ammo_classes)
        + len(material_refs)
    )
    total_covered = (
        weapon_covered
        + gear_covered
        + vehicle_covered
        + ammo_covered
        + (len(material_refs) - len(material_gaps))
    )
    total_gap = (
        len(weapon_gaps)
        + len(gear_gaps)
        + len(vehicle_gaps)
        + len(ammo_gaps)
        + len(material_gaps)
    )

    pct = (total_covered / total_found * 100) if total_found > 0 else 0
    print(f"\n  ── OVERALL ──")
    print(f"  Total items: {total_found}")
    print(f"  Covered: {total_covered} ({pct:.0f}%)")
    print(f"  Gaps: {total_gap}")

    # Most valuable gaps: gear with armor values, weapons with high caliber
    valuable = []
    for g in gear_with_armor_not_covered:
        valuable.append(
            {
                "type": "gear",
                "class_name": g["class_name"],
                "value": g["armor"],
                "detail": f"armor={g['armor']:.1f} pt={g['pass_through']:.2f}",
            }
        )
    for w in weapon_gaps:
        valuable.append(
            {
                "type": "weapon",
                "class_name": w["class_name"],
                "value": 1,
                "ace_bl": w.get("ace_barrel_length", "?"),
                "detail": f"cal={w['caliber_str']} speed={w['init_speed']}",
            }
        )

    valuable.sort(key=lambda x: -x["value"])
    if valuable:
        print(f"\n  ── TOP VALUABLE GAPS (gear with highest armor) ──")
        for v in valuable[:10]:
            print(f"    [{v['type']:8s}] {v['class_name']:45s} {v['detail']}")


def main():
    import argparse

    parser = argparse.ArgumentParser(
        description="Analyze a single PBO for IRL data gaps"
    )
    parser.add_argument("pbo_path", help="Path to the .pbo file")
    parser.add_argument(
        "--verbose", "-v", action="store_true", help="Show all unmatched classes"
    )
    args = parser.parse_args()

    if not os.path.isfile(args.pbo_path):
        print(f"Error: file not found: {args.pbo_path}")
        sys.exit(1)

    analyze_pbo(args.pbo_path, verbose=args.verbose)


if __name__ == "__main__":
    main()
