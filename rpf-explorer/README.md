# GTA V RPF Explorer (Android)

A standalone Android project in this repository for browsing user-supplied GTA V PC RPF7 archives.

## Current MVP scope
- Android system document picker; archives are copied to app cache on disk (not loaded wholesale into RAM).
- RPF7 header and TOC validation.
- Browse directory entries in **OPEN**, **NONE**, and AES RPF7 archives (AES requires a compatible local key).
- Extract plain binary entries and AES-encrypted binary entries when the local key is available; raw-DEFLATE compressed entries are decompressed.
- Resource entries are identified, listed, and exported as raw RSC7 files for OPEN/NONE archives; their raw payload/page flags are preserved when repacking. AES resource payload operations remain blocked; decoded preview is not implemented.
- Long-press entries to replace or remove; add files into the currently open folder.
- Save As New RPF rebuilds the directory TOC, name table, offsets, and 512-byte alignment without modifying the source.
- OPEN/NONE and AES archives are supported. For AES archives, import the user's own `GTA5.exe`; the app scans its AES-key fingerprint locally, decrypts TOCs and encrypted binary payloads, and writes the edited result as an unencrypted NONE archive. The key itself is not bundled.
- RSC7 resource entries are retained byte-for-byte; replacement/addition requires a valid RSC7 file smaller than 16 MiB so its page flags can be read unambiguously. Existing large resources can still be preserved.
- JUnit regression tests cover binary add/replace/remove, AES TOC/data decryption, round-trip reopening, source-hash preservation, and RSC7 payload/page-flag preservation (tests are authored but not yet executed).

## Build
Open this folder as an Android Gradle project in Android Studio, or run Gradle 8.9 with JDK 17:
`gradle assembleDebug`

APK output: `app/build/outputs/apk/debug/app-debug.apk`.

A GitHub Actions workflow builds the debug APK on pushes to the development branch and on manual dispatch. Download it from the workflow run's Artifacts section.

## Editor implementation roadmap

The in-app editor now stages **Add file**, **Replace file**, and **Remove** operations, then writes **Save as New RPF**. The source archive remains read-only. The writer rebuilds the TOC, name table, alignment and file offsets. Binary files are written uncompressed; unchanged compressed binaries are decompressed before writing. RSC7 resources are copied raw and retain their system/graphics page flags. The writer supports OPEN/NONE and AES RPF7 (AES inputs are decrypted and emitted as NONE). NG remains a separate compatibility milestone.

Planned validation includes round-trip open/save/reopen tests, duplicate-name and nested-folder cases, compressed binary files, large-file streaming, and checks that the original archive hash is unchanged.

## Important limitations
This is an initial functional parser, not a full OpenIV replacement. YTD/YDR/YDD decoded preview, NG encryption support, and exhaustive compatibility testing on retail game archives are not implemented yet. AES key extraction currently targets compatible PC `GTA5.exe` builds. Nested RPFs can be opened and staged changes can be applied back into their parent archive. The editor blocks NG archives and AES archives containing retained resource entries; it refuses malformed resource replacements. Test with a copy of an OPEN/NONE RPF7 archive first, then validate AES on a compatible PC game build. Do not overwrite original game archives.


AES key-fingerprint scan reference: CodeWalker `GTA5Keys` (MIT-licensed reference implementation); only the SHA-1 fingerprint is used, and the key is extracted from the user's selected executable at runtime.
