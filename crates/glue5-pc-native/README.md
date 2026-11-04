# Android PC-runtime foundation: current scope

This crate is an isolated engineering foundation for investigating whether PC-format GTA V data can be prepared for a future Android-native runtime. It does not replace Glue 5's existing WASM runtime, and it is not a playable GTA V port.

## Current capabilities

- Parse a bounded, versioned Glue5 JSON placement document (schema version 1) containing relative asset identifiers and position/rotation/scale vectors. The parser rejects traversal/absolute asset paths, malformed or non-finite transforms, invalid scales, oversized documents, and excessive placement counts. This is a Glue5-defined interchange schema, not a promise to accept every third-party GTA placement JSON dialect.
- Pass parsed placements, in order, through a `PlacementSink` adapter contract. The API reports partial progress and the failing placement if the runtime adapter rejects an asset/entity. No GTA engine-backed sink exists yet, so this is an integration seam and test harness, not in-game object spawning.
- Maintain a case-insensitive virtual asset mount index. It rejects traversal paths, empty/overflowing source ranges, and case-only duplicate names; successful lookups return the archive identifier, entry index, and validated source byte range. An RPF7 inventory adapter now populates the index under a caller-supplied virtual prefix, registering only in-bounds unencrypted/uncompressed binary entries and fixed-size resource entries; encrypted/compressed binaries and resource-size sentinels are skipped. The file-backed `copy_mounted_rpf7_asset_from_reader` API rechecks the archive ID, TOC entry kind, offset, length, metadata boundary, archive bounds, and caller output cap before streaming the raw stored bytes in 64 KiB chunks. Resource entries are copied in stored form and still require RSC7 decoding; the reader does not make them engine-ready.

- Inspect a candidate data directory and report whether the Android engine, Vulkan translation layer, and prepared game data are present.
- Parse a versioned data manifest and verify listed files by size and streaming SHA-256. Paths are constrained to the data root; missing files, mismatches, and paths escaping the root are rejected.
- Probe RPF7/RPF8 archive headers. Inventory metadata for RPF7 archives whose TOC uses the OPEN/NONE (unencrypted) mode. A seekable file-backed reader inventories only the bounded header/TOC/name-table region and records the full archive length without loading the large payload into RAM. File-backed APIs now stream eligible uncompressed binary entries from the archive into a caller-owned writer in fixed 64 KiB chunks; a file-backed RSC7 extraction entry point is also present; it resolves the legacy 0xFFFFFF resource-size sentinel from the bounded 16-byte resource header before streaming decode.
- Validate RPF7 binary/resource payload ranges, identify metadata overlap and payload overlap, and report resource page allocation sizes.
- Resolve the documented RPF7 resource-size sentinel from a bounded 16-byte resource header when enough bytes are available.
- Inspect the first 16 bytes of bounded RPF7 resource payloads for an RSC7 signature and report whether the header is valid, truncated, out of range, or has the wrong magic.
- Decompress bounded RSC7 raw-DEFLATE bodies into system and graphics segments, validating output size against page flags. A seek-based streaming decoder can write a selected payload directly to a caller-owned writer in fixed-size chunks, avoiding a full decompressed-resource allocation. The TOC-indexed `extract_rpf7_rsc7_entry_to_writer` API performs archive inventory/range validation first, then streams the selected entry directly to the writer. The decoded-resource API also exposes validated pointer and pointer-array reads directly against that resource.
- Borrow the decompressed RSC7 body as zero-copy system and graphics slices after checking that the body length exactly matches the page allocation metadata.
- Resolve and read bounded 64-bit serialized virtual pointers in the `0x5xxxxxxx` system and `0x6xxxxxxx` graphics address spaces.
- Parse candidate 8-byte pointer-list descriptors, read bounded fixed-size list records, decode arrays of 32-bit virtual pointers, and read each non-null target payload through the resolved system/graphics segment. Per-item segment/range validation is applied before returning slices. These are generic traversal primitives; they do not assert a particular GTA V class layout.
- Extract/copy only bounded, unencrypted, uncompressed RPF7 binary entries. Decode one selected RPF7 RSC7 resource range with a caller-supplied decompressed-output cap and a per-entry result report; this is a narrow resource extraction path, not batch mounting. In-memory and file-backed scanners recursively traverse eligible stored child archives with caller-supplied depth and total-archive limits; the file-backed variant seeks to child archives and reads only bounded TOC/name metadata instead of loading parent payloads. A TOC-indexed extraction entry point rebuilds the inventory from the supplied archive bytes before selecting a resource, avoiding caller-supplied ranges or mismatched inventories.

## Deliberate limitations

This is metadata inspection and a narrow binary-entry extraction path, not a general RPF/RSC decoder.

- AES/NG-encrypted RPF TOCs are not decrypted. The required game-specific keys and decryption implementation are not provided here.
- RPF8 archives are identified but not inventoried.
- Compressed or encrypted ordinary binary payloads are not extracted. RSC7 resource extraction is limited to an explicitly selected, validated resource range with raw-DEFLATE content; other RPF compression/encryption modes are unsupported.
- Recursive nested scans are limited to eligible stored, unencrypted RPF7 children and require caller-supplied maximum depth and total archive count. Compressed/encrypted children are skipped and RPF8 remains unsupported. Synthetic tests cover nested path reporting, depth and archive-count caps, and invalid child payload ranges.
- The RSC7 reader handles the 16-byte resource header + raw-DEFLATE body. The streaming path bounds input to the selected payload and output to the page allocation/caller cap; a corrupt stream can leave partial bytes in the caller's writer. File-backed RPF extraction resolves the legacy resource-size sentinel from its bounded header and streams only the selected validated range; archive-specific variants and real-file validation remain outstanding.
- Pointer resolution and pointer-array/list helpers validate address spaces, counts, element ranges, and bounds only; the pointer-array reader can return bounded byte slices for non-null targets, but does not interpret GTA structure layouts, relocate pointers, or reconstruct engine objects. The 8-byte list descriptor is a candidate generic layout and is not yet verified against real GTA V resources.
- Resource page sizes are used to bound decompression and pointer access, not to create GPU allocations.
- Current tests use synthetic fixtures. The parser's assumptions must be checked against legally obtained, real GTA V archives before relying on it for production data.

## Not implemented yet

A working Android-native game engine, D3D11/DXGI compatibility layer over Vulkan/Turnip, encrypted archive support, full RPF/RSC resource decoding, asset conversion/mounting, Android lifecycle/input/audio/save integration, and end-to-end device validation are all still outstanding.

The iOS executable and its Apple-platform dynamic libraries cannot be reused as Android binaries. Android requires compatible native ELF libraries and Android platform integration. PC game data alone is not executable game code.

## Safety and compatibility

Keep this work on the experimental `android-pc-runtime-foundation` branch. The normal Glue 5 build and WASM runtime remain separate and unchanged. Do not describe this crate or its CI tests as proof that GTA V currently runs on Android.
