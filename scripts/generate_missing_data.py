#!/usr/bin/env python3
"""
Generate ABE JSON data files from Arma 3+RPT dump data for weapons,
ammo types, and vehicles that exist in the game but are NOT yet in data/.

Pipeline:
  1. Parse the RPT dump (pipe-delimited lines from dump_arma_configs.sqf)
  2. Load existing data/ to determine what's missing
  3. Generate ammo JSONs from ACE3 ballistics fields (highest priority)
  4. Generate weapon JSONs from ACE3 physical params + caliber inference
  5. Generate vehicle JSONs for combat vehicles with armor data

Usage:
    python3 scripts/generate_missing_data.py
"""

import json
import os
import re
import sys
from collections import defaultdict
from pathlib import Path

# ── Paths ─────────────────────────────────────────────────────────────────
DATA_DIR = Path(os.path.dirname(__file__)).resolve().parent / "data"
DUMP_FILE = Path("/tmp/abe_dump_data.txt")

# ── Caliber inference from weapon class names ────────────────────────────
# Matches known Arma 3 caliber conventions
CALIBER_MAP = {
    # Arma 3 base weapons
    "MX": ("6.5mm", 6.5, "g7", 0.260, 400.0),
    "Katiba": ("6.5mm", 6.5, "g7", 0.260, 400.0),
    "CAR": ("6.5mm", 6.5, "g7", 0.260, 400.0),
    "Mk20": ("5.56mm", 5.56, "g1", 0.151, 380.0),
    "TRG": ("5.56mm", 5.56, "g1", 0.151, 380.0),
    "SDAR": ("5.56mm", 5.56, "g1", 0.151, 380.0),
    "SPAR": ("5.56mm", 5.56, "g1", 0.151, 380.0),
    "MSBS": ("5.56mm", 5.56, "g1", 0.151, 380.0),
    "AK": ("7.62mm", 7.62, "g7", 0.200, 380.0),
    "AKS": ("7.62mm", 7.62, "g7", 0.200, 380.0),
    "AKM": ("7.62mm", 7.62, "g7", 0.200, 380.0),
    "AK12": ("5.45mm", 5.45, "g7", 0.170, 380.0),
    "RPK": ("7.62mm", 7.62, "g7", 0.200, 380.0),
    # Sniper rifles
    "GM6": ("12.7mm", 12.7, "g7", 0.670, 400.0),
    "LRR": ("7.62mm", 7.62, "g7", 0.243, 380.0),
    "GM6": ("12.7mm", 12.7, "g7", 0.670, 350.0),
    "M320": ("7.62mm", 7.62, "g7", 0.243, 380.0),
    "EBR": ("7.62mm", 7.62, "g7", 0.243, 380.0),
    "DMR": ("7.62mm", 7.62, "g7", 0.243, 400.0),
    "Mk18": ("7.62mm", 7.62, "g7", 0.243, 380.0),
    "Mk14": ("7.62mm", 7.62, "g7", 0.243, 380.0),
    "CYRUS": ("9.3mm", 9.3, "g7", 0.300, 400.0),
    # SMG
    "SMG": ("9mm", 9.0, "g1", 0.165, 250.0),
    "PDW": ("9mm", 9.0, "g1", 0.165, 250.0),
    "Sting": ("9mm", 9.0, "g1", 0.165, 250.0),
    "Vermin": (".45 ACP", 11.43, "g1", 0.155, 230.0),
    # LMG
    "LIM": ("6.5mm", 6.5, "g7", 0.260, 380.0),
    "Zafir": ("7.62mm", 7.62, "g7", 0.200, 380.0),
    "Navid": ("9.3mm", 9.3, "g7", 0.300, 400.0),
    "MG": ("7.62mm", 7.62, "g7", 0.200, 380.0),
    # Shotguns
    "sgun_": ("12ga", 18.5, "g1", 0.040, 350.0),
    # Pistols
    "pistol_": ("9mm", 9.0, "g1", 0.165, 250.0),
    # Launchers
    "launch": ("40mm", 40.0, "g1", 0.000, 150.0),
}

