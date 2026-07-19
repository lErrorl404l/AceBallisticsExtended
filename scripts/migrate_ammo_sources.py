#!/usr/bin/env python3
"""
Migrate data/ammo/ JSON files: convert flat string `source` fields to a
structured `source` object (type, reference, methodology, confidence),
mirroring what was done for data/armor/materials/.
"""

import json
import os
import re
import glob

AMMO_DIR = os.path.join(os.path.dirname(__file__), "..", "data", "ammo")


def classify_source(text: str) -> dict:
    """Given a raw source string, produce a structured source block."""
    text = text.strip()

    # --- Pattern detection ---

    # 1. Arma 3 config values (game data, lowest confidence)
    if re.match(r"^Arma 3 config\.", text) or re.match(r"^Arma 3 config values", text):
        return {
            "type": "arma_config",
            "reference": text,
            "methodology": "Values extracted from Arma 3 config files (hit, airFriction, caliber). Not validated against IRL sources.",
            "confidence": "low",
        }

    # 2. ACE3 ballistics data (ACE3 mod extracted)
    if text.startswith("ACE3 ballistics:") or text.startswith("ACE3 data:"):
        return {
            "type": "ace3_data",
            "reference": text,
            "methodology": "Values sourced from ACE3 Advanced Ballistics configuration data. ACE3 sources its data from a mix of IRL references, manufacturer data, and community estimates.",
            "confidence": "medium",
        }

    # 3. Simple "IRL data from manufacturer/SAAMI specs" pattern
    if (
        "IRL data from manufacturer" in text
        or "IRL data from manufacturer/SAAMI specs" in text
    ):
        return {
            "type": "manufacturer_data",
            "reference": text,
            "methodology": "Values sourced from manufacturer published specifications and SAAMI/CIP/NATO EPVAT pressure standards.",
            "confidence": "high",
        }

    # 4. Contains "Est." or "Conservative est." or "estimated" (estimates)
    if re.search(r"\bEst\.|Conservative est\.|estimated|est\. ", text, re.IGNORECASE):
        return {
            "type": "estimated",
            "reference": text,
            "methodology": "BC values estimated from similar known projectiles using form factor approximations or Litz estimation methods. Not directly measured or published.",
            "confidence": "low",
        }

    # 5. References to Litz (Applied Ballistics) - academic/literature
    if re.search(r"Litz|Applied Ballistics", text, re.IGNORECASE):
        return {
            "type": "academic_literature",
            "reference": text,
            "methodology": "BC values sourced from Bryan Litz's Applied Ballistics published Doppler radar measurements and form factor analysis.",
            "confidence": "high",
        }

    # 6. References to US Army BRL, ARL, APG, DTIC - military/defence research
    if re.search(
        r"US Army BRL|ARL[-\s]|APG[-\s]|DTIC|Yuma Proving Ground|BRL[-\s]", text
    ):
        return {
            "type": "military_standard",
            "reference": text,
            "methodology": "Values sourced from US Army Ballistic Research Laboratory (BRL) / Armament Research, Development and Engineering Center (ARDEC) published reports, Doppler radar measurements, or SAAMI/CIP/NATO EPVAT test data.",
            "confidence": "high",
        }

    # 7. Arma 3 vanilla config values (descriptive)
    if text.startswith("Vanilla Arma 3 config"):
        return {
            "type": "arma_config",
            "reference": text,
            "methodology": "Values extracted from vanilla Arma 3 game configuration files. May not reflect IRL ballistics.",
            "confidence": "low",
        }

    # 8. Contains "ACE3 rhs_..." (ACE3 mod data from RHS)
    if (
        text.startswith("ACE3 rhs_")
        or "ACE3 ACE_bulletMass" in text
        or "ACE3 ACE_" in text
    ):
        return {
            "type": "ace3_data",
            "reference": text,
            "methodology": "Values sourced from ACE3 Advanced Ballistics configuration, possibly combined with RHS mod data.",
            "confidence": "medium",
        }

    # 9. LabRadar measured or Doppler measured - experimental
    if re.search(r"LabRadar|Doppler radar|Doppler measured", text, re.IGNORECASE):
        return {
            "type": "experimental_measurement",
            "reference": text,
            "methodology": "BC values measured by Doppler radar (e.g., LabRadar). Direct experimental measurement.",
            "confidence": "high",
        }

    # 10. References to specific manufacturer
    if re.search(
        r"(Hornady|Sierra|Barnes|Lapua|Federal|Remington|Alexander Arms|Berger|CheyTac|Jamison|Lost River|Speer[^.]|FN |HK |Fiocchi|Winchester|Nosler|Swift|Norma|RWS|PRVI)",
        text,
        re.IGNORECASE,
    ):
        return {
            "type": "manufacturer_data",
            "reference": text,
            "methodology": "Values sourced from manufacturer published ballistic data (factory BC tables, reloading manuals, or product specifications).",
            "confidence": "high",
        }

    # 11. Contains "consensus" indicating community-agreed values
    if "consensus" in text.lower():
        return {
            "type": "community_consensus",
            "reference": text,
            "methodology": "BC value is a consensus across multiple sources (published data, user measurements, community analysis) due to variation between published values.",
            "confidence": "medium",
        }

    # 12. Contains "Jane's" reference
    if "Jane's" in text:
        return {
            "type": "reference_publication",
            "reference": text,
            "methodology": "Values sourced from Jane's Defence reference publications.",
            "confidence": "high",
        }

    # 13. "Definitive BC" pattern
    if text.startswith("Definitive BC"):
        return {
            "type": "mixed",
            "reference": text,
            "methodology": "BC value compiled from multiple authoritative sources (Litz, ARL, community analysis) and synthesized into a definitive value.",
            "confidence": "high",
        }

    # 14. Fallback: unknown/generic
    return {
        "type": "unknown",
        "reference": text,
        "methodology": "Source of values could not be automatically classified. May originate from game config, manufacturer data, or estimates.",
        "confidence": "low",
    }


