```When Editing
本文档作用: 工程工作流程 (可用工具 / 调试 / 发布); MUST NOT 写工程说明 (→ README.md) / LLM 约束 (→ AGENTS.md)
遵循 AGENTS.md 文档编写规范
- 所有段落均为条件段, 根据工程实际决定保留或删除; 存在即为明确流程, MUST NOT 附加强度标记
- 发布内按顺序编号步骤; 顶部 TL;DR ≤ 5 行; 删除子段后重编号保持连续
- 风险点 / 不可逆操作用 `>` 引用块; 高危操作 MUST 标禁用条件
```

# 可用工具

- `gh` — 已登录（git push）

# 调试

双组件各自独立调试。

- CLI（`jj-bookmark-cli/`）：
  - 运行：`cargo run -- <args>`  # 例 `cargo run -- --help`
  - 测试：`cargo test`
  - 类型检查：`cargo check`
- App（`jj-bookmark-app/`）：
  - 快速编译：`cd jj-bookmark-app && swift build`
  - 构建 + 组装 `.app`（内嵌同版本 CLI）：`./jj-bookmark-app/package.sh [release|debug]`  # 默认 release
  - 运行：`open jj-bookmark-app/build/jj-bookmark.app`
  - 验证跨进程刷新：改动 `~/.config/jj-bookmark/bookmarks.json` 后 App 无需重启即刷新（FSEvents）

# 发布

代码变更完成后立即执行（= 需求交付的最后环节）。发布 = 本地验证 + 版本落定 + push tag；push tag `vX.Y.Z` 触发 GHA（`.github/workflows/release.yml`）自动打包 macOS App + CLI 并创建 GitHub Release（产物由 CI 构建，本地无需再传）。

## TL;DR

依序执行：

1. 验证：`cargo check && cargo test`（CLI）+ `./jj-bookmark-app/package.sh release`（App，含内嵌 CLI）
2. 写版本：改根 `VERSION` 一处 → `scripts/set-version.sh` 同步 `Cargo.toml`（App 版本由 `package.sh` 注入 `Info.plist`）+ `CHANGELOG.md` + `CHANGELOG.dev.md` 同步（与 tag 一致）
3. 发布：commit + annotated tag + push branch + tag → GHA 自动构建产物并创建 Release
4. 修上版 bug：amend + 删远程 tag + 重打 + force push（GHA 随新 tag 重跑）

## 1. 验证

- CLI：`cd jj-bookmark-cli && cargo check && cargo test`
- App：`./jj-bookmark-app/package.sh release`（构建 CLI + 编译 App + 组装 `.app` + 内嵌 CLI）

## 2. 写版本

- 版本号：默认递增 PATCH（第三位）；新功能 → MINOR；不兼容改动 → MAJOR。
- 单一版本源：编辑根 `VERSION` 一处 → 运行 `scripts/set-version.sh` 写入 `jj-bookmark-cli/Cargo.toml`；App 版本由 `package.sh` 从 `VERSION` 注入 `Info.plist`（无需手改）。
- 同步 `CHANGELOG.md` + `CHANGELOG.dev.md`（与 tag 一致）。

## 3. 发布

```bash
git add -A
git commit -m "release: vX.Y.Z"
git tag -a vX.Y.Z -m "vX.Y.Z"
git push origin master
git push origin vX.Y.Z
```

push tag 后 GHA 接管构建 + 发布（见 §5）；`gh run watch` 观察，完成后 `gh release view vX.Y.Z` 确认产物。

## 4. 修上版 bug

上版存在明显 bug 时，amend 修复后重新发布。

> `--force-with-lease` + 删远程 tag 会改写已推送历史；仅在「刚发布、远程未被他人拉取」时使用。

```bash
git commit --amend --no-edit
git tag -d vX.Y.Z
git push origin :refs/tags/vX.Y.Z
git tag -a vX.Y.Z -m "vX.Y.Z"
git push origin master --force-with-lease
git push origin vX.Y.Z
```

## 5. GHA（自动打包 + Release）

源: `.github/workflows/release.yml`。

- 触发：push tag `v*`。
- runner：`macos-26`（`.defaultIsolation` 需 Swift 6.2 / Xcode 26+；`macos-15` 仅 Xcode 16.x 不可用，勿降级）。
- 步骤：校验 `tag == vVERSION` → 选最新 Xcode + 断言 Swift ≥ 6.2 → rustup stable → `package.sh release` → ad-hoc 签名 → `ditto` 打 zip → `gh release create`。
- 产物（arm64 原生，未做 universal）：`jj-bookmark-macos-arm64.zip`（.app）、`jj-bookmark-cli-macos-arm64`（CLI）、`SHA256SUMS.txt`。
- notes：CHANGELOG 本版段 + `.github/release-notes-footer.md`（含 quarantine 移除说明）。
- 未签名/未公证 → 用户首次运行需 `xattr -dr com.apple.quarantine <path>`。

> tag 与 `VERSION` 不一致时 CI 直接失败（tag 已推送但不出 Release）；发前务必对齐。
