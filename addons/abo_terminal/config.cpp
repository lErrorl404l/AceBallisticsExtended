class CfgPatches {
    class abo_terminal {
        name = "Advanced Ballistics Extension - Terminal Ballistics";
        author = "ABE Team";
        requiredVersion = 2.02;
        requiredAddons[] = {
            "cba_main",
            "cba_xeh",
            "abo_core",
            "abo_external"
        };
        units[] = {};
        weapons[] = {};
    };
};

class CfgFunctions {
    class abe {
        class abo_terminal {
            file = "z\abe\addons\abo_terminal";
            class impact {};
            class fragment {};
            class yaw {};
        };
    };
};
