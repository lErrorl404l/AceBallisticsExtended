#!/usr/bin/env python3
"""
ABE Ammo HTML Report Generator
================================
Reads all ammo JSON files from data/ammo/ and generates a standalone HTML report
with sortable tables, inline SVG bar charts grouped by caliber, and an ammo
management view.

Usage:
    ./generate_ammo_report.py [--ammo-dir ../../data/ammo] [--output ../../data/reports/ammo_report.html]
"""

import argparse
import json
import math
import os
import re
import sys
from collections import defaultdict
from datetime import datetime
from html import escape

# ── Field mapping: normalize the two schemas (nested "projectile" vs flat) ──

AMMO_DIRS = [
    "handgun",
    "rifle",
    "heavy_127mm",
    "launcher",
    "shotgun",
    "apfsds",
]

CALIBER_LABELS = {
    4.6: "4.6×30mm",
    5.45: "5.45×39mm",
    5.56: "5.56×45mm",
    5.69: "5.56×45mm (M855A1)",
    5.7: "5.7×28mm",
    5.8: "5.8×42mm",
    6.5: "6.5mm",
    7.62: "7.62mm",
    7.72: "7.72mm",
    9.0: "9×19mm",
    9.01: "9×19mm",
    11.43: ".45 ACP",
    12.7: "12.7mm",
    14.5: "14.5mm",
    20.0: "20mm",
    23.0: "23mm",
    25.0: "25mm",
    30.0: "30mm",
    35.0: "35mm",
    40.0: "40mm",
}


def _get(d: dict | None, *keys: str, default: object = None) -> object:
    """Deep get from a dict via varargs keys."""
    if d is None:
        return default
    val: object = d
    for k in keys:
        if isinstance(val, dict):
            val = val.get(k, default)
        else:
            return default
    return val if val is not None else default


def _f(val: object, default_val: float = 0.0) -> float:
    """Safely coerce to float."""
    if val is None:
        return default_val
    if isinstance(val, (int, float)):
        return float(val)
    return default_val


def _s(val: object, default_val: str = "") -> str:
    """Safely coerce to str."""
    if val is None:
        return default_val
    if isinstance(val, str):
        return val
    return str(val)


def _b(val: object, default_val: bool = False) -> bool:
    """Safely coerce to bool."""
    if val is None:
        return default_val
    if isinstance(val, bool):
        return val
    if isinstance(val, (int, float)):
        return val != 0
    return default_val


def _i(val: object, default_val: int = 0) -> int:
    """Safely coerce to int."""
    if val is None:
        return default_val
    if isinstance(val, int):
        return val
    if isinstance(val, float):
        return int(val)
    return default_val


