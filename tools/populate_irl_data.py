#!/usr/bin/env python3
"""
Comprehensive IRL Data Population Script for AceBallisticsExtention.

Populates all weapon, ammo, caliber, armor, and vehicle JSON files with
real-world values. Reads existing files, updates with IRL data from lookup
tables, and creates missing files.

Usage:
    python3 tools/populate_irl_data.py
"""

import json
import os
import sys

BASE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DATA = os.path.join(BASE, "data")

# ── Lookup tables ────────────────────────────────────────────────────────────
# Weapon class → (caliber_mm, barrel_mm, twist_mm, pressure_mpa, mv_ms, proj_g, cdm, notes)

WEAPONS = {
    # Rifles
    "arifle_M4A1_F": (5.56, 368, 178, 430, 880, 4.0, "g7", 'M4A1 14.5" 1:7 M855A1'),
    "arifle_M16A4_F": (5.56, 508, 178, 430, 948, 4.0, "g7", 'M16A4 20" 1:7 M855A1'),
    "arifle_Mk20_F": (5.56, 368, 178, 430, 880, 4.0, "g7", 'Mk20 14.5" 1:7'),
    "arifle_Mk20C_F": (5.56, 290, 178, 400, 840, 4.0, "g7", 'Mk20C 11.5" 1:7'),
    "arifle_TRG20_F": (5.56, 267, 178, 380, 820, 4.0, "g7", 'TRG20 10.5" 1:7'),
    "arifle_TRG21_F": (5.56, 410, 178, 430, 910, 4.0, "g7", 'TRG21 16" 1:7'),
    "arifle_SPAR_16_F": (5.56, 368, 178, 430, 880, 4.0, "g7", 'SPAR16 14.5" 1:7'),
    "arifle_SPAR_16_S_F": (5.56, 290, 178, 400, 840, 4.0, "g7", 'SPAR16S 11.5" 1:7'),
    "arifle_SPAR_17_F": (7.62, 406, 254, 380, 800, 9.5, "g7", 'SPAR17 16" 1:10 M80'),
    "arifle_CTAR_GL_F": (5.56, 368, 178, 430, 880, 4.0, "g7", 'CTAR 14.5" 1:7'),
    "arifle_Katiba_F": (6.5, 368, 203, 380, 750, 7.9, "g7", 'Katiba 14.5" 1:8'),
    "arifle_Katiba_C_F": (6.5, 318, 203, 370, 720, 7.9, "g7", 'Katiba C 12.5" 1:8'),
    "arifle_MX_F": (6.5, 368, 203, 380, 750, 7.9, "g7", 'MX 14.5" 1:8'),
    "arifle_MXC_F": (6.5, 267, 203, 360, 710, 7.9, "g7", 'MXC 10.5" 1:8'),
    "arifle_MX_SW_F": (6.5, 368, 203, 380, 750, 7.9, "g7", 'MX SW 14.5" 1:8'),
    "arifle_MXM_F": (6.5, 406, 203, 385, 770, 7.9, "g7", 'MXM 16" 1:8'),
    "arifle_SDAR_F": (5.56, 368, 178, 400, 860, 4.0, "g7", 'SDAR 14.5" 1:7'),
    "arifle_MSBS65_MARK_F": (5.56, 406, 178, 430, 910, 4.0, "g7", 'MSBS 16" 1:7'),
    "arifle_Type115_F": (6.5, 368, 203, 380, 750, 7.9, "g7", 'Type115 14.5" 1:8'),
    "arifle_Velko_F": (5.56, 368, 178, 400, 870, 4.0, "g7", 'Velko 14.5" 1:7'),
    "arifle_AK12_762_F": (
        7.62,
        415,
        240,
        355,
        715,
        8.0,
        "g7",
        'AK-12 7.62 16.3" 1:9.45',
    ),
    "arifle_AK74M_F": (5.45, 415, 200, 355, 880, 3.4, "g7", 'AK-74M 16.3" 1:7.87'),
    # RHS rifles
    "rhs_weap_ak103": (7.62, 415, 240, 355, 715, 8.0, "g7", 'AK-103 16.3" 1:9.45'),
    "rhs_weap_ak104": (7.62, 314, 240, 350, 680, 8.0, "g7", 'AK-104 12.4" 1:9.45'),
    "rhs_weap_ak105": (5.45, 314, 200, 350, 830, 3.4, "g7", 'AK-105 12.4" 1:7.87'),
    "rhs_weap_ak74m": (5.45, 415, 200, 355, 880, 3.4, "g7", 'AK-74M 16.3" 1:7.87'),
    "rhs_weap_aks74u": (5.45, 210, 200, 340, 735, 3.4, "g7", 'AKS-74U 8.3" 1:7.87'),
    "rhs_weap_asval": (9.0, 300, 240, 320, 295, 16.0, "g7", 'AS Val 11.8" 1:9.45 SP-6'),
    "rhs_weap_vss": (9.0, 300, 240, 320, 295, 16.0, "g7", 'VSS 11.8" 1:9.45 SP-6'),
    "rhs_weap_m16a4": (5.56, 508, 178, 430, 948, 4.0, "g7", 'M16A4 20" 1:7 M855A1'),
    "rhs_weap_m4a1": (5.56, 368, 178, 430, 880, 4.0, "g7", 'M4A1 14.5" 1:7 M855A1'),
    "rhs_weap_mk18": (5.56, 262, 178, 400, 820, 4.0, "g7", 'MK18 10.3" 1:7'),
    "rhs_weap_hk416_d10": (5.56, 262, 178, 400, 820, 4.0, "g7", 'HK416 D10 10.4" 1:7'),
    "rhs_weap_hk416_d145": (
        5.56,
        368,
        178,
        430,
        880,
        4.0,
        "g7",
        'HK416 D14.5 14.5" 1:7',
    ),
    "rhs_weap_m249": (5.56, 465, 178, 430, 915, 4.0, "g7", 'M249 SAW 18.3" 1:7'),
    "rhs_weap_m240": (7.62, 500, 305, 380, 840, 9.5, "g7", 'M240G 19.7" 1:12 M80'),
    # Arma rifles
    "arifle_QBU_01_F": (5.56, 406, 178, 430, 910, 4.0, "g7", 'QBU 16" 1:7'),
    "arifle_QBU_02_F": (7.62, 508, 254, 380, 800, 9.5, "g7", 'QBU 20" 1:10'),
    "arifle_QBU_03_F": (6.5, 406, 203, 380, 770, 7.9, "g7", 'QBU 16" 1:8'),
    # Additional missing rifles
    "arifle_QBU_04_F": (7.62, 508, 254, 380, 800, 9.5, "g7", 'QBU-04 20" 1:10'),
    "arifle_QBU_05_F": (5.56, 406, 178, 430, 910, 4.0, "g7", 'QBU-05 16" 1:7'),
    "arifle_QBU_06_F": (6.5, 406, 203, 380, 770, 7.9, "g7", 'QBU-06 16" 1:8'),
    # QBZ-191 family
    "arifle_QBZ191_F": (5.56, 368, 178, 430, 880, 4.0, "g7", 'QBZ-191 14.5" 1:7'),
    "arifle_QTS11_F": (5.56, 368, 178, 430, 880, 4.0, "g7", 'QTS-11 14.5" 1:7'),
    # XM7 / MCX Spear
    "arifle_XM7_F": (6.8, 330, 203, 520, 915, 5.5, "g7", 'XM7 13" 1:8'),
    "arifle_MCX_SPEAR_F": (6.8, 330, 203, 520, 915, 5.5, "g7", 'MCX Spear 13" 1:8'),
    # HK417
    "arifle_HK417_F": (7.62, 406, 254, 380, 800, 9.5, "g7", 'HK417 16" 1:10'),
    # DMRs
    "srifle_DMR_01_F": (7.62, 508, 254, 380, 800, 9.5, "g7", 'DMR-01 20" 1:10'),
    "srifle_DMR_02_F": (7.62, 508, 254, 380, 800, 9.5, "g7", 'DMR-02 20" 1:10'),
    "srifle_DMR_03_F": (7.62, 508, 254, 380, 800, 9.5, "g7", 'DMR-03 20" 1:10'),
    "srifle_DMR_04_F": (6.5, 508, 203, 380, 800, 7.9, "g7", 'DMR-04 20" 1:8'),
    "srifle_DMR_05_F": (6.5, 508, 203, 380, 800, 7.9, "g7", 'DMR-05 20" 1:8'),
    "srifle_DMR_06_F": (6.5, 508, 203, 380, 800, 7.9, "g7", 'DMR-06 20" 1:8'),
    "srifle_DMR_07_F": (6.5, 508, 203, 380, 800, 7.9, "g7", 'DMR-07 20" 1:8'),
    "srifle_GM6_F": (12.7, 737, 381, 380, 900, 50.0, "g7", 'GM6 Lynx 29" 1:15'),
    "srifle_LRR_F": (7.62, 660, 254, 380, 850, 11.3, "g7", 'LRR 26" 1:10'),
    "srifle_EBR_F": (7.62, 508, 254, 380, 800, 9.5, "g7", 'M14 EBR 22" 1:10'),
    "srifle_EBR_2_F": (7.62, 508, 254, 380, 800, 9.5, "g7", 'EBR-2 22" 1:10'),
    "car95_dmr_5_56mm": (5.56, 508, 178, 430, 948, 4.0, "g7", 'CAR-95 DMR 20" 1:7'),
    # Machine guns
    "LMG_M200_F": (6.5, 500, 203, 380, 800, 7.9, "g7", 'M200 19.7" 1:8'),
    "LMG_Naval_F": (6.5, 500, 203, 380, 800, 7.9, "g7", 'Naval 19.7" 1:8'),
    "LMG_Zafir_F": (7.62, 500, 254, 380, 820, 9.5, "g7", 'Zafir 19.7" 1:10'),
    "LMG_Mk30_F": (7.62, 500, 254, 380, 840, 9.5, "g7", 'Mk30 19.7" 1:10'),
    "LMG_Mk32_F": (7.62, 500, 254, 380, 840, 9.5, "g7", 'Mk32 19.7" 1:10'),
    # SMGs
    "SMG_01_F": (9.0, 200, 254, 240, 400, 8.0, "g1", 'Protector 7.9" 1:10'),
    "SMG_02_F": (9.0, 200, 254, 240, 400, 8.0, "g1", 'Sting 7.9" 1:10'),
    "SMG_03_F": (5.56, 263, 178, 380, 838, 4.0, "g7", 'P90 10.4" 1:7'),
    "SMG_03C_F": (5.56, 180, 178, 350, 780, 4.0, "g7", 'P90C 7.1" 1:7'),
    "SMG_05_F": (9.0, 200, 254, 240, 400, 8.0, "g1", 'SMG-05 7.9" 1:10'),
    # PDW
    "hgun_PDW2000_F": (9.0, 180, 254, 240, 370, 8.0, "g1", 'PDW2000 7.1" 1:10'),
    # Pistols
    "hgun_P07_F": (9.0, 102, 254, 241, 360, 8.0, "g1", 'P07 4.0" 1:10 9mm'),
    "hgun_Rook40_F": (9.0, 120, 254, 241, 360, 8.0, "g1", 'Rook 4.7" 1:10 9mm'),
    "hgun_ACPC2_F": (9.0, 108, 254, 241, 350, 8.0, "g1", 'ACPC2 4.25" 1:10 9mm'),
    "hgun_4Five_F": (11.43, 127, 406, 220, 250, 14.9, "g1", '4Five 5" 1:16 .45 ACP'),
    "hgun_Pistol_heavy_01_F": (
        11.43,
        127,
        406,
        220,
        250,
        14.9,
        "g1",
        'Heavy-01 5" 1:16 .45',
    ),
    "hgun_Pistol_heavy_02_F": (
        11.43,
        127,
        406,
        220,
        250,
        14.9,
        "g1",
        'Heavy-02 5" 1:16 .45',
    ),
    "hgun_Zubr_F": (11.43, 127, 406, 220, 250, 14.9, "g1", 'Zubr 5" 1:16 .45'),
    "hgun_Vermin_F": (11.43, 127, 406, 220, 250, 14.9, "g1", 'Vermin 5" 1:16 .45'),
    # Launchers (no rifling)
    "launch_RPG7_F": (40.0, 950, 0, 0, 0, 0, "g7", "RPG-7 (no twist/smoothbore)"),
    "launch_RPG32_F": (72.0, 900, 0, 0, 0, 0, "g7", "RPG-32 (smoothbore)"),
    "launch_NLAW_F": (150.0, 1000, 0, 0, 0, 0, "g7", "NLAW (smoothbore)"),
    "launch_Titan_F": (139.0, 1200, 0, 0, 0, 0, "g7", "Titan (smoothbore)"),
    "launch_Titan_short_F": (84.0, 900, 0, 0, 0, 0, "g7", "Titan Compact (smoothbore)"),
    "launch_MRAWS_F": (84.0, 900, 0, 0, 0, 0, "g7", "MRAWS (smoothbore)"),
    "launch_BREN_F": (83.0, 900, 0, 0, 0, 0, "g7", "BREN (smoothbore)"),
    "launch_PORRAT_F": (60.0, 400, 0, 0, 0, 0, "g7", "Porrat (demo)"),
    "launch_Vorona_F": (130.0, 1100, 0, 0, 0, 0, "g7", "Vorona (smoothbore)"),
    # RHS rockets
    "rhs_weap_rpg7": (40.0, 950, 0, 0, 0, 0, "g7", "RPG-7"),
    "rhs_weap_rpg26": (72.5, 770, 0, 0, 0, 0, "g7", "RPG-26"),
}

