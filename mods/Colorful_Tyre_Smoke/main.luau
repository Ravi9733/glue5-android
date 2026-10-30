-- Colorful Tyre Smoke
-- Menu: F7 > MENUS > Colorful Tyre Smoke.
-- Uses GTA V's tyre-smoke colour native. Smoke density and particle lifetime
-- are controlled by the game and are not changed by this mod.

local menu = ui.menu("Colorful Tyre Smoke", "Colourful tyre smoke for cars and bikes")

local colours = {
    { name = "White",  r = 255, g = 255, b = 255 },
    { name = "Red",    r = 255, g = 35,  b = 35  },
    { name = "Blue",   r = 35,  g = 90,  b = 255 },
    { name = "Green",  r = 35,  g = 255, b = 70  },
    { name = "Purple", r = 160, g = 45,  b = 255 },
    { name = "Pink",   r = 255, g = 65,  b = 180 },
    { name = "Yellow", r = 255, g = 230, b = 25  },
    { name = "Orange", r = 255, g = 120, b = 20  },
    { name = "Cyan",   r = 20,  g = 230, b = 255 },
}

local colourNames = {}
for _, colour in colours do
    table.insert(colourNames, colour.name)
end

local selectedName = "White"
local rainbowOn = false
local enabled = true

local function colourByName(name)
    for _, colour in colours do
        if colour.name == name then
            return colour
        end
    end
    return colours[1]
end

local function currentVehicle()
    local me = player.playerPedId()
    if not ped.isPedInAnyVehicle(me, false) then
        return nil
    end

    local veh = ped.getVehiclePedIsIn(me, false)
    if not veh or veh == 0 then
        return nil
    end
    return veh
end

local function applyColour(colour)
    local veh = currentVehicle()
    if not veh then
        return false
    end

    vehicle.toggleVehicleMod(veh, 20, true)
    vehicle.setVehicleTyreSmokeColor(veh, colour.r, colour.g, colour.b)
    return true
end

local function rainbowColour()
    local t = os.clock() * 2.4
    return {
        r = math.floor((math.sin(t) * 0.5 + 0.5) * 255),
        g = math.floor((math.sin(t + 2.094) * 0.5 + 0.5) * 255),
        b = math.floor((math.sin(t + 4.188) * 0.5 + 0.5) * 255),
    }
end

-- GLUE 5 menu callbacks use the documented argument positions:
-- choice(label, options, default, callback)
-- toggle(label, default, callback)
menu:choice("Smoke colour", colourNames, "White", function(_, name)
    selectedName = name
    rainbowOn = false
    if enabled then
        applyColour(colourByName(selectedName))
    end
end)

menu:toggle("Rainbow mode", false, function(on)
    rainbowOn = on
end)

menu:toggle("Enable smoke colour", true, function(on)
    enabled = on
    if on and not rainbowOn then
        applyColour(colourByName(selectedName))
    end
end)

menu:button("Apply Colour", {
    note = "Apply the selected colour to your current vehicle",
    action = "Apply",
}, function()
    rainbowOn = false
    if applyColour(colourByName(selectedName)) then
        ui.notify(selectedName .. " tyre smoke applied")
    else
        ui.notify("Get into a car or bike first")
    end
end)

menu:button("Disable Rainbow", {
    note = "Stop cycling rainbow colours",
    action = "Disable",
}, function()
    rainbowOn = false
    ui.notify("Rainbow disabled")
end)

menu:label("Smoke density and duration are controlled by GTA V")

-- Open the actual menu page when this mod starts, rather than relying only
-- on the player finding it in the MENUS list.
menu:show()

while true do
    if enabled then
        local colour = if rainbowOn then rainbowColour() else colourByName(selectedName)
        applyColour(colour)
    end
    wait(250)
end
