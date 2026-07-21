# jj-bookmark — Raycast extension (dev-only)

Raycast 前端：在 Raycast 里 query + open jj-bookmark 书签。仅本机 dev 运行，NEVER 上架 Store。

## 能力

- **query**：`Search Bookmarks` command，load-once 全量后内存过滤（复刻 CLI `keyword_filter`：Unicode 空白分词含全角、每词命中任意字段 title/url/excerpt/note/folder/tags）
- **open**：回车走 `jj-bookmark --all open <id>`（默认浏览器打开 + 写 `last_visited`）；⌘⇧C 复制 URL、⌘⇧T 复制标题、⌘R 重载

## 集成面（单一核心）

- 读：`jj-bookmark --all ls --json --sort updated` → `{version, sources:{name:[...]}}`（[../README.md](../README.md) 架构）
- 写：仅 `open` 经 CLI（data-side 写操作），过滤/排序为扩展内存逻辑（仿 App/Web 前端）
- 二进制解析：`~/.local/bin/jj-bookmark` → `/Applications/jj-bookmark.app/Contents/Helpers/jj-bookmark` → PATH 兜底

## 运行

```bash
cd raycast
npm install          # 已装；node_modules 不入库
npm run dev          # = ray develop；无需 login。Raycast v2 Beta 在跑 → 自动导入 Beta，否则回退 v1
```

`npm run dev` 无需登录（本地 dev）。起来后扩展常驻 Raycast 根搜索（Development 分组，改码热重载）；进程停止即失去热重载。在 Raycast 里搜 `Search Bookmarks`，回车打开选中书签。

## 说明

- `package.json > author` = `hailv` 占位；仅 `ray publish`/`ray lint` 强校验 Store handle，dev 不阻塞。
- `ray profile`/`ray login` 才需账号；`ray develop`/`ray build` 纯本地，不需要。
