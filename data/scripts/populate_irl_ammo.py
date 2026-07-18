#!/usr/bin/env python3
"""
Populate missing IRL ammo values.

Ammo data is already 100% source-cited. This script:
  1. Fills missing mass_g for conventional bullet types that should have it
  2. Fills missing bc_g7 for conventional bullet types that should have it
  3. Adds fragmentation params for high-velocity FMJ/ball ammo known to fragment
  4. Sets missing cdm_id for bullets that need it (g1 for pistol, g7 for rifle)

Only touches ammo entries that are conventional bullets (not missiles,
rockets, bombs, grenades, shotgun submunitions, etc.).
"""

import json, os, re

AMMO_DIR = "data/ammo"

# ─── IRL Ammo Data Table ────────────────────────────────────────────────────
# Maps class name patterns to IRL mass (g), BC (G7), CDM, and fragmentation params
# Only for conventional bullet types that should have these fields.
# Format: (class_pattern, mass_g, bc_g7, cdm_id, frag_threshold, frag_count)

AMMO_DATA = [
    # === 9×19mm Parabellum ===
    ("B_9x19", 8.0, 0.152, "g1", 610, 1),
    ("9x19", 8.0, 0.152, "g1", 610, 1),
    ("9mm", 8.0, 0.152, "g1", 610, 1),
    # === 9×21mm ===
    ("9x21", 8.0, 0.140, "g1", None, 0),
    # === .45 ACP ===
    ("45ACP", 12.0, 0.110, "g1", None, 0),
    ("45acp", 12.0, 0.110, "g1", None, 0),
    # === 4.6×30mm (HK MP7) ===
    ("46x30", 2.0, 0.148, "g7", None, 0),
    # === 5.7×28mm (FN P90/ Five-seveN) ===
    ("570x28", 2.0, 0.160, "g7", None, 0),
    ("57x28", 2.0, 0.160, "g7", None, 0),
    # === 5.56×45mm NATO ===
    ("M855", 4.0, 0.151, "g7", 762, 50),
    ("M855A1", 4.02, 0.152, "g7", 800, 60),
    ("M193", 3.56, 0.133, "g7", 610, 30),
    ("SS109", 4.0, 0.151, "g7", 762, 50),
    ("Mk262", 4.99, 0.164, "g7", None, 0),
    ("Mk318", 4.09, 0.153, "g7", None, 0),
    ("b_556x45", 4.0, 0.151, "g7", None, 0),
    # === 5.45×39mm Soviet ===
    ("545x39", 3.43, 0.168, "g7", 700, 40),
    ("7N6", 3.43, 0.168, "g7", 700, 40),
    # === 5.8×42mm Chinese ===
    ("580x42", 4.45, 0.158, "g7", None, 0),
    ("58x42", 4.45, 0.158, "g7", None, 0),
    # === 6.5mm Creedmoor / Grendel ===
    ("65Creedmoor", 9.07, 0.270, "g7", None, 0),
    ("65Grendel", 7.97, 0.263, "g7", None, 0),
    # === 7.62×39mm Soviet ===
    ("762x39", 8.0, 0.205, "g7", None, 0),
    ("7.62x39", 8.0, 0.205, "g7", None, 0),
    # === 7.62×51mm NATO ===
    ("762x51", 9.5, 0.200, "g7", None, 0),
    ("7.62x51", 9.5, 0.200, "g7", None, 0),
    ("M80", 9.5, 0.200, "g7", None, 0),
    # === 7.62×54mmR ===
    ("762x54", 9.6, 0.210, "g7", None, 0),
    ("7.62x54", 9.6, 0.210, "g7", None, 0),
    ("LPS", 9.6, 0.210, "g7", None, 0),
    # === .300 Winchester Magnum ===
    ("300WinMag", 11.7, 0.268, "g7", None, 0),
    ("300WM", 11.7, 0.268, "g7", None, 0),
    # === .308 Winchester ===
    ("308Win", 9.5, 0.200, "g7", None, 0),
    # === .338 Lapua Magnum ===
    ("338Lapua", 16.2, 0.310, "g7", None, 0),
    ("338LM", 16.2, 0.310, "g7", None, 0),
    # === .408 CheyTac ===
    ("408CheyTac", 27.0, 0.380, "g7", None, 0),
    # === .50 BMG (12.7×99mm) ===
    ("127x99", 42.0, 0.340, "g7", None, 0),
    ("12.7x99", 42.0, 0.340, "g7", None, 0),
    ("50BMG", 42.0, 0.340, "g7", None, 0),
    # === 12.7×108mm ===
    ("127x108", 50.0, 0.380, "g7", None, 0),
    ("12.7x108", 50.0, 0.380, "g7", None, 0),
    # === 12.7×54mm (VSSK Vychlop) ===
    ("127x54", 6.12, 0.129, "g1", None, 0),
    # === 12 Gauge (slug) ===
    ("B_12Gauge_Slug", 28.0, 0.055, "g7", None, 0),
    ("12gauge_slug", 28.0, 0.055, "g7", None, 0),
    # === M829 series (APFSDS) ===
    ("m829", 4650, 0.82, "g7", None, 0),
    ("m829a1", 4650, 0.82, "g7", None, 0),
    ("m829a2", 4500, 0.85, "g7", None, 0),
    ("m829a3", 4300, 0.88, "g7", None, 0),
]