# Chamber pressure by caliber class (MPa)
CHAMBER_PRESSURE = {
    "5.45mm": 355.0,
    "5.56mm": 380.0,
    "6.5mm": 400.0,
    "7.62mm": 380.0,
    "9mm": 250.0,
    ".45 ACP": 230.0,
    "12.7mm": 400.0,
    "9.3mm": 440.0,
    "12ga": 70.0,
    "40mm": 0.0,
}


def strip_timestamp(line):
    """Remove RPT timestamp prefix (e.g. '23:01:08 ') from a log line."""
    return re.sub(r"^\d{2}:\d{2}:\d{2}\s+", "", line)


def parse_dump():
    """Parse the full RPT dump into categorized dicts."""
    weapons = {}
    ace_weapons = {}
    ammo = {}
    ace_ammo = {}
    mags = {}
    magwells = {}
    vehicles = {}
    ace_vehicles = {}

    with open(DUMP_FILE) as f:
        for raw_line in f:
            line = strip_timestamp(raw_line.strip())
            if not line:
                continue

            parts = line.split("|")
            prefix = parts[0]

            if prefix == "WPN" and len(parts) >= 9:
                cls = parts[1]
                rec = {
                    "parent": parts[2],
                    "initSpeed": float(parts[3]),
                    "barrel": float(parts[4]),
                    "twist": float(parts[5]),
                    "rail": float(parts[6]),
                    "muzzleMoment": float(parts[7]),
                    "magwells": parts[8],
                }
                weapons[cls] = rec
                if rec["barrel"] != 0 or rec["twist"] != 0:
                    ace_weapons[cls] = rec

            elif prefix == "AMMO" and len(parts) >= 14:
                cls = parts[1]
                rec = {
                    "caliber": float(parts[2]),
                    "mass": float(parts[3]),
                    "hit": float(parts[4]),
                    "indirectHit": float(parts[5]),
                    "ace_caliber": float(parts[6]),
                    "ace_mass": float(parts[7]),
                    "ace_bcs": parts[8],
                    "ace_dragModel": int(float(parts[9])),
                    "ace_velocities": parts[10],
                    "ace_barrelLengths": parts[11],
                    "ace_standardAtm": int(float(parts[12])),
                    "ace_tempShift": parts[13],
                }
                # Parse array strings
                rec["ace_bc_list"] = _parse_array(rec["ace_bcs"])
                rec["ace_vel_list"] = _parse_array(rec["ace_velocities"])
                rec["ace_bl_list"] = _parse_array(rec["ace_barrelLengths"])
                ammo[cls] = rec
                if rec["ace_caliber"] != 0 or rec["ace_mass"] != 0:
                    ace_ammo[cls] = rec

            elif prefix == "MAG" and len(parts) >= 6:
                mags[parts[1]] = {
                    "ammo_class": parts[2],
                    "initSpeed": float(parts[3]),
                    "count": int(float(parts[4])),
                    "is_belt": int(float(parts[5])),
                    "restrict_caliber": parts[6],
                }

            elif prefix == "MAGWELL" and len(parts) >= 3:
                magwells[parts[1]] = _parse_array(parts[2])

            elif prefix == "VEH" and len(parts) >= 8:
                cls = parts[1]
                rec = {
                    "parent": parts[2],
                    "armor": float(parts[3]),
                    "armorStructural": float(parts[4]),
                    "cargoIsCoDriver": int(float(parts[5])),
                    "ace_armorClass": parts[6],
                    "ace_hitpointArmor": parts[7],
                    "ace_canUseAces": int(float(parts[8])),
                }
                vehicles[cls] = rec
                if rec["armor"] > 0 or rec["armorStructural"] > 0:
                    ace_vehicles[cls] = rec

    return weapons, ace_weapons, ammo, ace_ammo, mags, magwells, vehicles, ace_vehicles


def _parse_array(s):
    """Parse SQF array string like '[1,2,3]' or '["a","b"]' into Python list."""
    s = s.strip()
    if not s.startswith("[") or not s.endswith("]"):
        return []
    # Remove brackets
    inner = s[1:-1].strip()
    if not inner:
        return []
    # Handle quoted strings
    if '"' in inner:
        return [x.strip().strip('"') for x in inner.split(",")]
    # Numeric
    return [float(x.strip()) for x in inner.split(",")]


