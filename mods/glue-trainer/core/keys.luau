--!strict
-- Key bindings are optional in this Glue5 Android runtime.
-- This build does not expose the input.onKey API. Registering it at module
-- load time crashes the whole trainer before its menus can be added.
-- Open Glue Trainer from the Glue5 Mods page instead of a keyboard shortcut.

export type Action = { name: string, label: string, run: () -> () }

local actions: { Action } = {}
local M = {}

function M.action(name: string, label: string, run: () -> ())
	table.insert(actions, { name = name, label = label, run = run })
end

function M.actions(): { Action }
	return actions
end

return M
