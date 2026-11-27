--!strict
-- FTrainer Android adaptation for GLUE 5.
-- Touch/controller menus are shown in Mods > MENUS (F7 during gameplay).
-- Vehicle and weapon lists are read from the installed game's live catalog,
-- including DLC/add-on names known to the running game.

local function notifyError(label: string, err: any)
    ui.notify(label .. " unavailable: " .. tostring(err))
end

local function safe(label: string, fn: () -> ())
    local ok, err = pcall(fn)
    if not ok then
        notifyError(label, err)
    end
    return ok
end

local function currentPed()
    local pedHandle = player.playerPedId()
    if not pedHandle or pedHandle == 0 then
        ui.notify("Player is not ready")
        return nil
    end
    return pedHandle
end

local function currentVehicle()
    local pedHandle = currentPed()
    if not pedHandle then return nil end
    if not ped.isPedInAnyVehicle(pedHandle, false) then
        ui.notify("Get into a vehicle first")
        return nil
    end
    return ped.getVehiclePedIsIn(pedHandle, false)
end

-- VEHICLE SPAWNER ------------------------------------------------------------

local vehicleModels = world.vehicleModels()
table.sort(vehicleModels)
local vehicleBusy = false

local function spawnVehicle(modelName: string)
    if vehicleBusy then
        ui.notify("A vehicle is already loading")
        return
    end
    local pedHandle = currentPed()
    if not pedHandle then return end

    vehicleBusy = true
    local model = modelName :: VehicleModel
    local modelRequested = false
    local ok, err = pcall(function()
        streaming.requestModel(model)
        modelRequested = true
        local deadline = os.clock() + 15
        while not streaming.hasModelLoaded(model) and os.clock() < deadline do
            wait(100)
        end
        if not streaming.hasModelLoaded(model) then
            error("model loading timed out: " .. modelName)
        end

        local pos = entity.getEntityCoords(pedHandle, false)
        local heading = entity.getEntityHeading(pedHandle)
        local radians = math.rad(heading)
        local spawnPos = vector.create(
            pos.x - math.sin(radians) * 6,
            pos.y + math.cos(radians) * 6,
            pos.z + 0.5
        )
        local car = vehicle.createVehicle(model, spawnPos, heading, true, false)
        if not car or car == 0 then
            error("the game refused to create " .. modelName)
        end
        ped.setPedIntoVehicle(pedHandle, car, -1)
        ui.notify("Spawned " .. modelName)
    end)
    if modelRequested then
        pcall(function()
            streaming.setModelAsNoLongerNeeded(model)
        end)
    end
    vehicleBusy = false
    if not ok then notifyError("Vehicle spawn", err) end
end

if #vehicleModels == 0 then
    ui.notify("Vehicle catalog is empty; try starting the game again")
