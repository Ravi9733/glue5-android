--!strict
-- RealityV Graphics for GLUE 5.
-- The app lists this menu under Mods > MENUS (F7 on keyboard, touch button beside Settings).

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

root:button("Vice City Colors", { note = "Saturated retro colours; not a texture/sky replacement", action = "Apply" }, function()
    apply("Vice City Colors", "rply_saturation", 0.9)
end)

root:button("Reset / Default Graphics", { note = "Clear the screen filter", action = "Reset" }, function()
    graphics.clearTimecycleModifier()
    ui.notify("Default graphics restored")
end)

-- Diagnostic: if this notification appears after the game starts, the script is loading.
ui.notify("RealityV Graphics loaded")
