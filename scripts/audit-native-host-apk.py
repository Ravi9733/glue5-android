#!/usr/bin/env python3
"""Read-only APK inventory for establishing an exact Android native-host contract.

This does not decompile DEX, decode AndroidManifest.xml, or prove runtime
compatibility. It produces a deterministic evidence bundle without extracting
or modifying the supplied APK.
"""
import hashlib
import json
import re
import sys
import zipfile
from pathlib import Path

HOST_TOKENS = (
    b"GTAVActivity", b"SDLActivity", b"loadLibraries", b"nativeRunMain",
    b"GameSurface", b"surfaceCreated", b"surfaceChanged", b"onResume",
    b"onPause", b"onDestroy", b"XhynV02Native",
)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def elf_info(data: bytes) -> dict:
    if len(data) < 20 or data[:4] != b"\x7fELF":
        return {"valid_elf": False}
    elf_class = data[4]
    endian = "<" if data[5] == 1 else ">" if data[5] == 2 else None
    if endian is None:
        return {"valid_elf": False, "reason": "unknown ELF byte order"}
    machine = int.from_bytes(data[18:20], "little" if endian == "<" else "big")
    return {
        "valid_elf": True,
        "class": {1: "ELF32", 2: "ELF64"}.get(elf_class, f"unknown({elf_class})"),
        "machine": machine,
        "aarch64": elf_class == 2 and machine == 183,
    }


def file_digest(path: Path) -> tuple[int, str]:
    digest = hashlib.sha256()
    size = 0
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            size += len(chunk)
            digest.update(chunk)
    return size, digest.hexdigest()


def member_digest(archive: zipfile.ZipFile, name: str, scan_tokens: bool = False) -> dict:
    digest = hashlib.sha256()
    size = 0
    prefix = b""
    found = set()
    overlap = b""
    with archive.open(name) as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            if len(prefix) < 20:
                prefix += chunk[:20 - len(prefix)]
            size += len(chunk)
            digest.update(chunk)
            if scan_tokens:
                window = overlap + chunk
                found.update(token.decode("ascii") for token in HOST_TOKENS if token in window)
                overlap = window[-max(map(len, HOST_TOKENS)):]
    return {"size_bytes": size, "sha256": digest.hexdigest(), "prefix": prefix,
            "host_strings_found": found}


def inspect(apk: Path) -> dict:
    size, apk_hash = file_digest(apk)
    result = {
        "file": apk.name,
        "size_bytes": size,
        "sha256": apk_hash,
        "manifest": None,
        "dex_files": [],
        "arm64_libraries": [],
        "host_strings_found": [],
        "limitations": [
            "Binary AndroidManifest.xml is inventoried but not decoded.",
            "DEX scanning reports literal strings only; it is not decompilation.",
            "Static APK inventory does not establish device launch or gameplay compatibility.",
        ],
    }
    with zipfile.ZipFile(apk) as archive:
        bad = archive.testzip()
        if bad:
            raise ValueError(f"corrupt ZIP member: {bad}")
        names = archive.namelist()
        if len(names) > 100000:
            raise ValueError("APK has an unreasonable number of ZIP entries")
        if "AndroidManifest.xml" in names:
            info = member_digest(archive, "AndroidManifest.xml")
            result["manifest"] = {"size_bytes": info["size_bytes"], "sha256": info["sha256"]}
        tokens = set()
        for name in names:
            if re.fullmatch(r"classes(?:[0-9]+)?\.dex", Path(name).name):
                info = member_digest(archive, name, scan_tokens=True)
                result["dex_files"].append({"path": name, "size_bytes": info["size_bytes"], "sha256": info["sha256"]})
                tokens.update(info["host_strings_found"])
            if name.startswith("lib/arm64-v8a/") and name.endswith(".so"):
                info = member_digest(archive, name)
                result["arm64_libraries"].append({
                    "path": name,
                    "size_bytes": info["size_bytes"],
                    "sha256": info["sha256"],
                    **elf_info(info["prefix"]),
                })
        result["host_strings_found"] = sorted(tokens)
    result["arm64_libraries"].sort(key=lambda item: item["path"])
    result["dex_files"].sort(key=lambda item: item["path"])
    return result


def self_test() -> None:
    import tempfile
    with tempfile.TemporaryDirectory() as temp:
        apk = Path(temp) / "fixture.apk"
        with zipfile.ZipFile(apk, "w") as archive:
            archive.writestr("AndroidManifest.xml", b"manifest-fixture")
            archive.writestr("classes.dex", b"fixture GTAVActivity SDLActivity nativeRunMain")
            archive.writestr("lib/arm64-v8a/libgtav.so", b"not-a-real-elf")
        result = inspect(apk)
        assert result["manifest"]["size_bytes"] > 0
        assert "GTAVActivity" in result["host_strings_found"]
        assert len(result["arm64_libraries"]) == 1
        assert result["arm64_libraries"][0]["valid_elf"] is False
    print("Read-only native host APK inventory self-test: OK")


if __name__ == "__main__":
    if sys.argv[1:] == ["--self-test"]:
        self_test()
    elif len(sys.argv) == 2:
        print(json.dumps(inspect(Path(sys.argv[1])), indent=2))
    else:
        raise SystemExit("usage: audit-native-host-apk.py <exact-preview2.apk> | --self-test")
