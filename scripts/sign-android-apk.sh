#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUILD_TOOLS="${ANDROID_BUILD_TOOLS:-$HOME/Library/Android/sdk/build-tools/36.0.0}"
ZIPALIGN="$BUILD_TOOLS/zipalign"
APKSIGNER="$BUILD_TOOLS/apksigner"

INPUT_APK="${1:-$ROOT_DIR/apps/story-tauri/src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release-unsigned.apk}"
OUTPUT_APK="${2:-$ROOT_DIR/dist/release/v0.1.0/Vellum-0.1.0-android-universal.apk}"
ALIGNED_APK="${OUTPUT_APK%.apk}-aligned.apk"

: "${VELLUM_ANDROID_KEYSTORE:?Set VELLUM_ANDROID_KEYSTORE to the signing keystore path}"
: "${VELLUM_ANDROID_KEYSTORE_PASSWORD:?Set VELLUM_ANDROID_KEYSTORE_PASSWORD}"
VELLUM_ANDROID_KEY_ALIAS="${VELLUM_ANDROID_KEY_ALIAS:-vellum}"
VELLUM_ANDROID_KEY_PASSWORD="${VELLUM_ANDROID_KEY_PASSWORD:-$VELLUM_ANDROID_KEYSTORE_PASSWORD}"
export VELLUM_ANDROID_KEYSTORE_PASSWORD
export VELLUM_ANDROID_KEY_PASSWORD

mkdir -p "$(dirname "$OUTPUT_APK")"

"$ZIPALIGN" -f -p 4 "$INPUT_APK" "$ALIGNED_APK"
"$APKSIGNER" sign \
  --ks "$VELLUM_ANDROID_KEYSTORE" \
  --ks-key-alias "$VELLUM_ANDROID_KEY_ALIAS" \
  --ks-pass env:VELLUM_ANDROID_KEYSTORE_PASSWORD \
  --key-pass env:VELLUM_ANDROID_KEY_PASSWORD \
  --out "$OUTPUT_APK" \
  "$ALIGNED_APK"

"$APKSIGNER" verify --verbose --print-certs "$OUTPUT_APK"
rm -f "$ALIGNED_APK"
