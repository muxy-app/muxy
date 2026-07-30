import AppKit
import Testing

@testable import Muxy

@Suite("Extension theme snapshot")
@MainActor
struct ExtensionThemeSnapshotTests {
    @Test("solid surface composites the native surface over the active background")
    func solidSurfaceMatchesActiveTheme() {
        let theme = ExtensionThemeSnapshot.current()
        let expected = ExtensionThemeSnapshot.opaque(MuxyTheme.nsSurface, over: MuxyTheme.nsBg)

        #expect(theme["surfaceSolid"] == hex(expected))
        #expect(theme["surfaceSolid"]?.count == 7)
        #expect(theme["surface"]?.count == 9)
    }

    @Test("accent foreground matches the native active theme")
    func accentForegroundMatchesActiveTheme() {
        let theme = ExtensionThemeSnapshot.current()

        #expect(theme["accentForeground"] == hex(MuxyTheme.nsAccentForeground))
    }

    @Test("opaque compositing resolves source alpha")
    func opaqueCompositingResolvesSourceAlpha() {
        let overlay = NSColor(srgbRed: 0.8, green: 0.4, blue: 0.2, alpha: 0.25)
        let background = NSColor(srgbRed: 0.2, green: 0.3, blue: 0.5, alpha: 1)

        let result = ExtensionThemeSnapshot.opaque(overlay, over: background)
        let resolved = result.usingColorSpace(.sRGB)

        #expect(resolved != nil)
        #expect(abs((resolved?.redComponent ?? 0) - 0.35) < 0.000_1)
        #expect(abs((resolved?.greenComponent ?? 0) - 0.325) < 0.000_1)
        #expect(abs((resolved?.blueComponent ?? 0) - 0.425) < 0.000_1)
        #expect(resolved?.alphaComponent == 1)
    }

    private func hex(_ color: NSColor) -> String {
        let resolved = color.usingColorSpace(.sRGB) ?? color
        return String(
            format: "#%02x%02x%02x",
            Int(round(resolved.redComponent * 255)),
            Int(round(resolved.greenComponent * 255)),
            Int(round(resolved.blueComponent * 255))
        )
    }
}
