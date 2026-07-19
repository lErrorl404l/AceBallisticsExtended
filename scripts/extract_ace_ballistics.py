#!/usr/bin/env python3
"""
Extract weapon and ammo ballistics data from ACE3 CfgAmmo/CfgWeapons HPP configs
and generate ABE JSON data files for weapons/ammo not already covered.

Pipeline:
  1. Parse CfgWeapons.hpp for ACE_barrelLength/ACE_barrelTwist per weapon class
  2. Parse CfgAmmo.hpp for ACE_caliber/ACE_bulletMass/BC/dragModel/velocity curves
  3. Parse CfgMagazines.hpp for magazine→ammo mapping (via `ammo` field)
  4. Parse CfgMagazineWells.hpp for magazineWell→magazine list
  5. Cross-reference weapon→magazineWell→magazine→ammo chain
  6. Interpolate muzzle velocity at weapon barrel length from ammo curve
  7. Generate ABE JSONs only for weapons/ammo NOT already in data/

Usage:
    python3 scripts/extract_ace_ballistics.py
"""

import json
import os
import re
import sys
from pathlib import Path
from collections import defaultdict

# ── Paths ─────────────────────────────────────────────────────────────────
ACE_DIR = Path("/tmp/ace_unpack")
DATA_DIR = Path(os.path.dirname(__file__)).resolve().parent / "data"

# ── SQF/HPP Config Parser ────────────────────────────────────────────────
# Uses a line-based recursive state machine to parse SQF config files.
# Much simpler and more robust than the token+recursive descent approach.


def parse_hpp(text):
    """Parse SQF HPP config text into a nested dict structure.

    The SQF config format is:
      class Name[: Parent] {
          prop = value;
          prop[] = {val1, val2};
          prop[] += {val3};
          class Child: Parent { ... };
      };

    This parser returns: {"CfgWeapons": {"Weapon1": {...}, ...}}
    """
    # Strip comments
    text = re.sub(r"//[^\n]*", "", text)
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.DOTALL)

    # Normalize whitespace: collapse runs of whitespace into single space,
    # but preserve newlines for brace tracking
    text = re.sub(r"[ \t]+", " ", text)
    text = re.sub(r"\n\s*\n", "\n", text)

    result = {}
    stack = [("", result)]  # (class_name, dict)

    i = 0
    while i < len(text):
        ch = text[i]
        if ch in " \t\r\n":
            i += 1
            continue
        if text[i : i + 2] == "/*":
            j = text.find("*/", i + 2)
            i = j + 2 if j != -1 else len(text)
            continue

        # Class definition
        m = re.match(r"class\s+(\w+)\s*(?::\s*(\w+))?\s*{", text[i:], re.DOTALL)
        if m:
            class_name = m.group(1)
            parent = m.group(2)
            d = {"__parent__": parent} if parent else {}
            stack[-1][1][class_name] = d
            stack.append((class_name, d))
            i += m.end()
            continue

        # Closing brace
        if ch == "}":
            if len(stack) > 1:
                stack.pop()
            i += 1
            continue

        # Skip standalone semicolons (empty statements, end of class defs)
        if ch == ";":
            i += 1
            continue

        # Property definition: name[][+]= value;
        m = re.match(
            r"(\w+)"  # property name
            r"(?:\[\s*\])?"  # optional []
            r"(?:\s*\+\s*)?"  # optional +
            r"\s*=\s*",  # =
            text[i:],
            re.DOTALL,
        )
        if m:
            prop_name = m.group(1)
            i += m.end()

            # Check if this property has an array value: { ... }
            if text[i : i + 1] == "{":
                values = []
                i += 1
                depth = 1
                val_start = i
                while i < len(text) and depth > 0:
                    if text[i] == "{":
                        depth += 1
                    elif text[i] == "}":
                        depth -= 1
                    elif text[i] == '"':
                        i += 1
                        while i < len(text) and text[i] != '"':
                            if text[i] == "\\":
                                i += 1
                            i += 1
                    i += 1
                # parse the array content
                arr_text = text[val_start : i - 1]
                # split by commas, respecting quotes
                values = _parse_array_values(arr_text)
                # Store in current class
                target = stack[-1][1]
                if prop_name in target and isinstance(target[prop_name], list):
                    target[prop_name].extend(values)
                else:
                    target[prop_name] = values
                # Skip to semicolon
                while i < len(text) and text[i] != ";":
                    i += 1
                if i < len(text):
                    i += 1
                continue
            else:
                # Scalar value: read until ;
                val_start = i
                while i < len(text) and text[i] != ";":
                    if text[i] == '"':
                        i += 1
                        while i < len(text) and text[i] != '"':
                            if text[i] == "\\":
                                i += 1
                            i += 1
                    i += 1
                raw_val = text[val_start:i].strip()
                if i < len(text):
                    i += 1  # skip ;
                # Parse the value
                parsed_val = _parse_scalar(raw_val)
                stack[-1][1][prop_name] = parsed_val
                continue

        # Something else — skip forward
        i += 1

    # Return only the top-level classes (the first level inside the implicit root)
    return result


