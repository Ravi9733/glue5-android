--!strict
-- GLUE5_ANDROID_VEHICLE_SPAWNER_V3
-- Single-menu Android vehicle spawner for Glue 5.
-- GTA natives take CREATE_VEHICLE coordinates as separate numeric arguments,
-- not as a vector. Model names are hashed by the native bridge.

local models = {
    "adder", "autarch", "banshee2", "blista", "buffalo", "comet2",
    "dominator", "elegy", "futo", "sultan", "zentorno",
    "baller", "cavalcade", "dubsta", "granger", "patriot",
    "bmx", "bati", "akuma", "sanchez",
    "benson", "phantom", "sandking", "guardian",
    "ambulance", "firetruk", "police", "police2", "sheriff", "riot",
}

local function spawnVehicleImpl(modelName: string)
    local playerPed = player.playerPedId()
    if not playerPed or playerPed == 0 then
        ui.notify("Spawn failed: player is not ready")
        return
    end

    local requestOk, requestErr = pcall(function()
        streaming.requestModel(modelName)
    end)
    if not requestOk then
        ui.notify("Could not request " .. modelName .. ": " .. tostring(requestErr))
        return
    end

    local loaded = false
    for _ = 1, 300 do
        local ok, result = pcall(function()
            return streaming.hasModelLoaded(modelName)
        end)
        if ok and result then
            loaded = true
            break
        end
        wait(0)
    end

    if not loaded then
        pcall(function() streaming.setModelAsNoLongerNeeded(modelName) end)
        ui.notify("Model did not load: " .. modelName)
        return
    end

    local ok, createdOrError = pcall(function()
        local pos = entity.getEntityCoords(playerPed, true)
        local heading = entity.getEntityHeading(playerPed)
        -- CREATE_VEHICLE expects x, y, z, heading as separate arguments.
        local vehicleHandle = vehicle.createVehicle(
            modelName, pos.x + 3.0, pos.y, pos.z, heading, true, false
        )
        if not vehicleHandle or vehicleHandle == 0 then
            error("CREATE_VEHICLE returned no vehicle")
        end
        -- Entering is best-effort; a vehicle can still be spawned if this fails.
        pcall(function()
            ped.setPedIntoVehicle(playerPed, vehicleHandle, -1)
        end)
        return vehicleHandle
    end)

    pcall(function() streaming.setModelAsNoLongerNeeded(modelName) end)

    if not ok then
        ui.notify("Spawn failed for " .. modelName .. ": " .. tostring(createdOrError))
    else
        ui.notify("Spawned " .. modelName)
    end
end

local spawning = false
local function spawnSelected(modelName: string)
    if spawning then
        ui.notify("Please wait for the current vehicle to finish loading")
        return
    end
    spawning = true
    local ok, err = pcall(function()
        spawnVehicleImpl(modelName)
    end)
    spawning = false
    if not ok then
        ui.notify("Vehicle spawn failed: " .. tostring(err))
    end
end

-- Exactly one vehicle selector and one spawn button, without duplicate category buttons.
local menu = ui.menu("Vehicle Spawner")
menu:label("Select a model, then tap Spawn Vehicle")
local selectedIndex = 1
menu:choice("Vehicle Model", models, selectedIndex, function(index)
    selectedIndex = index
end)
menu:button("Spawn Vehicle", function()
    spawnSelected(models[selectedIndex] or models[1])
end)

ui.notify("Vehicle Spawner loaded")
