class CfgPatches {
    class abo_core {
        name = "Advanced Ballistics Extension - Core";
        author = "ABE Team";
        requiredVersion = 2.0;
        requiredAddons[] = {
            "cba_main",
            "cba_xeh"
        };
        units[] = {};
        weapons[] = {};
    };
};

class CfgFunctions {
    class abe {
        class core {
            file = "z\abe\addons\abo_core";
            class init {};
            class fire {};
            class step {};
            class impact {};
            class health {};
        };
    };
};

class CfgWeapons {
    // Extended weapon configuration slots for ABE
    class Rifle_Base_F;
    class Rifle_Long_Base_F;
    class Pistol_Base_F;

    class abe_weapon_base {
        ABO_barrelLength = -1;      // mm, negative = auto-detect from model
        ABO_chamberPressure = -1;   // MPa
        ABO_riflingTwist = -1;      // mm
        ABO_cdmId = "";             // Drag model ID
        ABO_projectileMass = -1;    // grams
        ABO_zeroRange = 100;        // meters
    };
};

class CfgAmmo {
    class BulletBase;

    class abe_ammo_base {
        ABO_cdmId = "g7";           // Default drag model
        ABO_projectileMass = -1;    // grams (auto from caliber if -1)
        ABO_bcG1 = -1;
        ABO_bcG7 = -1;
        ABO_fragThreshold = -1;     // m/s, -1 = no fragmentation
        ABO_fragCount = 0;
        ABO_fragMassDist = "log_normal";
        ABO_caliberOverride = -1;   // mm
    };
};
