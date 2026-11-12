#!/usr/bin/env python3
"""Patch a decoded XHYN Preview 2 APK into a separate Glue 5 Native launcher package."""
from __future__ import annotations
import argparse
import xml.etree.ElementTree as ET
from pathlib import Path

ANDROID = "{http://schemas.android.com/apk/res/android}"
OLD_PACKAGE = "com.xhynph.xhynv.reborn174"
NEW_PACKAGE = "local.glue5.nativeapp"
OLD_AUTHORITY = OLD_PACKAGE + ".modspawn"
NEW_AUTHORITY = NEW_PACKAGE + ".modspawn"

LAUNCHER_SMALI = r""".class public Llocal/glue5/nativeapp/LauncherActivity;
.super Landroid/app/Activity;
.source "LauncherActivity.java"

.method public constructor <init>()V
    .locals 0
    invoke-direct {p0}, Landroid/app/Activity;-><init>()V
    return-void
.end method

.method protected onCreate(Landroid/os/Bundle;)V
    .locals 8
    invoke-super {p0, p1}, Landroid/app/Activity;->onCreate(Landroid/os/Bundle;)V

    new-instance v0, Landroid/widget/LinearLayout;
    invoke-direct {v0, p0}, Landroid/widget/LinearLayout;-><init>(Landroid/content/Context;)V
    const/4 v1, 0x1
    invoke-virtual {v0, v1}, Landroid/widget/LinearLayout;->setOrientation(I)V
    const/16 v1, 0x11
    invoke-virtual {v0, v1}, Landroid/widget/LinearLayout;->setGravity(I)V
    const/16 v1, 0x28
    invoke-virtual {v0, v1, v1, v1, v1}, Landroid/view/View;->setPadding(IIII)V
    const v1, -0xf3edf1
    invoke-virtual {v0, v1}, Landroid/view/View;->setBackgroundColor(I)V

    new-instance v2, Landroid/widget/TextView;
    invoke-direct {v2, p0}, Landroid/widget/TextView;-><init>(Landroid/content/Context;)V
    const-string v3, "GLUE 5 NATIVE"
    invoke-virtual {v2, v3}, Landroid/widget/TextView;->setText(Ljava/lang/CharSequence;)V
    const/high16 v3, 0x41c00000
    invoke-virtual {v2, v3}, Landroid/widget/TextView;->setTextSize(F)V
    const v3, -0x7e2986
    invoke-virtual {v2, v3}, Landroid/widget/TextView;->setTextColor(I)V
    const/16 v3, 0x11
    invoke-virtual {v2, v3}, Landroid/widget/TextView;->setGravity(I)V
    const/16 v3, 0x14
    invoke-virtual {v2, v3, v3, v3, v3}, Landroid/widget/TextView;->setPadding(IIII)V
    new-instance v3, Landroid/widget/LinearLayout$LayoutParams;
    const/4 v4, -0x1
    const/4 v5, -0x2
    invoke-direct {v3, v4, v5}, Landroid/widget/LinearLayout$LayoutParams;-><init>(II)V
    invoke-virtual {v0, v2, v3}, Landroid/widget/LinearLayout;->addView(Landroid/view/View;Landroid/view/ViewGroup$LayoutParams;)V

    new-instance v2, Landroid/widget/TextView;
    invoke-direct {v2, p0}, Landroid/widget/TextView;-><init>(Landroid/content/Context;)V
    const-string v3, "SDL3  |  VULKAN  |  ARM64"
    invoke-virtual {v2, v3}, Landroid/widget/TextView;->setText(Ljava/lang/CharSequence;)V
    const/high16 v3, 0x41700000
    invoke-virtual {v2, v3}, Landroid/widget/TextView;->setTextSize(F)V
    const/4 v3, -0x1
    invoke-virtual {v2, v3}, Landroid/widget/TextView;->setTextColor(I)V
    const/16 v3, 0x11
    invoke-virtual {v2, v3}, Landroid/widget/TextView;->setGravity(I)V
    const/16 v3, 0x10
    invoke-virtual {v2, v3, v3, v3, v3}, Landroid/widget/TextView;->setPadding(IIII)V
    new-instance v3, Landroid/widget/LinearLayout$LayoutParams;
    const/4 v4, -0x1
    const/4 v5, -0x2
    invoke-direct {v3, v4, v5}, Landroid/widget/LinearLayout$LayoutParams;-><init>(II)V
    invoke-virtual {v0, v2, v3}, Landroid/widget/LinearLayout;->addView(Landroid/view/View;Landroid/view/ViewGroup$LayoutParams;)V

    new-instance v2, Landroid/widget/Button;
    invoke-direct {v2, p0}, Landroid/widget/Button;-><init>(Landroid/content/Context;)V
    const-string v3, "START GAME"
    invoke-virtual {v2, v3}, Landroid/widget/Button;->setText(Ljava/lang/CharSequence;)V
    const-class v3, Lcom/gtavsource/android/StoragePermissionActivity;
    new-instance v4, Llocal/glue5/nativeapp/LauncherActivity$OpenActivity;
    invoke-direct {v4, p0, v3}, Llocal/glue5/nativeapp/LauncherActivity$OpenActivity;-><init>(Landroid/app/Activity;Ljava/lang/Class;)V
    invoke-virtual {v2, v4}, Landroid/widget/Button;->setOnClickListener(Landroid/view/View$OnClickListener;)V
    new-instance v3, Landroid/widget/LinearLayout$LayoutParams;
    const/4 v4, -0x1
    const/4 v5, -0x2
    invoke-direct {v3, v4, v5}, Landroid/widget/LinearLayout$LayoutParams;-><init>(II)V
    const/16 v4, 0x14
    iput v4, v3, Landroid/widget/LinearLayout$LayoutParams;->topMargin:I
    invoke-virtual {v0, v2, v3}, Landroid/widget/LinearLayout;->addView(Landroid/view/View;Landroid/view/ViewGroup$LayoutParams;)V

    new-instance v2, Landroid/widget/Button;
    invoke-direct {v2, p0}, Landroid/widget/Button;-><init>(Landroid/content/Context;)V
    const-string v3, "GAME SETTINGS"
    invoke-virtual {v2, v3}, Landroid/widget/Button;->setText(Ljava/lang/CharSequence;)V
    const-class v3, Lcom/gtavsource/android/GameMenuActivity;
    new-instance v4, Llocal/glue5/nativeapp/LauncherActivity$OpenActivity;
    invoke-direct {v4, p0, v3}, Llocal/glue5/nativeapp/LauncherActivity$OpenActivity;-><init>(Landroid/app/Activity;Ljava/lang/Class;)V
    invoke-virtual {v2, v4}, Landroid/widget/Button;->setOnClickListener(Landroid/view/View$OnClickListener;)V
    new-instance v3, Landroid/widget/LinearLayout$LayoutParams;
    const/4 v4, -0x1
    const/4 v5, -0x2
    invoke-direct {v3, v4, v5}, Landroid/widget/LinearLayout$LayoutParams;-><init>(II)V
    invoke-virtual {v0, v2, v3}, Landroid/widget/LinearLayout;->addView(Landroid/view/View;Landroid/view/ViewGroup$LayoutParams;)V

    invoke-virtual {p0, v0}, Landroid/app/Activity;->setContentView(Landroid/view/View;)V
    return-void
.end method
"""

