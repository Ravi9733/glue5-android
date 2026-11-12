# XHYN-V 0.3 host compatibility audit

## Scope

Reviewed the public `XHYN-PH/XHYN-V-reborn` branch
`migrate-1757-v03`, including its recursive Git tree, 0.3 status and
verification records, native build script, JNI declarations, and Java stubs.
This is a source-publication audit, not a decompilation of the supplied APK.

## Findings

- The 0.3 verification record identifies the Preview 2 APK as
  `com.xhynph.xhynv.reborn174`, version `0.3-preview2`, code 301, min SDK 30,
  target SDK 36, with 26 original native libraries. It explicitly marks
  device gameplay, Android UI execution, and physical GPU probing as untested.
- The published 0.3 Java tree contains helper/configuration classes and
  `XhynV02ProbeActivity`, which is a separate Vulkan probe surface. Its own
  UI says the triangle test is not game compatibility. It is not the game
  launcher or a substitute for the original Activity.
- The published `modifications/v0.3/stubs/.../GTAVActivity.java` is only a
  compile stub extending `android.app.Activity`; its only method throws
  `UnsupportedOperationException`. It cannot launch the game and does not
  disclose the real Activity lifecycle.
- `XhynV02Native` declares install, identity, probe, observe, metrics,
  capture and pause helpers. The probe accepts a `Surface`; these methods
  are not a general-purpose SDL host contract. Do not repurpose the probe
  Activity or JNI declarations as Glue 5's engine adapter.
- `build_native.py` builds the helper library
  `libxhyn_v02.so` using pinned NDK r27c and validates its probe shaders.
  It does not build the proprietary game engine or the XHYN launcher.
- The status report lists physical GPU execution, gameplay, sustained audio,
  surface recreation and pacing as outstanding validation. It also records
  unresolved renderer/BC-copy and memory/lifecycle investigations; no
  Snapdragon 8+ Gen 1 performance guarantee can be inferred.

## Integration decision

The public modification source still does not provide the exact original
Preview 2 launcher Activity implementation, its DEX bytecode, full manifest,
SDL version, native library loading order, or a reusable game-start API.
Therefore a safe second Android launcher module cannot yet be implemented
from the public source alone. In particular:

1. Do not instantiate the compile stub as a host Activity.
2. Do not use `XhynV02ProbeActivity` as the game Activity.
3. Do not call the helper's native install/probe methods as if they started
   the game.
4. Do not add `libgtav.so` to Glue 5's existing SDL3 library list.
5. Keep the existing `local.glue5` app and its Documents/Glue5 data untouched.

The supplied Preview 2 APK is the right artifact for a direct DEX/manifest/
native-library inventory, but the file bytes are not available to this GitHub
source audit. This is a concrete integration gate, not a reason to fabricate
a launcher or claim an APK build.

## Next implementation gate

Obtain the exact Preview 2 APK bytes in the build environment, extract and
record its package manifest, launcher Activity and superclass, complete DEX
method signatures, SDL version and ordered native library loads, then compare
the extracted files to the pinned APK SHA-256 from
`modifications/v0.3/VERIFICATION.json`. Only after that evidence exists can
the native shell adapter and separate Android target be implemented.
