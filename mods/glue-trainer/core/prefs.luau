--!strict
-- The player's preferences: the mod's settings.json (the defaults below with what the player
-- set over them), written back a moment after the menus change it.

export type Prefs = {
	keys: { [string]: string },
	remember: boolean,
	notify: boolean,
	speedUnit: string,
	flyTeleports: boolean,
	state: { [string]: any },
	favourites: { string },
}

local DEFAULTS: Prefs = {
	keys = {
		menu = "F8",
		["move.noclip"] = "F6",
		["move.freecam"] = "F5",
		["teleport.waypoint"] = "F9",
	},
	remember = true,
	notify = true,
	speedUnit = "km/h",
	flyTeleports = true,
	state = {},
	favourites = {},
}

local prefs = mod.settings(DEFAULTS)

local M = {}

M.data = prefs
M.defaults = DEFAULTS

local pending = false

-- Migrate only the old built-in bindings. Preserve any key the player chose manually.
if prefs.keys.menu == "F7" then
	prefs.keys.menu = "F8"
end
if prefs.keys["move.freecam"] == "F8" then
	prefs.keys["move.freecam"] = "F5"
end

-- Writes settings.json soon (several changes in a row write once).
function M.save()
	if pending then
		return
	end
	pending = true
	task.delay(1, function()
		pending = false
		mod.write("settings.json", json.encode(prefs, true) .. "\n")
	end)
end

-- Persist the default-key migration so the menu and freecam do not share F8.
M.save()

function M.get<T>(key: string, default: T): T
	if not prefs.remember then
		return default
	end
	local v = prefs.state[key]
	if v == nil then
		return default
	end
	return v
end

function M.set(key: string, value: any)
	if prefs.state[key] == value then
		return
	end
	prefs.state[key] = value
	M.save()
end

function M.reset()
	for k, v in DEFAULTS :: any do
		(prefs :: any)[k] = if type(v) == "table" then table.clone(v) else v
	end
	prefs.state = {}
	M.save()
end

return M
