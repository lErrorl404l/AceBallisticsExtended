#!/usr/bin/env python3
"""
IRL Weapon Product Catalog & Search Engine.

Analogous to irl_gear_terms.py for clothing — provides a product database
of real-world firearms with manufacturer, model, and search keywords.
Used to:
  1. Validate that `resolve_weapon()` correctly matches classnames
  2. Enrich matching with displayName-based fuzzy fallback
  3. Identify gaps in the IRL weapons database
"""

# ── Product database ──────────────────────────────────────────────────────
# Each entry: (manufacturer, model, weapon_type, caliber_mm, keywords)
# keywords are used for substring matching against classnames.
# The first keyword MUST be the ir_weapons.tsv key.

WEAPON_CATALOG = [
    # ── Rifles ──
    ("Colt", "M4A1", "arifle", 5.56, ["m4a1", "m4", "colt_m4"]),
    (
        "Colt",
        "M4A1 Block II",
        "arifle",
        5.56,
        ["m4a1_blockii", "m4a1blockii", "blockii"],
    ),
    ("Colt", "M16A4", "arifle", 5.56, ["m16a4", "m16"]),
    ("Heckler & Koch", "HK416", "arifle", 5.56, ["hk416"]),
    ("Heckler & Koch", "HK416A5", "arifle", 5.56, ["hk416a5", "hk416a"]),
    ("Heckler & Koch", "HK417", "srifle", 7.62, ["hk417"]),
    ("Heckler & Koch", "G36", "arifle", 5.56, ["g36"]),
    ("Heckler & Koch", "G36A2", "arifle", 5.56, ["g36a2", "g36a"]),
    ("Heckler & Koch", "G3", "srifle", 7.62, ["g3"]),
    ("Heckler & Koch", "HK33", "arifle", 5.56, ["hk33"]),
    ("Heckler & Koch", "HK53", "arifle", 5.56, ["hk53"]),
    ("FN Herstal", "SCAR-L", "arifle", 5.56, ["scar_l", "scar_light", "mk16"]),
    ("FN Herstal", "SCAR-H", "srifle", 7.62, ["scar_h", "scar_heavy", "mk17"]),
    ("FN Herstal", "FAL", "srifle", 7.62, ["fal", "fn_fal"]),
    ("FN Herstal", "F2000", "arifle", 5.56, ["f2000", "fn_f2000"]),
    ("FN Herstal", "P90", "smg", 5.7, ["p90", "fn_p90"]),
    ("Steyr", "AUG", "arifle", 5.56, ["aug", "steyr_aug"]),
    ("Steyr", "AUG A3", "arifle", 5.56, ["auga3", "aug_a3"]),
    (
        "IWI",
        "Tavor X95",
        "arifle",
        5.56,
        ["x95", "tar21", "tavor_x95", "tar_21", "mtar", "mtar21"],
    ),
    ("IWI", "Galil ACE", "arifle", 5.56, ["galil", "galil_ace", "ace_galil"]),
    ("IWI", "Negev", "lmg", 5.56, ["negev"]),
    ("IMI", "Uzi", "smg", 9.01, ["uzi", "mini_uzi", "micro_uzi", "microuzi"]),
    # ── British ──
    ("BAE Systems", "L85A2", "arifle", 5.56, ["l85", "l85a2", "l85a"]),
    ("BAE Systems", "L86A2 LSW", "lmg", 5.56, ["l86", "l86a2", "l86a"]),
    (
        "Accuracy International",
        "AWM",
        "srifle",
        8.58,
        ["awm", "l115", "l115a3", "a_i_awm"],
    ),
    ("Accuracy International", "AW", "srifle", 7.62, ["aw", "l118", "l96"]),
    ("Accuracy International", "AX50", "srifle", 12.7, ["ax50", "ax_50"]),
    ("Lewis Machine & Tool", "L129A1", "srifle", 7.62, ["l129", "l129a1", "lmt"]),
    # ── Russian ──
    ("Kalashnikov Concern", "AK-74M", "arifle", 5.45, ["ak74m", "ak74"]),
    ("Kalashnikov Concern", "AK-74", "arifle", 5.45, ["ak74"]),
    ("Kalashnikov Concern", "AK-47", "arifle", 7.62, ["ak47"]),
    ("Kalashnikov Concern", "AKM", "arifle", 7.62, ["akm"]),
    ("Kalashnikov Concern", "AK-103", "arifle", 7.62, ["ak103"]),
    ("Kalashnikov Concern", "AK-105", "arifle", 5.45, ["ak105", "ak10"]),
    ("Kalashnikov Concern", "AK-12", "arifle", 5.45, ["ak12"]),
    ("Kalashnikov Concern", "RPK-74", "lmg", 5.45, ["rpk74", "rpk"]),
    ("Kalashnikov Concern", "PKM", "lmg", 7.62, ["pkm", "pk_base", "pkp"]),
    ("Kalashnikov Concern", "PKP Pecheneg", "lmg", 7.62, ["pkp", "pecheneg"]),
    ("Tula Arms Plant", "SV-98", "srifle", 7.62, ["sv98", "sv_98"]),
    ("Degtyaryov Plant", "RPK-16", "lmg", 5.45, ["rpk16", "rpk_16"]),
    ("Izhmash", "SVD Dragunov", "srifle", 7.62, ["svd", "dragunov"]),
    ("Izhmash", "SVDS", "srifle", 7.62, ["svds"]),
    ("TsKIB SOO", "VSS Vintorez", "srifle", 9.0, ["vss", "vintorez"]),
    ("TsKIB SOO", "AS Val", "arifle", 9.0, ["asval", "as_val"]),
    ("KBP", "PP-19 Bizon", "smg", 9.01, ["bizon", "pp19", "pp_bizon"]),
    ("Zavod imeni Degtyaryova", "RPK-74M", "lmg", 5.45, ["rpk_74", "rpk_74m"]),
    # ── Pistols ──
    ("Glock", "Glock 17", "pistol", 9.01, ["glock17", "g17"]),
    ("Glock", "Glock 18", "pistol", 9.01, ["glock18", "g18"]),
    ("Glock", "Glock 19", "pistol", 9.01, ["glock19", "g19"]),
    ("Sig Sauer", "P226", "pistol", 9.01, ["p226", "sig_p226"]),
    ("Sig Sauer", "P320", "pistol", 9.01, ["p320", "sig_p320"]),
    ("Sig Sauer", "P229", "pistol", 9.01, ["p229", "sig_p229"]),
    ("Sig Sauer", "P220", "pistol", 9.01, ["p220", "sig_p220"]),
    ("Sig Sauer", "MCX", "arifle", 5.56, ["mcx", "sig_mcx"]),
    ("Heckler & Koch", "USP", "pistol", 9.01, ["usp", "hk_usp"]),
    ("Heckler & Koch", "Mk23", "pistol", 9.01, ["mk23", "hk_mk23"]),
    ("Heckler & Koch", "P7", "pistol", 9.01, ["p7", "hk_p7"]),
    ("Beretta", "M9", "pistol", 9.01, ["m9", "beretta_m9", "m9a1"]),
    ("Beretta", "M9A3", "pistol", 9.01, ["m9a3"]),
    ("FN Herstal", "FNX-45", "pistol", 11.43, ["fnx45", "fnx_45", "fnx"]),
    ("FN Herstal", "Five-seveN", "pistol", 5.7, ["57", "five_seven", "fn57", "fn_57"]),
    ("Smith & Wesson", "M&P", "pistol", 9.01, ["mnp", "mn_p"]),
    # ── SMGs ──
    ("Heckler & Koch", "MP5", "smg", 9.01, ["mp5"]),
    ("Heckler & Koch", "MP7", "smg", 4.6, ["mp7"]),
    ("Heckler & Koch", "UMP-45", "smg", 11.43, ["ump", "ump45", "ump_45"]),
    ("Heckler & Koch", "MP5K", "smg", 9.01, ["mp5k", "mp5_k"]),
    ("Heckler & Koch", "MP5SD", "smg", 9.01, ["mp5sd", "mp5_sd"]),
    ("Brügger & Thomet", "MP9", "smg", 9.01, ["mp9", "tp9"]),
    ("Brügger & Thomet", "APC9", "smg", 9.01, ["apc9", "apc_pro"]),
    ("MAGPUL", "PDR", "smg", 5.56, ["pdr", "magpul_pdr"]),
    ("RPC Fort", "Fort-221", "smg", 9.01, ["fort221", "fort_221"]),
    ("MAC", "MAC-10", "smg", 11.43, ["mac10"]),
    ("MAC", "MAC-11", "smg", 9.01, ["mac11", "mac_11"]),
    # ── Shotguns ──
    ("Remington", "M870", "sgun", 12.0, ["m870", "remington_870"]),
    ("Benelli", "M1014", "sgun", 12.0, ["m1014", "benelli_m4"]),
    ("Franchi", "SPAS-12", "sgun", 12.0, ["spas12"]),
    ("Mossberg", "M590", "sgun", 12.0, ["m590"]),
    # ── DMRs / Snipers ──
    (
        "Barrett",
        "M82 (M107)",
        "srifle",
        12.7,
        ["m82", "m107", "barrett_m82", "barrett_m107"],
    ),
    ("Barrett", "M95", "srifle", 12.7, ["m95", "barrett_m95"]),
    ("Barrett", "MRAD", "srifle", 8.58, ["mrad", "barrett_mrad"]),
    ("Remington", "M24", "srifle", 7.62, ["m24"]),
    ("Remington", "M40A5", "srifle", 7.62, ["m40a5", "m40"]),
    ("Remington", "M2010", "srifle", 7.62, ["m2010"]),
    ("Remington", "M700", "srifle", 7.62, ["m700", "remington_700", "r700"]),
    ("McMillan", "TAC-50", "srifle", 12.7, ["tac50", "tac_50"]),
    ("Steyr", "SSG 69", "srifle", 7.62, ["ssg69", "ssg_69"]),
    ("Knight's Armament", "M110", "srifle", 7.62, ["m110", "kac_m110"]),
    ("Knight's Armament", "SR-25", "srifle", 7.62, ["sr25", "kac_sr25"]),
    ("Knight's Armament", "Mk 11", "srifle", 7.62, ["mk11"]),
    ("Sako", "TRG-22", "srifle", 7.62, ["trg22", "sako_trg", "trg", "sako_trg22"]),
    ("Sako", "TRG-42", "srifle", 8.58, ["trg42", "sako_trg42"]),
    ("PGM", "Hécate II", "srifle", 12.7, ["hecate", "pgm_hecate"]),
    ("Heckler & Koch", "MSG90", "srifle", 7.62, ["msg90", "msg_90"]),
    ("Heckler & Koch", "PSG1", "srifle", 7.62, ["psg1"]),
    (
        "Desert Tactical",
        "SRS (Stealth Recon Scout)",
        "srifle",
        8.58,
        ["srs", "dt_srs", "srs_covert"],
    ),
    (
        "CheyTac",
        "M200 Intervention",
        "srifle",
        10.36,
        ["m200", "intervention", "cheytac"],
    ),
    # ── LMGs ──
    ("FN Herstal", "Minimi (M249)", "lmg", 5.56, ["m249", "minimi", "fn_minimi"]),
    ("FN Herstal", "MAG (M240)", "lmg", 7.62, ["m240", "fn_mag"]),
    ("Heckler & Koch", "MG4", "lmg", 5.56, ["mg4", "hk_mg4"]),
    ("Heckler & Koch", "MG5", "lmg", 7.62, ["mg5", "hk_mg5"]),
    ("Rheinmetall", "MG3", "lmg", 7.62, ["mg3"]),
    ("SIG", "MG 338", "lmg", 8.58, ["mg338", "sig_mg338"]),
    ("AAI Corporation", "M249 Para", "lmg", 5.56, ["m249_para", "m249para"]),
    ("Russian Federation", "NSV 'Utes'", "hmg", 12.7, ["nsv", "utes"]),
    ("Russian Federation", "KORD", "hmg", 12.7, ["kord"]),
    ("Browning", "M2HB", "hmg", 12.7, ["m2", "browning_m2", "m2hb"]),
    # ── Launchers ──
    (
        "Saab Bofors Dynamics",
        "Carl Gustaf M3",
        "launch",
        84.0,
        ["carl_gustaf", "m3_maaw", "maaw"],
    ),
    ("Saab Bofors Dynamics", "Carl Gustaf M4", "launch", 84.0, ["carl_gustaf_m4"]),
    ("RPG Concern", "RPG-7", "launch", 40.0, ["rpg7", "rpg_7"]),
    ("RPG Concern", "RPG-18", "launch", 64.0, ["rpg18", "rpg_18"]),
    ("RPG Concern", "RPG-26", "launch", 72.5, ["rpg26", "rpg_26"]),
    ("RPG Concern", "RPG-32", "launch", 105.0, ["rpg32", "rpg_32"]),
    ("RGS", "RPO-A Shmel", "launch", 93.0, ["rpo_a", "shmel"]),
    ("Tula KBP", "9K32 Strela-2", "launch", 72.0, ["strela", "9k32"]),
    ("MBDA", "FIM-92 Stinger", "launch", 70.0, ["stinger", "fim92"]),
    ("Raytheon", "FGM-148 Javelin", "launch", 127.0, ["javelin", "fgm148", "fgm_148"]),
    ("Raytheon", "M136 AT4", "launch", 84.0, ["m136", "at4"]),
    ("Nammo", "M72 LAW", "launch", 66.0, ["m72_law", "m72"]),
    ("Dynamit Nobel", "M79 Osa", "launch", 90.0, ["m79", "osa"]),
    # ── Missing mod weapons ──
    ("Springfield Armory", "M14", "srifle", 7.62, ["m14"]),
    ("Saco Defense", "M60", "lmg", 7.62, ["m60"]),
    ("Colt", "M1911A1", "pistol", 11.43, ["m1911", "colt_1911"]),
]

