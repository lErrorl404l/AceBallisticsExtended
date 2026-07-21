#!/usr/bin/env python3
"""Merge research data from all agents into real_weapons.tsv"""

import csv, re, sys
from collections import defaultdict

TSV_PATH = "/ext/Development/AceBallisticsExtention/data/real_weapons.tsv"

# ── Cartridge defaults (pressure_mpa, mass_g, velocity_ms) from SAAMI/CIP ──
CARTRIDGE_DEFAULTS = {
    # (caliber_mm, cartridge_key) -> (pressure_mpa, mass_g, velocity_ms, source)
    (5.56, "5.56×45mm NATO"): (430, 4.0, 900, "NATO EPVAT/M855"),
    (5.45, "5.45×39mm"): (355, 3.4, 880, "CIP/7N6"),
    (7.62, "7.62×51mm NATO"): (427, 9.5, 856, "SAAMI .308/M80"),
    (7.62, "7.62×39mm"): (355, 8.0, 710, "CIP/M43"),
    (7.62, "7.62×54mmR"): (390, 9.5, 830, "CIP/LPS"),
    (9.01, "9×19mm Parabellum"): (235, 8.0, 360, "CIP/124gr FMJ"),
    (9.0, "9×39mm"): (355, 16.8, 290, "CIP/SP-5"),
    (11.43, ".45 ACP"): (145, 15.0, 255, "SAAMI/230gr ball"),
    (5.7, "5.7×28mm"): (345, 2.0, 715, "CIP/SS190"),
    (4.6, "4.6×30mm"): (400, 1.9, 725, "CIP/DM11"),
    (12.7, "12.7×99mm NATO (.50 BMG)"): (379, 42.8, 860, "TM43/M33"),
    (8.58, ".338 Lapua Magnum"): (420, 16.2, 900, "CIP/250gr"),
    (10.36, ".408 CheyTac"): (440, 19.8, 1067, "CIP/305gr"),
    (6.5, "6.5mm"): (380, 7.0, 800, "generic"),
    (12.0, "12 Gauge"): (0, 28.0, 480, "SAAMI/1oz slug"),
    (18.5, "18.5mm"): (0, 28.0, 480, "SAAMI/1oz slug"),
    (7.62, "7.62×25mm Tokarev"): (250, 5.5, 470, "CIP/85gr"),
    (9.0, "9×18mm Makarov"): (160, 6.1, 300, "CIP/95gr"),
    (12.7, "12.7×108mm"): (360, 49.0, 820, "CIP/B-32"),
    (20.0, "20mm"): (0, 101.0, 1030, "NATO/M103"),
    (30.0, "30mm"): (0, 350.0, 1000, "PGU-14"),
    (40.0, "40mm"): (0, 0, 0, "launcher"),
    (84.0, "84mm"): (0, 0, 0, "launcher"),
    (93.0, "93mm"): (0, 0, 0, "launcher"),
    (72.0, "72mm"): (0, 0, 0, "launcher"),
    (70.0, "70mm"): (0, 0, 0, "launcher"),
    (127.0, "127mm"): (0, 0, 0, "launcher"),
    (66.0, "66mm"): (0, 0, 0, "launcher"),
    (90.0, "90mm"): (0, 0, 0, "launcher"),
    (105.0, "105mm"): (0, 0, 0, "launcher"),
    (72.5, "72.5mm"): (0, 0, 0, "launcher"),
    (64.0, "64mm"): (0, 0, 0, "launcher"),
    # .300 Win Mag / .300 BLK not in catalog but add for completeness
    (7.62, ".300 Winchester Magnum"): (441, 12.3, 880, "SAAMI/190gr"),
    (7.62, ".300 AAC Blackout"): (380, 8.1, 700, "SAAMI/125gr"),
}

