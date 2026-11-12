#!/usr/bin/env python3
"""Print minimal XHYN 0.3 host declarations from a local JADX output."""
from __future__ import annotations
import argparse
import re
from pathlib import Path

PATTERNS = (
    re.compile(
        r"^\s*(?:public|protected|private)?\s*(?:static\s+)?"
        r"(?:final\s+)?(?:class|interface)\s+GTAVActivity\b"
    ),
    re.compile(
        r"^\s*(?:public|protected|private)\s+[^=;]+\b"
        r"(?:loadLibraries|onResume|onPause|onDestroy|getLibraries|nativeRunMain)\s*\([^;]*\)\s*(?:throws [^{]+)?\{?"
    ),
    re.compile(
        r"^\s*(?:public|protected|private)\s+[^=;]+\b"
        r"(?:surfaceCreated|surfaceChanged|surfaceDestroyed)\s*\([^;]*\)\s*(?:throws [^{]+)?\{?"
    ),
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
        for number, line in enumerate(path.read_text(encoding="utf-8", errors="replace").splitlines(), 1):
            if any(pattern.search(line) for pattern in PATTERNS):
                print(f"{path.name}:{number}: {line.strip()[:300]}")
                found += 1
    if not found:
        print("No matching host declarations found; inspect JADX package/class naming locally.")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
