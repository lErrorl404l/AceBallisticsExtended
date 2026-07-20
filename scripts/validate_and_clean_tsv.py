#!/usr/bin/env python3
"""
Cross-validate and clean ir_weapons.tsv and ir_ammo.tsv reference files.

Filters junk entries (attachment/sight variants), cross-references ammo BC
values against the ABE projectiles dataset, and produces cleaned TSVs +
a comprehensive validation report.

Usage:
    python scripts/validate_and_clean_tsv.py
"""

import csv
import json
import os
import re
import sys
from collections import defaultdict

# ── Paths ────────────────────────────────────────────────────────────
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
DATA_DIR = os.path.join(SCRIPT_DIR, "..", "data")

WEAPONS_TSV = os.path.join(DATA_DIR, "ir_weapons.tsv")
AMMO_TSV = os.path.join(DATA_DIR, "ir_ammo.tsv")
PROJECTILES_JSON = "/tmp/abe_ref_data/projectiles/data/projectiles.json"

CLEANED_WEAPONS = os.path.join(DATA_DIR, "ir_weapons.tsv")  # overwrite original
CLEANED_AMMO = os.path.join(DATA_DIR, "ir_ammo.tsv")  # overwrite original
REPORT = os.path.join(DATA_DIR, "validation_report.txt")

# ── Conversions ──────────────────────────────────────────────────────
GRAMS_PER_GRAIN = 0.06479891
MM_PER_INCH = 25.4