def find_ammo_data(class_name):
    """Find IRL ammo data by class name pattern.

    Uses word-boundary matching to prevent false positives:
    e.g. '9mm' should match 'B_9x19_Ball' but NOT 'b_19mm_he'.
    """
    best = None
    best_len = 0
    for pattern, mass, bc, cdm, frag_thresh, frag_count in AMMO_DATA:
        # Use boundary check: pattern either at start of class_name or preceded by non-alnum
        idx = class_name.find(pattern)
        while idx != -1:
            # Check character before pattern (if any)
            before_ok = idx == 0 or not class_name[idx - 1].isalnum()
            # Check character after pattern (if any)
            after = idx + len(pattern)
            after_ok = after >= len(class_name) or not class_name[after].isalnum()
            if before_ok and after_ok:
                if len(pattern) > best_len:
                    best = (mass, bc, cdm, frag_thresh, frag_count)
                    best_len = len(pattern)
                break  # first valid match for this pattern
            idx = class_name.find(pattern, idx + 1)
    return best


def is_conventional_bullet(proj, class_name, d):
    """Check if this ammo is a conventional bullet (not rocket/missile/bomb/etc)."""
    # Skip missiles, rockets, grenades, bombs
    non_bullet_patterns = [
        "R_",
        "M_",
        "G_",
        "F_",
        "Grenade",
        "Missile",
        "Bomb",
        "Submunition",
        "Pellets",
        "Smoke",
        "Flare",
        "Mine",
        "ACE_Demo",
        "ACE_ammoexplosion",
        "Module",
        "ShipCannon",
        "Howitzer",
        "Gatling",
    ]
    for p in non_bullet_patterns:
        if p in class_name:
            return False

    # Shotgun submunitions
    if "submunition" in class_name.lower():
        return False

    # launcher ammo
    if "launcher" in d.get("type", "") or "missile" in d.get("type", ""):
        return False

    # If it already has mass but no BC, still try (might be valid)
    return True


def main():
    updated = 0
    skipped_conventional = 0
    skipped_non_bullet = 0

    for root, _dirs, files in os.walk(AMMO_DIR):
        for fname in sorted(files):
            if not fname.endswith(".json"):
                continue
            fpath = os.path.join(root, fname)
            with open(fpath) as f:
                d = json.load(f)

            cls = d.get("class", "")
            proj = d.get("projectile", d)

            if not is_conventional_bullet(proj, cls, d):
                skipped_non_bullet += 1
                continue

            # Check if conventional bullet data is already complete
            mass = proj.get("mass_g", proj.get("projectile_mass_g", None))
            bc = proj.get("bc_g7", proj.get("bcG7", None))
            cdm = proj.get("cdm_id", d.get("cdm_id", None))
            frag = proj.get("fragmentation", None)

            ammo = find_ammo_data(cls)
            changed = False

            if ammo:
                irl_mass, irl_bc, irl_cdm, irl_frag_thresh, irl_frag_count = ammo

                # Only fill if missing (not overwriting existing curated data)
                if (not mass or mass == 0) and irl_mass:
                    if "mass_g" in proj:
                        proj["mass_g"] = irl_mass
                    else:
                        proj["mass_g"] = irl_mass
                    changed = True

                if (not bc or bc == 0) and irl_bc:
                    if "bc_g7" in proj:
                        proj["bc_g7"] = irl_bc
                    else:
                        proj["bc_g7"] = irl_bc
                    changed = True

                if (not cdm) and irl_cdm:
                    if "cdm_id" in proj:
                        proj["cdm_id"] = irl_cdm
                    else:
                        proj["cdm_id"] = irl_cdm
                    changed = True

                # Fill fragmentation for bullet types known to fragment
                if not frag and irl_frag_thresh:
                    proj["fragmentation"] = {
                        "threshold_vel_ms": irl_frag_thresh,
                        "avg_fragments": irl_frag_count,
                        "mass_distribution": "log_normal",
                        "params": {"mean": 0.15, "std": 0.12},
                    }
                    changed = True

            if changed:
                with open(fpath, "w") as f:
                    json.dump(d, f, indent=2)
                    f.write("\n")
                updated += 1
                mass_str = (
                    f"{proj.get('mass_g', '?'):.1f}g" if proj.get("mass_g") else "?"
                )
                bc_str = f"{proj.get('bc_g7', '?'):.3f}" if proj.get("bc_g7") else "?"
                cdm_str = proj.get("cdm_id", "?")
                print(
                    f"  ✓ {fname:45s} mass={mass_str:>8s} bc={bc_str:>6s} cdm={cdm_str}"
                )
            else:
                skipped_conventional += 1

    print(
        f"\nUpdated: {updated}, skipped (had data): {skipped_conventional}, skipped (non-bullet): {skipped_non_bullet}"
    )


if __name__ == "__main__":
    main()
