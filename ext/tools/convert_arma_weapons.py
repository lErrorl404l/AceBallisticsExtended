#!/usr/bin/env python3
"""
Arma 3 CfgWeapons/Ammo → ABE JSON config converter.

Reads built-in reference tables of Arma 3 weapon and ammo data (derived from
public configs, community wikis, and real-world ballistic references) and
outputs ABE-compatible JSON files matching the Rust WeaponConfig / AmmoConfig
structs in ext/src/config.rs.

Usage:
    python convert_arma_weapons.py

Overrides (optional):
    --weapons-dir <path>    Output directory for weapon JSONs (default: ../../data/weapons)
    --ammo-dir    <path>    Output directory for ammo JSONs    (default: ../../data/ammo)
    --force                  Overwrite existing files (default: skip)

ACE3 / RHS extension:
    Add entries to REFERENCE_WEAPONS / REFERENCE_AMMO below. Follow the same
    field layout. Filenames are derived from the class name; override with a
    custom key in the data dict (key "filename").
"""

from __future__ import annotations

import json
import os
import sys
import argparse
from typing import Any

# ── Reference data ──────────────────────────────────────────────────────────
# Sources: Bohemia Interactive Arma 3 config dumps, Community Wiki,
#          Applied Ballistics (Litz), SAAMI / CIP pressure specs,
#          manufacturer-published muzzle velocities.

# Weapon field layout  (maps to Rust WeaponConfig):
#   class               str   Arma 3 CfgWeapons class name
#   caliber_mm          float Bullet diameter
#   barrel_length_mm    float Barrel length
#   rifling_twist_mm    float Twist rate in mm / revolution
#   chamber_pressure_mpa float SAAMI or CIP max pressure
#   cdm_id              str   Drag-model curve ID (g1, g7, …)
#   projectile_mass_g   float Typical projectile mass for zeroing
#   muzzle_velocity_ms  float Typical MV from this barrel length
#   zero_range_m        float Zero distance in metres (default 100)
#   notes               str   Free-text (not serialised, kept for reference)

