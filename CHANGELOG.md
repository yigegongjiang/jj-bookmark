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

[0.2.0]: https://github.com/yigegongjiang/jj-bookmark/releases/tag/v0.2.0
[0.1.2]: https://github.com/yigegongjiang/jj-bookmark/releases/tag/v0.1.2
[0.1.1]: https://github.com/yigegongjiang/jj-bookmark/releases/tag/v0.1.1
[0.1.0]: https://github.com/yigegongjiang/jj-bookmark/releases/tag/v0.1.0
