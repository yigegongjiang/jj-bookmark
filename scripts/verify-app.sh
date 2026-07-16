#!/usr/bin/env bash
# 无头自检已组装的 jj-bookmark.app：直接跑 bundle 内二进制（open 会丢 env），逐项断言后汇总。
# 承重底线（不依赖 WindowServer）：内嵌 CLI 版本 == VERSION + App 启动 + i18n。
# GUI 层（需 WindowServer）：窗口尺寸自检 → 设置窗口可开 + 闲置自动退出真实触发；连不上则降级跳过（不判失败）。
# 用法: ./verify-app.sh [/path/to/jj-bookmark.app]   (默认 build/jj-bookmark.app)
set -uo pipefail  # 不用 -e：逐项自行判定并累计失败

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="$(tr -d ' \t\n\r' < "$ROOT/VERSION")"
BUNDLE="${1:-$ROOT/jj-bookmark-app/build/jj-bookmark.app}"
APP_BIN="$BUNDLE/Contents/MacOS/jj-bookmark-app"
CLI_BIN="$BUNDLE/Contents/Helpers/jj-bookmark"
DATA_DIR="$(mktemp -d)"  # 隔离数据目录，避免污染真实偏好
trap 'rm -rf "$DATA_DIR"' EXIT

fail=0
pass() { printf '  \033[32mPASS\033[0m %s\n' "$1"; }
bad()  { printf '  \033[31mFAIL\033[0m %s\n' "$1"; fail=1; }
skip() { printf '  \033[33mSKIP\033[0m %s\n' "$1"; }

[ -x "$APP_BIN" ] || { echo "找不到 App 二进制: $APP_BIN" >&2; exit 2; }
[ -x "$CLI_BIN" ] || { echo "找不到内嵌 CLI: $CLI_BIN" >&2; exit 2; }

echo "==> 自检 bundle: $BUNDLE (期望版本 $VERSION)"

# [1] 内嵌 CLI 版本 == VERSION（承重底线）
cli_ver="$("$CLI_BIN" --version 2>&1)"; rc=$?
if [ $rc -eq 0 ] && printf '%s' "$cli_ver" | grep -qF "$VERSION"; then
  pass "内嵌 CLI 版本匹配: $cli_ver"
else
  bad "内嵌 CLI 版本不符: 期望含 $VERSION, 实得 '$cli_ver' (rc=$rc)"
fi

# [2] App 启动 + i18n（纯 Foundation，无 WindowServer；承重底线）
l10n="$(JJ_BOOKMARK_DUMP_L10N=1 "$APP_BIN" 2>&1)"; rc=$?
if [ $rc -eq 0 ] && printf '%s' "$l10n" | grep -q '^\[l10n\] detected'; then
  pass "App 启动 + i18n 自检: $(printf '%s' "$l10n" | grep '^\[l10n\] detected')"
else
  bad "App 启动/i18n 自检失败 (rc=$rc): $l10n"
fi

# [3] GUI 层探针：窗口尺寸自检（需 WindowServer；连不上则降级跳过）
win="$(JJ_BOOKMARK_DUMP_WINDOW=1 JJ_BOOKMARK_NO_INSTALL=1 JJ_BOOKMARK_DIR="$DATA_DIR" "$APP_BIN" 2>&1)"; rc=$?
if [ $rc -eq 0 ] && printf '%s' "$win" | grep -q '^\[window\] content ='; then
  pass "窗口尺寸自检: $(printf '%s' "$win" | grep '^\[window\] content =')"

  # [4] GUI 集成：设置窗口可开(无裁切) + 闲置自动退出真实触发。
  # 无 timeout 二进制 → perl alarm 兜底(时限 > AUTOEXIT_SECONDS)：
  # PASS = 进程在 alarm 前自退(status 0)；FAIL = 被 alarm 杀掉(rc&127≠0, 疑似卡死)。
  out="$(perl -e 'alarm 10; exec @ARGV' \
         env JJ_BOOKMARK_OPEN_SETTINGS=1 JJ_BOOKMARK_AUTOEXIT_SECONDS=2 \
             JJ_BOOKMARK_NO_INSTALL=1 JJ_BOOKMARK_DIR="$DATA_DIR" "$APP_BIN" 2>&1)"; rc=$?
  if [ $((rc & 127)) -ne 0 ]; then
    bad "闲置自动退出未触发(被 alarm 杀掉, 疑似卡死, rc=$rc)"
  elif printf '%s' "$out" | grep -q 'jj-bookmark\[selfcheck\]'; then
    pass "设置窗口自检 + 自动退出触发: $(printf '%s' "$out" | grep -o 'jj-bookmark\[selfcheck\].*' | head -1)"
  else
    bad "设置窗口 selfcheck 行缺失 (rc=$rc): $out"
  fi
else
  skip "GUI 层自检(WindowServer 不可达, rc=$rc) — 底线检查已足以放行"
fi

echo
if [ $fail -eq 0 ]; then
  echo "==> 自检通过 (version $VERSION)"
else
  echo "==> 自检存在失败项" >&2
  exit 1
fi
