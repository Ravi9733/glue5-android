# GTA V PC RPF to Android RPF Converter

Android ARM64 converter prototype for unencrypted GTA V RPF7 archives. It extracts an archive, converts supported GTA V Enhanced (Gen9) resources to Legacy resource layouts, and builds a new unencrypted RPF7 container. The original PC archive is never modified.

## Current supported pipeline

1. Select one .rpf archive and an output folder, or select a GTA V folder and run bulk conversion of up to 500 on-disk .rpf files. Bulk output mirrors the source folder path and preserves the original `.rpf` filename exactly, so existing DLC path references can still match when the output tree is overlaid in the same layout. Folder discovery is capped at 250,000 entries. Use a separate, preferably empty output folder; an existing output filename is rejected rather than silently renamed or overwritten.
2. Extract supported unencrypted archives with the Rust rpf-archive parser.
3. Convert Gen9 resources to Legacy with the pinned ragekit v0.3.1 C++ library:
   - .ytd texture dictionaries (Gen9 version 5 to Legacy version 13)
   - .ydr drawables and .ydd drawable dictionaries (Gen9 versions 154/159 to Legacy version 165)
   - .yft fragments (Gen9 version 171 to Legacy version 162)
   - .ypt particle dictionaries (Gen9 version 71 to Legacy version 68)
4. Preserve already-Legacy versions byte-for-byte and leave other file types unchanged.
5. Rebuild a new, unencrypted RPF7 archive with rage-rpf; validate the generated archive with both RPF parsers.

## Important limitations

This is not yet a universal PC-to-Android converter and is not yet verified against Glue5's runtime.

- Gen9-to-Legacy conversion is attempted only for .ytd, .ydr, .ydd, .yft and .ypt. A resource that fails parsing or conversion stops that archive; it is not silently copied as if converted.
- .ybn, map/meta, audio, scripts, configuration and other formats are not rewritten. Some GTA V Legacy/Enhanced formats are edition-independent, but this does not establish compatibility with every Android port.
- Nested RPF file entries are recursively extracted, converted and rebuilt up to depth 8 and 500 nested files. Each nested archive must remain below the RPF7 24-bit binary-entry limit (16 MiB); larger nested files fail closed rather than being truncated.
- RPF7 output is structurally validated; preserving archive filenames and relative folders helps retain DLC paths, but `dlclist.xml` / `setup2.xml` are not rewritten and runtime mounting, DLC registration, load order, and Glue5 in-game asset compatibility are not yet validated.
- Bulk conversion processes .rpf files visible in the selected Android folder tree and recursively rebuilds nested RPF file entries that fit the 16 MiB RPF7 binary-entry limit.
- Bulk mode is limited to 500 on-disk RPF files and 1 GiB per input archive. Resource conversion enforces 512 MiB per supported asset, 8 GiB total extracted/scanned input and one million files. Because the current RPF7 writer buffers its input and output in memory, each rebuilt archive is capped at 512 MiB of aggregate loose-file input and 1 GiB of generated archive bytes; larger archives fail safely instead of risking an Android out-of-memory crash.
- AES/NG encrypted archives are skipped/rejected unless legitimate matching key material is available. No encryption bypass is attempted.
- The C++ conversion engine is pinned to RatPrez/ragekit commit 9d4110ef0fce900787aaeccb52b67fa5402ff41b (v0.3.1). Its Gen9 conversion work is documented at https://github.com/RatPrez/ragekit/blob/v0.3.1/docs/enhanced.md.
- Physical Android and Glue5 in-game tests are still required.

## Safety

- Source PC RPF archives are read-only; all extraction and conversion happens in app cache and a separate output folder.
- Symlinks, oversized supported resources and excessive directory workloads are rejected.
- Keep a backup and ensure enough free storage for the extracted tree and rebuilt RPF.

## Build

GitHub Actions runs Rust archive tests (including nested RPF and RSC7 resource-entry round trips), host C++ synthetic Gen9-to-Legacy conversion tests for YTD/YDR/YDD/YFT/YPT plus parse/rebuild tests for YBN/YMAP/YTYP, and an Android ARM64 debug APK build. These are synthetic fixtures, not a test against a complete retail GTA V installation; a successful build does not prove Glue5 runtime compatibility.
