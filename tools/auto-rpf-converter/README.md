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
4. Parse and validate edition-independent resources with the pinned ragekit readers, then preserve their original bytes: .ybn, .ymap, .ytyp, .ycd, .ynd, .yvr, .ywr, .yld, .yed, .yfd, .ybd and .ynv. YCD/YED/YFD also run ragekit's pointer-census checks and fail closed if the pinned schema encounters unknown pointer-like fields. These formats do not need Gen9-to-Legacy conversion; unnecessary reserialization could change collision, map, navigation or animation data. They are reported as validated/preserved, never as converted.
5. Validate supported binary audio/metadata containers with the pinned ragekit readers, then preserve their original bytes:
   - .awc: read the audio container, decrypt with the user's imported gtav_awc_key.dat when required, decode PCM/IMA-ADPCM streams for validation, and require an exact read/write byte round trip. Enhanced codecs such as codec 7 remain opaque; no audio transcoding is claimed.
   - .rel and .dat4/.dat10/.dat15/.dat16/.dat22/.dat54/.dat149/.dat150/.dat151: parse the REL container, check the embedded type for .datN files, and require byte-identical write-back.
   - .gxt2 text tables, .nametable sidecars, PSO (.ymt/.ypdb/.ymf/.pso/.cut) and RBF (.ymt/.ymf) metadata: parse and require byte-identical write-back. An unrecognized .ymt/.ymf magic is preserved but not reported as validated.
   These formats are validated/preserved, not converted to another edition. RPF binary entries remain source-backed in the rebuilt archive.
6. Preserve already-Legacy versions byte-for-byte. Unsupported extensions such as .ysc, cache .dat files and other unrecognized files are preserved unchanged and are never reported as converted. The report separates parser-validated binary formats from unvalidated pass-through files.
7. Rebuild a new OPEN RPF7 archive with ragekit's RpfBuilder::save file-streaming writer. The output is reopened with the RPF reader and checked for OPEN encryption, paths, entry kinds, nested archives and converted resource versions. RSC7 resources are written as resource entries with their page flags. Nested .rpf files are rebuilt as child OPEN archives.

## Nested RPF and DLC layout

- Nested .rpf entries are opened directly, converted and rebuilt up to depth 8, with at most 500 nested archives per top-level archive. The rebuilt nested archive is put back under the same internal filename and path.
- Nested RPF entries are rebuilt as child OPEN RPF7 archives. Their original relative paths and filenames are retained. A nested archive is limited to 256 MiB because the pinned RAGEKIT writer materializes each child archive before writing its parent.
- Bulk output mirrors the selected folder's relative paths. For example, dlcpacks/dyfurai/dlc.rpf is emitted as dlcpacks/dyfurai/dlc.rpf under the selected output folder. The converter does not rename dlc.rpf or other referenced archives. It does not install or mount the output in Glue5; copy the output tree over the same relative paths in the game root configured by your Glue5 build, keeping a backup.
- dlclist.xml and setup2.xml are not edited. Preserving the existing path and filename avoids breaking references, but DLC registration, load order and runtime mounting still depend on the target Android port.


## DLC pack metadata validation

The Android app includes a read-only validator for one selected dlc.rpf. Choose the pack's actual dlc.rpf (not update.rpf) and tap **VALIDATE SELECTED DLC.RPF METADATA**. It opens the archive with the imported matching AES/NG keys when needed, parses its root setup2.xml, follows the declared datFile (normally content.xml), and validates the CDataFileMgr__ContentsOfDataFileXml data-file list. It checks the deviceName prefix, unsafe/traversal paths, missing references, and whether entries declared as RPF_FILE are nested RPF archives. Nested paths are resolved through the RPF tree. The validator never edits the source archive.

This remains a single-pack validator; it does not edit setup2.xml or content.xml. A separate full load-order audit now reads dlclist.xml from update.rpf and cross-checks dlcpacks entries against the selected game folder.


## Full DLC load-order audit

Select the GTA V root folder and tap **VALIDATE FULL DLC LOAD ORDER**. The audit is read-only:
- It prefers `mods/update/update.rpf` when present (OpenIV override), otherwise `update/update.rpf`.
- It reads `common/data/dlclist.xml` inside that archive using the imported matching AES/NG keys.
- It recognizes both `dlcpacks:/name/` and Windows-backslash variants, preserves the list order, and detects duplicate, unsafe and unsupported entries.
- For each `dlcpacks` entry it checks `mods/update/x64/dlcpacks/name/dlc.rpf` first, then `update/x64/dlcpacks/name/dlc.rpf`. It copies one pack at a time to app-private cache, preserves the basename `dlc.rpf` for NG key selection, and runs the existing setup2/content metadata validator.
- The report shows list position separately from each pack's `setup2.xml` order, plus missing content references and RPF_FILE type mismatches. Source archives and XML files are never modified.

