import AppKit

@MainActor
enum SystemPasteboardSnapshot {
    static func capture() -> [NSPasteboardItem] {
        guard let items = NSPasteboard.general.pasteboardItems else { return [] }
        return items.map { source in
            let copy = NSPasteboardItem()
            for type in source.types {
                if let data = source.data(forType: type) {
                    copy.setData(data, forType: type)
                }
            }
            return copy
        }
    }

    static func restore(items: [NSPasteboardItem]) {
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        guard !items.isEmpty else { return }
        pasteboard.writeObjects(items)
    }
}
