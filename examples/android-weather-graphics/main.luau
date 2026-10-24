--!strict
-- GLUE5_ANDROID_WEATHER_COLOR_V4
-- Android weather, time and lightweight graphics controls for Glue 5.

local weatherTypes = {
    "EXTRASUNNY", "CLEAR", "CLOUDS", "OVERCAST", "SMOG",
    "FOGGY", "CLEARING", "RAIN", "THUNDER", "SNOW",
    "BLIZZARD", "SNOWLIGHT", "XMAS", "HALLOWEEN",
}

local weatherMenu = ui.menu("Weather Changer")
weatherMenu:label("Choose a weather type")
weatherMenu:choice("Weather", weatherTypes, 1, function(index)
    local weather = weatherTypes[index]
    local ok = pcall(function() misc.setWeatherTypeNowPersist(weather) end)
    if not ok then
        ok = pcall(function() misc.setWeatherTypeNow(weather) end)
    end
    ui.notify(ok and ("Weather: " .. weather) or "Weather native unavailable")
end)
weatherMenu:button("Reset weather", function()
    local ok = pcall(function() misc.clearWeatherTypePersist() end)
    ui.notify(ok and "Weather reset" or "Weather reset native unavailable")
end)

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
        ui.notify(ok and ("Time set: " .. preset.name) or "Clock native unavailable")
    end)
end
timeMenu:button("Resume natural time", function()
    local ok = pcall(function() clock.pauseClock(false) end)
    ui.notify(ok and "Natural time resumed" or "Clock resume unavailable")
end)

-- These are built-in GTA timecycle colour grades, not custom shaders.
-- Anime/Cartoon presets boost colour and stylisation; true cel outlines require a shader.
local graphicsMenu = ui.menu("Graphics Style")
graphicsMenu:label("Choose a built-in look and set its strength")
local presets = {
    { name = "Original", modifier = "", strength = 0.0 },
    { name = "Vibrant", modifier = "rply_saturation", strength = 1.0 },
    { name = "Warm Natural", modifier = "cinema", strength = 0.7 },
    { name = "Bloom Style", modifier = "MP_corona_heist", strength = 0.75 },
    { name = "Film Style", modifier = "NG_filmic01", strength = 0.8 },
    { name = "Cinematic Film", modifier = "cinema_2", strength = 0.75 },
    { name = "Anime Color", modifier = "rply_saturation", strength = 1.0 },
    { name = "Cartoon Pop", modifier = "MP_corona_tournament", strength = 0.85 },
}
local activeIndex = 1
local activeStrength = presets[activeIndex].strength
local effectsEnabled = false

local function clearStyle()
    return pcall(function()
        graphics.clearTimecycleModifier()
        graphics.setTimecycleModifierStrength(0.0)
    end)
end

local function applyStyle(index: number, strength: number): boolean
    local preset = presets[index]
    if not preset then return false end
    if preset.modifier == "" then
        return clearStyle()
    end
    return pcall(function()
        graphics.setTimecycleModifier(preset.modifier)
        graphics.setTimecycleModifierStrength(strength)
    end)
end

graphicsMenu:toggle("Graphics Effects ON/OFF", true, function(enabled)
    effectsEnabled = enabled
    local ok
    if enabled then
        ok = applyStyle(activeIndex, activeStrength)
    else
        ok = clearStyle()
    end
    ui.notify(ok and (enabled and ("Applied: " .. presets[activeIndex].name) or "Graphics effects off")
        or "Graphics native failed; check the mod log")
end)

graphicsMenu:choice("Visual Style", {
    "Original", "Vibrant", "Warm Natural", "Bloom Style", "Film Style", "Cinematic Film",
    "Anime Color", "Cartoon Pop",
}, activeIndex, function(index)
    activeIndex = index
    activeStrength = presets[index].strength
    if index == 1 then
        effectsEnabled = false
        local ok = clearStyle()
        ui.notify(ok and "Original graphics restored" or "Could not reset graphics")
        return
    end
    effectsEnabled = true
    local ok = applyStyle(activeIndex, activeStrength)
    ui.notify(ok and ("Applied: " .. presets[index].name) or "Style native failed")
end)

graphicsMenu:slider("Effect Strength", activeStrength, 0.0, 1.0, 0.05, function(strength)
    activeStrength = strength
    if effectsEnabled then
        local ok = applyStyle(activeIndex, activeStrength)
        if not ok then ui.notify("Could not change effect strength") end
    end
end)

graphicsMenu:button("Reset Graphics", function()
    local ok = clearStyle()
    effectsEnabled = false
    activeIndex = 1
    activeStrength = 0.0
    ui.notify(ok and "Original graphics restored" or "Could not reset graphics")
end)

ui.notify("Weather, Time and Graphics loaded")