# ── Search engine ─────────────────────────────────────────────────────────


def normalize(classname: str) -> str:
    """Normalize an Arma weapon classname for matching."""
    s = classname.lower()

    # Strip known weapon type prefixes
    for prefix in [
        "arifle_",
        "srifle_",
        "hgun_",
        "smg_",
        "lmg_",
        "mmg_",
        "sgun_",
        "launch_",
        "pdw_",
        "dmr_",
        "hmg_",
        "gmg_",
        "mortar_",
        "weapon_",
        "weap_",
    ]:
        if s.startswith(prefix):
            s = s[len(prefix) :]
            break

    # Strip mod prefix (alphabetical segments followed by _)
    while True:
        parts = s.split("_", 1)
        if len(parts) < 2:
            break
        candidate = parts[0]
        rest = parts[1]

        # Candidate has digits → weapon model number, not mod prefix
        if any(c.isdigit() for c in candidate):
            break
        if len(candidate) < 2 or len(candidate) > 8:
            break
        if len(rest) < 3:
            break
        if not any(c.isalpha() for c in rest):
            break

        s = rest

    # Strip trailing variant codes
    for suffix in [
        "_f",
        "_test",
        "_base",
        "_blk",
        "_tan",
        "_wdl",
        "_khk",
        "_grn",
        "_des",
        "_arid",
        "_lush",
        "_tropic",
        "_snd",
        "_snds",
        "_gl",
        "_bipod",
        "_pointer",
        "_holo",
        "_rds",
        "_camo",
        "_camo2",
        "_winter",
        "_m81",
        "_multcam",
        "_ocp",
    ]:
        if s.endswith(suffix):
            s = s[: -len(suffix)]

    return s


