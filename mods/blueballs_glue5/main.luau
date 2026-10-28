--!nonstrict
-- Blue Balls: a Glue5-native, gameplay-safe collectible mini-game.
-- This is a reimplementation for Glue5, not the original PC script.
local menu = ui.menu("Blue Balls", "Collect the beach balls")

local balls = {}
local running = false
local score = 0
local scoreLabel = menu:label("Score: 0")

local function removeBall(ball)
    if ball and entity.doesEntityExist(ball) then
        entity.setEntityAsMissionEntity(ball, true, true)
        object.deleteObject(ball)
    end
end

local function clearBalls()
    for _, ball in balls do
        removeBall(ball)
    end
    table.clear(balls)
end

local function stopGame()
    running = false
    clearBalls()
    ui.notify("Blue Balls stopped. Final score: " .. tostring(score))
end

menu:button("Start / reset game", { note = "Spawn 5 balls near your character" }, function()
    running = false
    clearBalls()
    score = 0
    scoreLabel.label = "Score: 0"

    local me = player.playerPedId()
    local base = entity.getEntityCoords(me, true)
    local forward = entity.getEntityForwardVector(me)
    local made = 0

    for i = 1, 5 do
        local pos = base + forward * (3 + i * 2)
        local ball = spawn.prop("prop_beachball_02", pos, { dynamic = true, ground = true })
        if ball then
            table.insert(balls, ball)
            made += 1
        end
    end

    if made == 0 then
        ui.notify("Could not spawn balls. Try again in the open world.")
        return
    end

    running = true
    ui.notify("Game started! Walk close to each ball to collect it.")

    while running do
        local ped = player.playerPedId()
        local here = entity.getEntityCoords(ped, true)

        for i = #balls, 1, -1 do
            local ball = balls[i]
            if not entity.doesEntityExist(ball) then
                table.remove(balls, i)
            else
                local there = entity.getEntityCoords(ball, true)
                if geo.distance(here, there) < 2.2 then
                    removeBall(ball)
                    table.remove(balls, i)
                    score += 1
                    scoreLabel.label = "Score: " .. tostring(score)
                    ui.notify("Ball collected! Score: " .. tostring(score))
                end
            end
        end

        if #balls == 0 then
            running = false
            ui.notify("All balls collected! Final score: " .. tostring(score))
        end
        wait(250)
    end
end)

menu:button("Stop and clear balls", { note = "Remove any remaining collectibles" }, function()
    stopGame()
end)

mod.onStop(function()
    running = false
    clearBalls()
end)
