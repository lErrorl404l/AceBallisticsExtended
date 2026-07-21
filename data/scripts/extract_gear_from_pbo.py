#!/usr/bin/env python3
"""
Extract wearable armor from Arma 3 character PBOs via armake.
Parses config.bin for each PBO, finds all CfgWeapons entries that are
VestItem, HeadgearItem, UniformItem, or GlassesItem, and reports their
armor values, hitpoint protection, and model/rvmat references.

Usage:
    python extract_gear_from_pbo.py
    python extract_gear_from_pbo.py --pbo-dir /path/to/Addons
"""

import json
import os
import re
import subprocess
import sys
import xml.etree.ElementTree as ET
from collections import defaultdict
from pathlib import Path

ARMAGE_BIN = "armake"
PBO_DIR = "/ext/SteamLibrary/steamapps/common/Arma 3/Addons"
EXTRACT_DIR = "/tmp/pbo_gear_extract"

# ponytail: static \$STR_A3_* → English displayName lookup for base Arma 3 items.
# The gear-related keys live in compiled stringtables not readable from PBO XML
# (language_f.pbo only has vehicle/editor/UI keys). Add entries as new items appear.
STR_A3_LOOKUP = {
    # UniforMs
    "STR_A3_Combat_fatigues": "Combat Fatigues",
    "STR_A3_Combat_fatigues_tee": "Combat Fatigues (Tee)",
    "STR_A3_Recon_fatigues": "Recon Fatigues",
    "STR_A3_Ghillie_suit": "Ghillie Suit",
    "STR_A3_Ghillie_suit_Iran": "Ghillie Suit",
    "STR_A3_HELIPILOT_COVERALLS_NATO_0": "Heli Pilot Coveralls",
    "STR_A3_HELIPILOT_COVERALLS_AAF_0": "Heli Pilot Coveralls",
    "STR_A3_U_B_Wetsuit0": "Wetsuit",
    "STR_A3_U_OI_Wetsuit0": "Wetsuit",
    "STR_A3_U_IA_Wetsuit0": "Wetsuit",
    "STR_A3_combat_fatigues_worn": "Combat Fatigues (Worn)",
    "STR_A3_Pilot_coveralls": "Pilot Coveralls",
    "STR_A3_Pilot_coveralls_Iran": "Pilot Coveralls",
    "STR_A3_Iran_Fatigues_hex": "Combat Fatigues (Hex)",
    "STR_A3_Iran_fatigues_urban": "Combat Fatigues (Urban)",
    "STR_A3_Recon_fatigues_hex": "Recon Fatigues (Hex)",
    "STR_A3_officer_uniform": "Officer Uniform",
    "STR_A3_combat_fatigues_haf0": "Combat Fatigues",
    "STR_A3_combat_fatigues_haf1": "Combat Fatigues (S/S)",
    "STR_A3_combat_fatigues_haf_tee": "Combat Fatigues (Tee)",
    "STR_A3_pilot_coveralls_haf": "Pilot Coveralls",
    "STR_A3_ghillie_suit_haf": "Ghillie Suit",
    "STR_A3_COMMONER_CLOTHES_BLUE_0": "Polo Shirt (Blue)",
    "STR_A3_COMMONER_CLOTHES_BURGUNDY_0": "Polo Shirt (Burgundy)",
    "STR_A3_COMMONER_CLOTHES_STRIPED_0": "Polo Shirt (Striped)",
    "STR_A3_COMMONER_CLOTHES_TRICOLOR_0": "Polo Shirt (Tricolour)",
    "STR_A3_COMMONER_CLOTHES_SALMON_0": "Polo Shirt (Salmon)",
    "STR_A3_COMMONER_CLOTHES_REDWHITE_0": "Polo Shirt (Red/White)",
    "STR_A3_rangemaster_suit0": "Rangemaster Suit",
    "STR_A3_jacket_shorts": "Jacket & Shorts",
    "STR_A3_Characters_F_Beta_0": "Competitor Outfit",
    "STR_A3_GUERILLA_GARMENT": "Guerrilla Garment",
    "STR_A3_GUERILLA_OUTFIT_PLAIN_DARK": "Guerrilla Outfit (Dark)",
    "STR_A3_GUERILLA_OUTFIT_PATTERN": "Guerrilla Outfit (Pattern)",
    "STR_A3_GUERILLA_OUTFIT_PLAIN": "Guerrilla Outfit (Plain)",
    "STR_A3_GUERILLA_SMOCKS": "Guerrilla Smocks",
    "STR_A3_GUERILLA_UNIFORM": "Guerrilla Uniform",
    "STR_A3_GUERILLA_RAIMENT": "Guerrilla Raiment",
    "STR_A3_Worn_clothes0": "Worn Clothes",
    "STR_A3_worker_overalls": "Worker Overalls",
    "STR_A3_HUNTING_CLOTHES_0": "Hunting Clothes",
    "STR_A3_combat_uniform_csat_1": "Combat Uniform",
    "STR_A3_combat_uniform_csat_2": "Combat Uniform",
    "STR_A3_combat_uniform_csat_3": "Combat Uniform",
    "STR_A3_survival_fatigues_F0": "Survival Fatigues",
    "STR_A3_uniform_kerry0": "Uniform (Kerry)",
    "STR_A3_uniform_stavrou0": "Uniform (Stavrou)",
    "STR_A3_Journalist_clothes": "Journalist Clothes",
    "STR_A3_Scientist_clothes": "Scientist Clothes",
    "STR_A3_VRSUIT_NATO_0": "VR Suit",
    "STR_A3_VRSUIT_CSAT_0": "VR Suit",
    "STR_A3_VRSUIT_AAF_0": "VR Suit",
    "STR_A3_VRSUIT_CIV_0": "VR Suit",
    "STR_A3_Karts_Uniform00": "Karts Uniform",
    "STR_A3_Karts_Uniform01": "Karts Uniform",
    "STR_A3_Karts_Uniform02": "Karts Uniform",
    "STR_A3_Karts_Uniform03": "Karts Uniform",
    "STR_A3_Karts_Uniform04": "Karts Uniform (Black)",
    "STR_A3_Karts_Uniform05": "Karts Uniform (Blue)",
    "STR_A3_Karts_Uniform06": "Karts Uniform (Green)",
    "STR_A3_Karts_Uniform07": "Karts Uniform (Red)",
    "STR_A3_Karts_Uniform08": "Karts Uniform (White)",
    "STR_A3_Karts_Uniform09": "Karts Uniform (Yellow)",
    "STR_A3_Karts_Uniform10": "Karts Uniform (Orange)",
    "STR_A3_Karts_Uniform12": "Marshal Uniform",
    "STR_A3_B_Full_Ghillie_Lush_F0": "Full Ghillie (Lush)",
    "STR_A3_B_Full_Ghillie_SemiArid_F0": "Full Ghillie (Semi-Arid)",
    "STR_A3_B_Full_Ghillie_Arid_F0": "Full Ghillie (Arid)",
    "STR_A3_O_Full_Ghillie_Lush_F0": "Full Ghillie (Lush)",
    "STR_A3_O_Full_Ghillie_SemiArid_F0": "Full Ghillie (Semi-Arid)",
    "STR_A3_O_Full_Ghillie_Arid_F0": "Full Ghillie (Arid)",
    "STR_A3_I_Full_Ghillie_Lush_F0": "Full Ghillie (Lush)",
    "STR_A3_I_Full_Ghillie_SemiArid_F0": "Full Ghillie (Semi-Arid)",
    "STR_A3_I_Full_Ghillie_Arid_F0": "Full Ghillie (Arid)",
    # Vests
    "STR_A3_V_BandollierB_khk0": "Bandolier",
    "STR_A3_V_PlateCarrier1_rgr0": "Plate Carrier 1",
    "STR_A3_V_PlateCarrier2_rgr0": "Plate Carrier 2",
    "STR_A3_V_PLATECARRIER2_BLK0": "Plate Carrier 2 (BLK)",
    "STR_A3_V_PlateCarrierGL_rgr0": "Plate Carrier GL",
    "STR_A3_V_PlateCarrier1_blk0": "Plate Carrier 1",
    "STR_A3_V_PlateCarrierSpec_rgr0": "Plate Carrier Spec",
    "STR_A3_V_Chestrig_khk0": "Chest Rig",
    "STR_A3_ChestrigF_oli": "Chest Rig (Olive)",
    "STR_A3_V_TacVest_khk0": "Tactical Vest",
    "STR_A3_V_TacVest_camo0": "Tactical Vest (Camo)",
    "STR_A3_V_TacVest_blk_POLICE0": "Tactical Vest (Police)",
    "STR_A3_V_TacVestIR_blk0": "Tactical Vest (IR)",
    "STR_A3_V_HarnessO_brn0": "Harness",
    "STR_A3_V_HarnessOGL_brn0": "Harness GL",
    "STR_A3_V_PlateCarrierIA1_dgtl0": "Plate Carrier IA 1",
    "STR_A3_V_PlateCarrierIAGL_dgtl0": "Plate Carrier IA GL",
    "STR_A3_cfgvests_rebreather_nato0": "Rebreather",
    "STR_A3_V_PlateCarrier1_rgr_V_PlateCarrier_Kerry0": "Plate Carrier (Kerry)",
    "STR_A3_V_Press_F0": "Press Vest",
    "STR_V_Rangemaster_belt0": "Rangemaster Belt",
    # Helmets
    "STR_A3_H_HelmetB0": "Helmet (ECH)",
    "STR_A3_H_HelmetSpecB0": "Helmet Spec (FAST SF)",
    "STR_A3_H_Booniehat_khk0_boot": "Boonie Hat",
    "STR_A3_H_Cap_red0": "Cap (Red)",
    "STR_H_Cap_headphones0": "Cap (Headphones)",
    "STR_A3_H_MilCap_ocamo0": "Military Cap (Hex)",
    "STR_A3_H_Bandanna_surfer0": "Bandanna (Surfer)",
    "STR_A3_H_ShemagOpen_khk0_boot": "Shemag (Open)",
    "STR_A3_H_Beret_blk0": "Beret (Black)",
    "STR_A3_H_Watchcap_blk0": "Watch Cap (Black)",
    "STR_A3_H_StrawHat0": "Straw Hat",
    "STR_A3_H_Hat_blue0": "Hat (Blue)",
    # Glasses
    "STR_A3_CfgGlasses_G_B_Diving0": "Diving Mask",
    "STR_A3_CfgGlasses_G_O_Diving0": "Diving Mask",
    "STR_A3_CfgGlasses_G_I_Diving0": "Diving Mask",
    "STR_A3_CFGGLASSES_G_Balaclava_oli0": "Balaclava (Olive)",
    "STR_A3_CFGGLASSES_G_Balaclava_combat0": "Balaclava (Combat)",
    "STR_A3_CFGGLASSES_G_Balaclava_lowprofile0": "Balaclava (Low Profile)",
    "STR_A3_CFGGLASSES_G_Bandana_clean0": "Bandanna (Black)",
    "STR_A3_CFGGLASSES_G_Bandanna_oli0": "Bandanna (Olive)",
    "STR_A3_CFGGLASSES_G_Bandanna_khk0": "Bandanna (Khaki)",
    "STR_A3_CFGGLASSES_G_Bandanna_tan0": "Bandanna (Tan)",
    "STR_A3_CFGGLASSES_G_Bandanna_beast0": "Bandanna (Beast)",
    "STR_A3_CFGGLASSES_G_Bandana_shades0": "Bandanna (Shades)",
    "STR_A3_CFGGLASSES_G_Bandana_sport0": "Bandanna (Sport)",
    "STR_A3_CFGGLASSES_G_Bandana_aviator0": "Bandanna (Aviator)",
}

