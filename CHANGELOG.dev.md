```When Editing
本文档作用: 面向开发者的发版记录; CHANGELOG.md 的超集, 1:1 镜像 + 技术变更子项
遵循 AGENTS.md 文档编写规范
- 每条主项 = CHANGELOG.md 对应条目 (原文), 下方缩进子项承载技术变更
- 子项 MAY 写路径 / 函数 / 机制; ≤ 1 行
```

# Changelog (developer, follow [CHANGELOG.md](./CHANGELOG.md))

## [0.23.0] - 2026-08-19

- 作用域彻底简化为单一真实 source：`--source <NAME>` 只收真实 source 名，省略即 `default`（= `apply <URL>` 的落点）；`all` / `--all` 这类「全部 source」参数不再存在
  - `Scope` 枚举 / `includes_source` / `source_for_id` / `ALL_SCOPE` / `parse_new_source` 全部退役，`ScopeArgs::resolve() -> String`；各 `cmd_*` 直接收 `source: &str`
  - `output::print_human` 去 `show_groups`（`[source]` 分组头随之消失），`print_json` 改收 `(source, &[Bookmark])`，契约形状仍为 `{version, sources:{<source>:[...]}}`（单键）
- 全量读改为直接解析共享数据文件，`--json` 不再有程序化消费者
  - App 新增 `BookmarkStore.loadFromDisk()`（含 `version > 3` 上界校验），`CLIRunner` 只留写操作且每条命令显式带 `--source <该书签的 source>`；Raycast 用 `useCachedPromise` + `readFile` 取代 `useExec(ls --json)`
  - 依据：`store.rs` 的读侧免锁保证（原子 rename ⇒ 永远读到完整版本），且 web Worker 早已直接读同一份 JSON —— 三个消费者读法统一
- 按 ID 编辑 / 删除 / 打开只在指定 source 内生效；`mv` 恢复为只作用于目标 source
  - `not_found(source, id)` 恒带 source 名；`cmd_mv` 的 `[default(1)]` 影响面后缀移除（跨 source 扩大已不存在）
- 保存书签的内建指引不再跨 source 查重（只看默认 source）
  - `before_help` 三步全部落在默认 source（第 1 步 `ls <DOMAIN>`、第 2 步 `folders`、第 3 步 `apply`）—— 发现范围与落点一致，代价是 safari 等其他 source 的重复项不会被提示

## [0.22.0] - 2026-08-19

- 作用域参数定型为单个 `--source <NAME|all>`，省略即 `default` —— 与 `apply <URL>` 的落点是同一个默认值，读 / 写 / 改名不用分别记；要跨全部 source 时写 `--source all`（v0.21.0 的「省略 = 全部 source」已回退）
  - `ScopeArgs::resolve`：`None → Scope::Source(DEFAULT_SOURCE)`、`Some("all") → Scope::All`、其余 → `Scope::Source`
  - 调用侧改回显式跨 source：App `CLIRunner` 的 `loadAll` / `edit` / `remove` / `open` 与 Raycast `ls` / `open` 均传 `--source all`（`add` / `moveFolder` 仍传具体 source）
- `all` 成为保留值，不能再作真实 source 名；`--source all apply <URL>` 报错（新增书签必须落在单一 source）
  - `parse_new_source` 只用于 `--set-source`（拒 `all`）；`cmd_add` 对 `Scope::All` bail，`--source <NEW> apply <URL>` 仍是创建 source 的正当入口
- 保存书签的内建指引改为「`folders` 挑目录 → `apply` 存入」同为默认 source，不再混入其他 source 的旧目录体系
  - `before_help` 第 1 步查重用 `--source all ls <DOMAIN>`，第 2 步 `folders` 走默认 `default`（实机 default 29 条 vs 全部并集 93 条，safari 独有 64 条 `Brave::*` 旧体系；`ensure_leaf_placement` 只校验目标 source，异源路径会静默累积）
- `--source` 指向不存在的 source 时报错并列出已知 source（此前静默返回空结果，typo 看起来像「库里没有」）
  - `ensure_known_source` 在 `cmd_list` / `cmd_folders` 入口校验，`Scope::All` 跳过；写路径不校验

