#!/usr/bin/env python3
"""Guard reusable Glue 5 UI crates against engine/runtime dependencies."""
import json
import sys
from pathlib import Path

metadata = json.loads(Path(sys.argv[1]).read_text())
packages = {package["name"]: package for package in metadata["packages"]}

def dependencies(name):
    return {dependency["name"] for dependency in packages[name]["dependencies"]}

for name in ("glue5-ui-kit", "glue5-native-shell"):
    actual = dependencies(name)
    if actual != {"egui"}:
        raise SystemExit(
            f"{name} must depend only on egui; found: "
            + ", ".join(sorted(actual))
        )

app_dependencies = dependencies("glue5")
if "glue5-ui-kit" not in app_dependencies:
    raise SystemExit("glue5 must consume the standalone glue5-ui-kit crate")

if "glue5-native-shell" not in packages:
    raise SystemExit("glue5-native-shell is missing from the Cargo workspace")

print("Native UI dependency boundaries: OK (egui only)")
