#!/usr/bin/env python3
"""
Populate missing IRL (real-world) values for all weapons.

Fills in:
  - rifling_twist_mm / twist_rate_mm
  - chamber_pressure_mpa
  - projectile_mass_g
  - muzzle_velocity_ms (when missing)

Uses a comprehensive mapping of Arma class -> real-world firearm -> IRL specs.

Sources:
  - Twist rates: manufacturer specifications, firearms manuals
  - Chamber pressures: SAAMI Z299.1, CIP TDCC, NATO EPVAT
  - Projectile masses: standard military/ commercial loads
  - Muzzle velocities: from reference barrel lengths
"""

import json, os, re, sys

WEAPONS_DIR = "data/weapons"

# ─── IRL Weapon Data Table ────────────────────────────────────────────────────
# Maps Arma class patterns to real-world IRL data.
# Format: (class_substring, caliber, twist_mm, pressure_mpa, mass_g, mv_ms, real_name)
# Ordered by specificity (more specific patterns first).

IRL_WEAPON_DATA = [
    # === AR-15 / M16 family (5.56×45mm NATO) ===
    # Twist: M16A2/A4 1:7" (178mm), M4A1 1:7", HK416 1:7"
    # Pressure: NATO EPVAT 430 MPa (SAAMI 5.56mm: 430 MPa)
    # Mass: M855 4.0g (62gr), M193 3.56g (55gr)
    # MV: M4A1 14.5" → 880 m/s (M855), M16A4 20" → 948 m/s (M855)
    ("arifle_M16A4", 5.56, 178.0, 430.0, 4.0, 948, "M16A4"),
    ("arifle_M4A1", 5.56, 178.0, 430.0, 4.0, 880, "M4A1"),
    ("rhs_weap_m16a4", 5.56, 177.8, 430.0, 4.0, 948, "M16A4 (RHS)"),
    ("rhs_weap_m4a1", 5.56, 177.8, 430.0, 4.0, 880, "M4A1 (RHS)"),
    ("rhs_weap_mk18", 5.56, 177.8, 430.0, 4.0, 820, "MK18 CQBR"),
    ("rhs_weap_hk416_d10", 5.56, 177.8, 430.0, 4.0, 825, 'HK416 10.4"'),
    ("rhs_weap_hk416_d145", 5.56, 177.8, 430.0, 4.0, 885, 'HK416 14.5"'),
    # === AR-15 derivatives (generic 5.56) ===
    ("arifle_SPAR_01", 5.56, 178.0, 430.0, 4.0, 850, "SPAR-16 carbine"),
    ("arifle_SPAR_02", 5.56, 178.0, 430.0, 4.0, 880, "SPAR-16 rifle"),
    ("arifle_SPAR_03", 7.62, 305.0, 415.0, 9.5, 790, "SPAR-17"),
    ("arifle_Mk20", 5.56, 178.0, 430.0, 4.0, 910, "Mk20 SCAR-like"),
    ("arifle_Mk20C", 5.56, 178.0, 430.0, 4.0, 870, "Mk20 CQC"),
    ("arifle_SDAR", 5.56, 178.0, 430.0, 4.0, 850, "SDAR underwater rifle"),
    ("arifle_TRG20", 5.56, 178.0, 430.0, 4.0, 800, "TRG-20 carbine"),
    ("arifle_TRG21", 5.56, 178.0, 430.0, 4.0, 920, "TRG-21"),
    # === AK family ===
    # AK-47/AKM: 7.62×39mm, 1:9.45" (240mm), CIP 355 MPa, 8g (123gr) FMJ
    # AK-74: 5.45×39mm, 1:7.87" (200mm), CIP 355 MPa, 3.43g (53gr) 7N6
    # AK-12: 7.62×39mm, 1:9.45" (240mm)
    # RPK: extended barrel, same twist
    ("arifle_RPK12", 7.62, 240.0, 355.0, 8.0, 730, "RPK-12 (LMG)"),
    ("arifle_AK12U", 7.62, 240.0, 355.0, 8.0, 680, "AK-12U (carbine)"),
    ("arifle_AK12", 7.62, 240.0, 355.0, 8.0, 715, "AK-12"),
    ("arifle_AKM", 7.62, 240.0, 355.0, 8.0, 715, "AKM"),
    ("arifle_AKS", 5.45, 200.0, 355.0, 3.43, 735, "AKS-74U"),
    ("arifle_AK74M", 5.45, 200.0, 355.0, 3.43, 900, "AK-74M"),
    ("rhs_weap_ak103", 7.62, 199.9, 355.0, 7.97, 715, "AK-103"),
    ("rhs_weap_ak104", 7.62, 199.9, 355.0, 7.97, 690, "AK-104 (carbine)"),
    ("rhs_weap_ak105", 5.45, 199.9, 355.0, 3.43, 720, "AK-105 (carbine)"),
    ("rhs_weap_ak74m", 5.45, 199.9, 355.0, 3.43, 880, "AK-74M (RHS)"),
    ("rhs_weap_aks74u", 5.45, 160.0, 355.0, 3.43, 735, "AKS-74U (RHS)"),
    ("rhs_weap_rpk74m", 5.45, 199.9, 355.0, 3.43, 920, "RPK-74M"),
    ("rhs_weap_pkm", 7.62, 240.0, 360.0, 9.85, 825, "PKM"),
    ("rhs_weap_pkp", 7.62, 240.0, 360.0, 9.85, 825, "PKP Pecheneg"),
    # === MX family (fictional - based on 6.5mm Creedmoor / Grendel hybrid) ===
    # Fictional platform, use 6.5mm Grendel-like parameters
    ("arifle_MX_Base", 6.5, 254.0, 380.0, 7.9, 800, "MX 6.5mm (NATO)"),
    ("arifle_MX_SW_Base", 6.5, 254.0, 380.0, 7.9, 810, "MX SW 6.5mm"),
    ("arifle_MXM_Base", 6.5, 254.0, 380.0, 7.9, 830, "MXM 6.5mm (DMR)"),
    ("arifle_MXC_Base", 6.5, 254.0, 380.0, 7.9, 760, "MXC 6.5mm (carbine)"),
    ("arifle_Katiba_Base", 6.5, 254.0, 380.0, 7.9, 780, "Katiba 6.5mm"),
    ("arifle_Katiba_C_Base", 6.5, 254.0, 380.0, 7.9, 740, "Katiba 6.5mm carbine"),
    # === MSBS Grot (Polish, fictional) ===
    # 6.5mm version
    ("arifle_MSBS65", 6.5, 228.6, 380.0, 7.9, 820, "MSBS-65"),
    ("arifle_MSBS65_Mark", 6.5, 228.6, 380.0, 7.9, 830, "MSBS-65 DMR"),
    # === QBZ-191 / Type 191 (Chinese) ===
    ("arifle_QBZ191", 5.8, 185.0, 380.0, 4.45, 750, "QBZ-191"),
    # === CTAR / QBZ-95 (Chinese bullpup) ===
    # 5.8×42mm, twist ~1:7.3" (185mm), ~290 MPa, 4.45g DBP88
    ("arifle_CTAR", 5.8, 185.0, 380.0, 4.5, 750, "CTAR (QBZ-95)"),
    ("arifle_CTARS", 5.8, 185.0, 380.0, 4.5, 770, "CTARS (QBZ-95 LMG)"),
    # === ARX / Type 115 (Italian/Chinese hybrid, fictional) ===
    ("arifle_ARX", 6.5, 254.0, 345.0, 7.5, 800, "ARX 6.5mm"),
    # === HK417 / M110 (7.62×51mm DMR) ===
    ("arifle_HK417", 7.62, 279.0, 415.0, 9.5, 785, "HK417"),
    ("arifle_MCX_Spear", 6.8, 178.0, 380.0, 8.75, 910, "MCX Spear 6.8mm"),
    ("arifle_XM7", 6.8, 178.0, 380.0, 8.75, 910, "XM7 6.8mm"),
    # === RHS / Soviet special ===
    ("rhs_weap_asval", 9.0, 240.0, 280.0, 16.0, 295, "AS Val"),
    ("rhs_weap_vss", 9.0, 240.0, 280.0, 16.0, 295, "VSS Vintorez"),
    # === DMRs ===
    ("srifle_DMR_01", 7.62, 305.0, 415.0, 9.5, 790, "DMR-01 (FAL-type)"),
    ("srifle_DMR_02", 8.6, 238.0, 410.0, 16.2, 900, "DMR-02 .338 LM"),
    ("srifle_DMR_03", 7.62, 285.0, 415.0, 9.5, 790, "DMR-03 (HK417-type)"),
    (
        "srifle_DMR_04_base_F",
        13.0,
        250.0,
        380.0,
        48.2,
        870,
        "DMR-04 .50 cal (ASP-1 Kir)",
    ),
    ("srifle_DMR_04", 12.7, 381.0, 379.0, 42.0, 870, "DMR-04 .50 cal"),
    ("srifle_DMR_05", 9.3, 356.0, 380.0, 18.5, 860, "DMR-05 9.3mm (Cyrus)"),
    ("srifle_DMR_06", 6.5, 254.0, 380.0, 7.9, 830, "DMR-06 6.5mm"),
    ("srifle_DMR_07", 6.5, 254.0, 380.0, 7.9, 830, "DMR-07 6.5mm DMR"),
    ("srifle_EBR", 7.62, 305.0, 415.0, 9.5, 785, "EBR (M14-type)"),
    ("rhs_weap_m14ebrri", 7.62, 304.8, 415.0, 9.5, 800, "M14 EBR (RHS)"),
    ("rhs_weap_sr25", 7.62, 285.8, 415.0, 9.5, 780, "SR-25"),
    ("DMR_07_base", 5.56, 178.0, 380.0, 5.0, 820, "QBU-88"),
    # === Snipers ===
    ("srifle_GM6", 12.7, 381.0, 379.0, 42.0, 890, "GM6 Lynx .50 BMG"),
    ("srifle_LRR", 10.36, 330.0, 420.0, 27.0, 900, "LRR .408 CheyTac"),
    ("srifle_Cyrus", 9.3, 356.0, 380.0, 18.5, 860, "Cyrus 9.3mm"),
    ("srifle_MAR_10", 8.6, 238.0, 410.0, 16.2, 900, "MAR-10 .338 LM"),
    ("srifle_DMR_02_base", 8.6, 238.0, 410.0, 16.2, 900, "MAR-10 .338 LM"),
    ("srifle_DMR_05_base", 9.3, 356.0, 380.0, 18.5, 860, "Cyrus 9.3mm"),
    ("rhs_weap_m24", 7.62, 285.0, 415.0, 11.3, 790, "M24 SWS"),
    ("rhs_weap_svdp", 7.62, 238.8, 360.0, 9.7, 830, "SVD Dragunov"),
    ("rhs_weap_svds", 7.62, 238.8, 360.0, 9.85, 810, "SVDS"),
    # === Pistols ===
    ("hgun_P07", 9.0, 254.0, 241.0, 8.0, 360, "P-07 9mm"),
    ("hgun_Rook40", 9.0, 254.0, 241.0, 8.0, 360, "Rook-40 9mm"),
    ("hgun_ACPC2", 11.43, 406.0, 145.0, 15.0, 260, "ACPC2 .45 ACP"),
    ("hgun_Pistol_heavy_01", 11.43, 406.0, 145.0, 12.0, 250, ".45 Heavy Pistol"),
    ("hgun_Pistol_heavy_02", 11.43, 406.0, 145.0, 14.9, 260, ".45 Zubr"),
    ("hgun_Pistol_01", 9.0, 254.0, 241.0, 8.0, 350, "Compact 9mm"),
    ("SMG_01_Base", 11.43, 406.0, 145.0, 15.0, 280, "Vermin .45 SMG"),
    # === SMGs ===
    ("hgun_PDW2000", 9.0, 254.0, 241.0, 8.0, 380, "PDW2000 9mm"),
    ("SMG_02_Base", 9.0, 254.0, 241.0, 7.5, 400, "SMG-02 9mm"),
    ("SMG_03", 5.7, 228.6, 345.0, 2.0, 716, "P90 5.7×28mm"),
    ("SMG_03C", 5.7, 228.6, 345.0, 2.0, 690, "P90 SBR 5.7×28mm"),
    ("SMG_03_TR", 5.7, 228.6, 345.0, 2.0, 716, "P90 TR 5.7×28mm"),
    ("SMG_03C_TR", 5.7, 228.6, 345.0, 2.0, 690, "P90C TR 5.7×28mm"),
    ("SMG_05", 9.0, 254.0, 241.0, 8.0, 365, "SMG-05 9mm"),
    ("SMG_03_Base", 9.0, 254.0, 280.0, 7.5, 370, "Protector 9mm"),
    ("SMG_02_F", 9.0, 254.0, 241.0, 8.0, 400, "SMG-02"),
    # === Machine Guns ===
    ("LMG_Mk200_base", 6.5, 254.0, 380.0, 7.9, 810, "Mk200 6.5mm LMG"),
    ("LMG_Sandstorm_base", 6.5, 254.0, 380.0, 7.9, 800, "Navid 6.5mm LMG"),
    ("LMG_Zafir_Base", 7.62, 305.0, 360.0, 9.5, 820, "Zafir 7.62mm LMG"),
    ("LMG_MG5_base", 7.62, 305.0, 360.0, 9.5, 810, "MG5 7.62mm LMG"),
    ("LMG_M249", 5.56, 178.0, 430.0, 4.0, 915, "M249 SAW"),
    ("rhs_weap_m249", 5.56, 177.8, 430.0, 4.0, 915, "M249 SAW (RHS)"),
    ("rhs_weap_m240B", 7.62, 304.8, 415.0, 9.5, 840, "M240B"),
    ("rhs_weap_m240g", 7.62, 304.8, 415.0, 9.5, 850, "M240G"),
    ("MMG_02_base", 8.6, 234.95, 431.0, 19.44, 900, "LWMMG 8.6mm"),
    # === Shotguns ===
    ("sgun_HunterShotgun_01", 18.5, 600.0, 79.0, 28.0, 470, "Hunter 12ga"),
    # === Heavy MGs ===
    ("HMG_127", 12.7, 381.0, 379.0, 42.0, 890, "M2HB .50 BMG"),
    ("ACE_HMG_127_KORD", 12.7, 381.0, 360.0, 50.0, 860, "Kord 12.7mm"),
    # === Autocannons ===
    ("Cannon_30mm_Plane_CAS_02", 30.0, 240.0, 430.0, 350.0, 1030, "30mm autocannon"),
    ("Gatling_30mm_Plane_CAS_01", 30.0, 240.0, 430.0, 350.0, 1030, "30mm GAU-8"),
    ("weapon_Fighter_Gun20mm_AA", 20.0, 240.0, 380.0, 100.0, 1030, "20mm fighter gun"),
    ("weapon_Fighter_Gun_30mm", 30.0, 240.0, 430.0, 350.0, 1030, "30mm fighter gun"),
    # === Velko / fictional 5.56mm ===
    ("arifle_Velko", 5.56, 178.0, 430.0, 4.0, 850, "Velko 5.56mm"),
]

