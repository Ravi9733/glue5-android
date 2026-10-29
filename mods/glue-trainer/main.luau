--!strict
-- Menyoo-style Android Trainer: GLUE 5's trainer. Looks, vehicles, teleports, the world, the camera
-- and other players, from the Mods page's MENUS (F7), the Studio, or its own key (F4).
--
-- Each feature area is a module of features/ with a build(root) that adds its submenu; core/
-- holds what they share (preferences, the per-frame loop, features and key bindings, the
-- game's catalog), data/ the few tables the game does not give.

local keys = require("./core/keys")
local loop = require("./core/loop")
local hub = require("./features/hub")

local root = ui.menu("Menyoo-style Android Trainer", "Vehicles, appearance, teleport, movement and utilities")
keys.action("menu", "Open the trainer (F7)", function()
	root:show()
end)

hub.rows(root)
for _, feature in {
	require("./features/self"),
	require("./features/looks"),
	require("./features/teleport"),
	require("./features/vehicle"),
	require("./features/spawner"),
	require("./features/world"),
	require("./features/movement"),
	require("./features/players"),
	require("./features/misc"),
	require("./features/settings"),
} do
	feature.build(root)
end
-- Search and Favourites list every feature: filled last.
hub.build()

loop.start()
