#!/usr/bin/env python3
"""
Extract weapon/ammo data from ACE3 HPP configs and generate ABE JSON files.

Sources (from /tmp/ACE3/ git clone):
  - addons/ballistics/CfgWeapons.hpp    — weapon overrides (111+ vanilla)
  - addons/ballistics/CfgAmmo.hpp       — ammo ballistics (40+ calibers)
  - addons/ballistics/CfgMagazines.hpp   — magazine→ammo linkage
  - addons/ballistics/CfgMagazineWells.hpp — weapon→magazine linkage
  - addons/compat_*/CfgWeapons.hpp      — CDLC/mod weapon overrides (21 sets)

Pipeline:
  1. Parse all CfgWeapons.hpp files → {class: {barrelLength, barrelTwist, initSpeed, magWells}}
  2. Parse CfgAmmo.hpp → {class: {caliber, mass, BCs, dragModel, mvCurve, blCurve}}
  3. Parse CfgMagazines.hpp → {class: {ammoClass}}
  4. Parse CfgMagazineWells.hpp → {name: [magClasses]}
  5. Match each weapon to ammo via:
     a. magazineWell → magazine → ammo chain, OR
     b. class-name caliber inference (CALIBER_MAP fallback)
  6. Interpolate muzzle velocity at weapon barrel length
  7. Generate ABE JSONs

Usage:
    python3 scripts/extract_from_ace3.py
"""

import json
import os
import re
import sys
from pathlib import Path

# ── Paths ─────────────────────────────────────────────────────────────────
ACE3_DIR = Path("/tmp/ACE3")
BALLISTICS_DIR = ACE3_DIR / "addons" / "ballistics"
DATA_DIR = Path(__file__).resolve().parent.parent / "data"
WEAPON_DIRS = {
    "rifles": DATA_DIR / "weapons" / "rifles",
    "snipers": DATA_DIR / "weapons" / "snipers",
    "dmrs": DATA_DIR / "weapons" / "dmrs",
    "smgs": DATA_DIR / "weapons" / "smgs",
    "pistols": DATA_DIR / "weapons" / "pistols",
    "machine_guns": DATA_DIR / "weapons" / "machine_guns",
    "shotguns": DATA_DIR / "weapons" / "shotguns",
    "launchers": DATA_DIR / "weapons" / "launchers",
}
AMMO_DIR = DATA_DIR / "ammo"

# ── Mod prefixes to strip before caliber inference ──────────────────────
MOD_PREFIXES = [
    "r3f_",
    "rh_",
    "gm_",
    "aegis_",
    "agm_",
    "cpc_",
    "cup_",
    "vn_",
    "ws_",
    "pf_",
    "ef_",
]

