#!/usr/bin/env python3
"""
Parse ACE3 CfgAmmo.hpp config files and extract ballistics data.
Pulls ACE_caliber, ACE_bulletMass, ACE_ballisticCoefficients,
ACE_dragModel, ACE_muzzleVelocities, ACE_barrelLengths, etc.

Usage:
    python3 parse_ace_cfgammo_hpp.py <CfgAmmo.hpp> [CfgAmmo.hpp...]
"""

import json
import re
import sys
import os
from pathlib import Path

# Regex patterns for extracting ACE config values from HPP files
ACE_CALIBER = re.compile(r"ACE_caliber\s*=\s*([\d.]+)")
ACE_BULLET_MASS = re.compile(r"ACE_bulletMass\s*=\s*([\d.]+)")
ACE_BULLET_LENGTH = re.compile(r"ACE_bulletLength\s*=\s*([\d.]+)")
ACE_BC = re.compile(r"ACE_ballisticCoefficients\[\]\s*=\s*\{(.+?)\}", re.DOTALL)
ACE_DRAG_MODEL = re.compile(r"ACE_dragModel\s*=\s*(\d+)")
ACE_MV = re.compile(r"ACE_muzzleVelocities\[\]\s*=\s*\{(.+?)\}", re.DOTALL)
ACE_BL = re.compile(r"ACE_barrelLengths\[\]\s*=\s*\{(.+?)\}", re.DOTALL)
ACE_STD_ATM = re.compile(r'ACE_standardAtmosphere\s*=\s*"(\w+)"')
ACE_VEL_SD = re.compile(r"ACE_muzzleVelocityVariationSD\s*=\s*([\d.]+)")
ACE_PREFIX = re.compile(r"class\s+(\w+)\s*:\s*(\w+)\s*{")


def parse_hpp(filepath: str) -> dict:
    """Parse an ACE3 CfgAmmo.hpp file and extract ballistics entries."""
    with open(filepath) as f:
        content = f.read()

    # Find the CfgAmmo block
    m = re.search(r"class\s+CfgAmmo\s*{(.+)}", content, re.DOTALL)
    if not m:
        print(f"No CfgAmmo block found in {filepath}", file=sys.stderr)
        return {}

    cfg_content = m.group(1)

    results = {}
    # Find all class definitions inside CfgAmmo
    # We use a simpler approach: find all ACE_caliber assignments and get their parent class
    for m in re.finditer(r"class\s+(\w+)\s*:\s*(\w+)\s*{", cfg_content):
        class_name = m.group(1)
        parent_class = m.group(2)
        start = m.end()

        # Find the closing brace (simple brace counting)
        depth = 1
        pos = start
        while pos < len(cfg_content) and depth > 0:
            if cfg_content[pos] == "{":
                depth += 1
            elif cfg_content[pos] == "}":
                depth -= 1
            pos += 1
        block = cfg_content[start : pos - 1] if depth == 0 else cfg_content[start:]

        # Extract ACE fields from this block
        cal = ACE_CALIBER.search(block)
        mass = ACE_BULLET_MASS.search(block)
        length = ACE_BULLET_LENGTH.search(block)
        bc = ACE_BC.search(block)
        drag = ACE_DRAG_MODEL.search(block)
        mv = ACE_MV.search(block)
        bl = ACE_BL.search(block)
        atm = ACE_STD_ATM.search(block)
        sd = ACE_VEL_SD.search(block)

        if cal or mass or bc:
            entry = {
                "class": class_name,
                "parent": parent_class,
            }
            if cal:
                entry["caliber_mm"] = float(cal.group(1))
            if mass:
                entry["mass_g"] = float(mass.group(1))
            if length:
                entry["bullet_length_mm"] = float(length.group(1))
            if bc:
                vals = [float(x.strip()) for x in bc.group(1).split(",") if x.strip()]
                entry["bc_list"] = vals
            if drag:
                entry["drag_model"] = int(drag.group(1))
            if mv:
                vals = [float(x.strip()) for x in mv.group(1).split(",") if x.strip()]
                entry["mv_list"] = vals
            if bl:
                vals = [float(x.strip()) for x in bl.group(1).split(",") if x.strip()]
                entry["bl_list"] = vals
            if atm:
                entry["std_atmosphere"] = atm.group(1)
            if sd:
                entry["vel_sd"] = float(sd.group(1))

            results[class_name] = entry

    return results


def main():
    if len(sys.argv) < 2:
        print(f"Usage: {sys.argv[0]} <CfgAmmo.hpp> [more files...]")
        sys.exit(1)

    all_entries = {}
    for fp in sys.argv[1:]:
        entries = parse_hpp(fp)
        all_entries.update(entries)
        print(f"{fp}: {len(entries)} ballistics entries", file=sys.stderr)

    print(f"\nTotal: {len(all_entries)} unique ballistics entries", file=sys.stderr)

    # Output in same pipe format as SQF export for compatibility
    for name in sorted(all_entries.keys()):
        e = all_entries[name]
        bc_s = ",".join(str(b) for b in e.get("bc_list", []))
        mv_s = ",".join(str(v) for v in e.get("mv_list", []))
        bl_s = ",".join(str(v) for v in e.get("bl_list", []))
        print(
            f"HPP|{name}|{e.get('caliber_mm', '')}|{e.get('mass_g', '')}|{e.get('bullet_length_mm', '')}|{e.get('drag_model', '')}|{e.get('std_atmosphere', '')}|{e.get('vel_sd', '')}|{bc_s}|{mv_s}|{bl_s}|{e.get('parent', '')}"
        )


if __name__ == "__main__":
    main()
