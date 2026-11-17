# GTA V PC RPF to Android RPF Converter

Android ARM64 conversion prototype for GTA V RPF7 archives. It reads a selected archive directly from disk or recursively discovers RPF files in a selected GTA V folder, converts supported Enhanced (Gen9) resources to the Legacy resource layouts used by the classic PC edition, and builds new OPEN RPF7 archives. Original PC archives are never modified.

## Current supported pipeline

1. Select one .rpf archive and an output folder, or select a GTA V folder and bulk-process up to 500 on-disk RPF files. Folder discovery is capped at 250,000 items. Bulk output mirrors the original relative folder paths and preserves each .rpf basename, so existing DLC path references can still match when the output tree is overlaid at the same relative location. Use an empty output folder; existing output filenames are rejected rather than renamed or overwritten.
2. Read RPF7 archives directly with the pinned ragekit reader. Unchanged entries stay source-backed/lazy; only supported resources are extracted for parsing and conversion. OPEN/None archives need no keys. AES and NG archives are opened only when matching key material is imported from the user's own installation; missing or invalid keys cause conversion to fail. No key is bundled, guessed, uploaded or bypassed.
3. Convert Enhanced Gen9 resources to Legacy with the pinned ragekit v0.3.1 C++ library:
   - .ytd texture dictionaries: Gen9 version 5 to Legacy version 13
   - .ydr drawables and .ydd drawable dictionaries: Gen9 version 159 (and the CodeWalker-compatible 154 variant) to Legacy version 165
   - .yft fragments: Gen9 version 171 to Legacy version 162
   - .ypt particle dictionaries: Gen9 version 71 to Legacy version 68, except files containing the Enhanced-only ptfx_g9 behaviour are rejected safely
4. Parse and validate .ybn, .ymap and .ytyp resources with ragekit, then preserve their original bytes. These formats are edition-independent in the upstream Gen9 format documentation; rewriting them is unnecessary and could introduce collision quantization or map serialization changes. They are not counted as Gen9-to-Legacy conversions.
5. Preserve already-Legacy versions byte-for-byte. Unsupported extensions such as .ysc, .awc, .rel, .dat, PSO/meta and other files are preserved unchanged and are never reported as converted.
6. Rebuild a new OPEN RPF7 archive with ragekit's RpfBuilder::save file-streaming writer. The output is reopened with the RPF reader and checked for OPEN encryption, paths, entry kinds, nested archives and converted resource versions. RSC7 resources are written as resource entries with their page flags. Nested .rpf files are rebuilt as child OPEN archives.

## Nested RPF and DLC layout

- Nested .rpf entries are opened directly, converted and rebuilt up to depth 8, with at most 500 nested archives per top-level archive. The rebuilt nested archive is put back under the same internal filename and path.
- Nested RPF entries are rebuilt as child OPEN RPF7 archives. Their original relative paths and filenames are retained. A nested archive is limited to 256 MiB because the pinned RAGEKIT writer materializes each child archive before writing its parent.
- Bulk output mirrors the selected folder's relative paths. For example, dlcpacks/dyfurai/dlc.rpf is emitted as dlcpacks/dyfurai/dlc.rpf under the selected output folder. The converter does not rename dlc.rpf or other referenced archives. It does not install or mount the output in Glue5; copy the output tree over the same relative paths in the game root configured by your Glue5 build, keeping a backup.
- dlclist.xml and setup2.xml are not edited. Preserving the existing path and filename avoids breaking references, but DLC registration, load order and runtime mounting still depend on the target Android port.


## DLC pack metadata validation

The Android app includes a read-only validator for one selected dlc.rpf. Choose the pack's actual dlc.rpf (not update.rpf) and tap **VALIDATE SELECTED DLC.RPF METADATA**. It opens the archive with the imported matching AES/NG keys when needed, parses its root setup2.xml, follows the declared datFile (normally content.xml), and validates the CDataFileMgr__ContentsOfDataFileXml data-file list. It checks the deviceName prefix, unsafe/traversal paths, missing references, and whether entries declared as RPF_FILE are nested RPF archives. Nested paths are resolved through the RPF tree. The validator never edits the source archive.

This is pack-level validation only. It does not yet read dlclist.xml from update/update.rpf, compare the full game's DLC load order, resolve platform:/dlcPacks/ entries from base x64*.rpf archives, or rewrite setup2.xml/content.xml. Platform-dependent %PLATFORM% references are reported separately rather than guessed. A clean metadata report does not prove Glue5 mounts the pack.

## Key import

The app can import recognized key files from a user-selected folder into app-private storage:

| File | Required size |
|---|---:|
| gtav_aes_key.dat | 32 bytes |
| gtav_ng_key.dat | 101 × 272 bytes |
| gtav_ng_decrypt_tables.dat | 17 × 16 × 256 × 4 bytes |
| gtav_hash_lut.dat | 256 bytes |
| gtav_awc_key.dat | 16 bytes |

AES archives require the matching AES key. NG archives require all NG keys, decrypt tables and the hash LUT. The app does not generate keys from GTA5.exe plus magic.dat; users must provide matching key files. Imported files remain in app-private storage. Imports are staged and checked before activation, so a failed import retains the previous working key set. A successful import replaces the active key set with the selected files. The three NG files are accepted only as a complete group; partial NG key sets are rejected.

## Safety and resource limits

