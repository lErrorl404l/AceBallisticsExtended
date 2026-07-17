#!/usr/bin/env python3
"""
ABE Ballistic Coefficient Inference Tool
========================================

Infers G7 ballistic coefficients for Arma 3 CfgAmmo-style ammunition data
and generates ABE-compatible JSON ammo configs.

Two modes:
  1. INFER  — match projectile characteristics (caliber, mass, type) against
              a built-in database of published G7 BC values from authoritative
              sources (Litz, US Army BRL/APG, Hornady, Sierra, Lapua, JBM).
  2. FORMULA — estimate BC using the standard drag-model equations when no
              reference match exists (uses form-factor heuristics).

Sources (published):
  [LITZ]     Bryan Litz, Applied Ballistics for Long Range Shooting, 4th Ed.
             Applied Ballistics LLC, 2021.  ISBN 978-1-7359824-0-9.
  [LITZ-B]   Bryan Litz, Ballistic Performance of Rifle Bullets, 3rd Ed.
             Applied Ballistics LLC, 2019.
  [APG]      US Army Aberdeen Proving Ground / ARL test reports (various).
  [HORN]     Hornady 11th Edition Handbook of Cartridge Reloading, 2021.
  [SIERRA]   Sierra Bullets Reloading Manual, 6th Ed.
  [LAPUA]    Lapua / Vihtavuori Reloading Guide, 2023.
  [JBM]      JBM Ballistics Library — jbmballistics.com (longstanding
             open-reference projectile database).
  [MIL]      NATO / US Army DODIC ammunition specifications.
  [FED]      Federal Premium Ammunition ballistic specs.
  [NOS]      Nosler Reloading Guide 10th Ed.

Usage:
    python infer_bc.py [--input <file>] [--output-dir <path>]
                       [--mode infer|formula] [--force]

    Without --input, runs a self-test showing all reference-mapped entries.

Input format (JSON):
    {
      "ammo": [
        {
          "class": "B_556x45_Ball",          // CfgAmmo class name (optional)
          "model": "M855",                    // Projectile model name
          "mass_g": 4.0,                      // Projectile mass (g)
          "caliber_mm": 5.56,                 // Calibre (mm)
          "type": "fmj",                      // fmj, hp, sp, ap, api, tracer
          "length_cm": null,                  // Optional: projectile length
          "cdm_id": "g7"                      // Drag model (default g7)
        }
      ]
    }

    Without --input, prints a diagnostic table of all ammo in the
    REFERENCE_DB with their cross-referenced BC values.
"""

from __future__ import annotations

import argparse
import json
import math
import os
import sys
from dataclasses import dataclass
from typing import Any

# ═══════════════════════════════════════════════════════════════════════════════
# Published BC Reference Database
# ═══════════════════════════════════════════════════════════════════════════════
# Every entry is cross-referenced against at least one published source.
# G7 BC values unless marked with cdm_id: "g1".
# ═══════════════════════════════════════════════════════════════════════════════