REFERENCE_WEAPONS: dict[str, dict[str, Any]] = {
    # ── MX series (6.5 mm caseless — modelled on 6.5 Creedmoor ballistics) ──
    "arifle_MX_Base_F": {
        "caliber_mm": 6.5,
        "barrel_length_mm": 508.0,  # 20"
        "rifling_twist_mm": 254.0,  # 1:10" — common 6.5mm twist
        "chamber_pressure_mpa": 345.0,  # CIP 6.5 Creedmoor
        "cdm_id": "g7",
        "projectile_mass_g": 7.5,  # ≈115 gr
        "muzzle_velocity_ms": 820.0,  # 20" barrel, 115 gr @ ~2700 fps
        "zero_range_m": 100.0,
        "notes": 'MX 6.5mm assault rifle, 20" barrel, modelled on 6.5 Creedmoor',
        "filename": "mx_6_5mm.json",
    },
    "arifle_MXC_Base_F": {
        "caliber_mm": 6.5,
        "barrel_length_mm": 318.0,  # 12.5"
        "rifling_twist_mm": 254.0,
        "chamber_pressure_mpa": 345.0,
        "cdm_id": "g7",
        "projectile_mass_g": 7.5,
        "muzzle_velocity_ms": 760.0,  # shorter barrel ~ −25 fps / inch
        "zero_range_m": 100.0,
        "notes": 'MXC 6.5mm carbine, 12.5" barrel',
        "filename": "mxc_6_5mm.json",
    },
    "arifle_Katiba_Base_F": {
        "caliber_mm": 6.5,
        "barrel_length_mm": 508.0,
        "rifling_twist_mm": 254.0,
        "chamber_pressure_mpa": 345.0,
        "cdm_id": "g7",
        "projectile_mass_g": 7.5,
        "muzzle_velocity_ms": 820.0,
        "zero_range_m": 100.0,
        "notes": 'Katiba 6.5mm bullpup assault rifle, 20" barrel',
        "filename": "katiba_6_5mm.json",
    },
    # ── 5.56 mm NATO rifles ──
    "arifle_TRG21_base_F": {
        "caliber_mm": 5.56,
        "barrel_length_mm": 420.0,  # ≈16.5"
        "rifling_twist_mm": 178.0,  # 1:7"
        "chamber_pressure_mpa": 380.0,  # SAAMI 5.56 NATO
        "cdm_id": "g7",
        "projectile_mass_g": 4.0,  # 62 gr M855
        "muzzle_velocity_ms": 920.0,  # 16.5" barrel, M855
        "zero_range_m": 100.0,
        "notes": 'TRG-21 5.56mm bullpup assault rifle, 16.5" barrel',
        "filename": "trg21_5_56mm.json",
    },
    "arifle_Mk20_Base_F": {
        "caliber_mm": 5.56,
        "barrel_length_mm": 368.0,  # ≈14.5"
        "rifling_twist_mm": 178.0,
        "chamber_pressure_mpa": 380.0,
        "cdm_id": "g7",
        "projectile_mass_g": 4.0,
        "muzzle_velocity_ms": 900.0,  # slightly shorter barrel
        "zero_range_m": 100.0,
        "notes": 'Mk20 5.56mm assault rifle, 14.5" barrel',
        "filename": "mk20_5_56mm.json",
    },
    # ── SMG ──
    "SMG_01_Base_F": {
        "caliber_mm": 11.43,  # .45 ACP
        "barrel_length_mm": 200.0,  # ≈7.9"
        "rifling_twist_mm": 406.0,  # 1:16" common for .45 ACP
        "chamber_pressure_mpa": 130.0,  # SAAMI .45 ACP
        "cdm_id": "g1",
        "projectile_mass_g": 15.0,  # 230 gr
        "muzzle_velocity_ms": 280.0,  # typical from short SMG barrel
        "zero_range_m": 100.0,
        "notes": 'Vermin SMG .45 ACP, 7.9" barrel. G1 drag model for pistol bullet.',
        "filename": "vermin_45_acp.json",
    },
    # ── LMG ──
    "LMG_Mk200_base_F": {
        "caliber_mm": 6.5,
        "barrel_length_mm": 508.0,
        "rifling_twist_mm": 254.0,
        "chamber_pressure_mpa": 345.0,
        "cdm_id": "g7",
        "projectile_mass_g": 7.5,
        "muzzle_velocity_ms": 820.0,
        "zero_range_m": 100.0,
        "notes": 'Mk200 6.5mm LMG, 20" barrel, same CT round as MX',
        "filename": "mk200_6_5mm.json",
    },
    "LMG_Zafir_Base_F": {
        "caliber_mm": 7.62,
        "barrel_length_mm": 550.0,  # ≈21.7"
        "rifling_twist_mm": 305.0,  # 1:12" — 7.62 NATO
        "chamber_pressure_mpa": 345.0,  # CIP 7.62×51
        "cdm_id": "g7",
        "projectile_mass_g": 9.5,  # 147 gr
        "muzzle_velocity_ms": 840.0,  # 21.7" barrel
        "zero_range_m": 100.0,
        "notes": 'Zafir 7.62mm LMG, 21.7" barrel, 7.62×51 NATO',
        "filename": "zafir_7_62mm.json",
    },
    # ── Sniper rifles ──
    "srifle_LRR_base_F": {
        "caliber_mm": 10.36,  # .408 CheyTac
        "barrel_length_mm": 690.0,  # ≈27"
        "rifling_twist_mm": 330.0,  # 1:13" — CheyTac std
        "chamber_pressure_mpa": 380.0,  # CIP
        "cdm_id": "g7",
        "projectile_mass_g": 27.0,  # ≈416 gr
        "muzzle_velocity_ms": 830.0,  # 27" barrel
        "zero_range_m": 100.0,
        "notes": 'LRR .408 CheyTac sniper rifle, 27" barrel',
        "filename": "lrr_408.json",
    },
    "srifle_GM6_base_F": {
        "caliber_mm": 12.7,  # .50 BMG
        "barrel_length_mm": 737.0,  # ≈29"
        "rifling_twist_mm": 381.0,  # 1:15"
        "chamber_pressure_mpa": 380.0,  # CIP .50 BMG
        "cdm_id": "g7",
        "projectile_mass_g": 42.0,  # 650 gr
        "muzzle_velocity_ms": 860.0,  # 29" barrel
        "zero_range_m": 100.0,
        "notes": 'GM6 .50 BMG sniper rifle, 29" barrel',
        "filename": "gm6_50_bmg.json",
    },
    # ── Handguns ──
    "hgun_P07_base_F": {
        "caliber_mm": 9.0,
        "barrel_length_mm": 97.0,  # ≈3.8"
        "rifling_twist_mm": 254.0,  # 1:10"
        "chamber_pressure_mpa": 235.0,  # SAAMI 9 mm Para
        "cdm_id": "g1",
        "projectile_mass_g": 8.0,  # 124 gr
        "muzzle_velocity_ms": 350.0,  # compact barrel
        "zero_range_m": 25.0,  # handgun typical zero
        "notes": 'P07 9mm pistol, 3.8" barrel. G1 drag model.',
        "filename": "p07_9mm.json",
    },
    "hgun_Rook40_base_F": {
        "caliber_mm": 9.0,
        "barrel_length_mm": 120.0,  # ≈4.7"
        "rifling_twist_mm": 254.0,
        "chamber_pressure_mpa": 235.0,
        "cdm_id": "g1",
        "projectile_mass_g": 8.0,
        "muzzle_velocity_ms": 370.0,  # slightly longer barrel
        "zero_range_m": 25.0,
        "notes": 'Rook 9mm pistol, 4.7" barrel. G1 drag model.',
        "filename": "rook40_9mm.json",
    },
    # ── RHS USAF weapons ──
    # RHS M4A1 — 14.5" barrel, M855A1/M855 ballistics
    "rhs_weap_m4a1": {
        "caliber_mm": 5.56,
        "barrel_length_mm": 368.0,  # 14.5"
        "rifling_twist_mm": 178.0,  # 1:7"
        "chamber_pressure_mpa": 380.0,  # SAAMI 5.56 NATO
        "cdm_id": "g7",
        "projectile_mass_g": 4.0,  # 62 gr
        "muzzle_velocity_ms": 948.0,  # 14.5" barrel, M855A1
        "zero_range_m": 100.0,
        "notes": 'RHS M4A1, 14.5" barrel, 1:7" twist. M855A1 pressure.',
    },
    # RHS M16A4 — 20" barrel, M855 ballistics
    "rhs_weap_m16a4": {
        "caliber_mm": 5.56,
        "barrel_length_mm": 508.0,  # 20"
        "rifling_twist_mm": 178.0,  # 1:7"
        "chamber_pressure_mpa": 380.0,
        "cdm_id": "g7",
        "projectile_mass_g": 4.0,
        "muzzle_velocity_ms": 930.0,  # 20" barrel, M855
        "zero_range_m": 100.0,
        "notes": 'RHS M16A4, 20" barrel, 1:7" twist.',
    },
    # RHS Mk18 Mod 0 — 10.3" CQB barrel, M855 ballistics
    "rhs_weap_mk18": {
        "caliber_mm": 5.56,
        "barrel_length_mm": 262.0,  # 10.3"
        "rifling_twist_mm": 178.0,  # 1:7"
        "chamber_pressure_mpa": 380.0,
        "cdm_id": "g7",
        "projectile_mass_g": 4.0,
        "muzzle_velocity_ms": 810.0,  # 10.3" barrel, M855
        "zero_range_m": 100.0,
        "notes": 'RHS Mk18 Mod 0, 10.3" barrel, 1:7" twist.',
    },
    # RHS M249 SAW — 18.3" barrel, M855 ballistics
    "rhs_weap_m249": {
        "caliber_mm": 5.56,
        "barrel_length_mm": 465.0,  # 18.3"
        "rifling_twist_mm": 178.0,  # 1:7"
        "chamber_pressure_mpa": 380.0,
        "cdm_id": "g7",
        "projectile_mass_g": 4.0,
        "muzzle_velocity_ms": 915.0,  # 18.3" barrel, M855
        "zero_range_m": 100.0,
        "notes": 'RHS M249 SAW, 18.3" barrel, 1:7" twist.',
    },
    # RHS M240G — 7.62×51 GPMG, 24.8" barrel
    "rhs_weap_m240g": {
        "caliber_mm": 7.62,
        "barrel_length_mm": 630.0,  # 24.8"
        "rifling_twist_mm": 305.0,  # 1:12"
        "chamber_pressure_mpa": 360.0,  # CIP 7.62 NATO
        "cdm_id": "g7",
        "projectile_mass_g": 9.5,  # 147 gr
        "muzzle_velocity_ms": 853.0,  # 24.8" barrel, M80
        "zero_range_m": 100.0,
        "notes": 'RHS M240G, 24.8" barrel, 1:12" twist, 7.62×51.',
    },
    # RHS SR-25/M110 — 7.62×51 DMR, 20" barrel
    "rhs_weap_sr25": {
        "caliber_mm": 7.62,
        "barrel_length_mm": 508.0,  # 20"
        "rifling_twist_mm": 284.0,  # 1:11.2"
        "chamber_pressure_mpa": 360.0,
        "cdm_id": "g7",
        "projectile_mass_g": 9.5,  # 147 gr
        "muzzle_velocity_ms": 830.0,  # 20" barrel, M80
        "zero_range_m": 100.0,
        "notes": 'RHS SR-25/M110 DMR, 20" barrel, 1:11.2" twist.',
    },
    # RHS M14 EBR — 7.62×51 battle rifle, 22" barrel
    "rhs_weap_m14ebrri": {
        "caliber_mm": 7.62,
        "barrel_length_mm": 559.0,  # 22"
        "rifling_twist_mm": 286.0,  # 1:11.25"
        "chamber_pressure_mpa": 360.0,
        "cdm_id": "g7",
        "projectile_mass_g": 9.5,  # 147 gr
        "muzzle_velocity_ms": 850.0,  # 22" barrel, M80
        "zero_range_m": 100.0,
        "notes": 'RHS M14 EBR (Mk14 Mod 1), 22" barrel, 1:11.25" twist.',
    },
    # RHS SVD Dragunov — 7.62×54R DMR, 24.4" barrel
    "rhs_weap_svdp": {
        "caliber_mm": 7.62,
        "barrel_length_mm": 620.0,  # 24.4"
        "rifling_twist_mm": 320.0,  # 1:12.6"
        "chamber_pressure_mpa": 360.0,  # CIP 7.62×54R
        "cdm_id": "g7",
        "projectile_mass_g": 9.7,  # 150 gr 7N1
        "muzzle_velocity_ms": 830.0,  # 24.4" barrel, 7N1
        "zero_range_m": 100.0,
        "notes": 'RHS SVD Dragunov, 24.4" barrel, 1:12.6" twist, 7.62×54R 7N1.',
    },
    # RHS M240B — 7.62×51 GPMG, 21" barrel
    "rhs_weap_m240B": {
        "caliber_mm": 7.62,
        "barrel_length_mm": 533.0,  # 21"
        "rifling_twist_mm": 305.0,  # 1:12"
        "chamber_pressure_mpa": 360.0,
        "cdm_id": "g7",
        "projectile_mass_g": 9.5,
        "muzzle_velocity_ms": 840.0,  # 21" barrel, M80
        "zero_range_m": 100.0,
        "notes": 'RHS M240B, 21" barrel, 1:12" twist, 7.62×51.',
    },
}

