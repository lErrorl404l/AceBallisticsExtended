#!/usr/bin/env python3
"""
Parse SQF-exported weapons/magazines/ammo pipe-delimited files.
Cross-references weapon→magazine→ammo chains to determine caliber, muzzle velocity.
Generates weapon & ammo JSON files matching the project schema.

Usage:
    python3 parse_sqf_export.py \
        --weapons data/scripts/weapons \
        --magazines data/scripts/magazines \
        --ammo data/scripts/ammo
"""

import argparse
import json
import os
import re
import sys
from collections import defaultdict

# ── Caliber inference ──────────────────────────────────────────────────────────
# Arma 3 class names use encoded caliber patterns. Build a robust lookup.

# Known caliber codes in Arma 3 class names (diameter code → mm)
# 3-digit codes: 556=5.56, 762=7.62, 545=5.45, 408=10.36mm(.408"), 338=8.6mm(.338)
# 3-digit codes starting with 1: 127=12.7mm, 120=12.0mm, 125=12.5mm, 105=10.5mm
# 2-digit codes: 9=9mm, 40=40mm, 30=30mm, 20=20mm, 12=18.5mm(12ga)
# 2-digit codes: 65=6.5mm, 93=9.3mm
CALIBER_MAP = {
    # Three-digit codes (diameter in mm*100, or in mm*10 for 1xx)
    "556": 5.56,
    "545": 5.45,
    "570": 5.7,
    "580": 5.8,
    "762": 7.62,
    "338": 8.6,
    "408": 10.36,
    "127": 12.7,
    "120": 12.0,
    "125": 12.5,
    "105": 10.5,
    "093": 9.3,
    "065": 6.5,
    # Two-digit codes in xNN context (disambiguated by caliber map)
    "65": 6.5,
    "93": 9.3,
}

CALIBER_PATTERN = re.compile(r"(\d{1,3})x(\d+)", re.IGNORECASE)
CALIBER_DIRECT = re.compile(r"(\d+)(?:mm|_mm)", re.IGNORECASE)
CALIBER_NAKED = re.compile(r"_(\d+)_", re.IGNORECASE)  # like _408_ in B_408_Ball
GAUGE_PATTERN = re.compile(r"(\d+)Gauge", re.IGNORECASE)


def parse_caliber_from_name(name: str) -> tuple[float | None, str | None]:
    """Extract physical caliber in mm from an Arma 3 class name.
    Returns (caliber_mm, case_length_code) tuple.
    """
    # Check for gauge (12 Gauge → 18.5mm)
    m = GAUGE_PATTERN.search(name)
    if m:
        gauge = int(m.group(1))
        if gauge == 12:
            return (18.5, None)
        return (None, None)

    # Check for NNNxNN pattern (556x45, 65x39, 9x21, etc.)
    m = CALIBER_PATTERN.search(name)
    if m:
        code = m.group(1)
        case_len = m.group(2)
        if code in CALIBER_MAP:
            return (CALIBER_MAP[code], case_len)
        num = int(code)
        if len(code) == 3 and num >= 300:
            return (num / 100.0, case_len)
        if len(code) == 3 and 100 <= num <= 199:
            return (num / 10.0, case_len)
        if len(code) == 1 and 1 <= num <= 9:
            return (float(num), case_len)
        return (None, case_len)

    # Check for direct patterns like B_20mm, B_30mm_HE
    m = CALIBER_DIRECT.search(name)
    if m:
        num = int(m.group(1))
        if num <= 50:
            return (float(num), None)
        return (None, None)

    # Check for naked caliber codes like _408_ in B_408_Ball
    m = CALIBER_NAKED.search(name)
    if m:
        code = m.group(1)
        if code in CALIBER_MAP:
            return (CALIBER_MAP[code], None)
        num = int(code)
        if len(code) == 3 and 300 <= num <= 999:
            return (num / 100.0, None)
        if len(code) == 3 and 100 <= num <= 199:
            return (num / 10.0, None)

    return (None, None)


