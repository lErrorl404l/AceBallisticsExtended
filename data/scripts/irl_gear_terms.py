#!/usr/bin/env python3
"""
IRL Gear Term Database & snip Search Engine

Identifies real-world manufacturer and model names from Arma 3 gear classnames
and displayName text using a hybrid approach:

1. Substring matching on normalized classnames (handles compound identifiers)
2. Fuzzy token matching on displayName fields (handles natural language)

Principle: no hardcoded modder prefixes. The database only contains real-world
IRL products. The matching system automatically finds the IRL equivalent from
any mod's classname/displayName conventions.
"""

import re
from rapidfuzz import fuzz


# ═══════════════════════════════════════════════════════════════════════════════
# IRL Product Catalog
# ═══════════════════════════════════════════════════════════════════════════════
#
# Each entry: (search_term, manufacturer, model)
# - search_term: lowercase token to find as SUBSTRING in normalized classname
#   OR as token in displayName text
# - manufacturer: real-world manufacturer/company
# - model: specific product line name
#
# RULES:
# - Only real-world IRL items, never mod-specific entries
# - Each term must be >= 3 chars
# - More specific (longer) terms match before shorter ones

IRL_GEAR_TERMS = [
    # ══════════════════════════════════════════════════════════════════════════
    # Helmets
    # ══════════════════════════════════════════════════════════════════════════
    # Vanilla Arma IRL references
    ("helmetb", ("Gentex", "ECH")),
    ("helmetspecb", ("Ops-Core", "FAST SF")),
    ("helmetspec_", ("Ops-Core", "FAST")),
    ("helmeto_", ("Ops-Core", "Airframe")),
    ("helmetcrew", ("Gentex", "Crew Helmet")),
    ("helmetia_", ("CSAT (Iranian)", "IA Helmet")),
    ("pilothelmet", ("Gentex", "HGU-55")),
    # Ops-Core
    ("fast_sf", ("Ops-Core", "FAST SF")),
    ("fast mt", ("Ops-Core", "FAST MT")),
    ("fast_xp", ("Ops-Core", "FAST XP")),
    ("fast_", ("Ops-Core", "FAST")),
    ("airframe", ("Ops-Core", "Airframe")),
    ("fths", ("Ops-Core", "FTHS")),
    # Gentex MICH / ACH / ECH / PASGT
    ("mich2000", ("Gentex", "MICH 2000")),
    ("mich_2000", ("Gentex", "MICH 2000")),
    ("mich2001", ("Gentex", "MICH 2001")),
    ("mich_2001", ("Gentex", "MICH 2001")),
    ("mich2002", ("Gentex", "MICH 2002")),
    ("mich_2002", ("Gentex", "MICH 2002")),
    ("ech", ("Gentex", "ECH")),
    ("pasgt", ("Gentex", "PASGT")),
    # Team Wendy
    ("exfil", ("Team Wendy", "EXFIL")),
    ("epic_", ("Team Wendy", "EPIC")),
    ("tc_800", ("Team Wendy", "TC-800")),
    ("tc-800", ("Team Wendy", "TC-800")),
    ("tc_802", ("Team Wendy", "TC-802")),
    ("tc-802", ("Team Wendy", "TC-802")),
    # Galvion / Revision
    ("caiman", ("Galvion", "Batlskin Caiman")),
    ("batlskin", ("Galvion", "Batlskin")),
    # USMC
    ("lwh", ("USMC", "LWH")),
    # IHPS
    ("ihps", ("Gentex", "IHPS")),
    # M1 Helmet
    ("m1_helmet", ("Generic", "M1 Helmet")),
    ("helmet_m1", ("Generic", "M1 Helmet")),
    # British / Commonwealth
    ("mk6", ("British Army", "Mk 6 Helmet")),
    ("mk7", ("British Army", "Mk 7 Helmet")),
    ("mk5", ("British Army", "Mk 5 Helmet")),
    ("parahelmet", ("British Army", "Parachutist Helmet")),
    # M76 — used by both UK paras and German Fallschirmjäger
    ("m76_", ("British Army", "M76 Paratrooper Helmet")),
    # German
    ("m92_", ("German Bundeswehr", "M92 Gefechtshelm")),
    ("gefechtshelm", ("German Bundeswehr", "M92 Gefechtshelm")),
    # French
    ("spectra", ("Gallet", "F2 SPECTRA")),
    # Russian
    ("6b47", ("NPP KlASS", "6B47")),
    ("6b26", ("Armokom", "6B26")),
    ("6b27", ("Armokom", "6B27")),
    ("6b28", ("Armokom", "6B28")),
    ("6b14", ("NPP KlASS", "6B14")),
    ("zsh1_", ("NPP KlASS", "ZSh-1")),
    ("zsh7_", ("NPP KlASS", "ZSh-7")),
    ("ssh68", ("Soviet", "SSh-68")),
    ("ssh_68", ("Soviet", "SSh-68")),
    # Australian / Israeli
    ("ech_australia", ("Australian Defence Force", "ECH")),
    ("rbh303", ("Rabintex", "RBH-303")),
    ("rbh_", ("Rabintex", "RBH")),
    # Canadian
    ("cg634", ("Canadian Forces", "CG634")),
    # Avon
    ("cobra_plus", ("Avon Protection", "Cobra Plus")),
    # Hard Head Veterans
    ("hhv_", ("Hard Head Veterans", "Helmet")),
    # Bump
    ("bump_", ("Generic", "Bump Helmet")),
    # Pilot helmets
    ("hgu55", ("Gentex", "HGU-55")),
    ("hgu_55", ("Gentex", "HGU-55")),
    ("hgu84", ("Gentex", "HGU-84")),
    # Headsets
    ("comtac_iii", ("Peltor", "ComTac III")),
    ("comtac3", ("Peltor", "ComTac III")),
    ("comtac_vi", ("Peltor", "ComTac VI")),
    ("comtac6", ("Peltor", "ComTac VI")),
    ("comtac", ("Peltor", "ComTac")),
    ("sordin", ("MSA", "Sordin")),
    # Respirators
    ("m50_", ("Avon Protection", "M50")),
    ("fm50", ("Avon Protection", "FM50")),
    ("respirator", ("Generic", "Respirator")),
    # ══════════════════════════════════════════════════════════════════════════
    # Plate Carriers & snip Vests
    # ══════════════════════════════════════════════════════════════════════════
    # Vanilla Arma IRL references
    ("platecarrier1", ("Crye Precision", "JPC 1.0")),
    ("platecarrier2", ("Crye Precision", "JPC 2.0")),
    ("platecarrierspec", ("Crye Precision", "AVS")),
    ("platecarriergl", ("Crye Precision", "CPC")),
    # Crye Precision
    ("jpc_2_0", ("Crye Precision", "JPC 2.0")),
    ("jpc_2.", ("Crye Precision", "JPC 2.0")),
    ("jpc_maritime", ("Crye Precision", "JPC 2.0 Maritime")),
    ("jpc", ("Crye Precision", "JPC")),
    ("avs", ("Crye Precision", "AVS")),
    ("cpc", ("Crye Precision", "CPC")),
    ("airlite", ("Crye Precision", "Airlite SPC")),
    ("spc_", ("Crye Precision", "Airlite SPC")),
    ("lvs_", ("Crye Precision", "LVS")),
    # Ferro Concepts
    ("slickster", ("Ferro Concepts", "Slickster")),
    ("fcpc", ("Ferro Concepts", "FCPC V5")),
    # Spiritus Systems
    ("lv119", ("Spiritus Systems", "LV-119")),
    ("lv_119", ("Spiritus Systems", "LV-119")),
    ("lv120", ("Spiritus Systems", "LV-120")),
    ("lv_120", ("Spiritus Systems", "LV-120")),
    # Velocity Systems
    ("scarab", ("Velocity Systems", "SCARAB LT")),
    # Agilite
    ("k19", ("Agilite", "K19")),
    ("kzero", ("Agilite", "K-Zero")),
    ("k_zero", ("Agilite", "K-Zero")),
    # Shaw Concepts
    ("arc_v2", ("Shaw Concepts", "ARC V2")),
    ("arcv2", ("Shaw Concepts", "ARC V2")),
    # Defense Mechanisms
    ("mepc", ("Defense Mechanisms", "MEPC")),
    # First Spear
    ("strandhogg", ("First Spear", "Strandhögg")),
    ("siege_r", ("First Spear", "Siege-R")),
    # TYR Tactical
    ("pico", ("TYR Tactical", "PICO")),
    # Blue Force Gear
    ("plateminus", ("Blue Force Gear", "PLATEminus 6")),
    # Haley Strategic
    ("thorax", ("Haley Strategic", "Thorax")),
    # 5.11 Tactical
    ("tactec", ("5.11 Tactical", "TacTec")),
    # London Bridge Trading
    ("6094", ("London Bridge Trading", "LBT-6094")),
    ("lbt6094", ("London Bridge Trading", "LBT-6094")),
    ("marciras", ("LBT", "MAR-CIRAS")),
    ("ciras", ("LBT", "CIRAS")),
    # Eagle Industries
    ("mkv_", ("Eagle Industries", "MKV")),
    ("mkm_", ("Eagle Industries", "MKM")),
    ("mmac", ("Eagle Industries", "MMAC")),
    ("mbav", ("Eagle Industries", "MBAV")),
    # US Army issue
    ("iotv", ("US Army", "IOTV")),
    ("spcs", ("US Army", "SPCS")),
    ("msv_", ("US Army", "MSV")),
    ("otv", ("US Army", "OTV")),
    # Russian
    ("smersh", ("Russian MoD", "SMERSH")),
    # LBE / ALICE
    ("lbe_", ("USGI", "LBE")),
    ("lbv_", ("USGI", "LBV")),
    ("alice", ("USGI", "ALICE")),
    # Safariland
    ("safariland", ("Safariland", "Duty Gear")),
    # Rebreather
    ("rebreather", ("Draeger", "Rebreather")),
    # ══════════════════════════════════════════════════════════════════════════
    # Combat Uniforms
    # ══════════════════════════════════════════════════════════════════════════
    # Vanilla Arma
    ("combatuniform", ("Crye Precision", "G3 Combat Uniform")),
    # Crye Precision
    ("crye_g3", ("Crye Precision", "G3 Combat Uniform")),
    ("cryeg3", ("Crye Precision", "G3")),
    ("g3_", ("Crye Precision", "G3")),
    ("g4_", ("Crye Precision", "G4")),
    # UF Pro
    ("ufpro", ("UF Pro", "Striker XT")),
    ("uf_pro", ("UF Pro", "Striker XT")),
    ("striker_xt", ("UF Pro", "Striker XT")),
    ("striker_ht", ("UF Pro", "Striker HT")),
    # Patagonia
    ("pcu_level", ("Patagonia", "PCU Level 5")),
    ("pcu_", ("Patagonia", "PCU")),
    # Arc'teryx
    ("arcteryx", ("Arc'teryx", "LEAF")),
    ("leaf_", ("Arc'teryx", "LEAF")),
    # British
    ("denison", ("British Army", "Denison Smock")),
    ("uniform_dpm", ("British Army", "DPM Combat Uniform")),
    ("uniform_ddpm", ("British Army", "DPM Combat Uniform")),
    ("uniform_mtp", ("British Army", "MTP Combat Uniform")),
    ("uniform_ddcu", ("US Army", "DCU Combat Uniform")),
    # US Army
    ("og107", ("US Army", "OG-107 Utility Uniform")),
    ("acu_", ("US Army", "ACU")),
    ("dcu_", ("US Army", "DCU")),
    ("mccuu", ("USMC", "MCCUU")),
    ("nwu_", ("US Navy", "NWU")),
    # Russian
    ("vkbo", ("Russian MoD", "VKBO")),
    ("ratnik", ("Russian MoD", "Ratnik")),
    ("voin", ("Russian MoD", "Voin")),
    # Fragmentation suit
    ("fragsuit", ("Generic", "Fragmentation Suit")),
    ("frag_suit", ("Generic", "Fragmentation Suit")),
    # Ghillie suit
    ("ghillie", ("Generic", "Ghillie Suit")),
    ("fullghillie", ("Generic", "Ghillie Suit")),
    # Flight suit
    ("flight_suit", ("Generic", "Flight Suit")),
    # ══════════════════════════════════════════════════════════════════════════
    # Glasses / Facewear
    # ══════════════════════════════════════════════════════════════════════════
    # Oakley
    ("oakley", ("Oakley", "SI Ballistic Goggle")),
    ("si_ballistic", ("Oakley", "SI Ballistic")),
    ("si_ballistic_", ("Oakley", "SI Ballistic")),
    ("g_tactical", ("Oakley", "SI Ballistic")),
    ("m_frame", ("Oakley", "M Frame")),
    ("mframe", ("Oakley", "M Frame")),
    ("flak_jak", ("Oakley", "Flak Jak")),
    ("flakjak", ("Oakley", "Flak Jak")),
    # Revision
    ("revision", ("Revision Military", "Bullet Ant")),
    ("g_combat", ("Revision Military", "Bullet Ant")),
    ("bullet_ant", ("Revision Military", "Bullet Ant")),
    ("sawfly", ("Revision Military", "Sawfly")),
    ("desert_locust", ("Revision Military", "Desert Locust")),
    # ESS
    ("crossbow_", ("ESS", "Crossbow")),
    ("suppressor_", ("ESS", "Suppressor")),
    ("ice_nbc", ("ESS", "Ice NBC")),
    ("ess_", ("ESS", "Eyewear")),
    # Wiley X
    ("wileyx", ("Wiley X", "SG-1")),
    ("wiley_x", ("Wiley X", "SG-1")),
    # Pencott
    ("pencott", ("UF Pro", "Pencott Pattern")),
    # Balaclava / Bandanna
    ("balaclava", ("Generic", "Balaclava")),
    ("bandanna", ("Generic", "Bandanna")),
    ("bandana", ("Generic", "Bandanna")),
    # Headwear (non-helmet, non-armor)
    ("booniehat", ("Generic", "Boonie Hat")),
    ("boonie_hat", ("Generic", "Boonie Hat")),
    ("mil_cap", ("Generic", "Military Cap")),
    ("watch_cap", ("Generic", "Watch Cap")),
    ("shema", ("Generic", "Shemagh")),
    ("turban", ("Generic", "Turban")),
    ("straw_hat", ("Generic", "Straw Hat")),
    # Diving
    ("diving", ("Generic", "Diving Mask")),
    # Aviator
    ("aviator", ("Generic", "Aviator Sunglasses")),
    # Misc props
    ("cigarette", ("Generic", "Cigarette")),
    ("cigar", ("Generic", "Cigar")),
    # Beret
    ("beret", ("Generic", "Beret")),
]

