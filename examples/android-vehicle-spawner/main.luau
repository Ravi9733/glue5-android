--!strict
-- Android vehicle spawner for Glue 5.
-- Each category has its own menu and model list. Native hash parameters
-- accept model names directly, so no separate joaat/hash conversion is needed.

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
    local pedOk, playerPed = pcall(function()
        return player.playerPedId()
    end)
    if not pedOk or not playerPed or playerPed == 0 then
        ui.notify("Spawn failed: player is not ready")
        return
    end

    local requestOk, requestError = pcall(function()
        streaming.requestModel(modelName)
    end)
    if not requestOk then
        ui.notify("Spawn failed at model request: " .. modelName .. " (" .. tostring(requestError) .. ")")
        return
    end

    local loaded = false
    for _ = 1, 600 do
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
        ui.notify("Spawn failed: model not available in this game build: " .. modelName)
        return
    end

    local spawnOk, result = pcall(function()
        local pos = entity.getEntityCoords(playerPed, true)
        local heading = entity.getEntityHeading(playerPed)
        local radians = math.rad(heading)
        local x = pos.x - math.sin(radians) * 6.0
        local y = pos.y + math.cos(radians) * 6.0
        local z = pos.z + 1.0

        -- Pass the model name directly; the native bridge hashes Hash parameters.
        local vehicleHandle = vehicle.createVehicle(modelName, x, y, z, heading, true, false)
        if not vehicleHandle or vehicleHandle == 0 then
            return false, "createVehicle returned no vehicle"
        end

        local seatOk, seatError = pcall(function()
            ped.setPedIntoVehicle(playerPed, vehicleHandle, -1)
        end)
        if not seatOk then
            return false, "vehicle created, but entering it failed: " .. tostring(seatError)
        end
        return true, nil
    end)

    pcall(function()
        streaming.setModelAsNoLongerNeeded(modelName)
    end)

    if not spawnOk then
        ui.notify("Spawn failed: " .. modelName .. " (" .. tostring(result) .. ")")
    elseif result == false then
        ui.notify("Spawn failed: " .. modelName .. " (vehicle could not be created)")
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

ui.notify("Vehicle Spawner ready. Open Mods > MENUS, then choose Cars, Bikes, Boats or Planes.")
