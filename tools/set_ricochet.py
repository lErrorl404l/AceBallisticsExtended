#!/usr/bin/env python3
"""Set ricochet_angle_deg for all ammo JSON files based on projectile-type heuristics."""

import json, os, re

AMMO_DIR = os.path.join(os.path.dirname(__file__), "..", "data", "ammo")
AMMO_DIR = os.path.normpath(AMMO_DIR)


def has_ap_indicator(name_lower: str, model: str) -> bool:
    """Check if the filename or model indicates an AP round (no false on 'lapua')."""
    # File patterns: _api, _ap, api_, 7n22
    if re.search(r"(^|_)(api?)(_|\.)", name_lower):
        return True
    if "7n22" in name_lower:
        return True
    if "ss190" in name_lower or "ss190" in model:
        return True
    # Model checks: whole-word "AP", prefix/suffix "AP_"/"_AP", or "API"
    model_lower = model.lower()
    if re.search(r"(^|[ _])ap([ _$]|$)", model_lower):
        return True
    if "_api" in model_lower or model_lower.startswith("api_"):
        return True
    return False


def classify(filename: str, data: dict) -> float:
    """Determine ricochet angle from filename and content."""
    name_lower = filename.lower()

    # If nested format has a projectile block, check model
    proj = data.get("projectile", {})
    model = proj.get("model", "").lower()
    source = proj.get("source", "").lower()
    ptype = data.get("projectileType", "")

    # 1. APFSDS - long rod penetrators
    if "apfsds" in source or "m829a1" in name_lower or model == "m829a1":
        return 10.0

    # 2. EFP
    if "efp" in name_lower or "efp" in model:
        return 30.0

    # 3. HEAT / RPG / ATGM / missile with shaped charge
    heat_keywords = [
        "heat",
        "rpg7",
        "rpg32",
        "rpg_29",
        "maaws_heat",
        "nlaw",
        "pcml_at",
        "titan_at",
        "titan_aa",
        "vorona_atgm",
    ]
    if any(k in name_lower for k in heat_keywords):
        return 35.0
    if any(k in model for k in ["heat", "rpg", "titan", "nlaw", "pcml", "vorona"]):
        return 35.0

    # 4. Hollow point / JHP
    if "jhp" in name_lower or ptype == "jhp":
        return 25.0

    # 5. Slug
    if "slug" in name_lower:
        return 30.0

    # 6. Shotgun (buckshot/birdshot)
    if "buckshot" in name_lower or "birdshot" in name_lower:
        return 20.0

    # 7. Armour Piercing (AP, API) - careful: avoid "lapua" matching "ap"
    if has_ap_indicator(name_lower, model):
        return 15.0
    if ptype == "ap":
        return 15.0
    # Source-based AP check: explicit AP markers in source
    # Avoid false-positives on FMJ/ball rounds or "APG" (a US Army lab)
    if re.search(
        r"\b(armour.piercing|armor.piercing|hardened.?steel)\b", source, re.IGNORECASE
    ):
        if "fmj" not in source.lower() and "fmj" not in model:
            return 15.0
    # Check source for AP mention, but avoid "AP variant" references
    if re.search(r"\bAP\b", source, re.IGNORECASE) and "variant" not in source.lower():
        if "fmj" not in source.lower() and "fmj" not in model:
            return 15.0

    # 8. Subsonic
    if "subsonic" in name_lower:
        return 22.0
    if model in ("sp-5", "sp_5") or "sp5" in name_lower:
        return 22.0
    if "vssk" in name_lower:
        return 22.0

    # 9. Large caliber (12.7mm+)
    cal = data.get("caliberMm", 0.0) or proj.get("caliber_mm", 0.0)
    if cal >= 12.0:
        return 15.0

    # 10. Default: FMJ / ball
    return 20.0


def main():
    for fname in sorted(os.listdir(AMMO_DIR)):
        if not fname.endswith(".json"):
            continue
        fpath = os.path.join(AMMO_DIR, fname)

        with open(fpath, "r") as f:
            data = json.load(f)

        angle = classify(fname, data)

        # Determine whether it's nested (has projectile block) or flat
        if "projectile" in data:
            data["projectile"]["ricochet_angle_deg"] = angle
        else:
            data["ricochet_angle_deg"] = angle

        with open(fpath, "w") as f:
            json.dump(
                data,
                f,
                indent=("    " if "projectile" in data else "  "),
            )
            f.write("\n")

        print(f"  {fname}: {angle}°")


if __name__ == "__main__":
    main()
