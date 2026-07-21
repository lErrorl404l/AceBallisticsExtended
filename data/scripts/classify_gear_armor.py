#!/usr/bin/env python3
"""
ABE Gear Armor Classifier
=========================
Classifies wearable armor items (vests, helmets, uniforms) into material types
based on classname patterns and config armor values.

Usage:
  # From SQF dump (paste sqf_export_gear_armor.sqf output into file):
  python classify_gear_armor.py --input gear_dump.txt --output classified_gear

  # Dry-run with sample data:
  python classify_gear_armor.py --sample
"""

import argparse
import csv
import json
import re
import sys
from pathlib import Path
from typing import Any


# ── Heuristic classification rules ──────────────────────────────────────────────
# Format: (pattern, material, confidence, description)
# Patterns are matched in order (first match wins).

VEST_CLASSNAME_RULES = [
    # Heavy plate carriers → ceramic + UHMWPE composite (Level III-IV equivalent)
    (
        r"V_PlateCarrier[123]_",
        ("ceramic_sic", "uhmwpe"),
        0.90,
        "Plate carrier with hard ceramic inserts",
    ),
    (
        r"V_PlateCarrierSpec_",
        ("ceramic_sic", "uhmwpe"),
        0.90,
        "Special ops plate carrier",
    ),
    (
        r"V_PlateCarrierGL_",
        ("ceramic_sic", "uhmwpe"),
        0.90,
        "Plate carrier with grenade launcher",
    ),
    (r"V_PlateCarrierIA", ("ceramic_al2o3", "uhmwpe"), 0.85, "CSAT plate carrier"),
    (r"V_PlateCarrier_Kerry", ("ceramic_sic", "uhmwpe"), 0.85, "Kerry plate carrier"),
    # Medium vests → kevlar + optional plate
    (
        r"V_TacVest_",
        ("composite_kevlar", "spall_liner"),
        0.80,
        "Tactical vest with kevlar",
    ),
    (r"V_TacVestIR_", ("composite_kevlar", "spall_liner"), 0.80, "Tactical vest IR"),
    # Light vests → soft armor only
    (r"V_Press_F", ("composite_kevlar", ""), 0.70, "Press vest (light kevlar)"),
    (r"V_Rangemaster_", ("composite_kevlar", ""), 0.60, "Rangemaster vest"),
    (r"V_LegStrapBag_", ("composite_kevlar", ""), 0.50, "Leg strap bag"),
    # Light carriers (sport/carrier) → cloth, light padding
    (r"V_Chestrig_", ("kevlar_liner", ""), 0.60, "Chest rig (light)"),
    (r"V_Bandollier_", ("spall_liner", ""), 0.50, "Bandolier (minimal protection)"),
    # Harness / belt
    (r"V_HarnessO_", ("composite_kevlar", ""), 0.60, "Harness rig"),
    (r"V_HarnessOGL_", ("composite_kevlar", ""), 0.60, "Harness rig with GL"),
    (r"V_Safety_", ("spall_liner", ""), 0.40, "Safety vest (minimal)"),
    (r"V_Rebreather", ("spall_liner", ""), 0.50, "Rebreather (non-armor, air supply)"),
    # EOD / heavy armor → ceramic + UHMWPE
    (r"V_EOD_", ("ceramic_b4c", "uhmwpe"), 0.85, "EOD suit"),
    # Default fallback for unclassified vests
    (r"V_", ("composite_kevlar", ""), 0.40, "Generic vest (assumed soft armor)"),
]