# ─── Caliber-based defaults ───────────────────────────────────────────────────
# When no specific weapon match is found, use these caliber defaults.

CALIBER_DEFAULTS = {
    5.56: {"twist_mm": 178.0, "pressure_mpa": 430.0, "mass_g": 4.0},
    5.45: {"twist_mm": 200.0, "pressure_mpa": 355.0, "mass_g": 3.43},
    5.7: {"twist_mm": 228.6, "pressure_mpa": 345.0, "mass_g": 2.0},
    5.8: {"twist_mm": 185.0, "pressure_mpa": 380.0, "mass_g": 4.5},
    6.5: {"twist_mm": 254.0, "pressure_mpa": 380.0, "mass_g": 7.9},
    6.8: {"twist_mm": 178.0, "pressure_mpa": 380.0, "mass_g": 8.75},
    7.04: {"twist_mm": 178.0, "pressure_mpa": 380.0, "mass_g": 8.75},
    7.62: {"twist_mm": 305.0, "pressure_mpa": 415.0, "mass_g": 9.5},
    8.6: {"twist_mm": 238.0, "pressure_mpa": 410.0, "mass_g": 16.2},
    9.0: {"twist_mm": 254.0, "pressure_mpa": 241.0, "mass_g": 8.0},
    9.3: {"twist_mm": 356.0, "pressure_mpa": 380.0, "mass_g": 18.5},
    10.36: {"twist_mm": 330.0, "pressure_mpa": 420.0, "mass_g": 27.0},
    11.43: {"twist_mm": 406.0, "pressure_mpa": 145.0, "mass_g": 15.0},
    12.7: {"twist_mm": 381.0, "pressure_mpa": 379.0, "mass_g": 42.0},
    13.01: {"twist_mm": 250.0, "pressure_mpa": 380.0, "mass_g": 48.2},
    18.5: {"twist_mm": 600.0, "pressure_mpa": 79.0, "mass_g": 28.0},
    20.0: {"twist_mm": 240.0, "pressure_mpa": 380.0, "mass_g": 100.0},
    30.0: {"twist_mm": 240.0, "pressure_mpa": 430.0, "mass_g": 350.0},
}

