--!strict
-- PC Trainer V feature adapter for Glue 5 Android.
-- This is a Luau port of selected trainer features, NOT a Windows .asi loader.
-- Install folder: game/mods/pc-trainer-v-android/
-- Keep autostart enabled so the touch menu is registered at game start.

local menu = ui.menu("PC Trainer V - Android")
menu:label("PLAYER")
menu:toggle("Invincibility", false, function(enabled)
    local ok = pcall(function()
        local ped = player.playerPedId()
        entity.setEntityInvincible(ped, enabled)
    end)
    ui.notify(ok and (enabled and "Invincibility ON" or "Invincibility OFF")
        or "Invincibility native unavailable")
end)
menu:button("Heal player", function()
    local ok = pcall(function()
        entity.setEntityHealth(player.playerPedId(), 200)
    end)
    ui.notify(ok and "Player healed" or "Heal failed / native unavailable")
end)
menu:button("Restore armour", function()
    local ok = pcall(function()
        player.setPlayerMaxArmour(player.playerId(), 100)
        player.setPlayerArmour(player.playerId(), 100)
    end)
    ui.notify(ok and "Armour restored" or "Armour native unavailable")
end)

menu:label("WANTED LEVEL")
menu:slider("Wanted level", 0, 0, 5, 1, function(level)
    local ok = pcall(function()
        player.setPlayerWantedLevel(player.playerId(), level, false)
        player.setPlayerWantedLevelNow(player.playerId(), false)
    end)
    if not ok then ui.notify("Wanted level native unavailable") end
end)
menu:button("Clear wanted level", function()
    local ok = pcall(function()
        player.setPlayerWantedLevel(player.playerId(), 0, false)
        player.setPlayerWantedLevelNow(player.playerId(), false)
    end)
    ui.notify(ok and "Wanted level cleared" or "Wanted level native unavailable")
end)

menu:label("VEHICLE")
menu:button("Repair current vehicle", function()
    local ok = pcall(function()
        local ped = player.playerPedId()
        local veh = ped.getVehiclePedIsIn(ped, false)
        if not veh or veh == 0 then error("Not in a vehicle") end
        vehicle.setVehicleFixed(veh)
        vehicle.setVehicleDirtLevel(veh, 0.0)
    end)
    ui.notify(ok and "Vehicle repaired" or "Enter a vehicle first / native unavailable")
end)

menu:label("TIME")
local times = {
    { name = "Morning", h = 8 }, { name = "Noon", h = 12 },
    { name = "Sunset", h = 19 }, { name = "Night", h = 22 },
}
for _, t in ipairs(times) do
    menu:button("Time: " .. t.name, function()
        local ok = pcall(function()
            clock.setClockTime(t.h, 0, 0)
            clock.pauseClock(true)
        end)
        ui.notify(ok and ("Time set: " .. t.name) or "Clock native unavailable")
    end)
end
menu:button("Resume natural time", function()
    local ok = pcall(function() clock.pauseClock(false) end)
    ui.notify(ok and "Natural time resumed" or "Clock native unavailable")
end)

menu:label("WEATHER")
local weatherTypes = { "EXTRASUNNY", "CLEAR", "CLOUDS", "OVERCAST", "RAIN", "THUNDER", "FOGGY" }
menu:choice("Weather", weatherTypes, 1, function(index)
    local ok = pcall(function() misc.setWeatherTypeNowPersist(weatherTypes[index]) end)
    ui.notify(ok and ("Weather: " .. weatherTypes[index]) or "Weather native unavailable")
end)
menu:button("Reset weather", function()
    local ok = pcall(function() misc.clearWeatherTypePersist() end)
    ui.notify(ok and "Weather reset" or "Weather reset native unavailable")
end)

ui.notify("PC Trainer V Android adapter loaded")
