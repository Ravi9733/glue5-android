# Exact XHYN-V Preview 2 APK evidence collection

The public modification source does not contain the original 0.3 launcher
Activity implementation. The exact Preview 2 APK is therefore the required
source of truth for the next adapter stage. Do not infer its host contract from
the throwing compile stub or the standalone Vulkan probe.

## Read-only inventory

Run this against the exact Preview 2 APK (the expected published SHA-256 is
`a681765d9aae1404c59e3570682f3078bf9062ff800abfc7c5e1972f2783309f`):

```sh
python3 scripts/audit-native-host-apk.py /path/to/XHYN-V-0.3-Preview2.apk > xhyn-preview2-inventory.json
```

The script does not extract files or modify the APK. It records the APK hash,
manifest hash/size, DEX inventory, every `lib/arm64-v8a/*.so` hash and ELF
architecture, and relevant literal host strings found in DEX bytecode. A
matching APK hash is a prerequisite; if the hash differs, stop and identify
the exact build before using its results.

## Required decompilation evidence

The inventory is only a first pass. Use a trusted local JADX build to decode
the exact APK and inspect:

- the launcher Activity and its superclass;
- `loadLibraries()` and the ordered native library names;
- `SDLActivity.nativeRunMain` call sites and arguments;
- `GTAVActivity$GameSurface` creation/change/destroy callbacks;
- pause/resume/destroy and audio-focus handling;
- manifest package, exported launcher, orientation, process and configChanges.

Keep the full JADX output local; share only the relevant Activity/manifest
sections and native library inventory. Do not publish proprietary game assets,
keys, or complete decompiled source.

## Native library evidence

For every ARM64 library, record its exact filename and SHA-256. Check ELF
architecture and dependencies (for example with Android NDK `llvm-readelf
-d`). Preserve the load order from the actual host code; alphabetical order
or a directory listing is not a substitute.

## Adapter gate

Only after the APK hash, launcher Activity, SDL version, library order,
entrypoint, surface lifecycle, and data requirements are verified may the
Android adapter implement `NativeEngine`. Until then the shell remains
`NotAttached`. The inventory script's self-test runs in CI, but CI does not
have the proprietary Preview 2 APK and does not claim to validate it.


## Automated local evidence bundle

For a fuller read-only evidence bundle, install Android SDK command-line tools (apkanalyzer or aapt), JADX, and NDK LLVM utilities (llvm-readelf and llvm-nm) on a trusted workstation, then run:

```sh
python3 scripts/extract-native-host-evidence.py \
  /path/to/XHYN-V-0.3-Preview2.apk --out ./xhyn-preview2-evidence
```

The helper records the APK SHA-256, extracts only ARM64 shared libraries into the output folder, attempts manifest/DEX inspection, and records ELF headers, dynamic dependencies and exported symbols. Missing tools are reported explicitly; they are not silently treated as successful inspection. If JADX is available, inspect the generated `jadx/` directory for the exact launcher Activity and SDL lifecycle. The script never patches or repackages the APK.

Keep this evidence bundle private. Do not upload proprietary libraries, game assets, signing material, or a full JADX decompilation to the public repository. Share only the APK hash, relevant Activity method signatures, manifest facts, SDL version and ordered library names needed to define the adapter contract.