## [0.21.0] - 2026-08-19

- 作用域选项合二为一：`--all` 移除，不带 `--source` 即作用于全部 source（列表 / 搜索 / 按 ID 编辑·删除·打开都不再需要额外参数）；只想操作单个 source 时仍用 `--source <NAME>`
  - `ScopeArgs` 去 `all: bool` 与 `conflicts_with`，`resolve()` = `source.map_or(Scope::All, Scope::Source)`；`--all` 现报 unexpected argument
  - 调用侧同步去 `--all`：App `CLIRunner` 的 `loadAll` / `edit` / `remove` / `open`，Raycast `ls --json --sort visited` + `open`（`add` / `moveFolder` 仍显式传 `--source`）
  - 按 ID 全局查找安全性依据：`unique_id` 经 `Store::contains_id` 跨全部 source 排重（实机 1302 条 id 全唯一）
- `apply <URL>` 新增书签不带 `--source` 时仍存入 `default`（需唯一落点，行为不变）
  - `cmd_add` 把 `Scope::All` 收敛为 `DEFAULT_SOURCE`，替换原 `--all cannot be used when adding` 报错
- 文件夹改名 `mv` 的默认范围随之扩大到全部 source（原为仅 `default`）；输出改为列出受影响的 source 与条数
  - `cmd_mv` 返回 `(moved, Vec<String>)`，逐 source 累积 `name(count)` —— 默认全局后影响面必须自证，避免静默跨 source 改名

## [0.20.0] - 2026-08-19

- 列表与搜索合并为一个命令：`ls [KEYWORD]`，不带关键词即全量列表；原 `query` 命令移除，`query <KEYWORD>` 改用 `ls <KEYWORD>`
  - `Command::Query` 删除，`Command::Ls` 增可选位置参数 `keyword: Option<String>`（沿用 `keyword_filter` 内部分词，语义不变）；`cmd_list` 去 `jq_filter` 参数
  - `before_help` TL;DR 第 1 步改 `--all ls <DOMAIN>`；App `--all ls --json` / Raycast `--all ls --json --sort visited` 调用不变
- 移除内置 jq 过滤（原 `query --filter`）：数据文件是纯 JSON，直接用系统 `jq` 处理，能力更完整
  - 删 `src/filter.rs` + `apply_jq_filter`，Cargo 去 `jaq-core` / `jaq-std` / `jaq-json` —— 无程序化消费者（App / Raycast / web 均不用），且原实现强制输出反序列化为 `Bookmark`，弱于外部 `jq`

## [0.19.0] - 2026-08-19

- 移除网页元数据抓取：保存书签不再自动联网取标题 / 摘要 / 封面；App 新增时留空标题将保留网址原样，标题与摘要请在新增或编辑时自行填写
  - 删 `src/fetcher.rs` + `cmd_fetch` / `fetch_and_apply`，去 `fetch` 子命令与 `apply --fetch`（含 `before_help` TL;DR 配方），Cargo 去 `reqwest` + `scraper` —— CLI 自此无出站 HTTP（`push` 仍经 wrangler 子进程）
  - App 去 `CLIRunner.fetch` + `MainViewController.backgroundFetch`，新增书签不再起后台抓取任务
  - `cover` 字段保留在数据模型（App 仍解码，无渲染方），避免下次原子写静默丢弃 `bookmarks.json` 已有值

## [0.18.0] - 2026-07-27

- 导航页整页重做：卡片面板 + 搜索（`/` 聚焦、Enter 直开首个结果）+ 分组筛选 + 高频 / 最近 / 手动三态排序，暗亮双主题
  - `public/123.html` 重写：设计令牌（`--bg/--elev/--surface/--border/--accent` + `prefers-color-scheme` 双主题）、`auto-fill minmax(248px,1fr)` 卡片网格、30px 首字母色块（域名 hash 取 8 色板）、hover 浮出 ops
  - 静态 shell（header / 搜索 / 排序 / 添加）只建一次，仅 `#chips`/`#slot`/`#body` 重渲染 —— 搜索框不重建，输入焦点与光标不丢
