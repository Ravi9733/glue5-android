-- GLUE 5 Android graphics preset menu.
-- These use GTA V's built-in timecycle modifiers; they do not replace textures.
-- Open the mod's menu from the in-game Mods menu.

local menu = ui.menu("Graphics Presets")
local presets = {
    "Natural",
    "Colorful",
    "Cinematic",
    "Reset / Default",
}

local function apply_preset(name)
    if name == "Natural" then
        graphics.setTimecycleModifier("NG_filmic01")
        graphics.setTimecycleModifierStrength(0.35)
    elseif name == "Colorful" then
        graphics.setTimecycleModifier("vivid")
        graphics.setTimecycleModifierStrength(0.45)
    elseif name == "Cinematic" then
        graphics.setTimecycleModifier("MP_Arena_theme_attract")
        graphics.setTimecycleModifierStrength(0.30)
    else
        graphics.clearTimecycleModifier()
    end
    ui.notify("Graphics preset: " .. name)
end

menu:choice("Preset", presets, 1, function(index, name)
    apply_preset(name)
end)

menu:button("Reset graphics", function()
    apply_preset("Reset / Default")
end)

menu:label("Built-in colour filters only; no texture replacement.")
