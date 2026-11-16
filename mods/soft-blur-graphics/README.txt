Soft Blur & Colors — Glue5 Android

The bundled graphics menu provides lightweight visual presets:
- Blur: Very Low (0.06), Low (0.10), Soft (0.14)
- Colors: Natural (0.12), Light (0.28), Bright (0.45)
- Blur OFF
- Reset Graphics

The blur presets use GTA V's hud_def_blur timecycle modifier.
The color presets use GTA V's rply_saturation timecycle modifier.
The game API exposes one primary timecycle modifier at a time, so selecting a
color preset replaces the blur preset, and selecting a blur preset replaces
the color preset. Blur OFF / Reset Graphics clears the active modifier.

Values are intentionally subtle to preserve clear graphics and avoid heavy
post-processing. If a modifier is unavailable in a particular game build,
check the in-game mod log.