# ─── Color/camo variant detection ─────────────────────────────────────────────

COLOR_SUFFIXES = {
    "_blk",
    "_khk",
    "_snd",
    "_hex",
    "_ghex",
    "_arid",
    "_lush",
    "_camo",
    "_black",
    "_green",
    "_sand",
    "_coyote",
    "_olive",
    "_tropic",
    "_m81",
    "_fleck",
    "_rgr",
    "_exp",
    "_tna",
    "_base",
}

# ─── Helpers ──────────────────────────────────────────────────────────────────


def find_irl_data(class_name, caliber_mm):
    """Find IRL data for a weapon by class name match.

    Only matches if the pattern's caliber is within 1mm of the weapon's
    actual caliber, to prevent e.g. DMR-04 pattern (12.7mm) matching
    DMR-01 (7.62mm) via substring.
    """
    best = None
    best_len = 0

    for pattern, cal, twist, pressure, mass, mv, name in IRL_WEAPON_DATA:
        if pattern in class_name:
            # Skip pattern if caliber discrepancy > 1mm
            if abs(cal - caliber_mm) > 1.0:
                continue
            # Prefer longest match (most specific)
            if len(pattern) > best_len:
                twist_mm = twist or CALIBER_DEFAULTS.get(cal, {}).get("twist_mm")
                press = pressure or CALIBER_DEFAULTS.get(cal, {}).get("pressure_mpa")
                mass_g = mass or CALIBER_DEFAULTS.get(cal, {}).get("mass_g")
                if mass_g and mass_g > 500:
                    mass_g = None  # sanity: skip autocannon masses for SMGs
                best = (twist_mm, press, mass_g, mv, name, cal)
                best_len = len(pattern)

    return best


