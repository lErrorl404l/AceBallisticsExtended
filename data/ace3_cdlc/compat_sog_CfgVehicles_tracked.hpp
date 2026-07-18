// M113A1 https://man.fas.org/dod-101/sys/land/m113.htm
class Tank_F;
snip class APC_Tracked_01_base_F: Tank_F {
    class ACE_Actions;
snip };
snip class vn_armor_m113_base: APC_Tracked_01_base_F {
    EGVAR(refuel,fuelCapacity) = 360;
    snip class ACE_Actions: ACE_Actions {
        class ACE_MainActions;
    snip };
snip };
snip class vn_armor_m577_base: vn_armor_m113_base {
    class ACE_Actions: ACE_Actions {
        class ACE_MainActions: ACE_MainActions {
            position = "[0, 3.5, -1]";
        snip };
    snip };
snip };

snip // M41
class vn_armor_tank_base;
snip class vn_armor_m41_base: vn_armor_tank_base {
    EGVAR(refuel,fuelCapacity) = 530;
snip };

snip // M48
class vn_armor_m48_base: vn_armor_tank_base {
    class EGVAR(interaction,anims) {
        class hide_mainturret_backpacks {
            positions[] = {
                "[[-1.3, -0.7, -0.4], [0, 0, 1], deg (_target animationPhase 'MainTurret')] call CBA_fnc_vectRotate3D",
                "[[0.1, -2, -0.2], [0, 0, 1], deg (_target animationPhase 'MainTurret')] call CBA_fnc_vectRotate3D"
            };
            snip items[] = {"vn_b_pack_02_02", "vn_b_pack_04_02"};
            snip name = "$STR_a3_cfgvehicleclasses_backpacks0";
            snip text = "$STR_a3_cfgvehicleclasses_backpacks0";
        snip };
    snip };
snip };

snip // T-54
class vn_armor_t54_base: vn_armor_tank_base {
    class EGVAR(interaction,anims) {
        class hide_mainturret_backpacks {
            positions[] = {"[0, -0.2, 0] vectorAdd ([[-1.1, -0.85, -1.3], [0, 0, 1], deg (_target animationPhase 'MainTurret')] call CBA_fnc_vectRotate3D)"};
            snip items[] = {"vn_o_pack_01", "vn_o_pack_02"};
            snip name = "$STR_a3_cfgvehicleclasses_backpacks0";
            snip text = "$STR_a3_cfgvehicleclasses_backpacks0";
        snip };
    snip };
snip };

snip // PT-76A https://en.wikipedia.org/wiki/PT-76
class vn_armor_pt76_base: vn_armor_tank_base {
    EGVAR(refuel,fuelCapacity) = 250;
snip };

snip // PT-76B (This variant increased the fuel size from 250 to 400 liters)
class vn_armor_pt76b_base: vn_armor_pt76_base {
    EGVAR(refuel,fuelCapacity) = 400;
snip };

snip // Type 63
class vn_armor_type63_base: vn_armor_tank_base {
    EGVAR(refuel,fuelCapacity) = 545;
snip };
snip ENDOFFILE
