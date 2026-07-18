#!/usr/bin/env python3
"""Merge ACE3 barrel length data into weapon JSONs.

Sources:
  - data/scripts/ace_main_barrels.txt (ACE3 main addon, ~52 vanilla weapons)
  - data/scripts/ace_cfgweapons_barrels.txt (CDLC compat addons)
  - Fallback defaults for known platforms without ACE3 data
"""

import json, os, re

WEAPONS_DIR = "data/weapons"
SCRIPTS_DIR = "data/scripts"
BARREL_MAIN = os.path.join(SCRIPTS_DIR, "ace_main_barrels.txt")
BARREL_CDLC = os.path.join(SCRIPTS_DIR, "ace_cfgweapons_barrels.txt")

# --- Load barrel data ---


def load_barrel_file(path):
    """Returns dict of class -> barrel_mm from ACEW|class|parent|barrel_mm|... format."""
    data = {}
    if not os.path.exists(path):
        print(f"  [warn] {path} not found, skipping")
        return data
    with open(path) as f:
        for line in f:
            line = line.strip()
            if (
                not line
                or line.startswith("#")
                or line.startswith("snip")
                or line.startswith("Total")
            ):
                continue
            parts = line.split("|")
            if parts[0] != "ACEW" or len(parts) < 4:
                continue
            cls = parts[1].strip()
            # ACE3 main format: ACEW|class|barrel_mm|source (4 fields, barrel in parts[2])
            # CDLC format:      ACEW|class|parent|barrel_mm|twist|source (6 fields, barrel in parts[3])
            barrel_idx = 2 if len(parts) == 4 else 3
            barrel_str = parts[barrel_idx].strip()
            if barrel_str:
                try:
                    barrel = float(barrel_str)
                except ValueError:
                    continue
                data[cls] = barrel
                # Also register base-class variants so color variants map in
                base_v = re.sub(
                    r"_(blk|khk|snd|hex|ghex|arid|lush|camo|black|green|sand|coyote|olive|tropic|m81|fleck|rgr|exp|tna|base)_F$",
                    "_F",
                    cls,
                )
                if base_v != cls and base_v not in data:
                    data[base_v] = barrel
                base_v2 = re.sub(r"_F$", "_base_F", cls)
                if base_v2 != cls and base_v2 not in data:
                    data[base_v2] = barrel
    return data


# --- Fallback defaults for common weapons not in ACE3 ---

FALLBACK_BARRELS = {
    # === Real firearms needing barrel data ===
    # P90/Vermin (SMG_03 family) — ACE3: P90=407mm, P90C=264mm
    "SMG_03_black": 407.0,
    "SMG_03_TR_black": 407.0,
    "SMG_03_TR_hex": 407.0,
    "SMG_03C_black": 264.0,
    "SMG_03C_TR_black": 264.0,
    "SMG_03_TR_camo_F": 407.0,
    "SMG_03C_TR_hex": 264.0,
    # Shotguns — ACE3 data: Hunter=699.3mm, Sawedoff=349.7mm
    "sgun_HunterShotgun_01_F": 699.3,
    "sgun_HunterShotgun_01_sawedoff_F": 349.7,
    # DM rifle variant
    "srifle_DMR_06_hunter_F": 558.8,
    # Heavy MGs — ACE3 has HMG_M2=1143mm
    "HMG_127": 1143.0,
    "ACE_HMG_127_KORD": 1143.0,  # similar class
    # Vehicle-mounted MGs (same platforms, same barrels)
    "LMG_65mm_body": 550.0,  # ~same as MMG_01
    "LMG_M200": 381.0,  # ~same as Mk200
    "LMG_Minigun": 560.0,  # M134 Minigun
    "M134_minigun": 560.0,  # M134 Minigun
    "MMG_01_vehicle": 550.0,  # same as MMG_01
    "MMG_02_vehicle": 609.6,  # same as MMG_02
    # Autocannons (for reference)
    "Cannon_30mm_Plane_CAS_02_F": 2000.0,
    "Gatling_30mm_Plane_CAS_01_F": 2000.0,
    "weapon_Fighter_Gun20mm_AA": 1500.0,
    "weapon_Fighter_Gun_30mm": 2000.0,
    # Grenade launchers (not conventional firearms)
    "GMG_20mm": None,
    "GMG_40mm": None,
    # Demining disruptors (not firearms)
    "DeminingDisruptor_01_base_f": None,
    "DeminingDisruptor_01_F": None,
    # Launchers (rockets/missiles, no barrel length applicable)
    "launch_RPG32_F": None,
    "launch_NLAW_F": None,
    "launch_Titan_short_F": None,
    "launch_Titan_F": None,
    "launch_MRAWS_olive_F": None,
}


