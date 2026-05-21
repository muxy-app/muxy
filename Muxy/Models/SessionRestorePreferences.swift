import Foundation

enum SessionRestorePreferences {
    static let enabledKey = "muxy.sessionRestore.enabled"
    static let excludedCommandsKey = "muxy.sessionRestore.excludedCommands"

    static let maxSnapshots = 200

    static var isEnabled: Bool {
        get {
            let defaults = UserDefaults.standard
            if defaults.object(forKey: enabledKey) == nil { return true }
            return defaults.bool(forKey: enabledKey)
        }
        set { UserDefaults.standard.set(newValue, forKey: enabledKey) }
    }

    static var excludedCommands: [String] {
        get { UserDefaults.standard.stringArray(forKey: excludedCommandsKey) ?? defaultExcludedCommands }
        set { UserDefaults.standard.set(newValue, forKey: excludedCommandsKey) }
    }

    static var excludedCommandsText: String {
        get { excludedCommands.joined(separator: "\n") }
        set {
            excludedCommands = newValue
                .components(separatedBy: .newlines)
                .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
                .filter { !$0.isEmpty }
        }
    }

    static let defaultExcludedCommands = [
        "rm",
        "rmdir",
        "mv",
        "git push --force",
        "git push -f",
        "git reset --hard",
        "git clean",
        "docker system prune",
        "sudo",
    ]
}