def load_ammo(ammo_root: str) -> list[dict]:
    """Load all ammo JSON files, normalizing to a flat record dict."""
    records = []
    for subdir in AMMO_DIRS:
        d = os.path.join(ammo_root, subdir)
        if not os.path.isdir(d):
            continue
        for root, _dirs, files in os.walk(d):
            for fname in sorted(files):
                if not fname.endswith(".json"):
                    continue
                fpath = os.path.join(root, fname)
                try:
                    with open(fpath) as f:
                        raw = json.load(f)
                except (json.JSONDecodeError, OSError) as e:
                    print(f"  ⚠ {fpath}: {e}", file=sys.stderr)
                    continue

                # Normalize: pull projectile sub-object up
                proj_raw = raw.get("projectile")
                if not isinstance(proj_raw, dict):
                    proj_raw = raw

                mass = _f(_get(proj_raw, "mass_g")) or _f(
                    _get(proj_raw, "projectileMassG")
                )
                cal = _f(_get(proj_raw, "caliber_mm")) or _f(
                    _get(proj_raw, "caliberMm")
                )
                bc_g7 = _f(_get(proj_raw, "bc_g7"))
                if not bc_g7:
                    bc_g7 = _f(_get(proj_raw, "bcG7"))
                bc_g1 = _f(_get(proj_raw, "bc_g1")) or _f(_get(proj_raw, "bcG1"))
                cdm = _s(_get(proj_raw, "cdm_id"), "g7") or _s(
                    _get(proj_raw, "cdmId"), "g7"
                )
                model = _s(_get(proj_raw, "model"))
                source = _s(_get(proj_raw, "source"))
                chamber_p = _f(raw.get("chamber_pressure_mpa"))
                frag_thresh = _f(_get(proj_raw, "fragmentation", "threshold_vel_ms"))
                frag_count = _i(_get(proj_raw, "fragmentation", "avg_fragments"))
                proj_type = _s(_get(proj_raw, "projectileType")) or _s(
                    _get(proj_raw, "type")
                )
                tracer = _b(_get(proj_raw, "tracer_burn_time_s"))
                incendiary = _b(_get(proj_raw, "incendiary"))

                # Determine caliber group label
                cal_key = (
                    min(CALIBER_LABELS.keys(), key=lambda k: abs(k - cal))
                    if cal
                    else None
                )
                cal_label = (
                    CALIBER_LABELS.get(cal_key, f"{cal:.2f}mm")
                    if cal_key
                    else "Unknown"
                )

                records.append(
                    {
                        "class": raw.get(
                            "class", raw.get("ammoClass", fname.replace(".json", ""))
                        ),
                        "file": fname,
                        "dir": subdir,
                        "model": model or fname.replace(".json", ""),
                        "mass_g": mass,
                        "caliber_mm": cal,
                        "caliber_group": cal_label,
                        "bc_g7": bc_g7,
                        "bc_g1": bc_g1,
                        "cdm_id": cdm,
                        "chamber_pressure_mpa": chamber_p,
                        "frag_threshold_ms": frag_thresh,
                        "frag_count": frag_count,
                        "proj_type": proj_type
                        or ("fmj" if "Ball" in raw.get("class", "") else "unknown"),
                        "tracer": tracer,
                        "incendiary": incendiary,
                        "source": source[:120] if source else "",
                        "notes": raw.get("notes", ""),
                        "path": fpath,
                    }
                )

    return records


# ── HTML generation ──────────────────────────────────────────────────────────