HELMET_CLASSNAME_RULES = [
    # Ballistic helmets → composite kevlar / UHMWPE
    (r"H_HelmetB$", ("composite_kevlar", ""), 0.85, "Ballistic helmet base (ECH/MICH)"),
    (r"H_HelmetB_", ("composite_kevlar", ""), 0.85, "Ballistic helmet variant"),
    (r"H_HelmetO_", ("composite_kevlar", ""), 0.85, "Spec ops helmet"),
    (r"H_HelmetIA_", ("composite_kevlar", ""), 0.80, "IA helmet"),
    (r"H_HelmetCrew_", ("composite_kevlar", ""), 0.75, "Crew helmet"),
    (
        r"H_HelmetSpecB$",
        ("composite_kevlar", ""),
        0.85,
        "Spec ops ballistic helmet base",
    ),
    (
        r"H_HelmetSpecB_",
        ("composite_kevlar", ""),
        0.85,
        "Spec ops ballistic helmet variant",
    ),
    (r"H_PilotHelmet", ("composite_kevlar", ""), 0.70, "Pilot helmet"),
    # Heavy helmets → enhanced composite
    (
        r"H_HelmetHeavy_",
        ("ceramic_al2o3", "kevlar_liner"),
        0.80,
        "Heavy helmet with ceramic",
    ),
    (r"H_HelmetLeaderO_", ("composite_kevlar", ""), 0.80, "Leader helmet"),
    (r"H_HelmetAggressor_", ("composite_kevlar", ""), 0.75, "Aggressor helmet"),
    # Headsets / ear protection → rubber/polycarbonate (ComTac, Peltor, etc.)
    (
        r"(?i)comtac",
        ("rubber_elastomer", ""),
        0.70,
        "Tactical headset (rubber/polycarbonate)",
    ),
    (r"(?i)headset", ("polycarbonate", ""), 0.65, "Headset (electronics/plastic)"),
    # Fix existing headset/earpro entries that wrongly used "cloth"
    (r"H_HeadSet_", ("polycarbonate", ""), 0.60, "Headset (electronics/plastic)"),
    (
        r"H_EarProtectors_",
        ("rubber_elastomer", ""),
        0.60,
        "Ear protectors (rubber/plastic)",
    ),
    # Light / caps → cloth (no real ballistic protection)
    (r"H_Cap_", ("arma_plastic", ""), 0.50, "Cap (minimal protection)"),
    (r"H_Bandanna_", ("cloth", ""), 0.60, "Bandanna (cloth)"),
    (r"H_Booniehat_", ("cloth", ""), 0.60, "Boonie hat (cloth)"),
    (r"H_MilCap_", ("cloth", ""), 0.60, "Military cap (cloth)"),
    (r"H_Beret_", ("cloth", ""), 0.60, "Beret (cloth)"),
    (r"H_Wig_", ("cloth", ""), 0.50, "Wig (cloth)"),
    (r"H_StrawHat", ("cloth", ""), 0.60, "Straw hat (straw)"),
    (r"H_Hat_", ("cloth", ""), 0.60, "Hat (cloth)"),
    (r"H_Shemag_", ("cloth", ""), 0.60, "Shemag (cloth)"),
    (r"H_ShemagOpen", ("cloth", ""), 0.60, "Shemag open (cloth)"),
    (r"H_Watchcap", ("cloth", ""), 0.60, "Watch cap (knit)"),
    (r"H_Turban_", ("cloth", ""), 0.60, "Turban (cloth)"),
    (r"H_HeadBandage_", ("cloth", ""), 0.50, "Bandage (cloth)"),
    (r"H_HeadSet_", ("cloth", ""), 0.50, "Headset (plastic)"),
    (r"H_EarProtectors_", ("cloth", ""), 0.50, "Ear protectors (plastic)"),
    (r"H_RacingHelmet_", ("composite_kevlar", ""), 0.60, "Racing helmet"),
    (r"H_CrewHelmet_", ("composite_kevlar", ""), 0.60, "Crew helmet"),
    # Fallback
    (r"H_", ("composite_kevlar", ""), 0.40, "Generic helmet"),
]

