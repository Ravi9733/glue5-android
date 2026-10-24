# Iron Man-style armor prototype assets

A generated low-poly red/gold armored mannequin prototype is provided separately as `iron-man-armor-prototype-pack.zip` in the working conversation output. It contains a GLB preview model, a PNG armor texture atlas, and a reproducible generator script.

**Runtime limitation:** the GLB is not a GTA V ped asset and is not loaded by the current Glue5 mod. It has not been rigged to GTA V's player skeleton or converted to GTA V's model/texture formats. The existing `main.luau` flight controls are separate. Glue5's current documented mod API supports game assets already present in its installed asset catalog; custom player replacement still needs a verified implementation.
