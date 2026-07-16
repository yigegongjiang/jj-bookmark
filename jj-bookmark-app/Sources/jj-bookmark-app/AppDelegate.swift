import AppKit

final class AppDelegate: NSObject, NSApplicationDelegate, NSWindowDelegate {
    private var window: NSWindow!
    private var runner: CLIRunner?
    private let autoExit = AutoExitManager()
    private var settingsController: SettingsWindowController?  // 强持有，否则窗口一闪即释放

    // 只记尺寸（手动持久化 UserDefaults，绕开状态恢复下失效/冲突的 setFrameAutosaveName）；
    // 位置总是居中于鼠标当前所在屏幕（多显示器：在哪块屏就在哪块打开，避免记死位置后跑到屏外）。
    // 全程按 content 尺寸存取，避免 frame 尺寸（含标题栏）反复 set 导致按标题栏高度累积漂移。
    private static let widthKey = "JJBookmark.windowW"
    private static let heightKey = "JJBookmark.windowH"
    private static let defaultSize = NSSize(width: 1000, height: 640)
    private static let minSize = NSSize(width: 640, height: 420)

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.mainMenu = MainMenu.build()
        setupWindow()
        autoExit.start()  // 启动即 arm 倒计时（不等首次交互）
        NSApp.activate(ignoringOtherApps: true)

        // 测试门控：无 GUI 授权下自检窗口尺寸记忆（dump 实际 content/frame 后退出，不阻塞正常使用）。
        // 配合 argument-domain `-JJBookmark.windowW/H` 验证 restore 路径。
        if ProcessInfo.processInfo.environment["JJ_BOOKMARK_DUMP_WINDOW"] != nil {
            let cs = window.contentRect(forFrameRect: window.frame).size
            let fs = window.frame.size
            print("[window] content = \(Int(cs.width))x\(Int(cs.height))")
            print("[window] frame = \(Int(fs.width))x\(Int(fs.height))")
            print("[window] contentMin = \(Int(window.contentMinSize.width))x\(Int(window.contentMinSize.height))")
            exit(0)
        }

        // 测试门控：无 GUI 授权下自检设置窗口可开、无裁切（不影响正常使用）。
        if ProcessInfo.processInfo.environment["JJ_BOOKMARK_OPEN_SETTINGS"] != nil {
            showSettings(nil)
            if let cv = settingsController?.window?.contentView {
                cv.layoutSubtreeIfNeeded()
                NSLog("jj-bookmark[selfcheck] lang=\(L10n.lang) settings window=\(settingsController!.window!.frame.size) fitting=\(cv.fittingSize)")
            }
        }
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        true
    }

    private func setupWindow() {
        let contentSize = Self.launchContentSize()
        window = NSWindow(
            contentRect: NSRect(origin: .zero, size: contentSize),
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )
        window.title = "jj-bookmark"
        window.contentMinSize = Self.minSize  // 约束 content（与持久化的 content 尺寸同坐标系）
        window.isRestorable = false  // 关系统自动状态恢复，避免与手动尺寸持久化互相干扰
        window.delegate = self

        do {
            let runner = try CLIRunner.locate()
            self.runner = runner
            window.contentViewController = MainViewController(runner: runner)
            CLIInstaller.installIfNeeded(runner: runner)
        } catch {
            let alert = NSAlert()
            alert.messageText = L10n.errorLocateCLITitle
            alert.informativeText = L10n.errorLocateCLIText(error.localizedDescription)
            alert.alertStyle = .critical
            alert.runModal()
        }

        // 装配 contentViewController 后强制 content 尺寸（防被 VC fitting 覆盖），再居中于鼠标所在屏幕。
        window.setContentSize(contentSize)
        let vf = Self.mouseScreen().visibleFrame
        let fs = window.frame.size
        window.setFrameOrigin(NSPoint(x: vf.midX - fs.width / 2, y: vf.midY - fs.height / 2))
        window.makeKeyAndOrderFront(nil)
    }

    // 上次保存的 content 尺寸（合法才用，否则默认）；超屏则 clamp 到鼠标屏可见区域。
    private static func launchContentSize() -> NSSize {
        let w = UserDefaults.standard.double(forKey: widthKey)
        let h = UserDefaults.standard.double(forKey: heightKey)
        let size = (w >= minSize.width && h >= minSize.height) ? NSSize(width: w, height: h) : defaultSize

        let vf = mouseScreen().visibleFrame
        return NSSize(width: min(size.width, vf.width), height: min(size.height, vf.height))
    }

    private static func mouseScreen() -> NSScreen {
        let p = NSEvent.mouseLocation
        return NSScreen.screens.first { $0.frame.contains(p) }
            ?? NSScreen.main
            ?? NSScreen.screens.first
            ?? NSScreen()
    }

    private func saveWindowSize() {
        guard let window else { return }
        let cs = window.contentRect(forFrameRect: window.frame).size  // 存 content 尺寸，无标题栏漂移
        UserDefaults.standard.set(cs.width, forKey: Self.widthKey)
        UserDefaults.standard.set(cs.height, forKey: Self.heightKey)
    }

    // resize 即存（每次拖拽落盘）；close 兜底。auto-exit 绕过 close 也无碍——尺寸早已随 resize 落盘。
    func windowDidResize(_ notification: Notification) { saveWindowSize() }
    func windowWillClose(_ notification: Notification) { saveWindowSize() }

    // 菜单「偏好设置…」(⌘,) → 打开设置窗口（懒建、复用）。
    @objc func showSettings(_ sender: Any?) {
        guard let runner else {
            NSSound.beep()
            return
        }
        if settingsController == nil {
            settingsController = SettingsWindowController(runner: runner, autoExit: autoExit)
        }
        settingsController?.showWindow(nil)
        settingsController?.window?.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
    }
}
