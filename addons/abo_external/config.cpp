class CfgPatches {
    class abo_external {
        name = "Advanced Ballistics Extension - External Ballistics";
        author = "ABE Team";
        requiredVersion = 2.02;
        requiredAddons[] = {
            "cba_main",
            "cba_xeh",
            "abo_core",
            "abo_interior",
            "abo_environment"
        };
        units[] = {};
        weapons[] = {};
    };
};

class CfgFunctions {
    class abe {
        class abo_external {
            file = "z\abe\addons\abo_external";
            class step {};
            class calculateDrag {};
            class calculateWind {};
        };
    };
};
