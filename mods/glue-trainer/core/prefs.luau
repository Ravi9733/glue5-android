--!strict
-- Preferences use Glue5's supported persistent storage API.
-- The former mod.settings/mod.write calls are not part of this VM and caused
-- a nil-call during startup, preventing every trainer menu from being built.

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
		menu = "F7",
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

local saved = storage.get("trainer.preferences")
local prefs: Prefs = table.clone(DEFAULTS)

-- Restore only known fields; keep defaults for missing or malformed values.
if type(saved) == "table" then
	for key, value in saved do
		if key == "keys" and type(value) == "table" then
			prefs.keys = value
		elseif key == "state" and type(value) == "table" then
			prefs.state = value
		elseif key == "favourites" and type(value) == "table" then
			prefs.favourites = value
		elseif key == "remember" and type(value) == "boolean" then
			prefs.remember = value
		elseif key == "notify" and type(value) == "boolean" then
			prefs.notify = value
		elseif key == "flyTeleports" and type(value) == "boolean" then
			prefs.flyTeleports = value
		elseif key == "speedUnit" and type(value) == "string" then
			prefs.speedUnit = value
		end
	end
end

local M = {}
M.data = prefs
M.defaults = DEFAULTS

function M.save()
	storage.set("trainer.preferences", prefs)
end

function M.get<T>(key: string, default: T): T
	if not prefs.remember then
		return default
	end
	local value = prefs.state[key]
	if value == nil then
		return default
	end
	return value
end

function M.set(key: string, value: any)
	if prefs.state[key] == value then
		return
	end
	prefs.state[key] = value
	M.save()
end

function M.reset()
	prefs.keys = table.clone(DEFAULTS.keys)
	prefs.remember = DEFAULTS.remember
	prefs.notify = DEFAULTS.notify
	prefs.speedUnit = DEFAULTS.speedUnit
	prefs.flyTeleports = DEFAULTS.flyTeleports
	prefs.state = {}
	prefs.favourites = {}
	M.save()
end

return M
