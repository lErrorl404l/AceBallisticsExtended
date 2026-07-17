class CfgPatches {
    class abo_degradation {
        name = "Advanced Ballistics Extension - Degradation";
        author = "ABE Team";
        requiredVersion = 2.02;
        requiredAddons[] = {
            "cba_main",
            "cba_xeh",
            "abo_core"
        };
        units[] = {};
        weapons[] = {};
    };
};

class CfgFunctions {
    class abe {
        class abo_degradation {
            file = "z\abe\addons\abo_degradation";
            class barrelHeat {};
            class fouling {};
            class erosion {};
        };
    };
};