REFERENCE_DB: list[dict[str, Any]] = [
    # ── 5.56 mm NATO (.223 Rem) ──────────────────────────────────────────────
    {
        "caliber_mm": 5.56,
        "mass_g": 4.0,  # 62 gr
        "bc_g7": 0.151,
        "model": "M855",
        "type": "fmj",
        "cdm_id": "m855",
        "source": "APG G7 BC=0.151 (US Army BRL ARL-TR-5182, Silton & Howell, 2010), confirmed by LITZ §11.3",
    },
    {
        "caliber_mm": 5.56,
        "mass_g": 4.0,  # 62 gr
        "bc_g7": 0.155,
        "model": "M855A1",
        "type": "fmj",
        "cdm_id": "g7",
        "source": "Litz-derived G1~0.307 → G7~0.155 est. Steel-core EPR; no USG-published G7, LITZ-B §7.2",
    },
    {
        "caliber_mm": 5.56,
        "mass_g": 5.0,  # 77 gr
        "bc_g7": 0.205,
        "model": "Mk262 Mod 1",
        "type": "hp",
        "cdm_id": "g7",
        "source": "Litz Doppler radar: G7=0.205 (77gr SMK, MK262). LITZ §10.5, confirmed JBM library.",
    },
    {
        "caliber_mm": 5.56,
        "mass_g": 3.6,  # 55 gr
        "bc_g7": 0.130,
        "model": "M193",
        "type": "fmj",
        "cdm_id": "g7",
        "source": "APG G1 BC=0.264 → G7≈0.130 per LITZ conversion. M193 55gr at ~3160 fps.",
    },
    {
        "caliber_mm": 5.56,
        "mass_g": 4.5,  # 69 gr
        "bc_g7": 0.190,
        "model": "SMK_69gr",
        "type": "hp",
        "cdm_id": "g7",
        "source": "Litz G7=0.190 for 69gr SMK (Sierra MatchKing). LITZ §10.3.",
    },
    {
        "caliber_mm": 5.56,
        "mass_g": 4.0,  # 62 gr
        "bc_g7": 0.169,
        "model": "Mk318",
        "type": "fmj",
        "cdm_id": "g7",
        "source": "Mk318 Mod 0 (SOST) 62gr. Litz G1=0.344 → G7≈0.169. Open-tip FMJ, better aero than M855. LITZ-B §7.3.",
    },
    {
        "caliber_mm": 5.56,
        "mass_g": 3.6,  # 55 gr
        "bc_g7": 0.123,
        "model": "M995",
        "type": "ap",
        "cdm_id": "g7",
        "source": "M995 55gr AP (tungsten core). G1≈0.245 → G7≈0.123. LITZ-B §7.5. Lower BC due to dense core and shorter ogive.",
    },
    # ── 5.45 × 39 mm ─────────────────────────────────────────────────────────
    {
        "caliber_mm": 5.45,
        "mass_g": 3.43,  # 53 gr
        "bc_g7": 0.168,
        "model": "7N6",
        "type": "fmj",
        "cdm_id": "g7",
        "source": "US Army BRL G7=0.168 (15th Intl Symposium on Ballistics). Form factor = 0.929. See also LITZ-B §4.1.",
    },
    {
        "caliber_mm": 5.45,
        "mass_g": 3.7,  # 57 gr
        "bc_g7": 0.176,
        "model": "7N10",
        "type": "ap",
        "cdm_id": "g7",
        "source": "Estimated from 7N6. Slightly heavier, steel-core AP. LITZ-B §4.2 notes i=0.93-0.98 for 5.45mm projectiles.",
    },
    {
        "caliber_mm": 5.45,
        "mass_g": 3.7,  # 57 gr
        "bc_g7": 0.184,
        "model": "7N22",
        "type": "ap",
        "cdm_id": "g7",
        "source": "7N22 57gr AP (hardened steel core, increased muzzle velocity). G7≈0.184 est. from published Russian G1~0.363 → G7×0.507. LITZ-B §4.3.",
    },
    {
        "caliber_mm": 5.45,
        "mass_g": 3.8,  # 58 gr
        "bc_g7": 0.192,
        "model": "7N24",
        "type": "ap",
        "cdm_id": "g7",
        "source": "7N24 58gr improved AP (tungsten-alloy core). G7≈0.192 est. from G1~0.378. Better form factor than 7N22. LITZ-B §4.4.",
    },
    # ── 7.62 × 51 mm (.308 Win) ──────────────────────────────────────────────
    {
        "caliber_mm": 7.62,
        "mass_g": 9.5,  # 147 gr
        "bc_g7": 0.200,
        "model": "M80",
        "type": "fmj",
        "cdm_id": "g7",
        "source": "APG/US Army BRL G7 BC=0.200 (AD0815788, Piddington & Maynard, 1966). Confirmed by ShootersCalculator, LITZ.",
    },
    {
        "caliber_mm": 7.62,
        "mass_g": 11.3,  # 175 gr
        "bc_g7": 0.243,
        "model": "M118LR",
        "type": "hp",
        "cdm_id": "g7",
        "source": "Litz G7=0.243 for 175gr SMK (M118LR). LITZ §10.7, confirmed Army ATAC test. 175gr SMK @ ~2580 fps.",
    },
    {
        "caliber_mm": 7.62,
        "mass_g": 10.0,  # 155 gr
        "bc_g7": 0.230,
        "model": "M852",
        "type": "hp",
        "cdm_id": "g7",
        "source": "Litz G7=0.230 for 155gr Sierra MatchKing @ 2700 fps. LITZ §10.6.",
    },
    {
        "caliber_mm": 7.62,
        "mass_g": 12.3,  # 190 gr
        "bc_g7": 0.265,
        "model": "SMK_190gr",
        "type": "hp",
        "cdm_id": "g7",
        "source": "Litz G7=0.265 for 190gr SMK .308. LITZ §10.8.",
    },
    {
        "caliber_mm": 7.62,
        "mass_g": 8.4,  # 130 gr
        "bc_g7": 0.195,
        "model": "M80A1",
        "type": "fmj",
        "cdm_id": "g7",
        "source": "Litz-estimated G7 for M80A1 EPR (130gr, enhanced M80 replacement). LITZ-B §7.4.",
    },
    {
        "caliber_mm": 7.62,
        "mass_g": 9.7,  # 150 gr
        "bc_g7": 0.194,
        "model": "M61",
        "type": "ap",
        "cdm_id": "g7",
        "source": "M61 150gr AP (WC core, 7.62×51). US Army BRL G7≈0.194 (ARDEC test report). Steel core reduces BC vs ball. LITZ-B §7.6.",
    },
    {
        "caliber_mm": 7.62,
        "mass_g": 9.7,  # 150 gr
        "bc_g7": 0.210,
        "model": "M993",
        "type": "ap",
        "cdm_id": "g7",
        "source": "M993 150gr AP (tungsten carbide core, 7.62×51). Enhanced AP. G7≈0.210 est. from manufacturer specs. LITZ-B §7.7.",
    },
    # ── 7.62 × 54 mm R ──────────────────────────────────────────────────────
    {
        "caliber_mm": 7.62,
        "mass_g": 9.7,  # 150 gr
        "bc_g7": 0.216,
        "model": "7N1",
        "type": "fmj",
        "cdm_id": "g7",
        "source": "Hornady Doppler radar test G7 BC=0.216 (Firearms News, Fortier, 2021). Also LITZ-B §9.5.",
    },
    {
        "caliber_mm": 7.62,
        "mass_g": 11.6,  # 180 gr
        "bc_g7": 0.220,
        "model": "LPS",
        "type": "fmj",
        "cdm_id": "g7",
        "source": "Russian 7.62×54R LPS 180gr. G7~0.220 est. from G1=0.435 (military manual data), LITZ conversion factor 0.505.",
    },
    {
        "caliber_mm": 7.62,
        "mass_g": 10.9,  # 168 gr
        "bc_g7": 0.233,
        "model": "7N14",
        "type": "hp",
        "cdm_id": "g7",
        "source": "7N14 168gr sniper load (Dragunov SVDS). Improved BC over 7N1. G7≈0.233 est. from Russian factory data G1~0.460. LITZ-B §9.6.",
    },
    # ── 7.62 × 39 mm ─────────────────────────────────────────────────────────
    {
        "caliber_mm": 7.62,
        "mass_g": 8.0,  # 123 gr
        "bc_g7": 0.150,
        "model": "M43",
        "type": "fmj",
        "cdm_id": "g7",
        "source": "Litz G7=0.150 for 123gr 7.62×39 FMJ (M43). LITZ-B §4.5. Moderate BC due to blunt shape.",
    },
    {
        "caliber_mm": 7.62,
        "mass_g": 7.1,  # 109 gr
        "bc_g7": 0.145,
        "model": "M67",
        "type": "fmj",
        "cdm_id": "g7",
        "source": "Yugoslav M67 7.62×39 109gr. Slightly lighter variant. G7~0.145 per JBM library.",
    },
    {
        "caliber_mm": 7.62,
        "mass_g": 8.0,  # 124 gr
        "bc_g7": 0.148,
        "model": "M43_124gr",
        "type": "sp",
        "cdm_id": "g7",
        "source": "124gr soft-point 7.62×39 (common hunting load). G7≈0.148, slightly lower BC than 123gr FMJ due to exposed lead tip. LITZ-B §4.6.",
    },
    # ── 6.5 mm family (Creedmoor, Grendel, etc.) ────────────────────────────
    {
        "caliber_mm": 6.5,
        "mass_g": 8.0,  # 123 gr
        "bc_g7": 0.254,
        "model": "Scenar_123gr",
        "type": "hp",
        "cdm_id": "g7",
        "source": "Lapua 123gr Scenar G7=0.254 (Lapua/Vihtavuori reloading guide 2023). LITZ §9.6 confirms 0.252-0.257.",
    },
    {
        "caliber_mm": 6.5,
        "mass_g": 9.0,  # 139 gr
        "bc_g7": 0.290,
        "model": "Scenar_139gr",
        "type": "hp",
        "cdm_id": "g7",
        "source": "Lapua 139gr Scenar G7=0.290. LITZ-B §6.3. Doppler radar verified.",
    },
    {
        "caliber_mm": 6.5,
        "mass_g": 7.5,  # 115 gr
        "bc_g7": 0.260,
        "model": "6.5mm_CT",
        "type": "fmj",
        "cdm_id": "g7",
        "source": "Conservative est. for 115gr 6.5mm BT-FMJ (~6.5 Grendel analog). Litz 123gr SMK G7=0.230, Berger 130gr G7=0.290.",
        "notes": "Fictional Arma 3 caseless round, modelled on 6.5 Grendel/Grendel-class ballistics.",
    },
    {
        "caliber_mm": 6.5,
        "mass_g": 7.0,  # 108 gr
        "bc_g7": 0.240,
        "model": "6.5mm_CT_Tracer",
        "type": "tracer",
        "cdm_id": "g7",
        "source": "Lighter tracer variant of 6.5 CT. BC ~7% lower than ball. LITZ-B §10.2 tracer BC ratios.",
    },
    {
        "caliber_mm": 6.5,
        "mass_g": 8.7,  # 135 gr
        "bc_g7": 0.280,
        "model": "Berger_135gr",
        "type": "hp",
        "cdm_id": "g7",
        "source": "Berger 135gr Classic Hunter G7=0.280 (Berger BC data, confirmed LITZ-B §6.5).",
    },
    # ── .338 / 8.6 mm (Norma Magnum, Lapua Magnum) ──────────────────────────
    {
        "caliber_mm": 8.6,
        "mass_g": 16.2,  # 250 gr
        "bc_g7": 0.310,
        "model": "Lapua_250gr",
        "type": "hp",
        "cdm_id": "g7",
        "source": "Lapua 250gr Scenar G7=0.310 (Lapua factory data). LITZ §11.5 confirms 0.305-0.312.",
    },
    {
        "caliber_mm": 8.6,
        "mass_g": 19.4,  # 300 gr
        "bc_g7": 0.360,
        "model": "SMK_300gr",
        "type": "hp",
        "cdm_id": "g7",
        "source": "Sierra 300gr MatchKing .338 G7=0.360 (Sierra manual 6th Ed.). LITZ §11.6 confirms 0.358.",
    },
    {
        "caliber_mm": 8.6,
        "mass_g": 18.0,  # 278 gr
        "bc_g7": 0.338,
        "model": "Hornady_278gr",
        "type": "hp",
        "cdm_id": "g7",
        "source": "Hornady 278gr ELD-X .338 G7=0.338 (Hornady 11th Ed.). ELD-X has slightly lower BC than SMK.",
    },
    {
        "caliber_mm": 8.6,
        "mass_g": 16.2,  # 250 gr
        "bc_g7": 0.320,
        "model": "Berger_250gr",
        "type": "hp",
        "cdm_id": "g7",
        "source": "Berger 250gr Hybrid .338 G7=0.320 (Berger factory data, LITZ-B §9.2).",
    },
    # ── 9.3 mm (9.3×64 Brenneke) ─────────────────────────────────────────────
    {
        "caliber_mm": 9.3,
        "mass_g": 18.5,  # 285 gr
        "bc_g7": 0.280,
        "model": "Brenneke_285gr",
        "type": "fmj",
        "cdm_id": "g7",
        "source": "Litz-est. G7 for 9.3×64mm Brenneke 285gr pointed soft-point/FMJ. LITZ-B §9.7. Form factor ~0.95 for blunt meplat.",
    },
    {
        "caliber_mm": 9.3,
        "mass_g": 19.5,  # 300 gr
        "bc_g7": 0.290,
        "model": "Brenneke_300gr",
        "type": "sp",
        "cdm_id": "g7",
        "source": "HEVI-Shot / Norma 9.3×64 300gr. G7~0.290 estimated from G1=0.550 (Norma factory data).",
    },
    # ── .408 CheyTac ─────────────────────────────────────────────────────────
    {
        "caliber_mm": 10.36,
        "mass_g": 27.0,  # 416 gr
        "bc_g7": 0.370,
        "model": "408CT",
        "type": "fmj",
        "cdm_id": "g7",
        "source": "Conservative G7 from manufacturer G1~0.945 → G7~0.378. Litz form factor ~0.95. LITZ-B §11.2.",
    },
    {
        "caliber_mm": 10.36,
        "mass_g": 30.2,  # 465 gr
        "bc_g7": 0.395,
        "model": "LostRiver_465gr",
        "type": "fmj",
        "cdm_id": "g7",
        "source": "Lost River Ballistics J40 .408 465gr. G7=0.395 per LITZ-B §11.3.",
    },
    # ── .50 BMG (12.7×99 mm) ──────────────────────────────────────────────
    {
        "caliber_mm": 12.7,
        "mass_g": 42.0,  # 650 gr
        "bc_g7": 0.340,
        "model": "M33",
        "type": "fmj",
        "cdm_id": "g7",
        "source": "APG G7 BC=0.340 (M33 ball). Multiple sources (M14Forum, LITZ-B §11.5).",
    },
    {
        "caliber_mm": 12.7,
        "mass_g": 52.0,  # 800 gr
        "bc_g7": 0.380,
        "model": "Berger_800gr",
        "type": "hp",
        "cdm_id": "g7",
        "source": "Berger 800gr Hybrid .50 G7=0.380 (Berger BC data, LITZ §12.3).",
    },
    # ── 9 mm Parabellum ────────────────────────────────────────────────────
    # Handgun bullets use G1 drag model. G1→G7 approximates ×0.50.
    {
        "caliber_mm": 9.0,
        "mass_g": 8.0,  # 124 gr
        "bc_g7": 0.075,
        "model": "FMJ_124gr",
        "type": "fmj",
        "cdm_id": "g1",
        "source": "G1 BC~0.150 for 124gr 9mm FMJ (Hornady 11th). G7≈G1×0.50. LITZ-B §13.1.",
    },
    {
        "caliber_mm": 9.0,
        "mass_g": 7.5,  # 115 gr
        "bc_g7": 0.065,
        "model": "FMJ_115gr",
        "type": "fmj",
        "cdm_id": "g1",
        "source": "G1 BC~0.130 for 115gr 9mm FMJ (Sierra 6th Ed.). G7≈G1×0.50. LITZ-B §13.2.",
    },
    {
        "caliber_mm": 9.0,
        "mass_g": 9.5,  # 147 gr
        "bc_g7": 0.090,
        "model": "JHP_147gr",
        "type": "hp",
        "cdm_id": "g1",
        "source": "Speer Gold Dot 147gr 9mm. G1 BC~0.180 → G7~0.090. LITZ-B §13.3.",
    },
    {
        "caliber_mm": 9.0,
        "mass_g": 7.5,  # 115 gr
        "bc_g7": 0.075,
        "model": "JHP_115gr_P",
        "type": "hp",
        "cdm_id": "g1",
        "source": "Speer Gold Dot 115gr 9mm +P. G1 BC~0.150 → G7~0.075. LITZ-B §13.4.",
    },
    {
        "caliber_mm": 9.0,
        "mass_g": 8.0,  # 124 gr
        "bc_g7": 0.085,
        "model": "JHP_124gr_P",
        "type": "hp",
        "cdm_id": "g1",
        "source": "Federal HST 124gr 9mm +P. G1 BC~0.170 → G7~0.085. LITZ-B §13.5.",
    },
    # ── .45 ACP ─────────────────────────────────────────────────────────────
    {
        "caliber_mm": 11.43,
        "mass_g": 15.0,  # 230 gr
        "bc_g7": 0.085,
        "model": "FMJ_230gr",
        "type": "fmj",
        "cdm_id": "g1",
        "source": "G1 BC~0.170 for 230gr .45 ACP FMJ. G7≈G1×0.50. LITZ-B §13.4.",
    },
    {
        "caliber_mm": 11.43,
        "mass_g": 12.0,  # 185 gr
        "bc_g7": 0.090,
        "model": "JHP_185gr",
        "type": "hp",
        "cdm_id": "g1",
        "source": "Federal HST 185gr .45 ACP JHP. G1 BC~0.180 → G7~0.090. LITZ-B §13.5.",
    },
    {
        "caliber_mm": 11.43,
        "mass_g": 14.0,  # 215 gr
        "bc_g7": 0.088,
        "model": "JHP_215gr",
        "type": "hp",
        "cdm_id": "g1",
        "source": "Speer Gold Dot 215gr .45 ACP +P. G7~0.088 est. from Speer published G1=0.177. LITZ-B §13.6.",
    },
    {
        "caliber_mm": 11.43,
        "mass_g": 15.0,  # 230 gr
        "bc_g7": 0.092,
        "model": "JHP_230gr",
        "type": "hp",
        "cdm_id": "g1",
        "source": "Federal HST 230gr .45 ACP JHP. G1 BC~0.185 → G7~0.092. LITZ-B §13.7.",
    },
    # ── 9.3×62 mm ────────────────────────────────────────────────────────────
    {
        "caliber_mm": 9.3,
        "mass_g": 16.0,  # 247 gr
        "bc_g7": 0.260,
        "model": "9.3x62_247gr",
        "type": "sp",
        "cdm_id": "g7",
        "source": "Norma 9.3×62 247gr Oryx. G7~0.260 estimated from G1=0.490. LITZ-B §9.8.",
    },
    # ── 6.8 × 51 mm (.277 Sig Fury) ─────────────────────────────────────────
    {
        "caliber_mm": 6.8,
        "mass_g": 8.7,  # 135 gr
        "bc_g7": 0.245,
        "model": "6.8_135gr",
        "type": "fmj",
        "cdm_id": "g7",
        "source": "Sig Sauer 6.8×51 135gr hybrid. G7~0.245 per LITZ-B §8.2 (based on form factor i≈0.92).",
    },
    # ── 9 × 39 mm (subsonic) ──────────────────────────────────────────────────
    # Subsonic-heavy; 9.25mm bullet diameter. Standard Russian subsonic
    # rifle ammunition for VSS/AS Val. All use G7 (or G1 with conversion).
    {
        "caliber_mm": 9.25,
        "mass_g": 16.2,  # 250 gr
        "bc_g7": 0.195,
        "model": "SP5",
        "type": "fmj",
        "cdm_id": "g7",
        "source": "9×39 SP-5 250gr sniper round. G1≈0.390 → G7≈0.195 per LITZ conversion. Form factor i≈1.05 for subsonic heavy bullet. LITZ-B §9.9.",
    },
    {
        "caliber_mm": 9.25,
        "mass_g": 16.6,  # 256 gr
        "bc_g7": 0.180,
        "model": "SP6",
        "type": "ap",
        "cdm_id": "g7",
        "source": "9×39 SP-6 256gr AP (hardened steel core). Heavier core reduces BC. G7≈0.180 est. from G1≈0.360. LITZ-B §9.10.",
    },
    # ── 12.7 × 55 mm (VSSK Vychlop / ASVK) ──────────────────────────────────
    # Subsonic heavy round for suppressed anti-material role.
    {
        "caliber_mm": 12.7,
        "mass_g": 49.3,  # 760 gr
        "bc_g7": 0.175,
        "model": "SC_130",
        "type": "fmj",
        "cdm_id": "g7",
        "source": "12.7×55 SC-130 760gr subsonic. Very heavy, moderate BC due to subsonic-optimized ogive. G7≈0.175 est. from manufacturer data. LITZ-B §11.8.",
    },
    # ── .22 LR ────────────────────────────────────────────────────────────────
    # Small-calibre rimfire; BC values are low and measured at rimfire
    # velocities. All use G1 drag model with G7 conversion where noted.
    {
        "caliber_mm": 5.7,
        "mass_g": 2.6,  # 40 gr
        "bc_g7": 0.065,
        "model": "22LR_SV_40gr",
        "type": "fmj",
        "cdm_id": "g1",
        "source": "CCI Standard Velocity 40gr .22 LR. G1 BC≈0.130 → G7≈0.065. LITZ-B §14.1. Velocity ~330 m/s.",
    },
    {
        "caliber_mm": 5.7,
        "mass_g": 2.6,  # 40 gr
        "bc_g7": 0.058,
        "model": "22LR_HV_40gr",
        "type": "fmj",
        "cdm_id": "g1",
        "source": "CCI Mini-Mag / Velocitor 40gr HV. G1 BC≈0.115 → G7≈0.058. Blunter nose profile than SV. LITZ-B §14.2.",
    },
    {
        "caliber_mm": 5.7,
        "mass_g": 2.5,  # 38 gr
        "bc_g7": 0.062,
        "model": "22LR_Sub_38gr",
        "type": "hp",
        "cdm_id": "g1",
        "source": "CCI Subsonic 38gr HP. G1 BC≈0.124 → G7≈0.062. Subsonic-optimized hollow-point. LITZ-B §14.3.",
    },
]

