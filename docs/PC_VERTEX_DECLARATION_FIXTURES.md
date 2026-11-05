# Retail GTA V vertex declaration validation

## Current boundary

The decoder in `crates/glue5-pc-native/src/vertex_decode.rs` accepts explicit legacy declaration flags, stride, and component-type nibbles. The placement adapter deliberately requires a caller-supplied declaration. Unit fixtures currently prove the byte-layout arithmetic only; they do not prove that the flag/type mapping matches every retail GTA V build.

## Required fixture workflow

1. Use only a user-owned, unmodified GTA V Legacy RPF7 archive and record its build/version and SHA-256.
2. Preserve the smallest relevant YDR/YDD drawable resource and its referenced vertex/index buffer pages as a private test fixture. Do not commit copyrighted game assets.
3. Record the extracted declaration metadata (flags, type word, declared stride, semantic order) alongside a fixture manifest. Include the extraction tool/version and source archive hash.
4. Compare the parser output against an independently inspected declaration and verify position, normal, colour, UV, blend-weight and blend-index streams where present.
5. Add synthetic public tests for every supported nibble and rejection tests for unknown flags, unsupported encodings, stride mismatch, truncated streams, non-finite floats and out-of-range indices.

## Acceptance gate

A retail layout is not considered verified until a reproducible user-run fixture report confirms the declaration fields and decoded vertex values for a named game build. Never infer a layout from stride alone. Half-float and undocumented encodings remain rejected until their exact conversion rules are established.
