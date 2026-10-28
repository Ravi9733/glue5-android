--!strict
-- Safe Glue5 menu registration prototype.
-- The original GTA5-mods.com script is not ported; these controls are a safe menu stub.
local menu = ui.menu("Blue Balls")

menu:label("Glue5 menu test")
menu:label("Original PC mod features are not implemented yet.")

menu:button("Test notification", function()
    ui.notify("Blue Balls menu is working")
end)

menu:button("Start free play", function()
    ui.notify("Free play is not implemented in this prototype")
end)

menu:button("Stop game", function()
    ui.notify("No game session is running")
end)

while true do
    wait(1000)
end
