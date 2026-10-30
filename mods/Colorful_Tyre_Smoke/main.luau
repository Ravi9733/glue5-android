--!strict
-- Colorful Tyre Smoke
-- Open F7 > MENUS > Colorful Tyre Smoke.

-- GLUE 5 documents ui.menu(title): only one argument.
local menu = ui.menu("Colorful Tyre Smoke")

local colours = {
    { name = "White", r = 255, g = 255, b = 255 },
    { name = "Red", r = 255, g = 35, b = 35 },
    { name = "Blue", r = 35, g = 90, b = 255 },
    { name = "Green", r = 35, g = 255, b = 70 },
    { name = "Purple", r = 160, g = 45, b = 255 },
    { name = "Pink", r = 255, g = 65, b = 180 },
    { name = "Yellow", r = 255, g = 230, b = 25 },
    { name = "Orange", r = 255, g = 120, b = 20 },
    { name = "Cyan", r = 20, g = 230, b = 255 },
}
local names = {}
for _, colour in colours do
    table.insert(names, colour.name)
end

-- Use the documented menu item values instead of callbacks for choices/toggles.
local selected = menu:choice("Smoke colour", names, "White", "Choose a colour")
local rainbow = menu:toggle("Rainbow", false, "Cycle through rainbow colours")
local enabled = menu:toggle("Enable smoke colour", true, "Apply colour while driving")

local function currentVehicle()
    local me = player.playerPedId()
    if not ped.isPedInAnyVehicle(me, false) then
        ui.notify("Get into a car or bike first")
        return nil
    end
    local veh = ped.getVehiclePedIsIn(me, false)
    if not veh or veh == 0 then return nil end
    return veh
end

local function colourByName(name)
    for _, colour in colours do
        if colour.name == name then return colour end
    end
    return colours[1]
end

menu:button("Apply Colour", { note = "Apply the selected colour now", action = "Apply" }, function()
    local veh = currentVehicle()
    if not veh then return end
    local colour = colourByName(selected.option)
    vehicle.toggleVehicleMod(veh, 20, true)
    vehicle.setVehicleTyreSmokeColor(veh, colour.r, colour.g, colour.b)
    ui.notify(colour.name .. " tyre smoke applied")
end)

menu:button("Disable Rainbow", { note = "Stop rainbow cycling", action = "Disable" }, function()
    rainbow.value = false
    ui.notify("Rainbow disabled")
end)

local function rainbowColour()
    local t = os.clock() * 2.4
    return {
        r = math.floor((math.sin(t) * 0.5 + 0.5) * 255),
        g = math.floor((math.sin(t + 2.094) * 0.5 + 0.5) * 255),
        b = math.floor((math.sin(t + 4.188) * 0.5 + 0.5) * 255),
    }
end

while true do
    if enabled.value then
        local me = player.playerPedId()
        if ped.isPedInAnyVehicle(me, false) then
            local veh = ped.getVehiclePedIsIn(me, false)
            if veh and veh ~= 0 then
                local colour = if rainbow.value then rainbowColour() else colourByName(selected.option)
                vehicle.toggleVehicleMod(veh, 20, true)
                vehicle.setVehicleTyreSmokeColor(veh, colour.r, colour.g, colour.b)
            end
        end
    end
    wait(250)
end