# ── Weapon-specific barrel & twist data ──
# weapon_id -> (barrel_mm, twist_mm, twist_dir, variant_suffix_for_new_row)
WEAPON_BARRELS = {
    # Colt
    "colt_m4a1": (368, 178, 1, ""),
    "colt_m4a1_block_ii": (368, 178, 1, ""),
    "colt_m16a4": (508, 178, 1, ""),
    # H&K rifles
    "heckler_koch_hk416": (368, 178, 1, ""),
    "heckler_koch_hk416a5": (368, 178, 1, ""),
    "heckler_koch_hk417": (419, 279, 1, ""),
    "heckler_koch_g36": (480, 178, 1, ""),
    "heckler_koch_g36a2": (480, 178, 1, ""),
    "heckler_koch_g3": (450, 305, 1, ""),
    "heckler_koch_hk33": (390, 178, 1, ""),
    "heckler_koch_hk53": (211, 178, 1, ""),
    "heckler_koch_msg90": (600, 305, 1, ""),
    "heckler_koch_psg1": (650, 305, 1, ""),
    "heckler_koch_mp5": (225, 250, 1, ""),
    "heckler_koch_mp7": (180, 160, 1, ""),
    "heckler_koch_ump_45": (200, 406, 1, ""),
    "heckler_koch_mp5k": (115, 250, 1, ""),
    "heckler_koch_mp5sd": (146, 250, 1, ""),
    "heckler_koch_mg4": (482, 178, 1, ""),
    "heckler_koch_mg5": (550, 305, 1, ""),
    # HK pistols
    "heckler_koch_usp": (108, 250, 1, ""),
    "heckler_koch_mk23": (149, 378, 1, ""),
    "heckler_koch_p7": (105, 250, 1, ""),
    # FN
    "fn_herstal_scar_l": (355, 178, 1, ""),
    "fn_herstal_scar_h": (406, 279, 1, ""),
    "fn_herstal_fal": (533, 305, 1, ""),
    "fn_herstal_f2000": (400, 178, 1, ""),
    "fn_herstal_p90": (264, 231, 1, ""),
    "fn_herstal_minimi_m249": (465, 178, 1, ""),
    "fn_herstal_mag_m240": (630, 305, 1, ""),
    "fn_herstal_fnx_45": (114, 406, 1, ""),
    "fn_herstal_five_seven": (122, 231, 1, ""),
    # Steyr
    "steyr_aug": (508, 229, 1, ""),
    "steyr_aug_a3": (417, 229, 1, ""),
    "steyr_ssg_69": (650, 305, 1, ""),
    # IWI
    "iwi_tavor_x95": (330, 178, 1, ""),
    "iwi_galil_ace": (419, 178, 1, ""),
    "iwi_negev": (460, 178, 1, ""),
    # IMI
    "imi_uzi": (260, 254, 1, ""),
    # BAE
    "bae_systems_l85a2": (518, 178, 1, ""),
    "bae_systems_l86a2_lsw": (646, 178, 1, ""),
    # Accuracy International
    "accuracy_internation_awm": (686, 279, 1, ""),
    "accuracy_internation_aw": (660, 305, 1, ""),
    "accuracy_internation_ax50": (686, 381, 1, ""),
    # LMT
    "lewis_machine_tool_l129a1": (406, 286, 1, ""),
    # Kalashnikov
    "kalashnikov_concern_ak_74m": (415, 200, 1, ""),
    "kalashnikov_concern_ak_74": (415, 200, 1, ""),
    "kalashnikov_concern_ak_47": (415, 240, 1, ""),
    "kalashnikov_concern_akm": (415, 240, 1, ""),
    "kalashnikov_concern_ak_103": (415, 240, 1, ""),
    "kalashnikov_concern_ak_105": (314, 200, 1, ""),
    "kalashnikov_concern_ak_12": (415, 200, 1, ""),
    "kalashnikov_concern_rpk_74": (590, 200, 1, ""),
    "kalashnikov_concern_pkm": (645, 240, 1, ""),
    "kalashnikov_concern_pkp_pecheneg": (658, 240, 1, ""),
    # Tula / Izhmash
    "tula_arms_plant_sv_98": (650, 320, 1, ""),
    "degtyaryov_plant_rpk_16": (550, 200, 1, ""),
    "izhmash_svd_dragunov": (620, 320, 1, ""),
    "izhmash_svds": (565, 320, 1, ""),
    # TsKIB
    "tskib_soo_vss_vintorez": (200, 250, 1, ""),
    "tskib_soo_as_val": (200, 250, 1, ""),
    # KBP
    "kbp_pp_19_bizon": (225, 240, 1, ""),
    # RPK-74M
    "zavod_imeni_degtyary_rpk_74m": (590, 200, 1, ""),
    # Glock
    "glock_glock_17": (114, 250, 1, ""),
    "glock_glock_18": (114, 250, 1, ""),
    "glock_glock_19": (102, 250, 1, ""),
    # Sig Sauer
    "sig_sauer_p226": (112, 250, 1, ""),
    "sig_sauer_p320": (120, 250, 1, ""),
    "sig_sauer_p229": (99, 250, 1, ""),
    "sig_sauer_p220": (112, 406, 1, ""),
    "sig_sauer_mcx": (406, 178, 1, ""),
    # Barrett
    "barrett_m82_m107": (737, 381, 1, ""),
    "barrett_m95": (737, 381, 1, ""),
    "barrett_mrad": (660, 240, 1, ""),
    # Remington
    "remington_m24": (610, 286, 1, ""),
    "remington_m40a5": (635, 305, 1, ""),
    "remington_m2010": (610, 254, 1, ""),
    "remington_m700": (610, 305, 1, ""),
    "remington_m870": (470, 0, 0, ""),
    # McMillan
    "mcmillan_tac_50": (737, 381, 1, ""),
    # Knight's Armament
    "knight_s_armament_m110": (508, 279, 1, ""),
    "knight_s_armament_sr_25": (610, 286, 1, ""),
    "knight_s_armament_mk_11": (508, 279, 1, ""),
    # Sako
    "sako_trg_22": (660, 280, 1, ""),
    "sako_trg_42": (690, 254, 1, ""),
    # PGM
    "pgm_h_cate_ii": (700, 381, 1, ""),
    # Beretta
    "beretta_m9": (125, 250, 1, ""),
    "beretta_m9a3": (125, 250, 1, ""),
    # Smith & Wesson
    "smith_wesson_m_p": (108, 254, 1, ""),
    # Shotguns
    "benelli_m1014": (470, 0, 0, ""),
    "franchi_spas_12": (457, 0, 0, ""),
    "mossberg_m590": (508, 0, 0, ""),
    # B&T
    "br_gger_thomet_mp9": (130, 250, 1, ""),
    "br_gger_thomet_apc9": (175, 250, 1, ""),
    # MAGPUL
    "magpul_pdr": (267, 178, 1, ""),
    # RPC Fort
    "rpc_fort_fort_221": (468, 178, 1, ""),
    # MAC
    "mac_mac_10": (146, 406, 1, ""),
    "mac_mac_11": (133, 254, 1, ""),
    # Russian HMG
    "russian_federation_nsv_utes": (1070, 340, 1, ""),
    "russian_federation_kord": (1070, 340, 1, ""),
    # Browning
    "browning_m2hb": (1143, 381, 1, ""),
    # Rheinmetall
    "rheinmetall_mg3": (565, 305, 1, ""),
    # SIG MG 338
    "sig_mg_338": (610, 254, 1, ""),
    # M249 Para
    "aai_corporation_m249_para": (349, 178, 1, ""),
    # Launchers — add Carl Gustaf as rifled
    "saab_bofors_dynamics_carl_gustaf_m3": (1130, 840, 1, ""),
    "saab_bofors_dynamics_carl_gustaf_m4": (950, 840, 1, ""),
    # CheyTac
    "cheytac_m200_intervention": (737, 330, 1, ""),
    # Desert Tactical
    "desert_tactical_srs_stealth_recon_scout": (660, 254, 1, ""),
}

