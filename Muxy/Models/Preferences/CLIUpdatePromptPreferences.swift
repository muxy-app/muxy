import Foundation

enum CLIUpdatePromptPreferences {
    static let suppressedFormatVersionKey = "muxy.cli.suppressedUpdatePromptVersion"

    static func shouldPrompt(
        for formatVersion: Int,
        defaults: UserDefaults = .standard
    ) -> Bool {
        defaults.integer(forKey: suppressedFormatVersionKey) < formatVersion
    }

    static func suppress(
        formatVersion: Int,
        defaults: UserDefaults = .standard
    ) {
        defaults.set(formatVersion, forKey: suppressedFormatVersionKey)
    }
}
