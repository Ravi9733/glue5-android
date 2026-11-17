# RPF Explorer — implementation checklist

This branch is for the standalone Android RPF Explorer only. Do not change the Glue5 app or its main branch.

## Required editor behavior
- [ ] Add file from Android Storage Access Framework into a selected archive folder.
- [ ] Replace an existing binary entry while retaining its archive path/name.
- [ ] Remove an entry with an explicit confirmation.
- [ ] Save as a **new** RPF file; never overwrite the selected source archive by default.
- [ ] Reopen the generated archive and validate its header, TOC, names, offsets and extracted payload hashes.
- [ ] Stream large files; do not load a whole archive into RAM.
- [ ] Keep a working-copy/undo boundary so canceling never changes the source.

## Format and compatibility
- [ ] Correctly parse RPF7 directory, binary and resource entries, including size/offset sentinels.
- [ ] Correctly handle NONE and OPEN TOCs.
- [ ] Add AES TOC/data decryption using a key sourced from the user's own compatible game installation.
- [ ] Add NG TOC/data decryption using the compatible game key material; do not bundle Rockstar keys.
- [ ] Preserve untouched resource entries byte-for-byte when rebuilding.
- [ ] Replace/add resource assets only after validating RSC7 header and resource page flags.
- [ ] Handle nested RPFs as separately editable archives and safely replace the nested archive in its parent.
- [ ] Clearly identify unsupported archive variants instead of silently corrupting them.

## UI and safety
- [ ] Add toolbar actions: Add file, Replace selected, Remove selected, Save as new RPF.
- [ ] Show pending edits and target output path before saving.
- [ ] Warn when archive type/entry type is unsupported.
- [ ] Keep source archive read-only and report successful save only after reopen validation.

## Release gate
Do not create or publish an APK until the editor and parser tests pass on real OPEN/NONE archives and the encrypted-archive behavior is accurately reported. Build the APK only after the release gate is satisfied.
