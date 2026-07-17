class CfgPatches {
    class abo_armor {
        name = "Advanced Ballistics Extension - Armor";
        author = "ABE Team";
        requiredVersion = 2.02;
        requiredAddons[] = {
            "cba_main",
            "cba_xeh",
            "abo_core",
            "abo_penetration"
        };
        units[] = {};
        weapons[] = {};
    };
};

class CfgFunctions {
    class abe {
        class abo_armor {
            file = "z\abe\addons\abo_armor";
            class hitMaterial {};
            class resolveArray {};
        };
    };
};
