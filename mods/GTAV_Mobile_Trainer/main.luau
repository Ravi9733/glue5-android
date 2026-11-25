--!strict
-- GTAV Mobile Trainer (GLUE 5 port, first functional slice)
-- F7 / touch Mods button > MENUS > GTAV Mobile Trainer.
-- This uses GLUE 5's in-process Luau/native bridge; it does not inject
-- the original Fuzzy-specific Android agent.

local menu = ui.menu("GTAV Mobile Trainer", "Vehicle spawning · GLUE 5 native bridge")

local models = {
    "adder",
    "zentorno",
    "t20",
    "sultan",
    "kuruma",
    "bati",
    "insurgent",
}

local selected = 1
local busy = false
local spawnDistance = 3
local ammoCount = 250
local savedPosition = nil

-- Keep the saved teleport point between game sessions.
local storedPosition = storage.get("saved_position")
if storedPosition then
    savedPosition = vector.create(storedPosition.x, storedPosition.y, storedPosition.z)
end

local weapons = {
    "WEAPON_PISTOL",
    "WEAPON_COMBATPISTOL",
    "WEAPON_SMG",
    "WEAPON_ASSAULTRIFLE",
    "WEAPON_CARBINERIFLE",
    "WEAPON_PUMPSHOTGUN",
    "WEAPON_SNIPERRIFLE",
    "WEAPON_RPG",
}
local selectedWeapon = 1

menu:slider("Vehicle spawn distance", spawnDistance, 2, 15, 1, function(value)
    spawnDistance = value
end)

menu:choice("Vehicle model", models, selected, function(index)
    selected = index
end)

menu:button("Spawn selected vehicle", {
    note = "Loads the model, then spawns it a short distance from the player",
    action = "Spawn",
}, function()
    if busy then
        ui.notify("Vehicle spawn is already running")
        return
    end

    busy = true
    local model = models[selected]
    local ok, err = pcall(function()
        local me = player.playerPedId()
        if not me then
            error("Player ped is not ready")
        end

        streaming.requestModel(model)
        local deadline = os.clock() + 10
        while not streaming.hasModelLoaded(model) do
            if os.clock() >= deadline then
                error("Timed out loading model: " .. model)
            end
            wait(50)
        end

        local pos = entity.getEntityCoords(me, true)
        local car = vehicle.createVehicle(
            model,
            pos.x + spawnDistance,
            pos.y,
            pos.z,
            0,
            false,
            false
        )

        if not car then
            error("Game did not create the vehicle")
        end

        streaming.setModelAsNoLongerNeeded(model)
        ui.notify("Spawned " .. model)
    end)

    busy = false
    if not ok then
        ui.notify("Spawn failed: " .. tostring(err))
        print("Vehicle spawn error:", err)
    end
end)

menu:label("TELEPORT")

menu:button("Save current position", {
    note = "Stores your current in-game coordinates for this session",
    action = "Save",
}, function()
    local me = player.playerPedId()
    if not me then
        ui.notify("Player ped is not ready")
        return
    end
    local p = entity.getEntityCoords(me, true)
    savedPosition = vector.create(p.x, p.y, p.z)
    storage.set("saved_position", { x = p.x, y = p.y, z = p.z })
    ui.notify("Current position saved")
end)

menu:button("Return to saved position", {
    note = "Returns your player to the saved coordinates",
    action = "Return",
}, function()
    if not savedPosition then
        ui.notify("Save a position first")
        return
    end
    local me = player.playerPedId()
    if not me then
        ui.notify("Player ped is not ready")
        return
    end
    local p = savedPosition
    entity.setEntityCoords(me, p.x, p.y, p.z, false, false, false, true)
    ui.notify("Returned to saved position")
end)

menu:label("WEAPONS")

menu:choice("Weapon", weapons, selectedWeapon, function(index)
    selectedWeapon = index
end)

menu:slider("Weapon ammo", ammoCount, 50, 1000, 50, function(value)
    ammoCount = value
end)

menu:button("Give selected weapon", {
    note = "Gives the selected weapon with the ammo amount selected above",
    action = "Give",
}, function()
    local me = player.playerPedId()
    if not me then
        ui.notify("Player ped is not ready")
        return
    end
    local weaponName = weapons[selectedWeapon]
    weapon.giveWeaponToPed(me, weaponName, ammoCount, false, true)
    ui.notify("Weapon granted: " .. weaponName)
end)


menu:label("CURRENT VEHICLE")

local function currentVehicle()
    local me = player.playerPedId()
    if not me or not ped.isPedInAnyVehicle(me, false) then
        return nil
    end
    local veh = ped.getVehiclePedIsIn(me, false)
    if not veh or veh == 0 then return nil end
    return veh
end

menu:button("Repair current vehicle", {
    note = "Fixes body and engine damage on the vehicle you are driving",
    action = "Repair",
}, function()
    local veh = currentVehicle()
    if not veh then
        ui.notify("Get into a vehicle first")
        return
    end
    vehicle.setVehicleFixed(veh)
    ui.notify("Vehicle repaired")
end)

menu:button("Start current vehicle engine", {
    note = "Turns on the engine of the vehicle you are driving",
    action = "Engine",
}, function()
    local veh = currentVehicle()
    if not veh then
        ui.notify("Get into a vehicle first")
        return
    end
    vehicle.setVehicleEngineOn(veh, true, true, false)
    ui.notify("Vehicle engine started")
end)

menu:button("Stop current vehicle engine", {
    note = "Turns off the engine of the vehicle you are driving",
    action = "Stop",
}, function()
    local veh = currentVehicle()
    if not veh then
        ui.notify("Get into a vehicle first")
        return
    end
    vehicle.setVehicleEngineOn(veh, false, true, true)
    ui.notify("Vehicle engine stopped")
end)

menu:label("Player health / wanted-level controls need verified native bindings")

menu:label("PLAYER STATUS")

menu:button("Restore health", {
    note = "Restores player health to 200",
    action = "Heal",
}, function()
    local me = player.playerPedId()
    if not me then
        ui.notify("Player ped is not ready")
        return
    end
    entity.setEntityHealth(me, 200, 0)
    ui.notify("Player health restored")
end)

menu:button("Give full armour", {
    note = "Sets player armour to 100",
    action = "Armour",
}, function()
    local me = player.playerPedId()
    if not me then
        ui.notify("Player ped is not ready")
        return
    end
    ped.setPedArmour(me, 100)
    ui.notify("Full armour applied")
end)

menu:slider("Wanted level", 0, 0, 5, 1, function(level)
    local me = player.playerPedId()
    if not me then
        ui.notify("Player ped is not ready")
        return
    end
    local playerId = player.playerId()
    player.setPlayerWantedLevel(playerId, level, false)
    player.setPlayerWantedLevelNow(playerId, false)
    ui.notify("Wanted level: " .. tostring(level))
end)

menu:button("Clear wanted level", {
    note = "Sets the current player's wanted level to zero",
    action = "Clear",
}, function()
    local playerId = player.playerId()
    if not playerId then
        ui.notify("Player is not ready")
        return
    end
    player.clearPlayerWantedLevel(playerId)
    ui.notify("Wanted level cleared")
end)

menu:label("DLC / OPENIV STATUS")
menu:label("Loose-file overlay exists in Glue5; full RPF/DLC pack mounting is not implemented yet.")

ui.notify("GTAV Mobile Trainer loaded")
