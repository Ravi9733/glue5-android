--!strict
-- Vehicle Repair menu for GLUE 5 Android.
-- Repairs the vehicle the player is currently using.

local root = ui.menu("Vehicle Repair", "Instantly repair your current vehicle")

local function currentVehicle()
    local me = player.playerPedId()
    if not ped.isPedInAnyVehicle(me, false) then
        ui.notify("Get inside the vehicle first")
        return nil
    end
    local car = ped.getVehiclePedIsIn(me, false)
    if not car or not entity.doesEntityExist(car) then
        ui.notify("Could not find your current vehicle")
        return nil
    end
    return car
end

local function repairAll()
    local car = currentVehicle()
    if not car then return end
    vehicle.setVehicleFixed(car)
    vehicle.setVehicleDeformationFixed(car)
    vehicle.setVehicleEngineHealth(car, 1000.0)
    vehicle.setVehicleDirtLevel(car, 0.0)
    vehicle.setVehicleUndriveable(car, false)
    vehicle.setVehicleEngineOn(car, true, true, false)
    ui.notify("Vehicle fully repaired!")
end

root:button("INSTANT REPAIR", { note = "Fix body, engine, deformation and dirt", action = "Repair" }, repairAll)
root:button("Fix Body & Deformation", { note = "Repair bodywork and deformation", action = "Fix" }, function()
    local car = currentVehicle()
    if not car then return end
    vehicle.setVehicleFixed(car)
    vehicle.setVehicleDeformationFixed(car)
    ui.notify("Body repaired!")
end)
root:button("Fix Engine", { note = "Restore engine health and make it driveable", action = "Fix" }, function()
    local car = currentVehicle()
    if not car then return end
    vehicle.setVehicleEngineHealth(car, 1000.0)
    vehicle.setVehicleUndriveable(car, false)
    vehicle.setVehicleEngineOn(car, true, true, false)
    ui.notify("Engine repaired!")
end)
root:button("Clean Vehicle", { note = "Remove dirt", action = "Clean" }, function()
    local car = currentVehicle()
    if not car then return end
    vehicle.setVehicleDirtLevel(car, 0.0)
    ui.notify("Vehicle cleaned!")
end)

ui.notify("Vehicle Repair menu loaded")
