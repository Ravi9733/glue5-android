--!strict
-- GLUE5 Android: armored-player compatibility test.
-- Uses built-in GTA V models; this is NOT a custom Iron Man ped.

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

        -- Match Glue5's documented streaming API and wait for the model to load.
        streaming.requestModel(hash)
        local framesLeft = 600
        while not streaming.hasModelLoaded(hash) and framesLeft > 0 do
            wait()
            framesLeft -= 1
        end
        if not streaming.hasModelLoaded(hash) then
            error("Timed out loading game model: " .. modelName)
        end

        -- SET_PLAYER_MODEL(Player, Hash), for a model already in game data.
        native.invoke("0x00A1CADD00108836", player.playerId(), hash)
        streaming.setModelAsNoLongerNeeded(hash)
    end)

    busy = false
    if ok then
        ui.notify("Requested player model: " .. modelName)
    else
        ui.notify("Model change failed: " .. tostring(err))
    end
end

local menu = ui.menu(
    "Armored Player Test",
    "Built-in GTA V model only; not a custom Iron Man suit"
)
menu:label("Test model: u_m_y_juggernaut_01 (built into GTA V).")
menu:label("Custom Iron Man requires a GTA-ready ped asset and loader support.")

menu:button("Use Juggernaut Armor", {
    note = "Switch to the built-in Juggernaut ped",
    action = "Use Armor"
}, function()
    setPlayerModel(ARMORED_MODEL)
end)

menu:button("Restore Male Freemode", {
    note = "Switch back to the default male freemode ped",
    action = "Restore"
}, function()
    setPlayerModel(DEFAULT_MODEL)
end)

ui.notify("Armored Player Test menu loaded")
