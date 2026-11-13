# Native conversion core

The Android app uses two native layers:

- Rust rpf-archive / rage-rpf: bounded RPF parsing, unencrypted extraction and RPF7 repacking.
- C++ ragekit v0.3.1: Gen9-to-Legacy resource conversion for .ytd, .ydr, .ydd, .yft and .ypt.

The C++ library is pinned to commit 9d4110ef0fce900787aaeccb52b67fa5402ff41b. The app invokes its JNI bridge on the extracted app-private directory before RPF7 repacking.

## Current limitations

- Nested RPF file entries are recursively extracted and rebuilt by the Android orchestration layer, with depth 8 / 500-file caps and a 16 MiB RPF7 binary-entry ceiling. Larger nested archives fail closed.
- The current in-memory RPF7 writer accepts at most 512 MiB aggregate loose-file input and emits at most 1 GiB per archive to keep peak memory bounded on 8 GB Android devices.
- .ybn, map/meta, audio, scripts, configuration and other non-listed formats are not rewritten.
- Encrypted AES/NG archives need legitimate matching key material and fail closed otherwise.
- Repacked archives are structurally validated, but Glue5/Android runtime compatibility is not established.
- Host C++ tests now build synthetic Gen9 YTD/YDR/YDD/YFT/YPT resources, convert each to its Legacy version, parse the outputs, and verify Legacy pass-through. Rust tests cover OPEN RPF7 repacking, RSC7 resource-header/page-flag round trips, nested archive payloads and malformed-header rejection. These are synthetic fixtures, not real retail-game fixtures; physical Android and Glue5 in-game tests remain outstanding.