def extract_irl_info(classname: str, displayname: str = "") -> tuple:
    """Find best IRL match for a weapon classname.

    Returns (manufacturer, model, weapon_type, caliber_mm, confidence).
    """
    norm = normalize(classname)
    lower_display = displayname.lower().replace(" ", "_").replace("-", "_")

    best = None
    best_len = 0
    best_conf = 0.0

    for mfr, model, wtype, cal, keywords in WEAPON_CATALOG:
        for idx, kw in enumerate(keywords):
            if kw in norm or kw in lower_display:
                base = 0.75 if idx == 0 else 0.55
                conf = min(0.95, base + len(kw) * 0.025)
                # Prefer higher confidence; break ties by length
                if conf > best_conf or (conf == best_conf and len(kw) > best_len):
                    best = (mfr, model, wtype, cal, conf)
                    best_len = len(kw)
                    best_conf = conf

    if best:
        return best

    # Fallback: type-based default
    wtype = "rifle"
    for prefix, t in [
        ("arifle", "arifle"),
        ("srifle", "srifle"),
        ("hgun", "pistol"),
        ("smg", "smg"),
        ("lmg", "lmg"),
        ("mmg", "lmg"),
        ("sgun", "sgun"),
        ("launch", "launch"),
        ("pdw", "smg"),
        ("dmr", "srifle"),
        ("hmg", "hmg"),
    ]:
        if classname.lower().startswith(prefix):
            wtype = t
            break

    return ("Generic", wtype.capitalize(), wtype, 7.62, 0.30)


