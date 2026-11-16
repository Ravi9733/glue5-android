-- Glue5 Android: combine the soft GTA timecycle blur with a separate
-- extra timecycle modifier for the colorful/saturated look. GTA V supports
-- one primary and one extra modifier at the same time.
local root = ui.menu("Soft Blur & Colors")
local enabled = true
local blur_modifier = "hud_def_blur"
local blur_strength = 0.14
local color_modifier = "rply_saturation"
local color_strength = 0.30

local function apply_blur()
    if type(graphics.setTimecycleModifier) ~= "function"
        or type(graphics.clearTimecycleModifier) ~= "function" then
        ui.notify("Primary timecycle modifier is unavailable")
        return false
    end

    if enabled then
        graphics.setTimecycleModifier(blur_modifier)
        if type(graphics.setTimecycleModifierStrength) == "function" then
            graphics.setTimecycleModifierStrength(blur_strength)
        end
    else
        graphics.clearTimecycleModifier()
    end
    return true
end

local function apply_color()
    -- The extra modifier is independent of the primary blur modifier.
    -- Keep the blur active while boosting saturation.
    if type(graphics.setExtraTimecycleModifier) ~= "function"
        or type(graphics.clearExtraTimecycleModifier) ~= "function" then
        ui.notify("Extra color timecycle modifier is unavailable in this game build")
        return false
    end

    if color_strength > 0.0 then
        graphics.setExtraTimecycleModifier(color_modifier)
        if type(graphics.setExtraTimecycleModifierStrength) == "function" then
            graphics.setExtraTimecycleModifierStrength(color_strength)
        end
    else
        graphics.clearExtraTimecycleModifier()
    end
    return true
end

local function apply_all()
    local blur_ok = apply_blur()
    local color_ok = apply_color()
    return blur_ok and color_ok
end

local function set_blur(amount, message)
    blur_strength = amount
    enabled = true
    apply_blur()
    ui.notify(message)
end

local function set_color(amount, message)
    color_strength = amount
    apply_color()
    ui.notify(message)
end

root:button("Soft + Colorful (Default)", function()
    blur_strength = 0.14
    color_strength = 0.30
    enabled = true
    if apply_all() then ui.notify("Soft + Colorful enabled") end
end)

root:button("Blur: Very Low", function()
    set_blur(0.06, "Very low soft blur · Colorful look kept")
end)

root:button("Blur: Low", function()
    set_blur(0.10, "Low soft blur · Colorful look kept")
end)

root:button("Blur: Soft", function()
    set_blur(0.14, "Soft blur · Colorful look kept")
end)

root:button("Colors: Natural", function()
    set_color(0.12, "Natural colors · Blur kept")
end)

root:button("Colors: Light", function()
    set_color(0.22, "Light color boost · Blur kept")
end)

root:button("Colors: Bright", function()
    set_color(0.30, "Bright green and colorful look · Blur kept")
end)

root:button("Blur OFF (Keep Colors)", function()
    enabled = false
    if apply_blur() then ui.notify("Blur OFF · Colorful effect kept") end
end)

root:button("Colors OFF (Keep Blur)", function()
    color_strength = 0.0
    if apply_color() then ui.notify("Color boost OFF · Blur kept") end
end)

root:button("Reset Graphics", function()
    enabled = false
    color_strength = 0.0
    local blur_ok = apply_blur()
    local color_ok = apply_color()
    if blur_ok and color_ok then ui.notify("Graphics effects reset") end
end)

apply_all()
ui.notify("Soft + Colorful graphics loaded")

while true do
    wait()
end
