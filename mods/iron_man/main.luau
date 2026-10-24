--!strict
-- Iron Man visual starter for Glue5 Android.
-- Uses only UI and graphics calls already present in the project's existing mods.
-- Flight, suit replacement, and combat are intentionally not faked: their native APIs
-- are not verified in this Glue5 build.

local ironManMenu = ui.menu("Iron Man")
local visualMode = false

ironManMenu:toggle("Iron Man Visual Mode", false, function(enabled)
    visualMode = enabled
    if enabled then
        -- Uses the same timecycle API pattern as the project's weather/graphics mod.
        graphics.setTimecycleModifier("MP_corona_heist")
        graphics.setTimecycleModifierStrength(0.65)
        ui.notify("Iron Man visual mode ON")
    else
        graphics.clearTimecycleModifier()
        ui.notify("Iron Man visual mode OFF")
    end
end)

ironManMenu:button("Arc Reactor Status", function()
    ui.notify("Arc Reactor: online (visual mod)")
end)

ironManMenu:button("Flight / Repulsors Status", function()
    ui.notify("Flight and repulsors need verified Glue5 game natives; not enabled in this build.")
end)

ironManMenu:button("Reset Visual Effects", function()
    visualMode = false
    graphics.clearTimecycleModifier()
    ui.notify("Iron Man visual effects reset")
end)