- 导航页改为随处可编辑：卡片悬停即出编辑 / 删除，内联表单填名称·网址·分组·配色，手动排序下拖拽调链接与分组顺序、分组可就地改名；不再需要切换「编辑模式」
  - 数据 v2：`{version:2, groups:[名], links:[{name,url,group,color,createdAt}]}`，links 扁平且顺序 = 手动排序，`groups` 只记展示顺序；客户端 `migrate()` 兼容读 v1（`groups:[{name,links}]`）后写回 v2，`sanitizeNav()` 只收 v2（新增 group/color/createdAt 白名单校验，`links ≤ 1000`，落库前剪掉空分组）
  - HTML5 DnD：grip 覆盖在色块上（仅手动态渲染），drop 落在 cell = 插到其前、落在 grid 空白 = 追加到该组、落在 section = 分组整体重排；`moveLink()` 换算全局下标（`before > from` 时 -1）
- 导航页按本机点击次数排「高频」，次数只留在本浏览器，不上传
  - `localStorage` `jj-nav-clicks`（click + 中键 auxclick 计数，load 时按现存 URL 剪枝）、`jj-nav-sort` 记排序态；避免每次点击都 PUT 整份文档触发 etag CAS 抖动

## [0.17.1] - 2026-07-27

- 修复 `123.yigegongjiang.com` 登录后跳到「Page not found」、始终进不去导航页
  - 根因：Access 应用 destinations 残留 `jj-bookmark.fan-yang2019.workers.dev` + `123.fan-yang2019.workers.dev`，多域跨站 SSO 的 `/cdn-cgi/access/authorized` 挑中 workers.dev 设 cookie，而 `workers_dev = false` 下该域无路由 → 404 断链
  - 修：Access destinations 只留两个自定义域（AUD / 策略不变）；`wrangler.toml` + README 记入硬约束——`workers_dev` MUST NOT 打开，destinations MUST NOT 残留 `*.workers.dev`

## [0.17.0] - 2026-07-27

- 新增个人导航页（hao123 式）：`123.yigegongjiang.com`（或预览域 `/123`）直达，分组 + 链接在页面内直接增删改、拖拽排序、自动保存；数据只存云端，Google 登录访问
  - `jj-bookmark-web/public/123.html` 单文件页：浏览 / 编辑双模式、HTML5 DnD 排序（组内 / 跨组 / 组间）、600ms 防抖自动 PUT、`beforeunload` 护栏、URL 自动补 scheme + 空名回填 hostname
  - `src/index.js` `/api/nav` GET/PUT R2 `nav.json`：PUT 服务端字段白名单重建 + `If-Match` → R2 `onlyIf.etagMatches` 条件写，失配 409 → 客户端提示重载；host = `123.yigegongjiang.com` 时非 API 路径一律渲染 `/123` 资源，请求无 Access JWT（该域未被 Access 应用覆盖）则 302 主域 `/123` 走既有登录网关，覆盖后自动本域直出
  - 自定义域挂载：临时 `routes = [{ pattern, custom_domain = true }]` + 本机 `npx wrangler deploy` 一次性完成后还原 toml（GHA token 无域名权限，routes 不入库）；Access 复用既有应用加 hostname（AUD 不变，人类一次性配置）

## [0.16.0] - 2026-07-22

- App / Web 书签列表在标题与网址下多显示摘要与备注，更易辨认目标书签
  - App `BookmarkCellView` 加 excerpt / note 单行截断 label（空则 `isHidden` 塌陷），`MainViewController.tableView(_:heightOfRow:)` = 46 + 各存在项 17 变行高（单行 → 无需测量文本）；excerpt/note 复用 subtitle 的 selected 态配色，全文走 `toolTip`，note 加 `✎` 前缀区分
  - Web `index.html` bookmarkRow 在 `.sub` 下追加 `.detail` / `.detail.note`（仅非空 render，`-webkit-line-clamp: 2` 换行截断）
