#!/usr/bin/env python3
"""Generate ir_clothing.tsv from classified gear data + IRL gear search engine."""

import json
import os
import sys

# Add parent dir so we can import irl_gear_terms
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from irl_gear_terms import extract_irl_info

# Read classified data — could be batch output (1594+ items from all mods)
# or legacy single-PBO output (240 items).
for cls_candidate in [
    "data/scripts/extracted/classified_batch.json",
    "data/scripts/extracted/classified_gear_lookup.json",
]:
    if os.path.exists(cls_candidate):
        CLASSIFIED_PATH = cls_candidate
        break
else:
    raise FileNotFoundError("No classified data found")

GEAR_EXTRACTED_PATH = "data/scripts/extracted/gear_extracted_batch.json"
if not os.path.exists(GEAR_EXTRACTED_PATH):
    GEAR_EXTRACTED_PATH = "data/scripts/extracted/gear_extracted.json"

with open(CLASSIFIED_PATH) as f:
    classified = json.load(f)

with open(GEAR_EXTRACTED_PATH) as f:
    raw = json.load(f)
items = {i["classname"]: i for i in raw["items"]}


# ── Clothing category mapping ──────────────────────────────────────────────
def clothing_category(item_type, material, classname):
    if item_type == "Vest":
        if "EOD" in classname:
            return "eod_suit"
        if "Rebreather" in classname:
            return "rebreather"
        if "PlateCarrier" in classname or material.startswith("ceramic"):
            return "plate_carrier"
        if "TacVest" in classname:
            return "tactical_vest"
        if "Chestrig" in classname:
            return "chest_rig"
        if "Harness" in classname:
            return "harness"
        if "Bandollier" in classname:
            return "bandolier"
        return "armored_vest"
    elif item_type == "Headgear":
        if "ComTac" in classname or "comtac" in classname:
            return "tactical_headset"
        if "Headset" in classname or "HeadSet" in classname:
            return "headset"
        if "EarProtectors" in classname:
            return "ear_protectors"
        if "Helmet" in classname:
            return "ballistic_helmet"
        if "Cap" in classname:
            return "cap"
        if "Bandanna" in classname:
            return "headwear"
        if "Beret" in classname:
            return "beret"
        if "Booniehat" in classname:
            return "boonie_hat"
        if "MilCap" in classname:
            return "military_cap"
        if "Watchcap" in classname:
            return "watch_cap"
        if "Hat" in classname:
            return "hat"
        if "Shemag" in classname:
            return "shemag"
        if "StrawHat" in classname:
            return "straw_hat"
        return "headwear"
    elif item_type == "Uniform":
        if "Ghillie" in classname or "FullGhillie" in classname:
            return "ghillie_suit"
        if "Wetsuit" in classname:
            return "wetsuit"
        if "Pilot" in classname or "Coveralls" in classname:
            return "coveralls"
        if "CombatUniform" in classname:
            return "combat_uniform"
        if "Poloshirt" in classname:
            return "polo_shirt"
        if "CBRN" in classname:
            return "cbrn_suit"
        return "uniform"
    elif item_type == "Glasses":
        if "Balaclava" in classname:
            return "balaclava"
        if "Bandanna" in classname:
            return "bandanna"
        if "Diving" in classname:
            return "diving_mask"
        return "eyewear"
    return "other"


# ── NIJ rating from RHAe ───────────────────────────────────────────────────
def nij_rating(rha_mm):
    if rha_mm >= 80:
        return "IV"
    if rha_mm >= 40:
        return "III"
    if rha_mm >= 20:
        return "IIIA"
    return "N/A"


# ── Material thickness estimation ──────────────────────────────────────────
def estimate_thickness(rha_mm, material):
    if rha_mm <= 0:
        return 0.0
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
    }.get(material, 1.0)
    return rha_mm / max(mat_rha_factor, 0.01)


# ── Generate TSV ───────────────────────────────────────────────────────────
lines = [
    "# classname\titem_type\tclothing_category\tprimary_material\tbacking_material\t"
    "thickness_mm\tnij_rating\trha_mm\tconfidence\tmanufacturer\tmodel\tcamo"
]

for cn in sorted(classified.keys()):
    c = classified[cn]
    r = items.get(cn, {})

    # Use IRL gear search engine with displayName if available
    displayname = r.get("displayName", "")
    mfg, model, conf, camo = extract_irl_info(cn, displayname)
    # Use the confidence from the search engine if it beats the classifier
    if conf > c.get("confidence", 0):
        c["confidence"] = conf

    cat = clothing_category(c["type"], c["material"], cn)
    thick = estimate_thickness(c["rha_mm"], c["material"])
    nij = nij_rating(c["rha_mm"])

    lines.append(
        f"{cn}\t{c['type']}\t{cat}\t{c['material']}\t"
        f"{c.get('backing', '')}\t{thick:.1f}\t{nij}\t"
        f"{c['rha_mm']:.0f}\t{c['confidence']:.2f}\t{mfg}\t{model}\t{camo}"
    )

with open("data/ir_clothing.tsv", "w") as f:
    f.write("\n".join(lines) + "\n")

stats = {}
for l in lines[1:]:
    cat = l.split("\t")[2]
    stats[cat] = stats.get(cat, 0) + 1

print(f"Generated ir_clothing.tsv with {len(lines) - 1} entries")
print(f"Categories: {json.dumps(stats, indent=2)}")
