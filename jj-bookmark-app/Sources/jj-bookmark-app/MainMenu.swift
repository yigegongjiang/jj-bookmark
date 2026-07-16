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
        let menu = NSMenu(title: L10n.menuBookmarks)
        addItem(menu, L10n.menuNewBookmark, #selector(MainViewController.newBookmark), "n")
        addItem(menu, L10n.menuOpen, #selector(MainViewController.openSelected), "o")
        addItem(menu, L10n.menuEditItem, #selector(MainViewController.editSelected), "e")
        let del = NSMenuItem(title: L10n.menuDelete,
                             action: #selector(MainViewController.deleteSelected),
                             keyEquivalent: "\u{8}") // ⌫
        del.keyEquivalentModifierMask = [.command]
        menu.addItem(del)
        menu.addItem(.separator())
        addItem(menu, L10n.menuFind, #selector(MainViewController.focusSearch), "f")
        addItem(menu, L10n.menuRefresh, #selector(MainViewController.refresh), "r")
        item.submenu = menu
        return item
    }

    private static func addItem(_ menu: NSMenu, _ title: String, _ action: Selector, _ key: String) {
        let item = NSMenuItem(title: title, action: action, keyEquivalent: key)
        item.keyEquivalentModifierMask = [.command]
        menu.addItem(item)
    }

    private static let appName = L10n.appName

    private static func appMenu() -> NSMenuItem {
        let item = NSMenuItem()
        let menu = NSMenu()
        menu.addItem(withTitle: L10n.menuAbout(appName),
                     action: #selector(NSApplication.orderFrontStandardAboutPanel(_:)), keyEquivalent: "")
        menu.addItem(.separator())
        // target=AppDelegate（在响应链上）；标准 ⌘, 打开偏好设置。
        let settings = NSMenuItem(title: L10n.menuSettings,
                                  action: #selector(AppDelegate.showSettings(_:)), keyEquivalent: ",")
        settings.target = NSApp.delegate
        menu.addItem(settings)
        menu.addItem(.separator())
        menu.addItem(withTitle: L10n.menuHide(appName),
                     action: #selector(NSApplication.hide(_:)), keyEquivalent: "h")
        let hideOthers = NSMenuItem(title: L10n.menuHideOthers,
                                    action: #selector(NSApplication.hideOtherApplications(_:)), keyEquivalent: "h")
        hideOthers.keyEquivalentModifierMask = [.command, .option]
        menu.addItem(hideOthers)
        menu.addItem(withTitle: L10n.menuShowAll,
                     action: #selector(NSApplication.unhideAllApplications(_:)), keyEquivalent: "")
        menu.addItem(.separator())
        menu.addItem(withTitle: L10n.menuQuit(appName),
                     action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
        item.submenu = menu
        return item
    }

    private static func editMenu() -> NSMenuItem {
        let item = NSMenuItem()
        let menu = NSMenu(title: L10n.menuEditMenu)
        menu.addItem(withTitle: L10n.menuUndo, action: Selector(("undo:")), keyEquivalent: "z")
        let redo = NSMenuItem(title: L10n.menuRedo, action: Selector(("redo:")), keyEquivalent: "z")
        redo.keyEquivalentModifierMask = [.command, .shift]
        menu.addItem(redo)
        menu.addItem(.separator())
        menu.addItem(withTitle: L10n.menuCut, action: #selector(NSText.cut(_:)), keyEquivalent: "x")
        menu.addItem(withTitle: L10n.menuCopy, action: #selector(NSText.copy(_:)), keyEquivalent: "c")
        menu.addItem(withTitle: L10n.menuPaste, action: #selector(NSText.paste(_:)), keyEquivalent: "v")
        menu.addItem(withTitle: L10n.menuSelectAll, action: #selector(NSText.selectAll(_:)), keyEquivalent: "a")
        item.submenu = menu
        return item
    }

    private static func windowMenu() -> NSMenuItem {
        let item = NSMenuItem()
        let menu = NSMenu(title: L10n.menuWindow)
        menu.addItem(withTitle: L10n.menuMinimize, action: #selector(NSWindow.performMiniaturize(_:)), keyEquivalent: "m")
        menu.addItem(withTitle: L10n.menuZoom, action: #selector(NSWindow.performZoom(_:)), keyEquivalent: "")
        menu.addItem(.separator())
        menu.addItem(withTitle: L10n.menuBringAllToFront,
                     action: #selector(NSApplication.arrangeInFront(_:)), keyEquivalent: "")
        item.submenu = menu
        NSApp.windowsMenu = menu
        return item
    }
}
