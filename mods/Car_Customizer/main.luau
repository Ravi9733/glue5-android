-- Car Customizer for GLUE 5 Android / GTA V
-- Standalone mod. Menu API signatures follow GLUE 5's bundled Glue Trainer.

local root = ui.menu("Car Customizer", "Paint, built-in liveries, neon, tyre smoke and vehicle parts")

local function safe(label, fn)
    return function(...)
        local ok, err = pcall(fn, ...)
        if not ok then
            print(label .. ": " .. tostring(err))
            ui.notify(label .. ": " .. tostring(err))
        end
    end
end

local function rgb(r, g, b)
    return { r = r, g = g, b = b, a = 255 }
end

local toggleSlots = { [18] = true, [22] = true }
local slotNames = {
    [0] = "Spoiler", "Front bumper", "Rear bumper", "Side skirt", "Exhaust", "Frame",
    "Grille", "Hood", "Left fender", "Right fender", "Roof", "Engine", "Brakes",
    "Transmission", "Horn", "Suspension", "Armour", "Nitrous", "Turbo", "Subwoofer",
    "Tyre smoke", "Hydraulics", "Xenon lights", "Wheels", "Rear wheels", "Plate holder",
    "Vanity plate", "Trim", "Ornaments", "Dashboard", "Dial", "Door speakers", "Seats",
    "Steering wheel", "Shift lever", "Plaques", "Speakers", "Trunk", "Hydraulics",
    "Engine block", "Air filter", "Struts", "Arch cover", "Aerials", "Trim", "Tank",
    "Windows", "Window", "Livery"
}
local wheelTypes = { "Sport", "Muscle", "Lowrider", "SUV", "Offroad", "Tuner", "Bike", "High end" }
local tintNames = { "None", "Pure black", "Dark smoke", "Light smoke", "Stock", "Limo", "Green" }

local function currentVehicle()
    local me = player.playerPedId()
    if not ped.isPedInAnyVehicle(me, false) then
        return nil
    end
    return ped.getVehiclePedIsIn(me, false)
end

local function indexed(menu, label, count, current, nameFor, apply)
    if not count or count <= 0 then
        menu:label(label .. ": not supported on this vehicle")
        return
    end
    local options = {}
    for i = 0, count - 1 do
        table.insert(options, nameFor(i))
    end
    menu:choice(label, options, math.clamp(current + 1, 1, count), safe(label, function(index)
        apply(index - 1)
    end))
end