# Known mod/prefix words to strip during normalization.
MOD_PREFIXES = {
    "arifle",
    "srifle",
    "hgun",
    "smg",
    "lmg",
    "mmg",
    "sgun",
    "launch",
    "pdw",
    "dmr",
    "hmg",
    "gmg",
    "mortar",
    "u_",
    "v_",
    "h_",
    "g_",
    "b_",
    "o_",
    "i_",
    "c_",
    "bg_",
    "ig_",
    "blu",
    "opf",
    "ind",
    "civ",
    "rhs",
    "cup",
    "gm",
    "vn",
    "spe",
    "ws",
    "rf",
    "3cb",
    "uk3cb",
    "usm",
    "cwr",
    "cwr3",
    "age",
    "usp",
    "zulu",
    "uk",
    "usmc",
    "bw",
    "bwa",
    "pmc",
    "optre",
    "stbx",
    "sc_",
    "sc",
    "san",
    "tfw",
    "cr_",
    "cr",
    "rhs_",
    "cup_",
    "gm_",
    "vn_",
    "spe_",
    "3cb_",
    "uk3cb_",
    "age_",
    "usp_",
    "usm_",
    "cwr3_",
    "optre_",
    "stbx_",
}

# Camo / pattern / variant modifiers to strip from the tail of classnames.
CAMO_MODIFIERS = {
    # Colors
    "m81",
    "mcam",
    "multicam",
    "multicam_arid",
    "multicam_tropic",
    "multicam_black",
    "multicam_alpine",
    "ocp",
    "scorpion",
    "scorpion_w2",
    "dcu",
    "acu",
    "blk",
    "black",
    "grn",
    "green",
    "tan",
    "khk",
    "khaki",
    "od",
    "olive",
    "grey",
    "gray",
    "gry",
    "cb",
    "coyote",
    "coyote_brown",
    "rst",
    "ranger_green",
    "rg",
    "des",
    "desert",
    "arid",
    "semi",
    "semi_arid",
    "wood",
    "woodland",
    "forest",
    "snow",
    "snw",
    "wht",
    "white",
    "urban",
    "urb",
    "urbn",
    "tropic",
    "trop",
    "trp",
    "alp",
    "alpine",
    "mountain",
    "night",
    # British
    "dpm",
    "ddpm",
    "mtp",
    # USMC / USN
    "marpat",
    "marpat_wdl",
    "marpat_des",
    "cadpat",
    "cadpat_tw",
    "cadpat_ar",
    "aor1",
    "aor2",
    "aor_1",
    "aor_2",
    "nwu",
    "nwu_i",
    "nwu_ii",
    "nwu_iii",
    # US historical
    "erdl",
    "chocolate_chip",
    "6color",
    "3color",
    "3cd",
    "tiger",
    "tigerstripe",
    "tiger_stripe",
    # European
    "lizard",
    "cce",
    "daguet",
    "brush",
    "brushstroke",
    # German
    "flecktarn",
    "fleck",
    "tropentarn",
    "blumentarn",
    "strichtarn",
    "strich",
    # Nordic
    "m90",
    "m90k",
    "m05",
    "m04",
    # Italian / Polish
    "vegetato",
    "wz93",
    "vz95",
    # Russian
    "flora",
    "klmk",
    "ttsko",
    "vsr",
    "butan",
    "emr",
    "berezka",
    "partizan",
    # Commercial
    "kryptek",
    "atacs",
    "surpat",
    "pencott",
    "concamo",
    "phantomleaf",
    # Variants
    "camo",
    "nano",
    "nanocamo",
    "ocamo",
    "un",
    "unb",
    "un_st",
    "un_sts",
    "un_t",
    "un_ts",
    "un_s",
    "rolled",
    "gloves",
    "tucked",
    "sleeve",
    "sleeves",
    "shortsleeve",
    "longsleeve",
    "tee",
    "weathered",
    "net",
    "scrim",
    "burlap",
    "goggles",
    "rhino",
    "khaki",
    "mitchell",
    "rdf",
    "og107",
    "og_107",
    "cover",
    "naked",
    "bare",
    "unarmed",
}


