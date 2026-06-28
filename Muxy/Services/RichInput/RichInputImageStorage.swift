import Foundation

enum RichInputImageStorage {
    static func directoryURL() -> URL {
        MuxyFileStorage.appSupportDirectory()
            .appendingPathComponent("RichInputImages", isDirectory: true)
    }

    static func write(imageData: Data) -> URL? {
        let dir = directoryURL()
        do {
            try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        } catch {
            return nil
        }
        let ext = imageData.detectedImageExtension ?? "png"
        let url = dir.appendingPathComponent(UUID().uuidString + "." + ext)
        do {
            try imageData.write(to: url)
        } catch {
            return nil
        }
        return url
    }

    static func removeOrphans(referencedFilenames: Set<String>) {
        let dir = directoryURL()
        guard let entries = try? FileManager.default.contentsOfDirectory(
            at: dir,
            includingPropertiesForKeys: nil
        )
        else { return }
        for url in entries where !referencedFilenames.contains(url.lastPathComponent) {
            try? FileManager.default.removeItem(at: url)
        }
    }
}

extension Data {
    var detectedImageExtension: String? {
        guard count >= 8 else { return nil }
        let bytes = [UInt8](prefix(8))
        if bytes.starts(with: [0x89, 0x50, 0x4E, 0x47]) { return "png" }
        if bytes.starts(with: [0xFF, 0xD8, 0xFF]) { return "jpg" }
        if bytes.starts(with: [0x47, 0x49, 0x46, 0x38]) { return "gif" }
        if bytes.starts(with: [0x49, 0x49]) || bytes.starts(with: [0x4D, 0x4D]) { return "tiff" }
        if count >= 12, Array(self[8 ..< 12]) == [0x57, 0x45, 0x42, 0x50] { return "webp" }
        return nil
    }
}
