# GTA V PC RPF to Android RPF Converter

Android ARM64 prototype for converting supported, unencrypted GTA V PC RPF7 archives into a newly-built unencrypted RPF7 archive on Android. No desktop PC is required.

## Current conversion pipeline

1. Select one PC .rpf archive and an output folder.
2. Extract supported entries with the Rust RPF parser.
3. Convert supported Gen9 / GTA V Enhanced .ytd texture dictionaries to Legacy .ytd using rage-formats.
4. Preserve all other extracted files byte-for-byte (they are NOT format-converted).
5. Rebuild the extracted tree as a new RPF7 archive using rage-rpf, then parse the result for structural validation.
6. Save the new *_android.rpf file to the selected Android folder. The original PC archive is never modified.

## Important limitations

This is a first working conversion pipeline, not a universal GTA V PC-to-Android converter.

- Only Gen9 YTD to Legacy YTD is currently transformed. Legacy YTDs are retained as-is.
- YDR, YDD, YFT, YBN, YMAP, YTYP, YCD, YSC, AWC, REL and DAT files are preserved unchanged; no Android-specific conversion is claimed for them.
- The output is a newly packed unencrypted RPF7 container. It is not proven compatible with Glue5 or every Android GTA V port.
- Nested RPF recursion and full DLC folder / dlclist generation are not implemented in this first pipeline.
- AES/NG encrypted input archives are rejected unless legitimate matching key support is added. No encryption bypass is attempted.
- Runtime verification on a physical Android device and in-game Glue5 testing are still required.

## Safety

- Input archives are read-only; generated output is separate.
- Extraction and repacking enforce entry, per-file and total-size limits.
- Symlinks and unsafe archive paths are rejected.
- Keep a backup and ensure sufficient free storage for temporary extraction plus the new RPF.

## Build

GitHub Actions runs Rust native-core tests and builds an Android ARM64 debug APK. A successful build confirms compilation and packaging, not in-game compatibility.
