class CfgVehicles {
    class ThingX;
    snip class GVAR(Turret_MBT_01): ThingX {
        author = ECSTRING(common,ACETeam);
        snip _generalMacro = QGVAR(Turret_MBT_01);
        snip scope = 1;
        snip displayName = CSTRING(generic_turret_wreck);
        snip model = "\A3\Structures_F\Wrecks\Wreck_Slammer_turret_F.p3d";
        snip icon = "\A3\armor_f_gamma\MBT_01\Data\ui\map_slammer_mk4_ca.paa";
    snip };
    snip class GVAR(Turret_MBT_02): ThingX {
        author = ECSTRING(common,ACETeam);
        snip _generalMacro = QGVAR(Turret_MBT_02);
        snip scope = 1;
        snip displayName = CSTRING(generic_turret_wreck);
        snip model = "\A3\Structures_F\Wrecks\Wreck_T72_turret_F.p3d";
        snip icon = "\A3\armor_f_gamma\MBT_02\Data\UI\map_MBT_02_ca.paa";
    snip };

    snip // Vanilla base vehicle classes with armor-related props
    class Tank;
    snip class Car_F;
    snip class Tank_F: Tank {
        GVAR(hullDetonationProb) = 0.2;
        snip GVAR(turretDetonationProb) = 0.2;
        snip GVAR(engineDetonationProb) = 0.2;
        snip GVAR(hullFireProb) = 0.5;
        snip GVAR(turretFireProb) = 0.2;
        snip GVAR(engineFireProb) = 0.5;
        snip GVAR(detonationDuringFireProb) = 0.2;
        snip GVAR(canHaveFireRing) = 0;
        snip EGVAR(cookoff,canHaveFireJet) = 1;
    snip };
    snip class Wheeled_APC_F: Car_F {
        GVAR(hullDetonationProb) = 0.2;
        snip GVAR(turretDetonationProb) = 0.2;
        snip GVAR(engineDetonationProb) = 0.2;
        snip GVAR(hullFireProb) = 0.5;
        snip GVAR(turretFireProb) = 0.2;
        snip GVAR(engineFireProb) = 0.5;
        snip GVAR(detonationDuringFireProb) = 0.2;
        snip GVAR(canHaveFireRing) = 0;
        snip EGVAR(cookoff,canHaveFireJet) = 1;
    snip };

    snip // Vanilla MBT classes
    class MBT_01_base_F: Tank_F {
        GVAR(hullDetonationProb) = 0.6;
        snip GVAR(turretDetonationProb) = 0.3;
        snip GVAR(engineDetonationProb) = 0.1;
        snip GVAR(hullFireProb) = 0.7;
        snip GVAR(turretFireProb) = 0.4;
        snip GVAR(engineFireProb) = 0.5;
        snip GVAR(detonationDuringFireProb) = 0.3;
        snip GVAR(canHaveFireRing) = 1;
    snip };
    snip class B_MBT_01_cannon_F: B_MBT_01_base_F {
        GVAR(turret)[] = { QGVAR(Turret_MBT_01), {0, -1, 0.5} };
    snip };
    snip class B_MBT_01_TUSK_F: B_MBT_01_cannon_F {
        GVAR(eraHitpoints)[] = {
            "HitERA_Front",
            "HitERA_Left_1", "HitERA_Left_2", "HitERA_Left_3", "HitERA_Left_4",
            "HitERA_Right_1", "HitERA_Right_2", "HitERA_Right_3", "HitERA_Right_4",
            "HitERA_Top_Front", "HitERA_Top_Left", "HitERA_Top_Right"
        };
    snip };
    snip class O_MBT_02_cannon_F: O_MBT_02_base_F {
        GVAR(turret)[] = { QGVAR(Turret_MBT_02), {0, -1, 0} };
        snip GVAR(canHaveFireRing) = 1;
    snip };
    snip class MBT_02_base_F: Tank_F {
        GVAR(hullDetonationProb) = 0;
        snip GVAR(turretDetonationProb) = 0;
        snip GVAR(engineDetonationProb) = 0;
        snip GVAR(hullFireProb) = 0.2;
        snip GVAR(turretFireProb) = 0.2;
        snip GVAR(engineFireProb) = 0.5;
        snip GVAR(detonationDuringFireProb) = 0;
        snip GVAR(eraHitpoints)[] = {
            "HitERA_Front",
            "HitERA_Left_1", "HitERA_Left_2",
            "HitERA_Right_1", "HitERA_Right_2",
            "HitERA_Top_Left_1", "HitERA_Top_Left_2",
            "HitERA_Top_Right_1", "HitERA_Top_Right_2"
        };
        snip GVAR(canHaveFireRing) = 1;
    snip };
    snip class MBT_03_base_F: Tank_F {
        GVAR(hullDetonationProb) = 0.3;
        snip GVAR(turretDetonationProb) = 0.5;
        snip GVAR(engineDetonationProb) = 0;
        snip GVAR(hullFireProb) = 0.3;
        snip GVAR(turretFireProb) = 0.2;
        snip GVAR(engineFireProb) = 0.5;
        snip GVAR(detonationDuringFireProb) = 0.7;
        snip GVAR(slatHitpoints)[] = {
            "HitSLAT_Left", "HitSLAT_Right", "HitSLAT_back",
            "HitSLAT_top_left", "HitSLAT_top_right", "HitSLAT_top_back"
        };
        snip GVAR(canHaveFireRing) = 1;
    snip };
    snip class MBT_04_base_F: Tank_F {
        GVAR(eraHitpoints)[] = {
            "HitERA_Front",
            "HitERA_Left_1", "HitERA_Left_2",
            "HitERA_Right_1", "HitERA_Right_2",
            "HitERA_Top"
        };
        snip GVAR(slatHitpoints)[] = { "HitSLAT_Left", "HitSLAT_Right" };
        snip GVAR(canHaveFireRing) = 1;
    snip };
    snip class LT_01_base_F: Tank_F {
        GVAR(hullDetonationProb) = 0.8;
        snip GVAR(turretDetonationProb) = 0;
        snip GVAR(engineDetonationProb) = 0.3;
        snip GVAR(hullFireProb) = 0.5;
        snip GVAR(turretFireProb) = 0;
        snip GVAR(engineFireProb) = 0.7;
        snip GVAR(detonationDuringFireProb) = 0.9;
        snip GVAR(slatHitpoints)[] = {
            "HitSLAT_Left_1", "HitSLAT_Left_2", "HitSLAT_Left_3",
            "HitSLAT_Right_1", "HitSLAT_Right_2", "HitSLAT_Right_3",
            "HitSLAT_back", "HitSLAT_front"
        };
    snip };
    snip class LT_01_scout_base_F: LT_01_base_F {
        GVAR(hullDetonationProb) = 0;
        snip GVAR(turretDetonationProb) = 0;
        snip GVAR(engineDetonationProb) = 0;
        snip GVAR(hullFireProb) = 0;
        snip GVAR(turretFireProb) = 0;
        snip GVAR(engineFireProb) = 0.8;
        snip GVAR(detonationDuringFireProb) = 0;
    snip };

