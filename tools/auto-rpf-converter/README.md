# GTA V Auto RPF Converter

Android prototype for selecting a GTA V folder and inventorying its RPF archives.

## Current prototype (0.2.0)

- Uses Android's Storage Access Framework; the user selects the GTA V folder.
- Recursively finds files ending in `.rpf`.
- Reads the 16-byte RPF7 header and reports the archive entry count and encryption marker (OPEN, AES, NG, or unknown).
- Produces a shareable text report.
- Does not alter, extract, repack, or convert any game files.
- Marks AES/NG archives as requiring matching key material; it does not bypass encryption.

## Important compatibility note

An RPF archive being readable or writable is not the same as converting PC assets to the format expected by a particular Android port. RPF7 is the outer archive; streamed resources inside it (for example YTD/YDR/YFT/YMAP and RSC7 payloads) may need target-engine-specific transformation. The target Android port's loader/version must be tested before any asset is labelled compatible.

## Planned stages

1. RPF7 inventory, entry-count and encryption-marker detection (this prototype).
2. Archive parser integration and nested-RPF inventory.
3. Asset-type inspection (YTD, YDR, YDD, YFT, YBN, YMAP, YTYP, etc.).
4. Compare asset/resource versions and texture formats against the target Android engine.
5. Convert only formats proven incompatible; preserve compatible files byte-for-byte.
6. Build output archives in a separate folder and validate them.
7. Runtime test in the target Android port.

## Candidate format libraries

- https://github.com/RatPrez/ragekit
- https://github.com/VIRUXE/rpf-archive-rs
- https://github.com/VIRUXE/rage-formats

These are parser/format libraries, not proof of compatibility with the specific Android port. Review their licenses and build feasibility for Android/ARM64 before integrating.

## Build

Open this directory in Android Studio with JDK 17 and Android SDK 35, or use the repository GitHub Actions workflow.

The first release is a scanner, not yet an RPF converter. No game data is included.
