Glue 5 PC-mod runtime foundation
================================

This is the first isolated runtime component. It inspects binary headers and reports the compatibility boundary before a plugin can be considered for loading. It does not load or execute .asi/.dll files.

Why a compatibility layer is required
--------------------------------------
The target is a GTA V WASM build running through Glue 5. Typical GTA V PC trainer plugins are Windows PE x64 DLLs built for the Windows GTA V process and expect exports/hooks from ScriptHookV and GTA's native runtime. Android ARM64 cannot load those PE binaries as ordinary Android libraries, and Glue 5's Luau/native bridge is a different plugin ABI.

A full implementation therefore needs: (1) a plugin binary compatibility layer or documented porting interface; (2) a compatible implementation of the specific ScriptHookV exports and lifecycle expected by each supported plugin; (3) a bridge from supported calls into the matching GTA V WASM build's native registry and memory model; (4) sandboxing, fault isolation, dependency checks, and diagnostics; (5) real game testing for each named plugin and build. Recognizing a file does not make it compatible. No APK workflow is triggered by this source-only change.
