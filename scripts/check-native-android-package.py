#!/usr/bin/env python3
"""Fail-closed preflight for a separately packaged native Android staging tree.

Checks structure and ELF ABI only. It does not prove engine compatibility,
JNI correctness, signing, launch, or gameplay.
"""
import json
import re
import struct
import sys
import tempfile
import xml.etree.ElementTree as ET
from pathlib import Path

REQUIRED_FIELDS = {"schema_version", "application_id", "activity", "abi", "libraries"}
PACKAGE_ID = re.compile(r"^[A-Za-z][A-Za-z0-9_]*(?:\\.[A-Za-z][A-Za-z0-9_]*)+$")
ACTIVITY = re.compile(r"^[A-Za-z_$][A-Za-z0-9_$]*(?:\\.[A-Za-z_$][A-Za-z0-9_$]*)+$")
FORBIDDEN_NAMES = {"game.wasm", "libglue5.so", "libSDL3.so"}
ANDROID = "{http://schemas.android.com/apk/res/android}"


def elf_problem(path: Path) -> str | None:
    """Return an error unless path is a 64-bit little-endian AArch64 ELF."""
    try:
        with path.open("rb") as stream:
            header = stream.read(64)
    except OSError as exc:
        return f"cannot read native library {path.name}: {exc}"
    if len(header) < 20 or header[:4] != b"\\x7fELF":
        return f"native library is not ELF: {path.name}"
    if header[4] != 2:
        return f"native library is not ELF64: {path.name}"
    if header[5] != 1:
        return f"native library is not little-endian ELF: {path.name}"
    machine = struct.unpack_from("<H", header, 18)[0]
    if machine != 183:
        return f"native library is not AArch64 (e_machine={machine}): {path.name}"
    return None


def validate(root: Path) -> list[str]:
    errors = []
    manifest_path = root / "native-package.json"
    if not manifest_path.is_file():
        return ["missing native-package.json"]
    try:
        manifest = json.loads(manifest_path.read_text())
    except (OSError, json.JSONDecodeError) as exc:
        return [f"invalid native-package.json: {exc}"]
    if not isinstance(manifest, dict):
        return ["native-package.json must contain a JSON object"]

    missing = REQUIRED_FIELDS - manifest.keys()
    if missing:
        return ["missing manifest fields: " + ", ".join(sorted(missing))]
    if manifest["schema_version"] != 1:
        errors.append("schema_version must be 1")
    package_id = manifest["application_id"]
    if not isinstance(package_id, str) or not PACKAGE_ID.fullmatch(package_id) or package_id == "local.glue5":
        errors.append("application_id must be a valid dotted ID distinct from local.glue5")
    activity = manifest["activity"]
    if not isinstance(activity, str) or not ACTIVITY.fullmatch(activity):
        errors.append("activity must be a fully qualified verified host Activity")
    if manifest["abi"] != "arm64-v8a":
        errors.append("abi must be arm64-v8a")

    android_manifest = root / "AndroidManifest.xml"
    if not android_manifest.is_file():
        errors.append("missing staged AndroidManifest.xml")
    else:
        try:
            xml = ET.parse(android_manifest).getroot()
            if xml.tag != "manifest":
                errors.append("AndroidManifest.xml root element must be <manifest>")
            if xml.attrib.get("package") != package_id:
                errors.append("AndroidManifest.xml package does not match application_id")
            app = xml.find("application")
            declared = set()
            if app is not None:
                declared = {node.attrib.get(ANDROID + "name") for node in app.findall("activity")}
            if activity not in declared:
                errors.append("AndroidManifest.xml does not declare the configured fully qualified Activity")
        except (ET.ParseError, OSError) as exc:
            errors.append(f"invalid AndroidManifest.xml: {exc}")

    libraries = manifest["libraries"]
    if not isinstance(libraries, list) or not libraries:
        errors.append("libraries must be a non-empty list containing libgtav.so")
    elif any(not isinstance(name, str) or Path(name).name != name or not name.endswith(".so")
             for name in libraries):
        errors.append("libraries must contain only plain .so filenames")
    elif len(libraries) != len(set(libraries)):
        errors.append("libraries must contain unique filenames")
    else:
        if "libgtav.so" not in libraries:
            errors.append("libraries must include libgtav.so")
        for name in libraries:
            path = root / name
            if not path.is_file():
                errors.append(f"declared native library missing: {name}")
            else:
                problem = elf_problem(path)
                if problem:
                    errors.append(problem)

    # Search the entire staging tree so nested payloads cannot reintroduce WASM.
    for path in root.rglob("*"):
        if path.is_symlink():
            errors.append(f"symlinks are not allowed in native staging: {path.relative_to(root)}")
            continue
        if not path.is_file():
            continue
        if path.name in FORBIDDEN_NAMES or path.suffix.lower() == ".wasm":
            errors.append(f"forbidden WASM/Glue5 payload present: {path.relative_to(root)}")
    return errors


