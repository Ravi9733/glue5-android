--!strict
-- RealityV Graphics for GLUE 5 Android.
-- Uses documented game clock, weather and timecycle APIs.
-- Note: this is a Vice City-inspired preset, not a replacement sky texture.

local root = ui.menu("RealityV Graphics", "Colour presets, sunset time and weather")

local function applyFilter(label: string, modifier: string, strength: number)
    graphics.setTimecycleModifier(modifier)
    graphics.setTimecycleModifierStrength(strength)
    ui.notify(label .. " filter applied")
end

root:button("Natural Graphics", { note = "Subtle filmic colour", action = "Apply" }, function()
    applyFilter("Natural Graphics", "NG_filmic01", 0.35)
end)

root:button("Full Colorful", { note = "Saturated game colours", action = "Apply" }, function()
    applyFilter("Full Colorful", "rply_saturation", 0.65)
end)

root:button("Cinematic", { note = "Lighter filmic look; reduced darkness", action = "Apply" }, function()
    applyFilter("Cinematic", "NG_filmic03", 0.20)
end)

root:button("Vice City Sunset", { note = "Sunset clock, clear weather and saturated colours", action = "Apply" }, function()
    clock.setClockTime(18, 30, 0)
    misc.setWeatherTypeNowPersist("EXTRASUNNY")
    graphics.setTimecycleModifier("rply_saturation")
    graphics.setTimecycleModifierStrength(0.9)
    ui.notify("Vice City Sunset applied: 18:30, Extra Sunny")
end)

root:button("Vice City Night", { note = "Late evening with saturated colours", action = "Apply" }, function()
    clock.setClockTime(20, 0, 0)
    misc.setWeatherTypeNowPersist("EXTRASUNNY")
    graphics.setTimecycleModifier("rply_saturation")
    graphics.setTimecycleModifierStrength(0.8)
    ui.notify("Vice City Night applied: 20:00, Extra Sunny")
end)

root:button("Reset Filter", { note = "Clear only the screen filter", action = "Reset" }, function()
    graphics.clearTimecycleModifier()
    ui.notify("Screen filter cleared")
end)

ui.notify("RealityV Graphics loaded")
