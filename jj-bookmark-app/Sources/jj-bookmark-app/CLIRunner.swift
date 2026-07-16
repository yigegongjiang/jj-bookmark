import Foundation

// 定位并调用内嵌的同版本 CLI（bundle 内 Contents/Helpers/jj-bookmark）。
// 所有数据侧操作（加载 / 写 / 抓取）经此处 Process 调用；App 不复刻读写协议。
nonisolated struct CLIRunner: Sendable {
    let executableURL: URL

    enum CLIError: LocalizedError {
        case notFound(String)
        case failed(status: Int32, stderr: String)

        var errorDescription: String? {
            switch self {
            case .notFound(let p): return "找不到内嵌 CLI：\(p)"
            case .failed(_, let s): return s.isEmpty ? "CLI 执行失败" : s
            }
        }
    }

    /// 定位 CLI：优先环境变量 `JJ_BOOKMARK_CLI`（dev：`swift run` 时 Bundle.main 非 .app），
    /// 否则 bundle 内 `Contents/Helpers/jj-bookmark`。
    static func locate() throws -> CLIRunner {
        if let override = ProcessInfo.processInfo.environment["JJ_BOOKMARK_CLI"], !override.isEmpty {
            let url = URL(fileURLWithPath: override)
            guard FileManager.default.isExecutableFile(atPath: url.path) else {
                throw CLIError.notFound(url.path)
            }
            return CLIRunner(executableURL: url)
        }
        let url = Bundle.main.bundleURL.appendingPathComponent("Contents/Helpers/jj-bookmark")
        guard FileManager.default.isExecutableFile(atPath: url.path) else {
            throw CLIError.notFound(url.path)
        }
        return CLIRunner(executableURL: url)
    }

    /// 运行子命令，返回 stdout（成功）；非零退出抛出含 stderr 的错误。
    /// 关键：并发读 stdout/stderr 两个管道到 EOF，再 `waitUntilExit()`，
    /// 避免大输出（1306 条 ~1MB）写满管道缓冲导致死锁（advisor 铁律）。
    @discardableResult
    func run(_ args: [String]) throws -> Data {
        let proc = Process()
        proc.executableURL = executableURL
        proc.arguments = args
        let outPipe = Pipe()
        let errPipe = Pipe()
        proc.standardOutput = outPipe
        proc.standardError = errPipe
        try proc.run()

        // 后台读 stdout，前台读 stderr，DispatchGroup 同步 → 两管道并发排空。
        final class DataBox: @unchecked Sendable { var data = Data() }
        let outBox = DataBox()
        let outHandle = outPipe.fileHandleForReading
        let group = DispatchGroup()
        group.enter()
        DispatchQueue.global().async {
            outBox.data = outHandle.readDataToEndOfFile()
            group.leave()
        }
        let errData = errPipe.fileHandleForReading.readDataToEndOfFile()
        group.wait()
        proc.waitUntilExit()

        guard proc.terminationStatus == 0 else {
            let msg = String(data: errData, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
            throw CLIError.failed(status: proc.terminationStatus, stderr: msg)
        }
        return outBox.data
    }

    /// 加载全量书签（`ls --json`）。CLI 保证 --json 失败以非零码报错，不吐半截 JSON。
    func loadAll() throws -> [Bookmark] {
        let data = try run(["ls", "--json"])
        return try JSONDecoder().decode(BookmarkStore.self, from: data).bookmarks
    }

    // MARK: - 写操作（全部经 CLI，App 不复刻锁/原子写）

    /// 新增书签，返回新 id（从 CLI stdout "已添加 #<id>" 解析）。
    @discardableResult
    func add(url: String, title: String?, folder: String?, note: String?) throws -> Int64? {
        var args = ["add", url]
        if let t = title, !t.isEmpty { args += ["--title", t] }
        if let f = folder, !f.isEmpty { args += ["--folder", f] }
        if let n = note, !n.isEmpty { args += ["--note", n] }
        let out = try run(args)
        let s = String(data: out, encoding: .utf8) ?? ""
        guard let hash = s.firstIndex(of: "#") else { return nil }
        let digits = s[s.index(after: hash)...].prefix { $0.isNumber }
        return Int64(digits)
    }

    /// 后台抓取元数据（best-effort，失败忽略）。
    func fetch(id: Int64) throws { try run(["fetch", String(id)]) }

    func edit(id: Int64, title: String, url: String, excerpt: String, note: String, folder: String) throws {
        // 编辑面板一次提交全部字段（含清空为 ""）。
        try run(["edit", String(id),
                 "--title", title, "--url", url, "--excerpt", excerpt,
                 "--note", note, "--folder", folder])
    }

    func remove(id: Int64) throws { try run(["rm", String(id)]) }

    func open(id: Int64) throws { try run(["open", String(id)]) }

    func moveFolder(from old: String, to new: String) throws { try run(["mv", old, new]) }
}
