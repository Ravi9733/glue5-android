--!strict
-- Standalone Car Customizer. Its top-level menu is registered before loading features,
-- so F7 > MENUS always has a visible entry even if a feature module reports an error.

local root = ui.menu("Car Customizer", "Vehicle paint, built-in designs, neon, tyre smoke and tuning")

local ok, customs = pcall(require, "./features/customs")
if ok and customs then
	local loaded, err = pcall(function()
		customs.build(root)
	end)
	if not loaded then
		root:label("Customizer feature error: " .. tostring(err))
		print("Car Customizer feature error: " .. tostring(err))
		ui.notify("Car Customizer feature error: " .. tostring(err))
	end
else
	root:label("Customizer module failed to load: " .. tostring(customs))
	print("Car Customizer module error: " .. tostring(customs))
	ui.notify("Car Customizer module error: " .. tostring(customs))
end

ui.notify("Car Customizer menu registered — F7 > MENUS")