def infer_caliber_from_magazine_name(mag_name: str) -> float | None:
    """Extract caliber from magazine class names like 30Rnd_556x45_Stanag."""
    cal, _ = parse_caliber_from_name(mag_name)
    return cal


# ── Ballistic classification ──────────────────────────────────────────────────

BALLISTIC_SIMULATIONS = {"shotBullet", "shotShell", "shotSpread", "shotGrenade"}
NON_BALLISTIC_SIMULATIONS = {
    "shotMissile",
    "shotRocket",
    "shotSmokeX",
    "shotIlluminating",
    "shotMine",
    "shotDirectionalBomb",
    "shotBoundingMine",
    "shotTimeBomb",
    "shotDeploy",
    "shotSubmunitions",
    "shotLaser",
    "laserDesignate",
}

# Weapon base classes for handheld/ballistic weapons
HANDHELD_WEAPON_BASES = {
    "Rifle_Base_F",
    "Rifle_Short_Base_F",
    "Rifle_Long_Base_F",
    "Pistol_Base_F",
    "Pistol_Heavy_Base_F",
    "SMG_Base_F",
    "SMG_01_Base_F",
    "SMG_02_Base_F",
    "SMG_03_Base_F",
    "Launcher_Base_F",
    "Launcher_Short_Base_F",
    "MachineGun_Base_F",
    "LMG_base_F",
    "MMG_Base_F",
    "DMR_01_base_F",
    "EBR_base_F",
    "GM6_base_F",
    "LRS_base_F",
    "Shotgun_Base_F",
}

# Known vehicle/junk categories to skip
VEHICLE_WEAPON_BASES = {
    "MGun",
    "MGunCore",
    "CannonCore",
    "CannonCore",
    "RocketPods",
    "MissileLauncher",
    "weapon_LGBLauncherBase",
    "GMG_F",
    "LMG_RCWS",
    "HMG_127",
    "HMG_01",
    "HMG_M2",
    "SmokeLauncher",
    "CMFlareLauncher",
    "Default",
    "Binocular",
    "Laserdesignator",
    "GrenadeLauncher",
}


def is_variant_weapon(name: str, parent: str, handheld_set: set) -> bool:
    """Check if a weapon is a variant of another production weapon (not a base class)."""
    if not parent:
        return False
    # If parent is a known abstract base class → not a variant
    if parent in HANDHELD_WEAPON_BASES:
        return False
    # If parent name contains _base_ → intermediate abstract base, not variant
    if "_base_" in parent.lower() or parent.lower().endswith("_base_f"):
        return False
    # If parent is itself a handheld production weapon → this is a variant
    if parent in handheld_set:
        return True
    return False


def is_handheld_weapon(name: str, parent: str, base_parent: str) -> bool:
    """Classify a weapon as handheld ballistics-worthy vs vehicle/missile/etc."""
    # Check by parent/base class
    all_parents = {base_parent, parent}
    if all_parents & HANDHELD_WEAPON_BASES:
        return True
    if all_parents & VEHICLE_WEAPON_BASES & {"Binocular", "Laserdesignator"}:
        return False
    # Vehicle weapons have distinctive names
    vehicle_patterns = [
        "RCWS",
        "UGV",
        "APC",
        "MBT",
        "coax",
        "heli",
        "Transport",
        "gatling",
        "cannon_",
        "mortar_",
        "missiles_",
        "rockets_",
        "SmokeLauncher",
        "FlareLauncher",
        "CMFlare",
        "Laserdesignator",
        "GBU",
        "Bomb_",
        "Mk82",
        "Twin_Cannon",
    ]
    for pat in vehicle_patterns:
        if pat in name:
            return False
    return True


# ── Ammo classification ───────────────────────────────────────────────────────


def is_ballistic_ammo(simulation: str, hit: float, name: str) -> bool:
    """Check if ammo is an actual ballistic projectile worth keeping."""
    if simulation == "" and hit == 0:
        return False  # abstract base
    if simulation in NON_BALLISTIC_SIMULATIONS and not simulation.startswith("shot"):
        return False
    if simulation == "shotBullet" or simulation == "shotShell":
        return True
    if simulation == "shotGrenade" and hit > 0:
        return True
    if simulation == "shotSpread" and hit > 0:
        return True
    return False


