import Foundation

enum MuxyNotificationHooks {
    static var hookScriptPath: String? {
        if let bundled = Bundle.main.resourceURL?
            .appendingPathComponent("scripts/muxy-claude-hook.sh").path,
            FileManager.default.isExecutableFile(atPath: bundled)
        {
            return bundled
        }

        let devPath = findDevScriptPath()
        if let devPath, FileManager.default.isExecutableFile(atPath: devPath) {
            return devPath
        }

        return nil
    }

    private static func findDevScriptPath() -> String? {
        guard let execURL = Bundle.main.executableURL else { return nil }
        var dir = execURL.deletingLastPathComponent()
        for _ in 0 ..< 10 {
            let candidate = dir.appendingPathComponent("scripts/muxy-claude-hook.sh")
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
