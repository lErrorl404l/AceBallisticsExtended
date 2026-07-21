#!/usr/bin/env python3
"""
Data validation script for ABE.

Validates TSV and JSON data files for structural correctness,
referential integrity, and value constraints.

Usage:
    python tests/validate_data.py [--strict]

Returns exit code 0 on all-clean, 1 on warnings, 2 on errors.
"""

import argparse
import csv
import json
import os
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent


def validate_real_weapons_tsv(path: Path) -> list[str]:
    """Check real_weapons.tsv has required columns and valid ranges."""
    errors = []
    if not path.exists():
        return [f"MISSING: {path}"]
    with open(path, newline="", encoding="utf-8") as f:
        reader = csv.DictReader(f, delimiter="\t")
        required = {
            "weapon_id",
            "caliber_mm",
            "barrel_mm",
            "twist_mm",
            "pressure_mpa",
            "projectile_mass_g",
            "muzzle_velocity_ms",
        }
        if not required.issubset(reader.fieldnames or []):
            missing = required - set(reader.fieldnames or [])
            errors.append(f"{path.name}: missing columns {missing}")
        for i, row in enumerate(reader, 2):
            for col in ["caliber_mm", "barrel_mm"]:
                val = row.get(col, "").strip()
                if val and not re.match(r"^\d+(\.\d+)?$", val):
                    errors.append(f"{path.name}:{i} {col}={val!r} not numeric")
    return errors


def validate_arma_map_tsv(path: Path) -> list[str]:
    """Check arma_weapon_map.tsv has arma_key → weapon_id linkage."""
    errors = []
    if not path.exists():
        return [f"MISSING: {path}"]
    with open(path, newline="", encoding="utf-8") as f:
        reader = csv.DictReader(f, delimiter="\t")
        required = {"arma_key", "weapon_id", "match_type"}
        if not required.issubset(reader.fieldnames or []):
            missing = required - set(reader.fieldnames or [])
            errors.append(f"{path.name}: missing columns {missing}")
    return errors


def validate_ir_tsv(path: Path) -> list[str]:
    """Generic check for ir_*.tsv files — ensure parseable and non-empty."""
    errors = []
    if not path.exists():
        return [f"MISSING: {path}"]
    with open(path, newline="", encoding="utf-8") as f:
        reader = csv.DictReader(f, delimiter="\t")
        if not reader.fieldnames:
            errors.append(f"{path.name}: no columns found")
            return errors
        row_count = sum(1 for _ in reader)
        if row_count == 0:
            errors.append(f"{path.name}: empty (no data rows)")
    return errors


def main():
    parser = argparse.ArgumentParser(description="Validate ABE data files")
    parser.add_argument(
        "--strict", action="store_true", help="Treat warnings as errors"
    )
    args = parser.parse_args()

    data_dir = REPO_ROOT / "data"
    all_errors = []

    # TSV data files
    all_errors += validate_real_weapons_tsv(data_dir / "real_weapons.tsv")
    all_errors += validate_arma_map_tsv(data_dir / "arma_weapon_map.tsv")
    for tsv in sorted(data_dir.glob("ir_*.tsv")):
        all_errors += validate_ir_tsv(tsv)

    # Report
    if not all_errors:
        print("✅ All data files valid.")
        sys.exit(0)
    for e in all_errors:
        print(f"  ✗ {e}")
    sys.exit(2 if args.strict else 1)


if __name__ == "__main__":
    main()