def get_caliber_default(cal):
    """Get caliber-based defaults."""
    # Find closest caliber
    for cal_key, defaults in sorted(CALIBER_DEFAULTS.items()):
        if abs(cal - cal_key) < 0.5:
            return defaults
    return None


def normalize_class_name(cls):
    """Strip color suffixes to find base class."""
    # Strip recognized suffixes
    for suffix in COLOR_SUFFIXES:
        if cls.endswith(suffix + "_F"):
            base = cls[: -len(suffix) - 2] + "_F"
            return base
        if suffix == "_base" and cls.endswith("_base_F"):
            return cls
    return cls


# ─── Main ─────────────────────────────────────────────────────────────────────


def main():
    """Main: for each weapon, if a specific IRL match exists (curated table),
    OVERWRITE all fields. If only caliber default, fill missing fields only."""
    updated_irl = 0
    updated_fallback = 0
    skipped = 0

    for root, _dirs, files in os.walk(WEAPONS_DIR):
        for fname in sorted(files):
            if not fname.endswith(".json"):
                continue
            fpath = os.path.join(root, fname)
            with open(fpath) as f:
                weapon = json.load(f)

            cls = weapon.get("class", "")
            cal = weapon.get("caliber_mm", 0)
            if not cls or not cal:
                continue

            old_note = weapon.get("notes", "")
            changed = False
            is_launcher = "launch_" in cls or "GMG_" in cls or "Demining" in cls

            # Try IRL match first (curated data, authoritative)
            irl = find_irl_data(cls, cal)

            if irl and irl[3] and is_launcher:
                # Don't apply IRL MV data to launchers (not conventional firearms)
                irl = None

            if irl:
                # IRL MATCH FOUND → overwrite all fields with IRL data
                twist_key = "rifling_twist_mm"
                if irl[0]:  # twist
                    weapon[twist_key] = irl[0]
                if irl[1]:  # pressure
                    weapon["chamber_pressure_mpa"] = irl[1]
                if irl[2]:  # mass
                    weapon["projectile_mass_g"] = irl[2]
                if irl[3] and not is_launcher:  # MV
                    weapon["muzzle_velocity_ms"] = irl[3]

                changed = True
                updated_irl += 1

                # Update notes with IRL source
                new_note = f"IRL: {irl[4]}"
                if old_note and new_note not in old_note:
                    weapon["notes"] = f"{old_note.rstrip('.')}. {new_note}."
                elif not old_note:
                    weapon["notes"] = f"{new_note}."

            else:
                # No IRL match → fill missing fields with caliber defaults only
                cal_def = get_caliber_default(cal)
                if cal_def and not is_launcher:
                    # Only fill if missing
                    twist_key = "rifling_twist_mm"
                    if twist_key not in weapon and "twist_rate_mm" not in weapon:
                        weapon[twist_key] = cal_def["twist_mm"]
                        changed = True
                    if "chamber_pressure_mpa" not in weapon:
                        weapon["chamber_pressure_mpa"] = cal_def["pressure_mpa"]
                        changed = True
                    if "projectile_mass_g" not in weapon:
                        weapon["projectile_mass_g"] = cal_def["mass_g"]
                        changed = True

                    if changed:
                        updated_fallback += 1
                        if "IRL default" not in weapon.get("notes", ""):
                            src = "IRL default"
                            if old_note:
                                weapon["notes"] = f"{old_note.rstrip('.')}. {src}."
                            else:
                                weapon["notes"] = f"{src}."
                else:
                    skipped += 1
                    continue

            if changed:
                with open(fpath, "w") as f:
                    json.dump(weapon, f, indent=2)
                    f.write("\n")
                print(
                    f"  {'✓ IRL' if irl else '✓ DEF'} {cls}: "
                    f"twist={weapon.get('rifling_twist_mm', '?')} "
                    f"press={weapon.get('chamber_pressure_mpa', '?')} "
                    f"mass={weapon.get('projectile_mass_g', '?')} "
                    f"mv={weapon.get('muzzle_velocity_ms', '?')} "
                    f"({irl[4] if irl else 'caliber default'})"
                )
            else:
                skipped += 1

    print(
        f"\nUpdated IRL: {updated_irl}, Updated fallback: {updated_fallback}, Skipped: {skipped}"
    )


if __name__ == "__main__":
    main()
