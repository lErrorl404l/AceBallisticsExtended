#!/usr/bin/env python3
"""
ABE Config Generator
====================
Reads weapon JSONs from data/weapons/ and ammo JSONs from data/ammo/,
generates a .hpp config file with CfgWeapons and CfgAmmo entries that can
be #included into the Arma mod's config.cpp.

Usage:
    python3 data/scripts/generate_configs.py [--weapon-dir PATH] [--ammo-dir PATH] [--output PATH]

Python 3 standard library only — zero external dependencies.
"""

import argparse
import json
import os
import re
import sys
from typing import Any


# ── Safe coercions ──────────────────────────────────────────────────────────


def _f(val: Any, default: float = 0.0) -> float:
    """Safely coerce to float."""
    if val is None:
        return default
    if isinstance(val, (int, float)):
        return float(val)
    return default


def _s(val: Any, default: str = "") -> str:
    """Safely coerce to str."""
    if val is None:
        return default
    if isinstance(val, str):
        return val
    return str(val)


def _i(val: Any, default: int = 0) -> int:
    """Safely coerce to int."""
    if val is None:
        return default
    if isinstance(val, int):
        return val
    if isinstance(val, float):
        return int(val)
    return default


def _fmt(val: float) -> str:
    """Format a number for Arma config — int if whole, else trimmed float."""
    if val == int(val):
        return str(int(val))
    return f"{val:g}"


# ── Projectile type heuristics ──────────────────────────────────────────────


def determine_projectile_type(class_name: str, model: str = "") -> str:
    """Determine projectile type from class name and model name."""
    c = (class_name + " " + model).lower()
    if re.search(r"\b(ap(?!p)|armor.piercing|slap)\b", c):
        return "ap"
    if re.search(r"\b(hp|hollow.?point)\b", c):
        return "hp"
    if re.search(r"\bsp(?!l)|soft.?point\b", c):
        return "sp"
    if re.search(r"\b(incendiary|api|iap)\b", c):
        return "incendiary"
    if re.search(r"\b(tracer|trac)\b", c):
        return "tracer"
    if re.search(r"\b(slug|buckshot|pellet|shot)\b", c):
        return "shotgun"
    if "ball" in c:
        return "fmj"
    return "fmj"


# ── JSON file scanning ──────────────────────────────────────────────────────


def find_json_files(root_dir: str) -> list[str]:
    """Recursively find all .json files under root_dir, sorted."""
    if not os.path.isdir(root_dir):
        print(f"  WARNING: directory not found: {root_dir}", file=sys.stderr)
        return []
    result = []
    for dirpath, _dirnames, filenames in os.walk(root_dir):
        for fname in sorted(filenames):
            if fname.endswith(".json"):
                result.append(os.path.join(dirpath, fname))
    return result


def load_json_files(file_paths: list[str]) -> list[dict]:
    """Load and validate JSON files, returning a list of (filename, data) dicts."""
    records: list[dict] = []
    for fpath in file_paths:
        try:
            with open(fpath) as f:
                data = json.load(f)
            if not isinstance(data, dict):
                print(f"  SKIP {fpath}: top-level value is not a dict", file=sys.stderr)
                continue
            records.append(data)
        except (json.JSONDecodeError, OSError) as e:
            print(f"  SKIP {fpath}: {e}", file=sys.stderr)
    return records


# ── Arma config value rendering ─────────────────────────────────────────────


def arma_val(val: Any, is_str: bool = False) -> str:
    """Render a Python value as an Arma config literal."""
    if val is None:
        return '""' if is_str else "-1"
    if isinstance(val, bool):
        return "1" if val else "0"
    if isinstance(val, str):
        return f'"{val}"'
    if isinstance(val, (int, float)):
        return _fmt(float(val))
    return f'"{val}"' if is_str else "-1"


# ── Config block generators ─────────────────────────────────────────────────


