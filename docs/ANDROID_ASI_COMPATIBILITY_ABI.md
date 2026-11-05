# Android ASI compatibility bridge

## Safety and compatibility boundary

A Windows `.asi` file is a PE DLL, not an Android shared library. Detecting its architecture or parsing its import table does not make it executable. Glue5 currently performs static inspection and reports Windows plugins as unsupported. Do not pass PE bytes to `dlopen`, rename them to `.so`, or execute them in the Android process.

## Proposed staged bridge

1. **Inventory only:** retain bounded PE header/import inspection and report architecture, imported DLLs, delay-load imports, and unsupported API groups.
2. **Plugin ABI:** define a versioned, C-compatible Glue5 plugin interface with explicit init, tick, event, and shutdown callbacks; version negotiation; capability queries; and no direct access to renderer internals.
3. **Sandbox boundary:** load only signed/allowlisted ARM64 native Glue5 plugins through Android's native loader, in a restricted plugin process where practical. The plugin ABI is distinct from Windows ASI compatibility.
4. **Windows compatibility research:** separately prototype PE x64 loading, Win32 API shims, filesystem/thread/memory behavior, and the exact ScriptHookV/GTA interfaces required by a selected plugin. This is a separate compatibility runtime, not part of the native Glue5 ABI.
5. **Compatibility matrix:** test real user-owned plugins by hash and version. Report each as unsupported, inspectable, ABI-compatible, or runtime-tested; never infer support from a successful PE parse.

## Initial scope

The first safe deliverable is a versioned native Glue5 plugin ABI for newly authored Android plugins plus accurate PE diagnostics. Existing Windows ASI plugins remain unsupported until a real PE compatibility runtime and the required GTA/ScriptHook API bridge are implemented and tested.

## Acceptance gate

A plugin is runnable only after ABI/version validation, capability checks, lifecycle tests, unload/error tests, and an Android ARM64 device test. PE/ASI detection or import analysis alone must never set a runnable status.
