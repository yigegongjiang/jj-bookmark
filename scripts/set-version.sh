#!/usr/bin/env bash
# 单一版本源: 根 VERSION → 写入 CLI Cargo.toml 的 [package].version。
# App Info.plist 用模板占位 (@VERSION@)，由 package.sh 打包时替换，故此处只需同步 Cargo.toml。
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="$(tr -d ' \t\n\r' < "$ROOT/VERSION")"
[ -n "$VERSION" ] || { echo "set-version: VERSION 文件为空" >&2; exit 1; }

CARGO="$ROOT/jj-bookmark-cli/Cargo.toml"
[ -f "$CARGO" ] || { echo "set-version: 找不到 $CARGO" >&2; exit 1; }

# 仅替换 [package] 段内的 version 行, 不误伤依赖表里的 version。
awk -v v="$VERSION" '
  /^\[/        { section = $0 }
  section == "[package]" && /^version[[:space:]]*=/ { print "version = \"" v "\""; next }
                { print }
' "$CARGO" > "$CARGO.tmp" && mv "$CARGO.tmp" "$CARGO"

echo "set-version: VERSION=$VERSION -> $CARGO"
