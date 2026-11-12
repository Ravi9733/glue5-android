#!/usr/bin/env python3
"""Print a minimal, non-source XHYN 0.3 host contract from a local JADX output."""
from __future__ import annotations
import argparse
import re
from pathlib import Path

TARGETS = {
    "GTAVActivity.java": re.compile(
        r"^\s*(?:public|protected|private)?\s*(?:static\s+)?"
        r"(?:final\s+)?(?:class|interface)\s+GTAVActivity\b|"
        r"^\s*(?:public|protected|private)\s+[^=;]+\b"
        r"(?:loadLibraries|onResume|onPause|onDestroy|getLibraries|nativeRunMain)\s*\([^;]*\)\s*(?:throws [^{]+)?\{?"
    ),
    "GameSurface.java": re.compile(
        r"^\s*(?:public|protected|private)\s+[^=;]+\b"
        r"(?:surfaceCreated|surfaceChanged|surfaceDestroyed)\s*\([^;]*\)\s*(?:throws [^{]+)?\{?"
    ),
}

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("jadx_sources", type=Path)
    args = parser.parse_args()
    if not args.jadx_sources.is_dir():
        parser.error("JADX sources directory does not exist")
    found = 0
    for filename, pattern in TARGETS.items():
        for path in sorted(args.jadx_sources.rglob(filename)):
            for number, line in enumerate(path.read_text(encoding="utf-8", errors="replace").splitlines(), 1):
                if pattern.search(line):
                    print(f"{path.name}:{number}: {line.strip()[:300]}")
                    found += 1
    if not found:
        print("No matching host signatures found; inspect package/class names locally.")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
