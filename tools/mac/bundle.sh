#!/usr/bin/env bash
# Build Varos.app for macOS and install it where Spotlight / Launchpad / Finder find it.
#
#   tools/mac/bundle.sh            # build release (cargo no-ops if already fresh), bundle, sign, install
#   SKIP_BUILD=1 tools/mac/bundle.sh   # reuse varos/target/release/varos as-is
#   NO_INSTALL=1 OUT_DIR=/tmp/x tools/mac/bundle.sh   # build the bundle into /tmp/x only: nothing is
#                                  # copied to /Applications and Launch Services is not told
#   BIN=path/to/varos ...          # bundle this binary instead of target/release/varos (implies SKIP_BUILD)
#   BRIDGE_BIN=… CLI_BIN=…         # Bridge helpers to bundle (default: next to BIN, i.e. target/release/)
#   CODESIGN_ID="Varos Local"      # sign with this identity instead of ad-hoc (default: "-" = ad-hoc)
#
# Keychain note (ADR-0011 C1): the Bridge keys live in the login Keychain, whose access list trusts
# the signing identity of the binary that created them. Ad-hoc signatures change on every build, so
# macOS asks again ("Always Allow") after each rebuild. A stable self-signed code-signing identity
# (created once by the owner in Keychain Access ▸ Certificate Assistant) passed as CODESIGN_ID
# keeps the same identity across rebuilds and stops those prompts. The owner decides; nothing here
# creates certificates.
#
# The agent Bridge helpers (ADR-0011 §4) ship INSIDE the app so their path is stable:
#   Varos.app/Contents/MacOS/varos-bridge   (the MCP/stdio proxy agents register once)
#   Varos.app/Contents/MacOS/varos-cli      (owner commands: bridge register / pair / agents)
#
# Build outputs go under varos/target/mac/ (ignored via `target/`) unless OUT_DIR says otherwise.
# Nothing here needs sudo.
#
# File association (DFS S4): the Info.plist below declares `.vrs` (role Editor, rank Owner) and
# exports its UTI. That only ROUTES the file to Varos; the app receives it through the
# open-documents bridge in varos-app/src/mac_open.rs. Varos does not claim `.pdf`.
# The bundle is ad-hoc signed for LOCAL use only — it is not notarized and not for distribution.
set -euo pipefail

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "bundle.sh: macOS only" >&2
  exit 1
fi

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WORKSPACE="$REPO_ROOT/varos"
if [[ -n "${BIN:-}" ]]; then
  SKIP_BUILD=1
else
  BIN="$WORKSPACE/target/release/varos"
