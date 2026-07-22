```When Editing
本文档作用: 面向使用者的发版记录; 只写用户感受得到的变化, MUST NOT 写技术细节 (→ CHANGELOG.dev.md)
遵循 AGENTS.md 文档编写规范
- 写: 新功能 / 行为修复 / 体验 / 安全 / 命令迁移
- MUST NOT 写: 文件路径 / 函数名 / 组件名 / 依赖包名 / 重构细节
- 单条 ≤ 2 行, 单版本 ≤ 5 条; 段落: Added / Changed / Fixed / Removed / Security
- 无用户可感知变化 → 占位: `跟随版本同步发布`
```

# Changelog

[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) + [SemVer](https://semver.org/).

## [0.16.0] - 2026-07-22

### Changed

- App / Web 书签列表在标题与网址下多显示摘要与备注，更易辨认目标书签
- Raycast 默认展开详情面板（标题 / 网址 / 摘要 / 备注 + 元信息），⌘Y 切回紧凑列表

## [0.15.0] - 2026-07-22

### Changed

- folder 层级分隔符由 `A / B`（空格斜杠）改为 `A::B`（无空格），避免 AI 保存时丢空格致层级错位；本机数据已自动迁移

## [0.14.1] - 2026-07-21

### Changed

- CLI 保存指引先按 URL 域名检查已有书签；同域命中时先选择新增或编辑

## [0.14.0] - 2026-07-21

### Added

- 新增 Raycast 扩展（本地开发用，不含在发布包内）：在 Raycast 里搜索 / 打开书签

## [0.13.1] - 2026-07-21

### Changed

- App 打开书签后自动隐藏窗口，切至浏览器无需手动切走

## [0.13.0] - 2026-07-21

### Changed

- 书签只能挂到叶子文件夹：路径 `a / b / c` 只能挂 `c`，不能挂 `a` / `b`；挂到非叶文件夹时保存 / 移动会被拒绝
- 未分类书签（不填文件夹）不受此约束，可照常保存

## [0.12.0] - 2026-07-17

### Changed

- CLI 用 `apply <URL|ID>` 统一新增 / 编辑，`apply <ID> --delete` 删除；`--source` / `--all` 仅为根选项
- source 改为数据与 App / 网页侧栏的第一层分组，不再在每条书签内重复保存
- App 安装 CLI 改为符号链接，随 App 更新自动使用同版本内嵌 CLI
- App 重启后保持侧栏 source / folder 的展开状态与选中路径

### Removed

- 删除 `add` / `edit` / `rm` / `help` 子命令；帮助仅使用 `-h` / `--help`

## [0.11.0] - 2026-07-17

### Added

- 书签支持 source 分组：CLI 默认仅操作 `default`，可用 `--source <NAME>` / `--all` 切换，`sources` 查看统计
- App 与网页版默认显示全部 source

### Removed

- 删除一次性的 raindrop CSV `import` 子命令

## [0.10.0] - 2026-07-17

### Added

- macOS 通用版：一份安装包同时支持 Apple Silicon 与 Intel Mac

### Security

- 关闭 `*.workers.dev` 网页域名（含 Public 预览域），仅经自定义域访问

## [0.9.0] - 2026-07-17

### Added

- 新增 `jj-bookmark push`：把本地书签单向同步到网页版，浏览器里随处只读查看
- 网页版：只读预览页，支持文件夹树、多关键词搜索与排序；经 Google 登录访问

## [0.8.0] - 2026-07-16

### Changed

- 左侧文件夹栏默认宽度缩小；手动调整后下次启动沿用

## [0.7.0] - 2026-07-16

### Added

- `--help` 顶部直接给出添加流程，可先列出已有文件夹路径再保存书签

## [0.6.0] - 2026-07-16

### Changed

- 书签列表显示完整网址，仅用红色突出可注册主域名，便于快速定位站点

## [0.5.0] - 2026-07-16

### Changed

- 搜索支持空格分隔的多关键词，可分别命中标题、网址、描述、备注、文件夹和标签

## [0.4.3] - 2026-07-16

跟随版本同步发布

## [0.4.2] - 2026-07-16

跟随版本同步发布

## [0.4.1] - 2026-07-16

### Fixed

- 修复某些情况下主窗口右侧书签列表不显示（左侧文件夹栏占满整个窗口）

## [0.4.0] - 2026-07-16

### Added

- 记住主窗口尺寸；每次在鼠标所在的屏幕居中打开（多显示器友好）

## [0.3.0] - 2026-07-16

### Added

- App 多语言界面：中文 / 日文 / 英文，随系统语言自动切换（非中日语系默认英文）

### Changed

- 命令行工具输出改为英文

## [0.2.0] - 2026-07-16

### Added

- 偏好设置窗口（⌘,）：集中管理自动退出、命令行工具安装 / 重装、检查更新
- 闲置自动退出：打开链接后忘记关闭时到点自动退出；默认 1 分钟，可选 1 / 5 / 10 / 自定义

## [0.1.2] - 2026-07-16

跟随版本同步发布

## [0.1.1] - 2026-07-16

跟随版本同步发布

## [0.1.0] - 2026-07-16

### Added

- 书签工具：命令行 `jj-bookmark` 与 macOS App，共享一份 JSON 数据文件
- 保存 / 编辑 / 删除 / 打开 / 查询书签；按添加·编辑·访问时间与名称多维排序
- 保存 URL 自动抓取标题 / 描述 / 封面；从 raindrop CSV 批量导入
- App 三栏浏览 + 文件夹树 + 即时搜索；终端改动后无需重启即刷新
- 查询：内嵌 jq 引擎的 `--filter`，数据文件也可直接用 `jq` 处理

[0.11.0]: https://github.com/yigegongjiang/jj-bookmark/releases/tag/v0.11.0
[0.10.0]: https://github.com/yigegongjiang/jj-bookmark/releases/tag/v0.10.0
[0.9.0]: https://github.com/yigegongjiang/jj-bookmark/releases/tag/v0.9.0
[0.2.0]: https://github.com/yigegongjiang/jj-bookmark/releases/tag/v0.2.0
[0.1.2]: https://github.com/yigegongjiang/jj-bookmark/releases/tag/v0.1.2
[0.1.1]: https://github.com/yigegongjiang/jj-bookmark/releases/tag/v0.1.1
[0.1.0]: https://github.com/yigegongjiang/jj-bookmark/releases/tag/v0.1.0