# ── Additional variant rows to add ──
# (new_weapon_id, base_id, barrel_mm, twist_mm, twist_dir, variant_desc)
ADDITIONAL_VARIANTS = [
    # HK416 variants
    ("heckler_koch_hk416_10", "heckler_koch_hk416", 264, 178, 1, '10.4" barrel'),
    ("heckler_koch_hk416_165", "heckler_koch_hk416", 419, 178, 1, '16.5" barrel'),
    ("heckler_koch_hk416_20", "heckler_koch_hk416", 508, 178, 1, '20" barrel'),
    # HK417 variants
    ("heckler_koch_hk417_13", "heckler_koch_hk417", 330, 279, 1, '13" barrel'),
    ("heckler_koch_hk417_20", "heckler_koch_hk417", 508, 279, 1, '20" barrel'),
    # AUG variants
    ("steyr_aug_16", "steyr_aug", 417, 229, 1, '16" barrel'),
    # SCAR variants
    ("fn_herstal_scar_l_cqc", "fn_herstal_scar_l", 254, 178, 1, '10" CQC barrel'),
    ("fn_herstal_scar_l_lb", "fn_herstal_scar_l", 457, 178, 1, '18" LB barrel'),
    ("fn_herstal_scar_h_cqc", "fn_herstal_scar_h", 330, 279, 1, '13" CQC barrel'),
    ("fn_herstal_scar_h_lb", "fn_herstal_scar_h", 508, 279, 1, '20" LB barrel'),
    # Barrett M82 variants
    ("barrett_m82_m107_cqc", "barrett_m82_m107", 508, 381, 1, '20" CQC barrel'),
    # AK-12 short
    (
        "kalashnikov_concern_ak_12_k",
        "kalashnikov_concern_ak_12",
        290,
        200,
        1,
        "Short barrel",
    ),
    # RPK-16 short
    (
        "degtyaryov_plant_rpk_16_short",
        "degtyaryov_plant_rpk_16",
        370,
        200,
        1,
        "Short barrel",
    ),
    # SR-25 variants
    (
        "knight_s_armament_sr_25_20",
        "knight_s_armament_sr_25",
        508,
        286,
        1,
        '20" barrel',
    ),
    (
        "knight_s_armament_sr_25_16",
        "knight_s_armament_sr_25",
        406,
        286,
        1,
        '16" carbine',
    ),
    # Sako TRG short
    ("sako_trg_22_short", "sako_trg_22", 510, 280, 1, '20" barrel'),
    ("sako_trg_42_short", "sako_trg_42", 510, 254, 1, '20" barrel'),
    # M700 variant
    ("remington_m700_hvy", "remington_m700", 660, 305, 1, '26" heavy barrel'),
    # PSG1 as the base PSG1 is already in WEAPON_BARRELS
]

