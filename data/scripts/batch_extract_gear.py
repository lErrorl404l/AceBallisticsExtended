#!/usr/bin/env python3
"""
Batch gear extraction across base Arma 3 Addons + all workshop mods.

Scans every mod's addons directory for .pbo files, checks each for
wearable items via a quick config scan, and runs full extraction on
those that have any.

Usage:
    python3 data/scripts/batch_extract_gear.py
    python3 data/scripts/batch_extract_gear.py --base-only   # base game only
    python3 data/scripts/batch_extract_gear.py --workshop-only  # workshop only
"""

import json
import os
import re
import subprocess
import sys
import time
import xml.etree.ElementTree as ET
from collections import defaultdict
from pathlib import Path

# ── Config ──────────────────────────────────────────────────────────────────────
BASE_ADDONS = Path("/ext/SteamLibrary/steamapps/common/Arma 3/Addons")
WORKSHOP_DIR = Path("/ext/SteamLibrary/steamapps/workshop/content/107410")
OUTPUT_DIR = Path(__file__).resolve().parent / "extracted"
CLASSIFY_SCRIPT = Path(__file__).resolve().parent / "classify_gear_armor.py"
ARMAGE_BIN = "armake"

# Gear type patterns to look for in config text (quick filter)
GEAR_PARENTS = {
    "Vest_Base_F",
    "Vest_Camo_Base",
    "Vest_NoCamo_Base",
    "Headgear_Base_F",
    "H_Helmet_Base_F",
    "H_HelmetB",
    "Uniform_Base",
    "Glasses_Base_F",
    "G_Glasses_Base_F",
    "ItemCore",
}


# ── PBO reader (from extract_gear_from_pbo.py) ──────────────────────────────
def read_pbo(pbo_path: str) -> dict:
    with open(pbo_path, "rb") as f:
        data = f.read()
    n = len(data)
    pos = 0
    result = {}

    def read_asciiz(offset):
        end = data.find(b"\0", offset)
        if end < 0:
            return "", offset
        return data[offset:end].decode("ascii", errors="replace"), end + 1

    # Version entry
    if pos + 21 > n:
        raise ValueError(f"Too small: {pbo_path}")
    filename, pos = read_asciiz(pos)
    import struct

    (packing, orig_size, reserved, timestamp, data_size) = struct.unpack_from(
        "<IIIII", data, pos
    )
    pos += 20

    srev_magic = struct.unpack("<I", b"sreV")[0]
    if packing != srev_magic:
        for scan_pos in range(min(16, n)):
            p = data[scan_pos : scan_pos + 4]
            if len(p) == 4 and struct.unpack("<I", p)[0] == srev_magic:
                return _read_pbo_raw(data, scan_pos, pbo_path)
            if scan_pos + 4 > n:
                break
        raise ValueError(f"Bad version magic: {pbo_path}")

    # Header properties
    properties = {}
    while pos < n:
        key, pos = read_asciiz(pos)
        if not key:
            break
        value, pos = read_asciiz(pos)
        properties[key] = value

    # File entry headers
    file_entries = []
    while pos < n:
        filename, pos = read_asciiz(pos)
        if not filename:
            break
        if pos + 20 > n:
            break
        (packing, orig_size, reserved, timestamp, data_size) = struct.unpack_from(
            "<IIIII", data, pos
        )
        pos += 20
        file_entries.append(
            {
                "name": filename,
                "packing": packing,
                "orig_size": orig_size,
                "data_size": data_size,
            }
        )

    pos += 20  # skip terminator

    for entry in file_entries:
        if pos + entry["data_size"] > n:
            break
        result[entry["name"]] = data[pos : pos + entry["data_size"]]
        pos += entry["data_size"]

    return {
        "files": result,
        "properties": properties,
        "prefix": properties.get("prefix", ""),
    }