# ── Caliber inference from weapon/ammo class names ───────────────────────
# Tokens are matched case-insensitively against class name (after stripping
# common mod prefixes). Ordered by specificity (longer tokens first) so
# "AK12" matches before "AK", "Mk18" before "MK", etc.
CALIBER_MAP = {
    # 5.56mm
    "Mk20": ("5.56mm", 5.56, "g1", 0.151),
    "TRG": ("5.56mm", 5.56, "g1", 0.151),
    "SPAR": ("5.56mm", 5.56, "g1", 0.151),
    "MSBS": ("5.56mm", 5.56, "g1", 0.151),
    "Mk16": ("5.56mm", 5.56, "g1", 0.151),
    "M4A1": ("5.56mm", 5.56, "g1", 0.151),
    "M4": ("5.56mm", 5.56, "g1", 0.151),
    "M16A4": ("5.56mm", 5.56, "g1", 0.151),
    "M16A3": ("5.56mm", 5.56, "g1", 0.151),
    "M16A2": ("5.56mm", 5.56, "g1", 0.151),
    "M16": ("5.56mm", 5.56, "g1", 0.151),
    "M249": ("5.56mm", 5.56, "g1", 0.151),
    "Mk200": ("5.56mm", 5.56, "g1", 0.151),
    "CAR": ("5.56mm", 5.56, "g1", 0.151),
    "556": ("5.56mm", 5.56, "g1", 0.151),
    "Stanag": ("5.56mm", 5.56, "g1", 0.151),
    "HK416": ("5.56mm", 5.56, "g7", 0.151),
    "SIG551": ("5.56mm", 5.56, "g1", 0.151),
    "Famas": ("5.56mm", 5.56, "g1", 0.151),
    "FELIN": ("5.56mm", 5.56, "g1", 0.151),
    "Minimi_762": ("7.62mm", 7.62, "g7", 0.200),
    "Minimi": ("5.56mm", 5.56, "g1", 0.151),
    "M27": ("5.56mm", 5.56, "g1", 0.151),
    "Mk12": ("5.56mm", 5.56, "g7", 0.157),
    "CTAR": ("5.56mm", 5.56, "g1", 0.151),
    "CTARS": ("5.56mm", 5.56, "g1", 0.151),
    "SDAR": ("5.56mm", 5.56, "g1", 0.151),
    "HK33": ("5.56mm", 5.56, "g1", 0.151),
    "HK53": ("5.56mm", 5.56, "g1", 0.151),
    "Velko": ("5.56mm", 5.56, "g1", 0.151),
    "HK512": ("12ga", 18.53, "g1", 0.110),  # GM HK512 shotgun
    "XMS": ("5.56mm", 5.56, "g1", 0.151),
    "Galil": ("5.56mm", 5.56, "g1", 0.151),
    "ARX": ("6.5mm", 6.5, "g7", 0.260),
    "SLR": ("7.62mm", 7.62, "g7", 0.243),  # FN FAL variant
    "Galat": ("7.62mm", 7.62, "g7", 0.200),
    "Sten": ("9mm", 9.0, "g1", 0.160),
    "Muzi": ("9mm", 9.0, "g1", 0.160),  # Mac-10 / Uzi family
    # 5.45mm
    "AK12": ("5.45mm", 5.45, "g7", 0.170),
    "AK74": ("5.45mm", 5.45, "g7", 0.170),
    "AKS74": ("5.45mm", 5.45, "g7", 0.170),
    "545": ("5.45mm", 5.45, "g7", 0.170),
    # 4.73mm (G11 caseless)
    "G11": ("4.73mm", 4.73, "g7", 0.150),
    # 5.7mm
    "570x28": ("5.7mm", 5.7, "g1", 0.125),
    "fn57": ("5.7mm", 5.7, "g1", 0.125),
    "FiveSeven": ("5.7mm", 5.7, "g1", 0.125),
    "R57": ("5.7mm", 5.7, "g1", 0.125),
    # 4.6mm
    "mp7": ("4.6mm", 4.6, "g1", 0.115),
    "46x30": ("4.6mm", 4.6, "g1", 0.115),
    # 6.5mm
    "MX": ("6.5mm", 6.5, "g7", 0.260),
    "Katiba": ("6.5mm", 6.5, "g7", 0.260),
    "CAR95": ("6.5mm", 6.5, "g7", 0.260),
    "LIM": ("6.5mm", 6.5, "g7", 0.260),
    "65": ("6.5mm", 6.5, "g7", 0.260),
    "Caseless": ("6.5mm", 6.5, "g7", 0.260),
    # 7.62mm
    "HK417": ("7.62mm", 7.62, "g7", 0.243),
    "Mk11": ("7.62mm", 7.62, "g7", 0.243),
    "SR25": ("7.62mm", 7.62, "g7", 0.243),
    "AR10": ("7.62mm", 7.62, "g7", 0.243),
    "M110": ("7.62mm", 7.62, "g7", 0.243),
    "SAMR": ("7.62mm", 7.62, "g7", 0.243),
    "FRF2": ("7.62mm", 7.62, "g7", 0.200),
    "MAG58": ("7.62mm", 7.62, "g7", 0.200),
    "msg90": ("7.62mm", 7.62, "g7", 0.243),
    "mpikms": ("7.62mm", 7.62, "g7", 0.200),
    "TT33": ("7.62mm", 7.62, "g1", 0.160),
    "AK": ("7.62mm", 7.62, "g7", 0.200),
    "AKM": ("7.62mm", 7.62, "g7", 0.200),
    "RPK": ("7.62mm", 7.62, "g7", 0.200),
    "Zafir": ("7.62mm", 7.62, "g7", 0.200),
    "MG": ("7.62mm", 7.62, "g7", 0.200),
    "Mk18": ("7.62mm", 7.62, "g7", 0.243),
    "Mk14": ("7.62mm", 7.62, "g7", 0.243),
    "EBR": ("7.62mm", 7.62, "g7", 0.243),
    "DMR": ("7.62mm", 7.62, "g7", 0.243),
    "LRR": ("7.62mm", 7.62, "g7", 0.243),
    "M240": ("7.62mm", 7.62, "g7", 0.200),
    "M60": ("7.62mm", 7.62, "g7", 0.200),
    "G3": ("7.62mm", 7.62, "g7", 0.200),
    "FAL": ("7.62mm", 7.62, "g7", 0.200),
    "PK": ("7.62mm", 7.62, "g7", 0.200),
    "PKM": ("7.62mm", 7.62, "g7", 0.200),
    "762": ("7.62mm", 7.62, "g7", 0.200),
    # 7.92mm
    "792": ("7.92mm", 7.92, "g7", 0.200),
    "8mm": ("7.92mm", 7.92, "g7", 0.200),
    # 9mm / pistol
    "9mm": ("9mm", 9.0, "g1", 0.160),
    "9x21": ("9mm", 9.0, "g1", 0.160),
    "9x19": ("9mm", 9.0, "g1", 0.160),
    "Glock": ("9mm", 9.0, "g1", 0.160),
    "G17": ("9mm", 9.0, "g1", 0.160),
    "G18": ("9mm", 9.0, "g1", 0.160),
    "G19": ("9mm", 9.0, "g1", 0.160),
    "G34": ("9mm", 9.0, "g1", 0.160),
    "cz75": ("9mm", 9.0, "g1", 0.160),
    "p226": ("9mm", 9.0, "g1", 0.160),
    "p30": ("9mm", 9.0, "g1", 0.160),
    "p38": ("9mm", 9.0, "g1", 0.160),
    "p2000": ("9mm", 9.0, "g1", 0.160),
    "ppk": ("9mm", 9.0, "g1", 0.160),
    "m9": ("9mm", 9.0, "g1", 0.160),
    "M9": ("9mm", 9.0, "g1", 0.160),
    "mk22": ("9mm", 9.0, "g1", 0.160),
    "sw659": ("9mm", 9.0, "g1", 0.160),
    "PAMAS": ("9mm", 9.0, "g1", 0.160),
    "HKUSP": ("9mm", 9.0, "g1", 0.160),
    "MP5": ("9mm", 9.0, "g1", 0.160),
    "PDW": ("9mm", 9.0, "g1", 0.160),
    "MP2": ("9mm", 9.0, "g1", 0.160),
    "PM63": ("9mm", 9.0, "g1", 0.160),
    "P1": ("9mm", 9.0, "g1", 0.160),  # Walther P1
    "P210": ("9mm", 9.0, "g1", 0.160),
    "gsh18": ("9mm", 9.0, "g1", 0.160),
    "kimber": ("9mm", 9.0, "g1", 0.160),
    "sbr9": ("9mm", 9.0, "g1", 0.160),
    "tec9": ("9mm", 9.0, "g1", 0.160),
    "vp70": ("9mm", 9.0, "g1", 0.160),
    "vz61": (".32 ACP", 7.65, "g1", 0.125),
    "mak": ("9.65mm", 9.65, "g1", 0.160),
    # .45 ACP
    "45ACP": (".45 ACP", 11.43, "g1", 0.185),
    "45_ACP": (".45 ACP", 11.43, "g1", 0.185),
    "fnp45": (".45 ACP", 11.43, "g1", 0.185),
    "M1911": (".45 ACP", 11.43, "g1", 0.185),
    "USP45": (".45 ACP", 11.43, "g1", 0.185),
    "USP": (".45 ACP", 11.43, "g1", 0.185),
    # .22 LR
    "mk2": (".22 LR", 5.59, "g1", 0.125),
    "22LR": (".22 LR", 5.59, "g1", 0.125),
    # .50 AE
    "deagle": (".50 AE", 12.7, "g1", 0.200),
    "50AE": (".50 AE", 12.7, "g1", 0.200),
    # .357 Mag
    "python": (".357 Mag", 9.0, "g1", 0.160),
    "357": (".357 Mag", 9.0, "g1", 0.160),
    "mateba": (".357 Mag", 9.0, "g1", 0.160),
    "mp412": (".357 Mag", 9.0, "g1", 0.160),  # MP-412 REX
    # .50 Beowulf
    "50BW": (".50 Beowulf", 12.7, "g1", 0.200),
    # 12ga
    "12Gauge": ("12ga", 18.53, "g1", 0.110),
    "12gauge": ("12ga", 18.53, "g1", 0.110),
    # .50 BMG / 12.7mm
    "M107": (".50 BMG", 12.954, "g1", 1.050),
    "TAC50": (".50 BMG", 12.954, "g1", 1.050),
    "HECATE": (".50 BMG", 12.954, "g1", 1.050),
    "GM6": (".50 BMG", 12.954, "g1", 1.050),
    "127x99": (".50 BMG", 12.954, "g1", 1.050),
    "127x108": ("12.7x108mm", 12.954, "g1", 0.650),
    # Magnum calibers
    "338": (".338", 8.585, "g7", 0.310),
    "408": (".408", 10.363, "g7", 0.430),
    "93x64": ("9.3mm", 9.3, "g7", 0.280),
    "9_3": ("9.3mm", 9.3, "g7", 0.280),
    # RHS & remaining weapons
    "XM2010": (".300 Win Mag", 7.82, "g7", 0.310),
    "T5000": (".338 LM", 8.585, "g7", 0.310),
    "M590": ("12ga", 18.53, "g1", 0.110),
    "MP44": ("7.92mm", 7.92, "g7", 0.200),
    "KAR98": ("7.92mm", 7.92, "g7", 0.200),
    "STGW57": ("7.62mm", 7.62, "g7", 0.200),
    "M1garand": ("7.62mm", 7.62, "g7", 0.200),
    "L1A1": ("7.62mm", 7.62, "g7", 0.243),
    "SAVZ58": ("7.62mm", 7.62, "g7", 0.200),
    "M70": ("7.62mm", 7.62, "g7", 0.200),
    "M76": ("7.62mm", 7.62, "g7", 0.200),
    "M84": ("7.62mm", 7.62, "g7", 0.200),
    "M38": ("7.62mm", 7.62, "g7", 0.200),
    "Mosin": ("7.62mm", 7.62, "g7", 0.200),
    "SVD": ("7.62mm", 7.62, "g7", 0.200),
    "SVDS": ("7.62mm", 7.62, "g7", 0.200),
    "PSG1": ("7.62mm", 7.62, "g7", 0.243),
    "SG542": ("7.62mm", 7.62, "g7", 0.200),
    "MSG90": ("7.62mm", 7.62, "g7", 0.243),
    "VHSD2": ("5.56mm", 5.56, "g1", 0.151),
    "SG550": ("5.56mm", 5.56, "g1", 0.151),
    "SG551": ("5.56mm", 5.56, "g1", 0.151),
    "M3A1": (".45 ACP", 11.43, "g1", 0.185),
    "CZ99": ("9mm", 9.0, "g1", 0.160),
    "PYA": ("9mm", 9.0, "g1", 0.160),
    "PM": ("9mm", 9.0, "g1", 0.160),  # Makarov PM
    "P07": ("9mm", 9.0, "g1", 0.160),
    "Rook40": ("9mm", 9.0, "g1", 0.160),
    "ACPC2": (".45 ACP", 11.43, "g1", 0.185),
    "M24": ("7.62mm", 7.62, "g7", 0.243),
    "M14": ("7.62mm", 7.62, "g7", 0.243),
    "M21": ("7.62mm", 7.62, "g7", 0.243),
    "M1": ("7.62mm", 7.62, "g7", 0.200),
    "ASVAL": ("9x39mm", 9.0, "g7", 0.220),
    "VSS": ("9x39mm", 9.0, "g7", 0.220),
    "VHSK2": ("5.56mm", 5.56, "g1", 0.151),
    "KSG": ("12ga", 18.53, "g1", 0.110),
    "aa40": ("12ga", 18.53, "g1", 0.110),
    "HunterShotgun": ("12ga", 18.53, "g1", 0.110),
    "Pistol_heavy_01": (".45 ACP", 11.43, "g1", 0.185),
    "Pistol_heavy_02": ("9mm", 9.0, "g1", 0.160),
    "Pistol_01": ("9mm", 9.0, "g1", 0.160),
    # RH obscure
    "hb": ("5.56mm", 5.56, "g1", 0.151),  # RH HBAR variants
    "ttracker": (".357 Mag", 9.0, "g1", 0.160),
    "bull": (".50 AE", 12.7, "g1", 0.200),  # Desert Bull / BFR
}