# STR refs from other known mods (add as encountered)
STR_MOD_LOOKUP: dict[str, str] = {}


def load_stringtable_from_pbo(pbo_data: dict) -> dict[str, str]:
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


def resolve_display_name(
    display_name: str,
    pbo_stringtable: dict[str, str] | None = None,
) -> str:
    """Resolve a $STR_* displayName reference to its English text.

    Checks in order:
    1. PBO-embedded stringtable.xml (the most authoritative)
    2. STR_A3_LOOKUP for base game keys
    3. STR_MOD_LOOKUP for known mod keys
    4. Fallback: extract readable text from the key name
    """
    if not display_name.startswith("$STR_"):
        return display_name

    key = display_name[1:]  # strip leading $ → "STR_A3_Combat_fatigues"

    # Pass 1: PBO-embedded stringtable
    if pbo_stringtable is not None:
        val = pbo_stringtable.get(key.lower())
        if val:
            return val

    # Pass 2: base game lookup
    val = STR_A3_LOOKUP.get(key)
    if val:
        return val

    # Pass 3: known mod lookup
    val = STR_MOD_LOOKUP.get(key)
    if val:
        return val

    # Pass 4: fallback — extract readable suffix from the key
    # e.g. "STR_A3_Combat_fatigues" → "Combat Fatigues"
    # Strip leading "STR_<mod>_" prefix
    parts = key.split("_", 2)
    if len(parts) >= 3:
        rest = parts[2]  # everything after "STR_A3_"
        readable = rest.replace("_", " ").strip()
        # Remove trailing digits
        readable = re.sub(r"\d+$", "", readable).strip()
        if readable:
            return readable.title()

    return display_name


