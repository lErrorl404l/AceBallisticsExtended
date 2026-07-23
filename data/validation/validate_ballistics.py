#!/usr/bin/env python3
"""
Ballistic data validation engine.
Compares every entry in real_weapons.tsv against SAAMI/CIP cartridge reference ranges.
Produces quality grades and a detailed outlier report.

Usage:
    python data/validation/validate_ballistics.py
    python data/validation/validate_ballistics.py --qmd  # generate Quarto report
"""

import argparse
import csv
import json
import math
import os
import re
import sys
from collections import defaultdict, Counter
from datetime import datetime
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
DATA_DIR = REPO_ROOT / "data"
VALIDATION_DIR = REPO_ROOT / "data" / "validation"
TSV_PATH = DATA_DIR / "real_weapons.tsv"
REF_PATH = VALIDATION_DIR / "cartridge_ref.json"


def load_reference():
    """Load cartridge reference with alias lookup."""
    with open(REF_PATH) as f:
        ref = json.load(f)
    # Build alias map
    alias_map = {}
    for key, spec in ref.items():
        alias_map[key.lower()] = key
        for alias in spec.get("aliases", []):
            alias_map[alias.lower()] = key
    return ref, alias_map


def find_cartridge(cartridge, ref, alias_map):
    """Find the reference key for a cartridge name."""
    c = cartridge.strip().lower()
    if c in alias_map:
        return alias_map[c]
    # Try direct key match
    if c in ref:
        return c
    # Try partial match
    for key in ref:
        if c in key.lower() or key.lower() in c:
            return key
    return None


def get_range(spec, field):
    """Get min/max/typical for a field from a reference spec."""
    if field not in spec:
        return None, None, None
    return spec[field].get("min"), spec[field].get("max"), spec[field].get("typical")


