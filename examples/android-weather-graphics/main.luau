--!strict
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

-- Lightweight timecycle-only colour controls. No replacement textures or
-- additional shaders, to keep the performance cost low on Android.
local graphicsMenu = ui.menu("Colorful Graphics")
graphicsMenu:label("Apply a preset, then adjust its strength")
local colourPresets = {
    { name = "Natural Vibrant", modifier = "rply_saturation", strength = 0.55 },
    { name = "Rich Colours", modifier = "MP_corona_heist", strength = 0.35 },
    { name = "Warm Natural", modifier = "cinema", strength = 0.20 },
}
local activePreset = 1
local activeStrength = colourPresets[1].strength
local function applyColourPreset(index, strength)
    if index == 4 then
        local ok = pcall(function()
            graphics.clearTimecycleModifier()
        end)
        return ok
    end
    local preset = colourPresets[index]
    local ok = pcall(function()
        graphics.setTimecycleModifier(preset.modifier)
        graphics.setTimecycleModifierStrength(strength)
    end)
    return ok
end

graphicsMenu:choice("Colour preset", { "Natural Vibrant", "Rich Colours", "Warm Natural", "Original" }, 1, function(index)
    if index == 4 then
        local ok = applyColourPreset(index, 0.0)
        ui.notify(ok and "Original colours restored" or "Graphics reset native unavailable")
        return
    end
    activePreset = index
    activeStrength = colourPresets[index].strength
    local ok = applyColourPreset(index, activeStrength)
    ui.notify(ok and ("Applied: " .. colourPresets[index].name) or "Colour effect failed; check mod log")
end)
graphicsMenu:slider("Colour strength", activeStrength, 0.0, 1.0, 0.05, function(strength)
    activeStrength = strength
    local ok = applyColourPreset(activePreset, activeStrength)
    if not ok then
        ui.notify("Colour strength failed; check mod log")
    end
end)
graphicsMenu:button("Reset colours", function()
    local ok = pcall(function()
        graphics.clearTimecycleModifier()
    end)
    activePreset = 1
    activeStrength = colourPresets[1].strength
    ui.notify(ok and "Original colours restored" or "Graphics reset native unavailable")
end)

ui.notify("Weather, Time Changer and Colorful Graphics loaded.")
