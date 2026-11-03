# Turnip driver integration status

This branch contains an optional Android Adreno custom Vulkan-driver path. The app imports a user-supplied driver ZIP into app-private storage and packages AdrenoTools plus its hook libraries for arm64-v8a.

## Startup selection

- `Glue5Activity` sets the app-private files directory and Android native-library directory before SDL starts.
- If `files/turnip/driver_name` names an existing `.so` inside `files/turnip`, startup sets `GLUE5_VULKAN_DRIVER=turnip`.
- If no valid imported driver is present, startup explicitly selects the Android system Vulkan driver.
- A launch-intent extra `GLUE5_VULKAN_DRIVER=system` can force the system driver for recovery; `turnip` requests the imported driver.
- The wgpu-hal patch routes Vulkan entry loading through AdrenoTools only for the Turnip selection and falls back to the system loader if custom loading fails.

## Remaining verification

A successful APK build is not proof that the custom driver loaded or that the graphics glitch is fixed. Verify on an Adreno device using logcat and the app-private `turnip-status.txt`:

1. Confirm the log says Turnip loaded through AdrenoTools (not system fallback).
2. Confirm Vulkan enumerates the Adreno GPU and wgpu reports the expected driver.
3. Run a repeatable in-game route covering roads, buildings, distant LODs, streamed textures, and weather/time changes.
4. Check for texture upload, Vulkan validation, device-lost, and shader/pipeline errors.
5. Compare the same route with the system driver to isolate driver-specific regressions.

Do not describe the graphics issue as fixed until this device test passes. If the selected driver fails, force `GLUE5_VULKAN_DRIVER=system` and preserve the imported driver for diagnosis.