UNIFORM_CLASSNAME_RULES = [
    # Combat uniforms → cloth (cotton/nylon ripstop, zero ballistic protection)
    (r"U_B_CombatUniform", ("cloth", ""), 0.70, "Combat uniform (cotton/nylon)"),
    (r"U_O_CombatUniform", ("cloth", ""), 0.70, "Combat uniform (cotton/nylon)"),
    (r"U_I_CombatUniform", ("cloth", ""), 0.70, "Combat uniform (cotton/nylon)"),
    (r"U_BG_Guerrilla_", ("cloth", ""), 0.60, "Guerrilla uniform"),
    (r"U_IG_Guerilla", ("cloth", ""), 0.60, "Guerilla uniform"),
    (r"U_BasicBody", ("cloth", ""), 0.80, "Basic body (minimal undersuit)"),
    (r"U_B_CBRN_Suit", ("spall_liner", ""), 0.60, "CBRN suit (butyl rubber)"),
    (r"U_C_CBRN_Suit", ("spall_liner", ""), 0.60, "CBRN suit (butyl rubber)"),
    # Wetsuits → neoprene
    (r"U_B_Wetsuit", ("rubber_elastomer", ""), 0.70, "Wetsuit (neoprene)"),
    (r"U_O_Wetsuit", ("rubber_elastomer", ""), 0.70, "Wetsuit (neoprene)"),
    (r"U_I_Wetsuit", ("rubber_elastomer", ""), 0.70, "Wetsuit (neoprene)"),
    # Civilian clothes → cotton/polyester
    (r"U_C_Poloshirt", ("cloth", ""), 0.60, "Poloshirt (cotton)"),
    (r"U_C_Poor_", ("cloth", ""), 0.60, "Civilian clothes"),
    (r"U_C_WorkerCoveralls", ("cloth", ""), 0.60, "Coveralls (cotton/poly)"),
    (r"U_C_HunterBody", ("cloth", ""), 0.60, "Hunter clothing"),
    (r"U_C_Driver_", ("cloth", ""), 0.60, "Driver clothes"),
    # Flight gear → Nomex
    (r"U_B_PilotCoveralls", ("cloth", ""), 0.70, "Pilot coveralls (Nomex)"),
    (r"U_O_PilotCoveralls", ("cloth", ""), 0.70, "Pilot coveralls (Nomex)"),
    (r"U_I_pilotCoveralls", ("cloth", ""), 0.70, "Pilot coveralls (Nomex)"),
    (r"U_B_HeliPilotCoveralls", ("cloth", ""), 0.70, "Heli pilot coveralls (Nomex)"),
    (r"U_I_HeliPilotCoveralls", ("cloth", ""), 0.70, "Heli pilot coveralls (Nomex)"),
    (r"U_O_HeliPilotCoveralls", ("cloth", ""), 0.70, "Heli pilot coveralls (Nomex)"),
    # Ghillie suits → burlap/jute
    (r"U_B_GhillieSuit", ("cloth", ""), 0.75, "Ghillie suit (burlap)"),
    (r"U_O_GhillieSuit", ("cloth", ""), 0.75, "Ghillie suit (burlap)"),
    (r"U_I_GhillieSuit", ("cloth", ""), 0.75, "Ghillie suit (burlap)"),
    (r"U_B_FullGhillie", ("cloth", ""), 0.80, "Full ghillie (jute/hessian)"),
    (r"U_O_FullGhillie", ("cloth", ""), 0.80, "Full ghillie (jute/hessian)"),
    (r"U_I_FullGhillie", ("cloth", ""), 0.80, "Full ghillie (jute/hessian)"),
    # Special / unique
    (r"U_B_CTRG_", ("cloth", ""), 0.60, "CTRG uniform"),
    (r"U_C_Scientist", ("cloth", ""), 0.60, "Scientist coat"),
    (r"U_C_Journalist", ("cloth", ""), 0.60, "Journalist clothes"),
    (r"U_Marshal", ("cloth", ""), 0.60, "Marshals uniform"),
    (r"U_B_survival_uniform", ("cloth", ""), 0.60, "Survival suit"),
    (r"U_Competitor", ("cloth", ""), 0.60, "Competitor suit"),
    (r"U_O_SpecopsUniform", ("cloth", ""), 0.70, "Spec ops uniform"),
    (r"U_O_OfficerUniform", ("cloth", ""), 0.60, "Officer uniform"),
    (r"U_I_OfficerUniform", ("cloth", ""), 0.60, "Officer uniform"),
    (r"U_B_Protagonist_VR", ("cloth", ""), 0.60, "VR protagonist"),
    (r"U_O_Protagonist_VR", ("cloth", ""), 0.60, "VR protagonist"),
    (r"U_I_Protagonist_VR", ("cloth", ""), 0.60, "VR protagonist"),
    (r"U_C_Protagonist_VR", ("cloth", ""), 0.60, "VR protagonist"),
    (r"U_I_G_Story_Protagonist_F", ("cloth", ""), 0.60, "Story protagonist"),
    (r"U_I_G_resistanceLeader_F", ("cloth", ""), 0.60, "Resistance leader"),
    (r"U_Rangemaster", ("cloth", ""), 0.60, "Rangemaster uniform"),
    (r"U_OrestesBody", ("cloth", ""), 0.50, "Orestes body"),
    # Generic catch-all
    (r"U_", ("cloth", ""), 0.40, "Generic uniform"),
]

GLASSES_CLASSNAME_RULES = [
    # Balaclavas → cloth (face covering, not armor)
    (r"G_Balaclava_", ("cloth", ""), 0.70, "Balaclava (cloth face covering)"),
    # Bandannas → cloth
    (r"G_Bandanna_", ("cloth", ""), 0.70, "Bandanna (cloth)"),
    # Goggles / ballistic eyewear → polycarbonate
    (r"G_Combat_", ("polycarbonate", ""), 0.70, "Combat goggles (polycarbonate)"),
    (r"G_Tactical_", ("polycarbonate", ""), 0.70, "Tactical glasses (polycarbonate)"),
    (r"G_Sport_", ("polycarbonate", ""), 0.60, "Sport glasses"),
    # Diving → rubber
    (r"G_Diving", ("rubber_elastomer", ""), 0.80, "Diving mask (rubber)"),
    # Spectacles / sunglasses → polycarbonate
    (r"G_Spectacles", ("polycarbonate", ""), 0.60, "Spectacles (polycarbonate)"),
    (r"G_Squares", ("polycarbonate", ""), 0.60, "Squares (polycarbonate)"),
    (r"G_Aviator", ("polycarbonate", ""), 0.60, "Aviator glasses"),
    (r"G_Lady_", ("polycarbonate", ""), 0.60, "Lady glasses"),
    (r"G_Shades_", ("polycarbonate", ""), 0.60, "Shades"),
    (r"G_Sunglasses_", ("polycarbonate", ""), 0.60, "Sunglasses"),
    (r"G_Lowprofile", ("polycarbonate", ""), 0.60, "Low-profile goggles"),
    # Respirator → spall liner
    (r"G_Respirator_", ("spall_liner", ""), 0.60, "Respirator mask"),
    # Generic glasses catch-all
    (r"G_", ("polycarbonate", ""), 0.50, "Generic glasses"),
]


