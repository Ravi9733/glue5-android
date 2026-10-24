--!strict
-- GLUE5_ANDROID_IRON_MAN_POWERS_V1
-- Non-combat superhero movement controls for Glue 5.
-- This script does not contain a custom Iron Man character model.

local function getPed()
    local ped = player.playerPedId()
    if not ped or ped == 0 then
        error("Player is not ready")
    end
    return ped
end

local function moveBy(dx: number, dy: number, dz: number, label: string)
    local ok = pcall(function()
        local ped = getPed()
        local pos = entity.getEntityCoords(ped, true)
        entity.setEntityCoords(
            ped,
            pos.x + dx,
            pos.y + dy,
            pos.z + dz,
            false,
            false,
            false,
            false
        )
    end)
    ui.notify(ok and (label .. " activated") or (label .. " unavailable: native call failed"))
end

local menu = ui.menu("Iron Man Powers")
menu:label("Movement powers — no combat abilities")
menu:button("Take Off (+5 m)", function()
    moveBy(0.0, 0.0, 5.0, "Take off")
end)
menu:button("Land (-5 m)", function()
    moveBy(0.0, 0.0, -5.0, "Landing")
end)
menu:button("Forward Boost", function()
    local ok = pcall(function()
        local ped = getPed()
        local pos = entity.getEntityCoords(ped, true)
        local heading = entity.getEntityHeading(ped)
        local radians = heading * math.pi / 180.0
        entity.setEntityCoords(
            ped,
            pos.x - math.sin(radians) * 8.0,
            pos.y + math.cos(radians) * 8.0,
            pos.z,
            false,
            false,
            false,
            false
        )
    end)
    ui.notify(ok and "Forward boost activated" or "Forward boost unavailable")
end)
menu:button("Hover Up (+1 m)", function()
    moveBy(0.0, 0.0, 1.0, "Hover up")
end)
menu:button("Reset Position (small drop)", function()
    moveBy(0.0, 0.0, -1.0, "Position reset")
end)

ui.notify("Iron Man movement powers loaded")