# ── Ammo reference data ─────────────────────────────────────────────────────
# BC SOURCES (verified published references):
# ──────────────────────────────────────────────
# M855 (5.56×45, 62gr):
#   APG/US Army BRL G7 BC = 0.151 (ARL-TR-5182, Silton & Howell, 2010)
#   SS109 variant: APG G7 BC = 0.158 (same bullet, different proof lot)
#   Current file: m855.json → bc_g7 = 0.151
#
# M855A1 (5.56×45, 62gr enhanced):
#   No USG-published G7 BC. Litz-derived estimate: G1~0.307 → G7~0.155.
#   Enhanced steel-core construction, slightly higher BC than M855.
#   Current file: 556x45mm.json → bcG7 = 0.155
#
# M80 (7.62×51, 149gr):
#   APG/US Army BRL G7 BC = 0.200 (AD0815788, Piddington & Maynard, 1966)
#   Confirmed by ShootersCalculator.com and Wikipedia references.
#   Current files: m80.json → bc_g7 = 0.200; 762x51mm_m80.json → bcG7 = 0.200
#
# 7N6 (5.45×39, 53gr):
#   US Army BRL G7 BC = 0.168, form factor = 0.929 (15th Intl Symposium on Ballistics)
#   Wikipedia / Alchetron confirm: "BRL measured a BC (G7 BC) of 0.168"
#   Current file: 545x39mm.json → bcG7 = 0.168
#
# 7N1 (7.62×54R, 152gr):
#   Hornady Doppler radar test: G7 BC = 0.216 (Firearms News, Fortier, 2021)
#   US Army "G7 BC of approximately 0.206" per Wikipedia. 0.216 used as measured value.
#   Current file: 762x54r.json → bcG7 = 0.216
#
# 6.5mm CT (fictional, ≈6.5 Grendel/6.5 Creedmoor, 115gr):
#   No published BC for a fictional round. Conservative estimate based on:
#   - Sierra 123gr MK G7 = 0.230 (Litz)
#   - Lapua 123gr Scenar G7 = 0.254
#   - Lapua 139gr Scenar G7 = 0.290
#   Selected G7 = 0.260 for a modern 115gr boat-tail FMJ (conservative).
#   Current file: 65x39_caseless.json → bc_g7 = 0.260
#
# .408 CheyTac (419gr solid):
#   Manufacturer G1 = 0.945-0.949 (Jamison/CheyTac). G7 conversion ≈ G1 × 0.40 = 0.378
#   Litz form factor for VLD solid ≈ 0.95. Conservative G7 = 0.378.
#   Current file: 408_cheytac.json → bc_g7 = 0.378
#
# .50 BMG M33 (650gr):
#   APG G7 BC = 0.340 (confirmed M14Forum, multiple references).
#   Current file: 127x108_bmg.json → bc_g7 = 0.340
#
# 9mm 124gr FMJ / .45 ACP 230gr FMJ:
#   Handgun bullets use G1 drag model. G1-G7 conversion ≈ ×0.5.
#   G1 BC values: 9mm≈0.150, .45ACP≈0.170 → G7≈0.075, 0.085 respectively.
#   Current files: 9x21mm.json → bc_g7 = 0.075 (G1); 45acp.json → bc_g7 = 0.085 (G1)
# ──────────────────────────────────────────────
# Field layout (maps to Rust AmmoConfig → ProjectileConfig):
#   class        str   Arma 3 CfgAmmo class name
#   model        str   Short projectile model identifier
#   mass_g       float Projectile mass
#   caliber_mm   float Calibre
#   bc_g7        float Ballistic coefficient (G7 model)
#   cdm_id       str   Drag-model curve ID
#   frag         dict  Optional fragmentation config (None = no frag):
#       threshold_vel_ms  float
#       avg_fragments     int
#       mass_distribution str
#       params            dict[str, float]
#   notes        str   Reference notes (not serialised)

