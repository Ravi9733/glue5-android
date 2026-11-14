# GTA V PC RPF to Android RPF Converter

Android ARM64 conversion prototype for GTA V RPF7 archives. It extracts a selected archive or recursively discovers RPF files in a selected GTA V folder, converts supported Enhanced (Gen9) resources to the Legacy resource layouts used by the classic PC edition, and builds new OPEN RPF7 archives. Original PC archives are never modified.

## Current supported pipeline

1. Select one .rpf archive and an output folder, or select a GTA V folder and bulk-process up to 500 on-disk RPF files. Folder discovery is capped at 250,000 items. Bulk output mirrors the original relative folder paths and preserves each .rpf basename, so existing DLC path references can still match when the output tree is overlaid at the same relative location. Use an empty output folder; existing output filenames are rejected rather than renamed or overwritten.
2. Extract RPF7 archives with the pinned ragekit reader. OPEN/None archives need no keys. AES and NG archives are attempted only when the matching key material is imported from the user's own installation; missing or invalid keys cause extraction to fail. No key is bundled, guessed, uploaded or bypassed.
3. Convert Enhanced Gen9 resources to Legacy with the pinned ragekit v0.3.1 C++ library:
   - .ytd texture dictionaries: Gen9 version 5 to Legacy version 13
   - .ydr drawables and .ydd drawable dictionaries: Gen9 version 159 (and the CodeWalker-compatible 154 variant) to Legacy version 165
   - .yft fragments: Gen9 version 171 to Legacy version 162
   - .ypt particle dictionaries: Gen9 version 71 to Legacy version 68
4. Parse and serialize .ybn, .ymap and .ytyp resources with ragekit. These formats are edition-independent in the upstream Gen9 format documentation; they are not counted as Gen9-to-Legacy conversions. The converter reports them separately.
5. Preserve already-Legacy versions byte-for-byte. Unsupported extensions such as .ysc, .awc, .rel, .dat, PSO/meta and other files are preserved unchanged and are never reported as converted.
6. Rebuild a new OPEN RPF7 archive with ragekit's RpfBuilder::save file-streaming writer. The output is reopened with the RPF reader and checked for OPEN encryption and the expected file-entry count. RSC7 resources are written as resource entries with their page flags; nested .rpf files are stored as uncompressed binary entries.

## Nested RPF and DLC layout

- Nested .rpf entries are recursively extracted, converted and rebuilt up to depth 8, with at most 500 nested archives per top-level archive. The rebuilt nested archive is put back under the same internal filename and path.
- The RAGEKIT writer stores nested .rpf binary entries without deflate compression and uses the RPF7 32-bit uncompressed-size field. A nested RPF larger than 16 MiB is supported; the current aggregate loose-file input limit still applies.
- Bulk output mirrors the selected folder's relative paths. For example, dlcpacks/dyfurai/dlc.rpf is emitted as dlcpacks/dyfurai/dlc.rpf under the selected output folder. The converter does not rename dlc.rpf or other referenced archives.
- dlclist.xml and setup2.xml are not edited. Preserving the existing path and filename avoids breaking references, but DLC registration, load order and runtime mounting still depend on the target Android port.

## Key import

The app can import recognized key files from a user-selected folder into app-private storage:

| File | Required size |
|---|---:|
| gtav_aes_key.dat | 32 bytes |
| gtav_ng_key.dat | 101 × 272 bytes |
| gtav_ng_decrypt_tables.dat | 17 × 16 × 256 × 4 bytes |
| gtav_hash_lut.dat | 256 bytes |
| gtav_awc_key.dat | 16 bytes |

AES archives require the matching AES key. NG archives require all NG keys, decrypt tables and the hash LUT. The app does not generate keys from GTA5.exe plus magic.dat; users must provide matching key files. Imported files remain in app-private storage.

## Safety and resource limits

- Source PC archives are read-only; extraction and conversion run in app-private cache, then write a separate output archive.
- RPF input archives are limited to 1 GiB; individual extracted entries/resources are limited to 512 MiB; extracted output is limited to 8 GiB per archive.
- The converter scans at most one million extracted files and 8 GiB of asset data. Supported resources are limited to 512 MiB each.
- The RPF7 builder buffers source file contents in memory, so aggregate loose-file input is limited to 512 MiB per output archive and generated RPF output to 1 GiB. The writer now streams the completed archive to disk instead of building a second archive-sized output buffer, but large individual files can still require an additional temporary buffer.
- Symlinks, path traversal, absolute/drive-qualified paths, duplicate case-insensitive extracted paths, oversized inputs and output collisions are rejected. Failed nested archives are preserved and reported as skipped; they are not claimed as converted.
- Ensure sufficient free storage for the temporary extracted tree and the rebuilt output.

## Compatibility status and remaining work

This is not a universal PC-to-Android converter and is not yet validated against Glue5's runtime. It produces Legacy-format RSC7 assets inside a structurally validated RPF7 archive. A successful parse/build does not prove that a specific Android port mounts the archive, supports the same shader/material features, loads the DLC metadata or accepts every asset.

The supported Gen9-to-Legacy transformations are limited to YTD/YDR/YDD/YFT/YPT. YBN/YMAP/YTYP are parsed and serialized because their layouts are edition-independent. Audio, scripts, other metadata and unsupported resources remain unchanged. Real retail Enhanced/Legacy fixture coverage, large DLC validation, Android device testing and Glue5 in-game loading tests are still required.

## Pinned upstream implementation

The C++ conversion, keyed RPF extraction and OPEN RPF7 writer use RatPrez/ragekit v0.3.1, commit 9d4110ef0fce900787aaeccb52b67fa5402ff41b. The pin matches the upstream v0.3.1 tag. Its Gen9 layout and conversion behavior are documented in [ragekit's Enhanced format notes](https://github.com/RatPrez/ragekit/blob/v0.3.1/docs/enhanced.md), and its RPF7 reader/writer API is in [ragekit's RPF header](https://github.com/RatPrez/ragekit/blob/v0.3.1/include/ragekit/rpf.h).

## Tests and build

GitHub Actions runs:
- Rust native-core unit tests (legacy prototype)
- Host C++ tests for Gen9-to-Legacy YTD/YDR/YDD/YFT/YPT conversion and Legacy pass-through
- YBN/YMAP/YTYP parse/serialize tests
- An end-to-end folder-convert-to-OPEN-RPF7 integration test covering all supported resource families and unchanged files
- Synthetic AES-keyed RPF extraction and missing-key rejection
- A nested RPF7 round-trip larger than 16 MiB
- Android ARM64 debug APK build

Fixtures are synthetic, not a full retail GTA V installation. Physical Android and Glue5 in-game tests have not been completed.