- Raycast 默认展开详情面板（标题 / 网址 / 摘要 / 备注 + 元信息），⌘Y 切回紧凑列表
  - `search-bookmarks.tsx` `isShowingDetail` 默认 `true` + `List.Item.Detail`（markdown title/url/excerpt/note + Metadata folder/source/tags/favorite/visited）；detail 开启时窄列隐藏 accessories，故一眼信息迁入 Metadata，subtitle 亦仅紧凑态显示；⌘Y = `Common.ToggleQuickLook`

## [0.15.0] - 2026-07-22

- folder 层级分隔符由 `A / B`（空格斜杠）改为 `A::B`（无空格），避免 AI 保存时丢空格致层级错位；本机数据已自动迁移
  - 单点常量 `"::"`：CLI `model::FOLDER_SEP` / App `folderSep` / web `FOLDER_SEP`
  - 改造点：`query::folder_filter`、`is_ancestor`、`cmd_mv` 前缀拼接、Swift `FolderTree` split/join + hasPrefix、web `buildTree`/`subtreeCount`/子树过滤
  - AI-facing 文本：`--help` before_help + `apply --folder` 说明显式声明 `::`
  - 本机 `bookmarks.json` 经 jq `gsub(" / "; "::")` 一次性迁移（1227 folder），无 load 时兼容逻辑

## [0.14.1] - 2026-07-21

- CLI 保存指引先按 URL 域名检查已有书签；同域命中时先选择新增或编辑
  - `--help` TL;DR 先执行 `--all query <DOMAIN>`；不暴露默认 source

## [0.14.0] - 2026-07-21

- 新增 Raycast 扩展（本地开发用，不含在发布包内）：在 Raycast 里搜索 / 打开书签
  - `raycast/`（TS + @raycast/api）：`useExec` 调 CLI `ls --all --json` load-once + 内存 keyword 过滤（复刻 query.rs），`open` 走 CLI `open <id>`
  - 列表行改版：title=标题、subtitle=URL（原为 folder）、folder / source / favorite / 最近访问置右侧 accessory

## [0.13.1] - 2026-07-21

- App 打开书签后自动隐藏窗口，切至浏览器无需手动切走
  - `MainViewController.openSelected` 成功打开后调用 `NSApp.hide(nil)`；失败弹错时不隐藏

## [0.13.0] - 2026-07-21

- 书签只能挂到叶子文件夹：路径 `a / b / c` 只能挂 `c`，不能挂 `a` / `b`；挂到非叶文件夹时保存 / 移动会被拒绝
  - 约束语义：同一 source 内被占用的非空 `folder` 路径构成 antichain（无占用路径是另一占用路径的严格前缀祖先，分隔符 `" / "`）；单点校验 `ensure_leaf_placement`（`main.rs`）
  - 强制点 = CLI `apply`（`cmd_add` 新增、`cmd_edit` 换 source/folder 时）+ `cmd_mv`（前缀替换后校验每个落点 folder）；违反即 `bail!` 非零退出、`mutate` 不写入
  - targeted 校验：仅在放置 / 移动 folder 时触发，纯字段编辑与既有脏数据不追溯（无历史迁移）
- 未分类书签（`folder == ""`）不受此约束，可照常保存
  - `ensure_leaf_placement` 空 folder 直接放行（策略 A）；翻转为策略 B（必须有 folder）须新增对空 folder 的显式 `bail!`（删 guard 行为无变化，空串已被 `is_ancestor` 短路恒放行）
  - App `newBookmark` 仅在选中叶子 folder 节点时预填其路径（`children.isEmpty`），避免预填非叶路径被 CLI 拒绝

## [0.12.0] - 2026-07-17

- CLI 用 `apply <URL|ID>` 统一新增 / 编辑，`apply <ID> --delete` 删除；`--source` / `--all` 仅为根选项
  - clap root flatten scope，不向子命令传播参数；`apply` 按 URL / ID 分派并以 Added / Updated / Deleted 返回值区分结果
- source 改为数据与 App / 网页侧栏的第一层分组，不再在每条书签内重复保存
  - schema v3 = `{version,sources:{name:[bookmark]}}`，兼容读取 v1/v2；App / Web 解码后注入 source 供侧栏与搜索使用
