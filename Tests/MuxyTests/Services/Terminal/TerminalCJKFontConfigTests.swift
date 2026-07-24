import GhosttyKit
import Testing

@testable import Muxy

@Suite("Terminal CJK font config")
struct TerminalCJKFontConfigTests {
    @Test("configured CJK fallback is mapped across Chinese codepoints")
    func mapsConfiguredCJKFallback() throws {
        let config = """
        font-family = JetBrains Mono
        font-family = "PingFang SC"
        """

        let generated = try #require(TerminalCJKFontConfig.configText(userConfig: config))

        #expect(generated.hasSuffix("=PingFang SC\n"))
        #expect(generated.contains("U+3000-U+303F"))
        #expect(generated.contains("U+3400-U+4DBF"))
        #expect(generated.contains("U+4E00-U+9FFF"))
        #expect(generated.contains("U+F900-U+FAFF"))
        #expect(generated.contains("U+FF00-U+FFEF"))
    }

    @Test("system CJK fallback is used when configured fonts lack Chinese glyphs")
    func mapsSystemCJKFallback() throws {
        let generated = try #require(TerminalCJKFontConfig.configText(userConfig: "font-family = Hiragino Sans"))
        let systemFallback = try #require(TerminalCJKFontConfig.configText(userConfig: ""))

        #expect(generated == systemFallback)
    }

    @Test("partial CJK font is skipped for a complete Chinese fallback")
    func skipsPartialCJKFont() throws {
        let config = """
        font-family = Hiragino Sans
        font-family = PingFang SC
        """

        let generated = try #require(TerminalCJKFontConfig.configText(userConfig: config))

        #expect(generated.hasSuffix("=PingFang SC\n"))
    }

    @Test("UTF-8 BOM is ignored before parsing the first font family")
    func ignoresByteOrderMark() {
        let config = "\u{FEFF}font-family = PingFang SC"

        #expect(TerminalCJKFontConfig.fontFamilies(in: config) == ["PingFang SC"])
    }

    @Test("font-family reset removes earlier fallback candidates")
    func honorsFontFamilyReset() {
        let config = """
        font-family = PingFang SC
        font-family = ""
        font-family = Menlo
        """

        #expect(TerminalCJKFontConfig.fontFamilies(in: config) == ["Menlo"])
    }

    @Test("similar Ghostty keys are not parsed as regular font families")
    func ignoresOtherFontFamilyKeys() {
        let config = """
        font-family-bold = PingFang SC
        font-family = Menlo
        """

        #expect(TerminalCJKFontConfig.fontFamilies(in: config) == ["Menlo"])
    }

    @Test("Ghostty accepts the generated codepoint mapping")
    @MainActor
    func generatedMappingParses() throws {
        _ = GhosttyService.shared
        let config = try #require(ghostty_config_new())
        defer { ghostty_config_free(config) }

        TerminalCJKFontConfig.load(into: config, userConfig: "font-family = PingFang SC")
        ghostty_config_finalize(config)

        #expect(ghostty_config_diagnostics_count(config) == 0)
    }
}
