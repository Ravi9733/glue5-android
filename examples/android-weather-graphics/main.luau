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

-- Uses game timecycle presets only; no external textures or shader packs.
local graphicsMenu = ui.menu("Colorful Graphics")
graphicsMenu:label("Natural, richer colours - lightweight")
local colourPresets = {
    { name = "Natural Vibrant", modifier = "rply_saturation", strength = 0.65 },
    { name = "Rich Colours", modifier = "MP_corona_heist", strength = 0.45 },
    { name = "Warm Natural", modifier = "BarryFadeOut", strength = 0.25 },
}
graphicsMenu:choice("Colour preset", { "Natural Vibrant", "Rich Colours", "Warm Natural", "Original" }, 1, function(index)
    local ok = pcall(function()
        if index == 4 then
            graphics.clearTimecycleModifier()
        else
            local preset = colourPresets[index]
            graphics.setTimecycleModifier(preset.modifier)
            graphics.setTimecycleModifierStrength(preset.strength)
        end
    end)
    ui.notify(ok and (index == 4 and "Original colours restored" or ("Applied: " .. colourPresets[index].name)) or "Colour effect unavailable in this game build")
end)
graphicsMenu:slider("Colour strength", 0.65, 0.0, 1.0, 0.05, function(strength)
    local ok = pcall(function()
        graphics.setTimecycleModifierStrength(strength)
    end)
    if not ok then
        ui.notify("Colour strength native unavailable")
    end
end)
graphicsMenu:button("Reset colours", function()
    local ok = pcall(function()
        graphics.clearTimecycleModifier()
    end)
    ui.notify(ok and "Original colours restored" or "Colour reset native unavailable")
end)

ui.notify("Weather, Time Changer and Colorful Graphics loaded.")
