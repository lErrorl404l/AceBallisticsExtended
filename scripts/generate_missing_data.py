#!/usr/bin/env python3
"""
Generate ABE JSON data files from Arma 3+RPT dump data for ALL weapons,
ammo types, and vehicles — not just ACE3-parameterized ones.

Pipeline:
  1. Parse the RPT dump (pipe-delimited lines from dump_arma_configs.sqf)
  2. Load existing data/ to determine what's missing
  3. Build parent-class inheritance tree to distinguish weapons from items
  4. Generate ammo JSONs — ACE3 ballistics data where available, estimated
     from caliber otherwise
  5. Generate weapon JSONs — ACE3 physical params where available, estimated
     from weapon type/caliber otherwise
  6. Generate vehicle JSONs for combat vehicles with armor data

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

# ── Known weapon prefixes (standard Arma 3 small arms naming) ────────────
WEAPON_PREFIXES = (
    "arifle_",
    "srifle_",
    "smg_",
    "hgun_",
    "lmg_",
    "mmg_",
    "sgun_",
    "launch_",
    "pdw_",
    "DMR_",
    "HMG_",
    "GMG_",
    "mortar_",
    "ace_csw_",
)

# ── Weapon type directories ──────────────────────────────────────────────
WEAPON_TYPE_MAP = {
    "arifle_": "rifles",
    "srifle_": "snipers",
    "lmg_": "machine_guns",
    "mmg_": "machine_guns",
    "smg_": "smgs",
    "pdw_": "smgs",
    "hgun_": "pistols",
    "pistol_": "pistols",
    "launch_": "launchers",
    "sgun_": "shotguns",
    "DMR_": "dmrs",
    "HMG_": "machine_guns",
    "GMG_": "launchers",
    "mortar_": "launchers",
    "ace_csw_": "machine_guns",
}

# ── Default barrel length by weapon type (mm) ────────────────────────────
DEFAULT_BARREL = {
    "rifles": 508.0,  # ~20"
    "snipers": 610.0,  # ~24"
    "machine_guns": 610.0,  # ~24"
    "smgs": 254.0,  # ~10"
    "pdw": 254.0,
    "pistols": 127.0,  # ~5"
    "launchers": 500.0,
    "shotguns": 508.0,  # ~20"
    "dmrs": 508.0,
}

# ── Default twist by caliber (mm) ────────────────────────────────────────
DEFAULT_TWIST = {
    5.45: 200.0,
    5.56: 178.0,  # 1:7"
    6.5: 203.0,  # 1:8"
    7.62: 240.0,  # 1:9.45"
    9.0: 254.0,  # 1:10"
    9.3: 305.0,  # 1:12"
    11.43: 406.0,  # 1:16"
    12.7: 381.0,  # 1:15"
    18.5: 760.0,  # smooth-ish
}

# ── Chamber pressure by caliber class (MPa) ──────────────────────────────
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

# ── Caliber inference from weapon/ammo class names ───────────────────────
# Maps class-name tokens → (caliber label, mm, drag model, bc, pressure)
CALIBER_MAP = {
    # 5.56mm
    "Mk20": ("5.56mm", 5.56, "g1", 0.151, 380.0),
    "TRG": ("5.56mm", 5.56, "g1", 0.151, 380.0),
    "SDAR": ("5.56mm", 5.56, "g1", 0.151, 380.0),
    "SPAR": ("5.56mm", 5.56, "g1", 0.151, 380.0),
    "MSBS": ("5.56mm", 5.56, "g1", 0.151, 380.0),
    "Mk16": ("5.56mm", 5.56, "g1", 0.151, 380.0),
    "M4A1": ("5.56mm", 5.56, "g1", 0.151, 380.0),
    "M4": ("5.56mm", 5.56, "g1", 0.151, 380.0),
    "M16": ("5.56mm", 5.56, "g1", 0.151, 380.0),
    "M249": ("5.56mm", 5.56, "g1", 0.151, 380.0),
    "Mk200": ("5.56mm", 5.56, "g1", 0.151, 380.0),
    "556": ("5.56mm", 5.56, "g1", 0.151, 380.0),
    "556x45": ("5.56mm", 5.56, "g1", 0.151, 380.0),
    "Stanag": ("5.56mm", 5.56, "g1", 0.151, 380.0),
    "Mk1": ("5.56mm", 5.56, "g1", 0.151, 380.0),
    "CAR": ("5.56mm", 5.56, "g1", 0.151, 380.0),
    "Mk1_": ("5.56mm", 5.56, "g1", 0.151, 380.0),  # CAR-95
    # 5.45mm
    "AK12": ("5.45mm", 5.45, "g7", 0.170, 355.0),
    "AK74": ("5.45mm", 5.45, "g7", 0.170, 355.0),
    "AKS74": ("5.45mm", 5.45, "g7", 0.170, 355.0),
    "545": ("5.45mm", 5.45, "g7", 0.170, 355.0),
    "545x39": ("5.45mm", 5.45, "g7", 0.170, 355.0),
    # 6.5mm
    "MX": ("6.5mm", 6.5, "g7", 0.260, 400.0),
    "Katiba": ("6.5mm", 6.5, "g7", 0.260, 400.0),
    "CAR95": ("6.5mm", 6.5, "g7", 0.260, 400.0),
    "LIM": ("6.5mm", 6.5, "g7", 0.260, 380.0),
    "65": ("6.5mm", 6.5, "g7", 0.260, 400.0),
    "65x39": ("6.5mm", 6.5, "g7", 0.260, 400.0),
    "Caseless": ("6.5mm", 6.5, "g7", 0.260, 400.0),
    # 7.62mm NATO
    "AK": ("7.62mm", 7.62, "g7", 0.200, 380.0),
    "AKM": ("7.62mm", 7.62, "g7", 0.200, 380.0),
    "AKS": ("7.62mm", 7.62, "g7", 0.200, 380.0),
    "RPK": ("7.62mm", 7.62, "g7", 0.200, 380.0),
    "Zafir": ("7.62mm", 7.62, "g7", 0.200, 380.0),
    "MG": ("7.62mm", 7.62, "g7", 0.200, 380.0),
    "Mk18": ("7.62mm", 7.62, "g7", 0.243, 380.0),
    "Mk14": ("7.62mm", 7.62, "g7", 0.243, 380.0),
    "EBR": ("7.62mm", 7.62, "g7", 0.243, 380.0),
    "DMR": ("7.62mm", 7.62, "g7", 0.243, 400.0),
    "LRR": ("7.62mm", 7.62, "g7", 0.243, 380.0),
    "M320": ("7.62mm", 7.62, "g7", 0.243, 380.0),
    "Mk1_": ("7.62mm", 7.62, "g7", 0.200, 380.0),  # EBR/M14 variants
    "762": ("7.62mm", 7.62, "g7", 0.200, 380.0),
    "762x51": ("7.62mm", 7.62, "g7", 0.200, 380.0),
    "762x54": ("7.62mm", 7.62, "g7", 0.200, 380.0),
    "762x39": ("7.62mm", 7.62, "g7", 0.200, 380.0),
    # 7.62mm x51
    "M240": ("7.62mm", 7.62, "g7", 0.200, 380.0),
    "M60": ("7.62mm", 7.62, "g7", 0.200, 380.0),
    "G3": ("7.62mm", 7.62, "g7", 0.200, 380.0),
    "FAL": ("7.62mm", 7.62, "g7", 0.200, 380.0),
    "L1A1": ("7.62mm", 7.62, "g7", 0.200, 380.0),
    "PK": ("7.62mm", 7.62, "g7", 0.200, 380.0),
    "PKM": ("7.62mm", 7.62, "g7", 0.200, 380.0),
    "DP": ("7.62mm", 7.62, "g7", 0.200, 380.0),
    "DP28": ("7.62mm", 7.62, "g7", 0.200, 380.0),
    "M1919": ("7.62mm", 7.62, "g7", 0.200, 380.0),
    "SGMT": ("7.62mm", 7.62, "g7", 0.200, 380.0),
    "SGM": ("7.62mm", 7.62, "g7", 0.200, 380.0),
    # 7.92mm / 8mm Mauser
    "792": ("7.92mm", 7.92, "g7", 0.200, 380.0),
    "792x57": ("7.92mm", 7.92, "g7", 0.200, 380.0),
    "792x33": ("7.92mm", 7.92, "g7", 0.200, 380.0),
    "8mm": ("7.92mm", 7.92, "g7", 0.200, 380.0),
    "Kar98k": ("7.92mm", 7.92, "g7", 0.200, 380.0),
    "K98k": ("7.92mm", 7.92, "g7", 0.200, 380.0),
    "MG34": ("7.92mm", 7.92, "g7", 0.200, 380.0),
    "MG42": ("7.92mm", 7.92, "g7", 0.200, 380.0),
    # 7.65mm
    "765": ("7.65mm", 7.65, "g7", 0.190, 250.0),
    "765x17": ("7.65mm", 7.65, "g7", 0.190, 250.0),
    # 7.62x25 Tokarev
    "762x25": ("7.62mm", 7.62, "g7", 0.150, 250.0),
    # .30 Carbine
    "Carbine": ("7.62mm", 7.62, "g1", 0.150, 350.0),
    "M1Carbine": ("7.62mm", 7.62, "g1", 0.150, 350.0),
    "30Carbine": ("7.62mm", 7.62, "g1", 0.150, 350.0),
    # 9mm
    "SMG": ("9mm", 9.0, "g1", 0.165, 250.0),
    "PDW": ("9mm", 9.0, "g1", 0.165, 250.0),
    "Sting": ("9mm", 9.0, "g1", 0.165, 250.0),
    "9mm": ("9mm", 9.0, "g1", 0.165, 250.0),
    "9x19": ("9mm", 9.0, "g1", 0.165, 250.0),
    "9x18": ("9mm", 9.0, "g1", 0.165, 250.0),
    "9x21": ("9mm", 9.0, "g1", 0.165, 250.0),
    "PM": ("9mm", 9.0, "g1", 0.165, 250.0),  # Makarov
    "Sten": ("9mm", 9.0, "g1", 0.165, 250.0),
    "Mat49": ("9mm", 9.0, "g1", 0.165, 250.0),
    "M3A1": ("9mm", 9.0, "g1", 0.165, 250.0),  # Grease gun
    # .45 ACP
    "Vermin": (".45 ACP", 11.43, "g1", 0.155, 230.0),
    "45ACP": (".45 ACP", 11.43, "g1", 0.155, 230.0),
    "45": (".45 ACP", 11.43, "g1", 0.155, 230.0),
    "M1911": (".45 ACP", 11.43, "g1", 0.155, 230.0),
    "M3": (".45 ACP", 11.43, "g1", 0.155, 230.0),
    # 12.7mm / .50 BMG
    "GM6": ("12.7mm", 12.7, "g7", 0.670, 400.0),
    "127": ("12.7mm", 12.7, "g7", 0.670, 400.0),
    "127x": ("12.7mm", 12.7, "g7", 0.670, 400.0),
    "127x99": ("12.7mm", 12.7, "g7", 0.670, 400.0),
    "127x108": ("12.7mm", 12.7, "g7", 0.670, 400.0),
    "HMG": ("12.7mm", 12.7, "g7", 0.670, 400.0),
    "M2": ("12.7mm", 12.7, "g7", 0.670, 400.0),
    "M2_": ("12.7mm", 12.7, "g7", 0.670, 400.0),
    "NSV": ("12.7mm", 12.7, "g7", 0.670, 400.0),
    "DShKM": ("12.7mm", 12.7, "g7", 0.670, 400.0),
    "Dshkm": ("12.7mm", 12.7, "g7", 0.670, 400.0),
    "KORD": ("12.7mm", 12.7, "g7", 0.670, 400.0),
    # .338 Lapua
    "338": ("8.6mm", 8.6, "g7", 0.300, 440.0),
    "338NM": ("8.6mm", 8.6, "g7", 0.300, 440.0),
    "M200": ("8.6mm", 8.6, "g7", 0.300, 440.0),
    # .300 Win Mag
    "300WM": ("7.62mm", 7.62, "g7", 0.243, 420.0),
    # 9.3mm
    "CYRUS": ("9.3mm", 9.3, "g7", 0.300, 440.0),
    "Navid": ("9.3mm", 9.3, "g7", 0.300, 400.0),
    "93x64": ("9.3mm", 9.3, "g7", 0.300, 440.0),
    # 12ga
    "sgun_": ("12ga", 18.5, "g1", 0.040, 70.0),
    "12ga": ("12ga", 18.5, "g1", 0.040, 70.0),
    "Gauge": ("12ga", 18.5, "g1", 0.040, 70.0),
    # .303 British
    "303": ("7.7mm", 7.7, "g7", 0.250, 380.0),
    # 7.5mm Swiss/French
    "75x54": ("7.5mm", 7.5, "g7", 0.200, 380.0),
    "75mm": ("7.5mm", 7.5, "g7", 0.200, 380.0),
    # 7.5x55 Swiss
    # 14.5mm
    "145": ("14.5mm", 14.5, "g7", 0.700, 400.0),
    "145x114": ("14.5mm", 14.5, "g7", 0.700, 400.0),
    # 20mm
    "20mm": ("20mm", 20.0, "g7", 0.800, 400.0),
    "23mm": ("23mm", 23.0, "g7", 0.850, 400.0),
    "25mm": ("25mm", 25.0, "g7", 0.900, 400.0),
    "30mm": ("30mm", 30.0, "g7", 1.000, 400.0),
    "40mm": ("40mm", 40.0, "g1", 0.000, 150.0),
    # 14.5mm
    "14_5mm": ("14.5mm", 14.5, "g7", 0.700, 400.0),
    # .50 Beowulf / 12.7x55
    "127x55": ("12.7mm", 12.7, "g7", 0.500, 350.0),
    # .50 AE
    "50AE": ("12.7mm", 12.7, "g7", 0.500, 350.0),
    # 57mm
    "57mm": ("5.7mm", 5.7, "g1", 0.130, 350.0),
    # .30-06
    "3006": ("7.62mm", 7.62, "g7", 0.250, 380.0),
    "30_06": ("7.62mm", 7.62, "g7", 0.250, 380.0),
    "M1903": ("7.62mm", 7.62, "g7", 0.250, 380.0),
    "M1_Garand": ("7.62mm", 7.62, "g7", 0.250, 380.0),
    # x54R
    "762x54R": ("7.62mm", 7.62, "g7", 0.400, 380.0),
    "54R": ("7.62mm", 7.62, "g7", 0.400, 380.0),
    # 6.8mm / .277 FURY / 6.8x51
    "68mm": ("6.8mm", 6.8, "g7", 0.270, 420.0),
    # 6mm
    "6mm": ("6.0mm", 6.0, "g7", 0.220, 380.0),
    # VN mod specific
    "M14": ("7.62mm", 7.62, "g7", 0.243, 380.0),
    "M14A1": ("7.62mm", 7.62, "g7", 0.243, 380.0),
    "M40A1": ("7.62mm", 7.62, "g7", 0.243, 380.0),
    "M1897": ("12ga", 18.5, "g1", 0.040, 70.0),
    "Type56": ("7.62mm", 7.62, "g7", 0.200, 380.0),
    "SKS": ("7.62mm", 7.62, "g7", 0.200, 380.0),
    "M38": ("7.62mm", 7.62, "g7", 0.200, 380.0),
    "M36": ("7.62mm", 7.62, "g7", 0.200, 380.0),
    "M4956": (".45 ACP", 11.43, "g1", 0.155, 230.0),
    "XM177": ("5.56mm", 5.56, "g1", 0.151, 380.0),
    "XM16E1": ("5.56mm", 5.56, "g1", 0.151, 380.0),
    "RPD": ("7.62mm", 7.62, "g7", 0.200, 380.0),
    # GM mod specific
    "SG550": ("5.56mm", 5.56, "g1", 0.151, 380.0),
    "SG551": ("5.56mm", 5.56, "g1", 0.151, 380.0),
    "MPIK": ("7.62mm", 7.62, "g7", 0.200, 380.0),
    "MPiK": ("7.62mm", 7.62, "g7", 0.200, 380.0),
    "G3A3": ("7.62mm", 7.62, "g7", 0.200, 380.0),
    "G3A4": ("7.62mm", 7.62, "g7", 0.200, 380.0),
    "G3A4A1": ("7.62mm", 7.62, "g7", 0.200, 380.0),
    "LMGM62": ("7.62mm", 7.62, "g7", 0.200, 380.0),
    "MG3": ("7.62mm", 7.62, "g7", 0.200, 380.0),
    "LS45": ("9mm", 9.0, "g1", 0.165, 250.0),
    "P1": ("9mm", 9.0, "g1", 0.165, 250.0),
    "P2": ("9mm", 9.0, "g1", 0.165, 250.0),
    "PSG1": ("7.62mm", 7.62, "g7", 0.200, 380.0),
    # SPE mod specific
    "M1_Garand": ("7.62mm", 7.62, "g7", 0.250, 380.0),
    "M1Carbine": ("7.62mm", 7.62, "g1", 0.150, 350.0),
    "M1903": ("7.62mm", 7.62, "g7", 0.250, 380.0),
    "M1_Thompson": (".45 ACP", 11.43, "g1", 0.155, 230.0),
    "Thompson": (".45 ACP", 11.43, "g1", 0.155, 230.0),
    "MP40": ("9mm", 9.0, "g1", 0.165, 250.0),
    "MP28": ("9mm", 9.0, "g1", 0.165, 250.0),
    "K98k": ("7.92mm", 7.92, "g7", 0.200, 380.0),
    "G43": ("7.92mm", 7.92, "g7", 0.200, 380.0),
    "G41": ("7.92mm", 7.92, "g7", 0.200, 380.0),
    "STG44": ("7.92mm", 7.92, "g7", 0.200, 380.0),
    "StG44": ("7.92mm", 7.92, "g7", 0.200, 380.0),
    "FG42": ("7.92mm", 7.92, "g7", 0.200, 380.0),
    "MG34": ("7.92mm", 7.92, "g7", 0.200, 380.0),
    "MG42": ("7.92mm", 7.92, "g7", 0.200, 380.0),
    "M1919": ("7.62mm", 7.62, "g7", 0.200, 380.0),
    # Launchers
    "launch_": ("40mm", 40.0, "g1", 0.000, 150.0),
    # Unknown fallback — the key match sets caliber
    "Rifle": ("7.62mm", 7.62, "g7", 0.200, 380.0),
}

# Caliber-by-diameter key for parent-child detection
CALIBER_KEYWORDS = {
    5.45: ["545", "545x39", "ak12", "ak74", "aks74"],
    5.56: [
        "556",
        "556x45",
        "m4",
        "m16",
        "m249",
        "m193",
        "ss109",
        "stanag",
        "mk16",
        "mk18",
        "mk200",
        "spar",
        "trg",
        "mk20",
    ],
    6.5: ["65", "65x39", "caseless", "mx", "katiba"],
    7.62: [
        "762",
        "762x51",
        "762x54",
        "762x39",
        "7_62",
        "ak",
        "akm",
        "aks",
        "rpk",
        "zafir",
        "ebr",
        "dmr",
        "mg",
        "pk",
        "rpd",
    ],
    9.0: ["9mm", "9x19", "9x18", "9x21", "smg", "pdw", "sting"],
    11.43: ["45acp", "45", "verm"],
    12.7: [
        "127",
        "127x99",
        "127x108",
        "hmg",
        "m2",
        "nsv",
        "kord",
        "dshk",
        "127x",
        "50bmg",
        "50",
    ],
    9.3: ["93x64", "cyrus", "navid"],
    18.5: ["12ga", "12gauge", "gauge", "sgun"],
    7.92: ["792", "792x57", "792x33", "8mm", "k98", "mg34", "mg42", "mauser"],
    7.65: ["765", "765x17"],
    14.5: ["145", "145x114", "145mm"],
    8.6: ["338", "338nm", "lapua", "m200"],
    5.7: ["57", "5_7mm", "p90", "ps90", "five-seven"],
    7.7: ["303", "british", "303british"],
    7.5: ["75", "75x54", "swiss"],
    6.8: ["68mm", "68", "6_8mm"],
    6.0: ["6mm"],
    20.0: ["20mm"],
    23.0: ["23mm"],
    25.0: ["25mm"],
    30.0: ["30mm"],
    40.0: ["40mm"],
}

# Projectile mass (g) by caliber
MASS_BY_CALIBER = {
    5.45: 3.4,
    5.56: 4.0,
    5.7: 2.0,
    6.0: 6.0,
    6.5: 8.0,
    6.8: 7.5,
    7.5: 9.5,
    7.62: 9.5,
    7.65: 6.0,
    7.7: 11.3,
    7.92: 12.8,
    8.6: 16.2,
    9.0: 8.0,
    9.3: 16.2,
    11.43: 14.9,
    12.7: 41.9,
    14.5: 60.0,
    18.5: 28.0,
    20.0: 100.0,
    23.0: 175.0,
    25.0: 230.0,
    30.0: 350.0,
    40.0: 230.0,
}

# ── Weapon root classes (inherit from "Default") ─────────────────────────
WEAPON_ROOTS = {
    "PistolCore",
    "MGunCore",
    "LauncherCore",
    "GrenadeCore",
    "CannonCore",
    "GrenadeLauncher",
}

# ── Non-weapon root classes ──────────────────────────────────────────────
NONWEAPON_ROOTS = {
    "DetectorCore",
    "Laserdesignator_mounted",
    "CarHorn",
    "Put",
    "CSLA_blackbox",
    "CSLA_sirena",
    "CSLA_UMU3",
    "gm_cbrnmarker",
    "vn_smokegen_v",
    "ace_dragon_dummyStatic",
}

# ── Known non-weapon parent classes (not in dump, set by base game) ─────
NONWEAPON_PARENTS = {
    "Binocular",
    "NVGoggles",
    "Laserdesignator",
    "Rangefinder",
    "ItemCore",
    "InventoryItem_Base_F",
    "InventoryMuzzleItem_Base_F",
    "InventoryOpticsItem_Base_F",
    "InventoryFlashLightItem_Base_F",
    "InventoryUnderItem_Base_F",
    "InventoryFirstAidKitItem_Base_F",
    "Uniform_Base",
    "Vest_Camo_Base",
    "Vest_NoCamo_Base",
    "Backpack_Base_F",
    "HelmetBase",
    "H_HelmetB",
    "H_HelmetO",
    "H_HelmetLeader",
    "H_HelmetLight",
    "MedikitItem",
    "ToolKitItem",
    "ItemMap",
    "ItemGPS",
    "ItemRadio",
    "ItemCompass",
    "ItemWatch",
    "NVGoggles_OPFOR",
    "NVGoggles_INDEP",
}

# ── Known weapon parent classes (not in dump, set by base game) ─────────
WEAPON_PARENTS = {
    "Rifle_Base_F",
    "Rifle_Long_Base_F",
    "Pistol_Base_F",
    "Launcher_Base_F",
    "SMG_Base_F",
    "Shotgun_Base_F",
    "Rifle",
    "MissileLauncher",
}

# ── Non-weapon class name patterns (items, clothing, equipment) ──────────
NONWEAPON_KEYWORDS = [
    "vest",
    "_vest",
    "Vest",
    "uniform",
    "_uniform",
    "_headgear",
    "_helmet",
    "Helmet",
    "_backpack",
    "backpack",
    "_item_",
    "_inventoryitem_",
    "_firstaid",
    "_medikit",
    "_toolkit",
    "_binoc",
    "_nvgoggle",
    "_laser",
    "_rangefinder",
    "_radio",
    "_compass",
    "_map_",
    "_gps_",
    "_watch",
    "_muzzle",
    "_bayo",
    "_bipod",
    "_sight_",
    "_scope_",
    "_optic",
    "optic_",
    "_suppressor",
    "_silencer",
    "suppressor_",
    "muzzle_snds_",
    "_camowrap",
    "_headband",
    "_bandana",
    "_beret",
    "_beanie",
    "_boonie",
    "_cap_",
    "_hat_",
    "_glasses",
    "_scarf",
    "_patch_",
    "_bag_",
    "_pack_",
    "_sog_",
    "_tunnel",
    "_flag_",
    "_bodybag",
    "_homing",
    "_cbrnmarker",
    # attachments
    "acc_",
    "acc_flashlight",
    "_flashlight",
    "_pointer_IR",
    "saber_light",
    "saber_ir",
    # vehicle-level weapon systems (not infantry small arms)
    "cannon_",
    "autocannon_",
    "gatling_",
    "missile_",
    "rocket_",
    "bomb_",
    "Bomb_",
    "Mk82",
    "GBU",
    "cmflarelauncher",
    "smokegen",
    "_detector",
    "_launcher_base",
    "_cmflare",
    "pylon_",
    "_pylons",
    "mainTurret",
    "CommanderTurret",
    "LoaderTurret",
    "SquadLeaderTurret",
    "MachineGunTurret",
    "_coax",
    "_veh",
    "_v_0",
    "_v_int",
    # fake/dummy weapons
    "FakeWeapon",
    "FakeHorn",
    "FakeDetonator",
    "FakeHeadgear",
    # ACE3 CSW proxy stances
    "_proxy",
    "ace_compat_",
    # mod-specific non-weapon prefixes
    "H_SPE_",
    "V_SPE_",
    "U_SPE_",
    "gm_headgear",
    "gm_vest",
    "gm_uniform",
    "gm_item",
    "gm_inventoryItem",
    "gm_feroz",
    "gm_zf",
    "gm_zvn",
    "gm_surefire",
    "gm_flashlight",
    "gm_hk69",
    "gm_pallad",
    "gm_bicycle",
    "vn_b_vest",
    "vn_o_vest",
    "vn_b_uniform",
    "vn_o_uniform",
    "vn_b_headgear",
    "vn_o_headgear",
    "vn_b_item",
    "vn_acc_",
    "vn_suppressor",
    "vn_s_",
    "vn_o_",
    "vn_b_bandana",
    "vn_o_bandana",
    "vn_b_beret",
    "vn_b_beanie",
    "vn_b_boonie",
    "vn_b_cap",
    "vn_b_glasses",
    "vn_b_scarf",
    "vn_b_patch",
    "vn_b_bag",
    "vn_b_pack_",
    # weapon system launchers (vehicle-level)
    "weapon_AGM_",
    "weapon_KAB",
    "weapon_mim",
    "weapon_rim",
    "weapon_s750",
    "weapon_VLS",
    "weapon_HARM",
    "weapon_SDB",
    "weapon_KH58",
    "weapon_R73",
    "weapon_R77",
    "weapon_Fighter",
    "weapon_AMRAAM",
    "weapon_BIM9",
    "weapon_GBU",
    "weapon_rim116",
    "weapon_Cannon",
    "weapon_ShipCannon",
    # vehicle launcher types
    "rockets_",
    "rocketpod",
    "bomblauncher",
    "BombLauncher",
    "MissileLauncher",
    "missile_",
    # CSLA mod
    "CSLA_FAB",
    "CSLA_OFAB",
    "CSLA_pylons",
    "CSLA_BombLauncher",
    "CSLA_RocketPods",
    "CSLA_MissileLauncher",
    "CSLA_Ch25",
    "CSLA_R60",
    "CSLA_R73",
    "CSLA_P700",
    "CSLA_UB16",
    "CSLA_UB32",
    "CSLA_B8",
    "CSLA_LRM122",
    "CSLA_S24",
    # US85 mod
    "US85_BombLauncher",
    "US85_LaserBombLauncher",
    "US85_MissileLauncher",
    "US85_RocketPods",
    "US85_pylons",
    "US85_LAU",
    "US85_TER",
    "US85_M139",
    "US85_AGM",
    "US85_GBU",
    "US85_fuelTank",
    "US85_BMG71pylons",
    "US85_Laserdesignator",
    "US85_ANPVS4",
    "US85_M260",
    "US85_M261",
    # EF mod
    "EF_Weapon_",
    "EF_Twin_HMG",
    "EF_HMG_",
    "EF_Minigun_",
    "EF_autocannon_",
    "EF_missiles_",
    "EF_mortar_",
    "EF_LMG_coax",
    "EF_gatling_",
    "EF_weapon_",
    # lxWS mod
    "lxws_zu23",
    # SPE mod
    "SPE_Bomb_",
    "SPE_Tank",
    "SPE_Cannon",
    "SPE_Mortar",
    "SPE_Pak",
    "SPE_Flak",
    "SPE_LeFH",
    "SPE_StuH",
    "SPE_KwK",
    "SPE_StuK",
    "SPE_M3_L40",
    "SPE_M1_76mm",
    "SPE_M1_HC",
    "SPE_M4_Howitzer",
    "SPE_M6_L53",
    "SPE_NbW41",
    "SPE_Tank_Flamethrower",
    "SPE_AntiAir",
    "SPE_StaticGun",
    "SPE_2xMG151",
    "SPE_8xM2",
    "SPE_PlaneMGun",
    "SPE_PlaneCannon",
    "SPE_SmokeLauncher",
    "SPE_GER_SmokeLauncher",
    "SPE_GER_MineLauncher",
    "SPE_GER_CloseDefence",
    "SPE_SC50",
    "SPE_SC250",
    "SPE_SC500",
    "SPE_NC50",
    "SPE_NC250",
    "SPE_MLMG",
    "SPE_MG34_",
    "SPE_MG42_",
    "SPE_M1919A4_tripod",
    "SPE_M1919A6",
    "SPE_FlaK",
    "SPE_M2_x4",
    "SPE_Carbine_pouch",
    "SPE_Carbine_pouch",
    "SPE_TankMGun",
    "SPE_T34_",
    "SPE_Wurfrahmen",
    "SPE_M1_81",
    "SPE_MLE_27",
    "SPE_GrW278_1",
    "SPE_G503Horn",
    "SPE_OpelBlitzHorn",
    "SPE_M2_50",
    # gm mod
    "gm_cannon_",
    "gm_autoCannon",
    "gm_mortar",
    "gm_missileLauncher",
    "gm_RocketLauncher",
    "gm_BombLauncher",
    "gm_grenadeLauncher",
    "gm_120mm_",
    "gm_105mm_",
    "gm_100mm_",
    "gm_122mm_",
    "gm_145mm_",
    "gm_155mm_",
    "gm_20mm_",
    "gm_23mm_",
    "gm_25mm_",
    "gm_35mm_",
    "gm_73mm_",
    "gm_76mm_",
    "gm_fagot_",
    "gm_hot_",
    "gm_luna_",
    "gm_maljutka_",
    "gm_mars2_",
    "gm_milan_",
    "gm_mlrs_",
    "gm_ss11_",
    "gm_platan_",
    "gm_typ1_horn",
    "gm_typ2_horn",
    "gm_u1300l_horn",
    "gm_w123_horn",
    "gm_bicycle_bell",
    "gm_cbrnmarker",
    # vn mod
    "vn_cannon_",
    "vn_autocannon_",
    "vn_baseGatling",
    "vn_rocketpod",
    "vn_bomb_",
    "vn_missile",
    "vn_rocket_",
    "vn_mortar_",
    "vn_howitzer_",
    "vn_mgun_base",
    "vn_lmg_v",
    "vn_hmg_v",
    "vn_m134_v",
    "vn_m60_v",
    "vn_m1919_v",
    "vn_rpd_v",
    "vn_dp28_v",
    "vn_pk_v",
    "vn_sgm_v",
    "vn_sgmt_v",
    "vn_m2_v",
    "vn_dshkm_v",
    "vn_zpu4",
    "vn_zgu1",
    "vn_m129",
    "vn_m75",
    "vn_m195",
    "vn_m61a1",
    "vn_ns23_v",
    "vn_mk2_v",
    "vn_mk3_v",
    "vn_m2a1_v",
    "vn_v11m_v",
    "vn_m32a1_v",
    "vn_d56_v",
    "vn_t62_v",
    "vn_m41_v",
    "vn_m10_v",
    "vn_ato54",
    "vn_type56_v",
    "vn_m39a1",
    "vn_m2_twin",
    "vn_m8c_v",
    "vn_mk18_v",
    "vn_m7_v",
    "vn_m20a1b1",
    "vn_static_m40a1rr",
    "vn_fuel_",
    "vn_sa2_",
    "vn_atgm_",
    "vn_arm_launcher",
    "vn_cmflarelauncher",
    "vn_smokegen",
    "vn_CivCar",
    "vn_CivVan",
    "vn_DirtRanger",
    "vn_pbr_horn",
    "vn_t54_horn",
    "vn_pt76_horn",
    "vn_M274_MuleHorn",
    "vn_m113_horn",
    "vn_ship_horn",
    "vn_shantou_horn",
    "vn_ptf_horn",
    "vn_bicycle_bell",
    "vn_fakeweapon",
    "vn_m249",
    "vn_m14a1",
    "vn_mk16",
    "vn_mk18",
    # ACE3 non-small-arms
    "ACE_Flashlight",
    "ACE_MRE",
    "ACE_Clacker",
    "ACE_FakePrimary",
    "ACE_dogtag",
    "ACE_LMG_",
    "ACE_HMG_",
    "ACE_cannon_",
    "ACE_gatling_",
    "ACE_mortar_",
    "ACE_hellfire_",
    "ACE_hot_",
    "ACE_maverick_",
    "ACE_kh25",
    "ACE_missile_",
    "ace_javelin",
    "ace_csw_Titan",
    "ace_csw_HMG",
    "ace_csw_GMG",
    "ACE_AIR_SAFETY",
    "ace_dragon_super",
    "ace_dragon_dummy",
    "ace_missileguidance",
    "ace_compat_gm",
    "ace_compat_sog",
    "ace_compat_spe",
    # BIS non-small-arms
    "H_FakeHeadgear",
    "DeminingDisruptor",
    "ProbingWeapon",
    "ProbingLaser",
    "AlienBeam",
    "AlienDrone",
    "GravityCannon",
    "GravityShotgun",
    "SwarmMissile",
    "CM_Universal",
    "soundset",
    "B_Patrol_",
    "B_Soldier_",
    "_Patrol_Soldier_",
    "ACE_AIR_SAFETY",
    "ACE_VMH3",
    "BombCluster",
    "BombDemine",
]

# ── Weapon-type keyword → base weapon-type for non-prefix detection ─────
WEAPON_NAME_KEYWORDS = {
    "rifle": "rifles",
    "carbine": "rifles",
    "garand": "rifles",
    "m16": "rifles",
    "m4": "rifles",
    "ak": "rifles",
    "g3": "rifles",
    "f": "rifles",  # FAL
    "g43": "rifles",
    "g41": "rifles",
    "stg44": "rifles",
    "sg550": "rifles",
    "sg551": "rifles",
    "xm177": "rifles",
    "xm16": "rifles",
    "type56": "rifles",
    "sks": "rifles",
    "l1a1": "rifles",
    # Snipers/dmrs
    "sniper": "snipers",
    "dmr": "dmrs",
    "dmr_": "dmrs",
    "ebr": "dmrs",
    "m14": "dmrs",
    "m40": "snipers",
    "psg1": "snipers",
    "g3a3": "snipers",
    "g3a4": "snipers",
    "gm6": "snipers",
    "lrr": "snipers",
    "m1903": "snipers",
    "k98": "snipers",
    "m38": "snipers",
    "mosin": "snipers",
    "svd": "snipers",
    # SMGs
    "smg_": "smgs",
    "pdw_": "smgs",
    "mp5": "smgs",
    "mp40": "smgs",
    "mp28": "smgs",
    "sten": "smgs",
    "mat49": "smgs",
    "m3a1": "smgs",
    "thompson": "smgs",
    "uzi": "smgs",
    # LMGs
    "lmg_": "machine_guns",
    "mmg_": "machine_guns",
    "mg3": "machine_guns",
    "mg34": "machine_guns",
    "mg42": "machine_guns",
    "lmgm62": "machine_guns",
    "m249": "machine_guns",
    "m60": "machine_guns",
    "mk200": "machine_guns",
    "pk": "machine_guns",
    "rpk": "machine_guns",
    "rpd": "machine_guns",
    "dp28": "machine_guns",
    "m1919": "machine_guns",
    "dshkm": "machine_guns",
    "sgm": "machine_guns",
    # Pistols
    "pistol": "pistols",
    "hgun_": "pistols",
    "ppk": "pistols",
    "m1911": "pistols",
    "p08": "pistols",
    "p38": "pistols",
    "pm": "pistols",
    # Launchers
    "launch_": "launchers",
    # Shotguns
    "shotgun": "shotguns",
    "sgun_": "shotguns",
    "m1897": "shotguns",
    # HMG
    "hmg_": "machine_guns",
    "m2": "machine_guns",
    "nsv": "machine_guns",
    "kord": "machine_guns",
    # DMR
    "dmr_": "dmrs",
}

# ── Vehicle keywords (combat vehicles only) ─────────────────────────────
VEHICLE_KEYWORDS = [
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

# ── Vehicle exclusion names (non-combat) ─────────────────────────────────
HARD_NOPE_VEHICLES = [
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
    "caragana",
    "arborescens",
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

VEHICLE_EXCLUDE = [
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


def strip_timestamp(line):
    """Remove RPT timestamp prefix (e.g. '23:01:08 ') from a log line."""
    return re.sub(r"^\d{2}:\d{2}:\d{2}\s+", "", line)


def parse_array(s):
    """Parse SQF array string like '[1,2,3]' or '[\"a\",\"b\"]' into Python list."""
    s = s.strip()
    if not s.startswith("[") or not s.endswith("]"):
        return []
    inner = s[1:-1].strip()
    if not inner:
        return []
    if '"' in inner:
        return [x.strip().strip('"') for x in inner.split(",")]
    return [float(x.strip()) for x in inner.split(",")]


def parse_dump():
    """Parse the full RPT dump into categorized dicts + parent chain."""
    weapons = {}
    ace_weapons = {}
    ammo = {}
    ace_ammo = {}
    mags = {}
    magwells = {}
    vehicles = {}
    ace_vehicles = {}
    parent_chain = {}

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
                    "class": cls,
                }
                weapons[cls] = rec
                parent_chain[cls] = parts[2]
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
                    "class": cls,
                }
                rec["ace_bc_list"] = parse_array(rec["ace_bcs"])
                rec["ace_vel_list"] = parse_array(rec["ace_velocities"])
                rec["ace_bl_list"] = parse_array(rec["ace_barrelLengths"])
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
                magwells[parts[1]] = parse_array(parts[2])

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
                    "class": cls,
                }
                vehicles[cls] = rec
                if rec["armor"] > 0 or rec["armorStructural"] > 0:
                    ace_vehicles[cls] = rec

    return (
        weapons,
        ace_weapons,
        ammo,
        ace_ammo,
        mags,
        magwells,
        vehicles,
        ace_vehicles,
        parent_chain,
    )


def get_root_class(cls, parent_chain, visited=None):
    """Walk the parent chain to find the ultimate root class."""
    if visited is None:
        visited = set()
    if cls in visited or cls == "Default":
        return cls
    visited.add(cls)
    parent = parent_chain.get(cls)
    if parent is None or parent == "Default":
        return cls
    return get_root_class(parent, parent_chain, visited)


def get_ancestors(cls, parent_chain):
    """Get the full ancestor chain including the class itself."""
    chain = [cls]
    visited = {cls}
    while cls in parent_chain:
        p = parent_chain[cls]
        if p in visited or p == "Default":
            chain.append(p)
            break
        visited.add(p)
        chain.append(p)
        cls = p
        if len(chain) > 50:
            break
    return chain


def is_weapon_class(cls, parent_chain):
    """Determine if a CfgWeapons entry is an actual weapon (not clothing/item/etc).

    Uses full parent-chain tracing + name pattern matching.
    """
    cls_lower = cls.lower()

    # Fast path: known weapon prefix
    if any(cls.startswith(p) for p in WEAPON_PREFIXES):
        return True

    # Fast path: clear non-weapon keyword in name
    for kw in NONWEAPON_KEYWORDS:
        if kw in cls_lower:
            return False

    # Trace full ancestor chain
    ancestors = get_ancestors(cls, parent_chain)

    # Check each ancestor against known weapon/non-weapon sets
    for anc in ancestors:
        if anc in WEAPON_ROOTS:
            return True
        if anc in NONWEAPON_ROOTS:
            return False
        if anc in WEAPON_PARENTS:
            return True
        if anc in NONWEAPON_PARENTS:
            return False

    # Root is the last ancestor
    root = ancestors[-1] if ancestors else cls

    # If root is unknown, use heuristic — but only apply weapon-name
    # heuristics if there's NO non-weapon keyword in the class name.
    # (Non-weapon items often have gun names in their vest/uniform names.)
    for kw in WEAPON_NAME_KEYWORDS:
        if kw in cls_lower:
            return True
    return False


def is_infantry_weapon(cls, parent_chain):
    """Check if this is an infantry small arm (not vehicle cannon/missile/rocket)."""
    cls_lower = cls.lower()

    # Known small arms prefixes
    prefix_check = any(cls.startswith(p) for p in WEAPON_PREFIXES)
    if prefix_check:
        return True

    # Vehicle weapon systems — skip these
    vehicle_weapon_kw = [
        "_veh_",
        "_v_0",
        "_coax",
        "_turret",
        "mainTurret",
        "cannon_",
        "autocannon_",
        "gatling_",
        "missile_",
        "bomb_",
        "rocket_",
        "Bomb_",
        "GBU",
        "Mk82",
        "cmflauncher",
        "smokegen",
        "pylon_",
        "_pylons",
    ]
    for kw in vehicle_weapon_kw:
        if kw in cls_lower:
            return False

    # Check parent chain for vehicle weapon patterns
    root = get_root_class(cls, parent_chain)
    # CannonCore is vehicle weapon, not infantry
    if root == "CannonCore":
        return False

    return True


def infer_caliber_from_name(class_name):
    """Infer caliber from weapon/ammo class name using CALIBER_MAP + heuristics."""
    cls_lower = class_name.lower()
    base = re.sub(r"_(\w+_F)?$", "", class_name)
    parts = set(class_name.split("_"))
    parts_lower = set(cls_lower.split("_"))

    # Direct CALIBER_MAP lookup (case-insensitive)
    for key, (cal_type, cal_mm, cdm, bc, pressure) in sorted(
        CALIBER_MAP.items(), key=lambda x: -len(x[0])
    ):
        key_lower = key.lower()
        if key in parts or key_lower in parts_lower:
            pr = CHAMBER_PRESSURE.get(cal_type, pressure)
            return cal_type, cal_mm, cdm, bc, pr

    # Prefix-based
    if "sgun_" in cls_lower or "shotgun" in cls_lower:
        return "12ga", 18.5, "g1", 0.040, 70.0
    if "hgun_" in cls_lower or "pistol_" in cls_lower:
        return "9mm", 9.0, "g1", 0.165, 250.0
    if "launch_" in cls_lower:
        return "40mm", 40.0, "g1", 0.000, 0.0

    # Try caliber keywords in class name (substring, case-insensitive)
    for cal_mm, cals in sorted(CALIBER_KEYWORDS.items(), key=lambda x: -x[0]):
        for cal_kw in cals:
            if cal_kw in cls_lower:
                bc = {
                    5.56: 0.151,
                    7.62: 0.200,
                    9.0: 0.165,
                    12.7: 0.670,
                    6.5: 0.260,
                    5.45: 0.170,
                    9.3: 0.300,
                    8.6: 0.300,
                    7.92: 0.200,
                    11.43: 0.155,
                }.get(cal_mm, 0.200)
                cal_type = mm_to_caliber_label(cal_mm)
                pr = CHAMBER_PRESSURE.get(cal_type, 380.0)
                return cal_type, cal_mm, "g7", bc, pr

    # Try extracting caliber from patterns like "762x51"
    cal_match = re.search(r"(\d+)x(\d+)", class_name)
    if cal_match:
        cal_num = float(cal_match.group(1))
        if 25 <= cal_num <= 250:
            cal_mm = round(cal_num / 100.0, 1)
            cal_type = mm_to_caliber_label(cal_mm)
            return cal_type, cal_mm, "g7", 0.200, CHAMBER_PRESSURE.get(cal_type, 380.0)

    # Extended weapon-name pattern matching (case-insensitive substring)
    WEAPON_CAL_MAP = [
        # 4.73mm
        (["g11", "g11k2"], 4.73),
        # 5.45mm
        (
            [
                "aks74",
                "mpiaks74",
                "mpiak74",
                "mpiaks74n",
                "mpiaks74k",
                "mpiaks74nk",
                "rpk74",
                "rpkn",
            ],
            5.45,
        ),
        # 5.56mm
        (
            [
                "c7",
                "c7a1",
                "c8",
                "c8a1",
                "l85",
                "l86",
                "l98",
                "l22",
                "g36",
                "g36a0",
                "g36a1",
                "g36a2",
                "g36e",
                "g36k",
                "g36c",
                "sg543",
                "sg550",
                "sg551",
                "sg552",
                "sg553",
                "xm8",
                "xm25",
                "m16a4",
                "m16a2",
                "m16a1",
                "m16a3",
                "m4a1",
                "m4",
                "m249",
                "minimi",
                "xm177",
                "xm16e1",
                "ak101",
                "ak102",
                "ctar",
                "galat",
                "xms",
                "arx",
                "viper",
                "mk1_",
                "type_115",
            ],
            5.56,
        ),
        # 6.5mm
        (["mx", "katiba", "lim", "car95", "velko"], 6.5),
        # 7.62mm
        (
            [
                "g3",
                "g3a3",
                "g3a4",
                "g3a4a1",
                "l1a1",
                "l129a1",
                "l129",
                "m14",
                "m14a1",
                "m21",
                "m39",
                "ebr",
                "mk14",
                "akm",
                "akms",
                "akmsn",
                "akmsl",
                "mpi",
                "mpik",
                "rpk",
                "rpks",
                "m60",
                "m240",
                "mg3",
                "lmgm62",
                "m1919",
                "m1903",
                "m1_garand",
                "garand",
                "fal",
                "fn_fal",
                "sgmt",
                "sgm",
                "pk",
                "pkm",
                "pkp",
                "pkt",
                "pka",
                "sd_",
                "sdar",
                "mk20",
                "spar",
                "msbs",
                "trg",
                "slr",
                "slr_",
                "m40",
                "m40a1",
                "dmr_",
                "m14_",
            ],
            7.62,
        ),
        # 7.92mm
        (
            [
                "k98k",
                "k98",
                "kar98",
                "karabiner",
                "gewehr",
                "stg44",
                "stg",
                "fg42",
                "mg34",
                "mg42",
                "mg_42",
                "g43",
                "g41",
            ],
            7.92,
        ),
        # 8.6mm / .338
        (["m200", "m2010", "m2000", "awp", "awm", "archer", "blaser", "r93"], 8.6),
        # 9mm
        (
            [
                "mp5",
                "mp5k",
                "mp5sd",
                "mp40",
                "mp28",
                "mp34",
                "sten",
                "mat49",
                "m3a1",
                "uzi",
                "mac10",
                "pp2000",
                "pp19",
                "bison",
                "vityaz",
                "sr2",
                "sr3",
                "as_val",
                "vss",
                "vsk94",
                "9a91",
                "p1",
                "p2",
                "p7",
                "p8",
                "p9",
                "p220",
                "p226",
                "p228",
                "p229",
                "p320",
                "m9",
                "m92",
                "beretta",
                "glock",
                "g17",
                "g18",
                "g19",
                "usp",
                "mark23",
                "fnx",
                "fnp",
                "ppk",
                "pm",
                "makarov",
                "mp443",
                "mp446",
                "ls45",
                "ls_45",
                "psm",
            ],
            9.0,
        ),
        # .45 ACP
        (
            [
                "m1911",
                "thompson",
                "m1_thompson",
                "m3",
                "grease",
                "verm",
                "verm",
                "uziclaw",
            ],
            11.43,
        ),
        # 12.7mm
        (
            [
                "m2",
                "browning",
                "hmg",
                "nsv",
                "kord",
                "dshk",
                "gm6",
                "lynx",
                "m107",
                "m82",
                "m95",
                "m99",
                "barrett",
                "m2carbine",
                "m2_carbine",
                "m21",
                "m20a1b1",
                "shak12",
                "ash12",
                "ash_12",
                "zu23",
                "ns23",
            ],
            12.7,
        ),
        # 9.3mm
        (["cyrus", "navid", "93x64"], 9.3),
        # 12ga
        (["m1897", "winchester", "shotgun", "sgun_"], 18.5),
        # 40mm
        (["m79", "m203", "glx", "rpg32", "nlaw", "titan", "mraws", "vorona"], 40.0),
    ]

    for tokens, cal_mm in WEAPON_CAL_MAP:
        for token in tokens:
            if token in cls_lower:
                bc = {
                    5.45: 0.170,
                    5.56: 0.151,
                    6.5: 0.260,
                    7.62: 0.200,
                    7.92: 0.200,
                    8.6: 0.300,
                    9.0: 0.165,
                    9.3: 0.300,
                    11.43: 0.155,
                    12.7: 0.670,
                    18.5: 0.040,
                }.get(cal_mm, 0.200)
                cal_type = mm_to_caliber_label(cal_mm)
                pr = CHAMBER_PRESSURE.get(cal_type, 380.0)
                return cal_type, cal_mm, "g7", bc, pr

    # Fallback: use weapon type prefix to guess default caliber
    prefix_defaults = [
        ("arifle_", 5.56),
        ("srifle_", 7.62),
        ("smg_", 9.0),
        ("pdw_", 9.0),
        ("hgun_", 9.0),
        ("pistol_", 9.0),
        ("lmg_", 7.62),
        ("mmg_", 7.62),
        ("sgm_", 7.62),
    ]
    for prefix, default_cal in prefix_defaults:
        if cls_lower.startswith(prefix):
            bc = {5.56: 0.151, 7.62: 0.200, 9.0: 0.165}.get(default_cal, 0.200)
            cal_type = mm_to_caliber_label(default_cal)
            pr = CHAMBER_PRESSURE.get(cal_type, 380.0)
            return cal_type, default_cal, "g7", bc, pr

    return None, 0.0, "g7", 0.150, 380.0


def mm_to_caliber_label(cal_mm):
    """Convert mm to caliber label string."""
    if cal_mm < 5.0:
        return "handgun"
    elif cal_mm < 5.6:
        return "5.56mm"
    elif cal_mm <= 6.0:
        return "6mm"
    elif cal_mm <= 6.8:
        return "6.5mm"
    elif cal_mm <= 7.4:
        return "7.5mm"
    elif cal_mm <= 8.0:
        return "7.62mm"
    elif cal_mm <= 8.5:
        return "8mm"
    elif cal_mm <= 9.5:
        return "9mm"
    elif cal_mm <= 10.5:
        return "10mm"
    elif cal_mm <= 11.5:
        return "handgun"
    elif cal_mm <= 14.0:
        return "12.7mm"
    elif cal_mm <= 18.0:
        return "heavy_127mm"
    elif cal_mm <= 22.0:
        return "heavy_127mm"
    else:
        return "heavy_127mm"


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
    if caliber_mm <= 18.0:
        return "heavy_127mm"
    return "heavy_127mm"


def infer_projectile_mass(caliber_mm):
    """Default mass (g) for a given caliber."""
    return MASS_BY_CALIBER.get(caliber_mm, 10.0)


def infer_twist(caliber_mm):
    """Default rifling twist for a given caliber."""
    return DEFAULT_TWIST.get(caliber_mm, 250.0)


def get_weapon_type(cls):
    """Determine weapon type directory from class name."""
    cls_lower = cls.lower()
    if cls.startswith("arifle_"):
        return "rifles"
    if cls.startswith("srifle_"):
        return "snipers"
    if cls.startswith("lmg_") or cls.startswith("mmg_"):
        return "machine_guns"
    if cls.startswith("smg_") or cls.startswith("pdw_"):
        return "smgs"
    if cls.startswith("hgun_") or cls.startswith("pistol_"):
        return "pistols"
    if cls.startswith("launch_"):
        return "launchers"
    if cls.startswith("sgun_"):
        return "shotguns"
    if cls.startswith("DMR_"):
        return "dmrs"
    if cls.startswith("HMG_") or cls.startswith("ace_csw_hmg"):
        return "machine_guns"
    if cls.startswith("GMG_") or cls.startswith("ace_csw_gmg"):
        return "launchers"
    if cls.startswith("mortar_") or cls.startswith("ace_csw_mortar"):
        return "launchers"

    # Keywords in name
    for kw, wtype in WEAPON_NAME_KEYWORDS.items():
        if kw in cls_lower:
            return wtype

    return "rifles"  # fallback


def consolidate_weapon_variants(weapons):
    """Group weapon variants by base class, return unique bases with best params."""
    bases = {}
    for cls, rec in weapons.items():
        base = re.sub(
            r"(_(black|blk|khk|tna|wdl|hex|green|coyote|sand|arid|snd|lush|olive))(_F)?$",
            "",
            cls,
        )
        base = re.sub(r"_F$", "", base)

        if base not in bases or rec.get("barrel", 0) > 0:
            bases[base] = (cls, rec)
    return bases


def load_existing_classes():
    """Load all existing ABE data class names."""
    weapon_classes = {}
    ammo_classes = {}
    for f in DATA_DIR.rglob("*.json"):
        if "schema" in str(f) or ".omo" in str(f):
            continue
        try:
            with open(f) as fh:
                d = json.load(fh)
            if "projectile" in d and "caliber_mm" in d["projectile"]:
                ammo_classes[d.get("class", "")] = f
            elif "barrel_length_mm" in d:
                weapon_classes[d.get("class", "")] = f
            elif "armor_mm_rha" in d or "turret_armor" in d:
                weapon_classes[d.get("class", "")] = f
        except (json.JSONDecodeError, KeyError):
            pass
    return weapon_classes, ammo_classes


# ── Ammo Generation ──────────────────────────────────────────────────────


def generate_ammo(dump_ace_ammo, dump_all_ammo, existing_ammo, mags):
    """Generate ammo JSONs — ACE3 ballistics + estimated for non-ACE3."""
    generated = 0
    skipped = 0

    # Build index: ammo_class -> best initSpeed from magazines
    ammo_speed = {}
    for mag_name, mag_rec in mags.items():
        ammo_cls = mag_rec["ammo_class"]
        speed = mag_rec["initSpeed"]
        if ammo_cls not in ammo_speed or speed > ammo_speed[ammo_cls]:
            ammo_speed[ammo_cls] = speed

    # Process ACE3 ammo first
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

        # Reference muzzle velocity from middle barrel length, or from mags
        ref_mv = 800.0
        if rec["ace_vel_list"] and rec["ace_bl_list"]:
            mid = len(rec["ace_vel_list"]) // 2
            if mid < len(rec["ace_vel_list"]):
                ref_mv = float(rec["ace_vel_list"][mid])
        elif cls in ammo_speed:
            ref_mv = ammo_speed[cls]

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

    # ── Filter non-bullet / non-small-arms ammo ────────────────
    # Shells, missiles, mines, bombs, grenades, flares, etc.
    AMMO_EXCLUDE_KEYWORDS = [
        "_shell_",
        "_bomb_",
        "_missile_",
        "_rocket_",
        "_warhead_",
        "_mine_",
        "_grenade_",
        "_submunition_",
        "_penetrator_",
        "_flare_",
        "_illum_",
        "_smoke_",
        "_wp_",
        "_HE_",
        "_HEAT_",
        "_HEDP_",
        "_HEAP_",
        "_cluster_",
        "_cb_",
        "_fuel_",
        "FlareCore",
        "SmokeShell",
        "ShellCore",
        "MissileCore",
        "BombCore",
        "RocketCore",
        "GrenadeCore",
        "MineCore",
        "SubmunitionCore",
        "ShotDeploy",
        "TimeBombCore",
        "DirectionalBombCore",
        "BoundingMineCore",
        "LaserBombCore",
        "ArtilleryRocketCore",
        "HelicopterExplo",
        "SmallSecondary",
        "cmflare",
        "SmokeLauncher",
    ]

    # Process non-ACE3 ammo (estimate from caliber name + defaults)
    for cls, rec in sorted(dump_all_ammo.items()):
        if cls in dump_ace_ammo:
            continue  # already handled
        if rec["ace_caliber"] > 0 or rec["ace_mass"] > 0:
            continue  # ACE3-flagged but somehow missed above

        # Skip non-small-arms ammo
        cls_lower_ammo = cls.lower()
        skip_ammo = False
        for kw in AMMO_EXCLUDE_KEYWORDS:
            if kw in cls_lower_ammo:
                skip_ammo = True
                break
        if skip_ammo:
            continue

        # Try to infer caliber from class name
        cal_type, cal_mm, cdm_id, bc_val, pressure = infer_caliber_from_name(cls)

        # Skip if we can't even determine caliber
        if cal_type is None or cal_mm <= 0:
            continue

        safe_name = cls.lower().replace(" ", "_").replace("/", "_")
        cal_dir = get_caliber_dir(cal_mm)
        cal_subdir = DATA_DIR / "ammo" / cal_dir
        cal_subdir.mkdir(parents=True, exist_ok=True)
        out_path = cal_subdir / f"{safe_name}.json"

        if cls in existing_ammo or out_path.exists():
            skipped += 1
            continue

        # Estimate mass
        mass = infer_projectile_mass(cal_mm)

        # Get muzzle velocity from magazine reference
        ref_mv = 800.0
        if cls in ammo_speed:
            ref_mv = ammo_speed[cls]

        bc_key = "bc_g7" if cdm_id == "g7" else "bc_g1"

        ammo_json = {
            "class": cls,
            "projectile": {
                "model": safe_name,
                "caliber_mm": round(cal_mm, 3),
                "mass_g": mass,
                "muzzle_velocity_ms": round(ref_mv, 1),
                bc_key: round(bc_val, 3) if bc_val else 0.0,
                "cdm_id": cdm_id,
                "source": {
                    "type": "inferred",
                    "reference": f"Arma 3 CfgAmmo::{cls}",
                    "methodology": f"Caliber ({cal_type}) inferred from class name. Mass ({mass}g) estimated for caliber. Muzzle velocity ({ref_mv:.0f}m/s) from magazine initSpeed.",
                    "confidence": "low",
                },
            },
            "chamber_pressure_mpa": 0,
            "notes": f"Inferred caliber={cal_type}, mass={mass}g, BC={bc_val}, drag={cdm_id}. Source: estimated from game config.",
        }

        with open(out_path, "w") as f:
            json.dump(ammo_json, f, indent=2)
        generated += 1
        print(
            f"  [+{generated:3d}] {cls} ({cal_type}, ~{mass:.1f}g, ~{ref_mv:.0f}m/s -> {cal_dir}/)"
        )

    return generated, skipped


# ── Weapon Generation ────────────────────────────────────────────────────


def generate_weapons(
    dump_all_weapons, dump_ace_weapons, existing_weapons, parent_chain
):
    """Generate weapon JSONs — ACE3 params + estimated for non-ACE3."""
    generated = 0
    skipped = 0
    no_caliber = 0

    # Identify candidate weapons (actual small arms, not items/vehicle weapons)
    candidates = {}
    for cls, rec in dump_all_weapons.items():
        if not is_weapon_class(cls, parent_chain):
            continue
        if not is_infantry_weapon(cls, parent_chain):
            continue
        # Skip carry/tripod items
        cls_lower = cls.lower()
        if "carry" in cls_lower or "tripod" in cls_lower:
            continue
        candidates[cls] = rec

    # Consolidate variants
    bases = consolidate_weapon_variants(candidates)

    for base_name, (orig_cls, rec) in sorted(bases.items()):
        if orig_cls in existing_weapons:
            skipped += 1
            continue

        # Skip non-small-arm: barrel=30 (carry tripod), barrel=1 (dummy)
        if rec["barrel"] < 50 and rec["barrel"] > 0:
            skipped += 1
            continue

        # Infer caliber
        cal_type, cal_mm, cdm_id, bc, pressure = infer_caliber_from_name(orig_cls)
        if cal_type is None or cal_mm <= 0:
            no_caliber += 1
            continue

        # Determine weapon type directory
        wpn_dir = get_weapon_type(orig_cls)
        out_dir = DATA_DIR / "weapons" / wpn_dir
        out_dir.mkdir(parents=True, exist_ok=True)
        safe_name = orig_cls.lower()
        out_path = out_dir / f"{safe_name}.json"

        if out_path.exists():
            skipped += 1
            continue

        # Barrel length: ACE3 value or estimated
        barrel = (
            rec["barrel"] if rec["barrel"] > 0 else DEFAULT_BARREL.get(wpn_dir, 508.0)
        )
        # Twist: ACE3 value or estimated from caliber
        twist = rec["twist"] if rec["twist"] > 0 else infer_twist(cal_mm)

        # Muzzle velocity: from initSpeed or default
        init_speed = rec["initSpeed"]
        muzzle_vel = abs(init_speed) * 1000 if init_speed != 0 else 800.0
        # initSpeed in Arma is often in m/s or -m/s (negative = actual speed)
        if init_speed < 0:
            muzzle_vel = abs(init_speed)
        elif init_speed == 0:
            muzzle_vel = 800.0
        else:
            muzzle_vel = init_speed

        # Determine source confidence
        if rec["barrel"] > 0:
            source_type = "ACE3"
            confidence = "medium"
            methodology = f"Barrel/twist from ACE3. Caliber inferred from class naming ({cal_type})."
        else:
            source_type = "estimated"
            confidence = "low"
            methodology = f"Estimated barrel ({barrel:.0f}mm) by weapon type ({wpn_dir}), twist ({twist:.0f}mm) by caliber ({cal_type})."

        weapon_json = {
            "class": orig_cls,
            "caliber_mm": cal_mm,
            "barrel_length_mm": barrel,
            "rifling_twist_mm": twist,
            "chamber_pressure_mpa": pressure,
            "cdm_id": cdm_id,
            "projectile_mass_g": infer_projectile_mass(cal_mm),
            "muzzle_velocity_ms": muzzle_vel,
            "zero_range_m": 300 if wpn_dir in ("rifles", "snipers", "dmrs") else 100,
            "source": {
                "type": source_type,
                "reference": f"Arma 3 CfgWeapons::{orig_cls}",
                "methodology": methodology,
                "confidence": confidence,
            },
            "notes": f"Generated from Arma 3 config dump. Caliber: {cal_type}. Barrel: {barrel:.0f}mm. Twist: {twist:.0f}mm.",
        }

        with open(out_path, "w") as f:
            json.dump(weapon_json, f, indent=2)
        generated += 1
        print(
            f"  [+{generated:3d}] {orig_cls} ({cal_type}, {barrel:.0f}mm barrel{' [ACE3]' if rec['barrel'] > 0 else ' [EST]'})"
        )

    return generated, skipped, no_caliber


# ── Vehicle Generation ───────────────────────────────────────────────────


def is_combat_vehicle(cls, rec):
    """Whitelist: only generate real combat vehicles."""
    cls_lower = cls.lower()

    for pat in HARD_NOPE_VEHICLES:
        if pat in cls_lower:
            return False

    if rec["armor"] >= 10000 or rec["armorStructural"] >= 10000:
        return False
    if rec["armor"] < 20 and rec["armorStructural"] < 20:
        return False

    return any(kw in cls_lower for kw in VEHICLE_KEYWORDS)


def generate_vehicles(dump_vehicles, existing_weapons):
    """Generate vehicle JSONs for combat vehicles with armor data."""
    generated = 0
    skipped = 0
    out_dir = DATA_DIR / "vehicles"
    out_dir.mkdir(parents=True, exist_ok=True)

    for cls, rec in sorted(dump_vehicles.items()):
        if cls in VEHICLE_EXCLUDE or rec["parent"] in VEHICLE_EXCLUDE:
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
                "methodology": "Armor values from game config.",
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


# ── Main ─────────────────────────────────────────────────────────────────


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
        parent_chain,
    ) = parse_dump()
    print(f"  Weapons: {len(dp_weapons)} ({len(dp_ace_wpn)} with ACE3 params)")
    print(f"  Ammo: {len(dp_ammo)} ({len(dp_ace_ammo)} with ACE3 ballistics)")
    print(f"  Vehicles: {len(dp_veh)} ({len(dp_ace_veh)} with armor)")
    print(f"  Magazines: {len(dp_mags)}, Magwells: {len(dp_magwells)}")

    # Count small-arms weapons using classifier
    small_arms = [
        cls
        for cls in dp_weapons
        if is_weapon_class(cls, parent_chain) and is_infantry_weapon(cls, parent_chain)
    ]
    print(f"  Small arms identified: {len(small_arms)}")

    print("\n" + "=" * 60)
    print("GENERATING AMMO JSONS")
    print("=" * 60)
    ammo_gen, ammo_skip = generate_ammo(dp_ace_ammo, dp_ammo, existing_ammo, dp_mags)
    print(f"  Generated: {ammo_gen}, Skipped: {ammo_skip}")

    print("\n" + "=" * 60)
    print("GENERATING WEAPON JSONS")
    print("=" * 60)
    wpn_gen, wpn_skip, wpn_nocal = generate_weapons(
        dp_weapons, dp_ace_wpn, existing_weapons, parent_chain
    )
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