def _read_pbo_raw(data: bytes, offset: int, pbo_path: str) -> dict:
    import struct

    n, pos = len(data), offset

    def ra(off):
        end = data.find(b"\0", off)
        if end < 0:
            return "", off
        return data[off:end].decode("ascii", errors="replace"), end + 1

    filename, pos = ra(pos)
    (packing, _osz, _rsv, _ts, _ds) = struct.unpack_from("<IIIII", data, pos)
    pos += 20
    if struct.unpack("<I", b"sreV")[0] != packing:
        raise ValueError(f"Bad version magic at {offset}: {pbo_path}")

    properties = {}
    while pos < n:
        key, pos = ra(pos)
        if not key:
            break
        val, pos = ra(pos)
        properties[key] = val

    file_entries = []
    while pos < n:
        fn, pos = ra(pos)
        if not fn:
            break
        if pos + 20 > n:
            break
        (p, osz, rsv, ts, dsz) = struct.unpack_from("<IIIII", data, pos)
        pos += 20
        file_entries.append({"name": fn, "packing": p, "data_size": dsz})

    result = {}
    for entry in file_entries:
        if pos + entry["data_size"] > n:
            break
        result[entry["name"]] = data[pos : pos + entry["data_size"]]
        pos += entry["data_size"]
    return {
        "files": result,
        "properties": properties,
        "prefix": properties.get("prefix", ""),
    }


def derapify_bytes(bin_data: bytes) -> str | None:
    """Derapify binary config via armake."""
    rap_pos = -1
    search_limit = min(4096, len(bin_data) - 4)
    for off in range(search_limit):
        if bin_data[off] == 0 and bin_data[off + 1 : off + 4] == b"raP":
            rap_pos = off
            break
    if rap_pos < 0:
        return None
    clean_data = bin_data[rap_pos:]
    result = subprocess.run(
        ["armake", "derapify"],
        input=clean_data,
        capture_output=True,
        timeout=30,
    )
    if result.returncode != 0:
        return None
    return result.stdout.decode("utf-8", errors="replace")


def _load_stringtable(pbo_data: dict) -> dict[str, str]:
    """Load stringtable.xml from a PBO's files if present.

    Returns {lowercase_key: english_text} mapping.
    """
    if "stringtable.xml" not in pbo_data.get("files", {}):
        return {}
    try:
        root = ET.fromstring(pbo_data["files"]["stringtable.xml"])
        result = {}
        for key in root.findall(".//Key"):
            kid = key.get("ID", "")
            eng = key.find("English")
            if kid and eng is not None and eng.text:
                result[kid.lower()] = eng.text
        return result
    except Exception:
        return {}


def pbo_has_wearables(pbo_path: str) -> bool:
    """Quick check if a PBO has wearable items by scanning config."""
    try:
        pbo_data = read_pbo(pbo_path)
    except Exception:
        return False

    entries = pbo_data["files"]
    config_key = None
    for name in (
        "config.bin",
        "config.cpp",
        "config.hpp",
        "Config.bin",
        "Config.cpp",
        "Config.hpp",
    ):
        if name in entries:
            config_key = name
            break
    if not config_key:
        for name in entries:
            if "config" in name.lower():
                config_key = name
                break
    if not config_key:
        return False

    config_data = entries[config_key]

    # Detect text config
    is_text = False
    if len(config_data) > 16:
        peek = config_data[:16]
        if (peek[0:1] != b"\x00" and peek[4:7] != b"raP") and (
            peek[:5] == b"class" or peek[:2] == b"/*" or peek[:2] == b"//"
        ):
            is_text = True

    try:
        if is_text:
            text = config_data.decode("utf-8", errors="replace")
        else:
            text = derapify_bytes(config_data)
    except Exception:
        return False

    if not text:
        return False

    # Broad scan for wearable items — checks multiple patterns:
    # 1. Known gear base classes (Vest_Base_F, etc.)
    # 2. ItemInfo blocks with wearable type markers (ItemInfo: VestItem)
    # 3. CfgGlasses sections
    text_lower = text.lower()
    for parent in GEAR_PARENTS:
        if parent.lower() in text_lower:
            return True

    # Check for ItemInfo: {VestItem|HeadgearItem|UniformItem|GlassesItem}
    import re

    if re.search(
        r"class\s+ItemInfo\s*:\s*(VestItem|HeadgearItem|UniformItem|GlassesItem)",
        text,
        re.IGNORECASE,
    ):
        return True

    # Check for CfgGlasses section
    if re.search(r"class\s+CfgGlasses\s*\{", text, re.IGNORECASE):
        return True

    return False


