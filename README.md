```When Editing
本文档作用: 工程总览 (价值主张 / 使用 / 架构 / 结构); MUST NOT 写发布流程 (→ workflow.md) / LLM 约束 (→ AGENTS.md)
遵循 AGENTS.md 文档编写规范
- 章节按需增删, 只留项目真有的; 首行一行价值主张, MUST NOT 带 LLM 提示
- 短并列项用表格; 可执行步骤 fenced + `#` 注释同行
- NEVER 写「开发」段 (VibeCoding 不向人类解释 dev 命令)
```

# jj-bookmark

书签工具：Rust CLI (`jj-bookmark`) 为唯一核心，macOS App (Swift + AppKit) 为其 GUI 前端，共享一份 JSON 数据文件；可选 web 只读镜像（Cloudflare）。

## 使用

- CLI：`jj-bookmark-cli/` 构建出二进制 `jj-bookmark`；`apply <URL|ID>` 统一保存 / 编辑，`apply <ID> --delete` 删除，`ls [KEYWORD]` 统一列表 / 搜索；根选项只有 `--source <NAME>`，每条命令恒作用于单一 source，省略即 `default`（= `apply <URL>` 的落点）；其他命令见 `jj-bookmark --help`（仅英文）。
- App：`jj-bookmark-app/` 由 `package.sh` 组装出 macOS `.app`，桌面端全量浏览 / 编辑，记住窗口、左侧栏尺寸及展开 / 选中路径；bundle 内嵌同版本 CLI 作运行核心，`~/.local/bin/jj-bookmark` 可选安装为指向它的符号链接。偏好设置（⌘,）含闲置自动退出（默认 1 分钟，可选 1 / 5 / 10 / 自定义）、CLI 安装·重装、检查更新。界面随系统语言切换（中 / 日 / 英，非中日语系默认英文）。
- Web：`jj-bookmark-web/` = Cloudflare Worker + R2，两张页：只读预览页（folder 树 / 搜索 / 排序，数据由 `jj-bookmark push` 单向推送，web 无写入 / 无 pull）+ 个人导航页 `123.yigegongjiang.com`（卡片面板：搜索 / 分组筛选 / 高频·最近·手动排序 / 拖拽 / 内联增删改，R2 为唯一数据源）；均 Google 登录访问。详见 [jj-bookmark-web/README.md](./jj-bookmark-web/README.md)。
- 数据文件：`~/.config/jj-bookmark/bookmarks.json`（pretty JSON；顶层 `sources` 分组，条目不重复保存 source；可手改 / `jq` 处理）。

## 架构

- **CLI = 写侧唯一核心**：读写协议（锁 / 原子写 / 校验）/ 关键词搜索 / 排序，只在 Rust CLI 实现一遍；每条命令作用于单一 source（`--source`，默认 `default`）。
- **App = CLI 的 GUI 前端**：`.app` 内嵌同版本 `jj-bookmark`（`Contents/Helpers/`），写操作经 `Process` 调用它（每次显式带该书签的 source）；全量读直接解析共享 JSON 文件（原子 rename 保证读到完整版本），即时搜索 / 排序 / folder 树 / FSEvents 监听为 App 原生逻辑。
- **两个集成面**：共享 JSON 文件格式（App / Raycast / Web 的读来源）+ CLI `--json` 输出（同构形状，供 `jq` / 脚本）。无 FFI / 无共享库 / 无后台常驻。
- **Web = 只读镜像 + 独立导航页**：CLI `push` 把数据文件整份上传 R2；Worker 读同一 JSON 出 preview page，client 侧内存过滤 / 排序（仿 App）。单向：数据源恒为本地文件，web 不回写。导航页与书签数据无关：R2 `nav.json`（v2 扁平 links + 分组顺序）为唯一数据源，页面内 CRUD；点击次数只存浏览器 localStorage。
- 技术：CLI = Rust（`clap` + `serde_json`；无 jq 引擎，数据文件直接用外部 `jq` 处理）；App = Swift + AppKit 纯源码（无 SwiftUI / Storyboard / xib，SwiftPM executable + 模板 `Info.plist`）。
- 读写安全：原子写（tmp + fsync + rename）+ 独立 lock 文件 `flock` + `.bak` + 容错读；App 侧 FSEvents 监听目录刷新（协议见 data-model §6）。

## 项目结构

- `VERSION` — 单一版本源；`scripts/set-version.sh` 据此写 CLI `Cargo.toml`，`package.sh` 注入 App `Info.plist`
- `scripts/` — 构建脚本（`set-version.sh`）
- `jj-bookmark-cli/` — Rust CLI（cargo），产物二进制 `jj-bookmark`（唯一核心）
- `jj-bookmark-app/` — macOS App（Swift + AppKit，SwiftPM）+ `package.sh`（组装 `.app`、内嵌 CLI）
- `jj-bookmark-web/` — Cloudflare Worker（JS）+ 静态只读 preview page + R2 绑定；`push` 目标 + web 部署（GHA）
- `raycast/` — Raycast extension（TS，dev-only，NEVER 上架）：query + open，经 CLI `ls --json` / `open`；详见 [raycast/README.md](./raycast/README.md)