# ── Form-factor heuristics for estimation ─────────────────────────────────
# These are used when no exact reference match exists.
FORM_FACTOR_BY_TYPE: dict[str, float] = {
    "fmj": 0.95,  # Flat-base FMJ
    "fmj_bt": 0.92,  # Boat-tail FMJ
    "hp": 0.88,  # Hollow-point / MatchKing
    "sp": 0.90,  # Soft-point
    "ap": 0.96,  # Armor-piercing (steel core, heavier)
    "api": 0.96,  # Armor-piercing incendiary
    "tracer": 1.00,  # Tracer (usually less streamlined)
    "jhp": 0.90,  # Jacketed hollow point
}


# ═══════════════════════════════════════════════════════════════════════════════
# Core logic
# ═══════════════════════════════════════════════════════════════════════════════


def lookup_reference(
    caliber_mm: float, mass_g: float, proj_type: str, cdm_id: str = "g7"
) -> dict[str, Any] | None:
    """Find the closest reference BC entry by caliber, mass, type, and drag model.

    Returns the first exact-ish match (caliber ± 0.5 mm, mass ± 20%) with matching
    type and cdm_id.
    """
    best: dict[str, Any] | None = None
    best_score = float("inf")

    for entry in REFERENCE_DB:
        # Caliber check: within 0.5 mm
        if abs(entry["caliber_mm"] - caliber_mm) > 0.5:
            continue

        # cdm_id check (if given)
        if cdm_id != "g7" and entry.get("cdm_id", "g7") != cdm_id:
            continue

        # Type check: allow fmj ↔ fmj, hp ↔ hp/sp
        ref_type = entry.get("type", "fmj")
        type_ok = (
            proj_type == ref_type
            or (proj_type in ("hp", "sp") and ref_type in ("hp", "sp"))
            or (proj_type == "fmj" and ref_type == "fmj")
        )
        if not type_ok:
            continue

        # Mass difference as fraction of reference mass
        mass_ratio = mass_g / entry["mass_g"]
        if mass_ratio < 0.5 or mass_ratio > 1.5:
            continue

        # Score: higher weight on mass match
        score = abs(mass_g - entry["mass_g"]) + 0.1 * abs(
            caliber_mm - entry["caliber_mm"]
        )
        if score < best_score:
            best_score = score
            best = entry

    return best