def _parse_array_values(text):
    """Parse comma-separated values from an SQF array body."""
    values = []
    current = []
    in_str = False
    i = 0
    while i < len(text):
        ch = text[i]
        if in_str:
            if ch == "\\" and i + 1 < len(text):
                current.append(text[i : i + 2])
                i += 2
                continue
            if ch == '"':
                in_str = False
                values.append("".join(current))
                current = []
                i += 1
                continue
            current.append(ch)
            i += 1
            continue
        if ch == '"':
            in_str = True
            i += 1
            continue
        if ch == ",":
            if current:
                val = "".join(current).strip()
                pv = _parse_scalar(val)
                if pv is not None:
                    values.append(pv)
                current = []
            i += 1
            continue
        if ch in " \t\n\r":
            if current:
                current.append(ch)
            i += 1
            continue
        current.append(ch)
        i += 1
    if current:
        val = "".join(current).strip()
        pv = _parse_scalar(val)
        if pv is not None:
            values.append(pv)
    return values


def _parse_scalar(val):
    """Parse a single scalar value (number, string, or identifier)."""
    val = val.strip()
    if not val:
        return None
    if val.startswith('"') and val.endswith('"'):
        return val[1:-1]
    # Try as number
    try:
        if "." in val or "e" in val.lower():
            return float(val)
        else:
            return int(val)
    except (ValueError, TypeError):
        pass
    # Boolean?
    if val == "true":
        return True
    if val == "false":
        return False
    # String identifier
    return val


# ── Chamber Pressure Estimation ──────────────────────────────────────────


def estimate_chamber_pressure(caliber_mm, bullet_mass_g):
    """Estimate chamber pressure in MPa based on caliber and bullet mass."""
    # Values from CIP/SAAMI/NATO EPVAT standards
    profiles = [
        (5.56, 3.5, 5.0, 430),  # 5.56mm NATO (M855)
        (5.45, 3.2, 4.5, 350),  # 5.45x39mm
        (7.62, 7.5, 12.0, 415),  # 7.62mm NATO
        (7.62, 6.0, 8.5, 360),  # 7.62x39mm
        (7.62, 9.0, 13.0, 415),  # 7.62x54R
        (9.0, 7.0, 10.0, 250),  # 9mm Parabellum
        (6.5, 7.0, 10.0, 420),  # 6.5mm Grendel/Creedmoor
        (12.7, 40.0, 60.0, 350),  # .50 BMG
        (0.338, 15.0, 20.0, 400),  # .338 Lapua
        (0.408, 25.0, 30.0, 400),  # .408 CheyTac
        (9.3, 15.0, 20.0, 380),  # 9.3mm
    ]
    for cal, mass_min, mass_max, pressure in profiles:
        if abs(caliber_mm - cal) < 0.5 and mass_min <= bullet_mass_g <= mass_max:
            return pressure
    # Default estimates by caliber range
    if caliber_mm < 6.0:
        return 380  # small bore rifle
    elif caliber_mm < 8.0:
        return 400  # intermediate/full-power rifle
    elif caliber_mm < 10.0:
        return 350  # large pistol/small rifle
    elif caliber_mm < 15.0:
        return 250  # heavy pistol
    else:
        return 350  # anti-materiel


