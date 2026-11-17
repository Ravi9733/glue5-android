# RPF Explorer — implementation checklist

This branch is for the standalone Android RPF Explorer only. Do not change the Glue5 app or its main branch.

## Required editor behavior
- [x] Add file from Android Storage Access Framework into a selected archive folder.
- [x] Replace an existing binary or RSC7 resource entry while retaining its archive path/name (resource input must be a valid small RSC7 file).
- [x] Remove an entry with an explicit confirmation, including staged additions.
- [x] Save as a **new** RPF file; never overwrite the selected source archive by default.
- [x] Reopen the generated archive and validate its header, TOC, names, directory ranges, and extracted payload hashes.
- [x] Stream archive payloads and output files; compressed binary entries are currently bounded by the RPF 24-bit stored-size field.
- [x] Stage edits in app-private working files; canceling/discarding never changes the source archive (per-operation undo is not yet available).

## Format and compatibility
- [x] Parse RPF7 directory, binary and resource entries, including RSC7 size sentinel and 23-bit offsets.
- [x] Detect encrypted `.ysc` resource entries; export decrypted/decompressed standalone script bytecode when compatible local AES/NG material is available.
- [x] Keep encrypted YSC repacking blocked until a format-correct resource re-encryption writer is available.
- [x] Handle NONE and OPEN TOCs.
- [x] Add AES TOC/data decryption using the AES key extracted locally from the user's own compatible PC GTA5.exe.
- [x] Add streaming AES and NG block-decryption regression tests and app-private persistence for locally extracted NG tables.
- [x] Add NG TOC and binary payload decryption using key material extracted from the user's own compatible PC GTA5.exe; no game keys/tables are bundled.
- [x] Preserve untouched resource entries byte-for-byte and retain page flags when rebuilding.
- [x] Add/replace resource assets only after validating RSC7 header and reading resource page flags (large new RSC7 assets are blocked safely).
- [x] Handle nested RPFs as separately editable archives and stage the edited nested archive back into its parent.
- [x] Clearly identify unknown encryption markers and encrypted YSC repacking as unsupported instead of silently corrupting them.
- [x] Add AES YSC decrypt/decompress export regression coverage.
- [ ] Decode and visually preview YTD textures and YDR/YDD models (requires a full RSC7 page/resource decoder; header inspection is not a preview).

## UI and safety
- [x] Add actions for Add file, long-press Replace/Remove, and Save as new RPF.
- [x] Show staged-change count and require confirmation before opening the Android output-document picker.
- [x] Warn/block unknown encryption markers, encrypted YSC resource operations, oversized new RSC7 assets, and malformed entries.
- [x] Add a bounded-memory RSC7 header/version/page-size inspector and raw export action (decoded texture/model preview remains unsupported).
- [x] Keep source archive read-only and report save only after the writer's reopen/TOC/payload-hash validation.

## Release gate
Do not create or publish an APK until the editor and parser tests pass on real OPEN/NONE archives and the encrypted-archive behavior is accurately reported. Build the APK only after the release gate is satisfied.