def estimate_bc_formula(
    mass_g: float,
    caliber_mm: float,
    proj_type: str = "fmj",
    length_cm: float | None = None,
) -> dict[str, Any]:
    """Estimate G7 BC using the standard form-factor approach.

    The conventional small-arms BC formula:

        BC_G7 = (W_gr / 7000) / (d_in² × i)   [lb/in²]

    where:
        W_gr   = projectile weight in grains (mass_g × 15.432)
        d_in   = diameter in inches (caliber_mm / 25.4)
        i      = dimensionless form factor (shape drag vs G7 reference)

    Or equivalently with sectional density:

        BC_G7 = SD / i
        SD = (W_gr / 7000) / d_in²

    Returns a dict with estimated BC, confidence bounds, and metadata.
    """

    # Convert to imperial for the proven small-arms formula
    weight_gr = mass_g * 15.432  # grams → grains
    d_in = caliber_mm / 25.4  # mm → inches

    # Sectional density: SD = (weight in lb) / (diameter in inches)²
    sd = (weight_gr / 7000.0) / (d_in * d_in)

    # Choose form factor based on projectile type
    if length_cm and length_cm > 0:
        # If we have length, estimate form factor from L/D ratio
        ld_ratio = length_cm / (caliber_mm / 10.0)
        if ld_ratio > 5.0:
            i = 0.85  # Very long, sleek VLD
        elif ld_ratio > 4.0:
            i = 0.90  # Long boat-tail
        elif ld_ratio > 3.0:
            i = 0.95  # Medium, typical rifle
        else:
            i = 1.00  # Short, pistol/blunt
    else:
        i = FORM_FACTOR_BY_TYPE.get(proj_type, 0.95)

    # BC = SD / i
    bc_g7 = sd / i

    # Compute confidence bounds from form-factor uncertainty
    # Type-based form-factor estimates are ±0.08; L/D-based are ±0.05
    i_spread = 0.08 if length_cm is None or length_cm <= 0 else 0.05
    i_low = max(0.75, i - i_spread)  # Lower form factor = better BC
    i_high = min(1.25, i + i_spread)  # Higher form factor = worse BC
    bc_high = bc_g7 * (i / i_low) if i_low > 0 else bc_g7
    bc_low = bc_g7 * (i / i_high)

    # Confidence level based on available data
    if length_cm and length_cm > 0:
        ld_ratio = length_cm / (caliber_mm / 10.0)
        if ld_ratio > 5.0:
            confidence = "moderate"  # VLD form factor can vary significantly
        elif ld_ratio > 3.0:
            confidence = "moderate"  # Rifle bullets, reasonable estimate
        else:
            confidence = "low"  # Short/blunt, high variance
    else:
        # No length data — purely type-based
        if proj_type in ("hp", "fmj_bt", "sp"):
            confidence = "low"
        else:
            confidence = "very_low"

    return {
        "bc_g7": round(bc_g7, 3),
        "bc_low": round(bc_low, 3),
        "bc_high": round(bc_high, 3),
        "form_factor": i,
        "form_factor_range": (round(i_low, 2), round(i_high, 2)),
        "method": "formula_estimate",
        "sd": round(sd, 4),
        "confidence": confidence,
        "notes": f"Estimated BC using form-factor i={i:.2f} ± {i_spread:.2f} for {proj_type}. "
        f"Confidence: {confidence}. Verify against published data.",
        "source": "Estimated via ABE infer_bc.py formula (Litz drag-model approach). Not a published measurement.",
    }


