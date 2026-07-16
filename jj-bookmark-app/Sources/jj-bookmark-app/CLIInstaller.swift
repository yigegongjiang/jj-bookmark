import AppKit
import Foundation

// 首次启动可把内嵌 CLI 装到 ~/.local/bin，方便终端使用（roadmap Phase 5）。
// 仅版本不一致才提示覆盖，绝不静默覆盖用户可能更新的副本。
@MainActor
enum CLIInstaller {
    static let target = FileManager.default.homeDirectoryForCurrentUser
        .appendingPathComponent(".local/bin/jj-bookmark")

    static func installIfNeeded(runner: CLIRunner) {
        let env = ProcessInfo.processInfo.environment
        // dev（JJ_BOOKMARK_CLI）或测试（JJ_BOOKMARK_NO_INSTALL）时跳过。
        guard env["JJ_BOOKMARK_CLI"] == nil, env["JJ_BOOKMARK_NO_INSTALL"] == nil else { return }
        let bundleVersion = Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? ""
        guard !bundleVersion.isEmpty else { return } // 非 .app 运行

        let fm = FileManager.default
        if !fm.fileExists(atPath: target.path) {
            // 未安装：首启询问一次（记住已问，避免每次弹）。
            guard !UserDefaults.standard.bool(forKey: "cliInstallOffered") else { return }
            UserDefaults.standard.set(true, forKey: "cliInstallOffered")
            if confirm(title: "安装命令行工具？", okTitle: "安装",
                       text: "将 jj-bookmark 安装到 ~/.local/bin，便于终端使用（请确保该目录在 PATH 中）。") {
                copyCLI(from: runner.executableURL)
            }
            return
        }

        // 已安装：仅当版本不一致才询问更新。
        guard let installed = installedVersion(), installed != bundleVersion else { return }
        if confirm(title: "更新命令行工具？", okTitle: "更新",
                   text: "~/.local/bin 中的 jj-bookmark 为 \(installed)，App 内嵌为 \(bundleVersion)。是否更新？") {
            copyCLI(from: runner.executableURL)
        }
    }

    // Settings「安装 / 重装」入口：无条件覆盖，弹结果。返回是否成功。
    @discardableResult
    static func reinstall(runner: CLIRunner) -> Bool {
        let ok = copyCLI(from: runner.executableURL)
        let alert = NSAlert()
        if ok {
            alert.messageText = "命令行工具已安装"
            alert.informativeText = "jj-bookmark 已复制到 \(target.path)\n请确保 ~/.local/bin 在 PATH 中。"
        } else {
            alert.messageText = "安装失败"
            alert.informativeText = "无法写入 \(target.path)，详见 Console 日志。"
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
    private static func copyCLI(from src: URL) -> Bool {
        let fm = FileManager.default
        do {
            try fm.createDirectory(at: target.deletingLastPathComponent(), withIntermediateDirectories: true)
            if fm.fileExists(atPath: target.path) { try fm.removeItem(at: target) }
            try fm.copyItem(at: src, to: target)
            try fm.setAttributes([.posixPermissions: 0o755], ofItemAtPath: target.path)
            return true
        } catch {
            NSLog("jj-bookmark: 安装 CLI 失败: \(error)")
            return false
        }
    }

    private static func confirm(title: String, okTitle: String, text: String) -> Bool {
        let alert = NSAlert()
        alert.messageText = title
        alert.informativeText = text
        alert.addButton(withTitle: okTitle)
        alert.addButton(withTitle: "取消")
        return alert.runModal() == .alertFirstButtonReturn
    }
}