def normalize(classname: str) -> str:
    """Strip mod prefixes and type categories from a classname.

    Returns the part of the classname most likely to contain the IRL term.
    """
    s = classname.lower()

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
    ]:
        if s.startswith(prefix):
            s = s[len(prefix) :]
            break

    for prefix in [
        "u_b_",
        "u_o_",
        "u_i_",
        "u_c_",
        "u_bg_",
        "u_ig_",
        "v_b_",
        "v_o_",
        "v_i_",
        "v_c_",
        "h_b_",
        "h_o_",
        "h_i_",
        "h_c_",
        "g_b_",
        "g_o_",
        "g_i_",
        "g_c_",
        "b_",
        "o_",
        "i_",
        "c_",
    ]:
        if s.startswith(prefix):
            s = s[len(prefix) :]
            break

    parts = s.split("_")
    while len(parts) > 1 and (
        parts[0] in MOD_PREFIXES or f"{parts[0]}_" in MOD_PREFIXES
    ):
        parts = parts[1:]
    s = "_".join(parts)

    # ponytail: helmet_/uniform_/vest_/headgear_ prefixes NOT stripped here
    # because they're part of IRL_GEAR_TERMS entries (helmeto_ → Airframe,
    # uniform_dpm → DPM Combat Uniform). Stripping them breaks those matches.
    return s


