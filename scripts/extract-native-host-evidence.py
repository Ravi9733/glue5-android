#!/usr/bin/env python3
"""Extract host evidence from an authorized Android APK using installed Android tools.

This helper never patches or repackages the APK. It writes decoded evidence to
an output directory and reports unavailable tools rather than guessing APIs.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import subprocess
import sys
import zipfile
from pathlib import Path


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def run_tool(command: list[str], output: Path) -> dict:
    try:
        result = subprocess.run(command, capture_output=True, text=True, timeout=900, check=False)
        output.write_text(result.stdout + (("\n[stderr]\n" + result.stderr) if result.stderr else ""),
                          encoding="utf-8", errors="replace")
        return {"command": command, "exit_code": result.returncode, "output": str(output)}
    except (OSError, subprocess.TimeoutExpired) as exc:
        return {"command": command, "error": str(exc)}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("apk", nargs="?", type=Path)
    parser.add_argument("--out", type=Path, default=Path("native-host-evidence"))
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        import tempfile
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            apk = root / "fixture.apk"
            with zipfile.ZipFile(apk, "w") as archive:
                archive.writestr("AndroidManifest.xml", "<manifest package='fixture.test'/>")
                archive.writestr("classes.dex", b"dex\\n035\\0")
                archive.writestr("lib/arm64-v8a/libfixture.so", b"fixture")
            with zipfile.ZipFile(apk) as archive:
                names = set(archive.namelist())
            assert {"AndroidManifest.xml", "classes.dex", "lib/arm64-v8a/libfixture.so"} <= names
            assert len(sha256(apk)) == 64
        print("native host evidence extractor self-test: PASS")
        return 0

    if args.apk is None or not args.apk.is_file():
        parser.error("provide an existing authorized APK, or use --self-test")

    args.out.mkdir(parents=True, exist_ok=True)
    report: dict = {
        "apk": str(args.apk.resolve()),
        "sha256": sha256(args.apk),
        "tools": {},
        "notes": [
            "This is static evidence collection only; it does not prove gameplay compatibility.",
            "Review all output manually before implementing an engine adapter.",
        ],
    }

    with zipfile.ZipFile(args.apk) as archive:
        names = archive.namelist()
        report["apk_entries"] = len(names)
        report["dex_files"] = sorted(n for n in names if n.endswith(".dex"))
        report["arm64_libraries"] = sorted(
            n for n in names if n.startswith("lib/arm64-v8a/") and n.endswith(".so")
        )
        manifest = args.out / "manifest-binary.xml"
        if "AndroidManifest.xml" in names:
            manifest.write_bytes(archive.read("AndroidManifest.xml"))
            report["binary_manifest"] = str(manifest)

        lib_dir = args.out / "arm64-libs"
        lib_dir.mkdir(exist_ok=True)
        for name in report["arm64_libraries"]:
            target = lib_dir / Path(name).name
            with archive.open(name) as src, target.open("wb") as dst:
                shutil.copyfileobj(src, dst)
        report["extracted_arm64_libraries"] = str(lib_dir)

    apkanalyzer = shutil.which("apkanalyzer")
    aapt = shutil.which("aapt") or shutil.which("aapt2")
    jadx = shutil.which("jadx")
    readelf = shutil.which("llvm-readelf") or shutil.which("readelf")
    nm = shutil.which("llvm-nm") or shutil.which("nm")

    if apkanalyzer:
        report["tools"]["manifest"] = run_tool(
            [apkanalyzer, "manifest", "print", str(args.apk)], args.out / "manifest-decoded.xml"
        )
        report["tools"]["dex-packages"] = run_tool(
            [apkanalyzer, "dex", "packages", str(args.apk)], args.out / "dex-packages.txt"
        )
    elif aapt:
        report["tools"]["manifest"] = run_tool(
            [aapt, "dump", "xmltree", str(args.apk), "AndroidManifest.xml"],
            args.out / "manifest-decoded.txt",
        )
    else:
        report["tools"]["manifest"] = {"error": "install Android SDK apkanalyzer or aapt"}

    if jadx:
        report["tools"]["jadx"] = run_tool(
            [jadx, "--no-res", "-d", str(args.out / "jadx"), str(args.apk)],
            args.out / "jadx.log",
        )
    else:
        report["tools"]["jadx"] = {"error": "install jadx to decompile DEX for Activity/JNI review"}

    symbols = []
    for library in sorted((args.out / "arm64-libs").glob("*.so")):
        item = {"library": library.name}
        if readelf:
            item["dynamic"] = run_tool(
                [readelf, "-d", str(library)], args.out / (library.name + ".dynamic.txt")
            )
            item["headers"] = run_tool(
                [readelf, "-h", str(library)], args.out / (library.name + ".headers.txt")
            )
        if nm:
            item["exports"] = run_tool(
                [nm, "-D", "--defined-only", str(library)],
                args.out / (library.name + ".exports.txt"),
            )
        symbols.append(item)
    report["library_analysis"] = symbols
    report["available_tools"] = {
        "apkanalyzer": bool(apkanalyzer), "aapt": bool(aapt), "jadx": bool(jadx),
        "readelf": bool(readelf), "nm": bool(nm),
    }

    report_path = args.out / "report.json"
    report_path.write_text(json.dumps(report, indent=2), encoding="utf-8")
    print(f"Evidence written to: {report_path.resolve()}")
    print(f"APK SHA-256: {report['sha256']}")
    print("Static inspection only; no adapter or gameplay compatibility is implied.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