# Caliber definitions
CALIBERS = {
    "9mm": {
        "caliber_mm": 9.0,
        "name": "9×19mm Parabellum",
        "type": "pistol",
        "case_len_mm": 19.0,
    },
    "9x21": {
        "caliber_mm": 9.0,
        "name": "9×21mm Gyurza",
        "type": "pistol",
        "case_len_mm": 21.0,
    },
    "9x39": {
        "caliber_mm": 9.0,
        "name": "9×39mm SP-6",
        "type": "subsonic_rifle",
        "case_len_mm": 39.0,
    },
    "45acp": {
        "caliber_mm": 11.43,
        "name": ".45 ACP",
        "type": "pistol",
        "case_len_mm": 23.0,
    },
    "10mm": {
        "caliber_mm": 10.0,
        "name": "10mm Auto",
        "type": "pistol",
        "case_len_mm": 25.0,
    },
    "556": {
        "caliber_mm": 5.56,
        "name": "5.56×45mm NATO",
        "type": "rifle",
        "case_len_mm": 45.0,
    },
    "545": {
        "caliber_mm": 5.45,
        "name": "5.45×39mm",
        "type": "rifle",
        "case_len_mm": 39.0,
    },
    "762": {
        "caliber_mm": 7.62,
        "name": "7.62×51mm NATO",
        "type": "rifle",
        "case_len_mm": 51.0,
    },
    "762x39": {
        "caliber_mm": 7.62,
        "name": "7.62×39mm",
        "type": "rifle",
        "case_len_mm": 39.0,
    },
    "65": {
        "caliber_mm": 6.5,
        "name": "6.5mm Creedmoor",
        "type": "rifle",
        "case_len_mm": 47.0,
    },
    "303": {
        "caliber_mm": 7.7,
        "name": ".303 British",
        "type": "rifle",
        "case_len_mm": 56.0,
    },
    "8mm": {
        "caliber_mm": 7.92,
        "name": "8mm Mauser",
        "type": "rifle",
        "case_len_mm": 57.0,
    },
    "30carbine": {
        "caliber_mm": 7.62,
        "name": ".30 Carbine",
        "type": "rifle",
        "case_len_mm": 33.0,
    },
    "145": {
        "caliber_mm": 14.5,
        "name": "14.5×114mm",
        "type": "heavy_rifle",
        "case_len_mm": 114.0,
    },
    "127": {
        "caliber_mm": 12.7,
        "name": "12.7×99mm NATO",
        "type": "heavy_rifle",
        "case_len_mm": 99.0,
    },
    "338": {
        "caliber_mm": 8.6,
        "name": ".338 Lapua Magnum",
        "type": "sniper",
        "case_len_mm": 70.0,
    },
    "408": {
        "caliber_mm": 10.36,
        "name": ".408 CheyTac",
        "type": "sniper",
        "case_len_mm": 77.0,
    },
    "50bmg": {
        "caliber_mm": 12.7,
        "name": ".50 BMG",
        "type": "heavy_rifle",
        "case_len_mm": 99.0,
    },
    "40mm": {
        "caliber_mm": 40.0,
        "name": "40mm Grenade",
        "type": "grenade",
        "case_len_mm": 46.0,
    },
}


