-- Glue5 Android: experimental colorful graphics presets.
-- Uses Glue5's supported menu API. Timecycle names and visual strength
-- still need verification in the target game build.

local root = ui.menu("Colorful Graphics")

local function setLook(name, strength, message)
    graphics.setTimecycleModifier(name)
    graphics.setTimecycleModifierStrength(strength)
    ui.notify(message)
end

root:button("Super Colourful", function()
    setLook("rply_saturation", 0.75, "Super Colourful preset applied")
end)

root:button("Natural Green", function()
    setLook("rply_saturation", 0.40, "Natural Green preset applied")
end)

root:button("Vibrant Day", function()
    setLook("rply_saturation", 0.60, "Vibrant Day preset applied")
end)

root:button("Colour OFF / Reset", function()
    graphics.clearTimecycleModifier()
    ui.notify("Graphics effect reset")
end)

ui.notify("Colorful Graphics menu ready")

while true do
    wait()
end
