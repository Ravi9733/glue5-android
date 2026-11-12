#!/usr/bin/env python3
"""Fail-closed preflight for a future, separately packaged native Android APK.

This validates packaging inputs only; it does not prove engine compatibility or
replace the real-device release gates documented in native-android-packaging-plan.md.
"""
import json
import sys
import tempfile
from pathlib import Path

REQUIRED_FIELDS = {"schema_version", "application_id", "activity", "abi", "libraries"}
FORBIDDEN_NAMES = {"game.wasm", "libglue5.so", "libSDL3.so"}


def validate(root: Path) -> list[str]:
    errors = []
    manifest_path = root / "native-package.json"
    if not manifest_path.is_file():
        return ["missing native-package.json"]
    try:
        manifest = json.loads(manifest_path.read_text())
    except (OSError, json.JSONDecodeError) as exc:
        return [f"invalid native-package.json: {exc}"]

    missing = REQUIRED_FIELDS - manifest.keys()
    if missing:
        errors.append("missing manifest fields: " + ", ".join(sorted(missing)))
        return errors
    if manifest["schema_version"] != 1:
        errors.append("schema_version must be 1")
    package_id = manifest["application_id"]
    if not isinstance(package_id, str) or not package_id or package_id == "local.glue5":
        errors.append("application_id must be a non-empty ID distinct from local.glue5")
    if not isinstance(manifest["activity"], str) or not manifest["activity"]:
        errors.append("activity must name the verified native host Activity")
    if manifest["abi"] != "arm64-v8a":
        errors.append("abi must be arm64-v8a")
    libraries = manifest["libraries"]
    if not isinstance(libraries, list) or not libraries or "libgtav.so" not in libraries:
        errors.append("libraries must be a non-empty list containing libgtav.so")
    elif len(libraries) != len(set(libraries)) or any(
        not isinstance(name, str) or Path(name).name != name or not name.endswith(".so")
        for name in libraries
    ):
        errors.append("libraries must contain unique plain .so filenames")

    for name in FORBIDDEN_NAMES:
        if (root / name).exists():
            errors.append(f"forbidden WASM/Glue5 library present: {name}")
    for name in libraries if isinstance(libraries, list) else []:
        if isinstance(name, str) and not (root / name).is_file():
            errors.append(f"declared native library missing: {name}")
    return errors


def self_test() -> None:
    with tempfile.TemporaryDirectory() as temp:
        root = Path(temp)
        (root / "libgtav.so").write_bytes(b"test-only-placeholder")
        manifest = {
            "schema_version": 1,
            "application_id": "local.glue5.native",
            "activity": "com.example.verified.GTAVActivity",
            "abi": "arm64-v8a",
            "libraries": ["libgtav.so"],
        }
        (root / "native-package.json").write_text(json.dumps(manifest))
        assert validate(root) == [], validate(root)
        (root / "game.wasm").write_bytes(b"forbidden")
        assert any("game.wasm" in error for error in validate(root))
        (root / "game.wasm").unlink()
        manifest["application_id"] = "local.glue5"
        (root / "native-package.json").write_text(json.dumps(manifest))
        assert any("distinct from local.glue5" in error for error in validate(root))
    print("Native Android packaging preflight self-test: OK")


if __name__ == "__main__":
    if sys.argv[1:] == ["--self-test"]:
        self_test()
    elif len(sys.argv) == 2:
        failures = validate(Path(sys.argv[1]))
        if failures:
            print("\n".join(f"BLOCKED: {failure}" for failure in failures), file=sys.stderr)
            raise SystemExit(1)
        print("Native Android packaging inputs: structurally valid")
    else:
        raise SystemExit("usage: check-native-android-package.py <staging-dir> | --self-test")
