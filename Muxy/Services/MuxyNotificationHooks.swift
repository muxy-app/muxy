import Foundation
import os

private let logger = Logger(subsystem: "app.muxy", category: "MuxyNotificationHooks")

enum MuxyNotificationHooks {
    private static let hookScriptName = "muxy-claude-hook"

    static var hookScriptPath: String? {
        if let bundled = findBundledScript(hookScriptName, extension: "sh") {
            return bundled
        }

        let devPath = findDevScriptPath(hookScriptName + ".sh")
        if let devPath, FileManager.default.isExecutableFile(atPath: devPath) {
            return devPath
        }

        return nil
    }

    static func scriptPath(named name: String, extension ext: String) -> String? {
        if let bundled = findBundledScript(name, extension: ext) {
            return bundled
        }

        let devPath = findDevScriptPath(name + "." + ext)
        if let devPath, FileManager.default.fileExists(atPath: devPath) {
            return devPath
        }

        return nil
    }

    private static func findBundledScript(_ name: String, extension ext: String) -> String? {
        guard let url = Bundle.appResources.url(forResource: name, withExtension: ext) else {
            return nil
        }

        let path = url.path
        guard FileManager.default.fileExists(atPath: path) else { return nil }

        if !FileManager.default.isExecutableFile(atPath: path) {
            do {
                try FileManager.default.setAttributes(
                    [.posixPermissions: FilePermissions.executable],
                    ofItemAtPath: path
                )
            } catch {
                logger.error("Failed to set executable permission on \(path): \(error.localizedDescription)")
                return nil
            }
        }

        return path
    }

    private static func findDevScriptPath(_ fileName: String) -> String? {
        guard let execURL = Bundle.main.executableURL else { return nil }
        var dir = execURL.deletingLastPathComponent()
        for _ in 0 ..< 10 {
            let candidate = dir.appendingPathComponent("scripts/\(fileName)")
            if FileManager.default.fileExists(atPath: candidate.path) {
                return candidate.path
            }
            let parent = dir.deletingLastPathComponent()
            guard parent.path != dir.path else { break }
            dir = parent
        }
        return nil
    }
}
