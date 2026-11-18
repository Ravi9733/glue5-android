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
	menu:label("Online packages can be downloaded, SHA-256 verified and safely extracted. Native add-on spawning is still under development.")
	local query = menu:text("Search cars", "", { placeholder = "e.g. Mazda, Ferrari, drift, adder", max = 48 })
	local results = menu:submenu("Search results", "Cars currently available in loaded game data")
	results:label("Enter a name or model, then choose Search.")
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
	menu:separator("Online vehicle catalog")
	local online = menu:submenu("Browse online cars", "Fetch the curated HTTPS catalog maintained for Glue 5")
	local onlineStatus = online:label("Catalog not fetched yet.")
	local onlineResults = online:submenu("Catalog vehicles", "Packages published in the Glue 5 catalog")
	bind.action(online, "Refresh online catalog", {
		note = "Downloads a small HTTPS JSON catalog; no game files are changed",
		action = "Refresh",
	}, function(): string?
		local requested, requestError = pcall(function()
			world.requestVehicleCatalog()
		end)
		if not requested then return "Catalog request unavailable: " .. tostring(requestError) end
		onlineStatus:set("Connecting securely to Glue 5 catalog…")
		task.spawn(function()
			for _ = 1, 80 do
				wait(250)
				local statusOk, done, succeeded, message = pcall(function()
					return world.vehicleCatalogStatus()
				end)
				if statusOk and done then
					if not succeeded then
						onlineStatus:set("Catalog error: " .. tostring(message))
						return
					end
					local raw = world.vehicleCatalogJson()
					if not raw then
						onlineStatus:set("Catalog fetched, but cached data is missing.")
						return
					end
					local parsedOk, parsed = pcall(function() return json.decode(raw) end)
					if not parsedOk or type(parsed) ~= "table" or type(parsed.vehicles) ~= "table" then
						onlineStatus:set("Catalog rejected: invalid JSON structure.")
						return
					end
					onlineResults:clear()
					local count = 0
					for _, entry in parsed.vehicles do
						if type(entry) == "table" and type(entry.name) == "string"
							and type(entry.id) == "string" and type(entry.author) == "string" then
							count += 1
							local packageName = entry.name
							local packageId = entry.id
							local packageFormat = tostring(entry.format or "unknown")
							local row = onlineResults:submenu(packageName, "by " .. entry.author .. " | v"
								.. tostring(entry.version or "?") .. " | " .. packageFormat)
							row:label("Package size: " .. tostring(entry.size_bytes or "unknown") .. " bytes")
							bind.action(row, "Download and verify", {
								note = "Streams package to private app storage and checks SHA-256",
								action = "Download",
							}, function(): string?
								local queued, queueError = pcall(function()
									world.requestVehiclePackageDownload(packageId)
								end)
							bind.action(row, "Install downloaded package", {
								note = "Re-checks SHA-256, safely extracts ZIP and inspects GTA vehicle assets",
								action = "Install",
							}, function(): string?
								local queued, queueError = pcall(function()
									world.requestVehiclePackageInstall(packageId)
								end)
								if not queued then return "Cannot install package: " .. tostring(queueError) end
								onlineStatus:set("Safely installing " .. packageName .. "…")
								task.spawn(function()
									for _ = 1, 600 do
										wait(500)
										local statusOk, done, succeeded, message = pcall(function()
											return world.vehiclePackageInstallStatus()
										end)
										if statusOk and done then
											onlineStatus:set(tostring(message))
											return
										end
									end
									onlineStatus:set("Install is still running. Check the package status again shortly.")
								end)
								return "Install started for " .. packageName
							end)
								if not queued then return "Cannot start download: " .. tostring(queueError) end
								onlineStatus:set("Downloading " .. packageName .. "…")
								task.spawn(function()
									for _ = 1, 600 do
										wait(500)
										local statusOk, done, message = pcall(function()
											return world.vehiclePackageDownloadStatus()
										end)
										if statusOk and done then
											onlineStatus:set(tostring(message))
											return
										end
									end
									onlineStatus:set("Download is still running. Check Installed/Downloads later.")
								end)
								return "Download started for " .. packageName
							end)

						end
					end
					onlineStatus:set("Catalog updated: " .. tostring(count) .. " package(s) listed.")
					if count == 0 then
						onlineResults:label("No vehicle packages have been published yet. The secure catalog connection is ready.")
					end
					return
				end
			end
			onlineStatus:set("Catalog request timed out. Check internet access and try again.")
		end)
		return "Secure catalog request started"
	end)
	online:label("Downloads are streamed and SHA-256 verified into private app storage. Install safely extracts and inspects the package; it does not yet mount add-on DLC into the game.")
	online:label("Add-on RPF/DLC vehicles remain non-spawnable until native runtime mounting is implemented and verified.")
end

return M
