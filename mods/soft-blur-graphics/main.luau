-- Glue5 Android: keep the graphics menu alive even if this game.wasm
-- does not expose the optional extra-timecycle natives.
local root = ui.menu("Soft Blur + Colorful")

local colorfulOn = true
local blurOn = true
local colorName = "rply_saturation"
local colorStrength = 0.75
local blurName = "hud_def_blur"
local blurStrength = 0.20

local function hasExtraLayer()
    return type(graphics.setExtraTimecycleModifier) == "function"
        and type(graphics.setExtraTimecycleModifierStrength) == "function"
        and type(graphics.clearExtraTimecycleModifier) == "function"
end

local function applyColor()
    if type(graphics.setTimecycleModifier) ~= "function"
        or type(graphics.setTimecycleModifierStrength) ~= "function"
        or type(graphics.clearTimecycleModifier) ~= "function" then
        ui.notify("Color natives unavailable in this game build")
        return
    end
    if colorfulOn then
        graphics.setTimecycleModifier(colorName)
        graphics.setTimecycleModifierStrength(colorStrength)
    else
        graphics.clearTimecycleModifier()
    end
end

local function applyBlur()
    if not hasExtraLayer() then
        ui.notify("Soft Blur layer is not exposed by this game build")
        return
    end
    if blurOn then
        graphics.setExtraTimecycleModifier(blurName)
        graphics.setExtraTimecycleModifierStrength(blurStrength)
    else
        graphics.clearExtraTimecycleModifier()
    end
end

root:button("Soft Blur ON", function()
    blurOn = true
    applyBlur()
    if hasExtraLayer() then ui.notify("Soft Blur enabled") end
end)

root:button("Blur OFF", function()
    blurOn = false
    applyBlur()
    if hasExtraLayer() then ui.notify("Blur disabled") end
end)

root:button("Super Colourful", function()
    colorfulOn = true
    colorName = "rply_saturation"
    colorStrength = 0.75
    applyColor()
    ui.notify("Super Colourful selected")
end)

root:button("Natural Green", function()
    colorfulOn = true
    colorName = "rply_saturation"
    colorStrength = 0.40
    applyColor()
    ui.notify("Natural Green selected")
end)

root:button("Vibrant Day", function()
    colorfulOn = true
    colorName = "rply_saturation"
    colorStrength = 0.60
    applyColor()
    ui.notify("Vibrant Day selected")
end)

root:button("Reset Graphics", function()
    colorfulOn = false
    blurOn = false
    if type(graphics.clearTimecycleModifier) == "function" then
        graphics.clearTimecycleModifier()
    end
    if hasExtraLayer() then
        graphics.clearExtraTimecycleModifier()
    end
    ui.notify("Graphics reset")
end)

-- Do not let an optional unsupported native prevent the mod/menu from loading.
applyColor()
applyBlur()
ui.notify("Soft Blur + Colorful menu loaded")

while true do
    wait()
end