def find_pbo_dirs(args) -> list[tuple[str, str]]:
    """Find (pbo_path, source_label) pairs to process."""
    pairs = []
    if not args.workshop_only:
        print(f"Scanning base Addons: {BASE_ADDONS}")
        if BASE_ADDONS.is_dir():
            for f in sorted(os.listdir(BASE_ADDONS)):
                if f.endswith(".pbo"):
                    pairs.append((os.path.join(BASE_ADDONS, f), "base"))
    if not args.base_only:
        print(f"Scanning workshop mods...")
        if WORKSHOP_DIR.is_dir():
            for mod_id in sorted(os.listdir(WORKSHOP_DIR)):
                for addon_dir_name in ("addons", "Addons"):
                    addon_dir = WORKSHOP_DIR / mod_id / addon_dir_name
                    if addon_dir.is_dir():
                        for f in sorted(os.listdir(addon_dir)):
                            if f.endswith(".pbo"):
                                pairs.append((os.path.join(addon_dir, f), mod_id))
                        break  # found one addon dir
    print(f"Total PBOs found: {len(pairs)}")
    return pairs


def extract_config_text(pbo_path: str) -> str | None:
    """Get config text from a PBO (returns None on failure)."""
    try:
        pbo_data = read_pbo(pbo_path)
    except Exception:
        return None
    entries = pbo_data["files"]
    config_key = None
    for name in (
        "config.bin",
        "config.cpp",
        "config.hpp",
        "Config.bin",
        "Config.cpp",
        "Config.hpp",
    ):
        if name in entries:
            config_key = name
            break
    if not config_key:
        for name in entries:
            if "config" in name.lower():
                config_key = name
                break
    if not config_key:
        return None
    config_data = entries[config_key]
    is_text = False
    if len(config_data) > 16:
        peek = config_data[:16]
        if (peek[0:1] != b"\x00" and peek[4:7] != b"raP") and (
            peek[:5] == b"class" or peek[:2] == b"/*" or peek[:2] == b"//"
        ):
            is_text = True
    try:
        if is_text:
            return config_data.decode("utf-8", errors="replace")
        else:
            return derapify_bytes(config_data)
    except Exception:
        return None