def estimate_zero_range(caliber_mm, weapon_type):
    """Estimate zero range in metres."""
    if "pistol" in weapon_type.lower() or "smg" in weapon_type.lower():
        return 25
    elif (
        "sniper" in weapon_type.lower()
        or "dmr" in weapon_type.lower()
        or "lrr" in weapon_type.lower()
    ):
        return 100
    elif "mg" in weapon_type.lower() or "lm" in weapon_type.lower():
        return 300
    else:
        return 100  # default rifle


def estimate_effective_range(weapon_type, muzzle_velocity_ms):
    """Estimate effective range in metres."""
    if "pistol" in weapon_type.lower() or "smg" in weapon_type.lower():
        return 100
    elif "sniper" in weapon_type.lower() or "dmr" in weapon_type.lower():
        return 800
    elif "mg" in weapon_type.lower() or "lm" in weapon_type.lower():
        return 600
    elif "shotgun" in weapon_type.lower():
        return 50
    else:
        if muzzle_velocity_ms > 850:
            return 500
        else:
            return 400


def classify_weapon_type(class_name):
    """Classify weapon type from class name."""
    c = class_name.lower()
    if (
        "pistol" in c
        or "p07" in c
        or "p99" in c
        or "acp" in c
        or "deagle" in c
        or ".45" in c
    ):
        return "pistols"
    elif (
        "smg" in c
        or "pdw" in c
        or "mp5" in c
        or "mp7" in c
        or "vector" in c
        or "submachine" in c
    ):
        return "smgs"
    elif (
        "sniper" in c
        or "srifle" in c
        or "lrr" in c
        or "dmr" in c
        or "gm6" in c
        or "ebr" in c
        or "mar10" in c
    ):
        return "snipers"
    elif (
        "lg" in c
        or "mg" in c
        or "lm" in c
        or "m249" in c
        or "m240" in c
        or "rpk" in c
        or "minimi" in c
        or "negev" in c
        or "mk200" in c
    ):
        return "machine_guns"
    elif (
        "launch" in c
        or "rpg" in c
        or "at" in c
        or "titan" in c
        or "m72" in c
        or "maa" in c
        or "strela" in c
        or "igla" in c
        or "nlaw" in c
        or "pcml" in c
    ):
        return "launchers"
    elif "shotgun" in c or "sg" in c or "mplx" in c or "12ga" in c:
        return "shotguns"
    elif "carbine" in c or "short" in c or "mxc" in c or "mk20c" in c or "trg20" in c:
        return "rifles"  # carbines grouped with rifles
    else:
        return "rifles"


def interpolate_mv(muzzle_velocities, barrel_lengths, weapon_barrel_mm):
    """Interpolate muzzle velocity at a given barrel length."""
    if not muzzle_velocities or not barrel_lengths:
        return None
    if len(muzzle_velocities) != len(barrel_lengths):
        return None
    if len(muzzle_velocities) == 1:
        return muzzle_velocities[0]

    # Sort by barrel length
    pairs = sorted(zip(barrel_lengths, muzzle_velocities))
    bl_vec = [p[0] for p in pairs]
    mv_vec = [p[1] for p in pairs]

    # Clamp to range
    if weapon_barrel_mm <= bl_vec[0]:
        return mv_vec[0]
    if weapon_barrel_mm >= bl_vec[-1]:
        return mv_vec[-1]

    # Linear interpolation
    for i in range(len(bl_vec) - 1):
        if bl_vec[i] <= weapon_barrel_mm <= bl_vec[i + 1]:
            t = (weapon_barrel_mm - bl_vec[i]) / (bl_vec[i + 1] - bl_vec[i])
            return mv_vec[i] + t * (mv_vec[i + 1] - mv_vec[i])

    return None


def initSpeed_to_ms(init_speed, barrel_length_mm):
    """Convert ACE3 initSpeed (modifier or fixed) to muzzle velocity in m/s.

    ACE3 initSpeed conventions:
      - Positive value: fixed muzzle velocity in m/s
      - Negative value: multiplier applied to ammo's base muzzle velocity
         (from magazine's initSpeed or ammo reference velocity)
      - initSpeed -1.0 = use ammo's standard muzzle velocity at this barrel length
      - initSpeed -0.979444 = multiply ammo MV by 0.979444
    """
    if init_speed >= 0:
        return init_speed
    return abs(init_speed)  # Will be applied as a multiplier later