# Update cartridge mapping for specific non-default calibers
# (weapon_id -> (caliber_mm, cartridge))
CALIBER_OVERRIDES = {
    "kalashnikov_concern_ak_47": (7.62, "7.62×39mm"),
    "kalashnikov_concern_akm": (7.62, "7.62×39mm"),
    "kalashnikov_concern_ak_103": (7.62, "7.62×39mm"),
    "heckler_koch_hk417": (7.62, "7.62×51mm NATO"),
    "heckler_koch_g3": (7.62, "7.62×51mm NATO"),
    "heckler_koch_msg90": (7.62, "7.62×51mm NATO"),
    "heckler_koch_psg1": (7.62, "7.62×51mm NATO"),
    "heckler_koch_mg5": (7.62, "7.62×51mm NATO"),
    "fn_herstal_scar_h": (7.62, "7.62×51mm NATO"),
    "fn_herstal_mag_m240": (7.62, "7.62×51mm NATO"),
    "fn_herstal_fal": (7.62, "7.62×51mm NATO"),
    "lewis_machine_tool_l129a1": (7.62, "7.62×51mm NATO"),
    "remington_m24": (7.62, "7.62×51mm NATO"),
    "remington_m40a5": (7.62, "7.62×51mm NATO"),
    "remington_m700": (7.62, "7.62×51mm NATO"),
    "remington_m700_hvy": (7.62, "7.62×51mm NATO"),
    "knight_s_armament_m110": (7.62, "7.62×51mm NATO"),
    "knight_s_armament_sr_25": (7.62, "7.62×51mm NATO"),
    "knight_s_armament_mk_11": (7.62, "7.62×51mm NATO"),
    "sako_trg_22": (7.62, "7.62×51mm NATO"),
    "sako_trg_22_short": (7.62, "7.62×51mm NATO"),
    "steyr_ssg_69": (7.62, "7.62×51mm NATO"),
    "accuracy_internation_aw": (7.62, "7.62×51mm NATO"),
    "heckler_koch_mp5": (9.01, "9×19mm Parabellum"),
    "heckler_koch_mp5k": (9.01, "9×19mm Parabellum"),
    "heckler_koch_mp5sd": (9.01, "9×19mm Parabellum"),
    "br_gger_thomet_mp9": (9.01, "9×19mm Parabellum"),
    "br_gger_thomet_apc9": (9.01, "9×19mm Parabellum"),
    "heckler_koch_usp": (9.01, "9×19mm Parabellum"),
    "heckler_koch_p7": (9.01, "9×19mm Parabellum"),
    "heckler_koch_mk23": (11.43, ".45 ACP"),
    "heckler_koch_ump_45": (11.43, ".45 ACP"),
    "mac_mac_10": (11.43, ".45 ACP"),
    "mac_mac_11": (9.01, "9×19mm Parabellum"),
    "fn_herstal_fnx_45": (11.43, ".45 ACP"),
    "sig_sauer_p220": (11.43, ".45 ACP"),
    "tskib_soo_vss_vintorez": (9.0, "9×39mm"),
    "tskib_soo_as_val": (9.0, "9×39mm"),
    "accuracy_internation_awm": (8.58, ".338 Lapua Magnum"),
    "sako_trg_42": (8.58, ".338 Lapua Magnum"),
    "sako_trg_42_short": (8.58, ".338 Lapua Magnum"),
    "cheytac_m200_intervention": (10.36, ".408 CheyTac"),
    "pgm_h_cate_ii": (12.7, "12.7×99mm NATO (.50 BMG)"),
    "mcmillan_tac_50": (12.7, "12.7×99mm NATO (.50 BMG)"),
    "barrett_m95": (12.7, "12.7×99mm NATO (.50 BMG)"),
    "accuracy_internation_ax50": (12.7, "12.7×99mm NATO (.50 BMG)"),
    "browning_m2hb": (12.7, "12.7×99mm NATO (.50 BMG)"),
    "russian_federation_nsv_utes": (12.7, "12.7×108mm"),
    "russian_federation_kord": (12.7, "12.7×108mm"),
    "rheinmetall_mg3": (7.62, "7.62×51mm NATO"),
    "heckler_koch_mg4": (5.56, "5.56×45mm NATO"),
    "aai_corporation_m249_para": (5.56, "5.56×45mm NATO"),
    "sig_mg_338": (8.58, ".338 Lapua Magnum"),
    "fn_herstal_p90": (5.7, "5.7×28mm"),
    "heckler_koch_mp7": (4.6, "4.6×30mm"),
    "fn_herstal_five_seven": (5.7, "5.7×28mm"),
    "remington_m2010": (7.62, ".300 Winchester Magnum"),
    "desert_tactical_srs_stealth_recon_scout": (8.58, ".338 Lapua Magnum"),
}

