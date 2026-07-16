#!/usr/bin/env bash
# 组装 jj-bookmark.app：先构建同版本 CLI，再编译 App，最后拼 bundle 并内嵌 CLI。
# 用法: ./package.sh [release|debug]   (默认 release)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_DIR="$ROOT/jj-bookmark-app"
CLI_DIR="$ROOT/jj-bookmark-cli"
VERSION="$(tr -d ' \t\n\r' < "$ROOT/VERSION")"
CONFIG="${1:-release}"
case "$CONFIG" in
    release) XCODE_CONFIG="Release" ;;
    debug) XCODE_CONFIG="Debug" ;;
    *) echo "仅支持 release 或 debug: $CONFIG" >&2; exit 2 ;;
esac
XCODE_BUILD="$APP_DIR/.build/xcode"

echo "==> [1/4] 同步版本号 (VERSION=$VERSION)"
bash "$ROOT/scripts/set-version.sh"

echo "==> [2/4] 构建 CLI (release)"
( cd "$CLI_DIR" && cargo build --release )
CLI_BIN="$CLI_DIR/target/release/jj-bookmark"
[ -x "$CLI_BIN" ] || { echo "找不到 CLI 产物: $CLI_BIN" >&2; exit 1; }

echo "==> [3/4] 构建 App ($CONFIG)"
( cd "$APP_DIR" && xcodebuild \
    -scheme jj-bookmark-app \
    -configuration "$XCODE_CONFIG" \
    -destination "platform=macOS,arch=arm64" \
    -derivedDataPath "$XCODE_BUILD" \
    build )
APP_PRODUCTS="$XCODE_BUILD/Build/Products/$XCODE_CONFIG"
APP_BIN="$APP_PRODUCTS/jj-bookmark-app"
[ -x "$APP_BIN" ] || { echo "找不到 App 产物: $APP_BIN" >&2; exit 1; }

echo "==> [4/4] 组装 .app bundle"
BUNDLE="$APP_DIR/build/jj-bookmark.app"
rm -rf "$BUNDLE"
mkdir -p "$BUNDLE/Contents/MacOS" "$BUNDLE/Contents/Resources" "$BUNDLE/Contents/Helpers"

cp "$APP_BIN" "$BUNDLE/Contents/MacOS/jj-bookmark-app"
cp "$CLI_BIN" "$BUNDLE/Contents/Helpers/jj-bookmark"        # 内嵌同版本 CLI
cp "$APP_DIR/Resources/AppIcon.icns" "$BUNDLE/Contents/Resources/AppIcon.icns"
for RESOURCE_BUNDLE in "$APP_PRODUCTS/"*.bundle; do
    [ -d "$RESOURCE_BUNDLE" ] || continue
    cp -R "$RESOURCE_BUNDLE" "$BUNDLE/Contents/Resources/"
done
sed "s/@VERSION@/$VERSION/g" "$APP_DIR/Resources/Info.plist.in" > "$BUNDLE/Contents/Info.plist"
printf 'APPL????' > "$BUNDLE/Contents/PkgInfo"

echo "==> 完成: $BUNDLE (version $VERSION)"
echo -n "    内嵌 CLI 版本: "
"$BUNDLE/Contents/Helpers/jj-bookmark" --version
