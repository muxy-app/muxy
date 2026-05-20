import Testing
@testable import Muxy

struct GhosttyConfigFileTests {
    @Test("reads existing Ghostty config values")
    func readsExistingValue() {
        let content = "theme = Catppuccin\nmacos-option-as-alt = true\n"

        #expect(GhosttyConfigFile.value(for: "macos-option-as-alt", in: content) == "true")
    }

    @Test("setting value inserts missing key")
    func settingValueInsertsMissingKey() {
        let content = "theme = Catppuccin\n"

        let updated = GhosttyConfigFile.settingValue("left", for: "macos-option-as-alt", in: content)

        #expect(updated == "macos-option-as-alt = left\ntheme = Catppuccin\n")
    }

    @Test("setting value replaces existing key")
    func settingValueReplacesExistingKey() {
        let content = "theme = Catppuccin\nmacos-option-as-alt = false\n"

        let updated = GhosttyConfigFile.settingValue("true", for: "macos-option-as-alt", in: content)

        #expect(updated == "theme = Catppuccin\nmacos-option-as-alt = true\n")
    }

    @Test("removing value clears existing key")
    func removingValueClearsExistingKey() {
        let content = "theme = Catppuccin\nmacos-option-as-alt = true\n"

        let updated = GhosttyConfigFile.removingValue(for: "macos-option-as-alt", in: content)

        #expect(updated == "theme = Catppuccin\n")
    }

    @Test("missing and empty config values fall back to nil")
    func missingAndEmptyValuesFallBackToNil() {
        #expect(GhosttyConfigFile.value(for: "macos-option-as-alt", in: "") == nil)
        #expect(GhosttyConfigFile.value(for: "macos-option-as-alt", in: "theme = Catppuccin") == nil)
    }
}
