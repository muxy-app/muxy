import Foundation
import Testing

@testable import Muxy

@Suite("CLIUpdatePromptPreferences")
struct CLIUpdatePromptPreferencesTests {
    @Test("suppression applies through the dismissed wrapper format")
    func suppressionIsVersionSpecific() throws {
        let suiteName = "CLIUpdatePromptPreferencesTests.\(UUID().uuidString)"
        let defaults = try #require(UserDefaults(suiteName: suiteName))
        defer { defaults.removePersistentDomain(forName: suiteName) }

        #expect(CLIUpdatePromptPreferences.shouldPrompt(for: 1, defaults: defaults))

        CLIUpdatePromptPreferences.suppress(formatVersion: 2, defaults: defaults)

        #expect(!CLIUpdatePromptPreferences.shouldPrompt(for: 1, defaults: defaults))
        #expect(!CLIUpdatePromptPreferences.shouldPrompt(for: 2, defaults: defaults))
        #expect(CLIUpdatePromptPreferences.shouldPrompt(for: 3, defaults: defaults))
    }
}
