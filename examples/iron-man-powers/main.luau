--!strict
-- GLUE5_ANDROID_IRON_MAN_FLIGHT_V2
-- Touch-friendly continuous movement controls for Glue 5.
-- This is script-based movement, NOT a custom Iron Man character model.

local flightEnabled = false
local forwardStep = 0.35

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

local function moveForward(distance: number)
    local ped = getPed()
    local pos = entity.getEntityCoords(ped, true)
    local heading = entity.getEntityHeading(ped)
    local radians = heading * math.pi / 180.0
    entity.setEntityCoords(
        ped,
        pos.x - math.sin(radians) * distance,
        pos.y + math.cos(radians) * distance,
        pos.z,
        false,
        false,
        false,
        false
    )
end

local menu = ui.menu("Iron Man Flight")
menu:label("Flight moves the current character; suit model is not included.")
menu:button("Start / Stop Flight", function()
    flightEnabled = not flightEnabled
    ui.notify(flightEnabled and "Flight mode ON — move forward" or "Flight mode OFF")
end)
menu:button("Ascend (+2 m)", function()
    moveBy(0.0, 0.0, 2.0, "Ascend")
end)
menu:button("Descend (-2 m)", function()
    moveBy(0.0, 0.0, -2.0, "Descend")
end)
menu:button("Forward Boost", function()
    local ok = pcall(function()
        moveForward(8.0)
    end)
    ui.notify(ok and "Forward boost activated" or "Forward boost unavailable")
end)
menu:button("Land", function()
    flightEnabled = false
    moveBy(0.0, 0.0, -5.0, "Landing")
end)

task.spawn(function()
    while true do
        if flightEnabled then
            local ok = pcall(function()
                moveForward(forwardStep)
            end)
            if not ok then
                flightEnabled = false
                ui.notify("Flight stopped: movement native unavailable")
            end
            wait(50)
        else
            wait(100)
        end
    end
end)

ui.notify("Iron Man flight controls loaded")
