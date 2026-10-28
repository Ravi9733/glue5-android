--!strict
-- Glue5-safe Blue Balls mini-game port.
-- This is a yoga-ball collection mini-game, not the original PC mod's full feature set.
local menu = ui.menu("Blue Balls", "Yoga ball collection mini-game")

local running = false
local score = 0
local balls: { number } = {}
local BALL_MODEL = joaat("prop_poolball_1")

local function removeBall(handle: number)
    pcall(entity.setEntityAsMissionEntity, handle, true, true)
    pcall(entity.deleteEntity, handle)
end

local function clearBalls()
    for _, ball in balls do
        removeBall(ball)
    end
    table.clear(balls)
end

local function spawnBalls()
    local me = player.playerPedId()
    if not me then
        ui.notify("Player not ready yet")
        return
    end
    if not streaming.isModelInCdimage(BALL_MODEL) then
        ui.notify("Yoga ball model is unavailable in this build")
        return
    end
    streaming.requestModel(BALL_MODEL)
    local deadline = misc.getGameTimer() + 8000
    while not streaming.hasModelLoaded(BALL_MODEL) and misc.getGameTimer() < deadline do
        wait()
    end
    if not streaming.hasModelLoaded(BALL_MODEL) then
        ui.notify("Could not load yoga ball model")
        return
    end

    clearBalls()
    local origin = entity.getEntityCoords(me, true)
    for i = 1, 8 do
        local angle = (i / 8) * math.pi * 2
        local pos = origin + vector.create(math.cos(angle) * 3.5, math.sin(angle) * 3.5, 0.5)
        local ball = object.createObject(BALL_MODEL, pos, false, false, true)
        if ball then
            table.insert(balls, ball)
        end
    end
    streaming.setModelAsNoLongerNeeded(BALL_MODEL)
    ui.notify("8 yoga balls spawned nearby")
end

menu:button("Start free play", { note = "Walk close to a ball to collect it", action = "Start" }, function()
    running = true
    ui.notify("Blue Balls: free play started")
end)
menu:button("Spawn yoga balls", { note = "Places 8 collectibles around you", action = "Spawn" }, spawnBalls)
menu:button("Stop game", { action = "Stop" }, function()
    running = false
    ui.notify("Blue Balls paused")
end)
menu:button("Clear balls", { action = "Clear" }, clearBalls)
menu:button("Reset score", { action = "Reset" }, function()
    score = 0
    ui.notify("Score reset")
end)

menu:show()

mod.onStop(function()
    running = false
    clearBalls()
end)

while true do
    if running then
        local me = player.playerPedId()
        if me then
            local pos = entity.getEntityCoords(me, true)
            for i = #balls, 1, -1 do
                local ball = balls[i]
                if not entity.doesEntityExist(ball) then
                    table.remove(balls, i)
                elseif geo.distance(pos, entity.getEntityCoords(ball, true)) < 1.5 then
                    removeBall(ball)
                    table.remove(balls, i)
                    score += 1
                    ui.notify("Collected! Score: " .. tostring(score))
                end
            end
            draw.text("BLUE BALLS  " .. tostring(score), 0.78, 0.12, 0.025, 0x76E6C8FF)
        end
    end
    wait()
end