- App 安装 CLI 改为符号链接，随 App 更新自动使用同版本内嵌 CLI
  - `CLIInstaller` 用 `createSymbolicLink` 替代复制；识别旧文件 / 失效链接并提示替换，Settings 重装直接重建链接
- App 重启后保持侧栏 source / folder 的展开状态与选中路径
  - `UserDefaults` 持久化 `FolderNode.stateKey` 集合与当前选择；展开 / 折叠 / 选择时即时写入，加载时恢复并清理失效路径
- 删除 `add` / `edit` / `rm` / `help` 子命令；帮助仅使用 `-h` / `--help`
  - App 全部写操作改走 `apply`；clap 禁用 help subcommand

## [0.11.0] - 2026-07-17

- 书签支持 source 分组：CLI 默认仅操作 `default`，可用 `--source <NAME>` / `--all` 切换，`sources` 查看统计
  - schema v2 每条新增 `source`（旧数据缺字段兜底 `default`）；所有读写命令共享 source scope，`add --source` 定向新增，`edit --set-source` 跨 source 移动，人类可读 `--all` 输出显示 source
- App 与网页版默认显示全部 source
  - App 的加载 / 编辑 / 删除 / 打开 / 抓取 / folder 移动显式走 CLI `--all`；App/Web 搜索纳入 `source`，Web 空库契约升至 v2
- 删除一次性的 raindrop CSV `import` 子命令
  - 删除 `importer.rs` / `csv` 依赖 / 仅服务导入的时间解析，CLI 回归核心读写路径

## [0.10.0] - 2026-07-17

- macOS 通用版：一份安装包同时支持 Apple Silicon 与 Intel Mac
  - `jj-bookmark-app/package.sh` 重写为 `[host|universal]` 双模式恒 Release：host=按 `uname -m` 单架构（`install-local.sh` 用）；universal=Rust `rustup target add aarch64-apple-darwin x86_64-apple-darwin` 双 target build + `lipo -create` 合并 CLI + xcodebuild `-destination 'generic/platform=macOS' ARCHS="arm64 x86_64" ONLY_ACTIVE_ARCH=NO`（CI 用）；`release.yml` 改跑 `package.sh universal`，产物 `jj-bookmark-macos-universal.zip` / `jj-bookmark-cli-macos-universal`（后者直接从 bundle `Contents/Helpers/jj-bookmark` 拷贝，已 lipo 合并）；脚本末尾 `lipo -archs` 打印双 slice 校核；同时移除脚本 `[release|debug]` 分支恒 Release（本地调试用 `swift build` / `cargo run`）
- 关闭 `*.workers.dev` 网页域名（含 Public 预览域），仅经自定义域访问
  - `jj-bookmark-web/wrangler.toml` 加 `workers_dev = false` + `preview_urls = false`（已上自定义域 `jj-bookmark.yigegongjiang.com`；即使 Worker 侧 JWT 校验存在也一并关，缩小攻击面）；同步 `jj-bookmark-web/README.md` + `workflow.md` 里 *.workers.dev 相关措辞

## [0.9.0] - 2026-07-17

- 新增 `jj-bookmark push`：把本地书签单向同步到网页版，浏览器里随处只读查看
  - `pusher.rs`：经 wrangler `r2 object put <bucket>/<key> --file … --content-type application/json --remote` 上传数据文件到固定 R2（bucket `jj-bookmark`/key `bookmarks.json`，常量、无 env）；wrangler 解析 PATH 有则用、否则 `npx wrangler`；上传前 `read_store` 解析校验，损坏不推