- Source PC archives are read-only. The selected archive is copied to app-private cache, read file-backed by ragekit, and written to a separate temporary output archive before SAF export.
- Top-level RPF7 input/output archives are limited to 4 GiB minus one byte (the RPF7 header/key-size and resource-offset safety bound). Each entry is limited to 512 MiB; nested archives are limited to 256 MiB and depth 8.
- The converter validates at most one million RPF entries across the nested tree. Individual entries are limited to 512 MiB; nested archives are limited to 256 MiB so the RAGEKIT nested writer remains memory-bounded.
- The Android app now converts archives directly: RAGEKIT reads the source archive from disk, unchanged entries remain source-backed/lazy, and only supported resources are extracted for parsing/conversion. Supported resource inspection is limited to 512 MiB aggregate per top-level archive; one entry is limited to 512 MiB. RAGEKIT streams the top-level OPEN RPF7 writer to disk. Nested RPFs are rebuilt recursively but their completed child archives are materialized in memory, hence the stricter 256 MiB nested-archive limit.
- Symlinks, path traversal, absolute/drive-qualified paths, duplicate case-insensitive paths, oversized inputs and output collisions are rejected. An invalid nested archive, missing nested AES/NG key, excessive nesting or size-limit violation aborts the top-level conversion; no partial final archive is reported as successful.
- Ensure sufficient free storage for the cached source copy, temporary output archive and SAF destination.

## Compatibility status and remaining work

This is not a universal PC-to-Android converter and is not yet validated against Glue5's runtime. It produces Legacy-format RSC7 assets inside a structurally validated RPF7 archive. A successful parse/build does not prove that a specific Android port mounts the archive, supports the same shader/material features, loads the DLC metadata or accepts every asset.

The supported Gen9-to-Legacy transformations are limited to YTD/YDR/YDD/YFT/YPT; YPT files containing the Enhanced-only ptfx_g9 behaviour fail closed because Legacy has no equivalent. YBN/YMAP/YTYP are parsed for validation and preserved byte-for-byte because their layouts are edition-independent. Audio, scripts, other metadata and unsupported resources remain unchanged. Real retail Enhanced/Legacy fixture coverage, large DLC validation, Android device testing and Glue5 in-game loading tests are still required.

## Glue5 compatibility gate

The current Glue5 README explicitly says that files from a regular PC or console installation alone are not sufficient. Glue5 needs the supported WebAssembly build's `game.wasm` at build time, plus its prepared game assets and translated shaders at runtime. This converter only transforms selected resources inside RPF archives; it cannot generate the supported `game.wasm`, translated shader set, or the browser build's complete prepared data set.

The converter emits structurally validated OPEN RPF7 archives. That proves the archive can be parsed by the pinned ragekit reader, not that Glue5's browser-build asset pipeline will accept or mount it. The exact target archive preparation/key requirements remain unverified; do not replace a working Glue5 game-data folder with this output. Keep a separate backup and use only a test copy until the runtime requirement is proven.


The Glue5 project's own README and design notes say its runtime needs the supported browser build's prepared game data (re-keyed archives); a regular retail PC/console installation by itself is not sufficient. This converter currently emits structurally validated RPF7 archives with the OPEN encryption tag. It does not yet implement or verify the browser build's target re-keying/key scheme. Therefore this APK is a PC RPF extraction/resource-conversion prototype, not a certified Glue5-ready game-data converter.

Before calling the project complete, the exact target archive/key requirements of the supported browser build must be verified, the corresponding safe re-key/output mode must be implemented (or OPEN archives must be proven accepted), and the resulting data must be tested by loading it in Glue5. The relevant runtime requirement is documented in the [Glue5 README](https://github.com/Ravi9733/glue5-android#notes) and [design notes](https://github.com/Ravi9733/glue5-android/blob/main/docs/design.md#game-files-are-never-in-the-app).

## Pinned upstream implementation

The C++ conversion, keyed RPF extraction and OPEN RPF7 writer use RatPrez/ragekit v0.3.1, commit 9d4110ef0fce900787aaeccb52b67fa5402ff41b. The pin matches the upstream v0.3.1 tag. Its Gen9 layout and conversion behavior are documented in [ragekit's Enhanced format notes](https://github.com/RatPrez/ragekit/blob/v0.3.1/docs/enhanced.md), and its RPF7 reader/writer API is in [ragekit's RPF header](https://github.com/RatPrez/ragekit/blob/v0.3.1/include/ragekit/rpf.h).

## Tests and build

GitHub Actions runs:
- Rust native-core unit tests (legacy prototype)
- Host C++ tests for Gen9-to-Legacy YTD/YDR/YDD/YFT/YPT conversion and Legacy pass-through
- Android JVM tests for exact key-file lengths, the complete NG key group, and invalid/empty key selections
- YBN/YMAP/YTYP parse-and-preserve tests, plus fail-closed YPT coverage for the Enhanced-only ptfx_g9 behaviour
- End-to-end direct archive-to-archive conversion tests covering supported resource families, nested RPFs, AES/NG input, OPEN RPF7 output and encrypted .ysc stored-byte preservation
- Synthetic AES- and NG-keyed RPF extraction and missing-key rejection
- A nested RPF7 round-trip larger than 16 MiB
- Android ARM64 debug APK build

Fixtures are synthetic, not a full retail GTA V installation. Physical Android and Glue5 in-game tests have not been completed.


## Output integrity and Android packaging

- Native preflight walks the RPF entry tree, rejects unreachable/duplicate paths and enforces the nested depth/count/size limits before the writer is created.
- After a converted RPF is copied to the selected SAF output folder, the app reads the finalized document back with a fixed-size buffer and compares SHA-256 plus the byte count against the generated archive. A failed verification deletes the partial/finalized output and reports failure.
- The Android APK links the pinned ragekit C++ JNI library. The older Rust native-core crate remains covered by a separate CI test job as legacy prototype code, but its unused JNI wrapper and shared library are no longer packaged into the APK.
