--!strict
-- Glue 5/Luau rewrite inspired by ImmersiveHUD and ExtraHUD.
local menu = ui.menu("Immersive HUD", "HUD controls and extra information")

local showHealth = true
local showArmour = true
local showVehicleInfo = true
local hideHud = false
local hideRadar = false

menu:toggle("Hide original HUD", false, "Hide the game HUD while enabled", function(on)
    hideHud = on
end)
menu:toggle("Hide radar", false, "Hide or restore the minimap", function(on)
    hideRadar = on
    hud.displayRadar(not on)
end)
menu:separator("Extra HUD")
menu:toggle("Health", true, function(on)
    showHealth = on
end)
menu:toggle("Armour", true, function(on)
    showArmour = on
end)
menu:toggle("Vehicle speed and engine", true, function(on)
    showVehicleInfo = on
end)
menu:button("Reset HUD", { action = "Reset" }, function()
    showHealth = true
    showArmour = true
    showVehicleInfo = true
    hideHud = false
    hideRadar = false
    hud.displayRadar(true)
    ui.notify("HUD settings reset")
end)

mod.onStop(function()
    pcall(hud.displayRadar, true)
end)

while true do
    if hideHud then
        ui.hideHud()
    end

    local me = player.playerPedId()
    if me then
        local y = 0.15
        if showHealth then
            draw.text("HEALTH  " .. tostring(entity.getEntityHealth(me)), 0.82, y, 0.022, 0x76E6C8FF)
            y += 0.03
        end
        if showArmour then
            draw.text("ARMOUR  " .. tostring(ped.getPedArmour(me)), 0.82, y, 0.022, 0x79B8FFFF)
            y += 0.03
        end
        if showVehicleInfo then
            local car = ped.getVehiclePedIsIn(me, false)
            if car then
                local speed = math.floor(entity.getEntitySpeed(car) * 3.6 + 0.5)
                local engine = math.floor(vehicle.getVehicleEngineHealth(car) + 0.5)
                draw.text("SPEED  " .. tostring(speed) .. " km/h", 0.82, y, 0.022, 0xFFD166FF)
                y += 0.03
                draw.text("ENGINE  " .. tostring(engine), 0.82, y, 0.022, 0xFF9F80FF)
            end
        end
    end
    wait()
end