WEAPON_PREFIX_MAP = {
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
}


# ── HPP Parser ────────────────────────────────────────────────────────────
class HPPParser:
    """Parse SQF/HPP config file extracting weapon ACE parameters.

    Uses brace-depth counting to track class nesting, handling all SQF
    edge cases: forward declarations, empty inline classes, macro class
    names (GVAR, EGVAR, QPATHTOF), and multi-line array values.
    """

    _CLS = r"[^{\s};:/]+"

    def parse(self, text: str):
        """Return {class_name: {_parent, _props: {}}} dict."""
        tree = {}
        # class_stack[depth] holds the dict node at that brace depth
        class_stack = [tree]  # depth 0 = root tree
        depth = 0

        for line in text.split("\n"):
            raw = line.split("//")[0]
            stripped = raw.strip()
            if not stripped:
                continue

            opens = stripped.count("{")
            closes = stripped.count("}")

            # ── Class detection ──────────────────────────────────────────────
            m = re.match(
                r"^class\s+("
                + self._CLS
                + r")\s*(?::\s*("
                + self._CLS
                + r"))?\s*\{?\s*$",
                stripped,
            )
            if m:
                name, parent = m.group(1), m.group(2) or ""
                node = {"_parent": parent, "_props": {}}
                # Guard against depth exceeding stack
                if depth >= len(class_stack):
                    class_stack.extend([{}] * (depth + 1 - len(class_stack)))
                if isinstance(class_stack[depth], dict):
                    class_stack[depth][name] = node
                # Pre-set class_stack at depth+1 for the upcoming scope,
                # WITHOUT incrementing depth — the brace counter below
                # handles that from opens/closes.
                if opens > 0:
                    new_depth = depth + 1
                    if len(class_stack) <= new_depth:
                        class_stack.append(node)
                    else:
                        class_stack[new_depth] = node

            # ── Properties ───────────────────────────────────────────────────
            pm = re.match(r"^([\w\[\]]+)\s*(\+=|=)\s*(.+?)\s*;\s*$", stripped)
            if pm and depth > 0 and depth < len(class_stack):
                key = pm.group(1).rstrip("[").rstrip("]")
                key, op, value = pm.group(1), pm.group(2), pm.group(3).strip()
                if key.endswith("[]"):
                    key = key[:-2]
                props = class_stack[depth].setdefault("_props", {})
                parsed_val = self._parse_value(value)
                if op == "+=":
                    existing = props.get(key, [])
                    if not isinstance(existing, list):
                        existing = [existing]
                    if isinstance(parsed_val, list):
                        existing.extend(parsed_val)
                    else:
                        existing.append(parsed_val)
                    props[key] = existing
                else:
                    props[key] = parsed_val

            # ── Brace depth adjustment ───────────────────────────────────────
            old_depth = depth
            depth += opens - closes
            depth = max(0, depth)

        return tree

    def _parse_value(self, value: str):
        """Parse an SQF value string -> Python int/float/list/str."""
        value = value.strip()

        # SQF array: { ... }
        if value.startswith("{") and value.endswith("}"):
            inner = value[1:-1].strip()
            if not inner:
                return []
            elements = self._split_sqf_array(inner)
            return [self._parse_value(e) for e in elements]

        # Quoted string
        if value.startswith('"') and value.endswith('"'):
            return value[1:-1]

        # Macro call
        if value.startswith("CSTRING(") or value.startswith("ECSTRING("):
            return value

        # Numeric (including negative)
        try:
            return float(value) if "." in value or "e" in value.lower() else int(value)
        except ValueError:
            return value

    def _split_sqf_array(self, inner: str):
        """Split SQF array content into elements, respecting nested braces and quotes."""
        elements = []
        depth = 0
        current = []
        in_str = False
        str_char = None

        for ch in inner:
            if in_str:
                current.append(ch)
                if ch == str_char:
                    in_str = False
                continue
            if ch in ('"', "'"):
                in_str = True
                str_char = ch
                current.append(ch)
                continue
            if ch == "{":
                depth += 1
                current.append(ch)
                continue
            if ch == "}":
                depth -= 1
                if depth < 0:
                    break
                current.append(ch)
                continue
            if ch == "," and depth == 0:
                elements.append("".join(current).strip())
                current = []
                continue
            current.append(ch)

        if current:
            elements.append("".join(current).strip())
        return elements

    def parse_file(self, path):
        with open(path) as f:
            return self.parse(f.read())


