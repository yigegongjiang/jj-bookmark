import AppKit

// 转发 Delete / Enter 键给回调（NSTableView 默认不把这些键给 delegate）。
final class BookmarkTableView: NSTableView {
    var onDelete: (() -> Void)?
    var onEnter: (() -> Void)?

    override func keyDown(with event: NSEvent) {
        switch event.keyCode {
        case 51, 117: // delete / forward-delete
            onDelete?()
        case 36, 76: // return / enter
            onEnter?()
        default:
            super.keyDown(with: event)
        }
    }
}
