class CfgPatches {
    class abo_ricochet {
        name = "Advanced Ballistics Extension - Ricochet";
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
        class abo_ricochet {
            file = "z\abe\addons\abo_ricochet";
            class calculateRicochet {};
            class bounce {};
            class tumble {};
        };
    };
};
