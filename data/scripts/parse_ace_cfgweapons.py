#!/usr/bin/env python3
"""
Parse ACE3 CfgWeapons.hpp config files and extract barrel length + twist data.
Outputs pipe-delimited format compatible with parse_sqf_export.py's --ace-weapons.

Usage:
    python3 parse_ace_cfgweapons.py <CfgWeapons.hpp> [more files...]
"""

import json
import re
import sys
from collections import defaultdict

ACE_BARREL_LENGTH = re.compile(
    r"(?:ace_barrelLength|AC[E]_barrelLength|modelLength)\s*=\s*([\d.]+)"
)
ACE_BARREL_TWIST = re.compile(r"(?:ace_barrelTwist|AC[E]_barrelTwist)\s*=\s*([\d.]+)")
ACE_CLASS = re.compile(r"class\s+(\w+)\s*:\s*(\w+)\s*{")
ACE_MODEL_LENGTH = re.compile(r"modelLength\s*=\s*([\d.]+)")


def parse_cfgweapons_hpp(filepath: str, prefix: str = "") -> dict:
    """Parse an ACE3 CfgWeapons.hpp file and extract weapon barrel data."""
    with open(filepath) as f:
        content = f.read()

    # Handle #include directives by expanding them
    base_dir = "/".join(filepath.split("/")[:-1]) or "."
    while True:
        m = re.search(r'#include\s+"([^"]+)"', content)
        if not m:
            break
        inc_file = m.group(1)
        inc_path = f"{base_dir}/{inc_file}"
        try:
            with open(inc_path) as inc_f:
                inc_content = inc_f.read()
                content = content[: m.start()] + inc_content + content[m.end() :]
        except FileNotFoundError:
            print(f"  [WARN] #include not found: {inc_path}", file=sys.stderr)
            content = content[: m.start()] + content[m.end() :]

    # Find the CfgWeapons block
    m = re.search(r"class\s+CfgWeapons\s*{(.+)}", content, re.DOTALL)
    if not m:
        print(f"No CfgWeapons block found in {filepath}", file=sys.stderr)
        return {}

    cfg_content = m.group(1)

    results = {}
    for m in re.finditer(r"class\s+(\w+)\s*:\s*(\w+)\s*{", cfg_content):
        class_name = m.group(1)
        parent_class = m.group(2)
        start = m.end()

        depth = 1
        pos = start
        while pos < len(cfg_content) and depth > 0:
            if cfg_content[pos] == "{":
                depth += 1
            elif cfg_content[pos] == "}":
                depth -= 1
            pos += 1
        block = cfg_content[start : pos - 1] if depth == 0 else cfg_content[start:]

        bl = ACE_BARREL_LENGTH.search(block)
        bt = ACE_BARREL_TWIST.search(block)
        ml = ACE_MODEL_LENGTH.search(block)

        if bl or bt or ml:
            entry = {
                "class": class_name,
                "parent": parent_class,
                "source": prefix + filepath.split("/")[-1],
            }
            if bl:
                entry["barrel_length_mm"] = float(bl.group(1))
            if bt:
                entry["barrel_twist_mm"] = float(bt.group(1))
            if ml:
                entry["model_length_m"] = float(ml.group(1))
            results[class_name] = entry

    return results


def main():
    if len(sys.argv) < 2:
        print(f"Usage: {sys.argv[0]} <CfgWeapons.hpp> [more files...]")
        sys.exit(1)

    all_entries = {}
    for fp in sys.argv[1:]:
        # Determine prefix for source tracking
        prefix = ""
        for pfx in ["gm_", "lxWS_", "rf_", "SPE_", "sog_", "csla_", "vn_"]:
            if pfx in fp.lower():
                prefix = pfx
                break
        entries = parse_cfgweapons_hpp(fp, prefix)
        all_entries.update(entries)
        print(f"{fp}: {len(entries)} weapons with barrel data", file=sys.stderr)

    print(f"\nTotal: {len(all_entries)} unique weapon entries", file=sys.stderr)

    # Output: ACEW|class|parent|barrelLength_mm|barrelTwist_mm|source
    for name in sorted(all_entries.keys()):
        e = all_entries[name]
        bl = e.get("barrel_length_mm", "")
        bt = e.get("barrel_twist_mm", "")
        ml = e.get("model_length_m", "")
        print(f"ACEW|{name}|{e.get('parent', '')}|{bl}|{bt}|{ml}|{e.get('source', '')}")


if __name__ == "__main__":
    main()