# ── Air friction → BC estimation ──────────────────────────────────────────────


def air_friction_to_bc(air_friction: float, caliber_mm: float) -> float | None:
    """
    Rough BC estimate from Arma 3 airFriction.
    Based on the relationship: in Arma, airFriction roughly -0.001 corresponds
    to G7 BC ~0.2-0.3 for rifle calibers.
    """
    if air_friction >= 0:
        return None
    af = abs(air_friction)
    # Rough mapping from observed data:
    # airFriction = -0.00048 → .408, BC ~0.6+
    # airFriction = -0.00086 → .50 BMG, BC ~0.5
    # airFriction = -0.001 → 7.62mm, BC ~0.2
    # airFriction = -0.0012 → 5.56mm, BC ~0.15
    # airFriction = -0.0016 → 9mm, BC ~0.12
    # airFriction = -0.002 → subsonic, BC ~0.1
    # airFriction = -0.008 → shotgun slug
    # Scale by caliber
    if caliber_mm <= 0:
        caliber_mm = 7.62
    bc = 0.20 * (0.001 / af) * (caliber_mm / 7.62) ** 0.5
    return min(bc, 1.0)  # cap at 1.0


# ── Parser ────────────────────────────────────────────────────────────────────


def parse_pipe_file(path: str) -> list[list[str]]:
    """Parse pipe-delimited SQF export file."""
    rows = []
    with open(path) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            parts = line.split("|")
            rows.append(parts)
    return rows