fi
BRIDGE_BIN="${BRIDGE_BIN:-$(dirname "$BIN")/varos-bridge}"
CLI_BIN="${CLI_BIN:-$(dirname "$BIN")/varos-cli}"
OUT_DIR="${OUT_DIR:-$WORKSPACE/target/mac}"
APP="$OUT_DIR/Varos.app"
# NO_INSTALL must never touch the live install: building straight into /Applications would
# delete and replace /Applications/Varos.app in step 3 (Codex review 2026-10-04).
if [[ "${NO_INSTALL:-0}" == "1" ]]; then
  OUT_REAL="$(cd "$OUT_DIR" 2>/dev/null && pwd -P || printf '%s' "$OUT_DIR")"
  case "$OUT_REAL" in
    /Applications|/Applications/*) echo "bundle.sh: NO_INSTALL=1 refuses OUT_DIR under /Applications ($OUT_DIR)" >&2; exit 1 ;;
  esac
fi
ICON_SRC="$REPO_ROOT/icon.png"
APP_CARGO="$WORKSPACE/crates/varos-app/Cargo.toml"

BUNDLE_ID="com.varos.editor"
SIGN_ID="${CODESIGN_ID:--}"
DOC_UTI="com.varos.editor.document"

export PATH="$HOME/.cargo/bin:$PATH"

# ---- 1. build (cargo only recompiles what changed; a fresh build is a fast no-op) ----
if [[ "${SKIP_BUILD:-0}" != "1" ]]; then
  echo "==> cargo build --release -p varos-app -p varos-bridge -p varos-cli"
  (cd "$WORKSPACE" && cargo build --release -p varos-app -p varos-bridge -p varos-cli -j 4)
fi
[[ -x "$BIN" ]] || { echo "bundle.sh: missing $BIN (build failed or SKIP_BUILD=1 without a build)" >&2; exit 1; }
for helper in "$BRIDGE_BIN" "$CLI_BIN"; do
  [[ -x "$helper" ]] || { echo "bundle.sh: missing Bridge helper $helper (build varos-bridge and varos-cli, or set BRIDGE_BIN/CLI_BIN)" >&2; exit 1; }
done

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

# Redistribute the copyright/OFL notices with the fonts embedded in the executable.
FONT_ASSETS="$WORKSPACE/crates/varos-app/assets/fonts"
mkdir -p "$APP/Contents/Resources/Licenses/Fonts"
cp "$FONT_ASSETS/"*-LICENSE.txt "$FONT_ASSETS/"*-OFL.txt "$FONT_ASSETS/manifest.json" "$APP/Contents/Resources/Licenses/Fonts/"

# ---- 5. binary ----
cp "$BIN" "$APP/Contents/MacOS/varos"
chmod +x "$APP/Contents/MacOS/varos"
# Bridge helpers beside the app binary: one stable path, updated together with the app.
cp "$BRIDGE_BIN" "$APP/Contents/MacOS/varos-bridge"
cp "$CLI_BIN" "$APP/Contents/MacOS/varos-cli"
chmod +x "$APP/Contents/MacOS/varos-bridge" "$APP/Contents/MacOS/varos-cli"

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
# the association keys must survive any future edit of the heredoc above
plist_get() { plutil -extract "$1" raw -o - "$APP/Contents/Info.plist"; }
[[ "$(plist_get CFBundleDocumentTypes.0.CFBundleTypeExtensions.0)" == "vrs" \
  && "$(plist_get CFBundleDocumentTypes.0.CFBundleTypeRole)" == "Editor" \
  && "$(plist_get CFBundleDocumentTypes.0.LSHandlerRank)" == "Owner" \
  && "$(plist_get CFBundleDocumentTypes.0.LSItemContentTypes.0)" == "$DOC_UTI" \
  && "$(plist_get UTExportedTypeDeclarations.0.UTTypeIdentifier)" == "$DOC_UTI" ]] \
  || { echo "bundle.sh: Info.plist lost the .vrs document type / UTI declaration" >&2; exit 1; }
printf 'APPL????' > "$APP/Contents/PkgInfo"

# ---- 7. sign: ad-hoc by default, or CODESIGN_ID (local use only; not notarized) ----
# Helpers first (nested code), then the bundle. Keychain item access is tied to these
# signatures; ad-hoc re-signing after a rebuild can make macOS ask once to allow access.
for helper in varos-bridge varos-cli; do
  codesign --force --sign "$SIGN_ID" "$APP/Contents/MacOS/$helper"
done
codesign --force --deep --sign "$SIGN_ID" "$APP"
codesign --verify --deep --strict "$APP"
for helper in varos-bridge varos-cli; do
  codesign --verify --strict "$APP/Contents/MacOS/$helper"
done

if [[ "${NO_INSTALL:-0}" == "1" ]]; then
  echo "==> built (not installed): $APP"
  exit 0
fi

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

# One Varos on this Mac: the staging bundle in the build folder is a second "Varos.app" that
# Spotlight and Launchpad would list next to the installed one (owner report 2026-10-04:
# "several copies on the Mac"). Remove it once the install is verified.
rm -rf "$APP"

echo "==> installed: $DEST"
echo "    agents: register once with  \"$DEST/Contents/MacOS/varos-cli\" bridge register claude"
echo "    check the .vrs association: Finder ▸ Get Info on a .vrs file shows \"Open with: Varos\","
echo "    or run: $LSREG -dump | grep -c $DOC_UTI"
echo "    then double-click a .vrs (Varos closed, and again with Varos open) — it opens as a tab."
