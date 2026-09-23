#!/usr/bin/env bash
# Build Varos.app for macOS and install it where Spotlight / Launchpad / Finder find it.
#
#   tools/mac/bundle.sh            # build release (cargo no-ops if already fresh), bundle, sign, install
#   SKIP_BUILD=1 tools/mac/bundle.sh   # reuse varos/target/release/varos as-is
#
# Build outputs go under varos/target/mac/ (ignored via `target/`). Nothing here needs sudo.
# The bundle is ad-hoc signed for LOCAL use only — it is not notarized and not for distribution.
set -euo pipefail

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "bundle.sh: macOS only" >&2
  exit 1
fi

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WORKSPACE="$REPO_ROOT/varos"
BIN="$WORKSPACE/target/release/varos"
OUT_DIR="$WORKSPACE/target/mac"
APP="$OUT_DIR/Varos.app"
ICON_SRC="$REPO_ROOT/icon.png"
APP_CARGO="$WORKSPACE/crates/varos-app/Cargo.toml"

BUNDLE_ID="com.varos.editor"
DOC_UTI="com.varos.editor.document"

export PATH="$HOME/.cargo/bin:$PATH"

# ---- 1. build (cargo only recompiles what changed; a fresh build is a fast no-op) ----
if [[ "${SKIP_BUILD:-0}" != "1" ]]; then
  echo "==> cargo build --release -p varos-app"
  (cd "$WORKSPACE" && cargo build --release -p varos-app -j 4)
fi
[[ -x "$BIN" ]] || { echo "bundle.sh: missing $BIN (build failed or SKIP_BUILD=1 without a build)" >&2; exit 1; }

# ---- 2. version from varos-app's Cargo.toml ([package] version = "x.y.z") ----
VERSION="$(sed -n 's/^version[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' "$APP_CARGO" | head -n1)"
[[ -n "$VERSION" ]] || { echo "bundle.sh: could not read version from $APP_CARGO" >&2; exit 1; }
echo "==> version $VERSION"

# ---- 3. bundle skeleton ----
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

# ---- 4. icon: icon.png -> Varos.icns (all sizes 16..1024 incl. @2x) ----
[[ -f "$ICON_SRC" ]] || { echo "bundle.sh: missing $ICON_SRC" >&2; exit 1; }
ICONSET="$OUT_DIR/Varos.iconset"
rm -rf "$ICONSET"
mkdir -p "$ICONSET"
for size in 16 32 128 256 512; do
  double=$((size * 2))
  sips -s format png -z "$size" "$size" "$ICON_SRC" --out "$ICONSET/icon_${size}x${size}.png" >/dev/null
  sips -s format png -z "$double" "$double" "$ICON_SRC" --out "$ICONSET/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/Varos.icns"
rm -rf "$ICONSET"

# ---- 5. binary ----
cp "$BIN" "$APP/Contents/MacOS/varos"
chmod +x "$APP/Contents/MacOS/varos"

# ---- 6. Info.plist ----
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key>                 <string>Varos</string>
  <key>CFBundleDisplayName</key>          <string>Varos</string>
  <key>CFBundleIdentifier</key>           <string>${BUNDLE_ID}</string>
  <key>CFBundleExecutable</key>           <string>varos</string>
  <key>CFBundlePackageType</key>          <string>APPL</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>CFBundleVersion</key>              <string>${VERSION}</string>
  <key>CFBundleShortVersionString</key>   <string>${VERSION}</string>
  <key>CFBundleIconFile</key>             <string>Varos</string>
  <key>LSMinimumSystemVersion</key>       <string>13.0</string>
  <key>LSApplicationCategoryType</key>    <string>public.app-category.graphics-design</string>
  <key>NSHighResolutionCapable</key>      <true/>
  <key>NSHumanReadableCopyright</key>
  <string>Copyright © 2026 Varos contributors. Free software under the GNU General Public License v3.0 (GPL-3.0).</string>
  <key>CFBundleDocumentTypes</key>
  <array>
    <dict>
      <key>CFBundleTypeName</key>        <string>Varos Document</string>
      <key>CFBundleTypeRole</key>        <string>Editor</string>
      <key>LSHandlerRank</key>           <string>Owner</string>
      <key>CFBundleTypeIconFile</key>    <string>Varos</string>
      <key>CFBundleTypeExtensions</key>  <array><string>vrs</string></array>
      <key>LSItemContentTypes</key>      <array><string>${DOC_UTI}</string></array>
    </dict>
  </array>
  <key>UTExportedTypeDeclarations</key>
  <array>
    <dict>
      <key>UTTypeIdentifier</key>        <string>${DOC_UTI}</string>
      <key>UTTypeDescription</key>       <string>Varos Document</string>
      <key>UTTypeIconFile</key>          <string>Varos</string>
      <key>UTTypeConformsTo</key>        <array><string>public.data</string><string>public.content</string></array>
      <key>UTTypeTagSpecification</key>
      <dict>
        <key>public.filename-extension</key><array><string>vrs</string></array>
      </dict>
    </dict>
  </array>
</dict>
</plist>
PLIST
plutil -lint "$APP/Contents/Info.plist" >/dev/null
printf 'APPL????' > "$APP/Contents/PkgInfo"

# ---- 7. ad-hoc sign (local use only; not notarized) ----
codesign --force --deep --sign - "$APP"
codesign --verify --deep --strict "$APP"

# ---- 8. install: /Applications if writable without sudo, else ~/Applications ----
if [[ -w /Applications ]]; then
  DEST_DIR="/Applications"
else
  DEST_DIR="$HOME/Applications"
  mkdir -p "$DEST_DIR"
fi
DEST="$DEST_DIR/Varos.app"
rm -rf "$DEST"
# ditto keeps the bundle (and its signature) intact
ditto "$APP" "$DEST"
codesign --verify --deep --strict "$DEST"

# Tell Launch Services about the (re)installed bundle so Spotlight, Launchpad and the
# .vrs association pick it up right away.
LSREG="/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister"
if [[ -x "$LSREG" ]]; then
  "$LSREG" -f "$DEST" >/dev/null 2>&1 || true
fi

echo "==> installed: $DEST"