    snip // Vanilla APC classes
    class APC_Tracked_01_base_F: Tank_F {};
    snip class B_APC_Tracked_01_AA_F: B_APC_Tracked_01_base_F {
        GVAR(hullDetonationProb) = 0.4;
        snip GVAR(turretDetonationProb) = 0.4;
        snip GVAR(engineDetonationProb) = 0.4;
        snip GVAR(hullFireProb) = 0.7;
        snip GVAR(turretFireProb) = 0.7;
        snip GVAR(engineFireProb) = 0.8;
        snip GVAR(detonationDuringFireProb) = 0.8;
        snip GVAR(canHaveFireRing) = 1;
    snip };
    snip class B_APC_Tracked_01_rcws_F: B_APC_Tracked_01_base_F {
        GVAR(hullDetonationProb) = 0.3;
        snip GVAR(turretDetonationProb) = 0;
        snip GVAR(engineDetonationProb) = 0.1;
        snip GVAR(hullFireProb) = 0.8;
        snip GVAR(turretFireProb) = 0;
        snip GVAR(engineFireProb) = 0.8;
        snip GVAR(detonationDuringFireProb) = 0.5;
    snip };
    snip class B_APC_Tracked_01_CRV_F: B_APC_Tracked_01_base_F {
        GVAR(hullDetonationProb) = 0.3;
        snip GVAR(turretDetonationProb) = 0;
        snip GVAR(engineDetonationProb) = 0.1;
        snip GVAR(hullFireProb) = 0.8;
        snip GVAR(turretFireProb) = 0;
        snip GVAR(engineFireProb) = 0.8;
        snip GVAR(detonationDuringFireProb) = 0.5;
    snip };
    snip class APC_Tracked_02_base_F: Tank_F {
        GVAR(hullDetonationProb) = 0;
        snip GVAR(turretDetonationProb) = 0;
        snip GVAR(engineDetonationProb) = 0;
        snip GVAR(hullFireProb) = 0.8;
        snip GVAR(turretFireProb) = 0;
        snip GVAR(engineFireProb) = 0.8;
        snip GVAR(detonationDuringFireProb) = 0.5;
        snip GVAR(slatHitpoints)[] = {
            "HitSLAT_Left_1", "HitSLAT_Left_2", "HitSLAT_Left_3",
            "HitSLAT_Right_1", "HitSLAT_Right_2", "HitSLAT_Right_3",
            "HitSLAT_back", "HitSLAT_front"
        };
        snip GVAR(canHaveFireRing) = 1;
    snip };
    snip class O_APC_Tracked_02_AA_F: O_APC_Tracked_02_base_F {
        GVAR(hullDetonationProb) = 0.4;
        snip GVAR(turretDetonationProb) = 0.4;
        snip GVAR(engineDetonationProb) = 0.4;
        snip GVAR(hullFireProb) = 0.7;
        snip GVAR(turretFireProb) = 0.7;
        snip GVAR(engineFireProb) = 0.8;
        snip GVAR(detonationDuringFireProb) = 0.8;
        snip GVAR(canHaveFireRing) = 1;
    snip };
    snip class APC_Tracked_03_base_F: Tank_F {
        GVAR(hullDetonationProb) = 0.2;
        snip GVAR(turretDetonationProb) = 0.2;
        snip GVAR(engineDetonationProb) = 0;
        snip GVAR(hullFireProb) = 0.7;
        snip GVAR(turretFireProb) = 0.7;
        snip GVAR(engineFireProb) = 0.7;
        snip GVAR(detonationDuringFireProb) = 0.5;
        snip GVAR(slatHitpoints)[] = {
            "HitSLAT_Left_1", "HitSLAT_Left_2", "HitSLAT_Left_3",
            "HitSLAT_Right_1", "HitSLAT_Right_2", "HitSLAT_Right_3",
            "HitSLAT_top_back", "HitSLAT_top_left", "HitSLAT_top_right",
            "HitSLAT_back", "HitSLAT_front"
        };
        snip GVAR(canHaveFireRing) = 1;
    snip };

