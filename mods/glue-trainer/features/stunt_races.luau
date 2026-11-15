--!strict
-- Native Glue 5 offline stunt maps and checkpoint races; no Menyoo/ScriptHook runtime.

local bind = require("../core/bind")
local game = require("../core/game")
local M = {}

local activeRace = false
local respawnRequested = false
local raceVehicle: Vehicle? = nil
local raceCheckpoint: number? = nil
local routeBlips: { Blip } = {}

local function clearRoute()
	if raceCheckpoint then
		pcall(function()
			graphics.deleteCheckpoint(raceCheckpoint :: number)
		end)
		raceCheckpoint = nil
	end
	for _, blip in routeBlips do
		if hud.doesBlipExist(blip) then
			hud.removeBlip(blip)
		end
	end
	table.clear(routeBlips)
end

mod.onStop(function()
	activeRace = false
	pcall(clearRoute)
	if raceVehicle and entity.doesEntityExist(raceVehicle) then
		pcall(function()
			entity.freezeEntityPosition(raceVehicle :: Vehicle, false)
		end)
	end
	raceVehicle = nil
end)

local function showRoute(points: { vector }, current: number)
	clearRoute()
	for i = current, #points do
		local blip = hud.addBlipForCoord(points[i])
		if blip then
			hud.setBlipSprite(blip, 1)
			hud.setBlipColour(blip, if i == #points then 5 else 38)
			hud.setBlipScale(blip, 0.8)
			hud.setBlipRoute(blip, i == current)
			table.insert(routeBlips, blip)
		end
	end
end

local function setRaceCheckpoint(points: { vector }, index: number)
	if raceCheckpoint then
		pcall(function()
			graphics.deleteCheckpoint(raceCheckpoint :: number)
		end)
		raceCheckpoint = nil
	end
	local current = points[index]
	local nextPoint = points[math.min(index + 1, #points)]
	local ok, handle = pcall(function()
		return graphics.createCheckpoint(
			10,
			current.x, current.y, current.z,
			nextPoint.x, nextPoint.y, nextPoint.z,
			14.0, 40, 210, 255, 220, 0
		)
	end)
	if ok and handle and handle ~= 0 then
		raceCheckpoint = handle
		pcall(function()
			graphics.setCheckpointCylinderHeight(handle, 8.0, 8.0, 14.0)
			graphics.setCheckpointRgba(handle, 40, 210, 255, 220)
		end)
	end
end

local function spawnRaceVehicle(modelName: string, at: vector): (Vehicle?, string?)
	local name = string.lower(string.match(modelName, "^%s*(.-)%s*$") or "")
	if name == "" or not string.match(name, "^[%w_%-]+$") then
		return nil, "Enter a valid vehicle model name"
	end
	local hash = joaat(name)
	if not streaming.isModelInCdimage(hash) or not streaming.isModelAVehicle(hash) then
		return nil, name .. " is not a registered vehicle in the loaded game data"
	end
	if not streaming.load(hash) then
		return nil, "Vehicle model did not load: " .. name
	end
	local car = vehicle.createVehicle(hash, at, entity.getEntityHeading(game.me()), false, false, false)
	streaming.setModelAsNoLongerNeeded(hash)
	if not car then
		return nil, "The game could not create the race vehicle"
	end
	pcall(function()
		entity.setEntityAsMissionEntity(car, true, true)
	end)
	vehicle.setVehicleOnGroundProperly(car, 5)
	vehicle.setVehicleEngineOn(car, true, true, false)
	ped.setPedIntoVehicle(game.me(), car, VehicleSeat.DRIVER)
	return car, nil
end

local function runRace(routeName: string, modelName: string): string?
	if game.online() then
		return "Stunt races are Offline/Sandbox only; leave GTA Online first"
	end
	if activeRace then
		return "A stunt race is already running"
	end
	local ok, points = pcall(function()
		return world.loadRaceRoute("maps/" .. routeName)
	end)
	if not ok then
		return "Route load failed: " .. tostring(points)
	end
	if #points < 2 then
		return "This XML needs at least two <Checkpoint><Position> entries"
	end
	local car, why = spawnRaceVehicle(modelName, points[1])
	if not car then
		return why
	end
	raceVehicle = car
	respawnRequested = false
	entity.freezeEntityPosition(car, true)
	activeRace = true
	task.spawn(function()
		local finished = false
		local checkpoint = 1
		local startedAt = 0
		local okRace, raceError = pcall(function()
			game.notify("Stunt race starting")
			for count = 3, 1, -1 do
				if not activeRace then return end
				game.notify(tostring(count))
				wait(1000)
			end
			if not activeRace then return end
			game.notify("GO!")
			if raceVehicle and entity.doesEntityExist(raceVehicle) then
				entity.freezeEntityPosition(raceVehicle, false)
			end
			startedAt = os.clock()
			showRoute(points, 2)
			setRaceCheckpoint(points, 2)
			checkpoint = 2
			while activeRace and checkpoint <= #points do
				wait(0)
				if respawnRequested and raceVehicle and entity.doesEntityExist(raceVehicle) then
					entity.setEntityCoordsNoOffset(raceVehicle, points[checkpoint - 1], false, false, true)
					vehicle.setVehicleForwardSpeed(raceVehicle, 0)
					vehicle.setVehicleOnGroundProperly(raceVehicle, 5)
					respawnRequested = false
					game.notify("Respawned at checkpoint " .. tostring(checkpoint - 1))
				end
				if not raceVehicle or not entity.doesEntityExist(raceVehicle) then
					local replacement, spawnError = spawnRaceVehicle(modelName, points[checkpoint - 1])
					if not replacement then
						error(spawnError or "Race vehicle could not respawn")
					end
					raceVehicle = replacement
				elseif game.vehicle() ~= raceVehicle then
					ped.setPedIntoVehicle(game.me(), raceVehicle, VehicleSeat.DRIVER)
				end
				local position = entity.getEntityCoords(raceVehicle, true)
				if vector.magnitude(position - points[checkpoint]) <= 18 then
					game.notify("Checkpoint " .. tostring(checkpoint - 1) .. "/" .. tostring(#points - 1))
					checkpoint += 1
					if checkpoint <= #points then
						showRoute(points, checkpoint)
						setRaceCheckpoint(points, checkpoint)
					end
				end
			end
			finished = activeRace and checkpoint > #points
		end)
		clearRoute()
		activeRace = false
		if not okRace then
			game.notify("Stunt race stopped: " .. tostring(raceError))
		elseif finished then
			local seconds = math.max(0, os.clock() - startedAt)
			game.notify(string.format("RACE FINISHED! Time: %.2f seconds", seconds))
		else
			game.notify("Stunt race cancelled")
		end
	end)
	return "Race loaded: " .. tostring(#points) .. " checkpoints"
end

function M.build(root: Menu)
	local menu = root:submenu("STUNT RACES", "Offline stunt maps and checkpoint races using Glue 5 natives")
	menu:label("Import Menyoo / CodeWalker object XML, or Rockstar Creator race JSON.")
	menu:label("For .RAR/.7z downloads, extract first, then import the XML/JSON or ZIP.")
	menu:label("Object XML supports Menyoo PositionRotation and CodeWalker CEntityDef.")
	menu:label("Rockstar race JSON reads mission.race.chl checkpoint coordinates.")

	bind.action(menu, "Import Rockstar UGC JSON / ZIP", {
		note = "Import a Rockstar Creator job JSON or ZIP into maps/routes",
		action = "Import UGC",
	}, function(): string?
		local imported, err = pcall(function()
			world.requestRaceImport()
		end)
		if not imported then
			return "Race import picker unavailable: " .. tostring(err)
		end
		return "Choose a Rockstar Creator .json file or ZIP containing UGC JSON"
	end)

	bind.action(menu, "Import XML / ZIP", {
		note = "Choose an XML map or ZIP containing XML maps",
		action = "Import",
	}, function(): string?
		local imported, err = pcall(function()
			world.requestPlacementImport()
		end)
		if not imported then
			return "Import picker unavailable: " .. tostring(err)
		end
		return "Choose a stunt-map XML or ZIP in the Android file picker"
	end)

	local ok, maps = pcall(function()
		return world.listPlacementFiles()
	end)
	if not ok then
		menu:label("Could not read maps folder: " .. tostring(maps))
		return
	end
	local builder = menu:submenu("Create checkpoint route", "Stand at each checkpoint and save a local race route")
	local routeName = builder:text("Route name", "my_stunt_route", { placeholder = "letters, numbers, _ or -", max = 64 })
	local draft: { vector } = {}
	local draftCount = builder:label("Checkpoints captured: 0")
	bind.action(builder, "Add checkpoint at my position", {
		note = "Stand in the middle of the next gate; add checkpoints in driving order",
		action = "Add",
	}, function(): string?
		if #draft >= 128 then
			return "Checkpoint limit reached (128)"
		end
		table.insert(draft, game.position())
		draftCount:set("Checkpoints captured: " .. tostring(#draft))
		return "Checkpoint " .. tostring(#draft) .. " captured"
	end)
	bind.action(builder, "Undo last checkpoint", { action = "Undo" }, function(): string?
		if #draft == 0 then
			return "No checkpoint to undo"
		end
		table.remove(draft)
		draftCount:set("Checkpoints captured: " .. tostring(#draft))
		return "Removed last checkpoint; " .. tostring(#draft) .. " remain"
	end)
	bind.action(builder, "Clear route draft", { note = "Remove all unsaved checkpoints", action = "Clear" }, function(): string?
		draft = {}
		draftCount:set("Checkpoints captured: 0")
		return "Unsaved route draft cleared"
	end)
	bind.action(builder, "Save route XML", { note = "Requires at least two checkpoints; existing files are not overwritten", action = "Save" }, function(): string?
		local okSave, result = pcall(function()
			return world.saveRaceRoute(routeName.value, draft)
		end)
		if not okSave then
			return "Route save failed: " .. tostring(result)
		end
		draft = {}
		draftCount:set("Checkpoints captured: 0")
		return "Saved " .. tostring(result) .. ". Restart Glue 5 to refresh the route list."
	end)

	if #maps == 0 then
		menu:label("No XML maps imported yet. Create a checkpoint route above or import an XML map.")
	end

	local routeOk, raceRoutes = pcall(function()
		return world.listRaceRoutes()
	end)
	if not routeOk then
		menu:label("Could not read race routes: " .. tostring(raceRoutes))
	elseif #raceRoutes == 0 then
		menu:label("No playable checkpoint routes yet. Use Create checkpoint route above.")
	else
		local raceMenu = menu:submenu("Start checkpoint race", "Select a validated checkpoint route and vehicle")
		local route = raceMenu:choice("Route", raceRoutes, 1, "Only XML files with at least two checkpoints appear here")
		bind.action(raceMenu, "Preview selected route", {
			note = "Show checkpoint count and approximate route length",
			action = "Preview",
		}, function(): string?
			local okRoute, points = pcall(function()
				return world.loadRaceRoute("maps/" .. raceRoutes[route.value])
			end)
			if not okRoute then return "Route preview failed: " .. tostring(points) end
			local distance = 0.0
			for i = 2, #points do
				distance += vector.magnitude(points[i] - points[i - 1])
			end
			local propCount = "props: n/a"
			if string.sub(string.lower(raceRoutes[route.value]), -5) == ".json" then
				local propsOk, props = pcall(function()
					return world.loadPlacements("maps/routes/" .. raceRoutes[route.value])
				end)
				if propsOk then propCount = "static props: " .. tostring(#props) end
			end
			return string.format("%s | %d checkpoints | %.0f metres | %s",
				raceRoutes[route.value], #points, distance, propCount)
		end)
		bind.action(raceMenu, "Spawn selected Rockstar track props", {
			note = "Spawn mission.prop static objects from the selected Rockstar UGC JSON",
			action = "Spawn props",
		}, function(): string?
			local name = raceRoutes[route.value]
			if string.sub(string.lower(name), -5) ~= ".json" then
				return "Select a Rockstar UGC .json race file to spawn its props"
			end
			if game.online() then return "Track spawning is Offline/Sandbox only" end
			local spawned, result = pcall(function()
				return world.spawnPlacements("maps/routes/" .. name)
			end)
			if not spawned then return "UGC prop spawn failed: " .. tostring(result) end
			return "Rockstar UGC track props spawning: " .. name
		end)
		local vehicleName = raceMenu:text("Vehicle model", "adder", { placeholder = "e.g. adder, t20, dyfurai", max = 64 })
		bind.action(raceMenu, "Start race", {
			note = "3-second countdown, route checkpoints, timer and finish",
			action = "Race",
		}, function(): string?
			return runRace(raceRoutes[route.value], vehicleName.value)
		end)
		bind.action(raceMenu, "Respawn at last checkpoint", { note = "Move your race vehicle back to the last passed checkpoint", action = "Respawn" }, function(): string?
			if not activeRace then
				return "No stunt race is running"
			end
			respawnRequested = true
			return "Respawn requested"
		end)
		bind.action(raceMenu, "Stop race", { action = "Stop" }, function(): string?
			if not activeRace then
				return "No stunt race is running"
			end
			activeRace = false
			clearRoute()
			if raceVehicle and entity.doesEntityExist(raceVehicle) then
				entity.freezeEntityPosition(raceVehicle, false)
			end
			return "Stopping stunt race"
		end)
	end

	menu:separator("Spawn stunt track objects")
	bind.action(menu, "Clear spawned track objects", {
		note = "Remove every track object spawned by this trainer session",
		action = "Clear",
	}, function(): string?
		local okClear, removed = pcall(function()
			return world.clearSpawnedPlacements()
		end)
		if not okClear then
			return "Could not clear track objects: " .. tostring(removed)
		end
		return "Removed " .. tostring(removed) .. " spawned track objects"
	end)
	for _, name in maps do
		bind.action(menu, "Preview " .. name, {
			note = "Show object count, map bounds and a sample of prop models",
			action = "Preview",
		}, function(): string?
			local loaded, placements = pcall(function()
				return world.loadPlacements("maps/" .. name)
			end)
			if not loaded then
				return "Map preview failed: " .. tostring(placements)
			end
			local minX, minY, minZ = math.huge, math.huge, math.huge
			local maxX, maxY, maxZ = -math.huge, -math.huge, -math.huge
			local models: { string } = {}
			local seen: { [string]: boolean } = {}
			for _, item in placements do
				local p = item.position
				minX, minY, minZ = math.min(minX, p.x), math.min(minY, p.y), math.min(minZ, p.z)
				maxX, maxY, maxZ = math.max(maxX, p.x), math.max(maxY, p.y), math.max(maxZ, p.z)
				if not seen[item.model] then
					seen[item.model] = true
					if #models < 4 then table.insert(models, item.model) end
				end
			end
			return string.format(
				"%s: %d objects | bounds X %.0f..%.0f Y %.0f..%.0f Z %.0f..%.0f | models: %s",
				name, #placements, minX, maxX, minY, maxY, minZ, maxZ, table.concat(models, ", ")
			)
		end)
		bind.action(menu, name, {
			note = "Spawn validated and frozen track objects with Glue 5",
			action = "Spawn",
		}, function(): string?
			if game.online() then
				return "Track spawning is Offline/Sandbox only; leave GTA Online first"
			end
			local spawned, result = pcall(function()
				return world.spawnPlacements("maps/" .. name)
			end)
			if not spawned then
				return "Map load failed: " .. tostring(result)
			end
			return "Map spawning started: " .. name
		end)
	end

	local manage = menu:submenu("Manage imported maps", "Delete an imported XML map")
	for _, name in maps do
		bind.action(manage, "Delete " .. name, {
			note = "Permanently deletes this imported XML file",
			action = "Delete",
		}, function(): string?
			local deleted, err = pcall(function()
				world.deletePlacementFile(name)
			end)
			if not deleted then
				return "Delete failed: " .. tostring(err)
			end
			return "Deleted " .. name .. ". Restart the game to refresh the list."
		end)
	end
end

return M