# ── Armor-value-based thresholds (fallback for unknown items) ───────────────────
# These are based on ACE's discretization and vanilla value ranges.


def classify_by_armor_value(armor: float, pass_through: float, item_type: str) -> dict:
    """Fallback classification based on numeric armor/passthrough values.

    Uses ACE's proven armor level thresholds:
      Level 0 (armor < 12):  ~12mm RHAe  — cloth/light padding
      Level 1 (armor 12-18): ~30mm RHAe  — soft armor / light kevlar
      Level 2 (armor 18-22): ~42mm RHAe  — composite / light plate
      Level 3 (armor 22-26): ~80mm RHAe  — ceramic hard plate
      Level 4 (armor ≥ 26):  ~110mm RHAe — heavy ceramic / ESAPI
    """

    if item_type == "VestItem":
        if armor >= 26:
            return {
                "material": "ceramic_sic",
                "backing": "uhmwpe",
                "confidence": 0.50,
                "method": "armor_threshold",
                "note": "ACE L4 heavy ceramic plate (110mm RHAe)",
            }
        elif armor >= 18:
            return {
                "material": "ceramic_al2o3",
                "backing": "uhmwpe",
                "confidence": 0.45,
                "method": "armor_threshold",
                "note": "ACE L2-L3 light ceramic/composite plate (42-80mm RHAe)",
            }
        elif armor >= 12:
            return {
                "material": "composite_kevlar",
                "backing": "",
                "confidence": 0.40,
                "method": "armor_threshold",
                "note": "ACE L1 soft armor / kevlar (30mm RHAe)",
            }
        else:
            return {
                "material": "spall_liner",
                "backing": "",
                "confidence": 0.30,
                "method": "armor_threshold",
                "note": "ACE L0 minimal protection (12mm RHAe)",
            }

    elif item_type == "HeadgearItem":
        # Head uses armorStep=2 instead of body's 4
        # ACE head: level = round((armor - 6) / 2)  (base 2 + head offset 4)
        # L0: <8, L1: 8-10, L2: 10-12, L3: 12-14, L4: ≥14
        if armor >= 14:
            return {
                "material": "ceramic_al2o3",
                "backing": "kevlar_liner",
                "confidence": 0.45,
                "method": "armor_threshold",
                "note": "ACE L4 heavy helmet (110mm RHAe)",
            }
        elif armor >= 8:
            return {
                "material": "composite_kevlar",
                "backing": "",
                "confidence": 0.45,
                "method": "armor_threshold",
                "note": "ACE L1-L3 ballistic helmet (30-80mm RHAe)",
            }
        else:
            return {
                "material": "cloth",
                "backing": "",
                "confidence": 0.30,
                "method": "armor_threshold",
                "note": "ACE L0 light headgear (12mm RHAe)",
            }

    elif item_type == "UniformItem":
        if armor >= 10:
            return {
                "material": "composite_kevlar",
                "backing": "",
                "confidence": 0.35,
                "method": "armor_threshold",
                "note": "Armored uniform",
            }
        else:
            return {
                "material": "cloth",
                "backing": "",
                "confidence": 0.30,
                "method": "armor_threshold",
                "note": "Standard uniform",
            }

    elif item_type == "GlassesItem":
        return {
            "material": "polycarbonate",
            "backing": "",
            "confidence": 0.30,
            "method": "armor_threshold",
            "note": "Glasses",
        }

    return {
        "material": "unknown",
        "backing": "",
        "confidence": 0.10,
        "method": "armor_threshold",
        "note": "Unknown type",
    }


# ── Sentinel armor detection ──────────────────────────────────────────────────
# Many mods use armor=400 with passThrough=0 as a "disable vanilla damage"
# convention (ACE3 handles the actual armor simulation). When we see this
# pattern with no hitpoint data, we cannot determine real protection from
# the ItemInfo armor field alone — it's a sentinel, not a measurement.
#
# Detection threshold: armor >= 50 with pass_through near 0 is clearly
# beyond any real ACE/NIJ armor level and indicates a sentinel value.
# Vanilla systems cap at ~30 for Level IV (110mm RHAe equivalent).

SENTINEL_ARMOR_THRESHOLD = 50.0
SENTINEL_PASSTHROUGH_THRESHOLD = 0.05