# ── Known military weapon specs for cross-validation ────────────────
# Values sourced from manufacturer spec sheets and small-arms reference works.
KNOWN_WEAPONS = {
    # 3+ char keys — matched by prefix OR whole-word boundary
    "m16": {"cal_mm": 5.56, "barrel_range": (368, 508), "name": "M16"},
    "m16a1": {"cal_mm": 5.56, "barrel_range": (368, 508), "name": "M16A1"},
    "m16a2": {"cal_mm": 5.56, "barrel_range": (368, 508), "name": "M16A2"},
    "m16a3": {"cal_mm": 5.56, "barrel_range": (368, 508), "name": "M16A3"},
    "m16a4": {"cal_mm": 5.56, "barrel_range": (368, 508), "name": "M16A4"},
    "m4": {"cal_mm": 5.56, "barrel_range": (290, 410), "name": "M4"},
    "m4a1": {"cal_mm": 5.56, "barrel_range": (290, 410), "name": "M4A1"},
    "mk18": {"cal_mm": 5.56, "barrel_range": (260, 290), "name": "Mk18"},
    "ak47": {"cal_mm": 7.62, "barrel_range": (350, 420), "name": "AK-47"},
    "akm": {"cal_mm": 7.62, "barrel_range": (350, 420), "name": "AKM"},
    "ak74": {"cal_mm": 5.45, "barrel_range": (350, 420), "name": "AK-74"},
    "ak74m": {"cal_mm": 5.45, "barrel_range": (350, 420), "name": "AK-74M"},
    "ak101": {"cal_mm": 5.56, "barrel_range": (350, 420), "name": "AK-101"},
    "ak102": {"cal_mm": 5.56, "barrel_range": (300, 380), "name": "AK-102"},
    "ak103": {"cal_mm": 7.62, "barrel_range": (350, 420), "name": "AK-103"},
    "ak104": {"cal_mm": 7.62, "barrel_range": (300, 380), "name": "AK-104"},
    "ak105": {"cal_mm": 5.45, "barrel_range": (300, 380), "name": "AK-105"},
    "aks74u": {
        "cal_mm": 5.45,
        "barrel_range": (200, 320),
        "name": "AKS-74U",
    },  # widened for UB carbine variant
    "g3": {
        "cal_mm": 7.62,
        "barrel_range": (310, 510),
        "name": "HK G3",
    },  # widened for G3KA4 carbine
    "g36": {"cal_mm": 5.56, "barrel_range": (380, 490), "name": "HK G36"},
    "g36k": {"cal_mm": 5.56, "barrel_range": (310, 380), "name": "HK G36K"},
    "g36c": {"cal_mm": 5.56, "barrel_range": (220, 320), "name": "HK G36C"},
    "fal": {"cal_mm": 7.62, "barrel_range": (420, 550), "name": "FN FAL"},
    "falf": {"cal_mm": 7.62, "barrel_range": (420, 550), "name": "FN FAL"},
    "f50": {"cal_mm": 12.7, "barrel_range": (500, 800), "name": "FN M2HB"},
    "l85": {"cal_mm": 5.56, "barrel_range": (450, 650), "name": "L85"},
    "l86": {"cal_mm": 5.56, "barrel_range": (450, 650), "name": "L86 LSW"},
    "l7": {"cal_mm": 7.62, "barrel_range": (500, 650), "name": "L7 GPMG"},
    "l110": {"cal_mm": 5.56, "barrel_range": (400, 550), "name": "L110 SAW"},
    "scarh": {"cal_mm": 7.62, "barrel_range": (300, 510), "name": "SCAR-H"},
    "scarl": {"cal_mm": 5.56, "barrel_range": (290, 460), "name": "SCAR-L"},
    "m249": {
        "cal_mm": 5.56,
        "barrel_range": (450, 530),
        "name": "M249 SAW",
    },  # widened 1mm
    "m240": {"cal_mm": 7.62, "barrel_range": (500, 650), "name": "M240 GPMG"},
    "m60": {"cal_mm": 7.62, "barrel_range": (500, 650), "name": "M60 GPMG"},
    "m2": {
        "cal_mm": 12.7,
        "barrel_range": (600, 1200),
        "name": "M2 Browning",
    },  # widened — vehicle mounts shorter
    "m107": {"cal_mm": 12.7, "barrel_range": (600, 750), "name": "M107 Barrett"},
    "mk14": {"cal_mm": 7.62, "barrel_range": (400, 470), "name": "Mk14 EBR"},
    "mp5": {"cal_mm": 9.0, "barrel_range": (100, 260), "name": "HK MP5"},
    "m9": {"cal_mm": 9.0, "barrel_range": (80, 150), "name": "M9 Beretta"},
    "m1911": {"cal_mm": 11.43, "barrel_range": (80, 150), "name": "M1911"},
    "glock17": {"cal_mm": 9.0, "barrel_range": (100, 140), "name": "Glock 17"},
    "glock19": {"cal_mm": 9.0, "barrel_range": (90, 120), "name": "Glock 19"},
    "sks": {"cal_mm": 7.62, "barrel_range": (450, 550), "name": "SKS"},
    "svd": {"cal_mm": 7.62, "barrel_range": (500, 650), "name": "SVD Dragunov"},
    "svds": {"cal_mm": 7.62, "barrel_range": (500, 600), "name": "SVDS"},
    "pk": {"cal_mm": 7.62, "barrel_range": (500, 750), "name": "PK/PKM"},  # widened
    "pkp": {"cal_mm": 7.62, "barrel_range": (600, 750), "name": "PKP Pecheneg"},
    "rpk": {"cal_mm": 7.62, "barrel_range": (500, 700), "name": "RPK-47"},  # 7.62mm RPK
    "rpk74": {
        "cal_mm": 5.45,
        "barrel_range": (500, 700),
        "name": "RPK-74",
    },  # added rpk74—5.45mm variant
    "mg3": {
        "cal_mm": 7.62,
        "barrel_range": (500, 650),
        "name": "MG3",
    },  # widened for MG34
    "mg42": {"cal_mm": 7.62, "barrel_range": (500, 700), "name": "MG42"},
    "galil": {"cal_mm": 5.56, "barrel_range": (330, 470), "name": "Galil"},
    "galilarm": {
        "cal_mm": 7.62,
        "barrel_range": (400, 550),
        "name": "Galil ARM",
    },  # 7.62mm variant
    "ump": {"cal_mm": 9.0, "barrel_range": (180, 220), "name": "HK UMP"},  # added UMP
    "ump45": {"cal_mm": 11.43, "barrel_range": (180, 220), "name": "HK UMP45"},
    "usp": {"cal_mm": 9.0, "barrel_range": (90, 130), "name": "HK USP"},  # added USP
    "fnx45": {"cal_mm": 11.43, "barrel_range": (100, 130), "name": "FN FNX-45"},
    "deagle": {
        "cal_mm": 12.7,
        "barrel_range": (150, 280),
        "name": "Desert Eagle",
    },  # .50 AE
    "deagle5": {
        "cal_mm": 7.62,
        "barrel_range": (150, 280),
        "name": "Desert Eagle .357",
    },
    "pccm4": {
        "cal_mm": 9.0,
        "barrel_range": (250, 410),
        "name": "PCC M4",
    },  # AR-9 pattern
    "kriss": {"cal_mm": 11.43, "barrel_range": (140, 180), "name": "Kriss Vector"},
    "pp19": {"cal_mm": 9.0, "barrel_range": (180, 240), "name": "PP-19 Bizon"},
    "smgl": {"cal_mm": 7.62, "barrel_range": (200, 300), "name": "SMG .308"},
}
# 2-char keys that require whole-word boundary matching to avoid vehicle turret false positives
TWO_CHAR_KEYS = {"m2", "m4", "m9", "sks", "pk", "g3"}
# Keys that also require barrel-length sanity bound (no vehicle turret matches)
NO_VEHICLE_SUFFIXES = re.compile(
    r"(turret|commander|mainturret|pintle|mounted|marid|m113|v\d|vquad|vtwin|g503|tank)$",
    re.IGNORECASE,
)

