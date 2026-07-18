class CfgVehicles {

    // REPAIR, REFUEL, REARM

    class ReammoBox_F;
    snip class gm_AmmoBox_base: ReammoBox_F {
        EGVAR(cargo,size) = 1;
        snip EGVAR(cargo,canLoad) = 1;
        snip EGVAR(dragging,canCarry) = 1;
        snip EGVAR(dragging,carryPosition)[] = {0,1,1};
        snip EGVAR(dragging,carryDirection) = 0;
        snip EGVAR(dragging,canDrag) = 1;
        snip EGVAR(dragging,dragPosition)[] = {0,1.2,0};
        snip EGVAR(dragging,dragDirection) = 0;
    snip };

    snip class gm_jerrycan_base;
    snip class gm_jerrycan: gm_jerrycan_base {
        EGVAR(cargo,size) = 1;
        snip EGVAR(cargo,canLoad) = 1;
        snip EGVAR(dragging,canCarry) = 1;
        snip EGVAR(dragging,carryPosition)[] = {0,1,1};
        snip EGVAR(dragging,carryDirection) = 0;
        snip EGVAR(dragging,canDrag) = 1;
        snip EGVAR(dragging,dragPosition)[] = {0,1.2,0};
        snip EGVAR(dragging,dragDirection) = 0;
    snip };

    snip // STATIC
    class gm_ge_army_shelteraceII_repair_base;
    snip class gm_ge_army_shelteraceII_repair: gm_ge_army_shelteraceII_repair_base {
        EGVAR(repair,canRepair) = 1;
    snip };
    snip class gm_gc_army_shelterlakII_repair_base;
    snip class gm_gc_army_shelterlakII_repair: gm_gc_army_shelterlakII_repair_base {
        EGVAR(repair,canRepair) = 1;
    snip };

    snip // WHEELED
    class gm_wheeled_base;
    snip class gm_wheeled_truck_base;
    snip class gm_wheeled_APC_base;
    snip class gm_wheeled_motorcycle_base;

    snip // EAST wheeled
    class gm_wheeled_car_base: gm_wheeled_base {
        EGVAR(cargo,hasCargo) = 1;
        snip EGVAR(cargo,space) = 4;
        snip EGVAR(refuel,canReceive) = 1;
        snip EGVAR(vehicle_damage,hullDetonationProb) = 0.2;
        snip EGVAR(vehicle_damage,turretDetonationProb) = 0.03;
        snip EGVAR(vehicle_damage,engineDetonationProb) = 0.03;
        snip EGVAR(vehicle_damage,hullFireProb) = 0.6;
        snip EGVAR(vehicle_damage,turretFireProb) = 0.1;
        snip EGVAR(vehicle_damage,engineFireProb) = 0.2;
        snip EGVAR(vehicle_damage,detonationDuringFireProb) = 0.2;
        snip EGVAR(vehicle_damage,canHaveFireRing) = 0.1;
    snip };
    snip class gm_wheeled_bicycle_base: gm_wheeled_base {
        EGVAR(cargo,hasCargo) = 0;
        snip EGVAR(refuel,canReceive) = 0;
    snip };
    snip class gm_uaz469_base: gm_wheeled_car_base { EGVAR(refuel,fuelCapacity) = 78; snip };
    snip class gm_p601_base: gm_wheeled_car_base { EGVAR(refuel,fuelCapacity) = 26; snip };
    snip class gm_brdm2_base: gm_wheeled_APC_base {
        EGVAR(refuel,fuelCapacity) = 290;
        snip EGVAR(vehicle_damage,hullDetonationProb) = 0;
        snip EGVAR(vehicle_damage,turretDetonationProb) = 0.2;
        snip EGVAR(vehicle_damage,engineDetonationProb) = 0.2;
        snip EGVAR(vehicle_damage,hullFireProb) = 0.5;
        snip EGVAR(vehicle_damage,turretFireProb) = 0.7;
        snip EGVAR(vehicle_damage,engineFireProb) = 0.7;
        snip EGVAR(vehicle_damage,detonationDuringFireProb) = 0.5;
        snip EGVAR(vehicle_damage,canHaveFireRing) = 0;
        snip EGVAR(cookoff,canHaveFireJet) = 0;
    snip };
    snip class gm_brdm2um_base: gm_brdm2_base { /* variant */ };
    snip class gm_brdm2_9p133_base: gm_brdm2_base { /* variant */ };
    snip class gm_btr60_base: gm_wheeled_APC_base {
        EGVAR(refuel,fuelCapacity) = 290;
        snip EGVAR(cookoff,cookoffSelections)[] = {"commanderturret_hatch"};
    snip };
    snip class gm_btr60pb_base: gm_btr60_base { /* variant */ };
    snip class gm_ot64_base: gm_wheeled_APC_base { /* vehicle_damage props */ };
    snip class gm_ural375d_base: gm_wheeled_truck_base { EGVAR(refuel,fuelCapacity) = 360; snip };
    snip class gm_ural375d_mlrs_base: gm_ural375d_base { /* variant */ };
    snip class gm_ural375d_medic_base: gm_ural375d_base { /* variant */ };
    snip class gm_ural4320_base: gm_wheeled_truck_base { EGVAR(refuel,fuelCapacity) = 360; snip };
    snip class gm_ural4320_reammo_base: gm_ural4320_base { EGVAR(rearm,defaultSupply) = 1200; snip };
    snip class gm_ural4320_refuel_base: gm_ural4320_base { /* refuel */ };
    snip class gm_ural4320_medic_base: gm_ural4320_base { EGVAR(medical,medicClass) = 1; snip };
    snip class gm_ural4320_repair_base: gm_ural4320_base { /* variant */ };
    snip class gm_ural44202_base: gm_ural4320_base { /* variant */ };

    snip // WEST wheeled
    class gm_k125_base: gm_wheeled_motorcycle_base { EGVAR(refuel,fuelCapacity) = 14.5; snip };
    snip class gm_typ1_base: gm_wheeled_car_base { EGVAR(refuel,fuelCapacity) = 47.3; snip };
    snip class gm_iltis_base: gm_wheeled_car_base { EGVAR(refuel,fuelCapacity) = 83; snip };
    snip class gm_u1300l_base: gm_wheeled_truck_base { EGVAR(refuel,fuelCapacity) = 90; snip };
    snip class gm_u1300l_medic_base: gm_u1300l_base { EGVAR(medical,medicClass) = 1; snip };
    snip class gm_kat1_base: gm_wheeled_truck_base { EGVAR(refuel,fuelCapacity) = 270; snip };
    snip class gm_kat1_451_refuel_base: gm_kat1_451_base { EGVAR(refuel,fuelCargo) = 4600; snip };
    snip class gm_kat1_454_cargo_base: gm_kat1_454_base { EGVAR(cargo,space) = 10; snip };
    snip class gm_fuchs_base: gm_wheeled_APC_base { EGVAR(refuel,fuelCapacity) = 390; snip };
    snip class gm_luchs_base: gm_wheeled_APC_base { EGVAR(refuel,fuelCapacity) = 500; snip };

    snip // TRACKED
    class Tank_F;
    snip class gm_tracked_base: Tank_F {
        EGVAR(cargo,hasCargo) = 1;
        snip EGVAR(cargo,space) = 4;
        snip EGVAR(refuel,canReceive) = 1;
    snip };
    snip class gm_tracked_APC_base: gm_tracked_base { /* vehicle_damage */ };
    snip class gm_tracked_Tank_base: gm_tracked_base { /* vehicle_damage */ };

    snip // EAST tracked
    class gm_bmp1_base: gm_tracked_APC_base { EGVAR(refuel,fuelCapacity) = 460; snip };
    snip class gm_bmp1sp2_base: gm_bmp1_base { /* interaction anims */ };
    snip class gm_pt76_base: gm_tracked_Tank_base { EGVAR(refuel,fuelCapacity) = 250; snip };
    snip class gm_t55_base: gm_tracked_Tank_base { EGVAR(refuel,fuelCapacity) = 900; snip };
    snip class gm_zsu234_base: gm_tracked_Tank_base { EGVAR(refuel,fuelCapacity) = 812; snip };
    snip class gm_2s1_base: gm_tracked_Artillery_base { /* interaction */ };

    snip // WEST tracked
    class gm_Leopard1a0_base: gm_Leopard1_base { EGVAR(refuel,fuelCapacity) = 955; snip };
    snip class gm_Leopard1a1_base: gm_Leopard1a0_base { /* interaction */ };
    snip class gm_Gepard_base: gm_Leopard1_base { EGVAR(refuel,fuelCapacity) = 985; snip };
    snip class gm_BPz2a0_base: gm_BPz2_base { EGVAR(refuel,fuelCapacity) = 1160; snip };
    snip class gm_marder1_base: gm_tracked_APC_base { EGVAR(refuel,fuelCapacity) = 652; snip };
    snip class gm_m113_base: gm_tracked_APC_base { EGVAR(refuel,fuelCapacity) = 360; snip };
    snip class gm_m113a1g_medic_base: gm_m113a1g_base { EGVAR(medical,medicClass) = 1; snip };
    snip class gm_m113a1dk_medic_base: gm_m113a1dk_base { EGVAR(medical,medicClass) = 1; snip };

    snip // HELICOPTERS
    class gm_bo105_base: gm_helicopter_base { EGVAR(refuel,fuelCapacity) = 3700; snip };
    snip class gm_bo105p1m_vbh_swooper_base: gm_bo105p1m_vbh_base { EGVAR(fastroping,enabled) = 1; snip };
    snip class gm_ch53_base: gm_helicopter_base { EGVAR(refuel,fuelCapacity) = 3850; snip };
    snip class gm_ch53g_base: gm_ch53_base { EGVAR(refuel,fuelCapacity) = 8770; snip };
    snip class gm_mi2_base: gm_helicopter_base { EGVAR(refuel,fuelCapacity) = 600; snip };
    snip class gm_mi2sr_base: gm_mi2_base { EGVAR(refuel,fuelCapacity) = 1076; snip };
    snip class gm_mi2p_base: gm_mi2_base { EGVAR(refuel,fuelCapacity) = 1076; snip };
    snip class gm_mi2ch_base: gm_mi2_base { EGVAR(refuel,fuelCapacity) = 1076; snip };
    snip class gm_mi2platan_base: gm_mi2_base { EGVAR(refuel,fuelCapacity) = 1076; snip };

    snip // PLANES
    class gm_l410_base: gm_plane_base { EGVAR(refuel,fuelCapacity) = 1300; snip };
    snip class gm_do28d2_base: gm_plane_base { EGVAR(refuel,fuelCapacity) = 894; snip };
    snip class gm_do28d2_medevac_base: gm_do28d2_base { EGVAR(medical,medicClass) = 1; snip };
snip };
snip ENDOFFILE