# ═══════════════════════════════════════════════════════════════════════════════
# Camouflage Pattern Database
# ═══════════════════════════════════════════════════════════════════════════════
#
# Maps detection keywords -> (pattern_name, family, description)
# Order matters: more specific patterns before generic ones.

CAMO_DB = [
    # US Woodland
    ("m81_woodland", "M81 Woodland", "Woodland", "4-color temperate woodland"),
    ("m81", "M81 Woodland", "Woodland", "4-color temperate woodland"),
    ("woodland", "Woodland", "Woodland", "Generic woodland pattern"),
    ("erdl", "ERDL", "Woodland", "US M1948 leaf pattern"),
    # US Desert
    ("chocolate_chip", "6-Color Desert", "Desert", "Chocolate chip pattern"),
    ("6color", "6-Color Desert", "Desert", "Six-color desert pattern"),
    ("3color", "3-Color Desert", "Desert", "Three-color desert pattern"),
    ("dcu", "DCU", "Desert", "Desert Combat Uniform"),
    # MultiCam / OCP
    ("multicam_arid", "MultiCam Arid", "MultiCam", "Arid environment variant"),
    ("multicam_tropic", "MultiCam Tropic", "MultiCam", "Tropical environment variant"),
    ("multicam_black", "MultiCam Black", "MultiCam", "Black/dark variant"),
    ("multicam_alpine", "MultiCam Alpine", "MultiCam", "Snow/alpine variant"),
    ("multicam", "MultiCam", "MultiCam", "Multi-environment pattern"),
    ("mcam", "MultiCam", "MultiCam", "Multi-environment pattern"),
    ("ocp", "OCP (Scorpion W2)", "MultiCam", "Operational Camouflage Pattern"),
    ("scorpion_w2", "OCP (Scorpion W2)", "MultiCam", "Operational Camouflage Pattern"),
    ("scorpion", "OCP (Scorpion W2)", "MultiCam", "Operational Camouflage Pattern"),
    # UCP
    ("ucp", "UCP", "UCP", "Universal Camouflage Pattern"),
    ("universal", "UCP", "UCP", "Universal Camouflage Pattern"),
    # ACU
    ("acu", "ACU", "Solid", "Army Combat Uniform"),
    # British
    ("ddpm", "Desert DPM", "DPM", "Desert DPM variant"),
    ("dpm", "DPM", "DPM", "Disruptive Pattern Material"),
    ("mtp", "MTP", "DPM", "Multi-Terrain Pattern"),
    # German
    ("tropentarn", "Tropentarn", "Flecktarn", "Tropical Flecktarn"),
    ("flecktarn", "Flecktarn", "Flecktarn", "German spotted pattern"),
    ("fleck", "Flecktarn", "Flecktarn", "German spotted pattern"),
    # East German
    ("blumentarn", "Blumentarn", "Flecktarn", "East German flower pattern"),
    ("strichtarn", "Strichtarn", "Strichtarn", "East German rain pattern"),
    ("raindrop", "Strichtarn", "Strichtarn", "East German rain pattern"),
    # French
    ("cce", "CCE", "Lizard", "French Camouflage Centre Europe"),
    ("daguet", "Daguet", "Lizard", "French Daguet desert"),
    ("lizard", "Lizard", "Lizard", "French lizard pattern"),
    # USMC
    ("marpat_wdl", "MARPAT Woodland", "MARPAT", "USMC woodland digital"),
    ("marpat_des", "MARPAT Desert", "MARPAT", "USMC desert digital"),
    ("marpat", "MARPAT", "MARPAT", "Marine Pattern"),
    # CADPAT
    ("cadpat_tw", "CADPAT TW", "CADPAT", "Canadian temperate woodland"),
    ("cadpat_ar", "CADPAT AR", "CADPAT", "Canadian arid region"),
    ("cadpat", "CADPAT", "CADPAT", "Canadian Disruptive Pattern"),
    # US Navy
    ("aor1", "AOR-1", "AOR", "US Navy desert digital"),
    ("aor2", "AOR-2", "AOR", "US Navy woodland digital"),
    ("nwu_iii", "NWU Type III", "NWU", "Navy Working Uniform - blue"),
    ("nwu", "NWU", "NWU", "Navy Working Uniform"),
    # Tigerstripe
    ("tigerstripe", "Tigerstripe", "Tigerstripe", "Vietnam-era pattern"),
    ("tiger_stripe", "Tigerstripe", "Tigerstripe", "Vietnam-era pattern"),
    # Brushstroke
    ("brushstroke", "Brushstroke", "Brushstroke", "WW2-era British pattern"),
    ("brush", "Brushstroke", "Brushstroke", "WW2-era brush pattern"),
    # Russian / Soviet
    ("flora", "Flora", "Russian", "Russian woodland pattern"),
    ("klmk", "KLMK", "Russian", "Soviet camouflage suit"),
    ("ttsko", "TTsKO", "Russian", "Soviet 3-color pattern"),
    ("vsr", "VSR", "Russian", "Russian vertical pattern"),
    ("butan", "Butan", "Russian", "Russian urban pattern"),
    ("emr", "EMR", "Russian", "Russian digital pattern"),
    ("berezka", "Berezka", "Russian", "Soviet birch pattern"),
    ("partizan", "Partizan", "Russian", "Russian leaf pattern"),
    # Nordic
    ("m90", "M90", "Nordic", "Swedish splinter pattern"),
    ("m90k", "M90K", "Nordic", "Swedish desert variant"),
    ("m05", "M05", "Nordic", "Finnish digital pattern"),
    ("m04", "M04", "Nordic", "Finnish leaf pattern"),
    # Italian
    ("vegetato", "Vegetato", "Vegetato", "Italian digital pattern"),
    ("vegetata", "Vegetato", "Vegetato", "Italian digital pattern"),
    # Polish / Czech
    ("wz93", "wz. 93", "Polish", "Polish pantera pattern"),
    ("vz95", "vz. 95", "Czech", "Czech leaf pattern"),
    # Austrian
    ("m57", "M57", "Austrian", "Austrian pea pattern"),
    # Swiss
    ("m70", "M70", "Swiss", "Swiss splinter pattern"),
    # South African
    ("nutria", "Nutria", "SADF", "South African brown pattern"),
    ("soldier_2000", "Soldier 2000", "SADF", "South African pattern"),
    # Commercial
    ("kryptek", "Kryptek", "Kryptek", "Commercial tech pattern"),
    ("atacs", "A-TACS", "A-TACS", "Advanced Tactical Concealment System"),
    ("surpat", "SurPat", "SurPat", "Multicam-inspired pattern"),
    ("pencott", "Pencott", "Pencott", "UF Pro commercial pattern"),
    ("concamo", "Concamo", "Concamo", "German commercial pattern"),
    ("phantomleaf", "Phantomleaf", "Phantomleaf", "Commercial leaf pattern"),
    # Historical
    ("splitter", "Splittertarn", "Splinter", "WW2 German splinter"),
    ("splinter", "Splittertarn", "Splinter", "WW2 German splinter"),
    ("erbsen", "Erbsenmuster", "Flecktarn", "WW2 German pea pattern"),
    ("duck_hunter", "Duck Hunter", "Duck Hunter", "US M1942 frog skin"),
    # Chinese
    ("type07", "Type 07", "Chinese", "PLA digital pattern"),
    ("type99", "Type 99", "Chinese", "PLA woodland pattern"),
    # Australian
    ("auscam", "Auscam", "Auscam", "Australian pattern"),
    ("dpcu", "DPCU", "Auscam", "Australian desert pattern"),
    # Basic colors (generic)
    ("ranger_green", "Ranger Green", "Solid", "Ranger green"),
    ("coyote_brown", "Coyote Brown", "Solid", "Coyote brown"),
    ("coyote", "Coyote Brown", "Solid", "Coyote brown"),
    ("olive_drab", "Olive Drab", "Solid", "Olive drab"),
    ("khaki", "Khaki", "Solid", "Khaki"),
    ("white", "White", "Solid", "White"),
    ("black", "Black", "Solid", "Black"),
]