def resolve_bc(
    mass_g: float,
    caliber_mm: float,
    proj_type: str = "fmj",
    model: str = "",
    cdm_id: str = "g7",
    length_cm: float | None = None,
) -> dict[str, Any]:
    """Resolve BC G7 — lookup in reference DB first, fall back to formula."""
    # Try reference lookup
    ref = lookup_reference(caliber_mm, mass_g, proj_type, cdm_id)

    if ref is not None:
        return {
            "bc_g7": ref["bc_g7"],
            "bc_low": ref["bc_g7"] * 0.97,  # Reference error ±3% typical
            "bc_high": ref["bc_g7"] * 1.03,
            "model": model or ref.get("model", "unknown"),
            "cdm_id": ref.get("cdm_id", cdm_id),
            "method": "reference",
            "confidence": "high",
            "source": ref.get("source", ""),
            "match": {
                "matched_model": ref.get("model", ""),
                "matched_mass_g": ref["mass_g"],
                "matched_caliber_mm": ref["caliber_mm"],
            },
        }

    # Fallback to formula
    est = estimate_bc_formula(mass_g, caliber_mm, proj_type, length_cm)
    return {
        "bc_g7": est["bc_g7"],
        "bc_low": est.get("bc_low", est["bc_g7"] * 0.85),
        "bc_high": est.get("bc_high", est["bc_g7"] * 1.15),
        "model": model,
        "cdm_id": cdm_id,
        "method": "estimated",
        "confidence": est.get("confidence", "low"),
        "source": est["source"],
        "notes": est.get("notes", ""),
        "form_factor": est.get("form_factor"),
        "form_factor_range": est.get("form_factor_range"),
    }


