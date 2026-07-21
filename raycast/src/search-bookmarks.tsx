import { useMemo, useState } from "react";
import { existsSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { Action, ActionPanel, Icon, Keyboard, List, showHUD, showToast, Toast } from "@raycast/api";
import { useExec } from "@raycast/utils";

const pexec = promisify(execFile);

/** source 下拉「全部」哨兵值（真 source 名不会是它）。 */
const ALL_SOURCES = "__all__";

/** jj-bookmark 二进制解析：优先本机符号链接 → App 内嵌 helper → PATH 兜底（固定候选，无 config knob）。 */
const BIN: string =
  [join(homedir(), ".local/bin/jj-bookmark"), "/Applications/jj-bookmark.app/Contents/Helpers/jj-bookmark"].find(
    existsSync,
  ) ?? "jj-bookmark";

interface Bookmark {
  id: number;
  title: string;
  url: string;
  excerpt: string;
  note: string;
  folder: string;
  tags: string[];
  favorite: boolean;
  updated: number;
  last_visited: number;
  last_visited_jst: string;
  source: string;
}

interface Store {
  version: number;
  sources: Record<string, Omit<Bookmark, "source">[]>;
}

/** 复刻 CLI query.rs::keyword_filter：按 Unicode 空白分词（含全角 U+3000），每词大小写不敏感，全部命中任意可搜索字段。 */
function keywordFilter(items: Bookmark[], keyword: string): Bookmark[] {
  const terms = keyword
    .split(/\p{White_Space}+/u)
    .filter(Boolean)
    .map((t) => t.toLowerCase());
  if (terms.length === 0) return items;
  return items.filter((b) => {
    const hay = `${b.title} ${b.url} ${b.excerpt} ${b.note} ${b.folder} ${b.tags.join(" ")}`.toLowerCase();
    return terms.every((t) => hay.includes(t));
  });
}

export default function Command() {
  const [searchText, setSearchText] = useState("");
  const [source, setSource] = useState(ALL_SOURCES);
  // load-once：与 App/Web 前端一致，CLI 只负责 load，过滤在内存（对 CJK 最可预测）。
  const { data, isLoading, error, revalidate } = useExec(BIN, ["--all", "ls", "--json", "--sort", "visited"], {
    keepPreviousData: true,
  });

  const items = useMemo<Bookmark[]>(() => {
    if (!data) return [];
    try {
      const store: Store = JSON.parse(data);
      const flat = Object.entries(store.sources).flatMap(([src, arr]) => arr.map((b) => ({ ...b, source: src })));
      // 排序：最后打开时间(last_visited)倒序；未打开过(=0)并列时退回 updated 倒序 → 最近打开置顶，其余按最近更新。
      flat.sort((a, b) => b.last_visited - a.last_visited || b.updated - a.updated);
      return flat;
    } catch {
      return [];
    }
  }, [data]);

  // source 下拉选项：从实际数据动态取（无硬编码），保证与数据文件永不失配。
  const sourceNames = useMemo(() => Array.from(new Set(items.map((b) => b.source))).sort(), [items]);

  const filtered = useMemo(() => {
    const scoped = source === ALL_SOURCES ? items : items.filter((b) => b.source === source);
    return keywordFilter(scoped, searchText);
  }, [items, source, searchText]);

  async function open(b: Bookmark) {
    try {
      // 走 CLI open：默认浏览器打开 + 记录 last_visited（单一核心，data-side 写操作）。id 全局唯一，用 --all 命中任意 source。
      await pexec(BIN, ["--all", "open", String(b.id)]);
      await showHUD(`Opened: ${b.title || b.url}`);
    } catch (e) {
      await showToast({ style: Toast.Style.Failure, title: "Open failed", message: String(e) });
    }
  }

  return (
    <List
      isLoading={isLoading}
      filtering={false}
      onSearchTextChange={setSearchText}
      searchBarPlaceholder="Search bookmarks (title / url / folder / note / tags)…"
      searchBarAccessory={
        <List.Dropdown tooltip="Source" storeValue onChange={setSource}>
          <List.Dropdown.Item title="All Sources" value={ALL_SOURCES} />
          {sourceNames.map((s) => (
            <List.Dropdown.Item key={s} title={s} value={s} />
          ))}
        </List.Dropdown>
      }
    >
      {error ? (
        <List.EmptyView icon={Icon.Warning} title="Failed to load bookmarks" description={`${BIN}\n${String(error)}`} />
      ) : (
        filtered.map((b) => {
          const accessories: List.Item.Accessory[] = [];
          if (b.folder) accessories.push({ icon: Icon.Folder, tag: b.folder, tooltip: `Folder: ${b.folder}` });
          if (source === ALL_SOURCES && b.source !== "default") accessories.push({ tag: b.source });
          if (b.favorite) accessories.push({ icon: Icon.Star, tooltip: "Favorite" });
          if (b.last_visited > 0)
            accessories.push({ date: new Date(b.last_visited), tooltip: `Last visited ${b.last_visited_jst}` });
          return (
            <List.Item
              key={b.id}
              icon={b.favorite ? Icon.Star : Icon.Bookmark}
              title={b.title || b.url}
              subtitle={b.title ? b.url : undefined}
              accessories={accessories}
              keywords={[b.url, b.folder, ...b.tags]}
              actions={
                <ActionPanel>
                  <Action title="Open in Browser" icon={Icon.Globe} onAction={() => open(b)} />
                  <Action.CopyToClipboard title="Copy URL" content={b.url} shortcut={Keyboard.Shortcut.Common.Copy} />
                  <Action.CopyToClipboard
                    title="Copy Title"
                    content={b.title}
                    shortcut={{ modifiers: ["cmd", "shift"], key: "t" }}
                  />
                  <Action
                    title="Reload"
                    icon={Icon.ArrowClockwise}
                    shortcut={Keyboard.Shortcut.Common.Refresh}
                    onAction={() => revalidate()}
                  />
                </ActionPanel>
              }
            />
          );
        })
      )}
    </List>
  );
}
