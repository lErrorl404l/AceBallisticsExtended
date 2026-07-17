#!/usr/bin/env python3
"""
Add `twist_rate_mm` field to every weapon JSON file.

Reads the existing rifling twist field (either `rifling_twist_mm` in
snake_case files or `riflingTwistMm` in camelCase files) and copies
its value into a new `twist_rate_mm` field.

Safe to re-run — skips files that already have `twist_rate_mm` set.
"""

import json
import os
import glob

WEAPONS_DIR = os.path.join(
    os.path.dirname(os.path.abspath(__file__)),
    "..",
    "..",
    "data",
    "weapons",
)


def add_twist_rate(filepath: str) -> bool:
    """Add twist_rate_mm to one weapon file. Returns True if changed."""
    with open(filepath, "r") as f:
        data = json.load(f)

    # Already has the new field — skip
    if "twist_rate_mm" in data:
        return False

    # Find existing rifling twist value
    twist = data.get("rifling_twist_mm") or data.get("riflingTwistMm")
    if twist is None:
        print(
            f"  ⚠  {os.path.basename(filepath)} — no rifling twist field found, skipping"
        )
        return False

    data["twist_rate_mm"] = twist

    with open(filepath, "w") as f:
        json.dump(data, f, indent=2)
        f.write("\n")
    return True


def main() -> None:
    pattern = os.path.join(WEAPONS_DIR, "*.json")
    files = sorted(glob.glob(pattern))
    changed = 0
    skipped = 0
    missing = 0

    for fp in files:
        if not os.path.isfile(fp):
            continue
        with open(fp, "r") as f:
            data = json.load(f)
        if "twist_rate_mm" in data:
            skipped += 1
            continue
        twist = data.get("rifling_twist_mm") or data.get("riflingTwistMm")
        if twist is None:
            print(f"  ⚠  {os.path.basename(fp)} — no twist field found")
            missing += 1
            continue
        data["twist_rate_mm"] = twist
        with open(fp, "w") as f:
            json.dump(data, f, indent=2)
            f.write("\n")
        changed += 1

    print(
        f"\nDone — {changed} updated, {skipped} already had twist_rate_mm, {missing} missing twist data"
    )


if __name__ == "__main__":
    main()