# ── Main Extraction ──────────────────────────────────────────────────────


def load_existing_jsons():
    """Build set of existing weapon class names from data/weapons/*/*.json."""
    existing_weapons = set()
    existing_ammo = set()
    weapons_dir = DATA_DIR / "weapons"
    if weapons_dir.exists():
        for cat_dir in weapons_dir.iterdir():
            if cat_dir.is_dir():
                for f in cat_dir.iterdir():
                    if f.suffix == ".json":
                        try:
                            with open(f) as fh:
                                data = json.load(fh)
                                if "class" in data:
                                    existing_weapons.add(data["class"])
                        except (json.JSONDecodeError, KeyError):
                            existing_weapons.add(f.stem)
    ammo_dir = DATA_DIR / "ammo"
    if ammo_dir.exists():
        for cal_dir in ammo_dir.iterdir():
            if cal_dir.is_dir():
                for f in cal_dir.iterdir():
                    if f.suffix == ".json":
                        try:
                            with open(f) as fh:
                                data = json.load(fh)
                                if "class" in data:
                                    existing_ammo.add(data["class"])
                        except (json.JSONDecodeError, KeyError):
                            pass
    return existing_weapons, existing_ammo


def resolve_ammo_class(mag_class, mags_data, ammo_data):
    """Find the ammo class for a given magazine class by walking parent chain."""
    mag_entry = mags_data.get(mag_class, {})
    if isinstance(mag_entry, dict) and "ammo" in mag_entry:
        return mag_entry["ammo"]
    # Walk parent chain
    parent = mag_entry.get("__parent__", "")
    while parent:
        parent_entry = mags_data.get(parent, {})
        if isinstance(parent_entry, dict) and "ammo" in parent_entry:
            return parent_entry["ammo"]
        parent = (
            parent_entry.get("__parent__", "") if isinstance(parent_entry, dict) else ""
        )
    return None


def extract_magazine_wells(weapon_entry, weapon_class):
    """Extract magazineWells from a weapon config entry."""
    magwells = []
    for key in weapon_entry:
        if key == "magazineWell" or key.startswith("magazineWell"):
            wells = weapon_entry[key]
            if isinstance(wells, list):
                magwells.extend(wells)
            elif isinstance(wells, str):
                magwells.append(wells)
    return magwells


