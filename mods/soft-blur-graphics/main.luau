-- Glue5 Android: layered Soft Blur + Colorful Graphics.
-- The main timecycle controls colour; the extra timecycle layer controls blur,
-- so toggling one should not replace the other.

local root = ui.menu("Soft Blur + Colorful")

local colorfulOn = true
local blurOn = true
local colorName = "rply_saturation"
local colorStrength = 0.75
local blurName = "hud_def_blur"
local blurStrength = 0.20

local function applyColor()
    if colorfulOn then
        graphics.setTimecycleModifier(colorName)
        graphics.setTimecycleModifierStrength(colorStrength)
    else
        graphics.clearTimecycleModifier()
    end
end

local function applyBlur()
    if blurOn then
        graphics.setExtraTimecycleModifier(blurName)
        graphics.setExtraTimecycleModifierStrength(blurStrength)
    else
        graphics.clearExtraTimecycleModifier()
    end
end

root:button("Soft Blur ON", function()
    blurOn = true
    applyBlur()
    ui.notify("Soft Blur enabled")
end)

root:button("Blur OFF", function()
    blurOn = false
    applyBlur()
    ui.notify("Blur disabled")
end)

root:button("Super Colourful", function()
    colorfulOn = true
    colorName = "rply_saturation"
    colorStrength = 0.75
    applyColor()
    ui.notify("Super Colourful selected")
end)

root:button("Natural Green", function()
    colorfulOn = true
    colorName = "rply_saturation"
    colorStrength = 0.40
    applyColor()
    ui.notify("Natural Green selected")
end)

root:button("Vibrant Day", function()
    colorfulOn = true
    colorName = "rply_saturation"
    colorStrength = 0.60
    applyColor()
    ui.notify("Vibrant Day selected")
end)

root:button("Reset Graphics", function()
    colorfulOn = false
    blurOn = false
    graphics.clearTimecycleModifier()
    graphics.clearExtraTimecycleModifier()
    ui.notify("Graphics effects reset")
end)

-- Apply both layers when the mod starts.
applyColor()
applyBlur()
ui.notify("Soft Blur + Colorful menu loaded")

while true do
    wait()
end