# ── Safe numeric parsers ──────────────────────────────────────────────────
def _safe_float(v, default=0.0):
    """Parse float, returning default for non-numeric values like '22+3'."""
    if not v:
        return default
    try:
        return float(v)
    except (ValueError, TypeError):
        return default


def _safe_int(v, default: int | None = 0):
    """Parse int, returning default for non-numeric values like 'public'."""
    if not v:
        return default
    try:
        return int(v)
    except (ValueError, TypeError):
        return default


# Item types we care about
GEAR_TYPES = {"VestItem", "HeadgearItem", "UniformItem", "GlassesItem"}
# Normalized (lowercase) lookup for case-insensitive type matching
GEAR_TYPES_NORM = {t.lower(): t for t in GEAR_TYPES}
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
GEAR_PARENTS_NORM = {p.lower() for p in GEAR_PARENTS}


def read_pbo(pbo_path):
    """Read a PBO file and extract all entries into a dict of filename → bytes.

    Pure Python implementation following the official PBO format:
    https://community.bistudio.com/wiki/PBO_Format

    Format:
      1. Version entry (empty filename + 'sreV' magic + 4 zero fields)
      2. Header properties (key\\0value\\0... terminated by \\0\\0)
      3. File entries (filename\\0 + packing/u32 + orig_size/u32 +
         reserved/u32 + timestamp/u32 + data_size/u32)
      4. Terminator (empty filename + 5 zero u32s)
      5. File data blocks

    Handles both standard and Mikero/DePbo PBO formats.
    """
    import struct

    with open(pbo_path, "rb") as f:
        data = f.read()

    n = len(data)
    pos = 0
    result = {}

    def read_asciiz(offset):
        """Read a null-terminated string, returns (string, new_offset)."""
        end = data.find(b"\0", offset)
        if end < 0:
            return "", offset
        return data[offset:end].decode("ascii", errors="replace"), end + 1

    # ── Step 1: Version entry ──
    # First entry has empty filename and packing = 'sreV' magic
    if pos + 21 > n:
        raise ValueError(f"Not a valid PBO file (too small): {pbo_path}")
    filename, pos = read_asciiz(pos)
    (packing, orig_size, reserved, timestamp, data_size) = struct.unpack_from(
        "<IIIII", data, pos
    )
    pos += 20

    srev_magic = struct.unpack("<I", b"sreV")[0]
    if packing != srev_magic:
        # Some PBOs have a leading 0x00 byte before "sreV"
        # Scan first 16 bytes for the magic
        found = False
        for scan_pos in range(min(16, n)):
            p = data[scan_pos : scan_pos + 4]
            if len(p) == 4 and struct.unpack("<I", p)[0] == srev_magic:
                # Retry from this position
                return read_pbo_raw(data, scan_pos, pbo_path)
            if scan_pos + 4 > n:
                break
        raise ValueError(f"Not a PBO file (bad version magic): {pbo_path}")
    _ = orig_size, reserved, timestamp, data_size  # unused for version entry

    # ── Step 2: Header properties (key=value pairs, \\0\\0 terminated) ──
    properties = {}
    while pos < n:
        key, pos = read_asciiz(pos)
        if not key:
            break
        value, pos = read_asciiz(pos)
        properties[key] = value

    # ── Step 3: File entry headers ──
    file_entries = []
    while pos < n:
        filename, pos = read_asciiz(pos)
        if not filename:
            break  # empty filename = end of file entries
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

    # ── Step 3.5: Skip entry terminator (20 zero bytes after empty filename) ──
    pos += 20  # 5 uint32 fields of the terminator entry

    # ── Step 4: File data ──
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