# ── Ammo Database ─────────────────────────────────────────────────────────
def build_ammo_db(hpp_tree):
    """Build ammo class → {caliber, mass, BCs, dragModel, mvCurve, blCurve}"""
    ammo = {}
    cfg_ammo = hpp_tree.get("CfgAmmo", {})
    for cls_name, cls_data in cfg_ammo.items():
        if cls_name.startswith("_"):
            continue
        props = cls_data.get("_props", {})
        ace_bl = props.get("ACE_barrelLengths", [])
        ace_mv = props.get("ACE_muzzleVelocities", [])

        if not ace_bl or not ace_mv:
            continue  # no ACE velocity curve

        ammo[cls_name] = {
            "caliber_mm": props.get("ACE_caliber", 0),
            "bullet_mass_g": props.get("ACE_bulletMass", 0),
            "bcs": props.get("ACE_ballisticCoefficients", []),
            "drag_model": props.get("ACE_dragModel", 7),
            "barrel_lengths_mm": [float(b) for b in ace_bl],
            "muzzle_velocities_ms": [float(v) for v in ace_mv],
        }
    return ammo


def build_mag_ammo_map(hpp_tree):
    """Build magazine class → ammo class map."""
    mag_map = {}
    cfg_mags = hpp_tree.get("CfgMagazines", {})
    for cls_name, cls_data in cfg_mags.items():
        if cls_name.startswith("_"):
            continue
        ammo_cls = cls_data.get("_props", {}).get("ammo", "")
        if ammo_cls:
            mag_map[cls_name] = (
                str(ammo_cls) if not isinstance(ammo_cls, str) else ammo_cls
            )
    return mag_map


