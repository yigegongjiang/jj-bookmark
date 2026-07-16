#!/usr/bin/env bash
# 本机发布：打 release 包 → 拷贝到 /Applications → 对已安装 bundle 无头自检。
# 本机自建 .app 无 quarantine 属性，Gatekeeper 不校验签名，直接可跑（无需签名/公证）。
# 用法: ./install-local.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC="$ROOT/jj-bookmark-app/build/jj-bookmark.app"
DEST="/Applications/jj-bookmark.app"
VERSION="$(tr -d ' \t\n\r' < "$ROOT/VERSION")"

echo "==> [1/3] 打 release 包"
"$ROOT/jj-bookmark-app/package.sh" release

echo "==> [2/3] 安装到 /Applications"
# 关掉可能在跑的旧实例，否则占用二进制导致拷贝/运行异常
pkill -f '/jj-bookmark\.app/Contents/MacOS/jj-bookmark-app' 2>/dev/null || true
rm -rf "$DEST"
ditto "$SRC" "$DEST"  # 拷贝而非移动：build/ 内正本保留，可重复执行
echo "    已安装: $DEST"

echo "==> [3/3] 自检 /Applications 内的 app"
"$ROOT/scripts/verify-app.sh" "$DEST"

echo "==> 本机发布完成: $DEST (version $VERSION)"