def is_sentinel_armor(armor_value: float, pass_through: float) -> bool:
    """Detect modder sentinel armor (armor=400/pt=0 'disable damage' pattern).

    Returns True when the armor value is clearly beyond meaningful ACE/NIJ
    levels and passthrough is near zero, indicating the value is a sentinel
    rather than a real measurement of protection.
    """
    return (
        armor_value >= SENTINEL_ARMOR_THRESHOLD
        and pass_through <= SENTINEL_PASSTHROUGH_THRESHOLD
    )


def sentinel_estimate(item_type: str) -> float:
    """Return a conservative armor estimate for sentinel items with no hitpoints.

    When a mod uses armor=400/pt=0 and we have no hitpoint data, we estimate
    a reasonable protection level based on item type:
      - Vests: armor=22 (Level III, ~61-80mm RHAe — standard plate carrier)
      - Helmets: armor=12 (Level II, ~42mm RHAe — typical ballistic helmet)
      - Uniforms: armor=0 (no meaningful protection)
      - Glasses: armor=0 (no meaningful protection)
    """
    if item_type == "VestItem":
        return 22.0
    elif item_type == "HeadgearItem":
        return 8.0
    else:
        return 0.0


# ── ACE-compatible armor → RHAe conversion ──────────────────────────────────
# Based on ACE alternateArmorPenetration (PR #9217):
#   _armor = (_realDamage / _engineDamage) - UNSCALED_BASE_ARMOR(2)
#   bodyLevel = round((_armor - 8) / 4), capped 0-4
#   _armorThickness = [6, 15, 21, 40, 55] select level  (HALVED for vanilla
#   caliber scaling — multiply ×2 for real-world RHAe)
#
# So for a single gear item (no multiple-gear sum like ACE gets at runtime):
#   effective = perHitpointArmor - 10 (base 2 + body threshold 8)
#   level = round(effective / 4), clamped 0-4
#   real_RHAe = [12, 30, 42, 80, 110][level]


ACE_IN_GAME_RHA = [6.0, 15.0, 21.0, 40.0, 55.0]  # ACE's halved values
ACE_REAL_RHA = [12.0, 30.0, 42.0, 80.0, 110.0]  # Real-world RHAe (×2)
ACE_ARMOR_THRESHOLDS = [0, 10, 18, 26, 34]  # Armor value→level breakpoints


def armor_to_rha_mm(
    armor_value: float,
    pass_through: float = 1.0,
    hp_parts: list[dict] | None = None,
    item_type: str = "VestItem",
) -> float:
    """ACE-compatible RHAe conversion from per-hitpoint armor.

    Follows ACE alternateArmorPenetration's proven formula.
    ACE has separate step sizes for head (step=2) vs body (step=4):

      Body:  level = max(0, min(4, round((totalArmor - 10) / 4)))
      Head:  level = max(0, min(4, round((totalArmor - 6) / 2)))

    Maps to RHAe: L0=12mm, L1=30mm, L2=42mm, L3=80mm, L4=110mm
    (ACE's in-game values [6,15,21,40,55] × 2 for real-world RHAe)
    """
    if hp_parts:
        hp_armors = [h.get("armor", 0) for h in hp_parts]
        max_hp_armor = max(hp_armors) if hp_armors else armor_value
    else:
        # Check for sentinel armor (modder uses armor=400/pt=0 to disable vanilla damage)
        if is_sentinel_armor(armor_value, pass_through):
            max_hp_armor = sentinel_estimate(item_type)
        else:
            max_hp_armor = armor_value

    if max_hp_armor <= 0:
        return 0.0

    # ACE's armor step depends on hitpoint type
    # HeadgearItem uses step=2 (head), everything else step=4 (body)
    is_head = item_type == "HeadgearItem"
    step = 2.0 if is_head else 4.0

    # ACE formula: effective = totalArmor - UNSCALED_BASE_ARMOR(2) - step*2
    # Body step=4: effective = armor - 10
    # Head step=2: effective = armor - 6
    effective = max_hp_armor - (2.0 + step * 2.0)

    # ACE in-game RHAe values (halved, using same 5-level table for both body and head)
    ace_in_game = [6.0, 15.0, 21.0, 40.0, 55.0]
    # Real RHAe = × 2
    ace_real = [12.0, 30.0, 42.0, 80.0, 110.0]

    if effective <= 0:
        return ace_real[0]

    # Continuous interpolation between ACE levels
    # Body breakpoints: effective=0→L0, 4→L1, 8→L2, 12→L3, 16→L4
    # Head breakpoints: effective=0→L0, 2→L1, 4→L2, 6→L3, 8→L4
    eff_bp = [step * i for i in range(5)] + [
        100.0
    ]  # [0,4,8,12,16,100] body, [0,2,4,6,8,100] head

    for i in range(len(eff_bp) - 1):
        lo, hi = eff_bp[i], eff_bp[i + 1]
        if lo <= effective <= hi:
            t = (effective - lo) / (hi - lo) if hi > lo else 0.0
            rha = ace_real[i] + t * (ace_real[min(i + 1, 4)] - ace_real[i])
            return round(rha, 1)

    return ace_real[4]


