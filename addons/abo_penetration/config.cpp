class CfgPatches {
    class abo_penetration {
        name = "Advanced Ballistics Extension - Penetration";
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
        class abo_penetration {
            file = "z\abe\addons\abo_penetration";
            class calculatePenetration {};
            class calculateAngleEffect {};
            class overmatch {};
        };
    };
};
