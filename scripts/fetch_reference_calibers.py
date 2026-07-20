#!/usr/bin/env python3
"""Fetch military cartridge specs from Wikipedia and output structured JSON.

Produces data/reference/calibers.json — a curated reference of key military
cartridge specifications sourced from Wikipedia infoboxes.

Usage:
    python scripts/fetch_reference_calibers.py
"""

import json
import os
import re
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
REFERENCE_DIR = os.path.join(SCRIPT_DIR, "..", "data", "reference")
OUTPUT = os.path.join(REFERENCE_DIR, "calibers.json")

WIKI_BASE = "https://en.wikipedia.org/w/api.php"

# ── Key military calibers ─────────────────────────────────────────
# Each: (page_title_on_wikipedia, short_name, category, [fallback_titles])
CALIBERS = [
    ("5.56×45mm NATO", "5.56x45mm", "rifle", []),
    ("7.62×51mm NATO", "7.62x51mm", "rifle", []),
    ("7.62×39mm", "7.62x39mm", "rifle", []),
    ("5.45×39mm", "5.45x39mm", "rifle", []),
    ("9×19mm Parabellum", "9x19mm", "pistol", []),
    (".45 ACP", "45acp", "pistol", []),
    (".50 BMG", "12.7x99mm", "heavy", ["12.7×99mm NATO"]),
    ("12.7 × 108 mm", "12.7x108mm", "heavy", ["12.7×108mm"]),
    ("7.62×54mmR", "7.62x54r", "rifle", []),
    ("9×18mm Makarov", "9x18mm", "pistol", []),
    (
        ".300 Winchester Magnum",
        "300winmag",
        "rifle",
        ["300 Winchester Magnum", "300 Winchester Magnum (7.62×67mm)"],
    ),
    (".338 Lapua Magnum", "338lapua", "rifle", []),
    ("14.5 × 114 mm", "14.5x114mm", "heavy", ["14.5×114mm"]),
    (".303 British", "303british", "rifle", []),
    ("7.92×57mm Mauser", "7.92x57mm", "rifle", []),
    (".30-06 Springfield", "30-06", "rifle", []),
    ("FN 5.7×28mm", "5.7x28mm", "pistol", ["5.7×28mm"]),
    ("4.6×30mm", "4.6x30mm", "pistol", ["4.6x30mm"]),
    (
        "6.5mm Creedmoor",
        "65creedmoor",
        "rifle",
        ["6.5 Creedmoor", "6.5mm Creedmoor (6.5×48mm)"],
    ),
    (
        ".300 AAC Blackout",
        "300blk",
        "rifle",
        ["300 AAC Blackout", "300 AAC Blackout (7.62×35mm)"],
    ),
    (".50 Beowulf", "50beowulf", "rifle", []),
    (".458 SOCOM", "458socom", "rifle", []),
    (".500 S&W Magnum", "500sw", "pistol", []),
    (".40 S&W", "40sw", "pistol", []),
    (".357 Magnum", "357mag", "pistol", []),
    ("10mm Auto", "10mmauto", "pistol", []),
    (".380 ACP", "380acp", "pistol", []),
]

# Wikipedia infobox field names for {{Infobox firearm cartridge}}
# The actual wikitext keys differ from the human-readable labels.
WANTED_FIELDS = {
    "bullet_diameter_mm": ["bullet", "land"],
    "neck_diameter_mm": ["neck"],
    "shoulder_diameter_mm": ["shoulder"],
    "base_diameter_mm": ["base"],
    "rim_diameter_mm": ["rim_dia", "rim"],
    "case_length_mm": ["case_length"],
    "overall_length_mm": ["length"],
    "case_capacity_cm3": ["case_capacity"],
    "max_pressure_mpa": ["max_pressure", "max_pressure2"],
    "velocity_ms": ["vel1"],
    "energy_j": ["en1"],
    "primer_type": ["primer"],
}

# Match numbers with optional commas, followed by optional unit
NUMBER_WITH_UNITS = re.compile(
    r"([\d,]+(?:\.\d+)?)\s*(mm|cm3|MPa|psi|g|gr|m/s|J|kJ|in)\b",
    re.IGNORECASE,
)


