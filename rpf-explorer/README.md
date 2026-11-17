# GTA V RPF Explorer (Android)

A standalone Android project in this repository for browsing user-supplied GTA V PC RPF7 archives.

## Current MVP scope
- Android system document picker; archives are copied to app cache on disk (not loaded wholesale into RAM).
- RPF7 header and TOC validation.
- Browse directory entries in **OPEN**, **NONE**, **AES**, and **NG** RPF7 archives. AES/NG keys are extracted locally from a user-selected PC `GTA5.exe`; no game keys are bundled.
- Extract plain, AES-encrypted, and NG-encrypted binary entries when compatible local key material is available; raw-DEFLATE compressed entries are decompressed.
- Resource entries are identified, listed, inspected with a bounded-memory RSC7 header/page-flag inspector, and exported as raw RSC7 files for OPEN/NONE/NG archives; raw payload/page flags are preserved when repacking. AES resource operations remain blocked; decoded texture/model preview is not implemented.
- Long-press entries to replace or remove; add files into the currently open folder.
- Save As New RPF rebuilds the directory TOC, name table, offsets, and 512-byte alignment without modifying the source.
- OPEN/NONE and AES archives are supported. For AES archives, import the user's own `GTA5.exe`; the app scans its AES-key fingerprint locally, decrypts TOCs and encrypted binary payloads, and writes the edited result as an unencrypted NONE archive. The key itself is not bundled.
- RSC7 resource entries are retained byte-for-byte; replacement/addition requires a valid RSC7 file smaller than 16 MiB so its page flags can be read unambiguously. Existing large resources can still be preserved.
- JUnit regression tests cover binary add/replace/remove, AES TOC/data decryption, streaming AES, NG block/stream material handling, raw-DEFLATE conversion, nested-directory TOC rebuilding, source-hash preservation, and RSC7 payload/page-flag preservation. Check the test-only workflow for the latest commit before treating it as passed.

## Build
Open this folder as an Android Gradle project in Android Studio, or run Gradle 8.9 with JDK 17:
`gradle assembleDebug`

APK output: `app/build/outputs/apk/debug/app-debug.apk`.

The APK workflow is manual-dispatch only. A separate test-only workflow runs unit tests on development-branch pushes and does not create an APK.

## Editor implementation roadmap

The in-app editor now stages **Add file**, **Replace file**, and **Remove** operations, then writes **Save as New RPF**. The source archive remains read-only. The writer rebuilds the TOC, name table, alignment and file offsets. Binary files are written uncompressed; unchanged compressed binaries are decompressed before writing. RSC7 resources are copied raw and retain their system/graphics page flags. The writer supports OPEN/NONE, AES, and NG RPF7 (AES/NG inputs are decrypted and emitted as NONE). AES archives containing retained RSC7 resources remain blocked because their resource payload path has not been validated.

Planned validation includes round-trip open/save/reopen tests, duplicate-name and nested-folder cases, compressed binary files, large-file streaming, and checks that the original archive hash is unchanged.

## Important limitations
This is an initial functional parser, not a full OpenIV replacement. YTD/YDR/YDD decoded preview, NG encryption support, and exhaustive compatibility testing on retail game archives are not implemented yet. AES key extraction currently targets compatible PC `GTA5.exe` builds. Nested RPFs can be opened and staged changes can be applied back into their parent archive. The editor blocks unknown encryption markers and AES archives containing retained resource entries; NG TOC and binary payload decryption require local material extracted from a compatible PC executable. Test with a copy of an OPEN/NONE RPF7 archive first, then validate AES on a compatible PC game build. Do not overwrite original game archives.


AES key-fingerprint scan reference: CodeWalker `GTA5Keys` (MIT-licensed reference implementation); only the SHA-1 fingerprint is used, and the key is extracted from the user's selected executable at runtime.