def build_well_mag_map(hpp_tree):
    """Build mag well name → [mag classes] map."""
    well_map = {}
    cfg_wells = hpp_tree.get("CfgMagazineWells", {})
    for well_name, well_data in cfg_wells.items():
        if well_name.startswith("_"):
            continue
        mags = []
        for key, val in well_data.items():
            if key.startswith("_"):
                continue
            if isinstance(val, dict):
                addon_val = val.get("_props", {}).get("ADDON", [])
                if isinstance(addon_val, list):
                    mags.extend(
                        str(m) if not isinstance(m, str) else m for m in addon_val
                    )
                elif isinstance(addon_val, str):
                    mags.append(addon_val)
            elif isinstance(val, list):
                mags.extend(val)
        if mags:
            well_map[well_name] = mags
    return well_map


# ── Weapon Extraction ─────────────────────────────────────────────────────
def extract_weapons(hpp_tree, source_name="unknown"):
    """Extract weapon classes with ACE parameters from a CfgWeapons tree."""
    weapons = {}
    cfg_wpns = hpp_tree.get("CfgWeapons", {})
    for cls_name, cls_data in cfg_wpns.items():
        if cls_name.startswith("_"):
            continue
        props = cls_data.get("_props", {})
        barrel_length = props.get("ACE_barrelLength", 0)
        if not barrel_length:
            continue
        barrel_twist = props.get("ACE_barrelTwist", 0)
        init_speed = props.get("initSpeed", 0)

        # Extract magazine well references
        mag_wells = []
        for k, v in props.items():
            if k.startswith("magazineWell") or k == "magazineWell":
                if isinstance(v, list):
                    mag_wells.extend(str(x) if not isinstance(x, str) else x for x in v)
                elif isinstance(v, str):
                    mag_wells.append(v)

        weapons[cls_name] = {
            "barrel_length_mm": float(barrel_length) if barrel_length else 0,
            "barrel_twist_mm": float(barrel_twist) if barrel_twist else 0,
            "initSpeed": float(init_speed) if init_speed else 0,
            "mag_wells": mag_wells,
            "source": source_name,
            "parent": cls_data.get("_parent", ""),
        }
    return weapons


