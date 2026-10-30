--!strict
-- Car Customizer: one self-contained GLUE 5 mod file.
-- All menu calls and native names below follow GLUE 5's bundled Glue Trainer.

local root = ui.menu("Car Customizer", "Paint, built-in liveries, neon, tyre smoke and vehicle tuning")
local menu = root -- populate the registered F7 menu directly

local function safe(label: string, f: () -> ())
	return function(...)
		local ok, err = pcall(f :: any, ...)
		if not ok then
			print(`{label}: {err}`)
			ui.notify(`{label}: {err}`)
		end
	end
end

local function rgb(r: number, g: number, b: number): Rgba
	return { r = r, g = g, b = b, a = 255 }
end

local modNames = {
	[0] = "Spoiler", "Front bumper", "Rear bumper", "Side skirt", "Exhaust", "Frame", "Grille",
	"Hood", "Left fender", "Right fender", "Roof", "Engine", "Brakes", "Transmission",
	"Horn", "Suspension", "Armour", "Nitrous", "Turbo", "Subwoofer", "Tyre smoke",
	"Hydraulics", "Xenon lights", "Wheels", "Rear wheels", "Plate holder", "Vanity plate",
	"Trim", "Ornaments", "Dashboard", "Dial", "Door speakers", "Seats", "Steering wheel",
	"Shift lever", "Plaques", "Speakers", "Trunk", "Hydraulics", "Engine block",
	"Air filter", "Struts", "Arch cover", "Aerials", "Trim", "Tank", "Windows", "Window", "Livery"
}
local toggleSlots = { [17] = true, [18] = true, [19] = true, [20] = true, [21] = true, [22] = true }

local function addIndexed(m: Menu, label: string, count: number, current: number, name: (number) -> string, apply: (number) -> ())
	if count <= 0 then
		m:label(label .. ": not supported on this vehicle")
		return
	end
	local options = {}
	for i = 0, count - 1 do
		table.insert(options, name(i))
	end
	m:choice(label, options, math.clamp(current + 1, 1, count), "Select an available option", safe(label, function(i: number)
		apply(i - 1)
	end))
end

