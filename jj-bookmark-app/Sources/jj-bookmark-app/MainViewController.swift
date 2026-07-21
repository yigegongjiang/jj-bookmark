import AppKit

// 三栏主界面：左 folder 树（NSOutlineView）+ 右书签列表（NSTableView）+ 顶部搜索/排序。
// 数据经内嵌 CLI 全量加载，即时搜索/排序在内存原生做；FSEvents 变更自动刷新。
final class MainViewController: NSViewController, NSMenuItemValidation {
    private let runner: CLIRunner
    private var watcher: FSWatcher?

    // 数据
    private var allBookmarks: [Bookmark] = []
    private var folderRoots: [FolderNode] = []
    private var visible: [Bookmark] = []

    // 状态
    private var selectedFolder: FolderNode?
    private var searchText = ""
    private var sortKey: SortKey = .created
    private var sortOrder: SortOrder = .desc
    private var isRestoring = false

    // 视图
    private let outlineView = NSOutlineView()
    private let tableView = BookmarkTableView()
    private let splitView = NSSplitView()
    private let sidebarScroll = NSScrollView()
    private let searchField = NSSearchField()
    private let sortPopup = NSPopUpButton()
    private let orderButton = NSButton()
    private let newButton = NSButton()
    private let statusLabel = NSTextField(labelWithString: "")

    // 分栏尺寸约束（左侧 folder 栏固定宽 + 两栏各自最小宽，防被拖成 0）
    private static let sidebarWidthKey = "JJBookmark.sidebarWidth"
    private static let sidebarExpandedItemsKey = "JJBookmark.sidebarExpandedItems"
    private static let sidebarSelectedItemKey = "JJBookmark.sidebarSelectedItem"
    private let defaultSidebarWidth: CGFloat = 200
    private let sidebarMinWidth: CGFloat = 160
    private let contentMinWidth: CGFloat = 360
    private var didSetInitialSplitPosition = false
    private var didLoadSidebarState = false

    init(runner: CLIRunner) {
        self.runner = runner
        super.init(nibName: nil, bundle: nil)
    }

    @available(*, unavailable)
    required init?(coder _: NSCoder) { fatalError("not used") }

    // MARK: - 生命周期

    override func loadView() {
        let root = NSView(frame: NSRect(x: 0, y: 0, width: 1000, height: 640))
        let toolbar = makeToolbar()
        let split = makeSplitView()
        toolbar.translatesAutoresizingMaskIntoConstraints = false
        split.translatesAutoresizingMaskIntoConstraints = false
        root.addSubview(toolbar)
        root.addSubview(split)
        NSLayoutConstraint.activate([
            toolbar.topAnchor.constraint(equalTo: root.topAnchor),
            toolbar.leadingAnchor.constraint(equalTo: root.leadingAnchor),
            toolbar.trailingAnchor.constraint(equalTo: root.trailingAnchor),
            toolbar.heightAnchor.constraint(equalToConstant: 44),
            split.topAnchor.constraint(equalTo: toolbar.bottomAnchor),
            split.leadingAnchor.constraint(equalTo: root.leadingAnchor),
            split.trailingAnchor.constraint(equalTo: root.trailingAnchor),
            split.bottomAnchor.constraint(equalTo: root.bottomAnchor),
        ])
        view = root
    }

    override func viewDidLoad() {
        super.viewDidLoad()
        outlineView.dataSource = self
        outlineView.delegate = self
        tableView.dataSource = self
        tableView.delegate = self
        searchField.delegate = self

        tableView.onEnter = { [weak self] in self?.openSelected() }
        tableView.onDelete = { [weak self] in self?.deleteSelected() }
        tableView.menu = makeTableContextMenu()
        outlineView.menu = makeOutlineContextMenu()

        loadSortPreference()
        syncSortControls()
        reload()

        // FSEvents：终端/其他进程改数据后自动刷新（保留选中+滚动）。
        let watcher = FSWatcher(directory: AppPaths.dataDirectory()) { [weak self] in
            self?.reload()
        }
        watcher.start()
        self.watcher = watcher
    }

