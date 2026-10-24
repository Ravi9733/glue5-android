--!strict
-- GLUE5 Android: armored-player test (built-in GTA V model)
-- IMPORTANT: this is a built-in Juggernaut model, not a custom Iron Man mesh.
-- Glue5 currently does not expose a verified way for a mod to load arbitrary
-- GLB/FBX files as GTA V player peds. See assets/README.md for the custom-asset
-- conversion status.

local ARMORED_MODEL = "u_m_y_juggernaut_01"
local DEFAULT_MODEL = "mp_m_freemode_01"
local busy = false

local function setPlayerModel(modelName: string)
    if busy then
        ui.notify("Please wait: model change is already running")
        return
    end
    busy = true

    local ok, err = pcall(function()
        local hash = joaat(modelName)

        -- Supported Glue5 API: loads a model already present in the game.
        if not streaming.load(hash, nil, 10000) then
            error("Game model did not load: " .. modelName)
        end

        -- SET_PLAYER_MODEL(Player, Hash). This native changes the player's
        -- ped to a model that is already installed in the GTA V game data.
        native.invoke("0x00A1CADD00108836", player.playerId(), hash)
        streaming.setModelAsNoLongerNeeded(hash)
    end)

    busy = false
    if ok then
        ui.notify("Player model changed: " .. modelName)
    else
        ui.notify("Model change failed: " .. tostring(err))
    end
end

local menu = ui.menu("Armored Player Test", "Built-in GTA V model; custom Iron Man not yet loadable")
menu:label("This test uses the game's Juggernaut model.")
menu:label("Custom Iron Man requires a GTA-ready ped asset and loader support.")

menu:button("Use Juggernaut Armor", { action = "Use Armor" }, function()
    setPlayerModel(ARMORED_MODEL)
end)

menu:button("Restore Male Freemode", { action = "Restore" }, function()
    setPlayerModel(DEFAULT_MODEL)
end)

ui.notify("Armored Player Test menu loaded")
