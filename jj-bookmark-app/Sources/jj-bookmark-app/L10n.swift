import Foundation

// 纯 Swift 本地化表：中 / 日 / 英，默认英文。
// 语言在启动时按系统首选语言判定一次（zh* → 中文, ja* → 日文, 其余 → 英文），
// 与工程「纯源码、无资源文件」风格一致：无 .lproj / NSLocalizedString / package.sh 改动，
// 编译期 key 安全（缺文案不会退化成裸 key），全运行模式（swift run / .app）均可用。
// nonisolated：static let 单次线程安全初始化，可从任意上下文（含非主 actor 的 CLIRunner）调用。
nonisolated enum L10n {
    enum Lang { case en, zh, ja }

    /// 启动时判定一次。`-AppleLanguages "(ja)"` 等启动参数经 NSArgumentDomain 注入
    /// AppleLanguages，会反映在此列表首位，故可用于三语验收。
    static let lang: Lang = {
        let pref = (Locale.preferredLanguages.first ?? "en").lowercased()
        if pref.hasPrefix("zh") { return .zh }
        if pref.hasPrefix("ja") { return .ja }
        return .en // 默认英文（非 CJK 系统一律回退英文）
    }()

    private static func tr(_ en: String, _ zh: String, _ ja: String) -> String {
        switch lang {
        case .en: return en
        case .zh: return zh
        case .ja: return ja
        }
    }

    // App 名（不翻译）
    static let appName = "jj-bookmark"

    // MARK: - 主菜单（MainMenu）

    static var menuBookmarks: String { tr("Bookmarks", "书签", "ブックマーク") }
    static var menuNewBookmark: String { tr("New Bookmark", "新建书签", "新規ブックマーク") }
    static var menuOpen: String { tr("Open", "打开", "開く") }
    static var menuEditItem: String { tr("Edit…", "编辑…", "編集…") }
    static var menuDelete: String { tr("Delete", "删除", "削除") }
    static var menuFind: String { tr("Find", "查找", "検索") }
    static var menuRefresh: String { tr("Refresh", "刷新", "更新") }
    static func menuAbout(_ app: String) -> String { tr("About \(app)", "关于 \(app)", "\(app) について") }
    static var menuSettings: String { tr("Settings…", "偏好设置…", "設定…") }
    static func menuHide(_ app: String) -> String { tr("Hide \(app)", "隐藏 \(app)", "\(app) を隠す") }
    static var menuHideOthers: String { tr("Hide Others", "隐藏其他", "ほかを隠す") }
    static var menuShowAll: String { tr("Show All", "全部显示", "すべてを表示") }
    static func menuQuit(_ app: String) -> String { tr("Quit \(app)", "退出 \(app)", "\(app) を終了") }
    static var menuEditMenu: String { tr("Edit", "编辑", "編集") }
    static var menuUndo: String { tr("Undo", "撤销", "取り消す") }
    static var menuRedo: String { tr("Redo", "重做", "やり直す") }
    static var menuCut: String { tr("Cut", "剪切", "カット") }
    static var menuCopy: String { tr("Copy", "复制", "コピー") }
    static var menuPaste: String { tr("Paste", "粘贴", "ペースト") }
    static var menuSelectAll: String { tr("Select All", "全选", "すべてを選択") }
    static var menuWindow: String { tr("Window", "窗口", "ウインドウ") }
    static var menuMinimize: String { tr("Minimize", "最小化", "しまう") }
    static var menuZoom: String { tr("Zoom", "缩放", "拡大/縮小") }
    static var menuBringAllToFront: String { tr("Bring All to Front", "前置全部窗口", "すべてを前面に移動") }

    // MARK: - 主界面（MainViewController）

    static var searchPlaceholder: String {
        tr("Search bookmarks", "搜索书签", "ブックマークを検索")
    }
    static var toolbarNew: String { tr("＋ New", "＋ 新建", "＋ 新規") }
    static var columnFolder: String { tr("Folder", "文件夹", "フォルダ") }
    static var columnBookmark: String { tr("Bookmark", "书签", "ブックマーク") }

    static var errorLoadFailed: String { tr("Load failed", "加载失败", "読み込みに失敗しました") }
    static var errorOperationFailed: String { tr("Operation failed", "操作失败", "操作に失敗しました") }

    /// 状态栏：全部可见（shown == total）。
    static func statusTotal(_ total: Int) -> String {
        tr(total == 1 ? "1 item" : "\(total) items", "\(total) 条", "\(total) 件")
    }
    /// 状态栏：过滤中（shown / total）。
    static func statusFiltered(shown: Int, total: Int) -> String {
        tr("\(shown) / \(total) items", "\(shown) / \(total) 条", "\(shown) / \(total) 件")
    }

    static var orderAscending: String { tr("↑ Ascending", "↑ 升序", "↑ 昇順") }
    static var orderDescending: String { tr("↓ Descending", "↓ 降序", "↓ 降順") }

    // 排序键（SortKey.label）
    static var sortCreated: String { tr("Date Added", "添加时间", "追加日") }
    static var sortUpdated: String { tr("Date Modified", "编辑时间", "更新日") }
    static var sortVisited: String { tr("Last Visited", "最近访问", "最終アクセス") }
    static var sortTitle: String { tr("Name", "名称", "名前") }

    // 表单（runForm）
    static var formNewTitle: String { tr("New Bookmark", "新建书签", "新規ブックマーク") }
    static var formEditTitle: String { tr("Edit Bookmark", "编辑书签", "ブックマークを編集") }
    static var formRenameTitle: String {
        tr("Rename / Move Folder", "重命名 / 移动文件夹", "フォルダの名前変更 / 移動")
    }
    static var btnAdd: String { tr("Add", "添加", "追加") }
    static var btnSave: String { tr("Save", "保存", "保存") }
    static var btnRename: String { tr("Rename", "重命名", "名前を変更") }
    static var btnCancel: String { tr("Cancel", "取消", "キャンセル") }
    static var btnDelete: String { tr("Delete", "删除", "削除") }

    static var fieldTitle: String { tr("Title", "标题", "タイトル") }
    static var fieldFolder: String { tr("Folder", "文件夹", "フォルダ") }
    static var fieldDescription: String { tr("Description", "描述", "説明") }
    static var fieldNote: String { tr("Note", "备注", "メモ") }
    static var fieldNewPath: String { tr("New Path", "新路径", "新しいパス") }
    static var placeholderTitleHint: String {
        tr("Defaults to URL if empty", "留空则用 URL", "空欄なら URL を使用")
    }

    // 上下文菜单
    static var contextRenameMove: String { tr("Rename / Move…", "重命名 / 移动…", "名前変更 / 移動…") }

    // 删除确认
    static func deleteConfirmOne(_ title: String) -> String {
        tr("Delete “\(title)”?", "删除「\(title)」？", "「\(title)」を削除しますか？")
    }
    static func deleteConfirmMany(_ count: Int) -> String {
        tr("Delete \(count) selected bookmarks?",
           "删除选中的 \(count) 条书签？",
           "選択した \(count) 件のブックマークを削除しますか？")
    }
    static var deleteIrreversible: String {
        tr("This action cannot be undone.", "此操作不可撤销。", "この操作は取り消せません。")
    }

    // MARK: - 偏好设置（SettingsWindowController）

    static var settingsTitle: String { tr("Settings", "偏好设置", "設定") }
    static var autoExitCheckbox: String {
        tr("Automatically quit the app after being idle",
           "闲置一段时间后自动退出 App",
           "一定時間アイドル状態が続くとアプリを自動終了")
    }
    static var idleDuration: String { tr("Idle duration", "闲置时长", "アイドル時間") }
    static var unitMinutes: String { tr("minutes", "分钟", "分") }
    static func presetMinutes(_ m: Int) -> String {
        tr(m == 1 ? "1 minute" : "\(m) minutes", "\(m) 分钟", "\(m) 分")
    }
    static var custom: String { tr("Custom…", "自定义…", "カスタム…") }

    static var sectionAutoExit: String { tr("Auto-Quit", "自动退出", "自動終了") }
    static var hintAutoExit: String {
        tr("When used infrequently it's easy to forget to close the window after opening a link; the app quits automatically when the timer runs out. You can turn this off anytime.",
           "低频使用时打开 link 后常忘记关闭；到点自动退出。可随时关闭此项。",
           "たまにしか使わないと、リンクを開いた後に閉じ忘れがちです。時間切れで自動終了します。この項目はいつでもオフにできます。")
    }
    static var sectionCLI: String { tr("Command-Line Tool (CLI)", "命令行工具（CLI）", "コマンドラインツール（CLI）") }
    static var sectionAbout: String { tr("About · Updates", "关于 · 更新", "情報 · アップデート") }
    static func currentVersion(_ v: String) -> String {
        tr("Current version: \(v)", "当前版本：\(v)", "現在のバージョン：\(v)")
    }
    static var valueUnknown: String { tr("Unknown", "未知", "不明") }
    static var valueNotInstalled: String { tr("Not installed", "未安装", "未インストール") }
    static func cliStatus(bundle: String, installed: String) -> String {
        tr("App bundle: \(bundle)    ~/.local/bin: \(installed)",
           "App 内嵌：\(bundle)    ~/.local/bin：\(installed)",
           "アプリ内蔵：\(bundle)    ~/.local/bin：\(installed)")
    }
    static var btnInstallReinstall: String {
        tr("Install / Reinstall to ~/.local/bin",
           "安装 / 重装到 ~/.local/bin",
           "~/.local/bin にインストール / 再インストール")
    }
    static var btnCheckUpdates: String {
        tr("Check for Updates (open Releases page)",
           "检查更新（打开 Releases 页面）",
           "アップデートを確認（Releases ページを開く）")
    }

    // MARK: - CLI 安装（CLIInstaller）

    static var installTitle: String {
        tr("Install command-line tool?", "安装命令行工具？", "コマンドラインツールをインストールしますか？")
    }
    static var btnInstall: String { tr("Install", "安装", "インストール") }
    static var installText: String {
        tr("Install jj-bookmark to ~/.local/bin for terminal use (make sure that directory is in your PATH).",
           "将 jj-bookmark 安装到 ~/.local/bin，便于终端使用（请确保该目录在 PATH 中）。",
           "ターミナルで使えるよう jj-bookmark を ~/.local/bin にインストールします（そのディレクトリが PATH に含まれていることを確認してください）。")
    }
    static var updateTitle: String {
        tr("Update command-line tool?", "更新命令行工具？", "コマンドラインツールを更新しますか？")
    }
    static var btnUpdate: String { tr("Update", "更新", "更新") }
    static func updateText(installed: String, bundle: String) -> String {
        tr("jj-bookmark in ~/.local/bin is \(installed), the app bundle is \(bundle). Update it?",
           "~/.local/bin 中的 jj-bookmark 为 \(installed)，App 内嵌为 \(bundle)。是否更新？",
           "~/.local/bin の jj-bookmark は \(installed)、アプリ内蔵は \(bundle) です。更新しますか？")
    }
    static var installedTitle: String {
        tr("Command-line tool installed", "命令行工具已安装", "コマンドラインツールをインストールしました")
    }
    static func installedText(_ path: String) -> String {
        tr("jj-bookmark has been copied to \(path)\nMake sure ~/.local/bin is in your PATH.",
           "jj-bookmark 已复制到 \(path)\n请确保 ~/.local/bin 在 PATH 中。",
           "jj-bookmark を \(path) にコピーしました\n~/.local/bin が PATH に含まれていることを確認してください。")
    }
    static var installFailedTitle: String {
        tr("Installation failed", "安装失败", "インストールに失敗しました")
    }
    static func installFailedText(_ path: String) -> String {
        tr("Could not write to \(path); see the Console log for details.",
           "无法写入 \(path)，详见 Console 日志。",
           "\(path) に書き込めませんでした。詳細は Console ログを参照してください。")
    }

    // MARK: - CLI 定位 / 执行（CLIRunner / AppDelegate）

    static func errorCLINotFound(_ path: String) -> String {
        tr("Embedded CLI not found: \(path)", "找不到内嵌 CLI：\(path)", "内蔵 CLI が見つかりません：\(path)")
    }
    static var errorCLIFailed: String {
        tr("CLI execution failed", "CLI 执行失败", "CLI の実行に失敗しました")
    }
    static var errorLocateCLITitle: String {
        tr("Cannot locate the embedded CLI", "无法定位内嵌 CLI", "内蔵 CLI を見つけられません")
    }
    static func errorLocateCLIText(_ desc: String) -> String {
        tr("\(desc)\n\nDuring development you can set JJ_BOOKMARK_CLI to point at the CLI binary.",
           "\(desc)\n\n开发运行时可设 JJ_BOOKMARK_CLI 指向 CLI 二进制。",
           "\(desc)\n\n開発時は JJ_BOOKMARK_CLI を CLI バイナリのパスに設定できます。")
    }

    // MARK: - Folder 树（FolderTree）

    static var folderAll: String { tr("All", "全部", "すべて") }
    static var folderUncategorized: String { tr("Uncategorized", "未分类", "未分類") }
}
