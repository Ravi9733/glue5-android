--!strict
-- GLUE5_ANDROID_PHOTOREALISTIC_LIGHT_V2
-- Lightweight time, weather and built-in filmic graphics presets for Glue 5 Android.
-- Uses GTA V timecycle modifiers; this is not a custom shader/texture overhaul.

local weatherTypes = {
    "EXTRASUNNY", "CLEAR", "CLOUDS", "OVERCAST", "SMOG",
    "FOGGY", "CLEARING", "RAIN", "THUNDER", "SNOW",
    "BLIZZARD", "SNOWLIGHT", "XMAS", "HALLOWEEN",
}

local weatherMenu = ui.menu("Weather")
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

local timeMenu = ui.menu("Time")
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

-- Built-in GTA V timecycle grades; Photorealistic is a lightweight filmic
-- colour grade, not a custom lighting, geometry or texture shader.
local graphicsMenu = ui.menu("Graphics")
graphicsMenu:label("Choose a lightweight visual preset")
local presets = {
    { name = "Original", modifier = "", strength = 0.0 },
    { name = "Light Graphics", modifier = "NG_filmic01", strength = 0.30 },
    { name = "Photorealistic", modifier = "NG_filmic01", strength = 0.65 },
    { name = "Warm Natural", modifier = "cinema", strength = 0.60 },
}
local activeIndex = 1
local activeStrength = presets[activeIndex].strength
local effectsEnabled = false

local function clearStyle(): boolean
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

graphicsMenu:toggle("Graphics Effects ON/OFF", false, function(enabled)
    effectsEnabled = enabled
    local ok
    if enabled then
        ok = applyStyle(activeIndex, activeStrength)
    else
        ok = clearStyle()
    end
    ui.notify(ok and (enabled and ("Applied: " .. presets[activeIndex].name) or "Graphics effects off")
        or "Graphics native unavailable")
end)

graphicsMenu:choice("Visual Style", {
    "Original", "Light Graphics", "Photorealistic", "Warm Natural",
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
    ui.notify(ok and ("Applied: " .. presets[index].name) or "Style native unavailable")
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

ui.notify("Time, Weather and Graphics loaded")
