--!strict
-- Lightweight GTA V weather and colour controls for Glue 5 Android.
-- Weather and timecycle calls use the game's own native functions.

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

local presets = {
    "rply_saturation",
    "MP_corona_heist",
    "BarryFadeOut",
    "default",
}
local graphicsMenu = ui.menu("Colorful Graphics")
graphicsMenu:label("Lightweight in-game colour grading")
graphicsMenu:choice("Colour preset", presets, 1, function(index)
    local preset = presets[index]
    local ok = pcall(function()
        if preset == "default" then
            graphics.clearTimecycleModifier()
        else
            graphics.setTimecycleModifier(preset)
            graphics.setTimecycleModifierStrength(1.0)
        end
    end)
    ui.notify(ok and ("Colour preset: " .. preset) or "Timecycle native unavailable in this game build")
end)
graphicsMenu:slider("Colour strength", 1.0, 0.0, 1.5, 0.1, function(strength)
    local ok = pcall(function()
        graphics.setTimecycleModifierStrength(strength)
    end)
    if not ok then
        ui.notify("Colour strength native unavailable")
    end
end)
ui.notify("Weather Changer and Colorful Graphics loaded.")
