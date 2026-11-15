--!strict
-- Native Glue 5 offline stunt maps and checkpoint races; no Menyoo/ScriptHook runtime.

local bind = require("../core/bind")
local game = require("../core/game")
local M = {}

local activeRace = false
local raceVehicle: Vehicle? = nil
local routeBlips: { Blip } = {}

local function clearRoute()
	for _, blip in routeBlips do
		if hud.doesBlipExist(blip) then
			hud.removeBlip(blip)
		end
	end
	table.clear(routeBlips)
end

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
	vehicle.setVehicleOnGroundProperly(car, 5)
	vehicle.setVehicleEngineOn(car, true, true, false)
	ped.setPedIntoVehicle(game.me(), car, VehicleSeat.DRIVER)
	return car, nil
end

local function runRace(routeName: string, modelName: string): string?
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
	activeRace = true
	task.spawn(function()
		local finished = false
		local checkpoint = 1
		local startedAt = 0
		local okRace, raceError = pcall(function()
			game.notify("Stunt race starting")
			for count = 3, 1, -1 do
				game.notify(tostring(count))
				wait(1000)
			end
			game.notify("GO!")
			startedAt = os.clock()
			showRoute(points, 2)
			checkpoint = 2
			while activeRace and checkpoint <= #points do
				wait(0)
				if not raceVehicle or not entity.doesEntityExist(raceVehicle) then
					local replacement, spawnError = spawnRaceVehicle(modelName, points[checkpoint - 1])
					if not replacement then
						error(spawnError or "Race vehicle could not respawn")
					end
					raceVehicle = replacement
				end
				local position = entity.getEntityCoords(raceVehicle, true)
				if vector.magnitude(position - points[checkpoint]) <= 18 then
					game.notify("Checkpoint " .. tostring(checkpoint - 1) .. "/" .. tostring(#points - 1))
					checkpoint += 1
					if checkpoint <= #points then
						showRoute(points, checkpoint)
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
	menu:label("Import one XML map or a ZIP containing XML maps.")
	menu:label("Object XML supports named models and game-resolved ModelHash values.")
	menu:label("Race route XML needs at least two <Checkpoint><Position><X/Y/Z> entries.")

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
	if #maps == 0 then
		menu:label("No XML maps imported yet.")
		return
	end

	local builder = menu:submenu("Create checkpoint route", "Stand at each checkpoint and save a local race route")
	local routeName = builder:text("Route name", "my_stunt_route", { placeholder = "letters, numbers, _ or -", max = 64 })
	local draft: { vector } = {}
	builder:label("Checkpoints captured: 0")
	bind.action(builder, "Add checkpoint at my position", {
		note = "Stand in the middle of the next gate; add checkpoints in driving order",
		action = "Add",
	}, function(): string?
		if #draft >= 128 then
			return "Checkpoint limit reached (128)"
		end
		table.insert(draft, game.position())
		return "Checkpoint " .. tostring(#draft) .. " captured"
	end)
	bind.action(builder, "Undo last checkpoint", { action = "Undo" }, function(): string?
		if #draft == 0 then
			return "No checkpoint to undo"
		end
		table.remove(draft)
		return "Removed last checkpoint; " .. tostring(#draft) .. " remain"
	end)
	bind.action(builder, "Save route XML", { note = "Requires at least two checkpoints; existing files are not overwritten", action = "Save" }, function(): string?
		local okSave, result = pcall(function()
			return world.saveRaceRoute(routeName.value, draft)
		end)
		if not okSave then
			return "Route save failed: " .. tostring(result)
		end
		draft = {}
		return "Saved " .. tostring(result) .. ". Restart Glue 5 to refresh the route list."
	end)

	local raceMenu = menu:submenu("Start checkpoint race", "Select a route XML and race vehicle")
	local route = raceMenu:choice("Route", maps, 1, "XML with Checkpoint position entries")
	local vehicleName = raceMenu:text("Vehicle model", "adder", { placeholder = "e.g. adder, t20, dyfurai", max = 64 })
	bind.action(raceMenu, "Start race", {
		note = "3-second countdown, route checkpoints, timer and finish",
		action = "Race",
	}, function(): string?
		return runRace(maps[route.value], vehicleName.value)
	end)
	bind.action(raceMenu, "Stop race", { action = "Stop" }, function(): string?
		if not activeRace then
			return "No stunt race is running"
		end
		activeRace = false
		clearRoute()
		return "Stopping stunt race"
	end)

	menu:separator("Spawn stunt track objects")
	for _, name in maps do
		bind.action(menu, name, {
			note = "Spawn validated track objects with Glue 5",
			action = "Spawn",
		}, function(): string?
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
