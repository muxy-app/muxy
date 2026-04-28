import AppKit
import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "ProjectLogoStorage")

enum ProjectLogoStorage {
    private static func logosDirectory() -> URL {
        let dir = MuxyFileStorage.appSupportDirectory()
            .appendingPathComponent("logos", isDirectory: true)
        try? FileManager.default.createDirectory(
            at: dir,
            withIntermediateDirectories: true,
            attributes: [.posixPermissions: FilePermissions.privateDirectory]
        )
        return dir
    }

    static func save(croppedImage image: NSImage, forProjectID projectID: UUID) -> String? {
        guard let tiffData = image.tiffRepresentation,
              let bitmap = NSBitmapImageRep(data: tiffData),
              let pngData = bitmap.representation(using: .png, properties: [:])
        else {
            logger.error("Failed to convert cropped image to PNG")
            return nil
        }

        let filename = "\(projectID.uuidString).png"
        let destURL = logosDirectory()
            .appendingPathComponent(filename)

        do {
            if FileManager.default.fileExists(atPath: destURL.path) {
                try FileManager.default.removeItem(at: destURL)
            }
            try pngData.write(to: destURL, options: .atomic)
            return filename
        } catch {
            logger.error("Failed to save project logo: \(error)")
            return nil
        }
    }

    static func logoPath(for filename: String) -> String {
        logosDirectory()
            .appendingPathComponent(filename)
            .path
    }

    static func remove(forProjectID projectID: UUID) {
        let dir = logosDirectory()
        guard let contents = try? FileManager.default.contentsOfDirectory(
            at: dir,
            includingPropertiesForKeys: nil
        )
        else { return }

        let prefix = projectID.uuidString
        for file in contents where file.deletingPathExtension().lastPathComponent == prefix {
            try? FileManager.default.removeItem(at: file)
        }
    }
}
