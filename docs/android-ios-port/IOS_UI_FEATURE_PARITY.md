# iOS 1.5.2 UI and feature parity target

Target reference: `kevinerikjs/gta5-ios-port` public `main` (the older iOS project selected for this port).
Android implementation branch: `android-ios-port`.

The requirement is feature parity, not merely a similar-looking home screen. Every visible control must either work on Android or be clearly unavailable with a reason; no decorative/fake controls.

## App shell and start screen

Recreate the iOS app's visual hierarchy and behavior in the existing egui/SDL Android UI:

- GTA V Port identity, matching dark/glass visual language, background artwork, icon/title/version.
- Game files card with ready / needs-attention / problem / busy states, progress and actions.
- Story and Sandbox mode cards with selected/last-played state and descriptive text.
- Sandbox map selection where supported by the game files.
- Play/start state, shader preparation progress, startup errors and retry path.
- Settings, Controls layout, Mods, Credits, and other app destinations.
- Responsive phone portrait/landscape and tablet layout; safe areas and scrolling.
- Preserve controller/keyboard navigation as well as touch interaction.

The Android app already has an egui start page, mode cards, game-file status, Settings, Mods, Credits and responsive layouts. Keep the existing runtime and improve this surface rather than creating a second UI framework.

## Settings navigation and categories

Match the iOS category-based Settings home and expose equivalent functional pages:

1. **Graphics**
   - Presets and per-setting graphics overrides.
   - Render resolution / scale, screen shape or fill, frame rate and VSync.
   - Upscaler choice, including Bicubic; do not silently force FSR.
   - Sharpness, performance overlay and memory/performance profile.
   - Reset graphics to safe defaults while backing up the prior settings.
   - Clearly explain which settings apply immediately and which need restart.

2. **Controls**
   - Touch scheme and camera mode (stick/swipe).
   - Camera sensitivity for look and aim, targeting/aim assist.
   - Fire/aim behavior, sprint button, steering buttons, aircraft right stick.
   - Button size and opacity; editable touch layout with persistent positions.
   - HUD layout, minimap, help text, safe zone.
   - Hide touch controls with a physical controller.
   - Keyboard/mouse and controller input status.
   - Haptics/rumble where Android hardware supports it.

3. **Gyro**
   - Gyro aim on/off and sensitivity/invert options.
   - Tilt steering mode and full-lock angle.
   - Android sensor implementation and permission/capability checks; hide or disable only when unsupported.

4. **Game mode and language**
   - Story / Sandbox selection and mode descriptions.
   - Sandbox map selection.
   - Language selection; apply app language immediately and game language at next start where required.
   - Story save import with validation and safe destination handling.

5. **Game files**
   - Select/import the user's game files through Android's Storage Access Framework.
   - Verify required build ID and hash; report missing/wrong-version files precisely.
   - Download/resume/pause/cancel only when a configured source exists.
   - Keep game assets read-only and user saves in a persistent app data directory.
   - Never require uninstalling the app to update the APK.

6. **Mods**
   - Installed-mod list, enable/disable, details, errors/logs and removal.
   - ZIP/RAR/TAR/GZ import where Android libraries and formats support them.
   - OIV package handling only after Android-safe parsing/validation.
   - Loose files, DLC/RPF and asset overlays only where the engine/VFS can consume them.
   - Validate archive paths against traversal and reject unsafe/symlink entries.
   - Explain that Windows ASI/ScriptHook binaries do not become Android-native merely by importing them.
   - Native trainer / vehicle-spawn functionality must be implemented through supported Android hooks or Lua/native APIs, never represented as working before it is.

7. **Cheats and vehicle spawning** (only while game is running and only if the engine hooks are present)
   - God mode, never wanted, repair, weather, time, money and supported Story cheats.
   - Vehicle list, spawn and optional enter vehicle.
   - Disable or explain actions that the Android engine build cannot execute.

8. **Self Radio**
   - Import/manage the user's own music files, playlist and playback integration.
   - Android media access and audio focus behavior.

9. **This device**
   - Memory/runtime information, logs, version/build identifiers, diagnostics and support links.
   - Android equivalents for iOS device/memory reporting; no Apple APIs.

## In-game touch behavior

The iOS touch contexts are the reference for Android's touch overlay:

- On foot, driving, bicycle, boat, submarine, helicopter and plane.
- Pause/map, phone, cutscene, scripted/minigame, pointer/browser and menu contexts.
- Context-sensitive action buttons, wheels, camera drag, two-finger map zoom, scrolling and row selection.
- Left/right sticks, D-pad, face buttons, triggers and controller prompts.
- Editable layout, size, opacity, safe-area positioning, controller auto-hide and haptics.

The Android project already has a context-aware egui touch system and virtual gamepad injection. Preserve it and close specific behavior gaps against the iOS reference; do not replace it with static buttons.

## Visual acceptance

- Same screen structure, naming, hierarchy and option coverage as iOS 1.5.2, adapted to Android navigation conventions.
- Matching dark theme, typography hierarchy, cards, button states, selected states, spacing and icon language.
- All screens scroll correctly on phone portrait and remain usable in landscape.
- Every setting persists across app restart and visibly updates the actual runtime/configuration.
- Restart-only changes are labeled; live changes take effect immediately.
- Unsupported features are identified honestly and cannot be mistaken for working options.

## Delivery order

1. Home/start-screen and Settings navigation parity.
2. Graphics, controls, gyro, mode/language and device settings parity.
3. In-game touch context and layout parity.
4. Game-file import and verification parity.
5. Mod installer and mod management parity.
6. Cheats, vehicle spawning and Self Radio where engine support is verified.
7. Build, install/upgrade safety, automated checks and real-device validation.

## Current status

This is the complete parity target, not a claim that all screens or actions have already been ported. Existing Android UI/runtime features are reused. Each section must be implemented and tested before it is marked complete.