def ace_armor_level(armor_value: float) -> int:
    """ACE armor level 0-4 from a vanilla config armor value."""
    if armor_value <= 0:
        return 0
    # ACE formula for body: level = round((armor - 10) / 4)  (includes base 2 + offset 8)
    effective = armor_value - 10.0
    if effective <= 0:
        return 0
    return max(0, min(4, round(effective / 4.0)))


# ── Main classification logic ────────────────────────────────────────────────


def classify_item(
    classname: str,
    display_name: str,
    item_type: str,
    item_armor: float,
    item_pass_through: float,
    hp_parts: list[dict],
) -> dict:
    """Classify a single gear item into material type."""

    classname_lower = classname.lower()
    result = {
        "classname": classname,
        "display_name": display_name,
        "item_type": item_type.replace("Item", ""),  # VestItem → Vest
        "item_armor": item_armor,
        "item_pass_through": item_pass_through,
        "hitpoints": hp_parts,
        "material": "unknown",
        "backing_material": "",
        "confidence": 0.0,
        "classification_method": "none",
        "classification_note": "",
        "rha_equivalent_mm": armor_to_rha_mm(
            item_armor, item_pass_through, hp_parts, item_type
        ),
    }

    # Select classification rules based on type
    if item_type == "VestItem":
        rules = VEST_CLASSNAME_RULES
    elif item_type == "HeadgearItem":
        rules = HELMET_CLASSNAME_RULES
    elif item_type == "UniformItem":
        rules = UNIFORM_CLASSNAME_RULES
    elif item_type == "GlassesItem":
        rules = GLASSES_CLASSNAME_RULES
    else:
        rules = []

    # Try classname-based classification first (case-insensitive, match on lowercase)
    found_name_match = False
    for pattern, (material, backing), confidence, note in rules:
        if re.search(pattern, classname_lower, re.IGNORECASE):
            result["material"] = material
            result["backing_material"] = backing
            result["confidence"] = confidence
            result["classification_method"] = "classname"
            result["classification_note"] = note
            found_name_match = True
            break

    # ONLY fall back to armor-value heuristic if NO classname rule matched
    # (classname match is always more specific/accurate than armor fallback)
    if not found_name_match:
        fallback = classify_by_armor_value(item_armor, item_pass_through, item_type)
        if fallback["confidence"] > result["confidence"]:
            result["material"] = fallback["material"]
            result["backing_material"] = fallback["backing"]
            result["confidence"] = fallback["confidence"]
            result["classification_method"] = fallback["method"]
            result["classification_note"] = fallback["note"]

    # Reduce confidence when estimating RHA from sentinel armor (no hitpoints)
    # Modders use armor=400/pt=0 as a "disable vanilla damage" flag, so the
    # ItemInfo armor field doesn't reflect real protection. Our material guess
    # (from classname rules) is still valid, but the RHA is an estimate.
    if (
        not found_name_match
        and is_sentinel_armor(item_armor, item_pass_through)
        and not result["hitpoints"]
    ):
        # Classname rules didn't match AND sentinel + no hitpoints = double unknown
        result["confidence"] = min(result["confidence"], 0.20)
        result["classification_method"] = "sentinel_estimate"
        result["classification_note"] = (
            "Sentinel armor (armor=400/pt=0), no hitpoints — RHA estimated from "
            "material type, not item_armor field"
        )
    elif (
        found_name_match
        and is_sentinel_armor(item_armor, item_pass_through)
        and not result["hitpoints"]
    ):
        # Classname matched but armor is sentinel — material is reliable, RHA estimated
        # Reduce confidence slightly since we're guessing protection level
        result["confidence"] = result["confidence"] * 0.85
        result["classification_note"] = (
            f"{result['classification_note']}; sentinel armor detected, "
            "RHA estimated from material type"
        )

    return result


# ── Output generation ────────────────────────────────────────────────────────


