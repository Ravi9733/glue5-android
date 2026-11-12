#!/usr/bin/env python3
"""Regression test for restoring opaque res/ entries after Apktool rebuild."""
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
            "res/drawable/titanium_full_fire.bin": b"startup-artwork",
            "res/drawable/other_art.bin": b"opaque-artwork",
            "res/drawable/existing.png": b"original-png",
        }
        with zipfile.ZipFile(original, "w") as z:
            for name, data in payloads.items():
                z.writestr(name, data)
        with zipfile.ZipFile(rebuilt, "w") as z:
            z.writestr("res/drawable/existing.png", b"rebuilt-png")
            z.writestr("classes.dex", b"fake-dex")
        count = MOD.restore(original, rebuilt)
        assert count == 2
        with zipfile.ZipFile(rebuilt) as z:
            assert z.read("res/drawable/titanium_full_fire.bin") == payloads["res/drawable/titanium_full_fire.bin"]
            assert z.read("res/drawable/other_art.bin") == payloads["res/drawable/other_art.bin"]
            assert z.read("res/drawable/existing.png") == b"rebuilt-png"
            assert z.read("classes.dex") == b"fake-dex"
    print("opaque APK resource restoration test: PASS")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