def wiki_get_wikitext(title: str, retries: int = 3) -> str | None:
    """Fetch raw wikitext of a Wikipedia page's lead section (section 0)
    via the action=parse API — preserves {{Infobox ...}} templates."""
    params = {
        "action": "parse",
        "format": "json",
        "page": title,
        "prop": "wikitext",
        "section": 0,
    }
    url = WIKI_BASE + "?" + urllib.parse.urlencode(params)
    for attempt in range(retries):
        try:
            req = urllib.request.Request(
                url,
                headers={"User-Agent": "ABE-CaliberFetcher/1.0 (ballistics project)"},
            )
            with urllib.request.urlopen(req, timeout=15) as resp:
                data = json.loads(resp.read().decode())
            parsed = data.get("parse", {})
            if "error" in data:
                print(f"  API ERROR: {data['error'].get('info', '?')}", end="")
                return None
            wt = parsed.get("wikitext", {})
            return wt.get("*")
        except urllib.error.HTTPError as e:
            if e.code == 429:
                wait = 5 * (attempt + 1)
                print(
                    f"  RATE LIMITED (attempt {attempt + 1}), waiting {wait}s...",
                    end=" ",
                )
                sys.stdout.flush()
                time.sleep(wait)
            else:
                print(f"  HTTP {e.code}", end=" ")
                return None
        except Exception as e:
            if attempt == retries - 1:
                return None
            time.sleep(2)
    return None


def find_infobox_value(wikitext: str, wanted_keys: list[str]) -> str | None:
    """Extract a value from a Wikipedia infobox by key name.

    Parses the {{Infobox ...}} block from wikitext and tries each wanted key.
    """
    if not wikitext:
        return None

    # Find the infobox opening
    idx = wikitext.find("{{Infobox")
    if idx == -1:
        return None

    # Balance braces to find the full infobox block
    block = wikitext[idx:]
    depth = 0
    end = 0
    for i, ch in enumerate(block):
        if ch == "{":
            depth += 1
        elif ch == "}":
            depth -= 1
        if depth == 0 and i > 0:
            end = i + 1
            break
    if end == 0:
        return None

    infobox = block[:end]

    # Parse | key = value lines
    for line in infobox.split("\n"):
        stripped = line.strip()
        if not stripped.startswith("|") or "=" not in stripped:
            continue
        rest = stripped[1:].strip()
        key_raw, val_raw = rest.split("=", 1)
        key = key_raw.strip().lower()
        val = val_raw.strip()

        if key not in [k.lower() for k in wanted_keys]:
            continue

        # Resolve {{convert|N|unit|...}} and {{cvt|N|unit|...}} templates
        for tmpl in ("{{convert|", "{{cvt|"):
            while tmpl in val:
                start = val.index(tmpl)
                inner_end = val.index("}}", start)
                inner = val[start + len(tmpl) : inner_end]
                parts = inner.split("|")
                num_part = parts[0].strip().replace(",", "").replace("−", "-")
                # Handle ranges like "7.85|-|7.9" → take midpoint
                if "|-|" in num_part:
                    range_parts = num_part.split("|-|")
                    nums = []
                    for p in range_parts:
                        try:
                            nums.append(float(p.strip()))
                        except ValueError:
                            pass
                    if nums:
                        num_part = str(sum(nums) / len(nums))
                if num_part.replace(".", "").replace("-", "").isdigit():
                    replacement = num_part
                    if len(parts) > 1:
                        u = parts[1].strip().lower()
                        if u in ("mm", "cm3", "m/s", "g", "gr", "j", "kj"):
                            replacement = f"{num_part} {u}"
                    val = val[:start] + replacement + val[inner_end + 2 :]
                else:
                    val = val[:start] + val[inner_end + 2 :]

        # Strip HTML comments
        val = re.sub(r"<!--.*?-->", "", val)
        # Strip wiki links [[target|label]] → label
        val = re.sub(r"\[\[[^|]*\|?([^\]]*)\]\]", r"\1", val)
        # Strip any remaining {{...}}
        val = re.sub(r"\{\{[^}]*\}\}", "", val)
        val = val.strip()

        if val:
            return val

    return None


def normalize_value(text: str) -> tuple[float, str] | float | None:
    """Extract the primary numeric value and optional unit from text.

    Returns:
      tuple(float, unit) if a unit was found,
      float if bare number,
      None if nothing parseable.
    """
    if not text:
        return None
    text = text.strip().lower()

    # Match number + unit (handles commas: "52,939 psi" → "52939", "psi")
    m = NUMBER_WITH_UNITS.search(text)
    if m:
        raw_num = m.group(1).replace(",", "")
        num = float(raw_num)
        unit = m.group(2).lower()
        return (num, unit)

    # Bare number
    try:
        return float(text.replace(",", ""))
    except ValueError:
        pass

    return None


