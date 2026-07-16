import AppKit

final class AppDelegate: NSObject, NSApplicationDelegate {
    private var window: NSWindow!
    private var runner: CLIRunner?
    private let autoExit = AutoExitManager()
    private var settingsController: SettingsWindowController?  // 强持有，否则窗口一闪即释放

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.mainMenu = MainMenu.build()
        setupWindow()
        autoExit.start()  // 启动即 arm 倒计时（不等首次交互）
        NSApp.activate(ignoringOtherApps: true)

        // 测试门控：无 GUI 授权下自检设置窗口可开、无裁切（不影响正常使用）。
        if ProcessInfo.processInfo.environment["JJ_BOOKMARK_OPEN_SETTINGS"] != nil {
            showSettings(nil)
            if let cv = settingsController?.window?.contentView {
                cv.layoutSubtreeIfNeeded()
                NSLog("jj-bookmark[selfcheck] settings window=\(settingsController!.window!.frame.size) fitting=\(cv.fittingSize)")
            }
        }
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        true
    }

    private func setupWindow() {
        window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 1000, height: 640),
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )
        window.title = "jj-bookmark"
        window.setFrameAutosaveName("MainWindow")

        do {
            let runner = try CLIRunner.locate()
            self.runner = runner
            window.contentViewController = MainViewController(runner: runner)
            CLIInstaller.installIfNeeded(runner: runner)
        } catch {
            let alert = NSAlert()
            alert.messageText = "无法定位内嵌 CLI"
            alert.informativeText = "\(error.localizedDescription)\n\n开发运行时可设 JJ_BOOKMARK_CLI 指向 CLI 二进制。"
            alert.alertStyle = .critical
            alert.runModal()
        }

        window.center()
        window.makeKeyAndOrderFront(nil)
    }

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
