import Foundation

// 排序键与方向，语义镜像 CLI（data-model §10）。App 侧即时重排在内存原生做。
nonisolated enum SortKey: String, CaseIterable, Sendable {
    case created, updated, visited, title

    var label: String {
        switch self {
        case .created: return L10n.sortCreated
        case .updated: return L10n.sortUpdated
        case .visited: return L10n.sortVisited
        case .title: return L10n.sortTitle
        }
    }

    /// 各键默认方向：title 升序，其余降序。
    var defaultOrder: SortOrder { self == .title ? .asc : .desc }
}

nonisolated enum SortOrder: Sendable {
    case asc, desc
}

nonisolated enum Sorting {
    /// 原地排序。次级键固定 id 降序，保证确定性（与 CLI 一致）。
    /// 三个数字键逐条一致；title 为 best-effort（localizedStandardCompare，ICU 本地化）。
    static func sort(_ bms: inout [Bookmark], key: SortKey, order: SortOrder) {
        bms.sort { a, b in
            let cmp: ComparisonResult
            switch key {
            case .created: cmp = compare(a.created, b.created)
            case .updated: cmp = compare(a.updated, b.updated)
            case .visited: cmp = compare(a.lastVisited, b.lastVisited)
            case .title: cmp = a.title.localizedStandardCompare(b.title)
            }
            let primary = order == .asc ? cmp : cmp.reversed
            if primary != .orderedSame {
                return primary == .orderedAscending
            }
            return a.id > b.id // 次级键：id 降序
        }
    }

    private static func compare(_ a: Int64, _ b: Int64) -> ComparisonResult {
        a < b ? .orderedAscending : (a > b ? .orderedDescending : .orderedSame)
    }
}

private extension ComparisonResult {
    nonisolated var reversed: ComparisonResult {
        switch self {
        case .orderedAscending: return .orderedDescending
        case .orderedDescending: return .orderedAscending
        case .orderedSame: return .orderedSame
        }
    }
}
