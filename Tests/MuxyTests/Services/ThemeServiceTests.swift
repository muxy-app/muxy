import AppKit
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

    @Test("parseThemeSelection handles unquoted bare name")
    func parseUnquotedBareName() {
        let selection = ThemeService.parseThemeSelection("Muxy")

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

    @Test("parseThemeSelection falls back when dark side is missing")
    func parseDarkOnlyPair() {
        let selection = ThemeService.parseThemeSelection("dark:\"Muxy\"")

        #expect(selection.displayName == selection.rawValue)
        #expect(selection.resolvedName(isDark: true) == "Muxy")
        #expect(selection.resolvedName(isDark: false) == "Muxy")
    }

    @Test("parseThemeSelection falls back when light side is missing")
    func parseLightOnlyPair() {
        let selection = ThemeService.parseThemeSelection("light:\"Muxy Light\"")

        #expect(selection.displayName == selection.rawValue)
        #expect(selection.resolvedName(isDark: true) == "Muxy Light")
        #expect(selection.resolvedName(isDark: false) == "Muxy Light")
    }

    @Test("parseThemeSelection ignores commas inside quoted names")
    func parseQuotedCommaThemeName() {
        let selection = ThemeService.parseThemeSelection("dark:\"Dark, Variant\",light:\"Light\"")

        #expect(selection.resolvedName(isDark: true) == "Dark, Variant")
        #expect(selection.resolvedName(isDark: false) == "Light")
    }

    @Test("parseThemeSelection treats unknown keys as fallback")
    func parseUnknownKey() {
        let selection = ThemeService.parseThemeSelection("foo:\"Bar\"")

        #expect(selection.darkName == nil)
        #expect(selection.lightName == nil)
        #expect(selection.resolvedName(isDark: true) != nil)
        #expect(selection.resolvedName(isDark: false) != nil)
    }

    @Test("parseThemeSelection handles empty string")
    func parseEmptyString() {
        let selection = ThemeService.parseThemeSelection("")

        #expect(selection.darkName == nil)
        #expect(selection.lightName == nil)
        #expect(selection.resolvedName(isDark: true) == "")
        #expect(selection.resolvedName(isDark: false) == "")
    }

    @Test("isDarkAppearance uses user defaults before AppKit appearance")
    func isDarkAppearanceUsesUserDefaults() throws {
        let darkAppearance = try #require(NSAppearance(named: .darkAqua))
        let lightAppearance = try #require(NSAppearance(named: .aqua))

        #expect(ThemeService.isDarkAppearance(userInterfaceStyle: "dark", effectiveAppearance: lightAppearance))
        #expect(!ThemeService.isDarkAppearance(userInterfaceStyle: "light", effectiveAppearance: darkAppearance))
    }

    @Test("isDarkAppearance uses effective appearance")
    func isDarkAppearanceUsesEffectiveAppearance() throws {
        let darkAppearance = try #require(NSAppearance(named: .darkAqua))
        let lightAppearance = try #require(NSAppearance(named: .aqua))

        #expect(ThemeService.isDarkAppearance(userInterfaceStyle: nil, effectiveAppearance: darkAppearance))
        #expect(!ThemeService.isDarkAppearance(userInterfaceStyle: nil, effectiveAppearance: lightAppearance))
    }

    @Test("isDarkAppearance defaults to light when effective appearance is missing")
    func isDarkAppearanceDefaultsToLightWhenAppearanceIsMissing() {
        #expect(!ThemeService.isDarkAppearance(userInterfaceStyle: nil, effectiveAppearance: nil))
    }

    @Suite("Migration math")
    struct MigrationTests {
        private func migrationResult(
            for rawValue: String,
            defaultTheme: String = "Muxy"
        ) -> (dark: String, light: String) {
            let selection = ThemeService.parseThemeSelection(rawValue)
            let unified = selection.darkName ?? selection.lightName ?? selection.fallbackName ?? defaultTheme
            return (selection.darkName ?? unified, selection.lightName ?? unified)
        }

        @Test("single quoted name migrates to identical dark and light")
        func migrateSingleName() {
            let result = migrationResult(for: "\"Dracula\"")

            #expect(result.dark == "Dracula")
            #expect(result.light == "Dracula")
        }

        @Test("dark-only config mirrors dark theme to light side")
        func migrateDarkOnly() {
            let result = migrationResult(for: "dark:\"Dracula\"")

            #expect(result.dark == "Dracula")
            #expect(result.light == "Dracula")
        }

        @Test("light-only config mirrors light theme to dark side")
        func migrateLightOnly() {
            let result = migrationResult(for: "light:\"Muxy Light\"")

            #expect(result.dark == "Muxy Light")
            #expect(result.light == "Muxy Light")
        }

        @Test("already paired config has both sides non-nil, skipping migration")
        func alreadyPaired() {
            let selection = ThemeService.parseThemeSelection("dark:\"Dracula\",light:\"Muxy Light\"")

            #expect(selection.darkName != nil)
            #expect(selection.lightName != nil)
        }

        @Test("unquoted single name migrates to identical dark and light")
        func migrateUnquotedName() {
            let result = migrationResult(for: "Dracula")

            #expect(result.dark == "Dracula")
            #expect(result.light == "Dracula")
        }
    }
}
