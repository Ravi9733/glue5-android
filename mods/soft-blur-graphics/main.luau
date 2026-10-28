-- Glue5 Android: combined Soft Blur + Colorful Graphics menu.
-- Both feature sets are kept in this single mod so neither menu disappears.

local root = ui.menu("Soft Blur + Colorful")

root:button("Soft Blur ON", function()
    graphics.setTimecycleModifier("hud_def_blur")
    graphics.setTimecycleModifierStrength(0.20)
    ui.notify("Soft Blur enabled")
end)

root:button("Blur OFF", function()
    graphics.clearTimecycleModifier()
    ui.notify("Blur disabled")
end)

root:button("Super Colourful", function()
    graphics.setTimecycleModifier("rply_saturation")
    graphics.setTimecycleModifierStrength(0.75)
    ui.notify("Super Colourful selected")
end)

root:button("Natural Green", function()
    graphics.setTimecycleModifier("rply_saturation")
    graphics.setTimecycleModifierStrength(0.40)
    ui.notify("Natural Green selected")
end)

root:button("Vibrant Day", function()
    graphics.setTimecycleModifier("rply_saturation")
    graphics.setTimecycleModifierStrength(0.60)
    ui.notify("Vibrant Day selected")
end)

root:button("Reset Graphics", function()
    graphics.clearTimecycleModifier()
    ui.notify("Graphics effects reset")
end)

ui.notify("Soft Blur + Colorful menu loaded")

while true do
    wait()
end
