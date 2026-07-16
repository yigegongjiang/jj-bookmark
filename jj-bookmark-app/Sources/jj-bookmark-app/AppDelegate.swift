import AppKit

final class AppDelegate: NSObject, NSApplicationDelegate {
    private var window: NSWindow!

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.mainMenu = MainMenu.build()
        setupWindow()
        NSApp.activate(ignoringOtherApps: true)
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
}