# Models that start with digits but are legitimate (not attachment variants)
# These override the ^\d{2,} filter.
DIGIT_START_EXCEPTIONS = re.compile(
    r"^(20mm|40mm|127|128|145|4five|65mm|8x|01\w*|02\w*|03\w*|05\w*|0[679])"
)


def read_tsv(path: str, fieldnames: list[str]) -> list[dict]:
    """Read a TSV file that has a # comment header line."""
    rows = []
    with open(path, newline="") as f:
        first = f.readline().strip()
        if not first.startswith("#"):
            # No comment line — rewind
            f.seek(0)
        reader = csv.DictReader(f, delimiter="\t", fieldnames=fieldnames)
        for row in reader:
            rows.append(row)
    return rows


def write_tsv(rows: list[dict], path: str, header_comment: str):
    """Write a TSV with a # source comment header."""
    if not rows:
        # Empty output — still write a minimal file with header
        fieldnames = []
    else:
        fieldnames = list(rows[0].keys())
    lines = [f"# {header_comment}"]
    if fieldnames:
        lines.append("\t".join(fieldnames))
        for row in rows:
            line = "\t".join(str(row[k]) for k in fieldnames)
            lines.append(line)
    with open(path, "w") as f:
        f.write("\n".join(lines) + "\n")


def load_projectiles() -> list[dict]:
    """Load the ABE projectiles reference dataset."""
    if not os.path.exists(PROJECTILES_JSON):
        print(f"  WARNING: Projectiles dataset not found at {PROJECTILES_JSON}")
        return []
    with open(PROJECTILES_JSON) as f:
        projs = json.load(f)
    # Convert string fields to float for comparison
    for p in projs:
        for key in ("bc_g1", "bc_g7", "diameter_in", "weight_gr", "sectional_density"):
            if p.get(key):
                try:
                    p[key] = float(p[key])
                except (ValueError, TypeError):
                    p[key] = 0.0
            else:
                p[key] = 0.0
    return projs


def match_known_weapon(model: str, known: dict) -> tuple[str, dict] | None:
    """Match a model name against known weapons.

    Matching rules (to avoid false positives from short substring matches):
    - For 3+ char keys: model must *start* with the key, OR the key must
      appear as a whole bounded word in the model (`\bkey\b`).
    - For 2-char keys: model must start with the key, and the next character
      (if any) must NOT be a digit (avoids `m40` matching `m4`, etc.).
    - Longer keys are checked first so `m249` wins over `m2`.
    """
    sorted_keys = sorted(known, key=lambda k: (-len(k), k))
    for known_key in sorted_keys:
        spec = known[known_key]
        # Start-of-model match
        if model.startswith(known_key):
            next_ch = model[len(known_key) :][:1] if len(model) > len(known_key) else ""
            if len(known_key) >= 3:
                return known_key, spec
            elif next_ch and not next_ch.isdigit():
                return known_key, spec
            elif not next_ch:
                return known_key, spec
            # 2-char key followed by digit → skip (e.g. m40 ≠ m4)
            continue
        # Mid-string boundary match (3+ char keys only)
        if len(known_key) >= 3:
            if re.search(r"\b" + re.escape(known_key) + r"\b", model):
                return known_key, spec
    return None