REFERENCE_AMMO: dict[str, dict[str, Any]] = {
    "B_65x39_Caseless": {
        "model": "6.5mm_CT",
        "mass_g": 7.5,
        "caliber_mm": 6.5,
        "bc_g7": 0.250,
        "cdm_id": "g7",
        "frag": None,
        "notes": "6.5×39 mm caseless CT, ≈115 gr FMJ. Fictional round modelled on 6.5 Creedmoor ballistics. G7 BC from Litz equivalent.",
        "filename": "65x39_caseless.json",
    },
    "B_65x39_Caseless_ms": {
        "model": "6.5mm_CT_Tracer",
        "mass_g": 7.0,
        "caliber_mm": 6.5,
        "bc_g7": 0.240,
        "cdm_id": "g7",
        "frag": None,
        "notes": "6.5×39 mm tracer. Slightly lighter projectile, marginally lower BC.",
        "filename": "65x39_tracer.json",
    },
    "B_408_Ball": {
        "model": "408CT",
        "mass_g": 27.0,  # ≈416 gr
        "caliber_mm": 10.36,  # .408
        "bc_g7": 0.370,
        "cdm_id": "g7",
        "frag": None,
        "notes": ".408 CheyTac ball, 416 gr solid. No fragmentation (solid copper alloy). BC from conservative real-world measurements.",
        "filename": "408_cheytac.json",
    },
    "B_127x108_Ball": {
        "model": "M33",
        "mass_g": 42.0,  # 650 gr
        "caliber_mm": 12.7,  # .50 BMG
        "bc_g7": 0.340,
        "cdm_id": "g7",
        "frag": {
            "threshold_vel_ms": 700.0,
            "avg_fragments": 15,
            "mass_distribution": "log_normal",
            "params": {"mean": 0.15, "std": 0.08},
        },
        "notes": ".50 BMG M33 ball, 650 gr FMJ, APG G7 BC=0.340. Fragments above ~2300 fps.",
        "filename": "127x108_bmg.json",
    },
    "B_9x21_Ball": {
        "model": "FMJ_124gr",
        "mass_g": 8.0,  # 124 gr
        "caliber_mm": 9.0,
        "bc_g7": 0.075,  # G1≈0.150 → G7≈0.075 for handgun FMJ
        "cdm_id": "g1",
        "frag": None,
        "notes": "9×21 mm (9 mm Para equivalent), 124 gr FMJ. G1 drag model. No fragmentation.",
        "filename": "9x21mm.json",
    },
    "B_45ACP_Ball": {
        "model": "FMJ_230gr",
        "mass_g": 15.0,  # 230 gr
        "caliber_mm": 11.43,  # .45
        "bc_g7": 0.085,  # G1≈0.170 → G7≈0.085
        "cdm_id": "g1",
        "frag": None,
        "notes": ".45 ACP ball, 230 gr FMJ. G1 drag model. No fragmentation.",
        "filename": "45acp.json",
    },
    # Placeholder for ACE3 / RHS ammunition:
    # "rhs_mag_30Rnd_556x45_M855_Stanag": {
    #     "model": "m855",
    #     ...
    # },
}


