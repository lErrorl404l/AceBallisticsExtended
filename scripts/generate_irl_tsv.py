#!/usr/bin/env python3
"""Generate data/ir_weapons.tsv and data/ir_ammo.tsv from existing weapon/ammo JSONs.

Each TSV row becomes a phf::Map entry in the Rust binary via build.rs.
Keys are normalised class-name fragments used for substring matching.
"""

import json
import os
import re
import sys

DATA_DIR = os.path.join(os.path.dirname(__file__), "..", "data")


def normalize_class(name: str) -> str:
    """Match the Rust normalize() logic — strip Arma/mod prefixes, remove _ and -."""
    s = name.lower()

    # Strip weapon type prefixes
    for prefix in [
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
    ]:
        if s.startswith(prefix):
            s = s[len(prefix) :]
            break

    # Strip mod prefixes
    for prefix in [
        "rhs_weap_",
        "rhs_",
        "vn_",
        "gm_",
        "spe_",
        "csla_",
        "ace_",
        "ef_",
        "us85_",
        "fia_",
        "cwr_",
        "ws_",
        "rf_",
        "soe_",
    ]:
        if s.startswith(prefix):
            s = s[len(prefix) :]
            break

    return s.replace("_", "").replace("-", "")


def derive_weapon_key(data: dict, filename: str) -> str | None:
    """Derive a PHF map key from weapon JSON data or filename.

    Priority: explicit model/class fields -> stripped class name -> filename stem.
    Returns None for items without enough data (launchers, fake weapons).
    """
    # Skip launchers / fake weapons
    cls = data.get("class", "")
    if any(
        kw in cls.lower() for kw in ["fakeweapon", "launch_", "fake_weapon", "module"]
    ):
        return None

    # Use the class name if available
    if cls:
        cleaned = normalize_class(cls)
        if len(cleaned) >= 3:
            return cleaned

    # Fallback: filename stem
    stem = os.path.splitext(filename)[0]
    cleaned = normalize_class(stem)
    if len(cleaned) >= 3:
        return cleaned

    return None


def derive_ammo_key(data: dict, filename: str) -> str | None:
    """Derive a PHF map key from ammo JSON data."""
    cls = data.get("class", "")
    if cls:
        cleaned = normalize_class(cls)
        if len(cleaned) >= 3:
            return cleaned

    stem = os.path.splitext(filename)[0]
    cleaned = normalize_class(stem)
    if len(cleaned) >= 3:
        return cleaned

    return None


def collect_weapons() -> list[dict]:
    """Scan data/weapons/ for JSON files and extract weapon params."""
    weapons = []
    weapons_dir = os.path.join(DATA_DIR, "weapons")

    for root, _dirs, files in os.walk(weapons_dir):
        for fname in sorted(files):
            if not fname.endswith(".json"):
                continue
            fpath = os.path.join(root, fname)
            try:
                with open(fpath) as f:
                    data = json.load(f)
            except (json.JSONDecodeError, OSError):
                continue

            key = derive_weapon_key(data, fname)
            if key is None:
                continue

            cal = data.get("caliber_mm", 0) or 0
            barrel = data.get("barrel_length_mm", 0) or 0
            twist = data.get("rifling_twist_mm", 0) or 0
            pressure = data.get("chamber_pressure_mpa", 0) or 0
            mass = data.get("projectile_mass_g", 0) or 0

            # Skip entries missing critical values
            if barrel <= 0 or cal <= 0:
                continue

            weapons.append(
                {
                    "key": key,
                    "caliber_mm": cal,
                    "barrel_length_mm": barrel,
                    "barrel_twist_mm": twist,
                    "chamber_pressure_mpa": pressure,
                    "projectile_mass_g": mass,
                    "class": data.get("class", ""),
                }
            )

    return weapons