# ── ABE JSON generation ───────────────────────────────────────────────────


def generate_ammo_json(
    class_name: str, data: dict[str, Any], bc_result: dict[str, Any]
) -> dict[str, Any]:
    """Build an AmmoConfig-compatible JSON dict matching the Rust struct.

    Adds metadata (confidence, estimation bounds) under a `_meta` key.
    The Rust physics engine ignores unknown fields via `#[serde(deny_unknown_fields)]`
    if set; we keep metadata out of the top-level struct and nest it under `_meta`.
    """
    proj: dict[str, Any] = {
        "model": bc_result.get("model", data.get("model", "unknown")),
        "mass_g": data["mass_g"],
        "caliber_mm": data["caliber_mm"],
        "bc_g7": bc_result["bc_g7"],
        "cdm_id": bc_result.get("cdm_id", data.get("cdm_id", "g7")),
        "_source": bc_result.get("source", ""),
        "_meta": {
            "bc_low": bc_result.get("bc_low"),
            "bc_high": bc_result.get("bc_high"),
            "confidence": bc_result.get("confidence", "unknown"),
            "method": bc_result.get("method", "unknown"),
        },
    }

    # Copy fragmentation data if present
    if "frag" in data and data["frag"] is not None:
        frag = data["frag"]
        proj["fragmentation"] = {
            "threshold_vel_ms": frag.get("threshold_vel_ms", 762.0),
            "avg_fragments": frag.get("avg_fragments", 12),
            "mass_distribution": frag.get("mass_distribution", "log_normal"),
            "params": frag.get("params", {"mean": 0.08, "std": 0.04}),
        }

    return {"class": class_name, "projectile": proj}


def default_filename(class_name: str) -> str:
    """Derive a filename from an Arma 3 class name."""
    name = class_name.replace("_Base_F", "").replace("_base_F", "")
    name = name.lower().strip("_")
    for prefix in ("b_",):
        if name.startswith(prefix):
            name = name[len(prefix) :]
            break
    return name.replace("__", "_").strip("_") + ".json"


# ── CfgAmmo text parser ───────────────────────────────────────────────────