# ── Helpers ──────────────────────────────────────────────────────────────────


def default_filename(class_name: str) -> str:
    """Derive a filename from an Arma 3 class name."""
    name = class_name.replace("_Base_F", "").replace("_base_F", "")
    name = name.lower().strip("_")
    # Drop common Infantry/DLC prefixes for brevity
    for prefix in ("arifle_", "hgun_", "srifle_", "smg_", "lmg_", "b_"):
        if name.startswith(prefix):
            name = name[len(prefix) :]
            break
    return name.replace("__", "_").strip("_") + ".json"


def generate_weapon_json(class_name: str, data: dict[str, Any]) -> dict[str, Any]:
    """Build a WeaponConfig-compatible JSON dict (snake_case, Rust struct)."""
    return {
        "class": class_name,
        "caliber_mm": data["caliber_mm"],
        "barrel_length_mm": data["barrel_length_mm"],
        "rifling_twist_mm": data.get("rifling_twist_mm", 0.0),
        "chamber_pressure_mpa": data["chamber_pressure_mpa"],
        "cdm_id": data.get("cdm_id", "g7"),
        "projectile_mass_g": data["projectile_mass_g"],
        "muzzle_velocity_ms": data.get("muzzle_velocity_ms", 0.0),
        "zero_range_m": data.get("zero_range_m", 100.0),
    }


