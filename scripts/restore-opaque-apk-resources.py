#!/usr/bin/env python3
"""Restore the original compiled Android resource table and res/ tree after Apktool rebuild."""
from __future__ import annotations

import argparse
import os
import tempfile
import zipfile
from pathlib import Path

REQUIRED = "res/drawable/titanium_full_fire.bin"
RESOURCE_TABLE = "resources.arsc"


def restore(original: Path, rebuilt: Path) -> int:
    temp_path: Path | None = None
    with zipfile.ZipFile(original, "r") as src, zipfile.ZipFile(rebuilt, "r") as old:
        src_names = src.namelist()
        old_names = old.namelist()
        if len(src_names) != len(set(src_names)):
            raise SystemExit("original APK contains duplicate ZIP entries")
        if len(old_names) != len(set(old_names)):
            raise SystemExit("rebuilt APK contains duplicate ZIP entries")
        if REQUIRED not in src_names:
            raise SystemExit(f"required original startup artwork missing: {REQUIRED}")
        if RESOURCE_TABLE not in src_names:
            raise SystemExit("original APK has no compiled resources.arsc")

        # Keep Apktool's rewritten manifest and DEX/native payload, but replace
        # its recompiled resource table/tree with the exact original compiled data.
        fd, tmp = tempfile.mkstemp(prefix="apk-resource-restore-", suffix=".apk", dir=rebuilt.parent)
        os.close(fd)
        temp_path = Path(tmp)
        replaced = {RESOURCE_TABLE} | {
            name for name in src_names if name.startswith("res/") and not name.endswith("/")
        }
        with zipfile.ZipFile(temp_path, "w") as out:
            for info in old.infolist():
                if info.filename in replaced or info.is_dir():
                    continue
                out.writestr(info, old.read(info.filename))
            for info in src.infolist():
                if info.filename == RESOURCE_TABLE or (info.filename.startswith("res/") and not info.is_dir()):
                    out.writestr(info, src.read(info.filename))

    try:
        with zipfile.ZipFile(original, "r") as src, zipfile.ZipFile(temp_path, "r") as out:
            out_names = out.namelist()
            if len(out_names) != len(set(out_names)):
                raise SystemExit("restored APK contains duplicate ZIP entries")
            if src.read(RESOURCE_TABLE) != out.read(RESOURCE_TABLE):
                raise SystemExit("compiled resources.arsc differs from original")
            if src.read(REQUIRED) != out.read(REQUIRED):
                raise SystemExit(f"required startup artwork differs: {REQUIRED}")
            original_res = {
                n: src.read(n) for n in src.namelist()
                if n.startswith("res/") and not n.endswith("/")
            }
            restored_res = {
                n: out.read(n) for n in out.namelist()
                if n.startswith("res/") and not n.endswith("/")
            }
            if original_res != restored_res:
                raise SystemExit("restored res/ tree is not byte-identical to original")
        os.replace(temp_path, rebuilt)
    finally:
        if temp_path.exists():
            temp_path.unlink()

    print(f"original compiled resource table restored byte-for-byte: {RESOURCE_TABLE}")
    print(f"original res/ files restored byte-for-byte: {len(original_res)}")
    print(f"requiredStartupArtwork={REQUIRED}")
    return len(original_res)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("original_apk", type=Path)
    parser.add_argument("rebuilt_apk", type=Path)
    args = parser.parse_args()
    restore(args.original_apk, args.rebuilt_apk)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