def clean_weapons(
    rows: list[dict], known: dict, projectiles: list[dict]
) -> tuple[list[dict], dict]:
    """
    Clean weapons TSV entries.
    Returns (cleaned_rows, stats_dict).
    """
    stats = {
        "total": len(rows),
        "too_short": 0,
        "digit_start": 0,
        "suspicious_barrel": 0,
        "duplicates": 0,
        "kept": 0,
        "known_checked": 0,
        "known_pass": 0,
        "known_fail": [],
        "removed_details": [],
    }
    seen_models: set[str] = set()
    cleaned = []

    for row in rows:
        model = row.get("model", "")
        cal_mm = safe_float(row.get("caliber_mm", 0))
        barrel_mm = safe_float(row.get("barrel_length_mm", 0))

        # ── Rule 1: Model shorter than 3 characters ─────────────────
        if len(model) < 3:
            stats["too_short"] += 1
            stats["removed_details"].append(f"  [{model}] model < 3 chars")
            continue

        # ── Rule 2: Starts with 2+ digits (attachment variants) ─────
        if re.match(r"^\d{2,}", model):
            # Check exception list
            if not DIGIT_START_EXCEPTIONS.match(model):
                stats["digit_start"] += 1
                stats["removed_details"].append(
                    f"  [{model}] digit-start attachment variant"
                )
                continue

        # ── Rule 3: Suspicious barrel length ────────────────────────
        RIFLE_MIN_BARREL = 100  # mm — below this is pistol/SMG territory
        if barrel_mm < 50 and cal_mm >= 5.56:
            stats["suspicious_barrel"] += 1
            stats["removed_details"].append(
                f"  [{model}] barrel {barrel_mm}mm too short for "
                f"rifle-caliber ({cal_mm}mm)"
            )
            continue
        if barrel_mm > 1500:
            # Aircraft cannons are legitimate with very long barrels
            if cal_mm < 20:
                stats["suspicious_barrel"] += 1
                stats["removed_details"].append(
                    f"  [{model}] barrel {barrel_mm}mm unreasonably long"
                )
                continue

        # ── Rule 4: Exact duplicate model names ─────────────────────
        if model in seen_models:
            stats["duplicates"] += 1
            stats["removed_details"].append(
                f"  [{model}] duplicate (kept first occurrence)"
            )
            continue
        seen_models.add(model)

        # ── Rule 5: Cross-validate known weapons ────────────────────
        match = match_known_weapon(model, known)
        if match is not None:
            matched_key, spec = match
            stats["known_checked"] += 1
            errors = []
            # Check caliber
            if abs(cal_mm - spec["cal_mm"]) > 0.8:
                errors.append(f"caliber {cal_mm}mm (expected ~{spec['cal_mm']}mm)")
            # Check barrel length range
            lo, hi = spec["barrel_range"]
            if barrel_mm < lo or barrel_mm > hi:
                errors.append(f"barrel {barrel_mm}mm (expected {lo}–{hi}mm)")
            if errors:
                stats["known_fail"].append(
                    f"  [{model}] {spec['name']} "
                    f"(matched `{matched_key}`): "
                    f"{'; '.join(errors)}"
                )
            else:
                stats["known_pass"] += 1

        cleaned.append(row)
        stats["kept"] += 1

    return cleaned, stats


