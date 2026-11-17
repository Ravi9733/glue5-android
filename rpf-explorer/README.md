# GTA V RPF Explorer (Android)

A standalone Android project in this repository for browsing user-supplied GTA V PC RPF7 archives.

## Current MVP scope
- Android system document picker; archives are copied to app cache on disk (not loaded wholesale into RAM).
- RPF7 header and TOC validation.
- Browse directory entries in **OPEN** and **NONE** RPF7 archives.
- Extract unencrypted binary entries; raw-DEFLATE compressed entries are decompressed.
- Resource entries are identified, listed, and their raw RSC7 payload/page flags are preserved when repacking; resource preview/extraction is not implemented.
- Long-press entries to replace or remove; add files into the currently open folder.
- Save As New RPF rebuilds the directory TOC, name table, offsets, and 512-byte alignment without modifying the source.
- The writer supports OPEN/NONE archives only. AES/NG encrypted TOCs and encrypted binary payloads remain unsupported until user-supplied GTA V key handling is implemented.
- RSC7 resource entries are retained byte-for-byte; replacement/addition requires a valid RSC7 file. The RSC7 page flags are read from its header.
- JUnit regression tests cover binary add/replace/remove, round-trip reopening, source-hash preservation, and RSC7 payload/page-flag preservation.

## Build
Open this folder as an Android Gradle project in Android Studio, or run Gradle 8.9 with JDK 17:
`gradle assembleDebug`

APK output: `app/build/outputs/apk/debug/app-debug.apk`.

A GitHub Actions workflow builds the debug APK on pushes to the development branch and on manual dispatch. Download it from the workflow run's Artifacts section.

## Editor implementation roadmap

The in-app editor now stages **Add file**, **Replace file**, and **Remove** operations, then writes **Save as New RPF**. The source archive remains read-only. The writer rebuilds the TOC, name table, alignment and file offsets. Binary files are written uncompressed; unchanged compressed binaries are decompressed before writing. RSC7 resources are copied raw and retain their system/graphics page flags. The supported writer target is OPEN/NONE RPF7; AES/NG support remains a separate compatibility milestone.

Planned validation includes round-trip open/save/reopen tests, duplicate-name and nested-folder cases, compressed binary files, large-file streaming, and checks that the original archive hash is unchanged.

## Important limitations
This is an initial functional parser, not a full OpenIV replacement. YTD/YDR/YDD preview and resource extraction, nested archive traversal, AES/NG key support, and exhaustive compatibility testing are not implemented yet. The editor deliberately blocks repacking encrypted archives and refuses malformed resource replacements. Test with a copy of an OPEN/NONE RPF7 archive first. Do not overwrite original game archives.
