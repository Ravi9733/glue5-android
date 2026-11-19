--!strict
-- AI Car Hub: discover existing vehicles and safely fetch community packages.
-- Downloaded add-on DLC is not spawnable until Glue 5 has a native DLC loader.
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
	menu:label("Offline search works with cars already registered in GTA V.")
	menu:label("Online packages can be downloaded, verified and safely extracted; add-on spawning is still in development.")

	menu:separator("Native DLC loader diagnostics")
	local loaderDiagnostics = menu:submenu("Loader diagnostics", "Read-only counters from the game's own DLC lifecycle")
	local loaderStatus = loaderDiagnostics:label("Diagnostics not read yet.")
	local rpfStatus = loaderDiagnostics:label("RPF pipeline: not read.")
	local streamingStatus = loaderDiagnostics:label("Streaming/model pipeline: not read.")
	local deviceStatus = loaderDiagnostics:label("Device mounts: not read.")
	local pathStatus = loaderDiagnostics:label("Last paths: not read.")
	local lifecycleStatus = loaderDiagnostics:label("Content lifecycle: not read.")
	bind.action(loaderDiagnostics, "Refresh loader diagnostics", {
		note = "Reads counters only; does not mount or modify game files",
		action = "Refresh",
	}, function(): string?
		local ok, raw = pcall(function()
			return world.vehicleLoaderDiagnostics()
		end)
		if not ok or type(raw) ~= "string" then
			loaderStatus:set("Diagnostics unavailable: " .. tostring(raw))
			return "Could not read loader diagnostics"
		end
		local parsedOk, data = pcall(function() return json.decode(raw) end)
		if not parsedOk or type(data) ~= "table" then
			loaderStatus:set("Diagnostics response was invalid.")
			return "Invalid diagnostics"
		end
		loaderStatus:set(string.format(
			"CMountable ctor: %s | AddFolder: %s | LoadContent: %s | identity matches: %s | manager observed: %s | last match: %s",
			tostring(data.constructor_calls or 0), tostring(data.add_folder_calls or 0),
			tostring(data.load_calls or 0), tostring(data.load_identity_matches or 0),
			tostring(data.manager_observed or false), tostring(data.last_load_identity_match or false)
		))
		rpfStatus:set(string.format(
			"RPF load: %s | init: %s | mount: %s | AddRpfFile: %s | AddAllRpfFiles: %s | RegisterFile: %s",
			tostring(data.rpf_load_calls or 0), tostring(data.rpf_init_calls or 0),
			tostring(data.rpf_mount_calls or 0), tostring(data.datafile_add_calls or 0),
			tostring(data.add_all_rpf_calls or 0), tostring(data.register_file_calls or 0)
		))
		streamingStatus:set(string.format(
			"Streaming module: %s | file index: %s | requests: %s | loads: %s | model lookups: %s",
			tostring(data.stream_module_calls or 0), tostring(data.stream_index_calls or 0),
			tostring(data.stream_request_calls or 0), tostring(data.stream_load_calls or 0),
			tostring(data.model_lookup_calls or 0)
		))
		deviceStatus:set(string.format(
			"Mounts: %s | unmounts: %s | device lookups: %s | folder scans: %s",
			tostring(data.device_mount_calls or 0), tostring(data.device_unmount_calls or 0),
			tostring(data.device_lookups or 0), tostring(data.recursive_enumerations or 0)
		))
		pathStatus:set(string.format("RPF: %s | mount: %s | content folder: %s",
			tostring(data.last_rpf_path or "(none)"), tostring(data.last_mount_path or "(none)"),
			tostring(data.last_content_folder or "(none)")))
		lifecycleStatus:set(string.format("Enumerate: %s | update array: %s | change groups: %s | manager destroyed: %s | content shutdown: %s",
			tostring(data.begin_enumerate_calls or 0), tostring(data.update_content_array_calls or 0),
			tostring(data.execute_change_group_calls or 0), tostring(data.manager_destructor_calls or 0),
			tostring(data.mountable_shutdown_calls or 0)))
		return "Loader diagnostics refreshed"
	end)
	loaderDiagnostics:label("Paths are truncated to 192 characters. Observations only; no DLC mounting or vehicle spawning is enabled.")
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

	menu:separator("Installed vehicle packages")
	local installed = menu:submenu("Installed downloads", "Verified packages staged in private Glue 5 storage")
	local installedStatus = installed:label("Installed package list not loaded.")
	local installedRows = installed:submenu("Installed packages", "These files are staged; they are not yet mounted or spawnable.")
	installedRows:label("Tap Refresh installed list to inspect staged packages.")
	bind.action(installed, "Refresh installed list", {
		note = "Lists verified package folders without changing game data",
		action = "Refresh",
	}, function(): string?
		local ok, raw = pcall(function()
			return world.listInstalledVehiclePackages()
		end)
		if not ok or type(raw) ~= "string" then
			installedStatus:set("Installed list unavailable: " .. tostring(raw))
			return "Could not read installed packages"
		end
		local parsedOk, ids = pcall(function() return json.decode(raw) end)
		if not parsedOk or type(ids) ~= "table" then
			installedStatus:set("Installed list could not be parsed.")
			return "Invalid installed package list"
		end
		installedRows:clear()
		if #ids == 0 then
			installedRows:label("No staged vehicle packages found.")
		else
			for _, package in ids do
				if type(package) == "table" and type(package.id) == "string" then
					local title = type(package.name) == "string" and package.name or package.id
					local row = installedRows:submenu(title, package.id .. " | " .. tostring(package.format or "unknown"))
					row:label("Package ID: " .. package.id)
					row:label("Version: " .. tostring(package.version or "unknown"))
					row:label("Author: " .. tostring(package.author or "unknown"))
					row:label("Extracted files: " .. tostring(package.extracted_files or 0))
					row:label("Extracted bytes: " .. tostring(package.extracted_bytes or 0))
					row:label("RPF archives: " .. tostring(package.rpf_archives or 0))
					row:label("Vehicle assets: " .. tostring(package.vehicle_assets or 0))
					row:label("Status: " .. tostring(package.status or "staged_not_mounted"))
					row:label("Native DLC mount and vehicle spawning are not enabled yet.")
				end
			end
		end
		installedStatus:set("Found " .. tostring(#ids) .. " staged package(s).")
		return "Installed package list refreshed"
	end)

	menu:separator("Online vehicle catalog")
	local settings = menu:submenu("AI Car Hub settings", "Choose how downloaded cars are handled")
	settings:label("One-tap mode: download, verify, then automatically install the package.")
	settings:label("Direct in-game DLC mounting and spawning are locked until the native loader is implemented and tested.")
	settings:label("AI search currently uses offline matching; live AI recommendations need a populated catalog and service integration.")

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
									onlineStatus:set("Download is still running; check its status shortly.")
								end)
								return "Download started for " .. packageName
							end)
							bind.action(row, "Download + install (one tap)", {
								note = "Automatically verifies and stages the car after download; live game mounting is not enabled yet",
								action = "Download and install",
							}, function(): string?
								local queued, queueError = pcall(function()
									world.requestVehiclePackageDownload(packageId)
								end)
								if not queued then return "Cannot start download: " .. tostring(queueError) end
								onlineStatus:set("Downloading and verifying " .. packageName .. "…")
								task.spawn(function()
									local downloaded = false
									for _ = 1, 600 do
										wait(500)
										local ok, done, message = pcall(function()
											return world.vehiclePackageDownloadStatus()
										end)
										if ok and done then
											onlineStatus:set(tostring(message))
											downloaded = true
											break
										end
									end
									if not downloaded then
										onlineStatus:set("Download is still running or timed out; install was not started.")
										return
									end
									local installOk, installError = pcall(function()
										world.requestVehiclePackageInstall(packageId)
									end)
									if not installOk then
										onlineStatus:set("Downloaded, but install could not start: " .. tostring(installError))
										return
									end
									onlineStatus:set("Download verified. Installing " .. packageName .. "…")
									for _ = 1, 600 do
										wait(500)
										local ok, done, succeeded, message = pcall(function()
											return world.vehiclePackageInstallStatus()
										end)
										if ok and done then
											onlineStatus:set(tostring(message) .. " | Game spawn is not enabled yet.")
											return
										end
									end
									onlineStatus:set("Install is still running; check status shortly. Game spawn is not enabled yet.")
								end)
								return "One-tap download and install started for " .. packageName
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
									onlineStatus:set("Install is still running; check its status shortly.")
								end)
								return "Install started for " .. packageName
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
	online:label("Downloads are streamed and SHA-256 verified into private app storage. Install safely extracts and inspects the package.")
	online:label("One-tap download + install is available per car. It stages verified files only; RPF/DLC mounting and actual in-game spawning are not implemented yet.")
end

return M
