--!strict
-- Glue 5 Android-compatible trainer.
-- Copy this folder to game/mods/android-trainer/.
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

menu:label("VEHICLES")
menu:button("Spawn Adder", "Creates a car beside the player", function()
    local playerPed = player.playerPedId()
    if not playerPed then
        ui.notify("Player is not ready")
        return
    end

    local model = "adder"
    streaming.requestModel(model)
    local tries = 0
    while not streaming.hasModelLoaded(model) and tries < 300 do
        wait()
        tries += 1
    end

    if not streaming.hasModelLoaded(model) then
        ui.notify("Car model did not load")
        streaming.setModelAsNoLongerNeeded(model)
        return
    end

    local pos = entity.getEntityCoords(playerPed, true)
    local heading = entity.getEntityHeading(playerPed)
    local car = vehicle.createVehicle(model, pos.x + 3, pos.y, pos.z, heading, true, false)
    if car then
        -- Use the ped namespace; playerPed is only the player's entity handle.
        ped.setPedIntoVehicle(playerPed, car, -1)
        ui.notify("Adder spawned")
    else
        ui.notify("Could not spawn car")
    end
    streaming.setModelAsNoLongerNeeded(model)
end)

ui.notify("Android Trainer loaded. Open Mods > MENUS (touch menu button or F7).")