def grade_entry(row, ref_spec, cart_key):
    """
    Grade a single catalog entry against cartridge reference.
    Returns (grade, issues) where grade is A/B/C/F and issues is a list of (field, message).
    """
    issues = []
    severity = 0  # 0=ok, 1=minor, 2=major, 3=critical

    wid = row.get("weapon_id", "?")
    cartridge = row.get("cartridge", "")
    is_shotgun = ref_spec.get("is_shotgun", False) if ref_spec else False

    # Helper to safely parse float
    def pf(name):
        v = row.get(name, "").strip()
        try:
            return float(v)
        except (ValueError, TypeError):
            return None

    caliber = pf("caliber_mm")
    barrel = pf("barrel_mm")
    twist_mm = pf("twist_mm")
    twist_dir = pf("twist_dir")
    pressure = pf("pressure_mpa")
    mass = pf("projectile_mass_g")
    velocity = pf("muzzle_velocity_ms")

    if not ref_spec:
        return "?", [(f"unknown cartridge: {cartridge}", 7)]

    # ── Check 1: Caliber vs cartridge reference ──
    lo, hi, typ = get_range(ref_spec, "caliber_mm")
    if caliber and lo and hi:
        if caliber < lo * 0.85 or caliber > hi * 1.15:
            issues.append(
                (f"caliber_mm={caliber} outside ref [{lo},{hi}] for {cart_key}", 3)
            )
            severity = max(severity, 3)
        elif caliber < lo or caliber > hi:
            issues.append((f"caliber_mm={caliber} slightly outside ref [{lo},{hi}]", 1))

    # ── Check 2: Twist direction ──
    if twist_dir is not None and twist_dir not in (0, 1, 2):
        issues.append((f"twist_dir={twist_dir} invalid (must be 0/1/2)", 2))
        severity = max(severity, 2)
    ref_td = ref_spec.get("twist_dir")
    if ref_td is not None and twist_dir is not None and twist_dir != ref_td:
        if twist_dir == 0 and ref_td != 0:
            issues.append(
                (
                    f"twist_dir=0 (smoothbore) but {cart_key} expects rifled (dir={ref_td})",
                    2,
                )
            )
            severity = max(severity, 2)
        elif twist_dir != 0 and ref_td == 0:
            issues.append(
                (
                    f"twist_dir={twist_dir} (rifled) but {cart_key} expects smoothbore (dir=0)",
                    2,
                )
            )
            severity = max(severity, 2)

    # ── Check 3: Pressure ──
    lo, hi, typ = get_range(ref_spec, "pressure_mpa")
    if pressure and pressure > 0 and lo and hi:
        if pressure < lo * 0.7 or pressure > hi:
            issues.append(
                (f"pressure={pressure} MPa outside ref [{lo},{hi}] for {cart_key}", 3)
            )
            severity = max(severity, 3)
        elif pressure < lo or pressure > hi * 1.1:
            # Check if it's within +P range
            if pressure > hi * 1.05:
                issues.append(
                    (
                        f"pressure={pressure} > {cart_key} max ({hi} MPa) - possible +P",
                        1,
                    )
                )
            else:
                issues.append(
                    (f"pressure={pressure} slightly below {cart_key} min ({lo})", 1)
                )
        # Check vs typical
        if typ and pressure and abs(pressure - typ) / typ > 0.3:
            issues.append(
                (
                    f"pressure={pressure} deviates {abs(pressure - typ) / typ * 100:.0f}% from typical {typ}",
                    1,
                )
            )

    # ── Check 4: Projectile mass ──
    lo, hi, typ = get_range(ref_spec, "projectile_mass_g")
    if mass and mass > 0 and lo and hi:
        if mass < lo * 0.6 or mass > hi * 1.4:
            issues.append((f"mass={mass}g outside ref [{lo},{hi}] for {cart_key}", 3))
            severity = max(severity, 3)
        elif mass < lo * 0.8 or mass > hi * 1.2:
            issues.append((f"mass={mass}g unusual for {cart_key} (ref [{lo},{hi}])", 2))
            severity = max(severity, 2)

    # ── Check 5: Muzzle velocity ──
    lo, hi, typ = get_range(ref_spec, "muzzle_velocity_ms")
    if velocity and velocity > 0 and lo and hi:
        if velocity < lo * 0.5 or velocity > hi * 1.3:
            issues.append(
                (f"velocity={velocity}m/s outside ref [{lo},{hi}] for {cart_key}", 3)
            )
            severity = max(severity, 3)
        elif velocity < lo * 0.75 or velocity > hi * 1.15:
            issues.append(
                (f"velocity={velocity}m/s unusual for {cart_key} (ref [{lo},{hi}])", 2)
            )
            severity = max(severity, 2)

    # ── Check 6: Barrel length sanity ──
    if barrel and barrel > 0:
        if barrel < 30 and not is_shotgun:
            issues.append((f"barrel={barrel}mm extremely short", 2))
            severity = max(severity, 2)
        elif barrel > 1200:
            issues.append((f"barrel={barrel}mm extremely long", 2))
            severity = max(severity, 2)

    # ── Check 7: Velocity vs barrel length consistency ──
    if velocity and barrel and velocity > 0 and barrel > 0:
        if not is_shotgun and caliber and caliber < 15:
            # This is a rough heuristic
            expected_approx = min(velocity, 300 + (barrel - 100) * 0.5)
            # Flag very short barrel + high velocity for rifle calibers
            if barrel < 100 and velocity > 500 and caliber < 10:
                issues.append(
                    (
                        f"short barrel ({barrel}mm) + high velocity ({velocity}m/s) for caliber {caliber}",
                        1,
                    )
                )

    # ── Check 8: Shotgun-specific ──
    if is_shotgun:
        if twist_mm and twist_mm > 0 and twist_dir != 0:
            # Rifled shotgun barrel is OK
            pass

    # ── Assign grade ──
    if severity >= 3:
        grade = "F"
    elif severity >= 2:
        grade = "C"
    elif severity >= 1:
        grade = "B"
    else:
        grade = "A"

    return grade, issues


def validate_all():
    """Run full validation against real_weapons.tsv."""
    ref, alias_map = load_reference()

    results = []
    cart_stats = defaultdict(lambda: Counter())
    grade_counts = Counter()
    all_issues = []

    with open(TSV_PATH, newline="", encoding="utf-8") as f:
        reader = csv.DictReader(f, delimiter="\t")
        for row_num, row in enumerate(reader, 2):
            wid = row.get("weapon_id", "?")
            cartridge = row.get("cartridge", "")
            manufacturer = row.get("manufacturer", "")
            model = row.get("model", "")
            variant = row.get("variant", "")

            # Find cartridge in reference
            cart_key = find_cartridge(cartridge, ref, alias_map)
            ref_spec = ref.get(cart_key) if cart_key else None

            # Grade entry
            grade, issues = grade_entry(row, ref_spec, cart_key or cartridge)

            results.append(
                {
                    "row": row_num,
                    "weapon_id": wid,
                    "manufacturer": manufacturer,
                    "model": model,
                    "variant": variant,
                    "cartridge": cartridge,
                    "cart_key": cart_key or "UNKNOWN",
                    "grade": grade,
                    "issues": [msg for msg, _ in issues],
                    "severity": max([s for _, s in issues], default=0),
                }
            )

            grade_counts[grade] += 1
            cart_stats[cart_key or "UNKNOWN"]["total"] += 1
            cart_stats[cart_key or "UNKNOWN"][grade] += 1

            for msg, sev in issues:
                all_issues.append(
                    {
                        "weapon_id": wid,
                        "cartridge": cartridge,
                        "row": row_num,
                        "severity": sev,
                        "message": msg,
                    }
                )

    return results, grade_counts, cart_stats, all_issues