def read_pbo_raw(data, offset, pbo_path):
    """Read PBO data starting from a known offset (used for Mikero's leading-byte variant)."""
    import struct

    n = len(data)
    pos = offset

    def ra(off):
        end = data.find(b"\0", off)
        if end < 0:
            return "", off
        return data[off:end].decode("ascii", errors="replace"), end + 1

    filename, pos = ra(pos)
    (packing, _orig_sz, _rsv, _ts, _ds) = struct.unpack_from("<IIIII", data, pos)
    pos += 20
    if struct.unpack("<I", b"sreV")[0] != packing:
        raise ValueError(f"Bad PBO version magic at offset {offset}: {pbo_path}")

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
        file_entries.append(
            {"name": fn, "packing": p, "orig_size": osz, "data_size": dsz}
        )

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


def derapify_bytes(bin_data):
    """Derapify binary config data to HPP text by piping through armake.

    armake expects the \x00raP header (Arma 2 big-endian format), not raP\x00
    (Arma 3 little-endian). Some mods use a hybrid format with a text prefix
    before \x00raP. We search for \x00raP within the first 4096 bytes.
    """
    # Find \x00raP within first 4096 bytes
    rap_pos = -1
    search_limit = min(4096, len(bin_data) - 4)
    for off in range(search_limit):
        if bin_data[off] == 0 and bin_data[off + 1 : off + 4] == b"raP":
            rap_pos = off
            break

    if rap_pos < 0:
        return None

    # Extract from null byte before raP onward
    clean_data = bin_data[rap_pos:]

    result = subprocess.run(
        [ARMAGE_BIN, "derapify"],
        input=clean_data,
        capture_output=True,
        timeout=30,
    )
    if result.returncode != 0:
        return None
    return result.stdout.decode("utf-8", errors="replace")


def derapify_config(bin_path, out_path):
    """Derapify a config.bin to hpp."""
    if os.path.exists(out_path):
        return out_path
    result = subprocess.run(
        [ARMAGE_BIN, "derapify", bin_path, out_path],
        capture_output=True,
        text=True,
        timeout=30,
    )
    if result.returncode != 0:
        return None
    return out_path


def find_config_bins(extract_dir):
    """Find all config.bin files in the extraction tree."""
    bins = []
    for root, dirs, files in os.walk(extract_dir):
        for f in files:
            if f == "config.bin":
                bins.append(os.path.join(root, f))
    return bins


