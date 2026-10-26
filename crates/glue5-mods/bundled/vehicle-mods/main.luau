--!strict
-- Compact vehicle trainer and an experimental in-game stunt track.
-- Uses models already present in GTA V; it does not install PC Menyoo files.

local menu = ui.menu("Vehicle Mods")
local selectedType = 1

local categories = {
    { name = "Cars", model = "sultan" },
    { name = "Bikes", model = "bati" },
    { name = "Planes", model = "dodo" },
    { name = "Boats", model = "dinghy" },
}

menu:choice("Vehicle Type", { "Cars", "Bikes", "Planes", "Boats" }, 1, function(value)
    selectedType = value
end)

local function loadModel(model)
    streaming.requestModel(model)
    local tries = 0
    while not streaming.hasModelLoaded(model) and tries < 100 do
        wait()
        tries += 1
    end
    return streaming.hasModelLoaded(model)
end

local function spawnSelectedVehicle()
    local entry = categories[selectedType]
    local model = entry.model
    if not loadModel(model) then
        ui.notify("Vehicle model did not load: " .. model)
        return
    end

    local me = player.playerPedId()
    local pos = entity.getOffsetFromEntityInWorldCoords(me, 0.0, 5.0, 0.0)
    local heading = entity.getEntityHeading(me)
    local vehicleHandle = vehicle.createVehicle(model, pos.x, pos.y, pos.z, heading, true, false)

    if vehicleHandle ~= 0 then
        entity.setAsMissionEntity(vehicleHandle, true, true)
        ped.setPedIntoVehicle(me, vehicleHandle, -1)
        ui.notify(entry.name .. " spawned")
    else
        ui.notify("Vehicle could not be created")
    end
    streaming.setModelAsNoLongerNeeded(model)
end

menu:button("Spawn Selected Vehicle", spawnSelectedVehicle)

local function spawnStuntTrack()
    local me = player.playerPedId()
    local heading = entity.getEntityHeading(me)
    local segments = {
        { model = "stt_prop_stunt_track_start", forward = 12.0, up = 8.0 },
        { model = "stt_prop_stunt_track_straight", forward = 27.0, up = 8.0 },
        { model = "stt_prop_stunt_track_straight", forward = 42.0, up = 8.0 },
        { model = "stt_prop_stunt_track_straight", forward = 57.0, up = 10.0 },
        { model = "stt_prop_stunt_track_jump", forward = 72.0, up = 12.0 },
        { model = "stt_prop_stunt_ramp", forward = 88.0, up = 14.0 },
    }
    local created = 0

    for _, segment in segments do
        if loadModel(segment.model) then
            local pos = entity.getOffsetFromEntityInWorldCoords(me, 0.0, segment.forward, segment.up)
            local obj = object.createObject(segment.model, pos.x, pos.y, pos.z, true, true, false)
            if obj ~= 0 then
                entity.setEntityHeading(obj, heading)
                entity.setAsMissionEntity(obj, true, true)
                created += 1
            end
            streaming.setModelAsNoLongerNeeded(segment.model)
        end
        wait()
    end

    if created == #segments then
        ui.notify("Stunt track spawned (" .. tostring(created) .. " pieces)")
    else
        ui.notify("Stunt track partial: " .. tostring(created) .. "/" .. tostring(#segments) .. " pieces")
    end
end

menu:button("Spawn Stunt Track (Experimental)", spawnStuntTrack)
menu:button("Heal + Armour", function()
    local me = player.playerPedId()
    entity.setEntityHealth(me, entity.getEntityMaxHealth(me), 0, 0)
    ped.setPedArmour(me, 100)
    ui.notify("Health and armour restored")
end)
menu:button("Clear Wanted Level", function()
    player.clearPlayerWantedLevel(player.playerId())
    ui.notify("Wanted level cleared")
end)

ui.notify("Vehicle Mods ready")
while true do
    wait()
end
