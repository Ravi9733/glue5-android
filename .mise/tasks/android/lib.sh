# Shared by the Android tasks (sourced; not a task itself: not executable).
set -euo pipefail

package=local.glue5
project=platform/android
out=$PWD/target/android
apk=$out/gradle/outputs/apk/release/Glue5-release.apk
abi=arm64-v8a
target=aarch64-linux-android

die() { echo "$*" >&2; exit 1; }

setting() { sed -n "s/^glue5\.$1=//p" "$project/gradle.properties"; }

sdk_packages() {
    [ -n "${ANDROID_HOME:-}" ] || die "ANDROID_HOME is not set: run the task through mise"
    local packages=("platforms;android-$(setting compileSdk)" "build-tools;$(setting buildTools)" "ndk;$(setting ndk)" platform-tools)
    local missing=() package
    for package in "${packages[@]}"; do
        [ -d "$ANDROID_HOME/${package//;//}" ] || missing+=("$package")
    done
    if [ ${#missing[@]} -gt 0 ]; then
        local sdkmanager_bin
        sdkmanager_bin="$(command -v sdkmanager)" || die "sdkmanager is not on PATH"
        # GitHub Actions is non-interactive: accept Android SDK package licenses
        # before sdkmanager installs missing platform/build-tools/NDK packages.
        # sdkmanager exits after the final license prompt; yes may receive
        # SIGPIPE (141) even though licenses were accepted successfully.
        # Preserve the sdkmanager status rather than yes's expected SIGPIPE.
        set +o pipefail
        yes | "$sdkmanager_bin" --sdk_root="$ANDROID_HOME" --licenses >/dev/null
        set -o pipefail
        "$sdkmanager_bin" --sdk_root="$ANDROID_HOME" --install "${missing[@]}"
    fi
    export ANDROID_NDK_HOME=$ANDROID_HOME/ndk/$(setting ndk)
    export ANDROID_NDK_ROOT=$ANDROID_NDK_HOME
    PATH=$ANDROID_HOME/platform-tools:$PATH
}

android_libs() {
    export CMAKE_TOOLCHAIN_FILE_${target//-/_}=$ANDROID_NDK_HOME/build/cmake/android.toolchain.cmake
    cargo ndk --target "$abi" --platform "$(setting minSdk)" rustc --release -p glue5 --lib --crate-type cdylib

    rm -rf "$out/jniLibs" "$out/res"
    mkdir -p "$out/jniLibs/$abi" "$out/res/drawable-nodpi"

    # SDL3's build script places libSDL3.so in its Cargo OUT_DIR, not beside
    # libglue5.so. Search the Android target's release artifacts recursively.
    local glue_lib="target/$target/release/libglue5.so"
    [ -f "$glue_lib" ] || die "Android glue5 library was not produced: $glue_lib"
    local sdl_lib
    sdl_lib=$(find "target/$target/release" -type f -name 'libSDL3.so' -print -quit)
    if [ -z "$sdl_lib" ]; then
        echo "SDL3 shared library not found under target/$target/release." >&2
        echo "Matching SDL artifacts:" >&2
        find "target/$target" -type f \( -iname '*sdl3*' -o -iname 'libSDL*' \) -print | sed -n '1,80p' >&2
        die "sdl3-sys did not produce the Android libSDL3.so required by SDL's Java loader"
    fi
    cp "$glue_lib" "$out/jniLibs/$abi/libglue5.so"
    cp "$sdl_lib" "$out/jniLibs/$abi/libSDL3.so"
    cp "$ANDROID_NDK_HOME"/toolchains/llvm/prebuilt/*/sysroot/usr/lib/"$target"/libc++_shared.so "$out/jniLibs/$abi/"

    # ReShade is optional: package its Vulkan layer when the separate
    # ARM64 runtime build is available; the core Turnip game does not depend on it.
    local reshade_layer="$out/reshade-layer/libVkLayer_VKBASALT_post_processing.so"
    if [ -s "$reshade_layer" ]; then
        cp "$reshade_layer" "$out/jniLibs/$abi/libVkLayer_VKBASALT_post_processing.so"
    else
        echo "Optional Android ReShade FX layer is absent; building core APK without it."
    fi

    # Build the Android Vulkan loader hooks needed by optional AdrenoTools
    # custom-driver support. Packaging alone does not activate Turnip.
    local adrenotools_src="$out/adrenotools-src"
    local adrenotools_build="$out/adrenotools-build"
    # The Android cache may restore a stale/incomplete checkout directory.
    # Treat the root CMakeLists.txt as the authoritative source-tree check;
    # a .git directory alone does not mean the cached source is usable.
    if [ ! -f "$adrenotools_src/CMakeLists.txt" ]; then
        rm -rf "$adrenotools_src"
        git clone --depth 1 https://github.com/Bylaws/libadrenotools.git "$adrenotools_src"
    fi
    [ -f "$adrenotools_src/CMakeLists.txt" ] ||
        die "AdrenoTools clone is missing its root CMakeLists.txt: $adrenotools_src"
    git -C "$adrenotools_src" rev-parse --verify HEAD >/dev/null ||
        die "AdrenoTools source checkout is incomplete: $adrenotools_src"
    git -C "$adrenotools_src" submodule update --init --recursive
    cmake -S "$adrenotools_src" -B "$adrenotools_build" \
        -DCMAKE_TOOLCHAIN_FILE="$ANDROID_NDK_HOME/build/cmake/android.toolchain.cmake" \
        -DANDROID_ABI="$abi" -DANDROID_PLATFORM="android-$(setting minSdk)" \
        -DANDROID_NDK="$ANDROID_NDK_HOME" -DBUILD_SHARED_LIBS=ON \
        -DGEN_INSTALL_TARGET=OFF -DCMAKE_BUILD_TYPE=Release
    cmake --build "$adrenotools_build" --parallel 1
    for lib in libadrenotools.so libhook_impl.so libmain_hook.so libfile_redirect_hook.so libgsl_alloc_hook.so; do
        found=$(find "$adrenotools_build" -type f -name "$lib" -print -quit)
        [ -n "$found" ] || die "AdrenoTools build did not produce $lib"
        cp "$found" "$out/jniLibs/$abi/$lib"
    done
    cp platform/shared/Glue5.icon/Assets/01-background.png "$out/res/drawable-nodpi/glue5_background.png"
    cp platform/shared/Glue5.icon/Assets/02-five.png "$out/res/drawable-nodpi/glue5_five.png"
}

android_apk() {
    local sdl
    # Capture metadata before filtering: with pipefail, `head -1` can close
    # the pipe early and make cargo metadata exit with SIGPIPE (141).
    local metadata
    metadata=$(cargo metadata --format-version 1 --locked)
    sdl=$(printf '%s\\n' "$metadata" |
        grep -o '"manifest_path":"[^"]*/sdl3-src-[^"/]*/Cargo.toml"' | sed -n '1p' | cut -d'"' -f4)
    [ -n "$sdl" ] || die "sdl3-src is not among the dependencies"
    gradle -p "$project" --project-cache-dir "$out/gradle-cache" --quiet \
        "-Pglue5.sdlJava=${sdl%/Cargo.toml}/SDL/android-project/app/src/main/java" assembleRelease
}

launch() {
    local since pid logcat key extras=()
    for key in RUST_LOG $(compgen -e | grep '^GLUE5_'); do
        case $key in GLUE5_TEAM | GLUE5_GAME_DIR) continue ;; esac
        [ -n "${!key:-}" ] || continue
        extras+=(--es "$key" "'${!key//\'/\'\\\'\'}'")
    done
    since=$(adb shell date +%s.000)
    adb shell am start -W -S -n "$package/.Glue5Activity" ${extras[@]+"${extras[@]}"} >/dev/null
    pid=$(adb shell pidof -s "$package") || die "the app did not start"
    adb logcat -v brief --pid="$pid" &
    logcat=$!
    trap 'kill ${logcat:-} 2>/dev/null' EXIT
    adb shell "while [ -d /proc/$pid ]; do sleep 1; done"
    sleep 1
    adb logcat -d -b crash -v brief -T "$since"
}
