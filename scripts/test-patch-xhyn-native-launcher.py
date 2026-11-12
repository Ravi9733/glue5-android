#!/usr/bin/env python3
"""Synthetic regression test for the XHYN-to-Glue 5 Native APK patcher."""
from __future__ import annotations

import subprocess
import sys
import tempfile
import xml.etree.ElementTree as ET
from pathlib import Path

ANDROID = "{http://schemas.android.com/apk/res/android}"
PATCHER = Path(__file__).with_name("patch-xhyn-native-launcher.py")


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="xhyn-patcher-test-") as temp:
        root = Path(temp)
        smali = root / "smali"
        smali.mkdir()
        manifest = ET.Element("manifest", {
            "package": "com.xhynph.xhynv.reborn174",
            "xmlns:android": "http://schemas.android.com/apk/res/android",
        })
        app = ET.SubElement(manifest, "application")
        storage = ET.SubElement(app, "activity", {
            ANDROID + "name": "com.gtavsource.android.StoragePermissionActivity",
            ANDROID + "exported": "true",
        })
        intent = ET.SubElement(storage, "intent-filter")
        ET.SubElement(intent, "action", {ANDROID + "name": "android.intent.action.MAIN"})
        ET.SubElement(intent, "category", {ANDROID + "name": "android.intent.category.LAUNCHER"})
        ET.SubElement(app, "activity", {ANDROID + "name": "com.gtavsource.android.GTAVActivity"})
        ET.SubElement(app, "provider", {
            ANDROID + "name": "com.gtavsource.android.ModSpawnProvider",
            ANDROID + "authorities": "com.xhynph.xhynv.reborn174.modspawn",
        })
        ET.ElementTree(manifest).write(root / "AndroidManifest.xml", encoding="utf-8", xml_declaration=True)

        (smali / "com" / "gtavsource" / "android").mkdir(parents=True)
        base = smali / "com" / "gtavsource" / "android"
        (base / "ModSpawnProvider.smali").write_text(
            'const-string v0, "com.xhynph.xhynv.reborn174.modspawn"\n', encoding="utf-8"
        )
        (base / "Other.smali").write_text(
            'const-string v0, "com.xhynph.xhynv.reborn174"\n', encoding="utf-8"
        )
        (base / "GameRoot.smali").write_text(
            'const-string v0, "/storage/emulated/0/Games/GTAV"\n', encoding="utf-8"
        )

        subprocess.run([sys.executable, str(PATCHER), str(root)], check=True, capture_output=True, text=True)

        patched = ET.parse(root / "AndroidManifest.xml").getroot()
        assert patched.get("package") == "local.glue5.nativeapp"
        application = patched.find("application")
        assert application is not None
        launchers = [
            activity for activity in application.findall("activity")
            if activity.get(ANDROID + "name") == "local.glue5.nativeapp.LauncherActivity"
        ]
        assert len(launchers) == 1
        assert launchers[0].get(ANDROID + "exported") == "true"
        storage = next(a for a in application.findall("activity")
                       if a.get(ANDROID + "name") == "com.gtavsource.android.StoragePermissionActivity")
        assert storage.get(ANDROID + "exported") == "false"
        assert not storage.findall("intent-filter")
        provider = application.find("provider")
        assert provider is not None
        assert provider.get(ANDROID + "authorities") == "local.glue5.nativeapp.modspawn"

        all_smali = "\n".join(p.read_text(encoding="utf-8") for p in smali.rglob("*.smali"))
        assert "com.xhynph.xhynv.reborn174" not in all_smali
        assert "local.glue5.nativeapp.modspawn" in all_smali
        assert "local/glue5/nativeapp/LauncherActivity" in all_smali
        assert "START GAME" in (smali / "local/glue5/nativeapp/LauncherActivity.smali").read_text()
        assert "/storage/emulated/0/Games/GTAV" in (base / "GameRoot.smali").read_text()
    print("XHYN native launcher patcher regression test: PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