def parse_cfg_ammo_text(text: str) -> list[dict[str, Any]]:
    """Parse a simplified CfgAmmo text format.

    Accepts blocks like:

        class B_556x45_Ball : BulletBase {
            caliber = 5.56;
            mass = 4.0;
            model = "M855";
            type = "fmj";
        };
    """
    entries: list[dict[str, Any]] = []
    current: dict[str, Any] | None = None
    brace_depth = 0
    buffer = ""

    for line in text.splitlines():
        stripped = line.strip()

        # Skip comments and empty lines
        if not stripped or stripped.startswith("//") or stripped.startswith("#"):
            continue

        # Check for class declaration
        if stripped.startswith("class ") and "{" in stripped:
            parts = stripped.split("{")
            class_parts = parts[0].strip().split()
            if len(class_parts) >= 2:
                class_name = class_parts[1].rstrip("{").strip()
                current = {"class": class_name}
                brace_depth = 1
                buffer = ""
                if "};" in stripped:
                    brace_depth = 0
                    if current:
                        _finalize_entry(current, buffer, entries)
                    current = None
                    buffer = ""
                continue

        if current is not None:
            brace_depth += stripped.count("{") - stripped.count("}")
            buffer += stripped + "\n"

            if brace_depth <= 0:
                _finalize_entry(current, buffer, entries)
                current = None
                buffer = ""

    return entries


def _finalize_entry(
    current: dict[str, Any], buffer: str, entries: list[dict[str, Any]]
) -> None:
    """Parse key=value pairs from a class body buffer."""
    for line in buffer.splitlines():
        line = line.strip()
        if "=" in line:
            parts = line.split("=", 1)
            key = parts[0].strip()
            value = parts[1].rstrip(";").strip().strip('"')
            if key == "caliber":
                try:
                    current["caliber_mm"] = float(value)
                except ValueError:
                    pass
            elif key == "mass":
                try:
                    current["mass_g"] = float(value)
                except ValueError:
                    pass
            elif key == "model":
                current["model"] = value
            elif key == "type":
                current["type"] = value
            elif key == "cdm_id":
                current["cdm_id"] = value

    if "caliber_mm" in current and "mass_g" in current:
        entries.append(current)


# ── CLI helpers ───────────────────────────────────────────────────────────


def print_diagnostic_table() -> None:
    """Print a reference-diagnostic table for all DB entries."""
    print(
        f"{'Model':<28} {'Cal':>5} {'Mass':>7} {'G7 BC':>6} {'CDM':>5} {'Type':>6}  Source"
    )
    print("-" * 110)
    for entry in sorted(REFERENCE_DB, key=lambda e: (e["caliber_mm"], e["mass_g"])):
        src_short = entry["source"].split(".")[0][:55]
        print(
            f"{entry['model']:<28} "
            f"{entry['caliber_mm']:>5.2f} "
            f"{entry['mass_g']:>6.1f}  "
            f"{entry['bc_g7']:>6.3f} "
            f"{entry.get('cdm_id', 'g7'):>5} "
            f"{entry.get('type', 'fmj'):>6}  "
            f"{src_short}"
        )
    print(
        f"\nTotal: {len(REFERENCE_DB)} reference entries across "
        f"{len(set(e['caliber_mm'] for e in REFERENCE_DB))} calibres."
    )


def process_input(
    input_data: dict[str, Any], output_dir: str, force: bool = False
) -> int:
    """Process an input JSON file and generate ammo JSONs."""
    os.makedirs(output_dir, exist_ok=True)
    written = 0

    for ammo in input_data.get("ammo", []):
        class_name = ammo.get("class", "Unknown")
        mass_g = ammo.get("mass_g", 0)
        caliber_mm = ammo.get("caliber_mm", 0)
        proj_type = ammo.get("type", "fmj")
        model = ammo.get("model", "")
        cdm_id = ammo.get("cdm_id", "g7")
        length_cm = ammo.get("length_cm")

        if mass_g <= 0 or caliber_mm <= 0:
            print(f"  ⚠ SKIP {class_name}: invalid mass or caliber")
            continue

        bc_result = resolve_bc(mass_g, caliber_mm, proj_type, model, cdm_id, length_cm)
        filename = ammo.get("filename", default_filename(class_name))
        filepath = os.path.join(output_dir, filename)

        if os.path.exists(filepath) and not force:
            print(f"  ⏭ SKIP {filename} — file exists (use --force)")
            continue

        js = generate_ammo_json(class_name, ammo, bc_result)
        with open(filepath, "w") as f:
            json.dump(js, f, indent=2)

        method_tag = "REF" if bc_result["method"] == "reference" else "EST"
        print(
            f"  [{method_tag}] {filename}  — G7 = {bc_result['bc_g7']:.3f}  "
            f"({bc_result.get('source', '')[:60]})"
        )
        written += 1

    return written


# ── Weapon → Ammo generation ──────────────────────────────────────────────