def _try_strip(cls, data):
    """Try to match cls in data by progressively stripping suffixes."""
    if cls in data:
        return data[cls]

    # Try converting _base_F to _F (from ACE3 entries with _base naming)
    stripped = re.sub(r"_base_F$", "_F", cls)
    if stripped != cls and stripped in data:
        return data[stripped]

    # Iteratively strip trailing suffix chunks
    current = cls
    for _ in range(5):
        prev = current
        # Strip color/camo suffix
        current = re.sub(
            r"_(blk|khk|snd|hex|ghex|arid|lush|camo|black|green|sand|coyote|olive|tropic|m81|fleck|rgr|exp|tna|base)_F$",
            "_F",
            current,
        )
        # Strip GL/UBS/Mark/TR/C/S suffixes (standalone)
        current = re.sub(r"_(GL|UBS|Mark|TR|C|S)_F$", "_F", current)
        # Strip sawedoff
        current = re.sub(r"_sawedoff_F$", "_F", current)
        if current == prev:
            break
        if current in data:
            return data[current]
        # Also try with _base_F (some ACE3 entries use _base naming)
        base_try = re.sub(r"_F$", "_base_F", current)
        if base_try in data:
            return data[base_try]
    return None


def get_barrel_length(weapon_class, ace_data, cdlc_data):
    """Get barrel length from ACE3 data, CDLC data, or fallback."""
    merged = {**ace_data, **cdlc_data}

    result = _try_strip(weapon_class, merged)
    if result is not None:
        return result

    if weapon_class in FALLBACK_BARRELS:
        return FALLBACK_BARRELS[weapon_class]
    return "_UNKNOWN_"


# --- Main ---


def main():
    ace_data = load_barrel_file(BARREL_MAIN)
    cdlc_data = load_barrel_file(BARREL_CDLC)
    print(f"Loaded {len(ace_data)} ACE3 main + {len(cdlc_data)} CDLC barrel entries")

    updated = 0
    skipped = 0
    missing = []

    for root, _dirs, files in os.walk(WEAPONS_DIR):
        for fname in sorted(files):
            if not fname.endswith(".json"):
                continue
            fpath = os.path.join(root, fname)
            with open(fpath) as f:
                weapon = json.load(f)

            cls = weapon.get("class", "")
            if not cls:
                continue

            # Already populated?
            if "barrel_length_mm" in weapon and weapon["barrel_length_mm"] is not None:
                skipped += 1
                continue

            barrel = get_barrel_length(cls, ace_data, cdlc_data)
            if barrel is None:
                continue  # explicitly skipped (launchers, disruptors)
            if barrel == "_UNKNOWN_":
                missing.append(cls)
                continue

            weapon["barrel_length_mm"] = barrel

            # Update notes: replace "Barrel: ?mm." placeholder
            if "notes" in weapon:
                weapon["notes"] = re.sub(
                    r"Barrel:\s*\?+mm",
                    f"Barrel: {barrel:.0f}mm"
                    if barrel == int(barrel)
                    else f"Barrel: {barrel:.1f}mm",
                    weapon["notes"],
                )

            with open(fpath, "w") as f:
                json.dump(weapon, f, indent=2)
                f.write("\n")
            updated += 1
            print(f"  ✓ {cls}: {barrel}mm")

    print(f"\nUpdated: {updated}, already had: {skipped}, missing: {len(missing)}")
    if missing:
        print(f"Missing barrel data for: {', '.join(missing[:20])}")
        if len(missing) > 20:
            print(f"  ... and {len(missing) - 20} more")


if __name__ == "__main__":
    main()