    override func viewDidAppear() {
        super.viewDidAppear()
        // 窗口已定尺寸后恢复分隔条：无有效记录时用默认宽，之后尊重用户拖动。
        if !didSetInitialSplitPosition {
            didSetInitialSplitPosition = true
            let savedWidth = CGFloat(UserDefaults.standard.double(forKey: Self.sidebarWidthKey))
            let preferredWidth = savedWidth.isFinite && savedWidth >= sidebarMinWidth
                ? savedWidth : defaultSidebarWidth
            let maxWidth = max(sidebarMinWidth, splitView.bounds.width - contentMinWidth)
            splitView.setPosition(min(preferredWidth, maxWidth), ofDividerAt: 0)
        }
    }

    // MARK: - UI 构建

    private func makeToolbar() -> NSView {
        let bar = NSView()

        searchField.placeholderString = L10n.searchPlaceholder
        searchField.translatesAutoresizingMaskIntoConstraints = false

        for key in SortKey.allCases {
            sortPopup.addItem(withTitle: key.label)
            sortPopup.lastItem?.representedObject = key
        }
        sortPopup.target = self
        sortPopup.action = #selector(sortKeyChanged)
        sortPopup.translatesAutoresizingMaskIntoConstraints = false

        orderButton.bezelStyle = .rounded
        orderButton.target = self
        orderButton.action = #selector(toggleOrder)
        orderButton.translatesAutoresizingMaskIntoConstraints = false
        updateOrderButtonTitle()

        newButton.bezelStyle = .rounded
        newButton.title = L10n.toolbarNew
        newButton.target = self
        newButton.action = #selector(newBookmark)
        newButton.translatesAutoresizingMaskIntoConstraints = false

        bar.addSubview(searchField)
        bar.addSubview(sortPopup)
        bar.addSubview(orderButton)
        bar.addSubview(newButton)
        NSLayoutConstraint.activate([
            searchField.leadingAnchor.constraint(equalTo: bar.leadingAnchor, constant: 12),
            searchField.centerYAnchor.constraint(equalTo: bar.centerYAnchor),
            searchField.widthAnchor.constraint(greaterThanOrEqualToConstant: 220),
            sortPopup.leadingAnchor.constraint(equalTo: searchField.trailingAnchor, constant: 12),
            sortPopup.centerYAnchor.constraint(equalTo: bar.centerYAnchor),
            orderButton.leadingAnchor.constraint(equalTo: sortPopup.trailingAnchor, constant: 8),
            orderButton.centerYAnchor.constraint(equalTo: bar.centerYAnchor),
            newButton.leadingAnchor.constraint(greaterThanOrEqualTo: orderButton.trailingAnchor, constant: 8),
            newButton.trailingAnchor.constraint(equalTo: bar.trailingAnchor, constant: -12),
            newButton.centerYAnchor.constraint(equalTo: bar.centerYAnchor),
        ])
        return bar
    }

    private func makeSplitView() -> NSView {
        // 左：folder 树
        let folderColumn = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("folder"))
        folderColumn.title = L10n.columnFolder
        outlineView.addTableColumn(folderColumn)
        outlineView.outlineTableColumn = folderColumn
        outlineView.headerView = nil
        outlineView.rowSizeStyle = .default
        outlineView.autosaveExpandedItems = false
        outlineView.indentationPerLevel = 14
        sidebarScroll.documentView = outlineView
        sidebarScroll.hasVerticalScroller = true
        sidebarScroll.autohidesScrollers = true

