#!/usr/bin/env python3
"""Guard the reusable UI kit against accidental engine/runtime dependencies."""
import json
import sys
from pathlib import Path

metadata = json.loads(Path(sys.argv[1]).read_text())
packages = {package["name"]: package for package in metadata["packages"]}

kit = packages["glue5-ui-kit"]
kit_dependencies = {dependency["name"] for dependency in kit["dependencies"]}
if kit_dependencies != {"egui"}:
    raise SystemExit(
        "glue5-ui-kit must depend only on egui; found: "
        + ", ".join(sorted(kit_dependencies))
    )

app_dependencies = {dependency["name"] for dependency in packages["glue5"]["dependencies"]}
if "glue5-ui-kit" not in app_dependencies:
    raise SystemExit("glue5 must consume the standalone glue5-ui-kit crate")

print("UI kit dependency boundary: OK (egui only)")