local function build(menu, car)
    vehicle.setVehicleModKit(car, 0)

    menu:button("Upgrade everything", { note = "Apply the best available parts", action = "Upgrade" }, safe("Upgrade", function()
        for slot = 0, 48 do
            if toggleSlots[slot] then
                vehicle.toggleVehicleMod(car, slot, true)
            else
                local count = vehicle.getNumVehicleMods(car, slot)
                if count and count > 0 then
                    vehicle.setVehicleMod(car, slot, count - 1, false)
                end
            end
        end
        vehicle.setVehicleWindowTint(car, 1)
        ui.notify("Available upgrades applied")
    end))

    menu:separator("Custom paint")
    menu:color("Primary paint", rgb(vehicle.getVehicleCustomPrimaryColour(car)), safe("Primary paint", function(c)
        vehicle.setVehicleCustomPrimaryColour(car, c.r, c.g, c.b)
    end))
    menu:color("Secondary paint", rgb(vehicle.getVehicleCustomSecondaryColour(car)), safe("Secondary paint", function(c)
        vehicle.setVehicleCustomSecondaryColour(car, c.r, c.g, c.b)
    end))

    menu:separator("Neon and tyre smoke")
    local neon = menu:toggle("Neon underglow", vehicle.getVehicleNeonEnabled(car, 0), safe("Neon", function(on)
        for side = 0, 3 do
            vehicle.setVehicleNeonEnabled(car, side, on)
        end
    end))
    menu:color("Neon colour", rgb(vehicle.getVehicleNeonColour(car)), safe("Neon colour", function(c)
        vehicle.setVehicleNeonColour(car, c.r, c.g, c.b)
        if not neon.value then
            for side = 0, 3 do vehicle.setVehicleNeonEnabled(car, side, true) end
            neon.value = true
        end
    end))
    menu:color("Tyre-smoke colour", rgb(vehicle.getVehicleTyreSmokeColor(car)), safe("Tyre smoke", function(c)
        vehicle.toggleVehicleMod(car, 20, true)
        vehicle.setVehicleTyreSmokeColor(car, c.r, c.g, c.b)
    end))

    menu:separator("Stickers, stripes and appearance")
    indexed(menu, "Built-in livery / decals / stripes", vehicle.getVehicleLiveryCount(car), vehicle.getVehicleLivery(car), function(i)
        return "Design " .. tostring(i + 1)
    end, function(i)
        vehicle.setVehicleLivery(car, i)
    end)
    indexed(menu, "Window tint", vehicle.getNumVehicleWindowTints(), vehicle.getVehicleWindowTint(car), function(i)
        return tintNames[i] or ("Tint " .. tostring(i))
    end, function(i)
        vehicle.setVehicleWindowTint(car, i)
    end)
    indexed(menu, "Number-plate style", vehicle.getNumberOfVehicleNumberPlates(), vehicle.getVehicleNumberPlateTextIndex(car), function(i)
        return "Plate style " .. tostring(i + 1)
    end, function(i)
        vehicle.setVehicleNumberPlateTextIndex(car, i)
    end)
    menu:text("Number plate text", string.gsub(vehicle.getVehicleNumberPlateText(car), "%s+$", ""), { max = 8 }, safe("Number plate", function(value)
        vehicle.setVehicleNumberPlateText(car, value)
    end))

    menu:separator("Vehicle extras")
    local anyExtra = false
    for id = 1, 14 do
        if vehicle.doesExtraExist(car, id) then
            anyExtra = true
            menu:toggle("Extra " .. tostring(id), vehicle.isVehicleExtraTurnedOn(car, id), safe("Extra " .. tostring(id), function(on)
                -- The native's argument means disable, so invert the menu value.
                vehicle.setVehicleExtra(car, id, not on)
            end))
        end
    end
    if not anyExtra then menu:label("No extras on this vehicle") end

    menu:separator("Wheels and tuning")
    menu:choice("Wheel type", wheelTypes, math.clamp(vehicle.getVehicleWheelType(car) + 1, 1, #wheelTypes), safe("Wheel type", function(index)
        vehicle.setVehicleWheelType(car, index - 1)
    end))

    for slot = 0, 48 do
        if toggleSlots[slot] then
            if slot == 18 or slot == 22 then
                local label = slotNames[slot] or ("Slot " .. tostring(slot))
                menu:toggle(label, vehicle.isToggleModOn(car, slot), safe(label, function(on)
                    vehicle.toggleVehicleMod(car, slot, on)
                end))
            end
        else
            local count = vehicle.getNumVehicleMods(car, slot)
            if count and count > 0 then
                local label = vehicle.getModSlotName(car, slot)
                if not label or label == "" then label = slotNames[slot] or ("Slot " .. tostring(slot)) end
                local options = { "Stock" }
                for i = 0, count - 1 do
                    local partName = vehicle.getModTextLabel(car, slot, i)
                    table.insert(options, (partName and partName ~= "") and partName or (label .. " " .. tostring(i + 1)))
                end
                menu:choice(label, options, math.clamp(vehicle.getVehicleMod(car, slot) + 2, 1, #options), safe(label, function(index)
                    if index == 1 then
                        vehicle.removeVehicleMod(car, slot)
                    else
                        vehicle.setVehicleMod(car, slot, index - 2, false)
                    end
                end))
            end
        end
    end
end

-- Populate the registered F7 menu when it opens, for the vehicle currently being driven.
root:onOpen(safe("Car Customizer", function()
    root:clear()
    local car = currentVehicle()
    if not car then
        root:label("Get into a vehicle, then reopen Car Customizer.")
        return
    end
    build(root, car)
end))

ui.notify("Car Customizer loaded. Open F7 > MENUS.")
