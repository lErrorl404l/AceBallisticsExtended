class CfgPatches {
    class abo_environment {
        name = "Advanced Ballistics Extension - Environment";
        author = "ABE Team";
        requiredVersion = 2.02;
        requiredAddons[] = {
            "cba_main",
            "cba_xeh"
        };
        units[] = {};
        weapons[] = {};
    };
};

class CfgFunctions {
    class abe {
        class abo_environment {
            file = "z\abe\addons\abo_environment";
            class calculateAtmosphere {};
            class getWindGradient {};
            class getDensity {};
        };
    };
};