else
    local pageSize = 32
    local pageCount = math.ceil(#vehicleModels / pageSize)
    for page = 1, pageCount do
        local first = (page - 1) * pageSize + 1
        local last = math.min(page * pageSize, #vehicleModels)
        local pageModels = {}
        for i = first, last do table.insert(pageModels, vehicleModels[i]) end

        local selected = 1
        local m = ui.menu("Vehicles " .. page .. "/" .. pageCount)
        m:label("Installed game catalog · " .. #vehicleModels .. " models")
        m:choice("Vehicle model", pageModels, 1, function(index)
            selected = index
        end)
        m:button("Spawn selected vehicle", "Spawn the selected installed/DLC model", function()
            spawnVehicle(pageModels[selected])
        end)
    end
end

local vehicleMenu = ui.menu("Vehicle options")
vehicleMenu:button("Repair and clean vehicle", "Fix damage and remove dirt", function()
    local car = currentVehicle()
    if car then
        safe("Vehicle repair", function()
            vehicle.setVehicleFixed(car)
            vehicle.setVehicleDirtLevel(car, 0.0)
            ui.notify("Vehicle repaired and cleaned")
        end)
    end
end)
vehicleMenu:button("Flip vehicle upright", "Place your current vehicle on the ground", function()
    local car = currentVehicle()
    if car then
        safe("Vehicle flip", function()
            vehicle.setVehicleOnGroundProperly(car)
            ui.notify("Vehicle repositioned")
        end)
    end
end)
vehicleMenu:button("Max vehicle upgrades", "Apply the highest available level in each mod slot", function()
    local car = currentVehicle()
    if car then
        safe("Vehicle upgrades", function()
            vehicle.setVehicleModKit(car, 0)
            for modType = 0, 49 do
                local count = vehicle.getNumVehicleMods(car, modType)
                if count and count > 0 then
                    vehicle.setVehicleMod(car, modType, count - 1, false)
                end
            end
            vehicle.toggleVehicleMod(car, 18, true)
            vehicle.toggleVehicleMod(car, 22, true)
            ui.notify("Available vehicle upgrades applied")
        end)
    end
end)
local protectedVehicle = nil
vehicleMenu:toggle("Vehicle invincible", false, "Protect the vehicle you are driving", function(enabled)
    local car = if enabled then currentVehicle() else (protectedVehicle or currentVehicle())
    if car then
        local ok = safe("Vehicle protection", function()
            entity.setEntityInvincible(car, enabled)
        end)
        if ok then protectedVehicle = if enabled then car else nil end
    end
end)

local appearanceMenu = ui.menu("Vehicle appearance")
local primaryPaint = 0
local secondaryPaint = 0
appearanceMenu:slider("Primary paint ID", 0, 0, 159, 1, function(value)
    primaryPaint = math.floor(value)
end)
appearanceMenu:slider("Secondary paint ID", 0, 0, 159, 1, function(value)
    secondaryPaint = math.floor(value)
end)
appearanceMenu:button("Apply paint", "Apply selected primary and secondary paint IDs", function()
    local car = currentVehicle()
    if car then safe("Vehicle paint", function()
        vehicle.setVehicleModKit(car, 0)
        vehicle.setVehicleColours(car, primaryPaint, secondaryPaint)
        ui.notify("Vehicle paint applied")
    end) end
end)
appearanceMenu:button("Enable neon lights", "Turn on neon on all four sides", function()
    local car = currentVehicle()
    if car then safe("Vehicle neon", function()
        vehicle.setVehicleNeonLightsColour(car, 0, 255, 120)
        for side = 0, 3 do
            vehicle.setVehicleNeonLightEnabled(car, side, true)
        end
        ui.notify("Neon lights enabled")
    end) end
end)
appearanceMenu:button("Disable neon lights", "Turn off neon on all four sides", function()
    local car = currentVehicle()
    if car then safe("Vehicle neon", function()
        for side = 0, 3 do
            vehicle.setVehicleNeonLightEnabled(car, side, false)
        end
        ui.notify("Neon lights disabled")
    end) end
end)

-- PLAYER ---------------------------------------------------------------------

local playerMenu = ui.menu("Player options")
playerMenu:button("Restore health", "Restore player health", function()
    local pedHandle = currentPed()
    if pedHandle then safe("Health", function()
        entity.setEntityHealth(pedHandle, 200, 0)
        ui.notify("Health restored")
    end) end
end)
playerMenu:button("Give full armour", "Set player armour to 100", function()
    local pedHandle = currentPed()
    if pedHandle then safe("Armour", function()
        ped.setPedArmour(pedHandle, 100)
        ui.notify("Full armour")
    end) end
end)
playerMenu:toggle("God mode", false, "Toggle player invincibility", function(enabled)
    local pedHandle = currentPed()
    if pedHandle then safe("God mode", function()
        entity.setEntityInvincible(pedHandle, enabled)
        ui.notify(if enabled then "God mode ON" else "God mode OFF")
    end) end
end)

-- WEAPON SPAWNER -------------------------------------------------------------

local weaponNames = world.weapons()
table.sort(weaponNames)
if #weaponNames > 0 then
    local pageSize = 32
    local pageCount = math.ceil(#weaponNames / pageSize)
    for page = 1, pageCount do
        local first = (page - 1) * pageSize + 1
        local last = math.min(page * pageSize, #weaponNames)
        local pageWeapons = {}
        for i = first, last do table.insert(pageWeapons, weaponNames[i]) end

        local selected = 1
        local m = ui.menu("Weapons " .. page .. "/" .. pageCount)
        m:label("Installed weapon catalog · " .. #weaponNames .. " weapons")
        m:choice("Weapon", pageWeapons, 1, function(index)
            selected = index
        end)
        m:button("Give selected weapon", "Give weapon with 9999 rounds", function()
            local pedHandle = currentPed()
            if pedHandle then safe("Weapon", function()
                weapon.giveWeaponToPed(pedHandle, pageWeapons[selected] :: WeaponName, 9999, false, true)
                ui.notify("Given " .. pageWeapons[selected])
            end) end
        end)
    end
end

local weaponMenu = ui.menu("Weapon options")
weaponMenu:button("Give all installed weapons", "Give every weapon in the installed catalog", function()
    local pedHandle = currentPed()
    if pedHandle then safe("All weapons", function()
        for _, name in weaponNames do
            weapon.giveWeaponToPed(pedHandle, name :: WeaponName, 9999, false, false)
        end
        ui.notify("Installed weapons added")
    end) end
end)
weaponMenu:button("Remove all weapons", "Remove the player's weapons", function()
    local pedHandle = currentPed()
    if pedHandle then safe("Remove weapons", function()
        weapon.removeAllPedWeapons(pedHandle, true)
        ui.notify("Weapons removed")
    end) end
end)
weaponMenu:toggle("Infinite ammo clip", false, "Toggle unlimited ammunition in the current clip", function(enabled)
    local pedHandle = currentPed()
    if pedHandle then safe("Infinite ammo", function()
        weapon.setPedInfiniteAmmoClip(pedHandle, enabled)
    end) end
end)

-- WEATHER AND TIME -----------------------------------------------------------

local worldMenu = ui.menu("World options")
local weatherNames = {
    "EXTRASUNNY", "CLEAR", "CLOUDS", "SMOG", "FOGGY",
    "OVERCAST", "RAIN", "THUNDER", "CLEARING", "NEUTRAL",
    "SNOW", "BLIZZARD", "SNOWLIGHT", "XMAS", "HALLOWEEN"
}
local selectedWeather = 1
worldMenu:choice("Weather", weatherNames, 1, function(index)
    selectedWeather = index
end)
worldMenu:button("Apply weather", "Set selected weather", function()
    safe("Weather", function()
        misc.setWeatherTypeNowPersist(weatherNames[selectedWeather])
        ui.notify("Weather: " .. weatherNames[selectedWeather])
    end)
end)
local selectedHour = 12
worldMenu:slider("Time of day", 12, 0, 23, 1, function(value)
    selectedHour = math.floor(value)
end)
worldMenu:button("Apply time", "Set game time to selected hour", function()
    safe("Time", function()
        misc.setClockTime(selectedHour, 0, 0)
        ui.notify("Time changed")
    end)
end)

-- TELEPORTS ------------------------------------------------------------------

local teleportMenu = ui.menu("Teleport locations")
local locations = {
    { name = "Airport", x = -1034.6, y = -2733.6, z = 13.8 },
    { name = "Legion Square", x = 195.2, y = -933.8, z = 30.7 },
    { name = "Mount Chiliad", x = 450.7, y = 5566.6, z = 806.2 },
    { name = "Sandy Shores", x = 1853.2, y = 3686.4, z = 34.3 },
    { name = "Vinewood Sign", x = 711.3, y = 1198.2, z = 348.5 },
    { name = "Fort Zancudo", x = -2047.4, y = 3132.1, z = 32.8 },
    { name = "Maze Bank Tower", x = -75.0, y = -818.9, z = 326.2 }
}
for _, location in locations do
    teleportMenu:button(location.name, "Teleport player to " .. location.name, function()
        local pedHandle = currentPed()
        if pedHandle then safe("Teleport", function()
            entity.setEntityCoords(
                pedHandle,
                vector.create(location.x, location.y, location.z),
                false, false, false, false
            )
            ui.notify("Teleported to " .. location.name)
        end) end
    end)
end

ui.notify("FTrainer Android loaded · open F7 / Mods > MENUS")
