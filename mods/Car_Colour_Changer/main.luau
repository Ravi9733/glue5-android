--!strict
-- Car Colour Changer for GLUE 5 Android.
-- Uses the documented menu button API so the options remain visible on touch devices.

local root = ui.menu("Car Colour Changer", "Paint colours for the car you are driving")

local function currentVehicle()
    local me = player.playerPedId()
    if not ped.isPedInAnyVehicle(me, false) then
        ui.notify("Get inside a car first")
        return nil
    end
    local car = ped.getVehiclePedIsIn(me, false)
    if not car then
        ui.notify("Could not find your current car")
        return nil
    end
    return car
end

local function paint(label: string, r: number, g: number, b: number)
    local car = currentVehicle()
    if not car then return end
    vehicle.setVehicleCustomPrimaryColour(car, r, g, b)
    ui.notify(label .. " primary paint applied")
end

local function paintBoth(label: string, r: number, g: number, b: number, sr: number, sg: number, sb: number)
    local car = currentVehicle()
    if not car then return end
    vehicle.setVehicleCustomPrimaryColour(car, r, g, b)
    vehicle.setVehicleCustomSecondaryColour(car, sr, sg, sb)
    ui.notify(label .. " paint applied")
end

root:button("Red", { note = "Primary paint: red", action = "Apply" }, function() paint("Red", 220, 20, 60) end)
root:button("Blue", { note = "Primary paint: blue", action = "Apply" }, function() paint("Blue", 30, 90, 220) end)
root:button("Green", { note = "Primary paint: green", action = "Apply" }, function() paint("Green", 20, 180, 70) end)
root:button("Yellow", { note = "Primary paint: yellow", action = "Apply" }, function() paint("Yellow", 255, 210, 20) end)
root:button("Purple", { note = "Primary paint: purple", action = "Apply" }, function() paint("Purple", 150, 50, 210) end)
root:button("Orange", { note = "Primary paint: orange", action = "Apply" }, function() paint("Orange", 255, 110, 10) end)
root:button("White", { note = "Primary paint: white", action = "Apply" }, function() paint("White", 245, 245, 245) end)
root:button("Black", { note = "Primary paint: black", action = "Apply" }, function() paint("Black", 10, 10, 10) end)
root:button("Pink", { note = "Primary paint: pink", action = "Apply" }, function() paint("Pink", 255, 80, 170) end)
root:button("Cyan", { note = "Primary paint: cyan", action = "Apply" }, function() paint("Cyan", 20, 220, 220) end)
root:button("Random Colours", { note = "Random primary and secondary paint", action = "Random" }, function()
    paintBoth("Random", math.random(0, 255), math.random(0, 255), math.random(0, 255),
        math.random(0, 255), math.random(0, 255), math.random(0, 255))
end)
root:button("Restore Default Red", { note = "Restore red primary and black secondary", action = "Reset" }, function()
    paintBoth("Default", 220, 20, 60, 20, 20, 20)
end)

ui.notify("Car Colour Changer loaded — open MENUS and select it")
