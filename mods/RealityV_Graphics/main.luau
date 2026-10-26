-- RealityV-style graphics presets for GLUE 5.
-- Uses built-in timecycle modifiers; does not install replacement textures.

local root = ui.menu("RealityV Graphics", "Choose a graphics preset")

local function apply(name, strength)
    graphics.setTimecycleModifier(name)
    graphics.setTimecycleModifierStrength(strength)
end

root:action("Natural Graphics", function()
    apply("NG_filmic01", 0.45)
end)

root:action("Full Colorful", function()
    apply("vivid", 0.50)
end)

root:action("Cinematic", function()
    apply("MP_Arena_theme_attract", 0.35)
end)

root:action("Soft Blur", function()
    apply("hud_def_blur", 0.20)
end)

root:action("Strong Blur", function()
    apply("hud_def_blur", 0.45)
end)

root:action("Reset / Default Graphics", function()
    graphics.clearTimecycleModifier()
end)

-- Show the menu when this mod script starts, if supported by this GLUE 5 build.
if root.show then
    root:show()
end
