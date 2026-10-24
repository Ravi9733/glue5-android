--!strict
-- GLUE5_ANDROID_ARMORED_PLAYER_REPLACEMENT_V1
-- Replaces the current player with an armored model already installed in the game.
-- This is a built-in GTA model, NOT a custom Iron Man mesh.

local currentModel = "u_m_y_juggernaut_01"

local function replacePlayer(modelName: string)
    local hash = joaat(modelName)
    streaming.requestModel(hash)
    local deadline = 10000
    while not streaming.hasModelLoaded(hash) and deadline > 0 do
        wait(50)
        deadline -= 50
    end
    if not streaming.hasModelLoaded(hash) then
        error("Model failed to load: " .. modelName)
    end

    -- SET_PLAYER_MODEL(Player, Hash)
    native.invoke("0x00A1CADD00108836", player.playerId(), hash)
    streaming.setModelAsNoLongerNeeded(hash)
    currentModel = modelName
end

local menu = ui.menu("Armored Player + Flight")
menu:label("Uses built-in game models. Custom Iron Man assets are not loaded.")

menu:button("Replace Player: Juggernaut Armor", function()
    local ok, err = pcall(function()
        replacePlayer("u_m_y_juggernaut_01")
    end)
    ui.notify(ok and "Player model changed to Juggernaut" or ("Replacement failed: " .. tostring(err)))
end)

menu:button("Restore Male Freemode", function()
    local ok, err = pcall(function()
        replacePlayer("mp_m_freemode_01")
    end)
    ui.notify(ok and "Male freemode model restored" or ("Restore failed: " .. tostring(err)))
end)

menu:button("Spawn Armored NPC (optional)", function()
    local ok, err = pcall(function()
        local hash = joaat("u_m_y_juggernaut_01")
        streaming.requestModel(hash)
        while not streaming.hasModelLoaded(hash) do
            wait(50)
        end
        local ped = spawn.ped(hash)
        streaming.setModelAsNoLongerNeeded(hash)
        if not ped then
            error("Could not spawn armored NPC")
        end
    end)
    ui.notify(ok and "Armored NPC spawned" or ("Spawn failed: " .. tostring(err)))
end)

ui.notify("Armored player replacement menu loaded")
