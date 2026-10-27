-- Glue5 Android: Colorful / Clear Graphics + Blur ON/OFF
-- Uses the same UI and timecycle API as the existing RealityV graphics mod.
-- Note: Glue5's Lua API does not expose a verified shader-sharpening control here;
-- "Clear/Sharp look" uses a high-contrast preset, not true image sharpening.

local root = ui.menu("Colorful Sharp Graphics", "Colorful preset and blur toggle")

local selectedModifier = "vivid"
local selectedStrength = 0.50
local blurEnabled = false

local function applyCurrent()
    if blurEnabled then
        graphics.setTimecycleModifier("hud_def_blur")
        graphics.setTimecycleModifierStrength(0.20)
    else
        graphics.setTimecycleModifier(selectedModifier)
        graphics.setTimecycleModifierStrength(selectedStrength)
    end
end

root:action("Colorful Graphics ON", function()
    selectedModifier = "vivid"
    selectedStrength = 0.50
    blurEnabled = false
    applyCurrent()
end)

root:action("Clear / High-Contrast Look", function()
    selectedModifier = "NG_filmic01"
    selectedStrength = 0.45
    blurEnabled = false
    applyCurrent()
end)

root:action("Blur ON", function()
    blurEnabled = true
    applyCurrent()
end)

root:action("Blur OFF", function()
    blurEnabled = false
    applyCurrent()
end)

root:action("Reset Graphics", function()
    blurEnabled = false
    selectedModifier = "vivid"
    selectedStrength = 0.50
    graphics.clearTimecycleModifier()
end)

if root.show then
    root:show()
end
