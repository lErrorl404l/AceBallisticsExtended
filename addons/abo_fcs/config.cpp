class CfgPatches {
    class abo_fcs {
        name = "Advanced Ballistics Extension - Fire Control System";
        author = "ABE Team";
        requiredVersion = 2.02;
        requiredAddons[] = {
            "cba_main",
            "cba_xeh",
            "abo_core",
            "abo_external",
            "abo_environment"
        };
        units[] = {};
        weapons[] = {};
    };
};

class CfgFunctions {
    class abe {
        class abo_fcs {
            file = "z\abe\addons\abo_fcs";
            class calculateReticle {};
            class correctCant {};
            class correctAltitude {};
        };
    };
};
