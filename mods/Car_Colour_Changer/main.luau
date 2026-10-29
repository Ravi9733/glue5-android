--!strict
-- Car Colour Changer for GLUE 5 Android.
-- Built-in RGB colour picker for the vehicle the player is currently in.

local menu = ui.menu("Car Colour Changer", "Custom RGB paint for your current car")

local primary = { r = 220, g = 20, b = 60 }
local secondary = { r = 20, g = 20, b = 20 }

local function currentVehicle()
    local me = player.playerPedId()
    if not ped.isPedInAnyVehicle(me, false) then
        ui.notify("Get inside the car first, then choose a colour")
        return nil
    end
    local car = ped.getVehiclePedIsIn(me, false)
    if not car then
        ui.notify("Could not find your current car")
        return nil
    end
    return car
end

local function applyPrimary(c)
    primary = { r = c.r, g = c.g, b = c.b }
    local car = currentVehicle()
    if car then
        vehicle.setVehicleCustomPrimaryColour(car, primary.r, primary.g, primary.b)
        ui.notify("Primary colour applied")
    end
end

local function applySecondary(c)
    secondary = { r = c.r, g = c.g, b = c.b }
    local car = currentVehicle()
    if car then
        vehicle.setVehicleCustomSecondaryColour(car, secondary.r, secondary.g, secondary.b)
        ui.notify("Secondary colour applied")
    end
end

menu:color("Primary Colour", "#dc143c", { alpha = false }, applyPrimary)
menu:color("Secondary Colour", "#141414", { alpha = false }, applySecondary)

menu:button("Apply Both Colours", { note = "Apply both selected colours to your current car", action = "Apply" }, function()
    local car = currentVehicle()
    if car then
        vehicle.setVehicleCustomPrimaryColour(car, primary.r, primary.g, primary.b)
        vehicle.setVehicleCustomSecondaryColour(car, secondary.r, secondary.g, secondary.b)
        ui.notify("Both colours applied")
    end
end)

menu:button("Random Colours", { note = "Random primary and secondary paint", action = "Random" }, function()
    local car = currentVehicle()
    if car then
        primary = { r = math.random(0, 255), g = math.random(0, 255), b = math.random(0, 255) }
        secondary = { r = math.random(0, 255), g = math.random(0, 255), b = math.random(0, 255) }
        vehicle.setVehicleCustomPrimaryColour(car, primary.r, primary.g, primary.b)
        vehicle.setVehicleCustomSecondaryColour(car, secondary.r, secondary.g, secondary.b)
        ui.notify("Random colours applied")
    end
end)

ui.notify("Car Colour Changer menu loaded")