# ── Main ──────────────────────────────────────────────────────────────────────


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--weapons", required=True, help="Vanilla weapons export file")
    parser.add_argument(
        "--magazines", required=True, help="Vanilla magazines export file"
    )
    parser.add_argument("--ammo", required=True, help="Vanilla ammo export file")
    parser.add_argument("--ace-weapons", help="ACE weapons export file")
    parser.add_argument("--ace-magazines", help="ACE magazines export file")
    parser.add_argument("--ace-ammo", help="ACE ammo export file")
    parser.add_argument(
        "--ace-ballistics",
        help="ACE3 ballistics config export (from sqf_export_ace_ballistics.sqf)",
    )
    parser.add_argument("--out-dir", default="data")
    args = parser.parse_args()

    weapons = parse_pipe_file(args.weapons)
    magazines = parse_pipe_file(args.magazines)
    ammo_rows = parse_pipe_file(args.ammo)

    if args.ace_weapons:
        ace_w = parse_pipe_file(args.ace_weapons)
        weapons.extend(ace_w)
        print(f"ACE weapons: {len(ace_w)} added", file=sys.stderr)
    if args.ace_magazines:
        ace_m = parse_pipe_file(args.ace_magazines)
        magazines.extend(ace_m)
        print(f"ACE magazines: {len(ace_m)} added", file=sys.stderr)
    if args.ace_ammo:
        ace_a = parse_pipe_file(args.ace_ammo)
        ammo_rows.extend(ace_a)
        print(f"ACE ammo: {len(ace_a)} added", file=sys.stderr)

    # Parse ACE ballistics config data (ACE3 ballistic extension fields)
    ace_ballistics: dict[str, dict] = {}
    if args.ace_ballistics:
        for parts in parse_pipe_file(args.ace_ballistics):
            if len(parts) < 7:
                continue
            # ACE|name|caliber|mass|length|dragModel|stdAtm|velSD|BCs|MVs|barrelLengths|tempShifts|transonic|base
            name = parts[1]
            ace_ballistics[name] = {
                "caliber_mm": float(parts[2]) if parts[2] not in ("0", "") else None,
                "mass_g": float(parts[3]) if parts[3] not in ("0", "") else None,
                "bullet_length_mm": float(parts[4])
                if parts[4] not in ("0", "")
                else None,
                "drag_model": int(parts[5]) if parts[5] not in ("0", "") else None,
                "std_atmosphere": parts[6],
                "vel_sd": float(parts[7]) if parts[7] not in ("0", "") else None,
                "bc_list": [float(x) for x in parts[8].split(",") if x]
                if len(parts) > 8 and parts[8]
                else [],
                "mv_list": [float(x) for x in parts[9].split(",") if x]
                if len(parts) > 9 and parts[9]
                else [],
                "bl_list": [float(x) for x in parts[10].split(",") if x]
                if len(parts) > 10 and parts[10]
                else [],
                "temp_shifts": [float(x) for x in parts[11].split(",") if x]
                if len(parts) > 11 and parts[11]
                else [],
                "transonic_coef": float(parts[12])
                if len(parts) > 12 and parts[12] not in ("0", "")
                else None,
            }
        print(f"ACE ballistics: {len(ace_ballistics)} entries", file=sys.stderr)

    # Build indices
    mag_by_name: dict[str, dict] = {}
    for parts in magazines:
        # M|name|displayName|initSpeed|ammo|count|base
        if len(parts) < 6:
            continue
        name = parts[1]
        mag_by_name[name] = {
            "displayName": parts[2],
            "initSpeed": float(parts[3])
            if parts[3] not in ("0", "") and float(parts[3]) > 50
            else None,
            "ammo": parts[4],
            "count": int(float(parts[5])),
            "base": parts[6] if len(parts) > 6 else "",
        }

    ammo_by_name: dict[str, dict] = {}
    for parts in ammo_rows:
        # A|name|hit|caliber|typicalSpeed|airFriction|timeToLive|simulation|base
        if len(parts) < 8:
            continue
        name = parts[1]
        ammo_by_name[name] = {
            "hit": float(parts[2]),
            "caliber": float(parts[3]),
            "typicalSpeed": float(parts[4]) if parts[4] not in ("0", "") else None,
            "airFriction": float(parts[5]),
            "timeToLive": float(parts[6]),
            "simulation": parts[7],
            "base": parts[8] if len(parts) > 8 else "",
        }

    # Process weapons
    out_weapons_dir = os.path.join(args.out_dir, "weapons")
    out_ammo_dir = os.path.join(args.out_dir, "ammo")
    os.makedirs(out_weapons_dir, exist_ok=True)
    os.makedirs(out_ammo_dir, exist_ok=True)

    generated_weapons = 0
    skipped_weapons = 0
    generated_ammo_types = 0
    skipped_ammo_types = 0

    # Track ammo classes that are used by weapons (to know which to generate)
    used_ammo_classes = set()

    # ── First pass: identify handheld weapon names (for variant detection) ──
    handheld_names: set[str] = set()
    for parts in weapons:
        if len(parts) < 6:
            continue
        name = parts[1]
        base_parent = parts[6] if len(parts) > 6 else ""
        if is_handheld_weapon(name, base_parent, base_parent):
            handheld_names.add(name)

    # ── Second pass: generate weapon JSONs ──
    for parts in weapons:
        # W|name|displayName|modelLength|initSpeed|mags|base
        if len(parts) < 6:
            continue
        name = parts[1]
        display_name = parts[2]
        model_length = float(parts[3]) if parts[3] != "0" else 0.0
        init_speed_w = (
            float(parts[4])
            if parts[4] not in ("0", "") and float(parts[4]) > 50
            else None
        )
        mag_list = parts[5].split(",") if parts[5] else []
        base_parent = parts[6] if len(parts) > 6 else ""

        # Skip variants
        if is_variant_weapon(name, base_parent, handheld_names):
            skipped_weapons += 1
            continue

        # Skip non-handheld
        if not is_handheld_weapon(name, base_parent, base_parent):
            skipped_weapons += 1
            continue

        # Determine caliber from first magazine
        caliber = None
        for mag_name in mag_list:
            cal = infer_caliber_from_magazine_name(mag_name)
            if cal:
                caliber = cal
                break
        if not caliber:
            cal, _ = parse_caliber_from_name(name)
            if cal:
                caliber = cal

        # Get muzzle velocity from first magazine
        muzzle_velocity = init_speed_w
        primary_ammo = None
        for mag_name in mag_list:
            mag = mag_by_name.get(mag_name)
            if mag and mag["initSpeed"]:
                if not muzzle_velocity:
                    muzzle_velocity = mag["initSpeed"]
                ammo_name = mag["ammo"]
                if ammo_name and ammo_name in ammo_by_name:
                    primary_ammo = ammo_name
                    if not muzzle_velocity:
                        ammo_data = ammo_by_name[ammo_name]
                        if ammo_data["typicalSpeed"]:
                            muzzle_velocity = ammo_data["typicalSpeed"]
                    break

        # Determine barrel length from modelLength or estimate
        barrel_length = (
            model_length * 1000 if model_length > 0 else None
        )  # config is in meters

        # Build output JSON matching existing weapon schema
        cal_str = f"{caliber:.2f}" if caliber else "?"
        safe_name = name.lower().replace(" ", "_").replace("(", "").replace(")", "")
        filename = f"{safe_name}.json"
        filepath = os.path.join(out_weapons_dir, filename)

        weapon_data = {
            "class": name,
            "caliber_mm": caliber,
            "barrel_length_mm": barrel_length,
            "rifling_twist_mm": None,  # manual
            "chamber_pressure_mpa": None,  # manual - SAAMI/CIP reference needed
            "cdm_id": "g7",
            "projectile_mass_g": None,  # manual
            "muzzle_velocity_ms": muzzle_velocity,
            "zero_range_m": 100,
            "effective_range_m": None,  # manual estimate
            "notes": f"Arma 3 class: {name}. Cal: ~{cal_str}mm. Muzzle vel: {muzzle_velocity} m/s. Barrel: {barrel_length if barrel_length else '?'}mm.",
            "twist_rate_mm": None,  # manual
        }

        # Remove None values to keep JSON clean
        weapon_clean = {k: v for k, v in weapon_data.items() if v is not None}

        with open(filepath, "w") as f:
            json.dump(weapon_clean, f, indent=2)
        generated_weapons += 1

        # Track ammo used by this weapon
        if primary_ammo:
            used_ammo_classes.add(primary_ammo)
        for mag_name in mag_list:
            mag = mag_by_name.get(mag_name)
            if mag and mag["ammo"]:
                used_ammo_classes.add(mag["ammo"])

    # Process ammo
    for parts in ammo_rows:
        if len(parts) < 8:
            continue
        name = parts[1]
        hit = float(parts[2])
        caliber_arma = float(parts[3])
        typical_speed = float(parts[4]) if parts[4] not in ("0", "") else None
        air_friction = float(parts[5])
        time_to_live = float(parts[6])
        simulation = parts[7]
        base = parts[8] if len(parts) > 8 else ""

        # Determine physical caliber from class name
        caliber_mm, case_len = parse_caliber_from_name(name)

        # Only generate for ballistic ammo or ammo used by weapons
        if (
            not is_ballistic_ammo(simulation, hit, name)
            and name not in used_ammo_classes
        ):
            skipped_ammo_types += 1
            continue

        # Estimate BC from airFriction
        bc_g7 = None
        if caliber_mm:
            bc_g7 = air_friction_to_bc(air_friction, caliber_mm)

        # Rough mass estimate from hit value (Arma's hit ~ damage, loosely related to mass)
        # This is VERY approximate and should be replaced with reference data
        estimated_mass = None
        if caliber_mm and caliber_mm <= 15:
            # Rough: hit * some factor based on caliber
            # hit=9 for 5.56, hit=11.6 for 7.62, hit=30 for .50 BMG
            if hit > 0:
                if caliber_mm < 8:
                    estimated_mass = round(hit * 0.35 + 0.5, 2)
                elif caliber_mm < 10:
                    estimated_mass = round(hit * 0.65 + 1, 2)
                elif caliber_mm < 15:
                    estimated_mass = round(hit * 0.15 + 5, 2)

        # Apply ACE3 ballistics overrides if available
        ace_source = ""
        ace_bc = bc_g7
        ace_drag = "g7"
        if name in ace_ballistics:
            ab = ace_ballistics[name]
            if ab["caliber_mm"]:
                caliber_mm = ab["caliber_mm"]
            if ab["mass_g"]:
                estimated_mass = ab["mass_g"]
            if ab["bc_list"]:
                ace_bc = ab["bc_list"][0]
                bc_g7 = ace_bc
            if ab["drag_model"] == 7:
                ace_drag = "g7"
            elif ab["drag_model"] == 1:
                ace_drag = "g1"
            if ab["mv_list"] and ab["bl_list"]:
                # Store barrel-specific velocity data in notes
                mv_bl_pairs = ", ".join(
                    [f"{mv}m/s@{bl}mm" for mv, bl in zip(ab["mv_list"], ab["bl_list"])]
                )
                ace_source = f" ACE3: caliber={ab['caliber_mm']}mm, mass={ab['mass_g']}g, drag={ab['drag_model']}, BC={ab['bc_list']}. Velocities: {mv_bl_pairs}"
            else:
                ace_source = f" ACE3: caliber={ab['caliber_mm']}mm, mass={ab['mass_g']}g, drag={ab['drag_model']}, BC={ab['bc_list']}"
            # Recalculate category with ACE caliber
            cal_str = f"{caliber_mm:.2f}" if caliber_mm else f"{caliber_arma:.2f}(arma)"

        cal_str = f"{caliber_mm:.2f}" if caliber_mm else f"{caliber_arma:.2f}(arma)"
        safe_name = name.lower().replace(" ", "_").replace("(", "").replace(")", "")

        # Categorize ammo by physical caliber and case length
        if caliber_mm:
            if (
                "gauge" in name.lower()
                or "shotgun" in name.lower()
                or caliber_mm == 18.5
            ):
                category = "shotgun"
            elif caliber_mm <= 6.0:
                category = "rifle/5_56mm"
            elif caliber_mm <= 7.0:
                category = "rifle/6_5mm"
            elif caliber_mm <= 9.0:
                # 9x19/9x21 = handgun, 7.62 = rifle
                if caliber_mm >= 8.0 and case_len and int(case_len) < 30:
                    category = "handgun"
                else:
                    category = "rifle/7_62mm"
            elif 9.0 < caliber_mm <= 11.0:
                category = "rifle/other"
            elif caliber_mm <= 15.0:
                category = "heavy_127mm"
            elif caliber_mm <= 22.0:
                category = "shotgun"
            elif caliber_mm <= 100:
                category = "launcher"
            else:
                category = "launcher"
        else:
            category = "rifle/other"

        ammo_dir = os.path.join(out_ammo_dir, category)
        os.makedirs(ammo_dir, exist_ok=True)

        filename = f"{safe_name}.json"
        filepath = os.path.join(ammo_dir, filename)

        ammo_data = {
            "class": name,
            "projectile": {
                "model": name.split("_")[-1].lower() if "_" in name else name.lower(),
                "mass_g": estimated_mass,
                "caliber_mm": caliber_mm,
                "bc_g7": round(bc_g7, 3) if bc_g7 else None,
                "cdm_id": ace_drag if name in ace_ballistics else "g7",
                "source": (
                    f"ACE3 ballistics: {ace_source}"
                    if ace_source
                    else f"Arma 3 config. hit={hit}, airFriction={air_friction}, cal={cal_str}"
                ),
                "fragmentation": None,
                "frag_mass_mean": None,
                "frag_mass_std": None,
                "ricochet_angle_deg": None,
                "tracer_burn_time_s": 6.0 if "tracer" in name.lower() else 0.0,
                "incendiary": False,
                "incendiary_ignition_temp_k": 0.0,
            },
        }

        with open(filepath, "w") as f:
            json.dump(ammo_data, f, indent=2)
        generated_ammo_types += 1

    print(f"Weapons: {generated_weapons} generated, {skipped_weapons} skipped")
    print(f"Ammo: {generated_ammo_types} generated, {skipped_ammo_types} skipped")


if __name__ == "__main__":
    main()
