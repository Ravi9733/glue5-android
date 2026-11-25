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

menu:label("OpenIV / DLC mounting is still a separate target")

ui.notify("GTAV Mobile Trainer loaded")
