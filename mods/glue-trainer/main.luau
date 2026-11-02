--!strict
-- Menyoo-style Android trainer: touch-accessible features with a quick F8 entry point.
-- The vehicle spawner is first so it is immediately reachable after opening the menu.

local keys = require("./core/keys")
local loop = require("./core/loop")
local hub = require("./features/hub")

local root = ui.menu("Menyoo", "Vehicles, player, world, teleports and settings")
keys.action("menu", "Open Menyoo", function()
	root:show()
end)

hub.rows(root)

-- Put the vehicle catalogue first: search, game classes, and garage.
require("./features/spawner").build(root)
require("./features/customs").build(root, "Vehicle Customizer")

for _, feature in {
	require("./features/self"),
	require("./features/looks"),
	require("./features/teleport"),
	require("./features/vehicle"),
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
