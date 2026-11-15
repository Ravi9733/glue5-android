--!strict
-- Offline stunt-map placement entry point. Imported XML files live in this mod's maps/
-- directory and are spawned by Glue 5's own world placement bridge (no Menyoo/ScriptHook).

local bind = require("../core/bind")
local M = {}

function M.build(root: Menu)
	local menu = root:submenu("STUNT RACES", "Offline stunt maps imported as XML; native Glue 5 object spawning")
	menu:label("XML maps in mods/glue-trainer/maps/ appear here.")
	menu:label("Supports named-model Menyoo placements and CodeWalker CEntityDef XML.")
	menu:label("Numeric ModelHash-only XML is not supported yet.")
	bind.action(menu, "Import XML / ZIP", {
		note = "Choose an XML map or ZIP containing XML maps; restart the game to refresh the list",
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
		menu:label("Could not read the maps folder: " .. tostring(maps))
		return
	end
	if #maps == 0 then
		menu:label("No XML maps imported yet.")
		menu:label("Create maps/ inside glue-trainer and place supported .xml files there.")
		return
	end

	menu:separator("Available stunt maps")
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
end

return M
