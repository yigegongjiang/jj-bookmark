# jj-bookmark-web

一个 Worker 两张页：① 只读 web 预览——从 R2 读 `bookmarks.json` 渲染仿 App 浏览页（folder 树 + 搜索 + 排序），数据由 CLI `jj-bookmark push` 单向上传；② 个人导航页——`123.yigegongjiang.com`（或预览域 `/123`），卡片面板（搜索 / 分组筛选 / 高频·最近·手动三态排序 / 拖拽 / 内联增删改），R2 `nav.json` 为唯一数据源（无本地副本 / 无 CLI 参与）。

## 架构

- `src/index.js` — Worker：`/api/bookmarks` 读 R2（缺失兜底空库）；`/api/nav` GET/PUT `nav.json`（PUT 服务端字段白名单重建 + `If-Match`→R2 `onlyIf.etagMatches` 条件写，失配 409 防多 tab 相互覆盖）；host = `123.yigegongjiang.com` 时任意非 API 路径渲染导航页（请求不带 Access JWT = 该域尚未被 Access 应用覆盖 → 302 主域 `/123` 走既有登录网关，覆盖后自动本域直出）；其余路由走静态资源。
- `public/index.html` — 只读预览单文件 SPA（内联 CSS/JS）：拉 `/api/bookmarks` 后内存过滤 / 排序，无构建步骤。
- `public/123.html` — 导航页单文件 SPA（暗 / 亮双主题）：卡片网格 + 搜索（`/` 聚焦、Enter 开首个结果）+ 分组 chip 筛选 + 排序三态（高频 / 最近 / 手动）；hover 出编辑·删除，内联表单增改（名称 / URL / 分组 / 8 色），手动态下拖拽排序（组内 / 跨组 / 分组间）+ 分组重命名；变更 500ms 防抖自动 PUT 整份文档，409 时提示并重载。
- 导航数据 v2 = `{ version:2, groups:[名], links:[{name,url,group,color,createdAt}] }`：links 扁平，其顺序即手动排序（组内相对次序），`groups` 只记分组展示顺序；页面读到 v1（`groups:[{name,links}]`）在客户端迁移后写回，Worker 只接受 v2。点击次数属本机偏好，存 `localStorage`（不入库、不产生写流量）。
- `wrangler.toml` — R2 绑定 `BOOKMARKS`（bucket `jj-bookmark`，两份数据同 bucket 异 key）+ 静态资源 `ASSETS`（`run_worker_first`）+ Access 参数 `vars`。
- 认证 = 双层：① Cloudflare Access（Google IdP）在**边缘**按登录网关；② Worker 再校验 Access 注入的 JWT（`Cf-Access-Jwt-Assertion`：RS256 签名 + `iss`/`aud`/`exp`），堵住绕过边缘的口子。`run_worker_first` 令页面与 API 都经此校验。`CF_ACCESS_TEAM_DOMAIN`/`CF_ACCESS_AUD` 缺任一则跳过 ②（本地 dev / 未配场景）。
- 域接入 = 仅自定义域（`jj-bookmark.yigegongjiang.com` + `123.yigegongjiang.com`），均在 Cloudflare 侧挂载、不写 `routes`（GHA token 无域名权限，声明即部署失败）；新挂域名 = 临时在 `wrangler.toml` 加 `routes = [{ pattern = "...", custom_domain = true }]` → 本机 `npx wrangler deploy`（OAuth 有 zone 权限）→ 还原 toml。`workers_dev = false` + `preview_urls = false` 关闭 `*.workers.dev` 生产与 preview 域，缩小攻击面 —— 硬约束，MUST NOT 为任何理由打开；配套要求 Access destinations 也不得残留 workers.dev（见「前置」2）。

## 前置（人类在 Cloudflare 侧一次性配置）

1. 建 R2 bucket：`wrangler r2 bucket create jj-bookmark`（名字须与 `wrangler.toml` 一致，否则 deploy 校验失败）。
2. 配 Access：Zero Trust → Access controls → Applications，建一个 self-hosted 应用同时覆盖两个自定义域，IdP 选 Google，策略限定允许的邮箱 / 域。应用的 team domain 与 AUD tag 已写入 `wrangler.toml` `[vars]`（Worker 据此校验 JWT）；换应用 / 账号时同步更新这两个值。两域同应用 → AUD 一致、Worker 无需分支；各自单建应用会产生不同 AUD 导致 403。
   - destinations 只能是 `jj-bookmark.yigegongjiang.com` + `123.yigegongjiang.com`，MUST NOT 残留 `*.workers.dev`（建应用时 Cloudflare 常自动带上）：多域应用登录靠跨站 SSO，`/cdn-cgi/access/authorized` 会挑 destinations 里的某个域设 cookie，挑中 workers.dev 即 404（`workers_dev = false`），登录链断死且报 `Page not found`。
3. 配 GHA secrets（仓库 Settings → Secrets）：`CLOUDFLARE_API_TOKEN`（含 Workers + R2 编辑权限）、`CLOUDFLARE_ACCOUNT_ID`。

> 数据含内网 URL。Worker 自身校验 Access JWT，未带有效 token 一律 403；`workers.dev` 生产 + preview 域已在 `wrangler.toml` 关闭，仅自定义域可达。deploy 后即使边缘 Access 尚未覆盖某路由也不裸奔；R2 对象缺失时更只返回空库。仍 SHOULD 保持 Access 应用 + 策略在位（首要网关 + 提供 JWT）。

## 调试（本地，无需云端登录）

```bash
npm install                                                   # 装 wrangler
echo '{"version":3,"sources":{}}' > /tmp/seed.json
npx wrangler r2 object put jj-bookmark/bookmarks.json --file /tmp/seed.json --local  # 塞本地模拟 R2
npm run dev                                                   # 本地起 Worker（默认 http://localhost:8787）
```

`wrangler dev` 用本地 R2 模拟；`--local` 的 put 与 dev 共享同一持久化目录（默认 `.wrangler/`）。

## 部署

- 自动：push `master` 且改动 `jj-bookmark-web/**` → `.github/workflows/deploy-web.yml` 跑 `wrangler deploy`（亦可 workflow_dispatch 手动触发）。
- 手动：`npm run deploy`（需本机 `wrangler login` 或 `CLOUDFLARE_API_TOKEN`）。

## 数据流

- 书签预览：`CLI jj-bookmark push` → wrangler 上传 `~/.config/jj-bookmark/bookmarks.json` 到 R2 `jj-bookmark/bookmarks.json` → Worker `/api/bookmarks` 读取 → 页面渲染。
- 导航页：页面编辑 → `PUT /api/nav`（etag CAS）→ R2 `jj-bookmark/nav.json` → `GET /api/nav` 回读。web 即唯一 CRUD 入口，数据只存云端。
