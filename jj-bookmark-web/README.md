# jj-bookmark-web

只读 web 预览：Cloudflare Worker 从 R2 读 `bookmarks.json`，渲染仿 App 的浏览页（folder 树 + 搜索 + 排序）。单向同步——数据由 CLI `jj-bookmark push` 上传，web 侧无写入 / 无 pull。

## 架构

- `src/index.js` — Worker：`/api/bookmarks` 读 R2（缺失兜底空库），其余路由走静态资源。
- `public/index.html` — 单文件 SPA（内联 CSS/JS）：拉 `/api/bookmarks` 后内存过滤 / 排序，无构建步骤。
- `wrangler.toml` — R2 绑定 `BOOKMARKS`（bucket `jj-bookmark`）+ 静态资源 `ASSETS`。
- 认证：Cloudflare Access（Google IdP）在**边缘**网关，Worker 不做 JWT 校验（单一信任源）。

## 前置（人类在 Cloudflare 侧一次性配置）

1. 建 R2 bucket：`wrangler r2 bucket create jj-bookmark`（名字须与 `wrangler.toml` 一致，否则 deploy 校验失败）。
2. 配 Access：Zero Trust → Access → Applications，为本 Worker 域名建 self-hosted 应用，IdP 选 Google，策略限定允许的邮箱 / 域。
3. 配 GHA secrets（仓库 Settings → Secrets）：`CLOUDFLARE_API_TOKEN`（含 Workers + R2 编辑权限）、`CLOUDFLARE_ACCOUNT_ID`。

> 数据含内网 URL。**MUST 先让 Access 网关该路由，再 deploy**——否则 deploy 到 Access 生效之间 Worker 对公网裸奔。

## 调试（本地，无需云端登录）

```bash
npm install                                                   # 装 wrangler
echo '{"version":1,"bookmarks":[]}' > /tmp/seed.json
npx wrangler r2 object put jj-bookmark/bookmarks.json --file /tmp/seed.json --local  # 塞本地模拟 R2
npm run dev                                                   # 本地起 Worker（默认 http://localhost:8787）
```

`wrangler dev` 用本地 R2 模拟；`--local` 的 put 与 dev 共享同一持久化目录（默认 `.wrangler/`）。

## 部署

- 自动：push `master` 且改动 `jj-bookmark-web/**` → `.github/workflows/deploy-web.yml` 跑 `wrangler deploy`（亦可 workflow_dispatch 手动触发）。
- 手动：`npm run deploy`（需本机 `wrangler login` 或 `CLOUDFLARE_API_TOKEN`）。

## 数据流

`CLI jj-bookmark push` → wrangler 上传 `~/.config/jj-bookmark/bookmarks.json` 到 R2 `jj-bookmark/bookmarks.json` → Worker `/api/bookmarks` 读取 → 页面渲染。
