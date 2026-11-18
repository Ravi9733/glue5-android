--!strict
-- AI Car Hub entry point. This first UI milestone searches vehicles already
-- registered by the game; it never labels an unmounted ZIP/DLC package as spawnable.
local bind = require("../core/bind")
local catalog = require("../core/catalog")
local spawner = require("./spawner")

local M = {}

local function matches(entry: catalog.VehicleEntry, query: string): boolean
	local q = string.lower(query)
	return q == "" or string.find(string.lower(entry.name), q, 1, true) ~= nil
		or string.find(string.lower(entry.model), q, 1, true) ~= nil
end

function M.build(root: Menu)
	local menu = root:submenu("AI Car Hub", "Search, discover and deliver cars")
	menu:label("Smart search works offline with cars already registered in GTA V.")
	menu:label("Online car downloads and add-on DLC spawning are not connected yet.")
	local query = menu:text("Search cars", "", { placeholder = "e.g. Mazda, Ferrari, drift, adder", max = 48 })
	local results = menu:submenu("Search results", "Cars currently available in loaded game data")
	local status = results:label("Enter a name or model, then choose Search.")
	local function search()
		results:clear()
		local q = string.lower(string.match(query.value, "^%s*(.-)%s*$") or "")
		local all = catalog.vehicles()
		local found = 0
		for _, entry in all do
			if matches(entry, q) then
				found += 1
				if found <= 40 then
					bind.action(results, entry.name, {
						note = entry.model,
						action = "Spawn",
					}, function(): string?
						local car, why = spawner.spawn(entry.hash, entry.name)
						return if car then entry.name .. " delivered" else why
					end)
				end
			end
		end
		if found == 0 then
			results:label(if #all == 0 then "The game vehicle catalog is not ready yet." else "No matching registered cars found.")
		else
			results:label(string.format("%d matching car(s); showing up to 40.", found))
		end
	end
	bind.action(menu, "Search", {
		note = "Find matching cars in the game's loaded vehicle catalog",
		action = "Search",
	}, function(): string?
		search()
		return "Search complete"
	end)
	local suggestions = menu:submenu("Quick discovery", "Common searches")
	for _, term in { "super", "sport", "offroad", "muscle", "drift" } do
		bind.action(suggestions, term, { action = "Find" }, function(): string?
			query.value = term
			search()
			return "Showing matches for " .. term
		end)
	end
	menu:separator("Online downloads")
	menu:label("Coming next: connect the verified HTTPS catalog and Download → Verify → Install flow.")
	menu:label("Add-on cars will appear here only after the native RPF/DLC loader is implemented and tested.")
end

return M
