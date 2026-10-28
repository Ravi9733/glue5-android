-- Glue5 Android: minimal soft-blur graphics mod.
-- Glue5 menus use :button, not :action; ui.menu accepts only a title.

local root = ui.menu("Soft Blur Graphics")

root:button("Soft Blur ON", function()
    graphics.setTimecycleModifier("hud_def_blur")
    graphics.setTimecycleModifierStrength(0.20)
    ui.notify("Soft Blur enabled")
end)

root:button("Blur OFF", function()
    graphics.clearTimecycleModifier()
    ui.notify("Blur disabled")
end)

root:button("Reset Graphics", function()
    graphics.clearTimecycleModifier()
    ui.notify("Graphics effect reset")
end)