def gen_weapon_entry(weapon: dict) -> str | None:
    """Generate a CfgWeapons class entry from a weapon JSON dict."""
    class_name = _s(weapon.get("class"))
    if not class_name:
        return None

    lines: list[str] = []
    indent = "    "

    # Comment from notes field
    notes = _s(weapon.get("notes"))
    if notes:
        lines.append(f"{indent}// {notes}")

    lines.append(f"{indent}class {class_name} {{")

    fields: list[tuple[str, str, bool]] = [
        ("barrel_length_mm", "ABO_barrelLength", True),
        ("chamber_pressure_mpa", "ABO_chamberPressure", True),
        ("rifling_twist_mm", "ABO_riflingTwist", True),
        ("cdm_id", "ABO_cdmId", False),
        ("projectile_mass_g", "ABO_projectileMass", True),
        ("zero_range_m", "ABO_zeroRange", True),
    ]

    for json_key, config_key, is_num in fields:
        if json_key in weapon:
            val = weapon[json_key]
            if val is not None and val != "":
                lines.append(f"{indent}    {config_key} = {arma_val(val, not is_num)};")

    lines.append(f"{indent}}};")
    return "\n".join(lines)


def gen_ammo_entry(ammo: dict) -> str | None:
    """Generate a CfgAmmo class entry from an ammo JSON dict."""
    class_name = _s(ammo.get("class"))
    if not class_name:
        return None

    proj = ammo.get("projectile")
    if not isinstance(proj, dict):
        proj = {}

    lines: list[str] = []
    indent = "    "

    # Comment — model & source
    model = _s(proj.get("model"))
    source = _s(proj.get("source"))
    comment_parts = [p for p in [model, source[:100] if source else ""] if p]
    if comment_parts:
        lines.append(f"{indent}// {' — '.join(comment_parts)}")

    lines.append(f"{indent}class {class_name} {{")

    # ABO_projectileMass
    mass = _f(proj.get("mass_g"), -1)
    if mass > 0:
        lines.append(f"{indent}    ABO_projectileMass = {arma_val(mass)};")

    # ABO_cdmId
    cdm = _s(proj.get("cdm_id"), "g7")
    lines.append(f"{indent}    ABO_cdmId = {arma_val(cdm, True)};")

    # ABO_bcG7
    bc_g7 = _f(proj.get("bc_g7"), -1)
    if bc_g7 > 0:
        lines.append(f"{indent}    ABO_bcG7 = {arma_val(bc_g7)};")

    # ABO_bcG1 (if present)
    bc_g1 = _f(proj.get("bc_g1"), -1)
    if bc_g1 > 0:
        lines.append(f"{indent}    ABO_bcG1 = {arma_val(bc_g1)};")

    # Fragmentation block
    frag = proj.get("fragmentation")
    if isinstance(frag, dict):
        threshold = _f(frag.get("threshold_vel_ms"), -1)
        count = _i(frag.get("avg_fragments"), 0)
        dist = _s(frag.get("mass_distribution"), "log_normal")
        lines.append(f"{indent}    ABO_fragThreshold = {arma_val(threshold)};")
        lines.append(f"{indent}    ABO_fragCount = {arma_val(count)};")
        lines.append(f"{indent}    ABO_fragMassDist = {arma_val(dist, True)};")
    else:
        # Even without fragmentation block, emit defaults so SQF can read them
        lines.append(f"{indent}    ABO_fragThreshold = -1;")
        lines.append(f"{indent}    ABO_fragCount = 0;")
        lines.append(f'{indent}    ABO_fragMassDist = "log_normal";')

    # ABO_caliberOverride
    cal = _f(proj.get("caliber_mm"), -1)
    if cal > 0:
        lines.append(f"{indent}    ABO_caliberOverride = {arma_val(cal)};")

    # ABO_projectileType
    ptype = determine_projectile_type(class_name, _s(proj.get("model")))
    lines.append(f"{indent}    ABO_projectileType = {arma_val(ptype, True)};")

    lines.append(f"{indent}}};")
    return "\n".join(lines)


# ── Report / summary ────────────────────────────────────────────────────────


