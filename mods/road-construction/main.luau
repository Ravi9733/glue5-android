-- JSON placement smoke test: spawn each manifest object 3 metres in front of the player.
local placements = world.loadPlacements("road_construction.json")

task.spawn(function()
    local ped = player.getPlayerPed(player.playerId())
    if not ped then
        error("Road Construction JSON Test: local player ped is unavailable")
    end

    for _, item in placements do
        if streaming.load(item.model) then
            local position = entity.getOffsetFromEntityInWorldCoords(ped, 0.0, 3.0, 0.0)
            local objectHandle = object.createObject(item.model, position, false, false, false)
            streaming.setModelAsNoLongerNeeded(item.model)
            if objectHandle then
                entity.setEntityRotation(objectHandle, item.rotation, 2, true)
            end
        end
        wait()
    end
end)
