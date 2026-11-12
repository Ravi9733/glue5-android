#!/usr/bin/env python3
"""Print minimal XHYN host declarations and SDL call evidence from JADX output."""
from __future__ import annotations
import argparse
import re
from pathlib import Path

DECLARATIONS = (
    re.compile(r"^\s*(?:public|protected|private)?\s*(?:static\s+)?(?:final\s+)?(?:class|interface)\s+GTAVActivity\b"),
    re.compile(r"^\s*(?:public|protected|private)\s+[^=;]+\b(?:loadLibraries|onCreate|onResume|onPause|onDestroy|getLibraries|getMainFunction|nativeRunMain)\s*\([^;]*\)"),
    re.compile(r"^\s*(?:public|protected|private)\s+[^=;]+\b(?:surfaceCreated|surfaceChanged|surfaceDestroyed)\s*\([^;]*\)"),
)
SAFE_HOST_LINES = re.compile(
    r"SDLActivity|nativeRunMain|SDL_main|loadLibraries|getLibraries|getMainFunction|"
    r"System\.loadLibrary|loadLibrary\(|new String\[\]|"
    r"surfaceCreated|surfaceChanged|surfaceDestroyed|SurfaceHolder|"
    r"super\.(?:onCreate|onResume|onPause|onDestroy|loadLibraries)|"
    r"onCreate\(|onResume\(|onPause\(|onDestroy\(|"
    r"GameSurface|setMainReady|mSDLThread|mSurface|DxvkManager"
)

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("jadx_sources", type=Path)
    args = parser.parse_args()
    if not args.jadx_sources.is_dir():
        parser.error("JADX sources directory does not exist")
    found = 0
    for path in sorted(args.jadx_sources.rglob("*.java")):
        if "GTAVActivity" not in path.name and "GameSurface" not in path.name:
            continue
        lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
        selected: set[int] = set()
        for index, line in enumerate(lines):
            if any(pattern.search(line) for pattern in DECLARATIONS):
                selected.add(index)
            if SAFE_HOST_LINES.search(line):
                selected.add(index)
                if index > 0:
                    selected.add(index - 1)
                if index + 1 < len(lines):
                    selected.add(index + 1)
        if selected:
            print(f"## {path.name}")
            for index in sorted(selected):
                print(f"{index + 1}: {lines[index].strip()[:300]}")
                found += 1
    if not found:
        print("No matching host declarations found; inspect JADX package/class naming.")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
