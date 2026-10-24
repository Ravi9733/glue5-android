--!strict
-- GLUE5_ANDROID_WEATHER_COLOR_V3
-- Lightweight GTA V weather, time and natural colour controls for Glue 5 Android.

local weatherTypes = {
    "EXTRASUNNY", "CLEAR", "CLOUDS", "OVERCAST", "SMOG",
    "FOGGY", "CLEARING", "RAIN", "THUNDER", "SNOW",
    "BLIZZARD", "SNOWLIGHT", "XMAS", "HALLOWEEN",
}

local weatherMenu = ui.menu("Weather Changer")
weatherMenu:label("Choose a weather type")
weatherMenu:choice("Weather", weatherTypes, 1, function(index)
    local weather = weatherTypes[index]
    local ok = pcall(function()
        misc.setWeatherTypeNowPersist(weather)
    end)
    if not ok then
        ok = pcall(function()
            misc.setWeatherTypeNow(weather)
        end)
    end
    ui.notify(ok and ("Weather: " .. weather) or "Weather native unavailable in this game build")
end)
weatherMenu:button("Reset weather", function()
    local ok = pcall(function()
        misc.clearWeatherTypePersist()
    end)
    ui.notify(ok and "Weather override cleared" or "Weather reset native unavailable")
end)

-- Clock menu: preset buttons pause the clock so the selected time stays put.
local timeMenu = ui.menu("Time Changer")
local timePresets = {
    { name = "Morning", hour = 8, minute = 0 },
    { name = "Noon", hour = 12, minute = 0 },
    { name = "Afternoon", hour = 16, minute = 0 },
    { name = "Sunset", hour = 19, minute = 0 },
    { name = "Night", hour = 22, minute = 0 },
    { name = "Midnight", hour = 0, minute = 0 },
}
timeMenu:label("Choose the in-game time")
for _, preset in ipairs(timePresets) do
    timeMenu:button(preset.name, function()
        local ok = pcall(function()
            clock.setClockTime(preset.hour, preset.minute, 0)
            clock.pauseClock(true)
        end)
        ui.notify(ok and ("Time set: " .. preset.name) or "Clock native unavailable in this game build")
    end)
end
timeMenu:button("Resume natural time", function()
    local ok = pcall(function()
        clock.pauseClock(false)
    end)
    ui.notify(ok and "Natural game time resumed" or "Clock resume unavailable")
end)

-- Lightweight timecycle presets use GTA V's built-in post-processing;
-- no texture packs or extra shaders are loaded, to protect frame pacing.
local graphicsMenu = ui.menu("Colorful Graphics")
graphicsMenu:label("Apply a preset, then adjust its strength")
local colourPresets = {
    { name = "Vibrant", modifier = "rply_saturation", strength = 1.0 },
    { name = "Rich Colours", modifier = "MP_corona_heist", strength = 0.85 },
    { name = "Warm Natural", modifier = "cinema", strength = 0.7 },
}
local activePreset = 1
local activeStrength = colourPresets[1].strength
local colourfulEnabled = true

local function clearColours()
    return pcall(function()
        graphics.clearTimecycleModifier()
        graphics.setTimecycleModifierStrength(0.0)
    end)
end

local function applyColourPreset(index, strength)
    local preset = colourPresets[index]
    if not preset then return false end
    local ok = pcall(function()
        graphics.setTimecycleModifier(preset.modifier)
        graphics.setTimecycleModifierStrength(strength)
    end)
    return ok
end

graphicsMenu:toggle("Colour effects ON/OFF", true, function(enabled)
    colourfulEnabled = enabled
    if enabled then
        local ok = applyColourPreset(activePreset, activeStrength)
        ui.notify(ok and ("Applied: " .. colourPresets[activePreset].name) or "Colour native failed; see mod log")
    else
        local ok = clearColours()
        ui.notify(ok and "Colour effects off" or "Could not reset colour effect")
    end
end)

graphicsMenu:choice("Colour preset", { "Vibrant", "Rich Colours", "Warm Natural", "Original" }, 1, function(index)
    if index == 4 then
        colourfulEnabled = false
        local ok = clearColours()
        ui.notify(ok and "Original colours restored" or "Could not reset colour effect")
        return
    end
    activePreset = index
    activeStrength = colourPresets[index].strength
    colourfulEnabled = true
    local ok = applyColourPreset(activePreset, activeStrength)
    ui.notify(ok and ("Applied: " .. colourPresets[index].name) or "Colour preset failed; see mod log")
end)

graphicsMenu:slider("Colour strength", activeStrength, 0.0, 1.0, 0.05, function(strength)
    activeStrength = strength
    if colourfulEnabled then
        local ok = applyColourPreset(activePreset, activeStrength)
        if not ok then ui.notify("Colour strength native failed") end
    end
end)

graphicsMenu:button("Reset colours", function()
    local ok = clearColours()
    colourfulEnabled = false
    activePreset = 1
    activeStrength = colourPresets[1].strength
    ui.notify(ok and "Original colours restored" or "Could not reset colour effect")
end)

-- Fixed destinations for quick testing; only moves the player when ready.
local teleportMenu = ui.menu("Teleport")
local destinations = {
    { name = "Airport", x = -1034.6, y = -2733.6, z = 13.8 },
    { name = "City Centre", x = 215.8, y = -810.1, z = 30.7 },
    { name = "Mount Chiliad", x = 450.7, y = 5566.3, z = 806.2 },
    { name = "Sandy Shores", x = 1853.2, y = 3686.4, z = 34.3 },
    { name = "Beach", x = -1390.0, y = -584.0, z = 30.2 },
}
teleportMenu:label("Teleport to a saved location")
for _, destination in ipairs(destinations) do
    teleportMenu:button(destination.name, function()
        local ok, err = pcall(function()
            local playerPed = player.playerPedId()
            if not playerPed or playerPed == 0 then error("Player is not ready") end
            entity.setEntityCoords(playerPed, vector.create(destination.x, destination.y, destination.z), false, false, false, false)
        end)
        ui.notify(ok and ("Teleported: " .. destination.name) or ("Teleport failed: " .. tostring(err)))
    end)
end

ui.notify("Weather, Time, Colorful Graphics and Teleport menus loaded.")