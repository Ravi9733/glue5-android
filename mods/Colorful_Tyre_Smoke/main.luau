-- Colorful Tyre Smoke
-- F7 > MENUS > Colorful Tyre Smoke.
-- Colours tyre smoke on the current car or bike.
-- Smoke density and particle lifetime are controlled by GTA V.

local menu = ui.menu("Colorful Tyre Smoke", "Colourful smoke for cars and bikes")

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

local names = {}
for _, c in colours do
    table.insert(names, c.name)
end

local selectedName = "White"
local rainbowOn = false
local enabled = true

local function colourByName(name)
    for _, c in colours do
        if c.name == name then return c end
    end
    return colours[1]
end

local function currentVehicle()
    local me = player.playerPedId()
    if not ped.isPedInAnyVehicle(me, false) then return nil end
    local veh = ped.getVehiclePedIsIn(me, false)
    if not veh or veh == 0 then return nil end
    return veh
end

local function applyColour(c)
    local veh = currentVehicle()
    if not veh then return false end
    vehicle.toggleVehicleMod(veh, 20, true)
    vehicle.setVehicleTyreSmokeColor(veh, c.r, c.g, c.b)
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

-- IMPORTANT: GLUE 5 expects the choice default as a 1-based numeric index.
-- Passing "White" here causes the usize conversion error reported by the user.
menu:choice("Smoke colour", names, 1, function(index, name)
    selectedName = name or names[index] or "White"
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
    note = "Apply selected colour to the current car or bike",
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
    note = "Stop rainbow colour cycling",
    action = "Disable",
}, function()
    rainbowOn = false
    ui.notify("Rainbow disabled")
end)

menu:label("Smoke density and duration are game-controlled")

-- The menu is registered on the Mods page; show it when the mod starts.
menu:show()

while true do
    if enabled then
        local c = if rainbowOn then rainbowColour() else colourByName(selectedName)
        applyColour(c)
    end
    wait(250)
end
