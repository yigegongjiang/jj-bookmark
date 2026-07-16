import AppKit

// 纯代码构建主菜单（无 xib）。标准 App / Edit / Window 三块，满足退出、复制粘贴、窗口操作。
enum MainMenu {
    static func build() -> NSMenu {
        let mainMenu = NSMenu()
        mainMenu.addItem(appMenu())
        mainMenu.addItem(bookmarksMenu())
        mainMenu.addItem(editMenu())
        mainMenu.addItem(windowMenu())
        return mainMenu
    }

    // 书签操作 + 快捷键；target 留 nil → 经响应链路由到 MainViewController。
    private static func bookmarksMenu() -> NSMenuItem {
        let item = NSMenuItem()
        let menu = NSMenu(title: "书签")
        addItem(menu, "新建书签", #selector(MainViewController.newBookmark), "n")
        addItem(menu, "打开", #selector(MainViewController.openSelected), "o")
        addItem(menu, "编辑…", #selector(MainViewController.editSelected), "e")
        let del = NSMenuItem(title: "删除",
                             action: #selector(MainViewController.deleteSelected),
                             keyEquivalent: "\u{8}") // ⌫
        del.keyEquivalentModifierMask = [.command]
        menu.addItem(del)
        menu.addItem(.separator())
        addItem(menu, "查找", #selector(MainViewController.focusSearch), "f")
        addItem(menu, "刷新", #selector(MainViewController.refresh), "r")
        item.submenu = menu
        return item
    }

    private static func addItem(_ menu: NSMenu, _ title: String, _ action: Selector, _ key: String) {
        let item = NSMenuItem(title: title, action: action, keyEquivalent: key)
        item.keyEquivalentModifierMask = [.command]
        menu.addItem(item)
    }

    private static let appName = "jj-bookmark"

    private static func appMenu() -> NSMenuItem {
        let item = NSMenuItem()
        let menu = NSMenu()
        menu.addItem(withTitle: "关于 \(appName)",
                     action: #selector(NSApplication.orderFrontStandardAboutPanel(_:)), keyEquivalent: "")
        menu.addItem(.separator())
        // target=AppDelegate（在响应链上）；标准 ⌘, 打开偏好设置。
        let settings = NSMenuItem(title: "偏好设置…",
                                  action: #selector(AppDelegate.showSettings(_:)), keyEquivalent: ",")
        settings.target = NSApp.delegate
        menu.addItem(settings)
        menu.addItem(.separator())
        menu.addItem(withTitle: "隐藏 \(appName)",
                     action: #selector(NSApplication.hide(_:)), keyEquivalent: "h")
        let hideOthers = NSMenuItem(title: "隐藏其他",
                                    action: #selector(NSApplication.hideOtherApplications(_:)), keyEquivalent: "h")
        hideOthers.keyEquivalentModifierMask = [.command, .option]
        menu.addItem(hideOthers)
        menu.addItem(withTitle: "全部显示",
                     action: #selector(NSApplication.unhideAllApplications(_:)), keyEquivalent: "")
        menu.addItem(.separator())
        menu.addItem(withTitle: "退出 \(appName)",
                     action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
        item.submenu = menu
        return item
    }

    private static func editMenu() -> NSMenuItem {
        let item = NSMenuItem()
        let menu = NSMenu(title: "编辑")
        menu.addItem(withTitle: "撤销", action: Selector(("undo:")), keyEquivalent: "z")
        let redo = NSMenuItem(title: "重做", action: Selector(("redo:")), keyEquivalent: "z")
        redo.keyEquivalentModifierMask = [.command, .shift]
        menu.addItem(redo)
        menu.addItem(.separator())
        menu.addItem(withTitle: "剪切", action: #selector(NSText.cut(_:)), keyEquivalent: "x")
        menu.addItem(withTitle: "复制", action: #selector(NSText.copy(_:)), keyEquivalent: "c")
        menu.addItem(withTitle: "粘贴", action: #selector(NSText.paste(_:)), keyEquivalent: "v")
        menu.addItem(withTitle: "全选", action: #selector(NSText.selectAll(_:)), keyEquivalent: "a")
        item.submenu = menu
        return item
    }

    private static func windowMenu() -> NSMenuItem {
        let item = NSMenuItem()
        let menu = NSMenu(title: "窗口")
        menu.addItem(withTitle: "最小化", action: #selector(NSWindow.performMiniaturize(_:)), keyEquivalent: "m")
        menu.addItem(withTitle: "缩放", action: #selector(NSWindow.performZoom(_:)), keyEquivalent: "")
        menu.addItem(.separator())
        menu.addItem(withTitle: "前置全部窗口",
                     action: #selector(NSApplication.arrangeInFront(_:)), keyEquivalent: "")
        item.submenu = menu
        NSApp.windowsMenu = menu
        return item
    }
}
