#!/usr/bin/env bash
# 组装 jj-bookmark.app：构建 CLI + App（均 Release）→ 拼 bundle → 内嵌 CLI。
# 用法: ./package.sh [host|universal]   默认 host。
#   host       = 按 `uname -m` 单架构 Release；本机 install-local.sh 用。
#   universal  = arm64+x86_64 双架构 Release（Rust lipo + xcodebuild ARCHS）；CI 分发用。
# 恒 Release：本地调试请用 `swift build` / `cargo run`，本脚本不打 Debug。
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_DIR="$ROOT/jj-bookmark-app"
CLI_DIR="$ROOT/jj-bookmark-cli"
VERSION="$(tr -d ' \t\n\r' < "$ROOT/VERSION")"
MODE="${1:-host}"

case "$MODE" in
    host)
        case "$(uname -m)" in
            arm64)  RUST_TARGETS=("aarch64-apple-darwin"); XCODE_ARCHS="arm64" ;;
            x86_64) RUST_TARGETS=("x86_64-apple-darwin");  XCODE_ARCHS="x86_64" ;;
            *) echo "不支持的本机架构: $(uname -m)" >&2; exit 2 ;;
        esac
        ;;
    universal)
        RUST_TARGETS=("aarch64-apple-darwin" "x86_64-apple-darwin")
        XCODE_ARCHS="arm64 x86_64"
        ;;
    *) echo "仅支持 host 或 universal: $MODE" >&2; exit 2 ;;
esac
XCODE_BUILD="$APP_DIR/.build/xcode"

echo "==> [1/4] 同步版本号 (VERSION=$VERSION)"
bash "$ROOT/scripts/set-version.sh"

echo "==> [2/4] 构建 CLI (Release, targets: ${RUST_TARGETS[*]})"
(
    cd "$CLI_DIR"
    for t in "${RUST_TARGETS[@]}"; do
        rustup target add "$t" >/dev/null
        cargo build --release --target "$t"
    done
)
CLI_STAGED="$CLI_DIR/target/jj-bookmark-$MODE"
if [ "${#RUST_TARGETS[@]}" -eq 1 ]; then
    cp "$CLI_DIR/target/${RUST_TARGETS[0]}/release/jj-bookmark" "$CLI_STAGED"
else
    lipo -create -output "$CLI_STAGED" \
        "$CLI_DIR/target/aarch64-apple-darwin/release/jj-bookmark" \
        "$CLI_DIR/target/x86_64-apple-darwin/release/jj-bookmark"
fi
[ -x "$CLI_STAGED" ] || { echo "找不到 CLI 产物: $CLI_STAGED" >&2; exit 1; }

echo "==> [3/4] 构建 App (Release, ARCHS=$XCODE_ARCHS)"
(
    cd "$APP_DIR"
    xcodebuild \
        -scheme jj-bookmark-app \
        -configuration Release \
        -destination "generic/platform=macOS" \
        -derivedDataPath "$XCODE_BUILD" \
        ARCHS="$XCODE_ARCHS" ONLY_ACTIVE_ARCH=NO \
        build
)
APP_PRODUCTS="$XCODE_BUILD/Build/Products/Release"
APP_BIN="$APP_PRODUCTS/jj-bookmark-app"
[ -x "$APP_BIN" ] || { echo "找不到 App 产物: $APP_BIN" >&2; exit 1; }

echo "==> [4/4] 组装 .app bundle"
BUNDLE="$APP_DIR/build/jj-bookmark.app"
rm -rf "$BUNDLE"
mkdir -p "$BUNDLE/Contents/MacOS" "$BUNDLE/Contents/Resources" "$BUNDLE/Contents/Helpers"

cp "$APP_BIN" "$BUNDLE/Contents/MacOS/jj-bookmark-app"
cp "$CLI_STAGED" "$BUNDLE/Contents/Helpers/jj-bookmark"
cp "$APP_DIR/Resources/AppIcon.icns" "$BUNDLE/Contents/Resources/AppIcon.icns"
for RESOURCE_BUNDLE in "$APP_PRODUCTS/"*.bundle; do
    [ -d "$RESOURCE_BUNDLE" ] || continue
    cp -R "$RESOURCE_BUNDLE" "$BUNDLE/Contents/Resources/"
done
sed "s/@VERSION@/$VERSION/g" "$APP_DIR/Resources/Info.plist.in" > "$BUNDLE/Contents/Info.plist"
printf 'APPL????' > "$BUNDLE/Contents/PkgInfo"

echo "==> 完成: $BUNDLE (version $VERSION, mode $MODE)"
echo -n "    内嵌 CLI 版本: "
"$BUNDLE/Contents/Helpers/jj-bookmark" --version
echo -n "    CLI 架构: "
lipo -archs "$BUNDLE/Contents/Helpers/jj-bookmark"
echo -n "    App 架构:  "
lipo -archs "$BUNDLE/Contents/MacOS/jj-bookmark-app"
