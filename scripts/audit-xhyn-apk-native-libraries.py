#!/usr/bin/env python3
"""Inventory XHYN Preview 2 ARM64 libraries without publishing binary contents."""
from __future__ import annotations
import argparse
import hashlib
import json
import shutil
import subprocess
import tempfile
import zipfile
from pathlib import Path

EXPECTED_APK_SHA256 = "a681765d9aae1404c59e3570682f3078bf9062ff800abfc7c5e1972f2783309f"
HELPER_LIBRARIES = {"libxhyn_v02.so"}
EXPECTED_ENGINE_LIBRARY_COUNT = 26

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("apk", type=Path)
    parser.add_argument("--expected-sha256", default=EXPECTED_APK_SHA256)
    args = parser.parse_args()
    if not args.apk.is_file():
        parser.error("APK does not exist")
    digest = hashlib.sha256()
    with args.apk.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    apk_hash = digest.hexdigest()
    if args.expected_sha256 and apk_hash != args.expected_sha256:
        raise SystemExit(f"APK hash mismatch: {apk_hash}")
    readelf = shutil.which("readelf") or shutil.which("llvm-readelf")
    if not readelf:
        raise SystemExit("readelf is required")
    with zipfile.ZipFile(args.apk) as archive, tempfile.TemporaryDirectory() as temp:
        names = sorted(n for n in archive.namelist()
                       if n.startswith("lib/arm64-v8a/") and n.endswith(".so"))
        items = []
        for name in names:
            target = Path(temp) / Path(name).name
            h = hashlib.sha256()
            with archive.open(name) as src, target.open("wb") as dst:
                for block in iter(lambda: src.read(1024 * 1024), b""):
                    h.update(block)
                    dst.write(block)
            header = subprocess.run([readelf, "-h", str(target)], capture_output=True, text=True, check=True).stdout
            dynamic = subprocess.run([readelf, "-d", str(target)], capture_output=True, text=True, check=True).stdout
            machine = next((line.split(":", 1)[1].strip() for line in header.splitlines() if "Machine:" in line), "unknown")
            elf_class = next((line.split(":", 1)[1].strip() for line in header.splitlines() if "Class:" in line), "unknown")
            endian = next((line.split(":", 1)[1].strip() for line in header.splitlines() if "Data:" in line), "unknown")
            needed = []
            for line in dynamic.splitlines():
                if "(NEEDED)" in line:
                    needed.append(line.split("[", 1)[-1].split("]", 1)[0])
            items.append({"file": Path(name).name, "sha256": h.hexdigest(),
                          "elf_class": elf_class, "endianness": endian,
                          "machine": machine, "needed": needed})
    bad = [x["file"] for x in items if x["elf_class"] != "ELF64" or
           "little endian" not in x["endianness"].lower() or "aarch64" not in x["machine"].lower()]
    engine_names = {x["file"] for x in items} - HELPER_LIBRARIES
    missing_helpers = HELPER_LIBRARIES - {x["file"] for x in items}
    count_ok = len(engine_names) == EXPECTED_ENGINE_LIBRARY_COUNT and not missing_helpers
    report = {
        "apk_sha256": apk_hash,
        "arm64_library_count_total_including_xhyn_helper": len(items),
        "expected_original_engine_library_count": EXPECTED_ENGINE_LIBRARY_COUNT,
        "observed_engine_library_count_excluding_helper": len(engine_names),
        "xhyn_helper_libraries": sorted(HELPER_LIBRARIES & {x["file"] for x in items}),
        "java_getLibraries_order": ["SDL3", "gtav"],
        "all_libraries_are_aarch64_elf64_little_endian": not bad,
        "engine_library_count_matches_and_helper_present": count_ok,
        "invalid_libraries": bad,
        "missing_helpers": sorted(missing_helpers),
        "libraries": items,
    }
    print(json.dumps(report, indent=2))
    return 1 if bad or not count_ok else 0

if __name__ == "__main__":
    raise SystemExit(main())
