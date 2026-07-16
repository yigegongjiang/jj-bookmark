```When Editing
本文档作用: 面向开发者的发版记录; CHANGELOG.md 的超集, 1:1 镜像 + 技术变更子项
遵循 AGENTS.md 文档编写规范
- 每条主项 = CHANGELOG.md 对应条目 (原文), 下方缩进子项承载技术变更
- 子项 MAY 写路径 / 函数 / 机制; ≤ 1 行
```

# Changelog (developer, follow [CHANGELOG.md](./CHANGELOG.md))

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

[0.1.1]: https://github.com/yigegongjiang/jj-bookmark/releases/tag/v0.1.1
[0.1.0]: https://github.com/yigegongjiang/jj-bookmark/releases/tag/v0.1.0