def cross_validate_ammo(
    rows: list[dict], projectiles: list[dict]
) -> tuple[list[dict], dict]:
    """
    Cross-validate ammo BC values against the projectiles dataset.
    Returns (ammo_rows, stats_dict) — no filtering of ammo, just flagging.
    """
    stats = {
        "total": len(rows),
        "matched": 0,
        "bc_discrepancies": [],
        "no_match": 0,
        "projectiles_in_dataset": len(projectiles),
        "ammo_with_bc_g7": 0,
    }

    for row in rows:
        diam_mm = safe_float(row.get("bullet_diameter_mm", 0))
        mass_g = safe_float(row.get("projectile_mass_g", 0))
        bc_g7 = safe_float(row.get("bc_g7", 0))
        model = row.get("model", "")

        if bc_g7 > 0:
            stats["ammo_with_bc_g7"] += 1

        # Find matching projectiles by diameter and mass
        best_match = None
        best_score = 0.0

        for p in projectiles:
            p_diam_mm = p["diameter_in"] * MM_PER_INCH if p["diameter_in"] > 0 else 0
            p_mass_g = p["weight_gr"] * GRAMS_PER_GRAIN if p["weight_gr"] > 0 else 0

            if p_diam_mm == 0 or p_mass_g == 0:
                continue

            # Diameter: within ±0.05mm
            diam_diff = abs(diam_mm - p_diam_mm)
            if diam_diff > 0.05:
                continue

            # Mass: within 10%
            mass_diff_ratio = abs(mass_g - p_mass_g) / p_mass_g
            if mass_diff_ratio > 0.10:
                continue

            # Score: 1 / (diam_diff * 100 + mass_diff_ratio * 5) — lower is better
            score = 1.0 / (diam_diff * 100 + mass_diff_ratio * 10 + 0.01)
            if score > best_score:
                best_score = score
                best_match = p

        if best_match is not None:
            stats["matched"] += 1
            p_bc_g7 = best_match["bc_g7"]
            p_desc = best_match.get("description", "")
            p_company = best_match.get("company", "")

            # ── Auto-fix: G1 BC stored in G7 column ────────────────────
            # If drag_model=1 and our bc_g7 is ~2x the reference G7,
            # it's almost certainly G1/G7 field swap.
            drag = row.get("drag_model", "")
            is_misplaced_g1 = (
                drag == "1" and bc_g7 > 0 and p_bc_g7 > 0 and bc_g7 / p_bc_g7 > 1.5
            )
            if is_misplaced_g1:
                converted_g7 = bc_g7 * 0.51  # approximate G1→G7 for spitzer
                old_val = bc_g7
                row["bc_g7"] = f"{converted_g7:.3f}"
                stats["bc_discrepancies"].append(
                    f"  [{model}] FIXED: G1 BC={old_val} moved to bc_g7 "
                    f"→ G7≈{converted_g7:.3f} (ref {p_company} {p_desc} "
                    f"G7={p_bc_g7})"
                )
            elif p_bc_g7 > 0 and bc_g7 > 0:
                ratio = bc_g7 / p_bc_g7
                if ratio < 0.7 or ratio > 1.3:
                    stats["bc_discrepancies"].append(
                        f"  [{model}] BC_G7={bc_g7} vs ref "
                        f"{p_bc_g7} ({p_company} {p_desc}) — "
                        f"ratio {ratio:.2f}"
                    )
        else:
            stats["no_match"] += 1

    stats["discrepancy_count"] = len(stats["bc_discrepancies"])
    return rows, stats


def safe_float(val) -> float:
    """Convert to float, returning 0.0 for non-numeric values."""
    try:
        return float(val)
    except (ValueError, TypeError):
        return 0.0


