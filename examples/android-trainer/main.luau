--!strict
-- Copy this folder to game/mods/android-trainer/.
-- Uses Glue 5's own Luau/native bridge, not PC ScriptHookVDotNet or DLL mods.

local menu = ui.menu("Android Trainer")

menu:toggle("Invincibility", false, function(enabled)
    local ped = player.playerPedId()
    if ped then
        entity.setEntityInvincible(ped, enabled)
        ui.notify(enabled and "Invincibility enabled" or "Invincibility disabled")
    end
end)

menu:slider("Wanted level", 0, 0, 5, 1, function(level)
    local id = player.playerId()
    player.setPlayerWantedLevel(id, level, false)
    player.setPlayerWantedLevelNow(id, false)
end)

menu:button("Heal player", function()
    local ped = player.playerPedId()
    if ped then
        entity.setEntityHealth(ped, 200)
        ui.notify("Player healed")
    end
end)

menu:button("Clear wanted level", function()
    local id = player.playerId()
    player.setPlayerWantedLevel(id, 0, false)
    player.setPlayerWantedLevelNow(id, false)
    ui.notify("Wanted level cleared")
end)

menu:button("Spawn Adder", "Creates a car beside the player", function()
    local ped = player.playerPedId()
    if not ped then
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
        return
    end

    local pos = entity.getEntityCoords(ped, true)
    local heading = entity.getEntityHeading(ped)
    local car = vehicle.createVehicle(model, pos.x + 3, pos.y, pos.z, heading, true, false)
    if car then
        ped.setPedIntoVehicle(ped, car, -1)
        ui.notify("Adder spawned")
    else
        ui.notify("Could not spawn car")
    end
    streaming.setModelAsNoLongerNeeded(model)
end)

ui.notify("Android Trainer loaded. Open the Mods menu (touch menu button or F7).")
