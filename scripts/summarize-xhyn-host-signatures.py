#!/usr/bin/env python3
"""Print selected XHYN launcher, data-path and SDL lifecycle evidence from JADX output."""
from __future__ import annotations
import argparse
import re
from pathlib import Path

TARGET_CLASS_NAMES = (
    "GTAVActivity", "GameSurface", "GameDirectories", "GameRoot",
    "StoragePermissionActivity", "GameMenuActivity", "ModSpawnProvider",
    "ArtworkGuardApplication",
)
DECLARATIONS = (
    re.compile(r"^\s*(?:public|protected|private)?\s*(?:static\s+)?(?:final\s+)?(?:class|interface)\s+\w+\b"),
    re.compile(r"^\s*(?:public|protected|private)\s+[^=;]+\b(?:loadLibraries|onCreate|onResume|onPause|onDestroy|getLibraries|getMainFunction|nativeRunMain|gameRoot|start|resolve|current)\s*\([^;]*\)"),
    re.compile(r"^\s*(?:public|protected|private)\s+[^=;]+\b(?:surfaceCreated|surfaceChanged|surfaceDestroyed)\s*\([^;]*\)"),
)
SAFE_HOST_LINES = re.compile(
    r"SDLActivity|nativeRunMain|SDL_main|loadLibraries|getLibraries|getMainFunction|"
    r"System\.loadLibrary|loadLibrary\(|new String\[\]|"
    r"surfaceCreated|surfaceChanged|surfaceDestroyed|SurfaceHolder|"
    r"super\.(?:onCreate|onResume|onPause|onDestroy|loadLibraries)|"
    r"onCreate\(|onResume\(|onPause\(|onDestroy\(|"
    r"GameSurface|setMainReady|mSDLThread|mSurface|DxvkManager|"
    r"gameRoot\(|game_root|getExternalFilesDir|getFilesDir|getExternalStorageDirectory|"
    r"Environment|Documents|ACTION_OPEN_DOCUMENT_TREE|StoragePermissionActivity|"
    r"ModSpawnProvider|\.modspawn|getPackageName\(|authority|content://|"
    r"GameDirectories|GameRoot|resolve\(|takePersistableUriPermission|"
    r"DocumentsContract|Uri|File\(|Android/data|ArtworkGuardApplication|"
    r"CLIENT|CERT"
)

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("jadx_sources", type=Path)
    args = parser.parse_args()
    if not args.jadx_sources.is_dir():
        parser.error("JADX sources directory does not exist")
    found = 0
    for path in sorted(args.jadx_sources.rglob("*.java")):
        if not any(name in path.name for name in TARGET_CLASS_NAMES):
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
