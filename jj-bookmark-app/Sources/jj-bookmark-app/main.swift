import AppKit

// i18n 无头自检：dump 当前判定语言 + 代表性文案后退出（不起 GUI，纯 Foundation）。
// 配合 `-AppleLanguages "(ja)"` 等启动参数验证语言判定与英文兜底。与既有 JJ_BOOKMARK_* 钩子同风格。
if ProcessInfo.processInfo.environment["JJ_BOOKMARK_DUMP_L10N"] != nil {
    let lang = switch L10n.lang { case .en: "en"; case .zh: "zh"; case .ja: "ja" }
    let lines: [(String, String)] = [
        ("preferredLanguages.first", Locale.preferredLanguages.first ?? "nil"),
        ("detected", lang),
        ("menu.bookmarks", L10n.menuBookmarks),
        ("menu.quit", L10n.menuQuit(L10n.appName)),
        ("toolbar.new", L10n.toolbarNew),
        ("sort.updated", L10n.sortUpdated),
        ("status.one", L10n.statusTotal(1)),
        ("status.many", L10n.statusTotal(3)),
        ("status.filtered", L10n.statusFiltered(shown: 2, total: 9)),
        ("settings.title", L10n.settingsTitle),
        ("settings.autoExit", L10n.sectionAutoExit),
        ("settings.preset1", L10n.presetMinutes(1)),
        ("settings.preset5", L10n.presetMinutes(5)),
        ("settings.reinstall", L10n.btnInstallReinstall),
        ("settings.hint", L10n.hintAutoExit),
        ("delete.one", L10n.deleteConfirmOne("Example")),
        ("delete.many", L10n.deleteConfirmMany(3)),
        ("folder.all", L10n.folderAll),
        ("folder.uncategorized", L10n.folderUncategorized),
    ]
    for (k, v) in lines { print("[l10n] \(k) = \(v)") }
    exit(0)
}

// AppKit 纯源码入口：手动装配 NSApplication，无 Storyboard / @NSApplicationMain。
let app = NSApplication.shared
let delegate = AppDelegate()
app.delegate = delegate
app.setActivationPolicy(.regular) // 进 Dock、有主菜单、可聚焦
app.run()
