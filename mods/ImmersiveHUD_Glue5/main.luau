--!strict
local menu = ui.menu("Immersive HUD", "Glue 5 port")
local showHealth = true
menu:toggle("Health", true, function(on)
    showHealth = on
end)
menu:toggle("Armour", true, function(on)
    showArmour = on
end)
local showArmour = true
while true do
    local me = player.playerPedId()
    if me then
        if showHealth then
            draw.text("HEALTH " .. tostring(entity.getEntityHealth(me)), 0.82, 0.16, 0.022, 0x76E6C8FF)
        end
        if showArmour then
            draw.text("ARMOUR " .. tostring(ped.getPedArmour(me)), 0.82, 0.19, 0.022, 0x79B8FFFF)
        end
    end
    wait()
end