# Short color codes (checked separately to avoid noise)
CAMO_SHORT = {
    "blk": ("Black", "Solid"),
    "grn": ("Green", "Solid"),
    "khk": ("Khaki", "Solid"),
    "wht": ("White", "Solid"),
    "gry": ("Grey", "Solid"),
    "od": ("Olive Drab", "Solid"),
    "rg": ("Ranger Green", "Solid"),
    "rgr": ("Ranger Green", "Solid"),
    "cb": ("Coyote Brown", "Solid"),
    "snw": ("Snow", "Solid"),
    "wdl": ("Woodland", "Woodland"),
    "des": ("Desert", "Desert"),
    "urb": ("Urban", "Solid"),
    "trp": ("Tropical", "Tropical"),
    "alp": ("Alpine", "Snow"),
}


def detect_camo(classname: str, displayname: str = "") -> tuple[str, str]:
    """Detect camouflage pattern from classname and displayName.

    Returns (pattern_name, family) or ("", "") if no pattern found.
    """
    needle = f"{classname.lower()} {displayname.lower()}".replace("-", " ").replace(
        "_", " "
    )

    # Check full names first (longer = more specific)
    best_name = ""
    best_family = ""
    best_len = 0
    for keyword, name, family, _ in CAMO_DB:
        kw = keyword.replace("-", " ").replace("_", " ")
        if kw in needle and len(kw) > best_len:
            best_name = name
            best_family = family
            best_len = len(kw)

    if best_len >= 3:
        return (best_name, best_family)

    # Check short codes on token boundaries
    for part in needle.split():
        if part in CAMO_SHORT:
            name, family = CAMO_SHORT[part]
            if len(name) > len(best_name):
                best_name = name
                best_family = family

    return (best_name, best_family) if best_name else ("", "")