def migrate_file(filepath: str) -> bool:
    """Migrate a single JSON file. Returns True if modified."""
    with open(filepath, "r", encoding="utf-8") as f:
        data = json.load(f)

    # Find the source field - could be in projectile or at root
    source_str = None
    source_path = None  # list of keys to navigate

    if "projectile" in data and isinstance(data["projectile"], dict):
        if "source" in data["projectile"] and isinstance(
            data["projectile"]["source"], str
        ):
            source_str = data["projectile"]["source"]
            source_path = ["projectile", "source"]
    elif "source" in data and isinstance(data["source"], str):
        source_str = data["source"]
        source_path = ["source"]

    if source_str is None or source_path is None:
        return False  # no string source to migrate

    # Build structured source
    structured = classify_source(source_str)

    # Navigate to the right level and replace
    target = data
    for key in source_path[:-1]:
        target = target[key]
    target[source_path[-1]] = structured

    with open(filepath, "w", encoding="utf-8") as f:
        json.dump(data, f, indent=2, ensure_ascii=False)
        f.write("\n")

    return True


def main():
    pattern = os.path.join(AMMO_DIR, "**", "*.json")
    files = sorted(glob.glob(pattern, recursive=True))
    print(f"Found {len(files)} JSON files under data/ammo/")

    modified = 0
    skipped = 0
    errors = []

    for filepath in files:
        try:
            if migrate_file(filepath):
                modified += 1
            else:
                skipped += 1
        except Exception as e:
            errors.append((filepath, str(e)))

    print(f"\nModified: {modified}")
    print(f"Skipped (no string source): {skipped}")
    if errors:
        print(f"\nErrors ({len(errors)}):")
        for fp, err in errors:
            print(f"  {fp}: {err}")
    else:
        print("\nAll files migrated successfully!")


if __name__ == "__main__":
    main()