def generate_armor_tsv(items: list[dict], output_path: Path):
    """Generate TSV in the same format as ir_armor.tsv for wearable items."""
    rows = [
        [
            "# classname",
            "item_type",
            "material",
            "thickness_mm",
            "angle_deg",
            "backing",
            "backing_thickness_mm",
            "backing_angle_deg",
        ]
    ]

    for item in items:
        if item["material"] == "unknown" or item["material"] == "cloth":
            continue  # Skip non-armor items
        # Estimate thickness from RHAe / material_factor
        # We use a simplified mapping
        mat_rha_factor = {
            "ceramic_sic": 3.5,
            "ceramic_b4c": 4.5,
            "ceramic_al2o3": 2.5,
            "composite_kevlar": 0.6,
            "kevlar_liner": 0.2,
            "spall_liner": 0.1,
            "uhmwpe": 0.25,
            "polycarbonate": 0.06,
            "rubber_elastomer": 0.015,
            "arma_plastic": 0.06,
            "cloth": 0.01,
        }.get(item["material"], 1.0)

        thickness_mm = item["rha_equivalent_mm"] / max(mat_rha_factor, 0.01)

        rows.append(
            [
                item["classname"],
                item["item_type"],
                item["material"],
                f"{thickness_mm:.1f}",
                "0.0",
                item.get("backing_material", ""),
                f"{thickness_mm * 0.5:.1f}" if item.get("backing_material") else "0",
                "0.0",
            ]
        )

    with open(output_path, "w") as f:
        for row in rows:
            f.write("\t".join(row) + "\n")


def generate_lookup_json(items: list[dict], output_path: Path):
    """Generate a JSON lookup table for body armor classification."""
    # Structure: { "classname": { "material": ..., "backing": ..., "armor_level": ..., "rha_mm": ... } }
    lookup = {}
    for item in items:
        lookup[item["classname"]] = {
            "type": item["item_type"],
            "material": item["material"],
            "backing": item["backing_material"],
            "rha_mm": item["rha_equivalent_mm"],
            "confidence": item["confidence"],
            "method": item["classification_method"],
        }

    with open(output_path, "w") as f:
        json.dump(lookup, f, indent=2)


def generate_summary(items: list[dict]) -> dict:
    """Generate classification statistics."""
    total = len(items)
    by_type: dict[str, int] = {}
    by_method: dict[str, int] = {}
    by_confidence: dict[str, int] = {
        "high (>=0.8)": 0,
        "medium (0.4-0.8)": 0,
        "low (<0.4)": 0,
    }
    materials: dict[str, int] = {}

    for item in items:
        t = item["item_type"]
        by_type[t] = by_type.get(t, 0) + 1

        m = item["classification_method"]
        by_method[m] = by_method.get(m, 0) + 1

        c = item["confidence"]
        if c >= 0.8:
            by_confidence["high (>=0.8)"] += 1
        elif c >= 0.4:
            by_confidence["medium (0.4-0.8)"] += 1
        else:
            by_confidence["low (<0.4)"] += 1

        mat = item["material"]
        materials[mat] = materials.get(mat, 0) + 1

    return {
        "total_items": total,
        "by_type": by_type,
        "by_method": by_method,
        "by_confidence": by_confidence,
        "materials": dict(sorted(materials.items(), key=lambda x: -x[1])),
    }


# ── Sample data (for dry-run testing) ────────────────────────────────────────

SAMPLE_DUMP = """G|V_PlateCarrier1_blk|Carrier Lite (Black)|VestItem|VestItem|4|0.5|Chest:10:0.5:HitChest~Diaphragm:6:0.5:HitDiaphragm~Abdomen:6:0.5:HitAbdomen~Body:0:0.5:HitBody|
G|V_PlateCarrier2_blk|Carrier Rig (Black)|VestItem|VestItem|8|0.5|Chest:16:0.5:HitChest~Diaphragm:12:0.5:HitDiaphragm~Abdomen:12:0.5:HitAbdomen~Body:0:0.5:HitBody|
G|V_PlateCarrier3_blk|Carrier GL (Black)|VestItem|VestItem|12|0.5|Chest:24:0.5:HitChest~Diaphragm:18:0.5:HitDiaphragm~Abdomen:18:0.5:HitAbdomen~Body:0:0.5:HitBody|
G|V_TacVest_blk|Tactical Vest (Black)|VestItem|VestItem|4|0.5|Chest:4:0.5:HitChest~Diaphragm:2:0.5:HitDiaphragm~Abdomen:2:0.5:HitAbdomen~Body:0:0.5:HitBody|
G|H_HelmetB_plain|ECH (Plain)|HeadgearItem|HeadgearItem|0|0.3|Head:6:0.5:HitHead|
G|H_HelmetO_ocamo|Protector Helmet (Hex)|HeadgearItem|HeadgearItem|0|0.3|Head:8:0.4:HitHead|
G|H_Cap_blk|Cap (Black)|HeadgearItem|HeadgearItem|0|0.8|Head:0:0.8:HitHead|
G|U_B_CombatUniform_mcam|Combat Fatigues (MTP)|UniformItem|UniformItem|0|0.9||O_Soldier_F|HitChest:1~HitHead:1~HitLegs:1
G|G_Balaclava_blk|Balaclava (Black)|GlassesItem|GlassesItem|0|0.5|||
"""


# ── Parser ───────────────────────────────────────────────────────────────────


