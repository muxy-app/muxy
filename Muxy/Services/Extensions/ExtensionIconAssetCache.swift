import AppKit
import Foundation

@MainActor
final class ExtensionIconAssetCache {
    static let shared = ExtensionIconAssetCache()

    private var images: [String: NSImage] = [:]

    private init() {}

    func image(extensionID: String, url: URL) -> NSImage? {
        let key = cacheKey(extensionID: extensionID, url: url)
        if let cached = images[key] {
            return cached
        }
        guard let image = NSImage(contentsOf: url) else { return nil }
        image.isTemplate = true
        images[key] = image
        return image
    }

    func invalidate(extensionID: String) {
        let prefix = "\(extensionID)\t"
        images = images.filter { !$0.key.hasPrefix(prefix) }
    }

    func invalidateAll() {
        images.removeAll()
    }

    private func cacheKey(extensionID: String, url: URL) -> String {
        "\(extensionID)\t\(url.path)"
    }
}
