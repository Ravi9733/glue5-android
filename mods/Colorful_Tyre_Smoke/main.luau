--!strict
-- Colorful Tyre Smoke for GLUE 5. Cars and bikes use the same vehicle natives.
local menu = ui.menu("Colorful Tyre Smoke", "Choose tyre smoke colour")

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
for _, colour in colours do table.insert(names, colour.name) end

local selected = 1
local rainbow = false
local disabled = false

menu:choice("Smoke colour", names, selected, function(index: number)
	selected = index
	rainbow = false
	disabled = false
end)
menu:toggle("Rainbow", false, "Cycle through colours", function(on: boolean)
	rainbow = on
	if on then disabled = false end
end)
menu:toggle("Disable smoke colour", false, "Stop applying colour", function(on: boolean)
	disabled = on
	if on then rainbow = false end
end)

input.onKey(function(key, down)
	if down and key == "F7" then menu:show() end
end)

local function currentColour()
	if not rainbow then return colours[selected] end
	local t = os.clock() * 2.4
	return {
		r = math.floor((math.sin(t) * 0.5 + 0.5) * 255),
		g = math.floor((math.sin(t + 2.094) * 0.5 + 0.5) * 255),
		b = math.floor((math.sin(t + 4.188) * 0.5 + 0.5) * 255),
	}
end

while true do
	if not disabled then
		local me = player.playerPedId()
		if ped.isPedInAnyVehicle(me, false) then
			local car = ped.getVehiclePedIsIn(me, false)
			if car and car ~= 0 then
				local c = currentColour()
				vehicle.toggleVehicleMod(car, 20, true)
				vehicle.setVehicleTyreSmokeColor(car, c.r, c.g, c.b)
			end
		end
	end
	wait(250)
end