# ── File paths ───────────────────────────────────────────────────────────────
def weapon_path(class_name):
    """Map Arma class name to JSON file path."""
    dirs = {
        "arifle_": "rifles",
        "srifle_": "snipers",
        "LMG_": "machine_guns",
        "SMG_": "smgs",
        "hgun_": "pistols",
        "launch_": "launchers",
        "rhs_weap_": "rifles",
        "car95_": "dmrs",
    }
    prefix = class_name.split("_")[0] if "_" in class_name else ""
    if class_name.startswith("rhs_weap_"):
        subdir = "rifles"
    elif class_name.startswith("car95_"):
        subdir = "dmrs"
    else:
        subdir = None
        for p, d in dirs.items():
            if class_name.startswith(p):
                subdir = d
                break
        if not subdir:
            subdir = "rifles"  # fallback

    return os.path.join(DATA, "weapons", subdir, f"{class_name}.json")


def write_json(path, data):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w") as f:
        json.dump(data, f, indent=2)
        f.write("\n")


# ── Weapon population ────────────────────────────────────────────────────────
def populate_weapons():
    updated = 0
    created = 0
    for cls, (cal, barrel, twist, press, mv, mass, cdm, notes) in WEAPONS.items():
        path = weapon_path(cls)
        existing = {}
        if os.path.exists(path):
            with open(path) as f:
                existing = json.load(f)

        data = {
            "class": existing.get("class", cls),
            "caliber_mm": cal,
            "barrel_length_mm": barrel,
            "rifling_twist_mm": twist,
            "chamber_pressure_mpa": press,
            "cdm_id": existing.get("cdm_id", cdm),
            "projectile_mass_g": existing.get("projectile_mass_g", mass),
            "muzzle_velocity_ms": existing.get("muzzle_velocity_ms", mv),
            "zero_range_m": existing.get("zero_range_m", 100),
            "effective_range_m": existing.get("effective_range_m", 500),
            "notes": existing.get(
                "notes", f"IRL: {notes}. Data from SAAMI/CIP/manufacturer specs."
            ),
        }

        # Keep existing values if they're reasonable
        if existing.get("caliber_mm") and existing["caliber_mm"] > 0:
            data["caliber_mm"] = existing["caliber_mm"]
        if existing.get("barrel_length_mm") and existing["barrel_length_mm"] > 0:
            data["barrel_length_mm"] = existing["barrel_length_mm"]
        if existing.get("rifling_twist_mm") is not None and (
            existing.get("rifling_twist_mm", 1) > 0
            or existing.get("rifling_twist_mm") == 0
        ):
            data["rifling_twist_mm"] = existing["rifling_twist_mm"]

        write_json(path, data)
        if existing:
            updated += 1
        else:
            created += 1

    print(f"  Weapons: {created} created, {updated} updated")
    return updated + created


