import Foundation

// source 是第一层，folder 是 source 内部层级；path 永远只保存真实 folder 路径。
final class FolderNode {
    enum Kind { case all, source, normal, uncategorized }

    let kind: Kind
    let name: String
    let source: String?
    let path: String
    var children: [FolderNode] = []
    var count = 0

    init(kind: Kind, name: String, source: String?, path: String) {
        self.kind = kind
        self.name = name
        self.source = source
        self.path = path
    }

    var stateKey: String {
        let kindKey: String
        switch kind {
        case .all: kindKey = "all"
        case .source: kindKey = "source"
        case .normal: kindKey = "folder"
        case .uncategorized: kindKey = "uncategorized"
        }
        return "\(kindKey)|\(source ?? "")|\(path)"
    }

    func matches(_ bookmark: Bookmark) -> Bool {
        switch kind {
        case .all:
            return true
        case .source:
            return bookmark.source == source
        case .uncategorized:
            return bookmark.source == source && bookmark.folder.isEmpty
        case .normal:
            return bookmark.source == source
                && (bookmark.folder == path || bookmark.folder.hasPrefix(path + " / "))
        }
    }
}

enum FolderTree {
    static func build(from bookmarks: [Bookmark]) -> [FolderNode] {
        let all = FolderNode(kind: .all, name: L10n.folderAll, source: nil, path: "")
        all.count = bookmarks.count

        let grouped = Dictionary(grouping: bookmarks, by: \Bookmark.source)
        let sourceNodes = grouped.keys.sorted {
            $0.localizedStandardCompare($1) == .orderedAscending
        }.map { source in
            buildSource(source, bookmarks: grouped[source, default: []])
        }
        return [all] + sourceNodes
    }

    private static func buildSource(_ source: String, bookmarks: [Bookmark]) -> FolderNode {
        let root = FolderNode(kind: .source, name: source, source: source, path: "")
        root.count = bookmarks.count

        var direct: [String: Int] = [:]
        var uncategorized = 0
        for bookmark in bookmarks {
            if bookmark.folder.isEmpty {
                uncategorized += 1
            } else {
                direct[bookmark.folder, default: 0] += 1
            }
        }

        var nodes: [String: FolderNode] = [:]
        func ensure(_ path: String) -> FolderNode {
            if let node = nodes[path] { return node }
            let name = path.components(separatedBy: " / ").last ?? path
            let node = FolderNode(kind: .normal, name: name, source: source, path: path)
            nodes[path] = node
            return node
        }

        var topLevel: [FolderNode] = []
        for fullPath in direct.keys {
            var prefix = ""
            var parent: FolderNode?
            for (index, part) in fullPath.components(separatedBy: " / ").enumerated() {
                prefix = index == 0 ? part : prefix + " / " + part
                let node = ensure(prefix)
                if let parent {
                    if !parent.children.contains(where: { $0.path == node.path }) {
                        parent.children.append(node)
                    }
                } else if !topLevel.contains(where: { $0.path == node.path }) {
                    topLevel.append(node)
                }
                parent = node
            }
        }

        func computeCount(_ node: FolderNode) -> Int {
            let count = (direct[node.path] ?? 0)
                + node.children.reduce(0) { $0 + computeCount($1) }
            node.count = count
            return count
        }
        topLevel.forEach { _ = computeCount($0) }

        func sortRecursively(_ node: FolderNode) {
            node.children.sort { $0.name.localizedStandardCompare($1.name) == .orderedAscending }
            node.children.forEach(sortRecursively)
        }
        topLevel.sort { $0.name.localizedStandardCompare($1.name) == .orderedAscending }
        topLevel.forEach(sortRecursively)
        root.children = topLevel

        if uncategorized > 0 {
            let node = FolderNode(
                kind: .uncategorized,
                name: L10n.folderUncategorized,
                source: source,
                path: ""
            )
            node.count = uncategorized
            root.children.append(node)
        }
        return root
    }
}