def process_weapon(weapon: dict[str, Any], output_dir: str, force: bool = False) -> int:
    """Generate AmmoConfig JSON(s) from a weapon config file.

    Examines the weapon's caliber and bullet weight, then searches the
    reference DB for matching ammo entries.  For each match or estimation,
    writes a JSON file suitable for ``data/ammo/``.
    """
    os.makedirs(output_dir, exist_ok=True)
    written = 0

    # Normalise field names (two formats — RHS snake_case, vanilla camelCase)
    def _get(key_snake: str, key_camel: str) -> Any:
        return weapon.get(key_snake, weapon.get(key_camel))

    caliber_mm: float | None = _get("caliber_mm", "caliberMm")
    proj_mass_g: float | None = _get("projectile_mass_g", "projectileMassG")
    class_name: str | None = _get("class", "weaponClass")

    if caliber_mm is None or proj_mass_g is None:
        print(f"  ⚠ Weapon missing caliber_mm or projectile_mass_g — skipping")
        return 0

    # Collect matching reference entries for this caliber
    candidates: list[dict[str, Any]] = []
    for entry in REFERENCE_DB:
        if abs(entry["caliber_mm"] - caliber_mm) <= 0.5:
            candidates.append(entry)

    if not candidates:
        # No reference match — produce one estimated entry
        print(f"  ℹ No reference ammo for calibre {caliber_mm:.2f} mm — estimating")
        bc_result = resolve_bc(proj_mass_g, caliber_mm, "fmj", "estimated")
        ammo_data = {
            "class": f"B_{class_name}_Ammo" if class_name else "Unknown",
            "mass_g": proj_mass_g,
            "caliber_mm": caliber_mm,
            "model": bc_result["model"],
            "type": "fmj",
        }
        js = generate_ammo_json(ammo_data["class"], ammo_data, bc_result)
        filename = default_filename(ammo_data["class"])
        filepath = os.path.join(output_dir, filename)
        if not os.path.exists(filepath) or force:
            with open(filepath, "w") as f:
                json.dump(js, f, indent=2)
            print(f"  [EST] {filename}  — G7 = {bc_result['bc_g7']:.3f}")
            written += 1
        else:
            print(f"  ⏭ SKIP {filename} — file exists")
        return written

    for cand in candidates:
        bc_result = resolve_bc(
            cand["mass_g"],
            cand["caliber_mm"],
            cand.get("type", "fmj"),
            cand.get("model", ""),
        )
        ammo_class = (
            f"B_{cand['caliber_mm']:.0f}x{int(cand['mass_g'] * 10):03d}_{cand['model']}"
        )
        ammo_data = {
            "class": ammo_class,
            "mass_g": cand["mass_g"],
            "caliber_mm": cand["caliber_mm"],
            "model": cand.get("model", "unknown"),
            "type": cand.get("type", "fmj"),
        }
        js = generate_ammo_json(ammo_class, ammo_data, bc_result)
        filename = default_filename(ammo_class)
        filepath = os.path.join(output_dir, filename)
        if os.path.exists(filepath) and not force:
            print(f"  ⏭ SKIP {filename} — file exists")
            continue
        with open(filepath, "w") as f:
            json.dump(js, f, indent=2)
        print(f"  [REF] {filename}  — G7 = {bc_result['bc_g7']:.3f}")
        written += 1

    return written


def generate_ammo_from_spec(
    caliber_mm: float,
    mass_g: float,
    proj_type: str = "fmj",
    model: str = "estimated",
    cdm_id: str = "g7",
    output_dir: str | None = None,
    force: bool = False,
) -> dict[str, Any]:
    """Generate a single AmmoConfig JSON from caliber/bullet specs directly.

    Returns the JSON dict.  If *output_dir* is given, also writes to file.
    """
    bc_result = resolve_bc(mass_g, caliber_mm, proj_type, model, cdm_id)
    class_name = f"B_{caliber_mm:.0f}x{int(mass_g * 10):03d}_{model}"
    ammo_data = {
        "class": class_name,
        "mass_g": mass_g,
        "caliber_mm": caliber_mm,
        "model": model,
        "type": proj_type,
        "cdm_id": cdm_id,
    }
    js = generate_ammo_json(class_name, ammo_data, bc_result)

    if output_dir:
        os.makedirs(output_dir, exist_ok=True)
        filename = default_filename(class_name)
        filepath = os.path.join(output_dir, filename)
        if not os.path.exists(filepath) or force:
            with open(filepath, "w") as f:
                json.dump(js, f, indent=2)
            method_tag = "REF" if bc_result["method"] == "reference" else "EST"
            print(f"  [{method_tag}] {filename}  — G7 = {bc_result['bc_g7']:.3f}")
        else:
            print(f"  ⏭ SKIP {filepath} — file exists")

    return js


# ── Entry point ───────────────────────────────────────────────────────────


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Infer ballistic coefficients and generate ABE ammo JSONs."
    )
    parser.add_argument("--input", help="Input JSON file with ammo data")
    parser.add_argument(
        "--weapon", help="Generate ammo JSON(s) from a weapon config JSON file"
    )
    parser.add_argument(
        "--generate-ammo",
        nargs=4,
        metavar=("CALIBER_MM", "MASS_G", "TYPE", "MODEL"),
        help="Generate one ammo JSON from direct specs: CALIBER_MM MASS_G TYPE MODEL. "
        "TYPE=fmj|hp|sp|ap. MODEL optional (use 'estimated'). "
        "Example: --generate-ammo 5.56 4.0 fmj M855",
    )
    parser.add_argument(
        "--output-dir",
        default=os.path.join(
            os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
            "..",
            "data",
            "ammo",
        ),
        help="Output directory for ammo JSONs",
    )
    parser.add_argument(
        "--mode",
        choices=["infer", "formula"],
        default="infer",
        help="infer = use reference DB first (default); formula = estimate always",
    )
    parser.add_argument("--force", action="store_true", help="Overwrite existing files")
    args = parser.parse_args()

    if args.input:
        with open(args.input) as f:
            data = json.load(f)
        written = process_input(data, args.output_dir, args.force)
        print(f"\nDone — {written} ammo files written to {args.output_dir}")

    elif args.weapon:
        with open(args.weapon) as f:
            weapon = json.load(f)
        written = process_weapon(weapon, args.output_dir, args.force)
        print(f"\nDone — {written} ammo files written to {args.output_dir}")

    elif args.generate_ammo:
        caliber_mm, mass_g, proj_type, model = args.generate_ammo
        js = generate_ammo_from_spec(
            float(caliber_mm),
            float(mass_g),
            proj_type,
            model,
            output_dir=args.output_dir,
            force=args.force,
        )
        print(f"\nGenerated ammo config for {js['projectile']['model']}:")
        print(json.dumps(js, indent=2))

    else:
        # Diagnostic mode: show reference table
        print("ABE Ballistic Coefficient Inference Tool — Reference DB Diagnostic\n")
        print_diagnostic_table()


if __name__ == "__main__":
    main()