def main():
    print("=== ACE3 ABE Ballistics Data Extraction ===")
    print()

    # Load existing data
    existing_weapons, existing_ammo = load_existing_jsons()
    print(f"Existing weapon JSONs: {len(existing_weapons)} unique classes")
    print(f"Existing ammo JSONs:   {len(existing_ammo)} unique classes")
    print()

    # Parse HPP files
    print("Parsing CfgAmmo.hpp...")
    with open(ACE_DIR / "CfgAmmo.hpp") as f:
        ammo_raw = parse_hpp(f.read())
    ammo_cfg = ammo_raw.get("CfgAmmo", {})
    print(f"  Found {len(ammo_cfg)} ammo class entries")
    # Filter to entries with ACE ballistics data
    ace_ammo = {}
    for name, entry in ammo_cfg.items():
        if isinstance(entry, dict) and (
            "ACE_caliber" in entry or "ACE_ballisticCoefficients" in entry
        ):
            ace_ammo[name] = entry
    print(f"  ACE ballistics entries: {len(ace_ammo)}")

    print()
    print("Parsing CfgWeapons.hpp...")
    with open(ACE_DIR / "CfgWeapons.hpp") as f:
        weapons_raw = parse_hpp(f.read())
    weapons_cfg = weapons_raw.get("CfgWeapons", {})
    print(f"  Found {len(weapons_cfg)} weapon class entries")
    # Filter to entries with ACE_barrelLength
    ace_weapons = []
    for name, entry in weapons_cfg.items():
        if isinstance(entry, dict) and "ACE_barrelLength" in entry:
            ace_weapons.append((name, entry))
    print(f"  ACE ballistics entries: {len(ace_weapons)}")

    print()
    print("Parsing CfgMagazines.hpp...")
    with open(ACE_DIR / "CfgMagazines.hpp") as f:
        mags_raw = parse_hpp(f.read())
    mags_cfg = mags_raw.get("CfgMagazines", {})
    print(f"  Found {len(mags_cfg)} magazine entries")

    print()
    print("Parsing CfgMagazineWells.hpp...")
    with open(ACE_DIR / "CfgMagazineWells.hpp") as f:
        wells_raw = parse_hpp(f.read())
    wells_cfg = wells_raw.get("CfgMagazineWells", {})
    # Extract magazine lists per well
    well_mags = {}
    for well_name, well_data in wells_cfg.items():
        if isinstance(well_data, dict):
            for key in ("ADDON", "magazines", "wells"):
                if key in well_data:
                    mags = well_data[key]
                    if isinstance(mags, list):
                        if well_name not in well_mags:
                            well_mags[well_name] = []
                        well_mags[well_name].extend(mags)
                    break
    print(f"  Magazine wells with ACE mags: {len(well_mags)}")

    # ── Generate Weapon JSONs ────────────────────────────────────────
    print()
    print("=" * 60)
    print("GENERATING WEAPON JSONS")
    print("=" * 60)

    weapons_generated = 0
    weapons_skipped = 0
    weapons_no_ammo = 0

    for wname, wentry in ace_weapons:
        if wname in existing_weapons:
            weapons_skipped += 1
            continue

        # Extract ACE data
        barrel_length = wentry.get("ACE_barrelLength", None)
        if barrel_length is None:
            continue
        barrel_length = (
            float(barrel_length)
            if not isinstance(barrel_length, (int, float))
            else float(barrel_length)
        )

        twist = wentry.get("ACE_barrelTwist", None)
        if twist is not None:
            twist = (
                float(twist) if not isinstance(twist, (int, float)) else float(twist)
            )
        else:
            twist = 0.0

        init_speed_raw = wentry.get("initSpeed", None)
        init_speed_mod = 1.0
        fixed_mv = None
        if init_speed_raw is not None:
            init_speed_raw = (
                float(init_speed_raw)
                if not isinstance(init_speed_raw, (int, float))
                else float(init_speed_raw)
            )
            if init_speed_raw > 0:
                fixed_mv = init_speed_raw
            elif init_speed_raw < 0:
                init_speed_mod = abs(init_speed_raw)

        # Magazine wells → magazines → ammo
        magwells = extract_magazine_wells(wentry, wname)
        if not magwells:
            # Try vanilla magwell from class name patterns
            weapons_no_ammo += 1
            continue

        # Find ammo from first available magazine
        resolved_ammo_class = None
        best_mv = None
        best_caliber = None
        best_mass = None
        best_bc = None
        best_drag_model = None

        for mw in magwells:
            if mw not in well_mags:
                continue
            for mag_name in well_mags[mw]:
                ammo_class = resolve_ammo_class(mag_name, mags_cfg, ace_ammo)
                if ammo_class and ammo_class in ace_ammo:
                    resolved_ammo_class = ammo_class
                    ammo_entry = ace_ammo[ammo_class]

                    # Extract ammo data
                    caliber = ammo_entry.get("ACE_caliber", None)
                    mass = ammo_entry.get("ACE_bulletMass", None)
                    bc_list = ammo_entry.get("ACE_ballisticCoefficients", None)
                    drag_model = ammo_entry.get("ACE_dragModel", None)
                    mv_list = ammo_entry.get("ACE_muzzleVelocities", None)
                    bl_list = ammo_entry.get("ACE_barrelLengths", None)

                    if caliber is not None:
                        best_caliber = (
                            float(caliber)
                            if not isinstance(caliber, (int, float))
                            else float(caliber)
                        )
                    if mass is not None:
                        best_mass = (
                            float(mass)
                            if not isinstance(mass, (int, float))
                            else float(mass)
                        )
                    if bc_list and isinstance(bc_list, list) and len(bc_list) > 0:
                        bc_val = bc_list[0]
                        best_bc = (
                            float(bc_val)
                            if not isinstance(bc_val, (int, float))
                            else float(bc_val)
                        )
                    if drag_model is not None:
                        best_drag_model = (
                            int(drag_model)
                            if not isinstance(drag_model, (int, float))
                            else int(drag_model)
                        )

                    # Compute muzzle velocity
                    if fixed_mv is not None:
                        mv = fixed_mv
                    elif mv_list and bl_list:
                        if isinstance(mv_list, list) and isinstance(bl_list, list):
                            mv_vals = [
                                float(v)
                                if not isinstance(v, (int, float))
                                else float(v)
                                for v in mv_list
                            ]
                            bl_vals = [
                                float(v)
                                if not isinstance(v, (int, float))
                                else float(v)
                                for v in bl_list
                            ]
                            mv_interp = interpolate_mv(mv_vals, bl_vals, barrel_length)
                            if mv_interp is not None:
                                mv = mv_interp * init_speed_mod
                            else:
                                continue
                        else:
                            continue
                    else:
                        # Fallback: no MV data
                        continue

                    best_mv = mv
                    break  # Use first matching ammo

            if resolved_ammo_class:
                break

        if resolved_ammo_class is None:
            weapons_no_ammo += 1
            continue

        if best_mv is None:
            weapons_no_ammo += 1
            continue

        # Estimate missing parameters
        if best_caliber is None:
            best_caliber = 5.56  # default
        if best_mass is None or best_mass == 0:
            best_mass = 4.0  # default
        chamber_pressure = estimate_chamber_pressure(best_caliber, best_mass)
        weapon_type = classify_weapon_type(wname)
        zero_range = estimate_zero_range(best_caliber, weapon_type)
        effective_range = estimate_effective_range(weapon_type, best_mv)

        cdm_map = {1: "g1", 7: "g7", 8: "g8"}
        cdm_id = cdm_map.get(best_drag_model or 7, "g7")

        # Determine subdirectory
        subdir = DATA_DIR / "weapons" / weapon_type
        subdir.mkdir(parents=True, exist_ok=True)

        # Build JSON
        weapon_json = {
            "class": wname,
            "caliber_mm": best_caliber,
            "barrel_length_mm": barrel_length,
            "rifling_twist_mm": twist,
            "chamber_pressure_mpa": chamber_pressure,
            "cdm_id": cdm_id,
            "projectile_mass_g": round(best_mass, 2),
            "muzzle_velocity_ms": round(best_mv, 1),
            "zero_range_m": zero_range,
            "effective_range_m": effective_range,
            "notes": f"Extracted from ACE3 ace_ballistics. Caliber/BC from CfgAmmo::{resolved_ammo_class}. Chamber pressure estimated by caliber type.",
        }

        filename = subdir / f"{wname.lower()}.json"
        with open(filename, "w") as f:
            json.dump(weapon_json, f, indent=2)
        weapons_generated += 1
        print(
            f"  [+{weapons_generated:3d}] {wname} ({best_caliber}mm, {barrel_length}mm barrel, {best_mv:.0f} m/s)"
        )

    # ── Generate Ammo JSONs ──────────────────────────────────────────
    print()
    print("=" * 60)
    print("GENERATING AMMO JSONS")
    print("=" * 60)

    ammo_generated = 0
    ammo_skipped = 0

    # Caliber directory map
    caliber_dir_map = {}
    for cal_dir in (DATA_DIR / "ammo").iterdir():
        if cal_dir.is_dir():
            caliber_dir_map[cal_dir.name] = cal_dir

    for aname, aentry in ace_ammo.items():
        if aname in existing_ammo:
            ammo_skipped += 1
            continue

        # Extract ammo data
        caliber = aentry.get("ACE_caliber", None)
        if caliber is None:
            continue
        caliber = (
            float(caliber) if not isinstance(caliber, (int, float)) else float(caliber)
        )

        mass = aentry.get("ACE_bulletMass", None)
        if mass is not None:
            mass = float(mass) if not isinstance(mass, (int, float)) else float(mass)

        bc_list = aentry.get("ACE_ballisticCoefficients", None)
        bc_val = None
        if bc_list and isinstance(bc_list, list) and len(bc_list) > 0:
            bc_list_inner = bc_list[0] if isinstance(bc_list[0], list) else bc_list
            bv = (
                bc_list_inner[0]
                if isinstance(bc_list_inner, (list, tuple))
                else bc_list_inner
            )
            bc_val = float(bv) if not isinstance(bv, (int, float)) else float(bv)

        drag_model = aentry.get("ACE_dragModel", None)
        cdm_map = {1: "g1", 7: "g7", 8: "g8"}
        cdm_id = cdm_map.get(drag_model, "g7") if drag_model is not None else "g7"

        mv_list = aentry.get("ACE_muzzleVelocities", None)
        bl_list = aentry.get("ACE_barrelLengths", None)

        # Determine caliber directory
        cal_dir_name = None
        if caliber < 5.0:
            continue  # Skip very small calibers (pellets, etc)
        elif caliber <= 5.6:
            cal_dir_name = "5_45mm"
        elif caliber < 6.0:
            cal_dir_name = "5_56mm"
        elif caliber < 7.0:
            cal_dir_name = "6_5mm"
        elif caliber < 8.0:
            cal_dir_name = "7_62mm"
        elif caliber < 9.0:
            cal_dir_name = "8mm"  # .338, 9x39 etc
        elif caliber < 10.0:
            # 9mm handgun vs 9.3mm rifle
            if mass is not None and mass > 10.0:
                cal_dir_name = "rifle"  # 9.3x64
            else:
                cal_dir_name = "handgun"
        elif caliber < 13.0:
            cal_dir_name = "heavy_127mm"  # .408, .50
        elif caliber < 15.0:
            cal_dir_name = "heavy_127mm"  # 12.7mm
        else:
            cal_dir_name = "launcher"  # rockets, shells

        # Ensure directory exists
        ammo_subdir = DATA_DIR / "ammo" / cal_dir_name
        ammo_subdir.mkdir(parents=True, exist_ok=True)

        # Build filename
        # Use a sanitized version of the class name
        safe_name = aname.lower().replace(" ", "_").replace("/", "_")
        filename = ammo_subdir / f"{safe_name}.json"

        # If file already exists (different class name, same file), skip
        if filename.exists():
            ammo_skipped += 1
            continue

        # Build ammo JSON
        ammo_json = {
            "class": aname,
            "projectile": {
                "model": safe_name,
                "caliber_mm": caliber,
            },
            "chamber_pressure_mpa": 0,
            "notes": f"Extracted from ACE3 ace_ballistics CfgAmmo::{aname}.",
        }

        if mass is not None:
            ammo_json["projectile"]["mass_g"] = round(mass, 4)

        # Try reference velocity from middle barrel length
        ref_mv = None
        if (
            mv_list
            and bl_list
            and isinstance(mv_list, list)
            and isinstance(bl_list, list)
        ):
            mid_idx = len(mv_list) // 2
            ref_mv = (
                float(mv_list[mid_idx])
                if not isinstance(mv_list[mid_idx], (int, float))
                else float(mv_list[mid_idx])
            )
        if ref_mv:
            ammo_json["projectile"]["muzzle_velocity_ms"] = round(ref_mv, 1)

        # BC
        if bc_val:
            if cdm_id == "g7":
                ammo_json["projectile"]["bc_g7"] = round(bc_val, 3)
            elif cdm_id == "g1":
                ammo_json["projectile"]["bc_g1"] = round(bc_val, 3)
            else:
                ammo_json["projectile"]["bc_g8"] = round(bc_val, 3)
            ammo_json["projectile"]["cdm_id"] = cdm_id

        # Source block
        ammo_json["projectile"]["source"] = {
            "type": "reference_data",
            "reference": f"ACE3 ace_ballistics CfgAmmo::{aname}",
            "methodology": "Extracted from ACE3 ballistics config. Values as published by ACE3 team, originally sourced from manufacturer/DoD references.",
            "confidence": "high",
        }

        with open(filename, "w") as f:
            json.dump(ammo_json, f, indent=2)
        ammo_generated += 1
        print(
            f"  [+{ammo_generated:3d}] {aname} ({caliber}mm, BC={bc_val}, cdm={cdm_id})"
        )

    # ── Summary ──────────────────────────────────────────────────────
    print()
    print("=" * 60)
    print("SUMMARY")
    print("=" * 60)
    print(f"  Weapons generated: {weapons_generated}")
    print(f"  Weapons skipped (already exist): {weapons_skipped}")
    print(f"  Weapons skipped (no ammo chain): {weapons_no_ammo}")
    print(f"  Ammo generated:    {ammo_generated}")
    print(f"  Ammo skipped:      {ammo_skipped}")
    print()

    return 0


if __name__ == "__main__":
    sys.exit(main())