CSS = """\
* { box-sizing: border-box; margin: 0; padding: 0; }
body { font-family: 'Segoe UI', system-ui, -apple-system, sans-serif; background: #0d1117; color: #e6edf3; padding: 20px; max-width: 1400px; margin: 0 auto; }
h1 { color: #58a6ff; font-size: 1.8rem; margin-bottom: 4px; }
h2 { color: #79c0ff; font-size: 1.3rem; margin: 24px 0 12px; border-bottom: 1px solid #21262d; padding-bottom: 6px; }
.subtitle { color: #8b949e; font-size: 0.85rem; margin-bottom: 20px; }
.nav-bar { display: flex; gap: 8px; margin: 16px 0; flex-wrap: wrap; }
.nav-btn { padding: 6px 16px; border: 1px solid #30363d; border-radius: 6px; background: #161b22; color: #c9d1d9; cursor: pointer; font-size: 0.85rem; }
.nav-btn:hover { background: #1c2128; border-color: #58a6ff; }
.nav-btn.active { background: #1f6feb; border-color: #1f6feb; color: #fff; }
.page { display: none; }
.page.active { display: block; }
table { width: 100%; border-collapse: collapse; margin: 8px 0; font-size: 0.82rem; }
th { background: #161b22; color: #c9d1d9; padding: 8px 10px; text-align: left; border-bottom: 2px solid #30363d; cursor: pointer; user-select: none; white-space: nowrap; position: sticky; top: 0; z-index: 1; }
th:hover { color: #58a6ff; }
th::after { content: ' \\25B4\\25BE'; font-size: 0.6rem; color: #484f58; margin-left: 4px; }
th.sorted-asc::after { content: ' \\25B4'; color: #58a6ff; }
th.sorted-desc::after { content: ' \\25BE'; color: #58a6ff; }
td { padding: 6px 10px; border-bottom: 1px solid #21262d; }
tr:hover td { background: #161b22; }
.num { text-align: right; font-family: 'JetBrains Mono', 'Consolas', monospace; }
.bar-cell { position: relative; }
.bar { position: absolute; left: 10px; top: 3px; height: 20px; border-radius: 3px; opacity: 0.7; min-width: 2px; }
.bar-text { position: relative; z-index: 1; padding-left: 4px; }
.cal-group { margin: 16px 0; }
.cal-header { color: #58a6ff; font-weight: 600; font-size: 0.95rem; margin: 16px 0 8px; padding: 4px 0; border-bottom: 1px solid #21262d; }
.cal-count { color: #8b949e; font-size: 0.8rem; font-weight: normal; }
.summary-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(200px, 1fr)); gap: 12px; margin: 12px 0; }
.summary-card { background: #161b22; border: 1px solid #30363d; border-radius: 8px; padding: 16px; }
.summary-card .label { color: #8b949e; font-size: 0.75rem; text-transform: uppercase; letter-spacing: 0.5px; }
.summary-card .value { color: #e6edf3; font-size: 1.4rem; font-weight: 600; margin-top: 4px; }
.chart-container { margin: 20px 0; background: #161b22; border: 1px solid #30363d; border-radius: 8px; padding: 16px; overflow-x: auto; }
.chart-title { color: #c9d1d9; font-size: 0.95rem; font-weight: 600; margin-bottom: 12px; }
.bar-chart { display: flex; align-items: end; gap: 3px; height: 200px; padding: 0 4px; }
.bar-wrapper { display: flex; flex-direction: column; align-items: center; flex: 1; min-width: 12px; }
.bar-col { width: 100%; border-radius: 3px 3px 0 0; min-height: 2px; transition: opacity 0.2s; }
.bar-col:hover { opacity: 1; }
.bar-label { font-size: 0.6rem; color: #8b949e; margin-top: 4px; white-space: nowrap; transform: rotate(-45deg); transform-origin: left; max-width: 60px; overflow: hidden; text-overflow: ellipsis; }
.filter-bar { display: flex; gap: 8px; margin: 12px 0; flex-wrap: wrap; align-items: center; }
.filter-bar input, .filter-bar select { padding: 6px 10px; border: 1px solid #30363d; border-radius: 6px; background: #0d1117; color: #e6edf3; font-size: 0.85rem; }
.filter-bar input { flex: 1; min-width: 200px; }
.filter-bar select { min-width: 120px; }
.tag { display: inline-block; padding: 1px 6px; border-radius: 4px; font-size: 0.7rem; font-weight: 600; margin: 1px; }
.tag-fmj { background: #1f6feb33; color: #58a6ff; }
.tag-ap { background: #da363333; color: #ff7b72; }
.tag-hp { background: #3fb95033; color: #3fb950; }
.tag-sp { background: #d2992233; color: #d29922; }
.tag-tracer { background: #f0883e33; color: #f0883e; }
.tag-incendiary { background: #da363333; color: #ff7b72; }
.tag-unknown { background: #484f5833; color: #8b949e; }
a { color: #58a6ff; text-decoration: none; }
a:hover { text-decoration: underline; }
.search-highlight { background: #d2992266; padding: 0 2px; border-radius: 2px; }
.controls { display: flex; gap: 12px; align-items: center; flex-wrap: wrap; margin: 8px 0; }
.controls label { color: #8b949e; font-size: 0.8rem; }
"""


def _bar_color(idx: int) -> str:
    colors = [
        "#58a6ff",
        "#3fb950",
        "#d29922",
        "#f0883e",
        "#ff7b72",
        "#bc8cff",
        "#79c0ff",
        "#56d364",
    ]
    return colors[idx % len(colors)]