def convert_to_si(val, dest_field: str, imperial_infobox: bool):
    """Convert a value+unit to SI (mm, MPa, etc.)."""
    if isinstance(val, tuple):
        num, unit = val
        if unit == "psi":
            return round(num * 0.00689476, 4)
        if unit == "in":
            return round(num * 25.4, 4)
        if unit in ("gr",):
            return round(num * 0.06479891, 4)  # grains → grams
        return num  # already metric (mm, g, etc.)

    # Bare number with no unit
    if imperial_infobox:
        if dest_field in DIM_FIELDS:
            return round(val * 25.4, 4)  # inches → mm
        if dest_field == "max_pressure_mpa":
            return round(val * 0.00689476, 4)  # PSI → MPa
    return val


def is_imperial_infobox(wikitext: str) -> bool:
    """Check if the cartridge infobox uses imperial (non-SI) units.

    Returns True when is_SI_specs is absent, empty, or explicitly 'no'.
    Only returns False when is_SI_specs is explicitly 'yes'.
    """
    if not wikitext:
        return False
    raw = find_infobox_value(wikitext, ["is_SI_specs"])
    if raw is None:
        return True  # field absent → imperial default
    val = raw.strip().lower()
    if val in ("", "no", "0", "false"):
        return True  # empty or explicitly not SI → imperial
    return False  # explicitly 'yes' or '1' → metric


DIM_FIELDS = {
    "bullet_diameter_mm",
    "neck_diameter_mm",
    "shoulder_diameter_mm",
    "base_diameter_mm",
    "rim_diameter_mm",
    "case_length_mm",
    "overall_length_mm",
}


def fetch_caliber(title: str, short_name: str, fallbacks: list[str]) -> dict:
    """Fetch and parse specs for a single caliber from Wikipedia."""
    sys.stdout.write(f"  {title} ... ")
    sys.stdout.flush()

    wikitext = None
    for attempt in [title] + fallbacks:
        wikitext = wiki_get_wikitext(attempt)
        if wikitext:
            break
        time.sleep(0.5)

    if not wikitext:
        print("NOT FOUND")
        return {"title": title, "short_name": short_name, "status": "not_found"}

    imperial = is_imperial_infobox(wikitext)
    specs = {"title": title, "short_name": short_name, "status": "found"}
    for dest_field, search_keys in WANTED_FIELDS.items():
        raw = find_infobox_value(wikitext, search_keys)
        if raw:
            result = normalize_value(raw)
            if result is None:
                continue
            val = convert_to_si(result, dest_field, imperial)
            if isinstance(val, (int, float)) and val > 0:
                specs[dest_field] = val  # type: ignore[assignment]

    found = sum(1 for k in WANTED_FIELDS if k in specs)
    print(f"OK ({found} fields)")
    return specs


def main():
    os.makedirs(REFERENCE_DIR, exist_ok=True)
    print(f"Fetching {len(CALIBERS)} caliber specs from Wikipedia...\n")

    results = []
    for i, (title, short_name, category, fallbacks) in enumerate(CALIBERS):
        cal = fetch_caliber(title, short_name, fallbacks)
        cal["category"] = category
        results.append(cal)
        # Be polite to Wikipedia: 3.5s between requests to avoid rate limits
        if i < len(CALIBERS) - 1:
            time.sleep(3.5)

    # Sort by category then name
    results.sort(key=lambda c: (c.get("category", ""), c.get("short_name", "")))

    with open(OUTPUT, "w") as f:
        json.dump(results, f, indent=2)
    print(f"\nWrote {len(results)} calibers to {OUTPUT}")

    found = sum(1 for r in results if r.get("status") == "found")
    print(f"Fetched: {found}/{len(results)} calibers")

    # Per-category stats
    by_cat: dict[str, list[str]] = {}
    for r in results:
        cat = r.get("category", "other")
        by_cat.setdefault(cat, []).append(r.get("short_name", "?"))
    for cat, names in sorted(by_cat.items()):
        total = len(names)
        ok = sum(
            1
            for r in results
            if r.get("category") == cat and r.get("status") == "found"
        )
        print(f"  {cat}: {ok}/{total}")

    print("Done.")


if __name__ == "__main__":
    main()
