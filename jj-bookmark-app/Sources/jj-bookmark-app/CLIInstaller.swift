import AppKit
import Foundation

// 首次启动可把 ~/.local/bin/jj-bookmark 链接到 App 内嵌 CLI，方便终端使用。
// 旧复制文件 / 失效链接只提示替换，绝不静默覆盖。
@MainActor
enum CLIInstaller {
    static let target = FileManager.default.homeDirectoryForCurrentUser
        .appendingPathComponent(".local/bin/jj-bookmark")

    static func installIfNeeded(runner: CLIRunner) {
        // dev（JJ_BOOKMARK_CLI 指向源码构建的 CLI）时跳过安装。
        guard ProcessInfo.processInfo.environment["JJ_BOOKMARK_CLI"] == nil else { return }
        let bundleVersion = Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? ""
        guard !bundleVersion.isEmpty else { return } // 非 .app 运行

        let fm = FileManager.default
        if !targetExists(using: fm) {
            // 未安装：首启询问一次（记住已问，避免每次弹）。
            guard !UserDefaults.standard.bool(forKey: "cliInstallOffered") else { return }
            UserDefaults.standard.set(true, forKey: "cliInstallOffered")
            if confirm(title: L10n.installTitle, okTitle: L10n.btnInstall, text: L10n.installText) {
                linkCLI(to: runner.executableURL)
            }
            return
        }

        // 已安装：版本不一致或仍是旧复制文件时，询问改为指向当前 App 的链接。
        let installed = installedVersion()
        guard installed != bundleVersion || !isCurrentLink(to: runner.executableURL, using: fm) else {
            return
        }
        if confirm(title: L10n.updateTitle, okTitle: L10n.btnUpdate,
                   text: L10n.updateText(installed: installed ?? L10n.valueUnknown,
                                         bundle: bundleVersion)) {
            linkCLI(to: runner.executableURL)
        }
    }

    // Settings「安装 / 重装」入口：无条件覆盖，弹结果。返回是否成功。
    @discardableResult
    static func reinstall(runner: CLIRunner) -> Bool {
        let ok = linkCLI(to: runner.executableURL)
        let alert = NSAlert()
        if ok {
            alert.messageText = L10n.installedTitle
            alert.informativeText = L10n.installedText(target.path)
        } else {
            alert.messageText = L10n.installFailedTitle
            alert.informativeText = L10n.installFailedText(target.path)
            alert.alertStyle = .warning
        }
        alert.runModal()
        return ok
    }

    // 已安装 CLI 版本（供 Settings 显示）；未安装返回 nil。
    static func installedVersion() -> String? {
        let p = Process()
        p.executableURL = target
        p.arguments = ["--version"]
        let out = Pipe()
        p.standardOutput = out
        p.standardError = Pipe()
        guard (try? p.run()) != nil else { return nil }
        let data = out.fileHandleForReading.readDataToEndOfFile()
        p.waitUntilExit()
        // "jj-bookmark 0.1.0" → "0.1.0"
        return String(data: data, encoding: .utf8)?
            .split(separator: " ").last
            .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
    }

    @discardableResult
    private static func linkCLI(to source: URL) -> Bool {
        let fm = FileManager.default
        do {
            try fm.createDirectory(at: target.deletingLastPathComponent(), withIntermediateDirectories: true)
            if targetExists(using: fm) { try fm.removeItem(at: target) }
            try fm.createSymbolicLink(atPath: target.path, withDestinationPath: source.path)
            return true
        } catch {
            NSLog("jj-bookmark: 安装 CLI 失败: \(error)")
            return false
        }
    }

    private static func targetExists(using fm: FileManager) -> Bool {
        fm.fileExists(atPath: target.path)
            || (try? fm.destinationOfSymbolicLink(atPath: target.path)) != nil
    }

    private static func isCurrentLink(to source: URL, using fm: FileManager) -> Bool {
        (try? fm.destinationOfSymbolicLink(atPath: target.path)) == source.path
    }

    private static func confirm(title: String, okTitle: String, text: String) -> Bool {
        let alert = NSAlert()
        alert.messageText = title
        alert.informativeText = text
        alert.addButton(withTitle: okTitle)
        alert.addButton(withTitle: L10n.btnCancel)
        return alert.runModal() == .alertFirstButtonReturn
    }
}
