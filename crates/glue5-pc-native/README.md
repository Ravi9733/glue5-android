# Android PC-runtime foundation: current scope

This crate is an isolated engineering foundation for investigating whether PC-format GTA V data can be prepared for a future Android-native runtime. It does not replace Glue 5's existing WASM runtime, and it is not a playable GTA V port.

## Current capabilities

- Inspect a candidate data directory and report whether the Android engine, Vulkan translation layer, and prepared game data are present.
- Parse a versioned data manifest and verify listed files by size and streaming SHA-256. Paths are constrained to the data root; missing files, mismatches, and paths escaping the root are rejected.
- Probe RPF7/RPF8 archive headers. Inventory metadata for RPF7 archives whose TOC uses the OPEN/NONE (unencrypted) mode.
- Validate RPF7 binary/resource payload ranges, identify metadata overlap and payload overlap, and report resource page allocation sizes.
- Resolve the documented RPF7 resource-size sentinel from a bounded 16-byte resource header when enough bytes are available.
- Inspect the first 16 bytes of bounded RPF7 resource payloads for an RSC7 signature and report whether the header is valid, truncated, out of range, or has the wrong magic.
- Decompress bounded RSC7 raw-DEFLATE bodies into system and graphics segments, validating output size against page flags. The decoded-resource API also exposes validated pointer and pointer-array reads directly against that resource.
- Borrow the decompressed RSC7 body as zero-copy system and graphics slices after checking that the body length exactly matches the page allocation metadata.
- Resolve and read bounded 64-bit serialized virtual pointers in the `0x5xxxxxxx` system and `0x6xxxxxxx` graphics address spaces.
- Parse candidate 8-byte pointer-list descriptors, read bounded fixed-size list records, decode arrays of 32-bit virtual pointers, and read each non-null target payload through the resolved system/graphics segment. Per-item segment/range validation is applied before returning slices. These are generic traversal primitives; they do not assert a particular GTA V class layout.
- Extract/copy only bounded, unencrypted, uncompressed RPF7 binary entries. Decode one selected RPF7 RSC7 resource range with a caller-supplied decompressed-output cap and a per-entry result report; this is a narrow resource extraction path, not batch mounting. Scan one level of direct nested RPF7 archives when their payloads are eligible.

## Deliberate limitations

This is metadata inspection and a narrow binary-entry extraction path, not a general RPF/RSC decoder.

- AES/NG-encrypted RPF TOCs are not decrypted. The required game-specific keys and decryption implementation are not provided here.
- RPF8 archives are identified but not inventoried.
- Compressed or encrypted ordinary binary payloads are not extracted. RSC7 resource extraction is limited to an explicitly selected, validated resource range with raw-DEFLATE content; other RPF compression/encryption modes are unsupported.
- The nested archive scan is one level deep, not recursive.
- The RSC7 reader handles the 16-byte resource header + raw-DEFLATE body. RPF resource extraction currently slices one already-validated resource payload range and passes it to that decoder; archive-specific variants and real-file validation remain outstanding.
- Pointer resolution and pointer-array/list helpers validate address spaces, counts, element ranges, and bounds only; the pointer-array reader can return bounded byte slices for non-null targets, but does not interpret GTA structure layouts, relocate pointers, or reconstruct engine objects. The 8-byte list descriptor is a candidate generic layout and is not yet verified against real GTA V resources.
- Resource page sizes are used to bound decompression and pointer access, not to create GPU allocations.
- Current tests use synthetic fixtures. The parser's assumptions must be checked against legally obtained, real GTA V archives before relying on it for production data.

## Not implemented yet

A working Android-native game engine, D3D11/DXGI compatibility layer over Vulkan/Turnip, encrypted archive support, full RPF/RSC resource decoding, asset conversion/mounting, Android lifecycle/input/audio/save integration, and end-to-end device validation are all still outstanding.

The iOS executable and its Apple-platform dynamic libraries cannot be reused as Android binaries. Android requires compatible native ELF libraries and Android platform integration. PC game data alone is not executable game code.

## Safety and compatibility

Keep this work on the experimental `android-pc-runtime-foundation` branch. The normal Glue 5 build and WASM runtime remain separate and unchanged. Do not describe this crate or its CI tests as proof that GTA V currently runs on Android.