def collect_ammo() -> list[dict]:
    """Scan data/ammo/ for JSON files and extract ammo params."""
    ammo_list = []
    ammo_dir = os.path.join(DATA_DIR, "ammo")

    for root, _dirs, files in os.walk(ammo_dir):
        for fname in sorted(files):
            if not fname.endswith(".json"):
                continue
            fpath = os.path.join(root, fname)
            try:
                with open(fpath) as f:
                    data = json.load(f)
            except (json.JSONDecodeError, OSError):
                continue

            key = derive_ammo_key(data, fname)
            if key is None:
                continue

            # Extract projectile block
            proj = data.get("projectile") or {}
            if not proj:
                continue

            diameter = data.get("caliber_mm", proj.get("caliber_mm", 0)) or 0
            mass = proj.get("mass_g", 0) or 0
            bc_g1 = proj.get("bc_g1", 0) or 0
            bc_g7 = proj.get("bc_g7", 0) or 0
            cdm = proj.get("cdm_id", "")

            drag_model = 7  # default to G7
            if cdm.lower() in ("g1", "1"):
                drag_model = 1
            elif cdm.lower() in ("g7", "7"):
                drag_model = 7
            elif cdm.lower() in ("g8", "8"):
                drag_model = 8
            elif bc_g1 > 0 and bc_g7 <= 0:
                drag_model = 1  # G1 BC but no G7 BC → G1

            if diameter <= 0 or mass <= 0:
                continue

            ammo_list.append(
                {
                    "key": key,
                    "diameter_mm": diameter,
                    "mass_g": mass,
                    "bc_g1": bc_g1,
                    "bc_g7": bc_g7,
                    "drag_model": drag_model,
                    "class": data.get("class", ""),
                }
            )

    return ammo_list


def deduplicate(items: list[dict]) -> list[dict]:
    """Keep the entry with the most populated fields per key.

    When two JSONs map to the same key (e.g. arifle_MX_F.json and mx_6_5mm.json),
    keep the one with more non-zero fields.
    """
    best: dict[str, dict] = {}
    for item in items:
        key = item["key"]
        score = sum(1 for k, v in item.items() if isinstance(v, (int, float)) and v > 0)
        if key not in best or score > best[key].get("_score", 0):
            best[key] = item
            best[key]["_score"] = score
    for v in best.values():
        v.pop("_score", None)
    return list(best.values())


def write_weapons_tsv(weapons: list[dict], path: str):
    """Write ir_weapons.tsv — key, caliber, barrel, twist, pressure, mass."""
    header = "# model\tcaliber_mm\tbarrel_length_mm\tbarrel_twist_mm\tchamber_pressure_mpa\tprojectile_mass_g"
    lines = [header]

    for w in sorted(weapons, key=lambda x: x["key"]):
        lines.append(
            f"{w['key']}\t{w['caliber_mm']}\t{w['barrel_length_mm']}\t"
            f"{w['barrel_twist_mm']}\t{w['chamber_pressure_mpa']}\t{w['projectile_mass_g']}"
        )

    with open(path, "w") as f:
        f.write("\n".join(lines) + "\n")
    print(f"Wrote {len(weapons)} weapon entries to {path}")


def write_ammo_tsv(ammo_list: list[dict], path: str):
    """Write ir_ammo.tsv — key, diameter, mass, bc_g1, bc_g7, drag_model."""
    header = "# model\tbullet_diameter_mm\tprojectile_mass_g\tbc_g1\tbc_g7\tdrag_model"
    lines = [header]

    for a in sorted(ammo_list, key=lambda x: x["key"]):
        lines.append(
            f"{a['key']}\t{a['diameter_mm']}\t{a['mass_g']}\t"
            f"{a['bc_g1']}\t{a['bc_g7']}\t{a['drag_model']}"
        )

    with open(path, "w") as f:
        f.write("\n".join(lines) + "\n")
    print(f"Wrote {len(ammo_list)} ammo entries to {path}")


def main():
    print("Collecting weapon data...")
    weapons = collect_weapons()
    print(f"  Raw: {len(weapons)} entries")
    weapons = deduplicate(weapons)
    print(f"  Deduplicated: {len(weapons)} unique weapons")
    write_weapons_tsv(weapons, os.path.join(DATA_DIR, "ir_weapons.tsv"))

    print("\nCollecting ammo data...")
    ammo_list = collect_ammo()
    print(f"  Raw: {len(ammo_list)} entries")
    ammo_list = deduplicate(ammo_list)
    print(f"  Deduplicated: {len(ammo_list)} unique ammo")
    write_ammo_tsv(ammo_list, os.path.join(DATA_DIR, "ir_ammo.tsv"))

    print("\nDone.")


if __name__ == "__main__":
    main()