def parse_sqf_dump(text: str) -> list[dict]:
    """Parse the SQF export format: G|classname|display|type|base|armor|passThrough|hitpoints|uniformInfo"""
    items = []
    for line in text.strip().split("\n"):
        line = line.strip()
        if not line or line.startswith("#"):
            continue

        parts = line.split("|")
        # Minimum: G + classname + display + type + base + armor + passThrough + hp (8 fields)
        if len(parts) < 8:
            continue
        if parts[0] != "G":
            continue

        # Format: G|classname|display|type|base|armor|passthrough|hitpoints|[uniformInfo]
        # Use safe float parsing — weapons/attachments may slip through with non-numeric values
        def _sf(v, d=0.0):
            try:
                return float(v) if v else d
            except (ValueError, TypeError):
                return d

        item = {
            "classname": parts[1],
            "display_name": parts[2],
            "item_type": parts[3],
            "base_class": parts[4],
            "item_armor": _sf(parts[5]),
            "item_pass_through": _sf(parts[6], 1.0),
            "hp_raw": parts[7],
        }

        # Parse hitpoints (format: name:armor:passThrough:hitpointName~...)
        item["hitpoints"] = []
        for hp_entry in item["hp_raw"].split("~"):
            hp_entry = hp_entry.strip()
            if not hp_entry:
                continue
            hp_parts = hp_entry.split(":")
            if len(hp_parts) >= 2:
                item["hitpoints"].append(
                    {
                        "name": hp_parts[0],
                        "armor": _sf(hp_parts[1]),
                        "pass_through": _sf(hp_parts[2], 1.0)
                        if len(hp_parts) > 2 and hp_parts[2]
                        else 1.0,
                        "hitpoint_name": hp_parts[3] if len(hp_parts) > 3 else "",
                    }
                )

        # Uniform info in parts[8]: uniformClass|hitpoint:armor~...
        if item["item_type"] == "UniformItem" and len(parts) > 8:
            uni_parts = parts[8].split("|")
            item["uniform_class"] = uni_parts[0] if uni_parts else ""
            item["uniform_hitpoints"] = []
            if len(uni_parts) > 1:
                for hp_entry in uni_parts[1].split("~"):
                    hp_entry = hp_entry.strip()
                    if not hp_entry:
                        continue
                    hp_parts2 = hp_entry.split(":")
                    if len(hp_parts2) >= 2:
                        item["uniform_hitpoints"].append(
                            {
                                "name": hp_parts2[0],
                                "armor": float(hp_parts2[1]),
                            }
                        )
        else:
            item["uniform_class"] = ""
            item["uniform_hitpoints"] = []

        items.append(item)
    return items


# ── Main entry point ─────────────────────────────────────────────────────────


def main():
    parser = argparse.ArgumentParser(description="Classify Arma 3 gear armor materials")
    parser.add_argument(
        "--input", "-i", help="Input file from sqf_export_gear_armor.sqf"
    )
    parser.add_argument(
        "--output",
        "-o",
        default="classified_gear",
        help="Output prefix (creates .tsv and .json)",
    )
    parser.add_argument(
        "--sample", action="store_true", help="Run with built-in sample data (dry-run)"
    )
    args = parser.parse_args()

    if args.sample:
        text = SAMPLE_DUMP
    elif args.input:
        with open(args.input) as f:
            text = f.read()
    else:
        text = sys.stdin.read()

    # Parse
    items = parse_sqf_dump(text)
    print(f"Parsed {len(items)} gear items")

    # Classify (extract only the fields classify_item expects)
    classified = []
    for item in items:
        classified.append(
            classify_item(
                classname=item["classname"],
                display_name=item["display_name"],
                item_type=item["item_type"],
                item_armor=item["item_armor"],
                item_pass_through=item["item_pass_through"],
                hp_parts=item["hitpoints"],
            )
        )

    # Generate outputs
    output_prefix = Path(args.output)
    tsv_path = (
        output_prefix.with_suffix(".tsv")
        if output_prefix.suffix
        else output_prefix.with_name(output_prefix.name + "_armor.tsv")
    )
    json_path = (
        output_prefix.with_suffix(".json")
        if output_prefix.suffix
        else output_prefix.with_name(output_prefix.name + "_lookup.json")
    )

    generate_armor_tsv(classified, tsv_path)
    generate_lookup_json(classified, json_path)

    # Print summary
    summary = generate_summary(classified)
    print(f"\n=== Classification Summary ===")
    print(f"Total items: {summary['total_items']}")
    print(f"\nBy type: {json.dumps(summary['by_type'], indent=2)}")
    print(f"\nBy method: {json.dumps(summary['by_method'], indent=2)}")
    print(f"\nBy confidence: {json.dumps(summary['by_confidence'], indent=2)}")
    print(f"\nMaterials found: {json.dumps(summary['materials'], indent=2)}")
    print(f"\nOutputs:")
    print(f"  TSV: {tsv_path}")
    print(f"  JSON: {json_path}")


if __name__ == "__main__":
    main()
