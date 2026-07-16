import Foundation

// 数据目录：固定 ~/.config/jj-bookmark（与 CLI 默认一致，单一位置）。
nonisolated enum AppPaths {
    static func dataDirectory() -> URL {
        FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".config/jj-bookmark", isDirectory: true)
    }
}