def infer_caliber(class_name):
    """Infer caliber and ballistics from weapon class name."""
    # Remove camo/variant suffixes
    base = re.sub(
        r"(_(black|blk|khk|tna|wdl|hex|green|coyote|sand|arid|snd|lush|olive))(_F)?$",
        "",
        class_name,
    )

    # Direct lookups in CALIBER_MAP
    for key, (cal_type, cal_mm, cdm, bc, pressure) in CALIBER_MAP.items():
        if key in class_name:
            return cal_type, cal_mm, cdm, bc, CHAMBER_PRESSURE.get(cal_type, pressure)

    # Prefix-based inference
    if class_name.startswith("sgun_"):
        return "12ga", 18.5, "g1", 0.040, 70.0
    if class_name.startswith("pistol_") or class_name.startswith("hgun_"):
        return "9mm", 9.0, "g1", 0.165, 250.0
    if class_name.startswith("launch_"):
        return "40mm", 40.0, "g1", 0.000, 0.0

    # Unknown — flag it
    return None, 0.0, "g7", 0.150, 380.0


def infer_projectile_mass(caliber_mm, weapon_type=""):
    """Default mass for caliber when unknown."""
    masses = {
        5.45: 3.4,
        5.56: 4.0,
        6.5: 8.0,
        7.62: 9.5,
        9.0: 8.0,
        11.43: 14.9,
        12.7: 41.9,
        9.3: 16.2,
        18.5: 28.0,
    }
    return masses.get(caliber_mm, 10.0)


def get_caliber_dir(caliber_mm):
    """Map caliber mm to directory name, matching existing conventions."""
    if caliber_mm <= 0:
        return "unknown"
    if caliber_mm < 5.0:
        return "handgun"
    if caliber_mm <= 5.6:
        return "5_56mm"
    if caliber_mm <= 6.8:
        return "6_5mm"
    if caliber_mm <= 8.5:
        return "7_62mm"
    if caliber_mm <= 9.5:
        return "9mm"
    if caliber_mm <= 11.5:
        return "handgun"
    if caliber_mm <= 14.0:
        return "heavy_127mm"
    return "heavy_127mm"


def load_existing_classes():
    """Load all existing ABE data class names."""
    weapons = {}
    ammo = {}
    for f in DATA_DIR.rglob("*.json"):
        if "schema" in str(f) or ".omo" in str(f):
            continue
        try:
            with open(f) as fh:
                d = json.load(fh)
            if "projectile" in d and "caliber_mm" in d["projectile"]:
                ammo[d.get("class", "")] = f
            elif "barrel_length_mm" in d:
                weapons[d.get("class", "")] = f
            elif "armor_mm_rha" in d or "turret_armor" in d:
                weapons[d.get("class", "")] = f
        except (json.JSONDecodeError, KeyError):
            pass
    return weapons, ammo


def generate_ammo(dump_ace_ammo, existing_ammo):
    """Generate ammo JSONs from ACE3 ballistics data."""
    generated = 0
    skipped = 0

    for cls, rec in sorted(dump_ace_ammo.items()):
        safe_name = cls.lower().replace(" ", "_").replace("/", "_")
        cal_dir = get_caliber_dir(rec["ace_caliber"])
        cal_subdir = DATA_DIR / "ammo" / cal_dir
        cal_subdir.mkdir(parents=True, exist_ok=True)
        out_path = cal_subdir / f"{safe_name}.json"

        if cls in existing_ammo or out_path.exists():
            skipped += 1
            continue

        cdm_id = (
            f"g{rec['ace_dragModel']}" if rec["ace_dragModel"] in (1, 7, 8) else "g7"
        )
        bc_val = rec["ace_bc_list"][0] if rec["ace_bc_list"] else 0.0

        bc_key = "bc_g7" if cdm_id == "g7" else ("bc_g1" if cdm_id == "g1" else "bc_g8")

        # Reference muzzle velocity from middle barrel length
        ref_mv = 800.0
        if rec["ace_vel_list"] and rec["ace_bl_list"]:
            mid = len(rec["ace_vel_list"]) // 2
            if mid < len(rec["ace_vel_list"]):
                ref_mv = float(rec["ace_vel_list"][mid])

        ammo_json = {
            "class": cls,
            "projectile": {
                "model": safe_name,
                "caliber_mm": round(rec["ace_caliber"], 3),
                "mass_g": round(rec["ace_mass"], 4) if rec["ace_mass"] else 0.0,
                "muzzle_velocity_ms": round(ref_mv, 1),
                bc_key: round(bc_val, 3) if bc_val else 0.0,
                "cdm_id": cdm_id,
                "source": {
                    "type": "reference_data",
                    "reference": f"ACE3 ace_ballistics CfgAmmo::{cls}",
                    "methodology": "Extracted from ACE3 ballistics config via Arma 3 RPT dump.",
                    "confidence": "high",
                },
            },
            "chamber_pressure_mpa": 0,
            "notes": f"Generated from Arma 3 config dump. ACE3 ballistics: caliber={rec['ace_caliber']}mm, mass={rec['ace_mass']}g, drag={cdm_id}, BC={bc_val}.",
        }

        with open(out_path, "w") as f:
            json.dump(ammo_json, f, indent=2)
        generated += 1
        print(f"  [+{generated:3d}] {cls} ({rec['ace_caliber']:.2f}mm -> {cal_dir}/)")

    return generated, skipped


