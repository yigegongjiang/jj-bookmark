import AppKit

// 书签列表单元格：标题（主）+ 「域名 · folder · 日期」（次）两行。
final class BookmarkCellView: NSTableCellView {
    static let reuseID = NSUserInterfaceItemIdentifier("BookmarkCell")

    private let titleLabel = NSTextField(labelWithString: "")
    private let subtitleLabel = NSTextField(labelWithString: "")

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        titleLabel.font = .systemFont(ofSize: 13, weight: .medium)
        titleLabel.lineBreakMode = .byTruncatingTail
        titleLabel.cell?.usesSingleLineMode = true
        subtitleLabel.font = .systemFont(ofSize: 11)
        subtitleLabel.textColor = .secondaryLabelColor
        subtitleLabel.lineBreakMode = .byTruncatingTail
        subtitleLabel.cell?.usesSingleLineMode = true

        let stack = NSStackView(views: [titleLabel, subtitleLabel])
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 2
        stack.translatesAutoresizingMaskIntoConstraints = false
        addSubview(stack)
        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 8),
            stack.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -8),
            stack.centerYAnchor.constraint(equalTo: centerYAnchor),
        ])
    }

    @available(*, unavailable)
    required init?(coder _: NSCoder) { fatalError("not used") }

    func configure(with b: Bookmark) {
        titleLabel.stringValue = b.title.isEmpty ? b.url : b.title
        var parts = [b.domain]
        if !b.folder.isEmpty { parts.append(b.folder) }
        let date = String(b.createdJst.prefix(10)) // YYYY-MM-DD
        if !date.isEmpty { parts.append(date) }
        subtitleLabel.stringValue = parts.joined(separator: "  ·  ")
        titleLabel.toolTip = b.title
        subtitleLabel.toolTip = b.url
    }
}