def _strip_prefix(name):
    """Strip common mod prefixes from weapon class name."""
    for prefix in MOD_PREFIXES:
        if name.lower().startswith(prefix):
            rest = name[len(prefix) :]
            return rest, prefix
    return name, None


def infer_caliber(weapon_class):
    """Infer caliber from weapon class name using CALIBER_MAP.

    Strips common mod prefixes (R3F_, RH_, gm_, etc.) before matching
    so `R3F_HK416M` matches the `HK416` token.
    """
    stripped, prefix = _strip_prefix(weapon_class)

    # Match against stripped name first (higher priority)
    for token, (label, cal, drag, bc) in CALIBER_MAP.items():
        if token.lower() in stripped.lower():
            return {
                "label": label,
                "caliber_mm": cal,
                "cdm_id": f"g{drag}" if drag else "g7",
                "bc": bc,
            }

    # Fallback: match against full class name
    for token, (label, cal, drag, bc) in CALIBER_MAP.items():
        if token.lower() in weapon_class.lower():
            return {
                "label": label,
                "caliber_mm": cal,
                "cdm_id": f"g{drag}" if drag else "g7",
                "bc": bc,
            }

    # Try regex patterns (e.g. _556x45, _762x51, _9x19)
    m = re.search(r"_(\d+)x(\d+)", weapon_class)
    if m:
        cal = float(m.group(1)) / 10 if float(m.group(1)) > 100 else float(m.group(1))
        return {"label": f"{cal:.1f}mm", "caliber_mm": cal, "cdm_id": "g7", "bc": 0.200}
    return None


def interpolate_mv(barrel_length_mm, bl_curve, mv_curve):
    """Interpolate muzzle velocity at given barrel length from ammo curve."""
    if not bl_curve or not mv_curve:
        return 0
    if len(bl_curve) != len(mv_curve):
        return mv_curve[0] if mv_curve else 0
    if barrel_length_mm <= bl_curve[0]:
        return mv_curve[0]
    if barrel_length_mm >= bl_curve[-1]:
        return mv_curve[-1]
    for i in range(len(bl_curve) - 1):
        if bl_curve[i] <= barrel_length_mm <= bl_curve[i + 1]:
            frac = (barrel_length_mm - bl_curve[i]) / (bl_curve[i + 1] - bl_curve[i])
            return mv_curve[i] + frac * (mv_curve[i + 1] - mv_curve[i])
    return mv_curve[0]


