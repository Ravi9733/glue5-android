#!/usr/bin/env python3
"""Fail if the pinned Mugoat source no longer matches documented Android blockers.

This is a source audit, not an Android build or gameplay test. It deliberately
checks concrete platform-specific build assumptions before anyone attempts to
advertise an APK as ready.
"""
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[2]
UPSTREAM = ROOT / "third_party/mugoat-upstream/native-port/app"
checks = {
    "Apple-only preparation guard": (
        UPSTREAM / "Scripts/prepare.py",
        "This preparation script targets an Apple Silicon Mac with Xcode.",
    ),
    "Metal-only shader writer": (
        UPSTREAM / "ShaderTool/build_pgmsl.sh",
        "-DTINT_BUILD_MSL_WRITER=1",
    ),
    "SPIR-V writer disabled in shader tool": (
        UPSTREAM / "ShaderTool/build_pgmsl.sh",
        "-DTINT_BUILD_SPV_WRITER=0",
    ),
    "Metal renderer entry point": (
        UPSTREAM / "Source/NativeGraphics.h",
        "PGAttachMetalLayer",
    ),
}
errors = []
for label, (path, needle) in checks.items():
    if not path.is_file():
        errors.append(f"{label}: missing source file {path.relative_to(ROOT)}")
        continue
    if needle not in path.read_text(errors="replace"):
        errors.append(f"{label}: expected marker not found in {path.relative_to(ROOT)}")
    else:
        print(f"CONFIRMED blocker: {label}")

if errors:
    print("\n".join(f"ERROR: {e}" for e in errors), file=sys.stderr)
    sys.exit(1)

print(
    "No Android-ready renderer is implied by this check. "
    "The pinned upstream still has Apple/Metal build assumptions; "
    "an Android host, Vulkan renderer/shader path, and successful APK build remain required."
)
