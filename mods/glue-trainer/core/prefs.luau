--!strict
-- The player's preferences: the mod's settings.json (the defaults below with what the player
-- set over them, as mod.settings merges them, written back a moment after the menus change it.
-- Hand-edits of the file count too: it is read at every start.

export type Prefs = {
	-- Key bindings, by action (input key names: "F4", "LeftShift", "PadNorth").
	keys: { [string]: string },
	remember: boolean,
	notify: boolean,
	speedUnit: string,
	flyTeleports: boolean,
	state: { [string]: any },
	favourites: { string },
}

local DEFAULTS: Prefs = {
	-- F7 belongs to GLUE 5's Mods page; F4 opens the trainer directly.
	keys = {
		menu = "F4",
		["move.noclip"] = "F6",
		["move.freecam"] = "F8",
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

-- Migrate the old default: F7 opens GLUE 5's Mods page and is not delivered to mod key handlers.
if prefs.keys.menu == "F7" then
	prefs.keys.menu = "F4"
	M.save()
end

return M
