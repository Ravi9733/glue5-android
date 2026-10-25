Glue 5 PC-mod runtime foundation
================================

This is the first isolated runtime component. It inspects binary headers and reports the compatibility boundary before a plugin can be considered for loading. It does not load or execute .asi/.dll files.

Why a compatibility layer is required
--------------------------------------
The target is a GTA V WASM build running through Glue 5. Typical GTA V PC trainer plugins are Windows PE x64 DLLs built for the Windows GTA V process and expect exports/hooks from ScriptHookV and GTA's native runtime. Android ARM64 cannot load those PE binaries as ordinary Android libraries, and Glue 5's Luau/native bridge is a different plugin ABI.

A full implementation therefore needs: (1) a plugin binary compatibility layer or documented porting interface; (2) a compatible implementation of the specific ScriptHookV exports and lifecycle expected by each supported plugin; (3) a bridge from supported calls into the matching GTA V WASM build's native registry and memory model; (4) sandboxing, fault isolation, dependency checks, and diagnostics; (5) real game testing for each named plugin and build. Recognizing a file does not make it compatible. No APK workflow is triggered by this source-only change.


Source-based Lua import support
-------------------------------
Glue 5's mod manager also recognizes a mod folder containing `main.lua`, and its
`require("./module")` loader accepts `.lua` modules as well as `.luau`
(preferentially loading `.luau` when both exist). This only helps with source
mods that can be adapted to Glue 5's available namespaces and native calls.
It does not provide ScriptHookV functions, SHVDN/.NET APIs, native GTA PC
memory layouts, or compatibility with compiled Windows plugins. A PC Lua
trainer still needs its API calls ported and validated for the target WASM build.

Validation status
-----------------
These changes are source edits only. They have not been compiled or exercised
inside the game; no APK has been produced. Treat compatibility as experimental
until the Rust workspace builds and each supported mod is tested in-game.


PE dependency inspection
------------------------
The runtime foundation now has a bounds-checked PE import-table reader for
PE32/PE32+ images. It reports imported DLL names and flags common ScriptHookV,
.NET/SHVDN-related, and proxy-loader dependencies. The mod crate exposes this
as `inspect_pe_dependencies` for future diagnostics. This is static analysis
only: it neither resolves imports nor executes a plugin, and dependency names
alone cannot prove which APIs a plugin calls. It is groundwork for selecting
and implementing a compatibility subset, not Windows plugin support.


Compatibility blocker planning
------------------------------
The parser now groups imported DLLs into likely dependency areas (ScriptHookV,
Windows kernel/process APIs, Windows UI, C/C++ runtime, Direct3D, audio/input,
and unknown DLLs) and emits explicit blockers. This is deliberately conservative:
PE imports do not reveal every dynamically resolved API or prove which exports a
plugin calls. The plan always reports that the plugin is not executable in Glue5
until a real Windows x64 execution environment and game-specific API bridge exist.
