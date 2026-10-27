-- RealityV Graphics options for Glue5 Android.
-- Timecycle presets are approximate visual filters; these are not shader-level
-- photorealistic/cartoon renderers. Blur uses the available timecycle modifier.

local root = ui.menu("RealityV Graphics", "Graphics presets and blur controls")

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

local function preset(label, modifier, strength)
    root:action(label, function()
        selectedModifier = modifier
        selectedStrength = strength
        blurEnabled = false
        applyCurrent()
    end)
end

preset("Colorful / Vivid", "vivid", 0.50)
preset("Natural", "default", 0.25)
preset("Realistic / Filmic", "NG_filmic01", 0.45)
-- Approximate cartoon-like contrast only; a true cartoon shader is not exposed
-- by this script's verified Glue5 graphics API.
preset("Cartoon Style (Approx.)", "REDMIST", 0.55)

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