# ── JSON Generation ────────────────────────────────────────────────────────
def write_weapon_json(
    class_name,
    barrel_length,
    barrel_twist,
    caliber,
    ammo_data,
    source,
    init_speed=0,
):
    """Write a weapon JSON file."""
    # Determine weapon type directory
    weapon_type = None
    for prefix, wtype in WEAPON_PREFIX_MAP.items():
        if class_name.lower().startswith(prefix):
            weapon_type = wtype
            break
    if not weapon_type:
        weapon_type = "rifles"  # default

    target_dir = WEAPON_DIRS.get(weapon_type, DATA_DIR / "weapons" / weapon_type)
    target_dir.mkdir(parents=True, exist_ok=True)
    file_path = target_dir / f"{class_name}.json"

    # Check if already exists
    if file_path.exists():
        return False

    # Compute MV from ammo curve
    mv = 0
    if ammo_data and ammo_data.get("barrel_lengths_mm"):
        mv = interpolate_mv(
            barrel_length,
            ammo_data["barrel_lengths_mm"],
            ammo_data["muzzle_velocities_ms"],
        )

    # Fallback MV if interpolation failed
    if not mv and ammo_data and ammo_data.get("muzzle_velocities_ms"):
        mv = ammo_data["muzzle_velocities_ms"][0]

    # Fallback to ACE3 initSpeed if no ammo data
    if not mv and init_speed:
        mv = init_speed

    # Caliber/BC fallback
    cal_mm = caliber.get("caliber_mm", 5.56) if caliber else 5.56
    cdm = caliber.get("cdm_id", "g7") if caliber else "g7"
    bc = caliber.get("bc", 0.200) if caliber else 0.200

    # Get mass from ammo data or estimate
    mass_g = ammo_data.get("bullet_mass_g", 0) if ammo_data else 0
    if not mass_g:
        # Estimate from caliber
        area_factor = (cal_mm / 5.56) ** 2
        mass_g = round(4.0 * area_factor * (bc / 0.151), 1)

    # Chamber pressure estimate
    chamber_pressure = 380
    if cal_mm <= 5.56:
        chamber_pressure = 380
    elif cal_mm <= 7.62:
        chamber_pressure = 380
    elif cal_mm <= 9.0:
        chamber_pressure = 250
    elif cal_mm <= 12.7:
        chamber_pressure = 400
    else:
        chamber_pressure = 350

    notes = f"Extracted from ACE3 {source}"
    if ammo_data:
        notes += f" via {list(ammo_data.keys())[0] if isinstance(ammo_data, dict) else ammo_data}"

    weapon_data = {
        "class": class_name,
        "caliber_mm": round(cal_mm, 3),
        "barrel_length_mm": round(barrel_length, 1),
        "rifling_twist_mm": round(barrel_twist, 1)
        if barrel_twist
        else round(barrel_length * 0.4, 1),
        "chamber_pressure_mpa": chamber_pressure,
        "cdm_id": cdm,
        "projectile_mass_g": round(mass_g, 2),
        "muzzle_velocity_ms": round(mv, 1) if mv else 800.0,
        "zero_range_m": 100,
        "effective_range_m": 800,
        "notes": notes,
    }

    with open(file_path, "w") as f:
        json.dump(weapon_data, f, indent=2)
    return True


def write_ammo_json(class_name, ammo_data):
    """Write an ammo JSON file (optional - data/ammo/)."""
    target_dir = AMMO_DIR
    target_dir.mkdir(parents=True, exist_ok=True)
    file_path = target_dir / f"{class_name}.json"

    if file_path.exists():
        return False

    cal_mm = ammo_data.get("caliber_mm", 0)
    mass_g = ammo_data.get("bullet_mass_g", 0)
    bcs = ammo_data.get("bcs", [0.200])
    drag = ammo_data.get("drag_model", 7)

    ammo_json = {
        "class": class_name,
        "caliber_mm": float(cal_mm) if cal_mm else 5.56,
        "projectile_mass_g": float(mass_g) if mass_g else 4.0,
        "cdm_id": f"g{int(drag)}" if drag else "g7",
        "ballistic_coefficient": float(bcs[0]) if bcs else 0.200,
        "notes": "Extracted from ACE3 ace_ballistics CfgAmmo.hpp",
    }

    with open(file_path, "w") as f:
        json.dump(ammo_json, f, indent=2)
    return True


# ── Main Pipeline ─────────────────────────────────────────────────────────
def find_compat_dirs():
    """Find all directories under /tmp/ACE3/addons/ with CfgWeapons.hpp."""
    compat = []
    for d in sorted(ACE3_DIR.glob("addons/*/")):
        if (d / "CfgWeapons.hpp").exists() and "ballistics" not in d.name:
            compat.append(d)
    return compat


