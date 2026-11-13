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
- Select: request the selected model and spawn it.
- Back: cancel a pending model request or delete the last spawned entity.

This is a low-level API demonstration, not a polished touch trainer. The sample uses a fixed demonstration coordinate and has not been validated in live gameplay. Native argument signatures, spawning, deletion, and device stability still require in-game testing on the target Glue 5 build.
