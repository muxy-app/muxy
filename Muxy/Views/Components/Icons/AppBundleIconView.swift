import AppKit
import SwiftUI

@MainActor
struct AppBundleIconView: View {
    let appURL: URL
    let fallbackSystemName: String
    var size: CGFloat = 16

    var body: some View {
        if let image = AppBundleIconCache.shared.image(for: appURL, size: size) {
            Image(nsImage: image)
                .interpolation(.high)
                .antialiased(true)
        } else {
            Image(systemName: fallbackSystemName)
                .font(.system(size: size * 0.85, weight: .semibold))
                .foregroundStyle(.secondary)
                .frame(width: size, height: size)
        }
    }
}

@MainActor
private final class AppBundleIconCache {
    static let shared = AppBundleIconCache()

    private let cache = NSCache<NSString, NSImage>()

    func image(for appURL: URL, size: CGFloat) -> NSImage? {
        let pixelSize = max(1, Int(size.rounded()))
        let key = "\(appURL.path)#\(pixelSize)" as NSString
        if let cached = cache.object(forKey: key) {
            return cached
        }

        let source = NSWorkspace.shared.icon(forFile: appURL.path)
        let targetSize = NSSize(width: pixelSize, height: pixelSize)
        let image = NSImage(size: targetSize)
        image.lockFocus()
        defer { image.unlockFocus() }

        NSGraphicsContext.current?.imageInterpolation = .high
        source.draw(
            in: NSRect(origin: .zero, size: targetSize),
            from: NSRect(origin: .zero, size: source.size),
            operation: .sourceOver,
            fraction: 1.0,
            respectFlipped: true,
            hints: [.interpolation: NSImageInterpolation.high]
        )
        image.size = targetSize
        cache.setObject(image, forKey: key)
        return image
    }
}
