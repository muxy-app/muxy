import AppKit
import Foundation
import os

private let cacheLogger = Logger(subsystem: "app.muxy", category: "BrowserAnnotationCache")

@MainActor
final class BrowserAnnotationCacheStore {
    static let shared = BrowserAnnotationCacheStore()

    private let fileManager: FileManager
    private let directoryName = "browser-annotations"
    private let bundleSubdirectory = "app.muxy"

    init(fileManager: FileManager = .default) {
        self.fileManager = fileManager
    }

    var directoryURL: URL? {
        guard let cachesRoot = fileManager.urls(for: .cachesDirectory, in: .userDomainMask).first else {
            return nil
        }
        let directory = cachesRoot
            .appendingPathComponent(bundleSubdirectory, isDirectory: true)
            .appendingPathComponent(directoryName, isDirectory: true)
        do {
            try fileManager.createDirectory(at: directory, withIntermediateDirectories: true)
        } catch {
            cacheLogger.error("Failed to create cache directory: \(error.localizedDescription, privacy: .public)")
            return nil
        }
        return directory
    }

    func write(image: NSImage, id: UUID) -> URL? {
        guard let directory = directoryURL else { return nil }
        guard let data = pngData(from: image) else { return nil }
        let url = directory.appendingPathComponent("\(id.uuidString).png")
        do {
            try data.write(to: url, options: .atomic)
            return url
        } catch {
            cacheLogger.error("Failed to write screenshot: \(error.localizedDescription, privacy: .public)")
            return nil
        }
    }

    func remove(id: UUID) {
        guard let directory = directoryURL else { return }
        let url = directory.appendingPathComponent("\(id.uuidString).png")
        try? fileManager.removeItem(at: url)
    }

    func removeAll(olderThan interval: TimeInterval) {
        guard let directory = directoryURL else { return }
        let cutoff = Date().addingTimeInterval(-interval)
        let contents = (try? fileManager.contentsOfDirectory(
            at: directory,
            includingPropertiesForKeys: [.contentModificationDateKey]
        )) ?? []
        for url in contents {
            let resourceValues = try? url.resourceValues(forKeys: [.contentModificationDateKey])
            guard let modified = resourceValues?.contentModificationDate else { continue }
            if modified < cutoff {
                try? fileManager.removeItem(at: url)
            }
        }
    }

    private func pngData(from image: NSImage) -> Data? {
        guard let tiff = image.tiffRepresentation,
              let rep = NSBitmapImageRep(data: tiff)
        else { return nil }
        return rep.representation(using: .png, properties: [:])
    }
}
