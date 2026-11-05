# Android PC placement-content pipeline (experimental)

This document describes the current preparation API in `glue5-pc-native`.
It is not a claim that PC GTA V content can yet be played in Glue5.

## What the current pipeline does

The standalone entry point
`prepare_rpf7_archive_json_placement_pipeline` connects these existing stages:

1. Inspect an unencrypted GTA V Legacy RPF7 archive using the OPEN/NONE inventory parser.
2. Add the archive's file entries to an in-memory mount index under a caller-provided virtual prefix.
3. Resolve every JSON placement before processing any asset payload.
4. Read the referenced archive ranges with per-file and aggregate source-byte limits.
5. Decode supported RSC7 resources with per-resource and aggregate output limits.
6. Inspect drawable resources and read geometry buffers.
7. Assemble CPU meshes using a caller-supplied, verified Rockstar vertex declaration.
8. Apply placement scale, rotation and translation.
9. Pack world-space position/index data into bounded GPU payloads.
10. Optionally upload those payloads to a caller-supplied wgpu device with
    `upload_placement_gpu_payloads`.

The upload function creates vertex/index buffers only. The caller must supply
the wgpu device owned by Glue5's Vulkan renderer on Android.

## API sketch

```rust
use glue5_pc_native::{
    parse_json_placements,
    prepare_rpf7_archive_json_placement_pipeline,
    upload_placement_gpu_payloads,
    PlacementPipelineLimits,
    InspectedPlacementGeometryBuffers,
    VertexElement,
    ManifestError,
};

let placements = parse_json_placements(placement_json_bytes)?;
let prepared = prepare_rpf7_archive_json_placement_pipeline(
    rpf7_archive_bytes,
    "my_dlc.rpf",
    "dlcpacks/my_dlc",
    &placements,
    &PlacementPipelineLimits::default(),
    |geometry: &InspectedPlacementGeometryBuffers| -> Result<Vec<VertexElement>, ManifestError> {
        // Return only a layout decoded and validated from this retail resource.
        // Never infer declaration types from vertex stride alone.
        resolve_verified_vertex_layout(geometry)
    },
)?;
let gpu_buffers = upload_placement_gpu_payloads(
    glue5_vulkan_device,
    &prepared.gpu_payloads,
    16_384,
    1024 * 1024 * 1024,
)?;
```

The sketch intentionally leaves `resolve_verified_vertex_layout` and
`glue5_vulkan_device` as integration points; they are not implemented by this
preparation API.

## Current boundaries

- RPF7 OPEN/NONE metadata and supported uncompressed archive entries only.
  Encrypted/NG archives and RPF8 are rejected.
- RSC7 page decoding is not full RAGE object parsing or pointer relocation.
- YDR/YDD inspection is not yet a fully validated retail vertex-layout decoder.
  The pipeline requires a verified declaration resolver from the caller.
- YTD parsing remains experimental; this geometry-only path filters texture
  dictionaries out of drawable mesh inspection.
- GPU buffer upload is not a render pass: no material/shader binding, draw call,
  world/entity spawn, collision, or physics integration is performed here.
- The current Android application still uses the existing WASM game runtime.
  This placement pipeline does not replace `game.wasm` or provide a native
  GTA V game engine.
- No Turnip driver loader/selector is added by this pipeline. It uses whatever
  Vulkan adapter/device Glue5's renderer has already created.
- Windows ASI/DLL binaries are not executable Android plugins. A compatible
  scripting/native plugin ABI is still separate work.

## Tests

Unit tests in `placement_pipeline.rs` cover default budget sanity and rejection
of a truncated archive. A successful Android build and retail-asset/device
validation are still required before this pipeline can be called operational.
