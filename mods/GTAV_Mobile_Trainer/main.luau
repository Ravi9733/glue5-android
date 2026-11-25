--!strict
-- GTAV Mobile Trainer (GLUE 5 port, first functional slice)
-- F7 / touch Mods button > MENUS > GTAV Mobile Trainer.
-- This uses GLUE 5's in-process Luau/native bridge; it does not inject
-- the original Fuzzy-specific Android agent.

local menu = ui.menu("GTAV Mobile Trainer", "Vehicle spawning · GLUE 5 native bridge")

local models = {
    "adder",
    "zentorno",
    "t20",
    "sultan",
    "kuruma",
    "bati",
    "insurgent",
}

local selected = 1
local busy = false

menu:choice("Vehicle model", models, selected, function(index)
    selected = index
end)

menu:button("Spawn selected vehicle", {
    note = "Loads the model, then spawns it a short distance from the player",
    action = "Spawn",
}, function()
    if busy then
        ui.notify("Vehicle spawn is already running")
        return
    end

    busy = true
    local model = models[selected]
    local ok, err = pcall(function()
        local me = player.playerPedId()
        if not me then
            error("Player ped is not ready")
        end

        streaming.requestModel(model)
        local deadline = os.clock() + 10
        while not streaming.hasModelLoaded(model) do
            if os.clock() >= deadline then
                error("Timed out loading model: " .. model)
            end
            wait(50)
        end

        local pos = entity.getEntityCoords(me, true)
        local car = vehicle.createVehicle(
            model,
            pos.x + 3,
            pos.y,
            pos.z,
            0,
            false,
            false
        )

        if not car then
            error("Game did not create the vehicle")
        end

        streaming.setModelAsNoLongerNeeded(model)
        ui.notify("Spawned " .. model)
    end)

    busy = false
    if not ok then
        ui.notify("Spawn failed: " .. tostring(err))
        print("Vehicle spawn error:", err)
    end
end)

menu:label("Initial port: vehicle spawning only")
menu:label("Teleport, weapons and OpenIV/DLC are separate targets")

ui.notify("GTAV Mobile Trainer loaded")
