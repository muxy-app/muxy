import Foundation

enum RepositoryRoot {
    static func find(from filePath: String = #filePath) -> URL {
        var url = URL(fileURLWithPath: filePath)
        while url.pathComponents.count > 1 {
            url.deleteLastPathComponent()
            let packageManifest = url.appendingPathComponent("Package.swift")
            if FileManager.default.fileExists(atPath: packageManifest.path) {
                return url
            }
        }
        return URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
    }
}