# ── Read existing TSV ──
with open(TSV_PATH) as f:
    reader = csv.DictReader(f, delimiter="\t")
    rows = list(reader)
    fieldnames = reader.fieldnames

# Build dict by weapon_id
existing = {r["weapon_id"]: r for r in rows}

# ── Update existing rows ──
updated_count = 0
for wid, (barrel, twist, tw_dir, _) in WEAPON_BARRELS.items():
    if wid not in existing:
        print(f"WARN: {wid} not found in TSV, skipping", file=sys.stderr)
        continue
    r = existing[wid]
    r["barrel_mm"] = str(barrel)
    r["twist_mm"] = str(twist)
    r["twist_dir"] = str(tw_dir)
    r["confidence"] = "researched"

    # Apply caliber overrides
    cal_info = CALIBER_OVERRIDES.get(wid, (float(r["caliber_mm"]), r["cartridge"]))
    cal_mm, cartridge = cal_info
    r["caliber_mm"] = str(cal_mm)
    r["cartridge"] = cartridge

    # Apply cartridge defaults
    cd_key = (cal_mm, cartridge)
    if cd_key in CARTRIDGE_DEFAULTS:
        press, mass, vel, src = CARTRIDGE_DEFAULTS[cd_key]
        r["pressure_mpa"] = str(press)
        r["projectile_mass_g"] = str(mass)
        r["muzzle_velocity_ms"] = str(vel)
        src_field = f"researched ({src})"
        r["source"] = src_field

    updated_count += 1

# ── Add variant rows ──
new_rows = []
for new_wid, base_wid, barrel, twist, tw_dir, desc in ADDITIONAL_VARIANTS:
    if new_wid in existing:
        continue  # already exists
    if base_wid not in existing:
        print(f"WARN: base {base_wid} not found for variant {new_wid}", file=sys.stderr)
        continue
    base = existing[base_wid]
    new_row = dict(base)  # copy all fields
    new_row["weapon_id"] = new_wid
    new_row["variant"] = desc
    new_row["barrel_mm"] = str(barrel)
    new_row["twist_mm"] = str(twist)
    new_row["twist_dir"] = str(tw_dir)
    new_row["confidence"] = "researched"
    # Apply caliber overrides for the variant too
    cal_info = CALIBER_OVERRIDES.get(
        new_wid,
        CALIBER_OVERRIDES.get(base_wid, (float(base["caliber_mm"]), base["cartridge"])),
    )
    cal_mm, cartridge = cal_info
    new_row["caliber_mm"] = str(cal_mm)
    new_row["cartridge"] = cartridge
    cd_key = (cal_mm, cartridge)
    if cd_key in CARTRIDGE_DEFAULTS:
        press, mass, vel, src = CARTRIDGE_DEFAULTS[cd_key]
        new_row["pressure_mpa"] = str(press)
        new_row["projectile_mass_g"] = str(mass)
        new_row["muzzle_velocity_ms"] = str(vel)
        new_row["source"] = f"researched ({src})"
    new_rows.append(new_row)

# ── Write updated TSV ──
all_rows = rows + new_rows
with open(TSV_PATH, "w", newline="") as f:
    writer = csv.DictWriter(
        f, fieldnames=fieldnames, delimiter="\t", lineterminator="\n"
    )
    writer.writeheader()
    writer.writerows(all_rows)

print(f"Updated: {updated_count} existing rows", file=sys.stderr)
print(f"Added: {len(new_rows)} variant rows", file=sys.stderr)
print(
    f"Total: {len(all_rows)} rows ({len(all_rows) - len(rows)} net new)",
    file=sys.stderr,
)
