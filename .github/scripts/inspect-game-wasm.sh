#!/usr/bin/env bash
set -euo pipefail

wasm="${1:?usage: inspect-game-wasm.sh game.wasm output-dir}"
out="${2:?usage: inspect-game-wasm.sh game.wasm output-dir}"
mkdir -p "$out"

wasm-tools dump "$wasm" > "$out/sections.txt"
wasm-tools print "$wasm" > "$out/module.wat"

# Preserve the full textual module, but also produce compact indexes for review.
grep -E '\\(import |\\(export |\\(func \\$|\\(memory |\\(table ' \
  "$out/module.wat" > "$out/import-export-function-index.txt" || true

# These are search hints only, not proof that a callable hook exists.
# The WAT/name section and embedded strings are both searched so mangled
# C++ names and readable engine strings can be reviewed side by side.
strings -a "$wasm" > "$out/embedded-strings.txt"
grep -Ein \
  'fwArchetypeManager|GetArchetypeFromHashKey|Archetype|Packfile|Dlc|DLC|Streaming|Streamable|ResourceStore|Rpf|RPF|Yft|Ytd|Vehicle' \
  "$out/module.wat" "$out/embedded-strings.txt" \
  > "$out/engine-symbol-hints.txt" || true

{
  echo "Input: $wasm"
  sha256sum "$wasm"
  echo
  echo "NOTE: text matches are investigation leads, not verified callable hooks."
  echo "A function must be traced to its exact definition and calling convention."
} > "$out/README.txt"