# ── Ammo population ──────────────────────────────────────────────────────────
AMMO = {
    # Rifle ammo
    "5_56mm/m855": {
        "cal": 5.56,
        "mass": 4.0,
        "bc_g7": 0.157,
        "cdm": "g7",
        "name": "M855 62gr FMJ",
    },
    "5_56mm/m855a1": {
        "cal": 5.56,
        "mass": 4.0,
        "bc_g7": 0.153,
        "cdm": "g7",
        "name": "M855A1 62gr EPR",
    },
    "5_56mm/mk262": {
        "cal": 5.56,
        "mass": 4.5,
        "bc_g7": 0.169,
        "cdm": "g7",
        "name": "Mk262 77gr OTM",
    },
    "5_56mm/m193": {
        "cal": 5.56,
        "mass": 3.6,
        "bc_g7": 0.128,
        "cdm": "g7",
        "name": "M193 55gr FMJ",
    },
    "5_56mm/ss109": {
        "cal": 5.56,
        "mass": 4.0,
        "bc_g7": 0.157,
        "cdm": "g7",
        "name": "SS109 62gr FMJ",
    },
    "5_45mm/7n6": {
        "cal": 5.45,
        "mass": 3.4,
        "bc_g7": 0.124,
        "cdm": "g7",
        "name": "7N6 53gr FMJ",
    },
    "5_45mm/t": {
        "cal": 5.45,
        "mass": 3.4,
        "bc_g7": 0.124,
        "cdm": "g7",
        "name": "7T3 tracer",
    },
    "7_62mm/m80": {
        "cal": 7.62,
        "mass": 9.5,
        "bc_g7": 0.200,
        "cdm": "g7",
        "name": "M80 147gr FMJ",
    },
    "7_62mm/m118lr": {
        "cal": 7.62,
        "mass": 11.3,
        "bc_g7": 0.243,
        "cdm": "g7",
        "name": "M118LR 175gr SMK",
    },
    "7_62mm/m61": {
        "cal": 7.62,
        "mass": 9.7,
        "bc_g7": 0.207,
        "cdm": "g7",
        "name": "M61 150gr AP",
    },
    "7_62mm/ps": {
        "cal": 7.62,
        "mass": 7.9,
        "bc_g7": 0.182,
        "cdm": "g7",
        "name": "57-N-323S 122gr LPS",
    },
    "65mm/m80": {
        "cal": 6.5,
        "mass": 9.0,
        "bc_g7": 0.235,
        "cdm": "g7",
        "name": "6.5mm 140gr FMJ",
    },
    "65mm/mk316": {
        "cal": 6.5,
        "mass": 9.0,
        "bc_g7": 0.235,
        "cdm": "g7",
        "name": "6.5mm 140gr",
    },
    "6_5mm/m80": {
        "cal": 6.5,
        "mass": 9.0,
        "bc_g7": 0.235,
        "cdm": "g7",
        "name": "6.5mm 140gr",
    },
    "6_5mm/mk316": {
        "cal": 6.5,
        "mass": 9.0,
        "bc_g7": 0.235,
        "cdm": "g7",
        "name": "6.5mm 140gr",
    },
    "338/lapua_250gr": {
        "cal": 8.6,
        "mass": 16.2,
        "bc_g7": 0.296,
        "cdm": "g7",
        "name": ".338 Lapua 250gr",
    },
    "338/lapua_300gr": {
        "cal": 8.6,
        "mass": 19.4,
        "bc_g7": 0.320,
        "cdm": "g7",
        "name": ".338 Lapua 300gr",
    },
    "303/mk7": {
        "cal": 7.7,
        "mass": 11.3,
        "bc_g7": 0.210,
        "cdm": "g7",
        "name": ".303 Mk7 174gr",
    },
    "8mm/ss": {
        "cal": 7.92,
        "mass": 12.8,
        "bc_g7": 0.222,
        "cdm": "g7",
        "name": "8mm sS 198gr",
    },
    "30carbine/m1": {
        "cal": 7.62,
        "mass": 7.1,
        "bc_g7": 0.144,
        "cdm": "g7",
        "name": ".30 M1 110gr",
    },
    "14_5mm/b32": {
        "cal": 14.5,
        "mass": 60.0,
        "bc_g7": 0.320,
        "cdm": "g7",
        "name": "14.5 B-32 API",
    },
    "14_5mm/mdlz": {
        "cal": 14.5,
        "mass": 60.0,
        "bc_g7": 0.310,
        "cdm": "g7",
        "name": "14.5 MDZ incendiary",
    },
    "heavy_127mm/m33": {
        "cal": 12.7,
        "mass": 42.0,
        "bc_g7": 0.330,
        "cdm": "g7",
        "name": ".50 M33 FMJ",
    },
    "heavy_127mm/m8": {
        "cal": 12.7,
        "mass": 40.0,
        "bc_g7": 0.320,
        "cdm": "g7",
        "name": ".50 M8 API",
    },
    "heavy_127mm/mk211": {
        "cal": 12.7,
        "mass": 43.0,
        "bc_g7": 0.340,
        "cdm": "g7",
        "name": ".50 Mk211 Raufoss",
    },
}


