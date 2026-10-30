--!strict
-- Car Customizer: a standalone top-level GLUE 5 menu, listed directly in F7 > MENUS.
-- Uses only GLUE 5 menu and vehicle natives; no Glue Trainer dependency.

local root = ui.menu("Car Customizer", "Paint, neon, tyre smoke, built-in liveries and vehicle tuning")

local function currentVehicle(): Vehicle?
    local me = player.playerPedId()
    if not ped.isPedInAnyVehicle(me, false) then
        ui.notify("Get inside a vehicle first")
        return nil
    end
    local car = ped.getVehiclePedIsIn(me, false)
    if not car or car == 0 then
        ui.notify("Could not find your current vehicle")
        return nil
    end
    return car
end

local function rgb(r: number, g: number, b: number): Rgba
    return { r = r, g = g, b = b, a = 255 }
end

local paint = root:submenu("Paint & Lights", "Primary, secondary, neon and tyre-smoke colours")
paint:onOpen(function()
    paint:clear()
    local car = currentVehicle()
    if not car then
        paint:label("Enter a vehicle, then reopen this page.")
        return
    end
    vehicle.setVehicleModKit(car, 0)
    paint:separator("Custom paint")
    paint:color("Primary colour", rgb(vehicle.getVehicleCustomPrimaryColour(car)), function(c)
        vehicle.setVehicleCustomPrimaryColour(car, c.r, c.g, c.b)
    end)
    paint:color("Secondary colour", rgb(vehicle.getVehicleCustomSecondaryColour(car)), function(c)
        vehicle.setVehicleCustomSecondaryColour(car, c.r, c.g, c.b)
    end)
    paint:separator("Neon")
    paint:toggle("Neon enabled", vehicle.getVehicleNeonEnabled(car, 0), function(on)
        for side = 0, 3 do
            vehicle.setVehicleNeonEnabled(car, side, on)
        end
    end)
    paint:color("Neon colour", rgb(vehicle.getVehicleNeonColour(car)), function(c)
        vehicle.setVehicleNeonColour(car, c.r, c.g, c.b)
        for side = 0, 3 do
            vehicle.setVehicleNeonEnabled(car, side, true)
        end
    end)
    paint:separator("Tyres")
    paint:color("Tyre-smoke colour", rgb(vehicle.getVehicleTyreSmokeColor(car)), function(c)
        vehicle.toggleVehicleMod(car, 20, true)
        vehicle.setVehicleTyreSmokeColor(car, c.r, c.g, c.b)
    end)
end)

local looks = root:submenu("Stickers & Stripes", "Uses decals and stripes built into each vehicle")
looks:onOpen(function()
    looks:clear()
    local car = currentVehicle()
    if not car then
        looks:label("Enter a vehicle, then reopen this page.")
        return
    end
    vehicle.setVehicleModKit(car, 0)
    looks:separator("Built-in designs")
    local count = vehicle.getVehicleLiveryCount(car)
    if count > 0 then
        local options = {}
        for i = 0, count - 1 do
            table.insert(options, "Livery " .. tostring(i + 1))
        end
        looks:choice("Decal / stripe design", options, math.clamp(vehicle.getVehicleLivery(car) + 1, 1, count), function(i)
            vehicle.setVehicleLivery(car, i - 1)
        end)
    else
        looks:label("This vehicle has no built-in livery, sticker or stripe options.")
    end
    local plateCount = vehicle.getNumberOfVehicleNumberPlates()
    if plateCount > 0 then
        local plates = {}
        for i = 0, plateCount - 1 do
            table.insert(plates, "Plate style " .. tostring(i + 1))
        end
        looks:choice("Number-plate style", plates, math.clamp(vehicle.getVehicleNumberPlateTextIndex(car) + 1, 1, plateCount), function(i)
            vehicle.setVehicleNumberPlateTextIndex(car, i - 1)
        end)
    end
    looks:text("Number plate text", string.gsub(vehicle.getVehicleNumberPlateText(car), "%s+$", ""), { max = 8 }, function(value)
        vehicle.setVehicleNumberPlateText(car, value)
    end)
    looks:separator("Vehicle extras")
    local found = false
    for id = 1, 14 do
        if vehicle.doesExtraExist(car, id) then
            found = true
            looks:toggle("Extra " .. tostring(id), vehicle.isVehicleExtraTurnedOn(car, id), function(on)
                vehicle.setVehicleExtra(car, id, not on)
            end)
        end
    end
    if not found then
        looks:label("This vehicle has no optional extras.")
    end
end)

local tuning = root:submenu("Tuning & Upgrades", "Performance parts, wheels, turbo, xenon and extras")
tuning:onOpen(function()
    tuning:clear()
    local car = currentVehicle()
    if not car then
        tuning:label("Enter a vehicle, then reopen this page.")
        return
    end
    vehicle.setVehicleModKit(car, 0)
    tuning:button("Upgrade everything", { note = "Apply the best available parts", action = "Upgrade" }, function()
        vehicle.setVehicleModKit(car, 0)
        for slot = 0, 49 do
            if slot == 18 or slot == 22 then
                vehicle.toggleVehicleMod(car, slot, true)
            else
                local n = vehicle.getNumVehicleMods(car, slot)
                if n > 0 then
                    vehicle.setVehicleMod(car, slot, n - 1, false)
                end
            end
        end
        ui.notify("Available upgrades applied")
    end)
    local wheelNames = { "Sport", "Muscle", "Lowrider", "SUV", "Offroad", "Tuner", "Bike", "High End", "Benny Original", "Benny Bespoke", "Open Wheel", "Street", "Track" }
    tuning:choice("Wheel type", wheelNames, math.clamp(vehicle.getVehicleWheelType(car) + 1, 1, #wheelNames), function(i)
        vehicle.setVehicleWheelType(car, i - 1)
        ui.notify("Wheel type changed")
    end)
    tuning:toggle("Turbo", vehicle.isToggleModOn(car, 18), function(on)
        vehicle.toggleVehicleMod(car, 18, on)
    end)
    tuning:toggle("Xenon headlights", vehicle.isToggleModOn(car, 22), function(on)
        vehicle.toggleVehicleMod(car, 22, on)
    end)
    tuning:separator("Available parts")
    for slot = 0, 49 do
        if slot ~= 18 and slot ~= 22 then
            local n = vehicle.getNumVehicleMods(car, slot)
            if n > 0 then
                local options = { "Stock" }
                for i = 0, n - 1 do
                    table.insert(options, "Part " .. tostring(i + 1))
                end
                local current = vehicle.getVehicleMod(car, slot)
                tuning:choice("Mod slot " .. tostring(slot), options, math.clamp(current + 2, 1, #options), function(i)
                    if i == 1 then
                        vehicle.removeVehicleMod(car, slot)
                    else
                        vehicle.setVehicleMod(car, slot, i - 2, false)
                    end
                end)
            end
        end
    end
end)

root:button("Check current vehicle", { note = "Confirm the mod can see your vehicle", action = "Check" }, function()
    local car = currentVehicle()
    if car then
        ui.notify("Car Customizer is ready for this vehicle")
    end
end)

ui.notify("Car Customizer loaded — open F7 > MENUS > Car Customizer")
