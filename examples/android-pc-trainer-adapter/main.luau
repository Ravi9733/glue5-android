--!strict
-- PC Trainer V Android feature adapter for Glue 5.
-- This is a native-backed Luau port of selected trainer features, NOT a Windows ASI loader.
-- Install this ZIP from Glue 5 Studio's Install Mod page; then enable it in Mods.
-- Keep autostart enabled so the touch menu is registered at game start.

local menu = ui.menu("PC Trainer V - Android")
menu:label("ANDROID PORT · SUPPORTED FEATURES")

local function report(ok: boolean, success: string, failure: string)
    ui.notify(ok and success or failure)
end

menu:label("PLAYER")
menu:toggle("Invincibility", false, function(enabled)
    local ok = pcall(function()
        entity.setEntityInvincible(player.playerPedId(), enabled)
    end)
    report(ok, enabled and "Invincibility ON" or "Invincibility OFF",
        "Invincibility unavailable in this game build")
end)
menu:button("Heal player", function()
    local ok = pcall(function()
        entity.setEntityHealth(player.playerPedId(), 200)
    end)
    report(ok, "Player healed", "Heal unavailable in this game build")
end)
menu:button("Restore armour", function()
    local ok = pcall(function()
        player.setPlayerMaxArmour(player.playerId(), 100)
        player.setPlayerArmour(player.playerId(), 100)
    end)
    report(ok, "Armour restored", "Armour natives unavailable in this game build")
end)

menu:label("WANTED LEVEL")
menu:slider("Wanted level", 0, 0, 5, 1, function(level)
    local ok = pcall(function()
        player.setPlayerWantedLevel(player.playerId(), level, false)
        player.setPlayerWantedLevelNow(player.playerId(), false)
    end)
    if not ok then ui.notify("Wanted-level native unavailable") end
end)
menu:button("Clear wanted level", function()
    local ok = pcall(function()
        player.setPlayerWantedLevel(player.playerId(), 0, false)
        player.setPlayerWantedLevelNow(player.playerId(), false)
    end)
    report(ok, "Wanted level cleared", "Wanted-level natives unavailable")
end)

menu:label("VEHICLE")
menu:button("Repair current vehicle", function()
    local ok, err = pcall(function()
        local ped = player.playerPedId()
        local veh = ped.getVehiclePedIsIn(ped, false)
        if not veh or veh == 0 then error("Enter a vehicle first") end
        vehicle.setVehicleFixed(veh)
        vehicle.setVehicleDirtLevel(veh, 0.0)
    end)
    report(ok, "Vehicle repaired", tostring(err or "Repair unavailable"))
end)

menu:label("TIME")
local times = {
    { name = "Morning", hour = 8 },
    { name = "Noon", hour = 12 },
    { name = "Sunset", hour = 19 },
    { name = "Night", hour = 22 },
}
for _, t in times do
    menu:button("Time: " .. t.name, function()
        local ok = pcall(function()
            clock.setClockTime(t.hour, 0, 0)
            clock.pauseClock(true)
        end)
        report(ok, "Time set: " .. t.name, "Clock native unavailable")
    end)
end
menu:button("Resume natural time", function()
    local ok = pcall(function() clock.pauseClock(false) end)
    report(ok, "Natural time resumed", "Clock native unavailable")
end)

menu:label("WEATHER")
local weatherTypes = { "EXTRASUNNY", "CLEAR", "CLOUDS", "OVERCAST", "RAIN", "THUNDER", "FOGGY" }
menu:choice("Weather", weatherTypes, 2, function(index)
    local ok = pcall(function()
        misc.setWeatherTypeNowPersist(weatherTypes[index])
    end)
    report(ok, "Weather: " .. weatherTypes[index], "Weather native unavailable")
end)
menu:button("Reset weather", function()
    local ok = pcall(function() misc.clearWeatherTypePersist() end)
    report(ok, "Weather reset", "Weather reset native unavailable")
end)

ui.notify("PC Trainer V Android adapter loaded")