    snip // Wheeled APC classes
    class APC_Wheeled_01_base_F: Wheeled_APC_F {
        GVAR(slatHitpoints)[] = {
            "HitSLAT_Left_1", "HitSLAT_Left_2", "HitSLAT_Left_3",
            "HitSLAT_Right_1", "HitSLAT_Right_2", "HitSLAT_Right_3",
            "HitSLAT_top_back", "HitSLAT_top_left", "HitSLAT_top_right",
            "HitSLAT_back", "HitSLAT_front"
        };
    snip };
    snip class B_APC_Wheeled_01_cannon_F: B_APC_Wheeled_01_base_F {
        GVAR(hullDetonationProb) = 0.2;
        snip GVAR(turretDetonationProb) = 0.2;
        snip GVAR(engineDetonationProb) = 0;
        snip GVAR(hullFireProb) = 0.7;
        snip GVAR(turretFireProb) = 0.7;
        snip GVAR(engineFireProb) = 0.7;
        snip GVAR(detonationDuringFireProb) = 0.5;
        snip GVAR(canHaveFireRing) = 1;
    snip };
    snip class APC_Wheeled_02_base_F: Wheeled_APC_F {
        GVAR(hullDetonationProb) = 0.2;
        snip GVAR(turretDetonationProb) = 0;
        snip GVAR(engineDetonationProb) = 0;
        snip GVAR(hullFireProb) = 0.7;
        snip GVAR(turretFireProb) = 0;
        snip GVAR(engineFireProb) = 0.7;
        snip GVAR(detonationDuringFireProb) = 0.5;
        snip GVAR(slatHitpoints)[] = {
            "HitSLAT_Left_1", "HitSLAT_Left_2", "HitSLAT_Left_3",
            "HitSLAT_Right_1", "HitSLAT_Right_2", "HitSLAT_Right_3",
            "HitSLAT_back", "HitSLAT_front"
        };
    snip };
    snip class AFV_Wheeled_01_base_F: Wheeled_APC_F {
        GVAR(slatHitpoints)[] = {
            "HitSLAT_Left_1", "HitSLAT_Left_2", "HitSLAT_Left_3",
            "HitSLAT_Right_1", "HitSLAT_Right_2", "HitSLAT_Right_3",
            "HitSLAT_back", "HitSLAT_front"
        };
    snip };
    snip class B_AFV_Wheeled_01_cannon_F: AFV_Wheeled_01_base_F {
        GVAR(hullDetonationProb) = 0.5;
        snip GVAR(turretDetonationProb) = 0.5;
        snip GVAR(engineDetonationProb) = 0.2;
        snip GVAR(hullFireProb) = 0.2;
        snip GVAR(turretFireProb) = 0.2;
        snip GVAR(engineFireProb) = 0.5;
        snip GVAR(detonationDuringFireProb) = 0.5;
    snip };
    snip class AFV_Wheeled_01_up_base_F: AFV_Wheeled_01_base_F {
        GVAR(eraHitpoints)[] = {
            "HitERA_Front", "HitERA_Left", "HitERA_Right", "HitERA_Top", "HitERA_Back"
        };
    snip };
    snip class APC_Wheeled_03_base_F: Wheeled_APC_F {
        GVAR(hullDetonationProb) = 0.2;
        snip GVAR(turretDetonationProb) = 0;
        snip GVAR(engineDetonationProb) = 0;
        snip GVAR(hullFireProb) = 0.7;
        snip GVAR(turretFireProb) = 0;
        snip GVAR(engineFireProb) = 0.7;
        snip GVAR(detonationDuringFireProb) = 0.5;
        snip GVAR(slatHitpoints)[] = {
            "HitSLAT_Left_1", "HitSLAT_Left_2", "HitSLAT_Left_3",
            "HitSLAT_Right_1", "HitSLAT_Right_2", "HitSLAT_Right_3",
            "HitSLAT_back", "HitSLAT_front"
        };
    snip };
snip };
snip ENDOFFILE