def main():
    parser = HPPParser()

    # ═══ Phase 1: Parse ACE3 Ballistics Data ═══
    print("═" * 60)
    print("Phase 1: Parsing ACE3 ballistics data...")
    print("═" * 60)

    # Parse CfgAmmo.hpp
    ammo_tree = parser.parse_file(BALLISTICS_DIR / "CfgAmmo.hpp")
    ammo_db = build_ammo_db(ammo_tree)
    print(f"  CfgAmmo.hpp: {len(ammo_db)} ammo types with ACE velocity curves")

    # Parse CfgMagazines.hpp
    mag_tree = parser.parse_file(BALLISTICS_DIR / "CfgMagazines.hpp")
    mag_ammo_map = build_mag_ammo_map(mag_tree)
    print(f"  CfgMagazines.hpp: {len(mag_ammo_map)} magazine→ammo mappings")

    # Parse CfgMagazineWells.hpp
    well_tree = parser.parse_file(BALLISTICS_DIR / "CfgMagazineWells.hpp")
    well_mag_map = build_well_mag_map(well_tree)
    print(f"  CfgMagazineWells.hpp: {len(well_mag_map)} mag well definitions")

    # ═══ Phase 2: Parse Weapon Configs ═══
    print("\n" + "═" * 60)
    print("Phase 2: Parsing weapon configs...")
    print("═" * 60)

    all_weapons = {}

    # Ballistics addon (vanilla weapons)
    wpn_tree = parser.parse_file(BALLISTICS_DIR / "CfgWeapons.hpp")
    weapons = extract_weapons(wpn_tree, "ace_ballistics")
    all_weapons.update(weapons)
    print(f"  ballistics/CfgWeapons.hpp: {len(weapons)} weapons with ACE data")

    # Compat addons (CDLC/mod weapons)
    compat_dirs = find_compat_dirs()
    print(f"  Found {len(compat_dirs)} compat addons with CfgWeapons.hpp")

    compat_weapons = {}
    for d in compat_dirs:
        src_name = f"compat_{d.name}"
        wpn_tree = parser.parse_file(d / "CfgWeapons.hpp")
        wpns = extract_weapons(wpn_tree, src_name)
        compat_weapons.update(wpns)
        print(f"    {d.name}/CfgWeapons.hpp: {len(wpns)} weapons")

    all_weapons.update(compat_weapons)
    print(f"\n  Total weapons with ACE data: {len(all_weapons)}")

    # ═══ Phase 3: Cross-reference Weapons → Ammo ═══
    print("\n" + "═" * 60)
    print("Phase 3: Cross-referencing weapons to ammo...")
    print("═" * 60)

    # Build reverse map: caliber → [ammo classes]
    caliber_to_ammo = {}
    for ammo_cls, data in ammo_db.items():
        cal = data.get("caliber_mm", 0)
        if cal:
            cal_key = round(cal, 1)
            if cal_key not in caliber_to_ammo:
                caliber_to_ammo[cal_key] = []
            caliber_to_ammo[cal_key].append((ammo_cls, data))

    # For each weapon, find compatible ammo
    weapon_ammo_match = {}
    weapon_no_ammo = []

    for wpn_cls, wpn_data in all_weapons.items():
        cal_info = infer_caliber(wpn_cls)
        if not cal_info:
            weapon_no_ammo.append(wpn_cls)
            continue

        cal_key = round(cal_info["caliber_mm"], 1)
        candidates = caliber_to_ammo.get(cal_key, [])
        if not candidates:
            weapon_no_ammo.append(wpn_cls)
            continue

        # Pick the best matching ammo: prefer base ball ammo over specialty
        best = candidates[0]
        for ammo_cls, ammo_data in candidates:
            # Prefer the base ball ammo (not subsonic, not tracer dim)
            if (
                "Ball" in ammo_cls
                and "Subsonic" not in ammo_cls
                and "Tracer" not in ammo_cls
            ):
                best = (ammo_cls, ammo_data)
                break

        weapon_ammo_match[wpn_cls] = {
            "caliber": cal_info,
            "ammo": best[0],
            "ammo_data": best[1],
        }

    print(f"  Weapons matched to ammo: {len(weapon_ammo_match)}")
    print(f"  Weapons with no ammo match: {len(weapon_no_ammo)}")

    # ═══ Phase 4: Generate JSONs ═══
    print("\n" + "═" * 60)
    print("Phase 4: Generating ABE JSON files...")
    print("═" * 60)

    generated = 0
    skipped_existing = 0
    no_match = 0

    for wpn_cls, wpn_data in all_weapons.items():
        match = weapon_ammo_match.get(wpn_cls)
        if not match:
            # Try to generate with caliber info only (no ammo data)
            cal_info = infer_caliber(wpn_cls)
            if cal_info:
                if write_weapon_json(
                    wpn_cls,
                    wpn_data["barrel_length_mm"],
                    wpn_data["barrel_twist_mm"],
                    cal_info,
                    None,
                    wpn_data["source"],
                    init_speed=wpn_data.get("initSpeed", 0),
                ):
                    generated += 1
                    no_match += 1
                else:
                    skipped_existing += 1
            continue

        if write_weapon_json(
            wpn_cls,
            wpn_data["barrel_length_mm"],
            wpn_data["barrel_twist_mm"],
            match["caliber"],
            match["ammo_data"],
            wpn_data["source"],
            init_speed=wpn_data.get("initSpeed", 0),
        ):
            generated += 1
        else:
            skipped_existing += 1

    # Also generate ammo JSONs for ACE3-calibrated ammo
    ammo_generated = 0
    ammo_skipped = 0
    for ammo_cls, ammo_data in ammo_db.items():
        if write_ammo_json(ammo_cls, ammo_data):
            ammo_generated += 1
        else:
            ammo_skipped += 1

    print(f"\n  Weapon JSONs generated: {generated}")
    print(f"  Weapon JSONs skipped (already exist): {skipped_existing}")
    print(f"  Weapon JSONs with no ammo match: {no_match}")
    print(f"  Ammo JSONs generated: {ammo_generated}")
    print(f"  Ammo JSONs skipped (already exist): {ammo_skipped}")
    print(f"\n  No-ammo-match weapons ({len(weapon_no_ammo)} listed):")
    for w in weapon_no_ammo[:20]:
        print(f"    - {w}")
    if len(weapon_no_ammo) > 20:
        print(f"    ... and {len(weapon_no_ammo) - 20} more")

    print("\n" + "═" * 60)
    print("Done!")
    print("═" * 60)


if __name__ == "__main__":
    main()
