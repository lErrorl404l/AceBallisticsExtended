#!/usr/bin/env python3
"""Add chamber_pressure_mpa to ammo JSONs by caliber/cartridge.

Uses SAAMI/CIP standard maximum chamber pressures.
Mapping is by cartridge name (extracted from class name) and caliber.
"""

import json, os, re, glob

AMMO_DIRS = [
    "data/ammo/handgun",
    "data/ammo/rifle",
    "data/ammo/heavy_127mm",
    "data/ammo/launcher",
    "data/ammo/shotgun",
    "data/ammo/apfsds",
]

# SAAMI/CIP standard maximum chamber pressures (MPa)
# Sourced from SAAMI Z299.1 and CIP TDCC standards
#
# Key references:
#   NATO EPVAT: 5.56×45mm = 430 MPa, 7.62×51mm = 415 MPa, 9×19mm = 241 MPa
#   SAAMI: .45 ACP = 145 MPa, .50 BMG = 379 MPa
#   CIP: 7.62×39mm = 355 MPa, 5.45×39mm = 355 MPa, .338 Lapua = 420 MPa
#
# Format: (cartridge_match_type, identifying_string, pressure_mpa)
#   match_type: "exact" = whole class name contains string
#               "caliber" = caliber_mm within range, plus cartidge name check

PRESSURE_MAP = [
    # === Handgun cartridges ===
    # 9×19mm Parabellum (9mm Luger) — SAAMI max 35,000 psi = 241 MPa
    ("class_contains", "9x19", 241.0),
    ("class_contains", "9x21", 241.0),  # 9×21mm similar pressure
    ("class_contains", "9mm", 241.0),
    # .45 ACP — SAAMI max 21,000 psi = 145 MPa
    ("class_contains", "45ACP", 145.0),
    # 4.6×30mm (HK MP7) — CIP 400 MPa
    ("class_contains", "46x30", 400.0),
    ("class_contains", "4.6x30", 400.0),
    # 5.7×28mm (FN P90/Five-seveN) — SAAMI 50,000 psi = 345 MPa
    ("class_contains", "570x28", 345.0),
    ("class_contains", "57x28", 345.0),
    # .50 AE (Action Express) — 36,000 psi = 248 MPa
    ("class_contains", "50BW", 248.0),
    # === Rifle / Intermediate cartridges ===
    # 5.56×45mm NATO — NATO EPVAT 430 MPa
    ("class_contains", "556x45", 430.0),
    ("class_contains", "5.56", 430.0),
    ("caliber_and_class", (5.55, 5.58, "556"), 430.0),
    # 5.45×39mm — CIP 355 MPa
    ("class_contains", "545x39", 355.0),
    ("class_contains", "5.45", 355.0),
    # 5.8×42mm (QBZ-95) — Chinese standard ~290 MPa
    ("class_contains", "58x42", 290.0),
    ("class_contains", "5.8", 290.0),
    # 6.5mm Creedmoor — SAAMI 62,000 psi = 430 MPa
    ("class_contains", "65Creedmoor", 430.0),
    ("class_contains", "6.5Creedmoor", 430.0),
    # 6.5mm Grendel — SAAMI 52,000 psi = 360 MPa
    ("class_contains", "65Grendel", 360.0),
    ("class_contains", "6.5Grendel", 360.0),
    ("caliber_and_class", (6.48, 6.52, ""), 0),  # placeholder, not used
    # 7.62×39mm — CIP 355 MPa
    ("class_contains", "762x39", 355.0),
    ("class_contains", "7.62x39", 355.0),
    # 7.62×51mm NATO — NATO EPVAT 415 MPa
    ("class_contains", "762x51", 415.0),
    ("class_contains", "7.62x51", 415.0),
    ("class_contains", "7.62NATO", 415.0),
    # 7.62×54mmR — CIP 390 MPa
    ("class_contains", "762x54", 390.0),
    ("class_contains", "7.62x54", 390.0),
    # .300 Winchester Magnum — SAAMI 64,000 psi = 441 MPa
    ("class_contains", "300WinMag", 441.0),
    ("class_contains", "300WM", 441.0),
    # .308 Winchester (same as 7.62×51mm in practice) — 415 MPa
    ("class_contains", "308Win", 415.0),
    # .338 Lapua Magnum — CIP 420 MPa
    ("class_contains", "338Lapua", 420.0),
    ("class_contains", "338LM", 420.0),
    # .408 CheyTac — CIP 440 MPa
    ("class_contains", "408CheyTac", 440.0),
    # === Heavy machine gun / Anti-materiel ===
    # .50 BMG (12.7×99mm NATO) — SAAMI 55,000 psi = 379 MPa
    ("class_contains", "127x99", 379.0),
    ("class_contains", "12.7x99", 379.0),
    ("class_contains", "50BMG", 379.0),
    # 12.7×108mm — Soviet/Russian ~360 MPa
    ("class_contains", "127x108", 360.0),
    ("class_contains", "12.7x108", 360.0),
    # 12.7×54mm (VSSK Vychlop) — subsonic ~300 MPa
    ("class_contains", "127x54", 300.0),
    ("class_contains", "12.7x54", 300.0),
    # 12.7×33mm (Soviet pistol round for MTs-3) — ~250 MPa
    ("class_contains", "127x33", 250.0),
    ("class_contains", "12.7x33", 250.0),
    # 14.5×114mm — Soviet ~360 MPa
    ("class_contains", "145x114", 360.0),
    ("class_contains", "14.5x114", 360.0),
    # === Autocannon / Aircraft ===
    # 19mm (B_19mm_HE autocannon) — 380 MPa
    ("class_contains", "19mm", 380.0),
    # 20×102mm (M61 Vulcan / M197) — 380 MPa
    ("class_contains", "20mm", 380.0),
    ("caliber_and_class", (19.8, 20.2, "20"), 380.0),
    # 25×137mm (Bushmaster) — 380 MPa
    ("class_contains", "25mm", 380.0),
    # 30mm (various GAU-8, 2A42, etc)
    ("class_contains", "30mm_APFSDS", 430.0),  # APFSDS discarding sabot
    ("class_contains", "30mm_AP", 430.0),
    ("class_contains", "30mm_HE", 365.0),
    ("class_contains", "30mm_MP", 365.0),
    ("caliber_and_class", (29.8, 30.2, "30"), 365.0),
    # 35mm (Oerlikon GDM) — 380 MPa
    ("class_contains", "35mm", 380.0),
    # 40mm grenade launcher (low pressure) — 70-80 MPa
    ("class_contains", "40mm_HE", 80.0),
    ("class_contains", "40mm_HEDP", 80.0),
    ("class_contains", "40mm_GPR", 80.0),
    ("class_contains", "40mm_APFSDS", 80.0),
    ("caliber_and_class", (39.8, 40.2, "40mm"), 80.0),
    # === Shotgun ===
    # 12 gauge (18.5mm) — 79 MPa (SAAMI 12 ga max)
    ("class_contains", "12Gauge", 79.0),
    # === Rockets / Missiles (reference only, pressure not applicable) ===
    ("class_contains", "R_PG32", None),
    ("class_contains", "R_PG7", None),
    ("class_contains", "R_MAAWS", None),
    ("class_contains", "M_NLAW", None),
    ("class_contains", "M_Titan", None),
    ("class_contains", "M_Vorona", None),
    ("class_contains", "M_PCML", None),
    # === Penetrators (APFSDS — use propellant pressure) ===
    ("class_contains", "m829", 580.0),  # M829 series: ~580 MPa (120mm gun)
    ("class_contains", "m2a3_efp", None),  # EFP warhead, not gun-fired
    ("class_contains", "rpg_29", None),  # Rocket, not conventional pressure
    ("class_contains", "R_PG32", None),
    ("class_contains", "R_PG7", None),
    ("class_contains", "R_MAAWS", None),
    ("class_contains", "M_NLAW", None),
    ("class_contains", "M_Titan", None),
    ("class_contains", "M_Vorona", None),
    ("class_contains", "M_PCML", None),
]