def parse_gear_from_hpp(text, source_pbo, pbo_stringtable=None):
    """Parse derapified config.hpp text for wearable gear items.

    Detects items by finding `class ItemInfo: VestItem|HeadgearItem|UniformItem|GlassesItem`
    INSIDE class bodies within CfgWeapons. This catches ALL gear items regardless of
    parent class chain (many inherit from ItemCore with ItemInfo defining the type).

    Args:
        text: Config text content (string, not a file path).
        source_pbo: Source PBO name for tracking.
        pbo_stringtable: Optional {key: value} dict from stringtable.xml.
    """

    items = []

    # Locate all CfgWeapons block boundaries (case-insensitive: some mods use cfgWeapons)
    cfg_blocks = []
    pos = 0
    while True:
        idx = re.search(r"(?i)class\s+cfgweapons\b", text[pos:])
        if idx:
            idx = pos + idx.start()
        else:
            break
        block_start = text.find("{", idx)
        if block_start == -1:
            break
        depth = 1
        block_end = block_start + 1
        while depth > 0 and block_end < len(text):
            ch = text[block_end]
            if ch == "{":
                depth += 1
            elif ch == "}":
                depth -= 1
            block_end += 1
        cfg_blocks.append({"block": text[idx:block_end], "offset": idx})
        pos = block_end

    for entry in cfg_blocks:
        cfg_block = entry["block"]
        block_offset = entry["offset"]
        pos = 0
        block_len = len(cfg_block)

        while pos < block_len:
            try:
                # Find the next class definition
                m = re.search(r"class\s+(\S+)\s*:\s*(\S+)\s*\{", cfg_block[pos:])
                if not m:
                    break

                abs_start = pos + m.start()
                # Compute the actual position in the full file text
                # cfg_block starts at block_offset in text
                actual_pos_in_text = block_offset + abs_start
                prefix = text[:actual_pos_in_text]
                depth = prefix.count("{") - prefix.count("}")

                if depth != 1:
                    # Not top-level, skip past this match
                    pos = abs_start + 1
                    continue

                cls_name = m.group(1)
                parent = m.group(2).rstrip("{").strip()

                # Skip ItemInfo itself (it's the type marker, not a gear item)
                if cls_name == "ItemInfo":
                    pos = abs_start + 1
                    continue

                # Extract the full body of this class by counting braces
                class_start = pos + m.end()  # absolute position after opening brace
                inner_depth = 1
                class_end = class_start
                while inner_depth > 0 and class_end < block_len:
                    ch = cfg_block[class_end]
                    if ch == "{":
                        inner_depth += 1
                    elif ch == "}":
                        inner_depth -= 1
                    class_end += 1

                class_body = cfg_block[class_start : class_end - 1]

                # Check if this class has ANY ItemInfo block
                info_match = re.search(
                    r"class\s+ItemInfo\s*(?::\s*(\S+))?\s*\{",
                    class_body,
                )
                if not info_match:
                    # Fallback: check parent chain for known gear types (case-insensitive)
                    if (
                        parent.lower() not in GEAR_TYPES_NORM
                        and parent.lower() not in GEAR_PARENTS_NORM
                    ):
                        pos = class_end
                        continue

                # Determine item_type from ItemInfo parent first (case-insensitive), then fall back
                item_type = None
                if info_match:
                    info_parent = info_match.group(1)
                    if info_parent and info_parent.lower() in GEAR_TYPES_NORM:
                        item_type = GEAR_TYPES_NORM[info_parent.lower()]

                # Now parse the item
                item = parse_gear_class(
                    cls_name, parent, class_body, item_type, source_pbo, pbo_stringtable
                )
                if item:
                    items.append(item)

                pos = class_end

            except Exception:
                # Isolate individual class parse failures — skip past the
                # failed class body entirely to avoid infinite re-parse.
                # Common triggers: brace imbalance, armor='22+3' expressions.
                # class_end is always set before parse_gear_class is called
                # (where most exceptions happen), so the try/except guards it.
                try:
                    pos = max(pos, class_end)
                except NameError:
                    pos = pos + 1  # skip past this match

    return items


def parse_gear_class(
    classname, parent, body, item_type=None, source_pbo="", pbo_stringtable=None
):
    """Parse a single gear class definition.

    Args:
        classname: Config class name
        parent: Parent class name
        body: Class body text
        item_type: Determined from ItemInfo (if None, falls back to parent/prefix)
        source_pbo: Source PBO name
        pbo_stringtable: Optional {key: value} dict from stringtable.xml for STR_* resolution.
    """
    type_confidence = "inherit"

    if not item_type:
        # Fallback: determine from parent chain or classname prefix (case-insensitive)
        parent_lower = parent.lower()
        if parent_lower in {
            "vestitem",
            "vest_camo_base",
            "vest_nocamo_base",
            "vest_base_f",
        }:
            item_type = "VestItem"
        elif parent_lower in {
            "headgearitem",
            "headgearitem",
            "headgear_base_f",
            "h_helmet_base_f",
            "h_helmetb",
        }:
            item_type = "HeadgearItem"
        elif parent_lower in {"uniformitem", "uniform_base"}:
            item_type = "UniformItem"
        elif parent_lower in {"glassesitem", "glasses_base_f", "g_glasses_base_f"}:
            item_type = "GlassesItem"
        elif classname.startswith("V_"):
            item_type = "VestItem"
            type_confidence = "prefix"
        elif classname.startswith("H_"):
            item_type = "HeadgearItem"
            type_confidence = "prefix"
        elif classname.startswith("U_"):
            item_type = "UniformItem"
            type_confidence = "prefix"
        elif classname.startswith("G_"):
            item_type = "GlassesItem"
            type_confidence = "prefix"
        else:
            return None
    else:
        type_confidence = "iteminfo"

    # Basic fields
    display_name = extract_field(body, "displayName")
    display_name = resolve_display_name(display_name, pbo_stringtable)
    scope = extract_field(body, "scope")
    model = extract_field(body, "model")
    picture = extract_field(body, "picture")
    description_short = extract_field(body, "descriptionShort")

    # ItemInfo fields (handles "class ItemInfo {", "class ItemInfo: ItemInfo {", "class ItemInfo: VestItem {", etc)
    item_info_pattern = re.compile(r"class\s+ItemInfo\s*(?::\s*\S+)?\s*\{")
    m_info = item_info_pattern.search(body)
    if m_info:
        start2 = m_info.end()
        depth2 = 1
        pos2 = start2
        while pos2 < len(body) and depth2 > 0:
            if body[pos2] == "{":
                depth2 += 1
            elif body[pos2] == "}":
                depth2 -= 1
            pos2 += 1
        item_info = body[start2 : pos2 - 1]
    else:
        item_info = ""
    armor = extract_field(item_info, "armor") if item_info else 0
    pass_through = extract_field(item_info, "passThrough") if item_info else 1.0
    uniform_class = extract_field(item_info, "uniformClass") if item_info else ""
    container_class = extract_field(item_info, "containerClass") if item_info else ""
    mass = extract_field(item_info, "mass") if item_info else 0

    # hiddenSelectionsMaterials
    hsm = extract_array(body, "hiddenSelectionsMaterials")
    rvmat_paths = hsm if hsm else []

    # hiddenSelectionsTextures (may carry material hints too)
    hst = extract_array(body, "hiddenSelectionsTextures")

    # Parse HitpointsProtectionInfo
    hitpoints = []
    hp_block = (
        extract_block(item_info, "class HitpointsProtectionInfo") if item_info else ""
    )
    if hp_block:
        # Find all subclasses (each is a hitpoint)
        hp_pattern = re.compile(r"class\s+(\S+)\s*\{([^}]+)\}", re.DOTALL)
        for hp_match in hp_pattern.finditer(hp_block):
            hp_name = hp_match.group(1)
            hp_body = hp_match.group(2)
            hp_armor = (
                extract_field(hp_body, "armor") or extract_field(hp_body, "Armor") or 0
            )
            hp_pt = (
                extract_field(hp_body, "passThrough")
                or extract_field(hp_body, "PassThrough")
                or 1.0
            )
            hp_hitpoint = (
                extract_field(hp_body, "hitpointName")
                or extract_field(hp_body, "HitpointName")
                or ""
            )
            hitpoints.append(
                {
                    "name": hp_name,
                    "armor": _safe_float(hp_armor),
                    "passThrough": _safe_float(hp_pt, 1.0),
                    "hitpointName": hp_hitpoint,
                }
            )

    # Extract uniform hitpoints (resolved through uniformClass)
    uniform_hitpoints = []
    if item_type == "UniformItem" and uniform_class:
        uniform_hitpoints.append(
            {
                "uniformClass": uniform_class,
                "note": "resolve through CfgVehicles",
            }
        )

    return {
        "classname": classname,
        "parent": parent,
        "displayName": display_name or classname,
        "itemType": item_type,
        "typeConfidence": type_confidence,
        "scope": _safe_int(scope, default=None),
        "armor": _safe_float(armor),
        "passThrough": _safe_float(pass_through, 1.0),
        "modelPath": model or "",
        "rvmatPaths": rvmat_paths,
        "texturePaths": hst if hst else [],
        "uniformClass": uniform_class,
        "containerClass": container_class,
        "mass": int(mass) if mass else 0,
        "descriptionShort": description_short or "",
        "hitpoints": hitpoints,
        "sourcePBO": source_pbo,
    }