def consolidate_weapon_variants(ace_weapons):
    """Group weapon variants by base class, return unique bases with best params."""
    bases = {}

    for cls, rec in ace_weapons.items():
        # Strip camo suffixes to find base
        base = re.sub(
            r"(_(black|blk|khk|tna|wdl|hex|green|coyote|sand|arid|snd|lush|olive))(_F)?$",
            "",
            cls,
        )
        base = re.sub(r"_F$", "", base)

        if base not in bases or rec["barrel"] > 0:
            # Keep the variant with known barrel length, or first variant
            bases[base] = (cls, rec)

    return bases


def generate_weapons(dump_ace_weapons, existing_weapons):
    """Generate weapon JSONs from ACE3 physical params + caliber inference."""
    bases = consolidate_weapon_variants(dump_ace_weapons)
    generated = 0
    skipped = 0
    no_caliber = 0

    for base_name, (orig_cls, rec) in sorted(bases.items()):
        if orig_cls in existing_weapons:
            skipped += 1
            continue

        cal_type, cal_mm, cdm_id, bc, pressure = infer_caliber(orig_cls)

        if cal_type is None:
            no_caliber += 1
            continue

        # Determine weapon type directory
        if orig_cls.startswith("arifle_"):
            wpn_dir = "rifles"
        elif orig_cls.startswith("srifle_"):
            wpn_dir = "snipers"
        elif orig_cls.startswith("LMG_") or orig_cls.startswith("lmg_"):
            wpn_dir = "machine_guns"
        elif orig_cls.startswith("SMG_") or orig_cls.startswith("smg_"):
            wpn_dir = "smgs"
        elif orig_cls.startswith("pistol_") or orig_cls.startswith("hgun_"):
            wpn_dir = "pistols"
        elif orig_cls.startswith("launch_"):
            wpn_dir = "launchers"
        elif orig_cls.startswith("sgun_"):
            wpn_dir = "shotguns"
        else:
            wpn_dir = "rifles"  # fallback

        out_dir = DATA_DIR / "weapons" / wpn_dir
        out_dir.mkdir(parents=True, exist_ok=True)
        safe_name = orig_cls.lower()
        out_path = out_dir / f"{safe_name}.json"

        if out_path.exists():
            skipped += 1
            continue

        barrel = rec["barrel"] if rec["barrel"] > 0 else 508.0
        twist = rec["twist"] if rec["twist"] > 0 else 178.0

        weapon_json = {
            "class": orig_cls,
            "caliber_mm": cal_mm,
            "barrel_length_mm": barrel,
            "rifling_twist_mm": twist,
            "chamber_pressure_mpa": pressure,
            "cdm_id": cdm_id,
            "projectile_mass_g": infer_projectile_mass(cal_mm, wpn_dir),
            "muzzle_velocity_ms": 800.0,
            "zero_range_m": 300 if wpn_dir in ("rifles", "snipers", "dmrs") else 100,
            "source": {
                "type": "inferred",
                "reference": f"ACE3 ace_ballistics CfgWeapons::{orig_cls}",
                "methodology": f"Barrel/twist from ACE3. Caliber inferred from class naming ({cal_type}). Chamber pressure estimated for caliber.",
                "confidence": "medium",
            },
            "notes": f"ACE3 barrel={rec['barrel']}mm, twist={rec['twist']}mm. Caliber inferred as {cal_type}. Generated from Arma 3 config dump.",
        }

        with open(out_path, "w") as f:
            json.dump(weapon_json, f, indent=2)
        generated += 1
        print(f"  [+{generated:3d}] {orig_cls} ({cal_type}, {barrel:.0f}mm barrel)")

    return generated, skipped, no_caliber