def extract_irl_info(
    classname: str, displayname: str = ""
) -> tuple[str, str, float, str]:
    """Identify IRL manufacturer, model, and camo pattern from gear classname/displayName.

    Hybrid approach:
    1. Substring matching on displayName
    2. Substring matching on normalized classname
    3. Fuzzy token matching on displayName via RapidFuzz (fallback)
    4. Camouflage pattern detection

    Returns (manufacturer, model, confidence, camo_pattern).
    camo_pattern is "" if unrecognized.
    """
    normalized = normalize(classname)
    lower_norm = normalized.lower()
    lower_display = (
        displayname.lower()
        .replace(" ", "_")
        .replace(",", "")
        .replace("(", "")
        .replace(")", "")
        if displayname
        else ""
    )

    def _find_best(source: str) -> tuple[str, tuple, int]:
        """Find longest matching IRL term in source string."""
        best_term = ""
        best_match = ("", "")
        best_len = 0
        for term, (mfr, model) in IRL_GEAR_TERMS:
            if term in source and len(term) > best_len:
                best_term = term
                best_match = (mfr, model)
                best_len = len(term)
        return best_term, best_match, best_len

    camo = detect_camo(classname, displayname)
    camo_name = camo[0]

    # Pass 1: displayName (most explicit — modders write real names here)
    _, best_match, best_len = _find_best(lower_display)
    if best_len >= 3:
        conf = min(0.95, 0.50 + best_len * 0.03)
        return (best_match[0], best_match[1], conf, camo_name)

    # Pass 2: normalized classname
    _, best_match, best_len = _find_best(lower_norm)
    if best_len >= 3:
        conf = min(0.95, 0.50 + best_len * 0.03)
        return (best_match[0], best_match[1], conf, camo_name)

    # Pass 3: fuzzy fallback on displayName tokens
    if displayname:
        GEAR_STOP = {
            "helmet",
            "helmets",
            "vest",
            "vests",
            "uniform",
            "uniforms",
            "glasses",
            "headset",
            "headsets",
            "headwear",
            "facewear",
            "carrier",
            "carriers",
            "rig",
            "pack",
            "bag",
            "bags",
            "plate",
            "plates",
            "armor",
            "armour",
            "ballistic",
            "combat",
            "tactical",
            "spec",
            "specs",
            "gear",
            "equipment",
            "system",
            "systems",
            "goggle",
            "goggles",
            "eyewear",
            "protection",
            "protective",
            "military",
            "style",
            "replica",
            "digital",
            "pattern",
            "cover",
            "suit",
            "suits",
            "coveralls",
            "fatigues",
            "smock",
        }
        display_tokens = {
            t
            for t in displayname.lower().replace("-", " ").replace("_", " ").split()
            if len(t) >= 3 and t not in GEAR_STOP
        }
        best_fuzzy_score = 0.0
        best_fuzzy = ("", "")
        for term, (mfr, model) in IRL_GEAR_TERMS:
            if not model:
                continue
            term_tokens = {
                t
                for t in term.replace("-", " ").replace("_", " ").split()
                if len(t) >= 3 and t not in GEAR_STOP
            }
            if not term_tokens:
                continue
            score = (
                fuzz.token_set_ratio(
                    " ".join(sorted(display_tokens)),
                    " ".join(sorted(term_tokens)),
                )
                / 100.0
            )
            if score > best_fuzzy_score:
                best_fuzzy_score = score
                best_fuzzy = (mfr, model)
        if best_fuzzy_score >= 0.70:
            return (best_fuzzy[0], best_fuzzy[1], best_fuzzy_score, camo_name)

    # Fallback: clean description from classname
    parts = normalized.split("_")
    while len(parts) > 1 and parts[-1] in CAMO_MODIFIERS:
        parts = parts[:-1]
    stem = "_".join(parts) if parts else normalized
    if len(stem) > 2:
        desc = stem.replace("_", " ").strip().title()
        if len(desc) > 50:
            desc = desc[:50]
        return ("Generic", desc, 0.30, camo_name)
    return ("Generic", stem, 0.20, camo_name)


