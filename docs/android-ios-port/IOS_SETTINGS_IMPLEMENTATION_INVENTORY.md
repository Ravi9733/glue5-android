# iOS Settings port: implementation inventory

Reference: `kevinerikjs/gta5-ios-port` public `main`.
Target: Android build on `android-ios-port`. The iOS Settings home is the navigation contract: category list first, then category pages. Keep Controls' iOS option order and labels; do not flatten it into unrelated Android toggles.

## iOS category contract (verified from SettingsHomeViewController.swift)

| Category | When shown on iOS | Android reuse / work required |
|---|---|---|
| Graphics | Always | Reuse renderer settings in `crates/glue5/src/settings.rs` and Graphics controls in `ui/pages.rs`; Android must hide Apple-only MetalFX choices and retain Bicubic. |
| Controls | Always | Preserve iOS row order and camera tabs; map supported touch/gamepad settings to existing `settings.rs` and `touch.rs`. Gyro is a separate subsection inside Controls. |
| Cheats | Only while game is running | Reuse only verified game-thread hooks in `glue5-game`; do not expose actions without a working hook. |
| Game mode and language | Always | Story/Sandbox can reuse current `Mode`; language requires an actual localization layer before showing a selector. |
| Game files | Only before game starts | Reuse existing folder detection/import and stream pages; add iOS-style validation/progress only where Android APIs support it. |
| Mods | Always | Reuse existing `ui/mods.rs`, `glue5-mods`, and PC-mod runtime; audit archive formats and VFS support before claiming RPF/DLC/ASI compatibility. |
| Self Radio | Always | Not implemented as an Android game audio source yet; needs media import, playlist persistence and game audio/VFS bridge. |
| This device | Always | Reuse Android diagnostics/logging and memory reporting; do not port Apple device APIs. |

## Current Android source facts (verified on this branch)

- Settings currently opens as three segmented tabs: `GRAPHICS`, `CONTROLS`, and `GAME & FILES` in `crates/glue5/src/ui/pages.rs`.
- Mods already has its own destination from the home screen and a dedicated `crates/glue5/src/ui/mods.rs` page; it is not yet presented as an iOS Settings category.
- `Settings` already persists renderer, touch, haptics, Vulkan driver, mods and Studio preferences in `crates/glue5/src/settings.rs`.
- `crates/glue5-mods` already contains LuaU mods, file installation/storage, placements and a bundled trainer. This does not establish PC ASI/ScriptHook compatibility.
- Existing Android runtime uses SDL3/egui and Rust; UIKit/Metal-specific iOS controllers and APIs are not portable source files.

## Port order

1. Rebuild Settings navigation as the iOS category list, preserving the existing Android destinations and controls.
2. Keep the Controls page's iOS ordering; connect gyro only after Android SDL sensor data reaches gameplay input.
3. Add category visibility rules (Cheats only in-game; Game Files only before launch).
4. Audit each iOS category against Android runtime capability before implementing its page.
5. Implement Android-native Self Radio and device diagnostics.
6. Verify ZIP/RAR/7z/OIV handling, safe extraction, loose files and RPF/DLC overlay separately; explicitly label unsupported Windows ASI/ScriptHook files.
7. Run Android build and device tests before marking any category complete.

This inventory records source facts and implementation requirements; it does not claim that iOS features have already been ported.
