--!strict
-- Android vehicle spawner and weather/colour controls for Glue 5.
-- This is a Luau/native-bridge mod, not a PC .asi/.dll trainer.

local categories = {
    {
        title = "Cars",
        models = {
            "adder", "zentorno", "t20", "turismor", "entityxf", "cheetah",
            "infernus", "comet2", "elegy", "buffalo", "sultan", "kuruma",
            "dominator", "futo", "baller", "schafter2",
        },
    },
    {
        title = "Bikes",
        models = {
            "bati", "bati2", "akuma", "carbonrs", "daemon", "double",
            "hakuchou", "hexer", "nemesis", "pcj", "ruffian", "sanchez",
            "vader", "lectro",
        },
    },
    {
        title = "Boats",
        models = {
            "dinghy", "dinghy2", "jetmax", "marquis", "seashark",
            "seashark2", "speeder", "squalo", "toro", "tropic", "tug",
        },
    },
    {
        title = "Planes",
        models = {
            "dodo", "duster", "luxor", "shamal", "vestra", "mammatus",
            "cuban800", "velum", "velum2", "besra", "hydra", "lazer",
            "titan",
        },
    },
}

local function spawnVehicle(modelName)
    local playerPed = nil
    local pedOk, pedResult = pcall(function()
        return player.playerPedId()
    end)
    if pedOk then
        playerPed = pedResult
    end
    if not playerPed or playerPed == 0 then
        ui.notify("Spawn failed: player is not ready yet")
        return
    end

    local requestOk = pcall(function()
        streaming.requestModel(modelName)
    end)
    if not requestOk then
        ui.notify("Spawn failed: could not request model " .. modelName)
        return
    end

    local loaded = false
    for _ = 1, 300 do
        local checkOk, result = pcall(function()
            return streaming.hasModelLoaded(modelName)
        end)
        if checkOk and result then
            loaded = true
            break
        end
        wait(0)
    end

    if not loaded then
        pcall(function()
            streaming.setModelAsNoLongerNeeded(modelName)
        end)
        ui.notify("Spawn failed: model not loaded: " .. modelName)
        return
    end

    local spawnOk, vehicleHandle, detail = pcall(function()
        local pos = entity.getEntityCoords(playerPed, true)
        local heading = entity.getEntityHeading(playerPed)
        local spawnPos = vector.create(pos.x + 3.0, pos.y, pos.z)
        local created = vehicle.createVehicle(modelName, spawnPos, heading, true, false)
        if not created or created == 0 then
            return nil, "CREATE_VEHICLE returned an empty handle"
        end
        local entered = pcall(function()
            ped.setPedIntoVehicle(playerPed, created, -1)
        end)
        if not entered then
            return created, "spawned, but could not enter it"
        end
        return created, nil
    end)

    pcall(function()
        streaming.setModelAsNoLongerNeeded(modelName)
    end)

    if not spawnOk then
        ui.notify("Spawn failed: " .. modelName .. " (" .. tostring(vehicleHandle) .. ")")
    elseif not vehicleHandle or vehicleHandle == 0 then
        ui.notify("Spawn failed: " .. modelName .. " (" .. tostring(detail or "vehicle was not created") .. ")")
    elseif detail then
        ui.notify("Spawned: " .. modelName .. " - " .. detail)
    else
        ui.notify("Spawned: " .. modelName)
    end
end

local function createCategoryMenu(category)
    local menu = ui.menu("Spawn " .. category.title)
    local selected = 1
    menu:label(category.title .. " - choose a model")
    menu:choice("Model", category.models, selected, function(index)
        selected = index
        ui.notify(category.title .. ": " .. category.models[selected])
    end)
    menu:button("Spawn " .. category.title, "Load the selected model near the player", function()
        spawnVehicle(category.models[selected])
    end)
end

for _, category in ipairs(categories) do
    createCategoryMenu(category)
end

-- Weather, in-game clock and lightweight colour controls.
local weatherTypes = {
    "EXTRASUNNY", "CLEAR", "CLOUDS", "OVERCAST", "SMOG",
    "FOGGY", "CLEARING", "RAIN", "THUNDER", "SNOW",
    "BLIZZARD", "SNOWLIGHT", "XMAS", "HALLOWEEN",
}
local weatherMenu = ui.menu("Weather Changer")
weatherMenu:label("Select a weather type")
weatherMenu:choice("Weather", weatherTypes, 1, function(index)
    local weather = weatherTypes[index]
    local ok = pcall(function() misc.setWeatherTypeNowPersist(weather) end)
    if not ok then ok = pcall(function() misc.setWeatherTypeNow(weather) end) end
    ui.notify(ok and ("Weather changed: " .. weather) or "Weather native unavailable")
end)
weatherMenu:button("Clear weather override", function()
    local ok = pcall(function() misc.clearWeatherTypePersist() end)
    ui.notify(ok and "Weather override cleared" or "Weather reset unavailable")
end)

local timeMenu = ui.menu("Time Changer")
timeMenu:label("Choose in-game time")
local timePresets = {
    { name = "Morning", hour = 8 },
    { name = "Noon", hour = 12 },
    { name = "Afternoon", hour = 16 },
    { name = "Sunset", hour = 19 },
    { name = "Night", hour = 22 },
    { name = "Midnight", hour = 0 },
}
for _, preset in ipairs(timePresets) do
    timeMenu:button(preset.name, function()
        local ok = pcall(function()
            clock.setClockTime(preset.hour, 0, 0)
            clock.pauseClock(true)
        end)
        ui.notify(ok and ("Time set: " .. preset.name) or "Clock native unavailable")
    end)
end
timeMenu:button("Resume natural time", function()
    local ok = pcall(function() clock.pauseClock(false) end)
    ui.notify(ok and "Natural time resumed" or "Clock resume unavailable")
end)

local colourPresets = {
    { name = "Natural Vibrant", modifier = "rply_saturation", strength = 0.65 },
    { name = "Rich Colours", modifier = "MP_corona_heist", strength = 0.45 },
    { name = "Warm Natural", modifier = "BarryFadeOut", strength = 0.25 },
}
local graphicsMenu = ui.menu("Colorful Graphics")
graphicsMenu:label("Natural colours without extra textures")
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
    ui.notify(ok and (index == 4 and "Original colours restored" or ("Applied: " .. colourPresets[index].name)) or "Colour effect unavailable")
end)
graphicsMenu:slider("Colour strength", 0.65, 0.0, 1.0, 0.05, function(strength)
    local ok = pcall(function() graphics.setTimecycleModifierStrength(strength) end)
    if not ok then ui.notify("Colour strength native unavailable") end
end)
graphicsMenu:button("Reset colours", function()
    local ok = pcall(function() graphics.clearTimecycleModifier() end)
    ui.notify(ok and "Original colours restored" or "Colour reset unavailable")
end)

ui.notify("Vehicle Spawner, Weather, Time and Colorful Graphics loaded.")