- 网页版：只读预览页，支持文件夹树、多关键词搜索与排序；经 Google 登录访问
  - `jj-bookmark-web/`：Cloudflare Worker（`src/index.js`）读 R2 出 `/api/bookmarks`（对象缺失兜底空库），静态 `public/index.html` 单文件 SPA 仿 App 只读浏览（folder 子树过滤 / 多词搜 / created·updated·visited·title 排序 / eTLD+1 域名高亮启发式，非 PSL）；`wrangler.toml` 绑定 R2+Assets；GHA `deploy-web.yml`（push master + `jj-bookmark-web/**` / workflow_dispatch，`cloudflare/wrangler-action`）；认证双层：Cloudflare Access(Google) 边缘网关 + Worker 校验 `Cf-Access-Jwt-Assertion`（Web Crypto RS256 + iss/aud/exp，`run_worker_first` 令页面与 API 都受控，堵 `*.workers.dev` 直连），team domain/AUD 存 `wrangler.toml [vars]`（非机密），缺则跳过仅靠边缘
  - 移除 CLI 遗留 env 钩子 `JJ_BOOKMARK_DIR`（`Paths::resolve` 固定 `~/.config/jj-bookmark`，测试用 `Paths::from_dir` 直连），承接 0.4.3 去测试门控 env 的方向

## [0.8.0] - 2026-07-16

- 左侧文件夹栏默认宽度缩小；手动调整后下次启动沿用
  - 默认宽度 240→200；NSSplitViewDelegate resize 回调写 UserDefaults，启动恢复并按两栏最小宽约束裁剪

## [0.7.0] - 2026-07-16

- `--help` 顶部直接给出添加流程，可先列出已有文件夹路径再保存书签
  - 新增 `folders` 去重排序输出非空路径；`before_help` 串联 `folders` → `add`

## [0.6.0] - 2026-07-16

- 书签列表显示完整网址，仅用红色突出可注册主域名，便于快速定位站点
  - `BookmarkCellView` 用 attributed string 仅高亮 eTLD+1；`swift-psl` 处理 `co.jp` 等多段公共后缀；打包脚本携带 SwiftPM resource bundle

## [0.5.0] - 2026-07-16

- 搜索支持空格分隔的多关键词，可分别命中标题、网址、描述、备注、文件夹和标签
  - App/CLI 统一 Unicode 空白分词 + 大小写不敏感 AND 匹配；补多词跨字段与全字段回归测试

## [0.4.3] - 2026-07-16

- 跟随版本同步发布
  - 移除 App 无头自检能力：删 `scripts/verify-app.sh` + 源码内所有测试门控 env 钩子（`JJ_BOOKMARK_DUMP_L10N`/`DUMP_WINDOW`/`OPEN_SETTINGS`/`DUMP_LAYOUT`/`AUTOEXIT_SECONDS`/`NO_INSTALL`/`DIR`）；App 数据目录固定 `~/.config/jj-bookmark`（AppPaths 不再读 `JJ_BOOKMARK_DIR`，杜绝自检指向空目录导致的空白假象）；`install-local.sh` 交付闸改为仅打包+装 /Applications；仅保留 `JJ_BOOKMARK_CLI`（dev 从源码定位 CLI，正式包内死代码）；无用户可感知变化

## [0.4.2] - 2026-07-16

- 跟随版本同步发布
  - 本机发布链路：`scripts/install-local.sh`(package release → `ditto` 装 /Applications → 自检) + `scripts/verify-app.sh`(无头断言内嵌 CLI 版本/启动/i18n/设置窗口/自动退出, WindowServer 不可达则降级跳过 GUI 层); workflow.md 发布改以本机安装+自检为交付闸, GHA 转 fire-and-forget

## [0.4.1] - 2026-07-16

- 修复某些情况下主窗口右侧书签列表不显示（左侧文件夹栏占满整个窗口）
  - NSSplitView 右栏初始 frame 宽 0，首次按比例分配被永久压成 0；加 NSSplitViewDelegate(左栏 shouldAdjustSizeOfSubview 固定 + constrainMin/Max 两栏最小宽) + viewDidAppear setPosition 落定初始分隔位置 + 右栏补非零初始宽; env JJ_BOOKMARK_DUMP_LAYOUT 无头自检

## [0.4.0] - 2026-07-16