def test():
    """Verify known classnames produce expected IRL info."""
    test_cases = [
        # (classname, displayName, expected_manufacturer, expected_model)
        # ── Vanilla Arma plate carriers ──
        ("V_PlateCarrier1_rgr", "Plate Carrier (RGR)", "Crye Precision", "JPC 1.0"),
        ("V_PlateCarrier2_blk", "Plate Carrier 2 (BLK)", "Crye Precision", "JPC 2.0"),
        ("V_PlateCarrierSpec_mtp", "Plate Carrier Spec (MTP)", "Crye Precision", "AVS"),
        ("V_PlateCarrierGL_mtp", "Plate Carrier GL (MTP)", "Crye Precision", "CPC"),
        # ── Vanilla Arma helmets ──
        ("H_HelmetB", "Helmet (ECH)", "Gentex", "ECH"),
        ("H_HelmetSpecB_blk", "Helmet Spec (BLK)", "Ops-Core", "FAST SF"),
        ("H_HelmetO_ocamo", "Helmet (OCAMO)", "Ops-Core", "Airframe"),
        ("H_HelmetCrew_B", "Helmet Crew (B)", "Gentex", "Crew Helmet"),
        ("H_PilotHelmetFighter_F", "Pilot Helmet (F)", "Gentex", "HGU-55"),
        # ── Vanilla Arma uniforms ──
        (
            "U_B_CombatUniform_mcam",
            "Combat Uniform (MCAM)",
            "Crye Precision",
            "G3 Combat Uniform",
        ),
        # ── Vanilla Arma glasses ──
        ("G_Balaclava_blk", "Balaclava (BLK)", "Generic", "Balaclava"),
        ("G_Bandanna_tan", "Bandanna (TAN)", "Generic", "Bandanna"),
        ("G_Aviator", "Aviator Glasses", "Generic", "Aviator Sunglasses"),
        ("G_Diving", "Diving Mask", "Generic", "Diving Mask"),
        # ── Mod items — IRL from displayName ──
        ("AGE_ComTacs", "ComTac III Headset", "Peltor", "ComTac III"),
        ("usm_helmet_m1_wdl", "Helmet, M1, Woodland", "Generic", "M1 Helmet"),
        ("usm_helmet_pasgt_unb", "Helmet, PASGT, UN", "Gentex", "PASGT"),
        ("usm_vest_safariland6005", "Safariland 6005", "Safariland", "Duty Gear"),
        ("ZuluCustomHelmets_Airframe", "Airframe Helmet", "Ops-Core", "Airframe"),
        ("AGE_CryeG3_MCam", "Crye G3 Combat Uniform (MC)", "Crye Precision", "G3"),
        ("AGE_VKBO_Flora", "VKBO Summer Suit (Flora)", "Russian MoD", "VKBO"),
        ("CR_G_Balaclava_pink", "Balaclava (Pink)", "Generic", "Balaclava"),
        # ── Mod items — CWR3 ──
        (
            "cwr3_b_uk_uniform_dpm",
            "DPM Combat Uniform",
            "British Army",
            "DPM Combat Uniform",
        ),
        (
            "cwr3_b_uk_headgear_mk6_helmet_dpm",
            "Mk 6 Helmet (DPM)",
            "British Army",
            "Mk 6 Helmet",
        ),
        # ── Mod items — classname IRL match ──
        (
            "some_mod_jpc_multicam",
            "Crye JPC 2.0 (Multicam)",
            "Crye Precision",
            "JPC 2.0",
        ),
        ("mod_avs_vest", "Crye AVS Plate Carrier", "Crye Precision", "AVS"),
        ("mod_mich_helmet", "MICH 2000 Helmet", "Gentex", "MICH 2000"),
        ("mod_k19_vest", "Agilite K19 Plate Carrier", "Agilite", "K19"),
        ("mod_comtac", "Peltor ComTac III Headset", "Peltor", "ComTac III"),
        ("mod_sordin", "MSA Sordin Headset", "MSA", "Sordin"),
        ("mod_pasgt_un", "PASGT Helmet (UN)", "Gentex", "PASGT"),
        ("mod_sm_vest", "SMERSH Vest", "Russian MoD", "SMERSH"),
        ("mod_6b47", "6B47 Helmet (Flora)", "NPP KlASS", "6B47"),
        ("mod_exfil", "Team Wendy EXFIL Helmet", "Team Wendy", "EXFIL"),
        # ── Edge cases ──
        ("OPTRE_Glasses_Cigarette", "Cigarette", "Generic", "Cigarette"),
        ("U_Rangemaster", "Rangemaster Uniform", "Generic", "Rangemaster"),
        ("U_B_Wetsuit", "Wetsuit", "Generic", "Wetsuit"),
        ("U_B_GhillieSuit", "Ghillie Suit", "Generic", "Ghillie Suit"),
        ("G_Combat_Goggles_tna_F", "Combat Goggles (TNA)", "Generic", "Combat Goggles"),
        (
            "cwr3_b_uk_headgear_m76_olive",
            "M76 Helmet (Olive)",
            "British Army",
            "M76 Paratrooper Helmet",
        ),
        ("mod_lv119", "Spiritus LV-119 (MC)", "Spiritus Systems", "LV-119"),
        ("mod_oakley", "Oakley M Frame Glasses", "Oakley", "M Frame"),
        ("mod_fast_helmet", "Ops-Core FAST Helmet", "Ops-Core", "FAST"),
    ]

    passed = 0
    failed = 0
    for cn, dn, exp_mfr, exp_model in test_cases:
        mfr, model, conf, camo = extract_irl_info(cn, dn)
        ok = mfr.lower() == exp_mfr.lower() and (
            model.lower().startswith(exp_model.lower())
            or exp_model.lower() in model.lower()
        )
        if ok:
            passed += 1
        else:
            failed += 1
            print(f"FAIL: {cn} / {dn}")
            print(f"  Expected: ({exp_mfr}, {exp_model})")
            print(f"  Got:      ({mfr}, {model}) [conf={conf:.2f}] [camo={camo}]")

    total = passed + failed
    print(f"\n{passed}/{total} passed, {failed} failed")
    return failed == 0


if __name__ == "__main__":
    test()
