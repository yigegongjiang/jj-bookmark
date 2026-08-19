# jj-bookmark-web

一个 Worker、**一份数据**（R2 `bookmarks.json`）、两张页：① `/` 全库列表——仿 App 浏览页（folder 树 + 搜索 + 排序）；② `/123?f=<folder>` folder 卡片视图——缺省 folder `123`，即个人导航页（`123.yigegongjiang.com` 亦指向它）。两页都可内联增删改，与本地经 CLI `jj-bookmark sync` 双向收敛。

## 架构

- `src/index.js` — Worker：`/api/bookmarks` GET/PUT `bookmarks.json`（GET 带 `ETag` 响应头、对象缺失兜底空库；PUT 服务端字段白名单重建 + `If-Match`→R2 `onlyIf.etagMatches`，失配 409；缺 `If-Match` 时先 `head`，对象已存在即 409，防无条件写盖掉另一端刚写的内容）；host = `123.yigegongjiang.com` 时任意非 API 路径渲染卡片视图（请求不带 Access JWT = 该域尚未被 Access 应用覆盖 → 302 主域 `/123` 走既有登录网关，覆盖后自动本域直出）；其余路由走静态资源。
- `public/index.html` — 全库列表单文件 SPA（内联 CSS/JS）：拉 `/api/bookmarks` 后内存过滤 / 排序；hover 出编辑·删除，`<dialog>` 表单增改，删除写 `deleted` 墓碑；侧栏每个具名 folder 挂 `↗` 跳到该 folder 的卡片视图。
- `public/123.html` — folder 卡片视图单文件 SPA：root folder 取自 `?f=`（缺省 `123`），筛出 folder 为 `<root>` 或 `<root>::*` 的条目，按相对路径分组成区；搜索 / 分组 chip / 排序（分组·最近·名称）/ 增删改 / 分组重命名（= folder 前缀替换）。新链接跟随该 folder 既有条目的 source。
- 两页共用写协议：**页面持有完整 STORE**（含其他 folder 与墓碑），每次改动整份 PUT，成功后重新 GET（状态直接取自服务端，既不重算派生字段也顺带吸收 CLI 期间的写入），409 提示「未保存」并重载。写回丢掉墓碑 = 告诉另一端「这些条目从未存在」，下次合并即复活。无构建步骤。
- 卡片视图没有独立存储：分组 = folder 相对 root 的路径，卡片配色由域名散列而来（书签模型无 `color`），未分组落 `<root>::未分组`（root 下尚无子文件夹时直接落 `<root>`，避免破坏叶子挂载约束）。原 `nav.json` 的手动拖拽顺序 / 点击次数排序 / 自选配色随其一并退役 —— 书签模型不为它们加字段。
- `wrangler.toml` — R2 绑定 `BOOKMARKS`（bucket `jj-bookmark`，唯一对象 `bookmarks.json`）+ 静态资源 `ASSETS`（`run_worker_first`）+ Access 参数 `vars`。
- 认证 = 双层：① Cloudflare Access（Google IdP）在**边缘**按登录网关；② Worker 再校验 Access 注入的 JWT（`Cf-Access-Jwt-Assertion`：RS256 签名 + `iss`/`aud`/`exp`），堵住绕过边缘的口子。`run_worker_first` 令页面与 API 都经此校验。`CF_ACCESS_TEAM_DOMAIN`/`CF_ACCESS_AUD` 缺任一则跳过 ②（本地 dev / 未配场景）。
- 域接入 = 仅自定义域（`jj-bookmark.yigegongjiang.com` + `123.yigegongjiang.com`），均在 Cloudflare 侧挂载、不写 `routes`（GHA token 无域名权限，声明即部署失败）；新挂域名 = 临时在 `wrangler.toml` 加 `routes = [{ pattern = "...", custom_domain = true }]` → 本机 `npx wrangler deploy`（OAuth 有 zone 权限）→ 还原 toml。`workers_dev = false` + `preview_urls = false` 关闭 `*.workers.dev` 生产与 preview 域，缩小攻击面 —— 硬约束，MUST NOT 为任何理由打开；配套要求 Access destinations 也不得残留 workers.dev（见「前置」2）。

