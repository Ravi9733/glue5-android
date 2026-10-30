--!strict
-- Colorful Tyre Smoke. Its menu is listed automatically under Mods > MENUS (F7).
-- F7 belongs to the GLUE 5 app; this mod must not intercept it.

local menu = ui.menu("Colorful Tyre Smoke", "Tyre smoke colours for cars and bikes")

local colours = {
	{ name = "White", r = 255, g = 255, b = 255 },
	{ name = "Red", r = 255, g = 35, b = 35 },
	{ name = "Blue", r = 35, g = 90, b = 255 },
	{ name = "Green", r = 35, g = 255, b = 70 },
	{ name = "Purple", r = 160, g = 45, b = 255 },
	{ name = "Pink", r = 255, g = 65, b = 180 },
	{ name = "Yellow", r = 255, g = 230, b = 25 },
	{ name = "Orange", r = 255, g = 120, b = 20 },
	{ name = "Cyan", r = 20, g = 230, b = 255 },
}
local names = {}
for _, c in colours do
	table.insert(names, c.name)
end

local selected = 1
local rainbow = false
local enabled = true

local function applyColour()
	local me = player.playerPedId()
	if not ped.isPedInAnyVehicle(me, false) then
		ui.notify("Get into a car or bike first")
		return
	end
	local veh = ped.getVehiclePedIsIn(me, false)
	if not veh or veh == 0 then return end
	local c = colours[selected]
	vehicle.toggleVehicleMod(veh, 20, true)
	vehicle.setVehicleTyreSmokeColor(veh, c.r, c.g, c.b)
	ui.notify(c.name .. " tyre smoke applied")
end

menu:choice("Smoke colour", names, "White", function(index: number, _name: string)
	selected = index
	rainbow = false
	enabled = true
end)

menu:button("Apply Colour", { note = "Apply selected colour to your current vehicle", action = "Apply" }, applyColour)

menu:toggle("Rainbow", false, "Cycle through rainbow colours", function(on: boolean)
	rainbow = on
	enabled = true
end)

menu:toggle("Disable Rainbow", false, "Turn rainbow cycling off", function(on: boolean)
	if on then
		rainbow = false
	end
end)

menu:toggle("Enable smoke colour", true, "Apply the selected or rainbow colour while driving", function(on: boolean)
	enabled = on
end)

local function rainbowColour()
	local t = os.clock() * 2.4
	return {
		r = math.floor((math.sin(t) * 0.5 + 0.5) * 255),
		g = math.floor((math.sin(t + 2.094) * 0.5 + 0.5) * 255),
		b = math.floor((math.sin(t + 4.188) * 0.5 + 0.5) * 255),
	}
end

while true do
	if enabled then
		local me = player.playerPedId()
		if ped.isPedInAnyVehicle(me, false) then
			local veh = ped.getVehiclePedIsIn(me, false)
			if veh and veh ~= 0 then
				local c = if rainbow then rainbowColour() else colours[selected]
				vehicle.toggleVehicleMod(veh, 20, true)
				vehicle.setVehicleTyreSmokeColor(veh, c.r, c.g, c.b)
			end
		end
	end
	wait(250)
end
