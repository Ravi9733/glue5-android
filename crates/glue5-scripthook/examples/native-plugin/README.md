# Native ScriptHook sample plugin (Android ARM64)

This sample is a **native plugin example**, not a Windows ASI loader. It must be compiled as an Android ARM64 ELF shared library (`.so`) against the Glue 5 host ABI. Renaming a Windows `.asi` or `.dll` file does not convert it.

## Build on Linux/macOS

1. Install Android NDK r26 or newer.
2. Set `ANDROID_NDK_HOME` to the NDK directory (or set `ANDROID_NDK_ROOT`).
3. Run:

```sh
./build.sh
```

The default output is `build/arm64-v8a/sample.so`. To choose another output path:

```sh
./build.sh /absolute/path/to/sample.so
```

The build targets `arm64-v8a` with Android API level 29 by default. Override the API level with `ANDROID_API_LEVEL` if needed.

## Install

Copy the resulting `sample.so` into:

```text
<Glue 5 SDL user-data directory>/scripthook/plugins/sample.so
```

The host creates/scans the `scripthook/plugins` directory when it initializes. The SDL user-data directory is app-private on Android; a normal file manager may not be able to browse it. Use a future in-app plugin importer or a rooted file manager/ADB with appropriate access. Do not put this plugin in the game's `data/` or `b/<build>/` folders.

## Sample controls and limitations

- D-pad Up: select the Adder vehicle.
- D-pad Down: select the security ped.
- Select: request the selected model and spawn it 4 metres in front of the local player, using the player's current heading.
- Back: cancel a pending model request or delete the last spawned entity.

The player-relative spawn API checks the generated `PLAYER_PED_ID` and `GET_ENTITY_MATRIX` signatures and cancels spawning if it cannot read a valid transform. This is still a low-level API demonstration, not a polished touch trainer or a drop-in ScriptHookV replacement. It has not been validated in live gameplay. Native argument signatures, spawning, deletion, and device stability still require in-game testing on the target Glue 5 build.

## CI

The pull-request workflow installs Android command-line tools into the runner's temporary SDK directory, installs NDK r27d, builds the ARM64 sample, and uploads `sample.so` as a workflow artifact. A successful workflow run is required before treating the sample as build-verified.