def write_arm64_elf(path: Path) -> None:
    header = bytearray(64)
    header[:7] = b"\\x7fELF\\x02\\x01\\x01"
    struct.pack_into("<H", header, 18, 183)  # EM_AARCH64
    path.write_bytes(header)


def self_test() -> None:
    with tempfile.TemporaryDirectory() as temp:
        root = Path(temp)
        write_arm64_elf(root / "libgtav.so")
        (root / "AndroidManifest.xml").write_text(
            '<manifest xmlns:android="http://schemas.android.com/apk/res/android" '
            'package="local.glue5.native"><application>'
            '<activity android:name="com.example.verified.GTAVActivity"/>'
            '</application></manifest>'
        )
        manifest = {
            "schema_version": 1,
            "application_id": "local.glue5.native",
            "activity": "com.example.verified.GTAVActivity",
            "abi": "arm64-v8a",
            "libraries": ["libgtav.so"],
        }
        (root / "native-package.json").write_text(json.dumps(manifest))
        assert validate(root) == [], validate(root)

        nested = root / "assets" / "legacy"
        nested.mkdir(parents=True)
        (nested / "game.wasm").write_bytes(b"forbidden")
        assert any("assets/legacy/game.wasm" in error for error in validate(root))
        (nested / "game.wasm").unlink()

        manifest["application_id"] = "local.glue5"
        (root / "native-package.json").write_text(json.dumps(manifest))
        assert any("distinct from local.glue5" in error for error in validate(root))

        manifest["application_id"] = "local.glue5.native"
        manifest["libraries"] = [{"unexpected": "object"}]
        (root / "native-package.json").write_text(json.dumps(manifest))
        assert any("plain .so filenames" in error for error in validate(root))

        manifest["libraries"] = ["libgtav.so"]
        write_arm64_elf(root / "libgtav.so")
        (root / "libgtav.so").write_bytes(b"not an ELF")
        (root / "native-package.json").write_text(json.dumps(manifest))
        assert any("not ELF" in error for error in validate(root))

        (root / "libgtav.so").unlink()
        (root / "libgtav.so").write_bytes(b"\\x7fELF\\x02\\x01\\x01" + bytes(11) + struct.pack("<H", 62) + bytes(44))
        assert any("not AArch64" in error for error in validate(root))

        (root / "libgtav.so").unlink()
        write_arm64_elf(root / "libgtav.so")
        (root / "AndroidManifest.xml").write_text("<manifest><application/></manifest>")
        assert any("does not match application_id" in error for error in validate(root))

    print("Native Android packaging preflight self-test: OK")


if __name__ == "__main__":
    if sys.argv[1:] == ["--self-test"]:
        self_test()
    elif len(sys.argv) == 2:
        failures = validate(Path(sys.argv[1]))
        if failures:
            print("\\n".join(f"BLOCKED: {failure}" for failure in failures), file=sys.stderr)
            raise SystemExit(1)
        print("Native Android packaging inputs: structurally valid")
    else:
        raise SystemExit("usage: check-native-android-package.py <staging-dir> | --self-test")
