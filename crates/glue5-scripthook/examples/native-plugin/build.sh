#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
NDK="${ANDROID_NDK_HOME:-${ANDROID_NDK_ROOT:-}}"
API="${ANDROID_API_LEVEL:-29}"
OUT="${1:-"$ROOT/build/arm64-v8a/sample.so"}"

if [[ -z "$NDK" || ! -d "$NDK/toolchains/llvm/prebuilt" ]]; then
  echo "Set ANDROID_NDK_HOME (or ANDROID_NDK_ROOT) to an installed Android NDK." >&2
  exit 2
fi

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) HOST_TAG=linux-x86_64 ;;
  Linux-aarch64) HOST_TAG=linux-aarch64 ;;
  Darwin-arm64) HOST_TAG=darwin-arm64 ;;
  Darwin-x86_64) HOST_TAG=darwin-x86_64 ;;
  *) echo "Unsupported NDK host: $(uname -s)-$(uname -m)" >&2; exit 2 ;;
esac

CLANG="$NDK/toolchains/llvm/prebuilt/$HOST_TAG/bin/aarch64-linux-android${API}-clang"
if [[ ! -x "$CLANG" ]]; then
  echo "Android NDK compiler not found: $CLANG" >&2
  exit 2
fi

mkdir -p "$(dirname "$OUT")"
"$CLANG" \
  -std=c11 -O2 -fPIC -shared \
  -Wall -Wextra -Werror \
  -I"$ROOT/../../include" \
  "$ROOT/plugin.c" \
  -Wl,--no-undefined \
  -o "$OUT"

file "$OUT"
echo "Built Android ARM64 plugin: $OUT"