def is_combat_vehicle(cls, rec):
    """Whitelist: only generate real combat vehicles, nothing else."""
    cls_lower = cls.lower()

    hard_nope = [
        "item",
        "_base",
        "_module",
        "_object",
        "bodybag",
        "adenosine",
        "atropine",
        "banana",
        "bloodiv",
        "canteen",
        "spottingscope",
        "backpack",
        "carryall",
        "box_",
        "mine_",
        "toepopper",
        "bush_",
        "clutter",
        "crop_",
        "liana_",
        "shrub_",
        "tree_",
        "weapon_",
        "ammo_",
        "launcher",
        "pistol",
        "rifle_",
        "sgun_",
        "smg_",
        "srifle_",
        "lmg_",
        "mmg_",
        "binocular",
        "laserdesignator",
        "nvgoggles",
        "rangefinder",
        "uniform_",
        "vest_",
        "helmet",
        "map_",
        "gps_",
        "radio_",
        "compass",
        "watch_",
        "firstaid",
        "medikit",
        "toolkit_",
        "prop_",
        "sign_",
        "sandbag",
        "crate_",
        "sack_",
        "tent_",
        "lantern_",
        "bunker_",
        "wall_",
        "tower_",
        "lamp_",
        "bench_",
        "chair_",
        "table_",
        "shelf_",
        "pillbox",
        "radar_",
        "antenna_",
        "hq_",
        "hangar_",
        "powerline",
        "reservoir",
        "fueltank",
        "container_",
        "pier_",
        "dock_",
        "slum_",
        "shelter_",
        "shower_",
        "roadblock",
        "roadbarrier",
        "roadcone",
        "tapesign",
        "razorwire",
        "shootingpos",
        "target_",
        "part0",
        "part1",
        "flag_",
        "flags_",
        "fence_",
        "gate_",
        "stairs_",
        "ladder_",
        "door_",
        "window_",
        "roof_",
        "floor_",
        "camo_net",
        "net_",
        "helipad",
        "pavement",
    ]
    for pat in hard_nope:
        if pat in cls_lower:
            return False

    if rec["armor"] >= 10000 or rec["armorStructural"] >= 10000:
        return False
    if rec["armor"] < 20 and rec["armorStructural"] < 20:
        return False

    vehicle_kw = [
        "car",
        "jeep",
        "truck",
        "van_",
        "bus_",
        "tank",
        "apc_",
        "mbt_",
        "ifv_",
        "wheeled_",
        "tracked_",
        "motorcycle",
        "bike_",
        "quadbike",
        "helicopter",
        "plane_",
        "jet_",
        "ship_",
        "boat_",
        "submarine",
        "uav_",
        "drone_",
        "ambulance",
        "offroad",
        "suv_",
        "hatchback",
        "sedan",
        "pickup_",
        "trailer",
    ]
    return any(kw in cls_lower for kw in vehicle_kw)


