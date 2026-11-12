# GTA V Auto RPF Converter

Android prototype for selecting a GTA V folder and inventorying its RPF archives.

## Current prototype (0.3.0)

- Uses Android's Storage Access Framework; the user selects the GTA V folder.
- Recursively finds files ending in `.rpf`.
- Reads and validates bounded RPF7 table-of-contents metadata (entry count and names-table bounds).
- For OPEN/unencrypted archives, inventories recognized asset names such as YTD, YDR, YDD, YFT, YBN, YMAP and YTYP.
- For AES/NG archives, reports that the TOC is encrypted and requires matching key material; it does not attempt key recovery.
- Rejects TOCs above conservative safety limits (1,000,000 entries / 64 MiB names / 80 MiB total TOC).
- Produces a shareable text report.
- Does not alter, extract, repack, or convert any game files.
- Marks AES/NG archives as requiring matching key material; it does not bypass encryption.

## Important compatibility note

An RPF archive being readable or writable is not the same as converting PC assets to the format expected by a particular Android port. RPF7 is the outer archive; streamed resources inside it (for example YTD/YDR/YFT/YMAP and RSC7 payloads) may need target-engine-specific transformation. The target Android port's loader/version must be tested before any asset is labelled compatible.

## Planned stages

1. RPF7 header and OPEN TOC inventory (prototype 0.3).
2. Full archive parser integration and nested-RPF inventory.
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

This prototype inventories OPEN RPF7 asset names only. It does not extract, transform, repack, or convert assets. AES/NG TOCs are not inspected without valid keys. No game data is included. No game data is included.
