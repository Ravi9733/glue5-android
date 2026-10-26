# Android PC trainer compatibility work

This branch combines the Android ZIP installer with a native-backed Luau adapter
for selected PC Trainer V-style features.

## Architecture

Glue 5 runs the supported GTA V WebAssembly build using its wasm2c-generated
runtime. The existing mod engine runs Luau scripts on game script threads and
routes calls through the generated GTA native table. The ZIP installer accepts
a Glue 5 mod containing \`main.luau\`; it is not a Windows DLL loader.

The adapter reimplements a small set of trainer features using this bridge.
This is the realistic first step toward a PC-trainer-like experience on Android.
It is not equivalent to executing the original PC \`.asi\` binary.

## Scope

The adapter includes invincibility, healing, armour, wanted-level controls,
vehicle repair, time presets, and weather selection/reset. Every native call
must be confirmed against the generated database and then tested in the running
game. The CI APK build checks compilation, not in-game behavior.

## Not supported by this milestone

- Arbitrary Windows \`.asi\` or \`.dll\` files
- ScriptHookV or ScriptHookVDotNet
- Native plugins built for the iOS app
- PC memory offsets, executable hooks, and native code that assumes the Windows
  GTA V process layout

## Validation checklist

- [ ] Android APK builds in GitHub Actions
- [ ] Mod ZIP installs through the authenticated Studio installer
- [ ] Trainer menu appears in Mods → MENUS
- [ ] Each listed action tested in-game on Android
- [ ] Existing saves, controls, and startup work with the adapter disabled
