#!/bin/bash
# Builds the muxy-mobile SDK for the phone apps: an iOS XCFramework with Swift
# bindings and, when cargo-ndk is installed, Android libraries with Kotlin bindings.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="${1:-$ROOT/target/mobile-sdk}"
TOOLCHAIN="$(sed -n 's/^channel = "\(.*\)"/\1/p' "$ROOT/rust-toolchain.toml")"
IOS_TARGETS=(aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios)
ANDROID_ABIS=(arm64-v8a armeabi-v7a x86_64)

if [[ "$(uname -s)" != Darwin ]]; then
    echo "Error: building the iOS SDK requires macOS and Xcode" >&2
    exit 1
fi

cd "$ROOT"
# Replace only what this script writes; OUT may be a directory the user owns.
rm -rf "$OUT/swift" "$OUT/ios" "$OUT/MuxyMobile.xcframework" "$OUT/android"
mkdir -p "$OUT"

bindings() {
    local language="$1" destination="$2"
    cargo run --locked -p muxy-mobile --features bindgen --bin uniffi-bindgen -- \
        generate --library "$ROOT/target/debug/libmuxy_mobile.dylib" \
        --language "$language" --out-dir "$destination"
}

# Bindings are generated from a host build; they are identical for every target.
cargo build --locked -p muxy-mobile --lib

rustup target add --toolchain "$TOOLCHAIN" "${IOS_TARGETS[@]}"
for target in "${IOS_TARGETS[@]}"; do
    cargo build --locked --release -p muxy-mobile --lib --target "$target"
done

bindings swift "$OUT/swift"
HEADERS="$OUT/ios/headers"
mkdir -p "$HEADERS" "$OUT/ios/simulator"
cp "$OUT/swift/muxy_mobileFFI.h" "$HEADERS/"
cp "$OUT/swift/muxy_mobileFFI.modulemap" "$HEADERS/module.modulemap"
lipo -create \
    "target/aarch64-apple-ios-sim/release/libmuxy_mobile.a" \
    "target/x86_64-apple-ios/release/libmuxy_mobile.a" \
    -output "$OUT/ios/simulator/libmuxy_mobile.a"
xcodebuild -create-xcframework \
    -library "target/aarch64-apple-ios/release/libmuxy_mobile.a" -headers "$HEADERS" \
    -library "$OUT/ios/simulator/libmuxy_mobile.a" -headers "$HEADERS" \
    -output "$OUT/MuxyMobile.xcframework"

if command -v cargo-ndk >/dev/null; then
    targets=()
    for abi in "${ANDROID_ABIS[@]}"; do
        targets+=(-t "$abi")
    done
    rustup target add --toolchain "$TOOLCHAIN" \
        aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
    cargo ndk "${targets[@]}" -o "$OUT/android/jniLibs" build --locked --release -p muxy-mobile --lib
    bindings kotlin "$OUT/android/kotlin"
else
    echo "Skipping Android: install cargo-ndk to build it" >&2
fi

echo "Mobile SDK written to $OUT"