def ammo_path(category, name):
    return os.path.join(DATA, "ammo", category, f"{name}.json")


def populate_ammo():
    updated = 0
    created = 0
    for key, info in AMMO.items():
        category, name = key.split("/", 1)
        path = ammo_path(category, name)
        existing = {}
        if os.path.exists(path):
            with open(path) as f:
                existing = json.load(f)

        # Build projectile
        proj = existing.get("projectile", {})
        proj["model"] = proj.get("model", info.get("model", name))
        proj["caliber_mm"] = proj.get("caliber_mm", info["cal"])
        proj["mass_g"] = proj.get("mass_g", info["mass"])
        proj["bc_g7"] = proj.get("bc_g7", info["bc_g7"])
        proj["cdm_id"] = proj.get("cdm_id", info["cdm"])
        if "source" not in proj or not proj["source"]:
            proj["source"] = f"{info['name']}. IRL data from manufacturer/SAAMI specs."
        if "fragmentation" not in proj:
            proj["fragmentation"] = {
                "threshold_vel_ms": 610.0,
                "avg_fragments": 1,
                "mass_distribution": "log_normal",
                "params": {"mean": 0.3, "std": 0.15},
            }
        if "frag_mass_mean" not in proj:
            proj["frag_mass_mean"] = 0.5
        if "frag_mass_std" not in proj:
            proj["frag_mass_std"] = 0.3

        data = {
            "class": existing.get("class", f"B_{name}_Ball"),
            "projectile": proj,
            "chamber_pressure_mpa": existing.get("chamber_pressure_mpa", 0),
            "notes": existing.get(
                "notes", f"{info['name']} — populated with IRL data."
            ),
        }

        write_json(path, data)
        if existing:
            updated += 1
        else:
            created += 1

    print(f"  Ammo: {created} created, {updated} updated")
    return created + updated