def print_summary(grade_counts, total):
    """Print a text summary."""
    print(f"\n{'=' * 60}")
    print(f"BALLISTIC DATA VALIDATION REPORT")
    print(f"{'=' * 60}")
    print(f"Total entries: {total}")
    print(
        f"Grade A (clean): {grade_counts['A']} ({grade_counts['A'] / total * 100:.1f}%)"
    )
    print(
        f"Grade B (minor): {grade_counts['B']} ({grade_counts['B'] / total * 100:.1f}%)"
    )
    print(
        f"Grade C (warn):  {grade_counts['C']} ({grade_counts['C'] / total * 100:.1f}%)"
    )
    print(
        f"Grade F (fail):  {grade_counts['F']} ({grade_counts['F'] / total * 100:.1f}%)"
    )
    print(
        f"Grade ? (no ref): {grade_counts['?']} ({grade_counts['?'] / total * 100:.1f}%)"
    )
    print()


def write_csv_report(results, path):
    """Write detailed results as TSV for downstream processing."""
    with open(path, "w", newline="") as f:
        writer = csv.writer(f, delimiter="\t")
        writer.writerow(
            [
                "row",
                "weapon_id",
                "manufacturer",
                "model",
                "cartridge",
                "grade",
                "severity",
                "issues",
            ]
        )
        for r in results:
            writer.writerow(
                [
                    r["row"],
                    r["weapon_id"],
                    r["manufacturer"],
                    r["model"],
                    r["cartridge"],
                    r["grade"],
                    r["severity"],
                    "; ".join(r["issues"]) if r["issues"] else "",
                ]
            )