# ── Test cases ────────────────────────────────────────────────────────────

# Test cases: (classname, displayName, expected_mfr, expected_model, min_confidence)
TEST_CASES = [
    # Base game weapons (fictional — fallback to type-based)
    ("arifle_MX_F", "MX 6.5 mm", "Generic", "Arifle", 0.30),
    ("arifle_MX_Black_F", "MX (Black) 6.5 mm", "Generic", "Arifle", 0.30),
    ("arifle_MXC_F", "MX C 6.5 mm", "Generic", "Arifle", 0.30),
    ("arifle_Katiba_F", "Katiba 6.5 mm", "Generic", "Arifle", 0.30),
    # Real weapons by name
    ("arifle_M4A1_F", "M4A1 5.56 mm", "Colt", "M4A1", 0.80),
    ("arifle_M4A1_black_F", "M4A1 (Black) 5.56 mm", "Colt", "M4A1", 0.80),
    ("arifle_M16A4_F", "M16A4 5.56 mm", "Colt", "M16A4", 0.80),
    ("arifle_M16A4_GL_F", "M16A4 (GL) 5.56 mm", "Colt", "M16A4", 0.80),
    ("arifle_AK12_F", "AK-12 5.45 mm", "Kalashnikov Concern", "AK-12", 0.80),
    ("arifle_AKM_F", "AKM 7.62 mm", "Kalashnikov Concern", "AKM", 0.80),
    ("arifle_AK74_F", "AK-74 5.45 mm", "Kalashnikov Concern", "AK-74", 0.80),
    ("arifle_AK74M_F", "AK-74M 5.45 mm", "Kalashnikov Concern", "AK-74M", 0.80),
    ("arifle_AK103_F", "AK-103 7.62 mm", "Kalashnikov Concern", "AK-103", 0.80),
    ("arifle_HK416_F", "HK416 5.56 mm", "Heckler & Koch", "HK416", 0.80),
    ("arifle_HK416A5_F", "HK416A5 5.56 mm", "Heckler & Koch", "HK416A5", 0.80),
    ("srifle_M110_F", "M110 7.62 mm", "Knight's Armament", "M110", 0.80),
    ("srifle_L85A2_F", "L85A2 5.56 mm", "BAE Systems", "L85A2", 0.82),
    ("srifle_AWM_F", "AWM .338", "Accuracy International", "AWM", 0.82),
    (
        "srifle_M200_F",
        "M200 Intervention 10.36 mm",
        "CheyTac",
        "M200 Intervention",
        0.82,
    ),
    ("srifle_M82_F", "M82 (M107) 12.7 mm", "Barrett", "M82 (M107)", 0.80),
    ("srifle_M24_F", "M24 7.62 mm", "Remington", "M24", 0.80),
    ("srifle_SVD_F", "SVD Dragunov 7.62 mm", "Izhmash", "SVD Dragunov", 0.80),
    # Pistols
    ("hgun_Pistol_Heavy_F", "4-five .45 ACP", "Generic", "Pistol", 0.30),
    ("hgun_P07_F", "P07 9 mm", "Generic", "Pistol", 0.30),
    ("hgun_Rook40_F", "Rook-40 9 mm", "Generic", "Pistol", 0.30),
    ("hgun_Glock17_F", "Glock 17 9 mm", "Glock", "Glock 17", 0.80),
    ("hgun_Glock19_F", "Glock 19 9 mm", "Glock", "Glock 19", 0.80),
    ("hgun_P226_F", "P226 9 mm", "Sig Sauer", "P226", 0.80),
    ("hgun_M9_F", "M9 9 mm", "Beretta", "M9", 0.80),
    # SMGs
    ("SMG_MP5_F", "MP5 9 mm", "Heckler & Koch", "MP5", 0.80),
    ("SMG_MP7_F", "MP7 4.6 mm", "Heckler & Koch", "MP7", 0.80),
    ("SMG_P90_F", "P90 5.7 mm", "FN Herstal", "P90", 0.80),
    # Shotguns
    ("sgun_M1014_F", "M1014 12ga", "Benelli", "M1014", 0.80),
    ("sgun_M870_F", "M870 12ga", "Remington", "M870", 0.80),
    # LMGs
    ("lmg_M249_F", "M249 5.56 mm", "FN Herstal", "Minimi (M249)", 0.80),
    ("lmg_M240_F", "M240 7.62 mm", "FN Herstal", "MAG (M240)", 0.80),
    ("lmg_MG3_F", "MG3 7.62 mm", "Rheinmetall", "MG3", 0.80),
    # Launchers
    ("launch_RPG7_F", "RPG-7 40 mm", "RPG Concern", "RPG-7", 0.80),
    ("launch_M136_F", "M136 AT-4", "Raytheon", "M136 AT4", 0.80),
    ("launch_M72_F", "M72 LAW 66 mm", "Nammo", "M72 LAW", 0.80),
    ("launch_Stinger_F", "FIM-92 Stinger", "MBDA", "FIM-92 Stinger", 0.80),
    # SCAR variants
    ("arifle_SCAR_L_F", "SCAR-L 5.56 mm", "FN Herstal", "SCAR-L", 0.80),
    ("srifle_SCAR_H_F", "SCAR-H 7.62 mm", "FN Herstal", "SCAR-H", 0.80),
    ("arifle_FAL_F", "FAL 7.62 mm", "FN Herstal", "FAL", 0.80),
    ("arifle_G36_F", "G36 5.56 mm", "Heckler & Koch", "G36", 0.80),
    ("arifle_G3_F", "G3 7.62 mm", "Heckler & Koch", "G3", 0.80),
    ("arifle_AUG_F", "AUG 5.56 mm", "Steyr", "AUG", 0.80),
    ("arifle_F2000_F", "F2000 5.56 mm", "FN Herstal", "F2000", 0.80),
]


def test():
    """Run all weapon test cases."""
    passed = 0
    failed = 0

    print(f"{'Classname':45s} {'Expected':45s} {'Got':45s} {'Conf':6s}")
    print("-" * 145)

    for cls, display, exp_mfr, exp_model, min_conf in TEST_CASES:
        mfr, model, wtype, cal, conf = extract_irl_info(cls, display)
        expected = f"{exp_mfr} {exp_model}"
        got = f"{mfr} {model}"

        ok = conf >= min_conf
        if expected == got and ok:
            passed += 1
            status = "✓"
        else:
            failed += 1
            status = "✗"

        print(f"{cls:45s} {expected:45s} {got:45s} {conf:.2f} {status}")

    print(f"\n{'=' * 60}")
    print(f"Results: {passed}/{passed + failed} passed, {failed} failed")
    return failed == 0


if __name__ == "__main__":
    test()
