--!strict
-- One compact vehicle menu plus a few basic Rampage-style trainer actions.
-- This uses only the models already present in the installed game assets.

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

local function spawnSelectedVehicle()
    local entry = categories[selectedType]
    local model = entry.model

    streaming.requestModel(model)
    local tries = 0
    while not streaming.hasModelLoaded(model) and tries < 100 do
        wait()
        tries += 1
    end

    if not streaming.hasModelLoaded(model) then
        ui.notify("Vehicle model did not load: " .. model)
        return
    end

    local me = player.playerPedId()
    local pos = entity.getOffsetFromEntityInWorldCoords(me, 0.0, 5.0, 0.0)
    local heading = entity.getEntityHeading(me)
    local vehicleHandle = vehicle.createVehicle(
        model, pos.x, pos.y, pos.z, heading, true, false
    )

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
menu:label("Rampage-style Trainer")
menu:button("Heal + Armour", function()
    local me = player.playerPedId()
    entity.setEntityHealth(me, entity.getEntityMaxHealth(me))
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
