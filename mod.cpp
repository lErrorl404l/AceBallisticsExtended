class CfgPatches {
    class abe_main {
        name = "Advanced Ballistics Extension";
        author = "ABE Team";
        url = "https://github.com/lErrorl404l/AceBallisticsExtended";
        requiredVersion = 2.0;
        requiredAddons[] = {"A3_Data_F"};
        units[] = {};
        weapons[] = {};
    };
};

class CfgMod {
    author = "ABE Team";
    name = "Advanced Ballistics Extension";
    dir = "@abe";
    action = "https://github.com/lErrorl404l/AceBallisticsExtended";
    overview = "Data-driven ballistics framework for ARMA 3. Extension-based interior, external, terminal, and penetration ballistics.";
};
