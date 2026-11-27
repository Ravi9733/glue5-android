-- FTrainer-style Android trainer for GLUE 5.
-- Native Luau mod: touch menu is exposed by GLUE 5 under F7 > MENUS.
-- First implementation: safe model loading and vehicle spawning.

local menu = ui.menu("FTrainer Android", "Vehicle spawner and quick vehicle actions")

local models = {
    "adder", "zentorno", "t20", "cheetah", "turismor",
    "entityxf", "infernus", "vacca", "bullet", "osiris",
    "nero", "italigtb", "autarch", "deveste", "krieger",
    "sultan", "elegy", "buffalo", "comet2", "jester",
    "kuruma", "insurgent", "nightshark", "blazer", "bati"
}

local names = {}
for _, model in models do
    table.insert(names, model)
end

local selected = 1
local busy = false
local lastVehicle = nil

menu:choice("Vehicle model", names, 1, function(index)
    selected = index
end)

local function spawnSelected()
    if busy then
        ui.notify("Please wait: vehicle is loading")
        return
    end
    busy = true

    local modelName = models[selected]
    local model = modelName :: VehicleModel
    local pedHandle = player.playerPedId()
    if not pedHandle or pedHandle == 0 then
        ui.notify("Player is not ready")
        busy = false
        return
    end

    streaming.requestModel(model)
    local started = os.clock()
    while not streaming.hasModelLoaded(model) and os.clock() - started < 12 do
        wait(100)
    end

    if not streaming.hasModelLoaded(model) then
        ui.notify("Vehicle model load timed out: " .. modelName)
        busy = false
        return
    end

    local pos = entity.getEntityCoords(pedHandle, false)
    local heading = entity.getEntityHeading(pedHandle)
    local radians = math.rad(heading)
    local spawnPos = vector.create(
        pos.x - math.sin(radians) * 5,
        pos.y + math.cos(radians) * 5,
        pos.z + 0.5
    )

    local car = vehicle.createVehicle(model, spawnPos, heading, true, false)
    if car and car ~= 0 then
        lastVehicle = car
        ped.setPedIntoVehicle(pedHandle, car, -1)
        ui.notify("Spawned: " .. modelName)
    else
        ui.notify("Vehicle spawn failed: " .. modelName)
    end

    streaming.setModelAsNoLongerNeeded(model)
    busy = false
end

menu:button("Spawn selected vehicle", {
    note = "Load and spawn the selected car beside you",
    action = "Spawn"
}, spawnSelected)

menu:button("Repair current vehicle", {
    note = "Repair the vehicle you are driving",
    action = "Repair"
}, function()
    local pedHandle = player.playerPedId()
    if not ped.isPedInAnyVehicle(pedHandle, false) then
        ui.notify("Get into a vehicle first")
        return
    end
    local car = ped.getVehiclePedIsIn(pedHandle, false)
    vehicle.setVehicleFixed(car)
    vehicle.setVehicleDirtLevel(car, 0.0)
    ui.notify("Vehicle repaired")
end)

menu:button("Flip / put vehicle upright", {
    note = "Put your current vehicle back on its wheels",
    action = "Flip"
}, function()
    local pedHandle = player.playerPedId()
    if not ped.isPedInAnyVehicle(pedHandle, false) then
        ui.notify("Get into a vehicle first")
        return
    end
    local car = ped.getVehiclePedIsIn(pedHandle, false)
    vehicle.setVehicleOnGroundProperly(car)
    ui.notify("Vehicle repositioned")
end)

menu:label("More trainer categories will be added after device testing.")
ui.notify("FTrainer Android menu loaded")
