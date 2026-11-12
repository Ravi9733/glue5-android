# GTA V Auto RPF Converter

Android prototype for selecting a GTA V folder and inventorying its RPF archives.

## Current prototype (0.3.0)

- Uses Android's Storage Access Framework; the user selects the GTA V folder.
- Recursively finds files ending in `.rpf`.
- Reads and validates bounded RPF7 table-of-contents metadata (entry count and names-table bounds).
- For OPEN/unencrypted archives, inventories recognized asset names such as YTD, YDR, YDD, YFT, YBN, YMAP and YTYP.
- For AES/NG archives, reports that the TOC is encrypted and requires matching key material; it does not attempt key recovery.
- Rejects TOCs above conservative safety limits (1,000,000 entries / 64 MiB names / 80 MiB total TOC).
- Produces a shareable text report.
- Does not alter, extract, repack, or convert any game files.
- Marks AES/NG archives as requiring matching key material; it does not bypass encryption.

## Important compatibility note

An RPF archive being readable or writable is not the same as converting PC assets to the format expected by a particular Android port. RPF7 is the outer archive; streamed resources inside it (for example YTD/YDR/YFT/YMAP and RSC7 payloads) may need target-engine-specific transformation. The target Android port's loader/version must be tested before any asset is labelled compatible.

## Planned stages

1. RPF7 header and OPEN TOC inventory (prototype 0.3).
2. Full archive parser integration and nested-RPF inventory.
3. Asset-type inspection (YTD, YDR, YDD, YFT, YBN, YMAP, YTYP, etc.).
4. Compare asset/resource versions and texture formats against the target Android engine.
5. Convert only formats proven incompatible; preserve compatible files byte-for-byte.
6. Build output archives in a separate folder and validate them.
7. Runtime test in the target Android port.

## Engineering work in progress: real extraction and conversion

The next implementation uses the public-domain Rust libraries below rather than extending the prototype's hand-written TOC parser:

- `rpf-archive` 0.10.x (Unlicense): RPF7 directory/file entry parsing, OPEN archive extraction, nested RPF traversal, decompression and RPF7 writing.
- `rage-formats` 0.5.x (Unlicense): RSC7 resource parsing and format-specific readers/writers for YTD, YDR, YDD, YFT, YBN, YMAP, YTYP and related data.

The Android integration will use an ARM64 native library (JNI) with Android Storage Access Framework streams. It must not require a desktop PC or copy the whole GTA V installation into app-private storage. Files will be read in bounded chunks and converted outputs will be written to a separate user-selected destination; originals remain untouched.

### Conversion rules and release gates

1. Replace the prototype TOC reader with the upstream RPF parser and test it against real OPEN RPF7 archives, nested archives, compressed entries, empty files and malformed offsets.
2. Add extraction preview and per-archive progress/cancellation before enabling bulk extraction.
3. Identify each asset's RSC7 resource version/page flags and edition. Preserve byte-identical resources when compatible; never relabel a PC asset as Android-compatible without a target-runtime test.
4. Implement only conversions with a verified source/target layout. The current upstream research reports YTD Gen9-to-Legacy conversion as implemented; YDR/YDD/YFT conversion is not yet implemented. Those formats must be reported as unsupported rather than silently rewritten.
5. Build output RPFs only from validated converted assets, then reopen each output with the archive parser and verify every entry hash/size and extracted payload.
6. Run the resulting packs in the selected Android GTA V port. A successful APK build alone is not proof that the generated game assets work.

AES/NG archives remain locked unless the user supplies legitimate matching key material. The app will not recover keys from game executables or bypass encryption. No proprietary GTA V game data will be bundled in the APK.

### Current status

The 0.3 prototype APK builds successfully and performs inventory only. The native parser/converter bridge and actual asset conversion are not yet implemented. Do not treat this prototype as the finished converter.
