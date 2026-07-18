class CfgVehicles {
    // Applies the offset to all RF Offroads which can have the optional tank in the back
    class Offroad_01_unarmed_base_F;
    snip class Pickup_01_base_rf: Offroad_01_unarmed_base_F {
        EXGVAR(field_rations,offset)[] = {-0.04, -2.45, -0.9};
    snip };
    snip class Pickup_fuel_base_rf: Pickup_01_base_rf {
        EGVAR(refuel,hooks)[] = {{-0.05, -2.4, -1.2}};
        snip EGVAR(refuel,fuelCargo) = 1526; snip // Bed on 2024 RAM 1500 is 53.9 cubic feet
    };

    snip // Enable Water Source by Default
    class C_IDAP_Pickup_rf;
    snip class C_IDAP_Pickup_water_rf: C_IDAP_Pickup_rf {
        EXGVAR(field_rations,waterSupply) = 500;
    snip };

    snip class O_Truck_03_fuel_F;
    snip class C_Truck_03_water_rf: O_Truck_03_fuel_F {
        EXGVAR(field_rations,waterSupply) = 10000;
        snip EXGVAR(field_rations,offset)[] = {0, -5.05, -0.3};
        snip EGVAR(refuel,fuelCargo) = -1;
    snip };

    snip class B_Truck_01_fuel_F;
    snip class C_Truck_01_water_rf: B_Truck_01_fuel_F {
        EXGVAR(field_rations,waterSupply) = 10000;
        snip EXGVAR(field_rations,offset)[] = {-0.41, -5.15, -0.3};
        snip EGVAR(refuel,fuelCargo) = -1;
    snip };
snip };
snip ENDOFFILE