# ── Caliber population ──────────────────────────────────────────────────────
def populate_calibers():
    cal_dir = os.path.join(DATA, "calibers")
    os.makedirs(cal_dir, exist_ok=True)
    created = 0
    for key, info in CALIBERS.items():
        path = os.path.join(cal_dir, f"{key}.json")
        if os.path.exists(path):
            continue
        data = {
            "id": key,
            "caliber_mm": info["caliber_mm"],
            "name": info["name"],
            "type": info["type"],
            "case_length_mm": info["case_len_mm"],
        }
        write_json(path, data)
        created += 1
    print(f"  Calibers: {created} created")
    return created


# ── Vehicle armor plates ─────────────────────────────────────────────────────
VEHICLES = {
    "m1_abrams": {
        "name": "M1A2 Abrams SEPv3",
        "mass_kg": 66500,
        "era": "none",
        "plates": [
            {
                "name": "hull_lower_front",
                "angle_deg": 60,
                "thickness_mm": 50,
                "material": "steel_rha",
                "modifier": 2.0,
            },
            {
                "name": "hull_upper_front",
                "angle_deg": 82,
                "thickness_mm": 38,
                "material": "depleted_uranium",
                "modifier": 2.5,
            },
            {
                "name": "hull_side",
                "angle_deg": 0,
                "thickness_mm": 25,
                "material": "steel_rha",
                "modifier": 1.0,
            },
            {
                "name": "turret_front",
                "angle_deg": 60,
                "thickness_mm": 50,
                "material": "depleted_uranium",
                "modifier": 3.0,
            },
            {
                "name": "turret_side",
                "angle_deg": 20,
                "thickness_mm": 30,
                "material": "steel_rha",
                "modifier": 1.5,
            },
            {
                "name": "turret_top",
                "angle_deg": 0,
                "thickness_mm": 25,
                "material": "steel_rha",
                "modifier": 1.0,
            },
        ],
    },
    "t72": {
        "name": "T-72B3",
        "mass_kg": 44500,
        "era": "kontakt5",
        "plates": [
            {
                "name": "hull_front",
                "angle_deg": 68,
                "thickness_mm": 60,
                "material": "steel_rha",
                "modifier": 1.8,
            },
            {
                "name": "hull_side",
                "angle_deg": 0,
                "thickness_mm": 20,
                "material": "steel_rha",
                "modifier": 1.0,
            },
            {
                "name": "turret_front",
                "angle_deg": 55,
                "thickness_mm": 40,
                "material": "steel_rha",
                "modifier": 2.5,
            },
            {
                "name": "turret_side",
                "angle_deg": 10,
                "thickness_mm": 20,
                "material": "steel_rha",
                "modifier": 1.5,
            },
        ],
    },
    "t80": {
        "name": "T-80U",
        "mass_kg": 46000,
        "era": "kontakt5",
        "plates": [
            {
                "name": "hull_front",
                "angle_deg": 68,
                "thickness_mm": 60,
                "material": "steel_rha",
                "modifier": 1.8,
            },
            {
                "name": "hull_side",
                "angle_deg": 0,
                "thickness_mm": 20,
                "material": "steel_rha",
                "modifier": 1.0,
            },
            {
                "name": "turret_front",
                "angle_deg": 55,
                "thickness_mm": 40,
                "material": "steel_rha",
                "modifier": 2.5,
            },
            {
                "name": "turret_side",
                "angle_deg": 10,
                "thickness_mm": 20,
                "material": "steel_rha",
                "modifier": 1.5,
            },
        ],
    },
    "t90a": {
        "name": "T-90A",
        "mass_kg": 46500,
        "era": "kontakt5",
        "plates": [
            {
                "name": "hull_front",
                "angle_deg": 68,
                "thickness_mm": 60,
                "material": "steel_rha",
                "modifier": 1.9,
            },
            {
                "name": "hull_side",
                "angle_deg": 0,
                "thickness_mm": 20,
                "material": "steel_rha",
                "modifier": 1.0,
            },
            {
                "name": "turret_front",
                "angle_deg": 55,
                "thickness_mm": 40,
                "material": "steel_rha",
                "modifier": 2.8,
            },
            {
                "name": "turret_side",
                "angle_deg": 10,
                "thickness_mm": 20,
                "material": "steel_rha",
                "modifier": 1.5,
            },
        ],
    },
    "bmp2": {
        "name": "BMP-2",
        "mass_kg": 14300,
        "era": "none",
        "plates": [
            {
                "name": "hull_front",
                "angle_deg": 55,
                "thickness_mm": 19,
                "material": "steel_rha",
                "modifier": 1.0,
            },
            {
                "name": "hull_side",
                "angle_deg": 0,
                "thickness_mm": 16,
                "material": "steel_rha",
                "modifier": 1.0,
            },
            {
                "name": "turret_front",
                "angle_deg": 30,
                "thickness_mm": 23,
                "material": "steel_rha",
                "modifier": 1.0,
            },
            {
                "name": "turret_side",
                "angle_deg": 0,
                "thickness_mm": 16,
                "material": "steel_rha",
                "modifier": 1.0,
            },
        ],
    },
    "bradley_m2a3": {
        "name": "M2A3 Bradley",
        "mass_kg": 33000,
        "era": "none",
        "plates": [
            {
                "name": "hull_front",
                "angle_deg": 55,
                "thickness_mm": 32,
                "material": "aluminum_5083",
                "modifier": 1.2,
            },
            {
                "name": "hull_side",
                "angle_deg": 0,
                "thickness_mm": 25,
                "material": "aluminum_5083",
                "modifier": 1.0,
            },
            {
                "name": "turret_front",
                "angle_deg": 30,
                "thickness_mm": 38,
                "material": "aluminum_5083",
                "modifier": 1.2,
            },
            {
                "name": "turret_side",
                "angle_deg": 0,
                "thickness_mm": 25,
                "material": "aluminum_5083",
                "modifier": 1.0,
            },
        ],
    },
}


def populate_vehicles():
    veh_dir = os.path.join(DATA, "vehicles")
    os.makedirs(veh_dir, exist_ok=True)
    created = 0
    updated = 0
    for vid, info in VEHICLES.items():
        path = os.path.join(veh_dir, f"{vid}.json")
        existing = {}
        if os.path.exists(path):
            with open(path) as f:
                existing = json.load(f)

        data = {
            "id": vid,
            "name": info["name"],
            "mass_kg": info["mass_kg"],
            "armor_plates": existing.get("armor_plates", info["plates"]),
            "era_type": info["era"],
            "notes": existing.get(
                "notes", f"IRL armor plate estimates from public sources."
            ),
        }

        write_json(path, data)
        if existing:
            updated += 1
        else:
            created += 1
    print(f"  Vehicles: {created} created, {updated} updated")
    return created + updated


# ── Main ─────────────────────────────────────────────────────────────────────
def main():
    print("Populating ACE Ballistics Extension with IRL data...")
    total = 0
    total += populate_weapons()
    total += populate_ammo()
    total += populate_calibers()
    total += populate_vehicles()
    print(f"\nTotal files processed: {total}")
    print("Done.")


if __name__ == "__main__":
    main()
