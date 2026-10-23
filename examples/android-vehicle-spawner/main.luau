--!strict
-- Standalone Android-compatible vehicle spawner for Glue 5.
-- Uses GTA V natives exposed by the active game.wasm build.

local menu = ui.menu("Vehicle Spawner")
local models = {
    "adder",
    "zentorno",
    "t20",
    "turismor",
    "entityxf",
    "cheetah",
    "infernus",
    "comet2",
    "elegy",
    "buffalo",
    "sultan",
    "dominator",
    "bati",
    "sanchez",
    "police",
    "police2",
    "ambulance",
    "blazer",
}
local selected = 1

menu:label("Choose a vehicle and tap Spawn")
menu:choice("Vehicle model", models, selected, function(index)
    selected = index
    ui.notify("Selected: " .. models[selected])
end)

menu:button("Spawn vehicle", "Load and spawn beside the player", function()
    local pedHandle = player.playerPedId()
    if not pedHandle then
        ui.notify("Player is not ready yet")
        return
    end

    local modelName = models[selected]
    local modelHash = joaat(modelName)
    local requested, requestError = pcall(function()
        streaming.requestModel(modelHash)
    end)
    if not requested then
        ui.notify("Model request failed: " .. modelName)
        return
    end

    local loaded = false
    for _ = 1, 300 do
        local ok, result = pcall(function()
            return streaming.hasModelLoaded(modelHash)
        end)
        if ok and result then
            loaded = true
            break
        end
        wait(0)
    end

    if not loaded then
        ui.notify("Model did not load: " .. modelName)
        pcall(function()
            streaming.setModelAsNoLongerNeeded(modelHash)
        end)
        return
    end

    local ok, spawned = pcall(function()
        local pos = entity.getEntityCoords(pedHandle, true)
        local heading = entity.getEntityHeading(pedHandle)
        local car = vehicle.createVehicle(
            modelHash,
            pos.x + 3.0,
            pos.y,
            pos.z,
            heading,
            true,
            false
        )
        if not car then
            return false
        end
        ped.setPedIntoVehicle(pedHandle, car, -1)
        return true
    end)

    pcall(function()
        streaming.setModelAsNoLongerNeeded(modelHash)
    end)

    if ok and spawned then
        ui.notify("Spawned: " .. modelName)
    else
        ui.notify("Spawn failed: " .. modelName)
    end
end)

ui.notify("Vehicle Spawner loaded. Open Mods > MENUS.")
