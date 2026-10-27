-- Glue5 Android: minimal soft-blur graphics mod.
-- Uses the game's timecycle modifier API; effect depends on runtime support.

local root = ui.menu("Soft Blur Graphics", "Light blur effect")

root:action("Soft Blur ON", function()
    graphics.setTimecycleModifier("hud_def_blur")
    graphics.setTimecycleModifierStrength(0.20)
end)

root:action("Blur OFF", function()
    graphics.clearTimecycleModifier()
end)

root:action("Reset Graphics", function()
    graphics.clearTimecycleModifier()
end)

if root.show then root:show() end
