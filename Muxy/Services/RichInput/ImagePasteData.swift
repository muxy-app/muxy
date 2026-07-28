import AppKit

@MainActor
enum ImagePasteData {
    static func hasImage(in pasteboard: NSPasteboard = .general) -> Bool {
        guard pasteboard.string(forType: .string) == nil else { return false }
        return pasteboard.canReadObject(forClasses: [NSImage.self], options: nil)
    }

    static func pngData(from pasteboard: NSPasteboard = .general) -> Data? {
        guard hasImage(in: pasteboard) else { return nil }
        if let data = pasteboard.data(forType: .png) {
            return data
        }
        guard let image = NSImage(pasteboard: pasteboard) else { return nil }
        return pngData(from: image)
    }

    static func pngData(contentsOf url: URL) -> Data? {
        guard let data = try? Data(contentsOf: url) else { return nil }
        if data.detectedImageExtension == "png" {
            return data
        }
        guard let image = NSImage(data: data) else { return nil }
        return pngData(from: image)
    }

    private static func pngData(from image: NSImage) -> Data? {
        guard let tiffData = image.tiffRepresentation,
              let representation = NSBitmapImageRep(data: tiffData)
        else { return nil }
        return representation.representation(using: .png, properties: [:])
    }
}