def generate_vehicles(dump_vehicles, existing_weapons):
    """Generate vehicle JSONs for combat vehicles with armor data."""
    generated = 0
    skipped = 0

    combat_exclude = [
        "All",
        "AllVehicles",
        "Land",
        "LandVehicle",
        "Car",
        "Car_F",
        "Tank",
        "Tank_F",
        "Wheeled_APC_F",
        "Tracked_APC_F",
        "MBT_01_base_F",
        "MBT_02_base_F",
        "MBT_03_base_F",
        "APC_Tracked_01_base_F",
        "APC_Tracked_02_base_F",
        "APC_Tracked_03_base_F",
        "APC_Wheeled_01_base_F",
        "APC_Wheeled_02_base_F",
        "APC_Wheeled_03_base_F",
        "StaticWeapon",
        "StaticMGWeapon",
        "StaticCannon",
        "Ship",
        "Ship_F",
        "Submarine",
        "Helicopter",
        "Helicopter_Base_F",
        "Plane",
        "Plane_Base_F",
        "UAV",
        "Drone",
        "Strategic",
        "Thing",
        "Animal",
        "Man",
        "CAManBase",
    ]

    out_dir = DATA_DIR / "vehicles"
    out_dir.mkdir(parents=True, exist_ok=True)

    for cls, rec in sorted(dump_vehicles.items()):
        if cls in combat_exclude or rec["parent"] in combat_exclude:
            continue
        if not is_combat_vehicle(cls, rec):
            continue
        if cls in existing_weapons:
            skipped += 1
            continue

        safe_name = cls.lower()
        out_path = out_dir / f"{safe_name}.json"
        if out_path.exists():
            skipped += 1
            continue

        vehicle_json = {
            "class": cls,
            "armor_thickness_mm": int(rec["armor"]),
            "armor_structural": int(rec["armorStructural"]),
            "source": {
                "type": "game_data",
                "reference": f"Arma 3 configFile CfgVehicles::{cls}",
                "methodology": "Armor values from game config. ACE3 armor class/hitpoint values included when available.",
                "confidence": "medium",
            },
            "notes": f"armor={rec['armor']}, armorStructural={rec['armorStructural']}.",
        }
        if rec["ace_armorClass"]:
            vehicle_json["ace_armor_class"] = rec["ace_armorClass"]
        if rec["ace_hitpointArmor"]:
            vehicle_json["ace_hitpoint_armor"] = rec["ace_hitpointArmor"]

        with open(out_path, "w") as f:
            json.dump(vehicle_json, f, indent=2)
        generated += 1
        print(f"  [+{generated:3d}] {cls} (armor={rec['armor']})")

    return generated, skipped


def main():
    if not DUMP_FILE.exists():
        print(f"ERROR: Dump file not found at {DUMP_FILE}")
        return 1

    print("Loading existing data/...")
    existing_weapons, existing_ammo = load_existing_classes()
    print(f"  Existing weapons: {len(existing_weapons)}")
    print(f"  Existing ammo: {len(existing_ammo)}")

    print("\nParsing dump file...")
    (
        dp_weapons,
        dp_ace_wpn,
        dp_ammo,
        dp_ace_ammo,
        dp_mags,
        dp_magwells,
        dp_veh,
        dp_ace_veh,
    ) = parse_dump()
    print(f"  Weapons: {len(dp_weapons)} ({len(dp_ace_wpn)} with ACE3 params)")
    print(f"  Ammo: {len(dp_ammo)} ({len(dp_ace_ammo)} with ACE3 ballistics)")
    print(f"  Vehicles: {len(dp_veh)} ({len(dp_ace_veh)} with armor)")
    print(f"  Magazines: {len(dp_mags)}, Magwells: {len(dp_magwells)}")

    print("\n" + "=" * 60)
    print("GENERATING AMMO JSONS")
    print("=" * 60)
    ammo_gen, ammo_skip = generate_ammo(dp_ace_ammo, existing_ammo)
    print(f"  Generated: {ammo_gen}, Skipped: {ammo_skip}")

    print("\n" + "=" * 60)
    print("GENERATING WEAPON JSONS")
    print("=" * 60)
    wpn_gen, wpn_skip, wpn_nocal = generate_weapons(dp_ace_wpn, existing_weapons)
    print(f"  Generated: {wpn_gen}, Skipped: {wpn_skip}, No caliber: {wpn_nocal}")

    print("\n" + "=" * 60)
    print("GENERATING VEHICLE JSONS")
    print("=" * 60)
    veh_gen, veh_skip = generate_vehicles(dp_ace_veh, existing_weapons)
    print(f"  Generated: {veh_gen}, Skipped: {veh_skip}")

    print("\n" + "=" * 60)
    print("SUMMARY")
    print("=" * 60)
    print(f"  Ammo:     {ammo_gen} generated, {ammo_skip} skipped")
    print(
        f"  Weapons:  {wpn_gen} generated, {wpn_skip} skipped, {wpn_nocal} no caliber"
    )
    print(f"  Vehicles: {veh_gen} generated, {veh_skip} skipped")

    return 0


if __name__ == "__main__":
    sys.exit(main())
