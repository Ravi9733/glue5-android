# GTA V Auto RPF Converter

Android ARM64 prototype for inventorying and extracting supported GTA V RPF7 archives directly from Android. No desktop PC is required for the app workflow.

## Current build

- Select a GTA V folder using Android Storage Access Framework.
- Recursively inventory .rpf files and inspect bounded RPF7 metadata / OPEN TOC asset names.
- Select and extract one supported archive.
- Bulk-extract up to 500 confirmed OPEN RPF7 archives from the selected folder.
- Write extracted files to a separately selected destination folder; source archives are never modified.
- Native Rust extraction uses rpf-archive 0.10.0, with path traversal and extraction-size limits.
- AES/NG encrypted archives are skipped or rejected unless legitimate matching key support is supplied.

## What this app does NOT do yet

This is NOT yet a complete PC-to-Android RPF converter. Extraction only unpacks entries from archives the native parser can read. It does not transform RSC7 assets or create Android-ready RPF/DLC packs.

Not implemented or not verified:
- Nested RPF recursion as a conversion pipeline.
- Gen9-to-Legacy YTD conversion (research only; not integrated).
- YDR/YDD/YFT/YBN/YMAP/YTYP conversion.
- RPF repacking / DLC pack generation.
- Target Glue5 runtime compatibility validation and in-game tests.
- Encrypted archive support without legitimate matching keys.

## Safe usage

1. Select the GTA V root folder and wait for the inventory scan.
2. Select a separate output folder with enough free space.
3. Use single-archive extraction or bulk extraction.
4. Keep the app open during extraction. Large archives need temporary internal storage approximately equal to the selected archive plus extracted payload.
5. Check the report: failed or skipped archives are not treated as converted.

The app never changes source RPF files. Output is ordinary extracted content, not a replacement game installation. Keep backups and do not overwrite original game data.

## Build

GitHub Actions builds an Android debug APK and runs native Rust unit tests. A successful APK build verifies compilation only; it does not prove converted assets work in Glue5.

Workflow artifact: gta-v-auto-rpf-converter-debug.

## Licensing / game data

The app contains no proprietary GTA V game assets. Users must provide their own legally obtained game files. Encrypted archives require legitimate matching key material; this app does not recover or bypass keys.
