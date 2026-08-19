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

## [0.26.0] - 2026-08-19

### Added

- `/123` 变成**任意 folder 的卡片视图**：`/123?f=<folder>` 打开该 folder，缺省仍是 `123`；全库列表页侧栏每个 folder hover 出 `↗` 直接跳过去
- 两张页面互有入口：列表页顶部「123」按钮、卡片页顶部「全部书签」按钮

### Changed

- 导航页不再有自己的存储：原 `/123` 的 14 条链接已并入书签库 `default` source 的 `123::<原分组>` 下，两张页面从此共用同一份数据、同一套同步
- 卡片配色改由域名自动生成（不再可自选）

### Removed

- 导航页的手动拖拽排序、按点击次数排序、自选配色一并退役——书签模型里没有它们的位置，为它们加字段不值得

## [0.25.1] - 2026-08-19

### Fixed

- `sync` 缺凭据时的提示补全了最容易漏的一步（还要给 Access 应用加一条 `Service Auth` 策略，否则 Access 仍要求浏览器登录、同步报 302），并给出可直接粘贴的建文件命令

## [0.25.0] - 2026-08-19

### Added

- web 书签页可写：hover 出「编辑 / 删除」，右上「+ Add」新增；改动即时保存
- 删除改为可撤销：`apply <ID> --delete` 只做标记，`--restore` 恢复，`ls --deleted` 查看已删

### Changed

- `jj-bookmark sync` 取代 `push`：拉云端 → 按书签逐条比最后修改时间合并 → 写回，两端都能改；同一条两端同时改则后改的胜出
- 数据文件升级到 v4（首次写入自动完成，旧文件仍留在 `bookmarks.json.bak`）；升级后**旧版本的 jj-bookmark 无法读取**，请一并升级。已删书签永久保留在文件里（不可见、不占文件夹、编号不复用），这是两端同步不「复活」已删条目的前提；`sources` 因此会在有墓碑时多列一个 `(+N deleted)`

### Removed

- `push` 命令删除：它绕过冲突检查直连存储，留着会静默覆盖 web 端的编辑

## [0.24.0] - 2026-08-19

### Removed

- 书签的 `cover` / `tags` / `favorite` 三个字段从数据模型删除：三者始终无处填写、无处显示（`cover` 是早期导入残留的缩略图 URL，从不渲染），保留只是让每条记录多三行噪音
- 下一次写入书签时，数据文件里这三个字段会被自动清掉（旧值不再保留；改动前的文件仍留在 `bookmarks.json.bak`）

## [0.23.1] - 2026-08-19

### Fixed

- `--source` 现在放在子命令前后都生效（`jj-bookmark folders --source default` 此前报 `unexpected argument '--source' found`，只能写成 `jj-bookmark --source default folders`）；每个子命令的 `--help` 也会列出它

## [0.23.0] - 2026-08-19

### Changed

- 作用域彻底简化为单一真实 source：`--source <NAME>` 只收真实 source 名，省略即 `default`（= `apply <URL>` 的落点）；`all` / `--all` 这类「全部 source」参数不再存在
- 按 ID 编辑 / 删除 / 打开也只在指定 source 内生效：操作其他 source 的书签需带 `--source <NAME>`（App / Raycast 会自动带上该书签所属 source，使用上无变化）
- 文件夹改名 `mv` 恢复为只作用于目标 source，输出恢复简洁形式

### Removed

- 保存书签的内建指引不再跨 source 查重（只看默认 source）：其他 source 里的同一条书签不会再被提示为已存在

## [0.22.0] - 2026-08-19

### Changed

- 作用域参数定型为单个 `--source <NAME|all>`，省略即 `default` —— 与 `apply <URL>` 的落点是同一个默认值，读 / 写 / 改名不用分别记；要跨全部 source 时写 `--source all`（v0.21.0 的「省略 = 全部 source」已回退）
- `all` 成为保留值，不能再作真实 source 名；`--source all apply <URL>` 报错（新增书签必须落在单一 source）
- 保存书签的内建指引改为「`folders` 挑目录 → `apply` 存入」同为默认 source，不再混入其他 source 的旧目录体系

### Fixed

- `--source` 指向不存在的 source 时报错并列出已知 source（此前静默返回空结果，typo 看起来像「库里没有」）

## [0.21.0] - 2026-08-19

### Changed

- 作用域选项合二为一：`--all` 移除，不带 `--source` 即作用于全部 source（列表 / 搜索 / 按 ID 编辑·删除·打开都不再需要额外参数）；只想操作单个 source 时仍用 `--source <NAME>`
- `apply <URL>` 新增书签不带 `--source` 时仍存入 `default`（需唯一落点，行为不变）
- 文件夹改名 `mv` 的默认范围随之扩大到全部 source（原为仅 `default`）；输出改为列出受影响的 source 与条数，如 `Moved 2 bookmark(s): Shared → Tech [default(1), safari(1)]`

## [0.20.0] - 2026-08-19

### Changed

- 列表与搜索合并为一个命令：`ls [KEYWORD]`，不带关键词即全量列表；原 `query` 命令移除，`query <KEYWORD>` 改用 `ls <KEYWORD>`

### Removed

- 移除内置 jq 过滤（原 `query --filter`）：数据文件是纯 JSON，直接用系统 `jq` 处理，能力更完整

## [0.19.0] - 2026-08-19

### Removed

- 移除网页元数据抓取：保存书签不再自动联网取标题 / 摘要 / 封面；App 新增时留空标题将保留网址原样，标题与摘要请在新增或编辑时自行填写

## [0.18.0] - 2026-07-27

### Changed

- 导航页整页重做：卡片面板 + 搜索（`/` 聚焦、Enter 直开首个结果）+ 分组筛选 + 高频 / 最近 / 手动三态排序，暗亮双主题
- 导航页改为随处可编辑：卡片悬停即出编辑 / 删除，内联表单填名称·网址·分组·配色，手动排序下拖拽调链接与分组顺序、分组可就地改名；不再需要切换「编辑模式」
- 导航页按本机点击次数排「高频」，次数只留在本浏览器，不上传

## [0.17.1] - 2026-07-27

### Fixed

- 修复 `123.yigegongjiang.com` 登录后跳到「Page not found」、始终进不去导航页

## [0.17.0] - 2026-07-27

### Added

- 新增个人导航页（hao123 式）：`123.yigegongjiang.com`（或预览域 `/123`）直达，分组 + 链接在页面内直接增删改、拖拽排序、自动保存；数据只存云端，Google 登录访问

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
