#!/usr/bin/env python3
"""Summarize the decoded Android manifest without dumping resource contents."""
from __future__ import annotations
import json
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

ANDROID = "{http://schemas.android.com/apk/res/android}"

def main() -> int:
    if len(sys.argv) != 2:
        raise SystemExit("usage: summarize-xhyn-manifest.py <jadx-output-directory>")
    root_dir = Path(sys.argv[1])
    candidates = list(root_dir.rglob("AndroidManifest.xml"))
    if not candidates:
        raise SystemExit("JADX output contains no decoded AndroidManifest.xml")
    path = candidates[0]
    root = ET.parse(path).getroot()
    app = root.find("application")
    components = []
    if app is not None:
        for node in list(app):
            if node.tag not in {"activity", "activity-alias", "service", "receiver", "provider"}:
                continue
            item = {
                "type": node.tag,
                "name": node.get(ANDROID + "name"),
                "exported": node.get(ANDROID + "exported"),
                "authorities": node.get(ANDROID + "authorities"),
                "process": node.get(ANDROID + "process"),
                "permission": node.get(ANDROID + "permission"),
                "intent_filters": [],
            }
            for intent in node.findall("intent-filter"):
                item["intent_filters"].append({
                    "actions": [x.get(ANDROID + "name") for x in intent.findall("action")],
                    "categories": [x.get(ANDROID + "name") for x in intent.findall("category")],
                })
            components.append(item)
    report = {
        "manifest_source": str(path),
        "package": root.get("package"),
        "version_code": root.get(ANDROID + "versionCode"),
        "version_name": root.get(ANDROID + "versionName"),
        "min_sdk": (root.find("uses-sdk").get(ANDROID + "minSdkVersion")
                    if root.find("uses-sdk") is not None else None),
        "target_sdk": (root.find("uses-sdk").get(ANDROID + "targetSdkVersion")
                       if root.find("uses-sdk") is not None else None),
        "application": ({
            "name": app.get(ANDROID + "name"),
            "label": app.get(ANDROID + "label"),
            "allow_backup": app.get(ANDROID + "allowBackup"),
            "extract_native_libs": app.get(ANDROID + "extractNativeLibs"),
            "uses_cleartext_traffic": app.get(ANDROID + "usesCleartextTraffic"),
        } if app is not None else None),
        "components": components,
    }
    print(json.dumps(report, indent=2))
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
