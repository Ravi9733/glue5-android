#!/usr/bin/env python3
"""Regression test for restoring the original compiled resource table and res tree."""
from __future__ import annotations

import importlib.util
import tempfile
import zipfile
from pathlib import Path

MODULE = Path(__file__).with_name("restore-opaque-apk-resources.py")
SPEC = importlib.util.spec_from_file_location("restore_opaque", MODULE)
assert SPEC and SPEC.loader
MOD = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MOD)


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="opaque-res-test-") as tmp:
        root = Path(tmp)
        original, rebuilt = root / "original.apk", root / "rebuilt.apk"
        payloads = {
            "resources.arsc": b"compiled-resource-table-original",
            "res/drawable/titanium_full_fire.bin": b"startup-artwork",
            "res/drawable/other_art.bin": b"opaque-artwork",
            "res/drawable/existing.png": b"original-png",
        }
        with zipfile.ZipFile(original, "w") as z:
            for name, data in payloads.items():
                z.writestr(name, data)
        with zipfile.ZipFile(rebuilt, "w") as z:
            z.writestr("resources.arsc", b"apktool-recompiled-resource-table")
            z.writestr("res/drawable/existing.png", b"apktool-recompiled-png")
            z.writestr("classes.dex", b"patched-dex")
            z.writestr("AndroidManifest.xml", b"rewritten-manifest")

        count = MOD.restore(original, rebuilt)
        assert count == 3
        with zipfile.ZipFile(rebuilt) as z:
            assert z.read("resources.arsc") == payloads["resources.arsc"]
            for name, data in payloads.items():
                assert z.read(name) == data
            assert z.read("classes.dex") == b"patched-dex"
            assert z.read("AndroidManifest.xml") == b"rewritten-manifest"
            assert len(z.namelist()) == len(set(z.namelist()))
    print("compiled resource table + res tree restoration test: PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