def extract_field(text, field_name):
    """Extract a field value from config text. Handles quoted and unquoted values."""
    # Try quoted string first
    m = re.search(rf'{field_name}\s*=\s*"([^"]*)"', text)
    if m:
        return m.group(1)
    # Try numeric value
    m = re.search(rf"{field_name}\s*=\s*([\d.]+)", text)
    if m:
        return m.group(1)
    return ""


def extract_array(text, array_name):
    """Extract an array value from config text."""
    m = re.search(rf"{array_name}\s*=\s*\{{([^}}]*)\}}", text, re.DOTALL)
    if m:
        inner = m.group(1)
        items = re.findall(r'"([^"]*)"', inner)
        return items
    return []


def extract_block(text, block_name):
    """Extract a named class block, returning its inner content."""
    # Find the opening
    m = re.search(rf"{re.escape(block_name)}\s*\{{", text)
    if not m:
        return ""
    start = m.end()
    depth = 1
    pos = start
    while pos < len(text) and depth > 0:
        if text[pos] == "{":
            depth += 1
        elif text[pos] == "}":
            depth -= 1
        pos += 1
    return text[start : pos - 1]  # Return content without closing brace


def extract_rvmat_model_mappings(config_text):
    """Extract model→rvmat mappings from CfgModels in config text."""
    mappings = {}
    # Look for CfgModels sections with hiddenSelectionsMaterials
    cfg_models_pattern = re.compile(
        r"class\s+CfgModels\s*\{([^}]+(?:\{[^}]*\}[^}]*)*)\}", re.DOTALL
    )
    cm_match = cfg_models_pattern.search(config_text)
    if cm_match:
        cm_text = cm_match.group(1)
        # Find individual model definitions with material assignments
        model_pattern = re.compile(r"class\s+(\S+)\s*\{([^}]+)\}", re.DOTALL)
        for mm in model_pattern.finditer(cm_text):
            model_name = mm.group(1)
            model_body = mm.group(2)
            mats = extract_array(model_body, "hiddenSelectionsMaterials")
            if mats:
                mappings[model_name] = mats
    return mappings