# Additional caliber-based mapping for generic cases
def caliber_to_pressure(cal_mm):
    """Fallback: rough pressure by caliber for autocannons."""
    if 29.5 <= cal_mm <= 30.5:
        return 365.0
    elif 19.5 <= cal_mm <= 20.5:
        return 380.0
    elif 24.5 <= cal_mm <= 25.5:
        return 380.0
    elif 34.5 <= cal_mm <= 35.5:
        return 380.0
    elif 39.5 <= cal_mm <= 40.5:
        return 80.0
    return None


def find_pressure(class_name, caliber_mm):
    """Find chamber pressure for a given ammo class and caliber."""
    # Try explicit class-name containment matches first
    for match_type, key, pressure in PRESSURE_MAP:
        if match_type == "class_contains":
            if key in class_name:
                return pressure
        elif match_type == "caliber_and_class":
            lo, hi, class_key = key
            if lo <= caliber_mm <= hi and (not class_key or class_key in class_name):
                return pressure

    # Fallback: try by caliber alone
    return caliber_to_pressure(caliber_mm)


def main():
    updated = 0
    skipped = 0
    missing = []

    for d in AMMO_DIRS:
        if not os.path.isdir(d):
            continue
        for root, _dirs, files in os.walk(d):
            for fname in sorted(files):
                if not fname.endswith(".json"):
                    continue
                fpath = os.path.join(root, fname)
                with open(fpath) as f:
                    ammo = json.load(f)

                cls = ammo.get("class", "")
                proj = ammo.get("projectile", ammo)
                cal = proj.get("caliber_mm", 0)

                # Already has chamber_pressure_mpa?
                if (
                    "chamber_pressure_mpa" in ammo
                    and ammo["chamber_pressure_mpa"] is not None
                ):
                    skipped += 1
                    continue

                # For missiles/rockets, explicitly skip
                if any(
                    x in cls
                    for x in [
                        "R_PG32",
                        "R_PG7",
                        "R_MAAWS",
                        "M_NLAW",
                        "M_Titan",
                        "M_Vorona",
                        "M_PCML",
                    ]
                ):
                    skipped += 1
                    continue

                if cal is None or cal == 0:
                    missing.append(f"{cls} (cal={cal}, can't map)")
                    continue

                pressure = find_pressure(cls, cal)
                if pressure is None:
                    missing.append(f"{cls} (cal={cal})")
                    continue

                ammo["chamber_pressure_mpa"] = pressure

                # Add notes if not present
                if "notes" not in ammo:
                    ammo["notes"] = ""
                notes = ammo["notes"]

                # Don't duplicate
                if "Chamber" not in notes:
                    note_add = f"Chamber pressure: {pressure:.0f} MPa"
                    if notes:
                        ammo["notes"] = notes.rstrip(".") + ". " + note_add + "."
                    else:
                        ammo["notes"] = note_add + "."

                with open(fpath, "w") as f:
                    json.dump(ammo, f, indent=2)
                    f.write("\n")
                updated += 1
                print(f"  ✓ {cls}: {pressure} MPa")

    print(f"\nUpdated: {updated}, already had: {skipped}, missing: {len(missing)}")
    if missing:
        print(f"Missing chamber pressure for: {', '.join(missing[:20])}")
        if len(missing) > 20:
            print(f"  ... and {len(missing) - 20} more")


if __name__ == "__main__":
    main()
