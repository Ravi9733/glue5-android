-- Glue5 Android: one lightweight soft-blur option, using GTA V's
-- primary timecycle modifier (no optional extra-layer native required).
local root = ui.menu("Soft Blur")
local enabled = true
local modifier = "hud_def_blur"
local strength = 0.18

local function apply()
    if type(graphics.setTimecycleModifier) ~= "function"
        or type(graphics.clearTimecycleModifier) ~= "function" then
        ui.notify("Timecycle modifier is unavailable in this game build")
        return false
    end

    if enabled then
        graphics.setTimecycleModifier(modifier)
        if type(graphics.setTimecycleModifierStrength) == "function" then
            graphics.setTimecycleModifierStrength(strength)
        end
        return true
    end

    graphics.clearTimecycleModifier()
    return true
end

root:button("Soft Blur ON", function()
    enabled = true
    if apply() then ui.notify("Soft Blur enabled") end
end)

root:button("Blur OFF", function()
    enabled = false
    if apply() then ui.notify("Soft Blur disabled") end
end)

root:button("Reset Graphics", function()
    enabled = false
    if apply() then ui.notify("Graphics effect reset") end
end)

apply()
ui.notify("Soft Blur menu loaded")

while true do
    wait()
end
