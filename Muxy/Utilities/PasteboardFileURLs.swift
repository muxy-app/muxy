import AppKit

enum PasteboardFileURLs {
    static func urls(in pasteboard: NSPasteboard) -> [URL] {
        let options: [NSPasteboard.ReadingOptionKey: Any] = [.urlReadingFileURLsOnly: true]
        let objects = pasteboard.readObjects(forClasses: [NSURL.self], options: options) as? [URL]
        return objects?.filter(\.isFileURL) ?? []
    }
}
