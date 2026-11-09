# RPF archive support

This is a local, Unlicense-licensed vendor copy of
[VIRUXE/rpf-archive-rs](https://github.com/VIRUXE/rpf-archive-rs).

Glue 5 extends the RPF7 writer to preserve stored compressed/encrypted
payloads, compress newly added binary files, and retain per-entry encryption
flags while applying OpenIV archive operations. The upstream parser and
archive format implementation remain attributed to the original project.

The Android installer intentionally rejects NG-encrypted RPF7 archives and
unsupported archive commands rather than writing a potentially damaged file.
