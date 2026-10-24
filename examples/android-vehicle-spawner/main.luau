--!strict
-- GLUE5_ANDROID_VEHICLE_SPAWNER_V2
-- Android vehicle spawner for Glue 5.
-- This is a Luau/native-bridge mod, not a PC .asi/.dll trainer.

local categories = {
    {
        title = "Cars",
        models = { "blista", "buffalo", "comet2", "dominator", "elegy", "futo", "oracle", "premier", "sultan", "tailgater", "vigero", "zion" },
    },
    {
        title = "Bikes",
        models = { "akuma", "bati", "bati2", "carbonrs", "daemon", "double", "hakuchou", "nemesis", "pcj", "ruffian", "sanchez", "vader" },
    },
    {
        title = "Bicycles",
        models = { "bmx", "cruiser", "fixter", "scorcher", "tribike", "tribike2", "tribike3" },
    },
    {
        title = "SUVs",
        models = { "baller", "baller2", "cavalcade", "dubsta", "granger", "huntley", "landstalker", "mesa", "patriot", "radi", "seminole", "xls" },
    },
    {
        title = "Trucks",
        models = { "benson", "biff", "hauler", "mule", "phantom", "pounder", "barracks", "guardian", "sandking", "sandking2", "bobcatxl", "towtruck" },
    },
    {
        title = "Supercars",
        models = { "adder", "autarch", "banshee2", "bullet", "cheetah", "entityxf", "fmj", "infernus", "osiris", "reaper", "t20", "tempesta", "turismor", "tyrus", "vacca", "zentorno" },
    },
    {
        title = "Emergency Vehicles",
        models = { "ambulance", "firetruk", "police", "police2", "police3", "police4", "policeb", "policet", "pranger", "sheriff", "sheriff2", "riot", "fbi", "fbi2" },
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

-- One Vehicles menu with a separate model selector and spawn button per category.
-- Keeping all vehicle categories in one menu avoids cluttering the Mods menu.
local vehiclesMenu = ui.menu("Vehicle Spawner")
vehiclesMenu:label("Choose a category model, then press its Spawn button")

local selectedModels = {}
for _, category in ipairs(categories) do
    selectedModels[category.title] = 1
    vehiclesMenu:choice(category.title, category.models, 1, function(index)
        selectedModels[category.title] = index
    end)
    vehiclesMenu:button("Spawn " .. category.title, "Spawn the selected " .. category.title:lower():sub(1, -2), function()
        local index = selectedModels[category.title] or 1
        spawnVehicle(category.models[index])
    end)
end