Safety limits: at most 1,024 dlclist Items are parsed, 512 DLC pack entries are audited per Android run, each archive is limited to 4 GiB minus one byte, and the cumulative copied bytes (including update.rpf, normal DLC archives and scanned base x64 archives) are capped at 16 GiB. The remaining byte budget is passed into each SAF copy operation, so unknown-length provider documents cannot overrun the 16 GiB cap; the audit stops at the cap instead of silently skipping the remainder.

`platform:/dlcPacks/` entries are resolved read-only by scanning the selected game root's `x64w.rpf` through `x64a.rpf` files for the case-insensitive internal path `dlcpacks/<name>/dlc.rpf`. The nested pack is opened file-backed with RAGEKIT and its `setup2.xml` plus declared content XML are validated without extracting the whole base archive. A found but malformed/unsupported nested archive is reported separately from a pack not found at the expected internal path. If a base archive is encrypted without matching imported keys, unreadable, absent, or the cumulative 16 GiB audit budget is reached, remaining platform entries are reported as unknown/unresolved rather than falsely marked missing. If the nested `dlc.rpf` is found but its own AES/NG keys are missing, it is reported as key-locked (not malformed); importing the matching legitimate key material allows the metadata check. This is still a read-only list/metadata audit, not a game mount test; it does not certify Glue5 compatibility or rewrite DLC registration. The audit reads the selected `update.rpf` list; the separate `common.rpf` dlclist is not scanned.

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
- The Android app now converts archives directly: RAGEKIT reads the source archive from disk, unchanged entries remain source-backed/lazy, and only supported resources plus the explicitly validated audio/metadata binaries are extracted for parsing. Aggregate supported-asset inspection is limited to 512 MiB per top-level archive; one entry is limited to 512 MiB. RAGEKIT streams the top-level OPEN RPF7 writer to disk. Nested RPFs are rebuilt recursively but their completed child archives are materialized in memory, hence the stricter 256 MiB nested-archive limit.
- Symlinks, path traversal, absolute/drive-qualified paths, duplicate case-insensitive paths, oversized inputs and output collisions are rejected. An invalid nested archive, missing nested AES/NG key, excessive nesting or size-limit violation aborts the top-level conversion; no partial final archive is reported as successful.
- Ensure sufficient free storage for the cached source copy, temporary output archive and SAF destination.

## Compatibility status and remaining work

This is not a universal PC-to-Android converter and is not yet validated against Glue5's runtime. It produces Legacy-format RSC7 assets inside a structurally validated RPF7 archive. A successful parse/build does not prove that a specific Android port mounts the archive, supports the same shader/material features, loads the DLC metadata or accepts every asset.

The supported Gen9-to-Legacy transformations are limited to YTD/YDR/YDD/YFT/YPT; YPT files containing the Enhanced-only ptfx_g9 behaviour fail closed because Legacy has no equivalent. YBN/YMAP/YTYP/YCD/YND/YVR/YWR/YLD/YED/YFD/YBD/YNV are parsed by pinned ragekit readers and preserved byte-for-byte because their supported layouts are edition-independent. AWC/REL/DAT*/GXT2/PSO/RBF/nametable files are parser-validated and preserved byte-for-byte, not converted; AWC codecs 0 and 4 are decoded in memory for validation, while other codecs remain opaque. Scripts and unrecognized metadata/resources remain unchanged. Real retail Enhanced/Legacy fixture coverage, large DLC validation, Android device testing and Glue5 in-game loading tests are still required.

## Glue5 compatibility gate

The current Glue5 README explicitly says that files from a regular PC or console installation alone are not sufficient. Glue5 needs the supported WebAssembly build's `game.wasm` at build time, plus its prepared game assets and translated shaders at runtime. This converter only transforms selected resources inside RPF archives; it cannot generate the supported `game.wasm`, translated shader set, or the browser build's complete prepared data set.

The converter emits structurally validated OPEN RPF7 archives. That proves the archive can be parsed by the pinned RAGEKIT reader, not that Glue5's browser-build asset pipeline will accept or mount it. The exact archive-mounting and data-preparation requirements remain unverified; do not replace a working Glue5 game-data folder with this output. Keep a separate backup and use only a test copy until the runtime requirement is proven.


The Glue5 project's main README documents the runtime game directory as `data/` plus the matching `b/<build>/shaders/`, and requires the supported `game.wasm` at build time. It does not document a separate RPF re-keying algorithm or target archive key. Therefore a re-keying requirement must not be assumed. This converter emits structurally validated OPEN RPF7 archives, but whether the supported browser-build RPF reader accepts modified OPEN archives and whether every converted Legacy resource matches that build remain unverified. A regular retail PC/console installation alone is not sufficient to run Glue5.

