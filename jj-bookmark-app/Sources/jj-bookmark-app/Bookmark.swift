import Foundation

// 与 data-model §3 对齐的只读模型（App 侧仅解码 CLI `ls --json` 输出用于展示）。
// nonisolated：纯值类型，可在任意线程解码/传递（避开 defaultIsolation(MainActor) 限制）。
nonisolated struct Bookmark: Identifiable, Sendable, Hashable {
    var id: Int64
    var source: String
    var title: String
    var url: String
    var excerpt: String
    var note: String
    var folder: String
    var cover: String
    var tags: [String]
    var favorite: Bool
    var created: Int64
    var createdJst: String
    var updated: Int64
    var updatedJst: String
    var lastVisited: Int64
    var lastVisitedJst: String
}

extension Bookmark: Decodable {
    enum CodingKeys: String, CodingKey {
        case id, source, title, url, excerpt, note, folder, cover, tags, favorite
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
        cover = try c.decodeIfPresent(String.self, forKey: .cover) ?? ""
        tags = try c.decodeIfPresent([String].self, forKey: .tags) ?? []
        favorite = try c.decodeIfPresent(Bool.self, forKey: .favorite) ?? false
        created = try c.decodeIfPresent(Int64.self, forKey: .created) ?? 0
        createdJst = try c.decodeIfPresent(String.self, forKey: .createdJst) ?? ""
        updated = try c.decodeIfPresent(Int64.self, forKey: .updated) ?? 0
        updatedJst = try c.decodeIfPresent(String.self, forKey: .updatedJst) ?? ""
        lastVisited = try c.decodeIfPresent(Int64.self, forKey: .lastVisited) ?? 0
        lastVisitedJst = try c.decodeIfPresent(String.self, forKey: .lastVisitedJst) ?? ""
    }
}

// CLI `--json` 顶层契约 { version, bookmarks }（data-model §11）。
nonisolated struct BookmarkStore: Decodable, Sendable {
    var version: Int
    var bookmarks: [Bookmark]
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
        let searchable = [source, title, url, excerpt, note, folder, tags.joined(separator: " ")]
            .joined(separator: " ")
            .lowercased()
        return terms.allSatisfy { searchable.contains($0) }
    }
}