def _svg_bar_chart(
    records: list[dict], key: str, label: str, unit: str, height: int = 200
) -> str:
    """Generate an inline SVG bar chart for a given numeric key across records."""
    if not records:
        return "<p>No data</p>"

    items = [
        (r["model"] or r["class"], r[key], r["caliber_group"], r["file"])
        for r in records
        if r[key] > 0
    ]
    if not items:
        return "<p>No data</p>"

    items.sort(key=lambda x: x[1], reverse=True)
    max_val = max(v for _, v, _, _ in items)
    if max_val <= 0:
        return "<p>No data</p>"

    bar_w = max(12, min(60, 800 // len(items)))
    chart_w = len(items) * (bar_w + 4) + 40
    bar_scale = (height - 30) / max_val

    svg = f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {chart_w} {height + 40}" style="width:100%;max-width:{chart_w}px;">'
    svg += f'<text x="10" y="14" fill="#8b949e" font-size="11">{label} ({unit}) — top {len(items)}</text>'

    for i, (name, val, cal, fname) in enumerate(items):
        x = 20 + i * (bar_w + 4)
        bar_h = val * bar_scale
        y = height - bar_h
        color = _bar_color(i)
        svg += f'<rect x="{x}" y="{y}" width="{bar_w}" height="{bar_h}" fill="{color}" rx="2">'
        svg += f"<title>{escape(name)}: {val:.2f} {unit} ({cal})</title>"
        svg += "</rect>"
        svg += f'<text x="{x + bar_w / 2}" y="{height + 12}" fill="#8b949e" font-size="8" text-anchor="end" transform="rotate(-45,{x + bar_w / 2},{height + 12})">{escape(name[:18])}</text>'

    # Y axis labels
    for tick_n in range(0, 5):
        tick_val = max_val * tick_n / 4
        tick_y = height - tick_val * bar_scale
        svg += f'<text x="18" y="{tick_y + 3}" fill="#484f58" font-size="9" text-anchor="end">{tick_val:.0f}</text>'
        svg += f'<line x1="20" y1="{tick_y}" x2="{chart_w - 10}" y2="{tick_y}" stroke="#21262d" stroke-width="0.5"/>'

    svg += "</svg>"
    return svg


def _html_table(records: list[dict], show_cal_group: bool = True) -> str:
    """Generate an HTML table of ammo records."""
    rows = []
    for r in records:
        mass_str = f"{r['mass_g']:.2f}" if r["mass_g"] else "—"
        bc_str = f"{r['bc_g7']:.3f}" if r["bc_g7"] else ("—")
        press_str = (
            f"{r['chamber_pressure_mpa']:.0f}" if r["chamber_pressure_mpa"] else "—"
        )
        frag_s = f"{r['frag_threshold_ms']:.0f}" if r["frag_threshold_ms"] else "—"

        # Type tag
        ptype = r.get("proj_type", "unknown")
        type_tag = f'<span class="tag tag-{ptype}">{escape(ptype)}</span>'
        extras = ""
        if r["tracer"]:
            extras += '<span class="tag tag-tracer">T</span>'
        if r["incendiary"]:
            extras += '<span class="tag tag-incendiary">I</span>'

        # BC bar
        bc_pct = min(r["bc_g7"] / 0.5 * 100, 100) if r["bc_g7"] else 0

        cal_group = (
            f'<span class="cal-header">{escape(r["caliber_group"])}</span>'
            if show_cal_group
            else ""
        )

        rows.append(f"""<tr>
  <td><a href="file://{escape(r["path"])}">{escape(r["file"])}</a></td>
  <td>{escape(r["model"])}</td>
  <td>{cal_group}{escape(r["class"])}</td>
  <td class="num">{mass_str}</td>
  <td class="num">{r["caliber_mm"]:.2f}</td>
  <td class="num bar-cell"><div class="bar" style="width:{bc_pct:.0f}%;background:#58a6ff"></div><span class="bar-text">{bc_str}</span></td>
  <td>{escape(r["cdm_id"])}</td>
  <td class="num">{press_str}</td>
  <td class="num">{frag_s}</td>
  <td>{type_tag}{extras}</td>
  <td style="max-width:200px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;" title="{escape(r["source"])}">{escape(r["source"][:60])}</td>
</tr>""")

    return f"""<table id="ammo-table">
<thead><tr>
  <th data-col="file">File</th>
  <th data-col="model">Model</th>
  <th data-col="class">Class</th>
  <th data-col="mass" class="num">Mass (g)</th>
  <th data-col="caliber" class="num">Cal (mm)</th>
  <th data-col="bc" class="num">G7 BC</th>
  <th data-col="cdm">CDM</th>
  <th data-col="pressure" class="num">Press (MPa)</th>
  <th data-col="frag" class="num">Frag@(m/s)</th>
  <th data-col="type">Type</th>
  <th data-col="source">Source</th>
</tr></thead>
<tbody>
{chr(10).join(rows)}
</tbody>
</table>"""


def _caliber_charts(records: list[dict]) -> str:
    """Generate bar charts per caliber group."""
    by_cal = defaultdict(list)
    for r in records:
        by_cal[r["caliber_group"]].append(r)

    # Sort calibers roughly by diameter
    cal_order = sorted(
        by_cal.keys(),
        key=lambda c: (
            float(c.split("×")[0].replace("mm", "").replace(".", "0").split()[0])
            if c.split("×")[0].replace("mm", "").replace(".", "").isdigit()
            else 99
        ),
    )

    html = ""
    for cal in cal_order:
        items = by_cal[cal]
        html += f'<div class="cal-group">'
        html += f'<div class="cal-header">{escape(cal)} <span class="cal-count">({len(items)} rounds)</span></div>'

        # Charts for mass, BC, pressure
        mass_chart = _svg_bar_chart(items, "mass_g", "Projectile Mass", "g")
        bc_chart = _svg_bar_chart(items, "bc_g7", "G7 Ballistic Coefficient", "")
        press_chart = _svg_bar_chart(
            items, "chamber_pressure_mpa", "Chamber Pressure", "MPa"
        )

        html += '<div style="display:grid;grid-template-columns:repeat(auto-fit,minmax(300px,1fr));gap:12px;">'
        html += f'<div class="chart-container"><div class="chart-title">Mass</div>{mass_chart}</div>'
        html += f'<div class="chart-container"><div class="chart-title">G7 BC</div>{bc_chart}</div>'
        html += f'<div class="chart-container"><div class="chart-title">Chamber Pressure</div>{press_chart}</div>'
        html += "</div>"

        # Mini table for this caliber
        cal_recs = [r for r in records if r["caliber_group"] == cal]
        html += _html_table(cal_recs, show_cal_group=False)
        html += "</div>"

    return html


def _management_view(records: list[dict]) -> str:
    """Generate a management/filtering view for ammo data."""
    # Summary cards
    total = len(records)
    with_mass = sum(1 for r in records if r["mass_g"] > 0)
    with_bc = sum(1 for r in records if r["bc_g7"] > 0)
    with_pressure = sum(1 for r in records if r["chamber_pressure_mpa"] > 0)
    with_frag = sum(1 for r in records if r["frag_threshold_ms"] > 0)
    calibers = len(set(r["caliber_group"] for r in records))

    cards = f"""<div class="summary-grid">
  <div class="summary-card"><div class="label">Total Rounds</div><div class="value">{total}</div></div>
  <div class="summary-card"><div class="label">Caliber Groups</div><div class="value">{calibers}</div></div>
  <div class="summary-card"><div class="label">With Mass</div><div class="value">{with_mass}/{total}</div></div>
  <div class="summary-card"><div class="label">With G7 BC</div><div class="value">{with_bc}/{total}</div></div>
  <div class="summary-card"><div class="label">With Chamber Pressure</div><div class="value">{with_pressure}/{total}</div></div>
  <div class="summary-card"><div class="label">With Frag Threshold</div><div class="value">{with_frag}/{total}</div></div>
</div>"""

    # Missing data table
    missing_rows = []
    for r in records:
        gaps = []
        if not r["mass_g"]:
            gaps.append("mass")
        if not r["bc_g7"]:
            gaps.append("BC")
        if not r["chamber_pressure_mpa"]:
            gaps.append("pressure")
        if not r["frag_threshold_ms"]:
            gaps.append("frag threshold")
        if not r["source"]:
            gaps.append("source")
        if gaps:
            missing_rows.append(
                f'<tr><td><a href="file://{escape(r["path"])}">{escape(r["file"])}</a></td><td>{escape(r["model"])}</td><td>{escape(r["caliber_group"])}</td><td>{", ".join(gaps)}</td></tr>'
            )

    missing_table = ""
    if missing_rows:
        missing_table = f"""<h2>Incomplete Records ({len(missing_rows)})</h2>
<table><thead><tr><th>File</th><th>Model</th><th>Caliber</th><th>Missing Fields</th></tr></thead>
<tbody>{chr(10).join(missing_rows)}</tbody></table>"""
    else:
        missing_table = '<p style="color:#3fb950;">✓ All records are complete.</p>'

    # Caliber distribution
    cal_counts = defaultdict(int)
    for r in records:
        cal_counts[r["caliber_group"]] += 1

    dist_rows = ""
    for cal in sorted(
        cal_counts.keys(), key=lambda c: list(cal_counts.keys()).index(c)
    ):
        cnt = cal_counts[cal]
        pct = cnt / total * 100
        dist_rows += f"""<tr><td>{escape(cal)}</td><td class="num">{cnt}</td><td><div style="background:#161b22;border-radius:4px;overflow:hidden;"><div style="background:#58a6ff;width:{pct:.0f}%;height:20px;display:flex;align-items:center;padding-left:4px;font-size:0.75rem;min-width:fit-content;">{pct:.0f}%</div></div></td></tr>"""

    distribution = f"""<h2>Caliber Distribution</h2>
<table><thead><tr><th>Caliber</th><th class="num">Count</th><th>Share</th></tr></thead>
<tbody>{dist_rows}</tbody></table>"""

    return cards + missing_table + distribution


def generate_report(ammo_root: str, output_path: str):
    """Main report generation."""
    print(f"Loading ammo from {ammo_root} ...")
    records = load_ammo(ammo_root)
    print(f"  Loaded {len(records)} ammo records")

    # Sort by caliber group then class name
    records.sort(key=lambda r: (r["caliber_group"], r["class"]))

    # Pages — main table doesn't need per-row caliber headers (sorted by caliber)
    table_all = _html_table(records, show_cal_group=False)
    cal_charts = _caliber_charts(records)
    mgmt_view = _management_view(records)

    # Build full HTML
    now = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    html = f"""<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>ABE Ammo Report</title>
<style>{CSS}</style>
</head>
<body>

<h1>🔫 ABE Ammo Data Report</h1>
<div class="subtitle">{len(records)} rounds · {now} · Generated by generate_ammo_report.py</div>

<div class="nav-bar">
  <button class="nav-btn active" data-page="table">📊 Table View</button>
  <button class="nav-btn" data-page="charts">📈 Caliber Charts</button>
  <button class="nav-btn" data-page="manage">🔧 Management</button>
</div>

<div class="filter-bar">
  <input type="text" id="search" placeholder="Search by class, model, caliber, source...">
  <select id="caliber-filter">
    <option value="">All Calibers</option>
    {chr(10).join(f'<option value="{escape(g)}">{escape(g)}</option>' for g in sorted(set(r["caliber_group"] for r in records)))}
  </select>
  <select id="type-filter">
    <option value="">All Types</option>
    <option value="fmj">FMJ</option>
    <option value="ap">AP</option>
    <option value="hp">HP</option>
    <option value="sp">SP</option>
    <option value="unknown">Unknown</option>
  </select>
  <label><input type="checkbox" id="hide-incomplete"> Hide incomplete</label>
</div>

<div id="page-table" class="page active">{table_all}</div>
<div id="page-charts" class="page">{cal_charts}</div>
<div id="page-manage" class="page">{mgmt_view}</div>

<script>
// ── Navigation ──
document.querySelectorAll('.nav-btn').forEach(btn => {{
  btn.addEventListener('click', () => {{
    document.querySelectorAll('.nav-btn').forEach(b => b.classList.remove('active'));
    document.querySelectorAll('.page').forEach(p => p.classList.remove('active'));
    btn.classList.add('active');
    document.getElementById('page-' + btn.dataset.page).classList.add('active');
  }});
}});

// ── Column sorting ──
document.querySelectorAll('th[data-col]').forEach(th => {{
  th.addEventListener('click', () => {{
    const table = th.closest('table');
    const col = th.dataset.col;
    const tbody = table.querySelector('tbody');
    const rows = Array.from(tbody.querySelectorAll('tr'));
    const idx = Array.from(th.parentNode.children).indexOf(th);
    const isNum = th.classList.contains('num');
    const descending = th.classList.contains('sorted-asc');

    table.querySelectorAll('th').forEach(h => h.classList.remove('sorted-asc', 'sorted-desc'));
    th.classList.add(descending ? 'sorted-desc' : 'sorted-asc');

    rows.sort((a, b) => {{
      const va = a.children[idx]?.textContent.trim() || '';
      const vb = b.children[idx]?.textContent.trim() || '';
      if (isNum) {{
        const na = parseFloat(va.replace(/[^0-9.-]/g, '')) || 0;
        const nb = parseFloat(vb.replace(/[^0-9.-]/g, '')) || 0;
        return descending ? nb - na : na - nb;
      }}
      return descending ? vb.localeCompare(va) : va.localeCompare(vb);
    }});
    rows.forEach(r => tbody.appendChild(r));
  }});
}});

// ── Search + Filters ──
function applyFilters() {{
  const q = document.getElementById('search').value.toLowerCase();
  const calFilter = document.getElementById('caliber-filter').value;
  const typeFilter = document.getElementById('type-filter').value;
  const hideInc = document.getElementById('hide-incomplete').checked;

  document.querySelectorAll('#page-table tbody tr').forEach(tr => {{
    const txt = tr.textContent.toLowerCase();
    const cal = tr.children[4]?.textContent.trim() || '';
    const typeCell = tr.children[9]?.textContent.trim() || '';
    let show = true;

    if (q && !txt.includes(q)) show = false;
    if (calFilter && !cal.includes(calFilter.replace('mm', '').trim())) show = false;
    if (typeFilter && !typeCell.includes(typeFilter)) show = false;
    if (hideInc) {{
      const mass = parseFloat(tr.children[3]?.textContent) || 0;
      const bc = parseFloat(tr.children[5]?.textContent) || 0;
      if (mass === 0 || bc === 0) show = false;
    }}
    tr.style.display = show ? '' : 'none';
  }});
}}

document.getElementById('search').addEventListener('input', applyFilters);
document.getElementById('caliber-filter').addEventListener('change', applyFilters);
document.getElementById('type-filter').addEventListener('change', applyFilters);
document.getElementById('hide-incomplete').addEventListener('change', applyFilters);
</script>
</body>
</html>"""

    with open(output_path, "w") as f:
        f.write(html)

    print(f"\nReport written to {output_path}")
    print(
        f"  {len(records)} rounds across {len(set(r['caliber_group'] for r in records))} caliber groups"
    )
    return output_path


# ── CLI ──────────────────────────────────────────────────────────────────────


def main():
    parser = argparse.ArgumentParser(description="Generate ABE Ammo HTML Report")
    script_dir = os.path.dirname(os.path.abspath(__file__))
    default_ammo = os.path.normpath(os.path.join(script_dir, "../../data/ammo"))
    default_out = os.path.normpath(
        os.path.join(script_dir, "../../data/reports/ammo_report.html")
    )

    parser.add_argument(
        "--ammo-dir", default=default_ammo, help="Path to data/ammo directory"
    )
    parser.add_argument("--output", default=default_out, help="Output HTML file path")
    args = parser.parse_args()

    generate_report(args.ammo_dir, args.output)


if __name__ == "__main__":
    main()
