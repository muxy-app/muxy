import Testing

@testable import Muxy

@Suite("ThemeService")
struct ThemeServiceTests {
    @Test("parseThemeSelection preserves single theme names")
    func parseSingleThemeName() {
        let selection = ThemeService.parseThemeSelection("\"Muxy\"")

        #expect(selection.displayName == "Muxy")
        #expect(selection.resolvedName(isDark: true) == "Muxy")
        #expect(selection.resolvedName(isDark: false) == "Muxy")
    }

    @Test("parseThemeSelection resolves paired dark and light theme names")
    func parsePairedThemeNames() {
        let selection = ThemeService.parseThemeSelection("dark:\"Muxy\",light:\"Muxy Light\"")

        #expect(selection.displayName == "Dark: Muxy, Light: Muxy Light")
        #expect(selection.resolvedName(isDark: true) == "Muxy")
        #expect(selection.resolvedName(isDark: false) == "Muxy Light")
    }

    @Test("parseThemeSelection falls back when one side is missing")
    func parsePartialThemePair() {
        let selection = ThemeService.parseThemeSelection("dark:\"Muxy\"")

        #expect(selection.displayName == selection.rawValue)
        #expect(selection.resolvedName(isDark: true) == "Muxy")
        #expect(selection.resolvedName(isDark: false) == "Muxy")
    }

    @Test("parseThemeSelection ignores commas inside quoted names")
    func parseQuotedCommaThemeName() {
        let selection = ThemeService.parseThemeSelection("dark:\"Dark, Variant\",light:\"Light\"")

        #expect(selection.resolvedName(isDark: true) == "Dark, Variant")
        #expect(selection.resolvedName(isDark: false) == "Light")
    }
}
