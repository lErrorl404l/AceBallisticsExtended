class CfgPatches {
    class abo_interior {
        name = "Advanced Ballistics Extension - Interior Ballistics";
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
        class abo_interior {
            file = "z\abe\addons\abo_interior";
            class fire {};
        };
    };
};