        // 右：书签列表 + 底部状态条
        let bmColumn = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("bookmark"))
        bmColumn.title = L10n.columnBookmark
        tableView.addTableColumn(bmColumn)
        tableView.headerView = nil
        tableView.rowHeight = 46
        tableView.usesAlternatingRowBackgroundColors = true
        tableView.doubleAction = #selector(openSelected)
        tableView.target = self
        let rightScroll = NSScrollView()
        rightScroll.documentView = tableView
        rightScroll.hasVerticalScroller = true
        rightScroll.autohidesScrollers = true

        statusLabel.font = .systemFont(ofSize: 11)
        statusLabel.textColor = .secondaryLabelColor
        let rightContainer = NSView()
        rightScroll.translatesAutoresizingMaskIntoConstraints = false
        statusLabel.translatesAutoresizingMaskIntoConstraints = false
        rightContainer.addSubview(rightScroll)
        rightContainer.addSubview(statusLabel)
        NSLayoutConstraint.activate([
            rightScroll.topAnchor.constraint(equalTo: rightContainer.topAnchor),
            rightScroll.leadingAnchor.constraint(equalTo: rightContainer.leadingAnchor),
            rightScroll.trailingAnchor.constraint(equalTo: rightContainer.trailingAnchor),
            statusLabel.topAnchor.constraint(equalTo: rightScroll.bottomAnchor, constant: 2),
            statusLabel.leadingAnchor.constraint(equalTo: rightContainer.leadingAnchor, constant: 10),
            statusLabel.bottomAnchor.constraint(equalTo: rightContainer.bottomAnchor, constant: -4),
        ])

        splitView.isVertical = true
        splitView.dividerStyle = .thin
        splitView.delegate = self
        splitView.addArrangedSubview(sidebarScroll)
        splitView.addArrangedSubview(rightContainer)
        // 两栏都给非零初始宽：NSSplitView 首次按比例分配，若右栏宽 0 会被永久压成 0（右侧列表消失）。
        sidebarScroll.setFrameSize(NSSize(width: defaultSidebarWidth, height: 600))
        rightContainer.setFrameSize(NSSize(width: 800, height: 600))
        return splitView
    }

    // MARK: - 加载 / 刷新

    private func reload() {
        let defaults = UserDefaults.standard
        let snapshot = didLoadSidebarState
            ? captureExpansion()
            : Set(defaults.stringArray(forKey: Self.sidebarExpandedItemsKey) ?? [])
        let prevFolder = selectedFolder?.stateKey
            ?? defaults.string(forKey: Self.sidebarSelectedItemKey)
        do {
            allBookmarks = try runner.loadAll()
        } catch {
            showError(error, title: L10n.errorLoadFailed)
            return
        }
        folderRoots = FolderTree.build(from: allBookmarks)

        isRestoring = true
        outlineView.reloadData()
        restoreExpansion(snapshot)
        restoreFolderSelection(prevFolder)
        isRestoring = false
        didLoadSidebarState = true
        saveSidebarState()

        recomputeVisible()
    }

    /// 重算可见列表（folder + 搜索过滤 → 排序），并按 id 保留选中与顶部滚动位置。
    private func recomputeVisible() {
        let keepIDs = currentSelectionIDs()
        let topID = currentTopVisibleID()

        var list = allBookmarks
        if let folder = selectedFolder {
            list = list.filter { folder.matches($0) }
        }
        if !searchText.isEmpty {
            list = list.filter { $0.matchesSearch(searchText) }
        }
        Sorting.sort(&list, key: sortKey, order: sortOrder)
        visible = list
        tableView.reloadData()

        restoreTableSelection(keepIDs)
        restoreTableScroll(topID)
        updateStatus()
    }

    private func updateStatus() {
        let total = allBookmarks.count
        let shown = visible.count
        statusLabel.stringValue = shown == total
            ? L10n.statusTotal(total)
            : L10n.statusFiltered(shown: shown, total: total)
    }

    // MARK: - 状态捕获 / 恢复（按稳定 id / path，非行号）

    private func currentSelectionIDs() -> Set<Int64> {
        Set(tableView.selectedRowIndexes.compactMap {
            visible.indices.contains($0) ? visible[$0].id : nil
        })
    }

    private func currentTopVisibleID() -> Int64? {
        let range = tableView.rows(in: tableView.visibleRect)
        guard range.length > 0, visible.indices.contains(range.location) else { return nil }
        return visible[range.location].id
    }

    private func restoreTableSelection(_ ids: Set<Int64>) {
        guard !ids.isEmpty else { return }
        let rows = IndexSet(visible.indices.filter { ids.contains(visible[$0].id) })
        if !rows.isEmpty { tableView.selectRowIndexes(rows, byExtendingSelection: false) }
    }

    private func restoreTableScroll(_ topID: Int64?) {
        guard let topID, let idx = visible.firstIndex(where: { $0.id == topID }) else {
            if !visible.isEmpty { tableView.scrollRowToVisible(0) }
            return
        }
        let y = tableView.rect(ofRow: idx).minY
        tableView.scroll(NSPoint(x: 0, y: y))
    }

    private func captureExpansion() -> Set<String> {
        var keys = Set<String>()
        func walk(_ nodes: [FolderNode]) {
            for n in nodes {
                if !n.children.isEmpty, outlineView.isItemExpanded(n) { keys.insert(n.stateKey) }
                walk(n.children)
            }
        }
        walk(folderRoots)
        return keys
    }

    private func restoreExpansion(_ keys: Set<String>) {
        func walk(_ nodes: [FolderNode]) {
            for n in nodes {
                if keys.contains(n.stateKey) { outlineView.expandItem(n) }
                walk(n.children)
            }
        }
        walk(folderRoots)
    }

    private func saveSidebarState() {
        let defaults = UserDefaults.standard
        defaults.set(captureExpansion().sorted(), forKey: Self.sidebarExpandedItemsKey)
        if let key = selectedFolder?.stateKey {
            defaults.set(key, forKey: Self.sidebarSelectedItemKey)
        } else {
            defaults.removeObject(forKey: Self.sidebarSelectedItemKey)
        }
    }

    private func restoreFolderSelection(_ previousKey: String?) {
        let target: FolderNode?
        if let previousKey, let found = findFolder(stateKey: previousKey) {
            target = found
        } else {
            target = folderRoots.first // 默认「全部」（或之前的 folder 已不存在）
        }
        selectedFolder = target
        if let target {
            let row = outlineView.row(forItem: target)
            if row >= 0 {
                outlineView.selectRowIndexes([row], byExtendingSelection: false)
            }
        }
    }

    private func findFolder(stateKey: String) -> FolderNode? {
        func search(_ nodes: [FolderNode]) -> FolderNode? {
            for n in nodes {
                if n.stateKey == stateKey { return n }
                if let f = search(n.children) { return f }
            }
            return nil
        }
        return search(folderRoots)
    }

    // MARK: - 动作

    @objc private func sortKeyChanged() {
        guard let key = sortPopup.selectedItem?.representedObject as? SortKey else { return }
        sortKey = key
        sortOrder = key.defaultOrder
        updateOrderButtonTitle()
        saveSortPreference()
        recomputeVisible()
    }

    @objc private func toggleOrder() {
        sortOrder = sortOrder == .asc ? .desc : .asc
        updateOrderButtonTitle()
        saveSortPreference()
        recomputeVisible()
    }

    private func updateOrderButtonTitle() {
        orderButton.title = sortOrder == .asc ? L10n.orderAscending : L10n.orderDescending
    }

    private func syncSortControls() {
        if let idx = SortKey.allCases.firstIndex(of: sortKey) {
            sortPopup.selectItem(at: idx)
        }
        updateOrderButtonTitle()
    }

    // MARK: - 排序偏好记忆（UserDefaults）

    private func loadSortPreference() {
        let d = UserDefaults.standard
        if let raw = d.string(forKey: "sortKey"), let k = SortKey(rawValue: raw) {
            sortKey = k
        }
        if let o = d.string(forKey: "sortOrder") {
            sortOrder = o == "asc" ? .asc : .desc
        } else {
            sortOrder = sortKey.defaultOrder
        }
    }

    private func saveSortPreference() {
        let d = UserDefaults.standard
        d.set(sortKey.rawValue, forKey: "sortKey")
        d.set(sortOrder == .asc ? "asc" : "desc", forKey: "sortOrder")
    }

    // MARK: - 编辑动作（全部经 CLI，写后自刷新；FSEvents 亦会合并触发）

    @objc func newBookmark() {
        // 只在选中的是叶子 folder（无子节点）时预填其路径：书签只能挂叶子，
        // 预填非叶路径会被 CLI 拒绝。非叶 / 非 folder 节点留空由用户填。
        let sel = selectedFolder
        let folderDefault = (sel?.kind == .normal && sel?.children.isEmpty == true)
            ? (sel?.path ?? "") : ""
        let source = selectedFolder?.source ?? "default"
        guard let v = runForm(title: L10n.formNewTitle, okTitle: L10n.btnAdd, fields: [
            ("URL", "", "https://…"),
            (L10n.fieldTitle, "", L10n.placeholderTitleHint),
            (L10n.fieldFolder, folderDefault, "A / B"),
        ]) else { return }
        let url = v[0].trimmingCharacters(in: .whitespacesAndNewlines)
        guard !url.isEmpty else { return }
        performWrite {
            let newID = try runner.add(
                source: source,
                url: url,
                title: v[1],
                folder: v[2],
                note: nil
            )
            if let id = newID { backgroundFetch(id: id) } // 后台补全元数据，完成后 FSEvents 自刷新
        }
    }

    /// 后台抓取元数据（不阻塞 UI）；写入后经 FSEvents 触发刷新回填该行。
    private func backgroundFetch(id: Int64) {
        let runner = self.runner
        Task.detached {
            try? runner.fetch(id: id)
        }
    }

    @objc func editSelected() {
        guard let b = targetBookmarks().first else { return }
        guard let v = runForm(title: L10n.formEditTitle, okTitle: L10n.btnSave, fields: [
            (L10n.fieldTitle, b.title, ""),
            ("URL", b.url, ""),
            (L10n.fieldDescription, b.excerpt, ""),
            (L10n.fieldNote, b.note, ""),
            (L10n.fieldFolder, b.folder, ""),
        ]) else { return }
        performWrite {
            try runner.edit(id: b.id, title: v[0], url: v[1], excerpt: v[2], note: v[3], folder: v[4])
        }
    }

    @objc func deleteSelected() {
        let bms = targetBookmarks()
        guard !bms.isEmpty else { return }
        let alert = NSAlert()
        alert.messageText = bms.count == 1
            ? L10n.deleteConfirmOne(displayTitle(bms[0]))
            : L10n.deleteConfirmMany(bms.count)
        alert.informativeText = L10n.deleteIrreversible
        alert.addButton(withTitle: L10n.btnDelete)
        alert.addButton(withTitle: L10n.btnCancel)
        alert.alertStyle = .warning
        guard alert.runModal() == .alertFirstButtonReturn else { return }
        performWrite { for b in bms { try runner.remove(id: b.id) } }
    }

    @objc func openSelected() {
        let bms = targetBookmarks()
        guard !bms.isEmpty else { return }
        do {
            for b in bms { try runner.open(id: b.id) }
            reload()
            NSApp.hide(nil)
        } catch {
            showError(error, title: L10n.errorOperationFailed)
        }
    }

    @objc func focusSearch() {
        view.window?.makeFirstResponder(searchField)
    }

    @objc func refresh() {
        reload()
    }

    @objc private func renameSelectedFolder() {
        guard let node = clickedFolder(), node.kind == .normal, let source = node.source else { return }
        guard let v = runForm(title: L10n.formRenameTitle, okTitle: L10n.btnRename, fields: [
            (L10n.fieldNewPath, node.path, "A / B"),
        ]) else { return }
        let newPath = v[0].trimmingCharacters(in: .whitespacesAndNewlines)
        guard !newPath.isEmpty, newPath != node.path else { return }
        performWrite { try runner.moveFolder(source: source, from: node.path, to: newPath) }
    }

    /// 运行 CLI 写操作 → 成功即刷新；失败弹错。
    private func performWrite(_ action: () throws -> Void) {
        do {
            try action()
            reload()
        } catch {
            showError(error, title: L10n.errorOperationFailed)
        }
    }

    // MARK: - 选择目标

    /// 右键点击行优先（若在选区内则整选区），否则用当前选区。
    private func targetBookmarks() -> [Bookmark] {
        let clicked = tableView.clickedRow
        if clicked >= 0, visible.indices.contains(clicked) {
            if tableView.selectedRowIndexes.contains(clicked) { return selectedBookmarks() }
            return [visible[clicked]]
        }
        return selectedBookmarks()
    }

    private func selectedBookmarks() -> [Bookmark] {
        tableView.selectedRowIndexes.compactMap { visible.indices.contains($0) ? visible[$0] : nil }
    }

    private func clickedFolder() -> FolderNode? {
        let row = outlineView.clickedRow >= 0 ? outlineView.clickedRow : outlineView.selectedRow
        return row >= 0 ? (outlineView.item(atRow: row) as? FolderNode) : nil
    }

    private func displayTitle(_ b: Bookmark) -> String { b.title.isEmpty ? b.url : b.title }

    // MARK: - 上下文菜单

    private func makeTableContextMenu() -> NSMenu {
        let menu = NSMenu()
        menu.addItem(withTitle: L10n.menuOpen, action: #selector(openSelected), keyEquivalent: "")
        menu.addItem(withTitle: L10n.menuEditItem, action: #selector(editSelected), keyEquivalent: "")
        menu.addItem(.separator())
        menu.addItem(withTitle: L10n.menuDelete, action: #selector(deleteSelected), keyEquivalent: "")
        menu.items.forEach { $0.target = self }
        return menu
    }

    private func makeOutlineContextMenu() -> NSMenu {
        let menu = NSMenu()
        let item = NSMenuItem(title: L10n.contextRenameMove, action: #selector(renameSelectedFolder), keyEquivalent: "")
        item.target = self
        menu.addItem(item)
        return menu
    }

    // MARK: - 表单 / 错误

    /// 通用表单：标签 + 文本框若干，返回各框文本（顺序同 fields），取消返回 nil。
    private func runForm(title: String, okTitle: String,
                         fields: [(label: String, value: String, placeholder: String)]) -> [String]? {
        let alert = NSAlert()
        alert.messageText = title
        alert.addButton(withTitle: okTitle)
        alert.addButton(withTitle: L10n.btnCancel)

        let labelWidth: CGFloat = 56
        let fieldWidth: CGFloat = 340
        let rowH: CGFloat = 24, gap: CGFloat = 10
        let n = fields.count
        let totalH = CGFloat(n) * rowH + CGFloat(max(0, n - 1)) * gap
        let container = NSView(frame: NSRect(x: 0, y: 0, width: labelWidth + 8 + fieldWidth, height: totalH))

        var textFields: [NSTextField] = []
        for (i, f) in fields.enumerated() {
            let y = totalH - CGFloat(i + 1) * rowH - CGFloat(i) * gap
            let label = NSTextField(labelWithString: f.label)
            label.frame = NSRect(x: 0, y: y, width: labelWidth, height: rowH)
            label.alignment = .right
            let tf = NSTextField(string: f.value)
            tf.placeholderString = f.placeholder
            tf.frame = NSRect(x: labelWidth + 8, y: y, width: fieldWidth, height: rowH)
            container.addSubview(label)
            container.addSubview(tf)
            textFields.append(tf)
        }
        alert.accessoryView = container
        alert.window.initialFirstResponder = textFields.first

        guard alert.runModal() == .alertFirstButtonReturn else { return nil }
        return textFields.map { $0.stringValue }
    }

    private func showError(_ error: Error, title: String) {
        let alert = NSAlert()
        alert.messageText = title
        alert.informativeText = error.localizedDescription
        alert.alertStyle = .warning
        alert.runModal()
    }

    // MARK: - 菜单校验

    func validateMenuItem(_ item: NSMenuItem) -> Bool {
        switch item.action {
        case #selector(editSelected), #selector(openSelected), #selector(deleteSelected):
            return !targetBookmarks().isEmpty
        default:
            return true
        }
    }
}

// MARK: - Folder 树（NSOutlineView）

extension MainViewController: NSOutlineViewDataSource, NSOutlineViewDelegate {
    func outlineView(_: NSOutlineView, numberOfChildrenOfItem item: Any?) -> Int {
        guard let node = item as? FolderNode else { return folderRoots.count }
        return node.children.count
    }

    func outlineView(_: NSOutlineView, child index: Int, ofItem item: Any?) -> Any {
        guard let node = item as? FolderNode else { return folderRoots[index] }
        return node.children[index]
    }

    func outlineView(_: NSOutlineView, isItemExpandable item: Any) -> Bool {
        (item as? FolderNode).map { !$0.children.isEmpty } ?? false
    }

    func outlineView(_: NSOutlineView, viewFor _: NSTableColumn?, item: Any) -> NSView? {
        guard let node = item as? FolderNode else { return nil }
        let id = NSUserInterfaceItemIdentifier("FolderCell")
        let cell = (outlineView.makeView(withIdentifier: id, owner: self) as? NSTableCellView)
            ?? {
                let c = NSTableCellView()
                c.identifier = id
                let tf = NSTextField(labelWithString: "")
                tf.lineBreakMode = .byTruncatingTail
                tf.translatesAutoresizingMaskIntoConstraints = false
                c.addSubview(tf)
                c.textField = tf
                NSLayoutConstraint.activate([
                    tf.leadingAnchor.constraint(equalTo: c.leadingAnchor, constant: 2),
                    tf.trailingAnchor.constraint(equalTo: c.trailingAnchor, constant: -4),
                    tf.centerYAnchor.constraint(equalTo: c.centerYAnchor),
                ])
                return c
            }()
        cell.textField?.stringValue = "\(node.name)  (\(node.count))"
        cell.textField?.font = node.kind == .source
            ? .systemFont(ofSize: NSFont.systemFontSize, weight: .semibold)
            : .systemFont(ofSize: NSFont.systemFontSize)
        return cell
    }

    func outlineViewSelectionDidChange(_: Notification) {
        guard !isRestoring else { return }
        let row = outlineView.selectedRow
        selectedFolder = row >= 0 ? (outlineView.item(atRow: row) as? FolderNode) : nil
        saveSidebarState()
        recomputeVisible()
    }

    func outlineViewItemDidExpand(_: Notification) {
        if !isRestoring { saveSidebarState() }
    }

    func outlineViewItemDidCollapse(_: Notification) {
        if !isRestoring { saveSidebarState() }
    }
}

// MARK: - 书签列表（NSTableView）

extension MainViewController: NSTableViewDataSource, NSTableViewDelegate {
    func numberOfRows(in _: NSTableView) -> Int { visible.count }

    func tableView(_: NSTableView, viewFor _: NSTableColumn?, row: Int) -> NSView? {
        let cell = (tableView.makeView(withIdentifier: BookmarkCellView.reuseID, owner: self)
            as? BookmarkCellView) ?? {
                let c = BookmarkCellView()
                c.identifier = BookmarkCellView.reuseID
                return c
            }()
        cell.configure(with: visible[row])
        return cell
    }
}

// MARK: - 搜索

extension MainViewController: NSSearchFieldDelegate {
    func controlTextDidChange(_ obj: Notification) {
        guard (obj.object as? NSSearchField) === searchField else { return }
        searchText = searchField.stringValue
        recomputeVisible()
    }
}

// MARK: - 分栏约束（NSSplitView）

extension MainViewController: NSSplitViewDelegate {
    // 分隔条变化后立即记住左栏宽；初始化完成前的布局过程不落盘。
    func splitViewDidResizeSubviews(_ notification: Notification) {
        guard didSetInitialSplitPosition,
              (notification.object as? NSSplitView) === splitView
        else { return }
        UserDefaults.standard.set(sidebarScroll.frame.width, forKey: Self.sidebarWidthKey)
    }

    // 窗口缩放时固定左侧 folder 栏，只伸缩右侧列表栏。
    func splitView(_: NSSplitView, shouldAdjustSizeOfSubview subview: NSView) -> Bool {
        subview !== sidebarScroll
    }

    // 左侧 folder 栏最小宽。
    func splitView(_: NSSplitView, constrainMinCoordinate proposedMin: CGFloat, ofSubviewAt _: Int) -> CGFloat {
        max(proposedMin, sidebarMinWidth)
    }

    // 右侧列表栏最小宽（分隔条不能贴到右边缘把列表压没）。
    func splitView(_ splitView: NSSplitView, constrainMaxCoordinate proposedMax: CGFloat, ofSubviewAt _: Int) -> CGFloat {
        min(proposedMax, splitView.bounds.width - contentMinWidth)
    }
}
