#!/usr/bin/env python3
"""Restore opaque original res/ entries that Apktool/aapt cannot recompile."""
from __future__ import annotations

import argparse
import zipfile
from pathlib import Path

REQUIRED = "res/drawable/titanium_full_fire.bin"


def restore(original: Path, rebuilt: Path) -> int:
    with zipfile.ZipFile(original, "r") as src, zipfile.ZipFile(rebuilt, "a") as dst:
        source_names = set(src.namelist())
        if REQUIRED not in source_names:
            raise SystemExit(f"required original startup artwork missing: {REQUIRED}")
        rebuilt_names = set(dst.namelist())
        if len(rebuilt_names) != len(dst.namelist()):
            raise SystemExit("rebuilt APK already contains duplicate ZIP entries")
        candidates = sorted(
            name for name in source_names
            if name.startswith("res/") and not name.endswith("/")
            and name not in rebuilt_names
        )
        for name in candidates:
            dst.writestr(name, src.read(name), compress_type=zipfile.ZIP_DEFLATED)
    with zipfile.ZipFile(original, "r") as src, zipfile.ZipFile(rebuilt, "r") as dst:
        restored_names = set(dst.namelist())
        if REQUIRED not in restored_names:
            raise SystemExit(f"failed to restore required resource: {REQUIRED}")
        for name in candidates:
            if src.read(name) != dst.read(name):
                raise SystemExit(f"restored resource differs from original: {name}")
    print(f"opaque original resources restored: {len(candidates)}")
    print(f"requiredStartupArtwork={REQUIRED}")
    return len(candidates)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("original_apk", type=Path)
    parser.add_argument("rebuilt_apk", type=Path)
    args = parser.parse_args()
    restore(args.original_apk, args.rebuilt_apk)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
