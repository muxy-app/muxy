import AppKit
import Foundation

@MainActor
enum FileClipboard {
    private static var lastWriteChangeCount: Int?
    private static var lastWriteWasCut = false

    struct Contents {
        let paths: [String]
        let isCut: Bool
    }

    static func write(paths: [String], isCut: Bool) {
        let pb = NSPasteboard.general
        pb.clearContents()
        let urls = paths.map { URL(fileURLWithPath: $0) as NSURL }
        pb.writeObjects(urls)
        lastWriteChangeCount = pb.changeCount
        lastWriteWasCut = isCut
    }

    static var hasContents: Bool {
        read() != nil
    }

    static func read() -> Contents? {
        let pb = NSPasteboard.general
        guard let urls = pb.readObjects(forClasses: [NSURL.self], options: [.urlReadingFileURLsOnly: true]) as? [URL],
              !urls.isEmpty
        else { return nil }
        let isCut = pb.changeCount == lastWriteChangeCount && lastWriteWasCut
        return Contents(paths: urls.map(\.path), isCut: isCut)
    }

    static func clearCutMarker() {
        lastWriteChangeCount = nil
        lastWriteWasCut = false
    }
}
