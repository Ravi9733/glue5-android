# Android ARM64 PC-mod compatibility plan

This document defines the real compatibility work needed before Glue 5 can claim support for Windows GTA V `.asi`/`.dll` mods on Android ARM64.

## Important distinction

The current Glue 5 mod engine runs `main.luau` scripts against Glue 5's game-native bridge. It does not execute Windows PE binaries. A trainer-like Luau adapter is a port of selected features, not the original PC trainer.

Blink's upstream project primarily provides an x86-64 Linux/POSIX execution environment. Compiling it for ARM64 does not by itself provide Windows DLL loading, Win32 APIs, ScriptHookV, or GTA V PC memory layouts. Glue 5's game is a WASM build, not the Windows x86-64 GTA V executable expected by many PC mods.

## ARM64 architecture proposal

1. **Binary inspection (safe, no execution):** identify PE architecture, imports, exports, relocations, TLS, exception metadata, and required DLLs. Reject unsupported binaries with a clear report.
2. **Runtime decision:** do not load a binary until a tested execution backend and API contract are available. Report unsupported status rather than silently claiming success.
3. **x86-64 execution backend:** evaluate an interpreter/emulator that can legally and technically run the required instruction set on Android ARM64. Verify its license, Android/NDK support, syscall assumptions, threading, signal handling, and performance before integration.
4. **Windows compatibility layer:** implement only the Win32 APIs required by the selected test mod. General Windows DLL support is not implied.
5. **ScriptHookV compatibility layer:** provide compatible script registration, tick callbacks, native invocation, and input handling only where they can be mapped to Glue 5's WASM game's actual functions.
6. **Game bridge:** map each exposed operation to a verified Glue 5 game native. Never expose arbitrary PC memory addresses or PC executable offsets to the WASM game.
7. **Touch UI:** use Glue 5's existing Luau menu system for Android controls. Keep UI independent from the binary execution backend.
8. **Validation:** first run a harmless x86-64 test binary in the interpreter, then a minimal test DLL, then a minimal ScriptHook-style script, and finally one explicitly selected trainer feature in-game. Test cold start, repeated enable/disable, crashes, memory use, and frame time on ARM64 hardware.

## Initial acceptance tests

- APK builds with the Android NDK for `arm64-v8a`.
- PE inspector correctly identifies x86-64 DLLs and their imports without executing them.
- Unsupported DLLs show an actionable reason and cannot crash the game by being selected.
- A minimal x86-64 test program runs under the selected backend on Android ARM64.
- A minimal compatibility plugin can call only an explicitly whitelisted bridge operation.
- Each trainer feature is verified inside the exact Glue 5 WASM game build before being marked supported.

## Current status

**Planning / not implemented.** No general Windows PE execution, Win32 emulation, ScriptHookV replacement, or generic PC `.asi` support is currently delivered by this document or by the existing Luau trainer adapter. Do not label an APK as PC-mod compatible until the acceptance tests above pass.
