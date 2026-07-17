```When Editing
本文档作用: 面向开发者的发版记录; CHANGELOG.md 的超集, 1:1 镜像 + 技术变更子项
遵循 AGENTS.md 文档编写规范
- 每条主项 = CHANGELOG.md 对应条目 (原文), 下方缩进子项承载技术变更
- 子项 MAY 写路径 / 函数 / 机制; ≤ 1 行
```

# Changelog (developer, follow [CHANGELOG.md](./CHANGELOG.md))

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
