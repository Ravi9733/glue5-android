-- Glue5-native trainer adaptation for the GTA V WASM build.
-- This is a source Lua mod, not a loader for Windows .asi/.dll files.

local weather_options = {
    "CLEAR",
    "EXTRASUNNY",
    "CLOUDS",
    "OVERCAST",
    "SMOG",
    "FOGGY",
    "CLEARING",
    "RAIN",
    "THUNDER",
    "NEUTRAL",
    "SNOW",
    "BLIZZARD",
    "SNOWLIGHT",
    "XMAS",
    "HALLOWEEN",
}

local selected_weather = "CLEAR"
local selected_hour = 12

local menu = ui.menu("Glue5 Trainer")

menu:label("Android / WASM source trainer")
menu:label("Weather and clock controls")

menu:choice("Weather preset", weather_options, 1, function(_, value)
    selected_weather = value
end)

menu:button("Apply weather", function()
    if type(SetWeatherTypeNowPersist) ~= "function" then
        ui.notify("This build has no SetWeatherTypeNowPersist native.")
        return
    end

    local ok, err = pcall(SetWeatherTypeNowPersist, selected_weather)
    if not ok then
        ui.notify("Weather failed: " .. tostring(err))
        return
    end
    ui.notify("Weather set: " .. selected_weather)
end)

menu:slider("Clock hour", 12, 0, 23, 1, function(value)
    selected_hour = math.floor(value + 0.5)
end)

menu:button("Apply clock", function()
    if type(SetClockTime) ~= "function" then
        ui.notify("This build has no SetClockTime native.")
        return
    end

    local ok, err = pcall(SetClockTime, selected_hour, 0, 0)
    if not ok then
        ui.notify("Clock failed: " .. tostring(err))
        return
    end
    ui.notify(string.format("Clock set: %02d:00", selected_hour))
end)

menu:button("Reset to clear noon", function()
    local weather_ok, weather_err = pcall(SetWeatherTypeNowPersist, "CLEAR")
    local clock_ok, clock_err = pcall(SetClockTime, 12, 0, 0)
    if weather_ok and clock_ok then
        selected_weather = "CLEAR"
        selected_hour = 12
        ui.notify("Weather and clock reset.")
    else
        ui.notify("Reset failed: " .. tostring(weather_err or clock_err))
    end
end)

ui.notify("Glue5 Trainer loaded. Open the mod menu to change weather/time.")
