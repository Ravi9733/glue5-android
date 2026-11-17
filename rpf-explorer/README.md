# GTA V RPF Explorer (Android)

A standalone Android project in this repository for browsing user-supplied GTA V PC RPF7 archives.

## Current MVP scope
- Android system document picker; archives are copied to app cache on disk (not loaded wholesale into RAM).
- RPF7 header and TOC validation.
- Browse directory entries in **OPEN** and **NONE** RPF7 archives.
- Extract unencrypted binary entries; raw-DEFLATE compressed entries are decompressed.
- Resource entries are identified and listed, but not yet decoded or exported.
- AES/NG encrypted TOCs and encrypted file payloads are reported as unsupported until GTA V key handling is implemented.
- Original archives are never modified.

## Build
Open this folder as an Android Gradle project in Android Studio, or run Gradle 8.9 with JDK 17:
`gradle assembleDebug`

APK output: `app/build/outputs/apk/debug/app-debug.apk`.

A GitHub Actions workflow builds the debug APK on pushes to the development branch and on manual dispatch. Download it from the workflow run's Artifacts section.

## Editor implementation roadmap

The next development stage is an in-app archive editor. It will provide **Add file**, **Replace file**, and **Save as new RPF** operations. Edits must be staged in a working copy and the source archive must remain unchanged. The writer will rebuild the TOC, name table, alignment and file offsets rather than patching bytes in place. The first safe writer target is OPEN/NONE RPF7; AES/NG read support and resource-aware replacement are separate compatibility milestones and must be validated before claiming support.

Planned validation includes round-trip open/save/reopen tests, duplicate-name and nested-folder cases, compressed binary files, large-file streaming, and checks that the original archive hash is unchanged.

## Important limitations
This is an initial functional parser, not a full OpenIV replacement. YTD/YDR/YDD preview, resource extraction, nested archive traversal, AES/NG key support, archive editing/repacking, and exhaustive compatibility testing are not implemented yet. Test with a copy of an OPEN/NONE RPF7 mod archive first. Do not overwrite original game archives.