local function build(car: Vehicle)
	vehicle.setVehicleModKit(car, 0)

	menu:button("Upgrade everything", { note = "Max available performance and parts", action = "Upgrade" }, safe("Upgrade", function()
		for slot = 0, 48 do
			if toggleSlots[slot] then
				vehicle.toggleVehicleMod(car, slot, true)
			else
				local count = vehicle.getNumVehicleMods(car, slot)
				if count > 0 then vehicle.setVehicleMod(car, slot, count - 1, false) end
			end
		end
		vehicle.setVehicleWindowTint(car, 1)
		ui.notify("Available upgrades applied")
	end))

	menu:separator("Custom paint")
	menu:color("Primary paint", rgb(vehicle.getVehicleCustomPrimaryColour(car)), safe("Primary paint", function(c)
		vehicle.setVehicleCustomPrimaryColour(car, c.r, c.g, c.b)
	end))
	menu:color("Secondary paint", rgb(vehicle.getVehicleCustomSecondaryColour(car)), safe("Secondary paint", function(c)
		vehicle.setVehicleCustomSecondaryColour(car, c.r, c.g, c.b)
	end))
	menu:separator("Neon")
	local neon = menu:toggle("Neon underglow", vehicle.getVehicleNeonEnabled(car, 0), "Enable or disable underglow", safe("Neon", function(on)
		for side = 0, 3 do vehicle.setVehicleNeonEnabled(car, side, on) end
	end))
	menu:color("Neon colour", rgb(vehicle.getVehicleNeonColour(car)), safe("Neon colour", function(c)
		vehicle.setVehicleNeonColour(car, c.r, c.g, c.b)
		for side = 0, 3 do vehicle.setVehicleNeonEnabled(car, side, true) end
		neon.value = true
	end))
	menu:color("Tyre-smoke colour", rgb(vehicle.getVehicleTyreSmokeColor(car)), safe("Tyre smoke", function(c)
		vehicle.toggleVehicleMod(car, 20, true)
		vehicle.setVehicleTyreSmokeColor(car, c.r, c.g, c.b)
	end))

	menu:separator("Stickers, stripes and appearance")
	addIndexed(menu, "Built-in livery / decals", vehicle.getVehicleLiveryCount(car), vehicle.getVehicleLivery(car), function(i)
		return `Design {i + 1}`
	end, function(i)
		vehicle.setVehicleLivery(car, i)
	end)
	addIndexed(menu, "Window tint", vehicle.getNumVehicleWindowTints(), vehicle.getVehicleWindowTint(car), function(i)
		local names = { [0] = "None", "Pure black", "Dark smoke", "Light smoke", "Stock", "Limo", "Green" }
		return names[i] or `Tint {i}`
	end, function(i) vehicle.setVehicleWindowTint(car, i) end)
	addIndexed(menu, "Number-plate style", vehicle.getNumberOfVehicleNumberPlates(), vehicle.getVehicleNumberPlateTextIndex(car), function(i)
		return `Plate style {i + 1}`
	end, function(i) vehicle.setVehicleNumberPlateTextIndex(car, i) end)
	menu:text("Number plate text", string.gsub(vehicle.getVehicleNumberPlateText(car), "%s+$", ""), { max = 8 }, safe("Number plate", function(value)
		vehicle.setVehicleNumberPlateText(car, value)
	end))

	menu:separator("Vehicle extras")
	local foundExtra = false
	for id = 1, 14 do
		if vehicle.doesExtraExist(car, id) then
			foundExtra = true
			menu:toggle(`Extra {id}`, vehicle.isVehicleExtraTurnedOn(car, id), "Enable or disable this vehicle extra", safe(`Extra {id}`, function(on)
				vehicle.setVehicleExtra(car, id, not on)
			end))
		end
	end
	if not foundExtra then menu:label("No extras on this vehicle") end

	menu:separator("Wheels and performance")
	local wheelNames = { "Sport", "Muscle", "Lowrider", "SUV", "Offroad", "Tuner", "Bike", "High end" }
	menu:choice("Wheel type", wheelNames, math.clamp(vehicle.getVehicleWheelType(car) + 1, 1, #wheelNames), "Choose a wheel category", safe("Wheel type", function(i)
		vehicle.setVehicleWheelType(car, i - 1)
	end))
	for slot = 0, 48 do
		if toggleSlots[slot] then
			if slot == 18 or slot == 22 then
				local label = modNames[slot] or `Slot {slot}`
				menu:toggle(label, vehicle.isToggleModOn(car, slot), "Enable or disable this upgrade", safe(label, function(on)
					vehicle.toggleVehicleMod(car, slot, on)
				end))
			end
		else
			local count = vehicle.getNumVehicleMods(car, slot)
			if count > 0 then
				local label = vehicle.getModSlotName(car, slot)
				if label == "" then label = modNames[slot] or `Slot {slot}` end
				local options = { "Stock" }
				for i = 0, count - 1 do
					local text = vehicle.getModTextLabel(car, slot, i)
					table.insert(options, if text ~= "" then text else `{label} {i + 1}`)
				end
				menu:choice(label, options, math.clamp(vehicle.getVehicleMod(car, slot) + 2, 1, #options), "Choose a vehicle part", safe(label, function(i)
					if i == 1 then
						vehicle.removeVehicleMod(car, slot)
					else
						vehicle.setVehicleMod(car, slot, i - 2, false)
					end
				end))
			end
		end
	end
end

menu:onOpen(safe("Car Customizer", function()
	menu:clear()
	local me = player.playerPedId()
	if not ped.isPedInAnyVehicle(me, false) then
		menu:label("Get into a vehicle first, then reopen this menu.")
		return
	end
	local car = ped.getVehiclePedIsIn(me, false)
	if not car then
		menu:label("Could not find your vehicle. Try again.")
		return
	end
	build(car)
end))

ui.notify("Car Customizer loaded. Open F7 > MENUS.")
