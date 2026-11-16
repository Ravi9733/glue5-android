-- Glue5 Android: lightweight visual presets using GTA V timecycle modifiers.
-- Only one primary modifier can be active at a time in this game API.
local root = ui.menu("Soft Blur & Colors")
local enabled = true
local modifier = "hud_def_blur"
local strength = 0.10

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

local function preset(name, timecycle, amount, message)
    modifier = timecycle
    strength = amount
    enabled = true
    if apply() then ui.notify(message or name .. " enabled") end
end

root:button("Blur: Very Low", function()
    preset("Very Low Blur", "hud_def_blur", 0.06, "Very low blur · 0.06")
end)

root:button("Blur: Low", function()
    preset("Low Blur", "hud_def_blur", 0.10, "Low blur · 0.10")
end)

root:button("Blur: Soft", function()
    preset("Soft Blur", "hud_def_blur", 0.14, "Soft blur · 0.14")
end)

root:button("Colors: Natural", function()
    preset("Natural Colors", "rply_saturation", 0.12, "Natural colors")
end)

root:button("Colors: Light", function()
    preset("Light Colors", "rply_saturation", 0.28, "Light color boost")
end)

root:button("Colors: Bright", function()
    preset("Bright Colors", "rply_saturation", 0.45, "Brighter colors")
end)

root:button("Blur OFF", function()
    enabled = false
    if apply() then ui.notify("Blur and color effect disabled") end
end)

root:button("Reset Graphics", function()
    enabled = false
    if apply() then ui.notify("Graphics effect reset") end
end)

apply()
ui.notify("Soft Blur & Colors menu loaded")

while true do
    wait()
end
