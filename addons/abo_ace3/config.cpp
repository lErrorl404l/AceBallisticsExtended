class CfgPatches {
    class abo_ace3 {
        name = "Advanced Ballistics Extension - ACE3 Integration";
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
        class abo_ace3 {
            file = "z\abe\addons\abo_ace3";
            class hookAceBallistics {};
            class dispatchStandalone {};
        };
    };
};
