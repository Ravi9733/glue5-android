# Legacy Rust native core

This Rust crate is retained as a separately tested RPF parsing/extraction and RPF7-repacking prototype. It is not the active conversion implementation used by the current Android screen.

## Active Android pipeline

The current MainActivity calls the C++ NativeAssets JNI bridge. That bridge uses RatPrez/ragekit v0.3.1 for keyed RPF7 extraction, Gen9-to-Legacy resource conversion, nested RPF rebuilding and OPEN RPF7 output. The Rust NativeRpf JNI wrapper is retained for development compatibility but is not called by the current Android activity.

## Rust crate scope

- inspect_archive: RPF metadata inspection using rpf-archive.
- extract_archive: bounded extraction of archives accepted by the Rust parser; encrypted archives fail closed without keys.
- build_android_rpf7: legacy prototype writer that converts Gen9 YTD only and preserves other formats unchanged.
- Path traversal, symlink and configured size limits are checked.
- Rust unit tests cover RPF7 output parsing, nested archive payloads, RSC7 resource flags, pack budgets and malformed paths/headers.

Do not treat this crate's YTD-only conversion function as the Android app's active resource conversion path. The C++ ragekit pipeline is authoritative for the current APK.

## Limitations

The Rust crate does not implement nested archive orchestration, the app's AES/NG key import flow, YDR/YDD/YFT/YPT conversion, YBN/YMAP/YTYP parsing, DLC registration, or Glue5 runtime compatibility. Physical Android and Glue5 in-game tests remain outstanding.
