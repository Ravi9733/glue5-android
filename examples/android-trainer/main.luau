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

ui.notify("Android Trainer loaded. Open Mods > MENUS (touch menu button or F7).")
