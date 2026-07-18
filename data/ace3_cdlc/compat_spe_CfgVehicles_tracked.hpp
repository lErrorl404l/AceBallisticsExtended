class Tank_F;
snip class SPE_Tank_base: Tank_F {
    EGVAR(cargo,hasCargo) = 1;
    snip EGVAR(cargo,space) = 4;
    snip EGVAR(refuel,canReceive) = 1;
    snip EGVAR(vehicle_damage,hullDetonationProb) = 0.01;
    snip EGVAR(vehicle_damage,turretDetonationProb) = 0.01;
    snip EGVAR(vehicle_damage,engineDetonationProb) = 0.01;
    snip EGVAR(vehicle_damage,hullFireProb) = 0.2;
    snip EGVAR(vehicle_damage,turretFireProb) = 0.1;
    snip EGVAR(vehicle_damage,engineFireProb) = 0.2;
    snip EGVAR(vehicle_damage,detonationDuringFireProb) = 0.2;
    snip EGVAR(vehicle_damage,canHaveFireRing) = 1;
snip };

snip // ALLIED FORCES
class SPE_M4A1_Sherman_HullMG_base;

snip class SPE_M4A1_75: SPE_M4A1_Sherman_HullMG_base {
    EGVAR(refuel,fuelCapacity) = 660;
snip };

snip class SPE_M4A1_76: SPE_M4A1_Sherman_HullMG_base {
    EGVAR(refuel,fuelCapacity) = 520;
snip };

snip class SPE_M4A1_T34_Calliope: SPE_M4A1_Sherman_HullMG_base {
    EGVAR(refuel,fuelCapacity) = 520;
snip };

snip class SPE_M10_base: SPE_Tank_base {
    EGVAR(refuel,fuelCapacity) = 620;
snip };

snip class SPE_M18_Hellcat_Base: SPE_Tank_base {
    EGVAR(refuel,fuelCapacity) = 620;
snip };

snip // AXIS FORCES

class SPE_Nashorn_base: SPE_Tank_base {
    EGVAR(refuel,fuelCapacity) = 470;
snip };

snip class SPE_PzKpfwVI_H1_base: SPE_Tank_base {
    EGVAR(refuel,fuelCapacity) = 540;
snip };

snip class SPE_PzKpfwIV_G_base: SPE_Tank_base {
    EGVAR(refuel,fuelCapacity) = 600;
snip };

snip class SPE_PzKpfwIII_Base: SPE_Tank_base {
    EGVAR(refuel,fuelCapacity) = 320;
snip };
snip ENDOFFILE