def generate_ammo_json(class_name: str, data: dict[str, Any]) -> dict[str, Any]:
    """Build an AmmoConfig-compatible JSON dict (snake_case, Rust struct)."""
    proj: dict[str, Any] = {
        "model": data["model"],
        "mass_g": data["mass_g"],
        "caliber_mm": data["caliber_mm"],
        "bc_g7": data["bc_g7"],
        "cdm_id": data.get("cdm_id", "g7"),
    }

    frag = data.get("frag")
    if frag is not None:
        proj["fragmentation"] = {
            "threshold_vel_ms": frag["threshold_vel_ms"],
            "avg_fragments": frag["avg_fragments"],
            "mass_distribution": frag["mass_distribution"],
            "params": frag["params"],
        }

    return {"class": class_name, "projectile": proj}


def collect_existing_classes(directory: str) -> set[str]:
    """Scan JSON files in *directory* and return every class name found under
    any of the keys 'class', 'weaponClass', or 'ammoClass'."""
    classes: set[str] = set()
    try:
        for entry in os.scandir(directory):
            if not entry.name.endswith(".json") or not entry.is_file():
                continue
            try:
                with open(entry.path) as f:
                    obj = json.load(f)
                for key in ("class", "weaponClass", "ammoClass"):
                    val = obj.get(key)
                    if isinstance(val, str):
                        classes.add(val)
            except (json.JSONDecodeError, OSError):
                continue
    except FileNotFoundError:
        pass
    return classes


