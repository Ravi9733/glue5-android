-- Glue 5: a single, lightweight Soft Blur graphics mod.
-- Uses the game's primary timecycle modifier (no optional extra layer).
local root = ui.menu("Soft Blur Graphics")
local blurOn = true
local blurStrength = 0.12
local blurName = "hud_def_blur"

local function canApply()
    return type(graphics.setTimecycleModifier) == "function"
        and type(graphics.setTimecycleModifierStrength) == "function"
        and type(graphics.clearTimecycleModifier) == "function"
end

local function apply()
    if not canApply() then
        ui.notify("Soft Blur unavailable: graphics natives are missing")
        return false
    end
    if blurOn then
        graphics.setTimecycleModifier(blurName)
        graphics.setTimecycleModifierStrength(blurStrength)
    else
        graphics.clearTimecycleModifier()
    end
    return true
end

root:button("Soft Blur ON", function()
    blurOn = true
    if apply() then ui.notify("Soft Blur enabled") end
end)

root:button("Soft Blur OFF", function()
    blurOn = false
    if apply() then ui.notify("Soft Blur disabled") end
end)

root:button("Reset Graphics", function()
    blurOn = false
    if canApply() then graphics.clearTimecycleModifier() end
    ui.notify("Graphics reset")
end)

apply()
ui.notify("Soft Blur Graphics loaded")

while true do
    wait()
end
