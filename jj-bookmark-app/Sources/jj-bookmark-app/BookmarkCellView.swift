import AppKit
import PublicSuffixList

// 书签列表单元格：标题（主）+ 「完整 URL · folder · 日期」（次）两行；仅可注册主域名醒目显示。
final class BookmarkCellView: NSTableCellView {
    static let reuseID = NSUserInterfaceItemIdentifier("BookmarkCell")

    private let titleLabel = NSTextField(labelWithString: "")
    private let subtitleLabel = NSTextField(labelWithString: "")
    private var url = ""
    private var metadata = ""

    override var backgroundStyle: NSView.BackgroundStyle {
        didSet { updateSubtitle() }
    }

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        titleLabel.font = .systemFont(ofSize: 13, weight: .medium)
        titleLabel.lineBreakMode = .byTruncatingTail
        titleLabel.cell?.usesSingleLineMode = true

        subtitleLabel.font = .systemFont(ofSize: 11)
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
        url = b.url
        var parts: [String] = []
        if !b.folder.isEmpty { parts.append(b.folder) }
        let date = String(b.createdJst.prefix(10)) // YYYY-MM-DD
        if !date.isEmpty { parts.append(date) }
        metadata = parts.joined(separator: "  ·  ")
        updateSubtitle()
        titleLabel.toolTip = b.title
        subtitleLabel.toolTip = b.url
    }

    private func updateSubtitle() {
        let text = metadata.isEmpty ? url : "\(url)  ·  \(metadata)"
        let selected = backgroundStyle == .emphasized
        let attributed = NSMutableAttributedString(string: text, attributes: [
            .font: NSFont.systemFont(ofSize: 11),
            .foregroundColor: selected ? NSColor.alternateSelectedControlTextColor : .secondaryLabelColor,
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
