--!strict
-- RealityV Graphics for GLUE 5.
-- Menus use GLUE 5's documented Menu:button API; menus appear under F7 > MENUS.
-- Presets use built-in GTA V timecycle modifiers (no replacement textures).

local root = ui.menu("RealityV Graphics", "Natural, cinematic and Vice City-inspired colours")

local function apply(label: string, modifier: string, strength: number)
    graphics.setTimecycleModifier(modifier)
    graphics.setTimecycleModifierStrength(strength)
    ui.notify(label .. " applied")
end

root:button("Natural Graphics", { note = "Subtle filmic look", action = "Apply" }, function()
    apply("Natural Graphics", "NG_filmic01", 0.35)
end)

root:button("Full Colorful", { note = "More saturated colours", action = "Apply" }, function()
    apply("Full Colorful", "rply_saturation", 0.65)
end)

root:button("Cinematic", { note = "Filmic contrast", action = "Apply" }, function()
    apply("Cinematic", "NG_filmic03", 0.45)
end)

root:button("Vice City Colors", { note = "Strong saturated retro colours; not a texture/sky replacement", action = "Apply" }, function()
    apply("Vice City Colors", "rply_saturation", 0.9)
end)

root:button("Reset / Default Graphics", { note = "Remove this mod's screen filter", action = "Reset" }, function()
    graphics.clearTimecycleModifier()
    ui.notify("Default graphics restored")
end)

-- Ask GLUE 5 to open the Mods page at this menu after the mod starts.
root:show()