def build_summary(weapons: list[dict], ammo_list: list[dict]) -> str:
    """Build a short summary comment block."""
    weapon_classes = sorted(w.get("class", "?") for w in weapons if w.get("class"))
    ammo_classes = sorted(a.get("class", "?") for a in ammo_list if a.get("class"))
    w_with_pressure = sum(1 for w in weapons if _f(w.get("chamber_pressure_mpa")) > 0)
    a_with_bc = sum(
        1 for a in ammo_list if _f(a.get("projectile", {}).get("bc_g7")) > 0
    )

    return (
        f"// ABE Generated Config — {len(weapon_classes)} weapons, {len(ammo_classes)} ammo types\n"
        f"//   Weapons with chamber pressure data: {w_with_pressure}/{len(weapons)}\n"
        f"//   Ammo with G7 BC data: {a_with_bc}/{len(ammo_list)}\n"
    )


# ── Main generation ─────────────────────────────────────────────────────────


def generate_config(
    weapon_dir: str,
    ammo_dir: str,
    output_path: str,
) -> int:
    """Generate the full .hpp config file. Returns count of entries written."""
    # Scan and load
    print(f"Scanning weapons in {weapon_dir} ...")
    weapon_files = find_json_files(weapon_dir)
    weapons = load_json_files(weapon_files)
    print(f"  Loaded {len(weapons)} weapon JSONs")

    print(f"Scanning ammo in {ammo_dir} ...")
    ammo_files = find_json_files(ammo_dir)
    ammo_list = load_json_files(ammo_files)
    print(f"  Loaded {len(ammo_list)} ammo JSONs")

    # Generate entries, preserving sort by class name
    weapon_entries: list[tuple[str, str]] = []
    for w in weapons:
        entry = gen_weapon_entry(w)
        if entry is not None:
            key = _s(w.get("class", ""))
            weapon_entries.append((key, entry))

    ammo_entries: list[tuple[str, str]] = []
    for a in ammo_list:
        entry = gen_ammo_entry(a)
        if entry is not None:
            key = _s(a.get("class", ""))
            ammo_entries.append((key, entry))

    weapon_entries.sort(key=lambda x: x[0].lower())
    ammo_entries.sort(key=lambda x: x[0].lower())

    # Build output
    summary = build_summary(weapons, ammo_list)
    lines = [
        "// =============================================================================",
        "// ABE — Advanced Ballistics Extension",
        "// Auto-generated config entries for CfgWeapons and CfgAmmo.",
        "// Generated by data/scripts/generate_configs.py",
        "//",
        summary,
        "// =============================================================================",
        "",
        "class CfgWeapons {",
    ]

    for _key, entry in weapon_entries:
        lines.append(entry)

    lines.append("};")
    lines.append("")
    lines.append("class CfgAmmo {")

    for _key, entry in ammo_entries:
        lines.append(entry)

    lines.append("};")
    lines.append("")

    content = "\n".join(lines)

    # Write
    os.makedirs(os.path.dirname(output_path) or ".", exist_ok=True)
    with open(output_path, "w") as f:
        f.write(content)
        f.write("\n")

    total = len(weapon_entries) + len(ammo_entries)
    print(f"\nGenerated {output_path}")
    print(f"  CfgWeapons: {len(weapon_entries)} entries")
    print(f"  CfgAmmo:    {len(ammo_entries)} entries")
    print(f"  Total:      {total} entries")
    return total


# ── CLI ─────────────────────────────────────────────────────────────────────


def main():
    parser = argparse.ArgumentParser(
        description="Generate ABE Arma config entries from weapon/ammo JSON data",
    )
    script_dir = os.path.dirname(os.path.abspath(__file__))

    default_weapon = os.path.normpath(os.path.join(script_dir, "..", "weapons"))
    default_ammo = os.path.normpath(os.path.join(script_dir, "..", "ammo"))
    default_output = os.path.normpath(
        os.path.join(script_dir, "..", "generated", "abe_config.hpp")
    )

    parser.add_argument(
        "--weapon-dir",
        default=default_weapon,
        help="Path to data/weapons directory (default: ../weapons relative to script)",
    )
    parser.add_argument(
        "--ammo-dir",
        default=default_ammo,
        help="Path to data/ammo directory (default: ../ammo relative to script)",
    )
    parser.add_argument(
        "--output",
        default=default_output,
        help="Output .hpp file path (default: ../generated/abe_config.hpp)",
    )
    args = parser.parse_args()

    generate_config(
        weapon_dir=args.weapon_dir,
        ammo_dir=args.ammo_dir,
        output_path=args.output,
    )


if __name__ == "__main__":
    main()
