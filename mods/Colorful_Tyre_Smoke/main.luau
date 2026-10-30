-- Colorful Tyre Smoke
-- Open F7 > MENUS > Colorful Tyre Smoke.

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
for _, c in colours do
    table.insert(names, c.name)
end

local selectedName = "White"
local rainbowOn = false
local enabled = true

local function getColour(name)
    for _, c in colours do
        if c.name == name then return c end
    end
    return colours[1]
end

local function applyToCurrentVehicle(c)
    local me = player.playerPedId()
    if not ped.isPedInAnyVehicle(me, false) then return false end
    local veh = ped.getVehiclePedIsIn(me, false)
    if not veh or veh == 0 then return false end
    vehicle.toggleVehicleMod(veh, 20, true)
    vehicle.setVehicleTyreSmokeColor(veh, c.r, c.g, c.b)
    return true
end

menu:choice("Smoke colour", names, "White", function(_, name)
    selectedName = name
    rainbowOn = false
    if enabled then
        applyToCurrentVehicle(getColour(selectedName))
    end
end)

menu:toggle("Rainbow mode", false, function(on)
    rainbowOn = on
end)

menu:toggle("Enable smoke colour", true, function(on)
    enabled = on
    if on and not rainbowOn then
        applyToCurrentVehicle(getColour(selectedName))
    end
end)

menu:button("Apply Colour", { action = "Apply" }, function()
    rainbowOn = false
    if not applyToCurrentVehicle(getColour(selectedName)) then
        ui.notify("Get into a car or bike first")
    else
        ui.notify(selectedName .. " tyre smoke applied")
    end
end)

menu:button("Disable Rainbow", { action = "Disable" }, function()
    rainbowOn = false
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
    if enabled then
        local c = if rainbowOn then rainbowColour() else getColour(selectedName)
        applyToCurrentVehicle(c)
    end
    wait(250)
end