def parse_glasses_from_hpp(text, source_pbo, pbo_stringtable=None):
    """Parse CfgGlasses section from config text for glasses items.

    Glasses live in CfgGlasses (not CfgWeapons), and have NO ItemInfo blocks.
    They're simple structs extending None or other G_ items.

    Args:
        text: Config text content (string, not a file path).
        source_pbo: Source PBO name for tracking.
        pbo_stringtable: Optional {key: value} dict from stringtable.xml for STR_* resolution.
    """

    items = []
    # Find CfgGlasses block (case-insensitive)
    m = re.search(r"(?i)class\s+cfgGlasses\s*\{", text)
    if not m:
        return items

    # Extract CfgGlasses body
    start = m.end()
    depth = 1
    pos = start
    while depth > 0 and pos < len(text):
        if text[pos] == "{":
            depth += 1
        elif text[pos] == "}":
            depth -= 1
        pos += 1
    cfg_glasses_body = text[start : pos - 1]

    # Find all top-level classes in CfgGlasses
    # They look like: class G_Name: Parent { ... };
    # Skip forward declarations (class Name;)
    class_pattern = re.compile(r"class\s+(\S+)\s*:\s*(\S+)\s*\{")
    pos2 = 0
    while pos2 < len(cfg_glasses_body):
        m2 = class_pattern.search(cfg_glasses_body, pos2)
        if not m2:
            break

        cls_name = m2.group(1)
        parent = m2.group(2).rstrip("{").strip()

        # Skip if parent is None (the base empty class)
        if parent == "None":
            pos2 = m2.end()
            continue

        # Extract class body
        body_start = m2.end()
        inner_depth = 1
        body_end = body_start
        while inner_depth > 0 and body_end < len(cfg_glasses_body):
            ch = cfg_glasses_body[body_end]
            if ch == "{":
                inner_depth += 1
            elif ch == "}":
                inner_depth -= 1
            body_end += 1
        body = cfg_glasses_body[body_start : body_end - 1]

        # Read fields
        model = extract_field(body, "model")
        picture = extract_field(body, "picture")
        display_name = extract_field(body, "displayname") or extract_field(
            body, "displayName"
        )
        display_name = resolve_display_name(display_name, pbo_stringtable)
        scope = extract_field(body, "scope")
        mass = extract_field(body, "mass")

        items.append(
            {
                "classname": cls_name,
                "parent": parent,
                "displayName": display_name or cls_name,
                "itemType": "GlassesItem",
                "typeConfidence": "iteminfo",
                "scope": _safe_int(scope, 2),
                "armor": 0.0,
                "passThrough": 1.0,
                "modelPath": model or "",
                "rvmatPaths": [],
                "texturePaths": [],
                "uniformClass": "",
                "containerClass": "",
                "mass": int(mass) if mass else 0,
                "descriptionShort": "",
                "hitpoints": [],
                "uniformHitpoints": [],
                "sourcePBO": source_pbo,
            }
        )

        pos2 = body_end

    return items


def resolve_inheritance(items):
    """Walk parent chains to fill in zero armor values from parent classes.

    Builds an index of all extracted items, then for items with armor=0,
    traces the parent chain to inherit armor/passthrough values.
    """
    by_classname = {item["classname"]: item for item in items}

    for item in items:
        if item["armor"] > 0 or item["passThrough"] < 1.0:
            continue
        # Walk parent chain
        current = item
        visited = {item["classname"]}
        while current["armor"] <= 0 and current["passThrough"] >= 1.0:
            parent_name = current.get("parent", "")
            if not parent_name or parent_name in visited:
                break
            visited.add(parent_name)
            parent = by_classname.get(parent_name)
            if not parent:
                break
            if parent["armor"] > 0:
                item["armor"] = parent["armor"]
            if parent["passThrough"] < 1.0:
                item["passThrough"] = parent["passThrough"]
            if parent["hitpoints"] and not item["hitpoints"]:
                item["hitpoints"] = parent["hitpoints"]
            item["_inheritedFrom"] = parent_name
            current = parent


