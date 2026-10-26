# PC Trainer V — Android feature adapter for Glue 5

This package ports a small, explicitly listed subset of trainer-like features to
Glue 5's own Luau runtime. It is **not** the original Windows \`.asi\` file and
does not load ScriptHookV, Windows DLLs, or arbitrary PC mods.

## Included features

- Player invincibility toggle
- Heal player and restore armour
- Set/clear wanted level
- Repair the current vehicle
- Preset time and resume natural time
- Weather choice and reset

Each action calls the native bridge available to Glue 5's supported GTA V WASM
build. A native can still be missing or behave differently in a particular
build; errors are surfaced as notifications. These features require in-game
testing before being considered verified.

## Install

1. Build/install the Android APK from the \`android-pc-trainer-runtime\` workflow artifact.
2. Start Glue 5 with **Mods** enabled.
3. Open GLUE 5 Studio from the app's Mods settings and pair using its shown URL/code.
4. Open \`/install-mod\` on the paired Studio URL and upload the generated
   \`Glue5-PC-Trainer-V-Android.zip\` file.
5. Return to the game and open the app's **Mods → MENUS** page.

## Not included

The original PC trainer's \`.asi\`/\`.dll\`, ScriptHookV APIs, memory patches,
external assets, and functions requiring the Windows GTA V executable are not
supported by this adapter. No generic PC binary compatibility is claimed.
