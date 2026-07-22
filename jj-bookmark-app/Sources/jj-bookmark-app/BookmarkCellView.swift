import AppKit
import PublicSuffixList

// 书签列表单元格：标题（主）+ 「完整 URL · source · folder · 日期」（次），excerpt / note 各非空时再各占一行（单行截断）。
// 行高由 MainViewController.tableView(_:heightOfRow:) 按 excerpt/note 是否存在确定，空则该 label isHidden 自动塌陷。
final class BookmarkCellView: NSTableCellView {
    static let reuseID = NSUserInterfaceItemIdentifier("BookmarkCell")

    private let titleLabel = NSTextField(labelWithString: "")
    private let subtitleLabel = NSTextField(labelWithString: "")
    private let excerptLabel = NSTextField(labelWithString: "")
    private let noteLabel = NSTextField(labelWithString: "")
    private let stack: NSStackView
    private var url = ""
    private var metadata = ""

    override var backgroundStyle: NSView.BackgroundStyle {
        didSet { applyColors() }
    }

    override init(frame frameRect: NSRect) {
        titleLabel.font = .systemFont(ofSize: 13, weight: .medium)
        titleLabel.lineBreakMode = .byTruncatingTail
        titleLabel.cell?.usesSingleLineMode = true

        subtitleLabel.font = .systemFont(ofSize: 11)
        subtitleLabel.lineBreakMode = .byTruncatingTail
        subtitleLabel.cell?.usesSingleLineMode = true

        for label in [excerptLabel, noteLabel] {
            label.font = .systemFont(ofSize: 11)
            label.lineBreakMode = .byTruncatingTail
            label.cell?.usesSingleLineMode = true
        }

        stack = NSStackView(views: [titleLabel, subtitleLabel, excerptLabel, noteLabel])
        super.init(frame: frameRect)
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
        url = b.url
        var parts = [b.source]
        if !b.folder.isEmpty { parts.append(b.folder) }
        let date = String(b.createdJst.prefix(10)) // YYYY-MM-DD
        if !date.isEmpty { parts.append(date) }
        metadata = parts.joined(separator: "  ·  ")

        let excerpt = b.excerpt.trimmingCharacters(in: .whitespacesAndNewlines)
        excerptLabel.stringValue = excerpt
        excerptLabel.isHidden = excerpt.isEmpty
        excerptLabel.toolTip = excerpt.isEmpty ? nil : excerpt

        let note = b.note.trimmingCharacters(in: .whitespacesAndNewlines)
        noteLabel.stringValue = note.isEmpty ? "" : "✎ \(note)" // ✎ 前缀区分 note（用户批注）与 excerpt（页面摘要）
        noteLabel.isHidden = note.isEmpty
        noteLabel.toolTip = note.isEmpty ? nil : note

        applyColors()
        titleLabel.toolTip = b.title.isEmpty ? nil : b.title
        subtitleLabel.toolTip = b.url
    }

    private func applyColors() {
        let selected = backgroundStyle == .emphasized
        let secondary: NSColor = selected ? .alternateSelectedControlTextColor : .secondaryLabelColor
        excerptLabel.textColor = secondary
        noteLabel.textColor = secondary

        let text = metadata.isEmpty ? url : "\(url)  ·  \(metadata)"
        let attributed = NSMutableAttributedString(string: text, attributes: [
            .font: NSFont.systemFont(ofSize: 11),
            .foregroundColor: secondary,
        ])
        if let domainRange = registrableDomainRange(in: url) {
            attributed.addAttributes([
                .font: NSFont.systemFont(ofSize: 11, weight: .semibold),
                .foregroundColor: selected ? NSColor.alternateSelectedControlTextColor : .systemRed,
            ], range: NSRange(domainRange, in: url))
        }
        subtitleLabel.attributedStringValue = attributed
    }

    private func registrableDomainRange(in url: String) -> Range<String.Index>? {
        let components = URLComponents(string: url)
        let fallbackComponents = URLComponents(string: "https://\(url)")
        guard let host = components?.host ?? fallbackComponents?.host,
              let domain = PublicSuffixList.effectiveTLDPlusOne(host)
        else { return nil }

        let hostRange = components?.rangeOfHost
            ?? url.range(of: host, options: [.caseInsensitive])
        guard let hostRange else { return nil }
        return url.range(
            of: domain,
            options: [.caseInsensitive, .anchored, .backwards],
            range: hostRange
        )
    }
}