def main():
    import argparse

    parser = argparse.ArgumentParser(
        description="Extract wearable armor from Arma 3 PBO configs"
    )
    parser.add_argument(
        "--pbo",
        action="append",
        dest="pbo_files",
        help="Specific PBO file(s) to process (may be repeated)",
    )
    parser.add_argument(
        "--pbo-dir",
        dest="pbo_dir",
        default=PBO_DIR,
        help=f"Directory containing PBO files (default: {PBO_DIR})",
    )
    args = parser.parse_args()

    if args.pbo_files:
        pbo_files = []
        for p in args.pbo_files:
            if not os.path.isfile(p):
                print(f"Warning: PBO file not found: {p}")
                continue
            pbo_files.append(p)
        if not pbo_files:
            print("Error: no valid PBO files specified")
            sys.exit(1)
        pbo_files.sort()
    else:
        pbo_dir = args.pbo_dir
        pbo_files = sorted(
            [
                os.path.join(pbo_dir, f)
                for f in os.listdir(pbo_dir)
                if f.endswith(".pbo")
            ]
        )

    print(f"Found {len(pbo_files)} PBOs to process")

    all_items = []
    all_rvmat_mappings = {}
    processed = 0

    for pbo_path in pbo_files:
        pbo_name = Path(pbo_path).stem
        print(f"\n[{processed + 1}/{len(pbo_files)}] {pbo_name}...")

        try:
            pbo_data = read_pbo(pbo_path)
        except Exception as e:
            print(f"  PBO read failed: {e}")
            processed += 1
            continue

        entries = pbo_data["files"]

        # Look for config.bin (may be named differently in some PBOs)
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
            # Check for files containing "config" case-insensitively
            for name in entries:
                if "config" in name.lower():
                    config_key = name
                    break
        if not config_key:
            print(f"  No config found")
            processed += 1
            continue

        # Read and derapify config
        config_data = entries[config_key]

        # Detect plain-text config.cpp/config.hpp — skip armake, parse directly.
        # Binary raP starts with \\0raP; text starts with 'class', '/*', '//', etc.
        is_text_config = False
        if len(config_data) > 16:
            peek = config_data[:16]
            if (peek[0:1] != b"\x00" and peek[4:7] != b"raP") and (
                peek[:5] == b"class" or peek[:2] == b"/*" or peek[:2] == b"//"
            ):
                is_text_config = True

        try:
            if is_text_config:
                config_text = config_data.decode("utf-8", errors="replace")
            else:
                config_text = derapify_bytes(config_data)
        except Exception as e:
            print(f"  Config parse failed: {e}")
            processed += 1
            continue

        if not config_text:
            print(f"  Empty config")
            processed += 1
            continue

        # Load stringtable and resolve displayName refs
        pbo_stringtable = load_stringtable_from_pbo(pbo_data)

        # Parse gear items
        items = parse_gear_from_hpp(config_text, pbo_name, pbo_stringtable)
        glasses = parse_glasses_from_hpp(config_text, pbo_name, pbo_stringtable)
        all_items.extend(items)
        all_items.extend(glasses)

        # Extract rvmat model mappings from the config text (CfgModels section)
        mappings = extract_rvmat_model_mappings(config_text)
        all_rvmat_mappings.update(mappings)

        print(
            f"  {len(items)} gear + {len(glasses)} glasses, {len(mappings)} rvmat mappings"
        )
        processed += 1

    # Resolve inherited armor values (walk parent chain)
    resolve_inheritance(all_items)

    # Deduplicate by classname (first PBO wins)
    seen = set()
    unique_items = []
    for item in all_items:
        if item["classname"] not in seen and item["scope"] == 2:  # Only public scope
            seen.add(item["classname"])
            unique_items.append(item)

    # Summary
    by_type = defaultdict(int)
    for item in unique_items:
        by_type[item["itemType"]] += 1

    print(f"\n{'=' * 60}")
    print(f"TOTAL: {len(unique_items)} unique public wearable items extracted")
    print(f"By type: {dict(by_type)}")
    print(f"Rvmat model mappings: {len(all_rvmat_mappings)}")
    print(f"{'=' * 60}")

    # Save outputs
    os.makedirs("data/scripts/extracted", exist_ok=True)

    # Save full extracted data as JSON
    output_json = "data/scripts/extracted/gear_extracted.json"
    with open(output_json, "w") as f:
        json.dump(
            {
                "meta": {
                    "source": "ArmA 3 characters_f PBOs via armake",
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

    # Save as SQF-format dump (compatible with classify_gear_armor.py)
    output_txt = "data/scripts/extracted/gear_dump.txt"
    with open(output_txt, "w") as f:
        for item in unique_items:
            hp_str = "~".join(
                [
                    f"{hp['name']}:{hp['armor']}:{hp['passThrough']}:{hp['hitpointName']}"
                    for hp in item["hitpoints"]
                ]
            )
            uniform_info = ""
            if item["itemType"] == "UniformItem":
                uniform_info = f"{item['uniformClass']}|"
            dn = item["displayName"].replace("|", " ")
            f.write(
                f"G|{item['classname']}|{dn}|{item['itemType']}|{item.get('parent', '')}|{item['armor']}|{item['passThrough']}|{hp_str}|{uniform_info}\n"
            )
    print(f"Saved: {output_txt}")

    # Save rvmat paths grouped by item
    rvmat_output = "data/scripts/extracted/gear_rvmat_paths.json"
    rvmat_by_item = {}
    for item in unique_items:
        if item["rvmatPaths"]:
            rvmat_by_item[item["classname"]] = item["rvmatPaths"]
        # Also add model path-based rvmat hints
        model = item.get("modelPath", "")
        if model:
            # Attempt to find corresponding rvmat files (by naming convention)
            base = Path(model).stem
            for rvmat_model, rvmat_paths in all_rvmat_mappings.items():
                if (
                    base.lower() in rvmat_model.lower()
                    or rvmat_model.lower() in base.lower()
                ):
                    if item["classname"] not in rvmat_by_item:
                        rvmat_by_item[item["classname"]] = []
                    rvmat_by_item[item["classname"]].extend(rvmat_paths)

    # Deduplicate per item
    for k in rvmat_by_item:
        rvmat_by_item[k] = list(set(rvmat_by_item[k]))

    with open(rvmat_output, "w") as f:
        json.dump(rvmat_by_item, f, indent=2)
    print(f"Saved: {rvmat_output}")

    # Print first 20 items as preview
    print(f"\nFirst 20 items:")
    for item in unique_items[:20]:
        hp_summary = "; ".join(
            [f"{h['name']}={h['armor']}" for h in item["hitpoints"][:3]]
        )
        print(
            f"  {item['itemType']:15s} {item['classname']:40s} armor={item['armor']:5.1f} pt={item['passThrough']:.2f}  hp=[{hp_summary}]"
        )

    # Print items with rvmat references
    rvmat_items = [item for item in unique_items if item["rvmatPaths"]]
    if rvmat_items:
        print(f"\nItems with explicit hiddenSelectionsMaterials:")
        for item in rvmat_items[:10]:
            print(f"  {item['classname']:40s} rvmat={item['rvmatPaths']}")


if __name__ == "__main__":
    main()