def write_jsons(
    output_dir: str,
    reference: dict[str, dict[str, Any]],
    generator,
    kind: str,
    force: bool = False,
) -> int:
    """Generate JSON files from *reference* into *output_dir*.

    Returns the number of files written.
    """
    os.makedirs(output_dir, exist_ok=True)

    existing_classes = collect_existing_classes(output_dir)
    written = 0
    skipped = 0

    for class_name, data in sorted(reference.items()):
        filename = data.get("filename", default_filename(class_name))
        filepath = os.path.join(output_dir, filename)

        # Check class name collision
        if class_name in existing_classes:
            print(
                f"  ⏭ SKIP  {filename}  — class '{class_name}' exists in {output_dir}"
            )
            skipped += 1
            continue

        # Check filename collision
        if os.path.exists(filepath) and not force:
            print(f"  ⏭ SKIP  {filename}  — file exists (use --force to overwrite)")
            skipped += 1
            continue

        js = generator(class_name, data)
        with open(filepath, "w") as f:
            json.dump(js, f, indent=2)
        print(f"  ✓  {filename}")
        written += 1

    return written


# ── CLI ──────────────────────────────────────────────────────────────────────


def resolve_path(rel: str, script_dir: str) -> str:
    """Resolve *rel* relative to *script_dir*, then to cwd if not found."""
    candidate = os.path.join(script_dir, rel)
    if os.path.exists(os.path.dirname(candidate)):
        return os.path.abspath(candidate)
    return os.path.abspath(rel)


def main() -> None:
    script_dir = os.path.dirname(os.path.abspath(__file__))

    parser = argparse.ArgumentParser(
        description="Convert Arma 3 weapon/ammo reference data to ABE JSON configs."
    )
    parser.add_argument(
        "--weapons-dir",
        default=resolve_path("../../data/weapons", script_dir),
        help="Output directory for weapon JSONs",
    )
    parser.add_argument(
        "--ammo-dir",
        default=resolve_path("../../data/ammo", script_dir),
        help="Output directory for ammo JSONs",
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help="Overwrite existing files (default: skip)",
    )
    args = parser.parse_args()

    weapons_dir = args.weapons_dir
    ammo_dir = args.ammo_dir

    print(f"ABE Arma 3 → JSON converter")
    print(f"Weapons → {weapons_dir}")
    print(f"Ammo    → {ammo_dir}")
    print()

    w = write_jsons(
        weapons_dir, REFERENCE_WEAPONS, generate_weapon_json, "weapon", args.force
    )
    a = write_jsons(ammo_dir, REFERENCE_AMMO, generate_ammo_json, "ammo", args.force)
    print(f"\nDone — {w} weapons, {a} ammo files written.")


if __name__ == "__main__":
    main()
