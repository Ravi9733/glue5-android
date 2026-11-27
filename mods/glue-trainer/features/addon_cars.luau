--!strict
-- A dedicated top-level page for DLC vehicles. It uses the game's DLC vehicle
-- registry directly, rather than relying on the general world vehicle catalog.

local bind = require("../core/bind")
local game = require("../core/game")
local spawner = require("./spawner")

local M = {}

function M.build(root: Menu)
	local addon = root:submenu("Add-on Cars", "Cars registered by mounted DLC packs")

	local registered = addon:submenu("Registered DLC Cars", "Vehicles the game's DLC loader has registered")
	registered:onOpen(bind.safe("Add-on Cars", function()
		registered:clear()
		local count = extrametadata.getNumDlcVehicles()
		local found = 0
		for i = 0, count - 1 do
			local hash = extrametadata.getDlcVehicleModel(i)
			if hash ~= 0 and streaming.isModelInCdimage(hash) and streaming.isModelAVehicle(hash) then
				found += 1
				local fallback = string.format("DLC vehicle %d", i + 1)
				local name = game.label(vehicle.getDisplayNameFromVehicleModel(hash), fallback)
				bind.action(registered, name, { note = string.format("Model hash: %08X", hash), action = "Spawn" }, function(): string?
					local car, why = spawner.spawn(hash, name)
					return if car then name .. " spawned" else why
				end)
			end
		end
		if found == 0 then
			registered:label("No DLC vehicles are registered. Check the DLC pack and restart the game.")
		end
	end))

	local manual = addon:submenu("Spawn by model name", "Enter the exact modelName from vehicles.meta")
	local modelName = manual:text("Model name", "", { placeholder = "e.g. f40", max = 64 })
	bind.action(manual, "Spawn custom model", { note = "The DLC must be mounted and the model registered", action = "Spawn" }, function(): string?
		local model = string.lower(string.match(modelName.value, "^%s*(.-)%s*$") or "")
		if model == "" or not string.match(model, "^[%w_%-]+$") then
			return "Enter a valid vehicle model name"
		end
		local hash = joaat(model)
		if not streaming.isModelInCdimage(hash) then
			return model .. ": not registered by the game; DLC/RPF mounting did not load"
		end
		if not streaming.isModelAVehicle(hash) then
			return model .. ": registered model is not a vehicle"
		end
		local car, why = spawner.spawn(hash, model)
		return if car then model .. " spawned" else why
	end)
end

return M