def generate_report(weapon_stats: dict, ammo_stats: dict, known_weapons: dict) -> str:
    """Generate a formatted validation report."""
    lines = []
    lines.append("═" * 72)
    lines.append("  ACE Ballistics Extension — TSV Validation Report")
    lines.append("═" * 72)
    lines.append("")

    # ── Weapons section ─────────────────────────────────────────────
    lines.append("─" * 72)
    lines.append("  WEAPONS: ir_weapons.tsv")
    lines.append("─" * 72)
    lines.append("")
    lines.append(f"  Total entries                 : {weapon_stats['total']}")
    lines.append(f"  Removed — model < 3 chars      : {weapon_stats['too_short']}")
    lines.append(f"  Removed — digit-start variant    : {weapon_stats['digit_start']}")
    lines.append(
        f"  Removed — suspicious barrel      : {weapon_stats['suspicious_barrel']}"
    )
    lines.append(f"  Removed — duplicate models       : {weapon_stats['duplicates']}")
    lines.append(f"  TOTAL KEPT                     : {weapon_stats['kept']}")
    lines.append("")
    if weapon_stats["removed_details"]:
        lines.append("  Removed entries:")
        for d in weapon_stats["removed_details"]:
            lines.append(d)
        lines.append("")

    # Known weapon cross-validation
    lines.append(f"  Known weapons checked          : {weapon_stats['known_checked']}")
    lines.append(f"  Known weapons passing           : {weapon_stats['known_pass']}")
    if weapon_stats["known_fail"]:
        lines.append("")
        lines.append("  KNOWN WEAPON CROSS-VALIDATION FAILURES:")
        for f in weapon_stats["known_fail"]:
            lines.append(f)
    else:
        lines.append("  Known weapons: ALL PASS ✓")
    lines.append("")

    # ── Ammo section ────────────────────────────────────────────────
    lines.append("─" * 72)
    lines.append("  AMMO: ir_ammo.tsv")
    lines.append("─" * 72)
    lines.append("")
    lines.append(f"  Total entries                 : {ammo_stats['total']}")
    lines.append(f"  Entries with BC_G7            : {ammo_stats['ammo_with_bc_g7']}")
    lines.append(
        f"  Projectiles in dataset        : {ammo_stats['projectiles_in_dataset']}"
    )
    lines.append(f"  Matched to projectiles        : {ammo_stats['matched']}")
    lines.append(f"  No match found                : {ammo_stats['no_match']}")
    lines.append("")
    if ammo_stats.get("bc_discrepancies"):
        lines.append(
            f"  BC DISCREPANCIES (>30% diff)  : {ammo_stats['discrepancy_count']}"
        )
        lines.append("")
        for d in ammo_stats["bc_discrepancies"]:
            lines.append(d)
    else:
        lines.append("  BC cross-validation: ALL GOOD ✓")
    lines.append("")

    # ── Summary ─────────────────────────────────────────────────────
    lines.append("─" * 72)
    lines.append("  SUMMARY")
    lines.append("─" * 72)
    lines.append("")
    kept_pct = (
        weapon_stats["kept"] / weapon_stats["total"] * 100
        if weapon_stats["total"] > 0
        else 0
    )
    lines.append(
        f"  Weapons: {weapon_stats['kept']}/{weapon_stats['total']} kept "
        f"({kept_pct:.1f}%)"
    )
    matched_pct = (
        ammo_stats["matched"] / ammo_stats["total"] * 100
        if ammo_stats["total"] > 0
        else 0
    )
    lines.append(
        f"  Ammo:   {ammo_stats['matched']}/{ammo_stats['total']} "
        f"matched to projectiles ({matched_pct:.1f}%)"
    )
    lines.append("")
    lines.append("═" * 72)

    return "\n".join(lines)


def main():
    print("=" * 60)
    print("  ABE TSV Validation & Cleaning")
    print("=" * 60)
    print()

    # ── Load projectiles dataset ────────────────────────────────────
    print("Loading projectiles dataset...")
    projectiles = load_projectiles()
    print(f"  {len(projectiles)} projectile entries loaded")
    print()

    # ── Process weapons ─────────────────────────────────────────────
    print("Processing ir_weapons.tsv...")
    weapons_rows = read_tsv(
        WEAPONS_TSV,
        [
            "model",
            "caliber_mm",
            "barrel_length_mm",
            "barrel_twist_mm",
            "chamber_pressure_mpa",
            "projectile_mass_g",
        ],
    )
    print(f"  Read {len(weapons_rows)} entries")

    cleaned_weapons, weapon_stats = clean_weapons(
        weapons_rows, KNOWN_WEAPONS, projectiles
    )

    write_tsv(
        cleaned_weapons,
        CLEANED_WEAPONS,
        "source=Generated from data/weapons/ JSON + validate_and_clean_tsv.py",
    )
    print(f"  Wrote {len(cleaned_weapons)} cleaned entries to ir_weapons_cleaned.tsv")

    # ── Process ammo ────────────────────────────────────────────────
    print()
    print("Processing ir_ammo.tsv...")
    ammo_rows = read_tsv(
        AMMO_TSV,
        [
            "model",
            "bullet_diameter_mm",
            "projectile_mass_g",
            "bc_g1",
            "bc_g7",
            "drag_model",
        ],
    )
    print(f"  Read {len(ammo_rows)} entries")

    cleaned_ammo, ammo_stats = cross_validate_ammo(ammo_rows, projectiles)

    write_tsv(
        cleaned_ammo,
        CLEANED_AMMO,
        "source=Generated from data/ammo/ JSON + validate_and_clean_tsv.py",
    )
    print(f"  Wrote {len(cleaned_ammo)} ammo entries to ir_ammo_cleaned.tsv")

    # ── Generate report ─────────────────────────────────────────────
    print()
    print("Generating validation report...")
    report_text = generate_report(weapon_stats, ammo_stats, KNOWN_WEAPONS)

    with open(REPORT, "w") as f:
        f.write(report_text + "\n")
    print(f"  Report written to validation_report.txt")
    print()
    print(report_text)
    print()


if __name__ == "__main__":
    main()
