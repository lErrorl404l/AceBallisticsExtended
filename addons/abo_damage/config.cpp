class CfgPatches {
    class abo_damage {
        name = "Advanced Ballistics Extension - Damage Model";
        author = "ABE Team";
        requiredVersion = 2.02;
        requiredAddons[] = {
            "cba_main",
            "cba_xeh",
            "abo_core",
            "abo_terminal",
            "abo_bad"
        };
        units[] = {};
        weapons[] = {};
    };
};

class CfgFunctions {
    class abe {
        class abo_damage {
            file = "z\abe\addons\abo_damage";
            class applyDamage {};
            class handleChannel {};
        };
    };
};
