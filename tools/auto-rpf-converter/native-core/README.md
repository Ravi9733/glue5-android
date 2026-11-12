# Native RPF core

This is the first real extraction layer for the Android converter. It uses the
upstream `rpf-archive` parser rather than the prototype's hand-written TOC
reader.

## Current scope

- Parse archive metadata through `RpfArchive`.
- Extract supported unencrypted archive entries into a separate destination.
- Preserve the source archive.
- Reject path traversal and enforce archive / entry / total-output limits.
- Fail closed for encrypted archives when matching key material is unavailable.

## Not yet implemented

- Android JNI bridge and Storage Access Framework integration.
- Nested RPF recursion.
- Batch job progress / cancellation.
- RSC7 asset conversion or Android-port compatibility validation.
- Repacking converted assets.

This crate is a host-buildable core, not yet wired into the APK. Its use of
`rpf-archive` is pinned to 0.10.0. The upstream crate is Unlicense-licensed.
