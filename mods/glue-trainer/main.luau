--!strict
-- Glue Trainer: GLUE 5's trainer. Looks, vehicles, teleports, the world, the camera
-- and other players, from the Mods page's MENUS (F7), the Studio, or its own key (F4).
--
-- Each feature area is a module of features/ with a build(root) that adds its submenu; core/
-- holds what they share (preferences, the per-frame loop, features and key bindings, the game's catalog.

local keys = require("./core/keys")
local loop = require("./core/loop")
local hub = require("./features/hub")

local root = ui.menu("Glue Trainer", "You, vehicles, teleports and the world")
keys.action("menu", "Open the trainer", function()
	root:show()
end)

hub.rows(root)

-- Put this first so it is immediately visible in the F7 / touch Trainer menu.
require("./features/customs").build(root, "Car Customizer")

-- Keep the trainer usable if the AI Car Hub module or one of its UI bindings
-- fails on a particular game build. Always expose a diagnostic submenu.
local aiHubOk, aiHub = pcall(require, "./features/ai_car_hub")
if aiHubOk then
	local built, buildError = pcall(aiHub.build, root)
	if not built then
		local recovery = root:submenu("AI Car Hub", "AI vehicle discovery diagnostics")
		recovery:label("AI Car Hub failed to initialize: " .. tostring(buildError))
	end
else
	local recovery = root:submenu("AI Car Hub", "AI vehicle discovery diagnostics")
	recovery:label("AI Car Hub module failed to load: " .. tostring(aiHub))
end

-- Load Stunt Races independently and first. A syntax/runtime error in one feature
-- must never abort the whole trainer before its other menus can be registered.
local stuntOk, stuntFeature = pcall(require, "./features/stunt_races")
if stuntOk then
	local buildOk, buildError = pcall(stuntFeature.build, root)
	if not buildOk then
		local recovery = root:submenu("STUNT RACES", "Stunt race import recovery")
		recovery:label("Full race menu error: " .. tostring(buildError))
		recovery:button("Import XML / ZIP", { note = "Choose a downloaded stunt-map XML or ZIP", action = "Import" }, function()
			local ok, err = pcall(world.requestPlacementImport)
			ui.notify(if ok then "Choose a stunt-map XML or ZIP" else "XML import unavailable: " .. tostring(err))
		end)
		recovery:button("Import Rockstar JSON / ZIP", { note = "Choose a Rockstar Creator race JSON or ZIP", action = "Import" }, function()
			local ok, err = pcall(world.requestRaceImport)
			ui.notify(if ok then "Choose a Rockstar Creator JSON or ZIP" else "Race import unavailable: " .. tostring(err))
		end)
	end
else
	local recovery = root:submenu("STUNT RACES", "Stunt race import recovery")
	recovery:label("Stunt race module failed to load: " .. tostring(stuntFeature))
	recovery:button("Import XML / ZIP", { note = "Choose a downloaded stunt-map XML or ZIP", action = "Import" }, function()
		local ok, err = pcall(world.requestPlacementImport)
		ui.notify(if ok then "Choose a stunt-map XML or ZIP" else "XML import unavailable: " .. tostring(err))
	end)
	recovery:button("Import Rockstar JSON / ZIP", { note = "Choose a Rockstar Creator race JSON or ZIP", action = "Import" }, function()
		local ok, err = pcall(world.requestRaceImport)
		ui.notify(if ok then "Choose a Rockstar Creator JSON or ZIP" else "Race import unavailable: " .. tostring(err))
	end)
end

-- Isolate the rest of the feature modules too: one bad feature must not hide
-- the menus registered before or after it.
for _, path in {
	"./features/self",
	"./features/looks",
	"./features/teleport",
	"./features/vehicle",
	"./features/spawner",
	"./features/world",
	"./features/movement",
	"./features/players",
	"./features/misc",
	"./features/settings",
} do
	local ok, feature = pcall(require, path)
	if ok then
		local built, err = pcall(feature.build, root)
		if not built then
			local diagnostic = root:submenu("Feature error", path)
			diagnostic:label(tostring(err))
		end
	else
		local diagnostic = root:submenu("Feature load error", path)
		diagnostic:label(tostring(feature))
	end
end

-- Search and Favourites list every feature: filled last.
hub.build()

loop.start()
