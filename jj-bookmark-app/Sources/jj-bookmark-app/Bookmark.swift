import Foundation

// 与 data-model §3 对齐的只读模型（App 侧解码共享数据文件用于展示）。
// nonisolated：纯值类型，可在任意线程解码/传递（避开 defaultIsolation(MainActor) 限制）。
nonisolated struct Bookmark: Identifiable, Sendable, Hashable {
    var id: Int64
    var source: String
    var title: String
    var url: String
    var excerpt: String
    var note: String
    var folder: String
    var created: Int64
    var createdJst: String
    var updated: Int64
    var updatedJst: String
    var lastVisited: Int64
    var lastVisitedJst: String
}

extension Bookmark: Decodable {
    enum CodingKeys: String, CodingKey {
        case id, source, title, url, excerpt, note, folder
        case created
        case createdJst = "created_jst"
        case updated
        case updatedJst = "updated_jst"
        case lastVisited = "last_visited"
        case lastVisitedJst = "last_visited_jst"
    }

    // 逐字段容错解码：缺失字段兜底默认值，与契约「读取方可容错缺字段」一致。
    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        id = try c.decodeIfPresent(Int64.self, forKey: .id) ?? 0
        source = try c.decodeIfPresent(String.self, forKey: .source) ?? "default"
        title = try c.decodeIfPresent(String.self, forKey: .title) ?? ""
        url = try c.decodeIfPresent(String.self, forKey: .url) ?? ""
        excerpt = try c.decodeIfPresent(String.self, forKey: .excerpt) ?? ""
        note = try c.decodeIfPresent(String.self, forKey: .note) ?? ""
        folder = try c.decodeIfPresent(String.self, forKey: .folder) ?? ""
        created = try c.decodeIfPresent(Int64.self, forKey: .created) ?? 0
        createdJst = try c.decodeIfPresent(String.self, forKey: .createdJst) ?? ""
        updated = try c.decodeIfPresent(Int64.self, forKey: .updated) ?? 0
        updatedJst = try c.decodeIfPresent(String.self, forKey: .updatedJst) ?? ""
        lastVisited = try c.decodeIfPresent(Int64.self, forKey: .lastVisited) ?? 0
        lastVisitedJst = try c.decodeIfPresent(String.self, forKey: .lastVisitedJst) ?? ""
    }
}

// 顶层契约 { version, sources: { name: [...] } }（数据文件与 CLI `--json` 同构）；source 注入内存模型。
nonisolated struct BookmarkStore: Sendable {
    var version: Int
    var bookmarks: [Bookmark]
}

nonisolated extension BookmarkStore: Decodable {
    enum CodingKeys: String, CodingKey { case version, sources, bookmarks }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        version = try container.decodeIfPresent(Int.self, forKey: .version) ?? 0
        if let grouped = try container.decodeIfPresent([String: [Bookmark]].self, forKey: .sources) {
            bookmarks = grouped.keys.sorted().flatMap { source in
                grouped[source, default: []].map { bookmark in
                    var bookmark = bookmark
                    bookmark.source = source
                    return bookmark
                }
            }
        } else {
            bookmarks = try container.decodeIfPresent([Bookmark].self, forKey: .bookmarks) ?? []
        }
    }
}

extension Bookmark {
    /// 从 URL 提取用于列表展示的域名（去 scheme / www / 路径）。
    var domain: String {
        guard let host = URLComponents(string: url)?.host else { return url }
        return host.hasPrefix("www.") ? String(host.dropFirst(4)) : host
    }

    /// 多关键词搜索：按 Unicode 空白分词；全部词均须命中任意可搜索字段。
    func matchesSearch(_ query: String) -> Bool {
        let terms = query.split(whereSeparator: { $0.isWhitespace }).map { $0.lowercased() }
        if terms.isEmpty { return true }
        let searchable = [source, title, url, excerpt, note, folder]
            .joined(separator: " ")
            .lowercased()
        return terms.allSatisfy { searchable.contains($0) }
    }
}

// 读侧集成面 = 共享数据文件本身：原子 rename 保证永远读到某个完整版本（CLI store.rs），
// 故只读消费者无需加锁、无需经 CLI。写操作仍全部经 CLI（锁 / 原子写 / 校验不复刻）。
nonisolated extension BookmarkStore {
    /// App 支持的最高 schema version（对齐 CLI `CURRENT_VERSION`）。
    static let supportedVersion = 3

    enum LoadError: LocalizedError {
        case versionTooNew(Int)

        var errorDescription: String? {
            switch self {
            case .versionTooNew(let v):
                return "data file version \(v) is newer than this app supports (\(BookmarkStore.supportedVersion))"
            }
        }
    }

    /// 全量加载；文件不存在 = 空库。
    static func loadFromDisk() throws -> [Bookmark] {
        let url = AppPaths.dataDirectory().appendingPathComponent("bookmarks.json")
        guard FileManager.default.fileExists(atPath: url.path) else { return [] }
        let store = try JSONDecoder().decode(Self.self, from: try Data(contentsOf: url))
        guard store.version <= supportedVersion else {
            throw LoadError.versionTooNew(store.version)
        }
        return store.bookmarks
    }
}
