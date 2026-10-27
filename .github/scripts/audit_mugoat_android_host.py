#!/usr/bin/env python3
"""Inventory platform-bound Mugoat host code before an Android port.

This produces a deterministic report of Apple-only dependencies in the pinned
native host. It does not claim to implement those APIs.
"""
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[2]
SRC = ROOT / "third_party/mugoat-upstream/native-port/app/Source"
if not SRC.is_dir():
    raise SystemExit("Missing pinned upstream Source tree; check submodules.")

patterns = {
    "Apple frameworks": re.compile(r"^\s*#\s*import\s*[<\"](?:Metal|MetalKit|UIKit|AVFAudio|CoreGraphics|QuartzCore|IOSurface|Foundation)/?", re.M),
    "Apple target headers": re.compile(r"^\s*#\s*include\s*[<\"](?:TargetConditionals\.h|mach/[^>\"]+|sys/sysctl\.h)", re.M),
    "Metal API references": re.compile(r"\b(?:MTLDevice|MTLCommandQueue|MTLCommandBuffer|MTLTexture|MTLRenderPipelineState|CAMetalLayer|MTKView)\b"),
    "Mach memory/thread APIs": re.compile(r"\b(?:mach_vm_[a-zA-Z0-9_]+|vm_allocate|vm_deallocate|pthread_mach_thread_np|thread_info)\b"),
}
files = sorted(p for p in SRC.rglob("*") if p.is_file() and p.suffix in {".c", ".h", ".m", ".mm", ".cc", ".cpp"})
if not files:
    raise SystemExit("No native source files found.")
total = 0
for label, pattern in patterns.items():
    hits = []
    for path in files:
        try:
            source = path.read_text(errors="replace")
        except OSError:
            continue
        count = len(pattern.findall(source))
        if count:
            hits.append((path.relative_to(ROOT).as_posix(), count))
    print(f"## {label}: {sum(n for _, n in hits)} matches across {len(hits)} files")
    for filename, count in hits:
        print(f"{filename}: {count}")
    total += sum(n for _, n in hits)
print(f"\nScanned {len(files)} native source files; found {total} platform-bound matches.")
print("Android work still requires explicit replacements/adapters and a real NDK build.")
