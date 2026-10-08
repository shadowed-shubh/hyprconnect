#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ANDROID_DIR="$ROOT_DIR/android"
ANDROID_SDK_ROOT="${ANDROID_SDK_ROOT:-${ANDROID_HOME:-$HOME/Android/Sdk}}"
NDK_VERSION="${HYPRCONNECT_NDK_VERSION:-30.0.16248370}"
NDK_DIR="${ANDROID_NDK_HOME:-${ANDROID_NDK_ROOT:-$ANDROID_SDK_ROOT/ndk/$NDK_VERSION}}"
export ANDROID_SDK_ROOT

X86_TARGET="x86_64-linux-android"
ARM_TARGET="aarch64-linux-android"

die() {
    echo "build-android: $*" >&2
    exit 1
}

require_command() {
    command -v "$1" >/dev/null 2>&1 || die "required command not found: $1"
}

require_command cargo
require_command rustup
require_command java
require_command mktemp

[[ -x "$ANDROID_DIR/gradlew" ]] || die "Android Gradle wrapper is missing or not executable"
[[ -d "$NDK_DIR" ]] || die "Android NDK not found at $NDK_DIR (set ANDROID_NDK_HOME or HYPRCONNECT_NDK_VERSION)"

case "$(uname -s):$(uname -m)" in
    Linux:x86_64) HOST_TAG="linux-x86_64" ;;
    Darwin:x86_64) HOST_TAG="darwin-x86_64" ;;
    Darwin:arm64) HOST_TAG="darwin-arm64" ;;
    *) die "unsupported Android NDK host: $(uname -s):$(uname -m)" ;;
esac

TOOLCHAIN="$NDK_DIR/toolchains/llvm/prebuilt/$HOST_TAG/bin"
X86_LINKER="$TOOLCHAIN/x86_64-linux-android35-clang"
ARM_LINKER="$TOOLCHAIN/aarch64-linux-android35-clang"
[[ -x "$X86_LINKER" ]] || die "Android x86_64 linker not found: $X86_LINKER"
[[ -x "$ARM_LINKER" ]] || die "Android arm64 linker not found: $ARM_LINKER"

rustup target list --installed | grep -qx "$X86_TARGET" \
    || die "Rust target $X86_TARGET is not installed; run: rustup target add $X86_TARGET"
rustup target list --installed | grep -qx "$ARM_TARGET" \
    || die "Rust target $ARM_TARGET is not installed; run: rustup target add $ARM_TARGET"

echo "[1/4] Building Rust core for $X86_TARGET"
CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER="$X86_LINKER" \
    cargo build -p hyprconnect-core --target "$X86_TARGET"

echo "[2/4] Building Rust core for $ARM_TARGET"
CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$ARM_LINKER" \
    cargo build -p hyprconnect-core --target "$ARM_TARGET"

echo "[3/4] Generating UniFFI Kotlin bindings"
BINDINGS_DIR="$(mktemp -d)"
trap 'rm -rf "$BINDINGS_DIR"' EXIT
cargo run -q -p hyprconnect-core --bin uniffi-bindgen -- \
    generate "$ROOT_DIR/core/src/hyprconnect.udl" \
    --language kotlin \
    --out-dir "$BINDINGS_DIR"

GENERATED_BINDING="$BINDINGS_DIR/uniffi/hyprconnect_core/hyprconnect_core.kt"
[[ -f "$GENERATED_BINDING" ]] || die "UniFFI did not generate $GENERATED_BINDING"
cp "$GENERATED_BINDING" \
    "$ROOT_DIR/android-bindings/uniffi/hyprconnect_core/hyprconnect_core.kt"
cp "$GENERATED_BINDING" \
    "$ANDROID_DIR/app/src/main/kotlin/uniffi/hyprconnect_core/hyprconnect_core.kt"
cmp -s \
    "$ROOT_DIR/android-bindings/uniffi/hyprconnect_core/hyprconnect_core.kt" \
    "$ANDROID_DIR/app/src/main/kotlin/uniffi/hyprconnect_core/hyprconnect_core.kt" \
    || die "generated Kotlin binding copies differ"

cp "$ROOT_DIR/target/$X86_TARGET/debug/libhyprconnect_core.so" \
    "$ANDROID_DIR/app/src/main/jniLibs/x86_64/libuniffi_hyprconnect_core.so"
cp "$ROOT_DIR/target/$ARM_TARGET/debug/libhyprconnect_core.so" \
    "$ANDROID_DIR/app/src/main/jniLibs/arm64-v8a/libuniffi_hyprconnect_core.so"

echo "[4/4] Building Android debug APK"
"$ANDROID_DIR/gradlew" --no-daemon --console=plain \
    -p "$ANDROID_DIR" assembleDebug

echo "Android build complete: $ANDROID_DIR/app/build/outputs/apk/debug/app-debug.apk"