- 记住主窗口尺寸；每次在鼠标所在的屏幕居中打开（多显示器友好）
  - AppDelegate 手动持久化 content 尺寸到 UserDefaults(避开 setFrameAutosaveName/系统状态恢复冲突; 存 content 尺寸防标题栏高度漂移); launchContentSize() clamp 到鼠标屏 visibleFrame; windowDidResize/WillClose 落盘; isRestorable=false; env JJ_BOOKMARK_DUMP_WINDOW 自检

## [0.3.0] - 2026-07-16

- App 多语言界面：中文 / 日文 / 英文，随系统语言自动切换（非中日语系默认英文）
  - 纯 Swift 本地化表 `L10n`(`nonisolated`, `Locale.preferredLanguages` 判定, 英文兜底); 无 .lproj/NSLocalizedString/package.sh 改动; `Info.plist` 加 `CFBundleDevelopmentRegion=en`; env `JJ_BOOKMARK_DUMP_L10N` 无头自检
- 命令行工具输出改为英文
  - clap help + 运行时/错误字符串全量英译; 保留 `#<id>` 连续格式(App add 解析依赖); 注释/测试数据不动

## [0.2.0] - 2026-07-16

- 偏好设置窗口（⌘,）：集中管理自动退出、命令行工具安装 / 重装、检查更新
  - 纯代码 `SettingsWindowController`(NSStackView)；AppDelegate 强持有(否则窗口即释放)；菜单项 target=NSApp.delegate 走响应链；`CLIInstaller` 暴露 `reinstall`/`installedVersion`；更新=打开 Releases 页
- 闲置自动退出：打开链接后忘记关闭时到点自动退出；默认 1 分钟，可选 1 / 5 / 10 / 自定义
  - `AutoExitManager` idle-timer 启动即 arm；local 事件监听重置；`beginActivity(.userInitiatedAllowingIdleSystemSleep)` 防 App Nap 后台节流；`.common` runloop；模态/sheet 时推迟；env `JJ_BOOKMARK_AUTOEXIT_SECONDS` 便于测试

## [0.1.2] - 2026-07-16

- 跟随版本同步发布
  - 精简用户文案：App 安装/更新对话框次按钮「以后再说」→「取消」；README 命令列表改中文功能动词（去 save/delete 等非真实命令名）；无行为变更

## [0.1.1] - 2026-07-16

- 跟随版本同步发布
  - 精简面向用户文案：CLI `--help` about + README/CHANGELOG 一行简介去宣传修饰；无行为变更

## [0.1.0] - 2026-07-16

- 书签工具：命令行 `jj-bookmark` 与 macOS App，共享一份 JSON 数据文件
  - CLI(Rust)=唯一核心；App(Swift/AppKit,SwiftPM) bundle 内嵌同版本 CLI 经 `Process` 调用；单一 `VERSION` 源
- 保存 / 编辑 / 删除 / 打开 / 查询书签；按添加·编辑·访问时间与名称多维排序
  - 读写协议：原子写(tmp+fsync+rename)+独立 lock 文件 flock+锁内重读+.bak+容错读；排序次级键 id desc
- 保存 URL 自动抓取标题 / 描述 / 封面；从 raindrop CSV 批量导入
  - `reqwest`+`scraper` 抓 og:*/`<title>`（网络在锁外）；`csv` 解析，ISO8601 UTC→epoch ms + JST(+9h) 派生
- App 三栏浏览 + 文件夹树 + 即时搜索；终端改动后无需重启即刷新
  - FSEvents 监听目录(非 inode)+去抖合并；刷新按稳定 id/path 保留选中·展开·滚动
- 查询：内嵌 jq 引擎的 `--filter`，数据文件也可直接用 `jq` 处理
  - `jaq`(纯 Rust,in-process)驱动 `--filter`；关键词模糊搜与四键排序走原生比较

[0.2.0]: https://github.com/yigegongjiang/jj-bookmark/releases/tag/v0.2.0
[0.1.2]: https://github.com/yigegongjiang/jj-bookmark/releases/tag/v0.1.2
[0.1.1]: https://github.com/yigegongjiang/jj-bookmark/releases/tag/v0.1.1
[0.1.0]: https://github.com/yigegongjiang/jj-bookmark/releases/tag/v0.1.0
