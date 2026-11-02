-- Menyoo-style touch vehicle spawner for GLUE 5 Android.
-- Uses the game's own vehicle-model catalog and native script bridge.

local models = world.vehicleModels()
table.sort(models)

local pageSize = 28
local page = 1
local selectedModel = models[1]

local function spawnSelected()
    if not selectedModel then
        ui.notify("No vehicle model is available")
        return
    end

    local hash = joaat(selectedModel)
    if not streaming.isModelInCdimage(hash) or not streaming.isModelAVehicle(hash) then
        ui.notify("Vehicle model is not available: " .. selectedModel)
        return
    end

    streaming.requestModel(hash)
    local deadline = os.clock() + 10
    while not streaming.hasModelLoaded(hash) and os.clock() < deadline do
        wait(50)
    end

    if not streaming.hasModelLoaded(hash) then
        ui.notify("Model load timed out: " .. selectedModel)
        streaming.setModelAsNoLongerNeeded(hash)
        return
    end

    local pedId = player.playerPedId()
    local pos = entity.getOffsetFromEntityInWorldCoords(pedId, 0.0, 5.0, 0.0)
    local heading = entity.getEntityHeading(pedId)
    local spawned = vehicle.createVehicle(hash, pos.x, pos.y, pos.z, heading, false, false)

    if spawned and spawned ~= 0 then
        ped.setPedIntoVehicle(pedId, spawned, -1)
        ui.notify("Spawned " .. selectedModel)
    else
        ui.notify("Vehicle spawn failed: " .. selectedModel)
    end

    streaming.setModelAsNoLongerNeeded(hash)
end

local function buildMenu()
    local menu = ui.menu("Menyoo Vehicle Spawner")
    local pageCount = math.max(1, math.ceil(#models / pageSize))
    local first = (page - 1) * pageSize + 1
    local last = math.min(#models, first + pageSize - 1)
    local options = {}

    for i = first, last do
        table.insert(options, models[i])
    end

    if #options > 0 then
        local selectedIndex = 1
        for i, name in options do
            if name == selectedModel then
                selectedIndex = i
                break
            end
        end
        menu:choice("Vehicle model", options, selectedIndex, function(index, name)
            selectedModel = name or options[index]
        end)
    else
        menu:label("No vehicle models found in the loaded game")
    end

    menu:label("Page " .. page .. "/" .. pageCount .. " · " .. #models .. " vehicles")

    menu:button("Spawn selected vehicle", "Spawn ahead and enter the vehicle", function()
        spawnSelected()
    end)

    menu:button("Previous page", "Browse earlier vehicle models", function()
        if page > 1 then
            page = page - 1
            buildMenu()
        end
    end)

    menu:button("Next page", "Browse more vehicle models", function()
        if page < pageCount then
            page = page + 1
            buildMenu()
        end
    end)
end

if #models == 0 then
    ui.notify("Vehicle catalog is empty; start after the game world loads")
end

buildMenu()
