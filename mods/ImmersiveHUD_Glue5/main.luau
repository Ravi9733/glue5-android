--!strict
-- Minimal, robust Glue 5 port inspired by ImmersiveHUD + ExtraHUD.
-- The root menu is deliberately shown on startup so it is easy to find in F7.
local menu = ui.menu("Immersive HUD", "Health, armour and vehicle information")

local showHealth = true
local showArmour = true
local showVehicleInfo = true
local hideRadar = false

menu:toggle("Health display", true, function(on)
    showHealth = on
end)
menu:toggle("Armour display", true, function(on)
    showArmour = on
end)
menu:toggle("Vehicle speed and engine", true, function(on)
    showVehicleInfo = on
end)
menu:toggle("Hide radar", false, function(on)
    hideRadar = on
    hud.displayRadar(not on)
end)
menu:button("Reset HUD", { action = "Reset" }, function()
    showHealth = true
    showArmour = true
    showVehicleInfo = true
    hideRadar = false
    hud.displayRadar(true)
    menu:clear()
    menu:toggle("Health display", true, function(on) showHealth = on end)
    menu:toggle("Armour display", true, function(on) showArmour = on end)
    menu:toggle("Vehicle speed and engine", true, function(on) showVehicleInfo = on end)
    menu:toggle("Hide radar", false, function(on)
        hideRadar = on
        hud.displayRadar(not on)
    end)
    menu:button("Reset HUD", { action = "Reset" }, function()
        ui.notify("HUD is already reset")
    end)
    ui.notify("HUD settings reset")
end)

mod.onStop(function()
    pcall(hud.displayRadar, true)
end)

-- Open the menu once at startup. It should also remain listed under MENUS (F7).
menu:show()

while true do
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