## 前置（人类在 Cloudflare 侧一次性配置）

1. 建 R2 bucket：`wrangler r2 bucket create jj-bookmark`（名字须与 `wrangler.toml` 一致，否则 deploy 校验失败）。
2. 配 Access：Zero Trust → Access controls → Applications，建一个 self-hosted 应用同时覆盖两个自定义域，IdP 选 Google，策略限定允许的邮箱 / 域。应用的 team domain 与 AUD tag 已写入 `wrangler.toml` `[vars]`（Worker 据此校验 JWT）；换应用 / 账号时同步更新这两个值。两域同应用 → AUD 一致、Worker 无需分支；各自单建应用会产生不同 AUD 导致 403。
   - destinations 只能是 `jj-bookmark.yigegongjiang.com` + `123.yigegongjiang.com`，MUST NOT 残留 `*.workers.dev`（建应用时 Cloudflare 常自动带上）：多域应用登录靠跨站 SSO，`/cdn-cgi/access/authorized` 会挑 destinations 里的某个域设 cookie，挑中 workers.dev 即 404（`workers_dev = false`），登录链断死且报 `Page not found`。
3. 配 GHA secrets（仓库 Settings → Secrets）：`CLOUDFLARE_API_TOKEN`（含 Workers + R2 编辑权限）、`CLOUDFLARE_ACCOUNT_ID`。
4. 建 Access service token 供 CLI `sync` 用：Zero Trust → Access → Service Auth → 建 token，并给上面那个应用**加一条 action = Service Auth 的策略**（缺这条策略 Access 会去要 IdP 登录，CLI 表现为 302 到登录页）。token 写入本机 `~/.config/jj-bookmark/credentials.json`：`{"client_id": "<...>.access", "client_secret": "<...>"}`，然后 `chmod 600`（权限不对 CLI 直接拒绝运行）。
   - 该策略是**应用级**的：token 覆盖该应用的全部路由。单人自用可接受；要隔离须拆独立 Access 应用，但 AUD 不同、Worker 需加分支。

> 数据含内网 URL，且 web 现在可写。Worker 自身校验 Access JWT，未带有效 token 一律 403；`workers.dev` 生产 + preview 域已在 `wrangler.toml` 关闭，仅自定义域可达。deploy 后即使边缘 Access 尚未覆盖某路由也不裸奔；R2 对象缺失时更只返回空库。仍 SHOULD 保持 Access 应用 + 策略在位（首要网关 + 提供 JWT）。

## 调试（本地，无需云端登录）

```bash
npm install                                                   # 装 wrangler
echo '{"version":4,"sources":{}}' > /tmp/seed.json
npx wrangler r2 object put jj-bookmark/bookmarks.json --file /tmp/seed.json --local  # 塞本地模拟 R2
npm run dev                                                   # 本地起 Worker（默认 http://localhost:8787）
```

`wrangler dev` 用本地 R2 模拟；`--local` 的 put 与 dev 共享同一持久化目录（默认 `.wrangler/`）。

## 部署

- 自动：push `master` 且改动 `jj-bookmark-web/**` → `.github/workflows/deploy-web.yml` 跑 `wrangler deploy`（亦可 workflow_dispatch 手动触发）。
- 手动：`npm run deploy`（需本机 `wrangler login` 或 `CLOUDFLARE_API_TOKEN`）。

## 数据流

- 本地文件与 R2 `jj-bookmark/bookmarks.json` 双向同步——`jj-bookmark sync` 走 `GET`（取 `ETag`）→ 按 `id` 记录级 LWW 合并 → `PUT` 条件写；两张页面走同一套 API、同一个对象。协议 / 合并规则 / 已接受的妥协见 data-model §12。
