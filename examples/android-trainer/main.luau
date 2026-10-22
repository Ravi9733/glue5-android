--!strict
-- Glue 5 Android-compatible trainer.
-- Install as game/mods/android-trainer/main.luau with mod.json {"autostart": true}.
-- This is a Luau/native-bridge mod, not a PC .asi/.dll trainer.

local menu = ui.menu("Android Trainer")

menu:label("PLAYER")
menu:toggle("Invincibility", false, function(enabled)
    local playerPed = player.playerPedId()
    if playerPed then
        entity.setEntityInvincible(playerPed, enabled)
        ui.notify(enabled and "Invincibility enabled" or "Invincibility disabled")
    else
        ui.notify("Player is not ready")
    end
end)

menu:button("Heal player", function()
    local playerPed = player.playerPedId()
    if playerPed then
        entity.setEntityHealth(playerPed, 200)
        ui.notify("Player healed")
    else
        ui.notify("Player is not ready")
    end
end)

menu:label("WANTED LEVEL")
menu:slider("Wanted level", 0, 0, 5, 1, function(level)
    local playerId = player.playerId()
    player.setPlayerWantedLevel(playerId, level, false)
    player.setPlayerWantedLevelNow(playerId, false)
end)

menu:button("Clear wanted level", function()
    local playerId = player.playerId()
    player.setPlayerWantedLevel(playerId, 0, false)
    player.setPlayerWantedLevelNow(playerId, false)
    ui.notify("Wanted level cleared")
end)

menu:label("VEHICLE SPAWNER")
local vehicleModels = {
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
local selectedVehicle = 1

menu:choice("Vehicle model", vehicleModels, selectedVehicle, function(index)
    selectedVehicle = index
    ui.notify("Selected: " .. vehicleModels[selectedVehicle])
end)

menu:button("Spawn selected vehicle", "Loads the selected model near the player", function()
    local playerPed = player.playerPedId()
    if not playerPed then
        ui.notify("Player is not ready")
        return
    end

    local model = vehicleModels[selectedVehicle]
    local requested = pcall(function()
        streaming.requestModel(model)
    end)
    if not requested then
        ui.notify("Model request failed: " .. model)
        return
    end

    local tries = 0
    while tries < 300 do
        local ok, loaded = pcall(function()
            return streaming.hasModelLoaded(model)
        end)
        if ok and loaded then
            break
        end
        wait()
        tries += 1
    end

    local ok, loaded = pcall(function()
        return streaming.hasModelLoaded(model)
    end)
    if not ok or not loaded then
        ui.notify("Model unavailable: " .. model)
        pcall(function()
            streaming.setModelAsNoLongerNeeded(model)
        end)
        return
    end

    local okSpawn, result = pcall(function()
        local pos = entity.getEntityCoords(playerPed, true)
        local heading = entity.getEntityHeading(playerPed)
        local car = vehicle.createVehicle(model, pos.x + 3, pos.y, pos.z, heading, true, false)
        if car then
            ped.setPedIntoVehicle(playerPed, car, -1)
            return true
        end
        return false
    end)

    pcall(function()
        streaming.setModelAsNoLongerNeeded(model)
    end)

    if okSpawn and result then
        ui.notify("Spawned: " .. model)
    else
        ui.notify("Could not spawn: " .. model)
    end
end)

ui.notify("Android Trainer loaded. Open Mods > MENUS (touch menu button or F7).")
