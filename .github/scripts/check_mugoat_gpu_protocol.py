#!/usr/bin/env python3
"""Static compatibility guard for the Mugoat and Glue5 GPU ring opcode IDs.

This intentionally checks only framing constants and opcode numbers. Passing
this check does NOT prove payload-layout, memory-address, or synchronization
compatibility; those require runtime/backend tests.
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MUGOAT = ROOT / "third_party/mugoat-upstream/native-port/app/Source/MetalRenderer/MRRenderer.mm"
GLUE5 = ROOT / "crates/glue5-runtime/src/gpu.rs"
GLUE5_EXEC = ROOT / "crates/glue5-gpu/src/exec.rs"


def fail(message: str) -> None:
    print(f"GPU protocol check FAILED: {message}", file=sys.stderr)
    raise SystemExit(1)


def parse_mugoat(text: str) -> dict[str, int]:
    match = re.search(r"enum\s+Op\s*:\s*uint32_t\s*\{(.*?)\};", text, re.S)
    if not match:
        fail(f"could not find upstream enum Op in {MUGOAT}")
    result: dict[str, int] = {}
    for name, value in re.findall(r"\b([A-Z][A-Z0-9_]*)\s*=\s*(\d+)\b", match.group(1)):
        result[name] = int(value)
    if not result:
        fail("upstream opcode enum parsed as empty")
    return result


def parse_glue5(text: str) -> dict[str, int]:
    match = re.search(r"pub\s+mod\s+op\s*\{(.*?)\n\}", text, re.S)
    if not match:
        fail(f"could not find Rust pub mod op in {GLUE5}")
    result: dict[str, int] = {}
    for name, value in re.findall(r"pub\s+const\s+([A-Z][A-Z0-9_]*)\s*:\s*u32\s*=\s*(\d+)\s*;", match.group(1)):
        # SCENE_DONE is a Glue5 host hook, not an upstream engine opcode.
        if name != "SCENE_DONE":
            result[name] = int(value)
    if not result:
        fail("Glue5 opcode module parsed as empty")
    return result


def main() -> int:
    if not MUGOAT.is_file():
        fail(f"upstream submodule/file missing: {MUGOAT}; initialize third_party/mugoat-upstream first")
    if not GLUE5.is_file():
        fail(f"Glue5 GPU protocol file missing: {GLUE5}")
    if not GLUE5_EXEC.is_file():
        fail(f"Glue5 GPU executor missing: {GLUE5_EXEC}")

    upstream = parse_mugoat(MUGOAT.read_text(encoding="utf-8"))
    glue5 = parse_glue5(GLUE5.read_text(encoding="utf-8"))
    executor_text = GLUE5_EXEC.read_text(encoding="utf-8")
    missing = sorted(set(upstream) - set(glue5))
    extra = sorted(set(glue5) - set(upstream))
    mismatched = sorted((name, upstream[name], glue5[name]) for name in set(upstream) & set(glue5) if upstream[name] != glue5[name])

    if missing or extra or mismatched:
        if missing:
            print("Missing in Glue5:", ", ".join(missing), file=sys.stderr)
        if extra:
            print("Only in Glue5 (excluding SCENE_DONE):", ", ".join(extra), file=sys.stderr)
        for name, a, b in mismatched:
            print(f"Opcode mismatch {name}: Mugoat={a}, Glue5={b}", file=sys.stderr)
        fail("opcode tables differ")

    # WRAP is consumed by the shared ring reader before commands reach exec.rs.
    # Every other engine opcode should have an explicit executor reference.
    executor_ops = set(re.findall(r"op::([A-Z][A-Z0-9_]*)", executor_text))
    unhandled = sorted(set(upstream) - executor_ops - {"WRAP"})
    if unhandled:
        fail("engine opcodes with no executor reference (excluding reader-handled WRAP): " + ", ".join(unhandled))

    if "RING_DATA = 64" not in MUGOAT.read_text(encoding="utf-8") or "pub const RING_DATA: u32 = 64;" not in GLUE5.read_text(encoding="utf-8"):
        fail("RING_DATA offset is not 64 in both implementations")
    if upstream.get("WRAP") != 1 or glue5.get("WRAP") != 1:
        fail("WRAP opcode is not 1 in both implementations")

    print(f"GPU ring framing constants and {len(upstream)} engine opcode IDs match.")
    print(f"Glue5 GPU executor references all {len(upstream) - 1} non-WRAP engine opcodes.")
    print("Note: opcode references do not prove correct payload handling, shader formats, resource lifetime or GPU completion semantics.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