def main():
    import argparse

    parser = argparse.ArgumentParser(description="Batch gear extraction")
    parser.add_argument("--base-only", action="store_true", help="Base game only")
    parser.add_argument(
        "--workshop-only", action="store_true", help="Workshop mods only"
    )
    parser.add_argument(
        "--quick-scan",
        action="store_true",
        help="Only scan PBOs for wearables (no full extraction)",
    )
    args = parser.parse_args()

    os.makedirs(OUTPUT_DIR, exist_ok=True)

    # Phase 1: find & filter PBOs
    pbo_pairs = find_pbo_dirs(args)
    # Dedup by filename (same PBO may exist in multiple mods)
    seen_pbo = set()
    unique_pairs = []
    for path, src in pbo_pairs:
        basename = os.path.basename(path)
        if basename not in seen_pbo:
            seen_pbo.add(basename)
            unique_pairs.append((path, src))
    print(f"Unique PBOs: {len(unique_pairs)} (deduplicated from {len(pbo_pairs)})")

    # Phase 2: quick-scan for wearable items
    print("\nScanning for wearable items... (this takes a while)")
    wearable_pbos = []
    for path, src in unique_pairs:
        if pbo_has_wearables(path):
            wearable_pbos.append((path, src))
            print(f"  ✓ {os.path.basename(path):50s}  [{src}]")

    print(
        f"\nFound {len(wearable_pbos)} PBOs with wearable items out of {len(unique_pairs)} total"
    )

    if args.quick_scan:
        with open(OUTPUT_DIR / "wearable_pbo_list.txt", "w") as f:
            for path, src in wearable_pbos:
                f.write(f"{path}\t{src}\n")
        print(f"List saved to {OUTPUT_DIR / 'wearable_pbo_list.txt'}")
        return

    # Phase 3: now import and run the real extraction logic
    print(f"\nRunning full extraction on {len(wearable_pbos)} PBOs...")

    # Import the extract functions
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    from extract_gear_from_pbo import (
        parse_gear_from_hpp,
        parse_glasses_from_hpp,
        resolve_inheritance,
        extract_rvmat_model_mappings,
    )

    all_items = []
    all_rvmat_mappings = {}
    processed = 0
    failed = 0

    for pbo_path, src in wearable_pbos:
        pbo_name = Path(pbo_path).stem
        print(f"  [{processed + 1}/{len(wearable_pbos)}] {pbo_name} [{src}]...", end="")

        config_text = extract_config_text(pbo_path)
        if not config_text:
            print(" no config")
            failed += 1
            processed += 1
            continue

        # Load stringtable from the same PBO for displayName resolution
        try:
            pbo_data = read_pbo(pbo_path)
            pbo_stringtable = _load_stringtable(pbo_data)
        except Exception:
            pbo_stringtable = None

        try:
            items = parse_gear_from_hpp(config_text, pbo_name, pbo_stringtable)
            glasses = parse_glasses_from_hpp(config_text, pbo_name, pbo_stringtable)
            all_items.extend(items)
            all_items.extend(glasses)
            mappings = extract_rvmat_model_mappings(config_text)
            all_rvmat_mappings.update(mappings)
            print(f" {len(items)} gear + {len(glasses)} glasses")
        except Exception as e:
            print(f" error: {e}")
            failed += 1

        processed += 1

    # Resolve inheritance and deduplicate
    resolve_inheritance(all_items)

    seen = set()
    unique_items = []
    for item in all_items:
        if item["classname"] not in seen and item.get("scope", 0) == 2:
            seen.add(item["classname"])
            unique_items.append(item)

    by_type = defaultdict(int)
    for item in unique_items:
        by_type[item["itemType"]] += 1

    print(f"\n{'=' * 60}")
    print(f"TOTAL: {len(unique_items)} unique public wearable items extracted")
    print(f"By type: {dict(by_type)}")
    print(f"Failed PBOs: {failed}/{processed}")
    print(f"{'=' * 60}")

    # Save outputs
    output_json = OUTPUT_DIR / "gear_extracted_batch.json"
    with open(output_json, "w") as f:
        json.dump(
            {
                "meta": {
                    "count": len(unique_items),
                    "by_type": dict(by_type),
                    "rvmat_mappings": len(all_rvmat_mappings),
                },
                "items": unique_items,
                "rvmat_model_mappings": all_rvmat_mappings,
            },
            f,
            indent=2,
        )
    print(f"Saved: {output_json}")

    # Save as gear_dump format
    output_txt = OUTPUT_DIR / "gear_dump_batch.txt"
    with open(output_txt, "w") as f:
        for item in unique_items:
            hp_str = "~".join(
                [
                    f"{hp['name']}:{hp['armor']}:{hp['passThrough']}:{hp['hitpointName']}"
                    for hp in item.get("hitpoints", [])
                ]
            )
            uniform_info = ""
            if item["itemType"] == "UniformItem":
                uniform_info = f"{item.get('uniformClass', '')}|"
            dn = item.get("displayName", "").replace("|", " ")
            f.write(
                f"G|{item['classname']}|{dn}|"
                f"{item['itemType']}|{item.get('parent', '')}|"
                f"{item.get('armor', 0.0)}|{item.get('passThrough', 1.0)}|"
                f"{hp_str}|{uniform_info}\n"
            )
    print(f"Saved: {output_txt}")

    # Run classifier
    classify_cmd = [
        sys.executable,
        str(CLASSIFY_SCRIPT),
        "--input",
        str(output_txt),
        "--output",
        str(OUTPUT_DIR / "classified_batch.tsv"),
    ]
    print(f"\nRunning classifier...")
    subprocess.run(classify_cmd)
    print(f"Done.")


if __name__ == "__main__":
    main()
