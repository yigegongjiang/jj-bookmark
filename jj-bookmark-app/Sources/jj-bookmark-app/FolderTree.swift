import Foundation

// folder 树节点（NSOutlineView 的 item，引用类型）。由扁平路径构建（data-model §5）。
final class FolderNode {
    enum Kind { case all, normal, uncategorized }

    let kind: Kind
    let name: String // 展示名（路径末段）
    let path: String // 完整路径 "A / B"；all/uncategorized 为 ""，靠 kind 区分
    var children: [FolderNode] = []
    var count: Int = 0 // 子树书签总数

    init(kind: Kind, name: String, path: String) {
        self.kind = kind
        self.name = name
        self.path = path
    }

    /// 该节点是否命中某书签（选中节点即用此过滤右侧列表）。
    func matches(_ b: Bookmark) -> Bool {
        switch kind {
        case .all: return true
        case .uncategorized: return b.folder.isEmpty
        case .normal: return b.folder == path || b.folder.hasPrefix(path + " / ")
        }
    }
}

enum FolderTree {
    /// 从书签的扁平 folder 路径构建树：`全部` + 正常层级 + `未分类`。
    static func build(from bookmarks: [Bookmark]) -> [FolderNode] {
        var direct: [String: Int] = [:] // 每个精确路径的直接书签数
        var uncategorized = 0
        for b in bookmarks {
            if b.folder.isEmpty {
                uncategorized += 1
            } else {
                direct[b.folder, default: 0] += 1
            }
        }

        var nodes: [String: FolderNode] = [:]
        func ensure(_ path: String) -> FolderNode {
            if let n = nodes[path] { return n }
            let name = path.components(separatedBy: " / ").last ?? path
            let n = FolderNode(kind: .normal, name: name, path: path)
            nodes[path] = n
            return n
        }

        var topLevel: [FolderNode] = []
        for fullPath in direct.keys {
            var prefix = ""
            var parent: FolderNode?
            for (i, part) in fullPath.components(separatedBy: " / ").enumerated() {
                prefix = i == 0 ? part : prefix + " / " + part
                let node = ensure(prefix)
                if let p = parent {
                    if !p.children.contains(where: { $0.path == node.path }) {
                        p.children.append(node)
                    }
                } else if !topLevel.contains(where: { $0.path == node.path }) {
                    topLevel.append(node)
                }
                parent = node
            }
        }

        // 后序计算子树总数
        func computeCount(_ n: FolderNode) -> Int {
            let c = (direct[n.path] ?? 0) + n.children.reduce(0) { $0 + computeCount($1) }
            n.count = c
            return c
        }
        topLevel.forEach { _ = computeCount($0) }

        // 本地化名称排序
        func sortRec(_ n: FolderNode) {
            n.children.sort { $0.name.localizedStandardCompare($1.name) == .orderedAscending }
            n.children.forEach(sortRec)
        }
        topLevel.sort { $0.name.localizedStandardCompare($1.name) == .orderedAscending }
        topLevel.forEach(sortRec)

        var result: [FolderNode] = []
        let all = FolderNode(kind: .all, name: "全部", path: "")
        all.count = bookmarks.count
        result.append(all)
        result.append(contentsOf: topLevel)
        if uncategorized > 0 {
            let u = FolderNode(kind: .uncategorized, name: "未分类", path: "")
            u.count = uncategorized
            result.append(u)
        }
        return result
    }
}
