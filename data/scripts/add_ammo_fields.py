"""Add new ammo data fields to all ammo JSON files.

Reads each ammo JSON, determines appropriate defaults based on the file ID,
and writes the updated JSON back (preserving all existing values).
"""

import json
import os
import re

AMMO_DIR = os.path.join(os.path.dirname(__file__), "..", "ammo")


def get_defaults(file_id: str) -> dict:
    """Determine default values for the new fields based on file ID."""
    fid_lower = file_id.lower()

    # Tracer: check if ID contains "tracer" or "trac"
    has_tracer = bool(re.search(r"trac(?:er)?", fid_lower, re.IGNORECASE))

    # Incendiary: check if ID contains "incendiary", "api", "api_t", or "iap"
    has_incendiary = bool(
        re.search(r"(?:incendiary|api(?:_t)?|iap)", fid_lower, re.IGNORECASE)
    )

    return {
        "frag_mass_mean": 0.0,
        "frag_mass_std": 0.0,
        "ricochet_angle_deg": 0.0,
        "tracer_burn_time_s": 2.5 if has_tracer else 0.0,
        "incendiary": has_incendiary,
        "incendiary_ignition_temp_k": 550.0 if has_incendiary else 0.0,
    }


def main():
    files = sorted(f for f in os.listdir(AMMO_DIR) if f.endswith(".json"))
    updated = 0
    skipped = 0

    for filename in files:
        filepath = os.path.join(AMMO_DIR, filename)
        file_id = filename.replace(".json", "")

        with open(filepath, "r") as f:
            data = json.load(f)

        # The fields go inside the "projectile" object
        projectile = data.get("projectile")
        if projectile is None:
            print(f"SKIP {filename}: no 'projectile' key")
            skipped += 1
            continue

        defaults = get_defaults(file_id)

        # Only add fields that don't already exist
        changed = False
        for key, value in defaults.items():
            if key not in projectile:
                projectile[key] = value
                changed = True

        if changed:
            with open(filepath, "w") as f:
                json.dump(data, f, indent=4)
                f.write("\n")  # trailing newline
            updated += 1
            print(f"UPDATED {filename}: {defaults}")
        else:
            skipped += 1
            print(f"SKIP {filename}: all new fields already present")

    print(f"\nDone. Updated: {updated}, Skipped: {skipped}")


if __name__ == "__main__":
    main()
