import Foundation

// 数据目录解析，与 CLI 一致：优先 JJ_BOOKMARK_DIR，否则 ~/.config/jj-bookmark。
nonisolated enum AppPaths {
    static func dataDirectory() -> URL {
        if let d = ProcessInfo.processInfo.environment["JJ_BOOKMARK_DIR"], !d.isEmpty {
            return URL(fileURLWithPath: d, isDirectory: true)
        }
        return FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".config/jj-bookmark", isDirectory: true)
    }
}
