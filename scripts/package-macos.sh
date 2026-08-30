#!/usr/bin/env bash
# Assemble a universal (or single-arch) XManager.app and zip it with the CLI.
# Requires macOS: sips, iconutil, lipo, ditto.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION=""
OUT="$ROOT/dist"
UI=""
UI_ARM=""
UI_X64=""
CLI=""
CLI_ARM=""
CLI_X64=""
ICON_SRC="$ROOT/crates/xmanager-tauri/logo.png"
PLIST_IN="$ROOT/packaging/macos/Info.plist.in"

usage() {
  cat <<'EOF'
Usage: scripts/package-macos.sh --version X.Y.Z [options]

  --version VERSION          CFBundleShortVersionString (required)
  --ui PATH                  Single xmanager binary
  --ui-arm PATH              aarch64 xmanager (for universal)
  --ui-x64 PATH              x86_64 xmanager (for universal)
  --cli PATH                 Single xmanager-cli binary
  --cli-arm PATH             aarch64 xmanager-cli
  --cli-x64 PATH             x86_64 xmanager-cli
  --out DIR                  Output directory (default: dist)
EOF
}

die() {
  echo "package-macos: $*" >&2
  exit 1
}

require_file() {
  local path="$1"
  local label="$2"
  [[ -f "$path" ]] || die "$label not found: $path"
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version) VERSION="${2:-}"; shift 2 ;;
    --ui) UI="${2:-}"; shift 2 ;;
    --ui-arm) UI_ARM="${2:-}"; shift 2 ;;
    --ui-x64) UI_X64="${2:-}"; shift 2 ;;
    --cli) CLI="${2:-}"; shift 2 ;;
    --cli-arm) CLI_ARM="${2:-}"; shift 2 ;;
    --cli-x64) CLI_X64="${2:-}"; shift 2 ;;
    --out) OUT="${2:-}"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) die "unknown argument: $1" ;;
  esac
done

[[ -n "$VERSION" ]] || die "--version is required"
[[ "$(uname -s)" == Darwin ]] || die "must run on macOS"

command -v sips >/dev/null || die "sips not found"
command -v iconutil >/dev/null || die "iconutil not found"
command -v lipo >/dev/null || die "lipo not found"
command -v ditto >/dev/null || die "ditto not found"

lipo_or_copy() {
  local dest="$1"
  shift
  local files=()
  local f
  for f in "$@"; do
    [[ -n "$f" ]] || continue
    require_file "$f" "binary"
    files+=("$f")
  done
  [[ ${#files[@]} -gt 0 ]] || die "no binaries for $dest"
  mkdir -p "$(dirname "$dest")"
  if [[ ${#files[@]} -eq 1 ]]; then
    cp "${files[0]}" "$dest"
  else
    lipo -create "${files[@]}" -output "$dest"
  fi
  chmod +x "$dest"
  if command -v strip >/dev/null; then
    strip -x "$dest" || true
  fi
}

require_file "$ICON_SRC" "logo.png"
require_file "$PLIST_IN" "Info.plist.in"
require_file "$ROOT/.env.example" ".env.example"
require_file "$ROOT/LICENSE" "LICENSE"
require_file "$ROOT/packaging/README.txt" "packaging README"

STAGE_NAME="XManager-${VERSION}-macos-universal"
case "$OUT" in
  /*) ;;
  *) OUT="$PWD/$OUT" ;;
esac
mkdir -p "$OUT"
STAGE="$OUT/$STAGE_NAME"
rm -rf "$STAGE"
mkdir -p "$STAGE"

APP="$STAGE/XManager.app"
CONTENTS="$APP/Contents"
MACOS_DIR="$CONTENTS/MacOS"
RESOURCES="$CONTENTS/Resources"
mkdir -p "$MACOS_DIR" "$RESOURCES"

if [[ -n "$UI" ]]; then
  lipo_or_copy "$MACOS_DIR/xmanager" "$UI"
else
  [[ -n "$UI_ARM" && -n "$UI_X64" ]] || die "pass --ui or both --ui-arm and --ui-x64"
  lipo_or_copy "$MACOS_DIR/xmanager" "$UI_ARM" "$UI_X64"
fi

if [[ -n "$CLI" ]]; then
  lipo_or_copy "$STAGE/xmanager-cli" "$CLI"
else
  [[ -n "$CLI_ARM" && -n "$CLI_X64" ]] || die "pass --cli or both --cli-arm and --cli-x64"
  lipo_or_copy "$STAGE/xmanager-cli" "$CLI_ARM" "$CLI_X64"
fi

ICONSET="$(mktemp -d)/AppIcon.iconset"
mkdir -p "$ICONSET"
sips -z 16 16 "$ICON_SRC" --out "$ICONSET/icon_16x16.png" >/dev/null
sips -z 32 32 "$ICON_SRC" --out "$ICONSET/icon_16x16@2x.png" >/dev/null
sips -z 32 32 "$ICON_SRC" --out "$ICONSET/icon_32x32.png" >/dev/null
sips -z 64 64 "$ICON_SRC" --out "$ICONSET/icon_32x32@2x.png" >/dev/null
sips -z 128 128 "$ICON_SRC" --out "$ICONSET/icon_128x128.png" >/dev/null
sips -z 256 256 "$ICON_SRC" --out "$ICONSET/icon_128x128@2x.png" >/dev/null
sips -z 256 256 "$ICON_SRC" --out "$ICONSET/icon_256x256.png" >/dev/null
sips -z 512 512 "$ICON_SRC" --out "$ICONSET/icon_256x256@2x.png" >/dev/null
sips -z 512 512 "$ICON_SRC" --out "$ICONSET/icon_512x512.png" >/dev/null
sips -z 1024 1024 "$ICON_SRC" --out "$ICONSET/icon_512x512@2x.png" >/dev/null
iconutil -c icns "$ICONSET" -o "$RESOURCES/AppIcon.icns"
rm -rf "$(dirname "$ICONSET")"

sed "s/{{VERSION}}/${VERSION}/g" "$PLIST_IN" > "$CONTENTS/Info.plist"
printf 'APPL????' > "$CONTENTS/PkgInfo"

cp "$ROOT/.env.example" "$STAGE/.env.example"
cp "$ROOT/LICENSE" "$STAGE/LICENSE"
{
  printf 'XManager %s\n\n' "$VERSION"
  cat "$ROOT/packaging/README.txt"
} > "$STAGE/README.txt"

[[ -x "$MACOS_DIR/xmanager" ]] || die "app executable is not +x"
[[ -f "$RESOURCES/AppIcon.icns" ]] || die "AppIcon.icns missing"
grep -q "$VERSION" "$CONTENTS/Info.plist" || die "Info.plist missing version"

if [[ -n "$UI_ARM" && -n "$UI_X64" ]]; then
  info="$(lipo -info "$MACOS_DIR/xmanager")"
  echo "$info" | grep -q arm64 || die "UI binary missing arm64: $info"
  echo "$info" | grep -q x86_64 || die "UI binary missing x86_64: $info"
fi

ZIP="$OUT/${STAGE_NAME}.zip"
rm -f "$ZIP"
(cd "$OUT" && ditto -c -k --keepParent "$STAGE_NAME" "$ZIP")

echo "Wrote $ZIP"
lipo -info "$MACOS_DIR/xmanager" || true
lipo -info "$STAGE/xmanager-cli" || true
ls -la "$STAGE"