def generate_qmd(results, grade_counts, cart_stats, all_issues, total, path):
    """Generate a Quarto .qmd report file."""
    timestamp = datetime.now().strftime("%Y-%m-%d %H:%M")
    a_pct = grade_counts["A"] / total * 100 if total else 0
    b_pct = grade_counts["B"] / total * 100 if total else 0
    c_pct = grade_counts["C"] / total * 100 if total else 0
    f_pct = grade_counts["F"] / total * 100 if total else 0

    # Top issues by cartridge
    cart_fails = sorted(
        [
            (c, stats)
            for c, stats in cart_stats.items()
            if stats.get("F", 0) > 0 or stats.get("C", 0) > 0
        ],
        key=lambda x: -(x[1]["F"] + x[1]["C"]),
    )

    # Top failing entries
    failing = [r for r in results if r["grade"] in ("F", "C")]

    with open(path, "w") as f:
        f.write(f"""---
title: "Ballistic Data Validation Report"
subtitle: "AceBallisticsExtention — real_weapons.tsv"
date: "{timestamp}"
format:
  html:
    toc: true
    toc-depth: 3
    number-sections: true
    embed-resources: true
    theme: cosmo
---

## Executive Summary

| Metric | Value |
|--------|-------|
| **Total entries** | {total} |
| **Grade A (clean)** | {grade_counts["A"]} ({a_pct:.1f}%) |
| **Grade B (minor)** | {grade_counts["B"]} ({b_pct:.1f}%) |
| **Grade C (warning)** | {grade_counts["C"]} ({c_pct:.1f}%) |
| **Grade F (fail)** | {grade_counts["F"]} ({f_pct:.1f}%) |
| **Uncategorized** | {grade_counts.get("?", 0)} |
| **Total issues flagged** | {len(all_issues)} |

## Quality By Cartridge

| Cartridge | Total | A | B | C | F |
|----------|------|---|---|---|---|
""")

        for cart, stats in sorted(cart_stats.items(), key=lambda x: -x[1]["total"]):
            if cart == "UNKNOWN":
                continue
            f.write(
                f"| {cart} | {stats['total']} | {stats['A']} | {stats['B']} | {stats['C']} | {stats['F']} |\n"
            )

        # Unknown cartridges section
        unknown = cart_stats.get("UNKNOWN", {})
        if unknown.get("total", 0) > 0:
            f.write(f"""
### Unknown Cartridges
The following entries use cartridge names not found in the reference table. These need cartridge_ref.json entries added.

> {unknown["total"]} entries uncategorized.
""")

        f.write(f"""
## Failed Entries (Grade F)

These entries have values significantly outside SAAMI/CIP reference ranges.

""")

        for r in failing[:50]:
            issues = "; ".join(r["issues"]) if r["issues"] else "No issues"
            f.write(
                f"- **{r['weapon_id']}** ({r['manufacturer']} {r['model']}) — {r['cartridge']} — [{r['grade']}] {issues}\n"
            )

        if len(failing) > 50:
            f.write(f"\n*...and {len(failing) - 50} more failing entries*\n")

        f.write(f"""
## Grade C Entries (Warnings)

Entry with secondary issues that should be investigated.

""")

        warning_entries = [r for r in results if r["grade"] == "C"]
        if warning_entries:
            f.write("| Weapon ID | Manufacturer | Model | Cartridge | Issues |\n")
            f.write("|-----------|-------------|-------|-----------|--------|\n")
            for r in warning_entries[:30]:
                issues = "; ".join(r["issues"]) if r["issues"] else ""
                f.write(
                    f"| {r['weapon_id']} | {r['manufacturer']} | {r['model']} | {r['cartridge']} | {issues} |\n"
                )
        else:
            f.write("*No grade C entries.*\n")

        f.write(f"""
## All Issues by Severity

{len([x for x in all_issues if x["severity"] >= 3])} critical (severity 3) \\
{len([x for x in all_issues if x["severity"] == 2])} major (severity 2) \\
{len([x for x in all_issues if x["severity"] == 1])} minor (severity 1)

### Critical Issues (Severity 3)

""")

        critical = [x for x in all_issues if x["severity"] >= 3]
        if critical:
            f.write("| Weapon ID | Cartridge | Issue |\n")
            f.write("|-----------|----------|-------|\n")
            for x in critical:
                f.write(f"| {x['weapon_id']} | {x['cartridge']} | {x['message']} |\n")
        else:
            f.write("*No critical issues.*\n")

        f.write(f"""
---
*Report generated by validate_ballistics.py on {timestamp}.*
*Reference: cartridge_ref.json (SAAMI/CIP data)*
""")

    print(f"  → Quarto report written to {path}")


def main():
    parser = argparse.ArgumentParser(description="Validate ballistic data")
    parser.add_argument("--qmd", action="store_true", help="Generate Quarto report")
    args = parser.parse_args()

    print("Validating real_weapons.tsv...")
    results, grade_counts, cart_stats, all_issues = validate_all()
    total = len(results)

    print_summary(grade_counts, total)

    # Write TSV results
    tsv_path = VALIDATION_DIR / "validation_results.tsv"
    write_csv_report(results, tsv_path)
    print(f"  → Detailed results: {tsv_path}")

    # Print failing entries
    failing = [r for r in results if r["grade"] == "F"]
    if failing:
        print(f"\n⚠  {len(failing)} GRADE F ENTRIES:")
        for r in failing[:20]:
            issues = "; ".join(r["issues"]) if r["issues"] else ""
            print(f"  ✗ {r['weapon_id']:45s} | {r['cartridge']:20s} | {issues}")
        if len(failing) > 20:
            print(f"  ... and {len(failing) - 20} more")

    warning_entries = [r for r in results if r["grade"] == "C"]
    if warning_entries:
        print(f"\n⚡ {len(warning_entries)} GRADE C ENTRIES:")
        for r in warning_entries[:10]:
            issues = "; ".join(r["issues"]) if r["issues"] else ""
            print(f"  ⚡ {r['weapon_id']:45s} | {r['cartridge']:20s} | {issues}")

    if args.qmd:
        qmd_path = VALIDATION_DIR / "validation_report.qmd"
        generate_qmd(results, grade_counts, cart_stats, all_issues, total, qmd_path)

    # Return exit code based on results
    if grade_counts.get("F", 0) > 0:
        return 2
    elif grade_counts.get("C", 0) > 0:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
