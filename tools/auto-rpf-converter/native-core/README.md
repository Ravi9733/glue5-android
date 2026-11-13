# Native conversion core

The Android app uses two native layers:

- Rust rpf-archive / rage-rpf: bounded RPF parsing, unencrypted extraction and RPF7 repacking.
- C++ ragekit v0.3.1: Gen9-to-Legacy resource conversion for .ytd, .ydr, .ydd, .yft and .ypt.

The C++ library is pinned to commit 9d4110ef0fce900787aaeccb52b67fa5402ff41b. The app invokes its JNI bridge on the extracted app-private directory before RPF7 repacking.

## Current limitations

- Nested RPF recursion and nested archive rewriting are not implemented.
- .ybn, map/meta, audio, scripts, configuration and other non-listed formats are not rewritten.
- Encrypted AES/NG archives need legitimate matching key material and fail closed otherwise.
- Repacked archives are structurally validated, but Glue5/Android runtime compatibility is not established.
- Host tests currently include a synthetic Gen9 YTD-to-Legacy conversion fixture; real-game YDR/YDD/YFT/YPT fixture coverage and on-device tests remain outstanding.
