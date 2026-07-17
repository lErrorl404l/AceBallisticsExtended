class CfgPatches {
    class abo_bad {
        name = "Advanced Ballistics Extension - Behind-Armor Debris";
        author = "ABE Team";
        requiredVersion = 2.02;
        requiredAddons[] = {
            "cba_main",
            "cba_xeh",
            "abo_core",
            "abo_armor"
        };
        units[] = {};
        weapons[] = {};
    };
};

class CfgFunctions {
    class abe {
        class abo_bad {
            file = "z\abe\addons\abo_bad";
            class generateDebris {};
            class generateSpall {};
        };
    };
};