Before calling the project complete, verify the exact supported game build and data layout, establish from the actual target runtime whether OPEN RPF7 archives are accepted, confirm the resource edition requirements, and load representative real-game archives in Glue5. Implement only a preparation step proven necessary by the target runtime. Keep original data and a separate backup. The documented runtime requirements are in the [Glue5 README](https://github.com/Ravi9733/glue5-android#notes) and [design notes](https://github.com/Ravi9733/glue5-android/blob/main/docs/design.md#game-files-are-never-in-the-app).

## Pinned upstream implementation

The C++ conversion, keyed RPF extraction and OPEN RPF7 writer use RatPrez/ragekit v0.3.1, commit 9d4110ef0fce900787aaeccb52b67fa5402ff41b. The pin matches the upstream v0.3.1 tag. Its Gen9 layout and conversion behavior are documented in [ragekit's Enhanced format notes](https://github.com/RatPrez/ragekit/blob/v0.3.1/docs/enhanced.md), and its RPF7 reader/writer API is in [ragekit's RPF header](https://github.com/RatPrez/ragekit/blob/v0.3.1/include/ragekit/rpf.h).

## Tests and build

GitHub Actions runs:
- Rust native-core unit tests (legacy prototype)
- Host C++ tests for Gen9-to-Legacy YTD/YDR/YDD/YFT/YPT conversion and Legacy pass-through
- Android JVM tests for exact key-file lengths, the complete NG key group, and invalid/empty key selections
- YBN/YMAP/YTYP plus YCD/YND/YVR/YWR/YLD/YED/YFD/YBD/YNV parse-and-preserve tests, including pointer-census rejection for YCD/YED/YFD and fail-closed YPT coverage for Enhanced-only ptfx_g9 behaviour
- Binary audio/metadata tests: plain and whole-file encrypted AWC with a user-supplied test key, missing-key rejection, PCM decode, opaque codec-7 preservation, all nine DAT type/extension pairs, GXT2/PSO/RBF/nametable byte-exact round trips, both PSO/RBF variants under .ymt and .ymf, and OPEN RPF7 extraction equality
- Direct RPF7 round-trip tests proving the additional edition-independent resources remain byte-identical after repacking
- End-to-end direct archive-to-archive conversion tests covering supported resource families, nested RPFs, AES/NG input, OPEN RPF7 output, encrypted .ysc stored-byte preservation, AWC key-required parsing/decryption, REL/DAT type checks, GXT2/PSO/RBF/nametable exact round trips and binary-entry preservation
- Synthetic AES- and NG-keyed RPF extraction and missing-key rejection
- A nested RPF7 round-trip larger than 16 MiB
- Synthetic dlclist.xml parsing for Windows separators, platform entries, traversal rejection, unknown mount schemes and missing-file rejection
- Synthetic base x64w.rpf containing a nested platform `dlcPacks` entry, including metadata validation, expected-path not-found detection, malformed nested-RPF rejection, encrypted-child key-lock/unlock checks and unsafe pack-name rejection
- Android ARM64 debug APK build

Fixtures are synthetic, not a full retail GTA V installation. Physical Android and Glue5 in-game tests have not been completed.


## Output integrity and Android packaging

- Native preflight walks the RPF entry tree, rejects unreachable/duplicate paths and enforces the nested depth/count/size limits before the writer is created.
- After a converted RPF is copied to the selected SAF output folder, the app reads the finalized document back with a fixed-size buffer and compares SHA-256 plus the byte count against the generated archive. A failed verification deletes the partial/finalized output and reports failure.
- The Android APK links the pinned ragekit C++ JNI library. The older Rust native-core crate remains covered by a separate CI test job as legacy prototype code, but its unused JNI wrapper and shared library are no longer packaged into the APK.


## Platform DLC resolution details

The platform resolver uses the pinned RAGEKIT `Rpf::resolve` and `Rpf::open_child` APIs. It locates `dlcpacks/<pack>/dlc.rpf` in each base archive, rejects compressed or entry-encrypted nested archive entries instead of guessing how to open them, and opens valid nested archives file-backed so the base archive is not expanded into RAM. Community archive inventories document platform packs in base archives (for example `x64w.rpf/dlcpacks/mpbeach/dlc.rpf`); the scan covers `x64w.rpf` down through `x64a.rpf` because pack placement varies by title update. Synthetic fixtures validate the path and parser flow; retail game archive verification remains outstanding. References: [GTA V archives and content inventory](https://github.com/Goodvibes74/gta5-under-the-hood/blob/main/docs/core/archives-and-content.md) and [GTA V archive path inventory](https://smidity.com/en/docs/gta-paths).

## Temporary workspace cleanup

The Android app uses a guarded app-private workspace for each selected-archive conversion, each bulk archive, and each DLC audit. The workspace is cleared before use and deleted in a `finally` block after success or failure, so multi-gigabyte source/output copies do not accumulate between archives. Workspace paths are checked for symbolic links before recursive deletion. JVM tests cover stale-workspace replacement, cleanup after use, and symlink rejection.
