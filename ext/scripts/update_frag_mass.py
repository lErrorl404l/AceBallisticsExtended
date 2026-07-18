#!/usr/bin/env python3
"""Update frag_mass_mean and frag_mass_std for all ammo JSON files.

Categorizes by caliber/type heuristics and updates in-place.
"""

import json
import os
import re

AMMO_DIR = os.path.join(os.path.dirname(__file__), "..", "..", "data", "ammo")


def classify(filename: str, data: dict) -> tuple[float, float]:
    """Determine frag_mass_mean and frag_mass_std based on caliber/type."""
    proj = data.get("projectile", {})
    name = filename.lower()
    cal = proj.get("caliber_mm", 0)

    # --- Shotgun pellets (uniform pellets, frag_mass_mean = pellet_mass) ---
    if "birdshot" in name:
        # ~350 pellets @ ~0.08g each
        return (0.08, 0.0)
    if "buckshot" in name:
        # 9 pellets @ ~3.14g each (28.3g / 9)
        return (3.14, 0.0)

    # --- Shotgun slug ---
    if "slug" in name:
        return (20.0, 5.0)

    # --- Fragmenting 5.56 rounds (M193, MK262, M855A1) ---
    if any(x in name for x in ("m193", "mk262")):
        return (1.0, 0.6)
    if "m855a1" in name:
        return (1.0, 0.6)

    # --- AP rounds ---
    if re.search(r"\bap\b", name) or name.endswith("_m61.json") or "7n22" in name:
        return (1.0, 0.4)

    # --- API rounds ---
    if "api" in name:
        # 127x99_api.json specifically
        return (1.2, 0.5)

    # --- Tracer rounds ---
    if "tracer" in name:
        return (1.2, 0.4)

    # --- Subsonic rounds ---
    if "subsonic" in name:
        return (0.5, 0.3)

    # --- Pistol/PDW rounds (caliber ≤ 9.5mm, low velocity) ---
    if cal <= 9.5 and (
        name.startswith("9mm")
        or name.startswith("9x21")
        or name.startswith("45acp")
        or name.startswith("46x30")
        or name.startswith("57x28")
        or name.startswith("570x28")
        or name.startswith("9mm")
        or "9x21" in name
        or "45acp" in name
    ):
        return (0.5, 0.3)

    # --- Subsonic specialty rounds ---
    if "9x39" in name or "sp5" in name:
        return (0.5, 0.3)

    # --- Intermediate rounds (5.56-7.62x39 class) ---
    # 300 BLK supersonic is 7.62x35mm, comparable to 7.62x39
    if "300_blk_supersonic" in name or "300_blk_sup" in name:
        return (1.5, 0.5)

    # 5.45x39 rounds
    if any(x in name for x in ("545x39", "5.45")):
        return (1.5, 0.5)

    # 5.56x45 rounds (SS109, M855, generic 5.56)
    if any(x in name for x in ("556x45", "5.56", "m855", "ss109")):
        return (1.5, 0.5)

    # 5.8x42
    if "580x42" in name or "5.8" in name:
        return (1.5, 0.5)

    # 6.5x39 intermediate
    if "65x39" in name:
        return (1.5, 0.5)

    # 7.62x39
    if any(x in name for x in ("762x39", "7.62x39", "m43")):
        return (1.5, 0.5)

    # --- Full-power rifle (7.62x51+) ---
    # 7.62x51 / 7.62x54 / 7.62x67
    if "762x51" in name or "762x54" in name or "762x67" in name:
        return (2.0, 0.8)

    # .338 Lapua / .338 Norma
    if "338_lapua" in name or "338_norma" in name:
        return (2.0, 0.8)

    # .408 CheyTac
    if "408_cheytac" in name or "408" in name:
        return (2.0, 0.8)

    # 6.5 Creedmoor / 6.5x47 Lapua
    if "65_creedmoor" in name or "65x47" in name:
        return (2.0, 0.8)

    # 277 Fury (6.8x51mm)
    if "277_fury" in name:
        return (2.0, 0.8)

    # 9.3x64 Brenneke
    if "93x64" in name:
        return (2.0, 0.8)

    # M80
    if name.startswith("m80."):
        return (2.0, 0.8)

    # --- Large-caliber / anti-materiel (12.7mm+) ---
    if "127x108" in name or "127x99" in name:
        return (3.0, 1.5)

    # 50 Beowulf
    if "50_beowulf" in name:
        return (3.0, 1.5)

    # 12.7x54 VSSK (subsonic but large caliber)
    if "127x54" in name:
        return (0.5, 0.3)

    # --- ATGMs, rockets, HEAT ---
    if "rpg_" in name or "rpg7" in name or "rpg32" in name:
        return (5.0, 3.0)
    if "maaws" in name:
        return (5.0, 3.0)
    if "nlaw" in name:
        return (5.0, 3.0)
    if "pcml" in name:
        return (5.0, 3.0)
    if "titan_" in name:
        return (5.0, 3.0)
    if "vorona" in name:
        return (5.0, 3.0)

    # --- EFP ---
    if "efp" in name:
        return (10.0, 4.0)

    # --- APFSDS ---
    if "m829a1" in name:
        return (1.0, 0.4)

    # --- Catch-all by caliber ---
    if cal <= 9.5:
        return (0.5, 0.3)
    elif cal <= 8.0:
        # 5.56-7.62mm -> intermediate
        return (1.5, 0.5)
    else:
        return (2.0, 0.8)


def main():
    updated = 0
    skipped_old = 0
    skipped_no_projectile = 0

    for fname in sorted(os.listdir(AMMO_DIR)):
        if not fname.endswith(".json"):
            continue

        fpath = os.path.join(AMMO_DIR, fname)

        with open(fpath, "r") as f:
            content = f.read()

        try:
            data = json.loads(content)
        except json.JSONDecodeError:
            print(f"  SKIP (invalid JSON): {fname}")
            skipped_old += 1
            continue

        # Skip old-format files (no projectile block)
        if "projectile" not in data:
            print(f"  SKIP (old format): {fname}")
            skipped_old += 1
            continue

        mean, std = classify(fname, data)

        # Check if already set to non-zero values
        proj = data["projectile"]
        old_mean = proj.get("frag_mass_mean", None)
        old_std = proj.get("frag_mass_std", None)

        # Only update if currently 0.0 (or absent/null)
        if (old_mean is not None and old_mean != 0.0) or (
            old_std is not None and old_std != 0.0
        ):
            print(f"  SKIP (already set): {fname} (mean={old_mean}, std={old_std})")
            skipped_old += 1
            continue

        # Update the parsed data
        proj["frag_mass_mean"] = mean
        proj["frag_mass_std"] = std

        # Serialize back, preserving 4-space indent and trailing newline
        new_content = json.dumps(data, indent=4) + "\n"

        # Fix: json.dumps may not match exact original formatting.
        # The original files all have 4-space indent and sorted keys (no trailing whitespace).
        # This should be compatible with the Rust deserializer.

        with open(fpath, "w") as f:
            f.write(new_content)

        print(f"  UPDATED: {fname:40s} → mean={mean}, std={std}")
        updated += 1

    print(f"\nDone. {updated} ammo files updated, {skipped_old} skipped.")


if __name__ == "__main__":
    main()
