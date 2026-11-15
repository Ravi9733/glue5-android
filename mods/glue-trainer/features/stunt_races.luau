--!strict
-- Offline stunt-map placement entry point. Imported XML files live in this mod's maps/
-- directory and are spawned by Glue 5's own world placement bridge (no Menyoo/ScriptHook).

local bind = require("../core/bind")
local M = {}

local function safeFileName(value: string): string?
	local name = string.match(value, "^%s*(.-)%s*$") or ""
	if name == "" then
		return nil
	end
	if not string.match(name, "^[%w_%-]+%.xml$") then
		return nil
	end
	return name
end

function M.build(root: Menu)
	local menu = root:submenu("STUNT RACES", "Offline stunt maps imported as XML; native Glue 5 object spawning")
	menu:label("Put supported XML map files in mods/glue-trainer/maps/.")
	menu:label("This first importer stage supports named-model Menyoo placements and CodeWalker CEntityDef XML.")
	menu:label("Numeric ModelHash-only XML and ZIP import are not supported yet.")
	local file = menu:text("Map XML file", "", {
		placeholder = "example: stunt_race.xml",
		max = 80,
	})
	bind.action(menu, "Spawn stunt map", {
		note = "Loads up to 512 validated objects from maps/<file>",
		action = "Spawn",
	}, function(): string?
		local name = safeFileName(file.value)
		if not name then
			return "Enter a file name ending in .xml (letters, numbers, _ or - only)"
		end
		local ok, result = pcall(function()
			return world.spawnPlacements("maps/" .. name)
		end)
		if not ok then
			return "Map load failed: " .. tostring(result)
		end
		return "Map spawning started: " .. name
	end)
end

return M
