--!strict
-- RealityV Car Colour Changer for GLUE 5 Android.
-- Uses GLUE 5's built-in colour picker and vehicle colour natives.

local menu = ui.menu("Car Colour Changer", "Pick any colour for the car you are driving")

local primary = { r = 220, g = 20, b = 60 }
local secondary = { r = 20, g = 20, b = 20 }

local function currentVehicle()
    local me = player.playerPedId()
    local car = vehicle.getVehiclePedIsIn(me, false)
    if not car or car == 0 then
        ui.notify("Get inside a car first")
        return nil
    end
    return car
end

local function applyPrimary(c)
    primary = c
    local car = currentVehicle()
    if car then
        vehicle.setVehicleCustomPrimaryColour(car, c.r, c.g, c.b)
        ui.notify("Primary car colour applied")
    end
end

local function applySecondary(c)
    secondary = c
    local car = currentVehicle()
    if car then
        vehicle.setVehicleCustomSecondaryColour(car, c.r, c.g, c.b)
        ui.notify("Secondary car colour applied")
    end
end

menu:color("Primary Colour", "#dc143c", { alpha = false }, applyPrimary)
menu:color("Secondary Colour", "#141414", { alpha = false }, applySecondary)

menu:button("Apply Both Colours", { note = "Apply selected colours to the car you are in", action = "Apply" }, function()
    local car = currentVehicle()
    if car then
        vehicle.setVehicleCustomPrimaryColour(car, primary.r, primary.g, primary.b)
        vehicle.setVehicleCustomSecondaryColour(car, secondary.r, secondary.g, secondary.b)
        ui.notify("Both car colours applied")
    end
end)

menu:button("Random Colours", { note = "Choose random primary and secondary colours", action = "Random" }, function()
    local car = currentVehicle()
    if car then
        primary = { r = math.random(0, 255), g = math.random(0, 255), b = math.random(0, 255) }
        secondary = { r = math.random(0, 255), g = math.random(0, 255), b = math.random(0, 255) }
        vehicle.setVehicleCustomPrimaryColour(car, primary.r, primary.g, primary.b)
        vehicle.setVehicleCustomSecondaryColour(car, secondary.r, secondary.g, secondary.b)
        ui.notify("Random car colours applied")
    end
end)

ui.notify("Car Colour Changer loaded")
