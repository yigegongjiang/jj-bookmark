```When Editing
本文档作用: 工程工作流程 (可用工具 / 调试 / 发布); MUST NOT 写工程说明 (→ README.md) / LLM 约束 (→ AGENTS.md)
遵循 AGENTS.md 文档编写规范
- 所有段落均为条件段, 根据工程实际决定保留或删除; 存在即为明确流程, MUST NOT 附加强度标记
- 发布内按顺序编号步骤; 顶部 TL;DR ≤ 5 行; 删除子段后重编号保持连续
- 风险点 / 不可逆操作用 `>` 引用块; 高危操作 MUST 标禁用条件
```

# 可用工具

- `gh` — 已登录
- `npx wrangler` — 已登录（R2 / 部署）；无全局二进制，经 npx

# 调试

四组件各自独立调试。

- CLI（`jj-bookmark-cli/`）：`cargo check` / `cargo test` / `cargo run -- --help`
  - 碰真实数据的手动验证 → 复制数据文件到临时 HOME 再跑：`HOME=<tmp> ./target/release/jj-bookmark ...`（`Paths::resolve` 只认 `HOME`）
  - `sync` 需 `~/.config/jj-bookmark/credentials.json`，建法见 [jj-bookmark-web/README.md](./jj-bookmark-web/README.md) 前置
- App（`jj-bookmark-app/`）：
  - 快编（本机架构，不组 bundle）：`cd jj-bookmark-app && swift build`
  - 组 `.app`（Release + 内嵌同版本 CLI）：`./jj-bookmark-app/package.sh [host|universal]`  # 默认 host；universal 供 CI
  - 运行：`open jj-bookmark-app/build/jj-bookmark.app`
  - 跨进程刷新：改 `~/.config/jj-bookmark/bookmarks.json` 后 App 不重启即刷新（FSEvents）
- Raycast（`raycast/`，dev-only 不发布）：`cd raycast && npm run dev` / `npm run lint`
- Web（`jj-bookmark-web/`）：不用 `wrangler dev`（无真实数据且绕过 Access，验不出真问题）；代码变更直接进发布，push master 由 `deploy-web.yml` 部署
  - 改到写路径（页面增删改 / `/api/*` 的 PUT）→ 先在本地同源桩上跑通增 / 改 / 删 / 409 再部署；页面可写，坏代码上线会直接改坏真实数据
  - 视觉 / 交互改版或数据结构变更 → 部署完成后线上复核：ego-browser skill 开 `https://jj-bookmark.yigegongjiang.com`（书签页 `/`、导航页 `/123`）
  - 手段：截图验视觉 / 合成 DragEvent 验拖拽 / 页面内 `fetch('/api/nav')` 验字段白名单与 `If-Match` 409（curl 无 Access token 必 403）
  - 停在 Google 登录页 → 人类协助：`curl -s -G 'https://jj-cloudflare.yigegongjiang.com/notify' --data-urlencode 'text=jj-bookmark 线上复核需登录，请在 ego-browser 完成 Google 登录'`
  - 复核有问题 → 修复后重走部署 + 复核

# 发布

代码变更完成后立即执行（= 需求交付的最后环节）。

## TL;DR

依序执行：

1. 验证：`cd jj-bookmark-cli && cargo check && cargo test`
2. 写版本：根 `VERSION` → `scripts/set-version.sh`；同步 `CHANGELOG.md` + `CHANGELOG.dev.md`
3. 预部署：`./scripts/install-local.sh`
4. 发布：commit + annotated tag + push branch + tag

## 1. 验证

`cd jj-bookmark-cli && cargo check && cargo test`

App 构建由 §3 覆盖（打包失败即中止发布）。

## 2. 写版本

- 版本号：默认递增 PATCH；新功能 → MINOR；MAJOR 仅人类主动要求。
- 单一版本源：编辑根 `VERSION` 一处 → `scripts/set-version.sh` 写 CLI `Cargo.toml`；App 版本由 `package.sh` 注入 `Info.plist`。
- 同步 `CHANGELOG.md` + `CHANGELOG.dev.md`，与 tag 一致。

> tag 与 `VERSION` 不一致 → release GHA 直接失败（tag 已推送却无 Release）。

## 3. 预部署

```bash
./scripts/install-local.sh   # 本机架构 Release 包 → 装 /Applications；非 0 退出即中止发布
```

## 4. 发布

```bash
git add -A
git commit -m "release: vX.Y.Z"
git tag -a vX.Y.Z -m "vX.Y.Z"
git push origin master
git push origin vX.Y.Z
```

push 后两条 GHA 自动跑，fire-and-forget（不观察、不轮询，出错人类自行发现）：

- `release.yml` — tag `v*` 触发；runner `macos-26`（`.defaultIsolation` 需 Swift 6.2 / Xcode 26+，勿降级）；`package.sh universal` + ad-hoc 签名 → `jj-bookmark-macos-universal.zip` / `jj-bookmark-cli-macos-universal` / `SHA256SUMS.txt` + GitHub Release（notes = CHANGELOG 本版段 + `.github/release-notes-footer.md`）。未公证 → 用户首次运行需 `xattr -dr com.apple.quarantine <path>`。
- `deploy-web.yml` — master 改 `jj-bookmark-web/**` 触发，跑 `wrangler deploy`；secrets（`CLOUDFLARE_API_TOKEN` / `CLOUDFLARE_ACCOUNT_ID`）与 R2 bucket、Access 由人类预置，见 [jj-bookmark-web/README.md](./jj-bookmark-web/README.md)。

## 5. 修上版 bug

上版存在明显 bug 时，amend 修复后重发。

> 改写已推送历史 + 删远程 tag；仅在「刚发布、远程未被他人拉取」时使用。

```bash
./scripts/install-local.sh
git commit --amend --no-edit
git tag -d vX.Y.Z
git push origin :refs/tags/vX.Y.Z
git tag -a vX.Y.Z -m "vX.Y.Z"
git push origin master --force-with-lease
git push origin vX.Y.Z
```