LISTENER_SMALI = r""".class public final Llocal/glue5/nativeapp/LauncherActivity$OpenActivity;
.super Ljava/lang/Object;
.implements Landroid/view/View$OnClickListener;

.field private final activity:Landroid/app/Activity;
.field private final target:Ljava/lang/Class;

.method public constructor <init>(Landroid/app/Activity;Ljava/lang/Class;)V
    .locals 0
    invoke-direct {p0}, Ljava/lang/Object;-><init>()V
    iput-object p1, p0, Llocal/glue5/nativeapp/LauncherActivity$OpenActivity;->activity:Landroid/app/Activity;
    iput-object p2, p0, Llocal/glue5/nativeapp/LauncherActivity$OpenActivity;->target:Ljava/lang/Class;
    return-void
.end method

.method public onClick(Landroid/view/View;)V
    .locals 3
    iget-object v0, p0, Llocal/glue5/nativeapp/LauncherActivity$OpenActivity;->activity:Landroid/app/Activity;
    iget-object v1, p0, Llocal/glue5/nativeapp/LauncherActivity$OpenActivity;->target:Ljava/lang/Class;
    new-instance v2, Landroid/content/Intent;
    invoke-direct {v2, v0, v1}, Landroid/content/Intent;-><init>(Landroid/content/Context;Ljava/lang/Class;)V
    invoke-virtual {v0, v2}, Landroid/app/Activity;->startActivity(Landroid/content/Intent;)V
    return-void
.end method
"""

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("decoded_apk", type=Path)
    args = parser.parse_args()
    root = args.decoded_apk
    manifest_path = root / "AndroidManifest.xml"
    if not manifest_path.is_file():
        parser.error("decoded APK has no AndroidManifest.xml")
    tree = ET.parse(manifest_path)
    manifest = tree.getroot()
    if manifest.get("package") != OLD_PACKAGE:
        raise SystemExit(f"unexpected package: {manifest.get('package')!r}")
    ET.register_namespace("android", "http://schemas.android.com/apk/res/android")
    manifest.set("package", NEW_PACKAGE)
    app = manifest.find("application")
    if app is None:
        raise SystemExit("manifest application element missing")
    app.set(ANDROID + "label", "Glue 5 Native")

    found_launcher = False
    for activity in app.findall("activity"):
        name = activity.get(ANDROID + "name", "")
        if name == "com.gtavsource.android.StoragePermissionActivity":
            activity.set(ANDROID + "exported", "false")
            for intent_filter in list(activity.findall("intent-filter")):
                actions = [n.get(ANDROID + "name") for n in intent_filter.findall("action")]
                categories = [n.get(ANDROID + "name") for n in intent_filter.findall("category")]
                if "android.intent.action.MAIN" in actions and "android.intent.category.LAUNCHER" in categories:
                    activity.remove(intent_filter)
                    found_launcher = True
        if name == "com.gtavsource.android.GTAVActivity":
            activity.set(ANDROID + "exported", "false")
    if not found_launcher:
        raise SystemExit("expected StoragePermissionActivity launcher filter not found")

    for provider in app.findall("provider"):
        authority = provider.get(ANDROID + "authorities", "")
        if OLD_AUTHORITY in authority:
            provider.set(ANDROID + "authorities", authority.replace(OLD_AUTHORITY, NEW_AUTHORITY))

    launcher = ET.SubElement(app, "activity", {
        ANDROID + "name": NEW_PACKAGE + ".LauncherActivity",
        ANDROID + "exported": "true",
        ANDROID + "screenOrientation": "sensorLandscape",
        ANDROID + "theme": "@android:style/Theme.Material.NoActionBar",
        ANDROID + "configChanges": "orientation|screenSize|keyboardHidden",
    })
    intent_filter = ET.SubElement(launcher, "intent-filter")
    ET.SubElement(intent_filter, "action", {ANDROID + "name": "android.intent.action.MAIN"})
    ET.SubElement(intent_filter, "category", {ANDROID + "name": "android.intent.category.LAUNCHER"})
    tree.write(manifest_path, encoding="utf-8", xml_declaration=True)

    smali_roots = sorted(root.glob("smali*"))
    if not smali_roots:
        raise SystemExit("apktool did not decode any smali directory")
    provider_files = [p for base in smali_roots for p in base.rglob("ModSpawnProvider.smali")]
    if not provider_files:
        raise SystemExit("ModSpawnProvider.smali not found; refusing incomplete authority patch")
    patched_package_refs = 0
    for base in smali_roots:
        for path in base.rglob("*.smali"):
            data = path.read_text(encoding="utf-8")
            if OLD_PACKAGE in data:
                path.write_text(data.replace(OLD_PACKAGE, NEW_PACKAGE), encoding="utf-8")
                patched_package_refs += 1
    for path in provider_files:
        data = path.read_text(encoding="utf-8")
        if NEW_AUTHORITY not in data:
            raise SystemExit(f"provider authority was not rewritten in {path}")
    if any(OLD_PACKAGE in p.read_text(encoding="utf-8") for base in smali_roots for p in base.rglob("*.smali")):
        raise SystemExit("old application ID remains in decoded DEX strings")

    smali = smali_roots[0] / "local" / "glue5" / "nativeapp"
    smali.mkdir(parents=True, exist_ok=True)
    (smali / "LauncherActivity.smali").write_text(LAUNCHER_SMALI, encoding="utf-8")
    (smali / "LauncherActivity$OpenActivity.smali").write_text(LISTENER_SMALI, encoding="utf-8")

    # Verify the original external game-data default remains untouched.
    game_root = next((p for base in smali_roots for p in base.rglob("GameRoot.smali")), None)
    if game_root is None or "/storage/emulated/0/Games/GTAV" not in game_root.read_text(encoding="utf-8"):
        raise SystemExit("original external game-data default path was not preserved")
    print("native launcher patch: PASS")
    print(f"applicationId={NEW_PACKAGE}")
    print(f"engineActivity=com.gtavsource.android.GTAVActivity")
    print(f"launcherActivity={NEW_PACKAGE}.LauncherActivity")
    print(f"gameDataDefault=/storage/emulated/0/Games/GTAV (preserved)")
    print(f"modProviderAuthority={NEW_AUTHORITY}")
    print(f"patchedPackageStringFiles={patched_package_refs}")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
